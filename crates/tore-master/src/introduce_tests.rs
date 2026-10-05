//! Introductions on the master (slice J2): a scripted host and player on
//! the network simulator.

use std::net::SocketAddr;
use std::time::Duration;

use tore_net::master::{
    Build, Candidate, CandidateKind, Hint, Introduce, Introduction, IntroductionResult,
    ListingSummary, MappingType, MasterPacket, Meet, MeetAck, Probe, Register,
};
use tore_net::packet::DiscoverPhase;
use tore_net::sim::{SimNetwork, SimSocket};
use tore_net::{Datagrams, Entropy};

use crate::introduce::PER_LISTING;
use crate::master::{Master, MasterPort, Settings};

const MAIN: &str = "198.51.100.1:26901";
const PROBE: &str = "198.51.100.1:26902";
const HOST: &str = "203.0.113.10:26900";
const PLAYER: &str = "203.0.113.20:40000";

fn a(text: &str) -> SocketAddr {
    text.parse().unwrap()
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn build() -> Build {
    Build {
        protocol_version: 9,
        game_version: "0.1.3".into(),
        game_commit: "abc".into(),
        release: true,
    }
}

struct Rig {
    net: SimNetwork,
    master: Master,
    main: SimSocket,
    probe: SimSocket,
}

impl Rig {
    fn new() -> Self {
        let net = SimNetwork::new(5);
        net.set_now(Duration::from_secs(100));
        let main = net.bind(a(MAIN)).unwrap();
        let probe = net.bind(a(PROBE)).unwrap();
        Self {
            master: Master::new(Settings::default(), Entropy::Seeded(9), 0),
            net,
            main,
            probe,
        }
    }

    fn step(&mut self, dt: Duration) {
        self.net.advance(dt);
        let now = self.net.now();
        self.master
            .receive_from(now, MasterPort::Main, &mut self.main)
            .unwrap();
        self.master
            .receive_from(now, MasterPort::Probe, &mut self.probe)
            .unwrap();
        self.master.update(now);
        self.master
            .transmit(&mut self.main, Some(&mut self.probe))
            .unwrap();
    }

    /// A listed host at `address`: its socket and token.
    fn host(&mut self, address: &str, full: bool, mapping: MappingType) -> (SimSocket, u64, u64) {
        let mut socket = self.net.bind(a(address)).unwrap();
        if mapping == MappingType::PortPerDestination {
            // Probes from two outside ports of one address with one nonce.
            let mut other = self
                .net
                .bind(SocketAddr::new(a(address).ip(), 31_000))
                .unwrap();
            send(&mut socket, MAIN, &MasterPacket::Probe(Probe { nonce: 3 }));
            send(&mut other, PROBE, &MasterPacket::Probe(Probe { nonce: 3 }));
            self.step(ms(1));
            received(&mut socket);
            received(&mut other);
        }
        let summary = ListingSummary {
            protocol_version: 9,
            full,
            players: 1,
            capacity: 8,
            phase: DiscoverPhase::Lobby,
            game_version: "0.1.3".into(),
            game_commit: "abc".into(),
            name: "Friday night".into(),
            ..ListingSummary::default()
        };
        let mut register = Register {
            nonce: 1,
            cookie: 0,
            build: build(),
            dedicated: true,
            telemetry: false,
            install_id: 0,
            platform: 3,
            candidates: vec![Candidate::new(
                CandidateKind::Local,
                a("192.168.1.10:26900"),
            )],
            summary,
        };
        send(&mut socket, MAIN, &MasterPacket::Register(register.clone()));
        self.step(ms(1));
        let [(MasterPacket::Challenge(challenge), _)] = received(&mut socket)[..] else {
            panic!("no challenge")
        };
        register.cookie = challenge.cookie;
        send(&mut socket, MAIN, &MasterPacket::Register(register));
        self.step(ms(1));
        let [(MasterPacket::Listed(listed), _)] = received(&mut socket)[..] else {
            panic!("not listed")
        };
        (socket, listed.listing_id, listed.token)
    }
}

fn send(socket: &mut SimSocket, to: &str, packet: &MasterPacket) -> usize {
    let bytes = packet.encode().unwrap();
    socket.send_datagram(a(to), &bytes).unwrap();
    bytes.len()
}

fn received(socket: &mut SimSocket) -> Vec<(MasterPacket, usize)> {
    let mut buf = [0u8; 2_048];
    let mut got = Vec::new();
    while let Some((len, _)) = socket.recv_datagram(&mut buf).unwrap() {
        got.push((MasterPacket::decode(&buf[..len]).unwrap(), len));
    }
    got
}

fn introduce(listing_id: u64, nonce: u64, cookie: u64) -> Introduce {
    Introduce {
        nonce,
        cookie,
        listing_id,
        build: build(),
        mapping: MappingType::SamePort,
        candidates: vec![Candidate::new(
            CandidateKind::Local,
            a("192.168.2.20:40000"),
        )],
    }
}

/// Introduce with the cookie dance; the Introduction, if any came.
fn introduced(rig: &mut Rig, player: &mut SimSocket, request: Introduce) -> Option<Introduction> {
    send(player, MAIN, &MasterPacket::Introduce(request.clone()));
    rig.step(ms(1));
    let got = received(player);
    let cookie = match got[..] {
        [(MasterPacket::Challenge(challenge), 23)] => challenge.cookie,
        [] => return None,
        _ => panic!("{got:?}"),
    };
    send(
        player,
        MAIN,
        &MasterPacket::Introduce(Introduce { cookie, ..request }),
    );
    rig.step(ms(1));
    match received(player).pop() {
        Some((MasterPacket::Introduction(introduction), len)) => {
            assert!(len <= 1_000);
            Some(introduction)
        }
        None => None,
        other => panic!("{other:?}"),
    }
}

fn meets(socket: &mut SimSocket) -> Vec<Meet> {
    received(socket)
        .into_iter()
        .filter_map(|(packet, _)| match packet {
            MasterPacket::Meet(meet) => Some(meet),
            _ => None,
        })
        .collect()
}

#[test]
fn a_forged_introduce_gets_only_a_challenge_and_no_meet() {
    let mut rig = Rig::new();
    let (mut host, listing_id, _) = rig.host(HOST, false, MappingType::SamePort);
    let mut player = rig.net.bind(a(PLAYER)).unwrap();
    // A forger claims the player's address but cannot see the Challenge,
    // so its guesses at the cookie never introduce.
    for cookie in [0, 1, 0xDEAD_BEEF] {
        rig.net.inject(
            a(PLAYER),
            a(MAIN),
            &MasterPacket::Introduce(introduce(listing_id, 7, cookie))
                .encode()
                .unwrap(),
        );
    }
    rig.step(ms(1));
    let got = received(&mut player);
    assert_eq!(got.len(), 3);
    assert!(
        got.iter()
            .all(|(p, len)| matches!(p, MasterPacket::Challenge(_)) && *len == 23)
    );
    rig.step(ms(1_000));
    assert!(meets(&mut host).is_empty());
    assert_eq!(rig.master.introductions().under_way(), 0);
}

#[test]
fn an_introduction_and_its_meet_go_out_at_once_with_the_addresses_in_order() {
    let mut rig = Rig::new();
    let (mut host, listing_id, _) = rig.host(HOST, false, MappingType::SamePort);
    let mut player = rig.net.bind(a(PLAYER)).unwrap();
    let introduction = introduced(&mut rig, &mut player, introduce(listing_id, 7, 0)).unwrap();
    assert_eq!(introduction.result, IntroductionResult::Introduced);
    assert_eq!(introduction.hint, Hint::Race);
    assert_eq!(introduction.seen, a(PLAYER));
    let kinds: Vec<CandidateKind> = introduction
        .host_candidates
        .iter()
        .map(|c| c.kind)
        .collect();
    assert_eq!(kinds, [CandidateKind::Seen, CandidateKind::Local]);
    assert_eq!(introduction.host_candidates[0].address, a(HOST));
    // The Meet is at the host in the same turn.
    let meet = meets(&mut host);
    assert_eq!(meet.len(), 1);
    assert_eq!(meet[0].introduction_id, introduction.introduction_id);
    assert_eq!(meet[0].mapping, MappingType::SamePort);
    assert_eq!(
        meet[0].candidates,
        [
            Candidate::new(CandidateKind::Seen, a(PLAYER)),
            Candidate::new(CandidateKind::Local, a("192.168.2.20:40000")),
        ]
    );
    assert_eq!(rig.master.introductions().introduced, 1);
}

#[test]
fn a_meet_is_sent_three_times_250_ms_apart_until_acknowledged() {
    let mut rig = Rig::new();
    let (mut host, listing_id, token) = rig.host(HOST, false, MappingType::SamePort);
    let mut player = rig.net.bind(a(PLAYER)).unwrap();
    let first = introduced(&mut rig, &mut player, introduce(listing_id, 7, 0)).unwrap();
    // The first Meet went with the Introduction.
    assert_eq!(meets(&mut host).len(), 1);
    let start = rig.net.now() - ms(1);
    let mut at = Vec::new();
    while rig.net.now() < start + Duration::from_secs(2) {
        rig.step(ms(10));
        if !meets(&mut host).is_empty() {
            at.push(rig.net.now() - start);
        }
    }
    // Two more, 250 ms apart (the steps are 10 ms).
    assert_eq!(at.len(), 2, "{at:?}");
    assert!(at[0] >= ms(250) && at[0] < ms(260), "{at:?}");
    assert!(
        at[1] - at[0] >= ms(250) && at[1] - at[0] < ms(260),
        "{at:?}"
    );
    assert_eq!(meets(&mut host).len(), 0);

    // An ack stops the retries; one with the wrong token does not.
    let second = introduced(&mut rig, &mut player, introduce(listing_id, 8, 0)).unwrap();
    assert_ne!(second.introduction_id, first.introduction_id);
    assert_eq!(meets(&mut host).len(), 1);
    send(
        &mut host,
        MAIN,
        &MasterPacket::MeetAck(MeetAck {
            token: token ^ 1,
            introduction_id: second.introduction_id,
        }),
    );
    rig.step(ms(260));
    assert_eq!(meets(&mut host).len(), 1);
    assert_eq!(rig.master.introductions().bad_meet_acks, 1);
    send(
        &mut host,
        MAIN,
        &MasterPacket::MeetAck(MeetAck {
            token,
            introduction_id: second.introduction_id,
        }),
    );
    rig.step(ms(1));
    rig.step(ms(1_000));
    assert!(meets(&mut host).is_empty());
    assert_eq!(rig.master.introductions().meet_acks, 1);
}

#[test]
fn the_same_introduce_again_gets_the_same_introduction_and_no_second_meet() {
    let mut rig = Rig::new();
    let (mut host, listing_id, token) = rig.host(HOST, false, MappingType::SamePort);
    let mut player = rig.net.bind(a(PLAYER)).unwrap();
    let first = introduced(&mut rig, &mut player, introduce(listing_id, 7, 0)).unwrap();
    send(
        &mut host,
        MAIN,
        &MasterPacket::MeetAck(MeetAck {
            token,
            introduction_id: first.introduction_id,
        }),
    );
    rig.step(ms(1));
    meets(&mut host);
    let again = introduced(&mut rig, &mut player, introduce(listing_id, 7, 0)).unwrap();
    assert_eq!(again, first);
    assert!(meets(&mut host).is_empty());
    assert_eq!(rig.master.introductions().repeated, 1);
}

#[test]
fn introductions_are_forgotten_after_30_seconds() {
    let mut rig = Rig::new();
    let (_host, listing_id, _) = rig.host(HOST, false, MappingType::SamePort);
    let mut player = rig.net.bind(a(PLAYER)).unwrap();
    introduced(&mut rig, &mut player, introduce(listing_id, 7, 0)).unwrap();
    assert_eq!(rig.master.introductions().under_way(), 1);
    rig.step(Duration::from_secs(29));
    assert_eq!(rig.master.introductions().under_way(), 1);
    rig.step(Duration::from_secs(1));
    assert_eq!(rig.master.introductions().under_way(), 0);
}

#[test]
fn refusals_say_why() {
    let mut rig = Rig::new();
    let (_host, listing_id, _) = rig.host(HOST, false, MappingType::SamePort);
    let (_full, full_id, _) = rig.host("203.0.113.11:26900", true, MappingType::SamePort);
    let mut player = rig.net.bind(a(PLAYER)).unwrap();
    let refused = |rig: &mut Rig, player: &mut SimSocket, request: Introduce| {
        // An Introduce and its repeat with the cookie are two of the
        // source's four a second.
        rig.step(ms(500));
        let introduction = introduced(rig, player, request).unwrap();
        assert!(introduction.host_candidates.is_empty());
        assert_eq!(introduction.introduction_id, 0);
        (introduction.result, introduction.text)
    };
    assert_eq!(
        refused(&mut rig, &mut player, introduce(12345, 1, 0)),
        (
            IntroductionResult::NoListing,
            "That game is no longer listed.".into()
        )
    );
    let mut other = introduce(listing_id, 2, 0);
    other.build.game_commit = "def".into();
    other.build.release = false;
    other.build.game_version = "0.1.4-1-gdef".into();
    assert_eq!(
        refused(&mut rig, &mut player, other).0,
        IntroductionResult::OtherBuild
    );
    assert_eq!(
        refused(&mut rig, &mut player, introduce(full_id, 3, 0)).0,
        IntroductionResult::Full
    );
    assert_eq!(rig.master.introductions().refused, 3);
}

#[test]
fn thirty_a_minute_per_source_then_too_many() {
    let mut rig = Rig::new();
    let (_host, listing_id, _) = rig.host(HOST, false, MappingType::SamePort);
    let mut player = rig.net.bind(a(PLAYER)).unwrap();
    let mut results = Vec::new();
    for nonce in 0..42 {
        // Four a second is the source's limit before the cookie, and each
        // join sends two: a join every half second.
        rig.step(ms(500));
        let introduction =
            introduced(&mut rig, &mut player, introduce(listing_id, nonce, 0)).expect("an answer");
        results.push(introduction.result);
    }
    // A burst of 30, and one more every 2 seconds: 39 in the first 20
    // seconds at this pace, then too many until the bucket refills.
    let made = results
        .iter()
        .take_while(|r| **r == IntroductionResult::Introduced)
        .count();
    assert_eq!(made, 39, "{results:?}");
    // Then too many, but for one more each time 2 seconds have refilled
    // the bucket.
    let too_many = results[made..]
        .iter()
        .filter(|r| **r == IntroductionResult::TooMany)
        .count();
    assert_eq!(too_many, 2, "{results:?}");
}

#[test]
fn ten_a_second_per_listing_then_dropped() {
    let mut rig = Rig::new();
    let (_host, listing_id, _) = rig.host(HOST, false, MappingType::SamePort);
    let mut answered = 0;
    for i in 0..(PER_LISTING.burst + 5) {
        // Each from another source (another address), so only the listing's
        // limit binds.
        let mut player = rig
            .net
            .bind(SocketAddr::from(([198, 18, 0, i as u8 + 1], 40_000)))
            .unwrap();
        answered +=
            usize::from(introduced(&mut rig, &mut player, introduce(listing_id, 1, 0)).is_some());
    }
    assert_eq!(answered, PER_LISTING.burst as usize);
    assert_eq!(rig.master.introductions().dropped, 5);
}

#[test]
fn two_symmetric_routers_get_the_hint_to_ask_for_the_relay_at_once() {
    let mut rig = Rig::new();
    let (_host, listing_id, _) = rig.host(HOST, false, MappingType::PortPerDestination);
    let mut player = rig.net.bind(a(PLAYER)).unwrap();
    let mut request = introduce(listing_id, 7, 0);
    request.mapping = MappingType::PortPerDestination;
    let introduction = introduced(&mut rig, &mut player, request).unwrap();
    assert_eq!(introduction.host_mapping, MappingType::PortPerDestination);
    assert_eq!(introduction.hint, Hint::RelayNow);
    // A player whose router keeps one port races instead.
    let introduction = introduced(&mut rig, &mut player, introduce(listing_id, 8, 0)).unwrap();
    assert_eq!(introduction.hint, Hint::Race);
}
