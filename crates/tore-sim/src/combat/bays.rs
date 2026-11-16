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

/// The B-52's belly plane, feet up: the flat belly panel of `B52.SH` at z -9
/// units, 2/3 ft a unit (measured, B52-5). The retail bay rows hang at -13.3.
pub const B52_BELLY: f64 = -6.0;
/// Where the B-52's three bay rows (`B52.PT` rows 3, 5 and 6, in that order)
/// hang their stores, feet forward: the retail 39, 13 and -13 record units,
/// centred in the drawn 26 ft bay (-8.67 to +17.33 ft, the word `7a20`
/// plates), so each row owns an 8.67 ft slab of it.
pub const B52_ROW_FORWARD: [f64; 3] = [13., 13. / 3., -13. / 3.];
/// The length of one row's slab of the bay, feet.
pub const B52_SLAB: f64 = 26. / 3.;
/// How far ahead of a 500 lb class bomb's middle its shape's origin (the point
/// a store hangs and leaves from) lies, feet: the `MK82.SH` family spans -4.52
/// to +2.78 ft about its origin (measured, B52-5). A row's mount is its slab's
/// middle plus this, so the drawn bomb is centred in the slab.
pub const B52_ORIGIN_AHEAD: f64 = 0.87;
/// The B-52's per-row grid data as named constants, in the shape of a row of a
/// grid table (floor width, length, ceiling), for the merge with B2-5's `GRIDS`
/// (the coordinator moves the three rows there; B2-5's pitch rule, the span plus
/// 0.03 ft capped at 1.3 ft, replaces [`GRID_GAP`]'s span plus 0.15 ft). Feet.
/// Floor width: the opening's, 6 ft ([`B52_BAY_HALF`] each side of the centre
/// line, `centre` 0). Length: one row's slab ([`B52_SLAB`]). Ceiling: the roof
/// above the belly plane ([`B52_BAY_DEPTH`], flat, so `inner_depth` equals
/// `depth`); the opening runs the 26 ft of [`CENTRE_BAYS`] with no rail between
/// the rows' rails (they run the whole bay).
pub const B52_ROW_FLOOR_WIDTH: f64 = 2. * B52_BAY_HALF;
pub const B52_ROW_LENGTH: f64 = B52_SLAB;
pub const B52_ROW_CEILING: f64 = B52_BAY_DEPTH;

/// How deep the B-52's recess is under the belly plane, feet: fitted, so the
/// third tier of 500 lb class stores (fin span 1.7 ft) stands under the roof
/// and the roof stands inside the fuselage, whose top skin is 13.3 ft above
/// the belly (agent decision, B52-5, 2026-11-15).
pub const B52_BAY_DEPTH: f64 = 6.0;
/// Half the width of the B-52's bay opening, feet: the real bay is 6 ft wide
/// (aircraftinformation.info), and the belly's flat panel is 6.7 ft wide.
pub const B52_BAY_HALF: f64 = 3.0;
/// The gap a grid leaves between neighbouring stores, feet (fitted).
pub const GRID_GAP: f64 = 0.15;

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
    // The B-52's three bay rows (B52-5): the retail rows 3, 5 and 6, lifted
    // from 13.3 ft below the reference point to the belly plane, each the
    // 3 x 3 grid of [`GRIDS`] over its 8.67 ft slab of the bay.
    BayStation {
        aircraft: AircraftId::B52,
        row: 3,
        mount: [0., B52_BELLY, B52_ROW_FORWARD[0] + B52_ORIGIN_AHEAD],
    },
    BayStation {
        aircraft: AircraftId::B52,
        row: 5,
        mount: [0., B52_BELLY, B52_ROW_FORWARD[1] + B52_ORIGIN_AHEAD],
    },
    BayStation {
        aircraft: AircraftId::B52,
        row: 6,
        mount: [0., B52_BELLY, B52_ROW_FORWARD[2] + B52_ORIGIN_AHEAD],
    },
];

