//! Variable-length integers and the bucketed signed coding.
//!
//! An unsigned integer is written in groups of 7 value bits, lowest group
//! first, each followed by a continuation bit (set when another group
//! follows). At a byte boundary the bytes are those of ordinary LEB128. A
//! 64-bit value takes at most 10 groups. Signed integers go through zigzag
//! first, so small magnitudes of either sign are short.
//!
//! The bucketed coding is for residuals that are usually small but
//! sometimes not: the caller gives a ladder of bit widths, such as
//! `[4, 8, 16, 32]`; a value is written as the index of the first width that
//! holds it, then the value in that width, and a value beyond the last width
//! uses the escape index followed by a full signed varint.

use crate::{BitReader, BitWriter, CodecError, bits::signed_fits};

/// The most groups a 64-bit varint has.
const MAX_GROUPS: usize = 10;

/// Maps a signed number to an unsigned one so small magnitudes are small:
/// 0, -1, 1, -2, 2 become 0, 1, 2, 3, 4.
pub fn zigzag_encode(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

/// Undoes [`zigzag_encode`].
pub fn zigzag_decode(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}

impl BitWriter {
    /// Writes an unsigned variable-length integer, 1 to 10 groups.
    pub fn write_varint(&mut self, mut value: u64) {
        loop {
            let group = value & 0x7F;
            value >>= 7;
            let more = u64::from(value != 0);
            // Eight bits is always a valid width.
            let _ = self.write_bits(group | (more << 7), 8);
            if value == 0 {
                break;
            }
        }
    }

    /// Writes a signed variable-length integer (zigzag, then a varint).
    pub fn write_varint_signed(&mut self, value: i64) {
        self.write_varint(zigzag_encode(value));
    }

    /// Writes a signed value with the bucketed coding over `ladder`.
    ///
    /// `ladder` holds strictly ascending widths, each 1 to 64, else
    /// [`CodecError::InvalidLadder`]. The value goes in the first bucket whose
    /// width holds it as a two's complement number; a value beyond every
    /// bucket takes the escape.
    pub fn write_bucketed(&mut self, value: i64, ladder: &[u32]) -> Result<(), CodecError> {
        validate_ladder(ladder)?;
        let index_bits = index_bits(ladder.len());
        match ladder.iter().position(|&w| signed_fits(value, w)) {
            Some(index) => {
                self.write_bits(index as u64, index_bits)?;
                self.write_signed(value, ladder[index])
            }
            None => {
                self.write_bits(ladder.len() as u64, index_bits)?;
                self.write_varint_signed(value);
                Ok(())
            }
        }
    }
}

impl BitReader<'_> {
    /// Reads an unsigned variable-length integer. Rejects more than 10
    /// groups ([`CodecError::VarintTooLong`]), a value over 64 bits
    /// ([`CodecError::VarintOverflow`]) and a final group of zero after the
    /// first ([`CodecError::NonCanonical`]: the writer never pads).
    pub fn read_varint(&mut self) -> Result<u64, CodecError> {
        let mut value = 0u64;
        for index in 0..MAX_GROUPS {
            let byte = self.read_bits(8)?;
            let data = byte & 0x7F;
            let more = byte & 0x80 != 0;
            if index == MAX_GROUPS - 1 {
                // The tenth group carries only bit 63.
                if data > 1 {
                    return Err(CodecError::VarintOverflow);
                }
                if more {
                    return Err(CodecError::VarintTooLong);
                }
            }
            value |= data << (7 * index);
            if !more {
                if index > 0 && data == 0 {
                    return Err(CodecError::NonCanonical);
                }
                return Ok(value);
            }
        }
        Err(CodecError::VarintTooLong)
    }

    /// Reads a signed variable-length integer.
    pub fn read_varint_signed(&mut self) -> Result<i64, CodecError> {
        Ok(zigzag_decode(self.read_varint()?))
    }

    /// Reads a value written by [`BitWriter::write_bucketed`] with the same
    /// `ladder`. A bucket index beyond the escape is
    /// [`CodecError::BadBucket`]; a value that the writer would have put in
    /// an earlier bucket or the bucket's own range is
    /// [`CodecError::NonCanonical`].
    pub fn read_bucketed(&mut self, ladder: &[u32]) -> Result<i64, CodecError> {
        validate_ladder(ladder)?;
        let index = self.read_bits(index_bits(ladder.len()))? as usize;
        let value = if index < ladder.len() {
            self.read_signed(ladder[index])?
        } else if index == ladder.len() {
            self.read_varint_signed()?
        } else {
            return Err(CodecError::BadBucket);
        };
        // The writer always picks the first bucket that holds the value.
        let first = ladder.iter().position(|&w| signed_fits(value, w));
        let expected = first.unwrap_or(ladder.len());
        if expected != index {
            return Err(CodecError::NonCanonical);
        }
        Ok(value)
    }
}

