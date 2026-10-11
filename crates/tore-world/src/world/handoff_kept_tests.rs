//! A human who gives its plane back and takes it again finds the cockpit as
//! it left it (slice FX-J; docs/ARCHITECTURE.md, "What a returning player
//! keeps"), on the synthetic laser bomber (the F/A-18D's laser pod and two
//! GBU-10s) over the routed column: the weapon and NAV, the sight's pin,
//! the laser, the A-G page and the designation, the bomb sight's mode, the
//! radar switch and NAV's airport. A laser bomb it left guiding follows the
//! laser while the AI flies, and a checkpoint taken while the AI flies
//! carries all of it.

use super::*;
use crate::mission::{MissionSpec, Start};
use crate::seats::SeatCommand;
use crate::test_support::resources::{a2g_loadout, add_a2g_stores};
use crate::test_support::surface::{routed_resources, spec_with_target, target};
use tore_input::{PilotCommand, Switch};
use tore_sim::combat::{
    delivery::DeliveryMode,
    laser::LaserState,
    live::{self, Command as Combat, GuidedMode},
    sight::SightMode,
};

const PLANE: PlaneId = PlaneId(0);
/// Ticks of full downward slew before the pin.
const SLEW_TICKS: usize = 420;

fn spec() -> MissionSpec {
    let mut spec = spec_with_target(&target("QUCOL", 0, 0, 7));
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec.plane_loadouts.insert(PLANE.0, a2g_loadout());
    spec.cheat_loadouts = true;
    spec
}

fn world() -> World {
    let mut resources = routed_resources();
    add_a2g_stores(&mut resources);
    World::new(&spec(), &resources, Seating::Open).unwrap()
}

/// One tick: the mission commands, then `seat` (if it flies) with
/// `commands`, `pilot` commands and the trigger.
fn tick(
    world: &mut World,
    mission: &[MissionCommand],
    seat: Option<SeatId>,
    commands: Vec<SeatCommand>,
    pilot: Vec<PilotCommand>,
    trigger: bool,
) {
    let tick = world.tick();
    let inputs: Vec<SeatInput> = seat
        .map(|seat| {
            let mut input = SeatInput {
                seat,
                tick,
                trigger,
                commands,
                ..SeatInput::default()
            };
            input.pilot.commands = pilot;
            input
        })
        .into_iter()
        .collect();
    let mut out = TickOutput::default();
    world
        .step_with(mission, &inputs, &mut out, |_, _| Ok(()))
        .unwrap();
}

fn fly(world: &mut World, seat: Option<SeatId>, ticks: usize) {
    for _ in 0..ticks {
        tick(world, &[], seat, Vec::new(), Vec::new(), false);
    }
}

fn own(world: &World) -> &live::Ownship {
    world.combat.state.ownship(PLANE.0).expect("an ownship")
}

/// Seat 1 takes plane 0 and flies the laser bomber's run: the A-G page and
/// the laser bomb, the sight slewed down and pinned on the ground ahead,
/// Ctrl+M (which leaves a guided bomb on CCIP), the laser off by hand and a
/// bomb released, which locks on the spot as it falls. (The routed theater
/// has no airport; NAV's airport has its own test.)
fn bomber_run() -> World {
    let mut world = world();
    let seat = SeatId(1);
    tick(
        &mut world,
        &[MissionCommand::Take { seat, plane: PLANE }],
        Some(seat),
        vec![
            SeatCommand::Combat(Combat::ToggleAirGround),
            SeatCommand::CycleWeapon { forward: true },
        ],
        Vec::new(),
        false,
    );
    // The sight slewed down onto the ground some 20,000 ft ahead, inside the
    // pod's reach in this weather.
    for _ in 0..SLEW_TICKS {
        let tick = world.tick();
        let input = SeatInput {
            seat,
            tick,
            sight: [0, -127],
            ..SeatInput::default()
        };
        let mut out = TickOutput::default();
        world
            .step_with(&[], &[input], &mut out, |_, _| Ok(()))
            .unwrap();
    }
    tick(
        &mut world,
        &[],
        Some(seat),
        vec![
            SeatCommand::Combat(Combat::SightPinGround),
            SeatCommand::Combat(Combat::ToggleSeekerMode),
            // Off by hand: the spot is beyond the bomb's release zone in
            // this weather, which would leave it blind. With no spot it
            // waits, the release puts the laser back on, and the bomb locks
            // in its nose cone (the NA run's sequence).
            SeatCommand::Combat(Combat::ToggleLaser),
        ],
        Vec::new(),
        false,
    );
    fly(&mut world, Some(seat), 240);
    for _ in 0..4 {
        tick(&mut world, &[], Some(seat), Vec::new(), Vec::new(), true);
    }
    fly(&mut world, Some(seat), 120);
    world
}

