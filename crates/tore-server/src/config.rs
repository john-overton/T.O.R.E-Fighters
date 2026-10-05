//! The configuration file: one setting per line, a name and a value, `#` to the
//! end of the line is a comment. Every setting, default and range is in
//! docs/DEDICATED-SERVER.md, "The configuration file". An unknown name, a
//! value out of range and a name that appears twice are refused with the line.

use std::{
    collections::BTreeSet,
    fmt,
    path::{Path, PathBuf},
};

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
            snapshot_rate: 30,
            start: StartMode::FirstPlayer,
            time_limit_minutes: 0,
            empty_timeout_seconds: 60,
            after_end: AfterEnd::Restart,
            restart_delay_seconds: 30,
            status_interval_seconds: 10,
            broadcast: false,
            master: tore_net::master::DEFAULT_MASTER.into(),
            telemetry: true,
        }
    }

    /// Reads the text of a configuration file. A relative mission path is
    /// taken against `base`, the configuration file's folder.
    pub fn parse(text: &str, base: &Path) -> Result<Self, ConfigError> {
        let mut config = Self::defaults(base);
        let mut seen = BTreeSet::new();
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
        }
        Ok(config)
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
            _ => unreachable!("the name was checked against SETTINGS"),
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

/// Every setting's name, in the guide's order.
pub const SETTINGS: [&str; 17] = [
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
];

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
        assert_eq!(config.snapshot_rate, 30);
        assert_eq!(config.start, StartMode::FirstPlayer);
        assert_eq!(config.time_limit_minutes, 0);
        assert_eq!(config.empty_timeout_seconds, 60);
        assert_eq!(config.after_end, AfterEnd::Restart);
        assert_eq!(config.restart_delay_seconds, 30);
        assert_eq!(config.status_interval_seconds, 10);
        assert!(!config.broadcast);
        assert_eq!(config.master, tore_net::master::DEFAULT_MASTER);
        assert!(config.telemetry);
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
        ];
        assert_eq!(samples.len(), SETTINGS.len());
        for (sample, name) in samples.iter().zip(SETTINGS) {
            assert!(sample.starts_with(name), "{sample}");
            parse(sample).unwrap();
        }
    }
}
