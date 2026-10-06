//! Stage K's seams on the network simulator (slice K0's acceptance): the
//! host steps its world through the journal, and a twin replaying the ticks
//! it stepped codes to the same checkpoint; every new request a later slice
//! builds is refused in words and nobody is disconnected; the lobby carries
//! setting 21, the standby marks and an away player's reserved slot; the
//! host's transport answers a Reach for its own session; and the client
//! passes the standby records on in order, keeping its connection, while the
//! other new host messages wait for their slices. Synthetic resources.

use super::tests::{Rig, level_script, spec, weave};
use super::*;
use crate::host::{AfterEnd, NOT_AVAILABLE, StartMode};
use crate::journal::{apply_tick, drain};
use crate::settings::number;
use crate::wire::messages::{StandbyMark, kind};
use crate::wire::migration::{StandbyStatus, TakenOver};
use crate::wire::samples;
use tore_net::ReachRole;
use tore_net::packet::{Packet, Reach, ReachAnswer};
use tore_net::sim::LinkConfig;
use tore_world::resources::ResourceReads;

const MS: Duration = Duration::from_millis(1);

/// A game a player hosts: the first player to join is the King.
fn kings_rig() -> Rig {
    Rig::with_config(
        spec(2, 2, 20),
        LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
        13,
        |config| {
            config.house = Some(Rig::player_address(0));
            config.crown = crate::host::CrownRule::FirstPlayer;
            config.start = StartMode::King;
            config.after_end = AfterEnd::Restart;
            config.restart_delay = Duration::ZERO;
            config.empty_timeout = Duration::ZERO;
        },
    )
}

fn manual(rig: &mut Rig, callsign: &str) -> usize {
    let callsign = callsign.to_owned();
    rig.join(
        move |c| {
            c.callsign = callsign;
            c.auto_ready = false;
        },
        Box::new(|now, _, _| weave(now.as_secs_f64())),
    )
}

fn refusals(rig: &Rig, player: usize) -> Vec<(u8, String)> {
    rig.players[player]
        .events
        .iter()
        .filter_map(|e| match e {
            ClientEvent::Refused { request, reason } => Some((*request, reason.clone())),
            _ => None,
        })
        .collect()
}

fn gathered() -> (Rig, usize, usize) {
    let mut rig = kings_rig();
    let king = manual(&mut rig, "Viper");
    let cobra = manual(&mut rig, "Cobra");
    assert!(
        rig.run_until(Duration::from_secs(3), |r| {
            [king, cobra].iter().all(|&p| {
                r.players[p].client.phase() == ClientPhase::Lobby
                    && r.players[p]
                        .client
                        .lobby()
                        .is_some_and(|l| l.players.len() == 2)
            })
        }),
        "the players gather"
    );
    (rig, king, cobra)
}

