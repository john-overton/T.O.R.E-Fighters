//! Retail terrain data. Offsets verified against FA.EXE's loader at 0x4c5e85.
use crate::{Result, invalid, slice, u32_at};
use std::collections::BTreeMap;

pub const CELL_FEET: f32 = 8192.0;
pub const HEIGHT_FEET: f32 = 256.0;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainCell {
    pub color: u8,
    pub class: u8,
    pub elevation: u8,
}
#[derive(Debug)]
pub struct Theater {
    pub name: String,
    pub map: String,
    pub tiles: [usize; 2],
    pub cells_per_tile: usize,
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<TerrainCell>,
    pub coarse: Vec<TerrainCell>,
}
fn name(bytes: &[u8]) -> Result<String> {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    if !bytes[..end]
        .iter()
        .all(|b| b.is_ascii_graphic() || *b == b' ')
    {
        return Err(invalid("non-ASCII theater name"));
    }
    Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
}
impl Theater {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if slice(data, 0, 4)? != b"BIT2" {
            return Err(invalid("expected BIT2 terrain"));
        }
        slice(data, 0, 149)?;
        let name = name(&data[4..84])?;
        let map = name_resource(&data[84..100])?;
        let cells_per_tile = u32_at(data, 0x79)?;
        let tiles = [u32_at(data, 0x7d)?, u32_at(data, 0x81)?];
        let (cols, rows) = (u32_at(data, 0x89)?, u32_at(data, 0x8d)?);
        if !(1..=32).contains(&cells_per_tile)
            || tiles.iter().any(|v| !(1..=128).contains(v))
            || cols != tiles[0] * cells_per_tile
            || rows != tiles[1] * cells_per_tile
            || cols * rows > 1_048_576
        {
            return Err(invalid("invalid terrain dimensions"));
        }
        let (fine, coarse) = (u32_at(data, 0x91)?, u32_at(data, 0x85)?);
        if fine != 149
            || coarse != fine + cols * rows * 3
            || data.len() != coarse + tiles[0] * tiles[1] * 3
        {
            return Err(invalid("invalid terrain grid offsets/length"));
        }
        let decode = |b: &[u8]| {
            b.chunks_exact(3)
                .map(|c| TerrainCell {
                    color: c[0],
                    class: c[1],
                    elevation: c[2],
                })
                .collect()
        };
        Ok(Self {
            name,
            map,
            tiles,
            cells_per_tile,
            cols,
            rows,
            cells: decode(&data[fine..coarse]),
            coarse: decode(&data[coarse..]),
        })
    }
    pub fn cell(&self, col: usize, row: usize) -> TerrainCell {
        self.cells[row.min(self.rows - 1) * self.cols + col.min(self.cols - 1)]
    }
    /// Native 0x4c6040 switches to the coarse grid at step >= cells_per_tile.
    pub fn lookup(&self, col: i32, row: i32, step: usize) -> Option<TerrainCell> {
        if col < 0 || row < 0 {
            return None;
        }
        let (mut x, mut y) = (col as usize, row as usize);
        if step >= self.cells_per_tile {
            x /= self.cells_per_tile;
            y /= self.cells_per_tile;
            (x < self.tiles[0] && y < self.tiles[1]).then(|| self.coarse[y * self.tiles[0] + x])
        } else {
            (x < self.cols && y < self.rows).then(|| self.cell(x, y))
        }
    }
}
fn name_resource(bytes: &[u8]) -> Result<String> {
    let n = name(bytes)?.to_ascii_uppercase();
    if n.is_empty()
        || n.bytes()
            .any(|c| !c.is_ascii_alphanumeric() && c != b'.' && c != b'_' && c != b'~' && c != b'$')
    {
        return Err(invalid("invalid theater map resource name"));
    }
    Ok(n)
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TexturePlacement {
    pub col: i32,
    pub row: i32,
    pub texture: usize,
    pub rotation: u8,
}
#[derive(Debug, Default)]
pub struct Environment {
    pub map: String,
    pub layer: String,
    pub layer_parameter: Option<i32>,
    pub clouds: Option<i32>,
    pub wind: Option<[i32; 2]>,
    pub time: Option<[i32; 2]>,
    pub textures: BTreeMap<(i32, i32), TexturePlacement>,
}
impl Environment {
    /// Reads only top-level environment and tmap fields; never executes mission code.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(invalid("mission metadata exceeds limit"));
        }
        let text = String::from_utf8_lossy(bytes);
        if !text.starts_with("textFormat") {
            return Err(invalid("expected textFormat mission"));
        }
        let mut out = Self::default();
        for line in text.lines().filter(|l| !l.starts_with(char::is_whitespace)) {
            let parts: Vec<_> = line.trim_end_matches('\0').split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            let number = |i: usize| -> Result<i32> {
                parts
                    .get(i)
                    .ok_or_else(|| invalid("missing environment value"))?
                    .parse()
                    .map_err(|_| invalid("invalid environment integer"))
            };
            match parts[0] {
                "map" => {
                    out.map = name_resource(
                        parts
                            .get(1)
                            .ok_or_else(|| invalid("missing map"))?
                            .as_bytes(),
                    )?;
                }
                "layer" => {
                    out.layer = name_resource(
                        parts
                            .get(1)
                            .ok_or_else(|| invalid("missing layer"))?
                            .as_bytes(),
                    )?;
                    out.layer_parameter = Some(number(2)?);
                }
                "clouds" => out.clouds = Some(number(1)?),
                "wind" => out.wind = Some([number(1)?, number(2)?]),
                "time" => {
                    let t = [number(1)?, number(2)?];
                    if !(0..24).contains(&t[0]) || !(0..60).contains(&t[1]) {
                        return Err(invalid("invalid mission time"));
                    }
                    out.time = Some(t);
                }
                "tmap" => {
                    let v = [number(1)?, number(2)?, number(3)?, number(4)?];
                    if v[0] < -4096
                        || v[1] < -4096
                        || v[0] > 4096
                        || v[1] > 4096
                        || v[0] % 4 != 0
                        || v[1] % 4 != 0
                        || !(0..300).contains(&v[2])
                        || !(0..4).contains(&v[3])
                    {
                        return Err(invalid("unsupported tmap placement"));
                    }
                    out.textures.insert(
                        (v[0], v[1]),
                        TexturePlacement {
                            col: v[0],
                            row: v[1],
                            texture: v[2] as usize,
                            rotation: v[3] as u8,
                        },
                    );
                    if out.textures.len() > 3500 {
                        return Err(invalid("too many tmap placements"));
                    }
                }
                _ => {}
            }
        }
        Ok(out)
    }
}
/// Defined retail theater codes and source-name aliases (map/campaign names differ).
pub const THEATERS: &[(&str, &[&str])] = &[
    ("APA", &["APA"]),
    ("BAL", &["BAL"]),
    ("CUB", &["CUB"]),
    ("EGY", &["EGY"]),
    ("FRA", &["FRA"]),
    ("GRE", &["GRE"]),
    ("IRA", &["IRA"]),
    ("KURILE", &["KURILE", "KURIL"]),
    ("LFA", &["LFA"]),
    ("NSK", &["NSK"]),
    ("PGU", &["PGU"]),
    ("SPA", &["SPA"]),
    ("TVIET", &["TVIET", "VIET", "TVI"]),
    ("UKR", &["UKR"]),
    ("VLA", &["VLA"]),
    ("WTA", &["WTA"]),
];

