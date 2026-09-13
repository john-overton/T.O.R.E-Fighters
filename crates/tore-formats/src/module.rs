//! Read PL/PE sections as inert bounded data, never OS-load a retail module.
use crate::{Result, invalid, slice, u16_at, u32_at};
pub fn code(data: &[u8]) -> Result<(&[u8], usize)> {
    if data.len() > 16 * 1024 * 1024 || slice(data, 0, 2)? != b"MZ" {
        return Err(invalid("invalid module"));
    }
    let p = u32_at(data, 60)?;
    let sig = slice(data, p, 4)?;
    if sig != b"PL\0\0" && sig != b"PE\0\0" {
        return Err(invalid("invalid module signature"));
    }
    if u16_at(data, p + 4)? != 0x14c {
        return Err(invalid("not i386 data module"));
    }
    let n = u16_at(data, p + 6)?;
    let optional = u16_at(data, p + 20)?;
    if n > 32 || optional < 32 {
        return Err(invalid("invalid module sections"));
    }
    let base = u32_at(data, p + 24 + 28)?;
    for i in 0..n {
        let s = slice(data, p + 24 + optional + i * 40, 40)?;
        if &s[..4] == b"CODE" {
            let size = u32_at(s, 8)?.min(u32_at(s, 16)?);
            return Ok((slice(data, u32_at(s, 20)?, size)?, base + u32_at(s, 12)?));
        }
    }
    Err(invalid("missing CODE section"))
}

#[cfg(test)]
pub(crate) fn fixture(code: &[u8]) -> Vec<u8> {
    let mut b = vec![0; 256 + code.len()];
    b[..2].copy_from_slice(b"MZ");
    b[60..64].copy_from_slice(&64u32.to_le_bytes());
    b[64..68].copy_from_slice(b"PL\0\0");
    b[68..70].copy_from_slice(&0x14cu16.to_le_bytes());
    b[70..72].copy_from_slice(&1u16.to_le_bytes());
    b[84..86].copy_from_slice(&32u16.to_le_bytes());
    let at = 120;
    b[at..at + 4].copy_from_slice(b"CODE");
    for (off, v) in [(8, code.len()), (12, 4096), (16, code.len()), (20, 256)] {
        b[at + off..at + off + 4].copy_from_slice(&(v as u32).to_le_bytes());
    }
    b[256..].copy_from_slice(code);
    b
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_sections_are_bounded() {
        let b = fixture(&[0; 10]);
        assert_eq!(code(&b).unwrap().1, 4096);
        for n in 0..b.len() {
            assert!(code(&b[..n]).is_err());
        }
    }
}
