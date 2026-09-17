//! B44: steering execution. Heading, flight-path pitch and body bank approach
//! their requested values over simulation time; they are never assigned
//! instantaneously (spec: `docs/spec/ai.md`, "B44: Steering execution and
//! pursuit lead", first three paragraphs; request bounding also from B13).
//!
//! Everything here is a pure calculation. Nothing reads the flight adapters
//! or the renderer. The caller owns the aircraft's per-speed turn and roll
//! capability ([`AxisRates`]); the performance lookup that produces it is
//! unresolved and [`performance_rates`] says so.
//!
//! Provenance labels used below:
//!
//! - spec-derived: the rate-limited approach that stops exactly at the
//!   request, the seven-eighths reference-bank threshold, the quarter floors,
//!   the divide-by-three reduced-rate command with its 10 through 30 deg/s
//!   roll bound, the halved and 45 deg/s capped roll of other state branches,
//!   the -90 through +90 pitch bound, ceiling level-off, terrain raising the
//!   pitch request, and the distinct body pitch offset.
//! - fitted: the shape of the "scales with bank magnitude" curve, the reading
//!   of "can suppress" as any opposing bank suppressing, the cosine pitch
//!   curve, the order of ceiling and terrain adjustments, the 180-degree
//!   tie-break, and an unmodified `Ordinary` mode. Each is named at its site.

use super::{AiError, Result, ScalarSpeed, TICKS_PER_SECOND};

/// Seconds advanced by one fixed simulation tick.
pub const SECONDS_PER_TICK: f64 = 1.0 / TICKS_PER_SECOND as f64;

/// B44: below this fraction of the reference bank magnitude, heading
/// authority scales with bank; at or above it, full base authority applies.
pub const ROLL_IN_BANK_FRACTION: f64 = 7.0 / 8.0;
/// B44: heading and pitch authority never fall below this fraction of base.
pub const AUTHORITY_FLOOR_FRACTION: f64 = 0.25;
/// B44: reduced-rate commands divide heading, pitch and roll authority by this.
pub const REDUCED_RATE_DIVISOR: f64 = 3.0;
/// B44: reduced-rate roll authority is bounded to this range, deg/s.
pub const REDUCED_RATE_ROLL_MIN_DEG_PER_S: f64 = 10.0;
pub const REDUCED_RATE_ROLL_MAX_DEG_PER_S: f64 = 30.0;
/// B44: other state branches halve roll authority and cap it here, deg/s.
pub const OTHER_STATE_ROLL_CAP_DEG_PER_S: f64 = 45.0;
/// B13/B44: requested flight-path pitch is bounded to this magnitude.
pub const PITCH_REQUEST_LIMIT_DEG: f64 = 90.0;

/// Direction of the heading change the aircraft is executing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnDirection {
    /// Heading decreasing (counter-clockwise from above); bank negative.
    Left,
    /// Heading increasing (clockwise from above); bank positive.
    Right,
}
impl TurnDirection {
    /// Sign convention: right turns and right bank are positive.
    fn sign(self) -> f64 {
        match self {
            Self::Left => -1.0,
            Self::Right => 1.0,
        }
    }
    /// Direction of the shorter wrapped path from `current` to `requested`.
    /// `None` when there is no heading change to make. A 180-degree
    /// difference resolves to `Left`: fitted, the spec does not tie-break.
    pub fn toward(current_deg: f64, requested_deg: f64) -> Option<Self> {
        let delta = wrapped_delta(current_deg, requested_deg);
        if delta == 0.0 {
            None
        } else if delta > 0.0 {
            Some(Self::Right)
        } else {
            Some(Self::Left)
        }
    }
}

/// Which B44 rate branch a steering command runs under. Explicit so that no
/// branch is chosen implicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandMode {
    /// Base authority passes through unmodified. Fitted: B44 names only the
    /// reduced-rate command and "other state branches"; whether an
    /// unmodified branch exists in the original is unresolved.
    Ordinary,
    /// B44: heading, pitch and roll authority divided by three; roll bounded
    /// to 10 through 30 deg/s.
    ReducedRate,
    /// B44: roll authority halved and capped at 45 deg/s; heading and pitch
    /// are unchanged because the spec gives them no rule here.
    OtherState,
}

