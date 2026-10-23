//! The HUD's bomb and rocket sights (docs/spec/air-to-ground.md, "The HUD in
//! air-to-ground modes"): the CCIP pipper with its fall line or dot trail,
//! the level release "T", the rocket pipper, and their text. Everything drawn
//! comes from the readout's [`BombSight`], which combat works out each tick.
//!
//! Retail data (docs/formats/weapons.md, "Bomb sight"): the pipper is one
//! HUDSYM11 glyph, a 23-pixel ring with twelve clock marks and a centre dot,
//! whose inner 17-pixel arc grows clockwise from 12 o'clock in fifteen steps
//! as the range closes; the fall line is dotted from the flight path marker
//! (or the boresight) to the pipper, or a dot every quarter second of fall on
//! the F-16C, F-22, F-22N and X-31. The glyph is drawn here from those
//! measurements rather than from the font. The layout of the text, the
//! off-HUD caret and the whole level "T" are fitted or opinionated, as the
//! spec labels them.
use super::projected;
use crate::{flight, frame::FlightFrame, hud};
use tore_formats::{aircraft::AircraftId, font::Font, weapons::Weapon};
use tore_sim::{
    attitude::Vector,
    combat::{
        delivery::{Ccip, DeliveryMode, LevelRelease, Warning},
        missiles,
    },
};
use tore_world::readout::BombSight;

/// The pipper's outer ring radius, reference pixels (the glyph's 23-pixel
/// ring).
pub const RING: f64 = 11.;
/// The inner arc's radius (the glyph's 17-pixel arc).
pub const ARC: f64 = 8.;
/// The cardinal clock marks reach the glyph's 29-pixel cell edge.
const CARDINAL_MARK: f64 = 14.;
/// The other eight clock marks are short ticks outside the ring.
const MINOR_MARK: f64 = 13.;
/// Where the pipper may stand: the HUD's square cue region, as the gun's.
const LEFT: f64 = 184.;
const RIGHT: f64 = 456.;
const TOP: f64 = 106.;
fn bottom() -> f64 {
    f64::from(hud::AIM_BOTTOM - 10)
}
/// The top of the level release stem, reference pixels, under the heading
/// readout (opinionated, agent).
const STEM_TOP: f64 = 176.;
/// Dot trail dots nearer the eye than this are not drawn: just after release
/// the station's offset would sweep them across the HUD (fitted).
const TRAIL_NEAREST_FT: f64 = 1_000.;
/// The crossbar stands this far above the flight path marker, clear of its
/// fin, and reaches this far to each side (opinionated, agent).
const CROSSBAR_RISE: f64 = 12.;
const CROSSBAR_HALF: f64 = 16.;
/// The release marker's radius.
const MARKER: f64 = 3.;

/// The fall line options of the aircraft's HUD module (retail data, R1,
/// docs/formats/weapons.md, "Bomb sight", transcribed by module name): the
/// dot trail (HUD byte +0x23a), the flight path marker as the line's start
/// (+0x23b) and the speed it needs (+0x23e, ft/s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SightStyle {
    pub trail: bool,
    pub flight_path: bool,
    pub minimum_fps: f64,
}

impl SightStyle {
    pub fn of(aircraft: AircraftId) -> Self {
        let module = aircraft.hud();
        Self {
            trail: matches!(module, "F16C.HUD" | "F22.HUD" | "F22N.HUD" | "F31.HUD"),
            // Only the X-31 family's modules lack the flight path option.
            flight_path: module != "F31.HUD",
            minimum_fps: if matches!(
                module,
                "AV8.HUD" | "F4.HUD" | "MIG17.HUD" | "MIG21.HUD" | "YAK141.HUD"
            ) {
                146.
            } else {
                0.
            },
        }
    }
}

/// Where the pipper goes: on the HUD, or a caret at the HUD's lower edge
/// where the fall line leaves it (TORE's addition for a level pass; retail
/// draws nothing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PipperCue {
    On((f64, f64)),
    Caret((f64, f64)),
    /// A rocket's pipper off the field of view: an arrowhead pinned at the
    /// edge where it left, pointing the way to the impact (the unit
    /// direction). Opinionated, John 2026-10-12: the rocket pipper is always
    /// shown.
    Edge((f64, f64), (f64, f64)),
}

