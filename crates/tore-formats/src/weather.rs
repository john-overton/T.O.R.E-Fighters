//! Bounded reviewed LAY weather records. Data only; the native per-record
//! callback pointer at +0x136 is never resolved and never executed.
use crate::{Result, invalid, slice, u32_at};

/// Native record stride. Every layer scan advances by this (0x4b31b1, 0x4b3c76).
pub const RECORD: usize = 352;
/// Records past the sentinel are never read. This is an additional hard bound.
pub const MAX_RECORDS: usize = 32;
/// Effect selectors at +0x14e. Retail modules populate exactly five before the
/// record's resource name begins; `_WRWeatherEffects` is only known to pass 0.
pub const EFFECTS: usize = 5;
/// 0x4b3750 and 0x4b3c7c stop at the first record whose flag bit 0 is set.
const SENTINEL: u16 = 1;
/// The only other established flag bit. See `Layer::night_hazing`.
pub const NIGHT_HAZING: u16 = 0x40;

/// Bounded NUL-terminated resource name. Empty names are legitimate.
fn name(data: &[u8]) -> Result<String> {
    let end = data.iter().position(|c| *c == 0).unwrap_or(data.len());
    let text = std::str::from_utf8(&data[..end])
        .map_err(|_| invalid("weather record name is not text"))?
        .to_ascii_uppercase();
    if text
        .bytes()
        .any(|c| !c.is_ascii_alphanumeric() && c != b'.' && c != b'_' && c != b'~' && c != b'$')
    {
        return Err(invalid("invalid weather record resource name"));
    }
    Ok(text)
}

fn i32_at(data: &[u8], at: usize) -> Result<i32> {
    Ok(i32::from_le_bytes(slice(data, at, 4)?.try_into().unwrap()))
}

/// One reviewed 352-byte weather record. Palette components stay 6-bit as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    /// Bit 0 marks the table sentinel; the byte reaches `_currentLayer` at 0x4b3526.
    /// Observed retail values: 0x72 and 0x2e in DAY modules, 0x26/0x00/0x0a in
    /// CLOUD1, 0xa6/0x0a in FOG1. Only bits 0 and 6 have established meanings.
    pub flags: u16,
    /// Inclusive time-of-day bounds in seconds (0x4b3771, 0x4b3784).
    pub start_seconds: i32,
    pub end_seconds: i32,
    /// Inclusive altitude bounds in feet (0x4b31a3, 0x4b31ad).
    pub low_feet: i32,
    pub high_feet: i32,
    /// 31 sky colors; native 0x4b364a copies them to palette indices 224..255.
    pub sky: [[u8; 3]; 31],
    /// 32 terrain colors; native 0x4b365c copies them to palette indices 192..224.
    pub terrain: [[u8; 3]; 32],
    /// 0x4b4720 indexes this by an effect selector and takes the span minimum.
    /// Raw source values may exceed 100; only that consumer's ceiling clamps them.
    pub effects: [u8; EFFECTS],
    /// Per-record shape dependency at +0x153. Every retail module names `wave1.SH`.
    pub shape: String,
}

