//! Aspect-responsive flight composition; menus keep their original 640x480 canvas.
use crate::{aim_box, aircraft::Airframe, flight::State, instruments::Instruments, menu::Sprite};
use std::sync::Arc;
use tore_formats::text::GlyphCodes;
#[cfg(test)]
mod benchmark;
#[cfg(test)]
mod worker_tests;
// Fifteen percent smaller than the prior 0.85 layout. Angular cues retain world alignment.
pub const HUD_SCALE: f64 = 0.85 * 0.85;
struct PanelCache {
    source: Vec<u8>,
    size: [u32; 2],
    image: Sprite,
}
struct PanelJob {
    page: u8,
    cached: Option<Arc<PanelCache>>,
    slots: Vec<(usize, [u32; 2])>,
}

// A single page has no independent work to share. Both rasterisation and
// dirty-page scaling belong to each job, while composition remains ordered.
const MIN_PANEL_JOBS: usize = 2;

fn prepare_panel(
    cached: Option<&Arc<PanelCache>>,
    page: u8,
    size: [u32; 2],
    h: &Airframe,
    s: &State,
    panels: &Instruments,
) -> Arc<PanelCache> {
    let raster = panels.page(page, h, s);
    if let Some(cached) = cached
        && cached.size == size
        && cached.source == raster.pixels
    {
        return Arc::clone(cached);
    }
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
    Arc::new(PanelCache {
        source: source.rgba,
        size,
        image: Sprite {
            width: size[0] as usize,
            height: size[1] as usize,
            rgba: canvas.pixels,
            glyphs: vec![],
        },
    })
}

#[derive(Default)]
pub struct FlightCanvas {
    pub pixels: Vec<u8>,
    pub size: [u32; 2],
    panels: std::collections::BTreeMap<u8, Arc<PanelCache>>,
}
impl FlightCanvas {
    pub fn begin(&mut self, size: [u32; 2], h: &Airframe, s: &State, panels: &Instruments) {
        self.begin_with(tore_workers::shared(), size, h, s, panels);
    }

    fn begin_with(
        &mut self,
        workers: &tore_workers::Executor,
        size: [u32; 2],
        h: &Airframe,
        s: &State,
        panels: &Instruments,
    ) {
        self.size = size;
        self.pixels
            .resize(size[0] as usize * size[1] as usize * 4, 0);
        let (w, height) = (size[0] as f64, size[1] as f64);
        self.pixels.fill(0);
        let distinct = panels
            .pages
            .iter()
            .enumerate()
            .filter(|(index, page)| !panels.pages[..*index].contains(page))
            .count();
        if !workers.should_dispatch(distinct, MIN_PANEL_JOBS) {
            for (i, page) in panels.pages.iter().enumerate() {
                let rect = panels.screen_rect(i, [w, height]);
                let size = [rect.2.round() as u32, rect.3.round() as u32];
                let cached = prepare_panel(self.panels.get(page), *page, size, h, s, panels);
                self.compose_panel(*page, rect, cached);
            }
            return;
        }

        // The call point follows this frame's preview, readout, palette and
        // hover updates. All jobs borrow those same immutable inputs until
        // preparation joins; nothing from the window or GPU crosses here.
        let mut jobs: Vec<PanelJob> = Vec::with_capacity(distinct);
        let mut rects = Vec::with_capacity(panels.pages.len());
        for (index, &page) in panels.pages.iter().enumerate() {
            let rect = panels.screen_rect(index, [w, height]);
            rects.push(rect);
            let slot = (index, [rect.2.round() as u32, rect.3.round() as u32]);
            if let Some(job) = jobs.iter_mut().find(|job| job.page == page) {
                job.slots.push(slot);
            } else {
                jobs.push(PanelJob {
                    page,
                    cached: self.panels.get(&page).cloned(),
                    slots: vec![slot],
                });
            }
        }
        let prepared = workers.ordered_map(&jobs, MIN_PANEL_JOBS, |_, job| {
            let mut cached = job.cached.clone();
            job.slots
                .iter()
                .map(|&(index, size)| {
                    let next = prepare_panel(cached.as_ref(), job.page, size, h, s, panels);
                    cached = Some(Arc::clone(&next));
                    (index, next)
                })
                .collect::<Vec<_>>()
        });
        let mut slots = vec![None; panels.pages.len()];
        for (index, cached) in prepared.into_iter().flatten() {
            slots[index] = Some(cached);
        }
        for ((&page, rect), cached) in panels.pages.iter().zip(rects).zip(slots) {
            self.compose_panel(page, rect, cached.expect("every panel slot was prepared"));
        }
    }

