//! Fitted source-art apertures for drawing the HUD behind its combiner glass.
use crate::menu::Sprite;
use tore_formats::aircraft::AircraftId;

type Point = (i32, i32);

pub fn mask(source: &Sprite, id: AircraftId) -> Vec<u8> {
    let mut out = vec![0; source.width.saturating_mul(source.height)];
    if source.rgba.len() != out.len().saturating_mul(4) {
        return out;
    }
    // Shared artwork uses the same reviewed glass, never the donor's physics.
    let id = match id.cockpit_stem() {
        "F4" => AircraftId::A4E,
        "AV8" => AircraftId::Su27,
        "SU33" => AircraftId::Mig29,
        // The MiG-31 flies the Yak-141 cockpit (John, 2026-11-08).
        "Y141" => AircraftId::Yak141,
        // The Mirage 2000 flies the Rafale cockpit, as its record names
        // rafale.HUD (John, 2026-11-09).
        "RAF" => AircraftId::Rafale,
        // Batch 5 (B5-1): the Mirage F1 names rafale.HUD like the Mirage 2000
        // and flies the Rafale glass. The F-15E names av8.HUD like the F-15C,
        // and "AV8" above already resolves to the same glass.
        // F-15E: av8.HUD is the decided binding (opinionated, John,
        // 2026-11-10, F15E-7), no longer interim.
        _ => id,
    };
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
        // Fitted apertures reviewed against the imported high-resolution art.
        AircraftId::A7 => (
            (1280, 490),
            &[(484, 137), (800, 137), (722, 410), (555, 410)],
        ),
        AircraftId::F104 | AircraftId::A10 => (
            (1280, 490),
            &[
                (579, 158),
                (701, 158),
                (779, 210),
                (749, 434),
                (531, 434),
                (501, 210),
            ],
        ),
        AircraftId::F16C | AircraftId::F16A => (
            (1280, 490),
            &[
                (472, 99),
                (505, 74),
                (640, 60),
                (775, 74),
                (808, 99),
                (774, 366),
                (505, 366),
            ],
        ),
        AircraftId::Yak141 => (
            (1280, 490),
            &[
                (607, 83),
                (668, 83),
                (723, 113),
                (772, 206),
                (799, 401),
                (478, 401),
                (504, 206),
                (556, 113),
            ],
        ),
        AircraftId::Mig17 => (
            (1280, 490),
            &[
                (544, 161),
                (552, 147),
                (728, 147),
                (736, 161),
                (732, 335),
                (548, 335),
            ],
        ),
        AircraftId::C130
        | AircraftId::Ac130
        | AircraftId::E3
        | AircraftId::Il76
        | AircraftId::E2
        | AircraftId::V22
        | AircraftId::Ah64
        | AircraftId::Ch47
        | AircraftId::B747
        | AircraftId::A310 => ((1280, 490), &[(0, 0), (1279, 0), (1279, 489), (0, 489)]),
        // Interim, agent decision 2026-11-10, pending John (batch decision
        // round): the Gripen (~GRIH.PIC), F-5E (~F29H.PIC, the X-29's art) and
        // Tornado (~EF2H.PIC) name their own cockpit pictures, whose glass has
        // not been measured. No expected size, so the aperture fails closed
        // (an empty mask) until their cockpit slices measure a polygon.
        // Update (GRI-7, 2026-11-10): the Gripen's glass is measured below, so
        // only the F-5E and the Tornado still fail closed here.
        AircraftId::F5E | AircraftId::Tornado => ((0, 0), &[]),
        // The Gripen's own ~GRIH.PIC is 1279x490, not 1280. Fitted, measured
        // from the transparent run inside the teal combiner outline and
        // between the two slanted posts, down to the top of the base bar
        // (GRI-7, 2026-11-10): the arc from (632, 118) over (531, 136) to the
        // vertical sides at x 466 and 814 from y 190 to 262, the posts sloping
        // in to (510, 383) and (770, 383).
        AircraftId::Gripen => (
            (1279, 490),
            &[
                (632, 118),
                (583, 124),
                (531, 136),
                (501, 148),
                (477, 178),
                (466, 190),
                (466, 262),
                (469, 300),
                (510, 383),
                (770, 383),
                (811, 300),
                (814, 262),
                (814, 190),
                (803, 178),
                (779, 148),
                (749, 136),
                (697, 124),
                (648, 118),
            ],
        ),
        // The cockpit aliases above exhaust these identities.
        AircraftId::Av8
        | AircraftId::Mi24
        | AircraftId::F4B
        | AircraftId::F4J
        | AircraftId::F4E
        | AircraftId::F4G
        | AircraftId::F15
        | AircraftId::Mig25
        | AircraftId::Mig31
        | AircraftId::M2000
        | AircraftId::F15E
        | AircraftId::Mf1 => unreachable!("resolved cockpit alias"),
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

    /// GRI-7: the Gripen's own 1279x490 picture has a measured glass polygon
    /// (the arc, the vertical sides, the sloping posts and the base bar), not
    /// the empty mask of the B5-1 interim; any other size fails closed.
    #[test]
    fn the_gripen_glass_is_measured_for_its_1279_picture() {
        let source = transparent(1279, 490);
        let gripen = mask(&source, AircraftId::Gripen);
        assert_eq!(gripen.len(), 1279 * 490);
        let at = |x: usize, y: usize| gripen[y * 1279 + x];
        // Inside: centre of the glass, near the top of the arc, between posts.
        assert_eq!(at(640, 250), 1);
        assert_eq!(at(640, 121), 1);
        assert_eq!(at(500, 350), 1);
        // Outside: above the arc, outside the sides, the post area, the bar.
        assert_eq!(at(640, 110), 0);
        assert_eq!(at(470, 150), 0);
        assert_eq!(at(450, 250), 0);
        assert_eq!(at(480, 350), 0);
        assert_eq!(at(640, 390), 0);
        // Left and right are mirror images about the centre column 640.
        for y in [120, 150, 190, 262, 300, 350, 380] {
            let left = (0..640).filter(|x| at(*x, y) == 1).count();
            let right = (640..1279).filter(|x| at(*x, y) == 1).count();
            assert!(left.abs_diff(right) <= 1, "{y}: {left} {right}");
        }
        // An opaque art pixel inside the polygon stays out of the glass.
        let mut framed = transparent(1279, 490);
        framed.rgba[(200 * 1279 + 466) * 4 + 3] = 255;
        assert_eq!(mask(&framed, AircraftId::Gripen)[200 * 1279 + 466], 0);
        // A 1280x490 picture is not the Gripen's: fail closed.
        assert!(
            mask(&transparent(1280, 490), AircraftId::Gripen)
                .iter()
                .all(|v| *v == 0)
        );
    }

    /// F15E-7: the F-15E flies the F-15C's `av8.HUD` family, so its glass is the
    /// same measured polygon, not an empty one.
    #[test]
    fn the_f15e_draws_the_f15c_glass() {
        let source = transparent(1280, 490);
        let f15e = mask(&source, AircraftId::F15E);
        assert_eq!(f15e, mask(&source, AircraftId::F15));
        assert_eq!(f15e, mask(&source, AircraftId::Su27));
        assert!(f15e.contains(&1));
    }
}
