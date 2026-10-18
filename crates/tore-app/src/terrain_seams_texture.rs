//! Runtime statistical synthesis, never a spatial copy of imported artwork.
//! Fitted presentation: periodic world-space noise, histogram and variogram fit.
use super::{FALLBACK, Map, TILE, linear};
use std::collections::BTreeSet;

pub const LAGS: [usize; 6] = [2, 4, 8, 16, 32, 64];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mottle {
    /// Mean RGB, luminance SD, six RMS increments, dark RGB, light RGB.
    pub statistics: [[f32; 16]; 2],
    /// Noise cell size in feet and small-octave share.
    pub parameters: [f32; 2],
    pub source_pages: usize,
    pub source: Vec<u8>,
    pub pixels: Vec<u8>,
}

fn green(i: u8) -> bool {
    matches!(i, 192..=197 | 208..=215)
}
fn luminance(rgb: [f32; 3]) -> f32 {
    rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722
}

pub fn statistics(pages: &[&[u8]], palette: &[[u8; 3]; 256]) -> [f32; 16] {
    let colors = palette.map(|p| p.map(linear));
    let lum = colors.map(luminance);
    let mut histogram = [0_u64; 256];
    let mut increments = [0.; 6];
    let mut counts = [0_u64; 6];
    for page in pages {
        for y in 0..256 {
            for x in 0..256 {
                let a = page[y * 256 + x];
                if !green(a) {
                    continue;
                }
                histogram[usize::from(a)] += 1;
                for (k, lag) in LAGS.into_iter().enumerate() {
                    for (u, v) in [(x + lag, y), (x, y + lag)] {
                        if u < 256 && v < 256 {
                            let b = page[v * 256 + u];
                            if green(b) {
                                increments[k] +=
                                    f64::from(lum[usize::from(b)] - lum[usize::from(a)]).powi(2);
                                counts[k] += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    let total = histogram.iter().sum::<u64>().max(1) as f64;
    let mut out = [0.; 16];
    for k in 0..3 {
        out[k] = (0..256)
            .map(|i| f64::from(colors[i][k]) * histogram[i] as f64 / total)
            .sum::<f64>() as f32;
    }
    let mean = luminance([out[0], out[1], out[2]]);
    out[3] = (histogram
        .iter()
        .enumerate()
        .map(|(i, &n)| f64::from(lum[i] - mean).powi(2) * n as f64 / total)
        .sum::<f64>())
    .sqrt() as f32;
    for k in 0..6 {
        out[4 + k] = (increments[k] / counts[k].max(1) as f64).sqrt() as f32;
    }
    // Average colours in the lower/upper luminance halves. These retain the
    // measured chroma, rather than synthesizing grey light/dark patches.
    for (side, range) in [(0, 10..13), (1, 13..16)] {
        let chosen: Vec<_> = (0..256)
            .filter(|&i| (lum[i] >= mean) == (side == 1))
            .collect();
        let n = chosen.iter().map(|&i| histogram[i]).sum::<u64>().max(1) as f64;
        for (k, slot) in range.enumerate() {
            out[slot] = chosen
                .iter()
                .map(|&i| f64::from(colors[i][k]) * histogram[i] as f64 / n)
                .sum::<f64>() as f32;
        }
    }
    out
}

fn synthesize(histogram: &[u64; 256], palette: &[[u8; 3]; 256], scale: f32, small: f32) -> Vec<u8> {
    let mut ranked: Vec<_> = (0..65536)
        .map(|i| {
            // Both endpoints have identical values. Clamped page sampling then
            // stays continuous where world-aligned fallback tiles meet.
            let p = [
                (i % 256) as f32 / 255. * TILE,
                (i / 256) as f32 / 255. * TILE,
            ];
            let noise = |s, salt| {
                crate::vegetation::detail_noise(p.map(|v| v / s), (TILE / s) as i32, salt)
            };
            let value = noise(scale, 401) * (1. - small) + noise(scale / 4., 402) * small;
            (value, i)
        })
        .collect();
    ranked.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut indices: Vec<_> = (0..256).filter(|&i| histogram[i] != 0).collect();
    indices.sort_by(|&a, &b| {
        luminance(palette[a].map(linear)).total_cmp(&luminance(palette[b].map(linear)))
    });
    let total = histogram.iter().sum::<u64>();
    let mut out = vec![0; 65536];
    let mut rank = 0;
    let mut cumulative = 0;
    for i in indices {
        cumulative += histogram[i];
        let end = ((cumulative * 65536 + total / 2) / total) as usize;
        while rank < end {
            out[ranked[rank].1] = i as u8;
            rank += 1;
        }
    }
    // Equal-valued endpoint noise can straddle a histogram quantile. Enforce
    // identical borders explicitly, without copying any source-art pixels.
    for y in 0..256 {
        out[y * 256 + 255] = out[y * 256];
    }
    for x in 0..256 {
        out[255 * 256 + x] = out[x];
    }
    out
}

impl Mottle {
    pub fn build(map: &Map, land: Option<usize>, pixels: &[u8], palette: &[[u8; 3]; 256]) -> Self {
        let Some(land) = land else {
            return Self::default();
        };
        let source = &pixels[land * 65536..(land + 1) * 65536];
        // Generic green fill only. Sand, snow, water and authored features
        // never become woodland merely because this theater has woodland.
        // LAND has occasional neutral flecks (30 of 65,536 in the reviewed
        // source). They are fine grain, not an authored road or shoreline.
        if source.contains(&255)
            || source.iter().filter(|&&i| green(i)).count() * 100 < source.len() * 99
        {
            return Self::default();
        }
        let mut pages = BTreeSet::new();
        for tile in 0..map.cols * map.rows {
            for axis in 0..2 {
                if (0..256).any(|s| map.record(tile, axis, s)[0] & FALLBACK != 0) {
                    for t in [tile, tile + if axis == 0 { 1 } else { map.cols }] {
                        let p = map.material(t)[0] as usize;
                        if p != 0 && p - 1 != land {
                            pages.insert(p - 1);
                        }
                    }
                }
            }
        }
        if pages.is_empty() {
            return Self::default();
        }
        let pages: Vec<_> = pages
            .into_iter()
            .map(|p| &pixels[p * 65536..(p + 1) * 65536])
            .collect();
        let measured = statistics(&pages, palette);
        let mut histogram = [0_u64; 256];
        for page in &pages {
            for &i in *page {
                if green(i) {
                    histogram[usize::from(i)] += 1;
                }
            }
        }
        if histogram.iter().sum::<u64>() == 0 {
            return Self::default();
        }
        let mut best = (f32::INFINITY, Vec::new(), [0.; 16], [0.; 2]);
        for scale in [512., 1024., 2048., 4096., 8192., 16384.] {
            for small in [0., 0.25, 0.5, 0.75] {
                let candidate = synthesize(&histogram, palette, scale, small);
                let stats = statistics(&[&candidate], palette);
                let error: f32 = (4..10)
                    .map(|k| ((stats[k] + 1e-6) / (measured[k] + 1e-6)).ln().powi(2))
                    .sum();
                if error < best.0 {
                    best = (error, candidate, stats, [scale, small]);
                }
                if measured[3] < 1e-6 {
                    break;
                }
            }
            if measured[3] < 1e-6 {
                break;
            }
        }
        Self {
            statistics: [measured, best.2],
            parameters: best.3,
            source_pages: pages.len(),
            source: source.to_vec(),
            pixels: best.1,
        }
    }
}

/// Shared fitted residual blend: up to 80 ft boundary displacement, tapered
/// to zero at +/-250 ft. Mean colours retain the unperturbed 500 ft ramp.
pub fn blend_weight(signed_distance: f32, noise: f32) -> f32 {
    let d = signed_distance + 80. * noise * (1. - (signed_distance / 250.).powi(2)).max(0.);
    let t = ((d + 250.) / 500.).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthesis_is_statistical_periodic_and_deterministic() {
        let mut palette = [[0; 3]; 256];
        palette[192] = [40, 80, 30];
        palette[193] = [60, 130, 70];
        let mut hist = [0; 256];
        hist[192] = 1;
        hist[193] = 3;
        let a = synthesize(&hist, &palette, 4096., 0.25);
        assert_eq!(a, synthesize(&hist, &palette, 4096., 0.25));
        assert!((a.iter().filter(|&&i| i == 192).count() as f32 / 65536. - 0.25).abs() < 0.01);
        for i in 0..256 {
            assert_eq!(a[i * 256], a[i * 256 + 255]);
            assert_eq!(a[i], a[255 * 256 + i]);
        }
        let s = statistics(&[&a], &palette);
        assert!(s[4] < s[7] && s[7] < s[9]);
    }
    #[test]
    fn noisy_texture_transition_has_exact_500_ft_support() {
        for noise in [-1., -0.3, 0.5, 1.] {
            assert_eq!(blend_weight(-250., noise), 0.);
            assert_eq!(blend_weight(250., noise), 1.);
            assert!((blend_weight(0., noise) - 0.5).abs() < 0.24);
        }
        assert_ne!(blend_weight(0., -0.3), blend_weight(0., 0.5));
    }
}
