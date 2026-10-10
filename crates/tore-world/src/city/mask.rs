//! The light areas of a theater's city texture tiles (CF0 prototype).
//!
//! A ground texture placement (`tmap col row texture rotation`) covers four
//! grid cells, 32,768 ft, with a 256 by 256 indexed picture: 128 ft per
//! texel. The Ukraine city art paints its built-up land in one brown ramp of
//! palette indices, 198 (lightest, luma about 225 at noon) to 207 (darkest,
//! about 30), with greens at 192 to 197 for parks and fields and 255 for the
//! water cutout. Light roofs are 198 to 202; residential streets are long one
//! or two texel lines of 200 to 203 with 224 dots; downtown streets are dark
//! lines of 204 to 207.
//!
//! Every rule here reads palette indices, never resolved colours, so the mask
//! is the same in every weather, at every time of day and on every machine.
//! The index ramps were measured on the Ukraine art (fitted); other theaters
//! need their ramps measured before this applies to them.

use crate::resources::ResourceSource;
use std::{collections::BTreeMap, sync::Arc};

/// Texels per tile side.
pub const TEXELS: usize = 256;
/// Feet per texel: a 32,768 ft tile over 256 texels.
pub const TEXEL_FT: i64 = 128;
/// Feet per tile side.
pub const TILE_FT: i64 = 32_768;

/// A light roof or a painted house: a texel a building may stand on.
pub const SITE: u8 = 1;
/// A site texel whose eight neighbours are sites too: inside a roof at least
/// three texels (384 ft) across.
pub const SOLID: u8 = 2;
/// Part of a long, thin straight run along a texel row or column: a street.
pub const STREET_AXIS: u8 = 4;
/// A street run along the picture's down-right diagonal.
pub const STREET_D1: u8 = 8;
/// A street run along the picture's up-right diagonal.
pub const STREET_D2: u8 = 16;
/// Any street.
pub const STREET: u8 = STREET_AXIS | STREET_D1 | STREET_D2;
/// Park, field or garden green.
pub const GREEN: u8 = 32;
/// The water cutout.
pub const WATER: u8 = 64;

/// A street run is at least this many texels long (1,024 ft; fitted).
pub const STREET_MIN_RUN: u16 = 8;
/// ... and at most this many texels wide across the run (256 ft; fitted).
pub const STREET_MAX_WIDTH: u16 = 2;

fn roof(index: u8) -> bool {
    matches!(index, 198..=202 | 254)
}
fn light_street(index: u8) -> bool {
    matches!(index, 200..=203 | 224 | 254)
}
fn dark_street(index: u8) -> bool {
    matches!(index, 204..=207 | 224)
}
fn green(index: u8) -> bool {
    matches!(index, 192..=197 | 208..=215)
}

/// Whether a ground picture is city art: at least half its land texels in
/// the brown ramp and at least one in twenty a light roof (fitted, measured
/// on UKR1 to UKR6 and UKR26; shoreline, field and forest tiles stay under
/// 0.35 brown).
pub fn is_city_tile(pixels: &[u8]) -> bool {
    let land = pixels.iter().filter(|&&p| p != 255).count();
    let brown = pixels.iter().filter(|&&p| (198..=207).contains(&p)).count();
    let light = pixels.iter().filter(|&&p| (198..=201).contains(&p)).count();
    land > 0 && brown * 2 >= land && light * 20 >= land
}

/// The flags of every texel of one city picture, in picture order (row 0 is
/// the picture's first row, the tile's north edge before rotation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileMask {
    pub flags: Vec<u8>,
}

