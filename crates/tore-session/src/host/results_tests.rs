//! The results on the host (slice F2-D; docs/ARCHITECTURE.md, "The
//! multiplayer debrief"), on the network simulator: every connection gets
//! Results at the mission's end, a row for every plane with its last human
//! pilot's callsign (kept after the player leaves), the final scores in PvP
//! only, and a revival's wreck and new plane both listed.

use super::*;
use crate::settings::{Mode, Respawn, number};
use crate::wire::messages::{ResultStatus, Results, Winner};
use tore_sim::combat::ledger::Kill;

/// A host with every plane open and the mode `mode`, and two players
/// flying: Viper in plane 0 and Cobra in the enemy's lead (plane 2 with two
/// planes a side). A third connection, Hawk, waits in the lobby.
fn crowd(mode: Mode, settings: &[(u8, u32)]) -> (Rig, [usize; 3]) {
    let mut rig = Rig::new(
        spec(2, 2, 20),
        HostConfig {
            open_planes: OpenPlanes::All,
            ..config()
        },
        LinkConfig::one_way(5 * MS),
    );
    rig.host
        .settings
        .apply(&[(number::MODE, mode.value())])
        .unwrap();
    rig.host.settings.apply(settings).unwrap();
    let viper = rig.join(|c| c.callsign = "Viper".into());
    rig.clients[viper].ready = Some(Some(0));
    let cobra = rig.join(|c| c.callsign = "Cobra".into());
    rig.clients[cobra].ready = Some(Some(2));
    let hawk = rig.join(|c| c.callsign = "Hawk".into());
    rig.clients[hawk].ready = None;
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.seated(viper) && r.seated(cobra) && r.clients[hawk].lobby.is_some()
    }));
    (rig, [viper, cobra, hawk])
}

fn row(results: &Results, plane: u32) -> &crate::wire::messages::ResultRow {
    results
        .rows
        .iter()
        .find(|row| row.plane == plane)
        .unwrap_or_else(|| panic!("no row for plane {plane}"))
}

/// The world as combat leaves it when `owner` shoots `victim` down.
fn shoot_down(host: &mut Host, owner: u32, victim: u32) {
    let state = &mut host.world.combat.state;
    state.ledger.kill(Kill {
        owner,
        victim,
        category: 0x8000,
        aircraft: true,
    });
    match state.ownship_mut(victim) {
        Some(own) => own.hp = 0,
        None => {
            state
                .targets
                .iter_mut()
                .find(|t| t.id == victim)
                .unwrap()
                .hp = 0;
        }
    }
}

