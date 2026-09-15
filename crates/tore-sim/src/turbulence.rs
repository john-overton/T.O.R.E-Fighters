//! Physical turbulence, translated from `_FMTurbulence` at 0x477590. Renderer
//! independent, with caller-owned state and RNG so replay stays deterministic.
//! This is the aerodynamic disturbance, not the separately named sound-path
//! `Turbulence` at 0x434550, which responds to maneuvering.
use tore_formats::Result;
use tore_formats::flight_model::clock_rng::NativeRng;

/// 0x477ce9: on the ground or with the preference off, reconsider in 512 ticks.
pub const SUPPRESSED_DELAY: i64 = 0x200;
/// 0x477a3a: with no strength at all, reconsider in five seconds.
pub const IDLE_DELAY: i64 = 0x500;
/// 0x4777d4: strength falls linearly to nothing at a thousand feet above ground.
pub const CEILING_FEET: f64 = 1000.;
/// 0x477a57: the inclusive daytime window, 07:00 to 19:00.
const DAY_START: i32 = 25_200;
const DAY_END: i32 = 68_400;
/// 0x477cbe: a vertical rate this large also shakes the view.
const SHAKE_RATE: i32 = 0xc00;

/// Everything the generator reads, supplied explicitly by the caller.
#[derive(Clone, Copy, Debug)]
pub struct Conditions {
    pub agl_feet: f64,
    pub on_ground: bool,
    pub speed_fps: f64,
    pub seconds_of_day: i32,
    /// The aircraft's own `turbulencePercent` source field.
    pub percent: i16,
    /// 0x477a69 scales daytime strength by two thirds when T_Info/Collision
    /// reports native surface class 1. Native object overrides stay caller-owned.
    pub daytime_ground: bool,
    /// 0x477593: a preference bit suppresses turbulence entirely.
    pub enabled: bool,
    /// Maximum reviewed nearby-aircraft contribution (0..100). Zero means the
    /// caller has no qualifying neighbor; it does not create one implicitly.
    pub nearby_strength: u8,
}

/// One tick of disturbance. Angles are rates in radians per second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Disturbance {
    pub vertical_fps: f64,
    pub yaw: f64,
    pub pitch: f64,
    pub roll: f64,
    pub shake: bool,
}

impl Disturbance {
    /// Normalised severity for haptics, from the actual generated amplitudes.
    pub fn severity(self) -> f64 {
        let angles = self.yaw.abs().max(self.pitch.abs()).max(self.roll.abs());
        (angles / 0.12 + self.vertical_fps.abs() / 14.).min(1.)
    }
}

/// Per-aircraft event state. The original keeps this per instance too.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Turbulence {
    next_update: i64,
    start: i64,
    end: i64,
    /// 0x477ab0 stores twice the event length; the sine runs over that.
    cycle: i64,
    /// 24.8 feet per second.
    vertical: i32,
    /// Yaw, pitch and roll amplitudes in binary angle units.
    amplitudes: [i32; 3],
    shake: bool,
}

/// 0x4777d4: full at the surface, nothing at or above the ceiling.
pub fn low_altitude_strength(agl_feet: f64) -> i32 {
    if !agl_feet.is_finite() || agl_feet >= CEILING_FEET {
        return 0;
    }
    // Retail divides the negative height term first, truncating towards zero.
    let height_f8 = (agl_feet.max(0.) * 256.) as i64;
    100 - (100 * height_f8 / 256_000) as i32
}

/// 0x477b20: nothing at rest, full between 146 and 293 feet per second, and
/// falling away again to nothing at 586.
fn speed_shape(speed: i32) -> i32 {
    if speed <= 0x92 {
        100 * speed / 0x92
    } else if speed <= 0x125 {
        100
    } else {
        (100 + 100 * (0x125 - speed) / 0x125).max(0)
    }
}

