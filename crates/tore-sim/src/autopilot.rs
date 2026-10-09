//! Spec-derived mode behavior with fitted control gains. See docs/spec/autopilot.md.
//!
//! Four modes: off, heading and altitude (A), waypoint (Ctrl+A) and hover
//! hold (Ctrl+Alt+A, the helicopters and the V-22 only; VTOL overhaul slice
//! P9). Every mode flies by rewriting the pilot's inputs for the tick, before
//! the flight model sees them: the stick and pedals on every aircraft, and
//! the collective on the rotorcraft. Nothing moves the aircraft directly, so a
//! tick flown on the autopilot and the same inputs flown by hand are the same
//! flight.
//!
//! The heading and waypoint modes fly two laws. Conventional aircraft and the
//! vectoring jets keep the fixed-wing law (a bank command through the roll
//! rate, a climb command through the load factor), the jets only at flying
//! speed. The helicopters and the V-22 fly attitude loops through their
//! cyclic instead, since their stick commands a disk tilt, not a load factor,
//! and only at 40 kt and above, where hover hold stops. The helicopters, and
//! the V-22 with its nacelles up, hold the height on the collective and the
//! speed on the cyclic, as a helicopter's coupled autopilot does; the V-22
//! with its nacelles down holds the height on the pitch and leaves its power
//! lever to the pilot, as the throttle is on every other aircraft.
use crate::flight::{DT, FlightAxis, PilotCommand, PilotInput, State, Switch, trace::Release};
use crate::models::FlightModel;
use crate::models::variety::LiftKind;
use std::f64::consts::{PI, TAU};
use tore_input::{LiftCommand, TrimAxis};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Heading,
    Waypoint,
    /// Hover hold: the helicopters and the V-22 hold a ground point, the
    /// heading and a height above the ground (design 5.4).
    Hover,
}

/// Selected mission waypoint. Coordinates use simulation world X/Z in feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavigationTarget {
    pub number: u32,
    pub position: [f64; 2],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Autopilot {
    mode: Mode,
    /// Heading held, rad: the A modes' and hover hold's.
    heading: f64,
    /// Altitude held by the A modes, ft.
    altitude: f64,
    target: Option<NavigationTarget>,
    /// The powered-lift loops' memory; default on every other aircraft.
    loops: Loops,
}

/// What the rotorcraft loops remember from tick to tick: hover hold, and the
/// heading and waypoint modes on the helicopters and the V-22. Reset at every
/// engagement and coded with the autopilot in the exact flight state, so
/// snapshots and checkpoints restore a hold mid-flight.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Loops {
    /// Set on the first tick the loops fly after an engagement.
    started: bool,
    /// Hover hold: the ground point held, world X/Z ft. None while the
    /// drift is still being braked.
    point: Option<[f64; 2]>,
    /// Hover hold: the wheel height above the ground held, ft.
    height: f64,
    /// Hover hold: the pilot's collective (or throttle) lever position when
    /// the hold first saw it. Moving it 2 percent from there cancels.
    lever: Option<f64>,
    /// Outer loop integrators, rad: hover hold's forward and right tilt
    /// that holds the ground velocity, or the A modes' pitch attitude [0].
    tilt: [f64; 2],
    /// Attitude loop integrators: the stick and pedal travel that holds the
    /// commanded attitude and heading, [pitch, roll, yaw].
    stick: [f64; 3],
    /// The collective lever, 0..1, of hover hold and the rotorcraft A
    /// modes: their height loop's integrator.
    collective: f64,
    /// The rotorcraft A modes: whether they fly the collective (the
    /// helicopters, and the V-22 with its nacelles up).
    coupled: bool,
    /// The rotorcraft A modes: the ground speed along the heading they hold
    /// while they fly the collective, ft/s.
    speed: f64,
}

/// Which law the heading and waypoint modes fly on an aircraft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Airframe {
    /// Fixed-wing aircraft, and every aircraft off the hybrid model.
    Conventional,
    /// The AV-8 and Yak-141: the fixed-wing law, with flying speed.
    Jet,
    /// The helicopters and the V-22: attitude loops through the cyclic.
    Rotorcraft,
}

impl Airframe {
    fn of(s: &State) -> Self {
        if s.research.is_none() || s.native.is_some() {
            return Self::Conventional;
        }
        match s.model().powered_lift().map(|lift| lift.kind) {
            Some(LiftKind::Helicopter | LiftKind::Tiltrotor) => Self::Rotorcraft,
            Some(LiftKind::VectorJet) => Self::Jet,
            None => Self::Conventional,
        }
    }
}

