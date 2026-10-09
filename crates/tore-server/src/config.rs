//! The configuration file: one setting per line, a name and a value, `#` to the
//! end of the line is a comment. Every setting, default and range is in
//! docs/DEDICATED-SERVER.md, "The configuration file". An unknown name, a
//! value out of range and a name that appears twice are refused with the line.
//!
//! Since stage F phase 2 (slice F2-1) the file also takes the King's settings
//! by their registry names (`tore_session::settings`): `mode`, `kill-limit`,
//! `observer-delay` and the rest, and `king` and `king-mission` for a
//! server whose first player wears the crown.

use std::{
    collections::BTreeSet,
    fmt,
    path::{Path, PathBuf},
};
use tore_session::settings::{self, Mode, Setting, Visibility, number};

/// The snapshot rates that divide 120, the simulation's rate.
pub const SNAPSHOT_RATES: [u32; 8] = [10, 12, 15, 20, 24, 30, 40, 60];
/// The most players a server seats.
pub const MAX_PLAYERS: u8 = 30;
/// The most planes a mission has: six wings of five. Plane numbers run from 0.
pub const MAX_PLANES: u32 = 30;
/// The longest server name, in characters (agent decision).
pub const NAME_LIMIT: usize = 60;
/// The wire's string limit, in bytes, which the password keeps to.
pub const PASSWORD_LIMIT: usize = 255;
/// The longest time limit, in minutes: a week (agent decision).
pub const TIME_LIMIT_MAX: u32 = 7 * 24 * 60;
/// The longest empty timeout, in seconds: a day (agent decision).
pub const EMPTY_TIMEOUT_MAX: u32 = 24 * 60 * 60;
/// The longest restart delay and status interval, in seconds: an hour (agent decision).
pub const DELAY_MAX: u32 = 60 * 60;

/// Where the server listens (the transport's, shared with the game's host).
pub use tore_net::Listen;

/// Which planes humans may take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenPlanes {
    /// The friendly planes.
    Friendly,
    /// Every plane (load tests).
    All,
    /// These plane numbers, in order.
    List(Vec<u32>),
}

/// When the mission starts flying.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartMode {
    /// Not flying until the first player is seated.
    FirstPlayer,
    /// Flying from the start.
    Now,
}

/// What happens when a mission ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterEnd {
    Restart,
    Quit,
}

/// The settings of a server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub name: String,
    pub port: u16,
    pub address: Listen,
    pub password: Option<String>,
    pub max_players: u8,
    /// The mission file, made absolute against the configuration file's folder.
    pub mission: PathBuf,
    pub open_planes: OpenPlanes,
    pub snapshot_rate: u32,
    pub start: StartMode,
    /// Minutes; 0 for none.
    pub time_limit_minutes: u32,
    pub empty_timeout_seconds: u32,
    pub after_end: AfterEnd,
    pub restart_delay_seconds: u32,
    /// Seconds between status lines; 0 for none.
    pub status_interval_seconds: u32,
    /// List the server on the Internet Lobby (John, 2026-10-05: off unless
    /// the operator turns broadcasting on, as OpenRA's servers do).
    pub broadcast: bool,
    /// The master server, `HOST` or `HOST:PORT` (port 26901 when none).
    pub master: String,
    /// Send anonymous statistics to the master while broadcasting (John,
    /// 2026-10-05: on by default).
    pub telemetry: bool,
    /// Ask the router to forward the game port when the server starts, by
    /// UPnP, NAT-PMP or PCP (slice J4b). Off by default, since a server's
    /// port is normally forwarded by its owner.
    pub port_mapping: bool,
    /// `king first-player`: the first player to join wears the crown, and
    /// then the longest-connected (stage F phase 2; John, 2026-10-05: none
    /// by default).
    pub king: bool,
    /// `king-mission locked`: the King may not change the mission or the
    /// settings.
    pub king_mission_locked: bool,
    /// The King's settings the file gives (by registry number, in file
    /// order), beyond `max-players`, `password` and `time-limit`.
    pub settings: Vec<(u8, u32)>,
}

