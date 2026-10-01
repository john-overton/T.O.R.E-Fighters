//! Slice EF-K: a joined game whose loop is stalled is kept connected by its
//! Keepalive packets, and only by its own: from its own address, with its
//! own connection id. On the network simulator, on its virtual clock.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::{MS, VERSION, World, host_addr, player_addr};
use tore_net::packet::{Keepalive, Packet};
use tore_net::sim::{LinkConfig, SimSocket};
use tore_net::{CloseReason, Datagrams, DisconnectReason, ServerEvent};

/// The host alone steps for `time`, 1 ms at a time; the players' games are
/// stalled. `speak` is called each step with the time since the stall began
/// and may send datagrams for them.
fn stall(w: &mut World, time: Duration, mut speak: impl FnMut(&mut World, Duration)) {
    let start = w.now();
    let end = start + time;
    while w.now() < end {
        w.net.advance(MS);
        let now = w.now();
        speak(w, now - start);
        w.server
            .receive_from(&mut w.host_socket, now, &mut w.gate)
            .unwrap();
        w.server.update(now);
        w.server.transmit(&mut w.host_socket).unwrap();
        while let Some(event) = w.server.poll_event() {
            w.host_events.push((now, event));
        }
    }
}

/// One keepalive a second, as the thread sends them.
fn every_second(elapsed: Duration) -> bool {
    elapsed.as_millis().is_multiple_of(1000) && elapsed >= Duration::from_secs(1)
}

fn closed_on_host(w: &World) -> Vec<(Duration, CloseReason)> {
    w.host_events
        .iter()
        .filter_map(|(at, e)| match e {
            ServerEvent::Closed { reason, .. } => Some((*at, reason.clone())),
            _ => None,
        })
        .collect()
}

fn joined() -> (World, Vec<u8>) {
    let mut w = World::new(31, LinkConfig::one_way(20 * MS));
    w.join("Viper");
    assert!(w.run_until(Duration::from_secs(1), |w| w.connected(0)));
    w.run_for(Duration::from_secs(1));
    let datagram = w.players[0]
        .client
        .keepalive_datagram()
        .expect("joined: a keepalive");
    (w, datagram)
}

fn send(socket: &mut SimSocket, datagram: &[u8]) {
    socket.send_datagram(host_addr(), datagram).unwrap();
}

/// Acceptance: a game stalled for 15 seconds, speaking once a second through
/// its keepalive, is not dropped; the host hears it and nothing else, and the
/// game carries on when it resumes.
#[test]
fn a_fifteen_second_stall_with_keepalives_is_not_dropped() {
    let (mut w, datagram) = joined();
    assert_eq!(datagram.len(), 9);
    let connection = w.connection_of(0);
    let before = w.server.stats(connection).unwrap();
    let mut player = w.players.remove(0);
    stall(&mut w, Duration::from_secs(15), |_, elapsed| {
        if every_second(elapsed) {
            send(&mut player.socket, &datagram);
        }
    });
    assert!(closed_on_host(&w).is_empty(), "{:?}", w.host_events);
    let stats = w.server.stats(connection).expect("still connected");
    let heard = stats.keepalives - before.keepalives;
    assert!((13..=15).contains(&heard), "{heard} keepalives heard");
    // Heard, and nothing else: no packet counted, no round trip sampled.
    assert!(stats.since_last_received < Duration::from_millis(1100));
    assert_eq!(stats.packets_received_per_second, 0);
    assert_eq!(stats.bytes_received_per_second, 0);
    assert_eq!(stats.bad_packets, before.bad_packets);
    // The game resumes: it reads what waited for it and carries on, joined
    // on both sides.
    w.players.push(player);
    w.run_for(Duration::from_secs(3));
    assert!(closed_on_host(&w).is_empty(), "{:?}", w.host_events);
    assert!(
        !w.players[0]
            .events
            .iter()
            .any(|(_, e)| matches!(e, tore_net::ClientEvent::Closed(_))),
        "the client closed"
    );
    let stats = w.server.stats(connection).unwrap();
    assert!(stats.packets_received_per_second >= 9, "{stats:?}");
}

/// The host answers a keepalive with nothing at all.
#[test]
fn a_keepalive_is_never_answered() {
    let (mut w, datagram) = joined();
    let now = w.now();
    // Drain what the host had queued, then hand it a keepalive alone.
    w.server.update(now);
    while w.server.poll_transmit().is_some() {}
    w.server
        .receive(now, player_addr(0), &datagram, &mut w.gate);
    assert!(w.server.poll_transmit().is_none());
    assert!(w.server.poll_event().is_none());
}

/// Without keepalives a stalled game is dropped after 5 seconds, as before.
#[test]
fn a_silent_stall_is_still_dropped() {
    let (mut w, _) = joined();
    let _stalled = w.players.remove(0);
    stall(&mut w, Duration::from_secs(7), |_, _| {});
    let closed = closed_on_host(&w);
    assert_eq!(closed.len(), 1, "{closed:?}");
    assert_eq!(
        closed[0].1,
        CloseReason::Disconnected {
            reason: DisconnectReason::Timeout,
            by_peer: false,
        }
    );
}

/// A keepalive with the right connection id from any other address, or from
/// the connection's address with another id, or of another protocol
/// version, keeps nothing alive: the stalled game is dropped after 5 seconds
/// as if they had not been sent.
#[test]
fn a_keepalive_from_elsewhere_or_with_another_identity_keeps_nothing_alive() {
    let other_address: SocketAddr = player_addr(9);
    for case in ["other address", "other id", "other version"] {
        let (mut w, datagram) = joined();
        let connection = w.connection_of(0);
        let mut player = w.players.remove(0);
        let mut stranger = w.net.bind(other_address).unwrap();
        let forged = match case {
            "other id" => Packet::Keepalive(Keepalive {
                connection: connection.0 ^ 1,
            })
            .encode(VERSION)
            .unwrap(),
            "other version" => Packet::Keepalive(Keepalive {
                connection: connection.0,
            })
            .encode(VERSION + 1)
            .unwrap(),
            _ => datagram.clone(),
        };
        let counters = *w.server.counters();
        stall(&mut w, Duration::from_secs(7), |_, elapsed| {
            if elapsed.as_millis().is_multiple_of(250) {
                match case {
                    "other address" => send(&mut stranger, &forged),
                    _ => send(&mut player.socket, &forged),
                }
            }
        });
        let closed = closed_on_host(&w);
        assert_eq!(closed.len(), 1, "{case}: {closed:?}");
        assert_eq!(
            closed[0].1,
            CloseReason::Disconnected {
                reason: DisconnectReason::Timeout,
                by_peer: false,
            },
            "{case}"
        );
        let after = *w.server.counters();
        let (field, before, now) = match case {
            "other address" => (
                "unknown address",
                counters.unknown_address,
                after.unknown_address,
            ),
            "other id" => ("stale", counters.stale, after.stale),
            _ => ("invalid", counters.invalid, after.invalid),
        };
        assert!(now >= before + 19, "{case}: {field} {before} to {now}");
    }
}
