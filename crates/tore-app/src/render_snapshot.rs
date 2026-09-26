//! One plain-data picture of everything combat draws for a simulation tick.
//!
//! Live flight builds a [`RenderSnapshot`] at the end of every tick, keeps the
//! last two and draws their [`interpolate`]d mix with [`aircraft_batches`] and
//! [`combat_geometry`]. A mission replay draws decoded snapshots through the
//! same helpers, so both show the same picture. Snapshots never feed back into
//! sensors, physics or the AI.
use crate::{
    AppResult,
    aircraft::Airframe,
    flight,
    sim_renderer::{CombatGeometry, Contact},
    terrain::{Camera, World},
};
use std::collections::BTreeMap;
use tore_formats::{Pic, aircraft::AircraftId, shape::Shape};
use tore_sim::{
    attitude::{Basis, Vector, cross, unit},
    combat::live::{self, DAMAGE_SECTIONS, DamageSection, EffectKind},
    ejection::Phase,
    wreck,
};

/// Animated devices in snapshot order: gear, flaps, brake, hook, bay,
/// exhaust, elevator, aileron, rudder, speed (feet per second) and throttle.
pub const DEVICES: usize = 11;

/// Everything combat draws after one simulation tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderSnapshot {
    /// Combat tick this snapshot follows.
    pub tick: u64,
    /// The player's own aircraft, id 0. Live flight draws the player from its
    /// full flight state; a replay rebuilds it with [`pose_state`].
    pub player: AircraftPose,
    /// Every combat target in target order: AI aircraft, straight-flight
    /// fixtures and ground objects.
    pub targets: Vec<AircraftPose>,
    pub projectiles: Vec<ProjectilePose>,
    pub effects: Vec<EffectPose>,
    pub debris: Vec<DebrisPose>,
    /// Ejected pilots: the player's first, then AI aircraft in roster order.
    pub pilots: Vec<PilotPose>,
    /// Loaded aircraft models in draw order, one batch each.
    pub models: Vec<AircraftId>,
}
impl RenderSnapshot {
    #[allow(dead_code)] // Used by mission replays to find a recorded aircraft.
    pub fn target(&self, id: u32) -> Option<&AircraftPose> {
        self.targets.iter().find(|pose| pose.id == id)
    }
}

/// Which airframe draws a pose, and in which pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Draw {
    /// Not drawn: no model is loaded for it.
    #[default]
    Hidden,
    /// Drawn in this identity's aircraft batch, over the model's start state.
    Model(AircraftId),
    /// Drawn in the combat pass with the player's airframe, over the player's
    /// presented state: straight-flight fixtures and the player's debris.
    Ownship,
}

/// One aircraft or ground object as drawn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AircraftPose {
    pub id: u32,
    /// Exact aircraft identity; `None` for ground objects.
    #[allow(dead_code)] // Read by mission recordings and exports.
    pub aircraft: Option<AircraftId>,
    pub draw: Draw,
    pub position: Vector,
    /// Yaw, pitch and bank as drawn. A straight-flight fixture is level and
    /// faces along its velocity.
    pub attitude: [f64; 3],
    /// Ground-relative velocity, feet per second.
    pub velocity: Vector,
    /// The animated devices in [`DEVICES`] order. `None` when nothing
    /// simulates them, so the drawing rules keep the neutral pose.
    pub devices: Option<[f64; DEVICES]>,
    /// Nozzle heat inputs as drawn. Aircraft drawn from a model start state
    /// run their engine dry, so their nozzles follow the throttle.
    pub engine: Engine,
    pub damage: Damage,
    /// Physical airborne presence; only airborne targets are drawn.
    pub airborne: bool,
    /// Wreck phase once destroyed. `Grounded` and `Exploded` hide an aircraft
    /// drawn over the player's state (and the player itself).
    pub wreck: Option<wreck::Phase>,
    /// The player's crash flag, or a target with no hit points left.
    pub crashed: bool,
}

/// What the nozzle material reads, and whether the flame lights the scene.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Engine {
    /// Engine running with internal fuel.
    pub lit: bool,
    /// Afterburner lit, as the flight model resolves it.
    pub afterburner: bool,
    /// Angular rates the X-31 paddles and plume follow.
    pub rates: [f64; 3],
    /// The afterburner flame lights the scene
    /// (docs/spec/engine-material.md#afterburner-glow): the player's while
    /// its pilot is aboard and it has hit points, and an AI aircraft's
    /// while it flies with hit points, whatever its nozzle draws.
    pub flame: bool,
}

