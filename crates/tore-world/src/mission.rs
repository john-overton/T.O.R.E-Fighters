//! The mission as data: a Quick Mission written with stable names instead of
//! the creator's list positions, which differ between installs, and its text
//! form, the mission file of docs/DEDICATED-SERVER.md.
//!
//! [`MissionSpec`] is everything [`World::new`](crate::world::World::new)
//! builds a mission from: the theater and weather, the start, the six wings,
//! the AI's orders, the cheats, the flight models and, for a single-player
//! start, the loadout of plane 0. The game's creator turns its draft into
//! one; a server reads one from its mission file and sends the same text to
//! every joining player.
//!
//! The text is one `key value` line each, `#` starts a comment, and the first
//! line is `tore-mission 1`. A parse error names the line. Unknown keys,
//! aircraft, theaters, skills and out-of-range values are refused.

use crate::{
    ai_wings::{AiFlightModel, Preset},
    mission_layout::SEPARATION_NM,
    resources::ResourceSource,
    terrain::{CONDITION_NAMES, Overrides},
};
use std::collections::BTreeMap;
use std::fmt;
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::{
    ai::{
        AiError, Experience,
        engagement::GroupObjective,
        experience::EnemySkillOverride,
        launch::{
            DUMMY_SKILL, MAX_WING_MEMBERS, Side, WINGS_PER_SIDE, WingId, WingLaunch, WingSelection,
            legacy_pairs, resolve_wings,
        },
    },
    cheats::{Cheats, Damage},
    combat::loadout::Loadout,
};

/// The version in the first line of the text form.
pub const FORMAT_VERSION: u32 = 1;

/// The sixteen theaters the creator offers, by code.
pub const THEATERS: [&str; 16] = [
    "BAL", "CUB", "EGY", "LFA", "FRA", "GRE", "IRA", "KURILE", "TVIET", "SPA", "APA", "PGU", "NSK",
    "WTA", "UKR", "VLA",
];

/// The airborne start altitudes the creator offers, in feet.
pub const ALTITUDES_FT: [u32; 4] = [5_000, 10_000, 20_000, 40_000];

/// The creator's default altitude setting.
pub const DEFAULT_ALTITUDE_FT: u32 = 5_000;

/// A runway object's id is this plus its ordinal in the theater's layout.
pub const RUNWAY_OBJECT_BASE: u32 = 0x4000_0000;

/// Wings in all: friendly 1 to 3, then enemy 1 to 3.
pub const WINGS: usize = 6;

/// The largest wing the creator offers (friendly wing 1 counts the player).
pub const MAX_WING: usize = MAX_WING_MEMBERS;

/// The most planes a mission has: six wings of five. Planes are numbered
/// from 0.
pub const MAX_PLANES: u32 = (WINGS * MAX_WING) as u32;

/// The most stations a loadout lists.
pub const MAX_STATIONS: usize = 64;

/// One of the six weather and time of day choices the creator offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    Clear,
    Cloudy,
    Foggy,
    Dawn,
    Sunset,
    Night,
}

impl Condition {
    pub const ALL: [Self; 6] = [
        Self::Clear,
        Self::Cloudy,
        Self::Foggy,
        Self::Dawn,
        Self::Sunset,
        Self::Night,
    ];

    /// Its place in the recovered source table, which is the order of
    /// [`CONDITION_NAMES`].
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    pub fn name(self) -> &'static str {
        CONDITION_NAMES[self.index()]
    }

    fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// Where the player's aircraft, and with it the mission, starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// In the air at one of [`ALTITUDES_FT`].
    Airborne { altitude_ft: u32 },
    /// Parked on a runway, by its object id (`RUNWAY_OBJECT_BASE` plus its
    /// ordinal in the theater's layout). `altitude_ft` is the creator's
    /// altitude setting, which a ground start keeps: airborne aircraft of
    /// the mission need it to clear the ground.
    Ground { runway: u32, altitude_ft: u32 },
}

impl Start {
    /// The creator's altitude setting.
    pub fn altitude_ft(self) -> u32 {
        match self {
            Self::Airborne { altitude_ft } | Self::Ground { altitude_ft, .. } => altitude_ft,
        }
    }
}

/// How good a wing's pilots are. `Dummy` is the creator's training setting:
/// a constant-heading 400-knot target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skill {
    Novice,
    Average,
    Experienced,
    Ace,
    Dummy,
}

impl Skill {
    pub const ALL: [Self; 5] = [
        Self::Novice,
        Self::Average,
        Self::Experienced,
        Self::Ace,
        Self::Dummy,
    ];

    /// The level the launch payload takes: 0 to 3, or 4 for the dummy.
    pub fn level(self) -> i32 {
        match self {
            Self::Novice => Experience::Novice as i32,
            Self::Average => Experience::Average as i32,
            Self::Experienced => Experience::Experienced as i32,
            Self::Ace => Experience::Ace as i32,
            Self::Dummy => DUMMY_SKILL,
        }
    }

    /// The skill for a creator skill field (0 to 4).
    pub fn from_level(level: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|skill| skill.level() == level)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Novice => "novice",
            Self::Average => "average",
            Self::Experienced => "experienced",
            Self::Ace => "ace",
            Self::Dummy => "dummy",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|skill| skill.name() == name)
    }
}

/// One wing: what it flies, how many and how well. Friendly wing 1's count
/// includes the player, whose aircraft is that wing's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WingSpec {
    pub aircraft: AircraftId,
    pub count: usize,
    pub skill: Skill,
}

/// One station of the player's load: the weapon's resource name, the
/// station's capacity and how many it carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StationLoad {
    pub weapon: String,
    pub count: u16,
    pub quantity: u16,
}

/// The loadout of plane 0 when a player flies it from the start, as the
/// creator's Load Ordnance page leaves it. `None` in a spec means the
/// aircraft's standard load. A server does not use it: a networked plane
/// carries its aircraft's standard load (stage F lets a player choose).
#[derive(Clone, Debug, PartialEq)]
pub struct LoadoutSpec {
    pub fuel_lbs: f64,
    /// The loadout screen's Cheat: any store on any station.
    pub cheat: bool,
    /// One per station of the aircraft, in its station order.
    pub stations: Vec<StationLoad>,
}

impl LoadoutSpec {
    /// What a loadout holds, by name.
    pub fn of(load: &Loadout) -> Self {
        Self {
            fuel_lbs: load.fuel_lbs,
            cheat: load.cheat,
            stations: load
                .configuration
                .stations
                .iter()
                .zip(&load.quantities)
                .map(|(station, &quantity)| StationLoad {
                    weapon: station.weapon.source.clone(),
                    count: station.count,
                    quantity,
                })
                .collect(),
        }
    }

    /// Puts this load into `base`, the aircraft's standard loadout. A station
    /// that carries another weapon than the standard one has that weapon read
    /// from the resources (`label` tidies its display name, as the game's
    /// loadout screen does for the ones it offers). The result is not yet
    /// validated.
    pub fn apply(
        &self,
        mut base: Loadout,
        resources: &dyn ResourceSource,
        label: Option<&dyn Fn(&mut Weapon)>,
    ) -> crate::WorldResult<Loadout> {
        if self.stations.len() != base.configuration.stations.len() {
            return Err(format!(
                "the loadout has {} stations but {} has {}",
                self.stations.len(),
                base.aircraft.pt(),
                base.configuration.stations.len()
            )
            .into());
        }
        base.cheat = self.cheat;
        base.fuel_lbs = self.fuel_lbs;
        for (index, load) in self.stations.iter().enumerate() {
            let station = &mut base.configuration.stations[index];
            if station.weapon.source != load.weapon {
                let bytes = resources
                    .get(&load.weapon)
                    .ok_or_else(|| format!("missing loadout resource {}", load.weapon))?;
                let mut weapon = Weapon::parse(&load.weapon, bytes)?;
                if let Some(label) = label {
                    label(&mut weapon);
                }
                station.weapon = weapon;
            }
            station.count = load.count;
            base.quantities[index] = load.quantity;
        }
        Ok(base)
    }
}

impl LoadoutSpec {
    /// The loadout a player chose for one plane of an open mission, put onto
    /// the aircraft's standard load and checked. This is the multiplayer
    /// lobby's rule (EF4, agent decision): the checks are the ones the
    /// single-player Load Ordnance page makes before Fly, which live in
    /// `tore_sim::combat::loadout` and are shared, not copied
    /// ([`Loadout::validate`]: every quantity within its station's capacity
    /// for that weapon, only weapons connected to flight, the fuel within the
    /// tanks, the weight within the maximum take-off weight), plus three the
    /// page makes by its controls, which a message must be checked for:
    ///
    /// - one station for each of the aircraft's, in its order;
    /// - each station's capacity the one the page gives it: the standard
    ///   count for the standard weapon, else the station's capacity for the
    ///   weapon (what selecting it on the page sets);
    /// - with the creator's Guns only, nothing loaded but the gun (the
    ///   single-player build's own refusal, in its words).
    ///
    /// Cheat loading (any store on any station) is refused: a multiplayer
    /// aircraft carries what it really carries unless the King's `loadouts`
    /// setting says `any` ([`LoadoutSpec::check_in`]).
    pub fn check_for_plane(
        &self,
        aircraft: &tore_formats::aircraft::Aircraft,
        resources: &dyn ResourceSource,
        guns_only: bool,
    ) -> crate::WorldResult<Loadout> {
        self.check_under(aircraft, resources, guns_only, false)
    }

    /// [`LoadoutSpec::check_for_plane`] under the loadout rule of `mission`
    /// (stage F phase 2, slice F2-1): its Guns only, and its
    /// [`MissionSpec::cheat_loadouts`], under which a Cheat loading is
    /// allowed and checked by the Load Ordnance page's own cheat rules (each
    /// station's cheat capacity), as single player's page checks it.
    pub fn check_in(
        &self,
        aircraft: &tore_formats::aircraft::Aircraft,
        resources: &dyn ResourceSource,
        mission: &MissionSpec,
    ) -> crate::WorldResult<Loadout> {
        self.check_under(
            aircraft,
            resources,
            mission.guns_only,
            mission.cheat_loadouts,
        )
    }

