//! Taking a lineage's aircraft and the players' callsigns on the host (the
//! lobby pass's follow-up F1; docs/ARCHITECTURE.md, "Death, revival and
//! lives" and "Lead succession"), on the network simulator with the
//! scripted test clients: a late joiner whose slot's plane is lost takes the
//! lineage's respawned AI aircraft, or waits for the respawn and takes it;
//! the wait ends when the player leaves; with no respawn coming the take is
//! refused as before; the open planes and the King's slot locks are the
//! root's; a joiner who takes a flight's leading lineage plane owns its lead;
//! and the host names every seated player to the mission, so a stand-in's
//! line reads the owner's callsign.
//!
//! Mounted inside `revive_tests.rs`, whose rig helpers it shares.

use super::*;
use tore_sim::ai::launch::WingId;
use tore_world::world::lead_hold::LeadOwner;

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

/// A PvP host with `open` planes, two friendly and two enemy planes (Blue 1
/// planes 0 and 1, Red 1 planes 2 and 3), `settings` applied, and Viper
/// flying plane 0.
fn one_player(open: OpenPlanes, settings: &[(u8, u32)]) -> (Rig, usize) {
    let mut rig = Rig::new(
        spec(2, 2, 20),
        HostConfig {
            open_planes: open,
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
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    (rig, viper)
}

/// Runs until the AI has respawned `root`'s lineage: its new plane.
fn respawned(rig: &mut Rig, root: u32) -> u32 {
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.host.world().lineage_head(PlaneId(root)) != PlaneId(root)
    }));
    rig.host.world().lineage_head(PlaneId(root)).0
}

/// A late joiner asking for `slot` once the mission arrives.
fn joiner(rig: &mut Rig, callsign: &str, slot: u32) -> usize {
    let name = callsign.to_owned();
    let client = rig.join(|c| c.callsign = name);
    rig.clients[client].ready = Some(Some(slot));
    client
}

fn holder_of(rig: &Rig, slot: u32) -> Option<String> {
    rig.host
        .peers
        .values()
        .find(|peer| peer.lobby.slot == Some(PlaneId(slot)))
        .map(|peer| peer.callsign.clone())
}

#[test]
fn a_late_joiner_takes_its_slots_respawned_ai_aircraft() {
    // Only the listed planes are open: a lineage is open by its root.
    let (mut rig, _viper) =
        one_player(OpenPlanes::List(vec![0, 1, 2]), &[respawn(Respawn::Revive)]);
    destroy_ai(&mut rig, 1);
    let new = respawned(&mut rig, 1);
    assert_eq!(new, 4);
    assert!(rig.host.open(PlaneId(4)));
    assert!(!rig.host.open(PlaneId(3)));
    let hawk = joiner(&mut rig, "Hawk", 1);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(hawk)));
    assert_eq!(plane_of(&rig, hawk), new);
    assert_eq!(
        pilot(&rig, new),
        Pilot::Human(SeatId(rig.clients[hawk].seated.as_ref().unwrap().seat))
    );
    // The slot Hawk holds is the lineage's, plane 1's.
    assert_eq!(holder_of(&rig, 1).as_deref(), Some("Hawk"));
    // The host named Hawk to the mission before seating it.
    assert_eq!(
        rig.host.world().roster.plane_callsign(PlaneId(new)),
        Some("Hawk")
    );
    assert!(rig.clients[hawk].seat_refused.is_empty());
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn a_late_joiner_waits_for_the_respawn_and_takes_it() {
    let (mut rig, _viper) = one_player(
        OpenPlanes::All,
        &[respawn(Respawn::Revive), (number::REVIVE_DELAY, 60)],
    );
    destroy_ai(&mut rig, 1);
    rig.run(Duration::from_millis(500));
    assert!(rig.host.world().lineage_lost(PlaneId(1)));
    let hawk = joiner(&mut rig, "Hawk", 1);
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        !r.clients[hawk].notices.is_empty()
    }));
    let notice = rig.clients[hawk].notices[0].clone();
    assert!(
        (notice.starts_with("Plane 1 flies again in 0:5")
            || notice.starts_with("Plane 1 flies again in 1:00"))
            && notice.ends_with(": you take it then."),
        "{notice}"
    );
    assert!(!rig.seated(hawk));
    assert!(rig.clients[hawk].seat_refused.is_empty());
    assert_eq!(rig.host.revival.awaiting.len(), 1);
    // Ready, its slot plane 1's, while it waits.
    let entry = &rig
        .host
        .peers
        .values()
        .find(|p| p.callsign == "Hawk")
        .unwrap()
        .lobby;
    assert!(entry.ready);
    assert_eq!(entry.slot, Some(PlaneId(1)));
    // The respawn comes and Hawk flies it.
    assert!(rig.run_until(Duration::from_secs(62), |r| r.seated(hawk)));
    assert_eq!(plane_of(&rig, hawk), 4);
    assert!(rig.host.revival.awaiting.is_empty());
    let waited = rig.logs.iter().any(|log| {
        matches!(log, HostLog::Lobby { callsign, event: LobbyEvent::Revival(words), .. }
            if callsign == "Hawk" && words == "waits to take plane 1 when it respawns")
    });
    assert!(waited);
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn a_wait_ends_when_the_player_leaves_and_no_coming_respawn_is_refused() {
    let (mut rig, _viper) = one_player(
        OpenPlanes::All,
        &[respawn(Respawn::Revive), (number::REVIVE_DELAY, 60)],
    );
    destroy_ai(&mut rig, 1);
    rig.run(Duration::from_millis(500));
    let hawk = joiner(&mut rig, "Hawk", 1);
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        !r.host.revival.awaiting.is_empty()
    }));
    rig.clients[hawk].leave_flight();
    rig.run(Duration::from_millis(200));
    assert!(rig.host.revival.awaiting.is_empty());
    let entry = &rig
        .host
        .peers
        .values()
        .find(|p| p.callsign == "Hawk")
        .unwrap()
        .lobby;
    assert!(!entry.ready);
    // The respawn comes; Hawk, no longer waiting, stays in the lobby.
    rig.run(Duration::from_secs(61));
    assert_eq!(rig.host.world().lineage_head(PlaneId(1)), PlaneId(4));
    assert!(!rig.seated(hawk));

    // With AI respawn off the lineage is not coming back: the take is
    // refused as a lost plane's always was.
    let (mut rig, _viper) = one_player(
        OpenPlanes::All,
        &[respawn(Respawn::Revive), (number::AI_RESPAWN, 0)],
    );
    destroy_ai(&mut rig, 1);
    rig.run(Duration::from_millis(500));
    let hawk = joiner(&mut rig, "Hawk", 1);
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        !r.clients[hawk].seat_refused.is_empty()
    }));
    assert_eq!(
        rig.clients[hawk].seat_refused[0],
        "Plane 1 is destroyed or has lost its pilot."
    );
    assert!(rig.host.revival.awaiting.is_empty());
}

