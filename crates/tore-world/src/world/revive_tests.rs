//! Death and revival in the mission core (slice F2-V; docs/ARCHITECTURE.md,
//! "Death, revival and lives"), on an open mission built from synthetic
//! resources as a host builds it: Abandon keeps a wreck falling and lets
//! nobody take it; Revive puts a new plane of the same aircraft in the
//! seat's wing at the spawn, with its stores under each weapons rule, by the
//! handoff; a client's copy adds the same plane; a mission full of wrecks
//! retires the oldest that has rested 30 seconds, so a hundred revivals stay
//! within 64 planes; and the same revivals step the same way twice.

use super::*;
use crate::{
    mission::{MissionSpec, Skill, Start},
    seats::{Pilot, PlaneId, SeatId, SeatInput},
    test_support::resources::{THEATER, resources},
    world::{Cue, MissionCommand, Seating, TickOutput},
};
use tore_formats::aircraft::AircraftId;
use tore_sim::{
    ai::{launch::WingId, weapon_service::Rounds},
    combat::{ledger::ShotKind, live::is_gun},
};

const NM: f64 = FEET_PER_NAUTICAL_MILE;
/// The open mission's planes: Friendly Wing 1 is planes 0 and 1, the
/// enemy's Wing 1 planes 2 and 3.
const LEAD: PlaneId = PlaneId(0);
const WINGMAN: PlaneId = PlaneId(1);
const ENEMY: PlaneId = PlaneId(2);
const FRIENDLY_WING: WingId = WingId {
    side: Side::Friendly,
    index: 0,
};

/// Two friendly and two enemy F/A-18s, the enemy `separation_nm` ahead,
/// airborne at 10,000 feet, every plane on the AI.
fn open_mission(separation_nm: u32) -> World {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 2;
    spec.wings[3].count = 2;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = separation_nm;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    World::new(&spec, &resources(), Seating::Open).unwrap()
}

/// The open mission with seat 0 in plane 0 and seat 1 in plane 2.
fn seated_mission() -> World {
    let mut world = open_mission(20);
    world.take_plane(SeatId(0), LEAD).unwrap();
    world.take_plane(SeatId(1), ENEMY).unwrap();
    world
}

/// One tick with `commands` and neutral input for every seat that flies a
/// plane once they apply.
fn step(world: &mut World, commands: &[MissionCommand]) -> TickOutput {
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
    out
}

fn fly(world: &mut World, ticks: usize) {
    for _ in 0..ticks {
        step(world, &[]);
    }
}

fn cockpit(world: &World, plane: PlaneId) -> &super::super::Cockpit {
    world
        .cockpits
        .iter()
        .find(|c| c.plane == plane)
        .expect("a cockpit")
}

fn cockpit_mut(world: &mut World, plane: PlaneId) -> &mut super::super::Cockpit {
    world
        .cockpits
        .iter_mut()
        .find(|c| c.plane == plane)
        .expect("a cockpit")
}

/// The pilot of `plane` is killed: the plane is lost.
fn kill_pilot(world: &mut World, plane: PlaneId) {
    cockpit_mut(world, plane).flight.systems.pilot.dead = true;
}

/// `plane` is destroyed in the air: its wreck falls.
fn destroy(world: &mut World, plane: PlaneId) {
    world.combat.state.ownship_mut(plane.0).unwrap().hp = 0;
}

