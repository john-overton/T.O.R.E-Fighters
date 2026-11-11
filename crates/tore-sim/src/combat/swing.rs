//! The Tornado's swing wing as the simulation sees it (TOR-5, 2026-11-10).
//!
//! The wing's sweep is a pure function of the aircraft's speed and flaps, so
//! every peer works out the same turn. The drawn rig (`tore-app`
//! `tornado_animation`) turns the wing panels by it, and the stations on the
//! swinging wing ride the same turn here: a store on a swivelling wing pylon
//! hangs from the point the swept wing carries it to, and leaves from there.
//! The pylon swivels back by the wing's own turn (John, 2026-11-10, decision
//! 19: the wing pylons swivel to stay aligned with the airflow as the wing
//! sweeps), so the store itself never changes heading.
//!
//! Provenance: the schedule, the pivot and the choice of the swivelling row are
//! fitted or opinionated (agent decisions 2026-11-10, John's decision 19 for
//! the rotating pylons); the real sweep range 25 to 67 degrees is cited from
//! `TOR.INF` (Jane's).
use tore_formats::aircraft::AircraftId;

/// Shape units to feet at the Tornado's header scale 2 (2/3 ft a unit).
pub const FEET_PER_UNIT: f64 = 2. / 3.;
/// Real minimum and maximum wing sweep, degrees (`TOR.INF`, Jane's); the
/// shape's spread pose is the minimum.
pub const SWEEP_MIN: f64 = 25.;
pub const SWEEP_MAX: f64 = 67.;
/// Fitted speed schedule, knots: the wing stays spread up to the first speed
/// and is fully swept at the second. Flaps hold the wing out.
pub const SWEEP_FROM_KNOTS: f64 = 250.;
pub const SWEEP_TO_KNOTS: f64 = 600.;
/// Feet per second to knots.
pub const FPS_PER_KNOT: f64 = 1.68781;
/// The wing sweep pivots, shape units (right, forward, up), vertical axis:
/// the centre of the smallest circle round the fixed glove corners the wing
/// panel shares (fitted, TOR-3). The left glove sits a unit further out.
pub const PIVOT_R: [f32; 3] = [9.5, -1.5, 0.5];
pub const PIVOT_L: [f32; 3] = [-10.5, -1.5, 0.5];

/// How far the wing panel has turned from its spread pose, radians (0 spread,
/// `SWEEP_MAX - SWEEP_MIN` degrees fully swept), for an aircraft flying at
/// `speed_fps` with its flaps `flaps` out (0 to 1).
pub fn wing_turn(speed_fps: f64, flaps: f64) -> f64 {
    let knots = speed_fps / FPS_PER_KNOT;
    ((knots - SWEEP_FROM_KNOTS) / (SWEEP_TO_KNOTS - SWEEP_FROM_KNOTS)).clamp(0., 1.)
        * (SWEEP_MAX - SWEEP_MIN).to_radians()
        * (1. - flaps.clamp(0., 1.))
}

/// The speed in feet per second at which the wing has turned `degrees` of
/// sweep (25 spread to 67), flaps up: the inverse of [`wing_turn`], for
/// captures and measurements that pose the wing.
pub fn speed_for_sweep(degrees: f64) -> f64 {
    let share = ((degrees - SWEEP_MIN) / (SWEEP_MAX - SWEEP_MIN)).clamp(0., 1.);
    (SWEEP_FROM_KNOTS + share * (SWEEP_TO_KNOTS - SWEEP_FROM_KNOTS)) * FPS_PER_KNOT
}

/// The pivot of one wing, feet right, up and forward (the station frame).
pub fn pivot_feet(right: bool) -> [f64; 3] {
    let p = if right { PIVOT_R } else { PIVOT_L };
    [
        f64::from(p[0]) * FEET_PER_UNIT,
        f64::from(p[2]) * FEET_PER_UNIT,
        f64::from(p[1]) * FEET_PER_UNIT,
    ]
}

/// Where the wing carries a point of its own panel, feet right, up and
/// forward of the aircraft: `point` is where the point is with the wing
/// spread, `turn` the wing's turn ([`wing_turn`]), the tip moving aft. The
/// height does not change. `right` names the wing (a left-hand point is
/// carried by the left wing).
pub fn swept(right: bool, point: [f64; 3], turn: f64) -> [f64; 3] {
    let pivot = pivot_feet(right);
    // Tip aft: clockwise seen from above on the right wing (negative about +z).
    let angle = if right { -turn } else { turn };
    let (dx, dz) = (point[0] - pivot[0], point[2] - pivot[2]);
    let (s, c) = angle.sin_cos();
    [
        pivot[0] + dx * c - dz * s,
        point[1],
        pivot[2] + dx * s + dz * c,
    ]
}