    fn check_under(
        &self,
        aircraft: &tore_formats::aircraft::Aircraft,
        resources: &dyn ResourceSource,
        guns_only: bool,
        cheat_allowed: bool,
    ) -> crate::WorldResult<Loadout> {
        if self.cheat && !cheat_allowed {
            return Err("Cheat loading is not allowed in a multiplayer game.".into());
        }
        let standard = Loadout::new(aircraft, |name| {
            resources
                .get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing loadout resource {name}")))
        })?;
        let load = self.apply(standard.clone(), resources, None)?;
        for (index, station) in load.configuration.stations.iter().enumerate() {
            let default = &standard.configuration.stations[index];
            let expected = if station.weapon.source == default.weapon.source {
                i32::from(default.count)
            } else {
                load.capacity(index, &station.weapon)
            };
            if i32::from(station.count) != expected {
                return Err(format!(
                    "Station {} cannot hold {} {}.",
                    index + 1,
                    station.count,
                    station.weapon.source
                )
                .into());
            }
        }
        if guns_only
            && load
                .configuration
                .stations
                .iter()
                .zip(&load.quantities)
                .any(|(s, n)| s.weapon.source != load.aircraft.gun() && *n > 0)
        {
            return Err("Guns only is selected. Unload other weapons or return to setup and change the restriction.".into());
        }
        load.validate()?;
        Ok(load)
    }
}

/// One plane of an open mission, numbered as the game numbers them: plane 0
/// is the lead of friendly wing 1, then every other aircraft in wing order
/// (friendly wings 1 to 3, then enemy wings 1 to 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPlane {
    pub id: u32,
    pub wing: WingId,
    /// Its place in the wing, from 0 (the lead).
    pub member: u8,
    pub aircraft: AircraftId,
}

/// A Quick Mission with stable names. See the module comment.
#[derive(Clone, Debug, PartialEq)]
pub struct MissionSpec {
    /// The theater's layout code: one of [`THEATERS`], or a `~` layout variant
    /// the game's developer options reach (the text form takes only the
    /// sixteen).
    pub theater: String,
    pub condition: Condition,
    /// The start time, wind and cloud deck that replace the condition's own:
    /// what the game's `TORE_WEATHER_TIME`, `TORE_WIND` and
    /// `TORE_CLOUD_ALTITUDE` resolve to. Resolved by the caller, so building
    /// the mission reads no environment variable.
    pub weather: Overrides,
    pub start: Start,
    /// The enemy's distance in nautical miles, one of the creator's
    /// [`SEPARATION_NM`].
    pub separation_nm: u32,
    /// The AI's standing orders for groups that inherit.
    pub preset: Preset,
    /// The creator's air combat setting: guns only.
    pub guns_only: bool,
    /// Friendly wings 1 to 3, then enemy wings 1 to 3.
    pub wings: [WingSpec; WINGS],
    pub objectives: [GroupObjective; WINGS],
    /// Each wing must survive for the mission to succeed.
    pub must_survive: [bool; WINGS],
    /// The cheats in force from the first tick.
    pub cheats: Cheats,
    /// The human-flown planes fly the hybrid model (the default); `false` is
    /// the game's `--legacy-flight`.
    pub researched_flight: bool,
    /// The flight model of every AI aircraft.
    pub ai_flight_model: AiFlightModel,
    /// The game's `--enemy-skill`: every enemy wing at one level.
    pub enemy_skill: Option<EnemySkillOverride>,
    /// The game's `--fixture-wings`: straight-flight fixtures instead of AI
    /// wings. A development setting; a server does not use it.
    pub fixture_wings: bool,
    /// The loadout of plane 0; see [`LoadoutSpec`].
    pub loadout: Option<LoadoutSpec>,
    /// The loadouts of an open mission's planes, by plane number, as the
    /// players who hold them chose them in the lobby (stage F, EF4). A plane
    /// with none carries its aircraft's standard load. Single player has
    /// none: it keeps [`Self::loadout`] for plane 0.
    pub plane_loadouts: BTreeMap<u32, LoadoutSpec>,
    /// Friendly fire (stage F phase 2, the King's `friendly-fire`): when
    /// `false` no round damages an aircraft of its shooter's side. Text form
    /// `friendly-fire off`, on when absent; a networked mission's setting,
    /// which single player's build refuses off.
    pub friendly_fire: bool,
    /// The King's `loadouts any` (stage F phase 2): a plane's loadout may be a
    /// Cheat loading, any store on any station. Text form `loadouts any`,
    /// `own` when absent; a networked mission's setting, which single
    /// player's build refuses.
    pub cheat_loadouts: bool,
}

impl MissionSpec {
    /// A mission as the creator's defaults set it up: clear weather, an
    /// airborne start at 5,000 feet, the enemy 5 miles away, one friendly
    /// flight of one (the player) and no enemy.
    pub fn new(theater: &str, player: AircraftId) -> Self {
        let wing = |count, skill| WingSpec {
            aircraft: player,
            count,
            skill,
        };
        Self {
            theater: theater.to_owned(),
            condition: Condition::Clear,
            weather: Overrides::default(),
            start: Start::Airborne {
                altitude_ft: DEFAULT_ALTITUDE_FT,
            },
            separation_nm: 5,
            preset: Preset::Free,
            guns_only: false,
            wings: [
                wing(1, Skill::Average),
                wing(0, Skill::Novice),
                wing(0, Skill::Novice),
                wing(0, Skill::Novice),
                wing(0, Skill::Novice),
                wing(0, Skill::Novice),
            ],
            objectives: [GroupObjective::Inherit; WINGS],
            must_survive: [false; WINGS],
            cheats: Cheats::default(),
            researched_flight: true,
            ai_flight_model: AiFlightModel::AllHybrid,
            enemy_skill: None,
            fixture_wings: false,
            loadout: None,
            plane_loadouts: BTreeMap::new(),
            friendly_fire: true,
            cheat_loadouts: false,
        }
    }

    /// The player's aircraft: friendly wing 1's.
    pub fn player(&self) -> AircraftId {
        self.wings[0].aircraft
    }

    /// The wing at position `index`: friendly 1 to 3, then enemy 1 to 3.
    pub fn wing_id(index: usize) -> WingId {
        let per_side = usize::from(WINGS_PER_SIDE);
        let side = if index < per_side {
            Side::Friendly
        } else {
            Side::Enemy
        };
        WingId::new(side, (index % per_side) as u8).expect("a fixed wing")
    }

    /// The position of a wing in [`MissionSpec::wings`].
    pub fn wing_index(id: WingId) -> usize {
        usize::from(id.index)
            + if id.side == Side::Enemy {
                usize::from(WINGS_PER_SIDE)
            } else {
                0
            }
    }

    /// The runway of a ground start.
    pub fn ground_runway(&self) -> Option<u32> {
        match self.start {
            Start::Ground { runway, .. } => Some(runway),
            Start::Airborne { .. } => None,
        }
    }

    /// The enemy's distance in feet.
    pub fn separation_feet(&self) -> f64 {
        f64::from(self.separation_nm) * crate::mission_layout::FEET_PER_NM
    }

    /// The six wings as the launch payload takes them: friendly wing 1 gives
    /// up one slot to the player.
    pub fn wing_selections(&self) -> Vec<WingSelection> {
        self.selections(true)
    }

    /// The six wings for the AI, with the player's slot left out of friendly
    /// wing 1 when `player` flies its lead, and its full count otherwise.
    fn selections(&self, player: bool) -> Vec<WingSelection> {
        self.wings
            .iter()
            .enumerate()
            .map(|(index, wing)| WingSelection {
                wing: Self::wing_id(index),
                aircraft: wing.aircraft,
                count: wing.count.saturating_sub(usize::from(player && index == 0)),
                skill_level: wing.skill.level(),
            })
            .collect()
    }

    /// The six wings resolved into per-member launch rows, with the enemy
    /// skill override applied.
    pub fn wing_launches(&self) -> Result<Vec<WingLaunch>, AiError> {
        resolve_wings(&self.wing_selections(), self.enemy_skill)
    }

    /// [`Self::wing_launches`] for an open mission, where the AI flies every
    /// plane: friendly wing 1 keeps its full count, its lead (plane 0)
    /// included.
    pub fn open_wing_launches(&self) -> Result<Vec<WingLaunch>, AiError> {
        resolve_wings(&self.selections(false), self.enemy_skill)
    }

    /// The aircraft and count pairs the straight-flight fixtures take (no
    /// override: they are not the AI).
    pub fn fixture_pairs(&self) -> Vec<(AircraftId, usize)> {
        resolve_wings(&self.wing_selections(), None)
            .map(|wings| legacy_pairs(&wings))
            .unwrap_or_default()
    }

    /// Aircraft in the player's wing, the player included.
    pub fn player_wing_size(&self) -> usize {
        self.wings[0].count.clamp(1, MAX_WING)
    }

    /// Every plane of the mission when it is open (every plane on the AI,
    /// plane 0 included), in plane order.
    pub fn open_planes(&self) -> Vec<OpenPlane> {
        let mut planes = Vec::new();
        for (index, wing) in self.wings.iter().enumerate() {
            for member in 0..wing.count.min(MAX_WING) {
                planes.push(OpenPlane {
                    id: planes.len() as u32,
                    wing: Self::wing_id(index),
                    member: member as u8,
                    aircraft: wing.aircraft,
                });
            }
        }
        planes
    }

