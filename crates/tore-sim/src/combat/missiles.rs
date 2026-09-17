//! Spec-derived missile profiles and fitted motion, docs/spec/missiles.md.
//! Independent from renderer, host clock and the explicit compatibility adapter.
use super::{EnginePhase, altitude_performance};
use crate::attitude::{Basis, Vector, dot, unit};
use tore_formats::weapons::{Movement, Weapon, Zone};

pub const HZ: u64 = 120;
pub const DT: f64 = 1. / HZ as f64;
pub const NMI: f64 = 6076.;
pub const DWELL: u32 = 30;
pub const MEMORY: u32 = 240;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rules {
    Compatibility,
    #[default]
    Spec,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LaunchMode {
    #[default]
    Cued,
    Boresight,
}
impl LaunchMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cued => "CUED",
            Self::Boresight => "BORESIGHT",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guidance {
    Supported,
    Active,
    Infrared,
    Emitter,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    pub guidance: Guidance,
    pub activation_ft: Option<f64>,
    pub guidance_ticks: u64,
    pub memory_ticks: u32,
    pub radar_emissions: bool,
    pub jammer_emissions: bool,
}
impl Profile {
    pub fn validate(self) -> tore_formats::Result<()> {
        let activation_valid = match (self.guidance, self.activation_ft) {
            (Guidance::Active, Some(value)) => value.is_finite() && value > 0.,
            (Guidance::Active, None) | (_, Some(_)) => false,
            (_, None) => true,
        };
        if !activation_valid || self.guidance_ticks == 0 || self.memory_ticks == 0 {
            return Err(super::invalid(
                "invalid missile profile timing or activation",
            ));
        }
        Ok(())
    }

    /// Explicit reviewed identities only. This does not expand store allowlists.
    pub fn for_weapon(w: &Weapon) -> Option<Self> {
        let (guidance, active) = match w.source.as_str() {
            "AIM120.JT" | "MICA.JT" | "AA12.JT" => (Guidance::Active, Some(5.)),
            "AAML.JT" | "AGM84A.JT" | "AM39.JT" => (Guidance::Active, Some(8.)),
            "AIM54C.JT" => (Guidance::Active, Some(10.)),
            "AEMP1.JT" => (Guidance::Active, Some(3.)),
            "AS16.JT" => (Guidance::Active, Some(2.)),
            "R530.JT" | "AS7.JT" => (Guidance::Supported, None),
            "AA11.JT" | "AA11B.JT" | "AA2.JT" | "AA8.JT" | "AIM9M.JT" | "AIM9X.JT" | "R550.JT"
            | "AGM65G.JT" => (Guidance::Infrared, None),
            "AGM45.JT" | "AGM88.JT" => (Guidance::Emitter, None),
            _ => return None,
        };
        Some(Self {
            guidance,
            activation_ft: active.map(|v| v * NMI),
            guidance_ticks: u64::from(w.movement.remove_t) * 30,
            memory_ticks: MEMORY,
            radar_emissions: guidance == Guidance::Emitter,
            // Fitted conservative receiver policy, no evidence for jammer homing.
            jammer_emissions: false,
        })
    }
    pub fn independent(self) -> bool {
        self.guidance != Guidance::Supported
    }
    pub fn search_cap(self) -> f64 {
        if self.guidance == Guidance::Infrared {
            3f64
        } else {
            10f64
        }
        .to_radians()
    }
}

pub fn phase(m: &Movement, age: u64) -> EnginePhase {
    if age < u64::from(m.ignite_t) * 30 {
        EnginePhase::BeforeIgnition
    } else if age < u64::from(m.fuel_t) * 30 {
        EnginePhase::Powered
    } else {
        EnginePhase::Coast
    }
}
pub fn removed(m: &Movement, age: u64) -> bool {
    age >= u64::from(m.remove_t) * 30
}

/// 0x7fff means unrestricted on that axis; other limits remain independent.
/// This fitted spherical-angle interpretation avoids tan(180) collapsing a cone.
pub fn half_angle(raw: i16) -> f64 {
    if raw == i16::MAX {
        std::f64::consts::PI
    } else {
        (f64::from(raw.max(0)) / 182.).to_radians()
    }
}
pub fn geometry(
    z: &Zone,
    position: Vector,
    basis: Basis,
    target: Vector,
    cap: Option<f64>,
) -> bool {
    let d = sub(target, position);
    let distance = length(d);
    let forward = dot(d, basis.forward);
    let heading = dot(d, basis.right).atan2(forward).abs();
    let elevation = dot(d, basis.up)
        .atan2(forward.hypot(dot(d, basis.right)))
        .abs();
    let limit = |raw| half_angle(raw).min(cap.unwrap_or(std::f64::consts::PI));
    distance >= f64::from(z.minimum_range)
        && distance <= f64::from(z.maximum_range)
        && d[1] >= f64::from(z.minimum_altitude)
        && d[1] <= f64::from(z.maximum_altitude)
        && heading <= limit(z.heading) + 1e-12
        && elevation <= limit(z.pitch) + 1e-12
}
pub fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}
pub fn length(v: Vector) -> f64 {
    dot(v, v).sqrt()
}
pub fn closure(position: Vector, velocity: Vector, target: Vector, target_velocity: Vector) -> f64 {
    dot(sub(velocity, target_velocity), unit(sub(target, position)))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub velocity: Vector,
    pub gain: f64,
    pub budget: f64,
}
impl Motion {
    pub fn new(m: &Movement, velocity: Vector, altitude: f64) -> Self {
        let maximum = altitude_performance(
            m.maximum_speed,
            m.performance_at_0,
            m.performance_at_20,
            (altitude * 256.) as i32,
        );
        Self {
            velocity,
            gain: 0.,
            budget: (f64::from(maximum) - f64::from(m.initial_speed)).max(0.),
        }
    }
    /// Steering rotates velocity, preserving speed. No change of rail direction
    /// means full inherited climb/side-slip survives the unsteered boost.
    pub fn turn(&mut self, old: Vector, new: Vector) {
        let axis = crate::attitude::cross(old, new);
        let angle = length(axis).atan2(dot(old, new));
        if angle > 1e-12 {
            let b = Basis {
                right: [1., 0., 0.],
                up: [0., 1., 0.],
                forward: [0., 0., 1.],
            }
            .rotated(unit(axis).map(|v| v * angle));
            self.velocity = std::array::from_fn(|i| {
                b.right[i] * self.velocity[0]
                    + b.up[i] * self.velocity[1]
                    + b.forward[i] * self.velocity[2]
            });
        }
    }
    pub fn step(&mut self, m: &Movement, age: u64, forward: Vector) -> Vector {
        match phase(m, age) {
            EnginePhase::Powered => {
                let gain =
                    (f64::from(m.acceleration.max(0)) * DT).min((self.budget - self.gain).max(0.));
                self.gain += gain;
                for (v, f) in self.velocity.iter_mut().zip(forward) {
                    *v += f * gain;
                }
            }
            EnginePhase::Coast => {
                let speed = length(self.velocity);
                let next = (speed - f64::from(m.deceleration.max(0)) * DT)
                    .max(f64::from(m.final_speed.max(0)).min(speed));
                if speed > 0. {
                    self.velocity = self.velocity.map(|v| v * next / speed);
                }
            }
            EnginePhase::BeforeIgnition => {}
        }
        self.velocity.map(|v| v * DT)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solution {
    pub point: Vector,
    pub seconds: f64,
}
/// Fitted bounded straight-path estimate. Uses exactly the propulsion integrator
/// used in flight, with lead from observed velocity. Turn costs remain approximate.
pub fn intercept(
    m: &Movement,
    motion: Motion,
    position: Vector,
    target: Vector,
    velocity: Vector,
    age: u64,
    lifetime: u64,
) -> Option<Solution> {
    let end = lifetime.min(u64::from(m.remove_t) * 30);
    let mut predicted = motion;
    let forward = unit(sub(target, position));
    let mut traveled = [0.; 3];
    for tick in age..end {
        let delta = predicted.step(m, tick, forward);
        for i in 0..3 {
            traveled[i] += delta[i];
        }
        let seconds = (tick - age + 1) as f64 * DT;
        let point = std::array::from_fn(|i| target[i] + velocity[i] * seconds);
        if length(traveled) >= length(sub(point, position)) {
            return Some(Solution { point, seconds });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn movement() -> Movement {
        Movement {
            minimum_speed: 0,
            corner_speed: 0,
            maximum_speed: 1000,
            acceleration: 1000,
            deceleration: 100,
            initial_speed: 0,
            final_speed: 200,
            launch_retard: 100,
            ignite_t: 4,
            fuel_t: 12,
            remove_t: 80,
            powered_turn_rate: 1000,
            unpowered_turn_rate: 1000,
            performance_at_0: 100,
            performance_at_20: 100,
            cruise: [0; 4],
            jink: [0; 3],
        }
    }
    #[test]
    fn inherited_vector_finite_boost_and_closure() {
        let m = movement();
        let mut motion = Motion::new(&m, [40., 60., 600.], 10000.);
        for age in 0..360 {
            motion.step(&m, age, [0., 0., 1.]);
        }
        assert!(length(sub(motion.velocity, [40., 60., 1600.])) < 1e-8);
        assert_eq!(motion.gain, 1000.);
        assert!(
            (closure([0.; 3], motion.velocity, [0., 0., 10000.], [0., 0., -300.]) - 1900.).abs()
                < 1e-8
        );
        assert!(
            (closure([0.; 3], motion.velocity, [0., 0., 10000.], [0., 0., 300.]) - 1300.).abs()
                < 1e-8
        );
        assert_eq!(phase(&m, 119), EnginePhase::BeforeIgnition);
        assert_eq!(phase(&m, 120), EnginePhase::Powered);
        assert_eq!(phase(&m, 360), EnginePhase::Coast);
        motion.step(&m, 360, [0., 0., 1.]);
        assert!(length(motion.velocity) < length([40., 60., 1600.]));
    }
    #[test]
    fn prediction_leads_crossing_and_bounds_impossible_shots() {
        let m = movement();
        let motion = Motion::new(&m, [0., 0., 600.], 10000.);
        let solution = intercept(
            &m,
            motion,
            [0.; 3],
            [0., 0., 3000.],
            [300., 0., 0.],
            0,
            2400,
        )
        .unwrap();
        assert!(solution.point[0] > 0.);
        assert!(
            intercept(
                &m,
                motion,
                [0.; 3],
                [0., 0., 3000.],
                [0., 0., 10000.],
                0,
                2400
            )
            .is_none()
        );
        assert!(intercept(&m, motion, [0.; 3], [0., 0., 3000.], [0.; 3], 0, 30).is_none());
        assert!(removed(&m, 65536 * 30));
        let mut early = m;
        early.remove_t = 8;
        assert!(removed(&early, 240));
        assert_eq!(phase(&early, 240), EnginePhase::Powered);
    }
    #[test]
    fn independent_angles_and_wide_sentinel() {
        let z = Zone {
            heading: 32767,
            pitch: 182 * 90,
            minimum_range: 0,
            maximum_range: 10000,
            minimum_altitude: -10000,
            maximum_altitude: 10000,
        };
        let b = Basis::new(0., 0., 0.);
        assert!(geometry(&z, [0.; 3], b, [0., 0., -100.], None));
        let point = |angle: f64| {
            [
                1000. * angle.to_radians().sin(),
                0.,
                1000. * angle.to_radians().cos(),
            ]
        };
        assert!(geometry(&z, [0.; 3], b, point(3.), Some(3f64.to_radians())));
        assert!(!geometry(
            &z,
            [0.; 3],
            b,
            point(3.001),
            Some(3f64.to_radians())
        ));
    }
}

pub mod seeker;
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    pub profile: Profile,
    pub mode: LaunchMode,
    pub seeker: seeker::Seeker,
    pub enabled: bool,
    pub last_intercept: Option<Vector>,
    pub solution: Option<Solution>,
}
impl Flight {
    pub fn new(profile: Profile, mode: LaunchMode, target: Option<u32>) -> Self {
        Self {
            profile,
            mode,
            seeker: seeker::Seeker::new(target),
            enabled: profile.guidance != Guidance::Active || mode == LaunchMode::Boresight,
            last_intercept: None,
            solution: None,
        }
    }
}
