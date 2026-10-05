//! The checkpoint harness (docs/formats/checkpoint.md, "The equivalence
//! scenarios"): the container's refusals, the twin restore of the sections
//! coded so far, and the whole-world equivalence, which stays ignored until
//! every section is coded (stage H slice H9 un-ignores it).

use super::checkpoint_scenarios::{self, Scenario};
use super::{Cue, TickOutput, World};
use crate::checkpoint::{self, Section};
use tore_sim::checkpoint::CheckpointError;

/// Takes step `step` of `scenario`.
fn step(world: &mut World, scenario: &Scenario, step: u64, out: &mut TickOutput) {
    let (commands, inputs) = (scenario.drive)(world, step);
    world
        .step_with(&commands, &inputs, out, |_, _| Ok(()))
        .unwrap_or_else(|error| panic!("{}: step {step}: {error}", scenario.name));
}

/// The scenario built and stepped `steps` times.
fn flown(scenario: &Scenario, steps: u64) -> World {
    let mut world = (scenario.build)();
    let mut out = TickOutput::default();
    for n in 0..steps {
        step(&mut world, scenario, n, &mut out);
    }
    world
}

/// Every field of a tick's output but the why-records (the AI journal and a
/// radio call's origin), as exact text: floats print in their shortest exact
/// form. Destructured, so a new output field fails to compile here.
fn digest(out: &TickOutput) -> String {
    let TickOutput {
        cues,
        commanded,
        orders,
        events,
        releases,
        outcomes,
        // The AI's why-records: not restored.
        journal: _,
        emissions,
        fault,
        terms,
    } = out;
    let cues: Vec<String> = cues
        .iter()
        .map(|cue| match cue {
            Cue::Radio { seat, call } => {
                let crate::comms::Call {
                    label,
                    text,
                    stems,
                    kind,
                    route,
                    delay,
                    // Why the call was made: a why-record.
                    origin: _,
                } = call;
                format!("Radio {seat:?} {label:?} {text:?} {stems:?} {kind:?} {route:?} {delay:?}")
            }
            other => format!("{other:?}"),
        })
        .collect();
    format!(
        "{cues:?}\n{commanded}\n{orders:?}\n{events:?}\n{releases:?}\n{outcomes:?}\n\
         {emissions:?}\n{fault:?}\n{terms:?}"
    )
}

/// The sections this build codes, found by checkpointing each alone, and
/// the reason each other one gives. Only `NotCovered` is an acceptable
/// reason; anything else is a coder's failure.
fn coverage(world: &World) -> (Vec<Section>, Vec<&'static str>) {
    let mut covered = Vec::new();
    let mut missing = Vec::new();
    for section in Section::ALL {
        match world.checkpoint_sections(&[section]) {
            Ok(_) => covered.push(section),
            Err(CheckpointError::NotCovered(what)) => missing.push(what),
            Err(error) => panic!("the {} section failed: {error}", section.name()),
        }
    }
    (covered, missing)
}

/// Steps `a` and `b` alike for `then` steps from `from`, requiring the same
/// tick output every tick and the same coding of `sections` every 30 steps
/// and at the end.
fn lockstep(a: &mut World, b: &mut World, scenario: &Scenario, sections: &[Section]) {
    let (mut out_a, mut out_b) = (TickOutput::default(), TickOutput::default());
    for n in scenario.at..scenario.at + scenario.then {
        step(a, scenario, n, &mut out_a);
        step(b, scenario, n, &mut out_b);
        assert_eq!(
            digest(&out_a),
            digest(&out_b),
            "{}: the outputs differ at step {n}",
            scenario.name
        );
        if (n + 1 - scenario.at).is_multiple_of(30) || n + 1 == scenario.at + scenario.then {
            assert!(
                a.checkpoint_sections(sections).unwrap()
                    == b.checkpoint_sections(sections).unwrap(),
                "{}: the coded state differs after step {n}",
                scenario.name
            );
        }
    }
}

fn sizes(bytes: &[u8]) -> String {
    let layout = checkpoint::layout(bytes).unwrap();
    let mut text = format!("{} bytes: records {}", bytes.len(), layout.records_bytes);
    for (section, range) in &layout.sections {
        text += &format!(", {} {}", section.name(), range.len());
    }
    text
}

/// The twin restore: two copies flown alike to tick N, the covered sections
/// of one restored into the other, both flown on. The uncovered sections are
/// equal already, so any difference comes from the covered sections' coding.
#[test]
fn the_covered_sections_restore_into_a_twin_and_fly_on_identically() {
    for scenario in checkpoint_scenarios::all() {
        let mut a = flown(&scenario, scenario.at);
        let mut b = flown(&scenario, scenario.at);
        let (covered, missing) = coverage(&a);
        eprintln!(
            "{}: covered {:?}; not yet {:?}",
            scenario.name,
            covered.iter().map(|s| s.name()).collect::<Vec<_>>(),
            missing
        );
        let bytes = a.checkpoint_sections(&covered).unwrap();
        eprintln!("{}: {}", scenario.name, sizes(&bytes));
        assert_eq!(b.restore_sections(&bytes).unwrap(), covered);
        assert!(
            b.checkpoint_sections(&covered).unwrap() == bytes,
            "{}: a restored world codes differently",
            scenario.name
        );
        lockstep(&mut a, &mut b, &scenario, &covered);
    }
}

