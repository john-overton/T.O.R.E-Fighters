//! What an AI lead does with the flight data link, and what a wingman does
//! when a flightmate holds its bandit (slice G4, docs/ARCHITECTURE.md "Flight
//! data link"): the lead shares its target under loose control up to the
//! two-attacker allowance and not under medium, sorts at most once every thirty
//! seconds, skips wingmen that are out of weapons, low on fuel or badly hurt,
//! and leaves wingmen with another mission role alone; a yielding wingman takes
//! another bandit for ten seconds when it has one, keeps the only one it has,
//! and a human is never moved.

use super::*;
use crate::ai::link::{
    LinkEvent, LinkInput, MemberState, SORT_INTERVAL_TICKS, SideBandits, YIELD_TICKS,
};
use crate::ai::targeting::Side;
use crate::ai::wing::PlayerOrder;
use crate::datalink::sort::Bandit;
use crate::sensors::{self, Sensors};
use tests::{enable_test_radar, flat, object, setup, visible_object};

const NM: f64 = crate::sensors::FEET_PER_NAUTICAL_MILE;
const LEAD: u32 = 1;
const TWO: u32 = 2;
const THREE: u32 = 3;

/// The lead's nearest bandit, 10 nm north of the wing.
const NEAR: (u32, [f64; 3]) = (10, [0., 20_000., 10. * NM]);
/// Two more, 12 and 14 nm out.
const MIDDLE: (u32, [f64; 3]) = (11, [3_000., 20_000., 12. * NM]);
const FAR: (u32, [f64; 3]) = (12, [-3_000., 20_000., 14. * NM]);

/// A member of side 1's wing 0 at its place in the line, with a radar when
/// `radar` says so. Without one it sees nothing at ten miles.
fn member(id: u32, place: u8, radar: bool) -> AiActor {
    let mut setup = setup(id, 1, place, [f64::from(place) * 1_000., 20_000., 0.], 0.);
    setup.sensors = Some(Sensors::new(sensors::SensorProfiles {
        aircraft: tore_formats::aircraft::AircraftId::F18,
        radar: None,
        infrared: None,
        visual: None,
        jammer: None,
        signature: sensors::SignatureProfile::default(),
    }));
    let mut actor = AiActor::new(setup).unwrap();
    if radar {
        enable_test_radar(&mut actor);
    }
    actor
}

/// A lead with a radar and two wingmen that see nothing.
fn wing() -> AiMission {
    let mut mission = AiMission::new();
    mission.push(member(LEAD, 0, true));
    mission.push(member(TWO, 1, false));
    mission.push(member(THREE, 2, false));
    mission
}

/// The world of a tick: the wing's own aircraft and `bandits`, which the lead's
/// radar can perceive.
fn world_of(mission: &AiMission, bandits: &[(u32, [f64; 3])]) -> Vec<WorldObject> {
    let lead = &mission.actors[0];
    let mut world: Vec<_> = mission.actors.iter().map(|a| object(a, 1)).collect();
    world.extend(
        bandits
            .iter()
            .map(|&(id, position)| visible_object(lead, id, position)),
    );
    world
}

/// The picture of side 1: it knows `known`.
fn picture(known: &[(u32, [f64; 3])], states: &[MemberState]) -> LinkInput {
    LinkInput {
        bandits: vec![SideBandits {
            side: Side(1),
            bandits: known
                .iter()
                .map(|&(id, position)| Bandit { id, position })
                .collect(),
        }],
        states: states.to_vec(),
        ..LinkInput::default()
    }
}

/// Steps the wing for `ticks` ticks against `bandits`, handing the AI `input`
/// before each step. Returns every link event with its tick.
fn fly(
    mission: &mut AiMission,
    bandits: &[(u32, [f64; 3])],
    ticks: u64,
    input: &dyn Fn() -> LinkInput,
) -> Vec<(u64, LinkEvent)> {
    let mut events = Vec::new();
    for _ in 0..ticks {
        let world = world_of(mission, bandits);
        mission.set_link(input());
        let tick = mission.tick;
        let out = mission.step(&world, &flat, TimeOfDay(tick)).unwrap();
        events.extend(out.link.into_iter().map(|event| (tick, event)));
    }
    events
}

fn assigns(events: &[(u64, LinkEvent)]) -> Vec<(u32, u32, PlayerOrder)> {
    events
        .iter()
        .filter_map(|(_, event)| match *event {
            LinkEvent::Assign {
                receiver,
                target,
                order,
                ..
            } => Some((receiver, target, order)),
            LinkEvent::Yield { .. } => None,
        })
        .collect()
}

fn target_of(mission: &AiMission, id: u32) -> Option<u32> {
    mission.actor(id).unwrap().controller().target()
}