/// How a bay station lays its stores out, for a station holding more than one
/// store abreast or stacked: a grid of `lanes` side by side and `tiers` high,
/// one store deep (the row's slab is one store long). Fitted (B52-5, agent
/// decision). Stores fill the grid from the bottom tier up and, in a tier,
/// from the middle lane outward, right then left; the highest slot, the last
/// loaded, leaves first, so a row releases its bottom tier first and nothing
/// drops through another store. The lane and tier pitch follow the store's
/// size ([`GRID_GAP`] beyond its span).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BayGrid {
    pub aircraft: AircraftId,
    pub row: usize,
    pub lanes: u16,
    pub tiers: u16,
}

/// The aircraft whose bay rows are a grid; the F-22 and the F-117 hang their
/// stores in tandem and have none.
pub const GRIDS: &[BayGrid] = &[
    BayGrid {
        aircraft: AircraftId::B52,
        row: 3,
        lanes: 3,
        tiers: 3,
    },
    BayGrid {
        aircraft: AircraftId::B52,
        row: 5,
        lanes: 3,
        tiers: 3,
    },
    BayGrid {
        aircraft: AircraftId::B52,
        row: 6,
        lanes: 3,
        tiers: 3,
    },
];

impl BayGrid {
    /// The grid of the bay station mounted at `mount`, if it has one (a
    /// rack knows its mount, not its aircraft; the mounts of [`STATIONS`]
    /// are distinct).
    pub fn at(mount: [f64; 3]) -> Option<Self> {
        let grid = GRIDS.iter().find(|g| {
            STATIONS
                .iter()
                .find(|s| s.aircraft == g.aircraft && s.row == g.row)
                .is_some_and(|s| (0..3).all(|i| (s.mount[i] - mount[i]).abs() < 1e-6))
        });
        grid.copied()
    }
    /// The pitch of lanes and tiers for stores of this width and top
    /// (feet).
    pub fn pitch(width: f64, top: f64) -> f64 {
        width.max(2. * top) + GRID_GAP
    }
    /// The cell of the store that leaves `rank`th from a row loaded with
    /// `loaded` stores: (lane offset in lane pitches, tier). The row fills
    /// from the bottom tier, the middle lane first.
    pub fn cell(&self, rank: u16) -> (i32, u16) {
        let rank = rank.min(self.lanes * self.tiers - 1);
        let (tier, lane) = (rank / self.lanes, rank % self.lanes);
        let offset = match lane {
            0 => 0,
            n if n % 2 == 1 => (i32::from(n) + 1) / 2,
            n => -i32::from(n) / 2,
        };
        (offset, tier)
    }
    /// Where slot `slot` of a row loaded with `loaded` stores of `width` and
    /// `top` hangs, feet right, up and forward, on a station at `mount`.
    pub fn point(&self, mount: [f64; 3], loaded: u16, slot: u16, width: f64, top: f64) -> [f64; 3] {
        let loaded = loaded.clamp(1, self.lanes * self.tiers);
        let rank = loaded - 1 - slot.min(loaded - 1);
        let (lane, tier) = self.cell(rank);
        let pitch = Self::pitch(width, top);
        [
            mount[0] + f64::from(lane) * pitch,
            mount[1] + width.max(2. * top) / 2. + 0.08 + f64::from(tier) * pitch,
            mount[2],
        ]
    }
}

/// The belly cut-out of an aircraft whose bay is one centred opening of two
/// doors (the B-52): `half` feet either side of the centre line, from `aft` to
/// `fore` feet forward, cut into the belly plane at `belly` feet up with a
/// recess `depth` feet deep above it; each door is half the width, hinged on
/// its outer edge and swinging 90 degrees down. `unit` is the shape's feet a
/// unit (2/3 for `B52.SH`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CentreBay {
    pub aircraft: AircraftId,
    pub half: f64,
    pub aft: f64,
    pub fore: f64,
    pub belly: f64,
    pub depth: f64,
    pub unit: f64,
}