/// The whole-world equivalence: tick N checkpointed, restored into a world
/// fresh from the same build, and both flown on bit for bit.
#[test]
#[ignore = "stage H: needs every section coded; slice H9 un-ignores it"]
fn a_whole_world_restores_into_a_fresh_one_and_flies_on_identically() {
    for scenario in checkpoint_scenarios::all() {
        let mut a = flown(&scenario, scenario.at);
        let bytes = a.checkpoint().unwrap();
        eprintln!("{}: {}", scenario.name, sizes(&bytes));
        let mut b = (scenario.build)();
        b.restore(&bytes).unwrap();
        assert_eq!(b.tick(), a.tick());
        assert!(
            b.checkpoint().unwrap() == bytes,
            "{}: a restored world codes differently",
            scenario.name
        );
        lockstep(&mut a, &mut b, &scenario, &Section::ALL);
    }
}

/// Until every section is coded, a whole checkpoint names the first
/// section's missing coder, never another error.
#[test]
fn a_checkpoint_names_what_is_not_coded_yet() {
    let world = flown(&checkpoint_scenarios::single_player(), 120);
    let (covered, missing) = coverage(&world);
    match world.checkpoint() {
        Ok(bytes) => {
            assert!(missing.is_empty());
            assert_eq!(covered.len(), Section::ALL.len());
            assert!(checkpoint::layout(&bytes).is_ok());
        }
        Err(CheckpointError::NotCovered(what)) => assert!(missing.contains(&what), "{what}"),
        Err(error) => panic!("{error}"),
    }
}

/// Rewrites a checkpoint's CRC after an edit, so the edit reaches the checks
/// behind it.
fn with_crc(mut bytes: Vec<u8>) -> Vec<u8> {
    let end = bytes.len() - 4;
    let crc = tore_codec::crc32(&bytes[..end]);
    bytes[end..].copy_from_slice(&crc.to_le_bytes());
    bytes
}

#[test]
fn the_container_refuses_damage_other_missions_versions_and_switches() {
    let scenario = checkpoint_scenarios::single_player();
    let world = flown(&scenario, 240);
    let bytes = world.checkpoint_sections(&[Section::Radio]).unwrap();
    let layout = checkpoint::layout(&bytes).unwrap();
    assert_eq!(layout.version, checkpoint::VERSION);
    assert_eq!(layout.tick, world.tick());
    assert_eq!(layout.identity, world.mission_identity());
    assert_eq!(layout.sections.len(), 1);

    let mut fresh = (scenario.build)();
    let restore = |fresh: &mut World, bytes: &[u8]| fresh.restore_sections(bytes);
    assert!(restore(&mut fresh, &bytes).is_ok());
    // A whole restore needs every section.
    assert!(fresh.restore(&bytes).is_err());
    // Damaged: the CRC catches it.
    let mut damaged = bytes.clone();
    damaged[12] ^= 0x10;
    assert!(restore(&mut fresh, &damaged).is_err());
    // Not a checkpoint, another container version, an unknown switch.
    for at in [0, 8, 10] {
        let mut edited = bytes.clone();
        edited[at] ^= 0x01;
        assert!(restore(&mut fresh, &with_crc(edited)).is_err(), "byte {at}");
    }
    // The retail stall speed switch of another process.
    let mut edited = bytes.clone();
    edited[10] |= 1;
    assert!(restore(&mut fresh, &with_crc(edited)).is_err());
    // Bytes after the last section.
    let mut longer = bytes[..bytes.len() - 4].to_vec();
    longer.push(0);
    longer.extend_from_slice(&[0; 4]);
    assert!(restore(&mut fresh, &with_crc(longer)).is_err());
    // Too short to be anything.
    assert!(restore(&mut fresh, &bytes[..6]).is_err());
    // Another mission: the crowd fixture's world.
    let mut other = (checkpoint_scenarios::crowd_fight().build)();
    assert_ne!(other.mission_identity(), world.mission_identity());
    assert!(other.restore_sections(&bytes).is_err());
}

#[test]
fn damaged_checkpoints_are_refused_without_a_panic() {
    let scenario = checkpoint_scenarios::single_player();
    let world = flown(&scenario, 240);
    let bytes = world.checkpoint_sections(&[Section::Radio]).unwrap();
    let mut fresh = (scenario.build)();
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for cut in 0..bytes.len() {
        let _ = fresh.restore_sections(&bytes[..cut]);
    }
    for _ in 0..10_000 {
        // Past the CRC, so the framing and the section coders see the damage.
        let mut damaged = bytes.clone();
        let at = next() as usize % (damaged.len() - 4);
        damaged[at] ^= 1 << (next() % 8);
        let _ = fresh.restore_sections(&with_crc(damaged));
        let random: Vec<u8> = (0..(next() % 200) + 4).map(|_| next() as u8).collect();
        let _ = fresh.restore_sections(&with_crc(random));
    }
}
