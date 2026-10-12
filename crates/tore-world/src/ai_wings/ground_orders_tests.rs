//! A flight lead's orders onto the ground (slice ST-O): what each order
//! means ([`resolve`]), and a wingman taking or refusing it in the synthetic
//! defended template. No retail data: the fixture F/A-18's station 0 is given
//! a Mk 82 pack where a test needs a surface store.
use super::*;
use crate::mission::{Skill, WingSpec};
use crate::seats::PlaneId;
use crate::test_support::surface::{spec_with_target, surface_resources, target};
use crate::world::{Seating, TickOutput, World};
use tore_formats::aircraft::AircraftId;
use tore_sim::ai::engagement::{GroupObjective, StrikeRole};
use tore_sim::ai::launch::Side as Wing;
use tore_sim::ai::wing::PlayerOrder as O;
use tore_sim::combat::missiles::TargetRole;
use tore_sim::combat::sight::SightSensor;

const SAM: u16 = 0x1000;
const TANK: u16 = 0x400;
const US: Side = Side(1);
const THEM: Side = Side(2);

fn fact(id: u32, side: Side, class: u16, x: f64, z: f64) -> GroundFact {
    GroundFact {
        id,
        side,
        class,
        position: [x, 0., z],
    }
}

fn aim(facts: Vec<GroundFact>) -> GroundAim {
    GroundAim {
        facts,
        ..GroundAim::default()
    }
}

fn no_air(_: u32) -> bool {
    false
}

#[test]
fn alt_e_and_alt_r_take_a_ground_designation_else_the_pin() {
    let ground = GroundAim {
        facts: vec![fact(10, THEM, TANK, 0., 0.), fact(11, THEM, SAM, 500., 0.)],
        sight: None,
        pin: Some([480., 0., 0.]),
        spots: Vec::new(),
    };
    for order in [O::EngageMyTarget, O::EngageFromFormation] {
        // The designation first.
        assert_eq!(
            resolve(order, Some(10), &ground, US, &no_air),
            Plan::Ground {
                target: 10,
                position: [0.; 3],
                noun: Noun::Tank
            }
        );
        // Else the pin names the nearest hostile object.
        let Plan::Ground { target, noun, .. } = resolve(order, None, &ground, US, &no_air) else {
            panic!("the pin names the SAM");
        };
        assert_eq!((target, noun), (11, Noun::Sam));
        // A friendly or missing object is not a ground target: the air
        // path refuses it as it always did.
        let friendly = aim(vec![fact(10, US, TANK, 0., 0.)]);
        assert_eq!(
            resolve(order, Some(10), &friendly, US, &no_air),
            Plan::Air {
                order,
                selected: Some(10)
            }
        );
    }
    // An aircraft designation is an air order whatever the pin holds.
    assert_eq!(
        resolve(O::EngageMyTarget, Some(10), &ground, US, &|id| id == 10),
        Plan::Air {
            order: O::EngageMyTarget,
            selected: Some(10)
        }
    );
}

#[test]
fn a_pin_far_from_everything_names_nothing() {
    let ground = GroundAim {
        facts: vec![fact(10, THEM, TANK, 0., 0.)],
        sight: None,
        pin: Some([PIN_RADIUS_FT + 1., 0., 0.]),
        spots: Vec::new(),
    };
    assert_eq!(
        resolve(O::EngageMyTarget, None, &ground, US, &no_air),
        Plan::Air {
            order: O::EngageMyTarget,
            selected: None
        }
    );
}

#[test]
fn alt_f_takes_the_sights_contact_and_with_none_frees_the_wingmen() {
    let ground = GroundAim {
        facts: vec![fact(10, THEM, TANK, 0., 0.)],
        sight: Some(10),
        pin: None,
        spots: Vec::new(),
    };
    assert!(matches!(
        resolve(O::EngageIrLaser, None, &ground, US, &no_air),
        Plan::Ground { target: 10, .. }
    ));
    // The radar designation is not the IR/laser contact.
    assert!(matches!(
        resolve(O::EngageIrLaser, Some(99), &ground, US, &no_air),
        Plan::Ground { target: 10, .. }
    ));
    // An aircraft the sight tracks (the AC-130's) is an air engage order.
    assert_eq!(
        resolve(O::EngageIrLaser, None, &ground, US, &|id| id == 10),
        Plan::Air {
            order: O::EngageMyTarget,
            selected: Some(10)
        }
    );
    // Nothing held: attack on contact, the retail manual's meaning.
    assert_eq!(
        resolve(
            O::EngageIrLaser,
            Some(10),
            &GroundAim::default(),
            US,
            &no_air
        ),
        Plan::Air {
            order: O::AttackOnContact,
            selected: None
        }
    );
}

