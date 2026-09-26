//! The recording's data model: plain owned values the app fills once per tick.
//!
//! Units everywhere: feet, feet per second, radians, pounds, and ticks of
//! 1/120 second. World axes: X east, Y up (altitude above mean sea level),
//! Z north. Attitude is `[yaw, pitch, bank]`: yaw 0 faces north (+Z) and grows
//! clockwise (a quarter turn faces east), pitch is positive nose up, bank is
//! positive right wing down. The forward vector is
//! `[sin yaw cos pitch, sin pitch, cos yaw cos pitch]`.

use crate::FORMAT_VERSION;

/// Simulation ticks per second.
pub const TICKS_PER_SECOND: u64 = 120;

/// Recording header: who wrote it, when, and the resolved world, so a replay
/// never depends on environment variables or settings at playback time.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    /// The file's format version. The writer ignores this field and always
    /// writes [`FORMAT_VERSION`]; the reader reports what the file holds.
    pub format_version: u16,
    /// Game version, for example `0.1.0`.
    pub game_version: String,
    /// Source commit the game was built from.
    pub game_commit: String,
    /// When recording started, as UTC text: `2026-09-26T15:40:00Z`.
    pub recorded_at: String,
    /// What kind of flight this is.
    pub mission: MissionKind,
    /// The resolved world.
    pub world: World,
    /// Anything else worth keeping, in order: flight-model adapter, mission
    /// settings, cheats, probe notes such as "no weather stepping".
    pub extra: Vec<(String, String)>,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            game_version: String::new(),
            game_commit: String::new(),
            recorded_at: String::new(),
            mission: MissionKind::default(),
            world: World::default(),
            extra: Vec::new(),
        }
    }
}

impl Header {
    /// The first extra value stored under `key`.
    pub fn extra(&self, key: &str) -> Option<&str> {
        self.extra
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// What kind of flight a recording holds.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum MissionKind {
    QuickMission,
    #[default]
    FreeFlight,
    /// A headless AI probe run. Probes skip weather stepping, crew voice,
    /// music and HUD delivery; the header's extras say which.
    Probe,
    /// A kind this build does not know, kept verbatim.
    Other(String),
}

impl MissionKind {
    /// The stable text stored in the file.
    pub fn as_str(&self) -> &str {
        match self {
            Self::QuickMission => "quick-mission",
            Self::FreeFlight => "free-flight",
            Self::Probe => "probe",
            Self::Other(text) => text,
        }
    }

    pub fn parse(text: &str) -> Self {
        match text {
            "quick-mission" => Self::QuickMission,
            "free-flight" => Self::FreeFlight,
            "probe" => Self::Probe,
            other => Self::Other(other.to_owned()),
        }
    }

    /// A title for people: "Quick Mission", "Free Flight", "AI probe".
    pub fn title(&self) -> String {
        match self {
            Self::QuickMission => "Quick Mission".into(),
            Self::FreeFlight => "Free Flight".into(),
            Self::Probe => "AI probe".into(),
            Self::Other(text) if text.is_empty() => "Unknown mission".into(),
            Self::Other(text) => text.clone(),
        }
    }
}

/// The resolved world a recording was flown in.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct World {
    /// Base theater code, for example `UKR`.
    pub theater: String,
    /// Theater name for people, for example `Ukraine`.
    pub theater_name: String,
    /// Map layout resource, for example `UKR.MM` or a variant.
    pub layout: String,
    /// Weather condition index chosen for the mission, when there was one.
    pub weather: Option<u32>,
    /// Weather condition name, for example `cloudy`.
    pub weather_name: String,
    /// Seed that makes weather stepping repeatable, when known.
    pub weather_seed: Option<i64>,
    /// Start time of day in seconds after local midnight.
    pub time_of_day_s: f64,
    /// Wind as the velocity of the air in world axes, feet per second: the
    /// direction the air moves toward, not where it comes from.
    pub wind_fps: [f64; 3],
    /// Cloud settings.
    pub clouds: Clouds,
    /// Map size east and north in feet, from the origin corner. Exports use it
    /// to centre Tacview's map; without it they use the known theater size.
    pub extent_ft: Option<[f64; 2]>,
}

/// Cloud settings for the recording's weather.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Clouds {
    /// Weather module that draws the sky, for example `CLOUD1.LAY`.
    pub module: String,
    /// Height of a scattered cloud deck in feet above sea level, if any.
    pub deck_ft: Option<f64>,
}

