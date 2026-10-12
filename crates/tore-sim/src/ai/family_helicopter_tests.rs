//! The helicopter family (docs/spec/ai-strike.md, "Helicopters"): the pass
//! and store choice, the pop-up without the speed bleed, the AT-2 held to
//! impact, the stand-off attack under SAMs, the rocket burst and
//! determinism. Synthetic pictures and solutions only.
use super::super::tests::{TARGET, TARGET_AT, World, ground, inputs, own, picture, site, solution};
use super::super::{Order, Phase, Segment, Store, StoreKind, Strike};
use super::*;
use crate::ai::weapon_service::StationId;
use crate::checkpoint::{Models, from_bytes, to_bytes};
use crate::models::strike_class::StrikeClass;

fn at2(rounds: u32) -> Store {
    Store {
        station: StationId(1),
        kind: StoreKind::Command,
        rounds,
        cadence_ticks: 30,
        fire_range_ft: 10_000.,
        minimum_range_ft: 500.,
        maximum_range_ft: 18_000.,
        round_speed_fps: None,
        release_speed_share: 1.,
        fuze_arm_s: 0.,
        frag_radius_ft: 0.,
    }
}

fn rockets(rounds: u32) -> Store {
    Store {
        kind: StoreKind::Rocket,
        maximum_range_ft: 12_000.,
        minimum_range_ft: 50.,
        round_speed_fps: Some(2_000.),
        release_speed_share: 1.,
        fuze_arm_s: 0.,
        frag_radius_ft: 0.,
        ..at2(rounds)
    }
}

/// A Mi-24 at 1,500 ft, 24,000 ft south of the truck, heading north.
fn heli_world(store: Store) -> World {
    let mut inputs = inputs(vec![store], None);
    inputs.class = StrikeClass::Helicopter;
    World {
        own: own([0., 1_500., 0.], 200.),
        picture: picture(&[ground(TARGET, TARGET_AT, 0x200, None)], Vec::new()),
        inputs,
    }
}

fn steer(order: &Order) -> super::super::Steer {
    order.steer.expect("the helicopter steers")
}

#[test]
fn the_pass_and_the_store_are_fixed_functions_of_store_and_threat() {
    use crate::ai::awareness::ground::ThreatLevel as T;
    assert_eq!(choose_pass(StoreKind::Command, T::Missiles), Pass::StandOff);
    assert_eq!(choose_pass(StoreKind::Command, T::Guns), Pass::PopUp);
    assert_eq!(choose_pass(StoreKind::Command, T::Clear), Pass::PopUp);
    assert_eq!(choose_pass(StoreKind::Rocket, T::Missiles), Pass::PopUp);
    let both = [rockets(38), at2(4)];
    assert_eq!(choose_store(&both, false).unwrap().kind, StoreKind::Command);
    assert_eq!(
        choose_store(&[rockets(38), at2(0)], false).unwrap().kind,
        StoreKind::Rocket
    );
    let laser = Store {
        kind: StoreKind::Laser,
        ..at2(2)
    };
    assert!(choose_store(&[laser], false).is_none());
    assert_eq!(choose_store(&[laser], true).unwrap().kind, StoreKind::Laser);
}

#[test]
fn the_at2_popup_keeps_its_speed_and_holds_the_line_to_impact() {
    let mut world = heli_world(at2(4));
    let mut strike = Strike::new(3);
    // The choice: the pop-up, toward 1,500 ft over the target.
    let order = strike.step(&world.context(1));
    assert_eq!(strike.phase(), Phase::Attack);
    assert_eq!(strike.pass(), Some(Pass::PopUp));
    assert_eq!(strike.segment(), Segment::Approach);
    assert_eq!(steer(&order).speed, SpeedRequest::Maximum);
    // Inside 10,000 ft: the pull, 30 degrees over the line of sight, at
    // its speed (no retail bleed).
    world.own.position = [0., 1_500., TARGET_AT[2] - 9_000.];
    let order = strike.step(&world.context(2));
    assert_eq!(strike.segment(), Segment::Pull);
    let pull = steer(&order);
    assert_eq!(pull.speed, SpeedRequest::Maximum);
    let line = -(1_500_f64).atan2(9_000.).to_degrees();
    assert!(
        (pull.pitch_deg - (line + 30.)).abs() < 1e-6,
        "{}",
        pull.pitch_deg
    );
    // The shared release test passes: the AT-2 leaves.
    world.inputs.solution = Some(solution(TARGET_AT, TARGET_AT, true));
    let order = strike.step(&world.context(3));
    let release = order.release.expect("the AT-2 leaves");
    assert_eq!((release.station, release.target), (StationId(1), TARGET));
    assert_eq!(strike.segment(), Segment::Guide);
    // While it flies the nose stays on the target, and nothing else leaves.
    world.own.position = [400., 1_500., TARGET_AT[2] - 8_000.];
    for tick in 4..400 {
        world.inputs.solution = Some(Solution {
            guided_in_flight: true,
            ..solution(TARGET_AT, TARGET_AT, true)
        });
        let order = strike.step(&world.context(tick));
        assert!(order.release.is_none());
        assert_eq!(strike.segment(), Segment::Guide);
        let bearing = bearing_deg(world.own.position, TARGET_AT);
        assert!((steer(&order).heading_deg - bearing).abs() < 1e-9);
    }
    // Gone: the exit, at maximum speed.
    world.inputs.solution = Some(solution(TARGET_AT, TARGET_AT, false));
    strike.step(&world.context(400));
    let order = strike.step(&world.context(401));
    assert_eq!(strike.phase(), Phase::Exit);
    assert_eq!(steer(&order).speed, SpeedRequest::Maximum);
}

