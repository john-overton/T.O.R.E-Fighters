//! The cockpit readout for any seat (docs/ARCHITECTURE.md, "The flight screen
//! draws a frame"): each seat of the crowd gets a readout that names its own
//! plane's stores, contacts and damage, and a seat's readout does not change
//! when only another seat's ownship changes. Synthetic fixtures only.

use super::crowd::{E_HUMAN, E_LEAD, F_HUMAN, F_LEAD, HUMANS, crowded_mission, inputs};
use super::*;
use crate::{
    combat::launcher,
    readout::{CockpitReadout, PlainBits},
};
use tore_input::{PilotCommand, PilotInput, Switch};

/// The crowd with every human's radar on, flown `ticks` ticks.
fn flown_crowd(ticks: usize) -> World {
    let mut world = crowded_mission();
    let mut out = TickOutput::default();
    for tick in 0..ticks {
        let step = inputs(&world, |seat| {
            let mut pilot = PilotInput::default();
            if tick == 10 {
                pilot.commands.push(PilotCommand::Set(Switch::Radar, true));
            }
            SeatInput {
                seat,
                pilot,
                ..SeatInput::default()
            }
        });
        world.step(&step, &mut out).unwrap();
    }
    world
}

fn readout_of(world: &World, seat: u8) -> CockpitReadout {
    let seat = SeatId(seat);
    let cockpit = &world.cockpits[world.cockpit_of(seat).unwrap()];
    world
        .cockpit_readout(seat, launcher(&cockpit.flight))
        .unwrap()
}

