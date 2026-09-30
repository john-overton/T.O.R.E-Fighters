//! The entities a snapshot carries, their quantized states, and each entity's
//! record: in full, or against a baseline the client has acknowledged.
//!
//! See net-protocol.md, "Entities" and "Quantization". A state here is
//! already quantized: whole steps of the wire's units. The host keeps exactly
//! these numbers as baselines, so both ends hold the same baseline and
//! rounding never builds up.
//!
//! **Parsing never needs the baseline.** Every field of a record against a
//! baseline is a self-delimiting difference or an absolute value, so a
//! record whose baseline the client lacks is read past and dropped, and the
//! rest of the packet is still used (agent decision). Only
//! [`Delta::apply`] reads the baseline.

use super::bits::{self, STEP_LIMIT, angle_diff16, div_round, in_range, read_i32, read_u32};
use super::names::NameIndex;
use super::{WireError, WireResult, limits};
use tore_codec::{BitReader, BitWriter, CodecError};
use tore_formats::aircraft::AircraftId;
use tore_sim::combat::live::{DAMAGE_SECTIONS, DamageSection};
use tore_sim::{ejection, wreck};

/// Positions: 1/32 ft.
pub const POSITION_STEP: f64 = 1. / 32.;
/// Velocities: 1/64 ft/s.
pub const VELOCITY_STEP: f64 = 1. / 64.;
/// Aircraft speed: 1/4 ft/s.
pub const SPEED_STEP: f64 = 0.25;
/// Thrust-vectoring rates: 1/4096 rad/s.
pub const RATE_STEP: f64 = 1. / 4096.;
/// Simulation ticks a second, which the prediction divides by.
pub const TICKS_PER_SECOND: i128 = 120;

/// An angle of the wire (2^-16 of a turn) in radians, from -pi up to but not
/// including pi.
pub fn radians(angle: u16) -> f64 {
    bits::radians16(angle)
}

/// What kind of thing an entity is. Records are written kind by kind in this
/// order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    /// Every plane except the player's own, human-flown or not.
    Aircraft,
    /// A missile, bomb or rocket in flight (gun rounds are burst events).
    Projectile,
    /// A piece of a destroyed aircraft.
    Debris,
    /// An ejected pilot.
    Pilot,
}

impl EntityKind {
    /// Every kind, in record order.
    pub const ALL: [Self; 4] = [Self::Aircraft, Self::Projectile, Self::Debris, Self::Pilot];

    /// The kind's 2-bit code.
    pub fn code(self) -> u8 {
        match self {
            Self::Aircraft => 0,
            Self::Projectile => 1,
            Self::Debris => 2,
            Self::Pilot => 3,
        }
    }

    /// The kind of a 2-bit code (only the low two bits are read).
    pub fn from_code(code: u8) -> Self {
        Self::ALL[usize::from(code & 3)]
    }

    /// The most records of this kind one snapshot carries.
    pub fn limit(self) -> usize {
        match self {
            Self::Aircraft => limits::AIRCRAFT,
            Self::Projectile => limits::PROJECTILES,
            Self::Debris => limits::DEBRIS,
            Self::Pilot => limits::PILOTS,
        }
    }

    fn what(self) -> &'static str {
        match self {
            Self::Aircraft => "aircraft records",
            Self::Projectile => "projectile records",
            Self::Debris => "debris records",
            Self::Pilot => "pilot records",
        }
    }
}

/// One entity: its kind and its id. An aircraft's id is its plane id, a
/// projectile's its number, and a debris piece's or an ejected pilot's the
/// aircraft it came from (each aircraft releases at most one piece and one
/// pilot; agent decision).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityKey {
    pub kind: EntityKind,
    pub id: u32,
}

/// Where a thing is and how it moves, in the wire's steps: position in 1/32
/// ft, ground-relative velocity in 1/64 ft/s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Motion {
    pub position: [i64; 3],
    pub velocity: [i64; 3],
}

impl Motion {
    /// From feet and feet per second.
    pub fn of(position: [f64; 3], velocity: [f64; 3]) -> Self {
        Self {
            position: position.map(|v| bits::steps(v, POSITION_STEP)),
            velocity: velocity.map(|v| bits::steps(v, VELOCITY_STEP)),
        }
    }

