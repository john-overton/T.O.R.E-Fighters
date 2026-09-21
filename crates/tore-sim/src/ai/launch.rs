//! The launch payload that turns a Quick Mission setup screen into per-member
//! AI identities.
//!
//! Spec: [`docs/spec/ai-experience.md`](../../../../docs/spec/ai-experience.md),
//! section "Experience channels", and
//! [`docs/spec/ai.md`](../../../../docs/spec/ai.md), section "Existing code to
//! connect later" (the `quick_mission::QuickMission::dummy_wings` row, which
//! requires all six wings, side and member identity, and the experience origin
//! to survive the hand-off).
//!
//! Two spec rules drive everything here:
//!
//! - In a Quick Mission every aircraft in a wing carries the experience level
//!   chosen for that wing. The level is resolved once per wing and copied to
//!   every member; there is no per-member draw and no jitter. The manual's
//!   "range of skills within a wing" is not what the original does.
//! - The flight-menu enemy-skill preference is applied after the objects exist,
//!   so it overrides the wing selection for enemy aircraft only, and the
//!   resolved origin becomes [`ExperienceOrigin::EnemyOverride`] when it fires.
//!
//! This module is renderer independent and host independent. It takes the raw
//! menu selections, validates them, and returns a payload; it places nothing in
//! the world, and it knows nothing about formations, spacing or loadouts.

use super::experience::{
    EnemySkillOverride, ExperienceRequest, ResolvedExperience, apply_enemy_override,
    resolve_experience,
};
use super::{AiError, DecisionRandom, Experience, Result};
use tore_formats::aircraft::AircraftId;

/// Wings per side on the Quick Mission setup screen (wing 1 through wing 3).
pub const WINGS_PER_SIDE: u8 = 3;
/// Largest wing size the setup screen offers, from the counts field range
/// 0 through 5 (`docs/formats/quick-mission.md`). Friendly wing 1 reaches at
/// most four AI members because the player occupies one slot, which is the
/// host's subtraction, not this module's.
pub const MAX_WING_MEMBERS: usize = 5;
/// User-requested training mode, not a fifth experience-table entry.
pub const DUMMY_SKILL: i32 = 4;
pub const DUMMY_SPEED_FPS: f64 = 400. * crate::sensors::FEET_PER_NAUTICAL_MILE / 3600.;

/// Seed for the local draw state required by [`resolve_experience`].
///
/// The Quick Mission channel draws nothing: `resolve_experience` returns the
/// selected level unchanged for [`ExperienceRequest::QuickMission`]. The
/// function still takes a generator because other channels need one, so this
/// module owns a fixed seed rather than forcing every host to invent one. The
/// value cannot reach any output, which `seeds_cannot_change_the_payload`
/// proves.
const QUICK_MISSION_SEED: u64 = 0x0000_0000_0000_0001;

/// Which side of the Quick Mission setup screen a wing belongs to.
///
/// This is the assignment channel from the AI experience spec: friendly-air and
/// enemy-air are separate channels, and the enemy-skill override keys off this
/// and nothing else. It is deliberately not nationality: two wings can fly the
/// same nation's aircraft on opposite sides, and nationality never decides
/// whether the override applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Friendly,
    Enemy,
}

impl Side {
    /// True for the enemy channel. This is the flag
    /// [`apply_enemy_override`] expects.
    pub fn is_enemy(self) -> bool {
        matches!(self, Self::Enemy)
    }
}

/// One wing's identity: its side, and its zero-based position 0..=2, which is
/// wing 1 through wing 3 on the setup screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WingId {
    pub side: Side,
    pub index: u8,
}

impl WingId {
    /// A wing on the setup screen. `index` is zero based; 0..=2 only.
    pub fn new(side: Side, index: u8) -> Result<Self> {
        if index >= WINGS_PER_SIDE {
            return Err(AiError::InvalidInput("wing index outside 0..3"));
        }
        Ok(Self { side, index })
    }

