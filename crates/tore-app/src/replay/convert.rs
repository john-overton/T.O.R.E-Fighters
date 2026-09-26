//! What combat draws, converted into mission recording frames and back.
//!
//! The recorder turns each tick's [`RenderSnapshot`], plus the flight data a
//! snapshot does not carry ([`FlightData`]), into `tore_replay` frame pieces.
//! A replay turns a decoded frame back into a snapshot with [`snapshot`] and
//! draws it through the same [`crate::render_snapshot`] helpers live flight
//! uses, so the picture matches within the format's documented precision
//! (docs/REPLAYS.md). [`difference`] checks that claim tick by tick.
//!
//! The draw rules a snapshot carries per aircraft are the same for a whole
//! recording, so they live in the header as a [`Presentation`]. Ground
//! objects are not aircraft: a recording keeps only their hit points.
// The recorder uses the recording half and the replay viewer the rest.
#![allow(dead_code)]
use crate::render_snapshot::{
    AircraftPose, DEVICES, Damage, DebrisPose, Draw, EffectPose, Engine, PilotPose, ProjectilePose,
    RenderSnapshot,
};
use std::collections::BTreeMap;
use tore_formats::aircraft::AircraftId;
use tore_replay as replay;
use tore_sim::{
    combat::live::{self, DamageSection},
    ejection, wreck,
};

/// The exact identity a recording stores for an aircraft type: its PT
/// resource, or for the F/A-XX runtime variant its own key, so a variant is
/// never recorded as the aircraft it borrows from.
pub fn identity_key(id: AircraftId) -> &'static str {
    id.selection_key()
}

/// The aircraft type a recorded identity names.
pub fn identity(key: &str) -> Option<AircraftId> {
    AircraftId::parse(key).ok()
}

/// Header extra listing the loaded aircraft models in draw order.
pub const MODELS_KEY: &str = "draw.models";
/// Header extra: aircraft 1 to this id draw with their own model.
pub const SLOTS_KEY: &str = "draw.slots";

/// How a recording draws the aircraft other than the player, kept in the
/// header's extras because it holds for the whole flight.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Presentation {
    /// Loaded aircraft models in draw order. With none loaded every other
    /// aircraft draws with the player's airframe, as the range's fixtures do.
    pub models: Vec<AircraftId>,
    /// Aircraft ids 1 to `slots` draw with their own model; any other id has
    /// no model slot and is hidden.
    pub slots: u32,
}

impl Presentation {
    /// The rules live flight used for `snapshot`.
    pub fn of(snapshot: &RenderSnapshot) -> Self {
        Self {
            models: snapshot.models.clone(),
            slots: snapshot
                .targets
                .iter()
                .filter(|pose| matches!(pose.draw, Draw::Model(_)))
                .map(|pose| pose.id)
                .max()
                .unwrap_or(0),
        }
    }

    /// Header extras holding these rules.
    pub fn extras(&self) -> Vec<(String, String)> {
        let models: Vec<&str> = self.models.iter().map(|id| identity_key(*id)).collect();
        vec![
            (MODELS_KEY.into(), models.join(",")),
            (SLOTS_KEY.into(), self.slots.to_string()),
        ]
    }

    /// The rules a recording's header holds. A header without them reads as
    /// every aircraft drawn with the player's airframe.
    pub fn from_header(header: &replay::Header) -> Self {
        Self {
            models: header
                .extra(MODELS_KEY)
                .map(|text| text.split(',').filter_map(identity).collect())
                .unwrap_or_default(),
            slots: header
                .extra(SLOTS_KEY)
                .and_then(|text| text.parse().ok())
                .unwrap_or(0),
        }
    }

    /// How aircraft `id`, of type `aircraft`, draws.
    pub fn draw(&self, id: u32, aircraft: Option<AircraftId>) -> Draw {
        if id == 0 || self.models.is_empty() {
            Draw::Ownship
        } else if (1..=self.slots).contains(&id) {
            aircraft
                .filter(|kind| self.models.contains(kind))
                .map_or(Draw::Hidden, Draw::Model)
        } else {
            Draw::Hidden
        }
    }
}

/// Flight data a snapshot does not carry, for one aircraft and one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlightData {
    /// Airspeed, feet per second.
    pub airspeed: f64,
    /// Load factor, G.
    pub g: f64,
    pub fuel_lb: f64,
    /// Pilot controls: pitch, roll, yaw and throttle.
    pub controls: [f64; 4],
    /// Parked or rolling on the ground.
    pub on_ground: bool,
    /// Flying with a live pilot: not destroyed, crashed, ejected or dead.
    pub alive: bool,
    pub ejected: bool,
    /// The wreck has left the world.
    pub wreck_gone: bool,
}

/// One aircraft's recorded state from its drawn pose and flight data.
pub fn aircraft_state(pose: &AircraftPose, data: &FlightData) -> replay::AircraftState {
    replay::AircraftState {
        id: pose.id,
        position: pose.position,
        attitude: pose.attitude,
        velocity: pose.velocity,
        airspeed: data.airspeed,
        g: data.g,
        devices: pose.devices.unwrap_or([0.; DEVICES]),
        heat: heat(pose),
        flags: replay::AircraftFlags {
            engine_on: pose.engine.lit,
            afterburner: pose.engine.afterburner,
            airborne: pose.airborne,
            on_ground: data.on_ground,
            crashed: pose.crashed,
            wreck_gone: data.wreck_gone,
            alive: data.alive,
            ejected: data.ejected,
            animated: pose.devices.is_some(),
        },
        wreck_phase: wreck_code(pose.wreck),
        fuel_lb: data.fuel_lb,
        controls: data.controls,
        auxiliary_rates: pose.engine.rates,
        hp: pose.damage.hp,
        max_hp: pose.damage.initial_hp,
        sections: pose.damage.sections,
        structural_section: pose.damage.structural.map(|section| section as u8),
    }
}

