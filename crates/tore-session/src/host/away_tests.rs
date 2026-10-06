//! Slice F2-A's host tests (docs/ARCHITECTURE.md, "The AI flies an idle
//! player's aircraft"), on the network simulator with the scripted test
//! clients: Away hands the plane to the AI and keeps it for its player;
//! another player's take is refused; Back takes it back with its stores and
//! damage; a game that sends nothing for the setting's seconds is away by
//! the host's count; `never` and a longer setting leave the plane alone; and
//! the reservation ends when the AI loses the plane, when the player stops
//! watching it, and when the player leaves the game.

use super::*;
use crate::host::away::{LOST_WHILE_AWAY, NO_IDLE_AI, NOT_AWAY, WAITS};
use crate::settings::number;
use crate::wire::messages::{Observe, Observing, Subject};

/// Viper flying plane 0 and Cobra in the lobby, in co-op (no revival), the
/// enemy far off so nobody fires.
fn pair_with(config: HostConfig) -> (Rig, usize, usize) {
    let mut rig = Rig::new(spec(2, 1, 50), config, LinkConfig::one_way(5 * MS));
    let viper = rig.join(|c| c.callsign = "Viper".into());
    rig.clients[viper].ready = Some(Some(0));
    let cobra = rig.join(|c| c.callsign = "Cobra".into());
    rig.clients[cobra].ready = None;
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.seated(viper) && r.clients[cobra].lobby.is_some()
    }));
    rig.run(Duration::from_millis(300));
    (rig, viper, cobra)
}

fn pair() -> (Rig, usize, usize) {
    pair_with(config())
}

/// Whether the lobby state `client` last heard marks it away.
fn away_mark(rig: &Rig, client: usize) -> bool {
    rig.clients[client]
        .lobby
        .as_ref()
        .and_then(LobbyState::me)
        .is_some_and(|me| me.away)
}

fn pilot(rig: &Rig, plane: u32) -> Pilot {
    rig.host.world().roster.plane(PlaneId(plane)).unwrap().pilot
}

fn peer<'a>(rig: &'a Rig, callsign: &str) -> &'a Peer {
    rig.host
        .peers
        .values()
        .find(|p| p.callsign == callsign)
        .unwrap()
}

fn logged(rig: &Rig, callsign: &str, wanted: &LobbyEvent) -> bool {
    rig.logs.iter().any(|log| {
        matches!(log, HostLog::Lobby { callsign: c, event, .. } if c == callsign && event == wanted)
    })
}

fn refused(rig: &Rig, client: usize, request: u8) -> Vec<String> {
    rig.clients[client]
        .refused
        .iter()
        .filter(|(kind, _)| *kind == request)
        .map(|(_, reason)| reason.clone())
        .collect()
}

/// Sends Away and runs until the lobby state marks the player away.
fn go_away(rig: &mut Rig, client: usize) {
    rig.clients[client].send(&Message::Away);
    assert!(rig.run_until(Duration::from_secs(2), |r| away_mark(r, client)));
}

/// Sends Back and runs until the player is seated in a later flight than
/// `before`'s.
fn come_back(rig: &mut Rig, client: usize, before: u8) -> Seated {
    rig.clients[client].send(&Message::Back);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.clients[client]
            .seated
            .as_ref()
            .is_some_and(|s| s.flight != before)
    }));
    rig.clients[client].seated.clone().unwrap()
}

