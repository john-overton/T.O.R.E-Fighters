//! Values, events, and the events and checksums sections.

use crate::codec::{
    In, from_decimal, put_iv, put_opt_id, put_u64, put_uv, put_xf64, shortest_decimal, uv_len,
    zigzag,
};
use crate::error::{Result, corrupt};
use crate::limits::{MAX_EVENTS_PER_TICK, MAX_FIELDS_PER_EVENT, MAX_IDS_PER_VALUE};
use crate::model::{Event, Value};
use crate::spawns::{get_gap, put_gap};
use crate::strings::{Interner, StringTable};

const V_NONE: u8 = 0;
const V_FALSE: u8 = 1;
const V_TRUE: u8 = 2;
const V_INT: u8 = 3;
const V_NUM: u8 = 4;
const V_TEXT: u8 = 5;
const V_ID: u8 = 6;
const V_IDS: u8 = 7;
/// Tree nodes only: an integer as the change from the previous sample's.
const V_INT_DELTA: u8 = 8;
/// Tree nodes only: a number as a decimal change from the previous sample's.
const V_NUM_DELTA: u8 = 9;

/// Values equal bit for bit, so negative zero and NaN payloads survive the
/// "unchanged" shortcut.
pub(crate) fn value_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.to_bits() == y.to_bits(),
        _ => a == b,
    }
}

/// The decimal change from `reference` to `v`: `(exponent shift, mantissa
/// change)`. Both are written at the finer of their two decimal exponents.
fn decimal_delta(v: f64, reference: f64) -> Option<(i64, i64)> {
    let (mv, ev) = shortest_decimal(v)?;
    let (mr, er) = shortest_decimal(reference)?;
    let common = ev.min(er);
    let scale = |m: i64, e: i32| -> Option<i64> {
        m.checked_mul(10i64.checked_pow(u32::try_from(e - common).ok()?)?)
    };
    let delta = scale(mv, ev)?.checked_sub(scale(mr, er)?)?;
    let shift = i64::from(common - er);
    let back = scale(mr, er)?.checked_add(delta)?;
    (from_decimal(back, common)?.to_bits() == v.to_bits()).then_some((shift, delta))
}

fn apply_decimal_delta(reference: f64, shift: i64, delta: i64) -> Option<f64> {
    let (mr, er) = shortest_decimal(reference)?;
    let common = i64::from(er) + shift;
    if !(-40..=0).contains(&shift) {
        return None;
    }
    let scaled = mr.checked_mul(10i64.checked_pow(u32::try_from(-shift).ok()?)?)?;
    from_decimal(scaled.checked_add(delta)?, i32::try_from(common).ok()?)
}

fn xf64_len(v: f64) -> usize {
    let mut buf = Vec::with_capacity(9);
    put_xf64(&mut buf, v);
    buf.len()
}

pub(crate) fn put_value(
    buf: &mut Vec<u8>,
    strings: &mut Interner,
    value: &Value,
    reference: Option<&Value>,
) {
    match value {
        Value::None => buf.push(V_NONE),
        Value::Bool(false) => buf.push(V_FALSE),
        Value::Bool(true) => buf.push(V_TRUE),
        Value::Int(v) => {
            if let Some(Value::Int(r)) = reference
                && let Some(delta) = v.checked_sub(*r)
                && uv_len(zigzag(delta)) < uv_len(zigzag(*v))
            {
                buf.push(V_INT_DELTA);
                put_iv(buf, delta);
                return;
            }
            buf.push(V_INT);
            put_iv(buf, *v);
        }
        Value::Num(v) => {
            if let Some(Value::Num(r)) = reference
                && let Some((shift, delta)) = decimal_delta(*v, *r)
                && uv_len(zigzag(shift)) + uv_len(zigzag(delta)) < xf64_len(*v)
            {
                buf.push(V_NUM_DELTA);
                put_iv(buf, shift);
                put_iv(buf, delta);
                return;
            }
            buf.push(V_NUM);
            put_xf64(buf, *v);
        }
        Value::Text(text) => {
            buf.push(V_TEXT);
            strings.put(buf, text);
        }
        Value::Id(id) => {
            buf.push(V_ID);
            put_uv(buf, u64::from(*id));
        }
        Value::Ids(ids) => {
            buf.push(V_IDS);
            put_uv(buf, ids.len() as u64);
            let mut last = 0i64;
            for id in ids {
                put_iv(buf, i64::from(*id) - last);
                last = i64::from(*id);
            }
        }
    }
}