/// One aircraft's lit afterburner as lights: one in each engine's flame,
/// behind its outlet in `offsets` (right, up and forward from the aircraft's
/// reference point, in feet), sharing the aircraft's strength
/// (docs/spec/engine-material.md#afterburner-glow).
pub fn afterburner_glow(
    position: Vector,
    [yaw, pitch, bank]: [f64; 3],
    offsets: &[Vector],
) -> Vec<crate::countermeasure_renderer::Afterburner> {
    use crate::countermeasure_renderer::{AFTERBURNER_BEHIND_FEET, AFTERBURNER_SHARE, Afterburner};
    let basis = Basis::new(yaw, pitch, bank);
    let share = AFTERBURNER_SHARE / offsets.len().max(1) as f64;
    offsets
        .iter()
        .map(|o| Afterburner {
            position: std::array::from_fn(|i| {
                position[i]
                    + basis.right[i] * o[0]
                    + basis.up[i] * o[1]
                    + basis.forward[i] * (o[2] - AFTERBURNER_BEHIND_FEET)
            }),
            share,
        })
        .collect()
}

/// Where an aircraft's engine outlets sit, for its flame lights: its own
/// model's, from `model_outlets` (parallel to `models`), when a model is
/// loaded for its type and that is not the player's type; otherwise the
/// player's airframe's.
pub fn engine_outlets<'a>(
    aircraft: Option<AircraftId>,
    player: AircraftId,
    models: &[Airframe],
    model_outlets: &'a [Vec<Vector>],
    player_outlets: &'a [Vector],
) -> &'a [Vector] {
    aircraft
        .filter(|id| *id != player)
        .and_then(|id| models.iter().position(|model| model.profile.id == id))
        .and_then(|index| model_outlets.get(index))
        .map_or(player_outlets, Vec::as_slice)
}

/// The lit afterburners of the aircraft in `snapshot` other than the player,
/// in target order, at their poses. `offsets` gives an aircraft's engine
/// outlets.
pub fn target_glows<'a>(
    snapshot: &RenderSnapshot,
    offsets: impl Fn(&AircraftPose) -> &'a [Vector],
) -> Vec<crate::countermeasure_renderer::Afterburner> {
    snapshot
        .targets
        .iter()
        .filter(|pose| pose.engine.flame)
        .flat_map(|pose| afterburner_glow(pose.position, pose.attitude, offsets(pose)))
        .collect()
}

/// Damage as whole numbers. The drawn fractions derive from them exactly as
/// live flight derives them, so thresholds compare the same way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Damage {
    pub hp: i32,
    pub initial_hp: i32,
    /// Damage recorded against each section: nose, cockpit, core, left wing,
    /// right wing and tail.
    pub sections: [i32; DAMAGE_SECTIONS],
    /// The section whose break selects the damaged model, once one broke.
    pub structural: Option<DamageSection>,
}
impl Damage {
    /// Lost fraction of the hit points; 1 selects the destroyed body.
    pub fn fraction(&self) -> f64 {
        (1. - f64::from(self.hp.max(0)) / f64::from(self.initial_hp.max(1))).clamp(0., 1.)
    }
    /// Per-section damage fractions for surface marks.
    pub fn regions(&self) -> [f64; DAMAGE_SECTIONS] {
        self.sections
            .map(|amount| (f64::from(amount) / f64::from(self.initial_hp.max(1))).clamp(0., 1.))
    }
    /// Damage variant index, as the flight state stores it.
    pub fn variant(&self) -> Option<usize> {
        self.structural.map(|section| section as usize)
    }
}

/// One projectile in flight.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectilePose {
    pub id: u32,
    /// 0 is the player, otherwise the firing AI aircraft.
    #[allow(dead_code)] // Read by mission recordings and exports.
    pub owner: u32,
    /// Source weapon record, for example `AIM120.JT`.
    #[allow(dead_code)] // Read by mission recordings and exports.
    pub weapon: String,
    /// Shape drawn for a missile or store, when its record names one.
    pub shape: Option<String>,
    pub gun: bool,
    /// A gun round drawn with a tracer ribbon.
    pub tracer: bool,
    pub position: Vector,
    /// Position one tick earlier; tracers and missile strips span it.
    pub previous: Vector,
    pub direction: Vector,
    #[allow(dead_code)] // Read by mission recordings and exports.
    pub target: Option<u32>,
    /// Launched at the player.
    #[allow(dead_code)] // Read by mission recordings and exports.
    pub incoming: bool,
    /// Speed in 1/256 feet per second, as simulated.
    #[allow(dead_code)] // Read by mission recordings and exports.
    pub speed_f8: i32,
}

/// One flare, chaff cloud, launch flash, hit or explosion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectPose {
    pub kind: EffectKind,
    pub position: Vector,
    /// Ticks the effect has left.
    pub ticks: u16,
}

/// One detached piece of a destroyed aircraft.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DebrisPose {
    /// The destroyed aircraft; 0 is the player.
    #[allow(dead_code)] // Read by mission recordings and exports.
    pub owner: u32,
    pub draw: Draw,
    pub position: Vector,
    pub attitude: [f64; 3],
    /// Damage variant drawn, from the owner's structural section.
    pub variant: Option<usize>,
}

