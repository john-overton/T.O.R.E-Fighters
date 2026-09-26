//! The spawns section: effects and smoke or contrail puffs released in each
//! frame, kept apart from the frames so the viewer can gather two minutes of
//! smoke without decoding every aircraft.
//!
//! Positions are whole steps of 1/32 foot from the world origin. A puff is
//! predicted from the puff at the same place in the previous batch of its kind
//! (the simulation releases each kind in batches from a stable list of
//! sources), from the previous puff in this frame, or from nothing; the writer
//! picks whichever leaves the smallest residual.

use crate::codec::{In, put_iv, put_triple, put_uv, put_xf64, triple_len};
use crate::error::{Result, corrupt};
use crate::limits::{MAX_EFFECTS_PER_TICK, MAX_PUFFS_PER_TICK};
use crate::model::{EffectKind, EffectSpawn, PuffKind, PuffSpawn};
use crate::predict::precision::POSITION_FT as STEP;
use std::collections::HashMap;

const LIMIT: f64 = 1e9;

fn quantizable(p: [f64; 3]) -> bool {
    p.iter().all(|v| v.is_finite() && v.abs() <= LIMIT)
}

fn units(p: [f64; 3]) -> [i64; 3] {
    if quantizable(p) {
        p.map(|v| (v / STEP).round() as i64)
    } else {
        [0; 3]
    }
}

fn position(u: [i64; 3]) -> [f64; 3] {
    u.map(|v| v as f64 * STEP)
}

/// Frame gap coding shared by the sparse sections: the first entry stores its
/// frame index, later ones the frames skipped since the previous entry.
pub(crate) fn put_gap(buf: &mut Vec<u8>, frame: u32, last: &mut Option<u32>) {
    put_uv(
        buf,
        u64::from(match *last {
            None => frame,
            Some(previous) => frame - previous - 1,
        }),
    );
    *last = Some(frame);
}

pub(crate) fn get_gap(input: &mut In, last: &mut Option<u32>, frames: u32) -> Result<u32> {
    let gap = input.uv()?;
    let frame = match *last {
        None => gap,
        Some(previous) => u64::from(previous) + 1 + gap,
    };
    if frame >= u64::from(frames) {
        return Err(corrupt("an entry points past the chunk's last frame"));
    }
    *last = Some(frame as u32);
    Ok(frame as u32)
}

#[derive(Default)]
struct Batches {
    last: Vec<[i64; 3]>,
    before: Vec<[i64; 3]>,
}

/// Prediction state for puffs within one chunk.
#[derive(Default)]
struct PuffPredictor {
    batches: HashMap<u8, Batches>,
}

impl PuffPredictor {
    /// Candidate predictions in mode order.
    fn candidates(
        &self,
        kind: u8,
        ordinal: usize,
        previous: Option<[i64; 3]>,
    ) -> [Option<[i64; 3]>; 4] {
        let batch = self.batches.get(&kind);
        let last = batch.and_then(|b| b.last.get(ordinal)).copied();
        let before = batch.and_then(|b| b.before.get(ordinal)).copied();
        let second = match (last, before) {
            (Some(l), Some(b)) => Some([0, 1, 2].map(|i| l[i].wrapping_mul(2).wrapping_sub(b[i]))),
            _ => None,
        };
        [second, last, previous, Some([0; 3])]
    }

    fn finish_frame(&mut self, this_frame: HashMap<u8, Vec<[i64; 3]>>) {
        for (kind, units) in this_frame {
            let batch = self.batches.entry(kind).or_default();
            batch.before = std::mem::replace(&mut batch.last, units);
        }
    }
}

const PUFF_EXACT: u8 = 1 << 6;

/// Writer state for one chunk's spawns section.
#[derive(Default)]
pub(crate) struct SpawnCoder {
    entries: u64,
    last_frame: Option<u32>,
    body: Vec<u8>,
    puffs: PuffPredictor,
}