#[test]
fn alt_w_orders_every_object_of_the_designated_class_nearest_first() {
    // Three SAMs, one beyond the scan, one friendly, a tank among them.
    let ground = aim(vec![
        fact(1, THEM, SAM, 0., 0.),
        fact(2, THEM, SAM, 6_000., 0.),
        fact(3, THEM, SAM, CLASS_SCAN_FT + 10., 0.),
        fact(4, US, SAM, 100., 0.),
        fact(5, THEM, TANK, 50., 0.),
    ]);
    let Plan::Class { targets, noun } = resolve(O::EngageClass, Some(1), &ground, US, &no_air)
    else {
        panic!("a class order");
    };
    assert_eq!((targets, noun), (vec![1, 2], Noun::Sam));
    // The same from a tank: only tanks.
    let Plan::Class { targets, noun } = resolve(O::EngageClass, Some(5), &ground, US, &no_air)
    else {
        panic!("a class order");
    };
    assert_eq!((targets, noun), (vec![5], Noun::Tank));
}

#[test]
fn a_class_order_names_at_most_the_nearest_few() {
    let facts = (0..100)
        .map(|n| fact(n, THEM, TANK, f64::from(n) * 10., 0.))
        .collect();
    let Plan::Class { targets, .. } = resolve(O::EngageClass, Some(50), &aim(facts), US, &no_air)
    else {
        panic!("a class order");
    };
    assert_eq!(targets.len(), CLASS_ORDER_MAX);
    assert!(targets.contains(&50), "the designated object is always in");
}

#[test]
fn alt_w_on_an_aircraft_or_nothing_is_attack_on_contact() {
    let ground = aim(vec![fact(1, THEM, SAM, 0., 0.)]);
    let blanket = Plan::Air {
        order: O::AttackOnContact,
        selected: None,
    };
    assert_eq!(resolve(O::EngageClass, None, &ground, US, &no_air), blanket);
    assert_eq!(
        resolve(O::EngageClass, Some(7), &ground, US, &|id| id == 7),
        blanket
    );
}

#[test]
fn lase_my_target_needs_a_ground_object() {
    let ground = GroundAim {
        facts: vec![fact(10, THEM, TANK, 0., 0.)],
        sight: Some(10),
        pin: None,
        spots: Vec::new(),
    };
    assert_eq!(
        resolve(O::LaseMyTarget, Some(10), &ground, US, &no_air),
        Plan::Lase { target: 10 }
    );
    // Else what the sight holds.
    assert_eq!(
        resolve(O::LaseMyTarget, None, &ground, US, &no_air),
        Plan::Lase { target: 10 }
    );
    assert_eq!(
        resolve(O::LaseMyTarget, Some(3), &GroundAim::default(), US, &no_air),
        Plan::NoTarget
    );
}

#[test]
fn every_other_order_is_the_air_order_it_was() {
    for order in [O::Disengage, O::ProtectMe, O::Spacing, O::BugOut] {
        assert_eq!(
            resolve(order, Some(4), &GroundAim::default(), US, &no_air),
            Plan::Air {
                order,
                selected: Some(4)
            }
        );
    }
}

#[test]
fn the_class_words_pick_the_calls_noun() {
    assert_eq!(Noun::of_class(0x1000), Noun::Sam);
    assert_eq!(Noun::of_class(0x0800), Noun::Aaa);
    assert_eq!(Noun::of_class(0x2000), Noun::Ship);
    assert_eq!(Noun::of_class(0x0400), Noun::Tank);
    assert_eq!(Noun::of_class(0x0200), Noun::Vehicle);
    assert_eq!(Noun::of_class(0x0100), Noun::Structure);
    assert_eq!(Noun::of_class(0x8000), Noun::Bandit);
    assert_eq!(Noun::of_class(0x0040), Noun::Target);
    assert_eq!(calls::class_stems(Noun::Sam), ["^ATTACK", "^SAMS"]);
    assert_eq!(calls::class_stems(Noun::Tank), ["^ATTACK", "^TANKS"]);
}

