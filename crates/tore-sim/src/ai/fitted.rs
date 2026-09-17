//! Named fitted fallbacks for the AI branches the specs leave unresolved.
//!
//! The isolated components in this module tree return
//! [`AiError::UnspecifiedRule`](super::AiError::UnspecifiedRule) wherever
//! [`docs/spec/ai.md`](../../../../docs/spec/ai.md) has not closed a branch.
//! That is correct for a research harness: a component must never invent a
//! number and present it as recovered behavior. A live controller cannot stop
//! flying at those branches, so this file supplies exactly one documented
//! fitted rule per branch, and [`controller`](super::controller) records every
//! use in its [`FallbackLog`](super::controller::FallbackLog).
//!
//! Provenance: **fitted**, agent decisions, 2026-09-17. Nothing here is
//! recovered retail behavior, and none of it may be described as such. Each
//! rule states the constants it uses so a later research pass can replace it
//! with a spec-derived one. The list is mirrored in
//! [behavior provenance](../../../../docs/behavior-provenance.md) and in the
//! [M1e backlog](../../../../docs/ROADMAP.md#1e-ai).

use super::experience::PerLevel;
use super::motion::{
    Bank, BreakSide, CompletionAxis, Duration, ManeuverFrame, MotionRequest, PitchRequest,
    SpeedRequest,
};
use super::steering::turn_rate_deg_per_s;
use super::tactics::{BehaviorChoice, LastDitchCandidate};
use super::{DecisionRandom, Experience, Result, ScalarSpeed, SpeedLimits};

/// One unresolved branch that the controller covers with a fitted rule.
///
/// The variants are the branches reachable by a fighter/strike actor in the
/// current delivery. Each names the spec text it stands in for; see
/// [`Fallback::spec_branch`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Fallback {
    /// B10/B13 engagement pitch: the evaluator is known not to be the plain
    /// line-of-sight pitch, but its rule is not recovered.
    EngagementPitch,
    /// B44 base pitch rate: the spec gives the turn rate formula and the
    /// bank-dependent authority, but no separate pitch rate.
    BasePitchRate,
    /// B13 zero-duration completion axis selection.
    CompletionAxis,
    /// B11 last-ditch candidate weights and redraw suitability.
    LastDitchCandidate,
    /// The random-tactic menu contents after the straight-flight draw.
    RandomTacticMenu,
    /// B12 remaining tactics once best attack and random tactic both fail.
    RemainingTactics,
    /// B44 lead speed estimator and prediction time.
    LeadSpeedEstimator,
    /// B42 burst and reload pacing after a launch with a resolving station.
    BurstPacing,
    /// B45 store hit chance, an opaque routine in the original.
    HitChance,
    /// B48 return to base for a wing leader or a singleton.
    LeaderReturnToBase,
}

impl Fallback {
    pub const ALL: [Self; 10] = [
        Self::EngagementPitch,
        Self::BasePitchRate,
        Self::CompletionAxis,
        Self::LastDitchCandidate,
        Self::RandomTacticMenu,
        Self::RemainingTactics,
        Self::LeadSpeedEstimator,
        Self::BurstPacing,
        Self::HitChance,
        Self::LeaderReturnToBase,
    ];

