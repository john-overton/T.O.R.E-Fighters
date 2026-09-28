//! Bearing compass for the F7 player to target view: the HUD heading strip
//! stretched across the top of the view, with a marker at the target's
//! bearing. Opinionated, requested by John on 2026-09-28.
use crate::{
    flight_canvas::{FlightCanvas, HUD_SCALE},
    instruments::Instruments,
};
use tore_formats::font::Font;

/// Strip ends in layout units. On screen the strip is the middle half of the
/// view, narrowed to clear instrument windows, still showing the same span.
const LEFT: f64 = 160.;
const RIGHT: f64 = 480.;
const PER_DEGREE: f64 = 2.;
/// Degrees either side of the heading the strip shows.
const SPAN: f64 = (RIGHT - LEFT) / 2. / PER_DEGREE;
/// Gap above the strip, in 640x480 layer units.
const MARGIN: f64 = 6.;
/// Clearance from an instrument window's inner edge, in layer units.
const GAP: f64 = 4.;
/// Length of the off-strip arrow, in HUD units.
const ARROW: f64 = 12.;
// Rows below the strip's top, in HUD units, spaced as the HUD's own strip.
const MARKER: f64 = 4.;
const LABEL: f64 = 11.;
const TICK: f64 = 24.;
const CARET: f64 = 31.;
const READOUT: f64 = 39.;

