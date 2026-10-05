//! Assignments and their calls on the crowd fixture (docs/ARCHITECTURE.md,
//! "Flight data link", slice G3a): a human lead's attack orders become
//! assignments, the order voice says the assignment call from the wingman's
//! place, the other orders clear them, and the mission ends them. Plane 0 (seat
//! 0) leads the friendly wing with plane 1 (seat 1) beside it and AI planes 2
//! and 3 behind; the enemy wing is planes 4 to 7, all AI here.

use super::crowd::*;
use super::*;
use crate::datalink::{ClearReason, Entry, calls};
use crate::seats::SeatCommand;
use tore_input::{PilotCommand, PilotInput, Switch};
use tore_sim::{
    ai::{weapon_service::StoreCapability, wing::PlayerOrder},
    combat::live::Command,
};

/// The enemy the lead designates: an AI aircraft of the other side.
const TARGET: PlaneId = E_AI[0];
/// The tick the lead's radar is on and the target designated.
const DESIGNATE: usize = 40;

/// The fight of `datalink_tests`: the friendly wing is the lead, a second human
/// and two AI wingmen, armed the way a real mission arms them.
fn mission() -> World {
    let mut world = ai_mission();
    let wings = world.ai_wings.as_mut().unwrap();
    for id in 1..=7 {
        let actor = wings.mission_mut().actor_mut(id).unwrap();
        let stations = actor.stations_mut();
        stations[0].guided = false;
        stations[0].capability = StoreCapability::GUN;
        stations[1].guided = true;
        stations[1].capability = StoreCapability::AIR_TO_AIR_MISSILE;
    }
    world.take_plane(SeatId(1), F_HUMAN).unwrap();
    world
}

/// Steps one tick. The lead turns its radar on, designates [`TARGET`] and
/// gives `orders` (seat commands) at the tick each names.
fn step(world: &mut World, tick: usize, orders: &[(usize, SeatCommand)], out: &mut TickOutput) {
    let step_inputs = inputs(world, |seat| {
        let mut pilot = PilotInput::default();
        let mut commands = Vec::new();
        if seat == SeatId(0) {
            if tick == 10 {
                pilot.commands.push(PilotCommand::Set(Switch::Radar, true));
            }
            if tick == DESIGNATE {
                commands.push(SeatCommand::Combat(Command::DesignateTarget(TARGET.0)));
            }
            commands.extend(
                orders
                    .iter()
                    .filter(|(at, _)| *at == tick)
                    .map(|(_, command)| *command),
            );
        }
        SeatInput {
            pilot,
            commands,
            ..SeatInput::default()
        }
    });
    world.step(&step_inputs, out).unwrap();
}

/// Runs to the order's tick and one more, returning the world and the output
/// of the tick the order was given.
fn order_at(order: SeatCommand, tick: usize) -> (World, TickOutput) {
    let mut world = mission();
    let mut out = TickOutput::default();
    let orders = [(tick, order)];
    for at in 0..=tick {
        out = TickOutput::default();
        step(&mut world, at, &orders, &mut out);
    }
    (world, out)
}

/// The stems of the order voice the tick played for seat 0.
fn voice(out: &TickOutput) -> Vec<&'static str> {
    out.cues
        .iter()
        .find_map(|cue| match cue {
            Cue::OrderVoice { seat, stems } if *seat == SeatId(0) => Some(stems.clone()),
            _ => None,
        })
        .expect("the order voice")
}

fn position(world: &World, plane: u32) -> [f64; 3] {
    world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(plane)
        .unwrap()
        .flight()
        .position
}

/// What the call must say for the wingman at `receiver`, `addressee` named.
fn stems(world: &World, addressee: calls::Addressee, receiver: u32) -> Vec<&'static str> {
    let geometry = calls::Geometry::between(position(world, receiver), position(world, TARGET.0));
    calls::assignment_stems(&Default::default(), addressee, geometry)
}

