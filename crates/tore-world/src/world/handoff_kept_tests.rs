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

fn spec_at(altitude_ft: u32) -> MissionSpec {
    let mut spec = spec_with_target(&target("QUCOL", 0, 0, 7));
    spec.start = Start::Airborne { altitude_ft };
    spec.plane_loadouts.insert(PLANE.0, a2g_loadout());
    spec.cheat_loadouts = true;
    spec
}

fn world() -> World {
    world_at(10_000)
}

fn world_at(altitude_ft: u32) -> World {
    let mut resources = routed_resources();
    add_a2g_stores(&mut resources);
    World::new(&spec_at(altitude_ft), &resources, Seating::Open).unwrap()
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
/// Ctrl+M (which puts the glide bomb on LVL), the laser off by hand and a
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
    // Ctrl+M switches a glide bomb's sight too (FX-H): the GBU-10 is on LVL.
    assert_eq!(left.delivery, DeliveryMode::Level);
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

// ---------------------------------------------------------------------------
// Slice ST-K: the AI continues a human's ground attack (docs/spec/ai-strike.md,
// "Takeover and handback").

/// The AI's strike on plane 0, while it flies the plane.
fn strike(world: &World) -> Option<&tore_sim::ai::strike::Strike> {
    world.ai_wings.as_ref()?.mission().actor(PLANE.0)?.strike()
}

/// Seat 1 takes plane 0, heading at the column at 5,000 ft (the synthetic
/// GBU-10's release zone, 45 degrees under the nose, barely reaches the
/// ground from 10,000 ft), and sets up
/// a laser bomb run in LVL: the A-G page and the GBU-10, the nearest enemy
/// ground object designated (Ctrl+X, Easy targeting for that press only),
/// which the sight then tracks, Ctrl+M for LVL and the laser on by hand
/// (Ctrl+L, twice: the first turns the automatic laser off by hand). Returns the world and the designated object.
fn laser_pass() -> (World, u32) {
    laser_pass_with(true)
}

/// [`laser_pass`], with the laser on by hand (`by_hand`) or left on its
/// automatic rule (no Ctrl+L).
fn laser_pass_with(by_hand: bool) -> (World, u32) {
    let mut world = world_at(5_000);
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
    world.combat.state.cheats.easy_targeting = true;
    tick(
        &mut world,
        &[],
        Some(seat),
        vec![SeatCommand::Combat(Combat::NearestGround)],
        Vec::new(),
        false,
    );
    world.combat.state.cheats.easy_targeting = false;
    let target = own(&world).designated().expect("Ctrl+X designates");
    fly(&mut world, Some(seat), 30);
    tick(
        &mut world,
        &[],
        Some(seat),
        if by_hand {
            vec![
                SeatCommand::Combat(Combat::ToggleSeekerMode),
                SeatCommand::Combat(Combat::ToggleLaser),
            ]
        } else {
            vec![SeatCommand::Combat(Combat::ToggleSeekerMode)]
        },
        Vec::new(),
        false,
    );
    // The laser was on by the automatic rule (a laser weapon selected):
    // Ctrl+L turned it off by hand, a second Ctrl+L puts it on by hand.
    if by_hand && own(&world).laser.manual != Some(true) {
        tick(
            &mut world,
            &[],
            Some(seat),
            vec![SeatCommand::Combat(Combat::ToggleLaser)],
            Vec::new(),
            false,
        );
    }
    fly(&mut world, Some(seat), 120);
    (world, target)
}

/// The rounds left on the selected station.
fn rounds(world: &World) -> u16 {
    let own = own(world);
    own.rounds(own.selected)
}

#[test]
fn the_ai_finishes_a_humans_laser_pass_and_the_human_finds_its_cockpit() {
    let (mut world, target) = laser_pass();
    let left = cockpit(&world);
    let left_rounds = rounds(&world);
    assert_eq!(left.delivery, DeliveryMode::Level, "{left:?}");
    assert_eq!(left.laser_manual, Some(true), "{left:?}");
    assert_eq!(left.sight, Some(SightMode::Tracked(target)), "{left:?}");
    assert_eq!(left.designated, Some(target));
    assert_eq!(left_rounds, 2);
    assert!(world.combat.state.guided_flights().next().is_none());
    // Given back mid-pass, some 8 nm out.
    tick(
        &mut world,
        &[MissionCommand::GiveBack { seat: SeatId(1) }],
        None,
        Vec::new(),
        Vec::new(),
        false,
    );
    let taken = strike(&world).expect("the AI strikes on");
    assert_eq!(
        taken.takeover().map(|t| (t.target, t.mode)),
        Some((target, DeliveryMode::Level))
    );
    // The AI flies the same pass: a level run at the same target with the
    // laser bomb, the mode and the laser hand setting untouched; the bomb
    // guides on its laser to the end.
    let mut bomb = None;
    let mut modes = Vec::new();
    let mut lasing_in_flight = true;
    for at in 0..120 * 120 {
        fly(&mut world, None, 1);
        let state = &world.combat.state;
        let kept = state.kept(PLANE.0).expect("the cockpit is kept");
        assert_eq!(kept.delivery_mode, DeliveryMode::Level, "tick {at}");
        assert_eq!(kept.laser.manual, Some(true), "tick {at}");
        assert_eq!(kept.selected, left.selected, "tick {at}");
        if let Some(s) = strike(&world)
            && let Some(pass) = s.pass()
        {
            assert_eq!(pass, tore_sim::ai::strike::Pass::Level, "tick {at}");
            assert_eq!(s.target(), Some(target), "tick {at}");
        }
        if bomb.is_none() {
            bomb = state
                .guided_flights()
                .find(|(_, f)| f.owner == PLANE.0)
                .map(|(id, _)| id);
        }
        let Some(id) = bomb else { continue };
        match state.guided_flight(id) {
            Some(flight) => {
                modes.push(flight.mode);
                lasing_in_flight &= state.laser_spots().iter().any(|s| s.owner == PLANE.0);
            }
            None => break,
        }
    }
    let bomb = bomb.expect("the AI released the laser bomb");
    assert!(world.combat.state.guided_flight(bomb).is_none(), "it ended");
    assert!(world.combat.state.guided_impact(PLANE.0).is_some());
    let locked = modes
        .iter()
        .position(|m| *m == GuidedMode::Guiding)
        .expect("the bomb guided");
    assert!(
        modes[locked..].iter().all(|m| *m == GuidedMode::Guiding),
        "{modes:?}"
    );
    assert!(lasing_in_flight, "the laser held the target until impact");
    // Taken back from another seat: the cockpit as left, but one bomb
    // fewer, and the designation on the AI's current target (the same
    // one, or none to follow once it is destroyed).
    let seat = SeatId(2);
    tick(
        &mut world,
        &[MissionCommand::Take { seat, plane: PLANE }],
        Some(seat),
        Vec::new(),
        Vec::new(),
        false,
    );
    let back = cockpit(&world);
    assert_eq!(rounds(&world), left_rounds - 1);
    let standing = world
        .combat
        .state
        .targets
        .iter()
        .any(|t| t.id == target && t.hp > 0);
    if standing {
        assert_eq!(back.designated, Some(target));
        assert_eq!(back.sight, left.sight);
    }
    let without = |c: Cockpit| Cockpit {
        designated: None,
        sight: c.sight.map(|_| SightMode::Free),
        ..c
    };
    assert_eq!(without(back), without(left));
}

/// John's decision 11 (ST-INT): the AI never changes the human's laser
/// settings. A human who left the laser on its automatic rule gives the
/// plane back mid laser pass: the AI's pass and hold lase through an
/// AI-side want in combat, the kept cockpit's hand setting stays automatic
/// at every tick, and the human taking the plane back finds it so.
#[test]
fn the_ai_lases_a_kept_cockpit_without_touching_the_humans_laser_setting() {
    let (mut world, target) = laser_pass_with(false);
    let left = cockpit(&world);
    assert_eq!(left.laser_manual, None, "{left:?}");
    assert_eq!(left.designated, Some(target));
    tick(
        &mut world,
        &[MissionCommand::GiveBack { seat: SeatId(1) }],
        None,
        Vec::new(),
        Vec::new(),
        false,
    );
    assert!(strike(&world).is_some(), "the AI strikes on");
    let mut bomb = None;
    let mut wanted = false;
    let mut lasing_in_flight = true;
    for at in 0..120 * 120 {
        fly(&mut world, None, 1);
        let state = &world.combat.state;
        let kept = state.kept(PLANE.0).expect("the cockpit is kept");
        assert_eq!(kept.laser.manual, None, "tick {at}");
        wanted |=
            strike(&world).and_then(tore_sim::ai::strike::Strike::laser_target) == Some(target);
        if bomb.is_none() {
            bomb = state
                .guided_flights()
                .find(|(_, f)| f.owner == PLANE.0)
                .map(|(id, _)| id);
        }
        let Some(id) = bomb else { continue };
        if state.guided_flight(id).is_none() {
            break;
        }
        lasing_in_flight &= state.laser_spots().iter().any(|s| s.owner == PLANE.0);
    }
    assert!(wanted, "the AI's tactics wanted the laser on the target");
    assert!(bomb.is_some(), "the AI released the laser bomb");
    assert!(lasing_in_flight, "the laser held the target until impact");
    // A few seconds of the hold after the impact: still untouched.
    for at in 0..5 * 120 {
        fly(&mut world, None, 1);
        let kept = world.combat.state.kept(PLANE.0).expect("kept");
        assert_eq!(kept.laser.manual, None, "hold tick {at}");
    }
    let seat = SeatId(2);
    tick(
        &mut world,
        &[MissionCommand::Take { seat, plane: PLANE }],
        Some(seat),
        Vec::new(),
        Vec::new(),
        false,
    );
    assert_eq!(
        cockpit(&world).laser_manual,
        None,
        "the human's own setting"
    );
}

/// A seat takes plane 0 and gives it straight back, and the kept cockpit
/// (holding nothing to attack) is dropped: from here the AI flies plane 0
/// as an aircraft no human ever flew.
fn ai_flown() -> World {
    let mut world = world_at(5_000);
    let seat = SeatId(1);
    tick(
        &mut world,
        &[MissionCommand::Take { seat, plane: PLANE }],
        Some(seat),
        Vec::new(),
        Vec::new(),
        false,
    );
    tick(
        &mut world,
        &[MissionCommand::GiveBack { seat }],
        None,
        Vec::new(),
        Vec::new(),
        false,
    );
    assert!(strike(&world).is_none(), "nothing to continue");
    world.combat.state.take_kept(PLANE.0);
    world
}

#[test]
fn a_human_taking_an_ai_striker_inherits_its_attack() {
    let mut world = ai_flown();
    let target =
        crate::ai_wings::outcome::ground_targets(&world.terrain.surface, &world.combat.state)[0];
    world
        .ai_wings
        .as_mut()
        .unwrap()
        .set_strike_order(PLANE.0, Some(target));
    fly(&mut world, None, 240);
    let selection = strike(&world)
        .and_then(tore_sim::ai::strike::Strike::selection)
        .expect("the AI strikes");
    assert_eq!(selection.target, target);
    let ai = world
        .combat
        .state
        .ai_cockpits()
        .find(|(id, _)| *id == PLANE.0)
        .map(|(_, ai)| ai.cockpit.clone())
        .expect("the striker's own cockpit");
    assert_eq!(ai.selected, usize::from(selection.station.0));
    assert!(ai.armed);
    assert_eq!(ai.laser.manual, Some(true), "the AI lases by hand");
    assert_eq!(ai.sight.map(|s| s.mode), Some(SightMode::Tracked(target)));
    // The first take: its target designated on the A-G page, its store,
    // modes, sight and laser.
    let seat = SeatId(2);
    tick(
        &mut world,
        &[MissionCommand::Take { seat, plane: PLANE }],
        Some(seat),
        Vec::new(),
        Vec::new(),
        false,
    );
    let own = own(&world);
    assert_eq!(own.designated(), Some(target));
    assert!(own.sensors.air_ground());
    assert_eq!(
        (own.selected, own.armed, own.delivery_mode, own.launch_mode),
        (ai.selected, true, ai.delivery_mode, ai.launch_mode)
    );
    assert_eq!(own.laser.manual, Some(true));
    assert_eq!(own.sight.map(|s| s.mode), Some(SightMode::Tracked(target)));
    assert!(world.combat.state.ai_cockpits().next().is_none(), "taken");
    // It holds a second later, by the usual rules.
    fly(&mut world, Some(seat), 120);
    assert_eq!(own_designated(&world), Some(target));
}

fn own_designated(world: &World) -> Option<u32> {
    own(world).designated()
}

/// The column's boat (an enemy the AI's picture lists, 8,000 ft from the
/// tank the human designates) armed as a missile site with a 30,000 ft
/// reach, which covers the column. Its missile has no rail loaded, so it
/// never fires: only the AI's picture of it changes.
fn arm_the_boat(world: &mut World) -> u32 {
    use crate::surface::fire::{Band, Kind, MountArc, WeaponArms};
    use tore_sim::ai::surface::{Arm, Profile, Timing};
    let boat = crate::surface::UnitId(0x5000_0001);
    let record = own(world).configuration().stations[1].weapon.clone();
    let wide = Band {
        min_range: 0.,
        max_range: 30_000.,
        min_altitude: -1e9,
        max_altitude: 1e9,
    };
    let weapon = WeaponArms {
        record,
        kind: Kind::Missile,
        tuning: None,
        mounts: vec![MountArc {
            index: 99,
            rest: [0.; 2],
            limit: [0.; 2],
            offset: [0.; 3],
        }],
        profile: Profile {
            timing: Timing::from_quarters(4, 4, 4, 20, 4, 1),
            arm: Arm::Missile {
                salvo: 1,
                gap: 30,
                spacing: 30,
            },
        },
        detection: wide,
        launch: wide,
        magazine: 0,
        battery: false,
        ground_fire: false,
    };
    let row = world
        .combat
        .state
        .targets
        .iter()
        .find(|t| t.id == boat.0)
        .expect("the boat")
        .clone();
    let arsenal = &mut world.terrain.surface.arsenal;
    let mut arms = arsenal.units[0].clone();
    arms.unit = boat;
    arms.position = row.position;
    arms.side = row.side;
    arms.weapons = vec![weapon];
    arms.radar = None;
    arms.battery = None;
    let at = arsenal.units.partition_point(|a| a.unit < boat);
    arsenal.units.insert(at, arms);
    world
        .combat
        .surface
        .unit_mut(boat)
        .expect("its state")
        .engagers
        .push(Default::default());
    boat.0
}

#[test]
fn an_unsafe_give_back_stands_off_then_resumes_the_pass_once_safe() {
    let (mut world, target) = laser_pass();
    let site = arm_the_boat(&mut world);
    let left_rounds = rounds(&world);
    tick(
        &mut world,
        &[MissionCommand::GiveBack { seat: SeatId(1) }],
        None,
        Vec::new(),
        Vec::new(),
        false,
    );
    // The SAM covers the target: once the AI's picture holds the site
    // (its first rebuild), it holds off, turning away to stand off outside
    // the SAM's reach, and keeps the human's settings meanwhile.
    let mut stood_off = None;
    for _ in 0..10 * 120 {
        fly(&mut world, None, 1);
        stood_off = strike(&world)
            .expect("the takeover holds")
            .takeover()
            .and_then(|t| t.stand_off_ft());
        if stood_off.is_some() {
            break;
        }
    }
    for at in 0..30 * 120 {
        fly(&mut world, None, 1);
        let s = strike(&world).expect("the takeover holds");
        assert!(
            s.takeover().and_then(|t| t.stand_off_ft()).is_some(),
            "standing off, tick {at}"
        );
    }
    let radius = stood_off.expect("it stands off");
    assert!(radius > 30_000., "{radius}");
    assert!(world.combat.state.guided_flights().next().is_none());
    let kept = world.combat.state.kept(PLANE.0).unwrap();
    assert_eq!(
        (kept.delivery_mode, kept.laser.manual),
        (DeliveryMode::Level, Some(true))
    );
    // The SAM falls: once the picture says so (a rebuild at most 6 s on),
    // it turns back and flies the pass, in LVL with the laser bomb.
    world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == site)
        .unwrap()
        .hp = 0;
    let mut released = false;
    for _ in 0..240 * 120 {
        fly(&mut world, None, 1);
        if world
            .combat
            .state
            .guided_flights()
            .any(|(_, f)| f.owner == PLANE.0)
        {
            released = true;
            break;
        }
    }
    assert!(released, "it resumed and released");
    let s = strike(&world).unwrap();
    assert_eq!(s.pass(), Some(tore_sim::ai::strike::Pass::Level));
    assert_eq!(s.target(), Some(target));
    let rounds_now = world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(PLANE.0)
        .unwrap()
        .stations()[1]
        .store
        .rounds;
    assert_eq!(
        rounds_now,
        tore_sim::ai::weapon_service::Rounds::Finite(u32::from(left_rounds) - 1)
    );
}

