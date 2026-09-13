//! Aspect-responsive flight composition; menus keep their original 640x480 canvas.
use crate::{aircraft::Hornet, flight::State, instruments::Instruments, menu::Sprite};
pub const HUD_SCALE: f64 = 0.85;
struct PanelCache {
    source: Vec<u8>,
    size: [u32; 2],
    image: Sprite,
}
#[derive(Default)]
pub struct FlightCanvas {
    pub pixels: Vec<u8>,
    pub size: [u32; 2],
    background: Vec<u8>,
    panels: std::collections::BTreeMap<u8, PanelCache>,
    background_key: Option<([u32; 2], bool)>,
}
impl FlightCanvas {
    pub fn begin(
        &mut self,
        size: [u32; 2],
        h: &Hornet,
        s: &State,
        cockpit: bool,
        panels: &Instruments,
    ) {
        self.size = size;
        self.pixels
            .resize(size[0] as usize * size[1] as usize * 4, 0);
        let (w, height) = (size[0] as f64, size[1] as f64);
        if self.background_key != Some((size, cockpit)) {
            self.pixels.fill(0);
            if cockpit {
                let image = &h.sprites["~F18H.PIC"];
                let scale = (w / image.width as f64).max(height / image.height as f64);
                self.blit(
                    image,
                    (
                        (w - image.width as f64 * scale) / 2.,
                        0.,
                        image.width as f64 * scale,
                        image.height as f64 * scale,
                    ),
                );
            }
            self.background.clone_from(&self.pixels);
            self.background_key = Some((size, cockpit));
        } else {
            self.pixels.copy_from_slice(&self.background);
        }
        for (i, page) in panels.pages.iter().enumerate() {
            let raster = panels.page(*page, h, s);
            let rect = panels.layout.rect_on(i, [w, height]);
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
                        width: 160,
                        height: 156,
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
