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
fn bank_scale(p: &mut Paint<'_>, font: &Font, bank: f64) {
    let point = |angle: f64, radius: f64| {
        let a = angle.to_radians();
        (320. + radius * a.sin(), 285. + radius * a.cos())
    };
    for mark in (-180i32..180).step_by(10) {
        let angle = bank_tick_angle(mark, bank);
        if angle.abs() > 60. {
            continue;
        }
        let major = mark % 30 == 0;
        p.line(
            point(angle, if major { 35. } else { 38. }),
            point(angle, 42.),
        );
        if major {
            let text = mark.abs().to_string();
            let width = text
                .bytes()
                .map(|c| font.glyphs[c as usize].advance)
                .sum::<usize>() as i32;
            let (x, y) = point(angle, 58.);
            p.text(
                font,
                &text,
                x.round() as i32 - width / 2,
                y.round() as i32 - font.height as i32 / 2,
            );
        }
    }
    p.line((320., 329.), (316., 336.));
    p.line((316., 336.), (324., 336.));
    p.line((324., 336.), (320., 329.));
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
pub fn draw(
    pixels: &mut [u8],
    s: &State,
    font: &Font,
    ground: f64,
    ladder: bool,
    color: [u8; 3],
    zoom: f32,
) {
    let mut p = Paint {
        pixels,
        clip: (174, 96, 292, 222),
        color: [color[0], color[1], color[2], 255],
    };
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
        p.clip = (250, 182, 140, 104);
        for degrees in (-85..=85).step_by(5) {
            let el = (degrees as f64).to_radians();
            for side in [-1., 1.] {
                let spans: &[(f64, f64)] = if degrees < 0 {
                    &[(0.7, 1.2), (1.6, 2.1), (2.5, 3.)]
                } else {
                    &[(0.7, 3.)]
                };
                for &(a, b) in spans {
                    if let (Some(a), Some(b)) = (
                        project(s.pitch, s.bank, (side * a).to_radians(), el, zoom as f64),
                        project(s.pitch, s.bank, (side * b).to_radians(), el, zoom as f64),
                    ) {
                        p.line(a, b);
                    }
                }
            }
            if degrees != 0
                && let Some((x, y)) = project(s.pitch, s.bank, 3.5f64.to_radians(), el, zoom as f64)
            {
                p.text(font, &degrees.to_string(), x as i32, y as i32 - 4);
            }
        }
    }
    p.clip = (174, 96, 292, 222);
    // Fixed aircraft datum and kinematic flight path; no synthetic target cues.
    p.line((303., 240.), (313., 240.));
    p.line((327., 240.), (337., 240.));
    p.line((313., 240.), (320., 243.));
    p.line((320., 243.), (327., 240.));
    if s.speed > 10. {
        let gamma = s.velocity[1].atan2(s.velocity[0].hypot(s.velocity[2]));
        let bearing = s.velocity[0].atan2(s.velocity[2]) - s.yaw;
        if let Some((x, y)) = project(s.pitch, s.bank, bearing, gamma, zoom as f64)
            && (235. ..405.).contains(&x)
            && (155. ..285.).contains(&y)
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
    let speed = s.speed / 1.68781;
    for i in -3..=3 {
        let v = (speed / 10.).floor() * 10. + i as f64 * 10.;
        let y = 228. - (v - speed) * 2.;
        if (205. ..282.).contains(&y) {
            p.line((240., y), (246., y));
            if i % 2 == 0 && (y - 228.).abs() > 12. {
                p.text(font, &format!("{v:.0}"), 211, y as i32 - 4);
            }
        }
        let a = (s.position[1] / 100.).floor() * 100. + i as f64 * 100.;
        let y = 228. - (a - s.position[1]) * 0.2;
        if (205. ..282.).contains(&y) {
            p.line((393., y), (399., y));
            if i % 2 == 0 && (y - 228.).abs() > 12. {
                p.text(font, &format!("{a:.0}"), 402, y as i32 - 4);
            }
        }
    }
    p.text(font, "TAS", 211, 190);
    p.text(font, "MSL", 402, 190);
    p.readout_box(font, &format!("{speed:.0}"), 211, 223);
    p.readout_box(font, &format!("{:.0}", s.position[1]), 402, 223);
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
    p.text(
        font,
        &format!("AGL {:.0}", (s.position[1] - ground).max(0.)),
        244,
        291,
    );
    p.text(
        font,
        &format!("V/S {:+.0}", s.vertical_speed * 60.),
        336,
        291,
    );
    if s.crashed {
        p.text(font, "CRASHED - ESC", 277, 306);
    } else if !s.engine {
        p.text(font, "ENGINE OFF", 283, 306);
    } else {
        p.clip = (174, 96, 292, 254);
        bank_scale(&mut p, font, s.bank);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bank_scale_tracks_aircraft_attitude_through_full_rolls() {
        assert_eq!(bank_tick_angle(30, 30f64.to_radians()), 0.);
        assert_eq!(bank_tick_angle(-30, (-30f64).to_radians()), 0.);
        assert_eq!(bank_tick_angle(0, 360f64.to_radians()), 0.);
        assert!((bank_tick_angle(-180, 179f64.to_radians()) - 1.).abs() < 1e-9);
        assert!((bank_tick_angle(-180, 181f64.to_radians()) + 1.).abs() < 1e-9);
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
