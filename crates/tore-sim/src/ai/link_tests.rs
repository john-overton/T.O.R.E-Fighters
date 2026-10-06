//! The engagement table against the live scans it replaced (slice G1,
//! docs/ARCHITECTURE.md "How the AI reads the picture"): the audit in
//! [`super::super::link::audit`] compares them at every actor's turn of every
//! tick, here on the observation tests' 15 against 15 fixture with its
//! scripted damage, crashes, landing orders and human handoff, serially and
//! with workers.

use super::observation_tests::{compare_step, executors, fixture, scripted_changes};
use super::*;
use crate::ai::link::{self, Engagements, audit};

/// Ticks each seeded fight runs: the scripted changes take the first sixteen,
/// the rest let targets change hands as the wings close.
const TICKS: u64 = 600;
/// Ticks of the quick run in the normal suite.
const SHORT_TICKS: u64 = 240;

/// Each wing's members in actor order, as (index, id, side, wing).
fn rows(mission: &AiMission) -> Vec<(usize, u32, crate::ai::targeting::Side, u8)> {
    mission
        .actors
        .iter()
        .enumerate()
        .map(|(index, a)| (index, a.id(), a.identity.side, a.identity.wing))
        .collect()
}

/// Turns at which an actor read a wing member's target chosen earlier in the
/// same tick: the member stepped before it and changed target this tick. Only
/// the decision-order table reproduces these reads.
fn fresh_reads(before: &[Option<u32>], mission: &AiMission) -> u64 {
    let rows = rows(mission);
    let mut count = 0;
    for &(reader, id, side, wing) in &rows {
        if !mission.actors[reader].alive() {
            continue;
        }
        count += rows
            .iter()
            .filter(|&&(earlier, other, s, w)| {
                earlier < reader
                    && other != id
                    && s == side
                    && w == wing
                    && before.get(earlier).copied().flatten()
                        != link::engagement_of(&mission.actors[earlier])
            })
            .count() as u64;
    }
    count
}

/// One seeded fight of `ticks` ticks under the audit, the reference stepped
/// serially and the copy with `workers`, their complete states compared every
/// tick. Returns the audit's report and the same-tick reads it covered.
fn audited_fight(workers: &tore_workers::Executor, seed: u64, ticks: u64) -> (audit::Report, u64) {
    let mut reference = fixture(30, seed);
    let mut actual = reference.clone();
    let (mut reference_human, mut actual_human) = (None, None);
    let mut fresh = 0;
    audit::start();
    for tick in 0..ticks {
        scripted_changes(&mut reference, tick, &mut reference_human);
        scripted_changes(&mut actual, tick, &mut actual_human);
        let before: Vec<_> = reference.actors.iter().map(link::engagement_of).collect();
        // Once the human has actor 8, it holds a lock on an enemy (slice G2):
        // its wing's table gains a row the audit expects after the actors'.
        if reference_human.is_some() {
            reference.set_link(locks(&[(8, 16)]));
            actual.set_link(locks(&[(8, 16)]));
        }
        compare_step(
            &mut reference,
            &mut actual,
            workers,
            reference_human.as_ref(),
        )
        .unwrap();
        // The handoff removes and reinserts an actor between steps; the
        // count needs the same actor order before and after.
        if reference.actors.len() == before.len() {
            fresh += fresh_reads(&before, &reference);
        }
    }
    let report = audit::finish();
    assert!(
        reference.actor(2).is_some_and(|a| !a.alive()),
        "the crash killed actor 2"
    );
    assert!(
        reference.actor(8).is_some(),
        "the human handed actor 8 back"
    );
    // Both missions audit every actor's turn, the dead included.
    assert!(report.wing_checks >= 2 * 26 * ticks, "{seed}: {report:?}");
    (report, fresh)
}

