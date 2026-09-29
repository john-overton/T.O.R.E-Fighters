//! Bearing compass for the F7 player to target view: the HUD heading strip
//! stretched across the top of the view, centered on the bearing to the
//! target. Opinionated, requested by John on 2026-09-28.
use crate::{
    flight_canvas::{FlightCanvas, HUD_SCALE},
    instruments::Instruments,
};
use tore_formats::font::Font;
use tore_formats::text::GlyphCodes;

/// Strip ends in layout units. On screen the strip is the middle half of the
/// view, narrowed to clear instrument windows, still showing the same span.
const LEFT: f64 = 160.;
const RIGHT: f64 = 480.;
const PER_DEGREE: f64 = 2.;
/// Degrees either side of the target's bearing the strip shows.
const SPAN: f64 = (RIGHT - LEFT) / 2. / PER_DEGREE;
/// Gap above the strip, in 640x480 layer units.
const MARGIN: f64 = 6.;
/// Clearance from an instrument window's inner edge, in layer units.
const GAP: f64 = 4.;
// Rows below the strip's top, in HUD units, spaced as the HUD's own strip.
const LABEL: f64 = 4.;
const TICK: f64 = 17.;
const CARET: f64 = 24.;
const READOUT: f64 = 32.;

#[derive(Debug, PartialEq)]
struct Layout {
    /// Strip x and two-digit label of each 10-degree tick.
    ticks: Vec<(f64, String)>,
    bearing: String,
}
/// The strip in 640-wide units, centered on compass `bearing`.
fn layout(bearing: f64) -> Layout {
    let bearing = bearing.rem_euclid(360.);
    let first = ((bearing - SPAN) / 10.).ceil() as i32 * 10;
    let ticks = (0..)
        .map(|i| first + i * 10)
        .take_while(|&tick| f64::from(tick) <= bearing + SPAN)
        .map(|tick| {
            (
                (LEFT + RIGHT) / 2. + (f64::from(tick) - bearing) * PER_DEGREE,
                format!("{:02}", tick.rem_euclid(360) / 10),
            )
        })
        .collect();
    Layout {
        ticks,
        bearing: format!("{:03}", bearing.round() as u32 % 360),
    }
}
/// Compass bearing in degrees from `from` to `to`, as `hud::heading` reads a
/// yaw: x east, z north.
pub fn bearing(from: [f64; 3], to: [f64; 3]) -> f64 {
    (to[0] - from[0])
        .atan2(to[2] - from[2])
        .to_degrees()
        .rem_euclid(360.)
}
/// Draw the strip over the flight view, centered on the target's compass
/// `bearing`, in the HUD's font and color at the HUD's on-screen size,
/// smoothed like the in-flight messages. It narrows to clear the instrument
/// windows shown beside it.
pub fn draw(canvas: &mut FlightCanvas, instruments: &Instruments, font: &Font, bearing: f64) {
    let size = canvas.size.map(f64::from);
    let windows: Vec<_> = (0..instruments.pages.len())
        .map(|slot| instruments.screen_rect(slot, size))
        .collect();
    draw_within(canvas, font, instruments.hud_color, bearing, &windows);
}
/// Draw the strip over a view with no instrument windows, as the mission
/// replay shows F7, in `font` and `color`: the middle half of the view.
pub fn draw_clear(canvas: &mut FlightCanvas, font: &Font, color: [u8; 3], bearing: f64) {
    draw_within(canvas, font, color, bearing, &[]);
}
type Rect = (f64, f64, f64, f64);
fn draw_within(
    canvas: &mut FlightCanvas,
    font: &Font,
    color: [u8; 3],
    bearing: f64,
    windows: &[Rect],
) {
    let [w, h] = canvas.size.map(f64::from);
    let layer = (w / 640.).min(h / 480.);
    let scale = layer * HUD_SCALE;
    let y = |v: f64| MARGIN * layer + v * scale;
    let rows = (y(READOUT) + (font.height + 1) as f64 * scale).ceil() as usize + 1;
    let inset = GAP * layer + FlightCanvas::text_width(font, "00", scale) / 2.;
    let half = half_width(w, windows, rows as f64, inset);
    let c = w / 2.;
    let across = |x: f64| c + (x - (LEFT + RIGHT) / 2.) / ((RIGHT - LEFT) / 2.) * half;
    let mut ink = Ink {
        cover: vec![0.; canvas.size[0] as usize * rows],
        width: canvas.size[0] as usize,
        scale,
    };
    let layout = layout(bearing);
    let centered = |text: &str, x: f64| x - FlightCanvas::text_width(font, text, scale) / 2.;
    for (x, label) in &layout.ticks {
        let x = across(*x);
        ink.line((x, y(TICK)), (x, y(TICK + 5.)));
        ink.text(font, label, centered(label, x), y(LABEL));
    }
    ink.line((c - 5. * scale, y(CARET)), (c, y(CARET + 5.)));
    ink.line((c, y(CARET + 5.)), (c + 5. * scale, y(CARET)));
    ink.text(
        font,
        &layout.bearing,
        centered(&layout.bearing, c),
        y(READOUT),
    );
    ink.blend(canvas, color);
}
/// Half the strip's width on a `width`-wide view: a quarter of the view, less
/// whatever keeps it `inset` clear of windows reaching into its top `rows`.
fn half_width(width: f64, windows: &[Rect], rows: f64, inset: f64) -> f64 {
    let c = width / 2.;
    windows
        .iter()
        .filter(|&&(_, y, _, h)| y < rows && y + h > 0.)
        .filter_map(|&(x, _, w, _)| {
            if x + w <= c {
                Some(c - (x + w) - inset)
            } else if x >= c {
                Some(x - c - inset)
            } else {
                None
            }
        })
        .fold(width / 4., f64::min)
        .max(0.)
}
/// Area coverage of the strip's strokes and glyphs across the top rows of
/// the view, blended once so overlapping stamps never exceed full coverage.
struct Ink {
    cover: Vec<f64>,
    width: usize,
    scale: f64,
}
impl Ink {
    /// A `size`-wide square at (left, up); glyph boxes add, pen stamps take the most.
    fn square(&mut self, left: f64, up: f64, size: f64, add: bool) {
        let rows = self.cover.len() / self.width.max(1);
        let (right, down) = (left + size, up + size);
        for row in (up.floor().max(0.) as usize)..(down.ceil().max(0.) as usize).min(rows) {
            let dy = down.min(row as f64 + 1.) - up.max(row as f64);
            for column in
                (left.floor().max(0.) as usize)..(right.ceil().max(0.) as usize).min(self.width)
            {
                let dx = right.min(column as f64 + 1.) - left.max(column as f64);
                let cell = &mut self.cover[row * self.width + column];
                *cell = if add {
                    *cell + dx * dy
                } else {
                    cell.max(dx * dy)
                };
            }
        }
    }
    /// A one HUD pixel line, as the HUD draws its strip.
    fn line(&mut self, a: (f64, f64), b: (f64, f64)) {
        let s = self.scale;
        let steps = ((b.0 - a.0).hypot(b.1 - a.1) / s * 2.).ceil().max(1.) as usize;
        for step in 0..=steps {
            let t = step as f64 / steps as f64;
            let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            self.square(x - s / 2., y - s / 2., s, false);
        }
    }
    fn text(&mut self, font: &Font, text: &str, mut x: f64, y: f64) {
        let s = self.scale;
        for c in text.glyph_codes() {
            let Some(glyph) = font.glyphs.get(usize::from(c)) else {
                continue;
            };
            for &(gx, gy) in &glyph.pixels {
                self.square(x + gx as f64 * s, y + gy as f64 * s, s, true);
            }
            x += glyph.advance as f64 * s;
        }
    }
    fn blend(self, canvas: &mut FlightCanvas, color: [u8; 3]) {
        for (at, amount) in self.cover.into_iter().enumerate() {
            if amount > 0. {
                let (column, row) = (at % self.width, at / self.width);
                canvas.blend(column as i32, row as i32, color, amount.min(1.));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ticks_every_ten_degrees_centered_on_the_bearing() {
        let north = layout(0.);
        assert_eq!(north.ticks.len(), 17);
        assert_eq!(north.ticks[0], (160., "28".to_owned()));
        assert_eq!(north.ticks[8], (320., "00".to_owned()));
        assert_eq!(north.ticks[16], (480., "08".to_owned()));
        assert!(north.ticks.windows(2).all(|t| t[1].0 - t[0].0 == 20.));
        assert_eq!(north.bearing, "000");
        let east = layout(90.);
        assert_eq!(east.ticks[0], (160., "01".to_owned()));
        assert_eq!(east.ticks[8], (320., "09".to_owned()));
        assert_eq!(east.ticks[16], (480., "17".to_owned()));
        assert_eq!(east.bearing, "090");
        // Across north, and a bearing between ticks.
        let between = layout(355.);
        assert_eq!(between.ticks[0], (170., "28".to_owned()));
        assert_eq!(between.ticks[8], (330., "00".to_owned()));
        assert_eq!(between.ticks.last().unwrap(), &(470., "07".to_owned()));
        assert_eq!(between.bearing, "355");
        assert_eq!(layout(-5.), between);
        assert_eq!(layout(359.6).bearing, "000");
    }
    #[test]
    fn bearing_matches_hud_heading() {
        let at = [100., 5000., -40.];
        let toward = |d: [f64; 3]| bearing(at, std::array::from_fn(|i| at[i] + d[i]));
        assert_eq!(toward([0., 0., 1.]), 0.);
        assert_eq!(toward([1., 0., 0.]), 90.);
        assert_eq!(toward([0., 300., -1.]), 180.);
        assert_eq!(toward([-1., 0., 0.]), 270.);
        for yaw in [-2.5f64, -0.1, 0.7, 3.] {
            let d = toward([yaw.sin(), 0., yaw.cos()]);
            assert!((d - crate::hud::heading(yaw)).abs() < 1e-9);
        }
    }
    fn font() -> Font {
        Font {
            height: 8,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 6,
                    pixels: vec![(0, 0), (5, 7)],
                })
                .collect(),
        }
    }
    fn inked(canvas: &FlightCanvas) -> Vec<(f64, f64)> {
        let width = canvas.size[0] as usize;
        canvas
            .pixels
            .chunks_exact(4)
            .enumerate()
            .filter(|(_, p)| p[3] != 0)
            .map(|(i, _)| ((i % width) as f64, (i / width) as f64))
            .collect()
    }
    fn windows(instruments: &Instruments, size: [f64; 2]) -> Vec<Rect> {
        (0..instruments.pages.len())
            .map(|slot| instruments.screen_rect(slot, size))
            .collect()
    }
    #[test]
    fn strip_narrows_to_clear_top_windows() {
        use crate::instruments::Layout;
        let large = Instruments::default();
        assert_eq!(large.layout, Layout::Large);
        // 4:3: the top windows' inner edges sit 170 layer px from each side.
        let rects = windows(&large, [960., 720.]);
        assert_eq!(half_width(960., &rects, 80., 10.), 480. - 170. * 1.5 - 10.);
        // One top-left window still narrows both ends alike.
        let one = windows(&Instruments::new(Layout::Large, Some(6)), [960., 720.]);
        assert_eq!(half_width(960., &one, 80., 10.), 480. - 170. * 1.5 - 10.);
        // Wide views leave the middle half clear.
        let rects = windows(&large, [1920., 720.]);
        assert_eq!(half_width(1920., &rects, 80., 10.), 480.);
        // Small windows sit along the bottom, below the strip.
        let small = windows(&Instruments::new(Layout::Small, None), [960., 720.]);
        assert_eq!(small.len(), 6);
        assert_eq!(half_width(960., &small, 80., 10.), 240.);
        assert_eq!(half_width(960., &[], 80., 10.), 240.);
    }
    #[test]
    fn without_windows_the_strip_spans_the_middle_half() {
        let mut canvas = FlightCanvas::default();
        canvas.blank([960, 720]);
        draw_clear(&mut canvas, &font(), [0, 255, 0], 90.);
        let ink = inked(&canvas);
        let (left, right) = ink
            .iter()
            .fold((f64::MAX, 0f64), |(l, r), &(x, _)| (l.min(x), r.max(x)));
        // A quarter of the view either side of the middle, plus half a label.
        assert!(left < 250. && left > 200., "{left}");
        assert!(right > 710. && right < 760., "{right}");
        assert!(ink.iter().all(|&(_, y)| (8. ..80.).contains(&y)));
        let at = |x: usize, y: usize| canvas.pixels[(y * 960 + x) * 4..][..4].to_vec();
        let (x, y) = ink[0];
        assert_eq!(at(x as usize, y as usize)[..3], [0, 255, 0]);
    }
    #[test]
    fn draws_only_along_the_top_clear_of_the_windows() {
        for (size, bearing) in [([960, 720], 20.), ([1280, 720], 20.), ([960, 720], 355.)] {
            let instruments = Instruments::default();
            let mut canvas = FlightCanvas::default();
            canvas.blank(size);
            draw(&mut canvas, &instruments, &font(), bearing);
            let rects = windows(&instruments, size.map(f64::from));
            let ink = inked(&canvas);
            assert!(!ink.is_empty());
            for &(x, y) in &ink {
                assert!((8. ..80.).contains(&y));
                assert!(
                    rects
                        .iter()
                        .all(|&(wx, wy, ww, wh)| x < wx - 4. || x >= wx + ww + 4. || y >= wy + wh),
                    "{size:?} ink at {x},{y} touches a window"
                );
            }
        }
    }
}