/// ft/s per knot.
const KT: f64 = crate::runway_wind::FEET_PER_SECOND_PER_KNOT;
/// Standard gravity, ft/s².
const GRAVITY: f64 = 32.174;
/// Hover hold engages below this ground speed, and the rotorcraft heading
/// and waypoint modes at or above it, kt (design 5.4; the A modes' floor is
/// an agent decision so the two never overlap).
pub const HOVER_SPEED_KT: f64 = 40.;
/// The rotorcraft heading and waypoint modes let go below this ground
/// speed, kt (fitted).
pub const ROTORCRAFT_RELEASE_KT: f64 = 30.;
/// The jets' heading and waypoint modes engage at their 1 G stall speed and
/// let go below this share of it (fitted).
pub const JET_RELEASE_STALL_SHARE: f64 = 0.85;
/// Hover hold needs the V-22's nacelles at this angle or more, degrees
/// (design 5.4).
pub const HOVER_NACELLE_DEGREES: f64 = 75.;
/// The lowest wheel height hover hold holds, ft (design 5.4).
pub const HOVER_MIN_HEIGHT_FT: f64 = 10.;
/// Hover hold's pitch and bank command limit, degrees (design 5.4, fitted).
pub const HOVER_TILT_DEGREES: f64 = 15.;
/// Hover hold's climb and sink limit, ft/min (design 5.4, fitted).
pub const HOVER_CLIMB_FPM: f64 = 500.;
/// The ground speed below which hover hold takes the point it holds, kt
/// (design A1: drift below 1 kt).
pub const HOVER_CAPTURE_KT: f64 = 1.;
/// A trim key tap moves the held point this far, ft (design 5.4).
pub const HOVER_NUDGE_FT: f64 = 10.;
/// A cyclic trim key tap moves the speed the rotorcraft A modes hold, kt
/// (fitted; agent decision).
pub const A_NUDGE_KT: f64 = 2.;
/// A collective or throttle lever moving this share of travel cancels hover
/// hold (design 5.4).
pub const HOVER_LEVER_MOVE: f64 = 0.02;
/// The rotorcraft heading and waypoint modes' pitch command limit, degrees
/// (fitted, as hover hold's).
const ROTORCRAFT_PITCH_DEGREES: f64 = 15.;
/// The rotorcraft heading and waypoint modes' climb and sink limit, ft/min
/// (fitted: the fixed-wing law's 20 m/s is beyond these aircraft).
const ROTORCRAFT_CLIMB_FPM: f64 = 1_000.;

// Fitted loop gains (agent decisions, 2026-10-09), tuned on the AH-64,
// Mi-24, CH-47 and V-22 at every stability level, with and without the Easy
// flight physics cheat, in calm air, steady 15 kt winds from four sides and
// a gusting wind. "The coupled A modes" are the heading and waypoint modes
// while they fly the collective (`collective_coupled`).
/// Attitude loops: the commanded rate per radian of error, 1/s.
const ATTITUDE_BANDWIDTH: f64 = 3.;
/// Attitude loops: the commanded rate never exceeds this share of the
/// aircraft's full-stick hover rate.
const ATTITUDE_RATE_SHARE: f64 = 0.5;
/// Attitude loops: stick travel per unit of rate error, as a share of the
/// full-stick hover rate.
const ATTITUDE_GAIN: f64 = 3.;
/// Attitude loops integrate only within this error, rad, so a large
/// attitude change does not wind them up.
const ATTITUDE_WINDOW: f64 = 0.15;
/// The heading and sideslip loops integrate within this error, rad.
const HEADING_WINDOW: f64 = 0.3;
/// Attitude loops: integrator, travel per radian-second of error.
const ATTITUDE_INTEGRAL: f64 = 0.6;
/// Heading loop: commanded yaw rate per radian of heading error, 1/s.
const HEADING_BANDWIDTH: f64 = 2.;
/// The rotorcraft A modes: yaw rate commanded per radian of sideslip, 1/s.
const SIDESLIP_BANDWIDTH: f64 = 1.;
/// Hover hold: ground velocity commanded per foot from the point, 1/s.
const POSITION_GAIN: f64 = 0.15;
/// Hover hold: the ground velocity it closes on the point at most, ft/s.
const POSITION_SPEED_FPS: f64 = 10.;
/// Hover hold's velocity loops and the coupled A modes' speed loop:
/// acceleration commanded per ft/s of error, 1/s.
const VELOCITY_GAIN: f64 = 0.8;
/// The same loops' tilt integrator, rad per foot of velocity error.
const VELOCITY_INTEGRAL: f64 = 0.002;
/// Hover hold: climb commanded per foot of height error, 1/s.
const HEIGHT_GAIN: f64 = 0.25;
/// Hover hold and the coupled A modes: collective per ft/s of climb error.
const CLIMB_GAIN: f64 = 0.08;
/// The same loops' collective integrator per foot of climb error.
const CLIMB_INTEGRAL: f64 = 0.02;
/// The V-22's A modes with its nacelles down: pitch per radian of flight
/// path error.
const PATH_GAIN: f64 = 1.;
/// The same law's pitch integrator per radian-second of flight path error.
const PATH_INTEGRAL: f64 = 0.3;

fn angle(value: f64) -> f64 {
    (value + PI).rem_euclid(TAU) - PI
}

/// The 1 G stall speed at the aircraft's altitude, ft/s (900 without a
/// 1 G envelope row there).
fn stall_speed(s: &State) -> f64 {
    s.model()
        .configuration()
        .aerodynamics
        .envelopes
        .iter()
        .find(|e| e.g == 1)
        .and_then(|e| e.speeds(s.position[1]))
        .map_or(900., |p| p.0)
}

/// Ground speed along the ground, ft/s.
fn ground_speed(s: &State) -> f64 {
    s.velocity[0].hypot(s.velocity[2])
}

