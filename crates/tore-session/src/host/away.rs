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
//!   watching (it leaves the plane to the AI), leaves the game, or the
//!   mission ends; and when the AI loses the plane (the player is told, and
//!   under `respawn none` flies no other plane this mission).

use super::{ConnectionId, Host, Life, LobbyEvent, Stage, TICKS_PER_SECOND};
use crate::settings::Respawn;
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

/// A plane the AI flies for an away player, kept for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Reserved {
    pub(super) plane: PlaneId,
    /// The player's lobby id and callsign, for the lobby state and the
    /// words of a refusal.
    pub(super) lobby_id: u8,
    pub(super) callsign: String,
    /// Back was accepted: the take is on its way.
    pub(super) returning: bool,
}

/// The host's idle aircraft for the flying mission: the session's, which
/// stage K moves with the host. It starts afresh with each mission.
#[derive(Debug, Default)]
pub(super) struct Idle {
    /// Away asked for, handed to the AI at the next tick.
    asked: BTreeSet<ConnectionId>,
    /// The planes the AI flies for away players, by connection.
    pub(super) reserved: BTreeMap<ConnectionId, Reserved>,
    /// Each seated connection's flight and the tick the host first saw it
    /// flying: the stall's count starts there, not at an input never sent.
    flights: BTreeMap<ConnectionId, (u8, u64)>,
    /// Players whose plane the AI lost while they were away.
    lost: BTreeSet<ConnectionId>,
}

impl Idle {
    /// Whether the AI flies `plane` for an away player.
    pub(super) fn holds(&self, plane: PlaneId) -> bool {
        self.reserved.values().any(|r| r.plane == plane)
    }

    /// Whether the player with lobby id `id` is away.
    pub(super) fn marks(&self, id: u8) -> bool {
        self.reserved.values().any(|r| r.lobby_id == id)
    }
}

impl Host {
    /// The player's game has been away for the `idle-ai` setting's seconds
    /// (message 35): its plane goes to the AI at the next tick.
    pub(super) fn away_request(&mut self, connection: ConnectionId) -> Result<(), String> {
        if !matches!(self.life, Life::Flying) {
            return Err("The mission is not flying.".into());
        }
        if self.settings.idle_ai_seconds().is_none() {
            return Err(NO_IDLE_AI.into());
        }
        if self.idle.reserved.contains_key(&connection) {
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
        let Some(reserved) = self.idle.reserved.get(&connection).cloned() else {
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
            self.away_lost(connection);
            return Err(LOST_WHILE_AWAY.into());
        }
        if let Some(peer) = self.peers.get_mut(&connection) {
            peer.stage = Stage::Taking {
                seat,
                plane: reserved.plane,
            };
        }
        if let Some(reserved) = self.idle.reserved.get_mut(&connection) {
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

    /// Why the idle rules refuse `connection` taking `plane`, if they do:
    /// an away player takes only its own plane back, by Back; another
    /// player's away plane is kept for it; and a player whose plane the AI
    /// lost flies no other under `respawn none`.
    pub(super) fn away_take_refusal(
        &self,
        connection: ConnectionId,
        plane: PlaneId,
    ) -> Option<String> {
        if self.idle.reserved.contains_key(&connection) {
            return Some(WAITS.into());
        }
        if self.idle.lost.contains(&connection) && self.settings.respawn() == Respawn::None {
            return Some(super::revive::NO_REVIVAL.into());
        }
        self.idle
            .reserved
            .values()
            .find(|r| r.plane == plane)
            .map(|r| format!("Plane {} is kept for {}, who is away.", plane.0, r.callsign))
    }

    /// Whether the AI flies `plane` for an away player (another take of it
    /// is refused, an `ai-slot` revival skips it).
    pub(super) fn away_reserved(&self, plane: PlaneId) -> bool {
        self.idle.holds(plane)
    }

    /// Whether any player is away: the mission is not empty while one is.
    pub(super) fn anyone_away(&self) -> bool {
        !self.idle.reserved.is_empty()
    }

    /// The lobby state's away mark for the player with lobby id `id`.
    pub(super) fn away_mark(&self, id: u8) -> bool {
        self.idle.marks(id)
    }

    /// The mission ended: every reservation and count ends with it.
    pub(super) fn away_end(&mut self) {
        self.idle = Idle::default();
    }

    /// The AI lost the plane it flew for `connection`: the reservation ends,
    /// the player is told, and it watches on as any player without a plane.
    fn away_lost(&mut self, connection: ConnectionId) {
        let Some(reserved) = self.idle.reserved.remove(&connection) else {
            return;
        };
        self.idle.lost.insert(connection);
        self.send(connection, &Message::Notice(LOST_WHILE_AWAY.into()));
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

    /// Players who left the game take their asks and reservations with
    /// them; the plane stays the AI's.
    fn idle_departures(&mut self) {
        let peers = &self.peers;
        let here = |connection: &ConnectionId| {
            peers
                .get(connection)
                .is_some_and(|peer| !matches!(peer.stage, Stage::Closing { .. }))
        };
        let before = self.idle.reserved.len();
        self.idle.asked.retain(here);
        self.idle.reserved.retain(|connection, _| here(connection));
        self.idle.flights.retain(|connection, _| here(connection));
        self.idle.lost.retain(here);
        if self.idle.reserved.len() != before {
            self.lobby_dirty = true;
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
    /// watching it (it leaves the plane to the AI), or the AI lost it.
    fn idle_reservations(&mut self) {
        let ended: Vec<(ConnectionId, bool)> = self
            .idle
            .reserved
            .iter()
            .filter_map(|(connection, reserved)| {
                let peer = self.peers.get(connection)?;
                match peer.stage {
                    // Back in its plane (or in another: it flies).
                    Stage::Seated => Some((*connection, false)),
                    Stage::Taking { .. } => None,
                    _ if !self.ai_can_fly(reserved.plane) => Some((*connection, true)),
                    Stage::Lobby if peer.watch.is_none() => Some((*connection, false)),
                    _ => None,
                }
            })
            .collect();
        for (connection, lost) in ended {
            if lost {
                self.away_lost(connection);
                continue;
            }
            let Some(reserved) = self.idle.reserved.remove(&connection) else {
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

    /// Whether the AI flies `plane` with its pilot aboard: a plane it could
    /// hand over.
    fn ai_can_fly(&self, plane: PlaneId) -> bool {
        self.world
            .roster
            .plane(plane)
            .is_some_and(|entry| entry.pilot == Pilot::Ai)
            && self.world.can_take(NOBODY, plane).is_ok()
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
        if let Some(peer) = self.peers.get_mut(&connection) {
            peer.stage = Stage::Lobby;
            peer.seat = None;
            peer.plane = None;
        }
        self.idle.flights.remove(&connection);
        self.idle.reserved.insert(
            connection,
            Reserved {
                plane,
                lobby_id,
                callsign: callsign.clone(),
                returning: false,
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
