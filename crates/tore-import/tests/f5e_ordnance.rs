//! F5E-6: the F-5E's default load and countermeasures. This reads `F5E.PT`
//! and `F104.ECM` from the user's install and pins what the shared loading
//! rules start from after TORE's guarded corrections (B5-2 and B5-3, John,
//! 2026-11-10): the `F104R.SEE` radar stand-in, the `F104.ECM` dispenser
//! record (20 chaff and 20 flares), the M61 gun at 560 rounds (two guns of
//! 280), the centreline `F150.GAS` tank, an empty wing pair and the wingtip
//! `AIM9B.JT` pair. It skips quietly without a retail install and reads data
//! only; no retail bytes are committed. Evidence:
//! docs/formats/aircraft-ordnance.md, "The F-5E Tiger".
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
fn the_f5es_default_load_and_countermeasures_are_the_corrected_records() {
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
    let aircraft = Aircraft::parse(&read(AircraftId::F5E.pt())).unwrap();
    // Seven rows: the three fixed sensor and dispenser rows, the M61 gun, the
    // centreline tank, the wing pair (empty by default, four stores allowed)
    // and the wingtip rails.
    let rows: Vec<_> = aircraft
        .hardpoints
        .iter()
        .map(|h| (h.store.as_deref().unwrap_or(""), h.count))
        .collect();
    assert_eq!(
        rows,
        [
            ("VIS340.SEE", 1),
            ("F104R.SEE", 1),
            ("F104.ECM", 1),
            ("M61.JT", 560),
            ("F150.GAS", 1),
            ("", 4),
            ("AIM9B.JT", 2),
        ]
    );
    // The real F-5E carries two M39 guns with 280 rounds each; the record's
    // one gun row holds their 560 together. The jammer row is the F-104's
    // 20 chaff and 20 flares dispenser record.
    let ecm = Countermeasures::parse("F104.ECM", &read("F104.ECM")).unwrap();
    assert_eq!((ecm.chaff[0], ecm.flare[0]), (20, 20));
}