impl Config {
    /// The defaults of the guide's table. `base` is the folder a relative
    /// mission path is taken against.
    pub fn defaults(base: &Path) -> Self {
        Self {
            name: "T.O.R.E server".into(),
            port: 26900,
            address: Listen::Any,
            password: None,
            max_players: MAX_PLAYERS,
            mission: base.join("mission.txt"),
            open_planes: OpenPlanes::Friendly,
            snapshot_rate: tore_session::host::config::DEFAULT_SNAPSHOT_RATE,
            start: StartMode::FirstPlayer,
            time_limit_minutes: 0,
            empty_timeout_seconds: 60,
            after_end: AfterEnd::Restart,
            restart_delay_seconds: 30,
            status_interval_seconds: 10,
            broadcast: false,
            master: tore_net::master::DEFAULT_MASTER.into(),
            telemetry: true,
            port_mapping: false,
            king: false,
            king_mission_locked: false,
            settings: Vec::new(),
        }
    }

    /// Reads the text of a configuration file. A relative mission path is
    /// taken against `base`, the configuration file's folder.
    pub fn parse(text: &str, base: &Path) -> Result<Self, ConfigError> {
        let mut config = Self::defaults(base);
        let mut seen = BTreeSet::new();
        // The line of each King's setting, for the checks across lines.
        let mut lines: Vec<(usize, &str)> = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let content = raw.split('#').next().unwrap_or("").trim();
            if content.is_empty() {
                continue;
            }
            let (name, value) = match content.split_once(char::is_whitespace) {
                Some((name, value)) => (name, value.trim()),
                None => (content, ""),
            };
            let fail = |message: String| ConfigError { line, message };
            if !SETTINGS.contains(&name) {
                return Err(fail(format!(
                    "`{name}` is not a setting (settings are {})",
                    SETTINGS.join(", ")
                )));
            }
            if !seen.insert(name) {
                return Err(fail(format!("`{name}` appears twice")));
            }
            if value.is_empty() {
                return Err(fail(format!("`{name}` needs a value")));
            }
            config.set(name, value, base).map_err(fail)?;
            lines.push((line, name));
        }
        config.check_king_settings(&lines)?;
        Ok(config)
    }

    /// The checks across lines: a setting that applies only in PvP needs
    /// `mode pvp` (agent decision: a co-op server would never use it), and
    /// `king-mission` needs a King.
    fn check_king_settings(&self, lines: &[(usize, &str)]) -> Result<(), ConfigError> {
        let line_of = |key: &str| lines.iter().find(|(_, n)| *n == key).map_or(0, |(l, _)| *l);
        let pvp = self
            .settings
            .iter()
            .any(|&(n, v)| n == number::MODE && v == Mode::Pvp.value());
        if !pvp
            && let Some(setting) = self
                .settings
                .iter()
                .filter_map(|&(n, _)| settings::setting(n))
                .find(|s| s.pvp_only)
        {
            return Err(ConfigError {
                line: line_of(setting.name),
                message: format!(
                    "`{}` applies only in PvP: add `mode pvp`, or leave it out",
                    setting.name
                ),
            });
        }
        if self.king_mission_locked && !self.king {
            return Err(ConfigError {
                line: line_of("king-mission"),
                message: "`king-mission` needs a King: add `king first-player`".into(),
            });
        }
        Ok(())
    }

    fn set(&mut self, name: &str, value: &str, base: &Path) -> Result<(), String> {
        match name {
            "name" => {
                let length = value.chars().count();
                if length > NAME_LIMIT || value.chars().any(char::is_control) {
                    return Err(format!(
                        "`name` must be 1 to {NAME_LIMIT} printable characters"
                    ));
                }
                self.name = value.to_owned();
            }
            "port" => {
                self.port = number(name, value, 1, 65_535)? as u16;
            }
            "address" => {
                self.address = if value.eq_ignore_ascii_case("any") {
                    Listen::Any
                } else {
                    Listen::Address(value.parse().map_err(|_| {
                        format!("`address` must be `any` or an IP address, not `{value}`")
                    })?)
                };
            }
            "password" => {
                if value.len() > PASSWORD_LIMIT || value.chars().any(char::is_control) {
                    return Err(format!(
                        "`password` must be printable and at most {PASSWORD_LIMIT} bytes"
                    ));
                }
                self.password = Some(value.to_owned());
            }
            "max-players" => {
                self.max_players = number(name, value, 1, u32::from(MAX_PLAYERS))? as u8;
            }
            "mission" => {
                let path = PathBuf::from(value);
                self.mission = if path.is_absolute() {
                    path
                } else {
                    base.join(path)
                };
            }
            "open-planes" => {
                self.open_planes = match value {
                    "friendly" => OpenPlanes::Friendly,
                    "all" => OpenPlanes::All,
                    list => {
                        let mut planes = Vec::new();
                        for word in list.split([',', ' ', '\t']).filter(|w| !w.is_empty()) {
                            let plane = word.parse::<u32>().map_err(|_| {
                                format!(
                                    "`open-planes` must be `friendly`, `all` or plane numbers, not `{word}`"
                                )
                            })?;
                            if plane >= MAX_PLANES {
                                return Err(format!(
                                    "`open-planes` plane {plane} is out of range: a mission has planes 0 to {}",
                                    MAX_PLANES - 1
                                ));
                            }
                            if planes.contains(&plane) {
                                return Err(format!("`open-planes` lists plane {plane} twice"));
                            }
                            planes.push(plane);
                        }
                        OpenPlanes::List(planes)
                    }
                };
            }
            "snapshot-rate" => {
                let rate = number(name, value, 0, 1000)?;
                if !SNAPSHOT_RATES.contains(&rate) {
                    return Err(format!(
                        "`snapshot-rate` must be one of 10, 12, 15, 20, 24, 30, 40 or 60 (the rates that divide 120), not {rate}"
                    ));
                }
                self.snapshot_rate = rate;
            }
            "start" => {
                self.start = match value {
                    "first-player" => StartMode::FirstPlayer,
                    "now" => StartMode::Now,
                    other => {
                        return Err(format!(
                            "`start` must be `first-player` or `now`, not `{other}`"
                        ));
                    }
                };
            }
            "time-limit" => self.time_limit_minutes = number(name, value, 0, TIME_LIMIT_MAX)?,
            "empty-timeout" => {
                self.empty_timeout_seconds = number(name, value, 0, EMPTY_TIMEOUT_MAX)?
            }
            "after-end" => {
                self.after_end = match value {
                    "restart" => AfterEnd::Restart,
                    "quit" => AfterEnd::Quit,
                    other => {
                        return Err(format!(
                            "`after-end` must be `restart` or `quit`, not `{other}`"
                        ));
                    }
                };
            }
            "restart-delay" => self.restart_delay_seconds = number(name, value, 0, DELAY_MAX)?,
            "status-interval" => self.status_interval_seconds = number(name, value, 0, DELAY_MAX)?,
            "broadcast" => self.broadcast = switch(name, value)?,
            "master" => {
                tore_net::master::local::parse_master(value)
                    .map_err(|error| format!("`master` must be HOST or HOST:PORT: {error}"))?;
                self.master = value.to_owned();
            }
            "telemetry" => self.telemetry = switch(name, value)?,
            "port-mapping" => self.port_mapping = switch(name, value)?,
            "king" => {
                self.king = match value {
                    "first-player" => true,
                    "none" => false,
                    other => {
                        return Err(format!(
                            "`king` must be `first-player` or `none`, not `{other}`"
                        ));
                    }
                };
            }
            "king-mission" => {
                self.king_mission_locked = match value {
                    "locked" => true,
                    "open" => false,
                    other => {
                        return Err(format!(
                            "`king-mission` must be `locked` or `open`, not `{other}`"
                        ));
                    }
                };
            }
            king => {
                let setting =
                    settings::by_name(king).expect("the name was checked against SETTINGS");
                let value = king_value(setting, value)?;
                self.settings.push((setting.number, value));
            }
        }
        Ok(())
    }

    /// Where the server listens, as socket addresses' IP part.
    pub fn listen_description(&self) -> String {
        match self.address {
            Listen::Any => format!("UDP port {} on every address", self.port),
            Listen::Address(ip) => format!("UDP {}", std::net::SocketAddr::new(ip, self.port)),
        }
    }
}

