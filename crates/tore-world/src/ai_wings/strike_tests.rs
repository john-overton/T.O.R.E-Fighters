//! The strike bridge's lasing half (docs/spec/ai-strike.md, "Lasing", slice
//! ST-L) against the synthetic defended template: the lase order's API, the
//! cockpit's sight and laser it drives, the laser it lets go, and the
//! callsign the readout names an AI by. No retail data.
use crate::mission::{Skill, WingSpec};
use crate::seats::SeatInput;
use crate::test_support::surface::{spec_with_target, surface_resources, target};
use crate::world::{Seating, TickOutput, World};
use tore_formats::aircraft::AircraftId;
use tore_sim::ai::launch::Side as Wing;
use tore_sim::ai::strike::{LaseUntil, Phase};
use tore_sim::combat::live::StrikeTarget;
use tore_sim::combat::sight::{Gimbal, SightSensor};

fn world() -> World {
    let mut spec = spec_with_target(&target("QUCITY", 3, 3, 9));
    spec.wings[0].count = 2;
    spec.wings[3] = WingSpec {
        aircraft: AircraftId::F18,
        count: 2,
        skill: Skill::Average,
    };
    World::new(&spec, &surface_resources(), Seating::SinglePlayer).unwrap()
}

fn step(world: &mut World, ticks: u64) {
    let mut out = TickOutput::default();
    for _ in 0..ticks {
        let input = SeatInput {
            tick: world.tick(),
            ..SeatInput::default()
        };
        world.step(&[input], &mut out).unwrap();
    }
}

/// A Pave Spike-like pod: the laser, ten miles, the hemisphere.
fn pod() -> SightSensor {
    SightSensor {
        laser: true,
        range_ft: 60_760.,
        gimbal: Gimbal::Hemisphere,
        eye: [0.; 3],
        default_look: [0., -0.17],
    }
}

fn friendly(world: &World) -> Vec<u32> {
    world
        .ai_wings
        .as_ref()
        .unwrap()
        .slots()
        .iter()
        .filter(|slot| slot.side == Wing::Friendly)
        .map(|slot| slot.id)
        .collect()
}

#[test]
fn a_lase_order_needs_a_designator_and_an_ai_aircraft() {
    let mut world = world();
    let wingman = friendly(&world)[0];
    let targets =
        super::super::outcome::ground_targets(&world.terrain.surface, &world.combat.state);
    let target = targets[0];
    let wings = world.ai_wings.as_mut().unwrap();
    assert!(!wings.can_lase(wingman), "a synthetic F/A-18 has none");
    assert!(!wings.order_lase(wingman, target, LaseUntil::TargetDies));
    assert_eq!(wings.lasing_for(wingman), None);
    assert!(!wings.order_lase(9_999, target, LaseUntil::TargetDies));
    assert!(!wings.cancel_lase(wingman));
    // Its callsign is the one the messages use: wing, member.
    assert_eq!(wings.callsign(wingman).as_deref(), Some("Friendly 1-2"));
    assert_eq!(wings.callsign(9_999), None);
}

#[test]
fn a_lase_order_points_the_sight_and_the_laser_at_the_target_and_cancelling_lets_go() {
    let mut world = world();
    let wingman = friendly(&world)[0];
    let target =
        super::super::outcome::ground_targets(&world.terrain.surface, &world.combat.state)[0];
    let wings = world.ai_wings.as_mut().unwrap();
    let mut config = wings.configs.get(&wingman).unwrap().clone();
    config.sight = Some(pod());
    wings.configs.insert(wingman, config);
    assert!(wings.can_lase(wingman));
    assert!(wings.order_lase(wingman, target, LaseUntil::TargetDies));
    assert_eq!(wings.lasing_for(wingman), Some(target));

    step(&mut world, 60);
    let wings = world.ai_wings.as_ref().unwrap();
    let actor = wings.mission().actor(wingman).unwrap();
    assert_eq!(actor.strike().map(|s| s.phase()), Some(Phase::Lase));
    let cockpit = world
        .combat
        .state
        .strike_cockpit(wingman)
        .expect("the AI's strike cockpit");
    assert_eq!(cockpit.target, Some(StrikeTarget::Object(target)));
    assert_eq!(cockpit.sight.and_then(|s| s.target()), Some(target));
    assert_eq!(
        cockpit.laser.manual,
        Some(true),
        "on by hand while it lases"
    );
    // Its spot is an ordinary spot of its side once the laser lases (here it
    // may be out of range or masked; the state says which).
    assert!(cockpit.laser.wanted());

    // The order changes: the duty ends, the order it made goes, the laser
    // is let go at the next step.
    let wings = world.ai_wings.as_mut().unwrap();
    assert!(wings.cancel_lase(wingman));
    assert_eq!(wings.lasing_for(wingman), None);
    assert_eq!(
        wings.mission().actor(wingman).unwrap().strike_order(),
        None,
        "the ordered target was the duty's"
    );
    step(&mut world, 2);
    let cockpit = world.combat.state.strike_cockpit(wingman).unwrap();
    assert_eq!(cockpit.laser.manual, None);
    assert!(!cockpit.laser.wanted());
}

#[test]
fn a_lase_order_ends_when_the_target_is_destroyed() {
    let mut world = world();
    let wingman = friendly(&world)[0];
    let target =
        super::super::outcome::ground_targets(&world.terrain.surface, &world.combat.state)[0];
    let wings = world.ai_wings.as_mut().unwrap();
    let mut config = wings.configs.get(&wingman).unwrap().clone();
    config.sight = Some(pod());
    wings.configs.insert(wingman, config);
    assert!(wings.order_lase(wingman, target, LaseUntil::TargetDies));
    step(&mut world, 30);
    assert_eq!(
        world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actor(wingman)
            .unwrap()
            .strike()
            .map(|s| s.phase()),
        Some(Phase::Lase)
    );
    world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == target)
        .unwrap()
        .hp = 0;
    step(&mut world, 30);
    let wings = world.ai_wings.as_ref().unwrap();
    assert_eq!(wings.lasing_for(wingman), None);
}
