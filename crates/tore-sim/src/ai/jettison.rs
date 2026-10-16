//! Deterministic, opinionated load shedding. See docs/spec/jettison.md.
use super::Experience;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drop {
    Tanks,
    CombatLoad,
}

/// Distances are perceived aircraft contacts, never hidden world targets.
#[derive(Clone, Copy, Debug)]
pub struct Situation {
    pub skill: Experience,
    pub strike: bool,
    pub missile_defense: bool,
    pub defensive_break: bool,
    pub engagement_ft: Option<f64>,
    pub attacker_ft: Option<f64>,
}

pub fn decide(s: Situation) -> Option<Drop> {
    let near = |range: Option<f64>, nm: f64| {
        range.is_some_and(|r| r.is_finite() && r >= 0. && r <= nm * 6076.11549)
    };
    let attacked = near(
        s.attacker_ft,
        if s.strike {
            [4., 3., 2., 1.][s.skill.index()]
        } else {
            3.
        },
    );
    if attacked
        || (!s.strike && near(s.engagement_ft, 3.))
        || (s.strike && (s.missile_defense || s.defensive_break))
    {
        Some(Drop::CombatLoad)
    } else if s.missile_defense || s.defensive_break {
        Some(Drop::Tanks)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cruise() -> Situation {
        Situation {
            skill: Experience::Average,
            strike: false,
            missile_defense: false,
            defensive_break: false,
            engagement_ft: None,
            attacker_ft: None,
        }
    }
    #[test]
    fn cruise_and_distant_engagement_keep_stores() {
        assert_eq!(decide(cruise()), None);
        assert_eq!(
            decide(Situation {
                engagement_ft: Some(4. * 6076.11549),
                ..cruise()
            }),
            None
        );
    }
    #[test]
    fn missile_and_break_drop_tanks() {
        assert_eq!(
            decide(Situation {
                missile_defense: true,
                ..cruise()
            }),
            Some(Drop::Tanks)
        );
        assert_eq!(
            decide(Situation {
                defensive_break: true,
                ..cruise()
            }),
            Some(Drop::Tanks)
        );
    }
    #[test]
    fn merge_drops_combat_load_but_strike_does_not_chase() {
        assert_eq!(
            decide(Situation {
                engagement_ft: Some(6000.),
                ..cruise()
            }),
            Some(Drop::CombatLoad)
        );
        assert_eq!(
            decide(Situation {
                strike: true,
                engagement_ft: Some(6000.),
                ..cruise()
            }),
            None
        );
    }
    #[test]
    fn strike_defends_and_skill_sets_panic_distance() {
        for skill in Experience::ALL {
            let limit = [4., 3., 2., 1.][skill.index()] * 6076.11549;
            let s = Situation {
                strike: true,
                skill,
                attacker_ft: Some(limit),
                ..cruise()
            };
            assert_eq!(decide(s), Some(Drop::CombatLoad));
            assert_eq!(
                decide(Situation {
                    attacker_ft: Some(limit + 1.),
                    ..s
                }),
                None
            );
            assert_eq!(
                decide(Situation {
                    missile_defense: true,
                    attacker_ft: None,
                    ..s
                }),
                Some(Drop::CombatLoad)
            );
        }
    }
}