/// Per-aircraft turn, pitch and roll capability at the current speed, deg/s.
/// Supplied by the caller from its flight-performance lookup (B44: turn
/// capability depends on the lookup and on current speed, so sharing F.BI
/// does not give identical turns). Rates are non-negative magnitudes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisRates {
    pub turn_deg_per_s: f64,
    pub pitch_deg_per_s: f64,
    pub roll_deg_per_s: f64,
}
impl AxisRates {
    fn validate(self) -> Result<Self> {
        let ok = |v: f64| v.is_finite() && v >= 0.0;
        if ok(self.turn_deg_per_s) && ok(self.pitch_deg_per_s) && ok(self.roll_deg_per_s) {
            Ok(self)
        } else {
            Err(AiError::InvalidInput(
                "axis rates must be finite, non-negative deg/s",
            ))
        }
    }
}

/// B44 flight-performance lookup. The producer (which table, how speed
/// selects the row, and any aircraft-state override) is unknown in the spec,
/// so this always reports the rule as unspecified. Callers pass [`AxisRates`]
/// they obtained elsewhere until research closes it.
pub fn performance_rates(_speed: ScalarSpeed) -> Result<AxisRates> {
    Err(AiError::UnspecifiedRule(
        "B44 flight-performance lookup for turn, pitch and roll rates",
    ))
}

/// Attitude the steering executor drives. Flight-path pitch is the velocity
/// direction; the body offset is added to it for the nose direction. B44:
/// nose direction and velocity direction must remain distinct inputs, so
/// consumers must never read one as the other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SteeringState {
    /// 0 through 360 exclusive, wrapped.
    pub heading_deg: f64,
    /// Direction of the velocity vector above the horizon.
    pub flight_path_pitch_deg: f64,
    /// Body bank; right positive.
    pub bank_deg: f64,
    /// Body pitch minus flight-path pitch. Steering does not change it; its
    /// producer is unresolved (B44).
    pub body_pitch_offset_deg: f64,
}
impl SteeringState {
    /// Nose direction above the horizon.
    pub fn body_pitch_deg(&self) -> f64 {
        self.flight_path_pitch_deg + self.body_pitch_offset_deg
    }

    /// Advance every axis toward `request` by `dt_s` simulation seconds under
    /// the effective B44 rates. A zero `dt_s` (pause) produces no motion.
    pub fn step(&self, request: &SteeringRequest, rates: AxisRates, dt_s: f64) -> Result<Self> {
        let heading_request = wrap_heading(request.heading_deg)?;
        let pitch_request = bound_pitch_request(
            request.flight_path_pitch_deg,
            request.at_ceiling,
            request.terrain_pitch_floor_deg,
        )?;
        let turn = TurnDirection::toward(self.heading_deg, heading_request);
        let effective = effective_rates(
            request.mode,
            rates,
            self.bank_deg,
            request.reference_bank_deg,
            turn,
        )?;
        Ok(Self {
            heading_deg: approach_angle(
                self.heading_deg,
                heading_request,
                effective.turn_deg_per_s,
                dt_s,
            )?,
            flight_path_pitch_deg: approach_bounded(
                self.flight_path_pitch_deg,
                pitch_request,
                effective.pitch_deg_per_s,
                dt_s,
            )?,
            bank_deg: approach_bounded(
                self.bank_deg,
                request.bank_deg,
                effective.roll_deg_per_s,
                dt_s,
            )?,
            body_pitch_offset_deg: self.body_pitch_offset_deg,
        })
    }
}

/// One steering command, already resolved by the maneuver layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SteeringRequest {
    pub heading_deg: f64,
    /// Flight-path pitch before B13/B44 bounding.
    pub flight_path_pitch_deg: f64,
    pub bank_deg: f64,
    /// Bank magnitude the roll-in rule measures against. Its producer (the
    /// commanded bank, or a per-aircraft reference) is unresolved in B44, so
    /// the caller supplies it explicitly.
    pub reference_bank_deg: f64,
    /// True when the aircraft is at its ceiling (B44: positive pitch becomes
    /// level flight). The ceiling test itself is the caller's.
    pub at_ceiling: bool,
    /// Minimum pitch demanded by terrain avoidance, if active. B44: terrain
    /// avoidance can raise the pitch request; the producer and the exact
    /// clearance are unknown.
    pub terrain_pitch_floor_deg: Option<f64>,
    pub mode: CommandMode,
}

