//! The shared plane step and the exact state against the whole tick: a
//! human-flown plane's exact state is coded halfway through a mission and
//! decoded into a fresh copy, which, stepped only with
//! [`plane::OwnPlane::step`], stays identical to the `World`'s cockpit to the
//! last bit, its own state hash included (docs/ARCHITECTURE.md, "One step for
//! a human's plane").
//!
//! The copy is fed each tick only what a network client would have: the
//! seat's input, the ground objects standing before the tick, the weather
//! clock's reading after it, and the ownship terms and combat events the
//! `World`'s tick produced. It flies over a terrain of its own, whose weather
//! clock never moves, with an aircraft type of its own. Synthetic fixtures
//! only.

use super::crowd::{E_HUMAN, F_HUMAN, crowded_mission, inputs};
use super::plane::{ExactState, OwnPlane, OwnshipTerms, PlaneTick, WeatherReading};
use super::tick_tests::{DRONES, airport, mission, place_drone, player_aircraft, script};
use super::*;
use tore_input::{PilotCommand, PilotInput, Switch};
use tore_sim::combat::live::Event;

/// The copy is taken after this many ticks, and both fly to [`END`].
const COPY_AT: usize = 600;
const END: usize = 1200;

/// The terms `plane` took in the tick that made `out`.
fn terms_of(out: &TickOutput, plane: PlaneId) -> Option<OwnshipTerms> {
    out.terms
        .iter()
        .find(|(id, _)| *id == plane)
        .map(|(_, terms)| *terms)
}

/// The exact state of `world`'s cockpit of `plane`, with the terms of the
/// last tick.
fn exact_of(world: &World, plane: PlaneId, out: &TickOutput) -> ExactState {
    ExactState::of(
        &OwnPlane::of(cockpit_of(world, plane)),
        terms_of(out, plane).as_ref(),
    )
}

/// The copy a client would make: `world`'s plane coded with no baseline and
/// decoded with the aircraft type `model`, checked equal and with the same
/// hash.
fn decoded_copy(
    world: &World,
    plane: PlaneId,
    out: &TickOutput,
    model: &tore_sim::models::AircraftModel,
) -> OwnPlane {
    let exact = exact_of(world, plane, out);
    let bytes = exact.encode(None).unwrap();
    let decoded = ExactState::decode(&bytes, None, model).unwrap();
    assert_eq!(decoded, exact);
    assert_eq!(decoded.hash().unwrap(), exact.hash().unwrap());
    assert!(decoded.flight.research.is_some());
    assert_ne!(
        decoded.flight.stall_scale(),
        1.,
        "the stall speeds were never scaled"
    );
    decoded.into_own_plane(plane.0).0
}

/// Codes `world`'s plane against `base`, its exact state one snapshot back,
/// decodes it against the same, and returns the new state and the bytes it
/// took.
fn against_baseline(
    world: &World,
    plane: PlaneId,
    out: &TickOutput,
    base: &ExactState,
    model: &tore_sim::models::AircraftModel,
) -> (ExactState, usize) {
    let exact = exact_of(world, plane, out);
    let bytes = exact.encode(Some(base)).unwrap();
    let decoded = ExactState::decode(&bytes, Some(base), model).unwrap();
    assert_eq!(decoded, exact);
    (exact, bytes.len())
}

fn cockpit_of(world: &World, plane: PlaneId) -> &Cockpit {
    world
        .cockpits
        .iter()
        .find(|cockpit| cockpit.plane == plane)
        .expect("a human-flown plane")
}

/// The ground objects standing as the tick starts, which building contact
/// reads.
fn standing(world: &World) -> Vec<u32> {
    world
        .combat
        .state
        .targets
        .iter()
        .filter(|target| target.hp > 0)
        .map(|target| target.id)
        .collect()
}

/// The fixtures fly the legacy model; a human flies the hybrid one by
/// default, with its random stream and weight-scaled stall speeds.
fn fly_hybrid(world: &mut World, plane: PlaneId) {
    let index = world
        .cockpits
        .iter()
        .position(|cockpit| cockpit.plane == plane)
        .unwrap();
    let cockpit = &mut world.cockpits[index];
    cockpit.flight.enable_research(1).unwrap();
    cockpit.previous_flight = cockpit.flight.clone();
}

/// The terrain the copy flies over: the tick fixture's, built again.
fn own_terrain() -> terrain::Terrain {
    let mut terrain = crate::test_support::terrain();
    terrain.airport_scene = airport();
    terrain
}

/// One tick of the copy with what the `World`'s tick gave out, then the
/// systems messages drained as the `World` drains them into the seat's cues.
fn step_copy(
    copy: &mut OwnPlane,
    input: &SeatInput,
    standing: &[u32],
    world: &World,
    out: &TickOutput,
    terrain: &terrain::Terrain,
    config: &tore_sim::combat::live::Configuration,
) {
    let terms = out
        .terms
        .iter()
        .find(|(plane, _)| plane.0 == copy.plane)
        .map(|(_, terms)| terms);
    let tick = PlaneTick {
        sensors: input.sensors,
        pilot: &input.pilot,
        standing,
        weather: WeatherReading::of(&world.terrain.weather),
        terms,
        events: &out.events,
    };
    copy.step(&tick, terrain, config).unwrap();
    copy.flight.systems.messages.clear();
}

