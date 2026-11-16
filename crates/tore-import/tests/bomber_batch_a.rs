//! BMB-A1: the bomber batch A identities read the retail records as they are.
//! Pins the three records' decoded station tables (every row, in order), the
//! type sizes the parser accepts, the shape layouts the interim rigs expect and
//! the two state-guard opcodes `B2.SH` needs (`jl` 0x7c and `jg` 0x7f on its
//! signed drag-rudder word). It skips quietly without a retail install and
//! reads data only; no retail bytes are committed. Evidence:
//! docs/formats/aircraft-variety.md, the three 2026-11-15 source reviews and
//! "Bomber batch A identities (BMB-A1, 2026-11-15)".
use std::path::PathBuf;
use tore_formats::{
    aircraft::{Aircraft, AircraftId},
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

/// Each record parses as its own identity (the `typeSize` arms 612, 660 and
/// 588) and its decoded rows are the retail ones, in order, with the BMB-A2
/// station corrections: the B-52's 1,242 gun rounds and the F-117's empty
/// jammer row.
#[test]
fn the_three_records_parse_as_their_own_identities_with_their_retail_rows() {
    let Some(fa2) = fa2() else { return };
    let expect: [(AircraftId, &[(&str, i32)]); 3] = [
        (
            AircraftId::B52,
            &[
                ("VIS300.SEE", 1),
                ("B52R.SEE", 1),
                ("B52.ECM", 1),
                // B52-5 (2026-11-15): the three bay rows (3, 5, 6) hold nine and
                // start empty; row 7 is the appended underwing pylon pair with the
                // Harpoons (capacity 18).
                ("", 9),
                ("M61.JT", 1242),
                ("", 9),
                ("", 9),
                ("AGM84E.JT", 18),
            ],
        ),
        (
            AircraftId::B2,
            &[
                ("VIS340.SEE", 1),
                ("F18R.SEE", 1),
                ("F18.ECM", 1),
                ("PAVESPK.SEE", 1),
                ("AV8IR.SEE", 1),
                ("MK82.JT", 32),
                ("MK82.JT", 32),
                ("PAVEWA3.JT", 4),
                ("PAVEWA3.JT", 4),
            ],
        ),
        (
            AircraftId::F117,
            &[
                ("VIS340.SEE", 1),
                ("PAVESPK.SEE", 1),
                ("", 1),
                ("AV8IR.SEE", 1),
                ("PAVEWA3.JT", 1),
                ("PAVEWA3.JT", 1),
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

/// The shapes load with the layouts the interim rigs pin, and `B2.SH`, which
/// the reader refused before BMB-A1, loads. Its signed drag-rudder word 6498
/// picks the right (+1) or left (-1) rudder pair; 2 or more removes the right
/// pair and adds nothing.
#[test]
fn the_bomber_shapes_load_and_b2_follows_its_signed_rudder_word() {
    let Some(fa2) = fa2() else { return };
    for (name, faces, words) in [
        ("B52.SH", 271, vec![0x7a20, 0x7a26, 0x7a2c, 0x7a32, 0x7a38]),
        ("B2.SH", 268, vec![0x6486, 0x648c, 0x6498]),
        ("F117.SH", 207, vec![0x4c56, 0x4c5c]),
    ] {
        let shape = Shape::parse(&fa2.read(name).unwrap())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(shape.faces.len(), faces, "{name}");
        assert_eq!(
            shape.state_words.iter().copied().collect::<Vec<_>>(),
            words,
            "{name}"
        );
    }
    let b2 = fa2.read("B2.SH").unwrap();
    let with = |value: i32| {
        Shape::with_state(&b2, &[(0x6498, value)].into())
            .unwrap()
            .faces
            .len()
    };
    assert_eq!(with(0), 268);
    // +1 and -1 each swap one closed pair for six faces; 2 removes a pair.
    assert_eq!(with(1), 272);
    assert_eq!(with(-1), 272);
    assert_eq!(with(2), 266);
    // The gear word adds the review's 14 plates.
    assert_eq!(
        Shape::with_state(&b2, &[(0x648c, 1)].into())
            .unwrap()
            .faces
            .len(),
        268 + 14
    );
}
