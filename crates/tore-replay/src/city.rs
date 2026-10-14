//! The city section (nr-fx3, John 2026-10-13: replays track destroyed city
//! fill buildings): which of the city fill's background buildings were
//! knocked down, tick by tick, so a replay shows the flattened blocks and
//! the fires on them. A chunk keeps it apart from the frames, so a reader
//! that does not know the section skips it and every older file stays as it
//! was (a new section, not a new format version).
//!
//! A building is named by where it stands, its base centre in whole feet
//! (east, north), not by its index in the fill: the viewer rebuilds the fill
//! and finds each building there, so a fill rebuilt with more or fewer
//! buildings (other import resources, a later build) still shows the right
//! ones flat, and one it does not have is skipped.
//!
//! An entry names a frame, whether every building stood again first (a
//! restarted flight), and the buildings knocked down on it: the first
//! position as it is, each later one as its step from the one before, all
//! signed. Only changes are stored: the set at a tick is every entry up to
//! it, from the recording's start or its last stand-again.

use crate::codec::{In, put_iv, put_uv};
use crate::error::{Result, corrupt};
use crate::limits::MAX_CITY_PER_TICK;
use crate::model::Frame;
use crate::spawns::{get_gap, put_gap};

/// Writer state for one chunk's city section.
#[derive(Default)]
pub(crate) struct CityCoder {
    entries: u64,
    last_frame: Option<u32>,
    body: Vec<u8>,
}

/// An entry's flags: every building stood again before its buildings fell.
const STOOD: u64 = 1;

impl CityCoder {
    pub fn put(&mut self, index: u32, frame: &Frame) {
        if frame.city_fell.is_empty() && !frame.city_stood {
            return;
        }
        self.entries += 1;
        put_gap(&mut self.body, index, &mut self.last_frame);
        put_uv(&mut self.body, if frame.city_stood { STOOD } else { 0 });
        put_uv(&mut self.body, frame.city_fell.len() as u64);
        let mut last = [0i64; 2];
        for at in &frame.city_fell {
            for axis in 0..2 {
                let v = i64::from(at[axis]);
                put_iv(&mut self.body, v - last[axis]);
                last[axis] = v;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.body.len()
    }

    pub fn section(&self) -> Option<Vec<u8>> {
        (self.entries > 0).then(|| {
            let mut out = Vec::with_capacity(self.body.len() + 10);
            put_uv(&mut out, self.entries);
            out.extend_from_slice(&self.body);
            out
        })
    }
}

/// One entry: the frame, whether every building stood again first, and
/// where the buildings that fell stand.
pub(crate) type Entry = (u32, bool, Vec<[i32; 2]>);

/// The entries of a chunk's city section.
pub(crate) fn get_city(section: &[u8], frames: u32) -> Result<Vec<Entry>> {
    let mut input = In::new(section);
    let entries = input.count(frames as usize, "city entries")?;
    let mut out = Vec::with_capacity(entries);
    let mut last = None;
    for _ in 0..entries {
        let frame = get_gap(&mut input, &mut last, frames)?;
        let stood = match input.uv()? {
            0 => false,
            STOOD => true,
            _ => return Err(corrupt("a city entry's flags are unknown")),
        };
        let count = input.count(MAX_CITY_PER_TICK, "knocked-down city buildings")?;
        let mut fell = Vec::with_capacity(count);
        let mut at = [0i64; 2];
        for _ in 0..count {
            for value in &mut at {
                *value = value
                    .checked_add(input.iv()?)
                    .filter(|v| i32::try_from(*v).is_ok())
                    .ok_or_else(|| corrupt("a city building's position is out of range"))?;
            }
            fell.push(at.map(|v| v as i32));
        }
        out.push((frame, stood, fell));
    }
    if !input.done() {
        return Err(corrupt("the city section has trailing bytes"));
    }
    Ok(out)
}

/// Puts each entry's buildings on its frame.
pub(crate) fn apply_city(frames: &mut [Frame], entries: Vec<Entry>) {
    for (index, stood, fell) in entries {
        let frame = &mut frames[index as usize];
        frame.city_stood = stood;
        frame.city_fell = fell;
    }
}

/// The city fill's knocked-down buildings over a whole recording, in tick
/// order: what a viewer needs to show the city as it stood at any tick,
/// seeking and playing backwards included.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CityTrack {
    /// `(tick, stood, fell)`: on `tick` every building stood again first
    /// when `stood`, then the buildings standing at `fell` (east, north,
    /// whole feet) were knocked down.
    pub changes: Vec<(u64, bool, Vec<[i32; 2]>)>,
}

impl CityTrack {
    /// Whether the recording knocked down any city building (an older
    /// recording never does: it has no city section).
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// How many changes fall on or before `tick`: the set at `tick` is the
    /// same for any two ticks with the same count.
    pub fn changes_at(&self, tick: u64) -> usize {
        self.changes.partition_point(|(t, ..)| *t <= tick)
    }

