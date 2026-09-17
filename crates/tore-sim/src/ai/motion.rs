//! B13: motion command input limits, basic maneuver builders and the
//! quarter-second deadline clock.
//!
//! Spec home: `docs/spec/ai.md`, "B13: Basic maneuvers and command limits".
//! Everything here is spec-derived unless a doc comment says otherwise. The
//! engagement pitch evaluator is unresolved (B10), so requests carry it as the
//! opaque [`PitchRequest::Engagement`] value instead of a number.

use super::{AiError, QUARTER_SECOND_TICKS, Result, ScalarSpeed, SpeedLimits};

/// B13: straight climb requests this pitch.
pub const STRAIGHT_CLIMB_PITCH_DEG: i32 = 45;
/// B13: straight dive requests this pitch.
pub const STRAIGHT_DIVE_PITCH_DEG: i32 = -45;
/// B13: straight dive falls back to straight flight below this AGL altitude.
pub const STRAIGHT_DIVE_MINIMUM_AGL_FT: f64 = 3000.0;
/// B13: left and right breaks turn by this many degrees from current heading.
pub const BREAK_TURN_DEG: i32 = 170;
/// B13: the duration operand is clamped to this many nominal seconds.
pub const MAX_DURATION_SECONDS: i32 = 15;
/// B13: quarter-second counts added per nominal second of duration.
pub const QUARTERS_PER_SECOND: u64 = 4;
/// B13: pitch early completion applies to requested pitches above this.
pub const PITCH_EARLY_COMPLETION_MIN_PITCH_DEG: i32 = 25;
/// B13: pitch early completion applies while scalar speed is at most minimum
/// speed plus this recovered-domain increment.
pub const PITCH_EARLY_COMPLETION_SPEED_MARGIN: f64 = 25.0;

/// Wraps an integer heading into 0 through 359 degrees (B13, executable
/// confirmed). Headings are integer degrees as recovered from the command
/// input path; sub-degree precision is not part of this contract.
pub fn wrap_heading_deg(heading_deg: i32) -> u16 {
    heading_deg.rem_euclid(360) as u16
}

/// Requested pitch. The engagement pitch evaluator depends on relative
/// altitude and aircraft limits and is not recovered (B10); callers pass the
/// opaque variant through and the consumer resolves it later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PitchRequest {
    /// Explicit pitch in degrees, bounded to -90 through +90 by the constructor.
    Explicit(i32),
    /// The unresolved engagement-pitch evaluator's output.
    Engagement,
}

/// Requested bank. B13 supports an unconstrained mode and explicit bank
/// bounded to -180 through +180 degrees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bank {
    Unconstrained,
    Explicit(i32),
}

/// Requested speed. Explicit requests are positive scalar speeds in the
/// recovered domain and are constrained by the aircraft's limits. Negative
/// `homepos` operands (B15 distance regulation) are a pursuit concern and do
/// not pass through this type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpeedRequest {
    Explicit(ScalarSpeed),
    Corner,
    Maximum,
}
impl SpeedRequest {
    /// Resolves the request against own limits: explicit requests are held
    /// within minimum through maximum speed. A negative explicit request is
    /// invalid input here rather than a regulation request.
    pub fn resolve(self, limits: SpeedLimits) -> Result<ScalarSpeed> {
        match self {
            Self::Explicit(speed) => {
                if speed.0 < 0.0 {
                    return Err(AiError::InvalidInput(
                        "negative speed operand is a B15 regulation request, not a speed",
                    ));
                }
                Ok(speed.max(limits.minimum).min(limits.maximum))
            }
            Self::Corner => Ok(limits.corner),
            Self::Maximum => Ok(limits.maximum),
        }
    }
}

/// Command duration after B13 operand clamping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Duration {
    /// Nominal simulation seconds, 1 through 15.
    Timed(u8),
    /// Operand zero: the command completes geometrically, it is not a no-op.
    Geometric,
}
impl Duration {
    /// Clamps a raw duration operand to 0 through 15 seconds; zero selects
    /// geometric completion.
    pub fn from_operand(seconds: i32) -> Self {
        match seconds.clamp(0, MAX_DURATION_SECONDS) {
            0 => Self::Geometric,
            n => Self::Timed(n as u8),
        }
    }
}

