//! Parked aircraft placement (plan section 9.1, slice PA1).
use super::parked::{DeckSpot, aircraft_shape_scale, deck_spot, flown};
use super::tests::{surface_resources, target, world_with_target};
use super::*;
use crate::ai_wings::ENEMY_SIDE;
use tore_formats::{aircraft::AircraftId, carrier::Deck, shape::object_scale};

/// A deck 20 units wide and 200 long, 5 units up.
fn deck() -> Deck {
    Deck {
        height: 5.,
        area: 4000.,
        outline: vec![[-10., -100.], [10., -100.], [10., 100.], [-10., 100.]],
    }
}

#[test]
fn a_fleet_aircraft_keeps_its_deck_spot_in_hull_units() {
    // The carrier heads east (90 degrees): forward is east, right is south.
    let carrier = [1000, 0, 2000];
    let ahead = [1200, 0, 2000];
    let spot = deck_spot(&deck(), 4., 4., carrier, 90, ahead).unwrap();
    assert!((spot.units[0]).abs() < 1e-4 && (spot.units[1] - 50.).abs() < 1e-4);
    assert!((spot.feet[1] - 20.).abs() < 1e-9);
    // Placed at a third of the authored scale, the deck and its aircraft
    // shrink together: the same spot in hull units, a third of the feet.
    let small: DeckSpot = deck_spot(&deck(), 4., 4. / 3., carrier, 90, ahead).unwrap();
    assert_eq!(small.units, spot.units);
    for axis in 0..3 {
        assert!((small.feet[axis] * 3. - spot.feet[axis]).abs() < 1e-9);
    }
    // 80 ft north of the centre is 20 units to the carrier's left: off the
    // 10-unit half width.
    assert!(deck_spot(&deck(), 4., 4., carrier, 90, [1000, 0, 2080]).is_none());
    // Heading north the same point is 20 units ahead: on the deck.
    assert!(deck_spot(&deck(), 4., 4., carrier, 0, [1000, 0, 2080]).is_some());
}

#[test]
fn the_game_flies_only_its_own_types_by_exact_name() {
    assert_eq!(flown("MIG21.PT"), Some(AircraftId::Mig21));
    assert_eq!(flown("RAFALE.PT"), Some(AircraftId::Rafale));
    for parked_only in ["MIG21F.PT", "RAFALEF.PT", "SPE.PT", "F16E.PT"] {
        assert_eq!(flown(parked_only), None, "{parked_only}");
    }
}

#[test]
fn a_parked_aircraft_stands_on_its_gear_at_the_aircraft_scale_and_is_a_target() {
    let r = surface_resources();
    let world = world_with_target(&r, &target("QUCITY", 3, 3, 9));
    let surface = &world.terrain.surface;
    let id = UnitId(SURFACE_UNIT_BASE + 16);
    assert_eq!(surface.parked.len(), 1);
    assert_eq!(surface.parked_scene.len(), 1);
    let pose = &surface.parked_scene[0];
    assert_eq!((pose.id, pose.resource.as_str()), (id, "F18.PT"));
    assert_eq!(
        (pose.side, pose.target, pose.deck),
        (ENEMY_SIDE, true, None)
    );
    assert_eq!(pose.aircraft, Some(AircraftId::F18));
    let bytes = &r[&pose.shape];
    assert_eq!(pose.scale, object_scale(bytes).unwrap() / 3.);
    assert_eq!(pose.scale, aircraft_shape_scale(bytes).unwrap());
    // The lowest point of the gear-down shape meets the ground.
    let gear = tore_formats::parked_aircraft::gear(bytes).unwrap();
    assert_eq!(pose.gear_word, gear.word);
    let low = gear
        .down
        .faces
        .iter()
        .flat_map(|f| &f.positions)
        .map(|p| f64::from(p[2]))
        .fold(f64::INFINITY, f64::min);
    let [x, _, z] = pose.ground;
    let ground = f64::from(world.terrain.height(x as f32, z as f32));
    assert!((pose.ground[1] - ground).abs() < 1e-6);
    assert!((pose.origin[1] - (ground - low * pose.scale)).abs() < 1e-6);
    // Combat holds it as a parked target there, named for the window.
    let row = world
        .combat
        .state
        .targets
        .iter()
        .find(|t| t.id == id.0)
        .unwrap();
    assert_eq!(row.position, pose.origin);
    assert!(row.on_ground && !row.airborne);
    assert_eq!(row.category, pose.class);
    assert_eq!(row.hp, pose.hit_points);
    assert_eq!(world.combat.ground_name(id.0), Some(pose.name.as_str()));
    // The pose is not in the digest: two builds agree, and the same
    // surface without poses has the same digest.
    let mut bare = surface.clone();
    bare.parked_scene.clear();
    assert_eq!(bare.digest(), surface.digest());
}
