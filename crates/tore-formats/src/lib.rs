//! Bounded readers for user-owned Fighters Anthology menu resources.
//! No executable resource is ever executed. See docs/formats/menu.md.
mod dcl;
mod pic;
mod ui;
pub use pic::Pic;
use std::{collections::BTreeMap, io};
pub use ui::{Button, activity_buttons};

pub type Result<T> = io::Result<T>;
pub(crate) fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
pub(crate) fn slice(data: &[u8], start: usize, size: usize) -> Result<&[u8]> {
    data.get(
        start
            ..start
                .checked_add(size)
                .ok_or_else(|| invalid("offset overflow"))?,
    )
    .ok_or_else(|| invalid("resource truncated"))
}
pub(crate) fn u16_at(data: &[u8], at: usize) -> Result<usize> {
    Ok(u16::from_le_bytes(slice(data, at, 2)?.try_into().unwrap()) as usize)
}
pub(crate) fn u32_at(data: &[u8], at: usize) -> Result<usize> {
    Ok(u32::from_le_bytes(slice(data, at, 4)?.try_into().unwrap()) as usize)
}

#[derive(Debug)]
pub struct Entry {
    pub name: String,
    pub flag: u8,
    pub offset: usize,
    pub size: usize,
}
pub struct Archive {
    data: Vec<u8>,
    pub entries: BTreeMap<String, Entry>,
}
impl Archive {
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        if slice(&data, 0, 5)? != b"EALIB" {
            return Err(invalid("not an EALIB archive"));
        }
        let count = u16_at(&data, 5)?;
        let end = 7 + (count + 1) * 18;
        slice(&data, 0, end)?;
        let sentinel = 7 + count * 18;
        if data[sentinel..sentinel + 14].iter().any(|b| *b != 0)
            || u32_at(&data, sentinel + 14)? != data.len()
        {
            return Err(invalid("invalid archive sentinel"));
        }
        let mut entries = BTreeMap::new();
        for index in 0..count {
            let at = 7 + index * 18;
            let raw = &data[at..at + 13];
            let name = std::str::from_utf8(raw.split(|b| *b == 0).next().unwrap())
                .map_err(|_| invalid("non-ASCII archive name"))?
                .to_ascii_uppercase();
            if name.is_empty() || name.contains(['/', '\\']) || name == ".." {
                return Err(invalid("unsafe archive name"));
            }
            let offset = u32_at(&data, at + 14)?;
            let next = u32_at(&data, at + 32)?;
            if offset < end || next < offset || next > data.len() {
                return Err(invalid("archive entry outside data"));
            }
            let flag = data[at + 13];
            if !matches!(flag, 0 | 4) {
                return Err(invalid("unsupported archive compression flag"));
            }
            entries.insert(
                name.clone(),
                Entry {
                    name,
                    flag,
                    offset,
                    size: next - offset,
                },
            );
        }
        Ok(Self { data, entries })
    }
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        let entry = self
            .entries
            .get(&name.to_ascii_uppercase())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, format!("missing resource {name}"))
            })?;
        let data = slice(&self.data, entry.offset, entry.size)?;
        if entry.flag == 0 {
            if data.len() > 16 * 1024 * 1024 {
                return Err(invalid("menu resource exceeds 16 MiB"));
            }
            return Ok(data.to_vec());
        }
        dcl::explode(
            slice(data, 4, data.len().saturating_sub(4))?,
            u32_at(data, 0)?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_archives_fail_without_panicking() {
        for length in 0..80 {
            assert!(Archive::parse(vec![0; length]).is_err());
        }
    }
    #[test]
    fn stored_entry_and_sentinel_boundaries() {
        let mut data = b"EALIB\x01\x00".to_vec();
        data.extend_from_slice(b"TEST.TXT\0\0\0\0\0\0");
        data.extend_from_slice(&43_u32.to_le_bytes());
        data.extend_from_slice(&[0; 14]);
        data.extend_from_slice(&46_u32.to_le_bytes());
        data.extend_from_slice(b"abc");
        let archive = Archive::parse(data.clone()).unwrap();
        assert_eq!(archive.read("test.txt").unwrap(), b"abc");
        data[39] = 45;
        assert!(Archive::parse(data).is_err());
    }
}
