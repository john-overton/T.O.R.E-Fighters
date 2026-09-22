//! Runtime masks from reviewed flat source fills; fitted rear-view optics.
use crate::{attitude::Basis, flight::State, menu::Sprite, terrain::Camera};
use tore_formats::aircraft::AircraftId;
pub const SIZE: [u32; 2] = [768, 384];

pub struct Masks {
    pub pixels: Vec<u8>,
    pub rects: [[f32; 4]; 3],
}
/// Four-connected exact-color flood; bounds prevent unrelated artwork leaks.
pub fn masks(source: &Sprite, id: AircraftId) -> Option<Masks> {
    if [source.width, source.height] != [1280, 490] {
        return None;
    }
    if matches!(id, AircraftId::Su27 | AircraftId::Su35) {
        return flood(source, [[640, 40]]);
    }
    let seeds = match id {
        AircraftId::F18 => [[640, 40], [110, 380], [1170, 380]],
        AircraftId::F14 => [[640, 40], [140, 250], [1140, 250]],
        AircraftId::A4E => [[640, 40], [200, 300], [1080, 300]],
        AircraftId::X31
        | AircraftId::Mig29
        | AircraftId::Su27
        | AircraftId::Mig21
        | AircraftId::Su25
        | AircraftId::Mig23
        | AircraftId::Su35
        | AircraftId::F22
        | AircraftId::F22n
        | AircraftId::Faxx => return None,
        AircraftId::Rafale => [[640, 40], [190, 360], [1090, 360]],
    };
    flood(source, seeds)
}
fn flood<const N: usize>(source: &Sprite, seeds: [[usize; 2]; N]) -> Option<Masks> {
    let (w, h) = (source.width, source.height);
    if w == 0 || h == 0 || w.checked_mul(h)?.checked_mul(4)? != source.rgba.len() {
        return None;
    }
    let mut result = Masks {
        pixels: vec![0; w * h],
        rects: [[0.; 4]; 3],
    };
    for (index, [sx, sy]) in seeds.into_iter().enumerate() {
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
fn rear_basis(body: Basis) -> Basis {
    Basis {
        right: body.right.map(|v| -v),
        up: body.up,
        forward: body.forward.map(|v| -v),
    }
}
pub fn camera(state: &State) -> Camera {
    let body = Basis::new(state.yaw, state.pitch, state.bank);
    let [yaw, pitch, bank] = rear_basis(body).angles();
    let mut c = Camera::new();
    c.weather_slot = 1;
    // Fitted eye above/forward of the model origin, in feet.
    c.position = std::array::from_fn(|i| {
        (state.position[i] + body.up[i] * 7. + body.forward[i] * 10.) as f32
    });
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
        let m = flood(&s, [[2, 3], [12, 3], [22, 3]]).unwrap();
        assert_eq!(m.rects[0], [1., 2., 4., 3.]);
        assert_eq!(m.pixels.iter().filter(|v| **v != 0).count(), 36);
        assert_eq!(m.pixels[0], 0);
        assert!(flood(&s, [[2, 3], [2, 3], [22, 3]]).is_none());
        assert!(flood(&s, [[0, 0], [12, 3], [22, 3]]).is_none());
        s.rgba.fill(255);
        assert!(flood(&s, [[2, 3], [12, 3], [22, 3]]).is_none());
    }
}
