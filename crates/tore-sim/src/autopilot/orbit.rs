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
//! - **Search and orbit** (John, 2026-10-14: "in release mode show a status
//!   that the view isn't pinned and the autopilot just flies straight as a
//!   search mode; pinning will tell autopilot to gracefully circle that
//!   point of aim"): with the sight free the console's autopilot flies
//!   straight and level on a commanded heading, altitude and speed
//!   ([`Orbit::pinned`] false, [`ConsolePhase::Search`]); a pin or a track
//!   joins the orbit about it; letting the sight go takes the aircraft's
//!   current heading, altitude and speed as the commanded ones (John,
//!   2026-10-14) and flies straight from there. The console's keys step the
//!   commanded values ([`ConsoleCommand`]). A pin moved by a slew moves the
//!   orbit's centre only after [`CENTRE_PAUSE_S`] with the pin standing
//!   still (John, 2026-10-14); a tracked object is followed at once.
//!
//! All constants here are fitted (agent decisions, 2026-10-13 and, for the
//! search and the console's keys, 2026-10-14) unless their notes say
//! otherwise.
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
/// A pin that moved (a slew, or a new pin) moves the centre only after it
/// has stood still this long, seconds (John, 2026-10-14: "after a 5 second
/// pause with no slewing").
pub const CENTRE_PAUSE_S: f64 = 5.;
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
/// Search: the bank toward the commanded heading is at most this, degrees
/// (5 under the AC-130's 30, as the orbit plans).
pub const SEARCH_BANK_DEG: f64 = 25.;
/// Search: bank per radian of heading error near the heading (the heading
/// modes' 1.5).
pub const SEARCH_BANK_GAIN: f64 = 1.5;
/// The console's heading keys step the search heading this far, degrees.
pub const HEADING_STEP_DEG: f64 = 5.;
/// The console's altitude keys step the search altitude, and the orbit's
/// height above its point, this far, ft.
pub const ALTITUDE_STEP_FT: f64 = 500.;
/// The search altitude stays within these, ft above sea level (the terrain
/// floor still applies).
pub const SEARCH_ALTITUDE_FT: [f64; 2] = [1_000., 25_000.];
/// The console's throttle step keys move the speed this far, knots.
pub const SPEED_STEP_KT: f64 = 5.;
const KT: f64 = crate::runway_wind::FEET_PER_SECOND_PER_KNOT;
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
    /// A point is held (ORBIT): the sight is pinned or tracking. Without one
    /// the console searches: straight and level on `heading`, `altitude`
    /// and `speed_fps`.
    pub pinned: bool,
    /// The search's commanded heading (radians, the flight's yaw) and
    /// altitude (ft above sea level).
    pub heading: f64,
    pub altitude: f64,
    /// Seconds the point has stood still (a pin); the centre moves toward
    /// it from [`CENTRE_PAUSE_S`]. A track holds it at the pause, so the
    /// centre follows the object at once.
    pub still_s: f64,
}

/// A key of the sensor console (docs/spec/ac130-linked-guns.md, "The
/// console's keys"), carried by the seat command `Console`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleCommand {
    /// P: pin the ground under the crosshair, or let go of the pin or track
    /// (the world's to apply: it is the gunsight's).
    Pin,
    /// Shift+Left and Shift+Right: step the search heading,
    /// [`HEADING_STEP_DEG`] a step, right positive.
    Heading(i8),
    /// Shift+Up and Shift+Down: step the search altitude, or the orbit's
    /// height, [`ALTITUDE_STEP_FT`] a step, up positive.
    Altitude(i8),
    /// The throttle presets 1 to 6: the speed at this percentage of the
    /// band from corner to maximum speed (0 to 100).
    Speed(u8),
    /// The throttle steps 7 and 8: step the speed [`SPEED_STEP_KT`] a step.
    SpeedStep(i8),
}

/// What the console's autopilot is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsolePhase {
    /// No point held: straight and level on the commanded values.
    Search,
    /// A point held, still joining the circle.
    Join,
    /// On the circle about the point.
    Orbit,
}
impl ConsolePhase {
    /// The console's own word for it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Search => "SEARCH",
            Self::Join => "ORBIT JOIN",
            Self::Orbit => "ORBIT",
        }
    }
}

