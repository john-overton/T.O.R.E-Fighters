//! Explicit development live-fire adapter. Source configuration and recovered scalar
//! kernels are combined with authored scheduling, guidance and swept-sphere contacts.
//! This is NOT the diagnostic native-parity update or a retail AI implementation.
use super::ledger::{Kill, Resolution, ShotKind};
use super::missiles::{
    self, Flight, Guidance, LaunchMode, Motion, Rules, TargetRole,
    seeker::{self, Heat, Seeker, Status},
};
use super::{
    EnginePhase, FallState, PlayerTrigger, axial_speed, commanded_speed, engine_phase,
    launch_speed, removal_due, unload,
};
use crate::attitude::{Basis, Vector, cross, dot, unit};
use crate::sensors::{self, Observable, Observer, Sensors, Support, passive};
use std::collections::BTreeMap;
use tore_formats::{
    Result,
    aircraft::{Aircraft, AircraftId},
    weapons::Weapon,
};

fn draw(state: &mut u32, bound: u16) -> u16 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state % u32::from(bound)) as u16
}

pub const MAX_PROJECTILES: usize = 256;
/// Owner id of the player's own rounds. Every other owner is an AI actor.
pub const PLAYER_OWNER: u32 = 0;
pub const MAX_EFFECTS: usize = 64;
pub const MAX_HIT_RECORDS: usize = 128;

/// Exact category switch at FA 0x411470; category is not a bitmask here.
pub fn damage_class(category: u16) -> usize {
    match category {
        0x40 | 0x200 | 0x800 | 0x1000 => 4,
        0x100 => 2,
        0x400 => 3,
        0x2000 => 1,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    Safe,
    BayClosed,
    LauncherLost,
    StationFailed,
    Empty,
    Capacity,
    NoTarget,
    TargetDestroyed,
    WrongTarget,
    NoRadar,
    RadarOff,
    RadarFailed,
    RadarCoverage,
    RadarSearchOnly,
    RadarAcquiring,
    MinimumRange,
    MaximumRange,
    Altitude,
    FieldOfView,
}
impl Readiness {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Safe => "SAFE",
            Self::BayClosed => "OPENING BAY",
            Self::LauncherLost => "LAUNCHER LOST",
            Self::StationFailed => "STATION FAILED",
            Self::Empty => "EMPTY",
            Self::Capacity => "PROJECTILE LIMIT",
            Self::NoTarget => "NO TARGET",
            Self::WrongTarget => "TARGET TYPE",
            Self::TargetDestroyed => "TARGET DESTROYED",
            Self::NoRadar => "NO RADAR",
            Self::RadarOff => "RADAR OFF",
            Self::RadarFailed => "RADAR FAILED",
            Self::RadarCoverage => "RADAR COVERAGE",
            Self::RadarSearchOnly => "RWS SEARCH ONLY",
            Self::RadarAcquiring => "ACQUIRING",
            Self::MinimumRange => "MIN RANGE",
            Self::MaximumRange => "MAX RANGE",
            Self::Altitude => "ALTITUDE LIMIT",
            Self::FieldOfView => "SEEKER FOV",
        }
    }
}

