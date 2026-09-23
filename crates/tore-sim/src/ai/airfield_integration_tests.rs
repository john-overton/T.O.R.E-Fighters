//! Headless takeoff and landing sequences on a synthetic flat runway, flown
//! by the F/A-18D model with synthetic record values.
use super::tests::{object, setup, synthetic_profile};
use super::*;
use crate::ai::airfield::{self, GroundStart, LandingOrder, LandingReason, Phase, RunwayView};
use crate::ai::targeting::Side;
use crate::ai::wing::{Formation, ReceiverOutcome, RejectReason, WingRequest};
use crate::airport::ApproachEnd;

const AIRPORT: u32 = 7;
const HUMAN: u32 = 10;

fn runway() -> RunwayView {
    RunwayView {
        airport: AIRPORT,
        object: 70,
        center: [0., 0., 0.],
        heading: 0.,
        length_ft: 8000.,
        elevation_ft: 0.,
        anchors: None,
    }
}

/// Landable within the runway rectangle, flat terrain elsewhere.
fn surface(x: f64, z: f64) -> Surface {
    if x.abs() <= 100. && z.abs() <= 4000. {
        Surface::runway(0.)
    } else {
        Surface::terrain(0.)
    }
}

fn hornet(id: u32, member: u8, position: [f64; 3], yaw: f64) -> AiActor {
    let mut setup = setup(id, 1, member, position, yaw);
    let mut flight = flight::State::new(&synthetic_profile(AircraftId::F18), position).unwrap();
    assert!(matches!(
        flight.model(),
        crate::models::AircraftModel::F18(_)
    ));
    flight.yaw = yaw;
    flight.velocity = crate::attitude::Basis::new(yaw, 0., 0.)
        .forward
        .map(|v| v * flight.speed);
    setup.flight = flight;
    setup.home_airport = None;
    AiActor::new(setup).unwrap()
}

fn parked(id: u32, order: u8, position: [f64; 3]) -> AiActor {
    let mut actor = hornet(id, order, position, 0.);
    actor.flight_mut().enable_research(id as i32).unwrap();
    actor.flight_mut().start_on_runway(position, 0.).unwrap();
    actor.start_on_ground(GroundStart {
        runway: runway(),
        end: ApproachEnd::Near,
        order,
    });
    actor
}

fn world(mission: &AiMission, extra: Option<WorldObject>) -> Vec<WorldObject> {
    mission
        .actors()
        .iter()
        .map(|a| {
            let mut o = object(a, 1);
            o.on_ground = a.flight().research.as_ref().is_some_and(|r| r.on_ground);
            o
        })
        .chain(extra)
        .collect()
}

fn step(mission: &mut AiMission, extra: Option<WorldObject>) -> MissionOutput {
    let world = world(mission, extra);
    mission
        .step_with_surface(&world, &|x, z| surface(x, z).height, &surface, TimeOfDay(0))
        .unwrap()
}

/// A scripted human leader: parked until `release_s`, then a takeoff roll
/// at 8 ft/s^2 with liftoff at 250 ft/s and a 20 ft/s climb.
fn human(template: &AiActor, t: f64, release_s: f64) -> WorldObject {
    let mut o = object(template, 1);
    o.id = HUMAN;
    o.human_controlled = true;
    let rolling = (t - release_s).max(0.);
    let speed = (8. * rolling).min(400.);
    let liftoff = 250. / 8.;
    let along = if rolling < liftoff {
        4. * rolling * rolling
    } else {
        4. * liftoff * liftoff + 250. * (rolling - liftoff) + 0.5 * 8. * (rolling - liftoff).powi(2)
    };
    let climb = (rolling - liftoff).max(0.) * 20.;
    o.position = [0., climb, -3000. + along];
    o.velocity = [0., if climb > 0. { 20. } else { 0. }, speed];
    o.speed = ScalarSpeed(speed);
    o.heading_deg = 0.;
    o.on_ground = climb <= 0.;
    o
}

fn crashed(mission: &AiMission) -> bool {
    mission.actors().iter().any(|a| a.flight().crashed)
}

fn on_ground(actor: &AiActor) -> bool {
    actor
        .flight()
        .research
        .as_ref()
        .is_some_and(|r| r.on_ground)
}

