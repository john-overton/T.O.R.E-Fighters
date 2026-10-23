//! Moving a ground target with its unit (surface-AI slice M1).
use super::super::tests::fixture;
use super::super::*;
use crate::airport::OrientedBox;

fn tank_at(x: f64) -> OrientedBox {
    OrientedBox {
        center: [x, 10., 0.],
        half: [8., 5., 15.],
        heading: 0.,
        pitch: 0.,
        bank: 0.,
    }
}

fn state() -> State {
    let mut s = fixture(true);
    s.targets.clear();
    s.add_ground_target(0x5000_0000, tank_at(0.), 100, 0x400, Side(2))
        .unwrap();
    s
}

#[test]
fn a_moved_target_and_its_box_are_where_the_unit_is() {
    let mut s = state();
    let id = 0x5000_0000;
    assert!(s.move_ground_target(id, tank_at(500.), [50., 0., 0.], false));
    assert_eq!(s.ground_bounds(id), Some(tank_at(500.)));
    let row = s.targets.iter().find(|t| t.id == id).unwrap();
    // The aim point is in the upper half of the box, as registration puts it.
    assert_eq!(row.position, [500., 12.5, 0.]);
    assert_eq!(row.velocity, [50., 0., 0.]);
    assert_eq!(row.hp, 100);
    // A segment through the new place meets the box; through the old, not.
    let new = s.ground_bounds(id).unwrap();
    assert!(
        new.segment_fraction([500., 10., -50.], [500., 10., 50.])
            .is_some()
    );
    assert!(
        new.segment_fraction([0., 10., -50.], [0., 10., 50.])
            .is_none()
    );
}

#[test]
fn a_target_moved_before_the_step_is_on_the_aim_point_after_it() {
    let mut s = state();
    let id = 0x5000_0000;
    assert!(s.move_ground_target(id, tank_at(500.), [60., 0., 0.], true));
    let row = s.targets.iter().find(|t| t.id == id).unwrap();
    assert!((row.position[0] - (500. - 0.5)).abs() < 1e-9);
    s.step(&[], |_, _| 0.);
    let row = s.targets.iter().find(|t| t.id == id).unwrap();
    assert!((row.position[0] - 500.).abs() < 1e-9);
    // A dead unit does not drift: the step leaves it where it was put.
    s.targets.iter_mut().find(|t| t.id == id).unwrap().hp = 0;
    assert!(s.move_ground_target(id, tank_at(700.), [60., 0., 0.], true));
    s.step(&[], |_, _| 0.);
    let row = s.targets.iter().find(|t| t.id == id).unwrap();
    assert_eq!(row.position[0], 700.);
}

#[test]
fn only_a_ground_object_with_a_valid_volume_can_be_moved() {
    let mut s = state();
    assert!(!s.move_ground_target(7, tank_at(1.), [0.; 3], false));
    assert!(!s.move_ground_target(
        0x5000_0000,
        OrientedBox {
            half: [0.; 3],
            ..tank_at(1.)
        },
        [0.; 3],
        false
    ));
    assert_eq!(s.ground_bounds(0x5000_0000), Some(tank_at(0.)));
}
