//! Experience resolution and the per-level tables from
//! [`docs/spec/ai-experience.md`](../../../../docs/spec/ai-experience.md).
//!
//! This file owns numbers only. It resolves an assignment request to a level
//! (with its origin kept alongside) and exposes the experience-indexed
//! thresholds as typed constants. It performs no tactical decisions; the
//! tactics component draws against these thresholds at its own decision points.

use super::{DecisionRandom, Experience, Result};

/// Where a resolved level came from ("Experience channels"). Explicit
/// per-object mission values must stay distinguishable from generation
/// settings, so the origin travels with the level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExperienceOrigin {
    /// A per-object level saved in the mission; used as is.
    ExplicitPerObject,
    /// The mission editor's bulk assignment, jittered from `selected`.
    EditorAssignment { selected: Experience },
    /// Quick Mission generation: every member of a wing carries the wing's
    /// menu selection. Executable-confirmed on 2026-09-17 (generator, loader
    /// and post-load paths); the manual's "range of skills" is not implemented.
    QuickMission { selected: Experience },
    /// The flight-menu enemy-skill preference replaced the level.
    EnemyOverride,
}

/// A request to resolve one aircraft's level. Side/domain assignment channels
/// are the caller's concern; each channel resolves its own objects through
/// this same rule set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExperienceRequest {
    /// Explicit per-object mission value, level 0..3.
    Explicit { level: i32 },
    /// Editor bulk assignment from a selected level 0..3.
    Editor { selected: i32 },
    /// Quick Mission wing selection, level 0..3; applied uniformly to the wing.
    QuickMission { selected: i32 },
}

/// A resolved level and its origin. Resolution happens once per object; the
/// host must not call [`resolve_experience`] again during ordinary updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedExperience {
    pub level: Experience,
    pub origin: ExperienceOrigin,
}

/// Editor bulk assignment draw thresholds on a 0..=99 draw: draws below
/// [`EDITOR_DOWN_BELOW`] move one level down (33 of 100), draws from there
/// below [`EDITOR_UP_FROM`] leave the level unchanged (35 of 100), and the
/// rest move one level up (32 of 100).
pub const EDITOR_DOWN_BELOW: u8 = 33;
pub const EDITOR_UP_FROM: u8 = 68;

/// Resolve an experience request ("Experience channels").
///
/// - An explicit per-object level is used as is; nothing is drawn.
/// - Editor bulk assignment draws once and shifts the selected level by at
///   most one step, clamped to Novice..Ace.
/// - Quick Mission applies the wing's menu selection to every member, with
///   no draw; the generator, loader and post-load paths were traced and none
///   varies members within a wing.
///
/// Levels outside 0..3 are [`super::AiError::InvalidInput`].
pub fn resolve_experience(
    request: ExperienceRequest,
    random: &mut DecisionRandom,
) -> Result<ResolvedExperience> {
    match request {
        ExperienceRequest::Explicit { level } => Ok(ResolvedExperience {
            level: Experience::from_level(level)?,
            origin: ExperienceOrigin::ExplicitPerObject,
        }),
        ExperienceRequest::Editor { selected } => {
            let selected = Experience::from_level(selected)?;
            let draw = random.percent();
            let shift = if draw < EDITOR_DOWN_BELOW {
                -1
            } else if draw < EDITOR_UP_FROM {
                0
            } else {
                1
            };
            let level = (i32::from(selected.level()) + shift).clamp(0, 3);
            Ok(ResolvedExperience {
                level: Experience::from_level(level)?,
                origin: ExperienceOrigin::EditorAssignment { selected },
            })
        }
        ExperienceRequest::QuickMission { selected } => {
            let selected = Experience::from_level(selected)?;
            Ok(ResolvedExperience {
                level: selected,
                origin: ExperienceOrigin::QuickMission { selected },
            })
        }
    }
}

/// The flight-menu preference that forces every enemy aircraft to one level
/// at mission start (AI experience spec, "Experience channels"). It is applied
/// after objects exist, so it overrides mission files and Quick Mission
/// settings. `None` leaves the resolved level alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemySkillOverride {
    AllNovice,
    AllAverage,
}