#[test]
fn every_new_request_is_refused_in_words_until_its_slice_lands() {
    let (mut rig, king, cobra) = gathered();
    let now = rig.net.now();
    let messages = samples::migration_messages();
    let asked: Vec<Message> = messages
        .into_iter()
        .filter(|m| m.from_player() && m.kind() != kind::RELEASE)
        .collect();
    assert_eq!(
        asked.len(),
        9,
        "two Candidate and two Standby status samples"
    );
    for message in asked.iter().cloned() {
        rig.players[cobra].client.request(now, message);
    }
    // Release is the King's: refused to anyone else in the King's words.
    rig.players[cobra].client.request(now, Message::Release(1));
    rig.players[king].client.request(now, Message::Release(1));
    // The King's pin (setting 21) is K6's.
    let cobra_id = rig.players[cobra].client.lobby().unwrap().you;
    rig.players[king].client.change_settings(SettingsChange {
        values: vec![(number::HOST, 1 + u32::from(cobra_id))],
        ..SettingsChange::default()
    });
    rig.run(Duration::from_millis(400));
    let refused = refusals(&rig, cobra);
    for message in &asked {
        assert!(
            refused
                .iter()
                .any(|(k, r)| *k == message.kind() && r == NOT_AVAILABLE),
            "{}: {refused:?}",
            message.kind()
        );
    }
    assert!(
        refused
            .iter()
            .any(|(k, r)| *k == kind::RELEASE && r == "Only the King may do that."),
        "{refused:?}"
    );
    let refused = refusals(&rig, king);
    for request in [kind::RELEASE, kind::SETTINGS] {
        assert!(
            refused
                .iter()
                .any(|(k, r)| *k == request && r == NOT_AVAILABLE),
            "{request}: {refused:?}"
        );
    }
    // Nobody was disconnected, and the host stays calculated.
    assert!(!rig.closed(king) && !rig.closed(cobra));
    let lobby = rig.players[cobra].client.lobby().unwrap();
    assert!(lobby.settings.contains(&(number::HOST, 0)));
    assert!(lobby.players.iter().all(|p| p.standby == StandbyMark::None));
    assert!(lobby.slots.iter().all(|s| s.reserved.is_none()));
}

#[test]
fn a_backlog_and_a_standby_status_do_not_count_as_lobby_requests() {
    let (mut rig, _, cobra) = gathered();
    let now = rig.net.now();
    // Thirty of each in one second: past the 20 a second a player's lobby
    // requests have, yet every one is answered.
    for _ in 0..30 {
        let player = &mut rig.players[cobra].client;
        player.request(now, Message::StandbyStatus(StandbyStatus::default()));
        player.request(now, Message::Backlog(Box::new(samples::backlog())));
    }
    rig.run(Duration::from_millis(400));
    let refused = refusals(&rig, cobra);
    for request in [kind::STANDBY_STATUS, kind::BACKLOG] {
        assert_eq!(
            refused.iter().filter(|(k, _)| *k == request).count(),
            30,
            "{request}"
        );
    }
    // A lobby request's 20 a second still holds for the others.
    for _ in 0..30 {
        rig.players[cobra].client.request(
            now,
            Message::TakenOver(TakenOver {
                new_host: 1,
                tick: 1,
            }),
        );
    }
    rig.run(Duration::from_millis(400));
    let taken = refusals(&rig, cobra)
        .iter()
        .filter(|(k, _)| *k == kind::TAKEN_OVER)
        .count();
    assert!(taken <= 20, "{taken} answered");
}

#[test]
fn the_client_passes_standby_records_on_in_order_and_the_rest_wait_for_their_slices() {
    let (mut rig, _, cobra) = gathered();
    let records: Vec<Vec<u8>> = samples::migration_messages()
        .into_iter()
        .filter_map(|m| match m {
            Message::StandbyRecord(bytes) => Some(bytes),
            _ => None,
        })
        .collect();
    assert_eq!(records.len(), 10);
    let player = &mut rig.players[cobra].client;
    for message in samples::migration_messages() {
        if !message.from_player() {
            player.message(message);
        }
    }
    assert_eq!(player.take_standby_records(), records);
    assert!(player.take_standby_records().is_empty());
    // Nothing else changed: the player is in the lobby, connected.
    rig.run(Duration::from_millis(300));
    assert_eq!(rig.players[cobra].client.phase(), ClientPhase::Lobby);
}

