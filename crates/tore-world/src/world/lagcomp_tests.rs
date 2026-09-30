//! Lag compensation through the world (docs/ARCHITECTURE.md, "Hits and lag
//! compensation"): a seat's view in its input rewinds the gun rounds it fires,
//! and nothing else. Synthetic fixtures only.

use super::tick_tests::{DRONES, mission};
use super::*;
use crate::seats::{SeatCommand, SeatView};
use tore_sim::{
    attitude::Vector,
    combat::{live::Event, missiles},
};

/// 500 knots, feet per second.
const CROSSING_FPS: f64 = 500. * 6076.12 / 3600.;
/// A 150 ms round trip, a 100 ms interpolation delay and a tick of input
/// margin: the shooter's screen shows the other aircraft 31 ticks behind the
/// tick its input is for.
const BEHIND: u64 = 31;
/// The interpolation delay, ticks: 100 ms.
const DELAY: u8 = 12;
/// The tick the burst starts on and how long the trigger is held.
const FIRE: u64 = 60;
const BURST: u64 = 6;
/// How far out the first round meets the target, feet.
const RANGE: f64 = 1200.;
const TICKS: u64 = 240;

/// The fixture mission with nothing in it but the player, one drone the size
/// of an aircraft and the airport's objects.
fn range() -> World {
    let mut world = mission();
    world.ai_wings = None;
    world.roster = Roster::single_player(Some(comms::Crew::Rio), []);
    world
        .combat
        .state
        .targets
        .retain(|t| t.id == DRONES[0] || t.id > 1000);
    let drone = drone(&mut world);
    drone.radius = tore_sim::combat::live::AIRCRAFT_RADIUS_FT;
    world
}

fn drone(world: &mut World) -> &mut tore_sim::combat::live::Target {
    world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == DRONES[0])
        .unwrap()
}

/// Seat 0's input for the world's next tick: the trigger held during the
/// burst, `view` naming what its screen showed, and, from `commands_at`,
/// commands.
fn input(world: &World, view: bool, commands: Vec<SeatCommand>) -> SeatInput {
    let tick = world.tick();
    SeatInput {
        tick,
        trigger: (FIRE..FIRE + BURST).contains(&tick),
        view: view.then(|| SeatView {
            tick: tick.saturating_sub(BEHIND),
            interpolation_delay: DELAY,
        }),
        commands,
        ..SeatInput::default()
    }
}

/// Flies the range with no target in the way and follows the burst's first
/// round: where it is `RANGE` feet out, its direction there, and the tick.
fn calibrate() -> (Vector, Vector, u64) {
    let mut world = range();
    let mut out = TickOutput::default();
    let mut first: Option<(u32, Vector)> = None;
    for _ in 0..TICKS {
        let tick = world.tick();
        world
            .step(&[input(&world, false, Vec::new())], &mut out)
            .unwrap();
        let state = &world.combat.state;
        if first.is_none() {
            first = state
                .projectiles
                .iter()
                .filter(|p| p.owner == 0)
                .min_by_key(|p| p.id)
                .map(|p| (p.id, p.previous));
        }
        let Some((id, muzzle)) = first else {
            continue;
        };
        let p = state
            .projectiles
            .iter()
            .find(|p| p.id == id)
            .expect("the first round flew out of range too soon");
        let out_to = missiles::length(std::array::from_fn(|i| p.position[i] - muzzle[i]));
        if out_to >= RANGE {
            let direction =
                tore_sim::attitude::unit(std::array::from_fn(|i| p.position[i] - p.previous[i]));
            return (p.position, direction, tick);
        }
    }
    panic!("the burst never reached {RANGE} ft");
}

/// The range with the drone crossing the line of fire at 500 knots, placed
/// so that [`BEHIND`] ticks before the first round gets `RANGE` feet out,
/// the drone is where the round will be: where the shooter's screen shows it
/// as the round arrives, 218 ft short of where it really is by then.
fn crossing() -> World {
    let (meet, direction, arrival) = calibrate();
    let mut world = range();
    // Level and square to the line of fire.
    let across = tore_sim::attitude::unit([direction[2], 0., -direction[0]]);
    let velocity = across.map(|v| v * CROSSING_FPS);
    // A row moves before the hit search every tick, so on the tick after
    // `arrival - BEHIND` ticks it is searched `arrival - BEHIND + 1` ticks of
    // travel from where it starts.
    let travel = (arrival - BEHIND + 1) as f64 / 120.;
    let drone = drone(&mut world);
    drone.position = std::array::from_fn(|i| meet[i] - velocity[i] * travel);
    drone.velocity = velocity;
    drone.basis = tore_sim::attitude::Basis::new(across[0].atan2(across[2]), 0., 0.);
    world
}