/// Which side an aircraft flies for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Side {
    Friendly,
    Enemy,
    Neutral,
    #[default]
    Unknown,
}

impl Side {
    pub fn code(self) -> u8 {
        match self {
            Self::Friendly => 0,
            Self::Enemy => 1,
            Self::Neutral => 2,
            Self::Unknown => 3,
        }
    }

    pub fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Friendly,
            1 => Self::Enemy,
            2 => Self::Neutral,
            _ => Self::Unknown,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Friendly => "friendly",
            Self::Enemy => "enemy",
            Self::Neutral => "neutral",
            Self::Unknown => "unknown",
        }
    }
}

/// One aircraft's identity, registered once and referenced by id.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AircraftInfo {
    /// 0 is the player; AI aircraft start at 1.
    pub id: u32,
    /// Exact resource identity, for example `F18.PT`. Never aliased.
    pub pt: String,
    /// Display name, for example `F/A-18D`. Never aliased.
    pub name: String,
    /// Label, for example `You` or `Enemy 2-1`.
    pub label: String,
    pub side: Side,
    /// Wing number, 0 when the aircraft has no wing.
    pub wing: u16,
    /// Position within the wing, 0 when unknown.
    pub member: u16,
    /// Skill text, for example `Veteran`.
    pub skill: String,
    /// True for a human pilot.
    pub human: bool,
}

/// What a weapon is, for exports and the viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum WeaponClass {
    Gun,
    Missile,
    Bomb,
    Rocket,
    #[default]
    Other,
}

impl WeaponClass {
    pub fn code(self) -> u8 {
        match self {
            Self::Gun => 0,
            Self::Missile => 1,
            Self::Bomb => 2,
            Self::Rocket => 3,
            Self::Other => 4,
        }
    }

    pub fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Gun,
            1 => Self::Missile,
            2 => Self::Bomb,
            3 => Self::Rocket,
            _ => Self::Other,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Gun => "gun",
            Self::Missile => "missile",
            Self::Bomb => "bomb",
            Self::Rocket => "rocket",
            Self::Other => "other",
        }
    }
}

/// One weapon type's identity, registered once and referenced by id.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct WeaponInfo {
    pub id: u32,
    /// Weapon resource name.
    pub source: String,
    /// Shape resource the viewer draws, if any.
    pub shape: Option<String>,
    /// Display name, for example `AIM-120`.
    pub name: String,
    pub class: WeaponClass,
}

/// Everything recorded for one simulation tick.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Frame {
    pub tick: u64,
    pub aircraft: Vec<AircraftState>,
    pub projectiles: Vec<ProjectileState>,
    pub debris: Vec<DebrisState>,
    pub escapees: Vec<EscapeeState>,
    /// Effects that started this tick.
    pub new_effects: Vec<EffectSpawn>,
    /// Smoke and contrail puffs released this tick.
    pub new_puffs: Vec<PuffSpawn>,
    /// Surface objects whose hit points changed this tick: `(id, hp)`.
    pub surface_hp: Vec<(u32, i32)>,
    pub events: Vec<Event>,
    pub trees: Vec<TreeSample>,
    /// State checksum, set by the app once per second. See
    /// [`crate::state_checksum`].
    pub checksum: Option<u64>,
}

/// Number of animated device slots.
pub const DEVICE_COUNT: usize = 11;

/// Device slot indices for [`AircraftState::devices`].
pub mod device {
    /// Landing gear, 0 up to 1 down.
    pub const GEAR: usize = 0;
    /// Flaps, 0 to 1.
    pub const FLAPS: usize = 1;
    /// Air brake, 0 to 1.
    pub const BRAKE: usize = 2;
    /// Arresting hook, 0 to 1.
    pub const HOOK: usize = 3;
    /// Weapons bay doors, 0 to 1.
    pub const BAY: usize = 4;
    /// Exhaust nozzle, 0 to 1.
    pub const EXHAUST: usize = 5;
    /// Elevator deflection, -1 to 1.
    pub const ELEVATOR: usize = 6;
    /// Aileron deflection, -1 to 1.
    pub const AILERON: usize = 7;
    /// Rudder deflection, -1 to 1.
    pub const RUDDER: usize = 8;
    /// Speed in feet per second, which drives wing-sweep animation.
    pub const SPEED: usize = 9;
    /// Throttle, 0 to 1.
    pub const THROTTLE: usize = 10;
    /// Slot names in slot order.
    pub const NAMES: [&str; super::DEVICE_COUNT] = [
        "gear", "flaps", "brake", "hook", "bay", "exhaust", "elevator", "aileron", "rudder",
        "speed", "throttle",
    ];
}