    /// The position in feet.
    pub fn position_ft(&self) -> [f64; 3] {
        self.position.map(|q| bits::value(q, POSITION_STEP))
    }

    /// The velocity in feet per second.
    pub fn velocity_fps(&self) -> [f64; 3] {
        self.velocity.map(|q| bits::value(q, VELOCITY_STEP))
    }

    /// The position `ticks` later along the velocity, in steps, with integer
    /// arithmetic only so both ends agree exactly.
    pub fn predicted(&self, ticks: u32) -> [i128; 3] {
        // 1/64 ft/s for ticks of 1/120 s is (v * ticks / 240) steps of 1/32 ft.
        std::array::from_fn(|i| {
            i128::from(self.position[i])
                + div_round(
                    i128::from(self.velocity[i]) * i128::from(ticks),
                    TICKS_PER_SECOND * 2,
                )
        })
    }
}

/// An aircraft's animated devices: gear, flaps, brake, hook, bay and exhaust
/// at 1/255; elevator, aileron and rudder at 1/127; speed at 1/4 ft/s; the
/// throttle at 1/255.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Devices {
    pub levels: [u8; 6],
    pub surfaces: [i8; 3],
    pub speed: i32,
    pub throttle: u8,
}

/// Nozzle and flame inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EngineState {
    pub lit: bool,
    pub afterburner: bool,
    pub flame: bool,
    /// Angular rates the X-31 paddles and plume follow, 1/4096 rad/s.
    pub rates: [i32; 3],
}

/// Damage as whole numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DamageState {
    pub hp: i32,
    pub initial_hp: i32,
    pub sections: [i32; DAMAGE_SECTIONS],
    pub structural: Option<DamageSection>,
}

/// Airborne, crashed and the wreck's phase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub airborne: bool,
    pub crashed: bool,
    pub wreck: Option<wreck::Phase>,
}

/// An aircraft other than the player's own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AircraftState {
    /// Sent in full records only: the client keeps it from the baseline.
    pub aircraft: Option<AircraftId>,
    pub motion: Motion,
    /// Yaw, pitch and bank, 2^-16 of a turn.
    pub attitude: [u16; 3],
    pub devices: Option<Devices>,
    pub engine: EngineState,
    pub damage: DamageState,
    pub status: Status,
}

/// A missile, bomb or rocket in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectileState {
    /// Who fired it; sent in full records only, as are the fields to
    /// `aimed_at_player`.
    pub owner: u32,
    pub weapon: NameIndex,
    pub shape: Option<NameIndex>,
    pub target: Option<u32>,
    /// Launched at this connection's player.
    pub aimed_at_player: bool,
    pub motion: Motion,
    /// Azimuth (from +z towards +x) and elevation, 2^-16 of a turn: how it
    /// is drawn.
    pub direction: [u16; 2],
}

/// A piece of a destroyed aircraft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DebrisState {
    /// The aircraft it came from; full records only, as are `model` and
    /// `variant`.
    pub owner: u32,
    /// The owner's aircraft, whose broken model draws it.
    pub model: Option<AircraftId>,
    pub variant: Option<u8>,
    pub motion: Motion,
    pub attitude: [u16; 3],
}

/// An ejected pilot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PilotState {
    /// The aircraft it left; full records only.
    pub owner: u32,
    pub motion: Motion,
    pub heading: u16,
    pub phase: ejection::Phase,
}

/// Any entity's quantized state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityState {
    Aircraft(AircraftState),
    Projectile(ProjectileState),
    Debris(DebrisState),
    Pilot(PilotState),
}

/// An entity with its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entity {
    pub id: u32,
    pub state: EntityState,
}

impl Entity {
    /// The entity's key.
    pub fn key(&self) -> EntityKey {
        EntityKey {
            kind: self.state.kind(),
            id: self.id,
        }
    }
}

