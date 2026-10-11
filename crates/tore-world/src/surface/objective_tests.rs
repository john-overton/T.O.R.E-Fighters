//! Objectives, scoring and the debrief against the synthetic template of
//! `tests.rs` (docs/spec/surface-defenses.md, "Objectives, scoring and
//! debrief"). The template has three ground targets (a `<sam>` slot, a
//! bunker and a parked F/A-18, ordinals 0, 12 and 16), a Redfor supply truck
//! (14) and a friendly ZSU-23 (15); the base layout has an enemy SA-6, a
//! friendly ZSU-23 and an enemy supply truck. No retail data.
use super::tests::{surface_resources, target};
use super::*;
use crate::ai_wings::outcome::{self, Lineages, Requirements, Stance, Standing};
use crate::debrief::{self, Objective, Outcome, Status};
use crate::seats::SeatId;
use crate::world::World;
use tore_formats::surface_unit::class;
use tore_sim::ai::launch::Side as Wing;
use tore_sim::combat::ledger::{Kill, Resolution, ShotKind};

const TEMPLATE_SAM: u32 = SURFACE_UNIT_BASE;
const TEMPLATE_BUNKER: u32 = SURFACE_UNIT_BASE + 12;
const TEMPLATE_TRUCK: u32 = SURFACE_UNIT_BASE + 14;
const TEMPLATE_FRIENDLY_ZSU: u32 = SURFACE_UNIT_BASE + 15;
const PARKED_TARGET: u32 = SURFACE_UNIT_BASE + 16;
const TARGETS: [u32; 3] = [TEMPLATE_SAM, TEMPLATE_BUNKER, PARKED_TARGET];
/// The base layout's enemy SA-6 and friendly ZSU-23.
const LAYOUT_SA6: u32 = LAYOUT_OBJECT_BASE;
const LAYOUT_FRIENDLY_ZSU: u32 = LAYOUT_OBJECT_BASE + 1;
/// The player's plane.
const PLAYER: u32 = 0;

/// The template mission with two enemy F/A-18s, so there is an air
/// requirement for the ground one to join.
fn world() -> World {
    use crate::mission::{Defense, MissionSpec, Skill, WingSpec};
    use crate::test_support::resources::THEATER;
    use crate::world::Seating;
    use tore_formats::aircraft::AircraftId;
    let ground = target("QUCITY", 3, 3, 9);
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.ground_target = Some(ground.stem.clone());
    spec.aaa = Defense::from_level(ground.aaa).unwrap();
    spec.sam = Defense::from_level(ground.sam).unwrap();
    spec.surface_seed = ground.seed;
    spec.enemy_nationality = ground.enemy_nationality as u8;
    spec.wings[3] = WingSpec {
        aircraft: AircraftId::F18,
        count: 2,
        skill: Skill::Average,
    };
    let mut world = World::new(&spec, &surface_resources(), Seating::SinglePlayer).unwrap();
    // The fixture's PT has no class word; a retail fighter's is 0x8000.
    let parked = PARKED_TARGET;
    for row in &mut world.combat.state.targets {
        if row.id == parked {
            row.category = 0x8000;
        }
    }
    world
}

/// The plane's air requirements, then the ground targets joined.
fn requirements(world: &World, side: Wing) -> Requirements {
    let wings = world.ai_wings.as_ref().unwrap();
    Requirements::of(wings, PLAYER, side, &Lineages::default()).with_ground(side, &TARGETS)
}

fn row_category(world: &World, id: u32) -> u16 {
    world
        .combat
        .state
        .targets
        .iter()
        .find(|t| t.id == id)
        .unwrap()
        .category
}

/// `shooter` destroys surface object `id`: its row loses its hit points and
/// the ledger credits the kill, as combat does.
fn destroy(world: &mut World, id: u32, shooter: u32) {
    let category = row_category(world, id);
    let state = &mut world.combat.state;
    state.targets.iter_mut().find(|t| t.id == id).unwrap().hp = 0;
    state.ledger.kill(Kill {
        owner: shooter,
        victim: id,
        category,
        aircraft: false,
    });
}