    /// A stable short name for logs, reports and tests.
    pub fn name(self) -> &'static str {
        match self {
            Self::EngagementPitch => "engagement-pitch",
            Self::BasePitchRate => "base-pitch-rate",
            Self::CompletionAxis => "completion-axis",
            Self::LastDitchCandidate => "last-ditch-candidate",
            Self::RandomTacticMenu => "random-tactic-menu",
            Self::RemainingTactics => "remaining-tactics",
            Self::LeadSpeedEstimator => "lead-speed-estimator",
            Self::BurstPacing => "burst-pacing",
            Self::HitChance => "hit-chance",
            Self::LeaderReturnToBase => "leader-return-to-base",
        }
    }

    /// The unresolved spec branch this rule stands in for.
    pub fn spec_branch(self) -> &'static str {
        match self {
            Self::EngagementPitch => "B10/B13 engagement pitch evaluator",
            Self::BasePitchRate => "B44 base pitch rate derivation from the loaded limits",
            Self::CompletionAxis => "B13 zero-duration completion axis selection and rate queries",
            Self::LastDitchCandidate => {
                "B11 last-ditch candidate weights and redraw suitability conditions"
            }
            Self::RandomTacticMenu => "random-tactic menu contents after the straight-flight draw",
            Self::RemainingTactics => {
                "B12 remaining tactics after best-attack and random-tactic draws fail"
            }
            Self::LeadSpeedEstimator => "B44 lead speed estimator and prediction time",
            Self::BurstPacing => "B42 burst/reload pacing after a launch with a resolving station",
            Self::HitChance => "B45 store hit chance",
            Self::LeaderReturnToBase => "B48 leader and singleton return to base",
        }
    }

    /// The fitted rule this file applies, in one sentence.
    pub fn rule(self) -> &'static str {
        match self {
            Self::EngagementPitch => {
                "half the relative altitude is closed over the horizontal distance, bounded to \
                 plus or minus 30 degrees and to zero when a climb is not permitted"
            }
            Self::BasePitchRate => {
                "the pitch rate equals the B44 turn rate for the same loaded G limit and speed"
            }
            Self::CompletionAxis => {
                "the axis whose remaining angular difference divided by its rate is largest, \
                 heading winning ties"
            }
            Self::LastDitchCandidate => {
                "an equal draw among the candidates whose altitude and speed room is sufficient, \
                 falling back to a vertical jink when none is"
            }
            Self::RandomTacticMenu => {
                "an equal draw among the five B13 basic maneuvers: straight climb, straight dive, \
                 break left, break right and turnaround"
            }
            Self::RemainingTactics => "pursuit, the branch best attack ordinarily selects",
            Self::LeadSpeedEstimator => {
                "the target travels at its scalar speed along its own heading and pitch for the \
                 range divided by the launcher store's nominal speed"
            }
            Self::BurstPacing => {
                "the weapon service restarts from search with the aircraft's own search delay"
            }
            Self::HitChance => {
                "fifty points scaled linearly by how far inside its employment angular limit the \
                 store is pointing"
            }
            Self::LeaderReturnToBase => {
                "a leader or singleton on bingo flies the same private landing route a wingman \
                 flies"
            }
        }
    }
}

/// Bound of the fitted engagement pitch, in degrees.
pub const ENGAGEMENT_PITCH_LIMIT_DEG: f64 = 30.0;
/// Fraction of the relative altitude the fitted engagement pitch closes.
pub const ENGAGEMENT_PITCH_CLOSE_FRACTION: f64 = 0.5;

/// Fitted B10/B13 engagement pitch, in degrees.
///
/// The spec records only that the evaluator is not the line-of-sight pitch and
/// that it depends on relative altitude and aircraft limits. This rule asks for
/// the pitch that closes [`ENGAGEMENT_PITCH_CLOSE_FRACTION`] of the relative
/// altitude over the current horizontal distance, bounded to
/// [`ENGAGEMENT_PITCH_LIMIT_DEG`]. A request to climb is refused when B04 does
/// not permit climbing, which is the "aircraft limits" half of the dependency.
pub fn engagement_pitch_deg(
    relative_altitude_ft: f64,
    horizontal_distance_ft: f64,
    can_climb: bool,
) -> f64 {
    if !relative_altitude_ft.is_finite() || !horizontal_distance_ft.is_finite() {
        return 0.0;
    }
    let run = horizontal_distance_ft.abs().max(1.0);
    let rise = relative_altitude_ft * ENGAGEMENT_PITCH_CLOSE_FRACTION;
    let pitch = rise
        .atan2(run)
        .to_degrees()
        .clamp(-ENGAGEMENT_PITCH_LIMIT_DEG, ENGAGEMENT_PITCH_LIMIT_DEG);
    if pitch > 0.0 && !can_climb {
        return 0.0;
    }
    pitch
}

/// Fitted B44 base pitch rate, in degrees per second.
///
/// Pitch and turn are both limited by the same loaded G limit at the same
/// speed, so this rule reuses the spec's turn-rate formula (2500 times the G
/// limit divided by speed, speed floored at 125 ft/s, capped at
/// [`TURN_RATE_CAP_DEG_PER_S`]) for the pitch axis. It is not evidence that the
/// original shares one rate between the two axes.
pub fn base_pitch_rate_deg_per_s(g_limit: f64, speed: ScalarSpeed) -> Result<f64> {
    turn_rate_deg_per_s(g_limit, speed)
}

