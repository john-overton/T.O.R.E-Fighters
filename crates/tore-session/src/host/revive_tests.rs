//! Death and revival on the host (slice F2-V; docs/ARCHITECTURE.md, "Death,
//! revival and lives"), on the network simulator: each respawn rule, lives,
//! the delay, Join after a loss, lock sides, a late joiner's copy of the
//! spawned planes, and a lost plane held for its player in the lobby and
//! abandoned when it leaves the game.
//!
//! A plane is lost the way a mission loses one: the test kills its pilot
//! between ticks (the synthetic aircraft has no ejection seat), and the
//! host's next tick notices.

use super::*;
use crate::host::revive;
use crate::settings::{Mode, Respawn, number};
use crate::wire::messages::{Revival, Spawned};
use tore_world::world::revive::RevivalWeapons;

/// A host with every plane open, `friendly` and `enemy` planes, its
/// settings PvP with `settings` applied, and two players flying: Viper in
/// plane 0 and Cobra in the enemy's lead.
fn two_sides(friendly: usize, enemy: usize, settings: &[(u8, u32)]) -> (Rig, usize, usize) {
    let mut rig = Rig::new(
        spec(friendly, enemy, 20),
        HostConfig {
            open_planes: OpenPlanes::All,
            ..config()
        },
        LinkConfig::one_way(5 * MS),
    );
    rig.host
        .settings
        .apply(&[(number::MODE, Mode::Pvp.value())])
        .unwrap();
    rig.host.settings.apply(settings).unwrap();
    let viper = rig.join(|c| c.callsign = "Viper".into());
    rig.clients[viper].ready = Some(Some(0));
    let cobra = rig.join(|c| c.callsign = "Cobra".into());
    rig.clients[cobra].ready = Some(Some(friendly as u32));
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.seated(viper) && r.seated(cobra)
    }));
    (rig, viper, cobra)
}

fn respawn(rule: Respawn) -> (u8, u32) {
    (number::RESPAWN, rule.value())
}

/// The plane `client` flies now, as its last Seated said.
fn plane_of(rig: &Rig, client: usize) -> u32 {
    rig.clients[client].seated.as_ref().unwrap().plane
}

/// Kills the pilot of `client`'s plane and runs until it hears Revival.
fn lose(rig: &mut Rig, client: usize) -> Revival {
    let plane = PlaneId(plane_of(rig, client));
    let heard = rig.clients[client].revivals.len();
    rig.host
        .world
        .cockpits
        .iter_mut()
        .find(|c| c.plane == plane)
        .unwrap()
        .flight
        .systems
        .pilot
        .dead = true;
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        r.clients[client].revivals.len() > heard
    }));
    rig.clients[client].revivals.last().unwrap().clone()
}

/// Asks to fly again (message 28).
fn revive(rig: &mut Rig, client: usize) {
    let mission = rig.clients[client].number();
    rig.clients[client].send(&Message::Revive { mission });
}

/// Runs until `client` is seated in a plane other than `old`.
fn reseated(rig: &mut Rig, client: usize, old: u32) -> bool {
    rig.run_until(Duration::from_secs(2), |r| {
        r.clients[client]
            .seated
            .as_ref()
            .is_some_and(|s| s.plane != old)
    })
}

fn refusals(rig: &Rig, client: usize) -> Vec<String> {
    rig.clients[client]
        .refused
        .iter()
        .filter(|(kind, _)| *kind == kind::REVIVE)
        .map(|(_, reason)| reason.clone())
        .collect()
}

fn pilot(rig: &Rig, plane: u32) -> Pilot {
    rig.host.world().roster.plane(PlaneId(plane)).unwrap().pilot
}

/// Leaves the flight and runs until the debrief is in and the player is
/// back in the lobby.
fn back_to_lobby(rig: &mut Rig, client: usize) {
    rig.clients[client].debrief = None;
    rig.clients[client].leave_flight();
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.clients[client].debrief.is_some()
    }));
    rig.run(Duration::from_millis(200));
}

