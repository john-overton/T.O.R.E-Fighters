//! Headless import check (slice JT-4): the four decoded air-to-air records
//! enabled on John's request of 2026-11-06. AA6, AA9, PL7 and PL10 now have
//! reviewed flight profiles and are in `SUPPORTED_WEAPONS`, so a compatible
//! ported station can load them. Their decoded default carriers are still
//! outside the roster, so no imported aircraft is built with one.
//!
//! This check reads the user's own install (the `gameassets` link or
//! `TORE_GAME_DIR`) through the importer's media source and skips quietly
//! without it. It reads records only; no retail bytes are committed.
//! The numbers are the catalog rows in docs/spec/missiles.md, "First-pass
//! inventory matrix", not real-world performance claims.
use std::path::PathBuf;
use tore_formats::{
    aircraft::{Aircraft, AircraftId},
    weapons::{SeekerKind, Weapon},
};
use tore_import::MediaSource;

const NMI: f64 = 6076.;

/// A source range in feet, shown in nautical miles rounded like the spec.
fn nautical(feet: i32) -> f64 {
    (f64::from(feet) / NMI * 100.).round() / 100.
}

/// The reviewed spec row for one enabled record.
struct Spec {
    name: &'static str,
    hud: &'static str,
    seeker: SeekerKind,
    launch_min_nmi: f64,
    launch_max_nmi: f64,
    ignition_s: f64,
    burn_s: f64,
    removal_s: f64,
    seeker_max_nmi: f64,
}

/// docs/spec/missiles.md, "First-pass inventory matrix" (2026-11-06). Half
/// angles are 45/45 on all four.
const SPECS: [Spec; 4] = [
    Spec {
        name: "AA6.JT",
        hud: "AA-6A",
        seeker: SeekerKind::SemiActiveRadar,
        launch_min_nmi: 1.97,
        launch_max_nmi: 14.81,
        ignition_s: 1.,
        burn_s: 39.,
        removal_s: 80.,
        seeker_max_nmi: 14.81,
    },
    Spec {
        name: "AA9.JT",
        hud: "AA-9",
        seeker: SeekerKind::SemiActiveRadar,
        launch_min_nmi: 1.97,
        launch_max_nmi: 41.15,
        ignition_s: 2.,
        burn_s: 130.,
        removal_s: 264.,
        seeker_max_nmi: 41.15,
    },
    Spec {
        name: "PL10.JT",
        hud: "PL10-R",
        seeker: SeekerKind::SemiActiveRadar,
        launch_min_nmi: 1.32,
        launch_max_nmi: 24.69,
        ignition_s: 2.,
        burn_s: 18.,
        removal_s: 40.,
        seeker_max_nmi: 24.69,
    },
    Spec {
        name: "PL7.JT",
        hud: "PL-7",
        seeker: SeekerKind::Infrared,
        launch_min_nmi: 0.66,
        launch_max_nmi: 3.95,
        ignition_s: 0.,
        burn_s: 4.,
        removal_s: 20.,
        seeker_max_nmi: 8.23,
    },
];

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

#[test]
fn decoded_air_to_air_records_match_the_reviewed_spec_rows() {
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

    for spec in &SPECS {
        let library = libraries
            .iter()
            .find(|library| library.entries.contains_key(spec.name))
            .unwrap_or_else(|| panic!("{} is in neither archive", spec.name));
        let bytes = library.read(spec.name).unwrap();
        let weapon = Weapon::parse(spec.name, &bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", spec.name));
        assert_eq!(weapon.source, spec.name, "{} names itself", spec.name);
        assert_eq!(
            weapon.seeker_kind(),
            spec.seeker,
            "{} seeker class",
            spec.name
        );
        let hud = tore_formats::weapons::corrections::identity(spec.name)
            .map_or_else(|| weapon.hud_name.clone(), |(hud, _)| hud.to_string());
        assert_eq!(hud, spec.hud, "{} HUD label", spec.name);

        // The four are ordinary stores, not built-in-gun style records.
        assert_ne!(weapon.flags & 0x2, 0, "{} compatibility bit", spec.name);

        let zone = weapon.seeker.zones[1];
        assert_eq!(
            nautical(zone.minimum_range),
            spec.launch_min_nmi,
            "{} launch minimum",
            spec.name
        );
        assert_eq!(
            nautical(zone.maximum_range),
            spec.launch_max_nmi,
            "{} launch maximum",
            spec.name
        );
        assert_eq!(
            nautical(weapon.seeker.zones[0].maximum_range),
            spec.seeker_max_nmi,
            "{} seeker maximum",
            spec.name
        );
        // The acquisition cone is zone 0 (crates/tore-sim/src/combat/missiles/seeker.rs);
        // the launch envelope cone is zone 1. The spec's half-angle column is the
        // seeker's, so it is zone 0.
        assert_eq!(
            weapon.seeker.zones[0].heading, 8190,
            "{} horizontal half angle",
            spec.name
        );
        assert_eq!(
            weapon.seeker.zones[0].pitch, 8190,
            "{} vertical half angle",
            spec.name
        );

        let movement = weapon.movement;
        assert_eq!(
            f64::from(movement.ignite_t) / 4.,
            spec.ignition_s,
            "{} ignition",
            spec.name
        );
        assert_eq!(
            f64::from(movement.fuel_t - movement.ignite_t) / 4.,
            spec.burn_s,
            "{} burn",
            spec.name
        );
        assert_eq!(
            f64::from(movement.remove_t) / 4.,
            spec.removal_s,
            "{} removal",
            spec.name
        );
    }

    // No registered aircraft is built with one as a decoded default store. The
    // decoded default carriers (MiG-25, MiG-31, Q-5; none for PL10) are
    // outside `AircraftId::ALL`, so a player reaches these records only
    // through a source-compatible station.
    for id in AircraftId::ALL {
        let library = libraries
            .iter()
            .find(|library| library.entries.contains_key(id.pt()))
            .unwrap_or_else(|| panic!("{} is in neither archive", id.pt()));
        let bytes = library.read(id.pt()).unwrap();
        let aircraft =
            Aircraft::parse(&bytes).unwrap_or_else(|error| panic!("{}: {error}", id.pt()));
        for spec in &SPECS {
            assert!(
                !aircraft
                    .hardpoints
                    .iter()
                    .any(|h| h.store.as_deref() == Some(spec.name)),
                "{} decoded default store list names {}",
                id.pt(),
                spec.name
            );
        }
    }
}
