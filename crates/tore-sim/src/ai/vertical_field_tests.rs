//! Slice H2: the vertical takeoffs and landings flown on the powered-lift
//! physics, by the six vertical types on the synthetic records the other
//! powered-lift tests use, at a small flat airfield: a ground-started wing
//! of three lifts off one by one and climbs away, and each type comes down
//! vertically from the last approach gate, sits, and goes to its parking
//! slot.
use super::tests::{object, setup};
use super::*;
use crate::ai::airfield::{
    self, GroundStart, LandingOrder, LandingReason, Lift, Phase, RunwayView, Sequence,
};
use crate::airport::ApproachEnd;
use crate::flight::powered::hover_hold_tests::aircraft;

const AIRPORT: u32 = 9;
const ROTORS: [AircraftId; 4] = [
    AircraftId::Ah64,
    AircraftId::Mi24,
    AircraftId::Ch47,
    AircraftId::V22,
];
const JETS: [AircraftId; 2] = [AircraftId::Av8, AircraftId::Yak141];

/// A 3,000 ft strip at sea level, heading north; parking without anchors
/// is then a few hundred feet up the strip from the landing point.
fn runway() -> RunwayView {
    RunwayView {
        airport: AIRPORT,
        object: 90,
        center: [0., 0., 0.],
        heading: 0.,
        length_ft: 3000.,
        elevation_ft: 0.,
        anchors: None,
    }
}

/// The strip and an apron beside it are the airfield; grass around.
fn surface(x: f64, z: f64) -> Surface {
    if x.abs() <= 400. && z.abs() <= 1500. {
        Surface::runway(0.)
    } else {
        Surface::terrain(0.)
    }
}

fn step(mission: &mut AiMission) -> MissionOutput {
    let world: Vec<WorldObject> = mission
        .actors()
        .iter()
        .map(|a| {
            let mut o = object(a, 1);
            o.on_ground = a.flight().research.as_ref().is_some_and(|r| r.on_ground);
            o
        })
        .collect();
    mission
        .step_with_surface(&world, &|x, z| surface(x, z).height, &surface, TimeOfDay(0))
        .unwrap()
}

fn on_ground(actor: &AiActor) -> bool {
    actor
        .flight()
        .research
        .as_ref()
        .is_some_and(|r| r.on_ground)
}

fn wheel_ft(actor: &AiActor) -> f64 {
    let f = actor.flight();
    f.position[1] - f.model().configuration().equipment.ground_clearance_ft
}