    /// A one-line summary for a lobby or a game list, for example `UKR,
    /// clear, airborne at 20000 ft: F/A-18D Hornet x4 against MiG-29 Fulcrum-C x4`.
    pub fn summary(&self) -> String {
        let side = |enemy: bool| {
            let wings: Vec<String> = self
                .wings
                .iter()
                .enumerate()
                .filter(|(index, wing)| (*index >= 3) == enemy && wing.count > 0)
                .map(|(_, wing)| format!("{} x{}", wing.aircraft.label(), wing.count))
                .collect();
            if wings.is_empty() {
                "nobody".to_owned()
            } else {
                wings.join(", ")
            }
        };
        let start = match self.start {
            Start::Airborne { altitude_ft } => format!("airborne at {altitude_ft} ft"),
            Start::Ground { .. } => "ground start".to_owned(),
        };
        format!(
            "{}, {}, {start}: {} against {}",
            self.theater,
            self.condition.name(),
            side(false),
            side(true)
        )
    }

    /// Refuses a spec no mission can be built from. A spec that parses from
    /// text is valid.
    pub fn validate(&self) -> Result<(), MissionError> {
        if self.theater.is_empty()
            || !self
                .theater
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'~' || b == b'_')
        {
            return refuse(format!("`{}` is not a theater code", self.theater));
        }
        if !ALTITUDES_FT.contains(&self.start.altitude_ft()) {
            return refuse(format!(
                "the altitude must be one of {}, not {}",
                join(ALTITUDES_FT),
                self.start.altitude_ft()
            ));
        }
        if let Start::Ground { runway, .. } = self.start
            && runway < RUNWAY_OBJECT_BASE
        {
            return refuse(format!("{runway} is not a runway object"));
        }
        if !SEPARATION_NM.contains(&f64::from(self.separation_nm)) {
            return refuse(format!(
                "the separation must be one of {} nautical miles, not {}",
                join(SEPARATION_NM.map(|nm| nm as u32)),
                self.separation_nm
            ));
        }
        if let Some([hour, minute]) = self.weather.time
            && (!(0..24).contains(&hour) || !(0..60).contains(&minute))
        {
            return refuse(format!("{hour:02}:{minute:02} is not a time of day"));
        }
        if let Some([heading, speed]) = self.weather.wind
            && (!(-360..=360).contains(&heading) || !(0..=200).contains(&speed))
        {
            return refuse(format!(
                "the wind heading must be -360 to 360 degrees and its speed 0 to 200 feet a second, not {heading} and {speed}"
            ));
        }
        if let Some(deck) = self.weather.cloud_altitude
            && !(0..=400_000).contains(&deck)
        {
            return refuse(format!(
                "the cloud deck must be 0 to 400000 feet, not {deck}"
            ));
        }
        for (index, wing) in self.wings.iter().enumerate() {
            let least = usize::from(index == 0);
            if !(least..=MAX_WING).contains(&wing.count) {
                return refuse(format!(
                    "{} holds {least} to {MAX_WING} aircraft, not {}",
                    if index == 0 {
                        "friendly wing 1 (with the player)"
                    } else {
                        "a wing"
                    },
                    wing.count
                ));
            }
        }
        for (index, objective) in self.objectives.iter().enumerate() {
            let own = Self::wing_id(index);
            match *objective {
                GroupObjective::Intercept(target) if target.side == own.side => {
                    return refuse(format!(
                        "{} cannot intercept a wing on its own side",
                        wing_name(own)
                    ));
                }
                GroupObjective::Escort(target) if target.side != own.side || target == own => {
                    return refuse(format!(
                        "{} can only escort another wing on its own side",
                        wing_name(own)
                    ));
                }
                _ => {}
            }
        }
        for load in self.loadout.iter().chain(self.plane_loadouts.values()) {
            if !load.fuel_lbs.is_finite() || load.fuel_lbs < 0. {
                return refuse(format!("{} is not a fuel load", load.fuel_lbs));
            }
            if load.stations.len() > MAX_STATIONS {
                return refuse(format!(
                    "a loadout has at most {MAX_STATIONS} stations, not {}",
                    load.stations.len()
                ));
            }
            if let Some(bad) = load.stations.iter().find(|s| !resource_name(&s.weapon)) {
                return refuse(format!("`{}` is not a weapon resource name", bad.weapon));
            }
        }
        if let Some(plane) = self.plane_loadouts.keys().find(|p| **p >= MAX_PLANES) {
            return refuse(format!(
                "plane {plane} is beyond the mission's planes 0 to {}",
                MAX_PLANES - 1
            ));
        }
        Ok(())
    }

    /// The text form: the mission file of docs/DEDICATED-SERVER.md. Parsing
    /// it gives back an equal spec.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let mut line = |text: String| {
            out.push_str(&text);
            out.push('\n');
        };
        line(format!("tore-mission {FORMAT_VERSION}"));
        line(format!("theater {}", self.theater));
        line(format!("condition {}", self.condition.name()));
        if let Some([hour, minute]) = self.weather.time {
            line(format!("time-of-day {hour:02}:{minute:02}"));
        }
        if let Some([heading, speed]) = self.weather.wind {
            line(format!("wind {heading} {speed}"));
        }
        if let Some(deck) = self.weather.cloud_altitude {
            line(format!("cloud-deck {deck}"));
        }
        match self.start {
            Start::Airborne { altitude_ft } => line(format!("start airborne {altitude_ft}")),
            Start::Ground {
                runway,
                altitude_ft,
            } => {
                let ordinal = runway - RUNWAY_OBJECT_BASE;
                if altitude_ft == DEFAULT_ALTITUDE_FT {
                    line(format!("start ground {ordinal}"));
                } else {
                    line(format!("start ground {ordinal} {altitude_ft}"));
                }
            }
        }
        line(format!("separation-nm {}", self.separation_nm));
        line(format!("preset {}", self.preset.name()));
        line(format!("guns-only {}", yes_no(self.guns_only)));
        let absent = self.absent_wing();
        for (index, wing) in self.wings.iter().enumerate() {
            if index == 0 || *wing != absent {
                let id = Self::wing_id(index);
                line(format!(
                    "wing {} {} {} {} {}",
                    side_name(id.side),
                    id.display_number(),
                    wing.aircraft.selection_key(),
                    wing.count,
                    wing.skill.name()
                ));
            }
        }
        for (index, objective) in self.objectives.iter().enumerate() {
            if *objective != GroupObjective::Inherit {
                line(format!(
                    "objective {} {}",
                    wing_words(Self::wing_id(index)),
                    objective_words(*objective)
                ));
            }
        }
        for (index, survive) in self.must_survive.iter().enumerate() {
            if *survive {
                line(format!("survive {} yes", wing_words(Self::wing_id(index))));
            }
        }
        line(format!("cheats {}", cheats_words(&self.cheats)));
        // A networked mission's settings, written only when they differ
        // from the default, so a single-player spec's text is unchanged.
        if !self.friendly_fire {
            line("friendly-fire off".to_owned());
        }
        if self.cheat_loadouts {
            line("loadouts any".to_owned());
        }
        line(format!(
            "flight-model human {}",
            if self.researched_flight {
                "hybrid"
            } else {
                "legacy"
            }
        ));
        line(format!(
            "flight-model ai {}",
            match self.ai_flight_model {
                AiFlightModel::Standard => "standard",
                AiFlightModel::AllHybrid => "hybrid",
            }
        ));
        if let Some(skill) = self.enemy_skill {
            line(format!(
                "enemy-skill {}",
                match skill {
                    EnemySkillOverride::AllNovice => "novice",
                    EnemySkillOverride::AllAverage => "average",
                }
            ));
        }
        if self.fixture_wings {
            line("fixture-wings yes".to_owned());
        }
        if let Some(load) = &self.loadout {
            for text in load_lines(load) {
                line(format!("loadout {text}"));
            }
        }
        for (plane, load) in &self.plane_loadouts {
            for text in load_lines(load) {
                line(format!("plane-loadout {plane} {text}"));
            }
        }
        out
    }

    /// What a wing that has no line means: no aircraft, in the player's type.
    fn absent_wing(&self) -> WingSpec {
        WingSpec {
            aircraft: self.wings[0].aircraft,
            count: 0,
            skill: Skill::Novice,
        }
    }

    /// Reads the text form.
    pub fn from_text(text: &str) -> Result<Self, MissionError> {
        let mut parser = Parser::default();
        let mut first = true;
        for (number, raw) in text.lines().enumerate() {
            let number = number + 1;
            let content = raw.split('#').next().unwrap_or("").trim();
            if content.is_empty() {
                continue;
            }
            let words: Vec<&str> = content.split_whitespace().collect();
            if first {
                first = false;
                let header = if words.len() != 2 || words[0] != "tore-mission" {
                    Some("the first line must be `tore-mission 1`".to_owned())
                } else if words[1] != FORMAT_VERSION.to_string() {
                    Some(format!(
                        "this is mission file version {}, and only version {FORMAT_VERSION} is known",
                        words[1]
                    ))
                } else {
                    None
                };
                if let Some(message) = header {
                    return Err(MissionError(message).at_line(number));
                }
                continue;
            }
            parser.line(&words).map_err(|error| error.at_line(number))?;
        }
        if first {
            return refuse(
                "the mission file is empty: the first line must be `tore-mission 1`".into(),
            );
        }
        parser.finish()
    }
}

impl fmt::Display for MissionSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_text())
    }
}

/// Why a mission file or spec was refused, in words a person can act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionError(String);

impl MissionError {
    fn at_line(self, number: usize) -> Self {
        Self(format!("line {number}: {}", self.0))
    }
}

impl fmt::Display for MissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MissionError {}

type Parsed<T> = Result<T, MissionError>;

fn refuse<T>(message: String) -> Parsed<T> {
    Err(MissionError(message))
}

fn once<T>(slot: &mut Option<T>, what: &str, value: T) -> Parsed<()> {
    if slot.is_some() {
        return refuse(format!("{what} appears twice"));
    }
    *slot = Some(value);
    Ok(())
}

