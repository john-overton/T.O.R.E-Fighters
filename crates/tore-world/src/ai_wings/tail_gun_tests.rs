//! B52-6 (2026-11-15): the AI's slewed gun, the B-52's tail gun. A gun row
//! whose record points it astern (`slewH` 32,760, 30 degree arcs) is a
//! defensive gun for an AI aircraft: it engages a fighter inside its rear
//! arc, swings its barrel to the lead point, and never turns the aircraft to
//! lay it. A forward gun keeps the nose-laid rules. The synthetic half builds
//! on the same fixtures as the physical gun tests of `ai_wings`; the retail
//! half reads the user's own `B52.PT` and skips without an install.
use super::tests::{build, flat, player_object};
use super::*;
use std::path::PathBuf;
use tore_formats::aircraft::Aircraft;
use tore_import::MediaSource;
use tore_sim::ai::{
    gunnery,
    weapon_service::{RequestId, StationId},
    wing::{TargetOrder, WingRequest},
};
use tore_sim::combat::slew::Slew;

fn tail_gun() -> Slew {
    Slew::from_record([32_760, 0, 5_460, 5_460])
}

/// An AI aircraft (actor 3) at `at`, flying along +z with one M61 station
/// of three rounds on `slew`, ordered to free selection and advanced one
/// step with the player at `player`.
fn gunner(
    slew: Slew,
    at: Vector,
    player: Vector,
) -> (AiWings, Vec<live::Target>, tore_formats::weapons::Weapon) {
    let (mut wings, mut targets) = build(None);
    for actor in wings.mission.actors_mut() {
        actor.set_stations(Vec::new());
    }
    let mut gun = crate::test_support::combat_fixture(false)
        .own()
        .configuration()
        .stations[0]
        .weapon
        .clone();
    gun.source = "M61.JT".into();
    let actor = wings.mission.actor_mut(3).unwrap();
    let mut stations = simple_stations(0, 3, AI_STORE_SPEED);
    stations[0].slew = slew;
    // The B-52's real gun row has no angle limit of its own (the arcs are the slew's).
    stations[0].employment_limit_deg = None;
    actor.set_stations(stations);
    actor.set_guns([(1, gun.clone())].into());
    actor.flight_mut().position = at;
    actor.flight_mut().yaw = 0.;
    wings
        .mission
        .order(3, WingRequest::TargetAssignment(TargetOrder::FreeSelection))
        .unwrap()
        .unwrap();
    wings
        .advance(player_object(player), &mut targets, &flat)
        .unwrap();
    (wings, targets, gun)
}

fn event(target: u32) -> LaunchEvent {
    LaunchEvent {
        actor: 3,
        station: StationId(1),
        target,
        request_id: RequestId(7),
        projectiles: 1,
    }
}

/// The fixture's one fighter (id 1) sits at (0, 20,000, 0) whatever the player
/// argument says, so each test places the gunner (actor 3) around it.
///
/// A fighter 1,500 ft astern is inside the tail gun's arcs: the round leaves
/// astern, along the lead to the fighter, and the aircraft is not turned.
#[test]
fn an_ai_tail_gun_engages_a_fighter_astern_and_fires_back_at_it() {
    let at = [0., 20_000., 1_500.];
    let (mut wings, _, gun) = gunner(tail_gun(), at, [0., 20_000., 0.]);
    let actor = wings.mission.actor(3).unwrap();
    let observed = actor.gun_target().expect("the fighter is observed");
    let solution = gunnery::solve_mounted(
        &gun,
        &crate::combat::launcher(actor.flight()),
        actor.stations()[0].mount,
        tail_gun(),
        observed,
    )
    .unwrap();
    assert!(solution.aligned, "astern is inside the arcs");
    assert!(solution.aim.direction[2] < -0.95, "{:?}", solution.aim);
    let yaw = actor.flight().yaw;
    let mut rounds = Vec::new();
    let emitted = wings.realise(&event(observed.id), &mut rounds, &gun, 1, &[], Some(0));
    assert_eq!(emitted, 1, "the tail gun fires");
    assert_eq!(wings.mission.actor(3).unwrap().rounds_remaining(), 2);
    let direction = rounds[0].direction;
    assert!(direction[2] < -0.95, "fired astern: {direction:?}");
    // BMB-AI: the barrel is laid by a gunner with an aim error and a lag, so the
    // round leaves near the exact lead (a few degrees at most), not along it.
    let off = direction
        .iter()
        .zip(solution.aim.direction)
        .map(|(got, want)| got * want)
        .sum::<f64>()
        .clamp(-1., 1.)
        .acos()
        .to_degrees();
    assert!(off < 6., "{off} degrees off the lead");
    assert!(off > 0., "the exact lead is no longer fired along");
    assert_eq!(wings.mission.actor(3).unwrap().flight().yaw, yaw);
}

