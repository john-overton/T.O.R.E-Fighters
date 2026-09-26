//! Fighter tactical decisions: B10 missile reactions, B11 evasion and
//! last-ditch entry, B12 approach and pursuit offsets, B14 surface attack.
//!
//! Spec homes: `docs/spec/ai.md` (B10 through B14) and
//! `docs/spec/ai-experience.md`, "Fighter tactical choices". Every function is
//! a pure decision over explicit inputs plus caller-owned [`DecisionRandom`].
//! Nothing here reads live aircraft state.
//!
//! Wiring note for the parent: [`TacticalSituation`] carries the geometry
//! predicates that `geometry.rs` produces (B01 ahead, B02 facing, B03
//! distances, off-beam and heading/pitch differences), and
//! [`TacticalThresholds`] carries the per-level percentages that
//! `experience.rs` owns. Both are taken by value so this file has no build
//! dependency on those modules.

use super::motion::wrap_heading_deg;
use super::{AiError, DecisionRandom, Experience, Result};

// B10 constants.

/// B10: an infrared launch requests a climb to this pitch.
pub const INFRARED_CLIMB_PITCH_DEG: i32 = 90;
/// B10: an infrared launch sends a wingman break request on 50 of 100 draws.
pub const INFRARED_WINGMAN_BREAK_PERCENT: u8 = 50;
/// B10: the infrared wingman break request's heading offset.
pub const INFRARED_WINGMAN_BREAK_HEADING_DEG: i32 = 90;
/// B10: the infrared wingman break request's pitch offset.
pub const INFRARED_WINGMAN_BREAK_PITCH_DEG: i32 = 90;
/// B10: a radar launch turns toward the target when off-beam is at most this.
pub const RADAR_TURN_TOWARD_MAX_OFF_BEAM_DEG: f64 = 90.0;
/// B10: otherwise a radar launch turns current heading plus or minus this.
pub const RADAR_BREAK_TURN_DEG: i32 = 90;

// B11 constants.

/// B11: non-aircraft evasion altitude operand floor.
pub const NON_AIRCRAFT_EVASION_ALTITUDE_FT: f64 = 20000.0;
/// B11: coordinated escape is skipped on 70 of 100 draws.
pub const COORDINATED_ESCAPE_SKIP_PERCENT: u8 = 70;
/// B11: last-ditch entry requires the target within this spatial distance.
pub const LAST_DITCH_ENTRY_MAX_DISTANCE_FT: f64 = 5000.0;
/// B11: last-ditch entry requires heading difference at most this.
pub const LAST_DITCH_ENTRY_MAX_HEADING_DIFF_DEG: f64 = 35.0;
/// B11: at this target distance or more the fallback flies away.
pub const FLY_AWAY_MIN_DISTANCE_FT: f64 = 30000.0;

// B12 constants.

/// B12: air-to-air entry raises wing horizontal spacing to at least this.
pub const AIR_TO_AIR_MIN_WING_SPACING_FT: f64 = 5000.0;
/// B12: wing split needs a wing-approach value in this inclusive range.
pub const WING_SPLIT_APPROACH_RANGE_FT: std::ops::RangeInclusive<f64> = 5000.0..=30000.0;
/// B12: special approach separation, inclusive.
pub const SPECIAL_APPROACH_SEPARATION_FT: std::ops::RangeInclusive<f64> = 5000.0..=20000.0;
/// B12: special approach off-beam at most this.
pub const SPECIAL_APPROACH_MAX_OFF_BEAM_DEG: f64 = 20.0;
/// B12: special approach heading difference at least this.
pub const SPECIAL_APPROACH_MIN_HEADING_DIFF_DEG: f64 = 155.0;
/// B12: special approach pitch difference at most this.
pub const SPECIAL_APPROACH_MAX_PITCH_DIFF_DEG: f64 = 25.0;
/// B12: `random 100 > 75` enters the special approach on 76 of 100 draws.
pub const SPECIAL_APPROACH_ENTRY_PERCENT: u8 = 76;
/// B12: separation greater than this selects pursuit directly.
pub const DIRECT_PURSUIT_MIN_SEPARATION_FT: f64 = 15000.0;
/// B12: best attack may choose last-ditch within this distance.
pub const BEST_ATTACK_LAST_DITCH_MAX_DISTANCE_FT: f64 = 2000.0;
/// B12: best attack may choose last-ditch with heading difference at most this.
pub const BEST_ATTACK_LAST_DITCH_MAX_HEADING_DIFF_DEG: f64 = 35.0;
/// B12: the best-attack last-ditch gate, as the spec states it (75%).
pub const BEST_ATTACK_LAST_DITCH_PERCENT: u8 = 75;
/// Experience spec: a Novice with a target behind and facing has an earlier
/// 40% straight-flight choice before the best/random choices.
pub const NOVICE_BEHIND_FACING_STRAIGHT_PERCENT: u8 = 40;

// Pursuit offset constants (experience spec, "Fighter tactical choices").

/// The chased condition replaces the vertical offset with this magnitude.
pub const CHASED_VERTICAL_OFFSET_FT: i32 = 2000;
/// Nominal pursuit duration below 5000 feet of target distance.
pub const PURSUIT_DURATION_NEAR_S: u8 = 6;
/// Nominal pursuit duration at or beyond 5000 feet of target distance.
pub const PURSUIT_DURATION_FAR_S: u8 = 12;
/// Target distance below which the near pursuit duration applies.
pub const PURSUIT_NEAR_DISTANCE_FT: f64 = 5000.0;

// B14 constants.

/// B14: the alternative surface attack is selected on 25 of 100 draws.
pub const SURFACE_ALTERNATIVE_PERCENT: u8 = 25;
/// B14: dive-bomb needs altitude and horizontal distance each at least
/// `max(10000, 2 * turn radius)`.
pub const DIVE_BOMB_MIN_DISTANCE_FT: f64 = 10000.0;
/// B14: dive-bomb sits behind a 33% gate.
pub const DIVE_BOMB_PERCENT: u8 = 33;
/// B14: after dive-bomb fails, fast-high pass is chosen one time in three.
pub const FAST_HIGH_PASS_ONE_IN: u32 = 3;
/// B14: egress jinks while within this horizontal distance.
pub const EGRESS_JINK_WITHIN_HORIZONTAL_FT: f64 = 10000.0;
/// B14: egress departs until at least `max(30000, 2 * turn radius)` away.
pub const EGRESS_DEPARTURE_MIN_DISTANCE_FT: f64 = 30000.0;

/// What the assigned target is, as far as the fighter script distinguishes
/// (B06).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetClass {
    Aircraft {
        /// Fighter versus other aircraft.
        fighter: bool,
        /// Human-controlled target.
        human: bool,
    },
    NonAircraft,
}