/// The arguments of a line that must have exactly `count` of them.
fn arguments<'w, 'a>(words: &'w [&'a str], count: usize, usage: &str) -> Parsed<&'w [&'a str]> {
    if words.len() != count + 1 {
        return refuse(format!("expected `{usage}`"));
    }
    Ok(&words[1..])
}

fn whole(word: &str, what: &str) -> Parsed<u32> {
    word.parse::<u32>()
        .or_else(|_| refuse(format!("{what} must be a whole number, not `{word}`")))
}

fn signed(word: &str, what: &str) -> Parsed<i32> {
    word.parse::<i32>()
        .or_else(|_| refuse(format!("{what} must be a whole number, not `{word}`")))
}

fn yes_no_word(word: &str, key: &str) -> Parsed<bool> {
    match word {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => refuse(format!("`{key}` takes yes or no, not `{word}`")),
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Friendly => "friendly",
        Side::Enemy => "enemy",
    }
}

fn wing_name(id: WingId) -> String {
    format!("{} wing {}", side_name(id.side), id.display_number())
}

fn wing_words(id: WingId) -> String {
    format!("{} {}", side_name(id.side), id.display_number())
}

fn join<T: fmt::Display>(values: impl IntoIterator<Item = T>) -> String {
    values
        .into_iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn resource_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'~' | b'$' | b'-'))
}

fn parse_side(word: &str) -> Parsed<Side> {
    match word {
        "friendly" => Ok(Side::Friendly),
        "enemy" => Ok(Side::Enemy),
        _ => refuse(format!("a side is `friendly` or `enemy`, not `{word}`")),
    }
}

/// A `SIDE N` pair as a position in the spec's wings.
fn parse_wing(side: &str, number: &str) -> Parsed<usize> {
    let side = parse_side(side)?;
    match whole(number, "a wing number")? {
        n @ 1..=3 => Ok(MissionSpec::wing_index(
            WingId::new(side, (n - 1) as u8).expect("a fixed wing"),
        )),
        n => refuse(format!("a wing number is 1 to 3, not {n}")),
    }
}

fn parse_aircraft(word: &str) -> Parsed<AircraftId> {
    match AircraftId::SELECTABLE
        .into_iter()
        .find(|id| id.selection_key() == word)
    {
        Some(id) => Ok(id),
        None => refuse(format!(
            "`{word}` is not an aircraft; the choices are {}",
            join(AircraftId::SELECTABLE.map(AircraftId::selection_key))
        )),
    }
}

fn objective_words(objective: GroupObjective) -> String {
    match objective {
        GroupObjective::Inherit => "inherit".into(),
        GroupObjective::Free => "free".into(),
        GroupObjective::Cap => "cap".into(),
        GroupObjective::Intercept(target) => format!("intercept {}", wing_words(target)),
        GroupObjective::Escort(target) => format!("escort {}", wing_words(target)),
        GroupObjective::SelfDefense => "self-defense".into(),
        GroupObjective::Hold => "hold".into(),
    }
}

/// The cheat switches, by the names the text form uses.
const CHEAT_FLAGS: [&str; 14] = [
    "unlimited-ammo",
    "unlimited-fuel",
    "no-spins",
    "no-turbulence",
    "extra-g",
    "ignore-weapon-weights",
    "no-sun-whiteout",
    "no-g-effects",
    "no-screen-shake",
    "no-crashes",
    "easy-aiming",
    "ignore-midair-collisions",
    "easy-targeting",
    "guns-only",
];

fn cheat_flag<'c>(cheats: &'c mut Cheats, name: &str) -> Option<&'c mut bool> {
    Some(match name {
        "unlimited-ammo" => &mut cheats.unlimited_ammo,
        "unlimited-fuel" => &mut cheats.unlimited_fuel,
        "no-spins" => &mut cheats.no_spins,
        "no-turbulence" => &mut cheats.no_turbulence,
        "extra-g" => &mut cheats.extra_g,
        "ignore-weapon-weights" => &mut cheats.ignore_weapon_weights,
        "no-sun-whiteout" => &mut cheats.no_sun_whiteout,
        "no-g-effects" => &mut cheats.no_g_effects,
        "no-screen-shake" => &mut cheats.no_screen_shake,
        "no-crashes" => &mut cheats.no_crashes,
        "easy-aiming" => &mut cheats.easy_aiming,
        "ignore-midair-collisions" => &mut cheats.ignore_midair_collisions,
        "easy-targeting" => &mut cheats.easy_targeting,
        "guns-only" => &mut cheats.guns_only,
        _ => return None,
    })
}

fn cheats_words(cheats: &Cheats) -> String {
    let mut copy = *cheats;
    let mut words: Vec<String> = CHEAT_FLAGS
        .iter()
        .filter(|name| cheat_flag(&mut copy, name).is_some_and(|flag| *flag))
        .map(|name| (*name).to_owned())
        .collect();
    match cheats.damage {
        Damage::Normal => {}
        Damage::Invulnerable => words.push("damage=invulnerable".into()),
        Damage::Realistic => words.push("damage=realistic".into()),
    }
    if let Some(level) = cheats.enemy_ai {
        words.push(format!("enemy-ai={}", experience_name(level)));
    }
    if words.is_empty() {
        "none".into()
    } else {
        words.join(" ")
    }
}

fn experience_name(level: Experience) -> &'static str {
    match level {
        Experience::Novice => "novice",
        Experience::Average => "average",
        Experience::Experienced => "experienced",
        Experience::Ace => "ace",
    }
}

/// What the parser has read so far.
#[derive(Default)]
struct Parser {
    theater: Option<String>,
    condition: Option<Condition>,
    time: Option<[i32; 2]>,
    wind: Option<[i32; 2]>,
    cloud_deck: Option<i32>,
    start: Option<Start>,
    separation: Option<u32>,
    preset: Option<Preset>,
    guns_only: Option<bool>,
    wings: [Option<WingSpec>; WINGS],
    objectives: [Option<GroupObjective>; WINGS],
    survive: [Option<bool>; WINGS],
    cheats: Option<Cheats>,
    human_model: Option<bool>,
    ai_model: Option<AiFlightModel>,
    enemy_skill: Option<Option<EnemySkillOverride>>,
    fixture_wings: Option<bool>,
    load: LoadParts,
    plane_loads: BTreeMap<u32, LoadParts>,
    friendly_fire: Option<bool>,
    cheat_loadouts: Option<bool>,
}

/// One loadout's lines as the parser has read them.
#[derive(Default)]
struct LoadParts {
    fuel: Option<f64>,
    cheat: Option<bool>,
    stations: Vec<Option<StationLoad>>,
}

impl LoadParts {
    fn is_empty(&self) -> bool {
        self.fuel.is_none() && self.cheat.is_none() && self.stations.is_empty()
    }

    /// Reads `fuel POUNDS`, `cheat yes/no` or `station N WEAPON COUNT
    /// QUANTITY`; `key` is the line's key, for the messages.
    fn line(&mut self, key: &str, words: &[&str]) -> Parsed<()> {
        match words.first().copied() {
            Some("fuel") => {
                let [pounds] = arguments(words, 1, &format!("{key} fuel POUNDS"))? else {
                    unreachable!("one argument")
                };
                let Some(pounds) = pounds
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite() && *v >= 0.)
                else {
                    return refuse(format!("`{pounds}` is not a fuel load"));
                };
                once(&mut self.fuel, &format!("`{key} fuel`"), pounds)
            }
            Some("cheat") => {
                let [value] = arguments(words, 1, &format!("{key} cheat yes/no"))? else {
                    unreachable!("one argument")
                };
                once(
                    &mut self.cheat,
                    &format!("`{key} cheat`"),
                    yes_no_word(value, &format!("{key} cheat"))?,
                )
            }
            Some("station") => {
                let [index, weapon, count, quantity] =
                    arguments(words, 4, &format!("{key} station N WEAPON COUNT QUANTITY"))?
                else {
                    unreachable!("four arguments")
                };
                let index = whole(index, "the station number")? as usize;
                if index >= MAX_STATIONS {
                    return refuse(format!("station {index} is beyond the aircraft's stations"));
                }
                if !resource_name(weapon) {
                    return refuse(format!("`{weapon}` is not a weapon resource name"));
                }
                let count = u16::try_from(whole(count, "the station's capacity")?)
                    .or_else(|_| refuse("the station's capacity is 0 to 65535".into()))?;
                let quantity = u16::try_from(whole(quantity, "the quantity")?)
                    .or_else(|_| refuse("the quantity is 0 to 65535".into()))?;
                if self.stations.len() <= index {
                    self.stations.resize(index + 1, None);
                }
                once(
                    &mut self.stations[index],
                    &format!("`{key} station {index}`"),
                    StationLoad {
                        weapon: (*weapon).to_owned(),
                        count,
                        quantity,
                    },
                )
            }
            _ => refuse(format!(
                "expected `{key} fuel`, `{key} cheat` or `{key} station`, each with its values"
            )),
        }
    }

    /// The loadout, once every line is read: the fuel and every station from
    /// 0 up must be there.
    fn finish(self, key: &str) -> Parsed<LoadoutSpec> {
        let Some(fuel_lbs) = self.fuel else {
            return refuse(format!("the loadout has no `{key} fuel` line"));
        };
        let mut stations = Vec::new();
        for (index, station) in self.stations.into_iter().enumerate() {
            let Some(station) = station else {
                return refuse(format!("the loadout has no `{key} station {index}` line"));
            };
            stations.push(station);
        }
        Ok(LoadoutSpec {
            fuel_lbs,
            cheat: self.cheat.unwrap_or(false),
            stations,
        })
    }
}

/// A loadout's lines after their key: the fuel, the cheat and each station.
fn load_lines(load: &LoadoutSpec) -> Vec<String> {
    let mut lines = vec![
        format!("fuel {}", load.fuel_lbs),
        format!("cheat {}", yes_no(load.cheat)),
    ];
    for (index, station) in load.stations.iter().enumerate() {
        lines.push(format!(
            "station {index} {} {} {}",
            station.weapon, station.count, station.quantity
        ));
    }
    lines
}

