//! The AI's jetborne law for the AV-8 and the Yak-141 (slice H1 of the
//! helicopter and VTOL AI pass, docs/spec/ai.md "Rotorcraft and jet-lift
//! control"): the hover, the vertical climb and descent, and slow flight,
//! for the vertical and short takeoffs and landings the airfield sequences
//! fly (slice H2). Wingborne flight stays on the fixed-wing
//! [`super::steering_adapter`].
//!
//! The controls a pilot uses in the hover (the manual's vertical takeoff
//! and landing, and the physics of `flight::powered::jet`):
//!
//! - **Throttle for height**: the climb toward the commanded wheel height,
//!   within the commanded climb and sink, on the throttle, with an
//!   integrator for the hover power.
//! - **Nozzle for speed**: the ground velocity along the heading (toward
//!   the point, plus any slow-flight velocity) on the nozzle angle about
//!   vertical: aft of vertical accelerates, forward of it, to the braking
//!   stop, slows.
//! - **Attitude on the stick**: the nose held level, the bank tilting the
//!   lift for the velocity across the heading, the heading on the pedals
//!   (the tail puffer), through the same attitude loops as the rotorcraft
//!   ([`crate::autopilot::rotor_law`]).
//!
//! A loaded AV-8 cannot hover (the manual's "never be able to fully
//! hover"); the law then sinks at full throttle and the caller must choose
//! a runway. Every number here is fitted (agent decisions, 2026-10-12).
use super::rotor_adapter::HoverGuidance;
use crate::autopilot::Loops;
use crate::autopilot::rotor_law::{self, angle};
use crate::flight::{DT, PilotInput, State};
use crate::models::FlightModel;
use crate::models::variety::LiftKind;

const GRAVITY: f64 = 32.174;

/// Climb commanded per foot of height error, 1/s (the rotorcraft's).
pub const HEIGHT_GAIN: f64 = rotor_law::HEIGHT_GAIN;
/// Throttle per ft/s of climb error, and its integrator per foot.
pub const CLIMB_GAIN: f64 = 0.03;
pub const CLIMB_INTEGRAL: f64 = 0.01;
/// Position to ground velocity, 1/s (the rotorcraft's).
pub const POSITION_GAIN: f64 = rotor_law::POSITION_GAIN;
/// Velocity error to acceleration, 1/s, and the acceleration integrator
/// per foot of velocity error, 1/s².
pub const VELOCITY_GAIN: f64 = 0.6;
pub const VELOCITY_INTEGRAL: f64 = 0.05;
/// The nozzle's swing either side of vertical for the speed, degrees: aft
/// to 60 degrees (retail's short takeoff first step), forward to the
/// braking stop.
pub const NOZZLE_AFT_DEG: f64 = 60.;
/// The bank it may command for the velocity across the heading, degrees.
pub const BANK_LIMIT_DEG: f64 = 10.;
/// The nose attitude held, degrees.
pub const HOVER_PITCH_DEG: f64 = 0.;

/// Whether `s` is a vectoring jet on the hybrid flight model.
pub fn flies(s: &State) -> bool {
    s.research.is_some()
        && s.native.is_none()
        && s.model()
            .powered_lift()
            .is_some_and(|lift| lift.kind == LiftKind::VectorJet)
}

/// Fitted (agent decision, slice H2): the hover check of [`hover_throttle`]
/// is made this high above the ground, so the ground's suck-down right
/// under the wheels does not count.
pub const HOVER_CHECK_RISE_FT: f64 = 50.;

/// The throttle the engines need to hold `s` in a hover at its present
/// weight, stores included, at [`HOVER_CHECK_RISE_FT`] above `ground_ft`:
/// the hover trim's balance ([`State::trim_hover`]) of the weight against
/// the vertical thrust with its lapse and suck-down. `None` when `s` is not
/// a vectoring jet on the hybrid model. Above 1 it cannot hover at all.
pub fn hover_throttle(s: &State, ground_ft: f64) -> Option<f64> {
    if !flies(s) {
        return None;
    }
    let jet = s.model().powered_lift()?.jet?;
    let c = s.model().configuration();
    let weight = c.mass.empty_lbs + s.fuel + s.carried_lbs();
    let altitude = ground_ft + HOVER_CHECK_RISE_FT;
    let lapse = (-altitude.max(0.) / c.tuning.thrust_lapse_feet).exp();
    let vertical = std::f64::consts::FRAC_PI_2;
    let suck = crate::flight::powered::jet::suck_down(&jet, HOVER_CHECK_RISE_FT, vertical);
    let lift_engines = jet.lift_engines.map_or(0., |e| e.thrust_lbf);
    let thrust = (c.propulsion.military_thrust_lbf
        * crate::flight::powered::jet::nozzle_efficiency(&jet, vertical)
        + lift_engines)
        * lapse
        * (1. - suck);
    (thrust > 0.).then(|| weight / thrust)
}

