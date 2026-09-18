//! AI intent-to-input feedback controller. The aircraft model alone advances
//! attitude, velocity, position and telemetry; no AI movement override exists.
//! B44 remains a requested-envelope/reference service, not an achieved-motion
//! constraint. The fitted controller is specified in docs/spec/ai.md under
//! "Input-only aircraft control". Player adapters are unchanged.

use super::controller::MotionIntent;
use super::fitted::{self, Fallback};
use super::motion::Bank;
use super::steering::{
    self, AuthorityModifiers, AxisRates, SteeringRequest, SteeringState, TurnDirection,
};
use super::{AiError, Experience, Result, ScalarSpeed, SpeedLimits, experience};
use crate::flight::State;
use crate::models::FlightModel;
use tore_input::{PilotCommand, PilotInput, Switch};

/// Fitted heading-error response horizon; see the input-only control spec.
pub const BANK_COMMAND_LEAD_SECONDS: f64 = 1.0;

/// Fitted: the speed error, in ft/s, that moves the throttle from closed to
/// open in one tick. Rule: the throttle setting is the current setting plus
/// the speed error divided by this reference, clamped to 0 through 1. A
/// hundred feet per second is an agent choice, about 60 knots, which is a
/// large speed error for a fighter, so ordinary corrections are gentle and a
/// large one saturates. The spec gives no AI throttle rule at all.
pub const THROTTLE_REFERENCE_ERROR_FPS: f64 = 100.0;

/// Controls plus what the adapter had to fall back on.
#[derive(Clone, Debug, PartialEq)]
pub struct AdapterOutput {
    pub input: PilotInput,
    pub fallbacks: Vec<Fallback>,
    /// The rate-limited attitude the B44 rules asked for this tick, kept so a
    /// caller can compare requested against achieved.
    pub requested: SteeringState,
}

/// Per-actor steering state. One per AI aircraft, never shared.
///
/// The adapter holds no control history: every tick reads the actor's actual
/// attitude out of its flight state, so a lagging or damaged aircraft cannot
/// accumulate an invisible error here. The recorded request exists for
/// reporting and tests only and never feeds back into a command.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ControlAdapter {
    last_requested: Option<SteeringState>,
}

impl ControlAdapter {
    pub fn new() -> Self {
        Self {
            last_requested: None,
        }
    }

    /// The attitude the previous call asked for, or `None` before the first.
    pub fn last_requested(&self) -> Option<SteeringState> {
        self.last_requested
    }

