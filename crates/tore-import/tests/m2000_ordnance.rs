//! M2K-6: the Mirage 2000's default load and countermeasures are the retail
//! record's own. This reads `M2000.PT` and `F18.ECM` from the user's install
//! and pins what the shared loading rules start from: the DEFA gun at 250
//! rounds, the centreline `F150.GAS`, four `AS15.JT` on the belly row, two
//! `R530.JT` and two `R550.JT` on the wing rows, no wingtip row after TORE
//! retires it (John, 2026-11-10), and the jammer's 30 chaff and 30 flares.
//! It skips quietly without a retail install and reads data only; no retail
//! bytes are committed. Evidence: docs/formats/aircraft-ordnance.md, "The
//! Mirage 2000".
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
fn the_mirage_2000s_default_load_and_countermeasures_are_the_records_own() {
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
    let aircraft = Aircraft::parse(&read(AircraftId::M2000.pt())).unwrap();
    // Eight rows: the three fixed sensors, the DEFA gun and the four editable
    // carriers; the Rafale template's wingtip row is retired.
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
            ("F18.ECM", 1),
            ("DEFA.JT", 250),
            ("F150.GAS", 1),
            ("AS15.JT", 4),
            ("R530.JT", 2),
            ("R550.JT", 2),
        ]
    );
    // The real 2000C carries two DEFA 554 with 125 rounds each; the record's
    // one gun row holds their 250 together.
    let ecm = Countermeasures::parse("F18.ECM", &read("F18.ECM")).unwrap();
    assert_eq!((ecm.chaff[0], ecm.flare[0]), (30, 30));
}