/// The nozzle's heat input as drawn: none with the engine out, full with
/// the afterburner lit, otherwise the throttle, as the nozzle material reads
/// it. Kept for logs and Tacview; drawing uses the flags and devices.
fn heat(pose: &AircraftPose) -> f64 {
    if !pose.engine.lit {
        0.
    } else if pose.engine.afterburner {
        1.
    } else {
        pose.devices
            .map_or(0., |devices| devices[DEVICES - 1].clamp(0., 1.))
    }
}

/// A recorded aircraft as drawn. `aircraft` is its registered type and
/// `draw` comes from the recording's [`Presentation`].
pub fn aircraft_pose(
    state: &replay::AircraftState,
    aircraft: Option<AircraftId>,
    draw: Draw,
) -> AircraftPose {
    AircraftPose {
        id: state.id,
        aircraft,
        draw,
        position: state.position,
        attitude: state.attitude,
        velocity: state.velocity,
        devices: state.flags.animated.then_some(state.devices),
        engine: Engine {
            lit: state.flags.engine_on,
            afterburner: state.flags.afterburner,
            rates: state.auxiliary_rates,
        },
        damage: Damage {
            hp: state.hp,
            initial_hp: state.max_hp,
            sections: state.sections,
            structural: state.structural_section.and_then(section),
        },
        airborne: state.flags.airborne,
        wreck: wreck_phase(state.wreck_phase),
        crashed: state.flags.crashed,
    }
}

fn section(code: u8) -> Option<DamageSection> {
    Some(match code {
        0 => DamageSection::Nose,
        1 => DamageSection::Cockpit,
        2 => DamageSection::Core,
        3 => DamageSection::LeftWing,
        4 => DamageSection::RightWing,
        5 => DamageSection::Tail,
        _ => return None,
    })
}

/// Wreck phases as recorded: 0 none, 1 falling, 2 on the ground, 3 exploded.
pub fn wreck_code(phase: Option<wreck::Phase>) -> u8 {
    match phase {
        None => 0,
        Some(wreck::Phase::Falling) => 1,
        Some(wreck::Phase::Grounded) => 2,
        Some(wreck::Phase::Exploded) => 3,
    }
}

/// The wreck phase a recorded code names; codes this build does not know
/// read as no wreck.
pub fn wreck_phase(code: u8) -> Option<wreck::Phase> {
    match code {
        1 => Some(wreck::Phase::Falling),
        2 => Some(wreck::Phase::Grounded),
        3 => Some(wreck::Phase::Exploded),
        _ => None,
    }
}

/// A weapon type's registered identity. `id` is its number in this recording.
pub fn weapon_info(id: u32, weapon: &tore_formats::weapons::Weapon) -> replay::WeaponInfo {
    use tore_sim::combat::ledger::ShotKind;
    let class = if live::is_gun(weapon) {
        replay::WeaponClass::Gun
    } else {
        match ShotKind::of(weapon) {
            ShotKind::AirToAir | ShotKind::AirToGround => replay::WeaponClass::Missile,
            ShotKind::Bomb => replay::WeaponClass::Bomb,
            ShotKind::Gun => replay::WeaponClass::Gun,
            // Guided but neither air nor surface, rockets and other stores.
            ShotKind::Other if weapon.flags & 1 != 0 => replay::WeaponClass::Missile,
            ShotKind::Other => replay::WeaponClass::Other,
        }
    };
    replay::WeaponInfo {
        id,
        source: weapon.source.clone(),
        shape: weapon.shape.clone(),
        name: weapon.hud_name.clone(),
        class,
    }
}

/// A guided projectile's seeker, as recorded.
pub fn seeker(guidance: &tore_sim::combat::missiles::Flight) -> replay::Seeker {
    replay::Seeker {
        acquired: guidance.seeker.acquired,
        status: guidance.seeker.status as u8,
        quality: guidance.seeker.quality as f32,
        target: guidance.seeker.target,
    }
}

/// A projectile's recorded state. `weapon` is its registered weapon id;
/// `age` and `seeker` come from the simulation's projectile.
pub fn projectile_state(
    pose: &ProjectilePose,
    weapon: u32,
    age: u64,
    seeker: Option<replay::Seeker>,
) -> replay::ProjectileState {
    replay::ProjectileState {
        id: pose.id,
        owner: pose.owner,
        weapon,
        target: pose.target,
        position: pose.position,
        previous: pose.previous,
        direction: pose.direction,
        speed: f64::from(pose.speed_f8) / 256.,
        tracer: pose.tracer,
        incoming: pose.incoming,
        age: u32::try_from(age).unwrap_or(u32::MAX),
        seeker,
    }
}

/// A recorded projectile as drawn. A weapon the recording never registered
/// draws as a plain strip.
pub fn projectile_pose(
    state: &replay::ProjectileState,
    weapon: Option<&replay::WeaponInfo>,
) -> ProjectilePose {
    ProjectilePose {
        id: state.id,
        owner: state.owner,
        weapon: weapon.map(|w| w.source.clone()).unwrap_or_default(),
        shape: weapon.and_then(|w| w.shape.clone()),
        gun: weapon.is_some_and(|w| w.class == replay::WeaponClass::Gun),
        tracer: state.tracer,
        position: state.position,
        previous: state.previous,
        direction: state.direction,
        target: state.target,
        incoming: state.incoming,
        speed_f8: (state.speed * 256.).round() as i32,
    }
}

