//! The D2 acceptance on the simulator: 300 ms round trip, arrivals spread by
//! plus or minus 10 percent of the one-way delay, 5 percent loss and 1
//! percent duplicates each way.

mod common;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use common::{MS, World, host_addr, player_addr};
use tore_net::sim::{BurstLoss, LinkConfig, TraceEntry};
use tore_net::{ClientEvent, Event, ServerEvent};

const MESSAGES: usize = 10_000;
const ROUND_TRIP: Duration = Duration::from_millis(300);

/// Message `i`: its index, then filler. Every 97th is 1,000 bytes, so it
/// travels as four fragments; the rest are 4 to 253 bytes.
fn message(i: usize) -> Vec<u8> {
    let len = if i.is_multiple_of(97) {
        1000
    } else {
        4 + (i * 7919) % 250
    };
    let mut body = vec![(i % 251) as u8; len];
    body[..4].copy_from_slice(&(i as u32).to_le_bytes());
    body
}

fn big_message() -> Vec<u8> {
    (0..tore_net::MAX_MESSAGE_LEN)
        .map(|i| (i * 13 + 7) as u8)
        .collect()
}

fn micros(body: &[u8]) -> Duration {
    Duration::from_micros(u64::from_le_bytes(body[..8].try_into().unwrap()))
}

fn stamp(now: Duration) -> [u8; 8] {
    (now.as_micros() as u64).to_le_bytes()
}

/// Which Payloads from `from` to `to` the receiver could acknowledge, by
/// sequence: replaying the trace in arrival order (ties in send order), a
/// packet counts when it arrived, was not a duplicate and was at most 32
/// sequences behind the newest one taken. Before the client has taken an
/// Accepted it drops the host's Payloads.
fn acknowledgeable(
    trace: &[TraceEntry],
    from: SocketAddr,
    to: SocketAddr,
    wait_for_accepted: bool,
) -> HashMap<u16, bool> {
    let mut arrivals = Vec::new();
    let mut result = HashMap::new();
    let mut payloads = 0;
    for (index, t) in trace.iter().enumerate() {
        if t.from != from || t.to != to {
            continue;
        }
        let kind = t.datagram[4];
        let seq = u16::from_le_bytes([t.datagram[9], t.datagram[10]]);
        if kind == 6 {
            payloads += 1;
            result.insert(seq, false);
        }
        for (copy, at) in t.arrivals.iter().enumerate() {
            arrivals.push((*at, index, copy, kind, seq));
        }
    }
    assert!(payloads < 60_000, "sequences would wrap");
    arrivals.sort();
    let mut connected = !wait_for_accepted;
    let mut newest: Option<u16> = None;
    let mut taken = std::collections::HashSet::new();
    for (_, _, _, kind, seq) in arrivals {
        if kind == 4 {
            connected = true;
        }
        if kind != 6 || !connected {
            continue;
        }
        let take = match newest {
            None => true,
            Some(n) if tore_net::sequence_newer(seq, n) => true,
            Some(n) => {
                let back = n.wrapping_sub(seq);
                back != 0 && back <= 32 && !taken.contains(&seq)
            }
        };
        if take {
            if newest.is_none_or(|n| tore_net::sequence_newer(seq, n)) {
                newest = Some(seq);
            }
            taken.insert(seq);
            result.insert(seq, true);
        }
    }
    result
}

/// A judged packet: when, its sequence, and whether it was lost.
type Judged = (Duration, u16, bool);

/// Lost over judged in the 5 seconds before `now`, as the connection counts.
fn window_loss(judged: &[Judged], now: Duration) -> Option<f64> {
    let recent: Vec<&Judged> = judged
        .iter()
        .filter(|(at, _, _)| now.saturating_sub(*at) <= Duration::from_secs(5))
        .collect();
    let lost = recent.iter().filter(|(_, _, lost)| *lost).count();
    (!recent.is_empty()).then(|| lost as f64 / recent.len() as f64)
}

struct Outcome {
    to_host: Vec<Vec<u8>>,
    to_client: Vec<Vec<u8>>,
    big: Option<Vec<u8>>,
    client_judged: Vec<Judged>,
    host_judged: Vec<Judged>,
    /// (client round trip, host round trip, client loss, host loss, window
    /// loss from the client's own events, from the host's)
    samples: Vec<(Duration, Duration, f64, f64, f64, f64)>,
    host_arrival_order: Vec<u16>,
    spread: Duration,
    trace: Vec<TraceEntry>,
    elapsed: Duration,
}

