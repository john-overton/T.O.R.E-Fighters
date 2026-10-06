//! Orders to human wingmen and their replies on the crowd fixture
//! (docs/ARCHITECTURE.md, "Orders to human wingmen, and their replies", slice
//! F2-R). The friendly wing is plane 0 (seat 0, the lead), the human wingman
//! plane 1 (seat 1, member 1, "Red two") and AI planes 2 and 3. In the crowded
//! mission seats 2 and 3 fly the enemy leader and wingman, who are in another
//! flight on the other side and must hear none of it.

use super::crowd::*;
use super::datalink_assign_tests::{TARGET, mission};
use super::replies::Reply;
use super::*;
use crate::comms::journal::{Cause, Outcome, Source};
use crate::datalink::calls::{self, Addressee, Geometry};
use crate::seats::SeatCommand;
use tore_input::{PilotCommand, PilotInput, Switch};
use tore_sim::{
    ai::wing::{PlayerBreak, PlayerOrder},
    combat::live::Command,
};

/// The lead's seat and the human wingman's.
const LEAD: SeatId = SeatId(0);
const WING: SeatId = SeatId(1);
/// The tick the commands of a test are given: the lead's radar is on at 10 and
/// [`TARGET`] designated at 40, and the picture is published.
const AT: usize = 100;

type Script = [(usize, SeatId, SeatCommand)];

