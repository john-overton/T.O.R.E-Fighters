//! Reviewed `FA.EXE` build identities and their inert-table address sets.
//! Evidence: docs/formats/esa-installer.md. Work package B owns this file.
//!
//! Two builds are reviewed: the 1.0 executable that ships on the retail disc
//! and the 1.02F executable a patched installation carries. They hold the same
//! creator, cloud, lens-flare and radio tables at shifted addresses, so the
//! readers select an address set by fingerprint instead of carrying one delta.
//! Nothing here loads or runs the image; every address is a bounded read.
use crate::{Result, invalid};

/// A reviewed `FA.EXE` build. A third build is a new research pass, not a guess.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Build {
    /// 1.0, as packed in `disc1/SETUP.ESA` on the retail disc.
    Disc10,
    /// 1.02F, as installed by the retail patch.
    Patch102F,
}

/// Address set for one reviewed build. All values are virtual addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub build: Build,
    /// Name for the import report: `1.0 (disc)` or `1.02F`.
    pub name: &'static str,
    pub sha256: &'static str,
    pub size: usize,
    /// Creator field dispatch, 30 words of branch addresses.
    pub creator_fields: usize,
    /// Branch addresses that mean "this creator field has no list".
    pub creator_sentinels: [usize; 2],
    /// Creator target dispatch, 16 words of branch addresses.
    pub creator_targets: usize,
    /// The 7-byte cloud repeat call that carries period and subdivisions.
    pub cloud_call: usize,
    /// First of the 26-byte cloud placement records.
    pub cloud_records: usize,
    /// First of the 12-byte lens-flare descriptors.
    pub flare_records: usize,
    /// Subtract this from a [`crate::radio::STEMS`] address, which is 1.02F.
    pub radio_shift: usize,
}

/// Every reviewed build. Code moved by `0x4d0` in the creator region between
/// the two; `.data` moved by `0x4710` for the cloud records and `0x4608` for
/// the radio pointer pairs, so each table carries its own address rather than
/// one shared delta.
pub const LAYOUTS: [Layout; 2] = [
    Layout {
        build: Build::Disc10,
        name: "1.0 (disc)",
        sha256: "c7d2c1cc9d27a6b364eca4245892cb6ca61ca72afc9a46e5760ee1fe7d75ba9b",
        size: 1_299_968,
        creator_fields: 0x42e39c,
        creator_sentinels: [0x42e277, 0x42e2c9],
        creator_targets: 0x42e48c,
        cloud_call: 0x4a5a4a,
        cloud_records: 0x507b88,
        flare_records: 0x508190,
        radio_shift: 0x4608,
    },
    Layout {
        build: Build::Patch102F,
        name: "1.02F",
        sha256: "e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c",
        size: 1_319_424,
        creator_fields: 0x42e86c,
        creator_sentinels: [0x42e747, 0x42e799],
        creator_targets: 0x42e95c,
        cloud_call: 0x4a8bda,
        cloud_records: 0x50c298,
        flare_records: 0x50c8d8,
        radio_shift: 0,
    },
];

/// SHA-256 of a byte slice, for naming a source build. Not authentication.
pub fn sha256(data: &[u8]) -> String {
    crate::ui::fingerprint::sha256(data)
}

/// Identify a user-supplied `FA.EXE`. Unknown builds are refused, never guessed.
pub fn identify(exe: &[u8]) -> Result<&'static Layout> {
    if exe.len() > 16 * 1024 * 1024 {
        return Err(invalid("FA.EXE exceeds the reviewed 16 MiB bound"));
    }
    let hash = sha256(exe);
    LAYOUTS.iter().find(|l| l.sha256 == hash).ok_or_else(|| {
        invalid(&format!(
            "FA.EXE build {hash} has not been reviewed; only 1.0 (disc) and 1.02F are"
        ))
    })
}

