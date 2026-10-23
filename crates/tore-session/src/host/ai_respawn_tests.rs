//! AI respawn on the host (the lobby pass's slice R1; docs/ARCHITECTURE.md,
//! "Death, revival and lives"), on the network simulator with the scripted
//! test clients: an AI loss respawns after the King's delay at its original
//! spawn and every connection, a late joiner too, hears Spawned; lives count
//! per lineage; neither `respawn none` nor `ai-respawn off` respawns;
//! `ai-slot` refills its pool; a plane the AI flies for an away player is
//! that player's; a player who leaves the game leaves its lineage to the AI;
//! and a revival and a respawn made in one tick each get their own plane.
//!
//! Mounted inside `revive_tests.rs`, whose rig helpers it shares.

use super::*;
use crate::host::revive::{Holder, Pending};

/// The AI's plane `plane` is shot down.
fn destroy_ai(rig: &mut Rig, plane: u32) {
    rig.host
        .world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == plane)
        .unwrap()
        .hp = 0;
}

/// The host's log lines about AI respawns and revivals, as "who words".
fn revival_lines(rig: &Rig) -> Vec<String> {
    rig.logs
        .iter()
        .filter_map(|log| match log {
            HostLog::Lobby {
                callsign,
                event: LobbyEvent::Revival(words),
                ..
            } => Some(format!("{callsign} {words}")),
            _ => None,
        })
        .collect()
}

fn spawned_planes(rig: &Rig, client: usize) -> Vec<u32> {
    rig.clients[client]
        .spawned
        .iter()
        .map(|s| s.plane)
        .collect()
}

fn order_of(rig: &Rig, callsign: &str) -> u64 {
    rig.host
        .peers
        .values()
        .find(|p| p.callsign == callsign)
        .unwrap()
        .lobby
        .order
}

