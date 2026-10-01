//! The lobby on the network simulator (slice EF4's acceptance): a King and
//! players gather, take slots, arm, mark ready, fly, return and fly again;
//! the King's mission change; a player whose import cannot play the
//! mission; a kick; a player joining in flight; the dedicated server's
//! start rules; and a stalled game's recovery. Synthetic resources, the
//! client sessions driven through their lobby calls.

use super::tests::{Rig, spec, weave};
use super::*;
use crate::host::{AfterEnd, HostLog, Phase, StartMode};
use crate::wire::messages::{EndReason, LobbyPhase, kind};
use tore_net::sim::LinkConfig;
use tore_world::test_support::resources::resources;

const MS: Duration = Duration::from_millis(1);

/// A game a player hosts: the first player to join is the King, who starts
/// each mission; a mission's end returns everyone to the lobby at once.
fn kings_rig(friendly: usize) -> Rig {
    Rig::with_config(
        spec(friendly, 2, 20),
        LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
        7,
        |config| {
            config.king = Some(Rig::player_address(0));
            config.start = StartMode::King;
            config.after_end = AfterEnd::Restart;
            config.restart_delay = Duration::ZERO;
            config.empty_timeout = Duration::ZERO;
        },
    )
}

/// A player who drives the lobby itself, as a lobby screen would.
fn manual(rig: &mut Rig, callsign: &str) -> usize {
    let callsign = callsign.to_owned();
    rig.join(
        move |c| {
            c.callsign = callsign;
            c.auto_ready = false;
        },
        Box::new(|now, _, _| weave(now.as_secs_f64())),
    )
}

/// The synthetic F/A-18D's standard load.
fn standard() -> LoadoutSpec {
    let map = resources();
    let kind = tore_world::aircraft_type::AircraftType::load(&map, AircraftId::F18).unwrap();
    LoadoutSpec::of(
        &tore_sim::combat::loadout::Loadout::new(&kind.profile, |name| {
            map.get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
        })
        .unwrap(),
    )
}

fn lobby(rig: &Rig, player: usize) -> Option<&LobbyState> {
    rig.players[player].client.lobby()
}

fn me(rig: &Rig, player: usize) -> Option<&crate::wire::messages::LobbyPlayer> {
    lobby(rig, player).and_then(|l| l.me())
}

fn refusals(rig: &Rig, player: usize) -> Vec<(u8, String)> {
    rig.players[player]
        .events
        .iter()
        .filter_map(|e| match e {
            ClientEvent::Refused { request, reason } => Some((*request, reason.clone())),
            _ => None,
        })
        .collect()
}

fn count(rig: &Rig, player: usize, what: impl Fn(&ClientEvent) -> bool) -> usize {
    rig.players[player]
        .events
        .iter()
        .filter(|e| what(e))
        .count()
}

/// The ammunition of the ownship flying `plane` on the host.
fn ammo(rig: &Rig, plane: u32) -> Vec<u16> {
    rig.host
        .world()
        .combat
        .state
        .ownship(plane)
        .map(|own| own.ammo.clone())
        .unwrap_or_default()
}

/// Every player in the lobby has the mission and sees the others.
fn gathered(rig: &mut Rig, players: &[usize]) {
    let n = players.len();
    assert!(
        rig.run_until(Duration::from_secs(3), |r| {
            players.iter().all(|&p| {
                r.players[p].client.phase() == ClientPhase::Lobby
                    && lobby(r, p).is_some_and(|l| l.players.len() == n)
            })
        }),
        "the players gather"
    );
}