/// Pilot control indices for [`AircraftState::controls`].
pub mod control {
    /// Pitch stick, -1 to 1.
    pub const PITCH: usize = 0;
    /// Roll stick, -1 to 1.
    pub const ROLL: usize = 1;
    /// Rudder pedals, -1 to 1.
    pub const YAW: usize = 2;
    /// Throttle, 0 to 1.
    pub const THROTTLE: usize = 3;
    /// Control names in index order.
    pub const NAMES: [&str; 4] = ["pitch", "roll", "yaw", "throttle"];
}

/// Number of regional damage amounts.
pub const SECTION_COUNT: usize = 6;

/// Aircraft condition switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct AircraftFlags {
    pub engine_on: bool,
    pub afterburner: bool,
    pub airborne: bool,
    pub on_ground: bool,
    pub crashed: bool,
    pub wreck_gone: bool,
    pub alive: bool,
    pub ejected: bool,
}

impl AircraftFlags {
    /// Flag names in bit order.
    pub const NAMES: [&str; 8] = [
        "engine_on",
        "afterburner",
        "airborne",
        "on_ground",
        "crashed",
        "wreck_gone",
        "alive",
        "ejected",
    ];

    fn array(self) -> [bool; 8] {
        [
            self.engine_on,
            self.afterburner,
            self.airborne,
            self.on_ground,
            self.crashed,
            self.wreck_gone,
            self.alive,
            self.ejected,
        ]
    }

    pub fn bits(self) -> u16 {
        self.array()
            .iter()
            .enumerate()
            .fold(0, |bits, (i, on)| bits | (u16::from(*on) << i))
    }

    /// Unknown bits from newer files are ignored.
    pub fn from_bits(bits: u16) -> Self {
        let on = |i: u16| bits & (1 << i) != 0;
        Self {
            engine_on: on(0),
            afterburner: on(1),
            airborne: on(2),
            on_ground: on(3),
            crashed: on(4),
            wreck_gone: on(5),
            alive: on(6),
            ejected: on(7),
        }
    }

    /// Names of the flags that are set, in bit order.
    pub fn names(self) -> Vec<&'static str> {
        self.array()
            .iter()
            .zip(Self::NAMES)
            .filter(|(on, _)| **on)
            .map(|(_, name)| name)
            .collect()
    }
}

/// One aircraft's state in one tick.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AircraftState {
    pub id: u32,
    /// Feet, world axes.
    pub position: [f64; 3],
    /// `[yaw, pitch, bank]` in radians.
    pub attitude: [f64; 3],
    /// Ground-relative velocity, feet per second.
    pub velocity: [f64; 3],
    /// Airspeed, feet per second.
    pub airspeed: f64,
    /// Load factor in G.
    pub g: f64,
    /// Animated devices, in the order of [`device`].
    pub devices: [f64; DEVICE_COUNT],
    /// Resolved engine heat input, 0 to 1.
    pub heat: f64,
    pub flags: AircraftFlags,
    /// Wreck animation phase.
    pub wreck_phase: u8,
    /// Fuel on board, pounds.
    pub fuel_lb: f64,
    /// Pilot controls, in the order of [`control`].
    pub controls: [f64; 4],
    /// Hit points, exact.
    pub hp: i32,
    pub max_hp: i32,
    /// Regional damage amounts, exact.
    pub sections: [i32; SECTION_COUNT],
    /// The region whose structure failed, if any.
    pub structural_section: Option<u8>,
}

impl AircraftState {
    /// Unit vector along the nose.
    pub fn forward(&self) -> [f64; 3] {
        forward(self.attitude)
    }

    /// Ground speed, feet per second.
    pub fn ground_speed(&self) -> f64 {
        let [x, y, z] = self.velocity;
        (x * x + y * y + z * z).sqrt()
    }
}

/// Unit vector along the nose for `[yaw, pitch, bank]`.
pub fn forward(attitude: [f64; 3]) -> [f64; 3] {
    let (sy, cy) = attitude[0].sin_cos();
    let (sp, cp) = attitude[1].sin_cos();
    [sy * cp, sp, cy * cp]
}

