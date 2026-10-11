//! Stage K's wire (protocol 13, slice K0): the sixteen messages, the lobby's
//! standby marks and reserved slots, at their limits and past them. Every
//! bound is refused by the writer and by the reader. The round trip, fuzz
//! and golden tests cover the samples ([`super::samples::migration_messages`]);
//! the standby records' stream is `journal_tests.rs`'s.

use super::bits;
use super::inputs::{Command, InputFrame};
use super::messages::{Message, StandbyMark, kind};
use super::migration::{limits, *};
use super::{WireError, samples};
use std::net::SocketAddr;
use tore_codec::BitWriter;
use tore_net::master::{Candidate, CandidateKind};
use tore_world::seats::SeatCommand;

fn round_trip(message: &Message) {
    let bytes = message.encode().unwrap();
    assert!(bytes.len() <= super::limits::MESSAGE);
    assert_eq!(&Message::decode(message.kind(), &bytes).unwrap(), message);
}

fn address(n: u8) -> SocketAddr {
    SocketAddr::from(([203, 0, 113, n], 26_900))
}

fn target(player: u8, addresses: usize) -> ReachTarget {
    ReachTarget {
        player,
        addresses: (0..addresses as u8).map(address).collect(),
    }
}

#[test]
fn the_kinds_are_39_to_54_and_go_their_ways() {
    let messages = samples::migration_messages();
    let mut kinds: Vec<u8> = messages.iter().map(Message::kind).collect();
    kinds.dedup();
    assert_eq!(kinds, (39..=54).collect::<Vec<_>>());
    let from_player = [
        kind::CANDIDATE,
        kind::REACH_REPORT,
        kind::STANDBY_STATUS,
        kind::RESUME,
        kind::BACKLOG,
        kind::TAKEN_OVER,
        kind::RELEASE,
        kind::REJOIN,
    ];
    for message in &messages {
        assert_eq!(
            message.from_player(),
            from_player.contains(&message.kind()),
            "{}",
            message.kind()
        );
        round_trip(message);
    }
    // Kind 55 is the first free one.
    assert!(Message::decode(55, &[]).is_err());
}

#[test]
fn every_message_round_trips_at_its_limits() {
    let full_peers = ReachPeers {
        test: u16::MAX,
        players: (0..limits::REACH_PEERS as u8)
            .map(|p| target(p, limits::ADDRESSES))
            .collect(),
    };
    round_trip(&Message::ReachPeers(Box::new(full_peers)));
    round_trip(&Message::ReachTest(Box::new(ReachTest {
        test: 0,
        candidates: (0..3).map(|p| target(p, limits::ADDRESSES)).collect(),
    })));
    round_trip(&Message::Succession(Box::new(Succession {
        standbys: (0..2)
            .map(|player| Successor {
                player,
                warm: player == 0,
                addresses: (0..8)
                    .map(|n| Candidate::new(CandidateKind::Seen, address(n)))
                    .collect(),
            })
            .collect(),
    })));
    round_trip(&Message::UploadTest(UploadTest {
        test: 9,
        rate: u32::MAX,
        length_ms: limits::UPLOAD_MS,
    }));
    // The longest Backlog: 10 seconds of a stick moving every tick and 256
    // commands.
    let ticks: Vec<BacklogTick> = (0..limits::BACKLOG_TICKS)
        .map(|i| BacklogTick {
            frame: InputFrame {
                pitch: (i as i16) * 7,
                trigger: i % 2 == 0,
                ..InputFrame::default()
            },
            view_offset: 255,
            interpolation_delay: 63,
        })
        .collect();
    let backlog = Backlog {
        flight: 1,
        first_tick: u32::MAX - 2_000,
        commands: (0..limits::BACKLOG_COMMANDS as u32)
            .map(|i| BacklogCommand {
                offset: i * 4,
                command: Command::Seat(SeatCommand::ReleaseFlare),
            })
            .collect(),
        ticks,
    };
    let message = Message::Backlog(Box::new(backlog));
    round_trip(&message);
    assert!(message.encode().unwrap().len() < 16_000);
    // Every standby state, check result and processor.
    for state in [
        StandbyState::Building,
        StandbyState::Warm,
        StandbyState::Cold,
        StandbyState::Behind,
    ] {
        for check in [
            CheckResult::None,
            CheckResult::Equal,
            CheckResult::Different,
        ] {
            round_trip(&Message::StandbyStatus(StandbyStatus {
                state,
                check,
                ..StandbyStatus::default()
            }));
        }
    }
    for code in 0..5 {
        let processor = Processor::from_code(code).unwrap();
        assert_eq!(processor.code(), code);
        round_trip(&Message::Candidate(Box::new(CandidateReport {
            processor,
            ..samples::candidate_report()
        })));
    }
    assert_ne!(Processor::current(), Processor::Unknown);
}