// -- wingmen in the template --------------------------------------------------

/// The defended template with the player's F/A-18 and one AI wingman, and two
/// enemy F/A-18s.
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

fn wingman(world: &World) -> u32 {
    world
        .ai_wings
        .as_ref()
        .unwrap()
        .slots()
        .iter()
        .find(|slot| slot.side == Wing::Friendly)
        .unwrap()
        .id
}

/// A Mk 82 pack on station 0 of the wingman's configuration.
fn bomb_the_wingman(world: &mut World, designator: bool) {
    let id = wingman(world);
    let wings = world.ai_wings.as_mut().unwrap();
    let config = wings.configs.get_mut(&id).unwrap();
    let mut weapon = config.stations[0].weapon.clone();
    weapon.source = "MK82.JT".into();
    weapon.flags = 0x22012;
    weapon.movement.minimum_speed = 0;
    weapon.movement.maximum_speed = 1466;
    weapon.movement.initial_speed = 0;
    weapon.burst.actual_rounds_per_game = 1;
    weapon.burst.game_rounds_in_burst = 6;
    weapon.burst.projectiles_in_pod = 1;
    weapon.seeker.signature = 0;
    config.stations[0].weapon = weapon;
    if designator {
        let mut seeker = config.stations[0].weapon.seeker;
        seeker.signature = 1;
        config.sight = SightSensor::from_record(&seeker, 0);
    }
}

/// The hostile ground objects of one class word, ascending id.
fn hostile(world: &World, class: u16) -> Vec<u32> {
    let mut ids: Vec<u32> = world
        .combat
        .state
        .targets
        .iter()
        .filter(|t| {
            t.role == TargetRole::Surface && t.side == THEM && t.hp > 0 && t.category & class != 0
        })
        .map(|t| t.id)
        .collect();
    ids.sort_unstable();
    ids
}

fn give(world: &mut World, order: O, selected: Option<u32>) -> OrderReport {
    let ground = world.ground_aim(PlaneId(0), order);
    world
        .ai_wings
        .as_mut()
        .unwrap()
        .command_aimed(0, order, selected, None, None, None, &|_| false, &ground)
        .unwrap()
}

fn ordered(world: &World) -> Option<u32> {
    world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(wingman(world))
        .unwrap()
        .strike_order()
}

fn lines(world: &World) -> Vec<Chatter> {
    world.ai_wings.as_ref().unwrap().chatter.clone()
}

#[test]
fn a_wingman_with_no_surface_store_refuses_a_ground_order_and_says_so() {
    let mut world = world();
    let tank = *hostile(&world, TANK | 0x3f00)
        .first()
        .expect("hostile objects");
    let id = wingman(&world);
    let report = give(&mut world, O::EngageMyTarget, Some(tank));
    assert_eq!(
        report.message,
        "Wing order unavailable: no usable surface store"
    );
    assert!(report.reached.is_empty() && report.target.is_none());
    assert_eq!(ordered(&world), None);
    assert!(
        world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actor(id)
            .unwrap()
            .ground_picture()
            .is_none()
    );
    assert_eq!(
        lines(&world),
        [Chatter::Line {
            speaker: id,
            line: WingLine::Winchester
        }]
    );
}

#[test]
fn engage_my_target_onto_the_ground_sends_the_wingman_at_it() {
    let mut world = world();
    bomb_the_wingman(&mut world, false);
    let id = wingman(&world);
    let target = *hostile(&world, 0x3f00).first().expect("hostile objects");
    let report = give(&mut world, O::EngageMyTarget, Some(target));
    assert_eq!(report.reached, [id]);
    assert!(report.ground && report.target.is_none());
    assert!(
        report
            .message
            .starts_with("Engage my target: 1 applied, 0 rejected")
    );
    assert_eq!(ordered(&world), Some(target));
    // The lead's call names where it is, and the wingman answers "Engaging".
    assert_eq!(report.radio[..2], ["^RED", "^ATTACK"]);
    assert_eq!(
        lines(&world),
        [Chatter::Engage {
            speaker: id,
            aircraft: false
        }]
    );
    // Engage my target also loosens the wingman's formation.
    let wings = world.ai_wings.as_ref().unwrap();
    let (control, _, _) = wings
        .mission()
        .actor(id)
        .unwrap()
        .controller()
        .wing_settings();
    assert_eq!(
        control.unwrap_or(wing::WingControl::Loose),
        wing::WingControl::Loose
    );
}

