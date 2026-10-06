//! The AI flies an idle player's aircraft (stage F phase 2;
//! docs/ARCHITECTURE.md, "The AI flies an idle player's aircraft"): Away and
//! Back, the stall's count and the reservation. Slice F2-A.
//!
//! - **Away.** A player's game sends **Away** (message 35) once its controls
//!   have been neutral for the King's `idle-ai` seconds because of a menu, a
//!   lost focus or a lost controller. A seated player whose game sends no
//!   input at all for those seconds (its loop is stalled, the EF-K stall
//!   rule) is away by the host's own count. Either way, at the next tick the
//!   plane goes back to the AI (`MissionCommand::GiveBack`, the handoff's
//!   rules), the player is in the lobby, watching its own plane through the
//!   observer stream, and the plane is **reserved** for it: nobody else
//!   takes it, by Join or by an `ai-slot` revival, and the lobby state marks
//!   the player away.
//! - **Back.** **Back** (message 36), at the player's first flight input,
//!   takes the plane back at the next tick as a Take (a new flight), with
//!   its stores and damage as the AI left them. The King's rules on taking a
//!   plane (join in progress, slot locks, lock sides) do not apply: it is
//!   the player's own plane.
//! - **The reservation ends** when the player takes the plane back, stops
//!   watching (it leaves the plane to the AI), leaves the game on purpose, or
//!   the mission ends; and when the AI loses the plane (the player is told,
//!   and the revival rules have it from then on: its lives and the delay
//!   count, and under `respawn none` it flies no other plane this mission).
//!   A player whose connection drops while away keeps the reservation
//!   (slice K5): the table is `host::rejoin`'s, one for away and dropped
//!   players alike.

use super::rejoin::Reserved;
use super::{ConnectionId, Host, Life, LobbyEvent, Stage, TICKS_PER_SECOND};
use crate::wire::messages::{Message, Subject};
use std::collections::{BTreeMap, BTreeSet};
use tore_world::seats::{Pilot, PlaneId};
use tore_world::world::MissionCommand;
use tore_world::world::revive::NOBODY;

/// Why the host refuses Away or Back, in words.
pub const NO_IDLE_AI: &str = "The AI flies no idle aircraft in this game.";
pub const NOT_FLYING: &str = "You are not flying.";
pub const LOST: &str = "Your aircraft is lost: the AI cannot fly it.";
pub const NOT_AWAY: &str = "The AI is not flying your aircraft.";
pub const LOST_WHILE_AWAY: &str = "The AI lost your aircraft while you were away.";
pub const WAITS: &str =
    "Your aircraft waits for you: move the stick or press any flight key to take it back.";

/// What sent a player away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Cause {
    /// Its game said so (message 35).
    Asked,
    /// Its game sent no input for the setting's seconds.
    Stalled,
}

/// The host's idle aircraft for the flying mission: the session's, which
/// stage K moves with the host. It starts afresh with each mission. The
/// planes the AI flies for players are in `host::rejoin`'s table.
#[derive(Debug, Default)]
pub(super) struct Idle {
    /// Away asked for, handed to the AI at the next tick.
    asked: BTreeSet<ConnectionId>,
    /// Each seated connection's flight and the tick the host first saw it
    /// flying: the stall's count starts there, not at an input never sent.
    flights: BTreeMap<ConnectionId, (u8, u64)>,
}

impl Host {
    /// The reservation of `connection`'s player, if the AI flies a plane for
    /// it.
    fn reservation_of(&self, connection: ConnectionId) -> Option<&Reserved> {
        let order = self.peers.get(&connection)?.lobby.order;
        self.rejoin.reserved.get(&order)
    }

    /// The player is away: its game is connected and the AI flies its plane
    /// (slice F2-A).
    fn is_away(&self, connection: ConnectionId) -> bool {
        self.reservation_of(connection).is_some_and(|r| r.away)
    }