/// What a returning human checks of its cockpit.
#[derive(Debug, PartialEq)]
struct Cockpit {
    selected: usize,
    armed: bool,
    nav: bool,
    delivery: DeliveryMode,
    launch: tore_sim::combat::missiles::LaunchMode,
    sight: Option<SightMode>,
    laser_manual: Option<bool>,
    air_ground: bool,
    designated: Option<u32>,
    shots: u32,
    radar: bool,
    airport: Option<u32>,
}

fn cockpit(world: &World) -> Cockpit {
    let own = own(world);
    let cockpit = world
        .cockpits
        .iter()
        .find(|c| c.plane == PLANE)
        .expect("a cockpit");
    Cockpit {
        selected: own.selected,
        armed: own.armed,
        nav: cockpit.airport_nav_mode,
        delivery: own.delivery_mode,
        launch: own.launch_mode,
        sight: own.sight.map(|s| s.mode),
        laser_manual: own.laser.manual,
        air_ground: own.sensors.air_ground(),
        designated: own.designated(),
        shots: own.shots,
        radar: cockpit.flight.radar,
        airport: cockpit.airport_service.selected(),
    }
}

#[test]
fn a_returning_human_finds_its_cockpit_and_its_bomb_still_guiding() {
    let mut world = bomber_run();
    let left = cockpit(&world);
    let bomb = own(&world).configuration().stations[left.selected]
        .weapon
        .source
        .clone();
    assert_eq!(bomb, "GBU10.JT", "{left:?}");
    assert!(left.armed && !left.nav);
    assert!(matches!(left.sight, Some(SightMode::Pinned(_))), "{left:?}");
    // Ctrl+M switches an unguided bomb's sight only: the GBU-10 keeps
    // CCIP (the sim's kept-cockpit test covers LVL).
    assert_eq!(left.delivery, DeliveryMode::Ccip);
    assert!(left.air_ground);
    assert_eq!(left.shots, 1);
    let (id, flight) = world
        .combat
        .state
        .guided_flights()
        .map(|(id, f)| (id, *f))
        .next()
        .expect("the bomb in flight");
    assert_eq!(
        (flight.mode, flight.spot_owner),
        (GuidedMode::Guiding, Some(0))
    );
    // The radar switch off, then the human drops.
    tick(
        &mut world,
        &[],
        Some(SeatId(1)),
        Vec::new(),
        vec![PilotCommand::Set(Switch::Radar, false)],
        false,
    );
    let left = cockpit(&world);
    assert!(!left.radar);
    tick(
        &mut world,
        &[MissionCommand::GiveBack { seat: SeatId(1) }],
        None,
        Vec::new(),
        Vec::new(),
        false,
    );
    assert!(world.combat.state.ownship(PLANE.0).is_none());
    // The AI flies with its radar on; the human's cockpit is kept, and its
    // laser keeps the bomb's spot.
    let actor = world.ai_wings.as_ref().unwrap().mission().actor(0).unwrap();
    assert!(actor.flight().radar, "the AI flies with its radar on");
    assert!(world.combat.state.kept(0).is_some());
    for _ in 0..(3 * 120) {
        fly(&mut world, None, 1);
        let flight = world.combat.state.guided_flight(id).expect("still flying");
        assert_eq!(flight.mode, GuidedMode::Guiding);
        assert!(
            world
                .combat
                .state
                .laser_spots()
                .iter()
                .any(|s| s.owner == 0)
        );
    }
    // Taken again from another seat: every setting as it was left.
    let seat = SeatId(2);
    tick(
        &mut world,
        &[MissionCommand::Take { seat, plane: PLANE }],
        Some(seat),
        Vec::new(),
        Vec::new(),
        false,
    );
    assert!(world.combat.state.kept(0).is_none(), "taken");
    let back = cockpit(&world);
    // The pinned sight's look followed the pin while the AI flew.
    assert_eq!(back, left);
    assert_eq!(own(&world).laser.state, LaserState::Lasing);
    // The bomb ends on its spot.
    let mut modes = Vec::new();
    while let Some(flight) = world.combat.state.guided_flight(id) {
        modes.push(flight.mode);
        fly(&mut world, Some(seat), 1);
        assert!(modes.len() < 120 * 90, "the bomb never ended");
    }
    assert!(modes.iter().all(|m| *m == GuidedMode::Guiding), "{modes:?}");
    assert!(world.combat.state.guided_impact(0).is_some());
}