/// The geometry and state inputs the tactical decisions read. The parent
/// wires these from `geometry.rs` (B01 through B03) and the wing services.
/// Angles are degrees, distances are feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TacticalSituation {
    pub target: TargetClass,
    /// B03 spatial separation.
    pub spatial_distance_ft: f64,
    /// B01: larger absolute error strictly below 90 degrees.
    pub ahead: bool,
    /// B02: target's bearing error toward us at most 90 degrees.
    pub facing: bool,
    /// B01 off-beam, the larger absolute heading/pitch error.
    pub off_beam_deg: f64,
    /// Absolute heading difference between the two aircraft.
    pub heading_difference_deg: f64,
    /// Absolute pitch difference between the two aircraft.
    pub pitch_difference_deg: f64,
    /// Wing combat state (B06); its production is unresolved.
    pub wing_combat: bool,
    /// Wing approach state (B06); its production is unresolved.
    pub wing_approach: bool,
    /// B12 wing-approach value; its producer is unresolved, so it is an
    /// explicit optional input rather than target distance.
    pub wing_approach_value_ft: Option<f64>,
    /// Own height above the queried surface (B03).
    pub own_agl_ft: f64,
}
impl TacticalSituation {
    pub fn is_fighter(&self) -> bool {
        matches!(self.target, TargetClass::Aircraft { fighter: true, .. })
    }
    pub fn is_human(&self) -> bool {
        matches!(self.target, TargetClass::Aircraft { human: true, .. })
    }
    pub fn is_aircraft(&self) -> bool {
        matches!(self.target, TargetClass::Aircraft { .. })
    }
    /// The ahead/behind by facing/facing-away row of the tactical table.
    pub fn quadrant(&self) -> Quadrant {
        match (self.ahead, self.facing) {
            (true, true) => Quadrant::AheadFacing,
            (true, false) => Quadrant::AheadFacingAway,
            (false, true) => Quadrant::BehindFacing,
            (false, false) => Quadrant::BehindFacingAway,
        }
    }
}

/// Rows of the experience spec's tactical table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quadrant {
    AheadFacing,
    AheadFacingAway,
    BehindFacing,
    BehindFacingAway,
}

/// Percent thresholds for one quadrant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuadrantThresholds {
    /// "Prefer best attack" draw threshold.
    pub best_attack_percent: u8,
    /// "Otherwise choose random tactic" draw threshold, reached after the
    /// best-attack draw fails.
    pub random_tactic_percent: u8,
}

/// One experience level's tactical thresholds. `experience.rs` owns the
/// tables; the parent constructs this from them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TacticalThresholds {
    pub ahead_facing: QuadrantThresholds,
    pub ahead_facing_away: QuadrantThresholds,
    pub behind_facing: QuadrantThresholds,
    pub behind_facing_away: QuadrantThresholds,
    /// "Fly straight on entering the random-tactic menu".
    pub straight_on_random_menu_percent: u8,
    /// "Pursuit-point vertical displacement when chased".
    pub chased_vertical_displacement_percent: u8,
    /// Each initial pursuit offset component is drawn from `0..bound`
    /// (100, 50, 30, 10 per level).
    pub pursuit_offset_bound: u32,
}
impl TacticalThresholds {
    pub fn for_quadrant(&self, quadrant: Quadrant) -> QuadrantThresholds {
        match quadrant {
            Quadrant::AheadFacing => self.ahead_facing,
            Quadrant::AheadFacingAway => self.ahead_facing_away,
            Quadrant::BehindFacing => self.behind_facing,
            Quadrant::BehindFacingAway => self.behind_facing_away,
        }
    }
}

/// The typed outcome of a tactical decision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BehaviorChoice {
    /// Ordinary pursuit; offsets come from [`pursuit_offsets`].
    Pursuit,
    /// Last-ditch defense entered; the candidate selection is a separate,
    /// partly unresolved step ([`select_last_ditch`]).
    LastDitch,
    /// Straight flight (B13 builder).
    Straight,
    /// Wing split attempted (B12); its continuation is unresolved.
    WingSplit,
    SpecialApproach(SpecialApproach),
    CoordinatedEscape(CoordinatedEscape),
    /// Fly away from the target at maximum speed (B11 fallback).
    FlyAway,
    /// Vertical jink (B11 fallback).
    VerticalJink,
    NonAircraftEvasion(NonAircraftEvasion),
    MissileReaction(MissileReaction),
    SurfaceAttack(SurfaceAttack),
}

/// B10: the launch reason supplied by the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissileLaunch {
    Infrared,
    Radar,
}

/// B10: inputs for a missile reaction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MissileReactionInputs {
    pub current_heading_deg: i32,
    /// Horizontal bearing to the target, degrees.
    pub target_bearing_deg: i32,
    /// B01 off-beam toward the event's target.
    pub off_beam_deg: f64,
}

/// B10: own maneuver requested in reaction to a launch. Speeds and pitches
/// are spelled out in the variants; the bank of the radar branches and the
/// durations of all branches are not recorded in the spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissileReaction {
    /// Infrared: climb to +90 pitch at current heading and maximum speed,
    /// bank unconstrained.
    ClimbToPitch90 { heading_deg: u16 },
    /// Radar, off-beam at most 90: turn toward the target bearing at the
    /// engagement pitch and corner speed.
    TurnTowardTarget { heading_deg: u16 },
    /// Radar, off-beam over 90: current heading plus or minus 90 at equal
    /// probability. Pitch and speed for this branch are not in the spec.
    BreakNinety { heading_deg: u16, side: BreakSide },
}

/// Which way a heading change went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakSide {
    Left,
    Right,
}

/// B10: the wingman break request sent with a reaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WingmanBreak {
    /// Infrared: +90 heading and +90 pitch.
    Offsets { heading_deg: i32, pitch_deg: i32 },
    /// Radar: the spec confirms the request is sent but not its offsets.
    UnrecoveredOffsets,
}

/// B10: reaction plus the optional wingman break request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissileResponse {
    pub reaction: MissileReaction,
    pub wingman_break: Option<WingmanBreak>,
}

/// B10: reaction to an infrared or radar launch.
pub fn missile_reaction(
    launch: MissileLaunch,
    inputs: MissileReactionInputs,
    random: &mut DecisionRandom,
) -> MissileResponse {
    match launch {
        MissileLaunch::Infrared => MissileResponse {
            reaction: MissileReaction::ClimbToPitch90 {
                heading_deg: wrap_heading_deg(inputs.current_heading_deg),
            },
            wingman_break: random
                .site("infrared wingman break")
                .chance(INFRARED_WINGMAN_BREAK_PERCENT)
                .then_some(WingmanBreak::Offsets {
                    heading_deg: INFRARED_WINGMAN_BREAK_HEADING_DEG,
                    pitch_deg: INFRARED_WINGMAN_BREAK_PITCH_DEG,
                }),
        },
        MissileLaunch::Radar => {
            let reaction = if inputs.off_beam_deg <= RADAR_TURN_TOWARD_MAX_OFF_BEAM_DEG {
                MissileReaction::TurnTowardTarget {
                    heading_deg: wrap_heading_deg(inputs.target_bearing_deg),
                }
            } else {
                // Draw-to-side mapping is arbitrary; both sides are equally likely.
                let (side, turn) = match random.site("radar break side").choose(2) {
                    0 => (BreakSide::Left, -RADAR_BREAK_TURN_DEG),
                    _ => (BreakSide::Right, RADAR_BREAK_TURN_DEG),
                };
                MissileReaction::BreakNinety {
                    heading_deg: wrap_heading_deg(inputs.current_heading_deg + turn),
                    side,
                }
            };
            MissileResponse {
                reaction,
                wingman_break: Some(WingmanBreak::UnrecoveredOffsets),
            }
        }
    }
}

