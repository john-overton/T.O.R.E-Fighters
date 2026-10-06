//! The King's settings (stage F phase 2): one numbered registry that the
//! host, the dedicated server's configuration file, the lobby screen and the
//! logs all read, and the host's store of the values in force. See
//! docs/ARCHITECTURE.md, "The King's settings", and
//! docs/formats/net-protocol.md, "Settings by number", for the numbers and
//! each value's coding.
//!
//! A setting's value is what the wire carries: a whole number, in seconds,
//! nautical miles or players where it is a quantity, or the index of a named
//! choice. Each setting knows its allowed values, its default in each mode,
//! when the King may change it and whether it applies only in PvP.
//!
//! Slice F2-0 built the registry and the store with its defaults; the King's
//! changes and their phase rules are slice F2-1's (`host/king.rs`), which
//! uses [`Store::apply`] for the registry's own checks, and a dedicated
//! server's file sets them through [`HostConfig::settings`].

use crate::host::HostConfig;
use tore_world::world::revive::RevivalWeapons;

/// The settings' numbers, as the wire carries them.
pub mod number {
    pub const MODE: u8 = 1;
    pub const MAX_PLAYERS: u8 = 2;
    pub const JOIN_IN_PROGRESS: u8 = 3;
    pub const VISIBILITY: u8 = 4;
    pub const PASSWORD: u8 = 5;
    pub const FRIENDLY_FIRE: u8 = 6;
    pub const LOCK_SIDES: u8 = 7;
    pub const LOADOUTS: u8 = 8;
    pub const RESPAWN: u8 = 9;
    pub const LIVES: u8 = 10;
    pub const REVIVE_DELAY: u8 = 11;
    pub const REVIVE_DISTANCE: u8 = 12;
    pub const REVIVE_WEAPONS: u8 = 13;
    pub const FIGHT: u8 = 14;
    pub const TALLY: u8 = 15;
    pub const TIME_LIMIT: u8 = 16;
    pub const KILL_LIMIT: u8 = 17;
    pub const KILL_OWNER: u8 = 18;
    pub const OBSERVER_DELAY: u8 = 19;
    pub const IDLE_AI: u8 = 20;
    /// Stage K (slice K0): the host, calculated or pinned by the King.
    pub const HOST: u8 = 21;
}

/// Setting 21's value for the calculated host; a pinned player is 1 plus
/// its lobby id.
pub const CALCULATED_HOST: u32 = 0;

/// The `idle-ai` default, in co-op and PvP: 5 minutes (John, 2026-10-06).
pub const IDLE_AI_DEFAULT: u32 = 300;

/// The `lives` value that means no limit.
pub const UNLIMITED_LIVES: u32 = 255;

/// The game's kind: who humans may fly for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Humans fly on the friendly side (EF4's lobby).
    #[default]
    Coop,
    /// Humans fly on either side.
    Pvp,
}

/// Who may find the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Visibility {
    /// Joined by address only: no search is answered.
    Hidden,
    /// Answers the local network's search.
    Local,
    /// Also listed on the Internet Lobby: a game a player hosts lists
    /// itself while it is public (slice F2-1); a dedicated server's listing
    /// is its operator's `broadcast`, so it refuses this.
    Public,
}

/// What a player's loadout may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoadoutRule {
    /// What each aircraft really carries.
    Own,
    /// Any store on any station: the loadout page's Cheat.
    Any,
}

/// What a player whose plane is lost may do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Respawn {
    /// Nothing: the player watches until the mission ends.
    None,
    /// Take a free AI aircraft of the player's side.
    AiSlot,
    /// Retail's revival: a new aircraft just outside the battle.
    Revive,
}

/// Who fights whom, for scoring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fight {
    /// Side against side.
    Sides,
    /// Every player for itself (scoring only: who can hit whom is unchanged).
    FreeForAll,
}

/// What the score board ranks by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScoreTally {
    Kills,
    Damage,
    Ratio,
}

/// Whose kills must reach the kill limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KillOwner {
    /// Every player's together.
    Total,
    /// One side's players'.
    Side,
    /// One player's.
    Player,
}