#[test]
fn a_lead_shares_its_new_target_up_to_the_allowance_of_two() {
    let mut mission = wing();
    // The picture knows nothing, so the lead cannot sort and shares.
    let events = fly(&mut mission, &[NEAR], 60, &LinkInput::default);
    let lead_target = target_of(&mission, LEAD);
    assert_eq!(lead_target, Some(NEAR.0));
    // The lead counts as one of the two attackers, so one wingman is given the
    // target: the lowest member number among the idle ones.
    assert_eq!(
        assigns(&events),
        [(TWO, NEAR.0, PlayerOrder::EngageMyTarget)]
    );
    // The first wingman took it as a target order and kept its mission role.
    let wingman = mission.actor(TWO).unwrap();
    assert_eq!(wingman.controller().ordered_target(), Some(NEAR.0));
    assert_eq!(wingman.assignment, engagement::Assignment::default());
    assert_eq!(
        mission.actor(THREE).unwrap().controller().ordered_target(),
        None
    );
}

#[test]
fn nothing_is_shared_under_medium_control() {
    let mut mission = wing();
    mission.set_wing_control(WingControl::Medium);
    let events = fly(&mut mission, &[NEAR], 60, &LinkInput::default);
    assert_eq!(target_of(&mission, LEAD), Some(NEAR.0));
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn a_wingman_that_already_has_a_target_is_left_alone() {
    let mut mission = wing();
    // Wingman two sees the bandit itself: it has a target when the lead takes
    // its own, so only wingman three is idle (and the allowance has room for
    // the one: the lead and wingman two are already on it, so it has none).
    enable_test_radar(mission.actor_mut(TWO).unwrap());
    let events = fly(&mut mission, &[NEAR], 60, &LinkInput::default);
    assert_eq!(target_of(&mission, TWO), Some(NEAR.0));
    assert!(assigns(&events).is_empty(), "{events:?}");
}

#[test]
fn a_lead_that_knows_of_other_bandits_sorts_instead() {
    let mut mission = wing();
    let known = [NEAR, MIDDLE, FAR];
    let events = fly(&mut mission, &[NEAR], 60, &|| picture(&known, &[]));
    // Each wingman is dealt a different bandit, the lead's own is left out,
    // and none is the share.
    let given = assigns(&events);
    assert_eq!(given.len(), 2, "{events:?}");
    assert!(
        given
            .iter()
            .all(|(_, _, order)| *order == PlayerOrder::Sort)
    );
    let targets: Vec<u32> = given.iter().map(|(_, target, _)| *target).collect();
    assert!(!targets.contains(&NEAR.0), "{targets:?}");
    assert_ne!(targets[0], targets[1]);
    assert_eq!(
        given
            .iter()
            .map(|(receiver, ..)| *receiver)
            .collect::<Vec<_>>(),
        [TWO, THREE]
    );
    // The first wingman, which picks first, gets the bandit nearest it.
    assert_eq!(given[0], (TWO, MIDDLE.0, PlayerOrder::Sort));
    assert_eq!(given[1], (THREE, FAR.0, PlayerOrder::Sort));
    let tick = events[0].0;
    assert_eq!(mission.last_sort(Side(1), 0), Some(tick));
}

#[test]
fn a_flight_sorts_at_most_once_in_thirty_seconds() {
    let mut mission = AiMission::new();
    assert!(mission.sort_due(Side(1), 0, 0));
    mission.stamp_sort(Side(1), 0, 500);
    assert_eq!(mission.last_sort(Side(1), 0), Some(500));
    assert!(!mission.sort_due(Side(1), 0, 500));
    assert!(!mission.sort_due(Side(1), 0, 500 + SORT_INTERVAL_TICKS - 1));
    assert!(mission.sort_due(Side(1), 0, 500 + SORT_INTERVAL_TICKS));
    // Another flight keeps its own clock.
    assert!(mission.sort_due(Side(1), 1, 501));
    assert!(mission.sort_due(Side(2), 0, 501));
    mission.stamp_sort(Side(1), 0, 9_000);
    assert_eq!(mission.last_sort(Side(1), 0), Some(9_000));
}

#[test]
fn a_second_commit_inside_the_thirty_seconds_shares_and_does_not_sort() {
    let mut mission = wing();
    let known = [NEAR, MIDDLE, FAR];
    let first = fly(&mut mission, &[NEAR], 60, &|| picture(&known, &[]));
    assert_eq!(assigns(&first).len(), 2);
    let sorted_at = mission.last_sort(Side(1), 0).unwrap();
    // The lead loses its target to a kill: it takes the next one at once.
    let second = fly(&mut mission, &[MIDDLE], 60, &|| picture(&known, &[]));
    assert_eq!(target_of(&mission, LEAD), Some(MIDDLE.0));
    assert!(
        assigns(&second)
            .iter()
            .all(|(_, _, order)| *order != PlayerOrder::Sort),
        "{second:?}"
    );
    assert_eq!(mission.last_sort(Side(1), 0), Some(sorted_at));
}

#[test]
fn wingmen_that_are_winchester_low_on_fuel_or_hurt_are_skipped() {
    let state = |plane, winchester, bingo, heavy_damage| MemberState {
        plane,
        winchester,
        bingo,
        heavy_damage,
    };
    let known = [NEAR, MIDDLE, FAR];
    for skipped in [
        state(TWO, true, false, false),
        state(TWO, false, true, false),
        state(TWO, false, false, true),
    ] {
        let mut mission = wing();
        let events = fly(&mut mission, &[NEAR], 60, &|| picture(&known, &[skipped]));
        let given = assigns(&events);
        assert_eq!(given.len(), 1, "{skipped:?}: {events:?}");
        assert_eq!(given[0].0, THREE, "{skipped:?}");
        assert_eq!(given[0].2, PlayerOrder::Sort);
    }
    // And the share gives the target to the fit one.
    let mut mission = wing();
    let hurt = state(TWO, false, false, true);
    let events = fly(&mut mission, &[NEAR], 60, &|| picture(&[], &[hurt]));
    assert_eq!(
        assigns(&events),
        [(THREE, NEAR.0, PlayerOrder::EngageMyTarget)]
    );
}

#[test]
fn a_wingman_with_another_mission_role_or_still_neutral_is_not_given_a_target() {
    let known = [NEAR, MIDDLE, FAR];
    let mut mission = wing();
    mission.actor_mut(TWO).unwrap().assignment = engagement::Assignment {
        role: engagement::Role::Escort,
        ..engagement::Assignment::default()
    };
    mission.actor_mut(THREE).unwrap().neutral = true;
    let events = fly(&mut mission, &[NEAR], 60, &|| picture(&known, &[]));
    assert!(assigns(&events).is_empty(), "{events:?}");
}

#[test]
fn a_wing_led_by_a_human_gets_nothing_from_an_ai_wingman() {
    // The wing's lead is a human: its AI members are all followers.
    let mut mission = AiMission::new();
    mission.push(member(TWO, 1, true));
    mission.push(member(THREE, 2, false));
    mission.set_humans(vec![HumanMember {
        id: 90,
        side: Side(1),
        wing: 0,
        member: 0,
        pilot_alive: true,
    }]);
    let mut events = Vec::new();
    for tick in 0..60 {
        let mut world = world_of(&mission, &[NEAR]);
        let mut human = object(&mission.actors[0], 1);
        human.id = 90;
        human.human_controlled = true;
        world.push(human);
        mission.set_link(picture(&[NEAR, MIDDLE], &[]));
        let out = mission.step(&world, &flat, TimeOfDay(tick)).unwrap();
        events.extend(out.link.into_iter().map(|event| (tick, event)));
    }
    assert_eq!(mission.wing_leader(Side(1), 0), Some(90));
    assert!(events.is_empty(), "{events:?}");
}

// Yields.

/// One wingman with a radar and two bandits in view: it takes the nearer.
fn lone_wingman() -> AiMission {
    let mut mission = AiMission::new();
    mission.push(member(TWO, 1, true));
    mission
}

#[test]
fn a_yielding_wingman_takes_another_bandit_and_keeps_it_once_the_ten_seconds_lapse() {
    let mut mission = lone_wingman();
    let bandits = [NEAR, MIDDLE];
    fly(&mut mission, &bandits, 40, &LinkInput::default);
    assert_eq!(target_of(&mission, TWO), Some(NEAR.0));
    assert!(mission.yield_target(TWO, NEAR.0));
    let events = fly(&mut mission, &bandits, 10, &LinkInput::default);
    assert_eq!(target_of(&mission, TWO), Some(MIDDLE.0));
    // The yield is announced once.
    let announced: Vec<_> = events
        .iter()
        .filter_map(|(_, event)| match event {
            LinkEvent::Yield { actor, target } => Some((*actor, *target)),
            LinkEvent::Assign { .. } => None,
        })
        .collect();
    assert_eq!(announced, [(TWO, NEAR.0)]);
    // The yield holds for ten seconds, then lapses.
    fly(
        &mut mission,
        &bandits,
        YIELD_TICKS - 20,
        &LinkInput::default,
    );
    assert_eq!(target_of(&mission, TWO), Some(MIDDLE.0));
    fly(&mut mission, &bandits, 60, &LinkInput::default);
    assert!(mission.actor(TWO).unwrap().yields().is_empty());
    // It does not go back: the target it has is as good as the nearer one.
    assert_eq!(target_of(&mission, TWO), Some(MIDDLE.0));
}

#[test]
fn a_yielding_wingman_keeps_the_only_bandit_it_has() {
    let mut mission = lone_wingman();
    fly(&mut mission, &[NEAR], 40, &LinkInput::default);
    assert!(mission.yield_target(TWO, NEAR.0));
    fly(&mut mission, &[NEAR], 120, &LinkInput::default);
    assert_eq!(target_of(&mission, TWO), Some(NEAR.0));
}

#[test]
fn a_human_or_a_lost_aircraft_is_never_asked_to_yield() {
    let mut mission = lone_wingman();
    mission.set_humans(vec![HumanMember {
        id: 90,
        side: Side(1),
        wing: 0,
        member: 0,
        pilot_alive: true,
    }]);
    assert!(!mission.yield_target(90, NEAR.0));
    assert!(!mission.yield_target(77, NEAR.0));
    mission.actor_mut(TWO).unwrap().alive = false;
    assert!(!mission.yield_target(TWO, NEAR.0));
}
