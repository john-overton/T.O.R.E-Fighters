//! Tests of the laser (docs/spec/air-to-ground.md, "The laser"): the states
//! and transitions of the spec's diagram, the limits (range, cloud, gimbal,
//! terrain), the automatic rule and Ctrl+L.
use super::*;
use crate::sensors::WeatherBand;

const REACH: f64 = 60_760.;

fn inputs(held: Option<(Vector, Option<u32>)>, automatic: bool) -> LaserInputs {
    LaserInputs {
        designator: Some(REACH),
        held,
        eye: [0., 10_000., 0.],
        in_gimbal: true,
        automatic,
        weather: 1.,
    }
}

fn tank_at(ahead: f64) -> Option<(Vector, Option<u32>)> {
    Some(([0., 0., ahead], Some(7)))
}

fn open(_: Vector, _: Vector) -> bool {
    true
}

#[test]
fn the_laser_walks_the_state_diagram() {
    let mut laser = Laser::default();
    // Off: the sight holds nothing.
    laser.step(inputs(None, false), open);
    assert_eq!(laser.state, LaserState::Off);
    // Off to Ready: the sight tracks or pins.
    laser.step(inputs(tank_at(20_000.), false), open);
    assert_eq!(laser.state, LaserState::Ready);
    assert!(laser.spot(1, Side(1)).is_none(), "no spot while ready");
    // Ready to Lasing: a laser weapon selected (the automatic rule).
    laser.step(inputs(tank_at(20_000.), true), open);
    assert_eq!(laser.state, LaserState::Lasing);
    let spot = laser.spot(1, Side(1)).unwrap();
    assert_eq!(spot.position, [0., 0., 20_000.]);
    assert_eq!((spot.owner, spot.side, spot.target), (1, Side(1), Some(7)));
    assert!((laser.range_ft - 20_000_f64.hypot(10_000.)).abs() < 1e-9);
    // Lasing to Masked: out of range; back when it clears.
    laser.step(inputs(tank_at(70_000.), true), open);
    assert_eq!(
        (laser.state, laser.mask),
        (LaserState::Masked, Some(Mask::Range))
    );
    assert!(
        laser.spot(1, Side(1)).is_none(),
        "the spot goes while masked"
    );
    laser.step(inputs(tank_at(20_000.), true), open);
    assert_eq!(laser.state, LaserState::Lasing);
    // Lasing to Ready: no laser weapon selected or in flight.
    laser.step(inputs(tank_at(20_000.), false), open);
    assert_eq!(laser.state, LaserState::Ready);
    // Ready to Off: the sight dropped.
    laser.step(inputs(None, false), open);
    assert_eq!(laser.state, LaserState::Off);
    // Lasing to Off: dropped, or the target destroyed with no pin.
    laser.step(inputs(tank_at(20_000.), true), open);
    laser.step(inputs(None, true), open);
    assert_eq!(laser.state, LaserState::Off);
}

#[test]
fn cloud_halves_the_reach_and_terrain_and_the_gimbal_mask() {
    let weather = Weather {
        bands: vec![WeatherBand {
            low_ft: 3_000.,
            high_ft: 6_000.,
            effects: [100, 50, 10, 75, 100],
        }],
    };
    // The line from 10,000 ft to the ground spans the deck: half the reach.
    assert_eq!(weather_scale(&weather, 10_000., 0.), 0.5);
    // Above the deck the laser keeps its reach.
    assert_eq!(weather_scale(&weather, 10_000., 7_000.), 1.);
    let mut laser = Laser::default();
    let cloudy = LaserInputs {
        weather: weather_scale(&weather, 10_000., 0.),
        ..inputs(tank_at(40_000.), true)
    };
    laser.step(cloudy, open);
    assert_eq!(laser.limit_ft, REACH / 2.);
    assert_eq!(
        (laser.state, laser.mask),
        (LaserState::Masked, Some(Mask::Range))
    );
    // The same tank in clear air is lased.
    laser.step(inputs(tank_at(40_000.), true), open);
    assert_eq!(laser.state, LaserState::Lasing);
    // Terrain on the line.
    laser.step(inputs(tank_at(20_000.), true), |_, _| false);
    assert_eq!(laser.mask, Some(Mask::Terrain));
    // Outside the gimbal.
    let outside = LaserInputs {
        in_gimbal: false,
        ..inputs(tank_at(20_000.), true)
    };
    laser.step(outside, open);
    assert_eq!(laser.mask, Some(Mask::Gimbal));
    // The terrain test is not asked while another limit already masks.
    let mut asked = false;
    laser.step(inputs(tank_at(70_000.), true), |_, _| {
        asked = true;
        true
    });
    assert!(!asked);
}

#[test]
fn ctrl_l_overrides_the_automatic_rule_until_a_release_or_the_sight_drops() {
    let mut laser = Laser::default();
    // On by hand with no laser weapon selected.
    assert!(laser.toggle());
    laser.step(inputs(tank_at(20_000.), false), open);
    assert_eq!(laser.state, LaserState::Lasing);
    // Off by hand with a laser weapon selected: the hand wins.
    assert!(!laser.toggle());
    laser.step(inputs(tank_at(20_000.), true), open);
    assert_eq!(laser.state, LaserState::Ready);
    // A new laser weapon release gives the automatic rule back.
    laser.note_release();
    laser.step(inputs(tank_at(20_000.), true), open);
    assert_eq!(laser.state, LaserState::Lasing);
    assert_eq!(laser.manual, None);
    // A release does not end a manual on.
    laser.toggle();
    laser.toggle();
    assert_eq!(laser.manual, Some(true));
    laser.note_release();
    assert_eq!(laser.manual, Some(true));
    // A manual on ends when the sight stops holding a point, so the next
    // track starts ready.
    laser.step(inputs(tank_at(20_000.), false), open);
    assert_eq!(laser.state, LaserState::Lasing);
    laser.step(inputs(None, false), open);
    assert_eq!(laser.manual, None);
    laser.step(inputs(tank_at(20_000.), false), open);
    assert_eq!(laser.state, LaserState::Ready);
    // Ctrl+L before the sight holds anything lases as soon as it does.
    laser.step(inputs(None, false), open);
    laser.toggle();
    laser.step(inputs(None, false), open);
    laser.step(inputs(tank_at(20_000.), false), open);
    assert_eq!(laser.state, LaserState::Lasing);
}

#[test]
fn an_aircraft_without_a_designator_never_lases() {
    let mut laser = Laser::default();
    laser.toggle();
    laser.step(
        LaserInputs {
            designator: None,
            ..inputs(tank_at(20_000.), true)
        },
        open,
    );
    assert_eq!(laser.state, LaserState::Off);
    assert!(laser.spot(1, Side(1)).is_none());
}
