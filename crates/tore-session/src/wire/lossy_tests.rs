//! A snapshot decodes whatever earlier packets were lost: a long seeded run
//! drops, duplicates and reorders packets and their acknowledgements, and
//! checks the client's reconstruction of every entity against the host's
//! quantized state at that tick. Entities come and go, some are far (twice a
//! second), some packets are short of room, and a missile aimed at the
//! player is never left out.

use super::entity::{Entity, EntityKey, EntityKind, EntityState};
use super::priority::{NEAR_FT, Relevance};
use super::round_trip_tests::{entity, step};
use super::samples;
use super::snapshot::{EntityReceiver, EntitySender, RecordBody, SnapshotSection};
use std::collections::{BTreeMap, HashMap};
use tore_net::SplitMix64;

const TICKS_PER_SNAPSHOT: u32 = 4;

struct Flight {
    arrives: u32,
    sequence: u16,
    bytes: Vec<u8>,
}

struct Notice {
    at: u32,
    sequence: u16,
    delivered: bool,
}

fn relevance(entity: &Entity) -> Relevance {
    let far = entity.id.is_multiple_of(3);
    Relevance {
        distance_ft: if far { NEAR_FT * 2. } else { 1_000. },
        aimed_at_player: matches!(entity.state, EntityState::Projectile(p) if p.aimed_at_player),
        ..Relevance::NEAR
    }
}

