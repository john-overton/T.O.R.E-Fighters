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
use tore_world::test_support::resources::{THEATER, gunship_resources, resources};
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
    let mut expected_stores = readout.stores.clone();
    for (actual, expected) in back.stores.gun_aim.iter().zip(expected_stores.gun_aim) {
        assert!((actual - expected).abs() <= 0.5 / 127. + 1e-12);
    }
    expected_stores.gun_aim = back.stores.gun_aim;
    assert_eq!(back.stores, expected_stores);
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
    // The data link's share (slice G7): the scalars, marks and mates
    // exactly, the tracks in whole feet and quarter feet a second.
    let link = &readout.link;
    assert_eq!(back.link.radar, link.radar);
    assert_eq!(back.link.assigned, link.assigned);
    assert_eq!(back.link.marks, link.marks);
    assert_eq!(back.link.mates, link.mates);
    assert_eq!(back.link.tracks.len(), link.tracks.len());
    for (got, sent) in back.link.tracks.iter().zip(&link.tracks) {
        assert_eq!((got.target, got.source), (sent.target, sent.source));
        assert!((0..3).all(|i| (got.position[i] - sent.position[i]).abs() <= 0.5));
        assert!((0..3).all(|i| (got.velocity[i] - sent.velocity[i]).abs() <= 0.125));
    }
    // No assignment reads back as none; a plane with no radar keeps its link.
    let mut bare = readout.clone();
    bare.link.assigned = None;
    bare.link.radar = false;
    let again = QReadout::of(&bare, 400).readout(400, None, None).unwrap();
    assert_eq!((again.link.assigned, again.link.radar), (None, false));
    assert_eq!(again.link.marks, link.marks);
}

#[test]
fn a_client_lists_link_tracks_nearest_its_own_plane_first() {
    let readout = samples::readout();
    let q = QReadout::of(&readout, 400);
    let basis = tore_sim::attitude::Basis::new(0., 0., 0.);
    // Near track 9 (at 40,000, 12,000, 60,000): it comes first.
    let near_nine = q
        .readout(400, Some(([39_000., 12_000., 59_000.], &basis)), None)
        .unwrap();
    let order: Vec<u32> = near_nine.link.tracks.iter().map(|t| t.target).collect();
    assert_eq!(order, [9, 7]);
    // Without a plane, in target order.
    let plain = q.readout(400, None, None).unwrap();
    let order: Vec<u32> = plain.link.tracks.iter().map(|t| t.target).collect();
    assert_eq!(order, [7, 9]);
}