#[test]
fn a_king_and_two_players_arm_ready_fly_return_and_fly_again() {
    let mut rig = kings_rig(3);
    let king = manual(&mut rig, "Viper");
    let cobra = manual(&mut rig, "Cobra");
    let hawk = manual(&mut rig, "Hawk");
    gathered(&mut rig, &[king, cobra, hawk]);
    let l = lobby(&rig, cobra).unwrap();
    assert_eq!(l.king, Some(me(&rig, king).unwrap().id));
    assert!(lobby(&rig, king).unwrap().is_king());
    assert!(!l.is_king());
    assert_eq!(l.start, crate::wire::messages::StartRule::King);
    assert_eq!(l.slots.len(), 3, "the friendly planes");
    assert_eq!(l.phase, LobbyPhase::Lobby);

    // Slots.
    rig.players[king].client.take_slot(0);
    rig.players[cobra].client.take_slot(1);
    rig.players[hawk].client.take_any_slot();
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        [king, cobra, hawk]
            .iter()
            .all(|&p| me(r, p).is_some_and(|m| m.slot.is_some()))
    }));
    assert_eq!(me(&rig, hawk).unwrap().slot, Some(2), "the first free");
    // A held slot is refused to another.
    rig.players[hawk].client.take_slot(0);
    rig.run(Duration::from_millis(200));
    assert!(
        refusals(&rig, hawk)
            .iter()
            .any(|(k, r)| *k == kind::SLOT && r.contains("Viper holds plane 0")),
        "{:?}",
        refusals(&rig, hawk)
    );

    // Loadouts: the King's light one, Cobra's first refused with the page's
    // reason, then one without missiles; Hawk keeps the standard load.
    let mut light = standard();
    light.stations[0].quantity = 300;
    light.stations[1].quantity = 1;
    let mut over = standard();
    over.stations[1].quantity = 3;
    let mut guns = standard();
    guns.stations[1].quantity = 0;
    rig.players[king].client.send_loadout(Some(light.clone()));
    rig.players[cobra].client.send_loadout(Some(over));
    rig.run(Duration::from_millis(300));
    assert!(
        refusals(&rig, cobra)
            .iter()
            .any(|(k, r)| *k == kind::LOADOUT && r == "Station quantity exceeds capacity."),
        "{:?}",
        refusals(&rig, cobra)
    );
    rig.players[cobra].client.send_loadout(Some(guns.clone()));
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        me(r, king).is_some_and(|m| m.loadout) && me(r, cobra).is_some_and(|m| m.loadout)
    }));

    // Ready and start: refused until every slot holder is ready.
    rig.players[king].client.set_ready(true);
    rig.players[cobra].client.set_ready(true);
    rig.run(Duration::from_millis(200));
    rig.players[king].client.start_mission();
    rig.run(Duration::from_millis(200));
    assert!(
        refusals(&rig, king)
            .iter()
            .any(|(k, r)| *k == kind::START && r == "Not ready: Hawk."),
        "{:?}",
        refusals(&rig, king)
    );
    assert_eq!(rig.host.phase(), Phase::Lobby);
    // Only the King starts.
    rig.players[hawk].client.set_ready(true);
    rig.players[hawk].client.start_mission();
    rig.run(Duration::from_millis(200));
    assert!(
        refusals(&rig, hawk)
            .iter()
            .any(|(k, r)| *k == kind::START && r == "Only the King may do that.")
    );
    assert!(lobby(&rig, king).unwrap().all_ready());
    rig.players[king].client.start_mission();
    let everyone = [king, cobra, hawk];
    assert!(
        rig.run_until(Duration::from_secs(3), |r| everyone
            .iter()
            .all(|&p| r.seated(p))),
        "everyone flies"
    );
    // Each aircraft carries its player's loadout, on the host and as each
    // player was seated.
    assert_eq!(ammo(&rig, 0), [300, 1]);
    assert_eq!(ammo(&rig, 1), [500, 0]);
    assert_eq!(ammo(&rig, 2), [500, 2]);
    let seated = |p: usize| rig.players[p].client.loadout().cloned().unwrap();
    assert_eq!(
        seated(king)
            .stations
            .iter()
            .map(|s| s.quantity)
            .collect::<Vec<_>>(),
        [300, 1]
    );
    let fuel = seated(king).fuel_lbs;
    assert!(
        fuel <= light.fuel_lbs && fuel > light.fuel_lbs - 5.,
        "the loadout's fuel, less a tick's burn: {fuel}"
    );
    assert_eq!(seated(cobra).stations[1].quantity, 0);
    rig.run(Duration::from_secs(3));

    // The King ends the mission: debriefs, and everyone is back in the
    // lobby, still connected, slots and loadouts kept, ready cleared.
    rig.players[king].client.end_mission();
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        everyone.iter().all(|&p| {
            r.players[p]
                .events
                .iter()
                .any(|e| matches!(e, ClientEvent::Debrief(_)))
                && me(r, p).is_some_and(|m| !m.flying && !m.ready)
                && lobby(r, p).is_some_and(|l| l.phase == LobbyPhase::Lobby)
        })
    }));
    for &p in &everyone {
        assert_eq!(rig.players[p].client.phase(), ClientPhase::Lobby);
        assert!(rig.players[p].client.seat().is_none());
        assert!(rig.players[p].events.iter().any(|e| matches!(
            e,
            ClientEvent::MissionEnded(ended) if ended.reason == EndReason::EndedByServer
        )));
    }
    assert_eq!(me(&rig, king).unwrap().slot, Some(0));
    assert!(me(&rig, cobra).unwrap().loadout);

    // And again: a second flight of every connection, with the loadouts.
    for &p in &everyone {
        rig.players[p].client.set_ready(true);
    }
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        lobby(r, king).is_some_and(|l| l.all_ready())
    }));
    rig.players[king].client.start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        everyone.iter().all(|&p| r.seated(p))
    }));
    assert_eq!(ammo(&rig, 0), [300, 1]);
    assert_eq!(ammo(&rig, 1), [500, 0]);
    rig.run(Duration::from_secs(2));
    for &p in &everyone {
        let player = &rig.players[p];
        assert_eq!(
            count(&rig, p, |e| matches!(e, ClientEvent::Seated { .. })),
            2
        );
        assert!(
            !player
                .events
                .iter()
                .any(|e| matches!(e, ClientEvent::Closed(_))),
            "{:?}",
            player.events
        );
        let stats = player.client.clone_stats();
        assert!(stats.snapshots > 100, "{stats:?}");
    }
    assert!(
        !rig.logs.iter().any(|l| matches!(l, HostLog::Fault { .. })),
        "{:?}",
        rig.logs
    );
}

