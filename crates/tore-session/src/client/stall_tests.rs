//! A joined game that stalls while its player holds the trigger and a pull,
//! on the network simulator (EF-K follow-up): the host repeats the held
//! controls for half a second, as for any late player, then flies the plane
//! neutral, as a paused game's controls are (the lead's reading of John's
//! rule of 2026-09-30), until the first fresh input. With keepalives the
//! host logs the stall and its end. Synthetic resources.

use super::tests::{Rig, spec};
use super::*;
use crate::host::HostLog;
use std::cell::Cell;
use std::rc::Rc;
use tore_net::Datagrams;
use tore_net::sim::LinkConfig;

const MS: Duration = Duration::from_millis(1);

/// A player that flies level until `hold` is set, then pulls and holds the
/// trigger.
fn holding_player(rig: &mut Rig, hold: &Rc<Cell<bool>>) -> usize {
    let hold = Rc::clone(hold);
    rig.join(
        |_| {},
        Box::new(move |_, _, _| {
            if hold.get() {
                Controls {
                    pilot: PilotInput {
                        pitch: 0.7,
                        ..PilotInput::default()
                    },
                    trigger: true,
                    ..Controls::default()
                }
            } else {
                Controls::default()
            }
        }),
    )
}

/// The rounds left on the plane and its elevator, as the host flies it.
fn on_host(rig: &Rig, plane: u32) -> (u32, f64) {
    let world = rig.host.world();
    let cockpit = world
        .cockpits
        .iter()
        .find(|c| c.plane.0 == plane)
        .expect("the seat's cockpit");
    let readout = world
        .combat
        .cockpit_readout(
            plane,
            tore_world::combat::launcher(&cockpit.flight),
            world.ai_wings.as_ref(),
            Some(cockpit),
        )
        .expect("a readout");
    let rounds = readout
        .stores
        .ammo
        .iter()
        .map(|a| u32::from(a & 0x7fff))
        .sum();
    (rounds, cockpit.flight.elevator)
}

/// Seats a holding player, holds for 0.3 s, and stalls it; returns the
/// player, its plane and the rounds and elevator as the stall began.
fn seated_and_holding(rig: &mut Rig, hold: &Rc<Cell<bool>>) -> (usize, u32, (u32, f64)) {
    let player = holding_player(rig, hold);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(player)));
    rig.run(Duration::from_secs(2));
    hold.set(true);
    rig.run(Duration::from_millis(300));
    let plane = rig.players[player].client.seat().unwrap().1.0;
    let before = on_host(rig, plane);
    rig.players[player].stalled = true;
    (player, plane, before)
}

/// Without keepalives (a stall shorter than a second, or a game whose
/// thread cannot send): the held trigger and pull go on for half a second
/// and then stop, and the throttle stays.
#[test]
fn a_stalled_game_holding_the_trigger_and_a_pull_stops_after_half_a_second() {
    let mut rig = Rig::new(
        spec(2, 0, 20),
        LinkConfig::for_round_trip(20 * MS, 0., 0., 0.),
        21,
    );
    let hold = Rc::new(Cell::new(false));
    let (_, plane, (rounds_at_stall, elevator_at_stall)) = seated_and_holding(&mut rig, &hold);
    let throttle_at_stall = rig
        .host
        .world()
        .cockpits
        .iter()
        .find(|c| c.plane.0 == plane)
        .unwrap()
        .flight
        .throttle;
    // Still repeating the held controls 0.4 s in.
    rig.run(Duration::from_millis(400));
    let (rounds_held, elevator_held) = on_host(&rig, plane);
    // Neutral from about 0.5 s on: nothing more is fired.
    rig.run(Duration::from_millis(300));
    let (rounds_neutral, _) = on_host(&rig, plane);
    rig.run(Duration::from_millis(2300));
    let (rounds_end, elevator_end) = on_host(&rig, plane);
    let throttle_end = rig
        .host
        .world()
        .cockpits
        .iter()
        .find(|c| c.plane.0 == plane)
        .unwrap()
        .flight
        .throttle;
    eprintln!(
        "rounds {rounds_at_stall} at the stall, {rounds_held} at 0.4 s, {rounds_neutral} at \
         0.7 s, {rounds_end} at 3 s; elevator {elevator_at_stall:.3}, {elevator_held:.3}, \
         {elevator_end:.3}; throttle {throttle_at_stall:.3} then {throttle_end:.3}"
    );
    assert!(rounds_held < rounds_at_stall, "the held trigger fires on");
    assert_eq!(rounds_end, rounds_neutral, "no rounds once neutral");
    assert!(elevator_held.abs() > 0.5 * elevator_at_stall.abs() && elevator_at_stall.abs() > 0.);
    assert!(
        elevator_end.abs() < 0.1 * elevator_at_stall.abs(),
        "the pull stops: {elevator_end} against {elevator_at_stall}"
    );
    assert!(
        (throttle_end - throttle_at_stall).abs() < 1e-9,
        "the throttle stays"
    );
}

/// With keepalives a stall of 7 seconds is kept: the host logs the stall
/// and its end with how long it lasted, flies the plane neutral meanwhile,
/// and the first fresh input after it is flown again (the trigger fires).
#[test]
fn a_stall_the_keepalives_cover_is_logged_and_ends_at_the_first_fresh_input() {
    let mut rig = Rig::new(
        spec(2, 0, 20),
        LinkConfig::for_round_trip(20 * MS, 0., 0., 0.),
        22,
    );
    let hold = Rc::new(Cell::new(false));
    let (player, plane, _) = seated_and_holding(&mut rig, &hold);
    let keepalive = rig.players[player]
        .client
        .keepalive_datagram()
        .expect("joined");
    let logs_before = rig.logs.len();
    let mut neutral_rounds = None;
    for ms in 1..=7000u32 {
        rig.step();
        if ms % 1000 == 0 {
            rig.players[player]
                .socket
                .send_datagram(super::tests::host_address(), &keepalive)
                .unwrap();
        }
        if ms == 1500 {
            neutral_rounds = Some(on_host(&rig, plane).0);
        }
    }
    let (rounds_before_resume, _) = on_host(&rig, plane);
    assert_eq!(Some(rounds_before_resume), neutral_rounds);
    rig.players[player].stalled = false;
    rig.run(Duration::from_secs(2));
    let (rounds_after, _) = on_host(&rig, plane);
    assert!(
        rounds_after < rounds_before_resume,
        "the fresh input fires again"
    );
    assert!(!rig.closed(player));
    let lines: Vec<String> = rig.logs[logs_before..]
        .iter()
        .filter_map(HostLog::stall_text)
        .collect();
    eprintln!("{lines:?}");
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[0], "seat 0 Viper: game stalled, flying neutral");
    let after: f64 = lines[1]
        .strip_prefix("seat 0 Viper: game back after ")
        .and_then(|rest| rest.strip_suffix(" s"))
        .and_then(|seconds| seconds.parse().ok())
        .unwrap_or_else(|| panic!("{}", lines[1]));
    assert!((6.9..=7.3).contains(&after), "{after}");
}