/// The rocket pipper's cue for an impact projected at `impact`: the pipper
/// on the field, else an arrowhead pinned at the field's edge nearest the
/// impact and pointing at it, else (behind the view) the caret under
/// `reference`. Never nothing: the rocket pipper is always shown.
pub fn rocket_cue(reference: (f64, f64), impact: Option<(f64, f64)>) -> PipperCue {
    let Some((x, y)) = impact else {
        return PipperCue::Caret((reference.0.clamp(LEFT, RIGHT), bottom()));
    };
    if (LEFT..=RIGHT).contains(&x) && (TOP..=bottom()).contains(&y) {
        return PipperCue::On((x, y));
    }
    let at = (x.clamp(LEFT, RIGHT), y.clamp(TOP, bottom()));
    let (dx, dy) = (x - at.0, y - at.1);
    let length = dx.hypot(dy);
    PipperCue::Edge(at, (dx / length, dy / length))
}

/// The pipper cue for an impact projected at `impact` (`None` when it cannot
/// be projected) from the fall line's start at `reference`.
pub fn pipper_cue(reference: (f64, f64), impact: Option<(f64, f64)>) -> Option<PipperCue> {
    match impact {
        Some((x, y)) if (LEFT..=RIGHT).contains(&x) && (TOP..=bottom()).contains(&y) => {
            Some(PipperCue::On((x, y)))
        }
        Some((x, y)) if y > bottom() => {
            // Where the line from the reference to the impact crosses the
            // lower edge.
            let t = if y - reference.1 > 1e-9 {
                ((bottom() - reference.1) / (y - reference.1)).clamp(0., 1.)
            } else {
                0.
            };
            let at = reference.0 + (x - reference.0) * t;
            Some(PipperCue::Caret((at.clamp(LEFT, RIGHT), bottom())))
        }
        // Behind the view, or off the HUD above or to a side: a bomb's
        // impact below the nose that projects nowhere gets the caret under
        // the reference.
        None => Some(PipperCue::Caret((reference.0.clamp(LEFT, RIGHT), bottom()))),
        Some(_) => None,
    }
}

/// The HUD status word (docs/spec/air-to-ground.md, "Status word and mode
/// choice"): CCIP or LVL for a bomb, ARM for a rocket pod.
pub fn status(sight: &BombSight) -> &'static str {
    hud::status_word(false, false, true, Some(sight))
}

