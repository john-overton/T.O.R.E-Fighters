//! B2-7: the B-2 flies its own retail HUD and cockpit family by name
//! (`B2.HUD`, stem `B2`, `~B2H.PIC`; John, decision 17, opinionated) and keeps
//! the B-1B template's five sensor rows with no pod body (decision 18). This
//! reads `B2.PT`, `B2.HUD` and the `~B2*` pictures from the user's install and
//! pins the bindings: the five fixed sensor rows, the HUD module the identity
//! names, the instrument window frame that HUD references, the cockpit art
//! size, and the absence of any overlay picture. It skips quietly without a
//! retail install and reads data only; no retail bytes are committed.
//! Evidence: docs/formats/aircraft-variety.md, "B-2 sensors, HUD and cockpit
//! (B2-7, 2026-11-15)".
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
fn the_b2_flies_its_own_hud_and_cockpit_family_with_the_b1b_sensor_rows() {
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
    let id = AircraftId::B2;
    let record = Aircraft::parse(&read(id.pt())).unwrap();
    let sensors = record
        .hardpoints
        .iter()
        .filter_map(|h| h.store.clone())
        .filter(|s| s.ends_with(".SEE") || s.ends_with(".ECM"))
        .collect::<Vec<_>>();
    // Decision 18: the B-1B's five rows stay as retail, Pave Spike and the IR
    // pod included (fixed rows at the origin, no pod body, no station).
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
    assert_eq!(id.radar(), Some("F18R.SEE"));
    // Decision 17: the retail B-2's own HUD module and cockpit family. The
    // record's `hudName` is a null pointer, so the HUD is bound by name.
    assert_eq!(id.hud(), "B2.HUD");
    assert_eq!(id.cockpit_stem(), "B2");
    assert_eq!(id.stem(), "B2");
    assert_eq!(id.cockpit(), "~B2H.PIC");
    assert_eq!(id.instrument_panel(), "~B2_P.PIC");
    // No other flyable identity uses the family, except the two batch B
    // bombers whose records name the B-2's art (BMB-B1, 2026-11-15): the
    // B-1B (`bmbrcc.HUD`, the B-2's module with three bytes changed) takes
    // the cockpit stem, and the Tu-160 (`b2.HUD`) takes the stem and the HUD.
    for other in AircraftId::ALL {
        if other != id {
            let shares_art = matches!(other, AircraftId::B1b | AircraftId::Tu160);
            assert_eq!(other.cockpit_stem() == "B2", shares_art, "{other:?}");
            assert_eq!(
                other.hud() == "B2.HUD",
                other == AircraftId::Tu160,
                "{other:?}"
            );
        }
    }
    // `B2.HUD` references exactly the instrument window frame the identity
    // names (what `Airframe::load` requires of every flyable identity).
    let hud = Hud::parse(&read(id.hud())).unwrap();
    assert_eq!(hud.panel_resource().as_deref(), Some("~B2_P.PIC"));
    // The art and the instrument frame exist; the B-2 is a no-overlay stem
    // (`~B2_LH.PIC`, `~B2_CH.PIC` and `~B2_RH.PIC` do not exist).
    assert!(has(id.cockpit()), "{}", id.cockpit());
    assert!(has(&id.instrument_panel()));
    assert!(id.cockpit_overlays().iter().all(Option::is_none));
    for overlay in ["~B2_LH.PIC", "~B2_CH.PIC", "~B2_RH.PIC"] {
        assert!(!has(overlay), "{overlay} appeared");
    }
    // The whole `~B2*` picture family is the four pictures (low, high and
    // super-high cockpit art, and the instrument window frame).
    let mut names: Vec<_> = [&fa1, &fa2]
        .iter()
        .flat_map(|library| library.entries.keys())
        .filter(|name| name.starts_with("~B2") && name.ends_with(".PIC"))
        .cloned()
        .collect();
    names.sort();
    assert_eq!(names, ["~B2.PIC", "~B2H.PIC", "~B2S.PIC", "~B2_P.PIC"]);
    // The art TORE draws is 1394 x 508, not the 1280 x 490 of the older
    // families: the glass aperture in `hud_aperture.rs` is keyed to this size.
    let art = tore_formats::Pic::parse(&read(id.cockpit())).unwrap();
    assert_eq!((art.width, art.height), (1394, 508));
    // The record's own eye offset is a record field no host code reads (the
    // shared pilot eye and the B-2's fitted one are the only eyes). Read at
    // the shape's 1/3 ft a unit it is 10 ft up and 20 ft ahead, the fitted
    // seat in `mirrors.rs`: pinned so a later change shows.
    let eye: Vec<_> = ["viewOffset.x", "viewOffset.y", "viewOffset.z"]
        .iter()
        .map(|key| record.object[*key].value.clone())
        .collect();
    assert_eq!(eye, ["0", "30", "60"]);
}
