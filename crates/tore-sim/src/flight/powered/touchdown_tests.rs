//! The shared touchdown rule ([`crate::touchdown`]) through the hybrid
//! contact, for each class of aircraft, and a helicopter flown down onto open
//! ground and off it again. docs/spec/touchdown.md.

use super::helicopter::tests::{pt_aircraft, trimmed};
use super::tiltrotor::acceptance::pt_v22;
use super::*;
use crate::{
    flight::trace::Contact,
    research::{Research, Surface},
    touchdown::Class,
};
use tore_formats::aircraft::AircraftId;

const KT: f64 = 1.687_81;

fn open_ground() -> Surface {
    Surface::terrain(0.)
}

/// One contact tick of `s` arriving at `sink` ft/s and `knots` of ground
/// speed (north) on `surface`, with the wheels at the surface.
fn touch(mut s: State, sink: f64, knots: f64, surface: Surface) -> State {
    let c = s.model().configuration().clone();
    s.gear = 1.;
    s.gear_down = true;
    s.velocity = [0., -sink, knots * KT];
    s.position[1] = surface.height + c.equipment.ground_clearance_ft - 0.01;
    s.crashed = false;
    let mut r = Research::new(1).unwrap();
    let previous = s.position;
    r.contact(&mut s, surface, &c, 1., 0., previous);
    s.research = Some(r);
    s
}

fn helicopter(id: AircraftId) -> State {
    let mut s = State::new(&pt_aircraft(id), [0., 10., 0.]).unwrap();
    s.enable_research(1).unwrap();
    s
}

fn v22(nacelle_degrees: f64) -> State {
    let mut s = State::new(&pt_v22(), [0., 10., 0.]).unwrap();
    s.enable_research(1).unwrap();
    s.lift_controls.conversion_actual = nacelle_degrees / 97.5;
    s
}

fn jet(id: AircraftId, nozzle_degrees: f64) -> State {
    let mut s = super::jet::tests::state(&super::jet::tests::fixture(id), [0., 10., 0.]);
    s.lift_controls.vector_pitch_actual = nozzle_degrees / 100.;
    s
}

fn survived(s: &State) -> bool {
    !s.crashed && s.research.as_ref().unwrap().on_ground
}

fn gear_damaged(s: &State) -> bool {
    s.systems.has(16)
}

#[test]
fn classes_follow_the_nacelles_and_the_nozzles() {
    assert_eq!(Class::of(&helicopter(AircraftId::Ah64)), Class::Rotor);
    assert_eq!(Class::of(&helicopter(AircraftId::Mi24)), Class::Rotor);
    assert_eq!(Class::of(&v22(87.)), Class::Rotor);
    assert_eq!(Class::of(&v22(75.)), Class::Rotor);
    assert_eq!(Class::of(&v22(74.9)), Class::Fixed);
    assert_eq!(Class::of(&v22(0.)), Class::Fixed);
    for id in [AircraftId::Av8, AircraftId::Yak141] {
        assert_eq!(Class::of(&jet(id, 90.)), Class::Hover, "{id:?}");
        assert_eq!(Class::of(&jet(id, 60.)), Class::Hover, "{id:?}");
        assert_eq!(Class::of(&jet(id, 59.)), Class::Fixed, "{id:?}");
        assert_eq!(Class::of(&jet(id, 0.)), Class::Fixed, "{id:?}");
    }
}

#[test]
fn a_helicopter_sets_down_on_open_ground_within_the_limits() {
    for id in [AircraftId::Ah64, AircraftId::Mi24] {
        let s = touch(helicopter(id), 5., 0., open_ground());
        assert!(survived(&s) && !gear_damaged(&s), "{id:?}");
        assert!(matches!(s.trace().contact, Some(Contact::Rolling(_))));
        // At the limits themselves.
        let s = touch(helicopter(id), 10., 20., open_ground());
        assert!(survived(&s) && !gear_damaged(&s), "{id:?} at the limits");
    }
}

#[test]
fn a_harder_sink_damages_the_gear_and_a_much_harder_one_crashes() {
    let s = touch(helicopter(AircraftId::Ah64), 15., 0., open_ground());
    assert!(survived(&s) && gear_damaged(&s));
    assert!(
        s.systems
            .messages
            .iter()
            .any(|m| m == "Hard landing: gear damaged")
    );
    let s = touch(helicopter(AircraftId::Ah64), 25., 0., open_ground());
    assert!(s.crashed);
    let Some(Contact::Unsafe(u)) = s.trace().contact else {
        panic!("unsafe touchdown");
    };
    assert!(u.not_landable && !u.water);
}

