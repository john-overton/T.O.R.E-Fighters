//! The frames section: every frame of a chunk, in tick order. Each frame
//! holds its entity lists and one record per entity. The first frame of a
//! chunk has no earlier state, so every record in it is an exact key record:
//! that is the chunk's keyframe.
//!
//! A list is stored as "same as last tick", "last tick's list with some
//! entries removed and new ones appended", or in full.

use crate::codec::{In, put_iv, put_uv};
use crate::error::{Result, corrupt};
use crate::limits::{
    MAX_AIRCRAFT, MAX_DEBRIS, MAX_ESCAPEES, MAX_PROJECTILES, MAX_SURFACE_CHANGES_PER_TICK,
    MAX_SURFACE_STOCK_PER_TICK, MAX_SURFACE_UNITS,
};
use crate::model::{Frame, SurfaceStock};
use crate::predict::{
    AircraftPred, DebrisPred, EscapeePred, ProjectilePred, SurfacePred, get_aircraft, get_debris,
    get_escapee, get_projectile, get_surface, put_aircraft, put_debris, put_escapee,
    put_projectile, put_surface,
};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

const LIST_SAME: u64 = 0;
const LIST_EDIT: u64 = 1;
const LIST_FULL: u64 = 2;

const FRAME_SURFACE: u64 = 1;
/// Format 3: the surface units' poses are present (this frame's, or the
/// emptying of last frame's).
const FRAME_UNITS: u64 = 1 << 1;
/// Format 3: launcher and magazine changes follow.
const FRAME_STOCK: u64 = 1 << 2;
/// Format 3: the pieces of surface owners' debris follow.
const FRAME_PIECES: u64 = 1 << 3;
const FRAME_KNOWN: u64 = FRAME_SURFACE | FRAME_UNITS | FRAME_STOCK | FRAME_PIECES;

/// How one kind of list key is written.
trait ListKey: Copy + Eq + Hash {
    fn put(self, buf: &mut Vec<u8>, previous: Option<Self>);
    fn get(input: &mut In, previous: Option<Self>) -> Result<Self>;
}

impl ListKey for u32 {
    fn put(self, buf: &mut Vec<u8>, previous: Option<Self>) {
        put_iv(buf, i64::from(self) - i64::from(previous.unwrap_or(0)));
    }

    fn get(input: &mut In, previous: Option<Self>) -> Result<Self> {
        u32::try_from(i64::from(previous.unwrap_or(0)).wrapping_add(input.iv()?))
            .map_err(|_| corrupt("an id is out of range"))
    }
}

impl ListKey for (u32, u32) {
    fn put(self, buf: &mut Vec<u8>, previous: Option<Self>) {
        self.0.put(buf, previous.map(|p| p.0));
        put_uv(buf, u64::from(self.1));
    }

    fn get(input: &mut In, previous: Option<Self>) -> Result<Self> {
        Ok((u32::get(input, previous.map(|p| p.0))?, input.u32v()?))
    }
}

fn put_list<K: ListKey>(buf: &mut Vec<u8>, previous: &[K], keys: &[K]) {
    if previous == keys {
        put_uv(buf, LIST_SAME);
        return;
    }
    let present: HashSet<K> = keys.iter().copied().collect();
    let kept: Vec<K> = previous
        .iter()
        .copied()
        .filter(|k| present.contains(k))
        .collect();
    if keys.starts_with(&kept) {
        put_uv(buf, LIST_EDIT);
        let removed: Vec<usize> = previous
            .iter()
            .enumerate()
            .filter(|(_, k)| !present.contains(k))
            .map(|(i, _)| i)
            .collect();
        put_uv(buf, removed.len() as u64);
        let mut last = None;
        for i in removed {
            put_uv(buf, (i - last.map_or(0, |l| l + 1)) as u64);
            last = Some(i);
        }
        let added = &keys[kept.len()..];
        put_uv(buf, added.len() as u64);
        let mut last = None;
        for key in added {
            key.put(buf, last);
            last = Some(*key);
        }
    } else {
        put_uv(buf, LIST_FULL);
        put_uv(buf, keys.len() as u64);
        let mut last = None;
        for key in keys {
            key.put(buf, last);
            last = Some(*key);
        }
    }
}

