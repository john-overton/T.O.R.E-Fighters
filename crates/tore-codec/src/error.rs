use std::fmt;

/// Why a read or a write failed.
///
/// Readers return these for any bytes they are given; they never panic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecError {
    /// The data ended before the field did.
    UnexpectedEnd,
    /// A bit width outside what the call allows (for example more than 64).
    InvalidWidth,
    /// A value that does not fit the field it was given.
    ValueOutOfRange,
    /// A variable-length integer with more groups than a 64-bit value needs.
    VarintTooLong,
    /// A variable-length integer whose value does not fit in 64 bits.
    VarintOverflow,
    /// A valid but not canonical form: the writer never produces it, so the
    /// data is damaged or from another program.
    NonCanonical,
    /// A bucket ladder that is empty of sense: widths must be strictly
    /// ascending and each from 1 to 64.
    InvalidLadder,
    /// A bucket index beyond the ladder and its escape.
    BadBucket,
    /// A string longer than 255 bytes.
    StringTooLong,
    /// A string that is not valid UTF-8.
    InvalidUtf8,
    /// An exclusive-or record whose counts do not fit in 64 bits.
    BadXor,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::UnexpectedEnd => "the data ended before the field did",
            Self::InvalidWidth => "bit width out of range",
            Self::ValueOutOfRange => "value does not fit its field",
            Self::VarintTooLong => "variable-length integer is too long",
            Self::VarintOverflow => "variable-length integer overflows 64 bits",
            Self::NonCanonical => "non-canonical encoding",
            Self::InvalidLadder => "bucket ladder widths must ascend from 1 to 64",
            Self::BadBucket => "bucket index beyond the ladder",
            Self::StringTooLong => "string is longer than 255 bytes",
            Self::InvalidUtf8 => "string is not valid UTF-8",
            Self::BadXor => "exclusive-or record counts exceed 64 bits",
        };
        f.write_str(text)
    }
}

impl std::error::Error for CodecError {}
