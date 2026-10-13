//! The rotorcraft control laws, as pure functions of the flight state, an
//! explicit commanded target and the loops' memory (slice H1 of the
//! helicopter and VTOL AI pass). The player's hover hold and the coupled
//! heading and waypoint modes ([`super::Autopilot`]) call them with the
//! point, height, heading and speed they captured; the AI's rotor adapter
//! (`crate::ai::rotor_adapter`) calls them with the targets it commands.
//!
//! - [`hover`]: hover hold's cascade. Position to ground velocity to tilt
//!   to attitude to stick, heading to yaw rate to pedals, height to climb to
//!   collective, each with an integrator for the steady part.
//! - [`forward`]: the coupled forward-flight law. A bank and a climb command
//!   flown through attitude loops on the cyclic; while coupled, the climb on
//!   the collective and the ground speed along the heading on the pitch,
//!   otherwise the climb on the pitch (the V-22 with its nacelles down).
//!   The pedals keep the turn coordinated.
//!
//! Every gain is fitted (agent decisions, 2026-10-09, slice P9), tuned on
//! the AH-64, Mi-24, CH-47 and V-22; the extraction (2026-10-12) moved them
//! here unchanged, in the same order of floating-point operations, so the
//! player's modes fly bit for bit as before.
use super::{GRAVITY, HOVER_SPEED_KT, KT, Loops};
use crate::flight::{DT, PilotInput, State};
use crate::models::FlightModel;
use std::f64::consts::{PI, TAU};

/// Attitude loops: the commanded rate per radian of error, 1/s.
pub(crate) const ATTITUDE_BANDWIDTH: f64 = 3.;
/// Attitude loops: the commanded rate never exceeds this share of the
/// aircraft's full-stick hover rate.
const ATTITUDE_RATE_SHARE: f64 = 0.5;
/// Attitude loops: stick travel per unit of rate error, as a share of the
/// full-stick hover rate.
const ATTITUDE_GAIN: f64 = 3.;
/// Attitude loops integrate only within this error, rad, so a large
/// attitude change does not wind them up.
pub(crate) const ATTITUDE_WINDOW: f64 = 0.15;
/// The heading and sideslip loops integrate within this error, rad.
pub(crate) const HEADING_WINDOW: f64 = 0.3;
/// Attitude loops: integrator, travel per radian-second of error.
const ATTITUDE_INTEGRAL: f64 = 0.6;
/// Heading loop: commanded yaw rate per radian of heading error, 1/s.
pub(crate) const HEADING_BANDWIDTH: f64 = 2.;
/// The coupled forward law: yaw rate commanded per radian of sideslip, 1/s.
const SIDESLIP_BANDWIDTH: f64 = 1.;
/// Hover: ground velocity commanded per foot from the point, 1/s.
pub const POSITION_GAIN: f64 = 0.15;
/// Hover hold: the ground velocity it closes on the point at most, ft/s.
pub const POSITION_SPEED_FPS: f64 = 10.;
/// Hover's velocity loops and the coupled speed loop: acceleration
/// commanded per ft/s of error, 1/s.
pub(crate) const VELOCITY_GAIN: f64 = 0.8;
/// The same loops' tilt integrator, rad per foot of velocity error.
pub(crate) const VELOCITY_INTEGRAL: f64 = 0.002;
/// Hover: climb commanded per foot of height error, 1/s.
pub const HEIGHT_GAIN: f64 = 0.25;
/// Hover and the coupled law: collective per ft/s of climb error.
pub(crate) const CLIMB_GAIN: f64 = 0.08;
/// The same loops' collective integrator per foot of climb error.
pub(crate) const CLIMB_INTEGRAL: f64 = 0.02;
/// The uncoupled law: pitch per radian of flight path error.
const PATH_GAIN: f64 = 1.;
/// The same law's pitch integrator per radian-second of flight path error.
const PATH_INTEGRAL: f64 = 0.3;

/// What [`hover`] holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoverTarget {
    /// The ground point held, world X/Z ft; none brakes the drift to a stop
    /// (hover hold before its capture).
    pub point: Option<[f64; 2]>,
    /// A ground velocity flown on top of the point's, world X/Z ft/s: slow
    /// flight and air taxi. None adds nothing.
    pub velocity: Option<[f64; 2]>,
    /// The wheel height above the ground under the aircraft, ft.
    pub height: f64,
    /// The heading held, rad.
    pub heading: f64,
    /// The climb and the sink the height loop may command, ft/s, both
    /// positive.
    pub climb_limit: f64,
    pub descent_limit: f64,
    /// The ground speed the cascade may command toward the point, ft/s.
    pub close_speed: f64,
    /// The pitch and bank the cascade may command, rad.
    pub tilt_limit: f64,
}

/// What [`forward`] flies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForwardTarget {
    /// The bank commanded, rad.
    pub bank: f64,
    /// The climb commanded, ft/s (already within the caller's limits).
    pub climb: f64,
    /// While coupled: the ground speed along the heading held, ft/s.
    pub speed: f64,
    /// The pitch attitude the law may command, rad.
    pub pitch_limit: f64,
    /// Climb on the collective and speed on the pitch (true), or climb on
    /// the pitch, the collective left to the caller (false).
    pub coupled: bool,
}

pub(crate) fn angle(value: f64) -> f64 {
    (value + PI).rem_euclid(TAU) - PI
}

