//! An AI lead's data link work on the crowd fixture (docs/ARCHITECTURE.md,
//! "Flight data link", slice G4): the enemy wing's lead, plane 4, gives its
//! idle wingmen targets and the picture records and voices them, a new pair of
//! locks hands the AI member a yield, and a call heard by a human of the lead's
//! flight comes 3.5 seconds apart. The enemy wingmen (planes 5, 6 and 7) are
//! blinded and moved 80 nm back, so they are idle when the lead takes its
//! target and only the picture can tell them anything.

use super::datalink_assign_tests::{mission, step};
use super::*;
use crate::comms::journal::{Outcome, Reason};
use crate::datalink::{Entry, LeadAssignment};
use tore_sim::ai::wing::{PlayerOrder, TargetOrder, WingRequest};

const LEAD: u32 = 4;
const WINGMEN: [u32; 3] = [5, 6, 7];

/// The fight with the enemy wingmen far back and blind.
fn back_wing() -> World {
    let mut world = mission();
    let mission = world.ai_wings.as_mut().unwrap().mission_mut();
    for id in WINGMEN {
        let actor = mission.actor_mut(id).unwrap();
        actor.blind();
        actor.flight_mut().position[0] += 80. * 6_076.;
    }
    world
}

fn run(world: &mut World, from: usize, to: usize) -> Vec<Entry> {
    let mut journal = Vec::new();
    for tick in from..to {
        let mut out = TickOutput::default();
        step(world, tick, &[], &mut out);
        journal.extend(world.datalink.take_journal());
    }
    journal
}

fn assigned(journal: &[Entry]) -> Vec<(u32, u32, PlayerOrder)> {
    journal
        .iter()
        .filter_map(|entry| match *entry {
            Entry::Assign {
                plane,
                target,
                by,
                order,
                ..
            } if by == LEAD => Some((plane, target, order)),
            _ => None,
        })
        .collect()
}

fn target_of(world: &World, plane: u32) -> Option<u32> {
    world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(plane)
        .unwrap()
        .controller()
        .target()
}

/// The lead loses its target: it takes one again within a few ticks.
fn drop_the_leads_target(world: &mut World) {
    let mission = world.ai_wings.as_mut().unwrap().mission_mut();
    mission
        .order(
            LEAD,
            WingRequest::TargetAssignment(TargetOrder::FreeSelection),
        )
        .unwrap()
        .unwrap();
}

#[test]
fn an_ai_lead_shares_its_target_with_one_idle_wingman_and_says_so() {
    let mut world = back_wing();
    let journal = run(&mut world, 0, 300);
    // The lead and the first wingman make the allowance of two.
    let given = assigned(&journal);
    assert_eq!(given.len(), 1, "{journal:?}");
    let (plane, target, order) = given[0];
    assert_eq!((plane, order), (5, PlayerOrder::EngageMyTarget));
    assert_eq!(target_of(&world, LEAD), Some(target));
    assert_eq!(target_of(&world, 5), Some(target));
    assert_eq!(target_of(&world, 6), None);
    let held = world.datalink.assignment(5).unwrap();
    assert_eq!((held.target, held.by), (target, LEAD));
    // The wingman holds it by link while its own sensors hold nothing.
    let wings = world.ai_wings.as_ref().unwrap();
    assert_eq!(
        wings
            .mission()
            .actor(5)
            .unwrap()
            .controller()
            .ordered_target(),
        Some(target)
    );
}

#[test]
fn the_call_is_the_assignment_call_and_goes_unheard_by_another_side() {
    let mut world = back_wing();
    run(&mut world, 0, 300);
    let calls: Vec<_> = world
        .comms
        .journal()
        .entries()
        .filter(|entry| entry.stems.contains(&"^ATTACK".to_string()))
        .collect();
    assert_eq!(calls.len(), 1);
    let call = calls[0];
    // Wingman 5 is "Two" of the enemy flight; the lead speaks.
    assert!(
        call.text.starts_with("Two, attack bandit, bearing"),
        "{}",
        call.text
    );
    assert_eq!(call.origin.speaker, Some(LEAD));
    assert_eq!(call.outcome, Outcome::Unheard(Reason::EnemyFlight));
}