/// Every enemy aircraft of the mission is down, so only the ground decides.
fn down_the_enemy_air_force(world: &mut World) {
    let wings = world.ai_wings.as_mut().unwrap();
    let enemies: Vec<u32> = wings
        .slots()
        .iter()
        .filter(|slot| slot.side == Wing::Enemy)
        .map(|slot| slot.id)
        .collect();
    assert!(!enemies.is_empty());
    for id in enemies {
        wings
            .mission_mut()
            .actor_mut(id)
            .unwrap()
            .flight_mut()
            .crashed = true;
    }
}

fn succeeded(world: &World, side: Wing) -> bool {
    outcome::succeeded(
        &world.combat.state,
        world.ai_wings.as_ref(),
        PLAYER,
        true,
        side,
        &[],
        &Lineages::default(),
        Some(&world.terrain.surface),
    )
}

fn captured(world: &World) -> debrief::Report {
    debrief::capture(world, SeatId(0)).unwrap()
}

#[test]
fn the_ground_targets_join_the_air_requirement_in_one_destroy_objective() {
    let world = world();
    let air = Requirements::of(
        world.ai_wings.as_ref().unwrap(),
        PLAYER,
        Wing::Friendly,
        &Lineages::default(),
    );
    assert!(!air.destroy.is_empty(), "the mission has enemy aircraft");
    assert_eq!(
        outcome::ground_targets(&world.terrain.surface, &world.combat.state),
        TARGETS,
        "the 0x80 slot, bunker and parked fighter"
    );
    let all = requirements(&world, Wing::Friendly);
    assert_eq!(all.destroy.len(), air.destroy.len() + 3);
    assert!(TARGETS.iter().all(|id| all.destroy.contains(id)));
    assert_eq!(all.protect, air.protect);
    // Naming them again adds nothing.
    assert_eq!(all.clone().with_ground(Wing::Friendly, &TARGETS), all);
    // One combined Destroy line, retail's "Destroyed 0 of N targets".
    let report = captured(&world);
    assert_eq!(
        report.objectives,
        [Objective::Destroy {
            destroyed: 0,
            total: air.destroy.len() as u32 + 3,
        }]
    );
    assert_eq!(report.outcome, Outcome::Failure);
}

#[test]
fn a_redfor_flight_protects_the_targets_instead() {
    let all = requirements(&world(), Wing::Enemy);
    assert!(TARGETS.iter().all(|id| all.protect.contains(id)));
    assert!(TARGETS.iter().all(|id| !all.destroy.contains(id)));
}

#[test]
fn a_surface_target_is_destroyed_once_its_row_is_dead_and_never_by_being_unknown() {
    let mut world = world();
    let kills_all_air = |world: &World| -> Vec<u32> {
        let mut ids = requirements(world, Wing::Friendly).destroy;
        ids.retain(|id| !TARGETS.contains(id));
        ids
    };
    let air = kills_all_air(&world).len() as u32;
    let count = |world: &World| {
        let fates = outcome::ground(&world.terrain.surface, &world.combat.state, Wing::Friendly);
        let requirements = requirements(world, Wing::Friendly);
        Standing {
            ledger: &world.combat.state.ledger,
            plane: PLAYER,
            aircraft: &[],
            ground: &fates,
            requirements: &requirements,
        }
        .destroyed()
            - air
    };
    assert_eq!(count(&world), 0);
    destroy(&mut world, TEMPLATE_BUNKER, PLAYER);
    assert_eq!(count(&world), 1);
    destroy(&mut world, PARKED_TARGET, PLAYER);
    destroy(&mut world, TEMPLATE_SAM, PLAYER);
    assert_eq!(count(&world), 3);
    // A surface id the standing does not list is undecided, not destroyed
    // (it used to count as destroyed at once); an unlisted aircraft is gone.
    let ledger = &world.combat.state.ledger;
    let requirements = Requirements {
        destroy: vec![TEMPLATE_SAM, 77],
        protect: vec![TEMPLATE_BUNKER, 78],
    };
    let unknown = Standing {
        ledger,
        plane: PLAYER,
        aircraft: &[],
        ground: &[],
        requirements: &requirements,
    };
    assert_eq!(
        unknown.destroyed(),
        1,
        "aircraft 77 is gone, the SAM is not"
    );
    assert_eq!(
        unknown.protected(),
        1,
        "aircraft 78 is lost, the bunker is not"
    );
}

