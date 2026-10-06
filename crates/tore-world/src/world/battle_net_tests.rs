//! The battle net on the crowd fixture (docs/ARCHITECTURE.md, "Flight data
//! link", slice G8): Alt+N turns a seat's monitor on and off, and a human
//! lead's assignment calls (Engage my target and the sort) are repeated, with
//! the flight colour in front, to the monitoring seats of its side in other
//! flights and to nobody else. Plane 0 (seat 0) leads the friendly wing with AI
//! planes 2 and 3 behind it; plane 1 (seat 1) is moved into a flight of its own
//! ("Blue"), so it is another flight of the lead's side. The enemy wing is all AI.

use super::crowd::*;
use super::datalink_assign_tests::{TARGET, mission};
use super::*;
use crate::seats::{Pilot, Roster, SeatCommand, Slot};
use tore_input::{PilotCommand, PilotInput, Switch};
use tore_sim::{
    ai::{
        launch::{Side, WingId},
        wing::PlayerOrder,
    },
    combat::live::Command,
};

/// The tick the lead gives its order, after its radar is on and the target
/// designated (as the assignment tests do) and the picture is published.
const ORDER: usize = 100;

/// The mission with plane 1, the second human, in a flight of its own.
fn blue_mission() -> World {
    let mut world = mission();
    let planes: Vec<_> = world.roster.planes().to_vec();
    let crew = |seat: SeatId| world.roster.seat(seat).and_then(|seat| seat.crew);
    let mut humans = Vec::new();
    let mut ai = Vec::new();
    for plane in planes {
        let slot = if plane.id == F_HUMAN {
            Slot {
                wing: WingId {
                    side: Side::Friendly,
                    index: 1,
                },
                member: 0,
            }
        } else {
            plane.slot
        };
        match plane.pilot {
            Pilot::Human(seat) => humans.push((plane.id, slot, seat, crew(seat))),
            _ => ai.push((plane.id, slot)),
        }
    }
    world.roster = Roster::with_humans(humans, ai);
    world
}

/// One tick. The lead's radar is on at 10 and [`TARGET`] designated at 40;
/// `script` gives each seat's commands at the tick.
fn step(
    world: &mut World,
    tick: usize,
    script: &[(usize, SeatId, SeatCommand)],
    out: &mut TickOutput,
) {
    let inputs = inputs(world, |seat| {
        let mut pilot = PilotInput::default();
        let mut commands = Vec::new();
        if seat == SeatId(0) {
            if tick == 10 {
                pilot.commands.push(PilotCommand::Set(Switch::Radar, true));
            }
            if tick == 40 {
                commands.push(SeatCommand::Combat(Command::DesignateTarget(TARGET.0)));
            }
        }
        commands.extend(
            script
                .iter()
                .filter(|(at, who, _)| *at == tick && *who == seat)
                .map(|(_, _, command)| *command),
        );
        SeatInput {
            pilot,
            commands,
            ..SeatInput::default()
        }
    });
    world.step(&inputs, out).unwrap();
}

/// Every tick's output from 0 to `until`, the calls and lines of each tick kept
/// with its number.
struct Run {
    world: World,
    radio: Vec<(usize, SeatId, comms::Call)>,
    lines: Vec<(usize, SeatId, String)>,
    voices: Vec<(usize, SeatId)>,
}

fn run(script: &[(usize, SeatId, SeatCommand)], until: usize) -> Run {
    let mut world = blue_mission();
    let (mut radio, mut lines, mut voices) = (Vec::new(), Vec::new(), Vec::new());
    for tick in 0..until {
        let mut out = TickOutput::default();
        step(&mut world, tick, script, &mut out);
        radio.extend(
            radio_of(&out)
                .into_iter()
                .map(|(seat, call)| (tick, seat, call)),
        );
        lines.extend(
            messages_of(&out)
                .into_iter()
                .map(|(seat, text)| (tick, seat, text)),
        );
        voices.extend(out.cues.iter().filter_map(|cue| match cue {
            Cue::OrderVoice { seat, .. } => Some((tick, *seat)),
            _ => None,
        }));
    }
    Run {
        world,
        radio,
        lines,
        voices,
    }
}

const BLUE: SeatId = SeatId(1);
const MONITOR: (usize, SeatId, SeatCommand) = (1, BLUE, SeatCommand::BattleNet);
const ENGAGE: (usize, SeatId, SeatCommand) = (
    ORDER,
    SeatId(0),
    SeatCommand::WingOrder(PlayerOrder::EngageMyTarget),
);
const SORT: (usize, SeatId, SeatCommand) =
    (ORDER, SeatId(0), SeatCommand::WingOrder(PlayerOrder::Sort));