    /// The wing number a player sees on the setup screen, 1 through 3.
    pub fn display_number(self) -> u8 {
        self.index + 1
    }
}

/// One aircraft in a wing, with the level it will fly at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemberLaunch {
    /// Zero-based position within the wing; member 0 is the leader.
    pub member: u8,
    /// The resolved level and where it came from. Every member of a wing
    /// carries the same value, unless the enemy override replaced it, in which
    /// case every member of that wing carries the same replaced value.
    pub experience: ResolvedExperience,
    /// True for member 0 only.
    pub is_leader: bool,
}

/// One resolved wing: who it is, what it flies, and every member it launches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WingLaunch {
    pub wing: WingId,
    pub aircraft: AircraftId,
    /// The level chosen for this wing on the setup screen, kept even when the
    /// enemy override has replaced the members' levels, so the origin of the
    /// setting stays visible to the host. Dummy uses an inactive Novice placeholder.
    pub selected_level: Experience,
    /// Constant-heading 400-knot training target, independent of experience.
    pub dummy: bool,
    pub members: Vec<MemberLaunch>,
}

impl WingLaunch {
    /// Number of aircraft this wing launches.
    pub fn count(&self) -> usize {
        self.members.len()
    }

    /// True when the wing was selected with zero aircraft. Such a wing keeps
    /// its identity and its selected level, and launches nothing.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// The wing leader, absent for an empty wing.
    pub fn leader(&self) -> Option<&MemberLaunch> {
        self.members.first()
    }
}

/// One raw row of the setup screen, as a host reads it out of the menu.
///
/// `skill_level` is the menu position 0 Novice through 3 Ace, or 4 Dummy; it is validated,
/// never clamped. `count` is the number of aircraft this wing launches, after
/// the host has removed any slot the player occupies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WingSelection {
    pub wing: WingId,
    pub aircraft: AircraftId,
    pub count: usize,
    pub skill_level: i32,
}

/// Resolve every wing of a Quick Mission setup into per-member launch data.
///
/// For each wing the experience is resolved once, through
/// [`ExperienceRequest::QuickMission`], and given to every member unchanged.
/// The enemy-skill override is then applied per member with the wing's side,
/// so an enemy wing's members end up at the forced level with origin
/// [`ExperienceOrigin::EnemyOverride`], and friendly wings are untouched.
///
/// Wings are returned in the order given, empty wings included, so a host can
/// keep its own wing numbering. A skill selection outside 0..=4 is
/// [`AiError::InvalidInput`], as is a count above [`MAX_WING_MEMBERS`].
///
/// [`ExperienceOrigin::EnemyOverride`]: super::experience::ExperienceOrigin::EnemyOverride
pub fn resolve_wings(
    selections: &[WingSelection],
    enemy_override: Option<EnemySkillOverride>,
) -> Result<Vec<WingLaunch>> {
    // Required by resolve_experience; the Quick Mission channel never draws.
    let mut random = DecisionRandom::seeded(QUICK_MISSION_SEED);
    let mut wings = Vec::with_capacity(selections.len());
    for selection in selections {
        if selection.wing.index >= WINGS_PER_SIDE {
            return Err(AiError::InvalidInput("wing index outside 0..3"));
        }
        if selection.count > MAX_WING_MEMBERS {
            return Err(AiError::InvalidInput("wing size above the menu maximum"));
        }
        let resolved = resolve_experience(
            ExperienceRequest::QuickMission {
                selected: if selection.skill_level == DUMMY_SKILL {
                    0
                } else {
                    selection.skill_level
                },
            },
            &mut random,
        )?;
        let is_enemy = selection.wing.side.is_enemy();
        let dummy = selection.skill_level == DUMMY_SKILL;
        let experience = if dummy {
            resolved
        } else {
            apply_enemy_override(resolved, is_enemy, enemy_override)
        };
        let members = (0..selection.count)
            .map(|member| MemberLaunch {
                member: member as u8,
                experience,
                is_leader: member == 0,
            })
            .collect();
        wings.push(WingLaunch {
            wing: selection.wing,
            aircraft: selection.aircraft,
            selected_level: resolved.level,
            dummy,
            members,
        });
    }
    Ok(wings)
}

