//! B44: steering execution. Heading, flight-path pitch and body bank approach
//! their requested values over simulation time; they are never assigned
//! instantaneously (spec: `docs/spec/ai.md`, "B44: Steering execution and
//! pursuit lead"; request bounding and terrain completion also from B13).
//!
//! Everything here is a pure calculation. Nothing reads the flight adapters
//! or the renderer. The caller supplies the loaded G limit and roll limit the
//! flight model uses (after damage, hit-point and load reductions) and
//! [`AxisRates::from_limits`] turns them into turn and roll capability; the
//! terrain height 1000 ft ahead is host geometry and is supplied as a number.
//!
//! Provenance labels used below:
//!
//! - spec-derived: the rate-limited approach that stops exactly at the
//!   request, the turn-rate formula with its 125 ft/s speed floor and 40 deg/s
//!   cap, the turn radius with its 32767 ft cap, the seven-eighths
//!   reference-bank threshold and the reference bank as seven eighths of the
//!   aircraft's maximum bank, the quarter floors, the divide-by-three
//!   reduced-rate command with its 10 through 30 deg/s roll bound, the halved
//!   and 45 deg/s capped roll of other state branches, the -90 through +90
//!   pitch bound, ceiling level-off, the terrain floor (300 ft clearance from
//!   the record, 1.375 turn radii times the sine of the dive, 5 degree steps,
//!   at least +5 below clearance), its one second and quarter second cadence,
//!   the +20 deg/s pitch authority while it is active, the ground pitch hold
//!   and 35 deg/s ground turn floor, the 32 ft/s per second gravity speed
//!   change with its halving and climb guard, and the distinct body pitch
//!   offset.
//! - fitted: the shape of the "scales with bank magnitude" curve, the reading of "can suppress" as any opposing bank suppressing,
//!   the cosine pitch curve, the order of ceiling and terrain adjustments, the
//!   placement of the terrain pitch bonus and the ground turn floor after the
//!   bank and mode rules, the 180-degree tie-break, and an unmodified
//!   `Ordinary` mode. Each is named at its site.
//! - unspecified: the base pitch rate; the spec gives no formula for it
//!   separate from the G limit ([`pitch_rate_deg_per_s`]).

use super::{AiError, Result, ScalarSpeed, TICKS_PER_SECOND};

/// Seconds advanced by one fixed simulation tick.
pub const SECONDS_PER_TICK: f64 = 1.0 / TICKS_PER_SECOND as f64;

/// B44: below this fraction of the reference bank magnitude, heading
/// authority scales with bank; at or above it, full base authority applies.
pub const ROLL_IN_BANK_FRACTION: f64 = 7.0 / 8.0;
/// B44: the reference bank magnitude is the aircraft's own maximum bank;
/// the seven-eighths threshold above is measured against it.
pub const REFERENCE_BANK_FRACTION: f64 = 1.0;
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

/// B44 turn-rate gain: turn rate deg/s = 2500 * G limit / speed ft/s, the
/// original's own constant (executable-confirmed; 7 G at 500 ft/s gives
/// 35 deg/s and an 819 ft radius). It is far above the physical value.
pub const TURN_RATE_GAIN: f64 = 2500.0;
/// B44: speed is treated as at least this many ft/s in the turn-rate formula.
pub const TURN_RATE_SPEED_FLOOR_FPS: f64 = 125.0;
/// B44: turn rate is capped here, deg/s.
pub const TURN_RATE_CAP_DEG_PER_S: f64 = 40.0;
/// B44: turn radius is capped here, feet.
pub const TURN_RADIUS_CAP_FEET: f64 = 32767.0;

/// B44: the terrain floor's dive test uses this many turn radii.
pub const TERRAIN_DIVE_RADII: f64 = 1.375;
/// B44: candidate terrain-floor pitches are tested in steps of this size.
pub const TERRAIN_PITCH_STEP_DEG: f64 = 5.0;
/// B44: the floor is re-evaluated at this interval in ordinary flight, s.
pub const TERRAIN_REEVALUATION_ORDINARY_S: f64 = 1.0;
/// B44: the floor is re-evaluated at this interval when pitched below
/// [`TERRAIN_FAST_PITCH_DEG`] or below [`TERRAIN_FAST_AGL_FEET`], s.
pub const TERRAIN_REEVALUATION_FAST_S: f64 = 0.25;
/// B44: body pitch below this selects the fast cadence, degrees.
pub const TERRAIN_FAST_PITCH_DEG: f64 = -10.0;
/// B44: height above ground below this selects the fast cadence, feet.
pub const TERRAIN_FAST_AGL_FEET: f64 = 3000.0;
/// B44: pitch authority gained while the terrain floor is active, deg/s.
pub const TERRAIN_ACTIVE_PITCH_BONUS_DEG_PER_S: f64 = 20.0;

