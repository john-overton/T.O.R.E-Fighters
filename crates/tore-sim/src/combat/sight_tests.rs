//! Tests of the targeting sight (docs/spec/air-to-ground.md, "The targeting
//! sight"): a sight per sensor record and its gimbal, and the shared step on
//! a fighter's sight. The AC-130's own tests (`live/gunship_tests.rs`) pin the
//! gunsight's behaviour through the same step.
use super::*;
use crate::attitude::Basis;
use tore_formats::weapons::Zone;

fn zone(heading: i16, maximum_range: i32) -> Zone {
    Zone {
        heading,
        pitch: heading,
        minimum_range: 0,
        maximum_range,
        minimum_altitude: i32::MIN,
        maximum_altitude: i32::MAX,
    }
}

/// A sensor record as the retail SEE files give it: signature, zone 0 and
/// zone 1 headings, zone 0 range.
fn record(signature: u8, search: i16, track: i16, range: i32) -> Seeker {
    Seeker {
        flags: [0; 2],
        signature,
        look_down: 0,
        doppler_above: 0,
        doppler_below: 0,
        doppler_minimum_range: 0,
        all_aspect: 0,
        zones: [zone(search, range), zone(track, range)],
        chaff_flare_chance: 0,
        deception_chance: 0,
    }
}

fn degrees(d: f64) -> f64 {
    d.to_radians()
}

#[test]
fn each_sensor_record_gives_its_sight_and_gimbal() {
    // PAVESPK and PAVEKNF: a laser pod, all round, 60,760 ft, 425 lb.
    let pave = SightSensor::from_record(&record(1, 32767, 32767, 60_760), 425).unwrap();
    assert!(pave.laser);
    assert_eq!(pave.range_ft, 60_760.);
    assert_eq!(pave.gimbal, Gimbal::Hemisphere);
    assert_eq!(pave.eye, [0., -POD_EYE_DROP_FT, 0.]);
    assert_eq!(pave.default_look, FIGHTER_DEFAULT_LOOK);
    // SU24L and KA50L: a built-in laser, 60 degree search, 45 degree track.
    let built_in = SightSensor::from_record(&record(1, 10920, 8190, 60_760), 0).unwrap();
    assert!(built_in.laser);
    match built_in.gimbal {
        Gimbal::Cone { search, track } => {
            assert!((search - degrees(60.)).abs() < 1e-9);
            assert!((track - degrees(45.)).abs() < 1e-9);
        }
        Gimbal::Hemisphere => panic!("a built-in designator looks through a cone"),
    }
    assert_eq!(built_in.eye, [0.; 3]);
    // AAS-38: a FLIR pod (350 lb) with the same zones: the pod hemisphere.
    let aas38 = SightSensor::from_record(&record(2, 10920, 8190, 91_140), 350).unwrap();
    assert!(!aas38.laser);
    assert_eq!(aas38.gimbal, Gimbal::Hemisphere);
    // AV8IR and the other internal FLIRs: a cone.
    let flir = SightSensor::from_record(&record(2, 10920, 8190, 91_140), 0).unwrap();
    assert!(!flir.laser);
    assert!(matches!(flir.gimbal, Gimbal::Cone { .. }));
    // Radar, visual and warning receiver records give no sight.
    for signature in [0, 3, 4] {
        assert!(SightSensor::from_record(&record(signature, 10920, 8190, 60_760), 0).is_none());
    }
}

#[test]
fn the_hemisphere_is_the_ac130_dome_and_the_cone_holds_the_nose() {
    let dome = Gimbal::Hemisphere;
    for look in [[0.3, 0.2], [-2.5, -0.4], [1., -1.6]] {
        assert_eq!(
            dome.clamp(look),
            crate::combat::gunship::clamp_look(look),
            "the hemisphere is the gunsight's gimbal"
        );
        assert_eq!(
            dome.beyond(look),
            crate::combat::gunship::beyond_gimbal(look)
        );
    }
    let cone = Gimbal::Cone {
        search: degrees(60.),
        track: degrees(45.),
    };
    // Inside the track cone: followed. Between 45 and 60: in view, not
    // followed (a laser masks there). Past 60: held at the edge.
    assert!(cone.follows([degrees(30.), degrees(-20.)]));
    let wide = [degrees(50.), 0.];
    assert!(!cone.beyond(wide) && !cone.follows(wide));
    let behind = [degrees(120.), degrees(-10.)];
    assert!(cone.beyond(behind));
    let held = cone.clamp(behind);
    assert!((held[0].hypot(held[1]) - degrees(60.)).abs() < 1e-9);
    assert!(held[0] > 0. && held[1] < 0., "held toward the bearing");
    // A pin slewed outward from inside the cone stops at its edge.
    let (turned, cut) = cone.slew_pin([degrees(59.), 0.], [degrees(61.), 0.]);
    assert!(cut);
    assert!((turned[0] - degrees(60.)).abs() < 1e-9);
}

