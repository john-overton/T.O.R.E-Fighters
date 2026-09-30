//! FNV-1a 64 and CRC-32 (IEEE), one-shot and incremental.
//!
//! FNV-1a is a quick non-cryptographic hash for state fingerprints. CRC-32 is
//! the packet checksum of the wire protocol, the same polynomial as zlib and
//! Ethernet (reflected, 0xEDB88320). Neither resists an attacker; the
//! protocol's [security section] says what they are and are not for.
//!
//! [security section]: ../../../docs/formats/net-protocol.md#security

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64 of `data`.
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut hash = Fnv1a64::new();
    hash.update(data);
    hash.finish()
}

/// Incremental FNV-1a 64. Feeding the data in any split gives the one-shot
/// result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fnv1a64 {
    state: u64,
}

impl Fnv1a64 {
    /// A hash of nothing yet.
    pub const fn new() -> Self {
        Self { state: FNV_OFFSET }
    }

    /// Adds bytes.
    pub fn update(&mut self, data: &[u8]) {
        for &byte in data {
            self.state ^= u64::from(byte);
            self.state = self.state.wrapping_mul(FNV_PRIME);
        }
    }

    /// The hash of everything added so far; more can still be added.
    pub const fn finish(&self) -> u64 {
        self.state
    }
}

impl Default for Fnv1a64 {
    fn default() -> Self {
        Self::new()
    }
}

const fn build_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

static CRC_TABLE: [u32; 256] = build_crc_table();

/// CRC-32 (IEEE) of `data`.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = Crc32::new();
    crc.update(data);
    crc.finalize()
}

/// Incremental CRC-32 (IEEE). Feeding the data in any split gives the
/// one-shot result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crc32 {
    state: u32,
}

impl Crc32 {
    /// A checksum of nothing yet.
    pub const fn new() -> Self {
        Self { state: 0xFFFF_FFFF }
    }

    /// Adds bytes.
    pub fn update(&mut self, data: &[u8]) {
        let mut crc = self.state;
        for &byte in data {
            crc = CRC_TABLE[((crc ^ u32::from(byte)) & 0xFF) as usize] ^ (crc >> 8);
        }
        self.state = crc;
    }

    /// The checksum of everything added so far; more can still be added.
    pub const fn finalize(&self) -> u32 {
        !self.state
    }
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_vectors() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414F_A339
        );
        assert_eq!(crc32(&[0u8; 32]), 0x190A_55AD);
    }

    #[test]
    fn fnv1a64_known_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn incremental_matches_one_shot_for_any_split() {
        let data: Vec<u8> = (0..300u32).map(|i| (i * 7 + 3) as u8).collect();
        for split in 0..=data.len() {
            let (a, b) = data.split_at(split);
            let mut crc = Crc32::new();
            crc.update(a);
            crc.update(b);
            assert_eq!(crc.finalize(), crc32(&data));
            let mut fnv = Fnv1a64::new();
            fnv.update(a);
            fnv.update(b);
            assert_eq!(fnv.finish(), fnv1a64(&data));
        }
        // The running value can be read in the middle.
        let mut crc = Crc32::default();
        crc.update(b"12345");
        let _ = crc.finalize();
        crc.update(b"6789");
        assert_eq!(crc.finalize(), 0xCBF4_3926);
    }
}