/// A checkpoint taken while the AI flies the human's pass carries the
/// takeover: the copy steps on to the same release, tick for tick.
#[test]
fn a_takeover_rides_a_checkpoint() {
    let (mut world, _) = laser_pass();
    tick(
        &mut world,
        &[MissionCommand::GiveBack { seat: SeatId(1) }],
        None,
        Vec::new(),
        Vec::new(),
        false,
    );
    fly(&mut world, None, 120);
    let bytes = world.checkpoint().unwrap();
    let mut copy = world_at(5_000);
    copy.restore(&bytes).unwrap();
    assert_eq!(strike(&copy), strike(&world));
    for _ in 0..100 * 120 {
        fly(&mut world, None, 1);
        fly(&mut copy, None, 1);
    }
    assert!(
        world.combat.state.guided_impact(PLANE.0).is_some(),
        "the pass ended"
    );
    assert_eq!(copy.checkpoint().unwrap(), world.checkpoint().unwrap());
}

/// ST-O's finding: a take and a give-back rebuild the AI actor. A Strike
/// wing's plane and a wingman's ground order come back with it, across a
/// checkpoint taken while the human flies.
#[test]
fn a_take_and_a_give_back_keep_the_planes_strike_role_and_ground_order() {
    use tore_sim::ai::engagement::StrikeRole;
    let mut world = world_at(5_000);
    let target =
        crate::ai_wings::outcome::ground_targets(&world.terrain.surface, &world.combat.state)[0];
    let wings = world.ai_wings.as_mut().unwrap();
    wings.set_strike_role(PLANE.0, StrikeRole::Strike);
    wings.set_strike_order(PLANE.0, Some(target));
    let seat = SeatId(1);
    tick(
        &mut world,
        &[MissionCommand::Take { seat, plane: PLANE }],
        Some(seat),
        Vec::new(),
        Vec::new(),
        false,
    );
    assert!(world.combat.state.ownship(PLANE.0).is_some());
    let bytes = world.checkpoint().unwrap();
    let mut copy = world_at(5_000);
    copy.restore(&bytes).unwrap();
    for w in [&mut world, &mut copy] {
        assert!(
            w.ai_wings
                .as_ref()
                .unwrap()
                .handed_tasking(PLANE.0)
                .is_some()
        );
        fly(w, Some(seat), 60);
        tick(
            w,
            &[MissionCommand::GiveBack { seat }],
            None,
            Vec::new(),
            Vec::new(),
            false,
        );
        let actor = w
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actor(PLANE.0)
            .unwrap();
        assert_eq!(actor.strike_role(), StrikeRole::Strike);
        assert_eq!(actor.strike_order(), Some(target));
        assert!(
            w.ai_wings
                .as_ref()
                .unwrap()
                .handed_tasking(PLANE.0)
                .is_none()
        );
    }
    assert_eq!(copy.checkpoint().unwrap(), world.checkpoint().unwrap());
}

