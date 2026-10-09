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
//! - **A plane nobody held** (slice K5): the AI lost the aircraft it flew for
//!   a player who was away or dropped. The loss is noted for the player
//!   ([`Host::revival_note_lost`]), and when the player comes back (or is
//!   still here) the same rules apply with no seat holding the wreck: a free
//!   seat is taken for the revival, which makes its new plane in the lost
//!   one's wing ([`MissionCommand::ReviveLost`]).
//! - **AI respawn** (the lobby pass's slice R1, John 2026-10-09): every plane
//!   the mission started with roots a lineage. A lineage whose newest plane
//!   is lost while the AI holds it (the AI flew it for nobody, or its player
//!   left the game) respawns under the King's `ai-respawn` setting, by the
//!   same lives (counted per lineage) and delay as a human, at its original
//!   spawn point (recorded at the mission's first tick), with its root's
//!   lobby loadout or the standard load cut by the weapons rule
//!   ([`MissionCommand::Respawn`]); every connection is sent **Spawned**. A
//!   plane the AI flew for an away or dropped player is that player's
//!   revival, never the AI's. Human revivals come first each tick; an AI
//!   respawn waits for room.
//! - **The lead hold** (the lobby pass's slice R2, John 2026-10-09): in a
//!   game whose `respawn` is not `none` the host turns the mission core's
//!   lead hold on ([`MissionCommand::LeadHold`]), so a human lead keeps its
//!   flight's lead while dead, reviving or flying back
//!   (`tore_world::world::lead_hold`). Each tick it tells the world of every
//!   owner who has left the game ([`MissionCommand::LeadLeft`]): a seat no
//!   player in the game holds any more, or a plane the AI flew for a player
//!   that no player is kept for now. The log says who leads and who owns
//!   each flight's lead as it changes.

use super::{ConnectionId, Host, Life, LobbyEvent, Stage, TICKS_PER_SECOND, aircraft_of};
use crate::settings::Respawn;
use crate::wire::messages::{Message, Revival, Spawned};
use std::collections::BTreeMap;
use tore_sim::ai::launch::Side;
use tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
use tore_world::seats::{Pilot, PlaneId, SeatId};
use tore_world::world::lead_hold::{LeadOwner, Owned};
use tore_world::world::revive::{NewPlane, Spawn};
use tore_world::world::{MissionCommand, TickOutput};

/// Why a player cannot fly again, in words.
pub const NO_REVIVAL: &str = "No revival in this game.";
pub const NO_LIVES: &str = "No lives left.";
pub const NO_ROOM: &str = "Waiting for room for another aircraft.";
pub const NO_AI_SLOT: &str = "No AI aircraft of your side is free.";

/// One AI lineage's respawns this mission (slice R1), by its root plane.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Lineage {
    /// AI respawns used: what the King's lives count for the AI.
    pub(super) used: u32,
    /// The tick the host saw the lineage's newest plane lost to the AI, until
    /// it respawns: the delay counts from it.
    pub(super) lost: Option<u64>,
    /// The log was told it waits (for room, or by a refusal), so it is told
    /// once.
    pub(super) told: bool,
}

/// Who holds a lineage's newest plane (slice R1): what decides whether its
/// loss is the AI's to respawn. A hook for the lead hold (R2): which seat
/// flies a flight's member, and which player a lost one waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Holder {
    /// Nobody: the AI flies it, or flew it and lost it, or it is a wreck
    /// whose player left the game. The AI respawns it.
    Ai,
    /// A seat flies it, or holds it lost for its player.
    Seat(SeatId),
    /// The AI flies it, or lost it, for the player with this join order, who
    /// is away, dropped, or back and about to revive.
    Player(u64),
}

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
    /// The AI lineages that lost a plane, by root (slice R1).
    pub(super) lineages: BTreeMap<PlaneId, Lineage>,
    /// Where every plane the mission started with began, and its heading:
    /// each lineage's original spawn point (slice R1).
    pub(super) origins: BTreeMap<PlaneId, ([f64; 3], f64)>,
    /// The AI respawns this tick's commands make, by root, with their spawn:
    /// scratch, as `making`.
    respawning: Vec<(PlaneId, Spawn)>,
    /// The lead hold's owners before this tick's step, for the log of what
    /// changed (slice R2): scratch, as `making`.
    pub(super) lead_before: Vec<Owned>,
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

