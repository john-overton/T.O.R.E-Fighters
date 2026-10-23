//! Quantizers: floats as whole numbers of steps.
//!
//! These functions only convert; writing the resulting integer in a chosen
//! width is up to the caller (see [`crate::BitWriter::write_signed`] and
//! [`crate::BitWriter::write_bits`]). The table of steps the protocol uses
//! is in the "Quantization" section of
//! [`docs/formats/net-protocol.md`](../../../docs/formats/net-protocol.md).
//!
//! Rounding is to the nearest step, with halves away from zero. A value that
//! cannot be quantized (not finite, or beyond the range the caller allows)
//! gives `None`, which the protocol answers by sending the exact 64-bit
//! value instead.

use std::ops::RangeInclusive;

/// Steps for a 0 to 1 value in one byte.
pub const UNIT_U8_STEPS: u32 = 255;
/// Steps for a 0 to 1 value in two bytes.
pub const UNIT_U16_STEPS: u32 = 65_535;
/// Steps each side of zero for a -1 to 1 value in one byte.
pub const SIGNED_UNIT_I8_STEPS: u32 = 127;
/// Steps each side of zero for a -1 to 1 value in two bytes.
pub const SIGNED_UNIT_I16_STEPS: u32 = 32_767;

/// The most bits an angle may have: a float's 52 stored mantissa bits.
pub const MAX_ANGLE_BITS: u32 = 52;

const TWO_POW_63: f64 = 9_223_372_036_854_775_808.0;

/// Rounds `value / step` to the nearest whole number of steps. `None` when
/// `value` or `step` is not finite, `step` is not above zero, or the result
/// falls outside `range`.
pub fn quantize(value: f64, step: f64, range: RangeInclusive<i64>) -> Option<i64> {
    if !value.is_finite() || !step.is_finite() || step <= 0.0 {
        return None;
    }
    let scaled = (value / step).round();
    if !scaled.is_finite() || !(-TWO_POW_63..TWO_POW_63).contains(&scaled) {
        return None;
    }
    let q = scaled as i64;
    range.contains(&q).then_some(q)
}

/// The value `quantized` steps of size `step` stand for.
pub fn dequantize(quantized: i64, step: f64) -> f64 {
    quantized as f64 * step
}

fn angle_mask(bits: u32) -> Option<u64> {
    (1..=MAX_ANGLE_BITS)
        .contains(&bits)
        .then(|| (1u64 << bits) - 1)
}

/// An angle as a fraction of a turn, in `bits` bits (1 to 52): `turns` is
/// any finite number, wrapped into one turn, so 1.25 and -0.75 both give a
/// quarter turn. Result is 0 to `2^bits - 1`. `None` for a value that is not
/// finite or a bad width.
pub fn quantize_angle(turns: f64, bits: u32) -> Option<u64> {
    let mask = angle_mask(bits)?;
    if !turns.is_finite() {
        return None;
    }
    let fraction = turns - turns.floor();
    let steps = (1u64 << bits) as f64;
    // Rounding up to a whole turn wraps to zero.
    Some(((fraction * steps).round() as u64) & mask)
}

/// The fraction of a turn, 0 up to but not including 1, for an angle of
/// `bits` bits. `None` for a bad width or a value over `2^bits - 1`.
pub fn dequantize_angle(quantized: u64, bits: u32) -> Option<f64> {
    let mask = angle_mask(bits)?;
    if quantized > mask {
        return None;
    }
    Some(quantized as f64 / (1u64 << bits) as f64)
}

/// The short-way difference `to - from` of two angles of `bits` bits, in
/// steps: from `-2^(bits-1)` to `2^(bits-1) - 1`, so exactly half a turn
/// counts as negative. Inputs wrap modulo a turn. `None` for a bad width.
pub fn angle_diff(from: u64, to: u64, bits: u32) -> Option<i64> {
    let mask = angle_mask(bits)?;
    let diff = to.wrapping_sub(from) & mask;
    let half = 1u64 << (bits - 1);
    Some(if diff >= half {
        diff as i64 - (mask as i64 + 1)
    } else {
        diff as i64
    })
}

/// The short-way difference `to - from` of two angles in turns, from -0.5
/// up to but not including 0.5. `None` if either is not finite.
pub fn angle_diff_turns(from: f64, to: f64) -> Option<f64> {
    if !from.is_finite() || !to.is_finite() {
        return None;
    }
    let diff = to - from;
    Some(diff - (diff + 0.5).floor())
}

/// A 0 to 1 value as a whole number of `steps` (use [`UNIT_U8_STEPS`] or
/// [`UNIT_U16_STEPS`]). `None` outside 0 to 1, for a value that is not
/// finite, or for zero steps.
pub fn quantize_unit(value: f64, steps: u32) -> Option<u32> {
    if steps == 0 || !(0.0..=1.0).contains(&value) {
        return None;
    }
    let q = (value * f64::from(steps)).round() as u32;
    (q <= steps).then_some(q)
}

