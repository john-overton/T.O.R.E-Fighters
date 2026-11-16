//! Internal weapon bays: which aircraft have reviewed ones, where each bay
//! station hangs its stores and where its belly opening is
//! (docs/spec/stores-on-pylons.md, "F-22 naval stores" and "F-117 weapon bays").
//!
//! A bay station is one source hardpoint record whose location byte is the
//! source's internal bay (3). [`super::hardpoints::placed_on`] re-places it as
//! a fitted [`super::hardpoints::Location::Magazine`] at the mount in
//! [`STATIONS`], and [`State::bay_available`](crate::flight::State::bay_available)
//! is true for every aircraft that has a row here, so the doors open for a
//! release and close after it (`docs/spec/aircraft-animation.md`, "F-22 main
//! weapon bays"), and the open-bay signature term applies
//! (`docs/radar.md`, "Agent-authored signature model"). A new aircraft with
//! bays (the B-2, the B-52) adds rows to the tables below and its own door and
//! lining art in the drawing crate; nothing else in the simulation changes.
//! A bay that holds many stores (the B-2's) also takes a row in [`GRIDS`]:
//! its stores then hang in a grid fitted to the floor, abreast, along and in
//! tiers, instead of one lane (B2-5, docs/spec/stores-on-pylons.md, "B-2 weapon bays").
//!
//! The F-22's belly cuts and lining stay in `roster_animation` with their own
//! constants (unchanged). [`OPENINGS`] holds the belly cut-outs of the
//! aircraft whose doors are drawn by the shared helper (`bay_doors.rs` in the
//! app).
use tore_formats::aircraft::AircraftId;

/// One bay station: a source row and where its stores hang, feet right, up and
/// forward of the reference point. `up` is the floor of the magazine: a store
/// hangs half its fin span or body diameter (whichever is larger) plus 0.08 ft
/// above it ([`super::hardpoints::Rack::store_point`]), so for a bay with a
/// lowered trapeze it is the height of the lowest fin tip, not of a floor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BayStation {
    pub aircraft: AircraftId,
    pub row: usize,
    pub mount: [f64; 3],
}

/// The F-117's belly plane, feet up (`F117.SH` z -5 units at the height factor
/// 0.8869, a third of a foot a unit: -4.4345 units, -1.4782 ft).
pub const F117_BELLY: f64 = -1.4782;
/// How far the F-117's fin tips hang below its belly plane, feet. The GBU-27's
/// tail fins span 5.4 ft in a fuselage 3.3 to 4.1 ft thick over the bays, so
/// the store hangs on its trapeze with its axis 1.0 ft above the belly plane:
/// the lower fin tips hang between the open doors (which are 2.8 ft deep) and
/// the upper ones reach about 0.6 ft above the top skin (measured in the
/// bay clearance table; fitted, agent decision, F117-5, 2026-11-15).
pub const F117_TIP_BELOW_BELLY: f64 = 1.7;
/// The F-117's bay stations (rows 4 and 5, flags `0x1100`, one store each):
/// 3.0 ft either side of the centreline, 5.6 ft openings with a 0.4 ft keel
/// between them, wide enough for the GBU-27's 5.4 ft fin span (the F117-0
/// estimate of 2.55 ft would cross the two stores' fin tips). Fitted.
pub const F117_BAY_X: f64 = 3.0;

/// The B-2's two bays (B2-5, John's decision 21; measured in `B2.SH`, a third of
/// a foot a unit): centres at x -16 and +14 units, the release origin 20 units
/// forward, 8.0 ft wide and 19.3 ft long. Right, forward, feet.
pub const B2_BAY_X: [f64; 2] = [-16. / 3., 14. / 3.];
pub const B2_BAY_FORWARD: f64 = 20. / 3.;
/// The belly polygon's plane (`B2.SH` face `396d`, z -11 units): a store's
/// lowest fin tip sits on it, so the floor of the magazine is here. The
/// retail "-3.7 ft" of John's decision 21, rounded.
pub const B2_FLOOR: f64 = -11. / 3.;
/// How far the Paveway III rows hang below the Mk 82 rows' floor, feet (B2-5,
/// fitted): the GBU-27's 5.4 ft fins are taller than the bay is deep over
/// its outer columns, so its trapeze hangs 0.9 ft lower, the lower fin tips
/// 0.9 ft under the belly polygon between the open doors.
pub const B2_PAVEWAY_DROP: f64 = 0.9;
/// The lowest skin over the bays, the door panels' plane (z -12 units, 4.0 ft
/// down): the doors hinge here and the lined recess starts here.
pub const B2_BELLY: f64 = -12. / 3.;