    /// The player's game has been away for the `idle-ai` setting's seconds
    /// (message 35): its plane goes to the AI at the next tick.
    pub(super) fn away_request(&mut self, connection: ConnectionId) -> Result<(), String> {
        if !matches!(self.life, Life::Flying) {
            return Err("The mission is not flying.".into());
        }
        if self.settings.idle_ai_seconds().is_none() {
            return Err(NO_IDLE_AI.into());
        }
        if self.is_away(connection) {
            return Ok(());
        }
        let peer = self.peers.get(&connection).ok_or("")?;
        let plane = match (peer.stage, peer.plane) {
            (Stage::Seated, Some(plane)) => plane,
            _ => return Err(NOT_FLYING.into()),
        };
        if self.world.plane_lost(plane) {
            return Err(LOST.into());
        }
        self.idle.asked.insert(connection);
        Ok(())
    }

    /// The player is back at the controls (message 36): its plane is taken
    /// back at the next tick and a Seated message follows, or a refusal.
    pub(super) fn back_request(&mut self, connection: ConnectionId) -> Result<(), String> {
        // Back before the tick that would have handed the plane over.
        if self.idle.asked.remove(&connection) {
            return Ok(());
        }
        self.take_reserved(connection)
    }

    /// The player takes the plane the AI flies for it: an away player's by
    /// Back, a returned player's by Join (slice K5). Its stores and damage
    /// are as the AI left them, and the King's rules on taking a plane (join
    /// in progress, slot locks, lock sides) do not apply: it is the player's
    /// own plane. A plane the AI lost meanwhile ends the reservation, with
    /// the revival rules from then on.
    pub(super) fn take_reserved(&mut self, connection: ConnectionId) -> Result<(), String> {
        let order = self.order_of(connection).ok_or("")?;
        let Some(reserved) = self.rejoin.reserved.get(&order).cloned() else {
            return Err(NOT_AWAY.into());
        };
        let peer = self.peers.get(&connection).ok_or("")?;
        match peer.stage {
            Stage::Lobby => {}
            Stage::Taking { .. } | Stage::Seated => return Ok(()),
            Stage::Leaving | Stage::Closing { .. } => return Err(NOT_AWAY.into()),
        }
        let Some(seat) = self.free_seat() else {
            return Err("No seat is free.".into());
        };
        if self.world.can_take(seat, reserved.plane).is_err() {
            self.away_lost(order);
            return Err(LOST_WHILE_AWAY.into());
        }
        if let Some(peer) = self.peers.get_mut(&connection) {
            peer.stage = Stage::Taking {
                seat,
                plane: reserved.plane,
            };
        }
        if let Some(reserved) = self.rejoin.reserved.get_mut(&order) {
            reserved.returning = true;
        }
        self.lobby_dirty = true;
        self.lobby_log(
            reserved.callsign,
            LobbyEvent::Back {
                plane: reserved.plane.0,
            },
        );
        Ok(())
    }

    /// A returned player's Join (slice K5): it takes the plane kept for it
    /// when it has one, whatever plane it asked for. `None` when nothing is
    /// kept for it (or it is an away player, who uses Back).
    pub(super) fn rejoin_take(&mut self, connection: ConnectionId) -> Option<Result<(), String>> {
        let reserved = self.reservation_of(connection)?;
        if reserved.away {
            return None;
        }
        Some(self.take_reserved(connection))
    }

    /// Why the idle rules refuse `connection` taking `plane`, if they do:
    /// an away player takes only its own plane back, by Back; another
    /// player's plane the AI flies for it is kept for it. (A player whose
    /// plane the AI lost flies by the revival rules: `host::revive`.)
    pub(super) fn away_take_refusal(
        &self,
        connection: ConnectionId,
        plane: PlaneId,
    ) -> Option<String> {
        if self.is_away(connection) {
            return Some(WAITS.into());
        }
        let own = self.order_of(connection);
        self.rejoin
            .reserved_plane(plane)
            .filter(|(order, _)| Some(*order) != own)
            .map(|(_, r)| format!("Plane {} is kept for {}, who is away.", plane.0, r.callsign))
    }

    /// Whether the AI flies `plane` for a player who is away (another take of
    /// it is refused, an `ai-slot` revival skips it).
    pub(super) fn away_reserved(&self, plane: PlaneId) -> bool {
        self.rejoin.reserved_plane(plane).is_some()
    }