const PHASES: [ejection::Phase; 6] = [
    ejection::Phase::Seat,
    ejection::Phase::Freefall,
    ejection::Phase::Inflating,
    ejection::Phase::Parachute,
    ejection::Phase::Landed,
    ejection::Phase::Impact,
];
const WRECKS: [wreck::Phase; 3] = [
    wreck::Phase::Falling,
    wreck::Phase::Grounded,
    wreck::Phase::Exploded,
];
const SECTIONS: [DamageSection; DAMAGE_SECTIONS] = [
    DamageSection::Nose,
    DamageSection::Cockpit,
    DamageSection::Core,
    DamageSection::LeftWing,
    DamageSection::RightWing,
    DamageSection::Tail,
];

fn phase_code(phase: ejection::Phase) -> i64 {
    match phase {
        ejection::Phase::Seat => 0,
        ejection::Phase::Freefall => 1,
        ejection::Phase::Inflating => 2,
        ejection::Phase::Parachute => 3,
        ejection::Phase::Landed => 4,
        ejection::Phase::Impact => 5,
    }
}

fn wreck_code(phase: Option<wreck::Phase>) -> i64 {
    match phase {
        None => 0,
        Some(wreck::Phase::Falling) => 1,
        Some(wreck::Phase::Grounded) => 2,
        Some(wreck::Phase::Exploded) => 3,
    }
}

fn section_code(section: Option<DamageSection>) -> i64 {
    section.map_or(0, |s| s as i64 + 1)
}

/// An aircraft identity's 4-bit code: its place among the selectable ones.
fn aircraft_code(id: AircraftId) -> u64 {
    AircraftId::SELECTABLE
        .iter()
        .position(|a| *a == id)
        .unwrap_or(0) as u64
}

fn write_aircraft_id(w: &mut BitWriter, id: Option<AircraftId>) {
    bits::write_option(w, id, |w, id| {
        let _ = w.write_bits(aircraft_code(id), 4);
    });
}

fn read_aircraft_id(r: &mut BitReader<'_>) -> WireResult<Option<AircraftId>> {
    bits::read_option(r, |r| {
        AircraftId::SELECTABLE
            .get(r.read_bits(4)? as usize)
            .copied()
            .ok_or(WireError::Invalid("aircraft"))
    })
}

/// How a slow field is coded.
#[derive(Clone, Copy)]
enum Field {
    /// Unsigned in this many bits, at most this value.
    Unsigned(u32, i64),
    /// Two's complement in this many bits, at least this value.
    Signed(u32, i64),
    /// A signed varint that fits 32 bits.
    Int32,
}

impl Field {
    fn write(self, w: &mut BitWriter, value: i64) {
        match self {
            Self::Unsigned(width, _) => {
                let _ = w.write_bits(value as u64, width);
            }
            Self::Signed(width, _) => {
                let _ = w.write_signed(value, width);
            }
            Self::Int32 => w.write_varint_signed(value),
        }
    }

    fn read(self, r: &mut BitReader<'_>) -> WireResult<i64> {
        Ok(match self {
            Self::Unsigned(width, max) => {
                let value = r.read_bits(width)? as i64;
                if value > max {
                    return Err(WireError::Invalid("entity field"));
                }
                value
            }
            Self::Signed(width, min) => {
                let value = r.read_signed(width)?;
                if value < min {
                    return Err(WireError::Invalid("entity field"));
                }
                value
            }
            Self::Int32 => i64::from(read_i32(r)?),
        })
    }
}

const BIT: Field = Field::Unsigned(1, 1);
const LEVEL: Field = Field::Unsigned(8, 255);
const SURFACE: Field = Field::Signed(8, -127);

const DEVICES: &[Field] = &[
    BIT, LEVEL, LEVEL, LEVEL, LEVEL, LEVEL, LEVEL, SURFACE, SURFACE, SURFACE, LEVEL,
];
const ENGINE: &[Field] = &[BIT, BIT, BIT, Field::Int32, Field::Int32, Field::Int32];
const DAMAGE: &[Field] = &[
    Field::Int32,
    Field::Int32,
    Field::Int32,
    Field::Int32,
    Field::Int32,
    Field::Int32,
    Field::Int32,
    Field::Int32,
    Field::Unsigned(3, DAMAGE_SECTIONS as i64),
];
const STATUS: &[Field] = &[BIT, BIT, Field::Unsigned(2, 3)];
const PILOT: &[Field] = &[Field::Unsigned(3, 5)];

