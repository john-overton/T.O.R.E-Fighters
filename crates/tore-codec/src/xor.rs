//! Exact floats and integers against a baseline.
//!
//! The Gorilla time-series scheme: exclusive-or the value's bit pattern with a
//! baseline the reader also has. One bit says whether they are equal. If not,
//! the record is a 1 bit, the count of leading zero bits of the difference (6
//! bits, 0 to 63), the count of meaningful bits minus one (6 bits, 1 to 64
//! meaningful), and those meaningful bits: the difference from its first set
//! bit to its last, with the trailing zeros left out. The reader derives the
//! trailing zeros from the two counts.
//!
//! Every bit pattern round trips exactly, including NaN payloads, infinities,
//! negative zero and subnormals, because the coding works on bits, never on
//! the float's value. A value near its baseline (the same sign and exponent)
//! costs the mantissa bits that changed plus 13.

use crate::{BitReader, BitWriter, CodecError};

impl BitWriter {
    /// Writes `value` against `baseline`, exactly.
    pub fn write_f64_xor(&mut self, value: f64, baseline: f64) {
        self.write_u64_xor(value.to_bits(), baseline.to_bits());
    }

    /// Writes the 64-bit pattern `value` against `baseline`, exactly.
    pub fn write_u64_xor(&mut self, value: u64, baseline: u64) {
        let diff = value ^ baseline;
        if diff == 0 {
            self.write_bool(false);
            return;
        }
        let leading = diff.leading_zeros();
        let trailing = diff.trailing_zeros();
        let meaningful = 64 - leading - trailing;
        self.write_bool(true);
        // The widths below are all valid, so these cannot fail.
        let _ = self.write_bits(u64::from(leading), 6);
        let _ = self.write_bits(u64::from(meaningful - 1), 6);
        let _ = self.write_bits(diff >> trailing, meaningful);
    }
}

impl BitReader<'_> {
    /// Reads a float written by [`BitWriter::write_f64_xor`] against the same
    /// `baseline`.
    pub fn read_f64_xor(&mut self, baseline: f64) -> Result<f64, CodecError> {
        Ok(f64::from_bits(self.read_u64_xor(baseline.to_bits())?))
    }

    /// Reads a pattern written by [`BitWriter::write_u64_xor`] against the
    /// same `baseline`. A record whose counts pass 64 bits is
    /// [`CodecError::BadXor`]; one whose first or last meaningful bit is not
    /// set, which the writer never produces, is [`CodecError::NonCanonical`].
    pub fn read_u64_xor(&mut self, baseline: u64) -> Result<u64, CodecError> {
        if !self.read_bool()? {
            return Ok(baseline);
        }
        let leading = self.read_bits(6)? as u32;
        let meaningful = self.read_bits(6)? as u32 + 1;
        if leading + meaningful > 64 {
            return Err(CodecError::BadXor);
        }
        let bits = self.read_bits(meaningful)?;
        if bits & 1 == 0 || bits >> (meaningful - 1) != 1 {
            return Err(CodecError::NonCanonical);
        }
        let trailing = 64 - leading - meaningful;
        Ok(baseline ^ (bits << trailing))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(value: u64, baseline: u64) -> usize {
        let mut w = BitWriter::new();
        w.write_bits(0b101, 3).unwrap();
        w.write_u64_xor(value, baseline);
        let bits = w.bit_len() - 3;
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes);
        r.read_bits(3).unwrap();
        assert_eq!(r.read_u64_xor(baseline).unwrap(), value);
        assert!(r.only_zero_padding_left());
        bits
    }

    #[test]
    fn equal_values_cost_one_bit() {
        assert_eq!(round_trip(0, 0), 1);
        assert_eq!(round_trip(u64::MAX, u64::MAX), 1);
        assert_eq!(round_trip(1.5f64.to_bits(), 1.5f64.to_bits()), 1);
    }

    #[test]
    fn small_changes_are_cheap() {
        // A single low mantissa bit: 1 + 6 + 6 + 1.
        assert_eq!(round_trip(1.5f64.to_bits() ^ 1, 1.5f64.to_bits()), 14);
        // Anything is at most 1 + 6 + 6 + 64.
        assert_eq!(round_trip(u64::MAX, 0), 77);
        assert_eq!(round_trip(1, 0), 14);
        assert_eq!(round_trip(1 << 63, 0), 14);
    }

    #[test]
    fn every_special_float_round_trips_bit_exact() {
        let specials = [
            0.0f64,
            -0.0,
            1.0,
            -1.0,
            f64::MIN_POSITIVE,
            f64::MIN_POSITIVE / 2.0,
            f64::from_bits(1),
            f64::from_bits(0x000F_FFFF_FFFF_FFFF),
            f64::from_bits(0x800F_FFFF_FFFF_FFFF),
            f64::MAX,
            f64::MIN,
            f64::EPSILON,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
            f64::from_bits(0x7FF8_0000_0000_0001),
            f64::from_bits(0xFFF8_DEAD_BEEF_0001),
            f64::from_bits(0x7FF0_0000_0000_0001),
            f64::from_bits(u64::MAX),
            std::f64::consts::PI,
            1234.5678,
        ];
        for &v in &specials {
            for &b in &specials {
                let mut w = BitWriter::new();
                w.write_f64_xor(v, b);
                let bytes = w.finish();
                let back = BitReader::new(&bytes).read_f64_xor(b).unwrap();
                assert_eq!(back.to_bits(), v.to_bits());
            }
        }
    }

    #[test]
    fn single_bit_differences_at_every_position() {
        for shift in 0..64 {
            for baseline in [0u64, u64::MAX, 0x0123_4567_89AB_CDEF] {
                round_trip(baseline ^ (1 << shift), baseline);
                round_trip(baseline ^ (u64::MAX << shift), baseline);
                round_trip(baseline ^ (u64::MAX >> shift), baseline);
            }
        }
    }

    #[test]
    fn bad_records_are_rejected() {
        // Leading 40 and 30 meaningful pass 64.
        let mut w = BitWriter::new();
        w.write_bool(true);
        w.write_bits(40, 6).unwrap();
        w.write_bits(29, 6).unwrap();
        w.write_bits(u64::MAX, 30).unwrap();
        let bytes = w.finish();
        assert_eq!(
            BitReader::new(&bytes).read_u64_xor(0),
            Err(CodecError::BadXor)
        );
        // Meaningful block with a clear low bit.
        let mut w = BitWriter::new();
        w.write_bool(true);
        w.write_bits(0, 6).unwrap();
        w.write_bits(3, 6).unwrap();
        w.write_bits(0b1010, 4).unwrap();
        let bytes = w.finish();
        assert_eq!(
            BitReader::new(&bytes).read_u64_xor(0),
            Err(CodecError::NonCanonical)
        );
        // Meaningful block with a clear top bit.
        let mut w = BitWriter::new();
        w.write_bool(true);
        w.write_bits(0, 6).unwrap();
        w.write_bits(3, 6).unwrap();
        w.write_bits(0b0011, 4).unwrap();
        let bytes = w.finish();
        assert_eq!(
            BitReader::new(&bytes).read_u64_xor(0),
            Err(CodecError::NonCanonical)
        );
        // Truncated.
        assert_eq!(
            BitReader::new(&[0b0000_0001]).read_u64_xor(0),
            Err(CodecError::UnexpectedEnd)
        );
    }
}
