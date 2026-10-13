//! Where a station's stores hang and where each one leaves from
//! (docs/spec/stores-on-pylons.md, B3).
//!
//! A station is one source hardpoint record: one position (right, up and
//! forward of the aircraft's reference point, source units of a third of a
//! foot) and a quantity. A paired record carries its stores on both sides:
//! the record's position is the right-hand pylon and the left-hand one is its
//! mirror. Retail mirrors the lateral offset of a hardpoint with type flag 0x4
//! when the station's remaining count divided by `actualRoundsPerGame` is odd
//! (`HARDUnrotatedHardPos`, docs/formats/weapons.md#release-state), so
//! successive releases alternate sides. The same rule says how many stores
//! each side still holds, which is what the drawn pylons show.
use crate::attitude::Vector;
use tore_formats::aircraft::Hardpoint;

/// Retail hardpoint type flag 0x4: the record is a left and right pair.
pub const PAIRED_FLAG: i32 = 0x4;

/// How far either side of a wing record's position its two pylons stand,
/// feet, when a wing carries two or more of its stores (John 2026-10-12:
/// four stores on one wing row hang from two pylons on each wing). 2.8 ft
/// apart clears a Maverick's 2.4 ft fins; fitted (agent decision).
pub const DUAL_PYLON_SPREAD: f64 = 1.4;

/// The source location byte of a hardpoint record
/// (docs/formats/aircraft-ordnance.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Location {
    #[default]
    Centerline,
    Fuselage,
    InternalGun,
    InternalBay,
    Wing,
    Wingtip,
    /// A byte outside the six reviewed names.
    Other(u8),
}

impl Location {
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0 => Self::Centerline,
            1 => Self::Fuselage,
            2 => Self::InternalGun,
            3 => Self::InternalBay,
            4 => Self::Wing,
            5 => Self::Wingtip,
            other => Self::Other(other),
        }
    }
    pub fn byte(self) -> u8 {
        match self {
            Self::Centerline => 0,
            Self::Fuselage => 1,
            Self::InternalGun => 2,
            Self::InternalBay => 3,
            Self::Wing => 4,
            Self::Wingtip => 5,
            Self::Other(byte) => byte,
        }
    }
    /// Stores here hang outside the aircraft, on a pylon or rail.
    pub fn external(self) -> bool {
        matches!(
            self,
            Self::Centerline | Self::Fuselage | Self::Wing | Self::Wingtip
        )
    }
}

/// How a station's stores sit: its source location and whether the record
/// is a left and right pair.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rack {
    /// The source location byte ([`Location::from_byte`]).
    pub location: u8,
    /// Stores alternate between the record's position and its mirror.
    pub paired: bool,
}

impl Rack {
    /// The rack of a source hardpoint record. Paired: retail's flag 0x4
    /// (spec-derived), and also every wing or wingtip record, so a wing
    /// record retail leaves one-sided (the A-4E's inner Mk 82 row, the
    /// Mi-24's AT-2 rails) loads both wings (agent decision, 2026-10-12). An
    /// internal gun never pairs, whatever its flags.
    pub fn from_source(h: &Hardpoint) -> Self {
        let location = Location::from_byte(h.location);
        Self {
            location: h.location,
            paired: location != Location::InternalGun
                && (h.flags & PAIRED_FLAG != 0
                    || matches!(location, Location::Wing | Location::Wingtip)),
        }
    }
    pub fn location(self) -> Location {
        Location::from_byte(self.location)
    }
    /// A paired record whose position is off the centreline: one that has
    /// two sides to alternate between.
    pub fn two_sided(self, mount: Vector) -> bool {
        self.paired && mount[0].abs() > 1e-9
    }
    /// Where the next store leaves, relative to the aircraft (feet right,
    /// up and forward): the record's position, or its mirror when the
    /// station is paired and `units_after`, the whole releases it holds
    /// once this one has gone, is odd. A two-store pair leaves left, then
    /// right.
    pub fn release_mount(self, mount: Vector, units_after: u32) -> Vector {
        if self.two_sided(mount) && units_after % 2 == 1 {
            [-mount[0], mount[1], mount[2]]
        } else {
            mount
        }
    }
    /// How many pylons one side of the station has when that side was
    /// loaded with `per_side` stores: a wing side with two stores, or four
    /// or more, has two pylons (an inboard and an outboard one); three hang
    /// on one pylon's triple ejector rack, as do several stores at a
    /// fuselage, centreline or wingtip station (agent decision, 2026-10-12).
    pub fn pylons(self, per_side: u16) -> u16 {
        if self.location() == Location::Wing && (per_side == 2 || per_side >= 4) {
            2
        } else {
            1
        }
    }
    /// The pylon of store `slot` (0 first loaded, the highest empties first)
    /// on a side loaded with `per_side`, and how far outboard of the
    /// record's position that pylon is, feet: on a two-pylon side slot 0
    /// is inboard, 1 outboard, and further slots alternate (so the outboard
    /// pylon's stores leave first).
    pub fn pylon(self, per_side: u16, slot: u16) -> (u16, f64) {
        if self.pylons(per_side) < 2 {
            return (0, 0.);
        }
        let pylon = slot % 2;
        (
            pylon,
            if pylon == 0 {
                -DUAL_PYLON_SPREAD
            } else {
                DUAL_PYLON_SPREAD
            },
        )
    }
    /// Where the next store leaves, with its own pylon on a two-pylon wing
    /// side: [`Self::release_mount`], moved out or in by the pylon of the
    /// store that leaves. `loaded` is the stores the station was loaded with
    /// and `slot_after` how many its leaving side still holds once this one
    /// has gone (which is that store's slot).
    pub fn release_point(
        self,
        mount: Vector,
        loaded: u16,
        units_after: u32,
        slot_after: u16,
    ) -> Vector {
        let mut point = self.release_mount(mount, units_after);
        let side = usize::from(point[0] < 0. && self.two_sided(mount));
        let per_side = self.sides(mount, loaded)[side];
        let (_, outboard) = self.pylon(per_side, slot_after.min(per_side.saturating_sub(1)));
        point[0] += outboard * point[0].signum();
        point
    }
    /// How many of `items` stores hang on each side, [right, left]: the
    /// right side holds the odd one, because the left side empties first
    /// ([`Self::release_mount`]). An unpaired record holds them all at its
    /// own position, reported as the right side.
    pub fn sides(self, mount: Vector, items: u16) -> [u16; 2] {
        if self.two_sided(mount) {
            [items.div_ceil(2), items / 2]
        } else {
            [items, 0]
        }
    }
}

