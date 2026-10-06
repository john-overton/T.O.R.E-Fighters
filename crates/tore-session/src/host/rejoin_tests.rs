//! Slice K5's host tests (docs/ARCHITECTURE.md, "Rejoin tokens and
//! reservations"), on the network simulator with the scripted test clients:
//! a token for every player; a dropped player's plane kept for it and taken
//! back with the token, whatever the room; a leave, a Release and the
//! mission's end freeing it; a plane the AI loses while its player is away
//! bringing the revival rules; a token that expired, a kick voided or
//! another game issued refused in words; a restarted game replacing its
//! old connection; scores and lives following the token.

use super::*;
use crate::host::away::NOT_AWAY;
use crate::host::rejoin::{
    ALREADY_IN, LOST_AWAY, NOT_RESERVED, TOKEN_EXPIRED, TOKEN_UNKNOWN, TOKEN_VOIDED,
};
use crate::host::revive::NO_REVIVAL;
use crate::settings::{Respawn, number};
use crate::wire::messages::Kick;
use tore_net::Token;

/// Hawk (the King) in the lobby, Viper in plane 0 and Cobra in plane 1, with
/// plane 2 free, in co-op (no revival), the enemy far off.
fn trio_with(config: HostConfig) -> (Rig, usize, usize, usize) {
    let mut rig = Rig::new(spec(5, 1, 50), config, LinkConfig::one_way(5 * MS));
    let hawk = rig.join(|c| c.callsign = "Hawk".into());
    rig.clients[hawk].ready = None;
    let viper = rig.join(|c| c.callsign = "Viper".into());
    rig.clients[viper].ready = Some(Some(0));
    let cobra = rig.join(|c| c.callsign = "Cobra".into());
    rig.clients[cobra].ready = Some(Some(1));
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.seated(viper) && r.seated(cobra) && r.clients[hawk].lobby.is_some()
    }));
    rig.run(Duration::from_millis(300));
    // Hawk wears the crown, as the first to join a game with a King would.
    for peer in rig.host.peers.values_mut() {
        peer.king = peer.callsign == "Hawk";
    }
    (rig, hawk, viper, cobra)
}

fn trio() -> (Rig, usize, usize, usize) {
    trio_with(config())
}

fn peer<'a>(rig: &'a Rig, callsign: &str) -> &'a Peer {
    rig.host
        .peers
        .values()
        .find(|p| p.callsign == callsign)
        .unwrap_or_else(|| panic!("no {callsign}"))
}

fn pilot(rig: &Rig, plane: u32) -> Pilot {
    rig.host.world().roster.plane(PlaneId(plane)).unwrap().pilot
}

fn token_of(rig: &Rig, client: usize) -> Token {
    rig.clients[client].tokens.first().expect("a Token").token
}

/// The player's game goes silent: it leaves the rig, and the host drops it
/// after the transport's 5 seconds. Returns once the host has logged the
/// departure and run the tick that hands the plane to the AI.
fn drop_player(rig: &mut Rig, client: usize) -> TestClient {
    let silent = rig.clients.remove(client);
    let lefts = rig
        .logs
        .iter()
        .filter(|l| matches!(l, HostLog::Left { .. }))
        .count();
    assert!(rig.run_until(Duration::from_secs(8), |r| {
        r.logs
            .iter()
            .filter(|l| matches!(l, HostLog::Left { .. }))
            .count()
            > lefts
    }));
    rig.run(Duration::from_millis(100));
    silent
}

fn refused(rig: &Rig, client: usize, request: u8) -> Vec<String> {
    rig.clients[client]
        .refused
        .iter()
        .filter(|(k, _)| *k == request)
        .map(|(_, reason)| reason.clone())
        .collect()
}

/// Runs until `client` is seated in a flight later than `before`.
fn reseated(rig: &mut Rig, client: usize) -> bool {
    rig.run_until(Duration::from_secs(4), |r| r.seated(client))
}

