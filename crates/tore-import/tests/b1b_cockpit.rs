//! B1B-7: the B-1B flies the HUD module its record names (`bmbrcc.HUD`, the
//! B-2's module with three bytes changed) and the B-2's cockpit art (stem
//! `B2`, `~B2H.PIC`; coordinator first pass, decision 7), and keeps the five
//! sensor rows of the bomber template with no pod body (decision 5). This
//! reads `B1.PT`, `bmbrcc.HUD` and the `~B2*` pictures from the user's install
//! and pins the bindings: the five fixed sensor rows, the radar and jammer
//! records, the HUD module and the instrument window frame it references, the
//! shared art's size (the glass aperture in `hud_aperture.rs` is keyed to it)
//! and the eye the record carries but no host code reads. It skips quietly
//! without a retail install and reads data only; no retail bytes are
//! committed. Evidence: docs/formats/aircraft-variety.md, "B-1B Lancer
//! ordnance, sensors, HUD and cockpit (B1B-6/7, 2026-11-16)".
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
fn the_b1b_flies_its_record_hud_on_the_b2_art_with_five_fixed_sensor_rows() {
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
    let id = AircraftId::B1b;
    let record = Aircraft::parse(&read(id.pt())).unwrap();
    let sensors = record
        .hardpoints
        .iter()
        .filter_map(|h| h.store.clone())
        .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
        .collect::<Vec<_>>();
    // The five rows stay as retail: the F/A-18's radar and jammer, the visual
    // set, and Pave Spike and the IR pod as fixed rows (no designator bit, no
    // station, no pod body).
    assert_eq!(
        sensors,
        [
            "VIS340.SEE",
            "F18R.SEE",
            "F18.ECM",
            "PAVESPK.SEE",
            "AV8IR.SEE"
        ]
    );
    for row in record
        .hardpoints
        .iter()
        .filter(|h| h.store.as_deref().is_some_and(|s| s.ends_with(".SEE")))
    {
        assert_eq!(
            row.flags & 0x400,
            0,
            "{:?} carries the designator bit",
            row.store
        );
    }
    assert_eq!(id.radar(), Some("F18R.SEE"));
    // The record's own HUD module, the instrument window frame it names and
    // the B-2's cockpit art.
    assert_eq!(id.hud(), "BMBRCC.HUD");
    assert_eq!(id.cockpit_stem(), "B2");
    assert_eq!(id.stem(), "B1");
    assert_eq!(id.cockpit(), "~B2H.PIC");
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(
        hud.panel_resource().as_deref(),
        Some(id.instrument_panel().as_str())
    );
    assert!(has(id.cockpit()), "{}", id.cockpit());
    assert!(has(&id.instrument_panel()));
    assert!(id.cockpit_overlays().iter().all(Option::is_none));
    // The art is the B-2's 1394 x 508, the size its glass polygon is keyed to.
    let art = tore_formats::Pic::parse(&read(id.cockpit())).unwrap();
    assert_eq!((art.width, art.height), (1394, 508));
    // The record's own eye offset is a record field no host code reads (the
    // fitted seats in `mirrors.rs` are the only eyes). Pinned so a later
    // change shows. It is the B-2's template value (0, 30, 60); read at the
    // shape's 4/3 ft a unit it would be 40 and 80 ft, which is not the drawn
    // flight deck (the hump 64 to 72 ft ahead, 8 to 10.7 ft up), so it is
    // not used.
    let eye: Vec<_> = ["viewOffset.x", "viewOffset.y", "viewOffset.z"]
        .iter()
        .map(|key| record.object[*key].value.clone())
        .collect();
    assert_eq!(eye, ["0", "30", "60"]);
}
