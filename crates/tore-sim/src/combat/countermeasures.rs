//! Opinionated chaff and flare presentation (docs/spec/countermeasures.md).
//! Visual devices only: decoy odds are rolled at release and nothing here
//! reaches a seeker, the combat random stream or flight forces.
use crate::attitude::{Basis, Vector, dot};
use std::collections::VecDeque;

const TICK: f64 = 1. / 120.;
const GRAVITY: f64 = 32.174;
/// Terminal fall speed of a burning flare, ft/s.
const FLARE_TERMINAL: f64 = 100.;
/// 30 seconds, requested by John on 2026-09-26.
pub const FLARE_BURN_TICKS: u16 = 3600;
const FLARE_IGNITION_TICKS: u16 = 12;
/// The last 3 seconds: dimming while the flicker grows into a sputter,
/// requested by John on 2026-09-26.
pub const FLARE_FADE_TICKS: u16 = 360;
/// Sideways travel from the tail: 20 to 30 ft, 95 percent reached in 0.75 s.
const FLARE_SIDE_FEET: [f64; 2] = [20., 30.];
const FLARE_SIDE_SECONDS: f64 = 0.25;
const FLARE_KICK: f64 = 15.;
/// Where devices leave the airframe, relative to its reference point.
const TAIL_FEET: f64 = 15.;
const BELOW_FEET: f64 = 2.;
/// A landed flare rests this high, so the ground never hides its core.
const REST_FEET: f64 = 1.5;
pub const PUFF_SPACING_FEET: f64 = 5.;
const PUFF_MAX_GAP_TICKS: u16 = 6;
pub const PUFF_TRAIL_FEET: f64 = 200.;
pub const PUFF_LIFETIME_TICKS: u16 = 360;
const PUFF_FORMING_TICKS: u16 = 12;
/// Half-angle of the upward cone puffs drift in.
pub const PUFF_CONE_DEGREES: f64 = 30.;
const PUFF_SPEED: [f64; 2] = [8., 15.];
const PUFF_DRIFT_SECONDS: f64 = 1.5;
pub const MAX_FLARES: usize = 128;
pub const MAX_PUFFS_PER_FLARE: usize = 96;
pub const MAX_FLARE_PUFFS: usize = MAX_FLARES * MAX_PUFFS_PER_FLARE;
pub const CHAFF_LIFETIME_TICKS: u16 = 2400;
const CHAFF_FADE_TICKS: u16 = 600;
const CHAFF_FALL: f64 = 4.;
const CHAFF_DRAG_SECONDS: f64 = 0.12;
pub const MAX_CHAFF: usize = 64;

/// Where and how the releasing aircraft was moving.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Release {
    pub position: Vector,
    pub velocity: Vector,
    pub basis: Basis,
}
impl Release {
    fn outlet(&self) -> Vector {
        std::array::from_fn(|i| {
            self.position[i] - self.basis.forward[i] * TAIL_FEET - self.basis.up[i] * BELOW_FEET
        })
    }
}

