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
pub fn camera(state: &State) -> Camera {
    let body = Basis::new(state.yaw, state.pitch, state.bank);
    let [yaw, pitch, bank] = rear_basis(body).angles();
    let mut c = Camera::new();
    c.weather_slot = 1;
    c.position = pilot_eye(state.position, body);
    c.yaw = yaw as f32;
    c.pitch = pitch as f32;
    c.roll = -bank as f32;
    c.zoom = 1.;
    c
}
#[cfg(test)]
mod tests {
    use super::*;
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