fn horizontal(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

fn trace(kind: AircraftId, tick: u64, actor: &AiActor) {
    if std::env::var("TRACE").is_ok() && tick.is_multiple_of(120) {
        let f = actor.flight();
        eprintln!(
            "{kind:?} t{:.0} {} {:?} pos [{:.0} {:.0} {:.0}] wheel {:.1} v [{:.1} {:.1} {:.1}] speed {:.0} hdg {:.0} pitch {:.1} bank {:.1} thr {:.2} coll {:.2} noz {:.0} crashed {}",
            tick as f64 / 120.,
            actor.id(),
            actor.airfield_phase(),
            f.position[0],
            f.position[1],
            f.position[2],
            wheel_ft(actor),
            f.velocity[0],
            f.velocity[1],
            f.velocity[2],
            f.speed,
            f.yaw.to_degrees(),
            f.pitch.to_degrees(),
            f.bank.to_degrees(),
            f.throttle,
            f.lift_controls.collective,
            f.nozzle_degrees(),
            f.crashed,
        );
    }
}

/// `kind` parked at `position`, its internal fuel cut to `fuel_share`.
fn parked_with(
    kind: AircraftId,
    id: u32,
    member: u8,
    position: [f64; 3],
    fuel_share: f64,
) -> AiActor {
    let mut setup = setup(id, 1, member, position, 0.);
    setup.flight = flight::State::new(&aircraft(kind), position).unwrap();
    setup.flight.fuel *= fuel_share;
    setup.home_airport = None;
    let mut actor = AiActor::new(setup).unwrap();
    actor.flight_mut().enable_research(id as i32).unwrap();
    actor.flight_mut().start_on_runway(position, 0.).unwrap();
    actor.start_on_ground(GroundStart {
        runway: runway(),
        end: ApproachEnd::Near,
        order: member,
    });
    actor
}

/// Retail's vertical takeoff (states 0xa..0xc), John 2026-10-12: a
/// ground-started wing of three of each vertical type takes off one by one,
/// the lead first. A helicopter lifts off where it stands; a jet light
/// enough to hover lines up on the runway first. Each holds 5 s, climbs
/// straight up (within 100 ft of where it left the ground) to 500 ft (a
/// helicopter) or 800 ft (a jet, which then accelerates), and flies on. No
/// two hold on the runway at once. A Yak-141 on full fuel needs 99% of its
/// thrust to hover, over the 90% limit, so it makes retail's short takeoff
/// instead; on half fuel it takes off vertically.
#[test]
fn a_ground_started_wing_of_three_of_each_vertical_type_takes_off_vertically() {
    let cases = ROTORS
        .into_iter()
        .chain(JETS)
        .map(|kind| (kind, 1.0))
        .chain([(AircraftId::Yak141, 0.5)]);
    for (kind, fuel) in cases {
        let mut mission = AiMission::new();
        for member in 0..3u8 {
            // A taxi queue beside the strip, 300 ft apart.
            let position = [150., 0., -1300. + 300. * f64::from(member)];
            mission.push(parked_with(
                kind,
                1 + u32::from(member),
                member,
                position,
                fuel,
            ));
        }
        mission.start_in_formation();
        let lift = mission.actors()[0].airfield().unwrap().lift();
        let expected = match kind {
            _ if ROTORS.contains(&kind) => Lift::Rotor,
            AircraftId::Yak141 if fuel > 0.9 => Lift::ShortJet,
            _ => Lift::Jet,
        };
        assert_eq!(lift, expected, "{kind:?} {fuel}");
        let mut liftoff = [None::<(u64, [f64; 3])>; 3];
        let mut nozzle_at_liftoff = [0f64; 3];
        let mut drift = [0f64; 3];
        let mut top = [0f64; 3];
        let mut done = [None::<u64>; 3];
        let mut climbed = [false; 3];
        for tick in 0..(200 * 120) {
            step(&mut mission);
            let holding = mission
                .actors()
                .iter()
                .filter(|a| {
                    matches!(
                        a.airfield_phase(),
                        Some(Phase::VerticalHold | Phase::TakeoffRoll)
                    )
                })
                .count();
            assert!(holding <= 1, "{kind:?}: two on the runway at {tick}");
            for (i, actor) in mission.actors().iter().enumerate() {
                trace(kind, tick, actor);
                assert!(!actor.flight().crashed, "{kind:?} {i} crashed at {tick}");
                if liftoff[i].is_none() && !on_ground(actor) {
                    liftoff[i] = Some((tick, actor.flight().position));
                    nozzle_at_liftoff[i] = actor.flight().nozzle_degrees();
                }
                if actor.airfield_phase() == Some(Phase::VerticalClimb) {
                    climbed[i] = true;
                    if let Some((_, from)) = liftoff[i] {
                        drift[i] = drift[i].max(horizontal(actor.flight().position, from));
                    }
                    top[i] = top[i].max(wheel_ft(actor));
                }
                if done[i].is_none() && liftoff[i].is_some() && actor.airfield().is_none() {
                    done[i] = Some(tick);
                }
            }
            if done.iter().all(Option::is_some) {
                break;
            }
        }
        for i in 0..3 {
            assert!(
                done[i].is_some(),
                "{kind:?} {i} never finished: {liftoff:?}"
            );
            if lift == Lift::ShortJet {
                // The nozzles came down before the wheels left.
                assert!(
                    nozzle_at_liftoff[i] > 30.,
                    "{kind:?} {i} nozzle {}",
                    nozzle_at_liftoff[i]
                );
                continue;
            }
            let climb_to = if lift == Lift::Rotor {
                airfield::ROTOR_CLIMB_AGL_FT
            } else {
                airfield::JET_CLIMB_AGL_FT
            };
            assert!(climbed[i], "{kind:?} {i} never climbed vertically");
            assert!(drift[i] < 100., "{kind:?} {i} drifted {} ft", drift[i]);
            assert!(
                top[i] >= climb_to - airfield::CLIMB_REACHED_FT - 1.,
                "{kind:?} {i} topped at {}",
                top[i]
            );
        }
        let ticks = liftoff.map(|l| l.unwrap().0);
        assert!(
            ticks[0] < ticks[1] && ticks[1] < ticks[2],
            "{kind:?} order {ticks:?}"
        );
    }
}

/// `kind` airborne at the last approach gate of [`runway`], heading north
/// toward the landing point, its landing begun in the hover approach.
fn at_last_gate(kind: AircraftId) -> AiActor {
    let order = LandingOrder {
        runway: runway(),
        reason: LandingReason::Ordered,
    };
    let probe = Sequence::landing(order, [0.; 3], [0.; 3], 20_000., None).with_lift(Lift::Rotor);
    let gate = probe.gate(3);
    let rotor = ROTORS.contains(&kind);
    let speed = if rotor { 140. } else { 260. };
    let mut setup = setup(1, 1, 0, gate, 0.);
    let mut flight = flight::State::new(&aircraft(kind), gate).unwrap();
    flight.enable_research(1).unwrap();
    // Home from a mission: half its fuel burned (a Yak-141 on full fuel
    // cannot hover).
    let fuel = flight.fuel * 0.5;
    if rotor {
        flight =
            crate::ai::rotor_probe::moving(&aircraft(kind), gate[1], speed / 1.687_81, [0.; 3])
                .unwrap();
        flight.position = gate;
    } else {
        flight.yaw = 0.;
        flight.speed = speed;
        flight.velocity = [0., 0., speed];
        flight.start_airborne([0.; 3]);
        flight.gear_down = true;
        flight.gear = 1.;
    }
    flight.fuel = fuel;
    setup.flight = flight;
    setup.home_airport = None;
    let mut actor = AiActor::new(setup).unwrap();
    let lift = actor.airfield_lift();
    assert!(lift.vertical(), "{kind:?} {lift:?}");
    actor.landing_order = Some(order);
    actor.airfield = Some(
        Sequence::landing(order, gate, [0.; 3], 20_000., None)
            .with_lift(lift)
            .at_phase(Phase::HoverApproach, 0.),
    );
    actor
}

/// Retail's vertical landing (state 0x18), John 2026-10-12 ("vertical for
/// all three VTOL types too"): from the last gate each type comes to a
/// hover over the landing point, comes straight down and touches down under
/// the shared touchdown rule's limits (10 ft/s, 20 kt; 10 kt for a jet),
/// sits 5 s, and a helicopter air taxis to its slot, sets down there and
/// shuts down, a jet taxis in on its wheels.
#[test]
fn each_vertical_type_lands_vertically_at_the_landing_point_and_parks() {
    for kind in ROTORS.into_iter().chain(JETS) {
        let mut mission = AiMission::new();
        mission.push(at_last_gate(kind));
        mission.start_in_formation();
        let rotor = ROTORS.contains(&kind);
        let mut seen: Vec<Phase> = Vec::new();
        let mut first_contact: Option<(f64, f64, [f64; 3])> = None;
        let mut previous = (0., 0.);
        for tick in 0..(400 * 120) {
            step(&mut mission);
            let actor = &mission.actors()[0];
            trace(kind, tick, actor);
            assert!(
                !actor.flight().crashed,
                "{kind:?} crashed at {tick}: {seen:?}"
            );
            if let Some(phase) = actor.airfield_phase()
                && seen.last() != Some(&phase)
            {
                seen.push(phase);
            }
            if first_contact.is_none() && on_ground(actor) {
                first_contact = Some((previous.0, previous.1, actor.flight().position));
            }
            let f = actor.flight();
            previous = (
                -f.velocity[1],
                f.velocity[0].hypot(f.velocity[2]) / 1.687_81,
            );
            if actor.airfield_phase() == Some(Phase::Parked) {
                break;
            }
        }
        let want: &[Phase] = if rotor {
            &[
                Phase::HoverApproach,
                Phase::VerticalDescent,
                Phase::Settle,
                Phase::AirTaxi,
                Phase::VerticalDescent,
                Phase::Parked,
            ]
        } else {
            &[
                Phase::HoverApproach,
                Phase::VerticalDescent,
                Phase::Settle,
                Phase::TaxiClear,
                Phase::Parked,
            ]
        };
        assert_eq!(seen, want, "{kind:?}");
        let (sink, knots, at) = first_contact.unwrap();
        assert!(
            sink <= crate::touchdown::MAX_SINK_FPS,
            "{kind:?} sink {sink}"
        );
        let limit = if rotor {
            crate::touchdown::ROTOR_MAX_GROUND_KNOTS
        } else {
            crate::touchdown::HOVER_MAX_GROUND_KNOTS
        };
        assert!(knots <= limit, "{kind:?} {knots} kt");
        let actor = &mission.actors()[0];
        let landing = actor.airfield().unwrap().landing_point();
        assert!(horizontal(at, landing) < 60., "{kind:?} touched {at:?}");
        if rotor {
            let f = actor.flight();
            assert!(
                f.lift_controls.collective < 0.05,
                "{kind:?} collective down"
            );
            assert_eq!(actor.activity(), Activity::Landed);
        }
    }
}