/// One motion command as accepted by the command input path (B13).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionRequest {
    pub heading_deg: u16,
    pub pitch: PitchRequest,
    pub bank: Bank,
    pub speed: SpeedRequest,
    pub duration: Duration,
}
impl MotionRequest {
    /// Applies the B13 input limits: heading wraps into 0 through 359,
    /// explicit pitch is bounded to -90 through +90, explicit bank to -180
    /// through +180. Bounding is applied as a clamp; whether the original
    /// rejects or clamps out-of-range pitch and bank operands is a research
    /// question, and no caller in this module produces one.
    pub fn new(
        heading_deg: i32,
        pitch: PitchRequest,
        bank: Bank,
        speed: SpeedRequest,
        duration: Duration,
    ) -> Self {
        let pitch = match pitch {
            PitchRequest::Explicit(deg) => PitchRequest::Explicit(deg.clamp(-90, 90)),
            PitchRequest::Engagement => PitchRequest::Engagement,
        };
        let bank = match bank {
            Bank::Explicit(deg) => Bank::Explicit(deg.clamp(-180, 180)),
            Bank::Unconstrained => Bank::Unconstrained,
        };
        Self {
            heading_deg: wrap_heading_deg(heading_deg),
            pitch,
            bank,
            speed,
            duration,
        }
    }
}

/// Inputs the B13 builders need that the maneuvers themselves do not fix.
/// The source's straight, climb, dive and break requests specify heading,
/// pitch and speed only; their bank and duration operands are not recorded in
/// the spec, so the caller supplies them explicitly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManeuverFrame {
    pub current_heading_deg: i32,
    pub bank: Bank,
    pub duration: Duration,
}

/// B13: straight flight requests current heading, engagement pitch and
/// maximum speed.
pub fn straight(frame: ManeuverFrame) -> MotionRequest {
    MotionRequest::new(
        frame.current_heading_deg,
        PitchRequest::Engagement,
        frame.bank,
        SpeedRequest::Maximum,
        frame.duration,
    )
}

/// B13: straight climb requests +45 degrees pitch, falling back to straight
/// flight when the B04 climb predicate (`can_climb`) rejects climbing.
pub fn straight_climb(frame: ManeuverFrame, can_climb: bool) -> MotionRequest {
    if !can_climb {
        return straight(frame);
    }
    MotionRequest::new(
        frame.current_heading_deg,
        PitchRequest::Explicit(STRAIGHT_CLIMB_PITCH_DEG),
        frame.bank,
        SpeedRequest::Maximum,
        frame.duration,
    )
}

/// B13: straight dive requests -45 degrees pitch and falls back to straight
/// flight below 3000 feet AGL (exactly 3000 dives).
pub fn straight_dive(frame: ManeuverFrame, own_agl_ft: f64) -> MotionRequest {
    if own_agl_ft < STRAIGHT_DIVE_MINIMUM_AGL_FT {
        return straight(frame);
    }
    MotionRequest::new(
        frame.current_heading_deg,
        PitchRequest::Explicit(STRAIGHT_DIVE_PITCH_DEG),
        frame.bank,
        SpeedRequest::Maximum,
        frame.duration,
    )
}

/// B13: left break requests current heading minus 170 degrees, engagement
/// pitch and corner speed.
pub fn break_left(frame: ManeuverFrame) -> MotionRequest {
    break_by(frame, -BREAK_TURN_DEG)
}

/// B13: right break requests current heading plus 170 degrees, engagement
/// pitch and corner speed.
pub fn break_right(frame: ManeuverFrame) -> MotionRequest {
    break_by(frame, BREAK_TURN_DEG)
}

fn break_by(frame: ManeuverFrame, turn_deg: i32) -> MotionRequest {
    MotionRequest::new(
        frame.current_heading_deg + turn_deg,
        PitchRequest::Engagement,
        frame.bank,
        SpeedRequest::Corner,
        frame.duration,
    )
}