/// Every setting's name, in the guide's order: the server's own, then the
/// King's (stage F phase 2) by their registry names.
pub const SETTINGS: [&str; 38] = [
    "name",
    "port",
    "address",
    "password",
    "max-players",
    "mission",
    "open-planes",
    "snapshot-rate",
    "start",
    "time-limit",
    "empty-timeout",
    "after-end",
    "restart-delay",
    "status-interval",
    "broadcast",
    "master",
    "telemetry",
    "port-mapping",
    "king",
    "king-mission",
    "mode",
    "join-in-progress",
    "visibility",
    "friendly-fire",
    "lock-sides",
    "loadouts",
    "respawn",
    "lives",
    "revive-delay",
    "revive-distance",
    "revive-weapons",
    "fight",
    "tally",
    "kill-limit",
    "kill-owner",
    "observer-delay",
    "idle-ai",
    "ai-respawn",
];

/// How many of the registry's units one of the file's is: `revive-delay` and
/// `idle-ai` are written in minutes, as `time-limit` is (the lobby shows
/// minutes too); every other number as the registry counts it (nautical
/// miles, a count).
fn file_scale(setting: &Setting) -> u32 {
    if matches!(setting.number, number::REVIVE_DELAY | number::IDLE_AI) {
        60
    } else {
        1
    }
}

