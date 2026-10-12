//! B41 target retention, eligibility and ranking for an aircraft's weapon
//! target selector.
//!
//! Spec: `docs/spec/ai.md`, section B41. Every number here is that section's.
//! The candidate list is caller-supplied: this module ranks what it is given
//! and does not model sensors, information sharing or reacquisition, which
//! B41 leaves open.

use super::{AiError, Result};

/// Opaque object identity, used only to exclude self.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u32);

/// Opaque side identity; same-side candidates are rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Side(pub u32);

/// B41: what the selector knows about one candidate. Booleans whose producers
/// are unresolved (`type_allowed`, `seeker_eligible`) are explicit inputs and
/// never defaulted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CandidateTarget {
    pub id: ObjectId,
    pub side: Side,
    /// False for an invalid object (destroyed, removed, never resolved).
    pub valid: bool,
    /// False for a disallowed object type. The allowed-type mask is unresolved.
    pub type_allowed: bool,
    pub is_aircraft: bool,
    /// Reaction policy and available seeker eligibility, resolved by the
    /// caller from target class, mission masks and usable weapon inventory.
    pub seeker_eligible: bool,
    /// Spatial (three-dimensional) separation from the selector, feet.
    pub spatial_distance_feet: f64,
    /// Members of the selector's own wing already attacking this candidate.
    pub wing_attackers: u32,
}

/// B41: the retention shortcut keeps a valid existing target strictly inside
/// this spatial separation while the special retarget-policy flag is clear.
pub const RETENTION_DISTANCE_FEET: f64 = 20000.;
/// B41: score added per applicable penalty condition, feet.
pub const RANKING_PENALTY_FEET: f64 = 10000.;

/// B41: whether an aircraft selector keeps its current target without running
/// selection. A valid target strictly inside 20000 feet is retained while the
/// special retarget-policy flag is clear; exactly 20000 feet proceeds to
/// selection. This is neither a sensor range nor a missile range, and it is
/// not permission to shoot: aircraft have a terrain blocking check in the
/// later firing service (B42). The flag's meaning and producers are
/// unresolved, so it is an explicit input.
pub fn retain_current_target(
    current: Option<&CandidateTarget>,
    special_retarget_policy: bool,
) -> bool {
    !special_retarget_policy
        && current.is_some_and(|target| {
            target.valid && target.spatial_distance_feet < RETENTION_DISTANCE_FEET
        })
}

/// B41: the corresponding non-aircraft actor shortcut, which additionally
/// requires the current target not to be terrain blocked. Only this shortcut
/// is specified for non-aircraft actors; their full selector is not (see
/// [`select_target`] with [`SelectorKind::Surface`]).
pub fn retain_current_target_non_aircraft(
    current: Option<&CandidateTarget>,
    special_retarget_policy: bool,
    terrain_blocked: bool,
) -> bool {
    !terrain_blocked && retain_current_target(current, special_retarget_policy)
}

/// Which actor kind is selecting. Surface actors have additional ranking
/// terms and special zone behavior that B41 does not specify.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorKind {
    Aircraft,
    Surface,
}

/// B41: which selection route the caller wants. The mission/defense-priority
/// route bypasses distance ranking and is unspecified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionRoute {
    Ordinary,
    MissionDefensePriority,
}

/// The selecting actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selector {
    pub id: ObjectId,
    pub side: Side,
    pub kind: SelectorKind,
    /// Wing assignment allowance: the helper returns 1, 2 or 100 by policy
    /// flags whose meanings are unresolved, so the resolved value is an input.
    pub assignment_allowance: u32,
}

/// B41 eligibility: not self, valid, an allowed type, not on the selector's
/// side, and passing the caller-resolved seeker eligibility.
pub fn is_eligible(selector: &Selector, candidate: &CandidateTarget) -> bool {
    candidate.id != selector.id
        && candidate.valid
        && candidate.type_allowed
        && candidate.side != selector.side
        && candidate.seeker_eligible
}