#[test]
fn under_a_sam_it_attacks_from_low_at_the_weapons_range() {
    let mut world = heli_world(at2(4));
    const SAM: u32 = 0x5000_0009;
    world.picture = picture(
        &[
            ground(TARGET, TARGET_AT, 0x200, None),
            ground(
                SAM,
                [3_000., 0., TARGET_AT[2]],
                0x1000,
                site(30_000., 0., true),
            ),
        ],
        vec![SAM],
    );
    let mut strike = Strike::new(3);
    let order = strike.step(&world.context(1));
    assert_eq!(strike.pass(), Some(Pass::StandOff));
    // Masked: down toward 200 ft over the ground.
    assert!(steer(&order).pitch_deg < 0.);
    // At 60% of the AT-2's 18,000 ft zone it unmasks, climbing, at speed.
    world.own.position = [0., 300., TARGET_AT[2] - 10_500.];
    world.own.agl_ft = 300.;
    let order = strike.step(&world.context(2));
    assert_eq!(strike.segment(), Segment::Pull);
    let unmask = steer(&order);
    assert!(unmask.pitch_deg > 0.);
    assert_eq!(unmask.speed, SpeedRequest::Maximum);
    // It fires there and holds the line.
    world.inputs.solution = Some(solution(TARGET_AT, TARGET_AT, true));
    assert!(strike.step(&world.context(3)).release.is_some());
    assert_eq!(strike.segment(), Segment::Guide);
    // Unmasked too long with no shot: it masks again, turning away.
    let mut world = heli_world(at2(4));
    world.picture = picture(
        &[
            ground(TARGET, TARGET_AT, 0x200, None),
            ground(
                SAM,
                [3_000., 0., TARGET_AT[2]],
                0x1000,
                site(30_000., 0., true),
            ),
        ],
        vec![SAM],
    );
    let mut strike = Strike::new(3);
    strike.step(&world.context(1));
    world.own.position = [0., 300., TARGET_AT[2] - 10_500.];
    strike.step(&world.context(2));
    strike.step(&world.context(2 + UNMASK_TICKS));
    assert_eq!(strike.phase(), Phase::Exit);
    let order = strike.step(&world.context(3 + UNMASK_TICKS));
    assert!(
        (steer(&order).heading_deg - 180.).abs() < 1.,
        "away from the target"
    );
}

#[test]
fn rockets_leave_in_a_burst_while_the_cue_stays_in_the_circle() {
    let mut world = heli_world(rockets(38));
    let mut strike = Strike::new(3);
    strike.step(&world.context(1));
    assert_eq!(strike.pass(), Some(Pass::PopUp));
    world.own.position = [0., 1_500., TARGET_AT[2] - 9_000.];
    strike.step(&world.context(2));
    assert_eq!(strike.segment(), Segment::Pull);
    let (offset, _) = strike.aim();
    let aim = [
        TARGET_AT[0] + offset[0],
        TARGET_AT[1],
        TARGET_AT[2] + offset[1],
    ];
    world.own.position = [0., 1_400., TARGET_AT[2] - 7_000.];
    let mut fired = 0;
    for tick in 3..400 {
        world.inputs.solution = Some(solution(TARGET_AT, aim, fired == 0));
        if strike.step(&world.context(tick)).release.is_some() {
            fired += 1;
        }
    }
    assert_eq!(fired, ROCKETS_PER_PASS);
}

#[test]
fn the_helicopter_is_deterministic_and_round_trips_through_a_checkpoint() {
    let mut world = heli_world(at2(4));
    let mut a = Strike::new(5);
    let mut b = Strike::new(5);
    for tick in 0..300 {
        world.own.position[2] += 2.;
        assert_eq!(a.step(&world.context(tick)), b.step(&world.context(tick)));
    }
    let models = Models::default();
    let coded = to_bytes(&a, &models).unwrap();
    let mut twin: Strike = from_bytes(&coded, &models).unwrap();
    assert_eq!(twin, a);
    for tick in 300..600 {
        world.own.position[2] += 2.;
        assert_eq!(
            a.step(&world.context(tick)),
            twin.step(&world.context(tick))
        );
    }
}
