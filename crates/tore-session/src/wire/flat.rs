//! Numbers in a layout, and lists of them keyed by id, coded against a
//! baseline: what the cockpit readout's coding is built from.
//!
//! An entry is a row of whole numbers whose layout says how each is coded.
//! A position predicted from a velocity advances with it between the
//! baseline and now; an entry the prediction gets exactly right is not sent
//! at all. A list is coded as the ids removed and the entries that changed,
//! each in full or against the baseline's entry with its id, so that only
//! what changed costs bits and a list can be brought up to date over several
//! packets when it does not fit one (what is not sent stays as the baseline
//! predicts it, on both ends). Every record parses without its baseline.

use super::bits::{self, div_round, in_range, read_uladder, write_uladder};
use super::{WireError, WireResult};
use std::collections::BTreeMap;
use tore_codec::{BitReader, BitWriter, CodecError};

/// How one number of an entry is coded.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Kind {
    /// A position step predicted from the velocity at index `vel`: it moves
    /// by `vel × ticks / divisor` steps between the baseline and now. In full
    /// a signed varint; against a baseline the residual, bucketed.
    Pos { vel: usize, divisor: i64 },
    /// A value sent as its difference from the baseline's, bucketed.
    Diff,
    /// A value that changes now and then: absolute, behind a changed bit.
    Slow(Slow),
}

/// How a slow value is written.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Slow {
    /// Unsigned in this many bits, at most this value.
    Bits(u32, i64),
    /// A signed varint.
    Int,
}

/// An entry's layout.
pub(crate) type Schema = &'static [Kind];

const DIFF_LADDER: [u32; 6] = [2, 5, 8, 12, 16, 24];

fn moving(kind: Kind) -> bool {
    !matches!(kind, Kind::Slow(_))
}

/// `base` moved on by `ticks`: its predicted positions advance.
pub(crate) fn advance(schema: Schema, base: &[i64], ticks: u32) -> Vec<i64> {
    let mut out = base.to_vec();
    for (index, kind) in schema.iter().enumerate() {
        if let Kind::Pos { vel, divisor } = *kind {
            let moved = i128::from(base[index])
                + div_round(
                    i128::from(base[vel]) * i128::from(ticks),
                    i128::from(divisor),
                );
            // A baseline is always within range, so the advance is too unless
            // it ran for absurdly long; hold it at the end then.
            out[index] = moved.clamp(-(bits::STEP_LIMIT as i128), bits::STEP_LIMIT as i128) as i64;
        }
    }
    out
}

fn write_slow(w: &mut BitWriter, slow: Slow, value: i64) {
    match slow {
        Slow::Bits(width, _) => {
            let _ = w.write_bits(value as u64, width);
        }
        Slow::Int => w.write_varint_signed(value),
    }
}

fn read_slow(r: &mut BitReader<'_>, slow: Slow) -> WireResult<i64> {
    match slow {
        Slow::Bits(width, max) => {
            let value = r.read_bits(width)? as i64;
            if value > max {
                return Err(WireError::Invalid("readout field"));
            }
            Ok(value)
        }
        Slow::Int => in_range(i128::from(r.read_varint_signed()?), "readout field"),
    }
}

/// Writes every number of an entry in full.
pub(crate) fn write_full(w: &mut BitWriter, schema: Schema, values: &[i64]) {
    for (kind, value) in schema.iter().zip(values) {
        match *kind {
            Kind::Pos { .. } | Kind::Diff => w.write_varint_signed(*value),
            Kind::Slow(slow) => write_slow(w, slow, *value),
        }
    }
}

/// Reads an entry written in full.
pub(crate) fn read_full(r: &mut BitReader<'_>, schema: Schema) -> WireResult<Vec<i64>> {
    schema
        .iter()
        .map(|kind| match *kind {
            Kind::Pos { .. } | Kind::Diff => {
                in_range(i128::from(r.read_varint_signed()?), "readout value")
            }
            Kind::Slow(slow) => read_slow(r, slow),
        })
        .collect()
}

/// An entry against a baseline, as read: the moving fields' differences and
/// the slow fields that changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Delta {
    moved: Option<Vec<i64>>,
    slow: Option<Vec<Option<i64>>>,
}

/// Writes `now` against `base`, the baseline's entry already advanced.
pub(crate) fn write_delta(w: &mut BitWriter, schema: Schema, now: &[i64], base: &[i64]) {
    let diffs: Vec<i64> = schema
        .iter()
        .enumerate()
        .filter(|(_, kind)| moving(**kind))
        .map(|(i, _)| now[i] - base[i])
        .collect();
    let moved = diffs.iter().any(|d| *d != 0);
    w.write_bool(moved);
    if moved {
        for d in diffs {
            let _ = w.write_bucketed(d, &DIFF_LADDER);
        }
    }
    let slow: Vec<usize> = (0..schema.len()).filter(|i| !moving(schema[*i])).collect();
    let changed = slow.iter().any(|&i| now[i] != base[i]);
    w.write_bool(changed);
    if changed {
        for i in slow {
            w.write_bool(now[i] != base[i]);
            if now[i] != base[i]
                && let Kind::Slow(kind) = schema[i]
            {
                write_slow(w, kind, now[i]);
            }
        }
    }
}