#[test]
fn every_bound_is_refused_by_the_writer() {
    let refused = |message: Message| {
        assert!(message.encode().is_err(), "{message:?}");
    };
    let mut report = samples::candidate_report();
    report
        .candidates
        .push(Candidate::new(CandidateKind::Local, address(9)));
    refused(Message::Candidate(Box::new(report)));
    let mut report = samples::candidate_report();
    report.candidates[0].kind = CandidateKind::Seen;
    refused(Message::Candidate(Box::new(report)));
    refused(Message::ReachTest(Box::new(ReachTest {
        test: 0,
        candidates: (0..4).map(|p| target(p, 1)).collect(),
    })));
    refused(Message::ReachTest(Box::new(ReachTest {
        test: 0,
        candidates: vec![target(0, 9)],
    })));
    refused(Message::ReachPeers(Box::new(ReachPeers {
        test: 0,
        players: (0..65).map(|p| target(p, 1)).collect(),
    })));
    refused(Message::ReachReport(Box::new(ReachReport {
        test: 0,
        results: vec![ReachResult {
            player: 1,
            reached: Some(Reached {
                address: 16,
                round_trip_ms: 1,
            }),
        }],
    })));
    refused(Message::UploadTest(UploadTest {
        test: 0,
        rate: 1,
        length_ms: limits::UPLOAD_MS + 1,
    }));
    refused(Message::Succession(Box::new(Succession {
        standbys: (0..3)
            .map(|player| Successor {
                player,
                warm: false,
                addresses: Vec::new(),
            })
            .collect(),
    })));
    refused(Message::Resumed(Box::new(Resumed::Flying(ResumedFlight {
        flight: 0,
        seat: 1,
        plane: 1,
        tick: 1,
        last_command: 0,
        exact: Vec::new(),
        destroyed: Vec::new(),
        surface_digest: 0,
    }))));
    let mut backlog = samples::backlog();
    backlog.commands[0].offset = backlog.ticks.len() as u32;
    refused(Message::Backlog(Box::new(backlog)));
    let mut backlog = samples::backlog();
    backlog.ticks[0].interpolation_delay = 64;
    refused(Message::Backlog(Box::new(backlog)));
    let mut backlog = samples::backlog();
    backlog.ticks = vec![BacklogTick::default(); limits::BACKLOG_TICKS + 1];
    refused(Message::Backlog(Box::new(backlog)));
    refused(Message::StandbyRecord(Vec::new()));
    refused(Message::StandbyRecord(vec![0x0A]));
}

/// A raw body, written field by field with none of the writer's checks.
fn raw(write: impl FnOnce(&mut BitWriter)) -> Vec<u8> {
    let mut w = BitWriter::new();
    write(&mut w);
    bits::finish(w)
}

fn read(kind: u8, body: &[u8]) -> Result<Message, WireError> {
    Message::decode(kind, body)
}