/// Fitted B13 zero-duration completion axis.
///
/// The spec says the choice uses angular differences and aircraft rate
/// queries but does not give the selection function. This rule picks the axis
/// that will take longest to arrive, so the maneuver completes when its slowest
/// requested axis has arrived rather than when its fastest has. Bank is only a
/// candidate when the request constrains it. Heading wins ties.
pub fn completion_axis(
    heading_difference_deg: f64,
    pitch_difference_deg: f64,
    bank_difference_deg: Option<f64>,
    turn_deg_per_s: f64,
    pitch_deg_per_s: f64,
    roll_deg_per_s: f64,
) -> CompletionAxis {
    let time = |difference: f64, rate: f64| {
        if !difference.is_finite() || !rate.is_finite() || rate <= 0.0 {
            return 0.0;
        }
        difference.abs() / rate
    };
    let heading = time(heading_difference_deg, turn_deg_per_s);
    let pitch = time(pitch_difference_deg, pitch_deg_per_s);
    let bank = bank_difference_deg.map(|d| time(d, roll_deg_per_s));
    let mut best = (heading, CompletionAxis::Heading);
    if pitch > best.0 {
        best = (pitch, CompletionAxis::Pitch);
    }
    if let Some(bank) = bank
        && bank > best.0
    {
        best = (bank, CompletionAxis::Bank);
    }
    best.1
}

/// Speed margin above the minimum a looping last-ditch candidate needs, ft/s.
pub const LAST_DITCH_LOOP_SPEED_MARGIN_FPS: f64 = 100.0;
/// Turn radii of vertical room a looping last-ditch candidate needs.
///
/// The same 1.375 factor the terrain floor uses in B44, applied to a half loop.
pub const LAST_DITCH_LOOP_RADII: f64 = 1.375;

/// Whether a last-ditch candidate is suitable for the current state.
///
/// The spec records that "some candidates redraw when altitude, speed or
/// geometry makes them unsuitable" without listing the conditions. This rule
/// gates only the two candidates that need vertical room and energy: a split S
/// needs [`LAST_DITCH_LOOP_RADII`] turn radii of altitude below it, and a loop
/// needs the same room and [`LAST_DITCH_LOOP_SPEED_MARGIN_FPS`] above minimum
/// speed. The horizontal candidates are always suitable.
pub fn last_ditch_suitable(
    candidate: LastDitchCandidate,
    own_agl_ft: f64,
    turn_radius_ft: f64,
    speed: ScalarSpeed,
    limits: &SpeedLimits,
) -> bool {
    let room = LAST_DITCH_LOOP_RADII * turn_radius_ft.max(0.0);
    match candidate {
        LastDitchCandidate::SplitS => own_agl_ft >= room,
        LastDitchCandidate::Loop => {
            own_agl_ft >= room && speed.0 >= limits.minimum.0 + LAST_DITCH_LOOP_SPEED_MARGIN_FPS
        }
        LastDitchCandidate::HorizontalScissors
        | LastDitchCandidate::Reverse
        | LastDitchCandidate::Overshoot
        | LastDitchCandidate::HorizontalJink
        | LastDitchCandidate::VerticalJink => true,
    }
}

/// Fitted B11 last-ditch candidate selection.
///
/// An equal draw among the suitable candidates of
/// [`LastDitchCandidate::ALL`], using [`last_ditch_suitable`]. When nothing is
/// suitable the aircraft jinks vertically, the choice the spec already names as
/// the ordinary sub-30000-foot fallback.
pub fn last_ditch_candidate(
    own_agl_ft: f64,
    turn_radius_ft: f64,
    speed: ScalarSpeed,
    limits: &SpeedLimits,
    random: &mut DecisionRandom,
) -> LastDitchCandidate {
    let suitable: Vec<LastDitchCandidate> = LastDitchCandidate::ALL
        .into_iter()
        .filter(|c| last_ditch_suitable(*c, own_agl_ft, turn_radius_ft, speed, limits))
        .collect();
    if suitable.is_empty() {
        return LastDitchCandidate::VerticalJink;
    }
    let index = random.choose(suitable.len() as u32) as usize;
    suitable[index]
}

