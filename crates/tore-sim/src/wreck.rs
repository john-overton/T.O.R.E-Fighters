//! Fitted aerodynamic wreck motion and user-requested airburst timing.
//! See docs/spec/destroyed-aircraft.md. Independent of living aircraft control.
use crate::{
    attitude::{Basis, Vector, cross, dot, unit},
    flight::DT,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Falling,
    Grounded,
    Exploded,
}
/// Per-engine forward acceleration captured at destruction, in ft/s².
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Power {
    pub acceleration: [f64; 4],
    pub engine_count: u8,
    pub fuel_seconds: f64,
}
impl Power {
    pub fn symmetric(engine_count: u8, acceleration: f64, fuel_seconds: f64) -> Self {
        let engine_count = engine_count.clamp(1, 4);
        Self {
            acceleration: std::array::from_fn(|i| {
                if i < usize::from(engine_count) {
                    acceleration / f64::from(engine_count)
                } else {
                    0.
                }
            }),
            engine_count,
            fuel_seconds,
        }
    }
    pub fn total(self) -> f64 {
        if self.fuel_seconds > 0. {
            self.acceleration.iter().sum()
        } else {
            0.
        }
    }
    fn yaw_torque(self) -> f64 {
        if self.fuel_seconds <= 0. || self.engine_count <= 1 {
            return 0.;
        }
        self.acceleration
            .iter()
            .enumerate()
            .take(usize::from(self.engine_count))
            .map(|(i, a)| -(2. * i as f64 / f64::from(self.engine_count - 1) - 1.) * a * 0.03)
            .sum()
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Wreck {
    pub phase: Phase,
    pub ticks: u64,
    pub polls: u32,
    pub angular_rates: Vector,
    pub power: Power,
    bias: Vector,
    explosion_rng: u32,
}
fn next(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}
fn mix(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    (value ^ (value >> 16)).max(1)
}
pub fn poll_due(ticks: u64) -> bool {
    (120..=600).contains(&ticks) && ticks.is_multiple_of(120)
        || ticks > 600 && ticks.is_multiple_of(600)
}
impl Wreck {
    pub fn new(id: u32, destruction_tick: u64, inherited_rates: Vector) -> Self {
        let seed =
            mix(id ^ destruction_tick as u32 ^ (destruction_tick >> 32) as u32 ^ 0x5752_4543);
        let mut motion_rng = mix(seed ^ 0x4d4f_544e);
        let bias = std::array::from_fn(|_| f64::from(next(&mut motion_rng) % 2001) / 1000. - 1.);
        Self {
            phase: Phase::Falling,
            power: Power::default(),
            ticks: 0,
            polls: 0,
            angular_rates: std::array::from_fn(|i| {
                (inherited_rates[i] + bias[i] * 0.6).clamp(-3., 3.)
            }),
            bias,
            explosion_rng: mix(seed ^ 0x424f_4f4d),
        }
    }
    /// A transition is returned once only. Terrain is checked before the airburst roll.
    pub fn step(
        &mut self,
        position: &mut Vector,
        velocity: &mut Vector,
        basis: &mut Basis,
        ground: impl Fn(f64, f64) -> f64,
    ) -> Option<Phase> {
        if self.phase != Phase::Falling {
            return None;
        }
        self.ticks += 1;
        let speed = dot(*velocity, *velocity).sqrt();
        let axes = [basis.right, basis.up, basis.forward];
        let airflow = (speed / 400.).clamp(0., 2.);
        let align = cross(basis.forward, unit(*velocity));
        let rotation: Vector = std::array::from_fn(|i| {
            let torque = self.bias[i] * 0.9 * airflow
                + dot(align, axes[i]) * airflow
                + if i == 1 { self.power.yaw_torque() } else { 0. };
            self.angular_rates[i] = (self.angular_rates[i]
                + (torque - self.angular_rates[i] * 0.6) * DT)
                .clamp(-3., 3.);
            self.angular_rates[i] * DT
        });
        *basis = basis.rotated(std::array::from_fn(|i| {
            (0..3).map(|axis| axes[axis][i] * rotation[axis]).sum()
        }));
        let original_velocity = *velocity;
        for (axis, coefficient) in axes.into_iter().zip([0.0018, 0.0024, 0.00012]) {
            let component = dot(original_velocity, axis);
            let decrement = (coefficient * component.abs() * DT).min(0.5) * component;
            for i in 0..3 {
                velocity[i] -= axis[i] * decrement;
            }
        }
        for (i, v) in velocity.iter_mut().enumerate() {
            *v += basis.forward[i] * self.power.total() * DT;
        }
        self.power.fuel_seconds = (self.power.fuel_seconds - DT).max(0.);
        velocity[1] -= 32.174 * DT;
        for i in 0..3 {
            position[i] += velocity[i] * DT;
        }
        let height = ground(position[0], position[2]);
        if position[1] <= height {
            position[1] = height;
            *velocity = [0.; 3];
            self.angular_rates = [0.; 3];
            self.phase = Phase::Grounded;
            return Some(self.phase);
        }
        if poll_due(self.ticks) {
            self.polls += 1;
            if next(&mut self.explosion_rng) % 100 < 5 {
                self.phase = Phase::Exploded;
                *velocity = [0.; 3];
                return Some(self.phase);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn surviving_engines_push_with_the_body_and_asymmetric_power_adds_yaw() {
        let mut powered = Wreck::new(8, 0, [0.; 3]);
        powered.bias = [0.; 3];
        powered.angular_rates = [0.; 3];
        powered.power = Power::symmetric(2, 30., 10.);
        let mut unpowered = powered.clone();
        unpowered.power = Power::default();
        let mut one = powered.clone();
        one.power.acceleration[0] = 0.;
        let initial = (
            [0., 10000., 0.],
            [0.; 3],
            Basis::new(std::f64::consts::FRAC_PI_2, 0., 0.),
        );
        let (mut pp, mut pv, mut pb) = initial;
        let (mut up, mut uv, mut ub) = initial;
        let (mut op, mut ov, mut ob) = initial;
        for _ in 0..119 {
            powered.step(&mut pp, &mut pv, &mut pb, |_, _| 0.);
            unpowered.step(&mut up, &mut uv, &mut ub, |_, _| 0.);
            one.step(&mut op, &mut ov, &mut ob, |_, _| 0.);
        }
        assert!(pv[0] > uv[0] + 10.);
        assert!(pv[0] > pv[2].abs());
        assert!(one.angular_rates[1].abs() > powered.angular_rates[1].abs() + 0.1);
        assert!(one.power.fuel_seconds < 10.);
        one.power.fuel_seconds = 0.;
        assert_eq!(one.power.total(), 0.);
    }
    #[test]
    fn five_percent_boundary_and_independent_motion_randomness() {
        for roll in [4, 5] {
            let seed = (1..10000)
                .find(|seed| {
                    let mut s = *seed;
                    next(&mut s) % 100 == roll
                })
                .unwrap();
            let mut w = Wreck::new(4, 6, [0.; 3]);
            w.explosion_rng = seed;
            w.ticks = 119;
            let (mut p, mut v, mut b) = ([0., 10000., 0.], [0.; 3], Basis::new(0., 0., 0.));
            let outcome = w.step(&mut p, &mut v, &mut b, |_, _| 0.);
            assert_eq!(
                outcome,
                if roll == 4 {
                    Some(Phase::Exploded)
                } else {
                    None
                }
            );
            assert_eq!(w.polls, 1);
        }
        let a = Wreck::new(9, 10, [0.; 3]);
        let b = Wreck::new(9, 10, [2.; 3]);
        assert_eq!(a.explosion_rng, b.explosion_rng);
    }
    #[test]
    fn exact_poll_schedule_has_no_zero_or_six_second_roll() {
        let due: Vec<_> = (0..=2400).filter(|tick| poll_due(*tick)).collect();
        assert_eq!(due, [120, 240, 360, 480, 600, 1200, 1800, 2400]);
    }
    #[test]
    fn tumble_changes_attitude_and_loses_speed_without_render_side_effects() {
        let mut w = Wreck::new(1, 42, [0.; 3]);
        let mut position = [0., 10000., 0.];
        let mut velocity = [0., 0., 700.];
        let mut basis = Basis::new(0., 0., 0.);
        let mut replica = (w.clone(), position, velocity, basis);
        for _ in 0..119 {
            w.step(&mut position, &mut velocity, &mut basis, |_, _| 0.);
            replica
                .0
                .step(&mut replica.1, &mut replica.2, &mut replica.3, |_, _| 0.);
        }
        assert_eq!((w.clone(), position, velocity, basis), replica);
        assert_eq!(w.polls, 0);
        assert!(position[1] < 10000.);
        assert!(velocity[2] < 700.);
        assert_ne!(basis, Basis::new(0., 0., 0.));
        assert!(dot(basis.up, basis.right).abs() < 1e-10);
    }
    #[test]
    fn airburst_and_ground_contact_are_one_shot_and_ground_wins() {
        let seed = (1..10000)
            .find(|seed| {
                let mut s = *seed;
                next(&mut s) % 100 < 5
            })
            .unwrap();
        let mut w = Wreck::new(2, 9, [0.; 3]);
        w.explosion_rng = seed;
        w.ticks = 119;
        let (mut p, mut v, mut b) = ([0., 10000., 0.], [0.; 3], Basis::new(0., 0., 0.));
        assert_eq!(
            w.step(&mut p, &mut v, &mut b, |_, _| 0.),
            Some(Phase::Exploded)
        );
        assert_eq!(w.polls, 1);
        assert_eq!(w.step(&mut p, &mut v, &mut b, |_, _| 0.), None);
        let mut grounded = Wreck::new(2, 9, [0.; 3]);
        grounded.explosion_rng = seed;
        grounded.ticks = 119;
        p[1] = 0.;
        assert_eq!(
            grounded.step(&mut p, &mut v, &mut b, |_, _| 0.),
            Some(Phase::Grounded)
        );
        assert_eq!(grounded.polls, 0);
        assert_eq!(grounded.step(&mut p, &mut v, &mut b, |_, _| 0.), None);
    }
}