/// One attitude loop: the stick (or pedal) travel that drives `error`, rad,
/// to zero against the body `rate`, normalized by the aircraft's full-stick
/// hover `full_rate`, rad/s, plus its integrator.
pub(crate) fn attitude_loop(
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
/// cheat's weaker one. The laws add it back, as a pilot holding an
/// attitude away from the trim would, so they fly alike at every level.
pub(crate) fn retention(s: &State, command: [f64; 2]) -> [f64; 2] {
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

/// The full-stick hover rates, [pitch, roll, yaw], rad/s.
pub(crate) fn full_rates(s: &State) -> [f64; 3] {
    let rates = s.model().powered_lift().map_or([45., 45., 45.], |lift| {
        lift.targets.hover_rates_degrees_per_second
    });
    [rates[1], rates[0], rates[2]].map(f64::to_radians)
}

/// Ground speed along the heading, ft/s.
pub fn speed_along(s: &State) -> f64 {
    let (sin, cos) = s.yaw.sin_cos();
    s.velocity[0] * sin + s.velocity[2] * cos
}

/// One tick of the hover cascade toward `target` over terrain at `ground`
/// ft: rewrites the stick, pedals and collective of `input`.
pub fn hover(
    s: &State,
    ground: f64,
    target: &HoverTarget,
    loops: &mut Loops,
    input: &mut PilotInput,
) {
    let clearance = s.model().configuration().equipment.ground_clearance_ft;
    let height = s.position[1] - ground - clearance;
    if !loops.started {
        loops.started = true;
        loops.collective = s.lift_controls.collective;
    }
    let velocity = [s.velocity[0], s.velocity[2]];
    let mut wanted = target.point.map_or([0.; 2], |point| {
        [
            POSITION_GAIN * (point[0] - s.position[0]),
            POSITION_GAIN * (point[1] - s.position[2]),
        ]
    });
    if let Some(drift) = target.velocity {
        wanted = [wanted[0] + drift[0], wanted[1] + drift[1]];
    }
    let closing = wanted[0].hypot(wanted[1]);
    if closing > target.close_speed {
        wanted = wanted.map(|v| v * target.close_speed / closing);
    }
    let error = [wanted[0] - velocity[0], wanted[1] - velocity[1]];
    let (sin, cos) = s.yaw.sin_cos();
    // Along the heading and to its right.
    let along = [
        error[0] * sin + error[1] * cos,
        error[0] * cos - error[1] * sin,
    ];
    let limit = target.tilt_limit;
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
    let full = full_rates(s);
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
        angle(target.heading - s.yaw),
        r,
        full[2],
        HEADING_BANDWIDTH,
        HEADING_WINDOW,
        0.,
        &mut loops.stick[2],
    );
    let climb =
        (HEIGHT_GAIN * (target.height - height)).clamp(-target.descent_limit, target.climb_limit);
    let climb_error = climb - s.vertical_speed;
    loops.collective = (loops.collective + CLIMB_INTEGRAL * climb_error * DT).clamp(0., 1.);
    input.collective = Some((loops.collective + CLIMB_GAIN * climb_error).clamp(0., 1.));
    input.collective_rate = 0.;
}

/// One tick of the forward-flight law toward `target` in the `wind` (world
/// ft/s): rewrites the stick and pedals of `input`, and while coupled its
/// collective.
pub fn forward(
    s: &State,
    target: &ForwardTarget,
    wind: [f64; 3],
    loops: &mut Loops,
    input: &mut PilotInput,
) {
    let coupled = target.coupled;
    let pitch_limit = target.pitch_limit;
    if !loops.started {
        loops.started = true;
        loops.tilt[0] = s.pitch.clamp(-pitch_limit, pitch_limit);
    }
    let (sin, cos) = s.yaw.sin_cos();
    let along = s.velocity[0] * sin + s.velocity[2] * cos;
    if coupled && !loops.coupled {
        // The collective is held from here.
        loops.coupled = true;
        loops.collective = s.lift_controls.collective;
    }
    loops.coupled = coupled;
    let bank = target.bank;
    let climb = target.climb;
    let pitch = if coupled {
        // Speed on the cyclic, height on the collective.
        let error = target.speed - along;
        let proportional = -VELOCITY_GAIN * error / GRAVITY;
        if (proportional + loops.tilt[0]).abs() < pitch_limit {
            loops.tilt[0] =
                (loops.tilt[0] - VELOCITY_INTEGRAL * error * DT).clamp(-pitch_limit, pitch_limit);
        }
        let climb_error = climb - s.vertical_speed;
        loops.collective = (loops.collective + CLIMB_INTEGRAL * climb_error * DT).clamp(0., 1.);
        input.collective = Some((loops.collective + CLIMB_GAIN * climb_error).clamp(0., 1.));
        input.collective_rate = 0.;
        (proportional + loops.tilt[0]).clamp(-pitch_limit, pitch_limit)
    } else {
        // The climb on the pitch, the power lever the caller's.
        let path_error = (climb - s.vertical_speed) / s.speed.max(HOVER_SPEED_KT * KT);
        loops.tilt[0] =
            (loops.tilt[0] + PATH_INTEGRAL * path_error * DT).clamp(-pitch_limit, pitch_limit);
        (loops.tilt[0] + PATH_GAIN * path_error).clamp(-pitch_limit, pitch_limit)
    };
    let retained = retention(s, [pitch, bank]);
    let [p, q, r] = s.lift_controls.body_rates;
    let full = full_rates(s);
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
    // The pedals keep the turn coordinated: the yaw rate of the turn, and
    // the sideslip through the air steered out.
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