fn no_errors(rig: &Rig) {
    for (n, client) in rig.clients.iter().enumerate() {
        assert!(client.errors.is_empty(), "client {n}: {:?}", client.errors);
    }
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn every_player_is_granted_a_token_that_lasts_24_hours() {
    let (rig, hawk, viper, cobra) = trio();
    let mut tokens = Vec::new();
    for client in [hawk, viper, cobra] {
        let grants = &rig.clients[client].tokens;
        assert_eq!(grants.len(), 1, "one token each");
        assert_eq!(grants[0].life_seconds, 86_400);
        tokens.push(grants[0].token);
    }
    tokens.sort();
    tokens.dedup();
    assert_eq!(tokens.len(), 3, "each its own");
    assert_eq!(
        rig.host.token_of_for_test("Viper"),
        Some(token_of(&rig, viper))
    );
    no_errors(&rig);
}

#[test]
fn a_dropped_player_rejoins_its_reserved_plane_which_nobody_else_could_take() {
    let (mut rig, hawk, viper, _) = trio();
    let token = token_of(&rig, viper);
    let order = peer(&rig, "Viper").lobby.order;
    let first_flight = rig.clients[viper].seated.as_ref().unwrap().flight;
    drop_player(&mut rig, viper);
    // Plane 0 is the AI's, kept for Viper.
    assert_eq!(pilot(&rig, 0), Pilot::Ai);
    assert_eq!(
        rig.host.rejoin.reserved.get(&order).map(|r| r.plane),
        Some(PlaneId(0))
    );
    assert!(!rig.host.rejoin.reserved[&order].away);
    assert!(
        !rig.host.anyone_away(),
        "a dropped player is not an away one"
    );
    rig.run(Duration::from_millis(300));
    let slot = rig.clients[hawk]
        .lobby
        .as_ref()
        .unwrap()
        .slots
        .iter()
        .find(|s| s.plane == 0)
        .unwrap();
    assert_eq!(slot.reserved.as_deref(), Some("Viper"));
    // Nobody else takes it meanwhile.
    rig.clients[hawk].take(Some(0));
    rig.run(Duration::from_millis(300));
    assert_eq!(
        rig.clients[hawk].seat_refused,
        ["Plane 0 is kept for Viper, who is away."]
    );
    assert_eq!(pilot(&rig, 0), Pilot::Ai);

    // Viper's game comes back with its token.
    let back = rig.join(|c| {
        c.callsign = "Viper".into();
        c.token = Some(token);
    });
    assert!(reseated(&mut rig, back));
    let seated = rig.clients[back].seated.clone().unwrap();
    assert_eq!(seated.plane, 0);
    assert!(
        seated.flight >= 1 && first_flight >= 1,
        "a flight of its own"
    );
    assert_eq!(
        rig.clients[back].notices,
        ["Welcome back, Viper: your aircraft is waiting."]
    );
    assert_eq!(rig.clients[back].tokens.len(), 1);
    assert_eq!(rig.clients[back].tokens[0].token, token);
    // The same player: its callsign, its join order, its token.
    let me = peer(&rig, "Viper");
    assert_eq!(me.lobby.order, order);
    assert!(matches!(pilot(&rig, 0), Pilot::Human(_)));
    assert!(rig.host.rejoin.reserved.is_empty());
    assert!(logged_rejoin(&rig, Some(0)));
    rig.run(Duration::from_millis(300));
    no_errors(&rig);
}

fn logged_rejoin(rig: &Rig, plane: Option<u32>) -> bool {
    rig.logs.iter().any(|log| {
        matches!(log, HostLog::Lobby { callsign, event: LobbyEvent::Rejoined { plane: p }, .. }
            if callsign == "Viper" && *p == plane)
    })
}

#[test]
fn leaving_the_flight_or_the_game_on_purpose_keeps_nothing() {
    let (mut rig, hawk, viper, cobra) = trio();
    // Viper ends its flight and the game: the plane is free at once.
    rig.clients[viper].leave();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.closed(viper)));
    rig.run(Duration::from_millis(200));
    assert!(rig.host.rejoin.reserved.is_empty());
    rig.clients[hawk].take(Some(0));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(hawk)));
    assert_eq!(rig.clients[hawk].seated.as_ref().unwrap().plane, 0);
    // Its token still works for 24 hours: leaving does not void it.
    let now = rig.host.token_clock();
    assert!(
        rig.host.rejoin.keeps(
            rig.host
                .rejoin
                .players
                .iter()
                .find(|(_, r)| r.callsign == "Viper")
                .map(|(o, _)| *o)
                .unwrap(),
            now
        )
    );
    // Cobra's plane, kept for it after a drop, is cleared by the end of the
    // mission; its token and its lobby place stay.
    let cobra_token = token_of(&rig, cobra);
    drop_player(&mut rig, cobra);
    assert_eq!(rig.host.rejoin.reserved.len(), 1);
    rig.host.restart();
    rig.run(Duration::from_millis(300));
    assert!(rig.host.rejoin.reserved.is_empty());
    let back = rig.join(|c| {
        c.callsign = "Cobra".into();
        c.token = Some(cobra_token);
    });
    rig.clients[back].ready = None;
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.clients[back]
            .lobby
            .as_ref()
            .is_some_and(|l| l.me().is_some())
    }));
    rig.run(Duration::from_millis(500));
    let lobby = rig.clients[back].lobby.as_ref().unwrap();
    let me = lobby.me().unwrap();
    assert_eq!(me.callsign, "Cobra");
    assert_eq!(me.slot, Some(1), "the lobby kept its slot: {me:?}");
    assert_eq!(rig.clients[back].notices, ["Welcome back, Cobra."]);
    no_errors(&rig);
}