#[test]
fn away_hands_the_plane_to_the_ai_and_keeps_it_for_its_player() {
    let (mut rig, viper, cobra) = pair();
    let first = rig.clients[viper].seated.clone().unwrap();
    assert_eq!(first.plane, 0);
    go_away(&mut rig, viper);

    // The AI flies plane 0; Viper is in the lobby, watching it.
    assert_eq!(pilot(&rig, 0), Pilot::Ai);
    let viper_peer = peer(&rig, "Viper");
    assert_eq!(viper_peer.stage, Stage::Lobby);
    assert_eq!(
        viper_peer.watch.as_ref().map(observe::Watch::subject),
        Some(Subject::Aircraft(0))
    );
    assert!(matches!(
        rig.clients[viper].observing.first(),
        Some(Observing::Started(started)) if started.flight != first.flight
    ));
    let me = rig.clients[viper].lobby.as_ref().unwrap().me().unwrap();
    assert!(me.away && me.observing && !me.flying, "{me:?}");
    assert!(logged(
        &rig,
        "Viper",
        &LobbyEvent::Away {
            plane: 0,
            stalled: false
        }
    ));
    // The roster names no human for it.
    rig.run(Duration::from_millis(100));
    let roster = rig.clients[cobra].roster.as_ref().unwrap();
    assert_eq!(roster.planes[0].pilot, RosterPilot::Ai);

    // Kept: Cobra's Join for it is refused in words, and so is Viper's own
    // Join (Back takes it back).
    rig.clients[cobra].take(Some(0));
    rig.clients[viper].take(Some(0));
    rig.run(Duration::from_millis(300));
    assert_eq!(
        rig.clients[cobra].seat_refused,
        ["Plane 0 is kept for Viper, who is away."]
    );
    assert_eq!(rig.clients[viper].seat_refused, [WAITS]);
    assert_eq!(pilot(&rig, 0), Pilot::Ai);

    // Back: plane 0 again in a new flight, once the observer flight ended.
    let seated = come_back(&mut rig, viper, first.flight);
    assert_eq!(seated.plane, 0);
    assert!(matches!(
        rig.clients[viper].observing.last(),
        Some(Observing::Ended)
    ));
    assert_eq!(pilot(&rig, 0), Pilot::Human(SeatId(seated.seat)));
    assert!(logged(&rig, "Viper", &LobbyEvent::Back { plane: 0 }));
    assert!(rig.run_until(Duration::from_secs(2), |r| !away_mark(r, viper)));
    assert!(peer(&rig, "Viper").watch.is_none());
    // Back when not away is refused.
    rig.clients[viper].send(&Message::Back);
    rig.run(Duration::from_millis(200));
    assert_eq!(refused(&rig, viper, kind::BACK), [NOT_AWAY]);
    assert!(
        rig.clients[viper].errors.is_empty(),
        "{:?}",
        rig.clients[viper].errors
    );
    assert!(rig.faults().is_empty());
}

#[test]
fn back_retakes_the_plane_with_its_stores_and_damage() {
    let (mut rig, viper, _) = pair();
    let before = rig.clients[viper].seated.clone().unwrap();
    // A round gone from a station before Viper goes away.
    let own = rig.host.world.combat.state.ownship_mut(0).unwrap();
    let full = own.hp;
    let station = own.ammo.iter().position(|&n| n > 1).unwrap();
    own.ammo[station] -= 1;
    let left = own.ammo[station];
    go_away(&mut rig, viper);
    // A hit takes half the plane's hit points while the AI flies it.
    let row = rig
        .host
        .world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == 0)
        .unwrap();
    row.hp = row.initial_hp / 2;
    rig.run(Duration::from_millis(300));
    let seated = come_back(&mut rig, viper, before.flight);
    assert_eq!(seated.loadout.stations[station].quantity, left);
    let own = rig.host.world.combat.state.ownship(0).unwrap();
    assert_eq!(own.ammo[station], left);
    let fraction = f64::from(own.hp) / f64::from(full);
    assert!((fraction - 0.5).abs() < 0.02, "{fraction}");
    assert!(rig.faults().is_empty());
}

#[test]
fn a_game_that_sends_nothing_for_the_settings_seconds_is_away() {
    let (mut rig, viper, _) = pair();
    let before = rig.clients[viper].seated.clone().unwrap();
    // Its game sends no input (a stalled loop): the stall rule flies the
    // plane neutral, and the setting's seconds later the AI flies it. The
    // setting is 10 seconds here (the lists start at a minute), so the test
    // runs 10 simulated seconds, not 5 minutes.
    rig.host.settings.set_for_test(number::IDLE_AI, 10);
    rig.clients[viper].flying = false;
    rig.run(Duration::from_secs(9));
    assert!(!away_mark(&rig, viper));
    assert!(matches!(pilot(&rig, 0), Pilot::Human(_)));
    assert!(rig.run_until(Duration::from_secs(2), |r| away_mark(r, viper)));
    assert_eq!(pilot(&rig, 0), Pilot::Ai);
    assert!(logged(
        &rig,
        "Viper",
        &LobbyEvent::Away {
            plane: 0,
            stalled: true
        }
    ));
    // The game comes back and its player touches the controls.
    rig.clients[viper].flying = true;
    let seated = come_back(&mut rig, viper, before.flight);
    assert_eq!(seated.plane, 0);
    // A fresh flight starts its own count: seated again, it stays.
    rig.run(Duration::from_secs(2));
    assert!(!away_mark(&rig, viper));
    assert!(rig.faults().is_empty());
}