/// Debris pieces as recorded. Each piece's index counts its owner's pieces
/// in list order.
pub fn debris_states(debris: &[DebrisPose]) -> Vec<replay::DebrisState> {
    let mut counts: BTreeMap<u32, u32> = BTreeMap::new();
    debris
        .iter()
        .map(|piece| {
            let index = counts.entry(piece.owner).or_default();
            let state = replay::DebrisState {
                owner: piece.owner,
                index: *index,
                position: piece.position,
                attitude: piece.attitude,
            };
            *index += 1;
            state
        })
        .collect()
}

/// A recorded debris piece as drawn. `variant` is the owner's structural
/// section on that tick.
pub fn debris_pose(state: &replay::DebrisState, draw: Draw, variant: Option<usize>) -> DebrisPose {
    DebrisPose {
        owner: state.owner,
        draw,
        position: state.position,
        attitude: state.attitude,
        variant,
    }
}

/// Ejection phases as recorded: seat, free fall, inflating, parachute,
/// landed and impact, 0 to 5.
pub fn escape_code(phase: ejection::Phase) -> u8 {
    match phase {
        ejection::Phase::Seat => 0,
        ejection::Phase::Freefall => 1,
        ejection::Phase::Inflating => 2,
        ejection::Phase::Parachute => 3,
        ejection::Phase::Landed => 4,
        ejection::Phase::Impact => 5,
    }
}

/// The ejection phase a recorded code names; an unknown code reads as the
/// open parachute.
pub fn escape_phase(code: u8) -> ejection::Phase {
    match code {
        0 => ejection::Phase::Seat,
        1 => ejection::Phase::Freefall,
        2 => ejection::Phase::Inflating,
        4 => ejection::Phase::Landed,
        5 => ejection::Phase::Impact,
        _ => ejection::Phase::Parachute,
    }
}

pub fn escapee_state(pilot: &PilotPose) -> replay::EscapeeState {
    replay::EscapeeState {
        owner: pilot.owner,
        position: pilot.position,
        heading: pilot.heading,
        phase: escape_code(pilot.phase),
    }
}

pub fn pilot_pose(state: &replay::EscapeeState) -> PilotPose {
    PilotPose {
        owner: state.owner,
        position: state.position,
        heading: state.heading,
        phase: escape_phase(state.phase),
    }
}

pub fn effect_kind(kind: live::EffectKind) -> replay::EffectKind {
    match kind {
        live::EffectKind::Flare => replay::EffectKind::Flare,
        live::EffectKind::Chaff => replay::EffectKind::Chaff,
        live::EffectKind::Launch => replay::EffectKind::Launch,
        live::EffectKind::Hit => replay::EffectKind::Hit,
        live::EffectKind::Destroyed => replay::EffectKind::Destroyed,
        live::EffectKind::Ground => replay::EffectKind::Ground,
        live::EffectKind::DebrisImpact => replay::EffectKind::DebrisImpact,
    }
}

fn live_effect_kind(kind: replay::EffectKind) -> Option<live::EffectKind> {
    Some(match kind {
        replay::EffectKind::Flare => live::EffectKind::Flare,
        replay::EffectKind::Chaff => live::EffectKind::Chaff,
        replay::EffectKind::Launch => live::EffectKind::Launch,
        replay::EffectKind::Hit => live::EffectKind::Hit,
        replay::EffectKind::Destroyed => live::EffectKind::Destroyed,
        replay::EffectKind::Ground => live::EffectKind::Ground,
        replay::EffectKind::DebrisImpact => live::EffectKind::DebrisImpact,
        replay::EffectKind::Other(_) => return None,
    })
}

/// One chaff cartridge or flare as a recording keeps it: the releasing
/// aircraft, the kind, the aircraft's exact position, velocity and attitude
/// when it left, and its number, which sets its look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceRelease {
    pub owner: u32,
    /// [`live::EffectKind::Chaff`] or [`live::EffectKind::Flare`].
    pub kind: live::EffectKind,
    pub release: tore_sim::combat::countermeasures::Release,
    pub number: u64,
}

/// `chaff` or `flare`, as the `decoy` field names them.
pub fn decoy_name(kind: live::EffectKind) -> &'static str {
    if kind == live::EffectKind::Chaff {
        "chaff"
    } else {
        "flare"
    }
}

/// The `combat.countermeasure` entry for a released device. Every number is
/// kept exactly, so [`device_release`] gives back the same release and a
/// replay flies the device as combat flew it.
pub fn device_event(device: &DeviceRelease, left: Option<u32>) -> replay::Event {
    use replay::vocab::field;
    let tore_sim::combat::countermeasures::Release {
        position,
        velocity,
        basis,
    } = device.release;
    let mut event = replay::Event::new(replay::vocab::kind::COMBAT_COUNTERMEASURE)
        .with_subject(device.owner)
        .with(field::DECOY, decoy_name(device.kind))
        .with(field::NUMBER, device.number as i64);
    if let Some(left) = left {
        event = event.with(field::LEFT, i64::from(left));
    }
    let numbers = position
        .into_iter()
        .chain(velocity)
        .chain(basis.right)
        .chain(basis.up)
        .chain(basis.forward);
    let names = field::POSITION
        .into_iter()
        .chain(field::VELOCITY)
        .chain(field::BASIS);
    for (name, value) in names.zip(numbers) {
        event = event.with(name, value);
    }
    event.with_text(format!("released {}", decoy_name(device.kind)))
}

