//! The cockpit readout's coding: its numbers stand for the readout, a
//! record rebuilds what the host says the client holds whatever packets were
//! lost, and a full share brings the client up to the host's readout.

use super::connection::{ClientConnection, HostConnection};
use super::readout::{QReadout, ReadoutReceiver, ReadoutSender};
use super::samples;
use super::snapshot::SnapshotSection;
use std::collections::HashMap;
use tore_formats::aircraft::AircraftId;
use tore_net::SplitMix64;
use tore_world::combat::launcher;
use tore_world::mission::{MissionSpec, Skill, Start};
use tore_world::readout::CockpitReadout;
use tore_world::seats::{SeatId, SeatInput};
use tore_world::test_support::resources::{THEATER, resources};
use tore_world::world::{Seating, TickOutput, World};

#[test]
fn the_numbers_stand_for_the_readout() {
    let readout = samples::readout();
    let q = QReadout::of(&readout, 400);
    let back = q.readout(400, None, None).unwrap();
    // Rounding twice changes nothing.
    assert_eq!(QReadout::of(&back, 400), q);
    assert_eq!(back.plane, 3);
    assert_eq!(back.tick, 401);
    assert_eq!(back.stores, readout.stores);
    assert_eq!(back.damage, readout.damage);
    assert_eq!(back.targets, readout.targets);
    assert_eq!(back.seeker, readout.seeker);
    assert_eq!(back.target_window, readout.target_window);
    // The music's enemy is a scope position: whole feet.
    assert_eq!(back.music.aiming, readout.music.aiming);
    let (id, at) = back.music.designated_enemy.unwrap();
    assert_eq!(id, 7);
    assert!((0..3).all(|i| (at[i] - readout.music.designated_enemy.unwrap().1[i]).abs() <= 0.5));
    assert_eq!(back.rwr.missiles, readout.rwr.missiles);
    assert_eq!(back.sensors.trail(7), readout.sensors.trail(7));
    assert_eq!(back.sensors.tick, 399);
    let contact = back.sensors.contact(7).unwrap();
    assert_eq!(contact.position, readout.sensors.contacts[0].position);
    assert_eq!(contact.velocity, readout.sensors.contacts[0].velocity);
    assert_eq!(back.map[0].aircraft, Some(AircraftId::Mig29));
}

#[test]
fn a_connection_carries_the_readout() {
    let mut host = HostConnection::new(4);
    let mut client = ClientConnection::new(4);
    let readout = samples::readout();
    let packet = host
        .snapshot_with_readout(&samples::header(400), &[], Some(&readout), 0)
        .unwrap();
    host.sent(1);
    let (_, received) = client.snapshot(&packet.snapshot).unwrap();
    assert_eq!(received.readout, Some(QReadout::of(&readout, 400)));
    host.delivered(1);
    // Against the first, a readout whose things fly on as predicted costs
    // little; here they stand still, so their positions are corrected.
    let mut moved = readout.clone();
    moved.tick += 4;
    let again = host
        .snapshot_with_readout(&samples::header(404), &[], Some(&moved), 0)
        .unwrap();
    let report = again.readout.unwrap();
    let first = packet.readout.unwrap();
    assert!(report.bits * 2 < first.bits, "{report:?} against {first:?}");
    let (_, received) = client.snapshot(&again.snapshot).unwrap();
    assert_eq!(received.readout, Some(QReadout::of(&moved, 404)));
}

/// A small fight in the synthetic theater, flown by seat 0.
fn fight() -> World {
    let map = resources();
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 2;
    spec.wings[3].count = 2;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 2;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    World::new(&spec, &map, Seating::SinglePlayer).unwrap()
}

fn readout_of(world: &World) -> CockpitReadout {
    world
        .cockpit_readout(SeatId(0), launcher(&world.cockpits[0].flight))
        .unwrap()
}

