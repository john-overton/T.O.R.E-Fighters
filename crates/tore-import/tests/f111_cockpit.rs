//! F111-7: the F-111F flies the AC-130's HUD module and cockpit family by its
//! record's own name (`AC130.HUD`, stem `AC130`, `~AC130H.PIC`; coordinator
//! first pass, F111 item 8, opinionated) and keeps its record's five sensor
//! rows: the A-6 radar and jammer (`A6R.SEE`, `A6.ECM`, reviewed Transitional
//! in this slice), the visual and FLIR rows and the Pave Spike laser row, which
//! the F-111F presents as the PAVE TACK pod and its FLIR (text only). This reads
//! `F111.PT`, `AC130.HUD` and the `~AC130*` pictures from the user's install and
//! pins the bindings: the sensor rows, the names, the HUD module, the
//! instrument window frame, the cockpit art size, the absent overlays and the
//! family. It skips quietly without a retail install and reads data only; no
//! retail bytes are committed. Evidence: docs/formats/aircraft-variety.md,
//! "F-111F ordnance, sensors, HUD and cockpit (F111-6/7, 2026-11-16)".
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
fn the_f111_flies_the_ac130_hud_and_cockpit_family_with_the_pave_tack_rows() {
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
    let id = AircraftId::F111;
    let record = Aircraft::parse(&read(id.pt())).unwrap();
    let sensors = record
        .hardpoints
        .iter()
        .filter_map(|h| h.store.clone())
        .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
        .collect::<Vec<_>>();
    // The five rows stay as retail: the visual set, the A-6 radar and jammer,
    // the FLIR and the Pave Spike laser (fixed rows, no body, no station).
    assert_eq!(
        sensors,
        [
            "VIS240.SEE",
            "A6R.SEE",
            "A6.ECM",
            "AV8IR.SEE",
            "PAVESPK.SEE"
        ]
    );
    assert_eq!(id.radar(), Some("A6R.SEE"));
    // The laser and the FLIR are the Pave Tack's, in text only; the F-117 is
    // the only other aircraft with names, and every other aircraft has none.
    let names = id.sensor_names().unwrap();
    assert_eq!(
        (names.sight, names.infrared, names.pod),
        ("PAVE TACK", "FLIR", "Pave Tack")
    );
    // The coordinator's first pass (F111 item 8): the record's own HUD module
    // and the AC-130's cockpit family.
    assert_eq!(id.hud(), "AC130.HUD");
    assert_eq!(id.cockpit_stem(), "AC130");
    assert_eq!(id.stem(), "F111");
    assert_eq!(id.cockpit(), "~AC130H.PIC");
    assert_eq!(id.instrument_panel(), "~AC130_P.PIC");
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(hud.panel_resource().as_deref(), Some("~AC130_P.PIC"));
    assert!(has(id.cockpit()), "{}", id.cockpit());
    assert!(has(&id.instrument_panel()));
    // The AC-130 family has no overlay pictures, so the F-111F is a no-overlay
    // stem like the AC-130 itself.
    assert!(id.cockpit_overlays().iter().all(Option::is_none));
    for overlay in ["~AC130_LH.PIC", "~AC130_CH.PIC", "~AC130_RH.PIC"] {
        assert!(!has(overlay), "{overlay} appeared");
    }
    // The art TORE draws is 1280 x 490, the size `hud_aperture.rs` keys the
    // whole-picture glass to.
    let art = tore_formats::Pic::parse(&read(id.cockpit())).unwrap();
    assert_eq!((art.width, art.height), (1280, 490));
    // The record's own eye offset is a record field no host code reads (the
    // fitted eye in `mirrors.rs` is the one the views use); pinned so a later
    // change shows.
    let eye: Vec<_> = ["viewOffset.x", "viewOffset.y", "viewOffset.z"]
        .iter()
        .map(|key| record.object[*key].value.clone())
        .collect();
    assert_eq!(eye, ["0", "20", "60"]);
}
