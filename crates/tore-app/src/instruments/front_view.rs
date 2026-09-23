//! Forward-view symbology: horizon, flight path marker, airspeed and altitude.
//! Opinionated: John requested the marks on 2026-09-22; their layout is an
//! agent choice. The original window shows the picture alone. Angles use the
//! HUD camera projection so the marks stay registered with the picture.
use super::Raster;
use crate::{flight::State, hud};
use tore_formats::font::Font;

/// Camera picture inside the instrument raster: x, y, width, height.
const VIEW: (i32, i32, i32, i32) = (11, 21, 138, 114);
/// Half the horizon gap around the nose, keeping the marker readable.
const HORIZON_GAP: f64 = 10.;
/// Half the horizon bar's length, stopping short of the side readouts.
const HORIZON_HALF_LENGTH: f64 = 40.;

/// Flight data sampled with one camera frame, so the symbology describes the
/// same instant as the picture it overlays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Symbology {
    pitch: f64,
    bank: f64,
    /// Flight path bearing from the nose and climb angle, absent near a standstill.
    path: Option<(f64, f64)>,
    airspeed_knots: f64,
    altitude_ft: f64,
}
impl Symbology {
    pub fn new(s: &State, air: Option<&tore_sim::telemetry::AirData>) -> Self {
        Self {
            pitch: s.pitch,
            bank: s.bank,
            // Same speed floor as the HUD marker.
            path: (s.speed > 10.).then(|| {
                (
                    s.velocity[0].atan2(s.velocity[2]) - s.yaw,
                    s.velocity[1].atan2(s.velocity[0].hypot(s.velocity[2])),
                )
            }),
            airspeed_knots: air.map_or(s.speed / 1.68781, |d| d.true_airspeed_knots),
            altitude_ft: s.position[1],
        }
    }
}

/// A HUD screen point moved into the camera picture. Both use the same
/// vertical field of view, so only the centre and scale change.
fn to_view((x, y): (f64, f64)) -> (f64, f64) {
    let scale = f64::from(VIEW.3) / 480.;
    (
        f64::from(VIEW.0) + f64::from(VIEW.2) / 2. + (x - 320.) * scale,
        f64::from(VIEW.1) + f64::from(VIEW.3) / 2. + (y - 240.) * scale,
    )
}

/// Clip a segment to the camera picture.
fn clip(a: (f64, f64), b: (f64, f64)) -> Option<((i32, i32), (i32, i32))> {
    let (left, top) = (f64::from(VIEW.0), f64::from(VIEW.1));
    let (right, bottom) = (left + f64::from(VIEW.2) - 1., top + f64::from(VIEW.3) - 1.);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0f64, 1f64);
    for (p, q) in [
        (-dx, a.0 - left),
        (dx, right - a.0),
        (-dy, a.1 - top),
        (dy, bottom - a.1),
    ] {
        if p == 0. {
            if q < 0. {
                return None;
            }
        } else if p < 0. {
            t0 = t0.max(q / p);
        } else {
            t1 = t1.min(q / p);
        }
    }
    let at = |t: f64| ((a.0 + dx * t).round() as i32, (a.1 + dy * t).round() as i32);
    (t0 <= t1).then(|| (at(t0), at(t1)))
}

fn segment(r: &mut Raster, a: (f64, f64), b: (f64, f64), color: [u8; 4]) {
    if let Some((a, b)) = clip(a, b) {
        r.line(a, b, color);
    }
}

fn readout(
    r: &mut Raster,
    font: &Font,
    label: &str,
    value: &str,
    right_align: bool,
    color: [u8; 4],
) {
    let width = |t: &str| {
        t.bytes()
            .map(|c| font.glyphs[c as usize].advance as i32)
            .sum::<i32>()
    };
    let height = font.height as i32;
    // Plain text, like the labels: no box or backing behind the digits. The
    // digits sit just above the centre so a level horizon passes beneath them.
    let top = VIEW.1 + VIEW.3 / 2 - height - 3;
    let x = |t: &str| {
        if right_align {
            VIEW.0 + VIEW.2 - 3 - width(t)
        } else {
            VIEW.0 + 3
        }
    };
    r.text(font, value, x(value), top, color);
    r.text(font, label, x(label), top - height - 2, color);
}

