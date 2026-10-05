//! The flight data link's picture: who tracks what, who is locked on what and
//! who attacks what, for every flight of the mission, human and AI alike.
//! Stage G of the multiplayer plan; the guide is `docs/DATALINK.md`, the design
//! is `docs/ARCHITECTURE.md`, "Flight data link".
//!
//! [`DataLink`] observes. It reads the human ownships after combat
//! ([`DataLink::before_ai`]) and the AI actors after the AI step
//! ([`DataLink::after_ai`]) and never changes either. Locks and engagements
//! follow the tick they happen. The tracks and each member's coarse state are
//! published every [`PUBLISH_TICKS`] ticks, so between publishing ticks the
//! picture's tracks do not move. In this slice (G0) nothing consumes the
//! picture: later slices let the AI, the radio and the displays read it.
//!
//! What a member receives is worked out from the picture, never stored per
//! member: [`DataLink::view`] for a seat's readout and [`DataLink::ai_input`]
//! for the AI. A pair of members is linked by the rules of
//! [`tore_sim::datalink`].
//!
//! Collections are `BTreeMap`, `BTreeSet` and `Vec` in plane id order, as every
//! mission type is, and nothing here rolls a random number, so the picture is
//! identical on every platform and from one run to the next.

mod ai_input;
mod assign;
pub mod calls;
// Exact checkpoints (docs/formats/checkpoint.md): the data link section.
#[path = "datalink_checkpoint.rs"]
mod checkpoint;
mod journal;
mod picture;
mod view;

pub use ai_input::{AiInput, FlightFeed};
pub use assign::ClearReason;
pub use journal::{CAPACITY as JOURNAL_CAPACITY, Entry, Journal};
pub use picture::{
    Assignment, Damage, Engagement, FLIGHT_TRACKS, FlightId, FlightPicture, Fuel, Lock,
    MemberStatus, PUBLISH_TICKS, SEAT_TRACKS, Source, Track, Weapons, flight_key,
};
pub use view::{LinkView, TrackSource, ViewTrack};

use crate::{
    ai_wings::{AiWings, ENEMY_SIDE, FRIENDLY_SIDE},
    seats::{Pilot, Roster, SeatId},
    world::Cockpit,
};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::aircraft::AircraftId;
use tore_sim::{
    ai::{
        awareness::Snapshot,
        launch::Side,
        link,
        mission::AiActor,
        route::{self, FuelState},
        weapon_service::{self, Rounds, TargetClass},
    },
    combat::{
        live::{self, NO_SIDE},
        missiles::{Profile, TargetRole},
    },
    datalink::has_radar,
    models::FlightModel,
    sensors::Channel,
};

/// One plane of the mission as the picture knows it: where it flies, what it
/// can share and whether it is alive. Rebuilt every tick from the roster, so a
/// handoff between the AI and a human changes nothing here but `human`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    pub plane: u32,
    pub flight: FlightId,
    /// Its place in its wing from zero.
    pub member: u8,
    /// The aircraft type; `None` until combat holds one for the plane.
    pub aircraft: Option<AircraftId>,
    /// The aircraft type has a radar (`false` while the type is unknown). It
    /// changes what the member's player sees, never whether it is linked.
    pub radar: bool,
    /// A human flies it.
    pub human: bool,
    pub alive: bool,
    /// World position, feet, as the member's last step left it.
    pub position: [f64; 3],
}

/// What the picture reads of the world at one tick.
pub struct Scene<'a> {
    /// The combat tick.
    pub tick: u64,
    pub roster: &'a Roster,
    pub state: &'a live::State,
    pub cockpits: &'a [Cockpit],
    pub wings: Option<&'a AiWings>,
}

