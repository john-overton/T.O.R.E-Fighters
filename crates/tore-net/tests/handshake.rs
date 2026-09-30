//! Joining, refusals, timeouts, disconnects and the host's defences.

mod common;

use std::time::Duration;

use common::{MS, VERSION, World, host_addr, player_addr};
use tore_codec::BitWriter;
use tore_net::packet::{
    ACK_DELAY_NONE, ChallengeAnswer, ConnectRequest, Packet, PacketKind, PayloadHeader, Section,
};
use tore_net::sim::LinkConfig;
use tore_net::{
    ClientEvent, ClientState, CloseReason, DisconnectReason, Event, RATE_LIMIT_PER_ADDRESS,
    RefuseReason, ServerConfig, ServerEvent,
};

fn closed(events: &[(Duration, ClientEvent)]) -> Option<(Duration, CloseReason)> {
    events.iter().find_map(|(at, e)| match e {
        ClientEvent::Closed(reason) => Some((*at, reason.clone())),
        _ => None,
    })
}

fn host_closed(events: &[(Duration, ServerEvent)]) -> Vec<(Duration, CloseReason)> {
    events
        .iter()
        .filter_map(|(at, e)| match e {
            ServerEvent::Closed { reason, .. } => Some((*at, reason.clone())),
            _ => None,
        })
        .collect()
}

fn connected_count(events: &[(Duration, ServerEvent)]) -> usize {
    events
        .iter()
        .filter(|(_, e)| matches!(e, ServerEvent::Connected { .. }))
        .count()
}

fn lossy() -> LinkConfig {
    LinkConfig::for_round_trip(Duration::from_millis(300), 0.1, 0.05, 0.01)
}

#[test]
fn handshake_completes_on_a_lossy_link_and_never_amplifies() {
    for seed in 0..20 {
        let mut w = World::new(seed, lossy());
        w.net.start_trace();
        w.gate.password = "pw".into();
        let mut config = w.player_config("Viper", 0);
        config.password = "pw".into();
        w.join_with(config, 0);
        assert!(
            w.run_until(Duration::from_secs(10), |w| w.connected(0)),
            "seed {seed}"
        );
        let welcome = *w.players[0].client.welcome().unwrap();
        assert_eq!(welcome.session_id, 77);
        assert_eq!(welcome.ticks_per_second, 120);
        assert_eq!(welcome.host_tick, 1_000);
        assert_eq!(w.gate.seen.len(), 1);
        assert_eq!(w.gate.seen[0].callsign, "Viper");
        assert_eq!(w.gate.seen[0].game_commit, "5b7b9fd");
        assert_eq!(w.gate.seen[0].address, player_addr(0));
        // Every host reply to a handshake packet is smaller than the request.
        for t in w.net.take_trace() {
            let kind = PacketKind::from_u8(t.datagram[4]).unwrap();
            match kind {
                PacketKind::ConnectRequest | PacketKind::ChallengeAnswer => {
                    assert_eq!(t.datagram.len(), 1000)
                }
                PacketKind::Challenge => assert_eq!(t.datagram.len(), 21),
                PacketKind::Accepted => assert_eq!(t.datagram.len(), 31),
                _ => {}
            }
        }
        // Both sides then keep the connection up.
        w.run_for(Duration::from_secs(3));
        assert_eq!(w.players[0].client.state(), ClientState::Connected);
        assert_eq!(w.server.connections().count(), 1);
    }
}

#[test]
fn wrong_password_is_refused_with_its_reason() {
    let mut w = World::new(1, LinkConfig::one_way(10 * MS));
    w.gate.password = "right".into();
    let mut config = w.player_config("Viper", 0);
    config.password = "wrong".into();
    w.join_with(config, 0);
    w.run_for(Duration::from_secs(1));
    let (_, reason) = closed(&w.players[0].events).expect("closed");
    assert_eq!(
        reason,
        CloseReason::Refused {
            reason: RefuseReason::WrongPassword,
            text: "Wrong password.".into()
        }
    );
    assert_eq!(w.players[0].client.state(), ClientState::Closed);
    assert_eq!(w.server.connections().count(), 0);
}

#[test]
fn another_protocol_version_is_refused_naming_both() {
    let mut w = World::new(2, LinkConfig::one_way(10 * MS));
    let mut config = w.player_config("Viper", 0);
    config.protocol_version = VERSION + 1;
    w.join_with(config, 0);
    w.run_for(Duration::from_secs(1));
    let (_, reason) = closed(&w.players[0].events).expect("closed");
    let CloseReason::Refused { reason, text } = reason else {
        panic!("{reason:?}")
    };
    assert_eq!(reason, RefuseReason::ProtocolVersion);
    assert!(text.contains(&VERSION.to_string()) && text.contains(&(VERSION + 1).to_string()));
    assert!(w.gate.seen.is_empty());
}