macro_rules! coded {
    ($ty:ty: $($variant:ident = $value:literal),+ $(,)?) => {
        impl $ty {
            /// The value the wire and the setting carry.
            pub fn value(self) -> u32 {
                match self {
                    $(Self::$variant => $value),+
                }
            }

            /// The choice for `value`, if it names one.
            pub fn from_value(value: u32) -> Option<Self> {
                match value {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

coded!(Mode: Coop = 0, Pvp = 1);
coded!(Visibility: Hidden = 0, Local = 1, Public = 2);
coded!(LoadoutRule: Own = 0, Any = 1);
coded!(Respawn: None = 0, AiSlot = 1, Revive = 2);
coded!(Fight: Sides = 0, FreeForAll = 1);
coded!(ScoreTally: Kills = 0, Damage = 1, Ratio = 2);
coded!(KillOwner: Total = 0, Side = 1, Player = 2);

/// When the King may change a setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// In the lobby, between missions.
    InLobby,
    /// Any time, in flight too.
    AnyTime,
}

/// What a quantity counts, for its words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// A plain number (players, lives, kills).
    Count,
    Seconds,
    NauticalMiles,
}

/// The values a setting allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allowed {
    /// These values.
    List(&'static [u32]),
    /// Every whole number from the first to the second, and the listed
    /// extras.
    Range(u32, u32, &'static [u32]),
}

/// One setting of the registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setting {
    /// Its number on the wire.
    pub number: u8,
    /// Its name: the configuration file's key and the logs'.
    pub name: &'static str,
    pub allowed: Allowed,
    pub unit: Unit,
    /// Words for values: every value of a choice, and the special ones of a
    /// quantity ("none", "unlimited").
    pub words: &'static [(u32, &'static str)],
    /// The default in co-op, and in PvP.
    pub coop: u32,
    pub pvp: u32,
    pub change: Change,
    /// It applies only in PvP: greyed in co-op.
    pub pvp_only: bool,
}

const OFF_ON: &[(u32, &str)] = &[(0, "off"), (1, "on")];
const OFF_ON_VALUES: &[u32] = &[0, 1];

/// Every setting, in number order: the registry.
pub const REGISTRY: [Setting; 21] = [
    Setting {
        number: number::MODE,
        name: "mode",
        allowed: Allowed::List(&[0, 1]),
        unit: Unit::Count,
        words: &[(0, "co-op"), (1, "pvp")],
        coop: 0,
        pvp: 1,
        change: Change::InLobby,
        pvp_only: false,
    },
    Setting {
        number: number::MAX_PLAYERS,
        name: "max-players",
        allowed: Allowed::Range(1, 30, &[]),
        unit: Unit::Count,
        words: &[],
        coop: 30,
        pvp: 30,
        change: Change::AnyTime,
        pvp_only: false,
    },
    Setting {
        number: number::JOIN_IN_PROGRESS,
        name: "join-in-progress",
        allowed: Allowed::List(OFF_ON_VALUES),
        unit: Unit::Count,
        words: OFF_ON,
        coop: 1,
        pvp: 1,
        change: Change::AnyTime,
        pvp_only: false,
    },
    Setting {
        number: number::VISIBILITY,
        name: "visibility",
        allowed: Allowed::List(&[0, 1, 2]),
        unit: Unit::Count,
        words: &[(0, "hidden"), (1, "local"), (2, "public")],
        coop: 1,
        pvp: 1,
        change: Change::AnyTime,
        pvp_only: false,
    },
    Setting {
        number: number::PASSWORD,
        name: "password",
        allowed: Allowed::List(&[0, 1]),
        unit: Unit::Count,
        words: &[(0, "none"), (1, "set")],
        coop: 0,
        pvp: 0,
        change: Change::AnyTime,
        pvp_only: false,
    },
    Setting {
        number: number::FRIENDLY_FIRE,
        name: "friendly-fire",
        allowed: Allowed::List(OFF_ON_VALUES),
        unit: Unit::Count,
        words: OFF_ON,
        coop: 1,
        pvp: 1,
        change: Change::InLobby,
        pvp_only: false,
    },
    Setting {
        number: number::LOCK_SIDES,
        name: "lock-sides",
        allowed: Allowed::List(OFF_ON_VALUES),
        unit: Unit::Count,
        words: OFF_ON,
        coop: 0,
        pvp: 1,
        change: Change::InLobby,
        pvp_only: false,
    },
    Setting {
        number: number::LOADOUTS,
        name: "loadouts",
        allowed: Allowed::List(&[0, 1]),
        unit: Unit::Count,
        words: &[(0, "own"), (1, "any")],
        coop: 0,
        pvp: 0,
        change: Change::InLobby,
        pvp_only: false,
    },
    Setting {
        number: number::RESPAWN,
        name: "respawn",
        allowed: Allowed::List(&[0, 1, 2]),
        unit: Unit::Count,
        words: &[(0, "none"), (1, "ai-slot"), (2, "revive")],
        coop: 0,
        pvp: 2,
        change: Change::InLobby,
        pvp_only: false,
    },
    Setting {
        number: number::LIVES,
        name: "lives",
        allowed: Allowed::Range(0, 10, &[UNLIMITED_LIVES]),
        unit: Unit::Count,
        words: &[(UNLIMITED_LIVES, "unlimited")],
        coop: UNLIMITED_LIVES,
        pvp: UNLIMITED_LIVES,
        change: Change::InLobby,
        pvp_only: false,
    },
    Setting {
        number: number::REVIVE_DELAY,
        name: "revive-delay",
        allowed: Allowed::List(&[0, 60, 120, 180, 240, 300]),
        unit: Unit::Seconds,
        words: &[(0, "none")],
        coop: 0,
        pvp: 0,
        change: Change::InLobby,
        pvp_only: false,
    },
    Setting {
        number: number::REVIVE_DISTANCE,
        name: "revive-distance",
        allowed: Allowed::List(&[1, 5, 10, 20, 40]),
        unit: Unit::NauticalMiles,
        words: &[],
        coop: 10,
        pvp: 10,
        change: Change::InLobby,
        pvp_only: false,
    },
    Setting {
        number: number::REVIVE_WEAPONS,
        name: "revive-weapons",
        allowed: Allowed::List(&[0, 1, 2, 3]),
        unit: Unit::Count,
        words: &[
            (0, "missiles"),
            (1, "no-missiles"),
            (2, "guns"),
            (3, "half-guns"),
        ],
        coop: 0,
        pvp: 0,
        change: Change::InLobby,
        pvp_only: false,
    },
    Setting {
        number: number::FIGHT,
        name: "fight",
        allowed: Allowed::List(&[0, 1]),
        unit: Unit::Count,
        words: &[(0, "sides"), (1, "free-for-all")],
        coop: 0,
        pvp: 0,
        change: Change::InLobby,
        pvp_only: true,
    },
    Setting {
        number: number::TALLY,
        name: "tally",
        allowed: Allowed::List(&[0, 1, 2]),
        unit: Unit::Count,
        words: &[(0, "kills"), (1, "damage"), (2, "ratio")],
        coop: 0,
        pvp: 0,
        change: Change::InLobby,
        pvp_only: true,
    },
    Setting {
        number: number::TIME_LIMIT,
        name: "time-limit",
        allowed: Allowed::List(&[0, 60, 300, 600, 900, 1_200, 1_800]),
        unit: Unit::Seconds,
        words: &[(0, "none")],
        coop: 0,
        pvp: 600,
        change: Change::InLobby,
        // The time limit ends a co-op mission too.
        pvp_only: false,
    },
    Setting {
        number: number::KILL_LIMIT,
        name: "kill-limit",
        allowed: Allowed::List(&[0, 1, 2, 3, 5, 7, 10]),
        unit: Unit::Count,
        words: &[(0, "none")],
        coop: 0,
        pvp: 5,
        change: Change::InLobby,
        pvp_only: true,
    },
    Setting {
        number: number::KILL_OWNER,
        name: "kill-owner",
        allowed: Allowed::List(&[0, 1, 2]),
        unit: Unit::Count,
        words: &[(0, "total"), (1, "side"), (2, "player")],
        coop: 1,
        pvp: 1,
        change: Change::InLobby,
        pvp_only: true,
    },
    Setting {
        number: number::OBSERVER_DELAY,
        name: "observer-delay",
        allowed: Allowed::List(&[0, 10, 30, 60]),
        unit: Unit::Seconds,
        words: &[(0, "none")],
        coop: 0,
        pvp: 0,
        change: Change::InLobby,
        pvp_only: true,
    },
    Setting {
        number: number::IDLE_AI,
        name: "idle-ai",
        // Minutes, written in seconds as the wire carries them (John,
        // 2026-10-06: 10 seconds was too short for someone on autopilot who
        // steps away; the list is an agent decision).
        allowed: Allowed::List(&[0, 60, 120, 300, 600]),
        unit: Unit::Seconds,
        words: &[(0, "never")],
        coop: IDLE_AI_DEFAULT,
        pvp: IDLE_AI_DEFAULT,
        change: Change::AnyTime,
        pvp_only: false,
    },
    // Stage K: 0 calculated, or 1 plus the lobby id of the player the King
    // pins. A pin made in flight applies when the lobby returns.
    Setting {
        number: number::HOST,
        name: "host",
        allowed: Allowed::Range(0, 256, &[]),
        unit: Unit::Count,
        words: &[(CALCULATED_HOST, "calculated")],
        coop: CALCULATED_HOST,
        pvp: CALCULATED_HOST,
        change: Change::AnyTime,
        pvp_only: false,
    },
];

/// The setting with `number`.
pub fn setting(number: u8) -> Option<&'static Setting> {
    REGISTRY
        .get(usize::from(number).checked_sub(1)?)
        .filter(|s| s.number == number)
}