#[test]
fn never_and_a_longer_setting_leave_the_plane_with_its_player() {
    let (mut rig, viper, _) = pair();
    rig.host.settings.apply(&[(number::IDLE_AI, 120)]).unwrap();
    rig.clients[viper].flying = false;
    rig.run(Duration::from_secs(12));
    assert!(!away_mark(&rig, viper));
    assert!(matches!(pilot(&rig, 0), Pilot::Human(_)));
    rig.host.settings.apply(&[(number::IDLE_AI, 0)]).unwrap();
    rig.clients[viper].send(&Message::Away);
    rig.run(Duration::from_secs(20));
    assert_eq!(refused(&rig, viper, kind::AWAY), [NO_IDLE_AI]);
    assert!(!away_mark(&rig, viper));
    assert!(matches!(pilot(&rig, 0), Pilot::Human(_)));
    assert_eq!(peer(&rig, "Viper").stage, Stage::Seated);
}

#[test]
fn a_plane_the_ai_loses_is_lost_to_its_player() {
    let (mut rig, viper, cobra) = pair();
    go_away(&mut rig, viper);
    // The plane is destroyed while the AI flies it.
    rig.host
        .world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == 0)
        .unwrap()
        .hp = 0;
    assert!(rig.run_until(Duration::from_secs(2), |r| !away_mark(r, viper)));
    assert_eq!(rig.clients[viper].notices, [LOST_WHILE_AWAY]);
    assert!(logged(
        &rig,
        "Viper",
        &LobbyEvent::AwayEnded {
            plane: 0,
            lost: true
        }
    ));
    // Nothing to take back; co-op has no revival, so no other plane either,
    // while Cobra may take one. Viper watches on.
    rig.clients[viper].send(&Message::Back);
    rig.clients[viper].take(Some(1));
    rig.clients[cobra].take(Some(1));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(cobra)));
    rig.run(Duration::from_millis(200));
    assert_eq!(refused(&rig, viper, kind::BACK), [NOT_AWAY]);
    assert_eq!(rig.clients[viper].seat_refused, [revive::NO_REVIVAL]);
    assert!(peer(&rig, "Viper").watch.is_some());
}

#[test]
fn a_player_who_stops_watching_or_leaves_frees_its_plane() {
    let (mut rig, viper, cobra) = pair();
    go_away(&mut rig, viper);
    // Viper stops watching: it leaves its plane to the AI, and may Join
    // again as any player in the lobby (it still holds the slot).
    rig.clients[viper].send(&Message::Observe(Observe::Stop));
    assert!(rig.run_until(Duration::from_secs(1), |r| !away_mark(r, viper)));
    assert!(logged(
        &rig,
        "Viper",
        &LobbyEvent::AwayEnded {
            plane: 0,
            lost: false
        }
    ));
    assert!(rig.host.rejoin.reserved.is_empty());
    assert_eq!(pilot(&rig, 0), Pilot::Ai);
    let flight = rig.clients[viper].seated.as_ref().unwrap().flight;
    rig.clients[viper].take(Some(0));
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.clients[viper]
            .seated
            .as_ref()
            .is_some_and(|s| s.flight != flight)
    }));

    // Viper goes away and leaves the game: plane 0 is free for Cobra.
    go_away(&mut rig, viper);
    rig.clients[viper].client.disconnect(DisconnectReason::Left);
    rig.run(Duration::from_millis(300));
    assert!(rig.host.rejoin.reserved.is_empty());
    rig.clients[cobra].take(Some(0));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(cobra)));
    assert_eq!(rig.clients[cobra].seated.as_ref().unwrap().plane, 0);
    assert!(rig.faults().is_empty());
}

#[test]
fn a_mission_with_only_away_players_is_not_empty() {
    let (mut rig, viper, _) = pair_with(HostConfig {
        empty_timeout: Duration::from_secs(1),
        ..config()
    });
    let before = rig.clients[viper].seated.clone().unwrap();
    go_away(&mut rig, viper);
    rig.run(Duration::from_secs(3));
    assert_eq!(rig.host.phase(), Phase::Flying);
    come_back(&mut rig, viper, before.flight);
}