fn validate_ladder(ladder: &[u32]) -> Result<(), CodecError> {
    let mut previous = 0u32;
    for &width in ladder {
        if width == 0 || width > 64 || width <= previous {
            return Err(CodecError::InvalidLadder);
        }
        previous = width;
    }
    Ok(())
}

/// Bits for an index over `buckets` buckets plus the escape.
fn index_bits(buckets: usize) -> u32 {
    let values = buckets as u64 + 1;
    64 - (values - 1).leading_zeros()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn varint_bytes(value: u64) -> Vec<u8> {
        let mut w = BitWriter::new();
        w.write_varint(value);
        w.finish()
    }

    #[test]
    fn varints_match_leb128_at_a_byte_boundary() {
        assert_eq!(varint_bytes(0), vec![0x00]);
        assert_eq!(varint_bytes(127), vec![0x7F]);
        assert_eq!(varint_bytes(128), vec![0x80, 0x01]);
        assert_eq!(varint_bytes(300), vec![0xAC, 0x02]);
        assert_eq!(varint_bytes(u64::MAX).len(), 10);
        assert_eq!(
            varint_bytes(u64::MAX),
            vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01]
        );
    }

    #[test]
    fn varint_edges_round_trip() {
        let mut values = vec![0u64, 1, u64::MAX, u64::MAX - 1, 1 << 63];
        for shift in 0..64 {
            values.push(1 << shift);
            values.push((1 << shift) - 1);
            values.push((1 << shift) + 1);
        }
        for offset in [0u32, 3, 7] {
            for &v in &values {
                let mut w = BitWriter::new();
                w.write_bits(0, offset).unwrap();
                w.write_varint(v);
                let bytes = w.finish();
                let mut r = BitReader::new(&bytes);
                r.read_bits(offset).unwrap();
                assert_eq!(r.read_varint().unwrap(), v);
            }
        }
    }

    #[test]
    fn zigzag_values() {
        assert_eq!(zigzag_encode(0), 0);
        assert_eq!(zigzag_encode(-1), 1);
        assert_eq!(zigzag_encode(1), 2);
        assert_eq!(zigzag_encode(-2), 3);
        assert_eq!(zigzag_encode(i64::MAX), u64::MAX - 1);
        assert_eq!(zigzag_encode(i64::MIN), u64::MAX);
        for v in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX] {
            assert_eq!(zigzag_decode(zigzag_encode(v)), v);
            let mut w = BitWriter::new();
            w.write_varint_signed(v);
            let bytes = w.finish();
            assert_eq!(BitReader::new(&bytes).read_varint_signed().unwrap(), v);
        }
    }

    #[test]
    fn bad_varints_are_rejected() {
        // Eleven groups.
        let too_long = [0x80u8; 11];
        assert!(matches!(
            BitReader::new(&too_long).read_varint(),
            Err(CodecError::VarintTooLong)
        ));
        // Tenth group with a continuation bit.
        let mut ten_more = [0xFFu8; 10];
        ten_more[9] = 0x81;
        assert_eq!(
            BitReader::new(&ten_more).read_varint(),
            Err(CodecError::VarintTooLong)
        );
        // Tenth group holding more than bit 63.
        let mut overflow = [0xFFu8; 10];
        overflow[9] = 0x02;
        assert_eq!(
            BitReader::new(&overflow).read_varint(),
            Err(CodecError::VarintOverflow)
        );
        // A padded zero group.
        assert_eq!(
            BitReader::new(&[0x80, 0x00]).read_varint(),
            Err(CodecError::NonCanonical)
        );
        // Truncated.
        assert_eq!(
            BitReader::new(&[0x80]).read_varint(),
            Err(CodecError::UnexpectedEnd)
        );
    }

    const LADDER: [u32; 4] = [4, 8, 16, 32];

    #[test]
    fn bucketed_picks_the_smallest_bucket() {
        let cases: [(i64, usize); 10] = [
            (0, 0),
            (7, 0),
            (-8, 0),
            (8, 1),
            (-9, 1),
            (127, 1),
            (128, 2),
            (32_767, 2),
            (32_768, 3),
            (-2_147_483_648, 3),
        ];
        for (value, bucket) in cases {
            let mut w = BitWriter::new();
            w.write_bucketed(value, &LADDER).unwrap();
            // 3 bits of index plus the bucket width.
            assert_eq!(w.bit_len(), 3 + LADDER[bucket] as usize, "{value}");
            let bytes = w.finish();
            assert_eq!(
                BitReader::new(&bytes).read_bucketed(&LADDER).unwrap(),
                value
            );
        }
    }

    #[test]
    fn bucketed_escape_and_extremes_round_trip() {
        for value in [2_147_483_648, -2_147_483_649, i64::MAX, i64::MIN] {
            let mut w = BitWriter::new();
            w.write_bucketed(value, &LADDER).unwrap();
            let bytes = w.finish();
            let mut r = BitReader::new(&bytes);
            assert_eq!(r.read_bucketed(&LADDER).unwrap(), value);
            assert!(r.only_zero_padding_left());
        }
    }

    #[test]
    fn bucketed_with_other_ladders() {
        // No buckets: only the escape, with a zero-bit index.
        let mut w = BitWriter::new();
        w.write_bucketed(5, &[]).unwrap();
        assert_eq!(w.bit_len(), 8);
        let bytes = w.finish();
        assert_eq!(BitReader::new(&bytes).read_bucketed(&[]).unwrap(), 5);
        // A full-width last bucket never escapes.
        for ladder in [&[64u32][..], &[1, 64], &[1, 2, 3, 64]] {
            for v in [i64::MIN, -1, 0, 1, i64::MAX] {
                let mut w = BitWriter::new();
                w.write_bucketed(v, ladder).unwrap();
                let bytes = w.finish();
                assert_eq!(BitReader::new(&bytes).read_bucketed(ladder).unwrap(), v);
            }
        }
    }

    #[test]
    fn bad_ladders_and_buckets_are_rejected() {
        for bad in [&[0u32][..], &[65], &[8, 8], &[16, 8]] {
            let mut w = BitWriter::new();
            assert_eq!(w.write_bucketed(1, bad), Err(CodecError::InvalidLadder));
            assert_eq!(w.bit_len(), 0);
            assert_eq!(
                BitReader::new(&[0; 8]).read_bucketed(bad),
                Err(CodecError::InvalidLadder)
            );
        }
        // Index 5 over a 4-bucket ladder (index 4 is the escape).
        assert_eq!(
            BitReader::new(&[0b101]).read_bucketed(&LADDER),
            Err(CodecError::BadBucket)
        );
        // Zero written in the 8-bit bucket instead of the 4-bit one.
        let mut w = BitWriter::new();
        w.write_bits(1, 3).unwrap();
        w.write_signed(0, 8).unwrap();
        let bytes = w.finish();
        assert_eq!(
            BitReader::new(&bytes).read_bucketed(&LADDER),
            Err(CodecError::NonCanonical)
        );
        // A small value behind the escape.
        let mut w = BitWriter::new();
        w.write_bits(4, 3).unwrap();
        w.write_varint_signed(3);
        let bytes = w.finish();
        assert_eq!(
            BitReader::new(&bytes).read_bucketed(&LADDER),
            Err(CodecError::NonCanonical)
        );
    }
}
