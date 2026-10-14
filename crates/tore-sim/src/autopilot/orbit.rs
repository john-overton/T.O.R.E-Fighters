//! Orbit hold: the AC-130's attack orbit flown for the sensor console
//! (docs/spec/ac130-linked-guns.md, "The orbit"; John's request,
//! 2026-10-13: "an AI autopilot mode that circles the plane at least the
//! minimum optimum altitude and radius for attacking where the targeting
//! pod is centered, making smooth flight adjustments to keep the optics,
//! and all three guns available to bear").
//!
//! - **The orbit** is the AI gunship's ([`gunship::orbit_plan`]): a
//!   left-hand circle whose slant range to the centre is 80% of the guns'
//!   reach at 35 degrees of depression, flown halfway between corner and
//!   maximum speed, never banked past the AC-130's 30 degrees. It is
//!   planned once, at engagement, from the aircraft's speed limits there.
//! - **The minimum** ([`Orbit::minimum`]) is what the guns and the sensor
//!   need: the turn at the orbit speed and the full 30 degrees of bank, and
//!   a slant range the anti-aircraft guns at the centre cannot reach, both at
//!   35 degrees of depression (5 degrees below the steepest bank, so the
//!   centre always sits below the wing line, inside the camera's gimbal and
//!   inside every gun's 45 degree elevation arc). The planned orbit is never
//!   below it.
//! - **Flying it** ([`Orbit::fly`]) through the stick and the throttle, as a
//!   pilot would, so the flight model is untouched: the AI's circle law for
//!   the bank ([`gunship::orbit_steer`]), the bank command slewed at
//!   [`BANK_RATE_DEG_S`]; the climb toward the orbit height closed in
//!   [`CLIMB_TIME_S`] within [`CLIMB_LIMIT_FPS`], its rate of change held to
//!   [`CLIMB_ACCEL_FPS2`]; the speed held on the throttle. The centre the
//!   law flies about slides toward the sight's point at
//!   [`CENTRE_SPEED_FPS`], so a new pin re-centres the orbit without a jolt.
//!
//! All constants here are fitted (agent decisions, 2026-10-13) unless their
//! notes say otherwise.
use super::{GRAVITY, angle, pitch_for_climb, roll_for_bank};
use crate::ai::strike::gunship;
use crate::attitude::{Basis, Vector};
use crate::combat::gunship as guns;
use crate::flight::{PilotCommand, PilotInput, State};
use std::f64::consts::FRAC_PI_2;

const DT: f64 = 1. / 120.;
/// The bank command moves at most this fast, degrees a second.
pub const BANK_RATE_DEG_S: f64 = 5.;
/// The height error is closed in this many seconds...
pub const CLIMB_TIME_S: f64 = 15.;
/// ...within this climb or sink rate, ft/s (1,500 ft/min).
pub const CLIMB_LIMIT_FPS: f64 = 25.;
/// The climb command changes at most this fast, ft/s².
pub const CLIMB_ACCEL_FPS2: f64 = 3.;
/// The centre slides toward a new point at this speed, ft/s (about 120 kt:
/// faster than any ground vehicle, slow enough that a new pin miles away is
/// a gentle turn rather than a snap).
pub const CENTRE_SPEED_FPS: f64 = 200.;
/// The orbit is never flown lower than this above the ground under the
/// aircraft, ft (a terrain floor for a centre in a deep valley).
pub const TERRAIN_FLOOR_FT: f64 = 1_000.;
/// Anti-aircraft guns reach this far: the minimum slant range from the
/// centre (the M163 and ZSU-23 fire range, docs/spec/surface-defenses.md).
pub const THREAT_SLANT_FT: f64 = 7_500.;
/// The centre sits at least this far below the wing line at the steepest
/// bank, degrees: the minimum depression is the maximum bank plus it.
pub const SENSOR_MARGIN_DEG: f64 = 5.;
/// The throttle's proportional and integral gains, per ft/s of speed error
/// (and per second for the integral), and its fastest travel a second.
const THROTTLE_P: f64 = 0.01;
const THROTTLE_I: f64 = 0.002;
const THROTTLE_RATE: f64 = 0.2;

