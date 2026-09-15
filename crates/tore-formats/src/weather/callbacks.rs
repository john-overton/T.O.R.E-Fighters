//! Narrow LAY import-alias reader. No loader, relocation or executable dispatch.
use crate::{Result, invalid, slice, u16_at, u32_at};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Callback {
    #[default]
    None,
    /// FA main.dll export 0x4aace0 is a bare return.
    HorizonNoop,
    /// FA main.dll export 0x4b4320 updates the source record's tint scalar.
    Fog,
}

pub(super) fn resolve(data: &[u8], code: &[u8], base: usize, pointer: usize) -> Result<Callback> {
    if pointer == 0 {
        return Ok(Callback::None);
    }
    let alias = slice(
        code,
        pointer
            .checked_sub(base)
            .ok_or_else(|| invalid("weather callback before CODE"))?,
        6,
    )?;
    if alias[..2] != [0xff, 0x25] {
        return Err(invalid("unsupported weather callback alias"));
    }
    let iat = u32_at(alias, 2)?;
    let pe = u32_at(data, 60)?;
    let optional = u16_at(data, pe + 20)?;
    // Reviewed LAY modules use zero image base; do not silently reinterpret VAs.
    if optional < 32 || u32_at(data, pe + 24 + 28)? != 0 {
        return Err(invalid("unsupported weather callback image base"));
    }
    let table = pe + 24 + optional;
    let mut sections = Vec::new();
    let mut directory = None;
    for i in 0..u16_at(data, pe + 6)? {
        let row = slice(data, table + i * 40, 40)?;
        let bytes = slice(
            data,
            u32_at(row, 20)?,
            u32_at(row, 8)?.min(u32_at(row, 16)?),
        )?;
        let rva = u32_at(row, 12)?;
        if row.starts_with(b".idata\0") && directory.replace(rva).is_some() {
            return Err(invalid("duplicate weather import section"));
        }
        sections.push((rva, bytes));
    }
    let read = |rva: usize, size: usize| -> Result<&[u8]> {
        let mut found = None;
        for (base, bytes) in &sections {
            if let Some(offset) = rva.checked_sub(*base)
                && offset
                    .checked_add(size)
                    .is_some_and(|end| end <= bytes.len())
                && found.replace(&bytes[offset..offset + size]).is_some()
            {
                return Err(invalid("overlapping weather import sections"));
            }
        }
        found.ok_or_else(|| invalid("weather import RVA outside sections"))
    };
    let string = |rva: usize| -> Result<String> {
        let mut text = String::new();
        for i in 0..256 {
            let byte = read(
                rva.checked_add(i)
                    .ok_or_else(|| invalid("weather import overflow"))?,
                1,
            )?[0];
            if byte == 0 {
                return Ok(text);
            }
            if !byte.is_ascii_graphic() {
                return Err(invalid("invalid weather import name"));
            }
            text.push(char::from(byte));
        }
        Err(invalid("unterminated weather import name"))
    };
    let add = |rva: usize, offset: usize| {
        rva.checked_add(offset)
            .ok_or_else(|| invalid("weather import address overflow"))
    };
    let directory = directory.ok_or_else(|| invalid("weather callback imports missing"))?;
    for index in 0..64 {
        let descriptor = read(add(directory, index * 20)?, 20)?;
        if descriptor.iter().all(|b| *b == 0) {
            break;
        }
        let lookup = u32_at(descriptor, 0)?;
        let first = u32_at(descriptor, 16)?;
        let library = string(u32_at(descriptor, 12)?)?;
        let lookup = if lookup == 0 { first } else { lookup };
        for index in 0..1024 {
            let entry = u32_at(read(add(lookup, index * 4)?, 4)?, 0)?;
            if entry == 0 {
                break;
            }
            if add(first, index * 4)? != iat {
                continue;
            }
            if entry & 0x8000_0000 != 0 || !library.eq_ignore_ascii_case("main.dll") {
                return Err(invalid("unsupported weather callback import"));
            }
            return match string(entry + 2)?.as_str() {
                "_T_HorizonProc" => Ok(Callback::HorizonNoop),
                "_WRFogLayerUpdate" => Ok(Callback::Fog),
                _ => Err(invalid("unsupported weather callback symbol")),
            };
        }
    }
    Err(invalid("weather callback alias has no supported import"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(symbol: &str) -> (Vec<u8>, Vec<u8>) {
        let mut code = vec![0xff, 0x25];
        code.extend_from_slice(&0x2030u32.to_le_bytes());
        let mut data = crate::module::fixture(&code);
        data[70..72].copy_from_slice(&2u16.to_le_bytes());
        data[160..167].copy_from_slice(b".idata\0");
        let raw = data.len();
        for (offset, value) in [(8, 128), (12, 0x2000), (16, 128), (20, raw)] {
            data[160 + offset..164 + offset].copy_from_slice(&(value as u32).to_le_bytes());
        }
        data.resize(raw + 128, 0);
        for (offset, value) in [(0, 0x2030u32), (12, 0x2050), (16, 0x2030), (0x30, 0x2060)] {
            data[raw + offset..raw + offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        data[raw + 0x50..raw + 0x58].copy_from_slice(b"main.dll");
        data[raw + 0x62..raw + 0x62 + symbol.len()].copy_from_slice(symbol.as_bytes());
        (data, code)
    }

    #[test]
    fn resolves_only_reviewed_import_aliases() {
        for (symbol, expected) in [
            ("_WRFogLayerUpdate", Callback::Fog),
            ("_T_HorizonProc", Callback::HorizonNoop),
        ] {
            let (data, code) = fixture(symbol);
            assert_eq!(resolve(&data, &code, 4096, 4096).unwrap(), expected);
            for length in 0..data.len() {
                assert!(resolve(&data[..length], &code, 4096, 4096).is_err());
            }
            assert!(resolve(&data, &code, 4096, 4095).is_err());
            assert!(resolve(&data, &code, 4096, 4097).is_err());
            let mut bad = code.clone();
            bad[0] = 0xe9;
            assert!(resolve(&data, &bad, 4096, 4096).is_err());
        }
        let (data, code) = fixture("_Unknown");
        assert!(resolve(&data, &code, 4096, 4096).is_err());
        assert_eq!(resolve(&[], &[], 0, 0).unwrap(), Callback::None);
    }
}
