//! F15E-7: the F-15E flies the HUD and cockpit its record names, `av8.HUD`, the
//! same as TORE's F-15C (John, 2026-11-10, opinionated), and its sensors are the
//! F-15C's reviewed radar, jammer and gun records plus the record's internal
//! FLIR and fixed laser-designator row. This reads `F15E.PT`, `F15.PT`,
//! `AV8.HUD` and the Harrier cockpit family from the user's install and pins
//! that binding: the record's five fixed sensor rows, the HUD module the identity
//! names, the instrument window frame that HUD references, and the cockpit art
//! with its overlays. It skips quietly without a retail install and reads data
//! only; no retail bytes are committed. Evidence: docs/formats/aircraft-variety.md,
//! "F-15E sensors, HUD and cockpit (F15E-7, 2026-11-10)".
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
fn the_f15e_flies_the_f15c_hud_and_cockpit_by_name() {
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
    let sensors = |id: AircraftId| -> Vec<String> {
        Aircraft::parse(&read(id.pt()))
            .unwrap()
            .hardpoints
            .iter()
            .filter_map(|h| h.store.clone())
            .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
            .collect()
    };
    let id = AircraftId::F15E;
    // The record's five fixed sensor rows: visual, the F-15C's radar and
    // jammer, the internal FLIR and the fixed Pave Spike row.
    assert_eq!(
        sensors(id),
        [
            "VIS340.SEE",
            "F15R.SEE",
            "F15.ECM",
            "AV8IR.SEE",
            "PAVESPK.SEE"
        ]
    );
    // The F-15C's rows are the first three, so radar, RWR and ECM read the
    // same records.
    assert_eq!(
        sensors(AircraftId::F15),
        ["VIS340.SEE", "F15R.SEE", "F15.ECM"]
    );
    // The identity binds the F-15C's HUD and cockpit family by name; the F-15C
    // record itself is not aliased.
    assert_eq!(id.hud(), "AV8.HUD");
    assert_eq!(id.hud(), AircraftId::F15.hud());
    assert_ne!(id.pt(), AircraftId::F15.pt());
    assert_eq!(id.cockpit_stem(), "AV8");
    assert_eq!(id.cockpit_stem(), AircraftId::F15.cockpit_stem());
    assert_eq!(id.cockpit(), "~AV8H.PIC");
    assert_eq!(id.instrument_panel(), "~AV8_P.PIC");
    // The record's own eye offset (x 0, y 20, z 60) is the F-15C's; TORE reads
    // no per-aircraft eye (F15E-7), so it is recorded, not used.
    let eye = |id: AircraftId| -> Vec<String> {
        let record = Aircraft::parse(&read(id.pt())).unwrap();
        ["viewOffset.x", "viewOffset.y", "viewOffset.z"]
            .iter()
            .map(|key| record.object[*key].value.clone())
            .collect()
    };
    assert_eq!(eye(id), eye(AircraftId::F15));
    // `av8.HUD` references exactly the instrument window frame the identity
    // names (what `Airframe::load` requires of every flyable identity).
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(hud.panel_resource().as_deref(), Some("~AV8_P.PIC"));
    // The art, the instrument frame and every reviewed cockpit overlay exist.
    assert!(has(id.cockpit()), "{}", id.cockpit());
    assert!(has(&id.instrument_panel()));
    for overlay in id.cockpit_overlays().into_iter().flatten() {
        assert!(has(&overlay), "{overlay}");
    }
    // No F-15 or Strike Eagle cockpit art exists in either archive (F15E-0).
    assert!(
        ![&fa1, &fa2]
            .iter()
            .flat_map(|library| library.entries.keys())
            .any(|name| name.starts_with("~F15")),
        "a ~F15 cockpit resource appeared"
    );
}