#[test]
fn ground_started_wingmen_wait_for_the_human_leader_then_depart_in_order() {
    let mut mission = AiMission::new();
    mission.push(parked(1, 1, [40., 0., -3250.]));
    mission.push(parked(2, 2, [-40., 0., -3500.]));
    mission.set_external_leader(Side(1), 0, HUMAN);
    mission.start_in_formation();
    let starts: Vec<_> = mission
        .actors()
        .iter()
        .map(|a| a.flight().position)
        .collect();
    let template = hornet(99, 0, [0.; 3], 0.);
    let release_s = 30.;
    let mut roll_start = [None::<u64>; 2];
    let mut liftoff = [None::<u64>; 2];
    let mut completed = [None::<u64>; 2];
    // Bug out is ignored on the ground.
    assert_eq!(
        mission
            .order(2, land(LandingReason::BugOut))
            .unwrap()
            .unwrap(),
        ReceiverOutcome::Rejected(RejectReason::OnAirfield)
    );
    for tick in 0..(240 * 120) {
        let t = tick as f64 / 120.;
        let out = step(&mut mission, Some(human(&template, t, release_s)));
        assert!(!crashed(&mission), "crash at {t:.1}s");
        for (i, actor) in mission.actors().iter().enumerate() {
            if t < release_s {
                // The leader is parked: wingmen hold their spots.
                let moved = distance(actor.flight().position, starts[i]);
                assert!(moved < 1., "wingman {i} moved {moved} ft at {t:.1}s");
                assert_eq!(actor.activity(), Activity::Waiting);
            }
            trace(tick, actor);
            if roll_start[i].is_none() && actor.airfield_phase() == Some(Phase::TakeoffRoll) {
                roll_start[i] = Some(tick);
            }
            if liftoff[i].is_none() && !on_ground(actor) {
                liftoff[i] = Some(tick);
            }
            if completed[i].is_none() && liftoff[i].is_some() && actor.airfield_phase().is_none() {
                completed[i] = Some(tick);
            }
        }
        assert!(out.activities.len() >= 2);
    }
    let [r1, r2] = roll_start.map(|t| t.expect("each wingman started its roll"));
    let [l1, l2] = liftoff.map(|t| t.expect("each wingman lifted off"));
    assert!(
        completed.iter().all(Option::is_some),
        "each wingman finished its climb-out"
    );
    // One at a time, leader first: wingman 1 waits for the human leader to
    // be airborne, wingman 2 for the runway to be free again.
    let human_airborne = (release_s + 250. / 8.) * 120.;
    assert!(r1 as f64 > human_airborne, "wingman 1 rolled too early");
    assert!(r2 > l1, "wingman 2 rolled before wingman 1 was airborne");
    assert!(l2 > l1);
    for actor in mission.actors() {
        let f = actor.flight();
        assert!(
            f.position[1] > 1000.,
            "{} too low: {:?}",
            actor.id(),
            f.position
        );
        assert!(f.gear < 0.01 && f.flaps < 0.01, "gear and flaps up");
        assert!(
            matches!(actor.activity(), Activity::Formation | Activity::Rejoining),
            "{:?}",
            actor.activity()
        );
    }
}

fn trace(tick: u64, actor: &AiActor) {
    if std::env::var("TRACE").is_ok() && tick.is_multiple_of(240) {
        let f = actor.flight();
        eprintln!(
            "t{:.0} {} {:?} {:?} pos [{:.0} {:.0} {:.0}] v{:.0} vy{:.1} hdg{:.1} pitch{:.1} bank{:.1} gear{:.1} flaps{:.1} thr{:.2}",
            tick as f64 / 120.,
            actor.id(),
            actor.airfield_phase(),
            actor.activity(),
            f.position[0],
            f.position[1],
            f.position[2],
            f.speed,
            f.velocity[1],
            f.yaw.to_degrees(),
            f.pitch.to_degrees(),
            f.bank.to_degrees(),
            f.gear,
            f.flaps,
            f.throttle
        );
    }
}

/// Run until the actor parks, returning the tick, or panic after `limit_s`.
fn fly_until_parked(mission: &mut AiMission, id: u32, limit_s: u64) -> u64 {
    for tick in 0..limit_s * 120 {
        step(mission, None);
        let actor = mission.actor(id).unwrap();
        trace(tick, actor);
        assert!(
            !actor.flight().crashed,
            "crashed at {:.1}s",
            tick as f64 / 120.
        );
        if actor.airfield_phase() == Some(Phase::Parked) {
            return tick;
        }
    }
    let actor = mission.actor(id).unwrap();
    panic!(
        "not parked after {limit_s}s: {:?} at {:?}",
        actor.airfield_phase(),
        actor.flight().position
    );
}

