//! The coders of the lag-compensation history: one second of every aircraft's
//! hit volume, the largest state per aircraft in a combat checkpoint
//! (docs/formats/checkpoint.md).
//!
//! Each frame is coded against the one before it (*agent decision* of the
//! design): a tick is one more than the last, and an aircraft's volume moves
//! a little each tick, so the exclusive-or coding of an unchanged field costs
//! one bit. The first frame is coded against nothing. Two choices keep the
//! frames small without losing a bit:
//!
//! - A volume is coded against the same aircraft's volume in the frame
//!   before, found by its id, so aircraft coming and going do not spoil the
//!   baseline of the others.
//! - A volume's `previous` position is where the aircraft was the tick
//!   before, which is the earlier frame's `position`, bit for bit. One flag
//!   says so, and the three floats are not coded. (An exclusive-or of a
//!   moving float still costs about 55 bits, so this alone saves a fifth.)
//!
//! Only the newest [`REACHABLE`] frames are coded (*agent decision*). The
//! history keeps a second, but every lookup rewinds at most
//! [`MAX_REWIND_TICKS`] ticks (the host and the state both cap it), and the
//! frames before that are never read again: they only fall off the front as
//! new ones arrive. Leaving them out halves the largest part of a
//! checkpoint and changes no tick; a restored history is shorter until the
//! missing second grows back, and `len` and `span` (read by tests only) show
//! it. The same coding is written by the original and by a restored world, so
//! their checkpoints stay byte for byte equal.
//!
//! A decoded history is checked to be what `record` builds: at most
//! [`HISTORY_TICKS`] frames, ticks strictly rising, volumes in id order,
//! because `volume` searches both by bisection and a disordered list would
//! silently answer wrongly.

use super::{Frame, HISTORY_TICKS, History, HitVolume, MAX_REWIND_TICKS};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};
use std::collections::VecDeque;

/// The frames a lookup can reach: a rewind of [`MAX_REWIND_TICKS`] ticks
/// reads the frames of the newest tick and the 60 before it.
pub const REACHABLE: usize = MAX_REWIND_TICKS as usize + 1;
const _: () = assert!(REACHABLE <= HISTORY_TICKS);

/// Whether `a` and `b` are the same three floats, bit for bit.
fn same_bits(a: [f64; 3], b: [f64; 3]) -> bool {
    a.map(f64::to_bits) == b.map(f64::to_bits)
}

impl Checkpoint for HitVolume {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        let HitVolume {
            position,
            previous,
            basis,
            radius,
        } = self;
        position.save(s, base.map(|b| &b.position))?;
        let continues = base.is_some_and(|b| same_bits(b.position, *previous));
        continues.save(s, None)?;
        if !continues {
            previous.save(s, base.map(|b| &b.previous))?;
        }
        basis.save(s, base.map(|b| &b.basis))?;
        radius.save(s, base.map(|b| &b.radius))?;
        Ok(())
    }

    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        let position = Checkpoint::load(l, base.map(|b| &b.position))?;
        let continues = bool::load(l, None)?;
        let previous = match (continues, base) {
            (true, Some(base)) => base.position,
            (true, None) => return invalid("a rewind volume continues from nothing"),
            (false, _) => Checkpoint::load(l, base.map(|b| &b.previous))?,
        };
        Ok(HitVolume {
            position,
            previous,
            basis: Checkpoint::load(l, base.map(|b| &b.basis))?,
            radius: Checkpoint::load(l, base.map(|b| &b.radius))?,
        })
    }
}

impl Frame {
    /// This frame's volume of aircraft `id`.
    fn volume_of(&self, id: u32) -> Option<&HitVolume> {
        self.volumes
            .binary_search_by_key(&id, |(id, _)| *id)
            .ok()
            .map(|index| &self.volumes[index].1)
    }
}

impl Checkpoint for Frame {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        let Frame { tick, volumes } = self;
        tick.save(s, base.map(|b| &b.tick))?;
        s.count(volumes.len());
        for (index, (id, volume)) in volumes.iter().enumerate() {
            // The id against the baseline's id at the same place (the same
            // aircraft nearly always), the volume against that aircraft's.
            let guess = base.and_then(|b| b.volumes.get(index)).map(|(id, _)| id);
            id.save(s, guess)?;
            volume.save(s, base.and_then(|b| b.volume_of(*id)))?;
        }
        Ok(())
    }

    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        let tick = Checkpoint::load(l, base.map(|b| &b.tick))?;
        let count = l.count()?;
        let mut volumes: Vec<(u32, HitVolume)> = Vec::with_capacity(count);
        for index in 0..count {
            let guess = base.and_then(|b| b.volumes.get(index)).map(|(id, _)| id);
            let id: u32 = Checkpoint::load(l, guess)?;
            let volume = HitVolume::load(l, base.and_then(|b| b.volume_of(id)))?;
            volumes.push((id, volume));
        }
        Ok(Frame { tick, volumes })
    }
}