/// The flight data link's picture and the state that follows it.
#[derive(Clone, Debug, Default)]
pub struct DataLink {
    /// The combat tick of the last observation.
    tick: u64,
    /// Every plane of the roster, in plane id order.
    members: Vec<Member>,
    /// One picture per flight, as last published, friendly flights first.
    pictures: Vec<FlightPicture>,
    /// Locks held, by the plane that holds them.
    locks: BTreeMap<u32, Lock>,
    /// Who attacks what: an AI's target, a human's locked target.
    engaged: BTreeMap<u32, u32>,
    /// What the lead gave each member, by receiver ([`DataLink::assign`]).
    assignments: BTreeMap<u32, Assignment>,
    /// Lock pairs already warned of: (plane, plane, target). Written from G6.
    warned: BTreeSet<(u32, u32, u32)>,
    /// The tick of each seat's last sort warning. Written from G6.
    seat_warned: BTreeMap<SeatId, u64>,
    /// Planes already announced to the journal.
    announced: BTreeSet<u32>,
    /// Write-only, drained by the host.
    journal: Journal,
}

impl DataLink {
    /// The step's first half, after combat and before the AI: refreshes the
    /// members, reads each human's lock, and on a publishing tick publishes
    /// every flight's tracks and member state.
    pub fn before_ai(&mut self, scene: &Scene<'_>) {
        self.tick = scene.tick;
        self.refresh_members(scene);
        for index in 0..self.members.len() {
            let member = self.members[index];
            if !member.human {
                continue;
            }
            let lock = member
                .alive
                .then(|| human_lock(scene.state, member.plane))
                .flatten();
            self.set_lock(member.plane, lock, scene.tick);
            self.set_engagement(member.plane, lock);
        }
        if scene.tick > 0 && scene.tick.is_multiple_of(PUBLISH_TICKS) {
            self.publish(scene);
        }
    }

    /// The step's second half, after the AI: reads each AI actor's lock and
    /// target, then ends the assignments the mission has finished and marks
    /// the ones the receiver has locked.
    pub fn after_ai(&mut self, tick: u64, wings: Option<&AiWings>) {
        for index in 0..self.members.len() {
            let member = self.members[index];
            if member.human {
                continue;
            }
            let actor = wings.and_then(|wings| wings.mission().actor(member.plane));
            let alive = actor.is_some_and(AiActor::alive);
            self.members[index].alive = alive;
            if let Some(actor) = actor {
                self.members[index].position = actor.flight().position;
            }
            let (target, lock) = match actor.filter(|_| alive) {
                Some(actor) => ai_target(actor),
                None => (None, None),
            };
            self.set_lock(member.plane, lock, tick);
            self.set_engagement(member.plane, target);
        }
        self.settle(tick, wings);
    }

    fn refresh_members(&mut self, scene: &Scene<'_>) {
        self.members.clear();
        for plane in scene.roster.planes() {
            let human = matches!(plane.pilot, Pilot::Human(_));
            let (aircraft, alive, position) = if human {
                let cockpit = scene.cockpits.iter().find(|c| c.plane == plane.id);
                let ownship = scene.state.ownship(plane.id.0);
                (
                    ownship.map(|own| own.configuration().aircraft),
                    cockpit.is_some_and(|c| !c.flight.crashed)
                        && ownship.is_none_or(|own| own.hp > 0),
                    cockpit.map_or([0.; 3], |c| c.flight.position),
                )
            } else {
                let actor = scene
                    .wings
                    .and_then(|wings| wings.mission().actor(plane.id.0));
                (
                    actor.map(|actor| actor.identity().aircraft),
                    actor.is_some_and(AiActor::alive),
                    actor.map_or([0.; 3], |actor| actor.flight().position),
                )
            };
            let radar = aircraft.is_some_and(has_radar);
            if self.announced.insert(plane.id.0) {
                self.journal.push(Entry::Member {
                    tick: scene.tick,
                    plane: plane.id.0,
                    radar,
                });
            }
            self.members.push(Member {
                plane: plane.id.0,
                flight: plane.slot.wing,
                member: plane.slot.member,
                aircraft,
                radar,
                human,
                alive,
                position,
            });
        }
    }