/// Whether the heading and waypoint modes have lost the speed they fly at
/// on this aircraft.
fn too_slow(s: &State) -> bool {
    match Airframe::of(s) {
        Airframe::Conventional => false,
        Airframe::Jet => s.speed < JET_RELEASE_STALL_SHARE * stall_speed(s),
        Airframe::Rotorcraft => ground_speed(s) < ROTORCRAFT_RELEASE_KT * KT,
    }
}

/// Whether the heading and waypoint modes fly the collective on this
/// aircraft now: on the helicopters, and on the V-22 with its nacelles at 75
/// degrees or more (hover hold always does). Elsewhere the collective, the
/// V-22's power lever, is the pilot's, as the throttle is.
fn collective_coupled(s: &State) -> bool {
    Airframe::of(s) == Airframe::Rotorcraft
        && s.model().powered_lift().is_some_and(|lift| {
            lift.kind == LiftKind::Helicopter || s.nacelle_degrees() >= HOVER_NACELLE_DEGREES - 1e-9
        })
}

/// One attitude loop: the stick (or pedal) travel that drives `error`, rad,
/// to zero against the body `rate`, normalized by the aircraft's full-stick
/// hover `full_rate`, rad/s, plus its integrator.
fn attitude_loop(
    error: f64,
    rate: f64,
    full_rate: f64,
    bandwidth: f64,
    window: f64,
    feedforward: f64,
    integrator: &mut f64,
) -> f64 {
    let full_rate = full_rate.max(1e-3);
    let limit = ATTITUDE_RATE_SHARE * full_rate;
    let demand = (bandwidth * error).clamp(-limit, limit);
    if error.abs() < window {
        *integrator = (*integrator + ATTITUDE_INTEGRAL * error * DT).clamp(-1., 1.);
    }
    (ATTITUDE_GAIN * (demand - rate) / full_rate + *integrator + feedforward).clamp(-1., 1.)
}

/// The stick the stability level's attitude retention takes off the
/// autopilot's at the attitude it commands, [pitch, bank] in travel: the
/// Attitude level's hold about its reference, or the Easy flight physics
/// cheat's weaker one. The autopilot adds it back, as a pilot holding an
/// attitude away from the trim would, so its loops fly alike at every level.
fn retention(s: &State, command: [f64; 2]) -> [f64; 2] {
    use crate::flight::powered::sas;
    use tore_input::StabilityLevel;
    let (Some(lift), Some(level)) = (s.model().powered_lift(), s.stability_in_effect()) else {
        return [0.; 2];
    };
    let reference = s.lift_controls.aids.attitude_reference;
    let spans = sas::ATTITUDE_PER_TRAVEL_DEGREES.map(f64::to_radians);
    let hold = [
        (command[0] - reference[0]) / spans[0],
        angle(command[1] - reference[1]) / spans[1],
    ];
    if level == StabilityLevel::Attitude {
        hold.map(|h| h.clamp(-sas::ATTITUDE_AUTHORITY, sas::ATTITUDE_AUTHORITY))
    } else if s.easy_retention(&lift, level) {
        let weight = sas::EASY_RETENTION_GAIN * s.lift_controls.hover_fraction(lift.kind);
        hold.map(|h| {
            (weight * h).clamp(
                -sas::EASY_RETENTION_AUTHORITY,
                sas::EASY_RETENTION_AUTHORITY,
            )
        })
    } else {
        [0.; 2]
    }
}