/// Authored manual-range commands. Apply at tick boundaries for deterministic replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    NextWeapon,
    NextSelection,
    PreviousSelection,
    SelectNav,
    ToggleSeekerMode,
    CompatibilityWeapons,
    TargetHeat(u8),
    TargetDistance(u32),
    ClearRange,
    ToggleTargetRadar,
    /// T: the next radar contact, nearest first.
    Designate,
    /// Shift-T: the previous radar contact.
    DesignatePrevious,
    /// Enter: the visible sensor contact nearest the nose.
    DesignateVisual,
    /// Persistent selection of one current contact by its stable identity.
    DesignateTarget(u32),
    ClearDesignation,
    ToggleArm,
    Jettison,
    ReplaceTarget,
    CycleClass,
    FailStation,
    DamagePlayer,
    Incoming,
    ToggleTargetJammer,
    /// Release one chaff cartridge against radar missiles guiding on the player.
    ReleaseChaff,
    /// Release one flare against infrared missiles guiding on the player.
    ReleaseFlare,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HitRecord {
    pub tick: u64,
    pub target: u32,
    pub station: usize,
    pub class: usize,
    pub nominal: i32,
    pub applied: i32,
    pub hp_after: i32,
}
/// One projectile that damaged the player or a target, kept for the radio
/// hit, kill and "I'm hit" calls (docs/spec/radio-chatter.md). The host drains
/// the list each tick with [`State::take_strikes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Strike {
    /// Who fired it: [`PLAYER_OWNER`] or an AI actor id.
    pub owner: u32,
    /// The damaged target, or `None` for the player.
    pub victim: Option<u32>,
    /// The weapon's type flags: 0x1 guided, 0x10 bomb, 0x80 bullet.
    pub weapon_flags: u32,
    /// The damage left the victim with no hit points.
    pub destroyed: bool,
}
/// Strikes kept between drains; older ones are dropped first.
pub const MAX_STRIKES: usize = 64;
#[derive(Clone, Debug)]
pub struct Station {
    pub weapon: Weapon,
    pub mount: Vector,
    pub count: u16,
    pub internal: bool,
}
#[derive(Clone, Debug)]
pub struct Configuration {
    pub ecm: tore_formats::weapons::Countermeasures,
    pub system_damage: [u8; 45],
    pub damage_capacity: i32,
    pub fragment_offsets: [Vector; 2],
    pub afterburner_available: bool,
    pub hardpoint_slots: Vec<Option<usize>>,
    pub radar_hardpoint: usize,
    pub visual_hardpoint: usize,
    pub infrared_hardpoint: Option<usize>,
    pub rwr_hardpoint: Option<usize>,
    pub ecm_hardpoint: usize,
    pub aircraft: AircraftId,
    pub stations: Vec<Station>,
    pub hit_points: i32,
    pub target_category: u16,
    pub external_equipment_lbs: i32,
    pub external_fuel_lbs: [f64; 9],
    pub engines: u8,
    pub wreck_power: crate::wreck::Power,
    /// Imported sensor capability, resolved by parsed record channel.
    pub sensors: sensors::SensorProfiles,
}
impl Configuration {
    fn validate(&self) -> Result<()> {
        if self.stations.is_empty()
            || self.stations.len() > 32
            || self.damage_capacity <= 0
            || !self
                .fragment_offsets
                .iter()
                .flatten()
                .all(|v| v.is_finite())
            || self.hit_points <= 0
            || self.external_equipment_lbs < 0
            || !(1..=4).contains(&self.engines)
            || self
                .external_fuel_lbs
                .iter()
                .any(|fuel| !fuel.is_finite() || *fuel < 0.)
            || self.external_fuel_lbs.iter().sum::<f64>() > f64::from(self.external_equipment_lbs)
        {
            return Err(super::invalid("invalid live configuration bounds"));
        }
        for s in &self.stations {
            if let Some(profile) = missiles::Profile::for_weapon(&s.weapon) {
                profile.validate()?;
            }
            let m = &s.weapon.movement;
            if s.count == 0
                || s.count >= 32767
                || s.mount.iter().any(|v| !v.is_finite())
                || m.minimum_speed < 0
                || m.maximum_speed < m.minimum_speed
                || !(0..=i32::MAX / 256).contains(&m.acceleration)
                || !(0..=i32::MAX / 256).contains(&m.deceleration)
                || s.weapon.burst.actual_rounds_per_game == 0
            {
                return Err(super::invalid("invalid live station bounds"));
            }
        }
        Ok(())
    }
    pub fn from_source(
        a: &Aircraft,
        mut read: impl FnMut(&str) -> Result<Vec<u8>>,
    ) -> Result<Self> {
        let mut stations = Vec::new();
        let mut external_equipment_lbs = 0i32;
        let mut external_fuel_lbs = [0.; 9];
        for (index, h) in a
            .hardpoints
            .iter()
            .enumerate()
            .filter(|(_, h)| h.flags & 8 == 0)
        {
            if let Some(name) = h.store.as_deref() {
                let weight = if name.ends_with(".GAS") {
                    let tank = tore_formats::weapons::Tank::parse(&read(name)?)?;
                    if let Some(fuel) = external_fuel_lbs.get_mut(index) {
                        *fuel = f64::from(tank.fuel_weight) * f64::from(h.count);
                    }
                    i32::from(tank.empty_weight).checked_add(tank.fuel_weight)
                } else if name.ends_with(".SEE") {
                    let e = tore_formats::aircraft::Equipment::parse(name, &read(name)?)?;
                    Some(
                        e.fields
                            .get("weight")
                            .ok_or_else(|| super::invalid("missing equipment weight"))?
                            .number()?,
                    )
                } else {
                    Some(0)
                }
                .ok_or_else(|| super::invalid("external equipment mass overflow"))?;
                if weight < 0 {
                    return Err(super::invalid("negative external equipment weight"));
                }
                external_equipment_lbs = external_equipment_lbs
                    .checked_add(
                        weight
                            .checked_mul(h.count)
                            .ok_or_else(|| super::invalid("external mass overflow"))?,
                    )
                    .ok_or_else(|| super::invalid("external mass overflow"))?;
            }
        }
        for h in &a.hardpoints {
            let Some(name) = h.store.as_deref().filter(|n| n.ends_with(".JT")) else {
                continue;
            };
            let weapon = Weapon::parse(name, &read(name)?)?;
            // Restrict the live adapter to the actual default stations of the
            // reviewed aircraft. Catalog import never makes another type flyable.
            let permitted = match a.id {
                AircraftId::Mig29 => ["AA8.JT", "GSH301.JT"].contains(&name),
                AircraftId::Su27 => ["AA11.JT", "AA12.JT", "GSH301.JT"].contains(&name),
                AircraftId::Mig21 => ["AA2.JT", "GSH23.JT"].contains(&name),
                AircraftId::Su25 => ["AA8.JT", "AS7.JT", "B13.JT", "GSH301.JT"].contains(&name),
                AircraftId::Mig23 => ["AS7.JT", "B8.JT", "GSH6_30.JT"].contains(&name),
                AircraftId::Su35 => ["AA11B.JT", "AA12.JT", "AAML.JT", "GSH301.JT"].contains(&name),
                AircraftId::F22 | AircraftId::F22n | AircraftId::Faxx => {
                    ["AGM65G.JT", "AIM120.JT", "AIM9X.JT", "M61.JT"].contains(&name)
                }

                AircraftId::F18 => ["M61.JT", "AIM120.JT", "AGM65G.JT", "AIM9M.JT"].contains(&name),
                AircraftId::F14 => ["M61.JT", "AIM54C.JT", "AIM120.JT", "AIM9M.JT"].contains(&name),
                AircraftId::A4E => ["MK12.JT", "MK82.JT", "LAU61.JT"].contains(&name),
                AircraftId::X31 => ["M61.JT", "AIM120.JT", "AGM65G.JT", "AIM9X.JT"].contains(&name),
                AircraftId::Rafale => {
                    ["DEFA.JT", "AGM65G.JT", "MICA.JT", "R530.JT", "R550.JT"].contains(&name)
                }
            };
            if weapon.movement.acceleration > i32::MAX / 256
                || weapon.movement.deceleration > i32::MAX / 256
            {
                return Err(super::invalid("live acceleration exceeds fixed8 domain"));
            }
            if !permitted
                || h.count <= 0
                || h.count > 32766
                || weapon.burst.actual_rounds_per_game == 0
            {
                return Err(super::invalid("unreviewed live-fire station"));
            }
            stations.push(Station {
                weapon,
                mount: h.position.map(|v| f64::from(v) / 3.),
                count: h.count as u16,
                internal: h.flags & 8 != 0,
            });
        }
        let hit_points = a
            .object
            .get("hitPoints")
            .ok_or_else(|| super::invalid("missing aircraft hit points"))?
            .number()?;
        if hit_points <= 0 || stations.is_empty() || !stations[0].internal {
            return Err(super::invalid("invalid live-fire aircraft configuration"));
        }
        // Sensors resolve by parsed record channel, so a missing device is an
        // explicit state rather than another aircraft's radar.
        let profiles = sensors::SensorProfiles::from_source(a, &mut read)?;
        let station = |record: Option<&String>| {
            record.and_then(|record| {
                a.hardpoints.iter().position(|h| {
                    h.store
                        .as_deref()
                        .is_some_and(|n| n.eq_ignore_ascii_case(record))
                })
            })
        };
        let radar_hardpoint = station(profiles.radar.as_ref().map(|r| &r.record))
            .ok_or_else(|| super::invalid("missing reviewed radar station"))?;
        let visual_hardpoint = station(profiles.visual.as_ref().map(|v| &v.record))
            .ok_or_else(|| super::invalid("missing reviewed visual sensor"))?;
        let infrared_hardpoint = station(profiles.infrared.as_ref().map(|i| &i.record));
        let ecm_hardpoint = a
            .hardpoints
            .iter()
            .position(|h| h.store.as_deref().is_some_and(|n| n.ends_with(".ECM")))
            .ok_or_else(|| super::invalid("missing ECM"))?;
        let ecm = tore_formats::weapons::Countermeasures::parse(
            a.hardpoints[ecm_hardpoint].store.as_deref().unwrap(),
            &read(a.hardpoints[ecm_hardpoint].store.as_deref().unwrap())?,
        )?;
        let mut rwr_hardpoint = None;
        for (index, h) in a.hardpoints.iter().enumerate() {
            if let Some(name) = h.store.as_deref().filter(|name| name.ends_with(".SEE")) {
                let equipment = tore_formats::aircraft::Equipment::parse(name, &read(name)?)?;
                if equipment
                    .fields
                    .get("sig")
                    .map(|v| v.number())
                    .transpose()?
                    == Some(4)
                {
                    rwr_hardpoint = Some(index);
                }
            }
        }
        let mut system_damage = [0; 45];
        for (i, out) in system_damage.iter_mut().enumerate() {
            *out = a
                .fields
                .get(&format!("systemDamage[{i}]"))
                .ok_or_else(|| super::invalid("missing system damage"))?
                .number()? as u8;
        }
        let mut slot = 0;
        let hardpoint_slots = a
            .hardpoints
            .iter()
            .map(|h| {
                if h.store.as_deref().is_some_and(|n| n.ends_with(".JT")) {
                    let i = slot;
                    slot += 1;
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        let damage_capacity = hit_points
            .checked_mul(2)
            .filter(|v| *v <= i32::from(i16::MAX))
            .ok_or_else(|| super::invalid("native player damage capacity"))?;
        let engines = a
            .fields
            .get("engines")
            .map(|v| v.number())
            .transpose()?
            .unwrap_or(1)
            .clamp(1, 4) as u8;
        let fuel = a.number("internalFuel") + external_fuel_lbs.iter().sum::<f64>();
        let mass = a
            .object
            .get("weight")
            .map(|v| v.number())
            .transpose()?
            .unwrap_or(1) as f64
            + a.number("internalFuel")
            + f64::from(external_equipment_lbs)
            + stations
                .iter()
                .filter(|s| !s.internal)
                .map(|s| f64::from(s.weapon.weight) * f64::from(s.count))
                .sum::<f64>();
        let wreck_power = crate::wreck::Power::symmetric(
            engines,
            a.number("thrust") * 0.7 / mass.max(1.) * 32.174,
            fuel / (a.number("fuelConsumption") * 0.7).max(0.001),
        );
        Ok(Self {
            fragment_offsets: [
                super::debris::attachment(a.id, 0, &mut read)?,
                super::debris::attachment(a.id, 1, &mut read)?,
            ],
            ecm,
            system_damage,
            damage_capacity,
            afterburner_available: a
                .fields
                .get("aftThrust")
                .ok_or_else(|| super::invalid("missing afterburner thrust"))?
                .number()?
                != 0,
            hardpoint_slots,
            visual_hardpoint,
            radar_hardpoint,
            infrared_hardpoint,
            rwr_hardpoint,
            ecm_hardpoint,
            sensors: profiles,
            external_equipment_lbs,
            external_fuel_lbs,
            wreck_power,
            engines,
            aircraft: a.id,
            stations,
            hit_points,
            target_category: a
                .object
                .get("obj_class")
                .ok_or_else(|| super::invalid("missing object category"))?
                .number()? as u16,
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub aircraft: Option<AircraftId>,
    pub role: TargetRole,
    pub heat: Heat,
    pub radar_emitting: bool,
    pub id: u32,
    pub position: Vector,
    /// Ground-relative velocity, also used for the notch projection.
    pub velocity: Vector,
    pub basis: Basis,
    pub configuration: sensors::Configuration,
    pub signature: sensors::SignatureProfile,
    pub jammer: Option<sensors::JammerProfile>,
    pub jammer_active: bool,
    /// Physical airborne presence. Hit points reaching zero does not clear it.
    pub airborne: bool,
    /// Parked or rolling on a runway. Radar cannot see it (manual p.208);
    /// other sensors are unchanged. See [`sensors::Observable::on_ground`].
    pub on_ground: bool,
    pub wreck: Option<crate::wreck::Wreck>,
    pub wreck_power: crate::wreck::Power,
    pub radius: f64,
    pub hp: i32,
    pub initial_hp: i32,
    pub fragment_offsets: [Vector; 2],
    pub fragment_released: bool,
    pub localized_damage: LocalizedDamage,
    pub category: u16,
}

pub const DAMAGE_SECTIONS: usize = 6;
/// Contact sphere for an aircraft, feet.
const AIRCRAFT_RADIUS_FT: f64 = 28.;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageSection {
    Nose = 0,
    Cockpit = 1,
    Core = 2,
    LeftWing = 3,
    RightWing = 4,
    Tail = 5,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocalizedDamage {
    pub amounts: [i32; DAMAGE_SECTIONS],
    /// First reviewed A/C breakup pair whose local threshold was crossed.
    pub structural_variant: Option<usize>,
    pub structural_section: Option<DamageSection>,
}
impl LocalizedDamage {
    pub fn fractions(&self, initial_hp: i32) -> [f64; DAMAGE_SECTIONS] {
        self.amounts
            .map(|amount| (f64::from(amount) / f64::from(initial_hp.max(1))).clamp(0., 1.))
    }
    pub fn section(position: Vector, target: &Target) -> DamageSection {
        let offset = sub(position, target.position);
        let forward = crate::attitude::dot(offset, target.basis.forward) / target.radius.max(1.);
        let right = crate::attitude::dot(offset, target.basis.right) / target.radius.max(1.);
        let up = crate::attitude::dot(offset, target.basis.up) / target.radius.max(1.);
        if forward > 0.28 && up > 0.18 && right.abs() < 0.28 {
            DamageSection::Cockpit
        } else if forward > 0.45 {
            DamageSection::Nose
        } else if forward < -0.48 {
            DamageSection::Tail
        } else if right < -0.38 {
            DamageSection::LeftWing
        } else if right > 0.38 {
            DamageSection::RightWing
        } else {
            DamageSection::Core
        }
    }
    pub fn section_segment(from: Vector, to: Vector, target: &Target) -> DamageSection {
        Self::contact(from, to, target, 1.)
            .map(|(_, section)| section)
            .unwrap_or_else(|| Self::section(to, target))
    }
    /// `scale` enlarges every section, as Easy aiming does for the player's rounds.
    fn contact(
        from: Vector,
        to: Vector,
        target: &Target,
        scale: f64,
    ) -> Option<(f64, DamageSection)> {
        let local = |point: Vector| {
            let offset = sub(point, target.position);
            let radius = target.radius.max(1.) * scale;
            [
                crate::attitude::dot(offset, target.basis.right) / radius,
                crate::attitude::dot(offset, target.basis.up) / radius,
                crate::attitude::dot(offset, target.basis.forward) / radius,
            ]
        };
        let a = local(from);
        let b = local(to);
        let boxes = [
            (
                DamageSection::Cockpit,
                [-0.22, 0.08, 0.08],
                [0.22, 0.48, 0.48],
            ),
            (
                DamageSection::Core,
                [-0.28, -0.28, -0.38],
                [0.28, 0.22, 0.18],
            ),
            (
                DamageSection::Nose,
                [-0.32, -0.30, 0.42],
                [0.32, 0.32, 0.92],
            ),
            (
                DamageSection::LeftWing,
                [-0.92, -0.18, -0.28],
                [-0.25, 0.18, 0.38],
            ),
            (
                DamageSection::RightWing,
                [0.25, -0.18, -0.28],
                [0.92, 0.18, 0.38],
            ),
            (
                DamageSection::Tail,
                [-0.34, -0.25, -0.92],
                [0.34, 0.40, -0.34],
            ),
        ];
        boxes
            .into_iter()
            .filter_map(|(section, lo, hi)| {
                segment_box_fraction(a, b, lo, hi).map(|at| (at, section))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }
    fn record(&mut self, section: DamageSection, amount: i32, initial_hp: i32) {
        let slot = section as usize;
        self.amounts[slot] = self.amounts[slot].saturating_add(amount.max(0));
        let threshold = (initial_hp.max(1) * 3 + 3) / 4;
        if self.amounts[slot] >= threshold && self.structural_variant.is_none() {
            self.structural_variant = match section {
                DamageSection::Nose | DamageSection::Cockpit | DamageSection::Core => Some(0),
                DamageSection::LeftWing | DamageSection::RightWing | DamageSection::Tail => Some(1),
            };
            self.structural_section = Some(section);
        }
    }
}
impl Target {
    pub fn damage_fraction(&self) -> f64 {
        (1. - f64::from(self.hp.max(0)) / f64::from(self.initial_hp.max(1))).clamp(0., 1.)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Projectile {
    pub id: u32,
    /// Who fired this round. `0` is the player; any other value is an AI
    /// actor's id. Score counters are attributed with it, so an AI aircraft
    /// killing another AI aircraft does not credit the player.
    pub owner: u32,
    /// Actor-owned weapon for AI releases; player shots use the configured station.
    pub weapon: Option<Weapon>,
    pub guidance: Option<Flight>,
    pub motion: Option<Motion>,
    pub guidance_ticks: Option<u64>,
    pub age: u64,
    pub incoming: bool,
    pub station: usize,
    pub position: Vector,
    pub previous: Vector,
    pub direction: Vector,
    pub speed_f8: i32,
    pub launched_t: u16,
    pub target: Option<u32>,
    pub fall: FallState,
    /// Physical gun-round position within the source representative debit.
    pub gun_round: Option<u8>,
    /// Fitted presentation marker: every third physical gun round.
    pub tracer: bool,
}

/// One actor's current fire-control answer. The host replaces these snapshots
/// every fixed tick. A projectile can consume only the entry matching its owner
/// and the observation matching its retained target identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorSupport {
    pub owner: u32,
    pub observation: Option<seeker::Observation>,
    pub supported: bool,
    pub radar_position: Vector,
    pub radar_emitting: bool,
}
impl Projectile {
    pub fn weapon<'a>(&'a self, config: &'a Configuration) -> &'a Weapon {
        self.weapon
            .as_ref()
            .unwrap_or_else(|| &config.stations[self.station].weapon)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Flare,
    Chaff,
    Launch,
    Hit,
    Destroyed,
    Ground,
    DebrisImpact,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub position: Vector,
    pub kind: EffectKind,
    pub ticks: u16,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Fired(usize),
    SeekerActivated(u32),
    Pitbull(u32),
    Hit(u32),
    Destroyed(u32),
    Airburst(u32),
    Ground,
    TrackLost(u32),
    PlayerDamaged(i32),
    SubsystemDamaged(usize),
    PlayerDestroyed,
    PilotKilled,
    PlayerGroundImpact,
    Defeated(u32),
    /// A missile or bomb burst on an aircraft; the host knocks it around.
    Jolt(Jolt),
}
/// Blast on an aircraft: `target` None is the player. `strength` is the
/// warhead's damage against that aircraft over 100.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jolt {
    pub target: Option<u32>,
    pub from: Vector,
    pub strength: f64,
}
/// Mounted weapon audio state, independent of playback and rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeekerTone {
    pub strength: f64,
    pub ground: bool,
    pub radar: bool,
    pub locked: bool,
}
impl SeekerTone {
    /// Fitted HUD-percentage curve with the recovered half-volume search rule.
    pub fn ir_strength(percent: u8, locked: bool) -> f64 {
        (0.15 + 0.85 * f64::from(percent.min(100)) / 100.) * if locked { 1. } else { 0.5 }
    }
}

#[derive(Clone, Copy, Debug)]
struct RangeEstimate {
    station: usize,
    target: u32,
    mode: LaunchMode,
    maximum: f64,
    favorable: Option<missiles::FiringBand>,
}

#[derive(Clone, Debug)]
pub struct State {
    pub release_readiness: Readiness,
    pub launch_mode: LaunchMode,
    pub mounted: Seeker,
    /// Provisional bore return for HUD estimates only, never a designation or lock.
    pub bore_observation: Option<seeker::Observation>,
    mounted_key: Option<(usize, LaunchMode, Option<u32>)>,
    pub weapon_rules: Rules,
    range_estimate: Option<RangeEstimate>,
    config: Configuration,
    pub ammo: Vec<u16>,
    pub selected: usize,
    pub sensors: Sensors,
    /// Easy targeting's memory of the last selection, kept after sensor
    /// loss for the HUD square only; never grants weapon support.
    hud_selection: Option<u32>,
    /// Friendly aircraft identities, which T and Enter skip. Set by the host.
    pub friendlies: std::collections::BTreeSet<u32>,
    /// Passive emitters received this step, for the exposure instrument.
    pub emitters: Vec<passive::Emitter>,
    pub projectiles: Vec<Projectile>,
    pub targets: Vec<Target>,
    actor_support: BTreeMap<u32, ActorSupport>,
    pub missile_threats: super::threats::ThreatService,
    /// Ground contact volumes keyed by stable target ID. Aircraft remain spheres.
    ground_bounds: BTreeMap<u32, crate::airport::OrientedBox>,
    pub effects: Vec<Effect>,
    /// Presentation events retain emission positions independently of visual life.
    /// Bounded even when a headless host never drains them.
    sound_events: Vec<crate::acoustics::Emission>,
    pub smoke: super::smoke::Smoke,
    pub debris: Vec<super::debris::Piece>,
    player_fragment_released: bool,
    player_explosion_reported: bool,
    player_localized_damage: LocalizedDamage,
    pub shots: u32,
    pub hits: u32,
    pub kills: u32,
    /// Launches, outcomes and kills by shooter, for the debrief.
    pub ledger: super::ledger::Ledger,
    pub armed: bool,
    pub player_hp: i32,
    pub player_damage: i32,
    pub subsystem_counts: [u8; 45],
    pub last_subsystem: Option<usize>,
    pub radar_failed: bool,
    pub visual_failed: bool,
    pub infrared_failed: bool,
    pub rwr_failed: bool,
    pub ecm_failed: bool,
    pub chaff: u8,
    pub flares: u8,
    pub target_jammer: bool,
    rng: u32,
    pending_damage: bool,
    previous_player_position: Option<Vector>,
    pub history: Vec<HitRecord>,
    strikes: Vec<Strike>,
    pub range_category: u16,
    next_target_id: u32,
    external: bool,
    tick: u64,
    service_remainder: u16,
    triggers: Vec<PlayerTrigger>,
    gun_cadence: Vec<GunCadence>,
    /// Player session cheats: Invulnerable and Unlimited ammo.
    pub cheats: crate::cheats::Cheats,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GunCadence {
    pending: u16,
    next_scaled: u64,
    ordinal: u64,
}
#[derive(Clone, Copy)]
pub struct Launcher {
    /// Cockpit power switch, independent of active sensor channel/transmission.
    pub radar_power: bool,
    pub position: Vector,
    pub basis: Basis,
    pub speed_fps: f64,
    pub velocity: Vector,
    pub bay_ready: bool,
    /// Radar actually transmitting. Selecting infrared stops the emission.
    pub radar: bool,
    pub jammer: bool,
    pub alive: bool,
    /// Player sensor controls, applied as an input at each step so replay
    /// reproduces channel, scope range and history changes.
    pub controls: sensors::Controls,
}
impl State {
    pub fn player_damage_section(&self) -> Option<DamageSection> {
        self.player_localized_damage.structural_section
    }
    pub fn player_damage_regions(&self) -> [f64; DAMAGE_SECTIONS] {
        self.player_localized_damage
            .fractions(self.config.damage_capacity)
    }
    /// Development-only visual fixture. Gameplay damage always arrives through impacts.
    pub fn preview_localized_damage(&mut self, section: DamageSection, fraction: f64) {
        let fraction = fraction.clamp(0., 1.);
        let amount = (f64::from(self.config.damage_capacity) * fraction).round() as i32;
        self.player_localized_damage = LocalizedDamage::default();
        self.player_localized_damage
            .record(section, amount, self.config.damage_capacity);
        for target in &mut self.targets {
            let amount = (f64::from(target.initial_hp) * fraction).round() as i32;
            target.localized_damage = LocalizedDamage::default();
            target
                .localized_damage
                .record(section, amount, target.initial_hp);
        }
    }
    pub fn configuration(&self) -> &Configuration {
        &self.config
    }
    pub fn new(config: Configuration, external: bool) -> Result<Self> {
        config.validate()?;
        let ammo = config
            .stations
            .iter()
            .map(|s| if s.internal || external { s.count } else { 0 })
            .collect();
        let triggers = vec![PlayerTrigger::default(); config.stations.len()];
        let gun_cadence = vec![GunCadence::default(); config.stations.len()];
        let range_category = config.target_category;
        let sensors = Sensors::new(config.sensors.clone());
        Ok(Self {
            release_readiness: Readiness::Safe,
            launch_mode: LaunchMode::Cued,
            mounted: Seeker::default(),
            bore_observation: None,
            mounted_key: None,
            hud_selection: None,
            friendlies: Default::default(),
            weapon_rules: Rules::Spec,
            chaff: config.ecm.chaff[0],
            flares: config.ecm.flare[0],
            player_hp: config.damage_capacity,
            player_damage: 0,
            subsystem_counts: [0; 45],
            last_subsystem: None,
            radar_failed: false,
            visual_failed: false,
            infrared_failed: false,
            rwr_failed: false,
            ecm_failed: false,
            target_jammer: false,
            rng: 0x46414a54,
            pending_damage: false,
            previous_player_position: None,
            external,
            range_estimate: None,
            armed: true,
            history: vec![],
            strikes: vec![],
            range_category,
            next_target_id: 1,
            config,
            ammo,
            selected: 0,
            sensors,
            emitters: vec![],
            projectiles: vec![],
            targets: vec![],
            actor_support: BTreeMap::new(),
            missile_threats: super::threats::ThreatService::new(PLAYER_OWNER),
            ground_bounds: BTreeMap::new(),
            effects: vec![],
            sound_events: vec![],
            smoke: super::smoke::Smoke::default(),
            debris: Vec::new(),
            player_fragment_released: false,
            player_explosion_reported: false,
            player_localized_damage: LocalizedDamage::default(),
            shots: 0,
            hits: 0,
            kills: 0,
            ledger: Default::default(),
            tick: 0,
            service_remainder: 0,
            triggers,
            gun_cadence,
            cheats: Default::default(),
        })
    }
    pub fn release(&mut self) {
        for t in &mut self.triggers {
            t.release();
        }
        for cadence in &mut self.gun_cadence {
            cadence.pending = 0;
        }
    }

    /// Replace all non-player fire-control snapshots for the next missile step.
    /// Player support remains sourced from this State's own sensor component.
    pub fn set_actor_supports(&mut self, supports: impl IntoIterator<Item = ActorSupport>) {
        self.actor_support.clear();
        self.actor_support
            .extend(supports.into_iter().map(|support| (support.owner, support)));
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Current permitted missile measurements for RWR and AI awareness. This
    /// exposes seeker state and actor-owned support, never hidden target poses.
    pub fn missile_snapshots(&self, player: Launcher) -> Vec<super::threats::MissileSnapshot> {
        self.projectiles
            .iter()
            .filter_map(|projectile| {
                let weapon = projectile.weapon(&self.config);
                let flight = projectile.guidance.as_ref();
                let profile = flight
                    .map(|flight| flight.profile)
                    .or_else(|| missiles::Profile::for_weapon(weapon))?;
                let target = flight
                    .and_then(|flight| flight.seeker.target)
                    .or(projectile.target);
                let support = if projectile.owner == PLAYER_OWNER {
                    target.map(|id| ActorSupport {
                        owner: PLAYER_OWNER,
                        observation: self.sensors.observation(id).map(|contact| {
                            let delta = sub(contact.position, projectile.position);
                            seeker::Observation {
                                id,
                                position: contact.position,
                                velocity: contact.velocity,
                                quality: 1.,
                                off_axis: dot(unit(delta), projectile.direction)
                                    .clamp(-1., 1.)
                                    .acos(),
                                range: missiles::length(delta),
                            }
                        }),
                        supported: self.sensors.supports(id),
                        radar_position: player.position,
                        radar_emitting: player.radar && player.alive,
                    })
                } else {
                    self.actor_support.get(&projectile.owner).copied()
                };
                let supported = profile.guidance == Guidance::Supported
                    && support.is_some_and(|answer| {
                        answer.supported
                            && answer.radar_emitting
                            && answer.observation.is_some_and(|o| Some(o.id) == target)
                    });
                let velocity = projectile.motion.map_or_else(
                    || {
                        projectile
                            .direction
                            .map(|axis| axis * f64::from(projectile.speed_f8) / 256.)
                    },
                    |motion| motion.velocity,
                );
                Some(super::threats::MissileSnapshot {
                    id: projectile.id,
                    owner: projectile.owner,
                    position: projectile.position,
                    velocity,
                    guidance: profile.guidance,
                    target,
                    radar_active: flight.is_some_and(|flight| {
                        profile.guidance == Guidance::Active
                            && flight.enabled
                            && flight.seeker.status != Status::Expired
                    }),
                    radar_acquired: flight.is_some_and(|flight| {
                        flight.seeker.status == Status::Pitbull
                            && flight.seeker.observation.is_some()
                    }),
                    supported,
                    supporting_radar_position: supported
                        .then(|| support.map(|answer| answer.radar_position))
                        .flatten(),
                    alive: true,
                })
            })
            .collect()
    }
    /// Player selection ring: NAV, then each configured weapon station.
    /// Legacy range commands retain their old station-only behavior for tapes.
    pub fn cycle_selection(&mut self, forward: bool) {
        let count = self.ammo.len();
        let current = if self.armed { self.selected + 1 } else { 0 };
        let mut next = current;
        // Air combat guns only skips every station but the gun.
        loop {
            next = if forward {
                (next + 1) % (count + 1)
            } else {
                (next + count) % (count + 1)
            };
            if next == 0 || next == current || self.station_allowed(next - 1) {
                break;
            }
        }
        self.release();
        self.bore_observation = None;
        self.mounted = Seeker::default();
        self.mounted_key = None;
        self.launch_mode = LaunchMode::Cued;
        self.armed = next != 0;
        if self.armed {
            self.selected = next - 1;
        }
    }
    fn station_allowed(&self, station: usize) -> bool {
        !self.cheats.guns_only || is_gun(&self.config.stations[station].weapon)
    }
    /// Guns only turned on with a missile selected moves to the gun, or to
    /// NAV when the aircraft has none.
    fn enforce_guns_only(&mut self) {
        if !self.armed || self.station_allowed(self.selected) {
            return;
        }
        self.release();
        self.bore_observation = None;
        self.mounted = Seeker::default();
        self.mounted_key = None;
        self.launch_mode = LaunchMode::Cued;
        match (0..self.ammo.len()).find(|i| self.station_allowed(*i)) {
            Some(gun) => self.selected = gun,
            None => self.armed = false,
        }
    }
    pub fn select_next(&mut self) {
        self.release();
        self.bore_observation = None;
        self.mounted = Seeker::default();
        self.mounted_key = None;
        self.selected = (self.selected + 1) % self.ammo.len();
        if missiles::Profile::for_weapon(&self.config.stations[self.selected].weapon)
            .is_none_or(|p| !p.supports_boresight())
        {
            self.launch_mode = LaunchMode::Cued;
        }
    }
    /// T (forward) and Shift-T cycle radar contacts; friendly aircraft and
    /// wrecks are skipped.
    pub fn designate_next(&mut self, forward: bool) {
        let friendlies = &self.friendlies;
        self.sensors.cycle(forward, |id| friendlies.contains(&id));
        self.selection_changed();
    }
    /// Enter selects the visible sensor contact nearest the nose.
    pub fn designate_visual(&mut self, launcher: Launcher) {
        let friendlies = &self.friendlies;
        self.sensors
            .select_visual(launcher.position, launcher.basis, |id| {
                friendlies.contains(&id)
            });
        self.selection_changed();
    }
    fn selection_changed(&mut self) {
        self.hud_selection = self.designated();
        if self.designated().is_some() {
            self.launch_mode = LaunchMode::Cued;
        }
    }
    pub fn designated(&self) -> Option<u32> {
        self.sensors.selected()
    }
    /// The target the HUD square and target camera follow: the selection, or
    /// with Easy targeting the last selection after the sensors lose it.
    pub fn display_target(&self) -> Option<&Target> {
        let id = if self.cheats.easy_targeting {
            self.designated().or(self.hud_selection)
        } else {
            self.designated()
        }?;
        self.targets
            .iter()
            .find(|target| target.id == id && target.hp > 0)
    }
    pub fn command(&mut self, command: Command, launcher: Launcher) {
        match command {
            Command::ClearRange => {
                self.targets
                    .retain(|t| self.ground_bounds.contains_key(&t.id));
                self.sensors.clear_selection();
                self.hud_selection = None;
                self.bore_observation = None;
                self.mounted = Seeker::default();
            }
            Command::TargetDistance(distance) => {
                if (1..=1_000_000).contains(&distance) {
                    for target in self
                        .targets
                        .iter_mut()
                        .filter(|t| !self.ground_bounds.contains_key(&t.id))
                    {
                        target.position = std::array::from_fn(|i| {
                            launcher.position[i] + launcher.basis.forward[i] * f64::from(distance)
                        });
                    }
                }
            }
            Command::CompatibilityWeapons => {
                self.weapon_rules = Rules::Compatibility;
                self.launch_mode = LaunchMode::Cued;
                self.bore_observation = None;
                self.mounted = Seeker::default();
            }
            Command::TargetHeat(value) => {
                for t in self
                    .targets
                    .iter_mut()
                    .filter(|t| !self.ground_bounds.contains_key(&t.id))
                {
                    t.heat = match value {
                        0 => Heat::Unknown,
                        1 => Heat::Engine {
                            on: false,
                            throttle: 0.,
                            afterburner: false,
                        },
                        2 => Heat::Engine {
                            on: true,
                            throttle: 0.,
                            afterburner: false,
                        },
                        3 => Heat::Engine {
                            on: true,
                            throttle: 1.,
                            afterburner: false,
                        },
                        _ => Heat::Engine {
                            on: true,
                            throttle: 1.,
                            afterburner: true,
                        },
                    };
                }
            }
            Command::ToggleTargetRadar => {
                for t in self
                    .targets
                    .iter_mut()
                    .filter(|t| !self.ground_bounds.contains_key(&t.id))
                {
                    t.radar_emitting = !t.radar_emitting;
                }
            }
            Command::Incoming => {
                if self.projectiles.len() < MAX_PROJECTILES && self.player_hp > 0 {
                    let w = &self.config.stations[self.selected].weapon;
                    let position = std::array::from_fn(|i| {
                        launcher.position[i] + launcher.basis.forward[i] * 1800.
                    });
                    self.projectiles.push(Projectile {
                        id: self.shots,
                        owner: PLAYER_OWNER,
                        weapon: None,
                        guidance: None,
                        motion: None,
                        guidance_ticks: None,
                        age: 0,
                        incoming: true,
                        station: self.selected,
                        position,
                        previous: position,
                        direction: launcher.basis.forward.map(|v| -v),
                        speed_f8: launch_speed(&w.movement, (launcher.speed_fps * 256.) as i32)
                            .expect("validated speed")
                            * 256,
                        launched_t: (self.tick / 30) as u16,
                        target: if w.seeker.signature != 0 {
                            Some(0)
                        } else {
                            None
                        },
                        fall: FallState::default(),
                        gun_round: None,
                        tracer: false,
                    });
                }
            }
            Command::DamagePlayer => self.pending_damage = true,
            Command::ToggleTargetJammer => {
                self.target_jammer = !self.target_jammer;
                for t in self
                    .targets
                    .iter_mut()
                    .filter(|t| !self.ground_bounds.contains_key(&t.id))
                {
                    t.jammer_active = self.target_jammer;
                }
            }
            Command::ReleaseChaff => self.release_countermeasure(EffectKind::Chaff, launcher),
            Command::ReleaseFlare => self.release_countermeasure(EffectKind::Flare, launcher),
            Command::ToggleSeekerMode => {
                if self.weapon_rules == Rules::Compatibility || !self.guidance_available(launcher) {
                    return;
                }
                if missiles::Profile::for_weapon(&self.config.stations[self.selected].weapon)
                    .is_none_or(|p| !p.supports_boresight())
                {
                    return;
                }
                if self.designated().is_some()
                    && missiles::Profile::for_weapon(&self.config.stations[self.selected].weapon)
                        .is_some_and(|p| p.guidance == Guidance::Infrared)
                {
                    return;
                }
                self.launch_mode = if self.launch_mode == LaunchMode::Cued {
                    LaunchMode::Boresight
                } else {
                    LaunchMode::Cued
                };
                self.bore_observation = None;
                self.mounted = Seeker::default();
                self.mounted_key = None;
                self.release();
            }
            Command::NextWeapon => self.select_next(),
            Command::NextSelection => self.cycle_selection(true),
            Command::PreviousSelection => self.cycle_selection(false),
            Command::SelectNav => {
                self.release();
                self.armed = false;
                self.bore_observation = None;
                self.mounted = Seeker::default();
                self.mounted_key = None;
                self.launch_mode = LaunchMode::Cued;
            }
            Command::Designate => self.designate_next(true),
            Command::DesignatePrevious => self.designate_next(false),
            Command::DesignateVisual => self.designate_visual(launcher),
            Command::DesignateTarget(id) => {
                if self.sensors.designate(id) {
                    self.hud_selection = Some(id);
                }
                if self.designated().is_some() {
                    self.launch_mode = LaunchMode::Cued;
                }
            }
            Command::ClearDesignation => {
                self.sensors.clear_selection();
                self.hud_selection = None;
                self.bore_observation = None;
                self.mounted = Seeker::default();
                self.mounted_key = None;
                self.release();
            }
            Command::ToggleArm => {
                self.armed = !self.armed;
                self.release();
            }
            Command::Jettison => {
                if !self.config.stations[self.selected].internal {
                    unload(&mut self.ammo[self.selected], 0);
                    self.release();
                }
            }
            Command::ReplaceTarget => self.range_target(launcher),
            Command::CycleClass => {
                self.range_category = [self.config.target_category, 0x2000, 0x100, 0x400, 0x40]
                    [(damage_class(self.range_category) + 1) % 5];
                self.range_target(launcher);
            }
            // Native equipment damage marks the station's high bit. Selecting
            // the failure manually is a test fixture, not a recovered damage roll.
            Command::FailStation => {
                self.ammo[self.selected] |= 0x8000;
                self.release();
            }
        }
    }
    /// One device per press, as the retail "Chaff launched, %d left" message
    /// reports. Each missile guiding on the player with the matching seeker
    /// class rolls the B47 decoy chance (docs/spec/countermeasures.md). An
    /// empty or damaged dispenser releases nothing.
    fn release_countermeasure(&mut self, kind: EffectKind, launcher: Launcher) {
        let (count, effectiveness, signature) = match kind {
            EffectKind::Chaff => (&mut self.chaff, self.config.ecm.chaff[1], 3),
            _ => (&mut self.flares, self.config.ecm.flare[1], 2),
        };
        if *count == 0 || !launcher.alive || self.player_hp <= 0 {
            return;
        }
        if !self.cheats.unlimited_ammo {
            *count -= 1;
        }
        self.effect(launcher.position, kind);
        for projectile in &mut self.projectiles {
            let weapon = projectile
                .weapon
                .as_ref()
                .unwrap_or(&self.config.stations[projectile.station].weapon);
            let guiding = projectile.target == Some(0)
                && weapon.seeker.signature == signature
                && projectile.guidance.as_ref().is_none_or(|flight| {
                    flight.enabled && flight.seeker.acquired && flight.seeker.observation.is_some()
                });
            if !guiding {
                continue;
            }
            let threshold = crate::ai::threat::decoy_threshold(
                weapon.seeker.chaff_flare_chance.min(100),
                effectiveness.min(100),
            );
            if draw(&mut self.rng, 100) < u16::from(threshold) {
                self.ledger.resolve(projectile.id, Resolution::Spoofed);
                projectile.target = None;
                projectile.guidance = None;
            }
        }
    }
    fn apply_player_damage(&mut self, amount: i32, events: &mut Vec<Event>) {
        let applied = amount.max(0).min(self.player_hp);
        if applied == 0 {
            return;
        }
        self.player_hp -= applied;
        self.player_damage = self.player_damage.saturating_add(amount);
        events.push(Event::PlayerDamaged(applied));
        let chance = super::systems::subsystem_chance(
            self.player_damage,
            self.config.damage_capacity,
            amount,
        );
        if chance > 0
            && i32::from(draw(&mut self.rng, 100)) < chance
            && let Some(index) = super::systems::select(
                &self.config.system_damage,
                &self.subsystem_counts,
                self.player_damage,
                self.config.damage_capacity,
                self.config.afterburner_available,
                |n| draw(&mut self.rng, n),
            )
        {
            self.subsystem_counts[index] += 1;
            self.last_subsystem = Some(index);
            events.push(Event::SubsystemDamaged(index));
            if let Some(h) = index.checked_sub(36) {
                if let Some(Some(slot)) = self.config.hardpoint_slots.get(h) {
                    if self.rounds(*slot) > 0 {
                        self.ammo[*slot] |= 0x8000;
                    }
                } else if h == self.config.radar_hardpoint {
                    self.radar_failed = true;
                } else if h == self.config.visual_hardpoint {
                    self.visual_failed = true;
                } else if Some(h) == self.config.infrared_hardpoint {
                    self.infrared_failed = true;
                } else if Some(h) == self.config.rwr_hardpoint {
                    self.rwr_failed = true;
                } else if h == self.config.ecm_hardpoint {
                    for _ in 0..10 {
                        let roll = draw(&mut self.rng, 100);
                        if roll < 25 && self.config.ecm.mode_flags & 0x110 != 0 {
                            self.ecm_failed = true;
                            self.chaff = 0;
                            self.flares = 0;
                            break;
                        } else if (25..65).contains(&roll) && self.config.ecm.chaff[0] != 0 {
                            self.chaff = 0;
                            break;
                        } else if roll >= 65 && self.config.ecm.flare[0] != 0 {
                            self.flares = 0;
                            break;
                        }
                    }
                }
            }
        }
        for index in super::systems::accumulated_faults(
            &self.config.system_damage,
            &self.subsystem_counts,
            self.player_damage,
            self.config.damage_capacity,
        ) {
            self.subsystem_counts[index] += 1;
            self.last_subsystem = Some(index);
            events.push(Event::SubsystemDamaged(index));
        }
        if self.player_hp == 0 {
            self.release();
            events.push(Event::PlayerDestroyed);
        }
    }
    /// Ownship subsystem lifecycle reached a fatal outcome outside a projectile hit.
    pub fn systems_destroyed(&mut self) -> Option<Event> {
        if self.player_hp <= 0 {
            return None;
        }
        self.player_hp = 0;
        self.player_damage = self.player_damage.max(self.config.damage_capacity);
        self.release();
        Some(Event::PlayerDestroyed)
    }
    pub fn player_airburst(&mut self, position: Vector) -> Option<Event> {
        self.player_explosion(position, Event::Airburst(0))
    }
    pub fn player_ground_impact(&mut self, position: Vector) -> Option<Event> {
        self.player_explosion(position, Event::PlayerGroundImpact)
    }
    fn player_explosion(&mut self, position: Vector, event: Event) -> Option<Event> {
        if self.player_explosion_reported {
            return None;
        }
        self.player_explosion_reported = true;
        self.debris.retain(|piece| piece.owner != 0);
        self.effect(position, EffectKind::Destroyed);
        Some(event)
    }
    pub fn rounds(&self, station: usize) -> u16 {
        self.ammo[station] & 0x7fff
    }
    pub fn readiness(&self, launcher: Launcher) -> Readiness {
        if !launcher.alive || self.player_hp <= 0 {
            return Readiness::LauncherLost;
        }
        if !self.armed {
            return Readiness::Safe;
        }
        if self.ammo[self.selected] & 0x8000 != 0 {
            return Readiness::StationFailed;
        }
        if self.rounds(self.selected) == 0 {
            return Readiness::Empty;
        }
        if self.projectiles.len() >= MAX_PROJECTILES {
            return Readiness::Capacity;
        }
        if !launcher.bay_ready && !self.config.stations[self.selected].internal {
            return Readiness::BayClosed;
        }
        self.launch_solution(launcher)
    }
    fn launch_solution(&self, launcher: Launcher) -> Readiness {
        let w = &self.config.stations[self.selected].weapon;
        if w.seeker.signature == 0 {
            return Readiness::Ready;
        }
        let profile = (self.weapon_rules == Rules::Spec)
            .then(|| missiles::Profile::for_weapon(w))
            .flatten();
        if profile.is_some_and(|p| !p.guidance_available(launcher.radar_power)) {
            return Readiness::Ready;
        }
        if profile.is_some_and(|p| p.supports_boresight())
            && self.launch_mode == LaunchMode::Boresight
        {
            if self.bore_observation.is_some_and(|o| {
                missiles::length(sub(o.position, launcher.position))
                    < f64::from(w.seeker.zones[1].minimum_range.max(0))
            }) {
                return Readiness::MinimumRange;
            }
            return Readiness::Ready;
        }
        let Some(t) = self
            .designated()
            .and_then(|id| self.targets.iter().find(|t| t.id == id))
        else {
            return Readiness::NoTarget;
        };
        if profile.is_some_and(|p| !p.accepts(t)) {
            return Readiness::WrongTarget;
        }
        if t.hp <= 0 {
            return Readiness::TargetDestroyed;
        }
        if w.seeker.signature == 3 {
            // Equipment state answers immediately, before the shared support
            // result, so a failure reported between steps is not stale.
            if self.radar_failed {
                return Readiness::RadarFailed;
            }
            if !launcher.radar {
                return Readiness::RadarOff;
            }
            // One shared support answer for this specific target. The weapon
            // keeps its own envelope test below.
            match self.sensors.support(t.id) {
                Support::Tracked => {}
                Support::RadarFailed => return Readiness::RadarFailed,
                Support::RadarOff => return Readiness::RadarOff,
                Support::Unavailable => return Readiness::NoRadar,
                Support::SearchOnly => return Readiness::RadarSearchOnly,
                Support::Acquiring => return Readiness::RadarAcquiring,
                Support::TrackCoverage => return Readiness::RadarCoverage,
                Support::NotSelected | Support::NoObservation => return Readiness::NoTarget,
            }
        }
        if let Some(profile) = profile {
            if !missiles::geometry(
                &missiles::launch_geometry(w),
                launcher.position,
                launcher.basis,
                t.position,
                None,
            ) {
                let old = zone_readiness(
                    &missiles::launch_geometry(w),
                    launcher.position,
                    launcher.basis.forward,
                    t.position,
                );
                return if old == Readiness::Ready {
                    Readiness::FieldOfView
                } else {
                    old
                };
            }
            if matches!(profile.guidance, Guidance::Infrared | Guidance::Emitter)
                && (self.mounted.target != Some(t.id) || self.mounted.status != Status::Locked)
            {
                return Readiness::RadarAcquiring;
            }
            if self.mounted_solution(launcher).is_none() {
                return Readiness::MaximumRange;
            }
            return Readiness::Ready;
        }
        zone_readiness(
            &w.seeker.zones[1],
            launcher.position,
            launcher.basis.forward,
            t.position,
        )
    }
    pub fn external_fuel_lbs(&self) -> [f64; 9] {
        if self.external {
            self.config.external_fuel_lbs
        } else {
            [0.; 9]
        }
    }
    pub fn payload_lbs(&self) -> f64 {
        f64::from(if self.external {
            self.config.external_equipment_lbs
        } else {
            0
        }) + self
            .config
            .stations
            .iter()
            .zip(&self.ammo)
            .filter(|(s, _)| !s.internal)
            .map(|(s, count)| f64::from(s.weapon.weight.max(0)) * f64::from(*count & 0x7fff))
            .sum::<f64>()
    }
    /// An explicit, non-AI range target of the selected ported aircraft. No
    /// targets are inserted into ordinary free flight or fabricated on scopes.
    /// Straight-flight mission fixture. No steering, sensors transmitting or AI.
    pub fn add_dummy(&mut self, config: &Configuration, position: Vector, basis: Basis) {
        let id = self.next_target_id;
        self.next_target_id = id.checked_add(1).expect("target ID exhaustion");
        self.targets.push(Target {
            aircraft: Some(config.aircraft),
            role: TargetRole::Aircraft,
            id,
            position,
            basis,
            velocity: basis.forward.map(|v| v * 300.),
            heat: Heat::Engine {
                on: true,
                throttle: 0.7,
                afterburner: false,
            },
            radar_emitting: false,
            configuration: sensors::Configuration::CLEAN,
            signature: config.sensors.signature,
            jammer: config.sensors.jammer.clone(),
            jammer_active: false,
            airborne: true,
            on_ground: false,
            radius: AIRCRAFT_RADIUS_FT,
            hp: config.hit_points,
            initial_hp: config.hit_points,
            fragment_offsets: config.fragment_offsets,
            wreck: None,
            wreck_power: config.wreck_power,
            fragment_released: false,
            localized_damage: LocalizedDamage::default(),
            category: config.target_category,
        });
    }
    /// Replace imported scene objects without changing aircraft or fixture IDs.
    pub fn remove_ground_targets(&mut self) {
        self.projectiles.retain(|p| {
            !p.target
                .is_some_and(|id| self.ground_bounds.contains_key(&id))
        });
        self.targets
            .retain(|t| !self.ground_bounds.contains_key(&t.id));
        self.ground_bounds.clear();
        self.sensors.clear_selection();
        self.hud_selection = None;
    }
    /// Register one imported, stationary surface object without consuming an
    /// aircraft roster index. The caller owns the explicit disjoint ID range.
    pub fn add_ground_target(
        &mut self,
        id: u32,
        bounds: crate::airport::OrientedBox,
        hit_points: i32,
        category: u16,
    ) -> Result<()> {
        if id == 0
            || !bounds.valid()
            || hit_points <= 0
            || self.targets.iter().any(|target| target.id == id)
            || self.ground_bounds.contains_key(&id)
        {
            return Err(super::invalid("invalid or duplicate ground target"));
        }
        let basis = Basis::new(bounds.heading, bounds.pitch, bounds.bank);
        // Aim inside the upper half of the solid volume so a planar ground
        // object's own terrain endpoint does not occlude its sensor observation.
        let aim = std::array::from_fn(|i| bounds.center[i] + basis.up[i] * bounds.half[1] * 0.5);
        self.targets.push(Target {
            aircraft: None,
            role: TargetRole::Surface,
            heat: Heat::Unknown,
            radar_emitting: false,
            id,
            position: aim,
            velocity: [0.; 3],
            basis,
            configuration: sensors::Configuration::CLEAN,
            signature: sensors::SignatureProfile::default(),
            jammer: None,
            jammer_active: false,
            airborne: false,
            on_ground: false,
            radius: bounds.half[0].max(bounds.half[1]).max(bounds.half[2]),
            hp: hit_points,
            initial_hp: hit_points,
            fragment_offsets: [[0.; 3]; 2],
            wreck: None,
            wreck_power: crate::wreck::Power::default(),
            fragment_released: false,
            localized_damage: LocalizedDamage::default(),
            category,
        });
        self.ground_bounds.insert(id, bounds);
        Ok(())
    }
    pub fn range_target(&mut self, launcher: Launcher) {
        let w = &self.config.stations[self.selected].weapon;
        let distance = if w.seeker.signature == 0 {
            900.
        } else {
            f64::from(w.seeker.zones[1].minimum_range) + 3000.
        };
        // Retire the previous engagement atomically. Never let an old missile
        // hit or track a replacement fixture with a reused identity.
        self.release();
        self.projectiles.clear();
        self.effects.clear();
        self.smoke = super::smoke::Smoke::default();
        self.debris.clear();
        self.targets
            .retain(|t| self.ground_bounds.contains_key(&t.id));
        let id = self.next_target_id;
        self.next_target_id = self
            .next_target_id
            .checked_add(1)
            .expect("range ID exhaustion");
        // The fixture is a copy of this aircraft, so it carries the same PT
        // signatures and ECM record. No AI or autonomous behaviour is added.
        let yaw = launcher.basis.forward[0].atan2(launcher.basis.forward[2]);
        self.targets.push(Target {
            aircraft: Some(self.config.aircraft),
            role: TargetRole::Aircraft,
            heat: Heat::Unknown,
            radar_emitting: false,
            id,
            category: self.range_category,
            position: std::array::from_fn(|i| {
                launcher.position[i] + launcher.basis.forward[i] * distance
            }),
            velocity: launcher.basis.forward.map(|v| v * 300.),
            basis: Basis::new(yaw, 0., 0.),
            configuration: sensors::Configuration::CLEAN,
            signature: self.config.sensors.signature,
            jammer: self.config.sensors.jammer.clone(),
            jammer_active: self.target_jammer,
            airborne: true,
            on_ground: false,
            radius: AIRCRAFT_RADIUS_FT,
            hp: self.config.hit_points,
            initial_hp: self.config.hit_points,
            fragment_offsets: self.config.fragment_offsets,
            wreck: None,
            wreck_power: self.config.wreck_power,
            fragment_released: false,
            localized_damage: LocalizedDamage::default(),
        });
        self.sensors.clear_selection();
        self.hud_selection = None;
    }
    /// Any current observation of this object, on the selected scope channel
    /// or visually. Channels are never collapsed into one another.
    pub fn detects(&self, target: &Target) -> bool {
        self.sensors.observation(target.id).is_some()
    }
    pub fn guidance_available(&self, launcher: Launcher) -> bool {
        missiles::Profile::for_weapon(&self.config.stations[self.selected].weapon)
            .is_none_or(|p| p.guidance_available(launcher.radar_power))
    }
    pub fn can_lock(&self, launcher: Launcher) -> bool {
        if self.weapon_rules == Rules::Spec && !self.guidance_available(launcher) {
            return false;
        }
        if self.weapon_rules == Rules::Spec
            && missiles::Profile::for_weapon(&self.config.stations[self.selected].weapon)
                .is_some_and(|p| p.independent())
        {
            return self.mounted.target == self.designated()
                && self.mounted.target.is_some()
                && matches!(self.mounted.status, Status::Locked | Status::Pitbull);
        }
        self.config.stations[self.selected].weapon.seeker.signature != 0
            && self.launch_solution(launcher) == Readiness::Ready
    }

    /// The seeker tone plays only while the seeker is actively tracking:
    /// silence with nothing in it (John, 2026-09-23).
    pub fn seeker_tone(&self, launcher: Launcher) -> Option<SeekerTone> {
        let w = &self.config.stations[self.selected].weapon;
        let profile = missiles::Profile::for_weapon(w)?;
        if self.weapon_rules != Rules::Spec
            || !self.guidance_available(launcher)
            || !self.armed
            || !launcher.alive
            || self.player_hp <= 0
            || self.rounds(self.selected) == 0
            || self.ammo[self.selected] & 0x8000 != 0
            || profile.guidance == Guidance::Emitter
        {
            return None;
        }
        let radar = matches!(profile.guidance, Guidance::Active | Guidance::Supported);
        // A radar missile goes quiet with its target inside minimum range.
        let minimum = f64::from(w.seeker.zones[1].minimum_range.max(0));
        let too_close = |o: seeker::Observation| {
            radar && missiles::length(sub(o.position, launcher.position)) < minimum
        };
        // A radar missile in boresight sounds its lock tone on the bore return,
        // with or without a designated target (John, 2026-09-23).
        if radar && self.launch_mode == LaunchMode::Boresight {
            let o = self.bore_observation.filter(|o| !too_close(*o))?;
            return Some(SeekerTone {
                strength: 0.4 + 0.6 * o.quality.clamp(0., 1.),
                ground: false,
                radar,
                locked: true,
            });
        }
        let bore_ir = if self.launch_mode == LaunchMode::Boresight {
            // Use the HUD's eligible return, never a stale or hidden target.
            let observed = self.weapon_observation(launcher)?;
            let tracked = self.mounted.observation?;
            if tracked.id != observed.id {
                return None;
            }
            Some(observed)
        } else {
            None
        };
        // Otherwise sound only while the mounted seeker is tracking.
        if self.mounted.observation.is_none_or(too_close) {
            return None;
        }
        let locked = matches!(self.mounted.status, Status::Locked | Status::Pitbull)
            && bore_ir.is_none_or(|o| self.mounted.target == Some(o.id));
        Some(SeekerTone {
            strength: if radar {
                self.mounted.tone()
            } else {
                SeekerTone::ir_strength(self.estimated_hit_percent(launcher), locked)
            },
            ground: !radar && w.flags & 0x10000 == 0,
            radar,
            locked,
        })
    }
    /// Current observation used by the display, separate from launch authority.
    pub fn weapon_observation(&self, launcher: Launcher) -> Option<seeker::Observation> {
        if !self.armed || (self.weapon_rules == Rules::Spec && !self.guidance_available(launcher)) {
            return None;
        }
        if self.launch_mode == LaunchMode::Boresight {
            let w = &self.config.stations[self.selected].weapon;
            let profile = missiles::Profile::for_weapon(w)?;
            return self.bore_observation.filter(|o| {
                profile.guidance != Guidance::Infrared
                    || (missiles::geometry(
                        &missiles::launch_geometry(w),
                        launcher.position,
                        launcher.basis,
                        o.position,
                        None,
                    ) && missiles::intercept(
                        &w.movement,
                        Motion::new(&w.movement, launcher.velocity, launcher.position[1]),
                        launcher.position,
                        launcher.basis.forward,
                        o.position,
                        o.velocity,
                        0,
                        profile.guidance_ticks,
                    )
                    .is_some())
            });
        }
        self.mounted.observation.or_else(|| {
            let id = self.designated()?;
            let w = &self.config.stations[self.selected].weapon;
            if self.weapon_rules == Rules::Spec
                && missiles::Profile::for_weapon(w).is_some_and(|p| {
                    self.targets
                        .iter()
                        .find(|t| t.id == id)
                        .is_none_or(|t| !p.accepts(t))
                })
            {
                return None;
            }
            let contact = self.sensors.observation(id)?;
            let delta = missiles::sub(contact.position, launcher.position);
            Some(seeker::Observation {
                id: contact.id,
                position: contact.position,
                velocity: contact.velocity,
                quality: 1.,
                range: missiles::length(delta),
                off_axis: dot(unit(delta), launcher.basis.forward)
                    .clamp(-1., 1.)
                    .acos(),
            })
        })
    }
    pub fn mounted_solution(&self, launcher: Launcher) -> Option<missiles::Solution> {
        let w = &self.config.stations[self.selected].weapon;
        let profile = missiles::Profile::for_weapon(w)?;
        let observed = self.weapon_observation(launcher)?;
        missiles::intercept(
            &w.movement,
            Motion::new(&w.movement, launcher.velocity, launcher.position[1]),
            launcher.position,
            launcher.basis.forward,
            observed.position,
            observed.velocity,
            0,
            profile.guidance_ticks,
        )
    }
    pub fn estimated_max_range(&self, launcher: Launcher) -> Option<f64> {
        let observed = self.weapon_observation(launcher)?;
        self.range_estimate
            .filter(|e| {
                e.station == self.selected && e.target == observed.id && e.mode == self.launch_mode
            })
            .map(|e| e.maximum)
    }
    pub fn favorable_firing_band(&self, launcher: Launcher) -> Option<missiles::FiringBand> {
        let observed = self.weapon_observation(launcher)?;
        self.range_estimate
            .filter(|e| {
                e.station == self.selected && e.target == observed.id && e.mode == self.launch_mode
            })
            .and_then(|e| e.favorable)
    }
    /// Physical range validity is independent of rounded probability text.
    pub fn in_estimated_range(&self, launcher: Launcher) -> bool {
        let Some(o) = self.weapon_observation(launcher) else {
            return false;
        };
        let min =
            f64::from(self.config.stations[self.selected].weapon.seeker.zones[1].minimum_range);
        self.readiness(launcher) == Readiness::Ready
            && self
                .estimated_max_range(launcher)
                .is_some_and(|max| max > min && (min..=max).contains(&o.range))
            && self.mounted_solution(launcher).is_some()
    }
    pub fn estimated_hit_percent(&self, launcher: Launcher) -> u8 {
        let Some(observation) = self.weapon_observation(launcher) else {
            return 0;
        };
        let w = &self.config.stations[self.selected].weapon;
        let Some(profile) = missiles::Profile::for_weapon(w) else {
            return 0;
        };
        let mut zone = w.seeker.zones[1];
        zone.maximum_range = self.estimated_max_range(launcher).unwrap_or(0.).floor() as _;
        missiles::estimated_hit_percent(
            observation,
            self.mounted_solution(launcher),
            &zone,
            profile
                .guidance_ticks
                .min(u64::from(w.movement.remove_t) * 30) as f64
                / 120.,
            (self.launch_mode == LaunchMode::Boresight).then(|| profile.search_cap()),
        )
    }

    pub fn take_sound_events(&mut self) -> Vec<crate::acoustics::Emission> {
        std::mem::take(&mut self.sound_events)
    }
    /// Projectile damage since the last drain, oldest first.
    pub fn take_strikes(&mut self) -> Vec<Strike> {
        std::mem::take(&mut self.strikes)
    }
    fn strike(&mut self, strike: Strike) {
        if self.strikes.len() == MAX_STRIKES {
            self.strikes.remove(0);
        }
        self.strikes.push(strike);
    }

    fn emit_sound(&mut self, position: Vector, kind: crate::acoustics::Kind) {
        if self.sound_events.len() == 256 {
            self.sound_events.remove(0);
        }
        self.sound_events.push(crate::acoustics::Emission {
            kind,
            position,
            arrived: false,
        });
    }

    fn effect(&mut self, position: Vector, kind: EffectKind) {
        let sound = match kind {
            EffectKind::Hit | EffectKind::Ground => Some(crate::acoustics::Kind::Impact),
            EffectKind::Destroyed => Some(crate::acoustics::Kind::Explosion),
            _ => None,
        };
        if let Some(kind) = sound {
            self.emit_sound(position, kind);
        }
        if self.effects.len() == MAX_EFFECTS {
            self.effects.remove(0);
        }
        self.effects.push(Effect {
            position,
            kind,
            ticks: if kind == EffectKind::Destroyed {
                240
            } else {
                45
            },
        });
    }
    /// Exactly one host 120 Hz tick. Pausing means NOT calling this method.
    /// The host-to-native time conversion and stage ordering are authored here.
    pub fn step(
        &mut self,
        held: bool,
        launcher: Launcher,
        ground: impl Fn(f64, f64) -> f64,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        self.enforce_guns_only();
        if std::mem::take(&mut self.pending_damage) && self.player_hp > 0 {
            // Explicit no-AI hit fixture uses this aircraft's gun damage. Native
            // percent input is 100; deterministic adapter RNG is not native RNG.
            let base = scaled_weapon_damage(
                &self.config.stations[0].weapon,
                i32::from(
                    self.config.stations[0].weapon.damage.by_class
                        [damage_class(self.config.target_category)],
                ),
            ) as u16;
            let amount = super::systems::damage_amount(base, 100, draw(&mut self.rng, 40) as u8);
            self.player_localized_damage.record(
                DamageSection::Core,
                amount,
                self.config.damage_capacity,
            );
            self.apply_player_damage(amount, &mut events);
            self.emit_sound(launcher.position, crate::acoustics::Kind::Impact);
        }
        let now = (self.tick / 30) as u16;
        self.tick += 1;
        self.service_remainder += 256;
        let service = (self.service_remainder / 120) as i16;
        self.service_remainder %= 120;
        for e in &mut self.effects {
            e.ticks = e.ticks.saturating_sub(1);
        }
        self.effects.retain(|e| e.ticks > 0);
        // Shared observations are produced before this tick's firing decision,
        // so the scope, the target view and weapon support all agree.
        self.sensors.controls = launcher.controls;
        let observables: Vec<Observable> = self
            .targets
            .iter()
            .map(|t| {
                Observable {
                    id: t.id,
                    position: t.position,
                    velocity: t.velocity,
                    basis: t.basis,
                    configuration: t.configuration,
                    signature: t.signature,
                    jammer: t.jammer.clone(),
                    jammer_active: t.jammer_active,
                    radar_emitting: t.radar_emitting,
                    airborne: t.airborne,
                    destroyed: t.hp <= 0,
                }
                .on_ground(t.on_ground)
            })
            .collect();
        let observer = Observer {
            position: launcher.position,
            basis: launcher.basis,
            radar_powered: launcher.radar && launcher.alive,
            radar_failed: self.radar_failed,
            infrared_failed: self.infrared_failed || !launcher.alive,
            visual_failed: self.visual_failed || !launcher.alive,
        };
        let obscured = |from: Vector, to: Vector| terrain_hit(from, to, &ground).is_some();
        let height = |x: f64, z: f64| ground(x, z);
        let environment = sensors::Environment {
            ground: &height,
            obscured: &obscured,
        };
        self.sensors.keep_selection = self.cheats.easy_targeting;
        self.sensors.step(&observer, &observables, &environment);
        if let Some(id) = self.designated() {
            self.hud_selection = Some(id);
        } else if !self.cheats.easy_targeting {
            // A dropped target is gone; nothing is remembered for later.
            self.hud_selection = None;
        }
        self.emitters = passive::emitters(
            &observer,
            &observables,
            self.sensors.contacts(),
            &environment,
        );
        self.bore_observation = None;
        let index = self.selected;
        let w = &self.config.stations[index].weapon;
        if let Some(profile) =
            missiles::Profile::for_weapon(w).filter(|_| self.weapon_rules == Rules::Spec)
        {
            if !profile.guidance_available(launcher.radar_power) || !profile.supports_boresight() {
                self.launch_mode = LaunchMode::Cued;
            }
            if profile.guidance_available(launcher.radar_power)
                && self.armed
                && self.designated().is_none()
                && profile.supports_boresight()
            {
                self.launch_mode = LaunchMode::Boresight;
            }
            if profile.guidance == Guidance::Infrared && self.designated().is_some() {
                self.launch_mode = LaunchMode::Cued;
            }
            let assigned = if self.launch_mode == LaunchMode::Cued {
                self.designated()
            } else {
                None
            };
            let key = (index, self.launch_mode, assigned);
            if self.mounted_key != Some(key) {
                self.mounted = Seeker::new(assigned);
                self.mounted_key = Some(key);
            }
            if !profile.guidance_available(launcher.radar_power) {
                self.mounted = Seeker {
                    status: Status::Unguided,
                    ..Default::default()
                };
                self.mounted_key = None;
            }
            if self.armed
                && profile.guidance_available(launcher.radar_power)
                && launcher.alive
                && self.player_hp > 0
                && self.rounds(index) > 0
                && self.ammo[index] & 0x8000 == 0
            {
                let bore = self.launch_mode == LaunchMode::Boresight;
                let cap = bore.then(|| profile.search_cap());
                // Mounted IR may choose a stronger return. Released missiles keep identity.
                if bore && profile.guidance == Guidance::Infrared {
                    self.mounted.target = None;
                    self.mounted.acquired = false;
                    self.mounted.missing = 0;
                }
                let view = seeker::View {
                    position: launcher.position,
                    basis: launcher.basis,
                    cap,
                    obscured: &obscured,
                };
                let observations: Vec<_> = self
                    .targets
                    .iter()
                    .filter(|t| t.hp > 0)
                    .filter(|t| bore || assigned == Some(t.id))
                    .filter_map(|t| seeker::observe(w, profile, &view, t))
                    .filter(|o| {
                        !bore
                            || profile.guidance != Guidance::Active
                            || self.config.sensors.radar.as_ref().is_some_and(|r| {
                                o.range <= launcher.controls.range_nmi() * missiles::NMI
                                    && o.range <= r.track.maximum_ft
                            })
                    })
                    .collect();
                if bore {
                    self.bore_observation = observations
                        .iter()
                        .min_by(|a, b| seeker::compare_returns(a, b, profile))
                        .copied();
                }
                if bore && profile.guidance == Guidance::Active {
                    // The HUD estimate never pre-locks or assigns an active-radar shot.
                    self.mounted = Seeker::default();
                } else if self.launch_mode == LaunchMode::Cued
                    && matches!(profile.guidance, Guidance::Active | Guidance::Supported)
                {
                    let supported: Vec<_> = observations
                        .into_iter()
                        .filter(|o| self.sensors.supports(o.id))
                        .collect();
                    self.mounted.step(
                        missiles::Profile {
                            guidance: Guidance::Supported,
                            ..profile
                        },
                        &supported,
                    );
                } else {
                    self.mounted.step(profile, &observations);
                }
            } else {
                self.mounted = Seeker::new(assigned);
            }
        } else {
            self.bore_observation = None;
            self.mounted = Seeker::default();
            self.mounted_key = None;
        }
        if let Some(o) = self.weapon_observation(launcher) {
            if self.tick.is_multiple_of(60)
                || self.range_estimate.is_none_or(|e| {
                    e.station != index || e.target != o.id || e.mode != self.launch_mode
                })
            {
                self.range_estimate = missiles::Profile::for_weapon(w).map(|profile| {
                    let maximum = missiles::maximum_range(
                        w,
                        launcher.position,
                        launcher.basis.forward,
                        launcher.velocity,
                        o.position,
                        o.velocity,
                        profile.guidance_ticks,
                    );
                    RangeEstimate {
                        station: index,
                        target: o.id,
                        mode: self.launch_mode,
                        maximum,
                        favorable: missiles::firing_band(
                            w,
                            launcher.position,
                            launcher.basis,
                            launcher.velocity,
                            o,
                            maximum,
                            profile.guidance_ticks,
                            self.launch_mode == LaunchMode::Boresight,
                        ),
                    }
                });
            }
        } else {
            self.range_estimate = None;
        }
        self.release_readiness = self.readiness(launcher);
        let allowed = self.release_readiness == Readiness::Ready;
        let station = &self.config.stations[index];
        let w = &station.weapon;
        let guided = w.seeker.signature != 0;
        let gun = is_gun(w);
        let pressed = held && launcher.alive && !self.triggers[index].was_held;
        let due = self.triggers[index].poll(held && launcher.alive, w.flags, w.burst.game_burst_t, now)
                // A gun repress uses the retained physical-round deadline,
                // not the old representative burst's quarter-second deadline.
                || (gun && pressed);
        let (count, debit, gun_round, tracer) = if gun {
            let cadence = &mut self.gun_cadence[index];
            let physical_rounds = u16::from(w.burst.game_rounds_in_burst.max(1))
                .saturating_mul(u16::from(w.burst.actual_rounds_per_game.max(1)));
            if due && allowed {
                cadence.pending = cadence
                    .pending
                    .saturating_add(physical_rounds)
                    .min(physical_rounds);
                cadence.next_scaled = cadence
                    .next_scaled
                    .max(self.tick.saturating_mul(u64::from(physical_rounds)));
            }
            if !held || !launcher.alive {
                cadence.pending = 0;
            }
            if !allowed && cadence.pending > 0 {
                cadence.next_scaled = self
                    .tick
                    .saturating_mul(u64::from(physical_rounds))
                    .saturating_add(u64::from(w.burst.game_burst_t.max(1)).saturating_mul(30));
            }
            let ready = cadence.pending > 0
                && allowed
                && self.tick.saturating_mul(u64::from(physical_rounds)) >= cadence.next_scaled;
            if ready {
                let ordinal = cadence.ordinal;
                (
                    1,
                    1,
                    Some((ordinal % u64::from(w.burst.actual_rounds_per_game.max(1))) as u8),
                    ordinal.is_multiple_of(3),
                )
            } else {
                (0, 1, None, false)
            }
        } else if due && allowed {
            (
                usize::from(w.burst.game_rounds_in_burst.max(1)).min(32),
                u16::from(w.burst.actual_rounds_per_game),
                None,
                false,
            )
        } else {
            (0, u16::from(w.burst.actual_rounds_per_game), None, false)
        };
        if count > 0 {
            for _ in 0..count {
                let loaded = if self.cheats.unlimited_ammo {
                    self.rounds(index) > 0
                } else {
                    unload(&mut self.ammo[index], debit)
                };
                if self.projectiles.len() == MAX_PROJECTILES || !loaded {
                    break;
                }
                let position = std::array::from_fn(|i| {
                    launcher.position[i]
                        + launcher.basis.right[i] * station.mount[0]
                        + launcher.basis.up[i] * station.mount[1]
                        + launcher.basis.forward[i] * station.mount[2]
                });
                let target =
                    if self.weapon_rules == Rules::Spec && !self.guidance_available(launcher) {
                        None
                    } else if self.launch_mode == LaunchMode::Boresight {
                        self.mounted.target
                    } else {
                        self.designated()
                    };
                let guidance = missiles::Profile::for_weapon(w)
                    .filter(|_| self.weapon_rules == Rules::Spec)
                    .map(|profile| {
                        let mut flight =
                            Flight::new(profile, self.launch_mode, target, launcher.position);
                        flight.qualified_target = target.filter(|id| {
                            self.targets
                                .iter()
                                .find(|t| t.id == *id)
                                .is_some_and(|t| flight.eligible(w, t))
                        });
                        if self.mounted.acquired
                            && self.mounted.target == target
                            && (profile.guidance != Guidance::Active
                                || self.launch_mode == LaunchMode::Boresight)
                        {
                            flight.seeker = self.mounted.clone();
                        }
                        if !profile.guidance_available(launcher.radar_power) {
                            flight.unguided = true;
                            flight.enabled = false;
                            flight.seeker = Seeker::default();
                            flight.seeker.status = Status::Unguided;
                        }
                        flight
                    });
                self.projectiles.push(Projectile {
                    id: self.shots,
                    owner: PLAYER_OWNER,
                    weapon: None,
                    guidance,
                    guidance_ticks: (self.weapon_rules == Rules::Spec)
                        .then(|| missiles::Profile::for_weapon(w).map(|p| p.guidance_ticks))
                        .flatten(),
                    motion: (self.weapon_rules == Rules::Spec
                        && missiles::Profile::for_weapon(w).is_some())
                    .then(|| Motion::new(&w.movement, launcher.velocity, position[1])),
                    age: 0,
                    incoming: false,
                    station: index,
                    position,
                    previous: position,
                    direction: launcher.basis.forward,
                    speed_f8: launch_speed(&w.movement, (launcher.speed_fps * 256.) as i32)
                        .expect("validated speed limits")
                        * 256,
                    launched_t: now,
                    target: if guided { target } else { None },
                    fall: FallState::default(),
                    gun_round,
                    tracer,
                });
                if let Some(flight) = self.projectiles.last().and_then(|p| p.guidance.as_ref())
                    && flight.profile.guidance == Guidance::Active
                    && flight.enabled
                {
                    events.push(Event::SeekerActivated(self.shots));
                    if flight.seeker.acquired {
                        events.push(Event::Pitbull(self.shots));
                    }
                }
                self.shots += 1;
                events.push(Event::Fired(index));
                if gun {
                    let cadence = &mut self.gun_cadence[index];
                    cadence.pending -= 1;
                    cadence.ordinal = cadence.ordinal.wrapping_add(1);
                    cadence.next_scaled = cadence
                        .next_scaled
                        .saturating_add(u64::from(w.burst.game_burst_t.max(1)).saturating_mul(30));
                }
            }
        }
        if events.iter().any(|e| matches!(e, Event::Fired(_))) {
            self.effect(launcher.position, EffectKind::Launch);
            self.bore_observation = None;
            self.mounted = Seeker::default();
            self.mounted_key = None;
        }
        // Living target poses remain owned by their existing flight service.
        let old_targets: Vec<_> = self.targets.iter().map(|t| t.position).collect();
        let mut airbursts = Vec::new();
        for t in &mut self.targets {
            if t.hp > 0 {
                for i in 0..3 {
                    t.position[i] += t.velocity[i] / 120.;
                }
            } else if t.airborne {
                let wreck = t.wreck.get_or_insert_with(|| {
                    let mut wreck = crate::wreck::Wreck::new(t.id, self.tick, [0.; 3]);
                    if !matches!(t.heat, Heat::Engine { on: false, .. }) {
                        wreck.power = t.wreck_power;
                    }
                    wreck
                });
                if let Some(phase) =
                    wreck.step(&mut t.position, &mut t.velocity, &mut t.basis, &ground)
                {
                    t.airborne = false;
                    if phase == crate::wreck::Phase::Exploded {
                        airbursts.push((t.id, t.position));
                    }
                }
            }
        }
        for (id, position) in airbursts {
            self.debris.retain(|piece| piece.owner != id);
            self.effect(position, EffectKind::Destroyed);
            events.push(Event::Airburst(id));
        }
        let previous_player = self
            .previous_player_position
            .replace(launcher.position)
            .unwrap_or(launcher.position);
        let player = Target {
            aircraft: Some(self.config.aircraft),
            role: TargetRole::Aircraft,
            heat: Heat::Unknown,
            radar_emitting: launcher.radar,
            id: 0,
            position: launcher.position,
            velocity: launcher.velocity,
            basis: launcher.basis,
            configuration: sensors::Configuration::CLEAN,
            signature: self.config.sensors.signature,
            jammer: None,
            jammer_active: false,
            airborne: launcher.alive,
            on_ground: false,
            radius: AIRCRAFT_RADIUS_FT,
            hp: if launcher.alive { self.player_hp } else { 0 },
            initial_hp: self.config.damage_capacity,
            fragment_offsets: self.config.fragment_offsets,
            wreck: None,
            wreck_power: crate::wreck::Power::default(),
            fragment_released: self.player_fragment_released,
            localized_damage: self.player_localized_damage.clone(),
            category: self.config.target_category,
        };
        let mut player_hits = Vec::new();
        let mut strikes = Vec::new();
        let mut impacts = Vec::new();
        let mut sources = Vec::new();
        self.projectiles.retain_mut(|p| {
            let owned = p.weapon.clone();
            let w = owned
                .as_ref()
                .unwrap_or_else(|| &self.config.stations[p.station].weapon);
            let easy = self.cheats.easy_aiming && p.owner == PLAYER_OWNER && !p.incoming;
            let eased = (easy && !is_gun(w)).then(|| eased_weapon(w));
            let w = eased.as_ref().unwrap_or(w);
            let hitbox = if easy {
                crate::cheats::EASY_AIMING_HITBOX
            } else {
                1.
            };
            if p.age == 0 {
                p.direction = projectile_launch_direction(w, p.direction, p.id, p.owner, p.station);
                self.ledger.launch(
                    p.id,
                    p.owner,
                    if p.incoming {
                        Some(PLAYER_OWNER)
                    } else {
                        p.target
                    },
                    ShotKind::of(w),
                );
            }
            let m = &w.movement;
            if if p.motion.is_some() {
                missiles::removed(m, p.age) || p.position[1] > 100000.
            } else {
                removal_due(m, now, p.launched_t, (p.position[1] * 256.) as i32)
            } {
                self.ledger.resolve(p.id, Resolution::Missed);
                return false;
            }
            p.previous = p.position;
            let phase = if p.motion.is_some() {
                missiles::phase(m, p.age)
            } else {
                engine_phase(m, now, p.launched_t)
            };
            if w.seeker.signature != 0 && phase == super::EnginePhase::Powered {
                sources.push((
                    std::array::from_fn(|i| p.position[i] - p.direction[i] * 4.),
                    super::smoke::Kind::Missile,
                ));
            }
            let old_direction = p.direction;
            if p.guidance.is_some() {
                let was_active = p.guidance.as_ref().unwrap().enabled;
                let was_acquired = p.guidance.as_ref().unwrap().seeker.acquired;
                let actor_support = if p.owner == PLAYER_OWNER {
                    p.guidance
                        .as_ref()
                        .and_then(|flight| flight.seeker.target)
                        .map(|id| ActorSupport {
                            owner: PLAYER_OWNER,
                            supported: self.sensors.supports(id),
                            observation: self.sensors.observation(id).map(|contact| {
                                let delta = sub(contact.position, p.position);
                                seeker::Observation {
                                    id,
                                    position: contact.position,
                                    velocity: contact.velocity,
                                    quality: 1.,
                                    off_axis: dot(unit(delta), p.direction).clamp(-1., 1.).acos(),
                                    range: missiles::length(delta),
                                }
                            }),
                            radar_position: launcher.position,
                            radar_emitting: launcher.radar && launcher.alive,
                        })
                } else {
                    self.actor_support.get(&p.owner).copied()
                };
                guide_owned(
                    p,
                    w,
                    &self.targets,
                    (p.owner != PLAYER_OWNER).then_some(&player),
                    actor_support.as_ref(),
                    &obscured,
                    &ground,
                );
                let f = p.guidance.as_ref().unwrap();
                if f.profile.guidance == Guidance::Active {
                    if !was_active && f.enabled {
                        events.push(Event::SeekerActivated(p.id));
                    }
                    if !was_acquired && f.seeker.status == Status::Pitbull {
                        events.push(Event::Pitbull(p.id));
                    }
                }
            } else {
                if p.guidance_ticks.is_some_and(|end| p.age >= end) {
                    p.target = None;
                }
                if let Some(t) = p.target.and_then(|id| {
                    if p.incoming && id == 0 && player.hp > 0 {
                        Some(&player)
                    } else {
                        self.targets.iter().find(|t| t.id == id && t.hp > 0)
                    }
                }) {
                    // Required illumination is specific to this missile's own
                    // target, never to whatever the cockpit has selected now.
                    let supported = if p.weapon.is_some() {
                        self.targets.iter().any(|owner| {
                            owner.id == p.owner && owner.hp > 0 && owner.radar_emitting
                        })
                    } else {
                        p.incoming || self.sensors.supports(t.id)
                    };
                    if acquisition(w, p.position, p.direction, t.position, supported, 0)
                        && terrain_hit(p.position, t.position, &ground).is_none()
                    {
                        let desired = unit(sub(t.position, p.position));
                        let rate = if phase == EnginePhase::Powered {
                            m.powered_turn_rate
                        } else {
                            m.unpowered_turn_rate
                        };
                        // Authored pursuit, capped by source angle-rate field; native
                        // PN, lead, sun, Doppler, ECM and RNG contracts remain open.
                        let angle = dot(p.direction, desired).clamp(-1., 1.).acos();
                        let fraction = (f64::from(rate.max(0)) * std::f64::consts::TAU
                            / 65520.
                            / 120.
                            / angle.max(1e-9))
                        .min(1.);
                        p.direction = unit(std::array::from_fn(|i| {
                            p.direction[i] * (1. - fraction) + desired[i] * fraction
                        }));
                    } else {
                        events.push(Event::TrackLost(t.id));
                        p.target = None;
                    }
                } else if let Some(id) = p.target.take() {
                    events.push(Event::TrackLost(id));
                }
            }
            if let Some(motion) = &mut p.motion {
                motion.turn(old_direction, p.direction);
                let delta = motion.step(m, p.age, p.direction);
                for (position, delta) in p.position.iter_mut().zip(delta) {
                    *position += delta;
                }
                p.speed_f8 = (missiles::length(motion.velocity) * 256.) as i32;
            } else {
                if w.flags & 0x40 != 0 {
                    let target =
                        commanded_speed(m, phase, p.speed_f8, (p.position[1] * 256.) as i32) as i16;
                    p.speed_f8 = axial_speed(m, p.speed_f8, target, false, service)
                        .expect("validated movement");
                }
                let distance = f64::from(p.speed_f8) * f64::from(service) / 65536.;
                for i in 0..3 {
                    p.position[i] += p.direction[i] * distance;
                }
                p.position[1] = f64::from(
                    p.fall
                        .advance(
                            w.flags & 4 != 0,
                            phase,
                            service,
                            (p.position[1] * 256.) as i32,
                        )
                        .expect("positive service"),
                ) / 256.;
            }
            let armed = if p.motion.is_some() {
                p.age >= u64::from(w.damage.fuze_arm_t) * 30
            } else {
                now.wrapping_sub(p.launched_t) >= w.damage.fuze_arm_t
            };
            p.age += 1;
            let mut first: Option<(f64, Option<usize>)> = None;
            if armed
                && p.incoming
                && p.guidance.as_ref().is_none_or(|f| f.eligible(w, &player))
                && player.hp > 0
                && let Some(at) = if is_gun(w) {
                    let previous = std::array::from_fn(|i| {
                        p.previous[i] + player.position[i] - previous_player[i]
                    });
                    LocalizedDamage::contact(previous, p.position, &player, 1.).map(|v| v.0)
                } else {
                    segment_sphere(
                        sub(p.previous, previous_player),
                        sub(p.position, player.position),
                        player.radius + f64::from(w.damage.fuze_radius.max(0)),
                    )
                }
            {
                first = Some((at, Some(usize::MAX)));
            }
            if armed && !p.incoming {
                for (i, t) in self.targets.iter().enumerate().filter(|(_, t)| t.hp > 0) {
                    if p.guidance.as_ref().is_some_and(|f| !f.eligible(w, t)) {
                        continue;
                    }
                    let at = if let Some(bounds) = self.ground_bounds.get(&t.id) {
                        // Contact uses the reviewed/fitted solid box. Fuze blast
                        // radius remains a separate damage rule and does not turn
                        // a long runway into a giant interception sphere.
                        bounds.segment_fraction(p.previous, p.position)
                    } else if is_gun(w) && t.role == TargetRole::Aircraft {
                        let previous = std::array::from_fn(|axis| {
                            p.previous[axis] + t.position[axis] - old_targets[i][axis]
                        });
                        LocalizedDamage::contact(previous, p.position, t, hitbox).map(|v| v.0)
                    } else {
                        let radius = t.radius * hitbox + f64::from(w.damage.fuze_radius.max(0));
                        segment_sphere(
                            sub(p.previous, old_targets[i]),
                            sub(p.position, t.position),
                            radius,
                        )
                    };
                    if let Some(at) = at
                        && first.is_none_or(|f| at < f.0)
                    {
                        first = Some((at, Some(i)));
                    }
                }
            }
            // Sample the whole segment, then bisect the first crossing; bounded
            // contact approximation prevents fast rounds tunneling through terrain.
            if let Some(at) = terrain_hit(p.previous, p.position, &ground)
                && first.is_none_or(|f| at < f.0)
            {
                first = Some((at, None));
            }
            if let Some((at, target)) = first {
                let position =
                    std::array::from_fn(|i| p.previous[i] + (p.position[i] - p.previous[i]) * at);
                if target == Some(usize::MAX) {
                    let deception = super::systems::deception_chance(
                        self.config.ecm,
                        w.seeker.signature,
                        launcher.jammer && !self.ecm_failed,
                    );
                    if deception != 0
                        && i32::from(draw(&mut self.rng, 100))
                            >= super::systems::hit_chance(100, deception)
                    {
                        self.ledger.resolve(p.id, Resolution::Jammed);
                        events.push(Event::Defeated(0));
                    } else {
                        let base = projectile_damage(
                            p,
                            w,
                            i32::from(w.damage.by_class[damage_class(self.config.target_category)]),
                        ) as u16;
                        let amount =
                            super::systems::damage_amount(base, 100, draw(&mut self.rng, 40) as u8);
                        self.ledger
                            .resolve(p.id, Resolution::Hit(u32::try_from(amount).unwrap_or(0)));
                        let previous = std::array::from_fn(|i| {
                            p.previous[i] + player.position[i] - previous_player[i]
                        });
                        let section =
                            LocalizedDamage::section_segment(previous, p.position, &player);
                        player_hits.push((amount, section, is_gun(w), p.owner, w.flags));
                        impacts.push((position, EffectKind::Hit));
                        if !is_gun(w) {
                            events.push(Event::Jolt(Jolt {
                                target: None,
                                from: position,
                                strength: f64::from(
                                    w.damage.by_class[damage_class(self.config.target_category)],
                                ) / 100.,
                            }));
                        }
                    }
                    return false;
                }
                if let Some(i) = target {
                    let t = &mut self.targets[i];
                    let deception = super::systems::deception_chance(
                        self.config.ecm,
                        w.seeker.signature,
                        self.target_jammer && !self.ground_bounds.contains_key(&t.id),
                    );
                    if deception != 0
                        && i32::from(draw(&mut self.rng, 100))
                            >= super::systems::hit_chance(100, deception)
                    {
                        self.ledger.resolve(p.id, Resolution::Jammed);
                        events.push(Event::Defeated(t.id));
                        return false;
                    }
                    let class = damage_class(t.category);
                    let nominal = i32::from(w.damage.by_class[class]).max(0);
                    let previous = std::array::from_fn(|axis| {
                        p.previous[axis] + t.position[axis] - old_targets[i][axis]
                    });
                    let section = LocalizedDamage::section_segment(previous, p.position, t);
                    let scaled = projectile_damage(p, w, nominal);
                    if t.role == TargetRole::Aircraft && !is_gun(w) {
                        events.push(Event::Jolt(Jolt {
                            target: Some(t.id),
                            from: position,
                            strength: f64::from(nominal) / 100.,
                        }));
                    }
                    let critical = critical_hit(t, w, section, scaled);
                    let applied = if critical { t.hp } else { scaled.min(t.hp) };
                    t.hp -= applied;
                    if t.role == TargetRole::Aircraft {
                        t.localized_damage.record(section, scaled, t.initial_hp);
                    }
                    if self.history.len() == MAX_HIT_RECORDS {
                        self.history.remove(0);
                    }
                    self.history.push(HitRecord {
                        tick: self.tick,
                        target: t.id,
                        station: p.station,
                        class,
                        nominal,
                        applied,
                        hp_after: t.hp,
                    });
                    // Only the player's own rounds move the player's score.
                    // An AI aircraft killing another AI aircraft still raises
                    // the hit and destroyed events the host needs for damage,
                    // debris and effects.
                    if p.owner == PLAYER_OWNER {
                        self.hits += 1;
                    }
                    self.ledger
                        .resolve(p.id, Resolution::Hit(u32::try_from(applied).unwrap_or(0)));
                    let credit = Kill {
                        owner: p.owner,
                        victim: t.id,
                        category: t.category,
                        aircraft: t.role == TargetRole::Aircraft,
                    };
                    self.ledger.damaged(credit);
                    events.push(Event::Hit(t.id));
                    strikes.push(Strike {
                        owner: p.owner,
                        victim: Some(t.id),
                        weapon_flags: w.flags,
                        destroyed: t.hp == 0,
                    });
                    if t.hp == 0 {
                        if p.owner == PLAYER_OWNER {
                            self.kills += 1;
                        }
                        self.ledger.kill(credit);
                        events.push(Event::Destroyed(t.id));
                    }
                    impacts.push((
                        position,
                        if t.hp == 0 {
                            EffectKind::Destroyed
                        } else {
                            EffectKind::Hit
                        },
                    ));
                } else {
                    self.ledger.resolve(p.id, Resolution::Missed);
                    events.push(Event::Ground);
                    impacts.push((position, EffectKind::Ground));
                }
                return false;
            }
            true
        });
        for strike in strikes {
            self.strike(strike);
        }
        // Invulnerable: hits still show their impact effect but do no damage.
        if self.cheats.invulnerable {
            player_hits.clear();
        }
        for (amount, section, direct_gun, owner, weapon_flags) in player_hits {
            self.player_localized_damage
                .record(section, amount, self.config.damage_capacity);
            if direct_gun && section == DamageSection::Cockpit && self.player_hp > 0 {
                events.push(Event::PilotKilled);
            }
            let amount = if direct_gun
                && (section == DamageSection::Cockpit
                    || (section == DamageSection::Core
                        && amount >= self.config.damage_capacity / 2))
            {
                self.player_hp
            } else {
                amount
            };
            let alive = self.player_hp > 0;
            self.apply_player_damage(amount, &mut events);
            if alive && amount > 0 {
                self.strike(Strike {
                    owner,
                    victim: None,
                    weapon_flags,
                    destroyed: self.player_hp == 0,
                });
            }
        }
        for (p, kind) in impacts {
            self.effect(p, kind);
        }
        use super::smoke::Kind;
        for t in self
            .targets
            .iter()
            .filter(|t| t.airborne && t.damage_fraction() >= 0.5)
        {
            sources.push((
                std::array::from_fn(|i| t.position[i] - t.basis.forward[i] * 15.),
                Kind::Aircraft,
            ));
        }
        let falling_player = self.player_hp == 0
            && launcher.position[1] > ground(launcher.position[0], launcher.position[2]);
        if !self.player_explosion_reported
            && self.player_hp <= self.config.damage_capacity / 2
            && ((launcher.alive && self.player_hp > 0) || falling_player)
        {
            sources.push((
                std::array::from_fn(|i| launcher.position[i] - launcher.basis.forward[i] * 15.),
                Kind::Aircraft,
            ));
        }
        self.smoke.step(sources);
        let mut debris_impacts = Vec::new();
        self.debris.retain_mut(|piece| {
            if let Some(mut p) = piece.step(&ground) {
                p[1] += 6.;
                debris_impacts.push(p);
                false
            } else {
                true
            }
        });
        for p in debris_impacts {
            self.effect(p, EffectKind::DebrisImpact);
        }
        for t in &mut self.targets {
            if t.airborne
                && t.hp == 0
                && t.localized_damage.structural_section.is_some()
                && !t.fragment_released
            {
                t.fragment_released = true;
                let variant = t.aircraft.and_then(|aircraft| {
                    super::debris::damage_variant(
                        aircraft,
                        t.localized_damage.structural_section.unwrap() as usize,
                    )
                });
                if self.debris.len() < super::debris::MAX_PIECES
                    && let Some(variant) = variant
                {
                    self.debris.push(super::debris::Piece::new(
                        t.id,
                        variant,
                        t.position,
                        t.velocity,
                        t.basis,
                        t.fragment_offsets[variant],
                    ));
                }
            }
        }
        if self.player_hp == 0
            && !self.player_explosion_reported
            && self.player_localized_damage.structural_section.is_some()
            && !self.player_fragment_released
        {
            self.player_fragment_released = true;
            let variant = super::debris::damage_variant(
                self.config.aircraft,
                self.player_localized_damage.structural_section.unwrap() as usize,
            );
            if self.debris.len() < super::debris::MAX_PIECES
                && let Some(variant) = variant
            {
                self.debris.push(super::debris::Piece::new(
                    0,
                    variant,
                    launcher.position,
                    launcher.velocity,
                    launcher.basis,
                    self.config.fragment_offsets[variant],
                ));
            }
        }
        let snapshots = self.missile_snapshots(launcher);
        let heading_deg = launcher.basis.forward[0]
            .atan2(launcher.basis.forward[2])
            .to_degrees();
        let pitch_deg = launcher.basis.forward[1].clamp(-1., 1.).asin().to_degrees();
        self.missile_threats.observe(
            self.tick,
            super::threats::Receiver {
                id: PLAYER_OWNER,
                position: launcher.position,
                velocity: launcher.velocity,
                heading_deg,
                pitch_deg,
                skill: crate::ai::Experience::Ace,
                rwr_operating: launcher.alive && !self.rwr_failed,
                visual_operating: launcher.alive && !self.visual_failed,
                visibility_limit_ft: Some(5. * missiles::NMI),
            },
            &snapshots,
            |from, to| !obscured(from, to),
        );
        if !self.cheats.ignore_midair_collisions {
            self.midair_collisions(launcher, &mut events);
        }
        events
    }
    /// Aircraft whose paths came within their combined radii this tick
    /// collided. A midair collision destroys every aircraft involved,
    /// Invulnerable or not (John, 2026-09-23). No kill is credited.
    fn midair_collisions(&mut self, launcher: Launcher, events: &mut Vec<Event>) {
        const PLAYER: usize = usize::MAX;
        let mut bodies: Vec<(usize, Vector, Vector, f64)> = self
            .targets
            .iter()
            .enumerate()
            .filter(|(_, t)| t.role == TargetRole::Aircraft && t.airborne && t.hp > 0)
            .map(|(i, t)| (i, t.position, t.velocity, t.radius))
            .collect();
        if launcher.alive && self.player_hp > 0 {
            bodies.push((
                PLAYER,
                launcher.position,
                launcher.velocity,
                AIRCRAFT_RADIUS_FT,
            ));
        }
        // Invulnerable spares the player; whatever it hits is still destroyed.
        let player_spared = self.cheats.invulnerable;
        let mut struck = std::collections::BTreeSet::new();
        let mut blasts = Vec::new();
        for (n, a) in bodies.iter().enumerate() {
            for b in &bodies[n + 1..] {
                let now = sub(a.1, b.1);
                let before = sub(now, sub(a.2, b.2).map(|v| v * crate::flight::DT));
                if segment_sphere(before, now, a.3 + b.3).is_some() {
                    struck.extend([a.0, b.0]);
                    blasts.push(std::array::from_fn(|i| (a.1[i] + b.1[i]) / 2.));
                }
            }
        }
        for index in struck {
            if index == PLAYER && player_spared {
                continue;
            } else if index == PLAYER {
                self.player_hp = 0;
                self.player_damage = self.player_damage.max(self.config.damage_capacity);
                self.release();
                events.push(Event::PlayerDestroyed);
            } else {
                let t = &mut self.targets[index];
                t.hp = 0;
                events.push(Event::Destroyed(t.id));
            }
        }
        for blast in blasts {
            self.effect(blast, EffectKind::Destroyed);
        }
    }
}
fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}

fn segment_box_fraction(from: Vector, to: Vector, lo: Vector, hi: Vector) -> Option<f64> {
    let mut enter: f64 = 0.;
    let mut exit: f64 = 1.;
    for axis in 0..3 {
        let delta = to[axis] - from[axis];
        if delta.abs() < 1e-12 {
            if from[axis] < lo[axis] || from[axis] > hi[axis] {
                return None;
            }
            continue;
        }
        let a = (lo[axis] - from[axis]) / delta;
        let b = (hi[axis] - from[axis]) / delta;
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
        if enter > exit {
            return None;
        }
    }
    Some(enter.max(0.))
}

fn scaled_weapon_damage(w: &Weapon, damage: i32) -> i32 {
    let damage = damage.max(0);
    if is_gun(w) { damage / 3 } else { damage }
}

fn projectile_damage(p: &Projectile, w: &Weapon, damage: i32) -> i32 {
    let total = scaled_weapon_damage(w, damage);
    let Some(round) = p.gun_round else {
        return total;
    };
    let divisor = i32::from(w.burst.actual_rounds_per_game.max(1));
    total / divisor + i32::from(i32::from(round) < total.rem_euclid(divisor))
}

fn critical_hit(target: &Target, weapon: &Weapon, section: DamageSection, damage: i32) -> bool {
    target.role == TargetRole::Aircraft
        && is_gun(weapon)
        && (section == DamageSection::Cockpit
            || (section == DamageSection::Core && damage >= target.initial_hp / 2))
}

pub fn is_gun(w: &Weapon) -> bool {
    AircraftId::ALL
        .into_iter()
        .chain([AircraftId::Faxx])
        .any(|aircraft| w.source.eq_ignore_ascii_case(aircraft.gun()))
}

const GUN_DISPERSION_HALF_ANGLE: f64 = 0.25_f64.to_radians();

fn projectile_launch_direction(
    weapon: &Weapon,
    direction: Vector,
    id: u32,
    owner: u32,
    station: usize,
) -> Vector {
    if !is_gun(weapon) {
        return direction;
    }
    let forward = unit(direction);
    let seed = id
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(owner.rotate_left(13))
        .wrapping_add((station as u32).wrapping_mul(0x85eb_ca6b));
    let azimuth_u = f64::from(mix32(seed ^ 0xa511_e9b3)) / f64::from(u32::MAX);
    let radius_u = f64::from(mix32(seed ^ 0x63d8_3595)) / f64::from(u32::MAX);
    let azimuth = azimuth_u * std::f64::consts::TAU;
    let min_cos = GUN_DISPERSION_HALF_ANGLE.cos();
    let cos_theta = 1. - radius_u * (1. - min_cos);
    let sin_theta = (1. - cos_theta * cos_theta).max(0.).sqrt();
    let reference = if forward[1].abs() < 0.9 {
        [0., 1., 0.]
    } else {
        [1., 0., 0.]
    };
    let right = unit(cross(reference, forward));
    let up = unit(cross(forward, right));
    unit(std::array::from_fn(|i| {
        forward[i] * cos_theta
            + right[i] * sin_theta * azimuth.cos()
            + up[i] * sin_theta * azimuth.sin()
    }))
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}
fn acquisition(
    w: &Weapon,
    position: Vector,
    forward: Vector,
    target: Vector,
    radar: bool,
    zone: usize,
) -> bool {
    // PROJLock checks launcher illumination under flag 0x200. The reviewed
    // default radar stores all require radar at launch; only R530 has 0x200.
    if w.seeker.signature == 3 && !radar && (zone == 1 || w.flags & 0x200 != 0) {
        return false;
    }
    cone(&w.seeker.zones[zone], position, forward, target)
}
fn cone(
    z: &tore_formats::weapons::Zone,
    position: Vector,
    forward: Vector,
    target: Vector,
) -> bool {
    zone_readiness(z, position, forward, target) == Readiness::Ready
}
fn zone_readiness(
    z: &tore_formats::weapons::Zone,
    position: Vector,
    forward: Vector,
    target: Vector,
) -> Readiness {
    let d = sub(target, position);
    let distance = dot(d, d).sqrt();
    let angle = dot(unit(d), forward).clamp(-1., 1.).acos();
    if distance < f64::from(z.minimum_range) {
        Readiness::MinimumRange
    } else if distance > f64::from(z.maximum_range) {
        Readiness::MaximumRange
    } else if d[1] < f64::from(z.minimum_altitude) || d[1] > f64::from(z.maximum_altitude) {
        Readiness::Altitude
    } else if angle > f64::from(z.heading.min(z.pitch).max(0)) * std::f64::consts::TAU / 65520. {
        Readiness::FieldOfView
    } else {
        Readiness::Ready
    }
}

pub fn segment_sphere(a: Vector, b: Vector, radius: f64) -> Option<f64> {
    let d = sub(b, a);
    let c = dot(a, a) - radius * radius;
    if c <= 0. {
        return Some(0.);
    }
    let aa = dot(d, d);
    let bb = dot(a, d);
    let disc = bb * bb - aa * c;
    if aa <= 1e-15 || disc < 0. {
        return None;
    }
    let t = (-bb - disc.sqrt()) / aa;
    (0. ..=1.).contains(&t).then_some(t)
}
pub(crate) fn terrain_hit(a: Vector, b: Vector, ground: &impl Fn(f64, f64) -> f64) -> Option<f64> {
    let below = |t: f64| {
        let p: Vector = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t);
        p[1] <= ground(p[0], p[2])
    };
    if below(0.) {
        return Some(0.);
    }
    for i in 1..=8 {
        let mut high = f64::from(i) / 8.;
        if below(high) {
            let mut low = f64::from(i - 1) / 8.;
            for _ in 0..10 {
                let mid = (low + high) * 0.5;
                if below(mid) {
                    high = mid;
                } else {
                    low = mid;
                }
            }
            return Some(high);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::weapons::*;
    use tore_formats::weapons::{Guidance, Seeker};
    pub(super) fn fixture(guided: bool) -> State {
        let zone = Zone {
            heading: 12000,
            pitch: 12000,
            minimum_range: 0,
            maximum_range: 10000,
            minimum_altitude: i32::MIN,
            maximum_altitude: i32::MAX,
        };
        let seeker = Seeker {
            flags: [0; 2],
            signature: if guided { 3 } else { 0 },
            look_down: 0,
            doppler_above: 0,
            doppler_below: 0,
            doppler_minimum_range: 0,
            all_aspect: 0,
            zones: [zone; 2],
            chaff_flare_chance: 0,
            deception_chance: 0,
        };
        let w = Weapon {
            source: "SYNTHETIC.JT".into(),
            name: "Synthetic".into(),
            hud_name: "SYN".into(),
            shape: None,
            fire_sound: None,
            native_callback: "_PROJProc".into(),
            flags: if guided { 0x240 } else { 0x844 },
            object_flags: 0,
            weight: 10,
            movement: Movement {
                minimum_speed: 10,
                corner_speed: 1000,
                maximum_speed: 2000,
                acceleration: 100,
                deceleration: 2,
                initial_speed: 1000,
                final_speed: 500,
                launch_retard: 100,
                ignite_t: 0,
                fuel_t: 10,
                remove_t: 20,
                powered_turn_rate: 10000,
                unpowered_turn_rate: 10000,
                performance_at_0: 100,
                performance_at_20: 100,
                cruise: [0; 4],
                jink: [0; 3],
            },
            burst: Burst {
                projectiles_in_pod: 1,
                actual_rounds_per_game: 2,
                game_rounds_in_burst: 1,
                game_rounds_in_carpet_burst: 1,
                game_burst_t: 1,
                reload_t: 0,
                startup_shots: 0,
                random_fire_percent: 0,
                offset_fire_percent: 0,
                offset_fire_heading: 0,
                offset_fire_pitch: 0,
                sine_pattern: [0; 4],
            },
            seeker,
            guidance: Guidance {
                track_t: 1,
                track_max_g_raw: 1,
                target_sun_chance: 0,
                max_aon: 0,
                chances: [100; 4],
                hit_modifiers: [0; 9],
            },
            damage: Damage {
                by_class: [10; 5],
                fuze_arm_t: 0,
                fuze_radius: 0,
                side_hit_fuze_failure: 0,
                collateral_radius: 0,
                collateral_percent: 0,
            },
            effects: Effects {
                object_explosion: 0,
                land_explosion: 0,
                water_explosion: 0,
                crater_size: 0,
                smoke: [0; 5],
                max_sound_distance: 0,
                frequency_adjustment: 0,
            },
        };
        State::new(
            Configuration {
                fragment_offsets: [[0.; 3]; 2],
                ecm: tore_formats::weapons::Countermeasures {
                    weight: 0,
                    flags: 0,
                    mode_flags: 0x10,
                    chaff: [0; 4],
                    flare: [0; 4],
                    radar_deception_chance: 30,
                    radar_signature_add: 0,
                    radar_noise_range: [0; 2],
                    infrared_deception_chance: 0,
                    infrared_signature_add: 0,
                    infrared_lose_lock_time: 0,
                },
                system_damage: [0x11; 45],
                damage_capacity: 30,
                afterburner_available: true,
                hardpoint_slots: vec![Some(0)],
                radar_hardpoint: 1,
                visual_hardpoint: 3,
                ecm_hardpoint: 2,
                aircraft: AircraftId::F18,
                stations: vec![Station {
                    weapon: w,
                    mount: [0.; 3],
                    count: 11,
                    internal: !guided,
                }],
                hit_points: 20,
                target_category: 0x80,
                external_equipment_lbs: 0,
                external_fuel_lbs: [0.; 9],
                engines: 1,
                wreck_power: crate::wreck::Power::default(),
                infrared_hardpoint: None,
                rwr_hardpoint: None,
                sensors: sensor_profiles(),
            },
            true,
        )
        .unwrap()
    }

    #[test]
    fn player_wreck_keeps_smoking_until_impact_or_airburst_then_puffs_fade() {
        use super::super::smoke::Kind;
        let mut s = fixture(false);
        s.player_hp = 0;
        let mut l = launcher();
        l.alive = false;
        for tick in 0..120 {
            l.position[1] = 5000. - f64::from(tick);
            s.step(false, l, |_, _| 0.);
        }
        assert_eq!(s.smoke.puffs.len(), 10);
        assert!(s.smoke.puffs.iter().all(|p| p.kind == Kind::Aircraft));
        let mut airburst = s.clone();
        airburst.player_airburst(l.position);
        s.player_ground_impact([l.position[0], 0., l.position[2]]);
        for _ in 0..120 {
            s.step(false, l, |_, _| 0.);
            airburst.step(false, l, |_, _| 0.);
        }
        for state in [&s, &airburst] {
            assert_eq!(state.smoke.puffs.len(), 10);
            assert!(state.smoke.puffs.iter().all(|p| p.age >= 120));
        }
        for _ in 0..960 {
            s.step(false, l, |_, _| 0.);
        }
        assert!(s.smoke.puffs.is_empty());
        let mut grounded = fixture(false);
        grounded.player_hp = 0;
        l.position[1] = 0.;
        for _ in 0..24 {
            grounded.step(false, l, |_, _| 0.);
        }
        assert!(grounded.smoke.puffs.is_empty());
    }

    #[test]
    fn sound_emissions_retain_exact_impact_positions_and_are_consumed_once() {
        use crate::acoustics::Kind;
        let mut s = fixture(false);
        s.effect([100., 200., 300.], EffectKind::Ground);
        s.effect([-100., 400., 900.], EffectKind::Destroyed);
        s.effect([100., 200., 300.], EffectKind::Ground);
        let sounds = s.take_sound_events();
        assert_eq!(sounds.len(), 3);
        assert_eq!(sounds[0].position, [100., 200., 300.]);
        assert_eq!(sounds[0].kind, Kind::Impact);
        assert_eq!(sounds[1].position, [-100., 400., 900.]);
        assert_eq!(sounds[1].kind, Kind::Explosion);
        assert!(s.take_sound_events().is_empty());
        // Clearing short-lived visuals cannot erase an already emitted wave.
        s.effects.clear();
        assert_eq!(sounds.len(), 3);
        for _ in 0..1000 {
            s.effect([0.; 3], EffectKind::Hit);
        }
        assert_eq!(s.take_sound_events().len(), 256);
    }
    #[test]
    fn incoming_cockpit_hit_reports_pilot_death_without_needing_nose_breakup() {
        let mut s = fixture(false);
        // Synthetic gun data uses the exact reviewed gun identity for contact classification.
        s.config.stations[0].weapon.source = AircraftId::F18.gun().into();
        let l = launcher();
        s.command(Command::Incoming, l);
        let p = s.projectiles.last_mut().unwrap();
        p.position = std::array::from_fn(|i| {
            l.position[i] + l.basis.forward[i] * 100. + l.basis.up[i] * 11.
        });
        p.previous = p.position;
        p.age = 1;
        let mut events = Vec::new();
        for _ in 0..120 {
            events.extend(s.step(false, l, |_, _| 0.));
            if s.player_hp == 0 {
                break;
            }
        }
        assert!(events.contains(&Event::PilotKilled));
        assert!(events.contains(&Event::PlayerDestroyed));
        assert_eq!(s.player_damage_section(), None);
    }
    #[test]
    fn player_impact_explosion_cleans_up_once_and_excludes_later_airbursts() {
        let mut s = fixture(false);
        s.debris.push(super::super::debris::Piece::new(
            0,
            0,
            [0., 1., 0.],
            [0.; 3],
            Basis::new(0., 0., 0.),
            [0.; 3],
        ));
        assert_eq!(
            s.player_ground_impact([0., 0., 0.]),
            Some(Event::PlayerGroundImpact)
        );
        assert!(s.debris.is_empty());
        assert_eq!(
            s.effects
                .iter()
                .filter(|e| e.kind == EffectKind::Destroyed)
                .count(),
            1
        );
        assert_eq!(s.player_ground_impact([0., 0., 0.]), None);
        assert_eq!(s.player_airburst([0., 0., 0.]), None);
    }
    #[test]
    fn wreck_airburst_removes_target_and_fragments_without_awarding_another_kill() {
        let mut s = fixture(false);
        let mut aircraft = target(77, [0., 100000., 2000.], 100, 0x80);
        aircraft.hp = 0;
        aircraft.wreck_power = crate::wreck::Power::symmetric(2, 20., 10.);
        s.targets.push(aircraft);
        // Search deterministic destruction ticks rather than depending on global combat RNG.
        let born = (0..1000)
            .find(|born| {
                let mut w = crate::wreck::Wreck::new(77, *born, [0.; 3]);
                let (mut p, mut v, mut b) = ([0., 100000., 0.], [0.; 3], Basis::new(0., 0., 0.));
                for _ in 0..120 {
                    w.step(&mut p, &mut v, &mut b, |_, _| 0.);
                }
                w.phase == crate::wreck::Phase::Exploded
            })
            .unwrap();
        s.tick = born.saturating_sub(1);
        s.targets[0].wreck = Some(crate::wreck::Wreck::new(77, born, [0.; 3]));
        s.debris.push(super::super::debris::Piece::new(
            77,
            0,
            s.targets[0].position,
            [0.; 3],
            s.targets[0].basis,
            [0.; 3],
        ));
        let mut bursts = 0;
        for _ in 0..240 {
            bursts += s
                .step(false, launcher(), |_, _| 0.)
                .iter()
                .filter(|e| **e == Event::Airburst(77))
                .count();
        }
        assert_eq!(bursts, 1);
        assert!(!s.targets[0].airborne);
        assert_eq!(s.kills, 0);
        assert!(s.debris.iter().all(|p| p.owner != 77));
        assert!(s.effects.iter().any(|e| e.kind == EffectKind::Destroyed));
        assert_eq!(s.player_airburst([0., 5000., 0.]), Some(Event::Airburst(0)));
        assert_eq!(s.player_airburst([0., 5000., 0.]), None);
    }
    #[test]
    fn heavy_enemy_hit_degrades_components_without_detaching_a_live_nose() {
        let mut s = fixture(false);
        s.config.damage_capacity = 100;
        s.player_hp = 100;
        s.config.system_damage = [0; 45];
        for index in [19, 5, 14, 12] {
            s.config.system_damage[index] = 0x11;
        }
        let mut events = Vec::new();
        // A real incoming projectile sweeps the ownship nose. The zero seed
        // fixes the damage draw at its lower boundary for this synthetic test.
        let raw = if is_gun(&s.config.stations[0].weapon) {
            345
        } else {
            115
        };
        s.config.stations[0].weapon.damage.by_class = [raw; 5];
        s.rng = 0;
        s.command(Command::Incoming, launcher());
        for _ in 0..720 {
            events.extend(s.step(false, launcher(), |_, _| 0.));
            if s.player_hp < 100 {
                break;
            }
        }
        assert_eq!(s.player_hp, 8);
        let mut components = crate::aircraft_systems::Systems::default();
        for event in events {
            if let Event::SubsystemDamaged(index) = event {
                components.hit(index, 1.);
            }
        }
        for index in [19, 5, 14, 12] {
            assert_eq!(s.subsystem_counts[index], 1);
        }
        assert_eq!(components.power_available(), 0.75);
        assert_eq!(components.oil_pressure(), 0.5);
        assert_eq!(components.controls([1., 0., 0.], [0.; 3], 0)[0], 0.5);
        for _ in 0..120 {
            components.advance(true, 1., 1., 0.92, false, &mut 100.);
        }
        assert!(components.fluids.hydraulic < 1.);
        assert!(components.engine.temperature > 0.);
        s.step(false, launcher(), |_, _| 0.);
        assert!(s.debris.is_empty());
        s.apply_player_damage(7, &mut Vec::new());
        s.step(false, launcher(), |_, _| 0.);
        assert_eq!(s.player_hp, 1);
        assert!(s.debris.is_empty());
        s.apply_player_damage(1, &mut Vec::new());
        s.step(false, launcher(), |_, _| 0.);
        assert_eq!(s.player_hp, 0);
        assert_eq!(s.debris.len(), 1);
        s.step(false, launcher(), |_, _| 0.);
        assert_eq!(s.debris.len(), 1);
    }
    #[test]
    fn rwr_damage_is_a_receiver_fault_and_systems_destruction_is_once_only() {
        let mut s = fixture(false);
        s.config.rwr_hardpoint = Some(4);
        s.config.system_damage = [0; 45];
        s.config.system_damage[40] = 0x1f;
        s.player_hp = 10000;
        let mut events = Vec::new();
        for _ in 0..30 {
            s.apply_player_damage(4, &mut events);
        }
        assert_eq!(s.subsystem_counts[40], 1);
        assert!(s.rwr_failed);
        assert!(!s.radar_failed);
        let reset = State::new(s.config.clone(), true).unwrap();
        assert!(!reset.rwr_failed);
        assert_eq!(s.systems_destroyed(), Some(Event::PlayerDestroyed));
        assert_eq!(s.systems_destroyed(), None);
        assert_eq!(s.player_hp, 0);
    }
    #[test]
    fn player_selection_wraps_through_nav_and_arms_only_weapons() {
        let initial = fixture(false);
        let mut config = initial.configuration().clone();
        config.stations.push(config.stations[0].clone());
        let mut state = State::new(config, true).unwrap();
        let l = launcher();
        state.command(Command::SelectNav, l);
        assert!(!state.armed);
        assert!(
            !state
                .step(true, l, |_, _| 0.)
                .iter()
                .any(|e| matches!(e, Event::Fired(_)))
        );
        for (command, selected, armed) in [
            (Command::NextSelection, 0, true),
            (Command::NextSelection, 1, true),
            (Command::NextSelection, 1, false),
            (Command::PreviousSelection, 1, true),
            (Command::PreviousSelection, 0, true),
            (Command::PreviousSelection, 0, false),
        ] {
            state.command(command, l);
            assert_eq!((state.selected, state.armed), (selected, armed));
        }
    }

    #[test]
    fn localized_sections_follow_aircraft_basis_and_accumulate_before_breakup() {
        let mut target = target(1, [10., 20., 30.], 100, 0x80);
        target.basis = Basis::new(std::f64::consts::FRAC_PI_2, 0., 0.);
        let nose = std::array::from_fn(|i| target.position[i] + target.basis.forward[i] * 20.);
        let left = std::array::from_fn(|i| target.position[i] - target.basis.right[i] * 20.);
        let tail = std::array::from_fn(|i| target.position[i] - target.basis.forward[i] * 20.);
        assert_eq!(LocalizedDamage::section(nose, &target), DamageSection::Nose);
        assert_eq!(
            LocalizedDamage::section(left, &target),
            DamageSection::LeftWing
        );
        assert_eq!(LocalizedDamage::section(tail, &target), DamageSection::Tail);
        let cockpit_center: Vector = std::array::from_fn(|i| {
            target.position[i] + target.basis.up[i] * 6. + target.basis.forward[i] * 7.
        });
        let cockpit_from = std::array::from_fn(|i| cockpit_center[i] + target.basis.right[i] * 10.);
        let cockpit_to = std::array::from_fn(|i| cockpit_center[i] - target.basis.right[i] * 10.);
        assert_eq!(
            LocalizedDamage::section_segment(cockpit_from, cockpit_to, &target),
            DamageSection::Cockpit
        );
        let movement = [40., -12., 25.];
        let old_projectile: Vector = std::array::from_fn(|i| cockpit_from[i] - movement[i]);
        let relative_previous: Vector = std::array::from_fn(|i| old_projectile[i] + movement[i]);
        assert_eq!(
            LocalizedDamage::contact(relative_previous, cockpit_to, &target, 1.).map(|v| v.1),
            Some(DamageSection::Cockpit)
        );
        let core_from = std::array::from_fn(|i| target.position[i] - target.basis.right[i] * 5.);
        let core_to = std::array::from_fn(|i| target.position[i] + target.basis.right[i] * 5.);
        assert_eq!(
            LocalizedDamage::section_segment(core_from, core_to, &target),
            DamageSection::Core
        );
        let first_from: Vector =
            std::array::from_fn(|i| cockpit_center[i] + target.basis.right[i] * 20.);
        let first_to: Vector =
            std::array::from_fn(|i| cockpit_center[i] + target.basis.right[i] * 10.);
        let second_to: Vector =
            std::array::from_fn(|i| cockpit_center[i] - target.basis.right[i] * 10.);
        assert_eq!(
            LocalizedDamage::contact(first_from, first_to, &target, 1.),
            None
        );
        assert_eq!(
            LocalizedDamage::contact(first_to, second_to, &target, 1.).map(|v| v.1),
            Some(DamageSection::Cockpit)
        );
        target.localized_damage.record(DamageSection::Nose, 74, 100);
        assert_eq!(target.localized_damage.structural_variant, None);
        target.localized_damage.record(DamageSection::Nose, 1, 100);
        assert_eq!(target.localized_damage.structural_variant, Some(0));
        assert_eq!(
            target.localized_damage.structural_section,
            Some(DamageSection::Nose)
        );
        assert_eq!(target.localized_damage.fractions(100)[0], 0.75);
    }

    #[test]
    fn a_dropped_target_leaves_the_hud_unless_easy_targeting_keeps_it() {
        let mut state = fixture(true);
        let ownship = launcher();
        state.range_target(ownship);
        for _ in 0..120 {
            state.step(false, ownship, |_, _| 0.);
        }
        state.designate_next(true);
        let id = state.designated().expect("fixture contact");
        state
            .targets
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap()
            .position = [0., 1000., -5000.];
        for _ in 0..120 {
            state.step(false, ownship, |_, _| 0.);
        }
        assert_eq!(state.designated(), None);
        assert!(state.display_target().is_none());
        // Turning Easy targeting on later does not bring it back.
        state.cheats.easy_targeting = true;
        state.step(false, ownship, |_, _| 0.);
        assert!(state.display_target().is_none());
    }
    #[test]
    fn easy_targeting_keeps_the_selection_off_scope_without_weapon_support() {
        let mut state = fixture(true);
        state.cheats.easy_targeting = true;
        let ownship = launcher();
        state.range_target(ownship);
        for _ in 0..120 {
            state.step(false, ownship, |_, _| 0.);
        }
        state.designate_next(true);
        let id = state.designated().expect("fixture contact");
        assert_eq!(state.display_target().map(|t| t.id), Some(id));
        state
            .targets
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap()
            .position = [0., 1000., -5000.];
        for _ in 0..120 {
            state.step(false, ownship, |_, _| 0.);
        }
        assert_eq!(state.designated(), Some(id), "radar keeps it set");
        assert!(state.sensors.contact(id).is_none());
        assert!(state.weapon_observation(ownship).is_none());
        assert_eq!(state.display_target().map(|t| t.id), Some(id));
        state.targets.iter_mut().find(|t| t.id == id).unwrap().hp = 0;
        assert!(state.display_target().is_none());
        state.targets.iter_mut().find(|t| t.id == id).unwrap().hp = 10;
        state.command(Command::ClearDesignation, ownship);
        assert!(state.display_target().is_none());
    }

    #[test]
    fn guns_take_exact_integer_third_while_missiles_keep_damage() {
        let mut gun = fixture(false).configuration().stations[0].weapon.clone();
        gun.source = "M61.JT".into();
        assert_eq!(scaled_weapon_damage(&gun, 11), 3);
        assert_eq!(scaled_weapon_damage(&gun, 2), 0);
        let missile = fixture(true).configuration().stations[0].weapon.clone();
        assert_eq!(scaled_weapon_damage(&missile, 11), 11);
        let aircraft = target(1, [0.; 3], 20, 0x80);
        let mut surface = aircraft.clone();
        surface.role = TargetRole::Surface;
        assert!(critical_hit(&aircraft, &gun, DamageSection::Cockpit, 1));
        assert!(!critical_hit(&surface, &gun, DamageSection::Cockpit, 20));
    }
    #[test]
    fn gun_dispersion_is_bounded_normalized_symmetric_and_deterministic() {
        let mut gun = fixture(false).configuration().stations[0].weapon.clone();
        gun.source = "M61.JT".into();
        let forward = [0., 0., 1.];
        let first = projectile_launch_direction(&gun, forward, 42, 7, 0);
        assert_eq!(first, projectile_launch_direction(&gun, forward, 42, 7, 0));
        let mut mean = [0.; 3];
        let samples = 20_000;
        for id in 0..samples {
            let direction = projectile_launch_direction(&gun, forward, id, 7, 0);
            let length = dot(direction, direction).sqrt();
            let angle = dot(direction, forward).clamp(-1., 1.).acos();
            assert!((length - 1.).abs() < 1e-12);
            assert!(angle <= GUN_DISPERSION_HALF_ANGLE + 1e-12);
            for axis in 0..3 {
                mean[axis] += direction[axis] / f64::from(samples);
            }
        }
        assert!(mean[0].abs() < 5e-5, "lateral bias {}", mean[0]);
        assert!(mean[1].abs() < 5e-5, "vertical bias {}", mean[1]);
        let expected_cos = (1. + GUN_DISPERSION_HALF_ANGLE.cos()) * 0.5;
        assert!((mean[2] - expected_cos).abs() < 2e-7);
        let missile = fixture(true).configuration().stations[0].weapon.clone();
        assert_eq!(
            projectile_launch_direction(&missile, forward, 42, 7, 0),
            forward
        );
    }

    #[test]
    fn live_gun_release_applies_dispersion_once() {
        let mut s = fixture(false);
        s.config.stations[0].weapon.source = "M61.JT".into();
        let launcher = launcher();
        s.step(true, launcher, |_, _| 0.);
        let projectile = s.projectiles.first().expect("gun round was not released");
        let angle = dot(projectile.direction, launcher.basis.forward)
            .clamp(-1., 1.)
            .acos();
        assert!(angle > 0. && angle <= GUN_DISPERSION_HALF_ANGLE + 1e-12);
        let direction = projectile.direction;
        s.step(false, launcher, |_, _| 0.);
        assert_eq!(s.projectiles[0].direction, direction);
    }

    #[test]
    fn physical_gun_rounds_are_evenly_paced_and_preserve_ammo_rate() {
        let mut s = fixture(false);
        let w = &mut s.config.stations[0].weapon;
        w.source = "M61.JT".into();
        w.burst.actual_rounds_per_game = 2;
        w.burst.game_rounds_in_burst = 4;
        w.burst.game_burst_t = 1;
        s.ammo[0] = 1000;
        let launcher = launcher();
        let mut fired_ticks = Vec::new();
        let mut tracers = 0;
        for tick in 0..120 {
            let events = s.step(true, launcher, |_, _| -10000.);
            let fired = events
                .iter()
                .filter(|event| matches!(event, Event::Fired(0)))
                .count();
            assert!(fired <= 1, "gun emitted simultaneous rounds at tick {tick}");
            if fired == 1 {
                fired_ticks.push(tick);
                tracers += usize::from(s.projectiles.last().unwrap().tracer);
            }
        }
        assert_eq!(fired_ticks.len(), 32);
        assert_eq!(s.rounds(0), 968);
        assert_eq!(tracers, 11);
        assert!(
            fired_ticks
                .windows(2)
                .all(|pair| (3..=4).contains(&(pair[1] - pair[0])))
        );
    }

    #[test]
    fn gun_cadence_keeps_fractional_rate_and_repress_phase() {
        let mut s = fixture(false);
        let w = &mut s.config.stations[0].weapon;
        w.source = "M61.JT".into();
        w.burst.actual_rounds_per_game = 3;
        w.burst.game_rounds_in_burst = 7;
        w.burst.game_burst_t = 2;
        s.ammo[0] = 2000;
        let launcher = launcher();
        let mut fired = 0;
        for _ in 0..1200 {
            let events = s.step(true, launcher, |_, _| -10000.);
            let count = events
                .iter()
                .filter(|event| matches!(event, Event::Fired(0)))
                .count();
            assert!(count <= 1);
            fired += count;
        }
        assert_eq!(fired, 420);
        assert_eq!(s.rounds(0), 2000 - fired as u16);
        assert_eq!(s.gun_cadence[0].ordinal, fired as u64);

        let before = s.shots;
        s.release();
        for _ in 0..1 {
            assert!(
                !s.step(false, launcher, |_, _| -10000.)
                    .iter()
                    .any(|event| matches!(event, Event::Fired(0)))
            );
        }
        let first = (0..120)
            .find(|_| {
                s.step(true, launcher, |_, _| -10000.)
                    .iter()
                    .any(|event| matches!(event, Event::Fired(0)))
            })
            .unwrap();
        assert!(first < 4);
        assert_eq!(s.shots, before + 1);

        // A release shorter than the physical shot gap cannot accelerate fire.
        let mut s = fixture(false);
        let w = &mut s.config.stations[0].weapon;
        w.source = "M61.JT".into();
        w.burst.actual_rounds_per_game = 2;
        w.burst.game_rounds_in_burst = 4;
        w.burst.game_burst_t = 1;
        assert!(
            s.step(true, launcher, |_, _| -10000.)
                .contains(&Event::Fired(0))
        );
        s.release();
        s.step(false, launcher, |_, _| -10000.);
        for _ in 0..2 {
            assert!(
                !s.step(true, launcher, |_, _| -10000.)
                    .contains(&Event::Fired(0))
            );
        }
        assert!(
            s.step(true, launcher, |_, _| -10000.)
                .contains(&Event::Fired(0))
        );
    }

    #[test]
    fn physical_round_damage_partitions_without_rounding_inflation() {
        let mut s = fixture(false);
        let w = &mut s.config.stations[0].weapon;
        w.source = "M61.JT".into();
        w.burst.actual_rounds_per_game = 2;
        let mut p = Projectile {
            id: 0,
            owner: PLAYER_OWNER,
            weapon: None,
            guidance: None,
            motion: None,
            guidance_ticks: None,
            age: 0,
            incoming: false,
            station: 0,
            position: [0.; 3],
            previous: [0.; 3],
            direction: [0., 0., 1.],
            speed_f8: 0,
            launched_t: 0,
            target: None,
            fall: FallState::default(),
            gun_round: Some(0),
            tracer: true,
        };
        assert_eq!(projectile_damage(&p, w, 10), 2);
        p.gun_round = Some(1);
        assert_eq!(projectile_damage(&p, w, 10), 1);
        assert_eq!(projectile_damage(&p, w, 2), 0);
    }

    #[test]
    fn station_cycle_discards_queued_gun_rounds() {
        let mut s = fixture(false);
        s.config.stations[0].weapon.source = "M61.JT".into();
        s.config.stations[0].weapon.burst.game_rounds_in_burst = 4;
        s.step(true, launcher(), |_, _| -10000.);
        assert!(s.gun_cadence[0].pending > 0);
        s.select_next();
        assert_eq!(s.gun_cadence[0].pending, 0);
    }

    #[test]
    fn every_supported_aircraft_uses_its_canonical_gun_damage_and_dispersion() {
        let aircraft = AircraftId::ALL
            .into_iter()
            .chain([AircraftId::Faxx])
            .collect::<Vec<_>>();
        assert_eq!(aircraft.len(), 14);
        let launcher = launcher();
        for (number, id) in aircraft.into_iter().enumerate() {
            let mut state = fixture(false);
            state.config.aircraft = id;
            state.config.stations[0].weapon.source = id.gun().into();
            let gun = &state.config.stations[0].weapon;
            assert!(is_gun(gun), "{} gun was not recognized", id.label());
            assert_eq!(scaled_weapon_damage(gun, 11), 3, "{} damage", id.label());
            let expected = projectile_launch_direction(
                gun,
                launcher.basis.forward,
                number as u32,
                PLAYER_OWNER,
                0,
            );
            assert_eq!(
                expected,
                projectile_launch_direction(
                    gun,
                    launcher.basis.forward,
                    number as u32,
                    PLAYER_OWNER,
                    0,
                ),
                "{} deterministic direction",
                id.label()
            );
            let angle = dot(expected, launcher.basis.forward).clamp(-1., 1.).acos();
            assert!(
                angle <= GUN_DISPERSION_HALF_ANGLE + 1e-12,
                "{} dispersion angle {angle}",
                id.label()
            );
            state.shots = number as u32;
            state.step(true, launcher, |_, _| 0.);
            assert_eq!(
                state.projectiles[0].direction,
                expected,
                "{} live gun path",
                id.label()
            );
        }
        let mut missile = fixture(true).configuration().stations[0].weapon.clone();
        missile.source = "AIM120.JT".into();
        assert!(!is_gun(&missile));
        assert_eq!(scaled_weapon_damage(&missile, 11), 11);
        assert_eq!(
            projectile_launch_direction(&missile, launcher.basis.forward, 9, 4, 1),
            launcher.basis.forward
        );
    }
    #[test]
    fn live_gun_crosses_narrowphase_before_cockpit_kill_without_structural_loss() {
        let mut s = fixture(false);
        s.config.stations[0].weapon.source = "M61.JT".into();
        s.config.stations[0].weapon.damage.by_class[0] = 6;
        s.targets.clear();
        let target_position = [0., 1000., 300.];
        s.targets.push(target(9, target_position, 20, 0x80));
        let position = [10., target_position[1] + 6., target_position[2] + 7.];
        s.projectiles.push(Projectile {
            id: 99,
            owner: PLAYER_OWNER,
            weapon: None,
            guidance: None,
            motion: None,
            guidance_ticks: None,
            age: 0,
            incoming: false,
            station: 0,
            position,
            previous: position,
            direction: [-1., 0., 0.],
            speed_f8: 1200 * 256,
            launched_t: 0,
            target: None,
            fall: FallState::default(),
            gun_round: None,
            tracer: false,
        });
        let events = s.step(false, launcher(), |_, _| 0.);
        assert!(events.contains(&Event::Destroyed(9)));
        assert_eq!(
            s.targets[0].localized_damage.amounts[DamageSection::Cockpit as usize],
            2
        );
        assert_eq!(s.targets[0].localized_damage.structural_section, None);
    }
    #[test]
    fn preflight_transfers_conserve_counts_and_respect_capacity_steps() {
        use crate::combat::loadout::Loadout;
        use tore_formats::aircraft::Hardpoint;
        for (capacity, expected_step) in [(4, 1), (100, 1), (101, 10), (300, 10), (301, 100)] {
            let mut configuration = fixture(true).configuration().clone();
            configuration
                .stations
                .push(configuration.stations[0].clone());
            let hardpoint = Hardpoint {
                location: 4,
                flags: 8,
                position: [0; 3],
                store: Some(configuration.stations[0].weapon.source.clone()),
                count: capacity,
                weight_class: 0,
            };
            let mut load = Loadout {
                aircraft: AircraftId::F18,
                configuration,
                quantities: vec![capacity as u16, 0],
                fuel_lbs: 0.,
                internal_capacity_lbs: 1000.,
                empty_lbs: 10000.,
                maximum_lbs: 20000.,
                cheat: false,
                hardpoints: vec![hardpoint.clone(), hardpoint],
            };
            load.transfer(0, 1).unwrap();
            assert_eq!(
                load.quantities,
                [capacity as u16 - expected_step, expected_step]
            );
            load.quantities = vec![2, capacity as u16 - 1];
            load.transfer(0, 1).unwrap();
            assert_eq!(load.quantities, [1, capacity as u16]);
            load.transfer(0, 1).unwrap();
            load.transfer(0, 0).unwrap();
            assert_eq!(load.quantities, [1, capacity as u16]);
            load.quantities[1] = 0;
            load.transfer(0, 1).unwrap();
            assert_eq!(load.quantities, [0, 1]);
            load.transfer(0, 1).unwrap();
            assert_eq!(load.quantities, [0, 1]);
            load.quantities[0] = 2;
            load.hardpoints[1].store = Some("OTHER.JT".into());
            assert!(load.transfer(0, 1).is_err());
            assert_eq!(load.quantities, [2, 1]);
            load.hardpoints[1].store = load.hardpoints[0].store.clone();
            load.configuration.stations[1].weapon.source = "REPLACED.JT".into();
            load.transfer(0, 1).unwrap();
            assert_eq!(
                load.quantities,
                [2 - expected_step.min(2), expected_step.min(2)]
            );
            assert_eq!(
                load.configuration.stations[1].weapon.source,
                load.configuration.stations[0].weapon.source
            );
        }
    }
    #[test]
    fn guns_only_removes_internal_bay_and_external_weapons_without_refilling_guns() {
        use crate::combat::loadout::Loadout;
        for aircraft in AircraftId::SELECTABLE {
            let mut configuration = fixture(true).configuration().clone();
            configuration.aircraft = aircraft;
            configuration.stations[0].weapon.source = "AIM120.JT".into();
            let mut bay = configuration.stations[0].clone();
            bay.internal = true;
            let mut gun = bay.clone();
            gun.weapon.source = aircraft.gun().into();
            configuration.stations.extend([bay, gun]);
            let mut load = Loadout {
                aircraft,
                configuration,
                quantities: vec![2, 4, 7],
                fuel_lbs: 1000.,
                internal_capacity_lbs: 1000.,
                empty_lbs: 10000.,
                maximum_lbs: 20000.,
                cheat: false,
                hardpoints: vec![],
            };
            load.restrict_to_guns();
            assert_eq!(load.quantities, [0, 0, 7]);
            // Configuration retains valid station capacities; accepted ammo
            // is stored separately and restored by the host on restart.
            assert!(load.configuration.stations.iter().all(|s| s.count == 11));
            load.restrict_to_guns();
            assert_eq!(load.quantities, [0, 0, 7]);
        }
    }
    #[test]
    fn preflight_draft_capacity_fuel_mass_and_clone_isolation() {
        use crate::combat::loadout::Loadout;
        use tore_formats::aircraft::Hardpoint;
        let mut config = fixture(false).configuration().clone();
        config.stations[0].weapon.source = "M61.JT".into();
        let mut load = Loadout {
            aircraft: AircraftId::F18,
            configuration: config,
            quantities: vec![11],
            fuel_lbs: 900.,
            internal_capacity_lbs: 1000.,
            empty_lbs: 10000.,
            maximum_lbs: 11000.,
            cheat: false,
            hardpoints: vec![Hardpoint {
                location: 2,
                flags: 8,
                position: [0; 3],
                store: Some("M61.JT".into()),
                count: 1000,
                weight_class: 0,
            }],
        };
        let original = load.clone();
        load.change(0, 1);
        assert_eq!(load.quantities[0], 111);
        load.fuel(true);
        assert_eq!(load.fuel_lbs, 1000.);
        assert!(load.validate().is_ok());
        load.fuel(false);
        load.fuel(false);
        load.fuel(false);
        assert_eq!(load.fuel_lbs, 0.);
        assert_eq!(original.quantities[0], 11);
        assert_eq!(original.fuel_lbs, 900.);
        let mut other = load.configuration.stations[0].weapon.clone();
        other.source = "OTHER.JT".into();
        assert!(load.select(0, other).is_err());
        assert_eq!(load.quantities[0], 111);
        load.fuel_lbs = f64::NAN;
        assert!(load.validate().is_err());
        load.fuel_lbs = 1000.;
        load.maximum_lbs = 10999.;
        assert!(load.validate().is_err());
        load.maximum_lbs = 11000.;
        load.quantities[0] = 1001;
        assert!(load.validate().is_err());
    }
    /// Synthetic sensor suite: a 90/50 nmi radar shape so the default 10-mile
    /// display selects TWS, plus the short visual channel every aircraft has.
    fn sensor_profiles() -> sensors::SensorProfiles {
        let volume = |nmi: f64| sensors::Volume {
            azimuth_rad: 1.,
            elevation_rad: 1.,
            minimum_ft: 0.,
            maximum_ft: nmi * sensors::FEET_PER_NAUTICAL_MILE,
            minimum_relative_ft: f64::NEG_INFINITY,
            maximum_relative_ft: f64::INFINITY,
        };
        sensors::SensorProfiles {
            aircraft: AircraftId::F18,
            radar: Some(sensors::RadarProfile {
                record: "SYNTHETIC.SEE".into(),
                search: volume(90.),
                track: volume(50.),
                look_down: 0.,
                preset: sensors::Preset::Advanced,
                notch: sensors::Preset::Advanced.notch(),
                resistance: sensors::Preset::Advanced.resistance(),
                band: 0,
                source_flags: [0; 2],
                source_doppler: [0; 3],
            }),
            infrared: None,
            visual: Some(sensors::profile::VisualProfile {
                record: "SYNTHETIC.VIS".into(),
                search: volume(10.),
                track: volume(5.),
            }),
            jammer: None,
            signature: sensors::SignatureProfile::default(),
        }
    }
    fn launcher() -> Launcher {
        Launcher {
            position: [0., 1000., 0.],
            basis: Basis::new(0., 0., 0.),
            speed_fps: 300.,
            velocity: [0., 0., 300.],
            bay_ready: true,
            radar_power: true,
            radar: true,
            jammer: false,
            alive: true,
            controls: sensors::Controls::default(),
        }
    }
    #[test]
    fn player_chaff_decoys_only_radar_missiles_guiding_on_the_player() {
        let armed = |chance: u8| {
            let mut s = fixture(true);
            s.config.stations[0].weapon.seeker.chaff_flare_chance = chance;
            s.config.ecm.chaff[1] = 100;
            s.config.ecm.flare[1] = 100;
            s.chaff = 2;
            s.flares = 1;
            s.command(Command::Incoming, launcher());
            assert_eq!(s.projectiles[0].target, Some(0));
            s
        };
        // A flare cannot decoy a radar seeker, but it is still spent.
        let mut s = armed(100);
        s.command(Command::ReleaseFlare, launcher());
        assert_eq!((s.chaff, s.flares), (2, 0));
        assert_eq!(s.projectiles[0].target, Some(0));
        assert_eq!(s.effects.last().unwrap().kind, EffectKind::Flare);
        // Chaff at 100 x 100 percent always decoys the radar missile.
        s.command(Command::ReleaseChaff, launcher());
        assert_eq!(s.chaff, 1);
        assert_eq!(s.projectiles[0].target, None);
        assert!(s.projectiles[0].guidance.is_none());
        assert_eq!(s.effects.last().unwrap().kind, EffectKind::Chaff);
        // A resistant seeker keeps guiding.
        let mut s = armed(0);
        s.command(Command::ReleaseChaff, launcher());
        assert_eq!((s.chaff, s.projectiles[0].target), (1, Some(0)));
        // An empty dispenser releases nothing.
        let mut s = armed(100);
        s.chaff = 0;
        let effects = s.effects.len();
        s.command(Command::ReleaseChaff, launcher());
        assert_eq!(
            (s.effects.len(), s.projectiles[0].target),
            (effects, Some(0))
        );
        // Unlimited ammo releases without spending.
        let mut s = armed(100);
        s.cheats.unlimited_ammo = true;
        s.command(Command::ReleaseChaff, launcher());
        assert_eq!((s.chaff, s.projectiles[0].target), (2, None));
    }
    #[test]
    fn an_ai_round_does_not_credit_the_player_score() {
        // Drive a real shot into a real target twice: once owned by the
        // player and once owned by an AI actor. The damage, the hit event and
        // the destroyed event are identical; only the player's counters differ.
        fn run(owner: u32) -> (u32, u32, Vec<Event>) {
            let mut s = fixture(false);
            let l = launcher();
            s.range_target(l);
            let mut collected = Vec::new();
            let mut strikes = Vec::new();
            for _ in 0..600 {
                for p in &mut s.projectiles {
                    p.owner = owner;
                }
                collected.extend(s.step(true, l, |_, _| 0.));
                strikes.extend(s.take_strikes());
            }
            // Every damaging hit names its owner and victim for the radio.
            let hits: Vec<_> = collected
                .iter()
                .filter_map(|e| match e {
                    Event::Hit(id) => Some(*id),
                    _ => None,
                })
                .collect();
            assert_eq!(
                strikes
                    .iter()
                    .map(|s| s.victim.unwrap())
                    .collect::<Vec<_>>(),
                hits
            );
            assert!(strikes.iter().all(|s| s.owner == owner));
            assert_eq!(
                strikes.iter().filter(|s| s.destroyed).count(),
                collected
                    .iter()
                    .filter(|e| matches!(e, Event::Destroyed(_)))
                    .count()
            );
            assert!(s.take_strikes().is_empty(), "draining empties the log");
            (s.hits, s.kills, collected)
        }

        let (player_hits, player_kills, player_events) = run(PLAYER_OWNER);
        assert!(
            player_hits > 0,
            "the fixture never scored a hit, so the test proves nothing"
        );
        assert!(player_events.iter().any(|e| matches!(e, Event::Hit(_))));

        let (ai_hits, ai_kills, ai_events) = run(5);
        assert_eq!(ai_hits, 0, "an AI round credited the player with a hit");
        assert_eq!(ai_kills, 0, "an AI round credited the player with a kill");
        // The host still needs the events for damage, debris and effects.
        let player_hit_events = player_events
            .iter()
            .filter(|e| matches!(e, Event::Hit(_)))
            .count();
        let ai_hit_events = ai_events
            .iter()
            .filter(|e| matches!(e, Event::Hit(_)))
            .count();
        assert_eq!(
            player_hit_events, ai_hit_events,
            "ownership must change the score, not the damage"
        );
        let _ = player_kills;
    }

    #[test]
    fn mission_dummies_keep_distinct_identity_and_straight_velocity() {
        let mut s = fixture(true);
        let mut config = s.configuration().clone();
        config.hit_points = 37;
        let basis = Basis::new(0.3, 0., 0.);
        for n in 0..29 {
            s.add_dummy(&config, [n as f64 * 500., 5000., 6000.], basis);
        }
        let before = s.targets.clone();
        observe(&mut s, launcher(), 120);
        for (index, (a, b)) in before.iter().zip(&s.targets).enumerate() {
            assert_eq!(b.id, index as u32 + 1);
            assert_eq!(b.hp, 37);
            assert_eq!(b.signature, config.sensors.signature);
            assert_eq!(b.velocity, a.velocity);
            assert!(!b.radar_emitting && !b.jammer_active);
            for i in 0..3 {
                assert!((b.position[i] - a.position[i] - a.velocity[i]).abs() < 1e-7);
            }
        }
        s.range_target(launcher());
        assert_eq!(s.targets[0].id, 30);
    }
    /// Selection needs a current observation, so the shared sensors must have
    /// produced contacts before a designation command is applied.
    const ACQUISITION: usize = crate::sensors::track::ACQUISITION_STEPS as usize;
    fn observe(s: &mut State, l: Launcher, steps: usize) {
        for _ in 0..steps {
            s.step(false, l, |_, _| 0.);
        }
    }
    pub(super) fn target(id: u32, position: Vector, hp: i32, category: u16) -> Target {
        Target {
            aircraft: Some(AircraftId::F18),
            role: TargetRole::Aircraft,
            heat: Heat::Unknown,
            radar_emitting: false,
            id,
            position,
            velocity: [0.; 3],
            basis: Basis::new(0., 0., 0.),
            configuration: sensors::Configuration::CLEAN,
            signature: sensors::SignatureProfile::default(),
            jammer: None,
            jammer_active: false,
            airborne: true,
            on_ground: false,
            radius: 20.,
            hp,
            initial_hp: hp,
            fragment_offsets: [[0.; 3]; 2],
            wreck: None,
            wreck_power: crate::wreck::Power::default(),
            fragment_released: false,
            localized_damage: LocalizedDamage::default(),
            category,
        }
    }
    #[test]
    fn ground_projectile_damage_uses_object_class_and_destroys_once() {
        let mut s = fixture(false);
        s.targets.clear();
        s.config.stations[0].weapon.source = "M61.JT".into();
        s.config.stations[0].weapon.damage.by_class[2] = 17;
        let bounds = crate::airport::OrientedBox {
            center: [0., 1000., 300.],
            half: [30., 30., 30.],
            heading: 0.,
            pitch: 0.,
            bank: 0.,
        };
        s.add_ground_target(0x40000000, bounds, 17, 0x100).unwrap();
        let mut destroyed = 0;
        for _ in 0..300 {
            destroyed += s
                .step(true, launcher(), |_, _| 0.)
                .iter()
                .filter(|e| **e == Event::Destroyed(0x40000000))
                .count();
        }
        assert_eq!(destroyed, 1);
        assert_eq!(s.targets[0].hp, 0);
        assert_eq!(s.history[0].class, 2);
        assert_eq!(s.history[1].class, 2);
        assert_eq!([s.history[0].applied, s.history[1].applied], [3, 2]);
        assert_eq!(s.history[0].applied + s.history[1].applied, 5);
        assert_eq!(s.targets[0].localized_damage, LocalizedDamage::default());
    }
    #[test]
    fn range_controls_preserve_imported_ground_geometry_and_damage() {
        let mut s = fixture(false);
        let bounds = crate::airport::OrientedBox {
            center: [100., 20., 500.],
            half: [10., 10., 30.],
            heading: 0.3,
            pitch: 0.,
            bank: 0.,
        };
        s.add_ground_target(0x40000000, bounds, 750, 0x100).unwrap();
        s.targets
            .iter_mut()
            .find(|t| t.id == 0x40000000)
            .unwrap()
            .hp = 600;
        s.range_target(launcher());
        s.command(Command::TargetDistance(10000), launcher());
        s.command(Command::CycleClass, launcher());
        s.command(Command::ClearRange, launcher());
        assert_eq!(s.targets.len(), 1);
        let t = &s.targets[0];
        assert_eq!((t.id, t.hp, t.category), (0x40000000, 600, 0x100));
        assert_eq!(
            t.position,
            [
                bounds.center[0],
                bounds.center[1] + bounds.half[1] * 0.5,
                bounds.center[2]
            ]
        );
        assert_eq!(s.ground_bounds[&t.id], bounds);
    }
    #[test]
    fn swept_contact_handles_tunneling_moving_targets_and_nearest_root() {
        assert_eq!(
            segment_sphere([0., 0., -100.], [0., 0., 100.], 10.),
            Some(0.45)
        );
        assert_eq!(segment_sphere([0.; 3], [0.; 3], 1.), Some(0.));
        assert_eq!(segment_sphere([20., 0., -100.], [20., 0., 100.], 10.), None);
        assert_eq!(
            terrain_hit([0., 10., 0.], [0., -10., 100.], &|_, _| 0.),
            Some(0.5)
        );
    }
    #[test]
    fn source_debit_partial_last_round_empty_release_and_expiry() {
        let mut s = fixture(false);
        for _ in 0..300 {
            s.step(true, launcher(), |_, _| 0.);
        }
        assert_eq!(s.ammo, [0]);
        assert_eq!(s.shots, 6);
        for _ in 0..600 {
            s.step(false, launcher(), |_, _| 0.);
        }
        assert!(s.projectiles.is_empty());
        assert!(s.effects.is_empty());
        let mut s = fixture(false);
        s.step(true, launcher(), |_, _| 0.);
        s.release();
        for _ in 0..60 {
            s.step(false, launcher(), |_, _| 0.);
        }
        assert_eq!(s.shots, 1);
    }
    #[test]
    fn unlimited_ammo_fires_past_the_loaded_count_without_debit() {
        let mut s = fixture(false);
        s.cheats.unlimited_ammo = true;
        let loaded = s.ammo.clone();
        for _ in 0..300 {
            s.step(true, launcher(), |_, _| 0.);
        }
        assert_eq!(s.ammo, loaded);
        assert!(s.shots > 6);
    }
    #[test]
    fn invulnerable_cockpit_hit_neither_damages_nor_kills_the_pilot() {
        let mut s = fixture(false);
        s.cheats.invulnerable = true;
        s.config.stations[0].weapon.source = AircraftId::F18.gun().into();
        let l = launcher();
        s.command(Command::Incoming, l);
        let p = s.projectiles.last_mut().unwrap();
        p.position = std::array::from_fn(|i| {
            l.position[i] + l.basis.forward[i] * 100. + l.basis.up[i] * 11.
        });
        p.previous = p.position;
        p.age = 1;
        let hp = s.player_hp;
        let mut events = Vec::new();
        for _ in 0..120 {
            events.extend(s.step(false, l, |_, _| 0.));
        }
        assert!(s.projectiles.is_empty(), "the round still hits");
        assert_eq!(s.player_hp, hp);
        assert!(!events.iter().any(|e| matches!(
            e,
            Event::PilotKilled | Event::PlayerDestroyed | Event::PlayerDamaged(_)
        )));
    }
    #[test]
    fn detection_launch_and_inflight_lock_loss_are_distinct() {
        let mut s = fixture(true);
        let mut l = launcher();
        s.range_target(l);
        observe(&mut s, l, 1);
        s.designate_next(true);
        assert_eq!(s.designated(), Some(1));
        // Selection is immediate; the fire-control track is not.
        assert_eq!(s.readiness(l), Readiness::RadarAcquiring);
        observe(&mut s, l, ACQUISITION - 1);
        assert_eq!(s.sensors.acquired(), None);
        observe(&mut s, l, 1);
        assert_eq!(s.sensors.acquired(), Some(1));
        l.radar = false;
        assert!(!s.can_lock(l));
        s.step(true, l, |_, _| 0.);
        assert_eq!(s.ammo, [11]);
        // Radar off drops the target completely; it has to be selected again.
        assert_eq!(s.designated(), None);
        l.radar = true;
        observe(&mut s, l, 1);
        s.designate_next(true);
        observe(&mut s, l, 60);
        assert!(s.can_lock(l));
        s.step(true, l, |_, _| 0.);
        assert_eq!(s.ammo, [9]);
        assert_eq!(s.projectiles[0].target, Some(1));
        l.radar = false;
        s.step(false, l, |_, _| 0.);
        assert_eq!(s.projectiles[0].target, None);
        assert!(s.projectiles[0].position[2] > 0.);
    }
    #[test]
    fn actual_target_damage_destroys_once_and_generates_effects() {
        let mut s = fixture(false);
        s.targets.push(target(7, [0., 1000., 150.], 20, 0x80));
        let mut kills = 0;
        for _ in 0..180 {
            for e in s.step(true, launcher(), |_, _| 0.) {
                if matches!(e, Event::Destroyed(7)) {
                    kills += 1;
                }
            }
        }
        assert_eq!((s.hits, s.kills, kills, s.targets[0].hp), (2, 1, 1, 0));
        assert!(s.effects.iter().any(|e| e.kind == EffectKind::Destroyed));
        // The debrief ledger saw the same rounds, hits, damage and kill.
        let fired = s.ledger.total(|k| k.owner == PLAYER_OWNER);
        assert_eq!(fired.launched, s.shots);
        assert_eq!((fired.hit, fired.damage), (2, 20));
        assert_eq!(
            s.ledger.kills(),
            [Kill {
                owner: PLAYER_OWNER,
                victim: 7,
                category: 0x80,
                aircraft: s.targets[0].role == TargetRole::Aircraft,
            }]
        );
    }
    #[test]
    fn easy_aiming_widens_the_hit_volume_and_blasts_jolt_the_target() {
        let run = |easy: bool| {
            let mut s = fixture(false);
            s.cheats.easy_aiming = easy;
            // Between 1 and 1.5 target radii off the line of fire.
            s.targets.push(target(7, [25., 1000., 150.], 20, 0x80));
            let mut events = Vec::new();
            for _ in 0..180 {
                events.extend(s.step(true, launcher(), |_, _| 0.));
            }
            (s.hits, events)
        };
        assert_eq!(run(false).0, 0);
        let (hits, events) = run(true);
        assert!(hits > 0);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::Jolt(Jolt { target: Some(7), strength, .. }) if (*strength - 0.1).abs() < 1e-9
        )));
    }
    #[test]
    fn easy_aiming_missile_turns_faster_with_a_wider_cone() {
        let w = fixture(true).config.stations[0].weapon.clone();
        let eased = eased_weapon(&w);
        assert_eq!(
            f64::from(eased.movement.powered_turn_rate),
            (f64::from(w.movement.powered_turn_rate) * 1.5).round()
        );
        assert_eq!(
            f64::from(eased.seeker.zones[0].heading),
            (f64::from(w.seeker.zones[0].heading) * 1.25).round()
        );
        assert_eq!(eased.damage.by_class, w.damage.by_class);
    }
    #[test]
    fn an_incoming_missile_jolts_the_player_even_when_invulnerable() {
        let mut s = fixture(false);
        s.cheats.invulnerable = true;
        let l = launcher();
        s.command(Command::Incoming, l);
        let p = s.projectiles.last_mut().unwrap();
        p.position = std::array::from_fn(|i| l.position[i] + l.basis.forward[i] * 100.);
        p.previous = p.position;
        p.age = 1;
        let mut events = Vec::new();
        for _ in 0..120 {
            events.extend(s.step(false, l, |_, _| 0.));
        }
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Jolt(Jolt { target: None, .. })))
        );
        assert!(!events.iter().any(|e| matches!(e, Event::PlayerDamaged(_))));
    }
    #[test]
    fn guns_only_leaves_the_player_the_gun_and_nav() {
        let mut s = fixture(false);
        let mut gun = s.config.stations[0].clone();
        gun.weapon.source = AircraftId::F18.gun().into();
        s.config.stations.push(gun);
        s.ammo.push(100);
        s.armed = true;
        s.selected = 0;
        s.cheats.guns_only = true;
        s.enforce_guns_only();
        assert_eq!((s.armed, s.selected), (true, 1), "moved to the gun");
        s.cycle_selection(true);
        assert!(!s.armed, "NAV follows the gun");
        s.cycle_selection(true);
        assert_eq!((s.armed, s.selected), (true, 1), "the missile is skipped");
        s.cycle_selection(false);
        s.cycle_selection(false);
        assert_eq!((s.armed, s.selected), (true, 1));
        s.cheats.guns_only = false;
        s.cycle_selection(false);
        assert_eq!((s.armed, s.selected), (true, 0));
    }
    #[test]
    fn midair_collisions_destroy_everyone_but_an_invulnerable_player_unless_ignored() {
        let run = |ignore: bool, invulnerable: bool| {
            let mut s = fixture(false);
            s.cheats.invulnerable = invulnerable;
            s.cheats.ignore_midair_collisions = ignore;
            // Two AI aircraft closing head-on, and one well clear.
            let mut a = target(7, [0., 5000., 1000.], 20, 0x80);
            a.velocity = [0., 0., 600.];
            let mut b = target(8, [0., 5000., 1035.], 20, 0x80);
            b.velocity = [0., 0., -600.];
            let clear = target(9, [3000., 5000., 1000.], 20, 0x80);
            // A third aircraft sitting on the player.
            let on_player = target(10, [5., 1000., 5.], 20, 0x80);
            s.targets.extend([a, b, clear, on_player]);
            let events = s.step(false, launcher(), |_, _| 0.);
            (s, events)
        };
        let (s, events) = run(false, false);
        let hp: Vec<_> = s.targets.iter().map(|t| t.hp).collect();
        assert_eq!(hp, [0, 0, 20, 0]);
        assert_eq!(s.player_hp, 0);
        assert!(events.contains(&Event::PlayerDestroyed));
        assert!(events.contains(&Event::Destroyed(7)) && events.contains(&Event::Destroyed(8)));
        assert_eq!(s.kills, 0);
        // Invulnerable spares only the player.
        let (s, events) = run(false, true);
        assert!(s.player_hp > 0 && !events.contains(&Event::PlayerDestroyed));
        assert_eq!(s.targets[3].hp, 0);
        let (s, events) = run(true, false);
        assert!(s.targets.iter().all(|t| t.hp == 20) && s.player_hp > 0);
        assert!(!events.iter().any(|e| matches!(e, Event::Destroyed(_))));
    }
    #[test]
    fn fixed_ticks_replay_across_presentation_rates_and_pause() {
        let run = |fps: usize| {
            let mut s = fixture(false);
            s.targets.push(target(7, [0., 1000., 100000.], 100, 0x8000));
            s.targets[0].hp = 50;
            let mut clock = crate::flight::Clock { remainder: 0. };
            let mut tick = 0;
            for frame in 0..fps * 4 {
                // No call into either clock or combat while paused. Resume has
                // no elapsed wall-time backlog. Inputs are indexed by sim tick.
                if frame >= fps && frame < fps * 2 {
                    continue;
                }
                for _ in 0..clock.steps(1. / fps as f64) {
                    s.step(tick < 90, launcher(), |_, _| 0.);
                    tick += 1;
                }
            }
            (
                tick,
                s.ammo,
                s.projectiles,
                s.targets,
                s.effects,
                s.shots,
                s.smoke,
                s.debris,
            )
        };
        assert_eq!(run(30), run(60));
        assert_eq!(run(60), run(144));
    }
    #[test]
    fn capacity_failure_does_not_debit_and_dead_launcher_cannot_fire() {
        let mut s = fixture(false);
        s.config.stations[0].weapon.source = "M61.JT".into();
        s.config.stations[0].weapon.movement.remove_t = u16::MAX;
        let mut l = launcher();
        l.alive = false;
        s.step(true, l, |_, _| 0.);
        assert_eq!(s.shots, 0);
        l.alive = true;
        s.step(true, l, |_, _| 0.);
        let p = s.projectiles[0].clone();
        s.projectiles = vec![p; MAX_PROJECTILES];
        let ammo = s.ammo.clone();
        for _ in 0..30 {
            s.step(true, l, |_, _| 0.);
        }
        assert_eq!(s.ammo, ammo);
        s.projectiles.clear();
        for _ in 0..14 {
            assert!(
                !s.step(true, l, |_, _| -10000.)
                    .iter()
                    .any(|event| matches!(event, Event::Fired(0)))
            );
        }
        assert!(
            s.step(true, l, |_, _| -10000.)
                .iter()
                .any(|event| matches!(event, Event::Fired(0)))
        );
        assert_eq!(s.rounds(0), ammo[0] - 1);
    }
    #[test]
    fn motor_smoke_uses_powered_phase_and_aircraft_smoke_uses_health() {
        use super::super::smoke::Kind;
        let l = launcher();
        let mut s = fixture(true);
        s.config.stations[0].weapon.movement.ignite_t = 1;
        s.config.stations[0].weapon.movement.fuel_t = 2;
        s.command(Command::Incoming, l);
        s.projectiles[0].incoming = false;
        s.projectiles[0].target = None;
        s.projectiles[0].position = [0., 10000., 100000.];
        s.projectiles[0].direction = [0., 0., 1.];
        // Exercise the spec vector motor, keeping this fixture far from contacts.
        s.projectiles[0].motion = Some(Motion::new(
            &s.config.stations[0].weapon.movement,
            [0., 0., 1000.],
            10000.,
        ));
        observe(&mut s, l, 30);
        assert!(s.smoke.puffs.is_empty());
        observe(&mut s, l, 30);
        assert_eq!(s.smoke.puffs.len(), 4);
        assert!(s.smoke.puffs.iter().all(|p| p.kind == Kind::Missile));
        observe(&mut s, l, 30);
        assert_eq!(s.smoke.puffs.len(), 4);
        s.projectiles.clear();
        observe(&mut s, l, 480);
        assert!(s.smoke.puffs.is_empty());
        s.targets.push(target(1, [0., 5000., 100000.], 100, 0x8000));
        s.targets[0].hp = 51;
        observe(&mut s, l, 12);
        assert!(s.smoke.puffs.is_empty());
        s.targets[0].hp = 50;
        observe(&mut s, l, 12);
        assert_eq!(s.smoke.puffs.len(), 1);
        s.targets[0].hp = 0;
        observe(&mut s, l, 12);
        assert_eq!(s.smoke.puffs.len(), 2);
        s.targets[0].airborne = false;
        observe(&mut s, l, 12);
        assert_eq!(s.smoke.puffs.len(), 2);
        s.range_target(l);
        assert!(s.smoke.puffs.is_empty());
        let mut gun = fixture(false);
        observe(&mut gun, l, 6);
        gun.step(true, l, |_, _| 0.);
        assert!(gun.smoke.puffs.is_empty());
    }
    #[test]
    fn detached_piece_is_emitted_once_and_removed_with_one_ground_effect() {
        let mut s = fixture(false);
        let l = launcher();
        s.targets.push(target(7, [0., 5., 100000.], 100, 0x8000));
        s.targets[0].hp = 0;
        s.targets[0]
            .localized_damage
            .record(DamageSection::LeftWing, 75, 100);
        s.targets[0].velocity = [30., 0., 10.];
        s.step(false, l, |_, _| 0.);
        assert_eq!(s.debris.len(), 1);
        assert_eq!(s.debris[0].owner, 7);
        assert_eq!(s.debris[0].velocity, s.targets[0].velocity);
        let mut impacts = 0;
        for _ in 0..300 {
            let had_piece = !s.debris.is_empty();
            s.step(false, l, |_, _| 0.);
            if had_piece && s.debris.is_empty() {
                impacts += 1;
                assert_eq!(
                    s.effects
                        .iter()
                        .filter(|e| e.kind == EffectKind::DebrisImpact)
                        .count(),
                    1
                );
            }
        }
        assert_eq!(impacts, 1);
        assert!(s.debris.is_empty());
        assert!(s.effects.iter().all(|e| e.kind != EffectKind::DebrisImpact));
        s.targets[0].hp = 0;
        s.step(false, l, |_, _| 0.);
        assert!(
            s.debris.is_empty(),
            "further damage must not duplicate the same lost part"
        );
        s.player_hp = s.config.damage_capacity / 2;
        s.player_localized_damage.record(
            DamageSection::Nose,
            s.config.damage_capacity,
            s.config.damage_capacity,
        );
        s.step(false, l, |_, _| 0.);
        assert!(
            s.debris.is_empty(),
            "a live aircraft keeps catastrophic parts"
        );
        s.player_hp = 0;
        s.step(false, l, |_, _| 0.);
        assert_eq!(s.debris.len(), 1);
        assert_eq!(s.debris[0].owner, 0);
        assert_eq!(s.debris[0].velocity, l.velocity);
        let reset = State::new(s.configuration().clone(), true).unwrap();
        assert!(reset.debris.is_empty());
    }
    #[test]
    fn native_damage_category_switch_is_exact_not_a_mask() {
        for (category, index) in [
            (0x80, 0),
            (0x2000, 1),
            (0x100, 2),
            (0x400, 3),
            (0x40, 4),
            (0x200, 4),
            (0x800, 4),
            (0x1000, 4),
            (0x4000, 0),
            (0x8000, 0),
            (0x240, 0),
        ] {
            assert_eq!(damage_class(category), index);
        }
    }
    #[test]
    fn all_damage_classes_report_nominal_applied_and_cumulative_hp() {
        for (index, category) in [0x80, 0x2000, 0x100, 0x400, 0x40].into_iter().enumerate() {
            let mut s = fixture(false);
            s.config.stations[0].weapon.damage.by_class = [3, 7, 9, 11, 25];
            s.targets.push(target(7, [0., 1000., 150.], 20, category));
            for _ in 0..180 {
                s.step(true, launcher(), |_, _| 0.);
            }
            assert!(s.history.iter().all(|h| h.class == index));
            assert_eq!(
                s.history.iter().map(|h| h.applied).sum::<i32>(),
                20 - s.targets[0].hp
            );
            assert!(s.history.iter().all(|h| h.applied <= h.nominal));
        }
    }
    #[test]
    fn failed_station_keeps_mass_and_jettison_cannot_remove_internal_gun() {
        let mut s = fixture(true);
        let mass = s.payload_lbs();
        s.command(Command::FailStation, launcher());
        assert_eq!(s.readiness(launcher()), Readiness::StationFailed);
        assert_eq!(s.rounds(0), 11);
        assert_eq!(s.payload_lbs(), mass);
        s.step(true, launcher(), |_, _| 0.);
        assert_eq!(s.shots, 0);
        s.command(Command::Jettison, launcher());
        assert_eq!(s.payload_lbs(), 0.);
        assert_eq!(s.ammo[0], 0x8000); // Unloading cannot repair a failed station.
        let mut gun = fixture(false);
        gun.command(Command::Jettison, launcher());
        assert_eq!(gun.rounds(0), 11);
    }
    #[test]
    fn readiness_reports_inhibits_without_ammunition_consumption() {
        let mut s = fixture(true);
        let l = launcher();
        assert_eq!(s.readiness(l), Readiness::NoTarget);
        s.command(Command::ToggleArm, l);
        assert_eq!(s.readiness(l), Readiness::Safe);
        s.step(true, l, |_, _| 0.);
        assert_eq!(s.rounds(0), 11);
        s.command(Command::ToggleArm, l);
        s.range_target(l);
        observe(&mut s, l, 1);
        s.designate_next(true);
        observe(&mut s, l, ACQUISITION);
        assert_eq!(s.readiness(l), Readiness::Ready);
        let z = &mut s.config.stations[0].weapon.seeker.zones[1];
        z.minimum_range = 4000;
        assert_eq!(s.readiness(l), Readiness::MinimumRange);
        s.config.stations[0].weapon.seeker.zones[1].minimum_range = 0;
        s.config.stations[0].weapon.seeker.zones[1].maximum_range = 2000;
        assert_eq!(s.readiness(l), Readiness::MaximumRange);
    }
    #[test]
    fn replacement_clears_old_engagement_and_uses_fresh_identity() {
        let mut s = fixture(true);
        s.range_target(launcher());
        observe(&mut s, launcher(), 1);
        s.designate_next(true);
        observe(&mut s, launcher(), ACQUISITION);
        s.step(true, launcher(), |_, _| 0.);
        let ammo = s.ammo.clone();
        assert!(!s.projectiles.is_empty());
        s.range_target(launcher());
        assert_eq!(s.targets[0].id, 2);
        assert!(s.projectiles.is_empty() && s.effects.is_empty() && s.designated().is_none());
        assert_eq!(s.ammo, ammo);
    }
    #[test]
    fn autonomous_tracking_survives_radar_off_but_dead_target_is_retired() {
        let mut s = fixture(true);
        s.config.stations[0].weapon.flags &= !0x200;
        s.range_target(launcher());
        observe(&mut s, launcher(), 1);
        s.designate_next(true);
        observe(&mut s, launcher(), ACQUISITION);
        s.step(true, launcher(), |_, _| 0.);
        let mut l = launcher();
        l.radar = false;
        assert_eq!(s.readiness(l), Readiness::RadarOff);
        s.step(false, l, |_, _| 0.);
        assert_eq!(s.projectiles[0].target, Some(1));
        s.targets[0].hp = 0;
        let events = s.step(false, l, |_, _| 0.);
        assert_eq!(s.projectiles[0].target, None);
        assert!(events.contains(&Event::TrackLost(1)));
    }
    #[test]
    fn terrain_visibility_gates_designation_launch_scope_and_tracking() {
        let mut s = fixture(true);
        let l = launcher();
        s.range_target(l);
        observe(&mut s, l, 1);
        s.designate_next(true);
        observe(&mut s, l, ACQUISITION);
        let wall = |_: f64, z: f64| {
            if (1000. ..2000.).contains(&z) {
                2000.
            } else {
                0.
            }
        };
        // Masking removes the observation, so selection and the launch
        // permission end together and no round is consumed.
        s.step(true, l, wall);
        assert!(s.sensors.contacts().is_empty());
        assert_eq!(s.designated(), None);
        assert_eq!(s.readiness(l), Readiness::NoTarget);
        assert_eq!(s.rounds(0), 11);
        s.designate_next(true);
        assert_eq!(s.designated(), None);
        observe(&mut s, l, ACQUISITION + 1);
        s.designate_next(true);
        observe(&mut s, l, ACQUISITION);
        s.step(true, l, |_, _| 0.);
        assert_eq!(s.projectiles[0].target, Some(1));
        assert!(s.step(false, l, wall).contains(&Event::TrackLost(1)));
    }
    #[test]
    fn manual_command_tape_is_identical_with_pause_and_render_cadence() {
        let run = |fps: usize| {
            let mut s = fixture(true);
            let l = launcher();
            let mut clock = crate::flight::Clock { remainder: 0. };
            let mut tick = 0;
            for frame in 0..fps * 5 {
                if (fps..fps * 2).contains(&frame) {
                    continue;
                }
                for _ in 0..clock.steps(1. / fps as f64) {
                    let command = match tick {
                        0 => Some(Command::ReplaceTarget),
                        1 => Some(Command::Designate),
                        10 | 20 => Some(Command::ToggleArm),
                        40 => Some(Command::ClearDesignation),
                        50 => Some(Command::Designate),
                        60 => Some(Command::FailStation),
                        90 => Some(Command::Jettison),
                        100 => Some(Command::CycleClass),
                        _ => None,
                    };
                    if let Some(command) = command {
                        s.command(command, l);
                    }
                    s.step(tick % 30 == 0, l, |_, _| 0.);
                    tick += 1;
                }
            }
            (tick, format!("{s:?}"))
        };
        assert_eq!(run(30), run(60));
        assert_eq!(run(60), run(144));
    }
    #[test]
    fn cycling_classes_restores_exact_source_category() {
        let mut s = fixture(false);
        s.config.target_category = 0x8000;
        s.range_category = s.config.target_category;
        for _ in 0..5 {
            s.command(Command::CycleClass, launcher());
        }
        assert_eq!(s.range_category, 0x8000);
        assert_eq!(s.targets[0].category, 0x8000);
    }

    #[test]
    fn incoming_contacts_player_ecm_and_replay_are_connected() {
        let mut s = fixture(true);
        let mut l = launcher();
        l.jammer = true;
        s.config.ecm.radar_deception_chance = 100;
        s.command(Command::Incoming, l);
        s.projectiles[0].position = l.position;
        let ammo = s.ammo.clone();
        let mut replay = s.clone();
        let events = s.step(false, l, |_, _| 0.);
        assert!(events.contains(&Event::Defeated(0)));
        assert_eq!(s.player_hp, s.config.damage_capacity);
        assert_eq!(s.ammo, ammo);
        assert_eq!(events, replay.step(false, l, |_, _| 0.));
        assert_eq!(format!("{s:?}"), format!("{replay:?}"));
        s.ecm_failed = true; // A powered request cannot bypass equipment failure.
        s.command(Command::Incoming, l);
        s.projectiles[0].position = l.position;
        let events = s.step(false, l, |_, _| 0.);
        assert!(events.iter().any(|e| matches!(e, Event::PlayerDamaged(_))));
        assert!(s.player_hp < s.config.damage_capacity);
    }
    #[test]
    fn automatic_station_radar_ecm_failures_keep_mass_and_reset() {
        for index in [36, 37, 38] {
            let mut s = fixture(false);
            s.config.system_damage = [0; 45];
            s.config.system_damage[index] = 0x1f;
            s.config.damage_capacity = 1;
            s.player_hp = 10000;
            let mass = s.payload_lbs();
            let mut events = vec![];
            for _ in 0..30 {
                s.apply_player_damage(4, &mut events);
            }
            assert_eq!(s.subsystem_counts[index], 1);
            assert_eq!(s.last_subsystem, Some(index));
            if index == 36 {
                assert_ne!(s.ammo[0] & 0x8000, 0);
            }
            if index == 37 {
                assert!(s.radar_failed);
            }
            if index == 38 {
                assert!(s.ecm_failed);
            }
            assert_eq!(mass, s.payload_lbs());
            let reset = State::new(s.config.clone(), true).unwrap();
            assert!(!reset.radar_failed && !reset.ecm_failed);
            assert_eq!(reset.subsystem_counts, [0; 45]);
        }
    }

    #[test]
    fn sequential_launches_keep_their_own_targets_under_one_cockpit_track() {
        let mut s = fixture(true);
        // Fire and forget: the weapon needs support at launch, not after it.
        s.config.stations[0].weapon.flags &= !0x200;
        let l = launcher();
        s.targets.push(target(1, [400., 1000., 3000.], 20, 0x80));
        s.targets.push(target(2, [-400., 1000., 3000.], 20, 0x80));
        observe(&mut s, l, 1);
        s.command(Command::DesignateTarget(1), l);
        observe(&mut s, l, ACQUISITION);
        assert_eq!(s.sensors.acquired(), Some(1));
        assert!(s.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
        s.release();
        // Selecting the second target releases the first illumination at once.
        s.command(Command::DesignateTarget(2), l);
        assert_eq!(s.sensors.acquired(), None);
        assert_eq!(s.readiness(l), Readiness::RadarAcquiring);
        observe(&mut s, l, ACQUISITION);
        assert_eq!(s.sensors.acquired(), Some(2));
        assert!(s.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
        let targets: Vec<_> = s.projectiles.iter().map(|p| p.target).collect();
        assert_eq!(targets, [Some(1), Some(2)]);
        assert_eq!(s.sensors.acquired(), Some(2));
        assert_eq!(s.designated(), Some(2));
    }
    #[test]
    fn a_continuous_lock_weapon_loses_support_when_the_cockpit_switches_target() {
        let mut s = fixture(true);
        assert!(s.config.stations[0].weapon.flags & 0x200 != 0);
        let l = launcher();
        s.targets.push(target(1, [400., 1000., 3000.], 20, 0x80));
        s.targets.push(target(2, [-400., 1000., 3000.], 20, 0x80));
        observe(&mut s, l, 1);
        s.command(Command::DesignateTarget(1), l);
        observe(&mut s, l, ACQUISITION);
        assert!(s.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
        assert_eq!(s.projectiles[0].target, Some(1));
        s.release();
        s.command(Command::DesignateTarget(2), l);
        // Designating another contact is never illumination of the first.
        assert!(s.step(false, l, |_, _| 0.).contains(&Event::TrackLost(1)));
        assert_eq!(s.projectiles[0].target, None);
    }
    #[test]
    fn a_parked_aircraft_is_not_a_player_radar_contact_until_it_flies() {
        // Manual p.208: grounded aircraft do not appear on enemy radar.
        let mut s = fixture(true);
        let l = launcher();
        s.targets.push(target(1, [0., 1000., 3000.], 20, 0x80));
        s.targets[0].on_ground = true;
        observe(&mut s, l, 1);
        assert!(s.sensors.contact(1).is_none());
        s.command(Command::DesignateTarget(1), l);
        assert_eq!(s.designated(), None);
        s.targets[0].on_ground = false;
        observe(&mut s, l, 1);
        assert!(s.sensors.contact(1).is_some());
    }
    #[test]
    fn a_destroyed_aircraft_stays_a_contact_until_its_wreck_reaches_the_ground() {
        let mut s = fixture(true);
        let l = launcher();
        s.targets.push(target(1, [0., 1000., 3000.], 20, 0x80));
        observe(&mut s, l, 1);
        s.command(Command::DesignateTarget(1), l);
        observe(&mut s, l, ACQUISITION);
        s.targets[0].hp = 0;
        observe(&mut s, l, 1);
        // Hit points reaching zero removes combat viability, not the return.
        assert!(s.sensors.contact(1).is_some());
        assert_eq!(s.designated(), Some(1));
        assert_eq!(s.readiness(l), Readiness::TargetDestroyed);
        assert!(s.targets[0].airborne);
        for _ in 0..1200 {
            s.step(false, l, |_, _| 0.);
            if !s.targets[0].airborne {
                break;
            }
        }
        // A grounded wreck ends the air-to-air observation and selection on
        // the next step, since observations are produced before movement.
        assert!(!s.targets[0].airborne);
        s.step(false, l, |_, _| 0.);
        assert_eq!(s.targets[0].position[1], 0.);
        assert!(s.sensors.contact(1).is_none());
        assert_eq!(s.designated(), None);
        assert_eq!(s.kills, 0);
    }
    #[test]
    fn automatic_radar_failure_inhibits_launch_and_breaks_illumination() {
        let mut s = fixture(true);
        let l = launcher();
        s.range_target(l);
        observe(&mut s, l, 1);
        s.designate_next(true);
        observe(&mut s, l, ACQUISITION);
        let id = s.designated().expect("selected fixture target");
        assert!(s.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
        s.config.system_damage = [0; 45];
        s.config.system_damage[37] = 0x1f;
        s.config.damage_capacity = 1;
        s.player_hp = 10000;
        let mut events = vec![];
        for _ in 0..30 {
            s.apply_player_damage(4, &mut events);
        }
        assert!(s.radar_failed);
        assert_eq!(s.readiness(l), Readiness::RadarFailed);
        let ammo = s.ammo.clone();
        assert!(s.step(true, l, |_, _| 0.).contains(&Event::TrackLost(id)));
        assert_eq!(s.ammo, ammo);
    }

    fn guided_weapon(state: &State, source: &str, signature: u8) -> Weapon {
        let mut weapon = state.config.stations[0].weapon.clone();
        weapon.source = source.into();
        weapon.seeker.signature = signature;
        for zone in &mut weapon.seeker.zones {
            zone.minimum_range = 0;
            zone.maximum_range = 100_000;
            zone.minimum_altitude = -100_000;
            zone.maximum_altitude = 100_000;
            zone.heading = i16::MAX;
            zone.pitch = i16::MAX;
        }
        weapon
    }

    fn owned_shot(
        weapon: Weapon,
        owner: u32,
        target: u32,
        position: Vector,
        observation: seeker::Observation,
    ) -> Projectile {
        let profile = missiles::Profile::for_weapon(&weapon).unwrap();
        Projectile {
            id: 91,
            owner,
            weapon: Some(weapon.clone()),
            guidance: Some(Flight::from_supported_launch(
                profile,
                LaunchMode::Cued,
                observation,
                position,
            )),
            motion: Some(Motion::new(&weapon.movement, [0., 0., 600.], position[1])),
            guidance_ticks: Some(profile.guidance_ticks),
            age: 0,
            incoming: target == PLAYER_OWNER,
            station: 0,
            position,
            previous: position,
            direction: [0., 0., -1.],
            speed_f8: 600 * 256,
            launched_t: 0,
            target: Some(target),
            fall: FallState::default(),
            gun_round: None,
            tracer: false,
        }
    }

    #[test]
    fn ai_active_seeker_acquires_player_without_player_sensor_data() {
        let mut state = fixture(true);
        let weapon = guided_weapon(&state, "AIM120.JT", 3);
        let player = launcher();
        let observation = seeker::Observation {
            id: PLAYER_OWNER,
            position: player.position,
            velocity: player.velocity,
            quality: 1.,
            off_axis: 0.,
            range: 3_000.,
        };
        state.projectiles.push(owned_shot(
            weapon,
            7,
            PLAYER_OWNER,
            [0., player.position[1], 3_000.],
            observation,
        ));
        state.set_actor_supports([ActorSupport {
            owner: 7,
            observation: Some(observation),
            supported: true,
            radar_position: [0., 1000., 5000.],
            radar_emitting: true,
        }]);
        for _ in 0..missiles::DWELL {
            state.step(false, player, |_, _| 0.);
        }
        let flight = state.projectiles[0].guidance.as_ref().unwrap();
        assert_eq!(flight.seeker.target, Some(PLAYER_OWNER));
        assert_eq!(flight.seeker.status, Status::Pitbull);
        assert!(state.missile_snapshots(player)[0].radar_acquired);
    }

    #[test]
    fn supported_ai_shot_uses_only_its_owner_support_and_reacquires() {
        let state = fixture(true);
        let weapon = guided_weapon(&state, "R530.JT", 3);
        let target = target(44, [0., 1000., 4000.], 20, 0x80);
        let observation = seeker::Observation {
            id: target.id,
            position: target.position,
            velocity: target.velocity,
            quality: 1.,
            off_axis: 0.,
            range: 4000.,
        };
        let mut shot = owned_shot(weapon.clone(), 7, target.id, [0., 1000., 0.], observation);
        for _ in 0..missiles::DWELL {
            guide_owned(
                &mut shot,
                &weapon,
                std::slice::from_ref(&target),
                None,
                None,
                &|_, _| false,
                &|_, _| 0.,
            );
        }
        assert!(!shot.guidance.as_ref().unwrap().seeker.acquired);
        let support = ActorSupport {
            owner: 7,
            observation: Some(observation),
            supported: true,
            radar_position: [0., 1000., -1000.],
            radar_emitting: true,
        };
        for _ in 0..missiles::DWELL {
            guide_owned(
                &mut shot,
                &weapon,
                std::slice::from_ref(&target),
                None,
                Some(&support),
                &|_, _| false,
                &|_, _| 0.,
            );
        }
        assert_eq!(
            shot.guidance.as_ref().unwrap().seeker.status,
            Status::Locked
        );
        guide_owned(
            &mut shot,
            &weapon,
            std::slice::from_ref(&target),
            None,
            None,
            &|_, _| false,
            &|_, _| 0.,
        );
        assert_eq!(
            shot.guidance.as_ref().unwrap().seeker.status,
            Status::Memory
        );
        for _ in 0..missiles::DWELL {
            guide_owned(
                &mut shot,
                &weapon,
                std::slice::from_ref(&target),
                None,
                Some(&support),
                &|_, _| false,
                &|_, _| 0.,
            );
        }
        assert_eq!(
            shot.guidance.as_ref().unwrap().seeker.status,
            Status::Locked
        );
    }

    #[test]
    fn active_notch_enters_memory_instead_of_destroying_flight() {
        let state = fixture(true);
        let weapon = guided_weapon(&state, "AIM120.JT", 3);
        let mut crossing = target(5, [0., 0., 60_000.], 20, 0x80);
        crossing.signature.radar = 100.;
        crossing.velocity = [800., 0., 0.];
        let basis = Basis::new(0., 0., 0.);
        assert!(!missiles::active_radar_visible(
            &weapon,
            [0., 5000., 0.],
            basis,
            &crossing,
            crossing.position[1],
        ));
        crossing.velocity = [0., 0., 800.];
        assert!(missiles::active_radar_visible(
            &weapon,
            [0., 5000., 0.],
            basis,
            &crossing,
            crossing.position[1],
        ));
        let profile = missiles::Profile::for_weapon(&weapon).unwrap();
        let mut seeker = seeker::Seeker::new(Some(crossing.id));
        seeker.acquired = true;
        seeker.status = Status::Pitbull;
        seeker.step(profile, &[]);
        assert_eq!(seeker.status, Status::Memory);
        assert_eq!(seeker.target, Some(crossing.id));
    }

    #[test]
    fn snapshots_keep_compatibility_missile_bodies_visible() {
        let mut state = fixture(true);
        let weapon = guided_weapon(&state, "AIM120.JT", 3);
        let observation = seeker::Observation {
            id: 8,
            position: [0., 1000., 5000.],
            velocity: [0.; 3],
            quality: 1.,
            off_axis: 0.,
            range: 5000.,
        };
        let mut projectile = owned_shot(weapon, 8, 8, [0., 1000., 0.], observation);
        projectile.guidance = None;
        projectile.motion = None;
        state.projectiles.push(projectile);
        let snapshots = state.missile_snapshots(launcher());
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].guidance, missiles::Guidance::Active);
        assert!(!snapshots[0].radar_active);
        assert!(!snapshots[0].radar_acquired);
    }

    #[test]
    fn player_supported_snapshot_carries_actual_support() {
        let mut state = fixture(true);
        state.config.stations[0].weapon = guided_weapon(&state, "R530.JT", 3);
        let player = launcher();
        state.range_target(player);
        observe(&mut state, player, 1);
        state.designate_next(true);
        observe(&mut state, player, ACQUISITION);
        let id = state.designated().unwrap();
        let contact = state.sensors.observation(id).unwrap();
        let observation = seeker::Observation {
            id,
            position: contact.position,
            velocity: contact.velocity,
            quality: 1.,
            off_axis: 0.,
            range: missiles::length(sub(contact.position, player.position)),
        };
        state.projectiles.push(owned_shot(
            state.config.stations[0].weapon.clone(),
            PLAYER_OWNER,
            id,
            player.position,
            observation,
        ));
        let snapshot = state.missile_snapshots(player)[0];
        assert!(snapshot.supported);
        assert_eq!(snapshot.supporting_radar_position, Some(player.position));
    }
}

/// Easy aiming's copy of a player missile: a wider seeker cone and a faster
/// turn. The weapon data itself is unchanged.
fn eased_weapon(w: &Weapon) -> Weapon {
    let scale = |v: i16, k: f64| (f64::from(v) * k).round().clamp(0., f64::from(i16::MAX)) as i16;
    let mut w = w.clone();
    let turn = crate::cheats::EASY_AIMING_TURN;
    w.movement.powered_turn_rate = scale(w.movement.powered_turn_rate, turn);
    w.movement.unpowered_turn_rate = scale(w.movement.unpowered_turn_rate, turn);
    for zone in &mut w.seeker.zones {
        zone.heading = scale(zone.heading, crate::cheats::EASY_AIMING_CONE);
        zone.pitch = scale(zone.pitch, crate::cheats::EASY_AIMING_CONE);
    }
    w
}

fn guide_owned(
    p: &mut Projectile,
    w: &Weapon,
    targets: &[Target],
    player: Option<&Target>,
    support: Option<&ActorSupport>,
    obscured: &dyn Fn(Vector, Vector) -> bool,
    ground: &dyn Fn(f64, f64) -> f64,
) {
    let flight = p.guidance.as_mut().unwrap();
    let profile = flight.profile;
    if flight.unguided {
        return;
    }
    if p.age >= p.guidance_ticks.unwrap_or(profile.guidance_ticks) {
        flight.seeker.status = Status::Expired;
        flight.seeker.observation = None;
        return;
    }
    let supported = flight
        .seeker
        .target
        .filter(|id| {
            support.is_some_and(|answer| {
                answer.supported
                    && answer.radar_emitting
                    && answer.observation.is_some_and(|o| o.id == *id)
            })
        })
        .filter(|id| {
            targets
                .iter()
                .chain(player)
                .find(|t| t.id == *id)
                .is_some_and(|t| flight.eligible(w, t))
        })
        .and_then(|id| {
            support
                .and_then(|answer| answer.observation)
                .filter(|o| o.id == id)
        });
    // Only shared supported observations may update an initially silent shot.
    if !flight.seeker.acquired
        && let Some(contact) = supported
        && (flight.last_intercept.is_none() || p.age.is_multiple_of(12))
    {
        flight.solution = Some(missiles::lead(
            p.position,
            missiles::length(p.motion.unwrap().velocity),
            contact.position,
            contact.velocity,
        ));
        flight.last_intercept = Some(flight.solution.map_or(contact.position, |s| s.point));
    }
    if profile.guidance == Guidance::Active && !flight.enabled {
        flight.enabled = flight
            .last_intercept
            .zip(profile.activation_ft)
            .is_some_and(|(point, distance)| missiles::length(sub(point, p.position)) <= distance);
    }
    if flight.enabled {
        let cap = (flight.mode == LaunchMode::Boresight && !flight.seeker.acquired)
            .then(|| profile.search_cap());
        let basis = Basis::new(
            p.direction[0].atan2(p.direction[2]),
            p.direction[1].atan2(p.direction[0].hypot(p.direction[2])),
            0.,
        );
        let view = seeker::View {
            position: p.position,
            basis,
            cap,
            obscured,
        };
        let observations: Vec<_> = targets
            .iter()
            .chain(player)
            .filter(|t| flight.eligible(w, t))
            .filter(|t| {
                profile.guidance != Guidance::Supported || supported.is_some_and(|o| o.id == t.id)
            })
            .filter_map(|t| {
                let observed = seeker::observe(w, profile, &view, t)?;
                if profile.guidance == Guidance::Supported {
                    supported
                } else if profile.guidance == Guidance::Active
                    && !missiles::active_radar_visible(
                        w,
                        p.position,
                        basis,
                        t,
                        t.position[1] - ground(t.position[0], t.position[2]),
                    )
                {
                    None
                } else {
                    Some(observed)
                }
            })
            .collect();
        if profile.guidance == Guidance::Supported && !observations.is_empty() {
            flight.seeker.candidate = flight.seeker.target;
            flight.seeker.dwell = missiles::DWELL;
        }
        flight.seeker.step(profile, &observations);
        if flight.seeker.acquired && flight.seeker.observation.is_some() {
            flight.qualified_target = flight.seeker.target;
        }
        p.target = flight.seeker.target;
        if let Some(o) = flight
            .seeker
            .observation
            .filter(|_| matches!(flight.seeker.status, Status::Locked | Status::Pitbull))
            && (flight.last_intercept.is_none() || p.age.is_multiple_of(12))
        {
            flight.solution = Some(missiles::lead(
                p.position,
                missiles::length(p.motion.unwrap().velocity),
                o.position,
                o.velocity,
            ));
            flight.last_intercept = Some(flight.solution.map_or(o.position, |s| s.point));
        }
    } else {
        flight.seeker.status = Status::Midcourse;
    }
    // Remembered intercept is frozen on loss. Reacquisition may continue for the
    // full guidance lifetime, per John's 2026-09-17 revision.
    let can_steer = profile.guidance != Guidance::Supported || supported.is_some();
    if can_steer && let Some(point) = flight.last_intercept {
        let desired = unit(sub(point, p.position));
        let heading = missiles::commanded_heading(p.direction, p.motion.unwrap().velocity, desired);
        p.direction = missiles::steer(&w.movement, p.age, p.direction, heading);
    }
}

#[cfg(test)]
fn guide(
    p: &mut Projectile,
    w: &Weapon,
    targets: &[Target],
    sensors: &Sensors,
    obscured: &dyn Fn(Vector, Vector) -> bool,
) {
    let support = p
        .guidance
        .as_ref()
        .and_then(|flight| flight.seeker.target)
        .map(|id| ActorSupport {
            owner: p.owner,
            supported: sensors.supports(id),
            observation: sensors.observation(id).map(|contact| seeker::Observation {
                id,
                position: contact.position,
                velocity: contact.velocity,
                quality: 1.,
                off_axis: 0.,
                range: missiles::length(sub(contact.position, p.position)),
            }),
            radar_position: [0.; 3],
            radar_emitting: true,
        });
    guide_owned(p, w, targets, None, support.as_ref(), obscured, &|_, _| 0.);
}

#[cfg(test)]
#[path = "missile_tests.rs"]
mod missile_tests;