/// Wrap a heading into 0 through 359.999 degrees (B13).
pub fn wrap_heading(heading_deg: f64) -> Result<f64> {
    finite(heading_deg, "heading must be finite")?;
    Ok(heading_deg.rem_euclid(360.0))
}

/// Signed shorter path from `current` to `requested`, in -180 through 180.
fn wrapped_delta(current_deg: f64, requested_deg: f64) -> f64 {
    (requested_deg - current_deg + 180.0).rem_euclid(360.0) - 180.0
}

fn finite(value: f64, what: &'static str) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(AiError::InvalidInput(what))
    }
}

fn check_step_inputs(max_rate_deg_per_s: f64, dt_s: f64) -> Result<()> {
    if !max_rate_deg_per_s.is_finite() || max_rate_deg_per_s < 0.0 {
        return Err(AiError::InvalidInput(
            "approach rate must be finite and non-negative",
        ));
    }
    if !dt_s.is_finite() || dt_s < 0.0 {
        return Err(AiError::InvalidInput(
            "time step must be finite and non-negative",
        ));
    }
    Ok(())
}

/// Move a signed difference toward zero by at most `rate * dt`, never past
/// the request (B44: the ordinary approach stops at the requested value).
fn limited_step(delta: f64, max_rate_deg_per_s: f64, dt_s: f64) -> f64 {
    delta.signum() * delta.abs().min(max_rate_deg_per_s * dt_s)
}

/// B44 heading approach: move `current_deg` toward `requested_deg` along the
/// shorter wrapped path, at most `max_rate_deg_per_s * dt_s`, stopping
/// exactly at the request. The result is wrapped into 0 through 360.
pub fn approach_angle(
    current_deg: f64,
    requested_deg: f64,
    max_rate_deg_per_s: f64,
    dt_s: f64,
) -> Result<f64> {
    check_step_inputs(max_rate_deg_per_s, dt_s)?;
    let current = wrap_heading(current_deg)?;
    let requested = wrap_heading(requested_deg)?;
    let delta = wrapped_delta(current, requested);
    let step = limited_step(delta, max_rate_deg_per_s, dt_s);
    if step == delta {
        Ok(requested)
    } else {
        Ok((current + step).rem_euclid(360.0))
    }
}

/// B44 approach for a non-wrapping axis (flight-path pitch, bank): move
/// toward the request along the number line at most `max_rate * dt`,
/// stopping exactly at the request.
pub fn approach_bounded(
    current_deg: f64,
    requested_deg: f64,
    max_rate_deg_per_s: f64,
    dt_s: f64,
) -> Result<f64> {
    check_step_inputs(max_rate_deg_per_s, dt_s)?;
    finite(current_deg, "current angle must be finite")?;
    finite(requested_deg, "requested angle must be finite")?;
    let delta = requested_deg - current_deg;
    let step = limited_step(delta, max_rate_deg_per_s, dt_s);
    if step == delta {
        Ok(requested_deg)
    } else {
        Ok(current_deg + step)
    }
}