/// A recorded `combat.countermeasure` entry back into its release; `None`
/// when a value is missing.
pub fn device_release(event: &replay::Event) -> Option<DeviceRelease> {
    use replay::vocab::field;
    if event.kind != replay::vocab::kind::COMBAT_COUNTERMEASURE {
        return None;
    }
    let kind = match event.string(field::DECOY)? {
        "chaff" => live::EffectKind::Chaff,
        "flare" => live::EffectKind::Flare,
        _ => return None,
    };
    let vector = |names: &[&str]| -> Option<[f64; 3]> {
        Some([
            event.num(names[0])?,
            event.num(names[1])?,
            event.num(names[2])?,
        ])
    };
    Some(DeviceRelease {
        owner: event.subject?,
        kind,
        release: tore_sim::combat::countermeasures::Release {
            position: vector(&field::POSITION)?,
            velocity: vector(&field::VELOCITY)?,
            basis: tore_sim::attitude::Basis {
                right: vector(&field::BASIS[0..3])?,
                up: vector(&field::BASIS[3..6])?,
                forward: vector(&field::BASIS[6..9])?,
            },
        },
        number: u64::try_from(event.get(field::NUMBER)?.as_i64()?).ok()?,
    })
}

/// A playing effect as drawn, from its recorded start.
pub fn effect_pose(effect: &replay::LiveEffect) -> Option<EffectPose> {
    let left = u64::from(effect.duration_ticks).checked_sub(effect.age_ticks)?;
    Some(EffectPose {
        kind: live_effect_kind(effect.kind)?,
        position: effect.position,
        ticks: u16::try_from(left).ok().filter(|left| *left > 0)?,
    })
}

/// Finds the effects that started since the previous tick. The simulation
/// ages every effect by one tick, drops the ones that ran out, appends new
/// ones and drops the oldest at its cap, so last tick's survivors are a
/// prefix of this tick's list; whatever follows them is new.
#[derive(Clone, Debug, Default)]
pub struct EffectWatch {
    previous: Vec<EffectPose>,
}

impl EffectWatch {
    /// Effects in `current` that were not playing on the previous tick, as
    /// spawns lasting the ticks they have left, so a replay ages them
    /// exactly as the game does.
    pub fn started(&mut self, current: &[EffectPose]) -> Vec<replay::EffectSpawn> {
        let survivors: Vec<EffectPose> = self
            .previous
            .iter()
            .filter(|effect| effect.ticks > 1)
            .map(|effect| EffectPose {
                ticks: effect.ticks - 1,
                ..*effect
            })
            .collect();
        let same = |a: &EffectPose, b: &EffectPose| {
            a.kind == b.kind
                && a.ticks == b.ticks
                && a.position.map(f64::to_bits) == b.position.map(f64::to_bits)
        };
        let kept = (0..=survivors.len())
            .map(|dropped| &survivors[dropped..])
            .find(|tail| {
                tail.len() <= current.len() && tail.iter().zip(current).all(|(a, b)| same(a, b))
            })
            .map_or(0, <[EffectPose]>::len);
        self.previous = current.to_vec();
        current[kept..]
            .iter()
            .map(|effect| replay::EffectSpawn {
                kind: effect_kind(effect.kind),
                position: effect.position,
                duration_ticks: u32::from(effect.ticks),
            })
            .collect()
    }
}

/// Aircraft types and weapons a recording registered, by id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Identities {
    pub aircraft: BTreeMap<u32, AircraftId>,
    pub weapons: BTreeMap<u32, replay::WeaponInfo>,
}

impl Identities {
    pub fn of(recording: &replay::Recording) -> Self {
        Self {
            aircraft: recording
                .aircraft()
                .filter_map(|info| Some((info.id, identity(&info.pt)?)))
                .collect(),
            weapons: recording
                .weapons()
                .map(|info| (info.id, info.clone()))
                .collect(),
        }
    }
}

/// Longest an effect plays: an aircraft explosion's 240 ticks. Pass it to
/// [`replay::Recording::live_effects`] as the look-back.
pub const EFFECT_LOOKBACK_TICKS: u64 = 240;

/// The picture one recorded frame draws. `effects` are the effects still
/// playing at the frame's tick, from [`replay::Recording::live_effects`] with
/// [`EFFECT_LOOKBACK_TICKS`]; everything else comes from the frame.
pub fn snapshot(
    frame: &replay::Frame,
    effects: &[replay::LiveEffect],
    presentation: &Presentation,
    identities: &Identities,
) -> RenderSnapshot {
    let kind = |id: u32| identities.aircraft.get(&id).copied();
    let pose = |state: &replay::AircraftState| {
        aircraft_pose(
            state,
            kind(state.id),
            presentation.draw(state.id, kind(state.id)),
        )
    };
    let structural = |owner: u32| {
        frame
            .aircraft
            .iter()
            .find(|state| state.id == owner)
            .and_then(|state| state.structural_section)
            .map(usize::from)
    };
    let mut effects: Vec<EffectPose> = effects.iter().filter_map(effect_pose).collect();
    // The simulation keeps only its newest effects.
    if effects.len() > live::MAX_EFFECTS {
        effects.drain(..effects.len() - live::MAX_EFFECTS);
    }
    RenderSnapshot {
        tick: frame.tick,
        player: frame
            .aircraft
            .iter()
            .find(|state| state.id == 0)
            .map(pose)
            .unwrap_or_default(),
        targets: frame
            .aircraft
            .iter()
            .filter(|state| state.id != 0)
            .map(pose)
            .collect(),
        projectiles: frame
            .projectiles
            .iter()
            .map(|p| projectile_pose(p, identities.weapons.get(&p.weapon)))
            .collect(),
        effects,
        debris: frame
            .debris
            .iter()
            .map(|piece| {
                debris_pose(
                    piece,
                    presentation.draw(piece.owner, kind(piece.owner)),
                    structural(piece.owner),
                )
            })
            .collect(),
        pilots: frame.escapees.iter().map(pilot_pose).collect(),
        models: presentation.models.clone(),
    }
}

