//! Headless import check (slice JT-6, reworked): the ASROC record, its one
//! carrier the Ticonderoga, and how widely that ship is placed. ASROC's
//! guidance word is the air-target flag with the support flag 0x200, so the
//! record decodes as a supported-radar missile, exactly like the SA-6 whose
//! guidance word it shares. John kept it held on 2026-11-06: it is an
//! anti-submarine weapon, there is no submarine target, and the SA-6 copy
//! looks like a placeholder. The Ticonderoga carries the fitted derived SM-2
//! on its two ASROC rails instead (docs/spec/surface-defenses.md, "SAM
//! missiles").
//!
//! The check reads the user's own install (the `gameassets` link or
//! `TORE_GAME_DIR`) through the importer's media source and skips quietly
//! without it. It reads records only; no retail bytes are committed.
//! Evidence: docs/formats/aircraft-ordnance.md, "ASROC (JT-6)".
use std::{collections::BTreeSet, path::PathBuf};
use tore_formats::{surface_unit::SurfaceUnit, weapons::Weapon};
use tore_import::MediaSource;

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

#[test]
fn the_asroc_record_decodes_as_an_sa6_copy_the_ticonderoga_carries() {
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

    // The record: the air-target flag with support (never the surface flag),
    // seeker signature 3, the SA-6 launch envelope.
    let asroc = libraries
        .iter()
        .find(|library| library.entries.contains_key("ASROC.JT"))
        .map(|library| Weapon::parse("ASROC.JT", &library.read("ASROC.JT").unwrap()).unwrap())
        .expect("ASROC.JT is in FA_1 or FA_2");
    assert_eq!(asroc.source, "ASROC.JT");
    assert_eq!(asroc.flags & GUIDED, GUIDED, "a guided missile");
    assert_eq!(asroc.flags & AIR_TARGET, AIR_TARGET, "aims at aircraft");
    assert_eq!(
        asroc.flags & SURFACE_TARGET,
        0,
        "never aims at ships or the surfaced OSCAR"
    );
    assert_eq!(asroc.flags & SUPPORT, SUPPORT, "the launcher supports it");
    assert_eq!(asroc.seeker.signature, 3, "radar seeker");
    assert_eq!(
        asroc.seeker_kind(),
        tore_formats::weapons::SeekerKind::SemiActiveRadar
    );
    assert_eq!(asroc.movement.initial_speed, 0, "starts from rest");

    let zone0 = asroc.seeker.zones[0];
    let zone1 = asroc.seeker.zones[1];
    assert_eq!(zone0.maximum_range, 100_000, "seeker range");
    assert_eq!(zone0.minimum_altitude, 500, "seeker floor");
    assert_eq!((zone1.minimum_range, zone1.maximum_range), (500, 75_000));
    assert_eq!(
        (zone1.minimum_altitude, zone1.maximum_altitude),
        (1_000, 30_000)
    );
    let movement = asroc.movement;
    assert_eq!(
        (movement.ignite_t, movement.fuel_t, movement.remove_t),
        (8, 88, 160)
    );
    assert_eq!(zone0.heading, 32767, "wide acquisition cone");
    assert_eq!(zone1.heading, 8190, "45 degree launch cone");
    assert_eq!(zone1.pitch, 8190, "45 degree launch cone");

    // Carriers: exactly the Ticonderoga, on two ASROC rails beside two
    // Phalanx. No other unit and no aircraft names the record.
    let mut carriers: BTreeSet<String> = BTreeSet::new();
    let mut ticon_mounts: Vec<(usize, String)> = Vec::new();
    for library in libraries {
        for name in library.entries.keys() {
            if !name.to_ascii_uppercase().ends_with(".NT") {
                continue;
            }
            let unit = SurfaceUnit::parse(&library.read(name).unwrap()).unwrap();
            for (index, mount) in unit.weapons() {
                if mount.store.as_deref() == Some("ASROC.JT") {
                    carriers.insert(unit.resource.to_ascii_uppercase());
                    if unit.resource.eq_ignore_ascii_case("TICON.NT") {
                        ticon_mounts.push((index, "ASROC.JT".to_owned()));
                    }
                }
            }
        }
    }
    assert_eq!(
        carriers,
        ["TICON.NT".to_owned()].into_iter().collect(),
        "only the Ticonderoga carries ASROC"
    );
    assert_eq!(ticon_mounts.len(), 2, "two ASROC rails, like retail");

    // No aircraft's decoded station list names it: ASROC is a ship record.
    let mut aircraft_naming: BTreeSet<String> = BTreeSet::new();
    for library in libraries {
        for name in library.entries.keys() {
            if !name.to_ascii_uppercase().ends_with(".PT") {
                continue;
            }
            let bytes = library.read(name).unwrap().to_ascii_lowercase();
            let needle = b"string \"asroc.jt\"";
            if bytes.windows(needle.len()).any(|window| window == needle) {
                aircraft_naming.insert(name.to_ascii_uppercase());
            }
        }
    }
    assert!(
        aircraft_naming.is_empty(),
        "no aircraft is built with ASROC, found {aircraft_naming:?}"
    );

    // The Ticonderoga is placed by far more missions than the first reading
    // found: count every .M that names it. All of them live in FA_2.LIB.
    let mut missions: BTreeSet<String> = BTreeSet::new();
    for library in libraries {
        for name in library.entries.keys() {
            if !name.to_ascii_uppercase().ends_with(".M") {
                continue;
            }
            let bytes = library.read(name).unwrap().to_ascii_uppercase();
            if bytes.windows(8).any(|window| window == b"TICON.NT") {
                missions.insert(name.to_ascii_uppercase());
            }
        }
    }
    assert_eq!(
        missions.len(),
        181,
        "every retail .M that places the Ticonderoga (JT-6 rework count)"
    );
}