/// B44 heading authority in the bank-dependent movement branch.
///
/// - Bank opposing the turn direction: zero heading progress. Fitted: the
///   spec says opposing bank "can suppress" progress without giving the
///   threshold, so any opposing bank suppresses.
/// - `|bank| < 7/8 * |reference|`: authority scales with bank magnitude,
///   floored at one quarter of `base_turn_rate`. Fitted: the scale is linear
///   in `|bank| / (7/8 * |reference|)` so it meets full authority
///   continuously at the threshold; the spec names the threshold and the
///   floor but not the curve.
/// - Otherwise full base authority.
///
/// `turn_direction` of `None` means no heading change is requested; the
/// authority then has no direction to oppose, and only the scaling and floor
/// apply. A zero reference bank is invalid input.
pub fn heading_authority(
    base_turn_rate_deg_per_s: f64,
    bank_deg: f64,
    reference_bank_deg: f64,
    turn_direction: Option<TurnDirection>,
) -> Result<f64> {
    if !base_turn_rate_deg_per_s.is_finite() || base_turn_rate_deg_per_s < 0.0 {
        return Err(AiError::InvalidInput(
            "base turn rate must be finite and non-negative",
        ));
    }
    finite(bank_deg, "bank must be finite")?;
    let reference = reference_bank_deg.abs();
    if !reference.is_finite() || reference == 0.0 {
        return Err(AiError::InvalidInput(
            "reference bank magnitude must be finite and nonzero",
        ));
    }
    let opposing = turn_direction.is_some_and(|turn| bank_deg * turn.sign() < 0.0);
    if opposing {
        return Ok(0.0);
    }
    let threshold = ROLL_IN_BANK_FRACTION * reference;
    let magnitude = bank_deg.abs();
    if magnitude >= threshold {
        return Ok(base_turn_rate_deg_per_s);
    }
    let scaled = base_turn_rate_deg_per_s * (magnitude / threshold);
    Ok(scaled.max(AUTHORITY_FLOOR_FRACTION * base_turn_rate_deg_per_s))
}

/// B44 pitch authority: depends on bank with a floor of one quarter of
/// `base_pitch_rate`. The spec gives the floor but not the curve. Fitted:
/// authority scales with `cos(bank)` (full when wings level, floored as the
/// lift vector rolls away from vertical). Replace when research closes the
/// curve; the floor is the only spec-derived part.
pub fn pitch_authority(base_pitch_rate_deg_per_s: f64, bank_deg: f64) -> Result<f64> {
    if !base_pitch_rate_deg_per_s.is_finite() || base_pitch_rate_deg_per_s < 0.0 {
        return Err(AiError::InvalidInput(
            "base pitch rate must be finite and non-negative",
        ));
    }
    finite(bank_deg, "bank must be finite")?;
    let scaled = base_pitch_rate_deg_per_s * bank_deg.to_radians().cos();
    Ok(scaled.max(AUTHORITY_FLOOR_FRACTION * base_pitch_rate_deg_per_s))
}

/// Apply the B44 command-mode rules to base authority.
///
/// - `ReducedRate`: heading and pitch divided by three; roll divided by three
///   and bounded to 10 through 30 deg/s.
/// - `OtherState`: roll halved and capped at 45 deg/s.
/// - `Ordinary`: unchanged (fitted, see [`CommandMode::Ordinary`]).
pub fn command_mode_rates(mode: CommandMode, rates: AxisRates) -> Result<AxisRates> {
    let rates = rates.validate()?;
    Ok(match mode {
        CommandMode::Ordinary => rates,
        CommandMode::ReducedRate => AxisRates {
            turn_deg_per_s: rates.turn_deg_per_s / REDUCED_RATE_DIVISOR,
            pitch_deg_per_s: rates.pitch_deg_per_s / REDUCED_RATE_DIVISOR,
            roll_deg_per_s: (rates.roll_deg_per_s / REDUCED_RATE_DIVISOR).clamp(
                REDUCED_RATE_ROLL_MIN_DEG_PER_S,
                REDUCED_RATE_ROLL_MAX_DEG_PER_S,
            ),
        },
        CommandMode::OtherState => AxisRates {
            roll_deg_per_s: (rates.roll_deg_per_s / 2.0).min(OTHER_STATE_ROLL_CAP_DEG_PER_S),
            ..rates
        },
    })
}

/// Effective per-tick authority: bank-dependent heading and pitch authority
/// from the base rates, then the command-mode scaling.
pub fn effective_rates(
    mode: CommandMode,
    rates: AxisRates,
    bank_deg: f64,
    reference_bank_deg: f64,
    turn_direction: Option<TurnDirection>,
) -> Result<AxisRates> {
    let rates = rates.validate()?;
    let banked = AxisRates {
        turn_deg_per_s: heading_authority(
            rates.turn_deg_per_s,
            bank_deg,
            reference_bank_deg,
            turn_direction,
        )?,
        pitch_deg_per_s: pitch_authority(rates.pitch_deg_per_s, bank_deg)?,
        roll_deg_per_s: rates.roll_deg_per_s,
    };
    command_mode_rates(mode, banked)
}