#[test]
fn an_ai_loss_respawns_at_its_original_spawn_after_the_delay() {
    let (mut rig, viper, cobra) = two_sides(
        2,
        2,
        &[respawn(Respawn::Revive), (number::REVIVE_DELAY, 60)],
    );
    // Planes 0 and 1 are friendly, 2 and 3 the enemy's; Viper flies 0,
    // Cobra 2; the AI flies 1 and 3. Each lineage's start was recorded at
    // the mission's first tick.
    let (origin, heading) = rig.host.revival.origins[&PlaneId(1)];
    assert_eq!(rig.host.revival.origins.len(), 4);
    destroy_ai(&mut rig, 1);
    rig.run(Duration::from_millis(1_000));
    let lineage = rig.host.revival.lineages[&PlaneId(1)].clone();
    let since = lineage.lost.expect("the loss is noted");
    assert_eq!(lineage.used, 0);
    assert!(
        spawned_planes(&rig, viper).is_empty(),
        "not before the delay"
    );
    assert!(rig.run_until(Duration::from_secs(60), |r| {
        !r.clients[cobra].spawned.is_empty()
    }));
    // A minute after the loss was noted, plane 4 in the lost one's wing as
    // its next member, the AI's.
    let spawned = rig.clients[cobra].spawned[0].clone();
    assert_eq!(u64::from(spawned.tick), since + 60 * TICKS_PER_SECOND);
    assert_eq!(spawned.plane, 4);
    assert_eq!(
        (spawned.wing.side, spawned.wing.index, spawned.member),
        (Side::Friendly, 0, 2)
    );
    assert_eq!(spawned.aircraft, AircraftId::F18);
    rig.run(Duration::from_millis(50));
    assert_eq!(spawned_planes(&rig, viper), [4]);
    assert_eq!(pilot(&rig, 4), Pilot::Ai);
    // At the original spawn, or stepped back whole miles along the reverse
    // of its heading while something flies within 2,000 ft of it.
    assert_eq!(spawned.spawn.heading_rad, heading);
    let off = (spawned.spawn.position[0] - origin[0]).hypot(spawned.spawn.position[2] - origin[2])
        / tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
    assert!(off < 5.01 && (off - off.round()).abs() < 1e-6, "{off} nm");
    let lineage = &rig.host.revival.lineages[&PlaneId(1)];
    assert_eq!((lineage.used, lineage.lost), (1, None));
    assert_eq!(rig.host.world().lineage_head(PlaneId(1)), PlaneId(4));
    // The log says so.
    let lines = revival_lines(&rig);
    assert!(
        lines[0] == "Blue 1-2 lost plane 1: the AI respawns it in 1:00",
        "{lines:?}"
    );
    assert!(
        lines[1].starts_with("Blue 1-3 respawned in plane 4 at its original spawn, x "),
        "{lines:?}"
    );
    // A late joiner hears of it with the mission.
    let hawk = rig.join(|c| c.callsign = "Hawk".into());
    rig.clients[hawk].ready = None;
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        !r.clients[hawk].spawned.is_empty()
    }));
    assert_eq!(spawned_planes(&rig, hawk), [4]);
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn lives_count_per_lineage() {
    let (mut rig, _, cobra) = two_sides(2, 2, &[respawn(Respawn::Revive), (number::LIVES, 1)]);
    destroy_ai(&mut rig, 1);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        r.clients[cobra].spawned.len() == 1
    }));
    assert_eq!(spawned_planes(&rig, cobra), [4]);
    // The respawn used the lineage's one life.
    destroy_ai(&mut rig, 4);
    rig.run(Duration::from_secs(2));
    assert_eq!(spawned_planes(&rig, cobra), [4]);
    assert!(rig.host.world().lineage_lost(PlaneId(1)));
    assert!(
        revival_lines(&rig)
            .iter()
            .any(|l| l == "Blue 1-3 lost plane 4: no lives left, the AI does not respawn it"),
        "{:?}",
        revival_lines(&rig)
    );
    // Another lineage has its own.
    destroy_ai(&mut rig, 3);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        r.clients[cobra].spawned.len() == 2
    }));
    assert_eq!(spawned_planes(&rig, cobra), [4, 5]);
    assert_eq!(rig.host.world().lineage_head(PlaneId(3)), PlaneId(5));
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn neither_no_respawn_nor_ai_respawn_off_respawns_the_ai() {
    for settings in [
        vec![respawn(Respawn::None)],
        vec![respawn(Respawn::Revive), (number::AI_RESPAWN, 0)],
    ] {
        let (mut rig, viper, _) = two_sides(2, 2, &settings);
        destroy_ai(&mut rig, 1);
        rig.run(Duration::from_secs(2));
        assert!(spawned_planes(&rig, viper).is_empty(), "{settings:?}");
        assert!(rig.host.revival.lineages.is_empty());
        assert_eq!(rig.host.world().roster.planes().len(), 4);
    }
}