/// B11: the altitude frame of the non-aircraft evasion command is not yet
/// confirmed; the source mixes an AGL read with a command whose frame is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AltitudeFrame {
    Unresolved,
}

/// B11: evasion request against a non-aircraft target: unchanged heading,
/// maximum speed and an altitude operand of `max(AGL, 20000)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NonAircraftEvasion {
    pub altitude_operand_ft: f64,
    pub altitude_frame: AltitudeFrame,
}
impl NonAircraftEvasion {
    pub fn for_agl(own_agl_ft: f64) -> Self {
        Self {
            altitude_operand_ft: own_agl_ft.max(NON_AIRCRAFT_EVASION_ALTITUDE_FT),
            altitude_frame: AltitudeFrame::Unresolved,
        }
    }
}

/// B11: coordinated escape variants, chosen equally once the branch is
/// entered. Coordination geometry belongs to the wing services.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordinatedEscape {
    CrossTurn,
    LeftRightSplit,
    HighLowSplit,
}
impl CoordinatedEscape {
    pub const ALL: [Self; 3] = [Self::CrossTurn, Self::LeftRightSplit, Self::HighLowSplit];
}

/// B11: coordinated escape requires target behind and facing plus wing
/// combat and wing approach states.
pub fn coordinated_escape_eligible(situation: &TacticalSituation) -> bool {
    situation.is_aircraft()
        && !situation.ahead
        && situation.facing
        && situation.wing_combat
        && situation.wing_approach
}

/// B11: when eligible, a 70% skip leaves 30% for an equal split among the
/// three escapes. Returns `None` when ineligible or skipped.
pub fn coordinated_escape(
    situation: &TacticalSituation,
    random: &mut DecisionRandom,
) -> Option<CoordinatedEscape> {
    if !coordinated_escape_eligible(situation)
        || random
            .site("coordinated escape skip")
            .chance(COORDINATED_ESCAPE_SKIP_PERCENT)
    {
        return None;
    }
    Some(CoordinatedEscape::ALL[random.site("coordinated escape choice").choose(3) as usize])
}

/// B11: last-ditch entry needs the target behind, facing, within 5000 feet
/// and with heading difference at most 35 degrees.
pub fn last_ditch_entry(situation: &TacticalSituation) -> bool {
    situation.is_aircraft()
        && !situation.ahead
        && situation.facing
        && situation.spatial_distance_ft <= LAST_DITCH_ENTRY_MAX_DISTANCE_FT
        && situation.heading_difference_deg <= LAST_DITCH_ENTRY_MAX_HEADING_DIFF_DEG
}

/// B11: the last-ditch candidate actions named by the spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LastDitchCandidate {
    SplitS,
    HorizontalScissors,
    Reverse,
    Overshoot,
    Loop,
    HorizontalJink,
    VerticalJink,
}
impl LastDitchCandidate {
    pub const ALL: [Self; 7] = [
        Self::SplitS,
        Self::HorizontalScissors,
        Self::Reverse,
        Self::Overshoot,
        Self::Loop,
        Self::HorizontalJink,
        Self::VerticalJink,
    ];
}

/// B11: selecting among the last-ditch candidates redraws when altitude,
/// speed or geometry makes a candidate unsuitable. Those thresholds and the
/// branch weights are not recovered, so this returns `UnspecifiedRule`.
pub fn select_last_ditch(
    _situation: &TacticalSituation,
    _random: &mut DecisionRandom,
) -> Result<LastDitchCandidate> {
    Err(AiError::UnspecifiedRule(
        "B11 last-ditch candidate weights and redraw suitability conditions",
    ))
}

/// B11: the evasion decision. Non-aircraft targets get the altitude request;
/// against aircraft the order is coordinated escape, last-ditch entry, then
/// the distance fallback (fly away at 30000 feet or more, else vertical
/// jink). The order follows the spec's presentation.
pub fn evasion_choice(
    situation: &TacticalSituation,
    random: &mut DecisionRandom,
) -> BehaviorChoice {
    if !situation.is_aircraft() {
        return BehaviorChoice::NonAircraftEvasion(NonAircraftEvasion::for_agl(
            situation.own_agl_ft,
        ));
    }
    if let Some(escape) = coordinated_escape(situation, random) {
        return BehaviorChoice::CoordinatedEscape(escape);
    }
    if last_ditch_entry(situation) {
        return BehaviorChoice::LastDitch;
    }
    if situation.spatial_distance_ft >= FLY_AWAY_MIN_DISTANCE_FT {
        BehaviorChoice::FlyAway
    } else {
        BehaviorChoice::VerticalJink
    }
}

/// B12: air-to-air entry raises requested wing horizontal spacing to 5000
/// feet if it is smaller.
pub fn air_to_air_wing_spacing(requested_ft: f64) -> f64 {
    requested_ft.max(AIR_TO_AIR_MIN_WING_SPACING_FT)
}

/// B12: wing split is attempted only against an ahead fighter whose
/// wing-approach value lies in 5000 through 30000 feet. A missing value is
/// the source's zero, which never qualifies.
pub fn wing_split_eligible(situation: &TacticalSituation) -> bool {
    situation.ahead
        && situation.is_fighter()
        && situation
            .wing_approach_value_ft
            .is_some_and(|v| WING_SPLIT_APPROACH_RANGE_FT.contains(&v))
}

/// B12: special approach passes, chosen equally after entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialApproach {
    OffsetPass,
    OverheadPass,
}

/// B12: special approach geometry against a human-controlled target.
pub fn special_approach_eligible(situation: &TacticalSituation) -> bool {
    situation.is_human()
        && SPECIAL_APPROACH_SEPARATION_FT.contains(&situation.spatial_distance_ft)
        && situation.off_beam_deg <= SPECIAL_APPROACH_MAX_OFF_BEAM_DEG
        && situation.heading_difference_deg >= SPECIAL_APPROACH_MIN_HEADING_DIFF_DEG
        && situation.pitch_difference_deg <= SPECIAL_APPROACH_MAX_PITCH_DIFF_DEG
}

/// B12: when the geometry holds, 76 of 100 draws enter the special approach
/// and then choose offset or overhead pass equally.
pub fn special_approach(
    situation: &TacticalSituation,
    random: &mut DecisionRandom,
) -> Option<SpecialApproach> {
    if !special_approach_eligible(situation)
        || !random
            .site("special approach entry")
            .chance(SPECIAL_APPROACH_ENTRY_PERCENT)
    {
        return None;
    }
    Some(match random.site("special approach pass").choose(2) {
        0 => SpecialApproach::OffsetPass,
        _ => SpecialApproach::OverheadPass,
    })
}

