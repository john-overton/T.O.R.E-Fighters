//! Byte-level building blocks: little-endian integers, LEB128 varints,
//! zigzag signed varints, an exact float code, packed small residuals and the
//! FNV-1a 64 checksum. The reader side never panics on hostile bytes.

use crate::error::{Result, corrupt};

pub(crate) const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64, continuing from `hash` (start with [`FNV_OFFSET`]).
pub(crate) fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

pub(crate) fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

pub(crate) fn unzigzag(u: u64) -> i64 {
    ((u >> 1) as i64) ^ -((u & 1) as i64)
}

pub(crate) fn put_uv(buf: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        buf.push((v as u8) | 0x80);
        v >>= 7;
    }
    buf.push(v as u8);
}

pub(crate) fn put_iv(buf: &mut Vec<u8>, v: i64) {
    put_uv(buf, zigzag(v));
}

pub(crate) fn uv_len(mut v: u64) -> usize {
    let mut n = 1;
    while v >= 0x80 {
        v >>= 7;
        n += 1;
    }
    n
}

pub(crate) fn put_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub(crate) fn put_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub(crate) fn put_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Length-prefixed UTF-8 text.
pub(crate) fn put_text(buf: &mut Vec<u8>, text: &str) {
    put_uv(buf, text.len() as u64);
    buf.extend_from_slice(text.as_bytes());
}

/// The shortest decimal `m * 10^e` that reads back as exactly `v`, when the
/// mantissa fits in an i64. Negative zero and non-finite values have none.
pub(crate) fn shortest_decimal(v: f64) -> Option<(i64, i32)> {
    if !v.is_finite() || (v == 0. && v.is_sign_negative()) {
        return None;
    }
    if v == 0. {
        return Some((0, 0));
    }
    // LowerExp prints the shortest digits that round-trip, e.g. "-1.25e-3".
    let text = format!("{v:e}");
    let (mantissa, exponent) = text.split_once('e')?;
    let exponent: i32 = exponent.parse().ok()?;
    let negative = mantissa.starts_with('-');
    let digits = mantissa.trim_start_matches('-');
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let mut m: i64 = 0;
    for c in whole.chars().chain(fraction.chars()) {
        m = m.checked_mul(10)?.checked_add(i64::from(c.to_digit(10)?))?;
    }
    let e = exponent - fraction.len() as i32;
    Some((if negative { -m } else { m }, e))
}

/// `m * 10^e`, correctly rounded, so any decimal equal to a value's shortest
/// form reads back as that exact value.
pub(crate) fn from_decimal(m: i64, e: i32) -> Option<f64> {
    format!("{m}e{e}").parse().ok()
}

/// Exponents a decimal float may carry. f64 needs about -343 to 308.
const DECIMAL_EXPONENT_LIMIT: i64 = 400;

/// Exact float: a short decimal when that is smaller than the raw bits,
/// otherwise the raw IEEE bits (which keeps NaN payloads and negative zero).
pub(crate) fn put_xf64(buf: &mut Vec<u8>, v: f64) {
    if let Some((m, e)) = shortest_decimal(v) {
        let head = zigzag(i64::from(e)) << 1;
        if uv_len(head) + uv_len(zigzag(m)) < 9
            && from_decimal(m, e).map(f64::to_bits) == Some(v.to_bits())
        {
            put_uv(buf, head);
            put_iv(buf, m);
            return;
        }
    }
    buf.push(1);
    put_u64(buf, v.to_bits());
}

/// Packs three small residuals: one byte when all are within -2..=2, three
/// bytes when all are within -8..=7, otherwise a marker and three varints.
pub(crate) fn put_triple(buf: &mut Vec<u8>, r: [i64; 3]) {
    if r.iter().all(|v| (-2..=2).contains(v)) {
        buf.push(((r[0] + 2) * 25 + (r[1] + 2) * 5 + (r[2] + 2)) as u8);
    } else if r.iter().all(|v| (-8..=7).contains(v)) {
        let bits = (r[0] + 8) as u16 | ((r[1] + 8) as u16) << 4 | ((r[2] + 8) as u16) << 8;
        buf.push(126);
        put_u16(buf, bits);
    } else {
        buf.push(125);
        for v in r {
            put_iv(buf, v);
        }
    }
}

/// Packs two small residuals: one byte when both are within -5..=5.
pub(crate) fn put_pair(buf: &mut Vec<u8>, r: [i64; 2]) {
    if r.iter().all(|v| (-5..=5).contains(v)) {
        buf.push(((r[0] + 5) * 11 + (r[1] + 5)) as u8);
    } else {
        buf.push(121);
        for v in r {
            put_iv(buf, v);
        }
    }
}

/// Size of `put_triple`'s output, for choosing between predictions.
pub(crate) fn triple_len(r: [i64; 3]) -> usize {
    if r.iter().all(|v| (-2..=2).contains(v)) {
        1
    } else if r.iter().all(|v| (-8..=7).contains(v)) {
        3
    } else {
        1 + r.iter().map(|v| uv_len(zigzag(*v))).sum::<usize>()
    }
}

