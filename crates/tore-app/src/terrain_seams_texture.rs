//! Source contrast statistics and the fitted seam-boundary weight.
use super::linear;

pub const LAGS: [usize; 6] = [2, 4, 8, 16, 32, 64];

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
    fn noisy_texture_transition_has_exact_500_ft_support() {
        for noise in [-1., -0.3, 0.5, 1.] {
            assert_eq!(blend_weight(-250., noise), 0.);
            assert_eq!(blend_weight(250., noise), 1.);
            assert!((blend_weight(0., noise) - 0.5).abs() < 0.24);
        }
        assert_ne!(blend_weight(0., -0.3), blend_weight(0., 0.5));
    }
}