#[test]
fn a_full_server_refuses_before_asking_the_gate() {
    let config = ServerConfig {
        max_connections: 1,
        ..ServerConfig::new(VERSION)
    };
    let mut w = World::with_config(3, LinkConfig::one_way(10 * MS), config);
    w.join("One");
    w.join("Two");
    w.run_for(Duration::from_secs(1));
    assert!(w.connected(0));
    let (_, reason) = closed(&w.players[1].events).expect("closed");
    assert!(matches!(
        reason,
        CloseReason::Refused {
            reason: RefuseReason::ServerFull,
            ..
        }
    ));
    assert_eq!(w.gate.seen.len(), 1);
}

#[test]
fn no_answer_gives_up_after_ten_seconds_retrying_every_250_ms() {
    let mut w = World::new(4, LinkConfig::one_way(10 * MS));
    w.net.start_trace();
    // Nobody listens at the host address.
    drop(std::mem::replace(
        &mut w.host_socket,
        w.net.bind("10.9.9.9:1".parse().unwrap()).unwrap(),
    ));
    w.join("Viper");
    w.run_for(Duration::from_secs(11));
    let (at, reason) = closed(&w.players[0].events).expect("closed");
    assert_eq!(reason, CloseReason::NoAnswer);
    assert_eq!(at, Duration::from_secs(10));
    let requests = w
        .net
        .take_trace()
        .iter()
        .filter(|t| t.from == player_addr(0))
        .count();
    assert_eq!(requests, 40);
}

#[test]
fn silence_times_out_both_sides_after_five_seconds() {
    let mut w = World::new(5, LinkConfig::one_way(20 * MS));
    w.join("Viper");
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    w.run_for(Duration::from_secs(1));
    let cut = w.now();
    w.net.set_default_link(LinkConfig {
        loss: 1.0,
        ..LinkConfig::PERFECT
    });
    w.run_for(Duration::from_secs(6));
    let timeout = CloseReason::Disconnected {
        reason: DisconnectReason::Timeout,
        by_peer: false,
    };
    let (at, reason) = closed(&w.players[0].events).expect("client closed");
    assert_eq!(reason, timeout);
    let waited = at - cut;
    assert!(
        waited >= Duration::from_secs(4) && waited <= Duration::from_secs(5),
        "{waited:?}"
    );
    let host = host_closed(&w.host_events);
    assert_eq!(host.len(), 1);
    assert_eq!(host[0].1, timeout);
}

#[test]
fn a_disconnect_reaches_the_other_side_with_its_reason() {
    let mut w = World::new(6, LinkConfig::for_round_trip(100 * MS, 0.1, 0.3, 0.0));
    w.net.start_trace();
    w.join("Viper");
    assert!(w.run_until(Duration::from_secs(10), |w| w.connected(0)));
    w.players[0].client.disconnect(DisconnectReason::Left);
    w.run_for(Duration::from_millis(200));
    let disconnects = w
        .net
        .take_trace()
        .iter()
        .filter(|t| t.datagram[4] == PacketKind::Disconnect as u8)
        .count();
    assert_eq!(disconnects, 3);
    // With 30 percent loss, one of the three got through at this seed.
    let host = host_closed(&w.host_events);
    assert_eq!(host.len(), 1);
    assert_eq!(
        host[0].1,
        CloseReason::Disconnected {
            reason: DisconnectReason::Left,
            by_peer: true
        }
    );
    assert_eq!(
        closed(&w.players[0].events).unwrap().1,
        CloseReason::Disconnected {
            reason: DisconnectReason::Left,
            by_peer: false
        }
    );
    // The host ends a connection the same way.
    let mut w = World::new(7, LinkConfig::one_way(10 * MS));
    w.join("Viper");
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    let id = w.connection_of(0);
    w.server.disconnect(id, DisconnectReason::Kicked);
    w.run_for(Duration::from_millis(100));
    assert_eq!(
        closed(&w.players[0].events).unwrap().1,
        CloseReason::Disconnected {
            reason: DisconnectReason::Kicked,
            by_peer: true
        }
    );
}

/// The Challenge answer the player sent, from the trace.
fn sent_answer(w: &World) -> Vec<u8> {
    w.net
        .take_trace()
        .into_iter()
        .find(|t| t.datagram[4] == PacketKind::ChallengeAnswer as u8)
        .expect("an answer")
        .datagram
}

