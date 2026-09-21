//! Filled clean-aircraft chart. See docs/spec/envelope.md for fitted presentation.
use super::{CombatReadout, Raster};
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
    fn point(&self, speed: f64, altitude: f64) -> (i32, i32) {
        (
            11 + (speed / self.speed * 137.).round() as i32,
            134 - (altitude / self.altitude * 113.).round() as i32,
        )
    }
    fn marker(&self, speed: f64, altitude: f64) -> (i32, i32) {
        let (x, y) = self.point(speed, altitude);
        ((x - 2).clamp(11, 145), (y - 2).clamp(21, 131))
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
fn current(rows: &[Envelope], g: f64) -> Option<&Envelope> {
    rows.iter()
        .filter(|e| e.points.len() >= 3)
        .min_by_key(|e| (i64::from(e.g) - g.round() as i64).abs())
}
fn chart(r: &mut Raster, rows: &[Envelope], target: &[Envelope], mode: Mode, g: f64) -> Scale {
    let scale = Scale::new(rows, target);
    let current = current(rows, g);
    // Above the ceiling, continue the stall-side background up from the summit.
    let summit = rows
        .iter()
        .filter(|e| e.g > 0)
        .flat_map(|e| &e.points)
        .filter(|p| p[0].is_finite() && p[1].is_finite())
        .max_by(|a, b| a[1].total_cmp(&b[1]).then_with(|| b[0].total_cmp(&a[0])))
        .copied()
        .unwrap_or([0., 0.]);
    for y in 21..=134 {
        let altitude = (134 - y) as f64 / 113. * scale.altitude;
        let own = spans(rows, altitude);
        let other = spans(target, altitude);
        let left = own
            .iter()
            .map(|(_, low, _)| *low)
            .reduce(f64::min)
            .unwrap_or(summit[0]);
        let selected = current.and_then(|e| e.speeds(altitude));
        for x in 11..=148 {
            let speed = (x - 11) as f64 / 137. * scale.speed;
            let own_g = available(&own, speed);
            let background = if speed < left {
                STALL
            } else if altitude > summit[1] {
                HIGH
            } else {
                FAST
            };
            let color = if mode == Mode::Current {
                if selected.is_some_and(|(low, high)| (low..=high).contains(&speed)) {
                    BANDS[0]
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
    let right = |r: &mut Raster, s: &str, y| r.text(font, s, 147 - width(s), y, INK);
    r.text(font, &format!("{:.0} FT", state.position[1]), 13, 24, INK);
    right(r, &format!("{:.0} G", state.g), 24);
    right(r, &format!("{:.0} KTS", state.speed / 1.68781), 123);
    if mode == Mode::Compare {
        let status = if !locked {
            Some("NO LOCK")
        } else if target.is_none_or(|t| t.is_empty()) {
            Some("NO TARGET DATA")
        } else {
            None
        };
        if let Some(status) = status {
            r.text(font, status, 80 - width(status) / 2, 72, INK);
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
    fn pixel(r: &Raster, p: (i32, i32)) -> [u8; 4] {
        let i = (p.1 as usize * super::super::WIDTH + p.0 as usize) * 4;
        r.pixels[i..i + 4].try_into().unwrap()
    }
    #[test]
    fn filled_bands_current_curve_and_frame_clipping() {
        let rows = rows();
        let mut r = Raster::new();
        let scale = chart(&mut r, &rows, &[], Mode::All, 1.);
        assert_eq!(pixel(&r, scale.point(500., 2000.)), BANDS[3]);
        assert_eq!(pixel(&r, scale.point(220., 2000.)), BANDS[0]);
        assert_eq!(pixel(&r, scale.point(50., 2000.)), STALL);
        assert_eq!(pixel(&r, scale.point(1020., 2000.)), FAST);
        chart(&mut r, &rows, &[], Mode::Current, 4.);
        assert_eq!(pixel(&r, scale.point(500., 2000.)), BANDS[0]);
        assert_eq!(pixel(&r, scale.point(220., 2000.)), FAST);
        for y in 0..super::super::HEIGHT as i32 {
            for x in 0..super::super::WIDTH as i32 {
                if !(11..=148).contains(&x) || !(21..=134).contains(&y) {
                    assert_eq!(pixel(&r, (x, y)), [0; 4]);
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
                assert!((11..=148).contains(&x) && (21..=134).contains(&y));
            }
            assert_eq!(scale.marker(-100., -100.), (11, 131));
            assert_eq!(scale.marker(1e9, 1e9), (145, 21));
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
        let mut r = Raster::new();
        let scale = chart(&mut r, &rows, &target, Mode::Compare, 1.);
        assert_eq!(pixel(&r, scale.point(500., 2000.)), ADVANTAGE);
        assert_eq!(pixel(&r, scale.point(220., 2000.)), BANDS[0]);
        assert_eq!(current(&rows, 3.6).unwrap().g, 4);
    }
}