impl Autopilot {
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn label(&self) -> String {
        match self.mode {
            Mode::Off => String::new(),
            Mode::Heading => "HDG ALT".into(),
            Mode::Waypoint => self
                .target
                .map_or_else(|| "WP --".into(), |target| format!("WP {}", target.number)),
            Mode::Hover => "HOVER".into(),
        }
    }
    /// Hover hold's ground point, world X/Z ft, once the drift is braked.
    pub fn hover_point(&self) -> Option<[f64; 2]> {
        (self.mode == Mode::Hover)
            .then_some(self.loops.point)
            .flatten()
    }
    /// Future route selection supplies its waypoint number and world X/Z in feet, or None.
    /// This changes guidance immediately without recapturing the held altitude.
    pub fn set_navigation_target(&mut self, target: Option<NavigationTarget>) {
        self.target = target.filter(|p| p.position.iter().all(|v| v.is_finite()));
    }
    pub fn disengage(&mut self) {
        self.mode = Mode::Off;
    }
    pub(crate) fn select(
        &mut self,
        switch: Switch,
        setting: Option<bool>,
        heading: f64,
        altitude: f64,
    ) {
        let wanted = match switch {
            Switch::Autopilot => Mode::Heading,
            Switch::WaypointAutopilot => Mode::Waypoint,
            Switch::HoverHold => Mode::Hover,
            _ => return,
        };
        if !setting.unwrap_or(self.mode != wanted) {
            if self.mode == wanted {
                self.disengage();
            }
            return;
        }
        if self.mode == wanted {
            return;
        }
        // From off, or across hover hold and the A modes, the mode captures
        // anew; between the A modes it keeps the capture.
        if self.mode == Mode::Off || self.mode == Mode::Hover || wanted == Mode::Hover {
            self.heading = heading;
            self.altitude = altitude;
            self.loops = Loops::default();
        }
        self.mode = wanted;
    }
    /// Hover hold takes a cyclic trim key as a nudge of the point it holds:
    /// a tap (2 percent of travel) moves it 10 ft forward, back, left or
    /// right of the heading (design 5.4). Returns whether `command` was
    /// taken; the trim does not move. Pedal trim and the other trim
    /// commands pass to the trim, except Trim set, which hover hold and the
    /// rotorcraft A modes take and drop: its latch would hide their stick.
    pub(crate) fn intercept(&mut self, command: PilotCommand, s: &State) -> bool {
        let rotorcraft = Airframe::of(s) == Airframe::Rotorcraft;
        match command {
            PilotCommand::Lift(LiftCommand::TrimSet) => rotorcraft && self.mode != Mode::Off,
            PilotCommand::Lift(LiftCommand::TrimAdjust(TrimAxis::Pitch, amount))
                if matches!(self.mode, Mode::Heading | Mode::Waypoint)
                    && self.loops.coupled
                    && collective_coupled(s) =>
            {
                // Forward trim (negative pitch) is faster.
                if amount.is_finite() {
                    let knots = -amount / tore_input::trim_keys::TAP * A_NUDGE_KT;
                    self.loops.speed = (self.loops.speed + knots * KT).max(HOVER_SPEED_KT * KT);
                }
                true
            }
            PilotCommand::Lift(LiftCommand::TrimAdjust(axis, amount))
                if self.mode == Mode::Hover && axis != TrimAxis::Pedal =>
            {
                if amount.is_finite()
                    && let Some(point) = &mut self.loops.point
                {
                    let feet = amount / tore_input::trim_keys::TAP * HOVER_NUDGE_FT;
                    let (sin, cos) = s.yaw.sin_cos();
                    // Forward trim (nose down) is negative pitch.
                    let [x, z] = if axis == TrimAxis::Pitch {
                        [-feet * sin, -feet * cos]
                    } else {
                        [feet * cos, -feet * sin]
                    };
                    point[0] += x;
                    point[1] += z;
                }
                true
            }
            _ => false,
        }
    }
    fn bearing(&self, position: [f64; 3]) -> f64 {
        if self.mode == Mode::Waypoint
            && let Some(NavigationTarget {
                position: [x, z], ..
            }) = self.target
        {
            let dx = x - position[0];
            let dz = z - position[2];
            if dx.hypot(dz) > 1. / 0.3048 {
                return dx.atan2(dz);
            }
        }
        self.heading
    }
    /// [`Self::apply_over`] over still terrain at height `ground`.
    #[cfg(test)]
    pub(crate) fn apply(
        &mut self,
        s: &State,
        ground: f64,
        input: &mut PilotInput,
    ) -> Option<Release> {
        self.apply_over(s, &crate::research::Surface::terrain(ground), input)
    }
    /// One tick of the engaged mode over the `surface` under the aircraft
    /// (its height, and the wind the rotorcraft modes read): lets go when the
    /// pilot or the aircraft asks it to, else rewrites `input` with the
    /// stick, pedals and, on the rotorcraft, the collective the mode flies.
    /// Returns why it let go, if it did.
    pub(crate) fn apply_over(
        &mut self,
        s: &State,
        surface: &crate::research::Surface,
        input: &mut PilotInput,
    ) -> Option<Release> {
        let ground = surface.height;
        let c = s.model().configuration();
        let release = if s.crashed
            || s.position[1] <= ground + c.equipment.ground_clearance_ft
            || (self.mode == Mode::Hover && s.weight_on_wheels())
        {
            Some(Release::Ground)
        } else if input.pitch.abs().max(input.roll.abs()).max(input.yaw.abs()) > 0.15 {
            Some(Release::PilotOverride)
        } else if self.mode == Mode::Hover {
            self.coupled_release(s, input)
        } else if self.mode != Mode::Off {
            if too_slow(s) {
                Some(Release::TooSlow)
            } else if collective_coupled(s) {
                self.coupled_release(s, input)
            } else {
                None
            }
        } else {
            None
        };
        if release.is_some() {
            self.disengage();
        }
        match (self.mode, Airframe::of(s)) {
            (Mode::Off, _) => {}
            (Mode::Hover, _) => self.hover(s, ground, input),
            (_, Airframe::Rotorcraft) => self.rotorcraft(s, surface.wind, input),
            _ => self.conventional(s, input),
        }
        release
    }
    /// The heading and waypoint modes on conventional aircraft and the
    /// vectoring jets: a bank command through the roll rate and a climb
    /// command through the load factor (the module documentation's law).
    fn conventional(&self, s: &State, input: &mut PilotInput) {
        let c = s.model().configuration();
        let bank = (angle(self.bearing(s.position) - s.yaw) * 1.5)
            .clamp(-30_f64.to_radians(), 30_f64.to_radians());
        let roll_rate = angle(bank - s.bank);
        let stall = stall_speed(s);
        let authority = (s.speed / stall.max(1.)).powi(2).clamp(0., 1.);
        let roll_limit = if let Some(p) = c.controls {
            let axis = if s.research.is_some() {
                p.hybrid_roll.unwrap_or(p.roll)
            } else {
                p.roll
            };
            let bound = if roll_rate < 0. {
                -f64::from(axis.minimum)
            } else {
                f64::from(axis.maximum)
            };
            bound.to_radians() * (s.speed / (2. * stall.max(1.))).clamp(0., 1.)
        } else if s.research.is_some() {
            c.aerodynamics.roll_limit_rad_per_second.clamp(0.1, 6.) * authority
        } else {
            c.tuning.legacy_roll_limit_rad_per_second * authority
        };
        input.roll = (roll_rate / roll_limit.max(0.01)).clamp(-1., 1.);
        let climb = ((self.altitude - s.position[1]) / 10.).clamp(-20. / 0.3048, 20. / 0.3048);
        let desired_g = (1. + (climb - s.vertical_speed) / (3. * 32.174)) / s.bank.cos().max(0.5);
        let (mut low, mut high) = (-1_f64, 1_f64);
        for e in &c.aerodynamics.envelopes {
            if let Some((min, max)) = e.speeds(s.position[1])
                && s.speed >= min
                && s.speed <= max
            {
                low = low.min(e.g as f64);
                high = high.max(e.g as f64);
            }
        }
        // The flight model's fast-side hold, so the autopilot scales its
        // stick for the pull the aircraft actually has near top speed.
        if s.research.is_some()
            && let Some(hold) =
                crate::flight::fast_side_hold(&c.aerodynamics.envelopes, s.position[1], s.speed)
        {
            high = high.max(hold.g);
        }
        let loading = 1.
            + (s.fuel + s.carried_lbs()) / c.mass.empty_lbs
                * c.aerodynamics.loaded_elevator_percent
                / 100.;
        low /= loading;
        high /= loading;
        let delta = desired_g / authority.max(0.01) - 1.;
        input.pitch = (delta
            / if delta > 0. {
                (high - 1.).max(0.01)
            } else {
                (1. - low).max(0.01)
            })
        .clamp(-1., 1.);
        input.yaw = 0.;
    }

