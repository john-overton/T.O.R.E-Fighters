//! The dedicated server's `min-players` (John, 2026-10-16): the mission does
//! not start until that many players hold slots and are ready; the wait is
//! said in the log as a note (never a fault) and in the lobby's text. On the
//! network simulator with the scripted test clients.
//!
//! Mounted inside `tests.rs`, whose rig it shares.

use super::*;
use crate::settings::number;

fn server(min: u32, extra: &[(u8, u32)], start: StartMode) -> HostConfig {
    let mut settings = vec![(number::MIN_PLAYERS, min)];
    settings.extend_from_slice(extra);
    HostConfig {
        settings,
        start,
        ..config()
    }
}

fn rig(friendly: usize, config: HostConfig) -> Rig {
    Rig::new(spec(friendly, 0, 20), config, LinkConfig::one_way(10 * MS))
}

/// A player who has joined and takes a plane only when told to.
fn idle(rig: &mut Rig) -> usize {
    let client = rig.join(|_| {});
    rig.clients[client].ready = None;
    client
}

/// The notes the host logged, in order.
fn notes(rig: &Rig) -> Vec<String> {
    rig.logs
        .iter()
        .filter_map(|log| match log {
            HostLog::Note { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_mission_waits_for_the_minimum_and_starts_at_the_last_ready_player() {
    let mut rig = rig(4, server(3, &[], StartMode::FirstPlayer));
    let players: Vec<usize> = (0..3).map(|_| idle(&mut rig)).collect();
    rig.run(Duration::from_secs(1));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    // N minus 2 and N minus 1 ready: still waiting, and nobody is refused.
    for (index, &player) in players.iter().take(2).enumerate() {
        rig.clients[player].take(Some(index as u32));
        rig.run(Duration::from_millis(500));
        assert_eq!(rig.host.phase(), Phase::Lobby, "{} ready", index + 1);
        assert!(rig.clients[player].seat_refused.is_empty());
    }
    assert_eq!(
        rig.host.start_wait_reason().as_deref(),
        Some("Waiting for 1 more ready player (minimum 3).")
    );
    assert_eq!(
        notes(&rig),
        [
            "Waiting for 3 more ready players (minimum 3).",
            "Waiting for 2 more ready players (minimum 3).",
            "Waiting for 1 more ready player (minimum 3).",
        ]
    );
    // The Nth ready player starts it.
    rig.clients[players[2]].take(Some(2));
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        players.iter().all(|&p| r.seated(p))
    }));
    assert_eq!(rig.host.phase(), Phase::Flying);
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
    assert_eq!(rig.host.start_wait_reason(), None);
}

#[test]
fn a_player_without_a_slot_or_not_ready_does_not_count() {
    let mut rig = rig(2, server(2, &[], StartMode::FirstPlayer));
    let (a, b) = (idle(&mut rig), idle(&mut rig));
    rig.run(Duration::from_secs(1));
    rig.clients[a].take(Some(0));
    rig.run(Duration::from_millis(500));
    // B is connected but has not taken a slot.
    assert_eq!(rig.host.phase(), Phase::Lobby);
    assert!(rig.host.start_wait_reason().unwrap().contains("1 more"));
    rig.clients[b].take(Some(1));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(a) && r.seated(b)));
}

#[test]
fn zero_and_one_start_with_the_first_ready_player_and_zero_start_now_flies_at_once() {
    // `start first-player`: 0 and 1 are the same.
    for min in [0, 1] {
        let mut rig = rig(2, server(min, &[], StartMode::FirstPlayer));
        let a = idle(&mut rig);
        rig.run(Duration::from_secs(1));
        assert_eq!(rig.host.phase(), Phase::Lobby);
        rig.clients[a].take(Some(0));
        assert!(
            rig.run_until(Duration::from_secs(2), |r| r.seated(a)),
            "{min}"
        );
        assert!(notes(&rig).is_empty(), "{:?}", notes(&rig));
    }
    // `start now` with no minimum flies with nobody; with a minimum it waits.
    let flying = rig_now(0);
    assert_eq!(flying.host.phase(), Phase::Flying);
    let mut waiting = rig_now(1);
    assert_eq!(waiting.host.phase(), Phase::Lobby);
    let a = idle(&mut waiting);
    waiting.run(Duration::from_secs(1));
    assert_eq!(waiting.host.phase(), Phase::Lobby);
    waiting.clients[a].take(Some(0));
    assert!(waiting.run_until(Duration::from_secs(2), |r| r.seated(a)));
}

fn rig_now(min: u32) -> Rig {
    let mut rig = rig(2, server(min, &[], StartMode::Now));
    while let Some(log) = rig.host.poll_log() {
        rig.logs.push(log);
    }
    rig
}

