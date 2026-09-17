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

    #[test]
    fn bad_distances_are_invalid_input() {
        for distance in [-1., f64::NAN, f64::INFINITY] {
            let list = [hostile(2, distance)];
            let r = select_target(&OWN, SelectionRoute::Ordinary, &list);
            assert!(matches!(r, Err(AiError::InvalidInput(_))), "{distance}");
        }
    }
}