/// The copy and the cockpit are equal and code to the same bits: the same
/// own state hash, and the same flight at the start of the tick.
fn assert_copy_matches(tick: usize, copy: &OwnPlane, world: &World, out: &TickOutput) {
    let plane = PlaneId(copy.plane);
    let cockpit = cockpit_of(world, plane);
    let terms = terms_of(out, plane);
    assert_eq!(copy.flight, cockpit.flight, "tick {tick}: the flight");
    assert_eq!(copy.turbulence, cockpit.turbulence, "tick {tick}");
    assert_eq!(copy.turbulence_rng, cockpit.turbulence_rng, "tick {tick}");
    assert_eq!(copy.edge_message_at, cockpit.edge_message_at, "tick {tick}");
    assert_eq!(
        copy.overspeed_message_at, cockpit.overspeed_message_at,
        "tick {tick}"
    );
    assert_eq!(
        ExactState::of(copy, terms.as_ref()).hash().unwrap(),
        ExactState::of(&OwnPlane::of(cockpit), terms.as_ref())
            .hash()
            .unwrap(),
        "tick {tick}: the own state hash",
    );
    let bits = |flight: &flight::State| {
        let mut w = tore_codec::BitWriter::new();
        flight.write_exact(&mut w, None).unwrap();
        w.finish()
    };
    assert_eq!(
        bits(&copy.previous_flight),
        bits(&cockpit.previous_flight),
        "tick {tick}: the flight at the start of the tick",
    );
}

/// The single-player tick mission: low over rising ground, so turbulence acts;
/// the gear, throttle, flaps and airbrake move; the gun fires at a drone.
#[test]
fn a_copy_of_the_single_player_plane_steps_like_the_world() {
    let mut world = mission();
    fly_hybrid(&mut world, PlaneId(0));
    let terrain = own_terrain();
    let model = tore_sim::models::AircraftModel::for_aircraft(&player_aircraft()).unwrap();
    let config = world.combat.own().configuration().clone();
    let mut out = TickOutput::default();
    let mut copy = None;
    let mut turbulence_moved = false;
    for tick in 0..END {
        match tick {
            195 => place_drone(&mut world, DRONES[0]),
            695 => place_drone(&mut world, DRONES[1]),
            _ => {}
        }
        if tick == COPY_AT {
            copy = Some(decoded_copy(&world, PlaneId(0), &out, &model));
        }
        let mut input = script(tick);
        input.tick = world.tick();
        let standing = standing(&world);
        world.step(std::slice::from_ref(&input), &mut out).unwrap();
        if let Some(copy) = &mut copy {
            let before = copy.turbulence;
            step_copy(copy, &input, &standing, &world, &out, &terrain, &config);
            turbulence_moved |= copy.turbulence != before;
            assert_copy_matches(tick, copy, &world, &out);
        }
    }
    assert!(turbulence_moved, "turbulence never acted on the copy");
}

/// Rounds leave the gun at this speed, feet per second, as in `fight_tests.rs`.
const ROUND_FPS: f64 = 1032.;
/// The ticks a round flies to the plane.
const FLIGHT: f64 = 40.;
/// Our plane's seat in the crowd.
const SEAT: SeatId = SeatId(1);
/// The shooter's seat, flying the enemy's second human plane.
const SHOOTER_SEAT: SeatId = SeatId(3);
/// The shooter's bursts at our plane; the second finds it with two hit points.
const BURSTS: [usize; 2] = [800, 1000];
/// The ticks the shooter holds the trigger.
const HELD: usize = 16;

/// Puts the shooter behind `victim` on the line its first round will fly, so
/// it hits after [`FLIGHT`] ticks. Only the shooter moves: the victim's flight
/// is never touched outside the step.
fn line_up(world: &mut World, shooter: PlaneId, victim: PlaneId) {
    let v = cockpit_of(world, victim).flight.clone();
    let forward = attitude::Basis::new(v.yaw, v.pitch, v.bank).forward;
    let position: [f64; 3] = std::array::from_fn(|i| {
        v.position[i] + (v.velocity[i] - forward[i] * ROUND_FPS) / 120. * FLIGHT
    });
    let index = world
        .cockpits
        .iter()
        .position(|cockpit| cockpit.plane == shooter)
        .unwrap();
    let s = &mut world.cockpits[index].flight;
    s.position = position;
    s.velocity = v.velocity;
    (s.yaw, s.pitch, s.bank) = (v.yaw, v.pitch, v.bank);
    s.speed = v.speed;
}