/// Build a synthetic i386 PE holding the given sections, for reader tests.
/// Each section is `(name, virtual address, contents, executable)`.
#[cfg(test)]
pub(crate) fn fixture(sections: &[(&str, usize, Vec<u8>, bool)]) -> Vec<u8> {
    const BASE: usize = 0x400000;
    const PE: usize = 128;
    const OPTIONAL: usize = 224;
    let table = PE + 24 + OPTIONAL;
    let mut image = vec![0u8; (table + 40 * sections.len()).div_ceil(512) * 512];
    image[..2].copy_from_slice(b"MZ");
    image[60..64].copy_from_slice(&(PE as u32).to_le_bytes());
    image[PE..PE + 4].copy_from_slice(b"PE\0\0");
    image[PE + 4..PE + 6].copy_from_slice(&0x14cu16.to_le_bytes());
    image[PE + 6..PE + 8].copy_from_slice(&(sections.len() as u16).to_le_bytes());
    image[PE + 20..PE + 22].copy_from_slice(&(OPTIONAL as u16).to_le_bytes());
    image[PE + 24..PE + 26].copy_from_slice(&0x10bu16.to_le_bytes());
    image[PE + 52..PE + 56].copy_from_slice(&(BASE as u32).to_le_bytes());
    for (index, (name, va, bytes, code)) in sections.iter().enumerate() {
        let at = table + index * 40;
        image[at..at + name.len()].copy_from_slice(name.as_bytes());
        let raw = image.len();
        let size = bytes.len().div_ceil(512) * 512;
        for (offset, value) in [(8, bytes.len()), (12, va - BASE), (16, size), (20, raw)] {
            image[at + offset..at + offset + 4].copy_from_slice(&(value as u32).to_le_bytes());
        }
        if *code {
            image[at + 36..at + 40].copy_from_slice(&0x20000000u32.to_le_bytes());
        }
        image.resize(raw + size, 0);
        image[raw..raw + bytes.len()].copy_from_slice(bytes);
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_builds_are_named_and_refused() {
        let error = identify(b"not an executable").unwrap_err().to_string();
        assert!(error.contains(&sha256(b"not an executable")));
        assert!(error.contains("has not been reviewed"));
        assert!(identify(&vec![0; 17 * 1024 * 1024]).is_err());
    }
    #[test]
    fn reviewed_builds_are_distinct_and_shifted_uniformly() {
        let [disc, patch] = LAYOUTS;
        assert_eq!(disc.name, "1.0 (disc)");
        assert_eq!(patch.name, "1.02F");
        assert_ne!(disc.sha256, patch.sha256);
        assert_eq!(patch.creator_fields - disc.creator_fields, 0x4d0);
        assert_eq!(patch.creator_targets - disc.creator_targets, 0x4d0);
        // Only the creator region moved uniformly; the cloud call did not.
        assert!(patch.cloud_call > disc.cloud_call);
        for (a, b) in patch.creator_sentinels.iter().zip(&disc.creator_sentinels) {
            assert_eq!(a - b, 0x4d0);
        }
        assert_eq!(patch.cloud_records - disc.cloud_records, 0x4710);
        // The flare descriptors sit 0x38 further apart than the cloud records.
        assert_eq!(patch.flare_records - disc.flare_records, 0x4748);
        assert_eq!(disc.radio_shift, 0x4608);
        assert_eq!(patch.radio_shift, 0);
    }
    #[test]
    fn synthetic_sections_round_trip_through_the_pe_reader() {
        let image = fixture(&[
            ("CODE", 0x401000, vec![1, 2, 3, 4], true),
            (".data", 0x500000, vec![5, 6, 7, 8], false),
        ]);
        let pe = crate::ui::creator::Image::parse(&image).unwrap();
        assert_eq!(pe.read(0x401000, 4, true).unwrap(), [1, 2, 3, 4]);
        assert_eq!(pe.read(0x500000, 4, false).unwrap(), [5, 6, 7, 8]);
        assert!(pe.read(0x500000, 4, true).is_err());
    }
}