/// One ejected pilot, seat or parachute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PilotPose {
    /// The aircraft the pilot left; 0 is the player.
    #[allow(dead_code)] // Read by mission recordings and exports.
    pub owner: u32,
    pub position: Vector,
    pub heading: f64,
    pub phase: Phase,
}

pub fn devices(s: &flight::State) -> [f64; DEVICES] {
    [
        s.gear, s.flaps, s.brake, s.hook, s.bay, s.exhaust, s.elevator, s.aileron, s.rudder,
        s.speed, s.throttle,
    ]
}
pub fn set_devices(s: &mut flight::State, devices: [f64; DEVICES]) {
    [
        s.gear, s.flaps, s.brake, s.hook, s.bay, s.exhaust, s.elevator, s.aileron, s.rudder,
        s.speed, s.throttle,
    ] = devices;
}

/// The picture between two consecutive snapshots at tick fraction `alpha`.
///
/// Targets blend position and attitude from the previous tick and lerp their
/// devices; everything else is the current tick's. A target new this tick is
/// drawn where it is. The player follows its own presented-state rules. With
/// no previous snapshot (the first tick after a restart) the player blends
/// with itself, as live flight does.
pub fn interpolate(
    previous: Option<&RenderSnapshot>,
    current: &RenderSnapshot,
    alpha: f64,
) -> RenderSnapshot {
    let alpha = alpha.clamp(0., 1.);
    let before: BTreeMap<u32, &AircraftPose> = previous
        .map(|snapshot| {
            snapshot
                .targets
                .iter()
                .map(|pose| (pose.id, pose))
                .collect()
        })
        .unwrap_or_default();
    RenderSnapshot {
        tick: current.tick,
        player: presented_player(
            previous.map_or(&current.player, |snapshot| &snapshot.player),
            &current.player,
            alpha,
        ),
        targets: current
            .targets
            .iter()
            .map(|pose| blend(before.get(&pose.id).copied(), pose, alpha))
            .collect(),
        projectiles: current.projectiles.clone(),
        effects: current.effects.clone(),
        debris: current.debris.clone(),
        pilots: current.pilots.clone(),
        models: current.models.clone(),
    }
}

/// One target at tick fraction `alpha` (already clamped).
pub fn blend(previous: Option<&AircraftPose>, current: &AircraftPose, alpha: f64) -> AircraftPose {
    let mut pose = current.clone();
    if let Some(previous) = previous {
        pose.position = std::array::from_fn(|i| {
            previous.position[i] + (current.position[i] - previous.position[i]) * alpha
        });
        let [yaw, pitch, bank] = previous.attitude;
        let [next_yaw, next_pitch, next_bank] = current.attitude;
        pose.attitude = Basis::new(yaw, pitch, bank)
            .blended(Basis::new(next_yaw, next_pitch, next_bank), alpha)
            .angles();
    }
    if let Some(after) = current.devices {
        let before = previous.and_then(|pose| pose.devices).unwrap_or(after);
        pose.devices = Some(std::array::from_fn(|i| {
            before[i] + (after[i] - before[i]) * alpha
        }));
    }
    pose
}

/// The player between ticks, as [`flight::State::presented`] draws it: a
/// crashed aircraft that is not falling holds, otherwise position, attitude,
/// velocity and every device but the throttle follow the tick fraction.
fn presented_player(previous: &AircraftPose, current: &AircraftPose, alpha: f64) -> AircraftPose {
    if current.crashed && current.wreck != Some(wreck::Phase::Falling) {
        return current.clone();
    }
    let lerp = |a: f64, b: f64| a + (b - a) * alpha;
    let mut pose = current.clone();
    pose.position = std::array::from_fn(|i| lerp(previous.position[i], current.position[i]));
    let [yaw, pitch, bank] = previous.attitude;
    let [next_yaw, next_pitch, next_bank] = current.attitude;
    pose.attitude = Basis::new(yaw, pitch, bank)
        .blended(Basis::new(next_yaw, next_pitch, next_bank), alpha)
        .angles();
    pose.velocity = std::array::from_fn(|i| lerp(previous.velocity[i], current.velocity[i]));
    if let (Some(before), Some(mut after)) = (previous.devices, current.devices) {
        for (value, before) in after.iter_mut().zip(before).take(DEVICES - 1) {
            *value = lerp(before, *value);
        }
        pose.devices = Some(after);
    }
    pose
}

fn wreck_in(phase: wreck::Phase) -> wreck::Wreck {
    let mut wreck = wreck::Wreck::new(0, 0, [0.; 3]);
    wreck.phase = phase;
    wreck
}

