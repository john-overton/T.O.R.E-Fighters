//! The sort order on the crowd fixture (docs/ARCHITECTURE.md, "Flight data
//! link", slice G3c): the human lead's Alt+A deals each wingman a different
//! bandit from the picture, the lead's own target stays its own, the first AI
//! wingman's call plays at once and the next one 3.5 seconds later, and an
//! order from a plane that does not lead is turned away. Plane 0 (seat 0)
//! leads the friendly wing with the human plane 1 beside it and AI planes 2 and
//! 3 behind; the enemy wing is planes 4 to 7, all AI here, and the lead
//! designates plane 6 (so the sort leaves it to the lead).

use super::crowd::*;
use super::datalink_assign_tests::{TARGET, mission, step};
use super::*;
use crate::datalink::calls;
use crate::seats::SeatCommand;
use tore_sim::ai::wing::PlayerOrder;

/// The tick of the sort: the designation was made at 40 and the picture has
/// been published twice since.
const SORT: usize = 100;

const ORDER: SeatCommand = SeatCommand::WingOrder(PlayerOrder::Sort);

/// Runs to the sort tick, where the lead gives `before` and then the sort.
/// Returns the world and the sort tick's output.
fn sorted(before: &[(usize, SeatCommand)]) -> (World, TickOutput) {
    let mut world = mission();
    let mut out = TickOutput::default();
    let mut all = before.to_vec();
    all.push((SORT, ORDER));
    for tick in 0..=SORT {
        out = TickOutput::default();
        step(&mut world, tick, &all, &mut out);
    }
    (world, out)
}

fn targets_of(world: &World) -> Vec<(u32, u32)> {
    world
        .datalink
        .assignments()
        .iter()
        .map(|(plane, assignment)| (*plane, assignment.target))
        .collect()
}

#[test]
fn a_sort_deals_each_wingman_a_different_bandit_and_leaves_the_leads_target() {
    let (world, out) = sorted(&[]);
    assert!(matches!(
        &out.orders[..],
        [OrderReply {
            order: PlayerOrder::Sort,
            outcome: OrderOutcome::Given { message },
            ..
        }] if message == "Sort: 3 assigned"
    ));
    let held = targets_of(&world);
    assert_eq!(
        held.iter().map(|(plane, _)| *plane).collect::<Vec<_>>(),
        [1, 2, 3],
        "every wingman, the human too, and not the lead"
    );
    let mut targets: Vec<u32> = held.iter().map(|(_, target)| *target).collect();
    assert!(
        !targets.contains(&TARGET.0),
        "the lead's own designation stays its own"
    );
    targets.sort_unstable();
    targets.dedup();
    assert_eq!(targets.len(), 3, "three bandits, three wingmen: {held:?}");
    for assignment in world.datalink.assignments().values() {
        assert_eq!(assignment.order, PlayerOrder::Sort);
        assert_eq!(assignment.by, F_LEAD.0);
    }
    // The AI wingmen were ordered to attack the bandit each was dealt.
    let wings = world.ai_wings.as_ref().unwrap();
    for (plane, target) in &held {
        if *plane != F_HUMAN.0 {
            assert_eq!(
                wings.mission().actor(*plane).unwrap().controller().target(),
                Some(*target),
                "plane {plane}"
            );
        }
    }
}