/// A King's setting's value as the file writes it: one of its words, or a
/// number in the file's unit. `visibility public` is refused: a server's
/// listing is `broadcast`.
fn king_value(setting: &Setting, value: &str) -> Result<u32, String> {
    let scale = file_scale(setting);
    let read = setting
        .words
        .iter()
        .find(|(_, word)| *word == value)
        .map(|(v, _)| *v)
        .or_else(|| value.parse::<u32>().ok().and_then(|n| n.checked_mul(scale)))
        .filter(|v| setting.allows(*v));
    let allowed = || {
        let items: Vec<String> = match setting.allowed {
            settings::Allowed::List(values) => values
                .iter()
                .filter(|v| {
                    setting.number != number::VISIBILITY || **v != Visibility::Public.value()
                })
                .map(|&v| file_word(setting, v, scale))
                .collect(),
            settings::Allowed::Range(low, high, extra) => {
                let mut items = vec![format!("{} to {}", low / scale, high / scale)];
                items.extend(extra.iter().map(|&v| file_word(setting, v, scale)));
                items
            }
        };
        match items.as_slice() {
            [rest @ .., last] if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
            _ => items.join(""),
        }
    };
    match read {
        Some(v) if setting.number == number::VISIBILITY && v == Visibility::Public.value() => {
            Err("`visibility` is `hidden` or `local` on a server: `broadcast on` lists it on the Internet Lobby".into())
        }
        Some(v) => Ok(v),
        None => Err(format!(
            "`{}` must be {}, not `{value}`",
            setting.name,
            allowed()
        )),
    }
}

/// One allowed value as the file writes it: its word, or its number in the
/// file's unit.
fn file_word(setting: &Setting, value: u32, scale: u32) -> String {
    setting
        .words
        .iter()
        .find(|(v, _)| *v == value)
        .map_or_else(|| (value / scale).to_string(), |(_, w)| (*w).to_owned())
}