fn launcher(height: f64) -> Launcher {
    Launcher {
        position: [0., height, 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 400.,
        velocity: [0., 0., 400.],
        bay_ready: true,
        radar_power: true,
        radar: false,
        jammer: false,
        alive: true,
        body_present: true,
        controls: crate::sensors::Controls::default(),
    }
}

fn object(id: u32, position: Vector) -> SightObject {
    SightObject {
        id,
        position,
        velocity: [0.; 3],
        alive: true,
        airborne: false,
        friendly: false,
    }
}

fn pod() -> SightSensor {
    SightSensor::from_record(&record(1, 32767, 32767, 60_760), 425).unwrap()
}

#[test]
fn a_fighter_sight_looks_ahead_and_down_and_picks_under_its_crosshair() {
    let flat = |_: f64, _: f64| 0.;
    let l = launcher(5_000.);
    let mut sight = Sight::new(pod());
    let (aim, led) = sight.step(l, &[], &flat, 60_760., 1);
    // Ten degrees down from 5,000 ft (less the pod's drop) meets the ground
    // about 28,000 ft ahead.
    let ahead = (5_000. - POD_EYE_DROP_FT) / degrees(10.).tan();
    assert!((aim.position[2] - ahead).abs() < 1., "{:?}", aim.position);
    assert!(aim.position[1].abs() < 1e-3 && !led);
    assert_eq!(sight.held_point(), None, "a free sight holds no point");
    // Backslash over a truck under the crosshair tracks it.
    let truck = object(7, [0., 0., ahead + 50.]);
    sight.request = Some(SightRequest::Designate);
    sight.step(l, &[truck], &flat, 60_760., 2);
    assert_eq!(sight.mode, SightMode::Tracked(7));
    assert_eq!(sight.held_point(), Some(truck.position));
    // L drops it and the sight slews freely from where it looked.
    sight.drop_hold();
    assert_eq!(sight.mode, SightMode::Free);
    // A second L travels home to the default look.
    sight.look = [degrees(20.), degrees(-30.)];
    sight.drop_hold();
    assert!(sight.returning);
    for tick in 3..200 {
        sight.step(l, &[], &flat, 60_760., tick);
    }
    assert_eq!(sight.look, FIGHTER_DEFAULT_LOOK);
    // Shift+Backslash pins the ground under the crosshair.
    sight.request = Some(SightRequest::Pin);
    sight.step(l, &[truck], &flat, 60_760., 300);
    assert!(matches!(sight.mode, SightMode::Pinned(_)));
}

#[test]
fn slash_steps_right_through_the_visible_ground_objects_and_wraps() {
    let flat = |_: f64, _: f64| 0.;
    let l = launcher(5_000.);
    let mut sight = Sight::new(pod());
    let objects = [
        object(1, [-6_000., 0., 20_000.]),
        object(2, [0., 0., 25_000.]),
        object(3, [6_000., 0., 20_000.]),
        // Friendly, flying, destroyed or beyond the laser's reach: skipped.
        SightObject {
            friendly: true,
            ..object(4, [3_000., 0., 20_000.])
        },
        SightObject {
            airborne: true,
            ..object(5, [-3_000., 500., 20_000.])
        },
        SightObject {
            alive: false,
            ..object(6, [1_000., 0., 20_000.])
        },
        object(8, [0., 0., 80_000.]),
    ];
    let mut picks = vec![];
    for tick in 1..=4 {
        sight.request = Some(SightRequest::Next);
        sight.step(l, &objects, &flat, 60_760., tick);
        picks.push(sight.target().unwrap());
    }
    // The first press takes the one nearest the line of sight, then each
    // press steps right, wrapping to the leftmost.
    assert_eq!(picks, [2, 3, 1, 2]);
    // A ridge between the eye and the objects hides them.
    let ridge = |_: f64, z: f64| {
        if (9_000.0..11_000.).contains(&z) {
            4_500.
        } else {
            0.
        }
    };
    let mut sight = Sight::new(pod());
    sight.request = Some(SightRequest::Next);
    sight.step(l, &objects[..3], &ridge, 60_760., 1);
    assert_eq!(sight.target(), None);
}

#[test]
fn a_built_in_sight_stops_at_its_cone_and_says_so() {
    let flat = |_: f64, _: f64| 0.;
    let l = launcher(5_000.);
    let sensor = SightSensor::from_record(&record(1, 10920, 8190, 60_760), 0).unwrap();
    let mut sight = Sight::new(sensor);
    // A truck off the left wing, behind the cone.
    let truck = object(9, [-20_000., 0., -2_000.]);
    sight.track(9);
    sight.step(l, &[truck], &flat, 60_760., 5);
    assert_eq!(sight.target(), Some(9));
    assert_eq!(
        sight.notice,
        Some(SightNotice {
            notice: Notice::GimbalLimit,
            tick: 5
        })
    );
    assert!(sight.look[0].hypot(sight.look[1]) <= degrees(60.) + 1e-9);
    // The aim point stays on the truck itself.
    assert_eq!(sight.aim, Some(truck.position));
}