/// A group of slow fields, sent only when one of them changed.
struct Group {
    fields: &'static [Field],
    /// The first field says whether the rest exist: when it is 0 the rest
    /// are zero and not sent (an aircraft's devices).
    gated: bool,
}

const fn group(fields: &'static [Field]) -> Group {
    Group {
        fields,
        gated: false,
    }
}

const AIRCRAFT_GROUPS: &[Group] = &[
    Group {
        fields: DEVICES,
        gated: true,
    },
    group(ENGINE),
    group(DAMAGE),
    group(STATUS),
];
const PILOT_GROUPS: &[Group] = &[group(PILOT)];

/// The layout of a kind's changing fields.
struct Schema {
    angles: usize,
    fast: usize,
    groups: &'static [Group],
}

fn schema(kind: EntityKind) -> Schema {
    match kind {
        EntityKind::Aircraft => Schema {
            angles: 3,
            fast: 1,
            groups: AIRCRAFT_GROUPS,
        },
        EntityKind::Projectile => Schema {
            angles: 2,
            fast: 0,
            groups: &[],
        },
        EntityKind::Debris => Schema {
            angles: 3,
            fast: 0,
            groups: &[],
        },
        EntityKind::Pilot => Schema {
            angles: 1,
            fast: 0,
            groups: PILOT_GROUPS,
        },
    }
}

/// The fields a record sends only in full: what the entity is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Identity {
    Aircraft(Option<AircraftId>),
    Projectile {
        owner: u32,
        weapon: NameIndex,
        shape: Option<NameIndex>,
        target: Option<u32>,
        aimed_at_player: bool,
    },
    Debris {
        owner: u32,
        model: Option<AircraftId>,
        variant: Option<u8>,
    },
    Pilot {
        owner: u32,
    },
}

impl Identity {
    fn write(&self, w: &mut BitWriter) {
        match *self {
            Self::Aircraft(id) => write_aircraft_id(w, id),
            Self::Projectile {
                owner,
                weapon,
                shape,
                target,
                aimed_at_player,
            } => {
                w.write_varint(u64::from(owner));
                weapon.write(w);
                bits::write_option(w, shape, |w, shape| shape.write(w));
                bits::write_option(w, target, |w, target| w.write_varint(u64::from(target)));
                w.write_bool(aimed_at_player);
            }
            Self::Debris {
                owner,
                model,
                variant,
            } => {
                w.write_varint(u64::from(owner));
                write_aircraft_id(w, model);
                bits::write_option(w, variant, |w, variant| {
                    let _ = w.write_bits(u64::from(variant), 3);
                });
            }
            Self::Pilot { owner } => w.write_varint(u64::from(owner)),
        }
    }

    fn read(r: &mut BitReader<'_>, kind: EntityKind) -> WireResult<Self> {
        Ok(match kind {
            EntityKind::Aircraft => Self::Aircraft(read_aircraft_id(r)?),
            EntityKind::Projectile => Self::Projectile {
                owner: read_u32(r)?,
                weapon: NameIndex::read(r)?,
                shape: bits::read_option(r, NameIndex::read)?,
                target: bits::read_option(r, read_u32)?,
                aimed_at_player: r.read_bool()?,
            },
            EntityKind::Debris => Self::Debris {
                owner: read_u32(r)?,
                model: read_aircraft_id(r)?,
                variant: bits::read_option(r, |r| Ok(r.read_bits(3)? as u8))?,
            },
            EntityKind::Pilot => Self::Pilot {
                owner: read_u32(r)?,
            },
        })
    }
}

/// A state as numbers in its kind's layout.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Flat {
    motion: Motion,
    angles: [u16; 3],
    fast: [i64; 1],
    groups: Vec<Vec<i64>>,
}

fn flag(value: bool) -> i64 {
    i64::from(value)
}