impl TileMask {
    pub fn classify(pixels: &[u8]) -> Self {
        assert_eq!(pixels.len(), TEXELS * TEXELS);
        let n = TEXELS as i32;
        let at = |c: i32, r: i32| (r * n + c) as usize;
        let inside = |c: i32, r: i32| (0..n).contains(&c) && (0..n).contains(&r);
        // Run length through each texel along a direction, for one class.
        let runs = |class: &dyn Fn(u8) -> bool, dc: i32, dr: i32| -> Vec<u16> {
            let mut out = vec![0u16; TEXELS * TEXELS];
            for r in 0..n {
                for c in 0..n {
                    if !class(pixels[at(c, r)]) || out[at(c, r)] != 0 {
                        continue;
                    }
                    // Only start at the run's first texel.
                    if inside(c - dc, r - dr) && class(pixels[at(c - dc, r - dr)]) {
                        continue;
                    }
                    let mut length = 0u16;
                    let (mut x, mut y) = (c, r);
                    while inside(x, y) && class(pixels[at(x, y)]) {
                        length += 1;
                        x += dc;
                        y += dr;
                    }
                    let (mut x, mut y) = (c, r);
                    while inside(x, y) && class(pixels[at(x, y)]) {
                        out[at(x, y)] = length;
                        x += dc;
                        y += dr;
                    }
                }
            }
            out
        };
        let mut flags = vec![0u8; TEXELS * TEXELS];
        // Directions with their perpendicular, and the flag a thin run sets.
        let directions = [
            ((1, 0), (0, 1), STREET_AXIS),
            ((0, 1), (1, 0), STREET_AXIS),
            ((1, 1), (1, -1), STREET_D1),
            ((1, -1), (1, 1), STREET_D2),
        ];
        for class in [&light_street as &dyn Fn(u8) -> bool, &dark_street] {
            for ((dc, dr), (pc, pr), flag) in directions {
                let along = runs(class, dc, dr);
                let across = runs(class, pc, pr);
                for i in 0..TEXELS * TEXELS {
                    if along[i] >= STREET_MIN_RUN && across[i] <= STREET_MAX_WIDTH {
                        flags[i] |= flag;
                    }
                }
            }
        }
        for (i, &p) in pixels.iter().enumerate() {
            if p == 255 {
                flags[i] |= WATER;
            } else if green(p) {
                flags[i] |= GREEN;
            } else if roof(p) && flags[i] & STREET == 0 {
                flags[i] |= SITE;
            }
        }
        for r in 1..n - 1 {
            for c in 1..n - 1 {
                let all =
                    (-1..=1).all(|dr| (-1..=1).all(|dc| flags[at(c + dc, r + dr)] & SITE != 0));
                if all {
                    flags[at(c, r)] |= SOLID;
                }
            }
        }
        Self { flags }
    }
}

/// One city tile on the map: its picture's mask and quarter turns.
#[derive(Clone, Debug)]
pub struct PlacedTile {
    pub texture: String,
    pub rotation: u8,
    pub mask: Arc<TileMask>,
}

/// The theater's city tiles, by grid key (`col & !3`, `row & !3`).
#[derive(Clone, Debug, Default)]
pub struct LightMap {
    pub tiles: BTreeMap<(i32, i32), PlacedTile>,
}

impl LightMap {
    /// Reads every ground placement's picture once and keeps the city ones.
    pub fn build(
        resources: &dyn ResourceSource,
        environment: &tore_formats::theater::Environment,
        base: &str,
    ) -> Self {
        let mut masks: BTreeMap<String, Option<Arc<TileMask>>> = BTreeMap::new();
        let mut tiles = BTreeMap::new();
        for (key, placement) in &environment.textures {
            let name = placement.resource_name(base);
            let mask = masks
                .entry(name.clone())
                .or_insert_with(|| {
                    let pic = tore_formats::Pic::parse(resources.get(&name)?).ok()?;
                    if pic.width != TEXELS || pic.height != TEXELS || !pic.palette.is_empty() {
                        return None;
                    }
                    let pixels: Vec<u8> = pic
                        .pixels
                        .iter()
                        .zip(&pic.mask)
                        .map(|(p, m)| if *m { *p } else { 255 })
                        .collect();
                    is_city_tile(&pixels).then(|| Arc::new(TileMask::classify(&pixels)))
                })
                .clone();
            if let Some(mask) = mask {
                tiles.insert(
                    *key,
                    PlacedTile {
                        texture: name,
                        rotation: placement.rotation,
                        mask,
                    },
                );
            }
        }
        Self { tiles }
    }