/// B41 aircraft ranking score, feet: spatial distance plus 10000 for each of
/// the candidate not being an aircraft, at least one same-wing member already
/// attacking it, and the wing attacker count meeting or exceeding the
/// assignment allowance. The last two can both apply.
pub fn candidate_score(candidate: &CandidateTarget, assignment_allowance: u32) -> f64 {
    let penalties = [
        !candidate.is_aircraft,
        candidate.wing_attackers >= 1,
        candidate.wing_attackers >= assignment_allowance,
    ];
    let penalty_count = penalties.iter().filter(|applies| **applies).count();
    candidate.spatial_distance_feet + RANKING_PENALTY_FEET * penalty_count as f64
}

/// B41: the aircraft selector's ordinary choice among `candidates`, or `None`
/// when nothing is eligible. The lowest score wins; an equal score does not
/// displace the current best, so among ties the first in the caller's order
/// is chosen. That order is the caller's, not a recovered enumeration order,
/// and is not a gameplay requirement.
///
/// Errors: `UnspecifiedRule` for a surface selector or the
/// mission/defense-priority route; `InvalidInput` for a negative or
/// non-finite distance.
pub fn select_target<'a>(
    selector: &Selector,
    route: SelectionRoute,
    candidates: &'a [CandidateTarget],
) -> Result<Option<&'a CandidateTarget>> {
    match selector.kind {
        SelectorKind::Aircraft => {}
        SelectorKind::Surface => {
            return Err(AiError::UnspecifiedRule(
                "B41 surface selector ranking terms and zone behavior",
            ));
        }
    }
    match route {
        SelectionRoute::Ordinary => {}
        SelectionRoute::MissionDefensePriority => {
            return Err(AiError::UnspecifiedRule(
                "B41 mission/defense-priority selection route",
            ));
        }
    }
    let mut best: Option<(&CandidateTarget, f64)> = None;
    for candidate in candidates
        .iter()
        .filter(|candidate| is_eligible(selector, candidate))
    {
        let distance = candidate.spatial_distance_feet;
        if !(distance.is_finite() && distance >= 0.) {
            return Err(AiError::InvalidInput(
                "candidate spatial distance must be finite and non-negative",
            ));
        }
        let score = candidate_score(candidate, selector.assignment_allowance);
        if best.is_none_or(|(_, best_score)| score < best_score) {
            best = Some((candidate, score));
        }
    }
    Ok(best.map(|(candidate, _)| candidate))
}

/// Where a strike candidate comes from, highest priority first
/// (docs/spec/ai-strike.md, "What the AI knows"; plan section 4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StrikeSource {
    /// A flight lead's order or a taken-over designation: retail makes the
    /// ordered object the only candidate.
    Ordered,
    /// A target the mission's briefing names.
    Briefed,
    /// A hostile ground object the striker has seen near the objective.
    Opportunity,
}

/// What the strike selector knows about one ground candidate. The caller
/// builds it from the striker's ground picture
/// ([`crate::ai::awareness::ground::GroundPicture::strike_candidates`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrikeCandidate {
    pub id: ObjectId,
    pub side: Side,
    pub source: StrikeSource,
    /// Standing (not destroyed) as far as the striker knows.
    pub valid: bool,
    /// A carried surface store can attack it.
    pub usable: bool,
    /// Spatial separation from the striker, feet.
    pub spatial_distance_feet: f64,
    /// Other members of the striker's wing already attacking it.
    pub wing_attackers: u32,
    /// Opportunity only: within [`super::engagement::OPPORTUNITY_RADIUS_FT`]
    /// of the objective.
    pub near_objective: bool,
}

/// At most two attackers on one ground object (the B41 allowance,
/// docs/spec/ai-strike.md, "What the AI knows").
pub const STRIKE_ATTACKER_ALLOWANCE: u32 = 2;

