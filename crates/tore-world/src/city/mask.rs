//! The light areas of a theater's city texture tiles (CF0 prototype).
//!
//! A ground texture placement (`tmap col row texture rotation`) covers four
//! grid cells, 32,768 ft, with a 256 by 256 indexed picture: 128 ft per
//! texel. Fifteen of the sixteen theaters reuse one set of city pictures
//! (numbered 1 to 6, 26, 55, 56 and 60 in each theater's own prefix, the same
//! indices with the theater's palette), painting built-up land in one ramp of
//! palette indices, 198 (lightest) to 207 (darkest), with 192 to 197 for
//! parks and fields (sand in Egypt) and 255 for the water cutout. Light roofs
//! are the light end of the ramp; residential streets are long one or two
//! texel lines of its middle with 224 dots; downtown streets are dark lines
//! of its dark end. Measured 2026-10-10 over all sixteen theaters
//! (`tore-app --city-fill-survey`).
//!
//! Which ramp indices count as light or dark is read from the theater's
//! reference palette (the clear-day layer at noon), so a theater whose
//! palette paints the ramp differently gets its own classes; North Vietnam's
//! paints it as jungle and has no city pictures at all, and fills its retail
//! city clusters' footprints instead ([`LightMap::districts`]). Every rule
//! reads palette indices through that fixed reference, never the mission's
//! colours, so the mask is the same in every weather and on every machine.

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

/// The texel classes of a theater's palette indices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Classes {
    /// The city ramp, 198 to 207.
    pub ramp: [bool; 256],
    /// Light roofs: ramp indices (and 254) at least [`ROOF_LUMA`] bright.
    pub roof: [bool; 256],
    /// Residential street lines: ramp indices between [`STREET_LUMA`]'s
    /// bounds, and the 224 dots.
    pub light_street: [bool; 256],
    /// Downtown street lines: ramp indices darker than [`DARK_LUMA`], and 224.
    pub dark_street: [bool; 256],
    /// Parks, fields and gardens: 192 to 197 and 208 to 215.
    pub green: [bool; 256],
}

/// A ramp index this bright at noon is a light roof (luma, fitted: the
/// Ukraine ramp's 202 is 133, Egypt's 121, their 203s 107 and 103).
pub const ROOF_LUMA: u32 = 115;
/// A ramp index in this luma range is a residential street colour (fitted).
pub const STREET_LUMA: [u32; 2] = [100, 180];
/// A ramp index darker than this is a downtown street colour (fitted).
pub const DARK_LUMA: u32 = 90;

fn luma(c: [u8; 3]) -> u32 {
    (u32::from(c[0]) * 299 + u32::from(c[1]) * 587 + u32::from(c[2]) * 114) / 1000
}

impl Classes {
    /// The classes a reference palette gives.
    pub fn from_palette(palette: &[[u8; 3]; 256]) -> Self {
        let mut out = Self {
            ramp: [false; 256],
            roof: [false; 256],
            light_street: [false; 256],
            dark_street: [false; 256],
            green: [false; 256],
        };
        for (i, &color) in palette.iter().enumerate() {
            let l = luma(color);
            let ramp = (198..=207).contains(&i);
            out.ramp[i] = ramp;
            out.roof[i] = (ramp || i == 254) && l >= ROOF_LUMA;
            out.light_street[i] =
                (ramp && (STREET_LUMA[0]..=STREET_LUMA[1]).contains(&l)) || i == 224;
            out.dark_street[i] = (ramp && l < DARK_LUMA) || i == 224;
            out.green[i] = matches!(i, 192..=197 | 208..=215);
        }
        out
    }
    /// The Ukraine classes, for a theater without a readable reference
    /// palette.
    pub fn ukraine() -> Self {
        let mut out = Self {
            ramp: [false; 256],
            roof: [false; 256],
            light_street: [false; 256],
            dark_street: [false; 256],
            green: [false; 256],
        };
        for i in 0..256 {
            let i8 = i as u8;
            out.ramp[i] = (198..=207).contains(&i8);
            out.roof[i] = matches!(i8, 198..=202 | 254);
            out.light_street[i] = matches!(i8, 200..=203 | 224 | 254);
            out.dark_street[i] = matches!(i8, 204..=207 | 224);
            out.green[i] = matches!(i8, 192..=197 | 208..=215);
        }
        out
    }
}