/// Orbit hold's plan and memory. Coded with the autopilot in the exact
/// flight state, so the host, a client's prediction and a checkpoint fly
/// the same orbit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbit {
    /// The ground point the sight is centred on, world feet: the pin, the
    /// tracked object, or where the sight looked when it opened.
    pub point: Vector,
    /// The centre the law flies about; it slides toward `point`.
    pub centre: Vector,
    /// The planned orbit: horizontal radius and height above the centre,
    /// ft, and the speed it is flown at, ft/s.
    pub radius_ft: f64,
    pub height_ft: f64,
    pub speed_fps: f64,
    /// The shortest reach of the guns, ft.
    pub reach_ft: f64,
    /// The slewed bank command (radians, left negative), the climb command
    /// (ft/s) and the throttle's integral term.
    pub bank: f64,
    pub climb: f64,
    pub throttle: f64,
}

/// What the orbit is doing, for the sensor console and the HUD: one read of
/// the flight state, the same in single player, on a client and in a check.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitStatus {
    /// The point the sight is centred on and the centre flown about, world
    /// feet.
    pub point: Vector,
    pub centre: Vector,
    /// The planned radius, height above the centre and speed (ft, ft/s).
    pub radius_ft: f64,
    pub height_ft: f64,
    pub speed_fps: f64,
    /// The minimum radius and height ([`Orbit::minimum`]).
    pub minimum_radius_ft: f64,
    pub minimum_height_ft: f64,
    /// Where the aircraft is now: horizontal distance from the centre and
    /// height above it, ft, and the slant range to the sight's point.
    pub distance_ft: f64,
    pub height_now_ft: f64,
    pub slant_ft: f64,
    /// On the circle: within a quarter of the radius and 30 degrees of the
    /// circle law's heading (the AI's test, [`gunship::on_orbit`]).
    pub on_orbit: bool,
    /// Each gun (25, 40, 105 mm) could bear on the sight's point from where
    /// the aircraft is: inside its heading and elevation arcs, clear of its
    /// own airframe and within the guns' reach.
    pub guns_bearing: [bool; 3],
    /// The point lies inside the camera's gimbal (below the wing line).
    pub sensor_bearing: bool,
}
impl OrbitStatus {
    pub fn all_bearing(&self) -> bool {
        self.sensor_bearing && self.guns_bearing.iter().all(|b| *b)
    }
}

