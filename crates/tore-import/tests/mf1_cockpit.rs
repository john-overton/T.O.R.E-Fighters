//! MF1-7: the Mirage F1 flies the Rafale's HUD and cockpit by name, as its
//! record does (John, 2026-11-10, decision 25, opinionated, the same binding as
//! the Mirage 2000), and its sensors are the F/A-18's reviewed radar plus the
//! AV-8B's reviewed internal countermeasure suite. This reads `MF1.PT`,
//! `RAFALE.HUD` and the Rafale cockpit family from the user's install and pins
//! that binding: the record's sensor rows, the HUD module the identity names,
//! the instrument window frame that HUD references, and the cockpit art with its
//! overlays. It skips quietly without a retail install and reads data only; no
//! retail bytes are committed. Evidence: docs/formats/aircraft-variety.md,
//! "Mirage F1 sensors, HUD and cockpit (MF1-7, 2026-11-10)".
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
fn the_mirage_f1_flies_the_rafale_hud_and_cockpit_by_name() {
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
    let id = AircraftId::Mf1;
    // The record's fixed sensor rows: visual, the F/A-18's radar and the
    // AV-8B's internal countermeasure suite, then the centreline row's default
    // jammer pod (cleared by B5-2 for play, retail here).
    let rows = sensors(id);
    assert_eq!(rows[..3], ["VIS340.SEE", "F18R.SEE", "AV8.ECM"]);
    assert!(
        rows.len() == 3 || rows[3] == "ALQ167.ECM",
        "unexpected sensor rows {rows:?}"
    );
    // The first two rows are the Mirage 2000's and the F/A-18's; the jammer
    // row is the Mirage III's and the AV-8B's.
    assert_eq!(sensors(AircraftId::M2000)[..2], rows[..2]);
    assert!(sensors(AircraftId::Av8).contains(&rows[2]));
    // The identity binds the Rafale's HUD and cockpit family by name (the same
    // as the Mirage 2000); neither the Rafale's nor the Mirage 2000's record is
    // aliased.
    assert_eq!(id.hud(), "RAFALE.HUD");
    assert_eq!(id.hud(), AircraftId::Rafale.hud());
    assert_eq!(id.hud(), AircraftId::M2000.hud());
    assert_ne!(id.pt(), AircraftId::Rafale.pt());
    assert_ne!(id.pt(), AircraftId::M2000.pt());
    assert_eq!(id.pt(), "MF1.PT");
    assert_eq!(id.cockpit_stem(), "RAF");
    assert_eq!(id.cockpit(), "~RAFH.PIC");
    assert_eq!(id.instrument_panel(), "~RAF_P.PIC");
    // The record itself names rafale.HUD (what the decision follows).
    assert!(
        tore_formats::aircraft::references(&read(id.pt()))
            .iter()
            .any(|name| name.eq_ignore_ascii_case("rafale.HUD")),
        "MF1.PT no longer names rafale.HUD"
    );
    // The record's own eye offset is the Mirage 2000's (the Rafale's z is 80) and
    // is recorded, not used: TORE reads no per-aircraft eye (MF1-7).
    let eye = |id: AircraftId| -> Vec<String> {
        let record = Aircraft::parse(&read(id.pt())).unwrap();
        ["viewOffset.x", "viewOffset.y", "viewOffset.z"]
            .iter()
            .map(|key| record.object[*key].value.clone())
            .collect()
    };
    assert_eq!(eye(id), ["0", "20", "60"]);
    assert_eq!(eye(id), eye(AircraftId::M2000));
    // The HUD module references exactly that instrument window frame, which is
    // what `Airframe::load` requires of every flyable identity.
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(hud.panel_resource().as_deref(), Some("~RAF_P.PIC"));
    // The art, the instrument frame and every reviewed cockpit overlay exist.
    assert!(has(id.cockpit()), "{}", id.cockpit());
    assert!(has(&id.instrument_panel()));
    for overlay in id.cockpit_overlays().into_iter().flatten() {
        assert!(has(&overlay), "{overlay}");
    }
    // No Mirage F1 cockpit art exists in either archive (MF1-0).
    assert!(
        ![&fa1, &fa2]
            .iter()
            .flat_map(|library| library.entries.keys())
            .any(|name| name.starts_with("~MF")),
        "a ~MF cockpit resource appeared"
    );
}
