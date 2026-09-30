//! Small codings the game's sections share, on top of `tore-codec`.

use super::{WireError, WireResult};
use tore_codec::{BitReader, BitWriter, CodecError};

/// The largest whole number of steps a quantized quantity may have either side
/// of zero: 2^40 steps, 34 billion feet at 1/32 ft. A value beyond it (or
/// not finite) is clamped; see net-protocol.md, "Quantization".
pub const STEP_LIMIT: i64 = 1 << 40;

/// `value` as whole `step`s, rounded to the nearest (halves away from zero),
/// clamped to [`STEP_LIMIT`]. Not finite is zero.
pub fn steps(value: f64, step: f64) -> i64 {
    if !value.is_finite() {
        return 0;
    }
    let scaled = (value / step).round();
    if scaled >= STEP_LIMIT as f64 {
        STEP_LIMIT
    } else if scaled <= -(STEP_LIMIT as f64) {
        -STEP_LIMIT
    } else {
        scaled as i64
    }
}

/// `steps` of `step` as a value.
pub fn value(steps: i64, step: f64) -> f64 {
    steps as f64 * step
}

/// An angle in radians as 2^-16 of a turn. Not finite is zero.
pub fn turn16(radians: f64) -> u16 {
    tore_codec::quant::quantize_angle(radians / std::f64::consts::TAU, 16).unwrap_or(0) as u16
}

/// 2^-16 of a turn as radians, from -pi up to but not including pi.
pub fn radians16(angle: u16) -> f64 {
    f64::from(angle as i16) / 65_536. * std::f64::consts::TAU
}

/// The short-way difference `to - from` of two 16-bit angles.
pub fn angle_diff16(from: u16, to: u16) -> i64 {
    i64::from(to.wrapping_sub(from) as i16)
}

/// `a / b` rounded to the nearest whole number, halves up, for `b > 0`,
/// without overflow.
pub fn div_round(a: i128, b: i128) -> i128 {
    (2 * a + b).div_euclid(2 * b)
}

/// A value within [`STEP_LIMIT`], or an error naming `what`.
pub fn in_range(value: i128, what: &'static str) -> WireResult<i64> {
    if (-(STEP_LIMIT as i128)..=STEP_LIMIT as i128).contains(&value) {
        Ok(value as i64)
    } else {
        Err(WireError::Invalid(what))
    }
}

fn index_bits(len: usize) -> u32 {
    // Room for every bucket and the escape.
    usize::BITS - len.leading_zeros()
}

/// Writes an unsigned `value` in the first width of `ladder` that holds it
/// (widths ascending; 0 holds only zero), after the bucket's index; a value
/// beyond the last width takes the escape index and a varint.
pub fn write_uladder(w: &mut BitWriter, value: u64, ladder: &[u32]) {
    let bits = index_bits(ladder.len());
    match ladder
        .iter()
        .position(|&width| width == 64 || value >> width == 0)
    {
        Some(index) => {
            let _ = w.write_bits(index as u64, bits);
            let _ = w.write_bits(value, ladder[index]);
        }
        None => {
            let _ = w.write_bits(ladder.len() as u64, bits);
            w.write_varint(value);
        }
    }
}

/// Reads a value [`write_uladder`] wrote with the same `ladder`, rejecting
/// any form it would not have written.
pub fn read_uladder(r: &mut BitReader<'_>, ladder: &[u32]) -> WireResult<u64> {
    let index = r.read_bits(index_bits(ladder.len()))? as usize;
    let value = if index < ladder.len() {
        r.read_bits(ladder[index])?
    } else if index == ladder.len() {
        r.read_varint()?
    } else {
        return Err(CodecError::BadBucket.into());
    };
    let first = ladder
        .iter()
        .position(|&width| width == 64 || value >> width == 0)
        .unwrap_or(ladder.len());
    if first != index {
        return Err(CodecError::NonCanonical.into());
    }
    Ok(value)
}

/// Reads a varint that must fit 32 bits.
pub fn read_u32(r: &mut BitReader<'_>) -> WireResult<u32> {
    u32::try_from(r.read_varint()?).map_err(|_| CodecError::ValueOutOfRange.into())
}

/// Reads a signed varint that must fit 32 bits.
pub fn read_i32(r: &mut BitReader<'_>) -> WireResult<i32> {
    i32::try_from(r.read_varint_signed()?).map_err(|_| CodecError::ValueOutOfRange.into())
}

/// Writes a count as a varint.
pub fn write_count(w: &mut BitWriter, count: usize) {
    w.write_varint(count as u64);
}

/// Reads a count, refusing one over `limit` before anything is read on.
pub fn read_count(r: &mut BitReader<'_>, limit: usize, what: &'static str) -> WireResult<usize> {
    let count = r.read_varint()?;
    if count > limit as u64 {
        return Err(WireError::TooMany { what, limit });
    }
    Ok(count as usize)
}

/// Writes an optional value behind a presence bit.
pub fn write_option<T>(w: &mut BitWriter, value: Option<T>, write: impl FnOnce(&mut BitWriter, T)) {
    w.write_bool(value.is_some());
    if let Some(value) = value {
        write(w, value);
    }
}

