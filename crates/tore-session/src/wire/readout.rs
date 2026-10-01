//! The cockpit readout inside a snapshot (net-protocol.md, "Cockpit
//! readout"): what the seat's displays show, coded against the readout the
//! client acknowledged.
//!
//! [`QReadout`] is a [`CockpitReadout`] in the wire's whole numbers: a few
//! scalar groups and lists keyed by id. The record codes each part in turn,
//! in the order of [`PARTS`], which is also the order of importance: a
//! scalar group goes whole when it changed, a list sends its removed ids and
//! its changed entries, each against the baseline's entry with its id, with
//! positions predicted from their velocities. When the readout's share of the
//! packet is full, what is left waits: the client keeps the baseline's value
//! (moved along by its prediction) and so does the host's record of what the
//! client holds, so the next packet catches up from there.
//!
//! *Agent decisions:* the scope, map and warning receiver's positions are in
//! whole feet and their velocities in 1/4 ft/s, and an emitter's bearing,
//! strength and distance are as coarse as the warning receiver draws them (the target rows and seeker
//! observations, which the HUD and the views draw from, keep the entities'
//! 1/32 ft and 1/64 ft/s); a contact's bearing, elevation and distance are not
//! sent, since the client works them out around its own predicted plane
//! ([`QReadout::readout`]); lists come back in id order; and the tower's
//! service travels as its selected airport, its clearance and the objects out
//! of action, from which the client rebuilds a service that answers the ILS
//! as the host's does.

use super::bits::{self, read_u32};
use super::flat::{self, Kind, List, ListRaw, Schema, Slow};
use super::{WireError, WireResult};
use std::collections::{BTreeMap, VecDeque};
use tore_codec::{BitReader, BitWriter};
use tore_formats::aircraft::AircraftId;
use tore_sim::ai::controller::Activity;
use tore_sim::airport::{ApproachEnd, Scene, Service};
use tore_sim::attitude::{Basis, Vector};
use tore_sim::combat::live::{DamageSection, Readiness, SeekerTone};
use tore_sim::combat::missiles::{FiringBand, LaunchMode, seeker};
use tore_sim::combat::threats::{EvidenceSource, GuidanceClass, ThreatRecord};
use tore_sim::sensors::detection::Sighting;
use tore_sim::sensors::passive::{Emitter, Symbol};
use tore_sim::sensors::{Channel, Contact, Plot, Strobe, Support};
use tore_world::readout::{
    self as world_readout, AirportReadout, CockpitReadout, Countermeasures, DamageReadout,
    Estimates, InboundMissile, MapRow, MusicReadout, RwrReadout, SeekerReadout, SensorReadout,
    Stores, TargetRow, Targets, Trail,
};
use tore_world::snapshot::Damage;
use tore_world::target_window::{Pilot, TargetBrief, TargetObjective};

/// Scope, map and warning positions: 1 ft.
const COARSE_FT: f64 = 1.;
/// Their velocities: 1/4 ft/s, so a position moves `v × ticks / 480`.
const COARSE_FPS: f64 = 0.25;
/// Target rows and seeker observations, as the entities: 1/32 ft.
const FINE_FT: f64 = 1. / 32.;
/// Their velocities: 1/64 ft/s, so a position moves `v × ticks / 240`.
const FINE_FPS: f64 = 1. / 64.;
/// Angles in radians: 2^-12 of a turn.
const ANGLE: f64 = std::f64::consts::TAU / 4096.;
/// Threat bearings in degrees: 1/4 degree.
const DEGREES: f64 = 1. / 4.;
/// The warning receiver's emitter bearings: 2^-8 of a turn (1.4 degrees),
/// as fine as its dial draws them.
const DIAL: f64 = std::f64::consts::TAU / 256.;
/// An emitter's strength, which only chooses its symbol's emphasis: 1/32.
const EMPHASIS: f64 = 1. / 32.;
/// An emitter's distance: 1/8 nautical mile.
const EMITTER_NMI: f64 = 1. / 8.;
/// Strengths and qualities: 1/1024.
const FRACTION: f64 = 1. / 1024.;
/// Small angles (a strobe's width, an observation's off-axis): 1/4096 rad.
const SMALL_ANGLE: f64 = 1. / 4096.;
/// Nautical miles: 1/64.
const NMI: f64 = 1. / 64.;
/// Seconds: 1/64.
const SECONDS: f64 = 1. / 64.;

fn q(value: f64, step: f64) -> i64 {
    bits::steps(value, step)
}

fn v(steps: i64, step: f64) -> f64 {
    bits::value(steps, step)
}

fn flag(value: bool) -> i64 {
    i64::from(value)
}

const BIT: Kind = Kind::Slow(Slow::Bits(1, 1));
const D: Kind = Kind::Diff;

const CONTACT: Schema = &[
    Kind::Slow(Slow::Bits(2, 2)),
    Kind::Pos {
        vel: 4,
        divisor: 480,
    },
    Kind::Pos {
        vel: 5,
        divisor: 480,
    },
    Kind::Pos {
        vel: 6,
        divisor: 480,
    },
    D,
    D,
    D,
    BIT,
    BIT,
];
const MAP: Schema = &[
    Kind::Slow(Slow::Bits(2, 2)),
    Kind::Pos {
        vel: 4,
        divisor: 480,
    },
    Kind::Pos {
        vel: 5,
        divisor: 480,
    },
    Kind::Pos {
        vel: 6,
        divisor: 480,
    },
    D,
    D,
    D,
    BIT,
    BIT,
    BIT,
    BIT,
    Kind::Slow(Slow::Bits(4, 14)),
];
const PLOT: Schema = &[Kind::Slow(Slow::Bits(2, 2)), D, D, D, D, D, D, D];
const STROBE: Schema = &[D, D, D, D, D];
const EMITTER: Schema = &[D, BIT, D, Kind::Slow(Slow::Bits(2, 2)), D];
const THREAT: Schema = &[
    Kind::Slow(Slow::Bits(2, 3)),
    D,
    D,
    BIT,
    Kind::Pos {
        vel: 8,
        divisor: 480,
    },
    Kind::Pos {
        vel: 9,
        divisor: 480,
    },
    Kind::Pos {
        vel: 10,
        divisor: 480,
    },
    BIT,
    D,
    D,
    D,
    Kind::Slow(Slow::Bits(2, 3)),
    BIT,
    BIT,
    BIT,
    BIT,
    D,
];
const INBOUND: Schema = &[D, D, D, Kind::Slow(Slow::Bits(8, 255)), BIT];
const ROW: Schema = &[
    Kind::Slow(Slow::Bits(4, 14)),
    Kind::Pos {
        vel: 4,
        divisor: 240,
    },
    Kind::Pos {
        vel: 5,
        divisor: 240,
    },
    Kind::Pos {
        vel: 6,
        divisor: 240,
    },
    D,
    D,
    D,
    Kind::Slow(Slow::Int),
    Kind::Slow(Slow::Int),
    Kind::Slow(Slow::Int),
    Kind::Slow(Slow::Int),
    Kind::Slow(Slow::Int),
    Kind::Slow(Slow::Int),
    Kind::Slow(Slow::Int),
    Kind::Slow(Slow::Int),
    Kind::Slow(Slow::Bits(3, 6)),
];
const OBSERVATION: Schema = &[
    Kind::Pos {
        vel: 3,
        divisor: 240,
    },
    Kind::Pos {
        vel: 4,
        divisor: 240,
    },
    Kind::Pos {
        vel: 5,
        divisor: 240,
    },
    D,
    D,
    D,
    D,
    D,
    D,
];
const ENEMY: Schema = &[D, D, D];

