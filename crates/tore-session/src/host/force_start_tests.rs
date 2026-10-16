//! The King's force start (John, 2026-10-16): a 10 second countdown when
//! someone holding a slot is not ready, owned by the host. At zero, slot
//! holders who are still not ready fly with the loadout they have. Players
//! without a slot stay in the lobby. The minimum and an unheld plane with
//! the AI off are not overridden.
//!
//! Mounted inside `tests.rs`, whose rig it shares.

use super::*;
use crate::settings::number;
use crate::wire::messages::{SetReady, Slot, SlotRequest};

fn king_config(extra: &[(u8, u32)]) -> HostConfig {
    // A crowned first player makes the start the King's, the same rule a
    // server with `king first-player` uses. `StartMode::King` itself is only
    // legal for a game that has a house.
    HostConfig {
        start: StartMode::FirstPlayer,
        crown: CrownRule::FirstPlayer,
        settings: extra.to_vec(),
        ..config()
    }
}

fn opened(friendly: usize, host: HostConfig) -> Rig {
    Rig::new(spec(friendly, 0, 20), host, LinkConfig::one_way(10 * MS))
}

fn player(rig: &mut Rig, name: &str) -> usize {
    let client = rig.join(|c| c.callsign = name.into());
    rig.clients[client].ready = None;
    client
}

fn hold(rig: &mut Rig, client: usize, plane: u32) {
    let mission = rig.clients[client].number();
    rig.clients[client].send(&Message::Slot(Slot {
        mission,
        request: SlotRequest::Take(plane),
    }));
}

fn mark_ready(rig: &mut Rig, client: usize, ready: bool) {
    let mission = rig.clients[client].number();
    rig.clients[client].send(&Message::SetReady(SetReady { mission, ready }));
}

fn force(rig: &mut Rig, client: usize) {
    rig.clients[client].send(&Message::ForceStart);
}

