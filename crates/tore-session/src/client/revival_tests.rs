//! Slice F2-V's client test: a game whose plane is lost keeps the host's
//! Revival, asks to fly again, adds the spawned plane to its own copy of the
//! mission before the Seated message for it arrives, and flies on in it.
//! Synthetic resources, on the network simulator.

use super::tests::{Rig, level_script, spec};
use super::*;
use crate::host::{OpenPlanes, StartMode};
use crate::settings::{Mode, Respawn, number};
use tore_net::sim::LinkConfig;

const MS: Duration = Duration::from_millis(1);

#[test]
fn a_game_flies_again_in_the_plane_its_copy_of_the_mission_adds() {
    let mut rig = Rig::with_config(
        spec(2, 2, 20),
        LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
        7,
        |config| {
            config.start = StartMode::Now;
            config.open_planes = OpenPlanes::All;
        },
    );
    // The King's settings (slice F2-1 changes them in a lobby): PvP with
    // retail's revival.
    let settings = rig.host.settings_for_test();
    settings
        .apply(&[(number::MODE, Mode::Pvp.value())])
        .unwrap();
    settings
        .apply(&[
            (number::RESPAWN, Respawn::Revive.value()),
            (number::LIVES, 2),
        ])
        .unwrap();
    let viper = rig.join(|c| c.plane = Some(0), level_script());
    let cobra = rig.join(
        |c| {
            c.plane = Some(2);
            c.callsign = "Cobra".into();
        },
        level_script(),
    );
    assert!(rig.run_until(Duration::from_secs(5), |r| r.seated(viper)
        && r.seated(cobra)));
    rig.run(Duration::from_millis(500));
    assert!(rig.players[viper].client.revival().is_none());
    // Viper's pilot is killed between two of the host's ticks.
    let world = rig.host.world_for_test();
    let cockpit = world
        .cockpits
        .iter_mut()
        .find(|c| c.plane == PlaneId(0))
        .unwrap();
    cockpit.flight.systems.pilot.dead = true;
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.players[viper].client.revival().is_some()
    }));
    let client = &rig.players[viper].client;
    assert_eq!(
        client.revival_prompt().as_deref(),
        Some("Press Enter to fly again (2 lives left)")
    );
    assert!(client.may_fly_again());
    rig.players[viper].client.revive();
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[viper]
            .client
            .seat()
            .is_some_and(|(_, plane)| plane == PlaneId(4))
    }));
    // Both games' copies of the mission hold the new plane, in Viper's wing.
    for player in [viper, cobra] {
        let client = &rig.players[player].client;
        assert_eq!(client.spawned().len(), 1);
        let world = client.mission().unwrap();
        let entry = world.roster.plane(PlaneId(4)).expect("the spawned plane");
        assert_eq!(entry.slot.wing.side, tore_sim::ai::launch::Side::Friendly);
        assert_eq!(entry.slot.member, 2);
        assert!(world.ai_wings.as_ref().unwrap().configuration(4).is_some());
    }
    // Viper heard of the plane before it was seated in it.
    let events = &rig.players[viper].events;
    let spawned = events
        .iter()
        .position(|e| matches!(e, ClientEvent::Spawned(s) if s.plane == 4))
        .unwrap();
    let seated = events
        .iter()
        .position(|e| matches!(e, ClientEvent::Seated { plane: 4, .. }))
        .unwrap();
    assert!(spawned < seated);
    assert!(rig.players[viper].client.revival().is_none());
    // It flies on in the new plane, its prediction keeping to the host's.
    let corrections = rig.players[viper].client.corrections().len();
    rig.run(Duration::from_secs(2));
    let client = &mut rig.players[viper].client;
    assert_eq!(client.phase(), ClientPhase::Flying);
    let frame = client.frame(rig.net.now()).expect("a frame");
    assert_eq!(frame.plane, PlaneId(4));
    // As after any seating, at most a correction or two too small to show.
    let since = &client.corrections()[corrections..];
    assert!(
        since.len() <= 2 && since.iter().all(|c| !c.shown && c.feet < 1.),
        "{since:?}"
    );
    assert!(
        rig.host
            .world()
            .cockpits
            .iter()
            .any(|c| c.plane == PlaneId(4))
    );
}
