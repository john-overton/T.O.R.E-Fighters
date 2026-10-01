//! What the screen flies again instead of being told: chaff and flares, and
//! (for a networked flight, which has no combat) smoke and contrails. The
//! replay viewer and a networked client share this one place so a chaff
//! cloud, a flare and its smoke look the same wherever they are drawn.
//!
//! Nothing here changes the simulation. The rules are the simulation's own:
//! devices fly with [`Devices::step`] over the mission's ground and wind, and
//! smoke steps with [`Smoke::step`], so a puff is born, rises, drifts and
//! fades exactly as it does where combat runs. See docs/ARCHITECTURE.md,
//! "The client session" and docs/REPLAYS.md.
use crate::terrain::Terrain;
use tore_sim::combat::{countermeasures::Devices, live::EffectKind};

/// One chaff cartridge or flare as a recording or the network keeps it: the
/// releasing aircraft, the kind, the aircraft's exact position, velocity and
/// attitude when it left, its number, which sets its look, and the combat
/// tick after whose step it left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceRelease {
    pub owner: u32,
    /// [`EffectKind::Chaff`] or [`EffectKind::Flare`].
    pub kind: EffectKind,
    pub release: tore_sim::combat::countermeasures::Release,
    pub number: u64,
    pub tick: u64,
}

/// Lets `release` go: numbering continues after the releases before it, so
/// the device has the look it had where it was released.
pub fn release_device(devices: &mut Devices, release: &DeviceRelease) {
    devices.continue_after(release.number.saturating_sub(1));
    if release.kind == EffectKind::Chaff {
        devices.release_chaff(release.release);
    } else {
        devices.release_flare(release.release);
    }
}

/// Flies every device one 120 Hz tick over `world`'s ground, carried by its
/// wind (flare smoke drifts with the mission wind, as in flight).
pub fn fly_devices(devices: &mut Devices, world: &Terrain) {
    let ground = |x: f64, z: f64| f64::from(world.height(x as f32, z as f32));
    devices.wind = world.wind();
    devices.step(&ground);
}

// ---------------------------------------------------------------------------
// A networked flight's smoke, contrails, chaff and flares
// ---------------------------------------------------------------------------

use crate::snapshot::{AircraftPose, RenderSnapshot};
use std::collections::BTreeMap;
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::{
    attitude::{Basis, Vector},
    combat::{
        blast::MarkKind,
        missiles,
        smoke::{Kind, Smoke, contrail_altitude_ft},
    },
    wreck,
};

/// How a missile's motor burns, in simulation ticks after launch: smoke
/// leaves it only while it is powered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Motor {
    ignite: u64,
    burnout: u64,
}

impl Motor {
    /// The motor of `weapon`, or `None` for one that leaves no smoke (a gun
    /// round, or a store with no signature).
    pub fn of(weapon: &Weapon) -> Option<Self> {
        if weapon.seeker.signature == 0 {
            return None;
        }
        // `missiles::phase` is the rule; the two ticks it switches on are
        // found by asking it, so the rule has one home.
        let m = &weapon.movement;
        let powered = |age| missiles::phase(m, age) == tore_sim::combat::EnginePhase::Powered;
        let ignite =
            (0..=u64::from(m.ignite_t.max(m.fuel_t)) * 30 + 1).find(|age| powered(*age))?;
        let burnout = (ignite..=u64::from(m.fuel_t) * 30 + 1).find(|age| !powered(*age))?;
        Some(Self { ignite, burnout })
    }

    /// Whether the motor is burning `age` ticks after launch.
    pub fn powered(&self, age: u64) -> bool {
        (self.ignite..self.burnout).contains(&age)
    }
}

