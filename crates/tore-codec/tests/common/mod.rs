//! Shared by the integration tests: a seeded generator and a random field
//! sequence that can be written, read back and fed to a reader.

#![allow(dead_code)]

use tore_codec::{BitReader, BitWriter, CodecError};

/// SplitMix64, so the tests need no dependency.
pub struct SplitMix64(pub u64);

impl SplitMix64 {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 0 up to but not including `n`; `n` must be above zero.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    /// A value biased toward the edges, so edge cases come up often.
    pub fn edgy_u64(&mut self) -> u64 {
        match self.below(6) {
            0 => 0,
            1 => u64::MAX,
            2 => 1 << self.below(64),
            3 => (1u64 << self.below(64)).wrapping_sub(1),
            4 => self.next_u64() >> self.below(64),
            _ => self.next_u64(),
        }
    }
}

pub const LADDERS: [&[u32]; 5] = [
    &[4, 8, 16, 32],
    &[],
    &[1],
    &[7, 13, 64],
    &[2, 3, 5, 9, 17, 33],
];

/// One field of a random sequence.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Bits(u64, u32),
    Bool(bool),
    Signed(i64, u32),
    Varint(u64),
    VarintSigned(i64),
    Bucketed(i64, usize),
    Xor(u64, u64),
    Str(String),
    Bytes(Vec<u8>),
    Align,
}

pub fn random_field(rng: &mut SplitMix64) -> Field {
    match rng.below(10) {
        0 => {
            let bits = rng.below(65) as u32;
            let mask = if bits == 64 {
                u64::MAX
            } else {
                (1u64 << bits) - 1
            };
            Field::Bits(rng.edgy_u64() & mask, bits)
        }
        1 => Field::Bool(rng.below(2) == 1),
        2 => {
            let bits = 1 + rng.below(64) as u32;
            let value = (rng.edgy_u64() as i64) >> (64 - bits);
            Field::Signed(value, bits)
        }
        3 => Field::Varint(rng.edgy_u64()),
        4 => Field::VarintSigned(rng.edgy_u64() as i64),
        5 => {
            let ladder = rng.below(LADDERS.len() as u64) as usize;
            let shift = rng.below(64);
            Field::Bucketed((rng.edgy_u64() as i64) >> shift, ladder)
        }
        6 => {
            let base = rng.edgy_u64();
            let value = match rng.below(3) {
                0 => base,
                1 => base ^ (1 << rng.below(64)),
                _ => rng.edgy_u64(),
            };
            Field::Xor(value, base)
        }
        7 => {
            let len = rng.below(40) as usize;
            let text: String = (0..len)
                .map(|_| match rng.below(4) {
                    0 => 'a',
                    1 => 'é',
                    2 => '日',
                    _ => '\u{1F6E9}',
                })
                .collect();
            if text.len() > 255 {
                Field::Str(String::new())
            } else {
                Field::Str(text)
            }
        }
        8 => {
            let len = rng.below(20) as usize;
            Field::Bytes((0..len).map(|_| rng.next_u64() as u8).collect())
        }
        _ => Field::Align,
    }
}

pub fn write_field(w: &mut BitWriter, field: &Field) {
    match field {
        Field::Bits(v, b) => w.write_bits(*v, *b).unwrap(),
        Field::Bool(v) => w.write_bool(*v),
        Field::Signed(v, b) => w.write_signed(*v, *b).unwrap(),
        Field::Varint(v) => w.write_varint(*v),
        Field::VarintSigned(v) => w.write_varint_signed(*v),
        Field::Bucketed(v, l) => w.write_bucketed(*v, LADDERS[*l]).unwrap(),
        Field::Xor(v, b) => w.write_u64_xor(*v, *b),
        Field::Str(s) => w.write_str(s).unwrap(),
        Field::Bytes(b) => w.write_bytes(b),
        Field::Align => w.align(),
    }
}

/// Reads the field the shape of `like` describes and returns what it read.
pub fn read_like(r: &mut BitReader<'_>, like: &Field) -> Result<Field, CodecError> {
    Ok(match like {
        Field::Bits(_, b) => Field::Bits(r.read_bits(*b)?, *b),
        Field::Bool(_) => Field::Bool(r.read_bool()?),
        Field::Signed(_, b) => Field::Signed(r.read_signed(*b)?, *b),
        Field::Varint(_) => Field::Varint(r.read_varint()?),
        Field::VarintSigned(_) => Field::VarintSigned(r.read_varint_signed()?),
        Field::Bucketed(_, l) => Field::Bucketed(r.read_bucketed(LADDERS[*l])?, *l),
        Field::Xor(_, b) => Field::Xor(r.read_u64_xor(*b)?, *b),
        Field::Str(_) => Field::Str(r.read_str()?),
        Field::Bytes(b) => Field::Bytes(r.read_bytes(b.len())?),
        Field::Align => {
            r.align_strict()?;
            Field::Align
        }
    })
}