/// The theater's reference palette: the clear-day weather layer (condition
/// 0) at noon at sea level, untinted. Integer data from the import, the same
/// whatever the mission's weather, so the classifier reads every theater's
/// picture indices by their daylight colours on every machine.
pub fn reference_palette(resources: &dyn ResourceSource, map: &str) -> Option<[[u8; 3]; 256]> {
    let layer = tore_sim::environment::layer_resource(0, map).ok()?;
    let module = tore_formats::weather::Module::parse(resources.get(&layer)?).ok()?;
    let configuration = tore_sim::environment::Configuration::new(module, 12, 0, 0, None).ok()?;
    let base = *configuration.base_palette();
    let environment = tore_sim::environment::Environment::new(configuration);
    let sample = environment.sample(0.)?;
    Some(tore_formats::weather::expand_effects(&base, &sample, 0, 0))
}

/// Whether a ground picture is city art (fitted, 2026-10-10, over all
/// sixteen theaters):
///
/// - a city picture: at least half its land in the ramp; or a town picture
///   (55 and 56, sitting in fields): no water, at least a quarter of it in
///   the ramp and at least half of what is not park or field green
///   (shoreline pictures, which also have ramp sand, have water);
/// - at least one land texel in twenty a light roof;
/// - busy: at least one land texel in fifty stepping three or more ramp
///   shades from its right neighbour (Kurile's snow and rock, smooth ramp
///   art, stay under one in a hundred).
pub fn is_city_tile(pixels: &[u8], classes: &Classes) -> bool {
    let land = pixels.iter().filter(|&&p| p != 255).count();
    let green = pixels
        .iter()
        .filter(|&&p| classes.green[usize::from(p)])
        .count();
    let ramp = pixels
        .iter()
        .filter(|&&p| classes.ramp[usize::from(p)])
        .count();
    let light = pixels
        .iter()
        .filter(|&&p| classes.roof[usize::from(p)] && p != 254)
        .count();
    let busy = pixels
        .chunks_exact(TEXELS)
        .flat_map(|row| row.windows(2))
        .filter(|w| {
            classes.ramp[usize::from(w[0])]
                && classes.ramp[usize::from(w[1])]
                && w[0].abs_diff(w[1]) >= 3
        })
        .count();
    let city = ramp * 2 >= land;
    let town = land == pixels.len() && ramp * 4 >= land && ramp * 2 >= land - green;
    land > 0 && (city || town) && light * 20 >= land && busy * 50 >= land
}

/// The flags of every texel of one city picture, in picture order (row 0 is
/// the picture's first row, the tile's north edge before rotation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileMask {
    pub flags: Vec<u8>,
}

impl TileMask {
    pub fn classify(pixels: &[u8], classes: &Classes) -> Self {
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
        let light_street = |p: u8| classes.light_street[usize::from(p)];
        let dark_street = |p: u8| classes.dark_street[usize::from(p)];
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
            } else if classes.green[usize::from(p)] {
                flags[i] |= GREEN;
            } else if classes.roof[usize::from(p)] && flags[i] & STREET == 0 {
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

/// A retail city cluster's footprint at the retail shape scale (three times
/// its drawn size): where a theater without city pictures puts its fill, as
/// a round village thinning toward its edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct District {
    pub center: [i64; 2],
    pub half: [i64; 2],
    pub heading: i32,
}

impl District {
    /// The chance in a hundred that a texel centre at `x`, `z` is a site:
    /// 60 at the middle falling to 10 at the edge of the circle round the
    /// footprint's longer half (fitted: villages, not squares), 0 outside.
    fn density(&self, x: i64, z: i64) -> u64 {
        let r = self.half[0].max(self.half[1]).max(1);
        let d2 = (x - self.center[0]).pow(2) + (z - self.center[1]).pow(2);
        if d2 > r * r {
            return 0;
        }
        let d = crate::surface::layout::trig::isqrt(d2 as u128) as i64;
        (60 - 50 * d / r) as u64
    }
}

/// The picture column and row of world texel `tu` east, `tv` north inside a
/// placement turned `rotation` quarter turns, as the terrain mesh draws it.
fn picture_texel(tu: usize, tv: usize, rotation: u8) -> (usize, usize) {
    match rotation % 4 {
        0 => (tu, 255 - tv),
        1 => (255 - tv, 255 - tu),
        2 => (255 - tu, tv),
        _ => (tv, tu),
    }
}

/// The ground texture under a point as the terrain mesh draws it: the
/// texel's palette index, 255 where the picture is the water cutout (the
/// ocean pass shows through). `None` where no placement covers the point.
/// Reads each placement's picture once, when first asked (slice CL-AP's
/// airfield audit, which counts pavement drawn over water).
pub fn ground_texel<'a>(
    resources: &'a dyn ResourceSource,
    environment: &tore_formats::theater::Environment,
    base: &str,
) -> impl Fn(i64, i64) -> Option<u8> + 'a {
    let pictures: std::cell::RefCell<BTreeMap<String, Option<Vec<u8>>>> = Default::default();
    let by_key: BTreeMap<(i32, i32), (String, u8)> = environment
        .textures
        .iter()
        .map(|(key, p)| (*key, (p.resource_name(base), p.rotation)))
        .collect();
    move |x, z| {
        let key = LightMap::key(x, z);
        let (name, rotation) = by_key.get(&key)?;
        let mut pictures = pictures.borrow_mut();
        let pixels = pictures
            .entry(name.clone())
            .or_insert_with(|| {
                let pic = tore_formats::Pic::parse(resources.get(name)?).ok()?;
                (pic.width == TEXELS && pic.height == TEXELS && pic.palette.is_empty()).then(|| {
                    pic.pixels
                        .iter()
                        .zip(&pic.mask)
                        .map(|(p, m)| if *m { *p } else { 255 })
                        .collect()
                })
            })
            .as_ref()?;
        let cell = TILE_FT / 4;
        let tu = ((x - i64::from(key.0) * cell) / TEXEL_FT).clamp(0, 255) as usize;
        let tv = ((z - i64::from(key.1) * cell) / TEXEL_FT).clamp(0, 255) as usize;
        let (col, row) = picture_texel(tu, tv, *rotation);
        Some(pixels[row * TEXELS + col])
    }
}

