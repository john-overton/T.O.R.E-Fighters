//! Objectives follow lineages (the lobby pass's follow-up F1; docs/spec/debrief.md,
//! "Objectives in a game with respawns"), on an open mission built from
//! synthetic resources as a host builds it: every plane carries its wing's
//! objectives for whoever flies it; a respawned or revived plane of an
//! objective lineage is an objective wherever the original is (the target
//! window, the AI's own lists, the debrief); a destroy objective counts a
//! lineage once, the first time any plane of it is lost; a protect objective
//! fails on the first loss; a Redfor player reads its own side's objectives;
//! and a checkpoint keeps the joined lists.

use super::*;
use crate::{
    ai_wings::outcome::{Requirements, Standing},
    debrief::{self, Objective},
    mission::{MissionSpec, Start},
    seats::{PlaneId, SeatId, SeatInput},
    target_window::{TargetBrief, TargetObjective},
    test_support::resources::{THEATER, resources},
    world::{MissionCommand, Seating, TickOutput},
};
use tore_formats::aircraft::AircraftId;
use tore_sim::ai::{engagement::GroupObjective, launch::WingId};

/// Friendly Wing 1 is planes 0 and 1, the enemy's Wing 1 planes 2 and 3.
const LEAD: PlaneId = PlaneId(0);
const WINGMAN: PlaneId = PlaneId(1);
const ENEMY: PlaneId = PlaneId(2);
const ENEMY_TWO: PlaneId = PlaneId(3);
const FRIENDLY_WING: WingId = WingId {
    side: Side::Friendly,
    index: 0,
};
const ENEMY_WING: WingId = WingId {
    side: Side::Enemy,
    index: 0,
};

/// Two friendly and two enemy F/A-18s 20 nm apart, every plane on the AI.
/// Friendly Wing 1 intercepts the enemy's Wing 1 and must survive.
fn objective_mission() -> MissionSpec {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 2;
    spec.wings[3].count = 2;
    spec.separation_nm = 20;
    spec.objectives[0] = GroupObjective::Intercept(ENEMY_WING);
    spec.must_survive[0] = true;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

fn open(spec: &MissionSpec) -> World {
    World::new(spec, &resources(), Seating::Open).unwrap()
}

/// One tick with `commands` and neutral input for every seat that flies.
fn step(world: &mut World, commands: &[MissionCommand]) {
    let mut flying: Vec<SeatId> = world
        .roster
        .seats()
        .iter()
        .filter(|seat| seat.plane.is_some())
        .map(|seat| seat.id)
        .collect();
    for command in commands {
        match command {
            MissionCommand::Take { seat, .. } | MissionCommand::Revive { seat, .. } => {
                if !flying.contains(seat) {
                    flying.push(*seat);
                }
            }
            MissionCommand::GiveBack { seat } | MissionCommand::Abandon { seat } => {
                flying.retain(|s| s != seat)
            }
            _ => {}
        }
    }
    let tick = world.tick();
    let inputs: Vec<SeatInput> = flying
        .into_iter()
        .map(|seat| SeatInput {
            seat,
            tick,
            ..SeatInput::default()
        })
        .collect();
    let mut out = TickOutput::default();
    world
        .step_with(commands, &inputs, &mut out, |_, _| Ok(()))
        .unwrap();
}

fn fly(world: &mut World, ticks: usize) {
    for _ in 0..ticks {
        step(world, &[]);
    }
}

/// The AI's plane `plane` is shot down (its hit points gone).
fn destroy_ai(world: &mut World, plane: PlaneId) {
    world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == plane.0)
        .unwrap()
        .hp = 0;
}

/// The lost `root`'s lineage respawns at its original spawn: the new plane.
fn respawn(world: &mut World, root: PlaneId, origin: ([f64; 3], f64)) -> PlaneId {
    let spawn = world
        .respawn_spawn(
            root,
            origin.0,
            origin.1,
            &[],
            None,
            RevivalWeapons::Missiles,
        )
        .unwrap();
    step(
        world,
        &[MissionCommand::Respawn {
            root,
            spawn: Box::new(spawn),
        }],
    );
    world.lineage_head(root)
}

fn origin(world: &World, plane: PlaneId) -> ([f64; 3], f64) {
    world.plane_pose(plane).unwrap()
}

/// What the target window says of target `id` to the human flying `viewer`.
fn objective(world: &World, viewer: PlaneId, id: PlaneId) -> Option<TargetObjective> {
    TargetBrief::of(world.ai_wings.as_ref().unwrap(), viewer.0, id.0).objective
}

/// What `plane`'s debrief and result read the mission asks of it.
fn requirements(world: &World, plane: PlaneId) -> Requirements {
    let side = world.roster.plane(plane).unwrap().slot.wing.side;
    Requirements::of(
        world.ai_wings.as_ref().unwrap(),
        plane.0,
        side,
        &world.revival.objective_lineages(&world.roster),
    )
}

