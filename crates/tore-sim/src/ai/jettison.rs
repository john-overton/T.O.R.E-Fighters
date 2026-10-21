//! Deterministic, opinionated load shedding. See docs/spec/jettison.md.
use super::Experience;

/// What one AI pilot sheds this tick. Guns and air-to-air missiles are never
/// part of any drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drop {
    /// Every external fuel tank, empty or not.
    Tanks,
    /// The external air-to-ground load (bombs, rocket pods, air-to-ground
    /// missiles and selectable designator pods); tanks stay aboard.
    Ordnance,
    /// The tanks and the external air-to-ground load.
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
    /// The pilot is flying home because of a component failure
    /// ([`super::damage::Reason`]).
    pub damage_return: bool,
    /// Bingo fuel or worse: the pilot is flying home on fuel.
    pub fuel_home: bool,
    /// The airframe's damage fraction, 0 (intact) to 1.
    pub damage: f64,
    /// No external fuel remains: any tank still carried is only drag.
    pub tanks_dry: bool,
}

const NM_FT: f64 = 6076.11549;
/// Fitted, agent-chosen (2026-10-20): a defender this badly hit is losing.
pub const LOSING_DAMAGE: f64 = 0.25;

pub fn decide(s: Situation) -> Option<Drop> {
    let near = |range: Option<f64>, nm: f64| {
        range.is_some_and(|r| r.is_finite() && r >= 0. && r <= nm * NM_FT)
    };
    let attacked = near(
        s.attacker_ft,
        if s.strike {
            [4., 3., 2., 1.][s.skill.index()]
        } else {
            3.
        },
    );
    let defending = s.missile_defense || s.defensive_break;
    // Entering a fight: tanks go, empty or not, as a pilot drops them before
    // a merge. The weapons stay for the fight.
    let tanks = defending || attacked || (!s.strike && near(s.engagement_ft, 3.));
    // Shedding the air-to-ground load needs a reason to save the aircraft:
    // it is going home damaged or short of fuel and can no longer deliver
    // the load, or it is losing a close defensive fight, already badly hit.
    let shed =
        s.damage_return || s.fuel_home || (defending && attacked && s.damage >= LOSING_DAMAGE);
    match (tanks || (shed && s.tanks_dry), shed) {
        (true, true) => Some(Drop::CombatLoad),
        (false, true) => Some(Drop::Ordnance),
        (true, false) => Some(Drop::Tanks),
        (false, false) => None,
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
            damage_return: false,
            fuel_home: false,
            damage: 0.,
            tanks_dry: false,
        }
    }
    #[test]
    fn cruise_and_distant_engagement_keep_stores() {
        assert_eq!(decide(cruise()), None);
        assert_eq!(
            decide(Situation {
                engagement_ft: Some(4. * NM_FT),
                ..cruise()
            }),
            None
        );
    }
    #[test]
    fn missile_and_break_drop_tanks() {
        for s in [
            Situation {
                missile_defense: true,
                ..cruise()
            },
            Situation {
                defensive_break: true,
                ..cruise()
            },
        ] {
            assert_eq!(decide(s), Some(Drop::Tanks));
            assert_eq!(
                decide(Situation { strike: true, ..s }),
                Some(Drop::Tanks),
                "a defending striker keeps its bombs"
            );
        }
    }
    #[test]
    fn a_merge_drops_only_the_tanks() {
        let merge = Situation {
            engagement_ft: Some(6000.),
            ..cruise()
        };
        assert_eq!(decide(merge), Some(Drop::Tanks));
        // In the turning fight itself: defending, attacked, close.
        assert_eq!(
            decide(Situation {
                defensive_break: true,
                missile_defense: true,
                attacker_ft: Some(3000.),
                ..merge
            }),
            Some(Drop::Tanks)
        );
        // A strike pilot does not chase an aircraft it merely sees.
        assert_eq!(
            decide(Situation {
                strike: true,
                ..merge
            }),
            None
        );
    }
    #[test]
    fn strike_attack_drops_tanks_and_skill_sets_panic_distance() {
        for skill in Experience::ALL {
            let limit = [4., 3., 2., 1.][skill.index()] * NM_FT;
            let s = Situation {
                strike: true,
                skill,
                attacker_ft: Some(limit),
                ..cruise()
            };
            assert_eq!(decide(s), Some(Drop::Tanks));
            assert_eq!(
                decide(Situation {
                    attacker_ft: Some(limit + 1.),
                    ..s
                }),
                None
            );
            // Losing: defending against that attacker and already hit.
            let losing = Situation {
                defensive_break: true,
                damage: LOSING_DAMAGE,
                ..s
            };
            assert_eq!(decide(losing), Some(Drop::CombatLoad));
            assert_eq!(
                decide(Situation {
                    attacker_ft: Some(limit + 1.),
                    ..losing
                }),
                Some(Drop::Tanks),
                "a distant attacker is not a lost fight"
            );
            assert_eq!(
                decide(Situation {
                    damage: LOSING_DAMAGE - 0.01,
                    ..losing
                }),
                Some(Drop::Tanks),
                "a lightly hit defender keeps its load"
            );
        }
    }
    #[test]
    fn damage_or_fuel_home_sheds_the_air_to_ground_load() {
        for s in [
            Situation {
                damage_return: true,
                ..cruise()
            },
            Situation {
                fuel_home: true,
                ..cruise()
            },
        ] {
            // Tanks with fuel in them help get home; dry ones are only drag.
            assert_eq!(decide(s), Some(Drop::Ordnance));
            assert_eq!(
                decide(Situation {
                    tanks_dry: true,
                    ..s
                }),
                Some(Drop::CombatLoad)
            );
            assert_eq!(
                decide(Situation { strike: true, ..s }),
                Some(Drop::Ordnance)
            );
            assert_eq!(
                decide(Situation {
                    missile_defense: true,
                    ..s
                }),
                Some(Drop::CombatLoad)
            );
        }
        // Damage alone, without a return or a lost fight, keeps the load.
        assert_eq!(
            decide(Situation {
                damage: 0.9,
                ..cruise()
            }),
            None
        );
    }
}