/// [`swept`] undone: where a point at `point` now was with the wing spread.
pub fn unswept(right: bool, point: [f64; 3], turn: f64) -> [f64; 3] {
    swept(right, point, -turn)
}

/// The pylon's yaw against the wing panel, radians, positive to the
/// aircraft's right: the wing's own turn undone, so a store on a swivelling
/// pylon keeps its heading along the fuselage at every sweep.
pub fn pylon_swivel(right: bool, turn: f64) -> f64 {
    -turn * if right { 1. } else { -1. }
}

/// Whether source hardpoint `hardpoint` of aircraft `id` is a swivelling wing
/// pylon, carried by the swinging wing (opinionated, John 2026-11-10,
/// decision 19). The Tornado IDS's inner wing pair (row 7) swivels; the
/// centreline tank pair (row 5) and the shoulder pylons (row 6) are on the
/// fuselage and stay fixed. Row 8, the outer wing pair, is retired.
/// Update (TOR-6b, 2026-11-11): row 8 is now the AIM-9 side rail on the row 7
/// pylon, which swivels with it.
pub fn swivels(id: AircraftId, hardpoint: usize) -> bool {
    id == AircraftId::Tornado && matches!(hardpoint, 7 | 8)
}

/// Whether `id` has any swivelling station.
pub fn has_swivel(id: AircraftId) -> bool {
    id == AircraftId::Tornado
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schedule_spreads_below_250_knots_and_sweeps_fully_at_600() {
        assert_eq!(wing_turn(200. * FPS_PER_KNOT, 0.), 0.);
        assert_eq!(wing_turn(250. * FPS_PER_KNOT, 0.), 0.);
        let full = (SWEEP_MAX - SWEEP_MIN).to_radians();
        assert!((wing_turn(600. * FPS_PER_KNOT, 0.) - full).abs() < 1e-12);
        assert!((wing_turn(900. * FPS_PER_KNOT, 0.) - full).abs() < 1e-12);
        assert!((wing_turn(425. * FPS_PER_KNOT, 0.) - full / 2.).abs() < 1e-12);
        // Flaps hold the wing out.
        assert_eq!(wing_turn(900. * FPS_PER_KNOT, 1.), 0.);
    }

    #[test]
    fn the_inverse_schedule_poses_the_wing() {
        for degrees in [25., 35., 46., 60., 67.] {
            let turn = wing_turn(speed_for_sweep(degrees), 0.);
            assert!((turn.to_degrees() + SWEEP_MIN - degrees).abs() < 1e-9);
        }
    }

    #[test]
    fn a_swept_point_turns_about_the_pivot_and_comes_back() {
        for right in [true, false] {
            let side = if right { 1. } else { -1. };
            let point = [side * 10., -8., 3.];
            for degrees in [25., 67.] {
                let turn = (degrees - SWEEP_MIN).to_radians();
                let there = swept(right, point, turn);
                let pivot = pivot_feet(right);
                let radius = |p: [f64; 3]| (p[0] - pivot[0]).hypot(p[2] - pivot[2]);
                assert!((radius(there) - radius(point)).abs() < 1e-12);
                assert_eq!(there[1], point[1]);
                let back = unswept(right, there, turn);
                for i in 0..3 {
                    assert!((back[i] - point[i]).abs() < 1e-12);
                }
                // A point outboard of the pivot moves aft as the wing sweeps.
                if turn > 0. {
                    assert!(there[2] < point[2] + 1e-12 || point[2] < pivot[2]);
                }
            }
        }
    }

    #[test]
    fn only_the_tornados_inner_wing_pair_and_its_side_rail_swivel() {
        let rows: Vec<usize> = (0..9)
            .filter(|row| swivels(AircraftId::Tornado, *row))
            .collect();
        // Row 7, the inner wing pair, and row 8, the AIM-9 side rails on its
        // pylons (TOR-6b, 2026-11-11).
        assert_eq!(rows, [7, 8]);
        assert!(has_swivel(AircraftId::Tornado));
        for id in [AircraftId::F15E, AircraftId::Mig23, AircraftId::F14] {
            assert!(!has_swivel(id));
            assert!((0..9).all(|row| !swivels(id, row)));
        }
    }

    #[test]
    fn the_pylon_swivels_back_by_the_wings_turn() {
        assert_eq!(pylon_swivel(true, 0.5), -0.5);
        assert_eq!(pylon_swivel(false, 0.5), 0.5);
    }
}
