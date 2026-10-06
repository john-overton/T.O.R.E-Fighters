//! The client's side of host selection (slice K6): the Candidate report's
//! contents, the CPU measure, and an Upload test's paced burst of Filler.
//! The host's side, the reach tests end to end and the warnings are in
//! `host/succession_tests.rs`. Synthetic resources.

use super::super::tests::{Rig, spec};
use super::*;
use tore_net::packet::{self, PacketKind};
use tore_net::sim::LinkConfig;
use tore_world::test_support::resources::resources;

const MS: Duration = Duration::from_millis(1);

fn joined() -> (Rig, usize) {
    let mut rig = Rig::new(
        spec(2, 2, 20),
        LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
        21,
    );
    let player = rig.join(
        |c| c.auto_ready = false,
        Box::new(|_, _, _| crate::client::Controls::default()),
    );
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[player].client.phase() == ClientPhase::Lobby
    }));
    (rig, player)
}

#[test]
fn the_report_carries_the_switch_the_class_and_the_candidates_without_a_seen_one() {
    let (mut rig, player) = joined();
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        r.players[player].client.candidacy.sent.is_some()
    }));
    let client = &mut rig.players[player].client;
    let report = client.candidate_report();
    assert!(report.may_host, "on by default");
    assert_eq!(report.processor, Processor::current());
    assert_eq!(report.platform, crate::wire::Platform::current());
    assert_eq!((report.cpu_micros, report.cpu_mission), (0, 0));
    assert_eq!(
        client.candidacy.sent.as_ref(),
        Some(&report),
        "sent in the lobby"
    );
    let address = |port| format!("192.0.2.1:{port}").parse().unwrap();
    client.set_candidate(CandidateSettings {
        may_host: false,
        candidates: vec![
            Candidate::new(CandidateKind::Seen, address(1)),
            Candidate::new(CandidateKind::Local, address(2)),
            Candidate::new(CandidateKind::Mapped, address(3)),
            Candidate::new(CandidateKind::GlobalIpv6, address(4)),
            Candidate::new(CandidateKind::Local, address(5)),
        ],
        mapping: MappingType::SamePort,
        measure_cpu: false,
    });
    let report = client.candidate_report();
    assert!(!report.may_host);
    assert_eq!(report.mapping, MappingType::SamePort);
    assert_eq!(
        report
            .candidates
            .iter()
            .map(|c| c.address.port())
            .collect::<Vec<_>>(),
        vec![2, 3, 4],
        "no Seen candidate, at most three"
    );
    rig.run(Duration::from_millis(100));
    assert_eq!(
        rig.players[player].client.candidacy.sent.as_ref(),
        Some(&report),
        "a change goes out"
    );
}

#[test]
fn the_cpu_measure_steps_the_lobbys_mission_on_a_thread_of_its_own() {
    let micros = measure_cpu(&spec(2, 2, 20), &resources(), 24).expect("it builds and steps");
    assert!(micros >= 1);
    let (mut rig, player) = joined();
    rig.players[player].client.set_candidate(CandidateSettings {
        measure_cpu: true,
        ..CandidateSettings::default()
    });
    assert!(
        rig.run_until(Duration::from_secs(120), |r| {
            r.players[player].client.candidate_report().cpu_micros > 0
        }),
        "the measure arrives"
    );
    let report = rig.players[player].client.candidate_report();
    assert_eq!(report.cpu_mission, rig.host.mission_number());
}

#[test]
fn an_upload_test_sends_its_rate_for_its_length_in_filler_packets_under_1200_bytes() {
    let (mut rig, player) = joined();
    rig.net.start_trace();
    let started = rig.net.now();
    rig.players[player]
        .client
        .candidate_message(Message::UploadTest(UploadTest {
            test: 4,
            rate: 50_000,
            length_ms: 1_000,
        }));
    rig.run(Duration::from_millis(1_500));
    let from = Rig::player_address(player);
    let mut filler = 0;
    let mut last = Duration::ZERO;
    for entry in rig.net.take_trace() {
        if entry.from != from {
            continue;
        }
        assert!(entry.datagram.len() < 1_200);
        let Ok((PacketKind::Payload, body)) =
            packet::open(&entry.datagram, crate::wire::PROTOCOL_VERSION)
        else {
            continue;
        };
        let Ok(sections) =
            packet::decode_payload_header(body).and_then(|(_, rest)| packet::decode_sections(rest))
        else {
            continue;
        };
        for section in sections {
            if section.kind == crate::wire::SECTION_FILLER {
                assert!(section.body.iter().all(|&b| b == 0));
                filler += section.body.len();
                last = entry.at;
            }
        }
    }
    assert_eq!(filler, 50_000, "the rate for a second");
    let took = last - started;
    assert!(
        took >= Duration::from_millis(980) && took <= Duration::from_millis(1_010),
        "paced over the second: {took:?}"
    );
    assert!(rig.players[player].client.candidacy.burst.is_none());
}