/// The audit saw the cases that matter, not just empty lists.
fn assert_exercised(seed: u64, report: audit::Report, fresh: u64) {
    assert!(report.wing_attacking > 0, "{seed}: {report:?}");
    assert!(report.human_attacking > 0, "{seed}: {report:?}");
    assert!(report.leader_checks > 0, "{seed}: {report:?}");
    assert!(report.leader_targets > 0, "{seed}: {report:?}");
    assert!(fresh > 0, "{seed}: no same-tick read was exercised");
}

#[test]
fn engagement_table_equals_the_live_scan_on_fifteen_against_fifteen() {
    // A shuffled worker pool against the serial reference: the table is built
    // on the calling thread whatever prepares the observations.
    let (report, fresh) = audited_fight(&tore_workers::Executor::shuffled(31), 7, SHORT_TICKS);
    assert_exercised(7, report, fresh);
}

/// Slow (about four minutes in a debug build): for the full suite. Serial,
/// four workers and a shuffled pool, two seeds, ten simulated seconds each.
#[test]
#[ignore = "slow: the engagement audit's full run, for the full suite"]
fn engagement_table_equals_the_live_scan_on_every_executor_and_seed() {
    let workers: Vec<_> = executors()
        .into_iter()
        .enumerate()
        .filter(|(n, _)| [0, 3, 6].contains(n))
        .map(|(_, w)| w)
        .collect();
    assert_eq!(workers.len(), 3);
    for workers in &workers {
        for seed in [7, 12345] {
            let (report, fresh) = audited_fight(workers, seed, TICKS);
            assert_exercised(seed, report, fresh);
        }
    }
}

#[test]
fn audit_is_off_unless_started() {
    assert_eq!(audit::finish(), audit::Report::default());
    let mut mission = fixture(10, 7);
    let world = super::observation_tests::snapshot(&mission, None);
    mission
        .step_with_surface(
            &world,
            &tests::flat,
            &super::observation_tests::surface,
            TimeOfDay(0),
        )
        .unwrap();
    assert_eq!(audit::finish(), audit::Report::default());
}

#[test]
fn the_table_holds_living_actors_targets_by_wing() {
    let mut mission = fixture(10, 12345);
    for tick in 0..240 {
        let world = super::observation_tests::snapshot(&mission, None);
        mission
            .step_with_surface(
                &world,
                &tests::flat,
                &super::observation_tests::surface,
                TimeOfDay(tick),
            )
            .unwrap();
    }
    let targeted: Vec<u32> = mission
        .actors
        .iter()
        .filter(|a| a.controller.target().is_some())
        .map(AiActor::id)
        .collect();
    assert!(targeted.len() >= 2, "the fight chose targets: {targeted:?}");
    let mut table = Engagements::new(&mission.actors);
    for actor in &mission.actors {
        assert_eq!(table.target(actor.id()), actor.controller.target());
        let others: Vec<u32> = mission
            .actors
            .iter()
            .filter(|a| {
                a.id() != actor.id()
                    && a.identity.side == actor.identity.side
                    && a.identity.wing == actor.identity.wing
            })
            .filter_map(|a| a.controller.target())
            .collect();
        assert_eq!(
            table.wing_targets(actor.id(), actor.identity.side, actor.identity.wing),
            others
        );
    }
    // A dead actor attacks nothing and holds no lock, once its row is
    // rewritten.
    let index = mission
        .actors
        .iter()
        .position(|a| a.id() == targeted[0])
        .unwrap();
    mission.actors[index].flight.crashed = true;
    assert_eq!(
        table.target(targeted[0]),
        mission.actors[index].controller.target()
    );
    table.decided(index, &mission.actors[index]);
    assert_eq!(table.target(targeted[0]), None);
    assert_eq!(link::engagement_of(&mission.actors[index]), None);
    assert_eq!(link::lock_of(&mission.actors[index]), None);
}

// Slice G2: the humans' locked targets in the table.

use crate::ai::link::{HumanEngagement, LinkInput};
use crate::ai::targeting::Side;

