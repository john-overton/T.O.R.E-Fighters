//! Death and revival on the host (stage F phase 2; docs/ARCHITECTURE.md,
//! "Death, revival and lives"): the respawn rule, lives and the delay, and
//! the tick's Abandon and Revive commands. Slice F2-V.
//!
//! - **A loss.** Each tick the host looks for a seated player whose plane the
//!   world says is lost and sends it **Revival** (the rule, the lives left,
//!   the wait and why it cannot fly again, if it cannot).
//! - **The seat keeps the wreck** until the player flies again or leaves the
//!   game. A player who leaves its flight with a lost plane goes back to the
//!   lobby while the host *holds* its seat, the wreck flying on with neutral
//!   controls (this replaces the departed players' orphans); a player who
//!   leaves the game, is dropped or kicked has its lost plane abandoned
//!   ([`tore_world::world::MissionCommand::Abandon`]).
//! - **Flying again**: Revive (message 28) in flight, or Join (Take plane)
//!   from the lobby after a loss, which counts the same. By the King's
//!   `respawn` rule: `none` refuses; `ai-slot` abandons the lost plane and
//!   takes a free AI aircraft of the player's side, its own wing first, then
//!   the side's other wings, lowest id first, its stores cut by the weapons
//!   rule; `revive` revives the seat in a new plane at the revival point
//!   with the revival loadout, and every connection is sent **Spawned**
//!   before the player's Seated. Lives count revivals per player per mission;
//!   the delay counts from the loss.

use super::{ConnectionId, Host, Life, Stage, TICKS_PER_SECOND, aircraft_of};
use crate::settings::Respawn;
use crate::wire::messages::{Message, Revival, Spawned};
use std::collections::BTreeMap;
use tore_sim::ai::launch::Side;
use tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
use tore_world::seats::{Pilot, PlaneId, SeatId};
use tore_world::world::revive::{NewPlane, Spawn};
use tore_world::world::{MissionCommand, TickOutput};

/// Why a player cannot fly again, in words.
pub const NO_REVIVAL: &str = "No revival in this game.";
pub const NO_LIVES: &str = "No lives left.";
pub const NO_ROOM: &str = "Waiting for room for another aircraft.";
pub const NO_AI_SLOT: &str = "No AI aircraft of your side is free.";

/// One player's revivals this mission.
#[derive(Clone, Debug, Default)]
pub(super) struct Player {
    /// Revivals used.
    pub(super) used: u32,
    /// The plane it lost and the tick it was lost at, until it flies again.
    pub(super) lost: Option<(PlaneId, u64)>,
}

/// A revival asked for, made at the next tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pending {
    /// A new plane at the revival point.
    Revive { seat: SeatId },
    /// A free AI aircraft.
    AiSlot { seat: SeatId, plane: PlaneId },
}

/// A revival the tick's commands carry out.
#[derive(Clone, Debug)]
struct Made {
    connection: ConnectionId,
    seat: SeatId,
    /// The new plane, for Spawned (the `revive` rule).
    new: Option<NewPlane>,
    /// The AI aircraft taken (the `ai-slot` rule).
    taken: Option<PlaneId>,
}

/// The host's revival state for the flying mission: the session's, which
/// stage K moves with the host. It starts afresh with each mission.
#[derive(Debug, Default)]
pub(super) struct Revivals {
    /// By player (its join order, as the scores keep them).
    pub(super) players: BTreeMap<u64, Player>,
    /// Seats held for players in the lobby whose plane is lost: the seat
    /// keeps the wreck until the player flies again or leaves the game.
    pub(super) held: BTreeMap<ConnectionId, SeatId>,
    /// Revivals asked for, by connection.
    pub(super) pending: BTreeMap<ConnectionId, Pending>,
    /// Revivals this tick's commands make.
    making: Vec<Made>,
    /// The players told there is no room yet, so they are told once.
    waiting: Vec<ConnectionId>,
    /// The mean start position of each side's wings, friendly then enemy.
    starts: [Option<[f64; 3]>; 2],
    /// Every revival's Spawned message of the mission whose plane is still
    /// in it, for a player who joins later.
    pub(super) spawned: Vec<Spawned>,
}

fn side_index(side: Side) -> usize {
    match side {
        Side::Friendly => 0,
        Side::Enemy => 1,
    }
}

/// "0:45".
fn clock(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

impl Revivals {
    /// The seats the host holds for players in the lobby.
    pub(super) fn held_seats(&self) -> impl Iterator<Item = SeatId> + '_ {
        self.held.values().copied()
    }

    /// Whether a revival added `plane` to the mission.
    pub(super) fn added(&self, plane: PlaneId) -> bool {
        self.spawned.iter().any(|spawned| spawned.plane == plane.0)
    }
}

