//! The city fill online (protocol 25, review net finding 1): the buildings a
//! host's ground burst knocks flat fall in every player's copy of the
//! mission, a late joiner's too, so the prediction flies past them as the
//! host does. Synthetic data with a test fill on every machine.

use super::tests::{Rig, level_script, spec};
use super::*;
use std::sync::Arc;
use tore_net::sim::LinkConfig;

/// Three towers: two side by side, one far off.
const TOWERS: [[i32; 2]; 3] = [[10_000, 20_000], [10_400, 20_000], [30_000, 30_000]];

fn fill() -> Arc<tore_world::city::CityFill> {
    Arc::new(tore_world::test_support::city_fill(&TOWERS, 0.))
}

/// Builds each game's mission as the session does, with the test fill.
fn with_fill(client: &mut Client) {
    client.set_mission_builder(Box::new(|spec, resources| {
        let mut world = World::new(spec, resources, Seating::Open)?;
        world.terrain.city_fill = Some(fill());
        Ok(world)
    }));
}

/// A segment through the first tower below its roof.
const THROUGH: ([f64; 3], [f64; 3]) = ([9_900., 100., 20_000.], [10_100., 100., 20_000.]);

fn flattened(world: &World) -> Vec<u32> {
    world
        .terrain
        .city_fill
        .as_ref()
        .expect("a fill")
        .flattened()
}

#[test]
fn buildings_a_host_knocks_flat_fall_on_every_client_and_a_late_joiner() {
    let link = LinkConfig::for_round_trip(Duration::from_millis(40), 0.1, 0., 0.);
    let mut rig = Rig::new(spec(2, 1, 20), link, 31);
    rig.host.start_now();
    rig.host.world_for_test().terrain.city_fill = Some(fill());
    let player = rig.join(|_| {}, level_script());
    with_fill(&mut rig.players[player].client);
    assert!(rig.run_until(Duration::from_secs(4), |r| r.seated(player)));
    rig.run(Duration::from_secs(1));
    let mission = |rig: &Rig, n: usize| -> Vec<u32> {
        flattened(rig.players[n].client.mission().expect("loaded"))
    };
    assert!(mission(&rig, player).is_empty());
    assert!(
        rig.players[player]
            .client
            .mission()
            .unwrap()
            .terrain
            .fill_contact(THROUGH.0, THROUGH.1)
    );

    // A ground burst between the first two towers, as combat's step makes
    // it.
    let host_fill = rig.host.world().terrain.city_fill.clone().unwrap();
    assert_eq!(host_fill.flatten([10_200., 0., 20_000.], 300.), 2);
    assert!(rig.run_until(Duration::from_secs(2), |r| mission(r, player).len() == 2));
    assert_eq!(mission(&rig, player), flattened(rig.host.world()));
    // The copy of the mission the prediction flies through: no contact.
    assert!(
        !rig.players[player]
            .client
            .mission()
            .unwrap()
            .terrain
            .fill_contact(THROUGH.0, THROUGH.1)
    );

    // A player seated later is told every building already flat.
    let late = rig.join(|c| c.callsign = "Hawk".into(), level_script());
    with_fill(&mut rig.players[late].client);
    assert!(rig.run_until(Duration::from_secs(4), |r| r.seated(late)));
    assert!(rig.run_until(Duration::from_secs(2), |r| mission(r, late).len() == 2));
    assert_eq!(mission(&rig, late), [0, 1]);

    // A later burst reaches both.
    assert_eq!(host_fill.flatten([30_000., 0., 30_000.], 100.), 1);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        mission(r, player).len() == 3 && mission(r, late).len() == 3
    }));
}