/// Worst allowed difference for each quantity: half a step plus rounding.
mod tolerance {
    use tore_replay::precision::*;
    const SLACK: f64 = 1e-9;
    pub const POSITION: f64 = POSITION_FT / 2. + SLACK;
    pub const ANGLE: f64 = ANGLE_RAD / 2. + SLACK;
    pub const VELOCITY: f64 = VELOCITY_FPS / 2. + SLACK;
    pub const UNIT_DEVICE: f64 = UNIT / 2. + SLACK;
    pub const SIGNED_DEVICE: f64 = SIGNED / 2. + SLACK;
    pub const SPEED: f64 = SPEED_FPS / 2. + SLACK;
    pub const RATE: f64 = RATE_RAD_S / 2. + SLACK;
    /// Direction components, from the two quantized direction angles.
    pub const DIRECTION: f64 = 2e-5;
    /// Speed in the simulation's 1/256 ft/s steps, after rounding back.
    pub const SPEED_F8: i32 = (SPEED_FPS * 256. / 2.) as i32 + 1;
}

/// True when any component differs by more than `limit`, or is not a number.
fn far(a: [f64; 3], b: [f64; 3], limit: f64) -> bool {
    !(0..3).all(|i| (a[i] - b[i]).abs() <= limit)
}

/// True when two angles differ by more than the angle precision, the short
/// way round.
fn angle_far(a: f64, b: f64) -> bool {
    use std::f64::consts::{PI, TAU};
    let near = ((a - b + PI).rem_euclid(TAU) - PI).abs() <= tolerance::ANGLE;
    !near
}

fn aircraft_difference(live: &AircraftPose, replayed: &AircraftPose) -> Option<String> {
    let who = if live.id == 0 {
        "the player".to_owned()
    } else {
        format!("aircraft {}", live.id)
    };
    if far(live.position, replayed.position, tolerance::POSITION) {
        return Some(format!(
            "{who} is at {:?}, recorded {:?}",
            live.position, replayed.position
        ));
    }
    if (0..3).any(|i| angle_far(live.attitude[i], replayed.attitude[i])) {
        return Some(format!(
            "{who} has attitude {:?}, recorded {:?}",
            live.attitude, replayed.attitude
        ));
    }
    if far(live.velocity, replayed.velocity, tolerance::VELOCITY) {
        return Some(format!(
            "{who} moves at {:?}, recorded {:?}",
            live.velocity, replayed.velocity
        ));
    }
    match (live.devices, replayed.devices) {
        (None, None) => {}
        (Some(a), Some(b)) => {
            for slot in 0..DEVICES {
                let (limit, low, high) = match slot {
                    6..=8 => (tolerance::SIGNED_DEVICE, -1., 1.),
                    9 => (tolerance::SPEED, f64::MIN, f64::MAX),
                    _ => (tolerance::UNIT_DEVICE, 0., 1.),
                };
                // Change records keep devices within their range; key
                // records keep the exact value.
                let near = |truth: f64| (truth - b[slot]).abs() <= limit;
                if !near(a[slot]) && !near(a[slot].clamp(low, high)) {
                    return Some(format!(
                        "{who} has device {slot} at {}, recorded {}",
                        a[slot], b[slot]
                    ));
                }
            }
        }
        _ => {
            return Some(format!(
                "{who} has animated devices {}, recorded {}",
                live.devices.is_some(),
                replayed.devices.is_some()
            ));
        }
    }
    if live.engine.lit != replayed.engine.lit
        || live.engine.afterburner != replayed.engine.afterburner
        || far(live.engine.rates, replayed.engine.rates, tolerance::RATE)
    {
        return Some(format!(
            "{who} has engine {:?}, recorded {:?}",
            live.engine, replayed.engine
        ));
    }
    if (
        live.aircraft,
        live.draw,
        live.damage,
        live.airborne,
        live.wreck,
        live.crashed,
    ) != (
        replayed.aircraft,
        replayed.draw,
        replayed.damage,
        replayed.airborne,
        replayed.wreck,
        replayed.crashed,
    ) {
        return Some(format!(
            "{who} differs: live {:?} {:?} {:?} airborne={} wreck={:?} crashed={}, recorded {:?} {:?} {:?} airborne={} wreck={:?} crashed={}",
            live.aircraft,
            live.draw,
            live.damage,
            live.airborne,
            live.wreck,
            live.crashed,
            replayed.aircraft,
            replayed.draw,
            replayed.damage,
            replayed.airborne,
            replayed.wreck,
            replayed.crashed
        ));
    }
    None
}