/// Reviewed distance/signature/angular consumer at 0x477870..0x477a13.
/// Distance is 24.8 feet; angles are the native `_Angles` result after the
/// 50-foot lateral/vertical dead zones. Producers remain explicit so callers
/// cannot quietly turn visual mesh bounds into a validated native wake volume.
pub fn wake_strength(distance_f8: i32, signature: i16, heading: i16, pitch: i16) -> u8 {
    if !(0..2000 * 256).contains(&distance_f8) {
        return 0;
    }
    let reach = i64::from(signature.clamp(75, 200)) * 125 * 4096 / 100;
    let distance = (100 - i64::from(distance_f8) * 100 / reach).clamp(0, 100);
    let angle = (32760 - i32::from(heading).abs()).max(i32::from(pitch).abs());
    let angular = (100 - 100 * angle / 1820).clamp(0, 100);
    (distance * i64::from(angular) / 100) as u8
}

/// Authored float producer for the reviewed scalar wake consumer. Only actual
/// aircraft supplied by a caller belong here; no fictitious neighbors are made.
#[derive(Clone, Copy, Debug)]
pub struct NearbyAircraft {
    pub position: [f64; 3],
    /// Heading, pitch, bank in radians, separately from motion/speed.
    pub attitude: [f64; 3],
    pub speed_fps: f64,
    /// Native type +0x3f, `sigs[0]`; not an inferred wing span or mass.
    pub signature: i16,
}
impl NearbyAircraft {
    pub fn strength_at(self, position: [f64; 3]) -> u8 {
        use crate::attitude::{Basis, dot};
        if self.speed_fps < 293.
            || !self.speed_fps.is_finite()
            || position
                .iter()
                .chain(self.position.iter())
                .chain(self.attitude.iter())
                .any(|v| !v.is_finite())
        {
            return 0;
        }
        let delta = std::array::from_fn(|i| position[i] - self.position[i]);
        let distance = dot(delta, delta).sqrt();
        if distance >= 2000. {
            return 0;
        }
        let b = Basis::new(self.attitude[0], self.attitude[1], self.attitude[2]);
        let dead_zone = |x: f64| x.signum() * (x.abs() - 50.).max(0.);
        let x = dead_zone(dot(delta, b.right));
        let y = dead_zone(dot(delta, b.up));
        let z = dot(delta, b.forward);
        // Float orientation/atan and conversion remain host adaptations. The
        // source uses the imported integer matrix/atan lookup tables instead.
        let angle = |r: f64| (r * 65520. / std::f64::consts::TAU).round() as i16;
        wake_strength(
            (distance * 256.).floor() as i32,
            self.signature,
            angle(x.atan2(z)),
            angle(y.atan2(x.hypot(z))),
        )
    }
}

impl Turbulence {
    /// One native service step. `service_ticks` is the elapsed clock delta.
    /// Draws happen in the original's order so a shared seed replays.
    pub fn step(
        &mut self,
        tick: i64,
        service_ticks: i32,
        c: Conditions,
        rng: &mut NativeRng,
    ) -> Result<Disturbance> {
        if !c.enabled || c.on_ground {
            // 0x477ce4: clear the event outright rather than letting it finish.
            self.next_update = tick + SUPPRESSED_DELAY;
            self.start = 0;
            self.end = 0;
            return Ok(Disturbance::default());
        }
        // 0x4775b5 tests the active interval before the recurrence timer.
        if self.start <= tick && tick < self.end {
            return Ok(self.apply(tick, service_ticks));
        }
        if tick >= self.next_update {
            self.generate(tick, c, rng)?;
        }
        // Generation returns without applying the newly created event.
        Ok(Disturbance::default())
    }

