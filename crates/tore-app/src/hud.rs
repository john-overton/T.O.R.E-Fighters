//! Flight display projection. Source fonts; authored layout and symbology.
use crate::{flight::State, menu::Canvas};
use tore_formats::font::Font;

pub struct Paint<'a> {
    pub pixels: &'a mut [u8],
    pub clip: (i32, i32, i32, i32),
    pub color: [u8; 4],
}
impl Paint<'_> {
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let (cx, cy, cw, ch) = self.clip;
        let (left, top) = (x.max(cx), y.max(cy));
        Canvas(self.pixels).rect(
            (
                left,
                top,
                (x + w).min(cx + cw) - left,
                (y + h).min(cy + ch) - top,
            ),
            self.color,
        );
    }
    pub fn line(&mut self, a: (f64, f64), b: (f64, f64)) {
        // Bound work even if a projected endpoint is very far offscreen.
        let n = (b.0 - a.0)
            .abs()
            .max((b.1 - a.1).abs())
            .ceil()
            .clamp(1., 4096.) as i32;
        for i in 0..=n {
            self.rect(
                (a.0 + (b.0 - a.0) * i as f64 / n as f64).round() as i32,
                (a.1 + (b.1 - a.1) * i as f64 / n as f64).round() as i32,
                1,
                1,
            );
        }
    }
    fn readout_box(&mut self, font: &Font, text: &str, x: i32, y: i32) {
        let width = text
            .bytes()
            .map(|c| font.glyphs[c as usize].advance)
            .sum::<usize>() as i32;
        let (left, top, right, bottom) = (
            x - 4,
            y - 3,
            x + width.max(24) + 3,
            y + font.height as i32 + 2,
        );
        // Outline only: retain the world behind the original HUD glyphs.
        self.line((left as f64, top as f64), (right as f64, top as f64));
        self.line((right as f64, top as f64), (right as f64, bottom as f64));
        self.line((right as f64, bottom as f64), (left as f64, bottom as f64));
        self.line((left as f64, bottom as f64), (left as f64, top as f64));
        self.text(font, text, x, y);
    }
    pub fn text(&mut self, font: &Font, text: &str, mut x: i32, y: i32) {
        for ch in text.bytes() {
            let g = &font.glyphs[ch as usize];
            for &(xx, yy) in &g.pixels {
                self.rect(x + xx as i32, y + yy as i32, 1, 1);
            }
            x += g.advance as i32;
        }
    }
}
// Authored F-16-style bank scale requested by the user. The graduated arc
// rotates past a fixed index, keeping full rolls readable through +/-180.
pub const HUD_CLIP: (i32, i32, i32, i32) = (174, 96, 292, 354);
pub const AIM_BOTTOM: i32 = 390;
const BANK_CENTER_Y: f64 = 135.;
const BANK_RADIUS: f64 = 223.;
fn bank_point(angle: f64, radius: f64) -> (f64, f64) {
    let a = angle.to_radians();
    (320. + radius * a.sin(), BANK_CENTER_Y + radius * a.cos())
}
fn bank_scale(p: &mut Paint<'_>, font: &Font, bank: f64) {
    for mark in (-180i32..180).step_by(10) {
        let angle = bank_tick_angle(mark, bank);
        if angle.abs() > 30. {
            continue;
        }
        let major = mark % 30 == 0;
        p.line(
            bank_point(angle, if major { 216. } else { 219. }),
            bank_point(angle, BANK_RADIUS),
        );
        if major {
            let text = mark.abs().to_string();
            let width = text
                .bytes()
                .map(|c| font.glyphs[c as usize].advance)
                .sum::<usize>() as i32;
            let (x, y) = bank_point(angle, 239.);
            p.text(
                font,
                &text,
                x.round() as i32 - width / 2,
                y.round() as i32 - font.height as i32 / 2,
            );
        }
    }
    let tip_y = BANK_CENTER_Y + BANK_RADIUS + 1.;
    let base_y = tip_y + 7.;
    p.line((320., tip_y), (316., base_y));
    p.line((316., base_y), (324., base_y));
    p.line((324., base_y), (320., tip_y));
}
fn bank_tick_angle(mark: i32, bank: f64) -> f64 {
    (f64::from(mark) - bank.to_degrees() + 180.).rem_euclid(360.) - 180.
}
pub fn heading(yaw: f64) -> f64 {
    yaw.to_degrees().rem_euclid(360.)
}
/// A world direction projected through the same 60-degree camera as the GPU.
/// Camera roll is -bank. Screen Y points down.
pub fn project(
    pitch: f64,
    bank: f64,
    bearing: f64,
    elevation: f64,
    zoom: f64,
) -> Option<(f64, f64)> {
    let (sb, cb) = bearing.sin_cos();
    let (se, ce) = elevation.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let (sr, cr) = bank.sin_cos();
    let x = sb * ce;
    let y = se * cp - ce * cb * sp;
    let z = ce * cb * cp + se * sp;
    if z < 0.05 {
        return None;
    }
    let scale = 240. * 3f64.sqrt() * zoom / z;
    Some((
        320. + (x * cr - y * sr) * scale,
        240. - (x * sr + y * cr) * scale,
    ))
}
fn ladder_project(
    pitch: f64,
    bank: f64,
    bearing: f64,
    elevation: f64,
    zoom: f64,
) -> Option<(f64, f64)> {
    // A compact attitude ruler: compress the relative angle and its motion
    // about the forward point together. Horizon anchoring makes labels lead
    // the aircraft, with a rapidly growing error as pitch approaches vertical.
    // Local bearing keeps rung width finite at +/-90 degrees.
    let point = project(0., bank, bearing, elevation - pitch, zoom)?;
    let normal = [bank.sin(), bank.cos()];
    let delta = [point.0 - 320., point.1 - 240.];
    let distance = delta[0] * normal[0] + delta[1] * normal[1];
    Some((
        point.0 - normal[0] * distance * 0.25,
        point.1 - normal[1] * distance * 0.25,
    ))
}
/// The zero bar is the true horizon; numbered marks form the compact ruler.
fn rung_project(
    pitch: f64,
    bank: f64,
    bearing: f64,
    elevation: f64,
    zoom: f64,
) -> Option<(f64, f64)> {
    if elevation == 0. {
        project(pitch, bank, bearing, 0., zoom)
    } else {
        ladder_project(pitch, bank, bearing, elevation, zoom)
    }
}
#[allow(clippy::too_many_arguments)]
pub fn draw(
    pixels: &mut [u8],
    s: &State,
    font: &Font,
    ground: f64,
    air: Option<&tore_sim::telemetry::AirData>,
    ladder: bool,
    weapons: bool,
    color: [u8; 3],
    zoom: f32,
    ils: Option<(&tore_sim::airport::Guidance, &str, &str)>,
    wind: Option<&tore_sim::runway_wind::Assessment>,
) {
    let mut p = Paint {
        pixels,
        clip: HUD_CLIP,
        color: [color[0], color[1], color[2], 255],
    };
    if s.systems.has(31) {
        p.text(font, "FLIGHT DATA FAILED", 264, 225);
        return;
    }
    let hdg = heading(s.yaw);
    // Heading strip and pointer, wrapped across north.
    for i in -4..=4 {
        let tick = (hdg / 10.).floor() * 10. + i as f64 * 10.;
        let x = 320. + (tick - hdg) * 2.;
        if (260. ..380.).contains(&x) {
            p.line((x, 146.), (x, 151.));
            p.text(
                font,
                &format!("{:02.0}", tick.rem_euclid(360.) / 10.),
                x as i32 - 5,
                133,
            );
        }
    }
    p.line((315., 153.), (320., 158.));
    p.line((320., 158.), (325., 153.));
    p.text(font, &format!("{:03}", hdg.round() as u32 % 360), 311, 162);
    // Ladder is perspective projected and clipped away from the fixed tapes.
    if ladder {
        p.clip = (250, 182, 140, 166);
        for degrees in (-90..=90).step_by(5) {
            let el = (degrees as f64).to_radians();
            for side in [-1., 1.] {
                let spans: &[(f64, f64)] = if degrees < 0 {
                    &[(0.7, 1.2), (1.6, 2.1), (2.5, 3.)]
                } else {
                    &[(0.7, 3.)]
                };
                for &(a, b) in spans {
                    if let (Some(a), Some(b)) = (
                        rung_project(s.pitch, s.bank, (side * a).to_radians(), el, zoom as f64),
                        rung_project(s.pitch, s.bank, (side * b).to_radians(), el, zoom as f64),
                    ) {
                        p.line(a, b);
                    }
                }
            }
            if degrees != 0
                && let Some((x, y)) =
                    rung_project(s.pitch, s.bank, 3.5f64.to_radians(), el, zoom as f64)
            {
                p.text(font, &degrees.to_string(), x as i32, y as i32 - 4);
            }
        }
    }
    p.clip = HUD_CLIP;
    // Kinematic flight path marker; no fixed aircraft-datum bars.
    if s.speed > 10. {
        let gamma = s.velocity[1].atan2(s.velocity[0].hypot(s.velocity[2]));
        let bearing = s.velocity[0].atan2(s.velocity[2]) - s.yaw;
        if let Some((x, y)) = project(s.pitch, s.bank, bearing, gamma, zoom as f64)
            && (235. ..405.).contains(&x)
            && (155. ..380.).contains(&y)
        {
            for i in 0..36 {
                let t = i as f64 * std::f64::consts::TAU / 36.;
                p.rect((x + 4. * t.cos()) as i32, (y + 4. * t.sin()) as i32, 1, 1);
            }
            p.line((x - 11., y), (x - 4., y));
            p.line((x + 4., y), (x + 11., y));
            p.line((x, y - 8.), (x, y - 4.));
        }
    }
    let speed = air.map_or(s.speed / 1.68781, |d| d.true_airspeed_knots);
    p.text(font, "TAS", 211, 190);
    p.text(font, "MSL", 402, 190);
    p.readout_box(font, &format!("{speed:.0}"), 211, 235);
    p.readout_box(font, &format!("{:.0}", s.position[1]), 405, 235);
    p.text(font, &format!("{:.1}G", s.g), 235, 164);
    p.text(font, &format!("{:.0}%", s.throttle * 100.), 235, 178);
    if s.afterburner_active() {
        p.text(font, "AFT", 235, 150);
    }
    for (i, (label, value)) in [
        ("GEAR", s.gear),
        ("FLAP", s.flaps),
        ("BRAKE", s.brake),
        ("HOOK", s.hook),
    ]
    .iter()
    .enumerate()
    {
        if *value > 0.01 {
            p.text(font, label, 388, 140 + i as i32 * 11);
        }
    }
    if !weapons && ils.is_some_and(|(guidance, _, _)| guidance.active) {
        p.text(
            font,
            &format!(
                "AGL {:.0}",
                air.map_or(s.position[1] - ground, |d| d.altitude_agl_ft)
                    .max(0.)
            ),
            402,
            259,
        );
        p.text(
            font,
            &format!(
                "V/S {:+.0}",
                air.map_or(s.vertical_speed * 60., |d| d.vertical_speed_fpm)
            ),
            207,
            271,
        );
    }
    if s.autopilot.mode() != tore_sim::autopilot::Mode::Off {
        p.text(font, "AUTO", 211, 133);
        p.text(font, &s.autopilot.label(), 211, 145);
    }
    if let Some(wind) = wind {
        p.text(
            font,
            &wind_label(wind),
            244,
            if ils.is_some() { 96 } else { 106 },
        );
    }
    if let Some((guidance, airport, runway)) = ils {
        let airport: String = airport
            .chars()
            .filter(|c| c.is_ascii_graphic() || *c == ' ')
            .take(24)
            .collect();
        let runway: String = runway
            .chars()
            .filter(|c| c.is_ascii_graphic() || *c == ' ')
            .take(16)
            .collect();
        p.text(font, &format!("ILS {airport}"), 252, 106);
        p.text(
            font,
            &format!("RWY {runway} {:.1}NM", guidance.range_ft / 6_076.12),
            252,
            118,
        );
        if guidance.active {
            let vertical_x = 320. - guidance.localizer_normalized * 42.;
            let horizontal_y = 240. + guidance.glide_normalized * 30.;
            p.line((vertical_x, 207.), (vertical_x, 273.));
            p.line((278., horizontal_y), (362., horizontal_y));
            p.rect(318, 238, 5, 5);
        } else {
            p.text(font, "ILS ARM", 300, 258);
        }
    }
    if s.stall_alert(ground).is_some() {
        p.text(font, "STALL", 301, 274);
    }
    if s.crashed {
        p.text(font, "CRASHED - ESC", 277, 306);
    } else if !s.engine {
        p.text(font, "ENGINE OFF", 283, 306);
    } else if !weapons {
        p.clip = HUD_CLIP;
        bank_scale(&mut p, font, s.bank);
    }
}
fn wind_label(wind: &tore_sim::runway_wind::Assessment) -> String {
    let tail = if wind.tailwind_knots >= 0.5 {
        format!(
            " TW {:.0}/10{}",
            wind.tailwind_knots,
            if wind.tailwind_knots >= 10. {
                " LIMIT"
            } else {
                ""
            }
        )
    } else {
        String::new()
    };
    let cross = if wind.crosswind_knots.abs() < 0.5 {
        0.
    } else {
        wind.crosswind_knots
    };
    format!(
        "XW {cross:+.0}/{:.0} {}{tail} KT",
        wind.limit_knots,
        wind.severity.label()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wind_readout_distinguishes_crosswind_tailwind_and_ignored_headwind() {
        use tore_sim::runway_wind::{FEET_PER_SECOND_PER_KNOT as K, assessment};
        assert_eq!(
            wind_label(&assessment(40_000., [31. * K, 0., 0.], 0.).unwrap()),
            "XW +31/30 LIMIT KT"
        );
        assert_eq!(
            wind_label(&assessment(40_000., [0., 0., 10. * K], 0.).unwrap()),
            "XW +0/30 CALM TW 10/10 LIMIT KT"
        );
        assert_eq!(
            wind_label(&assessment(40_000., [0., 0., -40. * K], 0.).unwrap()),
            "XW +0/30 CALM KT"
        );
    }

    #[test]
    fn bank_scale_tracks_aircraft_attitude_through_full_rolls() {
        assert_eq!(bank_tick_angle(30, 30f64.to_radians()), 0.);
        assert_eq!(bank_tick_angle(-30, (-30f64).to_radians()), 0.);
        assert_eq!(bank_tick_angle(0, 360f64.to_radians()), 0.);
        assert!((bank_tick_angle(-180, 179f64.to_radians()) - 1.).abs() < 1e-9);
        assert!((bank_tick_angle(-180, 181f64.to_radians()) + 1.).abs() < 1e-9);
        let left = bank_point(-30., BANK_RADIUS);
        let center = bank_point(0., BANK_RADIUS);
        let right = bank_point(30., BANK_RADIUS);
        assert!((right.0 - left.0 - 223.).abs() < 1e-9);
        assert!((center.1 - left.1 - 223. * (1. - 30f64.to_radians().cos())).abs() < 1e-9);
        assert_eq!(center.1, 358.);
    }
    #[test]
    fn heading_wrap_and_horizon_projection() {
        assert!((heading(-0.1) - 354.270422).abs() < 0.00001);
        assert_eq!(project(0., 0., 0., 0., 1.), Some((320., 240.)));
        assert!(project(0.2, 0., 0., 0., 1.).unwrap().1 > 240.);
        assert!(project(0., 0., std::f64::consts::PI, 0., 1.).is_none());
        let a = project(0., 0., 0.1, 0., 1.).unwrap();
        let b = project(0., std::f64::consts::FRAC_PI_2, 0.1, 0., 1.).unwrap();
        assert!(a.0 > 320. && (b.0 - 320.).abs() < 0.001 && b.1 < 240.);
    }
    #[test]
    fn ladder_compresses_only_bank_normal_spacing() {
        for bank in [0., 45f64.to_radians(), 90f64.to_radians()] {
            let normal = [bank.sin(), bank.cos()];
            let tangent = [bank.cos(), -bank.sin()];
            let base = project(0., bank, 0., 0., 1.).unwrap();
            let raised = project(0., bank, 0., 5f64.to_radians(), 1.).unwrap();
            let compact_base = ladder_project(0., bank, 0., 0., 1.).unwrap();
            let compact_raised = ladder_project(0., bank, 0., 5f64.to_radians(), 1.).unwrap();
            let spacing = (raised.0 - base.0) * normal[0] + (raised.1 - base.1) * normal[1];
            let compact_spacing = (compact_raised.0 - compact_base.0) * normal[0]
                + (compact_raised.1 - compact_base.1) * normal[1];
            assert!((compact_spacing - spacing * 0.75).abs() < 1e-9);

            let left = project(0., bank, -3f64.to_radians(), 5f64.to_radians(), 1.).unwrap();
            let right = project(0., bank, 3f64.to_radians(), 5f64.to_radians(), 1.).unwrap();
            let compact_left =
                ladder_project(0., bank, -3f64.to_radians(), 5f64.to_radians(), 1.).unwrap();
            let compact_right =
                ladder_project(0., bank, 3f64.to_radians(), 5f64.to_radians(), 1.).unwrap();
            let width = (right.0 - left.0) * tangent[0] + (right.1 - left.1) * tangent[1];
            let compact_width = (compact_right.0 - compact_left.0) * tangent[0]
                + (compact_right.1 - compact_left.1) * tangent[1];
            assert!((compact_width - width).abs() < 1e-9);
            assert!(compact_left.0.is_finite() && compact_left.1.is_finite());
        }
        assert!(ladder_project(0., 0., std::f64::consts::PI, 0., 1.).is_none());
    }
    #[test]
    fn true_horizon_bar_meets_level_velocity_without_moving_numbered_marks() {
        for pitch in [-60_f64, -10., -4., 0., 4., 10., 60.].map(f64::to_radians) {
            for bank in [-180_f64, -90., -45., 0., 45., 90., 180.].map(f64::to_radians) {
                for zoom in [0.5, 1., 4.] {
                    let normal = [bank.sin(), bank.cos()];
                    let velocity = project(pitch, bank, 0., 0., zoom).unwrap();
                    for bearing in [-3_f64, -0.7, 0.7, 3.].map(f64::to_radians) {
                        let bar = rung_project(pitch, bank, bearing, 0., zoom).unwrap();
                        assert_eq!(Some(bar), project(pitch, bank, bearing, 0., zoom));
                        assert!(
                            ((bar.0 - velocity.0) * normal[0] + (bar.1 - velocity.1) * normal[1])
                                .abs()
                                < 1e-9
                        );
                        for degrees in [-90_f64, -85., -5., 5., 70., 85., 90.] {
                            let elevation = degrees.to_radians();
                            assert_eq!(
                                rung_project(pitch, bank, bearing, elevation, zoom),
                                ladder_project(pitch, bank, bearing, elevation, zoom)
                            );
                        }
                    }
                }
            }
        }
        // At nose-up level flight, the true bar must not leave a second compact zero.
        let pitch = 4f64.to_radians();
        assert!(
            (rung_project(pitch, 0., 0., 0., 1.).unwrap().1
                - ladder_project(pitch, 0., 0., 0., 1.).unwrap().1)
                > 7.
        );
    }

    #[test]
    fn compact_ladder_reads_actual_pitch_and_has_constant_motion() {
        for pitch_deg in (-90..=90).step_by(5) {
            let pitch = (pitch_deg as f64).to_radians();
            for bank in [-180_f64, -90., -45., 0., 45., 90., 180.].map(f64::to_radians) {
                for zoom in [0.5, 1., 4.] {
                    let normal = [bank.sin(), bank.cos()];
                    let center = ladder_project(pitch, bank, 0., pitch, zoom).unwrap();
                    assert!((center.0 - 320.).abs() < 1e-9 && (center.1 - 240.).abs() < 1e-9);
                    // A five-degree error has the same displacement at 0,70,85,90
                    // and while inverted. Pitch motion cannot run ahead of labels.
                    let next =
                        ladder_project(pitch, bank, 0., pitch + 5f64.to_radians(), zoom).unwrap();
                    let moved =
                        ladder_project(pitch + 5f64.to_radians(), bank, 0., pitch, zoom).unwrap();
                    let offset =
                        |(x, y): (f64, f64)| (x - 320.) * normal[0] + (y - 240.) * normal[1];
                    assert!((offset(next) + 27.27626585637572 * zoom).abs() < 1e-8);
                    assert!((offset(next) + offset(moved)).abs() < 1e-8);
                    let left =
                        ladder_project(pitch, bank, -3f64.to_radians(), pitch, zoom).unwrap();
                    let right =
                        ladder_project(pitch, bank, 3f64.to_radians(), pitch, zoom).unwrap();
                    let level_left =
                        ladder_project(0., bank, -3f64.to_radians(), 0., zoom).unwrap();
                    let level_right =
                        ladder_project(0., bank, 3f64.to_radians(), 0., zoom).unwrap();
                    assert!(((right.0 - left.0) - (level_right.0 - level_left.0)).abs() < 1e-9);
                    assert!(((right.1 - left.1) - (level_right.1 - level_left.1)).abs() < 1e-9);
                }
            }
        }
        let premature = ladder_project(70f64.to_radians(), 0., 0., 85f64.to_radians(), 1.).unwrap();
        assert!(premature.1 < 160.); // 85 is still fifteen degrees above the nose.
    }

    #[test]
    fn compact_ladder_has_visible_rungs_and_continuous_poles_through_a_loop() {
        use crate::attitude::Basis;
        let mut basis = Basis::new(0., 0., 0.);
        let mut previous_pole: Option<(f64, f64)> = None;
        for _ in 0..1440 {
            basis = basis.rotated(basis.right.map(|v| -v * std::f64::consts::TAU / 1440.));
            let [_, pitch, bank] = basis.angles();
            let visible = (-90..=90).step_by(5).any(|degree| {
                let elevation = (degree as f64).to_radians();
                [-3_f64, 3.].iter().all(|side| {
                    rung_project(pitch, bank, side.to_radians(), elevation, 1.).is_some_and(
                        |(x, y)| (250. ..390.).contains(&x) && (182. ..348.).contains(&y),
                    )
                })
            });
            assert!(
                visible,
                "empty ladder at pitch {} bank {}",
                pitch.to_degrees(),
                bank.to_degrees()
            );
            if pitch.abs() > 80f64.to_radians() {
                let pole = rung_project(
                    pitch,
                    bank,
                    0.,
                    pitch.signum() * std::f64::consts::FRAC_PI_2,
                    1.,
                )
                .unwrap();
                if let Some(previous) = previous_pole {
                    assert!((pole.0 - previous.0).hypot(pole.1 - previous.1) < 2.);
                }
                previous_pole = Some(pole);
            } else {
                previous_pole = None;
            }
        }
    }

    #[test]
    fn rendered_ladder_marks_the_actual_pitch_at_the_forward_point() {
        let mut state =
            State::new(&crate::flight::animation_tests::profile(), [0., 5000., 0.]).unwrap();
        let font = Font {
            height: 8,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 6,
                    pixels: vec![],
                })
                .collect(),
        };
        for degrees in (-90..=90).step_by(5) {
            state.pitch = (degrees as f64).to_radians();
            let mut with = vec![0; 640 * 480 * 4];
            let mut without = with.clone();
            draw(
                &mut with,
                &state,
                &font,
                0.,
                None,
                true,
                false,
                [0, 255, 0],
                1.,
                None,
                None,
            );
            draw(
                &mut without,
                &state,
                &font,
                0.,
                None,
                false,
                false,
                [0, 255, 0],
                1.,
                None,
                None,
            );
            let visible = (297..316).any(|x| {
                let at = (240 * 640 + x) * 4;
                with[at..at + 4] != without[at..at + 4]
            });
            assert!(
                visible,
                "actual {degrees}-degree rung missing at the forward point"
            );
        }
    }

    #[test]
    fn rendered_horizon_replaces_the_compact_zero_bar() {
        let mut state =
            State::new(&crate::flight::animation_tests::profile(), [0., 5000., 0.]).unwrap();
        state.pitch = 4f64.to_radians();
        let font = Font {
            height: 8,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 6,
                    pixels: vec![],
                })
                .collect(),
        };
        let mut with = vec![0; 640 * 480 * 4];
        let mut without = with.clone();
        draw(
            &mut with,
            &state,
            &font,
            0.,
            None,
            true,
            false,
            [0, 255, 0],
            1.,
            None,
            None,
        );
        draw(
            &mut without,
            &state,
            &font,
            0.,
            None,
            false,
            false,
            [0, 255, 0],
            1.,
            None,
            None,
        );
        let horizon = project(state.pitch, 0., 0., 0., 1.).unwrap().1.round() as usize;
        let compact = ladder_project(state.pitch, 0., 0., 0., 1.)
            .unwrap()
            .1
            .round() as usize;
        let difference = |row: usize| {
            (298..306).any(|x| {
                let at = (row * 640 + x) * 4;
                with[at..at + 4] != without[at..at + 4]
            })
        };
        assert!(difference(horizon));
        assert!(!difference(compact));
    }

    #[test]
    fn velocity_marker_matches_body_axes_through_banked_pulls() {
        use crate::attitude::{Basis, dot};
        for bank in [-2., -0.8, 0., 0.8, 2.] {
            let body = Basis::new(0.3, 0.4, bank);
            // Synthetic nose-up AoA plus side-slip, rotated with the aircraft.
            let velocity = std::array::from_fn(|i| {
                body.forward[i] * 700. - body.up[i] * 40. + body.right[i] * 15.
            });
            let gamma = velocity[1].atan2(velocity[0].hypot(velocity[2]));
            let bearing = velocity[0].atan2(velocity[2]) - 0.3;
            let (x, y) = project(0.4, bank, bearing, gamma, 1.).unwrap();
            let focal = 240. * 3f64.sqrt();
            let z = dot(velocity, body.forward);
            assert!((x - (320. + focal * dot(velocity, body.right) / z)).abs() < 1e-9);
            assert!((y - (240. - focal * dot(velocity, body.up) / z)).abs() < 1e-9);
        }
    }
}
