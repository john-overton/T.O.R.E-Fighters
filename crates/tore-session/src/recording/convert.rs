//! What combat draws, converted into mission recording frames and back.
//!
//! The recorder turns each tick's [`RenderSnapshot`], plus the flight data a
//! snapshot does not carry ([`FlightData`]), into `tore_replay` frame pieces.
//! A replay turns a decoded frame back into a snapshot with [`snapshot`] and
//! draws it through the same the game renderer helpers live flight
//! uses, so the picture matches within the format's documented precision
//! (docs/REPLAYS.md). [`difference`] checks that claim tick by tick.
//!
//! The draw rules a snapshot carries per aircraft are the same for a whole
//! recording, so they live in the header as a [`Presentation`]. Ground
//! objects are not aircraft: a recording keeps only their hit points, except
//! the surface units that follow a route, whose poses it keeps (format 3).
// The recorder uses the recording half and the replay viewer the rest.
#![allow(dead_code)]
use std::collections::BTreeMap;
use tore_formats::aircraft::AircraftId;
use tore_replay as replay;
use tore_sim::{
    combat::{
        blast,
        live::{self, DamageSection},
    },
    ejection, wreck,
};
use tore_world::snapshot::{
    AircraftPose, DEVICES, Damage, DebrisPose, Draw, EffectPose, Engine, MarkPose, PilotPose,
    ProjectilePose, RenderSnapshot,
};
use tore_world::surface::{SurfacePose, UnitId};

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

/// The aircraft type whose display name is exactly `name`: an identity
/// for an aircraft recorded without its PT name (the air-to-ground driver's
/// two-seat buddy runs and its A-10 record the player's type by name only;
/// nr-replayviews). An empty or unknown name is no identity.
pub fn by_label(name: &str) -> Option<AircraftId> {
    if name.trim().is_empty() {
        return None;
    }
    AircraftId::SELECTABLE
        .into_iter()
        .chain(AircraftId::ALL)
        .find(|id| id.label() == name)
}

/// Header extra listing the loaded aircraft models in draw order.
pub const MODELS_KEY: &str = "draw.models";
/// Header extra: aircraft up to this id draw with their own model.
pub const SLOTS_KEY: &str = "draw.slots";
/// Header extra: the recording's player is this plane. Absent when it is
/// plane 0, single player's, so those recordings are as they always were.
pub const PLAYER_KEY: &str = "draw.player";
/// The player of a recording that has none, an observer's: no plane has
/// this id, so nothing is drawn as the player's aircraft. It is what a
/// networked observer's frames carry as their own plane.
pub const NO_PLAYER: u32 = u32::MAX;

/// How a recording draws the aircraft other than the player, kept in the
/// header's extras because it holds for the whole flight.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Presentation {
    /// Loaded aircraft models in draw order. With none loaded every other
    /// aircraft draws with the player's airframe, as the range's fixtures do.
    pub models: Vec<AircraftId>,
    /// Aircraft ids up to `slots` (the player's aside) draw with their own
    /// model; any other id has no model slot and is hidden.
    pub slots: u32,
    /// The plane the recording is for, drawn with the player's airframe and
    /// shown as the snapshot's player. Plane 0 unless the recorded seat flies
    /// another.
    pub player: u32,
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
            player: 0,
        }
    }

    /// The same rules for a recording whose player is `plane`.
    pub fn with_player(self, plane: u32) -> Self {
        Self {
            player: plane,
            ..self
        }
    }

    /// Header extras holding these rules.
    pub fn extras(&self) -> Vec<(String, String)> {
        let models: Vec<&str> = self.models.iter().map(|id| identity_key(*id)).collect();
        let mut extras = vec![
            (MODELS_KEY.into(), models.join(",")),
            (SLOTS_KEY.into(), self.slots.to_string()),
        ];
        if self.player != 0 {
            extras.push((PLAYER_KEY.into(), self.player.to_string()));
        }
        extras
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
            player: header
                .extra(PLAYER_KEY)
                .and_then(|text| text.parse().ok())
                .unwrap_or(0),
        }
    }

    /// How aircraft `id`, of type `aircraft`, draws.
    pub fn draw(&self, id: u32, aircraft: Option<AircraftId>) -> Draw {
        if id == self.player || self.models.is_empty() {
            Draw::Ownship
        } else if id <= self.slots {
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
            flame: pose.engine.flame,
        },
        wreck_phase: wreck_code(pose.wreck),
        fuel_lb: data.fuel_lb,
        controls: data.controls,
        auxiliary_rates: pose.engine.rates,
        rotor_speed: pose.engine.rotor,
        // Real disks stay under 0.6 rad; the clamp keeps a runaway number
        // from making the writer refuse the whole frame.
        disk_tilt: pose
            .engine
            .rotor_tilt
            .map(|tilt| tilt.map(|v| v.clamp(-1.5, 1.5))),
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
        pose.devices.map_or(0., |devices| {
            devices[replay::device::THROTTLE].clamp(0., 1.)
        })
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
            rotor: state.rotor_speed,
            // Playback integrates the recorded rotor speed into the blade
            // angle; the disk tilt is recorded as it was flown.
            rotor_turns: 0.,
            rotor_tilt: state.disk_tilt,
            flame: state.flags.flame,
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
        stores: Vec::new(),
    }
}