#[test]
fn start_now_waits_for_the_minimum_and_says_so_without_a_fault() {
    let mut rig = rig_now(2);
    // The lobby just opened: the log already says why it waits.
    assert_eq!(
        notes(&rig),
        ["Waiting for 2 more ready players (minimum 2)."]
    );
    let a = idle(&mut rig);
    rig.run(Duration::from_secs(1));
    rig.clients[a].take(Some(0));
    rig.run(Duration::from_millis(500));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    // The console's `start now` cannot bypass the minimum, and says again.
    rig.host.start_now();
    while let Some(log) = rig.host.poll_log() {
        rig.logs.push(log);
    }
    assert_eq!(rig.host.phase(), Phase::Lobby);
    assert_eq!(
        notes(&rig).last().map(String::as_str),
        Some("Waiting for 1 more ready player (minimum 2).")
    );
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
    let b = idle(&mut rig);
    rig.run(Duration::from_secs(1));
    rig.clients[b].take(Some(1));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(a) && r.seated(b)));
}

#[test]
fn ai_off_starts_at_the_minimum_with_empty_slots_and_accepts_a_late_join() {
    let mut rig = rig(4, server(2, &[(number::AI, 0)], StartMode::FirstPlayer));
    let a = idle(&mut rig);
    rig.run(Duration::from_secs(1));
    rig.clients[a].take(Some(0));
    rig.run(Duration::from_millis(500));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    assert_eq!(
        rig.host.start_wait_reason().as_deref(),
        Some("Waiting for 1 more ready player (minimum 2).")
    );
    let b = idle(&mut rig);
    rig.run(Duration::from_secs(1));
    rig.clients[b].take(Some(1));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(a) && r.seated(b)));
    assert_eq!(rig.host.world.cockpits.len(), 2);
    assert!(
        rig.host
            .world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actors()
            .is_empty()
    );
    let c = idle(&mut rig);
    rig.run(Duration::from_secs(1));
    rig.clients[c].take(Some(2));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(c)));
    assert_eq!(rig.host.world.cockpits.len(), 3);
    assert!(
        rig.host
            .world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actors()
            .is_empty()
    );
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn the_minimum_gates_only_the_start_and_applies_again_after_a_restart() {
    let mut rig = rig(3, server(2, &[], StartMode::FirstPlayer));
    let (a, b) = (idle(&mut rig), idle(&mut rig));
    rig.run(Duration::from_secs(1));
    rig.clients[a].take(Some(0));
    rig.clients[b].take(Some(1));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(a) && r.seated(b)));
    // One leaves: the mission goes on for the other.
    rig.clients[b].leave();
    rig.run(Duration::from_secs(2));
    assert_eq!(rig.host.phase(), Phase::Flying);
    // The next mission asks for the minimum again.
    rig.host.restart();
    rig.run(Duration::from_secs(1));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    assert_eq!(
        rig.host.start_wait_reason().as_deref(),
        Some("Waiting for 2 more ready players (minimum 2).")
    );
    assert_eq!(
        notes(&rig).last().map(String::as_str),
        Some("Waiting for 2 more ready players (minimum 2).")
    );
}

#[test]
fn the_minimum_never_asks_for_more_than_the_game_can_seat() {
    // Two slots and a minimum of 4 (a file's check refuses it; a lowered
    // player limit or a closed slot can still make it so): it waits for 2.
    let mut rig = rig(2, server(4, &[], StartMode::FirstPlayer));
    let (a, b) = (idle(&mut rig), idle(&mut rig));
    rig.run(Duration::from_secs(1));
    rig.clients[a].take(Some(0));
    rig.run(Duration::from_millis(500));
    assert_eq!(
        rig.host.start_wait_reason().as_deref(),
        Some("Waiting for 1 more ready player (minimum 2).")
    );
    rig.clients[b].take(Some(1));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(a) && r.seated(b)));
}

#[test]
fn a_larger_minimum_than_the_player_limit_is_refused_at_the_host() {
    let config = HostConfig {
        max_players: 4,
        ..server(5, &[], StartMode::FirstPlayer)
    };
    let error = config.validate().unwrap_err().to_string();
    assert!(
        error.contains("min-players is 5") && error.contains("max-players is 4"),
        "{error}"
    );
    assert!(
        HostConfig {
            max_players: 4,
            ..server(4, &[], StartMode::FirstPlayer)
        }
        .validate()
        .is_ok()
    );
}

#[test]
fn ai_off_start_now_with_none_still_waits_for_one_ready_human() {
    let mut rig = rig(4, server(0, &[(number::AI, 0)], StartMode::Now));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    rig.host.start_now();
    assert_eq!(rig.host.phase(), Phase::Lobby);
    let player = idle(&mut rig);
    rig.run(Duration::from_secs(1));
    rig.clients[player].take(Some(0));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(player)));
}
