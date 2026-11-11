//! TOR-7: the Tornado flies the Eurofighter 2000's cockpit family by name, as
//! `TOR.PT` does (`e2000.HUD`; John, 2026-11-10, opinionated), and keeps its
//! retail sensors (decision 22: radar `F18R.SEE`, jammer `MIG31.ECM` and the
//! Pave Spike row, no pod body). This reads `TOR.PT`, `E2000.HUD` and the
//! `~EF2*` pictures from the user's install and pins the binding: the four
//! fixed sensor rows, the HUD module the identity names, the instrument window
//! frame that HUD references, every picture of the family the cockpit draws
//! and the absence of any Tornado cockpit art. It skips quietly without a
//! retail install and reads data only; no retail bytes are committed.
//! Evidence: docs/formats/aircraft-variety.md, "Tornado sensors, HUD and
//! cockpit (TOR-7, 2026-11-11)".
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
fn the_tornado_flies_the_eurofighter_hud_and_cockpit_by_name() {
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
    let id = AircraftId::Tornado;
    let sensors = Aircraft::parse(&read(id.pt()))
        .unwrap()
        .hardpoints
        .iter()
        .filter_map(|h| h.store.clone())
        .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
        .collect::<Vec<_>>();
    // The fixed rows stay as retail (decision 22): the visual sensor, the
    // F/A-18's radar, the MiG-31's jammer and the Pave Spike row.
    assert_eq!(
        sensors,
        ["VIS240.SEE", "F18R.SEE", "MIG31.ECM", "PAVESPK.SEE"]
    );
    // The identity binds the Eurofighter 2000's HUD and cockpit family by
    // name; the Eurofighter record (`E2000.PT`) is not aliased.
    assert_eq!(id.hud(), "E2000.HUD");
    assert_ne!(id.hud(), AircraftId::Rafale.hud());
    assert_eq!(id.cockpit_stem(), "EF2");
    assert_eq!(id.stem(), "TOR");
    assert_eq!(id.cockpit(), "~EF2H.PIC");
    assert_eq!(id.instrument_panel(), "~EF2_P.PIC");
    // No other flyable identity uses the family: the Tornado is the first.
    for other in AircraftId::ALL {
        if other != id {
            assert_ne!(other.cockpit_stem(), "EF2", "{other:?}");
            assert_ne!(other.hud(), "E2000.HUD", "{other:?}");
        }
    }
    // `E2000.HUD` references exactly the instrument window frame the identity
    // names (what `Airframe::load` requires of every flyable identity).
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(hud.panel_resource().as_deref(), Some("~EF2_P.PIC"));
    // The art, the instrument frame and every reviewed cockpit overlay exist.
    assert!(has(id.cockpit()), "{}", id.cockpit());
    assert!(has(&id.instrument_panel()));
    let overlays: Vec<_> = id.cockpit_overlays().into_iter().flatten().collect();
    assert_eq!(overlays, ["~EF2_LH.PIC", "~EF2_CH.PIC", "~EF2_RH.PIC"]);
    for overlay in &overlays {
        assert!(has(overlay), "{overlay}");
    }
    // The whole `~EF2*` family is the Eurofighter's thirteen pictures (TOR-0).
    let family = |library: &tore_formats::Archive| -> Vec<String> {
        library
            .entries
            .keys()
            .filter(|name| name.starts_with("~EF2"))
            .cloned()
            .collect()
    };
    let mut names: Vec<_> = family(&fa1).into_iter().chain(family(&fa2)).collect();
    names.sort();
    assert_eq!(names.len(), 13, "{names:?}");
    // No Tornado cockpit art exists in either archive (TOR-0).
    assert!(
        ![&fa1, &fa2]
            .iter()
            .flat_map(|library| library.entries.keys())
            .any(|name| name.starts_with("~TOR")),
        "a ~TOR cockpit resource appeared"
    );
    // The record's own eye offset is a record field that no host code reads
    // (the shared pilot eye is the only eye): pinned so a later change shows.
    let record = Aircraft::parse(&read(id.pt())).unwrap();
    let eye: Vec<_> = ["viewOffset.x", "viewOffset.y", "viewOffset.z"]
        .iter()
        .map(|key| record.object[*key].value.clone())
        .collect();
    assert_eq!(eye, ["0", "20", "60"]);
}