/// Every bay station, in aircraft and row order. Add a row per station.
pub const STATIONS: &[BayStation] = &[
    BayStation {
        aircraft: AircraftId::F22,
        row: 5,
        mount: [-5. / 3., -3., 1.4],
    },
    BayStation {
        aircraft: AircraftId::F22,
        row: 6,
        mount: [1.5, -3., 1.4],
    },
    BayStation {
        aircraft: AircraftId::F22,
        row: 7,
        mount: [3.9, -1.2, 1.7],
    },
    BayStation {
        aircraft: AircraftId::F22n,
        row: 5,
        mount: [-5. / 3., -3., 1.4],
    },
    BayStation {
        aircraft: AircraftId::F22n,
        row: 6,
        mount: [1.5, -3., 1.4],
    },
    BayStation {
        aircraft: AircraftId::F22n,
        row: 7,
        mount: [3.9, -1.05, 1.7],
    },
    BayStation {
        aircraft: AircraftId::F117,
        row: 4,
        mount: [-F117_BAY_X, F117_BELLY - F117_TIP_BELOW_BELLY - 0.08, 0.],
    },
    BayStation {
        aircraft: AircraftId::F117,
        row: 5,
        mount: [F117_BAY_X, F117_BELLY - F117_TIP_BELOW_BELLY - 0.08, 0.],
    },
    // The B-2's four bay rows: 5 and 6 the Mk 82 rows, 7 and 8 the Paveway III
    // rows (hung lower), one pair of rows a bay (left, right). A row is one
    // whole bay.
    BayStation {
        aircraft: AircraftId::B2,
        row: 5,
        mount: [B2_BAY_X[0], B2_FLOOR, B2_BAY_FORWARD],
    },
    BayStation {
        aircraft: AircraftId::B2,
        row: 6,
        mount: [B2_BAY_X[1], B2_FLOOR, B2_BAY_FORWARD],
    },
    BayStation {
        aircraft: AircraftId::B2,
        row: 7,
        mount: [B2_BAY_X[0], B2_FLOOR - B2_PAVEWAY_DROP, B2_BAY_FORWARD],
    },
    BayStation {
        aircraft: AircraftId::B2,
        row: 8,
        mount: [B2_BAY_X[1], B2_FLOOR - B2_PAVEWAY_DROP, B2_BAY_FORWARD],
    },
];

/// A bay station whose stores hang in a grid: as many abreast (across the bay)
/// and along it (front to back) as fit the `width` and `length` of its floor,
/// feet, a tier of that many to a layer and as many tiers as the load needs
/// (`hardpoints::grid_point`), up to the `ceiling` (feet up of the reference
/// point, the lowest the roof is over the stores: `hardpoints::grid_capacity`).
/// Rows with no entry keep the single lane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BayGrid {
    pub aircraft: AircraftId,
    pub row: usize,
    pub width: f64,
    pub length: f64,
    pub ceiling: f64,
}

/// The B-2's usable floor, feet (B2-5, fitted from the measured 8.0 by 19.3 ft
/// bay, John's decision 21): 5.3 ft wide, four stores of 1.3 ft abreast and 0.1 ft over, and
/// the bay's whole length, so a Mk 82 (7.3 ft) hangs 4 abreast and 2 long, a
/// Paveway III (13.9 ft) 4 abreast and 1 long, as the decision has it.
pub const B2_GRID: [f64; 2] = [5.3, 19.33];
/// How high the B-2's stacks may reach, feet up: 5.5 ft over the belly
/// polygon's plane (measured, the outer columns' top skin over four tiers of
/// Mk 82 is 0.3 ft clear of a stack 5.25 ft high; a fifth tier, the real 80
/// Mk 82, is 1.0 ft through it).
pub const B2_CEILING: f64 = B2_FLOOR + 5.5;