/// B44: turn authority on the ground is at least this, deg/s.
pub const GROUND_TURN_RATE_FLOOR_DEG_PER_S: f64 = 35.0;

/// B44: aircraft with the gravity flag change speed by this many ft/s per
/// second times the sine of flight-path pitch.
pub const GRAVITY_SPEED_RATE_FPS_PER_S: f64 = 32.0;
/// B44: the gravity speed change is halved when speed exceeds maximum speed
/// by more than this, ft/s.
pub const GRAVITY_HALVING_EXCESS_FPS: f64 = 100.0;

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

/// Per-aircraft turn, pitch and roll capability at the current speed, deg/s,
/// before the bank-dependent and command-mode rules. Rates are non-negative
/// magnitudes. Build it with [`AxisRates::from_limits`] from the loaded
/// control-limit block, or supply the fields directly in tests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisRates {
    pub turn_deg_per_s: f64,
    pub pitch_deg_per_s: f64,
    pub roll_deg_per_s: f64,
}
impl AxisRates {
    /// B44 performance selection. There is no separate AI performance table:
    /// the AI reads the same loaded G limit and roll limit the flight model
    /// uses, after damage, hit-point and load reductions, so the caller
    /// passes those reduced values.
    ///
    /// - `turn_deg_per_s` is [`turn_rate_deg_per_s`] at `speed`.
    /// - `roll_deg_per_s` is `roll_limit_deg_per_s` unmodified. The halving
    ///   with the 45 deg/s cap and the reduced-rate 10 through 30 bound are
    ///   applied later by [`command_mode_rates`], so they must not be applied
    ///   here as well.
    /// - `pitch_deg_per_s` is caller supplied; the spec gives no base pitch
    ///   rate formula ([`pitch_rate_deg_per_s`]).
    pub fn from_limits(
        g_limit: f64,
        speed: ScalarSpeed,
        roll_limit_deg_per_s: f64,
        pitch_deg_per_s: f64,
    ) -> Result<Self> {
        Self {
            turn_deg_per_s: turn_rate_deg_per_s(g_limit, speed)?,
            pitch_deg_per_s,
            roll_deg_per_s: roll_limit_deg_per_s,
        }
        .validate()
    }

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

/// B44 turn rate: [`TURN_RATE_GAIN`] times the current G limit divided by
/// speed in ft/s, with speed treated as at least 125 ft/s and the result
/// capped at 40 deg/s. A 7 G limit at 500 ft/s gives 35 deg/s.
pub fn turn_rate_deg_per_s(g_limit: f64, speed: ScalarSpeed) -> Result<f64> {
    if !g_limit.is_finite() || g_limit < 0.0 {
        return Err(AiError::InvalidInput(
            "G limit must be finite and non-negative",
        ));
    }
    finite(speed.0, "speed must be finite")?;
    let speed_fps = speed.0.max(TURN_RATE_SPEED_FLOOR_FPS);
    Ok((TURN_RATE_GAIN * g_limit / speed_fps).min(TURN_RATE_CAP_DEG_PER_S))
}

/// B44 base pitch rate. The spec gives no formula for it separate from the G
/// limit, so this always reports the rule as unspecified; callers supply the
/// pitch rate to [`AxisRates::from_limits`] from elsewhere until research
/// closes it.
pub fn pitch_rate_deg_per_s(_g_limit: f64, _speed: ScalarSpeed) -> Result<f64> {
    Err(AiError::UnspecifiedRule(
        "B44 base pitch rate derivation from the loaded limits",
    ))
}

/// B44 turn radius: speed divided by the turn rate in radians per second,
/// capped at 32767 ft. 500 ft/s at 35 deg/s gives 819 ft (818.5 unrounded).
/// A non-positive rate has no radius and is invalid input.
pub fn turn_radius_feet(speed: ScalarSpeed, turn_rate_deg_per_s: f64) -> Result<f64> {
    if !turn_rate_deg_per_s.is_finite() || turn_rate_deg_per_s <= 0.0 {
        return Err(AiError::InvalidInput(
            "turn rate must be finite and positive",
        ));
    }
    if !speed.0.is_finite() || speed.0 < 0.0 {
        return Err(AiError::InvalidInput(
            "speed must be finite and non-negative",
        ));
    }
    Ok((speed.0 / turn_rate_deg_per_s.to_radians()).min(TURN_RADIUS_CAP_FEET))
}

/// B44 reference bank for the bank-dependent heading authority: the
/// aircraft's own maximum bank. `maximum_bank_deg` is the aircraft's record
/// value; there is no default.
pub fn reference_bank_deg(maximum_bank_deg: f64) -> Result<f64> {
    if !maximum_bank_deg.is_finite() || maximum_bank_deg <= 0.0 {
        return Err(AiError::InvalidInput(
            "maximum bank must be finite and positive",
        ));
    }
    Ok(REFERENCE_BANK_FRACTION * maximum_bank_deg)
}

/// Inputs to the B44 terrain floor. All heights are feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainInputs {
    /// Own absolute altitude.
    pub altitude_feet: f64,
    /// Terrain height 1000 ft ahead of the aircraft. The lookup is host
    /// geometry; the caller performs it.
    pub terrain_ahead_feet: f64,
    /// The aircraft's minimum-altitude value (300 in every inspected record).
    /// Explicit input, no default.
    pub minimum_altitude_feet: f64,
    /// Current turn radius, from [`turn_radius_feet`].
    pub turn_radius_feet: f64,
}

