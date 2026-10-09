//! AI respawn across a host migration (the lobby pass's slice R1): the AI
//! loses a plane under a one minute delay, the host is cut off while the
//! respawn is pending, and the standby that takes over holds the same world
//! and the same lineage table, and respawns the plane when the delay ends.
//!
//! Mounted inside `resume_tests.rs`, whose rig it shares.

use super::*;
use crate::settings::{Respawn, number};

#[test]
fn a_respawn_pending_at_a_takeover_is_made_by_the_new_host() {
    let mut rig = Rig::new(crowd_spec(20));
    for (callsign, plane) in [("Viper", 1), ("Cobra", 2), ("Hawk", 3)] {
        rig.join(callsign, plane, true);
    }
    rig.host_mut()
        .settings
        .apply(&[
            (number::RESPAWN, Respawn::Revive.value()),
            (number::REVIVE_DELAY, 60),
        ])
        .unwrap();
    rig.fly();
    assert!(rig.host().settings.ai_respawn());
    // The AI's plane 4 is shot down; the standbys are appointed again so
    // their copy holds it (a change by hand is not journaled).
    shoot_down(rig.host_mut(), 0, 4);
    rig.host_mut().set_standbys_enabled(false);
    rig.run(Duration::from_millis(20));
    rig.host_mut().set_standbys_enabled(true);
    assert!(
        rig.run_until(Duration::from_secs(15), |r| r.host().ready_standbys().len()
            == 2),
        "the standbys ready again"
    );
    let since = rig.host().revival.lineages[&PlaneId(4)]
        .lost
        .expect("the loss is noted");
    assert!(rig.host().world.lineage_lost(PlaneId(4)));
    // Cut while the respawn is pending.
    rig.record_from = Some(rig.host().world.tick());
    rig.run(Duration::from_millis(500));
    rig.cut(0);
    assert!(
        rig.run_until(Duration::from_secs(3), |r| !r.takeovers.is_empty()),
        "standby 1 takes over"
    );
    rig.record_from = None;
    let (game, _, tick, coded) = rig.takeovers[0].clone();
    assert!(
        tick < since + 60 * TICKS_PER_SECOND,
        "the respawn is pending at T"
    );
    assert!(
        rig.checkpoints.get(&tick) == Some(&coded),
        "the new host's world at T differs from the old host's"
    );
    // The new host has the lineage table and makes the respawn on time.
    let new_host = rig.host_of(game);
    assert_eq!(new_host.revival.lineages[&PlaneId(4)].lost, Some(since));
    assert_eq!(
        new_host.revival.origins,
        rig.host().revival.origins,
        "the original spawns moved with the host"
    );
    assert!(
        rig.run_until(Duration::from_secs(62), |r| r
            .host_of(game)
            .world
            .lineage_head(PlaneId(4))
            != PlaneId(4)),
        "the new host respawns the plane"
    );
    let new_host = rig.host_of(game);
    let head = new_host.world.lineage_head(PlaneId(4));
    let spawned = new_host
        .revival
        .spawned
        .iter()
        .find(|s| s.plane == head.0)
        .expect("the respawn is kept for joiners");
    assert_eq!(u64::from(spawned.tick), since + 60 * TICKS_PER_SECOND);
    assert_eq!(new_host.revival.lineages[&PlaneId(4)].used, 1);
    assert_eq!(
        new_host.world.roster.plane(head).unwrap().pilot,
        tore_world::seats::Pilot::Ai
    );
}