    /// Why hover hold, or a rotorcraft A mode flying the collective, lets go
    /// this tick on the pilot's `input` or the aircraft's state, besides the
    /// ground and the stick (design 5.4): any collective input, an engine
    /// failure or the loss of the hydraulics.
    fn coupled_release(&mut self, s: &State, input: &PilotInput) -> Option<Release> {
        let collective_command = input.commands.iter().any(|command| {
            matches!(
                command,
                PilotCommand::SetAxis(FlightAxis::Collective, _)
                    | PilotCommand::AdjustAxis(FlightAxis::Collective, _)
                    | PilotCommand::Throttle(_)
                    | PilotCommand::AdjustThrottle(_)
            )
        });
        let lever = input.collective.or(input.throttle);
        let moved = match (lever, self.loops.lever) {
            (Some(now), Some(then)) => (now - then).abs() > HOVER_LEVER_MOVE,
            _ => false,
        };
        if lever.is_some() && self.loops.lever.is_none() {
            self.loops.lever = lever;
        }
        if collective_command || moved || input.collective_rate != 0. || input.throttle_rate != 0. {
            return Some(Release::PilotCollective);
        }
        let starved = s.fuel + s.systems.external_lbs() <= 0. && !s.cheats.unlimited_fuel;
        if !s.engine || s.systems.power_available() <= 0. || starved {
            return Some(Release::EngineFailure);
        }
        if s.systems.fluids.hydraulic <= 0. {
            return Some(Release::HydraulicsLost);
        }
        None
    }
    /// The full-stick hover rates, [pitch, roll, yaw], rad/s.
    fn full_rates(s: &State) -> [f64; 3] {
        let rates = s.model().powered_lift().map_or([45., 45., 45.], |lift| {
            lift.targets.hover_rates_degrees_per_second
        });
        [rates[1], rates[0], rates[2]].map(f64::to_radians)
    }
    /// Hover hold (design 5.4): brakes the drift, then holds the point it
    /// stopped at, the heading at engagement and the wheel height above the
    /// ground at engagement (at least 10 ft), through the cyclic, pedals and
    /// collective. A cascade per axis: position to ground velocity to tilt
    /// to attitude to stick, heading to yaw rate to pedals, height to climb
    /// to collective, each with an integrator for the steady part (the
    /// hover attitude, the wind, the cyclic and pedal trim, the collective).
    fn hover(&mut self, s: &State, ground: f64, input: &mut PilotInput) {
        let clearance = s.model().configuration().equipment.ground_clearance_ft;
        let height = s.position[1] - ground - clearance;
        let loops = &mut self.loops;
        if !loops.started {
            loops.started = true;
            loops.height = height.max(HOVER_MIN_HEIGHT_FT);
            loops.collective = s.lift_controls.collective;
        }
        let velocity = [s.velocity[0], s.velocity[2]];
        if loops.point.is_none() && velocity[0].hypot(velocity[1]) < HOVER_CAPTURE_KT * KT {
            loops.point = Some([s.position[0], s.position[2]]);
        }
        let mut wanted = loops.point.map_or([0.; 2], |point| {
            [
                POSITION_GAIN * (point[0] - s.position[0]),
                POSITION_GAIN * (point[1] - s.position[2]),
            ]
        });
        let closing = wanted[0].hypot(wanted[1]);
        if closing > POSITION_SPEED_FPS {
            wanted = wanted.map(|v| v * POSITION_SPEED_FPS / closing);
        }
        let error = [wanted[0] - velocity[0], wanted[1] - velocity[1]];
        let (sin, cos) = s.yaw.sin_cos();
        // Along the heading and to its right.
        let along = [
            error[0] * sin + error[1] * cos,
            error[0] * cos - error[1] * sin,
        ];
        let limit = HOVER_TILT_DEGREES.to_radians();
        let tilt: [f64; 2] = std::array::from_fn(|axis| {
            let proportional = VELOCITY_GAIN * along[axis] / GRAVITY;
            // Integrate only while the command is inside its limit, so
            // braking a fast drift does not wind the integrator up.
            if (proportional + loops.tilt[axis]).abs() < limit {
                loops.tilt[axis] =
                    (loops.tilt[axis] + VELOCITY_INTEGRAL * along[axis] * DT).clamp(-limit, limit);
            }
            (proportional + loops.tilt[axis]).clamp(-limit, limit)
        });
        let retained = retention(s, [-tilt[0], tilt[1]]);
        let [p, q, r] = s.lift_controls.body_rates;
        let full = Self::full_rates(s);
        input.pitch = attitude_loop(
            -tilt[0] - s.pitch,
            q,
            full[0],
            ATTITUDE_BANDWIDTH,
            ATTITUDE_WINDOW,
            retained[0],
            &mut loops.stick[0],
        );
        input.roll = attitude_loop(
            angle(tilt[1] - s.bank),
            p,
            full[1],
            ATTITUDE_BANDWIDTH,
            ATTITUDE_WINDOW,
            retained[1],
            &mut loops.stick[1],
        );
        input.yaw = attitude_loop(
            angle(self.heading - s.yaw),
            r,
            full[2],
            HEADING_BANDWIDTH,
            HEADING_WINDOW,
            0.,
            &mut loops.stick[2],
        );
        let climb_limit = HOVER_CLIMB_FPM / 60.;
        let climb = (HEIGHT_GAIN * (loops.height - height)).clamp(-climb_limit, climb_limit);
        let climb_error = climb - s.vertical_speed;
        loops.collective = (loops.collective + CLIMB_INTEGRAL * climb_error * DT).clamp(0., 1.);
        input.collective = Some((loops.collective + CLIMB_GAIN * climb_error).clamp(0., 1.));
        input.collective_rate = 0.;
    }
    /// The heading and waypoint modes on the helicopters and the V-22: the
    /// fixed-wing law's bank command and its climb command toward the held
    /// altitude (10 s, here within 1,000 ft/min), flown through attitude
    /// loops on the cyclic. While the mode flies the collective
    /// ([`collective_coupled`]) the climb goes to the collective and the
    /// pitch holds the ground speed along the heading taken at engagement
    /// (at least 40 kt; the pitch trim keys move it 2 kt a tap). Otherwise
    /// the climb becomes a pitch attitude by the flight path error. The
    /// pedals keep the turn coordinated, against the sideslip through the
    /// air, at every stability level.
    fn rotorcraft(&mut self, s: &State, wind: [f64; 3], input: &mut PilotInput) {
        let bearing = self.bearing(s.position);
        let coupled = collective_coupled(s);
        let loops = &mut self.loops;
        let pitch_limit = ROTORCRAFT_PITCH_DEGREES.to_radians();
        if !loops.started {
            loops.started = true;
            loops.tilt[0] = s.pitch.clamp(-pitch_limit, pitch_limit);
        }
        let (sin, cos) = s.yaw.sin_cos();
        let along = s.velocity[0] * sin + s.velocity[2] * cos;
        if coupled && !loops.coupled {
            // The collective and the speed are held from here.
            loops.coupled = true;
            loops.collective = s.lift_controls.collective;
            loops.speed = along.max(HOVER_SPEED_KT * KT);
        }
        loops.coupled = coupled;
        let bank = (angle(bearing - s.yaw) * 1.5).clamp(-30_f64.to_radians(), 30_f64.to_radians());
        let climb_limit = ROTORCRAFT_CLIMB_FPM / 60.;
        let climb = ((self.altitude - s.position[1]) / 10.).clamp(-climb_limit, climb_limit);
        let pitch = if coupled {
            // Speed on the cyclic, height on the collective.
            let error = loops.speed - along;
            let proportional = -VELOCITY_GAIN * error / GRAVITY;
            if (proportional + loops.tilt[0]).abs() < pitch_limit {
                loops.tilt[0] = (loops.tilt[0] - VELOCITY_INTEGRAL * error * DT)
                    .clamp(-pitch_limit, pitch_limit);
            }
            let climb_error = climb - s.vertical_speed;
            loops.collective = (loops.collective + CLIMB_INTEGRAL * climb_error * DT).clamp(0., 1.);
            input.collective = Some((loops.collective + CLIMB_GAIN * climb_error).clamp(0., 1.));
            input.collective_rate = 0.;
            (proportional + loops.tilt[0]).clamp(-pitch_limit, pitch_limit)
        } else {
            // Wingborne: the climb on the pitch, the power lever the pilot's.
            let path_error = (climb - s.vertical_speed) / s.speed.max(HOVER_SPEED_KT * KT);
            loops.tilt[0] =
                (loops.tilt[0] + PATH_INTEGRAL * path_error * DT).clamp(-pitch_limit, pitch_limit);
            (loops.tilt[0] + PATH_GAIN * path_error).clamp(-pitch_limit, pitch_limit)
        };
        let retained = retention(s, [pitch, bank]);
        let [p, q, r] = s.lift_controls.body_rates;
        let full = Self::full_rates(s);
        input.pitch = attitude_loop(
            pitch - s.pitch,
            q,
            full[0],
            ATTITUDE_BANDWIDTH,
            ATTITUDE_WINDOW,
            retained[0],
            &mut loops.stick[0],
        );
        input.roll = attitude_loop(
            angle(bank - s.bank),
            p,
            full[1],
            ATTITUDE_BANDWIDTH,
            ATTITUDE_WINDOW,
            retained[1],
            &mut loops.stick[1],
        );
        // The pedals keep the turn coordinated: the yaw rate of the turn,
        // and the sideslip through the air steered out.
        let air = [s.velocity[0] - wind[0], s.velocity[2] - wind[2]];
        let forward = air[0] * sin + air[1] * cos;
        let right = air[0] * cos - air[1] * sin;
        let sideslip = right.atan2(forward.max(1.));
        let turn = GRAVITY * s.bank.clamp(-1.2, 1.2).tan() / forward.max(HOVER_SPEED_KT * KT);
        input.yaw = attitude_loop(
            sideslip,
            r - turn,
            full[2],
            SIDESLIP_BANDWIDTH,
            HEADING_WINDOW,
            0.,
            &mut loops.stick[2],
        );
    }
}