/// The objective sentences of `seat`'s debrief.
fn objectives(world: &World, seat: SeatId) -> Vec<Objective> {
    debrief::capture(world, seat).unwrap().objectives
}

#[test]
fn every_plane_of_an_open_mission_carries_its_wings_objectives() {
    let mut world = open(&objective_mission());
    step(
        &mut world,
        &[MissionCommand::Take {
            seat: SeatId(0),
            plane: LEAD,
        }],
    );
    // A human who took the lead by handoff reads the lead's objectives: the
    // enemy wing to destroy, its own wing to keep alive.
    assert_eq!(
        objective(&world, LEAD, ENEMY),
        Some(TargetObjective::Destroy)
    );
    assert_eq!(
        objective(&world, LEAD, ENEMY_TWO),
        Some(TargetObjective::Destroy)
    );
    assert_eq!(
        objective(&world, LEAD, WINGMAN),
        Some(TargetObjective::Survive)
    );
    let asked = requirements(&world, LEAD);
    assert_eq!(asked.destroy, [ENEMY.0, ENEMY_TWO.0]);
    assert_eq!(asked.protect, [LEAD.0, WINGMAN.0]);
    assert_eq!(
        objectives(&world, SeatId(0)),
        [
            Objective::Destroy {
                destroyed: 0,
                total: 2
            },
            Objective::Protect {
                protected: 2,
                total: 2
            },
        ]
    );
    // The enemy wing was given nothing to do: it destroys every aircraft of
    // the other side, human-flown ones too.
    assert_eq!(requirements(&world, ENEMY).destroy, [LEAD.0, WINGMAN.0]);
}

#[test]
fn a_respawned_objective_is_still_an_objective_and_counts_once() {
    let mut world = open(&objective_mission());
    let enemy_origin = origin(&world, ENEMY);
    step(
        &mut world,
        &[MissionCommand::Take {
            seat: SeatId(0),
            plane: LEAD,
        }],
    );
    fly(&mut world, 30);
    destroy_ai(&mut world, ENEMY);
    step(&mut world, &[]);
    let again = respawn(&mut world, ENEMY, enemy_origin);
    assert_eq!(again, PlaneId(4));
    assert_eq!(world.revival.root_of(again), ENEMY);
    // Marked for the human, and the AI wingman intercepting the wing has it
    // on its own list too.
    assert_eq!(
        objective(&world, LEAD, again),
        Some(TargetObjective::Destroy)
    );
    let wings = world.ai_wings.as_ref().unwrap();
    let wingman = wings.mission().actor(WINGMAN.0).unwrap().assignment();
    assert_eq!(wingman.destroy_ids, [ENEMY.0, ENEMY_TWO.0, again.0]);
    // The respawned plane's own wing is the enemy's: it is not asked to
    // destroy itself, and keeps its wing's orders.
    let own = wings.mission().actor(again.0).unwrap().assignment();
    assert!(!own.destroy_ids.contains(&again.0));
    // The lineage counts once, destroyed by its first loss, though it flies
    // again.
    assert_eq!(requirements(&world, LEAD).destroy, [ENEMY.0, ENEMY_TWO.0]);
    assert_eq!(
        objectives(&world, SeatId(0))[0],
        Objective::Destroy {
            destroyed: 1,
            total: 2
        }
    );
    // Shot down again: still one lineage.
    destroy_ai(&mut world, again);
    step(&mut world, &[]);
    assert_eq!(
        objectives(&world, SeatId(0))[0],
        Objective::Destroy {
            destroyed: 1,
            total: 2
        }
    );
    // The other lineage's loss completes the objective.
    destroy_ai(&mut world, ENEMY_TWO);
    step(&mut world, &[]);
    let report = debrief::capture(&world, SeatId(0)).unwrap();
    assert_eq!(
        report.objectives[0],
        Objective::Destroy {
            destroyed: 2,
            total: 2
        }
    );
    assert_eq!(report.outcome, debrief::Outcome::Success);
}

#[test]
fn a_protected_lineage_fails_on_its_first_loss_and_its_respawn_is_still_protected() {
    let mut world = open(&objective_mission());
    let wingman_origin = origin(&world, WINGMAN);
    step(
        &mut world,
        &[MissionCommand::Take {
            seat: SeatId(0),
            plane: LEAD,
        }],
    );
    fly(&mut world, 2400);
    destroy_ai(&mut world, WINGMAN);
    step(&mut world, &[]);
    let again = respawn(&mut world, WINGMAN, wingman_origin);
    assert_eq!(
        objective(&world, LEAD, again),
        Some(TargetObjective::Survive)
    );
    assert_eq!(requirements(&world, LEAD).protect, [LEAD.0, WINGMAN.0]);
    assert_eq!(
        objectives(&world, SeatId(0))[1],
        Objective::Protect {
            protected: 1,
            total: 2
        }
    );
    assert_eq!(
        debrief::capture(&world, SeatId(0)).unwrap().outcome,
        debrief::Outcome::Failure
    );
}

