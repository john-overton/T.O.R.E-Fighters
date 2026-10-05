//! Stage H slice H10's tests on whole worlds (docs/formats/checkpoint.md): the
//! data link and score sections of the equivalence scenarios. The picture is
//! restored at ticks around a publish, a link wiped from a twin is restored
//! from the original and steps on identically, the score recorder's switch
//! and recorded targets travel, and damaged bytes are refused. The coders'
//! own round trips are beside the types.

use super::checkpoint_scenarios::{self, Scenario};
use super::{TickOutput, World};
use crate::checkpoint::Section;
use crate::datalink::{DataLink, PUBLISH_TICKS};

const LINK: [Section; 2] = [Section::DataLink, Section::Score];

fn advance(world: &mut World, scenario: &Scenario, from: u64, to: u64) {
    let mut out = TickOutput::default();
    for step in from..to {
        let (commands, inputs) = (scenario.drive)(world, step);
        world
            .step_with(&commands, &inputs, &mut out, |_, _| Ok(()))
            .unwrap();
    }
}

fn same_link(a: &DataLink, b: &DataLink, what: &str) {
    assert_eq!(a.tick(), b.tick(), "{what}: tick");
    assert_eq!(a.members(), b.members(), "{what}: members");
    assert_eq!(a.pictures(), b.pictures(), "{what}: pictures");
    assert_eq!(a.locks(), b.locks(), "{what}: locks");
    assert_eq!(a.engagements(), b.engagements(), "{what}: engagements");
    assert_eq!(a.assignments(), b.assignments(), "{what}: assignments");
    assert_eq!(a.warned(), b.warned(), "{what}: warned");
    assert_eq!(a.seat_warned(), b.seat_warned(), "{what}: seat warned");
}

/// The crowd fixture's fight with handoffs (two locks held from tick 560 on),
/// restored into a fresh world at ticks before, at and after each publish:
/// the picture, the locks and the engagements come out as the original's,
/// and the restored world codes the same bytes.
#[test]
fn the_picture_round_trips_at_ticks_around_a_publish() {
    let scenario = checkpoint_scenarios::crowd_handoffs();
    let mut world = (scenario.build)();
    let mut done = 0;
    let mut published = 0;
    let mut locked = 0;
    for tick in [
        1,
        PUBLISH_TICKS - 1,
        PUBLISH_TICKS,
        PUBLISH_TICKS + 1,
        2 * PUBLISH_TICKS - 1,
        2 * PUBLISH_TICKS,
        2 * PUBLISH_TICKS + 1,
        300,
        570,
        600,
        629,
        630,
        631,
    ] {
        advance(&mut world, &scenario, done, tick);
        done = tick;
        let bytes = world.checkpoint_sections(&LINK).unwrap();
        let mut fresh = (scenario.build)();
        assert_eq!(fresh.restore_sections(&bytes).unwrap(), LINK);
        let what = format!("tick {tick}");
        same_link(&world.datalink, &fresh.datalink, &what);
        // The header holds the world's tick, which only the combat section
        // restores, so the sections' bodies are what must match.
        let again = fresh.checkpoint_sections(&LINK).unwrap();
        let (x, y) = (
            crate::checkpoint::layout(&bytes).unwrap(),
            crate::checkpoint::layout(&again).unwrap(),
        );
        for section in LINK {
            assert!(
                x.body(&bytes, section) == y.body(&again, section),
                "{what}: a restored {} codes differently",
                section.name()
            );
        }
        // Before the first publish the picture is empty; after it, it is the
        // newest publishing tick's.
        match world.datalink.pictures().first() {
            None => assert!(tick < PUBLISH_TICKS, "{what}: nothing published"),
            Some(picture) => {
                published += 1;
                assert_eq!(picture.tick, world.tick() / PUBLISH_TICKS * PUBLISH_TICKS);
            }
        }
        locked += world.datalink.locks().len();
    }
    assert!(published >= 10 && locked > 0, "{published} {locked}");
}