fn assert_parked_on_runway(actor: &AiActor) {
    let f = actor.flight();
    assert!(on_ground(actor), "on the ground");
    assert!(f.speed < 1., "stopped: {}", f.speed);
    assert!(
        surface(f.position[0], f.position[2]).landable,
        "on the runway: {:?}",
        f.position
    );
    assert!(f.gear > 0.99);
    assert_eq!(actor.activity(), Activity::Landed);
}

fn land(reason: LandingReason) -> WingRequest {
    WingRequest::Land(LandingOrder {
        runway: runway(),
        reason,
    })
}

#[test]
fn an_airborne_wingman_ordered_to_land_flies_the_approach_and_parks() {
    let mut mission = AiMission::new();
    // Legacy flight model, 15 nm south and 90 degrees off the final course.
    mission.push(hornet(
        1,
        1,
        [30_000., 6_000., -90_000.],
        -std::f64::consts::FRAC_PI_2,
    ));
    assert!(mission.actor(1).unwrap().flight().research.is_none());
    let outcome = mission
        .order(1, land(LandingReason::Ordered))
        .unwrap()
        .unwrap();
    assert_eq!(outcome, ReceiverOutcome::AppliedNoMotion);
    step(&mut mission, None);
    let actor = mission.actor(1).unwrap();
    // The legacy actor switched to the researched model in flight, airborne.
    let research = actor.flight().research.as_ref().expect("researched model");
    assert!(!research.on_ground && !actor.flight().crashed);
    assert!(actor.flight().position[1] > 5_900.);
    assert_eq!(actor.airfield_phase(), Some(Phase::Inbound));
    fly_until_parked(&mut mission, 1, 1500);
    let actor = mission.actor(1).unwrap();
    assert_parked_on_runway(actor);
    // Touchdown and rollout toward the far end of the chosen runway end.
    assert!(actor.flight().position[2] > 0.);
    // Parked aircraft stay parked and refuse orders.
    for _ in 0..600 {
        step(&mut mission, None);
    }
    assert_parked_on_runway(mission.actor(1).unwrap());
    assert_eq!(
        mission
            .order(1, WingRequest::FormationSelection(Formation::Echelon))
            .unwrap()
            .unwrap(),
        ReceiverOutcome::Rejected(RejectReason::Landed)
    );
}

#[test]
fn player_landing_priority_holds_ai_at_marshal_until_cleared() {
    let mut mission = AiMission::new();
    mission.push(hornet(1, 1, [0., 4_000., -100_000.], 0.));
    mission.set_priority_landing(Some(AIRPORT));
    mission.set_priority_landing(Some(AIRPORT));
    mission
        .order(1, land(LandingReason::Ordered))
        .unwrap()
        .unwrap();
    let mut marshal_since = None;
    for tick in 0..600 * 120 {
        step(&mut mission, None);
        let actor = mission.actor(1).unwrap();
        trace(tick, actor);
        assert!(!actor.flight().crashed);
        assert_ne!(
            actor.airfield_phase(),
            Some(Phase::Final),
            "final while held"
        );
        if actor.airfield_phase() == Some(Phase::Marshal) {
            let since = *marshal_since.get_or_insert(tick);
            assert_eq!(actor.activity(), Activity::HoldingMarshal);
            if tick - since > 120 * 120 {
                break;
            }
            if tick - since > 60 * 120 {
                // Holding in the marshal square at 6000 ft plus 1000 ft for
                // wing position 1.
                let f = actor.flight();
                let off = f.position[0].hypot(f.position[2] + 2_000.);
                assert!(off < 90_000., "{off} ft from the landing point");
                assert!((f.position[1] - 7_000.).abs() < 1_000., "{:?}", f.position);
            }
        }
    }
    assert!(marshal_since.is_some(), "never reached marshal");
    mission.set_priority_landing(None);
    fly_until_parked(&mut mission, 1, 1500);
    assert_parked_on_runway(mission.actor(1).unwrap());
}

