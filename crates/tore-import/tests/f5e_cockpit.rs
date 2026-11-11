//! F5E-7: the F-5E flies the X-29's cockpit family by name, as `F5E.PT` does
//! (`f29.HUD`; John, 2026-11-10, opinionated), and its sensors are the corrected
//! records of B5-2 and B5-3. This reads `F5E.PT`, `F29.HUD` and the `~F29*`
//! pictures from the user's install and pins the binding: the three fixed
//! sensor rows, the HUD module the identity names, the instrument window frame
//! that HUD references, every picture of the family the cockpit draws and the
//! absence of any F-5E cockpit art. It skips quietly without a retail install
//! and reads data only; no retail bytes are committed. Evidence:
//! docs/formats/aircraft-variety.md, "F-5E sensors, HUD and cockpit (F5E-7,
//! 2026-11-10)".
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
fn the_f5e_flies_the_x29_hud_and_cockpit_by_name() {
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
    let id = AircraftId::F5E;
    let sensors = |id: AircraftId| -> Vec<String> {
        Aircraft::parse(&read(id.pt()))
            .unwrap()
            .hardpoints
            .iter()
            .filter_map(|h| h.store.clone())
            .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
            .collect()
    };
    // The fixed rows: the visual sensor, the F-104's radar standing in for the
    // real AN/APQ-159 (B5-2, John's pick D1) and the F-104's dispenser record
    // with 20 chaff and 20 flares (B5-3, John's pick D2).
    assert_eq!(sensors(id), ["VIS340.SEE", "F104R.SEE", "F104.ECM"]);
    // The identity binds the X-29's HUD and cockpit family by name; the X-29
    // record (`F29.PT`) and the X-31 are not aliased.
    assert_eq!(id.hud(), "F29.HUD");
    assert_ne!(id.hud(), AircraftId::X31.hud());
    assert_eq!(id.cockpit_stem(), "F29");
    assert_eq!(id.stem(), "F5E");
    assert_eq!(id.cockpit(), "~F29H.PIC");
    assert_eq!(id.instrument_panel(), "~F29_P.PIC");
    // No other flyable identity uses the family: the F-5E is the first.
    for other in AircraftId::ALL {
        if other != id {
            assert_ne!(other.cockpit_stem(), "F29", "{other:?}");
            assert_ne!(other.hud(), "F29.HUD", "{other:?}");
        }
    }
    // `F29.HUD` references exactly the instrument window frame the identity
    // names (what `Airframe::load` requires of every flyable identity).
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(hud.panel_resource().as_deref(), Some("~F29_P.PIC"));
    // The art, the instrument frame and every reviewed cockpit overlay exist.
    assert!(has(id.cockpit()), "{}", id.cockpit());
    assert!(has(&id.instrument_panel()));
    let overlays: Vec<_> = id.cockpit_overlays().into_iter().flatten().collect();
    assert_eq!(overlays, ["~F29_LH.PIC", "~F29_CH.PIC", "~F29_RH.PIC"]);
    for overlay in &overlays {
        assert!(has(overlay), "{overlay}");
    }
    // The whole `~F29*` family is the X-29's thirteen pictures (F5E-0).
    let family = |library: &tore_formats::Archive| -> Vec<String> {
        library
            .entries
            .keys()
            .filter(|name| name.starts_with("~F29"))
            .cloned()
            .collect()
    };
    let mut names: Vec<_> = family(&fa1).into_iter().chain(family(&fa2)).collect();
    names.sort();
    assert_eq!(names.len(), 13, "{names:?}");
    // No F-5E cockpit art exists in either archive (F5E-0).
    assert!(
        ![&fa1, &fa2]
            .iter()
            .flat_map(|library| library.entries.keys())
            .any(|name| name.starts_with("~F5")),
        "a ~F5 cockpit resource appeared"
    );
    // The record's own eye offset is a record field (the F-15C's, the F/A-18's and
    // the Mirage's value) that no host code reads: it is pinned so a later
    // change shows (the shared pilot eye is the only eye).
    let record = Aircraft::parse(&read(id.pt())).unwrap();
    let eye: Vec<_> = ["viewOffset.x", "viewOffset.y", "viewOffset.z"]
        .iter()
        .map(|key| record.object[*key].value.clone())
        .collect();
    assert_eq!(eye, ["0", "20", "60"]);
}
