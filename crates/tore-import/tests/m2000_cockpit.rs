//! M2K-7: the Mirage 2000 flies the Rafale's HUD and cockpit by name, as its
//! record does (John, 2026-11-09, opinionated), and its sensors are the F/A-18's
//! reviewed records. This reads `M2000.PT`, `RAFALE.HUD` and the Rafale cockpit
//! family from the user's install and pins that binding: the record's three
//! fixed sensor rows, the HUD module the identity names, the instrument window
//! frame that HUD references, and the cockpit art with its overlays. It skips
//! quietly without a retail install and reads data only; no retail bytes are
//! committed. Evidence: docs/formats/aircraft-variety.md, "Mirage 2000 sensors,
//! HUD and cockpit (M2K-7, 2026-11-10)".
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
fn the_mirage_2000_flies_the_rafale_hud_and_cockpit_by_name() {
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
    let id = AircraftId::M2000;
    let aircraft = Aircraft::parse(&read(id.pt())).unwrap();
    // The record's fixed sensor rows are the F/A-18's reviewed records.
    let sensors: Vec<_> = aircraft
        .hardpoints
        .iter()
        .filter_map(|h| h.store.as_deref())
        .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
        .collect();
    assert_eq!(sensors, ["VIS340.SEE", "F18R.SEE", "F18.ECM"]);
    // The identity binds the Rafale's HUD and cockpit family by name; the
    // Rafale record itself is not aliased.
    assert_eq!(id.hud(), "RAFALE.HUD");
    assert_eq!(id.hud(), AircraftId::Rafale.hud());
    assert_ne!(id.pt(), AircraftId::Rafale.pt());
    assert_eq!(id.cockpit_stem(), "RAF");
    assert_eq!(id.cockpit(), "~RAFH.PIC");
    assert_eq!(id.instrument_panel(), "~RAF_P.PIC");
    // The HUD module references exactly that instrument window frame, which
    // is what `Airframe::load` requires of every flyable identity.
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(hud.panel_resource().as_deref(), Some("~RAF_P.PIC"));
    // The art, the instrument frame and every reviewed cockpit overlay exist.
    assert!(has(id.cockpit()), "{}", id.cockpit());
    assert!(has(&id.instrument_panel()));
    for overlay in id.cockpit_overlays().into_iter().flatten() {
        assert!(has(&overlay), "{overlay}");
    }
    // No Mirage-specific cockpit art exists in either archive (M2K-0).
    assert!(
        ![&fa1, &fa2]
            .iter()
            .flat_map(|library| library.entries.keys())
            .any(|name| name.starts_with("~M20")),
        "a ~M20 cockpit resource appeared"
    );
}