/// Reads an entry against a baseline, without the baseline.
pub(crate) fn read_delta(r: &mut BitReader<'_>, schema: Schema) -> WireResult<Delta> {
    let moved = if r.read_bool()? {
        let diffs = schema
            .iter()
            .filter(|kind| moving(**kind))
            .map(|_| Ok(r.read_bucketed(&DIFF_LADDER)?))
            .collect::<WireResult<Vec<i64>>>()?;
        if diffs.iter().all(|d| *d == 0) {
            return Err(CodecError::NonCanonical.into());
        }
        Some(diffs)
    } else {
        None
    };
    let slow = if r.read_bool()? {
        let mut values = Vec::new();
        for kind in schema {
            if let Kind::Slow(slow) = *kind {
                values.push(if r.read_bool()? {
                    Some(read_slow(r, slow)?)
                } else {
                    None
                });
            }
        }
        if values.iter().all(Option::is_none) {
            return Err(CodecError::NonCanonical.into());
        }
        Some(values)
    } else {
        None
    };
    if moved.is_none() && slow.is_none() {
        return Err(CodecError::NonCanonical.into());
    }
    Ok(Delta { moved, slow })
}

/// The entry `delta` describes against `base`, the baseline's entry advanced.
pub(crate) fn apply(schema: Schema, delta: &Delta, base: &[i64]) -> WireResult<Vec<i64>> {
    let mut out = base.to_vec();
    let mut diffs = delta.moved.iter().flatten();
    let mut slows = delta.slow.iter().flatten();
    for (i, kind) in schema.iter().enumerate() {
        match *kind {
            Kind::Slow(_) => {
                if delta.slow.is_some()
                    && let Some(Some(value)) = slows.next()
                {
                    out[i] = *value;
                }
            }
            Kind::Pos { .. } | Kind::Diff => {
                if let Some(d) = diffs.next() {
                    out[i] = in_range(i128::from(base[i]) + i128::from(*d), "readout value")?;
                }
            }
        }
    }
    Ok(out)
}

/// Id differences in a list: zero, 4 bits, 10 bits or a varint.
const ID_LADDER: [u32; 3] = [0, 4, 10];

/// A list keyed by id.
pub(crate) type List = BTreeMap<u32, Vec<i64>>;

/// Every entry of `base` advanced by `ticks`.
pub(crate) fn advance_list(schema: Schema, base: &List, ticks: u32) -> List {
    base.iter()
        .map(|(id, values)| (*id, advance(schema, values, ticks)))
        .collect()
}

/// An entry of a list record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EntryRaw {
    Full(Vec<i64>),
    Delta(Delta),
}

/// A list record as read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ListRaw {
    pub removed: Vec<u32>,
    pub entries: Vec<(u32, EntryRaw)>,
}

fn write_ids(w: &mut BitWriter, ids: &[u32]) {
    bits::write_count(w, ids.len());
    let mut previous: Option<u32> = None;
    for id in ids {
        let step = previous.map_or(u64::from(*id), |p| u64::from(id - p - 1));
        write_uladder(w, step, &ID_LADDER);
        previous = Some(*id);
    }
}

fn read_id(r: &mut BitReader<'_>, previous: Option<u32>) -> WireResult<u32> {
    let step = read_uladder(r, &ID_LADDER)?;
    let id = match previous {
        None => step,
        Some(p) => u64::from(p) + step + 1,
    };
    u32::try_from(id).map_err(|_| WireError::Invalid("readout id"))
}

/// What a list costs and holds once written.
pub(crate) struct ListWrite {
    /// The record's bits, or `None` when nothing changed or nothing fit.
    pub bits: Option<BitWriter>,
    /// What the client holds afterwards: the baseline with the sent changes.
    pub after: List,
    /// Changes left out for room.
    pub waiting: usize,
}