#[test]
fn alt_r_raises_the_wingmans_formation_control_and_sends_it() {
    let mut world = world();
    bomb_the_wingman(&mut world, false);
    let id = wingman(&world);
    let target = *hostile(&world, 0x3f00).first().unwrap();
    let report = give(&mut world, O::EngageFromFormation, Some(target));
    assert_eq!(report.reached, [id]);
    assert_eq!(ordered(&world), Some(target));
    let wings = world.ai_wings.as_ref().unwrap();
    let (control, _, _) = wings
        .mission()
        .actor(id)
        .unwrap()
        .controller()
        .wing_settings();
    assert!(control >= Some(wing::WingControl::Medium));
}

#[test]
fn alt_w_on_a_sam_gives_the_wingman_every_sam_of_the_site() {
    let mut world = world();
    bomb_the_wingman(&mut world, false);
    let id = wingman(&world);
    let sams = hostile(&world, SAM);
    assert!(sams.len() >= 2, "the template has SAM sites");
    let report = give(&mut world, O::EngageClass, Some(sams[0]));
    assert_eq!(report.reached, [id]);
    assert_eq!(report.radio, ["^ATTACK", "^SAMS"]);
    let actor = world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(id)
        .unwrap();
    let picture = actor.ground_picture().expect("a class order has a picture");
    assert_eq!(picture.ordered(), None);
    assert!(picture.class_order().contains(&sams[0]));
    assert!(
        picture.class_order().iter().all(|t| sams.contains(t)),
        "only SAMs"
    );
    assert_eq!(
        lines(&world),
        [Chatter::Engage {
            speaker: id,
            aircraft: false
        }]
    );
}

#[test]
fn alt_w_with_nothing_designated_is_attack_on_contact() {
    let mut world = world();
    bomb_the_wingman(&mut world, false);
    let report = give(&mut world, O::EngageClass, None);
    assert_eq!(report.radio, ["^ATTACK", "^BANDITS"]);
    assert_eq!(ordered(&world), None);
    assert!(report.message.starts_with("Engage target class: 1 applied"));
}

#[test]
fn alt_f_with_the_sight_on_a_ground_object_sends_the_wingman_at_it() {
    let mut world = world();
    bomb_the_wingman(&mut world, false);
    let target = *hostile(&world, 0x3f00).first().unwrap();
    let ground = GroundAim {
        sight: Some(target),
        ..world.ground_aim(PlaneId(0), O::EngageIrLaser)
    };
    let report = world
        .ai_wings
        .as_mut()
        .unwrap()
        .command_aimed(
            0,
            O::EngageIrLaser,
            None,
            None,
            None,
            None,
            &|_| false,
            &ground,
        )
        .unwrap();
    assert!(report.ground);
    assert_eq!(ordered(&world), Some(target));
    // With nothing in the sight the wingmen are freed instead.
    let report = give(&mut world, O::EngageIrLaser, None);
    assert_eq!(report.radio, ["^ATTACK", "^BANDITS"]);
    assert_eq!(ordered(&world), None, "attack on contact ends the strike");
}

#[test]
fn lase_my_target_needs_a_designator_and_a_target() {
    let mut world = world();
    let id = wingman(&world);
    let target = *hostile(&world, 0x3f00).first().unwrap();
    // No ground target at all.
    let report = give(&mut world, O::LaseMyTarget, None);
    assert_eq!(
        report.message,
        "Wing order unavailable: no ground target designated"
    );
    assert_eq!(
        lines(&world),
        [Chatter::Line {
            speaker: id,
            line: WingLine::NoTarget
        }]
    );
    world.ai_wings.as_mut().unwrap().chatter.clear();
    // The wingman has no designator.
    let report = give(&mut world, O::LaseMyTarget, Some(target));
    assert_eq!(
        report.message,
        "Wing order unavailable: no designator aboard"
    );
    assert_eq!(
        lines(&world),
        [Chatter::Line {
            speaker: id,
            line: WingLine::NoDesignator
        }]
    );
    // With one aboard it takes the order, says so, and holds it for slice
    // ST-L's orbit.
    world.ai_wings.as_mut().unwrap().chatter.clear();
    bomb_the_wingman(&mut world, true);
    let report = give(&mut world, O::LaseMyTarget, Some(target));
    assert_eq!(report.reached, [id]);
    let actor = world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(id)
        .unwrap();
    assert_eq!(actor.lase_order(), Some(target));
    // ... and flies it as slice ST-L's lase duty (ST-INT).
    assert_eq!(
        world.ai_wings.as_ref().unwrap().lasing_for(id),
        Some(target)
    );
    assert_eq!(
        lines(&world),
        [Chatter::Line {
            speaker: id,
            line: WingLine::Lasing
        }]
    );
}