impl Parser {
    fn line(&mut self, words: &[&str]) -> Parsed<()> {
        let key = words[0];
        match key {
            "tore-mission" => refuse("`tore-mission` is only the first line".into()),
            "theater" => {
                let [code] = arguments(words, 1, "theater CODE")? else {
                    unreachable!("one argument")
                };
                if !THEATERS.contains(code) {
                    return refuse(format!(
                        "`{code}` is not a theater; the choices are {}",
                        THEATERS.join(", ")
                    ));
                }
                once(&mut self.theater, "`theater`", (*code).to_owned())
            }
            "condition" => {
                let [name] = arguments(words, 1, "condition NAME")? else {
                    unreachable!("one argument")
                };
                match Condition::parse(name) {
                    Some(condition) => once(&mut self.condition, "`condition`", condition),
                    None => refuse(format!(
                        "`{name}` is not a condition; the choices are {}",
                        CONDITION_NAMES.join(", ")
                    )),
                }
            }
            "time-of-day" => {
                let [text] = arguments(words, 1, "time-of-day HH:MM")? else {
                    unreachable!("one argument")
                };
                let time = text
                    .split_once(':')
                    .and_then(|(h, m)| Some([h.parse::<i32>().ok()?, m.parse::<i32>().ok()?]))
                    .filter(|[h, m]| (0..24).contains(h) && (0..60).contains(m));
                match time {
                    Some(time) => once(&mut self.time, "`time-of-day`", time),
                    None => refuse(format!("`{text}` is not a time of day, write HH:MM")),
                }
            }
            "wind" => {
                let [heading, speed] = arguments(words, 2, "wind HEADING FEET-PER-SECOND")? else {
                    unreachable!("two arguments")
                };
                let heading = signed(heading, "the wind heading")?;
                let speed = signed(speed, "the wind speed")?;
                if !(-360..=360).contains(&heading) || !(0..=200).contains(&speed) {
                    return refuse(
                        "the wind heading is -360 to 360 degrees and its speed 0 to 200 feet a second"
                            .into(),
                    );
                }
                once(&mut self.wind, "`wind`", [heading, speed])
            }
            "cloud-deck" => {
                let [feet] = arguments(words, 1, "cloud-deck FEET")? else {
                    unreachable!("one argument")
                };
                let feet = signed(feet, "the cloud deck")?;
                if !(0..=400_000).contains(&feet) {
                    return refuse("the cloud deck is 0 to 400000 feet".into());
                }
                once(&mut self.cloud_deck, "`cloud-deck`", feet)
            }
            "start" => self.start(words),
            "separation-nm" => {
                let [nm] = arguments(words, 1, "separation-nm N")? else {
                    unreachable!("one argument")
                };
                let nm = whole(nm, "the separation")?;
                if !SEPARATION_NM.contains(&f64::from(nm)) {
                    return refuse(format!(
                        "the separation is one of {} nautical miles, not {nm}",
                        join(SEPARATION_NM.map(|nm| nm as u32))
                    ));
                }
                once(&mut self.separation, "`separation-nm`", nm)
            }
            "preset" => {
                let [name] = arguments(words, 1, "preset NAME")? else {
                    unreachable!("one argument")
                };
                match name.parse::<Preset>() {
                    Ok(preset) => once(&mut self.preset, "`preset`", preset),
                    Err(_) => refuse(format!(
                        "`{name}` is not a preset; the choices are {}",
                        join(Preset::ALL.map(Preset::name))
                    )),
                }
            }
            "guns-only" => {
                let [value] = arguments(words, 1, "guns-only yes/no")? else {
                    unreachable!("one argument")
                };
                once(&mut self.guns_only, "`guns-only`", yes_no_word(value, key)?)
            }
            "wing" => self.wing(words),
            "objective" => self.objective(words),
            "survive" => {
                let [side, number, value] = arguments(words, 3, "survive SIDE N yes/no")? else {
                    unreachable!("three arguments")
                };
                let index = parse_wing(side, number)?;
                let what = format!("`survive {side} {number}`");
                once(&mut self.survive[index], &what, yes_no_word(value, key)?)
            }
            "cheats" => {
                if words.len() < 2 {
                    return refuse("expected `cheats none` or `cheats` and a list".into());
                }
                let cheats = Self::cheats(&words[1..])?;
                once(&mut self.cheats, "`cheats`", cheats)
            }
            "flight-model" => self.flight_model(words),
            "enemy-skill" => {
                let [name] = arguments(words, 1, "enemy-skill novice/average/none")? else {
                    unreachable!("one argument")
                };
                let skill = match *name {
                    "novice" => Some(EnemySkillOverride::AllNovice),
                    "average" => Some(EnemySkillOverride::AllAverage),
                    "none" => None,
                    _ => {
                        return refuse(format!(
                            "`{name}` is not an enemy skill; the choices are novice, average and none"
                        ));
                    }
                };
                once(&mut self.enemy_skill, "`enemy-skill`", skill)
            }
            "fixture-wings" => {
                let [value] = arguments(words, 1, "fixture-wings yes/no")? else {
                    unreachable!("one argument")
                };
                once(
                    &mut self.fixture_wings,
                    "`fixture-wings`",
                    yes_no_word(value, key)?,
                )
            }
            "loadout" => self.loadout(words),
            "plane-loadout" => self.plane_loadout(words),
            "friendly-fire" => {
                let [value] = arguments(words, 1, "friendly-fire on/off")? else {
                    unreachable!("one argument")
                };
                let on = match *value {
                    "on" => true,
                    "off" => false,
                    other => {
                        return refuse(format!("`friendly-fire` is `on` or `off`, not `{other}`"));
                    }
                };
                once(&mut self.friendly_fire, "`friendly-fire`", on)
            }
            "loadouts" => {
                let [value] = arguments(words, 1, "loadouts own/any")? else {
                    unreachable!("one argument")
                };
                let any = match *value {
                    "own" => false,
                    "any" => true,
                    other => {
                        return refuse(format!("`loadouts` is `own` or `any`, not `{other}`"));
                    }
                };
                once(&mut self.cheat_loadouts, "`loadouts`", any)
            }
            _ => refuse(format!("unknown setting `{key}`")),
        }
    }

    fn start(&mut self, words: &[&str]) -> Parsed<()> {
        let start = match words.get(1).copied() {
            Some("airborne") => {
                let [_, feet] = arguments(words, 2, "start airborne FEET")? else {
                    unreachable!("two arguments")
                };
                let altitude_ft = whole(feet, "the altitude")?;
                if !ALTITUDES_FT.contains(&altitude_ft) {
                    return refuse(format!(
                        "the airborne altitude is one of {} feet, not {altitude_ft}",
                        join(ALTITUDES_FT)
                    ));
                }
                Start::Airborne { altitude_ft }
            }
            Some("ground") => {
                if words.len() != 3 && words.len() != 4 {
                    return refuse("expected `start ground RUNWAY [FEET]`".into());
                }
                let ordinal = whole(words[2], "the runway number")?;
                if ordinal >= RUNWAY_OBJECT_BASE {
                    return refuse(format!("{ordinal} is not a runway number"));
                }
                let altitude_ft = match words.get(3) {
                    Some(feet) => whole(feet, "the altitude")?,
                    None => DEFAULT_ALTITUDE_FT,
                };
                if !ALTITUDES_FT.contains(&altitude_ft) {
                    return refuse(format!(
                        "the altitude is one of {} feet, not {altitude_ft}",
                        join(ALTITUDES_FT)
                    ));
                }
                Start::Ground {
                    runway: RUNWAY_OBJECT_BASE + ordinal,
                    altitude_ft,
                }
            }
            _ => return refuse("expected `start airborne FEET` or `start ground RUNWAY`".into()),
        };
        once(&mut self.start, "`start`", start)
    }

    fn wing(&mut self, words: &[&str]) -> Parsed<()> {
        let [side, number, aircraft, count, skill] =
            arguments(words, 5, "wing SIDE N AIRCRAFT COUNT SKILL")?
        else {
            unreachable!("five arguments")
        };
        let index = parse_wing(side, number)?;
        let aircraft = parse_aircraft(aircraft)?;
        let count = whole(count, "the aircraft count")? as usize;
        let least = usize::from(index == 0);
        if !(least..=MAX_WING).contains(&count) {
            return refuse(format!(
                "{} holds {least} to {MAX_WING} aircraft, not {count}",
                if index == 0 {
                    "friendly wing 1 (with the player)"
                } else {
                    "a wing"
                }
            ));
        }
        let Some(skill) = Skill::parse(skill) else {
            return refuse(format!(
                "`{skill}` is not a skill; the choices are {}",
                join(Skill::ALL.map(Skill::name))
            ));
        };
        let what = wing_name(MissionSpec::wing_id(index));
        once(
            &mut self.wings[index],
            &what,
            WingSpec {
                aircraft,
                count,
                skill,
            },
        )
    }

    fn objective(&mut self, words: &[&str]) -> Parsed<()> {
        if words.len() < 4 {
            return refuse("expected `objective SIDE N NAME [SIDE N]`".into());
        }
        let index = parse_wing(words[1], words[2])?;
        let own = MissionSpec::wing_id(index);
        let name = words[3];
        let rest = &words[4..];
        let target = || -> Parsed<WingId> {
            match rest {
                [side, number] => Ok(MissionSpec::wing_id(parse_wing(side, number)?)),
                _ => refuse(format!("`{name}` needs a wing: `{name} SIDE N`")),
            }
        };
        let objective = match name {
            "inherit" | "free" | "cap" | "self-defense" | "hold" if !rest.is_empty() => {
                return refuse(format!("`{name}` takes no wing"));
            }
            "inherit" => GroupObjective::Inherit,
            "free" => GroupObjective::Free,
            "cap" => GroupObjective::Cap,
            "self-defense" => GroupObjective::SelfDefense,
            "hold" => GroupObjective::Hold,
            "intercept" => {
                let target = target()?;
                if target.side == own.side {
                    return refuse(format!(
                        "{} cannot intercept a wing on its own side",
                        wing_name(own)
                    ));
                }
                GroupObjective::Intercept(target)
            }
            "escort" => {
                let target = target()?;
                if target.side != own.side || target == own {
                    return refuse(format!(
                        "{} can only escort another wing on its own side",
                        wing_name(own)
                    ));
                }
                GroupObjective::Escort(target)
            }
            other => {
                return refuse(format!(
                    "`{other}` is not an objective; the choices are inherit, free, cap, intercept, escort, self-defense and hold"
                ));
            }
        };
        once(
            &mut self.objectives[index],
            &format!("the objective of {}", wing_name(own)),
            objective,
        )
    }