/// Reads a list. Owner lists may repeat an owner, so `unique` is false
/// for them.
fn get_list<K: ListKey>(
    input: &mut In,
    previous: &[K],
    max: usize,
    what: &str,
    unique: bool,
) -> Result<Vec<K>> {
    let keys = match input.uv()? {
        LIST_SAME => previous.to_vec(),
        LIST_EDIT => {
            let removed = input.count(previous.len(), "removed list entries")?;
            let mut gone = vec![false; previous.len()];
            let mut next = 0usize;
            for _ in 0..removed {
                let i = next
                    .checked_add(input.count(previous.len(), "list positions")?)
                    .filter(|i| *i < previous.len())
                    .ok_or_else(|| corrupt("a removed list entry is out of range"))?;
                gone[i] = true;
                next = i + 1;
            }
            let mut keys: Vec<K> = previous
                .iter()
                .zip(&gone)
                .filter(|(_, g)| !**g)
                .map(|(k, _)| *k)
                .collect();
            let added = input.count(max, what)?;
            if keys.len() + added > max {
                return Err(corrupt(format!("more {what} than the limit of {max}")));
            }
            let mut last = None;
            for _ in 0..added {
                let key = K::get(input, last)?;
                keys.push(key);
                last = Some(key);
            }
            keys
        }
        LIST_FULL => {
            let n = input.count(max, what)?;
            let mut keys = Vec::with_capacity(n);
            let mut last = None;
            for _ in 0..n {
                let key = K::get(input, last)?;
                keys.push(key);
                last = Some(key);
            }
            keys
        }
        _ => return Err(corrupt("unknown list encoding")),
    };
    if unique && keys.iter().copied().collect::<HashSet<K>>().len() != keys.len() {
        return Err(corrupt(format!("the {what} list repeats an entry")));
    }
    Ok(keys)
}

/// Escapees have no id of their own: the key is the owner and how many
/// earlier escapees in the list share that owner.
fn escapee_keys(owners: &[u32]) -> Vec<(u32, u32)> {
    let mut seen: HashMap<u32, u32> = HashMap::new();
    owners
        .iter()
        .map(|owner| {
            let n = seen.entry(*owner).or_insert(0);
            let key = (*owner, *n);
            *n += 1;
            key
        })
        .collect()
}

/// The state both sides hold while coding one chunk's frames.
#[derive(Default)]
pub(crate) struct FrameCoder {
    aircraft_keys: Vec<u32>,
    aircraft: HashMap<u32, AircraftPred>,
    projectile_keys: Vec<u32>,
    projectiles: HashMap<u32, ProjectilePred>,
    debris_keys: Vec<(u32, u32)>,
    debris: HashMap<(u32, u32), DebrisPred>,
    escapee_owners: Vec<u32>,
    escapees: HashMap<(u32, u32), EscapeePred>,
    surface_keys: Vec<u32>,
    surface: HashMap<u32, SurfacePred>,
}

