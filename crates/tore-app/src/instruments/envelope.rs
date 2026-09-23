//! Filled clean-aircraft chart. See docs/spec/envelope.md for fitted presentation.
use super::{CombatReadout, Raster, SCREEN};
use crate::flight::State;
use tore_formats::{aircraft::Envelope, font::Font};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Current,
    All,
    Compare,
}
const BANDS: [[u8; 4]; 9] = [
    [44, 91, 107, 255],
    [48, 101, 117, 255],
    [56, 114, 131, 255],
    [67, 131, 149, 255],
    [78, 144, 161, 255],
    [86, 151, 167, 255],
    [94, 158, 174, 255],
    [101, 163, 178, 255],
    [108, 169, 183, 255],
];
const INK: [u8; 4] = [198, 225, 228, 255];
const STALL: [u8; 4] = [81, 145, 161, 255];
const FAST: [u8; 4] = [128, 177, 188, 255];
const HIGH: [u8; 4] = [177, 207, 213, 255];
const ADVANTAGE: [u8; 4] = [175, 72, 73, 255];
/// U mode's single fill, measured in retail at both 1 G and about 5.7 G.
const CURRENT: [u8; 4] = [48, 99, 118, 255];
/// Last screen column and row: the chart fills the whole screen.
const RIGHT: i32 = SCREEN.2 - 1;
const BOTTOM: i32 = SCREEN.3 - 1;

#[derive(Debug)]
struct Scale {
    speed: f64,
    altitude: f64,
}
impl Scale {
    fn new(rows: &[Envelope], target: &[Envelope]) -> Self {
        let mut speed = 1f64;
        let mut altitude = 1f64;
        for p in rows
            .iter()
            .chain(target)
            .filter(|e| e.g > 0)
            .flat_map(|e| &e.points)
        {
            if p[0].is_finite() && p[1].is_finite() {
                speed = speed.max(p[0]);
                altitude = altitude.max(p[1]);
            }
        }
        Self {
            speed: speed * 1.06,
            altitude: altitude * 1.12,
        }
    }
    /// Screen position: zero speed on the left edge, zero altitude on the
    /// bottom row, and the scale's maxima on the right edge and top row.
    fn point(&self, speed: f64, altitude: f64) -> (i32, i32) {
        (
            (speed / self.speed * f64::from(RIGHT)).round() as i32,
            BOTTOM - (altitude / self.altitude * f64::from(BOTTOM)).round() as i32,
        )
    }
    fn marker(&self, speed: f64, altitude: f64) -> (i32, i32) {
        let (x, y) = self.point(speed, altitude);
        ((x - 2).clamp(0, RIGHT - 3), (y - 2).clamp(0, BOTTOM - 3))
    }
}
fn marker_color(tick: u64) -> [u8; 4] {
    [
        [232, 248, 40, 255],
        [96, 207, 233, 255],
        [239, 249, 246, 255],
    ][(tick / 20 % 3) as usize]
}
fn spans(rows: &[Envelope], altitude: f64) -> Vec<(i32, f64, f64)> {
    rows.iter()
        .filter(|e| e.g > 0)
        .filter_map(|e| e.speeds(altitude).map(|(low, high)| (e.g, low, high)))
        .collect()
}
fn available(spans: &[(i32, f64, f64)], speed: f64) -> i32 {
    spans
        .iter()
        .filter(|(_, low, high)| speed >= *low && speed <= *high)
        .map(|(g, _, _)| *g)
        .max()
        .unwrap_or(0)
}
fn band(g: i32) -> [u8; 4] {
    BANDS[(g - 1).clamp(0, 8) as usize]
}
/// The row nearest live G rounded to a whole number. Rows run contiguously
/// from the aircraft's lowest to highest G, so G beyond either end keeps the
/// end row rather than jumping to a larger curve.
fn current(rows: &[Envelope], g: f64) -> Option<&Envelope> {
    rows.iter()
        .filter(|e| e.points.len() >= 3)
        .min_by_key(|e| (i64::from(e.g) - g.round() as i64).abs())
}
/// Divides everything outside one outline into three shades. The stall
/// shade lies left of the outline, and straight up from the slow end of its
/// ceiling. The high shade lies above the outline from there across to its
/// fastest point. The fast shade lies right of the outline below that point.
struct Outline<'a> {
    rows: Vec<&'a Envelope>,
    summit: [f64; 2],
    fastest: [f64; 2],
}
impl<'a> Outline<'a> {
    fn new(rows: Vec<&'a Envelope>) -> Self {
        let points = || {
            rows.iter()
                .flat_map(|e| &e.points)
                .filter(|p| p[0].is_finite() && p[1].is_finite())
        };
        // Highest point, slowest on a flat ceiling.
        let summit = points()
            .max_by(|a, b| a[1].total_cmp(&b[1]).then_with(|| b[0].total_cmp(&a[0])))
            .copied()
            .unwrap_or([0., 0.]);
        // Fastest point, lowest on a vertical fast edge.
        let fastest = points()
            .max_by(|a, b| a[0].total_cmp(&b[0]).then_with(|| b[1].total_cmp(&a[1])))
            .copied()
            .unwrap_or([0., 0.]);
        Self {
            rows,
            summit,
            fastest,
        }
    }
    /// Slowest edge of the outline at this altitude.
    fn stall_edge(&self, altitude: f64) -> f64 {
        if altitude > self.summit[1] {
            return self.summit[0];
        }
        self.rows
            .iter()
            .filter_map(|e| e.speeds(altitude))
            .map(|(low, _)| low)
            .reduce(f64::min)
            .unwrap_or(self.summit[0])
    }
    fn background(&self, stall_edge: f64, speed: f64, altitude: f64) -> [u8; 4] {
        if speed < stall_edge {
            STALL
        } else if altitude > self.fastest[1] {
            HIGH
        } else {
            FAST
        }
    }
}
fn chart(r: &mut Raster, rows: &[Envelope], target: &[Envelope], mode: Mode, g: f64) -> Scale {
    let scale = Scale::new(rows, target);
    let current = current(rows, g);
    // U mode shades around the selected row, so the space it gives up as
    // G rises takes the stall, high or fast shade it now lies in.
    let outline = Outline::new(match (mode, current) {
        (Mode::Current, Some(e)) => vec![e],
        _ => rows.iter().filter(|e| e.g > 0).collect(),
    });
    for y in 0..=BOTTOM {
        let altitude = f64::from(BOTTOM - y) / f64::from(BOTTOM) * scale.altitude;
        let own = spans(rows, altitude);
        let other = spans(target, altitude);
        let stall_edge = outline.stall_edge(altitude);
        let selected = current.and_then(|e| e.speeds(altitude));
        for x in 0..=RIGHT {
            let speed = f64::from(x) / f64::from(RIGHT) * scale.speed;
            let own_g = available(&own, speed);
            let background = outline.background(stall_edge, speed, altitude);
            let color = if mode == Mode::Current {
                if selected.is_some_and(|(low, high)| (low..=high).contains(&speed)) {
                    CURRENT
                } else {
                    background
                }
            } else if mode == Mode::Compare && available(&other, speed) > own_g {
                ADVANTAGE
            } else if own_g > 0 {
                band(own_g)
            } else {
                background
            };
            r.rect(x, y, 1, 1, color);
        }
    }
    scale
}
/// Whole-G readout; a light push shows 0 G rather than -0 G.
fn g_label(g: f64) -> String {
    format!("{:.0} G", g.round() + 0.)
}