/// The setting called `name`.
pub fn by_name(name: &str) -> Option<&'static Setting> {
    REGISTRY.iter().find(|s| s.name == name)
}

/// "a, b or c".
fn or_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

impl Setting {
    /// Whether the registry allows `value`.
    pub fn allows(&self, value: u32) -> bool {
        match self.allowed {
            Allowed::List(values) => values.contains(&value),
            Allowed::Range(min, max, extra) => {
                (min..=max).contains(&value) || extra.contains(&value)
            }
        }
    }

    /// The default in `mode`.
    pub fn default_in(&self, mode: Mode) -> u32 {
        match mode {
            Mode::Coop => self.coop,
            Mode::Pvp => self.pvp,
        }
    }

    /// `value` in words, for the logs and the lobby: the value's word, or
    /// the number in its unit ("10 nm", "5 minutes", "30 seconds").
    pub fn text(&self, value: u32) -> String {
        if let Some((_, word)) = self.words.iter().find(|(v, _)| *v == value) {
            return (*word).to_owned();
        }
        match self.unit {
            Unit::Count => value.to_string(),
            Unit::NauticalMiles => format!("{value} nm"),
            Unit::Seconds if value == 60 => "1 minute".to_owned(),
            Unit::Seconds if value > 0 && value.is_multiple_of(60) => {
                format!("{} minutes", value / 60)
            }
            Unit::Seconds if value == 1 => "1 second".to_owned(),
            Unit::Seconds => format!("{value} seconds"),
        }
    }