/// B12: the best-attack branch. Ordinarily pursuit; against a fighter behind
/// and facing within 2000 feet with heading difference at most 35 degrees, a
/// 75% gate chooses last-ditch instead.
pub fn best_attack(situation: &TacticalSituation, random: &mut DecisionRandom) -> BehaviorChoice {
    let last_ditch_geometry = situation.is_fighter()
        && !situation.ahead
        && situation.facing
        && situation.spatial_distance_ft <= BEST_ATTACK_LAST_DITCH_MAX_DISTANCE_FT
        && situation.heading_difference_deg <= BEST_ATTACK_LAST_DITCH_MAX_HEADING_DIFF_DEG;
    if last_ditch_geometry
        && random
            .site("best-attack last ditch")
            .chance(BEST_ATTACK_LAST_DITCH_PERCENT)
    {
        BehaviorChoice::LastDitch
    } else {
        BehaviorChoice::Pursuit
    }
}

/// Experience spec: the random-tactic menu draws straight flight first on the
/// per-level threshold. The rest of the menu is not recovered.
pub fn random_tactic(
    thresholds: &TacticalThresholds,
    random: &mut DecisionRandom,
) -> Result<BehaviorChoice> {
    if random
        .site("random-tactic straight flight")
        .chance(thresholds.straight_on_random_menu_percent)
    {
        return Ok(BehaviorChoice::Straight);
    }
    Err(AiError::UnspecifiedRule(
        "random-tactic menu contents after the straight-flight draw",
    ))
}

/// B12 approach flow: wing split attempt, special approach against a human,
/// direct pursuit beyond 15000 feet, the Novice behind-and-facing 40%
/// straight choice, then the experience-dependent best-attack/random draws.
/// The "remaining tactics" branch after both draws fail is unresolved.
pub fn approach_choice(
    situation: &TacticalSituation,
    experience: Experience,
    thresholds: &TacticalThresholds,
    random: &mut DecisionRandom,
) -> Result<BehaviorChoice> {
    if wing_split_eligible(situation) {
        return Ok(BehaviorChoice::WingSplit);
    }
    if let Some(pass) = special_approach(situation, random) {
        return Ok(BehaviorChoice::SpecialApproach(pass));
    }
    if situation.spatial_distance_ft > DIRECT_PURSUIT_MIN_SEPARATION_FT {
        return Ok(BehaviorChoice::Pursuit);
    }
    let quadrant = situation.quadrant();
    if experience == Experience::Novice
        && quadrant == Quadrant::BehindFacing
        && random
            .site("novice straight flight")
            .chance(NOVICE_BEHIND_FACING_STRAIGHT_PERCENT)
    {
        return Ok(BehaviorChoice::Straight);
    }
    let row = thresholds.for_quadrant(quadrant);
    if random.site("best attack").chance(row.best_attack_percent) {
        return Ok(best_attack(situation, random));
    }
    if random
        .site("random tactic")
        .chance(row.random_tactic_percent)
    {
        return random_tactic(thresholds, random);
    }
    Err(AiError::UnspecifiedRule(
        "B12 remaining tactics after best-attack and random-tactic draws fail",
    ))
}

/// The pursuit condition the source distinguishes. How head-on and chased
/// are detected is not recovered; the parent supplies the condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PursuitCondition {
    Ordinary,
    HeadOn,
    Chased,
}

/// Pursuit target-offset request in the B15 frame: the horizontal pair
/// rotates with target heading, the vertical component is world-vertical.
/// Lateral/longitudinal sign conventions are still open in B15.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PursuitOffsets {
    pub lateral_ft: i32,
    pub longitudinal_ft: i32,
    pub vertical_ft: i32,
    /// Nominal duration in simulation seconds (6 near, 12 far).
    pub nominal_duration_s: u8,
}

/// Experience spec: each initial offset component is drawn from `0..bound`
/// (0 through 99, 49, 29 or 9). Head-on zeros all three. Chased passes the
/// per-level vertical displacement gate to replace the vertical component
/// with +2000 or -2000 feet.
///
/// Fitted: the spec gives both signs but not how one is chosen; this draws
/// them with equal probability. Recorded as a research question.
pub fn pursuit_offsets(
    condition: PursuitCondition,
    thresholds: &TacticalThresholds,
    target_distance_ft: f64,
    random: &mut DecisionRandom,
) -> PursuitOffsets {
    let nominal_duration_s = if target_distance_ft < PURSUIT_NEAR_DISTANCE_FT {
        PURSUIT_DURATION_NEAR_S
    } else {
        PURSUIT_DURATION_FAR_S
    };
    let draw = |random: &mut DecisionRandom, site: &'static str| {
        random.site(site).below(thresholds.pursuit_offset_bound) as i32
    };
    let (lateral_ft, longitudinal_ft, mut vertical_ft) = match condition {
        PursuitCondition::HeadOn => (0, 0, 0),
        PursuitCondition::Ordinary | PursuitCondition::Chased => (
            draw(random, "pursuit offset lateral"),
            draw(random, "pursuit offset longitudinal"),
            draw(random, "pursuit offset vertical"),
        ),
    };
    if condition == PursuitCondition::Chased
        && random
            .site("chased vertical displacement")
            .chance(thresholds.chased_vertical_displacement_percent)
    {
        vertical_ft = match random.site("chased vertical side").choose(2) {
            0 => CHASED_VERTICAL_OFFSET_FT,
            _ => -CHASED_VERTICAL_OFFSET_FT,
        };
    }
    PursuitOffsets {
        lateral_ft,
        longitudinal_ft,
        vertical_ft,
        nominal_duration_s,
    }
}

/// B14 inputs. Whether `altitude_ft` is AGL or absolute is not settled by the
/// spec; the parent must pass the frame the dive-bomb check reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceAttackSituation {
    pub altitude_ft: f64,
    pub horizontal_distance_ft: f64,
    pub turn_radius_ft: f64,
}

/// B14: the surface attack profile chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceAttack {
    StraightRun,
    DiveBomb,
    FastHighPass,
    PopUp,
}

/// B14: approach shape for a chosen profile, as typed constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ApproachProfile {
    /// Target-offset vertical operand during the approach, feet.
    pub vertical_offset_ft: Option<f64>,
    /// Pop-up only: turn in toward the target within this distance, then
    /// request the angular offset.
    pub turn_in_distance_ft: Option<f64>,
    /// Pop-up only: angular offset requested at turn-in, degrees.
    pub angular_offset_deg: Option<f64>,
    /// The approach exits within this horizontal distance (or when the target
    /// is no longer ahead/valid).
    pub exit_distance_ft: f64,
}

impl SurfaceAttack {
    /// B14: the ordinary run approaches until horizontal separation is at
    /// most 2000 feet.
    pub const STRAIGHT_RUN_EXIT_FT: f64 = 2000.0;
    /// B14: pop-up vertical offset operand.
    pub const POP_UP_VERTICAL_OFFSET_FT: f64 = 500.0;
    /// B14: pop-up turns toward the target within this distance.
    pub const POP_UP_TURN_IN_FT: f64 = 10000.0;
    /// B14: pop-up angular offset at turn-in.
    pub const POP_UP_ANGULAR_OFFSET_DEG: f64 = 25.0;
    /// B14: pop-up exits within this distance.
    pub const POP_UP_EXIT_FT: f64 = 1500.0;
    /// B14: fast-high pass vertical offset operand.
    pub const FAST_HIGH_VERTICAL_OFFSET_FT: f64 = 5000.0;
    /// B14: fast-high pass exits within this distance.
    pub const FAST_HIGH_EXIT_FT: f64 = 2000.0;

