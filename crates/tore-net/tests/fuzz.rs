//! A seeded fuzz: 100,000 random and mutated packets against a connected host
//! and client. Nothing may panic; afterwards a fresh join still works.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::{MS, VERSION, World, host_addr, player_addr};
use tore_codec::BitWriter;
use tore_net::packet::{self, PacketKind};
use tore_net::sim::LinkConfig;
use tore_net::{ClientEvent, ClientState, Event, SplitMix64};

const PACKETS: usize = 100_000;

/// Rewrites the checksum so the packet reaches the decoders behind it.
fn reseal(bytes: &mut [u8]) {
    if bytes.len() < 5 {
        return;
    }
    if let Some(kind) = PacketKind::from_u8(bytes[4]) {
        let crc = packet::checksum(kind, VERSION, &bytes[4..]);
        bytes[..4].copy_from_slice(&crc.to_le_bytes());
    }
}

fn random_bytes(rng: &mut SplitMix64, len: usize) -> Vec<u8> {
    (0..len).map(|_| rng.next_u64() as u8).collect()
}

/// A Messages section body of random, sometimes nearly valid, records.
fn random_messages(rng: &mut SplitMix64) -> Vec<u8> {
    let mut w = BitWriter::new();
    let count = rng.below(6);
    w.write_bits(count, 8).unwrap();
    for _ in 0..count {
        let id = match rng.below(3) {
            0 => rng.below(8),
            1 => rng.below(600),
            _ => rng.next_u64(),
        };
        w.write_bits(id, 16).unwrap();
        let part = rng.below(4);
        w.write_bits(part, 2).unwrap();
        if part != 2 {
            w.write_bits(rng.below(256), 8).unwrap();
        }
        if part == 1 {
            w.write_bits(rng.below(1 << 16), 16).unwrap();
        }
        let len = match rng.below(3) {
            0 => 256,
            1 => rng.below(20),
            _ => rng.below(512),
        };
        w.write_bits(len, 9).unwrap();
        let body = random_bytes(rng, (len as usize).min(300));
        w.write_bytes(&body);
    }
    if rng.chance(0.1) {
        w.write_bits(rng.below(256), 8).unwrap();
    }
    w.finish()
}

/// A Payload with the right connection id and random everything else.
fn random_payload(rng: &mut SplitMix64, connection: u32) -> Vec<u8> {
    let mut w = BitWriter::new();
    w.write_bits(0, 32).unwrap();
    w.write_bits(6, 8).unwrap();
    let id = if rng.chance(0.9) {
        connection
    } else {
        rng.next_u64() as u32
    };
    w.write_bits(u64::from(id), 32).unwrap();
    for bits in [16, 16, 32] {
        w.write_bits(rng.next_u64(), bits).unwrap();
    }
    let delay = if rng.chance(0.5) {
        0xFFFF
    } else {
        rng.below(0x1_0000)
    };
    w.write_bits(delay, 16).unwrap();
    for _ in 0..rng.below(4) {
        let kind = rng.below(8);
        let body = if kind == 1 && rng.chance(0.8) {
            random_messages(rng)
        } else {
            let len = rng.below(64) as usize;
            random_bytes(rng, len)
        };
        w.write_bits(kind, 8).unwrap();
        let claimed = if rng.chance(0.9) {
            body.len() as u64
        } else {
            rng.below(2000)
        };
        w.write_bits(claimed, 16).unwrap();
        w.write_bytes(&body);
    }
    let mut bytes = w.finish();
    bytes.truncate(1300);
    reseal(&mut bytes);
    bytes
}

/// One fuzz packet: random bytes, a real packet mutated, or a forged one.
fn fuzz_packet(rng: &mut SplitMix64, samples: &[Vec<u8>], connection: u32) -> Vec<u8> {
    match rng.below(5) {
        0 => {
            let len = rng.below(1300) as usize;
            let mut bytes = random_bytes(rng, len);
            if rng.chance(0.5) && bytes.len() > 4 {
                bytes[4] = rng.below(14) as u8;
                reseal(&mut bytes);
            }
            bytes
        }
        1 | 2 => {
            let mut bytes = samples[rng.below(samples.len() as u64) as usize].clone();
            // Payload and Disconnect carry the connection id: usually give
            // them the current one so they get past the stale check.
            if matches!(bytes[4], 6 | 7) && rng.chance(0.8) {
                bytes[5..9].copy_from_slice(&connection.to_le_bytes());
            }
            for _ in 0..1 + rng.below(8) {
                match rng.below(4) {
                    0 if !bytes.is_empty() => {
                        let i = rng.below(bytes.len() as u64) as usize;
                        bytes[i] ^= 1 << rng.below(8);
                    }
                    1 if !bytes.is_empty() => {
                        let i = rng.below(bytes.len() as u64) as usize;
                        bytes[i] = rng.next_u64() as u8;
                    }
                    2 => bytes.truncate(rng.below(bytes.len() as u64 + 1) as usize),
                    _ => {
                        let extra = rng.below(40) as usize;
                        bytes.extend(random_bytes(rng, extra));
                    }
                }
            }
            if rng.chance(0.75) {
                reseal(&mut bytes);
            }
            bytes
        }
        3 => random_payload(rng, connection),
        _ => {
            // Handshake kinds with good checksums, often at the padded size.
            let len = if rng.chance(0.5) {
                1000
            } else {
                rng.below(64) as usize
            };
            let mut bytes = random_bytes(rng, len.max(5));
            bytes[4] = 1 + rng.below(7) as u8;
            if rng.chance(0.5) && bytes.len() >= 7 {
                bytes[5..7].copy_from_slice(&VERSION.to_le_bytes());
            }
            if rng.chance(0.3) {
                for b in bytes.iter_mut().skip(40) {
                    *b = 0;
                }
            }
            reseal(&mut bytes);
            bytes
        }
    }
}