#[test]
fn the_kings_mission_change_reaches_everyone_frees_vanished_slots_and_clears_ready() {
    let mut rig = kings_rig(3);
    let king = manual(&mut rig, "Viper");
    let cobra = manual(&mut rig, "Cobra");
    gathered(&mut rig, &[king, cobra]);
    rig.players[king].client.take_slot(0);
    rig.players[cobra].client.take_slot(2);
    rig.run(Duration::from_millis(200));
    rig.players[king].client.set_ready(true);
    rig.players[cobra].client.set_ready(true);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        lobby(r, king).is_some_and(|l| l.all_ready())
    }));
    let loaded = |rig: &Rig, p: usize| count(rig, p, |e| matches!(e, ClientEvent::MissionLoaded));
    let before = (loaded(&rig, king), loaded(&rig, cobra));

    // A player may not change it; nor may the King to one the import
    // cannot build.
    let mut two = spec(2, 1, 10);
    rig.players[cobra].client.change_mission(&two);
    let mut unbuildable = spec(2, 1, 10);
    unbuildable.wings[3].aircraft = AircraftId::Su27;
    rig.players[king].client.change_mission(&unbuildable);
    rig.run(Duration::from_millis(300));
    assert!(
        refusals(&rig, cobra)
            .iter()
            .any(|(k, r)| *k == kind::CHANGE_MISSION && r == "Only the King may do that.")
    );
    assert!(
        refusals(&rig, king)
            .iter()
            .any(|(k, r)| *k == kind::CHANGE_MISSION && r.contains("SU27")),
        "{:?}",
        refusals(&rig, king)
    );

    // The King's change: two friendly planes now.
    two.separation_nm = 10;
    rig.players[king].client.change_mission(&two);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        loaded(r, king) > before.0
            && loaded(r, cobra) > before.1
            && lobby(r, cobra).is_some_and(|l| l.mission == 2)
    }));
    assert_eq!(rig.players[cobra].client.spec(), Some(&two));
    let l = lobby(&rig, cobra).unwrap();
    assert_eq!(l.slots.len(), 2);
    assert_eq!(me(&rig, cobra).unwrap().slot, None, "plane 2 is gone");
    assert_eq!(me(&rig, king).unwrap().slot, Some(0), "plane 0 stays");
    assert!(l.players.iter().all(|p| !p.ready), "{l:?}");
    assert!(rig.logs.iter().any(|l| matches!(
        l,
        HostLog::Lobby {
            event: crate::host::LobbyEvent::MissionChanged { number: 2, .. },
            ..
        }
    )));
    // A request meant for the old mission is refused.
    rig.players[cobra].client.number = Some(1);
    rig.players[cobra].client.take_slot(1);
    rig.run(Duration::from_millis(200));
    assert!(
        refusals(&rig, cobra)
            .iter()
            .any(|(_, r)| r == "The mission has changed; choose again.")
    );
}

#[test]
fn a_joiner_whose_import_lacks_the_content_is_told_and_stays() {
    let mut rig = kings_rig(2);
    let king = manual(&mut rig, "Viper");
    // The other player's import lacks the aircraft's weapon.
    let normal = Arc::clone(&rig.resources);
    let mut lacking = resources();
    lacking.remove("AIM9M.JT");
    rig.resources = Arc::new(lacking);
    let cobra = manual(&mut rig, "Cobra");
    rig.resources = normal;
    gathered(&mut rig, &[king, cobra]);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        lobby(r, king).is_some_and(|l| l.players.iter().any(|p| p.unable.is_some()))
    }));
    let p = &rig.players[cobra];
    assert!(p.events.iter().any(|e| matches!(
        e,
        ClientEvent::ContentRefused { names, reason }
            if names.contains(&"AIM9M.JT".to_string()) && reason.contains("differs")
    )));
    assert_eq!(p.client.phase(), ClientPhase::Lobby);
    // It may not take a slot; the King flies without it.
    rig.players[cobra].client.take_slot(1);
    rig.players[king].client.take_slot(0);
    rig.run(Duration::from_millis(200));
    assert!(
        refusals(&rig, cobra)
            .iter()
            .any(|(_, r)| r.starts_with("Your game cannot play this mission"))
    );
    rig.players[king].client.set_ready(true);
    rig.run(Duration::from_millis(200));
    rig.players[king].client.start_mission();
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(king)));
    rig.run(Duration::from_secs(1));
    assert!(!rig.closed(cobra));
    assert_eq!(rig.players[cobra].client.phase(), ClientPhase::Lobby);
}

