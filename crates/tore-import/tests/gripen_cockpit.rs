//! GRI-7: the Saab Gripen flies its own HUD and cockpit family by name, as its
//! record does (John, 2026-11-10, opinionated), and its sensors are the
//! F/A-18's reviewed records plus the AAS-38 channel. This reads `GRIPEN.PT`,
//! `GRIPEN.HUD` and the thirteen `~GRI*.PIC` pictures from the user's install
//! and pins that binding: the record's four fixed sensor rows, the HUD module
//! the identity names (its own, not the Rafale's), the instrument window frame
//! that HUD references, the cockpit art's own sizes (the forward picture is
//! 1279 wide, not 1280) and every reviewed overlay. It skips quietly without a
//! retail install and reads data only; no retail bytes are committed. Evidence:
//! docs/formats/aircraft-variety.md, "Gripen sensors, HUD and cockpit (GRI-7,
//! 2026-11-10)".
use std::path::PathBuf;
use tore_formats::{
    Pic,
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
fn the_gripen_flies_its_own_hud_and_cockpit_family_by_name() {
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
    let id = AircraftId::Gripen;
    let aircraft = Aircraft::parse(&read(id.pt())).unwrap();
    // The record's fixed sensor rows: the F/A-18's visual, radar and jammer
    // records, then the AAS-38 channel (a pod record with no shape).
    let sensors: Vec<_> = aircraft
        .hardpoints
        .iter()
        .filter_map(|h| h.store.as_deref())
        .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
        .collect();
    assert_eq!(sensors, ["VIS340.SEE", "F18R.SEE", "F18.ECM", "AAS38.SEE"]);
    assert_eq!(id.radar(), Some("F18R.SEE"));
    // The identity binds the Gripen's own HUD and cockpit family; neither the
    // Rafale's nor any other record is aliased.
    assert_eq!(id.hud(), "GRIPEN.HUD");
    assert_ne!(id.hud(), AircraftId::Rafale.hud());
    assert_eq!(id.cockpit_stem(), "GRI");
    assert_eq!(id.cockpit(), "~GRIH.PIC");
    assert_eq!(id.instrument_panel(), "~GRI_P.PIC");
    assert_eq!(id.pt(), "GRIPEN.PT");
    // The HUD module references exactly that instrument window frame, which
    // is what `Airframe::load` requires of every flyable identity.
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(hud.panel_resource().as_deref(), Some("~GRI_P.PIC"));
    // The art set: thirteen pictures, each with its own size.
    let sizes = [
        ("~GRI.PIC", (535, 205)),
        ("~GRIH.PIC", (1279, 490)),
        ("~GRIS.PIC", (1601, 613)),
        ("~GRI_L.PIC", (61, 109)),
        ("~GRI_C.PIC", (130, 30)),
        ("~GRI_R.PIC", (61, 109)),
        ("~GRI_P.PIC", (81, 80)),
    ];
    for (name, size) in sizes {
        let pic = Pic::parse(&read(name)).unwrap();
        assert_eq!((pic.width, pic.height), size, "{name}");
    }
    let family = [
        "~GRI.PIC",
        "~GRIH.PIC",
        "~GRIS.PIC",
        "~GRI_L.PIC",
        "~GRI_C.PIC",
        "~GRI_R.PIC",
        "~GRI_LH.PIC",
        "~GRI_CH.PIC",
        "~GRI_RH.PIC",
        "~GRI_LS.PIC",
        "~GRI_CS.PIC",
        "~GRI_RS.PIC",
        "~GRI_P.PIC",
    ];
    for name in family {
        assert!(has(name), "{name}");
    }
    // Every reviewed cockpit overlay the identity loads exists.
    for overlay in id.cockpit_overlays().into_iter().flatten() {
        assert!(has(&overlay), "{overlay}");
    }
    // Exactly thirteen `~GRI` pictures exist (no extra family member).
    let count = [&fa1, &fa2]
        .iter()
        .flat_map(|library| library.entries.keys())
        .filter(|name| name.starts_with("~GRI") && name.ends_with(".PIC"))
        .count();
    assert_eq!(count, 13);
}