#[test]
fn a_bugged_out_wingman_leaves_the_wing_ignores_orders_and_lands() {
    let mut mission = AiMission::new();
    mission.push(hornet(1, 1, [0., 8_000., -120_000.], 0.));
    mission.set_external_leader(Side(1), 0, HUMAN);
    mission.start_in_formation();
    let lead = |mission: &AiMission, t: f64| {
        let mut o = object(mission.actor(1).unwrap(), 1);
        o.id = HUMAN;
        o.human_controlled = true;
        o.position = [0., 8_000., -119_000. - 700. * t];
        o.velocity = [0., 0., -700.];
        o.heading_deg = 180.;
        o
    };
    for tick in 0..10 * 120 {
        let leader = lead(&mission, tick as f64 / 120.);
        step(&mut mission, Some(leader));
    }
    assert!(
        mission
            .order(1, land(LandingReason::BugOut))
            .unwrap()
            .is_ok()
    );
    let actor = mission.actor(1).unwrap();
    assert!(actor.bugged_out());
    assert_eq!(actor.landing_order().unwrap().reason, LandingReason::BugOut);
    for request in [
        WingRequest::FormationSelection(Formation::Echelon),
        WingRequest::TargetAssignment(crate::ai::wing::TargetOrder::HoldFire),
        land(LandingReason::Ordered),
    ] {
        assert_eq!(
            mission.order(1, request).unwrap().unwrap(),
            ReceiverOutcome::Rejected(RejectReason::BuggedOut)
        );
    }
    let start = 10 * 120;
    for tick in start..start + 60 * 120 {
        let leader = lead(&mission, tick as f64 / 120.);
        step(&mut mission, Some(leader));
        let actor = mission.actor(1).unwrap();
        assert!(!matches!(
            actor.activity(),
            Activity::Formation | Activity::Rejoining
        ));
    }
    let actor = mission.actor(1).unwrap();
    assert!(actor.bugged_out());
    assert!(
        actor.flight().position[2] > -110_000.,
        "heading home, not following the leader south: {:?}",
        actor.flight().position
    );
    fly_until_parked(&mut mission, 1, 1500);
    assert_parked_on_runway(mission.actor(1).unwrap());
    assert!(mission.actor(1).unwrap().bugged_out());
}

#[test]
fn bingo_fuel_with_a_home_runway_lands_there() {
    let mut mission = AiMission::new();
    let mut actor = hornet(1, 0, [20_000., 9_000., 60_000.], std::f64::consts::PI);
    actor.set_home_runway(Some(runway()));
    // Endurance well under time-to-home plus five minutes: bingo.
    actor.set_internal_fuel(300.);
    mission.push(actor);
    step(&mut mission, None);
    assert_eq!(
        mission.actor(1).unwrap().landing_order().map(|o| o.reason),
        Some(LandingReason::Fuel)
    );
    fly_until_parked(&mut mission, 1, 1500);
    let actor = mission.actor(1).unwrap();
    assert_parked_on_runway(actor);
    // Arriving from the north with no wind, it lands toward the south.
    assert!(
        actor.flight().position[2] < 0.,
        "{:?}",
        actor.flight().position
    );
}

#[test]
fn ai_wingmen_join_their_leaders_landing_and_wait_their_turn() {
    let mut mission = AiMission::new();
    mission.push(hornet(1, 0, [0., 6_000., -110_000.], 0.));
    mission.push(hornet(2, 1, [-600., 6_000., -110_600.], 0.));
    mission.start_in_formation();
    mission
        .order(1, land(LandingReason::Ordered))
        .unwrap()
        .unwrap();
    let mut joined = None;
    let mut wingman_final = None;
    let mut leader_rolled_out = None;
    for tick in 0..1_200 * 120 {
        step(&mut mission, None);
        assert!(!crashed(&mission), "crash at {:.0}s", tick as f64 / 120.);
        let (leader, wingman) = (mission.actor(1).unwrap(), mission.actor(2).unwrap());
        trace(tick, leader);
        trace(tick, wingman);
        if joined.is_none() && wingman.landing_order().is_some() {
            assert_eq!(
                wingman.landing_order().unwrap().reason,
                LandingReason::JoinLeader
            );
            joined = Some(tick);
        }
        if leader_rolled_out.is_none()
            && matches!(
                leader.airfield_phase(),
                Some(Phase::TaxiClear | Phase::Parked)
            )
        {
            leader_rolled_out = Some(tick);
        }
        if wingman_final.is_none() && wingman.airfield_phase() == Some(Phase::Final) {
            wingman_final = Some(tick);
        }
        if leader.airfield_phase() == Some(Phase::Parked)
            && wingman.airfield_phase() == Some(Phase::Parked)
        {
            break;
        }
    }
    assert!(joined.is_some(), "wingman never joined the landing");
    // One lander at a time: the wingman starts final after the leader's rollout.
    assert!(wingman_final.unwrap() >= leader_rolled_out.unwrap());
    let (leader, wingman) = (mission.actor(1).unwrap(), mission.actor(2).unwrap());
    assert_parked_on_runway(leader);
    assert_parked_on_runway(wingman);
    assert!(distance(leader.flight().position, wingman.flight().position) > 100.);
}

