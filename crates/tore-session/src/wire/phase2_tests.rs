//! Stage F phase 2's wire (protocol 8, slice F2-0): the bounds and invalid
//! codes of every new message and field. The round trip, fuzz and golden
//! tests cover the samples ([`super::samples::phase_two_messages`]).

use super::bits;
use super::messages::{
    EndReason, Message, PasswordChange, Results, Revival, Scores, SettingsChange, kind,
};
use super::{WireError, samples};
use crate::settings::Respawn;
use tore_codec::BitWriter;

fn body(write: impl FnOnce(&mut BitWriter)) -> Vec<u8> {
    let mut w = BitWriter::new();
    write(&mut w);
    bits::finish(w)
}

fn invalid(kind: u8, bytes: &[u8]) -> bool {
    Message::decode(kind, bytes).is_err()
}

#[test]
fn every_new_kind_names_its_message_and_its_direction() {
    let players = [
        kind::PASS_CROWN,
        kind::SETTINGS,
        kind::SLOT_LOCK,
        kind::REVIVE,
        kind::OBSERVE,
        kind::AWAY,
        kind::BACK,
    ];
    let hosts = [
        kind::REVIVAL,
        kind::SPAWNED,
        kind::SCORES,
        kind::RESULTS,
        kind::OBSERVING,
    ];
    for message in samples::phase_two_messages() {
        let k = message.kind();
        if k == kind::MISSION_ENDED {
            continue;
        }
        assert!(
            players.contains(&k) == message.from_player()
                && (players.contains(&k) || hosts.contains(&k)),
            "kind {k}"
        );
    }
    // Kinds 23, 24 and 27 to 36, with chat's between.
    assert_eq!((kind::PASS_CROWN, kind::SETTINGS), (23, 24));
    assert_eq!((kind::SLOT_LOCK, kind::BACK), (27, 36));
    assert!(invalid(37, &[]));
}

#[test]
fn writers_refuse_what_the_protocol_does_not_allow() {
    let too_many = Message::Settings(Box::new(SettingsChange {
        values: vec![(1, 0); 65],
        ..SettingsChange::default()
    }));
    assert!(matches!(
        too_many.encode(),
        Err(WireError::TooMany { limit: 64, .. })
    ));
    let empty_password = Message::Settings(Box::new(SettingsChange {
        password: Some(PasswordChange::Set(String::new())),
        ..SettingsChange::default()
    }));
    assert_eq!(empty_password.encode(), Err(WireError::Invalid("password")));
    let lives = Message::Revival(Box::new(Revival {
        rule: Respawn::Revive,
        lives: Some(11),
        wait_seconds: 0,
        why: None,
    }));
    assert_eq!(lives.encode(), Err(WireError::Invalid("lives")));
    let mut scores = samples::scores();
    scores.kill_limit = 16;
    assert_eq!(
        Message::Scores(Box::new(scores)).encode(),
        Err(WireError::Invalid("kill limit"))
    );
    let mut scores = samples::scores();
    scores.players = vec![scores.players[0].clone(); 65];
    assert!(matches!(
        Message::Scores(Box::new(scores)).encode(),
        Err(WireError::TooMany { limit: 64, .. })
    ));
    let Message::Results(results) = samples::phase_two_messages()
        .into_iter()
        .find(|m| m.kind() == kind::RESULTS)
        .unwrap()
    else {
        unreachable!()
    };
    let mut damaged: Results = (*results).clone();
    damaged.rows[0].damage = 1_001;
    assert_eq!(
        Message::Results(Box::new(damaged)).encode(),
        Err(WireError::Invalid("result damage"))
    );
    let mut long: Results = (*results).clone();
    long.rows = vec![results.rows[0].clone(); 1_025];
    assert!(matches!(
        Message::Results(Box::new(long)).encode(),
        Err(WireError::TooMany { limit: 1_024, .. })
    ));
    // The most a Results message may carry fits a message.
    let mut most: Results = (*results).clone();
    most.rows = vec![results.rows[0].clone(); 1_024];
    most.scores = Some(Scores {
        players: vec![samples::scores().players[0].clone(); 64],
        ..samples::scores()
    });
    let message = Message::Results(Box::new(most));
    let bytes = message.encode().unwrap();
    assert_eq!(Message::decode(kind::RESULTS, &bytes).unwrap(), message);
}

#[test]
fn readers_refuse_invalid_codes() {
    // A slot lock's code 3.
    assert!(invalid(
        kind::SLOT_LOCK,
        &body(|w| {
            w.write_varint(7);
            w.write_varint(1);
            let _ = w.write_bits(3, 2);
        })
    ));
    // An observer's subject code 3.
    assert!(invalid(
        kind::OBSERVE,
        &body(|w| {
            w.write_bool(true);
            let _ = w.write_bits(3, 2);
        })
    ));
    // A respawn rule's code 3, and 11 lives.
    let revival = |rule: u64, lives: u64| {
        body(|w| {
            let _ = w.write_bits(rule, 2);
            w.write_bool(false);
            let _ = w.write_bits(lives, 4);
            w.write_varint(0);
            w.write_bool(false);
        })
    };
    assert!(!invalid(kind::REVIVAL, &revival(2, 10)));
    assert!(invalid(kind::REVIVAL, &revival(3, 1)));
    assert!(invalid(kind::REVIVAL, &revival(2, 11)));
    // An empty password, and more than 64 settings.
    assert!(invalid(
        kind::SETTINGS,
        &body(|w| {
            w.write_varint(0);
            w.write_bool(false);
            w.write_bool(true);
            w.write_bool(true);
            let _ = w.write_str("");
        })
    ));
    assert!(matches!(
        Message::decode(kind::SETTINGS, &body(|w| w.write_varint(65))),
        Err(WireError::TooMany { limit: 64, .. })
    ));
    // A Scores tally's code 3.
    assert!(invalid(
        kind::SCORES,
        &body(|w| {
            let _ = w.write_bits(3, 2);
        })
    ));
    // End reason 6, and more rows than the limit.
    assert!(invalid(
        kind::MISSION_ENDED,
        &body(|w| {
            let _ = w.write_bits(6, 3);
            w.write_bool(false);
        })
    ));
    assert!(matches!(
        Message::decode(
            kind::RESULTS,
            &body(|w| {
                let _ = w.write_bits(5, 3);
                w.write_varint(1_025);
            })
        ),
        Err(WireError::TooMany { limit: 1_024, .. })
    ));
    // A row count the bytes cannot hold.
    assert!(invalid(
        kind::RESULTS,
        &body(|w| {
            let _ = w.write_bits(5, 3);
            w.write_varint(1_000);
        })
    ));
    // Away and Back are empty.
    assert!(invalid(kind::AWAY, &[1]));
    assert!(!invalid(kind::BACK, &[]));
}

#[test]
fn the_kill_limit_ends_a_mission_as_reason_five() {
    let bytes = Message::MissionEnded(super::messages::MissionEnded {
        reason: EndReason::KillLimit,
        next_in_seconds: None,
    })
    .encode()
    .unwrap();
    // 3 bits of 5 (least significant bit first), then no next mission.
    assert_eq!(bytes, [0b0000_0101]);
}
