//! EQ-1 retail guard (2026-11-06): every SEE, ECM and GAS record in the user's
//! install decodes into a capability profile, and a radar or ECM record whose
//! tuning is unreviewed loads as unreviewed without enabling a flying channel.
//!
//! The check reads the user's own install (the `gameassets` link or
//! `TORE_GAME_DIR`) through the importer's media source and skips quietly
//! without it. It reads records only; no retail bytes are committed. Evidence:
//! docs/research/m3-asset-inventory.md, "Equipment", and
//! crates/tore-sim/src/sensors/profile.rs.
use std::collections::BTreeSet;
use std::path::PathBuf;
use tore_formats::weapons::{Countermeasures, Seeker, Tank};
use tore_import::MediaSource;
use tore_sim::sensors::profile::{JammerProfile, PassiveProfile, RadarProfile};

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

#[test]
fn every_equipment_record_decodes_into_a_profile() {
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
    let names: BTreeSet<String> = libraries
        .iter()
        .flat_map(|library| library.entries.keys().cloned())
        .filter(|name| {
            let upper = name.to_ascii_uppercase();
            upper.ends_with(".SEE") || upper.ends_with(".ECM") || upper.ends_with(".GAS")
        })
        .collect();

    let mut reviewed_radar = 0usize;
    let mut unreviewed_radar = 0usize;
    let mut reviewed_ecm = 0usize;
    let mut unreviewed_ecm = 0usize;
    for name in &names {
        let library = libraries
            .iter()
            .find(|library| library.entries.contains_key(name))
            .unwrap();
        let bytes = library.read(name).unwrap();
        let upper = name.to_ascii_uppercase();
        if upper.ends_with(".GAS") {
            Tank::parse(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
            continue;
        }
        if upper.ends_with(".ECM") {
            let ecm = Countermeasures::parse(name, &bytes)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            let profile = JammerProfile::from_countermeasures(name, &ecm)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(profile.record, upper, "{name} names itself");
            if profile.reviewed() {
                reviewed_ecm += 1;
            } else {
                unreviewed_ecm += 1;
            }
            continue;
        }
        let seeker = Seeker::parse(name, &bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        match seeker.signature {
            3 => {
                let profile = RadarProfile::from_seeker(name, &seeker)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
                assert_eq!(profile.record, upper, "{name} names itself");
                if profile.reviewed() {
                    reviewed_radar += 1;
                } else {
                    unreviewed_radar += 1;
                }
            }
            // The infrared and visual channels load the same way.
            2 => {
                PassiveProfile::from_seeker(name, &seeker, 2)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
            }
            0 => {
                PassiveProfile::from_seeker(name, &seeker, 0)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
            }
            // A laser designator, read by `combat::sight`, not a scope channel.
            1 => {}
            other => panic!("{name}: unexpected signature {other}"),
        }
    }

    // The inventory's equipment census (docs/research/m3-asset-inventory.md).
    assert_eq!(names.len(), 85, "the equipment record count");
    assert_eq!(reviewed_radar + unreviewed_radar, 35, "radar records");
    assert_eq!(reviewed_ecm + unreviewed_ecm, 30, "ECM records");
    assert!(reviewed_radar > 0 && reviewed_ecm > 0);
    assert!(unreviewed_radar > 0 && unreviewed_ecm > 0);
    println!(
        "equipment: {} records, radar {reviewed_radar} reviewed / {unreviewed_radar} unreviewed, \
         ECM {reviewed_ecm} reviewed / {unreviewed_ecm} unreviewed",
        names.len()
    );
}