impl Checkpoint for History {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let History { frames } = self;
        let skipped = frames.len().saturating_sub(REACHABLE);
        s.count(frames.len() - skipped);
        let mut before: Option<&Frame> = None;
        for frame in frames.iter().skip(skipped) {
            frame.save(s, before)?;
            before = Some(frame);
        }
        Ok(())
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let count = l.count()?;
        if count > REACHABLE {
            return invalid(format!(
                "a rewind history of {count} frames, at most {REACHABLE}"
            ));
        }
        let mut frames: VecDeque<Frame> = VecDeque::with_capacity(count);
        for _ in 0..count {
            let frame = Frame::load(l, frames.back())?;
            if frames
                .back()
                .is_some_and(|before| before.tick >= frame.tick)
            {
                return invalid("rewind frames out of order");
            }
            if frame.volumes.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
                return invalid("rewind volumes out of order");
            }
            frames.push_back(frame);
        }
        Ok(History { frames })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attitude::Basis;
    use crate::checkpoint::{Models, from_bytes, round_trip, to_bytes};

    /// One aircraft's volume at `tick`: it flies a gentle turn.
    fn volume(id: u32, tick: u64) -> HitVolume {
        let t = tick as f64 / 120.;
        let phase = f64::from(id) * 0.37;
        let position = [
            f64::from(id) * 2_000. + 450. * t,
            10_000. + 30. * (t + phase).sin(),
            60_000. - 520. * t * (1. + phase / 10.),
        ];
        let before = (tick.max(1) - 1) as f64 / 120.;
        let previous = [
            f64::from(id) * 2_000. + 450. * before,
            10_000. + 30. * (before + phase).sin(),
            60_000. - 520. * before * (1. + phase / 10.),
        ];
        HitVolume {
            position,
            previous,
            basis: Basis::new(0.02 * (t + phase).sin(), 0.01 * t, 0.1 * (t + phase)),
            radius: 28.,
        }
    }

    fn history(aircraft: u32, ticks: u64) -> History {
        let mut history = History::default();
        for tick in 1..=ticks {
            history.record(tick, (0..aircraft).map(|id| (id * 3 + 1, volume(id, tick))));
        }
        history
    }

    #[test]
    fn a_history_round_trips_and_answers_alike() {
        let models = Models::default();
        let original = history(6, 150);
        assert_eq!(original.len(), HISTORY_TICKS);
        let copy = round_trip(&original, &models).unwrap();
        // Only the frames a lookup can reach are carried.
        assert_eq!(copy.len(), REACHABLE);
        assert_eq!(copy.span().map(|s| s.1), original.span().map(|s| s.1));
        for id in [1, 4, 16, 99] {
            for rewind in 0..=MAX_REWIND_TICKS {
                assert_eq!(copy.volume(id, rewind), original.volume(id, rewind));
            }
        }
        // It records on exactly as the original does.
        let (mut a, mut b) = (original, copy);
        for tick in 151..=160 {
            for h in [&mut a, &mut b] {
                h.record(tick, [(1, volume(0, tick)), (9, volume(2, tick))]);
            }
        }
        for id in [1, 4, 9, 16] {
            for rewind in 0..=MAX_REWIND_TICKS {
                assert_eq!(a.volume(id, rewind), b.volume(id, rewind), "{id} {rewind}");
            }
        }
        assert_eq!(a.span().map(|s| s.1), b.span().map(|s| s.1));
        // And codes the same bytes as the original from then on.
        assert_eq!(
            to_bytes(&a, &models).unwrap(),
            to_bytes(&b, &models).unwrap()
        );
        assert!(History::default().is_empty());
        let empty = round_trip(&History::default(), &models).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn a_young_history_is_carried_whole() {
        let models = Models::default();
        let young = history(3, 40);
        let copy = round_trip(&young, &models).unwrap();
        assert_eq!(copy.len(), 40);
        assert_eq!(copy.span(), young.span());
    }

    #[test]
    fn coding_each_frame_against_the_one_before_is_smaller() {
        let models = Models::default();
        let full = history(30, HISTORY_TICKS as u64);
        assert_eq!(full.len(), HISTORY_TICKS);
        let delta = to_bytes(&full, &models).unwrap().body.len();
        // The same frames, each coded against nothing.
        let mut alone = Saver::new();
        alone.count(REACHABLE);
        for frame in full.frames.iter().skip(HISTORY_TICKS - REACHABLE) {
            frame.save(&mut alone, None).unwrap();
        }
        let alone = alone.finish_section().len();
        println!(
            "rewind history, 30 aircraft: {REACHABLE} of {HISTORY_TICKS} frames coded, \
             {delta} bytes against the previous frame, {alone} bytes without a baseline, \
             {} bytes raw",
            REACHABLE * 30 * (4 + 16 * 8)
        );
        assert!(delta < alone, "{delta} bytes, {alone} without a baseline");
    }

    #[test]
    fn a_disordered_or_oversized_history_is_refused() {
        let models = Models::default();
        let good = history(3, 20);
        // Frames that go backwards in time.
        let mut backwards = good.clone();
        backwards.frames.swap(3, 4);
        let coded = to_bytes(&backwards, &models).unwrap();
        assert!(from_bytes::<History>(&coded, &models).is_err());
        // Volumes out of id order.
        let mut disordered = good.clone();
        disordered.frames[5].volumes.swap(0, 1);
        let coded = to_bytes(&disordered, &models).unwrap();
        assert!(from_bytes::<History>(&coded, &models).is_err());
        // More frames than a lookup can reach.
        let mut s = Saver::new();
        s.count(REACHABLE + 1);
        let mut before: Option<&Frame> = None;
        let long: Vec<Frame> = (1..=REACHABLE as u64 + 1)
            .map(|tick| Frame {
                tick,
                volumes: vec![(1, volume(0, tick))],
            })
            .collect();
        for frame in &long {
            frame.save(&mut s, before).unwrap();
            before = Some(frame);
        }
        let coded = crate::checkpoint::Coded {
            body: s.finish_section(),
            records: Vec::new(),
        };
        assert!(from_bytes::<History>(&coded, &models).is_err());
        // Cuts.
        let coded = to_bytes(&good, &models).unwrap();
        for cut in (0..coded.body.len()).step_by(7) {
            let mut short = coded.clone();
            short.body.truncate(cut);
            assert!(from_bytes::<History>(&short, &models).is_err(), "cut {cut}");
        }
    }
}