/// `m:ss`, never negative.
pub fn clock(seconds: f64) -> String {
    if !seconds.is_finite() || seconds >= 6_000. {
        return "-:--".into();
    }
    let whole = seconds.max(0.).round() as u64;
    format!("{}:{:02}", whole / 60, whole % 60)
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    pixels: &mut [u8],
    frame: &FlightFrame,
    font: &Font,
    color: [u8; 3],
    zoom: f64,
    weapon: &Weapon,
    sight: &BombSight,
) {
    let s = frame.presented();
    let ro = &frame.readout;
    let mut paint = hud::Paint {
        pixels,
        clip: hud::HUD_CLIP,
        color: [color[0], color[1], color[2], 255],
    };
    let rounds = ro.stores.rounds(ro.stores.selected());
    paint.text(font, status(sight), 207, 259);
    paint.text(font, &format!("{rounds} {}", weapon.hud_name), 207, 271);
    let level = sight.level.filter(|_| sight.mode == DeliveryMode::Level);
    // Left column, third row: the range or release cue.
    match (sight.mode, level, sight.ccip) {
        (DeliveryMode::Level, Some(l), _) if l.overflown() => {
            paint.text(font, Warning::Overfly.label(), 207, 283)
        }
        (DeliveryMode::Level, Some(l), _) => {
            paint.text(font, &format!("REL {}", clock(l.release_seconds)), 207, 283)
        }
        (DeliveryMode::Level, None, _) if !sight.rocket => paint.text(font, "NO TARGET", 207, 283),
        (_, _, Some(c)) if c.in_range() => paint.text(font, "IN RNG", 207, 283),
        _ => {}
    }
    let warnings: Vec<&str> = sight
        .warnings
        .iter()
        .filter(|w| **w != Warning::Overfly)
        .map(|w| w.label())
        .collect();
    if !warnings.is_empty() {
        paint.text(font, &warnings.join(" "), 207, 295);
    }
    // Right column: the range, then the last bomb's time to impact.
    let range = match level {
        Some(l) => Some(l.slant_ft),
        None => sight.ccip.map(|c| c.slant_ft),
    };
    if let Some(range) = range {
        paint.text(font, &format!("R {:.1}", range / missiles::NMI), 402, 259);
    }
    if let Some(falling) = sight.falling {
        paint.text(font, &format!("IMP {}", clock(falling)), 402, 271);
    }
    if rounds == 0 || ro.estimates.readiness == tore_sim::combat::live::Readiness::StationFailed {
        return;
    }
    let style = SightStyle::of(frame.config.aircraft);
    let fpm = flight_path_marker(s, zoom);
    match level {
        Some(l) => draw_level(&mut paint, s, zoom, fpm, &Tee::of(&l)),
        None => {
            if let Some(c) = &sight.ccip {
                let bore = projected(
                    tore_sim::attitude::Basis::new(s.yaw, s.pitch, s.bank).forward,
                    s,
                    zoom,
                );
                let reference = fpm
                    .filter(|_| style.flight_path && s.speed >= style.minimum_fps)
                    .or(bore)
                    .unwrap_or((320., 240.));
                draw_ccip(
                    &mut paint,
                    s,
                    zoom,
                    reference,
                    c,
                    style.trail && !sight.rocket,
                    !sight.rocket,
                );
            }
        }
    }
}

/// The flight path marker's centre, as `hud::draw` places it.
pub(super) fn flight_path_marker(s: &flight::State, zoom: f64) -> Option<(f64, f64)> {
    if s.speed <= 10. {
        return None;
    }
    let gamma = s.velocity[1].atan2(s.velocity[0].hypot(s.velocity[2]));
    let bearing = s.velocity[0].atan2(s.velocity[2]) - s.yaw;
    hud::project(s.pitch, s.bank, bearing, gamma, zoom)
}

fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}

/// The pipper with its fall line (or dot trail, `trail`), for a bomb; a
/// rocket's pipper has no line (`line` false).
fn draw_ccip(
    paint: &mut hud::Paint<'_>,
    s: &flight::State,
    zoom: f64,
    reference: (f64, f64),
    c: &Ccip,
    trail: bool,
    line: bool,
) {
    let impact = projected(sub(c.impact, s.position), s, zoom);
    let cue = if line {
        pipper_cue(reference, impact)
    } else {
        Some(rocket_cue(reference, impact))
    };
    let Some(cue) = cue else {
        return;
    };
    paint.clip = (174, 96, 292, hud::AIM_BOTTOM - 96);
    if trail {
        for dot in c.trail() {
            let relative = sub(dot, s.position);
            if relative.iter().map(|v| v * v).sum::<f64>() < TRAIL_NEAREST_FT * TRAIL_NEAREST_FT {
                continue;
            }
            if let Some((x, y)) = projected(relative, s, zoom)
                && (LEFT..=RIGHT).contains(&x)
                && (TOP..=bottom()).contains(&y)
            {
                paint.rect(x.round() as i32, y.round() as i32, 1, 1);
            }
        }
    }
    match cue {
        PipperCue::On(point) => {
            if line && !trail {
                dotted(paint, reference, toward(reference, point, RING));
            }
            pipper(paint, point, c.arc_fraction());
        }
        PipperCue::Caret(point) => {
            if line && !trail {
                dotted(paint, reference, point);
            }
            caret(paint, point);
        }
        PipperCue::Edge(point, toward) => arrowhead(paint, point, toward),
    }
}

/// `to` pulled back toward `from` by `gap` pixels, so the line stops at the
/// ring.
fn toward(from: (f64, f64), to: (f64, f64), gap: f64) -> (f64, f64) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = dx.hypot(dy);
    if length <= gap {
        from
    } else {
        (to.0 - dx / length * gap, to.1 - dy / length * gap)
    }
}