/// One tick: the lead's radar and designation, and the commands `script`
/// gives each seat at the tick.
fn step(world: &mut World, tick: usize, script: &Script, out: &mut TickOutput) {
    let input = inputs(world, |seat| {
        let mut pilot = PilotInput::default();
        let mut commands = Vec::new();
        if seat == LEAD {
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
    world.step(&input, out).unwrap();
}

/// What a run made, each with the tick it came in.
struct Run {
    world: World,
    radio: Vec<(usize, SeatId, comms::Call)>,
    lines: Vec<(usize, SeatId, String)>,
    voices: Vec<(usize, SeatId, Vec<&'static str>)>,
    /// The lead's place, the wingman's and the target's as the tick of the
    /// last command found them.
    places: Option<([f64; 3], [f64; 3])>,
}

/// Runs `world` to `until`, with the wingman's and the target's places as
/// they were when tick `AT` began.
fn run(mut world: World, script: &Script, until: usize) -> Run {
    let (mut radio, mut lines, mut voices) = (Vec::new(), Vec::new(), Vec::new());
    let mut places = None;
    for tick in 0..until {
        if tick == AT {
            let wingman = world.cockpits[1].flight.position;
            let target = world
                .ai_wings
                .as_ref()
                .unwrap()
                .mission()
                .actor(TARGET.0)
                .unwrap()
                .flight()
                .position;
            places = Some((wingman, target));
        }
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
            Cue::OrderVoice { seat, stems } => Some((tick, *seat, stems.clone())),
            _ => None,
        }));
    }
    Run {
        world,
        radio,
        lines,
        voices,
        places,
    }
}

impl Run {
    /// The reply calls and order calls seat `seat` heard; the rest of the
    /// radio's traffic (the tower, the crew, a crash) is not what these tests
    /// are about.
    fn heard(&self, seat: SeatId) -> Vec<&comms::Call> {
        self.radio
            .iter()
            .filter(|(_, who, _)| *who == seat)
            .map(|(_, _, call)| call)
            .filter(|call| matches!(call.origin.cause, Cause::Replied(_) | Cause::Order { .. }))
            .collect()
    }
    fn lines_of(&self, seat: SeatId) -> Vec<&str> {
        self.lines
            .iter()
            .filter(|(_, who, _)| *who == seat)
            .map(|(_, _, text)| text.as_str())
            .collect()
    }
}

fn reply(at: usize, seat: SeatId, reply: Reply) -> (usize, SeatId, SeatCommand) {
    (at, seat, SeatCommand::WingReply(reply))
}

fn order(at: usize, order: PlayerOrder) -> (usize, SeatId, SeatCommand) {
    (at, LEAD, SeatCommand::WingOrder(order))
}

fn stems(call: &comms::Call) -> Vec<&str> {
    call.stems.iter().map(String::as_str).collect()
}

#[test]
fn each_reply_reaches_the_flights_humans_and_no_one_else() {
    for (reply_kind, stem) in [
        (Reply::Engaging, Some("^ENGAGE")),
        (Reply::Winchester, None),
        (Reply::BingoFuel, Some("^BINGO")),
        (Reply::NeedHelp, Some("^OFFME")),
    ] {
        let run = run(crowded_mission(), &[reply(AT, WING, reply_kind)], AT + 5);
        // The recording's own text where the pack has it, else the key's.
        let text = reply_kind.phrase(&run.world.phrases).text;
        let text = text.as_str();
        // The lead hears its wingman under the wingman's place, as a line and
        // the recording; the wingman hears its own call as itself.
        let lead = run.heard(LEAD);
        assert_eq!(lead.len(), 1, "{reply_kind:?}");
        assert_eq!(lead[0].label, "Red two");
        assert_eq!(lead[0].text, text);
        assert_eq!(stems(lead[0]), stem.into_iter().collect::<Vec<_>>());
        assert_eq!(lead[0].kind, comms::Kind::Chatter);
        let own = run.heard(WING);
        assert_eq!(own.len(), 1, "{reply_kind:?}");
        assert_eq!(own[0].label, "YOU");
        assert_eq!(own[0].text, text);
        // The other flight's humans hear nothing, and the AI wingmen are not
        // seats.
        assert!(run.heard(SeatId(2)).is_empty(), "{reply_kind:?}");
        assert!(run.heard(SeatId(3)).is_empty(), "{reply_kind:?}");
        assert!(run.lines_of(WING).is_empty(), "no refusal line");
    }
}

#[test]
fn a_reply_says_the_recordings_own_text_when_it_was_imported() {
    let mut world = crowded_mission();
    world.phrases.insert("^ENGAGE".into(), "Engaging!".into());
    world
        .phrases
        .insert("^OFFME".into(), "Get this guy off me".into());
    let run = run(
        world,
        &[
            reply(AT, WING, Reply::Engaging),
            reply(AT + 300, WING, Reply::NeedHelp),
        ],
        AT + 310,
    );
    let said: Vec<_> = run
        .heard(LEAD)
        .iter()
        .map(|call| (call.text.as_str(), call.stems.clone()))
        .collect();
    assert_eq!(
        said,
        [
            ("Engaging!", vec!["^ENGAGE".to_owned()]),
            ("Get this guy off me", vec!["^OFFME".to_owned()]),
        ]
    );
}

#[test]
fn a_call_has_a_line_even_when_the_pack_gave_no_text() {
    // With no imported text the reply and the order say their own names.
    let mut world = crowded_mission();
    world.phrases.clear();
    let run = run(
        world,
        &[
            reply(AT, WING, Reply::Engaging),
            order(AT + 300, PlayerOrder::Break(PlayerBreak::Left)),
        ],
        AT + 310,
    );
    let lead: Vec<_> = run.heard(LEAD).iter().map(|c| c.text.clone()).collect();
    assert_eq!(lead, ["Engaging"]);
    let wingman: Vec<_> = run.heard(WING).iter().map(|c| c.text.clone()).collect();
    assert_eq!(wingman, ["Engaging", "Break left"]);
}

#[test]
fn a_plane_that_leads_its_flight_has_no_one_to_answer() {
    // The lead and the enemy leader both lead their flights.
    for (seat, flightmate) in [(LEAD, WING), (SeatId(2), SeatId(3))] {
        let run = run(
            crowded_mission(),
            &[reply(AT, seat, Reply::Winchester)],
            AT + 5,
        );
        assert_eq!(run.lines_of(seat), ["You lead this flight."]);
        for any in [LEAD, WING, SeatId(2), SeatId(3)] {
            assert!(run.heard(any).is_empty(), "nobody hears {seat:?}'s reply");
        }
        assert!(run.heard(flightmate).is_empty());
    }
}

#[test]
fn a_refused_reply_is_in_the_radio_journal_with_its_reason() {
    let mut run = run(
        crowded_mission(),
        &[
            reply(AT, LEAD, Reply::Engaging),
            reply(AT, WING, Reply::Engaging),
        ],
        AT + 5,
    );
    let journal = run.world.comms.take_journal();
    let replies: Vec<_> = journal
        .iter()
        .filter(|entry| matches!(entry.origin.cause, Cause::Replied(_)))
        .collect();
    // The lead's refusal, then the wingman's call queued and delivered.
    assert!(replies.iter().all(|e| e.source() == Source::Reply));
    assert!(replies.iter().any(|e| matches!(
        &e.outcome,
        Outcome::Refused(why) if why.to_string().contains("leads its flight")
    )));
    assert!(
        replies
            .iter()
            .any(|e| matches!(e.outcome, Outcome::Queued { .. }))
    );
    assert!(
        replies
            .iter()
            .any(|e| matches!(e.outcome, Outcome::Delivered { .. }))
    );
}

#[test]
fn radio_silence_drops_a_reply_for_the_seat_that_has_it_on() {
    // The lead turns radio silence on, then the wingman calls.
    let run = run(
        crowded_mission(),
        &[
            (5, LEAD, SeatCommand::RadioSilence),
            reply(AT, WING, Reply::Winchester),
        ],
        AT + 5,
    );
    assert!(run.heard(LEAD).is_empty(), "the lead is not listening");
    let own = run.heard(WING);
    assert_eq!(own.len(), 1, "the wingman is");
    assert_eq!(own[0].text, "Winchester");
}

#[test]
fn a_speaker_under_radio_silence_is_told_the_call_went_out() {
    let run = run(
        crowded_mission(),
        &[
            (5, WING, SeatCommand::RadioSilence),
            reply(AT, WING, Reply::BingoFuel),
        ],
        AT + 5,
    );
    // The lead still hears it; the speaker only reads that it was sent.
    assert_eq!(run.heard(LEAD).len(), 1);
    assert!(run.heard(WING).is_empty());
    assert!(
        run.lines_of(WING)
            .contains(&"Bingo fuel: sent (radio silence is on)")
    );
}

#[test]
fn a_seat_may_call_once_in_two_seconds() {
    let run = run(
        crowded_mission(),
        &[
            reply(AT, WING, Reply::Engaging),
            reply(AT + 10, WING, Reply::Winchester),
            reply(AT + 250, WING, Reply::Winchester),
        ],
        AT + 260,
    );
    let said: Vec<_> = run
        .heard(LEAD)
        .iter()
        .map(|call| stems(call).first().copied().unwrap_or("-"))
        .collect();
    assert_eq!(said, ["^ENGAGE", "-"], "the second came too soon");
    let waits = run
        .lines_of(WING)
        .iter()
        .filter(|line| **line == "Reply: wait a moment")
        .count();
    assert_eq!(waits, 1);
}

#[test]
fn a_reply_from_a_plane_that_is_down_is_refused() {
    let mut world = crowded_mission();
    world.cockpits[1].flight.crashed = true;
    let run = run(world, &[reply(AT, WING, Reply::NeedHelp)], AT + 5);
    assert!(run.heard(LEAD).is_empty());
    assert_eq!(run.lines_of(WING), ["No reply: your aircraft is down"]);
}

#[test]
fn a_reply_changes_nothing_the_wing_does() {
    // The AI takes nothing from a reply: the same run without it ends in the
    // same positions.
    let with = run(
        crowded_mission(),
        &[reply(AT, WING, Reply::Engaging)],
        AT + 60,
    );
    let without = run(crowded_mission(), &[], AT + 60);
    for plane in [2u32, 3] {
        let place = |run: &Run| {
            run.world
                .ai_wings
                .as_ref()
                .unwrap()
                .mission()
                .actor(plane)
                .unwrap()
                .flight()
                .position
        };
        assert_eq!(place(&with), place(&without), "plane {plane}");
    }
}

#[test]
fn a_reply_run_is_the_same_twice() {
    let script = [reply(AT, WING, Reply::BingoFuel)];
    let (a, b) = (
        run(crowded_mission(), &script, AT + 20),
        run(crowded_mission(), &script, AT + 20),
    );
    assert_eq!(a.radio, b.radio);
    assert_eq!(a.lines, b.lines);
}

// The order call.

#[test]
fn a_leads_break_order_reaches_its_human_wingman_as_a_call_and_nobody_else() {
    let run = run(
        crowded_mission(),
        &[order(AT, PlayerOrder::Break(PlayerBreak::Left))],
        AT + 5,
    );
    let heard = run.heard(WING);
    assert_eq!(heard.len(), 1);
    // The lead's own label and recording, with the text its name gives when
    // the recording's text was not imported, and never silenced.
    assert_eq!(heard[0].label, "Red one");
    assert_eq!(
        heard[0].text,
        run.world.phrases.get("^BREAKLF").cloned().unwrap()
    );
    assert_eq!(stems(heard[0]), ["^BREAKLF"]);
    assert_eq!(heard[0].kind, comms::Kind::Important);
    // The lead hears its own order voice, as before; the other flight and the
    // lead's own channel get no call.
    assert!(run.heard(LEAD).is_empty());
    assert!(run.heard(SeatId(2)).is_empty());
    assert!(run.heard(SeatId(3)).is_empty());
    let voice: Vec<_> = run
        .voices
        .iter()
        .filter(|(_, seat, _)| *seat == LEAD)
        .collect();
    assert_eq!(voice.len(), 1);
    assert_eq!(voice[0].2, ["^BREAKLF"]);
}

#[test]
fn an_order_to_one_ai_wingman_is_not_heard_by_the_human() {
    let run = run(
        crowded_mission(),
        &[
            (AT - 1, LEAD, SeatCommand::WingRecipient(Some(2))),
            order(AT, PlayerOrder::Break(PlayerBreak::Right)),
        ],
        AT + 5,
    );
    assert!(run.heard(WING).is_empty(), "wingman 3 only was addressed");
}

#[test]
fn an_order_to_the_human_alone_is_voiced_for_the_lead_and_heard_by_the_human() {
    let run = run(
        crowded_mission(),
        &[
            (AT - 1, LEAD, SeatCommand::WingRecipient(Some(1))),
            order(AT, PlayerOrder::Break(PlayerBreak::High)),
        ],
        AT + 5,
    );
    let heard = run.heard(WING);
    assert_eq!(heard.len(), 1);
    assert_eq!(stems(heard[0]), ["^BREAKHI"]);
    // No AI wingman took it, and the lead still says it.
    let voice: Vec<_> = run
        .voices
        .iter()
        .filter(|(_, seat, _)| *seat == LEAD)
        .collect();
    assert_eq!(voice.len(), 1);
    assert_eq!(voice[0].2, ["^BREAKHI"]);
}

#[test]
fn an_order_with_no_recording_is_a_text_call() {
    let run = run(crowded_mission(), &[order(AT, PlayerOrder::BugOut)], AT + 5);
    let heard = run.heard(WING);
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0].text, "Bug out");
    assert!(heard[0].stems.is_empty());
}