impl Orbit {
    /// The orbit about `point` for guns that reach `reach_ft`, planned from
    /// the aircraft's speed limits where it is now.
    pub fn new(s: &State, point: Vector, reach_ft: f64) -> Self {
        let limits = crate::ai::mission::speed_limits_of(s);
        let max_bank = gunship::MAX_BANK_DEG;
        let reach = if reach_ft > 0. {
            reach_ft
        } else {
            gunship::DEFAULT_REACH_FT
        };
        let plan = gunship::orbit_plan(reach, limits.corner.0, limits.maximum.0, max_bank);
        let mut orbit = Self {
            point,
            centre: point,
            radius_ft: plan.radius_ft,
            height_ft: plan.height_ft,
            speed_fps: plan.speed_fps,
            reach_ft: reach,
            bank: s.bank,
            climb: s.vertical_speed,
            throttle: s.throttle,
        };
        // Short guns can plan inside the anti-aircraft guns' reach: the
        // orbit grows to the minimum, keeping its depression.
        let (radius, _) = orbit.minimum();
        if orbit.radius_ft < radius {
            orbit.radius_ft = radius;
            orbit.height_ft = radius * gunship::DEPRESSION_DEG.to_radians().tan();
        }
        orbit
    }
    /// The smallest orbit the guns and the camera allow at this speed:
    /// radius and height above the centre, ft. The radius is the turn at
    /// the orbit speed and the full 30 degrees of bank, or the radius that
    /// keeps the slant range beyond the anti-aircraft guns' reach (but within
    /// 90% of the AC-130's own), whichever is larger; the height puts the centre at the minimum depression,
    /// the maximum bank plus [`SENSOR_MARGIN_DEG`] (35 degrees).
    pub fn minimum(&self) -> (f64, f64) {
        let depression = minimum_depression();
        let turn =
            self.speed_fps * self.speed_fps / (GRAVITY * gunship::MAX_BANK_DEG.to_radians().tan());
        let threat = THREAT_SLANT_FT.min(gunship::SLANT_CAP_SHARE * self.reach_ft);
        let radius = turn.max(threat * depression.cos());
        (radius, radius * depression.tan())
    }
    /// One tick of the orbit: rewrites the stick, pedals and throttle the
    /// pilot gave (the console ignores them).
    pub(super) fn fly(&mut self, s: &State, ground: f64, input: &mut PilotInput) {
        // The centre slides toward the sight's point.
        let to: Vector = std::array::from_fn(|i| self.point[i] - self.centre[i]);
        let gap = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
        let step = CENTRE_SPEED_FPS * DT;
        if gap <= step {
            self.centre = self.point;
        } else {
            for (c, d) in self.centre.iter_mut().zip(to) {
                *c += d * step / gap;
            }
        }
        // The bank: the AI's circle law, slewed.
        let plan = gunship::OrbitPlan {
            radius_ft: self.radius_ft,
            height_ft: self.height_ft,
            slant_ft: self.radius_ft.hypot(self.height_ft),
            depression_deg: gunship::DEPRESSION_DEG,
            speed_fps: self.speed_fps,
            bank_deg: 0.,
        };
        let steer = gunship::orbit_steer(
            s.position,
            s.yaw.to_degrees(),
            s.speed,
            self.centre,
            &plan,
            gunship::MAX_BANK_DEG,
        );
        let wanted = steer.bank_deg.unwrap_or(0.).to_radians();
        let turn = BANK_RATE_DEG_S.to_radians() * DT;
        self.bank += angle(wanted - self.bank).clamp(-turn, turn);
        input.roll = roll_for_bank(s, self.bank);
        // The height: the orbit's over the centre, above the terrain floor.
        let altitude = (self.centre[1] + self.height_ft).max(ground + TERRAIN_FLOOR_FT);
        let climb =
            ((altitude - s.position[1]) / CLIMB_TIME_S).clamp(-CLIMB_LIMIT_FPS, CLIMB_LIMIT_FPS);
        let change = CLIMB_ACCEL_FPS2 * DT;
        self.climb += (climb - self.climb).clamp(-change, change);
        input.pitch = pitch_for_climb(s, self.climb);
        input.yaw = 0.;
        // The speed, on the throttle; the pilot's throttle waits.
        let error = self.speed_fps - s.speed;
        self.throttle = (self.throttle + THROTTLE_I * error * DT).clamp(0., 1.);
        let wanted = (self.throttle + THROTTLE_P * error).clamp(0., 1.);
        let travel = THROTTLE_RATE * DT;
        input.throttle = Some(s.throttle + (wanted - s.throttle).clamp(-travel, travel));
        input.throttle_rate = 0.;
        input.commands.retain(|c| {
            !matches!(
                c,
                PilotCommand::Throttle(_) | PilotCommand::AdjustThrottle(_)
            )
        });
    }
    /// The orbit's status with the aircraft in state `s`.
    pub fn status(&self, s: &State) -> OrbitStatus {
        let (minimum_radius_ft, minimum_height_ft) = self.minimum();
        let dx = s.position[0] - self.centre[0];
        let dz = s.position[2] - self.centre[2];
        let (guns_bearing, sensor_bearing) = bearing(s, self.point, self.reach_ft);
        let to: Vector = std::array::from_fn(|i| self.point[i] - s.position[i]);
        OrbitStatus {
            point: self.point,
            centre: self.centre,
            radius_ft: self.radius_ft,
            height_ft: self.height_ft,
            speed_fps: self.speed_fps,
            minimum_radius_ft,
            minimum_height_ft,
            distance_ft: dx.hypot(dz),
            height_now_ft: s.position[1] - self.centre[1],
            slant_ft: (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt(),
            on_orbit: gunship::on_orbit(
                s.position,
                s.yaw.to_degrees(),
                self.centre,
                self.radius_ft,
            ),
            guns_bearing,
            sensor_bearing,
        }
    }
}

/// The minimum depression of the centre from the orbit, radians: the
/// AC-130's maximum bank plus [`SENSOR_MARGIN_DEG`].
pub fn minimum_depression() -> f64 {
    (gunship::MAX_BANK_DEG + SENSOR_MARGIN_DEG).to_radians()
}

/// Whether each of the AC-130's guns (25, 40, 105 mm) could bear on
/// `point` from the aircraft in state `s`, and whether the camera's gimbal
/// holds it: the plain line from each gun's pivot (from the sensor dome's
/// eye for the camera) against the arcs of docs/spec/ac130-linked-guns.md,
/// the gun's own airframe and `reach_ft`.
pub fn bearing(s: &State, point: Vector, reach_ft: f64) -> ([bool; 3], bool) {
    let basis = Basis::new(s.yaw, s.pitch, s.bank);
    let body = |mount: Vector| -> (f64, f64, f64) {
        let origin: Vector = std::array::from_fn(|i| {
            s.position[i]
                + basis.right[i] * mount[0]
                + basis.up[i] * mount[1]
                + basis.forward[i] * mount[2]
        });
        let to: Vector = std::array::from_fn(|i| point[i] - origin[i]);
        let dot = |a: Vector| a[0] * to[0] + a[1] * to[1] + a[2] * to[2];
        let (right, up, forward) = (dot(basis.right), dot(basis.up), dot(basis.forward));
        (
            right.atan2(forward),
            up.atan2(right.hypot(forward)),
            (right * right + up * up + forward * forward).sqrt(),
        )
    };
    let guns = std::array::from_fn(|slot| {
        let (heading, elevation, range) = body(guns::pivot(slot));
        (heading + FRAC_PI_2).abs() <= guns::HEADING_ARC[slot]
            && elevation.abs() <= guns::ELEVATION_ARC[slot]
            && range <= reach_ft
            && guns::clear_airframe(slot, heading, elevation)
    });
    let (_, elevation, _) = body(guns::eye());
    (guns, elevation <= crate::combat::sight::GIMBAL_TOP)
}

impl State {
    /// Why orbit hold cannot engage now, as the message the pilot reads.
    pub fn orbit_refusal(&self) -> Option<&'static str> {
        if self.crashed {
            return Some("The aircraft is down");
        }
        if self.weight_on_wheels() {
            return Some("Sensor console: not on the ground");
        }
        if !self.systems.autopilot_available() || self.damage_regions[3..].iter().any(|v| *v > 0.) {
            return Some("Sensor console: the autopilot is damaged");
        }
        None
    }
    /// Engage orbit hold about the ground `point` (world feet) with guns
    /// that reach `reach_ft`. The host calls it when the AC-130's sensor
    /// console opens, after [`Self::orbit_refusal`].
    pub fn engage_orbit(&mut self, point: Vector, reach_ft: f64) {
        let orbit = Orbit::new(self, point, reach_ft);
        self.autopilot.engage_orbit(orbit);
    }
    /// Orbit hold's status, while it is engaged.
    pub fn orbit_status(&self) -> Option<OrbitStatus> {
        self.autopilot.orbit().map(|orbit| orbit.status(self))
    }
}

crate::flight::exact::exact_struct!(Orbit {
    point,
    centre,
    radius_ft,
    height_ft,
    speed_fps,
    reach_ft,
    bank,
    climb,
    throttle,
});

#[cfg(test)]
#[path = "orbit_tests.rs"]
mod tests;
