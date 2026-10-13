//! Where a station's stores hang and where each one leaves from
//! (docs/spec/stores-on-pylons.md, B3, BP2 and PY2).
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
//!
//! Every store hangs [`PYLON_DEPTH`] below the model's underside over its
//! pylon (John 2026-10-13: pylons 2 ft deep), not at the record's height,
//! and leaves from exactly where it hangs ([`Rack::store_point`]), so the
//! simulation and the drawing place a store at one point.
use super::store_sizes::StoreSize;
use crate::attitude::Vector;
use tore_formats::aircraft::Hardpoint;
use tore_formats::shape::Shape;

/// Retail hardpoint type flag 0x4: the record is a left and right pair.
pub const PAIRED_FLAG: i32 = 0x4;

/// How far either side of a wing record's position its two pylons stand,
/// feet, when a wing carries two or more of its stores (John 2026-10-12:
/// four stores on one wing row hang from two pylons on each wing). 2.8 ft
/// apart clears a Maverick's 2.4 ft fins; fitted (agent decision).
pub const DUAL_PYLON_SPREAD: f64 = 1.4;

/// A pylon's depth, feet, from the model's underside over it to the top of
/// the store (or rack beam) it carries: real pylons are 15 to 30 inches
/// (opinionated, John 2026-10-13: 2 ft on every aircraft).
pub const PYLON_DEPTH: f64 = 2.0;

/// A rack beam's depth under its pylon, feet (fitted).
pub const RACK_DEPTH: f64 = 0.3;

/// The clear gap between two fuselage stores one behind another, feet
/// (fitted, agent decision).
pub const TANDEM_GAP: f64 = 1.0;

/// How far ahead and behind a fuselage record the model's underside is
/// sampled for its tandem pylons, feet; a tandem pylon's surface is
/// interpolated from these.
pub const TANDEM_SAMPLE: f64 = 7.0;

/// How far either side of the centreline a fuselage pair recorded on the
/// centreline hangs, feet: the F-4E's and F-4G's Sparrow wells, which
/// retail records at x 0 (fitted, agent decision).
pub const FUSELAGE_WELL: f64 = 3.0;

/// How far either side of a pylon the model's underside is read, feet: the
/// store hangs under the lowest surface across its body (fitted).
pub const CLEARANCE: f64 = 0.6;

/// Where the model's underside is sampled over a station's pylons, by index
/// into [`Rack::over`].
pub mod sample {
    /// The record's own position (a single pylon).
    pub const RECORD: usize = 0;
    /// A two-pylon wing's inboard pylon.
    pub const INBOARD: usize = 1;
    /// A two-pylon wing's outboard pylon.
    pub const OUTBOARD: usize = 2;
    /// [`super::TANDEM_SAMPLE`] ahead of the record.
    pub const FRONT: usize = 3;
    /// [`super::TANDEM_SAMPLE`] behind the record.
    pub const REAR: usize = 4;
}

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

/// How a station's stores sit: its source location, whether the record is a
/// left and right pair, and the model's underside over its pylons.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rack {
    /// The source location byte ([`Location::from_byte`]).
    pub location: u8,
    /// Stores alternate between the record's position and its mirror.
    pub paired: bool,
    /// The height (feet up of the aircraft's reference point) of the model's
    /// underside over the station's pylons, at the [`sample`] points; `None`
    /// where nothing of the model is above (a wingtip rail), and all `None`
    /// when the model was not read. A store under a known surface hangs
    /// [`PYLON_DEPTH`] below it; otherwise at the record's height.
    pub over: [Option<f32>; 5],
}

/// One pylon of one side of a station: the point its stores hang from (feet
/// right, up and forward of the aircraft, already mirrored for the left
/// side; a lone store's origin) and the side's slots it carries, in loading
/// order.
#[derive(Clone, Debug, PartialEq)]
pub struct PylonPlace {
    pub point: Vector,
    pub slots: Vec<u16>,
}

