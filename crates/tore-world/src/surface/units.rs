//! The surface's groupings and changing state. The resolution slice (W1)
//! fixes these types; the layout slice fills [`Battery`], [`SupplyTruck`]
//! and [`GroupTransform`], the parked-aircraft slice [`ParkedAircraft`], the
//! controller, movement and resupply slices add their fields to
//! [`SurfaceUnitState`]. See docs/spec/surface-defenses.md, "SAM batteries",
//! "Resupply" and "Parked aircraft".
use super::UnitId;
use tore_sim::combat::live::Side;

/// The SAM systems that fight as a battery around a search radar (John,
/// 2026-10-10: SAMs that need a separate radar in real life).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BatterySystem {
    Sa2 = 1,
    Sa3 = 2,
    Sa6 = 3,
    Hawk = 4,
}

impl BatterySystem {
    pub const ALL: [Self; 4] = [Self::Sa2, Self::Sa3, Self::Sa6, Self::Hawk];
    /// The launcher NT of the system.
    pub fn launcher(self) -> &'static str {
        match self {
            Self::Sa2 => "SA2A.NT",
            Self::Sa3 => "SA3.NT",
            Self::Sa6 => "SA6.NT",
            Self::Hawk => "HAWK.NT",
        }
    }
    /// The system a launcher type belongs to, if it fights as a battery.
    pub fn of_launcher(resource: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|system| system.launcher().eq_ignore_ascii_case(resource))
    }
    /// The LIB radar an existing element of this system is, for adoption:
    /// GCI (the Tall King, the only radar with a sensor) for SA-2 and SA-3,
    /// the Straight Flush for SA-6. HAWK has no LIB radar; the layout slice
    /// defines its element.
    pub fn radar(self) -> Option<&'static str> {
        match self {
            Self::Sa2 | Self::Sa3 => Some("GCI.NT"),
            Self::Sa6 => Some("SFLUSH.NT"),
            Self::Hawk => None,
        }
    }
    /// Most launchers in one battery (fitted, pending John: decision 12.8).
    pub fn cap(self) -> usize {
        match self {
            Self::Sa2 => 1,
            Self::Sa3 | Self::Sa6 => 4,
            Self::Hawk => 6,
        }
    }
}

/// One SAM battery: a radar element and its launchers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Battery {
    pub system: BatterySystem,
    pub side: Side,
    /// The radar element: an adopted existing radar or an added one.
    pub radar: UnitId,
    /// The radar was added (`0x5C00_0000` range), not adopted.
    pub radar_added: bool,
    /// Launchers, ascending id.
    pub launchers: Vec<UnitId>,
    /// The battery's own MISTRK, when it has one.
    pub truck: Option<UnitId>,
}

/// A supply truck (TRUCK or MISTRK): one already standing in the layout or
/// template, or one the layout rules added for a manned slot or a battery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupplyTruck {
    pub id: UnitId,
    /// The unit it was added for; `None` for a truck already in the data.
    pub serves: Option<UnitId>,
    /// Added by the layout rules (`0x5800_0000` range).
    pub added: bool,
}

/// One of the template's aircraft, parked on the ground: a simulated
/// aircraft with engines off that never takes off (John, 2026-10-10). The
/// resolution slice lists them with their ids; the parked-aircraft slice
/// puts them in the world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParkedAircraft {
    pub id: UnitId,
    /// The PT, by exact name: `SU35.PT`, `MIG21F.PT`.
    pub resource: String,
    pub position: [i32; 3],
    pub angles: [i32; 3],
    pub nationality: Option<i32>,
    pub side: Side,
    /// Flag 0x80: a destroy target. Retail writes the others as `$4017`.
    pub target: bool,
}

/// The rigid move of a relocated template: a whole-degree rotation about
/// `pivot` (the targets' centroid, x and z feet), then a translation. The
/// identity until the layout slice relocates a template.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroupTransform {
    /// Degrees, 0 to 359.
    pub rotation_deg: i32,
    /// Feet, x and z.
    pub translation: [i32; 2],
    /// Feet, x and z.
    pub pivot: [i32; 2],
}

impl GroupTransform {
    pub const IDENTITY: Self = Self {
        rotation_deg: 0,
        translation: [0, 0],
        pivot: [0, 0],
    };
    pub fn is_identity(&self) -> bool {
        self.rotation_deg == 0 && self.translation == [0, 0]
    }
}

/// Where the two sides start with a ground target (filled by the layout
/// slice; docs/spec/surface-defenses.md, "Start placement"). Red is placed
/// from Blue by the creator's separation rule, so it needs no point here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartPoints {
    /// The targets' placed centroid, x and z feet: the point the starts
    /// follow.
    pub target: [i32; 2],
    /// Blue's airborne start, x and z feet.
    pub blue: [i32; 2],
    /// Blue's heading at its start, whole degrees clockwise from north: at
    /// the target.
    pub blue_heading_deg: i32,
    /// The theater runways (object ids) a ground start may use, nearest the
    /// target first and at least 15 nm from it: Blue's side, then Red's.
    pub blue_airfields: Vec<u32>,
    pub red_airfields: Vec<u32>,
}

/// One unit's changing state. Its hit points stay in combat's target row
/// (`live::State`), which the checkpoint already codes; the controller and
/// resupply slices add their fields here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceUnitState {
    pub id: UnitId,
    /// A unit that follows a route: where it is and how it is moving. `None`
    /// until its first tick (and for every unit with no route), when it
    /// stands where the mission put it.
    pub mover: Option<super::movement::Mover>,
}

impl SurfaceUnitState {
    pub fn new(id: UnitId) -> Self {
        Self { id, mover: None }
    }
}

/// Every unit's changing state, in the surface's unit order, with the digest
/// of the surface it belongs to, so a checkpoint never restores over a
/// different one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SurfaceState {
    pub digest: u64,
    pub units: Vec<SurfaceUnitState>,
}

impl SurfaceState {
    /// Every unit back to its state at the mission's start, as a restart
    /// does.
    pub fn reset(&mut self) {
        for unit in &mut self.units {
            *unit = SurfaceUnitState::new(unit.id);
        }
    }
    pub fn unit(&self, id: UnitId) -> Option<&SurfaceUnitState> {
        self.units
            .binary_search_by_key(&id, |unit| unit.id)
            .ok()
            .map(|at| &self.units[at])
    }
    pub fn unit_mut(&mut self, id: UnitId) -> Option<&mut SurfaceUnitState> {
        self.units
            .binary_search_by_key(&id, |unit| unit.id)
            .ok()
            .map(|at| &mut self.units[at])
    }
}
