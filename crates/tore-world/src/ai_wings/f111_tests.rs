//! F111-7 retail guard (2026-11-16): the AI F-111F's designator, read from the
//! user's own install (the `gameassets` link or `TORE_GAME_DIR`) through the
//! importer's media source. It skips quietly without one and reads records
//! only; no retail bytes are committed.
//!
//! The F-111F's laser is two records of one sensor: the fixed `PAVESPK.SEE`
//! row (its laser, shown as the PAVE TACK) and the Pave Tack pod its standard
//! load hangs on the bay row (F111-5). Both pilots take the one configuration
//! (`Configuration::from_source`) and the one sight rule
//! (`Configuration::sight_sensor`, the loaded pod first and the fixed row
//! otherwise), so these tests pin that an AI F-111F counts as a designator
//! with the pod aboard and without it, as the human's sight does, and that the
//! pod rides the bay row on which the bomb load is its alternative.
use super::tests::build;
use super::*;
use crate::test_support::combat_fixture;
use std::path::PathBuf;
use tore_formats::aircraft::Aircraft;
use tore_import::MediaSource;
use tore_sim::combat::sight::Gimbal;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

/// The real F-111F record and its default configuration, or `None` without a
/// retail install.
fn retail_f111() -> Option<(Aircraft, live::Configuration)> {
    let media = MediaSource::detect(&install()).ok()?;
    let archives: Vec<_> = ["FA_1.LIB", "FA_2.LIB", "FA_4B.LIB", "FA_4D.LIB"]
        .iter()
        .filter_map(|name| media.archive(name).ok())
        .collect();
    let read = |name: &str| -> std::io::Result<Vec<u8>> {
        archives
            .iter()
            .find_map(|library| library.read(name).ok())
            .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
    };
    let bytes = read(AircraftId::F111.pt()).ok()?;
    let aircraft = Aircraft::parse_as(&bytes, AircraftId::F111).expect("F111.PT parses");
    let config = live::Configuration::from_source(&aircraft, &read).expect("F111.PT loads");
    Some((aircraft, config))
}

/// The standard load hangs the Pave Tack on the bay row, and an AI aircraft
/// built from it lases for itself and for its wing: the sight's sensor is the
/// pod's hemisphere laser, `can_lase` and the ground-order capability say so,
/// and the strike class is Attack (the override, BMB-B1).
#[test]
fn an_ai_f111_with_its_bay_pod_is_a_designator() {
    let Some((aircraft, config)) = retail_f111() else {
        eprintln!("skipped: no retail install at {}", install().display());
        return;
    };
    assert_eq!(
        tore_sim::models::strike_class::StrikeClass::of_aircraft(&aircraft),
        tore_sim::models::strike_class::StrikeClass::Attack
    );
    let pod = config.designator_pod().expect("the standard load's pod");
    assert_eq!(pod.source, "PAVESPK.SEE");
    assert!(pod.sensor.laser);
    assert_eq!(config.designator_pods.len(), 1);
    // The pod hangs on the bay row's station, the one the bomb load displaces.
    let bay = tore_formats::aircraft::stations::F111_BAY;
    let slot = config.designator_pods[0].station;
    assert_eq!(config.hardpoint_slots[bay], Some(slot));
    // The sight takes the pod first (a hemisphere gimbal), and it lases.
    let sensor = config.sight_sensor().expect("a sight");
    assert!(sensor.laser);
    assert_eq!(sensor.gimbal, Gimbal::Hemisphere);

    let (mut wings, _) = build(None);
    let _ = combat_fixture(false);
    let id = wings.mission().actors()[1].id();
    wings.configs.insert(id, config.clone());
    assert!(wings.can_lase(id), "the AI F-111F flies the lasing hold");
    let capability = wings.capability(id, false);
    assert!(capability.designator, "{capability:?}");
    assert!(capability.strike, "{capability:?}");
}

/// Take the pod off (jettisoned, or displaced by two bombs on the bay row) and
/// the fixed Pave Spike row is still a laser, so the AI F-111F goes on being a
/// designator, with the fixed row's cone rather than the pod's hemisphere.
#[test]
fn an_ai_f111_without_its_pod_still_lases_from_the_fixed_row() {
    let Some((_, mut config)) = retail_f111() else {
        eprintln!("skipped: no retail install at {}", install().display());
        return;
    };
    config.designator_pods.clear();
    let sensor = config.sight_sensor().expect("the fixed row's sight");
    assert!(sensor.laser, "the fixed PAVESPK.SEE row lases");
    let (mut wings, _) = build(None);
    let id = wings.mission().actors()[1].id();
    wings.configs.insert(id, config);
    assert!(wings.can_lase(id));
    assert!(wings.capability(id, false).designator);
}
