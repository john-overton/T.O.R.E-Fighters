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