    /// The approach shape; dive-bomb rollout is unresolved.
    pub fn approach_profile(self) -> Result<ApproachProfile> {
        match self {
            Self::StraightRun => Ok(ApproachProfile {
                vertical_offset_ft: None,
                turn_in_distance_ft: None,
                angular_offset_deg: None,
                exit_distance_ft: Self::STRAIGHT_RUN_EXIT_FT,
            }),
            Self::PopUp => Ok(ApproachProfile {
                vertical_offset_ft: Some(Self::POP_UP_VERTICAL_OFFSET_FT),
                turn_in_distance_ft: Some(Self::POP_UP_TURN_IN_FT),
                angular_offset_deg: Some(Self::POP_UP_ANGULAR_OFFSET_DEG),
                exit_distance_ft: Self::POP_UP_EXIT_FT,
            }),
            Self::FastHighPass => Ok(ApproachProfile {
                vertical_offset_ft: Some(Self::FAST_HIGH_VERTICAL_OFFSET_FT),
                turn_in_distance_ft: None,
                angular_offset_deg: None,
                exit_distance_ft: Self::FAST_HIGH_EXIT_FT,
            }),
            Self::DiveBomb => Err(AiError::UnspecifiedRule("B14 dive-bomb rollout")),
        }
    }
}

/// B14: dive-bomb requires altitude and horizontal distance each at least
/// this.
pub fn dive_bomb_min_distance_ft(turn_radius_ft: f64) -> f64 {
    DIVE_BOMB_MIN_DISTANCE_FT.max(2.0 * turn_radius_ft)
}

/// B14: egress departs, climbing, until at least this far away before
/// turning back. Horizontal separation, not an altitude goal.
pub fn egress_departure_distance_ft(turn_radius_ft: f64) -> f64 {
    EGRESS_DEPARTURE_MIN_DISTANCE_FT.max(2.0 * turn_radius_ft)
}

