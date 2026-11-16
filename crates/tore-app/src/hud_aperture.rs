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
        // Mirage F1: rafale.HUD is the decided binding (opinionated, John,
        // 2026-11-10, MF1-7), no longer interim; "RAF" above is its glass.
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
        // The Su-34's su33cc.HUD is the Su-33 family's, the same module and the
        // same ~SU33H.PIC the MiG-29, Su-25 and MiG-23 fly, so it takes the
        // glass already measured for them (BMB-C1, 2026-11-15; coordinator
        // first pass, Su-34 item 7). Measured on the imported art in SU34-7
        // (2026-11-16, the retail-reading test of the Su-34's glass in this
        // file's tests): 66,681 pixels, no opaque pixel, 97.5 percent of the
        // 68,414 pixel transparent region the combiner frame closes. The
        // pilot eye is `mirrors::eye_offset`'s.
        AircraftId::Mig29 | AircraftId::Su25 | AircraftId::Mig23 | AircraftId::Su34 => (
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
        | AircraftId::A310
        // The B-52 flies the AC-130 panel family its BMBROH.HUD names
        // (BMB-A1, 2026-11-15; John, decision 14): the same ~AC130H.PIC.
        | AircraftId::B52
        // The F-111F flies the same ~AC130H.PIC its AC130.HUD names (BMB-B1,
        // 2026-11-15; coordinator first pass, F111 item 8): the whole picture,
        // as the B-52.
        | AircraftId::F111
        // The Tu-26 and Tu-95 fly the same ~AC130H.PIC their bmbroh.HUD names
        // (BMB-C1, 2026-11-15; coordinator first pass, Tu-26 item 10 and
        // Tu-95 item 10): the whole picture, as the B-52.
        | AircraftId::Tu26
        | AircraftId::Tu95 => ((1280, 490), &[(0, 0), (1279, 0), (1279, 489), (0, 489)]),
        // F-117 (F117-7, 2026-11-15): `F117.HUD` and the cockpit art
        // `~F117H.PIC` (1673 x 508, John decision 28, opinionated). BMB-A1
        // left this arm failing closed. The art has no combiner: its
        // transparent area is three panes, a left and a right one (about 189,000
        // pixels each, divided from the middle by the two canopy bow posts) and
        // the windscreen between the posts. A thin bar closes the windscreen at
        // y 128 and an open sky strip lies above it (x 735 to 940, its own
        // pane); the console's vent block and two teeth break the windscreen's
        // lower edge from y 346. The HUD is the windscreen's: the polygon runs
        // one pixel outside it (the bar's lower edge at y 128, the bow posts'
        // inner edges, x 711 to 725 on the left and 949 to 963 on the right,
        // widening from y 141 to 224 and narrowing again to y 308), and its
        // foot at y 367 lets the opaque art trim the console. It covers 56,562 of
        // the windscreen's 56,572 pixels. Measured on the imported art,
        // `the_f117_glass_is_the_windscreen_of_the_imported_art`. Fitted.
        AircraftId::F117 => (
            (1673, 508),
            &[
                (724, 128),
                (950, 128),
                (950, 141),
                (963, 224),
                (963, 262),
                (962, 272),
                (962, 300),
                (961, 308),
                (961, 367),
                (713, 367),
                (713, 308),
                (712, 300),
                (712, 272),
                (711, 262),
                (711, 224),
                (724, 141),
            ],
        ),
        // B-2 (B2-7, 2026-11-15): `B2.HUD` and the cockpit art `~B2H.PIC`
        // (1394 x 508, John decision 17, opinionated). The art has no
        // combiner: its transparent area is three panes divided by the left
        // diagonal strut and the right strut (the main window x 141 to 1021,
        // y 91 to 455, 247,572 pixels; a left side pane x 0 to 322 and a right
        // one x 1087 to 1393). The HUD is the main window's: the polygon runs
        // along its boundary (the top beam's lower edge at y 90, the right
        // strut's funnel and the monitor housing, the sloping console top and
        // the left strut), a pixel outside it, so the opaque art trims it and
        // the two side panes stay out. Measured on the imported art,
        // `the_b2_glass_is_the_main_window_of_the_imported_art`. Fitted.
        // The B-1B (bmbrcc.HUD) and the Tu-160 (b2.HUD) fly the B-2's cockpit
        // picture `~B2H.PIC` by the record (BMB-B1, 2026-11-15; coordinator
        // first pass, B1B item 7 and Tu-160 item 8), so they take the glass
        // measured for it (B2-7). Fitted for the B-2; the same art, so the
        // same polygon.
        AircraftId::B2 | AircraftId::B1b | AircraftId::Tu160 => (
            (1394, 508),
            &[
                (398, 90),
                (956, 90),
                (980, 155),
                (1006, 187),
                (1021, 299),
                (1020, 331),
                (1006, 347),
                (1006, 420),
                (540, 420),
                (460, 424),
                (420, 431),
                (380, 445),
                (340, 454),
                (300, 456),
                (140, 456),
                (144, 443),
                (175, 395),
                (217, 331),
                (273, 251),
                (334, 171),
            ],
        ),
        // F-5E (F5E-7, 2026-11-10): `F5E.PT` names f29.HUD, so it flies the
        // X-29's cockpit art `~F29H.PIC` by name (John, 2026-11-10,
        // opinionated). The glass is the opening between the two canopy
        // struts (inner edges at x 522 and 759), under the thin combiner arc
        // (its underside runs from y 132 at the struts to y 107 at the centre)
        // and above the coaming (y 372); the transparency test drops the
        // struts, the arc line and the coaming bumps. Measured on the imported
        // art, `the_f5e_glass_matches_the_imported_x29_art`.
        AircraftId::F5E => (
            (1280, 490),
            &[
                (522, 132),
                (535, 127),
                (550, 120),
                (565, 116),
                (580, 112),
                (610, 108),
                (640, 107),
                (670, 108),
                (700, 112),
                (715, 116),
                (730, 120),
                (745, 127),
                (759, 132),
                (759, 372),
                (522, 372),
            ],
        ),
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
        AircraftId::Tornado | AircraftId::TornadoClean => (
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
        // round): the Tornado (~EF2H.PIC) names its own cockpit picture, whose
        // glass has not been measured. No expected size, so the aperture fails
        // closed (an empty mask) until its cockpit slice measures a polygon.
        // Update (GRI-7, 2026-11-10): the Gripen's glass is measured below.
        // Update (F5E-7, 2026-11-10): the F-5E's glass is measured above.
        // Update (TOR-7, 2026-11-11): the Tornado's glass is measured above.
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

    /// Transparent pixels inside the F-5E polygon on the retail `~F29H.PIC`.
    const F5E_GLASS_PIXELS: usize = 58_556;
    /// Pixels inside the Su-33 family polygon on the retail `~SU33H.PIC`, and
    /// the transparent region the combiner frame closes there (SU34-7).
    const SU33_GLASS_PIXELS: usize = 66_681;
    const SU33_GLASS_REGION: usize = 68_414;

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

    #[test]
    fn the_f5e_draws_the_x29_glass_between_the_canopy_struts() {
        // Synthetic art: all transparent, so the mask is the polygon itself.
        let source = transparent(1280, 490);
        let mask = mask(&source, AircraftId::F5E);
        assert_eq!(mask.len(), 1280 * 490);
        // Centre of the glass is in; above the combiner arc, outside the
        // struts and below the coaming are out.
        assert_eq!(mask[240 * 1280 + 640], 1);
        assert_eq!(mask[100 * 1280 + 640], 0);
        assert_eq!(mask[240 * 1280 + 500], 0);
        assert_eq!(mask[240 * 1280 + 780], 0);
        assert_eq!(mask[380 * 1280 + 640], 0);
        // The Gripen's glass is measured for its own 1279-wide picture, so this
        // 1280-wide synthetic art fails closed for it. (The Tornado's glass is
        // measured too, TOR-7: it has its own test below.)
        assert!(mask.contains(&1));
        assert!(
            super::mask(&source, AircraftId::Gripen)
                .iter()
                .all(|v| *v == 0)
        );
        // Wrong picture size fails closed.
        assert!(
            super::mask(&transparent(640, 245), AircraftId::F5E)
                .iter()
                .all(|v| *v == 0)
        );
    }

    /// Retail-reading (skips without an install): the measured polygon on the
    /// real `~F29H.PIC`. The mask is the transparent opening between the canopy
    /// struts, so it never covers an opaque pixel, is one connected glass (the
    /// struts and the arc line stay out) and its area is pinned.
    #[test]
    fn the_f5e_glass_matches_the_imported_x29_art() {
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
        let name = AircraftId::F5E.cockpit();
        assert_eq!(name, "~F29H.PIC");
        let pic = tore_formats::Pic::parse(&library.read(name).unwrap()).unwrap();
        assert_eq!((pic.width, pic.height), (1280, 490));
        let rgba = pic.rgba(&[[0; 3]; 256]);
        let source = Sprite {
            width: pic.width,
            height: pic.height,
            rgba,
            glyphs: vec![],
        };
        let mask = mask(&source, AircraftId::F5E);
        let covered = mask.iter().filter(|v| **v == 1).count();
        assert_eq!(covered, F5E_GLASS_PIXELS, "glass area moved");
        let (width, height) = (source.width, source.height);
        let mut left = width;
        let mut right = 0;
        let (mut top, mut bottom) = (height, 0);
        for (at, covered_pixel) in mask.iter().enumerate() {
            if *covered_pixel == 1 {
                assert_eq!(source.rgba[at * 4 + 3], 0, "mask covers opaque art");
                let (x, y) = (at % width, at / width);
                left = left.min(x);
                right = right.max(x);
                top = top.min(y);
                bottom = bottom.max(y);
            }
        }
        // The glass spans the opening between the struts and sits under the arc.
        assert!((520..=530).contains(&left), "left {left}");
        assert!((752..=760).contains(&right), "right {right}");
        assert!((105..=112).contains(&top), "top {top}");
        assert!((365..=372).contains(&bottom), "bottom {bottom}");
        // Flood from the centre: every covered pixel is reached (one glass).
        let mut seen = vec![false; width * height];
        let mut todo = vec![240 * width + 640];
        let mut reached = 0;
        while let Some(at) = todo.pop() {
            if seen[at] || mask[at] != 1 {
                continue;
            }
            seen[at] = true;
            reached += 1;
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
        assert_eq!(reached, covered, "the glass is one connected region");
    }

    /// SU34-7: the Su-34 flies the Su-33 family's `~SU33H.PIC` (its `su33cc.HUD`
    /// names the stem `SU33`), so it draws the glass measured for the MiG-29,
    /// Su-25 and MiG-23. Retail-reading (skips without an install): the
    /// polygon never covers an opaque pixel and covers 97.5 percent of the
    /// one closed transparent region the combiner frame bounds (x 510 to 770,
    /// y 90 to 393; the rest is the polygon's chords at the corners and the
    /// console under y 388), with its area pinned.
    #[test]
    fn the_su34_draws_the_su33_art_glass_measured_on_the_imported_picture() {
        let source = transparent(1280, 490);
        assert_eq!(
            mask(&source, AircraftId::Su34),
            mask(&source, AircraftId::Mig29)
        );
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
        let name = AircraftId::Su34.cockpit();
        assert_eq!(name, "~SU33H.PIC");
        let Some(library) = ["FA_1.LIB", "FA_2.LIB"]
            .iter()
            .filter_map(|lib| media.archive(lib).ok())
            .find(|library| library.entries.contains_key(name))
        else {
            eprintln!("skipped: no retail {name}");
            return;
        };
        let pic = tore_formats::Pic::parse(&library.read(name).unwrap()).unwrap();
        assert_eq!((pic.width, pic.height), (1280, 490));
        let source = Sprite {
            width: pic.width,
            height: pic.height,
            rgba: pic.rgba(&[[0; 3]; 256]),
            glyphs: vec![],
        };
        let (width, height) = (source.width, source.height);
        let mask = mask(&source, AircraftId::Su34);
        let covered = mask.iter().filter(|v| **v == 1).count();
        assert_eq!(covered, SU33_GLASS_PIXELS, "glass area moved");
        for (at, covered_pixel) in mask.iter().enumerate() {
            if *covered_pixel == 1 {
                assert_eq!(source.rgba[at * 4 + 3], 0, "mask covers opaque art");
            }
        }
        // Flood the transparent pixels from the glass centre: the combiner
        // frame closes this region, so it is the whole glass.
        let mut interior = vec![false; width * height];
        let mut todo = vec![240 * width + 640];
        let (mut left, mut right, mut top, mut bottom) = (width, 0, height, 0);
        while let Some(at) = todo.pop() {
            if interior[at] || source.rgba[at * 4 + 3] != 0 {
                continue;
            }
            interior[at] = true;
            let (x, y) = (at % width, at / width);
            (left, right, top, bottom) = (left.min(x), right.max(x), top.min(y), bottom.max(y));
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
        let glass = interior.iter().filter(|v| **v).count();
        assert_eq!(glass, SU33_GLASS_REGION, "the combiner frame moved");
        assert_eq!((left, right, top, bottom), (510, 770, 90, 393));
        let inside = (0..mask.len())
            .filter(|&at| mask[at] == 1 && interior[at])
            .count();
        assert!(
            inside * 1000 >= glass * 970,
            "the polygon covers {inside} of the glass's {glass}"
        );
    }

    /// MF1-7: the Mirage F1 flies the Rafale's `rafale.HUD` family, so its glass
    /// is the Rafale's measured polygon (the Mirage 2000's too), not an empty one.
    #[test]
    fn the_mirage_f1_draws_the_rafale_glass() {
        let source = transparent(1280, 490);
        let mf1 = mask(&source, AircraftId::Mf1);
        assert_eq!(mf1, mask(&source, AircraftId::Rafale));
        assert_eq!(mf1, mask(&source, AircraftId::M2000));
        assert!(mf1.contains(&1));
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

    /// B2-7: the B-2's own 1394x508 picture has a measured glass polygon (the
    /// main window between the two struts); any other size fails closed.
    #[test]
    fn the_b2_glass_is_measured_for_its_1394_picture() {
        let source = transparent(1394, 508);
        let b2 = mask(&source, AircraftId::B2);
        assert_eq!(b2.len(), 1394 * 508);
        let at = |x: usize, y: usize| b2[y * 1394 + x];
        // Inside: the middle of the window, beside the HUD's centre.
        assert_eq!(at(697, 250), 1);
        assert_eq!(at(500, 380), 1);
        // Outside: above the top beam's lower edge, the left and right side
        // panes beyond the struts, the console below the sloping top.
        assert_eq!(at(697, 80), 0);
        assert_eq!(at(100, 250), 0);
        assert_eq!(at(1200, 250), 0);
        assert_eq!(at(697, 440), 0);
        // An opaque art pixel inside the polygon stays out of the glass.
        let mut framed = transparent(1394, 508);
        framed.rgba[(250 * 1394 + 697) * 4 + 3] = 255;
        assert_eq!(mask(&framed, AircraftId::B2)[250 * 1394 + 697], 0);
        // A 1280x490 picture is not the B-2's: fail closed.
        assert!(
            mask(&transparent(1280, 490), AircraftId::B2)
                .iter()
                .all(|v| *v == 0)
        );
        // The F-117's glass is keyed to its own 1673x508 picture, not this one.
        assert!(mask(&source, AircraftId::F117).iter().all(|v| *v == 0));
    }

    /// F117-7: the F-117's own 1673x508 picture has a measured glass polygon
    /// (the windscreen between the two bow posts); any other size fails
    /// closed.
    #[test]
    fn the_f117_glass_is_measured_for_its_1673_picture() {
        let source = transparent(1673, 508);
        let f117 = mask(&source, AircraftId::F117);
        assert_eq!(f117.len(), 1673 * 508);
        let at = |x: usize, y: usize| f117[y * 1673 + x];
        // Inside: the middle of the windscreen, beside the HUD's centre.
        assert_eq!(at(837, 250), 1);
        assert_eq!(at(740, 340), 1);
        // Outside: the sky strip above the bar, the left and right panes
        // beyond the bow posts, below the console foot.
        assert_eq!(at(837, 100), 0);
        assert_eq!(at(300, 250), 0);
        assert_eq!(at(1400, 250), 0);
        assert_eq!(at(837, 400), 0);
        // An opaque art pixel inside the polygon stays out of the glass.
        let mut framed = transparent(1673, 508);
        framed.rgba[(250 * 1673 + 837) * 4 + 3] = 255;
        assert_eq!(mask(&framed, AircraftId::F117)[250 * 1673 + 837], 0);
        // A 1394x508 picture is not the F-117's: fail closed.
        assert!(
            mask(&transparent(1394, 508), AircraftId::F117)
                .iter()
                .all(|v| *v == 0)
        );
    }

    /// Transparent pixels of the retail `~F117H.PIC`'s windscreen and the
    /// mask's coverage of them (F117-7).
    const F117_WINDSCREEN_PIXELS: usize = 56_572;
    const F117_GLASS_PIXELS: usize = 56_562;

    /// Retail-reading (skips without an install): the polygon on the real
    /// `~F117H.PIC` covers the windscreen almost whole, never an opaque
    /// pixel, and neither of the two side panes nor the sky strip above the
    /// bar.
    #[test]
    fn the_f117_glass_is_the_windscreen_of_the_imported_art() {
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
        let name = AircraftId::F117.cockpit();
        assert_eq!(name, "~F117H.PIC");
        let pic = tore_formats::Pic::parse(&library.read(name).unwrap()).unwrap();
        assert_eq!((pic.width, pic.height), (1673, 508));
        let source = Sprite {
            width: pic.width,
            height: pic.height,
            rgba: pic.rgba(&[[0; 3]; 256]),
            glyphs: vec![],
        };
        let mask = mask(&source, AircraftId::F117);
        let (width, height) = (source.width, source.height);
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
        let transparent_at = |at: usize| source.rgba[at * 4 + 3] == 0;
        // The transparent panes: the windscreen, the sky strip above its bar,
        // and one each side of the bow posts.
        let window = flood(250 * width + 837, &transparent_at);
        let strip = flood(60 * width + 837, &transparent_at);
        let left = flood(250 * width + 300, &transparent_at);
        let right = flood(250 * width + 1400, &transparent_at);
        assert_eq!(
            window.iter().filter(|v| **v).count(),
            F117_WINDSCREEN_PIXELS,
            "the posts and bar no longer close the windscreen"
        );
        assert!(!window[60 * width + 837] && !window[250 * width + 300]);
        assert!(!window[250 * width + 1400]);
        let covered = mask.iter().filter(|v| **v == 1).count();
        assert_eq!(covered, F117_GLASS_PIXELS, "glass area moved");
        for (at, covered_pixel) in mask.iter().enumerate() {
            if *covered_pixel == 1 {
                assert_eq!(source.rgba[at * 4 + 3], 0, "mask covers opaque art");
                assert!(window[at], "mask leaves the windscreen at {at}");
                assert!(
                    !strip[at] && !left[at] && !right[at],
                    "mask reaches another pane"
                );
            }
        }
        // Almost all of the windscreen is drawn.
        assert!(covered * 100 >= F117_WINDSCREEN_PIXELS * 98);
        let connected = flood(250 * width + 837, &|at| mask[at] == 1);
        assert_eq!(connected.iter().filter(|v| **v).count(), covered);
    }

    /// Transparent pixels of the retail `~B2H.PIC`'s main window and the mask's
    /// coverage of them (B2-7).
    const B2_MAIN_WINDOW_PIXELS: usize = 247_572;
    const B2_GLASS_PIXELS: usize = 246_986;

    /// Retail-reading (skips without an install): the polygon on the real
    /// `~B2H.PIC` covers the main window almost whole, never an opaque pixel,
    /// and neither of the two side panes.
    #[test]
    fn the_b2_glass_is_the_main_window_of_the_imported_art() {
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
        let name = AircraftId::B2.cockpit();
        assert_eq!(name, "~B2H.PIC");
        let pic = tore_formats::Pic::parse(&library.read(name).unwrap()).unwrap();
        assert_eq!((pic.width, pic.height), (1394, 508));
        let source = Sprite {
            width: pic.width,
            height: pic.height,
            rgba: pic.rgba(&[[0; 3]; 256]),
            glyphs: vec![],
        };
        let mask = mask(&source, AircraftId::B2);
        let (width, height) = (source.width, source.height);
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
        let transparent_at = |at: usize| source.rgba[at * 4 + 3] == 0;
        // The three transparent panes: the main window, and one each side of
        // the struts.
        let window = flood(250 * width + 697, &transparent_at);
        let left = flood(250 * width + 100, &transparent_at);
        let right = flood(250 * width + 1200, &transparent_at);
        assert_eq!(
            window.iter().filter(|v| **v).count(),
            B2_MAIN_WINDOW_PIXELS,
            "the struts no longer close the main window"
        );
        assert!(!window[250 * width + 100] && !window[250 * width + 1200]);
        let covered = mask.iter().filter(|v| **v == 1).count();
        assert_eq!(covered, B2_GLASS_PIXELS, "glass area moved");
        for (at, covered_pixel) in mask.iter().enumerate() {
            if *covered_pixel == 1 {
                assert_eq!(source.rgba[at * 4 + 3], 0, "mask covers opaque art");
                assert!(window[at], "mask leaves the main window at {at}");
                assert!(!left[at] && !right[at], "mask reaches a side pane");
            }
        }
        // Almost all of the window is drawn.
        assert!(covered * 100 >= B2_MAIN_WINDOW_PIXELS * 98);
        let connected = flood(250 * width + 697, &|at| mask[at] == 1);
        assert_eq!(connected.iter().filter(|v| **v).count(), covered);
    }
}