#[test]
fn killing_every_target_succeeds_and_a_friendly_ground_kill_does_not() {
    let mut world = world();
    down_the_enemy_air_force(&mut world);
    assert!(!succeeded(&world, Wing::Friendly));
    for id in TARGETS {
        destroy(&mut world, id, PLAYER);
    }
    assert!(succeeded(&world, Wing::Friendly));
    let report = captured(&world);
    assert_eq!(report.outcome, Outcome::Success);
    // Kills by class: the SAM, the bunker (Structure) and the parked fighter.
    assert_eq!(report.player.kills[4], 1, "SAM row");
    assert_eq!(report.player.kills[8], 1, "Structure row");
    assert_eq!(report.player.kills[0], 1, "parked fighter, Fighter row");
    assert_eq!(report.player.friendly_fire, 0);
    // The multiplayer results read the same kills: a parked fighter is in the
    // first three rows, the SAM and the bunker in the rest.
    let rows = debrief::results(&world);
    let player = rows.iter().find(|row| row.plane.0 == PLAYER).unwrap();
    assert_eq!((player.aircraft_kills, player.other_kills), (1, 2));
    assert_eq!(player.friendly_fire, 0);
    // The enemy supply truck and base SAM are ordinary kills.
    destroy(&mut world, TEMPLATE_TRUCK, PLAYER);
    destroy(&mut world, LAYOUT_SA6, PLAYER);
    assert!(succeeded(&world, Wing::Friendly));
    // A friendly unit that is not a target fails the mission: the template's
    // ZSU-23 (nationality3) and the base layout's.
    for friend in [TEMPLATE_FRIENDLY_ZSU, LAYOUT_FRIENDLY_ZSU] {
        let mut world = world_after_targets(&mut |w| destroy(w, friend, PLAYER));
        assert!(!succeeded(&world, Wing::Friendly), "{friend:#x}");
        let report = captured(&world);
        assert_eq!(report.outcome, Outcome::Failure);
        assert_eq!(report.player.friendly_fire, 1);
        // The same kill by someone else (another plane) is not the player's.
        world.combat.state.ledger = Default::default();
        destroy(&mut world, friend, 1);
        assert!(succeeded(&world, Wing::Friendly));
    }
}

/// A world with the enemy air force and every target down, then `more`.
fn world_after_targets(more: &mut dyn FnMut(&mut World)) -> World {
    let mut world = world();
    down_the_enemy_air_force(&mut world);
    for id in TARGETS {
        destroy(&mut world, id, PLAYER);
    }
    more(&mut world);
    world
}

/// Adds a Blue unit the way the layout rules add a battery radar or a supply
/// truck: origin Added, flags 0, a side, a combat row.
fn add_friendly(world: &mut World, id: UnitId) {
    let mut unit = world
        .terrain
        .surface
        .unit(UnitId(LAYOUT_FRIENDLY_ZSU))
        .unwrap()
        .clone();
    unit.id = id;
    unit.origin = Origin::Added;
    unit.flags = 0;
    unit.side = FRIENDLY_SIDE;
    let bounds = world
        .combat
        .state
        .ground_bounds(LAYOUT_FRIENDLY_ZSU)
        .unwrap();
    world.terrain.surface.units.push(unit);
    world.terrain.surface.units.sort_by_key(|unit| unit.id);
    world
        .terrain
        .surface
        .object_sides
        .insert(id.0, FRIENDLY_SIDE);
    world
        .combat
        .state
        .add_ground_target(id.0, bounds, 50, class::VEHICLE, FRIENDLY_SIDE)
        .unwrap();
}