fn human(id: u32, side: u32, wing: u8, member: u8) -> HumanMember {
    HumanMember {
        id,
        side: Side(side),
        wing,
        member,
        pilot_alive: false,
    }
}

fn locks(pairs: &[(u32, u32)]) -> LinkInput {
    LinkInput {
        humans: pairs
            .iter()
            .map(|&(plane, target)| HumanEngagement { plane, target })
            .collect(),
        ..LinkInput::default()
    }
}

#[test]
fn a_humans_lock_joins_its_own_wing_after_the_actors() {
    let mission = fixture(10, 12345);
    let (side, wing) = {
        let first = &mission.actors[0].identity;
        (first.side, first.wing)
    };
    let other_wing = mission
        .actors
        .iter()
        .map(|a| (a.identity.side, a.identity.wing))
        .find(|&sw| sw != (side, wing))
        .unwrap();
    let humans = [
        HumanMember {
            id: 9_001,
            side,
            wing,
            member: 7,
            pilot_alive: false,
        },
        HumanMember {
            id: 9_002,
            side: other_wing.0,
            wing: other_wing.1,
            member: 7,
            pilot_alive: false,
        },
    ];
    let plain = Engagements::new(&mission.actors);
    let input = locks(&[(9_001, 4_242), (9_002, 4_343), (7_777, 4_444)]);
    let table = Engagements::new(&mission.actors).with_humans(&humans, &input);
    let reader = mission.actors[0].id();
    // An actor reads its own wing's actors first, then the human's lock.
    let mut expected = plain.wing_targets(reader, side, wing);
    expected.push(4_242);
    assert_eq!(table.wing_targets(reader, side, wing), expected);
    assert_eq!(table.human_wing_targets(side, wing), vec![4_242]);
    assert_eq!(
        table.human_wing_targets(other_wing.0, other_wing.1),
        vec![4_343]
    );
    // A lock from a plane the mission knows no wing for counts for nobody.
    for (s, w) in [(side, wing), other_wing] {
        assert!(!table.wing_targets(reader, s, w).contains(&4_444));
    }
    // The human is no actor: asking for its target finds no row to rewrite.
    assert_eq!(table.target(reader), plain.target(reader));
    // A plane the mission also flies as an actor keeps its actor row.
    let doubled = [human(reader, side.0, wing, 0)];
    let table = Engagements::new(&mission.actors).with_humans(&doubled, &locks(&[(reader, 4_242)]));
    assert_eq!(table, plain);
    // No input, no change.
    assert_eq!(
        Engagements::new(&mission.actors).with_humans(&humans, &LinkInput::default()),
        plain
    );
}

/// A wingman (actor 1) with two bandits in view and a human of its wing
/// (plane 90): returns the bandit it chooses when the human locks `lock`.
fn chosen_with_human_lock(lock: Option<u32>) -> Option<u32> {
    use tests::{enable_test_radar, flat, object, perception_actor, visible_object};
    let mut mission = AiMission::new();
    let mut wingman = perception_actor(crate::ai::Experience::Ace);
    enable_test_radar(&mut wingman);
    mission.push(wingman);
    mission.set_humans(vec![human(90, 1, 0, 3)]);
    // Bandit 2 is the nearer by 3,000 ft, inside the 10,000 ft penalty.
    let near = [0., 20_000., 6_000.];
    let far = [0., 20_000., 9_000.];
    for tick in 0..30 {
        let actor = mission.actor(1).unwrap();
        let world = vec![
            object(actor, 1),
            visible_object(actor, 2, near),
            visible_object(actor, 3, far),
        ];
        mission.set_link(locks(
            &lock.map(|t| (90, t)).into_iter().collect::<Vec<_>>(),
        ));
        mission.step(&world, &flat, TimeOfDay(tick)).unwrap();
    }
    mission.actor(1).unwrap().controller().target()
}