/// The aircraft selector over ground targets (docs/spec/ai-strike.md, "What
/// the AI knows"; the surface half of B41 for a striking aircraft). Rules, in
/// order:
///
/// 1. An ordered target that stands is chosen alone (retail: the ordered
///    object is the only candidate), whatever its attackers.
/// 2. Otherwise the nearest briefed target that stands and has fewer than
///    [`STRIKE_ATTACKER_ALLOWANCE`] other wing attackers.
/// 3. Only once no briefed target it can attack stands: the nearest hostile
///    object seen within 10 nm of the objective, under the same attacker
///    allowance.
///
/// Every candidate must be on another side and attackable with a carried
/// store. Ties keep the lower id, so the choice never depends on the
/// caller's order. `None` when nothing qualifies, including briefed targets
/// that stand but are all fully attacked: the striker then waits for one to
/// free up rather than taking a third place (opinionated, agent). A
/// non-finite or negative distance makes a candidate ineligible.
pub fn select_strike_target(
    own: ObjectId,
    own_side: Side,
    candidates: &[StrikeCandidate],
) -> Option<&StrikeCandidate> {
    let eligible = |c: &&StrikeCandidate| {
        c.id != own
            && c.valid
            && c.usable
            && c.side != own_side
            && c.spatial_distance_feet.is_finite()
            && c.spatial_distance_feet >= 0.
    };
    let nearest = |source: StrikeSource| {
        candidates
            .iter()
            .filter(eligible)
            .filter(|c| c.source == source)
            .filter(|c| c.wing_attackers < STRIKE_ATTACKER_ALLOWANCE)
            .filter(|c| source != StrikeSource::Opportunity || c.near_objective)
            .min_by(|a, b| {
                a.spatial_distance_feet
                    .total_cmp(&b.spatial_distance_feet)
                    .then(a.id.0.cmp(&b.id.0))
            })
    };
    if let Some(ordered) = candidates
        .iter()
        .filter(eligible)
        .filter(|c| c.source == StrikeSource::Ordered)
        .min_by_key(|c| c.id.0)
    {
        return Some(ordered);
    }
    if candidates
        .iter()
        .any(|c| c.source == StrikeSource::Briefed && c.valid && c.usable && c.side != own_side)
    {
        return nearest(StrikeSource::Briefed);
    }
    nearest(StrikeSource::Opportunity)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN: Selector = Selector {
        id: ObjectId(1),
        side: Side(1),
        kind: SelectorKind::Aircraft,
        assignment_allowance: 2,
    };

    fn hostile(id: u32, distance: f64) -> CandidateTarget {
        CandidateTarget {
            id: ObjectId(id),
            side: Side(2),
            valid: true,
            type_allowed: true,
            is_aircraft: true,
            seeker_eligible: true,
            spatial_distance_feet: distance,
            wing_attackers: 0,
        }
    }

    fn pick(candidates: &[CandidateTarget]) -> Option<&CandidateTarget> {
        select_target(&OWN, SelectionRoute::Ordinary, candidates).unwrap()
    }

    #[test]
    fn retention_boundary_at_20000_feet() {
        assert!(retain_current_target(Some(&hostile(2, 19999.)), false));
        assert!(!retain_current_target(Some(&hostile(2, 20000.)), false));
        assert!(!retain_current_target(Some(&hostile(2, 19999.)), true));
        assert!(!retain_current_target(None, false));
        let mut gone = hostile(2, 100.);
        gone.valid = false;
        assert!(!retain_current_target(Some(&gone), false));
    }

    #[test]
    fn non_aircraft_retention_also_checks_terrain() {
        let near = hostile(2, 19999.);
        assert!(retain_current_target_non_aircraft(
            Some(&near),
            false,
            false
        ));
        assert!(!retain_current_target_non_aircraft(
            Some(&near),
            false,
            true
        ));
        assert!(!retain_current_target_non_aircraft(
            Some(&hostile(2, 20000.)),
            false,
            false
        ));
    }

    #[test]
    fn each_penalty_adds_10000() {
        let plain = hostile(2, 1000.);
        assert_eq!(candidate_score(&plain, 2), 1000.);
        let mut ground = plain;
        ground.is_aircraft = false;
        assert_eq!(candidate_score(&ground, 2), 11000.);
        let mut attacked = plain;
        attacked.wing_attackers = 1;
        assert_eq!(candidate_score(&attacked, 2), 11000.);
        // Allowance 1: a single attacker trips both wing penalties.
        assert_eq!(candidate_score(&attacked, 1), 21000.);
        let mut saturated = plain;
        saturated.wing_attackers = 2;
        assert_eq!(candidate_score(&saturated, 2), 21000.);
        assert_eq!(candidate_score(&saturated, 100), 11000.);
        // All three together: 30000 added.
        let mut all = saturated;
        all.is_aircraft = false;
        assert_eq!(candidate_score(&all, 2), 31000.);
    }

    #[test]
    fn wing_penalties_are_cumulative_in_selection() {
        // Attacked and at the allowance: both wing penalties, 20000 total, so
        // an unoccupied aircraft up to 20000 feet farther is preferred.
        let mut saturated = hostile(2, 1000.);
        saturated.wing_attackers = 2;
        assert_eq!(candidate_score(&saturated, 2), 21000.);
        assert_eq!(
            pick(&[saturated, hostile(3, 20999.)]).unwrap().id,
            ObjectId(3)
        );
        assert_eq!(
            pick(&[saturated, hostile(3, 21000.)]).unwrap().id,
            ObjectId(2)
        );
        // Non-aircraft as well: 30000 total.
        saturated.is_aircraft = false;
        assert_eq!(candidate_score(&saturated, 2), 31000.);
        assert_eq!(
            pick(&[saturated, hostile(3, 30999.)]).unwrap().id,
            ObjectId(3)
        );
        assert_eq!(
            pick(&[saturated, hostile(3, 31000.)]).unwrap().id,
            ObjectId(2)
        );
    }

    #[test]
    fn closer_non_aircraft_loses_to_farther_aircraft_inside_the_penalty() {
        let mut ground = hostile(2, 3000.);
        ground.is_aircraft = false;
        let aircraft = hostile(3, 12999.);
        assert_eq!(pick(&[ground, aircraft]).unwrap().id, ObjectId(3));
        // Once the aircraft is a full penalty farther, the tie keeps the first.
        let aircraft = hostile(3, 13000.);
        assert_eq!(pick(&[ground, aircraft]).unwrap().id, ObjectId(2));
        let farther = hostile(3, 13001.);
        assert_eq!(pick(&[ground, farther]).unwrap().id, ObjectId(2));
    }

    #[test]
    fn ties_keep_the_first_in_caller_order() {
        let a = hostile(2, 5000.);
        let b = hostile(3, 5000.);
        assert_eq!(pick(&[a, b]).unwrap().id, ObjectId(2));
        assert_eq!(pick(&[b, a]).unwrap().id, ObjectId(3));
        let closer = hostile(4, 4999.);
        assert_eq!(pick(&[a, b, closer]).unwrap().id, ObjectId(4));
    }

    #[test]
    fn ineligible_candidates_are_excluded() {
        let mut me = hostile(1, 10.);
        me.side = Side(2);
        let mut friend = hostile(5, 20.);
        friend.side = Side(1);
        let mut dead = hostile(6, 30.);
        dead.valid = false;
        let mut wrong_type = hostile(7, 40.);
        wrong_type.type_allowed = false;
        let mut blind = hostile(8, 50.);
        blind.seeker_eligible = false;
        let real = hostile(9, 60.);
        let list = [me, friend, dead, wrong_type, blind, real];
        assert_eq!(pick(&list).unwrap().id, ObjectId(9));
        assert!(pick(&list[..5]).is_none());
        assert!(pick(&[]).is_none());
    }

    #[test]
    fn surface_selector_and_priority_route_are_unspecified() {
        let surface = Selector {
            kind: SelectorKind::Surface,
            ..OWN
        };
        let list = [hostile(2, 100.)];
        let r = select_target(&surface, SelectionRoute::Ordinary, &list);
        assert!(matches!(r, Err(AiError::UnspecifiedRule(_))));
        let r = select_target(&OWN, SelectionRoute::MissionDefensePriority, &list);
        assert!(matches!(r, Err(AiError::UnspecifiedRule(_))));
    }

    fn ground(id: u32, source: StrikeSource, distance: f64) -> StrikeCandidate {
        StrikeCandidate {
            id: ObjectId(id),
            side: Side(2),
            source,
            valid: true,
            usable: true,
            spatial_distance_feet: distance,
            wing_attackers: 0,
            near_objective: true,
        }
    }

    fn strike(candidates: &[StrikeCandidate]) -> Option<u32> {
        select_strike_target(OWN.id, OWN.side, candidates).map(|c| c.id.0)
    }

    #[test]
    fn an_ordered_target_comes_first_whatever_its_attackers() {
        let mut ordered = ground(20, StrikeSource::Ordered, 90_000.);
        ordered.wing_attackers = 3;
        let briefed = ground(10, StrikeSource::Briefed, 1_000.);
        assert_eq!(strike(&[briefed, ordered]), Some(20));
        // A destroyed or friendly order falls back to the briefing.
        let mut gone = ordered;
        gone.valid = false;
        assert_eq!(strike(&[briefed, gone]), Some(10));
        let mut friendly = ordered;
        friendly.side = Side(1);
        assert_eq!(strike(&[briefed, friendly]), Some(10));
    }

    #[test]
    fn briefed_targets_go_nearest_first_with_two_attackers_at_most() {
        let near = ground(11, StrikeSource::Briefed, 3_000.);
        let far = ground(12, StrikeSource::Briefed, 9_000.);
        assert_eq!(strike(&[far, near]), Some(11));
        let mut full = near;
        full.wing_attackers = 2;
        assert_eq!(strike(&[far, full]), Some(12));
        let mut one = near;
        one.wing_attackers = 1;
        assert_eq!(strike(&[far, one]), Some(11));
        // Equal distances keep the lower id, in any order.
        let twin = ground(9, StrikeSource::Briefed, 3_000.);
        assert_eq!(strike(&[near, twin]), Some(9));
        assert_eq!(strike(&[twin, near]), Some(9));
        // Every briefed target fully attacked: wait, never a third place and
        // never an opportunity while they stand.
        let mut far_full = far;
        far_full.wing_attackers = 2;
        let seen = ground(30, StrikeSource::Opportunity, 500.);
        assert_eq!(strike(&[full, far_full, seen]), None);
    }

    #[test]
    fn opportunity_targets_wait_for_the_briefing_and_stay_near_the_objective() {
        let briefed = ground(11, StrikeSource::Briefed, 30_000.);
        let seen = ground(30, StrikeSource::Opportunity, 2_000.);
        assert_eq!(strike(&[briefed, seen]), Some(11));
        let mut destroyed = briefed;
        destroyed.valid = false;
        assert_eq!(strike(&[destroyed, seen]), Some(30));
        let mut away = seen;
        away.near_objective = false;
        assert_eq!(strike(&[destroyed, away]), None);
        let mut unarmed = seen;
        unarmed.usable = false;
        assert_eq!(strike(&[destroyed, unarmed]), None);
        assert_eq!(strike(&[]), None);
        for distance in [-1., f64::NAN, f64::INFINITY] {
            assert_eq!(
                strike(&[ground(31, StrikeSource::Opportunity, distance)]),
                None
            );
        }
    }

    #[test]
    fn bad_distances_are_invalid_input() {
        for distance in [-1., f64::NAN, f64::INFINITY] {
            let list = [hostile(2, distance)];
            let r = select_target(&OWN, SelectionRoute::Ordinary, &list);
            assert!(matches!(r, Err(AiError::InvalidInput(_))), "{distance}");
        }
    }
}