#[test]
fn a_friendly_added_radar_or_truck_is_friendly_fire_but_never_a_target() {
    for id in [
        UnitId::battery_radar(100).unwrap(),
        UnitId::supply_truck(100).unwrap(),
    ] {
        let mut world = world_after_targets(&mut |w| add_friendly(w, id));
        assert!(!world.terrain.surface.unit(id).unwrap().is_target());
        assert_eq!(
            outcome::ground_targets(&world.terrain.surface, &world.combat.state),
            TARGETS,
            "added units are never targets"
        );
        assert!(succeeded(&world, Wing::Friendly));
        destroy(&mut world, id.0, PLAYER);
        assert!(!succeeded(&world, Wing::Friendly), "{:#x}", id.0);
        assert_eq!(captured(&world).player.friendly_fire, 1);
        // Their own side's: the same kill by a Redfor plane is an ordinary
        // kill in the Vehicle row.
        let fates = outcome::ground(&world.terrain.surface, &world.combat.state, Wing::Enemy);
        assert_eq!(
            fates.iter().find(|g| g.id == id.0).unwrap().stance,
            Stance::Hostile
        );
    }
}

#[test]
fn a_redfor_flight_fails_when_a_protected_target_falls_and_its_own_units_are_friends() {
    let mut world = world();
    let result = |world: &World| {
        let fates = outcome::ground(&world.terrain.surface, &world.combat.state, Wing::Enemy);
        let requirements = Requirements::default().with_ground(Wing::Enemy, &TARGETS);
        let standing = Standing {
            ledger: &world.combat.state.ledger,
            plane: 9,
            aircraft: &[],
            ground: &fates,
            requirements: &requirements,
        };
        (
            standing.protected(),
            standing.destroyed(),
            standing.succeeded(),
        )
    };
    assert_eq!(result(&world), (3, 0, true));
    // The template belongs to Redfor, the base layout's friendly ZSU to Blue.
    let stance = |world: &World, id: u32, side| {
        outcome::ground(&world.terrain.surface, &world.combat.state, side)
            .into_iter()
            .find(|g| g.id == id)
            .unwrap()
            .stance
    };
    assert_eq!(
        stance(&world, TEMPLATE_TRUCK, Wing::Enemy),
        Stance::Friendly
    );
    assert_eq!(
        stance(&world, TEMPLATE_TRUCK, Wing::Friendly),
        Stance::Hostile
    );
    assert_eq!(
        stance(&world, LAYOUT_FRIENDLY_ZSU, Wing::Enemy),
        Stance::Hostile
    );
    // Scenery that is not a unit has no stance: it is never friendly.
    let fates = outcome::ground(&world.terrain.surface, &world.combat.state, Wing::Enemy);
    assert!(fates.iter().all(|g| g.id != LAYOUT_OBJECT_BASE + 4));
    assert!(fates.iter().all(|g| g.id != LAYOUT_OBJECT_BASE + 2));
    // Blue destroys one target: the protect objective is lost.
    destroy(&mut world, TEMPLATE_BUNKER, 1);
    assert_eq!(result(&world), (2, 0, false));
    // A Redfor plane that shoots its own supply truck (not a target) is
    // guilty of friendly fire; its own targets are not "friendly fire".
    let ledger_of = |world: &World| world.combat.state.ledger.clone();
    let mut world = self::world();
    destroy(&mut world, TEMPLATE_TRUCK, 9);
    let fates = outcome::ground(&world.terrain.surface, &world.combat.state, Wing::Enemy);
    let requirements = Requirements::default().with_ground(Wing::Enemy, &TARGETS);
    let ledger = ledger_of(&world);
    let standing = Standing {
        ledger: &ledger,
        plane: 9,
        aircraft: &[],
        ground: &fates,
        requirements: &requirements,
    };
    assert!(standing.friendly_fire());
    assert!(
        !standing.friendly(TEMPLATE_BUNKER),
        "an objective, not a friend"
    );
}