/// Apply the enemy-skill override to an enemy aircraft's resolved level.
/// Friendly aircraft are never affected; the caller decides side.
pub fn apply_enemy_override(
    resolved: ResolvedExperience,
    is_enemy_aircraft: bool,
    setting: Option<EnemySkillOverride>,
) -> ResolvedExperience {
    match (is_enemy_aircraft, setting) {
        (true, Some(EnemySkillOverride::AllNovice)) => ResolvedExperience {
            level: Experience::Novice,
            origin: ExperienceOrigin::EnemyOverride,
        },
        (true, Some(EnemySkillOverride::AllAverage)) => ResolvedExperience {
            level: Experience::Average,
            origin: ExperienceOrigin::EnemyOverride,
        },
        _ => resolved,
    }
}

/// One row of "Fighter tactical choices", indexed by [`Experience`].
pub type PerLevel<T> = [T; 4];

/// The four target situations that select a tactical threshold pair. Ahead
/// and facing are the B01/B02 predicates from the geometry component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetSituation {
    AheadFacing,
    AheadFacingAway,
    BehindFacing,
    BehindFacingAway,
}

impl TargetSituation {
    pub fn from_geometry(ahead: bool, facing: bool) -> Self {
        match (ahead, facing) {
            (true, true) => Self::AheadFacing,
            (true, false) => Self::AheadFacingAway,
            (false, true) => Self::BehindFacing,
            (false, false) => Self::BehindFacingAway,
        }
    }
}

/// Percent thresholds for one situation: the best-attack preference, then the
/// random-tactic choice reached only after best attack fails. Both are
/// conditional choices at that decision point, not per-tick probabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TacticalThresholds {
    pub best_attack_percent: u8,
    pub random_tactic_percent: u8,
}

const fn thresholds(best: u8, random: u8) -> TacticalThresholds {
    TacticalThresholds {
        best_attack_percent: best,
        random_tactic_percent: random,
    }
}

/// "Prefer best attack, target ahead and facing" / "Otherwise choose random".
pub const AHEAD_FACING: PerLevel<TacticalThresholds> = [
    thresholds(11, 50),
    thresholds(20, 34),
    thresholds(74, 14),
    thresholds(84, 6),
];
/// Target ahead and facing away.
pub const AHEAD_FACING_AWAY: PerLevel<TacticalThresholds> = [
    thresholds(8, 72),
    thresholds(20, 40),
    thresholds(72, 12),
    thresholds(84, 6),
];
/// Target behind and facing.
pub const BEHIND_FACING: PerLevel<TacticalThresholds> = [
    thresholds(16, 42),
    thresholds(20, 20),
    thresholds(72, 10),
    thresholds(90, 4),
];
/// Target behind and facing away.
pub const BEHIND_FACING_AWAY: PerLevel<TacticalThresholds> = [
    thresholds(16, 42),
    thresholds(20, 20),
    thresholds(72, 14),
    thresholds(84, 6),
];
/// "Fly straight on entering the random-tactic menu", percent.
pub const STRAIGHT_ON_RANDOM_MENU_PERCENT: PerLevel<u8> = [52, 24, 0, 0];
/// "Pursuit-point vertical displacement when chased", percent.
pub const PURSUIT_VERTICAL_DISPLACEMENT_PERCENT: PerLevel<u8> = [35, 50, 75, 95];
/// Exclusive upper bound of each initial pursuit target-offset component draw
/// (0 through 99, 49, 29 or 9). Pursuit-point variation only, not gun
/// dispersion.
pub const PURSUIT_OFFSET_DRAW_BOUND: PerLevel<u32> = [100, 50, 30, 10];
/// "Other experience effects": launch-reaction device schedule gate, percent.
/// Event eligibility, chaff/flare selector mapping and inventory effects are
/// unknown; this is not a probability of evading a missile.
pub const DEVICE_RELEASE_REACTION_PERCENT: PerLevel<u8> = [35, 50, 75, 90];

/// Threshold pair for a situation at a level.
pub fn tactical_thresholds(level: Experience, situation: TargetSituation) -> TacticalThresholds {
    let table = match situation {
        TargetSituation::AheadFacing => AHEAD_FACING,
        TargetSituation::AheadFacingAway => AHEAD_FACING_AWAY,
        TargetSituation::BehindFacing => BEHIND_FACING,
        TargetSituation::BehindFacingAway => BEHIND_FACING_AWAY,
    };
    table[level.index()]
}

