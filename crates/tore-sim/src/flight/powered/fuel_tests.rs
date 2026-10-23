//! Rotorcraft fuel burn (slice HF2): flow by the power the rotor draws, and
//! the endurance each type gets at a hover and at cruise.

use super::tests::{hover, run};
use crate::flight::{PilotInput, State};
use crate::models::FlightModel;
use tore_formats::aircraft::AircraftId;

const KNOTS: f64 = 1.687_81;
const ROTORCRAFT: [AircraftId; 4] = [
    AircraftId::Ah64,
    AircraftId::Mi24,
    AircraftId::Ch47,
    AircraftId::V22,
];

/// A hover with real fuel (the fixtures switch the cheat on).
fn burning(id: AircraftId) -> State {
    let mut s = hover(id, 1_000.);
    s.cheats.unlimited_fuel = false;
    s
}

/// The same aircraft trimmed level at `knots`, with real fuel.
fn cruising(id: AircraftId, knots: f64) -> State {
    let mut s = burning(id);
    let lift = s.model().powered_lift().unwrap();
    let c = s.model().configuration();
    let ok = if let Some(tilt) = lift.tiltrotor {
        s.trim_tiltrotor(knots * KNOTS, tilt.helicopter_nacelle_degrees)
    } else if super::tandem::Tandem::new(&lift, c).is_some() {
        s.trim_tandem(knots * KNOTS)
    } else {
        s.trim_single_rotor(knots * KNOTS)
    };
    assert!(ok, "{id:?} trims at {knots} kt");
    s
}

fn tank(s: &State) -> f64 {
    s.model().configuration().mass.internal_fuel_lbs
}

#[test]
fn a_hover_burns_what_the_rotor_draws_not_the_records_flow() {
    for id in ROTORCRAFT {
        let mut s = burning(id);
        // The record's flow is a whole number of pounds a second (3 for the
        // Mi-24); the burn below is a fraction of that, so it is not read.
        let flow = s.rotorcraft_fuel_flow().unwrap();
        let before = s.fuel;
        run(&mut s, &PilotInput::default(), 600);
        let burned = before - s.fuel;
        assert!(
            (burned / 5. / flow - 1.).abs() < 0.05,
            "{id:?} burned {burned} lb in 5 s at {flow} lb/s"
        );
        // The V-22 is three times the helicopters' size since slice HF3 (the
        // MV-22B's weights and 11,316 hp): about 1.4 lb/s at a hover.
        let most = if id == AircraftId::V22 { 2. } else { 1. };
        assert!(flow < most, "{id:?} hover flow {flow}");
    }
}

#[test]
fn the_flow_follows_the_power_hover_cruise_and_idle() {
    for id in ROTORCRAFT {
        let hover_flow = burning(id).rotorcraft_fuel_flow().unwrap();
        let cruise_flow = cruising(id, 100.).rotorcraft_fuel_flow().unwrap();
        // Level flight at 100 kt is below the hover power (translational
        // lift) for every type.
        assert!(
            cruise_flow < hover_flow,
            "{id:?} cruise {cruise_flow} against hover {hover_flow}"
        );
        // An engine at flight idle (no load) burns the idle share of full flow.
        let fuel = burning(id).rotor_fuel().unwrap();
        assert!(fuel.flow(0.) < 0.3 * fuel.full_flow(), "{id:?}");
        assert!(hover_flow > fuel.flow(0.));
        // Engines off burn nothing.
        let mut off = burning(id);
        off.engine = false;
        assert_eq!(off.rotorcraft_fuel_flow(), Some(0.));
    }
}

#[test]
fn the_mi24_flies_about_two_hours_not_eighteen_minutes() {
    let hover_minutes = {
        let s = burning(AircraftId::Mi24);
        tank(&s) / s.rotorcraft_fuel_flow().unwrap() / 60.
    };
    let cruise_minutes = {
        let s = cruising(AircraftId::Mi24, 100.);
        tank(&s) / s.rotorcraft_fuel_flow().unwrap() / 60.
    };
    // Real: about 1.5 to 2.5 hours on internal fuel (Mi-24V, 1,500 kg).
    assert!(
        (100. ..=180.).contains(&hover_minutes),
        "hover {hover_minutes} min"
    );
    assert!(
        (120. ..=240.).contains(&cruise_minutes),
        "cruise {cruise_minutes} min"
    );
}

#[test]
fn every_rotorcraft_lasts_over_an_hour_at_a_hover_and_cruise_beats_hover() {
    for id in ROTORCRAFT {
        let s = burning(id);
        let hover_min = tank(&s) / s.rotorcraft_fuel_flow().unwrap() / 60.;
        assert!(hover_min > 45., "{id:?} hover {hover_min} min");
        let c = cruising(id, 100.);
        let cruise_min = tank(&c) / c.rotorcraft_fuel_flow().unwrap() / 60.;
        assert!(cruise_min > hover_min, "{id:?}");
    }
}

/// Slice HF3 (John, 2026-10-13): the AI plans with the level flow at its
/// cruise home, 80 percent of the envelope's top speed (the V-22 on its
/// downstops), at sea level, whatever the moment's power.
#[test]
fn planning_uses_a_steady_cruise_flow() {
    for id in ROTORCRAFT {
        let s = burning(id);
        let plan = s.endurance_flow_lbs_per_second();
        let c = s.model().configuration();
        let top = c
            .aerodynamics
            .envelopes
            .iter()
            .filter_map(|e| e.speeds(0.))
            .map(|(_, high)| high)
            .fold(0_f64, f64::max);
        let speed = top * crate::ai::route::ROTORCRAFT_CRUISE_FRACTION;
        let mut level = s.clone();
        level.position[1] = 0.;
        let trimmed = if level.model().powered_lift().unwrap().tiltrotor.is_some() {
            level.trim_tiltrotor(speed, 0.)
        } else {
            level.trim_single_rotor(speed) || level.trim_tandem(speed)
        };
        assert!(trimmed, "{id:?} trims at {speed} ft/s");
        let cruise_flow = level.rotorcraft_fuel_flow().unwrap();
        assert!(
            (plan / cruise_flow - 1.).abs() < 0.02,
            "{id:?} plan {plan} against cruise {cruise_flow}"
        );
        assert!(s.rotor_fuel().unwrap().cruise_power.is_some(), "{id:?}");
        // Steady: the same at a hover and in level flight at 100 kt.
        let fast = cruising(id, 100.);
        assert!(
            (fast.endurance_flow_lbs_per_second() / plan - 1.).abs() < 0.01,
            "{id:?}"
        );
    }
}

#[test]
fn the_jets_and_aeroplanes_keep_the_records_flow() {
    for id in [AircraftId::Av8, AircraftId::Yak141] {
        let s = hover(id, 1_000.);
        assert!(s.rotor_fuel().is_none());
        assert!(s.rotorcraft_fuel_flow().is_none());
        let flow = s
            .model()
            .configuration()
            .propulsion
            .military_fuel_lbs_per_second
            * s.throttle.max(0.1);
        assert_eq!(s.endurance_flow_lbs_per_second(), flow);
    }
}
