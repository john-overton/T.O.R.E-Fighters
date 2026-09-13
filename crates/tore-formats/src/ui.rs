use crate::{Result, invalid, slice, u16_at, u32_at};
#[derive(Clone, Debug)]
pub struct Button {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub label: String,
}

/// Narrow CHOOSEAC reader. Resolve label relocations, never execute PL code.
pub fn activity_buttons(data: &[u8]) -> Result<Vec<Button>> {
    if slice(data, 0, 2)? != b"MZ" {
        return Err(invalid("DLG is not MZ"));
    }
    let pe = u32_at(data, 60)?;
    if !matches!(slice(data, pe, 4)?, b"PL\0\0" | b"PE\0\0") || u16_at(data, pe + 4)? != 0x14c {
        return Err(invalid("unsupported DLG executable header"));
    }
    let count = u16_at(data, pe + 6)?;
    if count > 16 {
        return Err(invalid("too many DLG sections"));
    }
    let table = pe + 24 + u16_at(data, pe + 20)?;
    let mut code = &[][..];
    let mut reloc = &[][..];
    let mut base = 0;
    for index in 0..count {
        let section = slice(data, table + index * 40, 40)?;
        let bytes = slice(
            data,
            u32_at(section, 20)?,
            u32_at(section, 8)?.min(u32_at(section, 16)?),
        )?;
        if section.starts_with(b"CODE\0") {
            code = bytes;
            base = u32_at(section, 12)?;
        }
        if section.starts_with(b".reloc\0") {
            reloc = bytes;
        }
    }
    let (x, y) = (u16_at(code, 4)? as i32, u16_at(code, 6)? as i32);
    let mut buttons = Vec::new();
    let mut cursor = 0;
    while cursor + 8 <= reloc.len() {
        let page = u32_at(reloc, cursor)?;
        let size = u32_at(reloc, cursor + 4)?;
        if size == 0 {
            break;
        }
        if size < 8 || size % 2 != 0 {
            return Err(invalid("invalid DLG relocation block"));
        }
        for word in slice(reloc, cursor + 8, size - 8)?.chunks_exact(2) {
            let value = u16_at(word, 0)?;
            if value >> 12 != 3 {
                continue;
            }
            let Some(slot) = (page + (value & 4095)).checked_sub(base) else {
                continue;
            };
            if slot < 20 || slot + 4 > code.len() {
                continue;
            }
            let Some(label_at) = u32_at(code, slot)?.checked_sub(base) else {
                continue;
            };
            let Some(tail) = code.get(label_at..) else {
                continue;
            };
            let Some(end) = tail.iter().position(|b| *b == 0) else {
                continue;
            };
            if end == 0 || end > 80 || !tail[..end].iter().all(|b| (32..127).contains(b)) {
                continue;
            }
            let record = slot - 20;
            let bx = x + u16_at(code, record + 4)? as i32;
            let by = y + u16_at(code, record + 6)? as i32;
            let width = u16_at(code, record + 18)? as i32;
            if bx < 0 || by < 0 || width < 50 || bx + width > 640 || by + 30 > 480 {
                continue;
            }
            buttons.push(Button {
                x: bx,
                y: by,
                width,
                label: String::from_utf8(tail[..end].to_vec()).unwrap(),
            });
        }
        cursor += size;
    }
    buttons.sort_by_key(|b| b.y);
    if buttons.len() != 8 {
        return Err(invalid("expected eight CHOOSEAC action records"));
    }
    Ok(buttons)
}