    fn generate(&mut self, tick: i64, c: Conditions, rng: &mut NativeRng) -> Result<()> {
        let strength = low_altitude_strength(c.agl_feet).max(i32::from(c.nearby_strength.min(100)));
        if strength <= 0 {
            self.next_update = tick + IDLE_DELAY;
            return Ok(());
        }
        let mut percent = i32::from(c.percent).max(0);
        if (DAY_START..=DAY_END).contains(&c.seconds_of_day) {
            if c.daytime_ground {
                percent = 2 * percent / 3;
            }
        } else {
            percent /= 4;
        }
        let speed = if c.speed_fps.is_finite() {
            (c.speed_fps as i32).max(0)
        } else {
            0
        };
        // 0x477a9e reads AX (quotient), not DX (remainder): 89..165 ticks.
        let length = i64::from(rng.below(0x1e00)? / 100 + 0x59);
        self.start = tick;
        self.end = tick + length;
        self.cycle = length * 2;
        // 0x477ab7: a stronger disturbance is also a more frequent one.
        let pace = (100 * speed / 0x24a).min(100) * strength / 100 * percent / 100;
        let divisor = 60 * pace / 100 + 15;
        self.next_update = tick + i64::from(rng.below(2 * (0x3c00 / divisor))?);
        // 0x477b73: yaw, pitch and roll scale as 1.82, 7.28 and 12.74 per unit.
        let amount = strength * speed_shape(speed) / 100 * percent / 100;
        for (axis, scale) in [182, 728, 1274].into_iter().enumerate() {
            self.amplitudes[axis] = rng.below(scale * amount / 100)?;
        }
        for amplitude in &mut self.amplitudes {
            if rng.chance(50)? {
                *amplitude = -*amplitude;
            }
        }
        // 0x477c3f: the vertical rate peaks at a different speed again.
        let lift = (100 * speed / 0x2dd).clamp(0, 100) * strength / 100 * percent / 100;
        let mut vertical = rng.below(0xe00)? * lift / 100;
        if rng.chance(50)? {
            vertical = -vertical;
        }
        self.vertical = vertical;
        self.shake = vertical.abs() >= SHAKE_RATE;
        Ok(())
    }

