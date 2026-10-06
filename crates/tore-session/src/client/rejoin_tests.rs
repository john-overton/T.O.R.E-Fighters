//! The client's side of rejoin (slice K5) against a real host on the
//! network simulator: the token the host grants is kept with the session's
//! id and handed to the game's store, again when the connection ends; a game
//! that joins with the token flies its reserved plane again; and one that
//! joined without it sends a Rejoin.

use super::rejoin::{KeptToken, TokenStore};
use super::tests::{Rig, level_script, spec};
use super::*;
use std::sync::{Arc, Mutex};
use tore_net::sim::LinkConfig;

const MS: Duration = Duration::from_millis(1);

/// A store that keeps what it is given where the test can read it.
#[derive(Clone, Default)]
struct Shelf(Arc<Mutex<Vec<KeptToken>>>);

impl TokenStore for Shelf {
    fn keep(&mut self, kept: &KeptToken) {
        self.0.lock().unwrap().push(kept.clone());
    }
}

fn rig() -> Rig {
    Rig::new(spec(2, 2, 50), LinkConfig::one_way(10 * MS), 21)
}

fn flier(rig: &mut Rig, callsign: &str, token: Option<tore_net::Token>) -> usize {
    let callsign = callsign.to_owned();
    rig.join(
        move |c| {
            c.callsign = callsign;
            c.token = token;
        },
        level_script(),
    )
}

#[test]
fn the_granted_token_is_kept_with_the_session_and_handed_to_the_store() {
    let mut rig = rig();
    let shelf = Shelf::default();
    let viper = flier(&mut rig, "Viper", None);
    rig.players[viper]
        .client
        .set_token_store(Box::new(shelf.clone()));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let kept = rig.players[viper].client.token().cloned().expect("a token");
    assert_eq!(kept.session_id, rig.host.session_id_for_test());
    assert_eq!(
        (kept.callsign.as_str(), kept.life_seconds),
        ("Viper", 86_400)
    );
    assert_eq!(
        Some(kept.token),
        rig.host.token_of_for_test("Viper"),
        "the host's own record"
    );
    assert_eq!(
        shelf.0.lock().unwrap().as_slice(),
        std::slice::from_ref(&kept)
    );
    // The connection ends: the token's 24 hours start from now.
    let now = rig.net.now();
    rig.players[viper].client.leave_game(now);
    assert!(rig.run_until(Duration::from_secs(4), |r| r.closed(viper)));
    let kept_list = shelf.0.lock().unwrap().clone();
    assert!(kept_list.len() >= 2, "kept again at the close");
    assert_eq!(kept_list.last().unwrap().token, kept.token);
}

#[test]
fn a_game_that_joins_with_its_token_flies_its_reserved_plane_again() {
    let mut rig = rig();
    let viper = flier(&mut rig, "Viper", None);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let plane = rig.players[viper].client.seat().map(|(_, p)| p.0).unwrap();
    let token = rig.players[viper].client.token().unwrap().token;
    // Its game stalls for good: the host drops it after 5 seconds.
    rig.players[viper].stalled = true;
    assert!(rig.run_until(Duration::from_secs(8), |r| { r.host.peers_for_test() == 0 }));
    rig.run(Duration::from_millis(200));
    let back = flier(&mut rig, "Viper", Some(token));
    assert!(rig.run_until(Duration::from_secs(4), |r| r.seated(back)));
    assert_eq!(
        rig.players[back].client.seat().map(|(_, p)| p.0),
        Some(plane)
    );
    assert_eq!(rig.players[back].client.token().unwrap().token, token);
    assert!(rig.players[back].events.iter().any(
        |e| matches!(e, ClientEvent::Notice(text) if text.starts_with("Welcome back, Viper"))
    ));
}

#[test]
fn a_game_that_joined_without_its_token_sends_a_rejoin() {
    let mut rig = rig();
    let viper = flier(&mut rig, "Viper", None);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let plane = rig.players[viper].client.seat().map(|(_, p)| p.0).unwrap();
    let token = rig.players[viper].client.token().unwrap().token;
    rig.players[viper].stalled = true;
    assert!(rig.run_until(Duration::from_secs(8), |r| { r.host.peers_for_test() == 0 }));
    rig.run(Duration::from_millis(200));
    let back = rig.join(
        |c| {
            c.callsign = "Viper".into();
            c.auto_ready = false;
        },
        level_script(),
    );
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[back].client.phase() == ClientPhase::Lobby
    }));
    rig.players[back].client.rejoin(token);
    rig.run(Duration::from_millis(400));
    rig.players[back].client.ready(None);
    assert!(rig.run_until(Duration::from_secs(4), |r| r.seated(back)));
    assert_eq!(
        rig.players[back].client.seat().map(|(_, p)| p.0),
        Some(plane)
    );
}