pub fn section(code: u8) -> Option<DamageSection> {
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
    replay::WeaponInfo {
        id,
        source: weapon.source.clone(),
        shape: weapon.shape.clone(),
        name: weapon.hud_name.clone(),
        class: weapon_class(weapon),
    }
}

/// What a weapon record is, for the viewer and the exports. The debrief's
/// store-flag rule (`ShotKind`) decides guns, missiles and bombs; the air to
/// ground round adds two refinements (docs/REPLAYS.md, "Weapons"):
///
/// - a guided glide bomb (the laser bombs and the JDAMs, which the debrief
///   counts with the missiles) is a bomb, so a replay draws and names it as
///   one;
/// - an unguided rocket (a pod of several rockets, no seeker) is a rocket,
///   where it used to be recorded as other.
pub fn weapon_class(weapon: &tore_formats::weapons::Weapon) -> replay::WeaponClass {
    if weapon.source.starts_with("JETTISON-") {
        return replay::WeaponClass::Other;
    }
    use tore_sim::combat::{guided, ledger::ShotKind};
    if live::is_gun(weapon) {
        return replay::WeaponClass::Gun;
    }
    if guided::is_glide_bomb(weapon) {
        return replay::WeaponClass::Bomb;
    }
    if is_rocket(weapon) {
        return replay::WeaponClass::Rocket;
    }
    match ShotKind::of(weapon) {
        ShotKind::AirToAir | ShotKind::AirToGround => replay::WeaponClass::Missile,
        ShotKind::Bomb => replay::WeaponClass::Bomb,
        ShotKind::Gun => replay::WeaponClass::Gun,
        // Guided but neither air nor surface, and other stores.
        ShotKind::Other if weapon.flags & 1 != 0 => replay::WeaponClass::Missile,
        ShotKind::Other => replay::WeaponClass::Other,
    }
}