#[test]
fn a_lead_that_commits_again_sorts_the_flight_from_the_picture() {
    let mut world = back_wing();
    run(&mut world, 0, 300);
    // The lead loses its target and takes one again: the picture holds all
    // four friendly aircraft by now, so it sorts instead of sharing.
    drop_the_leads_target(&mut world);
    let journal = run(&mut world, 300, 360);
    let lead_target = target_of(&world, LEAD).expect("the lead took a target again");
    let given = assigned(&journal);
    assert_eq!(given.len(), 3, "{journal:?}");
    assert!(
        given
            .iter()
            .all(|(_, _, order)| *order == PlayerOrder::Sort)
    );
    let mut targets: Vec<u32> = given.iter().map(|(_, target, _)| *target).collect();
    assert!(!targets.contains(&lead_target), "{given:?}");
    targets.sort_unstable();
    targets.dedup();
    assert!(
        targets.len() >= 2,
        "three wingmen share at most two a bandit: {given:?}"
    );
    for plane in WINGMEN {
        assert_eq!(
            world.datalink.assignment(plane).map(|a| a.order),
            Some(PlayerOrder::Sort)
        );
    }
    // A third commit inside the thirty seconds does not sort again.
    drop_the_leads_target(&mut world);
    let again = run(&mut world, 360, 420);
    assert!(
        assigned(&again)
            .iter()
            .all(|(_, _, order)| *order != PlayerOrder::Sort),
        "{again:?}"
    );
}

#[test]
fn the_same_fight_gives_the_same_assignments_every_run() {
    let once = || {
        let mut world = back_wing();
        let mut journal = run(&mut world, 0, 300);
        drop_the_leads_target(&mut world);
        journal.extend(run(&mut world, 300, 420));
        assigned(&journal)
    };
    assert_eq!(once(), once());
}

// The call, heard by the humans of the lead's flight.

#[test]
fn the_seats_of_the_leads_flight_hear_the_call_and_the_next_one_3_5_seconds_later() {
    // Plane 2, an AI member of the friendly flight, gives wingman 3 two
    // targets in one tick: the humans in seats 0 and 1 hear both, the second
    // 3.5 seconds after the first, in the lead's label.
    let mut world = mission();
    run(&mut world, 0, 60);
    let given = [6, 7].map(|target| LeadAssignment {
        lead: 2,
        receiver: 3,
        target,
        order: PlayerOrder::Sort,
    });
    world.voice_lead_assignments(&given);
    let sent = world.tick();
    let mut heard: Vec<(SeatId, u64, comms::Call)> = Vec::new();
    for tick in 60..60 + 600 {
        let mut out = TickOutput::default();
        step(&mut world, tick, &[], &mut out);
        for cue in &out.cues {
            if let Cue::Radio { seat, call } = cue
                && call.stems.contains(&"^ATTACK".into())
            {
                heard.push((*seat, world.tick() - sent, call.clone()));
            }
        }
    }
    for seat in [SeatId(0), SeatId(1)] {
        let calls: Vec<_> = heard.iter().filter(|(s, ..)| *s == seat).collect();
        assert_eq!(calls.len(), 2, "seat {seat:?}: {heard:?}");
        assert!(calls[0].1 < 30, "the first is said at once");
        assert!(
            (415..=425).contains(&(calls[1].1 - calls[0].1)),
            "3.5 seconds is 420 ticks: {} and {}",
            calls[0].1,
            calls[1].1
        );
        assert_eq!(calls[0].2.label, "Red three", "the lead speaks");
        assert!(
            calls[0].2.text.starts_with("Four, attack bandit"),
            "{}",
            calls[0].2.text
        );
        assert_eq!(calls[0].2.kind, comms::Kind::Important);
    }
}

#[test]
fn a_call_in_a_flight_nobody_hears_is_journaled_unheard() {
    let mut world = mission();
    run(&mut world, 0, 10);
    // Plane 4 leads the enemy wing; no human flies in it here.
    world.voice_lead_assignments(&[LeadAssignment {
        lead: 4,
        receiver: 5,
        target: 0,
        order: PlayerOrder::EngageMyTarget,
    }]);
    let calls: Vec<_> = world
        .comms
        .journal()
        .entries()
        .filter(|entry| entry.stems.contains(&"^ATTACK".to_string()))
        .collect();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].outcome, Outcome::Unheard(Reason::EnemyFlight));
}

// The yield.

#[test]
fn a_yield_reaches_the_ai_member_and_never_a_human() {
    let mut world = mission();
    run(&mut world, 0, 10);
    // Plane 3 is an AI wingman; plane 1 is a human.
    world.apply_yields(&[(3, 6), (1, 6), (99, 6)]);
    let wings = world.ai_wings.as_ref().unwrap().mission();
    let held = wings.actor(3).unwrap().yields();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].target, 6);
    assert!(wings.actor(2).unwrap().yields().is_empty());
}
