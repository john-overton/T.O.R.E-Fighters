//! AI respawn in the mission core (the lobby pass's slice R1;
//! docs/ARCHITECTURE.md, "Death, revival and lives"), on an open mission
//! built from synthetic resources as a host builds it: lineages and their
//! roots; Respawn adds an AI plane in the lost one's wing with the next
//! member number at the spawn the host chose, the lineage's original spawn
//! point, stepped back when it is taken; the weapons rule; a human's lost
//! plane is not the AI's, an abandoned one is; the replaced AI wreck retires
//! once rested; member 255 stops a wing; a client's copy adds the same
//! plane; the revival point stays on the map at 150 nm; and a checkpoint
//! keeps the roots.

use super::*;
use crate::{
    mission::{MissionSpec, Skill, Start},
    seats::{Pilot, PlaneId, SeatId, SeatInput},
    test_support::resources::{THEATER, resources},
    world::{MissionCommand, Seating, TickOutput},
};
use tore_formats::aircraft::AircraftId;
use tore_sim::{ai::launch::WingId, combat::live::is_gun};

const NM: f64 = FEET_PER_NAUTICAL_MILE;
/// Friendly Wing 1 is planes 0 and 1, the enemy's Wing 1 planes 2 and 3.
const LEAD: PlaneId = PlaneId(0);
const WINGMAN: PlaneId = PlaneId(1);
const ENEMY: PlaneId = PlaneId(2);
const FRIENDLY_WING: WingId = WingId {
    side: Side::Friendly,
    index: 0,
};

/// Two friendly and two enemy F/A-18s 20 nm apart, airborne at 10,000
/// feet, every plane on the AI; the enemy wing at `enemy_skill`.
fn open_mission(enemy_skill: Skill) -> World {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 2;
    spec.wings[3].count = 2;
    spec.wings[3].skill = enemy_skill;
    spec.separation_nm = 20;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    World::new(&spec, &resources(), Seating::Open).unwrap()
}

/// One tick with `commands` and neutral input for every seat that flies a
/// plane once they apply.
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
            MissionCommand::Take { seat, .. } => flying.push(*seat),
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

/// The AI's plane `plane` is destroyed.
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

/// Every plane's pose at the mission's first tick, as the host records it.
fn origins(world: &World) -> std::collections::BTreeMap<PlaneId, ([f64; 3], f64)> {
    world
        .lineage_roots()
        .into_iter()
        .map(|root| (root, world.plane_pose(root).unwrap()))
        .collect()
}

/// The respawn of `root`'s lineage at its original spawn with `weapons`.
fn respawn_at_origin(world: &World, root: PlaneId, origin: ([f64; 3], f64)) -> Spawn {
    world
        .respawn_spawn(
            root,
            origin.0,
            origin.1,
            &[],
            None,
            RevivalWeapons::Missiles,
        )
        .unwrap()
}

/// What the ownship or AI aircraft `plane` carries on each station.
fn rounds(world: &World, plane: PlaneId) -> Vec<u32> {
    use tore_sim::ai::weapon_service::Rounds;
    world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(plane.0)
        .unwrap()
        .stations()
        .iter()
        .map(|s| match s.store.rounds {
            Rounds::Finite(n) => n,
            Rounds::Unlimited => u32::MAX,
        })
        .collect()
}

