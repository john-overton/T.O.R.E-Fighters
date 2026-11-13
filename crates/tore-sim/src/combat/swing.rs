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
//! `TOR.INF` (Jane's). Update (TOR-11, 2026-11-12): the wing starts to sweep at
//! 350 mph, 304 knots (opinionated, John 2026-11-12; was 250 knots, fitted).
//! Update (TOR-12, 2026-11-12): the wing is placed 1.54 ft in, 0.1167 ft up and
//! 0.20 ft forward of the retail panel, mirrored left to right, with new pivots
//! ([`PIVOT_R`], [`WING_IN_FT`]), so the stations that ride on it move with it.
//! Update (TOR-14, 2026-11-13): every swivelling pylon is centred on the wing
//! panel's mid-chord at its span ([`mid_chord`]) and has one fixed length
//! ([`PYLON_LENGTH_FT`]) and depth, whatever the sweep.
use tore_formats::aircraft::AircraftId;

/// Shape units to feet at the Tornado's header scale 2 (2/3 ft a unit).
pub const FEET_PER_UNIT: f64 = 2. / 3.;
/// Real minimum and maximum wing sweep, degrees (`TOR.INF`, Jane's); the
/// shape's spread pose is the minimum.
pub const SWEEP_MIN: f64 = 25.;
pub const SWEEP_MAX: f64 = 67.;
/// Speed schedule, knots: the wing stays spread up to the first speed and is
/// fully swept at the second. Flaps hold the wing out. The end points are
/// fitted (agent decision 2026-11-10, after the MiG-23's rig); the start is
/// John's (2026-11-12: "I don't think for the tornado the wings should start
/// sweeping back until it's about 350 mph", opinionated): 350 mph is 304
/// knots, where it was a fitted 250. The full sweep stays 600 knots, so the
/// sweep now runs 304 to 600 knots, linear (about 0.14 degrees a knot, from
/// 0.12).
pub const SWEEP_FROM_KNOTS: f64 = 304.;
pub const SWEEP_TO_KNOTS: f64 = 600.;
/// Feet per second to knots.
pub const FPS_PER_KNOT: f64 = 1.68781;
/// The wing sweep pivots, shape units (right, forward, up), vertical axis.
/// Update (TOR-12, John 2026-11-12, from his Blender review of the wing): the
/// right pivot is (4.3333, -0.9007, 0.31) ft right, forward and up, that is
/// (6.5, -1.351025, 0.465) shape units, and the left pivot mirrors it. The old
/// pivots (9.5, -1.5) right and (-10.5, -1.5) left were the centre of the
/// smallest circle round the fixed glove corners the wing panel shared (fitted,
/// TOR-3); the glove fillets are gone, so that reason went with them.
pub const PIVOT_R: [f32; 3] = [6.5, -1.351_025, 0.465];
pub const PIVOT_L: [f32; 3] = [-6.5, -1.351_025, 0.465];

/// How the retail right wing panel is placed in the spread pose (TOR-12, John
/// 2026-11-12, from his Blender review): the whole panel moves 1.54 ft in
/// (toward the fuselage), 0.1167 ft up and 0.20 ft forward, and its root points
/// (the five vertices on the glove, x 9 and 10 units) move 2.3483 ft inboard
/// along the wing's span axis, so the root stays inside the fuselage through
/// the sweep. The left panel is an exact mirror of the right one: the retail
/// left panel sat one unit further out, and the placement drops that
/// asymmetry. Opinionated (John's measured values, not a source's).
pub const WING_IN_FT: f64 = 1.54;
pub const WING_UP_FT: f64 = 0.175 * FEET_PER_UNIT;
pub const WING_FORWARD_FT: f64 = 0.2;
pub const ROOT_SHIFT_FT: f64 = 2.3483;

