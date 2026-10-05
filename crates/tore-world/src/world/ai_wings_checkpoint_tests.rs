//! Stage H slice H6's tests on whole worlds (docs/formats/checkpoint.md): the
//! AI bridge's own state in the equivalence scenarios, as the world holds it
//! at several ticks, restored into the bridge of a world fresh from the build.
//!
//! The AI mission inside the AI wings section is slice H4's coder. Until it
//! merges, these tests code the bridge apart from the mission
//! (`AiWings::save_bridge`); the harness's twin restore covers the whole
//! section as soon as the mission is coded, and says so in its coverage line.

use super::checkpoint_scenarios::{self, Scenario};
use super::{TickOutput, World};
use tore_sim::checkpoint::{Loader, Models, Saver};

/// The scenario's world flown to `to` steps, from a world already at `from`.
fn advance(world: &mut World, scenario: &Scenario, from: u64, to: u64) {
    let mut out = TickOutput::default();
    for step in from..to {
        let (commands, inputs) = (scenario.drive)(world, step);
        world
            .step_with(&commands, &inputs, &mut out, |_, _| Ok(()))
            .unwrap();
    }
}

/// The bridge's coding: its section body and its shared records.
fn coded(wings: &crate::ai_wings::AiWings) -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut s = Saver::with_models(Models::default());
    wings.save_bridge(&mut s).unwrap();
    let body = s.finish_section();
    (body, s.into_records())
}

fn restore(wings: &mut crate::ai_wings::AiWings, coded: &(Vec<u8>, Vec<Vec<u8>>)) {
    let models = Models::default();
    let mut l = Loader::new(&coded.0, &coded.1, &models);
    wings.restore_bridge(&mut l).unwrap();
    l.finish().unwrap();
}

/// The bridge at each of several ticks of every scenario restores into a
/// fresh world's bridge, which then holds the same state and codes the same
/// bytes.
#[test]
fn the_bridge_restores_into_a_fresh_world_at_several_ticks() {
    let mut totals = std::collections::BTreeMap::<&str, usize>::new();
    for scenario in checkpoint_scenarios::all() {
        let mut world = (scenario.build)();
        let mut done = 0;
        for tick in [1, 120, 300, scenario.at, scenario.at + 300] {
            advance(&mut world, &scenario, done, tick);
            done = tick;
            let original = world.ai_wings.as_ref().expect("the scenario has AI wings");
            let bytes = coded(original);

            let mut fresh = (scenario.build)();
            let wings = fresh.ai_wings.as_mut().unwrap();
            restore(wings, &bytes);
            assert_eq!(
                wings.bridge_digest(),
                original.bridge_digest(),
                "{} at {tick}",
                scenario.name
            );
            assert!(
                coded(wings) == bytes,
                "{} at {tick}: the restored bridge codes differently",
                scenario.name
            );
            for (what, count) in original.bridge_census() {
                *totals.entry(what).or_default() += count;
            }
            println!(
                "{} at {tick}: bridge {} bytes, {} shared records",
                scenario.name,
                bytes.0.len(),
                bytes.1.len()
            );
        }
    }
    println!("what the fixtures held: {totals:?}");
    // The fixtures exercised the state the coder carries. What the scenarios
    // never hold (the AI firing, a damaged station, a cheat's skill, radio
    // events between ticks) is tested on constructed bridges beside the
    // coder.
    for what in [
        "slots",
        "humans",
        "weapons",
        "configurations",
        "last hit points",
        "activities",
        "handed over",
    ] {
        assert!(totals[what] > 0, "no scenario held any {what}");
    }
}

/// A restore replaces the structure with the checkpoint's: the open mission's
/// handoffs add and remove humans and slots, so a world fresh from the build
/// gets the humans the checkpoint holds.
#[test]
fn the_restored_bridge_has_the_checkpoints_humans_not_the_builds() {
    let scenario = checkpoint_scenarios::open_handoffs();
    let mut world = (scenario.build)();
    advance(&mut world, &scenario, 0, scenario.at);
    let original = world.ai_wings.as_ref().unwrap();
    let bytes = coded(original);
    let mut fresh = (scenario.build)();
    let wings = fresh.ai_wings.as_mut().unwrap();
    assert_ne!(
        wings.bridge_digest(),
        original.bridge_digest(),
        "the bridge has moved on since the build"
    );
    restore(wings, &bytes);
    assert_eq!(wings.bridge_digest(), original.bridge_digest());
}

/// Damaged bridge bytes are refused, or decode to something, but never
/// panic, whatever bit is flipped or where the bytes end.
#[test]
fn damaged_bridge_bytes_never_panic() {
    let scenario = checkpoint_scenarios::crowd_fight();
    let mut world = (scenario.build)();
    advance(&mut world, &scenario, 0, 400);
    let bytes = coded(world.ai_wings.as_ref().unwrap());
    let models = Models::default();
    let mut fresh = (scenario.build)();
    let wings = fresh.ai_wings.as_mut().unwrap();
    for cut in 0..bytes.0.len() {
        let mut l = Loader::new(&bytes.0[..cut], &bytes.1, &models);
        let _ = wings.restore_bridge(&mut l);
    }
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..3_000 {
        let mut damaged = bytes.0.clone();
        let at = next() as usize % damaged.len();
        damaged[at] ^= 1 << (next() % 8);
        let mut l = Loader::new(&damaged, &bytes.1, &models);
        let _ = wings.restore_bridge(&mut l);
    }
    // A record index past the table is refused, not indexed.
    let mut l = Loader::new(&bytes.0, &[], &models);
    let _ = wings.restore_bridge(&mut l);
}