impl Host {
    /// The King's settings, changeable, for the client's tests (slice F2-1
    /// builds the King's change).
    #[cfg(test)]
    pub(crate) fn settings_for_test(&mut self) -> &mut crate::settings::Store {
        &mut self.settings
    }

    /// The mission, changeable, for the client's tests that lose a plane.
    #[cfg(test)]
    pub(crate) fn world_for_test(&mut self) -> &mut tore_world::world::World {
        &mut self.world
    }

    /// The mission starts flying: revivals start afresh, and each side's
    /// start is where its wings are now.
    pub(super) fn revive_start(&mut self) {
        self.revival = Revivals::default();
        for side in [Side::Friendly, Side::Enemy] {
            self.revival.starts[side_index(side)] = self.world.side_mean(side);
        }
    }

    /// A player joins: the planes revivals added to the mission so far, for
    /// its copy of it, after the Mission message.
    pub(super) fn revive_connected(&mut self, connection: ConnectionId) {
        for spawned in self.revival.spawned.clone() {
            self.send(connection, &Message::Spawned(Box::new(spawned)));
        }
    }

    /// The callsign of the player whose lost plane the host holds in `seat`.
    pub(super) fn held_callsign(&self, seat: SeatId) -> Option<String> {
        self.revival
            .held
            .iter()
            .find(|(_, held)| **held == seat)
            .and_then(|(connection, _)| self.peers.get(connection))
            .map(|peer| peer.callsign.clone())
    }

    /// A seat whose plane cannot go back to the AI as its player leaves:
    /// held for the player while it stays connected (`connection`, leaving
    /// its flight), abandoned to the mission otherwise.
    pub(super) fn revive_keep(&mut self, seat: SeatId, commands: &mut Vec<MissionCommand>) {
        let staying = self
            .peers
            .iter()
            .find(|(_, peer)| peer.seat == Some(seat) && peer.stage == Stage::Leaving)
            .map(|(id, _)| *id);
        match staying {
            Some(connection) => {
                self.revival.held.insert(connection, seat);
            }
            None if self.world.can_abandon(seat).is_ok() => {
                commands.push(MissionCommand::Abandon { seat });
                self.roster_dirty = true;
            }
            None => {}
        }
    }

    /// The player asks to fly again after a loss (message 28), answered by
    /// a Seated message or a refusal. The caller has checked the mission's
    /// number.
    pub(super) fn revive_request(&mut self, connection: ConnectionId) -> Result<(), String> {
        if !matches!(self.life, Life::Flying) {
            return Err("The mission is not flying.".into());
        }
        let peer = self.peers.get(&connection).ok_or("")?;
        let seat = match peer.stage {
            Stage::Seated => {
                let plane = peer.plane.ok_or("")?;
                if !self.world.plane_lost(plane) {
                    return Err("Your aircraft is not lost.".into());
                }
                peer.seat.ok_or("")?
            }
            Stage::Lobby => *self
                .revival
                .held
                .get(&connection)
                .ok_or("You have lost no aircraft; press Join to fly.")?,
            Stage::Taking { .. } | Stage::Leaving | Stage::Closing { .. } => {
                return Err("You are taking or leaving a plane.".into());
            }
        };
        self.revive_now(connection, seat)
    }

    /// Join (Take plane) from the lobby by a player whose plane is lost
    /// counts as flying again: `Some` with the outcome when it is one.
    pub(super) fn revive_join(&mut self, connection: ConnectionId) -> Option<Result<(), String>> {
        let seat = *self.revival.held.get(&connection)?;
        Some(self.revive_now(connection, seat))
    }

    /// Why the revival rules refuse `connection` taking `plane` now, if they
    /// do: a player whose plane is lost joins only through the revival
    /// ([`Self::revive_join`] answers its Join first), so this refuses
    /// only a take that slipped past it.
    pub(super) fn revive_take_refusal(
        &self,
        connection: ConnectionId,
        _plane: PlaneId,
    ) -> Option<String> {
        self.revival
            .held
            .contains_key(&connection)
            .then(|| "Fly again by the game's revival rules.".to_owned())
    }

    /// The player's revival record.
    fn order_of(&self, connection: ConnectionId) -> Option<u64> {
        self.peers.get(&connection).map(|peer| peer.lobby.order)
    }

