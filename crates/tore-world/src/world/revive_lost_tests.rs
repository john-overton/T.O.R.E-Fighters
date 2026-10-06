//! Reviving a seat from a lost plane nobody holds (slice K5;
//! docs/ARCHITECTURE.md, "Rejoin tokens and reservations"): a player who
//! returns to a game whose AI lost the aircraft reserved for it flies again
//! by the revival rules, in a new plane of the lost one's aircraft and wing.
//! The seat flies no plane, so the existing Revive (which abandons the
//! seat's own lost plane) cannot do it.

use super::*;
use crate::{
    mission::{MissionSpec, Skill, Start},
    seats::{Pilot, PlaneId, SeatId, SeatInput},
    test_support::resources::{THEATER, resources},
    world::{MissionCommand, Seating, TickOutput},
};
use tore_formats::aircraft::AircraftId;

const LEAD: PlaneId = PlaneId(0);
const ENEMY: PlaneId = PlaneId(2);

fn open_mission() -> World {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 2;
    spec.wings[3].count = 2;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 50;
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
            MissionCommand::Take { seat, .. } | MissionCommand::ReviveLost { seat, .. } => {
                flying.push(*seat)
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

fn spawn_for(world: &World, plane: PlaneId) -> Spawn {
    let start = world.side_mean(Side::Friendly).unwrap();
    world
        .revival_spawn_from(
            plane,
            start,
            10. * FEET_PER_NAUTICAL_MILE,
            None,
            RevivalWeapons::Missiles,
        )
        .unwrap()
}

/// The AI's plane `plane` is destroyed.
fn destroy_ai(world: &mut World, plane: PlaneId) {
    let row = world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == plane.0)
        .unwrap();
    row.hp = 0;
}

#[test]
fn a_living_ai_plane_cannot_be_revived_from() {
    let mut world = open_mission();
    step(&mut world, &[]);
    assert!(
        world
            .revival_spawn_from(
                LEAD,
                [0., 0., 0.],
                10. * FEET_PER_NAUTICAL_MILE,
                None,
                RevivalWeapons::Missiles
            )
            .is_err()
    );
    let spawn = Spawn {
        position: [0., 10_000., 0.],
        heading_rad: 0.,
        speed_fps: 600.,
        loadout: world
            .revival_loadout(AircraftId::F18, None, RevivalWeapons::Missiles)
            .unwrap(),
    };
    assert!(world.revival_plane_from(LEAD, &spawn).is_err());
    let before = world.roster.planes().len();
    assert!(world.revive_lost_plane(SeatId(0), LEAD, &spawn).is_err());
    assert_eq!(
        world.roster.planes().len(),
        before,
        "a refusal changes nothing"
    );
    assert!(
        world
            .roster
            .seat(SeatId(0))
            .is_none_or(|s| s.plane.is_none())
    );
    // A plane that is not in the mission, too.
    assert!(
        world
            .revive_lost_plane(SeatId(0), PlaneId(99), &spawn)
            .is_err()
    );
}

#[test]
fn a_seat_with_no_plane_revives_from_an_ai_plane_the_ai_lost() {
    let mut world = open_mission();
    step(&mut world, &[]);
    destroy_ai(&mut world, LEAD);
    step(&mut world, &[]);
    assert!(world.can_take(NOBODY, LEAD).is_err(), "the AI lost plane 0");
    let spawn = spawn_for(&world, LEAD);
    let new = world.revival_plane_from(LEAD, &spawn).unwrap();
    assert_eq!(new.plane, PlaneId(4));
    assert_eq!(new.aircraft, AircraftId::F18);
    assert_eq!(new.slot.wing, world.roster.plane(LEAD).unwrap().slot.wing);
    step(
        &mut world,
        &[MissionCommand::ReviveLost {
            seat: SeatId(3),
            plane: LEAD,
            spawn: Box::new(spawn.clone()),
        }],
    );
    // The seat flies the new plane in the lost one's wing; the lost one is
    // as the AI left it.
    assert_eq!(
        world.roster.seat(SeatId(3)).unwrap().plane,
        Some(PlaneId(4))
    );
    assert_eq!(
        world.roster.plane(PlaneId(4)).unwrap().pilot,
        Pilot::Human(SeatId(3))
    );
    assert_eq!(world.roster.plane(PlaneId(4)).unwrap().slot, new.slot);
    assert_eq!(world.roster.plane(LEAD).unwrap().pilot, Pilot::Ai);
    assert!(!world.plane_lost(PlaneId(4)));
    // At the spawn's place.
    let cockpit = world
        .cockpits
        .iter()
        .find(|c| c.plane == PlaneId(4))
        .unwrap();
    let moved = (cockpit.flight.position[0] - spawn.position[0])
        .hypot(cockpit.flight.position[2] - spawn.position[2]);
    assert!(moved < 3. * spawn.speed_fps / 120., "{moved} ft");
}

#[test]
fn a_seat_that_flies_a_plane_cannot_revive_from_a_lost_one() {
    let mut world = open_mission();
    world.take_plane(SeatId(0), ENEMY).unwrap();
    step(&mut world, &[]);
    destroy_ai(&mut world, LEAD);
    step(&mut world, &[]);
    let spawn = spawn_for(&world, LEAD);
    let before = world.roster.planes().len();
    assert!(
        world
            .revive_lost_plane(SeatId(0), LEAD, &spawn)
            .is_err_and(|e| e.to_string().contains("already flies"))
    );
    assert_eq!(world.roster.planes().len(), before);
}

#[test]
fn a_wreck_already_abandoned_can_be_revived_from_too() {
    let mut world = open_mission();
    world.take_plane(SeatId(0), LEAD).unwrap();
    step(&mut world, &[]);
    world
        .cockpits
        .iter_mut()
        .find(|c| c.plane == LEAD)
        .unwrap()
        .flight
        .systems
        .pilot
        .dead = true;
    step(&mut world, &[MissionCommand::Abandon { seat: SeatId(0) }]);
    assert_eq!(world.roster.plane(LEAD).unwrap().pilot, Pilot::Lost);
    let spawn = spawn_for(&world, LEAD);
    step(
        &mut world,
        &[MissionCommand::ReviveLost {
            seat: SeatId(0),
            plane: LEAD,
            spawn: Box::new(spawn),
        }],
    );
    assert_eq!(
        world.roster.seat(SeatId(0)).unwrap().plane,
        Some(PlaneId(4))
    );
    assert_eq!(world.roster.plane(LEAD).unwrap().pilot, Pilot::Lost);
}

#[test]
fn the_same_revivals_from_lost_planes_step_the_same_way_twice() {
    let run = || {
        let mut world = open_mission();
        step(&mut world, &[]);
        destroy_ai(&mut world, LEAD);
        step(&mut world, &[]);
        let spawn = spawn_for(&world, LEAD);
        step(
            &mut world,
            &[MissionCommand::ReviveLost {
                seat: SeatId(3),
                plane: LEAD,
                spawn: Box::new(spawn),
            }],
        );
        for _ in 0..240 {
            step(&mut world, &[]);
        }
        world
            .cockpits
            .iter()
            .find(|c| c.plane == PlaneId(4))
            .map(|c| (c.flight.position, c.flight.speed))
            .unwrap()
    };
    assert_eq!(run(), run());
}