#[test]
fn the_king_kicks_a_player_with_a_reason() {
    let mut rig = kings_rig(2);
    let king = manual(&mut rig, "Viper");
    let cobra = rig.join(
        |c| c.callsign = "Cobra".into(),
        Box::new(|now, _, _| weave(now.as_secs_f64())),
    );
    gathered(&mut rig, &[king, cobra]);
    rig.players[king].client.take_slot(0);
    rig.run(Duration::from_millis(200));
    rig.players[king].client.set_ready(true);
    // Cobra, automatic, has taken its slot and is ready; the King starts.
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        lobby(r, king).is_some_and(|l| l.all_ready() && l.players.iter().all(|p| p.slot.is_some()))
    }));
    rig.players[king].client.start_mission();
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(king)
        && r.seated(cobra)));
    rig.run(Duration::from_millis(500));
    let id = me(&rig, cobra).unwrap().id;
    // The King cannot kick the King.
    let own = me(&rig, king).unwrap().id;
    rig.players[king].client.kick(own, "");
    rig.players[king].client.kick(id, "Wrong aircraft");
    assert!(rig.run_until(Duration::from_secs(2), |r| r.closed(cobra)));
    let p = &rig.players[cobra];
    let reason = p
        .events
        .iter()
        .find_map(|e| match e {
            ClientEvent::Closed(reason) => Some(reason.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        p.client.close_text(&reason),
        "The King removed you from the game: Wrong aircraft"
    );
    assert!(matches!(
        reason,
        CloseReason::Disconnected {
            reason: DisconnectReason::Kicked,
            by_peer: true
        }
    ));
    assert!(
        refusals(&rig, king)
            .iter()
            .any(|(k, r)| *k == kind::KICK && r == "The King cannot kick the King.")
    );
    rig.run(Duration::from_millis(200));
    assert_eq!(
        rig.host.world().roster.plane(PlaneId(1)).unwrap().pilot,
        tore_world::seats::Pilot::Ai,
        "the plane went back to the AI"
    );
    // A flying player hears of the lobby at most once a second.
    assert!(rig.run_until(Duration::from_millis(1500), |r| {
        lobby(r, king).is_some_and(|l| l.players.len() == 1)
    }));
}

#[test]
fn a_player_joining_in_flight_takes_an_ai_aircraft() {
    let mut rig = kings_rig(2);
    let king = manual(&mut rig, "Viper");
    gathered(&mut rig, &[king]);
    rig.players[king].client.take_slot(0);
    rig.run(Duration::from_millis(200));
    rig.players[king].client.set_ready(true);
    rig.run(Duration::from_millis(200));
    rig.players[king].client.start_mission();
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(king)));
    rig.run(Duration::from_secs(2));
    let ai = rig
        .host
        .world()
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(1)
        .unwrap()
        .flight()
        .position;
    // A late joiner with no lobby screen takes the AI aircraft in flight.
    let cobra = rig.join(
        |c| c.callsign = "Cobra".into(),
        Box::new(|now, _, _| weave(now.as_secs_f64())),
    );
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(cobra)));
    assert_eq!(rig.players[cobra].client.seat().unwrap().1, PlaneId(1));
    let taken = rig.players[cobra]
        .client
        .prediction()
        .unwrap()
        .plane()
        .flight
        .position;
    let moved = (0..3)
        .map(|i| (taken[i] - ai[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    assert!(
        moved > 100.,
        "it took the aircraft where it flew ({moved} ft)"
    );
    assert!(rig.run_until(Duration::from_millis(1500), |r| {
        lobby(r, king).is_some_and(|l| l.players.len() == 2 && l.players.iter().all(|p| p.flying))
    }));
}

/// A dedicated server: no King, the mission from its file, started by its
/// `start` setting.
fn servers_rig(start: StartMode, configure: impl FnOnce(&mut crate::host::HostConfig)) -> Rig {
    Rig::with_config(
        spec(2, 2, 20),
        LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
        9,
        |config| {
            config.start = start;
            configure(config);
        },
    )
}

#[test]
fn a_dedicated_server_starts_with_the_first_ready_player_and_others_join_in_flight() {
    let mut rig = servers_rig(StartMode::FirstPlayer, |_| {});
    let viper = manual(&mut rig, "Viper");
    let cobra = manual(&mut rig, "Cobra");
    gathered(&mut rig, &[viper, cobra]);
    assert_eq!(lobby(&rig, viper).unwrap().king, None);
    assert_eq!(
        lobby(&rig, viper).unwrap().start,
        crate::wire::messages::StartRule::FirstReady
    );
    // A slot alone does not start it.
    rig.players[viper].client.take_slot(0);
    rig.players[cobra].client.take_slot(1);
    rig.run(Duration::from_millis(500));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    // There is no King to start it.
    rig.players[viper].client.start_mission();
    rig.run(Duration::from_millis(200));
    assert!(
        refusals(&rig, viper)
            .iter()
            .any(|(k, r)| *k == kind::START && r == "Only the King may do that.")
    );
    rig.players[viper].client.set_ready(true);
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(viper)));
    assert_eq!(rig.host.phase(), Phase::Flying);
    assert!(!rig.seated(cobra), "not ready: still in the lobby");
    rig.run(Duration::from_secs(1));
    rig.players[cobra].client.set_ready(true);
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(cobra)));
    assert_eq!(rig.players[cobra].client.seat().unwrap().1, PlaneId(1));
}