pub(crate) fn get_value(
    input: &mut In,
    strings: &StringTable,
    reference: Option<&Value>,
) -> Result<Value> {
    Ok(match input.u8()? {
        V_NONE => Value::None,
        V_FALSE => Value::Bool(false),
        V_TRUE => Value::Bool(true),
        V_INT => Value::Int(input.iv()?),
        V_NUM => Value::Num(input.xf64()?),
        V_TEXT => Value::Text(strings.read(input)?),
        V_ID => Value::Id(input.u32v()?),
        V_IDS => {
            let n = input.count(MAX_IDS_PER_VALUE, "ids in one value")?;
            let mut ids = Vec::with_capacity(n);
            let mut last = 0i64;
            for _ in 0..n {
                last = last.wrapping_add(input.iv()?);
                ids.push(u32::try_from(last).map_err(|_| corrupt("an id is out of range"))?);
            }
            Value::Ids(ids)
        }
        V_INT_DELTA => match reference {
            Some(Value::Int(r)) => Value::Int(r.wrapping_add(input.iv()?)),
            _ => return Err(corrupt("an integer change has no earlier integer")),
        },
        V_NUM_DELTA => match reference {
            Some(Value::Num(r)) => {
                let shift = input.iv()?;
                let delta = input.iv()?;
                Value::Num(
                    apply_decimal_delta(*r, shift, delta)
                        .ok_or_else(|| corrupt("a number change does not apply"))?,
                )
            }
            _ => return Err(corrupt("a number change has no earlier number")),
        },
        _ => return Err(corrupt("unknown value type")),
    })
}

pub(crate) fn put_event(buf: &mut Vec<u8>, strings: &mut Interner, event: &Event) {
    strings.put(buf, &event.kind);
    put_opt_id(buf, event.subject);
    put_opt_id(buf, event.object);
    put_uv(buf, event.fields.len() as u64);
    for (name, value) in &event.fields {
        strings.put(buf, name);
        put_value(buf, strings, value, None);
    }
    strings.put(buf, &event.text);
}

pub(crate) fn get_event(input: &mut In, strings: &StringTable) -> Result<Event> {
    let kind = strings.read(input)?;
    let subject = input.opt_id()?;
    let object = input.opt_id()?;
    let n = input.count(MAX_FIELDS_PER_EVENT, "fields on one event")?;
    let mut fields = Vec::with_capacity(n);
    for _ in 0..n {
        let name = strings.read(input)?;
        let value = get_value(input, strings, None)?;
        fields.push((name, value));
    }
    let text = strings.read(input)?;
    Ok(Event {
        kind,
        subject,
        object,
        fields,
        text,
    })
}

/// Writer state for one chunk's events section.
#[derive(Default)]
pub(crate) struct EventCoder {
    entries: u64,
    last_frame: Option<u32>,
    body: Vec<u8>,
}

impl EventCoder {
    pub fn put(&mut self, frame: u32, events: &[Event], strings: &mut Interner) {
        if events.is_empty() {
            return;
        }
        self.entries += 1;
        put_gap(&mut self.body, frame, &mut self.last_frame);
        put_uv(&mut self.body, events.len() as u64);
        for event in events {
            put_event(&mut self.body, strings, event);
        }
    }