/// Result of the B44 terrain floor evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainFloor {
    /// Lowest permitted flight-path pitch, degrees, a multiple of 5 within
    /// -90 through +90. Feed it to [`bound_pitch_request`].
    pub pitch_floor_deg: f64,
    /// Altitude surplus above the required clearance; negative below it.
    pub margin_feet: f64,
}

/// B44 terrain floor. The aircraft must stay `minimum_altitude_feet` above
/// the terrain 1000 ft ahead. With `margin = altitude - terrain - minimum`,
/// candidate pitches run from -90 to +90 in 5 degree steps and the first one
/// whose dive fits is taken: `1.375 * turn_radius * sin(-pitch) <= margin`.
/// Level flight is therefore permitted at exactly the clearance, a dive only
/// when its arc fits inside the surplus, and below the clearance the floor is
/// a climb of at least 5 degrees, steeper as the deficit grows. When no
/// candidate fits the floor is +90.
pub fn terrain_pitch_floor(inputs: &TerrainInputs) -> Result<TerrainFloor> {
    finite(inputs.altitude_feet, "altitude must be finite")?;
    finite(inputs.terrain_ahead_feet, "terrain height must be finite")?;
    finite(
        inputs.minimum_altitude_feet,
        "minimum altitude must be finite",
    )?;
    if !inputs.turn_radius_feet.is_finite() || inputs.turn_radius_feet < 0.0 {
        return Err(AiError::InvalidInput(
            "turn radius must be finite and non-negative",
        ));
    }
    let margin_feet =
        inputs.altitude_feet - inputs.terrain_ahead_feet - inputs.minimum_altitude_feet;
    let arc = TERRAIN_DIVE_RADII * inputs.turn_radius_feet;
    let steps = (2.0 * PITCH_REQUEST_LIMIT_DEG / TERRAIN_PITCH_STEP_DEG) as i32;
    let pitch_floor_deg = (0..=steps)
        .map(|i| -PITCH_REQUEST_LIMIT_DEG + f64::from(i) * TERRAIN_PITCH_STEP_DEG)
        .find(|pitch| arc * (-pitch).to_radians().sin() <= margin_feet)
        .unwrap_or(PITCH_REQUEST_LIMIT_DEG);
    Ok(TerrainFloor {
        pitch_floor_deg,
        margin_feet,
    })
}

/// B44 terrain floor cadence: re-evaluated once a second in ordinary flight
/// and four times a second when body pitch is below -10 degrees or the
/// aircraft is within 3000 ft of the ground.
pub fn terrain_reevaluation_seconds(body_pitch_deg: f64, agl_feet: f64) -> Result<f64> {
    finite(body_pitch_deg, "body pitch must be finite")?;
    finite(agl_feet, "height above ground must be finite")?;
    if body_pitch_deg < TERRAIN_FAST_PITCH_DEG || agl_feet < TERRAIN_FAST_AGL_FEET {
        Ok(TERRAIN_REEVALUATION_FAST_S)
    } else {
        Ok(TERRAIN_REEVALUATION_ORDINARY_S)
    }
}