    /// 0x4775b5: outside its window an event contributes nothing.
    fn apply(&self, tick: i64, service_ticks: i32) -> Disturbance {
        if tick < self.start || tick >= self.end || self.cycle <= 0 || service_ticks <= 0 {
            return Disturbance::default();
        }
        // 0x477618: the phase spans one full turn across the doubled period.
        let elapsed = (tick - self.start).rem_euclid(self.cycle);
        let phase = elapsed as f64 * 65520. / self.cycle as f64;
        let sine = (phase * std::f64::consts::TAU / 65536.).sin();
        let rate = |amplitude: i32| f64::from(amplitude) * sine * std::f64::consts::TAU / 65536.;
        Disturbance {
            vertical_fps: f64::from(self.vertical) / 256.,
            yaw: rate(self.amplitudes[0]),
            pitch: rate(self.amplitudes[1]),
            roll: rate(self.amplitudes[2]),
            shake: self.shake,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conditions(agl: f64) -> Conditions {
        Conditions {
            agl_feet: agl,
            on_ground: false,
            speed_fps: 400.,
            seconds_of_day: 43_200,
            percent: 100,
            daytime_ground: false,
            enabled: true,
            nearby_strength: 0,
        }
    }

    fn run(c: Conditions, ticks: i64) -> (Turbulence, Vec<Disturbance>) {
        let mut state = Turbulence::default();
        let mut rng = NativeRng::seeded(1).unwrap();
        let out = (0..ticks)
            .map(|tick| state.step(tick, 2, c, &mut rng).unwrap())
            .collect();
        (state, out)
    }

    #[test]
    fn nearby_wake_has_speed_distance_dead_zone_and_rear_cone_gates() {
        let n = NearbyAircraft {
            position: [0.; 3],
            attitude: [0.; 3],
            speed_fps: 300.,
            signature: 100,
        };
        assert_eq!(n.strength_at([0., 0., -500.]), 75);
        assert_eq!(n.strength_at([0., 0., 500.]), 0);
        assert_eq!(n.strength_at([0., 0., -2000.]), 0);
        assert_eq!(
            NearbyAircraft {
                speed_fps: 292.,
                ..n
            }
            .strength_at([0., 0., -500.]),
            0
        );
        assert!(n.strength_at([25., 25., -500.]) > 0);
        assert_eq!(n.strength_at([500., 0., -500.]), 0);
        let east = NearbyAircraft {
            attitude: [std::f64::consts::FRAC_PI_2, 0., 0.],
            ..n
        };
        assert_eq!(east.strength_at([-500., 0., 0.]), 75);
        let (state, out) = run(
            Conditions {
                nearby_strength: 75,
                ..conditions(20000.)
            },
            4000,
        );
        assert!(out.iter().any(|d| *d != Disturbance::default()));
        assert_ne!(state, Turbulence::default());
    }

    #[test]
    fn strength_falls_to_nothing_by_the_recovered_ceiling() {
        assert_eq!(low_altitude_strength(0.), 100);
        assert_eq!(low_altitude_strength(500.), 50);
        assert_eq!(low_altitude_strength(999.), 1);
        assert_eq!(low_altitude_strength(0.5), 100);
        assert_eq!(low_altitude_strength(10.5), 99);
        assert_eq!(low_altitude_strength(CEILING_FEET), 0);
        assert_eq!(low_altitude_strength(40_000.), 0);
        assert_eq!(low_altitude_strength(f64::NAN), 0);
        assert_eq!(low_altitude_strength(-10.), 100);
    }

    #[test]
    fn speed_shape_matches_the_recovered_breakpoints() {
        assert_eq!(speed_shape(0), 0);
        assert_eq!(speed_shape(146), 100);
        assert_eq!(speed_shape(293), 100);
        assert_eq!(speed_shape(586), 0);
        assert_eq!(speed_shape(2000), 0);
    }

    #[test]
    fn high_altitude_and_ground_produce_no_disturbance() {
        for c in [
            conditions(5000.),
            Conditions {
                on_ground: true,
                ..conditions(10.)
            },
            Conditions {
                enabled: false,
                ..conditions(10.)
            },
        ] {
            let (_, out) = run(c, 4000);
            assert!(out.iter().all(|d| *d == Disturbance::default()));
        }
    }

    #[test]
    fn generation_uses_quotient_and_active_events_finish_before_recurrence() {
        let mut rng = NativeRng::seeded(1).unwrap();
        let mut draws = rng.clone();
        let first_draw = draws.below(0x1e00).unwrap();
        let mut state = Turbulence::default();
        assert_eq!(
            state.step(100, 2, conditions(0.), &mut rng).unwrap(),
            Disturbance::default()
        );
        assert_eq!(state.end - state.start, i64::from(first_draw / 100 + 89));
        assert!((89..=165).contains(&(state.end - state.start)));
        state.next_update = 101;
        let before = state;
        let rng_before = rng.clone();
        state.step(101, 2, conditions(0.), &mut rng).unwrap();
        assert_eq!(
            state, before,
            "an active interval wins over a due recurrence"
        );
        assert_eq!(rng, rng_before, "active events consume no new draws");
    }

    #[test]
    fn low_altitude_produces_bounded_bidirectional_events() {
        let (_, out) = run(conditions(100.), 6000);
        let active: Vec<_> = out
            .iter()
            .filter(|d| **d != Disturbance::default())
            .collect();
        assert!(!active.is_empty(), "expected turbulence near the ground");
        assert!(active.iter().any(|d| d.roll > 0.) && active.iter().any(|d| d.roll < 0.));
        for d in &active {
            // Roll is the largest axis and stays inside the recovered scale.
            assert!(d.roll.abs() <= 0.13, "roll {}", d.roll);
            assert!(d.yaw.abs() <= d.roll.abs().max(0.02));
            assert!(d.vertical_fps.abs() <= 14.1);
            assert!((0. ..=1.).contains(&d.severity()));
        }
    }

    #[test]
    fn night_and_the_daytime_ground_flag_both_reduce_strength() {
        let peak = |c| {
            run(c, 6000)
                .1
                .iter()
                .map(|d| d.roll.abs())
                .fold(0., f64::max)
        };
        let day = peak(conditions(0.));
        let night = peak(Conditions {
            seconds_of_day: 3600,
            ..conditions(0.)
        });
        let reduced = peak(Conditions {
            daytime_ground: true,
            ..conditions(0.)
        });
        assert!(night < day, "night {night} vs day {day}");
        assert!(reduced < day, "flagged {reduced} vs day {day}");
    }

    #[test]
    fn identical_seeds_replay_identically_and_state_is_per_aircraft() {
        let (a, first) = run(conditions(200.), 3000);
        let (b, second) = run(conditions(200.), 3000);
        assert_eq!(a, b);
        assert_eq!(first, second);
        // A second aircraft with its own state does not disturb the first.
        let mut shared = NativeRng::seeded(1).unwrap();
        let (mut one, mut two) = (Turbulence::default(), Turbulence::default());
        for tick in 0..3000 {
            one.step(tick, 2, conditions(200.), &mut shared).unwrap();
            two.step(tick, 2, conditions(900.), &mut shared).unwrap();
        }
        assert_ne!(one, two);
    }
}
