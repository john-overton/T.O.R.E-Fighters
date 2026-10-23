//! Autobalance (slice A1 of the lobby pass; setting 7's `balanced`, John
//! 2026-10-09): the host picks each player's side in a PvP game, and players
//! cannot change it. The rule is docs/ARCHITECTURE.md's "Autobalance as
//! built (A1)".
//!
//! - **Who counts:** human players holding a slot in the lobby, or taking
//!   or flying a plane, per side; in flight also a player back in the lobby
//!   whose first plane fixed its side. Observers and players whose game
//!   cannot play the mission are neither counted nor seated.
//! - **Seating** (a joiner, anyone the host freed, everyone without a slot
//!   when balance is turned on): the side with fewer humans; on a tie the
//!   side with more slots free to the player; on a tie Bluefor. A side with
//!   no free slot is skipped (uneven slot counts), and the player takes that
//!   side's lowest-numbered free slot. A player who fits nowhere waits
//!   without a slot ("Both sides are full.") and is seated when one frees.
//! - **Turning balance on** re-deals with the fewest moves: players without
//!   a slot are seated first; then, while one side has two or more humans
//!   more than the other, the most recently joined player of the larger
//!   side moves, the King and the house last. A mover loses its ready mark,
//!   and its loadout when the aircraft differs.
//! - **A player leaving** re-deals nothing (John, 2026-10-09): the next
//!   joiner goes to the smaller side; the King re-deals by turning balance
//!   off and on.
//! - **In flight** a late joiner is seated the same way on a free AI
//!   aircraft, for Join; a player whose first plane fixed its side keeps it
//!   (lock sides' rule), which revivals follow too.
//!
//! Nothing here keeps state: the lobby's slots and the court's sides are
//! the whole of it, so the session's exact checkpoints need nothing new.
//! Every choice above the John-marked ones is an agent decision (plan 6.3).

use super::{ConnectionId, Host, Life, LobbyEvent, Stage};
use crate::settings::side_name;
use crate::wire::messages::Message;
use std::cmp::Reverse;
use tore_sim::ai::launch::Side;
use tore_world::seats::PlaneId;

/// What a player who fits on neither side is told, once, as it joins; and
/// the refusal of its request for any slot.
pub const BOTH_FULL: &str = "Both sides are full.";

/// The sides in the order a tie gives them: Bluefor first.
const SIDES: [Side; 2] = [Side::Friendly, Side::Enemy];

fn index(side: Side) -> usize {
    match side {
        Side::Friendly => 0,
        Side::Enemy => 1,
    }
}

fn other(side: Side) -> Side {
    match side {
        Side::Friendly => Side::Enemy,
        Side::Enemy => Side::Friendly,
    }
}

impl Host {
    /// The side of `plane`: the roster's, else the lobby's slots'.
    fn plane_side(&self, plane: PlaneId) -> Option<Side> {
        self.world
            .roster
            .plane(plane)
            .map(|entry| entry.slot.wing.side)
            .or_else(|| {
                self.slots()
                    .iter()
                    .find(|slot| slot.id == plane.0)
                    .map(|slot| slot.wing.side)
            })
    }

    /// The side `connection` is on now: that of the plane it takes or
    /// flies, else of the slot it holds, else, in flight, the side its first
    /// plane fixed. `None` for a player with none of these.
    fn held_side(&self, connection: ConnectionId) -> Option<Side> {
        let peer = self.peers.get(&connection)?;
        let plane = match peer.stage {
            Stage::Taking { plane, .. } => Some(plane),
            Stage::Seated | Stage::Leaving => peer.plane.or(peer.lobby.slot),
            Stage::Lobby => peer.lobby.slot,
            Stage::Closing { .. } => return None,
        };
        if let Some(plane) = plane {
            return self.plane_side(plane);
        }
        if matches!(self.life, Life::Flying) {
            return self.court.side_of(&peer.callsign);
        }
        None
    }

    /// The humans on each side (Bluefor, Redfor), leaving out `except`:
    /// observers and players who cannot play the mission do not count.
    fn balance_counts(&self, except: Option<ConnectionId>) -> [usize; 2] {
        let mut counts = [0; 2];
        for (id, peer) in &self.peers {
            if Some(*id) == except
                || peer.lobby.unable.is_some()
                || (peer.watch.is_some() && !Self::in_flight(peer))
            {
                continue;
            }
            if let Some(side) = self.held_side(*id) {
                counts[index(side)] += 1;
            }
        }
        counts
    }

    /// The slots of `side` free to `connection` now, in plane order.
    fn balance_free(&self, connection: ConnectionId, side: Side) -> Vec<PlaneId> {
        self.slots()
            .iter()
            .filter(|slot| slot.wing.side == side)
            .map(|slot| PlaneId(slot.id))
            .filter(|&plane| self.side_slot_free(connection, plane))
            .collect()
    }

    /// The side and slot the balancing rule gives `connection`, counted
    /// apart from it: the side with fewer humans that has a free slot (on a
    /// tie, more free slots; then Bluefor), and its lowest free slot.
    /// `None` when both sides are full.
    fn balance_pick(&self, connection: ConnectionId) -> Option<(Side, PlaneId)> {
        let counts = self.balance_counts(Some(connection));
        SIDES
            .into_iter()
            .filter_map(|side| {
                let free = self.balance_free(connection, side);
                let first = *free.first()?;
                Some((
                    (counts[index(side)], Reverse(free.len()), index(side)),
                    side,
                    first,
                ))
            })
            .min_by_key(|(key, _, _)| *key)
            .map(|(_, side, plane)| (side, plane))
    }