    /// Turn one resolved maneuver into controls for this actor's own flight
    /// model. `terrain_pitch_floor_deg` is the active B44 floor when one
    /// applies. Returns the controls and every fitted fallback applied.
    ///
    /// `g_limit` and `roll_limit_deg_per_s` are the loaded limits the flight
    /// model supplies, with the documented fitted health reduction and the
    /// AI experience adjustment of [`ai_g_limits`] already applied by the
    /// caller. `maximum_bank_deg` is the aircraft's own maximum bank: it
    /// bounds the bank request and is the B44 reference bank.
    #[allow(clippy::too_many_arguments)]
    pub fn controls(
        &mut self,
        state: &State,
        intent: &MotionIntent,
        limits: &SpeedLimits,
        g_limit: f64,
        roll_limit_deg_per_s: f64,
        maximum_bank_deg: f64,
        terrain_pitch_floor_deg: Option<f64>,
        dt_s: f64,
    ) -> Result<AdapterOutput> {
        if !dt_s.is_finite() || dt_s < 0.0 {
            return Err(AiError::InvalidInput(
                "time step must be finite and non-negative",
            ));
        }
        finite(intent.heading_deg, "requested heading must be finite")?;
        finite(
            intent.flight_path_pitch_deg,
            "requested flight-path pitch must be finite",
        )?;
        finite(state.speed, "own speed must be finite")?;
        finite(state.throttle, "own throttle must be finite")?;

        let current = attitude(state)?;
        let speed = ScalarSpeed(state.speed);

        // B44 leaves the base pitch rate unresolved; the documented fitted
        // stand-in is the only source for it, and its use is always reported.
        let fallbacks = vec![Fallback::BasePitchRate];
        let pitch_deg_per_s = fitted::base_pitch_rate_deg_per_s(g_limit, speed)?;
        let rates = AxisRates::from_limits(g_limit, speed, roll_limit_deg_per_s, pitch_deg_per_s)?;

        let reference_bank_deg = steering::reference_bank_deg(maximum_bank_deg)?;
        let heading_request_deg = steering::wrap_heading(intent.heading_deg)?;
        let turn = TurnDirection::toward(current.heading_deg, heading_request_deg);
        let bank_request_deg = match intent.bank {
            Bank::Explicit(deg) => f64::from(deg).clamp(-maximum_bank_deg, maximum_bank_deg),
            Bank::Unconstrained => {
                let rate = heading_error_deg(current.heading_deg, heading_request_deg).to_radians()
                    / BANK_COMMAND_LEAD_SECONDS;
                let bank_limit = maximum_bank_deg
                    .min(if intent.formation_flight { 60.0 } else { 75.0 })
                    .min((1.0 / g_limit.max(1.0)).acos().to_degrees());
                (state.speed.max(125.0) * rate / 32.174)
                    .atan()
                    .to_degrees()
                    .clamp(-bank_limit, bank_limit)
            }
        };

        // Contact is tracked by the hybrid adapter; without it the adapter
        // cannot see the ground from this signature and reports airborne.
        let on_ground = state.research.as_ref().is_some_and(|r| r.on_ground);
        let request = SteeringRequest {
            heading_deg: heading_request_deg,
            flight_path_pitch_deg: intent.flight_path_pitch_deg,
            bank_deg: bank_request_deg,
            reference_bank_deg,
            // The ceiling test belongs to the maneuver layer, which owns the
            // aircraft's ceiling; it is not visible from this signature.
            at_ceiling: false,
            terrain_pitch_floor_deg,
            on_ground,
            mode: intent.mode,
        };
        let requested = current.step(&request, rates, dt_s)?;

        // B44 limits the desired roll rate, never the achieved trajectory.
        let effective = steering::effective_rates(
            request.mode,
            rates,
            current.bank_deg,
            reference_bank_deg,
            turn,
            AuthorityModifiers {
                terrain_floor_active: terrain_pitch_floor_deg.is_some(),
                on_ground,
            },
        )?;

        // Fitted feedback controller. These are control requests only: the
        // aircraft model owns every achieved attitude, velocity and position.
        let config = state.model().configuration();
        let stall = config
            .aerodynamics
            .envelopes
            .iter()
            .find(|e| e.g == 1)
            .and_then(|e| e.speeds(state.position[1]))
            .map_or(900.0, |e| e.0);
        let authority = (state.speed / stall.max(1.0)).powi(2).clamp(0.0, 1.0);
        let bank_error = heading_error_deg(current.bank_deg, bank_request_deg).to_radians();
        let desired_roll = (bank_error / 0.7 - state.roll_rate * 0.5).clamp(
            -effective.roll_deg_per_s.to_radians(),
            effective.roll_deg_per_s.to_radians(),
        );
        let roll_limit = if let Some(profile) = config.controls {
            let axis = if state.research.is_some() {
                profile.hybrid_roll.unwrap_or(profile.roll)
            } else {
                profile.roll
            };
            let bound = if desired_roll < 0.0 {
                -f64::from(axis.minimum)
            } else {
                f64::from(axis.maximum)
            };
            bound.to_radians() * (state.speed / (2.0 * stall.max(1.0))).clamp(0.0, 1.0)
        } else if state.research.is_some() {
            config
                .aerodynamics
                .roll_limit_rad_per_second
                .clamp(0.1, 6.0)
                * authority
        } else {
            config.tuning.legacy_roll_limit_rad_per_second * authority
        };
        let (mut low, mut high) = (-1.0_f64, 1.0_f64);
        for envelope in &config.aerodynamics.envelopes {
            if let Some((min, max)) = envelope.speeds(state.position[1])
                && state.speed >= min
                && state.speed <= max
            {
                low = low.min(f64::from(envelope.g));
                high = high.max(f64::from(envelope.g));
            }
        }
        let loading = 1.0
            + (state.fuel + state.payload_lbs) / config.mass.empty_lbs
                * config.aerodynamics.loaded_elevator_percent
                / 100.0;
        low /= loading;
        high /= loading;
        let pitch_goal = intent
            .flight_path_pitch_deg
            .max(terrain_pitch_floor_deg.unwrap_or(-90.0))
            .clamp(-90.0, 90.0);
        let pitch_error = (pitch_goal - current.flight_path_pitch_deg).to_radians();
        let desired_g = ((current.flight_path_pitch_deg.to_radians().cos()
            + state.speed * pitch_error / (3.0 * 32.174))
            / state.bank.cos().max(0.25))
        .clamp(low, g_limit.max(low));
        let delta = desired_g / authority.max(0.01) - 1.0;
        let pitch_input = delta
            / if delta > 0.0 {
                (high - 1.0).max(0.01)
            } else {
                (1.0 - low).max(0.01)
            };

        let input = PilotInput {
            pitch: if dt_s > 0.0 {
                pitch_input.clamp(-1.0, 1.0)
            } else {
                0.0
            },
            roll: if dt_s > 0.0 {
                (desired_roll / roll_limit.max(0.01)).clamp(-1.0, 1.0)
            } else {
                0.0
            },
            // The AI steers on bank and pitch only; B44 gives the rudder no
            // steering role, so the yaw axis is left alone.
            yaw: 0.0,
            throttle_rate: 0.0,
            throttle: Some(throttle_command(
                state.throttle,
                speed,
                intent.speed,
                limits,
            )?),
            commands: vec![PilotCommand::Set(Switch::Burner, intent.afterburner)],
        }
        .bounded();

        self.last_requested = Some(requested);
        Ok(AdapterOutput {
            input,
            fallbacks,
            requested,
        })
    }
}