fn step_world(world: &mut World, ticks: u32) {
    let mut out = TickOutput::default();
    for _ in 0..ticks {
        let input = crate::seats::SeatInput {
            tick: world.tick(),
            ..Default::default()
        };
        world.step(&[input], &mut out).unwrap();
    }
}

/// Alt+G, wired to slice ST-L (ST-INT): the wingman flies the lasing hold
/// with its laser on and its sight on the target, and its spot comes up on
/// the target; a later order ends the duty, and so does the duty's own end
/// (a threat or the target's death), after which it rejoins rather than
/// bombing the target it was lasing.
#[test]
fn lase_my_target_flies_the_lasing_hold_until_the_order_or_the_duty_ends() {
    use tore_sim::ai::strike::Phase;
    // No enemy fighters: one coming inside 5 nm would end the duty (a
    // threat), as the spec says.
    let mut spec = spec_with_target(&target("QUCITY", 3, 3, 9));
    spec.wings[0].count = 2;
    let mut world = World::new(&spec, &surface_resources(), Seating::SinglePlayer).unwrap();
    let id = wingman(&world);
    let target = *hostile(&world, 0x3f00).first().unwrap();
    bomb_the_wingman(&mut world, true);
    // A Pave Spike-like pod: 10 nm of reach and the hemisphere below.
    {
        let config = world
            .ai_wings
            .as_mut()
            .unwrap()
            .configs
            .get_mut(&id)
            .unwrap();
        let mut seeker = config.stations[0].weapon.seeker;
        seeker.signature = 1;
        seeker.zones[0].maximum_range = 60_760;
        config.sight = SightSensor::from_record(&seeker, 425);
    }
    let report = give(&mut world, O::LaseMyTarget, Some(target));
    assert_eq!(report.reached, [id]);
    fn actor(world: &World) -> &tore_sim::ai::mission::AiActor {
        let id = wingman(world);
        world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actor(id)
            .unwrap()
    }
    let mut lased = false;
    for _ in 0..200 {
        step_world(&mut world, 240);
        let a = actor(&world);
        assert_eq!(a.lase_order(), Some(target), "the order stands");
        assert_eq!(a.strike().map(|s| s.phase()), Some(Phase::Lase));
        let cockpit = world.combat.state.strike_cockpit(id).unwrap();
        assert_eq!(cockpit.laser.manual, Some(true), "the laser on by hand");
        assert_eq!(cockpit.sight.and_then(|s| s.target()), Some(target));
        if world
            .combat
            .state
            .laser_spots()
            .iter()
            .any(|spot| spot.owner == id)
        {
            lased = true;
            break;
        }
    }
    assert!(lased, "the spot came up");
    assert_eq!(a_release_count(&world, id), 0, "it lases, it does not bomb");
    // Disengage ends the duty with the order.
    give(&mut world, O::Disengage, None);
    let a = actor(&world);
    assert_eq!(a.lase_order(), None);
    assert_eq!(a.lasing(), None);
    assert!(a.ground_picture().is_none());
    // Ordered again, then the duty ends on its own (the target dies here; a
    // threat ends it the same way): the order goes with it, and the
    // wingman, a CAP one, drops the target and rejoins.
    give(&mut world, O::LaseMyTarget, Some(target));
    assert_eq!(
        world.ai_wings.as_ref().unwrap().lasing_for(id),
        Some(target)
    );
    step_world(&mut world, 30);
    world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == target)
        .unwrap()
        .hp = 0;
    step_world(&mut world, 60);
    let a = actor(&world);
    assert_eq!(a.lase_order(), None, "the order ends with its duty");
    assert_eq!(a.strike_order(), None);
    assert!(a.ground_picture().is_none());
    assert!(a.alive());
}

/// Rounds the wingman has released from its stations.
fn a_release_count(world: &World, id: u32) -> usize {
    world
        .combat
        .state
        .projectiles
        .iter()
        .filter(|p| p.owner == id)
        .count()
}