/// Every plane where the roster says it is: the AI's with an AI actor and a
/// combat row, a human's or an abandoned one's with a cockpit and an
/// ownship; one plane per seat; at most 64 planes.
fn assert_consistent(world: &World) {
    let wings = world.ai_wings.as_ref().unwrap();
    for plane in world.roster.planes() {
        let actor = wings.mission().actor(plane.id.0).is_some();
        let row = world
            .combat
            .state
            .targets
            .iter()
            .any(|t| t.id == plane.id.0);
        let cockpit = world.cockpits.iter().any(|c| c.plane == plane.id);
        let ownship = world.combat.state.ownship(plane.id.0).is_some();
        match plane.pilot {
            Pilot::Ai => assert!(actor && row && !cockpit && !ownship, "AI plane {:?}", plane),
            Pilot::Human(_) | Pilot::Lost => {
                assert!(!actor && !row && cockpit && ownship, "plane {:?}", plane)
            }
        }
    }
    assert_eq!(world.cockpits.len(), world.combat.state.ownships().len());
    for seat in world.roster.seats() {
        if let Some(plane) = seat.plane {
            assert_eq!(world.roster.seat_of(plane), Some(seat.id));
        }
    }
    assert!(world.roster.planes().len() <= MAX_PLANES);
}

/// What the ownship of `plane` carries on each station.
fn ammo(world: &World, plane: PlaneId) -> Vec<u16> {
    world
        .combat
        .state
        .ownship(plane.0)
        .unwrap()
        .ammo
        .iter()
        .map(|a| a & 0x7fff)
        .collect()
}

#[test]
fn the_revival_point_lies_off_the_battle_towards_the_sides_start() {
    // Two friendlies 30 nm from the enemy pair are not fighting; the enemy
    // pair 5 nm from a third friendly are, so only those three count.
    let aircraft = [
        (Side::Friendly, [0., 9000., -30. * NM]),
        (Side::Friendly, [0., 9000., -31. * NM]),
        (Side::Friendly, [0., 9000., 5. * NM]),
        (Side::Enemy, [0., 9000., 10. * NM]),
        (Side::Enemy, [0., 9000., 10. * NM]),
    ];
    let start = [0., 10_000., -40. * NM];
    let (position, heading) = point(&aircraft, start, 10. * NM, 12_000.);
    let centre = 25. * NM / 3.;
    assert!((position[0]).abs() < 1e-6);
    assert!((position[2] - (centre - 10. * NM)).abs() < 1e-6);
    assert_eq!(position[1], 12_000.);
    // Heading for the centre: north (+z).
    let forward = Basis::new(heading, 0., 0.).forward;
    assert!((forward[2] - 1.).abs() < 1e-9 && forward[0].abs() < 1e-9);
    // With nobody fighting, every living aircraft counts.
    let calm = [
        (Side::Friendly, [10. * NM, 0., 0.]),
        (Side::Enemy, [50. * NM, 0., 0.]),
    ];
    let (position, heading) = point(&calm, [60. * NM, 0., 0.], 5. * NM, 10_000.);
    assert!((position[0] - 35. * NM).abs() < 1e-6 && position[2].abs() < 1e-6);
    let forward = Basis::new(heading, 0., 0.).forward;
    assert!(
        (forward[0] + 1.).abs() < 1e-9,
        "heading back west to the centre"
    );
    // With nobody alive the start is the centre; a start on the centre
    // places the plane south of it, heading north.
    let (position, heading) = point(&[], [100., 0., 100.], 5. * NM, 10_000.);
    assert!((position[0] - 100.).abs() < 1e-6);
    assert!((position[2] - (100. - 5. * NM)).abs() < 1e-6);
    assert!(heading.abs() < 1e-12);
}