/// What a client knows about the mission that the picture does not say.
pub struct Surroundings<'a> {
    /// The mission's ground and wind.
    pub terrain: &'a Terrain,
    /// Where each aircraft type's engines exhaust, in feet right, up and
    /// forward of the aircraft's centre.
    pub outlets: &'a dyn Fn(AircraftId) -> &'a [Vector],
    /// The motor of the weapon a projectile carries, by the weapon's record
    /// name.
    pub motor: &'a dyn Fn(&str) -> Option<Motor>,
    /// The mission's contrail sortie, which sets each aircraft's onset
    /// altitude ([`contrail_altitude_ft`]).
    pub sortie: u64,
}

/// Everything a networked flight draws that the host does not send: the
/// smoke of hits, wrecks, motors and crash sites, the contrails, and the
/// chaff and flares in the air. Stepped once per client tick with the
/// picture drawn then, by the rules combat steps them with (agent decision:
/// the rules are the simulation's, the inputs are what the picture shows, so
/// a puff's phase, a missile's age and an outlet's first step can differ a
/// tick or two from the host's).
#[derive(Default)]
pub struct Effects {
    /// The puffs of hits, wrecks and motors, and of burning crash sites.
    pub smoke: Smoke,
    pub contrails: Smoke,
    pub devices: Devices,
    /// The client tick of every projectile's first appearance, which dates
    /// its motor.
    first_seen: BTreeMap<u32, u64>,
    tick: u64,
}

impl Effects {
    /// Advances one 120 Hz client tick over `picture`, then lets `releases`
    /// go (a device appears where it left, as in combat).
    pub fn step(
        &mut self,
        picture: &RenderSnapshot,
        around: &Surroundings<'_>,
        releases: &[DeviceRelease],
    ) {
        self.tick += 1;
        let wind = around.terrain.wind();
        self.smoke.wind = wind;
        self.contrails.wind = wind;

        // Projectiles: date each by the tick it was first drawn.
        self.first_seen.retain(|id, _| {
            picture
                .projectiles
                .iter()
                .any(|projectile| projectile.id == *id)
        });
        for projectile in &picture.projectiles {
            self.first_seen.entry(projectile.id).or_insert(self.tick);
        }

        let mut sources: Vec<(Vector, Kind)> = Vec::new();
        for projectile in picture.projectiles.iter().filter(|p| !p.gun) {
            let age = self.tick - self.first_seen[&projectile.id];
            if (around.motor)(&projectile.weapon).is_some_and(|motor| motor.powered(age)) {
                sources.push((
                    std::array::from_fn(|i| projectile.position[i] - projectile.direction[i] * 4.),
                    Kind::Missile,
                ));
            }
        }
        let poses = std::iter::once(&picture.player).chain(&picture.targets);
        for pose in poses.clone().filter(|pose| smoking(pose)) {
            let basis = basis_of(pose);
            sources.push((
                std::array::from_fn(|i| pose.position[i] - basis.forward[i] * 15.),
                Kind::Aircraft,
            ));
        }
        sources.extend(
            picture
                .marks
                .iter()
                .filter(|mark| mark.kind == MarkKind::Fire)
                .map(|mark| {
                    (
                        [
                            mark.position[0],
                            mark.position[1] + tore_sim::combat::smoke::BURNING_SOURCE_FT,
                            mark.position[2],
                        ],
                        Kind::Burning,
                    )
                }),
        );
        self.smoke.step(sources);

        // Contrails: every flying aircraft's engine outlets above its onset
        // altitude, keyed as combat keys them.
        let mut outlets = Vec::new();
        for pose in poses {
            let Some(aircraft) = pose.aircraft else {
                continue;
            };
            if !pose.airborne
                || pose.crashed
                || pose.damage.hp <= 0
                || !pose.engine.lit
                || pose.position[1] < contrail_altitude_ft(around.sortie, pose.id)
            {
                continue;
            }
            let basis = basis_of(pose);
            for (engine, offset) in (around.outlets)(aircraft).iter().enumerate() {
                let point = std::array::from_fn(|i| {
                    pose.position[i]
                        + basis.right[i] * offset[0]
                        + basis.up[i] * offset[1]
                        + basis.forward[i] * offset[2]
                });
                outlets.push((u64::from(pose.id) * 2 + engine as u64, point));
            }
        }
        self.contrails.step([]);
        self.contrails.contrails(outlets);

        fly_devices(&mut self.devices, around.terrain);
        for release in releases {
            release_device(&mut self.devices, release);
        }
    }
}