#[test]
fn a_repeated_answer_gets_the_same_accepted_and_no_second_connection() {
    let mut w = World::new(8, LinkConfig::one_way(10 * MS));
    w.net.start_trace();
    w.join("Viper");
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    let answer = sent_answer(&w);
    w.net.inject(player_addr(0), host_addr(), &answer);
    w.step(MS);
    let replies: Vec<Vec<u8>> = w
        .net
        .take_trace()
        .into_iter()
        .filter(|t| t.datagram[4] == PacketKind::Accepted as u8)
        .map(|t| t.datagram)
        .collect();
    assert_eq!(replies.len(), 1);
    let Packet::Accepted(again) = Packet::decode(&replies[0], VERSION).unwrap() else {
        panic!("not accepted")
    };
    assert_eq!(again.connection, w.connection_of(0).0);
    assert_eq!(connected_count(&w.host_events), 1);
    assert_eq!(w.gate.seen.len(), 1);
    w.run_for(Duration::from_secs(1));
    assert_eq!(w.players[0].client.state(), ClientState::Connected);
}

#[test]
fn a_new_join_from_the_same_address_replaces_the_old_connection() {
    let mut w = World::new(9, LinkConfig::one_way(10 * MS));
    w.join("Viper");
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    let old = w.connection_of(0);
    // The game restarts on the same port with a new nonce.
    let player = w.players.pop().unwrap();
    drop(player);
    let mut config = w.player_config("Viper", 0);
    config.entropy = tore_net::Entropy::Seeded(999);
    w.join_with(config, 0);
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    assert_ne!(w.connection_of(0), old);
    let closes = host_closed(&w.host_events);
    assert_eq!(closes.len(), 1);
    assert_eq!(closes[0].1, CloseReason::Replaced);
    assert_eq!(w.server.connections().count(), 1);
}

#[test]
fn a_stale_connection_id_is_dropped_without_counting_as_bad() {
    let mut w = World::new(10, LinkConfig::one_way(10 * MS));
    w.join("Viper");
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    let id = w.connection_of(0);
    let header = PayloadHeader {
        connection: id.0 ^ 1,
        sequence: 5_000,
        ack: 0,
        ack_bits: 0,
        ack_delay: ACK_DELAY_NONE,
    };
    let forged = Packet::Payload(
        header,
        vec![Section {
            kind: 2,
            body: vec![1, 2, 3],
        }],
    )
    .encode(VERSION)
    .unwrap();
    for _ in 0..100 {
        w.net.inject(player_addr(0), host_addr(), &forged);
    }
    w.step(MS);
    assert_eq!(w.server.counters().stale, 100);
    let stats = w.server.stats(id).unwrap();
    assert_eq!(stats.bad_packets, 0);
    assert!(!w.host_events.iter().any(|(_, e)| matches!(
        e,
        ServerEvent::Connection {
            event: Event::Payload { .. },
            ..
        }
    )));
    w.run_for(Duration::from_secs(1));
    assert_eq!(w.players[0].client.state(), ClientState::Connected);
}

#[test]
fn connect_requests_are_rate_limited_per_address() {
    let mut w = World::new(11, LinkConfig::one_way(10 * MS));
    w.net.start_trace();
    w.step(MS);
    let from = player_addr(3);
    let _socket = w.net.bind(from).unwrap();
    for nonce in 0..30u64 {
        let request = Packet::ConnectRequest(ConnectRequest {
            protocol_version: VERSION,
            nonce,
            game_version: String::new(),
            game_commit: String::new(),
        })
        .encode(VERSION)
        .unwrap();
        w.net.inject(from, host_addr(), &request);
    }
    w.step(MS);
    let challenges = w
        .net
        .take_trace()
        .iter()
        .filter(|t| t.datagram[4] == PacketKind::Challenge as u8)
        .count();
    assert_eq!(challenges, RATE_LIMIT_PER_ADDRESS as usize);
    assert_eq!(w.server.counters().rate_limited, 10);
}

#[test]
fn a_cookie_expires_after_its_slot_and_the_next() {
    let mut w = World::new(12, LinkConfig::one_way(10 * MS));
    w.net.start_trace();
    w.join("Viper");
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    let answer = sent_answer(&w);
    let Packet::ChallengeAnswer(ChallengeAnswer { .. }) = Packet::decode(&answer, VERSION).unwrap()
    else {
        panic!("not an answer")
    };
    w.players[0].client.disconnect(DisconnectReason::Left);
    w.run_for(Duration::from_secs(1));
    // Within 10 to 20 seconds the old cookie is still good (previous slot);
    // after 20 it is not.
    w.run_for(Duration::from_secs(20));
    w.net.inject(player_addr(0), host_addr(), &answer);
    w.step(MS);
    assert_eq!(w.server.counters().bad_cookie, 1);
    assert_eq!(connected_count(&w.host_events), 1);
}

/// A Payload from player 0 with the right id, a new sequence and `sections`.
fn forged_payload(w: &World, sequence: u16, sections: Vec<Section>) -> Vec<u8> {
    let header = PayloadHeader {
        connection: w.connection_of(0).0,
        sequence,
        ack: 0,
        ack_bits: 0,
        ack_delay: ACK_DELAY_NONE,
    };
    Packet::Payload(header, sections).encode(VERSION).unwrap()
}