/// Conservative named dependency selection, not recursive SH/object resolution.
/// All T2 grids remain included for the menu catalog, including a single-theater profile.
pub fn theater_resource(name: &str, theater: &str) -> bool {
    let n = name.to_ascii_uppercase();
    let code = theater.to_ascii_uppercase();
    let named = THEATERS
        .iter()
        .filter(|(id, _)| code == "ALL" || *id == code)
        .any(|(_, aliases)| {
            aliases.iter().any(|p| {
                n.starts_with(p)
                    || n.starts_with(&format!("~{p}"))
                    || n.starts_with(&format!("IFM{p}"))
            })
        });
    n.ends_with(".T2")
        || n.ends_with(".LAY")
        || n == "PALETTE.PAL"
        || named
        || ["SKY", "CLOUD", "GRND"]
            .iter()
            .any(|p| n.starts_with(p) && n.ends_with(".PIC"))
        || matches!(
            n.as_str(),
            "SUN.SH"
                | "MOON.SH"
                | "STARS.SH"
                | "CLOUD1.SH"
                | "CLOUDS.SH"
                | "_MOON.PIC"
                | "_CLOUD1.PIC"
                | "LAND.PIC"
                | "VLAND.PIC"
                | "V_LAND.PIC"
                | "QUIKMIS3.PIC"
                | "QUIKMISS.PIC"
        )
}