#[test]
fn the_kings_release_frees_a_reservation_and_the_player_finds_no_plane() {
    let (mut rig, hawk, viper, _) = trio();
    let token = token_of(&rig, viper);
    drop_player(&mut rig, viper);
    // Release of a plane nothing is kept on is refused; only the King may.
    rig.clients[hawk].send(&Message::Release(2));
    rig.run(Duration::from_millis(200));
    assert_eq!(refused(&rig, hawk, kind::RELEASE), [NOT_RESERVED]);
    rig.clients[hawk].send(&Message::Release(0));
    rig.run(Duration::from_millis(200));
    assert!(rig.host.rejoin.reserved.is_empty());
    assert!(
        rig.logs.iter().any(|l| matches!(l, HostLog::Lobby { callsign, event: LobbyEvent::Released { plane: 0 }, .. } if callsign == "Viper"))
    );
    // Free for anyone.
    rig.clients[hawk].take(Some(0));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(hawk)));
    assert_eq!(rig.clients[hawk].seated.as_ref().unwrap().plane, 0);
    // Viper comes back: welcome, but no aircraft waits; it takes a free one.
    let back = rig.join(|c| {
        c.callsign = "Viper".into();
        c.token = Some(token);
    });
    assert!(reseated(&mut rig, back));
    assert_eq!(rig.clients[back].notices, ["Welcome back, Viper."]);
    assert_eq!(rig.clients[back].seated.as_ref().unwrap().plane, 2);
    no_errors(&rig);
}