/// The scalar groups.
mod scalar {
    pub const HEADER: usize = 0;
    pub const STORES: usize = 1;
    pub const COUNTERMEASURES: usize = 2;
    pub const DAMAGE: usize = 3;
    pub const SEEKER: usize = 4;
    pub const ESTIMATES: usize = 5;
    pub const TARGETS: usize = 6;
    pub const AIRPORT: usize = 7;
    pub const WINDOW: usize = 8;
    pub const MUSIC: usize = 9;
    pub const LOCKS: usize = 10;
    pub const SENSORS: usize = 11;
    pub const COUNT: usize = 12;
}

/// The lists.
mod list {
    pub const SEEKER_OBSERVATION: usize = 0;
    pub const ESTIMATE_OBSERVATION: usize = 1;
    pub const DISPLAY: usize = 2;
    pub const VIEW: usize = 3;
    pub const ENEMY: usize = 4;
    pub const INBOUND: usize = 5;
    pub const THREATS: usize = 6;
    pub const EMITTERS: usize = 7;
    pub const CONTACTS: usize = 8;
    pub const STROBES: usize = 9;
    pub const PLOTS: usize = 10;
    pub const VISUAL: usize = 11;
    pub const MAP: usize = 12;
    pub const COUNT: usize = 13;
}

/// The most values a scalar group holds.
const SCALAR_LIMIT: usize = 1_024;
/// Points in one contact's trail.
const TRAIL_LIMIT: usize = 64;

/// One part of the record, in wire order.
#[derive(Clone, Copy, Debug)]
pub enum Part {
    Scalar(usize),
    List(usize),
    Trails,
}

/// The parts in the order they are written, which is their importance:
/// what is left out for room is what comes last.
pub const PARTS: [Part; 26] = [
    Part::Scalar(scalar::HEADER),
    Part::Scalar(scalar::STORES),
    Part::Scalar(scalar::COUNTERMEASURES),
    Part::Scalar(scalar::DAMAGE),
    Part::Scalar(scalar::SEEKER),
    Part::List(list::SEEKER_OBSERVATION),
    Part::Scalar(scalar::ESTIMATES),
    Part::List(list::ESTIMATE_OBSERVATION),
    Part::Scalar(scalar::TARGETS),
    Part::List(list::DISPLAY),
    Part::List(list::VIEW),
    Part::Scalar(scalar::AIRPORT),
    Part::Scalar(scalar::WINDOW),
    Part::Scalar(scalar::MUSIC),
    Part::List(list::ENEMY),
    Part::Scalar(scalar::LOCKS),
    Part::List(list::INBOUND),
    Part::List(list::THREATS),
    Part::List(list::EMITTERS),
    Part::Scalar(scalar::SENSORS),
    Part::List(list::CONTACTS),
    Part::List(list::STROBES),
    Part::List(list::PLOTS),
    Part::Trails,
    Part::List(list::VISUAL),
    Part::List(list::MAP),
];

fn schema(list: usize) -> Schema {
    [
        OBSERVATION,
        OBSERVATION,
        ROW,
        ROW,
        ENEMY,
        INBOUND,
        THREAT,
        EMITTER,
        CONTACT,
        STROBE,
        PLOT,
        CONTACT,
        MAP,
    ][list]
}

fn limit(list: usize) -> usize {
    use world_readout::*;
    [
        1,
        1,
        1,
        1,
        1,
        MAX_INBOUND,
        MAX_THREATS,
        MAX_EMITTERS,
        MAX_CONTACTS,
        MAX_STROBES,
        MAX_PLOTS,
        MAX_VISUAL,
        MAX_MAP,
    ][list]
}

/// The names of the parts, for measurements.
pub fn part_name(part: Part) -> &'static str {
    match part {
        Part::Scalar(index) => [
            "header",
            "stores",
            "countermeasures",
            "damage",
            "seeker",
            "estimates",
            "targets",
            "airport",
            "target window",
            "music",
            "locks",
            "sensors",
        ][index],
        Part::List(index) => [
            "seeker observation",
            "estimate observation",
            "display target",
            "view target",
            "designated enemy",
            "inbound",
            "threats",
            "emitters",
            "contacts",
            "strobes",
            "plots",
            "visual",
            "map",
        ][index],
        Part::Trails => "trails",
    }
}

/// A cockpit readout in the wire's whole numbers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QReadout {
    scalars: [Vec<i64>; scalar::COUNT],
    lists: [List; list::COUNT],
    /// Each contact's trail, its points in whole feet, oldest first.
    trails: BTreeMap<u32, Vec<[i64; 3]>>,
}

// Codes of the readout's enums. Each match is exhaustive, so a new variant
// cannot go uncoded.

fn readiness_code(r: Readiness) -> i64 {
    use Readiness as R;
    match r {
        R::Ready => 0,
        R::Safe => 1,
        R::BayClosed => 2,
        R::LauncherLost => 3,
        R::StationFailed => 4,
        R::Empty => 5,
        R::Capacity => 6,
        R::NoTarget => 7,
        R::TargetDestroyed => 8,
        R::WrongTarget => 9,
        R::NoRadar => 10,
        R::RadarOff => 11,
        R::RadarFailed => 12,
        R::RadarCoverage => 13,
        R::RadarSearchOnly => 14,
        R::RadarAcquiring => 15,
        R::MinimumRange => 16,
        R::MaximumRange => 17,
        R::Altitude => 18,
        R::FieldOfView => 19,
    }
}
const READINESS: [Readiness; 20] = [
    Readiness::Ready,
    Readiness::Safe,
    Readiness::BayClosed,
    Readiness::LauncherLost,
    Readiness::StationFailed,
    Readiness::Empty,
    Readiness::Capacity,
    Readiness::NoTarget,
    Readiness::TargetDestroyed,
    Readiness::WrongTarget,
    Readiness::NoRadar,
    Readiness::RadarOff,
    Readiness::RadarFailed,
    Readiness::RadarCoverage,
    Readiness::RadarSearchOnly,
    Readiness::RadarAcquiring,
    Readiness::MinimumRange,
    Readiness::MaximumRange,
    Readiness::Altitude,
    Readiness::FieldOfView,
];

fn status_code(s: seeker::Status) -> i64 {
    use seeker::Status as S;
    match s {
        S::Unguided => 0,
        S::Midcourse => 1,
        S::Search => 2,
        S::Acquiring => 3,
        S::Locked => 4,
        S::Pitbull => 5,
        S::Memory => 6,
        S::Lost => 7,
        S::Expired => 8,
    }
}
const STATUSES: [seeker::Status; 9] = [
    seeker::Status::Unguided,
    seeker::Status::Midcourse,
    seeker::Status::Search,
    seeker::Status::Acquiring,
    seeker::Status::Locked,
    seeker::Status::Pitbull,
    seeker::Status::Memory,
    seeker::Status::Lost,
    seeker::Status::Expired,
];

fn support_code(s: Support) -> i64 {
    match s {
        Support::Tracked => 0,
        Support::Acquiring => 1,
        Support::SearchOnly => 2,
        Support::TrackCoverage => 3,
        Support::NotSelected => 4,
        Support::NoObservation => 5,
        Support::RadarOff => 6,
        Support::RadarFailed => 7,
        Support::Unavailable => 8,
    }
}
const SUPPORTS: [Support; 9] = [
    Support::Tracked,
    Support::Acquiring,
    Support::SearchOnly,
    Support::TrackCoverage,
    Support::NotSelected,
    Support::NoObservation,
    Support::RadarOff,
    Support::RadarFailed,
    Support::Unavailable,
];

