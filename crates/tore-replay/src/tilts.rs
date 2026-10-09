//! The disk tilt section: each rotorcraft's rotor disk tilts, in radians,
//! quantized to 1/256 (the step the network protocol uses for the same
//! number). Like the rotors section it sits apart from the frames' aircraft
//! records, so a reader that does not know the section skips it and every
//! older file stays as it was (a new section, not a new format version).
//!
//! An entry names a frame, then the aircraft whose tilt changed on it: their
//! id and the four values (longitudinal then lateral, for each of the first
//! and second rotor). Between entries a value holds, within the chunk. A chunk
//! starts from nothing, so an aircraft's first nonzero sample in a chunk is
//! always an entry, and is written exactly (the format's key rule); later
//! changes are quantized. A value read is always within half a step of the
//! value written, and aircraft that never appear are level, which is also what
//! every aircraft without a rotor reads.

use crate::codec::{In, put_iv, put_uv, put_xf64};
use crate::error::{Result, corrupt};
use crate::limits::MAX_AIRCRAFT;
use crate::model::{AircraftState, Frame};
use crate::predict::precision::DISK_TILT as STEP;
use crate::spawns::{get_gap, put_gap};
use std::collections::{HashMap, HashSet};

/// The largest tilt a disk takes, in radians, so a damaged file cannot ask for
/// a huge number and the writer refuses nonsense. Real disks stay under about
/// 0.6 rad.
pub(crate) const MAX_DISK_TILT: f64 = 1.6;

/// A tilt as the number of steps it is stored in.
pub(crate) fn quantize(tilt: f64) -> i64 {
    (tilt.clamp(-MAX_DISK_TILT, MAX_DISK_TILT) / STEP).round() as i64
}

/// The tilt a stored number stands for.
pub(crate) fn dequantize(steps: i64) -> f64 {
    steps as f64 * STEP
}

fn flat(tilt: [[f64; 2]; 2]) -> [f64; 4] {
    [tilt[0][0], tilt[0][1], tilt[1][0], tilt[1][1]]
}

fn shaped(values: [f64; 4]) -> [[f64; 2]; 2] {
    [[values[0], values[1]], [values[2], values[3]]]
}

/// An entry's form: 0 and then the four exact numbers, or 1 and then four
/// quantized step counts.
const EXACT: u64 = 0;
const QUANTIZED: u64 = 1;

/// Writer state for one chunk's disk tilt section.
#[derive(Default)]
pub(crate) struct TiltCoder {
    entries: u64,
    last_frame: Option<u32>,
    body: Vec<u8>,
    /// What the reader holds for each aircraft now.
    held: HashMap<u32, [f64; 4]>,
    /// Aircraft whose first nonzero value this chunk was written exactly.
    exact: HashSet<u32>,
}