impl SpawnCoder {
    pub fn put(&mut self, frame: u32, effects: &[EffectSpawn], puffs: &[PuffSpawn]) {
        if effects.is_empty() && puffs.is_empty() {
            return;
        }
        let buf = &mut self.body;
        self.entries += 1;
        put_gap(buf, frame, &mut self.last_frame);
        put_uv(buf, effects.len() as u64);
        for effect in effects {
            buf.push(effect.kind.code());
            put_uv(buf, u64::from(effect.duration_ticks));
            if quantizable(effect.position) {
                buf.push(0);
                for v in units(effect.position) {
                    put_iv(buf, v);
                }
            } else {
                buf.push(1);
                for v in effect.position {
                    put_xf64(buf, v);
                }
            }
        }
        put_uv(buf, puffs.len() as u64);
        let mut ordinals: HashMap<u8, usize> = HashMap::new();
        let mut this_frame: HashMap<u8, Vec<[i64; 3]>> = HashMap::new();
        let mut previous = None;
        for puff in puffs {
            let kind = puff.kind.code();
            let ordinal = ordinals.entry(kind).or_insert(0);
            let kind_bits = kind.min(3);
            let layer_bits = puff.layer.min(2);
            let mut head = kind_bits | layer_bits << 2;
            let u = units(puff.position);
            let residual = if quantizable(puff.position) {
                let mut best = (usize::MAX, 0u8, [0i64; 3]);
                for (mode, candidate) in self
                    .puffs
                    .candidates(kind, *ordinal, previous)
                    .iter()
                    .enumerate()
                {
                    if let Some(pred) = candidate {
                        let r = [0, 1, 2].map(|i| u[i].wrapping_sub(pred[i]));
                        let len = triple_len(r);
                        if len < best.0 {
                            best = (len, mode as u8, r);
                        }
                    }
                }
                head |= best.1 << 4;
                Some(best.2)
            } else {
                head |= PUFF_EXACT;
                None
            };
            buf.push(head);
            if kind_bits == 3 {
                buf.push(kind);
            }
            if layer_bits == 2 {
                buf.push(puff.layer);
            }
            match residual {
                Some(r) => put_triple(buf, r),
                None => {
                    for v in puff.position {
                        put_xf64(buf, v);
                    }
                }
            }
            this_frame.entry(kind).or_default().push(u);
            previous = Some(u);
            *ordinal += 1;
        }
        self.puffs.finish_frame(this_frame);
    }

    pub fn section(&self) -> Option<Vec<u8>> {
        if self.entries == 0 {
            return None;
        }
        let mut out = Vec::with_capacity(self.body.len() + 10);
        put_uv(&mut out, self.entries);
        out.extend_from_slice(&self.body);
        Some(out)
    }

    pub fn len(&self) -> usize {
        self.body.len()
    }
}

/// Spawns of one frame: `(frame index, effects, puffs)`.
pub(crate) type FrameSpawns = (u32, Vec<EffectSpawn>, Vec<PuffSpawn>);

pub(crate) fn get_spawns(section: &[u8], frames: u32) -> Result<Vec<FrameSpawns>> {
    let mut input = In::new(section);
    let entries = input.count(frames as usize, "spawn entries")?;
    let mut out = Vec::with_capacity(entries);
    let mut last_frame = None;
    let mut predictor = PuffPredictor::default();
    for _ in 0..entries {
        let frame = get_gap(&mut input, &mut last_frame, frames)?;
        let n = input.count(MAX_EFFECTS_PER_TICK, "effects in one frame")?;
        let mut effects = Vec::with_capacity(n);
        for _ in 0..n {
            let kind = EffectKind::from_code(input.u8()?);
            let duration_ticks = input.u32v()?;
            let position = match input.u8()? {
                0 => {
                    let u = [input.iv()?, input.iv()?, input.iv()?];
                    position(u)
                }
                1 => [input.xf64()?, input.xf64()?, input.xf64()?],
                _ => return Err(corrupt("unknown effect position encoding")),
            };
            effects.push(EffectSpawn {
                kind,
                position,
                duration_ticks,
            });
        }
        let n = input.count(MAX_PUFFS_PER_TICK, "puffs in one frame")?;
        let mut puffs = Vec::with_capacity(n);
        let mut ordinals: HashMap<u8, usize> = HashMap::new();
        let mut this_frame: HashMap<u8, Vec<[i64; 3]>> = HashMap::new();
        let mut previous = None;
        for _ in 0..n {
            let head = input.u8()?;
            if head & 0x80 != 0 {
                return Err(corrupt("a puff has unknown bits"));
            }
            let kind = match head & 3 {
                3 => {
                    let code = input.u8()?;
                    if code < PuffKind::FIRST_OTHER {
                        return Err(corrupt("a puff kind is stored the long way"));
                    }
                    code
                }
                short => short,
            };
            let layer = match (head >> 2) & 3 {
                2 => {
                    let layer = input.u8()?;
                    if layer < 2 {
                        return Err(corrupt("a puff layer is stored the long way"));
                    }
                    layer
                }
                3 => return Err(corrupt("a puff layer is invalid")),
                short => short,
            };
            let ordinal = ordinals.entry(kind).or_insert(0);
            let (position, u) = if head & PUFF_EXACT != 0 {
                let p = [input.xf64()?, input.xf64()?, input.xf64()?];
                (p, units(p))
            } else {
                let mode = usize::from((head >> 4) & 3);
                let pred = predictor.candidates(kind, *ordinal, previous)[mode]
                    .ok_or_else(|| corrupt("a puff refers to a batch that does not exist"))?;
                let r = input.triple()?;
                let u = [0, 1, 2].map(|i| pred[i].wrapping_add(r[i]));
                (position(u), u)
            };
            puffs.push(PuffSpawn {
                layer,
                kind: PuffKind::from_code(kind),
                position,
            });
            this_frame.entry(kind).or_default().push(u);
            previous = Some(u);
            *ordinal += 1;
        }
        predictor.finish_frame(this_frame);
        out.push((frame, effects, puffs));
    }
    if !input.done() {
        return Err(corrupt("the spawns section has trailing bytes"));
    }
    Ok(out)
}