fn activity_code(a: Activity) -> i64 {
    use Activity as A;
    match a {
        A::Idle => 0,
        A::Formation => 1,
        A::Searching => 2,
        A::Acquiring => 3,
        A::Pursuing => 4,
        A::Attacking => 5,
        A::Defending => 6,
        A::Evading => 7,
        A::Breaking => 8,
        A::Rejoining => 9,
        A::ReturningToBase => 10,
        A::Waiting => 11,
        A::Taxiing => 12,
        A::TakingOff => 13,
        A::HoldingMarshal => 14,
        A::Landing => 15,
        A::Landed => 16,
        A::OutOfFuel => 17,
        A::Destroyed => 18,
    }
}
const ACTIVITIES: [Activity; 19] = [
    Activity::Idle,
    Activity::Formation,
    Activity::Searching,
    Activity::Acquiring,
    Activity::Pursuing,
    Activity::Attacking,
    Activity::Defending,
    Activity::Evading,
    Activity::Breaking,
    Activity::Rejoining,
    Activity::ReturningToBase,
    Activity::Waiting,
    Activity::Taxiing,
    Activity::TakingOff,
    Activity::HoldingMarshal,
    Activity::Landing,
    Activity::Landed,
    Activity::OutOfFuel,
    Activity::Destroyed,
];

fn channel_code(c: Channel) -> i64 {
    match c {
        Channel::Radar => 0,
        Channel::Infrared => 1,
        Channel::Visual => 2,
    }
}
const CHANNELS: [Channel; 3] = [Channel::Radar, Channel::Infrared, Channel::Visual];

fn aircraft_code(id: Option<AircraftId>) -> i64 {
    id.and_then(|id| AircraftId::SELECTABLE.iter().position(|a| *a == id))
        .map_or(0, |i| i as i64 + 1)
}

const SECTIONS: [DamageSection; 6] = [
    DamageSection::Nose,
    DamageSection::Cockpit,
    DamageSection::Core,
    DamageSection::LeftWing,
    DamageSection::RightWing,
    DamageSection::Tail,
];

fn coarse(p: Vector) -> [i64; 3] {
    p.map(|x| q(x, COARSE_FT))
}

fn contact_values(c: &Contact) -> Vec<i64> {
    let [x, y, z] = coarse(c.position);
    let [vx, vy, vz] = c.velocity.map(|x| q(x, COARSE_FPS));
    vec![
        channel_code(c.channel),
        x,
        y,
        z,
        vx,
        vy,
        vz,
        flag(c.track_eligible),
        flag(c.destroyed),
    ]
}

fn row_values(row: &TargetRow) -> Vec<i64> {
    let mut out = vec![aircraft_code(row.aircraft)];
    out.extend(row.position.map(|x| q(x, FINE_FT)));
    out.extend(row.velocity.map(|x| q(x, FINE_FPS)));
    out.push(i64::from(row.damage.hp));
    out.push(i64::from(row.damage.initial_hp));
    out.extend(row.damage.sections.iter().map(|s| i64::from(*s)));
    out.push(row.damage.structural.map_or(0, |s| s as i64 + 1));
    out
}

fn observation_values(o: &seeker::Observation) -> Vec<i64> {
    let mut out: Vec<i64> = o.position.map(|x| q(x, FINE_FT)).to_vec();
    out.extend(o.velocity.map(|x| q(x, FINE_FPS)));
    out.push(q(o.quality, FRACTION));
    out.push(q(o.off_axis, SMALL_ANGLE));
    out.push(q(o.range, COARSE_FT));
    out
}

fn option_pair(value: Option<i64>) -> [i64; 2] {
    [flag(value.is_some()), value.unwrap_or(0)]
}

impl QReadout {
    /// The readout before any is received: nothing in any group.
    pub fn empty() -> Self {
        Self::default()
    }

