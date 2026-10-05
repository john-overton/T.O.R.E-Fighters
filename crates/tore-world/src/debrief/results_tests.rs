//! The multiplayer results in the mission core (slice F2-D;
//! docs/ARCHITECTURE.md, "The multiplayer debrief"): a row for every plane
//! the mission had, from the same rule each seat's report uses, read for
//! each plane from its own side. The fixture is an open mission built from
//! synthetic resources as a host builds it.

use super::*;
use crate::{
    mission::{MissionSpec, Skill, Start},
    seats::{PlaneId, SeatId, SeatInput},
    test_support::resources::{THEATER, resources},
    world::{MissionCommand, Seating, TickOutput},
};
use tore_sim::ai::launch::Side;
use tore_sim::combat::ledger::{Kill, Resolution};

const NM: f64 = tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
const LEAD: PlaneId = PlaneId(0);
const WINGMAN: PlaneId = PlaneId(1);
const ENEMY: PlaneId = PlaneId(2);
const ENEMY_WINGMAN: PlaneId = PlaneId(3);

/// Two friendly and two enemy F/A-18s, airborne, every plane on the AI.
fn open_mission() -> World {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 2;
    spec.wings[3].count = 2;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 20;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    World::new(&spec, &resources(), Seating::Open).unwrap()
}

/// The open mission with seat 0 in plane 0 and seat 1 in plane 2.
fn seated_mission() -> World {
    let mut world = open_mission();
    world.take_plane(SeatId(0), LEAD).unwrap();
    world.take_plane(SeatId(1), ENEMY).unwrap();
    world
}