pub(super) fn draw(
    r: &mut Raster,
    font: &Font,
    rows: &[Envelope],
    state: &State,
    mode: Mode,
    combat: Option<&CombatReadout>,
) {
    let locked = combat.is_some_and(|c| {
        c.scope
            .contacts
            .iter()
            .any(|p| p.selected && p.acquired && !p.stale)
    });
    let target = combat.and_then(|c| c.envelope_target.as_deref());
    let comparing = mode == Mode::Compare && locked;
    let scale = chart(
        r,
        rows,
        if comparing {
            target.unwrap_or(&[])
        } else {
            &[]
        },
        mode,
        state.g,
    );
    let (x, y) = scale.marker(state.speed, state.position[1]);
    r.rect(x, y, 4, 4, marker_color(state.ticks));
    let width = |s: &str| {
        s.bytes()
            .map(|ch| font.glyphs[ch as usize].advance as i32)
            .sum::<i32>()
    };
    let right = |r: &mut Raster, s: &str, y| r.text(font, s, 136 - width(s), y, INK);
    r.text(font, &format!("{:.0} FT", state.position[1]), 2, 3, INK);
    right(r, &g_label(state.g), 3);
    right(r, &format!("{:.0} KTS", state.speed / 1.68781), 102);
    if mode == Mode::Compare {
        let status = if !locked {
            Some("NO LOCK")
        } else if target.is_none_or(|t| t.is_empty()) {
            Some("NO TARGET DATA")
        } else {
            None
        };
        if let Some(status) = status {
            r.text(font, status, 69 - width(status) / 2, 51, INK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rows() -> Vec<Envelope> {
        vec![
            Envelope {
                g: 1,
                points: vec![[100., 0.], [200., 10000.], [800., 10000.], [1000., 0.]],
            },
            Envelope {
                g: 4,
                points: vec![[300., 0.], [400., 5000.], [600., 5000.], [700., 0.]],
            },
        ]
    }
    /// Synthetic nested rows 1..=6 G: each shrinks towards a corner-speed core.
    fn nested() -> Vec<Envelope> {
        (1..=6)
            .map(|g| {
                let k = f64::from(g - 1);
                let stall = 150. + 70. * k;
                let top = 50000. - 7500. * k;
                let fast = 1500. - 70. * k;
                Envelope {
                    g,
                    points: vec![
                        [stall, 0.],
                        [stall + 150., top * 0.5],
                        [700., top],
                        [1100. - 40. * k, top],
                        [fast, top * 0.55],
                        [fast - 250., 0.],
                    ],
                }
            })
            .collect()
    }
    fn pixel(r: &Raster, p: (i32, i32)) -> [u8; 4] {
        r.at(p.0, p.1)
    }
    #[test]
    fn filled_bands_current_curve_and_frame_clipping() {
        let rows = rows();
        let mut r = Raster::screen();
        let scale = chart(&mut r, &rows, &[], Mode::All, 1.);
        assert_eq!(pixel(&r, scale.point(500., 2000.)), BANDS[3]);
        assert_eq!(pixel(&r, scale.point(220., 2000.)), BANDS[0]);
        assert_eq!(pixel(&r, scale.point(50., 2000.)), STALL);
        // The fastest 1 G point is at sea level, so everything right of it is high.
        assert_eq!(pixel(&r, scale.point(1020., 2000.)), HIGH);
        chart(&mut r, &rows, &[], Mode::Current, 4.);
        assert_eq!(pixel(&r, scale.point(500., 2000.)), CURRENT);
        assert_eq!(pixel(&r, scale.point(220., 2000.)), STALL);
        // Window pixels outside the screen stay untouched.
        let (sx, sy, sw, sh) = SCREEN;
        for y in 0..super::super::HEIGHT as i32 {
            for x in 0..super::super::WIDTH as i32 {
                if !(sx..sx + sw).contains(&x) || !(sy..sy + sh).contains(&y) {
                    assert_eq!(pixel(&r, (x - sx, y - sy)), [0; 4]);
                }
            }
        }
    }
    #[test]
    fn aircraft_bounds_and_marker_share_scale_and_color_clock() {
        for factor in [0.25, 1., 3.] {
            let rows: Vec<_> = rows()
                .into_iter()
                .map(|mut e| {
                    for p in &mut e.points {
                        p[0] *= factor;
                        p[1] *= factor;
                    }
                    e
                })
                .collect();
            let scale = Scale::new(&rows, &[]);
            for p in rows.iter().flat_map(|e| &e.points) {
                let (x, y) = scale.point(p[0], p[1]);
                assert!((0..=137).contains(&x) && (0..=113).contains(&y));
            }
            assert_eq!(scale.marker(-100., -100.), (0, 110));
            assert_eq!(scale.marker(1e9, 1e9), (134, 0));
        }
        assert_eq!(marker_color(0), marker_color(19));
        assert_ne!(marker_color(19), marker_color(20));
        assert_ne!(marker_color(20), marker_color(40));
        assert_eq!(marker_color(0), marker_color(60));
    }
    #[test]
    fn compare_uses_shared_scale_and_only_shades_target_advantage() {
        let rows = rows();
        let mut target = rows.clone();
        target[1].g = 8;
        let mut r = Raster::screen();
        let scale = chart(&mut r, &rows, &target, Mode::Compare, 1.);
        assert_eq!(pixel(&r, scale.point(500., 2000.)), ADVANTAGE);
        assert_eq!(pixel(&r, scale.point(220., 2000.)), BANDS[0]);
        assert_eq!(current(&rows, 3.6).unwrap().g, 4);
    }
    fn count(r: &Raster, color: [u8; 4]) -> usize {
        r.pixels.chunks_exact(4).filter(|p| *p == color).count()
    }
    #[test]
    fn current_region_steps_per_whole_g_shrinks_and_clamps_at_the_top_row() {
        let rows = nested();
        let area = |g: f64| {
            let mut r = Raster::screen();
            chart(&mut r, &rows, &[], Mode::Current, g);
            (count(&r, CURRENT), r.pixels)
        };
        let areas: Vec<_> = (1..=6).map(|g| area(f64::from(g)).0).collect();
        assert!(areas.windows(2).all(|w| w[0] > w[1]), "{areas:?}");
        assert!(areas[5] > 0);
        // No interpolation between rows: 1.4 G is the 1 G row, 1.6 G the 2 G row.
        assert_eq!(area(1.4).1, area(1.).1);
        assert_eq!(area(1.6).1, area(2.).1);
        // Beyond the highest row the region stays at that row.
        assert_eq!(area(6.).1, area(9.).1);
        assert_eq!(area(6.).1, area(12.4).1);
    }
    #[test]
    fn vacated_space_takes_the_current_row_backgrounds() {
        let rows = nested();
        let mut r = Raster::screen();
        let scale = chart(&mut r, &rows, &[], Mode::Current, 1.);
        for p in [
            (300., 2000.),
            (800., 20000.),
            (1100., 3000.),
            (1100., 10000.),
        ] {
            assert_eq!(pixel(&r, scale.point(p.0, p.1)), CURRENT, "{p:?}");
        }
        chart(&mut r, &rows, &[], Mode::Current, 6.);
        // Left of the 6 G stall edge: stall shade, as in the retail pull.
        assert_eq!(pixel(&r, scale.point(300., 2000.)), STALL);
        // Above the 6 G ceiling: stall shade left of its slow end, high beyond.
        assert_eq!(pixel(&r, scale.point(600., 20000.)), STALL);
        assert_eq!(pixel(&r, scale.point(800., 20000.)), HIGH);
        // Right of the 6 G fast edge: fast below its fastest point, high above.
        assert_eq!(pixel(&r, scale.point(1100., 3000.)), FAST);
        assert_eq!(pixel(&r, scale.point(1100., 10000.)), HIGH);
        assert_eq!(pixel(&r, scale.point(800., 3000.)), CURRENT);
    }
    #[test]
    fn all_curves_split_high_and_fast_at_the_fastest_point() {
        let rows = nested();
        let mut r = Raster::screen();
        let scale = chart(&mut r, &rows, &[], Mode::All, 1.);
        // 1 G fastest point is (1500 ft/s, 27,500 ft); its ceiling is 50,000 ft.
        assert_eq!(pixel(&r, scale.point(1450., 10000.)), FAST);
        assert_eq!(pixel(&r, scale.point(1480., 40000.)), HIGH);
        assert_eq!(pixel(&r, scale.point(600., 53000.)), STALL);
        assert_eq!(pixel(&r, scale.point(800., 53000.)), HIGH);
        assert_eq!(pixel(&r, scale.point(100., 2000.)), STALL);
    }
    #[test]
    fn low_and_negative_g_pick_the_nearest_row_and_whole_g_readout() {
        let mut rows = nested();
        assert_eq!(current(&rows, 0.2).unwrap().g, 1);
        assert_eq!(current(&rows, -3.).unwrap().g, 1);
        let mut zero = rows[0].clone();
        zero.g = 0;
        let mut push = rows[1].clone();
        push.g = -1;
        rows.splice(0..0, [push, zero]);
        assert_eq!(current(&rows, 0.2).unwrap().g, 0);
        assert_eq!(current(&rows, -1.2).unwrap().g, -1);
        assert_eq!(current(&rows, -7.).unwrap().g, -1);
        assert_eq!(current(&rows, 0.6).unwrap().g, 1);
        assert_eq!(g_label(5.7), "6 G");
        assert_eq!(g_label(1.4), "1 G");
        assert_eq!(g_label(-0.3), "0 G");
        assert_eq!(g_label(-1.6), "-2 G");
    }
    /// Development check: with TORE_ENVELOPE_DUMP=DIR, writes U-mode charts at
    /// 1 G and 6 G as PPM. TORE_ENVELOPE_ROWS=FILE substitutes local rows, one
    /// per line as `g speed,altitude speed,altitude ...` (never committed).
    #[test]
    fn dump_current_mode_charts_on_request() {
        let Some(dir) = std::env::var_os("TORE_ENVELOPE_DUMP") else {
            return;
        };
        let rows = std::env::var_os("TORE_ENVELOPE_ROWS").map_or_else(nested, |path| {
            std::fs::read_to_string(path)
                .unwrap()
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    let mut words = l.split_whitespace();
                    Envelope {
                        g: words.next().unwrap().parse().unwrap(),
                        points: words
                            .map(|p| {
                                let (s, a) = p.split_once(',').unwrap();
                                [s.parse().unwrap(), a.parse().unwrap()]
                            })
                            .collect(),
                    }
                })
                .collect()
        });
        for (mode, name) in [(Mode::Current, "u"), (Mode::All, "a")] {
            for g in [1., 6.] {
                let mut r = Raster::screen();
                chart(&mut r, &rows, &[], mode, g);
                let mut out = format!(
                    "P6\n{} {}\n255\n",
                    super::super::WIDTH,
                    super::super::HEIGHT
                )
                .into_bytes();
                for p in r.pixels.chunks_exact(4) {
                    out.extend_from_slice(&p[..3]);
                }
                let path = std::path::Path::new(&dir).join(format!("{name}-{g}g.ppm"));
                std::fs::write(path, out).unwrap();
            }
        }
    }
}