/// A drawable flight state for `pose`. `template` supplies the airframe's
/// flight model and every field the picture does not read, and should be a
/// start state (engine running, full fuel, undamaged systems). Without
/// devices the neutral model pose applies: gear, flaps, exhaust and bay in.
#[allow(dead_code)] // Used by mission replays to draw recorded aircraft.
pub fn pose_state(template: &flight::State, pose: &AircraftPose) -> flight::State {
    let mut s = template.clone();
    s.position = pose.position;
    [s.yaw, s.pitch, s.bank] = pose.attitude;
    s.velocity = pose.velocity;
    match pose.devices {
        Some(devices) => set_devices(&mut s, devices),
        None => [s.gear, s.flaps, s.exhaust, s.bay] = [0.; 4],
    }
    s.engine = pose.engine.lit;
    s.burner = pose.engine.afterburner;
    s.auxiliary_rates = pose.engine.rates;
    s.crashed = pose.crashed;
    s.wreck = pose.wreck.map(wreck_in);
    s.damage_fraction = pose.damage.fraction();
    s.damage_variant = pose.damage.variant();
    s.damage_regions = pose.damage.regions();
    s
}

/// A target drawn with its own model, over the model's start state.
fn model_pose(mut s: flight::State, pose: &AircraftPose) -> flight::State {
    s.position = pose.position;
    s.damage_fraction = pose.damage.fraction();
    s.damage_variant = pose.damage.variant();
    s.damage_regions = pose.damage.regions();
    [s.yaw, s.pitch, s.bank] = pose.attitude;
    s.gear = 0.;
    s.flaps = 0.;
    s.exhaust = 0.;
    s.bay = 0.;
    if let Some(devices) = pose.devices {
        set_devices(&mut s, devices);
    }
    s
}

/// A straight-flight fixture drawn with the player's airframe over the
/// player's presented state, which supplies bay, speed and throttle. The
/// pose's resolved engine decides the nozzle heat, so a replay's rebuilt
/// player state draws fixtures exactly as the live one does.
fn ownship_pose(template: &flight::State, pose: &AircraftPose) -> flight::State {
    let mut s = template.clone();
    s.wreck = pose.wreck.map(wreck_in);
    s.crashed = pose.damage.hp <= 0;
    s.engine = pose.engine.lit;
    s.burner = pose.engine.afterburner;
    s.position = pose.position;
    s.damage_fraction = pose.damage.fraction();
    s.damage_variant = pose.damage.variant();
    s.damage_regions = pose.damage.regions();
    [s.yaw, s.pitch, s.bank] = pose.attitude;
    s.exhaust = 0.;
    s.gear = 0.;
    s.flaps = 0.;
    s.elevator = 0.;
    s.aileron = 0.;
    s.rudder = 0.;
    s.brake = 0.;
    s.hook = 0.;
    if let Some(devices) = pose.devices {
        set_devices(&mut s, devices);
    }
    s
}

/// Per-model vertices for the aircraft drawn with their own models: one batch
/// per `snapshot.models` entry found in `models`, with each airborne
/// aircraft's vertex range for the spotting aid, then that model's debris.
pub fn aircraft_batches<'a>(
    snapshot: &RenderSnapshot,
    models: &'a [Airframe],
    camera: &Camera,
    world: &World,
) -> Vec<(&'a Airframe, Vec<f32>, Vec<Contact>)> {
    snapshot
        .models
        .iter()
        .filter_map(|id| models.iter().find(|model| model.profile.id == *id))
        .map(|model| {
            let draw = Draw::Model(model.profile.id);
            let mut vertices = Vec::new();
            let mut contacts = Vec::new();
            let extent = model.visual_extent();
            for target in snapshot
                .targets
                .iter()
                .filter(|t| t.draw == draw && t.airborne && Some(t.id) != camera.hidden_target)
            {
                let pose = model_pose(model.start(world), target);
                let first = vertices.len() / 10;
                vertices.extend(model.vertices(&pose, camera, world));
                contacts.extend(Contact::new(
                    first,
                    vertices.len() / 10,
                    pose.position,
                    extent,
                ));
            }
            for piece in snapshot.debris.iter().filter(|p| p.draw == draw) {
                let mut pose = model.start(world);
                pose.position = piece.position;
                pose.damage_variant = piece.variant;
                [pose.yaw, pose.pitch, pose.bank] = piece.attitude;
                vertices.extend(model.fragment_vertices(&pose, camera, world));
            }
            (model, vertices, contacts)
        })
        .collect()
}