pub(super) fn draw(r: &mut Raster, font: &Font, symbology: &Symbology, color: [u8; 3]) {
    let color = [color[0], color[1], color[2], 255];
    let Symbology {
        pitch, bank, path, ..
    } = *symbology;
    // A fixed-length bar on the true horizon, centred on the point below the nose.
    if let Some(nose) = hud::project(pitch, bank, 0., 0., 1.).map(to_view) {
        let along = (bank.cos(), -bank.sin());
        for side in [-1., 1.] {
            let at = |d: f64| (nose.0 + along.0 * d * side, nose.1 + along.1 * d * side);
            segment(r, at(HORIZON_GAP), at(HORIZON_HALF_LENGTH), color);
        }
    }
    if let Some((bearing, climb)) = path
        && let Some((x, y)) = hud::project(pitch, bank, bearing, climb, 1.).map(to_view)
    {
        let (x, y) = (x.round() as i32, y.round() as i32);
        // Hidden rather than pinned when it leaves the picture, as on the HUD.
        if (VIEW.0 + 8..VIEW.0 + VIEW.2 - 8).contains(&x)
            && (VIEW.1 + 6..VIEW.1 + VIEW.3 - 3).contains(&y)
        {
            r.circle(x, y, 3., color);
            r.line((x - 8, y), (x - 3, y), color);
            r.line((x + 3, y), (x + 8, y), color);
            r.line((x, y - 6), (x, y - 3), color);
        }
    }
    readout(
        r,
        font,
        "TAS",
        &format!("{:.0}", symbology.airspeed_knots),
        false,
        color,
    );
    readout(
        r,
        font,
        "MSL",
        &format!("{:.0}", symbology.altitude_ft),
        true,
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> Font {
        Font {
            height: 7,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 5,
                    pixels: vec![(0, 0)],
                })
                .collect(),
        }
    }
    fn symbology(pitch: f64, bank: f64, path: Option<(f64, f64)>) -> Symbology {
        Symbology {
            pitch: pitch.to_radians(),
            bank: bank.to_radians(),
            path: path.map(|(b, c): (f64, f64)| (b.to_radians(), c.to_radians())),
            airspeed_knots: 450.,
            altitude_ft: 12_500.,
        }
    }
    const COLOR: [u8; 4] = [10, 200, 30, 255];
    fn lit(r: &Raster, x: i32, y: i32) -> bool {
        let i = (y as usize * super::super::WIDTH + x as usize) * 4;
        r.pixels[i..i + 4] == COLOR
    }
    fn render(s: &Symbology) -> Raster {
        let mut r = Raster::new();
        draw(&mut r, &font(), s, [COLOR[0], COLOR[1], COLOR[2]]);
        r
    }

    #[test]
    fn view_mapping_matches_the_camera_panel_projection() {
        // The panel camera renders 138x114 with a 60 degree vertical field.
        assert_eq!(to_view((320., 240.)), (80., 78.));
        let (_, y) = to_view(hud::project(0., 0., 0., 10f64.to_radians(), 1.).unwrap());
        let expected = 78. - 57. * 3f64.sqrt() * 10f64.to_radians().tan();
        assert!((y - expected).abs() < 1e-9);
    }

    #[test]
    fn level_horizon_is_a_centred_bar_with_a_gap_at_the_nose() {
        let r = render(&symbology(0., 0., None));
        assert!(lit(&r, 40, 78) && lit(&r, 70, 78) && lit(&r, 90, 78) && lit(&r, 120, 78));
        assert!(!lit(&r, 80, 78));
        // The bar stops short of the side readouts.
        assert!(!lit(&r, 38, 78) && !lit(&r, 122, 78));
        // Clipped to the picture, never onto the frame or the buttons.
        let steep = render(&symbology(33., 0., None));
        assert!((135..156).all(|y| (0..160).all(|x| !lit(&steep, x, y))));
    }

    #[test]
    fn horizon_moves_opposite_pitch_and_tilts_with_bank() {
        let climbing = render(&symbology(10., 0., None));
        let below = 78 + (57. * 3f64.sqrt() * 10f64.to_radians().tan()).round() as i32;
        assert!(lit(&climbing, 40, below) && !lit(&climbing, 40, 78));
        // Right bank rolls the horizon's right end upward on screen.
        let banked = render(&symbology(0., 30., None));
        let dy = (30. * 30f64.to_radians().tan()).round() as i32;
        let near = |x, y: i32| (y - 1..=y + 1).any(|y| lit(&banked, x, y));
        assert!(near(110, 78 - dy) && near(50, 78 + dy));
        assert!(!near(110, 78 + dy) && !near(50, 78 - dy));
    }

    #[test]
    fn flight_path_marker_follows_the_velocity_and_hides_off_picture() {
        let level = render(&symbology(5., 0., Some((0., 0.))));
        let (_, y) = to_view(hud::project(5f64.to_radians(), 0., 0., 0., 1.).unwrap());
        let y = y.round() as i32;
        assert!(lit(&level, 80 - 8, y) && lit(&level, 80 + 8, y) && lit(&level, 80, y - 6));
        let without = render(&symbology(5., 0., None));
        assert!(!lit(&without, 80, y - 6));
        let off = render(&symbology(0., 0., Some((40., 0.))));
        assert_eq!(off.pixels, render(&symbology(0., 0., None)).pixels);
    }

    #[test]
    fn readouts_sit_inside_the_picture_on_opposite_sides() {
        let r = render(&symbology(0., 0., None));
        // The synthetic glyphs light their top-left pixel: "450" starts three
        // pixels inside the left edge and "12500" ends three inside the right.
        let top = 78 - 7 - 3;
        assert!(lit(&r, 14, top) && lit(&r, 19, top) && lit(&r, 24, top));
        assert!(lit(&r, 121, top) && lit(&r, 141, top) && !lit(&r, 146, top));
        // No box: nothing is drawn around the digits.
        assert!(!lit(&r, 14, top + 3) && !lit(&r, 13, top));
    }

    #[test]
    fn sampling_uses_true_airspeed_and_hides_the_marker_when_stopped() {
        let mut state =
            State::new(&crate::flight::animation_tests::profile(), [0., 5000., 0.]).unwrap();
        state.speed = 5.;
        assert_eq!(Symbology::new(&state, None).path, None);
        state.speed = 400.;
        state.velocity = [0., 0., 400.];
        state.yaw = 0.;
        let s = Symbology::new(&state, None);
        assert_eq!(s.path, Some((0., 0.)));
        assert!((s.airspeed_knots - 400. / 1.68781).abs() < 1e-9);
        assert_eq!(s.altitude_ft, 5000.);
    }
}