#[test]
fn a_lost_player_is_told_and_flies_again_in_a_new_plane() {
    let (mut rig, viper, cobra) = two_sides(2, 2, &[respawn(Respawn::Revive)]);
    let revival = lose(&mut rig, viper);
    assert_eq!(
        revival,
        Revival {
            rule: Respawn::Revive,
            lives: None,
            wait_seconds: 0,
            why: None,
        }
    );
    let seat = rig.clients[viper].seated.as_ref().unwrap().seat;
    revive(&mut rig, viper);
    assert!(reseated(&mut rig, viper, 0));
    let seated = rig.clients[viper].seated.clone().unwrap();
    // A new plane in Viper's wing, the next id, the same seat.
    assert_eq!((seated.plane, seated.seat), (4, seat));
    assert_eq!(pilot(&rig, 4), Pilot::Human(SeatId(seat)));
    assert_eq!(pilot(&rig, 0), Pilot::Lost);
    let entry = seated.roster.planes.iter().find(|p| p.id == 4).unwrap();
    assert_eq!(
        entry.pilot,
        RosterPilot::Human {
            seat,
            callsign: "Viper".into()
        }
    );
    // Every player is told of the new plane.
    for client in [viper, cobra] {
        let spawned: &Spawned = rig.clients[client].spawned.last().expect("Spawned");
        assert_eq!(spawned.plane, 4);
        assert_eq!(
            (spawned.wing.side, spawned.wing.index, spawned.member),
            (Side::Friendly, 0, 2)
        );
        assert_eq!(spawned.aircraft, AircraftId::F18);
    }
    let spawned = rig.clients[viper].spawned[0].clone();
    assert_eq!(spawned.tick, seated.tick);
    // At the mission's 10,000 feet or above, with the spawn's stores.
    assert!(spawned.spawn.position[1] >= 10_000.);
    assert_eq!(
        seated
            .loadout
            .stations
            .iter()
            .map(|s| s.quantity)
            .collect::<Vec<_>>(),
        spawned
            .spawn
            .loadout
            .stations
            .iter()
            .map(|s| s.quantity)
            .collect::<Vec<_>>()
    );
    // A revival's plane seats nobody more.
    let status = rig.host.status(rig.net.now());
    assert_eq!((status.aircraft, status.capacity), (5, 4));
    // Viper flies on in it, and nothing went wrong.
    rig.clients[viper].flying = true;
    rig.run(Duration::from_millis(500));
    assert!(
        rig.host
            .world()
            .cockpits
            .iter()
            .any(|c| c.plane == PlaneId(4))
    );
    assert!(!rig.host.world().plane_lost(PlaneId(4)));
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
    for client in [viper, cobra] {
        assert!(
            rig.clients[client].errors.is_empty(),
            "{:?}",
            rig.clients[client].errors
        );
    }
    // A plane not lost is not revived.
    revive(&mut rig, viper);
    rig.run(Duration::from_millis(100));
    assert_eq!(refusals(&rig, viper), ["Your aircraft is not lost."]);
}