/// B44 ground pitch override. On the ground the aircraft holds its entry
/// body pitch; it may pitch up above that only when faster than minimum
/// speed, or at any speed during the airborne part of a takeoff. A request
/// below the entry pitch never lowers it.
pub fn ground_pitch_override(
    entry_body_pitch_deg: f64,
    requested_pitch_deg: f64,
    speed: ScalarSpeed,
    minimum: ScalarSpeed,
    airborne_takeoff_part: bool,
) -> Result<f64> {
    finite(entry_body_pitch_deg, "entry pitch must be finite")?;
    finite(requested_pitch_deg, "requested pitch must be finite")?;
    finite(speed.0, "speed must be finite")?;
    finite(minimum.0, "minimum speed must be finite")?;
    let may_pitch_up = airborne_takeoff_part || speed > minimum;
    if may_pitch_up && requested_pitch_deg > entry_body_pitch_deg {
        Ok(requested_pitch_deg)
    } else {
        Ok(entry_body_pitch_deg)
    }
}

/// B44 gravity speed change over `dt_s` for aircraft with the gravity flag,
/// ft/s. Sign convention: climbing (positive flight-path pitch) loses speed,
/// diving gains it: `-32 * sin(pitch) * dt`. The change is halved when speed
/// exceeds `maximum` by more than 100 ft/s. While climbing the aircraft never
/// decelerates below `minimum`: at or below minimum the change is zero, and
/// above it the loss stops at minimum. Speed never falls below zero.
pub fn gravity_speed_delta_fps(
    flight_path_pitch_deg: f64,
    speed: ScalarSpeed,
    maximum: ScalarSpeed,
    minimum: ScalarSpeed,
    dt_s: f64,
) -> Result<f64> {
    finite(flight_path_pitch_deg, "flight-path pitch must be finite")?;
    finite(speed.0, "speed must be finite")?;
    finite(maximum.0, "maximum speed must be finite")?;
    finite(minimum.0, "minimum speed must be finite")?;
    if !dt_s.is_finite() || dt_s < 0.0 {
        return Err(AiError::InvalidInput(
            "time step must be finite and non-negative",
        ));
    }
    let mut delta = -GRAVITY_SPEED_RATE_FPS_PER_S * flight_path_pitch_deg.to_radians().sin() * dt_s;
    if speed.0 > maximum.0 + GRAVITY_HALVING_EXCESS_FPS {
        delta /= 2.0;
    }
    if flight_path_pitch_deg > 0.0 {
        delta = delta.max((minimum.0 - speed.0).min(0.0));
    }
    Ok(delta.max(-speed.0))
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
    /// The terrain floor counts as active exactly when the request carries a
    /// floor value.
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
            AuthorityModifiers {
                terrain_floor_active: request.terrain_pitch_floor_deg.is_some(),
                on_ground: request.on_ground,
            },
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
    /// Bank magnitude the roll-in rule measures against: seven eighths of
    /// the aircraft's maximum bank, from [`reference_bank_deg`].
    pub reference_bank_deg: f64,
    /// True when the aircraft is at its ceiling (B44: positive pitch becomes
    /// level flight). The ceiling test itself is the caller's.
    pub at_ceiling: bool,
    /// Lowest permitted pitch from [`terrain_pitch_floor`] while the terrain
    /// floor is enabled (B44: pursuit and route commands enable it); `None`
    /// when it is not. A value also grants the +20 deg/s pitch authority.
    pub terrain_pitch_floor_deg: Option<f64>,
    /// True while the aircraft is on the ground (B44: turn authority is then
    /// at least 35 deg/s). The pitch hold of [`ground_pitch_override`] needs
    /// the entry pitch and stays with the caller.
    pub on_ground: bool,
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
/// `reference_bank_deg` comes from [`reference_bank_deg`]. `turn_direction`
/// of `None` means no heading change is requested; the authority then has no
/// direction to oppose, and only the scaling and floor apply. A zero
/// reference bank is invalid input.
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

/// B44 situational modifiers to effective authority. Both flags are explicit
/// so that a caller never gets a bonus or a floor it did not ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorityModifiers {
    /// The terrain floor is active: pitch authority gains 20 deg/s.
    pub terrain_floor_active: bool,
    /// The aircraft is on the ground: turn authority is at least 35 deg/s.
    pub on_ground: bool,
}
impl AuthorityModifiers {
    /// Ordinary flight with neither modifier.
    pub const NONE: Self = Self {
        terrain_floor_active: false,
        on_ground: false,
    };
}

