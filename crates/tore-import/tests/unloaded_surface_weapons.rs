//! Headless import check (slice JT-3): the decoded ordnance records no surface
//! unit mounts. `HQ2J`, `HQ61`, `MIM23`, `ROLAND`, `SAN5`, `SAN8`, `AAA20`,
//! `M163`, `T20_1` and `T23_1` all decode from `FA_2.LIB`, but no NT hardpoint
//! names any of them. Only `T23_1.JT` is named by a PT record, and that is the
//! turret record of the unported Tu-26 and Tu-95.
//!
//! The check reads the user's own install (the `gameassets` link or
//! `TORE_GAME_DIR`) through the importer's media source and skips quietly
//! without it. It reads records only; no retail bytes are committed.
//! Evidence: docs/formats/aircraft-ordnance.md, "Surface records nothing
//! mounts".
use std::{collections::BTreeSet, path::PathBuf};
use tore_formats::{
    aircraft::{Aircraft, AircraftId},
    surface_unit::SurfaceUnit,
    weapons::Weapon,
};
use tore_import::MediaSource;

/// The ten decoded records the surface lists leave out (slice JT-3).
const DECODED_ONLY: [&str; 10] = [
    "HQ2J.JT",
    "HQ61.JT",
    "MIM23.JT",
    "ROLAND.JT",
    "SAN5.JT",
    "SAN8.JT",
    "AAA20.JT",
    "M163.JT",
    "T20_1.JT",
    "T23_1.JT",
];
/// The radar-guided surface records among them (seeker signature 3, support
/// flag 0x200).
const RADAR: [&str; 4] = ["HQ2J.JT", "HQ61.JT", "MIM23.JT", "ROLAND.JT"];
/// The infrared surface records among them (seeker signature 2).
const INFRARED: [&str; 2] = ["SAN5.JT", "SAN8.JT"];
/// The gun-round records among them.
const GUNS: [&str; 4] = ["AAA20.JT", "M163.JT", "T20_1.JT", "T23_1.JT"];
/// FA weapon flag 0x1: a guided missile.
const GUIDED: u32 = 0x1;
/// FA weapon flag 0x200: the launcher supports the missile.
const SUPPORT: u32 = 0x200;
/// FA weapon flag 0x10000: may aim at aircraft.
const AIR_TARGET: u32 = 0x10000;
/// FA weapon flag 0x20000: may aim at surface targets.
const SURFACE_TARGET: u32 = 0x20000;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

/// The record name in a BRF string line, as a byte needle.
fn string_line(name: &str) -> Vec<u8> {
    format!("string \"{name}\"")
        .to_ascii_lowercase()
        .into_bytes()
}

