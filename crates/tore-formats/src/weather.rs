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
    // Retail records name animated sets such as `OCEAN*06.PIC`.
    if text
        .bytes()
        .any(|c| !c.is_ascii_alphanumeric() && !b"._~$*".contains(&c))
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
    /// Visibility ramp at +0x12..+0x22, in units of 256 feet. Density runs 0..256.
    /// `_WRSetRemaps@8` shifts a 24.8-foot distance right 16 before comparing,
    /// and `_WRWeatherEffects` shifts `see_distance` left 16 against the same
    /// 24.8 distances: both give the identical 256-foot unit.
    pub fog_near: i32,
    pub fog_near_density: i32,
    pub fog_far: i32,
    pub fog_far_density: i32,
    pub see_distance: i32,
    /// Altitude haze ramp at +0x26..+0x32, in units of 256 feet above `low_feet`.
    pub haze_low: i32,
    pub haze_low_blend: i32,
    pub haze_high: i32,
    pub haze_high_blend: i32,
    /// Interpolated color at +0x36. 0x4b3ad0 resolves it against the shade table
    /// at `[0x580e1c]` and caches the match in the runtime-only field at +0x3a.
    pub shade: [u8; 3],
    /// 32 terrain colors; native 0x4b365c copies them to palette indices 192..224.
    pub terrain: [[u8; 3]; 32],
    /// A second interpolated color and scalar at +0xfb and +0xfe.
    pub tint: [u8; 3],
    pub tint_scalar: i32,
    /// Two texture decks at +0x102 and +0x118. 0x4b3a19 and 0x4b3a6c copy them
    /// whole instead of interpolating, and only when the source name is set.
    pub decks: [Deck; 2],
    /// Night light direction when the sun is down, binary angles (0x8000 = 180 degrees).
    pub moon_azimuth: i16,
    pub moon_elevation: i16,
    /// Seconds of day at which the sun sits about five degrees below the horizon.
    pub sunrise_seconds: i32,
    pub sunset_seconds: i32,
    pub sun_azimuth_morning: i16,
    pub sun_azimuth_evening: i16,
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
        let shade = slice(raw, 0x36, 3)?.try_into().unwrap();
        let tint = slice(raw, 0xfb, 3)?.try_into().unwrap();
        let tint_scalar = i32_at(raw, 0xfe)?;
        let decks = [Deck::parse(raw, 0x102)?, Deck::parse(raw, 0x118)?];
        let word = |at: usize| -> Result<i16> {
            Ok(i16::from_le_bytes(slice(raw, at, 2)?.try_into().unwrap()))
        };
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
            fog_near: i32_at(raw, 0x12)?,
            fog_near_density: i32_at(raw, 0x16)?,
            fog_far: i32_at(raw, 0x1a)?,
            fog_far_density: i32_at(raw, 0x1e)?,
            see_distance: i32_at(raw, 0x22)?,
            haze_low: i32_at(raw, 0x26)?,
            haze_low_blend: i32_at(raw, 0x2a)?,
            haze_high: i32_at(raw, 0x2e)?,
            haze_high_blend: i32_at(raw, 0x32)?,
            shade,
            sky,
            terrain,
            tint,
            tint_scalar,
            decks,
            moon_azimuth: word(0x13e)?,
            moon_elevation: word(0x140)?,
            sunrise_seconds: i32_at(raw, 0x142)?,
            sunset_seconds: i32_at(raw, 0x146)?,
            sun_azimuth_morning: word(0x14a)?,
            sun_azimuth_evening: word(0x14c)?,
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

/// One textured deck: a resource name, the altitude it sits at and the
/// power-of-two exponent giving its tile size in feet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Deck {
    pub name: String,
    pub altitude_feet: i32,
    pub tile_exponent: i32,
}

impl Deck {
    /// The name occupies 14 bytes; an empty name means the deck is unused and
    /// the native copy at 0x4b3a25 skips it entirely. A `*` in the name selects
    /// one of a numbered range at load time (0x4b4680), which is how a mission
    /// picks among SKY0 to SKY8.
    fn parse(record: &[u8], at: usize) -> Result<Self> {
        Ok(Self {
            name: name(slice(record, at, 14)?)?,
            altitude_feet: i32_at(record, at + 14)?,
            tile_exponent: i32_at(record, at + 18)?,
        })
    }