#[test]
fn a_hundred_thousand_bad_packets_never_panic() {
    let mut w = World::new(404, LinkConfig::one_way(5 * MS));
    w.net.start_trace();
    let mut player = w.join("Viper");
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    // Real traffic to mutate: messages both ways, large and small, and caller
    // sections.
    let id = w.connection_of(0);
    for i in 0..40u8 {
        w.players[0]
            .client
            .send_message(i, &vec![i; usize::from(i) * 40])
            .unwrap();
        w.server.send_message(id, i, &vec![i; 300]).unwrap();
        let now = w.now();
        w.players[0]
            .client
            .send_payload(now, &[(2, &[i; 20])])
            .unwrap();
        w.server
            .send_payload(now, id, &[(3, &[i; 50]), (4, &[i])])
            .unwrap();
        w.step(10 * MS);
    }
    let mut samples: Vec<Vec<u8>> = w.net.take_trace().into_iter().map(|t| t.datagram).collect();
    assert!(samples.len() > 50, "{} samples", samples.len());
    // Stage K's Reach and its answer (protocol 13), which the host answers
    // for its own session.
    w.server.set_reach_session(Some(0x5E55));
    for packet in [
        packet::Packet::Reach(packet::Reach {
            session_id: 0x5E55,
            nonce: 7,
            from: 1,
        }),
        packet::Packet::ReachAnswer(packet::ReachAnswer {
            nonce: 7,
            session_id: 0x5E55,
            role: tore_net::ReachRole::Hosting,
        }),
    ] {
        samples.push(packet.encode(VERSION).unwrap());
    }

    let mut rng = SplitMix64::new(0xF022);
    let mut joins = 1;
    let mut stranger_port = 50_000u16;
    let mut decoded = 0;
    for i in 0..PACKETS {
        let connection = w.players[player]
            .client
            .welcome()
            .map_or(0, |welcome| welcome.connection.0);
        let bytes = fuzz_packet(&mut rng, &samples, connection);
        decoded += usize::from(packet::Packet::decode(&bytes, VERSION).is_ok());
        let to_host = rng.chance(0.7);
        let (from, to): (SocketAddr, SocketAddr) = if to_host {
            if rng.chance(0.85) {
                (player_addr(player as u16), host_addr())
            } else {
                stranger_port = stranger_port.wrapping_add(1).max(50_000);
                (
                    SocketAddr::from(([10, 7, 0, 1], stranger_port)),
                    host_addr(),
                )
            }
        } else {
            (host_addr(), player_addr(player as u16))
        };
        w.net.inject(from, to, &bytes);
        // Keep real traffic flowing, and time moving so bad-packet windows
        // and cookies roll over.
        if i % 16 == 0 {
            let now = w.now();
            let _ = w.players[player].client.send_message(1, b"still here");
            let _ = w.players[player].client.send_payload(now, &[(2, b"input")]);
            w.step(if i % 64 == 0 { 100 * MS } else { MS });
        }
        if w.players[player].client.state() == ClientState::Closed {
            player = w.join(&format!("Viper{joins}"));
            joins += 1;
            w.run_until(Duration::from_secs(2), |w| w.connected(player));
        }
    }
    w.run_for(Duration::from_secs(6));
    let mut endings = std::collections::BTreeMap::new();
    for (_, event) in &w.host_events {
        if let tore_net::ServerEvent::Closed { reason, .. } = event {
            *endings.entry(format!("{reason:?}")).or_insert(0) += 1;
        }
    }
    eprintln!(
        "{PACKETS} fuzz packets, {decoded} decoded as packets, {joins} joins, host counters {:?}, endings {endings:?}",
        w.server.counters()
    );
    // A fresh join after all that still works end to end.
    let fresh = w.join("Fresh");
    assert!(w.run_until(Duration::from_secs(2), |w| w.connected(fresh)));
    w.players[fresh].client.send_message(7, b"hello").unwrap();
    assert!(w.run_until(Duration::from_secs(1), |w| {
        w.host_events.iter().any(|(_, e)| {
            matches!(e, tore_net::ServerEvent::Connection { event: Event::Message { kind: 7, body }, .. } if body == b"hello")
        })
    }));
    assert!(
        !w.players[fresh]
            .events
            .iter()
            .any(|(_, e)| matches!(e, ClientEvent::Closed(_)))
    );
}

#[test]
fn decoders_never_panic_on_random_bytes() {
    let mut rng = SplitMix64::new(77);
    for _ in 0..PACKETS {
        let len = rng.below(1300) as usize;
        let mut bytes = random_bytes(&mut rng, len);
        if rng.chance(0.5) && bytes.len() >= 5 {
            bytes[4] = 1 + rng.below(7) as u8;
            reseal(&mut bytes);
        }
        let _ = packet::Packet::decode(&bytes, VERSION);
        let body = bytes.get(5..).unwrap_or(&[]);
        let _ = packet::decode_connect_request(bytes.len(), body, VERSION);
        let _ = packet::decode_connect_request(1000, body, VERSION + 1);
        let _ = packet::decode_challenge_answer(1000, body);
        let _ = packet::decode_sections(body);
        let _ = packet::decode_payload_header(body);
    }
}
