//! T26-7: the Tu-26 flies the AC-130 family's HUD and cockpit art by its
//! record (`bmbroh.HUD`, stem `AC130`, `~AC130H.PIC`; coordinator first pass,
//! Tu-26 item 10) and keeps the record's sensor rows: the visual sight, the
//! radar and jammer, and the built-in laser designator `SU37L.SEE` as a fixed
//! row with no pod body. This reads `TU26.PT`, the HUD module and the cockpit
//! art from the user's install and pins the bindings. It skips quietly
//! without a retail install and reads data only; no retail bytes are
//! committed. Evidence: docs/formats/aircraft-variety.md, "Tu-26 Backfire-C
//! ordnance, sensors, HUD and cockpit (T26-6/7, 2026-11-16)".
use std::path::PathBuf;
use tore_formats::{
    aircraft::{Aircraft, AircraftId},
    hud::Hud,
};
use tore_import::MediaSource;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

#[test]
fn the_tu26_flies_the_ac130_family_hud_and_keeps_its_four_sensor_rows() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let (Ok(fa1), Ok(fa2)) = (media.archive("FA_1.LIB"), media.archive("FA_2.LIB")) else {
        eprintln!("skipped: no retail FA_1.LIB and FA_2.LIB");
        return;
    };
    let has = |name: &str| {
        [&fa1, &fa2]
            .iter()
            .any(|library| library.entries.contains_key(name))
    };
    let read = |name: &str| {
        [&fa1, &fa2]
            .iter()
            .find(|library| library.entries.contains_key(name))
            .unwrap_or_else(|| panic!("{name} is in neither archive"))
            .read(name)
            .unwrap()
    };
    let id = AircraftId::Tu26;
    let record = Aircraft::parse(&read(id.pt())).unwrap();
    let sensors = record
        .hardpoints
        .iter()
        .filter_map(|h| h.store.clone())
        .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
        .collect::<Vec<_>>();
    // The record's rows, in order: the visual sight, the radar, the jammer and
    // the built-in laser designator (a fixed row at the origin, no pod body).
    assert_eq!(
        sensors,
        ["VIS300.SEE", "TU26R.SEE", "TU26.ECM", "SU37L.SEE"]
    );
    assert_eq!(id.radar(), Some("TU26R.SEE"));
    // The HUD module is the AC-130 family's by the record's own name, with
    // the AC-130's instrument frame and cockpit art.
    assert_eq!(id.hud(), "BMBROH.HUD");
    assert_eq!(id.cockpit_stem(), "AC130");
    assert_eq!(id.cockpit(), "~AC130H.PIC");
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(
        hud.panel_resource().as_deref(),
        Some(id.instrument_panel().as_str())
    );
    assert!(has(id.cockpit()), "{}", id.cockpit());
    assert!(has(&id.instrument_panel()));
    let art = tore_formats::Pic::parse(&read(id.cockpit())).unwrap();
    assert_eq!((art.width, art.height), (1280, 490));
    // The Tu-26 shares the family with the B-52 and the Tu-95 (same HUD
    // module name), and with nobody else that is not an AC-130 relative.
    assert_eq!(AircraftId::B52.hud(), id.hud());
    assert_eq!(AircraftId::Tu95.hud(), id.hud());
    // The record's own eye offset is a record field no host code reads (the
    // fitted seat in `mirrors.rs` is the only Tu-26 eye): pinned so a later
    // change shows.
    let eye: Vec<_> = ["viewOffset.x", "viewOffset.y", "viewOffset.z"]
        .iter()
        .map(|key| record.object[*key].value.clone())
        .collect();
    assert_eq!(eye, ["0", "20", "60"]);
}