/// Fire at the player from a SAM and a gun: the debrief's enemy SAM and AAA
/// rows, not the AAM and Gun rows of enemy aircraft; a friendly unit's fire
/// is no enemy fire.
#[test]
fn hostile_surface_fire_fills_the_enemy_sam_and_aaa_rows() {
    let mut world = world();
    let wings = world.ai_wings.as_ref().unwrap();
    let enemy_plane = wings
        .slots()
        .iter()
        .find(|slot| slot.side == Wing::Enemy)
        .unwrap()
        .id;
    let template_sam = TEMPLATE_SAM + 1; // a manned `<sam>` slot is a launcher
    let template_aaa = SURFACE_UNIT_BASE + 6;
    let ledger = &mut world.combat.state.ledger;
    let mut shoot = |projectile: u32, owner: u32, kind: ShotKind, damage: Option<u32>| {
        ledger.launch(projectile, owner, Some(PLAYER), kind);
        ledger.resolve(
            projectile,
            damage.map_or(Resolution::Missed, Resolution::Hit),
        );
    };
    // Two SAMs (a template launcher and the base SA-6; one a hit), three gun
    // rounds from a template AAA gun (two hits).
    shoot(1, template_sam, ShotKind::AirToAir, Some(50));
    shoot(2, LAYOUT_SA6, ShotKind::AirToAir, None);
    for (projectile, damage) in [(3, Some(5)), (4, None), (5, Some(5))] {
        shoot(projectile, template_aaa, ShotKind::Gun, damage);
    }
    // An enemy aircraft's missile and gun round, and the friendly ZSU's fire.
    shoot(6, enemy_plane, ShotKind::AirToAir, Some(30));
    shoot(7, enemy_plane, ShotKind::Gun, Some(2));
    shoot(8, LAYOUT_FRIENDLY_ZSU, ShotKind::Gun, Some(2));
    let player = captured(&world).player;
    let tally = |t: tore_sim::combat::ledger::Tally| (t.launched, t.hit, t.damage);
    assert_eq!(tally(player.enemy_sam), (2, 1, 50));
    assert_eq!(tally(player.enemy_aaa), (3, 2, 10));
    assert_eq!(tally(player.enemy_aam), (1, 1, 30));
    assert_eq!(tally(player.enemy_gun), (1, 1, 2));
    let summary = captured(&world).summary();
    assert!(summary.contains("enemy_sam=1/2 enemy_aaa=2/3"), "{summary}");
}

#[test]
fn a_surface_units_kill_of_the_player_is_named() {
    let mut world = world();
    let name = world
        .terrain
        .surface
        .unit(UnitId(LAYOUT_SA6))
        .unwrap()
        .name
        .clone();
    assert!(!name.is_empty());
    // The SA-6 shot the player down: the ledger credits the kill.
    world.combat.state.ledger.kill(Kill {
        owner: LAYOUT_SA6,
        victim: PLAYER,
        category: 0x8000,
        aircraft: true,
    });
    world.combat.state.ownship_mut(PLAYER).unwrap().hp = 0;
    let player = captured(&world).player;
    assert_eq!(player.status, Status::Dead);
    assert_eq!(player.shot_down_by.as_deref(), Some(name.as_str()));
    assert!(
        captured(&world)
            .summary()
            .contains(&format!("shot_down_by={name}"))
    );
    // Nobody is named for a plane that is still flying.
    let alive = self::world();
    assert_eq!(captured(&alive).player.shot_down_by, None);
}

/// How many "Mission accomplished" calls a flown world announces in 12
/// seconds, with the enemy air force down from the start and, if asked, the
/// ground targets too.
fn announcements(targets_down: bool) -> usize {
    use crate::seats::SeatInput;
    use crate::world::{Cue, TickOutput};
    let mut world = world();
    world.setup.mission = Some((5000., 3000.));
    // The checks begin with the targets standing (a result already decided at
    // the start disables the call).
    for cockpit in &mut world.cockpits {
        cockpit.result = crate::ai_wings::outcome::Tracker::default();
        cockpit.result.step(0., Some(|| false), [0.; 3], true);
    }
    down_the_enemy_air_force(&mut world);
    if targets_down {
        for id in TARGETS {
            destroy(&mut world, id, PLAYER);
        }
    }
    let mut out = TickOutput::default();
    let mut calls = 0;
    for _ in 0..1_500 {
        let input = SeatInput {
            seat: SeatId(0),
            tick: world.tick(),
            ..SeatInput::default()
        };
        world.step(&[input], &mut out).unwrap();
        calls += out
            .cues
            .iter()
            .filter(|cue| {
                matches!(cue, Cue::Radio { call, .. }
                    if call.stems.first().is_some_and(|stem| stem == "^MISSACC"))
            })
            .count();
    }
    calls
}

