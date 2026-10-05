//! Stage F phase 2's seams on the network simulator (slice F2-0's
//! acceptance): the lobby state carries the King's settings; a request of
//! the King's from anyone else is refused as before; every new request the
//! later slices build is refused politely, in words, and nobody is
//! disconnected; a request for an earlier mission is refused; and the
//! client passes the host's new messages on as events. Synthetic resources.

use super::tests::{Rig, spec, weave};
use super::*;
use crate::host::{AfterEnd, NOT_AVAILABLE, StartMode};
use crate::settings::{self, Mode, Store};
use crate::wire::messages::{Lock, Observe, PasswordChange, Subject, kind};
use crate::wire::samples;
use tore_net::sim::LinkConfig;

const MS: Duration = Duration::from_millis(1);

/// A game a player hosts: the first player to join is the King.
fn kings_rig() -> Rig {
    Rig::with_config(
        spec(2, 2, 20),
        LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
        11,
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

/// A player who drives the lobby itself, as a lobby screen would.
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

fn refused(rig: &Rig, player: usize, request: u8, reason: &str) -> bool {
    refusals(rig, player)
        .iter()
        .any(|(k, r)| *k == request && r == reason)
}

/// The King and Cobra in the lobby.
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
fn the_lobby_state_carries_every_setting_by_number() {
    let (rig, _, cobra) = gathered();
    let lobby = rig.players[cobra].client.lobby().unwrap();
    let mut expected = Store::defaults(Mode::Coop);
    expected
        .apply(&[(
            settings::number::MAX_PLAYERS,
            u32::try_from(rig.host.config().max_players).unwrap(),
        )])
        .unwrap();
    assert_eq!(lobby.settings, expected.lobby_list());
    assert_eq!(lobby.settings.len(), settings::REGISTRY.len());
    // No locks or away players before their slices; nobody watches.
    assert!(lobby.slots.iter().all(|s| s.lock == Lock::Open));
    assert!(lobby.players.iter().all(|p| !p.observing && !p.away));
}

#[test]
fn every_new_request_is_refused_in_words_until_its_slice_lands() {
    let (mut rig, king, cobra) = gathered();
    let change = || SettingsChange {
        values: vec![(settings::number::FRIENDLY_FIRE, 0)],
        name: Some("Cobra's game".into()),
        password: Some(PasswordChange::Set("pw".into())),
    };

    // The King's requests from anyone else: as EF4's.
    let king_id = rig.players[king].client.lobby().unwrap().you;
    let player = &mut rig.players[cobra].client;
    player.pass_crown(king_id);
    player.change_settings(change());
    player.lock_slot(1, Lock::Closed);
    rig.run(Duration::from_millis(300));
    for request in [kind::PASS_CROWN, kind::SETTINGS, kind::SLOT_LOCK] {
        assert!(
            refused(&rig, cobra, request, "Only the King may do that."),
            "{request}: {:?}",
            refusals(&rig, cobra)
        );
    }

    // The King's own are built (F2-1): taken, the crown passed last.
    let cobra_id = rig.players[cobra].client.lobby().unwrap().you;
    let player = &mut rig.players[king].client;
    player.change_settings(change());
    player.lock_slot(1, Lock::Reserved("Cobra".into()));
    rig.run(Duration::from_millis(300));
    rig.players[king].client.pass_crown(cobra_id);
    rig.run(Duration::from_millis(300));
    assert!(
        refusals(&rig, king).is_empty(),
        "{:?}",
        refusals(&rig, king)
    );
    assert_eq!(
        rig.players[cobra].client.lobby().unwrap().king,
        Some(cobra_id)
    );

    // Everyone's: not built yet.
    let player = &mut rig.players[cobra].client;
    player.revive();
    player.observe(Observe::Watch(Subject::Aircraft(1)));
    player.away();
    player.back();
    rig.run(Duration::from_millis(300));
    for request in [kind::REVIVE, kind::AWAY, kind::BACK] {
        assert!(
            refused(&rig, cobra, request, NOT_AVAILABLE),
            "{request}: {:?}",
            refusals(&rig, cobra)
        );
    }
    // Watching is built (F2-O1): refused in words before the mission flies.
    assert!(refused(
        &rig,
        cobra,
        kind::OBSERVE,
        "The mission is not flying; watch once it flies."
    ));

    // A request meant for an earlier mission is refused as one.
    let now = rig.players[cobra].client.now;
    rig.players[cobra]
        .client
        .request(now, Message::Revive { mission: 999 });
    let now = rig.players[king].client.now;
    rig.players[king].client.request(
        now,
        Message::SlotLock(Box::new(SlotLock {
            mission: 999,
            plane: 1,
            lock: Lock::Open,
        })),
    );
    rig.run(Duration::from_millis(300));
    let changed = "The mission has changed; choose again.";
    assert!(refused(&rig, cobra, kind::REVIVE, changed));
    assert!(refused(&rig, king, kind::SLOT_LOCK, changed));

    // Nothing broke the protocol, and the King's settings stand.
    for p in [king, cobra] {
        assert_eq!(rig.players[p].client.phase(), ClientPhase::Lobby);
    }
    assert_eq!(rig.host.lobby_state(0).name, "Cobra's game");
}

#[test]
fn the_client_passes_the_hosts_new_messages_on_as_events() {
    let (mut rig, _, cobra) = gathered();
    let client = &mut rig.players[cobra].client;
    for message in samples::phase_two_messages()
        .into_iter()
        .filter(|m| (kind::REVIVAL..=kind::OBSERVING).contains(&m.kind()) && !m.from_player())
    {
        client.message(message);
    }
    let mut kinds = Vec::new();
    while let Some(event) = client.poll_event() {
        kinds.push(match event {
            ClientEvent::Revival(_) => kind::REVIVAL,
            ClientEvent::Spawned(_) => kind::SPAWNED,
            ClientEvent::Scores(_) => kind::SCORES,
            ClientEvent::Results(_) => kind::RESULTS,
            ClientEvent::Observing(_) => kind::OBSERVING,
            _ => continue,
        });
    }
    kinds.dedup();
    assert_eq!(
        kinds,
        [
            kind::REVIVAL,
            kind::SPAWNED,
            kind::SCORES,
            kind::RESULTS,
            kind::OBSERVING
        ]
    );
    rig.run(Duration::from_millis(200));
    assert_ne!(rig.players[cobra].client.phase(), ClientPhase::Closed);
}
