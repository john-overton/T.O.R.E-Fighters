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
    assert_eq!(frame.readout, expected);
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