/// The same fighter 1,500 ft ahead is outside the arcs: the tail gun does
/// not fire, and a forward gun (no slew) with the fighter behind it does not
/// fire either (its rule is unchanged: it must be laid on the target).
#[test]
fn an_ai_tail_gun_holds_its_fire_ahead_and_a_forward_gun_is_unchanged_astern() {
    let (mut ahead, _, gun) = gunner(tail_gun(), [0., 20_000., -1_500.], [0., 20_000., 0.]);
    let observed = ahead.mission.actor(3).unwrap().gun_target().unwrap();
    let mut rounds = Vec::new();
    assert_eq!(
        ahead.realise(&event(observed.id), &mut rounds, &gun, 1, &[], Some(0)),
        0
    );
    assert!(rounds.is_empty());
    let (mut forward, _, gun) = gunner(Slew::default(), [0., 20_000., 1_500.], [0., 20_000., 0.]);
    let observed = forward.mission.actor(3).unwrap().gun_target().unwrap();
    assert_eq!(
        forward.realise(&event(observed.id), &mut rounds, &gun, 1, &[], Some(0)),
        0
    );
    assert_eq!(forward.mission.actor(3).unwrap().rounds_remaining(), 3);
}

/// Where the gunner flies so the fighter at (0, 20,000, 0) is `degrees` off
/// its tail (to its right) and `range` ft away.
fn bearing_from_tail(degrees: f64, range: f64) -> Vector {
    let a = degrees.to_radians();
    [-range * a.sin(), 20_000., range * a.cos()]
}

/// The whole path, not just the release: the controller picks the tail gun
/// for a fighter behind the AI aircraft, asks for rounds, and never turns the
/// aircraft to lay it. The same AI aircraft with a forward gun and the fighter
/// behind it asks for none in the same time.
#[test]
fn the_ai_controller_asks_for_tail_gun_rounds_against_a_fighter_behind_it() {
    let launches = |slew: Slew| {
        let (mut wings, mut targets, _) = gunner(slew, [0., 20_000., 1_500.], [0., 20_000., 0.]);
        let held = wings.mission.actor(3).unwrap().flight().clone();
        let mut asked = 0;
        for _ in 0..240 {
            *wings.mission.actor_mut(3).unwrap().flight_mut() = held.clone();
            let output = wings
                .advance(player_object([0., 20_000., 0.]), &mut targets, &flat)
                .unwrap();
            asked += output
                .launches
                .iter()
                .filter(|e| e.actor == 3 && e.station == StationId(1))
                .count();
            // Flown back to where it was, the aircraft's yaw is the held
            // one: the controller's steering is not what the test measures.
        }
        asked
    };
    assert!(launches(tail_gun()) > 0, "the tail gun asks for rounds");
    assert_eq!(launches(Slew::default()), 0, "a forward gun cannot");
}