#[test]
fn an_attack_order_is_worded_from_the_human_wingmans_own_place() {
    let run = run(mission(), &[order(AT, PlayerOrder::EngageMyTarget)], AT + 5);
    let (wingman, target) = run.places.unwrap();
    let expected = calls::assignment_phrase(
        &run.world.phrases,
        Addressee::Flight(0),
        Geometry::between(wingman, target),
    );
    let heard = run.heard(WING);
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0].label, "Red one");
    assert_eq!(heard[0].text, expected.text);
    assert_eq!(heard[0].stems, expected.stems);
    assert_eq!(heard[0].kind, comms::Kind::Important);
    // The lead's own voice is from the first AI wingman's place, as in G3a.
    assert!(run.heard(LEAD).is_empty());
}

#[test]
fn an_attack_order_to_the_human_alone_is_said_from_its_place_for_both() {
    let run = run(
        mission(),
        &[
            (AT - 1, LEAD, SeatCommand::WingRecipient(Some(1))),
            order(AT, PlayerOrder::EngageMyTarget),
        ],
        AT + 5,
    );
    let (wingman, target) = run.places.unwrap();
    let expected = calls::assignment_phrase(
        &run.world.phrases,
        Addressee::Wingman(1),
        Geometry::between(wingman, target),
    );
    let heard = run.heard(WING);
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0].text, expected.text);
    let voice: Vec<_> = run
        .voices
        .iter()
        .filter(|(_, seat, _)| *seat == LEAD)
        .collect();
    assert_eq!(voice.len(), 1);
    assert_eq!(
        voice[0].2,
        calls::assignment_stems(
            &Default::default(),
            Addressee::Wingman(1),
            Geometry::between(wingman, target),
        )
    );
}

