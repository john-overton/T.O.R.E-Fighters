//! An AI lead's shares and sorts reaching a human wingman, on the crowd fixture
//! (docs/ARCHITECTURE.md, "Flight data link", slice G11): the enemy wing's AI
//! lead, plane 4, deals targets to its wingmen, and plane 5, which seat 1
//! flies as the lead's number Two, is one of them. The seat gets the
//! assignment (the cues are the readout's) and hears the call, "Two, attack
//! bandit, bearing ...", on its own channel; nothing is ordered of the human's
//! aircraft. The other enemy wingmen (planes 6 and 7) are blinded and moved
//! 80 nm back, so only the picture can tell them anything.

use super::crowd::*;
use super::datalink_assign_tests::step;
use super::*;
use crate::datalink::Entry;
use tore_sim::ai::{
    weapon_service::StoreCapability,
    wing::{PlayerOrder, TargetOrder, WingRequest},
};

const LEAD: u32 = 4;
const HUMAN: u32 = 5;
const SEAT: SeatId = SeatId(1);

/// The enemy wing led by AI plane 4 with seat 1 flying plane 5 beside it.
fn human_two() -> World {
    // The fixture of the other data link tests, armed the same way, but seat
    // 1 takes the enemy lead's number Two instead of the friendly wing's.
    let mut world = ai_mission();
    let mission = world.ai_wings.as_mut().unwrap().mission_mut();
    for id in 1..=7 {
        let actor = mission.actor_mut(id).unwrap();
        let stations = actor.stations_mut();
        stations[0].guided = false;
        stations[0].capability = StoreCapability::GUN;
        stations[1].guided = true;
        stations[1].capability = StoreCapability::AIR_TO_AIR_MISSILE;
    }
    world.take_plane(SEAT, PlaneId(HUMAN)).unwrap();
    let mission = world.ai_wings.as_mut().unwrap().mission_mut();
    for id in [6, 7] {
        let actor = mission.actor_mut(id).unwrap();
        actor.blind();
        actor.flight_mut().position[0] += 80. * 6_076.;
    }
    world
}

fn run(world: &mut World, from: usize, to: usize) -> (Vec<Entry>, Vec<comms::Call>) {
    let (mut journal, mut heard) = (Vec::new(), Vec::new());
    for tick in from..to {
        let mut out = TickOutput::default();
        step(world, tick, &[], &mut out);
        journal.extend(world.datalink.take_journal());
        for cue in &out.cues {
            if let Cue::Radio { seat, call } = cue
                && *seat == SEAT
                && call.stems.contains(&"^ATTACK".into())
            {
                heard.push(call.clone());
            }
        }
    }
    (journal, heard)
}

fn given(journal: &[Entry]) -> Vec<(u32, u32, PlayerOrder)> {
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
fn an_ai_lead_shares_its_target_with_the_human_wingman_and_calls_it() {
    let mut world = human_two();
    let (journal, heard) = run(&mut world, 0, 300);
    // The lead and the first wingman by member number make the allowance of
    // two, and the first wingman is the human.
    let shared = given(&journal);
    assert_eq!(shared.len(), 1, "{journal:?}");
    let (plane, target, order) = shared[0];
    assert_eq!((plane, order), (HUMAN, PlayerOrder::EngageMyTarget));
    let held = world.datalink.assignment(HUMAN).expect("the assignment");
    assert_eq!((held.target, held.by), (target, LEAD));
    // The seat's cockpit readout carries it, as for a human lead's order.
    let readout = world.datalink.readout(HUMAN);
    assert_eq!(
        readout.assigned.map(|a| (a.target, a.by)),
        Some((target, LEAD))
    );
    // The AI wingmen behind it are left alone: the allowance is full.
    assert!(world.datalink.assignment(6).is_none());
    // The call is the assignment call, on the human's own channel, from Two.
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert!(
        heard[0].text.starts_with("Two, attack bandit, bearing"),
        "{}",
        heard[0].text
    );
    assert_eq!(heard[0].kind, comms::Kind::Important);
    // Nothing was ordered of the human's aircraft: the AI does not fly it.
    let wings = world.ai_wings.as_ref().unwrap().mission();
    assert!(wings.actor(HUMAN).is_none());
}

#[test]
fn a_sort_deals_the_human_wingman_a_bandit_with_the_ai_wingmen() {
    let mut world = human_two();
    run(&mut world, 0, 300);
    drop_the_leads_target(&mut world);
    let (journal, heard) = run(&mut world, 300, 1_300);
    let sorted = given(&journal);
    let planes: Vec<u32> = sorted.iter().map(|(plane, ..)| *plane).collect();
    assert_eq!(planes, [5, 6, 7], "{journal:?}");
    assert!(sorted.iter().all(|(.., order)| *order == PlayerOrder::Sort));
    // Three wingmen share at most two a bandit.
    let mut targets: Vec<u32> = sorted.iter().map(|(_, target, _)| *target).collect();
    targets.sort_unstable();
    targets.dedup();
    assert!(targets.len() >= 2, "{sorted:?}");
    let own = world.datalink.assignment(HUMAN).unwrap();
    assert_eq!((own.order, own.by), (PlayerOrder::Sort, LEAD));
    // The seat heard its own call, and the others the AI wingmen's too (the
    // flight's humans hear the flight's net), in the order of the deal.
    assert_eq!(heard.len(), 3, "{heard:?}");
    let spoken: Vec<&str> = heard
        .iter()
        .map(|call| call.text.split(',').next().unwrap())
        .collect();
    assert_eq!(spoken, ["Two", "Three", "Four"]);
    assert!(
        heard[0].text.starts_with("Two, attack bandit"),
        "{}",
        heard[0].text
    );
}

#[test]
fn the_same_fight_deals_the_human_the_same_every_run() {
    let once = || {
        let mut world = human_two();
        let (mut journal, _) = run(&mut world, 0, 300);
        drop_the_leads_target(&mut world);
        journal.extend(run(&mut world, 300, 360).0);
        given(&journal)
    };
    assert_eq!(once(), once());
}