    /// Every allowed value in words, for a refusal: "co-op or pvp",
    /// "1 to 30", "0 to 10 or unlimited".
    pub fn values_text(&self) -> String {
        match self.allowed {
            Allowed::List(values) => {
                or_list(&values.iter().map(|&v| self.text(v)).collect::<Vec<_>>())
            }
            Allowed::Range(min, max, extra) => {
                let mut items = vec![format!("{} to {}", self.text(min), self.text(max))];
                items.extend(extra.iter().map(|&v| self.text(v)));
                or_list(&items)
            }
        }
    }

    /// Reads a value from its word or its number in the wire's unit
    /// ("co-op", "unlimited", "600"), if the registry allows it.
    pub fn parse(&self, text: &str) -> Option<u32> {
        let text = text.trim();
        let value = self
            .words
            .iter()
            .find(|(_, word)| word.eq_ignore_ascii_case(text))
            .map(|(v, _)| *v)
            .or_else(|| text.parse().ok())?;
        self.allows(value).then_some(value)
    }

    /// Why the registry refuses `value`, if it does: a value off the list,
    /// with the setting's name and its values.
    pub fn refusal(&self, value: u32) -> Option<String> {
        (!self.allows(value)).then(|| format!("{} is {}.", self.name, self.values_text()))
    }
}