    fn set_lock(&mut self, plane: u32, target: Option<u32>, tick: u64) {
        match (self.locks.get(&plane).copied(), target) {
            (Some(held), Some(target)) if held.target == target => {}
            (held, target) => {
                if let Some(held) = held {
                    self.locks.remove(&plane);
                    self.journal.push(Entry::Unlock {
                        tick,
                        plane,
                        target: held.target,
                    });
                }
                if let Some(target) = target {
                    self.locks.insert(
                        plane,
                        Lock {
                            target,
                            since: tick,
                        },
                    );
                    self.journal.push(Entry::Lock {
                        tick,
                        plane,
                        target,
                    });
                }
            }
        }
    }

    fn set_engagement(&mut self, plane: u32, target: Option<u32>) {
        match target {
            Some(target) => {
                self.engaged.insert(plane, target);
            }
            None => {
                self.engaged.remove(&plane);
            }
        }
    }

    /// Publishes every flight's tracks and member state.
    fn publish(&mut self, scene: &Scene<'_>) {
        self.pictures = self
            .flights()
            .into_iter()
            .map(|flight| self.publish_flight(scene, flight))
            .collect();
    }

    fn publish_flight(&self, scene: &Scene<'_>, flight: FlightId) -> FlightPicture {
        let in_flight = || self.members.iter().filter(move |m| m.flight == flight);
        // Every living member of the flight reports: every aircraft is linked.
        let reporters: Vec<&Member> = in_flight().filter(|m| m.alive).collect();
        let mut best: BTreeMap<u32, Track> = BTreeMap::new();
        for reporter in &reporters {
            let held = if reporter.human {
                human_tracks(scene, reporter)
            } else {
                ai_tracks(scene, reporter)
            };
            for track in held {
                match best.get(&track.target) {
                    // The freshest report wins; reporters come in plane id
                    // order, so a tie keeps the lower id.
                    Some(old) if old.observed >= track.observed => {}
                    _ => {
                        best.insert(track.target, track);
                    }
                }
            }
        }
        let lead = self.lead_of(scene, flight);
        let mut tracks: Vec<Track> = if lead.is_some() {
            best.into_values().collect()
        } else {
            Vec::new()
        };
        if let Some(lead) = lead {
            tracks.sort_by(|a, b| {
                squared_feet(a.position, lead.position)
                    .total_cmp(&squared_feet(b.position, lead.position))
                    .then(a.target.cmp(&b.target))
            });
            tracks.truncate(FLIGHT_TRACKS);
        }
        let status = reporters
            .iter()
            .map(|reporter| member_status(scene, reporter))
            .collect();
        FlightPicture {
            flight,
            tick: scene.tick,
            tracks,
            status,
        }
    }

    /// The member that leads `flight` now: the AI mission's current leader of
    /// the wing, or the lowest-numbered living member where it names none.
    fn lead_of(&self, scene: &Scene<'_>, flight: FlightId) -> Option<&Member> {
        let side = match flight.side {
            Side::Friendly => FRIENDLY_SIDE,
            Side::Enemy => ENEMY_SIDE,
        };
        let named = scene
            .wings
            .and_then(|wings| wings.mission().wing_leader(side, flight.index))
            .and_then(|plane| {
                self.members
                    .iter()
                    .find(|m| m.plane == plane && m.flight == flight && m.alive)
            });
        named.or_else(|| {
            self.members
                .iter()
                .filter(|m| m.flight == flight && m.alive)
                .min_by_key(|m| (m.member, m.plane))
        })
    }

    /// Every plane of the roster, in plane id order.
    pub fn members(&self) -> &[Member] {
        &self.members
    }

    pub fn member(&self, plane: u32) -> Option<&Member> {
        self.members.iter().find(|m| m.plane == plane)
    }

    /// Each flight's picture as last published, friendly flights first.
    pub fn pictures(&self) -> &[FlightPicture] {
        &self.pictures
    }

    pub fn picture(&self, flight: FlightId) -> Option<&FlightPicture> {
        self.pictures.iter().find(|p| p.flight == flight)
    }

    /// The lock `plane` holds.
    pub fn lock(&self, plane: u32) -> Option<Lock> {
        self.locks.get(&plane).copied()
    }

    /// Every lock held, by plane.
    pub fn locks(&self) -> &BTreeMap<u32, Lock> {
        &self.locks
    }