pub fn straight_on_random_menu_percent(level: Experience) -> u8 {
    STRAIGHT_ON_RANDOM_MENU_PERCENT[level.index()]
}

pub fn pursuit_vertical_displacement_percent(level: Experience) -> u8 {
    PURSUIT_VERTICAL_DISPLACEMENT_PERCENT[level.index()]
}

/// Exclusive bound for [`DecisionRandom::below`] on each offset component.
pub fn pursuit_offset_draw_bound(level: Experience) -> u32 {
    PURSUIT_OFFSET_DRAW_BOUND[level.index()]
}

pub fn device_release_reaction_percent(level: Experience) -> u8 {
    DEVICE_RELEASE_REACTION_PERCENT[level.index()]
}

/// Floor of the adjusted positive G limit, in G.
pub const ADJUSTED_POSITIVE_G_FLOOR: f64 = 2.0;
/// Ceiling of the adjusted negative G limit, in G (a negative number).
pub const ADJUSTED_NEGATIVE_G_CEILING: f64 = -2.0;

/// "Other experience effects": available-G adjustment for AI aircraft.
///
/// Novice and Average lose 1 G of positive limit (floor 2 G) and 1 G of
/// negative limit (ceiling -2 G). Experienced and Ace are unchanged. `exempt`
/// skips the adjustment entirely; the exemption's human-control meaning is
/// unknown, so this must not be applied to player flight limits.
pub fn adjusted_g_limits(
    level: Experience,
    positive_g: f64,
    negative_g: f64,
    exempt: bool,
) -> (f64, f64) {
    if exempt || level >= Experience::Experienced {
        return (positive_g, negative_g);
    }
    (
        (positive_g - 1.0).max(ADJUSTED_POSITIVE_G_FLOOR),
        (negative_g + 1.0).min(ADJUSTED_NEGATIVE_G_CEILING),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const DRAWS: usize = 100_000;
    /// Allowed deviation from the nominal share, in percent of draws.
    const TOLERANCE: f64 = 1.0;

    fn editor_distribution(selected: Experience) -> [f64; 4] {
        let mut random = DecisionRandom::seeded(0x5EED + u64::from(selected.level()));
        let mut counts = [0usize; 4];
        for _ in 0..DRAWS {
            let resolved = resolve_experience(
                ExperienceRequest::Editor {
                    selected: i32::from(selected.level()),
                },
                &mut random,
            )
            .unwrap();
            assert_eq!(
                resolved.origin,
                ExperienceOrigin::EditorAssignment { selected }
            );
            counts[resolved.level.index()] += 1;
        }
        counts.map(|n| n as f64 * 100.0 / DRAWS as f64)
    }

    fn assert_close(actual: [f64; 4], nominal: [f64; 4]) {
        for (a, n) in actual.iter().zip(nominal) {
            assert!((a - n).abs() <= TOLERANCE, "{actual:?} vs {nominal:?}");
        }
    }

    #[test]
    fn editor_distribution_matches_nominal_table() {
        assert_close(
            editor_distribution(Experience::Novice),
            [68.0, 32.0, 0.0, 0.0],
        );
        assert_close(
            editor_distribution(Experience::Average),
            [33.0, 35.0, 32.0, 0.0],
        );
        assert_close(
            editor_distribution(Experience::Experienced),
            [0.0, 33.0, 35.0, 32.0],
        );
        assert_close(editor_distribution(Experience::Ace), [0.0, 0.0, 33.0, 67.0]);
    }

    #[test]
    fn explicit_levels_never_reroll() {
        let mut random = DecisionRandom::seeded(1);
        for level in Experience::ALL {
            for _ in 0..50 {
                let resolved = resolve_experience(
                    ExperienceRequest::Explicit {
                        level: i32::from(level.level()),
                    },
                    &mut random,
                )
                .unwrap();
                assert_eq!(resolved.level, level);
                assert_eq!(resolved.origin, ExperienceOrigin::ExplicitPerObject);
            }
        }
        assert_eq!(random, DecisionRandom::seeded(1), "explicit levels drew");
    }

    #[test]
    fn quick_mission_is_uniform_per_wing() {
        let mut random = DecisionRandom::seeded(2);
        for level in Experience::ALL {
            for _ in 0..50 {
                let resolved = resolve_experience(
                    ExperienceRequest::QuickMission {
                        selected: i32::from(level.level()),
                    },
                    &mut random,
                )
                .unwrap();
                assert_eq!(resolved.level, level);
                assert_eq!(
                    resolved.origin,
                    ExperienceOrigin::QuickMission { selected: level }
                );
            }
        }
        assert_eq!(random, DecisionRandom::seeded(2), "quick mission drew");
    }

    #[test]
    fn enemy_override_forces_enemy_aircraft_only() {
        let ace = ResolvedExperience {
            level: Experience::Ace,
            origin: ExperienceOrigin::ExplicitPerObject,
        };
        assert_eq!(apply_enemy_override(ace, true, None), ace);
        assert_eq!(
            apply_enemy_override(ace, false, Some(EnemySkillOverride::AllNovice)),
            ace
        );
        let forced = apply_enemy_override(ace, true, Some(EnemySkillOverride::AllNovice));
        assert_eq!(forced.level, Experience::Novice);
        assert_eq!(forced.origin, ExperienceOrigin::EnemyOverride);
        let forced = apply_enemy_override(ace, true, Some(EnemySkillOverride::AllAverage));
        assert_eq!(forced.level, Experience::Average);
    }

    #[test]
    fn invalid_levels_are_rejected() {
        let mut random = DecisionRandom::seeded(3);
        for request in [
            ExperienceRequest::Explicit { level: 4 },
            ExperienceRequest::Explicit { level: -1 },
            ExperienceRequest::Editor { selected: 4 },
            ExperienceRequest::QuickMission { selected: 7 },
        ] {
            assert!(matches!(
                resolve_experience(request, &mut random),
                Err(crate::ai::AiError::InvalidInput(_))
            ));
        }
    }

    #[test]
    fn tactical_tables_match_spec() {
        let t = tactical_thresholds(Experience::Novice, TargetSituation::AheadFacing);
        assert_eq!((t.best_attack_percent, t.random_tactic_percent), (11, 50));
        let t = tactical_thresholds(Experience::Ace, TargetSituation::BehindFacing);
        assert_eq!((t.best_attack_percent, t.random_tactic_percent), (90, 4));
        let t = tactical_thresholds(Experience::Experienced, TargetSituation::AheadFacingAway);
        assert_eq!((t.best_attack_percent, t.random_tactic_percent), (72, 12));
        let t = tactical_thresholds(Experience::Average, TargetSituation::BehindFacingAway);
        assert_eq!((t.best_attack_percent, t.random_tactic_percent), (20, 20));
        assert_eq!(
            TargetSituation::from_geometry(false, true),
            TargetSituation::BehindFacing
        );
        assert_eq!(straight_on_random_menu_percent(Experience::Novice), 52);
        assert_eq!(straight_on_random_menu_percent(Experience::Ace), 0);
        assert_eq!(
            pursuit_vertical_displacement_percent(Experience::Average),
            50
        );
        assert_eq!(pursuit_offset_draw_bound(Experience::Novice), 100);
        assert_eq!(pursuit_offset_draw_bound(Experience::Ace), 10);
        assert_eq!(device_release_reaction_percent(Experience::Experienced), 75);
    }

    #[test]
    fn g_limits_adjust_low_levels_with_floor_and_ceiling() {
        assert_eq!(
            adjusted_g_limits(Experience::Novice, 9.0, -3.0, false),
            (8.0, -2.0)
        );
        assert_eq!(
            adjusted_g_limits(Experience::Average, 7.5, -4.0, false),
            (6.5, -3.0)
        );
        assert_eq!(
            adjusted_g_limits(Experience::Novice, 2.5, -2.5, false),
            (2.0, -2.0)
        );
        assert_eq!(
            adjusted_g_limits(Experience::Average, 2.0, -2.0, false),
            (2.0, -2.0)
        );
    }

    #[test]
    fn g_limits_skip_high_levels_and_exempt_aircraft() {
        assert_eq!(
            adjusted_g_limits(Experience::Experienced, 9.0, -3.0, false),
            (9.0, -3.0)
        );
        assert_eq!(
            adjusted_g_limits(Experience::Ace, 9.0, -3.0, false),
            (9.0, -3.0)
        );
        assert_eq!(
            adjusted_g_limits(Experience::Novice, 9.0, -3.0, true),
            (9.0, -3.0)
        );
    }
}
