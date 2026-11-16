//! F117-9 retail guard (2026-11-15): the AI behaviour of the F-117A, read from
//! the user's own install (the `gameassets` link or `TORE_GAME_DIR`) through
//! the importer's media source. It skips quietly without one and reads records
//! only; no retail bytes are committed. The behaviour half (the strike pass,
//! the lase, the landing, the fight with fighters) is the battery's:
//! `ai-strike-f117`, `ai-f117-*` and `docs/formats/aircraft-variety.md`,
//! "F-117A in missions, multiplayer and replay".
//!
//! The Tornado's precedent (TOR-9, open item 3) was an AI aircraft whose
//! release left from a position its human twin's did not. Both pilots take the
//! one configuration (`Configuration::from_source`) and the one release
//! (`State::release_strike` against `State::release_round`), so these tests
//! pin that the F-117's two bay stations leave their bombs from the bay's own
//! mounts when an AI flies it, with the doors shut.
use super::tests::build;
use super::*;
use crate::test_support::combat_fixture;
use std::path::PathBuf;
use tore_formats::aircraft::Aircraft;
use tore_import::MediaSource;
use tore_sim::ai::weapon_service::{RequestId, StationId};
use tore_sim::combat::{bays, hardpoints::Location, hardpoints::MAGAZINE_BYTE};

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

/// The real F-117A record and its configuration, or `None` without a retail
/// install.
fn retail_f117() -> Option<(Aircraft, live::Configuration)> {
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
    let bytes = read(AircraftId::F117.pt()).ok()?;
    let aircraft = Aircraft::parse_as(&bytes, AircraftId::F117).expect("F117.PT parses");
    let config = live::Configuration::from_source(&aircraft, &read).expect("F117.PT loads");
    Some((aircraft, config))
}

fn launch(actor: u32, station: u8, target: u32) -> LaunchEvent {
    LaunchEvent {
        actor,
        station: StationId(station),
        target,
        request_id: RequestId(1),
        projectiles: 1,
    }
}

/// The AI's view of the F-117A: bombing class Attack (no override, John,
/// 2026-11-15, decision 28), two one-store bay stations, every store a surface
/// store, no gun and no air-to-air missile, no radar. So no store is eligible
/// against an aircraft and the AI has nothing to engage a fighter with.
#[test]
fn an_ai_f117_is_an_attack_class_striker_with_no_store_for_an_aircraft() {
    let Some((aircraft, config)) = retail_f117() else {
        eprintln!("skipped: no retail install at {}", install().display());
        return;
    };
    assert_eq!(
        tore_sim::models::strike_class::StrikeClass::of_aircraft(&aircraft),
        tore_sim::models::strike_class::StrikeClass::Attack
    );
    assert_eq!(AircraftId::F117.radar(), None);
    assert!(config.gun_pods.is_empty());
    assert!(
        config
            .stations
            .iter()
            .all(|s| !live::is_gun(&s.weapon) && live::is_strike_store(&s.weapon)),
        "every store is a bomb"
    );
    let specs = station_specs(&config, false);
    assert_eq!(specs.len(), 2, "two bay stations, nothing else");
    for spec in &specs {
        assert!(!spec.capability.air, "{:?}", spec.capability);
        assert!(spec.capability.surface, "{:?}", spec.capability);
        assert_eq!(
            spec.rounds(),
            tore_sim::ai::weapon_service::Rounds::Finite(1),
            "one GBU-27 a bay"
        );
    }
    for station in &config.stations {
        assert_eq!(station.weapon.source, "PAVEWA3.JT");
        assert_eq!(station.rack.location(), Location::Magazine);
        assert_eq!(station.rack.location().byte(), MAGAZINE_BYTE);
    }
}

/// An AI F-117 lets a GBU-27 go from each bay station's own mount (3.0 ft
/// either side of the centreline, inside the bay's opening), not from the
/// record's raw hardpoint position, with its doors shut (the AI's release
/// waits for no door, as the F-22's).
#[test]
fn an_ai_f117_bomb_leaves_from_each_bay_station_with_the_doors_shut() {
    let Some((_, config)) = retail_f117() else {
        eprintln!("skipped: no retail install at {}", install().display());
        return;
    };
    let opening = bays::opening(AircraftId::F117).expect("the F-117's belly opening");
    let (mut wings, _) = build(None);
    let mut state = combat_fixture(false);
    state.projectiles.clear();
    let id = wings.mission().actors()[1].id();
    assert!(
        !wings.mission().actor(id).unwrap().flight().bay_available()
            || wings.mission().actor(id).unwrap().flight().bay == 0.,
        "the fixture's doors are shut"
    );
    wings.configs.insert(id, config.clone());
    let launcher = crate::combat::launcher(wings.mission().actor(id).unwrap().flight());
    let ground = |_: f64, _: f64| 0.;
    let mut sides = Vec::new();
    for (index, station) in config.stations.iter().enumerate() {
        state.projectiles.clear();
        let weapon = station.weapon.clone();
        let (row, want) = (
            4 + index,
            bays::station(AircraftId::F117, 4 + index).expect("a reviewed bay row"),
        );
        // Both pilots read the record's bay row through `placed_on`.
        assert_eq!(station.mount, want, "station {index} is bay row {row}");
        let emitted = wings.release(
            &launch(id, index as u8, 0x5000_0001),
            &mut state,
            &weapon,
            &[],
            &ground,
        );
        let projectiles = &state.projectiles;
        assert_eq!(
            projectiles.len(),
            emitted as usize,
            "station {index}: one released bomb, none dropped"
        );
        assert_eq!(emitted, 1, "station {index} lets its one GBU-27 go");
        let p = &projectiles[0];
        assert_eq!(p.owner, id);
        assert_eq!(
            p.weapon.as_ref().map(|w| w.source.as_str()),
            Some("PAVEWA3.JT")
        );
        // The fixture flies level with the basis the ownship has, so the
        // offset from the aircraft is the station's own release point.
        let offset = [
            p.position[0] - launcher.position[0],
            p.position[1] - launcher.position[1],
            p.position[2] - launcher.position[2],
        ];
        let point = station.release_point(1);
        for axis in 0..3 {
            assert!(
                (offset[axis] - point[axis]).abs() < 1e-6,
                "station {index} axis {axis}: {offset:?} against {point:?}"
            );
        }
        // And that point is the bay's: 3.0 ft out on its own side, the store's
        // 5.4 ft fin span between the bay's walls, its length between the ends.
        let sign = if index == 0 { -1. } else { 1. };
        assert!(
            (point[0] - sign * bays::F117_BAY_X).abs() < 1e-9,
            "{point:?}"
        );
        assert!(point[0] * sign - 2.7 > opening.inner && opening.outer > point[0] * sign + 2.7);
        assert!(point[2] - 7.94 > opening.aft && point[2] + 5.96 < opening.fore);
        assert!(point[1] > opening.belly && point[1] < opening.belly + opening.depth);
        sides.push(point[0].signum());
    }
    assert_eq!(sides, [-1., 1.], "one bomb from each side of the keel");
}