#[test]
fn fifty_bad_packets_in_five_seconds_end_the_connection() {
    let mut w = World::new(13, LinkConfig::one_way(10 * MS));
    w.join("Viper");
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    for i in 0..49u16 {
        // Section kind 9 is unknown in protocol 1.
        let bad = forged_payload(
            &w,
            1_000 + i,
            vec![Section {
                kind: 9,
                body: vec![],
            }],
        );
        w.net.inject(player_addr(0), host_addr(), &bad);
        w.step(50 * MS);
    }
    let id = w.connection_of(0);
    assert_eq!(w.server.stats(id).unwrap().bad_packets, 49);
    assert!(host_closed(&w.host_events).is_empty());
    let bad = forged_payload(
        &w,
        2_000,
        vec![Section {
            kind: 9,
            body: vec![],
        }],
    );
    w.net.inject(player_addr(0), host_addr(), &bad);
    w.run_for(Duration::from_millis(100));
    assert_eq!(
        host_closed(&w.host_events)[0].1,
        CloseReason::Disconnected {
            reason: DisconnectReason::BadPackets,
            by_peer: false
        }
    );
    assert_eq!(
        closed(&w.players[0].events).unwrap().1,
        CloseReason::Disconnected {
            reason: DisconnectReason::BadPackets,
            by_peer: true
        }
    );
}

#[test]
fn a_section_the_caller_rejects_drops_the_whole_packet_unacknowledged() {
    let mut w = World::new(14, LinkConfig::one_way(10 * MS));
    w.join("Viper");
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    w.gate.reject_section_kind = Some(3);
    w.players[0].client.send_message(1, b"carried").unwrap();
    let now = w.now();
    let sequence = w.players[0]
        .client
        .send_payload(now, &[(2, b"fine"), (3, b"rejected")])
        .unwrap();
    w.run_for(Duration::from_millis(25));
    let id = w.connection_of(0);
    assert_eq!(w.server.stats(id).unwrap().bad_packets, 1);
    // Nothing from that packet reached the host.
    assert!(!w.host_events.iter().any(|(_, e)| matches!(
        e,
        ServerEvent::Connection { event: Event::Payload { sequence: s, .. }, .. } if *s == sequence
    )));
    // It was never acknowledged; the message it carried came again later.
    w.gate.reject_section_kind = None;
    w.run_for(Duration::from_secs(5));
    assert!(w.players[0].events.iter().any(|(_, e)| matches!(
        e,
        ClientEvent::Connection(Event::Lost { sequence: s }) if *s == sequence
    )));
    assert!(w.host_events.iter().any(|(_, e)| matches!(
        e,
        ServerEvent::Connection { event: Event::Message { body, .. }, .. } if body == b"carried"
    )));
}

#[test]
fn a_message_beyond_the_window_is_a_protocol_error() {
    let mut w = World::new(15, LinkConfig::one_way(10 * MS));
    w.join("Viper");
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    // One whole message with id 300, when 0 is expected.
    let mut body = BitWriter::new();
    body.write_bits(1, 8).unwrap();
    body.write_bits(300, 16).unwrap();
    body.write_bits(0, 2).unwrap();
    body.write_bits(1, 8).unwrap();
    body.write_bits(0, 9).unwrap();
    let forged = forged_payload(
        &w,
        20_000,
        vec![Section {
            kind: 1,
            body: body.finish(),
        }],
    );
    w.net.inject(player_addr(0), host_addr(), &forged);
    w.run_for(Duration::from_millis(50));
    assert_eq!(
        host_closed(&w.host_events)[0].1,
        CloseReason::Disconnected {
            reason: DisconnectReason::ProtocolError,
            by_peer: false
        }
    );
}

#[test]
fn keepalives_keep_each_side_at_ten_packets_a_second() {
    let mut w = World::new(16, LinkConfig::one_way(30 * MS));
    w.join("Viper");
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    w.run_for(Duration::from_secs(3));
    let client = w.players[0].client.stats().unwrap();
    let host = w.server.stats(w.connection_of(0)).unwrap();
    assert_eq!(client.packets_sent_per_second, 10);
    assert_eq!(host.packets_sent_per_second, 10);
    assert_eq!(client.packets_received_per_second, 10);
    assert_eq!(client.bytes_sent_per_second, 10 * 19);
    assert!(client.round_trip_measured);
    // 60 ms of flight; the ack delay removes the wait for the next keepalive.
    let rtt = client.round_trip.as_secs_f64();
    assert!((rtt - 0.060).abs() < 0.002, "{rtt}");
    assert_eq!(client.loss, Some(0.0));
}