#[test]
fn readouts_rebuild_what_the_host_says_whatever_was_lost() {
    let mut world = fight();
    let scene = world.terrain.airport_scene.clone();
    let mut rng = SplitMix64::new(16);
    let mut host = ReadoutSender::new(4);
    let mut client = ReadoutReceiver::new(4);
    let mut out = TickOutput::default();
    let mut held: HashMap<u32, QReadout> = HashMap::new();
    let mut flights: Vec<(u32, u16, Vec<u8>, u32)> = Vec::new();
    let mut notices: Vec<(u32, u16, bool)> = Vec::new();
    let mut seen_lists = 0;
    for k in 0..900u32 {
        for _ in 0..4 {
            let input = SeatInput {
                tick: world.tick(),
                pilot: tore_sim::flight::PilotInput {
                    roll: if (world.tick() / 600).is_multiple_of(2) {
                        0.3
                    } else {
                        -0.3
                    },
                    pitch: 0.1,
                    ..Default::default()
                },
                trigger: world.tick() % 300 < 60,
                ..SeatInput::default()
            };
            world.step(&[input], &mut out).unwrap();
        }
        let tick = 400 + 4 * k;
        let readout = readout_of(&world);
        seen_lists = seen_lists
            .max(readout.sensors.contacts.len() + readout.visual.len() + readout.map.len());
        let q = QReadout::of(&readout, tick);
        // The share is sometimes tight.
        let budget = if rng.below(5) == 0 { 400 } else { 8 * 1_000 };
        let (bits, report) = host.build(tick, &q, budget);
        assert!(report.bits <= budget.max(200), "{report:?}");
        let sequence = k as u16;
        host.sent(sequence);
        let after = host.staged_for_tests(sequence).clone();
        held.insert(tick, after.clone());
        if budget > 400 && report.waiting == 0 {
            assert_eq!(after, q, "snapshot {k}: a full share leaves nothing behind");
        }
        // The whole section, so the record parses in its place.
        let section =
            super::snapshot::write_section(&samples::header(tick), Some(&bits), &[]).unwrap();
        if rng.below(100) >= 10 {
            for _ in 0..if rng.below(100) < 5 { 2 } else { 1 } {
                flights.push((k + 1 + rng.below(4) as u32, sequence, section.clone(), tick));
            }
        } else {
            notices.push((k + 34, sequence, false));
        }
        let (arriving, waiting): (Vec<_>, Vec<_>) = flights.drain(..).partition(|f| f.0 <= k);
        flights = waiting;
        let mut seen = Vec::new();
        for (_, sequence, bytes, tick) in arriving {
            if seen.contains(&sequence) {
                continue;
            }
            seen.push(sequence);
            let section = SnapshotSection::decode(&bytes).unwrap();
            let rebuilt = client
                .receive(section.readout.as_ref().unwrap(), tick)
                .unwrap();
            assert_eq!(
                rebuilt, held[&tick],
                "tick {tick}: the client holds what the host says"
            );
            let full = rebuilt.readout(tick, None, Some(&scene)).unwrap();
            if rebuilt.complete() {
                assert_eq!(QReadout::of(&full, tick), rebuilt);
            }
            let lost_ack = rng.below(100) < 5;
            notices.push((
                if lost_ack {
                    k + 34
                } else {
                    k + 1 + rng.below(3) as u32
                },
                sequence,
                !lost_ack,
            ));
        }
        let (due, later): (Vec<_>, Vec<_>) = notices.drain(..).partition(|n| n.0 <= k);
        notices = later;
        for (_, sequence, delivered) in due {
            if delivered {
                host.delivered(sequence);
            } else {
                host.lost(sequence);
            }
        }
    }
    assert!(seen_lists > 0, "the fight shows on the scope and the map");
}

#[test]
fn a_busy_readout_stays_within_the_packet() {
    use super::entity::{Entity, EntityKind};
    use super::priority::Relevance;
    use super::round_trip_tests::entity;
    let mut rng = SplitMix64::new(17);
    let mut readout = samples::readout();
    // Standing still, so that once across they stay as predicted.
    let template = tore_sim::sensors::Contact {
        velocity: [0.; 3],
        ..readout.sensors.contacts[0]
    };
    readout.sensors.contacts = (0..128)
        .map(|i| tore_sim::sensors::Contact {
            id: 100 + i,
            position: [f64::from(i) * 1_000., 9_000., 5_000.],
            ..template
        })
        .collect();
    readout.map = (0..512)
        .map(|i| tore_world::readout::MapRow {
            contact: tore_sim::sensors::Contact {
                id: 1_000 + i,
                position: [f64::from(i) * 300., 9_000., -5_000.],
                ..template
            },
            identified: i % 3 == 0,
            airborne: true,
            aircraft: None,
        })
        .collect();
    let entities: Vec<(Entity, Relevance)> = (1..=30)
        .map(|id| (entity(&mut rng, EntityKind::Aircraft, id), Relevance::NEAR))
        .collect();
    for messages in [0, 256] {
        let mut host = HostConnection::new(4);
        let mut client = ClientConnection::new(4);
        for tick in 0..20 {
            host.event(
                tick,
                &super::events::WireEvent::Message {
                    text: "z".repeat(120),
                },
            )
            .unwrap();
        }
        for k in 0..30u32 {
            let tick = 400 + 4 * k;
            let packet = host
                .snapshot_with_readout(&samples::header(tick), &entities, Some(&readout), messages)
                .unwrap();
            assert!(
                packet.bytes() <= tore_net::MAX_DATAGRAM,
                "{}",
                packet.bytes()
            );
            assert!(packet.shares.entities >= super::space::ENTITIES_MIN);
            host.sent(k as u16);
            let (_, received) = client.snapshot(&packet.snapshot).unwrap();
            assert!(received.readout.is_some());
            host.delivered(k as u16);
        }
        // Thirty packets bring the whole scope and map across.
        let (_, latest) = client.readout.latest().unwrap();
        assert_eq!(*latest, QReadout::of(&readout, latest_tick(&client)));
    }
}

fn latest_tick(client: &ClientConnection) -> u32 {
    client.readout.latest().unwrap().0
}
