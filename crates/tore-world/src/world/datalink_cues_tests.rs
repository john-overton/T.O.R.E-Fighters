//! The data link's cues on the crowd fixture (docs/ARCHITECTURE.md, "Flight
//! data link", slice G6): each seat's cockpit readout carries its share of the
//! picture, and two flightmates locking one aircraft warn the humans among
//! them once, with a line and a beep, and not again within ten seconds.

use super::crowd::{F_HUMAN, F_LEAD};
use super::datalink_tests::{fight_mission, step};
use super::*;
use crate::{
    combat::launcher,
    datalink::SORT_COOLDOWN_TICKS,
    readout::{CockpitReadout, LinkReadout},
};
use std::collections::BTreeMap;

fn readout_of(world: &World, plane: PlaneId) -> CockpitReadout {
    let seat = world
        .roster
        .plane(plane)
        .and_then(|p| match p.pilot {
            crate::seats::Pilot::Human(seat) => Some(seat),
            _ => None,
        })
        .expect("a human flies it");
    let cockpit = &world.cockpits[world.cockpit_of(seat).unwrap()];
    world
        .cockpit_readout(seat, launcher(&cockpit.flight))
        .unwrap()
}

#[test]
fn a_seats_readout_carries_its_planes_share_of_the_picture() {
    let mut world = fight_mission();
    let mut out = TickOutput::default();
    for tick in 0..600 {
        step(&mut world, tick, &mut out);
    }
    for plane in [F_LEAD, F_HUMAN] {
        let link = readout_of(&world, plane).link;
        assert!(link.radar && link.on_scope(), "the F/A-18D has a radar");
        assert_eq!(
            lock_marks_differ(&link, &world, plane),
            "",
            "plane {}",
            plane.0
        );
        // The two other planes of the wing, never the plane itself.
        let mates: Vec<u32> = link.mates.iter().map(|m| m.plane).collect();
        let expected: Vec<u32> = [0, 1, 2, 3].into_iter().filter(|p| *p != plane.0).collect();
        assert_eq!(mates, expected, "plane {}", plane.0);
    }
}

/// Why the readout's flightmate locks differ from what the picture holds for
/// `plane`, or an empty string.
fn lock_marks_differ(link: &LinkReadout, world: &World, plane: PlaneId) -> String {
    let me = world.datalink.member(plane.0).unwrap();
    // Every flightmate lock the picture holds appears as a locker bit.
    let mut expected: BTreeMap<u32, u16> = BTreeMap::new();
    for member in world.datalink.members() {
        if member.plane != plane.0
            && member.flight == me.flight
            && member.alive
            && let Some(lock) = world.datalink.lock(member.plane)
        {
            *expected.entry(lock.target).or_default() |= 1 << member.member;
        }
    }
    let held: BTreeMap<u32, u16> = link
        .marks
        .iter()
        .filter(|m| m.lockers != 0)
        .map(|m| (m.target, m.lockers))
        .collect();
    if held == expected {
        String::new()
    } else {
        format!("marks {held:?} against locks {expected:?}")
    }
}

/// Runs the fight, returning each tick's sort cues: the seat, the line and
/// whether the beep came with it.
fn sort_cues(ticks: usize) -> (World, Vec<(u64, SeatId, String, bool)>) {
    let mut world = fight_mission();
    let mut out = TickOutput::default();
    let mut found = Vec::new();
    for tick in 0..ticks {
        step(&mut world, tick, &mut out);
        let beeps: Vec<SeatId> = out
            .cues
            .iter()
            .filter_map(|cue| match cue {
                Cue::Radio { seat, call }
                    if call.stems == [crate::datalink::SORT_BEEP.to_string()] =>
                {
                    Some(*seat)
                }
                _ => None,
            })
            .collect();
        for cue in &out.cues {
            if let Cue::Message { seat, text } = cue
                && text.starts_with("Sort: ")
            {
                found.push((world.tick(), *seat, text.clone(), beeps.contains(seat)));
            }
        }
    }
    (world, found)
}

#[test]
fn flightmates_locking_one_aircraft_warn_the_humans_once_with_a_line_and_a_beep() {
    let (world, found) = sort_cues(1500);
    // Both humans designate the same bandit and lock it, so both are warned.
    assert!(!found.is_empty(), "no sort warning in the whole fight");
    for (tick, seat, text, beep) in &found {
        assert!(beep, "tick {tick}: the line has its beep");
        assert!(
            text.ends_with(" is locked on your target.") && text.starts_with("Sort: Red "),
            "{text}"
        );
        assert!(seat.0 <= 1, "only the friendly humans are in this flight");
    }
    // A seat is told at most once in ten seconds.
    for seat in [SeatId(0), SeatId(1)] {
        let ticks: Vec<u64> = found
            .iter()
            .filter(|(_, s, ..)| *s == seat)
            .map(|(tick, ..)| *tick)
            .collect();
        assert!(
            ticks
                .windows(2)
                .all(|pair| pair[1] - pair[0] >= SORT_COOLDOWN_TICKS),
            "seat {} was told at {ticks:?}",
            seat.0
        );
    }
    // Every warning is a pair the table holds or held.
    assert!(world.datalink.seat_warned().len() <= 2);
}

#[test]
fn two_runs_warn_alike() {
    let (_, first) = sort_cues(900);
    let (_, second) = sort_cues(900);
    assert_eq!(first, second);
}

/// The lead orders Engage my target to its flight: the assignments the merged
/// picture holds reach the displays as marks on the lead's readout and as the
/// assignment on each receiver's, and a wingman that locks the target it was
/// given is not a sort clash.
#[test]
fn an_assignment_reaches_the_displays_and_meant_locks_do_not_warn() {
    use super::datalink_assign_tests::{TARGET, mission, step as step_orders};
    use crate::seats::SeatCommand;
    use tore_sim::ai::wing::PlayerOrder;
    let mut world = mission();
    let order = [(60, SeatCommand::WingOrder(PlayerOrder::EngageMyTarget))];
    let mut warned = Vec::new();
    let mut checked = false;
    for tick in 0..600 {
        let mut out = TickOutput::default();
        step_orders(&mut world, tick, &order, &mut out);
        warned.extend(out.cues.iter().filter_map(|cue| match cue {
            Cue::Message { text, .. } if text.starts_with("Sort: ") => Some(text.clone()),
            _ => None,
        }));
        if tick == 100 {
            checked = true;
            let given: Vec<u32> = world.datalink.assignments().keys().copied().collect();
            assert!(!given.is_empty(), "the order assigned nobody");
            let lead = readout_of(&world, F_LEAD).link;
            let mark = lead
                .marks
                .iter()
                .find(|m| m.target == TARGET.0)
                .expect("the lead sees its flight's assignment");
            let bits = given
                .iter()
                .map(|plane| 1u16 << world.datalink.member(*plane).unwrap().member)
                .fold(0, |all, bit| all | bit);
            assert_eq!(mark.assigned_to, bits);
            assert_eq!(lead.assigned, None, "the lead gave it, it was not given");
            if let Some(row) = world.datalink.assignments().get(&F_HUMAN.0) {
                let own = readout_of(&world, F_HUMAN).link.assigned.expect("assigned");
                assert_eq!(
                    (own.target, own.by, own.acknowledged),
                    (row.target, row.by, row.acknowledged)
                );
            }
        }
    }
    assert!(checked);
    // The lead locked the target it assigned: no clash was announced for it.
    assert!(warned.is_empty(), "meant locks warned: {warned:?}");
}
