//! The lead hold in the AI mission (slice R2 of the lobby pass; John,
//! 2026-10-09): with a claim on its wing, a lost human lead's successor is a
//! stand-in, and the owner's new plane takes the lead back as soon as it
//! flies. The flight re-forms on it; a wingman that is fighting finishes its
//! fight first. With no claims the rule is today's.

use super::tests::{flat, object, setup, visible_object};
use super::*;
use crate::ai::targeting::Side;
use crate::ai::threat::TimeOfDay;
use crate::ai::wing::{TargetId, TargetOrder, WingRequest};

/// The owner's first plane (member 0), and the plane it revives in (member
/// 3): both human-flown, so world objects, not actors.
const OWNER: u32 = 10;
const REVIVED: u32 = 20;
/// An enemy aircraft the second wingman is ordered to attack.
const HOSTILE: u32 = 30;

/// Side 1's wing 0: the human lead `OWNER` and two AI wingmen, 1 and 2.
fn wing() -> AiMission {
    let mut mission = AiMission::new();
    mission.push(AiActor::new(setup(1, 1, 1, [-500., 20_000., -500.], 0.)).unwrap());
    mission.push(AiActor::new(setup(2, 1, 2, [500., 20_000., -1_000.], 0.)).unwrap());
    mission.set_humans(vec![human(OWNER, 0)]);
    mission.start_in_formation();
    mission
}

fn human(id: u32, member: u8) -> HumanMember {
    HumanMember {
        id,
        side: Side(1),
        wing: 0,
        member,
        pilot_alive: false,
    }
}

/// A human-flown plane as a world object, flying or lost.
fn plane(id: u32, position: [f64; 3], flying: bool) -> WorldObject {
    let actor = AiActor::new(setup(id, 1, 0, position, 0.)).unwrap();
    let mut object = object(&actor, 1);
    object.human_controlled = true;
    object.alive = flying;
    object
}

/// The world of a tick: the wing's actors, the humans' planes and the
/// hostile, alive or not.
fn world(mission: &AiMission, humans: &[WorldObject], hostile: bool) -> Vec<WorldObject> {
    let mut world: Vec<WorldObject> = mission
        .actors()
        .iter()
        .map(|actor| object(actor, 1))
        .chain(humans.iter().cloned())
        .collect();
    let mut bandit = visible_object(&mission.actors()[1], HOSTILE, [500., 20_000., 6_000.]);
    bandit.alive = hostile;
    bandit.destroyed = !hostile;
    if let Some(observable) = bandit.observable.as_mut() {
        observable.destroyed = !hostile;
    }
    world.push(bandit);
    world
}

fn claim(plane: Option<u32>, fresh: bool) -> LeadClaim {
    LeadClaim {
        side: Side(1),
        wing: 0,
        plane,
        fresh,
    }
}

/// One step with `humans` and the hostile alive or not.
fn step(mission: &mut AiMission, humans: &[WorldObject], hostile: bool) -> MissionOutput {
    let world = world(mission, humans, hostile);
    let tick = mission.tick();
    mission.step(&world, &flat, TimeOfDay(tick)).unwrap()
}