    /// The target `plane` attacks: an AI's target, a human's locked target.
    pub fn engaged(&self, plane: u32) -> Option<u32> {
        self.engaged.get(&plane).copied()
    }

    /// Every engagement, by plane.
    pub fn engagements(&self) -> Vec<Engagement> {
        self.engaged
            .iter()
            .map(|(&plane, &target)| Engagement { plane, target })
            .collect()
    }

    /// What the lead gave each member, by receiver.
    pub fn assignments(&self) -> &BTreeMap<u32, Assignment> {
        &self.assignments
    }

    /// Lock pairs already warned of. Empty until slice G6.
    pub fn warned(&self) -> &BTreeSet<(u32, u32, u32)> {
        &self.warned
    }

    /// The tick of each seat's last sort warning. Empty until slice G6.
    pub fn seat_warned(&self) -> &BTreeMap<SeatId, u64> {
        &self.seat_warned
    }

    /// The combat tick of the last observation.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Takes the journal's entries, oldest first.
    pub fn take_journal(&mut self) -> Vec<Entry> {
        self.journal.take()
    }
}

/// The squared distance between two points, feet.
fn squared_feet(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}

/// The target a human holds a radar lock on: its sensors' acquired target.
fn human_lock(state: &live::State, plane: u32) -> Option<u32> {
    state.ownship(plane)?.sensors.acquired()
}

/// An AI actor's target, and the lock it holds on it: the AI's one engagement
/// and lock rule ([`link::engagement_of`], [`link::lock_of`]), which the
/// warning receiver also reads ([`AiWings::locks_on`]).
fn ai_target(actor: &AiActor) -> (Option<u32>, Option<u32>) {
    (link::engagement_of(actor), link::lock_of(actor))
}

/// The hostile aircraft a human's radar, infrared sensor and eyes hold, in
/// the order the readout lists them: the active channel first, then visual.
fn human_tracks(scene: &Scene<'_>, member: &Member) -> Vec<Track> {
    let Some(view) = scene.state.view(member.plane) else {
        return Vec::new();
    };
    let own = view.ownship();
    let sensors = &own.sensors;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for contact in sensors.contacts().iter().chain(sensors.visual()) {
        if contact.destroyed || !seen.insert(contact.id) {
            continue;
        }
        let hostile = view.contact(contact.id).is_some_and(|row| {
            row.role == TargetRole::Aircraft
                && row.hp > 0
                && row.side != NO_SIDE
                && row.side != own.side
        });
        if hostile {
            out.push(Track {
                reporter: member.plane,
                target: contact.id,
                position: contact.position,
                velocity: contact.velocity,
                channel: contact.channel,
                observed: scene.tick,
            });
        }
    }
    out
}

/// The hostile aircraft an AI actor's awareness holds this tick.
fn ai_tracks(scene: &Scene<'_>, member: &Member) -> Vec<Track> {
    let Some(actor) = scene
        .wings
        .and_then(|wings| wings.mission().actor(member.plane))
    else {
        return Vec::new();
    };
    let side = actor.identity().side;
    actor
        .awareness()
        .current_observations()
        .filter(|seen| seen.target.side != side)
        .map(|seen| Track {
            reporter: member.plane,
            target: seen.target.id,
            position: seen.target.position,
            velocity: seen.velocity,
            channel: channel_of(seen),
            observed: seen.last_observed_tick,
        })
        .collect()
}

/// The channel an observation came from: the one stamped at its latest tick,
/// radar before infrared before visual. A fixture observation (tests only)
/// counts as visual.
fn channel_of(seen: &Snapshot) -> Channel {
    let latest = Some(seen.last_observed_tick);
    if seen.source_ticks.radar == latest {
        Channel::Radar
    } else if seen.source_ticks.infrared == latest {
        Channel::Infrared
    } else {
        Channel::Visual
    }
}

