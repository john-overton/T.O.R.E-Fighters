//! The plane step over objects: a helicopter may come down on the flat top of
//! a building, every other contact with an object and every fixed-wing
//! aircraft keeps the crash. docs/spec/touchdown.md. Synthetic fixtures only.

use super::plane::fly;
use crate::terrain::Terrain;
use tore_formats::aircraft::AircraftId;
use tore_sim::{
    airport::{OrientedBox, SourceKey, StaticObject},
    flight,
    models::FlightModel,
};

const BUILDING: u32 = 77;
const X: f64 = 2_000.;
const Z: f64 = 3_000.;

fn terrain_with_building(half: [f64; 3], pitch: f64) -> Terrain {
    let mut terrain = crate::test_support::terrain();
    // Level ground (the fixture's own cells tilt about 15 degrees).
    for cell in &mut terrain.theater.cells {
        cell.elevation = 4;
    }
    let ground = f64::from(terrain.height(X as f32, Z as f32));
    terrain.airport_scene.objects.push(StaticObject {
        id: BUILDING,
        source: SourceKey {
            layout: "T.MM".into(),
            ordinal: 0,
        },
        name: "Hangar".into(),
        object_type: "HANGR.OT".into(),
        bounds: OrientedBox {
            center: [X, ground + half[1], Z],
            half,
            heading: 0.,
            pitch,
            bank: 0.,
        },
        hit_points: 100,
        runway: false,
        category: 0x2000,
        radar_signature: 1.,
        infrared_signature: 0.,
        type_flags: 0,
        max_visible: 0,
    });
    terrain
}

fn roof(terrain: &Terrain) -> f64 {
    terrain.airport_scene.objects[0].bounds.center[1]
        + terrain.airport_scene.objects[0].bounds.half[1]
}

fn aircraft(id: AircraftId, x: f64, y: f64, z: f64) -> flight::State {
    let mut s = flight::State::new(&crate::test_support::powered_profile(id), [x, y, z]).unwrap();
    s.enable_research(1).unwrap();
    s.cheats.unlimited_fuel = true;
    s.gear = 1.;
    s.gear_down = true;
    s
}

fn fixed_wing(x: f64, y: f64, z: f64) -> flight::State {
    let mut s = flight::State::new(&crate::test_support::profile(), [x, y, z]).unwrap();
    s.enable_research(1).unwrap();
    s.gear = 1.;
    s.gear_down = true;
    s
}

/// Descends at `sink` ft/s from `start` over the terrain until it is down,
/// crashed or `ticks` have gone; the descent is held each tick, as a pilot on
/// the collective would hold it.
fn descend(
    mut s: flight::State,
    terrain: &Terrain,
    sink: f64,
    drift: [f64; 2],
    ticks: usize,
) -> flight::State {
    let mut previous = s.clone();
    for _ in 0..ticks {
        s.velocity = [drift[0], -sink, drift[1]];
        fly(
            &mut previous,
            &mut s,
            &Default::default(),
            terrain,
            &mut std::iter::once(BUILDING),
        );
        if s.crashed || s.research.as_ref().is_some_and(|r| r.on_ground) {
            break;
        }
    }
    s
}

#[test]
fn a_helicopter_lands_on_a_flat_roof_and_rests_on_its_height() {
    let terrain = terrain_with_building([60., 20., 60.], 0.);
    let top = roof(&terrain);
    let s = aircraft(AircraftId::Ah64, X + 10., top + 10., Z - 5.);
    let s = descend(s, &terrain, 5., [0., 0.], 120 * 10);
    assert!(!s.crashed, "{:?}", s.trace().contact);
    assert!(s.research.as_ref().unwrap().on_ground);
    assert!(
        (s.position[1] - (top + s.model().configuration().equipment.ground_clearance_ft)).abs()
            < 0.5,
        "rests at {} on a roof at {top}",
        s.position[1]
    );
}

#[test]
fn a_helicopter_sitting_on_a_roof_stays_there_and_a_hard_sink_still_crashes() {
    let terrain = terrain_with_building([60., 20., 60.], 0.);
    let top = roof(&terrain);
    let mut s = descend(
        aircraft(AircraftId::Ah64, X, top + 10., Z),
        &terrain,
        5.,
        [0., 0.],
        120 * 10,
    );
    let mut previous = s.clone();
    for _ in 0..240 {
        fly(
            &mut previous,
            &mut s,
            &tore_input::PilotInput {
                collective: Some(0.),
                ..Default::default()
            },
            &terrain,
            &mut std::iter::once(BUILDING),
        );
    }
    assert!(
        !s.crashed && s.research.as_ref().unwrap().on_ground,
        "{:?} y={} roof={top} {:?}",
        s.trace().contact,
        s.position[1],
        s.systems.messages
    );
    let hard = descend(
        aircraft(AircraftId::Ah64, X, top + 10., Z),
        &terrain,
        30.,
        [0., 0.],
        120 * 10,
    );
    assert!(hard.crashed);
}

#[test]
fn a_helicopter_that_misses_the_roof_comes_down_on_the_ground_beside_it() {
    let terrain = terrain_with_building([60., 20., 60.], 0.);
    let s = descend(
        aircraft(AircraftId::Ah64, X + 200., 80., Z),
        &terrain,
        5.,
        [0., 0.],
        120 * 40,
    );
    assert!(!s.crashed && s.research.as_ref().unwrap().on_ground);
    let ground = f64::from(terrain.height((X + 200.) as f32, Z as f32));
    assert!(s.position[1] < ground + 10., "{}", s.position[1]);
}

#[test]
fn a_fixed_wing_aircraft_still_crashes_into_the_roof() {
    let terrain = terrain_with_building([60., 20., 60.], 0.);
    let top = roof(&terrain);
    let s = descend(
        fixed_wing(X, top + 10., Z),
        &terrain,
        5.,
        [0., 0.],
        120 * 10,
    );
    assert!(s.crashed);
}

#[test]
fn flying_into_the_side_of_a_building_still_crashes_a_helicopter() {
    let terrain = terrain_with_building([60., 20., 60.], 0.);
    let top = roof(&terrain);
    // Level with the walls, moving in from the west at 60 ft/s.
    let s = aircraft(AircraftId::Ah64, X - 100., top - 10., Z);
    let mut s = s;
    let mut previous = s.clone();
    for _ in 0..120 * 4 {
        s.velocity = [60., 0., 0.];
        fly(
            &mut previous,
            &mut s,
            &Default::default(),
            &terrain,
            &mut std::iter::once(BUILDING),
        );
        if s.crashed {
            break;
        }
    }
    assert!(s.crashed);
}

#[test]
fn a_roof_over_ten_degrees_is_no_landing_surface_and_a_pole_is_not_either() {
    // A tilted top (12 degrees of pitch): the helicopter passes into the box.
    let terrain = terrain_with_building([60., 20., 60.], 12f64.to_radians());
    let top = roof(&terrain);
    let s = descend(
        aircraft(AircraftId::Ah64, X, top + 10., Z),
        &terrain,
        5.,
        [0., 0.],
        120 * 10,
    );
    assert!(s.crashed);
    // A thin antenna mast.
    let terrain = terrain_with_building([2., 60., 2.], 0.);
    let top = roof(&terrain);
    let s = descend(
        aircraft(AircraftId::Ah64, X, top + 10., Z),
        &terrain,
        5.,
        [0., 0.],
        120 * 10,
    );
    assert!(s.crashed);
}
