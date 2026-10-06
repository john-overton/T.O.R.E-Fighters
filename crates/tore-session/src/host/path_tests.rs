//! Slice J6's host tests: the lobby's player list carries how each player
//! reached the host, as its Challenge answer said it (protocol 9), and the
//! host's log says it when the player joins. On the network simulator with
//! the scripted test clients.

use super::*;
use crate::wire::Path;
use tore_net::Target;

/// Joins a client that says it reached the host `path`, whatever the
/// simulated address: the answer carries the target's path.
fn join_by(rig: &mut Rig, callsign: &str, path: Path) -> usize {
    let at = rig.join(|c| c.callsign = callsign.into());
    let config = ClientConfig {
        game_version: build().version,
        game_commit: build().commit,
        entropy: Entropy::Seeded(u64::from(rig.next_port)),
        ..ClientConfig::new(PROTOCOL_VERSION, callsign)
    };
    rig.clients[at].client = Client::connect_any(
        config,
        &[Target::new(host_address(), path)],
        None,
        rig.net.now(),
    )
    .unwrap();
    rig.clients[at].ready = None;
    at
}

/// The path the lobby state `client` last heard gives `callsign`.
fn heard(rig: &Rig, client: usize, callsign: &str) -> Option<Path> {
    rig.clients[client]
        .lobby
        .as_ref()?
        .players
        .iter()
        .find(|p| p.callsign == callsign)
        .map(|p| p.path)
}

#[test]
fn every_players_lobby_entry_carries_the_path_it_connected_by() {
    let mut rig = Rig::new(spec(4, 1, 50), config(), LinkConfig::one_way(5 * MS));
    let paths = [
        ("Alpha", Path::LocalNetwork),
        ("Bravo", Path::Punched),
        ("Charlie", Path::Relay),
        ("Delta", Path::Ipv6),
    ];
    let players: Vec<usize> = paths
        .iter()
        .map(|(name, path)| join_by(&mut rig, name, *path))
        .collect();
    assert!(rig.run_until(Duration::from_secs(5), |r| {
        players.iter().all(|&n| {
            r.clients[n]
                .lobby
                .as_ref()
                .is_some_and(|l| l.players.len() == 4)
        })
    }));
    // Every player hears every other's path, and its own.
    for &viewer in &players {
        for (name, path) in paths {
            assert_eq!(heard(&rig, viewer, name), Some(path), "{name}");
        }
    }
    // The host's log says it as each joined.
    for (name, path) in paths {
        assert!(
            rig.logs.iter().any(|log| matches!(
                log,
                HostLog::Connected { callsign, path: logged, .. }
                    if callsign == name && *logged == path
            )),
            "{name} joined by {path:?}"
        );
    }
}