/// Every gridded bay station (B2-5, fitted).
pub const GRIDS: &[BayGrid] = &[
    BayGrid {
        aircraft: AircraftId::B2,
        row: 5,
        width: B2_GRID[0],
        length: B2_GRID[1],
        ceiling: B2_CEILING,
    },
    BayGrid {
        aircraft: AircraftId::B2,
        row: 6,
        width: B2_GRID[0],
        length: B2_GRID[1],
        ceiling: B2_CEILING,
    },
    BayGrid {
        aircraft: AircraftId::B2,
        row: 7,
        width: B2_GRID[0],
        length: B2_GRID[1],
        ceiling: B2_CEILING,
    },
    BayGrid {
        aircraft: AircraftId::B2,
        row: 8,
        width: B2_GRID[0],
        length: B2_GRID[1],
        ceiling: B2_CEILING,
    },
];

/// The grid floor of bay station `row` of `id` (a variant reads its source's):
/// its usable width and length, feet, and its ceiling, if it has one.
pub fn grid(id: AircraftId, row: usize) -> Option<[f64; 3]> {
    let id = id.source();
    GRIDS
        .iter()
        .find(|g| g.aircraft == id && g.row == row)
        .map(|g| [g.width, g.length, g.ceiling])
}

/// Pairs of rows that share one bay: a bay holds one or the other (the B-2's
/// Mk 82 and Paveway III rows). Each pair is listed once.
pub const ALTERNATIVES: &[(AircraftId, usize, usize)] =
    &[(AircraftId::B2, 5, 7), (AircraftId::B2, 6, 8)];

/// The row that shares bay station `row`'s bay and so must stay empty while
/// it is loaded, if any.
pub fn alternative(id: AircraftId, row: usize) -> Option<usize> {
    let id = id.source();
    ALTERNATIVES.iter().find_map(|&(a, one, two)| {
        (a == id).then_some(())?;
        if row == one {
            Some(two)
        } else if row == two {
            Some(one)
        } else {
            None
        }
    })
}

/// The mount of bay station `row` of `id` (a variant reads its source's), if
/// it has one.
pub fn station(id: AircraftId, row: usize) -> Option<[f64; 3]> {
    let id = id.source();
    STATIONS
        .iter()
        .find(|s| s.aircraft == id && s.row == row)
        .map(|s| s.mount)
}

/// Whether `id` has reviewed internal bays: doors that open for a release.
pub fn reviewed(id: AircraftId) -> bool {
    let id = id.source();
    STATIONS.iter().any(|s| s.aircraft == id)
}

/// A belly cut-out and its doors, for the aircraft whose doors the shared door
/// helper draws. The right-hand bay spans `inner` to `outer` feet from the
/// centreline and the left mirrors it; both run from `aft` to `fore` feet
/// forward, cut into the belly plane at `belly` feet up, with a recess `depth`
/// feet deep above it. Each bay has two doors, half its width, hinged at the
/// inner and outer edges and swinging 90 degrees down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opening {
    pub aircraft: AircraftId,
    /// Feet right of the reference point of the line the two bays mirror about
    /// (0 on the F-117; the B-2's mesh axis is a third of a foot left).
    pub centre: f64,
    pub inner: f64,
    pub outer: f64,
    pub aft: f64,
    pub fore: f64,
    pub belly: f64,
    /// How deep the recess is above `belly`, at its outer wall. The B-2's roof
    /// rises toward its inner wall (the skin over it does), so the recess
    /// ceiling is one sloped plane under the skin: `inner_depth` is the depth
    /// at the inner wall (equal to `depth` for a flat roof).
    pub depth: f64,
    pub inner_depth: f64,
    /// Where each bay's trapeze rail runs, feet forward (aft end, fore end),
    /// along the bay's centre line under the recess roof; equal ends, no rail.
    pub rail: [f64; 2],
}

/// Half the width of a trapeze rail, and how far it hangs below the recess
/// roof, feet (fitted, F117-5).
pub const RAIL_HALF: f64 = 0.2;
pub const RAIL_DROP: f64 = 0.6;

/// Fitted, agent-authored, F117-5 (2026-11-15). The real door count was not
/// found; two doors a bay is fitted.
pub const OPENINGS: &[Opening] = &[
    Opening {
        aircraft: AircraftId::F117,
        centre: 0.,
        inner: 0.2,
        outer: 5.8,
        aft: -9.0,
        fore: 9.0,
        belly: F117_BELLY,
        depth: 2.3,
        inner_depth: 2.3,
        rail: [-5.5, 3.5],
    },
    // The B-2 (B2-5, measured): the door panels `3c84` and `3c65` span x -28 to
    // -4 and 2 to 26 units, y -9 to 49 units, about the mesh axis x -1 unit; the
    // recess roof follows the top skin, 5.0 ft over the outer edge to 11.3 ft
    // over the inner (fitted under it, see the clearance table). No rail.
    Opening {
        aircraft: AircraftId::B2,
        centre: -1. / 3.,
        inner: 1.0,
        outer: 9.0,
        aft: -3.0,
        fore: 49. / 3.,
        belly: B2_BELLY,
        depth: 4.6,
        inner_depth: 9.6,
        rail: [0., 0.],
    },
];