/// The AI-only experience G adjustment ("Other experience effects").
///
/// Novice and Average AI aircraft lose 1 G of positive limit, never below 2 G,
/// and 1 G of negative limit, never beyond -2 G. Experienced and Ace are
/// unchanged.
///
/// Returns the loaded limits unchanged for a human-flown aircraft. The
/// exemption is the object's human-control bit and no AI path sets it, so
/// player flight limits never pass through the adjustment.
pub fn ai_g_limits(
    level: Experience,
    positive_g: f64,
    negative_g: f64,
    human_controlled: bool,
) -> (f64, f64) {
    experience::adjusted_g_limits(level, positive_g, negative_g, human_controlled)
}

/// Fitted: the throttle setting for a commanded speed.
///
/// Rule: the commanded speed is held inside the aircraft's own minimum and
/// maximum, and the setting is the current setting plus the remaining speed
/// error divided by [`THROTTLE_REFERENCE_ERROR_FPS`], clamped to 0 through 1.
/// Reading the actual setting back each tick makes this self-correcting
/// without any stored integrator.
pub fn throttle_command(
    current_throttle: f64,
    speed: ScalarSpeed,
    requested: ScalarSpeed,
    limits: &SpeedLimits,
) -> Result<f64> {
    finite(requested.0, "requested speed must be finite")?;
    finite(limits.minimum.0, "minimum speed must be finite")?;
    finite(limits.maximum.0, "maximum speed must be finite")?;
    if limits.minimum.0 > limits.maximum.0 {
        return Err(AiError::InvalidInput(
            "minimum speed must not exceed maximum speed",
        ));
    }
    let target = requested.0.clamp(limits.minimum.0, limits.maximum.0);
    Ok((current_throttle + (target - speed.0) / THROTTLE_REFERENCE_ERROR_FPS).clamp(0.0, 1.0))
}