#[test]
fn a_formation_order_cancels_an_ordered_landing_in_the_early_approach() {
    let mut mission = AiMission::new();
    mission.push(hornet(1, 1, [0., 6_000., -110_000.], 0.));
    mission
        .order(1, land(LandingReason::Ordered))
        .unwrap()
        .unwrap();
    for _ in 0..120 {
        step(&mut mission, None);
    }
    assert_eq!(
        mission.actor(1).unwrap().airfield_phase(),
        Some(Phase::Inbound)
    );
    mission
        .order(1, WingRequest::FormationSelection(Formation::Echelon))
        .unwrap()
        .unwrap();
    let actor = mission.actor(1).unwrap();
    assert!(actor.landing_order().is_none() && actor.airfield_phase().is_none());
    step(&mut mission, None);
    assert!(mission.actor(1).unwrap().airfield_phase().is_none());
}

/// A synthetic airport layout in the retail anchor roles: parking to the
/// east, a parallel taxiway, the takeoff spot near the south threshold and a
/// taxi-back route from the rollout end.
fn anchored_runway() -> RunwayView {
    let at = |x: f64, z: f64| [x, 0., z];
    RunwayView {
        anchors: Some(airfield::AirfieldAnchors {
            taxi_out: [
                at(1_200., -1_000.),
                at(600., -1_000.),
                at(600., -4_100.),
                at(0., -4_070.),
            ],
            takeoff_spot: at(0., -3_700.),
            takeoff_heading: 0.,
            landing_point: at(0., -2_000.),
            landing_heading: 0.,
            taxi_in: [
                at(0., 1_000.),
                at(600., 1_300.),
                at(600., 0.),
                at(1_200., -500.),
            ],
            parking: std::array::from_fn(|k| at(1_500., -2_000. + 250. * k as f64)),
            parking_heading: std::f64::consts::FRAC_PI_2,
        }),
        ..runway()
    }
}

#[test]
fn anchored_airport_wingmen_taxi_out_from_parking_and_depart_in_order() {
    let runway = anchored_runway();
    let anchors = runway.anchors.unwrap();
    let mut mission = AiMission::new();
    for (id, order) in [(1, 1u8), (2, 2u8)] {
        let slot = anchors.parking[usize::from(order - 1)];
        let mut actor = hornet(id, order, slot, anchors.parking_heading);
        actor.flight_mut().enable_research(id as i32).unwrap();
        actor
            .flight_mut()
            .start_on_runway(slot, anchors.parking_heading)
            .unwrap();
        actor.start_on_ground(GroundStart {
            runway,
            end: ApproachEnd::Near,
            order,
        });
        mission.push(actor);
    }
    mission.set_external_leader(Side(1), 0, HUMAN);
    mission.start_in_formation();
    let template = hornet(99, 0, [0.; 3], 0.);
    // The scripted human sits on the takeoff spot, 700 ft further on than
    // the fallback test's spot.
    let human_at = |t: f64| {
        let mut o = human(&template, t, 30.);
        o.position[2] -= 700.;
        o
    };
    let mut liftoff = [None::<u64>; 2];
    let mut left_parking = [None::<u64>; 2];
    let mut taxi_legs = [0usize; 2];
    for tick in 0..(400 * 120) {
        let t = tick as f64 / 120.;
        step(&mut mission, Some(human_at(t)));
        assert!(!crashed(&mission), "crash at {t:.1}s");
        for (i, actor) in mission.actors().iter().enumerate() {
            trace(tick, actor);
            if actor.airfield_phase() == Some(Phase::Taxi) {
                left_parking[i].get_or_insert(tick);
                taxi_legs[i] =
                    taxi_legs[i].max(mission.actors()[i].airfield.as_ref().map_or(0, |s| s.leg()));
            }
            if liftoff[i].is_none() && !on_ground(actor) {
                liftoff[i] = Some(tick);
            }
        }
        if mission
            .actors()
            .iter()
            .all(|a| a.airfield_phase().is_none())
        {
            break;
        }
    }
    let [p1, p2] = left_parking.map(|t| t.expect("each wingman taxied out"));
    let [l1, l2] = liftoff.map(|t| t.expect("each wingman lifted off"));
    // Wingman 1 leaves parking once the human is airborne; wingman 2 once
    // wingman 1 is past its first taxiway leg.
    assert!(
        p1 as f64 / 120. > 30. + 250. / 8.,
        "wingman 1 left too early"
    );
    assert!(p2 > p1);
    assert!(l2 > l1);
    assert!(taxi_legs.iter().all(|legs| *legs >= 2), "{taxi_legs:?}");
    for actor in mission.actors() {
        assert!(
            actor.airfield_phase().is_none(),
            "{:?}",
            actor.airfield_phase()
        );
        assert!(actor.flight().position[1] > 600.);
    }
}

