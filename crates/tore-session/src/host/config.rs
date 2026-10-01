//! The host's settings: what the dedicated server's configuration file sets
//! (docs/DEDICATED-SERVER.md, "The configuration file") and the build the
//! host runs, with the rules each setting must keep.

use std::fmt;
use std::time::Duration;
use tore_net::Entropy;

/// Players a host seats at most (the wire's seat limit; John's default).
pub const MAX_PLAYERS: usize = 30;
/// The snapshot rates a host offers, a second.
pub const SNAPSHOT_RATES: [u32; 8] = [10, 12, 15, 20, 24, 30, 40, 60];

/// Which planes humans may take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenPlanes {
    /// Every friendly plane (the default).
    Friendly,
    /// Every plane, both sides: load tests (proper PvP is stage F).
    All,
    /// These plane numbers only.
    List(Vec<u32>),
}

impl OpenPlanes {
    /// Whether `plane`, on the friendly side when `friendly`, is open.
    pub fn allows(&self, plane: u32, friendly: bool) -> bool {
        match self {
            Self::Friendly => friendly,
            Self::All => true,
            Self::List(planes) => planes.contains(&plane),
        }
    }
}

/// When the mission starts flying.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartMode {
    /// The mission waits at tick 0, not flying and sending no snapshots,
    /// until the first player is seated (the default).
    FirstPlayer,
    /// It flies from the start.
    Now,
}

/// What follows a mission's end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterEnd {
    /// The same mission again, fresh, after the restart delay (the default).
    Restart,
    /// The host stops.
    Quit,
}

/// The build a host runs, which a joining game must match
/// (docs/ARCHITECTURE.md, "Joining, leaving and the end of a mission").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildId {
    /// The version string, for example `0.1.3`, or `0.1.3-4-gabc1234` for a
    /// build between tags.
    pub version: String,
    /// The commit the build stamps.
    pub commit: String,
    /// A tagged release build.
    pub release: bool,
}

impl BuildId {
    /// Whether a game reporting `version` and `commit` may join. Two builds
    /// of one commit always match. A tagged release host also takes a game
    /// of the same plain version (no `-` suffix), since every tagged build
    /// of a version is the same source.
    ///
    /// *Agent decision:* the game does not say whether it is a tagged build,
    /// so a local build of another commit that still reports the crate's
    /// plain version passes a release host's check. Release builds are what
    /// players run; the corner is a developer's.
    pub fn matches(&self, version: &str, commit: &str) -> bool {
        if commit == self.commit && commit != "unknown" {
            return true;
        }
        self.release && version == self.version && !version.contains('-')
    }
}

/// A host's settings. [`HostConfig::new`] gives the server guide's defaults.
#[derive(Clone, Debug)]
pub struct HostConfig {
    /// Shown to joining players.
    pub name: String,
    /// A password players must give, or `None`.
    pub password: Option<String>,
    /// 1 to 30.
    pub max_players: usize,
    /// Which planes humans may take.
    pub open_planes: OpenPlanes,
    /// Snapshots a second to each player: one of [`SNAPSHOT_RATES`].
    pub snapshot_rate: u32,
    /// When the mission starts flying.
    pub start: StartMode,
    /// Flying time after which the mission ends; `None` for none.
    pub time_limit: Option<Duration>,
    /// How long the mission flies on after the last player leaves.
    pub empty_timeout: Duration,
    /// What follows the end.
    pub after_end: AfterEnd,
    /// Between a mission's end and the next start.
    pub restart_delay: Duration,
    /// The build, for the join check.
    pub build: BuildId,
    /// Where the transport's secrets come from: [`Entropy::System`] on real
    /// sockets, [`Entropy::Seeded`] in tests and on the simulator only.
    pub entropy: Entropy,
    /// The retail stall-speed switch is on for this process: a host refuses
    /// to start with it (as it does when `tore_sim::flight` reports it on).
    pub retail_stall_speeds: bool,
}

impl HostConfig {
    /// The server guide's defaults for `build`.
    pub fn new(build: BuildId) -> Self {
        Self {
            name: "T.O.R.E server".into(),
            password: None,
            max_players: MAX_PLAYERS,
            open_planes: OpenPlanes::Friendly,
            snapshot_rate: 30,
            start: StartMode::FirstPlayer,
            time_limit: None,
            empty_timeout: Duration::from_secs(60),
            after_end: AfterEnd::Restart,
            restart_delay: Duration::from_secs(30),
            build,
            entropy: Entropy::System,
            retail_stall_speeds: false,
        }
    }

