//! The bit writer and the bounded bit reader.
//!
//! Bits are packed least significant bit first: the first bit written is bit 0
//! of byte 0, the ninth is bit 0 of byte 1, and a multi-bit value occupies its
//! bits from its own bit 0 upward. A value written with 8 bits at a byte
//! boundary is therefore the byte itself. See the "Overview" of
//! [`docs/formats/net-protocol.md`](../../../docs/formats/net-protocol.md).

use crate::CodecError;

/// Writes fields into a growing byte buffer.
///
/// Writing never fails for bits, bytes or bools. Calls that can be given
/// something unwritable (a width over 64, a value that does not fit) say so in
/// their documentation.
#[derive(Debug, Clone, Default)]
pub struct BitWriter {
    bytes: Vec<u8>,
    bit_len: usize,
}

impl BitWriter {
    /// An empty writer.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty writer with room for `bytes` bytes.
    pub fn with_capacity(bytes: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(bytes),
            bit_len: 0,
        }
    }

    /// Bits written so far.
    pub fn bit_len(&self) -> usize {
        self.bit_len
    }

    /// Bytes used so far, counting a partly filled last byte.
    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }

    /// True when the next bit starts a byte.
    pub fn is_aligned(&self) -> bool {
        self.bit_len.is_multiple_of(8)
    }

    /// The bytes written so far; a partly filled last byte has zero padding.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Writes the low `bits` bits of `value`, 0 to 64. Higher bits of `value`
    /// are ignored. A width over 64 is a caller bug and returns
    /// [`CodecError::InvalidWidth`] without writing.
    pub fn write_bits(&mut self, value: u64, bits: u32) -> Result<(), CodecError> {
        if bits > 64 {
            return Err(CodecError::InvalidWidth);
        }
        let mut rest = if bits == 64 {
            value
        } else {
            value & ((1u64 << bits) - 1)
        };
        let mut left = bits;
        while left > 0 {
            let offset = (self.bit_len % 8) as u32;
            if offset == 0 {
                self.bytes.push(0);
            }
            let take = (8 - offset).min(left);
            let chunk = (rest & ((1u64 << take) - 1)) as u8;
            if let Some(last) = self.bytes.last_mut() {
                *last |= chunk << offset;
            }
            rest >>= take;
            left -= take;
            self.bit_len += take as usize;
        }
        Ok(())
    }

    /// Writes one bit.
    pub fn write_bool(&mut self, value: bool) {
        // One bit is always a valid width.
        let _ = self.write_bits(u64::from(value), 1);
    }

    /// Writes `value` as a two's complement number in `bits` bits, 1 to 64.
    /// A value outside `-2^(bits-1)` to `2^(bits-1)-1` returns
    /// [`CodecError::ValueOutOfRange`] and a bad width
    /// [`CodecError::InvalidWidth`], in both cases without writing.
    pub fn write_signed(&mut self, value: i64, bits: u32) -> Result<(), CodecError> {
        if bits == 0 || bits > 64 {
            return Err(CodecError::InvalidWidth);
        }
        if !signed_fits(value, bits) {
            return Err(CodecError::ValueOutOfRange);
        }
        self.write_bits(value as u64, bits)
    }

    /// Pads with zero bits to the next byte boundary. Does nothing when
    /// already aligned.
    pub fn align(&mut self) {
        let offset = self.bit_len % 8;
        if offset != 0 {
            self.bit_len += 8 - offset;
        }
    }

    /// Writes whole bytes, at any bit position.
    pub fn write_bytes(&mut self, data: &[u8]) {
        if self.is_aligned() {
            self.bytes.extend_from_slice(data);
            self.bit_len += data.len() * 8;
        } else {
            for &byte in data {
                let _ = self.write_bits(u64::from(byte), 8);
            }
        }
    }

    /// Ends the writer and returns the bytes, the last one zero padded.
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// True when `value` fits in `bits` bits as a two's complement number.
/// `bits` must be 1 to 64.
pub(crate) fn signed_fits(value: i64, bits: u32) -> bool {
    if bits >= 64 {
        return true;
    }
    let limit = 1i64 << (bits - 1);
    (-limit..limit).contains(&value)
}