#[test]
fn the_core_announces_success_only_when_the_ground_targets_have_fallen_too() {
    assert_eq!(
        announcements(false),
        0,
        "the enemy air force alone is not enough"
    );
    assert_eq!(announcements(true), 1);
}

#[test]
fn the_combined_line_counts_air_and_ground_together() {
    let mut world = world();
    let before = captured(&world).objectives;
    let [
        Objective::Destroy {
            destroyed: 0,
            total,
        },
    ] = before[..]
    else {
        panic!("one Destroy line: {before:?}");
    };
    assert_eq!(total, 5, "two enemy aircraft and the three targets");
    destroy(&mut world, TEMPLATE_BUNKER, PLAYER);
    destroy(&mut world, PARKED_TARGET, PLAYER);
    let after = captured(&world).objectives;
    assert_eq!(
        after,
        [Objective::Destroy {
            destroyed: 2,
            total: 5
        }]
    );
    assert_eq!(after[0].sentence(), "Destroyed 2 of 5 targets.");
}

#[test]
fn radars_and_trucks_count_in_their_retail_class() {
    let mut world = world();
    let radar = BATTERY_RADAR_BASE; // an added Straight Flush: a Vehicle
    let truck = SUPPLY_TRUCK_BASE; // an added supply truck: a Vehicle
    let launcher = LAYOUT_SA6; // a battery launcher: SAM
    for id in [radar, truck, launcher, SURFACE_UNIT_BASE + 13] {
        destroy(&mut world, id, PLAYER);
    }
    let kills = captured(&world).player.kills;
    assert_eq!(kills[4], 1, "SAM row: the launcher");
    assert_eq!(kills[7], 2, "Vehicle row: the radar and the truck");
    assert_eq!(kills[6], 1, "Tank row: the T-72");
    assert_eq!(captured(&world).player.friendly_fire, 0);
}

#[test]
fn a_unit_that_shot_the_player_down_is_named_for_the_message() {
    let mut world = world();
    let ledger = &mut world.combat.state.ledger;
    assert_eq!(
        outcome::shot_down_by(&world.terrain.surface, ledger, PLAYER),
        None
    );
    ledger.damaged(Kill {
        owner: SURFACE_UNIT_BASE + 6,
        victim: PLAYER,
        category: 0x8000,
        aircraft: true,
    });
    let zsu = world
        .terrain
        .surface
        .unit(UnitId(SURFACE_UNIT_BASE + 6))
        .unwrap();
    assert_eq!(
        outcome::shot_down_by(&world.terrain.surface, &world.combat.state.ledger, PLAYER),
        Some(zsu.name.as_str())
    );
    // An aircraft's kill names no unit.
    let mut plain = self::world();
    plain.combat.state.ledger.kill(Kill {
        owner: 1,
        victim: PLAYER,
        category: 0x8000,
        aircraft: true,
    });
    assert_eq!(
        outcome::shot_down_by(&plain.terrain.surface, &plain.combat.state.ledger, PLAYER),
        None
    );
}

#[test]
fn the_target_window_says_obj_for_ground_targets() {
    use crate::target_window::TargetObjective;
    let mut world = world();
    let objective = |world: &World, id| world.combat.ground_objective(PLAYER, id);
    for id in TARGETS {
        assert_eq!(
            objective(&world, id),
            Some(TargetObjective::Destroy),
            "{id:#x}"
        );
    }
    // Other units, the base layout and aircraft are no objective.
    for id in [TEMPLATE_TRUCK, TEMPLATE_FRIENDLY_ZSU, LAYOUT_SA6, 1, 2] {
        assert_eq!(objective(&world, id), None, "{id:#x}");
    }
    // A Redfor plane keeps them.
    world.combat.state.ownship_mut(PLAYER).unwrap().side = ENEMY_SIDE;
    assert_eq!(
        objective(&world, TEMPLATE_BUNKER),
        Some(TargetObjective::Survive)
    );
    // A restart registers the same set.
    world.combat.reset(&mut world.cockpits[0].flight).unwrap();
    assert_eq!(
        objective(&world, PARKED_TARGET),
        Some(TargetObjective::Survive)
    );
}