    /// `readout`, which the host built for the snapshot of `tick`, in the
    /// wire's numbers.
    pub fn of(readout: &CockpitReadout, tick: u32) -> Self {
        let mut out = Self::default();
        let s = &mut out.scalars;
        s[scalar::HEADER] = vec![
            i64::from(readout.plane),
            readout.tick as i64 - i64::from(tick),
        ];
        let stores = &readout.stores;
        s[scalar::STORES] = [
            i64::from(stores.selected),
            flag(stores.armed),
            flag(stores.launch_mode == LaunchMode::Boresight),
            i64::from(stores.loaded),
        ]
        .into_iter()
        .chain(stores.ammo.iter().map(|a| i64::from(*a)))
        .collect();
        s[scalar::COUNTERMEASURES] = vec![
            i64::from(readout.countermeasures.chaff),
            i64::from(readout.countermeasures.flares),
        ];
        let d = &readout.damage;
        s[scalar::DAMAGE] = [
            i64::from(d.hp),
            i64::from(d.damage),
            flag(d.last_subsystem.is_some()),
            i64::from(d.last_subsystem.unwrap_or(0)),
            flag(d.radar_failed),
            flag(d.visual_failed),
            flag(d.infrared_failed),
            flag(d.rwr_failed),
            flag(d.ecm_failed),
            i64::from(d.shots),
            i64::from(d.hits),
            i64::from(d.kills),
        ]
        .into_iter()
        .chain(d.subsystem_counts.iter().map(|c| i64::from(*c)))
        .collect();
        let sk = &readout.seeker;
        let tone = sk.tone.as_ref();
        s[scalar::SEEKER] = [status_code(sk.status)]
            .into_iter()
            .chain(option_pair(sk.target.map(i64::from)))
            .chain([
                flag(tone.is_some()),
                tone.map_or(0, |t| q(t.strength, FRACTION)),
                flag(tone.is_some_and(|t| t.ground)),
                flag(tone.is_some_and(|t| t.radar)),
                flag(tone.is_some_and(|t| t.locked)),
            ])
            .collect();
        let e = &readout.estimates;
        s[scalar::ESTIMATES] = [
            readiness_code(e.readiness),
            flag(e.guidance_available),
            flag(e.can_lock),
        ]
        .into_iter()
        .chain(option_pair(e.max_range.map(|r| q(r, COARSE_FT))))
        .chain([
            flag(e.band.is_some()),
            e.band.map_or(0, |b| q(b.minimum, COARSE_FT)),
            e.band.map_or(0, |b| q(b.maximum, COARSE_FT)),
            flag(e.in_range),
            i64::from(e.hit_percent),
        ])
        .chain(option_pair(e.solution_seconds.map(|t| q(t, SECONDS))))
        .collect();
        let t = &readout.targets;
        let view_mode = match (&t.view, &t.display) {
            (None, _) => 0,
            (Some(view), Some(display)) if view == display => 1,
            (Some(_), _) => 2,
        };
        s[scalar::TARGETS] = option_pair(t.designated.map(i64::from))
            .into_iter()
            .chain([view_mode])
            .collect();
        let airport = &readout.airport;
        let service = airport.service.as_ref();
        let clearance = service.and_then(Service::clearance);
        s[scalar::AIRPORT] = [flag(airport.nav_mode), flag(service.is_some())]
            .into_iter()
            .chain(option_pair(
                service.and_then(Service::selected).map(i64::from),
            ))
            .chain([
                flag(clearance.is_some()),
                clearance.map_or(0, |c| i64::from(c.0)),
                clearance.map_or(0, |c| i64::from(c.1)),
                flag(clearance.is_some_and(|c| c.2 == ApproachEnd::Far)),
            ])
            .chain(
                service
                    .into_iter()
                    .flat_map(|s| s.out_of_action())
                    .map(i64::from),
            )
            .collect();
        s[scalar::WINDOW] = match &readout.target_window {
            None => vec![0],
            Some(brief) => {
                let objective = match brief.objective {
                    None => 0,
                    Some(TargetObjective::Survive) => 1,
                    Some(TargetObjective::Destroy) => 2,
                };
                let pilot = match brief.pilot {
                    Pilot::None => [0, 0, 0, 0],
                    Pilot::Dummy => [1, 0, 0, 0],
                    Pilot::Ai {
                        activity,
                        skill,
                        aims_at_viewer,
                    } => [
                        2,
                        activity_code(activity),
                        i64::from(skill),
                        flag(aims_at_viewer),
                    ],
                };
                [1, i64::from(brief.id), objective]
                    .into_iter()
                    .chain(pilot)
                    .collect()
            }
        };
        let music = &readout.music;
        s[scalar::MUSIC] = [flag(music.succeeded), flag(music.home)]
            .into_iter()
            .chain(music.aiming.iter().map(|id| i64::from(*id)))
            .collect();
        s[scalar::LOCKS] = readout.rwr.locks.iter().map(|l| i64::from(*l)).collect();
        let se = &readout.sensors;
        s[scalar::SENSORS] = [se.tick as i64 - readout.tick as i64]
            .into_iter()
            .chain(option_pair(se.selected.map(i64::from)))
            .chain(option_pair(se.acquired.map(i64::from)))
            .chain(option_pair(se.selected_support.map(support_code)))
            .chain(se.available.map(flag))
            .chain(se.operating.map(flag))
            .chain(option_pair(se.radar_track_nmi.map(|n| q(n, NMI))))
            .collect();

        let l = &mut out.lists;
        let mut one = |index: usize, id: u32, values: Vec<i64>| {
            l[index].insert(id, values);
        };
        if let Some(o) = &sk.observation {
            one(list::SEEKER_OBSERVATION, o.id, observation_values(o));
        }
        if let Some(o) = &e.observation {
            one(list::ESTIMATE_OBSERVATION, o.id, observation_values(o));
        }
        if let Some(row) = &t.display {
            one(list::DISPLAY, row.id, row_values(row));
        }
        if let (2, Some(row)) = (view_mode, &t.view) {
            one(list::VIEW, row.id, row_values(row));
        }
        if let Some((id, position)) = music.designated_enemy {
            one(list::ENEMY, id, coarse(position).to_vec());
        }
        for m in &readout.rwr.inbound {
            let mut values = coarse(m.position).to_vec();
            values.extend([i64::from(m.seeker_class), flag(m.aim120)]);
            l[list::INBOUND].insert(m.id, values);
        }
        for r in &readout.rwr.missiles {
            let [x, y, z] = r.position.map_or([0; 3], coarse);
            let [vx, vy, vz] = r.velocity.map_or([0; 3], |v| v.map(|x| q(x, COARSE_FPS)));
            let source = match r.source {
                EvidenceSource::ElectronicSupported => 0,
                EvidenceSource::ElectronicActive => 1,
                EvidenceSource::OwnLaunch => 2,
                EvidenceSource::Visual => 3,
            };
            let guidance = match r.guidance_class {
                None => 0,
                Some(GuidanceClass::Radar) => 1,
                Some(GuidanceClass::Infrared) => 2,
                Some(GuidanceClass::Passive) => 3,
            };
            l[list::THREATS].insert(
                r.missile_id,
                vec![
                    source,
                    r.observed_tick as i64 - readout.tick as i64,
                    q(r.bearing_deg, DEGREES),
                    flag(r.position.is_some()),
                    x,
                    y,
                    z,
                    flag(r.velocity.is_some()),
                    vx,
                    vy,
                    vz,
                    guidance,
                    flag(r.targeting_receiver),
                    flag(r.was_targeting_receiver),
                    flag(r.stale),
                    flag(r.radar_bearing_deg.is_some()),
                    r.radar_bearing_deg.map_or(0, |b| q(b, DEGREES)),
                ],
            );
        }
        for em in &readout.rwr.emitters {
            let symbol = match em.symbol {
                Symbol::Unknown => 0,
                Symbol::Aircraft => 1,
                Symbol::Ground => 2,
            };
            l[list::EMITTERS].insert(
                em.id,
                vec![
                    q(em.bearing_rad, DIAL),
                    flag(em.distance_nmi.is_some()),
                    em.distance_nmi.map_or(0, |d| q(d, EMITTER_NMI)),
                    symbol,
                    q(em.received, EMPHASIS),
                ],
            );
        }
        for c in &se.contacts {
            l[list::CONTACTS].insert(c.id, contact_values(c));
        }
        for st in &se.strobes {
            l[list::STROBES].insert(
                st.id,
                vec![
                    q(st.bearing_rad, ANGLE),
                    q(st.elevation_rad, ANGLE),
                    q(st.received, FRACTION),
                    q(st.half_width_rad, SMALL_ANGLE),
                    q(st.sidelobe_floor, FRACTION),
                ],
            );
        }
        for p in &se.plots {
            let [x, y, z] = coarse(p.position);
            l[list::PLOTS].insert(
                p.id,
                vec![
                    channel_code(p.channel),
                    q(p.bearing_rad, ANGLE),
                    q(p.elevation_rad, ANGLE),
                    q(p.distance_ft, COARSE_FT),
                    x,
                    y,
                    z,
                    i64::from(p.age),
                ],
            );
        }
        for c in &readout.visual {
            l[list::VISUAL].insert(c.id, contact_values(c));
        }
        for row in &readout.map {
            let mut values = contact_values(&row.contact);
            values.extend([
                flag(row.identified),
                flag(row.airborne),
                aircraft_code(row.aircraft),
            ]);
            l[list::MAP].insert(row.contact.id, values);
        }
        for trail in &se.trails {
            out.trails.insert(
                trail.id,
                se.trail(trail.id).iter().map(|p| coarse(*p)).collect(),
            );
        }
        out
    }

    /// Every group has arrived: none still waits for room. (The AI's locks
    /// and the inbound lists may be empty either way.)
    pub fn complete(&self) -> bool {
        self.scalars
            .iter()
            .enumerate()
            .all(|(index, group)| index == scalar::LOCKS || !group.is_empty())
    }

    /// The plane this readout is for, once its header has arrived.
    pub fn plane(&self) -> Option<u32> {
        self.scalars[scalar::HEADER].first().map(|p| *p as u32)
    }