/// The placed right wing panel's leading and trailing edges in the spread
/// pose, each as its inboard and outboard corner [right, forward] in feet
/// (TOR-14, 2026-11-13). They are the panel's outline corners (retail's shape
/// units, 2/3 ft each: leading edge (9, 7) to (31, -2), trailing edge, the
/// flap strip's outer edge, (10, -4) to (30, -9)) with John's placement
/// ([`WING_IN_FT`], [`WING_FORWARD_FT`] and, for the two root corners,
/// [`ROOT_SHIFT_FT`]) applied, exactly as `tornado_animation::wing_point`
/// places them; a test there pins the two against each other. The left panel
/// mirrors them.
pub const WING_LEADING: [[f64; 2]; 2] = [
    [
        9. * FEET_PER_UNIT - ROOT_SHIFT_FT - WING_IN_FT,
        7. * FEET_PER_UNIT + WING_FORWARD_FT,
    ],
    [
        31. * FEET_PER_UNIT - WING_IN_FT,
        -2. * FEET_PER_UNIT + WING_FORWARD_FT,
    ],
];
pub const WING_TRAILING: [[f64; 2]; 2] = [
    [
        10. * FEET_PER_UNIT - ROOT_SHIFT_FT - WING_IN_FT,
        -4. * FEET_PER_UNIT + WING_FORWARD_FT,
    ],
    [
        30. * FEET_PER_UNIT - WING_IN_FT,
        -9. * FEET_PER_UNIT + WING_FORWARD_FT,
    ],
];

/// Where along an edge of the placed panel (`edge`, two [right, forward]
/// corners) the span station `right` feet out is: its forward coordinate.
const fn edge_forward(edge: [[f64; 2]; 2], right: f64) -> f64 {
    let share = (right - edge[0][0]) / (edge[1][0] - edge[0][0]);
    edge[0][1] + share * (edge[1][1] - edge[0][1])
}

/// The forward coordinate (feet, aircraft frame, wing spread) of the middle
/// of the placed panel's chord at the span station `right` feet out of the
/// reference point: halfway between the leading edge and the trailing edge
/// (the flap strip's trailing edge, flaps up) along the streamwise line at
/// that span. The chord is the panel's own local section, so a pylon centred
/// there stays on the wing's middle through the sweep (TOR-14, 2026-11-13,
/// John: "all pylons need to be centered on the wing"; the definition of
/// mid-chord is the agent's). Right-hand wing; the left mirrors it.
pub const fn mid_chord(right: f64) -> f64 {
    (edge_forward(WING_LEADING, right) + edge_forward(WING_TRAILING, right)) / 2.
}

/// How long every swivelling wing pylon is drawn, feet, whatever the sweep or
/// the store (TOR-14, 2026-11-13; fitted, agent decision): 3.3 ft, the length
/// the Mk 82 pack's pylon had (0.45 of the 7.3 ft bomb), which is a little over
/// half the 6.4 ft panel chord at the inner pylon and 60 percent of the 5.46 ft
/// chord at the outer pods, so the pylon stays inside the panel's outline with
/// the wing spread, and its aft end stays within 0.4 ft of the flap strip's
/// hinge line (the strip droops onto at most 0.17 ft of it with the flaps
/// down). It was a share of the store's length (0.45 of it, at least twice its
/// depth) trimmed to wherever the wing's surface was under it, which was 5.9 ft
/// for the 13 ft pod and grew with the spread wing.
pub const PYLON_LENGTH_FT: f64 = 3.3;

/// The fixed length of the swivelling pylon of source hardpoint `hardpoint`
/// of aircraft `id`, when it has one ([`swivels`]).
pub fn pylon_length(id: AircraftId, hardpoint: usize) -> Option<f64> {
    swivels(id, hardpoint).then_some(PYLON_LENGTH_FT)
}

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
/// Update (TOR-13, 2026-11-12): rows 9 and 10 are the Tornado IDS (Combat)'s
/// outer wing pylons with the BOZ-107 and the Sky Shadow
/// ([`tore_formats::aircraft::stations::authored_pods`]); they are not source
/// rows, only station indices past them, and they swivel with the wing like
/// rows 7 and 8. Both Tornado versions share rows 7 and 8
/// ([`AircraftId::source`]).
pub fn swivels(id: AircraftId, hardpoint: usize) -> bool {
    id.source() == AircraftId::Tornado && matches!(hardpoint, 7..=10)
}

