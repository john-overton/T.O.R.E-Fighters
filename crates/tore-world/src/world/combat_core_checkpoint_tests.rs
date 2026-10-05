//! Stage H slice H3a's round trips on the equivalence fixtures
//! (docs/formats/checkpoint.md): combat's core state (`live::State`) with
//! missiles guiding and rounds in flight, at ticks of the crowd fight, in the
//! missile duel and in the damaged-aircraft mission. The coders themselves
//! are in `tore-sim` beside the types.
//!
//! A whole `live::State` also holds the effects slice's types (H3b). While
//! those report themselves not covered, the whole-state checks are skipped
//! (and say so); every part H3a owns is checked on its own either way.

use super::World;
use super::checkpoint_scenarios::{self, Scenario};
use tore_sim::checkpoint::{CheckpointError, Models, from_bytes, round_trip, to_bytes};
use tore_sim::combat::ledger::Ledger;
use tore_sim::combat::live::{Ownship, Projectile, State, Target};

/// The effects slice's types; see the module comment.
const EFFECTS_SLICE: [&str; 5] = [
    "combat::smoke::Smoke",
    "combat::countermeasures::Devices",
    "combat::debris::Piece",
    "combat::blast::Mark",
    "combat::blast::Rolls",
];

fn text<T: std::fmt::Debug>(value: &T) -> String {
    format!("{value:?}")
}

/// The pieces of `state` that slice H3a owns, each round-tripped. Returns the
/// counts of what they carried: guided missiles, rounds, rewinds, targets,
/// ownships and kills.
fn parts_round_trip(state: &State, label: &str) -> [usize; 6] {
    let models = Models::default();
    for own in state.ownships() {
        let copy: Ownship = round_trip(own, &models).unwrap();
        assert_eq!(text(&copy), text(own), "{label}: ownship {}", own.aircraft);
    }
    for row in &state.targets {
        let copy: Target = round_trip(row, &models).unwrap();
        assert_eq!(&copy, row, "{label}: target {}", row.id);
    }
    let mut guided = 0;
    for round in &state.projectiles {
        let copy: Projectile = round_trip(round, &models).unwrap();
        assert_eq!(&copy, round, "{label}: projectile {}", round.id);
        guided += usize::from(round.guidance.is_some());
    }
    let ledger: Ledger = round_trip(&state.ledger, &models).unwrap();
    assert_eq!(ledger.kills(), state.ledger.kills());
    assert_eq!(
        ledger.tallies().collect::<Vec<_>>(),
        state.ledger.tallies().collect::<Vec<_>>()
    );
    let history = round_trip(state.hit_volumes(), &models).unwrap();
    // The coding carries the newest 61 ticks, the ones a rewind can read.
    assert_eq!(
        history.span().map(|s| s.1),
        state.hit_volumes().span().map(|s| s.1)
    );
    for id in 0..40 {
        for rewind in 0..=60 {
            assert_eq!(
                history.volume(id, rewind),
                state.hit_volumes().volume(id, rewind)
            );
        }
    }
    [
        guided,
        state.projectiles.len(),
        state
            .projectiles
            .iter()
            .filter(|p| state.rewind_of(p.id) > 0)
            .count(),
        state.targets.len(),
        state.ownships().len(),
        state.ledger.kills().len(),
    ]
}

/// The state as a checkpoint carries it: the why-records drained.
fn drained(world: &mut World) -> String {
    let state = &mut world.combat.state;
    state.take_device_notes();
    state.take_decoy_rolls();
    state.ledger.take_outcomes();
    text(&*state)
}

/// Codes the world's whole combat state, or says the effects slice is not
/// merged yet.
fn whole(world: &World, label: &str) -> Option<State> {
    let models = Models::default();
    match to_bytes(&world.combat.state, &models) {
        Ok(coded) => {
            let copy: State = from_bytes(&coded, &models).unwrap();
            assert_eq!(
                to_bytes(&copy, &models).unwrap(),
                coded,
                "{label}: restored state codes differently"
            );
            println!(
                "{label}: combat state {} bytes and {} shared records ({} bytes)",
                coded.body.len(),
                coded.records.len(),
                coded.records.iter().map(Vec::len).sum::<usize>()
            );
            Some(copy)
        }
        Err(CheckpointError::NotCovered(what)) if EFFECTS_SLICE.contains(&what) => {
            println!("{label}: whole state skipped, {what} is not coded yet");
            None
        }
        Err(error) => panic!("{label}: the combat state did not code: {error}"),
    }
}

