//! PKWare DCL raw-literal mode, the mode used by the inspected FA archives.
//! Canonical length/distance alphabets are format constants (see provenance docs).
//! Altered implementation based on Mark Adler's blast; see THIRD_PARTY_NOTICES.md.
use crate::{Result, invalid};
use std::sync::OnceLock;
struct Bits<'a> {
    data: &'a [u8],
    position: usize,
}
impl Bits<'_> {
    fn read(&mut self, count: usize) -> Result<usize> {
        if self.position + count > self.data.len() * 8 {
            return Err(invalid("truncated DCL bitstream"));
        }
        let mut value = 0;
        for shift in 0..count {
            value |=
                (((self.data[self.position / 8] >> (self.position % 8)) & 1) as usize) << shift;
            self.position += 1;
        }
        Ok(value)
    }
}
struct Alphabet {
    counts: [usize; 14],
    symbols: Vec<usize>,
}
impl Alphabet {
    fn new(runs: &[u8]) -> Self {
        let lengths: Vec<usize> = runs
            .iter()
            .flat_map(|b| std::iter::repeat_n((b & 15) as usize, (b >> 4) as usize + 1))
            .collect();
        let mut counts = [0; 14];
        for length in &lengths {
            counts[*length] += 1;
        }
        let symbols = (1..14)
            .flat_map(|length| {
                lengths
                    .iter()
                    .enumerate()
                    .filter_map(move |(symbol, n)| (*n == length).then_some(symbol))
            })
            .collect();
        Self { counts, symbols }
    }
    fn decode(&self, bits: &mut Bits<'_>) -> Result<usize> {
        let (mut code, mut first, mut index) = (0, 0, 0);
        for length in 1..14 {
            code |= bits.read(1)? ^ 1;
            let count = self.counts[length];
            if code >= first && code < first + count {
                return Ok(self.symbols[index + code - first]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err(invalid("invalid DCL Huffman code"))
    }
}
pub fn explode(data: &[u8], expected: usize) -> Result<Vec<u8>> {
    if data.len() < 2 || data[0] != 0 || !(4..=6).contains(&data[1]) {
        return Err(invalid(
            "unsupported DCL header (only raw literals supported)",
        ));
    }
    if expected > 16 * 1024 * 1024 {
        return Err(invalid("DCL output exceeds 16 MiB"));
    }
    static TABLES: OnceLock<(Alphabet, Alphabet)> = OnceLock::new();
    let (lengths, distances) = TABLES.get_or_init(|| {
        (
            Alphabet::new(&[2, 35, 36, 53, 38, 23]),
            Alphabet::new(&[2, 20, 53, 230, 247, 151, 248]),
        )
    });
    let base = [3, 2, 4, 5, 6, 7, 8, 9, 10, 12, 16, 24, 40, 72, 136, 264];
    let extra = [0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8];
    let mut bits = Bits { data, position: 16 };
    let mut out = Vec::with_capacity(expected);
    loop {
        if bits.read(1)? == 0 {
            if out.len() == expected {
                return Err(invalid("DCL output exceeds declared size"));
            }
            out.push(bits.read(8)? as u8);
        } else {
            let symbol = lengths.decode(&mut bits)?;
            let length = base[symbol] + bits.read(extra[symbol])?;
            if length == 519 {
                break;
            }
            let distance_bits = if length == 2 { 2 } else { data[1] as usize };
            let distance =
                (distances.decode(&mut bits)? << distance_bits) + bits.read(distance_bits)? + 1;
            if distance > out.len() || out.len() + length > expected {
                return Err(invalid("DCL back-reference outside output"));
            }
            for _ in 0..length {
                out.push(out[out.len() - distance]);
            }
        }
    }
    if out.len() != expected {
        return Err(invalid("DCL output size mismatch"));
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_dcl_vector_and_truncations() {
        // Public blast format example; synthetic text, no game bytes.
        let data = [0, 4, 0x82, 0x24, 0x25, 0x8f, 0x80, 0x7f];
        assert_eq!(explode(&data, 13).unwrap(), b"AIAIAIAIAIAIA");
        for end in 0..data.len() {
            assert!(explode(&data[..end], 13).is_err());
        }
        assert!(explode(&data, 12).is_err());
        assert!(explode(&data, 14).is_err());
        assert!(explode(&data, usize::MAX).is_err());
    }
}
