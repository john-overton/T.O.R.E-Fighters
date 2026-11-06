//! Headless import check (slice JT-2): the decoded air-to-air missiles nothing
//! loads. AA6, AA9, PL7 and PL10 decode from `FA_2.LIB` with a guided seeker
//! class, but no registered aircraft lists one as a decoded default store and
//! `combat::missiles::Profile` has no reviewed flight profile for them, so the
//! ordnance catalog keeps them hidden.
//!
//! The check reads the user's own install (the `gameassets` link or
//! `TORE_GAME_DIR`) through the importer's media source and skips quietly
//! without it. It reads records only; no retail bytes are committed.
//! Evidence: docs/formats/aircraft-ordnance.md, "Air-to-air missiles nothing
//! loads".
use std::path::PathBuf;
use tore_formats::{
    aircraft::{Aircraft, AircraftId},
    weapons::{SeekerKind, Weapon},
};
use tore_import::MediaSource;

/// The four decoded air-to-air records the ordnance catalog hides (slice JT-2).
const UNLOADED: [&str; 4] = ["AA6.JT", "AA9.JT", "PL7.JT", "PL10.JT"];

/// The reviewed seeker classification from the record's own signature and
/// support flag. AA6, AA9 and PL10 are semi-active radar; PL7 is infrared.
fn expected_kind(name: &str) -> SeekerKind {
    match name {
        "PL7.JT" => SeekerKind::Infrared,
        _ => SeekerKind::SemiActiveRadar,
    }
}

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

#[test]
fn decoded_air_to_air_records_are_classified_and_mounted_by_no_registered_aircraft() {
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

    // Each record decodes with its reviewed seeker class and the normal-store
    // compatibility flag (bit 2 set), unlike the JT-1 cannon records.
    for name in UNLOADED {
        let library = libraries
            .iter()
            .find(|library| library.entries.contains_key(name))
            .unwrap_or_else(|| panic!("{name} is in neither archive"));
        let bytes = library.read(name).unwrap();
        let weapon = Weapon::parse(name, &bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(weapon.source, name, "{name} names itself");
        assert_eq!(
            weapon.seeker_kind(),
            expected_kind(name),
            "{name} seeker class"
        );
        assert_ne!(
            weapon.flags & 0x2,
            0,
            "{name} must keep normal-store compatibility"
        );
    }

    // No registered aircraft is built with one as a decoded default store. The
    // decoded default carriers (MiG-25, MiG-31, Q-5; none for PL10) are outside
    // `AircraftId::ALL`, so the roster can only reach them through a
    // source-compatible station, never a default.
    for id in AircraftId::ALL {
        let library = libraries
            .iter()
            .find(|library| library.entries.contains_key(id.pt()))
            .unwrap_or_else(|| panic!("{} is in neither archive", id.pt()));
        let bytes = library.read(id.pt()).unwrap();
        let aircraft =
            Aircraft::parse(&bytes).unwrap_or_else(|error| panic!("{}: {error}", id.pt()));
        for name in UNLOADED {
            assert!(
                !aircraft
                    .hardpoints
                    .iter()
                    .any(|h| h.store.as_deref() == Some(name)),
                "{} decoded default store list names {name}",
                id.pt()
            );
        }
    }
}