impl EntityState {
    /// The state's kind.
    pub fn kind(&self) -> EntityKind {
        match self {
            Self::Aircraft(_) => EntityKind::Aircraft,
            Self::Projectile(_) => EntityKind::Projectile,
            Self::Debris(_) => EntityKind::Debris,
            Self::Pilot(_) => EntityKind::Pilot,
        }
    }

    /// Where it is and how it moves.
    pub fn motion(&self) -> &Motion {
        match self {
            Self::Aircraft(s) => &s.motion,
            Self::Projectile(s) => &s.motion,
            Self::Debris(s) => &s.motion,
            Self::Pilot(s) => &s.motion,
        }
    }

    fn identity(&self) -> Identity {
        match *self {
            Self::Aircraft(s) => Identity::Aircraft(s.aircraft),
            Self::Projectile(s) => Identity::Projectile {
                owner: s.owner,
                weapon: s.weapon,
                shape: s.shape,
                target: s.target,
                aimed_at_player: s.aimed_at_player,
            },
            Self::Debris(s) => Identity::Debris {
                owner: s.owner,
                model: s.model,
                variant: s.variant,
            },
            Self::Pilot(s) => Identity::Pilot { owner: s.owner },
        }
    }

    /// True when `other` is the same thing: its full-record fields match, so
    /// a record may be coded against it.
    pub fn same_identity(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }

    fn flat(&self) -> Flat {
        match *self {
            Self::Aircraft(s) => {
                let d = s.devices.unwrap_or_default();
                let mut devices = vec![flag(s.devices.is_some())];
                devices.extend(d.levels.iter().map(|&v| i64::from(v)));
                devices.extend(d.surfaces.iter().map(|&v| i64::from(v)));
                devices.push(i64::from(d.throttle));
                let e = s.engine;
                let engine = vec![
                    flag(e.lit),
                    flag(e.afterburner),
                    flag(e.flame),
                    i64::from(e.rates[0]),
                    i64::from(e.rates[1]),
                    i64::from(e.rates[2]),
                ];
                let m = s.damage;
                let mut damage = vec![i64::from(m.hp), i64::from(m.initial_hp)];
                damage.extend(m.sections.iter().map(|&v| i64::from(v)));
                damage.push(section_code(m.structural));
                let status = vec![
                    flag(s.status.airborne),
                    flag(s.status.crashed),
                    wreck_code(s.status.wreck),
                ];
                Flat {
                    motion: s.motion,
                    angles: s.attitude,
                    fast: [i64::from(d.speed)],
                    groups: vec![devices, engine, damage, status],
                }
            }
            Self::Projectile(s) => Flat {
                motion: s.motion,
                angles: [s.direction[0], s.direction[1], 0],
                fast: [0],
                groups: Vec::new(),
            },
            Self::Debris(s) => Flat {
                motion: s.motion,
                angles: s.attitude,
                fast: [0],
                groups: Vec::new(),
            },
            Self::Pilot(s) => Flat {
                motion: s.motion,
                angles: [s.heading, 0, 0],
                fast: [0],
                groups: vec![vec![phase_code(s.phase)]],
            },
        }
    }