#[test]
fn with_no_revival_the_lost_plane_is_held_until_the_player_leaves_the_game() {
    // Co-op's default: no respawn.
    let (mut rig, viper, cobra) = two_sides(2, 2, &[respawn(Respawn::None)]);
    let revival = lose(&mut rig, viper);
    assert_eq!(revival.rule, Respawn::None);
    assert_eq!(revival.why.as_deref(), Some(revive::NO_REVIVAL));
    revive(&mut rig, viper);
    rig.run(Duration::from_millis(100));
    assert_eq!(refusals(&rig, viper), [revive::NO_REVIVAL]);
    // Viper leaves its flight: back in the lobby, its seat holding the
    // wreck, which flies on with nobody's controls.
    let seat = rig.clients[viper].seated.as_ref().unwrap().seat;
    back_to_lobby(&mut rig, viper);
    assert_eq!(pilot(&rig, 0), Pilot::Human(SeatId(seat)));
    assert_eq!(rig.host.revival.held.len(), 1);
    let roster = rig.clients[cobra].roster.clone().unwrap();
    assert_eq!(
        roster.planes[0].pilot,
        RosterPilot::Human {
            seat,
            callsign: "Viper".into()
        }
    );
    // Join after the loss obeys the rule.
    rig.clients[viper].take(None);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        !r.clients[viper].seat_refused.is_empty()
    }));
    assert_eq!(rig.clients[viper].seat_refused, [revive::NO_REVIVAL]);
    // Another player cannot take the held wreck.
    let hawk = rig.join(|c| c.callsign = "Hawk".into());
    rig.clients[hawk].ready = Some(Some(0));
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        !r.clients[hawk].seat_refused.is_empty()
    }));
    assert_eq!(
        rig.clients[hawk].seat_refused[0],
        "Plane 0 is destroyed or has lost its pilot."
    );
    // Viper leaves the game: its wreck is abandoned to the mission.
    rig.clients[viper].client.disconnect(DisconnectReason::Left);
    assert!(rig.run_until(Duration::from_secs(3), |r| pilot(r, 0) == Pilot::Lost));
    assert!(rig.host.revival.held.is_empty());
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn lives_run_out() {
    let (mut rig, viper, _) = two_sides(2, 2, &[respawn(Respawn::Revive), (number::LIVES, 1)]);
    let revival = lose(&mut rig, viper);
    assert_eq!((revival.lives, revival.why), (Some(1), None));
    revive(&mut rig, viper);
    assert!(reseated(&mut rig, viper, 0));
    let revival = lose(&mut rig, viper);
    assert_eq!(revival.lives, Some(0));
    assert_eq!(revival.why.as_deref(), Some(revive::NO_LIVES));
    revive(&mut rig, viper);
    rig.run(Duration::from_millis(100));
    assert_eq!(refusals(&rig, viper), [revive::NO_LIVES]);
    assert_eq!(plane_of(&rig, viper), 4);
}

#[test]
fn the_delay_counts_from_the_loss() {
    let (mut rig, viper, _) = two_sides(
        2,
        2,
        &[respawn(Respawn::Revive), (number::REVIVE_DELAY, 60)],
    );
    let revival = lose(&mut rig, viper);
    assert_eq!(revival.wait_seconds, 60);
    revive(&mut rig, viper);
    rig.run(Duration::from_millis(100));
    let refused = refusals(&rig, viper);
    assert_eq!(refused.len(), 1);
    assert!(
        refused[0] == "You can fly again in 1:00." || refused[0] == "You can fly again in 0:59.",
        "{refused:?}"
    );
    // The wait counts down from the loss.
    rig.run(Duration::from_millis(1500));
    revive(&mut rig, viper);
    rig.run(Duration::from_millis(100));
    let refused = refusals(&rig, viper);
    assert!(
        refused[1] == "You can fly again in 0:59." || refused[1] == "You can fly again in 0:58.",
        "{refused:?}"
    );
    assert_ne!(refused[0], refused[1]);
    // A minute after the loss Viper flies again.
    rig.run(Duration::from_secs(59));
    revive(&mut rig, viper);
    assert!(reseated(&mut rig, viper, 0), "{:?}", refusals(&rig, viper));
}

#[test]
fn join_after_a_loss_counts_as_a_revival() {
    let (mut rig, viper, _) = two_sides(2, 2, &[respawn(Respawn::Revive), (number::LIVES, 1)]);
    lose(&mut rig, viper);
    back_to_lobby(&mut rig, viper);
    // Join: a new plane by the revival, not the slot's lost one.
    rig.clients[viper].flying = true;
    rig.clients[viper].take(None);
    assert!(reseated(&mut rig, viper, 0));
    assert_eq!(plane_of(&rig, viper), 4);
    assert!(rig.host.revival.held.is_empty());
    // The life is used: the next loss and Join are refused.
    lose(&mut rig, viper);
    back_to_lobby(&mut rig, viper);
    rig.clients[viper].take(None);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        !r.clients[viper].seat_refused.is_empty()
    }));
    assert_eq!(rig.clients[viper].seat_refused, [revive::NO_LIVES]);
}