    fn cheats(words: &[&str]) -> Parsed<Cheats> {
        let mut cheats = Cheats::default();
        if words == ["none"] {
            return Ok(cheats);
        }
        let mut seen: Vec<&str> = Vec::new();
        for &word in words {
            if word == "none" {
                return refuse("`none` stands alone".into());
            }
            if seen.contains(&word) {
                return refuse(format!("the cheat `{word}` is listed twice"));
            }
            seen.push(word);
            if let Some(flag) = cheat_flag(&mut cheats, word) {
                *flag = true;
            } else if let Some(level) = word.strip_prefix("damage=") {
                cheats.damage = match level {
                    "invulnerable" => Damage::Invulnerable,
                    "realistic" => Damage::Realistic,
                    _ => {
                        return refuse(format!(
                            "`{level}` is not a damage cheat; the choices are invulnerable and realistic"
                        ));
                    }
                };
            } else if let Some(level) = word.strip_prefix("enemy-ai=") {
                let Some(level) = Experience::ALL
                    .into_iter()
                    .find(|e| experience_name(*e) == level)
                else {
                    return refuse(format!(
                        "`{level}` is not an enemy level; the choices are novice, average, experienced and ace"
                    ));
                };
                cheats.enemy_ai = Some(level);
            } else {
                return refuse(format!(
                    "`{word}` is not a cheat; the choices are none, {}, damage=invulnerable, damage=realistic and enemy-ai=LEVEL",
                    CHEAT_FLAGS.join(", ")
                ));
            }
        }
        Ok(cheats)
    }

    fn flight_model(&mut self, words: &[&str]) -> Parsed<()> {
        let [who, model] = arguments(words, 2, "flight-model human/ai MODEL")? else {
            unreachable!("two arguments")
        };
        match (*who, *model) {
            ("human", "hybrid") => once(&mut self.human_model, "`flight-model human`", true),
            ("human", "legacy") => once(&mut self.human_model, "`flight-model human`", false),
            ("ai", "standard") => once(
                &mut self.ai_model,
                "`flight-model ai`",
                AiFlightModel::Standard,
            ),
            ("ai", "hybrid") => once(
                &mut self.ai_model,
                "`flight-model ai`",
                AiFlightModel::AllHybrid,
            ),
            ("human", _) => refuse(format!(
                "`{model}` is not a flight model for humans; the choices are hybrid and legacy"
            )),
            ("ai", _) => refuse(format!(
                "`{model}` is not a flight model for the AI; the choices are standard and hybrid"
            )),
            _ => refuse(format!(
                "`{who}` is not a flight model owner; the choices are human and ai"
            )),
        }
    }

    fn loadout(&mut self, words: &[&str]) -> Parsed<()> {
        self.load.line("loadout", &words[1..])
    }

    fn plane_loadout(&mut self, words: &[&str]) -> Parsed<()> {
        let Some(plane) = words.get(1) else {
            return refuse("expected `plane-loadout PLANE fuel`, `cheat` or `station`".into());
        };
        let plane = whole(plane, "the plane number")?;
        if plane >= MAX_PLANES {
            return refuse(format!(
                "plane {plane} is beyond the mission's planes 0 to {}",
                MAX_PLANES - 1
            ));
        }
        self.plane_loads
            .entry(plane)
            .or_default()
            .line(&format!("plane-loadout {plane}"), &words[2..])
    }