    /// Where the buildings flat at `tick` stand, sorted, each once.
    pub fn flattened_at(&self, tick: u64) -> Vec<[i32; 2]> {
        let upto = self.changes_at(tick);
        let from = self.changes[..upto]
            .iter()
            .rposition(|(_, stood, _)| *stood)
            .unwrap_or(0);
        let mut out: Vec<[i32; 2]> = self.changes[from..upto]
            .iter()
            .flat_map(|(_, _, fell)| fell.iter().copied())
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Where the buildings flat at `tick` stand, each with the tick it fell
    /// on (nr-fx4): the first change since the last stand-again that names
    /// it. A burning building's smoke column grows from that tick, so a
    /// viewer shows the column at the height it had, however the playhead
    /// got to `tick`. Sorted by place, each once.
    pub fn fell_ticks_at(&self, tick: u64) -> Vec<([i32; 2], u64)> {
        let upto = self.changes_at(tick);
        let from = self.changes[..upto]
            .iter()
            .rposition(|(_, stood, _)| *stood)
            .unwrap_or(0);
        let mut out = std::collections::BTreeMap::new();
        for (when, _, fell) in &self.changes[from..upto] {
            for place in fell {
                out.entry(*place).or_insert(*when);
            }
        }
        out.into_iter().collect()
    }

    /// Every building the recording ever knocked down, each once.
    pub fn all(&self) -> Vec<[i32; 2]> {
        let mut all: Vec<[i32; 2]> = self
            .changes
            .iter()
            .flat_map(|(_, _, fell)| fell.iter().copied())
            .collect();
        all.sort_unstable();
        all.dedup();
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_track_gives_the_set_at_any_tick_and_a_stand_again_clears_it() {
        let track = CityTrack {
            changes: vec![
                (100, false, vec![[5, 1], [9, 1]]),
                (200, false, vec![[2, 0]]),
                (300, true, vec![[7, 7]]),
                (400, false, vec![[9, 1]]),
            ],
        };
        assert!(track.flattened_at(99).is_empty());
        assert_eq!(track.flattened_at(100), [[5, 1], [9, 1]]);
        assert_eq!(track.flattened_at(250), [[2, 0], [5, 1], [9, 1]]);
        assert_eq!(track.flattened_at(300), [[7, 7]]);
        assert_eq!(track.flattened_at(10_000), [[7, 7], [9, 1]]);
        assert_eq!(track.changes_at(250), 2);
        assert_eq!(track.all().len(), 4);
        // Each building's fall tick is its first change since the last
        // stand-again, so a seek and a play-through agree.
        assert_eq!(
            track.fell_ticks_at(250),
            [([2, 0], 200), ([5, 1], 100), ([9, 1], 100)]
        );
        assert_eq!(track.fell_ticks_at(300), [([7, 7], 300)]);
        assert_eq!(track.fell_ticks_at(450), [([7, 7], 300), ([9, 1], 400)]);
        assert!(track.fell_ticks_at(99).is_empty());
    }

    #[test]
    fn entries_round_trip_and_damage_is_refused() {
        let mut frames: Vec<Frame> = (0..4).map(|_| Frame::default()).collect();
        frames[1].city_fell = vec![[1_090_751, 839_493], [-5, 7], [i32::MAX, i32::MIN]];
        frames[3].city_stood = true;
        frames[3].city_fell = vec![[0, 0]];
        let mut coder = CityCoder::default();
        for (i, f) in frames.iter().enumerate() {
            coder.put(i as u32, f);
        }
        let section = coder.section().unwrap();
        let entries = get_city(&section, 4).unwrap();
        assert_eq!(entries[0].2, frames[1].city_fell);
        assert_eq!(entries[1], (3, true, vec![[0, 0]]));
        let mut back: Vec<Frame> = (0..4).map(|_| Frame::default()).collect();
        apply_city(&mut back, entries);
        assert_eq!(back, frames);
        // A frame past the chunk, and trailing bytes, are damage.
        assert!(get_city(&section, 2).is_err());
        let mut long = section.clone();
        long.push(0);
        assert!(get_city(&long, 4).is_err());
        // Nothing changed: no section at all.
        assert!(CityCoder::default().section().is_none());
    }
}