    /// The readout these numbers stand for, at the snapshot of `tick`. A
    /// contact's bearing, elevation and distance are worked out from
    /// `observer`, the client's own plane (position and attitude), or left at
    /// zero without one; the tower's service is rebuilt on `scene`, or left
    /// out without one. A group that has not arrived yet reads as empty.
    pub fn readout(
        &self,
        tick: u32,
        observer: Option<(Vector, &Basis)>,
        scene: Option<&Scene>,
    ) -> WireResult<CockpitReadout> {
        let bad = |what: &'static str| WireError::Invalid(what);
        let get = |group: usize| -> &[i64] { &self.scalars[group] };
        let pick = |values: &[i64], i: usize| values.get(i).copied().unwrap_or(0);
        let opt = |values: &[i64], i: usize| (pick(values, i) != 0).then(|| pick(values, i + 1));
        let u32_of = |x: i64| u32::try_from(x).map_err(|_| bad("readout id"));

        let header = get(scalar::HEADER);
        let plane = u32_of(pick(header, 0))?;
        let readout_tick = (i64::from(tick) + pick(header, 1)).max(0) as u64;

        let st = get(scalar::STORES);
        let ammo = st
            .get(4..)
            .unwrap_or(&[])
            .iter()
            .map(|a| u16::try_from(*a).map_err(|_| bad("rounds")))
            .collect::<WireResult<Vec<u16>>>()?;
        if ammo.len() > 32 {
            return Err(bad("stations"));
        }
        let stores = Stores {
            selected: u8::try_from(pick(st, 0)).map_err(|_| bad("station"))?,
            armed: pick(st, 1) != 0,
            launch_mode: if pick(st, 2) != 0 {
                LaunchMode::Boresight
            } else {
                LaunchMode::Cued
            },
            ammo,
            loaded: u32_of(pick(st, 3))?,
        };

        let cm = get(scalar::COUNTERMEASURES);
        let countermeasures = Countermeasures {
            chaff: u8::try_from(pick(cm, 0)).map_err(|_| bad("chaff"))?,
            flares: u8::try_from(pick(cm, 1)).map_err(|_| bad("flares"))?,
        };

        let dm = get(scalar::DAMAGE);
        let i32_of = |x: i64| i32::try_from(x).map_err(|_| bad("readout number"));
        let mut subsystem_counts = [0u8; 45];
        for (i, count) in subsystem_counts.iter_mut().enumerate() {
            *count = u8::try_from(pick(dm, 12 + i)).map_err(|_| bad("subsystem count"))?;
        }
        let damage = DamageReadout {
            hp: i32_of(pick(dm, 0))?,
            damage: i32_of(pick(dm, 1))?,
            subsystem_counts,
            last_subsystem: opt(dm, 2)
                .map(|x| u8::try_from(x).map_err(|_| bad("subsystem")))
                .transpose()?,
            radar_failed: pick(dm, 4) != 0,
            visual_failed: pick(dm, 5) != 0,
            infrared_failed: pick(dm, 6) != 0,
            rwr_failed: pick(dm, 7) != 0,
            ecm_failed: pick(dm, 8) != 0,
            shots: u32_of(pick(dm, 9))?,
            hits: u32_of(pick(dm, 10))?,
            kills: u32_of(pick(dm, 11))?,
        };

        let observation = |list: usize| -> WireResult<Option<seeker::Observation>> {
            self.lists[list]
                .iter()
                .next()
                .map(|(id, x)| {
                    Ok(seeker::Observation {
                        id: *id,
                        position: [v(x[0], FINE_FT), v(x[1], FINE_FT), v(x[2], FINE_FT)],
                        velocity: [v(x[3], FINE_FPS), v(x[4], FINE_FPS), v(x[5], FINE_FPS)],
                        quality: v(x[6], FRACTION),
                        off_axis: v(x[7], SMALL_ANGLE),
                        range: v(x[8], COARSE_FT),
                    })
                })
                .transpose()
        };
        let sk = get(scalar::SEEKER);
        let seeker = SeekerReadout {
            status: *STATUSES
                .get(pick(sk, 0) as usize)
                .ok_or(bad("seeker status"))?,
            target: opt(sk, 1).map(u32_of).transpose()?,
            observation: observation(list::SEEKER_OBSERVATION)?,
            tone: (pick(sk, 3) != 0).then(|| SeekerTone {
                strength: v(pick(sk, 4), FRACTION),
                ground: pick(sk, 5) != 0,
                radar: pick(sk, 6) != 0,
                locked: pick(sk, 7) != 0,
            }),
        };

        let es = get(scalar::ESTIMATES);
        let estimates = Estimates {
            readiness: *READINESS
                .get(pick(es, 0) as usize)
                .ok_or(bad("readiness"))?,
            guidance_available: pick(es, 1) != 0,
            can_lock: pick(es, 2) != 0,
            observation: observation(list::ESTIMATE_OBSERVATION)?,
            max_range: opt(es, 3).map(|r| v(r, COARSE_FT)),
            band: (pick(es, 5) != 0).then(|| FiringBand {
                minimum: v(pick(es, 6), COARSE_FT),
                maximum: v(pick(es, 7), COARSE_FT),
            }),
            in_range: pick(es, 8) != 0,
            hit_percent: u8::try_from(pick(es, 9)).map_err(|_| bad("hit percentage"))?,
            solution_seconds: opt(es, 10).map(|t| v(t, SECONDS)),
        };

        let row = |list: usize| -> WireResult<Option<TargetRow>> {
            self.lists[list]
                .iter()
                .next()
                .map(|(id, x)| {
                    Ok(TargetRow {
                        id: *id,
                        aircraft: aircraft_of(x[0]),
                        position: [v(x[1], FINE_FT), v(x[2], FINE_FT), v(x[3], FINE_FT)],
                        velocity: [v(x[4], FINE_FPS), v(x[5], FINE_FPS), v(x[6], FINE_FPS)],
                        damage: Damage {
                            hp: i32_of(x[7])?,
                            initial_hp: i32_of(x[8])?,
                            sections: [
                                i32_of(x[9])?,
                                i32_of(x[10])?,
                                i32_of(x[11])?,
                                i32_of(x[12])?,
                                i32_of(x[13])?,
                                i32_of(x[14])?,
                            ],
                            structural: (x[15] > 0).then(|| SECTIONS[x[15] as usize - 1]),
                        },
                    })
                })
                .transpose()
        };
        let tg = get(scalar::TARGETS);
        let display = row(list::DISPLAY)?;
        let view = match pick(tg, 2) {
            0 => None,
            1 => display.clone(),
            _ => row(list::VIEW)?,
        };
        let targets = Targets {
            designated: opt(tg, 0).map(u32_of).transpose()?,
            display,
            view,
        };

        let relative = |position: Vector| match observer {
            Some((at, basis)) => {
                let s = Sighting::new(at, basis, position);
                (s.azimuth_rad, s.elevation_rad, s.distance_ft)
            }
            None => (0., 0., 0.),
        };
        let contact = |id: u32, x: &[i64]| -> WireResult<Contact> {
            let position = [v(x[1], COARSE_FT), v(x[2], COARSE_FT), v(x[3], COARSE_FT)];
            let (bearing_rad, elevation_rad, distance_ft) = relative(position);
            Ok(Contact {
                id,
                channel: *CHANNELS.get(x[0] as usize).ok_or(bad("channel"))?,
                bearing_rad,
                elevation_rad,
                distance_ft,
                position,
                velocity: [
                    v(x[4], COARSE_FPS),
                    v(x[5], COARSE_FPS),
                    v(x[6], COARSE_FPS),
                ],
                track_eligible: x[7] != 0,
                destroyed: x[8] != 0,
            })
        };
        let contacts = self.lists[list::CONTACTS]
            .iter()
            .map(|(id, x)| contact(*id, x))
            .collect::<WireResult<Vec<_>>>()?;
        let plots = self.lists[list::PLOTS]
            .iter()
            .map(|(id, x)| {
                Ok(Plot {
                    id: *id,
                    channel: *CHANNELS.get(x[0] as usize).ok_or(bad("channel"))?,
                    bearing_rad: v(x[1], ANGLE),
                    elevation_rad: v(x[2], ANGLE),
                    distance_ft: v(x[3], COARSE_FT),
                    position: [v(x[4], COARSE_FT), v(x[5], COARSE_FT), v(x[6], COARSE_FT)],
                    age: u32_of(x[7])?,
                })
            })
            .collect::<WireResult<Vec<_>>>()?;
        let strobes = self.lists[list::STROBES]
            .iter()
            .map(|(id, x)| {
                Strobe::presented(
                    *id,
                    v(x[0], ANGLE),
                    v(x[1], ANGLE),
                    v(x[2], FRACTION),
                    v(x[3], SMALL_ANGLE),
                    v(x[4], FRACTION),
                )
            })
            .collect();
        let mut trails = Vec::new();
        let mut trail_points = Vec::new();
        for (id, points) in &self.trails {
            if points.len() > TRAIL_LIMIT
                || trail_points.len() + points.len() > usize::from(u16::MAX)
            {
                return Err(bad("trail"));
            }
            trails.push(Trail {
                id: *id,
                start: trail_points.len() as u16,
                len: points.len() as u8,
            });
            trail_points.extend(points.iter().map(|p| p.map(|x| v(x, COARSE_FT))));
        }
        let se = get(scalar::SENSORS);
        let sensors = SensorReadout {
            tick: (readout_tick as i64 + pick(se, 0)).max(0) as u64,
            selected: opt(se, 1).map(u32_of).transpose()?,
            acquired: opt(se, 3).map(u32_of).transpose()?,
            selected_support: opt(se, 5)
                .map(|c| SUPPORTS.get(c as usize).copied().ok_or(bad("track status")))
                .transpose()?,
            available: [pick(se, 7) != 0, pick(se, 8) != 0, pick(se, 9) != 0],
            operating: [pick(se, 10) != 0, pick(se, 11) != 0, pick(se, 12) != 0],
            radar_track_nmi: opt(se, 13).map(|n| v(n, NMI)),
            contacts,
            plots,
            strobes,
            trails,
            trail_points,
        };
        let visual = self.lists[list::VISUAL]
            .iter()
            .map(|(id, x)| contact(*id, x))
            .collect::<WireResult<Vec<_>>>()?;
        let map = self.lists[list::MAP]
            .iter()
            .map(|(id, x)| {
                Ok(MapRow {
                    contact: contact(*id, x)?,
                    identified: x[9] != 0,
                    airborne: x[10] != 0,
                    aircraft: aircraft_of(x[11]),
                })
            })
            .collect::<WireResult<Vec<_>>>()?;

        let emitters = self.lists[list::EMITTERS]
            .iter()
            .map(|(id, x)| Emitter {
                id: *id,
                bearing_rad: v(x[0], DIAL),
                distance_nmi: (x[1] != 0).then(|| v(x[2], EMITTER_NMI)),
                symbol: [Symbol::Unknown, Symbol::Aircraft, Symbol::Ground][x[3] as usize],
                received: v(x[4], EMPHASIS),
            })
            .collect();
        let missiles = self.lists[list::THREATS]
            .iter()
            .map(|(id, x)| ThreatRecord {
                missile_id: *id,
                source: [
                    EvidenceSource::ElectronicSupported,
                    EvidenceSource::ElectronicActive,
                    EvidenceSource::OwnLaunch,
                    EvidenceSource::Visual,
                ][x[0] as usize],
                observed_tick: (readout_tick as i64 + x[1]).max(0) as u64,
                bearing_deg: v(x[2], DEGREES),
                position: (x[3] != 0)
                    .then(|| [v(x[4], COARSE_FT), v(x[5], COARSE_FT), v(x[6], COARSE_FT)]),
                velocity: (x[7] != 0).then(|| {
                    [
                        v(x[8], COARSE_FPS),
                        v(x[9], COARSE_FPS),
                        v(x[10], COARSE_FPS),
                    ]
                }),
                guidance_class: [
                    None,
                    Some(GuidanceClass::Radar),
                    Some(GuidanceClass::Infrared),
                    Some(GuidanceClass::Passive),
                ][x[11] as usize],
                targeting_receiver: x[12] != 0,
                was_targeting_receiver: x[13] != 0,
                stale: x[14] != 0,
                radar_bearing_deg: (x[15] != 0).then(|| v(x[16], DEGREES)),
            })
            .collect();
        let inbound = self.lists[list::INBOUND]
            .iter()
            .map(|(id, x)| InboundMissile {
                id: *id,
                position: [v(x[0], COARSE_FT), v(x[1], COARSE_FT), v(x[2], COARSE_FT)],
                seeker_class: x[3] as u8,
                aim120: x[4] != 0,
            })
            .collect();
        let locks = get(scalar::LOCKS)
            .iter()
            .map(|l| u8::try_from(*l).map_err(|_| bad("lock")))
            .collect::<WireResult<Vec<u8>>>()?;

        let ap = get(scalar::AIRPORT);
        let service = match scene {
            Some(scene) if pick(ap, 1) != 0 => {
                let clearance = (pick(ap, 4) != 0)
                    .then(|| -> WireResult<_> {
                        Ok((
                            u32_of(pick(ap, 5))?,
                            u32_of(pick(ap, 6))?,
                            if pick(ap, 7) != 0 {
                                ApproachEnd::Far
                            } else {
                                ApproachEnd::Near
                            },
                        ))
                    })
                    .transpose()?;
                let out = ap
                    .get(8..)
                    .unwrap_or(&[])
                    .iter()
                    .map(|id| u32_of(*id))
                    .collect::<WireResult<Vec<u32>>>()?;
                Some(
                    Service::presented(scene, opt(ap, 2).map(u32_of).transpose()?, clearance, out)
                        .map_err(|_| bad("airport scene"))?,
                )
            }
            _ => None,
        };
        let airport = AirportReadout {
            nav_mode: pick(ap, 0) != 0,
            service,
        };

        let wd = get(scalar::WINDOW);
        let target_window = if pick(wd, 0) == 0 {
            None
        } else {
            Some(TargetBrief {
                id: u32_of(pick(wd, 1))?,
                objective: match pick(wd, 2) {
                    0 => None,
                    1 => Some(TargetObjective::Survive),
                    2 => Some(TargetObjective::Destroy),
                    _ => return Err(bad("objective")),
                },
                pilot: match pick(wd, 3) {
                    0 => Pilot::None,
                    1 => Pilot::Dummy,
                    2 => Pilot::Ai {
                        activity: *ACTIVITIES
                            .get(pick(wd, 4) as usize)
                            .ok_or(bad("activity"))?,
                        skill: u8::try_from(pick(wd, 5)).map_err(|_| bad("skill"))?,
                        aims_at_viewer: pick(wd, 6) != 0,
                    },
                    _ => return Err(bad("pilot")),
                },
            })
        };

        let mu = get(scalar::MUSIC);
        let music = MusicReadout {
            designated_enemy: self.lists[list::ENEMY].iter().next().map(|(id, x)| {
                (
                    *id,
                    [v(x[0], COARSE_FT), v(x[1], COARSE_FT), v(x[2], COARSE_FT)],
                )
            }),
            aiming: mu
                .get(2..)
                .unwrap_or(&[])
                .iter()
                .map(|id| u32_of(*id))
                .collect::<WireResult<_>>()?,
            succeeded: pick(mu, 0) != 0,
            home: pick(mu, 1) != 0,
        };

        Ok(CockpitReadout {
            plane,
            tick: readout_tick,
            stores,
            seeker,
            estimates,
            targets,
            sensors,
            visual,
            map,
            rwr: RwrReadout {
                emitters,
                missiles,
                inbound,
                locks,
            },
            damage,
            countermeasures,
            airport,
            target_window,
            music,
        })
    }

