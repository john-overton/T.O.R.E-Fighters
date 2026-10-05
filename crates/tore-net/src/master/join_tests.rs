//! The joiner against a scripted master (slice J2).

use std::net::SocketAddr;
use std::time::Duration;

use super::*;
use crate::master::{Challenge, Introduction, ProbeAnswer, Unsupported};
use crate::sim::SimNetwork;

const MASTER: &str = "198.51.100.1:26901";
const SECOND: &str = "198.51.100.1:26902";
const MASTER_2: &str = "198.51.100.2:26901";
const OWN: &str = "192.168.1.20:40000";
const SEEN: &str = "203.0.113.5:40000";

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

fn joiner(now: Duration) -> Joiner {
    let mut joiner = Joiner::new(
        JoinConfig {
            build: build(),
            listing_id: 77,
            entropy: Entropy::Seeded(3),
        },
        now,
    );
    joiner.set_masters(
        vec![a(MASTER), a(MASTER_2)],
        vec![Candidate::new(CandidateKind::Local, a(OWN))],
        now,
    );
    joiner
}

/// What the joiner sends, decoded, with where to.
fn sent(joiner: &mut Joiner) -> Vec<(SocketAddr, MasterPacket, usize)> {
    std::iter::from_fn(|| joiner.poll_transmit())
        .map(|t| {
            let len = t.datagram.len();
            (t.to, MasterPacket::decode(&t.datagram).unwrap(), len)
        })
        .collect()
}

fn give(joiner: &mut Joiner, now: Duration, from: &str, packet: MasterPacket) {
    joiner.receive(now, a(from), &packet.encode().unwrap());
}

fn answer(nonce: u64, port: ProbePort, seen: &str) -> MasterPacket {
    MasterPacket::ProbeAnswer(ProbeAnswer {
        nonce,
        port,
        seen: a(seen),
    })
}

fn probe_nonce(packets: &[(SocketAddr, MasterPacket, usize)]) -> u64 {
    packets
        .iter()
        .find_map(|(_, p, _)| match p {
            MasterPacket::Probe(probe) => Some(probe.nonce),
            _ => None,
        })
        .expect("a probe")
}

fn introduce(packets: &[(SocketAddr, MasterPacket, usize)]) -> Option<(SocketAddr, Introduce)> {
    packets.iter().find_map(|(to, p, len)| match p {
        MasterPacket::Introduce(i) => {
            assert_eq!(*len, super::super::packet::INTRODUCE_LEN);
            Some((*to, i.clone()))
        }
        _ => None,
    })
}

fn introduction(nonce: u64, result: IntroductionResult) -> MasterPacket {
    MasterPacket::Introduction(Introduction {
        nonce,
        result,
        introduction_id: 0xABCD,
        hint: Hint::Race,
        seen: a(SEEN),
        host_mapping: MappingType::SamePort,
        host_candidates: vec![
            Candidate::new(CandidateKind::Seen, a("198.51.100.77:26900")),
            Candidate::new(CandidateKind::Mapped, a("198.51.100.77:26950")),
            Candidate::new(CandidateKind::GlobalIpv6, a("[2001:db8:7::7]:26900")),
            Candidate::new(CandidateKind::Local, a("10.0.0.7:26900")),
        ],
        text: String::new(),
    })
}

#[test]
fn the_mapping_test_then_the_introduce_with_its_cookie_then_the_targets() {
    let start = Duration::from_secs(10);
    let mut joiner = joiner(start);
    assert_eq!(joiner.state(), JoinState::Testing);
    let first = sent(&mut joiner);
    // One Probe to each port, with one nonce.
    let tos: Vec<SocketAddr> = first.iter().map(|(to, _, _)| *to).collect();
    assert_eq!(tos, [a(MASTER), a(SECOND)]);
    let nonce = probe_nonce(&first);
    assert!(
        first
            .iter()
            .all(|(_, p, _)| matches!(p, MasterPacket::Probe(probe) if probe.nonce == nonce))
    );
    // The introduction waits for both answers.
    give(
        &mut joiner,
        start + ms(40),
        MASTER,
        answer(nonce, ProbePort::Main, SEEN),
    );
    assert!(introduce(&sent(&mut joiner)).is_none());
    give(
        &mut joiner,
        start + ms(41),
        SECOND,
        answer(nonce, ProbePort::Second, SEEN),
    );
    assert_eq!(joiner.mapping(), MappingType::SamePort);
    assert_eq!(
        joiner.poll_event(),
        Some(JoinEvent::MappingTested(MappingType::SamePort))
    );
    let (to, asked) = introduce(&sent(&mut joiner)).expect("an Introduce");
    assert_eq!(to, a(MASTER));
    assert_eq!((asked.cookie, asked.listing_id), (0, 77));
    assert_eq!(asked.mapping, MappingType::SamePort);
    assert_eq!(
        asked.candidates,
        [Candidate::new(CandidateKind::Local, a(OWN))]
    );
    // The Challenge's cookie goes back at once.
    give(
        &mut joiner,
        start + ms(80),
        MASTER,
        MasterPacket::Challenge(Challenge {
            nonce: asked.nonce,
            cookie: 99,
        }),
    );
    let (_, again) = introduce(&sent(&mut joiner)).expect("the Introduce again");
    assert_eq!((again.nonce, again.cookie), (asked.nonce, 99));
    give(
        &mut joiner,
        start + ms(120),
        MASTER,
        introduction(asked.nonce, IntroductionResult::Introduced),
    );
    assert_eq!(joiner.state(), JoinState::Introduced);
    let Some(JoinEvent::Introduced(introduced)) = joiner.poll_event() else {
        panic!("no introduction")
    };
    assert_eq!(introduced.introduction_id, 0xABCD);
    assert_eq!(introduced.seen, a(SEEN));
    let paths: Vec<Path> = introduced.targets.iter().map(|t| t.path).collect();
    assert_eq!(
        paths,
        [
            Path::Punched,
            Path::MappedPort,
            Path::Ipv6,
            Path::LocalNetwork
        ]
    );
    assert_eq!(introduced.targets[0].address, a("198.51.100.77:26900"));
    // Nothing more is asked once introduced.
    joiner.update(start + Duration::from_secs(5));
    assert!(sent(&mut joiner).is_empty());
}