    /// Builds a state from its identity and numbers, which the reader has
    /// checked field by field.
    fn from_flat(identity: Identity, flat: &Flat) -> WireResult<Self> {
        let g = |group: usize, field: usize| flat.groups[group][field];
        Ok(match identity {
            Identity::Aircraft(aircraft) => {
                let devices = (g(0, 0) == 1).then(|| Devices {
                    levels: std::array::from_fn(|i| g(0, 1 + i) as u8),
                    surfaces: std::array::from_fn(|i| g(0, 7 + i) as i8),
                    speed: flat.fast[0] as i32,
                    throttle: g(0, 10) as u8,
                });
                if devices.is_none()
                    && (flat.groups[0][1..].iter().any(|&v| v != 0) || flat.fast[0] != 0)
                {
                    return Err(WireError::Invalid("devices of an aircraft without them"));
                }
                Self::Aircraft(AircraftState {
                    aircraft,
                    motion: flat.motion,
                    attitude: flat.angles,
                    devices,
                    engine: EngineState {
                        lit: g(1, 0) == 1,
                        afterburner: g(1, 1) == 1,
                        flame: g(1, 2) == 1,
                        rates: std::array::from_fn(|i| g(1, 3 + i) as i32),
                    },
                    damage: DamageState {
                        hp: g(2, 0) as i32,
                        initial_hp: g(2, 1) as i32,
                        sections: std::array::from_fn(|i| g(2, 2 + i) as i32),
                        structural: match g(2, 8) {
                            0 => None,
                            code => Some(SECTIONS[code as usize - 1]),
                        },
                    },
                    status: Status {
                        airborne: g(3, 0) == 1,
                        crashed: g(3, 1) == 1,
                        wreck: match g(3, 2) {
                            0 => None,
                            code => Some(WRECKS[code as usize - 1]),
                        },
                    },
                })
            }
            Identity::Projectile {
                owner,
                weapon,
                shape,
                target,
                aimed_at_player,
            } => Self::Projectile(ProjectileState {
                owner,
                weapon,
                shape,
                target,
                aimed_at_player,
                motion: flat.motion,
                direction: [flat.angles[0], flat.angles[1]],
            }),
            Identity::Debris {
                owner,
                model,
                variant,
            } => Self::Debris(DebrisState {
                owner,
                model,
                variant,
                motion: flat.motion,
                attitude: flat.angles,
            }),
            Identity::Pilot { owner } => Self::Pilot(PilotState {
                owner,
                motion: flat.motion,
                heading: flat.angles[0],
                phase: PHASES[g(0, 0) as usize],
            }),
        })
    }

    /// Clamps the state's numbers to what the wire carries: positions and
    /// velocities within 2^40 steps. The quantizers already do; this is for
    /// states built by hand.
    fn check(&self) -> WireResult<()> {
        let m = self.motion();
        if m.position
            .iter()
            .chain(&m.velocity)
            .any(|v| v.abs() > STEP_LIMIT)
        {
            return Err(WireError::Invalid("entity position or velocity"));
        }
        if let Self::Aircraft(a) = self
            && let Some(d) = a.devices
            && d.surfaces.contains(&i8::MIN)
        {
            return Err(WireError::Invalid("control surface"));
        }
        Ok(())
    }
}

/// Position residuals after the prediction.
const POSITION_LADDER: [u32; 5] = [3, 6, 10, 14, 20];
/// Velocity differences.
const VELOCITY_LADDER: [u32; 5] = [3, 6, 10, 14, 20];
/// Angle differences, the short way (at most half a turn: 17 bits).
const ANGLE_LADDER: [u32; 5] = [3, 6, 9, 12, 17];
/// Speed differences.
const FAST_LADDER: [u32; 4] = [3, 6, 10, 16];

/// Writes `state` as a full record body (after the record header).
pub fn write_full(w: &mut BitWriter, state: &EntityState) -> WireResult<()> {
    state.check()?;
    state.identity().write(w);
    let flat = state.flat();
    let schema = schema(state.kind());
    for v in flat.motion.position.iter().chain(&flat.motion.velocity) {
        w.write_varint_signed(*v);
    }
    for angle in &flat.angles[..schema.angles] {
        let _ = w.write_bits(u64::from(*angle), 16);
    }
    for fast in &flat.fast[..schema.fast] {
        w.write_varint_signed(*fast);
    }
    for (values, group) in flat.groups.iter().zip(schema.groups) {
        for (index, (value, field)) in values.iter().zip(group.fields).enumerate() {
            field.write(w, *value);
            if group.gated && index == 0 && *value == 0 {
                break;
            }
        }
    }
    Ok(())
}