/// Moves the shooter well clear of the victim, 6,000 ft above it.
fn clear(world: &mut World, shooter: PlaneId) {
    let index = world
        .cockpits
        .iter()
        .position(|cockpit| cockpit.plane == shooter)
        .unwrap();
    world.cockpits[index].flight.position[1] += 6000.;
}

/// Our plane's stick, throttle, gear and airbrake, and its own burst.
fn our_pilot(tick: usize) -> PilotInput {
    let mut pilot = PilotInput::default();
    match tick {
        10 => pilot.commands.push(PilotCommand::Set(Switch::Radar, true)),
        620 => pilot.commands.push(PilotCommand::Throttle(0.8)),
        650 | 760 => pilot.commands.push(PilotCommand::Toggle(Switch::Gear)),
        700 => pilot.commands.push(PilotCommand::Set(Switch::Burner, true)),
        900 | 960 => pilot.commands.push(PilotCommand::Toggle(Switch::Airbrake)),
        _ => {}
    }
    match tick {
        640..680 => pilot.roll = 0.3,
        720..760 => pilot.pitch = 0.2,
        880..940 => pilot.yaw = -0.2,
        _ => {}
    }
    pilot
}

/// The crowd's fight: our plane, a human wingman of Friendly Wing 1, fires,
/// is hit by an enemy human, and is shot down, and its copy follows it through
/// the damage, the crash and the wreck.
#[test]
fn a_copy_of_a_human_wingman_steps_like_the_world_through_a_fight() {
    let mut world = crowded_mission();
    fly_hybrid(&mut world, F_HUMAN);
    let terrain = own_terrain();
    let model =
        tore_sim::models::AircraftModel::for_aircraft(&crate::test_support::aircraft()).unwrap();
    let config = world
        .combat
        .state
        .ownship(F_HUMAN.0)
        .unwrap()
        .configuration()
        .clone();
    let mut out = TickOutput::default();
    let mut copy = None;
    let (mut fired, mut damaged) = (0, 0);
    let mut snapshot: Option<ExactState> = None;
    let mut sizes = Vec::new();
    for tick in 0..END {
        for start in BURSTS {
            if tick + 1 == start {
                if start == BURSTS[1] {
                    world.combat.state.ownship_mut(F_HUMAN.0).unwrap().hp = 2;
                }
                line_up(&mut world, E_HUMAN, F_HUMAN);
            }
            if tick == start + 90 {
                clear(&mut world, E_HUMAN);
            }
        }
        if tick == COPY_AT {
            copy = Some(decoded_copy(&world, F_HUMAN, &out, &model));
        }
        let step_inputs = inputs(&world, |seat| SeatInput {
            pilot: if seat == SEAT {
                our_pilot(tick)
            } else {
                PilotInput::default()
            },
            trigger: (seat == SEAT && (660..676).contains(&tick))
                || (seat == SHOOTER_SEAT
                    && BURSTS
                        .iter()
                        .any(|start| (*start..start + HELD).contains(&tick))),
            ..SeatInput::default()
        });
        let input = step_inputs.iter().find(|i| i.seat == SEAT).unwrap().clone();
        let standing = standing(&world);
        world.step(&step_inputs, &mut out).unwrap();
        if let Some(copy) = &mut copy {
            step_copy(copy, &input, &standing, &world, &out, &terrain, &config);
            assert_copy_matches(tick, copy, &world, &out);
            for event in &out.events {
                match event {
                    Event::Fired { aircraft, .. } if *aircraft == F_HUMAN.0 => fired += 1,
                    Event::OwnshipDamaged { aircraft, .. } if *aircraft == F_HUMAN.0 => {
                        damaged += 1
                    }
                    _ => {}
                }
            }
        }
        // Every fourth tick from the copy on, the host's exact state coded
        // against the one a snapshot before, as the wire sends it.
        if tick >= COPY_AT && tick % 4 == 0 {
            let exact = match &snapshot {
                Some(base) => {
                    let (exact, size) = against_baseline(&world, F_HUMAN, &out, base, &model);
                    sizes.push(size);
                    exact
                }
                None => exact_of(&world, F_HUMAN, &out),
            };
            snapshot = Some(exact);
        }
    }
    let copy = copy.unwrap();
    let mean = sizes.iter().sum::<usize>() as f64 / sizes.len().max(1) as f64;
    eprintln!(
        "exact own state against one snapshot back, ticks 604 to 1,200: {} to {} bytes, mean \
         {mean:.0}; {} bytes with no baseline at the end",
        sizes.iter().min().unwrap(),
        sizes.iter().max().unwrap(),
        exact_of(&world, F_HUMAN, &out).encode(None).unwrap().len(),
    );
    assert!(fired > 0, "our plane never fired after the copy was taken");
    assert!(
        damaged > 1,
        "our plane was hit {damaged} times after the copy"
    );
    assert!(copy.flight.crashed, "our plane was not shot down");
    assert!(copy.flight.damage_fraction > 0.);
}