#[test]
fn a_dedicated_server_that_starts_now_flies_and_players_join_in_flight() {
    let mut rig = servers_rig(StartMode::Now, |_| {});
    rig.run(Duration::from_millis(500));
    assert_eq!(rig.host.phase(), Phase::Flying);
    let viper = rig.join(
        |c| c.callsign = "Viper".into(),
        Box::new(|now, _, _| weave(now.as_secs_f64())),
    );
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let l = lobby(&rig, viper).unwrap();
    assert_eq!(l.start, crate::wire::messages::StartRule::Flying);
    assert_eq!(l.phase, LobbyPhase::Flying);
}

#[test]
fn a_dedicated_server_keeps_its_players_through_the_restart_and_flies_again() {
    let mut rig = servers_rig(StartMode::FirstPlayer, |config| {
        config.time_limit = Some(Duration::from_secs(2));
        config.restart_delay = Duration::from_secs(1);
    });
    let viper = rig.join(
        |c| c.callsign = "Viper".into(),
        Box::new(|now, _, _| weave(now.as_secs_f64())),
    );
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    // The time limit ends it; the restart delay; then the lobby, where the
    // automatic ready flies the fresh mission.
    assert!(rig.run_until(Duration::from_secs(4), |r| {
        count(r, viper, |e| matches!(e, ClientEvent::Seated { .. })) == 2
    }));
    let p = &rig.players[viper];
    assert!(p.events.iter().any(|e| matches!(
        e,
        ClientEvent::MissionEnded(ended)
            if ended.reason == EndReason::TimeLimit && ended.next_in_seconds == Some(1)
    )));
    assert!(
        p.events
            .iter()
            .any(|e| matches!(e, ClientEvent::Debrief(_)))
    );
    assert!(!rig.closed(viper));
    // With `after-end quit` the server says the server is stopping.
    let mut rig = servers_rig(StartMode::FirstPlayer, |config| {
        config.time_limit = Some(Duration::from_secs(1));
        config.after_end = AfterEnd::Quit;
    });
    let viper = rig.join(|_| {}, Box::new(|now, _, _| weave(now.as_secs_f64())));
    assert!(rig.run_until(Duration::from_secs(5), |r| r.closed(viper)));
    assert!(rig.players[viper].events.iter().any(|e| matches!(
        e,
        ClientEvent::Closed(CloseReason::Disconnected {
            reason: DisconnectReason::ServerStopping,
            ..
        })
    )));
    assert_eq!(rig.host.phase(), Phase::Stopped);
}

/// EF3's finding: after its game stalls for 2 seconds, a client recovers
/// with at most one correction of its own plane.
#[test]
fn a_stalled_game_recovers_with_at_most_one_correction() {
    let mut rig = Rig::new(
        spec(2, 2, 20),
        LinkConfig::for_round_trip(20 * MS, 0., 0., 0.),
        5,
    );
    let player = rig.join(|_| {}, Box::new(|now, _, _| weave(now.as_secs_f64())));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(player)));
    rig.run(Duration::from_secs(3));
    let before = rig.players[player].client.corrections().len();
    rig.players[player].stalled = true;
    rig.run(Duration::from_secs(2));
    rig.players[player].stalled = false;
    rig.run(Duration::from_secs(4));
    let corrections = &rig.players[player].client.corrections()[before..];
    eprintln!("after the stall: {corrections:?}");
    assert!(corrections.len() <= 1, "{corrections:?}");
    let stats = rig.players[player].client.clone_stats();
    assert_eq!(rig.players[player].client.phase(), ClientPhase::Flying);
    assert!(
        stats.inputs_repeated >= 200,
        "the host repeated the stall's ticks"
    );
}