impl FrameCoder {
    /// Writes one frame. The caller has validated it.
    pub fn put(&mut self, buf: &mut Vec<u8>, frame: &Frame) {
        let flags = if frame.surface_hp.is_empty() {
            0
        } else {
            FRAME_SURFACE
        } | if frame.surface.is_empty() && self.surface_keys.is_empty() {
            0
        } else {
            FRAME_UNITS
        } | if frame.surface_stock.is_empty() {
            0
        } else {
            FRAME_STOCK
        } | if frame.debris_pieces.is_empty() {
            0
        } else {
            FRAME_PIECES
        };
        put_uv(buf, flags);

        let keys: Vec<u32> = frame.aircraft.iter().map(|a| a.id).collect();
        put_list(buf, &self.aircraft_keys, &keys);
        let mut next = HashMap::with_capacity(keys.len());
        for state in &frame.aircraft {
            let old = self.aircraft.remove(&state.id);
            next.insert(state.id, put_aircraft(buf, old, state));
        }
        self.aircraft = next;
        self.aircraft_keys = keys;

        let keys: Vec<u32> = frame.projectiles.iter().map(|p| p.id).collect();
        put_list(buf, &self.projectile_keys, &keys);
        let mut next = HashMap::with_capacity(keys.len());
        for state in &frame.projectiles {
            let old = self.projectiles.remove(&state.id);
            next.insert(state.id, put_projectile(buf, old, state));
        }
        self.projectiles = next;
        self.projectile_keys = keys;

        let keys: Vec<(u32, u32)> = frame.debris.iter().map(|d| (d.owner, d.index)).collect();
        put_list(buf, &self.debris_keys, &keys);
        let mut next = HashMap::with_capacity(keys.len());
        for (key, state) in keys.iter().zip(&frame.debris) {
            let old = self.debris.remove(key);
            next.insert(*key, put_debris(buf, old, state));
        }
        self.debris = next;
        self.debris_keys = keys;

        let owners: Vec<u32> = frame.escapees.iter().map(|e| e.owner).collect();
        put_list(buf, &self.escapee_owners, &owners);
        let keys = escapee_keys(&owners);
        let mut next = HashMap::with_capacity(keys.len());
        for (key, state) in keys.iter().zip(&frame.escapees) {
            let old = self.escapees.remove(key);
            next.insert(*key, put_escapee(buf, old, state));
        }
        self.escapees = next;
        self.escapee_owners = owners;

        if !frame.surface_hp.is_empty() {
            put_uv(buf, frame.surface_hp.len() as u64);
            let mut last = 0u32;
            for (id, hp) in &frame.surface_hp {
                put_iv(buf, i64::from(*id) - i64::from(last));
                put_iv(buf, i64::from(*hp));
                last = *id;
            }
        }

        if flags & FRAME_UNITS != 0 {
            let keys: Vec<u32> = frame.surface.iter().map(|u| u.id).collect();
            put_list(buf, &self.surface_keys, &keys);
            // Only the units whose record says something are written, by
            // their place in the list; the others move on as predicted.
            let mut next = HashMap::with_capacity(keys.len());
            let mut changed: Vec<(usize, Vec<u8>)> = Vec::new();
            for (place, state) in frame.surface.iter().enumerate() {
                let old = self.surface.remove(&state.id);
                let mut record = Vec::new();
                next.insert(state.id, put_surface(&mut record, old, state));
                if record != [0] {
                    changed.push((place, record));
                }
            }
            put_uv(buf, changed.len() as u64);
            let mut last = 0usize;
            for (place, record) in &changed {
                put_uv(buf, (place - last) as u64);
                buf.extend_from_slice(record);
                last = *place;
            }
            self.surface = next;
            self.surface_keys = keys;
        }

        if flags & FRAME_STOCK != 0 {
            put_uv(buf, frame.surface_stock.len() as u64);
            let mut last = 0u32;
            for stock in &frame.surface_stock {
                put_iv(buf, i64::from(stock.unit) - i64::from(last));
                put_uv(buf, u64::from(stock.mount));
                put_uv(buf, u64::from(stock.loaded));
                put_uv(buf, stock.reserve.map_or(0, |r| u64::from(r) + 1));
                last = stock.unit;
            }
        }

        if flags & FRAME_PIECES != 0 {
            put_uv(buf, frame.debris_pieces.len() as u64);
            let mut last = 0u32;
            for (owner, index, piece) in &frame.debris_pieces {
                put_iv(buf, i64::from(*owner) - i64::from(last));
                put_uv(buf, u64::from(*index));
                buf.push(*piece);
                last = *owner;
            }
        }
    }