#[test]
fn a_blanket_attack_order_is_attack_bandits_to_the_human_too() {
    let run = run(
        mission(),
        &[order(AT, PlayerOrder::AttackOnContact)],
        AT + 5,
    );
    let heard = run.heard(WING);
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0].text, "Attack bandits");
    assert_eq!(stems(heard[0]), ["^ATTACK", "^BANDITS"]);
}

#[test]
fn a_sort_calls_a_human_wingman_it_dealt_a_bandit() {
    let run = run(mission(), &[order(AT, PlayerOrder::Sort)], AT + 5);
    let heard = run.heard(WING);
    assert_eq!(heard.len(), 1, "the human is dealt a bandit like the rest");
    assert!(heard[0].text.starts_with("Two, attack bandit, bearing "));
    assert_eq!(heard[0].label, "Red one");
}

#[test]
fn an_order_the_dead_human_cannot_hear_makes_no_call() {
    let mut world = crowded_mission();
    world.cockpits[1].flight.crashed = true;
    let run = run(
        world,
        &[order(AT, PlayerOrder::Break(PlayerBreak::Left))],
        AT + 5,
    );
    assert!(run.heard(WING).is_empty());
}

#[test]
fn an_order_run_is_the_same_twice() {
    let script = [order(AT, PlayerOrder::EngageMyTarget)];
    let (a, b) = (
        run(mission(), &script, AT + 20),
        run(mission(), &script, AT + 20),
    );
    assert_eq!(a.radio, b.radio);
    assert_eq!(a.voices, b.voices);
}