fn run(seed: u64, snapshots: u32, loss: u64, duplicate: u64) {
    let mut rng = SplitMix64::new(seed);
    let mut host = EntitySender::new(TICKS_PER_SNAPSHOT);
    let mut client = EntityReceiver::new(TICKS_PER_SNAPSHOT);
    let mut next_id = [0u32; 4];
    let mut world: BTreeMap<EntityKey, Entity> = BTreeMap::new();
    let mut spawn = |rng: &mut SplitMix64, world: &mut BTreeMap<EntityKey, Entity>| {
        let kind = EntityKind::ALL[rng.below(4) as usize];
        let slot = &mut next_id[usize::from(kind.code())];
        *slot += 1 + rng.below(4) as u32;
        let e = entity(rng, kind, *slot);
        world.insert(e.key(), e);
    };
    for _ in 0..40 {
        spawn(&mut rng, &mut world);
    }
    // The host's states by tick, for checking what the client rebuilds.
    let mut history: HashMap<u32, BTreeMap<EntityKey, EntityState>> = HashMap::new();
    let mut flights: Vec<Flight> = Vec::new();
    let mut notices: Vec<Notice> = Vec::new();
    let mut newest_received: Option<u16> = None;
    let mut received_count = 0usize;
    let mut last_sent: HashMap<EntityKey, u32> = HashMap::new();
    let first_sequence = 65_000u16;
    for k in 0..snapshots {
        let tick = 400 + k * TICKS_PER_SNAPSHOT;
        let sequence = first_sequence.wrapping_add(k as u16);
        // The world moves; things come and go.
        let keys: Vec<EntityKey> = world.keys().copied().collect();
        for key in keys {
            if rng.below(100) == 0 {
                world.remove(&key);
            } else {
                let moved = step(&mut rng, &world[&key], TICKS_PER_SNAPSHOT);
                world.insert(key, moved);
            }
        }
        if rng.below(100) < 2 || world.len() < 20 {
            spawn(&mut rng, &mut world);
        }
        history.insert(tick, world.iter().map(|(k, e)| (*k, e.state)).collect());
        history.retain(|t, _| *t + 200 * TICKS_PER_SNAPSHOT >= tick);

        // The host builds and sends; some packets are short of room.
        let entities: Vec<(Entity, Relevance)> =
            world.values().map(|e| (*e, relevance(e))).collect();
        // The random states are far larger than real ones, so the usual
        // share is generous; one packet in ten is short.
        let budget = if rng.below(10) == 0 { 250 } else { 3_000 };
        let (bytes, report) = host
            .build(&samples::header(tick), &entities, budget)
            .unwrap();
        host.sent(sequence);
        let section = SnapshotSection::decode(&bytes).unwrap();
        for (e, rel) in &entities {
            if rel.forced(e.state.kind()) {
                assert!(
                    section.records.iter().any(|r| r.key == e.key()),
                    "snapshot {k}: the missile aimed at the player was left out"
                );
            }
        }
        for record in &section.records {
            if !matches!(record.body, RecordBody::Removed) {
                last_sent.insert(record.key, k);
            }
        }
        if budget == 3_000 {
            assert_eq!(report.waiting, 0, "snapshot {k}");
        }
        // Nothing waits for long: a near entity within a few snapshots, a
        // far one within a second and a bit.
        for e in world.values() {
            if let Some(&sent) = last_sent.get(&e.key()) {
                let limit = if e.id.is_multiple_of(3) { 40 } else { 12 };
                assert!(
                    k - sent <= limit,
                    "snapshot {k}: {:?} last sent at {sent}",
                    e.key()
                );
            }
        }

        // The network: loss, duplicates and reordering.
        if rng.below(100) >= loss {
            let copies = if rng.below(100) < duplicate { 2 } else { 1 };
            for _ in 0..copies {
                flights.push(Flight {
                    arrives: k + 2 + rng.below(5) as u32,
                    sequence,
                    bytes: bytes.clone(),
                });
            }
        } else {
            notices.push(Notice {
                at: k + 34,
                sequence,
                delivered: false,
            });
        }

        // Arrivals: the transport drops repeats and anything 32 or more
        // behind the newest; the rest is read and acknowledged.
        let (arriving, waiting): (Vec<Flight>, Vec<Flight>) =
            flights.drain(..).partition(|f| f.arrives <= k);
        flights = waiting;
        let mut seen_now: Vec<u16> = Vec::new();
        for flight in arriving {
            let behind = newest_received.map_or(0, |n| n.wrapping_sub(flight.sequence) as i16);
            if behind >= 32 || seen_now.contains(&flight.sequence) {
                continue;
            }
            seen_now.push(flight.sequence);
            if newest_received.is_none_or(|n| (flight.sequence.wrapping_sub(n) as i16) > 0) {
                newest_received = Some(flight.sequence);
            }
            let section = SnapshotSection::decode(&flight.bytes).unwrap();
            let received = client.receive(&section);
            assert!(
                received.unresolved.is_empty(),
                "snapshot {k}: {:?}",
                received.unresolved
            );
            received_count += 1;
            let states = &history[&section.header.tick];
            for e in &received.updated {
                assert_eq!(
                    states.get(&e.key()),
                    Some(&e.state),
                    "tick {}: {:?} rebuilt wrong",
                    section.header.tick,
                    e.key()
                );
            }
            for key in &received.removed {
                assert!(
                    !states.contains_key(key),
                    "tick {}: {key:?} removed while present",
                    section.header.tick
                );
            }
            // The acknowledgement comes back, or is lost with its packet's
            // window and the host judges the packet lost.
            let ack_lost = rng.below(100) < loss / 2;
            notices.push(Notice {
                at: if ack_lost {
                    k + 34
                } else {
                    k + 1 + rng.below(3) as u32
                },
                sequence: flight.sequence,
                delivered: !ack_lost,
            });
        }
        let (due, later): (Vec<Notice>, Vec<Notice>) = notices.drain(..).partition(|n| n.at <= k);
        notices = later;
        for notice in due {
            if notice.delivered {
                host.delivered(notice.sequence);
            } else {
                host.lost(notice.sequence);
            }
        }
    }
    assert!(received_count > snapshots as usize / 2);

    // Then a clean link: everything settles to the host's last state, and
    // nothing removed lingers.
    for k in snapshots..snapshots + 40 {
        let tick = 400 + k * TICKS_PER_SNAPSHOT;
        let sequence = first_sequence.wrapping_add(k as u16);
        let entities: Vec<(Entity, Relevance)> =
            world.values().map(|e| (*e, Relevance::NEAR)).collect();
        let (bytes, _) = host
            .build(&samples::header(tick), &entities, 4_000)
            .unwrap();
        host.sent(sequence);
        let received = client.receive(&SnapshotSection::decode(&bytes).unwrap());
        assert!(received.unresolved.is_empty());
        host.delivered(sequence);
    }
    let known: Vec<(EntityKey, EntityState)> = client
        .entities()
        .into_iter()
        .map(|(key, _, state)| (key, state))
        .collect();
    let expected: Vec<(EntityKey, EntityState)> =
        world.iter().map(|(k, e)| (*k, e.state)).collect();
    assert_eq!(known, expected);
    assert_eq!(host.removals_pending(), 0);
}

#[test]
fn snapshots_decode_whatever_was_lost() {
    for seed in 1..=4 {
        run(seed, 3_000, 10, 5);
    }
}

#[test]
fn snapshots_decode_under_heavy_loss() {
    run(99, 3_000, 40, 20);
}