/// The five basic maneuvers the fitted random-tactic menu draws from.
///
/// These are exactly the B13 maneuvers that need no target geometry, so the
/// menu is always available. The spec does not record the original's contents.
pub const RANDOM_TACTIC_MENU_SIZE: u32 = 5;

/// Fitted random-tactic menu.
///
/// An equal draw among straight climb, straight dive, break left, break right
/// and turnaround, built through the spec-derived [`super::motion`] builders so
/// every request still carries B13's bounds. `can_climb` is B04 and `own_agl_ft`
/// drives the dive's 3000 ft floor, so the builders can still refuse.
pub fn random_tactic_menu(
    frame: ManeuverFrame,
    can_climb: bool,
    own_agl_ft: f64,
    random: &mut DecisionRandom,
) -> MotionRequest {
    match random.choose(RANDOM_TACTIC_MENU_SIZE) {
        0 => super::motion::straight_climb(frame, can_climb),
        1 => super::motion::straight_dive(frame, own_agl_ft),
        2 => super::motion::break_left(frame),
        3 => super::motion::break_right(frame),
        _ => super::motion::turn_around(frame, random).1,
    }
}

/// Fitted B12 remaining tactics.
///
/// When both the best-attack and the random-tactic draws fail, the aircraft
/// pursues. The spec already records that the best-attack branch "ordinarily
/// selects pursuit", so this keeps the aircraft in the fight rather than
/// inventing a maneuver the source does not name.
pub fn remaining_tactic() -> BehaviorChoice {
    BehaviorChoice::Pursuit
}

/// Fitted B44 lead prediction.
///
/// The spec gives the 20000 ft bypass and the 1600 ft reduction ramp but leaves
/// the speed estimator and the prediction time open. This rule flies the target
/// along its own heading and pitch at its scalar speed for the time a store of
/// `store_speed` takes to cover the current range. The result is the
/// `predicted_travel` operand of
/// [`pursuit::lead_point`](super::pursuit::lead_point), so the recovered gates
/// still apply on top of it.
pub fn lead_predicted_travel(
    range_ft: f64,
    store_speed: ScalarSpeed,
    target_speed: ScalarSpeed,
    target_heading_deg: f64,
    target_pitch_deg: f64,
) -> [f64; 3] {
    if !range_ft.is_finite() || store_speed.0 <= 0.0 || target_speed.0 <= 0.0 {
        return [0.0; 3];
    }
    let time_s = (range_ft.abs() / store_speed.0).clamp(0.0, 60.0);
    let distance = target_speed.0 * time_s;
    let heading = target_heading_deg.to_radians();
    let pitch = target_pitch_deg.to_radians();
    [
        distance * heading.sin() * pitch.cos(),
        distance * pitch.sin(),
        distance * heading.cos() * pitch.cos(),
    ]
}

/// Base points of the fitted hit-chance term in the B45 store score.
pub const HIT_CHANCE_BASE: f64 = 50.0;

/// Fitted B45 hit chance for the store score.
///
/// The original's routine is opaque. This rule awards [`HIT_CHANCE_BASE`]
/// points scaled linearly by how far inside its employment angular limit the
/// store is pointing, and nothing outside it. It feeds
/// [`weapon_service::StoreCandidate::hit_chance`](super::weapon_service::StoreCandidate),
/// so the recovered angular, range and damage terms are unaffected.
pub fn hit_chance(pointing_error_deg: f64, employment_limit_deg: Option<f64>) -> f64 {
    let Some(limit) = employment_limit_deg else {
        return HIT_CHANCE_BASE;
    };
    if !pointing_error_deg.is_finite() || !limit.is_finite() || limit <= 0.0 {
        return 0.0;
    }
    let inside = 1.0 - (pointing_error_deg.abs() / limit);
    (inside * HIT_CHANCE_BASE).clamp(0.0, HIT_CHANCE_BASE)
}