#[test]
fn abandon_keeps_a_wreck_falling_that_nobody_can_take() {
    let mut world = seated_mission();
    fly(&mut world, 30);
    // A plane that is not lost cannot be abandoned, and the refusal changes
    // nothing.
    assert!(world.can_abandon(SeatId(0)).is_err());
    let tick = world.tick();
    let mut out = TickOutput::default();
    assert!(
        world
            .step_with(
                &[MissionCommand::Abandon { seat: SeatId(0) }],
                &[],
                &mut out,
                |_, _| Ok(())
            )
            .is_err()
    );
    assert_eq!(world.tick(), tick);
    assert_eq!(world.roster.seat(SeatId(0)).unwrap().plane, Some(LEAD));
    // Shot down in the air: the plane is lost and its wreck falls.
    destroy(&mut world, LEAD);
    fly(&mut world, 1);
    assert!(world.plane_lost(LEAD));
    assert!(world.can_give_back(SeatId(0)).is_err());
    let out = step(&mut world, &[MissionCommand::Abandon { seat: SeatId(0) }]);
    assert!(out.cues.iter().all(|cue| !matches!(cue,
        Cue::Message { seat, .. } if *seat == SeatId(0))));
    assert_eq!(world.roster.plane(LEAD).unwrap().pilot, Pilot::Lost);
    assert_eq!(world.roster.seat(SeatId(0)).unwrap().plane, None);
    assert_eq!(world.roster.seat_of(LEAD), None);
    assert_eq!(world.revival.lost().len(), 1);
    assert_eq!(world.revival.lost()[0].plane, LEAD);
    assert_consistent(&world);
    // Nobody can take it, its own seat included.
    assert!(world.can_take(SeatId(0), LEAD).is_err());
    assert!(world.can_take(SeatId(7), LEAD).is_err());
    // The wreck keeps falling with nobody's controls, and whatever it does
    // is said to no seat.
    let high = cockpit(&world, LEAD).flight.position[1];
    let mut said = Vec::new();
    for _ in 0..240 {
        let out = step(&mut world, &[]);
        said.extend(out.cues.into_iter().filter_map(|cue| match cue {
            Cue::Message {
                seat: SeatId(0),
                text,
            } => Some(text),
            _ => None,
        }));
    }
    let low = cockpit(&world, LEAD).flight.position[1];
    assert!(
        low < high - 100.,
        "the wreck fell from {high} ft to {low} ft"
    );
    assert!(said.is_empty(), "the old seat heard the wreck: {said:?}");
    // The seat waits; it can take a free AI plane.
    world.can_take(SeatId(0), WINGMAN).unwrap();
    assert_consistent(&world);
}

#[test]
fn a_revived_plane_takes_the_spawn_in_the_old_planes_wing_by_the_handoff() {
    let mut world = seated_mission();
    let start = world.side_mean(Side::Friendly).unwrap();
    fly(&mut world, 30);
    kill_pilot(&mut world, LEAD);
    fly(&mut world, 1);
    // The spawn: the point from the living aircraft, at the mission's
    // altitude and the aircraft's airborne start speed, the standard load
    // with full fuel.
    let spawn = world
        .revival_spawn(SeatId(0), start, 10. * NM, None, RevivalWeapons::Missiles)
        .unwrap();
    let (position, heading) = point(&world.living_aircraft(), start, 10. * NM, 10_000.);
    assert_eq!(
        [spawn.position[0], spawn.position[2]],
        [position[0], position[2]]
    );
    assert!(spawn.position[1] >= 10_000.);
    assert_eq!(spawn.heading_rad, heading);
    assert!(spawn.speed_fps > 300., "{} ft/s", spawn.speed_fps);
    let living = world.living_aircraft();
    assert_eq!(living.len(), 3, "the lost plane is not in the battle");
    let new = world.revival_plane(SeatId(0), &spawn).unwrap();
    assert_eq!(new.plane, PlaneId(4));
    assert_eq!(
        new.slot,
        Slot {
            wing: FRIENDLY_WING,
            member: 2
        }
    );
    assert_eq!(new.aircraft, AircraftId::F18);
    step(
        &mut world,
        &[MissionCommand::Revive {
            seat: SeatId(0),
            spawn: Box::new(spawn.clone()),
        }],
    );
    // The seat flies the new plane; the old one is abandoned.
    assert_eq!(
        world.roster.seat(SeatId(0)).unwrap().plane,
        Some(PlaneId(4))
    );
    assert_eq!(
        world.roster.plane(PlaneId(4)).unwrap().pilot,
        Pilot::Human(SeatId(0))
    );
    assert_eq!(world.roster.plane(PlaneId(4)).unwrap().slot, new.slot);
    assert_eq!(world.roster.plane(LEAD).unwrap().pilot, Pilot::Lost);
    assert_consistent(&world);
    // At the spawn's place, heading and speed, one tick on.
    let flight = &cockpit(&world, PlaneId(4)).flight;
    let moved =
        (flight.position[0] - spawn.position[0]).hypot(flight.position[2] - spawn.position[2]);
    assert!(moved < 3. * spawn.speed_fps / 120., "{moved} ft");
    let turned = (flight.yaw - spawn.heading_rad).rem_euclid(std::f64::consts::TAU);
    assert!(
        turned.min(std::f64::consts::TAU - turned) < 0.01,
        "{} against {}",
        flight.yaw,
        spawn.heading_rad
    );
    assert!((flight.speed - spawn.speed_fps).abs() < 0.05 * spawn.speed_fps);
    // Full fuel, less a tick's burn, and the standard stores.
    let full = world.full_fuel(AircraftId::F18).unwrap();
    assert!(
        flight.fuel > full - 10. && flight.fuel <= full,
        "{}",
        flight.fuel
    );
    let quantities: Vec<u16> = spawn.loadout.stations.iter().map(|s| s.quantity).collect();
    assert_eq!(ammo(&world, PlaneId(4)), quantities);
    assert!(!world.plane_lost(PlaneId(4)));
    // The next revival takes the next id and member.
    kill_pilot(&mut world, PlaneId(4));
    fly(&mut world, 1);
    let spawn = world
        .revival_spawn(SeatId(0), start, 10. * NM, None, RevivalWeapons::Guns)
        .unwrap();
    let next = world.revival_plane(SeatId(0), &spawn).unwrap();
    assert_eq!((next.plane, next.slot.member), (PlaneId(5), 3));
}

