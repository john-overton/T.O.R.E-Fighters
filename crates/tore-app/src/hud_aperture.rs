//! Fitted source-art apertures for drawing the HUD behind its combiner glass.
use crate::menu::Sprite;
use tore_formats::aircraft::AircraftId;

type Point = (i32, i32);

pub fn mask(source: &Sprite, id: AircraftId) -> Vec<u8> {
    let mut out = vec![0; source.width.saturating_mul(source.height)];
    if source.rgba.len() != out.len().saturating_mul(4) {
        return out;
    }
    let (expected, polygon): ((usize, usize), &[Point]) = match id {
        AircraftId::F18 => (
            (1280, 490),
            &[
                (562, 129),
                (711, 129),
                (776, 181),
                (739, 371),
                (536, 371),
                (497, 181),
            ],
        ),
        AircraftId::Rafale => (
            (1280, 490),
            &[
                (553, 128),
                (728, 128),
                (798, 179),
                (742, 389),
                (539, 389),
                (483, 179),
            ],
        ),
        AircraftId::F14 => (
            (1280, 490),
            &[
                (554, 139),
                (725, 139),
                (772, 205),
                (772, 330),
                (738, 405),
                (544, 405),
                (508, 330),
                (508, 205),
            ],
        ),
        AircraftId::A4E => (
            (1280, 490),
            &[
                (560, 119),
                (719, 119),
                (769, 160),
                (753, 337),
                (721, 337),
                (558, 337),
                (510, 319),
                (510, 160),
            ],
        ),
        AircraftId::X31 => (
            (1280, 490),
            &[
                (568, 132),
                (712, 132),
                (795, 185),
                (753, 386),
                (532, 386),
                (465, 185),
            ],
        ),
        AircraftId::Mig29 | AircraftId::Su25 | AircraftId::Mig23 => (
            (1280, 490),
            &[(511, 100), (770, 100), (752, 388), (528, 388)],
        ),
        AircraftId::Su27 => (
            (1280, 490),
            &[
                (553, 121),
                (699, 121),
                (780, 176),
                (718, 376),
                (561, 376),
                (490, 176),
            ],
        ),
        AircraftId::Mig21 => (
            (1280, 490),
            &[
                (436, 137),
                (786, 137),
                (774, 248),
                (832, 251),
                (813, 345),
                (476, 345),
            ],
        ),
        AircraftId::Su35 => (
            (1280, 490),
            &[(510, 116), (769, 116), (769, 353), (520, 353)],
        ),
        AircraftId::F22 | AircraftId::F22n | AircraftId::Faxx => (
            (1000, 490),
            &[
                (388, 91),
                (612, 91),
                (690, 190),
                (690, 286),
                (610, 401),
                (390, 401),
                (310, 286),
                (310, 190),
            ],
        ),
    };
    if (source.width, source.height) != expected {
        return out;
    }
    for y in 0..source.height {
        for x in 0..source.width {
            let at = y * source.width + x;
            if source.rgba[at * 4 + 3] == 0 && inside((x as i32, y as i32), polygon) {
                out[at] = 1;
            }
        }
    }
    out
}

fn inside((x, y): Point, polygon: &[Point]) -> bool {
    let mut odd = false;
    let mut previous = polygon[polygon.len() - 1];
    for &current in polygon {
        if (current.1 > y) != (previous.1 > y) {
            let crossing = f64::from(previous.0 - current.0) * f64::from(y - current.1)
                / f64::from(previous.1 - current.1)
                + f64::from(current.0);
            odd ^= f64::from(x) < crossing;
        }
        previous = current;
    }
    odd
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transparent(width: usize, height: usize) -> Sprite {
        Sprite {
            width,
            height,
            rgba: vec![0; width * height * 4],
            glyphs: vec![],
        }
    }

    #[test]
    fn fitted_glass_is_non_rectangular_and_excludes_opaque_frame() {
        let mut source = transparent(1280, 490);
        let frame = 180 * source.width + 500;
        source.rgba[frame * 4 + 3] = 255;
        let mask = mask(&source, AircraftId::F18);
        assert_eq!(mask.len(), 1280 * 490);
        assert_eq!(mask[200 * 1280 + 640], 1);
        assert_eq!(mask[140 * 1280 + 510], 0);
        assert_eq!(mask[frame], 0);
        assert!(mask.iter().all(|value| matches!(value, 0 | 1)));
    }

    #[test]
    fn a4_lower_gap_stays_outside_the_combiner() {
        let source = transparent(1280, 490);
        let mask = mask(&source, AircraftId::A4E);
        assert_eq!(mask[250 * 1280 + 640], 1);
        assert_eq!(mask[354 * 1280 + 560], 0);
    }

    #[test]
    fn shared_families_match_and_unknown_dimensions_fail_closed() {
        let source = transparent(1280, 490);
        assert_eq!(
            mask(&source, AircraftId::Mig29),
            mask(&source, AircraftId::Su25)
        );
        assert_eq!(
            mask(&source, AircraftId::Su25),
            mask(&source, AircraftId::Mig23)
        );
        assert!(
            mask(&transparent(640, 245), AircraftId::F18)
                .iter()
                .all(|v| *v == 0)
        );
        assert!(mask(&source, AircraftId::F22).iter().all(|v| *v == 0));
    }

    #[test]
    fn f22_and_faxx_share_the_reviewed_source_aperture() {
        let source = transparent(1000, 490);
        let f22 = mask(&source, AircraftId::F22);
        assert_eq!(f22, mask(&source, AircraftId::Faxx));
        assert_eq!(f22[200 * 1000 + 500], 1);
        assert_eq!(f22[100 * 1000 + 320], 0);
    }
}