    fn finish(self) -> Parsed<MissionSpec> {
        let Some(theater) = self.theater else {
            return refuse("the mission has no `theater` line".into());
        };
        let Some(first) = self.wings[0] else {
            return refuse("the mission has no `wing friendly 1` line".into());
        };
        let mut spec = MissionSpec::new(&theater, first.aircraft);
        let absent = spec.absent_wing();
        for (index, wing) in self.wings.into_iter().enumerate() {
            spec.wings[index] = wing.unwrap_or(absent);
        }
        if let Some(condition) = self.condition {
            spec.condition = condition;
        }
        spec.weather = Overrides {
            time: self.time,
            wind: self.wind,
            cloud_altitude: self.cloud_deck,
        };
        if let Some(start) = self.start {
            spec.start = start;
        }
        if let Some(nm) = self.separation {
            spec.separation_nm = nm;
        }
        if let Some(preset) = self.preset {
            spec.preset = preset;
        }
        spec.guns_only = self.guns_only.unwrap_or(false);
        for index in 0..WINGS {
            spec.objectives[index] = self.objectives[index].unwrap_or(GroupObjective::Inherit);
            spec.must_survive[index] = self.survive[index].unwrap_or(false);
        }
        spec.cheats = self.cheats.unwrap_or_default();
        spec.researched_flight = self.human_model.unwrap_or(true);
        spec.ai_flight_model = self.ai_model.unwrap_or(AiFlightModel::AllHybrid);
        spec.enemy_skill = self.enemy_skill.unwrap_or(None);
        spec.fixture_wings = self.fixture_wings.unwrap_or(false);
        spec.friendly_fire = self.friendly_fire.unwrap_or(true);
        spec.cheat_loadouts = self.cheat_loadouts.unwrap_or(false);
        if !self.load.is_empty() {
            spec.loadout = Some(self.load.finish("loadout")?);
        }
        for (plane, load) in self.plane_loads {
            let key = format!("plane-loadout {plane}");
            spec.plane_loadouts.insert(plane, load.finish(&key)?);
        }
        spec.validate()?;
        Ok(spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first fenced `text` block under "The mission file" in the guide.
    fn guide_example() -> String {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/DEDICATED-SERVER.md"
        );
        // A Windows checkout may turn the guide's line ends into CR LF.
        let guide = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
        let section = guide.split("\n## The mission file").nth(1).unwrap();
        let block = section.split("```text\n").nth(1).unwrap();
        block.split("```").next().unwrap().to_owned()
    }

    #[test]
    fn the_guides_example_parses() {
        let spec = MissionSpec::from_text(&guide_example()).unwrap();
        assert_eq!(spec.theater, "UKR");
        assert_eq!(spec.condition, Condition::Clear);
        assert_eq!(
            spec.start,
            Start::Airborne {
                altitude_ft: 20_000
            }
        );
        assert_eq!(spec.separation_nm, 20);
        assert_eq!(spec.preset, Preset::Free);
        assert!(!spec.guns_only);
        let wing = |aircraft, count, skill| WingSpec {
            aircraft,
            count,
            skill,
        };
        assert_eq!(spec.wings[0], wing(AircraftId::F18, 4, Skill::Experienced));
        assert_eq!(spec.wings[1], wing(AircraftId::F14, 2, Skill::Average));
        assert_eq!(spec.wings[2].count, 0);
        assert_eq!(
            spec.wings[3],
            wing(AircraftId::Mig29, 4, Skill::Experienced)
        );
        assert_eq!(spec.wings[4], wing(AircraftId::Su27, 2, Skill::Ace));
        assert_eq!(spec.wings[5].count, 0);
        assert_eq!(spec.must_survive, [false, true, false, false, false, false]);
        assert_eq!(
            spec.objectives[4],
            GroupObjective::Intercept(MissionSpec::wing_id(0))
        );
        assert_eq!(spec.cheats, Cheats::default());
        // The defaults of what the example leaves out: the hybrid model for
        // everyone, as the guide says, and no loadout.
        assert!(spec.researched_flight);
        assert_eq!(spec.ai_flight_model, AiFlightModel::AllHybrid);
        assert_eq!(spec.loadout, None);
    }

    #[test]
    fn the_guides_example_round_trips_through_the_text_form() {
        let spec = MissionSpec::from_text(&guide_example()).unwrap();
        let text = spec.to_text();
        assert_eq!(MissionSpec::from_text(&text).unwrap(), spec);
        // The text is stable: writing what was read changes nothing.
        assert_eq!(MissionSpec::from_text(&text).unwrap().to_text(), text);
        assert!(text.starts_with("tore-mission 1\n"));
    }

    fn busy() -> MissionSpec {
        let mut spec = MissionSpec::new("KURILE", AircraftId::Rafale);
        spec.condition = Condition::Night;
        spec.weather = Overrides {
            time: Some([5, 7]),
            wind: Some([-90, 45]),
            cloud_altitude: Some(12_000),
        };
        spec.start = Start::Ground {
            runway: RUNWAY_OBJECT_BASE + 3,
            altitude_ft: 10_000,
        };
        spec.separation_nm = 300;
        spec.preset = Preset::SelfDefense;
        spec.guns_only = true;
        spec.wings = [
            WingSpec {
                aircraft: AircraftId::Rafale,
                count: 5,
                skill: Skill::Dummy,
            },
            WingSpec {
                aircraft: AircraftId::Faxx,
                count: 1,
                skill: Skill::Ace,
            },
            WingSpec {
                aircraft: AircraftId::X31,
                count: 3,
                skill: Skill::Novice,
            },
            WingSpec {
                aircraft: AircraftId::Su35,
                count: 5,
                skill: Skill::Experienced,
            },
            // A wing with no aircraft keeps its type and skill.
            WingSpec {
                aircraft: AircraftId::F22n,
                count: 0,
                skill: Skill::Ace,
            },
            WingSpec {
                aircraft: AircraftId::Rafale,
                count: 2,
                skill: Skill::Novice,
            },
        ];
        spec.objectives = [
            GroupObjective::Escort(MissionSpec::wing_id(1)),
            GroupObjective::Cap,
            GroupObjective::SelfDefense,
            GroupObjective::Intercept(MissionSpec::wing_id(2)),
            GroupObjective::Hold,
            GroupObjective::Free,
        ];
        spec.must_survive = [true, false, true, false, true, true];
        spec.cheats = Cheats {
            damage: Damage::Realistic,
            unlimited_ammo: true,
            unlimited_fuel: true,
            no_spins: true,
            no_turbulence: true,
            extra_g: true,
            ignore_weapon_weights: true,
            no_sun_whiteout: true,
            no_g_effects: true,
            no_screen_shake: true,
            no_crashes: true,
            easy_aiming: true,
            ignore_midair_collisions: true,
            easy_targeting: true,
            guns_only: true,
            enemy_ai: Some(Experience::Experienced),
        };
        spec.researched_flight = false;
        spec.ai_flight_model = AiFlightModel::Standard;
        spec.enemy_skill = Some(EnemySkillOverride::AllNovice);
        spec.fixture_wings = true;
        spec.friendly_fire = false;
        spec.cheat_loadouts = true;
        spec.loadout = Some(LoadoutSpec {
            fuel_lbs: 4_500.5,
            cheat: true,
            stations: vec![
                StationLoad {
                    weapon: "DEFA.JT".into(),
                    count: 600,
                    quantity: 450,
                },
                StationLoad {
                    weapon: "AIM9X.JT".into(),
                    count: 2,
                    quantity: 0,
                },
            ],
        });
        spec
    }

    #[test]
    fn every_setting_round_trips_through_the_text_form() {
        let spec = busy();
        spec.validate().unwrap();
        let text = spec.to_text();
        assert_eq!(MissionSpec::from_text(&text).unwrap(), spec, "{text}");
    }

    #[test]
    fn every_choice_of_each_setting_round_trips() {
        let mut specs = Vec::new();
        for theater in THEATERS {
            specs.push(MissionSpec::new(theater, AircraftId::F18));
        }
        for aircraft in AircraftId::SELECTABLE {
            let mut spec = MissionSpec::new("UKR", aircraft);
            for wing in &mut spec.wings {
                wing.aircraft = aircraft;
            }
            specs.push(spec);
        }
        for condition in Condition::ALL {
            let mut spec = MissionSpec::new("UKR", AircraftId::F18);
            spec.condition = condition;
            specs.push(spec);
        }
        for skill in Skill::ALL {
            let mut spec = MissionSpec::new("UKR", AircraftId::F18);
            spec.wings[0].skill = skill;
            spec.wings[5].skill = skill;
            spec.wings[5].count = 1;
            specs.push(spec);
        }
        for preset in Preset::ALL {
            let mut spec = MissionSpec::new("UKR", AircraftId::F18);
            spec.preset = preset;
            specs.push(spec);
        }
        for altitude_ft in ALTITUDES_FT {
            let mut spec = MissionSpec::new("UKR", AircraftId::F18);
            spec.start = Start::Airborne { altitude_ft };
            specs.push(spec);
            let mut spec = MissionSpec::new("UKR", AircraftId::F18);
            spec.start = Start::Ground {
                runway: RUNWAY_OBJECT_BASE + 7,
                altitude_ft,
            };
            specs.push(spec);
        }
        for nm in SEPARATION_NM {
            let mut spec = MissionSpec::new("UKR", AircraftId::F18);
            spec.separation_nm = nm as u32;
            specs.push(spec);
        }
        for count in 0..=MAX_WING {
            let mut spec = MissionSpec::new("UKR", AircraftId::F18);
            spec.wings[1].count = count;
            spec.wings[0].count = count.max(1);
            specs.push(spec);
        }
        for damage in [Damage::Invulnerable, Damage::Normal, Damage::Realistic] {
            for enemy in [None, Some(Experience::Novice), Some(Experience::Ace)] {
                let mut spec = MissionSpec::new("UKR", AircraftId::F18);
                spec.cheats.damage = damage;
                spec.cheats.enemy_ai = enemy;
                specs.push(spec);
            }
        }
        for skill in [None, Some(EnemySkillOverride::AllAverage)] {
            let mut spec = MissionSpec::new("UKR", AircraftId::F18);
            spec.enemy_skill = skill;
            specs.push(spec);
        }
        for ai in [AiFlightModel::Standard, AiFlightModel::AllHybrid] {
            for human in [true, false] {
                let mut spec = MissionSpec::new("UKR", AircraftId::F18);
                spec.ai_flight_model = ai;
                spec.researched_flight = human;
                specs.push(spec);
            }
        }
        for index in 0..WINGS {
            let own = MissionSpec::wing_id(index);
            let mut objectives = vec![
                GroupObjective::Inherit,
                GroupObjective::Free,
                GroupObjective::Cap,
                GroupObjective::SelfDefense,
                GroupObjective::Hold,
            ];
            for other in 0..WINGS {
                let target = MissionSpec::wing_id(other);
                if target.side == own.side && target != own {
                    objectives.push(GroupObjective::Escort(target));
                } else if target.side != own.side {
                    objectives.push(GroupObjective::Intercept(target));
                }
            }
            for objective in objectives {
                let mut spec = MissionSpec::new("UKR", AircraftId::F18);
                spec.objectives[index] = objective;
                spec.must_survive[index] = true;
                specs.push(spec);
            }
        }
        for spec in specs {
            let text = spec.to_text();
            assert_eq!(MissionSpec::from_text(&text).unwrap(), spec, "{text}");
        }
    }

    #[test]
    fn comments_blank_lines_and_spacing_do_not_matter() {
        let spec = MissionSpec::from_text(
            "# a mission\n\n  tore-mission   1  # the version\ntheater UKR\n\twing  friendly 1 F18.PT 1 ace # me\n\n",
        )
        .unwrap();
        assert_eq!(spec.wings[0].skill, Skill::Ace);
        assert_eq!(spec.start, Start::Airborne { altitude_ft: 5_000 });
        assert_eq!(spec.condition, Condition::Clear);
    }

    /// The error a text produces.
    fn refused(text: &str) -> String {
        match MissionSpec::from_text(text) {
            Ok(spec) => panic!("parsed: {spec:?}"),
            Err(error) => error.to_string(),
        }
    }

    const BASE: &str = "tore-mission 1\ntheater UKR\nwing friendly 1 F18.PT 1 ace\n";

    #[test]
    fn a_refusal_names_the_line_and_what_is_wrong() {
        let with = |line: &str| format!("{BASE}{line}\n");
        let cases = [
            ("theater MOON", "line 4: `MOON` is not a theater"),
            ("theater UKR", "line 4: `theater` appears twice"),
            ("condition stormy", "line 4: `stormy` is not a condition"),
            (
                "wing friendly 2 F99.PT 1 ace",
                "line 4: `F99.PT` is not an aircraft",
            ),
            (
                "wing friendly 2 F22N.PT 1 legend",
                "line 4: `legend` is not a skill",
            ),
            (
                "wing friendly 2 F22N.PT 6 ace",
                "line 4: a wing holds 0 to 5 aircraft, not 6",
            ),
            (
                "wing friendly 2 F22N.PT x ace",
                "line 4: the aircraft count must be a whole number",
            ),
            (
                "wing friendly 4 F22N.PT 1 ace",
                "line 4: a wing number is 1 to 3, not 4",
            ),
            (
                "wing neutral 1 F22N.PT 1 ace",
                "line 4: a side is `friendly` or `enemy`",
            ),
            (
                "wing friendly 1 F14.PT 1 ace",
                "line 4: friendly wing 1 appears twice",
            ),
            (
                "wing friendly 2 F22N.PT 1",
                "line 4: expected `wing SIDE N AIRCRAFT COUNT SKILL`",
            ),
            (
                "start airborne 12345",
                "line 4: the airborne altitude is one of 5000, 10000, 20000, 40000 feet",
            ),
            (
                "start ground",
                "line 4: expected `start ground RUNWAY [FEET]`",
            ),
            (
                "start sideways 5",
                "line 4: expected `start airborne FEET` or `start ground RUNWAY`",
            ),
            (
                "separation-nm 7",
                "line 4: the separation is one of 1, 2, 5, 10, 20, 50, 100, 150, 200, 300",
            ),
            ("preset boring", "line 4: `boring` is not a preset"),
            ("guns-only maybe", "line 4: `guns-only` takes yes or no"),
            ("time-of-day 25:00", "line 4: `25:00` is not a time of day"),
            ("time-of-day noon", "line 4: `noon` is not a time of day"),
            (
                "wind 90 500",
                "line 4: the wind heading is -360 to 360 degrees and its speed 0 to 200",
            ),
            (
                "cloud-deck 999999",
                "line 4: the cloud deck is 0 to 400000 feet",
            ),
            (
                "objective enemy 1 intercept enemy 2",
                "line 4: enemy wing 1 cannot intercept a wing on its own side",
            ),
            (
                "objective enemy 1 escort enemy 1",
                "line 4: enemy wing 1 can only escort another wing on its own side",
            ),
            (
                "objective enemy 1 escort friendly 1",
                "line 4: enemy wing 1 can only escort another wing on its own side",
            ),
            (
                "objective enemy 1 cap friendly 1",
                "line 4: `cap` takes no wing",
            ),
            (
                "objective enemy 1 intercept",
                "line 4: `intercept` needs a wing",
            ),
            (
                "objective enemy 1 dance",
                "line 4: `dance` is not an objective",
            ),
            (
                "survive enemy 1 sometimes",
                "line 4: `survive` takes yes or no",
            ),
            (
                "cheats infinite-lives",
                "line 4: `infinite-lives` is not a cheat",
            ),
            ("cheats none no-spins", "line 4: `none` stands alone"),
            (
                "cheats no-spins no-spins",
                "line 4: the cheat `no-spins` is listed twice",
            ),
            (
                "cheats damage=fatal",
                "line 4: `fatal` is not a damage cheat",
            ),
            ("cheats enemy-ai=god", "line 4: `god` is not an enemy level"),
            (
                "flight-model human quantum",
                "line 4: `quantum` is not a flight model for humans",
            ),
            (
                "flight-model ai legacy",
                "line 4: `legacy` is not a flight model for the AI",
            ),
            ("enemy-skill ace", "line 4: `ace` is not an enemy skill"),
            (
                "fixture-wings sure",
                "line 4: `fixture-wings` takes yes or no",
            ),
            ("loadout fuel lots", "line 4: `lots` is not a fuel load"),
            (
                "loadout station 0 M61.JT 500",
                "line 4: expected `loadout station N WEAPON COUNT QUANTITY`",
            ),
            (
                "loadout station 0 M 61.JT 500 500",
                "line 4: expected `loadout station N WEAPON COUNT QUANTITY`",
            ),
            (
                "loadout station 0 M61/JT 500 500",
                "line 4: `M61/JT` is not a weapon resource name",
            ),
            (
                "loadout station 0 M61.JT 70000 1",
                "line 4: the station's capacity is 0 to 65535",
            ),
            ("loadout paint red", "line 4: expected `loadout fuel`"),
            ("colour blue", "line 4: unknown setting `colour`"),
            (
                "tore-mission 1",
                "line 4: `tore-mission` is only the first line",
            ),
        ];
        for (line, expected) in cases {
            let error = refused(&with(line));
            assert!(
                error.starts_with(expected),
                "{line:?} gave {error:?}, expected {expected:?}"
            );
        }
    }

    #[test]
    fn a_file_without_its_header_version_or_essentials_is_refused() {
        assert!(refused("").contains("the mission file is empty"));
        assert!(refused("# nothing\n").contains("the mission file is empty"));
        assert_eq!(
            refused("theater UKR\n"),
            "line 1: the first line must be `tore-mission 1`"
        );
        assert_eq!(
            refused("tore-mission 2\ntheater UKR\n"),
            "line 1: this is mission file version 2, and only version 1 is known"
        );
        assert_eq!(
            refused("tore-mission 1\nwing friendly 1 F18.PT 1 ace\n"),
            "the mission has no `theater` line"
        );
        assert_eq!(
            refused("tore-mission 1\ntheater UKR\n"),
            "the mission has no `wing friendly 1` line"
        );
        assert_eq!(
            refused("tore-mission 1\ntheater UKR\nwing friendly 1 F18.PT 0 ace\n"),
            "line 3: friendly wing 1 (with the player) holds 1 to 5 aircraft, not 0"
        );
    }

    #[test]
    fn the_loadout_needs_every_station_and_its_fuel() {
        let base = format!("{BASE}loadout station 1 M61.JT 500 500\n");
        assert_eq!(refused(&base), "the loadout has no `loadout fuel` line");
        assert_eq!(
            refused(&format!("{base}loadout fuel 100\n")),
            "the loadout has no `loadout station 0` line"
        );
        let spec = MissionSpec::from_text(&format!(
            "{base}loadout fuel 100\nloadout station 0 M61.JT 500 500\n"
        ))
        .unwrap();
        assert_eq!(spec.loadout.unwrap().stations.len(), 2);
    }

    #[test]
    fn a_ground_start_names_its_runway_by_ordinal() {
        let text = format!("{BASE}start ground 12\n");
        let spec = MissionSpec::from_text(&text).unwrap();
        assert_eq!(spec.ground_runway(), Some(RUNWAY_OBJECT_BASE + 12));
        assert_eq!(spec.start.altitude_ft(), DEFAULT_ALTITUDE_FT);
        assert!(spec.to_text().contains("start ground 12\n"));
        let high = MissionSpec::from_text(&format!("{BASE}start ground 12 20000\n")).unwrap();
        assert_eq!(high.start.altitude_ft(), 20_000);
        assert!(high.to_text().contains("start ground 12 20000\n"));
    }

    #[test]
    fn a_spec_built_in_code_is_validated_like_a_parsed_one() {
        let mut spec = MissionSpec::new("UKR", AircraftId::F18);
        spec.validate().unwrap();
        spec.wings[0].count = 0;
        assert!(
            spec.validate()
                .unwrap_err()
                .to_string()
                .contains("friendly wing 1")
        );
        spec.wings[0].count = 1;
        spec.separation_nm = 3;
        assert!(spec.validate().is_err());
        spec.separation_nm = 5;
        spec.weather.time = Some([24, 0]);
        assert!(spec.validate().is_err());
        spec.weather.time = None;
        spec.objectives[0] = GroupObjective::Intercept(MissionSpec::wing_id(1));
        assert!(spec.validate().is_err());
        spec.objectives[0] = GroupObjective::Inherit;
        spec.theater = "no way".into();
        assert!(spec.validate().is_err());
        // A `~` layout variant is for the game's developer options.
        spec.theater = "~UKR1".into();
        spec.validate().unwrap();
    }

    #[test]
    fn friendly_wing_one_gives_a_slot_to_the_player() {
        let mut spec = MissionSpec::new("UKR", AircraftId::F18);
        spec.wings[0].count = 4;
        spec.wings[3].count = 2;
        let launches = spec.wing_launches().unwrap();
        let counts: Vec<usize> = launches.iter().map(|wing| wing.members.len()).collect();
        assert_eq!(counts, [3, 0, 0, 2, 0, 0]);
        assert_eq!(spec.player_wing_size(), 4);
        assert_eq!(spec.player(), AircraftId::F18);
    }

    #[test]
    fn friendly_fire_and_the_loadout_rule_are_lines_only_when_not_the_default() {
        // Slice F2-1: a single-player spec's text does not change.
        let spec = MissionSpec::new("UKR", AircraftId::F18);
        let text = spec.to_text();
        assert!(!text.contains("friendly-fire") && !text.contains("loadouts"));
        assert!(spec.friendly_fire && !spec.cheat_loadouts);
        let mut networked = spec.clone();
        networked.friendly_fire = false;
        networked.cheat_loadouts = true;
        let text = networked.to_text();
        assert!(text.contains("\nfriendly-fire off\n") && text.contains("\nloadouts any\n"));
        assert_eq!(MissionSpec::from_text(&text).unwrap(), networked);
        let on =
            MissionSpec::from_text(&format!("{BASE}friendly-fire on\nloadouts own\n")).unwrap();
        assert!(on.friendly_fire && !on.cheat_loadouts);
        assert_eq!(
            refused(&format!("{BASE}friendly-fire maybe\n")),
            "line 4: `friendly-fire` is `on` or `off`, not `maybe`"
        );
        assert_eq!(
            refused(&format!("{BASE}loadouts cheat\n")),
            "line 4: `loadouts` is `own` or `any`, not `cheat`"
        );
        assert_eq!(
            refused(&format!("{BASE}loadouts any\nloadouts own\n")),
            "line 5: `loadouts` appears twice"
        );
    }

    #[test]
    fn a_cheat_loading_passes_only_under_the_any_rule_and_its_own_capacities() {
        let map = crate::test_support::resources::resources();
        let player = crate::aircraft_type::AircraftType::load(&map, AircraftId::F18).unwrap();
        let standard = LoadoutSpec::of(
            &Loadout::new(&player.profile, |name| {
                map.get(name)
                    .cloned()
                    .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
            })
            .unwrap(),
        );
        let mut cheat = standard.clone();
        cheat.cheat = true;
        let mut mission = MissionSpec::new("UKR", AircraftId::F18);
        let check = |load: &LoadoutSpec, mission: &MissionSpec| {
            load.check_in(&player.profile, &map, mission)
                .map(|_| ())
                .map_err(|error| error.to_string())
        };
        assert_eq!(
            check(&cheat, &mission),
            Err("Cheat loading is not allowed in a multiplayer game.".into())
        );
        assert_eq!(check(&standard, &mission), Ok(()));
        mission.cheat_loadouts = true;
        assert_eq!(check(&cheat, &mission), Ok(()));
        assert_eq!(check(&standard, &mission), Ok(()));
        // The page's cheat capacities still bind.
        let mut over = cheat.clone();
        over.stations[1].quantity = u16::MAX;
        assert!(check(&over, &mission).is_err());
        // Guns only comes from the mission too.
        mission.guns_only = true;
        assert!(check(&cheat, &mission).unwrap_err().contains("Guns only"));
        // The single-player call keeps its rule.
        assert!(cheat.check_for_plane(&player.profile, &map, false).is_err());
    }

    #[test]
    fn friendly_fire_off_builds_into_an_open_missions_combat_and_single_player_refuses_it() {
        use crate::resources::ResourceReads;
        use crate::world::{Seating, World};
        use tore_sim::combat::live::FriendlyFire;
        let map = crate::test_support::resources::resources();
        let mut spec = MissionSpec::new(crate::test_support::resources::THEATER, AircraftId::F18);
        spec.wings[3].count = 1;
        let reads = ResourceReads::new(&map);
        let world = World::new(&spec, &reads, Seating::Open).unwrap();
        assert_eq!(world.combat.state.friendly_fire, FriendlyFire::On);
        spec.friendly_fire = false;
        let world = World::new(&spec, &reads, Seating::Open).unwrap();
        assert_eq!(world.combat.state.friendly_fire, FriendlyFire::Off);
        let refused = World::new(&spec, &reads, Seating::SinglePlayer)
            .err()
            .unwrap()
            .to_string();
        assert!(
            refused.contains("multiplayer mission's settings"),
            "{refused}"
        );
        spec.friendly_fire = true;
        spec.cheat_loadouts = true;
        assert!(World::new(&spec, &reads, Seating::SinglePlayer).is_err());
    }
}
