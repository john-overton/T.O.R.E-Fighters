//! Seeded random round trips of mixed field sequences.

mod common;

use common::{SplitMix64, random_field, read_like, write_field};
use tore_codec::{BitReader, BitWriter};

#[test]
fn ten_thousand_mixed_sequences_round_trip() {
    let mut rng = SplitMix64(0x7012_E0F1_6417_E125);
    for case in 0..12_000 {
        let count = 1 + rng.below(24) as usize;
        let fields: Vec<_> = (0..count).map(|_| random_field(&mut rng)).collect();
        let mut w = BitWriter::new();
        for field in &fields {
            write_field(&mut w, field);
        }
        let bit_len = w.bit_len();
        let bytes = w.finish();
        assert_eq!(bytes.len(), bit_len.div_ceil(8), "case {case}");
        let mut r = BitReader::new(&bytes);
        for field in &fields {
            let back = read_like(&mut r, field)
                .unwrap_or_else(|e| panic!("case {case}: {field:?} failed with {e}: {fields:?}"));
            assert_eq!(&back, field, "case {case}");
        }
        assert_eq!(r.bit_position(), bit_len, "case {case}");
        assert!(r.only_zero_padding_left(), "case {case}");
    }
}

#[test]
fn the_same_fields_always_give_the_same_bytes() {
    let mut rng = SplitMix64(42);
    for _ in 0..200 {
        let fields: Vec<_> = (0..10).map(|_| random_field(&mut rng)).collect();
        let encode = || {
            let mut w = BitWriter::new();
            for field in &fields {
                write_field(&mut w, field);
            }
            w.finish()
        };
        assert_eq!(encode(), encode());
    }
}