/// Only the hosting game's own connection over the in-process link is
/// exempt from the silence timeout: a King at a network address that goes
/// silent is dropped after 5 seconds like anyone, and the game ends.
#[test]
fn a_king_at_a_network_address_is_still_dropped_for_silence() {
    let mut rig = kings_rig(2);
    let king = manual(&mut rig, "Viper");
    let cobra = manual(&mut rig, "Cobra");
    gathered(&mut rig, &[king, cobra]);
    rig.players[king].stalled = true;
    assert!(rig.run_until(Duration::from_secs(8), |r| r.closed(cobra)));
    assert!(rig.logs.iter().any(|l| matches!(
        l,
        HostLog::Left {
            reason: crate::host::LeaveReason::Silent,
            ..
        }
    )));
    assert_eq!(
        rig.players[cobra].client.goodbye(),
        Some(&crate::wire::messages::Goodbye::HostLeft)
    );
}

/// The King, seated and flying, and a second player in the lobby; returns
/// (king, cobra).
fn king_flying_and_one_in_the_lobby(rig: &mut Rig) -> (usize, usize) {
    let king = manual(rig, "Viper");
    let cobra = manual(rig, "Cobra");
    gathered(rig, &[king, cobra]);
    rig.players[king].client.take_slot(0);
    rig.run(Duration::from_millis(300));
    rig.players[king].client.set_ready(true);
    rig.run(Duration::from_millis(300));
    rig.players[king].client.start_mission();
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(king)));
    rig.run(Duration::from_secs(1));
    (king, cobra)
}

/// Review item 1: a player flooding the host with lobby requests is held to
/// 20 a second, nothing that changes nothing is broadcast, and the flying
/// player's lobby states come at most once a second; nobody is dropped.
#[test]
fn a_flood_of_lobby_requests_is_held_and_never_crowds_the_flying() {
    let mut rig = kings_rig(3);
    let (king, cobra) = king_flying_and_one_in_the_lobby(&mut rig);
    let lobby_states = |rig: &Rig| count(rig, king, |e| matches!(e, ClientEvent::Lobby));
    let before = lobby_states(&rig);
    let logs_before = rig.logs.len();
    // Two seconds of a request every millisecond: take, leave, and stale.
    for i in 0..2000u32 {
        let client = &mut rig.players[cobra].client;
        match i % 3 {
            0 => client.take_slot(1),
            1 => client.leave_slot(),
            _ => client.set_ready(true),
        }
        rig.step();
    }
    rig.run(Duration::from_millis(500));
    let states = lobby_states(&rig) - before;
    assert!(
        states <= 3,
        "{states} lobby states to the flying King in 2.5 s"
    );
    let cobra_answers = count(&rig, cobra, |e| {
        matches!(e, ClientEvent::Refused { .. } | ClientEvent::Lobby)
    });
    assert!(cobra_answers <= 120, "{cobra_answers} answers in 2.5 s");
    let floods = rig.logs[logs_before..]
        .iter()
        .filter(|l| {
            matches!(
                l,
                HostLog::Lobby {
                    event: crate::host::LobbyEvent::TooManyRequests,
                    ..
                }
            )
        })
        .count();
    assert!((2..=3).contains(&floods), "{floods} flood notes");
    assert!(
        rig.logs.len() - logs_before <= 140,
        "{} log lines",
        rig.logs.len() - logs_before
    );
    assert!(!rig.closed(king) && !rig.closed(cobra));
    assert!(rig.seated(king));
}

/// Review item 7: a loadout sent while the mission flies is refused, not
/// silently kept for a plane that flies what it was built with.
#[test]
fn a_loadout_sent_in_flight_is_refused() {
    let mut rig = kings_rig(3);
    let (king, cobra) = king_flying_and_one_in_the_lobby(&mut rig);
    rig.players[cobra].client.take_slot(1);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        me(r, cobra).is_some_and(|m| m.slot == Some(1))
    }));
    rig.players[cobra].client.send_loadout(Some(standard()));
    rig.run(Duration::from_millis(300));
    assert!(
        refusals(&rig, cobra)
            .iter()
            .any(|(k, r)| *k == kind::LOADOUT
                && r == "Loadouts are chosen in the lobby, before the mission flies."),
        "{:?}",
        refusals(&rig, cobra)
    );
    assert!(rig.seated(king));
}

