//! T95-7: the Tu-95 flies the AC-130 cockpit family its record names
//! (`bmbroh.HUD`, stem `AC130`, `~AC130H.PIC`; the coordinator's first pass,
//! BOMBERS-DECISIONS-C Tu-95 item 10) and keeps its four fixed sensor rows
//! with no pod body (`SU37L.SEE` included: a fixed row, not a station). This
//! reads `TU95.PT`, `BMBROH.HUD` and the AC-130 pictures from the user's
//! install and pins the bindings, the art size the glass aperture is keyed to
//! (`hud_aperture.rs`, 1280 x 490) and the record's own eye offset. It skips
//! quietly without a retail install and reads data only; no retail bytes are
//! committed. Evidence: docs/formats/aircraft-variety.md, "Tu-95 Bear-H
//! ordnance, sensors, HUD and cockpit (T95-6/7, 2026-11-16)".
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
fn the_tu95_flies_the_ac130_cockpit_family_with_four_fixed_sensor_rows_and_no_pod_body() {
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
    let id = AircraftId::Tu95;
    let record = Aircraft::parse(&read(id.pt())).unwrap();
    let sensors = record
        .hardpoints
        .iter()
        .filter_map(|h| h.store.clone())
        .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
        .collect::<Vec<_>>();
    // The retail rows: the visual sensor, the radar, the jammer and the
    // built-in laser designator. All four are fixed rows (flags 0x1008) at the
    // origin or the tail, none a pod station, so no pod body is drawn.
    assert_eq!(
        sensors,
        ["VIS300.SEE", "TU95R.SEE", "TU95.ECM", "SU37L.SEE"]
    );
    for row in record
        .hardpoints
        .iter()
        .filter(|h| h.store.as_deref().is_some_and(|s| s.ends_with(".SEE")))
    {
        assert_eq!(row.flags, 0x1008, "{:?}", row.store);
        assert_eq!(row.position, [0, 0, 0], "{:?}", row.store);
    }
    assert_eq!(id.radar(), Some("TU95R.SEE"));
    // The record's HUD module and the AC-130 family it names.
    assert_eq!(id.hud(), "BMBROH.HUD");
    assert_eq!(id.cockpit_stem(), "AC130");
    assert_eq!(id.cockpit(), "~AC130H.PIC");
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(
        hud.panel_resource().as_deref(),
        Some(id.instrument_panel().as_str())
    );
    assert!(has(id.cockpit()) && has(&id.instrument_panel()));
    // The art is the 1280 x 490 picture the Tu-95's glass polygon covers whole.
    let art = tore_formats::Pic::parse(&read(id.cockpit())).unwrap();
    assert_eq!((art.width, art.height), (1280, 490));
    // No Tu-95 cockpit art exists (T95-0): no `~TU95*` picture.
    let mut names: Vec<_> = [&fa1, &fa2]
        .iter()
        .flat_map(|library| library.entries.keys())
        .filter(|name| name.to_ascii_uppercase().starts_with("~TU95"))
        .cloned()
        .collect();
    names.sort();
    assert!(names.is_empty(), "{names:?}");
    // The record's own eye offset is a record field no host code reads. Read
    // at the shape's 2/3 ft a unit it is 6.7 ft up and 20 ft ahead, in the
    // wing area; the fitted seat in `mirrors.rs` is the drawn flight deck
    // (5 ft up, 53 ft ahead). Pinned so a later change shows.
    let eye: Vec<_> = ["viewOffset.x", "viewOffset.y", "viewOffset.z"]
        .iter()
        .map(|key| record.object[*key].value.clone())
        .collect();
    assert_eq!(eye, ["0", "20", "60"]);
}
