//! Tab, the gun key, over the network (slice FX-T): the key crosses as a
//! `GunKeyDown` and a `GunKeyUp` combat command, and the host fires the gun
//! whatever store the player has selected. On the network simulator;
//! synthetic resources.

use super::tests::{Rig, spec};
use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use tore_net::sim::LinkConfig;
use tore_sim::combat::live;
use tore_world::seats::SeatCommand;

const MS: Duration = Duration::from_millis(1);

/// A level player that gives the commands queued for it, once each.
fn commanding_player(rig: &mut Rig, queue: &Rc<RefCell<VecDeque<SeatCommand>>>) -> usize {
    let queue = Rc::clone(queue);
    rig.join(
        |_| {},
        Box::new(move |_, _, _| Controls {
            commands: queue.borrow_mut().drain(..).collect(),
            ..Controls::default()
        }),
    )
}

/// The plane's station that is a gun, its selected station and the rounds at
/// every station, as the host has them.
fn on_host(rig: &Rig, plane: u32) -> (usize, usize, Vec<u16>) {
    let world = rig.host.world();
    let own = world.combat.state.ownship(plane).expect("an ownship");
    let gun = own
        .configuration()
        .stations
        .iter()
        .position(|s| live::is_gun(&s.weapon))
        .expect("a gun");
    let rounds = (0..own.configuration().stations.len())
        .map(|i| own.rounds(i))
        .collect();
    (gun, own.selected, rounds)
}

#[test]
fn tab_from_a_client_fires_the_gun_on_the_host_with_another_store_selected() {
    let mut rig = Rig::new(
        spec(2, 0, 20),
        LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
        23,
    );
    let queue = Rc::new(RefCell::new(VecDeque::new()));
    let player = commanding_player(&mut rig, &queue);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(player)));
    rig.run(Duration::from_secs(2));
    let plane = rig.players[player].client.seat().unwrap().1.0;
    // Step off the gun to a store that is not one.
    for _ in 0..6 {
        let (gun, selected, _) = on_host(&rig, plane);
        if selected != gun {
            break;
        }
        queue
            .borrow_mut()
            .push_back(SeatCommand::CycleWeapon { forward: true });
        rig.run(Duration::from_millis(300));
    }
    let (gun, selected, before) = on_host(&rig, plane);
    assert_ne!(selected, gun, "another store is selected");
    assert!(before[gun] > 0, "the gun is loaded");
    // Tab down: the gun fires, the rest stays put.
    queue
        .borrow_mut()
        .push_back(SeatCommand::Combat(live::Command::GunKeyDown));
    rig.run(Duration::from_millis(1_500));
    let (_, still, during) = on_host(&rig, plane);
    assert!(during[gun] < before[gun], "{before:?} then {during:?}");
    assert_eq!(still, selected, "the selection did not move");
    for (i, rounds) in before.iter().enumerate().filter(|(i, _)| *i != gun) {
        assert_eq!(during[i], *rounds, "station {i}");
    }
    // Tab up: it stops.
    queue
        .borrow_mut()
        .push_back(SeatCommand::Combat(live::Command::GunKeyUp));
    rig.run(Duration::from_millis(600));
    let (_, _, stopped) = on_host(&rig, plane);
    rig.run(Duration::from_secs(2));
    let (_, _, end) = on_host(&rig, plane);
    assert_eq!(end, stopped, "no more rounds once Tab is up");
}
