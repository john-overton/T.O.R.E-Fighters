//! Slice F2-A's client tests (docs/ARCHITECTURE.md, "The AI flies an idle
//! player's aircraft"): a game that says it is away finds the AI flying its
//! plane, watches it, and takes it back with Back; a game whose loop stalls
//! past the setting (its keepalives keeping it connected) comes back to the
//! same; and a plane the AI loses is no longer the game's. On the network
//! simulator, synthetic resources.

use super::tests::{Rig, level_script, spec};
use super::*;
use crate::host::{HostLog, LobbyEvent, StartMode};
use tore_net::Datagrams;
use tore_net::sim::LinkConfig;

const MS: Duration = Duration::from_millis(1);

/// Viper flying plane 0 in co-op, the enemy far off.
fn flying() -> (Rig, usize) {
    let mut rig = Rig::with_config(
        spec(2, 1, 50),
        LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
        7,
        |config| config.start = StartMode::Now,
    );
    let viper = rig.join(|c| c.plane = Some(0), level_script());
    assert!(rig.run_until(Duration::from_secs(5), |r| r.seated(viper)));
    rig.run(Duration::from_millis(500));
    (rig, viper)
}

fn plane_of(rig: &Rig, player: usize) -> Option<u32> {
    rig.players[player].client.seat().map(|(_, plane)| plane.0)
}

#[test]
fn a_game_away_finds_the_ai_flying_its_plane_and_takes_it_back() {
    let (mut rig, viper) = flying();
    // Back before Away is nothing to send.
    rig.players[viper].client.back();
    assert!(!rig.players[viper].client.away_asked());
    rig.players[viper].client.away();
    assert!(rig.players[viper].client.away_asked());
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.players[viper].client.ai_flies() == Some(0)
    }));
    let client = &rig.players[viper].client;
    assert_eq!(client.phase(), ClientPhase::Lobby);
    assert!(client.seat().is_none() && !client.away_asked());
    assert!(client.watching().is_some());
    // It watches its own plane; the lobby marks it away, and that holds.
    rig.run(Duration::from_secs(1));
    let now = rig.net.now();
    let client = &mut rig.players[viper].client;
    let frame = client.observer_frame(now).expect("an observer frame");
    assert!(frame.picture.targets.iter().any(|t| t.id == 0));
    assert!(client.lobby().unwrap().me().unwrap().away);
    assert_eq!(client.ai_flies(), Some(0));

    client.back();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    assert_eq!(plane_of(&rig, viper), Some(0));
    let client = &rig.players[viper].client;
    assert_eq!(client.ai_flies(), None);
    assert!(client.watching().is_none());
    // It flies on, and the host is satisfied with its inputs again.
    rig.run(Duration::from_secs(2));
    assert!(rig.seated(viper));
    assert!(
        !rig.players[viper]
            .client
            .lobby()
            .unwrap()
            .me()
            .unwrap()
            .away
    );
}

#[test]
fn a_game_stalled_past_the_setting_comes_back_to_the_ai_flying() {
    let (mut rig, viper) = flying();
    let keepalive = rig.players[viper]
        .client
        .keepalive_datagram()
        .expect("joined");
    rig.players[viper].stalled = true;
    for ms in 1..=11_000u32 {
        rig.step();
        if ms % 1000 == 0 {
            rig.players[viper]
                .socket
                .send_datagram(super::tests::host_address(), &keepalive)
                .unwrap();
        }
    }
    assert!(rig.logs.iter().any(|log| matches!(
        log,
        HostLog::Lobby {
            event: LobbyEvent::Away {
                plane: 0,
                stalled: true
            },
            ..
        }
    )));
    rig.players[viper].stalled = false;
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.players[viper].client.ai_flies() == Some(0)
    }));
    assert!(!rig.closed(viper));
    rig.players[viper].client.back();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    assert_eq!(plane_of(&rig, viper), Some(0));
}

#[test]
fn a_plane_the_ai_loses_is_no_longer_the_games() {
    let (mut rig, viper) = flying();
    rig.players[viper].client.away();
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.players[viper].client.ai_flies() == Some(0)
    }));
    rig.run(Duration::from_millis(500));
    rig.host
        .world_for_test()
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == 0)
        .unwrap()
        .hp = 0;
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.players[viper].client.ai_flies().is_none()
    }));
    assert!(rig.players[viper].events.iter().any(|e| matches!(
        e,
        ClientEvent::Notice(text) if text == "The AI lost your aircraft while you were away."
    )));
    assert_eq!(rig.players[viper].client.phase(), ClientPhase::Lobby);
}

#[test]
fn an_away_game_leaves_the_game_cleanly() {
    let (mut rig, viper) = flying();
    rig.players[viper].client.away();
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.players[viper].client.ai_flies() == Some(0)
    }));
    rig.run(Duration::from_millis(500));
    let now = rig.net.now();
    rig.players[viper].client.leave_game(now);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.closed(viper)));
    let closed: Vec<&ClientEvent> = rig.players[viper]
        .events
        .iter()
        .filter(|e| matches!(e, ClientEvent::Closed(_)))
        .collect();
    assert!(
        matches!(
            closed[..],
            [ClientEvent::Closed(CloseReason::Disconnected {
                reason: DisconnectReason::Left,
                by_peer: false
            })]
        ),
        "{closed:?}"
    );
    rig.run(Duration::from_millis(500));
    assert!(rig.host.lobby_state(0).players.is_empty());
}