#[derive(Debug, PartialEq)]
enum Target {
    /// Strip x of the target's bearing.
    Marker(f64),
    /// Off the left end, with the three-digit bearing.
    Left(String),
    Right(String),
}
#[derive(Debug, PartialEq)]
struct Layout {
    /// Strip x and two-digit label of each 10-degree tick.
    ticks: Vec<(f64, String)>,
    heading: String,
    target: Target,
}
/// The strip in 640-wide units for compass `heading` and target `bearing`.
fn layout(heading: f64, bearing: f64) -> Layout {
    let heading = heading.rem_euclid(360.);
    let first = ((heading - SPAN) / 10.).ceil() as i32 * 10;
    let ticks = (0..)
        .map(|i| first + i * 10)
        .take_while(|&tick| f64::from(tick) <= heading + SPAN)
        .map(|tick| {
            (
                strip_x(f64::from(tick) - heading),
                format!("{:02}", tick.rem_euclid(360) / 10),
            )
        })
        .collect();
    // Relative bearing in (-180, 180]: a target dead astern reads right.
    let off = 180. - (heading - bearing + 180.).rem_euclid(360.);
    let target = if off.abs() <= SPAN {
        Target::Marker(strip_x(off))
    } else if off > 0. {
        Target::Right(three_digits(bearing))
    } else {
        Target::Left(three_digits(bearing))
    };
    Layout {
        ticks,
        heading: three_digits(heading),
        target,
    }
}
fn strip_x(off: f64) -> f64 {
    (LEFT + RIGHT) / 2. + off * PER_DEGREE
}
fn three_digits(degrees: f64) -> String {
    format!("{:03}", degrees.rem_euclid(360.).round() as u32 % 360)
}
/// Compass bearing in degrees from `from` to `to`, as `hud::heading` reads a
/// yaw: x east, z north.
pub fn bearing(from: [f64; 3], to: [f64; 3]) -> f64 {
    (to[0] - from[0])
        .atan2(to[2] - from[2])
        .to_degrees()
        .rem_euclid(360.)
}
/// Draw the strip over the flight view for aircraft `heading` and target
/// `bearing`, both compass degrees, in the HUD's font and color at the HUD's
/// on-screen size, smoothed like the in-flight messages. It narrows to clear
/// the instrument windows shown beside it.
pub fn draw(
    canvas: &mut FlightCanvas,
    instruments: &Instruments,
    font: &Font,
    heading: f64,
    bearing: f64,
) {
    let size = canvas.size.map(f64::from);
    let windows: Vec<_> = (0..instruments.pages.len())
        .map(|slot| instruments.screen_rect(slot, size))
        .collect();
    draw_within(
        canvas,
        font,
        instruments.hud_color,
        heading,
        bearing,
        &windows,
    );
}
type Rect = (f64, f64, f64, f64);
fn draw_within(
    canvas: &mut FlightCanvas,
    font: &Font,
    color: [u8; 3],
    heading: f64,
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
    let layout = layout(heading, bearing);
    let side = match layout.target {
        Target::Marker(_) => 0.,
        Target::Left(_) => -1.,
        Target::Right(_) => 1.,
    };
    let end = c + side * half;
    let centered = |text: &str, x: f64| x - FlightCanvas::text_width(font, text, scale) / 2.;
    for (x, label) in &layout.ticks {
        let x = across(*x);
        // The edge arrow takes the place of the ticks it would cross.
        if side != 0. && (end - x) * side < ARROW * scale {
            continue;
        }
        ink.line((x, y(TICK)), (x, y(TICK + 5.)));
        ink.text(font, label, centered(label, x), y(LABEL));
    }
    ink.line((c - 5. * scale, y(CARET)), (c, y(CARET + 5.)));
    ink.line((c, y(CARET + 5.)), (c + 5. * scale, y(CARET)));
    ink.text(
        font,
        &layout.heading,
        centered(&layout.heading, c),
        y(READOUT),
    );
    match &layout.target {
        Target::Marker(x) => {
            let (x, m, r) = (across(*x), y(MARKER), 4. * scale);
            ink.line((x, m - r), (x + r, m));
            ink.line((x + r, m), (x, m + r));
            ink.line((x, m + r), (x - r, m));
            ink.line((x - r, m), (x, m - r));
        }
        Target::Left(text) | Target::Right(text) => {
            // Outward arrow on the tick row inside the strip's end, the
            // bearing beneath it on the readout row.
            let at = |units: f64| end - side * units * scale;
            let m = y(TICK + 2.5);
            ink.line((at(ARROW - 1.), m), (at(0.5), m));
            ink.line((at(0.5), m), (at(4.5), m - 4. * scale));
            ink.line((at(0.5), m), (at(4.5), m + 4. * scale));
            let width = FlightCanvas::text_width(font, text, scale);
            let left = if side > 0. { end - width } else { end };
            ink.text(font, text, left, y(READOUT));
        }
    }
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
        for c in text.bytes() {
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
    fn marker(heading: f64, bearing: f64) -> Target {
        layout(heading, bearing).target
    }
    #[test]
    fn ticks_every_ten_degrees_at_two_pixels_per_degree() {
        let north = layout(0., 0.);
        assert_eq!(north.ticks.len(), 17);
        assert_eq!(north.ticks[0], (160., "28".to_owned()));
        assert_eq!(north.ticks[8], (320., "00".to_owned()));
        assert_eq!(north.ticks[16], (480., "08".to_owned()));
        assert!(north.ticks.windows(2).all(|t| t[1].0 - t[0].0 == 20.));
        assert_eq!(north.heading, "000");
        let east = layout(90., 0.);
        assert_eq!(east.ticks[0], (160., "01".to_owned()));
        assert_eq!(east.ticks[8], (320., "09".to_owned()));
        assert_eq!(east.ticks[16], (480., "17".to_owned()));
        assert_eq!(east.heading, "090");
        let between = layout(355., 0.);
        assert_eq!(between.ticks[0], (170., "28".to_owned()));
        assert_eq!(between.ticks.last().unwrap(), &(470., "07".to_owned()));
        assert_eq!(between.heading, "355");
    }
    #[test]
    fn marker_wraps_across_north() {
        assert_eq!(marker(355., 5.), Target::Marker(340.));
        assert_eq!(marker(5., 355.), Target::Marker(300.));
        assert_eq!(marker(-5., 5.), Target::Marker(340.));
    }
    #[test]
    fn marker_reaches_both_ends_of_the_strip() {
        assert_eq!(marker(10., 90.), Target::Marker(480.));
        assert_eq!(marker(10., 290.), Target::Marker(160.));
        assert_eq!(marker(0., 80.6), Target::Right("081".to_owned()));
        assert_eq!(marker(0., 279.4), Target::Left("279".to_owned()));
    }
    #[test]
    fn targets_beyond_the_strip_point_off_the_matching_end() {
        assert_eq!(marker(0., 100.), Target::Right("100".to_owned()));
        assert_eq!(marker(0., 250.), Target::Left("250".to_owned()));
        assert_eq!(marker(350., 200.), Target::Left("200".to_owned()));
        assert_eq!(marker(300., 60.), Target::Right("060".to_owned()));
        assert_eq!(marker(0., 359.5), Target::Marker(319.));
    }
    #[test]
    fn target_dead_astern_points_right() {
        assert_eq!(marker(0., 180.), Target::Right("180".to_owned()));
        assert_eq!(marker(90., 270.), Target::Right("270".to_owned()));
        assert_eq!(marker(270., 90.), Target::Right("090".to_owned()));
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
    fn draws_only_along_the_top_clear_of_the_windows() {
        for (size, bearing) in [([960, 720], 20.), ([1280, 720], 20.), ([960, 720], 180.)] {
            let instruments = Instruments::default();
            let mut canvas = FlightCanvas::default();
            canvas.blank(size);
            draw(&mut canvas, &instruments, &font(), 0., bearing);
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
    #[test]
    fn edge_arrow_and_bearing_stay_inside_the_strip() {
        let size = [960u32, 720];
        let rects = windows(&Instruments::default(), size.map(f64::from));
        for (heading, bearing, side) in [(0., 150., 1.), (0., 210., -1.), (350., 170., 1.)] {
            let mut canvas = FlightCanvas::default();
            canvas.blank(size);
            draw_within(&mut canvas, &font(), [0, 255, 0], heading, bearing, &rects);
            let scale = 1.5 * HUD_SCALE;
            let inset = GAP * 1.5 + FlightCanvas::text_width(&font(), "00", scale) / 2.;
            let half = half_width(960., &rects, 80., inset);
            let end = 480. + side * half;
            let ink = inked(&canvas);
            // Nothing beyond the arrow's end of the strip.
            assert!(ink.iter().all(|&(x, _)| (x + 0.5 - end) * side < 1.));
            // The arrow tip and the bearing text reach that end.
            assert!(ink.iter().any(|&(x, _)| (x + 0.5 - end).abs() < 1.5));
        }
    }
}