/// Flatten a payload to the aircraft/count pairs the current mission spawner
/// still takes. Side, member identity and experience are lost here, which is
/// exactly the loss the hook-up removes; keep the payload itself for anything
/// that needs those.
pub fn legacy_pairs(wings: &[WingLaunch]) -> Vec<(AircraftId, usize)> {
    wings.iter().map(|w| (w.aircraft, w.count())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::experience::ExperienceOrigin;

    fn wing(side: Side, index: u8) -> WingId {
        WingId::new(side, index).unwrap()
    }

    fn selection(side: Side, index: u8, count: usize, skill_level: i32) -> WingSelection {
        WingSelection {
            wing: wing(side, index),
            aircraft: AircraftId::F18,
            count,
            skill_level,
        }
    }

    #[test]
    fn dummy_is_a_mode_not_an_experience_table_index() {
        let wings = resolve_wings(
            &[selection(Side::Enemy, 0, 2, DUMMY_SKILL)],
            Some(EnemySkillOverride::AllAverage),
        )
        .unwrap();
        assert!(wings[0].dummy);
        assert_eq!(wings[0].selected_level, Experience::Novice);
        assert!(
            wings[0]
                .members
                .iter()
                .all(|m| m.experience.level == Experience::Novice)
        );
        assert!(!resolve_wings(&[selection(Side::Enemy, 0, 2, 3)], None).unwrap()[0].dummy);
    }

    #[test]
    fn every_member_of_a_wing_carries_the_wings_level() {
        let wings = resolve_wings(&[selection(Side::Friendly, 0, 4, 3)], None).unwrap();
        assert_eq!(wings.len(), 1);
        assert_eq!(wings[0].count(), 4);
        assert_eq!(wings[0].selected_level, Experience::Ace);
        for member in &wings[0].members {
            assert_eq!(member.experience.level, Experience::Ace);
            assert_eq!(
                member.experience.origin,
                ExperienceOrigin::QuickMission {
                    selected: Experience::Ace
                }
            );
        }
    }

    #[test]
    fn all_four_levels_round_trip() {
        for level in Experience::ALL {
            let selected = i32::from(level.level());
            let wings = resolve_wings(&[selection(Side::Enemy, 2, 5, selected)], None).unwrap();
            assert_eq!(wings[0].selected_level, level);
            assert!(wings[0].members.iter().all(|m| m.experience.level == level
                && m.experience.origin == ExperienceOrigin::QuickMission { selected: level }));
        }
    }

    #[test]
    fn the_enemy_override_touches_enemy_wings_only() {
        let selections = [
            selection(Side::Friendly, 0, 3, 3),
            selection(Side::Enemy, 0, 3, 3),
        ];
        for (setting, forced) in [
            (EnemySkillOverride::AllNovice, Experience::Novice),
            (EnemySkillOverride::AllAverage, Experience::Average),
        ] {
            let wings = resolve_wings(&selections, Some(setting)).unwrap();
            for member in &wings[0].members {
                assert_eq!(member.experience.level, Experience::Ace);
                assert_eq!(
                    member.experience.origin,
                    ExperienceOrigin::QuickMission {
                        selected: Experience::Ace
                    }
                );
            }
            for member in &wings[1].members {
                assert_eq!(member.experience.level, forced);
                assert_eq!(member.experience.origin, ExperienceOrigin::EnemyOverride);
            }
            // The menu selection survives the override on both sides.
            assert_eq!(wings[1].selected_level, Experience::Ace);
        }
    }

    #[test]
    fn invalid_levels_and_sizes_are_errors_not_clamps() {
        for level in [-1, 5, 99] {
            assert_eq!(
                resolve_wings(&[selection(Side::Friendly, 0, 1, level)], None),
                Err(AiError::InvalidInput("experience level outside 0..3"))
            );
        }
        assert_eq!(
            resolve_wings(&[selection(Side::Friendly, 0, 6, 0)], None),
            Err(AiError::InvalidInput("wing size above the menu maximum"))
        );
        assert_eq!(
            WingId::new(Side::Enemy, 3),
            Err(AiError::InvalidInput("wing index outside 0..3"))
        );
        assert!(WingId::new(Side::Enemy, 2).is_ok());
    }

    #[test]
    fn seeds_cannot_change_the_payload() {
        // The Quick Mission channel performs no draw, so a payload resolved
        // now must equal one resolved from any other generator state.
        let selections = [
            selection(Side::Friendly, 0, 4, 1),
            selection(Side::Enemy, 1, 5, 2),
        ];
        let expected = resolve_wings(&selections, None).unwrap();
        for seed in [0u64, 1, 7, 0xDEAD_BEEF, u64::MAX] {
            let mut random = DecisionRandom::seeded(seed);
            let mut wings = Vec::new();
            for s in &selections {
                let resolved = resolve_experience(
                    ExperienceRequest::QuickMission {
                        selected: s.skill_level,
                    },
                    &mut random,
                )
                .unwrap();
                wings.push(resolved);
            }
            for (wing, resolved) in expected.iter().zip(wings) {
                assert_eq!(wing.members[0].experience, resolved);
            }
        }
        assert_eq!(resolve_wings(&selections, None).unwrap(), expected);
    }

    #[test]
    fn members_are_numbered_from_the_leader() {
        let wings = resolve_wings(&[selection(Side::Friendly, 1, 5, 0)], None).unwrap();
        let members = &wings[0].members;
        assert_eq!(
            members.iter().map(|m| m.member).collect::<Vec<_>>(),
            [0, 1, 2, 3, 4]
        );
        assert!(members[0].is_leader);
        assert!(members[1..].iter().all(|m| !m.is_leader));
        assert_eq!(wings[0].leader().map(|m| m.member), Some(0));
        assert_eq!(wings[0].wing.display_number(), 2);
    }

    #[test]
    fn empty_wings_keep_their_identity_and_launch_nothing() {
        let wings = resolve_wings(&[selection(Side::Enemy, 2, 0, 2)], None).unwrap();
        assert_eq!(wings.len(), 1);
        assert!(wings[0].is_empty());
        assert_eq!(wings[0].count(), 0);
        assert!(wings[0].leader().is_none());
        assert_eq!(wings[0].selected_level, Experience::Experienced);
        assert_eq!(wings[0].wing.side, Side::Enemy);
    }

    #[test]
    fn sides_and_wing_order_survive_the_payload() {
        let selections = [
            selection(Side::Friendly, 0, 4, 0),
            selection(Side::Friendly, 1, 1, 1),
            selection(Side::Friendly, 2, 2, 2),
            selection(Side::Enemy, 0, 3, 3),
            selection(Side::Enemy, 1, 5, 0),
            selection(Side::Enemy, 2, 5, 1),
        ];
        let wings = resolve_wings(&selections, None).unwrap();
        assert_eq!(
            wings.iter().map(|w| w.wing).collect::<Vec<_>>(),
            selections.iter().map(|s| s.wing).collect::<Vec<_>>()
        );
        assert!(!wings[0].wing.side.is_enemy());
        assert!(wings[3].wing.side.is_enemy());
        assert_eq!(
            legacy_pairs(&wings),
            vec![
                (AircraftId::F18, 4),
                (AircraftId::F18, 1),
                (AircraftId::F18, 2),
                (AircraftId::F18, 3),
                (AircraftId::F18, 5),
                (AircraftId::F18, 5),
            ]
        );
    }
}