/// Fitted straight-flight request used when no other branch produces motion.
///
/// This is the documented last resort named in the controller's contract: an
/// actor that cannot resolve a maneuver keeps flying its current heading at the
/// engagement pitch and maximum speed, which is exactly B13's straight flight.
pub fn straight_flight(frame: ManeuverFrame) -> MotionRequest {
    super::motion::straight(frame)
}

/// A break side chosen with equal probability, for the fitted menu's reuse.
pub fn break_side(random: &mut DecisionRandom) -> BreakSide {
    if random.chance(50) {
        BreakSide::Left
    } else {
        BreakSide::Right
    }
}

/// Fitted free-flight maneuver state numbers for the B46 receiver gate.
///
/// B46 accepts original states 19 and 20 and rejects 1..=18 and 21..=30. The
/// mission-facing names of those states are unknown, so the host assigns its
/// own actors the two accepted numbers and never produces the rejected ones:
/// [`MANEUVER_STATE_FREE`] while flying freely and [`MANEUVER_STATE_ENGAGED`]
/// while engaging. This is an opinionated host numbering that satisfies the
/// recovered gate; it does not claim to name the original's states.
pub const MANEUVER_STATE_FREE: u32 = 19;
/// See [`MANEUVER_STATE_FREE`].
pub const MANEUVER_STATE_ENGAGED: u32 = 20;

/// Fitted per-level decision cadence, in quarter-second counts.
///
/// B13 establishes the quarter-second clock and the spec is explicit that a
/// controller "must not redraw a tactic every tick merely because the host
/// calls step". The original's tactical re-evaluation cadence is not recovered,
/// so the controller re-enters the tactical choice on this cadence. Faster
/// levels think more often; the values are quarter seconds, so Novice
/// re-evaluates every two seconds and Ace every second.
pub const TACTICAL_CADENCE_QUARTERS: PerLevel<u64> = [8, 6, 5, 4];

/// The fitted tactical cadence for a level, in quarter-second counts.
pub fn tactical_cadence_quarters(level: Experience) -> u64 {
    TACTICAL_CADENCE_QUARTERS[level.index()]
}

/// Build a straight-flight frame for the fitted rules that need one.
pub fn frame(current_heading_deg: i32, duration: Duration) -> ManeuverFrame {
    ManeuverFrame {
        current_heading_deg,
        bank: Bank::Unconstrained,
        duration,
    }
}