/// The 0 to 1 value for `quantized` of `steps`; `None` if it is over `steps`.
pub fn dequantize_unit(quantized: u32, steps: u32) -> Option<f64> {
    if steps == 0 || quantized > steps {
        return None;
    }
    Some(f64::from(quantized) / f64::from(steps))
}

/// A -1 to 1 value as a whole number of `steps` each side of zero (use
/// [`SIGNED_UNIT_I8_STEPS`] or [`SIGNED_UNIT_I16_STEPS`]), so the result is
/// `-steps` to `steps`. `None` outside -1 to 1, for a value that is not
/// finite, for zero steps or for more than `i32::MAX` steps.
pub fn quantize_signed_unit(value: f64, steps: u32) -> Option<i32> {
    if steps == 0 || steps > i32::MAX as u32 || !(-1.0..=1.0).contains(&value) {
        return None;
    }
    let q = (value * f64::from(steps)).round() as i32;
    (q.unsigned_abs() <= steps).then_some(q)
}

/// The -1 to 1 value for `quantized` of `steps`; `None` if its size is over
/// `steps`.
pub fn dequantize_signed_unit(quantized: i32, steps: u32) -> Option<f64> {
    if steps == 0 || quantized.unsigned_abs() > steps {
        return None;
    }
    Some(f64::from(quantized) / f64::from(steps))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_rounds_to_nearest() {
        let range = -1000..=1000;
        assert_eq!(quantize(0.0, 1.0 / 32.0, range.clone()), Some(0));
        assert_eq!(quantize(-0.0, 1.0 / 32.0, range.clone()), Some(0));
        assert_eq!(quantize(1.0, 1.0 / 32.0, range.clone()), Some(32));
        assert_eq!(
            quantize(1.0 / 64.0 + 1e-9, 1.0 / 32.0, range.clone()),
            Some(1)
        );
        assert_eq!(
            quantize(1.0 / 64.0 - 1e-9, 1.0 / 32.0, range.clone()),
            Some(0)
        );
        // Halves go away from zero.
        assert_eq!(quantize(0.5, 1.0, range.clone()), Some(1));
        assert_eq!(quantize(-0.5, 1.0, range.clone()), Some(-1));
        assert_eq!(quantize(2.5, 1.0, range.clone()), Some(3));
        assert_eq!(dequantize(32, 1.0 / 32.0), 1.0);
        assert_eq!(dequantize(-3, 0.25), -0.75);
    }

    #[test]
    fn quantize_refuses_what_it_cannot_hold() {
        let range = -10..=10;
        assert_eq!(quantize(10.0, 1.0, range.clone()), Some(10));
        assert_eq!(quantize(-10.0, 1.0, range.clone()), Some(-10));
        assert_eq!(quantize(10.6, 1.0, range.clone()), None);
        assert_eq!(quantize(-10.6, 1.0, range.clone()), None);
        assert_eq!(quantize(f64::NAN, 1.0, range.clone()), None);
        assert_eq!(quantize(f64::INFINITY, 1.0, range.clone()), None);
        assert_eq!(quantize(f64::NEG_INFINITY, 1.0, range.clone()), None);
        assert_eq!(quantize(1.0, 0.0, range.clone()), None);
        assert_eq!(quantize(1.0, -1.0, range.clone()), None);
        assert_eq!(quantize(1.0, f64::NAN, range.clone()), None);
        assert_eq!(quantize(1.0, f64::INFINITY, range.clone()), None);
    }

    #[test]
    fn quantize_handles_the_ends_of_i64() {
        let full = i64::MIN..=i64::MAX;
        assert_eq!(quantize(1e30, 1.0, full.clone()), None);
        assert_eq!(quantize(-1e30, 1.0, full.clone()), None);
        assert_eq!(quantize(f64::MAX, f64::MIN_POSITIVE, full.clone()), None);
        assert_eq!(quantize(-TWO_POW_63, 1.0, full.clone()), Some(i64::MIN));
        assert_eq!(quantize(TWO_POW_63, 1.0, full), None);
    }

    #[test]
    fn angles_wrap_and_round() {
        assert_eq!(quantize_angle(0.0, 16), Some(0));
        assert_eq!(quantize_angle(0.25, 16), Some(16_384));
        assert_eq!(quantize_angle(1.25, 16), Some(16_384));
        assert_eq!(quantize_angle(-0.75, 16), Some(16_384));
        assert_eq!(quantize_angle(-0.25, 16), Some(49_152));
        assert_eq!(quantize_angle(0.5, 1), Some(1));
        // Just under a full turn rounds up and wraps to zero.
        assert_eq!(quantize_angle(1.0 - 1e-12, 16), Some(0));
        assert_eq!(quantize_angle(-1e-20, 16), Some(0));
        assert_eq!(quantize_angle(f64::NAN, 16), None);
        assert_eq!(quantize_angle(f64::INFINITY, 16), None);
        assert_eq!(quantize_angle(0.0, 0), None);
        assert_eq!(quantize_angle(0.0, 53), None);
        assert!(quantize_angle(0.123, 52).is_some());
        for bits in [1, 2, 8, 16, 20, 32, 52] {
            let steps = 1u64 << bits;
            for q in [0, 1, steps / 2, steps - 1] {
                let turns = dequantize_angle(q, bits).unwrap();
                assert!((0.0..1.0).contains(&turns));
                assert_eq!(quantize_angle(turns, bits), Some(q), "{bits} {q}");
            }
            assert_eq!(dequantize_angle(steps, bits), None);
        }
    }

    #[test]
    fn angle_differences_take_the_short_way() {
        assert_eq!(angle_diff(0, 0, 16), Some(0));
        assert_eq!(angle_diff(0, 100, 16), Some(100));
        assert_eq!(angle_diff(100, 0, 16), Some(-100));
        assert_eq!(angle_diff(65_500, 10, 16), Some(46));
        assert_eq!(angle_diff(10, 65_500, 16), Some(-46));
        assert_eq!(angle_diff(0, 32_768, 16), Some(-32_768));
        assert_eq!(angle_diff(0, 32_767, 16), Some(32_767));
        assert_eq!(angle_diff(0, 1, 1), Some(-1));
        assert_eq!(angle_diff(0, 0, 0), None);
        assert_eq!(angle_diff(0, 0, 53), None);
        // Inputs wrap.
        assert_eq!(angle_diff(0, 65_536 + 5, 16), Some(5));
        // Adding the difference returns the target.
        for (from, to) in [(0u64, 200u64), (60_000, 100), (1, 65_535), (30_000, 30_000)] {
            let d = angle_diff(from, to, 16).unwrap();
            assert_eq!((from as i64 + d).rem_euclid(65_536) as u64, to);
        }
        assert_eq!(angle_diff_turns(0.9, 0.1), Some(0.19999999999999996));
        assert_eq!(angle_diff_turns(0.0, 0.5), Some(-0.5));
        assert_eq!(angle_diff_turns(0.0, 0.25), Some(0.25));
        assert_eq!(angle_diff_turns(f64::NAN, 0.0), None);
        assert_eq!(angle_diff_turns(0.0, f64::INFINITY), None);
    }

    #[test]
    fn unit_ranges() {
        for steps in [UNIT_U8_STEPS, UNIT_U16_STEPS] {
            assert_eq!(quantize_unit(0.0, steps), Some(0));
            assert_eq!(quantize_unit(1.0, steps), Some(steps));
            assert_eq!(quantize_unit(0.5, steps), Some(steps / 2 + 1));
            assert_eq!(quantize_unit(-0.0, steps), Some(0));
            assert_eq!(quantize_unit(-0.001, steps), None);
            assert_eq!(quantize_unit(1.001, steps), None);
            assert_eq!(quantize_unit(f64::NAN, steps), None);
            assert_eq!(dequantize_unit(0, steps), Some(0.0));
            assert_eq!(dequantize_unit(steps, steps), Some(1.0));
            assert_eq!(dequantize_unit(steps + 1, steps), None);
            for q in [0, 1, steps / 3, steps - 1, steps] {
                let v = dequantize_unit(q, steps).unwrap();
                assert_eq!(quantize_unit(v, steps), Some(q));
            }
        }
        assert_eq!(quantize_unit(0.5, 0), None);
        assert_eq!(dequantize_unit(0, 0), None);
    }

    #[test]
    fn signed_unit_ranges() {
        for steps in [SIGNED_UNIT_I8_STEPS, SIGNED_UNIT_I16_STEPS] {
            let s = steps as i32;
            assert_eq!(quantize_signed_unit(0.0, steps), Some(0));
            assert_eq!(quantize_signed_unit(1.0, steps), Some(s));
            assert_eq!(quantize_signed_unit(-1.0, steps), Some(-s));
            assert_eq!(quantize_signed_unit(-0.0, steps), Some(0));
            assert_eq!(quantize_signed_unit(1.001, steps), None);
            assert_eq!(quantize_signed_unit(-1.001, steps), None);
            assert_eq!(quantize_signed_unit(f64::INFINITY, steps), None);
            assert_eq!(dequantize_signed_unit(s, steps), Some(1.0));
            assert_eq!(dequantize_signed_unit(-s, steps), Some(-1.0));
            assert_eq!(dequantize_signed_unit(s + 1, steps), None);
            assert_eq!(dequantize_signed_unit(-s - 1, steps), None);
            assert_eq!(dequantize_signed_unit(i32::MIN, steps), None);
            for q in [-s, -s + 1, -1, 0, 1, s - 1, s] {
                let v = dequantize_signed_unit(q, steps).unwrap();
                assert_eq!(quantize_signed_unit(v, steps), Some(q));
            }
        }
        // Halves away from zero: 0.5 of 127 steps is 63.5, which is 64.
        assert_eq!(quantize_signed_unit(0.5, SIGNED_UNIT_I8_STEPS), Some(64));
        assert_eq!(quantize_signed_unit(-0.5, SIGNED_UNIT_I8_STEPS), Some(-64));
        assert_eq!(quantize_signed_unit(0.5, 0), None);
        assert_eq!(quantize_signed_unit(0.5, u32::MAX), None);
    }
}