const ENGAGE: usize = 60;

#[test]
fn a_blanket_attack_order_is_attack_bandits_and_assigns_nothing() {
    let (world, out) = order_at(SeatCommand::WingOrder(PlayerOrder::AttackOnContact), ENGAGE);
    assert_eq!(voice(&out), ["^ATTACK", "^BANDITS"]);
    assert!(world.datalink.assignments().is_empty());
}

#[test]
fn engage_my_target_assigns_and_says_the_call_from_the_first_wingman() {
    let mut world = mission();
    let mut out = TickOutput::default();
    // The call is measured from the wingman as the order finds it, before the
    // tick moves anything.
    for tick in 0..ENGAGE {
        step(&mut world, tick, &[], &mut out);
    }
    let expected = stems(&world, calls::Addressee::Flight(0), 2);
    let order = [(ENGAGE, SeatCommand::WingOrder(PlayerOrder::EngageMyTarget))];
    out = TickOutput::default();
    step(&mut world, ENGAGE, &order, &mut out);
    assert!(matches!(
        &out.orders[..],
        [OrderReply {
            outcome: OrderOutcome::Given { .. },
            ..
        }]
    ));
    assert_eq!(voice(&out), expected);
    assert_eq!(expected[0], "^RED", "the whole flight is addressed");
    assert!(expected.contains(&"^BEARING") && expected.contains(&"^ANGELS"));
    // The wingmen that took the order, and the human it addressed, hold the
    // assignment the lead gave; the lead holds none.
    let held = world.datalink.assignments().clone();
    assert!(held.contains_key(&F_HUMAN.0), "the human wingman");
    assert!(held.contains_key(&F_AI[0].0), "the first AI wingman");
    assert!(!held.contains_key(&F_LEAD.0));
    for (plane, assignment) in &held {
        assert_eq!(assignment.target, TARGET.0, "plane {plane}");
        assert_eq!(assignment.by, F_LEAD.0);
        assert_eq!(assignment.order, PlayerOrder::EngageMyTarget);
        assert_eq!(
            assignment.tick,
            world.tick() - 1,
            "given in the command phase"
        );
    }
    // Each assignment is journaled, and the order's voice holds the channel.
    let assigned = world
        .datalink
        .take_journal()
        .iter()
        .filter(|entry| matches!(entry, Entry::Assign { .. }))
        .count();
    assert_eq!(assigned, held.len());
    assert!(
        !world
            .comms
            .channel_free(SeatId(0), world.tick() as f64 / 120.)
    );
}

#[test]
fn an_order_to_one_wingman_names_its_place_and_engage_from_formation_calls_too() {
    for order in [
        PlayerOrder::EngageMyTarget,
        PlayerOrder::EngageFromFormation,
    ] {
        let mut world = mission();
        let mut out = TickOutput::default();
        for tick in 0..ENGAGE {
            step(&mut world, tick, &[], &mut out);
        }
        // Member 3 is the fourth aircraft of the flight: "Four".
        let expected = stems(&world, calls::Addressee::Wingman(3), 3);
        let orders = [
            (ENGAGE, SeatCommand::WingRecipient(Some(3))),
            (ENGAGE, SeatCommand::WingOrder(order)),
        ];
        out = TickOutput::default();
        step(&mut world, ENGAGE, &orders, &mut out);
        assert_eq!(voice(&out), expected, "{order:?}");
        assert_eq!(expected[0], "^NUM04");
        let held = world.datalink.assignments();
        assert_eq!(
            held.keys().copied().collect::<Vec<_>>(),
            [F_AI[1].0],
            "{order:?}"
        );
        assert_eq!(held[&F_AI[1].0].order, order);
    }
}