/// Which side a turnaround chose (B13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakSide {
    Left,
    Right,
}

/// B13: a turnaround chooses the left or right break with equal probability.
pub fn turn_around(
    frame: ManeuverFrame,
    random: &mut super::DecisionRandom,
) -> (BreakSide, MotionRequest) {
    match random.choose(2) {
        0 => (BreakSide::Left, break_left(frame)),
        _ => (BreakSide::Right, break_right(frame)),
    }
}

/// The 120 Hz simulation tick counter the AI samples for command deadlines.
/// It only advances through [`CommandClock::advance`], so a paused simulation
/// that issues no ticks never moves it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommandClock {
    tick: u64,
}
impl CommandClock {
    pub fn at_tick(tick: u64) -> Self {
        Self { tick }
    }
    pub fn tick(self) -> u64 {
        self.tick
    }
    /// B13: nominal timings are evaluated on a quarter-second clock, the
    /// tick count divided by 30.
    pub fn quarter_count(self) -> u64 {
        self.tick / QUARTER_SECOND_TICKS
    }
    pub fn advance(&mut self, ticks: u64) {
        self.tick += ticks;
    }
}

/// A quarter-second count at which a timed command becomes eligible to
/// expire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Deadline(pub u64);

/// B13: a timed duration of N seconds adds 4N quarter-second counts to the
/// clock sampled at submission. Geometric completion has no deadline.
pub fn deadline_for(duration: Duration, submitted_at: CommandClock) -> Option<Deadline> {
    match duration {
        Duration::Timed(seconds) => Some(Deadline(
            submitted_at.quarter_count() + QUARTERS_PER_SECOND * u64::from(seconds),
        )),
        Duration::Geometric => None,
    }
}

/// B13: expiration is eligible once the clock reaches the deadline.
pub fn is_expired(deadline: Deadline, clock: CommandClock) -> bool {
    clock.quarter_count() >= deadline.0
}

/// The axis a zero-duration command completes on (B13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionAxis {
    Heading,
    Pitch,
    Bank,
}

/// B13: zero-duration motion chooses one completion axis from heading, pitch
/// and, when constrained, bank, using angular differences and aircraft rate
/// queries. Those queries and the selection rule are not recovered, so this
/// returns `UnspecifiedRule`; no angular tolerance is invented here.
pub fn select_completion_axis(_request: &MotionRequest) -> Result<CompletionAxis> {
    Err(AiError::UnspecifiedRule(
        "B13 zero-duration completion axis selection and rate queries",
    ))
}