/// A link wiped from a twin and restored from the original steps on like the
/// original: the same link and recorder every step for 300 steps. (The wipe
/// alone is shown to differ, so the restore is what makes them equal.)
#[test]
fn a_link_restored_into_a_wiped_twin_steps_on_identically() {
    for scenario in [
        checkpoint_scenarios::crowd_handoffs(),
        checkpoint_scenarios::damaged_aircraft(),
    ] {
        let at = scenario.at;
        let mut a = scenario.flown(at);
        let mut b = scenario.flown(at);
        b.datalink = DataLink::default();
        b.score = None;
        assert!(
            a.checkpoint_sections(&LINK).unwrap() != b.checkpoint_sections(&LINK).unwrap(),
            "{}: the wipe changed nothing",
            scenario.name
        );
        let bytes = a.checkpoint_sections(&LINK).unwrap();
        assert_eq!(b.restore_sections(&bytes).unwrap(), LINK);
        let (mut out_a, mut out_b) = (TickOutput::default(), TickOutput::default());
        for step in at..at + 300 {
            let (commands, inputs) = (scenario.drive)(&mut a, step);
            a.step_with(&commands, &inputs, &mut out_a, |_, _| Ok(()))
                .unwrap();
            let (commands, inputs) = (scenario.drive)(&mut b, step);
            b.step_with(&commands, &inputs, &mut out_b, |_, _| Ok(()))
                .unwrap();
            assert!(
                a.checkpoint_sections(&LINK).unwrap() == b.checkpoint_sections(&LINK).unwrap(),
                "{}: the link differs after step {step}",
                scenario.name
            );
            // By value too, so a field the coder leaves out shows.
            same_link(&a.datalink, &b.datalink, &format!("step {step}"));
        }
    }
}

/// The recorder's switch and its recorded targets travel with the checkpoint,
/// both ways: a world with scoring off turns it on, and one with it on turns
/// it off.
#[test]
fn the_scoring_switch_and_the_recorded_targets_are_restored() {
    let on = checkpoint_scenarios::damaged_aircraft();
    let world = on.flown(on.at);
    let bytes = world.checkpoint_sections(&[Section::Score]).unwrap();
    let mut fresh = (on.build)();
    assert!(fresh.scoring());
    assert_eq!(fresh.score.as_ref().unwrap().recorded().count(), 0);
    fresh.set_scoring(false);
    assert_eq!(fresh.restore_sections(&bytes).unwrap(), [Section::Score]);
    assert!(fresh.scoring());
    assert_eq!(fresh.score, world.score);
    assert_eq!(
        fresh.score.as_ref().unwrap().recorded().collect::<Vec<_>>(),
        [7]
    );
    // The kill waiting is the original's: draining both gives the same facts.
    let (mut a, mut b) = (world, fresh);
    assert_eq!(a.take_score_facts(), b.take_score_facts());

    let off = checkpoint_scenarios::crowd_fight();
    let world = off.flown(500);
    assert!(!world.scoring());
    let bytes = world.checkpoint_sections(&[Section::Score]).unwrap();
    let mut fresh = (off.build)();
    fresh.set_scoring(true);
    fresh.restore_sections(&bytes).unwrap();
    assert!(!fresh.scoring());
}

/// Damaged data link and score sections are refused or decoded to something
/// odd, never a panic: every cut, and 10,000 flipped bits, past the CRC.
#[test]
fn damaged_link_sections_are_refused_without_a_panic() {
    let scenario = checkpoint_scenarios::damaged_aircraft();
    let world = scenario.flown(scenario.at);
    let bytes = world.checkpoint_sections(&LINK).unwrap();
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
    let mut refused = 0;
    for _ in 0..10_000 {
        let mut damaged = bytes.clone();
        let at = next() as usize % (damaged.len() - 4);
        damaged[at] ^= 1 << (next() % 8);
        if fresh.restore_sections(&with_crc(damaged)).is_err() {
            refused += 1;
        }
    }
    assert!(refused > 0, "no damaged section was refused");
}
