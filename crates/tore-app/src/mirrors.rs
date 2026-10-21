//! Runtime masks from reviewed flat source fills; fitted rear-view optics.
use crate::{attitude::Basis, camera::Camera, flight::State, menu::Sprite};
use tore_formats::aircraft::AircraftId;
pub const SIZE: [u32; 2] = [768, 384];

pub struct Masks {
    pub pixels: Vec<u8>,
    pub rects: [[f32; 4]; 3],
}
/// Seed pixels of the mirror regions in a cockpit art family's 1280x490
/// picture, centre, left and right (`None` where the art has no such mirror).
/// Authored identification hints from inspected art (fitted; 2026-10-12 for
/// the families added by slice BC), not native camera records. Families that
/// share a cockpit picture share a row: the picture's own name decides, so
/// the F-4 variants and the A-4E use the F-4 picture, the Su-27, AV-8B and
/// F-15 the AV-8 picture, the MiG-29, Su-25, MiG-23 and Mi-24 the SU-33 one,
/// the F-104 and A-10 the F-104 one. Pictures without a flat mirror fill
/// (F-16, F-22, X-31, MiG-17, MiG-21 and the transport/helicopter AC-130
/// picture) have none.
pub(crate) fn seeds(id: AircraftId) -> Option<[Option<[usize; 2]>; 3]> {
    let s = |x, y| Some([x, y]);
    Some(match id.cockpit_stem() {
        "F18" => [s(640, 40), s(110, 380), s(1170, 380)],
        "F14" => [s(640, 40), s(140, 250), s(1140, 250)],
        "F4" => [s(640, 40), s(200, 300), s(1080, 300)],
        "RAF" => [s(640, 40), s(190, 360), s(1090, 360)],
        "AV8" => [s(640, 40), s(204, 257), s(1064, 257)],
        "SU35" => [s(640, 40), s(203, 297), s(1075, 297)],
        "A7" => [s(641, 35), s(120, 304), s(1161, 304)],
        "F104" => [None, s(231, 232), s(1049, 232)],
        "SU33" => [None, s(176, 195), s(1103, 195)],
        "Y141" => [None, s(375, 91), s(902, 91)],
        _ => return None,
    })
}
/// Four-connected exact-color flood; bounds prevent unrelated artwork leaks.
pub fn masks(source: &Sprite, id: AircraftId) -> Option<Masks> {
    if [source.width, source.height] != [1280, 490] {
        return None;
    }
    flood(source, seeds(id)?)
}
fn flood(source: &Sprite, seeds: [Option<[usize; 2]>; 3]) -> Option<Masks> {
    let (w, h) = (source.width, source.height);
    if w == 0 || h == 0 || w.checked_mul(h)?.checked_mul(4)? != source.rgba.len() {
        return None;
    }
    let mut result = Masks {
        pixels: vec![0; w * h],
        rects: [[0.; 4]; 3],
    };
    for (index, seed) in seeds.into_iter().enumerate() {
        let Some([sx, sy]) = seed else {
            continue;
        };
        if sx >= w || sy >= h {
            return None;
        }
        let seed = sy * w + sx;
        let color = &source.rgba[seed * 4..seed * 4 + 4];
        if color[3] != 255 || result.pixels[seed] != 0 {
            return None;
        }
        let mut todo = vec![seed];
        let (mut x0, mut y0, mut x1, mut y1, mut count) = (w, h, 0, 0, 0);
        while let Some(at) = todo.pop() {
            if result.pixels[at] != 0 || &source.rgba[at * 4..at * 4 + 4] != color {
                continue;
            }
            result.pixels[at] = index as u8 + 1;
            count += 1;
            if count > w * h / 8 {
                return None;
            }
            let (x, y) = (at % w, at / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
            if x > 0 {
                todo.push(at - 1);
            }
            if x + 1 < w {
                todo.push(at + 1);
            }
            if y > 0 {
                todo.push(at - w);
            }
            if y + 1 < h {
                todo.push(at + w);
            }
        }
        if count < 4 {
            return None;
        }
        result.rects[index] = [x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32];
    }
    Some(result)
}
pub(crate) fn rear_basis(body: Basis) -> Basis {
    Basis {
        right: body.right.map(|v| -v),
        up: body.up,
        forward: body.forward.map(|v| -v),
    }
}
/// Fitted pilot's eye above and forward of the model origin, in feet.
pub(crate) fn pilot_eye(position: [f64; 3], body: Basis) -> [f64; 3] {
    std::array::from_fn(|i| position[i] + body.up[i] * 7. + body.forward[i] * 10.)
}
/// How much wider the rear camera's field of view is than a 1x view's 60
/// degrees vertical: 66 degrees vertical, about 105 across the 2:1 rear
/// picture (98 before). Opinionated, John 2026-10-19 ("maybe 10% wider").
pub const WIDENING: f64 = 1.1;
/// The rear camera's zoom: a 1x view's half-angle tangent over the widened
/// one's.
pub fn rear_zoom() -> f32 {
    let half = std::f64::consts::FRAC_PI_6;
    (half.tan() / (half * WIDENING).tan()) as f32
}
/// The one rear camera every mirror of a cockpit shows a crop of.
pub fn camera(state: &State) -> Camera {
    let body = Basis::new(state.yaw, state.pitch, state.bank);
    let [yaw, pitch, bank] = rear_basis(body).angles();
    let mut c = Camera::new();
    c.weather_slot = 1;
    c.position = pilot_eye(state.position, body);
    c.yaw = yaw as f32;
    c.pitch = pitch as f32;
    c.roll = -bank as f32;
    c.zoom = rear_zoom();
    c
}
/// Where mirror `id` (1 centre, 2 left, 3 right) of source rectangle
/// `rect` samples the rear picture at `uv` within the rectangle: an
/// aspect-preserving, horizontally reflected crop of the one picture. The
/// CPU twin of `cockpit.wgsl`'s mirror sampling, for tests.
#[cfg(test)]
pub fn crop(id: usize, rect: [f32; 4], uv: [f32; 2]) -> [f32; 2] {
    let width = (rect[2] / rect[3] / 2.).min(1.);
    let height = (2. * rect[3] / rect[2]).min(1.);
    let center = [0.5, 0.8, 0.2][id - 1];
    let start = (center - width * 0.5).clamp(0., 1. - width);
    [start + (1. - uv[0]) * width, 0.5 + (uv[1] - 0.5) * height]
}
/// Which visible flight frames draw the rear picture: every other one, the
/// mirrors showing the last picture in between (John, 2026-10-19). The first
/// visible frame after hidden ones always draws, so a mirror never shows an
/// old picture.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cadence {
    frame: u64,
    drawn: Option<u64>,
}
impl Cadence {
    /// Called once per flight frame; whether this frame draws the mirrors.
    pub fn due(&mut self, visible: bool) -> bool {
        self.frame += 1;
        if !visible {
            return false;
        }
        let due = self.drawn != Some(self.frame - 1);
        if due {
            self.drawn = Some(self.frame);
        }
        due
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mirrors_draw_every_other_visible_frame_and_never_start_stale() {
        let mut c = Cadence::default();
        let drawn: Vec<bool> = (0..6).map(|_| c.due(true)).collect();
        assert_eq!(drawn, [true, false, true, false, true, false]);
        // Hidden frames draw nothing; the first visible one after them draws.
        assert!(!c.due(false));
        assert!(!c.due(false));
        assert!(c.due(true));
        assert!(!c.due(true));
        // A single hidden frame between: the next visible frame draws even
        // though the frame before the gap did not.
        assert!(c.due(true));
        assert!(!c.due(false));
        assert!(c.due(true));
        // A fresh cadence (new aircraft or scene) draws at once.
        let mut c = Cadence::default();
        assert!(c.due(true));
        // Half the visible frames over a long run.
        let mut c = Cadence::default();
        assert_eq!((0..1000).filter(|_| c.due(true)).count(), 500);
    }
    #[test]
    fn rear_camera_is_ten_percent_wider_than_a_1x_view() {
        let zoom = rear_zoom();
        // Camera::uniform projects with focal = half height * sqrt(3) * zoom.
        let half = (1. / (3f64.sqrt() * f64::from(zoom))).atan();
        assert!((2. * half.to_degrees() - 66.).abs() < 1e-3, "{zoom}");
        let across = (2. * (2. * half.tan()).atan()).to_degrees();
        assert!((across - 104.86).abs() < 0.05, "{across}");
        assert!(zoom < 1.);
    }
    #[test]
    fn every_mirror_is_a_crop_of_the_one_rear_picture() {
        // Each mirror's corners land inside the 2:1 picture, keep the
        // mirror's own aspect on it, and the side mirrors look to their own
        // side of it (the picture is reflected: the left mirror shows the
        // right of the rear picture's centre, which is the aircraft's left).
        for (id, rect) in [
            (1, [500., 20., 280., 50.]),
            (2, [60., 340., 110., 70.]),
            (3, [1110., 340., 110., 70.]),
        ] {
            let corners = [[0., 0.], [1., 0.], [0., 1.], [1., 1.]].map(|uv| crop(id, rect, uv));
            for [u, v] in corners {
                assert!(
                    (0. ..=1.).contains(&u) && (0. ..=1.).contains(&v),
                    "{id} {u} {v}"
                );
            }
            let w = (corners[0][0] - corners[1][0]).abs() * SIZE[0] as f32;
            let h = (corners[2][1] - corners[0][1]).abs() * SIZE[1] as f32;
            assert!((w / h - rect[2] / rect[3]).abs() < 1e-3, "{id}");
        }
        let centre = |id, rect| crop(id, rect, [0.5, 0.5])[0];
        assert!(centre(2, [60., 340., 110., 70.]) > 0.5);
        assert!(centre(3, [1110., 340., 110., 70.]) < 0.5);
        assert_eq!(centre(1, [500., 20., 280., 50.]), 0.5);
        // The shader samples the same crops from a single rear texture.
        let shader = include_str!("cockpit.wgsl");
        assert!(shader.contains("array<f32,3>(.5,.8,.2)"));
        assert_eq!(shader.matches("textureSampleLevel(rear,").count(), 1);
    }
    #[test]
    fn rear_camera_tracks_body_through_bank_and_vertical_attitudes() {
        for (yaw, pitch, bank) in [
            (0., 0., 0.),
            (1., 0.5, 1.),
            (2., std::f64::consts::FRAC_PI_2, 2.),
            (3., 2., -1.),
        ] {
            let body = Basis::new(yaw, pitch, bank);
            let [y, p, b] = rear_basis(body).angles();
            let rear = Basis::new(y, p, b);
            assert!(crate::attitude::dot(rear.forward, body.forward) < -0.999999);
            assert!(crate::attitude::dot(rear.up, body.up) > 0.999999);
        }
    }
    #[test]
    fn flood_preserves_outlines_and_rejects_leaks_and_overlaps() {
        let mut s = Sprite {
            width: 30,
            height: 12,
            rgba: vec![0; 30 * 12 * 4],
            glyphs: vec![],
        };
        for x0 in [1, 11, 21] {
            for y in 2..5 {
                for x in x0..x0 + 4 {
                    s.rgba[(y * 30 + x) * 4..(y * 30 + x) * 4 + 4]
                        .copy_from_slice(&[10, 20, 30, 255]);
                }
            }
        }
        let all = |a: [usize; 2], b: [usize; 2], c: [usize; 2]| [Some(a), Some(b), Some(c)];
        let m = flood(&s, all([2, 3], [12, 3], [22, 3])).unwrap();
        assert_eq!(m.rects[0], [1., 2., 4., 3.]);
        assert_eq!(m.pixels.iter().filter(|v| **v != 0).count(), 36);
        assert_eq!(m.pixels[0], 0);
        assert!(flood(&s, all([2, 3], [2, 3], [22, 3])).is_none());
        assert!(flood(&s, all([0, 0], [12, 3], [22, 3])).is_none());
        // A side-only picture leaves the centre slot empty and keeps the
        // left and right slots' own numbers.
        let sides = flood(&s, [None, Some([12, 3]), Some([22, 3])]).unwrap();
        assert_eq!(sides.rects[0], [0.; 4]);
        assert_eq!(sides.rects[1], [11., 2., 4., 3.]);
        assert_eq!(sides.rects[2], [21., 2., 4., 3.]);
        assert_eq!(sides.pixels[3 * 30 + 12], 2);
        assert_eq!(sides.pixels[3 * 30 + 22], 3);
        assert_eq!(sides.pixels[3 * 30 + 2], 0);
        s.rgba.fill(255);
        assert!(flood(&s, all([2, 3], [12, 3], [22, 3])).is_none());
    }
    #[test]
    fn mirror_seeds_follow_the_cockpit_picture_family() {
        use AircraftId::*;
        let family = |ids: &[AircraftId]| {
            let first = seeds(ids[0]);
            assert!(first.is_some(), "{:?}", ids[0]);
            for id in ids {
                assert_eq!(seeds(*id), first, "{id:?}");
            }
        };
        family(&[A4E, F4B, F4J, F4E, F4G]);
        family(&[Su27, Av8, F15]);
        family(&[Mig29, Su25, Mig23, Mi24]);
        family(&[F104, A10]);
        for id in [F18, F14, Rafale, Su35, A7, Yak141] {
            assert!(seeds(id).is_some(), "{id:?}");
        }
        // The F-16, F-22, X-31, MiG-17, MiG-21 and AC-130 pictures have no
        // flat mirror fill.
        for id in [
            F16C, F22, F22n, Faxx, X31, Mig17, Mig21, C130, Ac130, E3, Il76, E2, V22, Ah64, Ch47,
            B747, A310,
        ] {
            assert!(seeds(id).is_none(), "{id:?}");
        }
        // Every table: seeds are inside the 1280x490 picture and distinct, and
        // a picture with side mirrors names both sides.
        for id in AircraftId::ALL {
            let Some(table) = seeds(id) else { continue };
            assert_eq!(table[1].is_some(), table[2].is_some(), "{id:?}");
            let mut points: Vec<_> = table.iter().flatten().collect();
            assert!(points.iter().all(|p| p[0] < 1280 && p[1] < 490));
            points.sort();
            points.dedup();
            assert_eq!(points.len(), table.iter().flatten().count());
        }
    }
}