#[test]
fn a_wingman_ranks_the_players_locked_bandit_with_the_penalty() {
    // Alone, the wingman takes the nearer bandit.
    assert_eq!(chosen_with_human_lock(None), Some(2));
    // The player locks the nearer one: it costs 10,000 ft more, so the wingman
    // takes the other. The player locking the far one changes nothing.
    assert_eq!(chosen_with_human_lock(Some(2)), Some(3));
    assert_eq!(chosen_with_human_lock(Some(3)), Some(2));
}

#[test]
fn the_step_consumes_the_link_input() {
    use tests::{enable_test_radar, flat, object, perception_actor, visible_object};
    let mut mission = AiMission::new();
    let mut wingman = perception_actor(crate::ai::Experience::Ace);
    enable_test_radar(&mut wingman);
    mission.push(wingman);
    mission.set_humans(vec![human(90, 1, 0, 3)]);
    mission.set_link(locks(&[(90, 2)]));
    assert_eq!(mission.link, locks(&[(90, 2)]));
    let actor = mission.actor(1).unwrap();
    let world = vec![
        object(actor, 1),
        visible_object(actor, 2, [0., 20_000., 6_000.]),
    ];
    mission.step(&world, &flat, TimeOfDay(0)).unwrap();
    assert_eq!(mission.link, LinkInput::default());
}

// Slice G3b: an assignment reaches the AI through the picture.

use crate::ai::link::Pursuit;
use crate::ai::wing::{TargetId, TargetOrder, WingRequest};

const NM: f64 = crate::sensors::FEET_PER_NAUTICAL_MILE;

/// 12 nm east and 20 nm north of the wingman: outside its radar.
const AHEAD_FAR: [f64; 3] = [12. * NM, 20_000., 20. * NM];
/// Dead ahead at 6,000 ft: well inside every weapon's envelope.
const AHEAD_CLOSE: [f64; 3] = [0., 20_000., 6_000.];

/// An aircraft of side 2 that no sensor of the mission can perceive.
fn blind_bandit(actor: &AiActor, id: u32, position: [f64; 3]) -> WorldObject {
    let mut bandit = tests::object(actor, 2);
    bandit.id = id;
    bandit.position = position;
    bandit
}

fn pursuit(target: u32, position: [f64; 3], velocity: [f64; 3], observed: u64) -> Pursuit {
    Pursuit {
        receiver: 1,
        target,
        position,
        velocity,
        observed,
    }
}

#[test]
fn a_track_becomes_a_flagged_target_view_carried_forward_to_now() {
    let actor = tests::perception_actor(crate::ai::Experience::Ace);
    let bandit = blind_bandit(&actor, 2, [0.; 3]);
    // Seen at tick 100 flying east at 600 ft/s, climbing 60 ft/s.
    let track = pursuit(2, [1_000., 20_000., 5_000.], [600., 60., 0.], 100);
    // Two seconds later (240 ticks at 120 Hz), the track has moved on.
    let view = track.view(340, Side(1), &bandit, true, 1).unwrap();
    assert_eq!(view.id, 2);
    assert_eq!(view.side, Side(2));
    assert_eq!(view.position, [2_200., 20_120., 5_000.]);
    assert!((view.heading_deg - 90.).abs() < 1e-9, "{view:?}");
    assert!(view.pitch_deg > 5. && view.pitch_deg < 6., "{view:?}");
    assert!((view.speed.0 - 600.0f64.hypot(60.)).abs() < 1e-9);
    assert!(view.link_track, "a link track is flagged");
    assert!(view.valid && view.type_allowed && view.is_aircraft);
    assert!(!view.sensor_supported && !view.terrain_blocked);
    assert_eq!((view.seeker_eligible, view.wing_attackers), (true, 1));
    // A report from the future is not carried backward.
    let early = track.view(50, Side(1), &bandit, true, 0).unwrap();
    assert_eq!(early.position, [1_000., 20_000., 5_000.]);
}