impl Layer {
    /// Reads one record. Rejects the sentinel; callers detect it with `is_sentinel`.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let raw = slice(data, 0, RECORD)?;
        if Self::is_sentinel(raw)? {
            return Err(invalid("weather record is the table sentinel"));
        }
        let start_seconds = i32_at(raw, 0x02)?;
        let end_seconds = i32_at(raw, 0x06)?;
        let low_feet = i32_at(raw, 0x0a)?;
        let high_feet = i32_at(raw, 0x0e)?;
        let mut sky = [[0; 3]; 31];
        let mut terrain = [[0; 3]; 32];
        for (entry, rgb) in sky.iter_mut().zip(raw[0x3e..0x9b].chunks_exact(3)) {
            entry.copy_from_slice(rgb);
        }
        for (entry, rgb) in terrain.iter_mut().zip(raw[0x9b..0xfb].chunks_exact(3)) {
            entry.copy_from_slice(rgb);
        }
        if sky.iter().chain(&terrain).flatten().any(|c| *c > 63) {
            return Err(invalid("invalid weather palette component"));
        }
        let mut effects = [0; EFFECTS];
        effects.copy_from_slice(slice(raw, 0x14e, EFFECTS)?);
        let shape = name(slice(raw, 0x153, RECORD - 0x153)?)?;
        if start_seconds < 0 || start_seconds > end_seconds || low_feet < 0 || low_feet > high_feet
        {
            return Err(invalid("weather record bounds are inverted or negative"));
        }
        Ok(Self {
            flags: crate::u16_at(raw, 0)? as u16,
            start_seconds,
            end_seconds,
            low_feet,
            high_feet,
            sky,
            terrain,
            effects,
            shape,
        })
    }

    pub fn is_sentinel(data: &[u8]) -> Result<bool> {
        Ok(crate::u16_at(data, 0)? as u16 & SENTINEL != 0)
    }

    /// `_currentLayer & 0x40` sets `_nightHazing` at 0x4b353c and returns early
    /// from `_DrawStreamer@12` at 0x49fdad. Retail DAY modules set it on exactly
    /// the two night records.
    pub fn night_hazing(&self) -> bool {
        self.flags & NIGHT_HAZING != 0
    }

    /// Inclusive, matching the native signed comparisons at 0x4b3771 and 0x4b3784.
    pub fn covers_time(&self, seconds: i32) -> bool {
        self.start_seconds <= seconds && seconds <= self.end_seconds
    }

    /// Inclusive, matching `_WRGetLayer@8` at 0x4b31a3 and 0x4b31ad.
    pub fn covers_altitude(&self, feet: i32) -> bool {
        self.low_feet <= feet && feet <= self.high_feet
    }

    /// Effect strength for one selector, clamped to the native 100 ceiling (0x4b474b).
    pub fn effect(&self, selector: usize) -> Result<u8> {
        let value = *self
            .effects
            .get(selector)
            .ok_or_else(|| invalid("weather effect selector outside record"))?;
        Ok(value.min(100))
    }
}

/// Every record in one LAY module, in table order, up to the sentinel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    /// The module's 256-entry base palette, still 6-bit as stored.
    pub base: [[u8; 3]; 256],
    pub layers: Vec<Layer>,
}

impl Module {
    /// Data-only view of a PL weather module. Resolves CODE RVAs, never loads code.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let (code, base_rva) = section(data)?;
        let resolve = |ptr: usize, size: usize| -> Result<&[u8]> {
            slice(
                code,
                ptr.checked_sub(base_rva)
                    .ok_or_else(|| invalid("weather RVA before CODE"))?,
                size,
            )
        };
        let raw = resolve(u32_at(code, 0x70)?, 768)?;
        let mut base = [[0; 3]; 256];
        for (entry, rgb) in base.iter_mut().zip(raw.chunks_exact(3)) {
            entry.copy_from_slice(rgb);
        }
        if base.iter().flatten().any(|c| *c > 63) {
            return Err(invalid("invalid weather palette component"));
        }
        let table = u32_at(code, 0x74)?;
        let mut layers = Vec::new();
        for index in 0..MAX_RECORDS {
            let record = resolve(table + index * RECORD, RECORD)?;
            if Layer::is_sentinel(record)? {
                if layers.is_empty() {
                    return Err(invalid("weather module has no records"));
                }
                return Ok(Self { base, layers });
            }
            layers.push(Layer::parse(record)?);
        }
        Err(invalid("weather record table has no sentinel"))
    }

    /// Expands one record over the base palette exactly as 0x4b364a and 0x4b365c do.
    pub fn palette(&self, index: usize) -> Result<[[u8; 3]; 256]> {
        let layer = self
            .layers
            .get(index)
            .ok_or_else(|| invalid("weather record index outside module"))?;
        Ok(expand(&self.base, layer))
    }
}