    /// Lives left for player `order`: `None` for unlimited.
    fn lives_left(&self, order: u64) -> Option<u32> {
        let used = self.revival.players.get(&order).map_or(0, |p| p.used);
        self.settings
            .lives()
            .map(|lives| lives.saturating_sub(used))
    }

    /// Ticks until player `order` may fly again.
    fn wait_ticks(&self, order: u64) -> u64 {
        let delay = u64::from(self.settings.revive_delay_seconds()) * TICKS_PER_SECOND;
        let lost = self
            .revival
            .players
            .get(&order)
            .and_then(|p| p.lost)
            .map_or(0, |(_, tick)| tick);
        (lost + delay).saturating_sub(self.world.tick())
    }

    /// Why player `order` cannot fly again now, if it cannot.
    fn revive_refusal(&self, order: u64) -> Option<String> {
        if self.settings.respawn() == Respawn::None {
            return Some(NO_REVIVAL.into());
        }
        if self.lives_left(order) == Some(0) {
            return Some(NO_LIVES.into());
        }
        let wait = self.wait_ticks(order);
        (wait > 0).then(|| {
            format!(
                "You can fly again in {}.",
                clock(wait.div_ceil(TICKS_PER_SECOND))
            )
        })
    }

    /// Flies `connection` again from `seat`, which holds its lost plane, by
    /// the respawn rule: at the next tick, or a refusal now.
    fn revive_now(&mut self, connection: ConnectionId, seat: SeatId) -> Result<(), String> {
        let order = self.order_of(connection).ok_or("")?;
        if let Some(why) = self.revive_refusal(order) {
            return Err(why);
        }
        if self.revival.pending.contains_key(&connection) {
            return Ok(());
        }
        let pending = match self.settings.respawn() {
            Respawn::None => return Err(NO_REVIVAL.into()),
            Respawn::Revive => {
                // The new plane flies in the lost one's wing: lock sides
                // allows it whenever it allowed the lost plane.
                let lost = self
                    .world
                    .roster
                    .seat(seat)
                    .and_then(|s| s.plane)
                    .ok_or("You have lost no aircraft; press Join to fly.")?;
                if let Some(why) = self.sides_refusal(connection, lost) {
                    return Err(why);
                }
                Pending::Revive { seat }
            }
            Respawn::AiSlot => Pending::AiSlot {
                seat,
                plane: self.free_ai_plane(connection, seat).ok_or(NO_AI_SLOT)?,
            },
        };
        self.revival.pending.insert(connection, pending);
        Ok(())
    }

    /// The `ai-slot` rule's aircraft for `connection`, whose lost plane
    /// `seat` holds: a free AI aircraft of its side, its own wing first, then
    /// the side's other wings, lowest id first, that is open to players,
    /// that the slot locks and lock sides allow it (slice F2-1's rules; join
    /// in progress is not asked, as a revival is no new pilot), and that
    /// nobody else holds or takes.
    fn free_ai_plane(&self, connection: ConnectionId, seat: SeatId) -> Option<PlaneId> {
        let lost = self.world.roster.seat(seat)?.plane?;
        let wing = self.world.roster.plane(lost)?.slot.wing;
        let free = |plane: &&tore_world::seats::Plane| {
            plane.pilot == Pilot::Ai
                && plane.slot.wing.side == wing.side
                && self.open(plane.id)
                && self.lock_refusal(connection, plane.id.0).is_none()
                && self.sides_refusal(connection, plane.id).is_none()
                && !self.reserved(plane.id)
                && self.holder(plane.id, connection).is_none()
                && !self.revival.pending.values().any(
                    |p| matches!(p, Pending::AiSlot { plane: taken, .. } if *taken == plane.id),
                )
                && self
                    .world
                    .can_take(tore_world::world::revive::NOBODY, plane.id)
                    .is_ok()
        };
        let planes = self.world.roster.planes();
        planes
            .iter()
            .filter(|plane| plane.slot.wing == wing)
            .find(free)
            .or_else(|| planes.iter().find(free))
            .map(|plane| plane.id)
    }

