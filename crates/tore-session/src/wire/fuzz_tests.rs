//! A seeded fuzz: 100,000 random and mutated section and message bodies
//! through every decoder, and the client's use of what decodes. Nothing may
//! panic.

use super::connection::ClientConnection;
use super::events::EventsSection;
use super::inputs::InputsSection;
use super::messages::{Message, kind};
use super::own_state::OwnStateReceiver;
use super::snapshot::{EntityReceiver, SnapshotSection};
use super::{SECTION_EVENTS, SECTION_OWN_STATE, SECTION_SNAPSHOT, samples};
use tore_net::SplitMix64;

const RUNS: usize = 100_000;

fn mutate(rng: &mut SplitMix64, sample: &[u8]) -> Vec<u8> {
    let mut bytes = sample.to_vec();
    match rng.below(6) {
        // Random bytes of a similar length.
        0 => {
            let len = rng.below(sample.len() as u64 * 2 + 8) as usize;
            bytes = (0..len).map(|_| rng.below(256) as u8).collect();
        }
        // Truncated.
        1 => {
            let len = rng.below(bytes.len() as u64 + 1) as usize;
            bytes.truncate(len);
        }
        // Bits flipped.
        2 => {
            for _ in 0..1 + rng.below(8) {
                if !bytes.is_empty() {
                    let at = rng.below(bytes.len() as u64) as usize;
                    bytes[at] ^= 1 << rng.below(8);
                }
            }
        }
        // Bytes overwritten with extremes.
        3 => {
            for _ in 0..1 + rng.below(4) {
                if !bytes.is_empty() {
                    let at = rng.below(bytes.len() as u64) as usize;
                    bytes[at] = [0x00, 0xFF, 0x80, 0x7F][rng.below(4) as usize];
                }
            }
        }
        // Bytes inserted.
        4 => {
            let at = rng.below(bytes.len() as u64 + 1) as usize;
            for _ in 0..1 + rng.below(16) {
                bytes.insert(at, rng.below(256) as u8);
            }
        }
        // A stretch of another place spliced in.
        _ => {
            if bytes.len() > 2 {
                let from = rng.below(bytes.len() as u64) as usize;
                let to = rng.below(bytes.len() as u64) as usize;
                let len = rng.below((bytes.len() - from.max(to)) as u64 + 1) as usize;
                let piece = bytes[from..from + len].to_vec();
                bytes[to..to + len].copy_from_slice(&piece);
            }
        }
    }
    bytes
}

#[test]
fn every_decoder_survives_random_and_mutated_bodies() {
    let inputs = samples::inputs().encode().unwrap();
    let (full, delta) = samples::snapshots();
    let events = samples::events().encode().unwrap();
    let messages: Vec<(u8, Vec<u8>)> = samples::messages(vec![7; 40])
        .iter()
        .map(|m| (m.kind(), m.encode().unwrap()))
        .collect();
    let own = OwnStateReceiver::new();
    let map = tore_world::test_support::resources::resources();
    let model = tore_sim::models::AircraftModel::for_aircraft(
        &tore_world::aircraft_type::AircraftType::load(
            &map,
            tore_formats::aircraft::AircraftId::F18,
        )
        .unwrap()
        .profile,
    )
    .unwrap();
    let mut rng = SplitMix64::new(20_260_930);
    let mut decoded = [0usize; 5];
    for run in 0..RUNS {
        match run % 5 {
            0 => {
                let body = mutate(&mut rng, &inputs);
                if let Ok(section) = InputsSection::decode(&body) {
                    decoded[0] += 1;
                    // What decodes encodes to the same bytes.
                    assert_eq!(section.encode().unwrap(), body);
                    let _ = section.view();
                    for (index, frame) in section.frames.iter().enumerate() {
                        let _ = section.frame_tick(index);
                        let _ = frame.seat_input(Default::default(), 0, &[], None);
                    }
                }
            }
            1 => {
                let sample = if rng.below(2) == 0 { &full } else { &delta };
                let body = mutate(&mut rng, sample);
                // A client that has the first snapshot, so records against
                // it are applied too.
                let mut client = EntityReceiver::new(4);
                client.receive(&SnapshotSection::decode(&full).unwrap());
                if let Ok(section) = SnapshotSection::decode(&body) {
                    decoded[1] += 1;
                    let _ = client.receive(&section);
                    let _ = client.entities();
                }
            }
            2 => {
                let body = mutate(&mut rng, &events);
                if let Ok(section) = EventsSection::decode(&body) {
                    decoded[2] += 1;
                    let mut client = ClientConnection::new(4);
                    let _ = client
                        .events
                        .receive(&section, rng.below(1 << 32) as u32, 3);
                    let _ = client.events.names_arrived(4096);
                }
            }
            3 => {
                let (kind, sample) = &messages[rng.below(messages.len() as u64) as usize];
                let kind = if rng.below(10) == 0 {
                    rng.below(256) as u8
                } else {
                    *kind
                };
                let body = mutate(&mut rng, sample);
                if let Ok(message) = Message::decode(kind, &body) {
                    decoded[3] += 1;
                    if let Message::Mission(mission) = &message {
                        let _ = mission.spec();
                    }
                    let _ = message.encode();
                }
            }
            _ => {
                // An own state's header and baseline lookup, and the
                // client's check of each section kind.
                let body = mutate(&mut rng, &full);
                if own.baseline(&body).is_ok() {
                    decoded[4] += 1;
                    let _ = own.clone().receive(&body, &model);
                }
                let client = ClientConnection::new(4);
                for kind in [SECTION_SNAPSHOT, SECTION_EVENTS, SECTION_OWN_STATE, 9] {
                    let _ = client.check(kind, &body);
                }
                let _ = Message::decode(kind::LEAVE, &body);
            }
        }
    }
    // The mutations reach past the first field of each decoder.
    assert!(decoded.iter().all(|&count| count > 0), "{decoded:?}");
}