    /// Ticks between snapshots: 120 over the snapshot rate.
    pub fn ticks_per_snapshot(&self) -> u32 {
        120 / self.snapshot_rate.max(1)
    }

    /// Checks every setting against its range.
    pub fn validate(&self) -> Result<(), HostError> {
        let bad = |text: String| Err(HostError::Setting(text));
        if !(1..=MAX_PLAYERS).contains(&self.max_players) {
            return bad(format!(
                "max-players is {}, but must be 1 to {MAX_PLAYERS}",
                self.max_players
            ));
        }
        if !SNAPSHOT_RATES.contains(&self.snapshot_rate) {
            return bad(format!(
                "snapshot-rate is {}, but must be one of 10, 12, 15, 20, 24, 30, 40 or 60",
                self.snapshot_rate
            ));
        }
        if self.name.is_empty() || self.name.len() > 64 {
            return bad("name must be 1 to 64 bytes".into());
        }
        if self
            .password
            .as_ref()
            .is_some_and(|p| p.is_empty() || p.len() > 255)
        {
            return bad("password must be 1 to 255 bytes".into());
        }
        if let OpenPlanes::List(planes) = &self.open_planes
            && planes.is_empty()
        {
            return bad("open-planes lists no plane".into());
        }
        if self.time_limit.is_some_and(|t| t.is_zero()) {
            return bad("time-limit of 0 is written as none".into());
        }
        if self.retail_stall_speeds || tore_sim::flight::retail_stall_speeds() {
            return Err(HostError::RetailStallSpeeds);
        }
        Ok(())
    }
}

/// Why a host could not start.
#[derive(Debug)]
pub enum HostError {
    /// A setting out of range, named.
    Setting(String),
    /// The retail stall-speed switch is on.
    RetailStallSpeeds,
    /// The mission could not be built from the import.
    Mission(String),
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Setting(text) => write!(f, "setting: {text}"),
            Self::RetailStallSpeeds => f.write_str(
                "the retail stall-speed switch (--retail-stall-speeds or \
                 TORE_RETAIL_STALL_SPEEDS) is on; every machine in a session must fly one \
                 configuration, so a host does not start with it",
            ),
            Self::Mission(text) => write!(f, "the mission cannot be built: {text}"),
        }
    }
}

impl std::error::Error for HostError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn build() -> BuildId {
        BuildId {
            version: "0.1.3".into(),
            commit: "abc".into(),
            release: true,
        }
    }

    #[test]
    fn defaults_follow_the_server_guide_and_validate() {
        let config = HostConfig::new(build());
        assert_eq!(config.max_players, 30);
        assert_eq!(config.snapshot_rate, 30);
        assert_eq!(config.ticks_per_snapshot(), 4);
        assert_eq!(config.empty_timeout, Duration::from_secs(60));
        assert_eq!(config.restart_delay, Duration::from_secs(30));
        assert_eq!(config.start, StartMode::FirstPlayer);
        assert_eq!(config.after_end, AfterEnd::Restart);
        assert!(matches!(config.entropy, Entropy::System));
        config.validate().unwrap();
    }

    #[test]
    fn settings_out_of_range_are_refused() {
        let base = HostConfig::new(build());
        for config in [
            HostConfig {
                max_players: 0,
                ..base.clone()
            },
            HostConfig {
                max_players: 31,
                ..base.clone()
            },
            HostConfig {
                snapshot_rate: 25,
                ..base.clone()
            },
            HostConfig {
                open_planes: OpenPlanes::List(vec![]),
                ..base.clone()
            },
            HostConfig {
                time_limit: Some(Duration::ZERO),
                ..base.clone()
            },
        ] {
            assert!(matches!(config.validate(), Err(HostError::Setting(_))));
        }
        let stall = HostConfig {
            retail_stall_speeds: true,
            ..base
        };
        assert!(matches!(
            stall.validate(),
            Err(HostError::RetailStallSpeeds)
        ));
    }

    #[test]
    fn builds_match_by_commit_or_by_a_release_version() {
        let release = build();
        assert!(release.matches("0.1.3", "abc"));
        assert!(release.matches("0.1.3", "def"), "another tagged build");
        assert!(!release.matches("0.1.2", "def"));
        assert!(!release.matches("0.1.3-4-gdef", "def"));
        let dev = BuildId {
            release: false,
            ..build()
        };
        assert!(dev.matches("0.1.3-4-gabc", "abc"));
        assert!(!dev.matches("0.1.3", "def"));
        let unknown = BuildId {
            commit: "unknown".into(),
            release: false,
            ..build()
        };
        assert!(!unknown.matches("0.1.3", "unknown"));
    }
}
