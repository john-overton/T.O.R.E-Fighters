//! TOR-6: the Panavia Tornado IDS's default load, its BK 27 gun and its
//! countermeasures are the retail records' own. This reads `TOR.PT`, `BK27.JT`,
//! `DEFA.JT`, `M61.JT` and `MIG31.ECM` from the user's install and pins what the
//! shared loading rules start from: the eight source rows (the four fixed
//! sensors, the BK27 gun at 360 rounds, the centreline tank row, the shoulder
//! Paveways and the inner wing Mk 82 pair; the outer wing row is retired), the
//! retail jammer record that stays as it is (no pod body), and the BK27
//! record's gun fields (muzzle velocity, damage by class, rounds per shot,
//! burst timing, fire sound), so a change to the shared gun code or the record
//! shows up. It skips quietly without a retail install and reads data only; no
//! retail bytes are committed. Evidence: docs/formats/aircraft-ordnance.md,
//! "The Panavia Tornado IDS (TOR-6, 2026-11-11)".
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
fn the_tornados_rows_gun_and_jammer_are_the_records_own() {
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
    let aircraft = Aircraft::parse(&read(AircraftId::Tornado.pt())).unwrap();
    // Eight source rows: the four fixed sensors, the BK27 gun, the centreline
    // tank row (capacity two; the default of one is the T3 default rule in
    // `aircraft_stations::default_counts`), the shoulder Paveways and the inner
    // wing Mk 82 pair. The retail outer wing Sidewinder pair (a ninth row,
    // `AIM9M.JT` x2) is retired by `aircraft_stations::retire` before the
    // record reaches the loading rules (John, 2026-11-10).
    let rows: Vec<_> = aircraft
        .hardpoints
        .iter()
        .map(|h| (h.store.as_deref().unwrap_or(""), h.count))
        .collect();
    assert_eq!(
        rows,
        [
            ("VIS240.SEE", 1),
            ("F18R.SEE", 1),
            ("MIG31.ECM", 1),
            ("PAVESPK.SEE", 1),
            ("BK27.JT", 360),
            ("F250.GAS", 2),
            ("PAVEWAY.JT", 4),
            ("MK82P3.JT", 2),
        ]
    );
    // Masks and weight classes of the tank row and the two weapon rows that
    // refuse stores, so a refusal list is read against the right rows.
    let masks: Vec<_> = aircraft
        .hardpoints
        .iter()
        .skip(5)
        .map(|h| (h.flags, h.weight_class))
        .collect();
    assert_eq!(masks, [(0x205, 40), (0x7e5, 80), (0x505, 30)]);

    // Countermeasures: the retail `MIG31.ECM` row stays (John, 2026-11-10,
    // decision 22): 60 chaff and 60 flares with its deception jammer, and no
    // pod body is drawn.
    let ecm = Countermeasures::parse("MIG31.ECM", &read("MIG31.ECM")).unwrap();
    assert_eq!((ecm.chaff[0], ecm.flare[0]), (60, 60));

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
    for other in [&defa, &m61] {
        assert_eq!(bk27.movement, other.movement, "{}", other.source);
        assert_eq!(bk27.burst, other.burst, "{}", other.source);
        assert_eq!(bk27.damage, other.damage, "{}", other.source);
        assert_eq!(bk27.effects, other.effects, "{}", other.source);
        assert_eq!(bk27.guidance.hit_modifiers, other.guidance.hit_modifiers);
        assert_eq!(bk27.guidance.max_aon, other.guidance.max_aon);
    }
    assert_eq!(bk27.guidance, defa.guidance);
}