#[test]
fn a_later_order_replaces_the_ground_order_but_a_formation_setting_does_not() {
    let mut world = world();
    bomb_the_wingman(&mut world, false);
    let target = *hostile(&world, 0x3f00).first().unwrap();
    give(&mut world, O::EngageMyTarget, Some(target));
    assert_eq!(ordered(&world), Some(target));
    // Spacing is a setting: the strike stands.
    give(&mut world, O::Spacing, None);
    assert_eq!(ordered(&world), Some(target));
    // Disengage ends it.
    give(&mut world, O::Disengage, None);
    assert_eq!(ordered(&world), None);
    // A class order replaces a single target, and lasing replaces both.
    give(&mut world, O::EngageMyTarget, Some(target));
    let sams = hostile(&world, SAM);
    give(&mut world, O::EngageClass, Some(sams[0]));
    assert_eq!(ordered(&world), None);
    give(&mut world, O::EngageMyTarget, Some(target));
    assert_eq!(ordered(&world), Some(target));
}

#[test]
fn a_wingman_whose_ordered_target_is_gone_rejoins_instead_of_going_home() {
    let mut world = world();
    bomb_the_wingman(&mut world, false);
    let id = wingman(&world);
    let target = *hostile(&world, 0x3f00).first().unwrap();
    give(&mut world, O::EngageMyTarget, Some(target));
    let step = |world: &mut World, ticks: u32| {
        let mut out = TickOutput::default();
        for _ in 0..ticks {
            let input = crate::seats::SeatInput {
                tick: world.tick(),
                ..Default::default()
            };
            world.step(&[input], &mut out).unwrap();
        }
    };
    // While the object stands the order stands, and the wingman strikes.
    step(&mut world, 240);
    assert_eq!(ordered(&world), Some(target));
    assert!(
        world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actor(id)
            .unwrap()
            .strikes()
    );
    // The object falls before the wingman gets there.
    world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == target)
        .unwrap()
        .hp = 0;
    // It pulls off, turns back, finds nothing left and rejoins: the order
    // ends (a Strike-role actor would go home instead).
    for _ in 0..80 {
        step(&mut world, 240);
        if ordered(&world).is_none() {
            break;
        }
    }
    let actor = world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(id)
        .unwrap();
    assert_eq!(ordered(&world), None, "the order ends with its target");
    assert!(actor.ground_picture().is_none());
    // It was not sent home to land: no landing order.
    assert!(actor.alive());
}

// -- Quick Mission roles --------------------------------------------------------

/// The template with the player's wingman and a second friendly wing of two.
fn world_with_second_wing() -> World {
    let mut spec = spec_with_target(&target("QUCITY", 3, 3, 9));
    spec.wings[0].count = 2;
    spec.wings[1] = WingSpec {
        aircraft: AircraftId::F18,
        count: 2,
        skill: Skill::Average,
    };
    spec.wings[3] = WingSpec {
        aircraft: AircraftId::F18,
        count: 2,
        skill: Skill::Average,
    };
    World::new(&spec, &surface_resources(), Seating::SinglePlayer).unwrap()
}

fn roles(world: &World) -> Vec<(Wing, u8, StrikeRole)> {
    let wings = world.ai_wings.as_ref().unwrap();
    wings
        .slots()
        .iter()
        .map(|slot| {
            (
                slot.side,
                slot.wing_number,
                wings.mission().actor(slot.id).unwrap().strike_role(),
            )
        })
        .collect()
}

fn set_class(world: &mut World, class: u16) {
    let wings = world.ai_wings.as_mut().unwrap();
    for config in wings.configs.values_mut() {
        config.target_category = class;
    }
}

#[test]
fn bomber_class_wings_strike_by_default_when_a_ground_target_is_set() {
    let mut world = world_with_second_wing();
    set_class(&mut world, 0x4000);
    let inherit = [GroupObjective::Inherit; 6];
    world
        .ai_wings
        .as_mut()
        .unwrap()
        .apply_strike_roles(&inherit, true);
    for (side, wing, role) in roles(&world) {
        // Only friendly wing 2; the player's wing and Redfor stay CAP.
        let strikes = side == Wing::Friendly && wing == 2;
        assert_eq!(role == StrikeRole::Strike, strikes, "{side:?} {wing}");
    }
    // The same wing with no ground target, or of fighters, flies CAP.
    let mut world = world_with_second_wing();
    set_class(&mut world, 0x4000);
    world
        .ai_wings
        .as_mut()
        .unwrap()
        .apply_strike_roles(&inherit, false);
    assert!(roles(&world).iter().all(|(_, _, r)| *r == StrikeRole::Cap));
    let mut world = world_with_second_wing();
    set_class(&mut world, 0x8000);
    world
        .ai_wings
        .as_mut()
        .unwrap()
        .apply_strike_roles(&inherit, true);
    assert!(roles(&world).iter().all(|(_, _, r)| *r == StrikeRole::Cap));
}

