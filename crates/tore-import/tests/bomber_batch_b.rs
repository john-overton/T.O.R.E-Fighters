//! BMB-B1 and BMB-B2: the bomber batch B identities read the retail records.
//! Pins the three records' decoded station tables (every row, in order, with
//! the BMB-B2 station corrections), the type sizes the parser accepts, the
//! corrected record values and the shape layouts the interim rigs expect. It
//! skips quietly without a retail install and reads data only; no retail
//! bytes are committed. Evidence: docs/formats/aircraft-variety.md, the three
//! batch B source reviews and "Bomber batch B identities (BMB-B1,
//! 2026-11-15)".
use std::path::PathBuf;
use tore_formats::{
    aircraft::{Aircraft, AircraftId, stations},
    shape::Shape,
};
use tore_import::MediaSource;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

fn fa2() -> Option<tore_formats::Archive> {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return None;
    };
    let archive = media.archive("FA_2.LIB").ok();
    if archive.is_none() {
        eprintln!("skipped: no retail FA_2.LIB");
    }
    archive
}

fn rows(aircraft: &Aircraft) -> Vec<(String, i32)> {
    aircraft
        .hardpoints
        .iter()
        .map(|h| (h.store.clone().unwrap_or_default(), h.count))
        .collect()
}

/// Each record parses as its own identity (the `typeSize` arms 636, 588 and
/// 660) and its decoded rows are the retail ones, in order.
#[test]
fn the_three_records_parse_as_their_own_identities_with_their_retail_rows() {
    let Some(fa2) = fa2() else { return };
    let expect: [(AircraftId, &[(&str, i32)]); 3] = [
        (
            AircraftId::B1b,
            &[
                ("VIS340.SEE", 1),
                ("F18R.SEE", 1),
                ("F18.ECM", 1),
                ("PAVESPK.SEE", 1),
                ("AV8IR.SEE", 1),
                ("MK82AIR.JT", 4),
                ("MK82AIR.JT", 4),
                ("MK82AIR.JT", 4),
            ],
        ),
        (
            AircraftId::Tu160,
            &[
                ("VIS300.SEE", 1),
                ("TU160R.SEE", 1),
                ("TU160.ECM", 1),
                ("SU37L.SEE", 1),
                ("AS16.JT", 6),
                ("AS16.JT", 6),
            ],
        ),
        (
            AircraftId::F111,
            &[
                ("VIS240.SEE", 1),
                ("A6R.SEE", 1),
                ("A6.ECM", 1),
                ("AV8IR.SEE", 1),
                ("PAVESPK.SEE", 1),
                ("GBU28.JT", 2),
                ("MK82P.JT", 2),
                ("AGM65G.JT", 4),
                ("MK82P.JT", 2),
            ],
        ),
    ];
    for (id, table) in expect {
        let aircraft = Aircraft::parse(&fa2.read(id.pt()).unwrap())
            .unwrap_or_else(|error| panic!("{}: {error}", id.pt()));
        assert_eq!(aircraft.id, id);
        let found = rows(&aircraft);
        let found: Vec<(&str, i32)> = found.iter().map(|(s, n)| (s.as_str(), *n)).collect();
        assert_eq!(found, table, "{} station table", id.pt());
    }
}

/// The BMB-B2 corrections reach the decoded records: weights, fuel, thrust,
/// the F-111F's flows and the station flags and classes.
#[test]
fn the_corrected_records_carry_the_real_figures_and_station_flags() {
    let Some(fa2) = fa2() else { return };
    let read = |id: AircraftId| Aircraft::parse(&fa2.read(id.pt()).unwrap()).unwrap();
    let n = |a: &Aircraft, key: &str| a.number(key) as i64;

    let b1b = read(AircraftId::B1b);
    assert_eq!(n(&b1b, "maxTakeoffWeight"), 477_000);
    assert_eq!(n(&b1b, "internalFuel"), 265_274);
    assert_eq!((n(&b1b, "thrust"), n(&b1b, "aftThrust")), (69_560, 123_120));
    assert_eq!(n(&b1b, "weight"), 192_000);
    let top = |a: &Aircraft, g: i32| {
        a.envelopes
            .iter()
            .find(|e| e.g == g)
            .unwrap()
            .points
            .iter()
            .map(|p| p[0])
            .fold(0., f64::max)
    };
    // Mach 1.25 above the tropopause: 1,210 ft/s on the 1 G row.
    assert_eq!(top(&b1b, 1), 1_210.);
    assert!(b1b.hardpoints[5..8].iter().all(|h| h.flags == 0x1100));

    let tu160 = read(AircraftId::Tu160);
    assert_eq!(n(&tu160, "weight"), 242_500);
    assert_eq!(n(&tu160, "internalFuel"), 326_285);
    assert_eq!(
        (n(&tu160, "thrust"), n(&tu160, "aftThrust")),
        (123_600, 220_400)
    );
    assert_eq!(n(&tu160, "maxTakeoffWeight"), 606_260);
    // The bay rows never carried the designator bit.
    assert!(tu160.hardpoints[4..6].iter().all(|h| h.flags == 0x1184));

    let f111 = read(AircraftId::F111);
    assert_eq!(n(&f111, "internalFuel"), 32_663);
    assert_eq!(
        (n(&f111, "thrust"), n(&f111, "aftThrust")),
        (29_120, 50_200)
    );
    assert_eq!(
        (n(&f111, "fuelConsumption"), n(&f111, "aftFuelConsumption")),
        (5, 35)
    );
    assert!(f111.hardpoints[5..9].iter().all(|h| h.flags == 0x3e5));
    assert!(f111.hardpoints[5..9].iter().all(|h| h.weight_class == 50));
    assert_eq!(stations::F111_WING_CLASS, 50);
    // Empty plus full fuel fits under the maximum on all three: none needs the
    // loadout's fuel cap.
    for a in [&b1b, &tu160, &f111] {
        assert!(n(a, "weight") + n(a, "internalFuel") < n(a, "maxTakeoffWeight"));
    }
}

/// The shapes load with the layouts the interim rigs pin (CODE size is the rig
/// loader's; faces and words here).
#[test]
fn the_batch_b_shapes_load_with_their_reviewed_layouts() {
    let Some(fa2) = fa2() else { return };
    for (name, faces, words, gear, plates) in [
        (
            "B1.SH",
            314,
            vec![0x7b10, 0x7b16, 0x7b1c, 0x7b22, 0x7b2e, 0x7b34, 0x7b3a],
            0x7b22,
            12,
        ),
        (
            "TU160.SH",
            316,
            vec![0x7ff0, 0x7ffc, 0x8002, 0x800e, 0x8014, 0x801a],
            0x8002,
            12,
        ),
        (
            "F111.SH",
            254,
            vec![0x7240, 0x7246, 0x724c, 0x7258, 0x725e, 0x7264],
            0x724c,
            22,
        ),
    ] {
        let bytes = fa2.read(name).unwrap();
        let shape = Shape::parse(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(shape.faces.len(), faces, "{name}");
        assert_eq!(
            shape.state_words.iter().copied().collect::<Vec<_>>(),
            words,
            "{name}"
        );
        // The gear word adds the reviews' plates.
        assert_eq!(
            Shape::with_state(&bytes, &[(gear, 1)].into())
                .unwrap()
                .faces
                .len(),
            faces + plates,
            "{name} gear"
        );
    }
}