/// B13: in the reviewed combat-state path, a requested pitch above 25
/// degrees can finish early when scalar speed is at most minimum speed plus
/// 25. This is the documented predicate only; other completion paths and the
/// noncombat/terrain exceptions are unresolved.
pub fn pitch_early_completion(
    requested_pitch_deg: i32,
    current_speed: ScalarSpeed,
    limits: SpeedLimits,
) -> bool {
    requested_pitch_deg > PITCH_EARLY_COMPLETION_MIN_PITCH_DEG
        && current_speed <= limits.minimum.plus(PITCH_EARLY_COMPLETION_SPEED_MARGIN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::DecisionRandom;

    fn frame(heading: i32) -> ManeuverFrame {
        ManeuverFrame {
            current_heading_deg: heading,
            bank: Bank::Unconstrained,
            duration: Duration::from_operand(6),
        }
    }

    fn limits() -> SpeedLimits {
        SpeedLimits {
            minimum: ScalarSpeed(200.0),
            maximum: ScalarSpeed(900.0),
            corner: ScalarSpeed(450.0),
        }
    }

    #[test]
    fn heading_wraps_into_0_through_359() {
        assert_eq!(wrap_heading_deg(0), 0);
        assert_eq!(wrap_heading_deg(359), 359);
        assert_eq!(wrap_heading_deg(360), 0);
        assert_eq!(wrap_heading_deg(-1), 359);
        assert_eq!(wrap_heading_deg(-170), 190);
        assert_eq!(wrap_heading_deg(10 + 170), 180);
        assert_eq!(wrap_heading_deg(200 + 170), 10);
    }

    #[test]
    fn constructor_bounds_pitch_and_bank() {
        let r = MotionRequest::new(
            725,
            PitchRequest::Explicit(120),
            Bank::Explicit(-250),
            SpeedRequest::Maximum,
            Duration::Geometric,
        );
        assert_eq!(r.heading_deg, 5);
        assert_eq!(r.pitch, PitchRequest::Explicit(90));
        assert_eq!(r.bank, Bank::Explicit(-180));
        let r = MotionRequest::new(
            0,
            PitchRequest::Explicit(-91),
            Bank::Explicit(181),
            SpeedRequest::Corner,
            Duration::Geometric,
        );
        assert_eq!(r.pitch, PitchRequest::Explicit(-90));
        assert_eq!(r.bank, Bank::Explicit(180));
        let r = MotionRequest::new(
            0,
            PitchRequest::Engagement,
            Bank::Unconstrained,
            SpeedRequest::Corner,
            Duration::Geometric,
        );
        assert_eq!(r.pitch, PitchRequest::Engagement);
        assert_eq!(r.bank, Bank::Unconstrained);
    }

    #[test]
    fn duration_operand_clamps_and_zero_is_geometric() {
        assert_eq!(Duration::from_operand(0), Duration::Geometric);
        assert_eq!(Duration::from_operand(-3), Duration::Geometric);
        assert_eq!(Duration::from_operand(1), Duration::Timed(1));
        assert_eq!(Duration::from_operand(15), Duration::Timed(15));
        assert_eq!(Duration::from_operand(16), Duration::Timed(15));
    }

    #[test]
    fn explicit_speed_is_held_within_limits() {
        let l = limits();
        assert_eq!(
            SpeedRequest::Explicit(ScalarSpeed(100.0)).resolve(l),
            Ok(ScalarSpeed(200.0))
        );
        assert_eq!(
            SpeedRequest::Explicit(ScalarSpeed(1000.0)).resolve(l),
            Ok(ScalarSpeed(900.0))
        );
        assert_eq!(
            SpeedRequest::Explicit(ScalarSpeed(500.0)).resolve(l),
            Ok(ScalarSpeed(500.0))
        );
        assert_eq!(SpeedRequest::Corner.resolve(l), Ok(ScalarSpeed(450.0)));
        assert_eq!(SpeedRequest::Maximum.resolve(l), Ok(ScalarSpeed(900.0)));
        assert!(
            SpeedRequest::Explicit(ScalarSpeed(-750.0))
                .resolve(l)
                .is_err()
        );
    }

    #[test]
    fn straight_requests_heading_engagement_pitch_and_maximum_speed() {
        let r = straight(frame(47));
        assert_eq!(r.heading_deg, 47);
        assert_eq!(r.pitch, PitchRequest::Engagement);
        assert_eq!(r.speed, SpeedRequest::Maximum);
        assert_eq!(r.duration, Duration::Timed(6));
    }

    #[test]
    fn climb_falls_back_to_straight_when_climbing_is_rejected() {
        let f = frame(90);
        assert_eq!(straight_climb(f, true).pitch, PitchRequest::Explicit(45));
        assert_eq!(straight_climb(f, true).speed, SpeedRequest::Maximum);
        assert_eq!(straight_climb(f, false), straight(f));
    }

    #[test]
    fn dive_falls_back_to_straight_below_3000_agl() {
        let f = frame(90);
        assert_eq!(straight_dive(f, 3000.0).pitch, PitchRequest::Explicit(-45));
        assert_eq!(straight_dive(f, 3001.0).pitch, PitchRequest::Explicit(-45));
        assert_eq!(straight_dive(f, 2999.0), straight(f));
    }

    #[test]
    fn breaks_turn_170_degrees_at_engagement_pitch_and_corner_speed() {
        let left = break_left(frame(10));
        assert_eq!(left.heading_deg, 200);
        assert_eq!(left.pitch, PitchRequest::Engagement);
        assert_eq!(left.speed, SpeedRequest::Corner);
        let right = break_right(frame(200));
        assert_eq!(right.heading_deg, 10);
        assert_eq!(right.speed, SpeedRequest::Corner);
    }

    #[test]
    fn turn_around_splits_evenly_between_breaks() {
        let mut random = DecisionRandom::seeded(11);
        let mut left = 0;
        let n = 20_000;
        for _ in 0..n {
            let (side, request) = turn_around(frame(100), &mut random);
            match side {
                BreakSide::Left => {
                    left += 1;
                    assert_eq!(request, break_left(frame(100)));
                }
                BreakSide::Right => assert_eq!(request, break_right(frame(100))),
            }
        }
        assert!((9_500..10_500).contains(&left), "{left}");
    }

    #[test]
    fn five_second_command_adds_twenty_quarter_counts() {
        let clock = CommandClock::at_tick(0);
        assert_eq!(deadline_for(Duration::Timed(5), clock), Some(Deadline(20)));
        assert_eq!(deadline_for(Duration::Geometric, clock), None);
        let later = CommandClock::at_tick(7 * QUARTER_SECOND_TICKS + 3);
        assert_eq!(deadline_for(Duration::Timed(5), later), Some(Deadline(27)));
    }

    #[test]
    fn expiration_is_first_tick_whose_quarter_count_reaches_the_deadline() {
        for submitted_tick in [0, 1, 29, 30, 31, 1234] {
            let submitted = CommandClock::at_tick(submitted_tick);
            let deadline = deadline_for(Duration::Timed(5), submitted).unwrap();
            let mut clock = submitted;
            while !is_expired(deadline, clock) {
                clock.advance(1);
            }
            assert_eq!(clock.quarter_count(), submitted.quarter_count() + 20);
            assert_eq!(clock.tick() % QUARTER_SECOND_TICKS, 0);
            let mut before = clock;
            before = CommandClock::at_tick(before.tick() - 1);
            assert!(!is_expired(deadline, before));
        }
    }

    #[test]
    fn clock_phase_makes_expiration_up_to_just_under_a_quarter_second_early() {
        let nominal_ticks = 5 * QUARTER_SECOND_TICKS * QUARTERS_PER_SECOND;
        let submitted = CommandClock::at_tick(0);
        let deadline = deadline_for(Duration::Timed(5), submitted).unwrap();
        let mut clock = submitted;
        while !is_expired(deadline, clock) {
            clock.advance(1);
        }
        assert_eq!(clock.tick() - submitted.tick(), nominal_ticks);

        let submitted = CommandClock::at_tick(QUARTER_SECOND_TICKS - 1);
        let deadline = deadline_for(Duration::Timed(5), submitted).unwrap();
        let mut clock = submitted;
        while !is_expired(deadline, clock) {
            clock.advance(1);
        }
        let elapsed = clock.tick() - submitted.tick();
        assert_eq!(elapsed, nominal_ticks - (QUARTER_SECOND_TICKS - 1));
        assert!(elapsed > nominal_ticks - QUARTER_SECOND_TICKS);
    }

    #[test]
    fn pause_without_ticks_never_expires_a_command() {
        let submitted = CommandClock::at_tick(600);
        let deadline = deadline_for(Duration::Timed(1), submitted).unwrap();
        let paused = submitted;
        for _ in 0..1_000 {
            assert!(!is_expired(deadline, paused));
        }
        let mut running = submitted;
        running.advance(4 * QUARTER_SECOND_TICKS);
        assert!(is_expired(deadline, running));
    }

    #[test]
    fn pitch_early_completion_boundaries() {
        let l = limits();
        let at_margin = ScalarSpeed(225.0);
        let above_margin = ScalarSpeed(226.0);
        assert!(pitch_early_completion(26, at_margin, l));
        assert!(pitch_early_completion(90, ScalarSpeed(200.0), l));
        assert!(!pitch_early_completion(25, at_margin, l));
        assert!(!pitch_early_completion(26, above_margin, l));
    }

    #[test]
    fn completion_axis_selection_is_unspecified() {
        let r = straight(frame(0));
        assert!(matches!(
            select_completion_axis(&r),
            Err(AiError::UnspecifiedRule(_))
        ));
    }
}