#[test]
fn a_seen_address_that_is_the_hosts_own_takes_its_path() {
    let seen_mapped = [
        Candidate::new(CandidateKind::Seen, a("198.51.100.77:26900")),
        Candidate::new(CandidateKind::Mapped, a("198.51.100.77:26900")),
    ];
    let paths: Vec<Path> = targets_of(&seen_mapped).iter().map(|t| t.path).collect();
    assert_eq!(paths, [Path::MappedPort]);
    let seen_v6 = [
        Candidate::new(CandidateKind::Seen, a("[2001:db8:7::7]:26900")),
        Candidate::new(CandidateKind::GlobalIpv6, a("[2001:db8:7::7]:26900")),
        Candidate::new(CandidateKind::Local, a("198.51.100.77:26900")),
    ];
    let paths: Vec<Path> = targets_of(&seen_v6).iter().map(|t| t.path).collect();
    assert_eq!(paths, [Path::Ipv6, Path::LocalNetwork]);
    // A local candidate equal to the seen address does not make it local.
    let seen_local = [
        Candidate::new(CandidateKind::Seen, a("127.0.0.1:26900")),
        Candidate::new(CandidateKind::Local, a("127.0.0.1:26900")),
    ];
    assert_eq!(targets_of(&seen_local).len(), 1);
    assert_eq!(targets_of(&seen_local)[0].path, Path::Punched);
}

#[test]
fn a_missing_second_answer_waits_a_second_and_no_translation_none() {
    let start = Duration::from_secs(10);
    let mut joiner = joiner(start);
    let nonce = probe_nonce(&sent(&mut joiner));
    give(
        &mut joiner,
        start + ms(30),
        MASTER,
        answer(nonce, ProbePort::Main, SEEN),
    );
    joiner.update(start + ms(499));
    assert!(sent(&mut joiner).is_empty());
    // The missing Probe goes once more, then the wait ends at a second.
    joiner.update(start + ms(500));
    let again = sent(&mut joiner);
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].0, a(SECOND));
    joiner.update(start + ms(999));
    assert!(introduce(&sent(&mut joiner)).is_none());
    joiner.update(start + ms(1000));
    assert_eq!(joiner.mapping(), MappingType::Unknown);
    assert!(introduce(&sent(&mut joiner)).is_some());

    // A player seen at its own address needs no second answer.
    let mut joiner = joiner_at(start);
    let nonce = probe_nonce(&sent(&mut joiner));
    give(
        &mut joiner,
        start + ms(30),
        MASTER,
        answer(nonce, ProbePort::Main, OWN),
    );
    assert_eq!(joiner.mapping(), MappingType::NoTranslation);
    assert!(introduce(&sent(&mut joiner)).is_some());
}

fn joiner_at(now: Duration) -> Joiner {
    joiner(now)
}