/// Review item 8: a lobby id is not given to the next player at once.
#[test]
fn lobby_ids_do_not_repeat_at_once() {
    let mut rig = kings_rig(3);
    let king = manual(&mut rig, "Viper");
    let cobra = manual(&mut rig, "Cobra");
    gathered(&mut rig, &[king, cobra]);
    let gone = me(&rig, cobra).unwrap().id;
    let now = rig.net.now();
    rig.players[cobra].client.disconnect(now);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        lobby(r, king).is_some_and(|l| l.players.len() == 1)
    }));
    let hawk = manual(&mut rig, "Hawk");
    assert!(rig.run_until(Duration::from_secs(2), |r| me(r, hawk).is_some()));
    assert_ne!(me(&rig, hawk).unwrap().id, gone);
}

/// Review item 5: the King's change to a mission a kept loadout does not
/// fit drops that loadout, and its player is told.
#[test]
fn a_mission_change_drops_a_loadout_the_new_mission_refuses() {
    let mut rig = kings_rig(3);
    let king = manual(&mut rig, "Viper");
    let cobra = manual(&mut rig, "Cobra");
    gathered(&mut rig, &[king, cobra]);
    rig.players[cobra].client.take_slot(1);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        me(r, cobra).is_some_and(|m| m.slot == Some(1))
    }));
    rig.players[cobra].client.send_loadout(Some(standard()));
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        me(r, cobra).is_some_and(|m| m.loadout)
    }));
    // Guns only now: the standard missiles do not fit.
    let mut guns = spec(3, 2, 20);
    guns.guns_only = true;
    rig.players[king].client.change_mission(&guns);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        lobby(r, cobra).is_some_and(|l| l.mission == 2) && me(r, cobra).is_some_and(|m| !m.loadout)
    }));
    assert_eq!(me(&rig, cobra).unwrap().slot, Some(1), "the slot stays");
    assert!(
        rig.players[cobra].events.iter().any(|e| matches!(
            e,
            ClientEvent::Notice(text) if text.contains("does not fit the new mission")
                && text.contains("Guns only")
        )),
        "{:?}",
        rig.players[cobra].events
    );
}

/// Review items 4 and 6: a loadout's other weapon is content-checked at the
/// flight's start; a player whose own copy differs is told and stays in the
/// lobby, and when the lobby returns it may fly the next mission.
#[test]
fn a_loadouts_other_weapon_is_checked_at_the_start_and_the_player_may_try_again() {
    let mut import = resources();
    import.insert(
        "AIM9X.JT".into(),
        tore_world::test_support::resources::loadable_missile("AIM9X.JT"),
    );
    let mut rig = Rig::with_import(
        spec(3, 2, 20),
        LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
        7,
        import.clone(),
        |config| {
            config.king = Some(Rig::player_address(0));
            config.start = StartMode::King;
            config.after_end = AfterEnd::Restart;
            config.restart_delay = Duration::ZERO;
            config.empty_timeout = Duration::ZERO;
        },
    );
    let king = manual(&mut rig, "Viper");
    // Cobra's copy of the other missile differs; the lobby's mission does
    // not read it.
    let mut other = import.clone();
    other.get_mut("AIM9X.JT").unwrap().push(0);
    rig.resources = Arc::new(other);
    let cobra = manual(&mut rig, "Cobra");
    rig.resources = Arc::new(import.clone());
    gathered(&mut rig, &[king, cobra]);
    assert!(me(&rig, cobra).unwrap().unable.is_none());
    let mut load = standard();
    let kind = tore_world::aircraft_type::AircraftType::load(&import, AircraftId::F18).unwrap();
    let base = tore_sim::combat::loadout::Loadout::new(&kind.profile, |name| {
        import
            .get(name)
            .cloned()
            .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
    })
    .unwrap();
    let missile = tore_formats::weapons::Weapon::parse("AIM9X.JT", &import["AIM9X.JT"]).unwrap();
    let capacity = base.capacity(1, &missile) as u16;
    load.stations[1].weapon = "AIM9X.JT".into();
    load.stations[1].count = capacity;
    load.stations[1].quantity = capacity;
    rig.players[king].client.take_slot(0);
    rig.players[cobra].client.take_slot(1);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        me(r, king).is_some_and(|m| m.slot.is_some())
    }));
    rig.players[king].client.send_loadout(Some(load));
    rig.players[king].client.set_ready(true);
    rig.players[cobra].client.set_ready(true);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        lobby(r, king).is_some_and(|l| l.all_ready())
    }));
    rig.players[king].client.start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.seated(king) && me(r, cobra).is_some_and(|m| m.unable.is_some())
    }));
    let p = &rig.players[cobra];
    assert!(
        p.events.iter().any(|e| matches!(
            e,
            ClientEvent::ContentRefused { names, .. } if names == &["AIM9X.JT".to_string()]
        )),
        "{:?}",
        p.events
    );
    assert!(!rig.seated(cobra) && !rig.closed(cobra));
    rig.run(Duration::from_secs(1));
    // The lobby returns: Cobra may take its slot and fly the next mission.
    rig.players[king].client.end_mission();
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        lobby(r, cobra).is_some_and(|l| l.phase == LobbyPhase::Lobby)
            && me(r, cobra).is_some_and(|m| m.unable.is_none())
    }));
    assert!(rig.players[cobra].client.unable().is_none());
    rig.players[cobra].client.take_slot(1);
    rig.players[king].client.send_loadout(None);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        me(r, cobra).is_some_and(|m| m.slot == Some(1))
    }));
    rig.players[king].client.set_ready(true);
    rig.players[cobra].client.set_ready(true);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        lobby(r, king).is_some_and(|l| l.all_ready())
    }));
    rig.players[king].client.start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(king)
        && r.seated(cobra)));
}