/// The theater's city tiles, by grid key (`col & !3`, `row & !3`).
#[derive(Clone, Debug, Default)]
pub struct LightMap {
    pub tiles: BTreeMap<(i32, i32), PlacedTile>,
    /// The cluster footprints filled instead, when the theater has no city
    /// pictures (North Vietnam).
    pub districts: Vec<District>,
}

/// A fixed number below 100 for a world texel, so a district's sites are
/// scattered the same on every machine.
fn texel_roll(x: i64, z: i64) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (z as u64).rotate_left(29);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    (h >> 33) % 100
}

/// Reads what [`LightMap::build`] reads, without classing anything.
pub fn read_inputs(
    resources: &dyn ResourceSource,
    environment: &tore_formats::theater::Environment,
    base: &str,
) {
    reference_palette(resources, &environment.map);
    for placement in environment.textures.values() {
        resources.get(&placement.resource_name(base));
    }
}

impl LightMap {
    /// Reads every ground placement's picture once and keeps the city ones,
    /// classed by the theater's reference palette. A theater with no city
    /// pictures fills `districts` instead.
    pub fn build(
        resources: &dyn ResourceSource,
        environment: &tore_formats::theater::Environment,
        base: &str,
        districts: &[District],
    ) -> Self {
        let palette = reference_palette(resources, &environment.map);
        if std::env::var_os("TORE_LOADING_PROFILE").is_some() {
            eprintln!(
                "loading city palette: {}",
                if palette.is_some() {
                    "theater"
                } else {
                    "ukraine-fallback"
                }
            );
        }
        let classes = palette.map_or_else(Classes::ukraine, |p| Classes::from_palette(&p));
        let mut pictures: BTreeMap<String, Option<Vec<u8>>> = BTreeMap::new();
        let mut picture = |name: &str| -> Option<Vec<u8>> {
            pictures
                .entry(name.to_owned())
                .or_insert_with(|| {
                    let pic = tore_formats::Pic::parse(resources.get(name)?).ok()?;
                    if pic.width != TEXELS || pic.height != TEXELS || !pic.palette.is_empty() {
                        return None;
                    }
                    Some(
                        pic.pixels
                            .iter()
                            .zip(&pic.mask)
                            .map(|(p, m)| if *m { *p } else { 255 })
                            .collect(),
                    )
                })
                .clone()
        };
        let mut masks: BTreeMap<String, Option<Arc<TileMask>>> = BTreeMap::new();
        let mut tiles = BTreeMap::new();
        for (key, placement) in &environment.textures {
            let name = placement.resource_name(base);
            let mask = match masks.get(&name) {
                Some(mask) => mask.clone(),
                None => {
                    let mask = picture(&name).and_then(|pixels| {
                        is_city_tile(&pixels, &classes)
                            .then(|| Arc::new(TileMask::classify(&pixels, &classes)))
                    });
                    masks.insert(name.clone(), mask.clone());
                    mask
                }
            };
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
        if !tiles.is_empty() || districts.is_empty() {
            return Self {
                tiles,
                districts: Vec::new(),
            };
        }
        // No city pictures: a synthetic tile per tile a district touches,
        // north up, sites on the district's land texels.
        let cell = TILE_FT / 4;
        let mut keys = std::collections::BTreeSet::new();
        for d in districts {
            let r = d.half[0].max(d.half[1]);
            let lo = Self::key(d.center[0] - r, d.center[1] - r);
            let hi = Self::key(d.center[0] + r, d.center[1] + r);
            for kz in (lo.1..=hi.1).step_by(4) {
                for kx in (lo.0..=hi.0).step_by(4) {
                    keys.insert((kx, kz));
                }
            }
        }
        for key in keys {
            let ground = environment
                .textures
                .get(&key)
                .and_then(|p| picture(&p.resource_name(base)).map(|px| (px, p.rotation)));
            let mut flags = vec![0u8; TEXELS * TEXELS];
            let mut any = false;
            for tv in 0..TEXELS {
                for tu in 0..TEXELS {
                    let x = i64::from(key.0) * cell + tu as i64 * TEXEL_FT + TEXEL_FT / 2;
                    let z = i64::from(key.1) * cell + tv as i64 * TEXEL_FT + TEXEL_FT / 2;
                    let water = ground.as_ref().is_some_and(|(px, rotation)| {
                        let (c, r) = picture_texel(tu, tv, *rotation);
                        px[r * TEXELS + c] == 255
                    });
                    let at = (255 - tv) * TEXELS + tu;
                    if water {
                        flags[at] = WATER;
                    } else {
                        let density = districts.iter().map(|d| d.density(x, z)).max().unwrap_or(0);
                        if texel_roll(x, z) < density {
                            flags[at] = SITE;
                            any = true;
                        }
                    }
                }
            }
            if any {
                tiles.insert(
                    key,
                    PlacedTile {
                        texture: String::new(),
                        rotation: 0,
                        mask: Arc::new(TileMask { flags }),
                    },
                );
            }
        }
        Self {
            tiles,
            districts: districts.to_vec(),
        }
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
        self.tiles
            .get(&key)
            .map_or(0, |tile| Self::flags_in(tile, key, x, z))
    }

    /// [`Self::flags_at`] for many nearby points: it remembers the last tile
    /// it looked up, which a lot's samples nearly always share.
    pub fn lookup(&self) -> impl Fn(i64, i64) -> u8 + '_ {
        type Last<'a> = Option<((i32, i32), Option<&'a PlacedTile>)>;
        let last: std::cell::Cell<Last<'_>> = std::cell::Cell::new(None);
        move |x, z| {
            let key = Self::key(x, z);
            let tile = match last.get() {
                Some((k, tile)) if k == key => tile,
                _ => {
                    let tile = self.tiles.get(&key);
                    last.set(Some((key, tile)));
                    tile
                }
            };
            tile.map_or(0, |tile| Self::flags_in(tile, key, x, z))
        }
    }

    pub(crate) fn flags_in(tile: &PlacedTile, key: (i32, i32), x: i64, z: i64) -> u8 {
        let cell = TILE_FT / 4;
        let tu = ((x - i64::from(key.0) * cell) / TEXEL_FT).clamp(0, 255) as usize;
        let tv = ((z - i64::from(key.1) * cell) / TEXEL_FT).clamp(0, 255) as usize;
        let (col, row) = picture_texel(tu, tv, tile.rotation);
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
        let mask = TileMask::classify(&p, &Classes::ukraine());
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
        assert!(!is_city_tile(&picture(194), &Classes::ukraine()));
        assert!(!is_city_tile(&picture(206), &Classes::ukraine()));
        let mut p = picture(206);
        for v in p.iter_mut().step_by(10) {
            *v = 199;
        }
        assert!(is_city_tile(&p, &Classes::ukraine()));
        // Smooth light ramp art (snow, rock) is not a city.
        let smooth: Vec<u8> = (0..TEXELS * TEXELS).map(|i| 199 + (i % 2) as u8).collect();
        assert!(!is_city_tile(&smooth, &Classes::ukraine()));
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
