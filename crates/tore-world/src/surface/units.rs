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

/// One of the template's aircraft, parked on the ground or a carrier deck: a
/// simulated aircraft with engines off that never takes off (John,
/// 2026-10-10). Resolution lists them with their ids;
/// [`super::parked`] puts them in the world.
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
    /// The carrier whose deck it stands on, for a fleet template's aircraft.
    pub deck: Option<UnitId>,
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

/// One mount's stock: the rounds loaded (a gun's magazine, a launcher's
/// rails), the spare magazines behind them (`None`: unlimited, a ship's gun;
/// `Some(0)` for rails, which only a truck refills) and the running round
/// number of a gun (its damage share and tracer). The resupply slice refills
/// `loaded` and `reserve`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MountStock {
    pub loaded: u32,
    pub reserve: Option<u32>,
    pub ordinal: u64,
}

/// A visual gun's last look at its target, refreshed every 0.5 s.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    pub target: u32,
    pub tick: u64,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
}

/// One controller of a unit, with what a gun keeps between rounds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Engager {
    pub controller: tore_sim::ai::surface::Controller,
    /// The burst's aim error, radians right and up.
    pub aim_error: [f64; 2],
    /// A barrage burst that lost its random-fire roll: it spends its time
    /// without firing.
    pub holding: bool,
    pub seen: Option<Seen>,
}

/// A unit's radar: on while a hostile is in range and 30 s after
/// (docs/spec/surface-defenses.md, "RWR emitters and radar state").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RadarState {
    pub on: bool,
    pub last_hostile: Option<u64>,
    /// Off until this tick after a HARM shutdown.
    pub shutdown_until: Option<u64>,
}

/// A SAM battery's controller and whether it fights on its optical backup.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BatteryState {
    pub controller: tore_sim::ai::surface::Controller,
    pub optical: bool,
}

/// Where the two sides start with a ground target (filled by the layout
/// slice; docs/spec/surface-defenses.md, "Start placement"): Red within
/// 5 nm of the target, Blue the mission's separation from Red.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartPoints {
    /// The targets' placed centroid, x and z feet: the point the starts
    /// follow.
    pub target: [i32; 2],
    /// Red's start, x and z feet: the enemy group's placement point.
    pub red: [i32; 2],
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
/// (`live::State`), which the checkpoint already codes. A unit is armed from
/// its [`super::fire::Arms`] on the first surface tick (and again after a
/// restart clears it); the resupply slice adds its fields here.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceUnitState {
    pub id: UnitId,
    /// A unit that follows a route: where it is and how it is moving. `None`
    /// until its first tick (and for every unit with no route), when it
    /// stands where the mission put it.
    pub mover: Option<super::movement::Mover>,
    pub armed: bool,
    /// One per weapon of its arms, in order.
    pub engagers: Vec<Engager>,
    /// One per hardpoint.
    pub mounts: Vec<MountStock>,
    pub radar: RadarState,
    /// A live friendly supply truck is within reach this tick. The resupply
    /// slice sets it before the surface tick ([`super::supply::step`]); it
    /// lets an empty gun swap a magazine from the truck.
    pub supply: bool,
    /// The running rearm and magazine refill timers.
    pub resupply: super::supply::Resupply,
}

impl SurfaceUnitState {
    pub fn new(id: UnitId) -> Self {
        Self {
            id,
            mover: None,
            armed: false,
            engagers: Vec::new(),
            mounts: Vec::new(),
            radar: RadarState::default(),
            supply: false,
            resupply: super::supply::Resupply::default(),
        }
    }
    /// Its launchers' empty rails: hardpoints of a missile weapon below
    /// their full load (from [`super::fire::Arms::loads`]).
    pub fn empty_rails(&self, arms: &super::fire::Arms) -> Vec<usize> {
        arms.weapons
            .iter()
            .filter(|w| w.kind == super::fire::Kind::Missile)
            .flat_map(|w| w.mounts.iter().map(|m| m.index))
            .filter(|&i| {
                self.mounts
                    .get(i)
                    .zip(arms.loads.get(i))
                    .is_some_and(|(now, full)| now.loaded < full.loaded)
            })
            .collect()
    }
    /// Refills every missile rail to its full load (the resupply slice's
    /// rearm).
    pub fn refill_rails(&mut self, arms: &super::fire::Arms) {
        for index in self.empty_rails(arms) {
            self.mounts[index].loaded = arms.loads[index].loaded;
        }
    }
}