/// A checkpoint taken while the AI flies the plane restores a world that
/// steps on exactly as the original (a standby or a new host carries the
/// kept cockpit), and the human who takes the plane on either finds the
/// same cockpit.
#[test]
fn a_kept_cockpit_rides_a_checkpoint_while_the_ai_flies() {
    let mut world = bomber_run();
    tick(
        &mut world,
        &[MissionCommand::GiveBack { seat: SeatId(1) }],
        None,
        Vec::new(),
        Vec::new(),
        false,
    );
    fly(&mut world, None, 60);
    let bytes = world.checkpoint().unwrap();
    let mut copy = self::world();
    copy.restore(&bytes).unwrap();
    assert_eq!(copy.combat.state.kept(0), world.combat.state.kept(0));
    for w in [&mut world, &mut copy] {
        fly(w, None, 240);
        tick(
            w,
            &[MissionCommand::Take {
                seat: SeatId(3),
                plane: PLANE,
            }],
            Some(SeatId(3)),
            Vec::new(),
            Vec::new(),
            false,
        );
        fly(w, Some(SeatId(3)), 30);
    }
    assert_eq!(cockpit(&copy), cockpit(&world));
    assert_eq!(copy.checkpoint().unwrap(), world.checkpoint().unwrap());
}

/// The radar switch and NAV's airport, on the handoff fixtures' mission
/// (which has an airport): plane 1 taken by seat 1, its radar off and an
/// airport selected, given back (the AI flies with its radar on) and taken
/// again by seat 2.
#[test]
fn the_radar_switch_and_navs_airport_come_back() {
    use crate::combat::fixtures;
    let mut world = super::tick_tests::mission();
    fixtures::set_types(&mut world.combat, fixtures::types());
    let plane = PlaneId(1);
    for target in &mut world.combat.state.targets {
        if (1..=4).contains(&target.id) {
            target.position[1] = 10_000.;
        }
    }
    for id in 1..=4 {
        let wings = world.ai_wings.as_mut().unwrap();
        wings
            .mission_mut()
            .actor_mut(id)
            .unwrap()
            .flight_mut()
            .position[1] = 10_000.;
    }
    let airport = world.terrain.airport_scene.airports[0].id;
    let seats = |world: &World, extra: Option<SeatId>| -> Vec<SeatInput> {
        let tick = world.tick();
        std::iter::once(SeatId(0))
            .chain(extra)
            .map(|seat| SeatInput {
                seat,
                tick,
                ..SeatInput::default()
            })
            .collect()
    };
    let mut out = TickOutput::default();
    let mut inputs = seats(&world, Some(SeatId(1)));
    inputs[1].pilot.commands = vec![PilotCommand::Set(Switch::Radar, false)];
    inputs[1].commands = vec![SeatCommand::Airport(AirportInput::Command(
        tore_sim::airport::Command::SelectAirport(airport),
    ))];
    world
        .step_with(
            &[MissionCommand::Take {
                seat: SeatId(1),
                plane,
            }],
            &inputs,
            &mut out,
            |_, _| Ok(()),
        )
        .unwrap();
    let left = world.cockpits.iter().find(|c| c.plane == plane).unwrap();
    assert!(!left.flight.radar);
    assert_eq!(left.airport_service.selected(), Some(airport));
    let inputs = seats(&world, None);
    world
        .step_with(
            &[MissionCommand::GiveBack { seat: SeatId(1) }],
            &inputs,
            &mut out,
            |_, _| Ok(()),
        )
        .unwrap();
    let actor = world.ai_wings.as_ref().unwrap().mission().actor(1).unwrap();
    assert!(actor.flight().radar, "the AI flies with its radar on");
    for _ in 0..120 {
        let inputs = seats(&world, None);
        world
            .step_with(&[], &inputs, &mut out, |_, _| Ok(()))
            .unwrap();
    }
    let inputs = seats(&world, Some(SeatId(2)));
    world
        .step_with(
            &[MissionCommand::Take {
                seat: SeatId(2),
                plane,
            }],
            &inputs,
            &mut out,
            |_, _| Ok(()),
        )
        .unwrap();
    let back = world.cockpits.iter().find(|c| c.plane == plane).unwrap();
    assert!(!back.flight.radar, "the radar switch as left");
    assert_eq!(back.airport_service.selected(), Some(airport));
}