#[test]
fn a_track_of_a_friend_a_wreck_or_another_aircraft_makes_no_view() {
    let actor = tests::perception_actor(crate::ai::Experience::Ace);
    let track = pursuit(2, [0.; 3], [0.; 3], 0);
    let bandit = blind_bandit(&actor, 2, [0.; 3]);
    assert!(track.view(0, Side(1), &bandit, true, 0).is_some());
    // A friend of the receiver is no target.
    assert!(track.view(0, Side(2), &bandit, true, 0).is_none());
    // A different aircraft than the one assigned.
    assert!(
        track
            .view(
                0,
                Side(1),
                &WorldObject {
                    id: 3,
                    ..bandit.clone()
                },
                true,
                0
            )
            .is_none()
    );
    // Destroyed, dead, or not an aircraft.
    for broken in [
        WorldObject {
            destroyed: true,
            ..bandit.clone()
        },
        WorldObject {
            alive: false,
            ..bandit.clone()
        },
        WorldObject {
            is_aircraft: false,
            ..bandit.clone()
        },
    ] {
        assert!(track.view(0, Side(1), &broken, true, 0).is_none());
    }
}

/// One ordered wingman (actor 1, ordered to attack aircraft 2) flown for
/// `ticks` against a bandit at `bandit_at` that the mission's sensors never
/// perceive, standing still. `track` says whether the
/// picture reports it each tick. Returns the mission and what the last
/// step's targets were, and whether it ever launched.
fn flown_blind(
    track: bool,
    ticks: u64,
    bandit_at: [f64; 3],
) -> (AiMission, Vec<crate::ai::controller::TargetView>, bool) {
    use tests::{enable_test_radar, flat, object, perception_actor};
    let mut mission = AiMission::new();
    let mut wingman = perception_actor(crate::ai::Experience::Ace);
    enable_test_radar(&mut wingman);
    mission.push(wingman);
    let velocity = [0., 0., 0.];
    let order = WingRequest::TargetAssignment(TargetOrder::ConcreteTarget(TargetId(2)));
    mission.order(1, order).unwrap().unwrap();
    let mut targets = Vec::new();
    let mut launched = false;
    for tick in 0..ticks {
        let actor = mission.actor(1).unwrap();
        let world = vec![object(actor, 1), blind_bandit(actor, 2, bandit_at)];
        let mut input = LinkInput::default();
        if track {
            input.pursuits.push(pursuit(2, bandit_at, velocity, tick));
        }
        mission.set_link(input);
        let out = mission.step(&world, &flat, TimeOfDay(tick)).unwrap();
        launched |= !out.launches.is_empty();
        targets = mission.actor(1).unwrap().trace.targets.clone();
    }
    (mission, targets, launched)
}

#[test]
fn an_assigned_wingman_flies_toward_a_track_it_cannot_see_and_does_not_fire() {
    let (mission, targets, launched) = flown_blind(true, 3_000, AHEAD_FAR);
    let actor = mission.actor(1).unwrap();
    // It keeps the order's target, which its own sensors never held.
    assert_eq!(actor.controller().target(), Some(2));
    assert_eq!(actor.controller().ordered_target(), Some(2));
    assert!(matches!(&targets[..], [view] if view.id == 2 && view.link_track));
    // It turned from north toward the east-northeast track.
    let position = actor.flight().position;
    assert!(position[0] > 3_000., "flew toward the east: {position:?}");
    assert!(position[2] > 10_000., "and closed: {position:?}");
    // Nothing it holds locks or fires on a target only the link holds.
    assert_eq!(link::lock_of(actor), None);
    assert!(!launched, "it fired on a target only the link held");
    // The link target is the actor's engagement, which the wing sees.
    assert_eq!(link::engagement_of(actor), Some(2));
}