#[test]
fn a_refusal_ends_the_join_with_its_text() {
    let start = Duration::from_secs(10);
    let mut joiner = joiner(start);
    sent(&mut joiner);
    joiner.update(start + MAPPING_WAIT);
    let (_, asked) = introduce(&sent(&mut joiner)).unwrap();
    let _ = joiner.poll_event();
    let mut refused = introduction(asked.nonce, IntroductionResult::Full);
    if let MasterPacket::Introduction(i) = &mut refused {
        i.host_candidates.clear();
    }
    give(&mut joiner, start + ms(1100), MASTER, refused);
    assert_eq!(joiner.state(), JoinState::Ended);
    assert_eq!(
        joiner.poll_event(),
        Some(JoinEvent::Refused {
            result: IntroductionResult::Full,
            text: "That game is full.".into()
        })
    );
    // An unsupported master says why.
    let mut joiner = joiner_at(start);
    give(
        &mut joiner,
        start,
        MASTER,
        MasterPacket::Unsupported(Unsupported {
            lowest: 2,
            highest: 2,
            text: "Update the game.".into(),
        }),
    );
    assert_eq!(
        joiner.poll_event(),
        Some(JoinEvent::Unsupported("Update the game.".into()))
    );
}

#[test]
fn an_unanswered_introduce_is_repeated_then_the_next_address_then_silent() {
    let start = Duration::from_secs(10);
    let mut joiner = joiner(start);
    sent(&mut joiner);
    let mut to = Vec::new();
    let mut now = start;
    let mut silent_at = None;
    while now < start + Duration::from_secs(20) {
        joiner.update(now);
        for (address, packet, _) in sent(&mut joiner) {
            if matches!(packet, MasterPacket::Introduce(_)) {
                to.push((now - start, address));
            }
        }
        while let Some(event) = joiner.poll_event() {
            if event == JoinEvent::MasterSilent {
                silent_at.get_or_insert(now - start);
            }
        }
        now += ms(50);
    }
    let tries: Vec<SocketAddr> = to.iter().map(|(_, address)| *address).collect();
    assert_eq!(
        tries,
        [
            a(MASTER),
            a(MASTER),
            a(MASTER),
            a(MASTER_2),
            a(MASTER_2),
            a(MASTER_2)
        ]
    );
    // A second apart, from the end of the mapping test's wait.
    assert_eq!(to[0].0, MAPPING_WAIT);
    assert_eq!(to[5].0, MAPPING_WAIT + INTRODUCE_RETRY * 5);
    assert_eq!(silent_at, Some(MAPPING_WAIT + INTRODUCE_RETRY * 6));
    assert_eq!(joiner.state(), JoinState::Ended);
}

#[test]
fn stale_answers_are_counted_and_change_nothing() {
    let start = Duration::from_secs(10);
    let mut joiner = joiner(start);
    sent(&mut joiner);
    give(&mut joiner, start, MASTER, answer(1, ProbePort::Main, SEEN));
    give(
        &mut joiner,
        start,
        MASTER,
        introduction(1, IntroductionResult::Introduced),
    );
    give(
        &mut joiner,
        start,
        MASTER,
        MasterPacket::Challenge(Challenge {
            nonce: 1,
            cookie: 2,
        }),
    );
    joiner.receive(start, a(MASTER), b"not a master packet");
    assert_eq!(joiner.state(), JoinState::Testing);
    assert_eq!(joiner.counters.unexpected, 3);
    assert_eq!(joiner.counters.malformed, 1);
}

#[test]
fn the_master_never_reaches_the_transport_and_relayed_claims_are_dropped() {
    let net = SimNetwork::new(1);
    let mut socket = net.bind(a(OWN)).unwrap();
    let mut master = net.bind(a(MASTER)).unwrap();
    let mut second = net.bind(a(SECOND)).unwrap();
    let host_address = a("198.51.100.77:26900");
    let mut host = net.bind(host_address).unwrap();
    let mut joiner = joiner(net.now());
    joiner.transmit(&mut socket).unwrap();
    net.advance(ms(1));
    let mut buf = [0u8; 2048];
    assert!(master.recv_datagram(&mut buf).unwrap().is_some());
    assert!(second.recv_datagram(&mut buf).unwrap().is_some());
    // A master answer and a host datagram arrive together.
    master
        .send_datagram(
            a(OWN),
            &answer(joiner.probe_nonce, ProbePort::Main, SEEN)
                .encode()
                .unwrap(),
        )
        .unwrap();
    host.send_datagram(a(OWN), b"game datagram").unwrap();
    net.inject(a("[100::1:0:7]:0"), a(OWN), b"forged relayed");
    net.advance(ms(1));
    let now = net.now();
    let mut routed = joiner.over(&mut socket, now);
    let mut got = Vec::new();
    while let Some((len, from)) = routed.recv_datagram(&mut buf).unwrap() {
        got.push((buf[..len].to_vec(), from));
    }
    assert_eq!(got, [(b"game datagram".to_vec(), host_address)]);
    assert_eq!(joiner.counters.received, 1);
    assert_eq!(joiner.counters.relayed_claims, 1);
    // A send to a relayed address goes nowhere until the relay.
    joiner
        .over(&mut socket, now)
        .send_datagram(a("[100::1:0:7]:0"), b"x")
        .unwrap();
    assert_eq!(joiner.counters.relay_sends_dropped, 1);
}
