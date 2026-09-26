//! The recorder's reasons and trees against a synthetic AI mission, and its
//! change detection against synthetic effect lists.
use super::super::{Message, Recorder, Tick};
use super::*;
use crate::ai_wings::tests::{aircraft, combat_fixture, payload, spawned};
use crate::combat::render_hash_tests as fixture;
use crate::terrain;
use tore_sim::flight::trace::Effect;

#[test]
fn an_effect_turns_on_and_off_once_through_flicker_and_a_moment_has_no_off() {
    let mut body = Body::default();
    let mut events = Vec::new();
    let wing = [Effect::WingDamaged];
    assert!(effect_changes(&mut body, 3, 0, &wing, &mut events));
    for tick in 1..10 {
        assert!(!effect_changes(&mut body, 3, tick, &wing, &mut events));
    }
    // A gap shorter than the hold is flicker: the same episode goes on.
    assert!(!effect_changes(&mut body, 3, 10, &[], &mut events));
    assert!(!effect_changes(&mut body, 3, 11, &wing, &mut events));
    for tick in 12..11 + EFFECT_HOLD_TICKS {
        assert!(!effect_changes(&mut body, 3, tick, &[], &mut events));
    }
    // Gone for the whole hold: it stopped.
    assert!(effect_changes(
        &mut body,
        3,
        11 + EFFECT_HOLD_TICKS,
        &[],
        &mut events
    ));
    // A moment: "on", and no "off" when it passes.
    assert!(effect_changes(
        &mut body,
        3,
        100,
        &[Effect::SurfaceDropped],
        &mut events
    ));
    for tick in 101..200 {
        effect_changes(&mut body, 3, tick, &[], &mut events);
    }
    let seen: Vec<(Option<&str>, Option<bool>, Option<bool>)> = events
        .iter()
        .map(|e| {
            (
                e.string(field::EFFECT),
                e.flag(field::ON),
                e.flag(field::MOMENTARY),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            (Some("Wing damaged"), Some(true), None),
            (Some("Wing damaged"), Some(false), None),
            (Some("Surface dropped"), Some(true), Some(true)),
        ]
    );
    let on = &events[0];
    assert_eq!(on.subject, Some(3));
    assert_eq!(on.string(field::FACTOR), Some("x0.5"));
    assert_eq!(on.string(field::REASON), Some("the wing system failed"));
}

/// What a synthetic two-against-two mission leaves behind: every actor's
/// flight state and decision state, the weapons in flight and the targets.
type Fingerprint = (
    Vec<(
        tore_sim::flight::State,
        tore_sim::ai::controller::Controller,
    )>,
    Vec<live::Projectile>,
    Vec<[f64; 3]>,
    i32,
);

/// Flies the synthetic mission for `ticks`, recording every tick when
/// `record`, and returns what it left and what was recorded. The friendly
/// pair is the player's wing, released to attack on contact after a
/// second; the enemy pair holds formation until it sees an attack.
fn fly(ticks: u64, record: bool) -> (Fingerprint, Vec<tore_replay::Frame>) {
    let targets = spawned();
    let mut selections = payload(None);
    selections[0].wing.index = 0;
    let mut wings =
        crate::ai_wings::AiWings::build_with(&selections, &targets, 0, |_| Ok((aircraft(), None)))
            .unwrap();
    let mut combat = fixture::combat(Vec::new(), Vec::new());
    combat.state = combat_fixture(true);
    combat.state.targets = targets;
    let player = flight::State::new(&aircraft(), [0., 20_000., -5_000.]).unwrap();
    let world = terrain::tests::world();
    combat.restart_render(&player, Some(&wings));
    let (mut recorder, receiver) = Recorder::detached(1 << 20, &[]);
    for tick in 0..ticks {
        if record {
            recorder.start_tick(None, &mut combat);
        }
        if tick == 120 {
            wings
                .command_at(
                    tore_sim::ai::wing::PlayerOrder::AttackOnContact,
                    None,
                    None,
                    None,
                )
                .unwrap();
        }
        let events = combat
            .state
            .step(false, crate::combat::launcher(&player), |_, _| 0.);
        wings.step(&mut combat.state, &player, &world).unwrap();
        combat.advance_render(&player, Some(&wings));
        if record {
            let journal = wings.take_ai_journal();
            let outcomes = combat.state.ledger.take_outcomes();
            recorder.begin(Tick {
                snapshot: combat.render_snapshot(),
                combat: &combat,
                flight: &player,
                previous: &player,
                pilot: &flight::PilotInput::default(),
                wings: Some(&wings),
                world: &world,
                events: &events,
                outcomes: &outcomes,
                journal: Some(&journal),
            });
            recorder.end(None, &mut combat);
        }
    }
    drop(recorder);
    let frames = receiver
        .try_iter()
        .filter_map(|m| match m {
            Message::Frame(frame) => Some(*frame),
            _ => None,
        })
        .collect();
    let actors = wings
        .mission()
        .actors()
        .iter()
        .map(|a| (a.flight().clone(), a.controller().clone()))
        .collect();
    let fingerprint = (
        actors,
        combat.state.projectiles.clone(),
        combat.state.targets.iter().map(|t| t.position).collect(),
        combat.state.player_hp,
    );
    (fingerprint, frames)
}

#[test]
fn recording_the_reasons_changes_nothing_the_mission_does() {
    const TICKS: u64 = 7_200;
    let (quiet, _) = fly(TICKS, false);
    let (recorded, frames) = fly(TICKS, true);
    assert_eq!(
        quiet, recorded,
        "reading the records must not steer anything"
    );
    assert_eq!(
        frames.len() as u64,
        TICKS - 1,
        "every tick but the open one"
    );
    // While alive, every AI aircraft thinks 10 times a second and records
    // telemetry 5 times a second; the player's telemetry is 30 a second.
    // Each aircraft keeps its own phase, so the work spreads over ticks.
    let mut due = 0;
    for frame in &frames {
        let has = |id: u32, channel: &str| {
            frame
                .trees
                .iter()
                .any(|t| t.subject == id && t.channel == channel)
        };
        for state in frame.aircraft.iter().filter(|a| a.flags.alive) {
            let id = u64::from(state.id);
            if state.id != 0 && (frame.tick + id).is_multiple_of(12) {
                due += 1;
                assert!(has(state.id, channel::AI_THOUGHT), "tick {}", frame.tick);
            }
            let period = if state.id == 0 { 4 } else { 24 };
            if (frame.tick + id).is_multiple_of(period) {
                due += 1;
                assert!(has(state.id, channel::FLIGHT_TELEMETRY));
            }
        }
    }
    assert!(due > 1_000, "{due}");
    // Every decision change is explained, and records a thought tree on
    // the very tick it happened.
    let mut changes = 0;
    for frame in &frames {
        for event in frame.events.iter().filter(|e| {
            matches!(
                e.kind.as_str(),
                kind::AI_ACTIVITY | kind::AI_TARGET | kind::AI_WEAPON_PHASE
            )
        }) {
            changes += 1;
            let subject = event.subject.unwrap();
            assert!(
                frame
                    .trees
                    .iter()
                    .any(|t| t.subject == subject && t.channel == channel::AI_THOUGHT),
                "{} at tick {} has no thought tree",
                event.kind,
                frame.tick
            );
            if event.kind != kind::AI_WEAPON_PHASE {
                assert!(
                    event.string(field::REASON).is_some_and(|r| !r.is_empty()),
                    "{event:?}"
                );
            }
        }
    }
    assert!(changes > 0, "the mission should decide something");
    let kinds: std::collections::BTreeSet<&str> = frames
        .iter()
        .flat_map(|f| f.events.iter().map(|e| e.kind.as_str()))
        .collect();
    // The fixture's stores have no weapon records, so the bridge drops its
    // launches; messages between aircraft have their own tests.
    for wanted in [
        kind::AI_ACTIVITY,
        kind::AI_TARGET,
        kind::AI_WEAPON_PHASE,
        kind::AI_FALLBACK,
        kind::FLIGHT_EFFECT,
    ] {
        assert!(kinds.contains(wanted), "no {wanted} in {kinds:?}");
    }
    // Every tree names the AI's activity and target, and the telemetry
    // carries the height the summary and Tacview read.
    for frame in &frames {
        for tree in &frame.trees {
            match tree.channel.as_str() {
                channel::AI_THOUGHT => {
                    assert!(tree.node(tore_replay::vocab::node::ACTIVITY).is_some());
                    assert!(tree.node(tore_replay::vocab::node::TARGET).is_some());
                }
                channel::FLIGHT_TELEMETRY => {
                    assert!(tree.node(tore_replay::vocab::node::AGL).is_some());
                }
                _ => {}
            }
        }
    }
}