fn notes(rig: &Rig) -> Vec<String> {
    rig.logs
        .iter()
        .filter_map(|log| match log {
            HostLog::Note { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn refusal(rig: &Rig, client: usize) -> Option<String> {
    rig.clients[client]
        .refused
        .iter()
        .rev()
        .find(|(kind, _)| *kind == crate::wire::messages::kind::FORCE_START)
        .map(|(_, reason)| reason.clone())
}

/// Two slot holders, only Hawk ready: the button's press starts the countdown.
#[test]
fn one_ready_player_starts_a_countdown_and_pressing_again_cancels_it() {
    let mut rig = opened(2, king_config(&[]));
    let king = player(&mut rig, "Maverick");
    let hawk = player(&mut rig, "Hawk");
    rig.run(Duration::from_millis(400));
    hold(&mut rig, king, 0);
    hold(&mut rig, hawk, 1);
    mark_ready(&mut rig, hawk, true);
    rig.run(Duration::from_millis(400));
    assert_eq!(rig.host.phase(), Phase::Lobby);

    force(&mut rig, king);
    rig.run(Duration::from_millis(400));
    let seconds = rig.clients[king]
        .lobby
        .as_ref()
        .and_then(|lobby| lobby.force_seconds);
    assert!(
        matches!(seconds, Some(n) if (8..=10).contains(&n)),
        "the host's seconds, got {seconds:?}"
    );
    assert!(
        rig.clients[hawk]
            .chat
            .iter()
            .any(|line| line == "The King is force starting in 10 seconds."),
        "{:?}",
        rig.clients[hawk].chat
    );
    assert!(
        notes(&rig)
            .iter()
            .any(|n| n == "Force start: the King is starting in 10 seconds."),
        "{:?}",
        notes(&rig)
    );

    force(&mut rig, king);
    rig.run(Duration::from_millis(400));
    assert_eq!(
        rig.clients[king]
            .lobby
            .as_ref()
            .and_then(|lobby| lobby.force_seconds),
        None
    );
    assert!(
        rig.clients[hawk]
            .chat
            .iter()
            .any(|line| line == "The King cancelled the force start.")
    );
    assert!(notes(&rig).iter().any(|n| n == "Force start cancelled."));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn nobody_ready_is_refused_and_everyone_ready_is_fly() {
    let mut rig = opened(2, king_config(&[]));
    let king = player(&mut rig, "Maverick");
    let hawk = player(&mut rig, "Hawk");
    rig.run(Duration::from_millis(400));
    hold(&mut rig, king, 0);
    hold(&mut rig, hawk, 1);
    rig.run(Duration::from_millis(300));
    force(&mut rig, king);
    rig.run(Duration::from_millis(300));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    assert_eq!(refusal(&rig, king).as_deref(), Some("Nobody is ready."));

    mark_ready(&mut rig, king, true);
    mark_ready(&mut rig, hawk, true);
    rig.run(Duration::from_millis(300));
    force(&mut rig, king);
    rig.run(Duration::from_millis(300));
    assert_eq!(rig.host.phase(), Phase::Lobby, "force start is not Fly");
    assert_eq!(refusal(&rig, king).as_deref(), Some("Everyone is ready."));
    rig.clients[king].send(&Message::Start);
    assert!(rig.run_until(Duration::from_secs(2), |r| r.host.phase() == Phase::Flying));
}

#[test]
fn everyone_becoming_ready_starts_at_once() {
    let mut rig = opened(2, king_config(&[]));
    let king = player(&mut rig, "Maverick");
    let hawk = player(&mut rig, "Hawk");
    rig.run(Duration::from_millis(400));
    hold(&mut rig, king, 0);
    hold(&mut rig, hawk, 1);
    mark_ready(&mut rig, hawk, true);
    rig.run(Duration::from_millis(300));
    force(&mut rig, king);
    rig.run(Duration::from_millis(300));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    mark_ready(&mut rig, king, true);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.seated(king) && r.seated(hawk)
    }));
    assert!(
        notes(&rig)
            .iter()
            .any(|n| n == "Force start: everyone was ready, so the mission started.")
    );
}

#[test]
fn an_unready_slot_holder_flies_and_a_player_without_a_slot_stays() {
    let mut rig = opened(2, king_config(&[]));
    let king = player(&mut rig, "Maverick");
    let hawk = player(&mut rig, "Hawk");
    let goose = player(&mut rig, "Goose");
    rig.run(Duration::from_millis(400));
    hold(&mut rig, king, 0);
    hold(&mut rig, hawk, 1);
    mark_ready(&mut rig, king, true);
    rig.run(Duration::from_millis(300));
    force(&mut rig, king);
    assert!(rig.run_until(Duration::from_secs(12), |r| {
        r.seated(king) && r.seated(hawk)
    }));
    assert!(!rig.seated(goose), "no slot, so the lobby");
    assert!(
        notes(&rig)
            .iter()
            .any(|n| n == "Force start: Hawk was not ready."),
        "{:?}",
        notes(&rig)
    );
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
    // No chosen loadout: the flight's list does not carry one for Hawk.
    let loads = rig.clients[hawk].loadouts.last();
    assert!(
        loads.is_none_or(|list| list.iter().all(|(plane, _)| *plane != 1)),
        "Hawk flies the standard stores, {loads:?}"
    );
}

#[test]
fn ai_off_refuses_while_a_slot_is_unheld_and_allows_it_once_every_slot_is_held() {
    let mut rig = opened(3, king_config(&[(number::AI, 0)]));
    let king = player(&mut rig, "Maverick");
    let hawk = player(&mut rig, "Hawk");
    rig.run(Duration::from_millis(400));
    hold(&mut rig, king, 0);
    hold(&mut rig, hawk, 1);
    mark_ready(&mut rig, king, true);
    rig.run(Duration::from_millis(300));
    force(&mut rig, king);
    rig.run(Duration::from_millis(300));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    assert!(
        refusal(&rig, king).is_some_and(|why| why.contains("AI") || why.contains("plane")),
        "{:?}",
        refusal(&rig, king)
    );

    let goose = player(&mut rig, "Goose");
    rig.run(Duration::from_millis(400));
    hold(&mut rig, goose, 2);
    rig.run(Duration::from_millis(300));
    force(&mut rig, king);
    rig.run(Duration::from_millis(400));
    assert!(
        rig.clients[king]
            .lobby
            .as_ref()
            .and_then(|lobby| lobby.force_seconds)
            .is_some(),
        "every slot is held, so the countdown runs"
    );
}

#[test]
fn the_minimum_is_not_overridden_and_a_crowned_server_counts_the_same_way() {
    let mut rig = opened(
        2,
        HostConfig {
            start: StartMode::FirstPlayer,
            crown: CrownRule::FirstPlayer,
            settings: vec![(number::MIN_PLAYERS, 2)],
            ..config()
        },
    );
    let king = player(&mut rig, "Maverick");
    let hawk = player(&mut rig, "Hawk");
    rig.run(Duration::from_millis(400));
    hold(&mut rig, king, 0);
    hold(&mut rig, hawk, 1);
    mark_ready(&mut rig, king, true);
    rig.run(Duration::from_millis(300));
    force(&mut rig, king);
    rig.run(Duration::from_millis(300));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    let why = refusal(&rig, king).unwrap_or_default();
    assert!(why.contains("minimum") || why.contains("Waiting"), "{why}");

    // The minimum is one: the crowned first player may force start.
    let mut rig = opened(
        2,
        HostConfig {
            start: StartMode::FirstPlayer,
            crown: CrownRule::FirstPlayer,
            settings: vec![(number::MIN_PLAYERS, 1)],
            ..config()
        },
    );
    let king = player(&mut rig, "Maverick");
    let hawk = player(&mut rig, "Hawk");
    rig.run(Duration::from_millis(400));
    hold(&mut rig, king, 0);
    hold(&mut rig, hawk, 1);
    mark_ready(&mut rig, king, true);
    rig.run(Duration::from_millis(300));
    force(&mut rig, king);
    rig.run(Duration::from_millis(400));
    assert!(
        rig.clients[king]
            .lobby
            .as_ref()
            .and_then(|lobby| lobby.force_seconds)
            .is_some()
    );
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}