/// Combat geometry drawn with the player's airframe: fixture targets over the
/// player's presented state, debris, weapons, tracers and effects.
pub fn combat_geometry(
    snapshot: &RenderSnapshot,
    art: &CombatArt,
    ownship: &Airframe,
    ownship_state: &flight::State,
    camera: &Camera,
    world: &World,
) -> CombatGeometry {
    let mut v = Vec::new();
    let mut contacts = Vec::new();
    let extent = ownship.visual_extent();
    for target in snapshot
        .targets
        .iter()
        .filter(|t| t.draw == Draw::Ownship && t.airborne && Some(t.id) != camera.hidden_target)
    {
        let pose = ownship_pose(ownship_state, target);
        let first = v.len() / 10;
        v.extend(ownship.vertices(&pose, camera, world));
        contacts.extend(Contact::new(first, v.len() / 10, pose.position, extent));
    }
    for piece in snapshot.debris.iter().filter(|p| p.draw == Draw::Ownship) {
        let mut pose = ownship_state.clone();
        // Detached pieces have their own lifecycle, not the observer's wreck state.
        pose.wreck = None;
        pose.position = piece.position;
        pose.damage_variant = piece.variant;
        [pose.yaw, pose.pitch, pose.bank] = piece.attitude;
        v.extend(ownship.fragment_vertices(&pose, camera, world));
    }
    // Attached external stores are hidden until the dedicated ordnance
    // rendering pass. Loadout/flight state and launched projectiles remain
    // independent of this presentation decision in every camera.
    for p in &snapshot.projectiles {
        if Some(p.id) == camera.hidden_projectile {
            continue;
        }
        if !p.gun
            && let Some(shape) = p.shape.as_ref().and_then(|name| art.shapes.get(name))
        {
            let right = unit([p.direction[2], 0., -p.direction[0]]);
            mesh(
                &mut v,
                shape,
                p.position,
                right,
                cross(p.direction, right),
                p.direction,
                &ownship.palette,
            );
        }
        if p.gun && p.tracer {
            tracer(&mut v, p.previous, p.position, camera);
        } else if !p.gun {
            // A visible thin strip marks the actual swept projectile segment.
            let right = Basis::new(f64::from(camera.yaw), f64::from(camera.pitch), 0.).right;
            let a: Vector = std::array::from_fn(|i| p.previous[i] + right[i] * 0.4);
            let b: Vector = std::array::from_fn(|i| p.previous[i] - right[i] * 0.4);
            for pos in [a, b, p.position] {
                vertex(&mut v, pos, [1., 0.8, 0.3]);
            }
        }
    }
    let basis = Basis::new(
        f64::from(camera.yaw),
        f64::from(camera.pitch),
        -f64::from(camera.roll),
    );
    for e in &snapshot.effects {
        // Chaff and flares are drawn by countermeasure_renderer.
        if matches!(
            e.kind,
            EffectKind::Launch | EffectKind::Flare | EffectKind::Chaff
        ) {
            continue;
        }
        let scale = if e.kind == EffectKind::Destroyed {
            75.
        } else {
            15.
        };
        let duration = if e.kind == EffectKind::Destroyed {
            240
        } else {
            45
        };
        let frame = (usize::from(duration - e.ticks) * 12 / usize::from(duration)).min(11);
        let frames = if e.kind == EffectKind::DebrisImpact {
            &art.ground_impacts
        } else {
            &art.explosions
        };
        for (xy, color) in &frames[frame] {
            for d in [
                [0., 0.],
                [1. / 20., 0.],
                [0., 1. / 20.],
                [0., 1. / 20.],
                [1. / 20., 0.],
                [1. / 20., 1. / 20.],
            ] {
                let pos: Vector = std::array::from_fn(|i| {
                    e.position[i]
                        + basis.right[i] * f64::from(xy[0] + d[0]) * scale
                        + basis.up[i] * f64::from(xy[1] + d[1]) * scale
                });
                vertex(&mut v, pos, *color);
            }
        }
    }
    CombatGeometry {
        vertices: v,
        contacts,
    }
}