/// A fighter 10 degrees off the tail is inside the 30 degree arcs and 40
/// degrees off is not (the lead of a fighter closing at 760 ft/s moves each
/// a little further round, 16 and 59 degrees).
#[test]
fn an_ai_tail_gun_does_not_reach_past_its_arcs() {
    for (degrees, fires) in [(10., true), (40., false)] {
        let at = bearing_from_tail(degrees, 3_000.);
        let (mut wings, _, gun) = gunner(tail_gun(), at, [0., 20_000., 0.]);
        let observed = wings.mission.actor(3).unwrap().gun_target().unwrap();
        let mut rounds = Vec::new();
        let emitted = wings.realise(&event(observed.id), &mut rounds, &gun, 1, &[], Some(0));
        assert_eq!(emitted == 1, fires, "{degrees} degrees off the tail");
    }
}

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

/// The real `B52.PT` row 4 reads as a tail gun in the combat configuration,
/// and the AI's station spec carries it (read from the user's install).
#[test]
fn the_retail_b52_gun_row_is_a_slewed_tail_gun_in_the_configuration_and_the_ai_spec() {
    let Ok(media) = MediaSource::detect(&install()) else {
        eprintln!("skipped: no retail install at {}", install().display());
        return;
    };
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
    let bytes = read(AircraftId::B52.pt()).expect("B52.PT");
    let aircraft = Aircraft::parse_as(&bytes, AircraftId::B52).expect("B52.PT parses");
    let config = live::Configuration::from_source(&aircraft, &read).expect("B52.PT loads");
    let gun = config
        .stations
        .iter()
        .position(|s| s.weapon.source.eq_ignore_ascii_case("M61.JT"))
        .expect("the tail gun");
    let slew = config.stations[gun].slew;
    assert!(slew.is_slewed());
    assert!((slew.rest[0] - std::f64::consts::PI).abs() < 1e-9);
    assert!((slew.limit[0] - 30_f64.to_radians()).abs() < 1e-9);
    // The Harpoons and the bay rows are not guns: they carry no slew.
    for (index, station) in config.stations.iter().enumerate() {
        if index != gun {
            assert!(!station.slew.is_slewed(), "{}", station.weapon.source);
        }
    }
    // The AI is given the same slew and no strafing gun.
    let specs = station_specs(&config, false);
    assert_eq!(specs[gun].slew, slew);
    assert!(
        super::strike::describe(&config, &specs)
            .iter()
            .all(|store| store.station != StationId(gun as u8)),
        "a tail gun is not a strafing gun"
    );
    // Flown by the AI with the real record (its gun zone and the Harpoons
    // on the pylons), the tail gun still asks for rounds against a fighter
    // 12 degrees off the tail, and the aircraft is not turned to lay it.
    let (mut wings, mut targets) = build(None);
    for actor in wings.mission.actors_mut() {
        actor.set_stations(Vec::new());
    }
    let weapon = config.stations[gun].weapon.clone();
    let actor = wings.mission.actor_mut(3).unwrap();
    actor.set_stations(specs.clone());
    actor.set_guns([(gun as u8, weapon)].into());
    actor.flight_mut().position = bearing_from_tail(12., 3_000.);
    actor.flight_mut().yaw = 0.;
    wings
        .mission
        .order(3, WingRequest::TargetAssignment(TargetOrder::FreeSelection))
        .unwrap()
        .unwrap();
    let held = {
        wings
            .advance(player_object([0., 20_000., 0.]), &mut targets, &flat)
            .unwrap();
        wings.mission.actor(3).unwrap().flight().clone()
    };
    let mut asked = 0;
    for _ in 0..240 {
        *wings.mission.actor_mut(3).unwrap().flight_mut() = held.clone();
        let output = wings
            .advance(player_object([0., 20_000., 0.]), &mut targets, &flat)
            .unwrap();
        asked += output
            .launches
            .iter()
            .filter(|e| e.actor == 3 && e.station == StationId(gun as u8))
            .count();
    }
    assert!(asked > 0, "the retail tail gun asks for rounds astern");
    // Fired by a human, the round leaves astern from the tail cone.
    let basis = tore_sim::attitude::Basis::new(0.4, 0.1, 0.0);
    let direction = config.stations[gun].fire_direction(basis);
    for (got, nose) in direction.iter().zip(basis.forward) {
        assert!((got + nose).abs() < 1e-9);
    }
}