/// Six-bit source components expand to eight bits; the native copies are not contiguous.
pub fn expand(base: &[[u8; 3]; 256], layer: &Layer) -> [[u8; 3]; 256] {
    let mut palette = *base;
    palette[224..255].copy_from_slice(&layer.sky);
    palette[192..224].copy_from_slice(&layer.terrain);
    for rgb in &mut palette {
        for c in rgb {
            *c = ((u16::from(*c) * 255 + 31) / 63) as u8;
        }
    }
    palette
}

/// Locates the single CODE section of a PL module and its RVA base.
pub(crate) fn section(data: &[u8]) -> Result<(&[u8], usize)> {
    let pe = u32_at(data, 60)?;
    if slice(data, pe, 4)? != b"PL\0\0" {
        return Err(invalid("expected PL weather module"));
    }
    let count = crate::u16_at(data, pe + 6)?;
    if count > 16 {
        return Err(invalid("too many weather sections"));
    }
    let table = pe + 24 + crate::u16_at(data, pe + 20)?;
    let mut code = None;
    for i in 0..count {
        let s = slice(data, table + i * 40, 40)?;
        if s.starts_with(b"CODE\0") {
            let raw = u32_at(s, 20)?;
            let len = u32_at(s, 8)?.min(u32_at(s, 16)?);
            code = Some((slice(data, raw, len)?, u32_at(s, 12)?));
        }
    }
    code.ok_or_else(|| invalid("weather CODE missing"))
}