    /// The side Autobalance gives `connection`: the one it is on, else the
    /// one the rule would seat it on. `None` while the host does not
    /// balance, or when it fits nowhere.
    pub(super) fn balanced_side(&self, connection: ConnectionId) -> Option<Side> {
        if !self.settings.balanced() {
            return None;
        }
        self.held_side(connection)
            .or_else(|| self.balance_pick(connection).map(|(side, _)| side))
    }

    /// A player without a slot asks for any (stage D's Ready with no plane
    /// named, or the lobby's Any): the slot the rule gives it.
    pub(super) fn balance_any(&self, connection: ConnectionId) -> Result<PlaneId, String> {
        self.balance_pick(connection)
            .map(|(_, plane)| plane)
            .ok_or_else(|| BOTH_FULL.to_owned())
    }

    /// Seats everyone waiting for a side, in the order they joined: run at
    /// every update while the host balances, in the lobby and in flight. A
    /// player waits when it is in the lobby, holds no slot, can play the
    /// mission and does not watch; in flight also when the AI aircraft of
    /// its slot is no longer free to it, unless a plane it flew fixed its
    /// side. Seated players are never moved here.
    pub(super) fn balance_update(&mut self) {
        if !self.settings.balanced() {
            return;
        }
        let flying = match self.life {
            Life::Lobby => false,
            Life::Flying => true,
            Life::Ended { .. } | Life::Stopped => return,
        };
        // Join in progress off: a late joiner flies nothing, so it is not
        // put on a side.
        if flying && self.new_pilot_refusal().is_some() {
            return;
        }
        let mut waiting: Vec<(u64, ConnectionId)> = self
            .peers
            .iter()
            .filter(|(id, peer)| {
                peer.stage == Stage::Lobby
                    && peer.lobby.unable.is_none()
                    && peer.watch.is_none()
                    && !(flying && self.court.side_of(&peer.callsign).is_some())
                    && match peer.lobby.slot {
                        None => true,
                        Some(plane) => flying && !self.side_slot_free(**id, plane),
                    }
            })
            .map(|(id, peer)| (peer.lobby.order, *id))
            .collect();
        waiting.sort_unstable();
        for (_, connection) in waiting {
            match self.balance_pick(connection) {
                Some((side, plane)) => {
                    let words = format!("Autobalance put you on {}.", side_name(side));
                    self.balance_seat(connection, plane, words);
                }
                None => {
                    // Said once, to a player who has had no lobby state yet:
                    // it joined just now.
                    let joined_now = self
                        .peers
                        .get(&connection)
                        .is_some_and(|peer| peer.lobby_sent.is_none() && peer.lobby.slot.is_none());
                    if joined_now {
                        self.send(connection, &Message::Notice(BOTH_FULL.into()));
                    }
                }
            }
        }
    }

    /// The King turned balance on (in the lobby): everyone without a slot
    /// is seated, then players move from the larger side, the most recently
    /// joined first and the King and the house last, until the sides are
    /// within one of each other or nobody can move.
    pub(super) fn balance_redeal(&mut self) {
        self.balance_update();
        loop {
            let counts = self.balance_counts(None);
            let larger = if counts[0] > counts[1] + 1 {
                Side::Friendly
            } else if counts[1] > counts[0] + 1 {
                Side::Enemy
            } else {
                break;
            };
            let smaller = other(larger);
            let mut movers: Vec<(bool, Reverse<u64>, ConnectionId)> = self
                .peers
                .iter()
                .filter(|(id, peer)| {
                    peer.stage == Stage::Lobby
                        && peer.lobby.unable.is_none()
                        && peer.watch.is_none()
                        && self.held_side(**id) == Some(larger)
                })
                .map(|(id, peer)| (peer.king || peer.house, Reverse(peer.lobby.order), *id))
                .collect();
            movers.sort_unstable();
            let Some((connection, plane)) = movers.into_iter().find_map(|(_, _, id)| {
                let free = self.balance_free(id, smaller);
                free.first().map(|plane| (id, *plane))
            }) else {
                break;
            };
            let words = format!("Autobalance moved you to {}.", side_name(smaller));
            self.balance_seat(connection, plane, words);
        }
    }

    /// Puts `connection` in `plane`'s slot and tells it `words`: its ready
    /// mark goes, and its loadout too unless the slot flies the same
    /// aircraft as the one it held.
    fn balance_seat(&mut self, connection: ConnectionId, plane: PlaneId, words: String) {
        let slots = self.slots();
        let aircraft = |plane: PlaneId| {
            slots
                .iter()
                .find(|slot| slot.id == plane.0)
                .map(|slot| slot.aircraft)
        };
        let Some(peer) = self.peers.get_mut(&connection) else {
            return;
        };
        let same = peer
            .lobby
            .slot
            .is_some_and(|held| aircraft(held) == aircraft(plane));
        if !same {
            peer.lobby.loadout = None;
        }
        peer.lobby.ready = false;
        peer.lobby.slot = Some(plane);
        let callsign = peer.callsign.clone();
        self.send(connection, &Message::Notice(words));
        self.lobby_log(callsign, LobbyEvent::Slot(Some(plane.0)));
        self.lobby_dirty = true;
    }
}