/// Every unit's changing state, in the surface's unit order, with the digest
/// of the surface it belongs to, so a checkpoint never restores over a
/// different one; the batteries' controllers; and the surface's random draws.
#[derive(Clone, Debug, Default)]
pub struct SurfaceState {
    pub digest: u64,
    pub units: Vec<SurfaceUnitState>,
    /// By index in `Surface::batteries`.
    pub batteries: Vec<BatteryState>,
    /// SplitMix64 state of aim errors, barrage rolls and HARM shutdowns.
    pub rng: u64,
    /// Anti-radiation missiles an emitter has already rolled against.
    pub harm_rolled: std::collections::BTreeSet<u32>,
    /// This tick's controller trace (`--surface-trace`). Not checkpointed.
    pub trace: Vec<super::fire::Trace>,
    /// This tick's missile locks, (aircraft, seeker class), for the RWR
    /// tone. Not checkpointed: rebuilt every tick.
    pub locks: Vec<(u32, u8)>,
    /// This tick's painting emitters, (aircraft, emitter id).
    pub painting: Vec<(u32, u32)>,
    /// Every armed unit's pose this tick. Not checkpointed: rebuilt every
    /// surface tick from the units' movers.
    pub places: std::collections::BTreeMap<UnitId, super::fire::Place>,
}

/// Equal when the checkpointed state is: the per-tick trace, locks,
/// painting and places are rebuilt by the next surface tick.
impl PartialEq for SurfaceState {
    fn eq(&self, other: &Self) -> bool {
        self.digest == other.digest
            && self.units == other.units
            && self.batteries == other.batteries
            && self.rng == other.rng
            && self.harm_rolled == other.harm_rolled
    }
}

impl SurfaceState {
    /// A fresh state for the surface with `digest` and `units`.
    pub fn new(digest: u64, units: Vec<SurfaceUnitState>) -> Self {
        Self {
            digest,
            units,
            rng: digest,
            ..Self::default()
        }
    }
    /// Every unit back to its state at the mission's start, as a restart
    /// does.
    pub fn reset(&mut self) {
        for unit in &mut self.units {
            *unit = SurfaceUnitState::new(unit.id);
        }
        self.batteries.clear();
        self.rng = self.digest;
        self.harm_rolled.clear();
        self.trace.clear();
        self.locks.clear();
        self.painting.clear();
        self.places.clear();
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
    /// Arms every unit not armed yet from `arsenal` and sizes the batteries.
    pub fn arm(&mut self, arsenal: &super::fire::Arsenal) {
        for arms in &arsenal.units {
            if let Some(unit) = self.unit_mut(arms.unit)
                && !unit.armed
            {
                unit.armed = true;
                unit.engagers = vec![Engager::default(); arms.weapons.len()];
                unit.mounts = arms.loads.clone();
            }
        }
        let batteries = arsenal
            .batteries
            .iter()
            .map(|b| b.index + 1)
            .max()
            .unwrap_or(0);
        if self.batteries.len() < batteries {
            self.batteries.resize(batteries, BatteryState::default());
        }
    }
    /// Missiles loaded on `weapon`'s rails of `arms`.
    pub fn rails(&self, arms: &super::fire::Arms, weapon: &super::fire::WeaponArms) -> u32 {
        let Some(unit) = self.unit(arms.unit) else {
            return 0;
        };
        weapon
            .mounts
            .iter()
            .filter_map(|m| unit.mounts.get(m.index))
            .map(|stock| stock.loaded)
            .sum()
    }
    /// The first loaded rail of `weapon` on `arms` whose heading arc covers
    /// `target`.
    pub fn rail_for(
        &self,
        arms: &super::fire::Arms,
        weapon: &super::fire::WeaponArms,
        target: [f64; 3],
    ) -> Option<usize> {
        let unit = self.unit(arms.unit)?;
        let place = self.place(arms);
        let muzzle = place.eye;
        let bearing = (target[0] - muzzle[0]).atan2(target[2] - muzzle[2]);
        weapon
            .mounts
            .iter()
            .filter(|m| unit.mounts.get(m.index).is_some_and(|s| s.loaded > 0))
            .find(|m| super::fire::covers(m, (bearing - place.heading).to_degrees()))
            .map(|m| m.index)
    }
    /// Where `arms`' unit stands this tick ([`super::fire::Arms::place`]).
    pub fn place(&self, arms: &super::fire::Arms) -> super::fire::Place {
        self.places
            .get(&arms.unit)
            .copied()
            .unwrap_or_else(|| arms.place(self.unit(arms.unit)))
    }
    /// The seeker classes (2 infrared, 3 radar) of the surface missile locks
    /// held on aircraft `plane` this tick, for the RWR tone
    /// (docs/spec/rwr.md#warning-tones).
    pub fn locks_on(&self, plane: u32) -> Vec<u8> {
        self.locks
            .iter()
            .filter(|(target, class)| *target == plane && matches!(class, 2 | 3))
            .map(|(_, class)| *class)
            .collect()
    }
    /// The surface emitters painting aircraft `plane` this tick: radars whose
    /// controller tracks or fires at it.
    pub fn painting(&self, plane: u32) -> Vec<u32> {
        self.painting
            .iter()
            .filter(|(target, _)| *target == plane)
            .map(|(_, emitter)| *emitter)
            .collect()
    }
}