impl TiltCoder {
    pub fn put(&mut self, frame: u32, aircraft: &[AircraftState]) {
        let mut changed = Vec::new();
        for a in aircraft {
            let now = flat(a.disk_tilt);
            let held = self.held.get(&a.id).copied().unwrap_or([0.; 4]);
            if now
                .iter()
                .zip(held)
                .all(|(n, h)| (n - h).abs() <= STEP / 2.)
            {
                continue;
            }
            let exact = now.iter().any(|v| *v != 0.) && self.exact.insert(a.id);
            changed.push((a.id, now, exact));
        }
        if changed.is_empty() {
            return;
        }
        self.entries += 1;
        put_gap(&mut self.body, frame, &mut self.last_frame);
        put_uv(&mut self.body, changed.len() as u64);
        for (id, now, exact) in changed {
            put_uv(&mut self.body, u64::from(id));
            if exact {
                put_uv(&mut self.body, EXACT);
                for value in now {
                    put_xf64(&mut self.body, value);
                }
                self.held.insert(id, now);
            } else {
                put_uv(&mut self.body, QUANTIZED);
                let steps = now.map(quantize);
                for value in steps {
                    put_iv(&mut self.body, value);
                }
                self.held.insert(id, steps.map(dequantize));
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

/// What a tilt entry sets: the frame, then `(id, tilt)` of each aircraft.
pub(crate) type Entry = (u32, Vec<(u32, [[f64; 2]; 2])>);

/// The entries of a chunk's disk tilt section.
pub(crate) fn get_tilts(section: &[u8], frames: u32) -> Result<Vec<Entry>> {
    let mut input = In::new(section);
    let entries = input.count(frames as usize, "disk tilt entries")?;
    let mut out = Vec::with_capacity(entries);
    let mut last = None;
    for _ in 0..entries {
        let frame = get_gap(&mut input, &mut last, frames)?;
        let count = input.count(MAX_AIRCRAFT, "disk tilts")?;
        let mut changed = Vec::with_capacity(count);
        for _ in 0..count {
            let id = input.u32v()?;
            let mut values = [0.; 4];
            match input.uv()? {
                EXACT => {
                    for value in &mut values {
                        *value = input.xf64()?;
                    }
                }
                QUANTIZED => {
                    for value in &mut values {
                        *value = dequantize(input.iv()?);
                    }
                }
                _ => return Err(corrupt("a disk tilt entry has an unknown form")),
            }
            if values
                .iter()
                .any(|v| !v.is_finite() || v.abs() > MAX_DISK_TILT + STEP)
            {
                return Err(corrupt("a disk tilt is out of range"));
            }
            changed.push((id, shaped(values)));
        }
        out.push((frame, changed));
    }
    if !input.done() {
        return Err(corrupt("the disk tilt section has trailing bytes"));
    }
    Ok(out)
}

/// Gives every aircraft in `frames` the disk tilt the entries last set for it.
pub(crate) fn apply_tilts(frames: &mut [Frame], entries: Vec<Entry>) {
    let mut entries = entries.into_iter().peekable();
    let mut current: HashMap<u32, [[f64; 2]; 2]> = HashMap::new();
    for (index, frame) in frames.iter_mut().enumerate() {
        while let Some((_, changed)) = entries.next_if(|(i, _)| *i as usize == index) {
            current.extend(changed);
        }
        for aircraft in &mut frame.aircraft {
            if let Some(tilt) = current.get(&aircraft.id) {
                aircraft.disk_tilt = *tilt;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(id: u32, disk_tilt: [[f64; 2]; 2]) -> AircraftState {
        AircraftState {
            id,
            disk_tilt,
            ..AircraftState::default()
        }
    }

    #[test]
    fn an_unchanged_or_level_disk_writes_nothing() {
        let mut coder = TiltCoder::default();
        coder.put(0, &[plane(1, [[0.; 2]; 2]), plane(2, [[0.; 2]; 2])]);
        assert!(coder.section().is_none());
        coder.put(1, &[plane(1, [[0.1, 0.], [0.; 2]])]);
        coder.put(2, &[plane(1, [[0.1 + STEP / 4., 0.], [0.; 2]])]);
        let entries = get_tilts(&coder.section().unwrap(), 3).unwrap();
        assert_eq!(
            entries.len(),
            1,
            "a change under half a step is not an entry"
        );
        assert_eq!(entries[0].0, 1);
        assert_eq!(entries[0].1[0].1[0][0], 0.1, "the first value is exact");
    }

    #[test]
    fn later_values_are_within_half_a_step_and_trailing_bytes_are_damage() {
        let mut coder = TiltCoder::default();
        let truth = [[0.2503, -0.0171], [-0.5, 0.0049]];
        coder.put(0, &[plane(7, [[0.01, 0.], [0.; 2]])]);
        coder.put(1, &[plane(7, truth)]);
        let section = coder.section().unwrap();
        let entries = get_tilts(&section, 2).unwrap();
        assert_eq!(entries[0].1[0].1[0][0], 0.01);
        let read = entries[1].1[0].1;
        for (rotor, axis) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            assert!((read[rotor][axis] - truth[rotor][axis]).abs() <= STEP / 2. + 1e-12);
        }
        let mut damaged = section;
        damaged.push(0);
        assert!(get_tilts(&damaged, 2).is_err());
    }
}
