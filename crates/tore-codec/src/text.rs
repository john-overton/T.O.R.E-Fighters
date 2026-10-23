//! Short strings: a length byte, then UTF-8.
//!
//! At most 255 bytes (not characters) long. Higher layers add their own limits,
//! such as the protocol's 15-character callsigns.

use crate::{BitReader, BitWriter, CodecError};

/// The longest string, in bytes.
pub const MAX_STRING_BYTES: usize = 255;

impl BitWriter {
    /// Writes a length byte and the string's UTF-8 bytes, at any bit
    /// position. A string over 255 bytes is [`CodecError::StringTooLong`] and
    /// writes nothing.
    pub fn write_str(&mut self, text: &str) -> Result<(), CodecError> {
        let len = text.len();
        if len > MAX_STRING_BYTES {
            return Err(CodecError::StringTooLong);
        }
        self.write_bits(len as u64, 8)?;
        self.write_bytes(text.as_bytes());
        Ok(())
    }
}

impl BitReader<'_> {
    /// Reads a string written by [`BitWriter::write_str`]. Invalid UTF-8 is
    /// [`CodecError::InvalidUtf8`]. The length is checked against the
    /// remaining data before anything is allocated.
    pub fn read_str(&mut self) -> Result<String, CodecError> {
        let len = self.read_bits(8)? as usize;
        let bytes = self.read_bytes(len)?;
        String::from_utf8(bytes).map_err(|_| CodecError::InvalidUtf8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_round_trip_at_any_offset() {
        let long = "x".repeat(MAX_STRING_BYTES);
        let multi = "é".repeat(127);
        for offset in 0..8 {
            for text in ["", "a", "Viper_2", "naïve \u{1F6E9} 日本", &long, &multi] {
                let mut w = BitWriter::new();
                w.write_bits(0, offset).unwrap();
                w.write_str(text).unwrap();
                let bytes = w.finish();
                let mut r = BitReader::new(&bytes);
                r.read_bits(offset).unwrap();
                assert_eq!(r.read_str().unwrap(), text);
            }
        }
    }

    #[test]
    fn length_limit_counts_bytes() {
        let mut w = BitWriter::new();
        assert_eq!(
            w.write_str(&"x".repeat(256)),
            Err(CodecError::StringTooLong)
        );
        assert_eq!(w.bit_len(), 0);
        // 128 two-byte characters are 256 bytes.
        assert_eq!(
            w.write_str(&"é".repeat(128)),
            Err(CodecError::StringTooLong)
        );
    }

    #[test]
    fn bad_strings_are_rejected() {
        assert_eq!(
            BitReader::new(&[2, 0xC3, 0x28]).read_str(),
            Err(CodecError::InvalidUtf8)
        );
        assert_eq!(
            BitReader::new(&[2, b'a']).read_str(),
            Err(CodecError::UnexpectedEnd)
        );
        assert_eq!(
            BitReader::new(&[]).read_str(),
            Err(CodecError::UnexpectedEnd)
        );
        assert_eq!(
            BitReader::new(&[255]).read_str(),
            Err(CodecError::UnexpectedEnd)
        );
        // A lone surrogate encoding.
        assert_eq!(
            BitReader::new(&[3, 0xED, 0xA0, 0x80]).read_str(),
            Err(CodecError::InvalidUtf8)
        );
    }
}
