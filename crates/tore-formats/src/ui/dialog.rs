//! Inert DLG geometry: identify draw records by relocated imported draw thunks.
//! No native thunk, widget callback or dynamic layout code is executed.
use crate::{Result, invalid, slice, u16_at, u32_at};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Dialog {
    pub origin: [i32; 2],
    pub size: [usize; 2],
    pub controls: Vec<Control>,
}

#[derive(Debug)]
pub struct Control {
    pub code_offset: usize,
    pub draw: String,
    /// Static local position; native setup may replace this before drawing.
    pub position: Option<[i32; 2]>,
    pub width: Option<usize>,
    pub action_id: Option<u8>,
    /// Literal or imported label symbol, never the thunk's machine code.
    pub label: Option<String>,
}

struct Section<'a> {
    name: &'a [u8],
    va: usize,
    bytes: &'a [u8],
}
fn at<'a>(sections: &[Section<'a>], va: usize, count: usize) -> Result<&'a [u8]> {
    for section in sections {
        if let Some(offset) = va.checked_sub(section.va)
            && let Some(bytes) = section.bytes.get(offset..offset.saturating_add(count))
        {
            return Ok(bytes);
        }
    }
    Err(invalid("dialog address outside file-backed sections"))
}
fn string(sections: &[Section<'_>], va: usize) -> Result<String> {
    let mut result = String::new();
    for i in 0..161 {
        let byte = at(sections, va + i, 1)?[0];
        if byte == 0 {
            return Ok(result);
        }
        if i == 160 || !(32..=126).contains(&byte) {
            return Err(invalid("invalid dialog string"));
        }
        result.push(char::from(byte));
    }
    Err(invalid("unterminated dialog string"))
}
fn signed_word(bytes: &[u8], offset: usize) -> Result<i32> {
    Ok(i32::from(u16_at(bytes, offset)? as i16))
}

pub fn parse(data: &[u8]) -> Result<Dialog> {
    // Apply the shared module envelope checks first.
    let (code, code_base) = crate::module::code(data)?;
    let pe = u32_at(data, 60)?;
    let optional = u16_at(data, pe + 20)?;
    if optional < 112 {
        return Err(invalid("dialog missing import directory"));
    }
    let base = u32_at(data, pe + 24 + 28)?;
    let mut sections = Vec::new();
    for i in 0..u16_at(data, pe + 6)? {
        let row = slice(data, pe + 24 + optional + i * 40, 40)?;
        sections.push(Section {
            name: &row[..8],
            va: base + u32_at(row, 12)?,
            bytes: slice(
                data,
                u32_at(row, 20)?,
                u32_at(row, 8)?.min(u32_at(row, 16)?),
            )?,
        });
    }
    let directory = base + u32_at(data, pe + 24 + 104)?;
    let directory_size = u32_at(data, pe + 24 + 108)?;
    if !(20..=1280).contains(&directory_size) {
        return Err(invalid("dialog import directory size"));
    }
    let descriptors = at(&sections, directory, directory_size)?;
    let mut imports = BTreeMap::new();
    let mut terminated = false;
    for descriptor in descriptors.chunks_exact(20) {
        if descriptor.iter().all(|b| *b == 0) {
            terminated = true;
            break;
        }
        let lookup = u32_at(descriptor, 0)?;
        let iat = u32_at(descriptor, 16)?;
        if iat == 0 {
            return Err(invalid("dialog import has no IAT"));
        }
        let lookup = base + if lookup == 0 { iat } else { lookup };
        let mut ended = false;
        for index in 0..256 {
            let name = u32_at(at(&sections, lookup + index * 4, 4)?, 0)?;
            if name == 0 {
                ended = true;
                break;
            }
            if name & 0x8000_0000 != 0 || imports.len() >= 256 {
                return Err(invalid("unsupported dialog import"));
            }
            if imports
                .insert(base + iat + index * 4, string(&sections, base + name + 2)?)
                .is_some()
            {
                return Err(invalid("overlapping dialog imports"));
            }
        }
        if !ended {
            return Err(invalid("unterminated dialog imports"));
        }
    }
    if !terminated {
        return Err(invalid("unterminated dialog import directory"));
    }
    let reloc = sections
        .iter()
        .find(|s| s.name.starts_with(b".reloc\0"))
        .ok_or_else(|| invalid("missing dialog relocations"))?
        .bytes;
    let mut cursor = 0;
    let mut controls = BTreeMap::new();
    while cursor < reloc.len() {
        let page = u32_at(reloc, cursor)?;
        let size = u32_at(reloc, cursor + 4)?;
        if size == 0 {
            if reloc[cursor..].iter().any(|b| *b != 0) {
                return Err(invalid("invalid dialog relocation terminator"));
            }
            break;
        }
        if size < 8 || size % 2 != 0 {
            return Err(invalid("invalid dialog relocation block"));
        }
        for word in slice(reloc, cursor + 8, size - 8)?.chunks_exact(2) {
            let value = u16_at(word, 0)?;
            if value >> 12 == 0 {
                continue;
            }
            if value >> 12 != 3 {
                return Err(invalid("unsupported dialog relocation"));
            }
            let slot = base + page + (value & 4095);
            let target = u32_at(at(&sections, slot, 4)?, 0)?;
            let Some(offset) = slot.checked_sub(code_base).filter(|o| *o < code.len()) else {
                continue;
            };
            // Relocations also refer to ordinary data and IAT records.
            let Ok(thunk) = at(&sections, target, 6) else {
                continue;
            };
            if thunk[..2] != [0xff, 0x25] {
                continue;
            }
            let Some(draw) = imports.get(&u32_at(thunk, 2)?) else {
                continue;
            };
            if !matches!(
                draw.as_str(),
                "_DrawAction" | "_DrawListBox" | "_DrawDial" | "_DrawRocker" | "_DrawText"
            ) {
                continue;
            }
            if controls.len() >= 256 {
                return Err(invalid("too many dialog controls"));
            }
            let record = slice(code, offset, 24)?;
            let position = (draw != "_DrawText")
                .then(|| -> Result<[i32; 2]> {
                    Ok([signed_word(record, 4)?, signed_word(record, 6)?])
                })
                .transpose()?;
            let (width, action_id, label) = if draw == "_DrawAction" {
                let label = u32_at(record, 20)?;
                let label = if let Ok(thunk) = at(&sections, label, 6)
                    && thunk[..2] == [0xff, 0x25]
                {
                    imports
                        .get(&u32_at(thunk, 2)?)
                        .cloned()
                        .ok_or_else(|| invalid("unknown action label import"))?
                } else {
                    string(&sections, label)?
                };
                (Some(u16_at(record, 18)?), Some(record[17]), Some(label))
            } else {
                (
                    if draw == "_DrawListBox" {
                        Some(u16_at(record, 8)?)
                    } else {
                        None
                    },
                    None,
                    None,
                )
            };
            if controls
                .insert(
                    offset,
                    Control {
                        code_offset: offset,
                        draw: draw.clone(),
                        position,
                        width,
                        action_id,
                        label,
                    },
                )
                .is_some()
            {
                return Err(invalid("duplicate dialog draw relocation"));
            }
        }
        cursor += size;
    }
    Ok(Dialog {
        origin: [signed_word(code, 4)?, signed_word(code, 6)?],
        size: [u16_at(code, 8)?, u16_at(code, 10)?],
        controls: controls.into_values().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(b: &mut [u8], at: usize, value: u32) {
        b[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; 972];
        b[..2].copy_from_slice(b"MZ");
        put(&mut b, 60, 64);
        b[64..68].copy_from_slice(b"PL\0\0");
        b[68..70].copy_from_slice(&0x14cu16.to_le_bytes());
        b[70..72].copy_from_slice(&3u16.to_le_bytes());
        b[84..86].copy_from_slice(&224u16.to_le_bytes());
        put(&mut b, 192, 0x2000);
        put(&mut b, 196, 40);
        for (i, name, va, raw, size) in [
            (0, b"CODE\0\0\0\0", 0x1000, 512, 160),
            (1, b".idata\0\0", 0x2000, 768, 192),
            (2, b".reloc\0\0", 0x3000, 960, 12),
        ] {
            let s = 312 + i * 40;
            b[s..s + 8].copy_from_slice(name);
            for (off, v) in [(8, size), (12, va), (16, size), (20, raw)] {
                put(&mut b, s + off, v);
            }
        }
        for (off, v) in [(4, 10u16), (6, 20), (8, 200), (10, 100)] {
            b[512 + off..514 + off].copy_from_slice(&v.to_le_bytes());
        }
        put(&mut b, 533, 0x1080);
        b[537..539].copy_from_slice(&30u16.to_le_bytes());
        b[539..541].copy_from_slice(&40u16.to_le_bytes());
        b[550] = 7;
        b[551..553].copy_from_slice(&80u16.to_le_bytes());
        put(&mut b, 553, 0x1060);
        b[608..613].copy_from_slice(b"Test\0");
        b[640..642].copy_from_slice(&[0xff, 0x25]);
        put(&mut b, 642, 0x2050);
        put(&mut b, 768, 0x2040);
        put(&mut b, 784, 0x2050);
        put(&mut b, 832, 0x2070);
        put(&mut b, 848, 0x2070);
        b[882..894].copy_from_slice(b"_DrawAction\0");
        put(&mut b, 960, 0x1000);
        put(&mut b, 964, 12);
        b[968..970].copy_from_slice(&0x3015u16.to_le_bytes());
        b
    }
    #[test]
    fn resolves_draw_import_and_static_action_geometry_without_execution() {
        let b = fixture();
        let d = parse(&b).unwrap();
        assert_eq!(d.origin, [10, 20]);
        assert_eq!(d.size, [200, 100]);
        assert_eq!(d.controls.len(), 1);
        let c = &d.controls[0];
        assert_eq!(c.position, Some([30, 40]));
        assert_eq!(c.width, Some(80));
        assert_eq!(c.action_id, Some(7));
        assert_eq!(c.label.as_deref(), Some("Test"));
        for length in 0..b.len() {
            assert!(parse(&b[..length]).is_err(), "length {length}");
        }
    }
    #[test]
    fn rejects_bad_import_label_and_relocation_records() {
        let b = fixture();
        for (offset, value) in [(832, 0x80000001), (553, 0xfffffff0), (964, 7)] {
            let mut bad = b.clone();
            put(&mut bad, offset, value);
            assert!(parse(&bad).is_err());
        }
        let mut duplicate = b.clone();
        duplicate[970..972].copy_from_slice(&0x3015u16.to_le_bytes());
        assert!(parse(&duplicate).is_err());
    }
}
