//! Opinionated presentation, John 2026-10-18. All distances are feet.
//! Shared edge masks and low-frequency colours, cached beside habitat/canopy.
#[path = "terrain_seams_audit.rs"]
pub mod audit;

use crate::{
    terrain::Terrain,
    vegetation::{Atlas, Kind},
};
use std::collections::BTreeMap;

pub const TILE: f32 = 32768.;
pub const HALF_WIDTH: f32 = 250.;
pub const SEGMENTS: usize = 256;
pub const RECORD: usize = 7;
pub const STRIDE: usize = 4 + 2 * SEGMENTS * RECORD;
pub const BLEND: u8 = 1;
pub const HEDGE: u8 = 2;
pub const FALLBACK: u8 = 4;
const PREVIOUS: u8 = 8;
const NEXT: u8 = 16;
const NOTICEABLE: f32 = 0.015;

pub fn enabled() -> crate::AppResult<bool> {
    match std::env::var("TORE_TERRAIN_SEAMS").as_deref() {
        Ok("0") => Ok(false),
        Ok("1") | Err(std::env::VarError::NotPresent) => Ok(true),
        _ => Err("TORE_TERRAIN_SEAMS must be 0 or 1".into()),
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map {
    pub cols: usize,
    pub rows: usize,
    /// Each tile: two palette endpoints plus weight and padding, then east
    /// and north masks. Opposite sides read the very same edge record.
    pub bytes: Vec<u8>,
    pub mean_error: f32,
}

fn linear(v: u8) -> f32 {
    let v = f32::from(v) / 255.;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Fit a two-index mixture to the exact linear-light mean. Source palette
/// ramps make this compact approximation useful; the audit reports its error.
pub fn mean(pixels: &[u8], palette: &[[u8; 3]; 256]) -> ([f32; 3], [u8; 3], f32) {
    mean_linear(pixels, &palette.map(|rgb| rgb.map(linear)))
}

fn mean_linear(pixels: &[u8], colors: &[[f32; 3]; 256]) -> ([f32; 3], [u8; 3], f32) {
    let mut counts = [0u32; 256];
    let mut used = Vec::new();
    let mut total = 0_u32;
    for &i in pixels {
        if i != 255 {
            let i = usize::from(i);
            if counts[i] == 0 {
                used.push(i);
            }
            counts[i] += 1;
            total += 1;
        }
    }
    used.sort_unstable();
    let total = total.max(1) as f32;
    let exact: [f32; 3] = std::array::from_fn(|k| {
        used.iter()
            .map(|&i| colors[i][k] * counts[i] as f32 / total)
            .sum()
    });
    let mut best = ([0; 3], f32::INFINITY);
    for &a in &used {
        for &b in &used {
            let delta: [f32; 3] = std::array::from_fn(|k| colors[b][k] - colors[a][k]);
            let norm = delta.iter().map(|v| v * v).sum::<f32>();
            let t = if norm > 0. {
                ((0..3)
                    .map(|k| (exact[k] - colors[a][k]) * delta[k])
                    .sum::<f32>()
                    / norm)
                    .clamp(0., 1.)
            } else {
                0.
            };
            let weight = (t * 255.).round() as u8;
            let error = (0..3)
                .map(|k| (colors[a][k] + delta[k] * f32::from(weight) / 255. - exact[k]).powi(2))
                .sum::<f32>();
            if error < best.1 {
                best = ([a as u8, b as u8, weight], error);
            }
        }
    }
    if used.is_empty() {
        return ([0.; 3], [0; 3], 0.);
    }
    (exact, best.0, best.1.sqrt())
}

pub fn pair_flags(a: Kind, b: Kind, blocked: bool, difference: f32) -> u8 {
    if blocked || difference < NOTICEABLE {
        return 0;
    }
    if a == b && matches!(a, Kind::Grass | Kind::Forest | Kind::Scrub) {
        BLEND
    } else if matches!(
        (a, b),
        (Kind::Field, Kind::Forest) | (Kind::Forest, Kind::Field)
    ) {
        HEDGE
    } else {
        0
    }
}

pub fn weight(distance: f32) -> f32 {
    let t = ((HALF_WIDTH - distance.abs()) / (2. * HALF_WIDTH)).clamp(0., 0.5);
    t * t * (3. - 2. * t)
}

impl Map {
    pub fn build(
        world: &Terrain,
        atlas: &Atlas,
        layers: &BTreeMap<(i32, i32), usize>,
        land: Option<usize>,
        pixels: &[u8],
    ) -> Self {
        let cols = (world.theater.cols - 1).div_ceil(4);
        let rows = (world.theater.rows - 1).div_ceil(4);
        let mut out = Self {
            cols,
            rows,
            bytes: vec![0; cols * rows * STRIDE],
            mean_error: 0.,
        };
        // Use the actual uploaded pages, including neutralized airfield art
        // and the fitted LAND fallback, never a guessed texture ordinal.
        let means: Vec<_> = pixels
            .chunks_exact(65536)
            .map(|p| mean(p, &atlas.palette))
            .collect();
        // Protect source city art even when the optional city fill is off.
        let city_classes = tore_world::city::mask::Classes::from_palette(&atlas.palette);
        let city_pages: Vec<_> = pixels
            .chunks_exact(65536)
            .map(|p| tore_world::city::mask::is_city_tile(p, &city_classes))
            .collect();
        let pages: Vec<_> = (0..cols * rows)
            .map(|i| {
                layers
                    .get(&((i % cols * 4) as i32, (i / cols * 4) as i32))
                    .copied()
                    .or(land)
            })
            .collect();
        for (i, &page) in pages.iter().enumerate() {
            if let Some(p) = page {
                out.bytes[i * STRIDE..i * STRIDE + 3].copy_from_slice(&means[p].1);
            }
        }
        let kinds: [Kind; 256] = std::array::from_fn(|i| atlas.kind_index(i as u8));
        let city = world.city_fill.as_ref().map(|f| f.layout.map.lookup());
        let mut bounds: Vec<_> = world
            .airport_scene
            .runways
            .iter()
            .map(|r| r.surface)
            .chain(
                world
                    .airport_scene
                    .objects
                    .iter()
                    .filter(|o| o.runway)
                    .map(|o| o.bounds),
            )
            .collect();
        for b in &mut bounds {
            b.half[0] += 348.;
            b.half[2] += 348.;
        }
        let sample = |x: i64, z: i64| -> Option<(Kind, u8)> {
            if x < 0
                || z < 0
                || x >= (world.theater.cols as i64 - 1) * 8192
                || z >= (world.theater.rows as i64 - 1) * 8192
            {
                return None;
            }
            let col = x as usize / 8192;
            let row = z as usize / 8192;
            let tile = (row / 4) * cols + col / 4;
            let key = ((col & !3) as i32, (row & !3) as i32);
            let place = world.environment.textures.get(&key);
            if place.is_none() && world.theater.cell(col, row).color == 255 {
                return None;
            }
            let page = pages[tile]?;
            if city_pages[page] {
                return None;
            }
            let (u, v) = ((x % 32768 / 128) as usize, (z % 32768 / 128) as usize);
            let (u, v) = crate::vegetation::rotate(u, v, place.map_or(0, |p| p.rotation));
            let index = pixels[page * 65536 + v * 256 + u];
            let kind = kinds[usize::from(index)];
            if !matches!(kind, Kind::Grass | Kind::Forest | Kind::Field | Kind::Scrub)
                || atlas.coast_blocked(x, z)
            {
                return None;
            }
            if world
                .city_fill
                .as_ref()
                .is_some_and(|fill| fill.layout.map.tiles.contains_key(&key))
                || city.as_ref().is_some_and(|get| {
                    get(x, z)
                        & (tore_world::city::mask::SITE
                            | tore_world::city::mask::STREET
                            | tore_world::city::mask::WATER)
                        != 0
                })
                || world.city_fill.as_ref().is_some_and(|fill| {
                    fill.layout.map.districts.iter().any(|d| {
                        let radius = d.half[0].max(d.half[1]) + 128;
                        (x - d.center[0]).pow(2) + (z - d.center[1]).pow(2) <= radius.pow(2)
                    })
                })
                || bounds
                    .iter()
                    .any(|b| b.contains_horizontal(x as f64, z as f64))
            {
                return None;
            }
            Some((kind, index))
        };
        let linear_palette = atlas.palette.map(|p| p.map(linear));
        for tile in 0..cols * rows {
            if pages[tile].is_none() {
                continue;
            }
            for axis in 0..2 {
                if (axis == 0 && tile % cols + 1 == cols) || (axis == 1 && tile / cols + 1 == rows)
                {
                    continue;
                }
                let next = tile + if axis == 0 { 1 } else { cols };
                if pages[next].is_none() {
                    continue;
                }
                let origin = [(tile % cols) as i64 * 32768, (tile / cols) as i64 * 32768];
                let keys = [
                    ((tile % cols * 4) as i32, (tile / cols * 4) as i32),
                    ((next % cols * 4) as i32, (next / cols * 4) as i32),
                ];
                let fallback = match (layers.contains_key(&keys[0]), layers.contains_key(&keys[1]))
                {
                    (false, true) if land.is_some() => Some(0),
                    (true, false) if land.is_some() => Some(1),
                    _ => None,
                };
                // Corners/junctions stay crisp. The 384 ft guard also keeps
                // orthogonal corrections from overlapping.
                for segment in 3..SEGMENTS - 3 {
                    let mut modes = [Kind::Water; 2];
                    let mut blocked = false;
                    let mut sampled = [[255u8; 9]; 2];
                    let mut mixed_field = [false; 2];
                    for (side, mode) in modes.iter_mut().enumerate() {
                        let mut counts = [0u8; 12];
                        for (ai, along) in [-128, 0, 128].into_iter().enumerate() {
                            for (di, depth) in [64, 192, 320].into_iter().enumerate() {
                                let mut p = origin;
                                p[axis] += 32768 + if side == 0 { -depth } else { depth };
                                p[1 - axis] += segment as i64 * 128 + 64 + along;
                                match sample(p[0], p[1]) {
                                    Some((kind, index)) => {
                                        counts[kind as usize] += 1;
                                        sampled[side][ai * 3 + di] = index;
                                    }
                                    None => blocked = true,
                                }
                            }
                        }
                        *mode = [Kind::Field, Kind::Grass, Kind::Forest, Kind::Scrub]
                            .into_iter()
                            .max_by_key(|k| counts[*k as usize])
                            .unwrap();
                        // Field colour boundaries are intentional, including
                        // a narrow field stripe inside otherwise natural art.
                        if counts[Kind::Field as usize] > 0 && *mode != Kind::Field {
                            mixed_field[side] = true;
                        }
                    }
                    let candidate = pair_flags(
                        modes[0],
                        modes[1],
                        blocked || mixed_field.iter().any(|v| *v),
                        1.,
                    );
                    // LAND is a generic fill, not an authored crop boundary.
                    // A matching T2 base and green fallback ramp establish a
                    // continuous background even when shade alone says Field.
                    let continuous = !blocked
                        && [-128, 0, 128].into_iter().all(|along| {
                            let mut a = origin;
                            let mut b = origin;
                            a[axis] += 32768 - 64;
                            b[axis] += 32768 + 64;
                            a[1 - axis] += segment as i64 * 128 + 64 + along;
                            b[1 - axis] = a[1 - axis];
                            let a = world
                                .theater
                                .cell(a[0] as usize / 8192, a[1] as usize / 8192);
                            let b = world
                                .theater
                                .cell(b[0] as usize / 8192, b[1] as usize / 8192);
                            a.color == b.color && a.class == b.class
                        });
                    let filler = fallback.is_some_and(|f| {
                        continuous
                            && !mixed_field[1 - f]
                            && matches!(modes[1 - f], Kind::Grass | Kind::Forest | Kind::Scrub)
                            && sampled[f].iter().all(|i| matches!(i,192..=197|208..=215))
                    });
                    if candidate == 0 && !filler {
                        continue;
                    }
                    let difference = |a: [f32; 3], b: [f32; 3]| {
                        (0..3).map(|k| (a[k] - b[k]).abs()).fold(0., f32::max)
                    };
                    let mut flags = 0;
                    if candidate & HEDGE != 0 {
                        let a = mean_linear(&sampled[0], &linear_palette);
                        let b = mean_linear(&sampled[1], &linear_palette);
                        if difference(a.0, b.0) >= NOTICEABLE {
                            flags |= HEDGE;
                        }
                    }
                    let edge = sampled.map(|p| [p[0], p[3], p[6]]);
                    let a = mean_linear(&edge[0], &linear_palette);
                    let b = mean_linear(&edge[1], &linear_palette);
                    if (candidate & BLEND != 0 || filler) && difference(a.0, b.0) >= NOTICEABLE {
                        flags |= BLEND;
                        if filler {
                            flags |= FALLBACK;
                        }
                        out.mean_error = out.mean_error.max(a.2).max(b.2);
                    }
                    let at = tile * STRIDE + 4 + (axis * SEGMENTS + segment) * RECORD;
                    out.bytes[at] = flags;
                    out.bytes[at + 1..at + 4].copy_from_slice(&a.1);
                    out.bytes[at + 4..at + 7].copy_from_slice(&b.1);
                }
            }
        }
        out
    }

    pub fn record(&self, tile: usize, axis: usize, segment: usize) -> &[u8] {
        let at = tile * STRIDE + 4 + (axis * SEGMENTS + segment) * RECORD;
        &self.bytes[at..at + RECORD]
    }
    #[cfg(test)]
    pub fn edge_delta(
        &self,
        tile: usize,
        axis: usize,
        along: f32,
        palette: &[[f32; 3]; 256],
    ) -> [f32; 3] {
        let current = (along / 128.).floor() as usize;
        let q = along / 128. - 0.5;
        let decode = |segment: i32| {
            let segment = segment.clamp(0, 255) as usize;
            let segment = if self.record(tile, axis, segment)[0] & BLEND != 0 {
                segment
            } else {
                current
            };
            let record = self.record(tile, axis, segment);
            let color = |i: usize| {
                let a = palette[usize::from(record[i])];
                let b = palette[usize::from(record[i + 1])];
                let t = f32::from(record[i + 2]) / 255.;
                std::array::from_fn::<_, 3, _>(|k| a[k] + (b[k] - a[k]) * t)
            };
            let a = color(1);
            let b = color(4);
            std::array::from_fn::<_, 3, _>(|k| b[k] - a[k])
        };
        let a = decode(q.floor() as i32);
        let b = decode(q.floor() as i32 + 1);
        std::array::from_fn(|k| a[k] + (b[k] - a[k]) * (q - q.floor()))
    }

    pub fn has_blend(&self) -> bool {
        self.bytes
            .chunks_exact(STRIDE)
            .any(|tile| tile[4..].chunks_exact(RECORD).any(|r| r[0] & BLEND != 0))
    }

    pub fn gpu_size(&self) -> [u32; 2] {
        let count = (self.cols * self.rows * 2 * SEGMENTS).max(1) as u32;
        let width = count.min(1024);
        [width, count.div_ceil(width)]
    }

    /// One RG32Uint texel per edge segment. The shader needs at most two
    /// coherent loads, including precomputed run-end flags, not byte gathers.
    pub fn pack(&self) -> Packed {
        let size = self.gpu_size();
        let mut bytes = vec![0; size[0] as usize * size[1] as usize * 8];
        for tile in 0..self.cols * self.rows {
            for axis in 0..2 {
                for segment in 0..SEGMENTS {
                    let r = self.record(tile, axis, segment);
                    if r[0] & BLEND == 0 {
                        continue;
                    }
                    let mut flags = r[0];
                    if segment > 0 && self.record(tile, axis, segment - 1)[0] & BLEND != 0 {
                        flags |= PREVIOUS;
                    }
                    if segment + 1 < SEGMENTS
                        && self.record(tile, axis, segment + 1)[0] & BLEND != 0
                    {
                        flags |= NEXT;
                    }
                    let at = ((tile * 2 + axis) * SEGMENTS + segment) * 8;
                    bytes[at..at + 8]
                        .copy_from_slice(&[r[1], r[2], r[3], flags, r[4], r[5], r[6], 0]);
                }
            }
        }
        Packed { size, bytes }
    }

    /// Same smooth segment endpoints as the shader. An excluded segment is
    /// always zero; accepted neighbours only feather inside accepted segments.
    fn edge_strength(&self, tile: usize, axis: usize, along: f32, flag: u8) -> f32 {
        let s = (along / 128.).floor() as usize;
        let has = |i: usize| i < 256 && self.record(tile, axis, i)[0] & flag != 0;
        if !has(s) {
            return 0.;
        }
        let t = (along / 128.).fract();
        let fade = |v: f32| {
            let v = v.clamp(0., 1.);
            v * v * (3. - 2. * v)
        };
        let a = if s > 0 && has(s - 1) { 1. } else { fade(t) };
        let b = if has(s + 1) { 1. } else { fade(1. - t) };
        a.min(b)
    }

    pub fn hedge(&self, x: f64, z: f64) -> f32 {
        if self.cols == 0 || x < 0. || z < 0. {
            return 0.;
        }
        let pos = [x as f32, z as f32];
        let cell = pos.map(|v| (v / TILE).floor() as usize);
        if cell[0] >= self.cols || cell[1] >= self.rows {
            return 0.;
        }
        let mut result = 0_f32;
        for axis in 0..2 {
            let local = pos[axis] % TILE;
            let negative = local < TILE * 0.5;
            if negative && cell[axis] == 0 {
                continue;
            }
            let distance = if negative { local } else { TILE - local };
            if distance >= HALF_WIDTH {
                continue;
            }
            let mut owner = cell;
            if negative {
                owner[axis] -= 1;
            }
            let tile = owner[1] * self.cols + owner[0];
            result = result.max(
                2. * weight(distance) * self.edge_strength(tile, axis, pos[1 - axis] % TILE, HEDGE),
            );
        }
        result
    }
}

/// CPU evaluation of the actual upload bytes. Unlit live-palette colour;
/// source weather remaps stay on the shared shader path.
pub struct Packed {
    pub size: [u32; 2],
    pub bytes: Vec<u8>,
}
impl Packed {
    fn record(&self, tile: usize, axis: usize, segment: usize) -> &[u8] {
        let at = ((tile * 2 + axis) * SEGMENTS + segment) * 8;
        &self.bytes[at..at + 8]
    }
    pub fn correction(&self, map: &Map, world: [f32; 2], palette: &[[f32; 3]; 256]) -> [f32; 3] {
        if map.cols == 0 || world.iter().any(|v| *v < 0.) {
            return [0.; 3];
        }
        let cell = world.map(|v| (v / TILE).floor() as usize);
        if cell[0] >= map.cols || cell[1] >= map.rows {
            return [0.; 3];
        }
        let local = world.map(|v| v % TILE);
        let near = local.map(|v| v.min(TILE - v));
        let axis = usize::from(near[1] < near[0]);
        if near[axis] >= HALF_WIDTH {
            return [0.; 3];
        }
        let negative = local[axis] < TILE / 2.;
        if (negative && cell[axis] == 0)
            || (!negative && cell[axis] + 1 == [map.cols, map.rows][axis])
        {
            return [0.; 3];
        }
        let owner = cell[1] * map.cols + cell[0] - if negative { [1, map.cols][axis] } else { 0 };
        let along = local[1 - axis] / 128.;
        let segment = along.floor() as usize;
        let t = along.fract();
        let current = self.record(owner, axis, segment);
        let flags = current[3];
        if flags & BLEND == 0 {
            return [0.; 3];
        }
        let smooth = |v: f32| v * v * (3. - 2. * v);
        let strength =
            (if flags & PREVIOUS != 0 { 1. } else { smooth(t) }).min(if flags & NEXT != 0 {
                1.
            } else {
                smooth(1. - t)
            });
        let delta = |r: &[u8]| {
            let mean = |off: usize| {
                let a = palette[usize::from(r[off])];
                let b = palette[usize::from(r[off + 1])];
                let t = f32::from(r[off + 2]) / 255.;
                std::array::from_fn::<_, 3, _>(|k| a[k] + (b[k] - a[k]) * t)
            };
            let a = mean(0);
            let b = mean(4);
            std::array::from_fn::<_, 3, _>(|k| b[k] - a[k])
        };
        let mut d = delta(current);
        let (neighbour, w) = if t < 0.5 && flags & PREVIOUS != 0 {
            (segment - 1, 0.5 - t)
        } else if t >= 0.5 && flags & NEXT != 0 {
            (segment + 1, t - 0.5)
        } else {
            (segment, 0.)
        };
        if w > 0. {
            let other = delta(self.record(owner, axis, neighbour));
            d = std::array::from_fn(|k| d[k] + (other[k] - d[k]) * w);
        }
        let w = weight(near[axis]) * strength * if negative { -1. } else { 1. };
        d.map(|v| v * w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn land_fill_shade_does_not_make_an_authored_field_boundary() {
        let (mut world, mut atlas, mut layers, mut pixels) = two_tiles();
        layers.remove(&(0, 0));
        atlas.palette[192] = [90, 160, 80];
        atlas.palette[194] = [20, 70, 24];
        atlas.tone = crate::vegetation::Tone {
            dark: 85,
            light: 110,
            yellow: 0.4,
        };
        pixels[..65536].fill(192);
        pixels[65536..].fill(194);
        let map = Map::build(&world, &atlas, &layers, Some(0), &pixels);
        assert_eq!(
            map.record(0, 0, 100)[0] & (BLEND | HEDGE | FALLBACK),
            BLEND | HEDGE | FALLBACK
        );
        // A deliberate change of the base class is not a fill-colour seam.
        for row in 0..5 {
            world.theater.cells[row * 9 + 4].class = 3;
        }
        assert_eq!(
            Map::build(&world, &atlas, &layers, Some(0), &pixels).record(0, 0, 100)[0] & BLEND,
            0
        );
        for row in 0..5 {
            world.theater.cells[row * 9 + 4].class = 2;
        }
        // Fields painted into authored art keep their rectangular boundary.
        pixels[..65536].fill(194);
        pixels[65536..].fill(192);
        assert_eq!(
            Map::build(&world, &atlas, &layers, Some(0), &pixels).record(0, 0, 100)[0] & BLEND,
            0
        );
    }

    #[test]
    fn packed_gpu_records_reproduce_the_500_ft_live_palette_ramp() {
        let (world, atlas, layers, pixels) = two_tiles();
        let map = Map::build(&world, &atlas, &layers, None, &pixels);
        let packed = map.pack();
        assert_eq!(
            packed.bytes.len(),
            packed.size[0] as usize * packed.size[1] as usize * 8
        );
        for palette in [atlas.palette, atlas.palette.map(|rgb| rgb.map(|v| v / 2))] {
            let palette = palette.map(|rgb| rgb.map(linear));
            for d in [-300., -250., -125., -1., 0., 1., 125., 250., 300.] {
                let p = [32768. + d, 12864.];
                let correction = packed.correction(&map, p, &palette);
                let delta = map.edge_delta(0, 0, p[1], &palette);
                for k in 0..3 {
                    assert!(
                        (correction[k] - delta[k] * weight(d) * if d < 0. { 1. } else { -1. })
                            .abs()
                            < 1e-7
                    );
                }
            }
        }
        // Largest reviewed grid fits the portable 2048-square texture limit.
        let large = Map {
            cols: 64,
            rows: 64,
            ..Default::default()
        };
        assert_eq!(large.gpu_size(), [1024, 2048]);
        assert_eq!(Map::default().pack().bytes, [0; 8]);
    }
    type Fixture = (Terrain, Atlas, BTreeMap<(i32, i32), usize>, Vec<u8>);
    fn two_tiles() -> Fixture {
        let mut world = tore_world::test_support::terrain();
        world.theater.cols = 9;
        world.theater.rows = 5;
        world.theater.cells = vec![
            tore_formats::theater::TerrainCell {
                color: 12,
                class: 2,
                elevation: 0
            };
            45
        ];
        let mut atlas = Atlas::synthetic(vec![12; 65536], 0);
        atlas.palette[13] = [55, 110, 40];
        let layers = BTreeMap::from([((0, 0), 0), ((4, 0), 1)]);
        let pixels = [vec![12; 65536], vec![13; 65536]].concat();
        (world, atlas, layers, pixels)
    }
    #[test]
    fn built_mask_excludes_water_roads_airports_and_rotated_stripes() {
        let (mut world, atlas, layers, mut pixels) = two_tiles();
        let check = |world: &Terrain, pixels: &[u8]| {
            Map::build(world, &atlas, &layers, None, pixels).bytes[4 + 100 * RECORD]
        };
        assert_eq!(check(&world, &pixels), BLEND);
        for index in [255, 40, 20] {
            pixels[65536..].fill(index);
            assert_eq!(check(&world, &pixels), 0);
        }
        pixels[65536..].fill(13);
        let surface = tore_sim::airport::OrientedBox {
            center: [32768., 0., 12864.],
            half: [500., 100., 500.],
            heading: 0.7,
            pitch: 0.,
            bank: 0.,
        };
        world.airport_scene.runways.push(tore_sim::airport::Runway {
            object: 1,
            airport: 1,
            name: "synthetic".into(),
            surface,
            approach_center: surface.center,
            elevation_ft: 0.,
            heading: surface.heading,
            length_ft: 1000.,
        });
        let map = Map::build(&world, &atlas, &layers, None, &pixels);
        assert_eq!(map.bytes[4 + 100 * RECORD], 0);
        assert_eq!(map.bytes[4 + 50 * RECORD], BLEND);
        world.airport_scene.runways.clear();
        world.environment.textures.insert(
            (4, 0),
            tore_formats::theater::TexturePlacement {
                col: 4,
                row: 0,
                texture: 1,
                rotation: 1,
                resource: None,
            },
        );
        // At rotation 1 the north picture edge becomes the west world edge.
        pixels[65536 + 253 * 256..].fill(40);
        assert_eq!(check(&world, &pixels), 0);
    }
    #[test]
    fn different_tile_means_do_not_change_already_matched_edges() {
        let (world, atlas, layers, mut pixels) = two_tiles();
        for row in 0..256 {
            pixels[65536 + row * 256] = 12;
        }
        let map = Map::build(&world, &atlas, &layers, None, &pixels);
        assert_ne!(&map.bytes[..3], &map.bytes[STRIDE..STRIDE + 3]);
        assert_eq!(map.record(0, 0, 100)[0], 0);
    }
    #[test]
    fn field_forest_belt_increases_trees_only_inside_its_band() {
        let (world, _, _, _) = two_tiles();
        let mut atlas = Atlas::synthetic(vec![20; 65536], 0);
        let mut map = Map {
            cols: 2,
            rows: 1,
            bytes: vec![0; 2 * STRIDE],
            mean_error: 0.,
        };
        for s in 80..120 {
            map.bytes[4 + s * RECORD] = HEDGE;
        }
        atlas.seams = std::sync::Arc::new(map);
        let edge = atlas.site(&world, 32760., 12864.);
        let inside = atlas.site(&world, 32000., 12864.);
        assert!(atlas.tree_density(&edge) > 0.8);
        assert_eq!(atlas.tree_density(&inside), 0.);
    }
    #[test]
    fn colour_seams_only_join_the_same_natural_class() {
        assert_eq!(pair_flags(Kind::Grass, Kind::Grass, false, 0.1), BLEND);
        assert_eq!(pair_flags(Kind::Grass, Kind::Grass, false, 0.01), 0);
        assert_eq!(pair_flags(Kind::Grass, Kind::Grass, true, 0.1), 0);
        for kind in [
            Kind::Water,
            Kind::River,
            Kind::Road,
            Kind::Snow,
            Kind::Rock,
            Kind::Sand,
            Kind::Field,
        ] {
            assert_eq!(pair_flags(kind, kind, false, 0.2), 0);
            assert_eq!(pair_flags(Kind::Grass, kind, false, 0.2), 0);
        }
        assert_eq!(pair_flags(Kind::Field, Kind::Forest, false, 0.1), HEDGE);
        assert_eq!(pair_flags(Kind::Field, Kind::Forest, true, 0.1), 0);
        assert_eq!(pair_flags(Kind::Forest, Kind::Grass, false, 0.1), 0);
    }
    #[test]
    fn built_mask_keeps_city_edges_crisp_even_on_green_roof_pixels() {
        use std::sync::Arc;
        use tore_world::city::{
            CityFill, Layout,
            mask::{GREEN, LightMap, PlacedTile, TileMask},
        };
        let (mut world, atlas, layers, pixels) = two_tiles();
        let map = LightMap {
            tiles: BTreeMap::from([(
                (4, 0),
                PlacedTile {
                    texture: "SYNTHETIC.PIC".into(),
                    rotation: 0,
                    mask: Arc::new(TileMask {
                        flags: vec![GREEN; 65536],
                    }),
                },
            )]),
            districts: vec![],
        };
        world.city_fill = Some(Arc::new(CityFill::assemble(
            Arc::new(Layout {
                layout: "SYNTHETIC.MM".into(),
                map,
                kit: Default::default(),
                cities: vec![],
                instances: vec![],
                candidates: 0,
                inputs: vec![],
            }),
            vec![],
            vec![],
        )));
        let map = Map::build(&world, &atlas, &layers, None, &pixels);
        assert_eq!(map.bytes[4 + 100 * RECORD], 0);
    }
    #[test]
    fn city_art_keeps_its_green_margin_when_city_fill_is_off() {
        let (world, mut atlas, layers, mut pixels) = two_tiles();
        assert!(world.city_fill.is_none());
        atlas.palette[200] = [150; 3];
        atlas.palette[205] = [50; 3];
        for y in 0..256 {
            for x in 32..224 {
                pixels[65536 + y * 256 + x] = if (x + y) % 2 == 0 { 200 } else { 205 };
            }
        }
        let map = Map::build(&world, &atlas, &layers, None, &pixels);
        assert_eq!(map.record(0, 0, 100)[0], 0);
    }
    #[test]
    fn gradient_is_500_feet_centred_and_preserves_detail() {
        for d in [250., 251., 500.] {
            assert_eq!(weight(d), 0.);
        }
        assert_eq!(weight(0.), 0.5);
        assert!((weight(125.) - 0.15625).abs() < 1e-6);
        let (a, b, grain) = (0.1, 0.5, 0.012);
        let left = a + grain + (b - a) * weight(0.);
        let right = b + grain + (a - b) * weight(0.);
        assert!((left - right).abs() < 1e-6);
        assert!((left - 0.3 - grain).abs() < 1e-6);
    }
    #[test]
    fn shared_hedge_mask_stops_at_exclusions_and_band_endpoints() {
        let mut m = Map {
            cols: 2,
            rows: 1,
            bytes: vec![0; 2 * STRIDE],
            mean_error: 0.,
        };
        for segment in 80..84 {
            m.bytes[4 + segment * RECORD] = HEDGE;
        }
        for d in [-249., -125., 0., 125., 249.] {
            assert!(m.hedge(32768. + d, 82.5 * 128.) > 0.);
        }
        for d in [-251., -250., 250., 251.] {
            assert_eq!(m.hedge(32768. + d, 82.5 * 128.), 0.);
        }
        assert_eq!(m.hedge(32768., 79.9 * 128.), 0.);
        assert_eq!(m.hedge(32768., 84. * 128.), 0.);
    }
    #[test]
    fn means_ignore_cutouts_and_resolve_through_live_palette() {
        let mut p = [[0; 3]; 256];
        p[10] = [32, 80, 24];
        p[20] = [96, 160, 64];
        p[255] = [255; 3];
        let (exact, encoded, error) = mean(&[10, 10, 20, 20, 255], &p);
        assert!(error < 0.001);
        for k in 0..3 {
            assert!((exact[k] - (linear(p[10][k]) + linear(p[20][k])) * 0.5).abs() < 1e-6);
        }
        assert_ne!(encoded[0], 255);
        assert_ne!(encoded[1], 255);
    }
}