/// Joins, then streams 10,000 messages each way, one 64 KB message to the
/// client, 60 input packets a second to the host and 30 snapshot packets a
/// second back, until everything arrived or `limit` passes.
fn run(seed: u64, link: LinkConfig, limit: Duration) -> Outcome {
    let mut w = World::new(seed, link);
    w.net.start_trace();
    w.join("Viper");
    assert!(
        w.run_until(Duration::from_secs(10), |w| w.connected(0)),
        "joined"
    );
    let id = w.connection_of(0);
    let start = w.now();
    let big = big_message();
    let (mut next_to_host, mut next_to_client, mut big_sent) = (0, 0, false);
    let (mut client_seen, mut host_seen) = (w.players[0].events.len(), w.host_events.len());
    let mut out = Outcome {
        to_host: Vec::new(),
        to_client: Vec::new(),
        big: None,
        client_judged: Vec::new(),
        host_judged: Vec::new(),
        samples: Vec::new(),
        host_arrival_order: Vec::new(),
        spread: Duration::ZERO,
        trace: Vec::new(),
        elapsed: Duration::ZERO,
    };
    let (mut last_input, mut last_snapshot) = (start, start);
    let mut next_sample = start + Duration::from_secs(10);
    while w.now() < start + limit {
        let now = w.now();
        // Keep a few hundred messages queued each way.
        while next_to_host < MESSAGES && w.players[0].client.stats().unwrap().messages_queued < 600
        {
            w.players[0]
                .client
                .send_message(1, &message(next_to_host))
                .unwrap();
            next_to_host += 1;
        }
        while next_to_client < MESSAGES && w.server.stats(id).unwrap().messages_queued < 600 {
            w.server
                .send_message(id, 2, &message(next_to_client))
                .unwrap();
            next_to_client += 1;
        }
        if !big_sent && now >= start + Duration::from_secs(5) {
            w.server.send_message(id, 9, &big).unwrap();
            big_sent = true;
        }
        // Game traffic stamped with its send time.
        if now - last_input >= Duration::from_micros(16_667) {
            last_input = now;
            w.players[0]
                .client
                .send_payload(now, &[(2, &stamp(now))])
                .unwrap();
        }
        if now - last_snapshot >= Duration::from_micros(33_333) {
            last_snapshot = now;
            w.server.send_payload(now, id, &[(3, &stamp(now))]).unwrap();
        }
        w.step(MS);
        let now = w.now();
        let client_events: Vec<(Duration, ClientEvent)> =
            w.players[0].events[client_seen..].to_vec();
        client_seen = w.players[0].events.len();
        for (at, event) in client_events {
            let ClientEvent::Connection(event) = event else {
                panic!("unexpected {event:?}")
            };
            match event {
                Event::Message { kind: 2, body } => out.to_client.push(body),
                Event::Message { kind: 9, body } => {
                    assert!(out.big.is_none(), "the big message arrived twice");
                    out.big = Some(body);
                }
                Event::Payload { sections, .. } => {
                    assert_eq!(sections.len(), 1);
                    w.players[0]
                        .client
                        .note_arrival(micros(&sections[0].body), at);
                }
                Event::Delivered { sequence } => out.client_judged.push((at, sequence, false)),
                Event::Lost { sequence } => out.client_judged.push((at, sequence, true)),
                other => panic!("unexpected {other:?}"),
            }
        }
        let host_events: Vec<(Duration, ServerEvent)> = w.host_events[host_seen..].to_vec();
        host_seen = w.host_events.len();
        for (at, event) in host_events {
            let ServerEvent::Connection { event, .. } = event else {
                panic!("unexpected {event:?}")
            };
            match event {
                Event::Message { kind: 1, body } => out.to_host.push(body),
                Event::Payload { sequence, sections } => {
                    out.host_arrival_order.push(sequence);
                    w.server.note_arrival(id, micros(&sections[0].body), at);
                }
                Event::Delivered { sequence } => out.host_judged.push((at, sequence, false)),
                Event::Lost { sequence } => out.host_judged.push((at, sequence, true)),
                other => panic!("unexpected {other:?}"),
            }
        }
        if now >= next_sample {
            next_sample += Duration::from_secs(1);
            let client = w.players[0].client.stats().unwrap();
            let host = w.server.stats(id).unwrap();
            out.samples.push((
                client.round_trip,
                host.round_trip,
                client.loss.unwrap(),
                host.loss.unwrap(),
                window_loss(&out.client_judged, now).unwrap(),
                window_loss(&out.host_judged, now).unwrap(),
            ));
        }
        if out.to_host.len() == MESSAGES && out.to_client.len() == MESSAGES && out.big.is_some() {
            break;
        }
    }
    out.elapsed = w.now() - start;
    out.spread = w.players[0].client.stats().unwrap().spread;
    out.trace = w.net.take_trace();
    out
}