#[test]
fn the_ai_slot_rule_takes_a_free_ai_aircraft_of_the_side_with_the_weapons_rule() {
    let (mut rig, viper, cobra) = two_sides(
        3,
        2,
        &[
            respawn(Respawn::AiSlot),
            (number::REVIVE_WEAPONS, RevivalWeapons::Guns.value()),
        ],
    );
    // Friendly planes 0 to 2, enemy planes 3 and 4. The King has closed
    // plane 1's slot (slice F2-1): the AI keeps it.
    assert_eq!(plane_of(&rig, cobra), 3);
    let king = *rig.host.peers.keys().next().unwrap();
    rig.host
        .lock_slot(king, 1, &crate::wire::messages::Lock::Closed)
        .unwrap();
    lose(&mut rig, viper);
    revive(&mut rig, viper);
    assert!(reseated(&mut rig, viper, 0));
    // Its own wing's free aircraft, lowest open id first, on its own side.
    assert_eq!(plane_of(&rig, viper), 2);
    assert_eq!(pilot(&rig, 0), Pilot::Lost);
    assert_eq!(pilot(&rig, 1), Pilot::Ai);
    // Guns only: every station but the gun empty.
    let own = rig.host.world().combat.state.ownship(2).unwrap();
    for (station, ammo) in own.configuration().stations.iter().zip(&own.ammo) {
        let carried = ammo & 0x7fff;
        if tore_sim::combat::live::is_gun(&station.weapon) {
            assert!(carried > 0);
        } else {
            assert_eq!(carried, 0, "{}", station.weapon.source);
        }
    }
    // No new plane: nobody was sent Spawned.
    assert!(rig.clients[cobra].spawned.is_empty());
    // The enemy's last free AI aircraft, then none.
    lose(&mut rig, cobra);
    revive(&mut rig, cobra);
    assert!(reseated(&mut rig, cobra, 3));
    assert_eq!(plane_of(&rig, cobra), 4);
    lose(&mut rig, cobra);
    revive(&mut rig, cobra);
    rig.run(Duration::from_millis(100));
    assert_eq!(refusals(&rig, cobra), [revive::NO_AI_SLOT]);
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn revivals_keep_a_player_on_its_side_under_lock_sides() {
    for rule in [Respawn::Revive, Respawn::AiSlot] {
        let (mut rig, viper, cobra) = two_sides(3, 3, &[respawn(rule), (number::LOCK_SIDES, 1)]);
        for client in [viper, cobra] {
            let side = |rig: &Rig| {
                rig.host
                    .world()
                    .roster
                    .plane(PlaneId(plane_of(rig, client)))
                    .unwrap()
                    .slot
                    .wing
                    .side
            };
            let before = side(&rig);
            let old = plane_of(&rig, client);
            lose(&mut rig, client);
            revive(&mut rig, client);
            assert!(reseated(&mut rig, client, old), "{rule:?}");
            assert_eq!(side(&rig), before, "{rule:?}");
        }
    }
}

#[test]
fn a_late_joiner_is_told_of_every_spawned_plane() {
    let (mut rig, viper, _) = two_sides(2, 2, &[respawn(Respawn::Revive)]);
    lose(&mut rig, viper);
    revive(&mut rig, viper);
    assert!(reseated(&mut rig, viper, 0));
    let hawk = rig.join(|c| c.callsign = "Hawk".into());
    rig.clients[hawk].ready = None;
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        !r.clients[hawk].spawned.is_empty()
    }));
    assert_eq!(rig.clients[hawk].spawned, rig.clients[viper].spawned);
    assert_eq!(rig.host.revival.spawned.len(), 1);
}