/// Bound a requested flight-path pitch (B13, B44): clamp to -90 through +90;
/// at the ceiling a positive request becomes level flight; a terrain floor
/// raises the request to at least that value. Fitted ordering: terrain is
/// applied after the ceiling rule so terrain avoidance is never cancelled by
/// the ceiling; the spec does not state their precedence. A terrain floor
/// outside the pitch bound is invalid input.
pub fn bound_pitch_request(
    requested_deg: f64,
    at_ceiling: bool,
    terrain_pitch_floor_deg: Option<f64>,
) -> Result<f64> {
    finite(requested_deg, "requested pitch must be finite")?;
    let mut pitch = requested_deg.clamp(-PITCH_REQUEST_LIMIT_DEG, PITCH_REQUEST_LIMIT_DEG);
    if at_ceiling && pitch > 0.0 {
        pitch = 0.0;
    }
    if let Some(floor) = terrain_pitch_floor_deg {
        if !floor.is_finite() || floor.abs() > PITCH_REQUEST_LIMIT_DEG {
            return Err(AiError::InvalidInput(
                "terrain pitch floor must lie within -90 through +90 degrees",
            ));
        }
        pitch = pitch.max(floor);
    }
    Ok(pitch)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    fn rates() -> AxisRates {
        AxisRates {
            turn_deg_per_s: 12.0,
            pitch_deg_per_s: 20.0,
            roll_deg_per_s: 90.0,
        }
    }

    #[test]
    fn roll_in_heading_progress_grows_with_bank_and_floors_at_one_quarter() {
        let reference = 60.0;
        let right = Some(TurnDirection::Right);
        let level = heading_authority(12.0, 0.0, reference, right).unwrap();
        assert!(close(level, 3.0), "{level}");
        let shallow = heading_authority(12.0, 10.0, reference, right).unwrap();
        assert!(close(shallow, 3.0), "{shallow}");
        let mid = heading_authority(12.0, 26.25, reference, right).unwrap();
        assert!(close(mid, 6.0), "{mid}");
        let deeper = heading_authority(12.0, 40.0, reference, right).unwrap();
        assert!(mid < deeper && deeper < 12.0, "{deeper}");
        let below = heading_authority(12.0, 52.4, reference, right).unwrap();
        assert!(below < 12.0, "{below}");
        let at = heading_authority(12.0, 52.5, reference, right).unwrap();
        assert!(close(at, 12.0), "{at}");
        let above = heading_authority(12.0, 60.0, reference, right).unwrap();
        assert!(close(above, 12.0), "{above}");
        let negative_reference = heading_authority(12.0, 60.0, -reference, right).unwrap();
        assert!(close(negative_reference, 12.0));
    }

    #[test]
    fn opposing_bank_suppresses_heading_progress() {
        let reference = 60.0;
        assert_eq!(
            heading_authority(12.0, -30.0, reference, Some(TurnDirection::Right)),
            Ok(0.0)
        );
        assert_eq!(
            heading_authority(12.0, 30.0, reference, Some(TurnDirection::Left)),
            Ok(0.0)
        );
        let aligned = heading_authority(12.0, -30.0, reference, Some(TurnDirection::Left)).unwrap();
        assert!(aligned > 0.0);
        let no_turn = heading_authority(12.0, -30.0, reference, None).unwrap();
        assert!(aligned == no_turn);
        assert!(heading_authority(12.0, 30.0, 0.0, None).is_err());
    }

    #[test]
    fn pitch_authority_floors_at_one_quarter() {
        assert!(close(pitch_authority(20.0, 0.0).unwrap(), 20.0));
        assert!(close(pitch_authority(20.0, 60.0).unwrap(), 10.0));
        assert!(close(pitch_authority(20.0, 90.0).unwrap(), 5.0));
        assert!(close(pitch_authority(20.0, 180.0).unwrap(), 5.0));
    }

    #[test]
    fn reduced_rate_divides_by_three_and_bounds_roll() {
        let r = command_mode_rates(CommandMode::ReducedRate, rates()).unwrap();
        assert!(close(r.turn_deg_per_s, 4.0));
        assert!(close(r.pitch_deg_per_s, 20.0 / 3.0));
        assert!(close(r.roll_deg_per_s, 30.0));
        let low = command_mode_rates(
            CommandMode::ReducedRate,
            AxisRates {
                roll_deg_per_s: 15.0,
                ..rates()
            },
        )
        .unwrap();
        assert!(close(low.roll_deg_per_s, 10.0));
        let inside = command_mode_rates(
            CommandMode::ReducedRate,
            AxisRates {
                roll_deg_per_s: 60.0,
                ..rates()
            },
        )
        .unwrap();
        assert!(close(inside.roll_deg_per_s, 20.0));
        let high = command_mode_rates(
            CommandMode::ReducedRate,
            AxisRates {
                roll_deg_per_s: 120.0,
                ..rates()
            },
        )
        .unwrap();
        assert!(close(high.roll_deg_per_s, 30.0));
    }

    #[test]
    fn other_state_halves_roll_and_caps_at_45() {
        let r = command_mode_rates(CommandMode::OtherState, rates()).unwrap();
        assert!(close(r.roll_deg_per_s, 45.0));
        assert!(close(r.turn_deg_per_s, 12.0));
        assert!(close(r.pitch_deg_per_s, 20.0));
        let fast = command_mode_rates(
            CommandMode::OtherState,
            AxisRates {
                roll_deg_per_s: 140.0,
                ..rates()
            },
        )
        .unwrap();
        assert!(close(fast.roll_deg_per_s, 45.0));
        let slow = command_mode_rates(
            CommandMode::OtherState,
            AxisRates {
                roll_deg_per_s: 40.0,
                ..rates()
            },
        )
        .unwrap();
        assert!(close(slow.roll_deg_per_s, 20.0));
        assert_eq!(
            command_mode_rates(CommandMode::Ordinary, rates()),
            Ok(rates())
        );
        assert!(
            command_mode_rates(
                CommandMode::Ordinary,
                AxisRates {
                    turn_deg_per_s: -1.0,
                    ..rates()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn heading_approach_takes_shorter_wrapped_path_without_overshoot() {
        assert!(close(approach_angle(350.0, 10.0, 10.0, 1.0).unwrap(), 0.0));
        assert!(close(approach_angle(350.0, 10.0, 15.0, 1.0).unwrap(), 5.0));
        assert!(close(approach_angle(350.0, 10.0, 30.0, 1.0).unwrap(), 10.0));
        assert!(close(
            approach_angle(350.0, 10.0, 1000.0, 1.0).unwrap(),
            10.0
        ));
        assert!(close(
            approach_angle(10.0, 350.0, 15.0, 1.0).unwrap(),
            355.0
        ));
        assert!(close(
            approach_angle(10.0, 350.0, 30.0, 1.0).unwrap(),
            350.0
        ));
        assert!(close(approach_angle(90.0, 90.0, 30.0, 1.0).unwrap(), 90.0));
        assert!(close(approach_angle(0.0, 180.0, 30.0, 1.0).unwrap(), 330.0));
        assert!(close(
            approach_angle(0.0, 6.0, 12.0, SECONDS_PER_TICK).unwrap(),
            0.1
        ));
        assert!(approach_angle(0.0, 6.0, -1.0, 1.0).is_err());
        assert!(approach_angle(0.0, 6.0, 1.0, -1.0).is_err());
    }

    #[test]
    fn bounded_approach_stops_at_request() {
        assert!(close(approach_bounded(0.0, 45.0, 20.0, 1.0).unwrap(), 20.0));
        assert!(close(approach_bounded(0.0, 45.0, 20.0, 3.0).unwrap(), 45.0));
        assert!(close(
            approach_bounded(30.0, -30.0, 90.0, 0.5).unwrap(),
            -15.0
        ));
        assert!(close(
            approach_bounded(30.0, -30.0, 90.0, 1.0).unwrap(),
            -30.0
        ));
        assert!(close(approach_bounded(5.0, 5.0, 90.0, 1.0).unwrap(), 5.0));
    }

    #[test]
    fn pitch_request_is_bounded_to_plus_minus_90() {
        assert_eq!(bound_pitch_request(120.0, false, None), Ok(90.0));
        assert_eq!(bound_pitch_request(-120.0, false, None), Ok(-90.0));
        assert_eq!(bound_pitch_request(90.0, false, None), Ok(90.0));
        assert_eq!(bound_pitch_request(-90.0, false, None), Ok(-90.0));
        assert_eq!(bound_pitch_request(45.0, false, None), Ok(45.0));
        assert!(bound_pitch_request(f64::NAN, false, None).is_err());
    }

    #[test]
    fn ceiling_levels_positive_pitch_only() {
        assert_eq!(bound_pitch_request(45.0, true, None), Ok(0.0));
        assert_eq!(bound_pitch_request(120.0, true, None), Ok(0.0));
        assert_eq!(bound_pitch_request(0.0, true, None), Ok(0.0));
        assert_eq!(bound_pitch_request(-45.0, true, None), Ok(-45.0));
    }

    #[test]
    fn terrain_floor_raises_pitch_request() {
        assert_eq!(bound_pitch_request(-45.0, false, Some(15.0)), Ok(15.0));
        assert_eq!(bound_pitch_request(30.0, false, Some(15.0)), Ok(30.0));
        assert_eq!(bound_pitch_request(45.0, true, Some(15.0)), Ok(15.0));
        assert_eq!(bound_pitch_request(45.0, false, Some(-60.0)), Ok(45.0));
        assert!(bound_pitch_request(0.0, false, Some(95.0)).is_err());
    }

    #[test]
    fn pause_produces_no_motion_and_step_moves_every_axis() {
        let state = SteeringState {
            heading_deg: 350.0,
            flight_path_pitch_deg: 0.0,
            bank_deg: 0.0,
            body_pitch_offset_deg: 3.0,
        };
        let request = SteeringRequest {
            heading_deg: 10.0,
            flight_path_pitch_deg: 20.0,
            bank_deg: 60.0,
            reference_bank_deg: 60.0,
            at_ceiling: false,
            terrain_pitch_floor_deg: None,
            mode: CommandMode::Ordinary,
        };
        let paused = state.step(&request, rates(), 0.0).unwrap();
        assert_eq!(paused, state);

        let moved = state.step(&request, rates(), 1.0).unwrap();
        // Wings level at the start of the tick: heading at the quarter floor.
        assert!(close(moved.heading_deg, 353.0), "{}", moved.heading_deg);
        assert!(close(moved.flight_path_pitch_deg, 20.0));
        assert!(close(moved.bank_deg, 60.0));
        assert!(close(moved.body_pitch_offset_deg, 3.0));
        assert!(close(moved.body_pitch_deg(), 23.0));

        let mut s = state;
        for _ in 0..(TICKS_PER_SECOND * 10) {
            s = s.step(&request, rates(), SECONDS_PER_TICK).unwrap();
        }
        assert!(close(s.heading_deg, 10.0), "{}", s.heading_deg);
        assert!(close(s.bank_deg, 60.0));
        assert!(close(s.flight_path_pitch_deg, 20.0));
    }

    #[test]
    fn step_suppresses_heading_while_banked_the_wrong_way() {
        let state = SteeringState {
            heading_deg: 0.0,
            flight_path_pitch_deg: 0.0,
            bank_deg: -30.0,
            body_pitch_offset_deg: 0.0,
        };
        let request = SteeringRequest {
            heading_deg: 90.0,
            flight_path_pitch_deg: 0.0,
            bank_deg: -30.0,
            reference_bank_deg: 60.0,
            at_ceiling: false,
            terrain_pitch_floor_deg: None,
            mode: CommandMode::Ordinary,
        };
        let moved = state.step(&request, rates(), 1.0).unwrap();
        assert!(close(moved.heading_deg, 0.0));
    }

    #[test]
    fn performance_lookup_is_unspecified() {
        assert!(matches!(
            performance_rates(ScalarSpeed(400.0)),
            Err(AiError::UnspecifiedRule(_))
        ));
    }

    #[test]
    fn turn_direction_follows_shorter_path() {
        assert_eq!(
            TurnDirection::toward(350.0, 10.0),
            Some(TurnDirection::Right)
        );
        assert_eq!(
            TurnDirection::toward(10.0, 350.0),
            Some(TurnDirection::Left)
        );
        assert_eq!(TurnDirection::toward(45.0, 45.0), None);
        assert_eq!(TurnDirection::toward(0.0, 180.0), Some(TurnDirection::Left));
    }
}
