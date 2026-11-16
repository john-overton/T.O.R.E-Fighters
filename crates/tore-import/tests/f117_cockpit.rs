//! F117-7: the F-117 flies its own retail HUD and cockpit family by name
//! (`F117.HUD`, stem `F117`, `~F117H.PIC`; John, decision 28, opinionated) and
//! keeps its record's sensor rows: `PAVESPK.SEE` as the IRADS turret and
//! `AV8IR.SEE` as the FLIR (decision 26), with no radar row at all. This reads
//! `F117.PT`, `F117.HUD` and the `~F117*` pictures from the user's install and
//! pins the bindings: the fixed sensor rows, the HUD module the identity
//! names, the instrument window frame that HUD references, the cockpit art
//! size, and the absence of any overlay picture. It skips quietly without a
//! retail install and reads data only; no retail bytes are committed.
//! Evidence: docs/formats/aircraft-variety.md, "F-117 ordnance, sensors and
//! cockpit (F117-6/7, 2026-11-15)".
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
fn the_f117_flies_its_own_hud_and_cockpit_family_with_the_irads_and_flir_rows() {
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
    let id = AircraftId::F117;
    let record = Aircraft::parse(&read(id.pt())).unwrap();
    let sensors = record
        .hardpoints
        .iter()
        .filter_map(|h| h.store.clone())
        .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
        .collect::<Vec<_>>();
    // Decision 26: the turret row is the laser designator and the IR row the
    // FLIR; neither has a station or a pod body. The retail jammer row's
    // default is cleared (BMB-A2), so the record carries no jammer, chaff or
    // flares: no `.ECM` store is left on it.
    assert_eq!(sensors, ["VIS340.SEE", "PAVESPK.SEE", "AV8IR.SEE"]);
    // The F-117 has no radar row (and so no radar).
    assert_eq!(id.radar(), None);
    assert!(!sensors.iter().any(|s| s == "F18R.SEE"));
    // The turret and the FLIR have their own names; nothing else does.
    let names = id.sensor_names().unwrap();
    assert_eq!((names.sight, names.infrared), ("IRADS", "FLIR"));
    // Decision 28: the retail F-117's own HUD module and cockpit family. The
    // record's `hudName` is a null pointer, so the HUD is bound by name.
    assert_eq!(id.hud(), "F117.HUD");
    assert_eq!(id.cockpit_stem(), "F117");
    assert_eq!(id.stem(), "F117");
    assert_eq!(id.cockpit(), "~F117H.PIC");
    assert_eq!(id.instrument_panel(), "~F117_P.PIC");
    // No other flyable identity uses the family.
    for other in AircraftId::ALL {
        if other != id {
            assert_ne!(other.cockpit_stem(), "F117", "{other:?}");
            assert_ne!(other.hud(), "F117.HUD", "{other:?}");
        }
    }
    // `F117.HUD` references exactly the instrument window frame the identity
    // names (what `Airframe::load` requires of every flyable identity).
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(hud.panel_resource().as_deref(), Some("~F117_P.PIC"));
    // The art and the instrument frame exist; the F-117 is a no-overlay stem
    // (`~F117_LH.PIC`, `~F117_CH.PIC` and `~F117_RH.PIC` do not exist).
    assert!(has(id.cockpit()), "{}", id.cockpit());
    assert!(has(&id.instrument_panel()));
    assert!(id.cockpit_overlays().iter().all(Option::is_none));
    for overlay in ["~F117_LH.PIC", "~F117_CH.PIC", "~F117_RH.PIC"] {
        assert!(!has(overlay), "{overlay} appeared");
    }
    // The whole `~F117*` picture family is the four pictures (low, high and
    // super-high cockpit art, and the instrument window frame).
    let mut names: Vec<_> = [&fa1, &fa2]
        .iter()
        .flat_map(|library| library.entries.keys())
        .filter(|name| name.starts_with("~F117") && name.ends_with(".PIC"))
        .cloned()
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["~F117.PIC", "~F117H.PIC", "~F117S.PIC", "~F117_P.PIC"]
    );
    // The art TORE draws is 1673 x 508, not the 1280 x 490 of the older
    // families: the glass aperture in `hud_aperture.rs` is keyed to this size.
    let art = tore_formats::Pic::parse(&read(id.cockpit())).unwrap();
    assert_eq!((art.width, art.height), (1673, 508));
    // The record's own eye offset is a record field no host code reads (the
    // shared pilot eye and the fitted ones in `mirrors.rs` are the only
    // eyes); pinned so a later change shows.
    let eye: Vec<_> = ["viewOffset.x", "viewOffset.y", "viewOffset.z"]
        .iter()
        .map(|key| record.object[*key].value.clone())
        .collect();
    assert_eq!(eye, ["0", "20", "60"]);
}