/// Whole releases a station holds: its rounds over the rounds one release
/// debits (`actualRoundsPerGame`, at least one).
pub fn units(rounds: u32, debit: u32) -> u32 {
    rounds / debit.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hardpoint(location: u8, flags: i32) -> Hardpoint {
        Hardpoint {
            location,
            flags,
            position: [30, -6, -3],
            store: None,
            count: 2,
            weight_class: 0,
        }
    }

    #[test]
    fn retail_flag_and_wing_rows_pair_and_guns_never_do() {
        assert!(Rack::from_source(&hardpoint(1, 0x0085)).paired);
        assert!(!Rack::from_source(&hardpoint(3, 0x05e0)).paired, "F-22 bay");
        assert!(Rack::from_source(&hardpoint(4, 0x1781)).paired, "A-4E wing");
        assert!(
            Rack::from_source(&hardpoint(4, 0x0188)).paired,
            "Mi-24 rail"
        );
        assert!(!Rack::from_source(&hardpoint(2, 0x000c)).paired, "A-7 gun");
        assert!(!Rack::from_source(&hardpoint(0, 0x0681)).paired);
    }

    #[test]
    fn a_pair_leaves_left_then_right_and_the_sides_follow() {
        let rack = Rack {
            location: 4,
            paired: true,
        };
        let mount = [10., -2., -1.];
        // Four stores: after each release 3, 2, 1, 0 remain.
        let sides: Vec<f64> = (0..4)
            .rev()
            .map(|after| rack.release_mount(mount, after)[0])
            .collect();
        assert_eq!(sides, [-10., 10., -10., 10.]);
        assert_eq!(rack.sides(mount, 4), [2, 2]);
        assert_eq!(rack.sides(mount, 3), [2, 1]);
        assert_eq!(rack.sides(mount, 1), [1, 0]);
        // The side a store leaves from is the side that loses one.
        for items in 1..8u16 {
            let before = rack.sides(mount, items);
            let after = rack.sides(mount, items - 1);
            let left = rack.release_mount(mount, u32::from(items - 1))[0] < 0.;
            assert_eq!(before[usize::from(left)] - 1, after[usize::from(left)]);
        }
    }

    #[test]
    fn centreline_and_unpaired_records_keep_their_position() {
        let pair = Rack {
            location: 0,
            paired: true,
        };
        assert_eq!(pair.release_mount([0., -5., 0.], 1), [0., -5., 0.]);
        assert_eq!(pair.sides([0., -5., 0.], 3), [3, 0]);
        let bay = Rack {
            location: 3,
            paired: false,
        };
        assert_eq!(bay.release_mount([-3., -8., 0.], 1), [-3., -8., 0.]);
        assert_eq!(units(38, 1), 38);
        assert_eq!(units(4, 2), 2);
        assert_eq!(units(4, 0), 4);
    }

    #[test]
    fn a_two_pylon_wing_releases_outboard_then_inboard_from_each_pylon() {
        let wing = Rack {
            location: 4,
            paired: true,
        };
        let mount = [10., -2., -1.];
        let s = DUAL_PYLON_SPREAD;
        // Four loaded, two a wing: left out, right out, left in, right in.
        let order: Vec<f64> = (0..4u32)
            .rev()
            .map(|after| {
                let side = usize::from(wing.release_mount(mount, after)[0] < 0.);
                let slot = wing.sides(mount, after as u16)[side];
                wing.release_point(mount, 4, after, slot)[0]
            })
            .collect();
        assert_eq!(order, [-(10. + s), 10. + s, -(10. - s), 10. - s]);
        // Two loaded (one a wing) and fuselage pairs keep the record's point.
        assert_eq!(wing.release_point(mount, 2, 1, 0)[0], -10.);
        let fuselage = Rack {
            location: 1,
            paired: true,
        };
        assert_eq!(fuselage.release_point(mount, 4, 3, 1)[0], -10.);
        assert_eq!(wing.pylons(3), 1);
        assert_eq!(wing.pylon(6, 5), (1, s));
    }
}