    /// This readout's lists moved on by `ticks`, as the client holds a
    /// baseline when it codes against it.
    fn advanced(&self, ticks: u32) -> Self {
        let mut out = self.clone();
        for (index, list) in out.lists.iter_mut().enumerate() {
            *list = flat::advance_list(schema(index), list, ticks);
        }
        out
    }
}

fn aircraft_of(code: i64) -> Option<AircraftId> {
    (code > 0)
        .then(|| AircraftId::SELECTABLE.get(code as usize - 1).copied())
        .flatten()
}

/// Trail point differences.
const TRAIL_LADDER: [u32; 5] = [3, 6, 10, 14, 20];

fn write_points(w: &mut BitWriter, from: Option<[i64; 3]>, points: &[[i64; 3]]) {
    let mut previous = from;
    for point in points {
        match previous {
            None => {
                for x in point {
                    w.write_varint_signed(*x);
                }
            }
            Some(p) => {
                for i in 0..3 {
                    let _ = w.write_bucketed(point[i] - p[i], &TRAIL_LADDER);
                }
            }
        }
        previous = Some(*point);
    }
}

/// A trail record as read: the points dropped from the front of the
/// baseline's (`None` for a trail in full) and the points added.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TrailRaw {
    dropped: Option<u32>,
    added: Vec<[i64; 3]>,
}