#[test]
fn the_first_call_plays_at_once_and_the_next_one_3_5_seconds_later() {
    let mut world = mission();
    let mut out = TickOutput::default();
    for tick in 0..SORT {
        step(&mut world, tick, &[], &mut out);
    }
    // What each AI wingman is told, from where it flies as the sort finds it.
    let geometry = |world: &World, plane: u32, target: u32| {
        let wings = world.ai_wings.as_ref().unwrap();
        let at = |id| wings.mission().actor(id).unwrap().flight().position;
        calls::Geometry::between(at(plane), at(target))
    };
    out = TickOutput::default();
    let orders = [(SORT, ORDER)];
    step(&mut world, SORT, &orders, &mut out);
    let held = targets_of(&world);
    let target_of = |plane| held.iter().find(|(p, _)| *p == plane).unwrap().1;
    // Plane 2 is the first AI wingman, member 2: "Three".
    let first = out
        .cues
        .iter()
        .find_map(|cue| match cue {
            Cue::OrderVoice { seat, stems } if *seat == SeatId(0) => Some(stems.clone()),
            _ => None,
        })
        .expect("the first call is the order voice");
    assert_eq!(first[0], "^NUM03");
    assert_eq!(
        first.len(),
        calls::assignment_stems(
            &Default::default(),
            calls::Addressee::Wingman(2),
            geometry(&world, 2, target_of(2))
        )
        .len()
    );
    assert!(first.contains(&"^ATTACK") && first.contains(&"^BEARING"));
    // The next call is queued for plane 3 ("Four"), 3.5 seconds on.
    let sent = world.tick();
    let mut heard = None;
    for tick in SORT + 1..SORT + 600 {
        out = TickOutput::default();
        step(&mut world, tick, &[], &mut out);
        let radio = out.cues.iter().find_map(|cue| match cue {
            Cue::Radio { seat, call }
                if *seat == SeatId(0) && call.stems.contains(&"^ATTACK".into()) =>
            {
                Some(call.clone())
            }
            _ => None,
        });
        if let Some(call) = radio {
            heard = Some((world.tick() - sent, call));
            break;
        }
    }
    let (after, call) = heard.expect("the second call is heard");
    assert!(
        (418..=424).contains(&after),
        "3.5 seconds is 420 ticks, heard after {after}"
    );
    assert_eq!(call.stems[0], "^NUM04");
    assert!(
        call.text.starts_with("Four, attack bandit"),
        "{}",
        call.text
    );
    assert_eq!(call.label, "Red one", "the lead speaks it");
    assert_eq!(call.kind, comms::Kind::Important);
}

#[test]
fn one_wingman_addressed_gets_the_sort_alone() {
    let (world, out) = sorted(&[(SORT, SeatCommand::WingRecipient(Some(3)))]);
    assert!(matches!(
        &out.orders[..],
        [OrderReply {
            outcome: OrderOutcome::Given { message },
            ..
        }] if message == "Sort: 1 assigned"
    ));
    let held = targets_of(&world);
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].0, 3);
    assert_ne!(held[0].1, TARGET.0);
}

#[test]
fn a_wingman_does_not_sort() {
    // The human wingman, plane 1, does not lead the wing.
    let mut world = mission();
    let mut out = TickOutput::default();
    let step_inputs = |world: &World, tick: usize| {
        inputs(world, |seat| {
            let commands = if seat == SeatId(1) && tick == SORT {
                vec![ORDER]
            } else {
                Vec::new()
            };
            SeatInput {
                commands,
                ..SeatInput::default()
            }
        })
    };
    for tick in 0..=SORT {
        out = TickOutput::default();
        let input = step_inputs(&world, tick);
        world.step(&input, &mut out).unwrap();
    }
    assert!(matches!(
        &out.orders[..],
        [OrderReply {
            seat: SeatId(1),
            outcome: OrderOutcome::Refused { message },
            ..
        }] if message == "Wing order unavailable: you are not leading your wing"
    ));
    assert!(world.datalink.assignments().is_empty());
}

#[test]
fn a_sort_with_nobody_to_deal_to_is_turned_away() {
    // Before the picture is published nothing is known: no bandits.
    let mut world = mission();
    let mut out = TickOutput::default();
    let orders = [(5, ORDER)];
    for tick in 0..=5 {
        out = TickOutput::default();
        step(&mut world, tick, &orders, &mut out);
    }
    assert!(matches!(
        &out.orders[..],
        [OrderReply {
            outcome: OrderOutcome::Refused { .. },
            ..
        }]
    ));
    assert!(world.datalink.assignments().is_empty());
}

#[test]
fn a_sort_is_the_same_in_every_run() {
    let (a, _) = sorted(&[]);
    let (b, _) = sorted(&[]);
    assert_eq!(targets_of(&a), targets_of(&b));
}