/// The first difference between a live snapshot and the one a recording
/// rebuilt, beyond the format's documented precision, in plain words.
/// `None` means both draw the same picture. Ground objects are left out:
/// a recording keeps only their hit points, and they are never drawn.
pub fn difference(live: &RenderSnapshot, replayed: &RenderSnapshot) -> Option<String> {
    if live.tick != replayed.tick {
        return Some(format!(
            "tick {} was recorded as {}",
            live.tick, replayed.tick
        ));
    }
    if let Some(d) = aircraft_difference(&live.player, &replayed.player) {
        return Some(d);
    }
    let aircraft: Vec<&AircraftPose> = live
        .targets
        .iter()
        .filter(|pose| pose.aircraft.is_some())
        .collect();
    if aircraft.len() != replayed.targets.len() {
        return Some(format!(
            "{} aircraft live, {} recorded",
            aircraft.len(),
            replayed.targets.len()
        ));
    }
    for (a, b) in aircraft.into_iter().zip(&replayed.targets) {
        if a.id != b.id {
            return Some(format!("aircraft {} was recorded as {}", a.id, b.id));
        }
        if let Some(d) = aircraft_difference(a, b) {
            return Some(d);
        }
    }
    if live.projectiles.len() != replayed.projectiles.len() {
        return Some(format!(
            "{} projectiles live, {} recorded",
            live.projectiles.len(),
            replayed.projectiles.len()
        ));
    }
    for (a, b) in live.projectiles.iter().zip(&replayed.projectiles) {
        if (
            a.id, a.owner, &a.weapon, &a.shape, a.gun, a.tracer, a.target, a.incoming,
        ) != (
            b.id, b.owner, &b.weapon, &b.shape, b.gun, b.tracer, b.target, b.incoming,
        ) || far(a.position, b.position, tolerance::POSITION)
            || far(a.previous, b.previous, tolerance::POSITION)
            || far(a.direction, b.direction, tolerance::DIRECTION)
            || (a.speed_f8 - b.speed_f8).abs() > tolerance::SPEED_F8
        {
            return Some(format!(
                "projectile {} differs: live {a:?}, recorded {b:?}",
                a.id
            ));
        }
    }
    if live.effects.len() != replayed.effects.len() {
        return Some(format!(
            "{} effects live, {} recorded",
            live.effects.len(),
            replayed.effects.len()
        ));
    }
    for (a, b) in live.effects.iter().zip(&replayed.effects) {
        if a.kind != b.kind
            || a.ticks != b.ticks
            || far(a.position, b.position, tolerance::POSITION)
        {
            return Some(format!("effect differs: live {a:?}, recorded {b:?}"));
        }
    }
    if live.debris.len() != replayed.debris.len() {
        return Some(format!(
            "{} debris pieces live, {} recorded",
            live.debris.len(),
            replayed.debris.len()
        ));
    }
    for (a, b) in live.debris.iter().zip(&replayed.debris) {
        if (a.owner, a.draw, a.variant) != (b.owner, b.draw, b.variant)
            || far(a.position, b.position, tolerance::POSITION)
            || (0..3).any(|i| angle_far(a.attitude[i], b.attitude[i]))
        {
            return Some(format!("debris differs: live {a:?}, recorded {b:?}"));
        }
    }
    if live.pilots.len() != replayed.pilots.len() {
        return Some(format!(
            "{} ejected pilots live, {} recorded",
            live.pilots.len(),
            replayed.pilots.len()
        ));
    }
    for (a, b) in live.pilots.iter().zip(&replayed.pilots) {
        if (a.owner, a.phase) != (b.owner, b.phase)
            || far(a.position, b.position, tolerance::POSITION)
            || angle_far(a.heading, b.heading)
        {
            return Some(format!("ejected pilot differs: live {a:?}, recorded {b:?}"));
        }
    }
    if live.models != replayed.models {
        return Some(format!(
            "models {:?} were recorded as {:?}",
            live.models, replayed.models
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::render_hash_tests as fixture;
    use crate::render_snapshot::{aircraft_batches, combat_geometry, interpolate, pose_state};
    use crate::replay::tests::TempDir;
    use std::path::Path;

    #[test]
    fn identities_round_trip_without_aliasing_a_variant() {
        for id in AircraftId::SELECTABLE {
            assert_eq!(identity(identity_key(id)), Some(id), "{id:?}");
        }
        // The F/A-XX borrows the F-22N's resources but is never recorded as one.
        assert_ne!(
            identity_key(AircraftId::Faxx),
            identity_key(AircraftId::F22n)
        );
        assert_eq!(identity_key(AircraftId::F18), "F18.PT");
        assert_eq!(identity_key(AircraftId::Rafale), "RAFALE.PT");
        assert_eq!(identity(""), None);
    }

    #[test]
    fn presentation_survives_the_header_and_routes_every_draw_mode() {
        let rules = Presentation {
            models: vec![AircraftId::Mig29, AircraftId::F18, AircraftId::Faxx],
            slots: 5,
        };
        let header = replay::Header {
            extra: rules.extras(),
            ..Default::default()
        };
        assert_eq!(Presentation::from_header(&header), rules);
        assert_eq!(rules.draw(0, Some(AircraftId::F18)), Draw::Ownship);
        assert_eq!(
            rules.draw(3, Some(AircraftId::Mig29)),
            Draw::Model(AircraftId::Mig29)
        );
        assert_eq!(rules.draw(6, Some(AircraftId::Mig29)), Draw::Hidden);
        assert_eq!(rules.draw(2, Some(AircraftId::Su27)), Draw::Hidden);
        assert_eq!(rules.draw(2, None), Draw::Hidden);
        // No models: the range's fixtures draw with the player's airframe.
        let range = Presentation::from_header(&replay::Header::default());
        assert_eq!(range, Presentation::default());
        assert_eq!(range.draw(4, Some(AircraftId::F18)), Draw::Ownship);
    }

    fn effect(kind: live::EffectKind, x: f64, ticks: u16) -> EffectPose {
        EffectPose {
            kind,
            position: [x, 5000., 0.],
            ticks,
        }
    }

    #[test]
    fn effects_are_recorded_once_when_they_start() {
        use live::EffectKind::{Destroyed, Flare, Hit};
        let mut watch = EffectWatch::default();
        let first = vec![effect(Hit, 1., 45), effect(Flare, 2., 2)];
        let started = watch.started(&first);
        assert_eq!(started.len(), 2);
        assert_eq!(started[1].duration_ticks, 2);
        // One tick on: one effect was added between ticks (already a tick
        // old) and one during this tick.
        let second = vec![
            effect(Hit, 1., 44),
            effect(Flare, 2., 1),
            effect(Destroyed, 3., 239),
            effect(Hit, 4., 45),
        ];
        let started = watch.started(&second);
        assert_eq!(
            started
                .iter()
                .map(|s| (s.position[0], s.duration_ticks))
                .collect::<Vec<_>>(),
            [(3., 239), (4., 45)]
        );
        // The flare ran out and the list hit its cap: the oldest was dropped.
        let third = vec![
            effect(Destroyed, 3., 238),
            effect(Hit, 4., 44),
            effect(Hit, 5., 45),
        ];
        let started = watch.started(&third);
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].position[0], 5.);
        // Nothing new.
        let fourth: Vec<_> = third
            .iter()
            .map(|e| EffectPose {
                ticks: e.ticks - 1,
                ..*e
            })
            .collect();
        assert!(watch.started(&fourth).is_empty());
    }

    /// Records a pair of snapshots as two consecutive ticks, the way the
    /// recorder does, and rebuilds them from the file.
    fn round_trip(dir: &Path, name: &str, live: &[RenderSnapshot; 2]) -> [RenderSnapshot; 2] {
        let presentation = Presentation::of(&live[1]);
        let header = replay::Header {
            extra: presentation.extras(),
            ..Default::default()
        };
        let mut writer = replay::Writer::create(dir.join(name), &header).unwrap();
        let mut weapons: BTreeMap<String, u32> = BTreeMap::new();
        let mut effects = EffectWatch::default();
        for snapshot in live {
            let aircraft: Vec<&AircraftPose> = std::iter::once(&snapshot.player)
                .chain(&snapshot.targets)
                .filter(|pose| pose.aircraft.is_some())
                .collect();
            for pose in &aircraft {
                writer
                    .register_aircraft(&replay::AircraftInfo {
                        id: pose.id,
                        pt: identity_key(pose.aircraft.unwrap()).into(),
                        ..Default::default()
                    })
                    .unwrap();
            }
            let data = FlightData {
                airspeed: 700.,
                g: 1.5,
                fuel_lb: 5_000.,
                controls: [0.1, -0.2, 0., 0.8],
                alive: true,
                ..Default::default()
            };
            let mut projectiles = Vec::new();
            for p in &snapshot.projectiles {
                let next = weapons.len() as u32;
                let id = *weapons.entry(p.weapon.clone()).or_insert(next);
                writer
                    .register_weapon(&replay::WeaponInfo {
                        id,
                        source: p.weapon.clone(),
                        shape: p.shape.clone(),
                        name: p.weapon.clone(),
                        class: if p.gun {
                            replay::WeaponClass::Gun
                        } else {
                            replay::WeaponClass::Missile
                        },
                    })
                    .unwrap();
                projectiles.push(projectile_state(p, id, 3, None));
            }
            writer
                .push(&replay::Frame {
                    tick: snapshot.tick,
                    aircraft: aircraft
                        .iter()
                        .map(|pose| aircraft_state(pose, &data))
                        .collect(),
                    projectiles,
                    debris: debris_states(&snapshot.debris),
                    escapees: snapshot.pilots.iter().map(escapee_state).collect(),
                    new_effects: effects.started(&snapshot.effects),
                    ..Default::default()
                })
                .unwrap();
        }
        let path = writer.finish(&replay::Footer::default()).unwrap();
        let recording = replay::Recording::open(path).unwrap();
        assert!(recording.complete() && recording.problems().is_empty());
        let identities = Identities::of(&recording);
        let presentation = Presentation::from_header(recording.header());
        live.each_ref().map(|live| {
            let frame = recording.frame(live.tick).unwrap().unwrap();
            let effects = recording
                .live_effects(live.tick, EFFECT_LOOKBACK_TICKS)
                .unwrap();
            snapshot(&frame, &effects, &presentation, &identities)
        })
    }

    /// Vertex streams drawn from a live and a replayed picture agree: the
    /// same vertices in the same order, each value within the position
    /// precision of 1/64 ft (quantized angles and devices move a vertex far
    /// less than that).
    fn assert_same_vertices(what: &str, live: &[f32], replayed: &[f32]) {
        assert_eq!(live.len(), replayed.len(), "{what}: vertex counts differ");
        let worst = live
            .iter()
            .zip(replayed)
            .map(|(a, b)| (a - b).abs() / (1. + a.abs() * 1e-4))
            .fold(0f32, f32::max);
        let bound = (tore_replay::precision::POSITION_FT / 2.) as f32 + 1e-3;
        assert!(worst < bound, "{what}: vertices differ by {worst}");
    }

    #[test]
    fn recorded_frames_draw_the_live_picture() {
        let dir = TempDir::new("convert");
        let ownship = fixture::hornet_airframe(true);
        let player = fixture::player();
        let world = crate::terrain::tests::world();
        let art = fixture::escape_art();
        let mut drawn = 0;
        for with_models in [true, false] {
            let mut combat = if with_models {
                fixture::combat(
                    fixture::models(),
                    (0..7).map(|i| (i % 3, [0.; 3])).collect(),
                )
            } else {
                fixture::combat(Vec::new(), Vec::new())
            };
            let scene = fixture::scene(combat.state.configuration());
            for ai_poses in [true, false] {
                let mut live = fixture::snapshots(&mut combat, &scene, ai_poses, &player);
                for (tick, snapshot) in (40..).zip(&mut live) {
                    snapshot.tick = tick;
                }
                // The scene's effects start on its current tick. (On the
                // second pass the reused combat state would otherwise show
                // them un-aged on the previous tick too, which live flight
                // never does: every effect ages each tick.)
                live[0].effects.clear();
                // Every ejection phase, the player's first.
                live[1].pilots = fixture::pilots()
                    .iter()
                    .enumerate()
                    .map(|(owner, escape)| PilotPose {
                        owner: owner as u32,
                        position: escape.position,
                        heading: escape.heading,
                        phase: escape.phase,
                    })
                    .collect();
                // Live flight sets the player's damage variant from the combat
                // state's structural section every tick; the fixture's flight
                // state and combat state are separate, so align them.
                for snapshot in &mut live {
                    snapshot.player.damage.structural = player
                        .damage_variant
                        .and_then(|variant| section(variant as u8));
                }
                // A falling player wreck.
                let player_pose = &mut live[1].player;
                player_pose.wreck = Some(wreck::Phase::Falling);
                player_pose.crashed = true;
                player_pose.damage.hp = 0;
                let name = format!("scene-{with_models}-{ai_poses}.tore-replay");
                let replayed = round_trip(dir.path(), &name, &live);
                for (a, b) in live.iter().zip(&replayed) {
                    assert_eq!(difference(a, b), None);
                }
                // The drawable player state a replay rebuilds.
                let [a, b] = [&live[1].player, &replayed[1].player].map(|p| pose_state(&player, p));
                assert_eq!(
                    (
                        a.engine,
                        a.burner,
                        a.crashed,
                        a.damage_variant,
                        a.damage_regions
                    ),
                    (
                        b.engine,
                        b.burner,
                        b.crashed,
                        b.damage_variant,
                        b.damage_regions
                    )
                );
                assert!(!far(a.auxiliary_rates, b.auxiliary_rates, tolerance::RATE));
                for alpha in [0., 0.37, 1.] {
                    let [a, b] =
                        [&live, &replayed].map(|pair| interpolate(Some(&pair[0]), &pair[1], alpha));
                    for camera in fixture::cameras() {
                        if with_models {
                            let batches = [&a, &b]
                                .map(|s| aircraft_batches(s, combat.models(), &camera, &world));
                            assert_eq!(batches[0].len(), batches[1].len());
                            for ((ma, va, ca), (mb, vb, cb)) in batches[0].iter().zip(&batches[1]) {
                                assert_eq!(ma.profile.id, mb.profile.id);
                                assert_same_vertices("model batch", va, vb);
                                assert_eq!(ca.len(), cb.len());
                                drawn += va.len();
                            }
                        }
                        let [ga, gb] = [&a, &b].map(|s| {
                            combat_geometry(s, &combat.art, &ownship, &player, &camera, &world)
                        });
                        assert_same_vertices("combat geometry", &ga.vertices, &gb.vertices);
                        assert_eq!(ga.contacts.len(), gb.contacts.len());
                        drawn += ga.vertices.len();
                        let [pa, pb] = [&a, &b].map(|s| {
                            art.vertices_for(
                                s.pilots.iter().map(|p| (p.position, p.heading, p.phase)),
                                &ownship.palette,
                                camera.position.map(f64::from),
                            )
                        });
                        assert_same_vertices("ejected pilots", &pa, &pb);
                    }
                }
            }
        }
        // The scene really drew models, fixtures, weapons and effects.
        assert!(drawn > 100_000, "{drawn}");
    }

    #[test]
    fn a_frame_rebuilds_every_state_it_can_hold() {
        use tore_sim::ejection::Phase;
        let pose = AircraftPose {
            id: 3,
            aircraft: Some(AircraftId::Mig29),
            draw: Draw::Model(AircraftId::Mig29),
            position: [1., 2., 3.],
            attitude: [0.5, -0.2, 3.],
            velocity: [100., 0., -200.],
            devices: Some([1., 0.5, 0.25, 0., 1., 0.75, -0.5, 0.5, 1., 812., 0.9]),
            engine: Engine {
                lit: true,
                afterburner: true,
                rates: [0.1, -0.4, 0.2],
            },
            damage: Damage {
                hp: 12,
                initial_hp: 90,
                sections: [1, 2, 3, 4, 5, 6],
                structural: Some(DamageSection::RightWing),
            },
            airborne: true,
            wreck: Some(wreck::Phase::Grounded),
            crashed: true,
        };
        for phase in [
            None,
            Some(wreck::Phase::Falling),
            Some(wreck::Phase::Grounded),
            Some(wreck::Phase::Exploded),
        ] {
            assert_eq!(wreck_phase(wreck_code(phase)), phase);
        }
        for phase in [
            Phase::Seat,
            Phase::Freefall,
            Phase::Inflating,
            Phase::Parachute,
            Phase::Landed,
            Phase::Impact,
        ] {
            assert_eq!(escape_phase(escape_code(phase)), phase);
        }
        let data = FlightData {
            airspeed: 820.,
            g: 4.5,
            fuel_lb: 3_000.,
            controls: [0.5, -0.5, 0.1, 1.],
            on_ground: false,
            alive: false,
            ejected: true,
            wreck_gone: false,
        };
        let state = aircraft_state(&pose, &data);
        assert_eq!(state.heat, 1.);
        assert!(state.flags.animated && state.flags.ejected && !state.flags.alive);
        assert_eq!(aircraft_pose(&state, pose.aircraft, pose.draw), pose);
        // A fixture nothing animates keeps the neutral pose.
        let fixture = AircraftPose {
            devices: None,
            ..pose.clone()
        };
        let state = aircraft_state(&fixture, &data);
        assert!(!state.flags.animated);
        assert_eq!(state.devices, [0.; DEVICES]);
        assert_eq!(
            aircraft_pose(&state, pose.aircraft, pose.draw).devices,
            None
        );
    }
}