#[test]
fn every_bound_is_refused_by_the_reader() {
    // A Candidate with a platform, a processor or a mapping the protocol
    // does not name, or four candidates.
    let candidate = |platform: u64, processor: u64, count: u64| {
        raw(|w| {
            w.write_bool(true);
            w.write_bits(platform, 3).unwrap();
            w.write_bits(processor, 3).unwrap();
            w.write_bits(count, 4).unwrap();
            for n in 0..count {
                w.write_bits(0, 3).unwrap();
                w.write_bool(false);
                w.write_bits(u64::from(n as u8), 32).unwrap();
                w.write_bits(26_900, 16).unwrap();
            }
            w.write_bits(0, 2).unwrap();
            w.write_varint(0);
            w.write_varint(0);
        })
    };
    assert!(read(kind::CANDIDATE, &candidate(3, 4, 3)).is_ok());
    assert!(read(kind::CANDIDATE, &candidate(4, 1, 0)).is_err());
    assert!(read(kind::CANDIDATE, &candidate(1, 5, 0)).is_err());
    assert!(read(kind::CANDIDATE, &candidate(1, 1, 4)).is_err());
    // A Succession of three; a Reach peers of 65; an Upload test of 2.001 s.
    assert!(read(kind::SUCCESSION, &raw(|w| w.write_bits(3, 2).unwrap())).is_err());
    assert!(
        read(
            kind::REACH_PEERS,
            &raw(|w| {
                w.write_bits(1, 16).unwrap();
                w.write_varint(65);
            })
        )
        .is_err()
    );
    assert!(
        read(
            kind::UPLOAD_TEST,
            &raw(|w| {
                w.write_bits(1, 16).unwrap();
                w.write_varint(1);
                w.write_bits(2_001, 16).unwrap();
            })
        )
        .is_err()
    );
    // A reach target with nine addresses.
    assert!(
        read(
            kind::REACH_TEST,
            &raw(|w| {
                w.write_bits(1, 16).unwrap();
                w.write_bits(1, 2).unwrap();
                w.write_bits(4, 8).unwrap();
                w.write_bits(9, 4).unwrap();
            })
        )
        .is_err()
    );
    // A standby status's check result 3.
    let status = |check: u64| {
        raw(|w| {
            w.write_bits(1, 32).unwrap();
            w.write_bits(1, 2).unwrap();
            w.write_varint(10);
            w.write_bits(1, 32).unwrap();
            w.write_bits(check, 2).unwrap();
            w.write_bool(false);
        })
    };
    assert!(read(kind::STANDBY_STATUS, &status(2)).is_ok());
    assert!(read(kind::STANDBY_STATUS, &status(3)).is_err());
    // A Backlog of 1,201 ticks; a command past its ticks.
    assert!(
        read(
            kind::BACKLOG,
            &raw(|w| {
                w.write_bits(1, 8).unwrap();
                w.write_bits(0, 32).unwrap();
                w.write_bits(1_201, 16).unwrap();
            })
        )
        .is_err()
    );
    // One tick, and a command at its second tick.
    let past = raw(|w| {
        w.write_bits(1, 8).unwrap();
        w.write_bits(0, 32).unwrap();
        w.write_bits(1, 16).unwrap();
        for width in [16, 16, 16, 8] {
            w.write_bits(0, width).unwrap();
        }
        w.write_bool(false);
        w.write_bool(false);
        // No powered-lift block (protocol 18).
        w.write_bool(false);
        w.write_bits(0, 7).unwrap();
        // No sight slew or zoom (protocol 21).
        w.write_bool(false);
        w.write_bits(0, 14).unwrap();
        w.write_varint(1);
        w.write_varint(1);
    });
    assert!(matches!(
        read(kind::BACKLOG, &past),
        Err(WireError::Invalid("backlog command after its ticks"))
    ));
    // An empty standby record, or one of type 10.
    assert!(read(kind::STANDBY_RECORD, &[]).is_err());
    assert!(read(kind::STANDBY_RECORD, &[0x0A]).is_err());
    assert!(read(kind::STANDBY_RECORD, &[0x09, 0, 0, 0, 0]).is_ok());
    // Trailing bytes after any body.
    for message in samples::migration_messages() {
        if matches!(message, Message::StandbyRecord(_)) {
            continue;
        }
        let mut body = message.encode().unwrap();
        body.push(0x80);
        assert!(read(message.kind(), &body).is_err(), "{}", message.kind());
    }
}

#[test]
fn a_lobby_carries_standby_marks_and_reserved_slots_and_refuses_mark_3() {
    let lobby = samples::lobby();
    assert!(
        lobby
            .players
            .iter()
            .any(|p| p.standby == StandbyMark::First)
    );
    assert!(
        lobby
            .players
            .iter()
            .any(|p| p.standby == StandbyMark::Second)
    );
    assert!(lobby.slots.iter().any(|s| s.reserved.is_some()));
    round_trip(&Message::Lobby(Box::new(lobby.clone())));
    for code in 0..3 {
        assert_eq!(StandbyMark::from_code(code).unwrap().code(), code);
    }
    assert_eq!(StandbyMark::from_code(3), None);
    // A lobby of one player whose mark is 3.
    let mut one = lobby;
    one.players.truncate(1);
    one.players[0].standby = StandbyMark::Second;
    one.slots.clear();
    one.settings.clear();
    let mut body = Message::Lobby(Box::new(one)).encode().unwrap();
    // The mark is the second bit pair from the end before the two empty
    // counts: find it by flipping each bit and keeping the one that turns
    // mark 2 into 3.
    let mut refused = false;
    for bit in 0..body.len() * 8 {
        body[bit / 8] ^= 1 << (bit % 8);
        if let Err(WireError::Invalid("standby mark")) = read(kind::LOBBY, &body) {
            refused = true;
        }
        body[bit / 8] ^= 1 << (bit % 8);
    }
    assert!(refused);
}

#[test]
fn a_resumed_player_not_flying_is_one_byte() {
    let body = Message::Resumed(Box::new(Resumed::NotFlying))
        .encode()
        .unwrap();
    assert_eq!(body, [0]);
    let body = Message::Rejoin(tore_net::Token(1)).encode().unwrap();
    assert_eq!(body.len(), 16);
    let body = Message::Token(TokenGrant {
        token: tore_net::Token(1),
        life_seconds: limits::TOKEN_LIFE_SECONDS,
    })
    .encode()
    .unwrap();
    assert_eq!(body.len(), 16 + 3);
}

#[test]
fn the_host_takes_a_filler_section_of_zero_bytes_only() {
    use super::SECTION_FILLER;
    use super::connection::HostConnection;
    assert_eq!(SECTION_FILLER, 6);
    assert!(HostConnection::check(SECTION_FILLER, &[]));
    assert!(HostConnection::check(SECTION_FILLER, &[0; 1_100]));
    assert!(!HostConnection::check(SECTION_FILLER, &[0, 0, 1]));
    assert!(!HostConnection::check(7, &[]));
}
