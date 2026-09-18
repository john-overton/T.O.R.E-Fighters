//! Reviewed wing-radio data only. See docs/formats/radio.md.
use crate::{Result, invalid, slice, u16_at, u32_at};
use std::collections::BTreeMap;

pub const STEMS: &[(&str, usize)] = &[
    ("^BREAKRT", 0x4ff170),
    ("^BREAKLF", 0x4ff178),
    ("^BREAKHI", 0x4ff180),
    ("^BREAKLO", 0x4ff188),
    ("^STEADY", 0x4ff190),
    ("^APPRCRT", 0x4ff198),
    ("^APPRCLF", 0x4ff1a0),
    ("^APPRCHI", 0x4ff1a8),
    ("^APPRCLO", 0x4ff1b0),
    ("^TIGHTEN", 0x4ff1c0),
    ("^CBTSPRD", 0x4ff1c8),
    ("^FORMHI", 0x4ff1d0),
    ("^FORMLVL", 0x4ff1d8),
    ("^FORMLOW", 0x4ff1e0),
    ("^ECHFORM", 0x4ff1e8),
    ("^ABRFORM", 0x4ff1f0),
    ("^ASTFORM", 0x4ff1f8),
    ("^LOSFORM", 0x4ff200),
    ("^MEDFORM", 0x4ff208),
    ("^DISENG", 0x4ff218),
    ("^CLRMY6", 0x4ff220),
    ("^ATTACK", 0x4ff230),
    ("^ENGAGE", 0x4ff3d8),
    ("^SHWTIME", 0x4ff3e8),
];

pub fn resource(name: &str) -> bool {
    name.strip_suffix(".5K")
        .is_some_and(|stem| STEMS.iter().any(|(s, _)| *s == stem))
}

/// Select bounded inert records from the reviewed executable layout.
/// Expected stems are layout guards, not substitute metadata.
pub fn phrases(data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    if data.len() > 16 * 1024 * 1024 || slice(data, 0, 2)? != b"MZ" {
        return Err(invalid("invalid radio image"));
    }
    let pe = u32_at(data, 60)?;
    if slice(data, pe, 4)? != b"PE\0\0" || u16_at(data, pe + 4)? != 0x14c {
        return Err(invalid("unsupported radio image"));
    }
    let count = u16_at(data, pe + 6)?;
    let optional = u16_at(data, pe + 20)?;
    if count > 32 || optional < 32 || u32_at(data, pe + 52)? != 0x400000 {
        return Err(invalid("unsupported radio layout"));
    }
    let mut section = None;
    for i in 0..count {
        let s = slice(data, pe + 24 + optional + i * 40, 40)?;
        if &s[..8] == b".data\0\0\0" {
            if section.is_some() {
                return Err(invalid("duplicate radio section"));
            }
            section = Some((
                slice(data, u32_at(s, 20)?, u32_at(s, 16)?.min(u32_at(s, 8)?))?,
                0x400000 + u32_at(s, 12)?,
            ));
        }
    }
    let (bytes, base) = section.ok_or_else(|| invalid("missing radio section"))?;
    let offset = |va: usize| {
        va.checked_sub(base)
            .ok_or_else(|| invalid("radio pointer outside section"))
    };
    let string = |va: usize| -> Result<&[u8]> {
        let start = offset(va)?;
        let rest = bytes
            .get(start..)
            .ok_or_else(|| invalid("radio pointer outside section"))?;
        let end = rest
            .iter()
            .take(128)
            .position(|b| *b == 0)
            .ok_or_else(|| invalid("unterminated radio text"))?;
        let text = &rest[..end];
        if text.is_empty() || !text.iter().all(|b| (32..127).contains(b)) {
            return Err(invalid("invalid radio text"));
        }
        Ok(text)
    };
    let mut result = BTreeMap::new();
    for (stem, va) in STEMS {
        let at = offset(*va)?;
        let text = string(u32_at(bytes, at)?)?;
        if string(u32_at(bytes, at + 4)?)? != stem.as_bytes() {
            return Err(invalid("unreviewed radio mapping"));
        }
        result.insert(format!("TORE_RADIO_{stem}"), text.to_vec());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut b = crate::module::fixture(&vec![0; 2048]);
        b[64..68].copy_from_slice(b"PE\0\0");
        b[116..120].copy_from_slice(&0x400000u32.to_le_bytes());
        b[120..128].copy_from_slice(b".data\0\0\0");
        b[132..136].copy_from_slice(&0xff000u32.to_le_bytes());
        let mut pos = 1100;
        for (stem, va) in STEMS {
            let at = 256 + va - 0x4ff000;
            for (slot, text) in [(0, "Synthetic phrase"), (4, *stem)] {
                b[at + slot..at + slot + 4]
                    .copy_from_slice(&(0x4ff000u32 + pos as u32).to_le_bytes());
                b[256 + pos..256 + pos + text.len()].copy_from_slice(text.as_bytes());
                pos += text.len() + 1;
            }
        }
        b
    }
    #[test]
    fn bounded_metadata_and_layout_guards() {
        let b = fixture();
        assert_eq!(phrases(&b).unwrap().len(), STEMS.len());
        for n in 0..b.len() {
            assert!(phrases(&b[..n]).is_err());
        }
        let mut bad = b.clone();
        bad[256 + 0x170..256 + 0x174].fill(255);
        assert!(phrases(&bad).is_err());
        let mut bad = b;
        bad[256 + 1100] = 0;
        assert!(phrases(&bad).is_err());
        assert!(!resource("^UNKNOWN.5K"));
        assert!(resource("^ENGAGE.5K"));
    }
}
