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
//! Every store's top hangs [`PYLON_DEPTH`] below the model's underside over
//! its pylon (John 2026-10-13: pylons about 1 ft deep, half of PY2's 2 ft),
//! not at the record's height, and leaves from exactly where it hangs
//! ([`Rack::store_point`]), so the simulation and the drawing place a store
//! at one point. Two stores on one pylon hang either side of a small bar at
//! its foot, alongside it as a wingtip missile sits on the tip
//! ([`slot_offset`]). The F-14's records hang at the real aircraft's
//! stations instead of where the records put them ([`station_move`]),
//! its AIM-9 on the glove pylon's shoulder rail ([`Location::Shoulder`]).
use super::store_sizes::StoreSize;
use crate::attitude::Vector;
use tore_formats::aircraft::{AircraftId, Hardpoint};
use tore_formats::shape::Shape;

/// Retail hardpoint type flag 0x4: the record is a left and right pair.
pub const PAIRED_FLAG: i32 = 0x4;

/// How far either side of a wing record's position its two pylons stand,
/// feet, when a wing carries two or more of its stores (John 2026-10-12:
/// four stores on one wing row hang from two pylons on each wing). 2.8 ft
/// apart clears a Maverick's 2.4 ft fins; fitted (agent decision).
pub const DUAL_PYLON_SPREAD: f64 = 1.4;

/// A wing pylon's depth, feet, from the model's underside over it to the
/// top of the stores it carries (opinionated, John 2026-10-13: "pylons need
/// to be about half the size they are now", from PY2's 2 ft).
pub const PYLON_DEPTH: f64 = 1.0;

/// How deep a fuselage or centreline station's launcher rail is, feet,
/// from the body's underside to the top of the stores: about 2 inches
/// (opinionated, John 2026-10-13: half of PY2's 4 inch rails).
pub const RAIL_DEPTH: f64 = 2. / 12.;

/// How far fore and aft of a wingtip record the tip's edge is read, feet.
pub const TIP_REACH: f64 = 2.5;

/// How deep the small bar at the foot of a pylon carrying two or more
/// stores side by side is, feet; their centre lines lie at its middle
/// (fitted, agent decision).
pub const RACK_DEPTH: f64 = 0.25;

/// How far a store hung beside a rack's bar keeps clear of the pylon,
/// feet, body to pylon side (fitted, agent decision).
pub const BAR_CLEAR: f64 = 0.1;

/// Half a pylon's thickness, feet: the drawn pylon is twice this
/// (`tore-app` `pylons`), and a store hung beside a bar clears it.
pub const PYLON_HALF_WIDTH: f64 = 0.14;

/// The clear gap between two fuselage stores one behind another, feet
/// (fitted, agent decision).
pub const TANDEM_GAP: f64 = 1.0;

/// The clear gap between the F-14's tunnel pallets one behind another,
/// feet: the rear store's nose just behind the front store's tail fins,
/// so the rear pair's tails reach less far past the tunnel's end (John
/// 2026-10-14: close up the pairs; fitted). An AIM-54 has no fins at its
/// nose, so nothing meets.
pub const PALLET_GAP: f64 = 0.25;

/// How far ahead and behind a fuselage record the model's underside is
/// sampled for its tandem pylons, feet; a tandem pylon's surface is
/// interpolated from these.
pub const TANDEM_SAMPLE: f64 = 7.0;

/// How far either side of the centreline a fuselage pair recorded on the
/// centreline hangs, feet: the F-4E's and F-4G's Sparrow wells, which
/// retail records at x 0 (fitted, agent decision).
pub const FUSELAGE_WELL: f64 = 3.0;

/// How far either side of a pylon the model is checked for a part lower
/// than the store's top (a nacelle or a body's flank beside it), feet: a
/// typical store's half width (fitted, agent decision).
pub const CLEARANCE: f64 = 0.6;

/// How far a part beside a store may reach below the store's top before
/// the store counts as cutting through it, feet (fitted, agent decision).
pub const CUT_TOLERANCE: f64 = 0.25;

/// The furthest a station is nudged sideways to clear a part beside it,
/// feet, in [`NUDGE_STEP`] steps (agent decision, 2026-10-13: John wants
/// short pylons, so a store moves aside rather than hanging deeper).
pub const MAX_NUDGE: f64 = 2.0;

/// How far ahead and behind a wing station with nothing of the model above
/// it the wing is looked for, feet ([`Underside::chord`]; fitted,
/// agent decision 2026-10-14).
pub const CHORD_REACH: f64 = 12.0;
pub const NUDGE_STEP: f64 = 0.25;

/// How far outboard of its pylon's centre line a store on that pylon's
/// shoulder rail has its centre line, feet: an AIM-9's fins (2.07 ft
/// across, at 45 degrees) clear the pylon and the fins of an AIM-120, AIM-7
/// or AIM-54 hung under the same pylon (fitted, agent decision 2026-10-14).
pub const SHOULDER_OUT: f64 = 1.25;

/// How far the body of a store on a shoulder rail clears the top of the
/// pylon's foot, feet: the rail juts off the pylon's side just above the
/// lugs of whatever store hangs under the pylon, so the shoulder store's
/// body is above that store's whole body, whatever it is (John 2026-10-14:
/// "the AIM-9 rail in real life is a jut off the main pylon slightly
/// elevated above the stores on the main wing pylon"; the gap is fitted).
pub const SHOULDER_GAP: f64 = 0.05;

/// How far above its pylon's foot a store of `size` on the shoulder rail
/// has its centre line, feet: its body's half depth and [`SHOULDER_GAP`].
pub fn shoulder_rise(size: StoreSize) -> f64 {
    size.top + SHOULDER_GAP
}

/// How far behind the wing's leading edge a wing station slid under the
/// chord sits, feet ([`Underside::chord`]): a launch rail about 4.3 ft long
/// (an AA-11's pylon) with its front at the leading edge, the missile's
/// nose ahead of it (fitted, agent decision 2026-10-14, John: "center the
/// rail to the leading edge"). A chord shorter than twice this puts it at
/// the chord's middle.
pub const BEHIND_LEADING_EDGE: f64 = 2.4;

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
    /// T.O.R.E's own, never a source record's: a store on the shoulder rail
    /// of another station's pylon, beside it ([`station_move`], the F-14's
    /// glove pylon Sidewinder).
    Shoulder,
    /// T.O.R.E's own, never a source record's: a fuselage station on the
    /// F-14's tunnel pallets, a fuselage station whose pairs one behind
    /// another close up to [`PALLET_GAP`] ([`station_move`]).
    Tunnel,
    /// A byte outside the six reviewed names.
    Other(u8),
}