#[test]
fn a_seats_readout_names_its_own_planes_stores_contacts_and_damage() {
    let mut world = flown_crowd(400);
    // The two friendly planes differ in what they carry and how hurt they are.
    {
        let own = world.combat.state.ownship_mut(F_LEAD.0).unwrap();
        own.hp -= 9;
        own.chaff = 7;
        own.ammo[0] = 123;
    }
    {
        let own = world.combat.state.ownship_mut(F_HUMAN.0).unwrap();
        own.hp -= 30;
        own.flares = 3;
        own.ammo[0] = 77;
        own.subsystem_counts[36] = 1;
    }
    let first = readout_of(&world, 0);
    let second = readout_of(&world, 1);
    let ours = |plane: PlaneId| world.combat.state.ownship(plane.0).unwrap();

    assert_eq!((first.plane, second.plane), (F_LEAD.0, F_HUMAN.0));
    for (readout, plane) in [(&first, F_LEAD), (&second, F_HUMAN)] {
        let own = ours(plane);
        assert_eq!(readout.stores.ammo, own.ammo);
        assert_eq!(readout.stores.selected(), own.selected);
        assert_eq!(readout.damage.hp, own.hp);
        assert_eq!(readout.damage.subsystem_counts, own.subsystem_counts);
        assert_eq!(readout.countermeasures.chaff, own.chaff);
        assert_eq!(readout.countermeasures.flares, own.flares);
        assert_eq!(readout.tick, world.combat.state.tick());
    }
    assert_ne!(first.stores.ammo, second.stores.ammo);
    assert_ne!(first.damage.hp, second.damage.hp);
    assert_eq!(first.countermeasures.chaff, 7);
    assert_eq!(second.countermeasures.flares, 3);
    assert_eq!(second.damage.subsystem_counts[36], 1);
    assert_eq!(first.damage.subsystem_counts[36], 0);

    // Each seat's contacts are the other planes from its own place: never
    // itself, and at the distance from its own plane.
    for (readout, plane) in [(&first, F_LEAD), (&second, F_HUMAN)] {
        let here = world.cockpits[world.cockpit_of(SeatId(plane.0 as u8)).unwrap()]
            .flight
            .position;
        assert!(
            !readout.sensors.contacts.is_empty(),
            "the radar of {plane:?} finds nobody"
        );
        for contact in &readout.sensors.contacts {
            assert_ne!(contact.id, plane.0, "a plane is never its own contact");
            let feet: f64 = (0..3)
                .map(|i| (contact.position[i] - here[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            assert!(
                (feet - contact.distance_ft).abs() < 100.,
                "contact {} of {plane:?} is {feet:.0} ft away, not {:.0}",
                contact.id,
                contact.distance_ft
            );
        }
    }
    let ids = |readout: &CockpitReadout| -> Vec<u32> {
        readout.sensors.contacts.iter().map(|c| c.id).collect()
    };
    assert!(ids(&first).contains(&F_HUMAN.0) || ids(&first).contains(&E_LEAD.0));
    assert_ne!(
        first.sensors.contacts, second.sensors.contacts,
        "each seat sees the others from its own place"
    );
    // The enemy seats have their own readouts too.
    assert_eq!(readout_of(&world, 2).plane, E_LEAD.0);
    assert_eq!(readout_of(&world, 3).plane, E_HUMAN.0);
}

#[test]
fn a_seats_readout_does_not_change_when_only_another_ownship_does() {
    let mut world = flown_crowd(400);
    let before: Vec<CockpitReadout> = (0..4).map(|seat| readout_of(&world, seat)).collect();
    // Only seat 1's plane changes: its stores, damage, countermeasures,
    // selection and sensors.
    {
        let own = world.combat.state.ownship_mut(F_HUMAN.0).unwrap();
        own.hp = 1;
        own.chaff = 0;
        own.flares = 0;
        own.ammo.iter_mut().for_each(|rounds| *rounds = 0);
        own.selected = (own.selected + 1) % own.ammo.len();
        own.radar_failed = true;
        own.shots += 40;
    }
    let after: Vec<CockpitReadout> = (0..4).map(|seat| readout_of(&world, seat)).collect();
    assert_ne!(before[1], after[1], "seat 1's own readout shows the change");
    assert_eq!(after[1].damage.hp, 1);
    for seat in [0, 2, 3] {
        assert_eq!(
            before[seat], after[seat],
            "seat {seat}'s readout changed with seat 1's ownship"
        );
    }
}

#[test]
fn the_frame_carries_the_readout_for_the_flight_it_presents() {
    let world = flown_crowd(200);
    let picture = world.combat.render_snapshot();
    // A frame between ticks builds it for the blended flight.
    let presented = world.presented_flight(SeatId(1), 0.5).unwrap();
    let expected = world
        .cockpit_readout(SeatId(1), launcher(&presented))
        .unwrap();
    let frame = world
        .flight_frame(SeatId(1), Some(presented), picture, &[])
        .unwrap();
    assert_eq!(*frame.readout, expected);
    assert_eq!(frame.readout.plane, F_HUMAN.0);
    assert_eq!(
        frame.config.aircraft,
        world
            .combat
            .state
            .ownship(F_HUMAN.0)
            .unwrap()
            .configuration()
            .aircraft
    );
    assert!(
        world
            .cockpit_readout(SeatId(9), launcher(frame.flight))
            .is_none()
    );
}

/// How big a busy seat's readout is, counted plain (no coding): measured on
/// the crowd at every hundredth tick of a 25-second fight, all four seats.
#[test]
fn a_busy_seats_readout_is_measured() {
    let mut world = crowded_mission();
    let mut out = TickOutput::default();
    let mut total = 0;
    let mut samples = 0;
    // The biggest: bits, tick, seat, contacts, trail points, bits per group.
    type Busiest = (usize, usize, u8, usize, usize, Vec<(&'static str, usize)>);
    let mut busiest: Option<Busiest> = None;
    for tick in 0..3000usize {
        let step = inputs(&world, |seat| {
            let mut pilot = PilotInput::default();
            if tick == 10 {
                pilot.commands.push(PilotCommand::Set(Switch::Radar, true));
            }
            SeatInput {
                seat,
                pilot,
                ..SeatInput::default()
            }
        });
        world.step(&step, &mut out).unwrap();
        if tick % 100 == 99 {
            for seat in 0..4u8 {
                let readout = readout_of(&world, seat);
                let bits = readout.plain_bits();
                total += bits;
                samples += 1;
                if busiest.as_ref().is_none_or(|best| bits > best.0) {
                    busiest = Some((
                        bits,
                        tick + 1,
                        seat,
                        readout.sensors.contacts.len(),
                        readout.sensors.trail_points.len(),
                        readout.group_bits().to_vec(),
                    ));
                }
            }
        }
    }
    let (bits, tick, seat, contacts, trail_points, groups) = busiest.unwrap();
    eprintln!(
        "cockpit readout, plain bits: busiest {bits} bits ({} bytes) at tick {tick} for seat \
         {seat} with {contacts} contacts and {trail_points} trail points; mean {:.0} bytes over \
         {samples} samples",
        bits.div_ceil(8),
        total as f64 / samples as f64 / 8.,
    );
    eprintln!(
        "  by group: {}",
        groups
            .iter()
            .map(|(name, bits)| format!("{name} {}", bits.div_ceil(8)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert!(bits > 0 && groups.iter().map(|(_, b)| b).sum::<usize>() == bits);
    let _ = (E_HUMAN, HUMANS);
}

/// In an open mission plane 0 is the AI's until a seat takes it: the seat has
/// no readout before, and the readout after names the plane it took.
#[test]
fn a_seat_that_takes_a_plane_in_an_open_mission_gets_its_readout() {
    use crate::{
        mission::{MissionSpec, Skill, Start},
        test_support::resources::{THEATER, resources},
    };
    let mut spec = MissionSpec::new(THEATER, tore_formats::aircraft::AircraftId::F18);
    spec.wings[0].count = 2;
    spec.wings[3].count = 2;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 5;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    let mut world = World::new(&spec, &resources(), Seating::Open).unwrap();
    let mut out = TickOutput::default();
    for _ in 0..120 {
        world.step(&[], &mut out).unwrap();
    }
    let lead = world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(0)
        .unwrap()
        .flight()
        .clone();
    assert!(
        world.cockpit_readout(SeatId(0), launcher(&lead)).is_none(),
        "nobody flies, so the seat has no readout"
    );
    let take = MissionCommand::Take {
        seat: SeatId(0),
        plane: PlaneId(0),
    };
    let tick = world.tick();
    let input = SeatInput {
        seat: SeatId(0),
        tick,
        ..SeatInput::default()
    };
    world
        .step_with(&[take], &[input], &mut out, |_, _| Ok(()))
        .unwrap();
    let readout = readout_of(&world, 0);
    assert_eq!(readout.plane, 0);
    let own = world.combat.state.ownship(0).unwrap();
    assert_eq!(readout.stores.ammo, own.ammo);
    assert_eq!(readout.damage.hp, own.hp);
    let frame = world
        .flight_frame(SeatId(0), None, world.combat.render_snapshot(), &[])
        .unwrap();
    assert_eq!(frame.readout.plane, 0);
}

/// Slice T1: a plane with a laser designator reads its sight and its own
/// spot; a friendly plane without one, with a laser weapon selected, reads
/// the spot as a buddy's; the other side reads no laser at all.
#[test]
fn a_lasing_plane_reads_its_own_spot_and_a_friendly_one_reads_it_as_a_buddys() {
    use tore_sim::combat::{
        laser::{Laser, LaserState},
        live::{self, Ownship},
        sight::{FIGHTER_DEFAULT_LOOK, Gimbal, SightSensor},
    };
    let mut world = flown_crowd(10);
    let rebuild = |world: &mut World, plane: PlaneId, change: &dyn Fn(&mut live::Configuration)| {
        let own = world.combat.state.ownship(plane.0).unwrap();
        let mut config = own.configuration().clone();
        change(&mut config);
        let fresh = Ownship::new(plane.0, own.side, config, true).unwrap();
        *world.combat.state.ownship_mut(plane.0).unwrap() = fresh;
    };
    rebuild(&mut world, F_LEAD, &|config| {
        config.sight = Some(SightSensor {
            laser: true,
            range_ft: 60_760.,
            gimbal: Gimbal::Hemisphere,
            eye: [0., -4., 0.],
            default_look: FIGHTER_DEFAULT_LOOK,
        });
    });
    rebuild(&mut world, F_HUMAN, &|config| {
        let weapon = &mut config.stations[0].weapon;
        weapon.seeker.signature = 1;
        weapon.flags |= 0x400;
        weapon.seeker.zones[0].maximum_range = 50_000;
    });
    let spot_at = {
        let cockpit = &world.cockpits[world.cockpit_of(SeatId(1)).unwrap()];
        let p = launcher(&cockpit.flight).position;
        [p[0], p[1] - 9_000., p[2] + 20_000.]
    };
    {
        let lead = world.combat.state.ownship_mut(F_LEAD.0).unwrap();
        lead.laser = Laser {
            state: LaserState::Lasing,
            point: Some(spot_at),
            ..Laser::default()
        };
        let human = world.combat.state.ownship_mut(F_HUMAN.0).unwrap();
        human.armed = true;
        human.selected = 0;
    }
    let lead = readout_of(&world, 0);
    assert!(lead.sight.is_some());
    let laser = lead.laser.unwrap();
    assert!(laser.designator);
    let spot = laser.spot.unwrap();
    assert!(spot.own);
    assert_eq!(spot.spot.position, spot_at);
    let human = readout_of(&world, 1);
    assert!(human.sight.is_none());
    let laser = human.laser.unwrap();
    assert!(!laser.designator);
    let buddy = laser.spot.unwrap();
    assert!(!buddy.own);
    assert_eq!(buddy.spot.owner, F_LEAD.0);
    assert!(laser.buddy.is_none(), "no own spot to show beside it");
    // The enemy reads nothing of the friendly spot.
    assert!(readout_of(&world, 3).laser.is_none());
}