/// The opening of `id`, if the shared helper draws its doors.
pub fn opening(id: AircraftId) -> Option<Opening> {
    let id = id.source();
    OPENINGS.iter().find(|o| o.aircraft == id).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_f22_stations_are_unchanged_and_the_f117_has_two() {
        assert_eq!(station(AircraftId::F22, 5), Some([-5. / 3., -3., 1.4]));
        assert_eq!(station(AircraftId::F22, 7), Some([3.9, -1.2, 1.7]));
        assert_eq!(station(AircraftId::F22n, 7), Some([3.9, -1.05, 1.7]));
        assert_eq!(station(AircraftId::F22, 4), None);
        let left = station(AircraftId::F117, 4).unwrap();
        let right = station(AircraftId::F117, 5).unwrap();
        assert_eq!((left[0], right[0]), (-F117_BAY_X, F117_BAY_X));
        assert_eq!(left[1], right[1]);
        assert_eq!(station(AircraftId::F117, 3), None);
        assert!(reviewed(AircraftId::F117) && reviewed(AircraftId::F22n));
        assert!(!reviewed(AircraftId::F16C) && reviewed(AircraftId::B2));
    }

    #[test]
    fn the_b2_has_two_bays_of_two_rows_a_grid_each_and_one_roof() {
        let o = opening(AircraftId::B2).unwrap();
        // One bay's two rows share its mount: the middle of its opening, 6.7
        // ft forward, on the belly polygon's plane.
        for (row, bay, other) in [(5, 0, 7), (6, 1, 8)] {
            let mount = station(AircraftId::B2, row).unwrap();
            assert_eq!(mount, [B2_BAY_X[bay], B2_FLOOR, B2_BAY_FORWARD]);
            // The Paveway row of the bay hangs lower on the same centre.
            let low = station(AircraftId::B2, other).unwrap();
            assert_eq!(low, [mount[0], B2_FLOOR - B2_PAVEWAY_DROP, mount[2]]);
            let side = if bay == 0 { -1. } else { 1. };
            let middle = o.centre + side * (o.inner + o.outer) / 2.;
            assert!((mount[0] - middle).abs() < 1e-9);
            assert!(mount[2] > o.aft && mount[2] < o.fore);
            assert_eq!(alternative(AircraftId::B2, row), Some(other));
            assert_eq!(alternative(AircraftId::B2, other), Some(row));
        }
        assert_eq!(station(AircraftId::B2, 4), None);
        assert_eq!(alternative(AircraftId::B2, 4), None);
        assert_eq!(alternative(AircraftId::F117, 4), None);
        // Every row has the same floor; the F-22's and F-117's keep their lane.
        assert_eq!(
            grid(AircraftId::B2, 6),
            Some([B2_GRID[0], B2_GRID[1], B2_CEILING])
        );
        assert_eq!(grid(AircraftId::B2, 8), grid(AircraftId::B2, 5));
        assert_eq!(grid(AircraftId::F22, 5), None);
        assert_eq!(grid(AircraftId::F117, 4), None);
        // The 8.0 ft by 19.3 ft bay, and a roof that rises toward the inner wall.
        assert!((o.outer - o.inner - 8.0).abs() < 1e-9);
        assert!((o.fore - o.aft - 19.333).abs() < 1e-3);
        assert!(o.inner_depth > o.depth && o.rail[0] == o.rail[1]);
    }

    #[test]
    fn the_f117_bays_are_wide_enough_for_a_gbu27_and_do_not_overlap() {
        let o = opening(AircraftId::F117).unwrap();
        let tip = 5.4 / 2.;
        let x = F117_BAY_X;
        // The 5.4 ft fin span fits between the door planes, and the two
        // bays leave a keel between their inner doors.
        assert!(x - tip > o.inner && x + tip < o.outer);
        assert!(o.inner > 0.);
        // A 13.9 ft GBU-27 (-7.94 to +5.96 ft about its mount) fits the length.
        assert!(o.aft < -7.94 && o.fore > 5.96);
    }
}