/// One AI jet's jetborne memory: the integrators of the attitude loops
/// (`stick`), the hover power (`collective`, used here for the throttle)
/// and the speed (`tilt`: along and across the heading).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JetLift {
    pub(super) loops: Loops,
}

/// What one tick commanded, for the trace and the drivers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JetLiftCommand {
    /// Nozzle angle commanded, degrees from aft.
    pub nozzle_deg: f64,
    /// Climb commanded, ft/s.
    pub climb_fps: f64,
    /// Bank commanded, degrees.
    pub bank_deg: f64,
}

impl JetLift {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget the integrators: the next hover starts from the aircraft's
    /// own throttle (slice H2, a new liftoff or a landing's hover).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The controls for this tick toward `guidance` over terrain at `ground`
    /// ft.
    pub fn controls(
        &mut self,
        s: &State,
        guidance: &HoverGuidance,
        ground: f64,
    ) -> (PilotInput, JetLiftCommand) {
        let stop = s
            .model()
            .powered_lift()
            .and_then(|lift| lift.jet)
            .map_or(100., |jet| jet.nozzle_range_degrees);
        let loops = &mut self.loops;
        if !loops.started {
            loops.started = true;
            loops.collective = s.throttle;
        }
        let clearance = s.model().configuration().equipment.ground_clearance_ft;
        let height = s.position[1] - ground - clearance;
        // Height on the throttle.
        let climb = (HEIGHT_GAIN * (guidance.height_agl_ft - height)).clamp(
            -guidance.max_descent_fps.max(0.),
            guidance.max_climb_fps.max(0.),
        );
        let climb_error = climb - s.vertical_speed;
        loops.collective = (loops.collective + CLIMB_INTEGRAL * climb_error * DT).clamp(0., 1.);
        let throttle = (loops.collective + CLIMB_GAIN * climb_error).clamp(0., 1.);
        // The ground velocity wanted: toward the point, plus the slow
        // flight, within the closing speed.
        let mut wanted = [
            POSITION_GAIN * (guidance.point[0] - s.position[0]) + guidance.velocity_fps[0],
            POSITION_GAIN * (guidance.point[1] - s.position[2]) + guidance.velocity_fps[1],
        ];
        let closing = wanted[0].hypot(wanted[1]);
        let close = guidance.max_speed_fps.max(0.);
        if closing > close {
            wanted = wanted.map(|v| v * close / closing);
        }
        let error = [wanted[0] - s.velocity[0], wanted[1] - s.velocity[2]];
        let (sin, cos) = s.yaw.sin_cos();
        let along = [
            error[0] * sin + error[1] * cos,
            error[0] * cos - error[1] * sin,
        ];
        // Acceleration wanted along and across the heading, ft/s².
        let accel: [f64; 2] = std::array::from_fn(|axis| {
            let limit = GRAVITY * 0.5;
            let proportional = VELOCITY_GAIN * along[axis];
            if (proportional + loops.tilt[axis]).abs() < limit {
                loops.tilt[axis] =
                    (loops.tilt[axis] + VELOCITY_INTEGRAL * along[axis] * DT).clamp(-limit, limit);
            }
            (proportional + loops.tilt[axis]).clamp(-limit, limit)
        });
        // Nozzle for the speed along: aft of vertical (under 90) pushes
        // forward.
        let nozzle_deg =
            (90. - (accel[0] / GRAVITY).atan().to_degrees()).clamp(NOZZLE_AFT_DEG, stop);
        // Bank for the speed across.
        let bank = (accel[1] / GRAVITY)
            .atan()
            .clamp(-BANK_LIMIT_DEG.to_radians(), BANK_LIMIT_DEG.to_radians());
        let pitch = HOVER_PITCH_DEG.to_radians();
        let retained = rotor_law::retention(s, [pitch, bank]);
        let [p, q, r] = s.lift_controls.body_rates;
        let full = rotor_law::full_rates(s);
        let mut input = PilotInput {
            throttle: Some(throttle),
            vector_pitch: Some(nozzle_deg / stop),
            ..PilotInput::default()
        };
        input.pitch = rotor_law::attitude_loop(
            pitch - s.pitch,
            q,
            full[0],
            rotor_law::ATTITUDE_BANDWIDTH,
            rotor_law::ATTITUDE_WINDOW,
            retained[0],
            &mut loops.stick[0],
        );
        input.roll = rotor_law::attitude_loop(
            angle(bank - s.bank),
            p,
            full[1],
            rotor_law::ATTITUDE_BANDWIDTH,
            rotor_law::ATTITUDE_WINDOW,
            retained[1],
            &mut loops.stick[1],
        );
        input.yaw = rotor_law::attitude_loop(
            angle(guidance.heading_deg.to_radians() - s.yaw),
            r,
            full[2],
            rotor_law::HEADING_BANDWIDTH,
            rotor_law::HEADING_WINDOW,
            0.,
            &mut loops.stick[2],
        );
        (
            input.bounded(),
            JetLiftCommand {
                nozzle_deg,
                climb_fps: climb,
                bank_deg: bank.to_degrees(),
            },
        )
    }
}