/// Every other pixel: retail's line pattern 0x55555555 (rhythm fitted).
fn dotted(paint: &mut hud::Paint<'_>, a: (f64, f64), b: (f64, f64)) {
    let n = (b.0 - a.0)
        .abs()
        .max((b.1 - a.1).abs())
        .ceil()
        .clamp(1., 4096.) as i32;
    for i in (0..=n).step_by(2) {
        let t = f64::from(i) / f64::from(n);
        paint.rect(
            (a.0 + (b.0 - a.0) * t).round() as i32,
            (a.1 + (b.1 - a.1) * t).round() as i32,
            1,
            1,
        );
    }
}

/// The pipper glyph at `(x, y)` with `arc` (0 to 1) of its inner arc.
pub fn pipper(paint: &mut hud::Paint<'_>, (x, y): (f64, f64), arc: f64) {
    let point = |radius: f64, angle: f64| (x + radius * angle.sin(), y - radius * angle.cos());
    const SEGMENTS: usize = 72;
    let step = std::f64::consts::TAU / SEGMENTS as f64;
    for i in 0..SEGMENTS {
        let a = i as f64 * step;
        paint.line(point(RING, a), point(RING, a + step));
    }
    for mark in 0..12 {
        let a = f64::from(mark) * std::f64::consts::TAU / 12.;
        let outer = if mark % 3 == 0 {
            CARDINAL_MARK
        } else {
            MINOR_MARK
        };
        paint.line(point(RING + 1., a), point(outer, a));
    }
    // The inner arc from 12 o'clock, clockwise.
    let segments = (arc.clamp(0., 1.) * 48.).round() as usize;
    let step = std::f64::consts::TAU / 48.;
    for i in 0..segments {
        let a = i as f64 * step;
        paint.line(point(ARC, a), point(ARC, a + step));
    }
    paint.rect(x.round() as i32, y.round() as i32, 1, 1);
}

/// A caret pointing down at the HUD's lower edge.
fn caret(paint: &mut hud::Paint<'_>, (x, y): (f64, f64)) {
    paint.line((x - 5., y - 6.), (x, y));
    paint.line((x, y), (x + 5., y - 6.));
}

/// An arrowhead with its tip at `tip`, pointing along the unit `toward`: the
/// [`caret`] turned to any direction.
fn arrowhead(paint: &mut hud::Paint<'_>, (x, y): (f64, f64), (dx, dy): (f64, f64)) {
    let (px, py) = (-dy, dx);
    for side in [-1., 1.] {
        paint.line(
            (x - dx * 6. + px * 5. * side, y - dy * 6. + py * 5. * side),
            (x, y),
        );
    }
}

/// What the T needs to be drawn: the bomb sight's level release and the
/// guided bomb's sight share the drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Tee {
    /// The stem's turn from the flight path, radians, positive right.
    pub steering_rad: f64,
    /// The marker on the stem: 1 at the top, 0 on the crossbar.
    pub marker: f64,
    /// More than a minute out: the stem is dashed.
    pub far: bool,
    /// Passed: the crossbar only.
    pub overflown: bool,
}

impl Tee {
    pub(super) fn of(l: &LevelRelease) -> Self {
        Self {
            steering_rad: l.steering_rad,
            marker: l.marker_fraction(),
            far: l.far(),
            overflown: l.overflown(),
        }
    }
}