    /// The grid key of the tile holding world point `x`, `z` (feet).
    pub fn key(x: i64, z: i64) -> (i32, i32) {
        let cell = TILE_FT / 4;
        (
            (x.div_euclid(cell) as i32) & !3,
            (z.div_euclid(cell) as i32) & !3,
        )
    }

    /// The flags of the texel under world point `x`, `z`; zero off the city
    /// tiles. The picture's quarter turns are the terrain mesh's (scenery
    /// `build_mesh`): picture row 0 is the north edge at rotation 0.
    pub fn flags_at(&self, x: i64, z: i64) -> u8 {
        let key = Self::key(x, z);
        let Some(tile) = self.tiles.get(&key) else {
            return 0;
        };
        let cell = TILE_FT / 4;
        let tu = ((x - i64::from(key.0) * cell) / TEXEL_FT).clamp(0, 255) as usize;
        let tv = ((z - i64::from(key.1) * cell) / TEXEL_FT).clamp(0, 255) as usize;
        let (col, row) = match tile.rotation % 4 {
            0 => (tu, 255 - tv),
            1 => (255 - tv, 255 - tu),
            2 => (255 - tu, tv),
            _ => (tv, tu),
        };
        let flags = tile.mask.flags[row * TEXELS + col];
        // A quarter turn swaps the two diagonals.
        if tile.rotation % 2 == 1 {
            let d1 = flags & STREET_D1 != 0;
            let d2 = flags & STREET_D2 != 0;
            (flags & !(STREET_D1 | STREET_D2))
                | if d1 { STREET_D2 } else { 0 }
                | if d2 { STREET_D1 } else { 0 }
        } else {
            flags
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(fill: u8) -> Vec<u8> {
        vec![fill; TEXELS * TEXELS]
    }

    #[test]
    fn a_thin_long_line_is_a_street_and_a_roof_is_a_site() {
        // Dark ground, a light one-texel street along row 10, and a 6 by 6
        // light roof.
        let mut p = picture(206);
        for c in 0..TEXELS {
            p[10 * TEXELS + c] = 201;
        }
        for r in 40..46 {
            for c in 40..46 {
                p[r * TEXELS + c] = 199;
            }
        }
        let mask = TileMask::classify(&p);
        assert_ne!(mask.flags[10 * TEXELS + 100] & STREET_AXIS, 0);
        assert_eq!(mask.flags[10 * TEXELS + 100] & SITE, 0);
        assert_ne!(mask.flags[42 * TEXELS + 42] & SITE, 0);
        assert_ne!(mask.flags[42 * TEXELS + 42] & SOLID, 0);
        // A roof edge texel is a site but not solid.
        assert_eq!(mask.flags[40 * TEXELS + 40] & SOLID, 0);
        // A wide dark area is not a street.
        assert_eq!(mask.flags[100 * TEXELS + 100] & STREET, 0);
    }

    #[test]
    fn city_tiles_need_brown_land_and_light_roofs() {
        assert!(!is_city_tile(&picture(194)));
        assert!(!is_city_tile(&picture(206)));
        let mut p = picture(206);
        for v in p.iter_mut().step_by(10) {
            *v = 199;
        }
        assert!(is_city_tile(&p));
    }

    #[test]
    fn rotation_maps_world_texels_like_the_terrain_mesh() {
        let mut flags = vec![0u8; TEXELS * TEXELS];
        // Picture row 0, column 0: the north-west corner at rotation 0.
        flags[0] = SITE;
        let mask = Arc::new(TileMask { flags });
        let mut map = LightMap::default();
        let tile = |rotation| PlacedTile {
            texture: String::new(),
            rotation,
            mask: Arc::clone(&mask),
        };
        map.tiles.insert((0, 0), tile(0));
        assert_eq!(map.flags_at(10, TILE_FT - 10), SITE);
        assert_eq!(map.flags_at(10, 10), 0);
        map.tiles.insert((0, 0), tile(2));
        assert_eq!(map.flags_at(TILE_FT - 10, 10), SITE);
        map.tiles.insert((0, 0), tile(1));
        assert_eq!(map.flags_at(TILE_FT - 10, TILE_FT - 10), SITE);
        map.tiles.insert((0, 0), tile(3));
        assert_eq!(map.flags_at(10, 10), SITE);
    }
}