#[test]
fn acceptance_at_300_ms_and_5_percent_loss() {
    let link = LinkConfig::for_round_trip(ROUND_TRIP, 0.1, 0.05, 0.01);
    let out = run(20_260_930, link, Duration::from_secs(120));

    // Every message arrived once and in order, and the large one whole.
    assert_eq!(out.to_host.len(), MESSAGES, "after {:?}", out.elapsed);
    assert_eq!(out.to_client.len(), MESSAGES);
    for (i, body) in out.to_host.iter().enumerate() {
        assert_eq!(body, &message(i), "message {i} to the host");
    }
    for (i, body) in out.to_client.iter().enumerate() {
        assert_eq!(body, &message(i), "message {i} to the client");
    }
    assert_eq!(out.big.as_deref(), Some(big_message().as_slice()));

    // The link really lost, duplicated and reordered.
    let lost = out.trace.iter().filter(|t| t.copies == 0).count();
    let doubled = out.trace.iter().filter(|t| t.copies == 2).count();
    assert!(lost > 100 && doubled > 10, "lost {lost}, doubled {doubled}");
    let reordered = out
        .host_arrival_order
        .windows(2)
        .filter(|w| tore_net::sequence_newer(w[0], w[1]))
        .count();
    assert!(reordered > 10, "reordered {reordered}");

    // Every packet judged delivered was taken by the receiver, and every one
    // judged lost was not.
    for (judged, from, to, wait) in [
        (&out.client_judged, player_addr(0), host_addr(), false),
        (&out.host_judged, host_addr(), player_addr(0), true),
    ] {
        let truth = acknowledgeable(&out.trace, from, to, wait);
        let late = judged
            .iter()
            .filter(|(_, s, lost)| *lost && truth[s])
            .count();
        for (_, sequence, lost) in judged.iter() {
            assert_eq!(truth[sequence], !lost, "sequence {sequence} from {from}");
        }
        assert_eq!(late, 0);
    }

    // The round trip within 5 percent at every sample. It runs a little low
    // (1 to 4 percent at this seed and others): only the receiver's newest
    // packet carries an ack delay, and under reordering the newest is more
    // often a fast one.
    assert!(out.samples.len() >= 5);
    for (client_rtt, host_rtt, ..) in &out.samples {
        for rtt in [client_rtt, host_rtt] {
            let error = (rtt.as_secs_f64() / ROUND_TRIP.as_secs_f64() - 1.0).abs();
            assert!(error < 0.05, "round trip {rtt:?}");
        }
    }
    // The loss estimate is exact: at every sample it equals the share of this
    // side's packets judged in the last 5 seconds that the receiver never
    // took, which the check above ties to the simulator's own record. Over
    // the whole run that share is within 1 point of the link's 5 percent; a
    // single 5-second window swings by more (2 to 8 percent across seeds), as
    // a few hundred packets' worth of 5 percent loss does.
    for (_, _, client_loss, host_loss, client_truth, host_truth) in &out.samples {
        assert!((client_loss - client_truth).abs() < 1e-12);
        assert!((host_loss - host_truth).abs() < 1e-12);
    }
    let whole_run =
        |judged: &[Judged]| judged.iter().filter(|j| j.2).count() as f64 / judged.len() as f64;
    let (client_mean, host_mean) = (whole_run(&out.client_judged), whole_run(&out.host_judged));
    assert!(
        (client_mean - 0.05).abs() < 0.01,
        "client loss {client_mean}"
    );
    assert!((host_mean - 0.05).abs() < 0.01, "host loss {host_mean}");

    // Arrival spread: two uniform plus or minus 15 ms delays differ by 10 ms
    // on average.
    let spread = out.spread.as_secs_f64() * 1000.0;
    assert!((7.0..13.0).contains(&spread), "spread {spread} ms");

    eprintln!(
        "{MESSAGES} messages each way and 64 KB in {:.1} s simulated; {} samples; \
         round trip client {:?}..{:?}; loss whole run client {client_mean:.4} host {host_mean:.4}, \
         client range {:.4}..{:.4}; spread {spread:.2} ms; reordered {reordered}; lost {lost}; doubled {doubled}",
        out.elapsed.as_secs_f64(),
        out.samples.len(),
        out.samples.iter().map(|s| s.0).min().unwrap(),
        out.samples.iter().map(|s| s.0).max().unwrap(),
        out.samples.iter().map(|s| s.2).fold(1.0, f64::min),
        out.samples.iter().map(|s| s.2).fold(0.0, f64::max),
    );
}

#[test]
fn burst_loss_still_delivers_everything_in_order() {
    let link = LinkConfig {
        burst: Some(BurstLoss {
            enter: 0.01,
            leave: 0.25,
            loss: 0.9,
        }),
        ..LinkConfig::for_round_trip(Duration::from_millis(150), 0.1, 0.02, 0.01)
    };
    let out = run(7, link, Duration::from_secs(120));
    assert_eq!(out.to_host.len(), MESSAGES);
    assert_eq!(out.to_client.len(), MESSAGES);
    assert!(
        out.to_host
            .iter()
            .enumerate()
            .all(|(i, b)| *b == message(i))
    );
    assert!(
        out.to_client
            .iter()
            .enumerate()
            .all(|(i, b)| *b == message(i))
    );
    assert_eq!(out.big.as_deref(), Some(big_message().as_slice()));
}

#[test]
fn a_seed_repeats_the_whole_session_exactly() {
    let link = LinkConfig::for_round_trip(ROUND_TRIP, 0.1, 0.05, 0.01);
    let a = run(3, link, Duration::from_secs(8));
    let b = run(3, link, Duration::from_secs(8));
    assert!(a.trace.len() > 1000);
    assert_eq!(a.trace, b.trace);
    let c = run(4, link, Duration::from_secs(8));
    assert_ne!(a.trace, c.trace);
}
