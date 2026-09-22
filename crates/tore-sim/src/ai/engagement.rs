//! Mission role and rules-of-engagement filtering for current observations.
//!
//! This module never discovers a target. Callers supply the actor's current
//! [`TargetView`] observations and reports containing only independently known
//! attacker identities. Assignment metadata classifies those observations but
//! cannot turn an unobserved object into a target.

use super::{controller::TargetView, targeting};

const FEET_PER_NAUTICAL_MILE: f64 = 6_076.;
pub const ESCORT_EXIT_LEASH_FT: f64 = 10. * FEET_PER_NAUTICAL_MILE;
pub const ESCORT_REENTRY_LEASH_FT: f64 = 8. * FEET_PER_NAUTICAL_MILE;
pub const SUPPORTING_RADAR_IDENTIFICATION_TOLERANCE_DEG: f64 = 2.;

/// Group-level Quick Mission objective before aircraft IDs are assigned.
/// Resolving a group only assigns duties; it never grants sensor knowledge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroupObjective {
    #[default]
    Inherit,
    Free,
    Cap,
    Intercept(super::launch::WingId),
    Escort(super::launch::WingId),
    SelfDefense,
    Hold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    FreeEngagement,
    CombatAirPatrol,
    Intercept,
    Escort,
    Disengage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
    WeaponsHold,
    SelfDefense,
    ProtectAssigned,
    EngageAssigned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostileEscort {
    /// The hostile principal this aircraft is known to escort.
    pub principal_id: u32,
    pub escort_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatrolRegion {
    pub center_ft: [f64; 3],
    pub radius_ft: f64,
}

impl PatrolRegion {
    pub fn contains(self, position: [f64; 3]) -> bool {
        let dx = position[0] - self.center_ft[0];
        let dz = position[2] - self.center_ft[2];
        self.radius_ft.is_finite()
            && self.radius_ft >= 0.
            && dx.mul_add(dx, dz * dz) <= self.radius_ft * self.radius_ft
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    pub role: Role,
    pub stance: Stance,
    pub protected_ids: Vec<u32>,
    pub destroy_ids: Vec<u32>,
    /// Relationships are mission metadata. They affect priority only after the
    /// escort itself is present in the current observation list.
    pub hostile_escorts: Vec<HostileEscort>,
    pub patrol: Option<PatrolRegion>,
}

impl Default for Assignment {
    fn default() -> Self {
        Self {
            role: Role::FreeEngagement,
            stance: Stance::EngageAssigned,
            protected_ids: Vec::new(),
            destroy_ids: Vec::new(),
            hostile_escorts: Vec::new(),
            patrol: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProtectedView {
    /// Mission-logistics position for an explicitly assigned friendly. Callers
    /// must not populate this from an enemy world object or private AI state.
    pub id: u32,
    pub position: [f64; 3],
    pub alive: bool,
}

/// A perceived attack report. `None` means the attack is known but its attacker
/// is not independently identified, so it cannot authorize an aircraft target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreatReport {
    pub attacker_id: Option<u32>,
    pub defended_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    OwnDefense,
    ProtectedThreat,
    HostileEscort,
    Assigned,
    Free,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub id: u32,
    pub priority: Priority,
}

/// Per-aircraft policy state. The only persistence is escort-leash hysteresis.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Policy {
    escort_outside_leash: bool,
}

impl Policy {
    pub fn reset(&mut self) {
        self.escort_outside_leash = false;
    }

    pub fn escort_outside_leash(&self) -> bool {
        self.escort_outside_leash
    }

    pub fn must_rejoin(&self) -> bool {
        self.escort_outside_leash
    }

    /// Whether a frozen observation may guide a search. This never admits the
    /// record to combat selection or weapon employment.
    pub fn allows_investigation(
        &self,
        assignment: &Assignment,
        remembered: &TargetView,
        reports: &[ThreatReport],
        own_id: u32,
    ) -> bool {
        if assignment.stance == Stance::WeaponsHold
            || remembered.id == own_id
            || !remembered.valid
            || !remembered.is_aircraft
        {
            return false;
        }

        let own_attackers = known_attackers(reports, own_id);
        if own_attackers.contains(&remembered.id) {
            return true;
        }
        if assignment.stance == Stance::SelfDefense
            || assignment.role == Role::Disengage
            || self.escort_outside_leash
        {
            return false;
        }

        let protected_attackers: Vec<_> = reports
            .iter()
            .filter(|report| assignment.protected_ids.contains(&report.defended_id))
            .filter_map(|report| report.attacker_id)
            .collect();
        self.mission_priority(assignment, remembered, &protected_attackers)
            .is_some()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn select(
        &mut self,
        own_id: u32,
        own_side: targeting::Side,
        own_position: [f64; 3],
        assignment: &Assignment,
        targets: &[TargetView],
        protected: &[ProtectedView],
        reports: &[ThreatReport],
        current_target: Option<u32>,
        assignment_allowance: u32,
    ) -> Option<Selection> {
        self.update_leash(own_position, assignment, protected);
        if assignment.stance == Stance::WeaponsHold {
            return None;
        }

        let own_attackers = known_attackers(reports, own_id);
        let protected_attackers: Vec<u32> = reports
            .iter()
            .filter(|report| assignment.protected_ids.contains(&report.defended_id))
            .filter_map(|report| report.attacker_id)
            .collect();

        let mut candidates = Vec::new();
        for target in targets {
            if !eligible(own_id, own_side, target) {
                continue;
            }
            let priority = if own_attackers.contains(&target.id) {
                Some(Priority::OwnDefense)
            } else if assignment.stance == Stance::SelfDefense
                || assignment.role == Role::Disengage
                || self.escort_outside_leash
            {
                None
            } else {
                self.mission_priority(assignment, target, &protected_attackers)
            };
            if let Some(priority) = priority {
                candidates.push((target, priority));
            }
        }

        let best_priority = candidates.iter().map(|(_, priority)| *priority).min()?;
        if let Some(id) = current_target
            && candidates
                .iter()
                .any(|(target, priority)| target.id == id && *priority == best_priority)
        {
            return Some(Selection {
                id,
                priority: best_priority,
            });
        }

        candidates
            .into_iter()
            .filter(|(_, priority)| *priority == best_priority)
            .map(|(target, priority)| {
                let distance = spatial_distance(own_position, target.position);
                let candidate = targeting::CandidateTarget {
                    id: targeting::ObjectId(target.id),
                    side: target.side,
                    valid: target.valid,
                    type_allowed: target.type_allowed,
                    is_aircraft: target.is_aircraft,
                    seeker_eligible: target.seeker_eligible,
                    spatial_distance_feet: distance,
                    wing_attackers: target.wing_attackers,
                };
                (
                    Selection {
                        id: target.id,
                        priority,
                    },
                    targeting::candidate_score(&candidate, assignment_allowance),
                )
            })
            .min_by(|(a, a_score), (b, b_score)| {
                a_score.total_cmp(b_score).then_with(|| a.id.cmp(&b.id))
            })
            .map(|(selection, _)| selection)
    }

    fn mission_priority(
        &self,
        assignment: &Assignment,
        target: &TargetView,
        protected_attackers: &[u32],
    ) -> Option<Priority> {
        match assignment.stance {
            Stance::WeaponsHold | Stance::SelfDefense => None,
            Stance::ProtectAssigned => {
                if protected_attackers.contains(&target.id) {
                    Some(Priority::ProtectedThreat)
                } else if assignment.hostile_escorts.iter().any(|relationship| {
                    relationship.escort_id == target.id
                        && protected_attackers.contains(&relationship.principal_id)
                }) {
                    Some(Priority::HostileEscort)
                } else if assignment.destroy_ids.contains(&target.id) {
                    Some(Priority::Assigned)
                } else {
                    None
                }
            }
            Stance::EngageAssigned => match assignment.role {
                Role::FreeEngagement => Some(Priority::Free),
                Role::CombatAirPatrol => assignment
                    .patrol
                    .filter(|region| region.contains(target.position))
                    .map(|_| Priority::Free),
                Role::Intercept | Role::Escort => assignment
                    .destroy_ids
                    .contains(&target.id)
                    .then_some(Priority::Assigned),
                Role::Disengage => None,
            },
        }
    }

    fn update_leash(
        &mut self,
        own_position: [f64; 3],
        assignment: &Assignment,
        protected: &[ProtectedView],
    ) {
        if assignment.role != Role::Escort {
            self.escort_outside_leash = false;
            return;
        }
        let distance = protected
            .iter()
            .filter(|view| view.alive && assignment.protected_ids.contains(&view.id))
            .map(|view| horizontal_distance(own_position, view.position))
            .min_by(f64::total_cmp);
        let Some(distance) = distance else {
            self.escort_outside_leash = true;
            return;
        };
        if self.escort_outside_leash {
            if distance <= ESCORT_REENTRY_LEASH_FT {
                self.escort_outside_leash = false;
            }
        } else if distance > ESCORT_EXIT_LEASH_FT {
            self.escort_outside_leash = true;
        }
    }
}

/// Resolve a supporting-radar bearing only from current observations. The
/// missile owner and any unobserved world actors are intentionally absent.
pub fn identify_supporting_attacker(
    own_position: [f64; 3],
    own_heading_deg: f64,
    radar_bearing_deg: f64,
    observed: &[TargetView],
    emitter_ids: &[u32],
    own_side: targeting::Side,
) -> Option<u32> {
    if !own_position.iter().all(|value| value.is_finite())
        || !own_heading_deg.is_finite()
        || !radar_bearing_deg.is_finite()
    {
        return None;
    }
    let mut matches = observed
        .iter()
        .filter(|target| {
            target.valid
                && target.is_aircraft
                && target.side != own_side
                && emitter_ids.contains(&target.id)
        })
        .filter(|target| {
            let dx = target.position[0] - own_position[0];
            let dz = target.position[2] - own_position[2];
            let relative_bearing = (dx.atan2(dz).to_degrees() - own_heading_deg).rem_euclid(360.);
            angular_difference_deg(relative_bearing, radar_bearing_deg)
                <= SUPPORTING_RADAR_IDENTIFICATION_TOLERANCE_DEG
        })
        .map(|target| target.id);
    let identified = matches.next()?;
    matches.next().is_none().then_some(identified)
}

fn known_attackers(reports: &[ThreatReport], defended_id: u32) -> Vec<u32> {
    reports
        .iter()
        .filter(|report| report.defended_id == defended_id)
        .filter_map(|report| report.attacker_id)
        .collect()
}

fn eligible(own_id: u32, own_side: targeting::Side, target: &TargetView) -> bool {
    target.id != own_id
        && target.side != own_side
        && target.valid
        && target.type_allowed
        && target.seeker_eligible
}

fn spatial_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f64>()
        .sqrt()
}

fn horizontal_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

fn angular_difference_deg(a: f64, b: f64) -> f64 {
    ((a - b + 180.).rem_euclid(360.) - 180.).abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ScalarSpeed;

    const OWN: u32 = 1;
    const FRIEND: targeting::Side = targeting::Side(1);

    fn target(id: u32, distance: f64) -> TargetView {
        TargetView {
            id,
            side: targeting::Side(2),
            position: [distance, 0., 0.],
            heading_deg: 0.,
            pitch_deg: 0.,
            speed: ScalarSpeed(300.),
            maximum_speed: ScalarSpeed(600.),
            is_aircraft: true,
            is_fighter: true,
            human_controlled: false,
            valid: true,
            type_allowed: true,
            seeker_eligible: true,
            wing_attackers: 0,
            terrain_blocked: false,
            sensor_supported: true,
        }
    }

    fn assignment(role: Role, stance: Stance) -> Assignment {
        Assignment {
            role,
            stance,
            protected_ids: vec![],
            destroy_ids: vec![],
            hostile_escorts: vec![],
            patrol: None,
        }
    }

    fn select(
        policy: &mut Policy,
        assignment: &Assignment,
        targets: &[TargetView],
        protected: &[ProtectedView],
        reports: &[ThreatReport],
        current: Option<u32>,
    ) -> Option<Selection> {
        policy.select(
            OWN, FRIEND, [0.; 3], assignment, targets, protected, reports, current, 1,
        )
    }

    #[test]
    fn protected_threat_outranks_nearer_unrelated_contact_and_escort() {
        let mut a = assignment(Role::Escort, Stance::ProtectAssigned);
        a.protected_ids.push(10);
        a.hostile_escorts.push(HostileEscort {
            principal_id: 3,
            escort_id: 4,
        });
        let reports = [ThreatReport {
            attacker_id: Some(3),
            defended_id: 10,
        }];
        let protected = [ProtectedView {
            id: 10,
            position: [0.; 3],
            alive: true,
        }];
        let selected = select(
            &mut Policy::default(),
            &a,
            &[target(2, 100.), target(4, 200.), target(3, 5_000.)],
            &protected,
            &reports,
            None,
        );
        assert_eq!(
            selected,
            Some(Selection {
                id: 3,
                priority: Priority::ProtectedThreat
            })
        );
    }

    #[test]
    fn own_defense_preempts_protection_and_current_retention() {
        let mut a = assignment(Role::Escort, Stance::ProtectAssigned);
        a.protected_ids.push(10);
        let reports = [
            ThreatReport {
                attacker_id: Some(2),
                defended_id: 10,
            },
            ThreatReport {
                attacker_id: Some(3),
                defended_id: OWN,
            },
        ];
        let protected = [ProtectedView {
            id: 10,
            position: [0.; 3],
            alive: true,
        }];
        assert_eq!(
            select(
                &mut Policy::default(),
                &a,
                &[target(2, 100.), target(3, 5_000.)],
                &protected,
                &reports,
                Some(2),
            ),
            Some(Selection {
                id: 3,
                priority: Priority::OwnDefense
            })
        );
    }

    #[test]
    fn hold_unknown_attackers_and_same_side_never_authorize_fire() {
        let hold = assignment(Role::FreeEngagement, Stance::WeaponsHold);
        assert_eq!(
            select(
                &mut Policy::default(),
                &hold,
                &[target(2, 100.)],
                &[],
                &[ThreatReport {
                    attacker_id: Some(2),
                    defended_id: OWN
                }],
                None,
            ),
            None
        );
        let self_defense = assignment(Role::Disengage, Stance::SelfDefense);
        let mut friendly = target(2, 100.);
        friendly.side = FRIEND;
        assert_eq!(
            select(
                &mut Policy::default(),
                &self_defense,
                &[friendly, target(3, 200.)],
                &[],
                &[ThreatReport {
                    attacker_id: None,
                    defended_id: OWN
                }],
                None,
            ),
            None
        );
    }

    #[test]
    fn patrol_requires_a_region_and_includes_its_exact_boundary() {
        let mut cap = assignment(Role::CombatAirPatrol, Stance::EngageAssigned);
        assert_eq!(
            select(
                &mut Policy::default(),
                &cap,
                &[target(2, 100.)],
                &[],
                &[],
                None
            ),
            None
        );
        cap.patrol = Some(PatrolRegion {
            center_ft: [0.; 3],
            radius_ft: 100.,
        });
        assert_eq!(
            select(
                &mut Policy::default(),
                &cap,
                &[target(2, 100.), target(3, 101.)],
                &[],
                &[],
                None,
            )
            .map(|selection| selection.id),
            Some(2)
        );
    }

    #[test]
    fn escort_leash_has_exact_hysteresis_and_reset() {
        let mut a = assignment(Role::Escort, Stance::EngageAssigned);
        a.protected_ids.push(10);
        a.destroy_ids.push(2);
        let target = [target(2, 100.)];
        let protected_at = |distance| {
            [ProtectedView {
                id: 10,
                position: [distance, 0., 0.],
                alive: true,
            }]
        };
        let mut policy = Policy::default();
        assert!(
            select(
                &mut policy,
                &a,
                &target,
                &protected_at(ESCORT_EXIT_LEASH_FT),
                &[],
                None
            )
            .is_some()
        );
        assert_eq!(
            select(
                &mut policy,
                &a,
                &target,
                &protected_at(ESCORT_EXIT_LEASH_FT + 1.),
                &[],
                None
            ),
            None
        );
        assert!(policy.escort_outside_leash());
        assert_eq!(
            select(
                &mut policy,
                &a,
                &target,
                &protected_at(ESCORT_REENTRY_LEASH_FT + 1.),
                &[],
                None
            ),
            None
        );
        assert!(
            select(
                &mut policy,
                &a,
                &target,
                &protected_at(ESCORT_REENTRY_LEASH_FT),
                &[],
                None
            )
            .is_some()
        );
        policy.reset();
        assert!(!policy.escort_outside_leash());
    }

    #[test]
    fn current_target_is_kept_only_inside_the_best_priority() {
        let mut free = assignment(Role::FreeEngagement, Stance::EngageAssigned);
        let targets = [target(8, 100.), target(2, 1_000.)];
        assert_eq!(
            select(&mut Policy::default(), &free, &targets, &[], &[], Some(2))
                .map(|selection| selection.id),
            Some(2)
        );
        free.destroy_ids.push(8);
        let reports = [ThreatReport {
            attacker_id: Some(8),
            defended_id: OWN,
        }];
        assert_eq!(
            select(
                &mut Policy::default(),
                &free,
                &targets,
                &[],
                &reports,
                Some(2),
            ),
            Some(Selection {
                id: 8,
                priority: Priority::OwnDefense
            })
        );
    }

    #[test]
    fn invalid_destroy_objective_and_unarmed_neutral_are_not_forced_targets() {
        let mut intercept = assignment(Role::Intercept, Stance::EngageAssigned);
        intercept.destroy_ids.extend([2, 3]);
        let mut destroyed = target(2, 100.);
        destroyed.valid = false;
        let mut neutral = target(3, 200.);
        neutral.seeker_eligible = false;
        assert_eq!(
            select(
                &mut Policy::default(),
                &intercept,
                &[destroyed, neutral],
                &[],
                &[],
                None,
            ),
            None
        );
    }

    #[test]
    fn default_preserves_free_engagement_and_b41_ranking_with_stable_ties() {
        let default = Assignment::default();
        let mut occupied = target(9, 100.);
        occupied.wing_attackers = 1;
        assert_eq!(
            select(
                &mut Policy::default(),
                &default,
                &[occupied, target(4, 1_000.)],
                &[],
                &[],
                None,
            )
            .map(|selection| selection.id),
            Some(4)
        );
        assert_eq!(
            select(
                &mut Policy::default(),
                &default,
                &[target(9, 1_000.), target(4, 1_000.)],
                &[],
                &[],
                None,
            )
            .map(|selection| selection.id),
            Some(4)
        );
    }

    #[test]
    fn remembered_permission_is_search_only_and_obeys_stance_reports_and_patrol() {
        let remembered = target(2, 100.);
        let policy = Policy::default();

        let hold = assignment(Role::FreeEngagement, Stance::WeaponsHold);
        assert!(!policy.allows_investigation(&hold, &remembered, &[], OWN));

        let self_defense = assignment(Role::Disengage, Stance::SelfDefense);
        assert!(!policy.allows_investigation(&self_defense, &remembered, &[], OWN));
        assert!(policy.allows_investigation(
            &self_defense,
            &remembered,
            &[ThreatReport {
                attacker_id: Some(2),
                defended_id: OWN,
            }],
            OWN,
        ));

        let mut protect = assignment(Role::Escort, Stance::ProtectAssigned);
        protect.protected_ids.push(10);
        assert!(!policy.allows_investigation(&protect, &remembered, &[], OWN));
        assert!(policy.allows_investigation(
            &protect,
            &remembered,
            &[ThreatReport {
                attacker_id: Some(2),
                defended_id: 10,
            }],
            OWN,
        ));

        let mut cap = assignment(Role::CombatAirPatrol, Stance::EngageAssigned);
        cap.patrol = Some(PatrolRegion {
            center_ft: [0.; 3],
            radius_ft: 100.,
        });
        assert!(policy.allows_investigation(&cap, &remembered, &[], OWN));
        let outside = target(3, 101.);
        assert!(!policy.allows_investigation(&cap, &outside, &[], OWN));

        // Investigation permission cannot create a current combat target.
        assert!(policy.allows_investigation(&Assignment::default(), &remembered, &[], OWN));
        assert_eq!(
            select(
                &mut Policy::default(),
                &Assignment::default(),
                &[],
                &[],
                &[],
                None,
            ),
            None
        );
    }

    #[test]
    fn remembered_escort_investigation_stops_outside_the_leash() {
        let mut a = assignment(Role::Escort, Stance::EngageAssigned);
        a.protected_ids.push(10);
        a.destroy_ids.push(2);
        let protected = [ProtectedView {
            id: 10,
            position: [ESCORT_EXIT_LEASH_FT + 1., 0., 0.],
            alive: true,
        }];
        let mut policy = Policy::default();
        assert_eq!(select(&mut policy, &a, &[], &protected, &[], None), None);
        assert!(!policy.allows_investigation(&a, &target(2, 100.), &[], OWN));
    }

    #[test]
    fn supporting_attacker_requires_one_current_hostile_emitter_on_bearing() {
        let hostile = target(2, 1_000.);
        assert_eq!(
            identify_supporting_attacker([0.; 3], 90., 0., &[hostile], &[2], FRIEND),
            Some(2)
        );
        assert_eq!(
            identify_supporting_attacker([0.; 3], 90., 0., &[], &[2], FRIEND),
            None
        );
        assert_eq!(
            identify_supporting_attacker([0.; 3], 90., 0., &[hostile], &[], FRIEND),
            None
        );

        let mut friendly = hostile;
        friendly.side = FRIEND;
        assert_eq!(
            identify_supporting_attacker([0.; 3], 90., 0., &[friendly], &[2], FRIEND),
            None
        );

        let mut second = target(3, 1_000.);
        second.position[2] = 10.;
        assert_eq!(
            identify_supporting_attacker([0.; 3], 90., 0., &[hostile, second], &[2, 3], FRIEND,),
            None
        );
    }
}