#[test]
fn each_weapons_rule_cuts_the_revived_stores() {
    let mut world = seated_mission();
    let start = world.side_mean(Side::Friendly).unwrap();
    kill_pilot(&mut world, LEAD);
    fly(&mut world, 1);
    let config = world
        .standard_configuration(AircraftId::F18)
        .unwrap()
        .clone();
    let gun = |i: usize| is_gun(&config.stations[i].weapon);
    let air = |i: usize| ShotKind::of(&config.stations[i].weapon) == ShotKind::AirToAir;
    // The synthetic F/A-18 carries a gun and a store that is not one (its
    // record is no air-to-air missile by the debrief's rule: the next test
    // has those).
    let stations = config.stations.len();
    assert!((0..stations).any(gun));
    assert!((0..stations).any(|i| !gun(i)));
    let standard: Vec<u16> = config.stations.iter().map(|s| s.count).collect();
    for rule in RevivalWeapons::ALL {
        let spawn = world
            .revival_spawn(SeatId(0), start, 5. * NM, None, rule)
            .unwrap();
        let got: Vec<u16> = spawn.loadout.stations.iter().map(|s| s.quantity).collect();
        let expected: Vec<u16> = (0..stations)
            .map(|i| match rule {
                RevivalWeapons::Missiles => standard[i],
                RevivalWeapons::NoMissiles if air(i) => 0,
                RevivalWeapons::NoMissiles => standard[i],
                RevivalWeapons::Guns if gun(i) => standard[i],
                RevivalWeapons::HalfGuns if gun(i) => standard[i].div_ceil(2),
                RevivalWeapons::Guns | RevivalWeapons::HalfGuns => 0,
            })
            .collect();
        assert_eq!(got, expected, "{rule:?}");
        assert_eq!(
            spawn.loadout.fuel_lbs,
            world.full_fuel(AircraftId::F18).unwrap()
        );
    }
    // A revived plane carries what its spawn says.
    let spawn = world
        .revival_spawn(SeatId(0), start, 5. * NM, None, RevivalWeapons::HalfGuns)
        .unwrap();
    step(
        &mut world,
        &[MissionCommand::Revive {
            seat: SeatId(0),
            spawn: Box::new(spawn.clone()),
        }],
    );
    let quantities: Vec<u16> = spawn.loadout.stations.iter().map(|s| s.quantity).collect();
    assert_eq!(ammo(&world, PlaneId(4)), quantities);
    // The player's own loadout is the base when it is given: one station
    // emptied stays empty, and the rule cuts the rest.
    let mut chosen = world
        .revival_loadout(AircraftId::F18, None, RevivalWeapons::Missiles)
        .unwrap();
    let emptied = (0..stations).find(|i| gun(*i)).unwrap();
    chosen.stations[emptied].quantity = 0;
    let load = world
        .revival_loadout(AircraftId::F18, Some(&chosen), RevivalWeapons::HalfGuns)
        .unwrap();
    assert_eq!(load.stations[emptied].quantity, 0);
    // A loadout naming a weapon the mission never carried is refused.
    chosen.stations[emptied].weapon = "NOSUCH.JT".into();
    assert!(
        world
            .revival_loadout(AircraftId::F18, Some(&chosen), RevivalWeapons::Missiles)
            .is_err()
    );
}