#[test]
fn anchored_airport_landing_taxis_back_to_a_parking_slot() {
    let runway = anchored_runway();
    let anchors = runway.anchors.unwrap();
    let mut mission = AiMission::new();
    mission.push(hornet(1, 1, [-20_000., 7_000., -80_000.], 0.3));
    mission
        .order(
            1,
            WingRequest::Land(LandingOrder {
                runway,
                reason: LandingReason::Ordered,
            }),
        )
        .unwrap()
        .unwrap();
    fly_until_parked(&mut mission, 1, 1500);
    let actor = mission.actor(1).unwrap();
    let f = actor.flight();
    assert!(on_ground(actor) && f.speed < 1.);
    assert_eq!(actor.airfield.as_ref().unwrap().slot(), Some(0));
    let slot = anchors.parking[0];
    let off = (f.position[0] - slot[0]).hypot(f.position[2] - slot[2]);
    assert!(
        off <= airfield::PARK_THROTTLE_OFF_FT + 50.,
        "{off} ft from slot 0"
    );
}

fn warning() -> super::super::controller::ThreatReport {
    super::super::controller::ThreatReport {
        missile_id: 500,
        seeker: SeekerClass::Radar,
        launcher_id: 77,
        launcher_same_side: false,
        distance_at_launch_ft: 20_000.,
        launch_tick: 0,
    }
}

#[test]
fn warnings_are_ignored_on_the_ground_and_abandon_an_early_approach() {
    // B47: ignored while taking off.
    let mut mission = AiMission::new();
    mission.push(parked(1, 1, [40., 0., -3250.]));
    mission.set_external_leader(Side(1), 0, HUMAN);
    let template = hornet(99, 0, [0.; 3], 0.);
    mission.actor_mut(1).unwrap().report_threat(warning());
    step(&mut mission, Some(human(&template, 0., 1e9)));
    let actor = mission.actor(1).unwrap();
    assert_eq!(actor.airfield_phase(), Some(Phase::Waiting));
    assert!(actor.pending_threats.is_empty());

    // B47: abandoned in the first approach states, resumed afterwards.
    let mut mission = AiMission::new();
    mission.push(hornet(1, 1, [0., 6_000., -50_000.], 0.));
    mission.set_priority_landing(Some(AIRPORT));
    mission
        .order(1, land(LandingReason::Ordered))
        .unwrap()
        .unwrap();
    for _ in 0..240 {
        step(&mut mission, None);
    }
    assert_eq!(
        mission.actor(1).unwrap().airfield_phase(),
        Some(Phase::Marshal)
    );
    mission.actor_mut(1).unwrap().report_threat(warning());
    step(&mut mission, None);
    let actor = mission.actor(1).unwrap();
    assert_eq!(actor.airfield_phase(), None);
    assert!(
        actor.landing_order().is_some(),
        "the landing is kept for later"
    );
    for _ in 0..240 {
        step(&mut mission, None);
    }
    assert!(
        mission.actor(1).unwrap().airfield_phase().is_some(),
        "landing resumed"
    );
}

/// Regression (review, 2026-09-23): a landing cancelled on the approach gates
/// used to leave the gear and flaps down and the speedbrake open in free
/// flight. Leaving a landing now raises and closes them.
#[test]
fn cancelling_a_landing_on_the_gates_cleans_the_aircraft_up() {
    let mut mission = AiMission::new();
    mission.push(hornet(1, 1, [0., 6_000., -60_000.], 0.));
    mission
        .order(1, land(LandingReason::Ordered))
        .unwrap()
        .unwrap();
    let mut on_gates = false;
    for _ in 0..300 * 120 {
        step(&mut mission, None);
        let actor = mission.actor(1).unwrap();
        assert!(!actor.flight().crashed);
        if actor.airfield_phase() == Some(Phase::Approach) && actor.flight().gear_down {
            on_gates = true;
            break;
        }
    }
    assert!(on_gates, "never reached the approach gates gear down");
    mission
        .order(1, WingRequest::FormationSelection(Formation::Echelon))
        .unwrap()
        .unwrap();
    let f = mission.actor(1).unwrap().flight();
    assert!(
        !f.gear_down && !f.flaps_down && !f.brake_out,
        "{:?}",
        (f.gear_down, f.flaps_down, f.brake_out)
    );
    step(&mut mission, None);
    assert!(mission.actor(1).unwrap().airfield_phase().is_none());
}