/// Codes `now` against `base` (already advanced) within `budget` bits:
/// removals first, then new entries, then changed ones, the largest change
/// first. What does not fit stays as `base` has it, and is counted.
pub(crate) fn write_list(schema: Schema, base: &List, now: &List, budget: usize) -> ListWrite {
    let mut removals: Vec<u32> = base
        .keys()
        .filter(|id| !now.contains_key(id))
        .copied()
        .collect();
    let mut changes: Vec<(u32, bool, BitWriter)> = Vec::new();
    for (id, values) in now {
        match base.get(id) {
            Some(old) if old == values => {}
            Some(old) => {
                let mut w = BitWriter::new();
                w.write_bool(false);
                write_delta(&mut w, schema, values, old);
                changes.push((*id, false, w));
            }
            None => {
                let mut w = BitWriter::new();
                w.write_bool(true);
                write_full(&mut w, schema, values);
                changes.push((*id, true, w));
            }
        }
    }
    if removals.is_empty() && changes.is_empty() {
        return ListWrite {
            bits: None,
            after: base.clone(),
            waiting: 0,
        };
    }
    // New entries first, then the biggest changes.
    changes.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(b.2.bit_len().cmp(&a.2.bit_len()))
            .then(a.0.cmp(&b.0))
    });
    let id_bits = |id: u32| {
        let mut w = BitWriter::new();
        write_uladder(&mut w, u64::from(id), &ID_LADDER);
        w.bit_len()
    };
    // Two counts of at most three bytes each.
    let mut left = budget.saturating_sub(48);
    let mut kept_removals = Vec::new();
    let mut waiting = 0;
    for id in removals.drain(..) {
        let cost = id_bits(id);
        if cost <= left {
            left -= cost;
            kept_removals.push(id);
        } else {
            waiting += 1;
        }
    }
    let mut chosen: Vec<(u32, BitWriter)> = Vec::new();
    for (id, _, w) in changes {
        let cost = id_bits(id) + w.bit_len();
        if cost <= left {
            left -= cost;
            chosen.push((id, w));
        } else {
            waiting += 1;
        }
    }
    if kept_removals.is_empty() && chosen.is_empty() {
        return ListWrite {
            bits: None,
            after: base.clone(),
            waiting,
        };
    }
    chosen.sort_by_key(|(id, _)| *id);
    let mut after = base.clone();
    for id in &kept_removals {
        after.remove(id);
    }
    for (id, _) in &chosen {
        after.insert(*id, now[id].clone());
    }
    let mut w = BitWriter::new();
    write_ids(&mut w, &kept_removals);
    bits::write_count(&mut w, chosen.len());
    let mut previous: Option<u32> = None;
    for (id, body) in &chosen {
        let step = previous.map_or(u64::from(*id), |p| u64::from(id - p - 1));
        write_uladder(&mut w, step, &ID_LADDER);
        bits::append(&mut w, body);
        previous = Some(*id);
    }
    ListWrite {
        bits: Some(w),
        after,
        waiting,
    }
}

/// Reads a list record of at most `limit` entries.
pub(crate) fn read_list(
    r: &mut BitReader<'_>,
    schema: Schema,
    limit: usize,
) -> WireResult<ListRaw> {
    let mut raw = ListRaw::default();
    let removed = bits::read_count(r, limit, "readout removals")?;
    let mut previous = None;
    for _ in 0..removed {
        let id = read_id(r, previous)?;
        raw.removed.push(id);
        previous = Some(id);
    }
    let count = bits::read_count(r, limit, "readout entries")?;
    let mut previous = None;
    for _ in 0..count {
        let id = read_id(r, previous)?;
        previous = Some(id);
        let entry = if r.read_bool()? {
            EntryRaw::Full(read_full(r, schema)?)
        } else {
            EntryRaw::Delta(read_delta(r, schema)?)
        };
        raw.entries.push((id, entry));
    }
    Ok(raw)
}

/// The list a record describes against `base` (already advanced); refuses
/// a list longer than `limit`.
pub(crate) fn apply_list(
    schema: Schema,
    raw: &ListRaw,
    base: &List,
    limit: usize,
) -> WireResult<List> {
    let mut out = base.clone();
    for id in &raw.removed {
        out.remove(id);
    }
    for (id, entry) in &raw.entries {
        let values = match entry {
            EntryRaw::Full(values) => values.clone(),
            EntryRaw::Delta(delta) => {
                let old = base
                    .get(id)
                    .ok_or(WireError::Invalid("readout entry without a baseline"))?;
                apply(schema, delta, old)?
            }
        };
        out.insert(*id, values);
    }
    if out.len() > limit {
        return Err(WireError::TooMany {
            what: "readout entries",
            limit,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA: Schema = &[
        Kind::Slow(Slow::Bits(2, 2)),
        Kind::Pos {
            vel: 3,
            divisor: 480,
        },
        Kind::Diff,
        Kind::Diff,
        Kind::Slow(Slow::Int),
    ];

    #[test]
    fn lists_round_trip_and_fill_in_over_several_records() {
        let mut base = List::new();
        base.insert(3, vec![1, 1_000, 4_000, 480, -5]);
        base.insert(9, vec![0, 0, 10, 0, 0]);
        let advanced = advance_list(SCHEMA, &base, 4);
        assert_eq!(advanced[&3][1], 1_004);
        let mut now = advanced.clone();
        now.remove(&9);
        now.get_mut(&3).unwrap()[2] = 4_098;
        for id in 10..40 {
            now.insert(id, vec![2, i64::from(id) * 7, 1, -3, 1]);
        }
        let mut have = advanced.clone();
        let mut rounds = 0;
        loop {
            let written = write_list(SCHEMA, &have, &now, 200);
            let Some(w) = written.bits else { break };
            let bytes = w.finish();
            let raw = read_list(&mut BitReader::new(&bytes), SCHEMA, 64).unwrap();
            have = apply_list(SCHEMA, &raw, &have, 64).unwrap();
            assert_eq!(have, written.after);
            rounds += 1;
            assert!(rounds < 20);
        }
        assert!(rounds > 1, "200 bits cannot hold thirty new entries");
        assert_eq!(have, now);
    }
}