/// Reads a full record body of `kind`.
pub fn read_full(r: &mut BitReader<'_>, kind: EntityKind) -> WireResult<EntityState> {
    let identity = Identity::read(r, kind)?;
    let schema = schema(kind);
    let read_steps = |r: &mut BitReader<'_>| -> WireResult<i64> {
        in_range(
            i128::from(r.read_varint_signed()?),
            "entity position or velocity",
        )
    };
    let mut motion = Motion::default();
    for v in motion.position.iter_mut().chain(motion.velocity.iter_mut()) {
        *v = read_steps(r)?;
    }
    let mut angles = [0u16; 3];
    for angle in &mut angles[..schema.angles] {
        *angle = r.read_bits(16)? as u16;
    }
    let mut fast = [0i64; 1];
    for value in &mut fast[..schema.fast] {
        *value = i64::from(read_i32(r)?);
    }
    let mut groups = Vec::with_capacity(schema.groups.len());
    for group in schema.groups {
        let mut values = vec![0; group.fields.len()];
        for (index, field) in group.fields.iter().enumerate() {
            values[index] = field.read(r)?;
            if group.gated && index == 0 && values[0] == 0 {
                break;
            }
        }
        groups.push(values);
    }
    EntityState::from_flat(
        identity,
        &Flat {
            motion,
            angles,
            fast,
            groups,
        },
    )
}

/// A record against a baseline, as read: every difference and every changed
/// slow field, before the baseline is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delta {
    kind: EntityKind,
    /// `None`: exactly as predicted.
    motion: Option<MotionDelta>,
    /// Per group: `None` unchanged, else per field `None` unchanged or the new
    /// value.
    groups: Vec<Option<Vec<Option<i64>>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MotionDelta {
    position: [i64; 3],
    velocity: [i64; 3],
    angles: [i64; 3],
    fast: [i64; 1],
}

/// Writes `state` against `base`, its state `ticks` earlier, which the client
/// has. The caller has checked that they are the same entity
/// ([`EntityState::same_identity`]).
pub fn write_delta(
    w: &mut BitWriter,
    state: &EntityState,
    base: &EntityState,
    ticks: u32,
) -> WireResult<()> {
    state.check()?;
    if state.kind() != base.kind() {
        return Err(WireError::Invalid("baseline of another kind"));
    }
    let schema = schema(state.kind());
    let now = state.flat();
    let then = base.flat();
    let predicted = then.motion.predicted(ticks);
    let position: [i128; 3] =
        std::array::from_fn(|i| i128::from(now.motion.position[i]) - predicted[i]);
    let velocity: [i128; 3] = std::array::from_fn(|i| {
        i128::from(now.motion.velocity[i]) - i128::from(then.motion.velocity[i])
    });
    let angles: [i64; 3] = std::array::from_fn(|i| angle_diff16(then.angles[i], now.angles[i]));
    let fast = [now.fast[0] - then.fast[0]];
    let moved = position.iter().chain(&velocity).any(|v| *v != 0)
        || angles[..schema.angles].iter().any(|v| *v != 0)
        || fast[..schema.fast].iter().any(|v| *v != 0);
    w.write_bool(moved);
    if moved {
        // Both states are within 2^40 steps, so every difference fits i64.
        for v in position {
            w.write_bucketed(v as i64, &POSITION_LADDER)?;
        }
        for v in velocity {
            w.write_bucketed(v as i64, &VELOCITY_LADDER)?;
        }
        for v in &angles[..schema.angles] {
            w.write_bucketed(*v, &ANGLE_LADDER)?;
        }
        for v in &fast[..schema.fast] {
            w.write_bucketed(*v, &FAST_LADDER)?;
        }
    }
    for ((values, before), group) in now.groups.iter().zip(&then.groups).zip(schema.groups) {
        let changed = values != before;
        w.write_bool(changed);
        if changed {
            for (index, ((value, old), field)) in
                values.iter().zip(before).zip(group.fields).enumerate()
            {
                w.write_bool(value != old);
                if value != old {
                    field.write(w, *value);
                }
                // Gone: the rest are zero.
                if group.gated && index == 0 && *value == 0 {
                    break;
                }
            }
        }
    }
    Ok(())
}

