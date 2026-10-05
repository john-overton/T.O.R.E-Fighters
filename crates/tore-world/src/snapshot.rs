//! One plain-data picture of everything combat draws for a simulation tick.
//!
//! Live flight builds a [`RenderSnapshot`] at the end of every tick and keeps
//! the last two. The frame's picture is their [`interpolate`]d mix at the
//! frame's tick fraction, which the app owns (`CombatView`). Snapshots never
//! feed back into sensors, physics or the AI. Nothing here draws: the vertex
//! building that turns a snapshot into a picture is `render_snapshot.rs`.
use std::collections::BTreeMap;
use tore_formats::aircraft::AircraftId;
use tore_sim::flight;
use tore_sim::{
    attitude::{Basis, Vector},
    combat::live::{DAMAGE_SECTIONS, DamageSection, EffectKind},
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
    /// Craters and crash-site fires.
    pub marks: Vec<MarkPose>,
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
    /// The original explosion type it shows, when it has one.
    pub blast: Option<u8>,
}

/// One crater or crash-site fire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkPose {
    pub kind: tore_sim::combat::blast::MarkKind,
    pub position: Vector,
    /// Ticks since it started, which sets a fire's frame.
    pub age: u64,
    /// A fire's strength, 1 until it fades in its last minute.
    pub strength: f32,
}
impl MarkPose {
    pub fn of(mark: &tore_sim::combat::blast::Mark, tick: u64) -> Self {
        Self {
            kind: mark.kind,
            position: mark.position,
            age: tick.saturating_sub(mark.born),
            strength: mark.strength(),
        }
    }
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
        marks: current.marks.clone(),
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

pub fn wreck_in(phase: wreck::Phase) -> wreck::Wreck {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::combat::live;

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
                faults: Default::default(),
                category: 0,
                side: tore_sim::combat::live::NO_SIDE,
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
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "snapshot_checkpoint.rs"]
mod checkpoint;
