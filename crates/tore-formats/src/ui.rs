use crate::{Result, invalid, slice, u16_at, u32_at};
pub mod dialog;
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

/// Inert FA menu nodes: sibling/child RVAs, fixed header, inline text.
/// Top-level and anonymous submenu containers have a 24-byte header;
/// selectable rows have an 18-byte header followed by the 0x1e marker.
#[derive(Clone, Debug)]
pub struct MenuNode {
    pub label: String,
    pub shortcut: String,
    pub children: Vec<MenuNode>,
}
pub fn flight_menu(data: &[u8]) -> Result<Vec<MenuNode>> {
    menu_tree(data)
}

/// Shared inert tree grammar reviewed for FMENUD, QM_MENU and ARMPLANE.
/// Labels and hierarchy are data; visibility/check-state callbacks are not run.
pub fn menu_tree(data: &[u8]) -> Result<Vec<MenuNode>> {
    let (code, base) = crate::module::code(data)?;
    fn nodes(
        code: &[u8],
        base: usize,
        mut at: usize,
        depth: usize,
        seen: &mut std::collections::BTreeSet<usize>,
    ) -> Result<Vec<MenuNode>> {
        if depth > 8 {
            return Err(invalid("menu nesting limit"));
        }
        let mut result = Vec::new();
        loop {
            if seen.len() >= 256 || !seen.insert(at) {
                return Err(invalid("cyclic/oversize menu"));
            }
            let header = slice(code, at, 24)?;
            let next = u32_at(header, 0)?;
            let child = u32_at(header, 4)?;
            let start = at
                + if depth == 0 || header[18] == 0 {
                    24
                } else {
                    19
                };
            if depth > 0 && header[18] != 0 && header[18] != 0x1e {
                return Err(invalid("unreviewed menu row"));
            }
            let tail = code
                .get(start..)
                .ok_or_else(|| invalid("menu label outside CODE"))?;
            let end = tail
                .iter()
                .take(161)
                .position(|b| *b == 0)
                .ok_or_else(|| invalid("unterminated menu label"))?;
            if !tail[..end]
                .iter()
                .all(|b| (32..=127).contains(b) || [1, 0x1d].contains(b))
            {
                return Err(invalid("unsupported menu text"));
            }
            let raw =
                String::from_utf8(tail[..end].to_vec()).map_err(|_| invalid("menu encoding"))?;
            let (label, shortcut) = raw.split_once('\x01').unwrap_or((&raw, ""));
            let children = if child == 0 {
                vec![]
            } else {
                nodes(
                    code,
                    base,
                    child
                        .checked_sub(base)
                        .ok_or_else(|| invalid("menu child RVA"))?,
                    depth + 1,
                    seen,
                )?
            };
            if label.is_empty() {
                result.extend(children);
            } else {
                result.push(MenuNode {
                    label: label.replace('\x1d', "->"),
                    shortcut: shortcut.replace('\x7f', "").trim().into(),
                    children,
                });
            }
            if next == 0 {
                break;
            }
            at = next
                .checked_sub(base)
                .ok_or_else(|| invalid("menu sibling RVA"))?;
        }
        Ok(result)
    }
    nodes(code, base, 0, 0, &mut Default::default())
}
#[cfg(test)]
mod menu_tests {
    use super::*;
    #[test]
    fn menu_links_are_bounded_and_cycles_rejected() {
        let mut c = vec![0; 60];
        c[4..8].copy_from_slice(&4122u32.to_le_bytes());
        c[24] = b'?';
        c[44] = 0x1e;
        c[45..57].copy_from_slice(b"End\x01Ctrl-Q\0\0");
        let tree = flight_menu(&crate::module::fixture(&c)).unwrap();
        assert_eq!(tree[0].children[0].shortcut, "Ctrl-Q");
        c[26..30].copy_from_slice(&4122u32.to_le_bytes());
        assert!(flight_menu(&crate::module::fixture(&c)).is_err());
        c[26..30].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(flight_menu(&crate::module::fixture(&c)).is_err());
    }

    #[test]
    fn anonymous_containers_preserve_order_and_reject_shared_children() {
        let mut c = vec![0; 140];
        let pointer = |offset: usize| (4096 + offset as u32).to_le_bytes();
        c[4..8].copy_from_slice(&pointer(32));
        c[24..29].copy_from_slice(b"Root\0");
        c[36..40].copy_from_slice(&pointer(64));
        c[64..68].copy_from_slice(&pointer(100));
        c[82] = 0x1e;
        c[83..89].copy_from_slice(b"First\0");
        c[118] = 0x1e;
        c[119..126].copy_from_slice(b"Second\0");
        let tree = menu_tree(&crate::module::fixture(&c)).unwrap();
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[0].children[0].label, "First");
        assert_eq!(tree[0].children[1].label, "Second");
        c[104..108].copy_from_slice(&pointer(64));
        assert!(menu_tree(&crate::module::fixture(&c)).is_err());
    }

    #[test]
    fn invalid_markers_encoding_and_unterminated_labels_fail() {
        let mut c = vec![0; 256];
        c[4..8].copy_from_slice(&4128u32.to_le_bytes());
        c[24..29].copy_from_slice(b"Root\0");
        c[50] = 0x1e;
        c[51..56].copy_from_slice(b"Item\0");
        assert!(menu_tree(&crate::module::fixture(&c)).is_ok());
        c[50] = 0x1f;
        assert!(menu_tree(&crate::module::fixture(&c)).is_err());
        c[50] = 0x1e;
        c[51] = 0x80;
        assert!(menu_tree(&crate::module::fixture(&c)).is_err());
        c[51..212].fill(b'a');
        assert!(menu_tree(&crate::module::fixture(&c)).is_err());
    }
}