/// Fitted, agent-authored, B52-5 (2026-11-15). The real bay has three
/// interconnected door sections on each side; two leaves are drawn.
pub const CENTRE_BAYS: &[CentreBay] = &[CentreBay {
    aircraft: AircraftId::B52,
    half: B52_BAY_HALF,
    aft: -13. * 2. / 3.,
    fore: 26. * 2. / 3.,
    belly: B52_BELLY,
    depth: B52_BAY_DEPTH,
    unit: 2. / 3.,
}];

/// The centred bay of `id`, if it has one.
pub fn centre_bay(id: AircraftId) -> Option<CentreBay> {
    let id = id.source();
    CENTRE_BAYS.iter().find(|o| o.aircraft == id).copied()
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
    fn the_b52_has_three_row_grids_in_one_centred_bay() {
        let rows: Vec<_> = [3, 5, 6]
            .map(|row| station(AircraftId::B52, row).unwrap())
            .into();
        assert!(reviewed(AircraftId::B52) && station(AircraftId::B52, 4).is_none());
        let bay = centre_bay(AircraftId::B52).unwrap();
        // The bay is 26 ft long, 6 ft wide, and the three rows share it
        // equally, 8.67 ft apart, from the retail 13, 4.3 and -4.3 ft.
        assert!((bay.fore - bay.aft - 26.).abs() < 1e-9 && bay.half * 2. == 6.);
        for mount in &rows {
            assert_eq!((mount[0], mount[1]), (0., B52_BELLY));
            let middle = mount[2] - B52_ORIGIN_AHEAD;
            assert!(middle - B52_SLAB / 2. >= bay.aft - 1e-9);
            assert!(middle + B52_SLAB / 2. <= bay.fore + 1e-9);
        }
        assert!((rows[0][2] - rows[1][2] - B52_SLAB).abs() < 1e-9);
        assert!((rows[1][2] - rows[2][2] - B52_SLAB).abs() < 1e-9);
        // A grid is found by its mount, and only for the B-52.
        let grid = BayGrid::at(rows[1]).unwrap();
        assert_eq!((grid.lanes, grid.tiers, grid.row), (3, 3, 5));
        assert!(BayGrid::at(station(AircraftId::F117, 4).unwrap()).is_none());
    }

    #[test]
    fn a_grid_fills_from_the_bottom_middle_and_the_last_loaded_leaves_first() {
        let grid = BayGrid::at(station(AircraftId::B52, 3).unwrap()).unwrap();
        let mount = station(AircraftId::B52, 3).unwrap();
        // Nine Mk 82s (1.27 ft span, 0.45 ft radius): 3 lanes at 1.42 ft, 3
        // tiers; slot 8 leaves first from the bottom middle, slot 0 last from
        // the top left.
        let at = |loaded, slot| grid.point(mount, loaded, slot, 1.27, 0.45);
        let pitch = BayGrid::pitch(1.27, 0.45);
        assert!((pitch - 1.42).abs() < 1e-9);
        assert_eq!(at(9, 8)[0], 0.);
        assert!((at(9, 7)[0] - pitch).abs() < 1e-9 && (at(9, 6)[0] + pitch).abs() < 1e-9);
        assert!((at(9, 8)[1] - (B52_BELLY + 0.635 + 0.08)).abs() < 1e-9);
        assert!((at(9, 0)[1] - at(9, 8)[1] - 2. * pitch).abs() < 1e-9);
        // Every cell is its own: nine stores, nine places, all inside the bay.
        let mut places: Vec<_> = (0..9).map(|k| at(9, k)).collect();
        places.dedup();
        assert_eq!(places.len(), 9);
        assert!(places.iter().all(|p| p[0].abs() + 0.635 < B52_BAY_HALF));
        // Three loaded sit on the bottom tier, the first loaded in the middle.
        assert!((at(3, 2)[1] - at(9, 8)[1]).abs() < 1e-9 && at(3, 2)[0] == 0.);
        assert!((0..3).all(|k| (at(3, k)[1] - at(9, 8)[1]).abs() < 1e-9));
        // Nothing hangs below the one it releases through.
        for loaded in 1..=9 {
            for slot in 1..loaded {
                assert!(at(loaded, slot)[1] <= at(loaded, slot - 1)[1] + 1e-9);
            }
        }
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