/// The actor's current attitude in the recovered degree domain.
///
/// Heading is the body heading, `yaw` measured from +Z toward +X. Flight-path
/// pitch is the direction of the velocity vector; body pitch minus it is the
/// B44 body pitch offset, which steering carries through unchanged. A stopped
/// aircraft has no velocity direction, so its nose direction is used and the
/// offset is zero.
fn attitude(state: &State) -> Result<SteeringState> {
    finite(state.yaw, "own heading must be finite")?;
    finite(state.pitch, "own pitch must be finite")?;
    finite(state.bank, "own bank must be finite")?;
    let body_pitch_deg = signed_angle_deg(state.pitch.to_degrees());
    let [vx, vy, vz] = state.velocity;
    let horizontal = vx.hypot(vz);
    let flight_path_pitch_deg =
        if vy.is_finite() && horizontal.is_finite() && horizontal + vy != 0.0 {
            vy.atan2(horizontal).to_degrees()
        } else {
            body_pitch_deg
        };
    Ok(SteeringState {
        heading_deg: steering::wrap_heading(state.yaw.to_degrees())?,
        flight_path_pitch_deg,
        bank_deg: signed_angle_deg(state.bank.to_degrees()),
        body_pitch_offset_deg: body_pitch_deg - flight_path_pitch_deg,
    })
}

/// Signed shorter path from `current` to `requested`, in -180 through 180.
fn heading_error_deg(current_deg: f64, requested_deg: f64) -> f64 {
    (requested_deg - current_deg + 180.0).rem_euclid(360.0) - 180.0
}

/// Wrap an angle into -180 through 180 degrees.
fn signed_angle_deg(degrees: f64) -> f64 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}