/// The trails part: removed ids, then each changed trail as the points it
/// dropped from the front of the baseline's and the points it added (or in
/// full), points as differences from the one before. Returns the bits and
/// what the client holds afterwards; a trail that does not fit waits.
fn write_trails(
    base: &BTreeMap<u32, Vec<[i64; 3]>>,
    now: &BTreeMap<u32, Vec<[i64; 3]>>,
    budget: usize,
) -> (Option<BitWriter>, BTreeMap<u32, Vec<[i64; 3]>>) {
    let removed: Vec<u32> = base
        .keys()
        .filter(|id| !now.contains_key(id))
        .copied()
        .collect();
    let mut changes: Vec<(u32, BitWriter)> = Vec::new();
    for (id, points) in now {
        let old = base.get(id);
        if old == Some(points) {
            continue;
        }
        let mut w = BitWriter::new();
        let shift = old.and_then(|old| {
            (0..=old.len()).find(|k| {
                let kept = &old[*k..];
                kept.len() <= points.len() && points[..kept.len()] == *kept
            })
        });
        match (old, shift) {
            (Some(old), Some(k)) if old.len() > k => {
                w.write_bool(false);
                w.write_varint(k as u64);
                let added = &points[old.len() - k..];
                bits::write_count(&mut w, added.len());
                write_points(&mut w, old.last().copied(), added);
            }
            _ => {
                w.write_bool(true);
                bits::write_count(&mut w, points.len());
                write_points(&mut w, None, points);
            }
        }
        changes.push((*id, w));
    }
    if removed.is_empty() && changes.is_empty() {
        return (None, base.clone());
    }
    let mut left = budget.saturating_sub(48 + 40 * removed.len());
    let mut after = base.clone();
    for id in &removed {
        after.remove(id);
    }
    let mut chosen = Vec::new();
    for (id, w) in changes {
        let cost = 40 + w.bit_len();
        if cost <= left {
            left -= cost;
            after.insert(id, now[&id].clone());
            chosen.push((id, w));
        }
    }
    let mut w = BitWriter::new();
    bits::write_count(&mut w, removed.len());
    for id in &removed {
        w.write_varint(u64::from(*id));
    }
    bits::write_count(&mut w, chosen.len());
    for (id, body) in &chosen {
        w.write_varint(u64::from(*id));
        bits::append(&mut w, body);
    }
    (Some(w), after)
}

fn read_points(
    r: &mut BitReader<'_>,
    from: Option<[i64; 3]>,
    count: usize,
) -> WireResult<Vec<[i64; 3]>> {
    let mut previous = from;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let point = match previous {
            None => {
                let mut p = [0; 3];
                for x in &mut p {
                    *x = bits::in_range(i128::from(r.read_varint_signed()?), "trail point")?;
                }
                p
            }
            Some(p) => {
                let mut q = [0; 3];
                for i in 0..3 {
                    q[i] = bits::in_range(
                        i128::from(p[i]) + i128::from(r.read_bucketed(&TRAIL_LADDER)?),
                        "trail point",
                    )?;
                }
                q
            }
        };
        out.push(point);
        previous = Some(point);
    }
    Ok(out)
}

/// The trails part as read. Differences after the first point need the
/// baseline's last point for a trail that is extended, so the points are
/// kept as differences until [`apply_trails`]: here the first point of an
/// extension is read against the origin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TrailsRaw {
    removed: Vec<u32>,
    changed: Vec<(u32, TrailRaw)>,
}

fn read_trails(r: &mut BitReader<'_>) -> WireResult<TrailsRaw> {
    let mut raw = TrailsRaw::default();
    let removed = bits::read_count(r, world_readout::MAX_CONTACTS, "trails removed")?;
    for _ in 0..removed {
        raw.removed.push(read_u32(r)?);
    }
    let count = bits::read_count(r, world_readout::MAX_CONTACTS, "trails")?;
    for _ in 0..count {
        let id = read_u32(r)?;
        let full = r.read_bool()?;
        let (dropped, n) = if full {
            (None, bits::read_count(r, TRAIL_LIMIT, "trail points")?)
        } else {
            let dropped = read_u32(r)?;
            (
                Some(dropped),
                bits::read_count(r, TRAIL_LIMIT, "trail points")?,
            )
        };
        // An extension's first point is a difference from the baseline's
        // last: read it against the origin and move it in `apply_trails`.
        let from = dropped.map(|_| [0; 3]);
        raw.changed.push((
            id,
            TrailRaw {
                dropped,
                added: read_points(r, from, n)?,
            },
        ));
    }
    Ok(raw)
}

fn apply_trails(
    raw: &TrailsRaw,
    base: &BTreeMap<u32, Vec<[i64; 3]>>,
) -> WireResult<BTreeMap<u32, Vec<[i64; 3]>>> {
    let mut out = base.clone();
    for id in &raw.removed {
        out.remove(id);
    }
    for (id, trail) in &raw.changed {
        let points = match trail.dropped {
            None => trail.added.clone(),
            Some(k) => {
                let old = base
                    .get(id)
                    .ok_or(WireError::Invalid("trail without a baseline"))?;
                let k = k as usize;
                if k >= old.len() {
                    return Err(WireError::Invalid("trail shift"));
                }
                let last = old[old.len() - 1];
                let mut points = old[k..].to_vec();
                points.extend(trail.added.iter().map(|p| {
                    [
                        (last[0] + p[0]).clamp(-bits::STEP_LIMIT, bits::STEP_LIMIT),
                        (last[1] + p[1]).clamp(-bits::STEP_LIMIT, bits::STEP_LIMIT),
                        (last[2] + p[2]).clamp(-bits::STEP_LIMIT, bits::STEP_LIMIT),
                    ]
                }));
                points
            }
        };
        if points.len() > TRAIL_LIMIT {
            return Err(WireError::Invalid("trail"));
        }
        out.insert(*id, points);
    }
    if out.len() > world_readout::MAX_CONTACTS {
        return Err(WireError::TooMany {
            what: "trails",
            limit: world_readout::MAX_CONTACTS,
        });
    }
    Ok(out)
}

/// One part of a readout record as read.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PartRaw {
    Unchanged,
    Scalar(Vec<i64>),
    List(ListRaw),
    Trails(TrailsRaw),
}

/// A readout record as read, before its baseline is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadoutRaw {
    /// Snapshots back to the baseline; 0 is against the empty readout.
    pub back: u8,
    parts: Vec<PartRaw>,
}

/// Reads a readout record (after its presence bit).
pub(crate) fn read_record(r: &mut BitReader<'_>) -> WireResult<ReadoutRaw> {
    let back = r.read_bits(5)? as u8;
    let mut parts = Vec::with_capacity(PARTS.len());
    for part in PARTS {
        if !r.read_bool()? {
            parts.push(PartRaw::Unchanged);
            continue;
        }
        parts.push(match part {
            Part::Scalar(_) => {
                let count = bits::read_count(r, SCALAR_LIMIT, "readout values")?;
                let mut values = Vec::with_capacity(count.min(64));
                for _ in 0..count {
                    values.push(bits::in_range(
                        i128::from(r.read_varint_signed()?),
                        "readout value",
                    )?);
                }
                PartRaw::Scalar(values)
            }
            Part::List(index) => PartRaw::List(flat::read_list(r, schema(index), limit(index))?),
            Part::Trails => PartRaw::Trails(read_trails(r)?),
        });
    }
    Ok(ReadoutRaw { back, parts })
}

impl ReadoutRaw {
    /// The readout this record describes against `base`, the readout it
    /// names, `ticks` earlier (or the empty readout).
    pub fn apply(&self, base: &QReadout, ticks: u32) -> WireResult<QReadout> {
        let base = base.advanced(ticks);
        let mut out = base.clone();
        for (part, raw) in PARTS.iter().zip(&self.parts) {
            match (part, raw) {
                (_, PartRaw::Unchanged) => {}
                (Part::Scalar(index), PartRaw::Scalar(values)) => {
                    out.scalars[*index] = values.clone()
                }
                (Part::List(index), PartRaw::List(list)) => {
                    out.lists[*index] =
                        flat::apply_list(schema(*index), list, &base.lists[*index], limit(*index))?;
                }
                (Part::Trails, PartRaw::Trails(trails)) => {
                    out.trails = apply_trails(trails, &base.trails)?
                }
                _ => return Err(WireError::Invalid("readout part")),
            }
        }
        Ok(out)
    }
}

