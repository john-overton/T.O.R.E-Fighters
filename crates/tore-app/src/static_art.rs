//! Full-resolution indexed scenery images, split into bounded GPU pages.
use crate::AppResult;
use tore_formats::Pic;

#[derive(Clone, Copy)]
pub struct Image {
    pub first: usize,
    pub width: usize,
    pub height: usize,
}

impl Image {
    pub fn append(pic: &Pic, pages: &mut Vec<u8>, first: usize) -> AppResult<Self> {
        if !pic.palette.is_empty() {
            return Err("scenery image overrides the world palette".into());
        }
        let count = pic.width.div_ceil(256) * pic.height.div_ceil(256);
        if first + count > 4096 {
            return Err("scenery artwork exceeds portable page budget".into());
        }
        for py in 0..pic.height.div_ceil(256) {
            for px in 0..pic.width.div_ceil(256) {
                let mut page = vec![255; 65536];
                for y in 0..256.min(pic.height - py * 256) {
                    for x in 0..256.min(pic.width - px * 256) {
                        let at = (py * 256 + y) * pic.width + px * 256 + x;
                        if pic.mask[at] {
                            page[y * 256 + x] = pic.pixels[at];
                        }
                    }
                }
                pages.extend(page);
            }
        }
        Ok(Self {
            first,
            width: pic.width,
            height: pic.height,
        })
    }

    /// Split at page boundaries in texture space, retaining interpolated world
    /// positions. This changes storage, not the shape or its source resolution.
    pub fn triangle(
        &self,
        mut triangle: [[f32; 10]; 3],
        out: &mut Vec<f32>,
        limit: usize,
    ) -> AppResult<()> {
        for v in &mut triangle {
            v[3] = v[3].clamp(0., self.width as f32);
            v[4] = v[4].clamp(0., self.height as f32);
        }
        let range = |axis: usize, size: usize| {
            let min = triangle
                .iter()
                .map(|p| p[axis])
                .fold(f32::INFINITY, f32::min);
            let max = triangle
                .iter()
                .map(|p| p[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            let last = size.div_ceil(256) - 1;
            ((min / 256.).floor() as usize).min(last)..=((max / 256.).floor() as usize).min(last)
        };
        for py in range(4, self.height) {
            for px in range(3, self.width) {
                let mut polygon = triangle.to_vec();
                for (axis, edge, greater) in [
                    (3, (px * 256) as f32, true),
                    (3, ((px + 1) * 256) as f32, false),
                    (4, (py * 256) as f32, true),
                    (4, ((py + 1) * 256) as f32, false),
                ] {
                    polygon = clip(&polygon, axis, edge, greater);
                }
                if polygon.len() < 3 {
                    continue;
                }
                for i in 1..polygon.len() - 1 {
                    if out.len().saturating_add(30) > limit {
                        return Err("static scene exceeds 32 MiB geometry budget".into());
                    }
                    for mut v in [polygon[0], polygon[i], polygon[i + 1]] {
                        v[3] = (v[3] - (px * 256) as f32) / 256.;
                        v[4] = (v[4] - (py * 256) as f32) / 256.;
                        v[5] = (self.first + py * self.width.div_ceil(256) + px) as f32;
                        out.extend(v);
                    }
                }
            }
        }
        Ok(())
    }
}

fn clip(input: &[[f32; 10]], axis: usize, edge: f32, greater: bool) -> Vec<[f32; 10]> {
    let mut out = Vec::new();
    let Some(mut a) = input.last().copied() else {
        return out;
    };
    let inside = |v: &[f32; 10]| {
        if greater {
            v[axis] >= edge
        } else {
            v[axis] <= edge
        }
    };
    for &b in input {
        if inside(&a) != inside(&b) {
            let t = (edge - a[axis]) / (b[axis] - a[axis]);
            let mut v = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t);
            v[axis] = edge;
            // Keep the discrete source palette/fog attributes, never average indices.
            v[9] = if t < 0.5 { a[9] } else { b[9] };
            out.push(v);
        }
        if inside(&b) {
            out.push(b);
        }
        a = b;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pages_preserve_large_non_square_image_and_mask() {
        let mut pic = Pic {
            width: 513,
            height: 259,
            pixels: vec![0; 513 * 259],
            mask: vec![true; 513 * 259],
            palette: vec![],
            glyphs: vec![],
        };
        for (i, p) in pic.pixels.iter_mut().enumerate() {
            *p = (i % 251) as u8;
        }
        pic.mask[256] = false;
        let mut pages = Vec::new();
        Image::append(&pic, &mut pages, 17).unwrap();
        assert_eq!(pages.len(), 6 * 65536);
        for y in 0..259 {
            for x in 0..513 {
                let at = ((y / 256) * 3 + x / 256) * 65536 + (y % 256) * 256 + x % 256;
                assert_eq!(
                    pages[at],
                    if pic.mask[y * 513 + x] {
                        pic.pixels[y * 513 + x]
                    } else {
                        255
                    }
                );
            }
        }
        assert_eq!(pages[2 * 65536 + 1], 255);
    }
    #[test]
    fn page_clipping_retains_triangle_area_and_uv_mapping() {
        let art = Image {
            first: 20,
            width: 512,
            height: 512,
        };
        let vertex = |x, y| [x, 0., y, x, y, 0., 0., 0., 0., 200.];
        let mut out = Vec::new();
        art.triangle(
            [vertex(0., 0.), vertex(512., 0.), vertex(0., 512.)],
            &mut out,
            3000,
        )
        .unwrap();
        let mut area = 0.;
        for tri in out.chunks_exact(30) {
            let (a, b, c) = (&tri[..10], &tri[10..20], &tri[20..]);
            area += ((b[0] - a[0]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[0] - a[0])).abs() * 0.5;
            for v in tri.chunks_exact(10) {
                assert!((0. ..=1.).contains(&v[3]) && (0. ..=1.).contains(&v[4]));
                let page = v[5] as usize - 20;
                assert!((v[3] * 256. + (page % 2 * 256) as f32 - v[0]).abs() < 0.001);
                assert!((v[4] * 256. + (page / 2 * 256) as f32 - v[2]).abs() < 0.001);
            }
        }
        assert!((area - 512. * 512. * 0.5).abs() < 0.001);
        let mut limited = Vec::new();
        assert!(
            art.triangle(
                [vertex(0., 0.), vertex(512., 0.), vertex(0., 512.)],
                &mut limited,
                30
            )
            .is_err()
        );
        assert!(
            limited.len() <= 30,
            "reject before expanded geometry exceeds its budget"
        );
    }
}