#[test]
fn the_hosts_transport_answers_a_reach_for_its_own_session() {
    let (mut rig, _, cobra) = gathered();
    let session = rig.players[cobra]
        .events
        .iter()
        .find_map(|e| match e {
            ClientEvent::Connected { session_id, .. } => Some(*session_id),
            _ => None,
        })
        .unwrap();
    let mut socket = rig.net.bind("10.0.0.9:41000".parse().unwrap()).unwrap();
    let host: SocketAddr = "10.0.0.1:26900".parse().unwrap();
    for (session_id, nonce) in [(session, 5), (session ^ 1, 6)] {
        let reach = Packet::Reach(Reach {
            session_id,
            nonce,
            from: 9,
        })
        .encode(crate::wire::PROTOCOL_VERSION)
        .unwrap();
        tore_net::Datagrams::send_datagram(&mut socket, host, &reach).unwrap();
    }
    rig.run(Duration::from_millis(200));
    let mut buf = [0u8; 1_500];
    let mut answers = Vec::new();
    while let Ok(Some((len, _))) = tore_net::Datagrams::recv_datagram(&mut socket, &mut buf) {
        answers.push(Packet::decode(&buf[..len], crate::wire::PROTOCOL_VERSION).unwrap());
    }
    assert_eq!(
        answers,
        [Packet::ReachAnswer(ReachAnswer {
            nonce: 5,
            session_id: session,
            role: ReachRole::Hosting,
        })]
    );
}

#[test]
fn an_away_players_slot_reads_reserved_for_it() {
    let mut rig = Rig::with_config(
        spec(2, 2, 20),
        LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
        17,
        |config| config.start = StartMode::Now,
    );
    let viper = rig.join(|_| {}, level_script());
    assert!(rig.run_until(Duration::from_secs(5), |r| r.seated(viper)));
    let plane = rig.players[viper].client.seat().unwrap().1;
    let now = rig.net.now();
    rig.players[viper].client.request(now, Message::Away);
    assert!(
        rig.run_until(Duration::from_secs(3), |r| {
            r.players[viper].client.lobby().is_some_and(|l| {
                l.slots
                    .iter()
                    .any(|s| s.plane == plane.0 && s.reserved.as_deref() == Some("Viper"))
            })
        }),
        "the away player's slot is reserved for it"
    );
}

#[test]
fn the_host_steps_through_the_journal_and_a_twin_codes_to_its_checkpoint() {
    let mut rig = Rig::with_config(
        spec(3, 3, 5),
        LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
        19,
        |config| config.start = StartMode::Now,
    );
    rig.host.keep_journal();
    let spec_text = rig.host.flight_spec_text().to_owned();
    for _ in 0..2 {
        rig.join(|_| {}, level_script());
    }
    let mut checkpoints = BTreeMap::new();
    let end = rig.net.now() + Duration::from_secs(8);
    while rig.net.now() < end {
        rig.step();
        let world = rig.host.world();
        if world.tick().is_multiple_of(30) && !checkpoints.contains_key(&world.tick()) {
            checkpoints.insert(world.tick(), world.checkpoint().unwrap());
        }
    }
    assert!(rig.seated(0) && rig.seated(1));
    let journal = rig.host.take_journal();
    assert!(journal.len() > 900, "{} ticks", journal.len());
    assert!(journal.iter().any(|t| !t.mission.is_empty()), "the takes");
    assert!(journal.iter().any(|t| t.inputs.len() == 2));
    // The twin: the flight's world built fresh, with scoring on as the host
    // turns it on when the mission starts flying (slice K1 records it as a
    // change of the first tick).
    let spec = MissionSpec::from_text(&spec_text).unwrap();
    let mut twin = World::new(&spec, &ResourceReads::new(&rig.resources), Seating::Open).unwrap();
    twin.set_scoring(true);
    let mut out = tore_world::world::TickOutput::default();
    let mut compared = 0;
    for tick in &journal {
        apply_tick(&mut twin, tick, &mut out).unwrap();
        let _ = drain(&mut twin);
        if let Some(expected) = checkpoints.get(&twin.tick()) {
            assert!(
                twin.checkpoint().unwrap() == *expected,
                "the twin differs from the host at tick {}",
                twin.tick()
            );
            compared += 1;
        }
    }
    assert!(compared >= 30, "{compared} checkpoints compared");
    assert_eq!(
        twin.checkpoint().unwrap(),
        rig.host.world().checkpoint().unwrap()
    );
}