    fn compose_panel(&mut self, page: u8, rect: (f64, f64, f64, f64), cached: Arc<PanelCache>) {
        self.blit(
            &cached.image,
            (
                rect.0.round(),
                rect.1.round(),
                cached.size[0] as f64,
                cached.size[1] as f64,
            ),
        );
        self.panels.insert(page, cached);
    }
    /// Blend `color` at `alpha` over one screen pixel; outside the view is ignored.
    pub fn blend(&mut self, x: i32, y: i32, color: [u8; 3], alpha: f64) {
        let [width, height] = self.size.map(|n| n as i32);
        if x < 0 || y < 0 || x >= width || y >= height {
            return;
        }
        let at = (y * width + x) as usize * 4;
        let p = &mut self.pixels[at..at + 4];
        // Straight-alpha "over", as in `veil`.
        let under = f64::from(p[3]) / 255. * (1. - alpha);
        let out = alpha + under;
        for c in 0..3 {
            p[c] = ((f64::from(color[c]) * alpha + f64::from(p[c]) * under) / out).round() as u8;
        }
        p[3] = (out * 255.).round() as u8;
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
    /// How many canvas pixels one HUD pixel covers at `zoom`.
    pub fn hud_pixel(&self, zoom: f64) -> f64 {
        (f64::from(self.size[0]) / 640.).min(f64::from(self.size[1]) / 480.) * HUD_SCALE * zoom
    }
    /// Strokes given in HUD pixels about `at`, smoothed like the HUD so they
    /// are no brighter or bolder.
    fn hud_strokes(
        &mut self,
        [x, y]: [f64; 2],
        zoom: f64,
        color: [u8; 3],
        strokes: &[aim_box::Stroke],
    ) {
        let scale = self.hud_pixel(zoom);
        for &(a, b) in strokes {
            let steps = ((b.0 - a.0).hypot(b.1 - a.1) * scale * 2.).ceil().max(1.) as usize;
            for step in 0..=steps {
                let t = step as f64 / steps as f64;
                let px = x + (a.0 + (b.0 - a.0) * t) * scale;
                let py = y + (a.1 + (b.1 - a.1) * t) * scale;
                self.dot(px, py, scale, color);
            }
        }
    }
    /// The target square outside the HUD (Easy targeting's, and the AC-130's
    /// aim-point box): the HUD's own 14-pixel square and one-pixel line at the
    /// HUD's on-screen scale for `zoom`, with the friendly X for a friendly
    /// target, a centre dot for a pinned point or corner brackets for a point
    /// the sight only sweeps over.
    pub fn target_square(
        &mut self,
        at: [f64; 2],
        zoom: f64,
        color: [u8; 3],
        style: aim_box::Style,
    ) {
        self.hud_strokes(at, zoom, color, &aim_box::box_strokes(style));
    }
    /// The small diamond where the AC-130's guns will hit.
    pub fn aim_diamond(&mut self, at: [f64; 2], zoom: f64, color: [u8; 3]) {
        self.hud_strokes(
            at,
            zoom,
            color,
            &aim_box::diamond_strokes(aim_box::DIAMOND_HALF),
        );
    }
    /// An arrow at the screen edge towards a box that is off screen, the
    /// shape of the HUD's own edge arrow. `direction` is a unit vector, x
    /// right and y down.
    /// It keeps clear of the `windows` (the instrument panels), which would
    /// hide it.
    pub fn edge_chevron(
        &mut self,
        direction: [f64; 2],
        zoom: f64,
        color: [u8; 3],
        windows: &[aim_box::Rect],
    ) {
        // Far enough in that the whole arrow, and the line's width, show.
        let margin = aim_box::EDGE_ARROW * 1.5 * self.hud_pixel(zoom);
        let tip =
            aim_box::edge_point_clear(self.size.map(f64::from), margin, direction, windows, margin);
        self.hud_strokes(
            tip,
            zoom,
            color,
            &aim_box::chevron_strokes(direction, aim_box::EDGE_ARROW, aim_box::EDGE_ARROW / 2.),
        );
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
    /// An empty, fully transparent canvas of `size`: the mission replay's
    /// view has no instruments.
    pub fn blank(&mut self, size: [u32; 2]) {
        self.size = size;
        self.pixels.clear();
        self.pixels
            .resize(size[0] as usize * size[1] as usize * 4, 0);
    }
    /// A 640x480 layer drawn in two halves at the scale `legacy_layer` uses,
    /// centred across: the top half against the top edge and the bottom half
    /// against the bottom edge. On a canvas at least 4:3 wide this is exactly
    /// where `legacy_layer` puts it; on a taller one the halves part, so a
    /// bar along the layer's bottom stays on the view's bottom edge.
    pub fn anchored_layer(&mut self, pixels: &[u8]) {
        let (w, h) = (self.size[0] as f64, self.size[1] as f64);
        let scale = (w / 640.).min(h / 480.);
        for (top, y) in [(0, 0.), (240, h - 240. * scale)] {
            let (mut left, mut first, mut right, mut last) = (640usize, 480usize, 0usize, 0usize);
            for row in top..top + 240 {
                for (x, p) in pixels[row * 640 * 4..(row + 1) * 640 * 4]
                    .chunks_exact(4)
                    .enumerate()
                {
                    if p[3] != 0 {
                        left = left.min(x);
                        right = right.max(x + 1);
                        first = first.min(row);
                        last = last.max(row + 1);
                    }
                }
            }
            if right == 0 {
                continue;
            }
            let (width, height) = (right - left, last - first);
            let mut rgba = Vec::with_capacity(width * height * 4);
            for row in first..last {
                rgba.extend_from_slice(&pixels[(row * 640 + left) * 4..(row * 640 + right) * 4]);
            }
            self.scaled(
                &Sprite {
                    width,
                    height,
                    rgba,
                    glyphs: vec![],
                },
                (
                    (w - 640. * scale) / 2. + left as f64 * scale,
                    y + (first - top) as f64 * scale,
                    width as f64 * scale,
                    height as f64 * scale,
                ),
            );
        }
    }
    /// Rectangles of a 640x480 layer, placed where `legacy_layer` puts the
    /// whole layer (centred and scaled to fit the view) but drawn by nearest
    /// pixel like `anchored_layer`, so small text stays crisp. Only those
    /// rectangles are read: the debug panels and menus know where they drew.
    pub fn centered_rects(&mut self, pixels: &[u8], rects: &[(i32, i32, i32, i32)]) {
        let (w, h) = (self.size[0] as f64, self.size[1] as f64);
        let scale = (w / 640.).min(h / 480.);
        let (left, top) = ((w - 640. * scale) / 2., (h - 480. * scale) / 2.);
        for &(x, y, rw, rh) in rects {
            let (x0, y0) = (x.clamp(0, 640) as usize, y.clamp(0, 480) as usize);
            let (x1, y1) = (
                (x + rw).clamp(0, 640) as usize,
                (y + rh).clamp(0, 480) as usize,
            );
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            let (width, height) = (x1 - x0, y1 - y0);
            let mut rgba = Vec::with_capacity(width * height * 4);
            for row in y0..y1 {
                rgba.extend_from_slice(&pixels[(row * 640 + x0) * 4..(row * 640 + x1) * 4]);
            }
            self.scaled(
                &Sprite {
                    width,
                    height,
                    rgba,
                    glyphs: vec![],
                },
                (
                    left + x0 as f64 * scale,
                    top + y0 as f64 * scale,
                    width as f64 * scale,
                    height as f64 * scale,
                ),
            );
        }
    }
    /// Draws `s` scaled into the rectangle by nearest pixel, the way the
    /// menus' 640x480 canvas is scaled, blending partly transparent pixels
    /// over what is there. Cheap enough for the replay's interface layer,
    /// which is drawn every frame.
    fn scaled(&mut self, s: &Sprite, (x, y, w, h): (f64, f64, f64, f64)) {
        let dw = self.size[0] as usize;
        let taps = |start: i32, end: i32, origin: f64, span: f64, source: usize| {
            (start..end)
                .map(|d| {
                    let u = ((d as f64 + 0.5 - origin) * source as f64 / span).floor();
                    (d as usize, u.clamp(0., (source - 1) as f64) as usize)
                })
                .collect::<Vec<_>>()
        };
        let columns = taps(
            (x.round() as i32).max(0),
            ((x + w).round() as i32).min(self.size[0] as i32),
            x,
            w,
            s.width,
        );
        let rows = taps(
            (y.round() as i32).max(0),
            ((y + h).round() as i32).min(self.size[1] as i32),
            y,
            h,
            s.height,
        );
        for &(yy, sy) in &rows {
            for &(xx, sx) in &columns {
                let from = (sy * s.width + sx) * 4;
                let p = &s.rgba[from..from + 4];
                let at = (yy * dw + xx) * 4;
                if p[3] == 0 {
                    continue;
                }
                if p[3] == 255 || self.pixels[at + 3] == 0 {
                    self.pixels[at..at + 4].copy_from_slice(p);
                    continue;
                }
                // Straight-alpha "over".
                let a = f64::from(p[3]) / 255.;
                let old = f64::from(self.pixels[at + 3]) / 255.;
                let out = a + old * (1. - a);
                for (c, value) in p.iter().enumerate().take(3) {
                    self.pixels[at + c] = ((f64::from(*value) * a
                        + f64::from(self.pixels[at + c]) * old * (1. - a))
                        / out)
                        .round() as u8;
                }
                self.pixels[at + 3] = (out * 255.).round() as u8;
            }
        }
    }
    /// The width of `text` in `font` drawn at `scale`, in canvas pixels.
    pub fn text_width(font: &tore_formats::font::Font, text: &str, scale: f64) -> f64 {
        text.glyph_codes()
            .filter_map(|c| font.glyphs.get(usize::from(c)))
            .map(|g| g.advance as f64 * scale)
            .sum()
    }
    /// `text` in `font` with its top left at (x, y), each font pixel drawn
    /// as a `scale`-wide smoothed dot, then a dark shadow a font pixel down
    /// and right where the text left the canvas clear, so labels read over
    /// sky and ground alike.
    pub fn text(
        &mut self,
        font: &tore_formats::font::Font,
        text: &str,
        [x, y]: [f64; 2],
        scale: f64,
        color: [u8; 3],
    ) {
        for (offset, color) in [(0., color), (scale, [0, 0, 0])] {
            let mut pen = x + offset;
            for c in text.glyph_codes() {
                let Some(glyph) = font.glyphs.get(usize::from(c)) else {
                    continue;
                };
                for &(gx, gy) in &glyph.pixels {
                    self.dot(
                        pen + (gx as f64 + 0.5) * scale,
                        y + offset + (gy as f64 + 0.5) * scale,
                        scale,
                        color,
                    );
                }
                pen += glyph.advance as f64 * scale;
            }
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
        let whole_pixels = w == s.width as f64
            && h == s.height as f64
            && x >= 0.
            && y >= 0.
            && x.fract() == 0.
            && y.fract() == 0.
            && x + w <= self.size[0] as f64
            && y + h <= self.size[1] as f64;
        // Retain the row-copy path for completely opaque cached panels.
        if whole_pixels && s.rgba.chunks_exact(4).all(|p| p[3] == 255) {
            for row in 0..s.height {
                let at = ((y as usize + row) * dw + x as usize) * 4;
                self.pixels[at..at + s.width * 4]
                    .copy_from_slice(&s.rgba[row * s.width * 4..(row + 1) * s.width * 4]);
            }
            return;
        }

        // Transparent bezel corners must not make an already-sized panel run
        // through the four-tap filter again. Below this coordinate bound its
        // intermediate products are half-integers below 2^48, exactly stored
        // in f64, so the old filter's weights are exactly [1, 0, 0, 0]. The
        // bound also keeps its signed pixel-loop coordinates in range.
        if whole_pixels && self.size.iter().all(|&dimension| dimension <= 1 << 24) {
            for row in 0..s.height {
                let at = ((y as usize + row) * dw + x as usize) * 4;
                let source = &s.rgba[row * s.width * 4..(row + 1) * s.width * 4];
                let destination = &mut self.pixels[at..at + s.width * 4];
                for (source, destination) in
                    source.chunks_exact(4).zip(destination.chunks_exact_mut(4))
                {
                    match source[3] {
                        0 => continue,
                        255 => destination.copy_from_slice(source),
                        _ => {
                            let source_alpha = f64::from(source[3]) / 255.;
                            let old = f64::from(destination[3]) / 255.;
                            let alpha = source_alpha + old * (1. - source_alpha);
                            for c in 0..3 {
                                let value = f64::from(source[c]) * source_alpha;
                                destination[c] = ((value
                                    + f64::from(destination[c]) * old * (1. - source_alpha))
                                    / alpha)
                                    .round() as u8;
                            }
                            destination[3] = (alpha * 255.).round() as u8;
                        }
                    }
                }
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

    // The pre-optimization filter, independent of blit's integer-size paths.
    fn filtered_reference(
        canvas: &mut FlightCanvas,
        s: &Sprite,
        (x, y, w, h): (f64, f64, f64, f64),
    ) {
        let dw = canvas.size[0] as usize;
        for yy in (y.floor() as i32).max(0)..((y + h).ceil() as i32).min(canvas.size[1] as i32) {
            for xx in (x.floor() as i32).max(0)..((x + w).ceil() as i32).min(canvas.size[0] as i32)
            {
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
                let old = canvas.pixels[at + 3] as f64 / 255.;
                let alpha = rgba[3] + old * (1. - rgba[3]);
                for (c, value) in rgba.iter().enumerate().take(3) {
                    canvas.pixels[at + c] =
                        ((value + canvas.pixels[at + c] as f64 * old * (1. - rgba[3])) / alpha)
                            .round() as u8;
                }
                canvas.pixels[at + 3] = (alpha * 255.).round() as u8;
            }
        }
    }

    #[test]
    fn integer_blit_matches_filter_for_every_source_and_destination_alpha() {
        for seed in [0u8, 1, 127, 255] {
            let mut source = Sprite {
                width: 256,
                height: 256,
                rgba: Vec::new(),
                glyphs: Vec::new(),
            };
            let mut destination = Vec::new();
            for source_alpha in 0..=255u8 {
                for destination_alpha in 0..=255u8 {
                    source.rgba.extend_from_slice(&[
                        source_alpha.wrapping_mul(37) ^ destination_alpha ^ seed,
                        destination_alpha.wrapping_add(31).wrapping_add(seed),
                        source_alpha ^ destination_alpha.rotate_left(1),
                        source_alpha,
                    ]);
                    destination.extend_from_slice(&[
                        destination_alpha.wrapping_mul(11) ^ seed,
                        255 - source_alpha,
                        source_alpha
                            .wrapping_add(destination_alpha)
                            .wrapping_add(seed),
                        destination_alpha,
                    ]);
                }
            }
            let mut actual = FlightCanvas {
                size: [256, 256],
                pixels: destination.clone(),
                ..Default::default()
            };
            let mut expected = FlightCanvas {
                size: [256, 256],
                pixels: destination,
                ..Default::default()
            };
            actual.blit(&source, (0., 0., 256., 256.));
            filtered_reference(&mut expected, &source, (0., 0., 256., 256.));
            assert_eq!(actual.pixels, expected.pixels, "RGB seed {seed}");
        }
    }

    fn patterned_sprite(width: usize, height: usize) -> Sprite {
        Sprite {
            width,
            height,
            rgba: (0..width * height)
                .flat_map(|index| {
                    let value = index as u8;
                    [
                        value.wrapping_mul(37),
                        value.wrapping_add(127),
                        255 - value,
                        value.wrapping_mul(17),
                    ]
                })
                .collect(),
            glyphs: Vec::new(),
        }
    }

    fn compare_blit(source: &Sprite, size: [u32; 2], rect: (f64, f64, f64, f64)) {
        let pixels = patterned_sprite(size[0] as usize, size[1] as usize).rgba;
        let mut actual = FlightCanvas {
            size,
            pixels: pixels.clone(),
            ..Default::default()
        };
        let mut expected = FlightCanvas {
            size,
            pixels,
            ..Default::default()
        };
        actual.blit(source, rect);
        filtered_reference(&mut expected, source, rect);
        assert_eq!(
            actual.pixels, expected.pixels,
            "canvas {size:?}, rect {rect:?}"
        );
    }

    #[test]
    fn integer_blit_matches_panel_sizes_and_canvas_boundaries() {
        // Large panels at 640, 1280 and 1920 window widths, plus a small
        // 1920-wide panel. The rounded width may be odd while height is even.
        for (width, height) in [(1, 1), (162, 160), (243, 240), (365, 360), (216, 214)] {
            let source = patterned_sprite(width, height);
            for (x, y) in [(0., 0.), (7., 0.), (0., 5.), (7., 5.)] {
                compare_blit(
                    &source,
                    [width as u32 + 7, height as u32 + 5],
                    (x, y, width as f64, height as f64),
                );
            }
        }
    }

    #[test]
    fn integer_blit_preserves_scaled_fractional_and_clipped_filters() {
        let source = patterned_sprite(8, 5);
        for rect in [
            (-1., 0., 8., 5.),
            (0., -1., 8., 5.),
            (10., 10., 8., 5.),
            (0.5, 1., 8., 5.),
            (1., 0.5, 8., 5.),
            (1., 1., 12., 8.),
            (1., 1., 4., 3.),
        ] {
            compare_blit(&source, [17, 13], rect);
        }
    }

    #[test]
    fn target_square_scales_with_the_window_and_clips_at_its_edge() {
        let mut canvas = FlightCanvas {
            size: [96, 96],
            pixels: vec![0; 96 * 96 * 4],
            ..Default::default()
        };
        canvas.size = [640, 480];
        canvas.pixels = vec![0; 640 * 480 * 4];
        canvas.target_square(
            [320., 240.],
            1.,
            [0, 255, 0],
            aim_box::Style::Tracked { friendly: false },
        );
        let alpha = |c: &FlightCanvas, x: usize, y: usize| c.pixels[(y * 640 + x) * 4 + 3];
        // At 640 by 480 the HUD scale is 0.7225: a 10-pixel square of thin,
        // partly covered lines, never a solid bold outline.
        let edge = (320. + 7. * HUD_SCALE) as usize;
        assert!(alpha(&canvas, edge, 240) > 0);
        assert!(alpha(&canvas, edge, 240) < 255);
        assert_eq!(alpha(&canvas, 320, 240), 0);
        assert_eq!(alpha(&canvas, edge + 3, 240), 0);
        canvas.target_square(
            [0., 0.],
            1.,
            [0, 255, 0],
            aim_box::Style::Tracked { friendly: true },
        );
    }
    #[test]
    fn edge_chevron_sits_inside_the_edge_and_points_outward() {
        let mut canvas = FlightCanvas {
            size: [640, 480],
            pixels: vec![0; 640 * 480 * 4],
            ..Default::default()
        };
        canvas.edge_chevron([1., 0.], 1., [0, 255, 0], &[]);
        let lit: Vec<(usize, usize)> = canvas
            .pixels
            .chunks_exact(4)
            .enumerate()
            .filter(|(_, p)| p[3] > 0)
            .map(|(i, _)| (i % 640, i / 640))
            .collect();
        assert!(!lit.is_empty());
        let right = lit.iter().map(|p| p.0).max().unwrap();
        let left = lit.iter().map(|p| p.0).min().unwrap();
        // The tip is 12 HUD pixels in from the right edge, the arms trail
        // back towards the middle, and the arrow is centred on the row.
        assert!(right < 640 && right > 640 - 20, "{right}");
        assert!(left < right - 4);
        let rows = (
            lit.iter().map(|p| p.1).min().unwrap(),
            lit.iter().map(|p| p.1).max().unwrap(),
        );
        assert!(rows.0 < 240 && rows.1 > 240 && (rows.0 + rows.1).abs_diff(480) <= 2);
    }
    #[test]
    fn aim_diamond_is_smaller_than_the_box() {
        let extent = |draw: &dyn Fn(&mut FlightCanvas)| {
            let mut canvas = FlightCanvas {
                size: [640, 480],
                pixels: vec![0; 640 * 480 * 4],
                ..Default::default()
            };
            draw(&mut canvas);
            let xs: Vec<usize> = canvas
                .pixels
                .chunks_exact(4)
                .enumerate()
                .filter(|(_, p)| p[3] > 0)
                .map(|(i, _)| i % 640)
                .collect();
            xs.iter().max().unwrap() - xs.iter().min().unwrap()
        };
        let diamond = extent(&|c| c.aim_diamond([320., 240.], 1., [0, 255, 0]));
        let square = extent(&|c| {
            c.target_square([320., 240.], 1., [0, 255, 0], aim_box::Style::Free);
        });
        assert!(
            diamond * 2 <= square + 2 && diamond >= 3,
            "{diamond} {square}"
        );
        // A pinned point is the square plus a centre dot.
        let mut canvas = FlightCanvas {
            size: [640, 480],
            pixels: vec![0; 640 * 480 * 4],
            ..Default::default()
        };
        canvas.target_square([320., 240.], 1., [0, 255, 0], aim_box::Style::Pinned);
        assert!(canvas.pixels[(240 * 640 + 320) * 4 + 3] > 0);
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
    fn an_anchored_layer_keeps_its_bottom_on_the_bottom_edge() {
        let mut layer = vec![0u8; 640 * 480 * 4];
        for (x, y) in [(15, 5), (15, 470)] {
            layer[(y * 640 + x) * 4..][..4].copy_from_slice(&[200, 10, 10, 255]);
        }
        let alpha =
            |c: &FlightCanvas, x: usize, y: usize| c.pixels[(y * c.size[0] as usize + x) * 4 + 3];
        // 4:3 and wide: where the centred layer draws it.
        for (size, left, scale) in [([1280, 960], 0., 2.), ([1920, 1080], 240., 2.25)] {
            let mut canvas = FlightCanvas::default();
            canvas.blank(size);
            canvas.anchored_layer(&layer);
            for y in [5., 470.] {
                let at = |v: f64| (v * scale + scale / 2.) as usize;
                assert_eq!(
                    alpha(&canvas, left as usize + at(15.), at(y)),
                    255,
                    "{size:?}"
                );
            }
        }
        // Tall: the top half at the top, the bottom half at the bottom.
        let mut canvas = FlightCanvas::default();
        canvas.blank([640, 1000]);
        canvas.anchored_layer(&layer);
        assert_eq!(alpha(&canvas, 15, 5), 255);
        assert_eq!(alpha(&canvas, 15, 1000 - 10), 255);
        assert_eq!(alpha(&canvas, 15, 260 + 470), 0);
        // Blanking clears the canvas for the next frame.
        canvas.blank([640, 1000]);
        assert!(canvas.pixels.iter().all(|v| *v == 0));
    }
    #[test]
    fn centered_rects_draw_only_their_part_of_the_layer_centred() {
        let mut layer = vec![0u8; 640 * 480 * 4];
        for (x, y) in [(20, 30), (20, 400), (600, 30)] {
            layer[(y * 640 + x) * 4..][..4].copy_from_slice(&[200, 10, 10, 255]);
        }
        let alpha =
            |c: &FlightCanvas, x: usize, y: usize| c.pixels[(y * c.size[0] as usize + x) * 4 + 3];
        // Wide: centred across; tall: centred up and down, unlike the
        // anchored layer, so a tall panel never parts in the middle.
        for (size, left, top, scale) in [
            ([1920, 1080], 240., 0., 2.25),
            ([640, 1000], 0., 260., 1.),
            ([1280, 960], 0., 0., 2.),
        ] {
            let mut canvas = FlightCanvas::default();
            canvas.blank(size);
            canvas.centered_rects(&layer, &[(10, 20, 100, 400), (-5, -5, 2, 2)]);
            let at = |v: f64, origin: f64| (origin + v * scale + scale / 2.) as usize;
            assert_eq!(alpha(&canvas, at(20., left), at(30., top)), 255, "{size:?}");
            assert_eq!(
                alpha(&canvas, at(20., left), at(400., top)),
                255,
                "{size:?}"
            );
            // Outside the rectangles nothing is drawn.
            assert_eq!(alpha(&canvas, at(600., left), at(30., top)), 0, "{size:?}");
        }
    }
    #[test]
    fn scaled_layers_keep_hard_pixels_and_blend_translucent_ones() {
        let sprite = Sprite {
            width: 2,
            height: 1,
            rgba: vec![200, 10, 10, 255, 0, 0, 255, 128],
            glyphs: vec![],
        };
        let mut canvas = FlightCanvas::default();
        canvas.blank([9, 3]);
        // Under the translucent pixel, an opaque white one.
        canvas.pixels[(9 + 6) * 4..][..4].copy_from_slice(&[255; 4]);
        canvas.scaled(&sprite, (0.5, 0., 9., 3.));
        let at = |x: usize, y: usize| canvas.pixels[(y * 9 + x) * 4..][..4].to_vec();
        // Each source pixel covers four and a half columns: 0.5..5 and 5..9.5.
        assert_eq!(at(0, 0), [0; 4]);
        assert_eq!(at(1, 1), [200, 10, 10, 255]);
        assert_eq!(at(4, 2), [200, 10, 10, 255]);
        assert_eq!(at(7, 0), [0, 0, 255, 128]);
        assert_eq!(at(6, 1), [127, 127, 255, 255]);
    }
    #[test]
    fn labels_draw_scaled_with_a_shadow() {
        let font = tore_formats::font::Font {
            height: 2,
            glyphs: (0..256)
                .map(|c| tore_formats::font::Glyph {
                    advance: 3,
                    pixels: if c == usize::from(b'A') {
                        vec![(0, 0), (1, 1)]
                    } else {
                        vec![]
                    },
                })
                .collect(),
        };
        assert_eq!(FlightCanvas::text_width(&font, "AA", 2.), 12.);
        let mut canvas = FlightCanvas::default();
        canvas.blank([20, 20]);
        canvas.text(&font, "A", [4., 4.], 2., [255, 255, 255]);
        let at = |x: usize, y: usize| &canvas.pixels[(y * 20 + x) * 4..][..4];
        assert_eq!(at(4, 4), [255, 255, 255, 255]);
        assert_eq!(at(7, 7), [255, 255, 255, 255]);
        // The shadow fills in beside the text, never over it.
        assert_eq!(at(9, 9), [0, 0, 0, 255]);
        assert_eq!(at(6, 4), [0; 4]);
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
