//! Other aircraft's stores online (protocol 27, B3): a client draws every
//! aircraft's pylons and stores from its own copy of the loadouts, as built
//! until the host tells it otherwise, and a Stores event empties a station
//! as the host's aircraft lets its stores go, for a late joiner too.
//! Synthetic data only.

use super::tests::{Rig, level_script, spec};
use super::*;
use tore_net::sim::LinkConfig;

fn link() -> LinkConfig {
    LinkConfig::for_round_trip(Duration::from_millis(40), 0.1, 0., 0.)
}

/// The stores player `player` draws on aircraft `id` now.
fn drawn(rig: &mut Rig, player: usize, id: u32) -> Vec<tore_world::snapshot::StorePose> {
    let now = rig.net.now();
    let frame = rig.players[player].client.frame(now).expect("flying");
    std::iter::once(&frame.picture.player)
        .chain(&frame.picture.targets)
        .find(|pose| pose.id == id)
        .map(|pose| pose.stores.clone())
        .expect("the aircraft is drawn")
}

#[test]
fn a_client_draws_another_players_and_the_ais_stores_as_the_host_has_them() {
    let mut rig = Rig::new(spec(3, 2, 20), link(), 31);
    rig.host.start_now();
    let first = rig.join(|_| {}, level_script());
    let second = rig.join(|_| {}, level_script());
    assert!(rig.run_until(Duration::from_secs(4), |r| r.seated(first)
        && r.seated(second)));
    rig.run(Duration::from_secs(1));
    let now = rig.net.now();
    let plane = rig.players[second]
        .client
        .frame(now)
        .expect("flying")
        .plane
        .0;
    // The other player's aircraft and an AI aircraft, as built: their
    // pylons and every store, from the first player's copy of the mission.
    let host = rig.host.world();
    let built = |id: u32| {
        let config = host.aircraft_configuration(id).expect("a loadout");
        tore_world::snapshot::store_poses_of(config, &tore_world::snapshot::as_built_stores(config))
    };
    let other = built(plane);
    assert!(
        !other.is_empty(),
        "the synthetic F/A-18D draws its AIM-9 station"
    );
    let ai = host
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actors()
        .iter()
        .map(|a| a.id())
        .find(|id| !built(*id).is_empty())
        .expect("an AI aircraft with stores");
    let ai_built = built(ai);
    let station = host.station_stores()[&plane][0];
    assert_eq!(drawn(&mut rig, first, plane), other);
    assert_eq!(drawn(&mut rig, first, ai), ai_built);

    // The second player lets one AIM-9 go on the host: the first player's
    // picture empties that station once its drawn tick reaches the change.
    let own = rig
        .host
        .world_for_test()
        .combat
        .state
        .ownship_mut(plane)
        .expect("the second player's aircraft");
    own.ammo[usize::from(station.station)] -= 1;
    rig.run(Duration::from_secs(1));
    let after = drawn(&mut rig, first, plane);
    assert_eq!(after[0].items, other[0].items - 1);
    assert_eq!(after[1..], other[1..]);

    // A player who joins later is told it at its seat.
    let late = rig.join(|_| {}, level_script());
    assert!(rig.run_until(Duration::from_secs(4), |r| r.seated(late)));
    rig.run(Duration::from_secs(1));
    assert_eq!(drawn(&mut rig, late, plane), after);
    assert_eq!(drawn(&mut rig, late, ai), ai_built);
}