/// Regression (review, 2026-09-23): cancelling a wingman's join of its
/// leader's landing did not stick; the join was re-issued the next tick.
#[test]
fn a_cancelled_join_stays_cancelled_while_the_leader_lands() {
    let mut mission = AiMission::new();
    mission.push(hornet(1, 0, [0., 6_000., -60_000.], 0.));
    mission.push(hornet(2, 1, [-600., 6_000., -60_600.], 0.));
    mission.start_in_formation();
    mission
        .order(1, land(LandingReason::Ordered))
        .unwrap()
        .unwrap();
    let mut joined = false;
    for _ in 0..60 * 120 {
        step(&mut mission, None);
        if mission.actor(2).unwrap().landing_order().is_some() {
            joined = true;
            break;
        }
    }
    assert!(joined, "wingman never joined the landing");
    mission
        .order(2, WingRequest::FormationSelection(Formation::Echelon))
        .unwrap()
        .unwrap();
    for _ in 0..30 * 120 {
        step(&mut mission, None);
        let wingman = mission.actor(2).unwrap();
        assert!(
            wingman.landing_order().is_none() && wingman.airfield_phase().is_none(),
            "the cancelled join came back"
        );
    }
}

/// Regression (review, 2026-09-23): legacy-adapter aircraft read the runway
/// plane as their ground inside an airport box. They keep the terrain height.
#[test]
fn legacy_aircraft_keep_the_terrain_height_over_a_runway() {
    let mut mission = AiMission::new();
    mission.push(hornet(1, 1, [0., 3_000., -1_000.], 0.));
    assert!(mission.actor(1).unwrap().flight().research.is_none());
    let world = world(&mission, None);
    // A raised runway plane under a terrain of 0 ft.
    let raised = |x: f64, z: f64| {
        if x.abs() <= 100. && z.abs() <= 4000. {
            Surface::runway(2_990.)
        } else {
            Surface::terrain(0.)
        }
    };
    mission
        .step_with_surface(&world, &|_, _| 0., &raised, TimeOfDay(0))
        .unwrap();
    let actor = mission.actor(1).unwrap();
    assert!(!actor.flight().crashed && actor.flight().escape.is_none());
    assert!(actor.flight().position[1] > 2_900.);
}

/// Fly an ordered landing until final below `below_ft`, then put the
/// aircraft `height_ft` above the runway at `z` on the centerline, descending
/// at `path_deg`: a low, sinking final that the recovery estimate calls a
/// dive, with `damage` applied. The synthetic runway is paved from z -4,000
/// to 4,000 ft.
fn steep_final(below_ft: f64, height_ft: f64, path_deg: f64, z: f64, damage: f64) -> AiMission {
    // The synthetic record has no ejection seat; give it one (flags 0x10).
    let position = [0., 6_000., -60_000.];
    let mut profile = synthetic_profile(AircraftId::F18);
    let flags = profile.fields.get_mut("flags").unwrap();
    flags.value = (flags.number().unwrap() as i64 | 0x10).to_string();
    let mut setup = setup(1, 1, 1, position, 0.);
    let mut flight = flight::State::new(&profile, position).unwrap();
    flight.velocity = [0., 0., flight.speed];
    setup.flight = flight;
    setup.home_airport = None;
    let mut mission = AiMission::new();
    mission.push(AiActor::new(setup).unwrap());
    assert!(mission.actor(1).unwrap().flight().seat_available());
    mission
        .order(1, land(LandingReason::Ordered))
        .unwrap()
        .unwrap();
    for tick in 0..400 * 120 {
        step(&mut mission, None);
        let actor = mission.actor(1).unwrap();
        trace(tick, actor);
        assert!(!actor.flight().crashed);
        if actor.airfield_phase() == Some(Phase::Final) && actor.flight().position[1] < below_ft {
            let f = mission.actor_mut(1).unwrap().flight_mut();
            f.position = [0., height_ft, z];
            f.yaw = 0.;
            f.bank = 0.;
            f.pitch = -path_deg.to_radians();
            f.velocity = crate::attitude::Basis::new(f.yaw, f.pitch, 0.)
                .forward
                .map(|v| v * f.speed);
            f.damage_fraction = damage;
            assert!(
                crate::ejection::assess(f, |_, _| 0.).is_some(),
                "the fixture must be an ejection hazard"
            );
            return mission;
        }
    }
    panic!("never reached final");
}