fn lose_ai_plane(rig: &mut Rig, plane: u32) {
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

#[test]
fn a_plane_lost_while_away_gives_the_watch_under_no_revival() {
    let (mut rig, _, viper, _) = trio();
    let token = token_of(&rig, viper);
    let order = peer(&rig, "Viper").lobby.order;
    drop_player(&mut rig, viper);
    lose_ai_plane(&mut rig, 0);
    rig.run(Duration::from_millis(300));
    // The reservation ended, and the loss counts for the player.
    assert!(rig.host.rejoin.reserved.is_empty());
    assert!(rig.host.revival.players[&order].lost.is_some());
    let back = rig.join(|c| {
        c.callsign = "Viper".into();
        c.token = Some(token);
    });
    rig.run(Duration::from_secs(2));
    let client = &rig.clients[back];
    assert!(
        client.notices.contains(&LOST_AWAY.to_owned()),
        "{:?}",
        client.notices
    );
    let revival = client.revivals.last().expect("a Revival");
    assert_eq!(revival.rule, Respawn::None);
    assert_eq!(revival.why.as_deref(), Some(NO_REVIVAL));
    // It watches until the round ends: no plane for it.
    assert!(client.seated.is_none());
    assert_eq!(
        client.seat_refused.last().map(String::as_str),
        Some(NO_REVIVAL)
    );
    no_errors(&rig);
}

#[test]
fn a_plane_lost_while_away_counts_lives_and_the_delay_on_return() {
    for rule in [Respawn::Revive, Respawn::AiSlot] {
        let (mut rig, _, viper, _) = trio();
        rig.host
            .settings
            .apply(&[
                (number::RESPAWN, rule.value()),
                (number::LIVES, 2),
                (number::REVIVE_DELAY, 60),
            ])
            .unwrap();
        let token = token_of(&rig, viper);
        let order = peer(&rig, "Viper").lobby.order;
        drop_player(&mut rig, viper);
        lose_ai_plane(&mut rig, 0);
        rig.run(Duration::from_millis(300));
        let back = rig.join(|c| {
            c.callsign = "Viper".into();
            c.token = Some(token);
        });
        // Back at once: the delay (a minute from the loss) is still running.
        rig.run(Duration::from_millis(900));
        let revival = rig.clients[back]
            .revivals
            .last()
            .expect("a Revival")
            .clone();
        assert_eq!((revival.rule, revival.lives), (rule, Some(2)), "{rule:?}");
        assert!(revival.wait_seconds > 0, "{revival:?}");
        let early = rig.clients[back]
            .seat_refused
            .iter()
            .any(|why| why.starts_with("You can fly again in"));
        assert!(early, "{:?}", rig.clients[back].seat_refused);
        assert!(rig.clients[back].seated.is_none());
        // After the delay it flies again, and a life is spent.
        rig.run(Duration::from_secs(60));
        rig.clients[back].take(None);
        assert!(reseated(&mut rig, back), "{rule:?}");
        let seated = rig.clients[back].seated.clone().unwrap();
        assert_ne!(seated.plane, 0, "{rule:?}");
        assert_eq!(rig.host.revival.players[&order].used, 1, "{rule:?}");
        assert_eq!(peer(&rig, "Viper").lobby.order, order);
        rig.run(Duration::from_millis(300));
        no_errors(&rig);
    }
}

#[test]
fn a_plane_the_ai_loses_while_its_player_is_still_here_but_away_counts_too() {
    let (mut rig, _, viper, _) = trio();
    rig.host
        .settings
        .apply(&[
            (number::RESPAWN, Respawn::Revive.value()),
            (number::LIVES, 1),
            (number::REVIVE_DELAY, 0),
        ])
        .unwrap();
    let order = peer(&rig, "Viper").lobby.order;
    rig.clients[viper].send(&Message::Away);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.host.rejoin.reserved.get(&order).is_some_and(|r| r.away)
    }));
    lose_ai_plane(&mut rig, 0);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        !r.clients[viper].revivals.is_empty()
    }));
    assert_eq!(rig.clients[viper].revivals[0].lives, Some(1));
    // Join flies it again, by the rules, and spends the life.
    rig.clients[viper].take(None);
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.clients[viper]
            .seated
            .as_ref()
            .is_some_and(|s| s.plane != 0)
    }));
    assert_eq!(rig.host.revival.players[&order].used, 1);
    no_errors(&rig);
}

/// The away player's menu (slice F2-O4): Spawn in Aircraft after the AI lost
/// the reserved aircraft is the revival rules' Revive, as it was for a
/// player at the controls; where the rules allow nothing it is refused in
/// the rules' words, which the menu shows beside the dimmed row.
#[test]
fn spawn_in_aircraft_after_a_loss_while_away_is_the_revival_rules_revive() {
    let (mut rig, _, viper, _) = trio();
    rig.host
        .settings
        .apply(&[
            (number::RESPAWN, Respawn::Revive.value()),
            (number::LIVES, 1),
            (number::REVIVE_DELAY, 0),
        ])
        .unwrap();
    let order = peer(&rig, "Viper").lobby.order;
    rig.clients[viper].send(&Message::Away);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.host.rejoin.reserved.get(&order).is_some_and(|r| r.away)
    }));
    lose_ai_plane(&mut rig, 0);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        !r.clients[viper].revivals.is_empty()
    }));
    // No reserved aircraft is left: Take Back Flight would be refused.
    rig.clients[viper].send(&Message::Back);
    rig.run(Duration::from_millis(300));
    assert_eq!(
        refused(&rig, viper, kind::BACK),
        ["The AI is not flying your aircraft."]
    );
    let mission = rig.clients[viper].number();
    rig.clients[viper].send(&Message::Revive { mission });
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.clients[viper]
            .seated
            .as_ref()
            .is_some_and(|s| s.plane != 0)
    }));
    assert_eq!(rig.host.revival.players[&order].used, 1);
    no_errors(&rig);
}