/// The console's autopilot for a screen to draw (slice nr-conb): the phase,
/// the commanded values and, holding a point, the orbit's status.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConsoleStatus {
    pub phase: ConsolePhase,
    /// The search's commanded heading, degrees 0 to 360 (as the heading
    /// readouts: the flight's yaw), and altitude, ft above sea level.
    pub heading_deg: f64,
    pub altitude_ft: f64,
    /// The commanded speed, knots (search and orbit alike).
    pub speed_kt: f64,
    /// The orbit's commanded height above its point, ft.
    pub orbit_height_ft: f64,
    /// The orbit, while a point is held.
    pub orbit: Option<OrbitStatus>,
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
    /// The console's autopilot for guns that reach `reach_ft`, planned from
    /// the aircraft's speed limits where it is now, searching: it rolls out
    /// on the heading the roll-out reaches and holds the altitude it is at.
    /// [`Self::pin`] joins an orbit.
    pub fn new(s: &State, reach_ft: f64) -> Self {
        let limits = crate::ai::mission::speed_limits_of(s);
        let max_bank = gunship::MAX_BANK_DEG;
        let reach = if reach_ft > 0. {
            reach_ft
        } else {
            gunship::DEFAULT_REACH_FT
        };
        let plan = gunship::orbit_plan(reach, limits.corner.0, limits.maximum.0, max_bank);
        let here = s.position;
        let mut orbit = Self {
            point: here,
            centre: here,
            radius_ft: plan.radius_ft,
            height_ft: plan.height_ft,
            speed_fps: plan.speed_fps,
            reach_ft: reach,
            bank: s.bank,
            climb: s.vertical_speed,
            throttle: s.throttle,
            pinned: false,
            heading: s.yaw.rem_euclid(std::f64::consts::TAU),
            altitude: s.position[1].clamp(SEARCH_ALTITUDE_FT[0], SEARCH_ALTITUDE_FT[1]),
            still_s: CENTRE_PAUSE_S,
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
        let (wanted, altitude) = if self.pinned {
            self.circle(s)
        } else {
            (search_bank(s, self.heading), self.altitude)
        };
        let turn = BANK_RATE_DEG_S.to_radians() * DT;
        self.bank += angle(wanted - self.bank).clamp(-turn, turn);
        input.roll = roll_for_bank(s, self.bank);
        // The height above the terrain floor.
        let altitude = altitude.max(ground + TERRAIN_FLOOR_FT);
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
    /// The orbit's wanted bank (radians) and altitude (ft) this tick: the
    /// centre slides toward the sight's point, the bank is the AI's circle
    /// law, the height the orbit's over the centre.
    fn circle(&mut self, s: &State) -> (f64, f64) {
        self.still_s = (self.still_s + DT).min(CENTRE_PAUSE_S);
        let to: Vector = std::array::from_fn(|i| self.point[i] - self.centre[i]);
        let gap = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
        let step = CENTRE_SPEED_FPS * DT;
        if self.still_s < CENTRE_PAUSE_S {
            // The pin is still moving, or has just stopped: the centre waits.
        } else if gap <= step {
            self.centre = self.point;
        } else {
            for (c, d) in self.centre.iter_mut().zip(to) {
                *c += d * step / gap;
            }
        }
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
        (
            steer.bank_deg.unwrap_or(0.).to_radians(),
            self.centre[1] + self.height_ft,
        )
    }
    /// Hold `point` (a pin, or with `tracked` the ground under a tracked
    /// object): from the search the orbit is joined about it at once (the
    /// bank and climb commands' slews make the join gentle). Holding one
    /// already, a moved pin waits [`CENTRE_PAUSE_S`] standing still before
    /// the centre slides to it; a track's centre follows at once.
    pub fn pin(&mut self, point: Vector, tracked: bool) {
        if !point.iter().all(|v| v.is_finite()) {
            return;
        }
        if !self.pinned {
            self.pinned = true;
            self.centre = point;
            self.still_s = CENTRE_PAUSE_S;
            self.fit();
        } else if tracked {
            self.still_s = CENTRE_PAUSE_S;
        } else if point != self.point {
            self.still_s = 0.;
        }
        self.point = point;
    }
    /// Let the point go: the aircraft's current heading, altitude and speed
    /// become the commanded ones (John, 2026-10-14), and it flies straight
    /// from there.
    pub fn release(&mut self, s: &State) {
        if self.pinned {
            self.pinned = false;
            self.take_current(s);
        }
    }
    /// The search's commanded heading, altitude and speed: the aircraft's now.
    fn take_current(&mut self, s: &State) {
        self.heading = s.yaw.rem_euclid(std::f64::consts::TAU);
        self.altitude = s.position[1].clamp(SEARCH_ALTITUDE_FT[0], SEARCH_ALTITUDE_FT[1]);
        if s.speed.is_finite() && s.speed > 0. {
            self.speed_fps = s.speed;
        }
    }
    /// The fastest orbit: its minimum slant range (the turn at 30 degrees of
    /// bank seen at the minimum depression) stays within
    /// [`gunship::SLANT_CAP_SHARE`] of the guns' reach, ft/s.
    pub fn orbit_speed_cap(&self) -> f64 {
        let depression = minimum_depression();
        (gunship::SLANT_CAP_SHARE
            * self.reach_ft
            * depression.cos()
            * GRAVITY
            * gunship::MAX_BANK_DEG.to_radians().tan())
        .sqrt()
    }
    /// The orbit's height range above its point at its radius, ft: the
    /// camera's minimum depression up to the guns' slant cap.
    pub fn height_range(&self) -> (f64, f64) {
        let low = self.radius_ft * minimum_depression().tan();
        let cap = gunship::SLANT_CAP_SHARE * self.reach_ft;
        let high = (cap * cap - self.radius_ft * self.radius_ft).max(0.).sqrt();
        (low, high.max(low))
    }
    /// Keep the orbit flyable after its speed changed: no faster than
    /// [`Self::orbit_speed_cap`] while it holds a point, and the radius and
    /// height no smaller than the minimum at that speed.
    fn fit(&mut self) {
        if self.pinned {
            self.speed_fps = self.speed_fps.min(self.orbit_speed_cap());
        }
        let (radius, _) = self.minimum();
        self.radius_ft = self.radius_ft.max(radius);
        let (low, _) = self.height_range();
        self.height_ft = self.height_ft.max(low);
    }
    /// One of the console's keys ([`ConsoleCommand`], all but the pin, which
    /// is the gunsight's): steps the commanded values and returns the line
    /// the pilot reads. `limits` is the speed band, corner to maximum, ft/s.
    pub fn command(&mut self, command: ConsoleCommand, limits: (f64, f64)) -> String {
        let (low, high) = (limits.0.min(limits.1), limits.0.max(limits.1));
        let high = if self.pinned {
            high.min(self.orbit_speed_cap()).max(low)
        } else {
            high
        };
        match command {
            ConsoleCommand::Pin => "Sensor console: pin".into(),
            ConsoleCommand::Heading(_) if self.pinned => {
                "Orbiting: P lets the point go to steer".into()
            }
            ConsoleCommand::Heading(steps) => {
                let step = f64::from(steps) * HEADING_STEP_DEG.to_radians();
                // Steps land on whole multiples of the step.
                let degrees = (self.heading + step).to_degrees();
                let snapped = (degrees / HEADING_STEP_DEG).round() * HEADING_STEP_DEG;
                self.heading = snapped.rem_euclid(360.).to_radians();
                format!("Search heading {:03.0}", heading_degrees(self.heading))
            }
            ConsoleCommand::Altitude(steps) if self.pinned => {
                let (low, high) = self.height_range();
                let wanted = self.height_ft + f64::from(steps) * ALTITUDE_STEP_FT;
                self.height_ft = wanted.clamp(low, high);
                let limit = if wanted < low {
                    ": the camera's lowest"
                } else if wanted > high {
                    ": the guns' highest"
                } else {
                    ""
                };
                format!(
                    "Orbit height {:.0} ft above the point{limit}",
                    self.height_ft
                )
            }
            ConsoleCommand::Altitude(steps) => {
                let wanted = (self.altitude / ALTITUDE_STEP_FT).round() * ALTITUDE_STEP_FT
                    + f64::from(steps) * ALTITUDE_STEP_FT;
                self.altitude = wanted.clamp(SEARCH_ALTITUDE_FT[0], SEARCH_ALTITUDE_FT[1]);
                format!("Search altitude {:.0} ft", self.altitude)
            }
            ConsoleCommand::Speed(percent) => {
                let share = f64::from(percent.min(100)) / 100.;
                self.speed_fps = (low + share * (high - low)).clamp(low, high);
                self.fit();
                format!("Speed {:.0} kt", self.speed_fps / KT)
            }
            ConsoleCommand::SpeedStep(steps) => {
                let knots = (self.speed_fps / KT / SPEED_STEP_KT).round() * SPEED_STEP_KT
                    + f64::from(steps) * SPEED_STEP_KT;
                self.speed_fps = (knots * KT).clamp(low, high);
                self.fit();
                format!("Speed {:.0} kt", self.speed_fps / KT)
            }
        }
    }
    /// The console's status with the aircraft in state `s`.
    pub fn console_status(&self, s: &State) -> ConsoleStatus {
        let orbit = self.pinned.then(|| self.status(s));
        ConsoleStatus {
            phase: match orbit {
                None => ConsolePhase::Search,
                Some(o) if o.on_orbit => ConsolePhase::Orbit,
                Some(_) => ConsolePhase::Join,
            },
            heading_deg: heading_degrees(self.heading),
            altitude_ft: self.altitude,
            speed_kt: self.speed_fps / KT,
            orbit_height_ft: self.height_ft,
            orbit,
        }
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

/// A heading (radians, the flight's yaw) as the readouts show it, degrees
/// 0 to 360.
pub fn heading_degrees(heading: f64) -> f64 {
    let degrees = heading.to_degrees().rem_euclid(360.);
    if degrees >= 359.999_999 { 0. } else { degrees }
}

/// How far the heading still turns while the bank `bank` (radians) rolls
/// out at [`BANK_RATE_DEG_S`] at `speed` (ft/s), radians, signed as the
/// turn: the integral of the turn rate g tan(bank) / v over a roll-out at a
/// steady rate, g (-ln cos bank) / (v r).
pub fn rollout_turn(bank: f64, speed: f64) -> f64 {
    let rate = BANK_RATE_DEG_S.to_radians();
    let bank = bank.clamp(-1.4, 1.4);
    bank.signum() * GRAVITY * -bank.abs().cos().ln() / (speed.max(1.) * rate)
}

/// The search's wanted bank toward `heading` (radians): the bank whose
/// roll-out at [`BANK_RATE_DEG_S`] turns exactly the error that is left,
/// within [`SEARCH_BANK_GAIN`] times the error near the heading and
/// [`SEARCH_BANK_DEG`]; so a heading change rolls in, holds the bank and
/// rolls out onto the heading without overshooting it.
fn search_bank(s: &State, heading: f64) -> f64 {
    let error = angle(heading - s.yaw);
    let rate = BANK_RATE_DEG_S.to_radians();
    // Inverse of [`rollout_turn`]: cos(bank) = exp(-|error| v r / g).
    let rollout = (-error.abs() * s.speed.max(1.) * rate / GRAVITY)
        .exp()
        .clamp(-1., 1.)
        .acos();
    error.signum()
        * rollout
            .min(SEARCH_BANK_GAIN * error.abs())
            .min(SEARCH_BANK_DEG.to_radians())
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
    /// Why the sensor console cannot open now, as the message the pilot
    /// reads.
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
    /// Open the console's autopilot searching (no point held), for guns
    /// that reach `reach_ft`. The host calls it when the AC-130's sensor
    /// console opens with a free sight, after [`Self::orbit_refusal`].
    pub fn engage_console(&mut self, reach_ft: f64) {
        let mut orbit = Orbit::new(self, reach_ft);
        // The search starts on the aircraft's current heading, altitude and
        // speed (agent decision, as a release does).
        orbit.take_current(self);
        self.autopilot.engage_orbit(orbit);
    }
    /// Open the console's autopilot orbiting the ground `point` (world
    /// feet) with guns that reach `reach_ft`: the console opened on a pin
    /// or a track.
    pub fn engage_orbit(&mut self, point: Vector, reach_ft: f64) {
        let mut orbit = Orbit::new(self, reach_ft);
        orbit.pin(point, false);
        self.autopilot.engage_orbit(orbit);
    }
    /// The sight let its point go: the console searches on from here.
    pub fn release_orbit(&mut self) {
        let mut orbit = match self.autopilot.orbit() {
            Some(orbit) if orbit.pinned => *orbit,
            _ => return,
        };
        orbit.release(self);
        self.autopilot.set_orbit(orbit);
    }
    /// One of the console's keys: the line the pilot reads, or None with
    /// the console closed.
    pub fn console_command(&mut self, command: ConsoleCommand) -> Option<String> {
        let mut orbit = *self.autopilot.orbit()?;
        let limits = crate::ai::mission::speed_limits_of(self);
        let message = orbit.command(command, (limits.corner.0, limits.maximum.0));
        self.autopilot.set_orbit(orbit);
        Some(message)
    }
    /// The orbit's status, while the console's autopilot holds a point.
    pub fn orbit_status(&self) -> Option<OrbitStatus> {
        self.autopilot
            .orbit()
            .filter(|orbit| orbit.pinned)
            .map(|orbit| orbit.status(self))
    }
    /// The console's autopilot, searching or orbiting, while the console is
    /// open.
    pub fn console_status(&self) -> Option<ConsoleStatus> {
        self.autopilot
            .orbit()
            .map(|orbit| orbit.console_status(self))
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
    pinned,
    heading,
    altitude,
    still_s,
});

#[cfg(test)]
#[path = "orbit_tests.rs"]
mod tests;