#[test]
fn a_connection_carries_the_readout() {
    let mut host = HostConnection::new(4);
    let mut client = ClientConnection::for_flight(4, 0);
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

/// Sends `snapshots` readouts, the `k`th made by `next` for the snapshot of
/// tick `400 + 4k`, through a host and a client losing, duplicating and
/// reordering packets and their acknowledgements, and checks after every
/// arrival that the client holds what the host says it holds.
fn lossy(
    seed: u64,
    snapshots: u32,
    scene: Option<&tore_sim::airport::Scene>,
    mut next: impl FnMut(u32, &mut SplitMix64) -> CockpitReadout,
) {
    let mut rng = SplitMix64::new(seed);
    let mut host = ReadoutSender::new(4);
    let mut client = ReadoutReceiver::new(4);
    let mut held: HashMap<u32, QReadout> = HashMap::new();
    let mut flights: Vec<(u32, u16, Vec<u8>, u32)> = Vec::new();
    let mut notices: Vec<(u32, u16, bool)> = Vec::new();
    for k in 0..snapshots {
        let tick = 400 + 4 * k;
        let readout = next(k, &mut rng);
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
            let full = rebuilt.readout(tick, None, scene).unwrap();
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
}

#[test]
fn readouts_rebuild_what_the_host_says_whatever_was_lost() {
    let mut world = fight();
    let scene = world.terrain.airport_scene.clone();
    let mut out = TickOutput::default();
    let mut seen_lists = 0;
    let mut seen_link = 0;
    lossy(16, 900, Some(&scene), |_, _| {
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
        let readout = readout_of(&world);
        seen_lists = seen_lists
            .max(readout.sensors.contacts.len() + readout.visual.len() + readout.map.len());
        seen_link = seen_link.max(readout.link.tracks.len() + readout.link.mates.len());
        readout
    });
    assert!(seen_lists > 0, "the fight shows on the scope and the map");
    assert!(seen_link > 1, "the fight's data link has tracks and a mate");
}

/// The data link's parts (slice G7): tracks that jump on publishing ticks,
/// come and go; locks, net locks and assignments that change at any
/// snapshot; flightmates whose state changes. Whatever is lost, the client
/// holds what the host says, and each part reads back.
#[test]
fn the_links_parts_rebuild_what_the_host_says_whatever_was_lost() {
    use tore_world::datalink::{Damage, Fuel, TrackSource, Weapons};
    use tore_world::readout::{LinkAssigned, LinkMark, LinkMate, LinkTrack, MemberRef};
    let base = samples::readout();
    let mut tracks: Vec<LinkTrack> = (0..24)
        .map(|i| LinkTrack {
            target: 20 + 3 * i,
            position: [f64::from(i) * 5_000., 15_000., 40_000.],
            velocity: [f64::from(i) * 30. - 300., 2., -700.],
            source: [TrackSource::Own, TrackSource::Flight, TrackSource::Network][i as usize % 3],
        })
        .collect();
    let mut changed = 0;
    lossy(31, 1_200, None, |k, rng| {
        let tick = 400 + 4 * k;
        let mut readout = base.clone();
        readout.tick = u64::from(tick);
        // Tracks move only on the host's publishing ticks.
        if tick.is_multiple_of(30) {
            for track in &mut tracks {
                for axis in 0..3 {
                    track.position[axis] += track.velocity[axis] * 0.25;
                }
                if rng.below(10) == 0 {
                    track.velocity[0] += 25.;
                }
            }
            if rng.below(3) == 0 {
                let i = rng.below(24) as usize;
                tracks[i].target = if tracks[i].target >= 1_000 {
                    20 + 3 * i as u32
                } else {
                    1_000 + i as u32
                };
            }
            changed += 1;
        }
        readout.link.tracks = tracks.clone();
        readout.link.assigned = (rng.below(3) > 0).then(|| LinkAssigned {
            target: 20 + 3 * rng.below(24) as u32,
            by: rng.below(4) as u32,
            acknowledged: rng.below(2) == 0,
        });
        readout.link.radar = k % 200 < 150;
        readout.link.marks = (0..rng.below(33) as u32)
            .map(|i| LinkMark {
                target: 20 + 3 * i,
                lockers: rng.below(32) as u16,
                net_lock: (rng.below(4) == 0).then(|| MemberRef {
                    flight: rng.below(3) as u8,
                    member: rng.below(5) as u8,
                }),
                assigned_to: rng.below(32) as u16,
            })
            .collect();
        readout.link.mates = (1..=rng.below(5) as u32)
            .map(|plane| LinkMate {
                plane,
                member: plane as u8,
                fuel: [
                    Fuel::Normal,
                    Fuel::Joker,
                    Fuel::Bingo,
                    Fuel::Fumes,
                    Fuel::Out,
                ][rng.below(5) as usize],
                weapons: [Weapons::Missiles, Weapons::GunsOnly, Weapons::Winchester]
                    [rng.below(3) as usize],
                damage: [Damage::None, Damage::Light, Damage::Heavy][rng.below(3) as usize],
            })
            .collect();
        readout
    });
    assert!(changed > 60, "{changed} publishing ticks");
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
        let mut client = ClientConnection::for_flight(4, 0);
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

/// Bug B1: when the client's newest acknowledgement is over 127 snapshots old
/// (31 before slice B2: a stall of the game, or a round trip over a second at
/// 30 a second; now a stall of over 2 seconds at 60 a second), the host codes the
/// readout against the empty readout, and in a tight share the sensor flags
/// wait behind the parts before them. The client then held a readout whose
/// sensor group had not arrived, and the radar page read its empty flags as
/// NOT INSTALLED. The cockpit shows what last arrived for the plane until
/// the group comes again; the baselines stay as the host holds them.
#[test]
fn a_group_waiting_after_a_restart_from_the_empty_readout_shows_what_last_arrived() {
    use tore_sim::sensors::Channel;
    let mut host = ReadoutSender::new(4);
    let mut client = ReadoutReceiver::new(4);
    let readout = samples::readout();
    let receive = |client: &mut ReadoutReceiver, bits: &tore_codec::BitWriter, tick: u32| {
        let section =
            super::snapshot::write_section(&samples::header(tick), Some(bits), &[]).unwrap();
        let section = SnapshotSection::decode(&section).unwrap();
        client
            .receive(section.readout.as_ref().unwrap(), tick)
            .unwrap()
    };
    let shown = |client: &ReadoutReceiver| {
        let (tick, q) = client.presented().unwrap();
        q.readout(tick, None, None).unwrap()
    };
    // The first readout arrives whole and is acknowledged.
    let (bits, report) = host.build(400, &QReadout::of(&readout, 400), 8 * 1_000);
    assert_eq!(report.waiting, 0);
    host.sent(0);
    receive(&mut client, &bits, 400);
    host.delivered(0);
    assert!(shown(&client).sensors.available(Channel::Radar));
    // The game stalls: 140 snapshots go unacknowledged, so the next record
    // is against the empty readout, in a share too small for the sensors.
    for k in 1..=140u32 {
        let tick = 400 + 4 * k;
        let (bits, report) = host.build(tick, &QReadout::of(&readout, tick), 8 * 1_000);
        host.sent(k as u16);
        if k == 140 {
            assert_eq!(report.waiting, 0, "a full share sends it all");
        }
        let _ = bits;
    }
    let tick = 400 + 4 * 141;
    let (bits, report) = host.build(tick, &QReadout::of(&readout, tick), 120);
    assert!(report.waiting > 0, "the tight share leaves parts waiting");
    host.sent(141);
    let held = receive(&mut client, &bits, tick);
    assert_eq!(
        &held,
        host.staged_for_tests(141),
        "the baseline is the host's"
    );
    let raw = held.readout(tick, None, None).unwrap();
    assert!(
        !raw.sensors.available(Channel::Radar),
        "the held readout's sensor flags have not arrived"
    );
    let cockpit = shown(&client);
    assert_eq!(cockpit.sensors.available, readout.sensors.available);
    assert_eq!(cockpit.sensors.operating, readout.sensors.operating);
    assert_eq!(
        cockpit.sensors.radar_track_nmi,
        readout.sensors.radar_track_nmi
    );
    assert_eq!(cockpit.sensors.selected, readout.sensors.selected);
    // The gun-mount angles travel at 1/127, so they come back on that grid.
    let mut expected = readout.stores.clone();
    for (shown, sent) in cockpit.stores.gun_aim.iter().zip(expected.gun_aim) {
        assert!((shown - sent).abs() <= 0.5 / 127. + 1e-12);
    }
    expected.gun_aim = cockpit.stores.gun_aim;
    assert_eq!(
        cockpit.stores, expected,
        "so do the stores, which waited too"
    );
    assert_eq!(
        (cockpit.plane, cockpit.tick),
        (raw.plane, raw.tick),
        "the header is new"
    );
    // Acknowledged, the next record has room: the flags arrive and the
    // cockpit shows the held readout itself.
    host.delivered(141);
    let tick = 400 + 4 * 142;
    let mut changed = readout.clone();
    changed.sensors.operating[0] = false;
    let (bits, report) = host.build(tick, &QReadout::of(&changed, tick), 8 * 1_000);
    assert_eq!(report.waiting, 0);
    host.sent(142);
    let held = receive(&mut client, &bits, tick);
    let (_, presented) = client.presented().unwrap();
    assert!(matches!(presented, std::borrow::Cow::Borrowed(_)));
    assert_eq!(*presented, held);
    assert!(!shown(&client).sensors.operating(Channel::Radar));
    // Another plane's readout starts afresh: nothing of the last plane's is
    // shown for it.
    let mut other = readout.clone();
    other.plane = 4;
    let tick = 400 + 4 * 143;
    let (bits, _) = ReadoutSender::new(4).build(tick, &QReadout::of(&other, tick), 120);
    receive(&mut client, &bits, tick);
    assert_eq!(shown(&client).plane, 4);
    assert!(!shown(&client).sensors.available(Channel::Radar));
}

/// Slice B2: the readout codes against an acknowledged readout up to 127
/// snapshots old (2.1 seconds at 60 a second), and the client still holds
/// it; one older starts again from the empty readout. Every snapshot between
/// is received and none acknowledged, as on a slow round trip or a stall.
#[test]
fn a_readout_codes_against_a_baseline_up_to_127_snapshots_back() {
    const TICKS: u32 = 2;
    let readout = |k: u32| {
        let mut r = samples::readout();
        r.sensors.contacts[0].position[0] += 150. * f64::from(k);
        r.stores.ammo[0] -= (k % 7) as u16;
        QReadout::of(&r, 400 + TICKS * k)
    };
    for (gap, back) in [
        (1, 1),
        (31, 31),
        (32, 32),
        (126, 126),
        (127, 127),
        (128, 0),
        (200, 0),
    ] {
        let mut host = ReadoutSender::new(TICKS);
        let mut client = ReadoutReceiver::new(TICKS);
        let mut send = |host: &mut ReadoutSender, k: u32| {
            let tick = 400 + TICKS * k;
            let (bits, report) = host.build(tick, &readout(k), 8 * 2_000);
            assert_eq!(report.waiting, 0, "gap {gap}, snapshot {k}");
            host.sent(k as u16);
            let section =
                super::snapshot::write_section(&samples::header(tick), Some(&bits), &[]).unwrap();
            let section = SnapshotSection::decode(&section).unwrap();
            let raw = section.readout.unwrap();
            let held = client.receive(&raw, tick).unwrap();
            assert_eq!(&held, host.staged_for_tests(k as u16), "gap {gap}");
            assert_eq!(held, readout(k), "gap {gap}, snapshot {k}");
            raw.back
        };
        assert_eq!(send(&mut host, 0), 0);
        host.delivered(0);
        for k in 1..gap {
            send(&mut host, k);
        }
        assert_eq!(send(&mut host, gap), back, "gap {gap}");
    }
}

#[test]
fn the_gunsight_group_stands_for_the_sight() {
    use tore_sim::combat::gunship::{Notice, Sight, SightNotice};
    use tore_sim::combat::gunship_impact::Impact;
    use tore_sim::combat::live::Readiness;
    // Protocol 21 (gunsight slice S4). The sample is on the wire's grid, so
    // it comes back exactly.
    let readout = samples::readout();
    let back = QReadout::of(&readout, 400)
        .readout(400, None, None)
        .unwrap();
    assert_eq!(back.gunsight, readout.gunsight);
    // A plane with no gunsight has none.
    let mut plain = readout.clone();
    plain.gunsight = None;
    let back = QReadout::of(&plain, 400).readout(400, None, None).unwrap();
    assert_eq!(back.gunsight, None);
    // Every mode, impact kind and notice, off the grid: within half a step.
    let aim = [123_456.789, 321.123, -98_765.432_1];
    let point = |d: f64| [aim[0] + d, aim[1] - d, aim[2] + 2. * d];
    for (sight, impacts, notice, aimed) in [
        (
            Sight::Free,
            [
                Some(Impact::Air {
                    point: point(10.01),
                    seconds: 2.001,
                    range_ft: 4_000.4,
                }),
                Some(Impact::Ground {
                    point: point(-3.3),
                    seconds: 3.3,
                    range_ft: 5_000.,
                }),
                None,
            ],
            None,
            true,
        ),
        (
            Sight::Tracked(70_001),
            [None, None, None],
            Some(SightNotice {
                notice: Notice::NoGroundPoint,
                tick: 12,
            }),
            false,
        ),
        (Sight::Pinned(point(0.)), [None; 3], None, true),
        (
            Sight::Tracked(8),
            [None; 3],
            Some(SightNotice {
                notice: Notice::GimbalLimit,
                tick: 11,
            }),
            true,
        ),
    ] {
        let mut readout = readout.clone();
        let gunsight = tore_world::readout::GunsightReadout {
            sight,
            look: [2.345_678, -1.234_567],
            returning: true,
            aim: aimed.then_some(aim),
            impacts,
            impacts_tick: 399,
            status: [
                Readiness::GunSlewing,
                Readiness::MaximumRange,
                Readiness::NoTarget,
            ],
            notice,
            zoom: 6,
        };
        readout.gunsight = Some(gunsight.clone());
        let q = QReadout::of(&readout, 400);
        let got = q.readout(400, None, None).unwrap().gunsight.unwrap();
        // Rounding twice changes nothing.
        let mut again = readout.clone();
        again.gunsight = Some(got.clone());
        assert_eq!(QReadout::of(&again, 400), q);
        let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() <= 0.125);
        match (got.sight, gunsight.sight) {
            (Sight::Pinned(a), Sight::Pinned(b)) => assert!(near(a, b)),
            (a, b) => assert_eq!(a, b),
        }
        let turn = std::f64::consts::TAU / 1_048_576.;
        assert!((0..2).all(|i| (got.look[i] - gunsight.look[i]).abs() <= turn));
        assert!(got.returning);
        assert_eq!(got.aim.is_some(), aimed);
        if aimed {
            assert!(near(got.aim.unwrap(), aim));
        }
        for (a, b) in got.impacts.iter().zip(&gunsight.impacts) {
            match (a, b) {
                (None, None) => {}
                (Some(a), Some(b)) => {
                    assert_eq!(std::mem::discriminant(a), std::mem::discriminant(b));
                    assert!(near(a.point(), b.point()), "{a:?} {b:?}");
                    assert!((a.seconds() - b.seconds()).abs() <= 1. / 128.);
                    assert!((a.range_ft() - b.range_ft()).abs() <= 0.5);
                }
                _ => panic!("{a:?} against {b:?}"),
            }
        }
        assert_eq!(got.impacts_tick, 399);
        assert_eq!(got.status, gunsight.status);
        assert_eq!(got.notice, gunsight.notice);
        assert_eq!(got.zoom, 6);
    }
    // Damaged values are refused, never a panic.
    let good = QReadout::of(&readout, 400);
    for (index, value) in [
        (0, 3),
        (12, 4),
        (34, 4),
        (36, 0),
        (36, 7),
        (31, 99),
        (1, -1),
    ] {
        let mut bad = good.clone();
        bad.gunsight_values_for_tests()[index] = value;
        if index == 1 {
            bad.gunsight_values_for_tests()[0] = 2;
        }
        assert!(bad.readout(400, None, None).is_err(), "value {index}");
    }
    let mut short = good.clone();
    short.gunsight_values_for_tests().pop();
    assert!(short.readout(400, None, None).is_err());
}

#[test]
fn an_ac130s_readout_carries_its_gunsight_through_a_connection() {
    // A synthetic AC-130 slewing its sight: the host's readout, sent and
    // received, shows the sight the sim holds (within the wire's steps).
    let mut spec = MissionSpec::new(THEATER, AircraftId::Ac130);
    spec.start = Start::Airborne { altitude_ft: 5_000 };
    let mut world = World::new(&spec, &gunship_resources(), Seating::SinglePlayer).unwrap();
    let mut out = TickOutput::default();
    let mut host = HostConnection::new(4);
    let mut client = ClientConnection::for_flight(4, 0);
    let mut checked = 0;
    for tick in 0..480u64 {
        world
            .step(
                &[SeatInput {
                    tick,
                    sight: if tick < 240 { [127, -40] } else { [0, 0] },
                    sight_zoom: 2,
                    ..SeatInput::default()
                }],
                &mut out,
            )
            .unwrap();
        if tick % 4 != 3 {
            continue;
        }
        let readout = readout_of(&world);
        let sim = world
            .combat
            .state
            .ownship(0)
            .unwrap()
            .gunship
            .clone()
            .unwrap();
        let shown = readout.gunsight.clone().unwrap();
        assert_eq!(shown.look, sim.look);
        assert_eq!(shown.zoom, 2);
        let header = samples::header(world.tick() as u32);
        let packet = host
            .snapshot_with_readout(&header, &[], Some(&readout), 0)
            .unwrap();
        let sequence = checked as u16;
        host.sent(sequence);
        let (_, received) = client.snapshot(&packet.snapshot).unwrap();
        host.delivered(sequence);
        let got = received
            .readout
            .unwrap()
            .readout(world.tick() as u32, None, None)
            .unwrap()
            .gunsight
            .unwrap();
        let turn = std::f64::consts::TAU / 1_048_576.;
        assert!((0..2).all(|i| (got.look[i] - sim.look[i]).abs() <= turn));
        assert_eq!(got.sight, tore_sim::combat::gunship::Sight::Free);
        assert_eq!(got.status, sim.status);
        assert!((0..3).all(|i| (got.aim.unwrap()[i] - sim.aim.unwrap()[i]).abs() <= 0.125));
        checked += 1;
    }
    assert_eq!(checked, 120);
    // It slewed: the look moved off the default view.
    let look = world
        .combat
        .state
        .ownship(0)
        .unwrap()
        .gunship
        .as_ref()
        .unwrap()
        .look;
    assert_ne!(look, tore_sim::combat::gunship::DEFAULT_LOOK);
}