    /// Whether any player is away: the mission is not empty while one is. A
    /// dropped player is not (its game is gone).
    pub(super) fn anyone_away(&self) -> bool {
        self.rejoin.reserved.values().any(|r| r.away)
    }

    /// The lobby state's away mark for the player with lobby id `id`.
    pub(super) fn away_mark(&self, id: u8) -> bool {
        self.rejoin
            .reserved
            .values()
            .any(|r| r.away && r.lobby_id == id)
    }

    /// The mission ended: every reservation and count ends with it.
    pub(super) fn away_end(&mut self) {
        self.idle = Idle::default();
        self.rejoin.reserved.clear();
    }

    /// The AI lost the plane it flew for the player `order`: the reservation
    /// ends, and the player's plane counts as lost from now on, so the
    /// revival rules (lives, the delay, `respawn`) have it when it comes
    /// back or if it is here: it is told now.
    pub(super) fn away_lost(&mut self, order: u64) {
        let Some(reserved) = self.rejoin.reserved.remove(&order) else {
            return;
        };
        let tick = self.world.tick();
        self.revival_note_lost(order, reserved.plane, tick);
        if let Some(connection) = self.connection_of(order) {
            self.send(connection, &Message::Notice(LOST_WHILE_AWAY.into()));
            let revival = self.revival_message(order);
            self.send(connection, &Message::Revival(Box::new(revival)));
        }
        self.lobby_log(
            reserved.callsign,
            LobbyEvent::AwayEnded {
                plane: reserved.plane.0,
                lost: true,
            },
        );
        self.lobby_dirty = true;
    }

    /// The tick's handoffs for away and returning players, added to
    /// `commands` before the step: reservations that ended are dropped, and
    /// every player away (asked, or stalled by the host's count) has its
    /// plane given back to the AI.
    pub(super) fn away_commands(&mut self, tick: u64, commands: &mut Vec<MissionCommand>) {
        self.idle_departures();
        self.idle_flights(tick);
        self.idle_reservations();
        self.idle_handoffs(tick, commands);
    }

    /// Players who left the game take their asks with them; the plane stays
    /// the AI's, and an away player whose connection has gone keeps the
    /// plane as a dropped player's (`host::rejoin`).
    fn idle_departures(&mut self) {
        let peers = &self.peers;
        let here = |connection: &ConnectionId| {
            peers
                .get(connection)
                .is_some_and(|peer| !matches!(peer.stage, Stage::Closing { .. }))
        };
        self.idle.asked.retain(here);
        self.idle.flights.retain(|connection, _| here(connection));
        let present: BTreeSet<u64> = self
            .peers
            .values()
            .filter(|peer| !matches!(peer.stage, Stage::Closing { .. }))
            .map(|peer| peer.lobby.order)
            .collect();
        for (order, reserved) in &mut self.rejoin.reserved {
            if reserved.away && !present.contains(order) {
                reserved.away = false;
                self.lobby_dirty = true;
            }
        }
    }

    /// Notes the tick each seated connection's flight was first seen, from
    /// which the stall's count runs.
    fn idle_flights(&mut self, tick: u64) {
        for (connection, peer) in &self.peers {
            if peer.stage == Stage::Seated {
                let entry = self
                    .idle
                    .flights
                    .entry(*connection)
                    .or_insert((peer.flight, tick));
                if entry.0 != peer.flight {
                    *entry = (peer.flight, tick);
                }
            } else {
                self.idle.flights.remove(connection);
            }
        }
    }