fn switch(name: &str, value: &str) -> Result<bool, String> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        other => Err(format!("`{name}` must be `on` or `off`, not `{other}`")),
    }
}

fn number(name: &str, value: &str, low: u32, high: u32) -> Result<u32, String> {
    let parsed: u32 = value
        .parse()
        .map_err(|_| format!("`{name}` must be a whole number, not `{value}`"))?;
    if !(low..=high).contains(&parsed) {
        return Err(format!("`{name}` must be {low} to {high}, not {parsed}"));
    }
    Ok(parsed)
}

/// A line of the configuration file the server refuses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> PathBuf {
        PathBuf::from("/srv/tore")
    }

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::parse(text, &base())
    }

    fn refused(text: &str) -> String {
        parse(text).expect_err("the file is refused").to_string()
    }

    #[test]
    fn an_empty_file_is_the_guides_defaults() {
        let config = parse("").unwrap();
        assert_eq!(config, Config::defaults(&base()));
        assert_eq!(config.name, "T.O.R.E server");
        assert_eq!(config.port, 26900);
        assert_eq!(config.address, Listen::Any);
        assert_eq!(config.password, None);
        assert_eq!(config.max_players, 30);
        assert_eq!(config.mission, base().join("mission.txt"));
        assert_eq!(config.open_planes, OpenPlanes::Friendly);
        assert_eq!(config.snapshot_rate, 60);
        assert_eq!(config.start, StartMode::FirstPlayer);
        assert_eq!(config.time_limit_minutes, 0);
        assert_eq!(config.empty_timeout_seconds, 60);
        assert_eq!(config.after_end, AfterEnd::Restart);
        assert_eq!(config.restart_delay_seconds, 30);
        assert_eq!(config.status_interval_seconds, 10);
        assert!(!config.broadcast);
        assert_eq!(config.master, tore_net::master::DEFAULT_MASTER);
        assert!(config.telemetry);
        assert!(!config.king && !config.king_mission_locked);
        assert!(config.settings.is_empty());
    }

    #[test]
    fn the_kings_settings_read_by_their_registry_names() {
        let config = parse(
            "mode pvp\nkill-limit 3\nobserver-delay 30\nrevive-delay 2\nlives unlimited\n\
             idle-ai never\nvisibility hidden\nfriendly-fire off\nloadouts any\nking first-player\n\
             king-mission locked\njoin-in-progress off\nkill-owner player\nrevive-distance 40\n",
        )
        .unwrap();
        assert!(config.king && config.king_mission_locked);
        assert_eq!(
            config.settings,
            [
                (number::MODE, 1),
                (number::KILL_LIMIT, 3),
                (number::OBSERVER_DELAY, 30),
                (number::REVIVE_DELAY, 120),
                (number::LIVES, settings::UNLIMITED_LIVES),
                (number::IDLE_AI, 0),
                (number::VISIBILITY, 0),
                (number::FRIENDLY_FIRE, 0),
                (number::LOADOUTS, 1),
                (number::JOIN_IN_PROGRESS, 0),
                (number::KILL_OWNER, 2),
                (number::REVIVE_DISTANCE, 40),
            ]
        );
        // The values the file allows, in its own units.
        assert_eq!(
            refused("kill-limit 4\nmode pvp"),
            "line 1: `kill-limit` must be none, 1, 2, 3, 5, 7 or 10, not `4`"
        );
        assert_eq!(
            refused("revive-delay 6"),
            "line 1: `revive-delay` must be none, 1, 2, 3, 4 or 5, not `6`"
        );
        // The idle time is written in minutes (John, 2026-10-06: minutes in
        // the lobby too; the list is an agent decision).
        assert_eq!(
            parse("idle-ai 2").unwrap().settings,
            [(number::IDLE_AI, 120)]
        );
        assert_eq!(
            parse("idle-ai 10").unwrap().settings,
            [(number::IDLE_AI, 600)]
        );
        assert_eq!(
            refused("idle-ai 3"),
            "line 1: `idle-ai` must be never, 1, 2, 5 or 10, not `3`"
        );
        assert_eq!(
            refused("lives 11"),
            "line 1: `lives` must be 0 to 10 or unlimited, not `11`"
        );
        assert_eq!(
            refused("mode duel"),
            "line 1: `mode` must be co-op or pvp, not `duel`"
        );
        assert!(refused("visibility public").contains("`broadcast on`"));
        assert!(refused("visibility everyone").contains("hidden or local"));
        assert_eq!(
            refused("king always"),
            "line 1: `king` must be `first-player` or `none`, not `always`"
        );
        assert!(refused("king-mission shut").contains("`locked` or `open`"));
        // Across lines: PvP's settings need PvP, a locked mission a King.
        assert_eq!(
            refused("port 1\nobserver-delay 10"),
            "line 2: `observer-delay` applies only in PvP: add `mode pvp`, or leave it out"
        );
        assert_eq!(
            refused("king-mission locked"),
            "line 1: `king-mission` needs a King: add `king first-player`"
        );
        assert!(parse("king none\nking-mission open\nmode co-op\ntime-limit 90").is_ok());
    }

    /// The lobby pass (slice W0): `lock-sides balanced`, the revival
    /// distances to 150 nautical miles and `ai-respawn`.
    #[test]
    fn the_lobby_pass_words_read() {
        assert_eq!(
            parse("mode pvp\nlock-sides balanced\nrevive-distance 150\nai-respawn off\n")
                .unwrap()
                .settings,
            [
                (number::MODE, 1),
                (number::LOCK_SIDES, 2),
                (number::REVIVE_DISTANCE, 150),
                (number::AI_RESPAWN, 0),
            ]
        );
        for distance in [50, 75, 100, 150] {
            assert_eq!(
                parse(&format!("revive-distance {distance}"))
                    .unwrap()
                    .settings,
                [(number::REVIVE_DISTANCE, distance)]
            );
        }
        assert_eq!(
            parse("ai-respawn on").unwrap().settings,
            [(number::AI_RESPAWN, 1)]
        );
        assert_eq!(
            refused("revive-distance 60"),
            "line 1: `revive-distance` must be 1, 5, 10, 20, 40, 50, 75, 100 or 150, not `60`"
        );
        assert_eq!(
            refused("mode pvp\nlock-sides even"),
            "line 2: `lock-sides` must be off, on or balanced, not `even`"
        );
        // Sides are PvP's alone now.
        assert_eq!(
            refused("lock-sides balanced"),
            "line 1: `lock-sides` applies only in PvP: add `mode pvp`, or leave it out"
        );
    }

    #[test]
    fn broadcast_and_telemetry_are_on_or_off_and_the_master_an_address() {
        let config = parse("broadcast on\ntelemetry off\nmaster 127.0.0.1:26911").unwrap();
        assert!(config.broadcast && !config.telemetry);
        assert_eq!(config.master, "127.0.0.1:26911");
        assert!(!parse("broadcast off").unwrap().broadcast);
        assert!(refused("broadcast yes").contains("`on` or `off`"));
        assert!(refused("telemetry 1").contains("`on` or `off`"));
        assert_eq!(
            parse("master master.example.org").unwrap().master,
            "master.example.org"
        );
        assert_eq!(parse("master [::1]:26911").unwrap().master, "[::1]:26911");
        assert!(refused("master host:0").contains("HOST or HOST:PORT"));
        assert!(refused("master [::1").contains("HOST or HOST:PORT"));
    }

    #[test]
    fn port_mapping_is_off_by_default_and_a_switch() {
        assert!(!parse("port 26900").unwrap().port_mapping);
        assert!(parse("port-mapping on").unwrap().port_mapping);
        assert!(!parse("port-mapping off").unwrap().port_mapping);
        assert!(refused("port-mapping maybe").contains("`on` or `off`"));
    }

    #[test]
    fn comments_blank_lines_and_spacing_are_ignored() {
        let config =
            parse("# my server\n\n  port   27000   # the port\n\tname  Night Owls  \n").unwrap();
        assert_eq!(config.port, 27000);
        assert_eq!(config.name, "Night Owls");
    }

    #[test]
    fn name_keeps_its_spaces_and_is_bounded() {
        assert_eq!(parse("name Two  Words").unwrap().name, "Two  Words");
        assert!(parse(&format!("name {}", "x".repeat(60))).is_ok());
        assert!(refused(&format!("name {}", "x".repeat(61))).contains("1 to 60"));
    }

    #[test]
    fn port_is_1_to_65535() {
        assert_eq!(parse("port 1").unwrap().port, 1);
        assert_eq!(parse("port 65535").unwrap().port, 65535);
        assert!(refused("port 0").starts_with("line 1: `port` must be 1 to 65535"));
        assert!(refused("port 65536").contains("must be 1 to 65535"));
        assert!(refused("port http").contains("whole number"));
        assert!(refused("port -5").contains("whole number"));
    }

    #[test]
    fn address_is_any_or_an_ip() {
        assert_eq!(parse("address any").unwrap().address, Listen::Any);
        assert_eq!(
            parse("address 127.0.0.1").unwrap().address,
            Listen::Address("127.0.0.1".parse().unwrap())
        );
        assert_eq!(
            parse("address ::1").unwrap().address,
            Listen::Address("::1".parse().unwrap())
        );
        assert!(refused("address example.com").contains("`any` or an IP address"));
    }

    #[test]
    fn password_is_printable_and_bounded() {
        assert_eq!(
            parse("password swordfish 1").unwrap().password.as_deref(),
            Some("swordfish 1")
        );
        assert!(parse(&format!("password {}", "p".repeat(255))).is_ok());
        assert!(refused(&format!("password {}", "p".repeat(256))).contains("255 bytes"));
    }

    #[test]
    fn max_players_is_1_to_30() {
        assert_eq!(parse("max-players 1").unwrap().max_players, 1);
        assert_eq!(parse("max-players 30").unwrap().max_players, 30);
        assert!(refused("max-players 0").contains("1 to 30"));
        assert!(refused("max-players 31").contains("1 to 30"));
    }

    #[test]
    fn mission_is_taken_against_the_configuration_folder() {
        assert_eq!(
            parse("mission duel.txt").unwrap().mission,
            base().join("duel.txt")
        );
        assert_eq!(
            parse("mission maps/duel.txt").unwrap().mission,
            base().join("maps/duel.txt")
        );
        let absolute = std::env::temp_dir().join("duel.txt");
        assert_eq!(
            parse(&format!("mission {}", absolute.display()))
                .unwrap()
                .mission,
            absolute
        );
    }

    #[test]
    fn open_planes_is_friendly_all_or_a_list() {
        assert_eq!(
            parse("open-planes all").unwrap().open_planes,
            OpenPlanes::All
        );
        assert_eq!(
            parse("open-planes friendly").unwrap().open_planes,
            OpenPlanes::Friendly
        );
        assert_eq!(
            parse("open-planes 0, 2 5,7").unwrap().open_planes,
            OpenPlanes::List(vec![0, 2, 5, 7])
        );
        assert!(refused("open-planes 30").contains("planes 0 to 29"));
        assert!(refused("open-planes 1 1").contains("twice"));
        assert!(refused("open-planes some").contains("plane numbers"));
    }

    #[test]
    fn snapshot_rate_is_one_that_divides_120() {
        for rate in SNAPSHOT_RATES {
            assert_eq!(120 % rate, 0);
            assert_eq!(
                parse(&format!("snapshot-rate {rate}"))
                    .unwrap()
                    .snapshot_rate,
                rate
            );
        }
        for rate in [0, 1, 8, 25, 35, 45, 120] {
            assert!(
                refused(&format!("snapshot-rate {rate}"))
                    .contains("10, 12, 15, 20, 24, 30, 40 or 60"),
                "{rate}"
            );
        }
    }

    #[test]
    fn start_is_first_player_or_now() {
        assert_eq!(parse("start now").unwrap().start, StartMode::Now);
        assert_eq!(
            parse("start first-player").unwrap().start,
            StartMode::FirstPlayer
        );
        assert!(refused("start later").contains("`first-player` or `now`"));
    }

    #[test]
    fn the_timers_have_their_ranges() {
        assert_eq!(parse("time-limit 0").unwrap().time_limit_minutes, 0);
        assert_eq!(parse("time-limit 90").unwrap().time_limit_minutes, 90);
        assert!(parse("time-limit 10080").is_ok());
        assert!(refused("time-limit 10081").contains("0 to 10080"));
        assert_eq!(parse("empty-timeout 0").unwrap().empty_timeout_seconds, 0);
        assert!(parse("empty-timeout 86400").is_ok());
        assert!(refused("empty-timeout 86401").contains("0 to 86400"));
        assert_eq!(parse("restart-delay 0").unwrap().restart_delay_seconds, 0);
        assert!(parse("restart-delay 3600").is_ok());
        assert!(refused("restart-delay 3601").contains("0 to 3600"));
        assert_eq!(
            parse("status-interval 0").unwrap().status_interval_seconds,
            0
        );
        assert!(parse("status-interval 3600").is_ok());
        assert!(refused("status-interval 3601").contains("0 to 3600"));
    }

    #[test]
    fn after_end_is_restart_or_quit() {
        assert_eq!(parse("after-end quit").unwrap().after_end, AfterEnd::Quit);
        assert_eq!(
            parse("after-end restart").unwrap().after_end,
            AfterEnd::Restart
        );
        assert!(refused("after-end loop").contains("`restart` or `quit`"));
    }

    #[test]
    fn an_unknown_name_is_refused_with_its_line() {
        let message = refused("port 26900\n\n# a typo\nmax-player 4\n");
        assert!(
            message.starts_with("line 4: `max-player` is not a setting"),
            "{message}"
        );
        assert!(message.contains("max-players"));
    }

    #[test]
    fn a_repeated_name_and_a_missing_value_are_refused() {
        assert_eq!(refused("port 1\nport 2"), "line 2: `port` appears twice");
        assert_eq!(refused("port"), "line 1: `port` needs a value");
        assert_eq!(refused("port # nothing"), "line 1: `port` needs a value");
    }

    #[test]
    fn every_setting_is_in_the_name_list() {
        // Keeps `SETTINGS` and `set` in step: each name must parse somewhere.
        let samples = [
            "name x",
            "port 1",
            "address any",
            "password x",
            "max-players 1",
            "mission x",
            "open-planes all",
            "snapshot-rate 10",
            "start now",
            "time-limit 1",
            "empty-timeout 1",
            "after-end quit",
            "restart-delay 1",
            "status-interval 1",
            "broadcast on",
            "master localhost",
            "telemetry off",
            "port-mapping off",
            "king first-player",
            "king-mission open",
            "mode co-op",
            "join-in-progress on",
            "visibility local",
            "friendly-fire on",
            "lock-sides off",
            "loadouts own",
            "respawn none",
            "lives 3",
            "revive-delay 0",
            "revive-distance 10",
            "revive-weapons guns",
            "fight sides",
            "tally kills",
            "kill-limit none",
            "kill-owner side",
            "observer-delay 0",
            "idle-ai 10",
            "ai-respawn off",
        ];
        assert_eq!(samples.len(), SETTINGS.len());
        for (sample, name) in samples.iter().zip(SETTINGS) {
            assert!(sample.starts_with(name), "{sample}");
            // A setting of PvP's alone is refused in co-op.
            let pvp = settings::by_name(name).is_some_and(|s| s.pvp_only);
            let text = if pvp {
                format!("mode pvp\n{sample}")
            } else {
                (*sample).to_owned()
            };
            parse(&text).unwrap();
        }
    }
}