#[test]
fn spawn_in_aircraft_under_no_revival_is_refused_in_the_rules_words() {
    let (mut rig, _, viper, _) = trio();
    let order = peer(&rig, "Viper").lobby.order;
    rig.clients[viper].send(&Message::Away);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.host.rejoin.reserved.get(&order).is_some_and(|r| r.away)
    }));
    lose_ai_plane(&mut rig, 0);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        !r.clients[viper].revivals.is_empty()
    }));
    assert_eq!(rig.clients[viper].revivals[0].rule, Respawn::None);
    let mission = rig.clients[viper].number();
    rig.clients[viper].send(&Message::Revive { mission });
    rig.run(Duration::from_millis(500));
    assert_eq!(peer(&rig, "Viper").stage, Stage::Lobby);
    assert!(
        rig.clients[viper]
            .seat_refused
            .iter()
            .chain(rig.clients[viper].refused.iter().map(|(_, why)| why))
            .any(|why| why == NO_REVIVAL),
        "{:?} {:?}",
        rig.clients[viper].seat_refused,
        rig.clients[viper].refused
    );
}

/// The King released an away player's aircraft: no revival is noted, so the
/// menu's Spawn in Aircraft takes any free aircraft by the usual take rules.
#[test]
fn spawn_in_aircraft_after_a_release_takes_a_free_aircraft() {
    let (mut rig, hawk, viper, _) = trio();
    let order = peer(&rig, "Viper").lobby.order;
    rig.clients[viper].send(&Message::Away);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.host.rejoin.reserved.get(&order).is_some_and(|r| r.away)
    }));
    rig.clients[hawk].send(&Message::Release(0));
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.host.rejoin.reserved.is_empty()
    }));
    assert!(
        rig.clients[viper].revivals.is_empty(),
        "a release is no loss"
    );
    let before = rig.clients[viper].seated.as_ref().map(|s| s.flight);
    rig.clients[viper].take(None);
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.clients[viper].seated.as_ref().map(|s| s.flight) != before
    }));
    no_errors(&rig);
}

#[test]
fn a_token_that_expired_another_game_issued_or_a_kick_voided_is_refused_in_words() {
    let (mut rig, hawk, viper, cobra) = trio();
    let viper_token = token_of(&rig, viper);
    let cobra_token = token_of(&rig, cobra);
    let cobra_id = peer(&rig, "Cobra").lobby.id;
    // A kick voids.
    rig.host.kick_player(cobra_id, "go").unwrap();
    assert!(rig.run_until(Duration::from_secs(2), |r| r.closed(cobra)));
    rig.run(Duration::from_millis(200));
    let _ = hawk;
    drop_player(&mut rig, viper);

    // Another game's token: unknown here.
    let stranger = rig.join(|c| {
        c.callsign = "Stranger".into();
        c.token = Some(Token(0x1357_9bdf_0246_8ace_1357_9bdf_0246_8ace));
    });
    rig.clients[stranger].ready = None;
    // The kicked player's: voided.
    let kicked = rig.join(|c| {
        c.callsign = "Cobra".into();
        c.token = Some(cobra_token);
    });
    rig.clients[kicked].ready = None;
    rig.run(Duration::from_secs(1));
    assert_eq!(
        rig.clients[stranger].notices,
        [format!("{TOKEN_UNKNOWN} You join as a new player.")]
    );
    assert_eq!(
        rig.clients[kicked].notices,
        [format!("{TOKEN_VOIDED} You join as a new player.")]
    );
    // Both joined as new players with tokens of their own.
    assert_eq!(rig.clients[stranger].tokens.len(), 1);
    assert_ne!(rig.clients[kicked].tokens[0].token, cobra_token);
    assert_eq!(
        rig.host.rejoin.reserved.len(),
        1,
        "only Viper's plane is kept"
    );

    // 24 hours after Viper was last connected: expired.
    rig.host
        .advance_token_clock_for_test(Duration::from_secs(86_400 + 1));
    let late = rig.join(|c| {
        c.callsign = "Viper".into();
        c.token = Some(viper_token);
    });
    rig.clients[late].ready = None;
    rig.run(Duration::from_secs(1));
    assert_eq!(
        rig.clients[late].notices,
        [format!("{TOKEN_EXPIRED} You join as a new player.")]
    );
    assert!(rig.clients[late].seated.is_none());
    no_errors(&rig);
}

