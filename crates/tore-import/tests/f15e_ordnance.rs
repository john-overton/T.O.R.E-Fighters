//! F15E-6: the F-15E Strike Eagle's default load, its M61 gun and its
//! countermeasures are the retail records' own. This reads `F15E.PT`,
//! `M61.JT` and `F15.ECM` from the user's install and pins what the shared
//! loading rules start from: the M61 at 450 rounds (John kept the record's
//! count, 2026-11-10; the real jet carries 512), the centreline `F350.GAS`,
//! twelve `MK82.JT` on the belly pair, four `AIM120.JT` on the wing pair, and
//! the jammer's chaff and flares. It skips quietly without a retail install and
//! reads data only; no retail bytes are committed. Evidence:
//! docs/formats/aircraft-ordnance.md, "The F-15E Strike Eagle (F15E-6,
//! 2026-11-10)".
use std::path::PathBuf;
use tore_formats::{
    aircraft::{Aircraft, AircraftId},
    weapons::{Countermeasures, Weapon},
};
use tore_import::MediaSource;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

#[test]
fn the_f15es_default_load_gun_and_countermeasures_are_the_records_own() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let (Ok(fa1), Ok(fa2)) = (media.archive("FA_1.LIB"), media.archive("FA_2.LIB")) else {
        eprintln!("skipped: no retail FA_1.LIB and FA_2.LIB");
        return;
    };
    let read = |name: &str| {
        [&fa1, &fa2]
            .iter()
            .find(|library| library.entries.contains_key(name))
            .unwrap_or_else(|| panic!("{name} is in neither archive"))
            .read(name)
            .unwrap()
    };
    let aircraft = Aircraft::parse(&read(AircraftId::F15E.pt())).unwrap();
    // Nine rows: the three fixed sensors, the internal FLIR and the fixed Pave
    // Spike row, the M61 gun and the three editable carriers.
    let rows: Vec<_> = aircraft
        .hardpoints
        .iter()
        .map(|h| (h.store.as_deref().unwrap_or(""), h.count))
        .collect();
    assert_eq!(
        rows,
        [
            ("VIS340.SEE", 1),
            ("F15R.SEE", 1),
            ("F15.ECM", 1),
            ("AV8IR.SEE", 1),
            ("PAVESPK.SEE", 1),
            ("M61.JT", 450),
            ("F350.GAS", 1),
            ("MK82.JT", 12),
            ("AIM120.JT", 4),
        ]
    );
    // Countermeasures: the F-15 family's own `F15.ECM`.
    let ecm = Countermeasures::parse("F15.ECM", &read("F15.ECM")).unwrap();
    assert_eq!((ecm.chaff[0], ecm.flare[0]), (30, 30));

    // The gun is the M61 record the F-15C, F-16 and F/A-18 share.
    let m61 = Weapon::parse("M61.JT", &read("M61.JT")).unwrap();
    assert_eq!(
        m61.flags & 0x2,
        0,
        "a built-in gun round, not a pylon store"
    );
    assert_eq!(m61.shape.as_deref(), Some("BULLET.SH"));
    assert_eq!(m61.burst.actual_rounds_per_game, 2);
}
