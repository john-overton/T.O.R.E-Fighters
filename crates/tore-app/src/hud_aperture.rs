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
        // Tornado IDS (TOR-7, 2026-11-11): `TOR.PT` names e2000.HUD, so it
        // flies the Eurofighter's cockpit art `~EF2H.PIC` by name (John,
        // 2026-11-10, opinionated). The glass is the one closed transparent
        // region inside the combiner frame: the thin frame lines (the green
        // arc on top, the two side lines, the green bar at y 365) and the two
        // support cables bound it, so it is flood-bounded by the art itself
        // (73,769 pixels, x 475 to 803, y 105 to 364). The polygon runs along
        // those lines every 16 rows, so it never reaches the transparent sky
        // outside the frame and loses about 1.2 percent of the glass at the
        // chords. Measured on the imported art,
        // `the_tornado_glass_matches_the_imported_eurofighter_art`.
        AircraftId::Tornado => (
            (1280, 490),
            &[
                (611, 105),
                (523, 121),
                (498, 137),
                (485, 153),
                (480, 169),
                (476, 185),
                (475, 201),
                (474, 217),
                (486, 233),
                (489, 249),
                (489, 265),
                (484, 281),
                (488, 297),
                (494, 313),
                (505, 329),
                (526, 345),
                (551, 361),
                (553, 364),
                (725, 364),
                (727, 361),
                (752, 345),
                (773, 329),
                (784, 313),
                (790, 297),
                (794, 281),
                (789, 265),
                (789, 249),
                (791, 233),
                (804, 217),
                (803, 201),
                (802, 185),
                (798, 169),
                (793, 153),
                (780, 137),
                (755, 121),
                (667, 105),
            ],
        ),
        // Interim, agent decision 2026-11-10, pending John (batch decision
        // round): the Gripen (~GRIH.PIC) and F-5E (~F29H.PIC, the X-29's art)
        // name their own cockpit pictures, whose glass has not been measured
        // here (the Tornado's is measured above, TOR-7). No expected size, so
        // the aperture fails closed (an empty mask) until their cockpit slices
        // measure a polygon.
        AircraftId::Gripen | AircraftId::F5E => ((0, 0), &[]),
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

    /// Opaque-glass pixels of the polygon on the retail `~EF2H.PIC`, and the
    /// whole closed transparent region inside the combiner frame.
    const TORNADO_GLASS_PIXELS: usize = 72_917;
    const TORNADO_FRAME_INTERIOR: usize = 73_769;

    #[test]
    fn the_tornado_draws_the_eurofighter_glass_inside_the_combiner_frame() {
        // Synthetic art: all transparent, so the mask is the polygon itself.
        let source = transparent(1280, 490);
        let mask = mask(&source, AircraftId::Tornado);
        assert_eq!(mask.len(), 1280 * 490);
        // The centre of the glass is in; above the arc, outside the side lines
        // and below the bottom bar are out.
        assert_eq!(mask[250 * 1280 + 640], 1);
        assert_eq!(mask[100 * 1280 + 640], 0);
        assert_eq!(mask[250 * 1280 + 450], 0);
        assert_eq!(mask[250 * 1280 + 830], 0);
        assert_eq!(mask[380 * 1280 + 640], 0);
        // An opaque pixel inside the polygon is left out.
        let mut framed = transparent(1280, 490);
        framed.rgba[(250 * 1280 + 640) * 4 + 3] = 255;
        assert_eq!(
            super::mask(&framed, AircraftId::Tornado)[250 * 1280 + 640],
            0
        );
        // Wrong picture size fails closed.
        assert!(
            super::mask(&transparent(640, 245), AircraftId::Tornado)
                .iter()
                .all(|v| *v == 0)
        );
    }

    /// Retail-reading (skips without an install): the measured polygon on the
    /// real `~EF2H.PIC`. The art closes the glass itself (frame lines and two
    /// support cables), so the mask must sit inside the flood-bounded region
    /// of transparent pixels around the centre, cover almost all of it, never
    /// cover an opaque pixel, and be one connected glass.
    #[test]
    fn the_tornado_glass_matches_the_imported_eurofighter_art() {
        let root = std::env::var_os("TORE_GAME_DIR").map_or_else(
            || {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../gameassets/fighters-anthology")
            },
            std::path::PathBuf::from,
        );
        let Ok(media) = tore_import::MediaSource::detect(&root) else {
            eprintln!("skipped: no retail install at {}", root.display());
            return;
        };
        let Ok(library) = media.archive("FA_1.LIB") else {
            eprintln!("skipped: no retail FA_1.LIB");
            return;
        };
        let name = AircraftId::Tornado.cockpit();
        assert_eq!(name, "~EF2H.PIC");
        let pic = tore_formats::Pic::parse(&library.read(name).unwrap()).unwrap();
        assert_eq!((pic.width, pic.height), (1280, 490));
        let source = Sprite {
            width: pic.width,
            height: pic.height,
            rgba: pic.rgba(&[[0; 3]; 256]),
            glyphs: vec![],
        };
        let mask = mask(&source, AircraftId::Tornado);
        let (width, height) = (source.width, source.height);
        // Flood the transparent pixels from the glass centre: the frame art
        // closes this region, so it is the whole glass.
        let flood = |start: usize, allowed: &dyn Fn(usize) -> bool| -> Vec<bool> {
            let mut seen = vec![false; width * height];
            let mut todo = vec![start];
            while let Some(at) = todo.pop() {
                if seen[at] || !allowed(at) {
                    continue;
                }
                seen[at] = true;
                let x = at % width;
                if x > 0 {
                    todo.push(at - 1);
                }
                if x + 1 < width {
                    todo.push(at + 1);
                }
                if at >= width {
                    todo.push(at - width);
                }
                if at + width < width * height {
                    todo.push(at + width);
                }
            }
            seen
        };
        let centre = 250 * width + 640;
        let interior = flood(centre, &|at| source.rgba[at * 4 + 3] == 0);
        assert_eq!(
            interior.iter().filter(|v| **v).count(),
            TORNADO_FRAME_INTERIOR,
            "the combiner frame no longer closes the glass"
        );
        let covered = mask.iter().filter(|v| **v == 1).count();
        assert_eq!(covered, TORNADO_GLASS_PIXELS, "glass area moved");
        for (at, covered_pixel) in mask.iter().enumerate() {
            if *covered_pixel == 1 {
                assert_eq!(source.rgba[at * 4 + 3], 0, "mask covers opaque art");
                assert!(interior[at], "mask leaves the frame at {at}");
            }
        }
        // Almost all of the glass is drawn (the polygon chords lose about 1.2%).
        assert!(covered * 100 >= TORNADO_FRAME_INTERIOR * 98);
        let connected = flood(centre, &|at| mask[at] == 1);
        assert_eq!(
            connected.iter().filter(|v| **v).count(),
            covered,
            "the glass is one connected region"
        );
    }
}