/// Combat effect and weapon art: explosion and ground-impact frames, the smoke
/// sheet, weapon shapes by name and ejected-pilot poses.
pub struct CombatArt {
    pub smoke: Pic,
    shapes: BTreeMap<String, Shape>,
    explosions: Vec<Vec<([f32; 2], [f32; 3])>>,
    ground_impacts: Vec<Vec<([f32; 2], [f32; 3])>>,
    pub escape: Option<crate::ejection_art::Art>,
}
impl CombatArt {
    /// Loads the sampled original effect art. `palette` supplies the colours
    /// the effect sheets do not carry themselves.
    pub fn load(data: &BTreeMap<String, Vec<u8>>, palette: &[[u8; 3]; 256]) -> AppResult<Self> {
        let pic = Pic::parse(
            data.get("AIRLRG.PIC")
                .ok_or("missing AIRLRG.PIC combat art")?,
        )?;
        if pic.width != 256 || pic.height != 232 {
            return Err("unreviewed AIRLRG frame sheet".into());
        }
        // Fitted 3x4 frame layout over visually reviewed original effect art.
        let explosions = effect_frames(&pic, palette, 58);
        let impact = Pic::parse(data.get("GRDLRGA.PIC").ok_or("missing GRDLRGA.PIC")?)?;
        if impact.width != 256 || impact.height != 252 {
            return Err("unreviewed ground impact sheet".into());
        }
        let ground_impacts = effect_frames(&impact, palette, 63);
        let smoke = Pic::parse(data.get("SMOKE.PIC").ok_or("missing SMOKE.PIC")?)?;
        if smoke.width != 256 || smoke.height != 43 {
            return Err("unreviewed smoke sheet dimensions".into());
        }
        Ok(Self {
            smoke,
            shapes: BTreeMap::new(),
            explosions,
            ground_impacts,
            escape: match crate::ejection_art::Art::load(data) {
                Ok(art) => Some(art),
                Err(error) => {
                    log::warn!("Optional ejection artwork unavailable: {error}");
                    None
                }
            },
        })
    }
    /// Loads every weapon shape a configuration's stations name.
    pub fn add_weapon_shapes(
        &mut self,
        config: &live::Configuration,
        data: &BTreeMap<String, Vec<u8>>,
    ) {
        self.add_shapes(
            config
                .stations
                .iter()
                .filter_map(|station| station.weapon.shape.as_deref()),
            data,
        );
    }
    /// Loads named weapon shapes; a missing or empty one draws as a strip.
    pub fn add_shapes<'a>(
        &mut self,
        names: impl IntoIterator<Item = &'a str>,
        data: &BTreeMap<String, Vec<u8>>,
    ) {
        for name in names {
            let shape = data
                .get(name)
                .and_then(|bytes| Shape::parse(bytes).ok())
                .filter(|shape| !shape.faces.is_empty());
            match shape {
                Some(shape) => {
                    self.shapes.insert(name.to_owned(), shape);
                }
                None => log::warn!(
                    "Combat: {name} uses a tracer marker; line/point drawing remains open"
                ),
            }
        }
    }
    /// Synthetic art for drawing tests without retail media.
    #[cfg(test)]
    pub(crate) fn synthetic(
        shapes: BTreeMap<String, Shape>,
        explosions: Vec<Vec<([f32; 2], [f32; 3])>>,
        ground_impacts: Vec<Vec<([f32; 2], [f32; 3])>>,
    ) -> Self {
        Self {
            smoke: Pic {
                width: 1,
                height: 1,
                pixels: vec![0],
                mask: vec![true],
                palette: Vec::new(),
                glyphs: Vec::new(),
            },
            shapes,
            explosions,
            ground_impacts,
            escape: None,
        }
    }
}

fn effect_frames(
    pic: &Pic,
    base: &[[u8; 3]; 256],
    cell_height: usize,
) -> Vec<Vec<([f32; 2], [f32; 3])>> {
    let mut palette = *base;
    palette[..pic.palette.len()].copy_from_slice(&pic.palette);
    (0..12)
        .map(|frame| {
            let mut cells = Vec::new();
            for y in 0..20 {
                for x in 0..20 {
                    let index = pic.pixels[(frame / 3 * cell_height + y * cell_height / 20)
                        * pic.width
                        + frame % 3 * 80
                        + x * 80 / 20];
                    if index != 255 {
                        cells.push((
                            [x as f32 / 20. - 0.5, 0.5 - y as f32 / 20.],
                            palette[index as usize].map(|v| f32::from(v) / 255.),
                        ));
                    }
                }
            }
            cells
        })
        .collect()
}
fn mesh(
    out: &mut Vec<f32>,
    shape: &Shape,
    position: Vector,
    right: Vector,
    up: Vector,
    forward: Vector,
    palette: &[[u8; 3]; 256],
) {
    for face in &shape.faces {
        // These are texture-only SH faces (including the missile exhaust
        // sheets). Palette index zero is not an opaque substitute for them.
        // Keep them omitted until the weapon texture/animation path is decoded.
        if matches!(face.subtype, 0x4c | 0x5c | 0x6c | 0x7c) {
            continue;
        }
        for i in 1..face.positions.len() - 1 {
            for j in [0, i, i + 1] {
                let q = face.positions[j];
                let pos = std::array::from_fn(|k| {
                    position[k]
                        + (right[k] * f64::from(q[0])
                            + up[k] * f64::from(q[2])
                            + forward[k] * f64::from(q[1]))
                            / 3.
                });
                vertex(
                    out,
                    pos,
                    palette[face.colors[j] as usize].map(|c| f32::from(c) / 255.),
                );
                let layer = out.len() - 5;
                out[layer] = -1.;
            }
        }
    }
}
/// Camera-facing luminous ribbon over the actual swept gun segment.
fn tracer(out: &mut Vec<f32>, previous: Vector, position: Vector, camera: &Camera) {
    let segment: Vector = std::array::from_fn(|i| position[i] - previous[i]);
    if tore_sim::attitude::dot(segment, segment) < 1e-12 {
        return;
    }
    let view: Vector = std::array::from_fn(|i| f64::from(camera.position[i]) - position[i]);
    let cross = cross(segment, view);
    let (start, ribbon, side) = if tore_sim::attitude::dot(cross, cross)
        > 0.25 * tore_sim::attitude::dot(view, view).max(1e-12)
    {
        (previous, segment, unit(cross))
    } else {
        // Viewed along its path, retain a small glow instead of collapsing
        // the ribbon into a line with zero screen area.
        let basis = Basis::new(f64::from(camera.yaw), f64::from(camera.pitch), 0.);
        (
            std::array::from_fn(|i| position[i] - basis.up[i] * 0.25),
            basis.up.map(|v| v * 0.5),
            basis.right,
        )
    };
    for [along, across] in [
        [0., -1.],
        [1., -1.],
        [1., 1.],
        [0., -1.],
        [1., 1.],
        [0., 1.],
    ] {
        let pos: Vector =
            std::array::from_fn(|i| start[i] + ribbon[i] * along + side[i] * across * 1.2);
        out.extend([
            pos[0] as f32,
            pos[1] as f32,
            pos[2] as f32,
            along as f32,
            across as f32,
            -8.,
            1.,
            1.,
            1.,
            -1.,
        ]);
    }
}