/// Reads a record body against a baseline, of `kind`, without the baseline.
pub fn read_delta(r: &mut BitReader<'_>, kind: EntityKind) -> WireResult<Delta> {
    let schema = schema(kind);
    let motion = if r.read_bool()? {
        let mut m = MotionDelta {
            position: [0; 3],
            velocity: [0; 3],
            angles: [0; 3],
            fast: [0],
        };
        for v in &mut m.position {
            *v = r.read_bucketed(&POSITION_LADDER)?;
        }
        for v in &mut m.velocity {
            *v = r.read_bucketed(&VELOCITY_LADDER)?;
        }
        for v in &mut m.angles[..schema.angles] {
            *v = r.read_bucketed(&ANGLE_LADDER)?;
            if !(-32_768..32_768).contains(v) {
                return Err(WireError::Invalid("angle difference"));
            }
        }
        for v in &mut m.fast[..schema.fast] {
            *v = r.read_bucketed(&FAST_LADDER)?;
        }
        if m.position
            .iter()
            .chain(&m.velocity)
            .chain(&m.angles)
            .chain(&m.fast)
            .all(|v| *v == 0)
        {
            return Err(CodecError::NonCanonical.into());
        }
        Some(m)
    } else {
        None
    };
    let mut groups = Vec::with_capacity(schema.groups.len());
    for group in schema.groups {
        if r.read_bool()? {
            let mut values = vec![None; group.fields.len()];
            for (index, field) in group.fields.iter().enumerate() {
                values[index] = if r.read_bool()? {
                    Some(field.read(r)?)
                } else {
                    None
                };
                if group.gated && index == 0 && values[0] == Some(0) {
                    // Gone: the rest become zero.
                    for value in &mut values[1..] {
                        *value = Some(0);
                    }
                    break;
                }
            }
            if values.iter().all(Option::is_none) {
                return Err(CodecError::NonCanonical.into());
            }
            groups.push(Some(values));
        } else {
            groups.push(None);
        }
    }
    Ok(Delta {
        kind,
        motion,
        groups,
    })
}

impl Delta {
    /// The state this record describes, from `base`, the entity's state
    /// `ticks` earlier. An error when the result is not a valid state (the
    /// record was damaged or written against another baseline).
    pub fn apply(&self, base: &EntityState, ticks: u32) -> WireResult<EntityState> {
        if base.kind() != self.kind {
            return Err(WireError::Invalid("baseline of another kind"));
        }
        let then = base.flat();
        let mut flat = then.clone();
        let predicted = then.motion.predicted(ticks);
        let m = self.motion.clone().unwrap_or(MotionDelta {
            position: [0; 3],
            velocity: [0; 3],
            angles: [0; 3],
            fast: [0],
        });
        for (i, predicted) in predicted.iter().enumerate() {
            flat.motion.position[i] =
                in_range(predicted + i128::from(m.position[i]), "entity position")?;
            flat.motion.velocity[i] = in_range(
                i128::from(then.motion.velocity[i]) + i128::from(m.velocity[i]),
                "entity velocity",
            )?;
            flat.angles[i] = then.angles[i].wrapping_add(m.angles[i] as u16);
        }
        let fast = i128::from(then.fast[0]) + i128::from(m.fast[0]);
        flat.fast[0] = i64::from(i32::try_from(fast).map_err(|_| WireError::Invalid("speed"))?);
        for (group, change) in flat.groups.iter_mut().zip(&self.groups) {
            if let Some(change) = change {
                for (value, new) in group.iter_mut().zip(change) {
                    if let Some(new) = new {
                        *value = *new;
                    }
                }
            }
        }
        EntityState::from_flat(base.identity(), &flat)
    }
}

/// Every count a snapshot's entity part may hold, by kind.
pub fn check_count(kind: EntityKind, count: usize) -> WireResult<()> {
    if count > kind.limit() {
        return Err(WireError::TooMany {
            what: kind.what(),
            limit: kind.limit(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prediction_uses_whole_steps_both_ways() {
        let motion = Motion {
            position: [0, 100, -100],
            velocity: [240, -240, 1],
        };
        // 240/64 ft/s for 1 tick is 1/32 ft: one step.
        assert_eq!(motion.predicted(1), [1, 99, -100]);
        assert_eq!(motion.predicted(4), [4, 96, -100]);
        assert_eq!(motion.predicted(120), [120, -20, -99]);
    }
}
