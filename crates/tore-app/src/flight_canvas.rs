//! Aspect-responsive flight composition; menus keep their original 640x480 canvas.
use crate::{aircraft::Airframe, flight::State, instruments::Instruments, menu::Sprite};
// Fifteen percent smaller than the prior 0.85 layout. Angular cues retain world alignment.
pub const HUD_SCALE: f64 = 0.85 * 0.85;
struct PanelCache {
    source: Vec<u8>,
    size: [u32; 2],
    image: Sprite,
}
#[derive(Default)]
pub struct FlightCanvas {
    pub pixels: Vec<u8>,
    pub size: [u32; 2],
    panels: std::collections::BTreeMap<u8, PanelCache>,
}
impl FlightCanvas {
    pub fn begin(&mut self, size: [u32; 2], h: &Airframe, s: &State, panels: &Instruments) {
        self.size = size;
        self.pixels
            .resize(size[0] as usize * size[1] as usize * 4, 0);
        let (w, height) = (size[0] as f64, size[1] as f64);
        self.pixels.fill(0);
        for (i, page) in panels.pages.iter().enumerate() {
            let raster = panels.page(*page, h, s);
            let rect = panels.screen_rect(i, [w, height]);
            let size = [rect.2.round() as u32, rect.3.round() as u32];
            let cached = self.panels.remove(page);
            let cached = match cached {
                Some(cached) if cached.size == size && cached.source == raster.pixels => cached,
                _ => {
                    let mut canvas = FlightCanvas {
                        size,
                        pixels: vec![0; size[0] as usize * size[1] as usize * 4],
                        ..Default::default()
                    };
                    let source = Sprite {
                        width: crate::instruments::WIDTH,
                        height: crate::instruments::HEIGHT,
                        rgba: raster.pixels,
                        glyphs: vec![],
                    };
                    canvas.blit(&source, (0., 0., size[0] as f64, size[1] as f64));
                    PanelCache {
                        source: source.rgba,
                        size,
                        image: Sprite {
                            width: size[0] as usize,
                            height: size[1] as usize,
                            rgba: canvas.pixels,
                            glyphs: vec![],
                        },
                    }
                }
            };
            self.blit(
                &cached.image,
                (
                    rect.0.round(),
                    rect.1.round(),
                    size[0] as f64,
                    size[1] as f64,
                ),
            );
            self.panels.insert(*page, cached);
        }
    }
    /// Blend `color` over the whole flight view, `coverage(radius)` per pixel
    /// with radius 0 at the centre and 1 at the corners.
    pub fn veil(&mut self, color: [u8; 3], coverage: impl Fn(f64) -> f64) {
        let [w, h] = self.size.map(f64::from);
        let (cx, cy) = (w / 2., h / 2.);
        let corner = cx.hypot(cy).max(1.);
        for (i, p) in self.pixels.chunks_exact_mut(4).enumerate() {
            let x = (i % self.size[0] as usize) as f64 + 0.5;
            let y = (i / self.size[0] as usize) as f64 + 0.5;
            let a = coverage((x - cx).hypot(y - cy) / corner);
            if a <= 0. {
                continue;
            }
            // Straight-alpha "over", so the result still blends onto the world.
            let under = f64::from(p[3]) / 255. * (1. - a);
            let out = a + under;
            for c in 0..3 {
                p[c] = ((f64::from(color[c]) * a + f64::from(p[c]) * under) / out).round() as u8;
            }
            p[3] = (out * 255.).round() as u8;
        }
    }
    /// Easy targeting's square outside the HUD: the HUD's own 14-pixel square
    /// and one-pixel line at the HUD's on-screen scale for `zoom`, smoothed
    /// like the HUD so it is no brighter or bolder, with the friendly X.
    pub fn target_square(&mut self, [x, y]: [f64; 2], zoom: f64, color: [u8; 3], friendly: bool) {
        let scale =
            (f64::from(self.size[0]) / 640.).min(f64::from(self.size[1]) / 480.) * HUD_SCALE * zoom;
        let mut line = |a: (f64, f64), b: (f64, f64)| {
            let steps = ((b.0 - a.0).hypot(b.1 - a.1) * scale * 2.).ceil().max(1.) as usize;
            for step in 0..=steps {
                let t = step as f64 / steps as f64;
                let px = x + (a.0 + (b.0 - a.0) * t) * scale;
                let py = y + (a.1 + (b.1 - a.1) * t) * scale;
                self.dot(px, py, scale, color);
            }
        };
        for (a, b) in [
            ((-7., -7.), (7., -7.)),
            ((7., -7.), (7., 7.)),
            ((7., 7.), (-7., 7.)),
            ((-7., 7.), (-7., -7.)),
        ] {
            line(a, b);
        }
        if friendly {
            line((-3., -3.), (3., 3.));
            line((-3., 3.), (3., -3.));
        }
    }
    /// A `size`-wide square pen centred on (x, y), with partial pixels at its
    /// edges, blended so overlapping stamps never exceed full coverage.
    fn dot(&mut self, x: f64, y: f64, size: f64, color: [u8; 3]) {
        let half = size / 2.;
        let [w, h] = self.size.map(|v| v as i64);
        for py in ((y - half).floor() as i64).max(0)..((y + half).ceil() as i64).min(h) {
            for px in ((x - half).floor() as i64).max(0)..((x + half).ceil() as i64).min(w) {
                let cover =
                    |p: i64, c: f64| ((p + 1) as f64).min(c + half) - (p as f64).max(c - half);
                let a =
                    (cover(px, x).clamp(0., 1.) * cover(py, y).clamp(0., 1.) * 255.).round() as u8;
                let i = (py * w + px) as usize * 4;
                if a > self.pixels[i + 3] {
                    self.pixels[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], a]);
                }
            }
        }
    }
    pub fn weapon_debug(&mut self, pixels: &[u8]) {
        let mut rgba = Vec::with_capacity(250 * 96 * 4);
        for y in 0..96 {
            rgba.extend_from_slice(&pixels[y * 640 * 4..(y * 640 + 250) * 4]);
        }
        let scale = (f64::from(self.size[0]) / 640.).min(f64::from(self.size[1]) / 480.);
        self.blit(
            &Sprite {
                width: 250,
                height: 96,
                rgba,
                glyphs: vec![],
            },
            (
                f64::from(self.size[0]) - 258. * scale,
                8. * scale,
                250. * scale,
                96. * scale,
            ),
        );
    }
    pub fn hud_zoom(&self, zoom: f32) -> f32 {
        let [w, h] = self.size;
        zoom / HUD_SCALE as f32 * (h as f32 / (480. * (w as f32 / 640.).min(h as f32 / 480.)))
    }
    pub fn legacy_layer(&mut self, pixels: &[u8], factor: f64) {
        let (mut left, mut top, mut right, mut bottom) = (640usize, 480usize, 0usize, 0usize);
        for (i, p) in pixels.chunks_exact(4).enumerate() {
            if p[3] != 0 {
                left = left.min(i % 640);
                right = right.max(i % 640 + 1);
                top = top.min(i / 640);
                bottom = bottom.max(i / 640 + 1);
            }
        }
        if right == 0 {
            return;
        }
        let width = right - left;
        let height = bottom - top;
        let mut rgba = Vec::with_capacity(width * height * 4);
        for y in top..bottom {
            rgba.extend_from_slice(&pixels[(y * 640 + left) * 4..(y * 640 + right) * 4]);
        }
        let (w, h) = (self.size[0] as f64, self.size[1] as f64);
        let scale = (w / 640.).min(h / 480.) * factor;
        self.blit(
            &Sprite {
                width,
                height,
                rgba,
                glyphs: vec![],
            },
            (
                (w - 640. * scale) / 2. + left as f64 * scale,
                (h - 480. * scale) / 2. + top as f64 * scale,
                width as f64 * scale,
                height as f64 * scale,
            ),
        );
    }
    fn blit(&mut self, s: &Sprite, (x, y, w, h): (f64, f64, f64, f64)) {
        let dw = self.size[0] as usize;
        // Cached instrument rasters are opaque and already at their destination size.
        if w == s.width as f64
            && h == s.height as f64
            && x >= 0.
            && y >= 0.
            && x.fract() == 0.
            && y.fract() == 0.
            && x + w <= self.size[0] as f64
            && y + h <= self.size[1] as f64
            && s.rgba.chunks_exact(4).all(|p| p[3] == 255)
        {
            for row in 0..s.height {
                let at = ((y as usize + row) * dw + x as usize) * 4;
                self.pixels[at..at + s.width * 4]
                    .copy_from_slice(&s.rgba[row * s.width * 4..(row + 1) * s.width * 4]);
            }
            return;
        }

        for yy in (y.floor() as i32).max(0)..((y + h).ceil() as i32).min(self.size[1] as i32) {
            for xx in (x.floor() as i32).max(0)..((x + w).ceil() as i32).min(self.size[0] as i32) {
                let u = ((xx as f64 + 0.5 - x) * s.width as f64 / w - 0.5)
                    .clamp(0., (s.width - 1) as f64);
                let v = ((yy as f64 + 0.5 - y) * s.height as f64 / h - 0.5)
                    .clamp(0., (s.height - 1) as f64);
                let (sx, sy) = (u.floor() as usize, v.floor() as usize);
                let (fx, fy) = (u - sx as f64, v - sy as f64);
                let mut rgba = [0.; 4];
                for (px, py, weight) in [
                    (sx, sy, (1. - fx) * (1. - fy)),
                    ((sx + 1).min(s.width - 1), sy, fx * (1. - fy)),
                    (sx, (sy + 1).min(s.height - 1), (1. - fx) * fy),
                    (
                        (sx + 1).min(s.width - 1),
                        (sy + 1).min(s.height - 1),
                        fx * fy,
                    ),
                ] {
                    let at = (py * s.width + px) * 4;
                    let alpha = s.rgba[at + 3] as f64 / 255.;
                    for (c, sum) in rgba.iter_mut().enumerate().take(3) {
                        *sum += s.rgba[at + c] as f64 * alpha * weight;
                    }
                    rgba[3] += alpha * weight;
                }
                if rgba[3] == 0. {
                    continue;
                }
                let at = (yy as usize * dw + xx as usize) * 4;
                let old = self.pixels[at + 3] as f64 / 255.;
                let alpha = rgba[3] + old * (1. - rgba[3]);
                for (c, value) in rgba.iter().enumerate().take(3) {
                    self.pixels[at + c] =
                        ((value + self.pixels[at + c] as f64 * old * (1. - rgba[3])) / alpha)
                            .round() as u8;
                }
                self.pixels[at + 3] = (alpha * 255.).round() as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_square_scales_with_the_window_and_clips_at_its_edge() {
        let mut canvas = FlightCanvas {
            size: [96, 96],
            pixels: vec![0; 96 * 96 * 4],
            ..Default::default()
        };
        canvas.size = [640, 480];
        canvas.pixels = vec![0; 640 * 480 * 4];
        canvas.target_square([320., 240.], 1., [0, 255, 0], false);
        let alpha = |c: &FlightCanvas, x: usize, y: usize| c.pixels[(y * 640 + x) * 4 + 3];
        // At 640 by 480 the HUD scale is 0.7225: a 10-pixel square of thin,
        // partly covered lines, never a solid bold outline.
        let edge = (320. + 7. * HUD_SCALE) as usize;
        assert!(alpha(&canvas, edge, 240) > 0);
        assert!(alpha(&canvas, edge, 240) < 255);
        assert_eq!(alpha(&canvas, 320, 240), 0);
        assert_eq!(alpha(&canvas, edge + 3, 240), 0);
        canvas.target_square([0., 0.], 1., [0, 255, 0], true);
    }
    #[test]
    fn veil_darkens_the_world_and_instruments_edges_first() {
        let mut canvas = FlightCanvas {
            size: [4, 2],
            pixels: vec![0; 4 * 2 * 4],
            ..Default::default()
        };
        // An opaque white instrument pixel in the top-left corner.
        canvas.pixels[..4].copy_from_slice(&[255; 4]);
        canvas.veil([0, 0, 0], |radius| if radius > 0.5 { 0.5 } else { 0. });
        assert_eq!(&canvas.pixels[..4], &[128, 128, 128, 255]);
        // An empty corner now darkens the world under it.
        assert_eq!(&canvas.pixels[12..16], &[0, 0, 0, 128]);
        // The centre is untouched.
        assert_eq!(&canvas.pixels[4..8], &[0; 4]);
    }
    #[test]
    fn transparent_filtering_keeps_color_without_dark_halos() {
        let mut canvas = FlightCanvas {
            size: [4, 4],
            pixels: vec![0; 64],
            ..Default::default()
        };
        let sprite = Sprite {
            width: 2,
            height: 2,
            rgba: vec![0, 200, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            glyphs: vec![],
        };
        canvas.blit(&sprite, (0., 0., 4., 4.));
        for p in canvas.pixels.chunks_exact(4).filter(|p| p[3] > 0) {
            assert_eq!(p[1], 200);
        }
    }
    #[test]
    fn shrinking_hud_preserves_camera_projection_on_wide_and_tall_screens() {
        for size in [[1920, 1080], [800, 1200], [960, 720]] {
            let canvas = FlightCanvas {
                size,
                ..Default::default()
            };
            let scale = (size[0] as f64 / 640.).min(size[1] as f64 / 480.) * HUD_SCALE;
            let focal = 240. * 3f64.sqrt() * canvas.hud_zoom(1.) as f64 * scale;
            assert!((focal - size[1] as f64 * 0.5 * 3f64.sqrt()).abs() < 0.001);
        }
    }
}