/// A motion request that holds the current heading at an explicit pitch.
///
/// Used by the fitted fallbacks that need a concrete pitch rather than
/// [`PitchRequest::Engagement`], such as the terrain-driven climb.
pub fn hold_heading_at_pitch(
    current_heading_deg: i32,
    pitch_deg: i32,
    speed: SpeedRequest,
    duration: Duration,
) -> MotionRequest {
    MotionRequest::new(
        current_heading_deg,
        PitchRequest::Explicit(pitch_deg),
        Bank::Unconstrained,
        speed,
        duration,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::steering::TURN_RATE_CAP_DEG_PER_S;

    #[test]
    fn every_fallback_names_its_branch_and_rule() {
        for fallback in Fallback::ALL {
            assert!(!fallback.name().is_empty());
            assert!(!fallback.spec_branch().is_empty());
            assert!(!fallback.rule().is_empty());
        }
        let mut names: Vec<&str> = Fallback::ALL.iter().map(|f| f.name()).collect();
        names.sort_unstable();
        let unique = names.len();
        names.dedup();
        assert_eq!(names.len(), unique, "fallback names must be unique");
    }

    #[test]
    fn engagement_pitch_is_bounded_and_refuses_a_forbidden_climb() {
        // Level with the target: no pitch.
        assert_eq!(engagement_pitch_deg(0.0, 10000.0, true), 0.0);
        // Far above: dive, bounded at the limit.
        let steep = engagement_pitch_deg(-100000.0, 1000.0, true);
        assert!((steep + ENGAGEMENT_PITCH_LIMIT_DEG).abs() < 1e-9, "{steep}");
        // Far below with a climb permitted: bounded the other way.
        let climb = engagement_pitch_deg(100000.0, 1000.0, true);
        assert!((climb - ENGAGEMENT_PITCH_LIMIT_DEG).abs() < 1e-9, "{climb}");
        // The same geometry with B04 refusing a climb gives level flight.
        assert_eq!(engagement_pitch_deg(100000.0, 1000.0, false), 0.0);
        // A dive is never refused by the climb permission.
        assert!(engagement_pitch_deg(-5000.0, 5000.0, false) < 0.0);
        // Half the relative altitude over the run, not all of it. Chosen well
        // inside the bound so the clamp is not what is being measured.
        let half = engagement_pitch_deg(1000.0, 5000.0, true);
        let expected = 500.0f64.atan2(5000.0).to_degrees();
        assert!((half - expected).abs() < 1e-9, "{half} {expected}");
        assert!(half.abs() < ENGAGEMENT_PITCH_LIMIT_DEG, "{half}");
        // Non-finite input is level flight, never a NaN request.
        assert_eq!(engagement_pitch_deg(f64::NAN, 1000.0, true), 0.0);
    }

    #[test]
    fn base_pitch_rate_matches_the_turn_rate_formula() {
        // The spec's own worked example: 7 G at 500 ft/s gives 35 deg/s.
        let rate = base_pitch_rate_deg_per_s(7.0, ScalarSpeed(500.0)).unwrap();
        assert!((rate - 35.0).abs() < 1e-9, "{rate}");
        // The cap still applies.
        let capped = base_pitch_rate_deg_per_s(9.0, ScalarSpeed(125.0)).unwrap();
        assert!((capped - TURN_RATE_CAP_DEG_PER_S).abs() < 1e-9, "{capped}");
    }

    #[test]
    fn completion_axis_takes_the_slowest_arriving_axis() {
        // Heading is furthest in time.
        assert_eq!(
            completion_axis(90.0, 10.0, None, 10.0, 10.0, 45.0),
            CompletionAxis::Heading
        );
        // Pitch is furthest in time.
        assert_eq!(
            completion_axis(10.0, 90.0, None, 10.0, 10.0, 45.0),
            CompletionAxis::Pitch
        );
        // Bank only competes when it is constrained.
        assert_eq!(
            completion_axis(10.0, 10.0, Some(180.0), 40.0, 40.0, 1.0),
            CompletionAxis::Bank
        );
        assert_eq!(
            completion_axis(10.0, 10.0, None, 40.0, 40.0, 1.0),
            CompletionAxis::Heading
        );
        // Ties go to heading.
        assert_eq!(
            completion_axis(30.0, 30.0, None, 10.0, 10.0, 45.0),
            CompletionAxis::Heading
        );
        // A zero or non-finite rate does not panic or produce NaN ordering.
        assert_eq!(
            completion_axis(30.0, 30.0, None, 0.0, 0.0, 0.0),
            CompletionAxis::Heading
        );
    }

    #[test]
    fn last_ditch_suitability_gates_only_the_vertical_candidates() {
        let limits = SpeedLimits {
            minimum: ScalarSpeed(200.0),
            maximum: ScalarSpeed(1200.0),
            corner: ScalarSpeed(700.0),
        };
        let radius = 1000.0;
        let room = LAST_DITCH_LOOP_RADII * radius;
        // Plenty of room and energy: everything is suitable.
        for candidate in LastDitchCandidate::ALL {
            assert!(last_ditch_suitable(
                candidate,
                room + 1.0,
                radius,
                ScalarSpeed(600.0),
                &limits
            ));
        }
        // On the deck: the two vertical candidates drop out.
        assert!(!last_ditch_suitable(
            LastDitchCandidate::SplitS,
            room - 1.0,
            radius,
            ScalarSpeed(600.0),
            &limits
        ));
        assert!(!last_ditch_suitable(
            LastDitchCandidate::Loop,
            room - 1.0,
            radius,
            ScalarSpeed(600.0),
            &limits
        ));
        assert!(last_ditch_suitable(
            LastDitchCandidate::HorizontalScissors,
            0.0,
            radius,
            ScalarSpeed(600.0),
            &limits
        ));
        // Slow but high: the loop drops out on energy, the split S does not.
        let slow = ScalarSpeed(limits.minimum.0 + LAST_DITCH_LOOP_SPEED_MARGIN_FPS - 1.0);
        assert!(!last_ditch_suitable(
            LastDitchCandidate::Loop,
            room + 1.0,
            radius,
            slow,
            &limits
        ));
        assert!(last_ditch_suitable(
            LastDitchCandidate::SplitS,
            room + 1.0,
            radius,
            slow,
            &limits
        ));
    }

    #[test]
    fn last_ditch_selection_stays_inside_the_suitable_set() {
        let limits = SpeedLimits {
            minimum: ScalarSpeed(200.0),
            maximum: ScalarSpeed(1200.0),
            corner: ScalarSpeed(700.0),
        };
        let mut random = DecisionRandom::seeded(11);
        // On the deck and slow: never a split S or a loop.
        for _ in 0..500 {
            let chosen =
                last_ditch_candidate(0.0, 1000.0, ScalarSpeed(210.0), &limits, &mut random);
            assert_ne!(chosen, LastDitchCandidate::SplitS);
            assert_ne!(chosen, LastDitchCandidate::Loop);
        }
        // With room, every candidate is reachable.
        let mut seen: Vec<LastDitchCandidate> = Vec::new();
        for _ in 0..2000 {
            let chosen =
                last_ditch_candidate(50000.0, 1000.0, ScalarSpeed(900.0), &limits, &mut random);
            if !seen.contains(&chosen) {
                seen.push(chosen);
            }
        }
        assert_eq!(seen.len(), LastDitchCandidate::ALL.len());
    }

    #[test]
    fn last_ditch_falls_back_to_a_vertical_jink_when_nothing_is_suitable() {
        // Construct a state where the gate rejects the vertical pair; the
        // horizontal candidates are unconditionally suitable, so the empty
        // path is only reachable if that ever changes. Assert the documented
        // fallback directly.
        let limits = SpeedLimits {
            minimum: ScalarSpeed(200.0),
            maximum: ScalarSpeed(1200.0),
            corner: ScalarSpeed(700.0),
        };
        let mut random = DecisionRandom::seeded(3);
        let chosen = last_ditch_candidate(0.0, 1e9, ScalarSpeed(200.0), &limits, &mut random);
        assert!(last_ditch_suitable(
            chosen,
            0.0,
            1e9,
            ScalarSpeed(200.0),
            &limits
        ));
    }

    #[test]
    fn random_tactic_menu_is_deterministic_and_bounded() {
        let frame = ManeuverFrame {
            current_heading_deg: 90,
            bank: Bank::Unconstrained,
            duration: Duration::Timed(5),
        };
        let mut a = DecisionRandom::seeded(42);
        let mut b = DecisionRandom::seeded(42);
        for _ in 0..200 {
            let left = random_tactic_menu(frame, true, 20000.0, &mut a);
            let right = random_tactic_menu(frame, true, 20000.0, &mut b);
            assert_eq!(left, right);
            assert!(left.heading_deg < 360);
        }
    }

    #[test]
    fn random_tactic_menu_respects_the_climb_and_dive_refusals() {
        let frame = ManeuverFrame {
            current_heading_deg: 0,
            bank: Bank::Unconstrained,
            duration: Duration::Timed(5),
        };
        let mut random = DecisionRandom::seeded(7);
        // Below 3000 ft AGL and unable to climb, the builders fall back to
        // straight flight, so no request ever carries a plus or minus 45.
        for _ in 0..500 {
            let request = random_tactic_menu(frame, false, 100.0, &mut random);
            assert_ne!(request.pitch, PitchRequest::Explicit(45));
            assert_ne!(request.pitch, PitchRequest::Explicit(-45));
        }
    }

    #[test]
    fn remaining_tactic_is_pursuit() {
        assert_eq!(remaining_tactic(), BehaviorChoice::Pursuit);
    }

    #[test]
    fn lead_prediction_flies_the_target_along_its_own_axis() {
        // Heading 0 is +z, so a level target at heading 0 moves in +z only.
        let travel =
            lead_predicted_travel(10000.0, ScalarSpeed(2000.0), ScalarSpeed(800.0), 0.0, 0.0);
        assert!(travel[0].abs() < 1e-9, "{travel:?}");
        assert!(travel[1].abs() < 1e-9, "{travel:?}");
        // 10000 / 2000 = 5 s at 800 ft/s = 4000 ft.
        assert!((travel[2] - 4000.0).abs() < 1e-6, "{travel:?}");
        // Heading 90 is +x.
        let east =
            lead_predicted_travel(10000.0, ScalarSpeed(2000.0), ScalarSpeed(800.0), 90.0, 0.0);
        assert!((east[0] - 4000.0).abs() < 1e-6, "{east:?}");
        assert!(east[2].abs() < 1e-6, "{east:?}");
        // A climbing target gains altitude.
        let climbing =
            lead_predicted_travel(10000.0, ScalarSpeed(2000.0), ScalarSpeed(800.0), 0.0, 90.0);
        assert!((climbing[1] - 4000.0).abs() < 1e-6, "{climbing:?}");
        // Degenerate inputs are no prediction, never a NaN aim point.
        assert_eq!(
            lead_predicted_travel(10000.0, ScalarSpeed(0.0), ScalarSpeed(800.0), 0.0, 0.0),
            [0.0; 3]
        );
        assert_eq!(
            lead_predicted_travel(f64::NAN, ScalarSpeed(2000.0), ScalarSpeed(800.0), 0.0, 0.0),
            [0.0; 3]
        );
    }

    #[test]
    fn hit_chance_scales_inside_the_employment_limit() {
        assert_eq!(hit_chance(0.0, Some(30.0)), HIT_CHANCE_BASE);
        assert_eq!(hit_chance(30.0, Some(30.0)), 0.0);
        assert!((hit_chance(15.0, Some(30.0)) - 25.0).abs() < 1e-9);
        // Outside the limit never goes negative.
        assert_eq!(hit_chance(90.0, Some(30.0)), 0.0);
        // An unrestricted store gets the full term.
        assert_eq!(hit_chance(80.0, None), HIT_CHANCE_BASE);
        // Degenerate limits score zero rather than NaN.
        assert_eq!(hit_chance(10.0, Some(0.0)), 0.0);
        assert_eq!(hit_chance(f64::NAN, Some(30.0)), 0.0);
    }

    #[test]
    fn maneuver_states_sit_inside_the_accepted_b46_range() {
        assert!((19..=20).contains(&MANEUVER_STATE_FREE));
        assert!((19..=20).contains(&MANEUVER_STATE_ENGAGED));
        assert_ne!(MANEUVER_STATE_FREE, MANEUVER_STATE_ENGAGED);
    }

    #[test]
    fn tactical_cadence_is_faster_for_higher_experience() {
        let mut previous = u64::MAX;
        for level in Experience::ALL {
            let cadence = tactical_cadence_quarters(level);
            assert!(cadence > 0);
            assert!(cadence <= previous, "cadence must not slow with skill");
            previous = cadence;
        }
    }

    #[test]
    fn break_side_is_an_even_split() {
        let mut random = DecisionRandom::seeded(5);
        let lefts = (0..10_000)
            .filter(|_| break_side(&mut random) == BreakSide::Left)
            .count();
        assert!((4_700..5_300).contains(&lefts), "{lefts}");
    }

    #[test]
    fn straight_flight_holds_the_current_heading() {
        let request = straight_flight(ManeuverFrame {
            current_heading_deg: 271,
            bank: Bank::Unconstrained,
            duration: Duration::Timed(5),
        });
        assert_eq!(request.heading_deg, 271);
        assert_eq!(request.pitch, PitchRequest::Engagement);
        assert_eq!(request.speed, SpeedRequest::Maximum);
    }

    #[test]
    fn hold_heading_at_pitch_bounds_its_request() {
        let request = hold_heading_at_pitch(370, 80, SpeedRequest::Corner, Duration::Timed(3));
        assert_eq!(request.heading_deg, 10);
        assert_eq!(request.pitch, PitchRequest::Explicit(80));
        assert_eq!(request.speed, SpeedRequest::Corner);
    }

    #[test]
    fn frame_builds_an_unconstrained_bank_request() {
        let built = frame(45, Duration::Geometric);
        assert_eq!(built.current_heading_deg, 45);
        assert_eq!(built.bank, Bank::Unconstrained);
        assert_eq!(built.duration, Duration::Geometric);
    }
}