#[test]
fn a_token_works_until_24_hours_after_its_player_left() {
    let (mut rig, _, viper, _) = trio();
    let token = token_of(&rig, viper);
    drop_player(&mut rig, viper);
    rig.host
        .advance_token_clock_for_test(Duration::from_secs(86_400 - 60));
    let back = rig.join(|c| {
        c.callsign = "Viper".into();
        c.token = Some(token);
    });
    assert!(reseated(&mut rig, back));
    assert_eq!(rig.clients[back].seated.as_ref().unwrap().plane, 0);
    no_errors(&rig);
}

#[test]
fn a_full_game_admits_the_token_holder_and_no_one_else() {
    let (mut rig, _, viper, _) = trio();
    rig.host
        .settings
        .apply(&[(number::MAX_PLAYERS, 3)])
        .unwrap();
    let token = token_of(&rig, viper);
    drop_player(&mut rig, viper);
    // Hawk and Cobra are there; a newcomer fills the third place.
    let newcomer = rig.join(|c| {
        c.callsign = "Newcomer".into();
    });
    rig.clients[newcomer].ready = None;
    rig.run(Duration::from_secs(1));
    assert!(rig.clients[newcomer].lobby.is_some());
    // The game is full: another newcomer is refused, Viper's token is not.
    let late = rig.join(|c| {
        c.callsign = "Late".into();
    });
    rig.clients[late].ready = None;
    rig.run(Duration::from_secs(1));
    assert!(rig.closed(late), "the full game refuses a stranger");
    let back = rig.join(|c| {
        c.callsign = "Viper".into();
        c.token = Some(token);
    });
    assert!(reseated(&mut rig, back));
    assert_eq!(rig.clients[back].seated.as_ref().unwrap().plane, 0);
}

#[test]
fn a_game_restarted_before_the_host_noticed_replaces_its_old_connection() {
    let (mut rig, _, viper, _) = trio();
    let token = token_of(&rig, viper);
    let order = peer(&rig, "Viper").lobby.order;
    let back = rig.join(|c| {
        c.callsign = "Viper".into();
        c.token = Some(token);
    });
    assert!(rig.run_until(Duration::from_secs(4), |r| {
        r.closed(viper) && r.seated(back)
    }));
    assert_eq!(rig.clients[back].seated.as_ref().unwrap().plane, 0);
    assert_eq!(peer(&rig, "Viper").lobby.order, order);
    assert_eq!(
        rig.host
            .peers
            .values()
            .filter(|p| p.callsign == "Viper")
            .count(),
        1
    );
    assert!(rig.host.rejoin.reserved.is_empty());
    no_errors_except_closed(&rig, viper);
}

fn no_errors_except_closed(rig: &Rig, old: usize) {
    for (n, client) in rig.clients.iter().enumerate() {
        if n != old {
            assert!(client.errors.is_empty(), "client {n}: {:?}", client.errors);
        }
    }
    assert!(rig.faults().is_empty());
}

#[test]
fn a_game_that_joined_without_its_token_sends_a_rejoin_and_is_the_player_again() {
    let (mut rig, _, viper, cobra) = trio();
    let token = token_of(&rig, viper);
    let order = peer(&rig, "Viper").lobby.order;
    drop_player(&mut rig, viper);
    let cobra = cobra - 1;
    // Joined by a typed address: no token in the Challenge answer.
    let back = rig.join(|c| {
        c.callsign = "Viper".into();
    });
    rig.clients[back].ready = None;
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.clients[back].lobby.is_some()
    }));
    // An unknown token, and one from a player who flies, are refused.
    rig.clients[back].send(&Message::Rejoin(Token(7)));
    rig.clients[cobra].send(&Message::Rejoin(token));
    rig.run(Duration::from_millis(300));
    assert_eq!(refused(&rig, back, kind::REJOIN), [TOKEN_UNKNOWN]);
    assert_eq!(refused(&rig, cobra, kind::REJOIN), [ALREADY_IN]);
    // Its own: it is the player again, and Join takes the plane back.
    rig.clients[back].send(&Message::Rejoin(token));
    rig.run(Duration::from_millis(300));
    assert!(
        rig.clients[back]
            .notices
            .contains(&"Welcome back, Viper: your aircraft is waiting.".to_owned())
    );
    assert_eq!(peer(&rig, "Viper").lobby.order, order);
    rig.clients[back].take(None);
    assert!(reseated(&mut rig, back));
    assert_eq!(rig.clients[back].seated.as_ref().unwrap().plane, 0);
    // A Back from a player who was never away is told so.
    rig.clients[back].send(&Message::Back);
    rig.run(Duration::from_millis(200));
    assert_eq!(refused(&rig, back, kind::BACK), [NOT_AWAY]);
    no_errors(&rig);
}