/// A guided weapon's seeker.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Seeker {
    pub acquired: bool,
    pub status: u8,
    pub quality: f32,
    pub target: Option<u32>,
}

/// One live projectile in one tick.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ProjectileState {
    /// Player projectiles take ids from a shot counter; AI projectiles start at
    /// `1 << 24`.
    pub id: u32,
    /// Aircraft that fired it.
    pub owner: u32,
    /// [`WeaponInfo`] id.
    pub weapon: u32,
    pub target: Option<u32>,
    pub position: [f64; 3],
    /// Position one tick earlier, used for tracer segments and hits.
    pub previous: [f64; 3],
    /// Unit vector along the direction of travel.
    pub direction: [f64; 3],
    /// Feet per second.
    pub speed: f64,
    pub tracer: bool,
    /// True when it is inbound on the player.
    pub incoming: bool,
    /// Ticks since launch.
    pub age: u32,
    pub seeker: Option<Seeker>,
}

/// One debris piece in one tick.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DebrisState {
    /// Aircraft it came from.
    pub owner: u32,
    /// Stable index among that aircraft's pieces.
    pub index: u32,
    pub position: [f64; 3],
    pub attitude: [f64; 3],
}

/// One ejected pilot in one tick.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EscapeeState {
    /// Aircraft the pilot left.
    pub owner: u32,
    pub position: [f64; 3],
    /// Radians, the same convention as yaw.
    pub heading: f64,
    pub phase: u8,
}

/// Visual effect kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EffectKind {
    Flare,
    Chaff,
    Launch,
    Hit,
    Destroyed,
    Ground,
    DebrisImpact,
    /// A kind this build does not know. Codes 0 to 6 belong to the named kinds.
    Other(u8),
}

impl EffectKind {
    /// First code free for `Other`.
    pub const FIRST_OTHER: u8 = 7;

    pub fn code(self) -> u8 {
        match self {
            Self::Flare => 0,
            Self::Chaff => 1,
            Self::Launch => 2,
            Self::Hit => 3,
            Self::Destroyed => 4,
            Self::Ground => 5,
            Self::DebrisImpact => 6,
            Self::Other(code) => code,
        }
    }

    pub fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Flare,
            1 => Self::Chaff,
            2 => Self::Launch,
            3 => Self::Hit,
            4 => Self::Destroyed,
            5 => Self::Ground,
            6 => Self::DebrisImpact,
            other => Self::Other(other),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Flare => "flare",
            Self::Chaff => "chaff",
            Self::Launch => "launch",
            Self::Hit => "hit",
            Self::Destroyed => "destroyed",
            Self::Ground => "ground",
            Self::DebrisImpact => "debris_impact",
            Self::Other(_) => "other",
        }
    }
}

/// An effect that started this tick. The viewer rebuilds it from its start
/// time, so reverse play needs no stored per-tick effect state.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectSpawn {
    pub kind: EffectKind,
    pub position: [f64; 3],
    pub duration_ticks: u32,
}

/// Smoke puff kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PuffKind {
    Missile,
    Aircraft,
    Contrail,
    /// A kind this build does not know. Codes 0 to 2 belong to the named kinds.
    Other(u8),
}

impl PuffKind {
    /// First code free for `Other`.
    pub const FIRST_OTHER: u8 = 3;

    pub fn code(self) -> u8 {
        match self {
            Self::Missile => 0,
            Self::Aircraft => 1,
            Self::Contrail => 2,
            Self::Other(code) => code,
        }
    }

    pub fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Missile,
            1 => Self::Aircraft,
            2 => Self::Contrail,
            other => Self::Other(other),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Missile => "missile",
            Self::Aircraft => "aircraft",
            Self::Contrail => "contrail",
            Self::Other(_) => "other",
        }
    }

    /// How long a puff of this kind lives, matching the simulation's smoke:
    /// 480 ticks for missile smoke, 960 for aircraft smoke, 14,400 for
    /// contrails. Unknown kinds have no known lifetime.
    pub fn lifetime_ticks(self) -> Option<u64> {
        match self {
            Self::Missile => Some(480),
            Self::Aircraft => Some(960),
            Self::Contrail => Some(14_400),
            Self::Other(_) => None,
        }
    }

    /// Smoke rises 2 feet per second; contrails stay where they were left.
    pub fn rise_fps(self) -> f64 {
        match self {
            Self::Contrail => 0.,
            _ => 2.,
        }
    }
}