#[test]
fn the_rules_tell_the_gun_air_to_air_missiles_and_other_stores_apart() {
    let world = open_mission(20);
    let config = world.standard_configuration(AircraftId::F18).unwrap();
    let gun = config
        .stations
        .iter()
        .find(|s| is_gun(&s.weapon))
        .unwrap()
        .weapon
        .clone();
    let mut missile = config
        .stations
        .iter()
        .find(|s| !is_gun(&s.weapon))
        .unwrap()
        .weapon
        .clone();
    // Guided, against aircraft: an air-to-air missile.
    missile.flags = 1 | 0x10000;
    let mut maverick = missile.clone();
    maverick.flags = 1 | 0x20000;
    let mut bomb = missile.clone();
    bomb.flags = 0x10;
    use RevivalWeapons::*;
    for (rule, expected) in [
        (Missiles, [501, 4, 4, 4]),
        (NoMissiles, [501, 0, 4, 4]),
        (Guns, [501, 0, 0, 0]),
        (HalfGuns, [251, 0, 0, 0]),
    ] {
        let got = [
            rule.cut(&gun, 501),
            rule.cut(&missile, 4),
            rule.cut(&maverick, 4),
            rule.cut(&bomb, 4),
        ];
        assert_eq!(got, expected, "{rule:?}");
    }
    assert_eq!(HalfGuns.cut(&gun, 1), 1);
    assert_eq!(HalfGuns.cut(&gun, 0), 0);
}

#[test]
fn the_ai_slot_rule_cuts_an_ai_aircrafts_stores_before_the_take() {
    let mut world = seated_mission();
    let config = world
        .ai_wings
        .as_ref()
        .unwrap()
        .configuration(WINGMAN.0)
        .unwrap()
        .clone();
    world.cut_ai_stores(WINGMAN, RevivalWeapons::Guns).unwrap();
    world.take_plane(SeatId(5), WINGMAN).unwrap();
    for (station, carried) in config.stations.iter().zip(ammo(&world, WINGMAN)) {
        if is_gun(&station.weapon) {
            assert!(carried > 0);
        } else {
            assert_eq!(carried, 0, "{}", station.weapon.source);
        }
    }
    assert_consistent(&world);
}