#[test]
fn a_respawn_adds_an_ai_plane_in_the_wing_at_its_original_spawn() {
    let mut world = open_mission(Skill::Average);
    let origins = origins(&world);
    assert_eq!(world.lineage_roots(), [LEAD, WINGMAN, ENEMY, PlaneId(3)]);
    // Long enough for the wing to leave the wingman's start behind.
    fly(&mut world, 2400);
    assert!(!world.lineage_lost(WINGMAN));
    let spawn = respawn_at_origin(&world, WINGMAN, origins[&WINGMAN]);
    assert!(
        world.respawn_new(WINGMAN, &spawn).is_err(),
        "a lineage that still flies does not respawn"
    );
    destroy_ai(&mut world, WINGMAN);
    step(&mut world, &[]);
    assert!(world.lineage_lost(WINGMAN));
    assert!(!world.lineage_lost(LEAD));
    // The spawn: the original point and heading, raised clear of the ground,
    // the airborne start speed and the standard load with full fuel.
    let spawn = respawn_at_origin(&world, WINGMAN, origins[&WINGMAN]);
    let (origin, heading) = origins[&WINGMAN];
    assert_eq!(
        [spawn.position[0], spawn.position[2]],
        [origin[0], origin[2]]
    );
    assert!(spawn.position[1] >= origin[1]);
    assert_eq!(spawn.heading_rad, heading);
    assert!(spawn.speed_fps > 300.);
    assert!(spawn.loadout.fuel_lbs > 0.);
    let new = world.respawn_new(WINGMAN, &spawn).unwrap();
    assert_eq!(new.plane, PlaneId(4));
    assert_eq!(
        new.slot,
        Slot {
            wing: FRIENDLY_WING,
            member: 2
        }
    );
    step(
        &mut world,
        &[MissionCommand::Respawn {
            root: WINGMAN,
            spawn: Box::new(spawn.clone()),
        }],
    );
    // The new plane is the AI's, in the wing, where the spawn put it (one
    // tick on), with the wing's own skill.
    let entry = world.roster.plane(new.plane).unwrap();
    assert_eq!((entry.slot, entry.pilot), (new.slot, Pilot::Ai));
    let wings = world.ai_wings.as_ref().unwrap();
    let actor = wings.mission().actor(new.plane.0).unwrap();
    assert!(actor.alive());
    assert!(!actor.identity().is_leader());
    let launch = world
        .setup
        .ai
        .as_ref()
        .unwrap()
        .wings
        .iter()
        .find(|w| w.wing == FRIENDLY_WING)
        .unwrap();
    assert_eq!(actor.experience(), launch.members[0].experience);
    let distance = (0..3)
        .map(|i| (actor.flight().position[i] - spawn.position[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    assert!(distance < spawn.speed_fps, "{distance} ft from the spawn");
    // The lineage: its root, the new plane its head, no longer lost.
    assert_eq!(world.revival.root_of(new.plane), WINGMAN);
    assert_eq!(world.lineage(WINGMAN), [WINGMAN, new.plane]);
    assert_eq!(world.lineage_head(WINGMAN), new.plane);
    assert_eq!(world.lineage_heads()[&WINGMAN], new.plane);
    assert!(!world.lineage_lost(WINGMAN));
    assert_eq!(world.revival.added(), [new.plane]);
    // The replaced AI wreck waits to retire; nothing else changed hands.
    let waiting: Vec<PlaneId> = world.revival.lost().iter().map(|l| l.plane).collect();
    assert_eq!(waiting, [WINGMAN]);
    assert_eq!(world.roster.plane(WINGMAN).unwrap().pilot, Pilot::Ai);
    // Respawned again only once lost again.
    let again = respawn_at_origin(&world, WINGMAN, origins[&WINGMAN]);
    assert!(world.can_respawn(WINGMAN, &again).is_err());
    // An added plane roots no lineage.
    assert!(world.can_respawn(new.plane, &again).is_err());
}

#[test]
fn a_taken_respawn_point_steps_back_along_the_heading() {
    let mut world = open_mission(Skill::Average);
    let origins = origins(&world);
    fly(&mut world, 2);
    destroy_ai(&mut world, WINGMAN);
    step(&mut world, &[]);
    let (origin, heading) = origins[&WINGMAN];
    let back = Basis::new(heading, 0., 0.).forward.map(|v| -v);
    let stepped = |steps: f64| {
        [
            origin[0] + back[0] * steps * NM,
            origin[2] + back[2] * steps * NM,
        ]
    };
    let at = |taken: &[[f64; 3]]| {
        let spawn = world
            .respawn_spawn(
                WINGMAN,
                origin,
                heading,
                taken,
                None,
                RevivalWeapons::Missiles,
            )
            .unwrap();
        [spawn.position[0], spawn.position[2]]
    };
    let close = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6;
    // Two ticks in, the lead still flies within 2,000 ft of the wingman's
    // start: one step back.
    assert!(close(at(&[]), stepped(1.)));
    // A respawn placed this tick 1 nm back takes that point too.
    let one_back = [stepped(1.)[0], origin[1], stepped(1.)[1]];
    assert!(close(at(&[one_back]), stepped(2.)));
    // Never more than five steps.
    let all: Vec<[f64; 3]> = (0..10)
        .map(|n| {
            let p = stepped(f64::from(n));
            [p[0], origin[1], p[1]]
        })
        .collect();
    assert!(close(at(&all), stepped(5.)));
}

#[test]
fn the_weapons_rule_cuts_a_respawns_stores() {
    let mut world = open_mission(Skill::Average);
    let origins = origins(&world);
    fly(&mut world, 2400);
    destroy_ai(&mut world, ENEMY);
    step(&mut world, &[]);
    let (origin, heading) = origins[&ENEMY];
    let spawn = world
        .respawn_spawn(ENEMY, origin, heading, &[], None, RevivalWeapons::Guns)
        .unwrap();
    step(
        &mut world,
        &[MissionCommand::Respawn {
            root: ENEMY,
            spawn: Box::new(spawn.clone()),
        }],
    );
    let new = world.lineage_head(ENEMY);
    assert_eq!(new, PlaneId(4));
    let config = world
        .ai_wings
        .as_ref()
        .unwrap()
        .configuration(new.0)
        .unwrap()
        .clone();
    let carried = rounds(&world, new);
    let mut guns = 0;
    for (station, rounds) in config.stations.iter().zip(&carried) {
        if is_gun(&station.weapon) {
            guns += 1;
            assert!(*rounds > 0);
        } else {
            assert_eq!(*rounds, 0, "{}", station.weapon.source);
        }
    }
    assert!(guns > 0);
    assert_eq!(world.roster.plane(new).unwrap().slot.wing.side, Side::Enemy);
}

#[test]
fn a_humans_lost_plane_is_its_own_until_it_is_abandoned() {
    let mut world = open_mission(Skill::Average);
    let origins = origins(&world);
    world.take_plane(SeatId(0), LEAD).unwrap();
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
    // Lost, and the seat still holds it: the human's revival, not the AI's.
    assert!(world.plane_lost(LEAD));
    assert!(!world.lineage_lost(LEAD));
    let spawn = respawn_at_origin(&world, LEAD, origins[&LEAD]);
    assert!(world.can_respawn(LEAD, &spawn).is_err());
    // The player leaves the game: the wreck is the mission's, and the AI's
    // to respawn.
    step(&mut world, &[MissionCommand::Abandon { seat: SeatId(0) }]);
    assert!(world.lineage_lost(LEAD));
    step(
        &mut world,
        &[MissionCommand::Respawn {
            root: LEAD,
            spawn: Box::new(spawn),
        }],
    );
    let new = world.lineage_head(LEAD);
    assert_eq!(new, PlaneId(4));
    assert_eq!(world.roster.plane(new).unwrap().pilot, Pilot::Ai);
    // The abandoned wreck is in the retiring list once, as before.
    let waiting: Vec<PlaneId> = world.revival.lost().iter().map(|l| l.plane).collect();
    assert_eq!(waiting, [LEAD]);
}

#[test]
fn a_revival_continues_its_planes_lineage() {
    let mut world = open_mission(Skill::Average);
    world.take_plane(SeatId(0), WINGMAN).unwrap();
    fly(&mut world, 30);
    world
        .cockpits
        .iter_mut()
        .find(|c| c.plane == WINGMAN)
        .unwrap()
        .flight
        .systems
        .pilot
        .dead = true;
    step(&mut world, &[]);
    let start = world.side_mean(Side::Friendly).unwrap();
    let spawn = world
        .revival_spawn(SeatId(0), start, 5. * NM, None, RevivalWeapons::Missiles)
        .unwrap();
    step(
        &mut world,
        &[MissionCommand::Revive {
            seat: SeatId(0),
            spawn: Box::new(spawn),
        }],
    );
    let new = world.roster.seat(SeatId(0)).unwrap().plane.unwrap();
    assert_eq!(world.revival.root_of(new), WINGMAN);
    assert_eq!(world.lineage(WINGMAN), [WINGMAN, new]);
    // A human flies it: not lost.
    assert!(!world.lineage_lost(WINGMAN));
}

#[test]
fn the_replaced_ai_wreck_retires_once_it_has_rested() {
    let mut world = open_mission(Skill::Average);
    let origins = origins(&world);
    fly(&mut world, 2);
    destroy_ai(&mut world, ENEMY);
    step(&mut world, &[]);
    let spawn = respawn_at_origin(&world, ENEMY, origins[&ENEMY]);
    step(
        &mut world,
        &[MissionCommand::Respawn {
            root: ENEMY,
            spawn: Box::new(spawn),
        }],
    );
    let new = world.lineage_head(ENEMY);
    assert_eq!(world.room(), MAX_PLANES - 5);
    // The wreck falls, comes to rest, and may retire 30 seconds later.
    let mut rested_at = None;
    for _ in 0..30_000 {
        step(&mut world, &[]);
        if rested_at.is_none() {
            rested_at = world.revival.lost()[0].resting_since;
        }
        if world.retirable().is_some() {
            break;
        }
    }
    let rested_at = rested_at.expect("the AI wreck never came to rest");
    assert_eq!(world.retirable(), Some(ENEMY));
    assert!(world.tick() - rested_at >= RETIRE_AFTER_TICKS);
    assert_eq!(world.room(), MAX_PLANES - 5 + 1);
    world.retire_plane(ENEMY).unwrap();
    assert!(world.roster.plane(ENEMY).is_none());
    assert!(
        world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actor(ENEMY.0)
            .is_none()
    );
    assert!(world.combat.state.targets.iter().all(|t| t.id != ENEMY.0));
    let retired = world.revival.retired();
    assert_eq!((retired[0].id, retired[0].pilot), (ENEMY, Pilot::Lost));
    assert!(world.revival.lost().is_empty());
    // The lineage keeps its root, and its newest plane flies on.
    assert_eq!(world.lineage_roots(), [LEAD, WINGMAN, ENEMY, PlaneId(3)]);
    assert_eq!(world.lineage_head(ENEMY), new);
    fly(&mut world, 120);
    assert!(
        world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actor(new.0)
            .is_some()
    );
}

#[test]
fn a_lineage_whose_newest_plane_retired_still_respawns() {
    let mut world = open_mission(Skill::Average);
    let origins = origins(&world);
    world.take_plane(SeatId(0), LEAD).unwrap();
    fly(&mut world, 2);
    world
        .cockpits
        .iter_mut()
        .find(|c| c.plane == LEAD)
        .unwrap()
        .flight
        .crashed = true;
    step(&mut world, &[]);
    step(&mut world, &[MissionCommand::Abandon { seat: SeatId(0) }]);
    // Retired for room before the AI's delay ran out.
    world.retire_plane(LEAD).unwrap();
    assert!(world.lineage_lost(LEAD));
    let spawn = respawn_at_origin(&world, LEAD, origins[&LEAD]);
    step(
        &mut world,
        &[MissionCommand::Respawn {
            root: LEAD,
            spawn: Box::new(spawn),
        }],
    );
    let new = world.lineage_head(LEAD);
    let entry = world.roster.plane(new).unwrap();
    assert_eq!(
        (entry.slot.wing, entry.slot.member, entry.pilot),
        (FRIENDLY_WING, 2, Pilot::Ai)
    );
}

#[test]
fn a_wing_out_of_member_numbers_stops_respawning() {
    let mut world = open_mission(Skill::Average);
    let origins = origins(&world);
    fly(&mut world, 2);
    destroy_ai(&mut world, WINGMAN);
    step(&mut world, &[]);
    let spawn = respawn_at_origin(&world, WINGMAN, origins[&WINGMAN]);
    // A wing that has had member 254 (by hand: 250 respawns' worth) gives
    // 255 to nobody: the wire's member is a byte, 255 kept back.
    world.revival.retired.push(Plane {
        id: PlaneId(90),
        slot: Slot {
            wing: FRIENDLY_WING,
            member: 254,
        },
        pilot: Pilot::Lost,
    });
    let error = world.can_respawn(WINGMAN, &spawn).unwrap_err().to_string();
    assert!(error.contains("no member number left"), "{error}");
    // The other side's wings are unaffected.
    destroy_ai(&mut world, ENEMY);
    step(&mut world, &[]);
    let spawn = respawn_at_origin(&world, ENEMY, origins[&ENEMY]);
    world.can_respawn(ENEMY, &spawn).unwrap();
}

#[test]
fn a_dummy_wings_respawn_is_a_training_target() {
    let mut world = open_mission(Skill::Dummy);
    let origins = origins(&world);
    fly(&mut world, 2);
    destroy_ai(&mut world, ENEMY);
    step(&mut world, &[]);
    let spawn = respawn_at_origin(&world, ENEMY, origins[&ENEMY]);
    step(
        &mut world,
        &[MissionCommand::Respawn {
            root: ENEMY,
            spawn: Box::new(spawn),
        }],
    );
    let new = world.lineage_head(ENEMY);
    let wings = world.ai_wings.as_ref().unwrap();
    assert!(wings.mission().actor(new.0).unwrap().is_dummy());
    assert!(!wings.mission().actor(WINGMAN.0).unwrap().is_dummy());
}

#[test]
fn a_clients_copy_adds_the_respawned_plane_from_the_spawned_message() {
    let mut world = open_mission(Skill::Average);
    let origins = origins(&world);
    fly(&mut world, 2);
    destroy_ai(&mut world, WINGMAN);
    step(&mut world, &[]);
    let spawn = respawn_at_origin(&world, WINGMAN, origins[&WINGMAN]);
    let new = world.respawn_new(WINGMAN, &spawn).unwrap();
    step(
        &mut world,
        &[MissionCommand::Respawn {
            root: WINGMAN,
            spawn: Box::new(spawn.clone()),
        }],
    );
    // A client's copy: the mission as built, all on the AI.
    let mut copy = open_mission(Skill::Average);
    copy.add_plane(&new).unwrap();
    let host = world.roster.plane(new.plane).unwrap();
    let client = copy.roster.plane(new.plane).unwrap();
    assert_eq!((client.slot, client.pilot), (host.slot, host.pilot));
    let row = |w: &World| {
        let row = w
            .combat
            .state
            .targets
            .iter()
            .find(|t| t.id == new.plane.0)
            .unwrap();
        (row.side, row.aircraft)
    };
    assert_eq!(row(&copy), row(&world));
    assert_eq!(
        copy.ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actor(new.plane.0)
            .unwrap()
            .experience(),
        world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actor(new.plane.0)
            .unwrap()
            .experience()
    );
    assert_eq!(rounds(&copy, new.plane), rounds(&world, new.plane));
}

#[test]
fn a_revival_far_from_a_battle_near_the_edge_stays_on_the_map() {
    let mut world = open_mission(Skill::Average);
    world.take_plane(SeatId(0), LEAD).unwrap();
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
    let bounds = world.map_bounds();
    let start = world.side_mean(Side::Friendly).unwrap();
    for distance in [1., 10., 40., 50., 75., 100., 150.] {
        let spawn = world
            .revival_spawn(
                SeatId(0),
                start,
                distance * NM,
                None,
                RevivalWeapons::Missiles,
            )
            .unwrap();
        let [x, _, z] = spawn.position;
        assert!(
            (bounds.min[0]..=bounds.max[0]).contains(&x)
                && (bounds.min[1]..=bounds.max[1]).contains(&z),
            "{distance} nm: ({x}, {z}) off the map {bounds:?}"
        );
    }
}

#[test]
fn onto_map_walks_a_point_back_towards_the_centre() {
    let bounds = MapBounds {
        min: [0., 0.],
        max: [100., 200.],
    };
    // On the map: unchanged.
    assert_eq!(
        onto_map([50., 7., 50.], [10., 0., 10.], &bounds),
        [50., 7., 50.]
    );
    // Off the east edge: back along the line to it, the altitude kept.
    assert_eq!(
        onto_map([150., 7., 60.], [50., 0., 10.], &bounds),
        [100., 7., 35.]
    );
    // Off two edges: the nearer cut wins.
    assert_eq!(
        onto_map([-50., 7., 300.], [50., 0., 100.], &bounds),
        [0., 7., 200.]
    );
    // A centre off the map is clamped onto it first.
    assert_eq!(
        onto_map([500., 7., 500.], [300., 0., 300.], &bounds),
        [100., 7., 200.]
    );
}

#[test]
fn a_checkpoint_keeps_the_lineages() {
    let mut world = open_mission(Skill::Average);
    let origins = origins(&world);
    fly(&mut world, 2);
    destroy_ai(&mut world, WINGMAN);
    step(&mut world, &[]);
    let spawn = respawn_at_origin(&world, WINGMAN, origins[&WINGMAN]);
    step(
        &mut world,
        &[MissionCommand::Respawn {
            root: WINGMAN,
            spawn: Box::new(spawn),
        }],
    );
    destroy_ai(&mut world, ENEMY);
    fly(&mut world, 10);
    let bytes = world.checkpoint().unwrap();
    let mut restored = open_mission(Skill::Average);
    restored.restore(&bytes).unwrap();
    assert_eq!(restored.revival, world.revival);
    assert_eq!(restored.revival.root_of(PlaneId(4)), WINGMAN);
    assert!(restored.lineage_lost(ENEMY));
    // Both respawn the enemy the same way and step on alike.
    for copy in [&mut world, &mut restored] {
        let spawn = respawn_at_origin(copy, ENEMY, origins[&ENEMY]);
        step(
            copy,
            &[MissionCommand::Respawn {
                root: ENEMY,
                spawn: Box::new(spawn),
            }],
        );
        fly(copy, 120);
    }
    assert_eq!(restored.checkpoint().unwrap(), world.checkpoint().unwrap());
}