/// Fly `seconds` and fail on any ejection or crash; returns the go-arounds.
fn fly_without_ejecting(mission: &mut AiMission, seconds: u64) -> u32 {
    for tick in 0..seconds * 120 {
        step(mission, None);
        let actor = mission.actor(1).unwrap();
        trace(tick, actor);
        assert!(actor.flight().escape.is_none(), "ejected at {tick}");
        assert!(!actor.flight().crashed, "crashed at {tick}");
        if actor.airfield_phase() == Some(Phase::Parked) {
            break;
        }
    }
    mission
        .actor(1)
        .unwrap()
        .airfield()
        .map_or(0, |s| s.go_arounds())
}

/// Opinionated (John, 2026-09-23): a steep, fast-sinking final that would
/// touch down short of the runway is a go-around, not an ejection; the
/// aircraft then tries again and lands.
#[test]
fn a_steep_sinking_final_short_of_the_runway_goes_around_instead_of_ejecting() {
    let mut mission = steep_final(400., 60., 9., -5_600., 0.);
    assert!(fly_without_ejecting(&mut mission, 20) >= 1, "no go-around");
    assert!(mission.actor(1).unwrap().flight().position[1] > 300.);
    fly_until_parked(&mut mission, 1, 900);
    assert!(mission.actor(1).unwrap().flight().escape.is_none());
}

/// A recoverable bad final is still a go-around when pavement is below it.
/// Waiting until impact is unavoidable defeats the airfield ejection guard.
#[test]
fn a_steep_sinking_final_over_the_runway_goes_around_instead_of_ejecting() {
    let mut mission = steep_final(400., 80., 12., -3_600., 0.);
    assert!(fly_without_ejecting(&mut mission, 20) >= 1, "no go-around");
    assert!(mission.actor(1).unwrap().flight().position[1] > 300.);
    fly_until_parked(&mut mission, 1, 900);
    assert!(mission.actor(1).unwrap().flight().escape.is_none());
}

/// Normal landing geometry over the runway, low and sinking, is neither an
/// ejection nor a go-around: the aircraft flares and lands.
#[test]
fn a_low_sinking_final_over_the_runway_lands_without_ejecting() {
    let mut mission = steep_final(400., 60., 6., -3_600., 0.);
    assert_eq!(fly_without_ejecting(&mut mission, 120), 0);
    assert_eq!(
        mission.actor(1).unwrap().airfield_phase(),
        Some(Phase::Parked)
    );
}

/// A catastrophe on final (critical damage) still ejects.
#[test]
fn a_critically_damaged_aircraft_on_final_still_ejects() {
    let mut mission = steep_final(600., 180., 20., -3_600., 0.6);
    let mut ejected = false;
    for tick in 0..10 * 120 {
        step(&mut mission, None);
        let actor = mission.actor(1).unwrap();
        trace(tick, actor);
        if actor.flight().escape.is_some() {
            ejected = true;
            break;
        }
    }
    assert!(ejected, "a critically damaged lander must still eject");
}

/// Regression, found at Simferopol (UKR, 2026-09-23): the straight line
/// from the marshal to the approach gates ran through the mountains south
/// of the field and aircraft flew into them. On the way to the runway they
/// now keep clear of high terrain ahead, then land.
#[test]
fn the_approach_climbs_over_a_ridge_under_the_gates() {
    // A 3,000 ft ridge across the approach, 22,000 to 28,000 ft short of
    // the landing point, above the straight line between the first two
    // gates; the runway and the rest of the world are at sea level.
    let ridge = |x: f64, z: f64| {
        if (-30_000.0..=-24_000.0).contains(&z) {
            Surface::terrain(3_000.)
        } else {
            surface(x, z)
        }
    };
    let mut mission = AiMission::new();
    mission.push(hornet(1, 1, [0., 7_000., -80_000.], 0.));
    mission
        .order(1, land(LandingReason::Ordered))
        .unwrap()
        .unwrap();
    let mut lowest_over_ridge = f64::MAX;
    for tick in 0..1_500 * 120 {
        let world = world(&mission, None);
        mission
            .step_with_surface(&world, &|x, z| ridge(x, z).height, &ridge, TimeOfDay(0))
            .unwrap();
        let actor = mission.actor(1).unwrap();
        trace(tick, actor);
        let f = actor.flight();
        assert!(
            !f.crashed,
            "crashed at {:.0}s at {:?}",
            tick as f64 / 120.,
            f.position
        );
        if (-30_000.0..=-24_000.0).contains(&f.position[2]) {
            lowest_over_ridge = lowest_over_ridge.min(f.position[1] - 3_000.);
        }
        if actor.airfield_phase() == Some(Phase::Parked) {
            break;
        }
    }
    assert!(
        lowest_over_ridge > 300.,
        "{lowest_over_ridge:.0} ft over the ridge"
    );
    assert_parked_on_runway(mission.actor(1).unwrap());
}