    /// The tick's revivals and abandoned planes, added to `commands` before
    /// the step: a held seat whose player has gone is abandoned, and each
    /// revival asked for is made if it still may be.
    pub(super) fn revive_commands(&mut self, _tick: u64, commands: &mut Vec<MissionCommand>) {
        self.revival.making.clear();
        // Held seats whose player left the game: the wreck is the mission's.
        let gone: Vec<(ConnectionId, SeatId)> = self
            .revival
            .held
            .iter()
            .filter(|(connection, _)| {
                self.peers
                    .get(connection)
                    .is_none_or(|peer| matches!(peer.stage, Stage::Closing { .. }))
            })
            .map(|(connection, seat)| (*connection, *seat))
            .collect();
        for (connection, seat) in gone {
            self.revival.held.remove(&connection);
            self.revival.pending.remove(&connection);
            if self.world.can_abandon(seat).is_ok() {
                commands.push(MissionCommand::Abandon { seat });
                self.roster_dirty = true;
            }
        }
        let pending = std::mem::take(&mut self.revival.pending);
        for (connection, wanted) in pending {
            let Some(peer) = self.peers.get(&connection) else {
                continue;
            };
            let flying_lost = peer.stage == Stage::Seated;
            if !(flying_lost || self.revival.held.contains_key(&connection)) {
                continue;
            }
            match wanted {
                Pending::Revive { seat } => self.make_revival(connection, seat, commands),
                Pending::AiSlot { seat, plane } => {
                    if self
                        .world
                        .can_take(tore_world::world::revive::NOBODY, plane)
                        .is_err()
                        || self.reserved(plane)
                    {
                        // Taken or lost meanwhile: the player may ask again.
                        self.refuse_revival(connection, NO_AI_SLOT.into());
                        continue;
                    }
                    let weapons = self.settings.revive_weapons();
                    if let Err(error) = self.world.cut_ai_stores(plane, weapons) {
                        self.refuse_revival(connection, error.to_string());
                        continue;
                    }
                    if self.world.can_abandon(seat).is_ok() {
                        commands.push(MissionCommand::Abandon { seat });
                    }
                    commands.push(MissionCommand::Take { seat, plane });
                    self.start_making(connection, seat);
                    self.revival.making.push(Made {
                        connection,
                        seat,
                        new: None,
                        taken: Some(plane),
                    });
                }
            }
        }
    }

    /// A revival's new plane at the next tick, or the player told why not
    /// yet: no room keeps it waiting.
    fn make_revival(
        &mut self,
        connection: ConnectionId,
        seat: SeatId,
        commands: &mut Vec<MissionCommand>,
    ) {
        let Some(spawn) = self.revival_spawn(connection, seat) else {
            return;
        };
        if !self.world.room_for_one() {
            self.revival
                .pending
                .insert(connection, Pending::Revive { seat });
            if !self.revival.waiting.contains(&connection) {
                self.revival.waiting.push(connection);
                if let Some(order) = self.order_of(connection) {
                    let mut revival = self.revival_message(order);
                    revival.why = Some(NO_ROOM.into());
                    self.send(connection, &Message::Revival(Box::new(revival)));
                }
            }
            return;
        }
        match self.world.revival_plane(seat, &spawn) {
            Ok(new) => {
                commands.push(MissionCommand::Revive {
                    seat,
                    spawn: Box::new(spawn),
                });
                self.start_making(connection, seat);
                self.revival.making.push(Made {
                    connection,
                    seat,
                    new: Some(new),
                    taken: None,
                });
            }
            Err(error) => self.refuse_revival(connection, error.to_string()),
        }
    }

    /// Where and with what `connection`'s revival from `seat` appears now:
    /// the revival point from its side's start at the King's distance, with
    /// the loadout it chose in the lobby when that was for this aircraft.
    fn revival_spawn(&mut self, connection: ConnectionId, seat: SeatId) -> Option<Spawn> {
        let lost = self.world.roster.seat(seat)?.plane?;
        let side = self.world.roster.plane(lost)?.slot.wing.side;
        let aircraft = aircraft_of(&self.world, lost)?;
        let chosen = self.peers.get(&connection).and_then(|peer| {
            let slot = peer.lobby.slot?;
            let slot_aircraft = self
                .spec
                .open_planes()
                .into_iter()
                .find(|p| p.id == slot.0)?
                .aircraft;
            (slot_aircraft == aircraft)
                .then(|| peer.lobby.loadout.clone())
                .flatten()
        });
        let start = self.revival.starts[side_index(side)]
            .or_else(|| self.world.side_mean(side))
            .unwrap_or([0., 0., 0.]);
        let distance = f64::from(self.settings.revive_distance_nm()) * FEET_PER_NAUTICAL_MILE;
        let weapons = self.settings.revive_weapons();
        let spawn = self
            .world
            .revival_spawn(seat, start, distance, chosen.as_ref(), weapons)
            // A lobby loadout that no longer fits gives way to the standard
            // load.
            .or_else(|_| {
                self.world
                    .revival_spawn(seat, start, distance, None, weapons)
            });
        match spawn {
            Ok(spawn) => Some(spawn),
            Err(error) => {
                self.refuse_revival(connection, error.to_string());
                None
            }
        }
    }