    /// Reads one frame's states into `frame`.
    pub fn get(&mut self, input: &mut In, frame: &mut Frame, version: u16) -> Result<()> {
        let flags = input.uv()?;
        if flags & !FRAME_KNOWN != 0 || (version < 3 && flags & !FRAME_SURFACE != 0) {
            return Err(corrupt("a frame has unknown flags"));
        }

        let keys = get_list(input, &self.aircraft_keys, MAX_AIRCRAFT, "aircraft", true)?;
        let mut next = HashMap::with_capacity(keys.len());
        for id in &keys {
            let pred = get_aircraft(input, self.aircraft.remove(id), version)?;
            frame.aircraft.push(pred.state(*id));
            next.insert(*id, pred);
        }
        self.aircraft = next;
        self.aircraft_keys = keys;

        let keys = get_list(
            input,
            &self.projectile_keys,
            MAX_PROJECTILES,
            "projectiles",
            true,
        )?;
        let mut next = HashMap::with_capacity(keys.len());
        for id in &keys {
            let pred = get_projectile(input, self.projectiles.remove(id))?;
            frame.projectiles.push(pred.state(*id));
            next.insert(*id, pred);
        }
        self.projectiles = next;
        self.projectile_keys = keys;

        let keys = get_list(input, &self.debris_keys, MAX_DEBRIS, "debris pieces", true)?;
        let mut next = HashMap::with_capacity(keys.len());
        for key in &keys {
            let pred = get_debris(input, self.debris.remove(key))?;
            frame.debris.push(pred.state(key.0, key.1));
            next.insert(*key, pred);
        }
        self.debris = next;
        self.debris_keys = keys;

        let owners = get_list(
            input,
            &self.escapee_owners,
            MAX_ESCAPEES,
            "ejected pilots",
            false,
        )?;
        let keys = escapee_keys(&owners);
        let mut next = HashMap::with_capacity(keys.len());
        for key in &keys {
            let pred = get_escapee(input, self.escapees.remove(key))?;
            frame.escapees.push(pred.state(key.0));
            next.insert(*key, pred);
        }
        self.escapees = next;
        self.escapee_owners = owners;

        if flags & FRAME_SURFACE != 0 {
            let n = input.count(MAX_SURFACE_CHANGES_PER_TICK, "surface changes")?;
            let mut last = 0u32;
            for _ in 0..n {
                let id = u32::get(input, Some(last))?;
                let hp = i32::try_from(input.iv()?)
                    .map_err(|_| corrupt("a surface hit point value is out of range"))?;
                frame.surface_hp.push((id, hp));
                last = id;
            }
        }

        if flags & FRAME_UNITS != 0 {
            let keys = get_list(
                input,
                &self.surface_keys,
                MAX_SURFACE_UNITS,
                "surface units",
                true,
            )?;
            let changed = input.count(keys.len(), "changed surface units")?;
            let mut records: HashMap<usize, SurfacePred> = HashMap::with_capacity(changed);
            let mut place = 0usize;
            for n in 0..changed {
                let step = input.count(keys.len(), "surface unit positions")?;
                place = if n == 0 {
                    step
                } else {
                    place
                        .checked_add(step)
                        .ok_or_else(|| corrupt("a surface unit place overflows"))?
                };
                let id = *keys
                    .get(place)
                    .ok_or_else(|| corrupt("a surface unit place is out of range"))?;
                if n > 0 && step == 0 {
                    return Err(corrupt("a surface unit is listed twice"));
                }
                records.insert(place, get_surface(input, self.surface.remove(&id))?);
            }
            let mut next = HashMap::with_capacity(keys.len());
            for (place, id) in keys.iter().enumerate() {
                let pred = match records.remove(&place) {
                    Some(pred) => pred,
                    // Unchanged: the prediction holds.
                    None => get_surface(&mut In::new(&[0]), self.surface.remove(id))?,
                };
                frame.surface.push(pred.state(*id));
                next.insert(*id, pred);
            }
            self.surface = next;
            self.surface_keys = keys;
        }

        if flags & FRAME_STOCK != 0 {
            let n = input.count(MAX_SURFACE_STOCK_PER_TICK, "surface stock changes")?;
            let mut last = 0u32;
            for _ in 0..n {
                let unit = u32::get(input, Some(last))?;
                let mount = u16::try_from(input.uv()?)
                    .map_err(|_| corrupt("a hardpoint number is out of range"))?;
                let loaded = input.u32v()?;
                let reserve = match input.uv()? {
                    0 => None,
                    n => Some(
                        u32::try_from(n - 1)
                            .map_err(|_| corrupt("a magazine reserve is out of range"))?,
                    ),
                };
                frame.surface_stock.push(SurfaceStock {
                    unit,
                    mount,
                    loaded,
                    reserve,
                });
                last = unit;
            }
        }

        if flags & FRAME_PIECES != 0 {
            let n = input.count(MAX_DEBRIS, "debris pieces")?;
            let mut last = 0u32;
            for _ in 0..n {
                let owner = u32::get(input, Some(last))?;
                let index = input.u32v()?;
                let piece = input.u8()?;
                frame.debris_pieces.push((owner, index, piece));
                last = owner;
            }
        }
        Ok(())
    }
}
