//! Stage H slice H7's tests on whole worlds (docs/formats/checkpoint.md): the
//! real radio objects of the tick mission round trip, and the comms, wing
//! status and radio sections restore into a twin that was never given the
//! calls waiting in the original, which then delivers them on identically.
//! The coders themselves, and their step-on tests, are beside the types.

use super::checkpoint_scenarios::{self, Scenario};
use super::{Cue, TickOutput, World};
use crate::airfield_radio::AirfieldRadio;
use crate::checkpoint::Section;
use crate::comms::{Call, Hearer, Kind, Phrase};
use crate::crew_voice::CrewVoice;
use tore_sim::checkpoint::{Models, from_bytes, to_bytes};

const RADIO: [Section; 3] = [Section::Comms, Section::WingStatus, Section::Radio];

/// Steps `world` from its `from`th step up to (not including) step `to`.
fn advance(world: &mut World, scenario: &Scenario, from: u64, to: u64) {
    let mut out = TickOutput::default();
    for step in from..to {
        let (commands, inputs) = (scenario.drive)(world, step);
        world
            .step_with(&commands, &inputs, &mut out, |_, _| Ok(()))
            .unwrap();
    }
}

/// A tick's radio cues without their origins (why-records), and its other
/// outputs as text.
fn heard(out: &TickOutput) -> String {
    let cues: Vec<String> = out
        .cues
        .iter()
        .map(|cue| match cue {
            Cue::Radio { seat, call } => format!(
                "{seat:?} {:?} {:?} {:?} {:?} {:?}",
                call.line(),
                call.stems,
                call.kind,
                call.route,
                call.delay
            ),
            other => format!("{other:?}"),
        })
        .collect();
    format!("{cues:?} {:?} {:?}", out.orders, out.events)
}

/// The radio objects of the tick mission's player, at ticks through the
/// tower conversation, round trip: the tower conversation and the crew voice.
#[test]
fn the_tick_missions_tower_conversation_and_crew_voice_round_trip() {
    let scenario = checkpoint_scenarios::single_player();
    let models = Models::default();
    let mut world = (scenario.build)();
    let mut done = 0;
    let mut notices = 0;
    for tick in [90, 300, 600, 900, 1200] {
        advance(&mut world, &scenario, done, tick);
        done = tick;
        for cockpit in &world.cockpits {
            let radio = &cockpit.airfield_radio;
            let coded = to_bytes(radio, &models).unwrap();
            let copy: AirfieldRadio = from_bytes(&coded, &models).unwrap();
            assert_eq!(to_bytes(&copy, &models).unwrap(), coded, "tower at {tick}");
            notices += coded.body.len();

            let voice = &cockpit.crew_voice;
            let coded = to_bytes(voice, &models).unwrap();
            let copy: CrewVoice = from_bytes(&coded, &models).unwrap();
            assert_eq!(to_bytes(&copy, &models).unwrap(), coded, "crew at {tick}");
        }
        assert_eq!(world.cockpits.len(), 1);
    }
    assert!(notices > 0);
}

/// Two worlds flown alike to the scenario's tick, then calls and cooldowns
/// put into the first only, the radio sections restored from the first into
/// the second, and both flown on: the second delivers the calls the first
/// holds, with the same cues and the same coded radio, tick by tick.
fn twin_with_calls_pending(scenario: &Scenario) {
    let mut a = (scenario.build)();
    let mut b = (scenario.build)();
    advance(&mut a, scenario, 0, scenario.at);
    advance(&mut b, scenario, 0, scenario.at);

    let now = a.tick() as f64 / 120.;
    let seats: Vec<_> = a.comms.seats().collect();
    assert!(!seats.is_empty());
    let everyone: Vec<Hearer> = seats.iter().copied().map(Hearer::seat).collect();
    let line = |label: &str, text: &str, kind| {
        Call::new(label, Phrase::default().raw(text, Some("^CONTACT")), kind)
    };
    a.comms.send(
        now,
        line("Red two", "Radar contact", Kind::Chatter).after(0.5),
        &everyone,
    );
    a.comms.send(
        now,
        line("Tower", "Cleared to land", Kind::Important).after(2.),
        &everyone,
    );
    a.comms.send(
        now,
        line("Red three", "Fox two", Kind::Important).after(5.),
        &everyone[..1],
    );
    assert!(a.comms.cooldown("radio-gun", now, 4.));
    assert!(
        a.comms
            .seat_cooldown(seats[0], "crew-radar-warning", now, 6.)
    );
    assert_ne!(
        a.checkpoint_sections(&RADIO).unwrap(),
        b.checkpoint_sections(&RADIO).unwrap(),
        "the original holds calls the twin does not"
    );

    let bytes = a.checkpoint_sections(&RADIO).unwrap();
    assert_eq!(b.restore_sections(&bytes).unwrap(), RADIO);
    assert!(
        b.checkpoint_sections(&RADIO).unwrap() == bytes,
        "{}: a restored radio codes differently",
        scenario.name
    );

    let (mut out_a, mut out_b) = (TickOutput::default(), TickOutput::default());
    let mut injected = 0;
    for step in scenario.at..scenario.at + 900 {
        for (world, out) in [(&mut a, &mut out_a), (&mut b, &mut out_b)] {
            let (commands, inputs) = (scenario.drive)(world, step);
            world
                .step_with(&commands, &inputs, out, |_, _| Ok(()))
                .unwrap();
        }
        assert_eq!(
            heard(&out_a),
            heard(&out_b),
            "{}: the outputs differ at step {step}",
            scenario.name
        );
        injected += out_a
            .cues
            .iter()
            .filter(|cue| {
                matches!(cue, Cue::Radio { call, .. }
                    if call.text == "Radar contact" || call.text == "Cleared to land"
                        || call.text == "Fox two")
            })
            .count();
        if (step + 1 - scenario.at).is_multiple_of(30) {
            assert!(
                a.checkpoint_sections(&RADIO).unwrap() == b.checkpoint_sections(&RADIO).unwrap(),
                "{}: the radio differs after step {step}",
                scenario.name
            );
        }
    }
    assert!(
        injected >= 3,
        "{}: the pending calls were delivered ({injected})",
        scenario.name
    );
}

#[test]
fn the_tick_mission_restores_pending_calls_into_a_twin() {
    twin_with_calls_pending(&checkpoint_scenarios::single_player());
}

#[test]
fn the_crowd_fight_restores_pending_calls_into_a_twin() {
    twin_with_calls_pending(&checkpoint_scenarios::crowd_fight());
}