    /// A revival is made at this tick: a seated player's buffered input
    /// (for its lost plane) gives way to neutral input, as a take's does.
    fn start_making(&mut self, connection: ConnectionId, _seat: SeatId) {
        self.revival.waiting.retain(|c| *c != connection);
        if let Some(peer) = self.peers.get_mut(&connection)
            && peer.stage == Stage::Seated
        {
            peer.inputs = super::InputBuffer::new();
        }
    }

    /// A revival that cannot be made: the player is told why.
    fn refuse_revival(&mut self, connection: ConnectionId, reason: String) {
        self.refuse(connection, crate::wire::messages::kind::REVIVE, reason);
    }

    /// The seats that need neutral input this tick from the host: held
    /// seats, and the seats an `ai-slot` revival takes a plane for.
    pub(super) fn revive_inputs(&self) -> Vec<SeatId> {
        let mut seats: Vec<SeatId> = self.revival.held_seats().collect();
        seats.extend(
            self.revival
                .making
                .iter()
                .filter(|made| made.taken.is_some())
                .map(|made| made.seat),
        );
        seats
    }

    /// After the step: the revivals made are seated (Spawned to every
    /// connection first, so each copy of the mission has the plane), lost
    /// planes are noticed and their players sent Revival, and retired
    /// planes leave the Spawned list.
    pub(super) fn revive_after(&mut self, tick: u64, out: &TickOutput) {
        for made in std::mem::take(&mut self.revival.making) {
            let plane = match (&made.new, made.taken) {
                (Some(new), _) => new.plane,
                (None, Some(taken)) => taken,
                (None, None) => continue,
            };
            if self.world.roster.seat_of(plane) != Some(made.seat) {
                continue;
            }
            if let Some(new) = &made.new {
                let spawned = Spawned {
                    plane: new.plane.0,
                    tick: tick as u32,
                    wing: new.slot.wing,
                    member: new.slot.member,
                    aircraft: new.aircraft,
                    spawn: new.spawn.clone(),
                };
                let to: Vec<ConnectionId> = self
                    .peers
                    .iter()
                    .filter(|(_, peer)| !matches!(peer.stage, Stage::Closing { .. }))
                    .map(|(id, _)| *id)
                    .collect();
                for id in to {
                    self.send_as_of(id, tick, Message::Spawned(Box::new(spawned.clone())));
                }
                self.revival.spawned.push(spawned);
            }
            self.revival.held.remove(&made.connection);
            if let Some(order) = self.order_of(made.connection) {
                let player = self.revival.players.entry(order).or_default();
                player.used += 1;
                player.lost = None;
            }
            self.roster_dirty = true;
            self.lobby_dirty = true;
            self.seated(made.connection, made.seat, plane, tick, out);
        }
        // Retired planes leave the list a joiner is sent.
        let roster = &self.world.roster;
        self.revival
            .spawned
            .retain(|spawned| roster.plane(PlaneId(spawned.plane)).is_some());
        // Lost planes: each seated player whose plane is newly lost.
        let lost: Vec<(ConnectionId, u64, PlaneId)> = self
            .peers
            .iter()
            .filter(|(_, peer)| peer.stage == Stage::Seated)
            .filter_map(|(id, peer)| Some((*id, peer.lobby.order, peer.plane?)))
            .filter(|(_, _, plane)| self.world.plane_lost(*plane))
            .collect();
        for (connection, order, plane) in lost {
            let player = self.revival.players.entry(order).or_default();
            if player.lost.is_some_and(|(was, _)| was == plane) {
                continue;
            }
            player.lost = Some((plane, tick));
            let revival = self.revival_message(order);
            self.send(connection, &Message::Revival(Box::new(revival)));
        }
    }

    /// The Revival message for player `order` now.
    fn revival_message(&self, order: u64) -> Revival {
        let lives = self
            .lives_left(order)
            // The registry's lives go to 10, all the wire carries.
            .map(|left| left.min(10) as u8);
        let why = match self.settings.respawn() {
            Respawn::None => Some(NO_REVIVAL.to_owned()),
            _ if lives == Some(0) => Some(NO_LIVES.to_owned()),
            _ => None,
        };
        Revival {
            rule: self.settings.respawn(),
            lives,
            wait_seconds: u32::try_from(self.wait_ticks(order).div_ceil(TICKS_PER_SECOND))
                .unwrap_or(u32::MAX),
            why,
        }
    }
}

// Stage K: the revivals part of the session's state (slice K1).
#[path = "revive_state.rs"]
pub(super) mod state;