fn step(world: &mut World, commands: &[MissionCommand]) {
    let mut flying: Vec<SeatId> = world
        .roster
        .seats()
        .iter()
        .filter(|seat| seat.plane.is_some())
        .map(|seat| seat.id)
        .collect();
    for command in commands {
        if let MissionCommand::Abandon { seat } = command {
            flying.retain(|s| s != seat);
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
    world
        .step_with(commands, &inputs, &mut TickOutput::default(), |_, _| Ok(()))
        .unwrap();
}

fn cockpit_mut(world: &mut World, plane: PlaneId) -> &mut crate::world::Cockpit {
    world
        .cockpits
        .iter_mut()
        .find(|c| c.plane == plane)
        .expect("a cockpit")
}

fn row(rows: &[PlaneResult], plane: PlaneId) -> PlaneResult {
    *rows
        .iter()
        .find(|row| row.plane == plane)
        .unwrap_or_else(|| panic!("no row for plane {}", plane.0))
}

fn credit(owner: PlaneId, victim: u32, category: u16, aircraft: bool) -> Kill {
    Kill {
        owner: owner.0,
        victim,
        category,
        aircraft,
    }
}

/// `owner` fires `launched` rounds of `kind` at `aim`, `hit` of them hitting
/// for 10 points each.
fn shoot(world: &mut World, owner: PlaneId, kind: ShotKind, launched: u32, hit: u32) {
    let ledger = &mut world.combat.state.ledger;
    for n in 0..launched {
        let projectile = owner.0 * 1_000 + kind as u32 * 100 + n;
        ledger.launch(projectile, owner.0, None, kind);
        ledger.resolve(
            projectile,
            if n < hit {
                Resolution::Hit(10)
            } else {
                Resolution::Missed
            },
        );
    }
}

#[test]
fn a_fresh_thirty_aircraft_mission_has_a_row_for_every_plane_in_page_order() {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    for wing in &mut spec.wings {
        wing.count = 5;
    }
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    let world = World::new(&spec, &resources(), Seating::Open).unwrap();
    let rows = results(&world);
    assert_eq!(rows.len(), 30);
    assert_eq!(rows.len(), world.roster.planes().len());
    // Friendly side first, then each wing, then each member.
    let keys: Vec<(bool, u8, u8)> = rows
        .iter()
        .map(|r| {
            (
                r.slot.wing.side.is_enemy(),
                r.slot.wing.index,
                r.slot.member,
            )
        })
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
    assert_eq!(keys.iter().filter(|k| !k.0).count(), 15);
    for row in &rows {
        assert_eq!(row.aircraft, AircraftId::F18);
        assert_eq!(row.status, RowStatus::Alive);
        assert_eq!(row.damage, 0.);
        assert_eq!((row.aircraft_kills, row.other_kills), (0, 0));
        assert_eq!(row.friendly_fire, 0);
        assert_eq!(row.air_to_air, Tally::default());
    }
}

#[test]
fn each_row_reads_its_own_planes_kills_losses_and_shots_human_or_ai() {
    let mut world = seated_mission();
    // The human lead downs the enemy wingman (AI) with a missile after
    // 12 gun rounds that hit four times, then its own wingman by mistake.
    shoot(&mut world, LEAD, ShotKind::AirToAir, 2, 1);
    shoot(&mut world, LEAD, ShotKind::Gun, 12, 4);
    world
        .combat
        .state
        .ledger
        .kill(credit(LEAD, ENEMY_WINGMAN.0, 0x8000, true));
    world
        .combat
        .state
        .ledger
        .kill(credit(LEAD, WINGMAN.0, 0x4000, true));
    // The enemy human destroys a structure and then the lead (credited by
    // the last hit alone: no recorded kill), and the enemy AI flies on.
    shoot(&mut world, ENEMY, ShotKind::AirToGround, 3, 2);
    shoot(&mut world, ENEMY, ShotKind::Bomb, 2, 1);
    world
        .combat
        .state
        .ledger
        .kill(credit(ENEMY, 900, 0x100, false));
    world
        .combat
        .state
        .ledger
        .damaged(credit(ENEMY, LEAD.0, 0x8000, true));
    // The fates: the lead's pilot died, both AI wingmen crashed.
    cockpit_mut(&mut world, LEAD).flight.systems.pilot.dead = true;
    let wings = world.ai_wings.as_mut().unwrap().mission_mut();
    for plane in [WINGMAN, ENEMY_WINGMAN] {
        wings.actor_mut(plane.0).unwrap().flight_mut().crashed = true;
    }
    step(&mut world, &[]);
    let rows = results(&world);
    assert_eq!(rows.len(), 4);

    let lead = row(&rows, LEAD);
    assert_eq!(lead.status, RowStatus::Dead);
    assert_eq!(lead.damage, 1.);
    // One fighter, one friendly (a bomber of its own side): friendly fire.
    assert_eq!((lead.aircraft_kills, lead.other_kills), (1, 0));
    assert_eq!(lead.friendly_fire, 1);
    assert_eq!((lead.air_to_air.launched, lead.air_to_air.hit), (2, 1));
    assert_eq!((lead.gun.launched, lead.gun.hit), (12, 4));
    assert_eq!(lead.air_to_ground, Tally::default());

    // An AI plane has a row like a human's, and its loss is credited.
    let wingman = row(&rows, WINGMAN);
    assert_eq!(wingman.status, RowStatus::Dead);
    assert_eq!(wingman.damage, 1.);
    assert_eq!(wingman.aircraft_kills, 0);

    // The enemy human: the structure is "other", the lead's loss a kill by
    // the last hit, and missiles and bombs share the air-to-ground column.
    let enemy = row(&rows, ENEMY);
    assert_eq!(enemy.status, RowStatus::Alive);
    assert_eq!((enemy.aircraft_kills, enemy.other_kills), (1, 1));
    assert_eq!(enemy.friendly_fire, 0);
    assert_eq!(
        (enemy.air_to_ground.launched, enemy.air_to_ground.hit),
        (5, 3)
    );
    assert_eq!(enemy.air_to_ground.damage, 30);

    let wingman = row(&rows, ENEMY_WINGMAN);
    assert_eq!(wingman.status, RowStatus::Dead);
    assert_eq!(wingman.slot.wing.side, Side::Enemy);
    // The shots of the rows add up to the ledger's.
    let launched: u32 = rows
        .iter()
        .map(|r| r.air_to_air.launched + r.gun.launched + r.air_to_ground.launched)
        .sum();
    assert_eq!(launched, 2 + 12 + 5);
}

#[test]
fn a_lost_planes_ejected_pilot_shows_ejected_and_a_flying_plane_its_damage() {
    let mut world = seated_mission();
    cockpit_mut(&mut world, ENEMY).flight.systems.pilot.ejected = true;
    cockpit_mut(&mut world, LEAD).flight.damage_fraction = 0.4;
    let rows = results(&world);
    assert_eq!(row(&rows, ENEMY).status, RowStatus::Ejected);
    assert_eq!(row(&rows, ENEMY).damage, 1.);
    assert_eq!(row(&rows, LEAD).status, RowStatus::Alive);
    assert_eq!(row(&rows, LEAD).damage, 0.4);
    assert_eq!(row(&rows, WINGMAN).damage, 0.);
}

#[test]
fn a_revived_player_has_its_new_plane_and_the_retired_wreck_keeps_its_row() {
    let mut world = seated_mission();
    let start = world.side_mean(Side::Friendly).unwrap();
    shoot(&mut world, LEAD, ShotKind::Gun, 6, 3);
    world
        .combat
        .state
        .ledger
        .kill(credit(LEAD, ENEMY_WINGMAN.0, 0x8000, true));
    cockpit_mut(&mut world, LEAD).flight.crashed = true;
    step(&mut world, &[]);
    let spawn = world
        .revival_spawn(
            SeatId(0),
            start,
            10. * NM,
            None,
            crate::world::revive::RevivalWeapons::Missiles,
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
    assert_eq!(new, PlaneId(4));
    // Before the wreck is retired it is a lost plane, dead.
    let rows = results(&world);
    assert_eq!(rows.len(), 5);
    assert_eq!(row(&rows, LEAD).status, RowStatus::Dead);
    assert_eq!(row(&rows, new).status, RowStatus::Alive);
    assert_eq!(row(&rows, new).aircraft, AircraftId::F18);
    assert_eq!(row(&rows, new).slot.wing, row(&rows, LEAD).slot.wing);
    assert_eq!(row(&rows, new).aircraft_kills, 0);

    // Retired for room: the plane leaves the roster, the row stays, with
    // what it did.
    world.retire_plane(LEAD).unwrap();
    assert!(world.roster.plane(LEAD).is_none());
    let rows = results(&world);
    assert_eq!(rows.len(), 5);
    let wreck = row(&rows, LEAD);
    assert_eq!(wreck.status, RowStatus::Retired);
    assert_eq!(wreck.damage, 1.);
    assert_eq!(wreck.aircraft, AircraftId::F18);
    assert_eq!(wreck.aircraft_kills, 1);
    assert_eq!((wreck.gun.launched, wreck.gun.hit), (6, 3));
    // Its place in the order: friendly wing 1, member 0, ahead of the rest.
    assert_eq!(rows[0].plane, LEAD);
}

#[test]
fn the_same_ledger_gives_the_same_rows_twice() {
    let mut world = seated_mission();
    shoot(&mut world, ENEMY, ShotKind::Gun, 9, 2);
    world
        .combat
        .state
        .ledger
        .damaged(credit(ENEMY, WINGMAN.0, 0x8000, true));
    let wings = world.ai_wings.as_mut().unwrap().mission_mut();
    wings.actor_mut(WINGMAN.0).unwrap().flight_mut().crashed = true;
    let rows = results(&world);
    assert_eq!(rows, results(&world));
    assert_eq!(row(&rows, WINGMAN).status, RowStatus::Dead);
    assert_eq!(row(&rows, ENEMY).aircraft_kills, 1);
}
