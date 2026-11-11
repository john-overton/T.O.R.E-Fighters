//! GRI-6: the Saab Gripen's default load, its BK 27 gun and its countermeasures
//! are the retail records' own. This reads `GRIPEN.PT`, `BK27.JT`, `DEFA.JT`,
//! `M61.JT` and `F18.ECM` from the user's install and pins what the shared
//! loading rules start from: the BK27 gun at 570 rounds (John kept the record's
//! count, 2026-11-10), the centreline `F150.GAS`, two `AGM65G.JT` on the inner
//! wing, four `AIM120.JT` on the outer wing pair, two `AIM9M.JT` on the tips,
//! and the jammer's 30 chaff and 30 flares. It also pins the BK27 record's gun
//! fields (muzzle velocity, damage by class, rounds per shot, burst timing,
//! fire sound) so a change to the shared gun code or the record shows up.
//! It skips quietly without a retail install and reads data only; no retail
//! bytes are committed. Evidence: docs/formats/aircraft-ordnance.md, "The Saab
//! Gripen (GRI-6, 2026-11-10)".
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
fn the_gripens_default_load_gun_and_countermeasures_are_the_records_own() {
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
    let aircraft = Aircraft::parse(&read(AircraftId::Gripen.pt())).unwrap();
    // Nine rows: the three fixed sensors, the AAS-38 placeholder, the BK27 gun
    // and the four editable carriers.
    let rows: Vec<_> = aircraft
        .hardpoints
        .iter()
        .map(|h| (h.store.as_deref().unwrap_or(""), h.count))
        .collect();
    assert_eq!(
        rows,
        [
            ("VIS340.SEE", 1),
            ("F18R.SEE", 1),
            ("F18.ECM", 1),
            ("AAS38.SEE", 1),
            ("BK27.JT", 570),
            ("F150.GAS", 1),
            ("AGM65G.JT", 2),
            ("AIM120.JT", 4),
            ("AIM9M.JT", 2),
        ]
    );
    // Countermeasures: the fleet's `F18.ECM`.
    let ecm = Countermeasures::parse("F18.ECM", &read("F18.ECM")).unwrap();
    assert_eq!((ecm.chaff[0], ecm.flare[0]), (30, 30));

    // The BK27 record: a gun round with the shared gun movement and burst
    // fields, its own fire sound.
    let bk27 = Weapon::parse("BK27.JT", &read("BK27.JT")).unwrap();
    let defa = Weapon::parse("DEFA.JT", &read("DEFA.JT")).unwrap();
    let m61 = Weapon::parse("M61.JT", &read("M61.JT")).unwrap();
    assert_eq!(
        bk27.flags & 0x2,
        0,
        "a built-in gun round, not a pylon store"
    );
    assert_eq!(bk27.movement.initial_speed, 2933);
    assert_eq!(bk27.burst.actual_rounds_per_game, 2);
    assert_eq!(bk27.damage.by_class, [25, 2, 7, 7, 25]);
    assert_eq!(bk27.fire_sound.as_deref(), Some("&FSTGUN3.11K"));
    assert_eq!(bk27.effects.max_sound_distance, 6000);
    assert_eq!(bk27.shape.as_deref(), Some("BULLET.SH"));
    // Everything the shared gun code reads, apart from the fire sound, is the
    // M61 and DEFA records' own values.
    for other in [&defa, &m61] {
        assert_eq!(bk27.movement, other.movement, "{}", other.source);
        assert_eq!(bk27.burst, other.burst, "{}", other.source);
        assert_eq!(bk27.damage, other.damage, "{}", other.source);
        assert_eq!(bk27.effects, other.effects, "{}", other.source);
        assert_eq!(bk27.guidance.hit_modifiers, other.guidance.hit_modifiers);
        assert_eq!(bk27.guidance.max_aon, other.guidance.max_aon);
    }
    // DEFA and BK27 agree on the guidance `chances` row too; the M61's
    // differs, and no gun code reads that row.
    assert_eq!(bk27.guidance, defa.guidance);
}