#[test]
fn a_bandit_in_perfect_firing_position_is_never_fired_on_from_the_link_alone() {
    // The control: the same bandit, seen by the wingman's radar, is fired on.
    // Here the picture alone holds it, and the wingman holds the order's
    // target, closes on it and never launches or locks.
    let (mission, targets, launched) = flown_blind(true, 3_000, AHEAD_CLOSE);
    let actor = mission.actor(1).unwrap();
    assert!(!launched);
    assert_eq!(link::lock_of(actor), None);
    assert_eq!(actor.controller().target(), Some(2));
    assert!(matches!(&targets[..], [view] if view.link_track));
    assert!(
        !matches!(
            actor.controller().weapon_phase(),
            crate::ai::weapon_service::Phase::Tracking | crate::ai::weapon_service::Phase::Fire
        ),
        "{:?}",
        actor.controller().weapon_phase()
    );
}

#[test]
fn without_a_track_the_order_is_lost_as_it_was_before() {
    let (mission, targets, launched) = flown_blind(false, 600, AHEAD_FAR);
    assert!(!launched);
    let actor = mission.actor(1).unwrap();
    assert_eq!(actor.controller().target(), None);
    assert!(targets.is_empty());
    assert!(actor.flight().position[0].abs() < 3_000.);
}

#[test]
fn a_track_for_an_aircraft_the_order_did_not_name_is_not_flown() {
    use tests::{enable_test_radar, flat, object, perception_actor};
    let mut mission = AiMission::new();
    let mut wingman = perception_actor(crate::ai::Experience::Ace);
    enable_test_radar(&mut wingman);
    mission.push(wingman);
    // No order at all: the picture's assignment alone moves nothing.
    for tick in 0..60 {
        let actor = mission.actor(1).unwrap();
        let world = vec![
            object(actor, 1),
            blind_bandit(actor, 2, [12. * NM, 20_000., 20. * NM]),
        ];
        mission.set_link(LinkInput {
            pursuits: vec![pursuit(2, [12. * NM, 20_000., 20. * NM], [0.; 3], tick)],
            ..LinkInput::default()
        });
        mission.step(&world, &flat, TimeOfDay(tick)).unwrap();
    }
    let actor = mission.actor(1).unwrap();
    assert_eq!(actor.controller().target(), None);
    assert!(actor.trace.targets.is_empty());
}

#[test]
fn once_its_own_sensors_hold_the_target_the_wingman_fires_on_it_itself() {
    use tests::{enable_test_radar, flat, object, perception_actor, visible_object};
    let mut mission = AiMission::new();
    let mut wingman = perception_actor(crate::ai::Experience::Ace);
    enable_test_radar(&mut wingman);
    mission.push(wingman);
    let order = WingRequest::TargetAssignment(TargetOrder::ConcreteTarget(TargetId(2)));
    mission.order(1, order).unwrap().unwrap();
    let ahead = [0., 20_000., 6_000.];
    let mut views = Vec::new();
    let mut launched = false;
    for tick in 0..3_000 {
        let actor = mission.actor(1).unwrap();
        // The bandit sits in the wingman's radar, and the picture reports it too.
        let world = vec![object(actor, 1), visible_object(actor, 2, ahead)];
        mission.set_link(LinkInput {
            pursuits: vec![pursuit(2, ahead, [0.; 3], tick)],
            ..LinkInput::default()
        });
        let out = mission.step(&world, &flat, TimeOfDay(tick)).unwrap();
        launched |= !out.launches.is_empty();
        views.extend(
            mission
                .actor(1)
                .unwrap()
                .trace
                .targets
                .iter()
                .filter(|t| t.id == 2)
                .map(|t| t.link_track),
        );
    }
    assert!(!views.is_empty(), "the wingman perceived the bandit");
    assert!(
        views.iter().all(|link| !link),
        "its own observation replaces the link track"
    );
    assert!(
        launched,
        "and an aircraft that holds the target itself fires"
    );
}