#[test]
fn the_kings_slot_lock_is_the_lineages() {
    let (mut rig, _viper) = one_player(OpenPlanes::All, &[respawn(Respawn::Revive)]);
    destroy_ai(&mut rig, 1);
    let new = respawned(&mut rig, 1);
    // The King keeps plane 1's slot for the AI: its respawn is closed too.
    let king = *rig.host.peers.keys().next().unwrap();
    rig.host
        .lock_slot(king, 1, &crate::wire::messages::Lock::Closed)
        .unwrap();
    let hawk = rig.join(|c| c.callsign = "Hawk".into());
    rig.clients[hawk].ready = None;
    rig.run(Duration::from_secs(1));
    rig.clients[hawk].take(Some(new));
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        !r.clients[hawk].seat_refused.is_empty()
    }));
    assert_eq!(
        rig.clients[hawk].seat_refused[0],
        "Plane 1 is closed: the AI flies it."
    );
}

#[test]
fn a_joiner_who_takes_a_leading_lineage_plane_owns_the_flights_lead() {
    let (mut rig, _viper) = one_player(OpenPlanes::All, &[respawn(Respawn::Revive)]);
    // Red 1 is all AI: both its planes are lost and respawn, the first
    // respawn leading.
    destroy_ai(&mut rig, 2);
    destroy_ai(&mut rig, 3);
    let first = respawned(&mut rig, 2);
    respawned(&mut rig, 3);
    let red = WingId {
        side: Side::Enemy,
        index: 0,
    };
    let leader = |r: &Rig| {
        r.host
            .world()
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .wing_leader(tore_world::ai_wings::ENEMY_SIDE, 0)
    };
    assert!(rig.run_until(Duration::from_secs(1), |r| leader(r) == Some(first)));
    assert_eq!(rig.host.world().lead_owner(red), None);
    let hawk = joiner(&mut rig, "Hawk", 2);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(hawk)));
    assert_eq!(plane_of(&rig, hawk), first);
    let seat = SeatId(rig.clients[hawk].seated.as_ref().unwrap().seat);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        r.host.world().lead_owner(red) == Some(LeadOwner::Seat(seat))
    }));
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn the_host_names_each_seated_player_and_a_stand_in_reads_the_owners_callsign() {
    let (mut rig, viper, cobra) = two_sides(2, 2, &[respawn(Respawn::Revive)]);
    let world = rig.host.world();
    assert_eq!(world.roster.plane_callsign(PlaneId(0)), Some("Viper"));
    assert_eq!(world.roster.plane_callsign(PlaneId(2)), Some("Cobra"));
    // Hawk flies Viper's wingman, plane 1.
    let hawk = joiner(&mut rig, "Hawk", 1);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(hawk)));
    assert_eq!(plane_of(&rig, hawk), 1);
    // Viper is lost: Hawk stands in and reads Viper's callsign.
    lose(&mut rig, viper);
    let line = "You lead the flight until Viper flies again.";
    let read = |r: &Rig| {
        r.clients[hawk].events.iter().any(|e| {
            matches!(&e.event, crate::wire::events::WireEvent::Message { text } if text == line)
        })
    };
    assert!(rig.run_until(Duration::from_secs(2), read));
    // A revival keeps the name on the new plane.
    revive(&mut rig, viper);
    assert!(reseated(&mut rig, viper, 0));
    let new = plane_of(&rig, viper);
    assert_eq!(
        rig.host.world().roster.plane_callsign(PlaneId(new)),
        Some("Viper")
    );
    let _ = cobra;
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}