/// An aircraft pose's orientation.
fn basis_of(pose: &AircraftPose) -> Basis {
    let [yaw, pitch, bank] = pose.attitude;
    Basis::new(yaw, pitch, bank)
}

/// Whether an aircraft trails the smoke of its damage: flying and at least
/// half its hit points lost, until its wreck lies on the ground.
fn smoking(pose: &AircraftPose) -> bool {
    pose.airborne
        && pose.damage.initial_hp > 0
        && pose.damage.fraction() >= 0.5
        && !matches!(
            pose.wreck,
            Some(wreck::Phase::Grounded | wreck::Phase::Exploded)
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Damage, Draw, Engine, MarkPose, ProjectilePose};
    use tore_sim::combat::countermeasures::Release;

    fn terrain() -> Terrain {
        tore_world::test_support::terrain()
    }

    fn missile_weapon() -> Weapon {
        let resources = tore_world::test_support::resources::resources();
        Weapon::parse("AIM9M.JT", &resources["AIM9M.JT"]).unwrap()
    }

    fn missile(id: u32, weapon: &Weapon) -> ProjectilePose {
        ProjectilePose {
            id,
            owner: 1,
            weapon: weapon.source.clone(),
            shape: None,
            gun: false,
            tracer: false,
            position: [100., 2_000., 100.],
            previous: [100., 2_000., 100.],
            direction: [0., 0., 1.],
            target: None,
            incoming: false,
            speed_f8: 256 * 1_000,
        }
    }

    fn aircraft(id: u32, position: Vector, hp: i32) -> AircraftPose {
        AircraftPose {
            id,
            aircraft: Some(AircraftId::F18),
            draw: Draw::Model(AircraftId::F18),
            position,
            attitude: [0.; 3],
            velocity: [0., 0., 600.],
            devices: None,
            engine: Engine {
                lit: true,
                ..Engine::default()
            },
            damage: Damage {
                hp,
                initial_hp: 100,
                ..Damage::default()
            },
            airborne: true,
            wreck: None,
            crashed: false,
        }
    }

    const OUTLETS: [Vector; 2] = [[-3., 0., -20.], [3., 0., -20.]];

    fn around<'a>(
        terrain: &'a Terrain,
        motor: &'a dyn Fn(&str) -> Option<Motor>,
        outlets: &'a dyn Fn(AircraftId) -> &'a [Vector],
    ) -> Surroundings<'a> {
        Surroundings {
            terrain,
            outlets,
            motor,
            sortie: 0,
        }
    }

    #[test]
    fn a_motor_burns_from_ignition_to_burnout_and_a_gun_has_none() {
        let weapon = missile_weapon();
        let motor = Motor::of(&weapon).expect("a missile with a signature has a motor");
        let m = &weapon.movement;
        assert!(motor.powered(u64::from(m.ignite_t) * 30));
        assert!(motor.powered(u64::from(m.fuel_t) * 30 - 1));
        assert!(!motor.powered(u64::from(m.fuel_t) * 30));
        let resources = tore_world::test_support::resources::resources();
        let gun = Weapon::parse("M61.JT", &resources["M61.JT"]).unwrap();
        assert_eq!(Motor::of(&gun), None);
    }

    #[test]
    fn a_missile_smokes_every_eighth_tick_while_powered_and_then_stops() {
        let (terrain, weapon) = (terrain(), missile_weapon());
        let burn = u64::from(weapon.movement.fuel_t) * 30;
        let motor = |name: &str| {
            (name == weapon.source)
                .then(|| Motor::of(&weapon))
                .flatten()
        };
        let outlets = |_: AircraftId| &OUTLETS[..];
        let around = around(&terrain, &motor, &outlets);
        let mut effects = Effects::default();
        let picture = RenderSnapshot {
            projectiles: vec![missile(7, &weapon)],
            ..RenderSnapshot::default()
        };
        for _ in 0..burn + 10 {
            effects.step(&picture, &around, &[]);
        }
        let puffs = &effects.smoke.puffs;
        assert!(puffs.iter().all(|p| p.kind == Kind::Missile));
        // One puff in eight ticks of a burn that lasts `burn` ticks, the
        // newest of them (age 10 to 12 at most) well inside the 480-tick life.
        assert_eq!(puffs.len() as u64, burn / 8);
        assert!(puffs.back().unwrap().age >= 10);
        // A round that is a gun makes none.
        let mut gun = missile(8, &weapon);
        gun.gun = true;
        let mut quiet = Effects::default();
        let picture = RenderSnapshot {
            projectiles: vec![gun],
            ..RenderSnapshot::default()
        };
        for _ in 0..100 {
            quiet.step(&picture, &around, &[]);
        }
        assert!(quiet.smoke.puffs.is_empty());
    }

    #[test]
    fn a_missile_that_leaves_the_picture_and_returns_is_a_new_launch() {
        let (terrain, weapon) = (terrain(), missile_weapon());
        let motor = |name: &str| {
            (name == weapon.source)
                .then(|| Motor::of(&weapon))
                .flatten()
        };
        let outlets = |_: AircraftId| &OUTLETS[..];
        let around = around(&terrain, &motor, &outlets);
        let mut effects = Effects::default();
        let with = RenderSnapshot {
            projectiles: vec![missile(7, &weapon)],
            ..RenderSnapshot::default()
        };
        for _ in 0..400 {
            effects.step(&with, &around, &[]);
        }
        let before = effects.smoke.puffs.len();
        effects.step(&RenderSnapshot::default(), &around, &[]);
        effects.step(&with, &around, &[]);
        for _ in 0..16 {
            effects.step(&with, &around, &[]);
        }
        assert!(effects.smoke.puffs.len() > before - 2, "burning again");
    }

    #[test]
    fn an_aircraft_at_half_hit_points_trails_smoke_until_its_wreck_is_down() {
        let terrain = terrain();
        let motor = |_: &str| None;
        let outlets = |_: AircraftId| &OUTLETS[..];
        let around = around(&terrain, &motor, &outlets);
        let mut hurt = aircraft(3, [0., 5_000., 0.], 50);
        let healthy = aircraft(4, [500., 5_000., 0.], 51);
        let mut effects = Effects::default();
        let picture = RenderSnapshot {
            player: aircraft(0, [900., 5_000., 0.], 100),
            targets: vec![hurt.clone(), healthy],
            ..RenderSnapshot::default()
        };
        for _ in 0..120 {
            effects.step(&picture, &around, &[]);
        }
        // Ten puffs in a second, all from the hurt aircraft, 15 feet behind it.
        assert_eq!(effects.smoke.puffs.len(), 10);
        assert!(effects.smoke.puffs.iter().all(|p| p.kind == Kind::Aircraft));
        let wind = terrain.wind();
        assert!(effects.smoke.puffs.iter().all(|p| {
            let seconds = f64::from(p.age) / 120.;
            (p.position[0] - wind[0] * seconds).abs() < 1e-6
        }));
        let newest = effects.smoke.puffs.back().unwrap();
        assert!((newest.position[2] - wind[2] * f64::from(newest.age) / 120. + 15.).abs() < 1e-6);
        hurt.wreck = Some(wreck::Phase::Grounded);
        let mut grounded = Effects::default();
        let picture = RenderSnapshot {
            targets: vec![hurt],
            ..RenderSnapshot::default()
        };
        for _ in 0..120 {
            grounded.step(&picture, &around, &[]);
        }
        assert!(grounded.smoke.puffs.is_empty());
    }

    #[test]
    fn a_fire_mark_raises_a_column_from_twenty_feet_above_it() {
        let terrain = terrain();
        let motor = |_: &str| None;
        let outlets = |_: AircraftId| &OUTLETS[..];
        let around = around(&terrain, &motor, &outlets);
        let picture = RenderSnapshot {
            marks: vec![
                MarkPose {
                    kind: MarkKind::Fire,
                    position: [10., 100., 10.],
                    age: 0,
                    strength: 1.,
                },
                MarkPose {
                    kind: MarkKind::Crater(0),
                    position: [500., 100., 10.],
                    age: 0,
                    strength: 1.,
                },
            ],
            ..RenderSnapshot::default()
        };
        let mut effects = Effects::default();
        for _ in 0..24 {
            effects.step(&picture, &around, &[]);
        }
        assert_eq!(effects.smoke.puffs.len(), 2);
        assert!(effects.smoke.puffs.iter().all(|p| p.kind == Kind::Burning));
        assert!(effects.smoke.puffs.iter().all(|p| p.position[1] > 119.));
    }

    #[test]
    fn contrails_form_above_each_aircraft_onset_altitude_from_each_engine() {
        let terrain = terrain();
        let motor = |_: &str| None;
        let outlets = |_: AircraftId| &OUTLETS[..];
        let around = around(&terrain, &motor, &outlets);
        let (high, low) = (
            aircraft(5, [0., 36_000., 0.], 100),
            aircraft(6, [0., 20_000., 0.], 100),
        );
        let mut effects = Effects::default();
        for tick in 0..24 {
            // Moving, as outlets must have moved to leave a puff.
            let mut moving = high.clone();
            moving.position[2] = f64::from(tick) * 5.;
            let mut flying_low = low.clone();
            flying_low.position[2] = f64::from(tick) * 5.;
            let picture = RenderSnapshot {
                targets: vec![moving, flying_low],
                ..RenderSnapshot::default()
            };
            effects.step(&picture, &around, &[]);
        }
        // Two engines, one puff each per twelve ticks, twice in 24 ticks.
        assert_eq!(effects.contrails.puffs.len(), 4);
        assert!(
            effects
                .contrails
                .puffs
                .iter()
                .all(|p| p.kind == Kind::Contrail)
        );
        assert!(
            effects
                .contrails
                .puffs
                .iter()
                .all(|p| p.position[1] > 35_000.)
        );
    }

    #[test]
    fn released_devices_fly_as_the_replay_flies_them() {
        let terrain = terrain();
        let motor = |_: &str| None;
        let outlets = |_: AircraftId| &OUTLETS[..];
        let around = around(&terrain, &motor, &outlets);
        let release = |number, kind| DeviceRelease {
            owner: 1,
            kind,
            release: Release {
                position: [100., 3_000., 100.],
                velocity: [0., 0., 600.],
                basis: Basis::new(0.2, 0.1, 0.3),
            },
            number,
            tick: 0,
        };
        let releases = [release(1, EffectKind::Flare), release(2, EffectKind::Chaff)];
        let mut effects = Effects::default();
        let mut direct = Devices::default();
        effects.step(&RenderSnapshot::default(), &around, &releases);
        fly_devices(&mut direct, &terrain);
        for release in &releases {
            release_device(&mut direct, release);
        }
        for _ in 0..500 {
            effects.step(&RenderSnapshot::default(), &around, &[]);
            fly_devices(&mut direct, &terrain);
            assert_eq!(effects.devices.flares, direct.flares);
            assert_eq!(effects.devices.chaff, direct.chaff);
        }
        assert!(!effects.devices.flares.is_empty() && !effects.devices.chaff.is_empty());
    }
}