impl Pending {
    fn seat(self) -> SeatId {
        match self {
            Pending::Revive { seat } | Pending::AiSlot { seat, .. } => seat,
        }
    }
}

impl Revivals {
    /// The seats revivals asked for will take at the next tick: a seat that
    /// flew no plane is taken for the revival (slice K5).
    pub(super) fn pending_seats(&self) -> impl Iterator<Item = SeatId> + '_ {
        self.pending.values().map(|pending| pending.seat())
    }

    /// The seats the host holds for players in the lobby.
    pub(super) fn held_seats(&self) -> impl Iterator<Item = SeatId> + '_ {
        self.held.values().copied()
    }

    /// The connection a revival this tick seats in `seat` (follow-up F1).
    pub(super) fn making_connection(&self, seat: SeatId) -> Option<ConnectionId> {
        self.making
            .iter()
            .find(|made| made.seat == seat)
            .map(|made| made.connection)
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
        // Each lineage's original spawn point (slice R1).
        for root in self.world.lineage_roots() {
            if let Some(pose) = self.world.plane_pose(root) {
                self.revival.origins.insert(root, pose);
            }
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
            Stage::Lobby => match self.revival.held.get(&connection) {
                Some(seat) => *seat,
                None if self.seatless_lost(connection) => {
                    self.free_seat().ok_or("No seat is free.")?
                }
                None => return Err("You have lost no aircraft; press Join to fly.".into()),
            },
            Stage::Taking { .. } | Stage::Leaving | Stage::Closing { .. } => {
                return Err("You are taking or leaving a plane.".into());
            }
        };
        self.revive_now(connection, seat)
    }

    /// Join (Take plane) from the lobby by a player whose plane is lost
    /// counts as flying again: `Some` with the outcome when it is one.
    pub(super) fn revive_join(&mut self, connection: ConnectionId) -> Option<Result<(), String>> {
        if let Some(seat) = self.revival.held.get(&connection).copied() {
            return Some(self.revive_now(connection, seat));
        }
        if self.seatless_lost(connection) {
            return Some(match self.free_seat() {
                Some(seat) => self.revive_now(connection, seat),
                None => Err("No seat is free.".into()),
            });
        }
        None
    }

    /// Whether the player `order` has a lost plane noted and no seat holds
    /// it (the AI lost it while the player was away, or the player left the
    /// game with it lost): its revival takes a free seat.
    pub(super) fn revival_lost(&self, order: u64) -> bool {
        self.revival
            .players
            .get(&order)
            .is_some_and(|player| player.lost.is_some())
    }

    /// Whether `connection`, in the lobby, has a lost plane no seat holds.
    fn seatless_lost(&self, connection: ConnectionId) -> bool {
        self.peers
            .get(&connection)
            .is_some_and(|peer| peer.stage == Stage::Lobby)
            && !self.revival.held.contains_key(&connection)
            && self
                .order_of(connection)
                .is_some_and(|order| self.revival_lost(order))
    }

    /// The AI lost `plane`, which it flew for the player `order` who was
    /// away or gone: the revival rules count from this tick.
    pub(super) fn revival_note_lost(&mut self, order: u64, plane: PlaneId, tick: u64) {
        self.revival.players.entry(order).or_default().lost = Some((plane, tick));
    }

    /// The lost plane `connection` would revive from: the one its seat
    /// holds, or the one noted for it when no seat holds any.
    fn lost_plane(&self, connection: ConnectionId, seat: SeatId) -> Option<PlaneId> {
        self.world
            .roster
            .seat(seat)
            .and_then(|s| s.plane)
            .or_else(|| {
                let order = self.order_of(connection)?;
                self.revival
                    .players
                    .get(&order)?
                    .lost
                    .map(|(plane, _)| plane)
            })
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
        (self.revival.held.contains_key(&connection) || self.seatless_lost(connection))
            .then(|| "Fly again by the game's revival rules.".to_owned())
    }

    /// The player's revival record.
    pub(super) fn order_of(&self, connection: ConnectionId) -> Option<u64> {
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
                    .lost_plane(connection, seat)
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
        let lost = self.lost_plane(connection, seat)?;
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
    pub(super) fn revive_commands(&mut self, tick: u64, commands: &mut Vec<MissionCommand>) {
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
            if !(flying_lost
                || self.revival.held.contains_key(&connection)
                || self.seatless_lost(connection))
            {
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
        // Then the AI's respawns, with the room the humans left (slice R1).
        self.ai_respawn_commands(tick, commands);
        // The lead hold, after the abandons of players gone (slice R2).
        self.lead_hold_commands(commands);
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
        // The seat holds its lost plane, or none does (slice K5).
        let holds = self
            .world
            .roster
            .seat(seat)
            .is_some_and(|s| s.plane.is_some());
        let lost = self.lost_plane(connection, seat);
        let made = match (holds, lost) {
            (false, Some(lost)) => self
                .world
                .revival_plane_from(lost, &spawn)
                .map(|new| (new, Some(lost))),
            _ => self
                .world
                .revival_plane(seat, &spawn)
                .map(|new| (new, None)),
        };
        match made {
            Ok((new, from)) => {
                commands.push(match from {
                    Some(plane) => MissionCommand::ReviveLost {
                        seat,
                        plane,
                        spawn: Box::new(spawn),
                    },
                    None => MissionCommand::Revive {
                        seat,
                        spawn: Box::new(spawn),
                    },
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
        let lost = self.lost_plane(connection, seat)?;
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
            .revival_spawn_from(lost, start, distance, chosen.as_ref(), weapons)
            // A lobby loadout that no longer fits gives way to the standard
            // load.
            .or_else(|_| {
                self.world
                    .revival_spawn_from(lost, start, distance, None, weapons)
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
    /// seats, and the seats a revival makes a plane for.
    pub(super) fn revive_inputs(&self) -> Vec<SeatId> {
        let mut seats: Vec<SeatId> = self.revival.held_seats().collect();
        // A revival from a plane nobody held seats a seat that flew none
        // (slice K5): the others' inputs come from their players.
        seats.extend(self.revival.making.iter().map(|made| made.seat));
        seats
    }

    /// After the step: the revivals made are seated (Spawned to every
    /// connection first, so each copy of the mission has the plane), lost
    /// planes are noticed and their players sent Revival, and retired
    /// planes leave the Spawned list.
    pub(super) fn revive_after(&mut self, tick: u64, out: &TickOutput) {
        for mut made in std::mem::take(&mut self.revival.making) {
            let plane = match (&mut made.new, made.taken) {
                // The plane the revival made, as the step numbered it: an
                // earlier revival or respawn of the same tick may have taken
                // the id and member it was checked with (slice R1).
                (Some(new), _) => {
                    let Some(made_plane) = self
                        .world
                        .roster
                        .seat(made.seat)
                        .and_then(|seat| seat.plane)
                        .and_then(|plane| self.world.roster.plane(plane))
                    else {
                        continue;
                    };
                    new.plane = made_plane.id;
                    new.slot = made_plane.slot;
                    new.plane
                }
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
                // Where it came back, for the log (slice R1).
                if let Some(callsign) = self.peers.get(&made.connection).map(|p| p.callsign.clone())
                {
                    let words = format!("flies again in plane {}", new.plane.0);
                    let words = self.spawn_words(words, new.spawn.position);
                    self.lobby_log(callsign, LobbyEvent::Revival(words));
                }
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
        // The AI's respawns (slice R1).
        self.ai_respawn_after(tick);
        // The lead hold's changes, for the log (slice R2).
        self.lead_hold_after();
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
    pub(super) fn revival_message(&self, order: u64) -> Revival {
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

/// "Red 2-3": a plane's side, wing and member as the logs name it (slice
/// R1).
fn plane_label(slot: tore_world::seats::Slot) -> String {
    let side = match slot.wing.side {
        Side::Friendly => "Blue",
        Side::Enemy => "Red",
    };
    format!(
        "{side} {}-{}",
        u32::from(slot.wing.index) + 1,
        u32::from(slot.member) + 1
    )
}

/// Feet as nautical miles, one decimal.
fn nm(feet: f64) -> String {
    format!("{:.1}", feet / FEET_PER_NAUTICAL_MILE)
}

impl Host {
    /// Who holds the newest plane `head` of a lineage (slice R1).
    pub(crate) fn lineage_holder(&self, head: PlaneId) -> Holder {
        if let Some(Pilot::Human(seat)) = self.world.roster.plane(head).map(|p| p.pilot) {
            return Holder::Seat(seat);
        }
        if let Some((order, _)) = self.rejoin.reserved_plane(head) {
            return Holder::Player(order);
        }
        let ai_flown = self
            .world
            .roster
            .plane(head)
            .is_some_and(|p| p.pilot == Pilot::Ai);
        let noted = self
            .revival
            .players
            .iter()
            .find(|(_, player)| player.lost.is_some_and(|(plane, _)| plane == head))
            .map(|(order, _)| *order);
        match noted {
            // The AI lost it while it flew it for the player: the player's.
            Some(order) if ai_flown => Holder::Player(order),
            // A wreck whose player left the game: the player's while it is
            // back to revive from it, the AI's otherwise.
            Some(order) if self.connection_of(order).is_some() => Holder::Player(order),
            _ => Holder::Ai,
        }
    }

    /// Whether the AI respawns its lost lineages in this mission: the
    /// King's `ai-respawn` while `respawn` is not `none`.
    pub(super) fn ai_respawns(&self) -> bool {
        self.settings.ai_respawn()
    }

    /// The tick's AI respawns, after the human revivals in `commands` (slice
    /// R1): each lineage whose newest plane the AI has lost and whose delay
    /// is over, with lives left and room, respawns at its original spawn.
    fn ai_respawn_commands(&mut self, tick: u64, commands: &mut Vec<MissionCommand>) {
        self.revival.respawning.clear();
        if !self.ai_respawns() {
            return;
        }
        // The room the tick's human revivals leave.
        let humans = self
            .revival
            .making
            .iter()
            .filter(|made| made.new.is_some())
            .count();
        let mut room = self.world.room().saturating_sub(humans);
        let delay = u64::from(self.settings.revive_delay_seconds()) * TICKS_PER_SECOND;
        let lives = self.settings.lives();
        let weapons = self.settings.revive_weapons();
        let mut taken: Vec<[f64; 3]> = Vec::new();
        for (root, head) in self.world.lineage_heads() {
            let lost = self.world.head_lost(head) && self.lineage_holder(head) == Holder::Ai;
            if !lost {
                if let Some(lineage) = self.revival.lineages.get_mut(&root) {
                    lineage.lost = None;
                    lineage.told = false;
                }
                continue;
            }
            let label = self
                .world
                .roster
                .plane(head)
                .or_else(|| self.world.revival.retired().iter().find(|p| p.id == head))
                .map_or_else(|| format!("plane {}", root.0), |p| plane_label(p.slot));
            let lineage = self.revival.lineages.entry(root).or_default();
            let used = lineage.used;
            let since = match lineage.lost {
                Some(since) => since,
                None => {
                    lineage.lost = Some(tick);
                    lineage.told = false;
                    let words = if lives.is_some_and(|lives| used >= lives) {
                        format!(
                            "lost plane {}: no lives left, the AI does not respawn it",
                            head.0
                        )
                    } else if delay == 0 {
                        format!("lost plane {}: the AI respawns it at once", head.0)
                    } else {
                        format!(
                            "lost plane {}: the AI respawns it in {}",
                            head.0,
                            clock(delay.div_ceil(TICKS_PER_SECOND))
                        )
                    };
                    self.lobby_log(label.clone(), LobbyEvent::Revival(words));
                    tick
                }
            };
            if lives.is_some_and(|lives| used >= lives) || tick < since + delay {
                continue;
            }
            if room == 0 {
                self.respawn_waits(root, label, "waits for room to respawn".into());
                continue;
            }
            let Some(&(origin, heading)) = self.revival.origins.get(&root) else {
                self.respawn_waits(root, label, "has no original spawn to respawn at".into());
                continue;
            };
            let chosen = self.spec.plane_loadouts.get(&root.0).cloned();
            let spawn = self
                .world
                .respawn_spawn(root, origin, heading, &taken, chosen.as_ref(), weapons)
                // A lobby loadout that no longer fits gives way to the
                // standard load.
                .or_else(|_| {
                    self.world
                        .respawn_spawn(root, origin, heading, &taken, None, weapons)
                });
            let checked = spawn.and_then(|spawn| {
                self.world.can_respawn(root, &spawn)?;
                Ok(spawn)
            });
            match checked {
                Ok(spawn) => {
                    taken.push(spawn.position);
                    room -= 1;
                    commands.push(MissionCommand::Respawn {
                        root,
                        spawn: Box::new(spawn.clone()),
                    });
                    self.revival.respawning.push((root, spawn));
                }
                Err(error) => self.respawn_waits(root, label, format!("cannot respawn: {error}")),
            }
        }
    }

    /// A respawn that must wait, told to the log once.
    fn respawn_waits(&mut self, root: PlaneId, label: String, words: String) {
        if let Some(lineage) = self.revival.lineages.get_mut(&root)
            && !lineage.told
        {
            lineage.told = true;
            self.lobby_log(label, LobbyEvent::Revival(words));
        }
    }

    /// After the step: each AI respawn made is sent to every connection as
    /// Spawned and kept for joiners, its lineage counts a life, and a wreck
    /// the AI took over from a player who left the game is the player's no
    /// longer (slice R1).
    fn ai_respawn_after(&mut self, tick: u64) {
        for (root, spawn) in std::mem::take(&mut self.revival.respawning) {
            let plane = self.world.lineage_head(root);
            if plane == root
                || !self.world.revival.added().contains(&plane)
                || self.revival.spawned.iter().any(|s| s.plane == plane.0)
            {
                continue;
            }
            let Some(entry) = self.world.roster.plane(plane).copied() else {
                continue;
            };
            let Some(aircraft) = aircraft_of(&self.world, plane) else {
                continue;
            };
            let spawned = Spawned {
                plane: plane.0,
                tick: tick as u32,
                wing: entry.slot.wing,
                member: entry.slot.member,
                aircraft,
                spawn: spawn.clone(),
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
            let lineage = self.revival.lineages.entry(root).or_default();
            lineage.used += 1;
            lineage.lost = None;
            lineage.told = false;
            // The lineage is the AI's now: a player who left the game with
            // its wreck no longer revives from it.
            let replaced: Vec<PlaneId> = self.world.lineage(root);
            for player in self.revival.players.values_mut() {
                if player
                    .lost
                    .is_some_and(|(lost, _)| replaced.contains(&lost))
                {
                    player.lost = None;
                }
            }
            self.roster_dirty = true;
            let words = format!("respawned in plane {} at its original spawn", plane.0);
            let words = self.spawn_words(words, spawn.position);
            self.lobby_log(plane_label(entry.slot), LobbyEvent::Revival(words));
        }
    }

    /// `words` and where `at` lies on the map, for the log: "..., x 81.2
    /// nm, z 40.0 nm (map x 1.3 to 278.7 nm, z 1.3 to 266.4 nm)".
    fn spawn_words(&self, words: String, at: [f64; 3]) -> String {
        let bounds = self.world.map_bounds();
        format!(
            "{words}, x {} nm, z {} nm (map x {} to {} nm, z {} to {} nm)",
            nm(at[0]),
            nm(at[2]),
            nm(bounds.min[0]),
            nm(bounds.max[0]),
            nm(bounds.min[1]),
            nm(bounds.max[1]),
        )
    }
}

/// "Blue 1": a wing as the logs name it (slice R2).
fn wing_label(wing: tore_sim::ai::launch::WingId) -> String {
    let side = match wing.side {
        Side::Friendly => "Blue",
        Side::Enemy => "Red",
    };
    format!("{side} {}", u32::from(wing.index) + 1)
}

impl Host {
    /// The lead hold's commands for the tick (the lobby pass's slice R2): on
    /// while the King's `respawn` is not `none`, off otherwise (single
    /// player has no host and never has it); and each owner who has left the
    /// game is told to the world, which passes its lead on.
    fn lead_hold_commands(&mut self, commands: &mut Vec<MissionCommand>) {
        self.revival.lead_before = self.world.lead_owners().to_vec();
        let on = self.settings.respawn() != Respawn::None;
        if self.world.lead_hold() != on {
            commands.push(MissionCommand::LeadHold { on });
        }
        if !self.world.lead_hold() {
            return;
        }
        for owned in self.world.lead_owners().to_vec() {
            // A seat whose plane goes back to the AI this tick: the world
            // keeps the lead for the plane's player, and the next tick asks
            // again whether anyone is kept for it.
            let given_back = commands.iter().any(|command| {
                matches!((command, owned.owner),
                    (MissionCommand::GiveBack { seat }, LeadOwner::Seat(owner)) if *seat == owner)
            });
            if given_back || self.lead_owner_here(owned.owner) {
                continue;
            }
            let words = format!(
                "lead's owner, {}, has left the game: the lead passes on",
                self.owner_words(owned.owner)
            );
            self.lobby_log(wing_label(owned.wing), LobbyEvent::Lead(words));
            commands.push(MissionCommand::LeadLeft { owner: owned.owner });
        }
    }

    /// Whether the player behind a lead `owner` is still in the game: a
    /// player (not closing) flies from, takes with, or has held or pending
    /// the seat; or a player is still kept for the plane the AI flies (an
    /// away or dropped player, or one back to revive from it).
    fn lead_owner_here(&self, owner: LeadOwner) -> bool {
        match owner {
            LeadOwner::Seat(seat) => {
                self.peers.values().any(|peer| match peer.stage {
                    Stage::Closing { .. } => false,
                    Stage::Taking { seat: taking, .. } => taking == seat,
                    _ => peer.seat == Some(seat),
                }) || self.revival.held_seats().any(|held| held == seat)
                    || self.revival.pending_seats().any(|pending| pending == seat)
            }
            LeadOwner::Away(plane) => !matches!(self.lineage_holder(plane), Holder::Ai),
        }
    }

    /// The player behind a lead owner, for the log.
    fn owner_words(&self, owner: LeadOwner) -> String {
        match owner {
            LeadOwner::Seat(seat) => self
                .peers
                .values()
                .find(|peer| peer.seat == Some(seat))
                .map(|peer| peer.callsign.clone())
                .or_else(|| self.held_callsign(seat))
                .unwrap_or_else(|| format!("the player of seat {}", seat.0)),
            LeadOwner::Away(plane) => match self.reserved_for(plane) {
                Some(callsign) => format!("{callsign} (away)"),
                None => format!("the player of plane {}", plane.0),
            },
        }
    }

    /// A plane as the lead hold's log names it: its player and the plane,
    /// or the plane and the AI.
    fn leader_words(&self, plane: PlaneId) -> String {
        match self.world.roster.seat_of(plane) {
            Some(seat) => format!(
                "{} in plane {}",
                self.owner_words(LeadOwner::Seat(seat)),
                plane.0
            ),
            None => format!("plane {} (AI)", plane.0),
        }
    }

    /// After the step (slice R2): each lead change of the tick and each
    /// change of a wing's owner, for the log. Nothing with the hold off.
    fn lead_hold_after(&mut self) {
        let before = std::mem::take(&mut self.revival.lead_before);
        if !self.world.lead_hold() {
            return;
        }
        let changes = self
            .world
            .ai_wings
            .as_ref()
            .map(|wings| wings.last_output().leadership.clone())
            .unwrap_or_default();
        for change in changes {
            let wing = tore_sim::ai::launch::WingId {
                side: if change.side == tore_world::ai_wings::ENEMY_SIDE {
                    Side::Enemy
                } else {
                    Side::Friendly
                },
                index: change.wing,
            };
            let leader = self.leader_words(PlaneId(change.leader));
            let words = if change.reclaimed {
                format!("lead goes back to {leader}")
            } else if change.acting {
                let owner = self
                    .world
                    .lead_owner(wing)
                    .map_or_else(|| "its owner".to_owned(), |o| self.owner_words(o));
                format!("lead passes to {leader}, standing in for {owner}")
            } else {
                format!("lead passes to {leader}")
            };
            self.lobby_log(wing_label(wing), LobbyEvent::Lead(words));
        }
        let after = self.world.lead_owners().to_vec();
        for owned in &after {
            let was = before
                .iter()
                .find(|b| b.wing == owned.wing)
                .map(|b| b.owner);
            if was == Some(owned.owner) {
                continue;
            }
            let words = match owned.owner {
                LeadOwner::Seat(_) => {
                    format!("lead belongs to {}", self.owner_words(owned.owner))
                }
                // Kept for an away or dropped player; a plane nobody is kept
                // for is told as left at the next tick.
                LeadOwner::Away(plane) if self.reserved_for(plane).is_some() => format!(
                    "lead is kept for {}, the AI flying plane {}",
                    self.owner_words(owned.owner),
                    plane.0
                ),
                LeadOwner::Away(_) => continue,
            };
            self.lobby_log(wing_label(owned.wing), LobbyEvent::Lead(words));
        }
        for was in &before {
            if !after.iter().any(|owned| owned.wing == was.wing) {
                self.lobby_log(
                    wing_label(was.wing),
                    LobbyEvent::Lead(
                        "lead has no owner now: the flight's own succession leads it".into(),
                    ),
                );
            }
        }
    }
}

// Stage K: the revivals part of the session's state (slice K1).
#[path = "revive_state.rs"]
pub(super) mod state;