/// The app deliberately keeps its initial Ukraine runtime bundle.
pub fn ukraine_resource(name: &str) -> bool {
    theater_resource(name, "UKR")
}
/// Data-only view of a PL weather module. Resolves CODE RVAs, never loads code.
/// The chosen keyframe is explicit; native time/altitude interpolation is pending.
pub fn layer_palette(data: &[u8], keyframe: usize) -> Result<[[u8; 3]; 256]> {
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
    let (code, base) = code.ok_or_else(|| invalid("weather CODE missing"))?;
    let resolve = |ptr: usize, size: usize| -> Result<&[u8]> {
        slice(
            code,
            ptr.checked_sub(base)
                .ok_or_else(|| invalid("weather RVA before CODE"))?,
            size,
        )
    };
    let raw = resolve(u32_at(code, 0x70)?, 768)?;
    let frames = resolve(
        u32_at(code, 0x74)?,
        keyframe
            .checked_add(1)
            .and_then(|v| v.checked_mul(352))
            .filter(|s| *s <= 352 * 32)
            .ok_or_else(|| invalid("weather keyframe limit"))?,
    )?;
    let frame = &frames[keyframe * 352..];
    if u32_at(frame, 0)? & 1 != 0 {
        return Err(invalid("weather keyframe is sentinel"));
    }
    let mut palette = [[0; 3]; 256];
    for (i, rgb) in raw.chunks_exact(3).enumerate() {
        palette[i].copy_from_slice(rgb);
    }
    // Native 0x4b364a copies 31 sky colors to palette 224; 0x4b365c
    // copies 32 terrain colors to palette 192. They are not contiguous in index order.
    for (i, rgb) in frame[0x3e..0x9b].chunks_exact(3).enumerate() {
        palette[224 + i].copy_from_slice(rgb);
    }
    for (i, rgb) in frame[0x9b..0xfb].chunks_exact(3).enumerate() {
        palette[192 + i].copy_from_slice(rgb);
    }
    for rgb in &mut palette {
        for c in rgb {
            if *c > 63 {
                return Err(invalid("invalid weather palette component"));
            }
            *c = ((*c as u16 * 255 + 31) / 63) as u8;
        }
    }
    Ok(palette)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; 149 + 4 * 3 + 3];
        b[..4].copy_from_slice(b"BIT2");
        b[4..8].copy_from_slice(b"Test");
        b[84..89].copy_from_slice(b"T.PIC");
        for (o, v) in [
            (0x79, 2u32),
            (0x7d, 1),
            (0x81, 1),
            (0x85, 161),
            (0x89, 2),
            (0x8d, 2),
            (0x91, 149),
        ] {
            b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
        b[149..152].copy_from_slice(&[212, 3, 7]);
        b[161..164].copy_from_slice(&[255, 1, 0]);
        b
    }
    fn weather_fixture() -> Vec<u8> {
        let mut b = vec![0; 0x400 + 0x80 + 768 + 352];
        b[60..64].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x80..0x84].copy_from_slice(b"PL\0\0");
        b[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
        b[0x98..0x9d].copy_from_slice(b"CODE\0");
        for (offset, value) in [
            (0xa0, 0x80 + 768 + 352u32),
            (0xa4, 0x1000),
            (0xa8, 0x80 + 768 + 352),
            (0xac, 0x400),
            (0x470, 0x1080),
            (0x474, 0x1080 + 768),
        ] {
            b[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        b[0x480..0x780].fill(21);
        b[0x780 + 0x3e..0x780 + 0x9b].fill(63);
        b[0x780 + 0x9b..0x780 + 0xfb].fill(0);
        b
    }
    #[test]
    fn weather_rvas_and_native_palette_ramp_order() {
        let b = weather_fixture();
        let p = layer_palette(&b, 0).unwrap();
        assert_eq!(p[0], [85; 3]);
        assert_eq!(p[191], [85; 3]);
        assert_eq!(p[192], [0; 3]);
        assert_eq!(p[223], [0; 3]);
        assert_eq!(p[224], [255; 3]);
        assert_eq!(p[254], [255; 3]);
        assert_eq!(p[255], [85; 3]);
        for n in 0..b.len() {
            assert!(layer_palette(&b[..n], 0).is_err());
        }
        assert!(layer_palette(&b, usize::MAX).is_err());
        let mut bad = b.clone();
        bad[0x780] = 1;
        assert!(layer_palette(&bad, 0).is_err());
        bad = b.clone();
        bad[0x480] = 64;
        assert!(layer_palette(&bad, 0).is_err());
        bad[0x470..0x474].copy_from_slice(&0x0fffu32.to_le_bytes());
        assert!(layer_palette(&bad, 0).is_err());
    }
    #[test]
    fn packed_header_and_lookup() {
        let t = Theater::parse(&fixture()).unwrap();
        assert_eq!(
            t.cell(0, 0),
            TerrainCell {
                color: 212,
                class: 3,
                elevation: 7
            }
        );
        assert_eq!(t.lookup(0, 0, 2).unwrap().color, 255);
        assert_eq!(t.lookup(-1, 0, 1), None);
        assert_eq!(t.lookup(2, 0, 1), None);
    }
    #[test]
    fn rejects_truncation_offsets_and_oversize() {
        let b = fixture();
        for n in 0..b.len() {
            assert!(Theater::parse(&b[..n]).is_err());
        }
        for o in [0x79, 0x85, 0x89, 0x91] {
            let mut bad = b.clone();
            bad[o..o + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(Theater::parse(&bad).is_err());
        }
    }
    #[test]
    fn metadata_is_top_level_and_bounded() {
        let e=Environment::parse(b"textFormat\nmap ukr.T2\nlayer day2.LAY 0\ntime 12 30\nwind 160 7\ntmap 4 8 2 3\n\tclouds 9\n").unwrap();
        assert_eq!(e.wind, Some([160, 7]));
        assert_eq!(e.clouds, None);
        let border = Environment::parse(b"textFormat\ntmap -4 8 34 3\n").unwrap();
        assert_eq!(border.textures[&(-4, 8)].col, -4);
        assert_eq!(e.textures[&(4, 8)].rotation, 3);
        assert!(Environment::parse(b"textFormat\ntmap 0 0 -1 0").is_err());
        assert!(Environment::parse(b"textFormat\ntime 25 0").is_err());
    }
}
