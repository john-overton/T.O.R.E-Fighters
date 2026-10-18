//! Opinionated flat-tile enrichment, John 2026-10-18. Original art is retained.
//! Statistics and world-space modulation are derived once, then cached.
use super::{Map, TILE, baked, linear};
use crate::{
    terrain::Terrain,
    vegetation::{Atlas, Kind},
};
use std::collections::BTreeMap;

pub const SIDE: usize = 96;
pub const PATCH_BYTES: usize = SIDE * SIDE * 8;
pub const SCALES: [usize; 4] = [2, 4, 8, 16];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Flat {
    /// RGB mean, four low-pass SDs, RGB/luminance regression, green coverage,
    /// then fitted noise scale, small share, mean and amplitude.
    pub pages: Vec<[f32; 15]>,
    /// Target source page + 1, zero for pictures already carrying mottling.
    pub targets: Vec<u32>,
    /// Modulation patch + 1 per world tile, zero for unchanged tiles.
    pub cells: Vec<u32>,
    /// Fade at west/east/south/north edges unless a flat neighbour shares style.
    pub boundaries: Vec<u8>,
}

pub fn luminance(c: [f32; 3]) -> f32 {
    c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722
}
fn green(i: u8) -> bool {
    matches!(i,192..=197|208..=215)
}
fn sd(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.;
    }
    let mean = values.iter().map(|&v| f64::from(v)).sum::<f64>() / values.len() as f64;
    (values
        .iter()
        .map(|&v| (f64::from(v) - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64)
        .sqrt() as f32
}

pub fn measure(pixels: &[u8], palette: &[[u8; 3]; 256]) -> [f32; 15] {
    let colors = palette.map(|c| c.map(linear));
    let image: Vec<_> = pixels.iter().map(|&i| colors[usize::from(i)]).collect();
    let valid: Vec<_> = pixels.iter().map(|&i| green(i)).collect();
    measure_rgb(&image, &valid)
}

pub fn measure_rgb(image: &[[f32; 3]], valid_pixels: &[bool]) -> [f32; 15] {
    let mut result = [0.; 15];
    result[10] = valid_pixels.iter().filter(|&&v| v).count() as f32 / 65536.;
    for (level, step) in SCALES.into_iter().enumerate() {
        let mut blocks = Vec::new();
        for y in (0..256).step_by(step) {
            for x in (0..256).step_by(step) {
                let mut sum = [0.; 3];
                let mut valid = 0;
                for v in y..y + step {
                    for u in x..x + step {
                        let index = v * 256 + u;
                        if valid_pixels[index] {
                            valid += 1;
                            for k in 0..3 {
                                sum[k] += image[index][k];
                            }
                        }
                    }
                }
                // Roads, rivers and fields cannot manufacture a flatness score.
                if valid == step * step {
                    blocks.push(sum.map(|v| v / valid as f32));
                }
            }
        }
        let l: Vec<_> = blocks.iter().map(|&c| luminance(c)).collect();
        result[3 + level] = sd(&l);
        if level == 1 && !blocks.is_empty() {
            for k in 0..3 {
                result[k] = blocks.iter().map(|c| c[k]).sum::<f32>() / blocks.len() as f32;
            }
            let mean = luminance([result[0], result[1], result[2]]);
            let variance = result[4] * result[4];
            if variance > 1e-10 {
                for k in 0..3 {
                    result[7 + k] = blocks
                        .iter()
                        .zip(&l)
                        .map(|(c, &y)| (c[k] - result[k]) * (y - mean))
                        .sum::<f32>()
                        / (blocks.len() as f32 * variance);
                }
            }
        }
    }
    result
}

fn noise(world: [f32; 2], scale: f32, small: f32) -> f32 {
    // No period or tile-local seed: the field is stable across the whole world.
    let warp = [
        crate::vegetation::detail_noise(world.map(|v| v / 8192.), 0, 411),
        crate::vegetation::detail_noise(world.map(|v| v / 8192.), 0, 412),
    ];
    let p = std::array::from_fn::<_, 2, _>(|k| world[k] + (warp[k] - 0.5) * scale);
    crate::vegetation::detail_noise(p.map(|v| v / scale), 0, 413) * (1. - small)
        + crate::vegetation::detail_noise(p.map(|v| v / (scale / 4.)), 0, 414) * small
}
fn grid_sample(grid: &[f32], uv: [f32; 2]) -> f32 {
    let p = uv.map(|v| v.clamp(0., 1.) * (SIDE - 1) as f32);
    let b = p.map(|v| v.floor() as usize);
    let f = p.map(|v| v - v.floor());
    let mut sum = 0.;
    for y in 0..2 {
        for x in 0..2 {
            sum += grid[(b[1] + y).min(SIDE - 1) * SIDE + (b[0] + x).min(SIDE - 1)]
                * if x == 0 { 1. - f[0] } else { f[0] }
                * if y == 0 { 1. - f[1] } else { f[1] };
        }
    }
    sum
}
fn fit(target: &mut [f32; 15]) {
    let mut best = (f32::INFINITY, [0.; 4]);
    for scale in [512., 1024., 2048., 4096., 8192.] {
        for small in [0., 0.25, 0.5] {
            let grid: Vec<_> = (0..SIDE * SIDE)
                .map(|i| {
                    noise(
                        [
                            658432. + (i % SIDE) as f32 * TILE / (SIDE - 1) as f32,
                            319488. + (i / SIDE) as f32 * TILE / (SIDE - 1) as f32,
                        ],
                        scale,
                        small,
                    )
                })
                .collect();
            let image: Vec<_> = (0..65536)
                .map(|i| grid_sample(&grid, [(i % 256) as f32 / 255., (i / 256) as f32 / 255.]))
                .collect();
            let mut deviations = [0.; 4];
            for (k, step) in SCALES.into_iter().enumerate() {
                let mut means = Vec::new();
                for y in (0..256).step_by(step) {
                    for x in (0..256).step_by(step) {
                        let mut sum = 0.;
                        for v in y..y + step {
                            for u in x..x + step {
                                sum += image[v * 256 + u];
                            }
                        }
                        means.push(sum / (step * step) as f32);
                    }
                }
                deviations[k] = sd(&means);
            }
            let amplitude = target[4] / deviations[1].max(1e-6);
            let error = (0..4)
                .map(|k| {
                    ((deviations[k] * amplitude + 1e-6) / (target[3 + k] + 1e-6))
                        .ln()
                        .powi(2)
                })
                .sum::<f32>();
            if error < best.0 {
                best = (
                    error,
                    [scale, small, image.iter().sum::<f32>() / 65536., amplitude],
                );
            }
        }
    }
    target[11..15].copy_from_slice(&best.1);
}

/// Summed-area feature mask. A 512 ft guard exceeds the filtered field's
/// 345 ft support, so painted features receive exactly zero modulation.
struct Protected(Vec<u32>);
pub fn feature_mask(pixels: &[u8], atlas: &Atlas) -> Vec<bool> {
    let kinds: Vec<_> = pixels.iter().map(|&i| atlas.kind_index(i)).collect();
    (0..65536)
        .map(|i| match kinds[i] {
            Kind::Grass | Kind::Forest | Kind::Scrub => false,
            // Baltic fine green grain crosses the fitted Field tone threshold.
            // A painted green field has a coherent core; brown crop art is always
            // protected. The 512 ft dilation also protects the core's boundary.
            Kind::Field if green(pixels[i]) => {
                let x = i % 256;
                let y = i / 256;
                (y.saturating_sub(1)..=(y + 1).min(255)).all(|v| {
                    (x.saturating_sub(1)..=(x + 1).min(255))
                        .all(|u| kinds[v * 256 + u] == Kind::Field)
                })
            }
            _ => true,
        })
        .collect()
}
impl Protected {
    fn new(pixels: &[u8], atlas: &Atlas) -> Self {
        let features = feature_mask(pixels, atlas);
        let mut p = vec![0; 257 * 257];
        for y in 0..256 {
            for x in 0..256 {
                let bad = u32::from(features[y * 256 + x]);
                p[(y + 1) * 257 + x + 1] =
                    bad + p[y * 257 + x + 1] + p[(y + 1) * 257 + x] - p[y * 257 + x];
            }
        }
        Self(p)
    }
    fn blocked(&self, uv: [f32; 2], rotation: u32) -> bool {
        let (x, y) = crate::vegetation::rotate(
            (uv[0] * 256.).clamp(0., 255.) as usize,
            (uv[1] * 256.).clamp(0., 255.) as usize,
            rotation as u8,
        );
        let x0 = x.saturating_sub(4);
        let y0 = y.saturating_sub(4);
        let x1 = (x + 5).min(256);
        let y1 = (y + 5).min(256);
        self.0[y1 * 257 + x1] + self.0[y0 * 257 + x0]
            != self.0[y0 * 257 + x1] + self.0[y1 * 257 + x0]
    }
}

impl Flat {
    pub fn build(
        map: &Map,
        world: &Terrain,
        atlas: &Atlas,
        land: Option<usize>,
        pixels: &[u8],
    ) -> (Self, Vec<u8>) {
        let mut out = Self {
            pages: pixels
                .chunks_exact(65536)
                .map(|p| measure(p, &atlas.palette))
                .collect(),
            targets: vec![0; pixels.len() / 65536],
            cells: vec![0; map.cols * map.rows],
            boundaries: vec![0; map.cols * map.rows],
        };
        let city_classes = tore_world::city::mask::Classes::from_palette(&atlas.palette);
        let city: Vec<_> = pixels
            .chunks_exact(65536)
            .map(|p| tore_world::city::mask::is_city_tile(p, &city_classes))
            .collect();
        let eligible = |p: usize| {
            out.pages[p][10] >= 0.6
                && luminance([out.pages[p][0], out.pages[p][1], out.pages[p][2]]) > 0.005
                && !city[p]
        };
        let reference = land
            .filter(|&p| eligible(p) && out.pages[p][4] > 0.003)
            .or_else(|| {
                (0..out.pages.len())
                    .filter(|&p| eligible(p))
                    .max_by(|&a, &b| out.pages[a][4].total_cmp(&out.pages[b][4]))
            });
        let Some(reference) = reference.filter(|&p| out.pages[p][4] > 0.003) else {
            return (out, Vec::new());
        };
        let low: Vec<_> = (0..out.pages.len())
            .map(|p| eligible(p) && (3..7).all(|k| out.pages[p][k] < out.pages[reference][k] * 0.5))
            .collect();
        let mut counts: Vec<BTreeMap<usize, usize>> = vec![BTreeMap::new(); out.pages.len()];
        for tile in 0..map.cols * map.rows {
            let p = map.material(tile)[0];
            if p == 0 || !low[p as usize - 1] {
                continue;
            }
            for neighbour in neighbours(map, tile).into_iter().flatten() {
                let q = map.material(neighbour)[0];
                if q != 0
                    && eligible(q as usize - 1)
                    && !low[q as usize - 1]
                    && out.pages[q as usize - 1][4] >= out.pages[reference][4] * 0.7
                {
                    *counts[p as usize - 1].entry(q as usize - 1).or_default() += 1;
                }
            }
        }
        for (p, flat) in low.into_iter().enumerate() {
            if flat {
                out.targets[p] = counts[p]
                    .iter()
                    .max_by_key(|(p, n)| (**n, std::cmp::Reverse(**p)))
                    .map_or(reference, |(&p, _)| p) as u32
                    + 1;
            }
        }
        let mut fitted = std::collections::BTreeSet::new();
        for target in &out.targets {
            if *target != 0 && fitted.insert(*target) {
                fit(&mut out.pages[*target as usize - 1]);
            }
        }
        let masks: Vec<_> = pixels
            .chunks_exact(65536)
            .map(|p| Protected::new(p, atlas))
            .collect();
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
            b.half[0] += 860.;
            b.half[2] += 860.;
        }
        let mut patches = Vec::new();
        for tile in 0..map.cols * map.rows {
            let material = map.material(tile);
            if material[0] == 0 {
                continue;
            }
            let page = material[0] as usize - 1;
            let target = out.targets[page];
            if target == 0 {
                continue;
            }
            let key = ((tile % map.cols * 4) as i32, (tile / map.cols * 4) as i32);
            if world
                .city_fill
                .as_ref()
                .is_some_and(|f| f.layout.map.tiles.contains_key(&key))
            {
                continue;
            }
            let source = out.pages[page];
            let target = out.pages[target as usize - 1];
            let origin = [
                (tile % map.cols) as f32 * TILE,
                (tile / map.cols) as f32 * TILE,
            ];
            let first = patches.len();
            let mut changed = false;
            for y in 0..SIDE {
                for x in 0..SIDE {
                    let uv = [x as f32 / (SIDE - 1) as f32, y as f32 / (SIDE - 1) as f32];
                    let p = std::array::from_fn::<_, 2, _>(|k| origin[k] + uv[k] * TILE);
                    let blocked = masks[page].blocked(uv, material[1])
                        || [-512, 0, 512].into_iter().any(|dx| {
                            [-512, 0, 512]
                                .into_iter()
                                .any(|dz| atlas.coast_blocked(p[0] as i64 + dx, p[1] as i64 + dz))
                        })
                        || bounds
                            .iter()
                            .any(|b| b.contains_horizontal(f64::from(p[0]), f64::from(p[1])))
                        || world.city_fill.as_ref().is_some_and(|f| {
                            f.layout.map.districts.iter().any(|d| {
                                let r = d.half[0].max(d.half[1]) + 640;
                                (p[0] as i64 - d.center[0]).pow(2)
                                    + (p[1] as i64 - d.center[1]).pow(2)
                                    <= r * r
                            })
                        });
                    let n = (noise(p, target[11], target[12]) - target[13]) * target[14];
                    let rgb = std::array::from_fn::<_, 3, _>(|k| {
                        if blocked {
                            0.
                        } else {
                            target[k] - source[k] + target[7 + k] * n
                        }
                    });
                    changed |= rgb.iter().any(|v| v.abs() > 1e-7);
                    for v in rgb
                        .into_iter()
                        .chain([luminance([source[0], source[1], source[2]]).max(1e-6)])
                    {
                        patches.extend_from_slice(&baked::half(v).to_le_bytes());
                    }
                }
            }
            if changed {
                out.cells[tile] = (first / PATCH_BYTES + 1) as u32;
            } else {
                patches.truncate(first);
            }
        }
        for tile in 0..out.cells.len() {
            if out.cells[tile] != 0 {
                let own = out.targets[map.material(tile)[0] as usize - 1];
                for (side, neighbour) in neighbours(map, tile).into_iter().enumerate() {
                    if neighbour.is_none_or(|n| {
                        out.cells[n] == 0 || out.targets[map.material(n)[0] as usize - 1] != own
                    }) {
                        out.boundaries[tile] |= 1 << side;
                    }
                }
            }
        }
        (out, patches)
    }

    pub fn apply_raw(
        &self,
        fields: &[u8],
        tile: usize,
        uv: [f32; 2],
        source: [f32; 3],
    ) -> [f32; 3] {
        let slot = self.cells.get(tile).copied().unwrap_or(0);
        if slot == 0 || fields.is_empty() {
            return source;
        }
        let response = sample(
            &fields[(slot as usize - 1) * PATCH_BYTES..slot as usize * PATCH_BYTES],
            uv,
        );
        apply(source, response, fade(uv, self.boundaries[tile]))
    }
}
fn neighbours(map: &Map, tile: usize) -> [Option<usize>; 4] {
    [
        (!tile.is_multiple_of(map.cols)).then(|| tile - 1),
        (tile % map.cols + 1 < map.cols).then_some(tile + 1),
        (tile >= map.cols).then(|| tile - map.cols),
        (tile / map.cols + 1 < map.rows).then_some(tile + map.cols),
    ]
}
pub fn sample(pixels: &[u8], uv: [f32; 2]) -> [f32; 4] {
    let p = uv.map(|v| v.clamp(0., 1.) * (SIDE - 1) as f32);
    let b = p.map(|v| v.floor() as usize);
    let f = p.map(|v| v - v.floor());
    let mut sum = [0.; 4];
    for y in 0..2 {
        for x in 0..2 {
            let at = ((b[1] + y).min(SIDE - 1) * SIDE + (b[0] + x).min(SIDE - 1)) * 8;
            let w = if x == 0 { 1. - f[0] } else { f[0] } * if y == 0 { 1. - f[1] } else { f[1] };
            for k in 0..4 {
                sum[k] += baked::float(u16::from_le_bytes(
                    pixels[at + k * 2..at + k * 2 + 2].try_into().unwrap(),
                )) * w;
            }
        }
    }
    sum
}
pub fn fade(uv: [f32; 2], flags: u8) -> f32 {
    [uv[0], 1. - uv[0], uv[1], 1. - uv[1]]
        .into_iter()
        .enumerate()
        .filter(|(i, _)| flags & (1 << i) != 0)
        .map(|(_, d)| {
            let t = (d * TILE / 250.).clamp(0., 1.);
            t * t * (3. - 2. * t)
        })
        .fold(1., f32::min)
}
pub fn apply(source: [f32; 3], response: [f32; 4], fade: f32) -> [f32; 3] {
    let scale = luminance(source) / response[3].max(1e-6) * fade;
    std::array::from_fn(|k| (source[k] + response[k] * scale).max(0.))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coherent_painted_fields_are_protected_but_green_fine_grain_is_retained() {
        let mut atlas = Atlas::synthetic(vec![196; 65536], 0);
        atlas.palette[196] = [28, 89, 69];
        atlas.palette[195] = [32, 93, 73];
        atlas.tone = crate::vegetation::Tone {
            dark: 68,
            light: 72,
            yellow: 0.34,
        };
        let mut pixels = vec![196; 65536];
        for y in (0..256).step_by(3) {
            for x in (0..256).step_by(3) {
                pixels[y * 256 + x] = 195;
            }
        }
        for y in 100..109 {
            for x in 100..109 {
                pixels[y * 256 + x] = 195;
            }
        }
        let features = feature_mask(&pixels, &atlas);
        assert!(!features[30 * 256 + 30]);
        assert!(features[104 * 256 + 104]);
        let protected = Protected::new(&pixels, &atlas);
        // World V is flipped for rotation zero. The painted field's outer
        // row is protected by the coherent-core margin as well as its centre.
        assert!(protected.blocked([100.5 / 256., (255. - 104. + 0.5) / 256.], 0));
    }
    #[test]
    fn world_noise_is_stable_and_does_not_repeat_at_tile_spacing() {
        let p = [12345., 45678.];
        assert_eq!(noise(p, 2048., 0.25), noise(p, 2048., 0.25));
        assert_ne!(
            noise(p, 2048., 0.25),
            noise([p[0] + TILE, p[1]], 2048., 0.25)
        );
        assert_eq!(fade([0., 0.5], 1), 0.);
        assert_eq!(fade([250. / TILE, 0.5], 1), 1.);
        assert_eq!(fade([0., 0.5], 0), 1.);
    }
}
