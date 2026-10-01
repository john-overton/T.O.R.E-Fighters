//! Inert DLG geometry: identify draw records by relocated imported draw thunks.
//! No native thunk, widget callback or dynamic layout code is executed.
use crate::{Result, invalid, slice, u16_at, u32_at};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Dialog {
    pub origin: [i32; 2],
    pub size: [usize; 2],
    /// A dialog whose size is zero draws a named picture at its origin instead
    /// of a generic panel; the name (for example `MODEM`) is stored in the header.
    pub picture: Option<String>,
    pub controls: Vec<Control>,
}

#[derive(Debug)]
pub struct Control {
    pub code_offset: usize,
    pub draw: String,
    /// Static local position; native setup may replace this before drawing.
    pub position: Option<[i32; 2]>,
    /// Actions and list boxes: width in pixels. Check boxes: the side of the
    /// square. Edit boxes: the value at record offset 8 (40 on the network
    /// prompts, 25 on the pilot name); its unit is not established.
    pub width: Option<usize>,
    pub action_id: Option<u8>,
    /// Literal or imported label symbol, never the thunk's machine code.
    /// Action buttons and text records carry one.
    pub label: Option<String>,
    /// List boxes only: how many rows the box shows (record offset 16).
    pub rows: Option<usize>,
    /// Text records only: the 16-bit value after the position. It is 9 on every
    /// text record except the last of a dialog, which carries 10; its meaning is
    /// not established.
    pub text_tail: Option<u16>,
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

/// The picture name a size-zero dialog stores at header offset 14: 1 to 12
/// upper-case letters, digits or underscores ending in a zero byte.
fn picture_name(code: &[u8]) -> Option<String> {
    let field = code.get(14..27)?;
    let end = field.iter().position(|b| *b == 0)?;
    let name = &field[..end];
    (!name.is_empty()
        && name
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || *b == b'_'))
    .then(|| String::from_utf8_lossy(name).into_owned())
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
                "_DrawAction"
                    | "_DrawListBox"
                    | "_DrawDial"
                    | "_DrawRocker"
                    | "_DrawText"
                    | "_DrawEditBox"
                    | "_DrawCheck"
            ) {
                continue;
            }
            if controls.len() >= 256 {
                return Err(invalid("too many dialog controls"));
            }
            let record = slice(code, offset, 24)?;
            // Actions, list boxes and rockers keep their position at +4; a text
            // record keeps its label pointer at +4 and its position at +12.
            let position = if draw == "_DrawText" {
                [signed_word(record, 12)?, signed_word(record, 14)?]
            } else {
                [signed_word(record, 4)?, signed_word(record, 6)?]
            };
            let label_at = |pointer: usize, what: &'static str| -> Result<String> {
                if let Ok(thunk) = at(&sections, pointer, 6)
                    && thunk[..2] == [0xff, 0x25]
                {
                    imports
                        .get(&u32_at(thunk, 2)?)
                        .cloned()
                        .ok_or_else(|| invalid(what))
                } else {
                    string(&sections, pointer)
                }
            };
            let (width, action_id, label) = match draw.as_str() {
                "_DrawAction" => (
                    Some(u16_at(record, 18)?),
                    Some(record[17]),
                    Some(label_at(
                        u32_at(record, 20)?,
                        "unknown action label import",
                    )?),
                ),
                // A text record whose label pointer is zero gets its words from
                // the game at run time (the nationality check boxes).
                "_DrawText" => (
                    None,
                    None,
                    match u32_at(record, 4)? {
                        0 => None,
                        pointer => Some(label_at(pointer, "unknown text label import")?),
                    },
                ),
                "_DrawListBox" | "_DrawEditBox" => (Some(u16_at(record, 8)?), None, None),
                "_DrawCheck" => (Some(u16_at(record, 12)?), None, None),
                _ => (None, None, None),
            };
            let rows = (draw == "_DrawListBox")
                .then(|| u16_at(record, 16))
                .transpose()?;
            let text_tail = (draw == "_DrawText")
                .then(|| u16_at(record, 16))
                .transpose()?
                .map(|v| v as u16);
            if controls
                .insert(
                    offset,
                    Control {
                        code_offset: offset,
                        draw: draw.clone(),
                        position: Some(position),
                        width,
                        action_id,
                        label,
                        rows,
                        text_tail,
                    },
                )
                .is_some()
            {
                return Err(invalid("duplicate dialog draw relocation"));
            }
        }
        cursor += size;
    }
    let size = [u16_at(code, 8)?, u16_at(code, 10)?];
    Ok(Dialog {
        origin: [signed_word(code, 4)?, signed_word(code, 6)?],
        size,
        picture: if size == [0, 0] {
            picture_name(code)
        } else {
            None
        },
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

    /// Build a module whose CODE section holds `code` (at VA 0x1000) and whose
    /// draw thunks import `names`; `slots` lists (record offset, import index)
    /// pairs, each written as a relocated pointer to that import's thunk.
    fn module(mut code: Vec<u8>, names: &[&str], slots: &[(usize, usize)]) -> Vec<u8> {
        const THUNKS: usize = 0x300;
        code.resize(0x400, 0);
        let mut idata = vec![0u8; 0x300];
        idata[0..4].copy_from_slice(&0x2040u32.to_le_bytes());
        idata[16..20].copy_from_slice(&0x2080u32.to_le_bytes());
        for (i, name) in names.iter().enumerate() {
            let name_at = 0x100 + 32 * i;
            idata[0x40 + 4 * i..0x44 + 4 * i]
                .copy_from_slice(&(0x2000 + name_at as u32).to_le_bytes());
            idata[name_at + 2..name_at + 2 + name.len()].copy_from_slice(name.as_bytes());
            let thunk = THUNKS + 8 * i;
            code[thunk..thunk + 2].copy_from_slice(&[0xff, 0x25]);
            code[thunk + 2..thunk + 6].copy_from_slice(&(0x2080 + 4 * i as u32).to_le_bytes());
        }
        let mut reloc = Vec::new();
        reloc.extend_from_slice(&0x1000u32.to_le_bytes());
        reloc.extend_from_slice(&((8 + 2 * slots.len()) as u32).to_le_bytes());
        for (offset, import) in slots {
            let thunk = 0x1000 + THUNKS + 8 * import;
            code[*offset..*offset + 4].copy_from_slice(&(thunk as u32).to_le_bytes());
            reloc.extend_from_slice(&(0x3000 | *offset as u16).to_le_bytes());
        }
        reloc.extend_from_slice(&[0; 8]);
        let mut b = vec![0; 512];
        b[..2].copy_from_slice(b"MZ");
        put(&mut b, 60, 64);
        b[64..68].copy_from_slice(b"PL\0\0");
        b[68..70].copy_from_slice(&0x14cu16.to_le_bytes());
        b[70..72].copy_from_slice(&3u16.to_le_bytes());
        b[84..86].copy_from_slice(&224u16.to_le_bytes());
        put(&mut b, 192, 0x2000);
        put(&mut b, 196, 40);
        for (i, name, va, raw, size) in [
            (0, b"CODE\0\0\0\0", 0x1000, 512, 0x400),
            (1, b".idata\0\0", 0x2000, 512 + 0x400, 0x300),
            (2, b".reloc\0\0", 0x3000, 512 + 0x700, reloc.len()),
        ] {
            let s = 312 + i * 40;
            b[s..s + 8].copy_from_slice(name);
            for (off, v) in [(8, size), (12, va), (16, size), (20, raw)] {
                put(&mut b, s + off, v as u32);
            }
        }
        b.extend_from_slice(&code);
        b.extend_from_slice(&idata);
        b.extend_from_slice(&reloc);
        b
    }
    fn word(code: &mut [u8], at: usize, value: i32) {
        code[at..at + 2].copy_from_slice(&(value as i16).to_le_bytes());
    }
    /// A header with the given origin and size, then room for records at 26.
    fn header(origin: [i32; 2], size: [i32; 2]) -> Vec<u8> {
        let mut code = vec![0; 0x300];
        word(&mut code, 4, origin[0]);
        word(&mut code, 6, origin[1]);
        word(&mut code, 8, size[0]);
        word(&mut code, 10, size[1]);
        code
    }
    #[test]
    fn text_records_give_position_label_and_tail() {
        let mut code = header([86, 96], [0, 0]);
        // Record at 40: thunk, label pointer, zero, x, y, tail. Label in code at 200.
        code[200..213].copy_from_slice(b"Player name\0\0");
        put(&mut code, 44, 0x1000 + 200);
        word(&mut code, 52, 106);
        word(&mut code, 54, -4);
        word(&mut code, 56, 9);
        // A second text with a null label (filled in by the game at run time).
        word(&mut code, 78, 65);
        word(&mut code, 80, 37);
        word(&mut code, 82, 3);
        let b = module(code, &["_DrawText"], &[(40, 0), (66, 0)]);
        let d = parse(&b).unwrap();
        assert_eq!(d.controls.len(), 2);
        let (first, second) = (&d.controls[0], &d.controls[1]);
        assert_eq!(first.draw, "_DrawText");
        assert_eq!(first.position, Some([106, -4]));
        assert_eq!(first.label.as_deref(), Some("Player name"));
        assert_eq!(first.text_tail, Some(9));
        assert_eq!(
            (first.width, first.action_id, first.rows),
            (None, None, None)
        );
        assert_eq!(second.position, Some([65, 37]));
        assert_eq!(second.label, None);
        assert_eq!(second.text_tail, Some(3));
    }
    #[test]
    fn list_boxes_give_their_row_count() {
        let mut code = header([10, 80], [619, 395]);
        word(&mut code, 44, 38);
        word(&mut code, 46, 105);
        word(&mut code, 48, 200);
        word(&mut code, 56, 4);
        word(&mut code, 70, 35);
        word(&mut code, 72, 46);
        word(&mut code, 74, 549);
        word(&mut code, 82, 8);
        let b = module(code, &["_DrawListBox"], &[(40, 0), (66, 0)]);
        let d = parse(&b).unwrap();
        assert_eq!(d.picture, None);
        let rows: Vec<_> = d
            .controls
            .iter()
            .map(|c| (c.position.unwrap(), c.width.unwrap(), c.rows.unwrap()))
            .collect();
        assert_eq!(rows, [([38, 105], 200, 4), ([35, 46], 549, 8)]);
        assert!(d.controls.iter().all(|c| c.text_tail.is_none()));
    }
    #[test]
    fn edit_and_check_boxes_give_position_and_size() {
        let mut code = header([10, 200], [620, 124]);
        word(&mut code, 44, 56);
        word(&mut code, 46, 20);
        word(&mut code, 48, 40);
        // A check box repeats its position at +8 and holds its side at +12.
        word(&mut code, 70, 30);
        word(&mut code, 72, 30);
        word(&mut code, 74, 30);
        word(&mut code, 76, 30);
        word(&mut code, 78, 19);
        word(&mut code, 80, 19);
        let b = module(code, &["_DrawEditBox", "_DrawCheck"], &[(40, 0), (66, 1)]);
        let d = parse(&b).unwrap();
        assert_eq!(d.controls[0].draw, "_DrawEditBox");
        assert_eq!(d.controls[0].position, Some([56, 20]));
        assert_eq!(d.controls[0].width, Some(40));
        assert_eq!(d.controls[1].draw, "_DrawCheck");
        assert_eq!(d.controls[1].position, Some([30, 30]));
        assert_eq!(d.controls[1].width, Some(19));
        assert_eq!(d.controls[1].rows, None);
    }
    #[test]
    fn a_size_zero_dialog_names_its_picture() {
        let mut code = header([80, 84], [0, 0]);
        code[14..20].copy_from_slice(b"MODEM\0");
        let b = module(code, &[], &[]);
        let d = parse(&b).unwrap();
        assert_eq!((d.origin, d.size), ([80, 84], [0, 0]));
        assert_eq!(d.picture.as_deref(), Some("MODEM"));
        // A name with digits and an underscore, as long as the field allows.
        let mut code = header([0, 0], [0, 0]);
        code[14..27].copy_from_slice(b"AB_12CD34567\0");
        assert_eq!(
            parse(&module(code, &[], &[])).unwrap().picture.as_deref(),
            Some("AB_12CD34567")
        );
    }
    #[test]
    fn only_size_zero_dialogs_with_a_clean_name_report_a_picture() {
        // A sized dialog's bytes at 14 are not a name (retail has ff 00 00 00).
        let mut code = header([10, 80], [619, 395]);
        code[14..19].copy_from_slice(b"MODEM");
        assert_eq!(parse(&module(code, &[], &[])).unwrap().picture, None);
        for bad in [&b"\xff\0\0\0"[..], b"modem\0", b"MO DEM\0", b"\0MODEM\0"] {
            let mut code = header([0, 0], [0, 0]);
            code[14..14 + bad.len()].copy_from_slice(bad);
            assert_eq!(parse(&module(code, &[], &[])).unwrap().picture, None);
        }
        // No terminator inside the field.
        let mut code = header([0, 0], [0, 0]);
        code[14..27].copy_from_slice(b"ABCDEFGHIJKLM");
        assert_eq!(parse(&module(code, &[], &[])).unwrap().picture, None);
    }
    #[test]
    fn a_bad_text_label_pointer_is_an_error() {
        let mut code = header([0, 0], [0, 0]);
        put(&mut code, 44, 0x9000);
        assert!(parse(&module(code, &["_DrawText"], &[(40, 0)])).is_err());
    }

    /// The retail dialogs, when the install is present (the `gameassets` link
    /// or `TORE_GAME_DIR`); the test skips quietly otherwise.
    fn retail(name: &str) -> Option<Dialog> {
        let root = std::env::var_os("TORE_GAME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../gameassets/fighters-anthology")
            });
        let archive = crate::Archive::open(root.join("FA_2.LIB")).ok()?;
        let data = archive.read(&format!("{name}.DLG")).ok()?;
        Some(parse(&data).unwrap_or_else(|e| panic!("{name}: {e}")))
    }
    fn summary(d: &Dialog, draw: &str) -> Vec<([i32; 2], Option<usize>, Option<String>)> {
        d.controls
            .iter()
            .filter(|c| c.draw == draw)
            .map(|c| (c.position.unwrap(), c.rows.or(c.width), c.label.clone()))
            .collect()
    }
    #[test]
    fn retail_network_dialogs_decode_completely() {
        let Some(newnet) = retail("NEWNET") else {
            eprintln!("skipped: no retail install");
            return;
        };
        // NEWNET: a generic 619 by 395 panel, four buttons, a 4 row Games list, a rocker.
        assert_eq!(
            (newnet.origin, newnet.size, newnet.picture.as_deref()),
            ([10, 80], [619, 395], None)
        );
        let buttons: Vec<_> = newnet
            .controls
            .iter()
            .filter(|c| c.draw == "_DrawAction")
            .map(|c| {
                (
                    c.label.clone().unwrap(),
                    c.position.unwrap(),
                    c.width,
                    c.action_id,
                )
            })
            .collect();
        assert_eq!(
            buttons,
            [
                ("New".to_string(), [96, 339], Some(85), Some(1)),
                ("Join".to_string(), [219, 339], Some(85), Some(0)),
                ("Options".to_string(), [342, 339], Some(85), Some(0)),
                ("_cancelString".to_string(), [465, 339], Some(85), Some(2)),
            ]
        );
        assert_eq!(
            summary(&newnet, "_DrawListBox"),
            [([38, 105], Some(4), None)]
        );
        assert_eq!(newnet.controls.last().unwrap().draw, "_DrawRocker");
        assert_eq!(newnet.controls.last().unwrap().position, Some([270, 120]));
        // The host's and the joiner's player lists: 8 rows, 549 wide.
        for (name, count) in [("NETNEW", 4), ("NETJOIN", 2)] {
            let d = retail(name).unwrap();
            assert_eq!((d.origin, d.size), ([10, 100], [619, 375]), "{name}");
            assert_eq!(d.controls.len(), count, "{name}");
            let list = d
                .controls
                .iter()
                .find(|c| c.draw == "_DrawListBox")
                .unwrap();
            assert_eq!(
                (list.position, list.width, list.rows),
                (Some([35, 46]), Some(549), Some(8))
            );
        }
        // Options panels, the message prompts, the modem status box.
        for (name, size) in [("NETTCP", [465, 395]), ("NETIPX2", [465, 275])] {
            let d = retail(name).unwrap();
            assert_eq!((d.origin, d.size), ([89, 80], size), "{name}");
            let labels: Vec<_> = d
                .controls
                .iter()
                .map(|c| c.label.clone().unwrap())
                .collect();
            assert_eq!(labels, ["_okString", "Default", "_cancelString"], "{name}");
        }
        let prompt = retail("NETCEDT").unwrap();
        assert_eq!((prompt.origin, prompt.size), ([10, 200], [620, 124]));
        let edit = prompt
            .controls
            .iter()
            .find(|c| c.draw == "_DrawEditBox")
            .unwrap();
        assert_eq!((edit.position, edit.width), (Some([56, 20]), Some(40)));
        let status = retail("MODEMSTS").unwrap();
        assert_eq!(
            (status.origin, status.size, status.picture.as_deref()),
            ([140, 174], [360, 190], None)
        );
        assert_eq!(
            summary(&status, "_DrawText"),
            [([135, 35], None, Some("Connection status".into()))]
        );
        // MODEM names its picture and carries two column headings.
        let modem = retail("MODEM").unwrap();
        assert_eq!((modem.origin, modem.size), ([80, 84], [0, 0]));
        assert_eq!(modem.picture.as_deref(), Some("MODEM"));
        assert_eq!(
            summary(&modem, "_DrawText"),
            [
                ([106, 32], None, Some("Player name".into())),
                ([286, 32], None, Some("Phone number".into())),
            ]
        );
        let tails: Vec<_> = modem.controls.iter().filter_map(|c| c.text_tail).collect();
        assert_eq!(tails, [9, 10]);
        // The callsign picker: 14 rows and a heading.
        let callsign = retail("CALLSIGN").unwrap();
        assert_eq!(
            summary(&callsign, "_DrawListBox"),
            [([30, 35], Some(14), None)]
        );
        assert_eq!(
            summary(&callsign, "_DrawText"),
            [([43, 10], None, Some("Choose your callsign.".into()))]
        );
    }
    #[test]
    fn every_retail_multiplayer_dialog_decodes() {
        if retail("NEWNET").is_none() {
            eprintln!("skipped: no retail install");
            return;
        }
        let names = [
            "MODEM", "MODEMCOM", "MODEMSTS", "MODLIST", "SERIAL", "COM", "COMLIST", "NEWNET",
            "NETNEW", "NETJOIN", "NETTCP", "NETIPX", "NETIPX2", "NETDIR", "NETEDT", "NETBEDT",
            "NETCEDT", "CALLSIGN", "EDITSIGN", "MC_DELAY", "MC_DIST", "MC_DLG", "MC_KILLS",
            "MC_KILLT", "MC_LIVES", "MC_NAME", "MC_NAT", "MC_NAT2", "MC_NATF", "MC_SCR", "MC_TIME",
            "MC_WETH",
        ];
        for name in names {
            let d = retail(name).unwrap();
            assert!(!d.controls.is_empty() || d.size == [0, 0], "{name}");
            // A size-zero dialog names the picture it draws; a sized one does not.
            assert_eq!(d.picture.is_some(), d.size == [0, 0], "{name}");
            for c in &d.controls {
                assert!(c.position.is_some(), "{name} {}", c.draw);
                // Only the three dialogs of check boxes leave text labels to the game.
                if c.draw == "_DrawText" && c.label.is_none() {
                    assert!(["MC_NAT", "MC_NATF"].contains(&name), "{name}");
                }
                assert_eq!(c.rows.is_some(), c.draw == "_DrawListBox", "{name}");
                assert_eq!(c.text_tail.is_some(), c.draw == "_DrawText", "{name}");
            }
        }
        // The pictures behind the size-zero dialogs.
        for (name, picture) in [
            ("MODEM", "MODEM"),
            ("MODEMCOM", "MODEMCOM"),
            ("SERIAL", "SERIAL"),
            ("COM", "COM"),
            ("NETIPX", "NETIPX"),
            ("NETDIR", "NETDIR"),
            ("MC_DLG", "MC"),
        ] {
            assert_eq!(
                retail(name).unwrap().picture.as_deref(),
                Some(picture),
                "{name}"
            );
        }
        // Row counts of the pick lists, in the survey's table.
        for (name, rows) in [
            ("MODLIST", 10),
            ("COMLIST", 10),
            ("MC_NAT2", 15),
            ("MC_KILLT", 4),
        ] {
            let d = retail(name).unwrap();
            let list = d
                .controls
                .iter()
                .find(|c| c.draw == "_DrawListBox")
                .unwrap();
            assert_eq!(list.rows, Some(rows), "{name}");
        }
    }
}