    /// The numbered alternatives a wildcard name stands for, in order.
    /// Returns the name itself when it carries no wildcard.
    pub fn alternatives(&self) -> Result<Vec<String>> {
        let Some((head, rest)) = self.name.split_once('*') else {
            return Ok(vec![self.name.clone()]);
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let tail = &rest[digits.len()..];
        if digits.len() != 2 {
            return Err(invalid("weather deck wildcard needs two bound digits"));
        }
        let low = digits[..1]
            .parse::<u32>()
            .map_err(|_| invalid("bad bound"))?;
        let high = digits[1..]
            .parse::<u32>()
            .map_err(|_| invalid("bad bound"))?;
        if low > high {
            return Err(invalid("weather deck wildcard bounds are inverted"));
        }
        Ok((low..=high).map(|i| format!("{head}{i}{tail}")).collect())
    }
}

/// `0x4b3b60`: `*dest += ((src - *dest) * factor) >> 8`.
fn lerp(dest: i32, src: i32, factor: i32) -> i32 {
    dest + (((src - dest) * factor) >> 8)
}

/// `0x4b3b80` applies the same step to each component of one color.
fn lerp_color(dest: &mut [u8; 3], src: [u8; 3], factor: i32) {
    for (d, s) in dest.iter_mut().zip(src) {
        *d = lerp(i32::from(*d), i32::from(s), factor) as u8;
    }
}

/// `0x4b39a6` merges the flag byte rather than interpolating it.
fn merge_flags(dest: u16, src: u16, factor: i32) -> u16 {
    let mut merged = dest | src;
    // Bit 0x10 follows whichever record the factor is nearer to.
    let dominant = if factor < 0x80 { dest } else { src };
    if dominant & 0x10 == 0 {
        merged &= !0x10;
    }
    // With bit 0x80 present in the destination, bit 0x20 becomes an intersection.
    if dest & 0x80 != 0 && (dest & src & 0x20) == 0 {
        merged &= !0x20;
    }
    merged
}

impl Layer {
    /// `0x4b3820`. Blends `source` into this record as `position` runs across
    /// `span`, exactly as the native time and altitude overlap windows do.
    /// Bounds take the union; named dependencies replace rather than blend.
    pub fn blend(&mut self, source: &Layer, position: i32, span: i32) {
        if position <= 0 {
            return;
        }
        if position >= span {
            self.clone_from(source);
            return;
        }
        let factor = (position << 8) / span;
        self.low_feet = self.low_feet.min(source.low_feet);
        self.high_feet = self.high_feet.max(source.high_feet);
        // 0x4b3892 onward: the visibility and altitude-haze ramps interpolate.
        for (d, s) in [
            (&mut self.fog_near, source.fog_near),
            (&mut self.fog_near_density, source.fog_near_density),
            (&mut self.fog_far, source.fog_far),
            (&mut self.fog_far_density, source.fog_far_density),
            (&mut self.see_distance, source.see_distance),
            (&mut self.haze_low, source.haze_low),
            (&mut self.haze_low_blend, source.haze_low_blend),
            (&mut self.haze_high, source.haze_high),
            (&mut self.haze_high_blend, source.haze_high_blend),
        ] {
            *d = lerp(*d, s, factor);
        }
        lerp_color(&mut self.shade, source.shade, factor);
        lerp_color(&mut self.tint, source.tint, factor);
        self.tint_scalar = lerp(self.tint_scalar, source.tint_scalar, factor);
        for (d, s) in self.sky.iter_mut().zip(source.sky) {
            lerp_color(d, s, factor);
        }
        for (d, s) in self.terrain.iter_mut().zip(source.terrain) {
            lerp_color(d, s, factor);
        }
        self.flags = merge_flags(self.flags, source.flags, factor);
        self.start_seconds = self.start_seconds.min(source.start_seconds);
        self.end_seconds = self.end_seconds.max(source.end_seconds);
        for (d, s) in self.decks.iter_mut().zip(&source.decks) {
            if !s.name.is_empty() {
                d.clone_from(s);
            }
        }
    }
}

/// One record distance unit is 256 feet. See `Layer::fog_near`.
pub const DISTANCE_FEET: f64 = 256.;
/// Flag bit 0x02 enables the altitude haze pass at 0x4b3cb0.
pub const ALTITUDE_HAZE: u16 = 0x02;

impl Layer {
    /// `0x4b3cb0`. Blends this record's color ramps toward its haze color by an
    /// amount that rises with height above the band floor. The terrain ramp takes
    /// the full blend; the sky ramp fades it out from index 30 down to index 16.
    pub fn apply_altitude_haze(&mut self, altitude_feet: i32) {
        if self.flags & ALTITUDE_HAZE == 0 {
            return;
        }
        // Both sides are shifted into 256-foot steps first (0x4b3ccc, 0x4b3ccf).
        let above = (altitude_feet >> 8) - (self.low_feet >> 8);
        let blend = if above <= self.haze_low {
            self.haze_low_blend
        } else if above >= self.haze_high {
            self.haze_high_blend
        } else {
            let span = self.haze_high - self.haze_low;
            let factor = ((above - self.haze_low) << 8) / span;
            lerp(self.haze_low_blend, self.haze_high_blend, factor).min(0x100)
        };
        if blend <= 0 {
            return;
        }
        let haze = self.shade;
        for entry in &mut self.terrain {
            lerp_color(entry, haze, blend);
        }
        // 0x4b3d47: the weight decays linearly over the top fifteen sky entries.
        let step = blend / 15;
        let mut weight = blend;
        for index in (16..31).rev() {
            lerp_color(&mut self.sky[index], haze, weight);
            weight -= step;
        }
    }

    /// `0x4b3410`. Haze density 0..256 at one distance, plus whether the target
    /// is inside `see_distance` at all.
    pub fn visibility(&self, distance_feet: f64) -> (i32, bool) {
        let distance = (distance_feet / DISTANCE_FEET) as i32;
        let density = if distance <= self.fog_near {
            self.fog_near_density
        } else if distance >= self.fog_far {
            self.fog_far_density
        } else {
            let span = self.fog_far - self.fog_near;
            self.fog_near_density
                + (self.fog_far_density - self.fog_near_density) * (distance - self.fog_near) / span
        };
        (density.clamp(0, 0x100), self.see_distance >= distance)
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