#[test]
fn a_clients_copy_adds_the_same_plane_from_the_spawned_message() {
    let mut world = seated_mission();
    let start = world.side_mean(Side::Friendly).unwrap();
    kill_pilot(&mut world, LEAD);
    fly(&mut world, 1);
    let spawn = world
        .revival_spawn(SeatId(0), start, 20. * NM, None, RevivalWeapons::NoMissiles)
        .unwrap();
    let new = world.revival_plane(SeatId(0), &spawn).unwrap();
    step(
        &mut world,
        &[MissionCommand::Revive {
            seat: SeatId(0),
            spawn: Box::new(spawn.clone()),
        }],
    );
    // A client's copy: the mission as built, never stepped, all on the AI.
    let mut copy = open_mission(20);
    copy.add_plane(&new).unwrap();
    let entry = copy.roster.plane(new.plane).unwrap();
    assert_eq!((entry.slot, entry.pilot), (new.slot, Pilot::Ai));
    let wings = copy.ai_wings.as_ref().unwrap();
    let slot = wings.slot(new.plane.0).unwrap();
    assert_eq!(
        (
            slot.aircraft,
            slot.side,
            slot.wing_number,
            slot.member_number
        ),
        (AircraftId::F18, Side::Friendly, 1, 3)
    );
    // Its configuration and stores are the host's: what Seated reads.
    let config = wings.configuration(new.plane.0).unwrap();
    let host = world
        .combat
        .state
        .ownship(new.plane.0)
        .unwrap()
        .configuration();
    let names = |c: &live::Configuration| -> Vec<String> {
        c.stations.iter().map(|s| s.weapon.source.clone()).collect()
    };
    assert_eq!(names(config), names(host));
    let rounds: Vec<Rounds> = wings
        .mission()
        .actor(new.plane.0)
        .unwrap()
        .stations()
        .iter()
        .map(|s| s.store.rounds)
        .collect();
    let quantities: Vec<Rounds> = spawn
        .loadout
        .stations
        .iter()
        .map(|s| Rounds::Finite(u32::from(s.quantity)))
        .collect();
    assert_eq!(rounds, quantities);
    let row = copy
        .combat
        .state
        .targets
        .iter()
        .find(|t| t.id == new.plane.0)
        .unwrap();
    assert_eq!(row.position, spawn.position);
    assert_eq!(row.side, FRIENDLY_SIDE);
    // The same message again is refused, changing nothing.
    assert!(copy.add_plane(&new).is_err());
    assert_eq!(copy.roster.planes().len(), 5);
}

#[test]
fn a_wreck_is_retired_only_after_resting_thirty_seconds() {
    let mut world = seated_mission();
    // Lost: crashed where it flies, so its wreck falls and then rests.
    cockpit_mut(&mut world, LEAD).flight.crashed = true;
    fly(&mut world, 1);
    step(&mut world, &[MissionCommand::Abandon { seat: SeatId(0) }]);
    // It falls first, then rests.
    let mut rested_at = None;
    for _ in 0..(RETIRE_AFTER_TICKS as usize + 2400) {
        step(&mut world, &[]);
        if rested_at.is_none() {
            rested_at = world.revival.lost()[0].resting_since;
        }
        let Some(since) = rested_at else {
            assert_eq!(world.retirable(), None);
            continue;
        };
        let rested = world.tick() - since;
        assert_eq!(
            world.retirable().is_some(),
            rested >= RETIRE_AFTER_TICKS,
            "rested {rested} ticks"
        );
        if rested > RETIRE_AFTER_TICKS {
            break;
        }
    }
    assert!(rested_at.is_some(), "the wreck never came to rest");
    world.retire_plane(LEAD).unwrap();
    assert!(world.roster.plane(LEAD).is_none());
    assert_eq!(world.revival.retired().len(), 1);
    assert_eq!(world.revival.retired()[0].id, LEAD);
    assert!(world.revival.lost().is_empty());
    assert!(world.combat.state.ownship(LEAD.0).is_none());
    assert_consistent(&world);
    // A plane that is not an abandoned wreck is never retired.
    assert!(world.retire_plane(ENEMY).is_err());
    assert!(world.retire_plane(WINGMAN).is_err());
    fly(&mut world, 10);
    // The retired plane's id is never given again.
    cockpit_mut(&mut world, ENEMY).flight.systems.pilot.dead = true;
    fly(&mut world, 1);
    let start = world.side_mean(Side::Enemy).unwrap();
    let spawn = world
        .revival_spawn(SeatId(1), start, 5. * NM, None, RevivalWeapons::Missiles)
        .unwrap();
    assert_eq!(
        world.revival_plane(SeatId(1), &spawn).unwrap().plane,
        PlaneId(4)
    );
}