/// A bounded cursor over untrusted bytes. Every read checks its length.
pub(crate) struct In<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> In<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn done(&self) -> bool {
        self.pos >= self.data.len()
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.remaining() {
            return Err(corrupt("a record runs past the end of its section"));
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Result<u32> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn u64(&mut self) -> Result<u64> {
        let b = self.bytes(8)?;
        let mut a = [0; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }

    pub fn uv(&mut self) -> Result<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.u8()?;
            if shift == 63 && byte > 1 {
                return Err(corrupt("a number is too large"));
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(corrupt("a number is too long"))
    }

    pub fn iv(&mut self) -> Result<i64> {
        Ok(unzigzag(self.uv()?))
    }

    pub fn u32v(&mut self) -> Result<u32> {
        u32::try_from(self.uv()?).map_err(|_| corrupt("an id is out of range"))
    }

    /// A count that must not exceed `max`.
    pub fn count(&mut self, max: usize, what: &str) -> Result<usize> {
        let n = self.uv()?;
        if n > max as u64 {
            return Err(corrupt(format!("{n} {what}, more than the limit of {max}")));
        }
        Ok(n as usize)
    }

    /// Optional id stored as 0 for none, otherwise id + 1.
    pub fn opt_id(&mut self) -> Result<Option<u32>> {
        match self.uv()? {
            0 => Ok(None),
            n => u32::try_from(n - 1)
                .map(Some)
                .map_err(|_| corrupt("an id is out of range")),
        }
    }

    pub fn text(&mut self, max: usize) -> Result<String> {
        let n = self.count(max, "bytes of text")?;
        let bytes = self.bytes(n)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| corrupt("text is not UTF-8"))
    }

    pub fn xf64(&mut self) -> Result<f64> {
        let head = self.uv()?;
        if head == 1 {
            return Ok(f64::from_bits(self.u64()?));
        }
        if head & 1 != 0 {
            return Err(corrupt("unknown number encoding"));
        }
        let e = unzigzag(head >> 1);
        if e.abs() > DECIMAL_EXPONENT_LIMIT {
            return Err(corrupt("a decimal exponent is out of range"));
        }
        let m = self.iv()?;
        from_decimal(m, e as i32).ok_or_else(|| corrupt("a decimal number does not parse"))
    }

    pub fn triple(&mut self) -> Result<[i64; 3]> {
        let tag = self.u8()?;
        match tag {
            0..=124 => {
                let t = i64::from(tag);
                Ok([t / 25 - 2, t / 5 % 5 - 2, t % 5 - 2])
            }
            125 => Ok([self.iv()?, self.iv()?, self.iv()?]),
            126 => {
                let bits = self.u16()?;
                if bits >= 1 << 12 {
                    return Err(corrupt("a packed residual has stray bits"));
                }
                let nibble = |shift: u16| i64::from((bits >> shift) & 15) - 8;
                Ok([nibble(0), nibble(4), nibble(8)])
            }
            _ => Err(corrupt("unknown residual encoding")),
        }
    }

    pub fn pair(&mut self) -> Result<[i64; 2]> {
        let tag = self.u8()?;
        match tag {
            0..=120 => {
                let t = i64::from(tag);
                Ok([t / 11 - 5, t % 11 - 5])
            }
            121 => Ok([self.iv()?, self.iv()?]),
            _ => Err(corrupt("unknown residual encoding")),
        }
    }
}

pub(crate) fn put_opt_id(buf: &mut Vec<u8>, id: Option<u32>) {
    put_uv(buf, id.map_or(0, |id| u64::from(id) + 1));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_and_zigzag_round_trip_extremes() {
        for v in [
            0,
            1,
            -1,
            63,
            -64,
            64,
            i64::MAX,
            i64::MIN,
            1 << 40,
            -(1 << 40),
        ] {
            let mut buf = vec![];
            put_iv(&mut buf, v);
            assert_eq!(In::new(&buf).iv().unwrap(), v);
        }
        for v in [0, 127, 128, u64::MAX] {
            let mut buf = vec![];
            put_uv(&mut buf, v);
            assert_eq!(buf.len(), uv_len(v));
            assert_eq!(In::new(&buf).uv().unwrap(), v);
        }
        // An eleventh continuation byte and a too-large final byte are rejected.
        assert!(In::new(&[0xff; 11]).uv().is_err());
        assert!(
            In::new(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02])
                .uv()
                .is_err()
        );
    }

    #[test]
    fn exact_floats_keep_every_bit() {
        let values = [
            0.,
            -0.,
            1.,
            -1.,
            0.1,
            1234.5,
            12345.678901234567,
            f64::MIN_POSITIVE,
            5e-324,
            f64::MAX,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::from_bits(0x7ff8_0000_dead_beef),
            std::f64::consts::PI,
        ];
        for v in values {
            let mut buf = vec![];
            put_xf64(&mut buf, v);
            assert!(buf.len() <= 9);
            let back = In::new(&buf).xf64().unwrap();
            assert_eq!(back.to_bits(), v.to_bits(), "{v:e}");
        }
        let mut buf = vec![];
        put_xf64(&mut buf, 0.5);
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn packed_residuals_round_trip_at_every_boundary() {
        for a in -10..=10 {
            for b in [-300, -9, -8, -2, 0, 2, 7, 8, 300] {
                for c in [-3, -2, 0, 2, 3] {
                    let mut buf = vec![];
                    put_triple(&mut buf, [a, b, c]);
                    assert_eq!(buf.len(), triple_len([a, b, c]));
                    assert_eq!(In::new(&buf).triple().unwrap(), [a, b, c]);
                }
                let mut buf = vec![];
                put_pair(&mut buf, [a, b]);
                assert_eq!(In::new(&buf).pair().unwrap(), [a, b]);
            }
        }
        assert!(In::new(&[127]).triple().is_err());
        assert!(In::new(&[126, 0, 0x10]).triple().is_err());
        assert!(In::new(&[122]).pair().is_err());
    }
}