#[test]
fn the_crowds_combat_state_round_trips_at_300_600_and_900() {
    let scenario = checkpoint_scenarios::crowd_fight();
    let mut world = (scenario.build)();
    let mut done = 0;
    let (mut rounds, mut rewound, mut kills) = (0, 0, 0);
    for tick in [300, 600, 900] {
        advance(&mut world, &scenario, done, tick);
        done = tick;
        let [guided, flying, rewinds, targets, rows, killed] =
            parts_round_trip(&world.combat.state, &format!("crowd {tick}"));
        println!(
            "crowd {tick}: {guided} guided, {flying} rounds, {rewinds} rewinds, {targets} targets, \
             {rows} ownships, {killed} kills, history {} bytes",
            to_bytes(world.combat.state.hit_volumes(), &Models::default())
                .unwrap()
                .body
                .len()
        );
        rounds += flying;
        rewound += rewinds;
        kills += killed;
        whole(&world, &format!("crowd {tick}"));
    }
    assert!(rounds > 0, "no round was ever in flight");
    // The fixture really held what the coders carry. (Rewinds are rounds a
    // human fired with a view; the fixture's humans have none, so none is
    // required.)
    println!("crowd: {rounds} rounds, {rewound} rewinds, {kills} kills");
}

#[test]
fn the_missile_duel_and_the_damaged_mission_round_trip() {
    for scenario in [
        checkpoint_scenarios::missile_duel(),
        checkpoint_scenarios::damaged_aircraft(),
        checkpoint_scenarios::crowd_handoffs(),
    ] {
        let world = scenario.flown(scenario.at);
        let [guided, flying, ..] = parts_round_trip(&world.combat.state, scenario.name);
        if scenario.name == "missile duel" {
            assert!(guided > 0, "no guided missile at the checkpoint");
        }
        println!(
            "{}: {guided} guided, {flying} rounds, history {} bytes",
            scenario.name,
            to_bytes(world.combat.state.hit_volumes(), &Models::default())
                .unwrap()
                .body
                .len()
        );
        whole(&world, scenario.name);
    }
}

#[test]
fn a_restored_combat_state_steps_on_identically() {
    // The whole-state check: restore the combat state into a twin world that
    // is otherwise equal, then step both and compare. Skipped while the
    // effects slice's types are not coded.
    for scenario in [
        checkpoint_scenarios::missile_duel(),
        checkpoint_scenarios::crowd_fight(),
    ] {
        let mut original = scenario.flown(scenario.at);
        let mut twin = scenario.flown(scenario.at);
        let Some(copy) = whole(&original, scenario.name) else {
            continue;
        };
        twin.combat.state = copy;
        let bytes = |world: &mut World| {
            drained(world);
            to_bytes(&world.combat.state, &Models::default()).unwrap()
        };
        assert_eq!(
            bytes(&mut original),
            bytes(&mut twin),
            "{}: restored",
            scenario.name
        );
        let mut out = super::TickOutput::default();
        for step in scenario.at..scenario.at + 600 {
            for world in [&mut original, &mut twin] {
                let (commands, inputs) = (scenario.drive)(world, step);
                world
                    .step_with(&commands, &inputs, &mut out, |_, _| Ok(()))
                    .unwrap();
            }
            if step % 100 == 0 {
                assert_eq!(
                    bytes(&mut original),
                    bytes(&mut twin),
                    "{}: step {step} after the restore",
                    scenario.name
                );
                // The restored rewind history is shorter for its first second
                // (it carries the ticks a rewind can read); after that the two
                // are equal in every field.
                if step >= scenario.at + 70 {
                    assert_eq!(
                        drained(&mut original),
                        drained(&mut twin),
                        "{}: step {step} after the restore",
                        scenario.name
                    );
                }
            }
        }
    }
}

/// Steps `world` from its `from`th step up to (not including) step `to`.
fn advance(world: &mut World, scenario: &Scenario, from: u64, to: u64) {
    let mut out = super::TickOutput::default();
    for step in from..to {
        let (commands, inputs) = (scenario.drive)(world, step);
        world
            .step_with(&commands, &inputs, &mut out, |_, _| Ok(()))
            .unwrap();
    }
}