/// Reads fields from a byte slice, every read bounded.
///
/// A read that fails returns an error and leaves the position unspecified;
/// callers drop the whole packet on the first error.
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    /// A reader at the first bit of `bytes`.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn total_bits(&self) -> usize {
        self.bytes.len().saturating_mul(8)
    }

    /// Bits read so far.
    pub fn bit_position(&self) -> usize {
        self.pos
    }

    /// Bits left to read.
    pub fn bits_remaining(&self) -> usize {
        self.total_bits().saturating_sub(self.pos)
    }

    /// True when the next bit starts a byte.
    pub fn is_aligned(&self) -> bool {
        self.pos.is_multiple_of(8)
    }

    /// True when every remaining bit is zero (including when none remain):
    /// only padding is left.
    pub fn only_zero_padding_left(&self) -> bool {
        let mut pos = self.pos;
        let total = self.total_bits();
        while pos < total {
            let offset = (pos % 8) as u32;
            let Some(&byte) = self.bytes.get(pos / 8) else {
                return true;
            };
            if byte >> offset != 0 {
                return false;
            }
            pos += (8 - offset) as usize;
        }
        true
    }

    /// Reads `bits` bits, 0 to 64, as an unsigned number.
    pub fn read_bits(&mut self, bits: u32) -> Result<u64, CodecError> {
        if bits > 64 {
            return Err(CodecError::InvalidWidth);
        }
        if bits as usize > self.bits_remaining() {
            return Err(CodecError::UnexpectedEnd);
        }
        let mut out = 0u64;
        let mut got = 0u32;
        while got < bits {
            let offset = (self.pos % 8) as u32;
            let Some(&byte) = self.bytes.get(self.pos / 8) else {
                return Err(CodecError::UnexpectedEnd);
            };
            let take = (8 - offset).min(bits - got);
            let chunk = u64::from(byte >> offset) & ((1u64 << take) - 1);
            out |= chunk << got;
            got += take;
            self.pos += take as usize;
        }
        Ok(out)
    }

    /// Reads one bit.
    pub fn read_bool(&mut self) -> Result<bool, CodecError> {
        Ok(self.read_bits(1)? == 1)
    }

    /// Reads a two's complement number of `bits` bits, 1 to 64.
    pub fn read_signed(&mut self, bits: u32) -> Result<i64, CodecError> {
        if bits == 0 || bits > 64 {
            return Err(CodecError::InvalidWidth);
        }
        let raw = self.read_bits(bits)?;
        let shift = 64 - bits;
        Ok(((raw << shift) as i64) >> shift)
    }

    /// Skips to the next byte boundary, whatever the skipped bits hold.
    pub fn align(&mut self) {
        let offset = self.pos % 8;
        if offset != 0 {
            self.pos = (self.pos + 8 - offset).min(self.total_bits());
        }
    }

    /// Skips to the next byte boundary and requires the skipped bits to be
    /// zero, as the writer leaves them: [`CodecError::NonCanonical`] if not.
    pub fn align_strict(&mut self) -> Result<(), CodecError> {
        let offset = (self.pos % 8) as u32;
        if offset == 0 {
            return Ok(());
        }
        let skip = 8 - offset;
        // The skipped bits are inside a byte that exists, since `pos` is
        // inside it.
        if self.read_bits(skip)? != 0 {
            return Err(CodecError::NonCanonical);
        }
        Ok(())
    }

    /// Reads `count` whole bytes, at any bit position. Fails before
    /// allocating when fewer than `count` bytes remain.
    pub fn read_bytes(&mut self, count: usize) -> Result<Vec<u8>, CodecError> {
        if count.saturating_mul(8) > self.bits_remaining() {
            return Err(CodecError::UnexpectedEnd);
        }
        if self.is_aligned() {
            let start = self.pos / 8;
            let slice = self
                .bytes
                .get(start..start + count)
                .ok_or(CodecError::UnexpectedEnd)?;
            self.pos += count * 8;
            return Ok(slice.to_vec());
        }
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(self.read_bits(8)? as u8);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_least_significant_bit_first() {
        let mut w = BitWriter::new();
        w.write_bits(0b101, 3).unwrap();
        w.write_bits(0b11111, 5).unwrap();
        w.write_bits(0xAB, 8).unwrap();
        w.write_bool(true);
        assert_eq!(w.bit_len(), 17);
        assert_eq!(w.byte_len(), 3);
        assert_eq!(w.finish(), vec![0b1111_1101, 0xAB, 0b0000_0001]);
    }

    #[test]
    fn every_width_round_trips_at_every_offset() {
        let values = [
            0u64,
            1,
            2,
            0x5555_5555_5555_5555,
            0xAAAA_AAAA_AAAA_AAAA,
            u64::MAX,
            0x8000_0000_0000_0000,
            0x0123_4567_89AB_CDEF,
        ];
        for offset in 0..8u32 {
            for bits in 0..=64u32 {
                for &v in &values {
                    let mut w = BitWriter::new();
                    w.write_bits(0, offset).unwrap();
                    w.write_bits(v, bits).unwrap();
                    w.write_bool(true);
                    assert_eq!(w.bit_len(), offset as usize + bits as usize + 1);
                    let bytes = w.finish();
                    let mut r = BitReader::new(&bytes);
                    assert_eq!(r.read_bits(offset).unwrap(), 0);
                    let mask = if bits == 64 {
                        u64::MAX
                    } else {
                        (1u64 << bits) - 1
                    };
                    assert_eq!(r.read_bits(bits).unwrap(), v & mask, "{bits} {offset}");
                    assert!(r.read_bool().unwrap());
                    assert!(r.only_zero_padding_left());
                }
            }
        }
    }

    #[test]
    fn widths_over_64_are_errors() {
        let mut w = BitWriter::new();
        assert_eq!(w.write_bits(1, 65), Err(CodecError::InvalidWidth));
        assert_eq!(w.bit_len(), 0);
        let mut r = BitReader::new(&[0; 16]);
        assert_eq!(r.read_bits(65), Err(CodecError::InvalidWidth));
    }

    #[test]
    fn signed_fields_sign_extend() {
        for bits in 1..=64u32 {
            let min = if bits == 64 {
                i64::MIN
            } else {
                -(1i64 << (bits - 1))
            };
            let max = if bits == 64 {
                i64::MAX
            } else {
                (1i64 << (bits - 1)) - 1
            };
            for v in [min, min + 1, -1, 0, 1, max - 1, max] {
                if !(min..=max).contains(&v) {
                    continue;
                }
                let mut w = BitWriter::new();
                w.write_signed(v, bits).unwrap();
                let bytes = w.finish();
                assert_eq!(BitReader::new(&bytes).read_signed(bits).unwrap(), v);
            }
            if bits < 64 {
                let mut w = BitWriter::new();
                assert_eq!(
                    w.write_signed(max + 1, bits),
                    Err(CodecError::ValueOutOfRange)
                );
                assert_eq!(
                    w.write_signed(min - 1, bits),
                    Err(CodecError::ValueOutOfRange)
                );
            }
        }
        let mut w = BitWriter::new();
        assert_eq!(w.write_signed(0, 0), Err(CodecError::InvalidWidth));
    }

    #[test]
    fn align_and_bytes() {
        let mut w = BitWriter::new();
        w.write_bits(0b11, 2).unwrap();
        w.align();
        assert!(w.is_aligned());
        w.write_bytes(&[1, 2, 3]);
        w.write_bits(0, 1).unwrap();
        w.write_bytes(&[0xFE, 0x7F]);
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes);
        assert_eq!(r.read_bits(2).unwrap(), 0b11);
        r.align_strict().unwrap();
        assert_eq!(r.read_bytes(3).unwrap(), vec![1, 2, 3]);
        assert_eq!(r.read_bits(1).unwrap(), 0);
        assert_eq!(r.read_bytes(2).unwrap(), vec![0xFE, 0x7F]);
        assert!(r.only_zero_padding_left());
        assert_eq!(r.read_bytes(1), Err(CodecError::UnexpectedEnd));
    }

    #[test]
    fn strict_align_rejects_nonzero_padding() {
        let mut r = BitReader::new(&[0b0000_0111]);
        r.read_bits(1).unwrap();
        assert_eq!(r.align_strict(), Err(CodecError::NonCanonical));
        let mut r = BitReader::new(&[0b0000_0111]);
        r.read_bits(1).unwrap();
        r.align();
        assert_eq!(r.bits_remaining(), 0);
    }

    #[test]
    fn padding_detection() {
        let r = BitReader::new(&[0, 0, 0]);
        assert!(r.only_zero_padding_left());
        let r = BitReader::new(&[0, 0, 0x80]);
        assert!(!r.only_zero_padding_left());
        let mut r = BitReader::new(&[0x01, 0x00]);
        r.read_bits(1).unwrap();
        assert!(r.only_zero_padding_left());
        assert!(BitReader::new(&[]).only_zero_padding_left());
    }

    #[test]
    fn reads_past_the_end_are_errors() {
        let mut r = BitReader::new(&[0xFF]);
        assert_eq!(r.read_bits(9), Err(CodecError::UnexpectedEnd));
        assert_eq!(r.read_bits(8).unwrap(), 0xFF);
        assert_eq!(r.read_bool(), Err(CodecError::UnexpectedEnd));
        assert_eq!(r.read_bits(0).unwrap(), 0);
        assert_eq!(r.read_bytes(usize::MAX), Err(CodecError::UnexpectedEnd));
    }
}