/// SplitMix64 finaliser: stable per release, never shared with gameplay.
fn mix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
/// Uniform in [0, 1) from a seed and a stream number.
fn unit(seed: u64, stream: u64) -> f64 {
    (mix(seed ^ mix(stream)) >> 11) as f64 / (1_u64 << 53) as f64
}
fn range(seed: u64, stream: u64, [lo, hi]: [f64; 2]) -> f64 {
    lo + (hi - lo) * unit(seed, stream)
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlarePuff {
    pub position: Vector,
    velocity: Vector,
    pub age: u16,
    /// The flare's path length when this puff was left behind.
    path: f64,
    opacity: f32,
}
impl FlarePuff {
    pub fn radius(&self) -> f64 {
        2.5 + 4. * f64::from(self.age) / 120.
    }
    pub fn opacity(&self) -> f32 {
        self.opacity
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Flare {
    pub position: Vector,
    /// Ballistic velocity through the air, excluding the sideways throw.
    velocity: Vector,
    /// Sideways throw: unit direction times its full distance.
    side: Vector,
    /// Movement over the last tick, ft/s, for the flame's stretch.
    pub motion: Vector,
    pub age: u16,
    seed: u64,
    path: f64,
    last_puff: (f64, u16),
    resting: bool,
    pub puffs: VecDeque<FlarePuff>,
}
impl Flare {
    pub fn burning(&self) -> bool {
        self.age < FLARE_BURN_TICKS
    }
    /// Relative brightness: 0 before ignition and after burnout, with a
    /// deterministic ±15 percent flicker while it burns. Over the last three
    /// seconds it dims to nothing while the flicker grows into a sputter.
    pub fn intensity(&self) -> f64 {
        if !self.burning() {
            return 0.;
        }
        let ignition = (f64::from(self.age) / f64::from(FLARE_IGNITION_TICKS)).min(1.);
        let fade = (f64::from(FLARE_BURN_TICKS - self.age) / f64::from(FLARE_FADE_TICKS)).min(1.);
        let dying = 1. - fade;
        let t = f64::from(self.age) * TICK;
        let phase = |stream| unit(self.seed, stream) * std::f64::consts::TAU;
        let wave = |hz: f64, stream| (std::f64::consts::TAU * hz * t + phase(stream)).sin();
        let flicker = 0.6 * wave(7.3, 10) + 0.4 * wave(13.1, 11);
        let sputter = 0.5 * wave(17.9, 12) + 0.5 * wave(29.3, 13);
        let amount = 0.15 + 0.55 * dying;
        let wobble = flicker * (1. - dying) + sputter * dying;
        (ignition * fade * (1. + amount * wobble)).max(0.)
    }
    /// Seed shared with the renderer's flame noise.
    pub fn seed(&self) -> u32 {
        self.seed as u32
    }
    fn step(&mut self, ground: &impl Fn(f64, f64) -> f64) {
        let before = self.position;
        if !self.resting {
            let speed = dot(self.velocity, self.velocity).sqrt();
            let drag = GRAVITY / (FLARE_TERMINAL * FLARE_TERMINAL) * speed;
            let damping = 1. / (1. + drag * TICK);
            for i in 0..3 {
                self.velocity[i] *= damping;
            }
            self.velocity[1] -= GRAVITY * TICK;
            let t = f64::from(self.age) * TICK;
            let throw = (-t / FLARE_SIDE_SECONDS).exp() - (-(t + TICK) / FLARE_SIDE_SECONDS).exp();
            for i in 0..3 {
                self.position[i] += self.velocity[i] * TICK + self.side[i] * throw;
            }
            let floor = ground(self.position[0], self.position[2]);
            if self.position[1] <= floor + REST_FEET {
                self.position[1] = floor + REST_FEET;
                self.velocity = [0.; 3];
                self.resting = true;
            }
        }
        let moved: Vector = std::array::from_fn(|i| self.position[i] - before[i]);
        self.motion = moved.map(|v| v / TICK);
        self.path += dot(moved, moved).sqrt();
        self.age = self.age.saturating_add(1);
        for puff in &mut self.puffs {
            let slow = (-TICK / PUFF_DRIFT_SECONDS).exp();
            for i in 0..3 {
                puff.position[i] += puff.velocity[i] * TICK;
                puff.velocity[i] *= slow;
            }
            puff.age = puff.age.saturating_add(1);
            let aged = 1. - f64::from(puff.age) / f64::from(PUFF_LIFETIME_TICKS);
            let behind = 1. - (self.path - puff.path) / PUFF_TRAIL_FEET;
            // Smoke thickens just behind the flame, leaving the head clear.
            let forming = (f64::from(puff.age) / f64::from(PUFF_FORMING_TICKS)).min(1.);
            puff.opacity = (0.55 * forming * aged.min(behind).max(0.)) as f32;
        }
        self.puffs
            .retain(|p| p.age < PUFF_FORMING_TICKS || p.opacity > 0.);
        if self.burning()
            && (self.path - self.last_puff.0 >= PUFF_SPACING_FEET
                || self.age - self.last_puff.1 >= PUFF_MAX_GAP_TICKS)
        {
            self.last_puff = (self.path, self.age);
            if self.puffs.len() == MAX_PUFFS_PER_FLARE {
                self.puffs.pop_front();
            }
            // Uniform over the spherical cap within the cone around straight up.
            let stream = u64::from(self.age) * 4;
            let cos_min = PUFF_CONE_DEGREES.to_radians().cos();
            let cos = cos_min + (1. - cos_min) * unit(self.seed, stream);
            let sin = (1. - cos * cos).max(0.).sqrt();
            let azimuth = std::f64::consts::TAU * unit(self.seed, stream + 1);
            let speed = range(self.seed, stream + 2, PUFF_SPEED);
            self.puffs.push_back(FlarePuff {
                position: self.position,
                velocity: [
                    sin * azimuth.cos() * speed,
                    cos * speed,
                    sin * azimuth.sin() * speed,
                ],
                age: 0,
                path: self.path,
                opacity: 0.,
            });
        }
    }
    fn finished(&self) -> bool {
        !self.burning() && self.puffs.is_empty()
    }
}

/// One chaff cartridge. The renderer spreads its strips from `seed` and `age`.
#[derive(Clone, Debug, PartialEq)]
pub struct Chaff {
    pub position: Vector,
    velocity: Vector,
    pub age: u16,
    pub seed: u32,
}
impl Chaff {
    pub fn seconds(&self) -> f64 {
        f64::from(self.age) * TICK
    }
    pub fn opacity(&self) -> f32 {
        (f32::from(CHAFF_LIFETIME_TICKS - self.age.min(CHAFF_LIFETIME_TICKS))
            / f32::from(CHAFF_FADE_TICKS))
        .min(1.)
    }
    fn step(&mut self, ground: &impl Fn(f64, f64) -> f64) {
        // Relative to still air the cloud stops almost at once, then settles.
        let keep = (-TICK / CHAFF_DRAG_SECONDS).exp();
        self.velocity[0] *= keep;
        self.velocity[2] *= keep;
        self.velocity[1] = -CHAFF_FALL + (self.velocity[1] + CHAFF_FALL) * keep;
        for i in 0..3 {
            self.position[i] += self.velocity[i] * TICK;
        }
        let floor = ground(self.position[0], self.position[2]);
        if self.position[1] < floor {
            self.position[1] = floor;
            self.velocity = [0.; 3];
        }
        self.age = self.age.saturating_add(1);
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Devices {
    pub flares: VecDeque<Flare>,
    pub chaff: VecDeque<Chaff>,
    releases: u64,
}
impl Devices {
    /// How many devices have been released so far. Each release's number,
    /// and with it the device's look, follows from this count.
    pub fn released(&self) -> u64 {
        self.releases
    }
    /// Continues the numbering after `released` earlier releases, so a
    /// device rebuilt from a mission recording looks as it did in flight.
    pub fn continue_after(&mut self, released: u64) {
        self.releases = released;
    }
    fn seed(&mut self) -> u64 {
        self.releases += 1;
        mix(self.releases)
    }
    /// One flare device leaves as a pair, one thrown to each side.
    pub fn release_flare(&mut self, release: Release) {
        let seed = self.seed();
        let outlet = release.outlet();
        for (n, sign) in [(0, 1.), (1, -1.)] {
            if self.flares.len() == MAX_FLARES {
                self.flares.pop_front();
            }
            let seed = mix(seed ^ n);
            let side = range(seed, 0, FLARE_SIDE_FEET) * sign;
            self.flares.push_back(Flare {
                position: outlet,
                velocity: std::array::from_fn(|i| {
                    release.velocity[i] - release.basis.up[i] * FLARE_KICK
                }),
                side: release.basis.right.map(|v| v * side),
                motion: release.velocity,
                age: 0,
                seed,
                path: 0.,
                last_puff: (0., 0),
                resting: false,
                puffs: VecDeque::new(),
            });
        }
    }
    pub fn release_chaff(&mut self, release: Release) {
        let seed = self.seed();
        if self.chaff.len() == MAX_CHAFF {
            self.chaff.pop_front();
        }
        self.chaff.push_back(Chaff {
            position: release.outlet(),
            velocity: release.velocity,
            age: 0,
            seed: seed as u32,
        });
    }
    /// Exactly one 120 Hz combat tick.
    pub fn step(&mut self, ground: &impl Fn(f64, f64) -> f64) {
        for flare in &mut self.flares {
            flare.step(ground);
        }
        self.flares.retain(|f| !f.finished());
        for chaff in &mut self.chaff {
            chaff.step(ground);
        }
        self.chaff.retain(|c| c.age < CHAFF_LIFETIME_TICKS);
    }
    pub fn puffs(&self) -> impl Iterator<Item = &FlarePuff> {
        self.flares.iter().flat_map(|f| f.puffs.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attitude::cross;

    fn level(speed: f64) -> Release {
        Release {
            position: [0., 10000., 0.],
            velocity: [0., 0., speed],
            basis: Basis::new(0., 0., 0.),
        }
    }
    fn run(devices: &mut Devices, ticks: usize) {
        for _ in 0..ticks {
            devices.step(&|_, _| 0.);
        }
    }

    #[test]
    fn a_flare_release_is_a_mirrored_pair_thrown_twenty_to_thirty_feet() {
        for seed in 0..20 {
            let mut devices = Devices {
                releases: seed,
                ..Default::default()
            };
            let release = level(675.);
            devices.release_flare(release);
            assert_eq!(devices.flares.len(), 2);
            run(&mut devices, 120);
            let sides: Vec<f64> = devices
                .flares
                .iter()
                .map(|f| dot(f.position, release.basis.right))
                .collect();
            assert!(sides[0] > 0. && sides[1] < 0., "{sides:?}");
            for side in sides {
                assert!((19. ..=30.).contains(&side.abs()), "{side}");
            }
            // Drag leaves the pair well behind a 400 kt jet after a second.
            let aircraft = 675.;
            for flare in &devices.flares {
                assert!(aircraft - flare.position[2] > 250.);
                assert!(flare.position[1] < 10000. - BELOW_FEET);
            }
        }
    }

    #[test]
    fn flares_burn_thirty_seconds_with_a_flicker_and_rest_on_the_ground() {
        let mut devices = Devices::default();
        devices.release_flare(Release {
            position: [0., 40., 0.],
            ..level(300.)
        });
        let mut brightest: f64 = 0.;
        let mut dimmest: f64 = 2.;
        for tick in 1..=FLARE_BURN_TICKS - FLARE_FADE_TICKS {
            devices.step(&|_, _| 0.);
            if tick >= 24 {
                brightest = brightest.max(devices.flares[0].intensity());
                dimmest = dimmest.min(devices.flares[0].intensity());
            }
        }
        assert!((1.1..=1.15).contains(&brightest));
        assert!((0.85..0.9).contains(&dimmest));
        // The last three seconds dim and sputter.
        let mut previous = devices.flares[0].intensity();
        let mut swings = 0;
        for _ in 0..FLARE_FADE_TICKS - 1 {
            devices.step(&|_, _| 0.);
            let now = devices.flares[0].intensity();
            if (now - previous).abs() > 0.05 {
                swings += 1;
            }
            previous = now;
        }
        assert!(swings > 20, "{swings}");
        assert!(previous < 0.02);
        devices.step(&|_, _| 0.);
        let flare = &devices.flares[0];
        assert!(!flare.burning());
        assert_eq!(flare.intensity(), 0.);
        assert_eq!(flare.position[1], REST_FEET);
        assert!(flare.resting);
        // It kept burning where it landed.
        assert!(flare.puffs.iter().any(|p| p.age < 20));
        // Smoke outlives the flame, then the device retires.
        run(&mut devices, usize::from(PUFF_LIFETIME_TICKS));
        assert!(devices.flares.is_empty());
    }

    #[test]
    fn flare_smoke_rises_inside_the_cone_and_fades_by_two_hundred_feet() {
        let mut devices = Devices::default();
        devices.release_flare(level(675.));
        let cone = PUFF_CONE_DEGREES.to_radians().cos();
        for _ in 0..360 {
            devices.step(&|_, _| 0.);
            for flare in &devices.flares {
                for puff in &flare.puffs {
                    let speed = dot(puff.velocity, puff.velocity).sqrt();
                    assert!(puff.velocity[1] / speed >= cone - 1e-9);
                    let behind = flare.path - puff.path;
                    if behind >= PUFF_TRAIL_FEET {
                        panic!("puff {behind} ft behind is still drawn");
                    }
                }
            }
        }
        let flare = &devices.flares[0];
        let trail = flare.path - flare.puffs.front().unwrap().path;
        assert!(trail < PUFF_TRAIL_FEET);
        // Early, fast flight leaves a puff every eight feet of path.
        assert!(flare.puffs.len() >= 20);
    }

    #[test]
    fn chaff_stops_in_the_air_settles_and_expires() {
        let mut devices = Devices::default();
        devices.release_chaff(level(675.));
        run(&mut devices, 120);
        let chaff = &devices.chaff[0];
        assert!(chaff.velocity[2].abs() < 0.5);
        assert!((chaff.velocity[1] + CHAFF_FALL).abs() < 0.01);
        // 0.12 s time constant: about 80 ft of carry at 675 ft/s.
        assert!((chaff.position[2] - (-TAIL_FEET + 675. * 0.12)).abs() < 5.);
        assert_eq!(chaff.opacity(), 1.);
        run(
            &mut devices,
            usize::from(CHAFF_LIFETIME_TICKS - CHAFF_FADE_TICKS / 2 - 120),
        );
        assert!((devices.chaff[0].opacity() - 0.5).abs() < 0.01);
        run(&mut devices, usize::from(CHAFF_FADE_TICKS / 2));
        assert!(devices.chaff.is_empty());
    }

    #[test]
    fn budgets_retire_the_oldest_devices_first() {
        let mut devices = Devices::default();
        for n in 0..MAX_CHAFF + 3 {
            devices.release_chaff(Release {
                position: [n as f64, 1000., 0.],
                ..level(0.)
            });
            devices.release_flare(level(0.));
        }
        assert_eq!(devices.chaff.len(), MAX_CHAFF);
        assert_eq!(devices.chaff[0].position[0], 3.);
        assert_eq!(devices.flares.len(), MAX_FLARES);
        run(&mut devices, 200);
        assert!(devices.puffs().count() <= MAX_FLARE_PUFFS);
    }

    #[test]
    fn devices_leave_from_the_tail_along_any_attitude() {
        let basis = Basis::new(1.1, 0.4, -0.7);
        let release = Release {
            position: [500., 2000., -300.],
            velocity: basis.forward.map(|v| v * 500.),
            basis,
        };
        let mut devices = Devices::default();
        devices.release_flare(release);
        devices.release_chaff(release);
        let offset: Vector =
            std::array::from_fn(|i| devices.chaff[0].position[i] - release.position[i]);
        assert!((dot(offset, basis.forward) + TAIL_FEET).abs() < 1e-9);
        assert!((dot(offset, basis.up) + BELOW_FEET).abs() < 1e-9);
        let side = cross(basis.up, basis.forward);
        assert!(dot(devices.flares[0].side, side) > 0.);
        assert!(dot(devices.flares[1].side, side) < 0.);
    }
}
