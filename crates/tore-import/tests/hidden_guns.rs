//! Headless import check (slice JT-1): the decoded cannon records nothing can
//! load. ADEN, BK27, GAU13 and GSH6_23 are named pods and 20MM_4 and ADENIN are
//! guns, but their FA records carry projectile flag 2 clear, so the station
//! rule admits them only where a station's own default names them. Every
//! decoded carrier is an aircraft the roster does not import, so no registered
//! aircraft gains one and the records stay out of `SUPPORTED_WEAPONS`.
//!
//! The check reads the user's own install (the `gameassets` link or
//! `TORE_GAME_DIR`) through the importer's media source and skips quietly
//! without it. It reads records only; no retail bytes are committed.
//! Evidence: docs/formats/aircraft-ordnance.md, "Cannon records nothing mounts".
use std::path::PathBuf;
use tore_formats::{
    aircraft::{Aircraft, AircraftId},
    weapons::Weapon,
};
use tore_import::MediaSource;

/// The six decoded cannon records the ordnance catalog hides (slice JT-1).
const HIDDEN: [&str; 6] = [
    "20MM_4.JT",
    "ADEN.JT",
    "ADENIN.JT",
    "BK27.JT",
    "GAU13.JT",
    "GSH6_23.JT",
];
/// FA projectile flag 2: set on a record a compatible station admits.
const STATION_COMPATIBLE: u32 = 0x2;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

#[test]
fn decoded_cannon_records_are_internal_gun_style_and_mounted_by_no_roster_aircraft() {
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

    // Each record decodes as a built-in-gun-style round: flag 2 clear, so only
    // a station's own default admits it (loading.rs `allowed_count`).
    for name in HIDDEN {
        let library = libraries
            .iter()
            .find(|library| library.entries.contains_key(name))
            .unwrap_or_else(|| panic!("{name} is in neither archive"));
        let bytes = library.read(name).unwrap();
        let weapon = Weapon::parse(name, &bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(weapon.source, name, "{name} names itself");
        assert_eq!(
            weapon.flags & STATION_COMPATIBLE,
            0,
            "{name} must keep its built-in-gun compatibility flag"
        );
        assert_eq!(weapon.seeker.signature, 0, "{name} signature");
    }

    // No registered aircraft is built with one or may pod-mount one, and no
    // registered aircraft's decoded station list names one as a store.
    for id in AircraftId::ALL {
        let library = libraries
            .iter()
            .find(|library| library.entries.contains_key(id.pt()))
            .unwrap_or_else(|| panic!("{} is in neither archive", id.pt()));
        let bytes = library.read(id.pt()).unwrap();
        let aircraft =
            Aircraft::parse(&bytes).unwrap_or_else(|error| panic!("{}: {error}", id.pt()));
        for name in HIDDEN {
            assert!(
                !id.mounts_gun(name),
                "{} must not claim to mount {name}",
                id.pt()
            );
            assert!(
                !aircraft
                    .hardpoints
                    .iter()
                    .any(|h| h.store.as_deref() == Some(name)),
                "{} decoded station list names {name}",
                id.pt()
            );
        }
    }
}