#[test]
fn every_connection_gets_the_results_with_a_row_for_every_plane() {
    let (mut rig, clients) = crowd(Mode::Pvp, &[(number::KILL_LIMIT, 0)]);
    // Viper shoots down the enemy's AI plane (3), and Cobra's plane is hit.
    shoot_down(&mut rig.host, 0, 3);
    rig.run(Duration::from_millis(200));
    assert!(clients.iter().all(|c| rig.clients[*c].results.is_empty()));
    rig.host.end_mission(EndReason::TimeLimit, None);
    rig.run(Duration::from_millis(300));
    for client in clients {
        let c = &rig.clients[client];
        assert_eq!(c.results.len(), 1, "client {client}");
        assert!(c.errors.is_empty(), "{:?}", c.errors);
        let results = &c.results[0];
        assert_eq!(results.reason, EndReason::TimeLimit);
        // Four planes: Viper's, its AI wingman, Cobra's and the enemy AI.
        assert_eq!(
            results.rows.iter().map(|r| r.plane).collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        assert_eq!(row(results, 0).callsign.as_deref(), Some("Viper"));
        assert_eq!(row(results, 2).callsign.as_deref(), Some("Cobra"));
        assert_eq!(row(results, 1).callsign, None);
        assert_eq!(row(results, 3).callsign, None);
        assert_eq!(row(results, 0).status, ResultStatus::Alive);
        assert_eq!(row(results, 3).status, ResultStatus::Dead);
        assert_eq!(row(results, 3).damage, 1000);
        assert_eq!(row(results, 0).aircraft_kills, 1);
        assert_eq!(row(results, 2).aircraft_kills, 0);
        // PvP carries the final scores, with the winner by the tally.
        let scores = results.scores.as_ref().expect("PvP scores");
        assert_eq!(scores.winner, Winner::Side(Side::Friendly));
        assert_eq!(scores.players[0].callsign, "Viper");
        assert_eq!(scores.players[0].kills, 1);
    }
    // Results come before the mission's end, the scores before them.
    assert!(rig.clients[clients[0]].ended.is_some());
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn co_op_results_carry_no_scores() {
    let (mut rig, [viper, ..]) = crowd(Mode::Coop, &[]);
    rig.host.end_mission(EndReason::EndedByServer, None);
    rig.run(Duration::from_millis(300));
    let results = &rig.clients[viper].results[0];
    assert_eq!(results.reason, EndReason::EndedByServer);
    assert!(results.scores.is_none(), "co-op has no winner to name");
    assert_eq!(results.rows.len(), 4);
    // The words the log and the bots use for it.
    let summary = crate::client::results::summary(results);
    assert_eq!(
        summary,
        "4 aircraft, 2 flown by players: 0 Viper alive 0k, 1 AI alive 0k, \
         2 Cobra alive 0k, 3 AI alive 0k"
    );
}

#[test]
fn a_player_who_left_its_flight_keeps_its_planes_row() {
    let (mut rig, [viper, cobra, _]) = crowd(Mode::Pvp, &[]);
    rig.clients[cobra].leave_flight();
    rig.run(Duration::from_millis(500));
    assert_eq!(
        rig.host.world().roster.plane(PlaneId(2)).unwrap().pilot,
        tore_world::seats::Pilot::Ai,
        "the AI flies Cobra's plane again"
    );
    rig.host.end_mission(EndReason::TimeLimit, None);
    rig.run(Duration::from_millis(300));
    let results = &rig.clients[viper].results[0];
    assert_eq!(row(results, 2).callsign.as_deref(), Some("Cobra"));
    assert_eq!(row(results, 0).callsign.as_deref(), Some("Viper"));
    // Cobra, back in the lobby, is told too.
    assert_eq!(rig.clients[cobra].results.len(), 1);
}

#[test]
fn a_revival_lists_the_lost_plane_and_the_new_one_under_the_same_callsign() {
    let (mut rig, [viper, ..]) = crowd(Mode::Pvp, &[(number::RESPAWN, Respawn::Revive.value())]);
    rig.host
        .world
        .cockpits
        .iter_mut()
        .find(|c| c.plane.0 == 0)
        .unwrap()
        .flight
        .systems
        .pilot
        .dead = true;
    rig.run(Duration::from_millis(300));
    let mission = rig.clients[viper].number();
    rig.clients[viper].send(&Message::Revive { mission });
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.clients[viper]
            .seated
            .as_ref()
            .is_some_and(|s| s.plane != 0)
    }));
    rig.host.end_mission(EndReason::TimeLimit, None);
    rig.run(Duration::from_millis(300));
    let results = &rig.clients[viper].results[0];
    assert_eq!(results.rows.len(), 5);
    assert_eq!(row(results, 0).status, ResultStatus::Dead);
    assert_eq!(row(results, 0).callsign.as_deref(), Some("Viper"));
    assert_eq!(row(results, 4).status, ResultStatus::Alive);
    assert_eq!(row(results, 4).callsign.as_deref(), Some("Viper"));
    assert_eq!(row(results, 4).wing, row(results, 0).wing);
}

#[test]
fn a_retired_planes_row_survives_in_the_message() {
    let (mut rig, [viper, ..]) = crowd(Mode::Pvp, &[(number::RESPAWN, Respawn::Revive.value())]);
    shoot_down(&mut rig.host, 0, 3);
    rig.host
        .world
        .cockpits
        .iter_mut()
        .find(|c| c.plane.0 == 0)
        .unwrap()
        .flight
        .systems
        .pilot
        .dead = true;
    rig.run(Duration::from_millis(300));
    let mission = rig.clients[viper].number();
    rig.clients[viper].send(&Message::Revive { mission });
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.clients[viper]
            .seated
            .as_ref()
            .is_some_and(|s| s.plane != 0)
    }));
    // The wreck is retired for room, as a full mission would.
    rig.host.world.retire_plane(PlaneId(0)).unwrap();
    rig.host.end_mission(EndReason::TimeLimit, None);
    rig.run(Duration::from_millis(300));
    let results = &rig.clients[viper].results[0];
    let wreck = row(results, 0);
    assert_eq!(wreck.status, ResultStatus::Retired);
    assert_eq!(wreck.callsign.as_deref(), Some("Viper"));
    assert_eq!(wreck.aircraft_kills, 1, "what it did stays on the row");
    assert_eq!(results.rows.len(), 5);
}