/// A game that joined without its token (a typed address) and sends it in a
/// Rejoin while the host still lists its old connection: the new connection
/// replaces the old, same player and same plane (slice B8; the Challenge
/// answer's case is the test above).
#[test]
fn a_late_rejoin_replaces_the_old_connection_the_host_still_lists() {
    let (mut rig, _, viper, _) = trio();
    let token = token_of(&rig, viper);
    let order = peer(&rig, "Viper").lobby.order;
    let back = rig.join(|c| {
        c.callsign = "Viper".into();
    });
    rig.clients[back].ready = None;
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.clients[back].lobby.is_some()
    }));
    // The old connection has said nothing and is still listed.
    assert!(!rig.closed(viper));
    rig.clients[back].send(&Message::Rejoin(token));
    assert!(rig.run_until(Duration::from_secs(4), |r| r.closed(viper)));
    assert!(
        rig.clients[back]
            .notices
            .iter()
            .any(|n| n.starts_with("Welcome back, Viper")),
        "{:?}",
        rig.clients[back].notices
    );
    assert_eq!(peer(&rig, "Viper").lobby.order, order);
    assert_eq!(
        rig.host
            .peers
            .values()
            .filter(|p| p.callsign == "Viper")
            .count(),
        1
    );
    // The old connection's plane is the player's: Join takes it again.
    rig.clients[back].take(None);
    assert!(reseated(&mut rig, back));
    assert_eq!(rig.clients[back].seated.as_ref().unwrap().plane, 0);
    assert!(refused(&rig, back, kind::REJOIN).is_empty());
    no_errors_except_closed(&rig, viper);
}

#[test]
fn scores_and_lives_follow_the_token_and_a_kick_clears_them() {
    let (mut rig, _, viper, _) = trio();
    let token = token_of(&rig, viper);
    let order = peer(&rig, "Viper").lobby.order;
    rig.host.revival.players.entry(order).or_default().used = 1;
    assert!(
        rig.host.score.player(order).is_some(),
        "scored while it flies"
    );
    drop_player(&mut rig, viper);
    rig.run(Duration::from_millis(500));
    assert!(
        rig.host.score.player(order).is_some(),
        "its tally stays while its token works"
    );
    let back = rig.join(|c| {
        c.callsign = "Viper".into();
        c.token = Some(token);
    });
    assert!(reseated(&mut rig, back));
    assert_eq!(peer(&rig, "Viper").lobby.order, order);
    assert_eq!(rig.host.revival.players[&order].used, 1);
    assert!(rig.host.score.player(order).is_some());
    // A kick voids the token and the tally goes with the player.
    let id = peer(&rig, "Viper").lobby.id;
    rig.host.kick_player(id, "").unwrap();
    assert!(rig.run_until(Duration::from_secs(2), |r| r.closed(back)));
    rig.run(Duration::from_millis(500));
    assert!(rig.host.score.player(order).is_none());
    assert!(rig.host.rejoin.reserved.is_empty(), "a kick keeps nothing");
}

#[test]
fn only_the_king_releases_and_a_kick_message_is_not_a_drop() {
    let (mut rig, hawk, viper, cobra) = trio();
    rig.clients[cobra].send(&Message::Release(0));
    rig.run(Duration::from_millis(200));
    assert_eq!(
        refused(&rig, cobra, kind::RELEASE),
        ["Only the King may do that."]
    );
    let id = peer(&rig, "Viper").lobby.id;
    rig.clients[hawk].send(&Message::Kick(Kick {
        player: id,
        reason: "no".into(),
    }));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.closed(viper)));
    rig.run(Duration::from_millis(300));
    assert!(rig.host.rejoin.reserved.is_empty());
    assert_eq!(pilot(&rig, 0), Pilot::Ai);
}