    /// Reservations that ended: the player took the plane back, stopped
    /// watching it (it leaves the plane to the AI), or the AI lost it, even
    /// while the player's game was gone.
    fn idle_reservations(&mut self) {
        let ended: Vec<(u64, bool)> = self
            .rejoin
            .reserved
            .iter()
            .filter_map(|(order, reserved)| {
                let peer = self.peers.values().find(|peer| peer.lobby.order == *order);
                match peer.map(|peer| peer.stage) {
                    // Back in its plane (or in another: it flies).
                    Some(Stage::Seated) => Some((*order, false)),
                    Some(Stage::Taking { .. }) => None,
                    _ if !self.plane_kept(reserved.plane) => Some((*order, true)),
                    // An away player stopped watching: it leaves the plane to
                    // the AI. A returned player in the lobby just waits.
                    Some(Stage::Lobby)
                        if reserved.away && peer.is_some_and(|p| p.watch.is_none()) =>
                    {
                        Some((*order, false))
                    }
                    _ => None,
                }
            })
            .collect();
        for (order, lost) in ended {
            if lost {
                self.away_lost(order);
                continue;
            }
            let Some(reserved) = self.rejoin.reserved.remove(&order) else {
                continue;
            };
            self.lobby_dirty = true;
            if !reserved.returning {
                self.lobby_log(
                    reserved.callsign,
                    LobbyEvent::AwayEnded {
                        plane: reserved.plane.0,
                        lost: false,
                    },
                );
            }
        }
    }

    /// Whether the AI keeps `plane` for a player: it flies it with its pilot
    /// aboard, or the hand-over to it is still on its way.
    fn plane_kept(&self, plane: PlaneId) -> bool {
        match self.world.roster.plane(plane).map(|entry| entry.pilot) {
            Some(Pilot::Ai) => self.world.can_take(NOBODY, plane).is_ok(),
            Some(Pilot::Human(_)) => true,
            Some(Pilot::Lost) | None => false,
        }
    }

    /// The players away now: those whose game asked, and, with the setting
    /// on, every seated player whose game has sent no input for its seconds
    /// (the host's count of a stall).
    fn idle_handoffs(&mut self, tick: u64, commands: &mut Vec<MissionCommand>) {
        let mut away: Vec<(ConnectionId, Cause)> = std::mem::take(&mut self.idle.asked)
            .into_iter()
            .map(|connection| (connection, Cause::Asked))
            .collect();
        if let Some(seconds) = self.settings.idle_ai_seconds() {
            let quiet = u64::from(seconds) * TICKS_PER_SECOND;
            for (connection, peer) in &self.peers {
                if peer.stage != Stage::Seated || away.iter().any(|(c, _)| c == connection) {
                    continue;
                }
                let since = self.idle.flights.get(connection).map_or(tick, |f| f.1);
                let heard = u64::from(peer.inputs.newest()).max(since);
                if tick.saturating_sub(heard) >= quiet {
                    away.push((*connection, Cause::Stalled));
                }
            }
        }
        for (connection, cause) in away {
            self.hand_to_ai(connection, cause, commands);
        }
    }

    /// `connection`'s plane goes back to the AI this tick, kept for it, and
    /// the player watches it from the lobby.
    fn hand_to_ai(
        &mut self,
        connection: ConnectionId,
        cause: Cause,
        commands: &mut Vec<MissionCommand>,
    ) {
        let Some(peer) = self.peers.get(&connection) else {
            return;
        };
        let (Stage::Seated, Some(seat), Some(plane)) = (peer.stage, peer.seat, peer.plane) else {
            return;
        };
        // A lost plane cannot go to the AI: the revival rules have it.
        if self.world.can_give_back(seat).is_err() {
            return;
        }
        commands.push(MissionCommand::GiveBack { seat });
        let callsign = peer.callsign.clone();
        let lobby_id = peer.lobby.id;
        let order = peer.lobby.order;
        if let Some(peer) = self.peers.get_mut(&connection) {
            peer.stage = Stage::Lobby;
            peer.seat = None;
            peer.plane = None;
        }
        self.idle.flights.remove(&connection);
        self.rejoin.reserved.insert(
            order,
            Reserved {
                plane,
                lobby_id,
                callsign: callsign.clone(),
                returning: false,
                away: true,
            },
        );
        // Its messages may fill its packets again, as back in the lobby.
        self.server
            .set_message_budget(connection, tore_net::MAX_DATAGRAM);
        self.roster_dirty = true;
        self.lobby_dirty = true;
        self.lobby_log(
            callsign,
            LobbyEvent::Away {
                plane: plane.0,
                stalled: cause == Cause::Stalled,
            },
        );
        // The player watches its own plane (the observer stream, with the
        // PvP observer delay as every observer has it), from this tick.
        let _ = self.start_watch(connection, Subject::Aircraft(plane.0));
    }
}