#[test]
fn the_ai_slot_rule_respawns_and_refills_its_pool() {
    let (mut rig, viper, cobra) = two_sides(2, 2, &[respawn(Respawn::AiSlot)]);
    // Viper loses plane 0 and takes its wing's free AI aircraft, plane 1.
    lose(&mut rig, viper);
    revive(&mut rig, viper);
    assert!(reseated(&mut rig, viper, 0));
    assert_eq!(plane_of(&rig, viper), 1);
    // The wreck it left is the AI's: respawned as plane 4, a free AI
    // aircraft of the wing again.
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        r.clients[cobra].spawned.len() == 1
    }));
    assert_eq!(spawned_planes(&rig, cobra), [4]);
    assert_eq!(pilot(&rig, 4), Pilot::Ai);
    assert_eq!(rig.host.world().lineage_head(PlaneId(0)), PlaneId(4));
    lose(&mut rig, viper);
    revive(&mut rig, viper);
    assert!(reseated(&mut rig, viper, 1));
    assert_eq!(plane_of(&rig, viper), 4);
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn a_plane_the_ai_flies_for_an_away_player_is_that_players() {
    let (mut rig, viper, cobra) = two_sides(2, 2, &[respawn(Respawn::Revive)]);
    rig.clients[viper].send(&Message::Away);
    assert!(rig.run_until(Duration::from_secs(2), |r| pilot(r, 0) == Pilot::Ai));
    assert_eq!(
        rig.host.lineage_holder(PlaneId(0)),
        Holder::Player(order_of(&rig, "Viper"))
    );
    // The AI loses it: the player's revival, never the AI's respawn.
    destroy_ai(&mut rig, 0);
    rig.run(Duration::from_secs(2));
    assert!(spawned_planes(&rig, cobra).is_empty());
    let order = order_of(&rig, "Viper");
    assert_eq!(
        rig.host.revival.players[&order]
            .lost
            .map(|(plane, _)| plane),
        Some(PlaneId(0))
    );
    assert_eq!(rig.host.lineage_holder(PlaneId(0)), Holder::Player(order));
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn a_player_who_leaves_the_game_leaves_its_lineage_to_the_ai() {
    let (mut rig, viper, cobra) = two_sides(2, 2, &[respawn(Respawn::Revive)]);
    let order = order_of(&rig, "Viper");
    lose(&mut rig, viper);
    // Back in the lobby with its seat held: still Viper's.
    let seat = rig.clients[viper].seated.as_ref().unwrap().seat;
    back_to_lobby(&mut rig, viper);
    rig.run(Duration::from_secs(1));
    assert_eq!(
        rig.host.lineage_holder(PlaneId(0)),
        Holder::Seat(SeatId(seat))
    );
    assert!(spawned_planes(&rig, cobra).is_empty());
    // Viper leaves the game: the wreck is abandoned, and the AI respawns it.
    rig.clients[viper].client.disconnect(DisconnectReason::Left);
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.clients[cobra].spawned.len() == 1
    }));
    assert_eq!(spawned_planes(&rig, cobra), [4]);
    assert_eq!(pilot(&rig, 0), Pilot::Lost);
    assert_eq!(pilot(&rig, 4), Pilot::Ai);
    // Viper revives from it no more.
    assert!(
        rig.host
            .revival
            .players
            .get(&order)
            .is_none_or(|p| p.lost.is_none())
    );
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn a_revival_and_a_respawn_in_one_tick_each_get_their_own_plane() {
    let (mut rig, viper, cobra) = two_sides(2, 2, &[respawn(Respawn::Revive)]);
    lose(&mut rig, viper);
    // Between two ticks the AI loses plane 3 and Viper's revival is asked
    // for: the next tick makes both.
    let connection = *rig
        .host
        .peers
        .iter()
        .find(|(_, p)| p.callsign == "Viper")
        .unwrap()
        .0;
    let seat = SeatId(rig.clients[viper].seated.as_ref().unwrap().seat);
    destroy_ai(&mut rig, 3);
    rig.host
        .revival
        .pending
        .insert(connection, Pending::Revive { seat });
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        r.clients[cobra].spawned.len() == 2
    }));
    // The human's revival comes first and takes plane 4; the AI's respawn
    // plane 5, each as the step numbered it.
    let spawned = &rig.clients[cobra].spawned;
    assert_eq!(spawned[0].tick, spawned[1].tick);
    assert_eq!(spawned_planes(&rig, cobra), [4, 5]);
    assert_eq!(
        (spawned[0].wing.side, spawned[0].member),
        (Side::Friendly, 2)
    );
    assert_eq!((spawned[1].wing.side, spawned[1].member), (Side::Enemy, 2));
    assert!(reseated(&mut rig, viper, 0));
    assert_eq!(plane_of(&rig, viper), 4);
    assert_eq!(pilot(&rig, 5), Pilot::Ai);
    assert_eq!(rig.host.world().lineage_head(PlaneId(3)), PlaneId(5));
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}