/// B14: 25% alternative selection; within it, dive-bomb when the distance
/// conditions hold behind a 33% gate, else fast-high pass one third and
/// pop-up two thirds. Otherwise the ordinary straight run.
pub fn surface_attack_choice(
    situation: &SurfaceAttackSituation,
    random: &mut DecisionRandom,
) -> SurfaceAttack {
    if !random
        .site("surface alternative attack")
        .chance(SURFACE_ALTERNATIVE_PERCENT)
    {
        return SurfaceAttack::StraightRun;
    }
    let min_distance = dive_bomb_min_distance_ft(situation.turn_radius_ft);
    if situation.altitude_ft >= min_distance
        && situation.horizontal_distance_ft >= min_distance
        && random.site("dive bomb").chance(DIVE_BOMB_PERCENT)
    {
        return SurfaceAttack::DiveBomb;
    }
    if random.site("fast high pass").choose(FAST_HIGH_PASS_ONE_IN) == 0 {
        SurfaceAttack::FastHighPass
    } else {
        SurfaceAttack::PopUp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIALS: usize = 100_000;

    fn within(count: usize, percent: f64) -> bool {
        let expected = TRIALS as f64 * percent / 100.0;
        (count as f64 - expected).abs() <= TRIALS as f64 * 0.01
    }

    fn aircraft(fighter: bool, human: bool) -> TacticalSituation {
        TacticalSituation {
            target: TargetClass::Aircraft { fighter, human },
            spatial_distance_ft: 8000.0,
            ahead: true,
            facing: true,
            off_beam_deg: 10.0,
            heading_difference_deg: 170.0,
            pitch_difference_deg: 5.0,
            wing_combat: false,
            wing_approach: false,
            wing_approach_value_ft: None,
            own_agl_ft: 10000.0,
        }
    }

    fn novice() -> TacticalThresholds {
        TacticalThresholds {
            ahead_facing: QuadrantThresholds {
                best_attack_percent: 11,
                random_tactic_percent: 50,
            },
            ahead_facing_away: QuadrantThresholds {
                best_attack_percent: 8,
                random_tactic_percent: 72,
            },
            behind_facing: QuadrantThresholds {
                best_attack_percent: 16,
                random_tactic_percent: 42,
            },
            behind_facing_away: QuadrantThresholds {
                best_attack_percent: 16,
                random_tactic_percent: 42,
            },
            straight_on_random_menu_percent: 52,
            chased_vertical_displacement_percent: 35,
            pursuit_offset_bound: 100,
        }
    }

    fn ace() -> TacticalThresholds {
        TacticalThresholds {
            ahead_facing: QuadrantThresholds {
                best_attack_percent: 84,
                random_tactic_percent: 6,
            },
            ahead_facing_away: QuadrantThresholds {
                best_attack_percent: 84,
                random_tactic_percent: 6,
            },
            behind_facing: QuadrantThresholds {
                best_attack_percent: 90,
                random_tactic_percent: 4,
            },
            behind_facing_away: QuadrantThresholds {
                best_attack_percent: 84,
                random_tactic_percent: 6,
            },
            straight_on_random_menu_percent: 0,
            chased_vertical_displacement_percent: 95,
            pursuit_offset_bound: 10,
        }
    }

    // B10

    #[test]
    fn infrared_launch_climbs_to_90_and_breaks_wingman_half_the_time() {
        let mut random = DecisionRandom::seeded(1);
        let inputs = MissileReactionInputs {
            current_heading_deg: 350,
            target_bearing_deg: 20,
            off_beam_deg: 120.0,
        };
        let mut breaks = 0;
        for _ in 0..TRIALS {
            let r = missile_reaction(MissileLaunch::Infrared, inputs, &mut random);
            assert_eq!(
                r.reaction,
                MissileReaction::ClimbToPitch90 { heading_deg: 350 }
            );
            match r.wingman_break {
                Some(WingmanBreak::Offsets {
                    heading_deg: 90,
                    pitch_deg: 90,
                }) => breaks += 1,
                None => {}
                other => panic!("{other:?}"),
            }
        }
        assert!(within(breaks, 50.0), "{breaks}");
    }

    #[test]
    fn radar_launch_turns_toward_target_at_or_below_90_off_beam() {
        let mut random = DecisionRandom::seeded(2);
        for off_beam in [0.0, 89.0, 90.0] {
            let inputs = MissileReactionInputs {
                current_heading_deg: 10,
                target_bearing_deg: 400,
                off_beam_deg: off_beam,
            };
            let r = missile_reaction(MissileLaunch::Radar, inputs, &mut random);
            assert_eq!(
                r.reaction,
                MissileReaction::TurnTowardTarget { heading_deg: 40 }
            );
            assert_eq!(r.wingman_break, Some(WingmanBreak::UnrecoveredOffsets));
        }
    }

    #[test]
    fn radar_launch_over_90_off_beam_breaks_ninety_either_way_equally() {
        let mut random = DecisionRandom::seeded(3);
        let inputs = MissileReactionInputs {
            current_heading_deg: 30,
            target_bearing_deg: 200,
            off_beam_deg: 91.0,
        };
        let mut left = 0;
        for _ in 0..TRIALS {
            let r = missile_reaction(MissileLaunch::Radar, inputs, &mut random);
            assert_eq!(r.wingman_break, Some(WingmanBreak::UnrecoveredOffsets));
            match r.reaction {
                MissileReaction::BreakNinety {
                    heading_deg: 300,
                    side: BreakSide::Left,
                } => left += 1,
                MissileReaction::BreakNinety {
                    heading_deg: 120,
                    side: BreakSide::Right,
                } => {}
                other => panic!("{other:?}"),
            }
        }
        assert!(within(left, 50.0), "{left}");
    }

    // B11

    #[test]
    fn non_aircraft_evasion_altitude_operand_is_max_of_agl_and_20000() {
        let mut random = DecisionRandom::seeded(4);
        let mut s = aircraft(false, false);
        s.target = TargetClass::NonAircraft;
        s.own_agl_ft = 1500.0;
        let low = evasion_choice(&s, &mut random);
        assert_eq!(
            low,
            BehaviorChoice::NonAircraftEvasion(NonAircraftEvasion {
                altitude_operand_ft: 20000.0,
                altitude_frame: AltitudeFrame::Unresolved,
            })
        );
        s.own_agl_ft = 25000.0;
        let high = evasion_choice(&s, &mut random);
        assert_eq!(
            high,
            BehaviorChoice::NonAircraftEvasion(NonAircraftEvasion::for_agl(25000.0))
        );
    }

    #[test]
    fn coordinated_escape_needs_all_conditions_then_enters_30_percent_evenly() {
        let mut s = aircraft(true, false);
        s.ahead = false;
        s.facing = true;
        s.wing_combat = true;
        s.wing_approach = true;
        assert!(coordinated_escape_eligible(&s));
        for breaker in [
            |s: &mut TacticalSituation| s.ahead = true,
            |s: &mut TacticalSituation| s.facing = false,
            |s: &mut TacticalSituation| s.wing_combat = false,
            |s: &mut TacticalSituation| s.wing_approach = false,
            |s: &mut TacticalSituation| s.target = TargetClass::NonAircraft,
        ] {
            let mut broken = s;
            breaker(&mut broken);
            assert!(!coordinated_escape_eligible(&broken));
        }
        let mut random = DecisionRandom::seeded(5);
        let mut counts = [0usize; 3];
        let mut skipped = 0;
        for _ in 0..TRIALS {
            match coordinated_escape(&s, &mut random) {
                None => skipped += 1,
                Some(CoordinatedEscape::CrossTurn) => counts[0] += 1,
                Some(CoordinatedEscape::LeftRightSplit) => counts[1] += 1,
                Some(CoordinatedEscape::HighLowSplit) => counts[2] += 1,
            }
        }
        assert!(within(skipped, 70.0), "{skipped}");
        for c in counts {
            assert!(within(c, 10.0), "{counts:?}");
        }
    }

    #[test]
    fn last_ditch_entry_boundaries() {
        let mut s = aircraft(true, false);
        s.ahead = false;
        s.facing = true;
        s.spatial_distance_ft = 5000.0;
        s.heading_difference_deg = 35.0;
        assert!(last_ditch_entry(&s));
        s.spatial_distance_ft = 5001.0;
        assert!(!last_ditch_entry(&s));
        s.spatial_distance_ft = 5000.0;
        s.heading_difference_deg = 36.0;
        assert!(!last_ditch_entry(&s));
        s.heading_difference_deg = 35.0;
        s.ahead = true;
        assert!(!last_ditch_entry(&s));
        s.ahead = false;
        s.facing = false;
        assert!(!last_ditch_entry(&s));
    }

    #[test]
    fn evasion_falls_back_to_fly_away_at_30000_and_jink_below() {
        let mut random = DecisionRandom::seeded(6);
        let mut s = aircraft(true, false);
        s.ahead = false;
        s.facing = true;
        s.heading_difference_deg = 90.0;
        s.spatial_distance_ft = 30000.0;
        assert_eq!(evasion_choice(&s, &mut random), BehaviorChoice::FlyAway);
        s.spatial_distance_ft = 29999.0;
        assert_eq!(
            evasion_choice(&s, &mut random),
            BehaviorChoice::VerticalJink
        );
        s.spatial_distance_ft = 4000.0;
        s.heading_difference_deg = 20.0;
        assert_eq!(evasion_choice(&s, &mut random), BehaviorChoice::LastDitch);
    }

    #[test]
    fn last_ditch_selection_is_unspecified_but_candidates_are_listed() {
        let mut random = DecisionRandom::seeded(7);
        let s = aircraft(true, false);
        assert_eq!(LastDitchCandidate::ALL.len(), 7);
        assert!(matches!(
            select_last_ditch(&s, &mut random),
            Err(AiError::UnspecifiedRule(_))
        ));
    }

    // B12

    #[test]
    fn air_to_air_entry_raises_wing_spacing_to_5000() {
        assert_eq!(air_to_air_wing_spacing(1000.0), 5000.0);
        assert_eq!(air_to_air_wing_spacing(5000.0), 5000.0);
        assert_eq!(air_to_air_wing_spacing(7000.0), 7000.0);
    }

    #[test]
    fn wing_split_boundaries() {
        let mut s = aircraft(true, false);
        for (value, expected) in [
            (None, false),
            (Some(0.0), false),
            (Some(4999.0), false),
            (Some(5000.0), true),
            (Some(30000.0), true),
            (Some(30001.0), false),
        ] {
            s.wing_approach_value_ft = value;
            assert_eq!(wing_split_eligible(&s), expected, "{value:?}");
        }
        s.wing_approach_value_ft = Some(10000.0);
        s.ahead = false;
        assert!(!wing_split_eligible(&s));
        s.ahead = true;
        s.target = TargetClass::Aircraft {
            fighter: false,
            human: false,
        };
        assert!(!wing_split_eligible(&s));
    }

    #[test]
    fn special_approach_boundaries() {
        let mut s = aircraft(false, true);
        s.spatial_distance_ft = 5000.0;
        s.off_beam_deg = 20.0;
        s.heading_difference_deg = 155.0;
        s.pitch_difference_deg = 25.0;
        assert!(special_approach_eligible(&s));
        s.spatial_distance_ft = 20000.0;
        assert!(special_approach_eligible(&s));
        s.spatial_distance_ft = 4999.0;
        assert!(!special_approach_eligible(&s));
        s.spatial_distance_ft = 20001.0;
        assert!(!special_approach_eligible(&s));
        s.spatial_distance_ft = 10000.0;
        s.off_beam_deg = 21.0;
        assert!(!special_approach_eligible(&s));
        s.off_beam_deg = 20.0;
        s.heading_difference_deg = 154.0;
        assert!(!special_approach_eligible(&s));
        s.heading_difference_deg = 155.0;
        s.pitch_difference_deg = 26.0;
        assert!(!special_approach_eligible(&s));
        s.pitch_difference_deg = 25.0;
        s.target = TargetClass::Aircraft {
            fighter: true,
            human: false,
        };
        assert!(!special_approach_eligible(&s));
    }

    #[test]
    fn special_approach_enters_76_percent_and_splits_passes_evenly() {
        let mut random = DecisionRandom::seeded(8);
        let s = aircraft(false, true);
        assert!(special_approach_eligible(&s));
        let mut offset = 0;
        let mut overhead = 0;
        for _ in 0..TRIALS {
            match special_approach(&s, &mut random) {
                Some(SpecialApproach::OffsetPass) => offset += 1,
                Some(SpecialApproach::OverheadPass) => overhead += 1,
                None => {}
            }
        }
        assert!(within(offset + overhead, 76.0), "{}", offset + overhead);
        assert!(within(offset, 38.0), "{offset}");
        assert!(within(overhead, 38.0), "{overhead}");
    }

    #[test]
    fn separation_over_15000_selects_pursuit_directly() {
        let mut random = DecisionRandom::seeded(9);
        let mut s = aircraft(true, false);
        s.spatial_distance_ft = 15001.0;
        for _ in 0..100 {
            assert_eq!(
                approach_choice(&s, Experience::Novice, &novice(), &mut random),
                Ok(BehaviorChoice::Pursuit)
            );
        }
        s.spatial_distance_ft = 15000.0;
        let mut saw_non_pursuit = false;
        for _ in 0..1000 {
            if approach_choice(&s, Experience::Novice, &novice(), &mut random)
                != Ok(BehaviorChoice::Pursuit)
            {
                saw_non_pursuit = true;
            }
        }
        assert!(saw_non_pursuit);
    }

    #[test]
    fn novice_ahead_facing_draws_match_the_combined_table() {
        let mut random = DecisionRandom::seeded(10);
        let s = aircraft(true, false);
        let mut pursuit = 0;
        let mut straight = 0;
        let mut random_rest = 0;
        let mut remaining = 0;
        for _ in 0..TRIALS {
            match approach_choice(&s, Experience::Novice, &novice(), &mut random) {
                Ok(BehaviorChoice::Pursuit) => pursuit += 1,
                Ok(BehaviorChoice::Straight) => straight += 1,
                Err(AiError::UnspecifiedRule(what)) if what.starts_with("random-tactic") => {
                    random_rest += 1;
                }
                Err(AiError::UnspecifiedRule(what)) if what.starts_with("B12 remaining") => {
                    remaining += 1;
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(within(pursuit, 11.0), "{pursuit}");
        assert!(within(straight, 44.5 * 0.52), "{straight}");
        assert!(within(random_rest, 44.5 * 0.48), "{random_rest}");
        assert!(within(remaining, 44.5), "{remaining}");
    }

    #[test]
    fn ace_ahead_facing_draws_match_the_combined_table() {
        let mut random = DecisionRandom::seeded(11);
        let s = aircraft(true, false);
        let mut pursuit = 0;
        let mut random_rest = 0;
        let mut remaining = 0;
        for _ in 0..TRIALS {
            match approach_choice(&s, Experience::Ace, &ace(), &mut random) {
                Ok(BehaviorChoice::Pursuit) => pursuit += 1,
                Err(AiError::UnspecifiedRule(what)) if what.starts_with("random-tactic") => {
                    random_rest += 1;
                }
                Err(AiError::UnspecifiedRule(what)) if what.starts_with("B12 remaining") => {
                    remaining += 1;
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(within(pursuit, 84.0), "{pursuit}");
        assert!(within(random_rest, 0.96), "{random_rest}");
        assert!(within(remaining, 15.04), "{remaining}");
    }

    #[test]
    fn novice_behind_facing_has_an_earlier_40_percent_straight_choice() {
        let mut random = DecisionRandom::seeded(12);
        let mut s = aircraft(true, false);
        s.ahead = false;
        s.facing = true;
        s.heading_difference_deg = 90.0;
        let mut straight = 0;
        for _ in 0..TRIALS {
            if approach_choice(&s, Experience::Novice, &novice(), &mut random)
                == Ok(BehaviorChoice::Straight)
            {
                straight += 1;
            }
        }
        // 40% early, then 60% * (1 - 0.16) * 0.42 * 0.52 from the menu.
        let expected = 40.0 + 60.0 * 0.84 * 0.42 * 0.52;
        assert!(within(straight, expected), "{straight} vs {expected}");

        let mut random = DecisionRandom::seeded(13);
        let mut early = 0;
        for _ in 0..TRIALS {
            let mut t = ace();
            t.behind_facing.random_tactic_percent = 0;
            if approach_choice(&s, Experience::Ace, &t, &mut random) == Ok(BehaviorChoice::Straight)
            {
                early += 1;
            }
        }
        assert_eq!(early, 0);
    }

    #[test]
    fn best_attack_chooses_last_ditch_on_75_percent_only_in_close_behind_geometry() {
        let mut s = aircraft(true, false);
        s.ahead = false;
        s.facing = true;
        s.spatial_distance_ft = 2000.0;
        s.heading_difference_deg = 35.0;
        let mut random = DecisionRandom::seeded(14);
        let last_ditch = (0..TRIALS)
            .filter(|_| best_attack(&s, &mut random) == BehaviorChoice::LastDitch)
            .count();
        assert!(within(last_ditch, 75.0), "{last_ditch}");

        let always_pursuit = |s: &TacticalSituation| {
            let mut random = DecisionRandom::seeded(15);
            (0..1000).all(|_| best_attack(s, &mut random) == BehaviorChoice::Pursuit)
        };
        let mut far = s;
        far.spatial_distance_ft = 2001.0;
        assert!(always_pursuit(&far));
        let mut wide = s;
        wide.heading_difference_deg = 36.0;
        assert!(always_pursuit(&wide));
        let mut ahead = s;
        ahead.ahead = true;
        assert!(always_pursuit(&ahead));
        let mut away = s;
        away.facing = false;
        assert!(always_pursuit(&away));
        let mut bomber = s;
        bomber.target = TargetClass::Aircraft {
            fighter: false,
            human: false,
        };
        assert!(always_pursuit(&bomber));
    }

    #[test]
    fn wing_split_and_special_approach_take_priority_in_approach_flow() {
        let mut random = DecisionRandom::seeded(16);
        let mut s = aircraft(true, true);
        s.wing_approach_value_ft = Some(12000.0);
        assert_eq!(
            approach_choice(&s, Experience::Ace, &ace(), &mut random),
            Ok(BehaviorChoice::WingSplit)
        );
        s.wing_approach_value_ft = None;
        let mut passes = 0;
        for _ in 0..TRIALS {
            if let Ok(BehaviorChoice::SpecialApproach(_)) =
                approach_choice(&s, Experience::Ace, &ace(), &mut random)
            {
                passes += 1;
            }
        }
        assert!(within(passes, 76.0), "{passes}");
    }

    // Pursuit offsets

    #[test]
    fn pursuit_offsets_draw_below_the_level_bound_and_pick_duration_by_distance() {
        let mut random = DecisionRandom::seeded(17);
        let novice = novice();
        let mut max_seen = 0;
        for _ in 0..TRIALS {
            let o = pursuit_offsets(PursuitCondition::Ordinary, &novice, 4999.0, &mut random);
            assert_eq!(o.nominal_duration_s, 6);
            for c in [o.lateral_ft, o.longitudinal_ft, o.vertical_ft] {
                assert!((0..100).contains(&c), "{c}");
                max_seen = max_seen.max(c);
            }
        }
        assert_eq!(max_seen, 99);
        let o = pursuit_offsets(PursuitCondition::Ordinary, &novice, 5000.0, &mut random);
        assert_eq!(o.nominal_duration_s, 12);
        let ace = ace();
        for _ in 0..1000 {
            let o = pursuit_offsets(PursuitCondition::Ordinary, &ace, 20000.0, &mut random);
            assert!((0..10).contains(&o.lateral_ft));
            assert!((0..10).contains(&o.longitudinal_ft));
            assert!((0..10).contains(&o.vertical_ft));
        }
    }

    #[test]
    fn head_on_zeros_all_offsets() {
        let mut random = DecisionRandom::seeded(18);
        let o = pursuit_offsets(PursuitCondition::HeadOn, &novice(), 3000.0, &mut random);
        assert_eq!((o.lateral_ft, o.longitudinal_ft, o.vertical_ft), (0, 0, 0));
    }

    #[test]
    fn chased_vertical_displacement_follows_the_level_gate_with_equal_signs() {
        let mut random = DecisionRandom::seeded(19);
        let mut up = 0;
        let mut down = 0;
        for _ in 0..TRIALS {
            let o = pursuit_offsets(PursuitCondition::Chased, &novice(), 3000.0, &mut random);
            match o.vertical_ft {
                2000 => up += 1,
                -2000 => down += 1,
                v => assert!((0..100).contains(&v)),
            }
        }
        assert!(within(up + down, 35.0), "{}", up + down);
        assert!(within(up, 17.5), "{up}");
        assert!(within(down, 17.5), "{down}");
        let mut random = DecisionRandom::seeded(20);
        let displaced = (0..TRIALS)
            .filter(|_| {
                pursuit_offsets(PursuitCondition::Chased, &ace(), 3000.0, &mut random)
                    .vertical_ft
                    .abs()
                    == 2000
            })
            .count();
        assert!(within(displaced, 95.0), "{displaced}");
    }

    // B14

    fn surface(altitude: f64, horizontal: f64, turn_radius: f64) -> SurfaceAttackSituation {
        SurfaceAttackSituation {
            altitude_ft: altitude,
            horizontal_distance_ft: horizontal,
            turn_radius_ft: turn_radius,
        }
    }

    #[test]
    fn surface_attack_rates_with_dive_bomb_available() {
        let mut random = DecisionRandom::seeded(21);
        let s = surface(12000.0, 12000.0, 4000.0);
        let mut counts = [0usize; 4];
        for _ in 0..TRIALS {
            let i = match surface_attack_choice(&s, &mut random) {
                SurfaceAttack::StraightRun => 0,
                SurfaceAttack::DiveBomb => 1,
                SurfaceAttack::FastHighPass => 2,
                SurfaceAttack::PopUp => 3,
            };
            counts[i] += 1;
        }
        assert!(within(counts[0], 75.0), "{counts:?}");
        assert!(within(counts[1], 25.0 * 0.33), "{counts:?}");
        assert!(within(counts[2], 25.0 * 0.67 / 3.0), "{counts:?}");
        assert!(within(counts[3], 25.0 * 0.67 * 2.0 / 3.0), "{counts:?}");
    }

    #[test]
    fn surface_attack_rates_without_dive_bomb() {
        let mut random = DecisionRandom::seeded(22);
        let s = surface(9999.0, 12000.0, 1000.0);
        let mut counts = [0usize; 4];
        for _ in 0..TRIALS {
            let i = match surface_attack_choice(&s, &mut random) {
                SurfaceAttack::StraightRun => 0,
                SurfaceAttack::DiveBomb => 1,
                SurfaceAttack::FastHighPass => 2,
                SurfaceAttack::PopUp => 3,
            };
            counts[i] += 1;
        }
        assert_eq!(counts[1], 0);
        assert!(within(counts[0], 75.0), "{counts:?}");
        assert!(within(counts[2], 25.0 / 3.0), "{counts:?}");
        assert!(within(counts[3], 50.0 / 3.0), "{counts:?}");
    }

    #[test]
    fn dive_bomb_distance_boundaries_use_max_of_10000_and_twice_turn_radius() {
        assert_eq!(dive_bomb_min_distance_ft(1000.0), 10000.0);
        assert_eq!(dive_bomb_min_distance_ft(5000.0), 10000.0);
        assert_eq!(dive_bomb_min_distance_ft(6000.0), 12000.0);
        let dive_possible = |s: SurfaceAttackSituation| {
            let mut random = DecisionRandom::seeded(23);
            (0..2000).any(|_| surface_attack_choice(&s, &mut random) == SurfaceAttack::DiveBomb)
        };
        assert!(dive_possible(surface(10000.0, 10000.0, 1000.0)));
        assert!(!dive_possible(surface(9999.0, 10000.0, 1000.0)));
        assert!(!dive_possible(surface(10000.0, 9999.0, 1000.0)));
        assert!(dive_possible(surface(12000.0, 12000.0, 6000.0)));
        assert!(!dive_possible(surface(11999.0, 12000.0, 6000.0)));
    }

    #[test]
    fn approach_profiles_carry_the_spec_distances() {
        let run = SurfaceAttack::StraightRun.approach_profile().unwrap();
        assert_eq!(run.exit_distance_ft, 2000.0);
        assert_eq!(run.vertical_offset_ft, None);
        let pop = SurfaceAttack::PopUp.approach_profile().unwrap();
        assert_eq!(pop.vertical_offset_ft, Some(500.0));
        assert_eq!(pop.turn_in_distance_ft, Some(10000.0));
        assert_eq!(pop.angular_offset_deg, Some(25.0));
        assert_eq!(pop.exit_distance_ft, 1500.0);
        let fast = SurfaceAttack::FastHighPass.approach_profile().unwrap();
        assert_eq!(fast.vertical_offset_ft, Some(5000.0));
        assert_eq!(fast.exit_distance_ft, 2000.0);
        assert!(matches!(
            SurfaceAttack::DiveBomb.approach_profile(),
            Err(AiError::UnspecifiedRule(_))
        ));
        assert_eq!(EGRESS_JINK_WITHIN_HORIZONTAL_FT, 10000.0);
        assert_eq!(egress_departure_distance_ft(1000.0), 30000.0);
        assert_eq!(egress_departure_distance_ft(20000.0), 40000.0);
    }
}
