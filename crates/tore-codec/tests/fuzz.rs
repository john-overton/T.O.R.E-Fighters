//! The bounded reader never panics, whatever the bytes.

mod common;

use common::{LADDERS, SplitMix64, random_field, read_like, write_field};
use tore_codec::{BitReader, BitWriter};

/// Runs a random sequence of reads over `bytes`. Returns how many reads
/// succeeded. Any error just stops the sequence; a panic fails the test.
fn read_randomly(bytes: &[u8], rng: &mut SplitMix64) -> usize {
    let mut r = BitReader::new(bytes);
    let mut ok = 0;
    for _ in 0..40 {
        let before = r.bits_remaining();
        let result = match rng.below(14) {
            0 => r.read_bits(rng.below(70) as u32).map(drop),
            1 => r.read_bool().map(drop),
            2 => r.read_signed(rng.below(70) as u32).map(drop),
            3 => r.read_varint().map(drop),
            4 => r.read_varint_signed().map(drop),
            5 => r
                .read_bucketed(LADDERS[rng.below(LADDERS.len() as u64) as usize])
                .map(drop),
            6 => r.read_u64_xor(rng.next_u64()).map(drop),
            7 => r.read_f64_xor(f64::from_bits(rng.next_u64())).map(drop),
            8 => r.read_str().map(drop),
            9 => r.read_bytes(rng.edgy_u64() as usize % 600).map(drop),
            10 => r.read_bytes(rng.edgy_u64() as usize).map(drop),
            11 => r.align_strict(),
            12 => {
                r.align();
                Ok(())
            }
            _ => {
                let _ = r.only_zero_padding_left();
                Ok(())
            }
        };
        assert!(r.bit_position() <= bytes.len() * 8);
        assert_eq!(r.bit_position() + r.bits_remaining(), bytes.len() * 8);
        assert!(r.bits_remaining() <= before, "reads never move backward");
        match result {
            Ok(()) => ok += 1,
            Err(_) => break,
        }
    }
    ok
}

#[test]
fn hundred_thousand_random_and_truncated_inputs_never_panic() {
    let mut rng = SplitMix64(0xF022_5EED_0000_0001);
    let mut successes = 0usize;
    for case in 0..100_000u32 {
        let bytes: Vec<u8> = match case % 4 {
            // Pure random bytes of random length.
            0 => (0..rng.below(80)).map(|_| rng.next_u64() as u8).collect(),
            // Random bytes with long runs of ones or zeros.
            1 => {
                let fill = if rng.below(2) == 0 { 0x00 } else { 0xFF };
                (0..rng.below(40))
                    .map(|_| {
                        if rng.below(5) == 0 {
                            rng.next_u64() as u8
                        } else {
                            fill
                        }
                    })
                    .collect()
            }
            // A valid encoding, truncated at a random point.
            2 => {
                let mut w = BitWriter::new();
                for _ in 0..1 + rng.below(12) {
                    write_field(&mut w, &random_field(&mut rng));
                }
                let mut bytes = w.finish();
                let keep = rng.below(bytes.len() as u64 + 1) as usize;
                bytes.truncate(keep);
                bytes
            }
            // A valid encoding with a few bytes flipped.
            _ => {
                let mut w = BitWriter::new();
                for _ in 0..1 + rng.below(12) {
                    write_field(&mut w, &random_field(&mut rng));
                }
                let mut bytes = w.finish();
                for _ in 0..1 + rng.below(3) {
                    if !bytes.is_empty() {
                        let at = rng.below(bytes.len() as u64) as usize;
                        bytes[at] ^= 1 << rng.below(8);
                    }
                }
                bytes
            }
        };
        successes += read_randomly(&bytes, &mut rng);
    }
    // Sanity: the fuzz reaches successful reads too, not only early errors.
    assert!(successes > 100_000, "only {successes} reads succeeded");
}

#[test]
fn truncating_a_valid_encoding_at_every_length_gives_errors_or_values() {
    let mut rng = SplitMix64(7);
    for _ in 0..500 {
        let fields: Vec<_> = (0..10).map(|_| random_field(&mut rng)).collect();
        let mut w = BitWriter::new();
        for field in &fields {
            write_field(&mut w, field);
        }
        let bytes = w.finish();
        for keep in 0..bytes.len() {
            let mut r = BitReader::new(&bytes[..keep]);
            let mut failed = false;
            for field in &fields {
                if read_like(&mut r, field).is_err() {
                    failed = true;
                    break;
                }
            }
            // Fewer bytes than the full encoding cannot hold every field
            // unless the tail was only zero padding; either way: no panic.
            let _ = failed;
        }
    }
}

#[test]
fn every_single_byte_and_bit_shift_reads_safely() {
    let mut rng = SplitMix64(9);
    for first in 0..=255u8 {
        for second in [0u8, 0x80, 0xFF, 0x01] {
            let bytes = [
                first, second, first, second, first, second, first, second, first, second, 0xFF,
            ];
            for _ in 0..20 {
                read_randomly(&bytes, &mut rng);
            }
        }
    }
}