/// Whether `id` has any swivelling station.
pub fn has_swivel(id: AircraftId) -> bool {
    id.source() == AircraftId::Tornado
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schedule_spreads_below_304_knots_and_sweeps_fully_at_600() {
        assert_eq!(wing_turn(200. * FPS_PER_KNOT, 0.), 0.);
        // John, 2026-11-12: 350 mph is 304 knots, and the wing is still
        // spread there; the old 250 knots start is gone.
        assert_eq!(wing_turn(SWEEP_FROM_KNOTS * FPS_PER_KNOT, 0.), 0.);
        assert_eq!(wing_turn(250. * FPS_PER_KNOT, 0.), 0.);
        assert_eq!(wing_turn(300. * FPS_PER_KNOT, 0.), 0.);
        assert!((SWEEP_FROM_KNOTS - 350. * 0.868_976).abs() < 0.5);
        assert!(wing_turn(305. * FPS_PER_KNOT, 0.) > 0.);
        let full = (SWEEP_MAX - SWEEP_MIN).to_radians();
        assert!((wing_turn(600. * FPS_PER_KNOT, 0.) - full).abs() < 1e-12);
        assert!((wing_turn(900. * FPS_PER_KNOT, 0.) - full).abs() < 1e-12);
        assert!((wing_turn(452. * FPS_PER_KNOT, 0.) - full / 2.).abs() < 1e-12);
        // Flaps hold the wing out.
        assert_eq!(wing_turn(900. * FPS_PER_KNOT, 1.), 0.);
    }

    #[test]
    fn the_pivots_are_johns_and_mirror_each_other() {
        // John, 2026-11-12 (TOR-12): right pivot (4.3333, -0.9007, 0.31) ft.
        let r = pivot_feet(true);
        assert!((r[0] - 4.333_333).abs() < 1e-5);
        assert!((r[2] + 0.900_683).abs() < 1e-5);
        assert!((r[1] - 0.31).abs() < 1e-5);
        let l = pivot_feet(false);
        assert_eq!((l[0], l[1], l[2]), (-r[0], r[1], r[2]));
        assert!((WING_UP_FT - 0.116_666_7).abs() < 1e-6);
    }

    #[test]
    fn the_mid_chord_is_halfway_between_the_placed_panels_edges() {
        // TOR-14: the placed right panel's leading edge runs from (2.1117,
        // 4.8667) to (19.1267, -1.1333) ft and its trailing edge from (2.7784,
        // -2.4667) to (18.46, -5.8), read off the drawn panel's corners.
        assert!((WING_LEADING[0][0] - 2.1117).abs() < 1e-4);
        assert!((WING_LEADING[0][1] - 4.8667).abs() < 1e-4);
        assert!((WING_LEADING[1][0] - 19.1267).abs() < 1e-4);
        assert!((WING_LEADING[1][1] + 1.1333).abs() < 1e-4);
        assert!((WING_TRAILING[0][0] - 2.7784).abs() < 1e-4);
        assert!((WING_TRAILING[0][1] + 2.4667).abs() < 1e-4);
        assert!((WING_TRAILING[1][0] - 18.46).abs() < 1e-4);
        assert!((WING_TRAILING[1][1] + 5.8).abs() < 1e-4);
        // The inner pylon's span (7.96 ft out) and the outer pods' (14.46).
        assert!(
            (mid_chord(7.96) + 0.3815).abs() < 1e-3,
            "{}",
            mid_chord(7.96)
        );
        assert!(
            (mid_chord(14.46) + 2.2187).abs() < 1e-3,
            "{}",
            mid_chord(14.46)
        );
        // Aft with the span, as the panel's chord runs.
        assert!(mid_chord(14.46) < mid_chord(7.96));
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
    fn only_the_tornados_wing_pylons_and_the_outer_pod_pylons_swivel() {
        let rows: Vec<usize> = (0..12)
            .filter(|row| swivels(AircraftId::Tornado, *row))
            .collect();
        // Row 7, the inner wing pair, row 8, the AIM-9 side rails on its
        // pylons (TOR-6b, 2026-11-11), and stations 9 and 10, the Combat
        // version's outer pod pylons (TOR-13, 2026-11-12). The Clean version
        // swivels the same rows (it has no pod on 9 and 10).
        assert_eq!(rows, [7, 8, 9, 10]);
        assert!(has_swivel(AircraftId::Tornado));
        for row in 0..12 {
            assert_eq!(
                swivels(AircraftId::TornadoClean, row),
                swivels(AircraftId::Tornado, row)
            );
        }
        assert!(has_swivel(AircraftId::TornadoClean));
        for id in [AircraftId::F15E, AircraftId::Mig23, AircraftId::F14] {
            assert!(!has_swivel(id));
            assert!((0..12).all(|row| !swivels(id, row)));
        }
    }

    #[test]
    fn the_pylon_swivels_back_by_the_wings_turn() {
        assert_eq!(pylon_swivel(true, 0.5), -0.5);
        assert_eq!(pylon_swivel(false, 0.5), 0.5);
    }
}