/// Puff layer for smoke.
pub const LAYER_SMOKE: u8 = 0;
/// Puff layer for contrails.
pub const LAYER_CONTRAILS: u8 = 1;

/// A smoke or contrail puff released this tick.
#[derive(Clone, Debug, PartialEq)]
pub struct PuffSpawn {
    /// [`LAYER_SMOKE`] or [`LAYER_CONTRAILS`].
    pub layer: u8,
    pub kind: PuffKind,
    pub position: [f64; 3],
}

/// A value carried by an event field or a display tree node.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    #[default]
    None,
    Bool(bool),
    Int(i64),
    Num(f64),
    Text(String),
    /// An entity id: aircraft, projectile, weapon or surface object.
    Id(u32),
    Ids(Vec<u32>),
}

impl Value {
    /// The number held by `Int` or `Num`.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Int(v) => Some(*v as f64),
            Self::Num(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            Self::Id(v) => Some(i64::from(*v)),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// The id held by `Id`, or by a non-negative `Int` that fits.
    pub fn as_id(&self) -> Option<u32> {
        match self {
            Self::Id(v) => Some(*v),
            Self::Int(v) => u32::try_from(*v).ok(),
            _ => None,
        }
    }

    pub fn as_ids(&self) -> Option<&[u32]> {
        match self {
            Self::Ids(v) => Some(v),
            _ => None,
        }
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Int(i64::from(v))
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Num(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::Text(v.to_owned())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl From<Vec<u32>> for Value {
    fn from(v: Vec<u32>) -> Self {
        Self::Ids(v)
    }
}

/// Something that happened at one tick: combat, AI, flight, comms, audio,
/// player or system. Kinds and field names come from [`crate::vocab`], so new
/// producers never need a format change.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Event {
    pub kind: String,
    /// The main actor: shooter, speaker, sender, the aircraft that changed.
    pub subject: Option<u32>,
    /// The other party: target, killer, recipient.
    pub object: Option<u32>,
    pub fields: Vec<(String, Value)>,
    /// Free text for people, for example the words of a radio call.
    pub text: String,
}

impl Event {
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            ..Self::default()
        }
    }

    pub fn with_subject(mut self, id: u32) -> Self {
        self.subject = Some(id);
        self
    }

    pub fn with_object(mut self, id: u32) -> Self {
        self.object = Some(id);
        self
    }

    pub fn with(mut self, name: &str, value: impl Into<Value>) -> Self {
        self.fields.push((name.to_owned(), value.into()));
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    /// The first field called `name`.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    pub fn num(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(Value::as_f64)
    }

    pub fn id(&self, name: &str) -> Option<u32> {
        self.get(name).and_then(Value::as_id)
    }

    pub fn string(&self, name: &str) -> Option<&str> {
        self.get(name).and_then(Value::as_str)
    }

    pub fn flag(&self, name: &str) -> Option<bool> {
        self.get(name).and_then(Value::as_bool)
    }
}

/// One display tree: an AI's thinking, flight-model telemetry or a weapon's
/// guidance, as panels show it. Delta-coded in the file, so unchanged lines
/// cost almost nothing.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TreeSample {
    /// Aircraft or projectile the tree describes.
    pub subject: u32,
    /// A channel from [`crate::vocab::channel`].
    pub channel: String,
    pub nodes: Vec<Node>,
}

impl TreeSample {
    /// The first node labelled `label`.
    pub fn node(&self, label: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.label == label)
    }
}

/// One line of a display tree.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Node {
    /// Indent level, 0 to 31.
    pub depth: u8,
    pub label: String,
    pub value: Value,
    /// Unit from [`crate::vocab::unit`], or empty.
    pub unit: String,
    /// The "because" text: why the value is what it is.
    pub note: String,
}

impl Node {
    pub fn new(depth: u8, label: &str, value: impl Into<Value>) -> Self {
        Self {
            depth,
            label: label.to_owned(),
            value: value.into(),
            ..Self::default()
        }
    }

    pub fn with_unit(mut self, unit: &str) -> Self {
        self.unit = unit.to_owned();
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }
}

/// Written once when recording finishes.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Footer {
    /// The tick the mission ended.
    pub end_tick: u64,
    /// Debrief outcome and anything else worth keeping, in order.
    pub result: Vec<(String, String)>,
}