#[test]
fn a_revived_player_keeps_its_lineages_objectives() {
    let mut world = open(&objective_mission());
    step(
        &mut world,
        &[MissionCommand::Take {
            seat: SeatId(0),
            plane: LEAD,
        }],
    );
    fly(&mut world, 30);
    world
        .cockpits
        .iter_mut()
        .find(|c| c.plane == LEAD)
        .unwrap()
        .flight
        .systems
        .pilot
        .dead = true;
    step(&mut world, &[]);
    let start = world.side_mean(Side::Friendly).unwrap();
    let spawn = world
        .revival_spawn(
            SeatId(0),
            start,
            5. * tore_sim::sensors::FEET_PER_NAUTICAL_MILE,
            None,
            RevivalWeapons::Missiles,
        )
        .unwrap();
    step(
        &mut world,
        &[MissionCommand::Revive {
            seat: SeatId(0),
            spawn: Box::new(spawn),
        }],
    );
    let new = world.roster.seat(SeatId(0)).unwrap().plane.unwrap();
    assert_eq!(world.revival.root_of(new), LEAD);
    // The new plane is asked what the lead was asked.
    assert_eq!(
        objective(&world, new, ENEMY),
        Some(TargetObjective::Destroy)
    );
    assert_eq!(
        objective(&world, new, WINGMAN),
        Some(TargetObjective::Survive)
    );
    let mission = world.ai_wings.as_ref().unwrap().mission();
    assert_eq!(mission.must_survive(new.0), [LEAD.0, WINGMAN.0, new.0]);
    // Its own wing must survive and it was lost once: that objective has
    // failed, counted by the lineage.
    assert_eq!(requirements(&world, new).protect, [LEAD.0, WINGMAN.0]);
    assert_eq!(
        objectives(&world, SeatId(0))[1],
        Objective::Protect {
            protected: 1,
            total: 2
        }
    );
}

#[test]
fn a_redfor_player_reads_its_own_sides_objectives() {
    let mut spec = objective_mission();
    spec.objectives[3] = GroupObjective::Intercept(FRIENDLY_WING);
    spec.must_survive[3] = true;
    let mut world = open(&spec);
    step(
        &mut world,
        &[MissionCommand::Take {
            seat: SeatId(0),
            plane: ENEMY,
        }],
    );
    assert_eq!(
        objective(&world, ENEMY, LEAD),
        Some(TargetObjective::Destroy)
    );
    assert_eq!(
        objective(&world, ENEMY, ENEMY_TWO),
        Some(TargetObjective::Survive)
    );
    let asked = requirements(&world, ENEMY);
    assert_eq!(asked.destroy, [LEAD.0, WINGMAN.0]);
    assert_eq!(asked.protect, [ENEMY.0, ENEMY_TWO.0]);
}

#[test]
fn single_player_objectives_are_unchanged() {
    // A single player mission gives only the player its objectives, as
    // before: no AI plane carries a human's assignment.
    let world = World::new(&objective_mission(), &resources(), Seating::SinglePlayer).unwrap();
    let mission = world.ai_wings.as_ref().unwrap().mission();
    for plane in [WINGMAN, ENEMY, ENEMY_TWO] {
        assert!(mission.must_survive(plane.0).is_empty());
    }
    assert_eq!(
        objective(&world, LEAD, ENEMY),
        Some(TargetObjective::Destroy)
    );
    let fates = [
        ai_wings::outcome::Aircraft {
            id: 0,
            friendly: true,
            alive: true,
        },
        ai_wings::outcome::Aircraft {
            id: 1,
            friendly: true,
            alive: true,
        },
    ];
    let asked = requirements(&world, LEAD);
    let standing = Standing {
        ledger: &world.combat.state.ledger,
        plane: 0,
        aircraft: &fates,
        requirements: &asked,
    };
    assert_eq!(asked.destroy, [ENEMY.0, ENEMY_TWO.0]);
    assert_eq!(standing.destroyed(), 2, "planes not listed count as gone");
}

#[test]
fn a_checkpoint_keeps_the_joined_objectives() {
    let spec = objective_mission();
    let mut world = open(&spec);
    let enemy_origin = origin(&world, ENEMY);
    step(
        &mut world,
        &[MissionCommand::Take {
            seat: SeatId(0),
            plane: LEAD,
        }],
    );
    fly(&mut world, 30);
    destroy_ai(&mut world, ENEMY);
    step(&mut world, &[]);
    let again = respawn(&mut world, ENEMY, enemy_origin);
    fly(&mut world, 10);
    let bytes = world.checkpoint().unwrap();
    let mut restored = open(&spec);
    restored.restore(&bytes).unwrap();
    assert_eq!(
        objective(&restored, LEAD, again),
        Some(TargetObjective::Destroy)
    );
    assert_eq!(
        restored
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .human_assignment(LEAD.0),
        world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .human_assignment(LEAD.0)
    );
    for copy in [&mut world, &mut restored] {
        fly(copy, 120);
    }
    assert_eq!(restored.checkpoint().unwrap(), world.checkpoint().unwrap());
}