#[test]
fn ground_speed_slope_and_water_each_refuse_a_helicopter() {
    let ah = || helicopter(AircraftId::Ah64);
    assert!(gear_damaged(&touch(ah(), 3., 25., open_ground())));
    assert!(touch(ah(), 3., 40., open_ground()).crashed);
    let mut steep = open_ground();
    steep.slope_deg = 10.5;
    assert!(touch(ah(), 3., 0., steep).crashed);
    steep.slope_deg = 9.5;
    assert!(survived(&touch(ah(), 3., 0., steep)));
    let mut water = open_ground();
    water.water = true;
    assert!(touch(ah(), 1., 0., water).crashed);
}

#[test]
fn the_v22_lands_off_airfield_in_helicopter_mode_only() {
    assert!(survived(&touch(v22(87.), 5., 5., open_ground())));
    assert!(touch(v22(87.), 5., 40., open_ground()).crashed);
    assert!(touch(v22(0.), 1., 0., open_ground()).crashed);
    assert!(touch(v22(60.), 1., 0., open_ground()).crashed);
}

#[test]
fn a_hovering_jet_has_half_the_speed_allowance_and_a_wingborne_one_none() {
    for id in [AircraftId::Av8, AircraftId::Yak141] {
        assert!(
            survived(&touch(jet(id, 90.), 5., 8., open_ground())),
            "{id:?}"
        );
        let s = touch(jet(id, 90.), 5., 12., open_ground());
        assert!(survived(&s) && gear_damaged(&s), "{id:?} 12 kt");
        assert!(
            touch(jet(id, 90.), 5., 20., open_ground()).crashed,
            "{id:?}"
        );
        assert!(touch(jet(id, 20.), 1., 0., open_ground()).crashed, "{id:?}");
    }
}

#[test]
fn an_airfield_stays_as_it_was_for_every_class() {
    // A firm landing the off-airfield limits would damage, on a runway.
    let s = touch(helicopter(AircraftId::Ah64), 15., 40., Surface::runway(0.));
    assert!(survived(&s) && !gear_damaged(&s));
}

#[test]
fn a_helicopter_settles_on_open_ground_and_lifts_off_again() {
    let mut s = trimmed(AircraftId::Ah64, 40., 0.);
    s.lift_controls.aids.stability = tore_input::StabilityLevel::Attitude;
    s.gear = 1.;
    s.gear_down = true;
    let ground = |_: f64, _: f64| Surface::terrain(0.);
    // The pilot holds a 5 ft/s descent on the collective until the wheels
    // touch.
    let mut touched = false;
    for _ in 0..120 * 90 {
        let collective =
            (s.lift_controls.collective + DT * 0.05 * (-5. - s.vertical_speed)).clamp(0., 1.);
        let input = PilotInput {
            collective: Some(collective),
            ..Default::default()
        };
        s.step_surface(&input, ground);
        if s.research.as_ref().is_some_and(|r| r.on_ground) || s.crashed {
            touched = true;
            break;
        }
        assert!(!s.crashed, "crashed in the air at {:?}", s.position);
    }
    assert!(touched, "never touched down, at {:?}", s.position);
    let at_touchdown = s.trace().contact;
    assert!(!s.crashed, "{at_touchdown:?}");
    let sink = s.trace().contact;
    assert!(!s.crashed, "{sink:?}");
    // Sits.
    for _ in 0..240 {
        let down = PilotInput {
            collective: Some(0.),
            ..Default::default()
        };
        s.step_surface(&down, ground);
    }
    assert!(
        !s.crashed && s.research.as_ref().unwrap().on_ground,
        "{:?} {:?}",
        s.trace().contact,
        s.position
    );
    // And lifts off again on the collective.
    for _ in 0..120 * 20 {
        let input = PilotInput {
            collective: Some(1.0),
            ..Default::default()
        };
        s.step_surface(&input, ground);
        if s.position[1] > 60. {
            break;
        }
    }
    assert!(!s.crashed);
    assert!(s.position[1] > 60., "climbed to {}", s.position[1]);
}
