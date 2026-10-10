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
/// The surface-to-air records surface units fire that fly supported by their
/// launcher's radar (docs/spec/surface-defenses.md, "SAM missiles"): radar
/// records (seeker 3) with the support flag 0x200, as the missile inventory
/// proposes, plus SA-19 and SA-N-11, which lack the flag but arm the HAWK,
/// Roland, 2S6 and Kirov (default, pending John). Target role aircraft.
/// ASROC stays held and the SS-N-9 is anti-ship: neither has a profile.
pub const SURFACE_SUPPORTED: [&str; 13] = [
    "SA2A.JT",
    "SA3.JT",
    "SA6.JT",
    "SA15.JT",
    "SA19.JT",
    "R440.JT",
    "MIS.JT",
    "SAN3.JT",
    "SAN4.JT",
    "SAN7.JT",
    "SAN9.JT",
    "SAN11.JT",
    "SEA_SPAR.JT",
];
/// The surface-to-air records with infrared seekers (seeker 2), which home on
/// their own: shoulder-launched and vehicle SAMs.
pub const SURFACE_INFRARED: [&str; 6] = [
    "FIM92.JT", "SA7.JT", "SA9.JT", "SA13.JT", "SA14.JT", "SA16.JT",
];

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

    /// Guidance and active-seeker range, in nautical miles, of each reviewed
    /// identity, by weapon record name.
    fn reviewed_guidance(source: &str) -> Option<(Guidance, Option<f64>)> {
        if SURFACE_SUPPORTED.contains(&source) {
            return Some((Guidance::Supported, None));
        }
        if SURFACE_INFRARED.contains(&source) {
            return Some((Guidance::Infrared, None));
        }
        Some(match source {
            "AIM120.JT" | "MICA.JT" | "AA12.JT" => (Guidance::Active, Some(5.)),
            "AAML.JT" | "AGM84A.JT" | "AM39.JT" => (Guidance::Active, Some(8.)),
            "AIM54C.JT" => (Guidance::Active, Some(10.)),
            "AEMP1.JT" => (Guidance::Active, Some(3.)),
            "AS16.JT" => (Guidance::Active, Some(2.)),
            "R530.JT" | "AS7.JT" | "AA10.JT" | "AIM7.JT" | "AIM7E.JT" => {
                (Guidance::Supported, None)
            }
            "AA11.JT" | "AA11B.JT" | "AA2.JT" | "AA8.JT" | "AIM9M.JT" | "AIM9X.JT" | "R550.JT"
            | "AGM65G.JT" | "AIM9B.JT" => (Guidance::Infrared, None),
            "AGM45.JT" | "AGM88.JT" => (Guidance::Emitter, None),
            _ => return None,
        })
    }

    /// Whether the weapon record named `source` has a reviewed profile, as
    /// [`Profile::for_weapon`] decides: for presentation that knows a
    /// weapon only by name, such as a mission replay's sound.
    pub fn reviewed(source: &str) -> bool {
        Self::reviewed_guidance(source).is_some()
    }

    /// Explicit reviewed identities only. This does not expand store allowlists.
    pub fn for_weapon(w: &Weapon) -> Option<Self> {
        let (guidance, active) = Self::reviewed_guidance(&w.source)?;
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

/// Record flag: the weapon sags under gravity while its motor is unlit.
pub const SAG_FLAG: u32 = 0x4;
/// Record flag: the release ejects the weapon downward.
pub const EJECT_FLAG: u32 = 0x8;
/// Record flag: the weapon flies the record's cruise altitude profile.
pub const CRUISE_FLAG: u32 = 0x20;
/// Sag acceleration and cap, and the ejection kick, feet per second.
pub const SAG_FPS2: f64 = 32.;
pub const SAG_MAX_FPS: f64 = 80.;
pub const EJECT_FPS: f64 = 32.;
/// Record cruise distances and altitudes count 256-foot steps.
pub const CRUISE_UNIT_FT: f64 = 256.;
/// Fitted: an air-to-air lob climbs toward its cruise altitude over this
/// much horizontal distance, 2 nmi, instead of pointing straight at it.
pub const LOB_LOOKAHEAD_FT: f64 = 2. * NMI;

/// The record's cruise altitude profile: two distance stages, each holding
/// an altitude above the target, then direct homing. docs/spec/missiles.md#drop-launch-sag-and-cruise-profile
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cruise {
    /// Each stage's horizontal distance from the target and altitude above
    /// it, feet, outer stage first. Inside the inner distance it homes direct.
    pub stages: [(f64, f64); 2],
    /// Air-to-air lob: climb to the cruise altitude rather than aim at it.
    pub lob: bool,
}
impl Cruise {
    pub fn for_weapon(w: &Weapon) -> Option<Self> {
        let [d1, a1, d2, a2] = w.movement.cruise.map(|v| f64::from(v) * CRUISE_UNIT_FT);
        (w.flags & CRUISE_FLAG != 0 && d1.max(d2) > 0.).then(|| Self {
            stages: [(d1, a1), (d2, a2)],
            lob: Profile::for_weapon(w).is_some_and(|p| p.role == TargetRole::Aircraft),
        })
    }
    /// Where a missile at `position` steers for `intercept`.
    pub fn aim(self, position: Vector, intercept: Vector) -> Vector {
        let horizontal = [intercept[0] - position[0], 0., intercept[2] - position[2]];
        let distance = length(horizontal);
        let [(outer, outer_above), (inner, inner_above)] = self.stages;
        if distance < inner {
            return intercept;
        }
        let above = if distance < outer {
            inner_above
        } else {
            outer_above
        };
        let altitude = intercept[1] + above;
        if !self.lob || distance < 1. {
            return [intercept[0], altitude, intercept[2]];
        }
        let reach = distance.min(LOB_LOOKAHEAD_FT) / distance;
        [
            position[0] + horizontal[0] * reach,
            altitude,
            position[2] + horizontal[2] * reach,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub velocity: Vector,
    pub gain: f64,
    pub budget: f64,
    /// Downward sag while the motor is unlit, feet per second, applied to
    /// position apart from the flight path. Lighting the motor clears it.
    pub sink: f64,
    pub sags: bool,
    pub cruise: Option<Cruise>,
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
            sink: 0.,
            sags: false,
            cruise: None,
        }
    }
    /// A release of `w`: the record's ejection, sag and cruise profile.
    pub fn launch(w: &Weapon, velocity: Vector, altitude: f64) -> Self {
        Self {
            sink: if w.flags & EJECT_FLAG != 0 {
                EJECT_FPS
            } else {
                0.
            },
            sags: w.flags & SAG_FLAG != 0,
            cruise: Cruise::for_weapon(w),
            ..Self::new(&w.movement, velocity, altitude)
        }
    }
    /// Where the missile steers for `intercept`.
    pub fn aim(&self, position: Vector, intercept: Vector) -> Vector {
        self.cruise
            .map_or(intercept, |c| c.aim(position, intercept))
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
        let mut delta = self.velocity.map(|v| v * DT);
        if phase(m, age) == EnginePhase::Powered {
            self.sink = 0.;
        } else if self.sags {
            self.sink = (self.sink + SAG_FPS2 * DT).min(SAG_MAX_FPS);
        }
        delta[1] -= self.sink * DT;
        delta
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
    // Fins are locked until the motor lights.
    let rate = match phase(m, age) {
        EnginePhase::BeforeIgnition => 0,
        EnginePhase::Powered => m.powered_turn_rate,
        EnginePhase::Coast => m.unpowered_turn_rate,
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

/// How often, in ticks, the coasting fly-out of [`intercept`] checks whether
/// the target has fallen out of reach.
const COAST_CHECK_TICKS: u64 = 16;

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
        // Once the motor is out the missile only slows, so the most the gap
        // can still close is fixed by its speed now: a target farther off than
        // that, plus the 25 feet that count as a hit, is never reached, and
        // the rest of the fly-out could only say no. See `unreachable`.
        if (tick - age).is_multiple_of(COAST_CHECK_TICKS)
            && phase(m, tick) == EnginePhase::Coast
            && unreachable(
                length(previous),
                coasting_reach(&motion, length(velocity), end - tick),
            )
        {
            #[cfg(test)]
            note_flown(tick - age);
            return None;
        }
        let desired = commanded_heading(
            forward,
            motion.velocity,
            unit(sub(motion.aim(position, aim), position)),
        );
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
            #[cfg(test)]
            note_flown(tick - age + 1);
            return Some(Solution {
                point: target,
                seconds: (tick - age + 1) as f64 * DT,
            });
        }
    }
    #[cfg(test)]
    note_flown(end.saturating_sub(age));
    None
}

/// The most a coasting missile and its target can still close the gap
/// between them over `ticks` more ticks of fly-out (feet). The motor is out,
/// so the missile's speed never rises (a turn only costs speed, the slowdown
/// only takes it) and its sag never exceeds the largest the fly-out allows;
/// the target keeps its speed.
fn coasting_reach(motion: &Motion, target_speed: f64, ticks: u64) -> f64 {
    let sink = if motion.sags {
        motion.sink.max(SAG_MAX_FPS)
    } else {
        motion.sink
    };
    (length(motion.velocity) + sink + target_speed) * ticks as f64 * DT
}

#[cfg(test)]
thread_local! {
    static FLOWN_TICKS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
fn note_flown(ticks: u64) {
    FLOWN_TICKS.with(|flown| flown.set(flown.get() + ticks));
}
/// The ticks of missile fly-out [`intercept`] has run on this thread so far:
/// a count of the work the firing estimates cost, for tests to bound, since
/// the time it takes varies with the machine. Compiled for tests only.
#[cfg(test)]
pub fn flown_ticks() -> u64 {
    FLOWN_TICKS.with(|flown| flown.get())
}

/// Imported minimum/angle/altitude limits, without the obsolete fixed launch max.
pub fn launch_geometry(w: &Weapon) -> Zone {
    Zone {
        maximum_range: i32::MAX,
        ..w.seeker.zones[1]
    }
}

/// How many ticks a fly-out of `intercept` runs at most: the guidance time or
/// the weapon's removal time, whichever is first.
fn lifetime_ticks(m: &Movement, lifetime: u64) -> u64 {
    lifetime.min(u64::from(m.remove_t) * 30)
}

/// The farthest a missile launched with `motion` can possibly fly in a whole
/// fly-out of `intercept` (feet), however it steers. Each tick's speed is
/// bounded by the speed the motor could have reached with every bit of thrust
/// spent in the line (steering only loses speed), the coast slowdown applied
/// as it is flown, and the largest downward sag or ejection speed on top.
fn travel_ceiling(m: &Movement, motion: &Motion, lifetime: u64) -> f64 {
    let sink = SAG_MAX_FPS.max(EJECT_FPS).max(motion.sink);
    let mut speed = length(motion.velocity);
    let mut gain = motion.gain;
    let mut total = 0.;
    for tick in 0..lifetime_ticks(m, lifetime) {
        match phase(m, tick) {
            EnginePhase::Powered => {
                let step =
                    (f64::from(m.acceleration.max(0)) * DT).min((motion.budget - gain).max(0.));
                gain += step;
                speed += step;
            }
            EnginePhase::Coast => {
                speed = (speed - f64::from(m.deceleration.max(0)) * DT)
                    .max(f64::from(m.final_speed.max(0)).min(speed));
            }
            EnginePhase::BeforeIgnition => {}
        }
        total += (speed + sink) * DT;
    }
    total
}

/// Whether a target `range` feet away is provably out of the reach of a
/// fly-out that covers at most `travel` feet between the missile and the
/// target together: `intercept` only succeeds once the closest approach is
/// within 25 feet, and each tick closes the gap by no more than the two
/// objects' own movement. The margin covers rounding in the fly-out.
fn unreachable(range: f64, travel: f64) -> bool {
    range > 25. + travel + 1e-6 * (range + travel) + 1e-3
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
    let motion = Motion::launch(w, velocity, position[1]);
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
    // A target the missile cannot cover in its whole flight needs no fly-out:
    // `intercept` could only run to the end and say no. See `unreachable`.
    let ceiling = travel_ceiling(
        &w.movement,
        &Motion::launch(w, velocity, position[1]),
        lifetime,
    );
    let travel =
        ceiling + length(target_velocity) * lifetime_ticks(&w.movement, lifetime) as f64 * DT;
    let reaches = |range: f64| {
        if unreachable(range, travel) {
            return false;
        }
        intercept(
            &w.movement,
            Motion::launch(w, velocity, position[1]),
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
    let bore_cap = bore.then(|| profile.search_cap());
    let life = lifetime_ticks(&w.movement, lifetime);
    let life_seconds = life as f64 * DT;
    for sample in 0..=16 {
        let range = lower + f64::from(sample) * step;
        let target = std::array::from_fn(|i| position[i] + bearing[i] * range);
        let observed = seeker::Observation {
            position: target,
            range,
            ..observation
        };
        let score_of =
            |solution| estimated_hit_percent(observed, solution, &zone, life_seconds, bore_cap);
        // Only a score of 70 or more counts, and a later interception scores
        // no higher than an earlier one. So the fly-out need only run as long
        // as an interception can still score 70, which is exact: a shot that
        // would hit later scores below 70 either way.
        let late = |ticks: u64| {
            score_of(Some(Solution {
                point: target,
                seconds: ticks as f64 * DT,
            }))
        };
        let horizon = if life == 0 || late(1) < 70 {
            0
        } else {
            let (mut good, mut bad) = (1, life + 1);
            while bad - good > 1 {
                let middle = good + (bad - good) / 2;
                if late(middle) >= 70 {
                    good = middle;
                } else {
                    bad = middle;
                }
            }
            good
        };
        let solution =
            if horizon > 0 && geometry(&launch_geometry(w), position, basis, target, None) {
                intercept(
                    &w.movement,
                    Motion::launch(w, velocity, position[1]),
                    position,
                    basis.forward,
                    target,
                    observation.velocity,
                    0,
                    horizon,
                )
            } else {
                None
            };
        let score = score_of(solution);
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
    /// The AIM54C.JT motor and turn numbers, with the retail release flags.
    fn phoenix() -> (Movement, Motion) {
        let m = Movement {
            maximum_speed: 5866,
            acceleration: 733,
            deceleration: 146,
            final_speed: 1026,
            ignite_t: 8,
            fuel_t: 564,
            remove_t: 1132,
            powered_turn_rate: 10920,
            unpowered_turn_rate: 8190,
            performance_at_0: 75,
            cruise: [78, 20, 78, 20],
            ..movement()
        };
        let motion = Motion {
            sink: EJECT_FPS,
            sags: true,
            cruise: Some(Cruise {
                stages: [(78. * 256., 20. * 256.); 2],
                lob: true,
            }),
            ..Motion::new(&m, [0., 0., 900.], 20000.)
        };
        (m, motion)
    }
    #[test]
    fn drop_falls_unsteered_until_the_motor_lights() {
        let (m, mut motion) = phoenix();
        let mut position = [0., 20000., 0.];
        let forward = [0., 0., 1.];
        // A target up and to the side cannot turn the unlit missile.
        assert_eq!(steer(&m, 0, forward, unit([1., 1., 1.])), forward);
        for age in 0..240 {
            let delta = motion.step(&m, age, forward);
            for i in 0..3 {
                position[i] += delta[i];
            }
        }
        // 32 ft/s kick, 32 ft/s² sag capped at 80 ft/s: about 124 ft in 2 s.
        assert!((20000. - position[1] - 124.).abs() < 1., "{}", position[1]);
        assert_eq!(motion.sink, SAG_MAX_FPS);
        assert_eq!(motion.velocity, [0., 0., 900.]);
        // Lighting the motor ends the sag at once and frees the fins.
        let delta = motion.step(&m, 240, forward);
        assert_eq!(motion.sink, 0.);
        assert!(delta[1].abs() < 1e-12);
        assert_ne!(steer(&m, 240, forward, unit([1., 1., 1.])), forward);
        // A rail missile's kick is gone on its first powered step.
        let mut rail = Motion {
            sink: EJECT_FPS,
            sags: true,
            ..Motion::new(&movement(), [0., 0., 900.], 0.)
        };
        rail.step(
            &Movement {
                ignite_t: 0,
                ..movement()
            },
            0,
            forward,
        );
        assert_eq!(rail.sink, 0.);
        // Burnout brings the sag back.
        let mut coast = Motion {
            sags: true,
            ..Motion::new(&m, [0., 0., 900.], 0.)
        };
        coast.step(&m, 564 * 30, forward);
        assert!((coast.sink - SAG_FPS2 * DT).abs() < 1e-12);
    }
    #[test]
    fn cruise_stages_hold_altitude_above_the_target_then_home_direct() {
        let harpoon = Cruise {
            stages: [(78. * 256., 4. * 256.), (20. * 256., 12. * 256.)],
            lob: false,
        };
        let target = [0., 0., 0.];
        let aim = |z: f64| harpoon.aim([0., 500., z], target);
        assert_eq!(aim(30000.), [0., 1024., 0.]);
        assert_eq!(aim(10000.), [0., 3072., 0.]);
        assert_eq!(aim(5000.), target);
        // Distance is horizontal only.
        assert_eq!(harpoon.aim([0., 90000., 19968.], target), [0., 1024., 0.]);
        // A lob climbs over the look-ahead, not along the whole distance.
        let lob = Cruise {
            lob: true,
            ..harpoon
        };
        let point = lob.aim([0., 0., 60760.], target);
        assert!((point[2] - (60760. - LOB_LOOKAHEAD_FT)).abs() < 1e-9);
        assert_eq!(point[1], 1024.);
        assert_eq!(lob.aim([0., 0., 5000.], target), target);
    }
    #[test]
    fn phoenix_lob_climbs_cruises_and_dives_onto_the_target() {
        let (m, mut motion) = phoenix();
        let target = [0., 20000., 30. * NMI];
        let mut position = [0., 20000., 0.];
        let mut forward = [0., 0., 1.];
        let mut highest: f64 = 0.;
        let mut lowest_drop: f64 = position[1];
        for age in 0..u64::from(m.remove_t) * 30 {
            let desired = commanded_heading(
                forward,
                motion.velocity,
                unit(sub(motion.aim(position, target), position)),
            );
            let next = steer(&m, age, forward, desired);
            motion.turn(forward, next);
            forward = next;
            let delta = motion.step(&m, age, forward);
            for i in 0..3 {
                position[i] += delta[i];
            }
            if age < 240 {
                lowest_drop = lowest_drop.min(position[1]);
            }
            highest = highest.max(position[1]);
            if length(sub(target, position)) < 100. {
                break;
            }
        }
        assert!(lowest_drop < 19880.);
        // It levels near 5,120 ft above the target and still arrives.
        assert!((24900. ..25400.).contains(&highest), "{highest}");
        assert!(length(sub(target, position)) < 100.);
        // The climbing prediction reaches the same target.
        let (m, motion) = phoenix();
        assert!(
            intercept(
                &m,
                motion,
                [0., 20000., 0.],
                [0., 0., 1.],
                target,
                [0.; 3],
                0,
                1132 * 30
            )
            .is_some()
        );
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

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "missiles_checkpoint.rs"]
mod checkpoint;