fn net_calls(run: &Run) -> Vec<&(usize, SeatId, comms::Call)> {
    run.radio
        .iter()
        .filter(|(_, _, call)| call.net == comms::Net::Battle)
        .collect()
}

#[test]
fn alt_n_toggles_the_monitor_and_tells_the_pilot() {
    let run = run(&[MONITOR, (50, BLUE, SeatCommand::BattleNet)], 60);
    let said: Vec<_> = run
        .lines
        .iter()
        .filter(|(_, seat, text)| *seat == BLUE && text.contains("attle net"))
        .map(|(tick, _, text)| (*tick, text.as_str()))
        .collect();
    assert_eq!(said, [(1, "Monitoring battle net"), (50, "Battle net off")]);
    assert!(!run.world.comms.monitors_battle(BLUE));
    assert!(!run.world.comms.monitors_battle(SeatId(0)));
}

#[test]
fn a_leads_engage_order_is_repeated_to_the_monitor_of_another_flight_and_no_one_else() {
    let run = run(&[MONITOR, ENGAGE], ORDER + 30);
    assert!(
        run.voices.contains(&(ORDER, SeatId(0))),
        "the lead's own voice"
    );
    let net = net_calls(&run);
    assert_eq!(net.len(), 1, "{:?}", run.radio);
    let (tick, seat, call) = net[0];
    assert_eq!((*tick, *seat), (ORDER, BLUE), "at once, to the monitor");
    assert_eq!(call.label, "Net Red one");
    assert_eq!(call.kind, comms::Kind::Important);
    assert_eq!(call.stems[0], "^RED", "the flight colour first");
    assert_ne!(
        call.stems[1], "^RED",
        "an attack call to the whole flight already names it"
    );
    assert!(call.stems.contains(&"^ATTACK".into()) && call.stems.contains(&"^BEARING".into()));
    // The lead does not hear itself on the net, and the monitor's own flight
    // would not: no other seat has a battle call.
    assert!(
        run.radio
            .iter()
            .all(|(_, seat, call)| { call.net == comms::Net::Wing || *seat == BLUE })
    );
}

#[test]
fn a_call_to_one_wingman_gets_the_flight_colour_before_the_wingman() {
    let run = run(
        &[
            MONITOR,
            (50, SeatId(0), SeatCommand::WingRecipient(Some(2))),
            ENGAGE,
        ],
        ORDER + 30,
    );
    let net = net_calls(&run);
    assert_eq!(net.len(), 1);
    let call = &net[0].2;
    assert_eq!(
        &call.stems[..2],
        ["^RED", "^NUM03"],
        "Red, then wingman Three"
    );
    assert!(call.text.starts_with("Red, "), "{}", call.text);
}

#[test]
fn nobody_monitoring_means_nothing_on_the_net_and_the_order_is_as_before() {
    let run = run(&[ENGAGE], ORDER + 30);
    assert!(net_calls(&run).is_empty());
    assert!(run.voices.contains(&(ORDER, SeatId(0))));
    // The journal has no battle net line either.
    let mut world = run.world;
    assert!(
        world
            .comms
            .take_journal()
            .iter()
            .all(|entry| entry.net == comms::Net::Wing)
    );
}

#[test]
fn a_monitor_that_switched_off_before_the_order_hears_nothing() {
    let run = run(
        &[MONITOR, (50, BLUE, SeatCommand::BattleNet), ENGAGE],
        ORDER + 30,
    );
    assert!(net_calls(&run).is_empty());
}

#[test]
fn a_blanket_attack_order_names_no_target_and_is_not_repeated() {
    let run = run(
        &[
            MONITOR,
            (
                ORDER,
                SeatId(0),
                SeatCommand::WingOrder(PlayerOrder::AttackOnContact),
            ),
        ],
        ORDER + 30,
    );
    assert!(run.voices.contains(&(ORDER, SeatId(0))));
    assert!(net_calls(&run).is_empty(), "it assigns nothing");
}

#[test]
fn every_call_of_a_sort_is_repeated_with_the_timing_of_the_lead_s_own() {
    let run = run(&[MONITOR, SORT], ORDER + 600);
    let net = net_calls(&run);
    assert_eq!(net.len(), 2, "two AI wingmen, two calls: {net:?}");
    assert_eq!((net[0].0, net[0].1), (ORDER, BLUE));
    assert!(
        net[0].2.text.starts_with("Red, Three, attack bandit"),
        "{}",
        net[0].2.text
    );
    let later = net[1].0 - ORDER;
    assert!(
        (418..=424).contains(&later),
        "3.5 seconds is 420 ticks, heard after {later}"
    );
    assert!(
        net[1].2.text.starts_with("Red, Four, attack bandit"),
        "{}",
        net[1].2.text
    );
    assert_eq!(net[1].2.label, "Net Red one");
}