/// The level release "T": the azimuth steering stem, the crossbar across the
/// flight path marker and the release marker sliding down the stem.
pub(super) fn draw_level(
    paint: &mut hud::Paint<'_>,
    s: &flight::State,
    zoom: f64,
    fpm: Option<(f64, f64)>,
    l: &Tee,
) {
    let Some((fx, fy)) = fpm else {
        return;
    };
    let bar = fy - CROSSBAR_RISE;
    paint.line((fx - CROSSBAR_HALF, bar), (fx + CROSSBAR_HALF, bar));
    if l.overflown {
        return;
    }
    // The stem stands at the track the impact must follow: the flight path's
    // track turned by the steering correction.
    let track = s.velocity[0].atan2(s.velocity[2]);
    let gamma = s.velocity[1].atan2(s.velocity[0].hypot(s.velocity[2]));
    let Some((sx, _)) = hud::project(s.pitch, s.bank, track + l.steering_rad - s.yaw, gamma, zoom)
    else {
        return;
    };
    let (x, beyond) = if sx < LEFT {
        (LEFT, Some(-1.))
    } else if sx > RIGHT {
        (RIGHT, Some(1.))
    } else {
        (sx, None)
    };
    let top = STEM_TOP.min(bar - 20.);
    if l.far {
        let mut y = top;
        while y < bar {
            paint.line((x, y), (x, (y + 4.).min(bar)));
            y += 8.;
        }
    } else {
        paint.line((x, top), (x, bar));
    }
    if let Some(side) = beyond {
        // The stem is off the HUD: an arrowhead says which way to turn.
        let y = (top + bar) / 2.;
        paint.line((x - side * 6., y - 5.), (x, y));
        paint.line((x, y), (x - side * 6., y + 5.));
    }
    let marker = bar - (bar - top) * l.marker;
    for i in 0..24 {
        let a = f64::from(i) * std::f64::consts::TAU / 24.;
        let b = f64::from(i + 1) * std::f64::consts::TAU / 24.;
        paint.line(
            (x + MARKER * a.cos(), marker + MARKER * a.sin()),
            (x + MARKER * b.cos(), marker + MARKER * b.sin()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(draw: impl FnOnce(&mut hud::Paint<'_>)) -> std::collections::BTreeSet<(i32, i32)> {
        let mut pixels = vec![0; 640 * 480 * 4];
        draw(&mut hud::Paint {
            pixels: &mut pixels,
            clip: hud::HUD_CLIP,
            color: [0, 255, 0, 255],
        });
        pixels
            .chunks_exact(4)
            .enumerate()
            .filter(|(_, px)| px[3] != 0)
            .map(|(i, _)| ((i % 640) as i32, (i / 640) as i32))
            .collect()
    }

    #[test]
    fn the_pipper_is_the_retail_glyphs_ring_marks_arc_and_dot() {
        let bare = lit(|p| pipper(p, (320., 240.), 0.));
        // The centre dot, the 23-pixel ring and the cardinal marks to the
        // 29-pixel cell edge.
        assert!(bare.contains(&(320, 240)));
        for (x, y) in [(320, 229), (331, 240), (320, 251), (309, 240)] {
            assert!(bare.contains(&(x, y)), "ring at ({x}, {y})");
        }
        for (x, y) in [(320, 226), (334, 240), (320, 254), (306, 240)] {
            assert!(bare.contains(&(x, y)), "cardinal mark at ({x}, {y})");
        }
        // Nothing of the inner arc.
        assert!(!bare.contains(&(320, 232)));
        let x0 = bare.iter().map(|p| p.0).min().unwrap();
        let x1 = bare.iter().map(|p| p.0).max().unwrap();
        assert_eq!(x1 - x0 + 1, 29);
        // A quarter of the arc: from 12 o'clock to 3 o'clock, clockwise.
        let quarter = lit(|p| pipper(p, (320., 240.), 0.25));
        assert!(quarter.contains(&(320, 232)));
        assert!(quarter.contains(&(328, 240)));
        assert!(!quarter.contains(&(320, 248)));
        assert!(!quarter.contains(&(312, 240)));
        // The whole arc closes it.
        let full = lit(|p| pipper(p, (320., 240.), 1.));
        for (x, y) in [(320, 232), (328, 240), (320, 248), (312, 240)] {
            assert!(full.contains(&(x, y)), "arc at ({x}, {y})");
        }
        assert!(bare.is_subset(&quarter) && quarter.is_subset(&full));
    }

    #[test]
    fn an_impact_below_the_hud_gets_the_caret_on_the_lower_edge() {
        let reference = (320., 240.);
        assert_eq!(
            pipper_cue(reference, Some((330., 300.))),
            Some(PipperCue::On((330., 300.)))
        );
        // Below the edge: the caret where the line crosses it.
        let Some(PipperCue::Caret((x, y))) = pipper_cue(reference, Some((380., 540.))) else {
            panic!("no caret");
        };
        assert_eq!(y, bottom());
        assert!((x - (320. + 60. * (bottom() - 240.) / 300.)).abs() < 1e-9);
        // Far to the side and below: clamped to the HUD.
        let Some(PipperCue::Caret((x, _))) = pipper_cue(reference, Some((2_000., 900.))) else {
            panic!("no caret");
        };
        assert_eq!(x, RIGHT);
        // Projecting nowhere: the caret under the reference.
        assert_eq!(
            pipper_cue((300., 250.), None),
            Some(PipperCue::Caret((300., bottom())))
        );
        // Above the HUD (a climb): nothing.
        assert_eq!(pipper_cue(reference, Some((320., 50.))), None);
    }

    #[test]
    fn the_hud_modules_choose_the_trail_and_the_lines_start() {
        let f18 = SightStyle::of(AircraftId::F18);
        assert_eq!(
            f18,
            SightStyle {
                trail: false,
                flight_path: true,
                minimum_fps: 0.
            }
        );
        for id in [
            AircraftId::F16C,
            AircraftId::F22,
            AircraftId::F22n,
            AircraftId::X31,
        ] {
            assert!(SightStyle::of(id).trail, "{id:?}");
        }
        assert!(!SightStyle::of(AircraftId::X31).flight_path);
        for id in [
            AircraftId::A4E,
            AircraftId::F4B,
            AircraftId::Mig21,
            AircraftId::Av8,
        ] {
            assert_eq!(SightStyle::of(id).minimum_fps, 146., "{id:?}");
        }
    }

    #[test]
    fn times_read_minutes_and_seconds() {
        assert_eq!(clock(7.4), "0:07");
        assert_eq!(clock(75.), "1:15");
        assert_eq!(clock(-3.), "0:00");
        assert_eq!(clock(f64::INFINITY), "-:--");
    }
    #[test]
    fn the_rocket_pipper_is_always_shown() {
        let centre = (320., 240.);
        // On the field: the pipper.
        assert_eq!(
            rocket_cue(centre, Some((330., 250.))),
            PipperCue::On((330., 250.))
        );
        // Below, above and beside the field: an arrowhead pinned at the edge
        // nearest it, pointing at the impact.
        let PipperCue::Edge(at, toward) = rocket_cue(centre, Some((330., 900.))) else {
            panic!("below the field");
        };
        assert_eq!(at, (330., bottom()));
        assert!((toward.0).abs() < 1e-9 && (toward.1 - 1.).abs() < 1e-9);
        let PipperCue::Edge(at, toward) = rocket_cue(centre, Some((330., -400.))) else {
            panic!("above the field");
        };
        assert_eq!(at, (330., TOP));
        assert!((toward.1 + 1.).abs() < 1e-9);
        let PipperCue::Edge(at, toward) = rocket_cue(centre, Some((1_000., 240.))) else {
            panic!("beside the field");
        };
        assert_eq!(at, (RIGHT, 240.));
        assert!((toward.0 - 1.).abs() < 1e-9);
        // A corner points diagonally.
        let PipperCue::Edge(at, toward) = rocket_cue(centre, Some((RIGHT + 50., bottom() + 50.)))
        else {
            panic!("corner");
        };
        assert_eq!(at, (RIGHT, bottom()));
        assert!((toward.0 - toward.1).abs() < 1e-9 && toward.0 > 0.);
        // Behind the view: the caret under the flight path.
        assert_eq!(rocket_cue(centre, None), PipperCue::Caret((320., bottom())));
    }

    #[test]
    fn an_edge_arrowhead_is_the_caret_turned() {
        let caret_pixels = lit(|p| caret(p, (320., 300.)));
        let arrow_pixels = lit(|p| arrowhead(p, (320., 300.), (0., 1.)));
        assert_eq!(caret_pixels, arrow_pixels);
        let right = lit(|p| arrowhead(p, (456., 240.), (1., 0.)));
        assert!(right.contains(&(456, 240)) && right.contains(&(450, 235)));
    }
}