/// Effective per-tick authority: bank-dependent heading and pitch authority
/// from the base rates, then the command-mode scaling, then the situational
/// modifiers. Fitted ordering: the terrain pitch bonus is added and the
/// ground turn floor applied after the bank curve and the mode division, so
/// the bonus is the full 20 deg/s the spec states and the floor is the full
/// 35 deg/s; the spec does not state where in the chain they sit.
pub fn effective_rates(
    mode: CommandMode,
    rates: AxisRates,
    bank_deg: f64,
    reference_bank_deg: f64,
    turn_direction: Option<TurnDirection>,
    modifiers: AuthorityModifiers,
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
    let mut effective = command_mode_rates(mode, banked)?;
    if modifiers.terrain_floor_active {
        effective.pitch_deg_per_s += TERRAIN_ACTIVE_PITCH_BONUS_DEG_PER_S;
    }
    if modifiers.on_ground {
        effective.turn_deg_per_s = effective
            .turn_deg_per_s
            .max(GROUND_TURN_RATE_FLOOR_DEG_PER_S);
    }
    Ok(effective)
}

/// Bound a requested flight-path pitch (B13, B44): clamp to -90 through +90;
/// at the ceiling a positive request becomes level flight; a terrain floor
/// (from [`terrain_pitch_floor`]) raises the request to at least that value.
/// Fitted ordering: terrain is applied after the ceiling rule so terrain
/// avoidance is never cancelled by the ceiling; the spec does not state their
/// precedence. A terrain floor outside the pitch bound is invalid input.
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

    fn request() -> SteeringRequest {
        SteeringRequest {
            heading_deg: 10.0,
            flight_path_pitch_deg: 20.0,
            bank_deg: 60.0,
            reference_bank_deg: 60.0,
            at_ceiling: false,
            terrain_pitch_floor_deg: None,
            on_ground: false,
            mode: CommandMode::Ordinary,
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
    fn reference_bank_is_seven_eighths_of_maximum_bank() {
        assert!(close(reference_bank_deg(80.0).unwrap(), 80.0));
        assert!(close(reference_bank_deg(64.0).unwrap(), 64.0));
        assert!(reference_bank_deg(0.0).is_err());
        assert!(reference_bank_deg(-80.0).is_err());
        assert!(reference_bank_deg(f64::NAN).is_err());
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
    fn ported_fighters_roll_at_the_45_cap_and_a_bomber_at_15() {
        // B44: every ported fighter's roll limit is 180 deg/s or more, so
        // they all roll at the 45 deg/s cap; a B-52 (roll limit 30) rolls at 15.
        let fighter = AxisRates::from_limits(7.0, ScalarSpeed(500.0), 180.0, 20.0).unwrap();
        assert!(close(fighter.roll_deg_per_s, 180.0));
        let ordinary = command_mode_rates(CommandMode::OtherState, fighter).unwrap();
        assert!(close(ordinary.roll_deg_per_s, 45.0));
        let bomber = AxisRates::from_limits(2.0, ScalarSpeed(500.0), 30.0, 5.0).unwrap();
        let ordinary = command_mode_rates(CommandMode::OtherState, bomber).unwrap();
        assert!(close(ordinary.roll_deg_per_s, 15.0));
    }

    #[test]
    fn turn_rate_follows_g_limit_over_speed_with_floor_and_cap() {
        assert!(close(
            turn_rate_deg_per_s(7.0, ScalarSpeed(500.0)).unwrap(),
            35.0
        ));
        assert!(close(
            turn_rate_deg_per_s(7.0, ScalarSpeed(800.0)).unwrap(),
            21.875
        ));
        // 140 deg/s before the cap at 125 ft/s.
        assert!(close(
            turn_rate_deg_per_s(7.0, ScalarSpeed(125.0)).unwrap(),
            40.0
        ));
        // Below the speed floor the rate is the 125 ft/s rate.
        assert!(close(
            turn_rate_deg_per_s(7.0, ScalarSpeed(60.0)).unwrap(),
            40.0
        ));
        assert!(close(
            turn_rate_deg_per_s(1.0, ScalarSpeed(60.0)).unwrap(),
            turn_rate_deg_per_s(1.0, ScalarSpeed(125.0)).unwrap()
        ));
        assert!(close(
            turn_rate_deg_per_s(1.0, ScalarSpeed(125.0)).unwrap(),
            20.0
        ));
        assert!(close(
            turn_rate_deg_per_s(0.0, ScalarSpeed(500.0)).unwrap(),
            0.0
        ));
        assert!(turn_rate_deg_per_s(-1.0, ScalarSpeed(500.0)).is_err());
        assert!(turn_rate_deg_per_s(7.0, ScalarSpeed(f64::NAN)).is_err());
        let rates = AxisRates::from_limits(7.0, ScalarSpeed(500.0), 200.0, 20.0).unwrap();
        assert!(close(rates.turn_deg_per_s, 35.0));
        assert!(close(rates.roll_deg_per_s, 200.0));
        assert!(close(rates.pitch_deg_per_s, 20.0));
        assert!(AxisRates::from_limits(7.0, ScalarSpeed(500.0), 200.0, -1.0).is_err());
    }

    #[test]
    fn turn_radius_is_speed_over_rate_in_radians_with_cap() {
        let radius = turn_radius_feet(ScalarSpeed(500.0), 35.0).unwrap();
        assert!((radius - 819.0).abs() < 1.0, "{radius}");
        assert!(close(radius.round(), 819.0));
        // 500 ft/s at 0.5 deg/s would be 57296 ft; capped.
        assert!(close(
            turn_radius_feet(ScalarSpeed(500.0), 0.5).unwrap(),
            TURN_RADIUS_CAP_FEET
        ));
        assert!(turn_radius_feet(ScalarSpeed(500.0), 0.0).is_err());
        assert!(turn_radius_feet(ScalarSpeed(500.0), -5.0).is_err());
        assert!(turn_radius_feet(ScalarSpeed(-1.0), 5.0).is_err());
    }

    #[test]
    fn pitch_rate_derivation_is_unspecified() {
        assert!(matches!(
            pitch_rate_deg_per_s(7.0, ScalarSpeed(400.0)),
            Err(AiError::UnspecifiedRule(_))
        ));
    }

    fn terrain(margin_feet: f64, turn_radius_feet: f64) -> TerrainInputs {
        TerrainInputs {
            altitude_feet: 1500.0 + margin_feet,
            terrain_ahead_feet: 1200.0,
            minimum_altitude_feet: 300.0,
            turn_radius_feet,
        }
    }

    #[test]
    fn terrain_floor_permits_a_dive_only_when_its_arc_fits_the_surplus() {
        const R: f64 = 819.0;
        let full_arc = TERRAIN_DIVE_RADII * R;
        let vertical = terrain_pitch_floor(&terrain(full_arc + 0.5, R)).unwrap();
        assert!(close(vertical.pitch_floor_deg, -90.0));
        assert!(close(vertical.margin_feet, full_arc + 0.5));
        let almost = terrain_pitch_floor(&terrain(full_arc - 0.5, R)).unwrap();
        assert!(close(almost.pitch_floor_deg, -85.0));
        // A 30 degree dive needs 1.375 * 819 * sin 30 = 563.06 ft.
        let thirty = terrain_pitch_floor(&terrain(563.1, R)).unwrap();
        assert!(close(thirty.pitch_floor_deg, -30.0));
        let short = terrain_pitch_floor(&terrain(563.0, R)).unwrap();
        assert!(close(short.pitch_floor_deg, -25.0));
    }

    #[test]
    fn terrain_floor_allows_level_flight_at_clearance_and_climbs_below_it() {
        const R: f64 = 819.0;
        let level = terrain_pitch_floor(&terrain(0.0, R)).unwrap();
        assert!(close(level.pitch_floor_deg, 0.0));
        let just_below = terrain_pitch_floor(&terrain(-1.0, R)).unwrap();
        assert!(close(just_below.pitch_floor_deg, 5.0));
        // 5 degrees recovers 98 ft, 10 recovers 196; a 150 ft deficit needs 10.
        let deficit = terrain_pitch_floor(&terrain(-150.0, R)).unwrap();
        assert!(close(deficit.pitch_floor_deg, 10.0));
        let deep = terrain_pitch_floor(&terrain(-600.0, R)).unwrap();
        assert!(close(deep.pitch_floor_deg, 35.0));
        // Deeper than 1.375 R below the clearance nothing fits.
        let hopeless = terrain_pitch_floor(&terrain(-2000.0, R)).unwrap();
        assert!(close(hopeless.pitch_floor_deg, 90.0));
        assert!(bound_pitch_request(-45.0, false, Some(hopeless.pitch_floor_deg)).unwrap() == 90.0);
        // A zero radius makes every dive fit above clearance and none below.
        let zero_radius = terrain_pitch_floor(&terrain(1.0, 0.0)).unwrap();
        assert!(close(zero_radius.pitch_floor_deg, -90.0));
        let zero_below = terrain_pitch_floor(&terrain(-1.0, 0.0)).unwrap();
        assert!(close(zero_below.pitch_floor_deg, 90.0));
        assert!(terrain_pitch_floor(&terrain(0.0, -1.0)).is_err());
        assert!(terrain_pitch_floor(&terrain(f64::NAN, R)).is_err());
    }

    #[test]
    fn terrain_reevaluation_is_quarter_second_when_nose_down_or_low() {
        assert_eq!(terrain_reevaluation_seconds(0.0, 5000.0), Ok(1.0));
        assert_eq!(terrain_reevaluation_seconds(-10.0, 5000.0), Ok(1.0));
        assert_eq!(terrain_reevaluation_seconds(-11.0, 5000.0), Ok(0.25));
        assert_eq!(terrain_reevaluation_seconds(0.0, 3000.0), Ok(1.0));
        assert_eq!(terrain_reevaluation_seconds(0.0, 2999.0), Ok(0.25));
        assert_eq!(terrain_reevaluation_seconds(30.0, 100.0), Ok(0.25));
        assert!(terrain_reevaluation_seconds(f64::NAN, 100.0).is_err());
    }

    #[test]
    fn terrain_floor_adds_pitch_authority_and_ground_floors_turn_rate() {
        let plain = effective_rates(
            CommandMode::Ordinary,
            rates(),
            0.0,
            60.0,
            None,
            AuthorityModifiers::NONE,
        )
        .unwrap();
        assert!(close(plain.pitch_deg_per_s, 20.0));
        assert!(close(plain.turn_deg_per_s, 3.0));
        let terrain = effective_rates(
            CommandMode::Ordinary,
            rates(),
            0.0,
            60.0,
            None,
            AuthorityModifiers {
                terrain_floor_active: true,
                on_ground: false,
            },
        )
        .unwrap();
        assert!(close(terrain.pitch_deg_per_s, 40.0));
        assert!(close(terrain.turn_deg_per_s, 3.0));
        let ground = effective_rates(
            CommandMode::Ordinary,
            rates(),
            0.0,
            60.0,
            Some(TurnDirection::Right),
            AuthorityModifiers {
                terrain_floor_active: false,
                on_ground: true,
            },
        )
        .unwrap();
        assert!(close(ground.turn_deg_per_s, 35.0));
        assert!(close(ground.pitch_deg_per_s, 20.0));
        let fast_ground = effective_rates(
            CommandMode::Ordinary,
            AxisRates {
                turn_deg_per_s: 160.0,
                ..rates()
            },
            60.0,
            60.0,
            Some(TurnDirection::Right),
            AuthorityModifiers {
                terrain_floor_active: false,
                on_ground: true,
            },
        )
        .unwrap();
        assert!(close(fast_ground.turn_deg_per_s, 160.0));
    }

    #[test]
    fn ground_override_holds_entry_pitch_unless_pitching_up_is_allowed() {
        let minimum = ScalarSpeed(150.0);
        // Slow on the ground: hold entry pitch whatever is requested.
        assert_eq!(
            ground_pitch_override(2.0, 15.0, ScalarSpeed(100.0), minimum, false),
            Ok(2.0)
        );
        assert_eq!(
            ground_pitch_override(2.0, -10.0, ScalarSpeed(100.0), minimum, false),
            Ok(2.0)
        );
        // Exactly minimum is not above it.
        assert_eq!(
            ground_pitch_override(2.0, 15.0, ScalarSpeed(150.0), minimum, false),
            Ok(2.0)
        );
        // Above minimum: may pitch up, never down.
        assert_eq!(
            ground_pitch_override(2.0, 15.0, ScalarSpeed(151.0), minimum, false),
            Ok(15.0)
        );
        assert_eq!(
            ground_pitch_override(2.0, -10.0, ScalarSpeed(200.0), minimum, false),
            Ok(2.0)
        );
        // Airborne takeoff part: may pitch up at any speed.
        assert_eq!(
            ground_pitch_override(2.0, 15.0, ScalarSpeed(100.0), minimum, true),
            Ok(15.0)
        );
        assert_eq!(
            ground_pitch_override(2.0, -10.0, ScalarSpeed(100.0), minimum, true),
            Ok(2.0)
        );
        assert!(ground_pitch_override(f64::NAN, 15.0, ScalarSpeed(100.0), minimum, true).is_err());
    }

    #[test]
    fn gravity_changes_speed_by_32_times_sine_of_pitch() {
        let maximum = ScalarSpeed(900.0);
        let minimum = ScalarSpeed(150.0);
        let climb =
            gravity_speed_delta_fps(30.0, ScalarSpeed(500.0), maximum, minimum, 1.0).unwrap();
        assert!(close(climb, -16.0), "{climb}");
        let dive =
            gravity_speed_delta_fps(-30.0, ScalarSpeed(500.0), maximum, minimum, 1.0).unwrap();
        assert!(close(dive, 16.0), "{dive}");
        let level =
            gravity_speed_delta_fps(0.0, ScalarSpeed(500.0), maximum, minimum, 1.0).unwrap();
        assert!(close(level, 0.0));
        let tick = gravity_speed_delta_fps(
            -90.0,
            ScalarSpeed(500.0),
            maximum,
            minimum,
            SECONDS_PER_TICK,
        )
        .unwrap();
        assert!(close(tick, 32.0 / TICKS_PER_SECOND as f64));
        assert!(gravity_speed_delta_fps(30.0, ScalarSpeed(500.0), maximum, minimum, -1.0).is_err());
    }

    #[test]
    fn gravity_change_halves_above_maximum_plus_100() {
        let maximum = ScalarSpeed(900.0);
        let minimum = ScalarSpeed(150.0);
        let at_edge =
            gravity_speed_delta_fps(-30.0, ScalarSpeed(1000.0), maximum, minimum, 1.0).unwrap();
        assert!(close(at_edge, 16.0));
        let over =
            gravity_speed_delta_fps(-30.0, ScalarSpeed(1001.0), maximum, minimum, 1.0).unwrap();
        assert!(close(over, 8.0));
        let over_climb =
            gravity_speed_delta_fps(30.0, ScalarSpeed(1001.0), maximum, minimum, 1.0).unwrap();
        assert!(close(over_climb, -8.0));
    }

    #[test]
    fn gravity_never_decelerates_below_minimum_while_climbing() {
        let maximum = ScalarSpeed(900.0);
        let minimum = ScalarSpeed(150.0);
        let at_minimum = gravity_speed_delta_fps(30.0, minimum, maximum, minimum, 1.0).unwrap();
        assert!(close(at_minimum, 0.0));
        let below =
            gravity_speed_delta_fps(30.0, ScalarSpeed(120.0), maximum, minimum, 1.0).unwrap();
        assert!(close(below, 0.0));
        // 10 ft/s above minimum: the 16 ft/s loss stops at minimum.
        let near =
            gravity_speed_delta_fps(30.0, ScalarSpeed(160.0), maximum, minimum, 1.0).unwrap();
        assert!(close(near, -10.0));
        // Diving below minimum still gains speed.
        let dive_slow =
            gravity_speed_delta_fps(-30.0, ScalarSpeed(120.0), maximum, minimum, 1.0).unwrap();
        assert!(close(dive_slow, 16.0));
        // Speed floors at zero.
        let stalled =
            gravity_speed_delta_fps(90.0, ScalarSpeed(5.0), maximum, ScalarSpeed(0.0), 1.0)
                .unwrap();
        assert!(close(stalled, -5.0));
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
        // The computed floor feeds the bound directly.
        let floor = terrain_pitch_floor(&terrain(-150.0, 819.0)).unwrap();
        assert_eq!(
            bound_pitch_request(-45.0, false, Some(floor.pitch_floor_deg)),
            Ok(10.0)
        );
    }

    #[test]
    fn pause_produces_no_motion_and_step_moves_every_axis() {
        let state = SteeringState {
            heading_deg: 350.0,
            flight_path_pitch_deg: 0.0,
            bank_deg: 0.0,
            body_pitch_offset_deg: 3.0,
        };
        let request = request();
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
            ..request()
        };
        let moved = state.step(&request, rates(), 1.0).unwrap();
        assert!(close(moved.heading_deg, 0.0));
    }

    #[test]
    fn step_applies_terrain_bonus_and_ground_floor() {
        let state = SteeringState {
            heading_deg: 0.0,
            flight_path_pitch_deg: -40.0,
            bank_deg: 0.0,
            body_pitch_offset_deg: 0.0,
        };
        let terrain = SteeringRequest {
            heading_deg: 0.0,
            flight_path_pitch_deg: -45.0,
            bank_deg: 0.0,
            terrain_pitch_floor_deg: Some(10.0),
            ..request()
        };
        // Base pitch 20 deg/s plus the 20 deg/s terrain bonus over one second,
        // toward the raised request of +10.
        let moved = state.step(&terrain, rates(), 1.0).unwrap();
        assert!(
            close(moved.flight_path_pitch_deg, 0.0),
            "{}",
            moved.flight_path_pitch_deg
        );
        let ground = SteeringRequest {
            heading_deg: 90.0,
            flight_path_pitch_deg: 0.0,
            bank_deg: 0.0,
            on_ground: true,
            ..request()
        };
        // Wings level would leave heading at the quarter floor (3 deg/s); the
        // ground floor lifts it to 35.
        let taxi = state.step(&ground, rates(), 1.0).unwrap();
        assert!(close(taxi.heading_deg, 35.0), "{}", taxi.heading_deg);
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