#[test]
fn a_hundred_revivals_stay_within_64_planes() {
    let mut world = seated_mission();
    let start = world.side_mean(Side::Friendly).unwrap();
    // Long enough for a wreck to have rested 30 seconds.
    fly(&mut world, RETIRE_AFTER_TICKS as usize + 1);
    let mut highest = 0;
    for revival in 0..100 {
        let plane = world.roster.seat(SeatId(0)).unwrap().plane.unwrap();
        cockpit_mut(&mut world, plane).flight.crashed = true;
        fly(&mut world, 1);
        // Every wreck so far rested long ago (the test's shortcut for the
        // 30 seconds the previous test flies).
        for lost in &mut world.revival.lost {
            lost.resting_since = Some(0);
        }
        let spawn = world
            .revival_spawn(SeatId(0), start, 10. * NM, None, RevivalWeapons::Missiles)
            .unwrap();
        world.can_revive(SeatId(0), &spawn).unwrap();
        step(
            &mut world,
            &[MissionCommand::Revive {
                seat: SeatId(0),
                spawn: Box::new(spawn),
            }],
        );
        let now = world.roster.seat(SeatId(0)).unwrap().plane.unwrap();
        assert_eq!(now.0, 4 + revival, "ids never repeat");
        highest = highest.max(world.roster.planes().len());
        assert!(world.roster.planes().len() <= MAX_PLANES);
        assert_consistent(&world);
    }
    assert_eq!(highest, MAX_PLANES);
    // Retired oldest first: the first wrecks went.
    let retired: Vec<u32> = world.revival.retired().iter().map(|p| p.id.0).collect();
    assert_eq!(retired.len(), 100 + 4 - MAX_PLANES);
    assert_eq!(retired[0], LEAD.0);
    assert!(retired.windows(2).all(|pair| pair[0] < pair[1]));
    // With no wreck rested, a full mission has no room.
    let plane = world.roster.seat(SeatId(0)).unwrap().plane.unwrap();
    cockpit_mut(&mut world, plane).flight.crashed = true;
    fly(&mut world, 1);
    for lost in &mut world.revival.lost {
        lost.resting_since = None;
    }
    assert!(!world.room_for_one());
    let spawn = world
        .revival_spawn(SeatId(0), start, 10. * NM, None, RevivalWeapons::Missiles)
        .unwrap();
    assert!(world.can_revive(SeatId(0), &spawn).is_err());
}

/// The mission at one moment, to the bit: every cockpit's flight, every
/// ownship and every aircraft row.
fn digest(world: &World) -> String {
    let mut text = format!("tick {}\n", world.tick());
    for c in &world.cockpits {
        let f = &c.flight;
        text += &format!(
            "cockpit {} {:?} {:?} {:?} {}\n",
            c.plane.0,
            f.position,
            f.velocity,
            [f.yaw, f.pitch, f.bank],
            f.fuel
        );
    }
    for o in world.combat.state.ownships() {
        text += &format!("ownship {} {} {:?}\n", o.aircraft, o.hp, o.ammo);
    }
    for t in &world.combat.state.targets {
        text += &format!("row {} {:?} {:?} {}\n", t.id, t.position, t.velocity, t.hp);
    }
    text + &format!("{:?}\n{:?}\n", world.roster, world.revival)
}

#[test]
fn the_same_revivals_step_the_same_way_twice() {
    let run = || {
        let mut world = seated_mission();
        let start = world.side_mean(Side::Friendly).unwrap();
        fly(&mut world, 60);
        destroy(&mut world, LEAD);
        fly(&mut world, 1);
        let spawn = world
            .revival_spawn(SeatId(0), start, 10. * NM, None, RevivalWeapons::NoMissiles)
            .unwrap();
        step(
            &mut world,
            &[MissionCommand::Revive {
                seat: SeatId(0),
                spawn: Box::new(spawn),
            }],
        );
        fly(&mut world, 240);
        digest(&world)
    };
    assert_eq!(run(), run());
}
