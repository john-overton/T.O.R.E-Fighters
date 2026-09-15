//! Inert active FA selector tables, with a reviewed executable fingerprint.
use crate::{Result, invalid, slice, u16_at, u32_at};

// Exact list cardinalities for the fingerprinted active selector contract.
const COUNTS: [usize; 49] = [
    0, 0, 0, 60, 6, 4, 0, 6, 4, 0, 6, 4, 0, 16, 4, 7, 3, 6, 2, 2, 60, 6, 4, 0, 6, 4, 0, 6, 4, 0, 0,
    4, 4, 9, 7, 8, 7, 8, 6, 9, 8, 10, 7, 7, 7, 7, 7, 9, 8,
];

#[derive(Clone, Debug)]
pub struct Options {
    pub fields: Vec<Vec<String>>,
    pub targets: Vec<Vec<String>>,
}
impl Options {
    /// App/CLI cache contains only inert UTF-8 option lists, never the executable.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = b"TOREQM01".to_vec();
        for list in self.fields.iter().chain(&self.targets) {
            bytes.extend_from_slice(&(list.len() as u16).to_le_bytes());
            for text in list {
                bytes.extend_from_slice(&(text.len() as u16).to_le_bytes());
                bytes.extend_from_slice(text.as_bytes());
            }
        }
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65536 || slice(bytes, 0, 8)? != b"TOREQM01" {
            return Err(invalid("invalid creator cache"));
        }
        let mut at = 8;
        let mut lists = Vec::new();
        for _ in 0..49 {
            let count = u16_at(bytes, at)?;
            at += 2;
            if count > 256 {
                return Err(invalid("creator list too long"));
            }
            let mut list = Vec::new();
            for _ in 0..count {
                let len = u16_at(bytes, at)?;
                at += 2;
                if len == 0 || len > 160 {
                    return Err(invalid("creator text length"));
                }
                let text = slice(bytes, at, len)?;
                at += len;
                if !text.iter().all(|c| (32..=126).contains(c)) {
                    return Err(invalid("creator text encoding"));
                }
                list.push(String::from_utf8(text.to_vec()).unwrap());
            }
            lists.push(list);
        }
        if at != bytes.len() {
            return Err(invalid("trailing creator data"));
        }
        for (id, list) in lists.iter().enumerate() {
            if list.len() != COUNTS[id] {
                return Err(invalid("creator field contract"));
            }
        }
        let targets = lists.split_off(33);
        Ok(Self {
            fields: lists,
            targets,
        })
    }

    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() > 16 * 1024 * 1024
            || crate::ui::fingerprint::sha256(data)
                != "e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c"
        {
            return Err(invalid("creator tables require the reviewed FA.EXE build"));
        }
        let image = Image::parse(data)?;
        let dispatch = image.read(0x42e86c, 240, true)?;
        let mut fields = vec![Vec::new(); 33];
        for (id, field) in fields.iter_mut().enumerate().skip(3) {
            let branch = u32_at(dispatch, (id - 3) * 4)?;
            if matches!(branch, 0x42e747 | 0x42e799) {
                continue;
            }
            *field = image.list(branch)?;
        }
        let dispatch = image.read(0x42e95c, 64, true)?;
        let targets = (0..16)
            .map(|i| image.list(u32_at(dispatch, i * 4)?))
            .collect::<Result<_>>()?;
        Ok(Self { fields, targets })
    }
}
struct Image<'a> {
    data: &'a [u8],
    sections: Vec<(usize, usize, usize, bool)>,
}
impl<'a> Image<'a> {
    fn parse(data: &'a [u8]) -> Result<Self> {
        if slice(data, 0, 2)? != b"MZ" {
            return Err(invalid("missing MZ"));
        }
        let pe = u32_at(data, 60)?;
        if slice(data, pe, 4)? != b"PE\0\0" || u16_at(data, pe + 4)? != 0x14c {
            return Err(invalid("expected i386 PE"));
        }
        let count = u16_at(data, pe + 6)?;
        let optional = u16_at(data, pe + 20)?;
        if !(1..=32).contains(&count) || optional < 96 || u16_at(data, pe + 24)? != 0x10b {
            return Err(invalid("invalid PE header"));
        }
        let base = u32_at(data, pe + 52)?;
        let mut sections = Vec::new();
        for i in 0..count {
            let s = slice(data, pe + 24 + optional + i * 40, 40)?;
            let va = base
                .checked_add(u32_at(s, 12)?)
                .ok_or_else(|| invalid("VA overflow"))?;
            let size = u32_at(s, 16)?;
            let raw = u32_at(s, 20)?;
            slice(data, raw, size)?;
            sections.push((va, size, raw, u32_at(s, 36)? & 0x20000000 != 0));
        }
        Ok(Self { data, sections })
    }
    fn read(&self, va: usize, size: usize, code: bool) -> Result<&'a [u8]> {
        if size == 0 || size > 16384 {
            return Err(invalid("table size exceeds bound"));
        }
        for &(start, len, raw, exec) in &self.sections {
            if code == exec && va >= start && va - start <= len && size <= len - (va - start) {
                return slice(self.data, raw + va - start, size);
            }
        }
        Err(invalid("table outside expected section"))
    }
    fn list(&self, branch: usize) -> Result<Vec<String>> {
        let code = self.read(branch, 6, true)?;
        if code[0] != 0xb8 || code[5] != 0xc3 {
            return Err(invalid("unreviewed constant pointer grammar"));
        }
        let va = u32_at(code, 1)?;
        let mut values = Vec::new();
        let mut item = String::new();
        for i in 0..8192 {
            let c = self.read(va + i, 1, false)?[0];
            if c == 0 {
                if item.is_empty() {
                    return if values.is_empty() {
                        Err(invalid("empty list"))
                    } else {
                        Ok(values)
                    };
                }
                values.push(std::mem::take(&mut item));
                if values.len() > 256 {
                    return Err(invalid("too many options"));
                }
            } else if !(32..=126).contains(&c) || item.len() >= 160 {
                return Err(invalid("invalid option text"));
            } else {
                item.push(char::from(c));
            }
        }
        Err(invalid("unterminated list"))
    }
}
/// Shared archive profile for creator metadata and original ordnance UI resources.
/// Does not imply all catalog objects are flyable or all stores are executable.
pub fn resource(name: &str) -> bool {
    name.ends_with(".PT")
        || name.ends_with(".JT")
        || name.starts_with('$') && name.ends_with(".PIC")
        || [
            "QUIKMIS3.PIC",
            "QUIKMISS.DLG",
            "QUICK14.DLG",
            "QM_MENU.MNU",
            "LOADORD.DLG",
            "ARMPLANE.MNU",
            "ORD_AIR3.PIC",
            "FNTWPNB.PIC",
            "FNTWPNY.PIC",
            "ARMFONT.PIC",
            "SMLFONT.PIC",
            "PANELFNT.PIC",
            "ROCKER00.PIC",
            "DIAL00.PIC",
            "DIAL04.PIC",
        ]
        .contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_rejects_truncation_trailing_and_missing_lists() {
        let mut o = Options {
            fields: COUNTS[..33]
                .iter()
                .map(|n| vec!["synthetic".into(); *n])
                .collect(),
            targets: COUNTS[33..]
                .iter()
                .map(|n| vec!["synthetic".into(); *n])
                .collect(),
        };
        for id in [0, 1, 2, 6, 9, 12, 23, 26, 29, 30] {
            o.fields[id].clear();
        }
        let b = o.encode();
        assert!(Options::decode(&b).is_ok());
        for n in 0..b.len() {
            assert!(Options::decode(&b[..n]).is_err());
        }
        let mut extra = b.clone();
        extra.push(0);
        assert!(Options::decode(&extra).is_err());
        o.targets[0].clear();
        assert!(Options::decode(&o.encode()).is_err());
    }
    #[test]
    fn bounded_pointer_lists_and_fingerprint() {
        let mut data = vec![0xb8, 0, 2, 0, 0, 0xc3];
        data.extend_from_slice(b"One\0Two\0\0");
        let image = Image {
            data: &data,
            sections: vec![(256, 6, 0, true), (512, 9, 6, false)],
        };
        assert_eq!(image.list(256).unwrap(), vec!["One", "Two"]);
        assert!(image.read(256, 6, false).is_err());
        assert!(image.read(512, 10, false).is_err());
        assert!(Options::parse(&data).is_err());
        for n in 0..9 {
            let image = Image {
                data: &data[..6 + n],
                sections: vec![(256, 6, 0, true), (512, n, 6, false)],
            };
            assert!(image.list(256).is_err());
        }
    }
}