/// Why a King's change of setting `number` to `value` is refused by the
/// registry, if it is: an unknown number, a value off its list, or the
/// password by number (it has a field of its own). The phase rules and what
/// depends on the host (the player limit's floor, a listing) are the
/// host's (`host/king.rs`, slice F2-1).
pub fn refusal(number: u8, value: u32) -> Option<String> {
    let Some(setting) = setting(number) else {
        return Some(format!("There is no setting {number}."));
    };
    if number == number::PASSWORD {
        return Some("The password is set on its own, not by number.".into());
    }
    setting.refusal(value)
}

/// `values` in words, for the logs: "mode pvp, kill-limit 5".
pub fn words(values: &[(u8, u32)]) -> String {
    values
        .iter()
        .filter_map(|&(number, value)| {
            setting(number).map(|s| format!("{} {}", s.name, s.text(value)))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The values in force on a host: every setting of the registry, the
/// game's name and its password. The lobby state carries them by number
/// ([`Store::lobby_list`]), the password only as whether one is set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    /// By number less one.
    values: [u32; REGISTRY.len()],
    name: String,
    password: Option<String>,
}

impl Store {
    /// Every setting at its default in `mode`, with no name and no password.
    pub fn defaults(mode: Mode) -> Self {
        Self {
            values: REGISTRY.map(|s| s.default_in(mode)),
            name: String::new(),
            password: None,
        }
    }

    /// A host's settings at the start: co-op's defaults with the
    /// configuration's other settings ([`HostConfig::settings`], a dedicated
    /// server's file; a new mode's defaults first, as a King's change), then
    /// its name, password, player limit and time limit.
    pub fn from_config(config: &HostConfig) -> Self {
        let mut store = Self::defaults(Mode::Coop);
        // `HostConfig::validate` has checked them; a refused list changes
        // nothing.
        let _ = store.apply(&config.settings);
        store.name = config.name.clone();
        store.password = config.password.clone();
        store.put(
            number::MAX_PLAYERS,
            u32::try_from(config.max_players).unwrap_or(u32::MAX),
        );
        // The configuration's own limit, kept as given: a dedicated server's
        // file allows longer limits than the King's list.
        store.put(
            number::TIME_LIMIT,
            config.time_limit.map_or(0, |limit| {
                u32::try_from(limit.as_secs()).unwrap_or(u32::MAX)
            }),
        );
        store
    }

    /// Sets a value past the registry's lists, so a test that waits on
    /// simulated time need not wait minutes.
    #[cfg(test)]
    pub(crate) fn set_for_test(&mut self, number: u8, value: u32) {
        self.put(number, value);
    }

    fn put(&mut self, number: u8, value: u32) {
        if let Some(slot) = usize::from(number)
            .checked_sub(1)
            .and_then(|i| self.values.get_mut(i))
        {
            *slot = value;
        }
    }

    /// The value of setting `number`; the password's is 1 when one is set.
    pub fn get(&self, number: u8) -> Option<u32> {
        if number == number::PASSWORD {
            return setting(number).map(|_| u32::from(self.password.is_some()));
        }
        self.values
            .get(usize::from(number).checked_sub(1)?)
            .copied()
    }

    fn value(&self, number: u8) -> u32 {
        self.get(number).unwrap_or_default()
    }

    /// Every setting by number, in number order, as the lobby state carries
    /// them.
    pub fn lobby_list(&self) -> Vec<(u8, u32)> {
        REGISTRY
            .iter()
            .map(|s| (s.number, self.value(s.number)))
            .collect()
    }

    /// The game's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The password, when one is set.
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// Applies a King's values all or none: the first the registry refuses
    /// ([`refusal`]) refuses them all, with its words. A new mode first sets
    /// every setting the King changes in the lobby to that mode's defaults
    /// (agent decision: the ones changed any time, such as the player limit
    /// and the password, belong to the game and are kept); the other values
    /// then apply in order. The phase rules are the caller's.
    pub fn apply(&mut self, values: &[(u8, u32)]) -> Result<(), String> {
        if let Some(why) = values
            .iter()
            .find_map(|&(number, value)| refusal(number, value))
        {
            return Err(why);
        }
        if let Some(&(_, mode)) = values.iter().find(|(n, _)| *n == number::MODE)
            && let Some(mode) = Mode::from_value(mode)
            && mode != self.mode()
        {
            for setting in REGISTRY.iter().filter(|s| s.change == Change::InLobby) {
                self.put(setting.number, setting.default_in(mode));
            }
        }
        for &(number, value) in values {
            self.put(number, value);
        }
        Ok(())
    }

    /// Sets the game's name.
    pub fn set_name(&mut self, name: &str) {
        name.clone_into(&mut self.name);
    }

    /// Sets or clears the password.
    pub fn set_password(&mut self, password: Option<&str>) {
        self.password = password.map(str::to_owned);
    }

    pub fn mode(&self) -> Mode {
        Mode::from_value(self.value(number::MODE)).unwrap_or_default()
    }

    pub fn max_players(&self) -> u32 {
        self.value(number::MAX_PLAYERS)
    }

    pub fn join_in_progress(&self) -> bool {
        self.value(number::JOIN_IN_PROGRESS) != 0
    }

    pub fn visibility(&self) -> Visibility {
        Visibility::from_value(self.value(number::VISIBILITY)).unwrap_or(Visibility::Local)
    }

    pub fn friendly_fire(&self) -> bool {
        self.value(number::FRIENDLY_FIRE) != 0
    }

    pub fn lock_sides(&self) -> bool {
        self.value(number::LOCK_SIDES) != 0
    }

    pub fn loadouts(&self) -> LoadoutRule {
        LoadoutRule::from_value(self.value(number::LOADOUTS)).unwrap_or(LoadoutRule::Own)
    }

    pub fn respawn(&self) -> Respawn {
        Respawn::from_value(self.value(number::RESPAWN)).unwrap_or(Respawn::None)
    }

    /// Revivals a player has each mission; `None` for unlimited.
    pub fn lives(&self) -> Option<u32> {
        Some(self.value(number::LIVES)).filter(|&lives| lives != UNLIMITED_LIVES)
    }

    pub fn revive_delay_seconds(&self) -> u32 {
        self.value(number::REVIVE_DELAY)
    }

    pub fn revive_distance_nm(&self) -> u32 {
        self.value(number::REVIVE_DISTANCE)
    }

    pub fn revive_weapons(&self) -> RevivalWeapons {
        RevivalWeapons::from_value(self.value(number::REVIVE_WEAPONS)).unwrap_or_default()
    }

    pub fn fight(&self) -> Fight {
        Fight::from_value(self.value(number::FIGHT)).unwrap_or(Fight::Sides)
    }

    pub fn tally(&self) -> ScoreTally {
        ScoreTally::from_value(self.value(number::TALLY)).unwrap_or(ScoreTally::Kills)
    }

    /// The time limit in seconds; `None` for none.
    pub fn time_limit_seconds(&self) -> Option<u32> {
        Some(self.value(number::TIME_LIMIT)).filter(|&s| s != 0)
    }

    /// The kill limit; `None` for none.
    pub fn kill_limit(&self) -> Option<u32> {
        Some(self.value(number::KILL_LIMIT)).filter(|&k| k != 0)
    }

    pub fn kill_owner(&self) -> KillOwner {
        KillOwner::from_value(self.value(number::KILL_OWNER)).unwrap_or(KillOwner::Side)
    }

    pub fn observer_delay_seconds(&self) -> u32 {
        self.value(number::OBSERVER_DELAY)
    }

    /// Seconds away before the AI flies a player's aircraft; `None` for
    /// never.
    pub fn idle_ai_seconds(&self) -> Option<u32> {
        Some(self.value(number::IDLE_AI)).filter(|&s| s != 0)
    }

    /// The lobby id of the player the King pinned as the host; `None` for
    /// the calculated host (stage K).
    pub fn pinned_host(&self) -> Option<u8> {
        self.value(number::HOST)
            .checked_sub(1)
            .and_then(|id| u8::try_from(id).ok())
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