    pub fn section(&self) -> Option<Vec<u8>> {
        (self.entries > 0).then(|| {
            let mut out = Vec::with_capacity(self.body.len() + 10);
            put_uv(&mut out, self.entries);
            out.extend_from_slice(&self.body);
            out
        })
    }

    pub fn len(&self) -> usize {
        self.body.len()
    }
}

/// Events of each frame that has any: `(frame index, events)`.
pub(crate) fn get_events(
    section: &[u8],
    frames: u32,
    strings: &StringTable,
) -> Result<Vec<(u32, Vec<Event>)>> {
    let mut input = In::new(section);
    let entries = input.count(frames as usize, "event entries")?;
    let mut out = Vec::with_capacity(entries);
    let mut last = None;
    for _ in 0..entries {
        let frame = get_gap(&mut input, &mut last, frames)?;
        let n = input.count(MAX_EVENTS_PER_TICK, "events in one frame")?;
        let mut events = Vec::with_capacity(n);
        for _ in 0..n {
            events.push(get_event(&mut input, strings)?);
        }
        out.push((frame, events));
    }
    if !input.done() {
        return Err(corrupt("the events section has trailing bytes"));
    }
    Ok(out)
}

/// Writer state for one chunk's checksums section.
#[derive(Default)]
pub(crate) struct ChecksumCoder {
    entries: u64,
    last_frame: Option<u32>,
    body: Vec<u8>,
}

impl ChecksumCoder {
    pub fn put(&mut self, frame: u32, checksum: Option<u64>) {
        if let Some(checksum) = checksum {
            self.entries += 1;
            put_gap(&mut self.body, frame, &mut self.last_frame);
            put_u64(&mut self.body, checksum);
        }
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

pub(crate) fn get_checksums(section: &[u8], frames: u32) -> Result<Vec<(u32, u64)>> {
    let mut input = In::new(section);
    let entries = input.count(frames as usize, "checksums")?;
    let mut out = Vec::with_capacity(entries);
    let mut last = None;
    for _ in 0..entries {
        let frame = get_gap(&mut input, &mut last, frames)?;
        out.push((frame, input.u64()?));
    }
    if !input.done() {
        return Err(corrupt("the checksums section has trailing bytes"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_changes_are_exact_and_short() {
        for (reference, v) in [
            (6.1, 6.0),
            (6.0, 6.1),
            (480.25, 480.5),
            (0.1, 0.30000000000000004),
            (1e-7, 3.5),
            (1234.5678, -2.0),
            (0.0, 12.75),
        ] {
            let mut strings = Interner::default();
            let mut buf = vec![];
            put_value(
                &mut buf,
                &mut strings,
                &Value::Num(v),
                Some(&Value::Num(reference)),
            );
            let table = StringTable::default();
            let back = get_value(&mut In::new(&buf), &table, Some(&Value::Num(reference))).unwrap();
            assert!(value_eq(&back, &Value::Num(v)), "{reference} -> {v}");
        }
        let mut buf = vec![];
        put_value(
            &mut buf,
            &mut Interner::default(),
            &Value::Num(6.2),
            Some(&Value::Num(6.1)),
        );
        assert_eq!(buf.len(), 3);
    }

    #[test]
    fn every_value_type_round_trips() {
        let values = [
            Value::None,
            Value::Bool(false),
            Value::Bool(true),
            Value::Int(-5),
            Value::Int(i64::MIN),
            Value::Num(-0.),
            Value::Num(f64::NAN),
            Value::Text("hello".into()),
            Value::Id(u32::MAX),
            Value::Ids(vec![3, 1, u32::MAX, 0]),
        ];
        let mut strings = Interner::default();
        let mut buf = vec![];
        for v in &values {
            put_value(&mut buf, &mut strings, v, None);
        }
        let mut table = StringTable::default();
        table.define(&strings.take_section().unwrap()).unwrap();
        let mut input = In::new(&buf);
        for v in &values {
            assert!(value_eq(&get_value(&mut input, &table, None).unwrap(), v));
        }
        assert!(input.done());
    }
}