impl State {
    /// Why the autopilot `switch` cannot engage now, as the message the
    /// pilot reads, or None (turning a mode off is never refused).
    pub(crate) fn autopilot_refusal(
        &self,
        switch: Switch,
        setting: Option<bool>,
    ) -> Option<&'static str> {
        let mode = match switch {
            Switch::Autopilot => Mode::Heading,
            Switch::WaypointAutopilot => Mode::Waypoint,
            Switch::HoverHold => Mode::Hover,
            _ => return None,
        };
        let current = self.autopilot.mode();
        if !setting.unwrap_or(current != mode) || current == mode {
            return None;
        }
        let airframe = Airframe::of(self);
        if mode == Mode::Hover {
            let lift = self.model().powered_lift().map(|lift| lift.kind);
            if airframe != Airframe::Rotorcraft {
                return Some("Hover hold is not available on this aircraft");
            }
            if lift == Some(LiftKind::Tiltrotor)
                && self.nacelle_degrees() < HOVER_NACELLE_DEGREES - 1e-9
            {
                return Some("Hover hold needs the nacelles at 75 degrees or more");
            }
            if self.weight_on_wheels() {
                return Some("Hover hold is not available on the ground");
            }
            if ground_speed(self) >= HOVER_SPEED_KT * KT {
                return Some("Hover hold needs less than 40 knots");
            }
            let starved =
                self.fuel + self.systems.external_lbs() <= 0. && !self.cheats.unlimited_fuel;
            if !self.engine || self.systems.power_available() <= 0. || starved {
                return Some("Hover hold needs engine power");
            }
            if self.systems.fluids.hydraulic <= 0. {
                return Some("Hover hold needs hydraulic power");
            }
            return None;
        }
        match airframe {
            Airframe::Conventional => None,
            Airframe::Jet => {
                (self.speed < stall_speed(self)).then_some("Autopilot needs flying speed")
            }
            Airframe::Rotorcraft => (ground_speed(self) < HOVER_SPEED_KT * KT)
                .then_some("Autopilot needs 40 knots; Ctrl+Alt+A holds a hover"),
        }
    }
}