fn finite(value: f64, what: &'static str) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(AiError::InvalidInput(what))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::DecisionRandom;
    use crate::ai::controller::Completion;
    use crate::ai::motion::{CompletionAxis, Duration, MotionRequest, PitchRequest, SpeedRequest};
    use crate::ai::steering::{CommandMode, SECONDS_PER_TICK};
    use crate::flight::integration_tests::profile;

    const G_LIMIT: f64 = 7.0;
    const ROLL_LIMIT: f64 = 180.0;
    const MAXIMUM_BANK: f64 = 80.0;

    fn limits() -> SpeedLimits {
        SpeedLimits {
            minimum: ScalarSpeed(200.0),
            maximum: ScalarSpeed(1600.0),
            corner: ScalarSpeed(700.0),
        }
    }

    fn state() -> State {
        let mut s = State::new(&profile(), [0.0, 15000.0, 0.0]).unwrap();
        s.yaw = 0.0;
        s.pitch = 0.0;
        s.bank = 0.0;
        s.velocity = [0.0, 0.0, s.speed];
        s
    }

    fn intent(heading_deg: f64, pitch_deg: f64, speed: f64) -> MotionIntent {
        MotionIntent {
            formation_flight: false,
            afterburner: false,
            id: 1,
            request: MotionRequest::new(
                heading_deg as i32,
                PitchRequest::Explicit(pitch_deg as i32),
                Bank::Unconstrained,
                SpeedRequest::Explicit(ScalarSpeed(speed)),
                Duration::Timed(5),
            ),
            heading_deg,
            flight_path_pitch_deg: pitch_deg,
            speed: ScalarSpeed(speed),
            bank: Bank::Unconstrained,
            completion: Completion::Axis(CompletionAxis::Heading),
            steering_point: None,
            mode: CommandMode::Ordinary,
        }
    }

    fn controls(state: &State, intent: &MotionIntent, dt_s: f64) -> AdapterOutput {
        ControlAdapter::new()
            .controls(
                state,
                intent,
                &limits(),
                G_LIMIT,
                ROLL_LIMIT,
                MAXIMUM_BANK,
                None,
                dt_s,
            )
            .unwrap()
    }

    #[test]
    fn formation_heading_follows_bank_through_a_shallow_turn() {
        for direction in [-1.0, 1.0] {
            let mut s = state();
            s.speed = 800.;
            s.velocity = [0., 0., 800.];
            let mut command = intent(direction * 10., 0., 800.);
            command.formation_flight = true;
            command.mode = CommandMode::OtherState;
            let mut peak_bank = 0.0_f64;
            for _ in 0..2400 {
                let output = controls(&s, &command, crate::flight::DT);
                assert_eq!(output.input.yaw, 0.);
                s.step(&output.input, |_, _| 0.0);
                peak_bank = peak_bank.max(s.bank.abs().to_degrees());
            }
            assert!(peak_bank > 20. && peak_bank <= 60.);
            assert!(heading_error_deg(s.yaw.to_degrees(), direction * 10.).abs() < 0.1);
            assert!(s.bank.abs().to_degrees() < 0.1);
        }
    }

    #[test]
    fn a_commanded_heading_change_rolls_into_the_turn_on_the_correct_side() {
        let mut s = state();
        let right = controls(&s, &intent(90.0, 0.0, s.speed), SECONDS_PER_TICK);
        assert!(right.input.roll > 0.0, "{}", right.input.roll);
        assert!(right.requested.bank_deg > 0.0);

        let left = controls(&s, &intent(270.0, 0.0, s.speed), SECONDS_PER_TICK);
        assert!(left.input.roll < 0.0, "{}", left.input.roll);
        assert!(left.requested.bank_deg < 0.0);

        let straight = controls(&s, &intent(0.0, 0.0, s.speed), SECONDS_PER_TICK);
        assert_eq!(straight.input.roll, 0.0);

        // Across the 0/360 wrap the shorter path still decides the side.
        s.yaw = 350f64.to_radians();
        s.velocity = crate::attitude::Basis::new(s.yaw, 0.0, 0.0)
            .forward
            .map(|v| v * s.speed);
        let over_zero = controls(&s, &intent(10.0, 0.0, s.speed), SECONDS_PER_TICK);
        assert!(over_zero.input.roll > 0.0, "{}", over_zero.input.roll);

        s.yaw = 10f64.to_radians();
        s.velocity = crate::attitude::Basis::new(s.yaw, 0.0, 0.0)
            .forward
            .map(|v| v * s.speed);
        let under_zero = controls(&s, &intent(350.0, 0.0, s.speed), SECONDS_PER_TICK);
        assert!(under_zero.input.roll < 0.0, "{}", under_zero.input.roll);
    }

    #[test]
    fn the_approach_stops_at_the_request_instead_of_overshooting() {
        // B44: a step long enough to cover the whole difference lands exactly
        // on the request, and the deflection it asks for is still bounded.
        let mut s = state();
        s.yaw = 89f64.to_radians();
        s.bank = 10f64.to_radians();
        s.velocity = crate::attitude::Basis::new(s.yaw, 0.0, 0.0)
            .forward
            .map(|v| v * s.speed);
        let mut command = intent(90.0, 5.0, s.speed);
        command.bank = Bank::Explicit(20);
        let long = controls(&s, &command, 10.0);
        assert_eq!(long.requested.heading_deg, 90.0);
        assert_eq!(long.requested.flight_path_pitch_deg, 5.0);
        assert_eq!(long.requested.bank_deg, 20.0);
        assert!(long.input.roll.abs() <= 1.0 && long.input.pitch.abs() <= 1.0);

        // Repeating from the arrived attitude asks for nothing further.
        s.yaw = 90f64.to_radians();
        s.bank = 20f64.to_radians();
        s.pitch = 5f64.to_radians();
        s.velocity = crate::attitude::Basis::new(s.yaw, 5f64.to_radians(), 0.0)
            .forward
            .map(|v| v * s.speed);
        let arrived = controls(&s, &command, 10.0);
        assert_eq!(arrived.requested.heading_deg, 90.0);
        assert!(arrived.input.roll.abs() < 1e-9, "{}", arrived.input.roll);
        assert!(
            arrived.input.pitch > 0.0,
            "a banked aircraft still needs lift to hold its flight path"
        );
    }

    #[test]
    fn a_climb_commands_nose_up_and_a_dive_commands_nose_down() {
        let s = state();
        let climb = controls(&s, &intent(0.0, 20.0, s.speed), SECONDS_PER_TICK);
        assert!(climb.input.pitch > 0.0, "{}", climb.input.pitch);
        assert!(climb.requested.flight_path_pitch_deg > 0.0);

        let dive = controls(&s, &intent(0.0, -20.0, s.speed), SECONDS_PER_TICK);
        assert!(dive.input.pitch < 0.0, "{}", dive.input.pitch);
        assert!(dive.requested.flight_path_pitch_deg < 0.0);

        let level = controls(&s, &intent(0.0, 0.0, s.speed), SECONDS_PER_TICK);
        assert_eq!(level.input.pitch, 0.0);
    }

    #[test]
    fn throttle_follows_the_commanded_speed_within_the_limits() {
        let mut s = state();
        s.throttle = 0.5;
        let faster = controls(&s, &intent(0.0, 0.0, s.speed + 50.0), SECONDS_PER_TICK);
        assert!(faster.input.throttle.unwrap() > 0.5);
        let slower = controls(&s, &intent(0.0, 0.0, s.speed - 50.0), SECONDS_PER_TICK);
        assert!(slower.input.throttle.unwrap() < 0.5);
        let held = controls(&s, &intent(0.0, 0.0, s.speed), SECONDS_PER_TICK);
        assert!((held.input.throttle.unwrap() - 0.5).abs() < 1e-9);

        // A request outside the aircraft's own limits is held at the limit.
        let absurd = controls(&s, &intent(0.0, 0.0, 99_000.0), SECONDS_PER_TICK);
        assert_eq!(absurd.input.throttle, Some(1.0));
        let stopped = controls(&s, &intent(0.0, 0.0, 0.0), SECONDS_PER_TICK);
        let at_minimum =
            throttle_command(0.5, ScalarSpeed(s.speed), limits().minimum, &limits()).unwrap();
        assert_eq!(stopped.input.throttle, Some(at_minimum));
    }

    #[test]
    fn every_control_is_finite_and_bounded_over_a_randomised_sweep() {
        let mut random = DecisionRandom::seeded(0x005E_EDA1);
        let mut s = state();
        let mut adapter = ControlAdapter::new();
        for _ in 0..4000 {
            s.yaw = f64::from(random.below(360)).to_radians();
            s.pitch = (f64::from(random.below(181)) - 90.0).to_radians();
            s.bank = (f64::from(random.below(361)) - 180.0).to_radians();
            s.speed = f64::from(random.below(1800));
            s.throttle = f64::from(random.below(101)) / 100.0;
            s.velocity = crate::attitude::Basis::new(s.yaw, s.pitch, s.bank)
                .forward
                .map(|v| v * s.speed);
            let mut command = intent(
                f64::from(random.below(360)),
                f64::from(random.below(181)) - 90.0,
                f64::from(random.below(2000)),
            );
            command.mode = match random.below(3) {
                0 => CommandMode::Ordinary,
                1 => CommandMode::ReducedRate,
                _ => CommandMode::OtherState,
            };
            if random.chance(50) {
                command.bank = Bank::Explicit(random.below(361) as i32 - 180);
            }
            let floor = random
                .chance(50)
                .then(|| f64::from(random.below(19)) * 5.0 - 90.0);
            let dt = if random.chance(10) {
                0.0
            } else {
                SECONDS_PER_TICK
            };
            let out = adapter
                .controls(
                    &s,
                    &command,
                    &limits(),
                    G_LIMIT,
                    ROLL_LIMIT,
                    MAXIMUM_BANK,
                    floor,
                    dt,
                )
                .unwrap();
            for axis in [out.input.pitch, out.input.roll, out.input.yaw] {
                assert!(axis.is_finite() && (-1.0..=1.0).contains(&axis), "{axis}");
            }
            let throttle = out.input.throttle.unwrap();
            assert!(
                throttle.is_finite() && (0.0..=1.0).contains(&throttle),
                "{throttle}"
            );
            assert!(out.requested.heading_deg.is_finite());
            assert!(out.requested.flight_path_pitch_deg.abs() <= 90.0 + 1e-9);
        }
    }

    #[test]
    fn the_experience_g_adjustment_applies_to_ai_aircraft_only() {
        assert_eq!(
            ai_g_limits(Experience::Novice, 9.0, -3.0, false),
            (8.0, -2.0)
        );
        assert_eq!(
            ai_g_limits(Experience::Average, 7.5, -4.0, false),
            (6.5, -3.0)
        );
        // The 2 G and -2 G floors.
        assert_eq!(
            ai_g_limits(Experience::Novice, 2.5, -2.5, false),
            (2.0, -2.0)
        );
        assert_eq!(
            ai_g_limits(Experience::Average, 2.0, -2.0, false),
            (2.0, -2.0)
        );
        // Experienced and Ace are unchanged.
        assert_eq!(
            ai_g_limits(Experience::Experienced, 9.0, -3.0, false),
            (9.0, -3.0)
        );
        assert_eq!(ai_g_limits(Experience::Ace, 9.0, -3.0, false), (9.0, -3.0));
        // A human-flown aircraft keeps its full limits at every level.
        for level in Experience::ALL {
            assert_eq!(ai_g_limits(level, 9.0, -3.0, true), (9.0, -3.0));
        }
    }

    #[test]
    fn the_base_pitch_rate_fallback_is_reported() {
        let s = state();
        let out = controls(&s, &intent(90.0, 10.0, s.speed), SECONDS_PER_TICK);
        assert_eq!(out.fallbacks, vec![Fallback::BasePitchRate]);
    }

    #[test]
    fn the_same_state_and_intent_give_identical_output_twice() {
        let s = state();
        let command = intent(123.0, -12.0, 700.0);
        let mut adapter = ControlAdapter::new();
        let run = |adapter: &mut ControlAdapter| {
            adapter
                .controls(
                    &s,
                    &command,
                    &limits(),
                    G_LIMIT,
                    ROLL_LIMIT,
                    MAXIMUM_BANK,
                    Some(-10.0),
                    SECONDS_PER_TICK,
                )
                .unwrap()
        };
        let first = run(&mut adapter);
        let second = run(&mut adapter);
        assert_eq!(first, second);
        assert_eq!(
            first.input.pitch.to_bits(),
            second.input.pitch.to_bits(),
            "pitch is not bit-identical"
        );
        assert_eq!(first.input.roll.to_bits(), second.input.roll.to_bits());
        assert_eq!(
            first.input.throttle.unwrap().to_bits(),
            second.input.throttle.unwrap().to_bits()
        );
        // A fresh adapter produces the same output as a used one.
        assert_eq!(first, run(&mut ControlAdapter::new()));
    }

    #[test]
    fn invalid_inputs_are_rejected_rather_than_flown() {
        let s = state();
        let command = intent(90.0, 0.0, s.speed);
        let mut adapter = ControlAdapter::new();
        let call = |adapter: &mut ControlAdapter, bank, dt| {
            adapter.controls(&s, &command, &limits(), G_LIMIT, ROLL_LIMIT, bank, None, dt)
        };
        assert!(call(&mut adapter, 0.0, SECONDS_PER_TICK).is_err());
        assert!(call(&mut adapter, MAXIMUM_BANK, -1.0).is_err());
        assert!(call(&mut adapter, MAXIMUM_BANK, f64::NAN).is_err());
        assert!(adapter.last_requested().is_none());
        assert!(call(&mut adapter, MAXIMUM_BANK, SECONDS_PER_TICK).is_ok());
        assert!(adapter.last_requested().is_some());
    }

    #[test]
    fn stepping_a_real_flight_state_turns_the_aircraft_toward_the_command() {
        let mut s = State::new(&profile(), [0.0, 15000.0, 0.0]).unwrap();
        s.yaw = 0.0;
        s.pitch = 0.0;
        s.bank = 0.0;
        s.velocity = [0.0, 0.0, s.speed];
        let command = intent(90.0, 0.0, s.speed);
        let mut adapter = ControlAdapter::new();
        let start = heading_error_deg(0.0, 90.0).abs();
        let mut banked = 0.0f64;
        for _ in 0..600 {
            let out = adapter
                .controls(
                    &s,
                    &command,
                    &limits(),
                    G_LIMIT,
                    ROLL_LIMIT,
                    MAXIMUM_BANK,
                    None,
                    crate::flight::DT,
                )
                .unwrap();
            s.step(&out.input, |_, _| 0.0);
            banked = banked.max(s.bank.to_degrees());
        }
        assert!(!s.crashed);
        let heading = s.yaw.to_degrees().rem_euclid(360.0);
        let error = heading_error_deg(heading, 90.0).abs();
        assert!(
            banked > 30.0,
            "the aircraft never rolled into the turn: {banked}"
        );
        assert!(
            error < start - 30.0,
            "heading {heading} did not close on 90 from an error of {start}"
        );
    }
}