#[test]
fn the_orders_that_give_something_else_clear_the_assignments() {
    for order in [
        PlayerOrder::Disengage,
        PlayerOrder::ProtectMe,
        PlayerOrder::AttackOnContact,
    ] {
        let mut world = mission();
        let mut out = TickOutput::default();
        let orders = [
            (ENGAGE, SeatCommand::WingOrder(PlayerOrder::EngageMyTarget)),
            (ENGAGE + 30, SeatCommand::WingOrder(order)),
        ];
        for tick in 0..=ENGAGE + 30 {
            step(&mut world, tick, &orders, &mut out);
        }
        assert!(
            world.datalink.assignments().is_empty(),
            "{order:?} left {:?}",
            world.datalink.assignments()
        );
        let cleared: Vec<_> = world
            .datalink
            .take_journal()
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Clear { why, .. } => Some(why),
                _ => None,
            })
            .collect();
        assert!(!cleared.is_empty());
        assert!(cleared.iter().all(|why| *why == ClearReason::Order));
    }
}

#[test]
fn an_assignment_ends_when_the_target_is_destroyed() {
    let mut world = mission();
    let mut out = TickOutput::default();
    let orders = [(ENGAGE, SeatCommand::WingOrder(PlayerOrder::EngageMyTarget))];
    for tick in 0..=ENGAGE + 5 {
        step(&mut world, tick, &orders, &mut out);
    }
    assert!(!world.datalink.assignments().is_empty());
    world.datalink.take_journal();
    // The target is shot down: the combat row and the AI actor's hit points go.
    let row = world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|row| row.id == TARGET.0)
        .unwrap();
    row.hp = 0;
    for tick in ENGAGE + 6..ENGAGE + 40 {
        step(&mut world, tick, &orders, &mut out);
    }
    assert!(
        world.datalink.assignments().is_empty(),
        "{:?}",
        world.datalink.assignments()
    );
    let why: Vec<_> = world
        .datalink
        .take_journal()
        .into_iter()
        .filter_map(|entry| match entry {
            Entry::Clear { why, target, .. } => {
                assert_eq!(target, TARGET.0);
                Some(why)
            }
            _ => None,
        })
        .collect();
    assert!(!why.is_empty());
    assert!(
        why.iter().all(|why| *why == ClearReason::TargetLost),
        "{why:?}"
    );
}

#[test]
fn a_wingman_that_locks_the_assigned_target_acknowledges_it_once() {
    let mut world = mission();
    let mut out = TickOutput::default();
    let orders = [(ENGAGE, SeatCommand::WingOrder(PlayerOrder::EngageMyTarget))];
    let mut journal = Vec::new();
    for tick in 0..2400 {
        step(&mut world, tick, &orders, &mut out);
        journal.extend(world.datalink.take_journal());
    }
    let acknowledged: Vec<(u32, u32)> = journal
        .iter()
        .filter_map(|entry| match *entry {
            Entry::Acknowledge { plane, target, .. } => Some((plane, target)),
            _ => None,
        })
        .collect();
    assert!(
        !acknowledged.is_empty(),
        "nobody locked the assigned target"
    );
    // Once for each wingman: no pair appears twice.
    let distinct: std::collections::BTreeSet<_> = acknowledged.iter().copied().collect();
    assert_eq!(distinct.len(), acknowledged.len(), "{acknowledged:?}");
    assert!(acknowledged.iter().all(|&(_, target)| target == TARGET.0));
}

#[test]
fn the_assignments_are_the_same_in_two_runs() {
    let run = || {
        let mut world = mission();
        let mut out = TickOutput::default();
        let orders = [
            (ENGAGE, SeatCommand::WingOrder(PlayerOrder::EngageMyTarget)),
            (ENGAGE + 600, SeatCommand::WingOrder(PlayerOrder::Disengage)),
        ];
        let mut journal = Vec::new();
        for tick in 0..1200 {
            step(&mut world, tick, &orders, &mut out);
            journal.extend(world.datalink.take_journal());
        }
        (format!("{:?}", world.datalink.assignments()), journal)
    };
    assert_eq!(run(), run());
}