/// Review item 3: inputs that stay late for seconds (the path to the host
/// slows by 100 ms while the input clock steers) still cost a correction
/// at least every [`CORRECTION_HOLD_TICKS`] and a snapshot, never one held
/// to the end.
#[test]
fn sustained_late_inputs_are_corrected_within_the_hold() {
    let mut rig = Rig::new(
        spec(2, 2, 20),
        LinkConfig::for_round_trip(20 * MS, 0., 0., 0.),
        5,
    );
    let player = rig.join(|_| {}, Box::new(|now, _, _| weave(now.as_secs_f64())));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(player)));
    rig.run(Duration::from_secs(3));
    let before = rig.players[player].client.corrections().len();
    rig.net.set_link(
        Rig::player_address(0),
        super::tests::host_address(),
        LinkConfig::one_way(110 * MS),
    );
    let repeated = rig.players[player].client.clone_stats().inputs_repeated;
    rig.run(Duration::from_secs(4));
    let stats = rig.players[player].client.clone_stats();
    let late = stats.inputs_repeated - repeated;
    let corrections = &rig.players[player].client.corrections()[before..];
    let applied: Vec<u64> = corrections.iter().map(|c| c.now).collect();
    let longest = applied.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0);
    eprintln!(
        "{late} ticks repeated; {} corrections, applied at {applied:?}, the longest gap {longest} ticks, the largest {:.3} ft",
        corrections.len(),
        corrections.iter().map(|c| c.feet).fold(0., f64::max)
    );
    assert!(late > 60, "the inputs were late for a while: {late}");
    assert!(!corrections.is_empty());
    // While the host repeated them, each correction came within the hold
    // and a few snapshots of the last (before the review, every one waited
    // until the inputs were on time again).
    assert!(corrections.len() >= 10, "{applied:?}");
    assert!(longest <= CORRECTION_HOLD_TICKS + 4 * 4, "{applied:?}");
}

/// A client whose clock falls behind the host's newest snapshot (a starved
/// game: the host's own state is already past it) sends no inputs that
/// could arrive in time, so no margin would ever say it is late. It jumps
/// ahead of the host, and is the host's again at every snapshot; before the
/// fix it stayed behind, its plane taken back at every own state.
#[test]
fn a_client_behind_the_host_jumps_ahead_and_recovers() {
    let mut rig = Rig::new(
        spec(2, 2, 20),
        LinkConfig::for_round_trip(20 * MS, 0., 0., 0.),
        5,
    );
    let player = rig.join(|_| {}, Box::new(|now, _, _| weave(now.as_secs_f64())));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(player)));
    rig.run(Duration::from_secs(3));
    let before = rig.players[player].client.clone_stats();
    // Set the clock 10 ticks behind the host's newest snapshot.
    let client = &mut rig.players[player].client;
    let newest = f64::from(client.snapshot_tick.unwrap());
    client.input_clock.jump_to(newest - 10.);
    rig.run(Duration::from_secs(1));
    let after = rig.players[player].client.clone_stats();
    assert!(after.behind > before.behind, "it noticed: {after:?}");
    let settled = rig.players[player].client.corrections().len();
    let compared = after.hashes_compared;
    rig.run(Duration::from_secs(5));
    let end = rig.players[player].client.clone_stats();
    let corrections = &rig.players[player].client.corrections()[settled..];
    assert!(corrections.is_empty(), "{corrections:?}");
    assert!(
        end.hashes_compared >= compared + 140,
        "the prediction is compared with the host's at every snapshot again: {end:?}"
    );
    assert_eq!(end.mismatches, after.mismatches, "{end:?}");
    let margin = end.input_margin.unwrap();
    assert!((2..=8).contains(&margin), "input margin {margin}");
}