#[test]
fn a_claim_makes_a_stand_in_and_the_owners_new_plane_takes_the_lead_back() {
    let mut mission = wing();
    let owner = plane(OWNER, [0., 20_000., 0.], true);
    mission.set_lead_claims(vec![claim(Some(OWNER), false)]);
    step(&mut mission, std::slice::from_ref(&owner), true);
    assert_eq!(mission.wing_leader(Side(1), 0), Some(OWNER));
    assert!(!mission.wing_acting(Side(1), 0));

    // The owner is lost: wingman 1 stands in, and the wing flies its
    // mission of opportunity as today.
    let lost = plane(OWNER, [0., 20_000., 0.], false);
    let output = step(&mut mission, std::slice::from_ref(&lost), true);
    assert_eq!(mission.wing_leader(Side(1), 0), Some(1));
    assert!(mission.wing_acting(Side(1), 0));
    let change = output.leadership[0];
    assert_eq!((change.leader, change.previous), (1, OWNER));
    assert!(change.acting && !change.reclaimed);
    assert_eq!(mission.opportunities().len(), 1);
    // Steps on, the stand-in stays one while the claim does not fly.
    for _ in 0..30 {
        step(&mut mission, std::slice::from_ref(&lost), true);
    }
    assert!(mission.wing_acting(Side(1), 0));

    // Wingman 2 is ordered onto the hostile and is fighting it.
    mission
        .order_wing_report(
            Side(1),
            0,
            None,
            Some(2),
            WingRequest::TargetAssignment(TargetOrder::ConcreteTarget(TargetId(HOSTILE))),
        )
        .unwrap();
    for _ in 0..10 {
        step(&mut mission, std::slice::from_ref(&lost), true);
    }
    assert_eq!(
        mission.actor(2).unwrap().controller().target(),
        Some(HOSTILE)
    );

    // Wingman 1 is told to hold fire, which takes it out of the fight.
    mission
        .order_wing_report(
            Side(1),
            0,
            None,
            Some(1),
            WingRequest::TargetAssignment(TargetOrder::HoldFire),
        )
        .unwrap();
    assert_eq!(mission.actor(1).unwrap().controller().target(), None);

    // The owner revives in a new plane (member 3): it is crowned at once.
    mission.set_humans(vec![human(OWNER, 0), human(REVIVED, 3)]);
    mission.set_lead_claims(vec![claim(Some(REVIVED), false)]);
    let revived = plane(REVIVED, [0., 20_000., -3_000.], true);
    let humans = [lost.clone(), revived.clone()];
    let output = step(&mut mission, &humans, true);
    assert_eq!(mission.wing_leader(Side(1), 0), Some(REVIVED));
    assert!(!mission.wing_acting(Side(1), 0));
    let change = output.leadership[0];
    assert_eq!((change.leader, change.previous), (REVIVED, 1));
    assert!(change.reclaimed && !change.acting);
    // The opportunity is over; the followers re-form in member order.
    assert!(mission.opportunities().is_empty());
    assert_eq!(mission.actor(1).unwrap().wing_slot(), 1);
    assert_eq!(mission.actor(2).unwrap().wing_slot(), 2);
    assert!(!mission.actor(1).unwrap().identity().is_leader());
    // Wingman 1 flies back into formation at once; wingman 2 finishes its
    // fight first (John, 2026-10-09).
    assert_eq!(mission.actor(1).unwrap().controller().target(), None);
    assert_eq!(mission.reform_after(), [2]);
    assert_eq!(
        mission.actor(2).unwrap().controller().target(),
        Some(HOSTILE)
    );

    // The hostile goes down: wingman 2's fight is over and it re-forms.
    for _ in 0..600 {
        step(&mut mission, &humans, false);
        if mission.reform_after().is_empty() {
            break;
        }
    }
    assert!(
        mission.reform_after().is_empty(),
        "wingman 2 never re-formed"
    );
    assert_eq!(mission.actor(2).unwrap().controller().target(), None);
    assert_eq!(mission.wing_leader(Side(1), 0), Some(REVIVED));
}

#[test]
fn a_fresh_claim_is_a_new_lead_not_one_given_back() {
    let mut mission = wing();
    let lost = plane(OWNER, [0., 20_000., 0.], false);
    step(&mut mission, std::slice::from_ref(&lost), true);
    // Today's rule: the lead passed to wingman 1, and nobody stands in.
    assert_eq!(mission.wing_leader(Side(1), 0), Some(1));
    assert!(!mission.wing_acting(Side(1), 0));
    // A human now owns the lead (it waits to revive) and has not led.
    mission.set_lead_claims(vec![claim(None, true)]);
    step(&mut mission, std::slice::from_ref(&lost), true);
    assert!(mission.wing_acting(Side(1), 0), "the AI lead stands in now");
    mission.set_humans(vec![human(OWNER, 0), human(REVIVED, 3)]);
    mission.set_lead_claims(vec![claim(Some(REVIVED), true)]);
    let revived = plane(REVIVED, [0., 20_000., -3_000.], true);
    let output = step(&mut mission, &[lost, revived], true);
    assert_eq!(mission.wing_leader(Side(1), 0), Some(REVIVED));
    let change = output.leadership[0];
    assert!(
        !change.reclaimed && !change.acting,
        "a new lead: today's call"
    );
}

#[test]
fn without_claims_the_succession_is_todays_and_nothing_is_held() {
    let mut mission = wing();
    let lost = plane(OWNER, [0., 20_000., 0.], false);
    let output = step(&mut mission, std::slice::from_ref(&lost), true);
    let change = output.leadership[0];
    assert_eq!(
        (change.leader, change.acting, change.reclaimed),
        (1, false, false)
    );
    // A revived human is a wingman: the lead stays with wingman 1.
    mission.set_humans(vec![human(OWNER, 0), human(REVIVED, 3)]);
    let revived = plane(REVIVED, [0., 20_000., -3_000.], true);
    let output = step(&mut mission, &[lost, revived], true);
    assert!(output.leadership.is_empty());
    assert_eq!(mission.wing_leader(Side(1), 0), Some(1));
    assert!(mission.lead_claims().is_empty() && mission.reform_after().is_empty());
}

#[test]
fn claims_and_waiting_re_forms_round_trip_in_a_checkpoint() {
    use crate::checkpoint::{Models, round_trip};
    let mut mission = wing();
    let lost = plane(OWNER, [0., 20_000., 0.], false);
    mission.set_lead_claims(vec![claim(Some(OWNER), false)]);
    step(&mut mission, &[lost], true);
    mission.reform_after = vec![2];
    let mut models = Models::default();
    for actor in mission.actors() {
        models
            .insert(actor.identity().aircraft, actor.flight().import_model())
            .unwrap();
    }
    let restored = round_trip(&mission, &models).unwrap();
    assert_eq!(restored.lead_claims(), mission.lead_claims());
    assert_eq!(restored.reform_after(), [2]);
    assert!(restored.wing_acting(Side(1), 0));
}