/// A member's coarse state, as a pilot would report it.
fn member_status(scene: &Scene<'_>, member: &Member) -> MemberStatus {
    let (fuel, weapons, damage) = if member.human {
        human_state(scene, member.plane)
    } else {
        ai_state(scene, member.plane)
    };
    MemberStatus {
        plane: member.plane,
        fuel,
        weapons,
        damage,
    }
}

/// How hurt an aircraft is: none at full hit points, heavy at half or less.
fn damage_of(hp: i32, initial: i32) -> Damage {
    if hp >= initial {
        Damage::None
    } else if hp.saturating_mul(2) <= initial {
        Damage::Heavy
    } else {
        Damage::Light
    }
}

/// `fitted` (agent decision, 2026-10-05): a human's fuel level. The radio
/// judges joker and bingo against the flight home, and the human's home point
/// belongs to the crew voice, so the picture reports only the two levels that
/// need no home: fumes under the critical endurance, and out. Endurance is the
/// rule of the crew voice: all remaining fuel at the military flow scaled by
/// the throttle, floored at 10 percent.
fn human_state(scene: &Scene<'_>, plane: u32) -> (Fuel, Weapons, Damage) {
    let Some(own) = scene.state.ownship(plane) else {
        return Default::default();
    };
    let fuel = scene
        .cockpits
        .iter()
        .find(|c| c.plane.0 == plane)
        .map_or(Fuel::Normal, |cockpit| {
            let flight = &cockpit.flight;
            let propulsion = &flight.model().configuration().propulsion;
            let flow = (propulsion.military_fuel_lbs_per_second * flight.throttle.max(0.1))
                .max(tore_sim::ai::mission::MINIMUM_FUEL_FLOW_LBS_PER_S);
            let endurance = (flight.fuel + flight.systems.external_lbs()) / flow;
            if endurance <= 0. {
                Fuel::Out
            } else if endurance < route::CRITICAL_ENDURANCE_S {
                Fuel::Fumes
            } else {
                Fuel::Normal
            }
        });
    let config = own.configuration();
    let mut missiles = false;
    let mut guns = false;
    for (station, store) in config.stations.iter().enumerate() {
        // A failed station (bit 0x8000 of its count) fires nothing.
        if own
            .ammo
            .get(station)
            .is_none_or(|count| count & 0x8000 != 0)
            || own.rounds(station) == 0
        {
            continue;
        }
        if live::is_gun(&store.weapon) {
            guns = true;
        } else if Profile::for_weapon(&store.weapon)
            .is_some_and(|profile| profile.role == TargetRole::Aircraft)
        {
            missiles = true;
        }
    }
    let weapons = if missiles {
        Weapons::Missiles
    } else if guns {
        Weapons::GunsOnly
    } else {
        Weapons::Winchester
    };
    (fuel, weapons, damage_of(own.hp, config.damage_capacity))
}

fn ai_state(scene: &Scene<'_>, plane: u32) -> (Fuel, Weapons, Damage) {
    let Some(actor) = scene.wings.and_then(|wings| wings.mission().actor(plane)) else {
        return Default::default();
    };
    let fuel = match actor.controller().fuel_state() {
        Some(FuelState::Caution) => Fuel::Joker,
        Some(FuelState::Bingo) => Fuel::Bingo,
        Some(FuelState::Critical) => Fuel::Fumes,
        Some(FuelState::OutOfFuel) => Fuel::Out,
        Some(FuelState::Ok | FuelState::NoManagement) | None => Fuel::Normal,
    };
    let stocked = |guided: bool| {
        actor.stations().iter().any(|station| {
            station.guided == guided
                && !station.store.inhibited
                && weapon_service::store_eligible(station.capability, TargetClass::Air)
                && !matches!(station.store.rounds, Rounds::Finite(0))
        })
    };
    let weapons = if stocked(true) {
        Weapons::Missiles
    } else if stocked(false) {
        Weapons::GunsOnly
    } else {
        Weapons::Winchester
    };
    let damage = scene
        .state
        .targets
        .iter()
        .find(|row| row.id == plane)
        .map_or(Damage::None, |row| damage_of(row.hp, row.initial_hp));
    (fuel, weapons, damage)
}
