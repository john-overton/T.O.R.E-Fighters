//! What a client was given, kept for converting its capture into a replay
//! (docs/ARCHITECTURE.md, "Converting a capture into a replay").
//!
//! The observer sits inside a [`super::Client`] that is run again from a
//! capture. It keeps, flight by flight: every state any snapshot carried for
//! every entity, the events, the roster, the host's exact states of the own
//! plane and the plane's predicted ticks. Nothing here changes what the
//! client does; a client with no observer pays for none of it.

use super::prediction::Trace;
use crate::wire::entity::{EntityKey, EntityState};
use crate::wire::events::ReceivedEvent;
use crate::wire::messages::Roster;
use crate::wire::names::ReceivedNames;
use crate::wire::snapshot::ReceivedSnapshot;
use std::collections::BTreeMap;
use std::time::Duration;
use tore_sim::combat::live::Configuration;
use tore_sim::flight::{self, PilotInput};
use tore_world::snapshot::AircraftPose;
use tore_world::terrain::Terrain;
use tore_world::world::plane::OwnshipTerms;

/// The own plane at one tick, as a replay stores an aircraft.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnSample {
    pub tick: u64,
    /// The plane as drawn: position, attitude, velocity, devices, engine,
    /// damage and wreck.
    pub pose: AircraftPose,
    /// Airspeed, feet per second.
    pub airspeed: f64,
    pub g: f64,
    /// Internal and external fuel, pounds.
    pub fuel_lb: f64,
    /// Pitch, roll and yaw as flown (zero in a sample taken from the host's
    /// exact state, which does not carry the stick) and the throttle.
    pub controls: [f64; 4],
    pub on_ground: bool,
    pub alive: bool,
    pub ejected: bool,
    pub wreck_gone: bool,
}

/// The own plane's flight `flight` at `tick` as a sample. `pilot` is the
/// stick the tick was stepped with, when it was stepped.
pub fn own_sample(
    tick: u64,
    plane: u32,
    flight: &flight::State,
    config: &Configuration,
    terms: Option<&OwnshipTerms>,
    terrain: &Terrain,
    pilot: Option<&PilotInput>,
) -> OwnSample {
    let hp = terms.map_or(config.damage_capacity, |t| t.hp);
    let ground = terrain
        .surface(flight.position[0], flight.position[2])
        .height;
    let stick = pilot.map_or([0.; 3], |p| [p.pitch, p.roll, p.yaw]);
    OwnSample {
        tick,
        pose: super::player_pose(plane, flight, config, terms),
        airspeed: flight.speed,
        g: flight.g,
        fuel_lb: flight.fuel + flight.systems.external_lbs(),
        controls: [stick[0], stick[1], stick[2], flight.throttle],
        on_ground: flight.supported_at(ground),
        alive: !flight.crashed && hp > 0 && !flight.systems.pilot.dead && flight.escape.is_none(),
        ejected: flight.escape.is_some(),
        wreck_gone: flight.wreck_gone(),
    }
}

/// An event with the client time it arrived.
#[derive(Clone, Debug, PartialEq)]
pub struct SeenEvent {
    pub arrived: Duration,
    pub event: ReceivedEvent,
}

/// One flight of the connection: from its first section to its end.
#[derive(Clone, Debug, Default)]
pub struct FlightSeen {
    /// The connection's flight number.
    pub flight: u8,
    /// Client time the flight began and, when it did, ended.
    pub began: Duration,
    pub ended: Option<Duration>,
    /// The seat, the plane and the tick the player was seated at.
    pub seat: Option<SeatSeen>,
    /// The roster as last sent.
    pub roster: Option<Roster>,
    /// Every state received for each entity, by host tick.
    pub states: BTreeMap<EntityKey, Vec<(u32, EntityState)>>,
    /// The newest tick each entity was removed at.
    pub removed: BTreeMap<EntityKey, u32>,
    /// The tick of every snapshot, with its client arrival time.
    pub snapshots: Vec<(u32, Duration)>,
    /// The own plane's predicted ticks and the host's exact states, in the
    /// order the client saw them.
    pub trace: Vec<Trace>,
    pub events: Vec<SeenEvent>,
    /// The connection's name table at the end of the flight.
    pub names: ReceivedNames,
}

/// Where the player sat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeatSeen {
    pub seat: u8,
    pub plane: u32,
    pub tick: u32,
}

/// What a client was given, over a whole session.
#[derive(Clone, Debug, Default)]
pub struct Observed {
    pub flights: Vec<FlightSeen>,
}

impl Observed {
    fn current(&mut self) -> Option<&mut FlightSeen> {
        self.flights.last_mut().filter(|f| f.ended.is_none())
    }

    /// A flight begins (its first section arrived, or its Seated message).
    pub(crate) fn begin_flight(
        &mut self,
        now: Duration,
        flight: u8,
        names: Option<&ReceivedNames>,
    ) {
        self.end_flight(now, names);
        self.flights.push(FlightSeen {
            flight,
            began: now,
            ..FlightSeen::default()
        });
    }

    /// The current flight ends; `names` is the table it used.
    pub(crate) fn end_flight(&mut self, now: Duration, names: Option<&ReceivedNames>) {
        if let Some(flight) = self.current() {
            flight.ended = Some(now);
            if let Some(names) = names {
                flight.names = names.clone();
            }
        }
    }

    /// A flight is in progress: starts one when the last has ended.
    pub(crate) fn ensure_flight(&mut self, now: Duration, flight: u8) {
        if self.current().is_none() {
            self.flights.push(FlightSeen {
                flight,
                began: now,
                ..FlightSeen::default()
            });
        }
    }

    pub(crate) fn seated(&mut self, seat: SeatSeen, roster: &Roster, sample: OwnSample) {
        if let Some(flight) = self.current() {
            flight.seat = Some(seat);
            flight.roster = Some(roster.clone());
            flight.trace.push(Trace::Host {
                sample,
                differs: false,
            });
        }
    }

    pub(crate) fn roster(&mut self, roster: &Roster) {
        if let Some(flight) = self.current() {
            flight.roster = Some(roster.clone());
        }
    }

    pub(crate) fn snapshot(&mut self, now: Duration, received: &ReceivedSnapshot) {
        let Some(flight) = self.current() else {
            return;
        };
        flight.snapshots.push((received.tick, now));
        for entity in &received.updated {
            let states = flight.states.entry(entity.key()).or_default();
            match states.binary_search_by_key(&received.tick, |(tick, _)| *tick) {
                Ok(at) => states[at].1 = entity.state,
                Err(at) => states.insert(at, (received.tick, entity.state)),
            }
        }
        for key in &received.removed {
            let at = flight.removed.entry(*key).or_insert(received.tick);
            *at = (*at).max(received.tick);
        }
    }

    pub(crate) fn event(&mut self, now: Duration, event: &ReceivedEvent) {
        if let Some(flight) = self.current() {
            flight.events.push(SeenEvent {
                arrived: now,
                event: event.clone(),
            });
        }
    }

    pub(crate) fn trace(&mut self, trace: Vec<Trace>) {
        if trace.is_empty() {
            return;
        }
        if let Some(flight) = self.current() {
            flight.trace.extend(trace);
        }
    }
}