/// An unguided rocket: a pod of several rounds with no seeker and no bomb
/// or gun flags (the LAU-61, B-8M1, B-13L and LAU-10 pods; the same test
/// the HUD's rocket pipper uses).
pub fn is_rocket(weapon: &tore_formats::weapons::Weapon) -> bool {
    use tore_sim::combat::ballistics;
    !live::is_gun(weapon)
        && !ballistics::is_bomb(weapon)
        && weapon.flags & 1 == 0
        && ballistics::is_pod(weapon)
        && weapon.seeker.signature == 0
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
        powered: false,
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

/// The piece of each of a surface owner's debris pieces, as recorded:
/// `(owner, index, piece)` with `index` counted as [`debris_states`] counts.
/// Aircraft debris takes its variant from the owner's structural section
/// instead, so it is left out. Parked aircraft throw pieces (`_B`, `_D`), and
/// `DebrisPose::variant` is the piece for them.
pub fn debris_pieces(debris: &[DebrisPose]) -> Vec<(u32, u32, u8)> {
    let mut counts: BTreeMap<u32, u32> = BTreeMap::new();
    debris
        .iter()
        .filter_map(|piece| {
            let index = counts.entry(piece.owner).or_default();
            let at = *index;
            *index += 1;
            let variant = piece.variant.filter(|_| is_surface_id(piece.owner))?;
            Some((piece.owner, at, u8::try_from(variant).unwrap_or(u8::MAX)))
        })
        .collect()
}

/// Whether `id` is in the surface objects' id range (`0x4000_0000` up to
/// the shots surface units fire).
pub fn is_surface_id(id: u32) -> bool {
    (tore_world::surface::LAYOUT_OBJECT_BASE..tore_world::surface::SURFACE_UNIT_END).contains(&id)
}

/// The routed surface units as a frame records them.
pub fn surface_states(poses: &[SurfacePose]) -> Vec<replay::SurfaceState> {
    poses
        .iter()
        .map(|pose| replay::SurfaceState {
            id: pose.id.0,
            position: pose.position,
            attitude: pose.attitude,
            wrecked: pose.wrecked,
        })
        .collect()
}

/// The recorded surface units as the picture draws them; the shape is left
/// to the drawing, which knows each unit's look.
pub fn surface_poses(states: &[replay::SurfaceState]) -> Vec<SurfacePose> {
    states
        .iter()
        .map(|state| SurfacePose {
            id: UnitId(state.id),
            position: state.position,
            attitude: state.attitude,
            shape: None,
            wrecked: state.wrecked,
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
        // A recording lists the second crew member of a two-seater as a
        // second escapee of the same owner; nothing draws them differently.
        crew: false,
    }
}

/// A live effect as a recording keeps it: a hit, kill or ground strike
/// with a reviewed explosion type keeps that type.
pub fn effect_kind(kind: live::EffectKind, blast: Option<u8>) -> replay::EffectKind {
    // A flak burst has its own code (format 3) with the explosion type it
    // showed, 27 or 28, so a replay lights and puffs it as flight does.
    if kind == live::EffectKind::Flak
        && let Some(explosion) = blast.filter(|b| tore_sim::combat::blast::explosion(*b).is_some())
    {
        return replay::EffectKind::Flak { explosion };
    }
    let on = match kind {
        live::EffectKind::Hit | live::EffectKind::Flak => Some(replay::Strike::Hit),
        live::EffectKind::Destroyed => Some(replay::Strike::Destroyed),
        live::EffectKind::Ground => Some(replay::Strike::Ground),
        _ => None,
    };
    if let (Some(on), Some(explosion)) = (
        on,
        blast.filter(|b| tore_sim::combat::blast::explosion(*b).is_some()),
    ) {
        return replay::EffectKind::Blast { on, explosion };
    }
    match kind {
        live::EffectKind::Flare => replay::EffectKind::Flare,
        live::EffectKind::Chaff => replay::EffectKind::Chaff,
        live::EffectKind::Launch => replay::EffectKind::Launch,
        live::EffectKind::Hit | live::EffectKind::Flak => replay::EffectKind::Hit,
        live::EffectKind::Destroyed => replay::EffectKind::Destroyed,
        live::EffectKind::Ground => replay::EffectKind::Ground,
        live::EffectKind::DebrisImpact => replay::EffectKind::DebrisImpact,
    }
}

/// A recorded effect's live kind and explosion type. Craters and fires are
/// marks, not effects.
pub fn live_effect_kind(kind: replay::EffectKind) -> Option<(live::EffectKind, Option<u8>)> {
    Some(match kind {
        replay::EffectKind::Flare => (live::EffectKind::Flare, None),
        replay::EffectKind::Chaff => (live::EffectKind::Chaff, None),
        replay::EffectKind::Launch => (live::EffectKind::Launch, None),
        replay::EffectKind::Hit => (live::EffectKind::Hit, None),
        replay::EffectKind::Destroyed => (live::EffectKind::Destroyed, None),
        replay::EffectKind::Ground => (live::EffectKind::Ground, None),
        replay::EffectKind::DebrisImpact => (live::EffectKind::DebrisImpact, None),
        replay::EffectKind::Blast { on, explosion } => (
            match on {
                replay::Strike::Hit => live::EffectKind::Hit,
                replay::Strike::Destroyed => live::EffectKind::Destroyed,
                replay::Strike::Ground => live::EffectKind::Ground,
            },
            Some(explosion),
        ),
        replay::EffectKind::Flak { explosion } => (live::EffectKind::Flak, Some(explosion)),
        replay::EffectKind::Crater(_) | replay::EffectKind::Fire | replay::EffectKind::Other(_) => {
            return None;
        }
    })
}

/// A crater or crash-site fire as a recording keeps it: a spawn lasting
/// the ticks it has left, or for good.
pub fn mark_spawn(mark: &blast::Mark) -> replay::EffectSpawn {
    replay::EffectSpawn {
        kind: match mark.kind {
            blast::MarkKind::Crater(size) => replay::EffectKind::Crater(size),
            blast::MarkKind::Fire => replay::EffectKind::Fire,
        },
        position: mark.position,
        duration_ticks: mark.ticks,
        scale: mark.scale,
    }
}

/// Every crater and fire a recording started, read once, since they can
/// last the whole mission.
pub fn recorded_marks(recording: &replay::Recording) -> Vec<(u64, replay::EffectSpawn)> {
    recording
        .spawns(0, u64::MAX)
        .map(|spawns| {
            spawns
                .effects
                .into_iter()
                .filter(|(_, e)| {
                    matches!(
                        e.kind,
                        replay::EffectKind::Crater(_) | replay::EffectKind::Fire
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The craters and fires alive at `tick`, keeping only the newest fires up
/// to the simulation's cap (craters have none since 2026-10-12).
pub fn marks_at(marks: &[(u64, replay::EffectSpawn)], tick: u64) -> Vec<MarkPose> {
    // Whether each is a fire of limited life: only those count against the
    // cap (a building's burns for good, nr-fx2), as in the simulation.
    let mut alive: Vec<(bool, MarkPose)> = marks
        .iter()
        .filter(|(start, e)| *start <= tick && tick - start < u64::from(e.duration_ticks))
        .map(|(start, e)| {
            let ticks = if e.duration_ticks == blast::FOREVER {
                blast::FOREVER
            } else {
                e.duration_ticks - (tick - start) as u32
            };
            let mark = blast::Mark {
                position: e.position,
                kind: match e.kind {
                    replay::EffectKind::Crater(size) => blast::MarkKind::Crater(size),
                    _ => blast::MarkKind::Fire,
                },
                ticks,
                born: *start,
                serial: 0,
                scale: e.scale,
            };
            (mark.is_limited_fire(), MarkPose::of(&mark, tick))
        })
        .collect();
    let fires = alive.iter().filter(|(limited, _)| *limited).count();
    let mut drop = fires.saturating_sub(blast::MAX_FIRES);
    alive.retain(|(limited, _)| {
        if drop > 0 && *limited {
            drop -= 1;
            false
        } else {
            true
        }
    });
    alive.into_iter().map(|(_, pose)| pose).collect()
}

/// One chaff cartridge or flare as a recording or the network keeps it: the
/// releasing aircraft, the kind, the aircraft's exact position, velocity and
/// attitude when it left, its number, which sets its look, and the combat
/// tick after whose step it left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceRelease {
    pub owner: u32,
    /// [`EffectKind::Chaff`] or [`EffectKind::Flare`].
    pub kind: live::EffectKind,
    pub release: tore_sim::combat::countermeasures::Release,
    pub number: u64,
    pub tick: u64,
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
        .with(field::NUMBER, device.number as i64)
        .with(field::AFTER_TICK, device.tick as i64);
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
/// when a value is missing. `tick` is the entry's own tick, which stands in
/// for a missing `after_tick`.
pub fn device_release(event: &replay::Event, tick: u64) -> Option<DeviceRelease> {
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
        tick: match event.get(field::AFTER_TICK) {
            Some(value) => u64::try_from(value.as_i64()?).ok()?,
            None => tick,
        },
    })
}

/// A playing effect as drawn, from its recorded start.
pub fn effect_pose(effect: &replay::LiveEffect) -> Option<EffectPose> {
    let left = u64::from(effect.duration_ticks).checked_sub(effect.age_ticks)?;
    let (kind, blast) = live_effect_kind(effect.kind)?;
    Some(EffectPose {
        kind,
        position: effect.position,
        ticks: u16::try_from(left).ok().filter(|left| *left > 0)?,
        blast,
        scale: effect.scale,
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
                && a.blast == b.blast
                && a.scale == b.scale
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
                kind: effect_kind(effect.kind, effect.blast),
                position: effect.position,
                duration_ticks: u32::from(effect.ticks),
                scale: effect.scale,
            })
            .collect()
    }
}

/// Aircraft types and weapons a recording registered, by id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Identities {
    pub aircraft: BTreeMap<u32, AircraftId>,
    pub weapons: BTreeMap<u32, replay::WeaponInfo>,
    /// The surface units the recording names (format 3).
    pub surface: BTreeMap<u32, replay::SurfaceInfo>,
}

impl Identities {
    pub fn of(recording: &replay::Recording) -> Self {
        Self {
            aircraft: recording
                .aircraft()
                .filter_map(|info| {
                    Some((
                        info.id,
                        identity(&info.pt).or_else(|| by_label(&info.name))?,
                    ))
                })
                .collect(),
            weapons: recording
                .weapons()
                .map(|info| (info.id, info.clone()))
                .collect(),
            surface: recording
                .surface_units()
                .map(|info| (info.id, info.clone()))
                .collect(),
        }
    }
}

/// Longest an effect plays: an aircraft explosion's 240 ticks.
pub const EFFECT_LOOKBACK_TICKS: u64 = 240;

/// The effects the game was drawing at `tick`, in its order. The simulation
/// keeps at most [`live::MAX_EFFECTS`] and drops the oldest when a new one
/// starts at that cap, even if the dropped one had time left, so this
/// replays the cap over the recorded starts rather than keeping the newest
/// of everything still within its time: an effect cut short in the game
/// stays gone after other effects run out.
///
/// Starting from an empty list twice the longest effect back gives exactly
/// the game's list. Effects missed by starting late sit at the front of the
/// game's list, so the game drops them before any later one, and every one
/// has run out a longest effect later; from then on both lists hold the
/// same effects.
pub fn playing_effects(
    recording: &replay::Recording,
    tick: u64,
) -> replay::Result<Vec<replay::LiveEffect>> {
    let spawns = recording.spawns(tick.saturating_sub(2 * EFFECT_LOOKBACK_TICKS), tick)?;
    Ok(cap_effects(&spawns.effects, tick))
}

/// [`playing_effects`] over a list of recorded starts in tick order. Craters
/// and fires share the effect stream but are marks, outside the list.
pub fn cap_effects(spawns: &[(u64, replay::EffectSpawn)], tick: u64) -> Vec<replay::LiveEffect> {
    let playing_at =
        |at: u64, (start, e): &(u64, replay::EffectSpawn)| at - start < u64::from(e.duration_ticks);
    let mut list: Vec<(u64, replay::EffectSpawn)> = Vec::new();
    for spawn in spawns
        .iter()
        .filter(|(start, e)| *start <= tick && live_effect_kind(e.kind).is_some())
    {
        list.retain(|playing| playing_at(spawn.0, playing));
        if list.len() == live::MAX_EFFECTS {
            list.remove(0);
        }
        list.push(spawn.clone());
    }
    list.into_iter()
        .filter(|playing| playing_at(tick, playing))
        .map(|(spawn_tick, e)| replay::LiveEffect {
            spawn_tick,
            age_ticks: tick - spawn_tick,
            kind: e.kind,
            position: e.position,
            duration_ticks: e.duration_ticks,
            scale: e.scale,
        })
        .collect()
}

/// The picture one recorded frame draws. `effects` are the effects still
/// playing at the frame's tick, from [`playing_effects`]; everything else
/// comes from the frame.
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
            .find(|state| state.id == presentation.player)
            .map(pose)
            .unwrap_or_else(|| AircraftPose {
                id: presentation.player,
                ..AircraftPose::default()
            }),
        targets: frame
            .aircraft
            .iter()
            .filter(|state| state.id != presentation.player)
            .map(pose)
            .collect(),
        projectiles: frame
            .projectiles
            .iter()
            .map(|p| projectile_pose(p, identities.weapons.get(&p.weapon)))
            .collect(),
        effects,
        marks: Vec::new(),
        debris: frame
            .debris
            .iter()
            .map(|piece| {
                // A surface owner's piece is recorded; an aircraft's variant
                // follows its structural section.
                let variant = frame
                    .debris_pieces
                    .iter()
                    .find(|(owner, index, _)| (*owner, *index) == (piece.owner, piece.index))
                    .map(|(_, _, piece)| usize::from(*piece))
                    .or_else(|| structural(piece.owner));
                debris_pose(
                    piece,
                    presentation.draw(piece.owner, kind(piece.owner)),
                    variant,
                )
            })
            .collect(),
        pilots: frame.escapees.iter().map(pilot_pose).collect(),
        models: presentation.models.clone(),
        surface: surface_poses(&frame.surface),
    }
}

/// Worst allowed difference for each quantity: half a step plus rounding.
pub mod tolerance {
    use tore_replay::precision::*;
    const SLACK: f64 = 1e-9;
    pub const POSITION: f64 = POSITION_FT / 2. + SLACK;
    pub const ANGLE: f64 = ANGLE_RAD / 2. + SLACK;
    pub const VELOCITY: f64 = VELOCITY_FPS / 2. + SLACK;
    pub const UNIT_DEVICE: f64 = UNIT / 2. + SLACK;
    pub const SIGNED_DEVICE: f64 = SIGNED / 2. + SLACK;
    pub const SPEED: f64 = SPEED_FPS / 2. + SLACK;
    pub const RATE: f64 = RATE_RAD_S / 2. + SLACK;
    pub const ROTOR: f64 = ROTOR_SPEED / 2. + SLACK;
    pub const TILT: f64 = DISK_TILT / 2. + SLACK;
    /// Direction components, from the two quantized direction angles.
    pub const DIRECTION: f64 = 2e-5;
    /// Speed in the simulation's 1/256 ft/s steps, after rounding back.
    pub const SPEED_F8: i32 = (SPEED_FPS * 256. / 2.) as i32 + 1;
}

/// True when any component differs by more than `limit`, or is not a number.
pub fn far(a: [f64; 3], b: [f64; 3], limit: f64) -> bool {
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
        || live.engine.flame != replayed.engine.flame
        || far(live.engine.rates, replayed.engine.rates, tolerance::RATE)
        || (live.engine.rotor - replayed.engine.rotor).abs() > tolerance::ROTOR
        || live
            .engine
            .rotor_tilt
            .iter()
            .flatten()
            .zip(replayed.engine.rotor_tilt.iter().flatten())
            .any(|(live, replayed)| (live - replayed).abs() > tolerance::TILT)
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
/// a recording keeps only their hit points, and they are never drawn. The
/// routed surface units' poses are compared.
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
    if live.player.stores != replayed.player.stores {
        return Some(format!("aircraft {}'s stores differ", live.player.id));
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
        if a.stores != b.stores {
            return Some(format!("aircraft {}'s stores differ", a.id));
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
            || a.blast != b.blast
            || a.ticks != b.ticks
            || far(a.position, b.position, tolerance::POSITION)
        {
            return Some(format!("effect differs: live {a:?}, recorded {b:?}"));
        }
    }
    if live.marks.len() != replayed.marks.len() {
        return Some(format!(
            "{} craters and fires live, {} recorded",
            live.marks.len(),
            replayed.marks.len()
        ));
    }
    for (a, b) in live.marks.iter().zip(&replayed.marks) {
        if a.kind != b.kind
            || far(a.position, b.position, tolerance::POSITION)
            || (a.strength - b.strength).abs() > 0.01
        {
            return Some(format!("mark differs: live {a:?}, recorded {b:?}"));
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
    if live.surface.len() != replayed.surface.len() {
        return Some(format!(
            "{} routed surface units live, {} recorded",
            live.surface.len(),
            replayed.surface.len()
        ));
    }
    for (a, b) in live.surface.iter().zip(&replayed.surface) {
        if (a.id, a.wrecked) != (b.id, b.wrecked)
            || far(a.position, b.position, tolerance::POSITION)
            || (0..3).any(|i| angle_far(a.attitude[i], b.attitude[i]))
        {
            return Some(format!("surface unit differs: live {a:?}, recorded {b:?}"));
        }
    }
    None
}