fn vertex(out: &mut Vec<f32>, pos: Vector, color: [f32; 3]) {
    // Trailing -1 opts out of the weather palette: this color is already resolved.
    out.extend([
        pos[0] as f32,
        pos[1] as f32,
        pos[2] as f32,
        0.,
        0.,
        -6., // Emissive effect; mesh() opts solid weapon bodies into lighting.
        color[0],
        color[1],
        color[2],
        -1.,
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(id: u32, position: Vector, attitude: [f64; 3]) -> AircraftPose {
        AircraftPose {
            id,
            position,
            attitude,
            ..Default::default()
        }
    }

    #[test]
    fn devices_follow_the_tick_fraction() {
        let mut previous = pose(1, [0.; 3], [0.; 3]);
        previous.devices = Some([1., 1., 0., 0., 0., 0., 0., 0., 0., 0., 0.]);
        let mut current = previous.clone();
        current.devices = Some([0., 0., 1., 1., 1., 1., 0.4, -0.4, 0.2, 400., 1.]);
        let at = |alpha| blend(Some(&previous), &current, alpha).devices.unwrap();
        assert_eq!(at(0.)[..2], [1., 1.]);
        let quarter = at(0.25);
        assert_eq!(
            [quarter[0], quarter[1], quarter[2], quarter[9]],
            [0.75, 0.75, 0.25, 100.]
        );
        assert_eq!(quarter[6..9], [0.1, -0.1, 0.05]);
        assert_eq!(at(1.)[..2], [0., 0.]);
        // First seen this tick: drawn as simulated.
        assert_eq!(blend(None, &current, 0.5).devices, current.devices);
        // Nothing simulates them: the drawing rules keep the neutral pose.
        current.devices = None;
        assert_eq!(blend(Some(&previous), &current, 0.5).devices, None);
    }

    #[test]
    fn targets_blend_from_the_previous_tick_and_new_ones_stay_put() {
        let previous = RenderSnapshot {
            targets: vec![pose(1, [0., 0., 0.], [0.1, 0., 0.])],
            ..Default::default()
        };
        let mut current = RenderSnapshot {
            targets: vec![
                pose(1, [10., 20., 30.], [0.3, 0.1, 0.2]),
                pose(2, [5., 5., 5.], [1., 0., 0.]),
            ],
            ..Default::default()
        };
        current.targets[0].velocity = [7., 8., 9.];
        current.targets[0].damage.hp = 3;
        let mid = interpolate(Some(&previous), &current, 0.5);
        assert_eq!(mid.targets[0].position, [5., 10., 15.]);
        assert!((mid.targets[0].attitude[0] - 0.2).abs() < 0.01);
        // Everything but the pose is the current tick's.
        assert_eq!(mid.targets[0].velocity, [7., 8., 9.]);
        assert_eq!(mid.targets[0].damage.hp, 3);
        assert_eq!(mid.targets[1], current.targets[1]);
        // Tick fractions outside one tick clamp to it.
        assert_eq!(
            interpolate(Some(&previous), &current, 3.).targets[0].position,
            [10., 20., 30.]
        );
        // Right after a restart nothing blends.
        assert_eq!(interpolate(None, &current, 0.5).targets, current.targets);
    }

    #[test]
    fn a_grounded_player_wreck_holds_and_the_throttle_never_blends() {
        let mut previous = pose(0, [0.; 3], [0.; 3]);
        previous.devices = Some([0.; DEVICES]);
        let mut current = pose(0, [100., 0., 0.], [0.; 3]);
        current.devices = Some([1.; DEVICES]);
        let snapshot = |player: &AircraftPose| RenderSnapshot {
            player: player.clone(),
            ..Default::default()
        };
        let mid = interpolate(Some(&snapshot(&previous)), &snapshot(&current), 0.5).player;
        assert_eq!(mid.position, [50., 0., 0.]);
        let devices = mid.devices.unwrap();
        assert!(devices[..DEVICES - 1].iter().all(|v| *v == 0.5));
        assert_eq!(devices[DEVICES - 1], 1.);
        current.crashed = true;
        current.wreck = Some(wreck::Phase::Grounded);
        let held = interpolate(Some(&snapshot(&previous)), &snapshot(&current), 0.5).player;
        assert_eq!(held, current);
        current.wreck = Some(wreck::Phase::Falling);
        let falling = interpolate(Some(&snapshot(&previous)), &snapshot(&current), 0.5).player;
        assert_eq!(falling.position, [50., 0., 0.]);
    }

    #[test]
    fn damage_fractions_match_the_live_whole_number_rules() {
        let target = |hp, initial_hp, amounts| {
            let mut t = live::Target {
                aircraft: None,
                role: tore_sim::combat::missiles::TargetRole::Aircraft,
                heat: tore_sim::combat::missiles::seeker::Heat::Unknown,
                radar_emitting: false,
                id: 1,
                position: [0.; 3],
                velocity: [0.; 3],
                basis: Basis::new(0., 0., 0.),
                configuration: tore_sim::sensors::Configuration::CLEAN,
                signature: Default::default(),
                jammer: None,
                jammer_active: false,
                airborne: true,
                on_ground: false,
                radius: 28.,
                hp,
                initial_hp,
                fragment_offsets: [[0.; 3]; 2],
                wreck: None,
                wreck_power: Default::default(),
                fragment_released: false,
                localized_damage: Default::default(),
                category: 0,
            };
            t.localized_damage.amounts = amounts;
            t.localized_damage.structural_section = Some(DamageSection::Tail);
            t
        };
        for (hp, initial_hp, amounts) in [
            (100, 100, [0; DAMAGE_SECTIONS]),
            (37, 113, [1, 2, 3, 40, 50, 60]),
            (0, 90, [90, 0, 0, 0, 0, 200]),
            (-5, 0, [3, 0, 0, 0, 0, 0]),
        ] {
            let t = target(hp, initial_hp, amounts);
            let damage = Damage {
                hp,
                initial_hp,
                sections: amounts,
                structural: t.localized_damage.structural_section,
            };
            assert_eq!(damage.fraction().to_bits(), t.damage_fraction().to_bits());
            assert_eq!(
                damage.regions().map(f64::to_bits),
                t.localized_damage.fractions(initial_hp).map(f64::to_bits)
            );
            assert_eq!(damage.variant(), Some(DamageSection::Tail as usize));
        }
    }

    #[test]
    fn tracer_ribbon_is_finite_camera_facing_and_visible_end_on() {
        let mut camera = Camera::new();
        camera.position = [0., 0., -100.];
        camera.yaw = 0.;
        camera.pitch = 0.;
        for end in [[20., 0., 0.], [0., 0., 20.]] {
            let mut output = Vec::new();
            tracer(&mut output, [0.; 3], end, &camera);
            assert_eq!(output.len(), 60);
            assert!(output.iter().all(|v| v.is_finite()));
            let points: Vec<[f32; 2]> = output.chunks_exact(10).map(|v| [v[0], v[1]]).collect();
            let a = [points[1][0] - points[0][0], points[1][1] - points[0][1]];
            let b = [points[2][0] - points[0][0], points[2][1] - points[0][1]];
            assert!((a[0] * b[1] - a[1] * b[0]).abs() > 0.1);
            assert!(output.chunks_exact(10).all(|v| v[5] == -8.));
        }
        let mut output = Vec::new();
        tracer(&mut output, [0.; 3], [0.; 3], &camera);
        assert!(output.is_empty());
    }

    #[test]
    fn palette_mesh_does_not_turn_texture_only_exhaust_into_solid_faces() {
        let face = tore_formats::shape::Face {
            fog: tore_formats::shape::FogMode::Enabled,
            positions: vec![[0., 0., 0.], [3., 0., 0.], [0., 3., 0.]],
            colors: vec![1; 3],
            uv: vec![],
            texture: "SYNTHETIC.PIC".into(),
            subtype: 0x61,
            normal: None,
            address: 0,
        };
        let mut exhaust = face.clone();
        exhaust.subtype = 0x4c;
        let shape = Shape {
            lines: vec![],
            faces: vec![face, exhaust],
            state_words: Default::default(),
        };
        let mut out = vec![];
        mesh(
            &mut out,
            &shape,
            [0.; 3],
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            &[[255; 3]; 256],
        );
        // One triangle of ten-float vertices; the exhaust face is omitted.
        assert_eq!(out.len(), 30);
    }
}