crate::flight::exact::exact_enum!(Mode {
    Off = 0,
    Heading = 1,
    Waypoint = 2,
    Hover = 3,
});
crate::flight::exact::exact_struct!(NavigationTarget { number, position });
crate::flight::exact::exact_struct!(Loops {
    started,
    point,
    height,
    lever,
    tilt,
    stick,
    collective,
    coupled,
    speed,
});
crate::flight::exact::exact_struct!(Autopilot {
    mode,
    heading,
    altitude,
    target,
    loops,
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight::{PilotCommand, integration_tests::profile};
    fn state(hybrid: bool) -> State {
        let mut s = State::new(&profile(), [0., 10000., 0.]).unwrap();
        if hybrid {
            s.enable_research(42).unwrap();
        }
        s
    }
    fn toggle(s: &mut State, switch: Switch) {
        s.step(
            &PilotInput {
                commands: vec![PilotCommand::Toggle(switch)],
                ..Default::default()
            },
            |_, _| 0.,
        );
    }
    #[test]
    fn mode_capture_fallback_override_and_contact() {
        let mut s = state(true);
        toggle(&mut s, Switch::Autopilot);
        let capture = (s.autopilot.heading, s.autopilot.altitude);
        s.yaw = 1.;
        s.position[1] += 100.;
        toggle(&mut s, Switch::WaypointAutopilot);
        assert_eq!((s.autopilot.heading, s.autopilot.altitude), capture);
        assert_eq!(s.autopilot.bearing(s.position), capture.0);
        assert_eq!(s.autopilot.label(), "WP --");
        s.autopilot.set_navigation_target(Some(NavigationTarget {
            number: 1,
            position: [s.position[0], s.position[2]],
        }));
        assert_eq!(s.autopilot.bearing(s.position), capture.0);
        s.autopilot.set_navigation_target(Some(NavigationTarget {
            number: 1,
            position: [f64::NAN, 1.],
        }));
        assert_eq!(s.autopilot.target, None);
        toggle(&mut s, Switch::WaypointAutopilot);
        assert_eq!(s.autopilot.mode(), Mode::Off);
        for axis in 0..3 {
            toggle(&mut s, Switch::Autopilot);
            let mut input = PilotInput::default();
            match axis {
                0 => input.pitch = 0.15,
                1 => input.roll = -0.15,
                _ => input.yaw = 0.15,
            }
            s.step(&input, |_, _| 0.);
            assert_eq!(s.autopilot.mode(), Mode::Heading);
            input.pitch *= 1.01;
            input.roll *= 1.01;
            input.yaw *= 1.01;
            s.step(&input, |_, _| 0.);
            assert_eq!(s.autopilot.mode(), Mode::Off);
        }
        toggle(&mut s, Switch::Autopilot);
        s.step(&PilotInput::default(), |_, _| 20000.);
        assert_eq!(s.autopilot.mode(), Mode::Off);
    }
    #[test]
    fn waypoint_updates_shortest_turn_and_manual_equipment() {
        let mut s = state(true);
        s.yaw = 359_f64.to_radians();
        s.command(PilotCommand::Toggle(Switch::WaypointAutopilot));
        let target = |degrees: f64| {
            let radians = degrees.to_radians();
            [radians.sin() * 100000., radians.cos() * 100000.]
        };
        s.autopilot.set_navigation_target(Some(NavigationTarget {
            number: 7,
            position: target(1.),
        }));
        assert_eq!(s.autopilot.label(), "WP 7");
        let mut ap = s.autopilot.clone();
        let mut input = PilotInput::default();
        ap.apply(&s, 0., &mut input);
        assert!(input.roll > 0. && input.roll < 0.1);
        ap.set_navigation_target(Some(NavigationTarget {
            number: 8,
            position: target(357.),
        }));
        ap.apply(&s, 0., &mut PilotInput::default());
        let mut opposite = PilotInput::default();
        ap.apply(&s, 0., &mut opposite);
        assert!(opposite.roll < 0. && opposite.roll > -0.1);
        s.step(
            &PilotInput {
                throttle: Some(0.5),
                commands: vec![PilotCommand::Toggle(Switch::Gear)],
                ..Default::default()
            },
            |_, _| 0.,
        );
        assert_eq!(s.throttle, 0.5);
        assert!(s.gear_down);
        assert_eq!(s.autopilot.mode(), Mode::Waypoint);
        s.crashed = true;
        s.step(&PilotInput::default(), |_, _| 0.);
        assert_eq!(s.autopilot.mode(), Mode::Off);
    }

    #[test]
    fn mode_tape_replays_identically() {
        use tore_input::recording;
        let frames: Vec<_> = (0..1200)
            .map(|tick| PilotInput {
                commands: match tick {
                    0 | 900 => vec![PilotCommand::Toggle(Switch::Autopilot)],
                    400 | 700 => vec![PilotCommand::Toggle(Switch::WaypointAutopilot)],
                    _ => vec![],
                },
                ..Default::default()
            })
            .collect();
        let mut tape = format!("{}\n", recording::HEADER).into_bytes();
        let mut live = state(true);
        for (i, frame) in frames.iter().enumerate() {
            recording::write_frame(&mut tape, i as u64 + 1, frame).unwrap();
            live.step(frame, |_, _| 0.);
        }
        let mut replay = state(true);
        for frame in recording::read(tape.as_slice()).unwrap() {
            replay.step(&frame, |_, _| 0.);
        }
        assert_eq!(live, replay);
        assert_eq!(state(true).autopilot.mode(), Mode::Off);
    }

    #[test]
    fn holds_and_turns_closed_loop_in_both_adapters() {
        for hybrid in [false, true] {
            for nav in [false, true] {
                let mut s = state(hybrid);
                s.command(PilotCommand::Toggle(if nav {
                    Switch::WaypointAutopilot
                } else {
                    Switch::Autopilot
                }));
                let heading = s.yaw;
                if nav {
                    s.autopilot.set_navigation_target(Some(NavigationTarget {
                        number: 1,
                        position: [100000., 100000.],
                    }));
                }
                s.bank = 20_f64.to_radians();
                s.position[1] -= 100.;
                for _ in 0..120 * 120 {
                    s.step(&PilotInput::default(), |_, _| 0.);
                }
                let error = angle(s.autopilot.bearing(s.position) - s.yaw).to_degrees();
                assert!(
                    error.abs() < 3.,
                    "hybrid={hybrid} nav={nav} heading error={error}"
                );
                assert!(
                    (s.position[1] - 10000.).abs() < 100.,
                    "hybrid={hybrid} nav={nav} altitude={}",
                    s.position[1]
                );
                assert!(s.bank.abs() < 5_f64.to_radians());
                assert!(!s.crashed);
                assert!(s.stall_alert(0.).is_none());
                assert_eq!(s.throttle, 0.7);
                if nav {
                    assert!(angle(s.yaw - heading).abs() > 0.2);
                }
            }
        }
    }
}