/// [`Location::Shoulder`]'s byte, outside the source's 0 to 5.
pub const SHOULDER_BYTE: u8 = 0x80;
/// [`Location::Tunnel`]'s byte, outside the source's 0 to 5.
pub const TUNNEL_BYTE: u8 = 0x81;

impl Location {
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0 => Self::Centerline,
            1 => Self::Fuselage,
            2 => Self::InternalGun,
            3 => Self::InternalBay,
            4 => Self::Wing,
            5 => Self::Wingtip,
            SHOULDER_BYTE => Self::Shoulder,
            TUNNEL_BYTE => Self::Tunnel,
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
            Self::Shoulder => SHOULDER_BYTE,
            Self::Tunnel => TUNNEL_BYTE,
            Self::Other(byte) => byte,
        }
    }
    /// Stores here hang outside the aircraft, on a pylon or rail.
    pub fn external(self) -> bool {
        matches!(
            self,
            Self::Centerline
                | Self::Fuselage
                | Self::Tunnel
                | Self::Wing
                | Self::Wingtip
                | Self::Shoulder
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
    /// [`Self::depth`] below it; otherwise at the record's height.
    pub over: [Option<f32>; 5],
    /// A wingtip station's tip edge: [how far right (feet) the model's
    /// outermost edge beside the record is, the height of that edge's
    /// middle]. The store's centre line sits on it (John 2026-10-13: no
    /// rail below the tip). `None` for other stations, or when the model was
    /// not read.
    pub tip: Option<[f32; 2]>,
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
            tip: None,
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
        if matches!(
            self.location(),
            Location::Wing | Location::Fuselage | Location::Tunnel
        ) && (per_side == 2 || per_side >= 4)
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
        } else if self.location() == Location::Tunnel {
            [0., sign * (length + PALLET_GAP) / 2.]
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
    /// How deep the pylon or rail is from the surface over it to its
    /// stores' tops (lugs): [`RAIL_DEPTH`] at a fuselage or centreline
    /// station, [`PYLON_DEPTH`] on a wing (and the pylon a shoulder rail
    /// rides on).
    pub fn depth(self) -> f64 {
        match self.location() {
            Location::Fuselage | Location::Tunnel | Location::Centerline => RAIL_DEPTH,
            _ => PYLON_DEPTH,
        }
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
    /// surface hangs its stores' tops (lugs) [`Self::depth`] below it,
    /// those beside a rack's bar included; a wingtip rail or an unknown
    /// surface keeps the record's height. Internal stations have one place,
    /// the record's.
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
                if let (Location::Wingtip, Some([edge, middle])) = (self.location(), self.tip) {
                    // On the tip's edge, centred on it: no pylon or rail.
                    let edge = if mount[0].abs() > 1e-9 {
                        f64::from(edge) * mount[0].signum() * mirror
                    } else {
                        0.
                    };
                    return PylonPlace {
                        point: [edge, f64::from(middle), mount[2] + offset[1]],
                        slots,
                    };
                }
                let up = match self.surface(offset) {
                    // On a shoulder rail: its centre line a little above
                    // the foot of the pylon beside it.
                    Some(surface) if self.location() == Location::Shoulder => {
                        surface - self.depth() + shoulder_rise(size)
                    }
                    Some(surface)
                        if self.location().external() && self.location() != Location::Wingtip =>
                    {
                        surface - self.depth() - size.top
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
        let offset = slot_offset(k as u16, on, size);
        std::array::from_fn(|i| place.point[i] + offset[i])
    }
    /// How far the model's underside over store `slot`'s pylon is above
    /// the store's lugs, feet ([`Self::depth`], on a rack too; a store under
    /// a rack's bar is lower by the bar and its neighbour's half body), or
    /// `None` for a store on a wingtip rail, inside the aircraft or under no
    /// known surface. The battery's release scenario reports it.
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
        let (_, offset) = self.pylon(per_side, slot.min(per_side - 1), size.length);
        let surface = self.surface(offset)?;
        let point = self.store_point(mount, loaded, side, slot, size);
        Some(surface - (point[1] + size.top))
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

/// How far either side of its pylon a store of `size` hung beside a rack's
/// bar has its centre line, feet: its body clear of the pylon by
/// [`BAR_CLEAR`], and at least 0.36 of its width across the fins, so the
/// two stores' fins (set at 45 degrees) clear each other (fitted, agent
/// decision). The bar reaches out to it.
pub fn shoulder(size: StoreSize) -> f64 {
    (PYLON_HALF_WIDTH + BAR_CLEAR + size.top).max(0.36 * size.width)
}

/// Where the `k`th store of a pylon carrying `on` stores of `size` sits
/// against the pylon's point (where a lone store's origin is): feet right,
/// up and forward. One sits at the point. Two hang off the sides of a
/// small bar at the pylon's foot, one each side, their centre lines at the
/// bar's middle and [`shoulder`] out, as a wingtip missile sits on the tip
/// (John 2026-10-13: "the panel should be a small bar on the pylon and
/// ordnance hangs to the side"); their tops stay the pylon's depth under the
/// surface, so the bar's middle is at the point's height. Three as a triple
/// ejector rack: the two beside the bar and the third under it. More as such
/// racks ([`rack_group`]) one behind another, 1.05 lengths apart (fitted,
/// agent decision).
pub fn slot_offset(k: u16, on: u16, size: StoreSize) -> Vector {
    if on <= 1 {
        return [0.; 3];
    }
    let per = rack_group(on);
    let groups = on.div_ceil(per);
    let group = k / per;
    let forward = (f64::from(group) - f64::from(groups - 1) / 2.) * 1.05 * size.length;
    let out = shoulder(size);
    let [right, up] = match k % per {
        0 => [-out, 0.],
        1 => [out, 0.],
        _ => [0., -(RACK_DEPTH / 2. + size.top)],
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
        && rack.location() == Location::Wingtip
    {
        rack.tip = model
            .tip_edge(
                mount[0].abs(),
                mount[1],
                [mount[2] - TIP_REACH, mount[2] + TIP_REACH],
            )
            .map(|[edge, low, high]| [edge as f32, ((low + high) / 2.) as f32]);
    }
    if let Some(model) = model
        && rack.location().external()
        && rack.location() != Location::Wingtip
    {
        let depth = rack.depth();
        let from = mount[1];
        let mut right = mount[0].abs();
        // A wing station with nothing of the model above it, where the
        // wing is ahead of or behind it (the Yak-141's outer row, recorded
        // ahead of its swept wing), slides to under the wing's chord at its
        // distance out, its rail's front at the leading edge.
        if rack.location() == Location::Wing
            && right > 1e-9
            && model.height(right, mount[2], from, 6).is_none()
            && let Some([trailing, leading]) = model.chord(right, mount[2], from)
        {
            mount[2] = leading - BEHIND_LEADING_EDGE.min((leading - trailing) / 2.);
        }
        // A store beside a deeper part (a nacelle, a body's flank) moves
        // aside until it clears it; failing that it hangs under that part,
        // as PY2 hung every store.
        let mut lowest = false;
        if right > 1e-9 && model.cuts(right, mount[2], from, depth) {
            match model.nudge(right, mount[2], from, depth) {
                Some(nudge) => {
                    right += nudge;
                    mount[0] = right * mount[0].signum();
                }
                None => lowest = true,
            }
        }
        let surface = |r: f64, f: f64| {
            if lowest {
                model.lowest(r, f, from)
            } else {
                model.height(r, f, from, 6)
            }
        };
        rack.over = sample_points(right, mount[2]).map(|(r, f)| surface(r, f).map(|h| h as f32));
    }
    (mount, rack)
}

/// Where the model's underside is read for a pylon `right` feet out and
/// `forward` feet ahead, as [right, forward] in [`sample`] order.
fn sample_points(right: f64, forward: f64) -> [(f64, f64); 5] {
    [
        (right, forward),
        ((right - DUAL_PYLON_SPREAD).max(0.), forward),
        (right + DUAL_PYLON_SPREAD, forward),
        (right, forward + TANDEM_SAMPLE),
        (right, forward - TANDEM_SAMPLE),
    ]
}

/// How a moved station's stores hang ([`station_move`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hang {
    /// As the record's location hangs them, under the surface straight
    /// above the pylon.
    AsRecorded,
    /// Under the lowest surface over the store's length, `half` feet either
    /// side of the station, so a store under a part that slopes along it (an
    /// F-14 tank under its intake ramp) does not cut into it.
    Lowest { half: f64 },
    /// On the shoulder rail of the pylon [`SHOULDER_OUT`] inboard of the
    /// station ([`Location::Shoulder`]): its centre line [`shoulder_rise`]
    /// above that pylon's foot, the pylon's surface read where the pylon's
    /// own station is, `pylon_forward` feet forward, so the two share a
    /// foot.
    Shoulder { pylon_forward: f64 },
    /// As recorded, on tunnel pallets ([`Location::Tunnel`]): pairs one
    /// behind another close up to [`PALLET_GAP`].
    Pallets,
}

/// Where one aircraft type's source record hangs its stores in T.O.R.E,
/// where the real aircraft's stations are not where the record is: feet
/// right of the centreline (the right-hand side; a paired record mirrors
/// it) and forward of the reference point, and how the stores hang there.
/// The record's height, quantities and weapons are kept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StationMove {
    /// The source location the record must have; any other record keeps
    /// its own position.
    pub location: Location,
    pub right: f64,
    pub forward: f64,
    pub hang: Hang,
}

/// The F-14's glove pylon, feet out: just outboard of the intake nacelle
/// (the model's nacelle runs 3.5 to 6.5 ft out), under the glove.
const F14_GLOVE: f64 = 7.5;
/// Where the F-14's tunnel pairs are centred, feet forward: the front pair
/// 1.6 to 14.6 ft forward (where the tanks hung, its nose behind the
/// intakes), the rear pair -11.6 to 1.4 ft, its tails 0.6 ft past the
/// tunnel's end at about -11 ft.
const F14_PALLETS_FORWARD: f64 = 1.5;
/// Where the F-14's glove pylon's stores hang, feet forward: the AIM-120's
/// record (source 14 units), under the middle of the glove.
const F14_GLOVE_FORWARD: f64 = 14. / 3.;
/// Where the F-14's shoulder rail AIM-9 hangs, feet forward: its nose
/// level with the AIM-120's under the pylon (12 ft against 9.35 ft long),
/// which also keeps its tail fins clear of the glove's trailing edge.
const F14_SHOULDER_FORWARD: f64 = 6.0;

/// Source record `hardpoint`'s station on aircraft `id`, where T.O.R.E
/// moves it to the real aircraft's station (opinionated, John 2026-10-14:
/// "default tank pylons are actually under engine intake nacelles", "AIM-54s
/// grouped closer together and front missiles tucked backward where tanks
/// are", "AIM-120 default pylon tucked under wing flaring next to AIM-9";
/// the numbers are agent decisions read from the model). The F-14's real
/// stations: 1A/1B and 8A/8B the glove pylons (an AIM-9 on the shoulder
/// rail, an AIM-7, AIM-54 or AIM-120 under the pylon), 2 and 7 under the
/// intake nacelles (the tanks), 3 to 6 the tunnel pallets between the
/// nacelles (AIM-54s or AIM-7s, two ahead and two behind):
///
/// - record 4, the tunnel (AIM-54s): 1.25 ft either side of the centreline
///   (was 3.0, and 2.5 after PY3's move aside), on tunnel pallets whose
///   pairs one behind another close up to [`PALLET_GAP`] either side of
///   1.5 ft forward, so the front pair hangs about where the tanks hung and
///   the rear pair just behind it (front 1.6 to 14.6 ft, rear -11.6 to
///   1.4 ft forward);
/// - record 5, the tanks: under the nacelles, 4.75 ft out (was 3.0, in the
///   tunnel), at the record's 6.67 ft forward, their tops under the lowest
///   part of the nacelle along them;
/// - record 6, the glove pylon (AIM-120): 7.5 ft out (was 6.0, beside the
///   nacelle), at the record's 4.67 ft forward;
/// - record 7, the AIM-9: on that pylon's shoulder rail, [`SHOULDER_OUT`]
///   outboard of it (8.75 ft out, was 8.67), its nose level with the
///   AIM-120's (6.0 ft forward, was 1.67).
///
/// The Yak-141 (John 2026-10-14: "the wingtip is actually a wing pylon,
/// and both pylons are fully nestled under the wing"): its two wing rows
/// are its inner and outer underwing pylons, both well inside the swept
/// wing's chord, each rail's front at the leading edge, so the missiles'
/// noses reach a little ahead of it ([`YAK141_INNER`], [`YAK141_OUTER`]).
pub fn station_move(id: AircraftId, hardpoint: usize) -> Option<StationMove> {
    let at = |location, right, forward, hang| {
        Some(StationMove {
            location,
            right,
            forward,
            hang,
        })
    };
    match (id, hardpoint) {
        (AircraftId::F14, 4) => at(Location::Fuselage, 1.25, F14_PALLETS_FORWARD, Hang::Pallets),
        (AircraftId::F14, 5) => at(
            Location::Fuselage,
            4.75,
            20. / 3.,
            Hang::Lowest { half: 6.5 },
        ),
        (AircraftId::F14, 6) => at(
            Location::Wing,
            F14_GLOVE,
            F14_GLOVE_FORWARD,
            Hang::AsRecorded,
        ),
        (AircraftId::F14, 7) => at(
            Location::Wing,
            F14_GLOVE + SHOULDER_OUT,
            F14_SHOULDER_FORWARD,
            Hang::Shoulder {
                pylon_forward: F14_GLOVE_FORWARD,
            },
        ),
        // The Yak-141: two underwing pylons a side, both well inboard under
        // the wing's chord, their rails' fronts at the leading edge.
        (AircraftId::Yak141, 4) => at(
            Location::Wing,
            YAK141_INNER[0],
            YAK141_INNER[1],
            Hang::AsRecorded,
        ),
        (AircraftId::Yak141, 5) => at(
            Location::Wing,
            YAK141_OUTER[0],
            YAK141_OUTER[1],
            Hang::AsRecorded,
        ),
        _ => None,
    }
}

/// The Yak-141's inner underwing pylon, [feet out, feet forward] (agent
/// decision read from the model, John 2026-10-14: "both pylons are fully
/// nestled under the wing"): 7.5 ft out, 2.3 ft outboard of the body's
/// side, where the swept leading edge is about 0.8 ft forward and the
/// trailing edge 11 ft back; its AA-10 (14.8 ft) centred 2.8 ft back puts
/// its 6.7 ft rail's front 0.25 ft behind the leading edge and its nose
/// 4.6 ft ahead of it. Retail records the row 8.33 ft out at 0 ft forward.
const YAK141_INNER: [f64; 2] = [7.5, -2.8];
/// The Yak-141's outer underwing pylon (retail's row 16.67 ft out, which
/// is past two thirds of the model's 18.5 ft half span, by the tip): 11 ft
/// out, where the leading edge is about 2 ft back; its AA-11 (9.5 ft)
/// centred 4.4 ft back puts its 4.3 ft rail's front 0.25 ft behind the
/// leading edge, its nose 2.35 ft ahead of it and its tail 1.8 ft ahead
/// of the trailing edge.
const YAK141_OUTER: [f64; 2] = [11.0, -4.4];

/// [`placed`] for source record `hardpoint` of aircraft `id`: at the real
/// station where [`station_move`] moves it (no move aside there: the
/// station is placed by hand), otherwise where [`placed`] puts it.
pub fn placed_on(
    id: AircraftId,
    hardpoint: usize,
    h: &Hardpoint,
    model: Option<&Underside>,
) -> (Vector, Rack) {
    let Some(moved) =
        station_move(id, hardpoint).filter(|m| m.location == Location::from_byte(h.location))
    else {
        return placed(h, model);
    };
    let mut mount = h.position.map(|v| f64::from(v) / 3.);
    mount[0] = moved.right;
    mount[2] = moved.forward;
    let mut rack = Rack::from_source(h);
    let (pylon, forward) = match moved.hang {
        Hang::Shoulder { pylon_forward } => {
            rack.location = SHOULDER_BYTE;
            (moved.right - SHOULDER_OUT, pylon_forward)
        }
        Hang::Pallets => {
            rack.location = TUNNEL_BYTE;
            (moved.right, mount[2])
        }
        _ => (moved.right, mount[2]),
    };
    if let Some(model) = model {
        let from = mount[1];
        rack.over = sample_points(pylon, forward)
            .map(|(r, f)| model.height(r, f, from, 6).map(|h| h as f32));
        if let Hang::Lowest { half } = moved.hang {
            let steps = (half / 0.5).round() as i32;
            rack.over[sample::RECORD] = (-steps..=steps)
                .filter_map(|k| model.height(pylon, mount[2] + f64::from(k) * 0.5, from, 6))
                .reduce(f64::min)
                .map(|h| h as f32);
        }
    }
    (mount, rack)
}

/// The hardpoint records an aircraft's default load is built from: the
/// source records, except the A-4E's, whose default load is the real
/// aircraft's three tanks (John 2026-10-13: "A-4E: the real three tanks
/// (centreline + inner wings)"). Retail loads four 150 gallon tanks on the
/// inner wing row (5.7 ft out) and four Mk 82s on a one-sided row by the
/// fuselage, which [`placed`] hangs on the centreline. Here the inner row
/// holds two (one a wing, the most it can then take, as the real inner
/// pylons do) and the centreline row one tank in place of the Mk 82s, which
/// stay loadable there. About 1,000 lb less fuel than retail's four.
pub fn default_records(a: &tore_formats::aircraft::Aircraft) -> Vec<Hardpoint> {
    let mut records = a.hardpoints.clone();
    if a.id == tore_formats::aircraft::AircraftId::A4E {
        let tank = records
            .iter()
            .find(|h| h.store.as_deref().is_some_and(|s| s.ends_with(".GAS")))
            .and_then(|h| h.store.clone());
        for h in &mut records {
            let x = (f64::from(h.position[0]) / 3.).abs();
            match (h.location, h.store.as_deref()) {
                // The inner wing row's tanks: one a wing.
                (4, Some(s)) if s.ends_with(".GAS") => h.count = 2,
                // The one-sided row by the fuselage (the centreline station):
                // a tank instead of its Mk 82s.
                (4, Some(s)) if s.ends_with(".JT") && h.flags & PAIRED_FLAG == 0 && x < 3. => {
                    if let Some(tank) = &tank {
                        h.store = Some(tank.clone());
                        h.count = 1;
                    }
                }
                _ => {}
            }
        }
    }
    records
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
    /// The model's outermost point on the right at most 0.3 ft outboard of
    /// `right` and 4 ft inboard of it, between `forward[0]` and `forward[1]`
    /// and within 3 ft of `up`: [how far right, lowest, highest] of the model
    /// within 0.3 ft of that edge, feet; `None` when nothing is there.
    pub fn tip_edge(&self, right: f64, up: f64, forward: [f64; 2]) -> Option<[f64; 3]> {
        let mut points = Vec::new();
        for triangle in &self.triangles {
            for i in 0..3 {
                let [mut a, mut b] = [triangle[i], triangle[(i + 1) % 3]];
                if a[1] > b[1] {
                    std::mem::swap(&mut a, &mut b);
                }
                if b[1] < forward[0] || a[1] > forward[1] {
                    continue;
                }
                let at = |t: f64| -> [f64; 3] { std::array::from_fn(|k| a[k] + t * (b[k] - a[k])) };
                let span = b[1] - a[1];
                let [t0, t1] = if span < 1e-9 {
                    [0., 1.]
                } else {
                    [
                        ((forward[0] - a[1]) / span).max(0.),
                        ((forward[1] - a[1]) / span).min(1.),
                    ]
                };
                points.extend([at(t0), at(t1)]);
            }
        }
        let near =
            |q: &[f64; 3]| q[0] <= right + 0.3 && q[0] >= right - 4. && (q[2] - up).abs() <= 3.;
        let edge = points
            .iter()
            .filter(|q| near(q))
            .map(|q| q[0])
            .fold(f64::MIN, f64::max);
        if edge == f64::MIN {
            return None;
        }
        let (low, high) = points
            .iter()
            .filter(|q| near(q) && q[0] >= edge - 0.3)
            .fold((f64::MAX, f64::MIN), |(l, h), q| (l.min(q[2]), h.max(q[2])));
        Some([edge, low, high])
    }
    /// Whether a store hung `depth` under the surface over `right`,
    /// `forward` would cut through a part of the model beside it: some face
    /// within [`CLEARANCE`] either side reaches more than [`CUT_TOLERANCE`]
    /// below the store's top.
    pub fn cuts(&self, right: f64, forward: f64, from: f64, depth: f64) -> bool {
        let Some(surface) = self.height(right, forward, from, 6) else {
            return false;
        };
        let top = surface - depth;
        [-1., -0.5, 0.5, 1.].iter().any(|k| {
            let r = right + k * CLEARANCE;
            r >= 0.
                && self
                    .over(r, forward, from)
                    .is_some_and(|h| h < top - CUT_TOLERANCE)
        })
    }
    /// The smallest sideways move, feet (outboard positive, outboard first
    /// at equal distance), up to [`MAX_NUDGE`] that keeps a store at
    /// `right`, `forward` hung `depth` under the surface over it from
    /// cutting through the model ([`Self::cuts`]), or `None`.
    pub fn nudge(&self, right: f64, forward: f64, from: f64, depth: f64) -> Option<f64> {
        let steps = (MAX_NUDGE / NUDGE_STEP).round() as i32;
        (1..=steps)
            .flat_map(|n| [f64::from(n) * NUDGE_STEP, -f64::from(n) * NUDGE_STEP])
            .find(|d| {
                let r = right + d;
                r > CLEARANCE
                    && self.height(r, forward, from, 0).is_some()
                    && !self.cuts(r, forward, from, depth)
            })
    }
    /// The stretch of the model straight above `right` (within 12 ft above
    /// `from`) nearest `forward`, [its back, its front] feet forward,
    /// looked for up to [`CHORD_REACH`] ahead and behind in 0.25 ft steps;
    /// `None` when there is none.
    pub fn chord(&self, right: f64, forward: f64, from: f64) -> Option<[f64; 2]> {
        let steps = (CHORD_REACH / 0.25).round() as i32;
        let covered = |k: i32| {
            self.over(right, forward + f64::from(k) * 0.25, from)
                .is_some()
        };
        let near = (0..=steps).flat_map(|k| [k, -k]).find(|k| covered(*k))?;
        let toward = if near >= 0 { 1 } else { -1 };
        let mut far = near;
        while (far + toward).abs() <= steps && covered(far + toward) {
            far += toward;
        }
        let [back, front] = [near.min(far), near.max(far)].map(|k| forward + f64::from(k) * 0.25);
        Some([back, front])
    }
    /// The lowest underside across a store's body, [`CLEARANCE`] either
    /// side of `right`: where a store that cannot move aside hangs from.
    pub fn lowest(&self, right: f64, forward: f64, from: f64) -> Option<f64> {
        [-CLEARANCE, 0., CLEARANCE]
            .iter()
            .filter_map(|o| self.height((right + o).max(0.), forward, from, 6))
            .reduce(f64::min)
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
    fn a_store_hangs_a_foot_below_the_surface_over_its_pylon() {
        // The surface over the record is 1 ft up, 1.5 ft inboard and 0.5 ft
        // outboard of it (a wing's dihedral).
        let wing = Rack {
            over: [Some(1.), Some(1.5), Some(0.5), None, None],
            ..rack(4)
        };
        let mount = [10., -6., 0.];
        // One a side: the store's top (its lugs) 1 ft under the record's
        // surface, whatever the record's own height.
        let one = wing.store_point(mount, 2, 0, 0, SIZE);
        assert!((one[1] + SIZE.top - (1. - PYLON_DEPTH)).abs() < 1e-9);
        // Two a side: each under its own pylon's surface.
        let inboard = wing.store_point(mount, 4, 1, 0, SIZE);
        let outboard = wing.store_point(mount, 4, 1, 1, SIZE);
        assert!((inboard[1] + SIZE.top - (1.5 - PYLON_DEPTH)).abs() < 1e-9);
        assert!((outboard[1] + SIZE.top - (0.5 - PYLON_DEPTH)).abs() < 1e-9);
        assert!(inboard[0] < 0. && outboard[0] < inboard[0], "left side");
        // Three on one pylon: the two beside the bar keep their tops 1 ft
        // under the surface, either side of the pylon; the third hangs
        // under the bar.
        let three: Vec<Vector> = (0..3)
            .map(|k| wing.store_point(mount, 6, 0, k, SIZE))
            .collect();
        for side in &three[..2] {
            assert!((side[1] + SIZE.top - (1. - PYLON_DEPTH)).abs() < 1e-9);
        }
        assert!((three[0][0] - (10. - shoulder(SIZE))).abs() < 1e-9);
        assert!((three[1][0] - (10. + shoulder(SIZE))).abs() < 1e-9);
        assert!((three[2][1] - (three[0][1] - RACK_DEPTH / 2. - SIZE.top)).abs() < 1e-9);
        assert!((three[2][0] - 10.).abs() < 1e-9);
        // A wingtip rail, or no surface found, keeps the record's height.
        let tip = Rack {
            location: 5,
            ..wing
        };
        assert_eq!(tip.store_point(mount, 2, 0, 0, SIZE), mount);
        assert_eq!(rack(4).store_point(mount, 2, 0, 0, SIZE), mount);
        // A wingtip store's centre line sits on the tip's edge, either side.
        let on_tip = Rack {
            tip: Some([9.5, 0.25]),
            ..tip
        };
        assert_eq!(on_tip.store_point(mount, 2, 0, 0, SIZE), [9.5, 0.25, 0.]);
        assert_eq!(on_tip.store_point(mount, 2, 1, 0, SIZE), [-9.5, 0.25, 0.]);
        assert_eq!(on_tip.store_depth(mount, 2, 0, 0, SIZE), None);
        // A centreline or fuselage store hangs on a 2 inch rail.
        let belly = Rack {
            location: 0,
            paired: false,
            ..wing
        };
        let under = belly.store_point([0., -6., 0.], 1, 0, 0, SIZE);
        assert!((under[1] + SIZE.top - (1. - RAIL_DEPTH)).abs() < 1e-9);
        assert!((RAIL_DEPTH - 1. / 6.).abs() < 1e-12, "2 inches");
        assert!((PYLON_DEPTH - 1.).abs() < 1e-12, "a foot");
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
        // Each hangs on a 2 inch rail under the surface over it,
        // interpolated between the samples 7 ft ahead and behind.
        let surface = |forward_of_record: f64| -3. + forward_of_record / TANDEM_SAMPLE;
        for point in &points {
            let lugs = point[1] + SIZE.top;
            assert!((lugs - (surface(point[2] - 5.) - RAIL_DEPTH)).abs() < 1e-9);
        }
    }

    #[test]
    fn a_centreline_rack_of_four_is_two_pairs_one_behind_another() {
        let size = StoreSize {
            length: 8.,
            top: 0.45,
            width: 1.27,
        };
        let offsets: Vec<Vector> = (0..4).map(|k| slot_offset(k, 4, size)).collect();
        assert!(offsets[0][0] < 0. && offsets[1][0] > 0. && offsets[0][2] < 0.);
        assert!(offsets[2][0] < 0. && offsets[3][0] > 0. && offsets[2][2] > 0.);
        assert!(offsets.iter().all(|o| o[1] == 0.));
        // Three: a triple rack, the third under the bar.
        assert!((slot_offset(2, 3, size)[1] + RACK_DEPTH / 2. + size.top).abs() < 1e-9);
        // Six: two triple racks, one ahead of the other.
        assert!(slot_offset(0, 6, size)[2] < 0. && slot_offset(3, 6, size)[2] > 0.);
    }

    #[test]
    fn two_on_one_pylon_hang_beside_its_bar_like_a_wingtip_missile() {
        // A pair of AIM-9s: each body clears the pylon and the two sets of
        // fins (at 45 degrees) clear each other.
        let aim9 = StoreSize {
            length: 9.35,
            top: 0.21,
            width: 2.07,
        };
        let [left, right] = [0, 1].map(|k| slot_offset(k, 2, aim9));
        assert_eq!(left[0], -right[0]);
        assert_eq!(
            [left[1], right[1]],
            [0., 0.],
            "centre lines level with the bar"
        );
        assert!(right[0] - aim9.top >= PYLON_HALF_WIDTH + BAR_CLEAR - 1e-9);
        assert!(2. * right[0] >= 0.72 * aim9.width - 1e-9);
        // Mk 82s, fat for their fins: spaced by their bodies.
        let mk82 = StoreSize {
            length: 7.3,
            top: 0.45,
            width: 1.27,
        };
        assert!((shoulder(mk82) - (PYLON_HALF_WIDTH + BAR_CLEAR + 0.45)).abs() < 1e-9);
        // On a centreline rail their tops stay 2 inches under the belly.
        let belly = Rack {
            location: 0,
            paired: false,
            over: [Some(-2.); 5],
            tip: None,
        };
        for slot in 0..2 {
            let p = belly.store_point([0., -6., 0.], 2, 0, slot, mk82);
            assert!((p[1] + mk82.top - (-2. - RAIL_DEPTH)).abs() < 1e-9);
            let depth = belly.store_depth([0., -6., 0.], 2, 0, slot, mk82).unwrap();
            assert!((depth - RAIL_DEPTH).abs() < 1e-9);
        }
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
        // A wingtip missile recorded 0.67 ft past the 20 ft tip, level with
        // it, reads no underside: its centre line sits on the tip's edge.
        let tip = Hardpoint {
            location: 5,
            position: [62, -3, 0],
            ..f4e
        };
        let (_, tip) = placed(&tip, Some(&model));
        assert_eq!(tip.over, [None; 5]);
        assert_eq!(tip.tip, Some([20., -1.]), "a wingtip reads the tip's edge");
    }

    #[test]
    fn the_a4e_defaults_to_its_real_three_tanks() {
        let row = |flags, x, store: &str, count| Hardpoint {
            location: 4,
            flags,
            position: [x, -16, -6],
            store: Some(store.into()),
            count,
            weight_class: 32,
        };
        let aircraft = |id| tore_formats::aircraft::Aircraft {
            id,
            name: String::new(),
            shape: String::new(),
            fields: Default::default(),
            object: Default::default(),
            hardpoints: vec![
                row(0x1781, 8, "MK82.JT", 4),
                row(0x1785, 17, "F150.GAS", 4),
                row(0x1585, 26, "LAU61.JT", 2),
            ],
            envelopes: Vec::new(),
            sounds: Default::default(),
        };
        let a4e = default_records(&aircraft(tore_formats::aircraft::AircraftId::A4E));
        // A tank on the centreline row instead of the Mk 82s, one a wing on
        // the inner row, the rockets as retail.
        assert_eq!(a4e[0].store.as_deref(), Some("F150.GAS"));
        assert_eq!(a4e[0].count, 1);
        assert_eq!(a4e[1].count, 2);
        assert_eq!(a4e[2].store.as_deref(), Some("LAU61.JT"));
        assert_eq!(a4e[2].count, 2);
        // Every other aircraft keeps its records.
        let other = aircraft(tore_formats::aircraft::AircraftId::F4B);
        let kept = default_records(&other);
        assert!(
            kept.iter()
                .zip(&other.hardpoints)
                .all(|(a, b)| a.store == b.store && a.count == b.count)
        );
    }

    /// A model of a flat wing 1 ft below the reference out to 20 ft, with a
    /// nacelle hanging 3 ft below it from 3 to 5 ft out.
    fn nacelle_wing() -> Underside {
        let quad = |x0: f32, x1: f32, z: f32| tore_formats::shape::Face {
            positions: vec![
                [x0 * 3., -60., z * 3.],
                [x1 * 3., -60., z * 3.],
                [x1 * 3., 60., z * 3.],
                [x0 * 3., 60., z * 3.],
            ],
            colors: vec![1; 4],
            fog: Default::default(),
            uv: Vec::new(),
            texture: String::new(),
            subtype: 0x20,
            normal: None,
            address: 0,
        };
        let shape = Shape {
            lines: Vec::new(),
            faces: vec![quad(0., 20., -1.), quad(3., 5., -4.)],
            billboards: Vec::new(),
            state_words: Default::default(),
        };
        Underside::of(&shape, 1. / 3.)
    }

    #[test]
    fn a_store_beside_a_nacelle_moves_aside_rather_than_hanging_deeper() {
        let model = nacelle_wing();
        let wing = |x: i32| Hardpoint {
            location: 4,
            flags: 0x4,
            position: [x, -15, 0],
            store: Some("AGM65G.JT".into()),
            count: 2,
            weight_class: 0,
        };
        // 5.5 ft out, half a foot past the nacelle's edge: the store's
        // inboard side would cut into it, so the station moves out until
        // the nacelle is beyond its body, and hangs a foot under the wing.
        assert!(model.cuts(5.5, 0., -5., PYLON_DEPTH));
        let (mount, rack) = placed(&wing(16), Some(&model));
        assert!(mount[0] > 5.5 && mount[0] <= 5.5 + MAX_NUDGE, "{mount:?}");
        assert!(!model.cuts(mount[0], 0., -5., PYLON_DEPTH));
        assert_eq!(rack.over[sample::RECORD], Some(-1.));
        // Clear of it already: not moved.
        let (mount, _) = placed(&wing(30), Some(&model));
        assert_eq!(mount[0], 10.);
        // Under the nacelle itself: nothing cuts, it hangs under it.
        let (mount, rack) = placed(&wing(12), Some(&model));
        assert_eq!(mount[0], 4.);
        assert_eq!(rack.over[sample::RECORD], Some(-4.));
    }

    /// A model shaped like the F-14's underside, feet: the tunnel ceiling
    /// 2.7 ft down out to 3 ft, an intake nacelle from 3.5 to 6 ft out whose
    /// bottom ramps from 2.7 ft down at 13 ft forward to 4 ft down at 0 and
    /// behind, and the glove 0.6 ft up from 6.5 to 12 ft out.
    fn f14_like() -> Underside {
        let face = |positions: Vec<[f32; 3]>| tore_formats::shape::Face {
            positions: positions.into_iter().map(|p| p.map(|v| v * 3.)).collect(),
            colors: vec![1; 4],
            fog: Default::default(),
            uv: Vec::new(),
            texture: String::new(),
            subtype: 0x20,
            normal: None,
            address: 0,
        };
        let flat = |x0: f32, x1: f32, up: f32| {
            face(vec![
                [x0, -20., up],
                [x1, -20., up],
                [x1, 30., up],
                [x0, 30., up],
            ])
        };
        let shape = Shape {
            lines: Vec::new(),
            faces: vec![
                flat(0., 3., -2.7),
                flat(6.5, 12., 0.6),
                // The nacelle: flat behind, then the intake ramp.
                face(vec![
                    [3.5, -20., -4.],
                    [6., -20., -4.],
                    [6., 0., -4.],
                    [3.5, 0., -4.],
                ]),
                face(vec![
                    [3.5, 0., -4.],
                    [6., 0., -4.],
                    [6., 13., -2.7],
                    [3.5, 13., -2.7],
                ]),
            ],
            billboards: Vec::new(),
            state_words: Default::default(),
        };
        Underside::of(&shape, 1. / 3.)
    }

    /// The F-14's weapon and tank records, as F14.PT has them
    /// (docs/formats/aircraft-ordnance.md).
    fn f14_record(index: usize) -> Hardpoint {
        let (location, flags, position, store, count) = match index {
            4 => (1, 0x0155, [9, -23, 24], "AIM54C.JT", 4),
            5 => (1, 0x0605, [9, -23, 20], "F250.GAS", 2),
            6 => (4, 0x07f5, [18, -20, 14], "AIM120.JT", 2),
            _ => (4, 0x0485, [26, -16, 5], "AIM9M.JT", 2),
        };
        Hardpoint {
            location,
            flags,
            position,
            store: Some(store.into()),
            count,
            weight_class: 0,
        }
    }

    #[test]
    fn the_f14_hangs_its_stores_at_its_real_stations() {
        let model = f14_like();
        let f14 = |index| placed_on(AircraftId::F14, index, &f14_record(index), Some(&model));
        let phoenix = StoreSize::of_shape(Some("PHENX.SH"));
        let tank = StoreSize::body(super::super::store_sizes::tank_body(1_675.));
        // The tunnel: the Phoenixes 1.25 ft either side of the centreline,
        // close enough that their fins (3 ft across, at 45 degrees) just
        // clear, two ahead and two behind; the front pair hangs where the
        // tanks hung (6.67 ft forward), the rear pair behind it.
        let (mount, tunnel) = f14(4);
        assert_eq!([mount[0], mount[2]], [1.25, 1.5]);
        assert_eq!(tunnel.location(), Location::Tunnel);
        let points: Vec<Vector> = (0..4)
            .map(|slot: u16| tunnel.store_point(mount, 4, usize::from(slot % 2), slot / 2, phoenix))
            .collect();
        let front = points.iter().map(|p| p[2]).fold(f64::MIN, f64::max);
        let rear = points.iter().map(|p| p[2]).fold(f64::MAX, f64::min);
        assert!((front - 1.5 - (phoenix.length + PALLET_GAP) / 2.).abs() < 1e-9);
        assert!(
            (front - rear) - phoenix.length >= PALLET_GAP - 1e-9,
            "nose behind tail"
        );
        assert!(front - phoenix.length / 2. > 0. && (front - 20. / 3.).abs() < 1.5);
        // The rear pair's tails end about 0.6 ft past the tunnel (-11 ft).
        assert!((rear - phoenix.length / 2. + 11.6).abs() < 0.05);
        let fin = std::f64::consts::FRAC_1_SQRT_2 * phoenix.width / 2.;
        assert!(
            2. * 1.25 - 2. * fin > 0.25,
            "the pairs' fins clear each other"
        );
        for p in &points {
            assert!((p[1] + phoenix.top - (-2.7 - RAIL_DEPTH)).abs() < 1e-4);
        }
        // The tanks: under the nacelles, at the record's distance forward,
        // their tops under the lowest part of the intake ramp above them.
        let (mount, nacelle) = f14(5);
        assert_eq!([mount[0], mount[2]], [4.75, 20. / 3.]);
        for side in 0..2 {
            let p = nacelle.store_point(mount, 2, side, 0, tank);
            assert!((p[0].abs() - 4.75).abs() < 1e-9);
            // The ramp's lowest point read: 6.5 ft behind the record.
            let lowest = -4. + (20. / 3. - 6.5) * 1.3 / 13.;
            assert!(
                (p[1] + tank.top - (lowest - RAIL_DEPTH)).abs() < 1e-4,
                "{p:?}"
            );
        }
        // The glove pylon: 7.5 ft out, its AIM-120 1 ft under the glove.
        let amraam = StoreSize::of_shape(Some("AMRAM.SH"));
        let (mount, glove) = f14(6);
        assert_eq!([mount[0], mount[2]], [7.5, 14. / 3.]);
        assert_eq!(glove.location(), Location::Wing);
        let under = glove.store_point(mount, 2, 0, 0, amraam);
        assert!((under[1] + amraam.top - (0.6 - PYLON_DEPTH)).abs() < 1e-4);
        // The AIM-9 on that pylon's shoulder rail: outboard of it, a little
        // above its foot (the AIM-120's lugs), its nose level with the
        // AIM-120's.
        let aim9 = StoreSize::of_shape(Some("SIDEM.SH"));
        let (mount, rail) = f14(7);
        assert_eq!(rail.location(), Location::Shoulder);
        assert!(rail.two_sided(mount));
        let beside = rail.store_point(mount, 2, 1, 0, aim9);
        assert!((beside[0] + 7.5 + SHOULDER_OUT).abs() < 1e-9, "left side");
        assert!((beside[1] - (0.6 - PYLON_DEPTH + aim9.top + SHOULDER_GAP)).abs() < 1e-4);
        // Its body is above the lugs of whatever hangs under the pylon.
        assert!(beside[1] - aim9.top > under[1] + amraam.top);
        assert!(((beside[2] + aim9.length / 2.) - (under[2] + amraam.length / 2.)).abs() < 0.05);
        assert_eq!(rail.release_point(mount, 2, 1, 0, aim9), beside);
        // Every other aircraft, and an F-14 record not where F14.PT has it,
        // hang where the general rule puts them.
        let f18 = f14_record(6);
        assert_eq!(
            placed_on(AircraftId::F18, 6, &f18, Some(&model)),
            placed(&f18, Some(&model))
        );
        let elsewhere = Hardpoint {
            location: 0,
            ..f14_record(6)
        };
        assert_eq!(
            placed_on(AircraftId::F14, 6, &elsewhere, Some(&model)),
            placed(&elsewhere, Some(&model))
        );
    }

    /// A store on the shoulder rail clears the pylon, and its fins clear
    /// those of the missiles the F-14's glove pylon carries under it, all
    /// with fins at 45 degrees: no fin of one reaches across a fin of the
    /// other, wherever along their lengths the fins are.
    #[test]
    fn a_shoulder_rail_sidewinder_clears_the_missile_under_its_pylon() {
        let root2 = std::f64::consts::SQRT_2;
        for upper in ["SIDEM.SH", "SIDEB.SH", "AM9X.SH"] {
            let upper = StoreSize::of_shape(Some(upper));
            let reach = upper.width / 2.;
            // Its inner fins clear the pylon's side.
            assert!(SHOULDER_OUT - reach / root2 > PYLON_HALF_WIDTH, "{upper:?}");
            for lower in ["AMRAM.SH", "SPAR.SH", "PHENX.SH"] {
                let lower = StoreSize::of_shape(Some(lower));
                // The lower store's centre is its body's half depth under
                // the foot; the upper's shoulder_rise above it. The facing
                // fins (the upper's down and in, the lower's down and out)
                // meet only if both reach the crossing point.
                let drop = shoulder_rise(upper) + lower.top;
                let to_crossing_upper = (SHOULDER_OUT + drop) / root2;
                let to_crossing_lower = (SHOULDER_OUT - drop) / root2;
                assert!(
                    to_crossing_upper > reach || to_crossing_lower > lower.width / 2.,
                    "{upper:?} over {lower:?}"
                );
                // Bodies apart.
                assert!(SHOULDER_OUT.hypot(drop) > upper.top + lower.top);
            }
        }
    }

    #[test]
    fn a_wing_station_ahead_of_a_swept_wing_slides_under_its_chord() {
        // A wing 1 ft up swept back: its leading edge from 4 ft forward at
        // the root to 7 ft back at 18 ft out, its trailing edge from 6 ft
        // back to 11 ft back. At 16.67 ft out (the Yak-141's outer row,
        // recorded at 0 ft forward) the chord runs from about -6 to -10.6 ft.
        let wing = tore_formats::shape::Face {
            positions: vec![
                [0., 12., 3.],
                [54., -21., 3.],
                [54., -33., 3.],
                [0., -18., 3.],
            ],
            colors: vec![1; 4],
            fog: Default::default(),
            uv: Vec::new(),
            texture: String::new(),
            subtype: 0x20,
            normal: None,
            address: 0,
        };
        let shape = Shape {
            lines: Vec::new(),
            faces: vec![wing],
            billboards: Vec::new(),
            state_words: Default::default(),
        };
        let model = Underside::of(&shape, 1. / 3.);
        let outer = Hardpoint {
            location: 4,
            flags: 0x07e5,
            position: [50, -10, 0],
            store: Some("AA11.JT".into()),
            count: 2,
            weight_class: 0,
        };
        let (mount, rack) = placed(&outer, Some(&model));
        let lead = 4. - 11. * 16.67 / 18.;
        let trail = -6. - 5. * 16.67 / 18.;
        assert!((mount[2] - (lead + trail) / 2.).abs() < 0.2, "{mount:?}");
        assert_eq!(mount[0], 50. / 3.);
        assert!((rack.over[sample::RECORD].unwrap() - 1.).abs() < 1e-6);
        // Under the wing already: not moved.
        let inner = Hardpoint {
            position: [25, -10, 0],
            ..outer.clone()
        };
        assert_eq!(placed(&inner, Some(&model)).0[2], 0.);
        // The Yak-141's own rows hang at its two underwing pylons instead,
        // both under the swept wing, so nothing needs to slide.
        for (index, record, [right, forward]) in
            [(4, inner, YAK141_INNER), (5, outer, YAK141_OUTER)]
        {
            let (mount, rack) = placed_on(AircraftId::Yak141, index, &record, Some(&model));
            assert_eq!([mount[0], mount[2]], [right, forward]);
            assert_eq!(rack.location(), Location::Wing);
            assert!((rack.over[sample::RECORD].unwrap() - 1.).abs() < 1e-6);
            let lead = 4. - 11. * right / 18.;
            let trail = -6. - 5. * right / 18.;
            assert!(
                forward < lead && forward > trail,
                "under the chord at {right}"
            );
        }
        const { assert!(YAK141_OUTER[0] - YAK141_INNER[0] >= 3.) };
    }
}