/// ST-K's follow-up at the integration: a wingman's class order (Alt+W)
/// and lase order (Alt+G, ST-O) also come back with the plane after a take
/// and a give-back, across a checkpoint taken while the human flies.
#[test]
fn a_take_and_a_give_back_keep_the_planes_class_and_lase_orders() {
    let world = world_at(5_000);
    let targets =
        crate::ai_wings::outcome::ground_targets(&world.terrain.surface, &world.combat.state);
    assert!(targets.len() >= 2, "the template has two ground targets");
    let mut class = vec![targets[0], targets[1]];
    class.sort_unstable();
    let seat = SeatId(1);
    for order in ["class", "lase"] {
        let mut world = world_at(5_000);
        {
            let actor = world
                .ai_wings
                .as_mut()
                .unwrap()
                .mission_mut()
                .actor_mut(PLANE.0)
                .unwrap();
            match order {
                "class" => actor.order_strike_class(class.clone()),
                _ => actor.order_lase(targets[0]),
            }
        }
        tick(
            &mut world,
            &[MissionCommand::Take { seat, plane: PLANE }],
            Some(seat),
            Vec::new(),
            Vec::new(),
            false,
        );
        assert!(world.combat.state.ownship(PLANE.0).is_some());
        let kept = world
            .ai_wings
            .as_ref()
            .unwrap()
            .handed_tasking(PLANE.0)
            .cloned()
            .expect("the tasking is kept while the human flies");
        match order {
            "class" => assert_eq!((kept.class_order.clone(), kept.lase), (class.clone(), None)),
            _ => assert_eq!((kept.class_order.len(), kept.lase), (0, Some(targets[0]))),
        }
        let bytes = world.checkpoint().unwrap();
        let mut copy = world_at(5_000);
        copy.restore(&bytes).unwrap();
        for w in [&mut world, &mut copy] {
            fly(w, Some(seat), 60);
            tick(
                w,
                &[MissionCommand::GiveBack { seat }],
                None,
                Vec::new(),
                Vec::new(),
                false,
            );
            let actor = w
                .ai_wings
                .as_ref()
                .unwrap()
                .mission()
                .actor(PLANE.0)
                .unwrap();
            match order {
                "class" => {
                    assert_eq!(
                        actor.ground_picture().map(|g| g.class_order().to_vec()),
                        Some(class.clone())
                    );
                    assert_eq!(actor.lase_order(), None);
                }
                _ => {
                    assert_eq!(actor.lase_order(), Some(targets[0]));
                    // The order flies its duty again (slice ST-L), whose
                    // order onto the target stands with it.
                    let wings = w.ai_wings.as_ref().unwrap();
                    assert!(wings.can_lase(PLANE.0), "the fixture has a pod");
                    assert_eq!(wings.lasing_for(PLANE.0), Some(targets[0]));
                    assert_eq!(actor.strike_order(), Some(targets[0]));
                }
            }
        }
        assert_eq!(copy.checkpoint().unwrap(), world.checkpoint().unwrap());
    }
}
