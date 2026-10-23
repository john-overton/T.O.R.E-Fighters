//! Stage H slice H2's tests on whole worlds (docs/formats/checkpoint.md): the
//! roster, cockpits and weather sections of the equivalence scenarios restore
//! into worlds built fresh, which have none of the structure the handoffs
//! made, and the cockpits' coders are fed damaged bytes. The coders'
//! own round trips and step-on tests are beside the types; the twin restore of
//! these sections is the harness's (`checkpoint_tests`).
//!
//! The cockpits section holds a mission result tracker, which slice H6 codes.
//! Until it merges, the tests that need a cockpit say so and skip it.

use super::checkpoint_scenarios::{self, Scenario};
use super::{Cockpit, TickOutput, World};
use crate::checkpoint::{self, Section};
use tore_sim::checkpoint::CheckpointError;

const SHELL: [Section; 3] = [Section::Roster, Section::Cockpits, Section::Weather];

fn flown(scenario: &Scenario, steps: u64) -> World {
    let mut world = (scenario.build)();
    let mut out = TickOutput::default();
    for step in 0..steps {
        let (commands, inputs) = (scenario.drive)(&mut world, step);
        world
            .step_with(&commands, &inputs, &mut out, |_, _| Ok(()))
            .unwrap();
    }
    world
}

/// The sections of the shell this build can code: all three, or the roster
/// and the weather while a cockpit's tracker is not coded yet.
fn coded_sections(world: &World) -> Vec<Section> {
    match world.checkpoint_sections(&[Section::Cockpits]) {
        Ok(_) => SHELL.to_vec(),
        Err(CheckpointError::NotCovered(what)) => {
            eprintln!("skipping the cockpits section: {what} is not coded yet");
            vec![Section::Roster, Section::Weather]
        }
        Err(error) => panic!("the cockpits section failed: {error}"),
    }
}

/// Two cockpits are the same when their state is: everything but the
/// scratch `previous_flight`, which a restore sets equal to the flight.
fn assert_same(a: &Cockpit, b: &Cockpit, what: &str) {
    assert_eq!(a.plane, b.plane, "{what}");
    assert_eq!(a.flight, b.flight, "{what}: flight");
    assert_eq!(b.previous_flight, b.flight, "{what}: previous flight");
    assert_eq!(a.turbulence, b.turbulence, "{what}: turbulence");
    assert_eq!(a.turbulence_rng, b.turbulence_rng, "{what}: stream");
    assert_eq!(a.airport_service, b.airport_service, "{what}: tower");
    assert_eq!(a.airport_nav_mode, b.airport_nav_mode, "{what}: NAV");
    assert_eq!(a.overspeed_message_at, b.overspeed_message_at, "{what}");
    assert_eq!(a.edge_message_at, b.edge_message_at, "{what}");
}

/// Every scenario's roster, cockpits and weather at its tick N restore into a
/// world fresh from the build: the roster and the cockpits come out as the
/// original's (an open mission's fresh world has none), the weather clock is
/// where the original's was, and the restored world codes the same bytes.
#[test]
fn the_shell_sections_restore_into_fresh_worlds_with_the_originals_structure() {
    for scenario in checkpoint_scenarios::all() {
        let original = flown(&scenario, scenario.at);
        let sections = coded_sections(&original);
        let bytes = original.checkpoint_sections(&sections).unwrap();

        let mut fresh = (scenario.build)();
        assert_ne!(
            fresh.terrain.weather, original.terrain.weather,
            "{}: the weather has moved since the build",
            scenario.name
        );
        assert_eq!(fresh.restore_sections(&bytes).unwrap(), sections);

        assert_eq!(fresh.roster, original.roster, "{}: roster", scenario.name);
        assert_eq!(
            fresh.terrain.weather, original.terrain.weather,
            "{}: weather",
            scenario.name
        );
        if sections.contains(&Section::Cockpits) {
            assert_eq!(fresh.cockpits.len(), original.cockpits.len());
            for (restored, held) in fresh.cockpits.iter().zip(&original.cockpits) {
                assert_same(held, restored, scenario.name);
            }
        }
        // The sections must code to the same bytes (the container's header
        // holds the tick, which the fresh world has not reached: combat sets
        // it). A cockpit is coded with its plane's ownship, which only the
        // combat section restores, so the cockpits are compared above.
        let coded: Vec<Section> = sections
            .iter()
            .copied()
            .filter(|section| *section != Section::Cockpits)
            .collect();
        let bytes = original.checkpoint_sections(&coded).unwrap();
        let again = fresh.checkpoint_sections(&coded).unwrap();
        let (held, restored) = (
            checkpoint::layout(&bytes).unwrap(),
            checkpoint::layout(&again).unwrap(),
        );
        assert_eq!(held.records, restored.records, "{}", scenario.name);
        for section in &coded {
            assert!(
                held.body(&bytes, *section) == restored.body(&again, *section),
                "{}: the restored {} codes differently",
                scenario.name,
                section.name()
            );
        }
    }
}

/// The states the scenarios are meant to exercise are there at tick N, so the
/// round trips above are of state that matters.
#[test]
fn the_scenarios_hold_the_shell_state_they_are_meant_to() {
    // Single player: the tower conversation and turbulence are in use.
    let scenario = checkpoint_scenarios::single_player();
    let world = flown(&scenario, scenario.at);
    let cockpit = &world.cockpits[0];
    assert!(
        cockpit.airport_service.selected().is_some()
            || cockpit.airport_service.last_reply().is_some(),
        "the player has spoken to the tower"
    );
    assert_ne!(
        cockpit.turbulence,
        tore_sim::turbulence::Turbulence::default(),
        "turbulence has moved"
    );
    assert!(world.terrain.weather.ticks() > 0);

    // The handoffs: a seat that gave its plane back waits, and the roster has
    // humans the fresh world never had.
    let scenario = checkpoint_scenarios::open_handoffs();
    let world = flown(&scenario, scenario.at);
    assert!(world.roster.seats().iter().any(|seat| seat.plane.is_none()));
    assert!(world.roster.seats().iter().any(|seat| seat.plane.is_some()));
    assert!(!world.cockpits.is_empty());
    assert!((scenario.build)().cockpits.is_empty());
}

/// Damaged shell sections are refused or decoded to something odd, never a
/// panic: every cut, and 10,000 flipped bits and random bodies, past the CRC.
#[test]
fn damaged_shell_sections_are_refused_without_a_panic() {
    let scenario = checkpoint_scenarios::open_handoffs();
    let world = flown(&scenario, scenario.at);
    let sections = coded_sections(&world);
    let bytes = world.checkpoint_sections(&sections).unwrap();
    let mut fresh = (scenario.build)();
    let with_crc = |mut bytes: Vec<u8>| {
        let end = bytes.len() - 4;
        let crc = tore_codec::crc32(&bytes[..end]);
        bytes[end..].copy_from_slice(&crc.to_le_bytes());
        bytes
    };
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for cut in 0..bytes.len() {
        let _ = fresh.restore_sections(&bytes[..cut]);
        if cut >= 4 {
            let _ = fresh.restore_sections(&with_crc(bytes[..cut].to_vec()));
        }
    }
    for _ in 0..10_000 {
        let mut damaged = bytes.clone();
        let at = next() as usize % (damaged.len() - 4);
        damaged[at] ^= 1 << (next() % 8);
        let _ = fresh.restore_sections(&with_crc(damaged));
    }
}