/// Synthetic PL weather module for committed tests. Contains no retail bytes:
/// every value is generated here, and no original artwork or record is copied.
pub fn synthetic_module(records: usize) -> Vec<u8> {
    // The base palette occupies the first 768 bytes; the table follows it.
    let table = 0x400;
    let mut code = vec![0; table + (records + 1) * RECORD];
    code[0x70..0x74].copy_from_slice(&0x100u32.to_le_bytes());
    code[0x74..0x78].copy_from_slice(&(0x100 + table as u32).to_le_bytes());
    for i in 0..records {
        let at = table + i * RECORD;
        code[at + 0x02..at + 0x06].copy_from_slice(&(i as i32 * 3600).to_le_bytes());
        code[at + 0x06..at + 0x0a].copy_from_slice(&(i as i32 * 3600 + 3599).to_le_bytes());
        code[at + 0x0e..at + 0x12].copy_from_slice(&100_000i32.to_le_bytes());
        for (j, c) in code[at + 0x3e..at + 0xfb].iter_mut().enumerate() {
            *c = (j % 64) as u8;
        }
        code[at + 0x14e] = 80;
        code[at + 0x14f] = 40;
        code[at + 0x153..at + 0x15b].copy_from_slice(b"WAVE1.SH");
    }
    code[table + records * RECORD] = 1;
    let pe = 0x80;
    let mut data = vec![0; pe + 24 + 40];
    data[60..64].copy_from_slice(&(pe as u32).to_le_bytes());
    data[pe..pe + 4].copy_from_slice(b"PL\0\0");
    data[pe + 6..pe + 8].copy_from_slice(&1u16.to_le_bytes());
    let entry = pe + 24;
    data.resize(entry + 40, 0);
    data[entry..entry + 5].copy_from_slice(b"CODE\0");
    let raw = data.len();
    data[entry + 8..entry + 12].copy_from_slice(&(code.len() as u32).to_le_bytes());
    data[entry + 12..entry + 16].copy_from_slice(&0x100u32.to_le_bytes());
    data[entry + 16..entry + 20].copy_from_slice(&(code.len() as u32).to_le_bytes());
    data[entry + 20..entry + 24].copy_from_slice(&(raw as u32).to_le_bytes());
    data.extend_from_slice(&code);
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_record_up_to_the_sentinel() {
        let m = Module::parse(&synthetic_module(4)).unwrap();
        assert_eq!(m.layers.len(), 4);
        assert_eq!(m.layers[2].start_seconds, 7200);
        assert_eq!(m.layers[2].end_seconds, 10799);
        assert!(m.layers[2].covers_time(7200));
        assert!(m.layers[2].covers_time(10799));
        assert!(!m.layers[2].covers_time(10800));
        assert!(m.layers[0].covers_altitude(0));
        assert!(m.layers[0].covers_altitude(100_000));
        assert!(!m.layers[0].covers_altitude(100_001));
        assert_eq!(m.layers[0].effect(0).unwrap(), 80);
        assert_eq!(m.layers[0].effect(1).unwrap(), 40);
        assert!(m.layers[0].effect(EFFECTS).is_err());
        assert_eq!(m.layers[0].shape, "WAVE1.SH");
    }

    #[test]
    fn expands_six_bit_components_over_the_base_palette() {
        let m = Module::parse(&synthetic_module(1)).unwrap();
        let p = m.palette(0).unwrap();
        assert_eq!(p[0], [0, 0, 0]);
        assert_eq!(p[224], [0, 4, 8]);
        assert!(m.palette(1).is_err());
        // Six-bit 63 must reach full scale, not 252.
        assert_eq!(
            p[224 + 21],
            [63, 0, 1].map(|c: u16| ((c * 255 + 31) / 63) as u8)
        );
    }

    #[test]
    fn effect_strength_is_capped_at_the_native_ceiling() {
        let mut data = synthetic_module(1);
        let at = data.len() - RECORD * 2 + 0x14e;
        data[at] = 200;
        assert_eq!(
            Module::parse(&data).unwrap().layers[0].effect(0).unwrap(),
            100
        );
    }

    #[test]
    fn night_hazing_reads_only_the_flag_word() {
        let mut data = synthetic_module(1);
        let record = data.len() - RECORD * 2;
        // 0x72 is the retail DAY night value; 0x2e is its daytime counterpart.
        data[record] = 0x72;
        let night = Module::parse(&data).unwrap().layers.remove(0);
        assert_eq!(night.flags, 0x72);
        assert!(night.night_hazing());
        data[record] = 0x2e;
        let day = Module::parse(&data).unwrap().layers.remove(0);
        assert_eq!(day.flags, 0x2e);
        assert!(!day.night_hazing());
        // The time field starts at +0x02 and must not leak into the flag word.
        assert_eq!(day.start_seconds, night.start_seconds);
    }

    #[test]
    fn malformed_modules_fail_without_panicking() {
        for length in 0..600 {
            assert!(Module::parse(&vec![0; length]).is_err());
        }
        let mut truncated = synthetic_module(2);
        truncated.truncate(truncated.len() - RECORD);
        assert!(Module::parse(&truncated).is_err());
        assert!(Module::parse(&synthetic_module(0)).is_err());
    }

    #[test]
    fn inverted_and_out_of_range_records_are_rejected() {
        let mut data = synthetic_module(1);
        let record = data.len() - RECORD * 2;
        data[record + 0x02..record + 0x06].copy_from_slice(&5000i32.to_le_bytes());
        data[record + 0x06..record + 0x0a].copy_from_slice(&1000i32.to_le_bytes());
        assert!(Module::parse(&data).is_err());
        // Retail DAY modules end their last record at i32::MAX; that must be accepted.
        let mut data = synthetic_module(1);
        data[record + 0x06..record + 0x0a].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(Module::parse(&data).is_ok());
        let mut data = synthetic_module(1);
        data[record + 0x02..record + 0x06].copy_from_slice(&(-1i32).to_le_bytes());
        assert!(Module::parse(&data).is_err());
        let mut data = synthetic_module(1);
        data[record + 0x0a..record + 0x0e].copy_from_slice(&(-1i32).to_le_bytes());
        assert!(Module::parse(&data).is_err());
        let mut data = synthetic_module(1);
        data[record + 0x3e] = 64;
        assert!(Module::parse(&data).is_err());
    }
}