/// What one readout record held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadoutReport {
    /// Bits of the record.
    pub bits: usize,
    /// Parts sent and parts that changed but waited for room.
    pub sent: usize,
    pub waiting: usize,
    /// Bits each part took, in [`PARTS`] order.
    pub part_bits: [usize; 26],
}

/// Writes the record of `now` against `base` (the readout it names, not yet
/// advanced) `ticks` earlier, within `budget` bits. Returns the bits, what
/// the client holds afterwards and what went in.
pub(crate) fn write_record(
    back: u8,
    now: &QReadout,
    base: &QReadout,
    ticks: u32,
    budget: usize,
) -> (BitWriter, QReadout, ReadoutReport) {
    let base = base.advanced(ticks);
    let mut after = base.clone();
    let mut w = BitWriter::new();
    let _ = w.write_bits(u64::from(back), 5);
    let mut report = ReadoutReport::default();
    // A bit for every part is spent whatever happens.
    let mut left = budget.saturating_sub(5 + PARTS.len());
    for (index, part) in PARTS.iter().enumerate() {
        let start = w.bit_len();
        match *part {
            Part::Scalar(group) => {
                let values = &now.scalars[group];
                if *values == base.scalars[group] {
                    w.write_bool(false);
                } else {
                    let mut body = BitWriter::new();
                    bits::write_count(&mut body, values.len());
                    for value in values {
                        body.write_varint_signed(*value);
                    }
                    // The header always goes, so the client knows the plane.
                    if body.bit_len() <= left || group == scalar::HEADER {
                        left = left.saturating_sub(body.bit_len());
                        w.write_bool(true);
                        bits::append(&mut w, &body);
                        after.scalars[group] = values.clone();
                        report.sent += 1;
                    } else {
                        w.write_bool(false);
                        report.waiting += 1;
                    }
                }
            }
            Part::List(list) => {
                let written =
                    flat::write_list(schema(list), &base.lists[list], &now.lists[list], left);
                report.waiting += written.waiting;
                match written.bits {
                    Some(body) => {
                        left = left.saturating_sub(body.bit_len());
                        w.write_bool(true);
                        bits::append(&mut w, &body);
                        after.lists[list] = written.after;
                        report.sent += 1;
                    }
                    None => w.write_bool(false),
                }
            }
            Part::Trails => {
                let (body, trails) = write_trails(&base.trails, &now.trails, left);
                match body {
                    Some(body) => {
                        left = left.saturating_sub(body.bit_len());
                        w.write_bool(true);
                        bits::append(&mut w, &body);
                        after.trails = trails;
                        report.sent += 1;
                    }
                    None => w.write_bool(false),
                }
            }
        }
        report.part_bits[index] = w.bit_len() - start;
    }
    report.bits = w.bit_len();
    (w, after, report)
}

/// The host's readout bookkeeping for one connection: what the client holds
/// as of each packet, and the newest acknowledged.
#[derive(Clone, Debug)]
pub struct ReadoutSender {
    ticks_per_snapshot: u32,
    acked: Option<(u32, QReadout)>,
    packets: VecDeque<(Option<u16>, u32, QReadout)>,
}

impl ReadoutSender {
    /// Bookkeeping for a host that sends a snapshot every
    /// `ticks_per_snapshot` ticks.
    pub fn new(ticks_per_snapshot: u32) -> Self {
        Self {
            ticks_per_snapshot: ticks_per_snapshot.clamp(1, 120),
            acked: None,
            packets: VecDeque::new(),
        }
    }

    /// The record of `readout` for the snapshot of `tick`, within `budget`
    /// bits, against the newest acknowledged readout if it is 1 to 31 whole
    /// snapshots old, else against the empty one. Staged like the entities.
    pub fn build(
        &mut self,
        tick: u32,
        readout: &QReadout,
        budget: usize,
    ) -> (BitWriter, ReadoutReport) {
        self.discard();
        let empty = QReadout::empty();
        let (back, base, ticks) = self
            .acked
            .as_ref()
            .and_then(|(base_tick, base)| {
                let ticks = tick.checked_sub(*base_tick)?;
                let back = ticks / self.ticks_per_snapshot;
                (ticks > 0 && ticks % self.ticks_per_snapshot == 0 && back <= 31)
                    .then_some((back as u8, base, ticks))
            })
            .unwrap_or((0, &empty, 0));
        let (w, after, report) = write_record(back, readout, base, ticks, budget);
        self.packets.push_back((None, tick, after));
        (w, report)
    }

    /// What the client holds once the packet numbered `sequence` arrives.
    #[cfg(test)]
    pub(crate) fn staged_for_tests(&self, sequence: u16) -> &QReadout {
        &self
            .packets
            .iter()
            .find(|p| p.0 == Some(sequence))
            .expect("a packet sent")
            .2
    }

    /// The staged record went out numbered `sequence`.
    pub fn sent(&mut self, sequence: u16) {
        if let Some(packet) = self.packets.back_mut()
            && packet.0.is_none()
        {
            packet.0 = Some(sequence);
        }
        while self.packets.len() > 64 {
            self.packets.pop_front();
        }
    }

    /// The staged record was not sent.
    pub fn discard(&mut self) {
        if self.packets.back().is_some_and(|p| p.0.is_none()) {
            self.packets.pop_back();
        }
    }

    /// The packet numbered `sequence` was delivered.
    pub fn delivered(&mut self, sequence: u16) {
        let Some(index) = self.packets.iter().position(|p| p.0 == Some(sequence)) else {
            return;
        };
        if let Some((_, tick, readout)) = self.packets.remove(index)
            && self.acked.as_ref().is_none_or(|(t, _)| *t < tick)
        {
            self.acked = Some((tick, readout));
        }
    }

    /// The packet numbered `sequence` was lost.
    pub fn lost(&mut self, sequence: u16) {
        self.packets.retain(|p| p.0 != Some(sequence));
    }
}

/// The client's readouts by snapshot tick, kept as baselines.
#[derive(Clone, Debug)]
pub struct ReadoutReceiver {
    ticks_per_snapshot: u32,
    readouts: VecDeque<(u32, QReadout)>,
    newest: Option<u32>,
}

impl ReadoutReceiver {
    /// None received yet.
    pub fn new(ticks_per_snapshot: u32) -> Self {
        Self {
            ticks_per_snapshot: ticks_per_snapshot.clamp(1, 120),
            readouts: VecDeque::new(),
            newest: None,
        }
    }

    /// Takes the record of the snapshot of `tick`. An error when its
    /// baseline is not held (it never is when the host codes only against
    /// what the client acknowledged).
    pub fn receive(&mut self, raw: &ReadoutRaw, tick: u32) -> WireResult<QReadout> {
        let ticks = u32::from(raw.back) * self.ticks_per_snapshot;
        let readout = if raw.back == 0 {
            raw.apply(&QReadout::empty(), 0)?
        } else {
            let base_tick = tick
                .checked_sub(ticks)
                .ok_or(WireError::Invalid("readout baseline"))?;
            let base = self
                .readouts
                .iter()
                .find(|(t, _)| *t == base_tick)
                .map(|(_, r)| r)
                .ok_or(WireError::Invalid("readout baseline not received"))?;
            raw.apply(base, ticks)?
        };
        if !self.readouts.iter().any(|(t, _)| *t == tick) {
            self.readouts.push_back((tick, readout.clone()));
        }
        if self.newest.is_none_or(|n| tick > n) {
            self.newest = Some(tick);
            let keep = tick.saturating_sub(64 * self.ticks_per_snapshot);
            self.readouts.retain(|(t, _)| *t >= keep);
        }
        Ok(readout)
    }

    /// The newest readout received, with its snapshot tick.
    pub fn latest(&self) -> Option<(u32, &QReadout)> {
        self.readouts
            .iter()
            .max_by_key(|(t, _)| *t)
            .map(|(t, r)| (*t, r))
    }
}