#[test]
fn no_nt_unit_mounts_a_decoded_only_surface_record() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let (Ok(fa1), Ok(fa2)) = (media.archive("FA_1.LIB"), media.archive("FA_2.LIB")) else {
        eprintln!("skipped: no retail FA_1.LIB and FA_2.LIB");
        return;
    };
    let libraries = [&fa1, &fa2];

    // Every NT hardpoint: no decoded record may be its default store.
    let mut units = 0;
    for library in libraries {
        for name in library.entries.keys() {
            if !name.to_ascii_uppercase().ends_with(".NT") {
                continue;
            }
            let bytes = library.read(name).unwrap();
            let unit = SurfaceUnit::parse(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
            units += 1;
            for (index, mount) in unit.weapons() {
                if let Some(store) = &mount.store {
                    assert!(
                        !DECODED_ONLY.contains(&store.as_str()),
                        "{} mount {index} carries {store}",
                        unit.resource
                    );
                }
            }
        }
    }
    assert_eq!(units, 84, "the 84 retail NT records");

    // Every PT record's decoded strings: only the Tu-26 and Tu-95 turret record
    // name one of the ten, and it is T23_1.JT.
    let mut carriers: BTreeSet<(String, String)> = BTreeSet::new();
    for library in libraries {
        for name in library.entries.keys() {
            if !name.to_ascii_uppercase().ends_with(".PT") {
                continue;
            }
            let bytes = library.read(name).unwrap().to_ascii_lowercase();
            for record in DECODED_ONLY {
                if bytes
                    .windows(string_line(record).len())
                    .any(|window| window == string_line(record))
                {
                    carriers.insert((record.to_owned(), name.to_ascii_uppercase()));
                }
            }
        }
    }
    let expected: BTreeSet<(String, String)> = [
        ("T23_1.JT".to_owned(), "TU26.PT".to_owned()),
        ("T23_1.JT".to_owned(), "TU95.PT".to_owned()),
    ]
    .into_iter()
    .collect();
    assert_eq!(carriers, expected, "decoded PT carriers of the ten records");

    // No imported aircraft is built with one either.
    for id in AircraftId::ALL {
        let library = libraries
            .iter()
            .find(|library| library.entries.contains_key(id.pt()))
            .unwrap_or_else(|| panic!("{} is in neither archive", id.pt()));
        let bytes = library.read(id.pt()).unwrap();
        let aircraft =
            Aircraft::parse(&bytes).unwrap_or_else(|error| panic!("{}: {error}", id.pt()));
        for record in DECODED_ONLY {
            assert!(
                !id.guns().contains(&record),
                "{} must not mount {record} as a gun",
                id.pt()
            );
            assert!(
                !aircraft
                    .hardpoints
                    .iter()
                    .any(|hardpoint| hardpoint.store.as_deref() == Some(record)),
                "{} decoded station list names {record}",
                id.pt()
            );
        }
    }
}

#[test]
fn the_decoded_records_keep_the_classes_the_surface_lists_leave_out() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let (Ok(fa1), Ok(fa2)) = (media.archive("FA_1.LIB"), media.archive("FA_2.LIB")) else {
        eprintln!("skipped: no retail FA_1.LIB and FA_2.LIB");
        return;
    };
    let libraries = [&fa1, &fa2];

    for name in DECODED_ONLY {
        let library = libraries
            .iter()
            .find(|library| library.entries.contains_key(name))
            .unwrap_or_else(|| panic!("{name} is in neither archive"));
        let bytes = library.read(name).unwrap();
        let weapon = Weapon::parse(name, &bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(weapon.source, name, "{name} names itself");
        assert_eq!(
            weapon.flags & AIR_TARGET,
            AIR_TARGET,
            "{name} may aim at aircraft"
        );
        if GUNS.contains(&name) {
            assert_eq!(weapon.flags & GUIDED, 0, "{name} is an unguided round");
            assert!(weapon.movement.initial_speed > 0, "{name} muzzle speed");
            // The two aircraft turret rounds have no seeker; the two radar
            // AAA records carry signature 3 for their fire-control radar.
            if ["T20_1.JT", "T23_1.JT"].contains(&name) {
                assert_eq!(weapon.seeker.signature, 0, "{name} has no seeker");
            }
        } else {
            assert_eq!(
                weapon.flags & SURFACE_TARGET,
                0,
                "{name} is not a surface-target record"
            );
            assert_eq!(weapon.flags & GUIDED, GUIDED, "{name} is a guided missile");
            assert_eq!(weapon.movement.initial_speed, 0, "{name} starts from rest");
        }
    }
    for name in RADAR {
        let library = libraries
            .iter()
            .find(|library| library.entries.contains_key(name))
            .unwrap();
        let weapon = Weapon::parse(name, &library.read(name).unwrap()).unwrap();
        assert_eq!(weapon.seeker.signature, 3, "{name} radar seeker");
        assert_eq!(weapon.flags & SUPPORT, SUPPORT, "{name} support flag");
    }
    for name in INFRARED {
        let library = libraries
            .iter()
            .find(|library| library.entries.contains_key(name))
            .unwrap();
        let weapon = Weapon::parse(name, &library.read(name).unwrap()).unwrap();
        assert_eq!(weapon.seeker.signature, 2, "{name} infrared seeker");
    }
}