impl Rack {
    /// The rack of a source hardpoint record. Paired: retail's flag 0x4
    /// (spec-derived), and also every wing or wingtip record, so a wing
    /// record retail leaves one-sided (the Mi-24's AT-2 rails) loads both
    /// wings (agent decision, 2026-10-12). An internal gun never pairs,
    /// whatever its flags. The model's underside is not known ([`placed`]).
    pub fn from_source(h: &Hardpoint) -> Self {
        let location = Location::from_byte(h.location);
        Self {
            location: h.location,
            paired: location != Location::InternalGun
                && (h.flags & PAIRED_FLAG != 0
                    || matches!(location, Location::Wing | Location::Wingtip)),
            over: [None; 5],
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
    /// The record's position or its mirror, whichever side the next store
    /// leaves: the mirror when the station is paired and `units_after`, the
    /// whole releases it holds once this one has gone, is odd. A two-store
    /// pair leaves left, then right.
    pub fn release_mount(self, mount: Vector, units_after: u32) -> Vector {
        if self.two_sided(mount) && units_after % 2 == 1 {
            [-mount[0], mount[1], mount[2]]
        } else {
            mount
        }
    }
    /// How many pylons one side of the station has when that side was
    /// loaded with `per_side` stores: a wing side with two stores, or four
    /// or more, has two pylons (an inboard and an outboard one), and a
    /// fuselage side the same count one behind another (John 2026-10-13:
    /// fuselage missile pairs in tandem, as on the F-14, F-15 and F-4);
    /// three hang on one pylon's triple ejector rack, as do several stores
    /// at a centreline or wingtip station (agent decision).
    pub fn pylons(self, per_side: u16) -> u16 {
        if matches!(self.location(), Location::Wing | Location::Fuselage)
            && (per_side == 2 || per_side >= 4)
        {
            2
        } else {
            1
        }
    }
    /// The pylon of store `slot` (0 first loaded, the highest empties
    /// first) on a side loaded with `per_side` stores `length` feet long,
    /// and where that pylon is against the record's position, [outboard,
    /// forward] feet. On a two-pylon side slot 0 is on pylon 0 and slot 1 on
    /// pylon 1, further slots alternating, so pylon 1's stores leave first:
    /// a wing's pylon 1 is the outboard one, a fuselage's the front one,
    /// spaced so the stores clear by [`TANDEM_GAP`].
    pub fn pylon(self, per_side: u16, slot: u16, length: f64) -> (u16, [f64; 2]) {
        if self.pylons(per_side) < 2 {
            return (0, [0.; 2]);
        }
        let pylon = slot % 2;
        let sign = if pylon == 0 { -1. } else { 1. };
        let offset = if self.location() == Location::Fuselage {
            [0., sign * (length + TANDEM_GAP) / 2.]
        } else {
            [sign * DUAL_PYLON_SPREAD, 0.]
        };
        (pylon, offset)
    }
    /// The slots of a side loaded with `per_side` stores of `length` that
    /// hang on `pylon`.
    fn slots_on(self, per_side: u16, pylon: u16, length: f64) -> Vec<u16> {
        (0..per_side)
            .filter(|s| self.pylon(per_side, *s, length).0 == pylon)
            .collect()
    }
    /// The model's underside over a pylon `offset` ([outboard, forward]
    /// feet) from the record, when it is known.
    fn surface(self, offset: [f64; 2]) -> Option<f64> {
        let at = |i: usize| self.over[i].map(f64::from);
        if offset[0] < -1e-9 {
            at(sample::INBOARD)
        } else if offset[0] > 1e-9 {
            at(sample::OUTBOARD)
        } else if offset[1].abs() > 1e-9 {
            let end = if offset[1] > 0. {
                sample::FRONT
            } else {
                sample::REAR
            };
            match (at(sample::RECORD), at(end)) {
                (Some(here), Some(there)) => {
                    Some(here + (there - here) * offset[1].abs() / TANDEM_SAMPLE)
                }
                (here, there) => here.or(there),
            }
        } else {
            at(sample::RECORD)
        }
    }
    /// The pylons of one side (0 right, 1 left) of a station at `mount`
    /// loaded with `per_side` stores of `size` on that side: where each
    /// hangs its stores and which slots it carries. A pylon under a known
    /// surface hangs its stores' lugs (or its rack beam) [`PYLON_DEPTH`]
    /// below it; a wingtip rail or an unknown surface keeps the record's
    /// height. Internal stations have one place, the record's.
    pub fn side_pylons(
        self,
        mount: Vector,
        side: usize,
        per_side: u16,
        size: StoreSize,
    ) -> Vec<PylonPlace> {
        let mirror = if side == 1 { -1. } else { 1. };
        (0..self.pylons(per_side))
            .map(|p| {
                let slots = self.slots_on(per_side, p, size.length);
                let offset = slots
                    .first()
                    .map_or([0.; 2], |s| self.pylon(per_side, *s, size.length).1);
                let right = if mount[0].abs() > 1e-9 {
                    (mount[0] + offset[0] * mount[0].signum()) * mirror
                } else {
                    0.
                };
                let rack = if slots.len() >= 2 { RACK_DEPTH } else { 0. };
                let up = match self.surface(offset) {
                    Some(surface)
                        if self.location().external() && self.location() != Location::Wingtip =>
                    {
                        surface - PYLON_DEPTH - rack - size.top
                    }
                    _ => mount[1],
                };
                PylonPlace {
                    point: [right, up, mount[2] + offset[1]],
                    slots,
                }
            })
            .collect()
    }
    /// Where store `slot` of side `side` (0 right, 1 left) hangs, feet right,
    /// up and forward of the aircraft, on a station at `mount` loaded with
    /// `loaded` stores of `size`: its pylon's point and its place on the
    /// pylon's rack ([`slot_offset`]). The drawing hangs it here and it
    /// leaves from here. An internal station's stores all leave from its
    /// record's position.
    pub fn store_point(
        self,
        mount: Vector,
        loaded: u16,
        side: usize,
        slot: u16,
        size: StoreSize,
    ) -> Vector {
        let per_side = self.sides(mount, loaded)[side.min(1)].max(1);
        let slot = slot.min(per_side - 1);
        let mirror = if side == 1 && self.two_sided(mount) {
            -1.
        } else {
            1.
        };
        if !self.location().external() {
            return [mount[0] * mirror, mount[1], mount[2]];
        }
        let (pylon, _) = self.pylon(per_side, slot, size.length);
        let places = self.side_pylons(mount, side.min(1), per_side, size);
        let Some(place) = places.get(usize::from(pylon)) else {
            return [mount[0] * mirror, mount[1], mount[2]];
        };
        let k = place.slots.iter().position(|s| *s == slot).unwrap_or(0);
        let on = place.slots.len() as u16;
        let offset = slot_offset(k as u16, on, size.length, size.width);
        std::array::from_fn(|i| place.point[i] + offset[i])
    }
    /// How far the model's underside over store `slot`'s pylon is above
    /// the store's lugs, feet ([`PYLON_DEPTH`], plus [`RACK_DEPTH`] on a
    /// rack), or `None` for a store on a wingtip rail, inside the aircraft
    /// or under no known surface. The battery's release scenario reports it.
    pub fn store_depth(
        self,
        mount: Vector,
        loaded: u16,
        side: usize,
        slot: u16,
        size: StoreSize,
    ) -> Option<f64> {
        if !self.location().external() || self.location() == Location::Wingtip {
            return None;
        }
        let per_side = self.sides(mount, loaded)[side.min(1)].max(1);
        let (pylon, offset) = self.pylon(per_side, slot.min(per_side - 1), size.length);
        let surface = self.surface(offset)?;
        let place = self
            .side_pylons(mount, side.min(1), per_side, size)
            .into_iter()
            .nth(usize::from(pylon))?;
        Some(surface - (place.point[1] + size.top))
    }
    /// Where the next store leaves, relative to the aircraft (feet right, up
    /// and forward): where it hangs ([`Self::store_point`]) on the side
    /// [`Self::release_mount`] picks. `loaded` is the stores the station was
    /// loaded with and `slot_after` how many its leaving side still holds
    /// once this one has gone (which is that store's slot).
    pub fn release_point(
        self,
        mount: Vector,
        loaded: u16,
        units_after: u32,
        slot_after: u16,
        size: StoreSize,
    ) -> Vector {
        let left = self.release_mount(mount, units_after)[0] < 0. && self.two_sided(mount);
        self.store_point(mount, loaded, usize::from(left), slot_after, size)
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

/// How many stores one rack group holds on a pylon carrying `on`: four hang
/// as two pairs one behind another (a multiple ejector rack), otherwise in
/// threes (a triple ejector rack: two shoulders and one below).
pub fn rack_group(on: u16) -> u16 {
    if on == 4 { 2 } else { 3 }
}

/// Where the `k`th store of a pylon carrying `on` stores of `length` and
/// `width` (across its fins) sits against the pylon's point: feet right, up
/// and forward. One sits at the point; two side by side, 0.55 widths either
/// side; three as a triple ejector rack, the third 0.95 widths below; more
/// as such racks ([`rack_group`]) one behind another, 1.05 lengths apart
/// (fitted, agent decision).
pub fn slot_offset(k: u16, on: u16, length: f64, width: f64) -> Vector {
    if on <= 1 {
        return [0.; 3];
    }
    let spread = 0.55 * width;
    let per = rack_group(on);
    let groups = on.div_ceil(per);
    let group = k / per;
    let forward = (f64::from(group) - f64::from(groups - 1) / 2.) * 1.05 * length;
    let [right, up] = match k % per {
        0 => [-spread, 0.],
        1 => [spread, 0.],
        _ => [0., -0.95 * width],
    };
    [right, up, forward]
}

/// The position and rack of source hardpoint record `h` as the stores hang
/// (feet right, up and forward), with the model's underside over its pylons
/// read from `model` when there is one. Two records are moved to where real
/// stores hang (agent decisions, 2026-10-13): a one-sided wing record within
/// 3 ft of the centreline (the A-4E's Mk 82 row, 2.7 ft out, which retail
/// releases from one side) is a centreline rack; a paired fuselage weapon
/// record on the centreline holding two or more (the F-4E's and F-4G's
/// Sparrows) hangs [`FUSELAGE_WELL`] either side, in its wells.
pub fn placed(h: &Hardpoint, model: Option<&Underside>) -> (Vector, Rack) {
    let mut mount = h.position.map(|v| f64::from(v) / 3.);
    let mut rack = Rack::from_source(h);
    match rack.location() {
        Location::Wing if h.flags & PAIRED_FLAG == 0 && mount[0].abs() < 3. => {
            mount[0] = 0.;
            rack.location = Location::Centerline.byte();
            rack.paired = false;
        }
        Location::Fuselage
            if rack.paired
                && mount[0].abs() < 1e-9
                && h.count >= 2
                && h.store.as_deref().is_some_and(|s| s.ends_with(".JT")) =>
        {
            mount[0] = FUSELAGE_WELL;
        }
        _ => {}
    }
    if let Some(model) = model
        && rack.location().external()
        && rack.location() != Location::Wingtip
    {
        let right = mount[0].abs();
        let points = [
            (right, mount[2]),
            ((right - DUAL_PYLON_SPREAD).max(0.), mount[2]),
            (right + DUAL_PYLON_SPREAD, mount[2]),
            (right, mount[2] + TANDEM_SAMPLE),
            (right, mount[2] - TANDEM_SAMPLE),
        ];
        // The lowest surface across the store's body, so a store beside a
        // nacelle or a body's flank hangs clear of it.
        let surface = |r: f64, f: f64| {
            [-CLEARANCE, 0., CLEARANCE]
                .iter()
                .filter_map(|o| model.height((r + o).max(0.), f, mount[1], 6))
                .reduce(f64::min)
        };
        rack.over = points.map(|(r, f)| surface(r, f).map(|h| h as f32));
    }
    (mount, rack)
}

/// An aircraft model's faces in feet, for finding its underside over a
/// pylon: the clean shape, right, forward and up.
#[derive(Clone, Debug, Default)]
pub struct Underside {
    triangles: Vec<[[f64; 3]; 3]>,
}

impl Underside {
    /// `shape`'s faces, scaled to feet by `scale`.
    pub fn of(shape: &Shape, scale: f64) -> Self {
        let mut triangles = Vec::new();
        for face in &shape.faces {
            let p: Vec<[f64; 3]> = face
                .positions
                .iter()
                .map(|q| q.map(|v| f64::from(v) * scale))
                .collect();
            for i in 1..p.len().saturating_sub(1) {
                triangles.push([p[0], p[i], p[i + 1]]);
            }
        }
        Self { triangles }
    }
    /// The height (feet up) of the lowest face straight over the point
    /// `right`, `forward` (feet) that is at most 12 ft above `from`, or
    /// `None` where there is none.
    pub fn over(&self, right: f64, forward: f64, from: f64) -> Option<f64> {
        let mut lowest: Option<f64> = None;
        for [a, b, c] in &self.triangles {
            // Barycentric weights of the point in the triangle's plan view.
            let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
            if d.abs() < 1e-9 {
                continue;
            }
            let u = ((b[1] - c[1]) * (right - c[0]) + (c[0] - b[0]) * (forward - c[1])) / d;
            let v = ((c[1] - a[1]) * (right - c[0]) + (a[0] - c[0]) * (forward - c[1])) / d;
            let w = 1. - u - v;
            if u < 0. || v < 0. || w < 0. {
                continue;
            }
            let height = u * a[2] + v * b[2] + w * c[2] - from;
            if (0. ..=12.).contains(&height) && lowest.is_none_or(|l| height < l) {
                lowest = Some(height);
            }
        }
        lowest.map(|h| from + h)
    }
    /// [`Self::over`] at `right` (the model is mirror symmetric, so a
    /// left-hand point is looked up on the right) and, when nothing is
    /// straight over it, up to `inboard_steps` half feet inboard (a pod
    /// under the gap between a body and a nacelle).
    pub fn height(&self, right: f64, forward: f64, from: f64, inboard_steps: u8) -> Option<f64> {
        let right = right.abs();
        (0..=inboard_steps).find_map(|step| self.over(right - f64::from(step) * 0.5, forward, from))
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

    fn rack(location: u8) -> Rack {
        Rack {
            location,
            paired: true,
            ..Default::default()
        }
    }

    /// A store 10 ft long, 1 ft across its body and 2 ft across its fins.
    const SIZE: StoreSize = StoreSize {
        length: 10.,
        top: 0.5,
        width: 2.,
    };

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
        let rack = rack(4);
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
        let pair = rack(0);
        assert_eq!(pair.release_mount([0., -5., 0.], 1), [0., -5., 0.]);
        assert_eq!(pair.sides([0., -5., 0.], 3), [3, 0]);
        let bay = Rack {
            location: 3,
            ..Default::default()
        };
        assert_eq!(bay.release_mount([-3., -8., 0.], 1), [-3., -8., 0.]);
        assert_eq!(
            bay.release_point([-3., -8., 0.], 2, 1, 1, SIZE),
            [-3., -8., 0.]
        );
        assert_eq!(units(38, 1), 38);
        assert_eq!(units(4, 2), 2);
        assert_eq!(units(4, 0), 4);
    }

    /// The order a station of `loaded` stores lets them go: where each
    /// leaves, as the release picks its side and slot.
    fn order(rack: Rack, mount: Vector, loaded: u16) -> Vec<Vector> {
        (0..u32::from(loaded))
            .rev()
            .map(|after| {
                let side = usize::from(rack.release_mount(mount, after)[0] < 0.);
                let slot = rack.sides(mount, after as u16)[side];
                rack.release_point(mount, loaded, after, slot, SIZE)
            })
            .collect()
    }

    #[test]
    fn a_two_pylon_wing_releases_outboard_then_inboard_from_each_pylon() {
        let wing = rack(4);
        let mount = [10., -2., -1.];
        let s = DUAL_PYLON_SPREAD;
        // Four loaded, two a wing: left out, right out, left in, right in.
        let xs: Vec<f64> = order(wing, mount, 4).iter().map(|p| p[0]).collect();
        assert_eq!(xs, [-(10. + s), 10. + s, -(10. - s), 10. - s]);
        // Two loaded (one a wing) keep the record's point.
        assert_eq!(wing.release_point(mount, 2, 1, 0, SIZE)[0], -10.);
        assert_eq!(wing.pylons(3), 1);
        assert_eq!(wing.pylon(6, 5, 10.), (1, [s, 0.]));
    }

    #[test]
    fn a_store_hangs_two_feet_below_the_surface_over_its_pylon() {
        // The surface over the record is 1 ft up, 1.5 ft inboard and 0.5 ft
        // outboard of it (a wing's dihedral).
        let wing = Rack {
            over: [Some(1.), Some(1.5), Some(0.5), None, None],
            ..rack(4)
        };
        let mount = [10., -6., 0.];
        // One a side: the store's top (its lugs) 2 ft under the record's
        // surface, whatever the record's own height.
        let one = wing.store_point(mount, 2, 0, 0, SIZE);
        assert!((one[1] + SIZE.top - (1. - PYLON_DEPTH)).abs() < 1e-9);
        // Two a side: each under its own pylon's surface.
        let inboard = wing.store_point(mount, 4, 1, 0, SIZE);
        let outboard = wing.store_point(mount, 4, 1, 1, SIZE);
        assert!((inboard[1] + SIZE.top - (1.5 - PYLON_DEPTH)).abs() < 1e-9);
        assert!((outboard[1] + SIZE.top - (0.5 - PYLON_DEPTH)).abs() < 1e-9);
        assert!(inboard[0] < 0. && outboard[0] < inboard[0], "left side");
        // Three on one pylon: the rack beam sits between the stores and the
        // pylon, and the shoulders hang under it.
        let three = wing.store_point(mount, 6, 0, 0, SIZE);
        assert!((three[1] + SIZE.top - (1. - PYLON_DEPTH - RACK_DEPTH)).abs() < 1e-9);
        // A wingtip rail, or no surface found, keeps the record's height.
        let tip = Rack {
            location: 5,
            ..wing
        };
        assert_eq!(tip.store_point(mount, 2, 0, 0, SIZE), mount);
        assert_eq!(rack(4).store_point(mount, 2, 0, 0, SIZE), mount);
    }

    #[test]
    fn fuselage_pairs_hang_one_behind_another_and_the_front_ones_leave_first() {
        let fuselage = Rack {
            over: [Some(-3.), None, None, Some(-2.), Some(-4.)],
            ..rack(1)
        };
        let mount = [3., -8., 5.];
        let half = (SIZE.length + TANDEM_GAP) / 2.;
        assert_eq!(fuselage.pylons(2), 2);
        assert_eq!(fuselage.pylons(1), 1);
        let points = order(fuselage, mount, 4);
        // Left front, right front, left rear, right rear.
        let expect = [
            [-3., 5. + half],
            [3., 5. + half],
            [-3., 5. - half],
            [3., 5. - half],
        ];
        for (point, [right, forward]) in points.iter().zip(expect) {
            assert!((point[0] - right).abs() < 1e-9 && (point[2] - forward).abs() < 1e-9);
        }
        // Spaced by the store's length and the gap: they do not touch.
        assert!((points[0][2] - points[2][2]).abs() - SIZE.length >= TANDEM_GAP - 1e-9);
        // Each hangs 2 ft under the surface over it, interpolated between
        // the samples 7 ft ahead and behind.
        let surface = |forward_of_record: f64| -3. + forward_of_record / TANDEM_SAMPLE;
        for point in &points {
            let lugs = point[1] + SIZE.top;
            assert!((lugs - (surface(point[2] - 5.) - PYLON_DEPTH)).abs() < 1e-9);
        }
    }

    #[test]
    fn a_centreline_rack_of_four_is_two_pairs_one_behind_another() {
        let offsets: Vec<Vector> = (0..4).map(|k| slot_offset(k, 4, 8., 1.)).collect();
        assert!(offsets[0][0] < 0. && offsets[1][0] > 0. && offsets[0][2] < 0.);
        assert!(offsets[2][0] < 0. && offsets[3][0] > 0. && offsets[2][2] > 0.);
        assert!(offsets.iter().all(|o| o[1] == 0.));
        // Three: a triple rack, the third below.
        assert!(slot_offset(2, 3, 8., 1.)[1] < -0.9);
        // Six: two triple racks, one ahead of the other.
        assert!(slot_offset(0, 6, 8., 1.)[2] < 0. && slot_offset(3, 6, 8., 1.)[2] > 0.);
    }

    #[test]
    fn records_move_to_real_stations_and_read_the_model_over_them() {
        // The A-4E's one-sided Mk 82 row 2.7 ft out: a centreline rack.
        let a4e = Hardpoint {
            location: 4,
            flags: 0x1781,
            position: [8, -16, -6],
            store: Some("MK82.JT".into()),
            count: 4,
            weight_class: 0,
        };
        let (mount, rack) = placed(&a4e, None);
        assert_eq!(mount[0], 0.);
        assert_eq!(rack.location(), Location::Centerline);
        assert!(!rack.paired);
        // The F-4E's Sparrows, recorded on the centreline: in their wells.
        let f4e = Hardpoint {
            location: 1,
            flags: 0x00c5,
            position: [0, -20, 0],
            store: Some("AIM7.JT".into()),
            count: 4,
            weight_class: 0,
        };
        let (mount, rack) = placed(&f4e, None);
        assert_eq!(mount[0], FUSELAGE_WELL);
        assert!(rack.two_sided(mount));
        // A centreline tank stays.
        let tank = Hardpoint {
            store: Some("F150.GAS".into()),
            count: 1,
            ..f4e.clone()
        };
        assert_eq!(placed(&tank, None).0[0], 0.);
        // A flat underside 1 ft below the reference, 20 ft either side and
        // 20 ft fore and aft: every sample reads it.
        let flat = Shape {
            lines: Vec::new(),
            faces: vec![tore_formats::shape::Face {
                positions: vec![
                    [0., -60., -3.],
                    [60., -60., -3.],
                    [60., 60., -3.],
                    [0., 60., -3.],
                ],
                colors: vec![1; 4],
                fog: Default::default(),
                uv: Vec::new(),
                texture: String::new(),
                subtype: 0x20,
                normal: None,
                address: 0,
            }],
            billboards: Vec::new(),
            state_words: Default::default(),
        };
        let model = Underside::of(&flat, 1. / 3.);
        let (_, rack) = placed(&f4e, Some(&model));
        assert_eq!(rack.over, [Some(-1.); 5]);
        // A wingtip rail reads nothing.
        let tip = Hardpoint { location: 5, ..f4e };
        assert_eq!(placed(&tip, Some(&model)).1.over, [None; 5]);
    }
}
