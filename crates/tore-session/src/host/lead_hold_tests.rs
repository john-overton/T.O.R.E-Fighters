//! The lead hold on the host (the lobby pass's slice R2; docs/ARCHITECTURE.md,
//! "Lead succession"), on the network simulator with the scripted test
//! clients: the host turns the mission core's hold on in a game with
//! revival and never with `respawn none`; a lost lead is stood in for and
//! given back on revival; an owner in the lobby with a held seat, or away,
//! keeps the lead; an owner who leaves the game, or leaves the flight with a
//! living plane, passes it on; and the log says each change.
//!
//! Mounted inside `revive_tests.rs`, whose rig helpers it shares.

use super::*;
use tore_sim::ai::launch::{Side, WingId};
use tore_world::world::lead_hold::LeadOwner;

const BLUE_1: WingId = WingId {
    side: Side::Friendly,
    index: 0,
};
const RED_1: WingId = WingId {
    side: Side::Enemy,
    index: 0,
};

/// The host's log lines about the lead hold, as "wing words".
fn lead_lines(rig: &Rig) -> Vec<String> {
    rig.logs
        .iter()
        .filter_map(|log| match log {
            HostLog::Lobby {
                callsign,
                event: LobbyEvent::Lead(words),
                ..
            } => Some(format!("{callsign} {words}")),
            _ => None,
        })
        .collect()
}

fn leader(rig: &Rig) -> Option<u32> {
    rig.host
        .world()
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .wing_leader(tore_world::ai_wings::FRIENDLY_SIDE, 0)
}

fn owner(rig: &Rig) -> Option<LeadOwner> {
    rig.host.world().lead_owner(BLUE_1)
}

/// Viper's seat now.
fn seat_of(rig: &Rig, client: usize) -> SeatId {
    SeatId(rig.clients[client].seated.as_ref().unwrap().seat)
}

#[test]
fn the_host_holds_leads_with_revival_and_never_with_none() {
    let (rig, viper, cobra) = two_sides(2, 2, &[respawn(Respawn::Revive)]);
    let world = rig.host.world();
    assert!(world.lead_hold());
    // Viper flies Blue 1's lead (plane 0), Cobra Red 1's (plane 2).
    assert_eq!(owner(&rig), Some(LeadOwner::Seat(seat_of(&rig, viper))));
    assert_eq!(
        world.lead_owner(RED_1),
        Some(LeadOwner::Seat(seat_of(&rig, cobra)))
    );
    let lines = lead_lines(&rig);
    assert!(
        lines.contains(&"Blue 1 lead belongs to Viper".to_owned()),
        "{lines:?}"
    );
    // `respawn none`: no hold, nothing owned, nothing logged.
    let (rig, _, _) = two_sides(2, 2, &[respawn(Respawn::None)]);
    assert!(!rig.host.world().lead_hold());
    assert!(rig.host.world().lead_owners().is_empty());
    assert!(lead_lines(&rig).is_empty());
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn a_lost_lead_is_stood_in_for_and_given_back_on_revival() {
    let (mut rig, viper, _) = two_sides(2, 2, &[respawn(Respawn::Revive)]);
    let seat = seat_of(&rig, viper);
    lose(&mut rig, viper);
    rig.run(Duration::from_millis(100));
    assert_eq!(leader(&rig), Some(1));
    assert!(rig.host.world().lead_acting(BLUE_1));
    assert_eq!(owner(&rig), Some(LeadOwner::Seat(seat)));
    revive(&mut rig, viper);
    assert!(reseated(&mut rig, viper, 0));
    rig.run(Duration::from_millis(100));
    assert_eq!(leader(&rig), Some(4));
    assert!(!rig.host.world().lead_acting(BLUE_1));
    let lines = lead_lines(&rig);
    for line in [
        "Blue 1 lead passes to plane 1 (AI), standing in for Viper",
        "Blue 1 lead goes back to Viper in plane 4",
    ] {
        assert!(lines.contains(&line.to_owned()), "{line:?} in {lines:?}");
    }
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn an_owner_in_the_lobby_with_a_held_seat_keeps_the_lead() {
    let (mut rig, viper, _) = two_sides(2, 2, &[respawn(Respawn::Revive)]);
    let seat = seat_of(&rig, viper);
    lose(&mut rig, viper);
    back_to_lobby(&mut rig, viper);
    rig.run(Duration::from_secs(2));
    assert_eq!(owner(&rig), Some(LeadOwner::Seat(seat)));
    assert!(rig.host.world().lead_acting(BLUE_1));
    assert!(
        lead_lines(&rig)
            .iter()
            .all(|l| !l.contains("left the game"))
    );
}

#[test]
fn an_owner_who_leaves_the_game_passes_the_lead_on() {
    let (mut rig, viper, _) = two_sides(2, 2, &[respawn(Respawn::Revive)]);
    lose(&mut rig, viper);
    rig.clients[viper].client.disconnect(DisconnectReason::Left);
    assert!(rig.run_until(Duration::from_secs(3), |r| owner(r).is_none()));
    // Nobody else flies in Blue 1: its stand-in leads it as its own.
    assert_eq!(leader(&rig), Some(1));
    assert!(!rig.host.world().lead_acting(BLUE_1));
    let lines = lead_lines(&rig);
    for line in [
        "Blue 1 lead's owner, the player of seat 0, has left the game: the lead passes on",
        "Blue 1 lead has no owner now: the flight's own succession leads it",
    ] {
        assert!(lines.contains(&line.to_owned()), "{line:?} in {lines:?}");
    }
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn an_away_owner_keeps_the_lead_through_its_plane() {
    let (mut rig, viper, _) = two_sides(2, 2, &[respawn(Respawn::Revive)]);
    rig.clients[viper].send(&Message::Away);
    assert!(rig.run_until(Duration::from_secs(2), |r| pilot(r, 0) == Pilot::Ai));
    rig.run(Duration::from_secs(2));
    assert_eq!(owner(&rig), Some(LeadOwner::Away(PlaneId(0))));
    assert_eq!(leader(&rig), Some(0));
    let lines = lead_lines(&rig);
    assert!(
        lines.contains(&"Blue 1 lead is kept for Viper (away), the AI flying plane 0".to_owned()),
        "{lines:?}"
    );
    assert!(lines.iter().all(|l| !l.contains("left the game")));
}

#[test]
fn an_owner_who_leaves_the_flight_with_a_living_plane_leaves_the_lead() {
    let (mut rig, viper, _) = two_sides(2, 2, &[respawn(Respawn::Revive)]);
    back_to_lobby(&mut rig, viper);
    assert!(rig.run_until(Duration::from_secs(2), |r| owner(r).is_none()));
    // The AI keeps plane 0 and its lead.
    assert_eq!(leader(&rig), Some(0));
    assert_eq!(pilot(&rig, 0), Pilot::Ai);
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}
