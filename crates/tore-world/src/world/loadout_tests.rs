//! Per-plane loadouts of an open mission (slice EF4): the players' choices
//! in the lobby, carried in the spec's text, checked by the Load Ordnance
//! page's rules, flown by the AI until a human takes the plane, and kept by
//! the human. Synthetic resources only.

use super::*;
use crate::{
    mission::{LoadoutSpec, MissionSpec, Skill, Start},
    test_support::resources::{THEATER, resources},
};
use tore_formats::aircraft::AircraftId;

/// Friendly Wing 1 of three and the enemy's Wing 1 of two, airborne.
fn spec() -> MissionSpec {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 3;
    spec.wings[3].count = 2;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 5;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

/// The synthetic F/A-18D's standard load: the gun with 500 rounds and two
/// missiles.
fn standard() -> LoadoutSpec {
    let map = resources();
    let player = aircraft_type::AircraftType::load(&map, AircraftId::F18).unwrap();
    LoadoutSpec::of(
        &tore_sim::combat::loadout::Loadout::new(&player.profile, |name| {
            map.get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
        })
        .unwrap(),
    )
}

/// One missile and 300 rounds, with half the fuel.
fn light() -> LoadoutSpec {
    let mut load = standard();
    load.fuel_lbs = (load.fuel_lbs / 2.).round();
    load.stations[0].quantity = 300;
    load.stations[1].quantity = 1;
    load
}

fn refused(spec: &MissionSpec, seating: Seating) -> String {
    match World::new(spec, &resources(), seating) {
        Ok(_) => panic!("the mission built"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn plane_loadouts_survive_the_text_form() {
    let mut spec = spec();
    spec.plane_loadouts.insert(1, light());
    spec.plane_loadouts.insert(2, standard());
    let text = spec.to_text();
    assert!(
        text.contains(&format!("plane-loadout 1 fuel {}\n", light().fuel_lbs)),
        "{text}"
    );
    assert!(
        text.contains("plane-loadout 1 station 1 AIM9M.JT 2 1\n"),
        "{text}"
    );
    assert_eq!(MissionSpec::from_text(&text).unwrap(), spec);
}

#[test]
fn a_plane_loadout_line_out_of_place_is_refused_with_its_line() {
    let base = "tore-mission 1\ntheater UKR\nwing friendly 1 F18.PT 2 average\n";
    let error = |extra: &str| {
        MissionSpec::from_text(&format!("{base}{extra}"))
            .unwrap_err()
            .to_string()
    };
    assert_eq!(
        error("plane-loadout 1 station 0 M61.JT 500 500\n"),
        "the loadout has no `plane-loadout 1 fuel` line"
    );
    assert_eq!(
        error("plane-loadout 30 fuel 100\n"),
        "line 4: plane 30 is beyond the mission's planes 0 to 29"
    );
    assert_eq!(
        error("plane-loadout 1 fuel 100\nplane-loadout 1 fuel 200\n"),
        "line 5: `plane-loadout 1 fuel` appears twice"
    );
    assert_eq!(
        error("plane-loadout one fuel 100\n"),
        "line 4: the plane number must be a whole number, not `one`"
    );
}

#[test]
fn the_open_planes_are_the_rosters() {
    let spec = spec();
    let world = World::new(&spec, &resources(), Seating::Open).unwrap();
    let planes = spec.open_planes();
    assert_eq!(planes.len(), world.roster.planes().len());
    for plane in planes {
        let entry = world.roster.plane(PlaneId(plane.id)).unwrap();
        assert_eq!(entry.slot.wing, plane.wing);
        assert_eq!(entry.slot.member, plane.member);
        let slot = world.ai_wings.as_ref().unwrap().slot(plane.id).unwrap();
        assert_eq!(slot.aircraft, plane.aircraft);
    }
    assert_eq!(
        spec.summary(),
        "UKR, clear, airborne at 10000 ft: F/A-18D Hornet x3 against F/A-18D Hornet x2"
    );
}

#[test]
fn a_human_taking_a_loaded_plane_keeps_its_loadout() {
    let mut spec = spec();
    spec.plane_loadouts.insert(1, light());
    let mut world = World::new(&spec, &resources(), Seating::Open).unwrap();
    // The AI flies it with the loadout's fuel until a human takes it.
    let actor = world.ai_wings.as_ref().unwrap().mission().actor(1).unwrap();
    let fuel = light().fuel_lbs;
    assert!(fuel > 0. && fuel < standard().fuel_lbs);
    assert_eq!(actor.flight().fuel, fuel);
    world.take_plane(SeatId(0), PlaneId(1)).unwrap();
    let own = world.combat.state.ownship(1).unwrap();
    assert_eq!(own.ammo, [300, 1]);
    assert_eq!(world.cockpits[0].flight.fuel, fuel);
    // A plane with no loadout carries the standard load.
    world.take_plane(SeatId(1), PlaneId(2)).unwrap();
    assert_eq!(world.combat.state.ownship(2).unwrap().ammo, [500, 2]);
}

#[test]
fn a_loadout_the_page_would_not_allow_is_refused_with_its_reason() {
    let map = resources();
    let player = aircraft_type::AircraftType::load(&map, AircraftId::F18).unwrap();
    let check = |load: &LoadoutSpec, guns_only: bool| {
        load.check_for_plane(&player.profile, &map, guns_only)
            .map(|_| ())
            .map_err(|error| error.to_string())
    };
    assert_eq!(check(&light(), false), Ok(()));
    let mut over = standard();
    over.stations[1].quantity = 3;
    assert_eq!(
        check(&over, false),
        Err("Station quantity exceeds capacity.".into())
    );
    let mut cheat = standard();
    cheat.cheat = true;
    assert_eq!(
        check(&cheat, false),
        Err("Cheat loading is not allowed in a multiplayer game.".into())
    );
    let mut stretched = standard();
    stretched.stations[0].count = 5_000;
    stretched.stations[0].quantity = 5_000;
    assert_eq!(
        check(&stretched, false),
        Err("Station 1 cannot hold 5000 M61.JT.".into())
    );
    let mut short = standard();
    short.stations.pop();
    assert!(check(&short, false).unwrap_err().contains("stations"));
    let mut tanks = standard();
    tanks.fuel_lbs = 1e9;
    assert!(check(&tanks, false).is_err());
    assert_eq!(
        check(&standard(), true),
        Err("Guns only is selected. Unload other weapons or return to setup and change the restriction.".into())
    );
    let mut guns = standard();
    guns.stations[1].quantity = 0;
    assert_eq!(check(&guns, true), Ok(()));
}

#[test]
fn plane_loadouts_are_refused_where_they_do_not_belong() {
    let mut spec = spec();
    spec.plane_loadouts.insert(1, light());
    assert!(refused(&spec, Seating::SinglePlayer).contains("multiplayer"));
    let mut missing = self::spec();
    missing.plane_loadouts.insert(9, light());
    assert_eq!(
        refused(&missing, Seating::Open),
        "The mission has no plane 9 to load."
    );
    let mut over = self::spec();
    let mut load = standard();
    load.stations[1].quantity = 3;
    over.plane_loadouts.insert(0, load);
    assert_eq!(
        refused(&over, Seating::Open),
        "Plane 0's loadout: Station quantity exceeds capacity."
    );
}
