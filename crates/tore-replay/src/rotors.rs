//! The rotors section: each rotorcraft's rotor speed, a share of its governed
//! 100 percent, quantized to a thousandth. A chunk keeps it apart from the
//! frames' aircraft records, so a reader that does not know the section
//! skips it and every older file stays as it was (a new section, not a new
//! format version).
//!
//! An entry names a frame, then the aircraft whose rotor speed changed on
//! it: their id and the new value. Between entries a value holds, within the
//! chunk; the chunk's first sample of an aircraft is always an entry, since
//! a chunk starts from nothing, and is written exactly (the format's key
//! rule); later changes are quantized. Aircraft that never appear are 0,
//! which is also what every aircraft without a rotor reads.

use crate::codec::{In, put_uv, put_xf64};
use crate::error::{Result, corrupt};
use crate::limits::MAX_AIRCRAFT;
use crate::model::{AircraftState, Frame};
use crate::predict::precision::ROTOR_SPEED as STEP;
use crate::spawns::{get_gap, put_gap};
use std::collections::{HashMap, HashSet};

/// The largest value a rotor speed takes, so the quantized number stays
/// small and a damaged file cannot ask for a huge one.
pub(crate) const MAX_ROTOR_SPEED: f64 = 60.;

/// A rotor speed as the number of steps it is stored in.
pub(crate) fn quantize(speed: f64) -> u64 {
    (speed.clamp(0., MAX_ROTOR_SPEED) / STEP).round() as u64
}

/// The rotor speed a stored number stands for.
pub(crate) fn dequantize(steps: u64) -> f64 {
    steps as f64 * STEP
}

/// Writer state for one chunk's rotors section.
#[derive(Default)]
pub(crate) struct RotorCoder {
    entries: u64,
    last_frame: Option<u32>,
    body: Vec<u8>,
    /// What the reader holds for each aircraft now.
    held: HashMap<u32, f64>,
    /// Aircraft whose first nonzero value this chunk was written exactly.
    exact: HashSet<u32>,
}

/// An entry's value: 0 and then the exact number, or one more than the
/// quantized steps.
const EXACT: u64 = 0;

impl RotorCoder {
    pub fn put(&mut self, frame: u32, aircraft: &[AircraftState]) {
        let mut changed = Vec::new();
        for a in aircraft {
            let held = self.held.get(&a.id).copied().unwrap_or(0.);
            if (a.rotor_speed - held).abs() <= STEP / 2. {
                continue;
            }
            let exact = a.rotor_speed > 0. && self.exact.insert(a.id);
            changed.push((a.id, a.rotor_speed, exact));
        }
        if changed.is_empty() {
            return;
        }
        self.entries += 1;
        put_gap(&mut self.body, frame, &mut self.last_frame);
        put_uv(&mut self.body, changed.len() as u64);
        for (id, speed, exact) in changed {
            put_uv(&mut self.body, u64::from(id));
            if exact {
                put_uv(&mut self.body, EXACT);
                put_xf64(&mut self.body, speed);
                self.held.insert(id, speed);
            } else {
                let steps = quantize(speed);
                put_uv(&mut self.body, steps + 1);
                self.held.insert(id, dequantize(steps));
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

/// What a rotors entry sets: the frame, then `(id, speed)` of each aircraft.
pub(crate) type Entry = (u32, Vec<(u32, f64)>);

/// The entries of a chunk's rotors section.
pub(crate) fn get_rotors(section: &[u8], frames: u32) -> Result<Vec<Entry>> {
    let mut input = In::new(section);
    let entries = input.count(frames as usize, "rotor entries")?;
    let mut out = Vec::with_capacity(entries);
    let mut last = None;
    for _ in 0..entries {
        let frame = get_gap(&mut input, &mut last, frames)?;
        let count = input.count(MAX_AIRCRAFT, "rotor speeds")?;
        let mut changed = Vec::with_capacity(count);
        for _ in 0..count {
            let id = input.u32v()?;
            let speed = match input.uv()? {
                EXACT => input.xf64()?,
                steps => dequantize(steps - 1),
            };
            if !(0. ..=MAX_ROTOR_SPEED).contains(&speed) {
                return Err(corrupt("a rotor speed is out of range"));
            }
            changed.push((id, speed));
        }
        out.push((frame, changed));
    }
    if !input.done() {
        return Err(corrupt("the rotors section has trailing bytes"));
    }
    Ok(out)
}

/// Gives every aircraft in `frames` the rotor speed the entries last set for
/// it.
pub(crate) fn apply_rotors(frames: &mut [Frame], entries: Vec<Entry>) {
    let mut entries = entries.into_iter().peekable();
    let mut current: HashMap<u32, f64> = HashMap::new();
    for (index, frame) in frames.iter_mut().enumerate() {
        while let Some((_, changed)) = entries.next_if(|(i, _)| *i as usize == index) {
            current.extend(changed);
        }
        for aircraft in &mut frame.aircraft {
            if let Some(speed) = current.get(&aircraft.id) {
                aircraft.rotor_speed = *speed;
            }
        }
    }
}