/// Flies [`crossing`] with or without the seat's view: whether the burst hit
/// the drone.
fn burst_hits(view: bool) -> bool {
    let mut world = crossing();
    let mut out = TickOutput::default();
    let mut hit = false;
    for _ in 0..TICKS {
        world
            .step(&[input(&world, view, Vec::new())], &mut out)
            .unwrap();
        hit |= out
            .events
            .iter()
            .any(|e| matches!(e, Event::Hit(id) if *id == DRONES[0]));
    }
    hit
}

#[test]
fn a_burst_aimed_at_the_drawn_target_hits_with_the_view_and_misses_without() {
    assert!(
        burst_hits(true),
        "a burst aimed where the screen showed the target must hit"
    );
    assert!(
        !burst_hits(false),
        "without the view the target has moved 218 ft on"
    );
}

#[test]
fn the_rewind_is_the_view_capped_beyond_the_interpolation_delay() {
    use crate::combat::gun_rewind;
    let view = |behind: u64, delay: u8| {
        Some(SeatView {
            tick: 1000 - behind,
            interpolation_delay: delay,
        })
    };
    // Single player and every local seat: no view, no rewind.
    assert_eq!(gun_rewind(1000, None), 0);
    // 150 ms round trip, 100 ms delay, a tick of margin.
    assert_eq!(gun_rewind(1000, view(31, 12)), 31);
    // A slow link: the part beyond the delay is capped at 30 ticks.
    assert_eq!(gun_rewind(1000, view(100, 12)), 42);
    assert_eq!(gun_rewind(1000, view(43, 12)), 42);
    // The whole is capped at 60 ticks, however long the delay.
    assert_eq!(gun_rewind(1000, view(100, 40)), 60);
    assert_eq!(gun_rewind(1000, view(1000, 63)), 60);
    // A view of this tick or a later one is no rewind.
    assert_eq!(gun_rewind(1000, view(0, 12)), 0);
    assert_eq!(
        gun_rewind(
            1000,
            Some(SeatView {
                tick: 1010,
                interpolation_delay: 12
            })
        ),
        0
    );
}

/// The rewinds seat 0's rounds carry when its view is `behind` ticks behind
/// with a 100 ms delay, and which station fired each.
fn carried(behind: u64, missile: bool) -> Vec<(usize, u16)> {
    let mut world = range();
    let mut out = TickOutput::default();
    let mut rounds = std::collections::BTreeMap::new();
    for _ in 0..FIRE + BURST + 2 {
        let tick = world.tick();
        let commands = if missile && tick == 10 {
            vec![SeatCommand::CycleWeapon { forward: true }]
        } else {
            Vec::new()
        };
        let mut input = input(&world, true, commands);
        input.view = Some(SeatView {
            tick: tick.saturating_sub(behind),
            interpolation_delay: DELAY,
        });
        world.step(&[input], &mut out).unwrap();
        let state = &world.combat.state;
        for p in state.projectiles.iter().filter(|p| p.owner == 0) {
            rounds.insert(p.id, (p.station, state.rewind_of(p.id)));
        }
    }
    assert!(!rounds.is_empty(), "the seat fired nothing");
    rounds.into_values().collect()
}

#[test]
fn a_seats_gun_rounds_carry_the_capped_rewind_and_its_missiles_none() {
    assert!(carried(BEHIND, false).iter().all(|&r| r == (0, 31)));
    // A 1 s round trip: 12 ticks of delay and 30 beyond it.
    assert!(carried(120, false).iter().all(|&r| r == (0, 42)));
    let missiles = carried(BEHIND, true);
    assert!(missiles.iter().any(|&(station, _)| station == 1));
    assert!(
        missiles.iter().all(|&(_, rewind)| rewind == 0),
        "{missiles:?}"
    );
}
