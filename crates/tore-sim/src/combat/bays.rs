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
];

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
    pub inner: f64,
    pub outer: f64,
    pub aft: f64,
    pub fore: f64,
    pub belly: f64,
    pub depth: f64,
    /// Where each bay's trapeze rail runs, feet forward (aft end, fore end),
    /// along the bay's centre line under the recess roof.
    pub rail: [f64; 2],
}

/// Half the width of a trapeze rail, and how far it hangs below the recess
/// roof, feet (fitted, F117-5).
pub const RAIL_HALF: f64 = 0.2;
pub const RAIL_DROP: f64 = 0.6;

/// Fitted, agent-authored, F117-5 (2026-11-15). The real door count was not
/// found; two doors a bay is fitted.
pub const OPENINGS: &[Opening] = &[Opening {
    aircraft: AircraftId::F117,
    inner: 0.2,
    outer: 5.8,
    aft: -9.0,
    fore: 9.0,
    belly: F117_BELLY,
    depth: 2.3,
    rail: [-5.5, 3.5],
}];

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
        assert!(!reviewed(AircraftId::F16C) && !reviewed(AircraftId::B2));
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