/// Reads an optional value behind a presence bit.
pub fn read_option<T>(
    r: &mut BitReader<'_>,
    read: impl FnOnce(&mut BitReader<'_>) -> WireResult<T>,
) -> WireResult<Option<T>> {
    if r.read_bool()? {
        Ok(Some(read(r)?))
    } else {
        Ok(None)
    }
}

/// Writes a string of at most 255 bytes; a longer one is cut at a character
/// boundary (the writers' texts are the game's own and never that long).
pub fn write_str(w: &mut BitWriter, text: &str) {
    let mut end = text.len().min(tore_codec::text::MAX_STRING_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let _ = w.write_str(&text[..end]);
}

/// Reads a string of at most 255 bytes.
pub fn read_str(r: &mut BitReader<'_>) -> WireResult<String> {
    Ok(r.read_str()?)
}

/// Writes a byte string of any length up to a message's 64 KB: a varint
/// length, then the bytes.
pub fn write_long_bytes(w: &mut BitWriter, bytes: &[u8]) {
    w.write_varint(bytes.len() as u64);
    w.write_bytes(bytes);
}

/// Reads a byte string [`write_long_bytes`] wrote, of at most `limit` bytes;
/// the length is checked against the limit and the data left before
/// anything is allocated.
pub fn read_long_bytes(
    r: &mut BitReader<'_>,
    limit: usize,
    what: &'static str,
) -> WireResult<Vec<u8>> {
    let len = read_count(r, limit, what)?;
    Ok(r.read_bytes(len)?)
}

/// Writes a long UTF-8 text.
pub fn write_long_str(w: &mut BitWriter, text: &str) {
    write_long_bytes(w, text.as_bytes());
}

/// Reads a long UTF-8 text of at most `limit` bytes.
pub fn read_long_str(
    r: &mut BitReader<'_>,
    limit: usize,
    what: &'static str,
) -> WireResult<String> {
    String::from_utf8(read_long_bytes(r, limit, what)?).map_err(|_| CodecError::InvalidUtf8.into())
}

/// Appends every bit `source` holds to `destination`.
pub fn append(destination: &mut BitWriter, source: &BitWriter) {
    let bytes = source.as_bytes();
    let mut left = source.bit_len();
    for &byte in bytes {
        let take = left.min(8) as u32;
        let _ = destination.write_bits(u64::from(byte), take);
        left -= take as usize;
        if left == 0 {
            break;
        }
    }
}

/// The body's bytes: padded with zero bits to a whole byte.
pub fn finish(mut w: BitWriter) -> Vec<u8> {
    w.align();
    w.finish()
}

/// Checks that only the zero padding of the last byte follows.
pub fn end(r: &mut BitReader<'_>) -> WireResult<()> {
    r.align_strict().map_err(|_| WireError::Trailing)?;
    if r.bits_remaining() != 0 {
        return Err(WireError::Trailing);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ladders_round_trip_and_refuse_other_forms() {
        let ladder = [0, 4, 10];
        for value in [0, 1, 15, 16, 1023, 1024, u64::from(u32::MAX), u64::MAX] {
            let mut w = BitWriter::new();
            write_uladder(&mut w, value, &ladder);
            let bytes = w.finish();
            let mut r = BitReader::new(&bytes);
            assert_eq!(read_uladder(&mut r, &ladder).unwrap(), value);
        }
        // Zero in the four-bit bucket is not how the writer puts it.
        let mut w = BitWriter::new();
        w.write_bits(1, 2).unwrap();
        w.write_bits(0, 4).unwrap();
        let bytes = w.finish();
        assert!(read_uladder(&mut BitReader::new(&bytes), &ladder).is_err());
    }

    #[test]
    fn steps_clamp_and_round() {
        assert_eq!(steps(1.0, 1. / 32.), 32);
        assert_eq!(steps(-0.015_625, 1. / 32.), -1);
        assert_eq!(steps(f64::NAN, 1.), 0);
        assert_eq!(steps(f64::INFINITY, 1.), 0);
        assert_eq!(steps(1e300, 1.), STEP_LIMIT);
        assert_eq!(steps(-1e300, 1.), -STEP_LIMIT);
    }

    #[test]
    fn angles_wrap_the_short_way() {
        assert_eq!(angle_diff16(65_535, 1), 2);
        assert_eq!(angle_diff16(1, 65_535), -2);
        assert_eq!(turn16(std::f64::consts::PI), 32_768);
        assert!((radians16(16_384) - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(div_round(5, 2), 3);
        assert_eq!(div_round(-5, 2), -2);
        assert_eq!(div_round(-7, 2), -3);
    }

    #[test]
    fn appended_bits_keep_their_order() {
        let mut source = BitWriter::new();
        source.write_bits(0b1_0110_1011_0101, 13).unwrap();
        let mut w = BitWriter::new();
        w.write_bits(0b101, 3).unwrap();
        append(&mut w, &source);
        assert_eq!(w.bit_len(), 16);
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes);
        assert_eq!(r.read_bits(3).unwrap(), 0b101);
        assert_eq!(r.read_bits(13).unwrap(), 0b1_0110_1011_0101);
    }
}
