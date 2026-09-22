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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetRole {
    Aircraft,
    Surface,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    pub role: TargetRole,
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
        let role = match w.source.as_str() {
            "AGM65G.JT" | "AGM45.JT" | "AGM88.JT" | "AGM84A.JT" | "AM39.JT" | "AS16.JT"
            | "AS7.JT" => TargetRole::Surface,
            _ => TargetRole::Aircraft,
        };
        Some(Self {
            role,
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
    pub fn guidance_available(self, radar_power: bool) -> bool {
        radar_power || self.guidance == Guidance::Infrared
    }
    pub fn supports_boresight(self) -> bool {
        self.role == TargetRole::Aircraft && self.independent()
    }
    pub fn accepts(self, target: &super::live::Target) -> bool {
        self.role == target.role && (self.role == TargetRole::Surface || target.airborne)
    }
    pub fn search_cap(self) -> f64 {
        5f64.to_radians()
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
    let inside_circle =
        cap.is_none_or(|angle| distance > 0. && forward / distance >= angle.cos() - 1e-12);
    inside_circle
        && distance >= f64::from(z.minimum_range)
        && distance <= f64::from(z.maximum_range)
        && d[1] >= f64::from(z.minimum_altitude)
        && d[1] <= f64::from(z.maximum_altitude)
        && heading <= limit(z.heading) + 1e-12
        && elevation <= limit(z.pitch) + 1e-12
}

/// Fitted active missile radar response using the shared Advanced notch preset:
/// 60 ft/s half width and 0.45 centre range factor in full ground clutter.
/// The continuous range penalty feeds normal seeker memory, so entering the
/// notch does not immediately destroy or permanently defeat the missile.
pub fn active_radar_visible(
    w: &Weapon,
    position: Vector,
    basis: Basis,
    target: &super::live::Target,
    target_height_agl_ft: f64,
) -> bool {
    let nominal = f64::from(w.seeker.zones[0].maximum_range);
    let sighting = crate::sensors::detection::Sighting::new(position, &basis, target.position);
    let signature = target.signature.effective_radar(
        &target.basis,
        sub(position, target.position),
        target.configuration,
    );
    let clutter =
        crate::sensors::detection::clutter_exposure(&sighting, target_height_agl_ft.max(0.));
    let notch = crate::sensors::detection::notch_factor(
        &crate::sensors::Preset::Advanced.notch(),
        clutter,
        crate::sensors::detection::notch_speed_fps(&sighting, target.velocity),
    );
    let range =
        crate::sensors::detection::effective_range_ft(nominal, signature / 100., 1., notch, 1.);
    signature > 0. && sighting.distance_ft <= range
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
    /// Steering rotates inherited velocity and applies fitted maneuver loss. No change of rail direction
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
            let loss = (-0.03 * angle * angle / DT).exp();
            self.velocity = std::array::from_fn(|i| {
                (b.right[i] * self.velocity[0]
                    + b.up[i] * self.velocity[1]
                    + b.forward[i] * self.velocity[2])
                    * loss
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
/// Opinionated HUD estimate, not a calibrated probability or guidance permission.
pub fn estimated_hit_percent(
    observation: seeker::Observation,
    solution: Option<Solution>,
    zone: &Zone,
    lifetime_seconds: f64,
    bore_cap: Option<f64>,
) -> u8 {
    let Some(solution) = solution else {
        return 0;
    };
    let min = f64::from(zone.minimum_range);
    let max = f64::from(zone.maximum_range);
    if max <= min || !(min..=max).contains(&observation.range) || lifetime_seconds <= 0. {
        return 0;
    }
    let r = ((observation.range - min) / (max - min)).clamp(0., 1.);
    let centring = bore_cap.map_or(1., |cap| seeker::centre_weight(observation.off_axis, cap));
    let margin = (1. - 0.6 * solution.seconds / lifetime_seconds).clamp(0., 1.);
    (95. * observation.quality.clamp(0., 1.) * centring * (1. - 0.75 * r * r) * margin)
        .round()
        .clamp(0., 95.) as u8
}

/// Constant-speed lead from observed motion only. No target state lookup.
pub fn lead(position: Vector, speed: f64, target: Vector, velocity: Vector) -> Solution {
    let r = sub(target, position);
    let a = dot(velocity, velocity) - speed * speed;
    let b = 2. * dot(r, velocity);
    let c = dot(r, r);
    let mut seconds = 0.;
    if a.abs() < 1e-9 {
        if b < -1e-9 {
            seconds = -c / b;
        }
    } else {
        let discriminant = b * b - 4. * a * c;
        if discriminant >= 0. {
            seconds = [
                (-b - discriminant.sqrt()) / (2. * a),
                (-b + discriminant.sqrt()) / (2. * a),
            ]
            .into_iter()
            .filter(|t| *t >= 0. && t.is_finite())
            .min_by(f64::total_cmp)
            .unwrap_or(0.);
        }
    }
    Solution {
        point: std::array::from_fn(|i| target[i] + velocity[i] * seconds),
        seconds,
    }
}

/// Correct inherited slip/climb using flight-path error, not nose error alone.
pub fn commanded_heading(forward: Vector, velocity: Vector, desired: Vector) -> Vector {
    if length(velocity) < 1. {
        return desired;
    }
    let flight_path = unit(velocity);
    unit(std::array::from_fn(|i| {
        forward[i] + desired[i] - flight_path[i]
    }))
}

/// Same exact angular limit in prediction and live guidance.
pub fn steer(m: &Movement, age: u64, forward: Vector, desired: Vector) -> Vector {
    let angle = dot(forward, desired).clamp(-1., 1.).acos();
    let rate = if phase(m, age) == EnginePhase::Powered {
        m.powered_turn_rate
    } else {
        m.unpowered_turn_rate
    };
    let step = (f64::from(rate.max(0)) * std::f64::consts::TAU / 65520. * DT).min(angle);
    if angle < 1e-12 || step <= 0. {
        return forward;
    }
    let mut axis = crate::attitude::cross(forward, desired);
    if length(axis) < 1e-9 {
        axis = crate::attitude::cross(
            forward,
            if forward[1].abs() < 0.9 {
                [0., 1., 0.]
            } else {
                [1., 0., 0.]
            },
        );
    }
    let axis = unit(axis);
    let tangent = crate::attitude::cross(axis, forward);
    unit(std::array::from_fn(|i| {
        forward[i] * step.cos() + tangent[i] * step.sin()
    }))
}

/// Fitted 120 Hz flyout with observed constant-velocity target and shared physics.
#[allow(clippy::too_many_arguments)]
pub fn intercept(
    m: &Movement,
    mut motion: Motion,
    mut position: Vector,
    mut forward: Vector,
    mut target: Vector,
    velocity: Vector,
    age: u64,
    lifetime: u64,
) -> Option<Solution> {
    let end = lifetime.min(u64::from(m.remove_t) * 30);
    let mut aim = target;
    for tick in age..end {
        if tick == age || tick.is_multiple_of(12) {
            aim = lead(position, length(motion.velocity), target, velocity).point;
        }
        let previous = sub(target, position);
        let desired = commanded_heading(forward, motion.velocity, unit(sub(aim, position)));
        let next = steer(m, tick, forward, desired);
        motion.turn(forward, next);
        forward = next;
        let delta = motion.step(m, tick, forward);
        for i in 0..3 {
            position[i] += delta[i];
            target[i] += velocity[i] * DT;
        }
        let relative = sub(target, position);
        let segment = sub(relative, previous);
        let u = (-dot(previous, segment) / dot(segment, segment).max(1e-12)).clamp(0., 1.);
        let closest = std::array::from_fn(|i| previous[i] + segment[i] * u);
        if length(closest) <= 25. {
            return Some(Solution {
                point: target,
                seconds: (tick - age + 1) as f64 * DT,
            });
        }
    }
    None
}

/// Imported minimum/angle/altitude limits, without the obsolete fixed launch max.
pub fn launch_geometry(w: &Weapon) -> Zone {
    Zone {
        maximum_range: i32::MAX,
        ..w.seeker.zones[1]
    }
}

/// Max useful launch distance bounded by available motion/time, not nominal max.
#[allow(clippy::too_many_arguments)]
pub fn maximum_range(
    w: &Weapon,
    position: Vector,
    forward: Vector,
    velocity: Vector,
    target: Vector,
    target_velocity: Vector,
    lifetime: u64,
) -> f64 {
    let minimum = f64::from(w.seeker.zones[1].minimum_range.max(0));
    let motion = Motion::new(&w.movement, velocity, position[1]);
    let seconds = lifetime.min(u64::from(w.movement.remove_t) * 30) as f64 * DT;
    // A conservative search ceiling: both objects' maximum possible travel.
    // It is only a bound for the solve, never the displayed range itself.
    let travel_bound = (length(velocity) + motion.budget + length(target_velocity)) * seconds + 25.;
    let cap = if Profile::for_weapon(w).is_some_and(|p| p.guidance == Guidance::Active) {
        travel_bound
    } else {
        // IR/emitter/supported seekers must measure before independent steering.
        travel_bound.min(f64::from(w.seeker.zones[0].maximum_range))
    };
    if cap <= minimum {
        return 0.;
    }
    let bearing = unit(sub(target, position));
    let reaches = |range| {
        intercept(
            &w.movement,
            Motion::new(&w.movement, velocity, position[1]),
            position,
            forward,
            std::array::from_fn(|i| position[i] + bearing[i] * range),
            target_velocity,
            0,
            lifetime,
        )
        .is_some()
    };
    if reaches(cap) {
        return cap;
    }
    let step = (cap - minimum) / 16.;
    for sample in (0..16).rev() {
        let mut low = minimum + f64::from(sample) * step;
        if !reaches(low) {
            continue;
        }
        let mut high = low + step;
        for _ in 0..10 {
            let middle = (low + high) * 0.5;
            if reaches(middle) {
                low = middle;
            } else {
                high = middle;
            }
        }
        return low;
    }
    0.
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiringBand {
    pub minimum: f64,
    pub maximum: f64,
}
/// Fitted favorable interval, not a guaranteed kill or calibrated no-escape zone.
#[allow(clippy::too_many_arguments)]
pub fn firing_band(
    w: &Weapon,
    position: Vector,
    basis: Basis,
    velocity: Vector,
    observation: seeker::Observation,
    maximum: f64,
    lifetime: u64,
    bore: bool,
) -> Option<FiringBand> {
    let minimum = f64::from(w.seeker.zones[1].minimum_range.max(0));
    if maximum <= minimum {
        return None;
    }
    let profile = Profile::for_weapon(w)?;
    let lower = minimum + 0.1 * (maximum - minimum);
    let step = (maximum - lower) / 16.;
    let bearing = unit(sub(observation.position, position));
    let mut zone = w.seeker.zones[1];
    zone.maximum_range = maximum.floor() as _;
    let mut start = None;
    let mut best: Option<FiringBand> = None;
    for sample in 0..=16 {
        let range = lower + f64::from(sample) * step;
        let target = std::array::from_fn(|i| position[i] + bearing[i] * range);
        let solution = if geometry(&launch_geometry(w), position, basis, target, None) {
            intercept(
                &w.movement,
                Motion::new(&w.movement, velocity, position[1]),
                position,
                basis.forward,
                target,
                observation.velocity,
                0,
                lifetime,
            )
        } else {
            None
        };
        let score = estimated_hit_percent(
            seeker::Observation {
                position: target,
                range,
                ..observation
            },
            solution,
            &zone,
            lifetime.min(u64::from(w.movement.remove_t) * 30) as f64 * DT,
            bore.then(|| profile.search_cap()),
        );
        if score >= 70 {
            let first = *start.get_or_insert(range);
            if range > first && best.is_none_or(|b| range - first > b.maximum - b.minimum) {
                best = Some(FiringBand {
                    minimum: first,
                    maximum: range,
                });
            }
        } else {
            start = None;
        }
    }
    best
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
        let mut m = movement();
        // Crossing interception now needs enough actual turn authority.
        m.powered_turn_rate = 6000;
        m.unpowered_turn_rate = 6000;
        let motion = Motion::new(&m, [0., 0., 600.], 10000.);
        let solution = intercept(
            &m,
            motion,
            [0.; 3],
            [0., 0., 1.],
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
                [0., 0., 1.],
                [0., 0., 3000.],
                [0., 0., 10000.],
                0,
                2400
            )
            .is_none()
        );
        assert!(
            intercept(
                &m,
                motion,
                [0.; 3],
                [0., 0., 1.],
                [0., 0., 3000.],
                [0.; 3],
                0,
                30
            )
            .is_none()
        );
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
    pub unguided: bool,
    pub launch_origin: Vector,
    /// Qualification survives terminal closure; minR is not a retention range.
    pub qualified_target: Option<u32>,
    pub profile: Profile,
    pub mode: LaunchMode,
    pub seeker: seeker::Seeker,
    pub enabled: bool,
    pub last_intercept: Option<Vector>,
    pub solution: Option<Solution>,
}
impl Flight {
    pub fn eligible(&self, w: &Weapon, target: &super::live::Target) -> bool {
        self.profile.accepts(target)
            && (self.qualified_target == Some(target.id)
                || length(sub(target.position, self.launch_origin))
                    >= f64::from(w.seeker.zones[1].minimum_range.max(0)))
    }

    pub fn new(
        profile: Profile,
        mode: LaunchMode,
        target: Option<u32>,
        launch_origin: Vector,
    ) -> Self {
        Self {
            unguided: false,
            launch_origin,
            qualified_target: None,
            profile,
            mode,
            seeker: seeker::Seeker::new(target),
            enabled: profile.guidance != Guidance::Active || mode == LaunchMode::Boresight,
            last_intercept: None,
            solution: None,
        }
    }

    /// Seed a cued release exclusively from the firing actor's observation.
    /// Later support may refresh this intercept, but hidden target state never
    /// enters the midcourse solution.
    pub fn from_supported_launch(
        profile: Profile,
        mode: LaunchMode,
        observation: seeker::Observation,
        launch_origin: Vector,
    ) -> Self {
        let mut flight = Self::new(profile, mode, Some(observation.id), launch_origin);
        flight.qualified_target = Some(observation.id);
        flight.last_intercept = Some(observation.position);
        flight.seeker.target = Some(observation.id);
        flight
    }
}
