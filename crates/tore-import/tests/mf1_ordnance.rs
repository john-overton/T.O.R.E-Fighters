//! MF1-6: the Mirage F1's default load and countermeasures start from the
//! retail record. This reads `MF1.PT` and `AV8.ECM` from the user's install and
//! pins what the shared loading rules start from: the DEFA gun at 250 rounds,
//! the centreline row with no default pod (the record's `ALQ167.ECM` cleared by
//! TORE, John 2026-11-10), two `F250.GAS` on the belly row (the record's four,
//! capped by TORE, John 2026-11-11), two `R550.JT` on the wing row and two on
//! the wingtip row, and the jammer record's 30 chaff and 30 flares (O4: kept,
//! John 2026-11-11). It
//! skips quietly without a retail install and reads data only; no retail bytes
//! are committed. Evidence: docs/formats/aircraft-ordnance.md, "The Mirage F1".
use std::path::PathBuf;
use tore_formats::{
    aircraft::{Aircraft, AircraftId},
    weapons::Countermeasures,
};
use tore_import::MediaSource;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

#[test]
fn the_mirage_f1s_default_load_and_countermeasures_start_from_the_records_own() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let (Ok(fa1), Ok(fa2)) = (media.archive("FA_1.LIB"), media.archive("FA_2.LIB")) else {
        eprintln!("skipped: no retail FA_1.LIB and FA_2.LIB");
        return;
    };
    let read = |name: &str| {
        [&fa1, &fa2]
            .iter()
            .find(|library| library.entries.contains_key(name))
            .unwrap_or_else(|| panic!("{name} is in neither archive"))
            .read(name)
            .unwrap()
    };
    let aircraft = Aircraft::parse(&read(AircraftId::Mf1.pt())).unwrap();
    // Eight rows: the three fixed sensors, the DEFA gun, the centreline row and
    // the three editable carriers (belly, wing, wingtip). `Aircraft::parse`
    // applies TORE's two default changes to the retail rows: the centreline's
    // `ALQ167.ECM` default is cleared (the row stays, B5-2) and the belly row
    // holds two `F250.GAS` where the record has four (MF1-5b).
    let rows: Vec<_> = aircraft
        .hardpoints
        .iter()
        .map(|h| (h.store.as_deref().unwrap_or(""), h.count))
        .collect();
    assert_eq!(
        rows,
        [
            ("VIS340.SEE", 1),
            ("F18R.SEE", 1),
            ("AV8.ECM", 1),
            ("DEFA.JT", 250),
            ("", 1),
            ("F250.GAS", 2),
            ("R550.JT", 2),
            ("R550.JT", 2),
        ]
    );
    // The jammer record: chaff and flares are its own capacity, read once.
    let ecm = Countermeasures::parse("AV8.ECM", &read("AV8.ECM")).unwrap();
    assert_eq!((ecm.chaff[0], ecm.flare[0]), (30, 30));
}