#[test]
fn an_explicit_objective_overrides_the_class_default() {
    let mut world = world_with_second_wing();
    set_class(&mut world, 0x8000);
    let mut objectives = [GroupObjective::Inherit; 6];
    objectives[1] = GroupObjective::Strike;
    // Even for the player's wing the objective is ignored (index 0), and a
    // Redfor wing's too.
    objectives[0] = GroupObjective::Strike;
    objectives[3] = GroupObjective::Strike;
    world
        .ai_wings
        .as_mut()
        .unwrap()
        .apply_strike_roles(&objectives, true);
    for (side, wing, role) in roles(&world) {
        assert_eq!(
            role == StrikeRole::Strike,
            side == Wing::Friendly && wing == 2,
            "{side:?} {wing}"
        );
    }
    // A bomber wing told to patrol keeps CAP.
    let mut world = world_with_second_wing();
    set_class(&mut world, 0x4000);
    let mut objectives = [GroupObjective::Inherit; 6];
    objectives[1] = GroupObjective::Cap;
    world
        .ai_wings
        .as_mut()
        .unwrap()
        .apply_strike_roles(&objectives, true);
    assert!(roles(&world).iter().all(|(_, _, r)| *r == StrikeRole::Cap));
}

/// A GBU-10-like laser bomb on station 0 of the wingman's configuration, and
/// no designator.
fn laser_bomb_the_wingman(world: &mut World) {
    let id = wingman(world);
    let wings = world.ai_wings.as_mut().unwrap();
    let config = wings.configs.get_mut(&id).unwrap();
    let mut weapon = config.stations[0].weapon.clone();
    weapon.source = "GBU10.JT".into();
    weapon.flags = 0x2a543;
    weapon.seeker.signature = 1;
    weapon.movement.ignite_t = 0;
    weapon.movement.fuel_t = 0;
    weapon.movement.launch_retard = 90;
    weapon.burst.actual_rounds_per_game = 1;
    weapon.burst.game_rounds_in_burst = 1;
    weapon.burst.projectiles_in_pod = 1;
    config.stations[0].weapon = weapon;
    config.sight = None;
}

/// Slice ST-L's spot rule in the order (ST-INT): a wingman whose only
/// surface store is a laser bomb and who has no designator takes Alt+E on
/// an object a same-side laser spot lies on (its lead lasing it), and
/// refuses it with no such spot.
#[test]
fn a_laser_only_wingman_takes_an_order_onto_an_object_its_side_lases() {
    let mut world = world();
    let id = wingman(&world);
    let target = *hostile(&world, 0x3f00).first().unwrap();
    laser_bomb_the_wingman(&mut world);
    let order = |world: &mut World, spots: Vec<(Side, u32)>| {
        let ground = GroundAim {
            spots,
            ..world.ground_aim(PlaneId(0), O::EngageMyTarget)
        };
        world
            .ai_wings
            .as_mut()
            .unwrap()
            .command_aimed(
                0,
                O::EngageMyTarget,
                Some(target),
                None,
                None,
                None,
                &|_| false,
                &ground,
            )
            .unwrap()
    };
    // Nobody lasing, or only the other side's spot, or a spot on another
    // object: no usable surface store.
    let other = *hostile(&world, 0x3f00).last().unwrap();
    for spots in [Vec::new(), vec![(THEM, target)], vec![(US, other)]] {
        if spots
            .first()
            .is_some_and(|(_, on)| *on == target && other == target)
        {
            continue;
        }
        let report = order(&mut world, spots);
        assert_eq!(
            report.message,
            "Wing order unavailable: no usable surface store"
        );
        assert_eq!(ordered(&world), None);
    }
    // The lead lases the target: the wingman takes it.
    let report = order(&mut world, vec![(US, target)]);
    assert_eq!(report.reached, [id]);
    assert_eq!(ordered(&world), Some(target));
}
