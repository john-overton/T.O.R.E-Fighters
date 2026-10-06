//! The host's settings: what the dedicated server's configuration file sets
//! (docs/DEDICATED-SERVER.md, "The configuration file") and the build the
//! host runs, with the rules each setting must keep.

use crate::settings::{self, Mode};
use std::fmt;
use std::time::Duration;
use tore_net::Entropy;

/// Players a host seats at most (the wire's seat limit; John's default).
pub const MAX_PLAYERS: usize = 30;
/// The snapshot rates a host offers, a second.
pub const SNAPSHOT_RATES: [u32; 8] = [10, 12, 15, 20, 24, 30, 40, 60];
/// The default snapshot rate: every second tick (John, 2026-10-06; 30
/// before). The lower rates stay for a weak uplink.
pub const DEFAULT_SNAPSHOT_RATE: u32 = 60;

/// Which planes humans may take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenPlanes {
    /// The mode's planes (the default): every friendly plane in co-op, every
    /// plane of both sides in PvP (slice F2-1).
    Friendly,
    /// Every plane, both sides, in either mode.
    All,
    /// These plane numbers only, in either mode.
    List(Vec<u32>),
}

impl OpenPlanes {
    /// Whether `plane`, on the friendly side when `friendly`, is open in
    /// co-op.
    pub fn allows(&self, plane: u32, friendly: bool) -> bool {
        self.allows_in(plane, friendly, Mode::Coop)
    }

    /// Whether `plane`, on the friendly side when `friendly`, is open in
    /// `mode`: [`OpenPlanes::Friendly`] follows the mode.
    pub fn allows_in(&self, plane: u32, friendly: bool, mode: Mode) -> bool {
        match self {
            Self::Friendly => friendly || mode == Mode::Pvp,
            Self::All => true,
            Self::List(planes) => planes.contains(&plane),
        }
    }
}

/// Who wears the crown (stage F phase 2, slice F2-1; docs/ARCHITECTURE.md,
/// "The King, the crown and the house").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CrownRule {
    /// Nobody: a dedicated server's default, whose mission and settings are
    /// its file's and whose start rule is its own.
    #[default]
    None,
    /// The house when it joins ([`HostConfig::house`]), else the first
    /// player to join; when the King leaves, the longest-connected player.
    /// A game a player hosts, and a dedicated server's `king first-player`.
    FirstPlayer,
}

/// When the mission starts flying.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartMode {
    /// The lobby waits, the mission not flying and sending no snapshots,
    /// until the first player holding a slot is ready (the default; the
    /// dedicated server's `start first-player`). Later ready players join
    /// in flight.
    FirstPlayer,
    /// It flies from the start; players who get ready join in flight.
    Now,
    /// The King starts it, once every player holding a slot is ready: a game
    /// a player hosts ([`HostConfig::house`]). A dedicated server with a King
    /// keeps its own rule, which gives way to the King's start while a King
    /// is connected.
    King,
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
    /// The house's address (stage F phase 2; until then `king`): in a game a
    /// player hosts, the hosting player's own connection over the
    /// in-process link (`tore_net::LINK_ADDRESS`), which no UDP sender can
    /// have. The house wears the crown when it joins, and its leaving ends
    /// the game for everyone (until stage K migrates the host). `None` on a
    /// dedicated server. Set with [`StartMode::King`] and
    /// [`CrownRule::FirstPlayer`].
    pub house: Option<std::net::SocketAddr>,
    /// Who wears the crown: only the King changes the mission and the
    /// settings, locks slots, starts and ends the mission, kicks and passes
    /// the crown.
    pub crown: CrownRule,
    /// The King may not change the mission or the settings (a dedicated
    /// server's `king-mission locked`): its file's stay. Start, kick, slot
    /// locks and the crown still work.
    pub mission_locked: bool,
    /// The King's settings the configuration sets beyond the fields above
    /// (a dedicated server's file; [`crate::settings`]), by number: never
    /// the player limit, the password or the time limit, which are fields
    /// of their own.
    pub settings: Vec<(u8, u32)>,
    /// The King's `public` visibility can list the game on the Internet
    /// Lobby: a game a player hosts, whose hosting thread lists it. A
    /// dedicated server's listing is its operator's `broadcast`.
    pub listable: bool,
    /// The host's own content (stage L): what every player's is compared
    /// with. `None` computes it from the resources the host is given when it
    /// starts, with the source the pack's entry gives; a caller that knows
    /// the data folder (the dedicated server, the hosting game) may pass one
    /// it computed already, with the source read from the import report too.
    pub content: Option<std::sync::Arc<super::content::GameContent>>,
}

impl HostConfig {
    /// The server guide's defaults for `build`.
    pub fn new(build: BuildId) -> Self {
        Self {
            name: "T.O.R.E server".into(),
            password: None,
            max_players: MAX_PLAYERS,
            open_planes: OpenPlanes::Friendly,
            snapshot_rate: DEFAULT_SNAPSHOT_RATE,
            start: StartMode::FirstPlayer,
            time_limit: None,
            empty_timeout: Duration::from_secs(60),
            after_end: AfterEnd::Restart,
            restart_delay: Duration::from_secs(30),
            build,
            entropy: Entropy::System,
            retail_stall_speeds: false,
            house: None,
            crown: CrownRule::None,
            mission_locked: false,
            settings: Vec::new(),
            listable: false,
            content: None,
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
        if (self.start == StartMode::King) != self.house.is_some() {
            return bad("the King's start is a game a player hosts, which has a house".into());
        }
        if self.house.is_some() && self.crown != CrownRule::FirstPlayer {
            return bad("the house of a game a player hosts wears the crown".into());
        }
        self.check_settings().map_err(HostError::Setting)?;
        if self.retail_stall_speeds || tore_sim::flight::retail_stall_speeds() {
            return Err(HostError::RetailStallSpeeds);
        }
        Ok(())
    }

    /// Checks [`HostConfig::settings`]: each a setting of the registry with
    /// an allowed value, given once, and not one with a field of its own;
    /// `public` only for a game that can be listed; a setting that applies
    /// only in PvP only with `mode pvp` (agent decision: a co-op server would
    /// ignore it, so it is refused rather than kept unseen).
    pub fn check_settings(&self) -> Result<(), String> {
        let mut seen = std::collections::BTreeSet::new();
        for &(number, value) in &self.settings {
            let Some(setting) = settings::setting(number) else {
                return Err(format!("there is no setting {number}"));
            };
            if matches!(
                number,
                settings::number::MAX_PLAYERS
                    | settings::number::PASSWORD
                    | settings::number::TIME_LIMIT
                    | settings::number::SNAPSHOT_RATE
            ) {
                return Err(format!("{} is set by its own field", setting.name));
            }
            if let Some(why) = settings::refusal(number, value) {
                return Err(why.trim_end_matches('.').to_owned());
            }
            if !seen.insert(number) {
                return Err(format!("{} is given twice", setting.name));
            }
            if number == settings::number::VISIBILITY
                && value == settings::Visibility::Public.value()
                && !self.listable
            {
                return Err(PUBLIC_IS_BROADCAST.trim_end_matches('.').to_owned());
            }
        }
        let pvp = self
            .settings
            .iter()
            .any(|&(n, v)| n == settings::number::MODE && v == Mode::Pvp.value());
        if !pvp
            && let Some(setting) = self
                .settings
                .iter()
                .filter_map(|&(n, _)| settings::setting(n))
                .find(|s| s.pvp_only)
        {
            return Err(format!("{} applies only in PvP (mode pvp)", setting.name));
        }
        Ok(())
    }
}

/// Why a game that cannot be listed refuses `public` visibility: a
/// dedicated server lists itself by its operator's `broadcast`.
pub const PUBLIC_IS_BROADCAST: &str =
    "A dedicated server lists itself on the Internet Lobby only by its operator's `broadcast on`.";

/// Why a dedicated server refuses the King's change of the snapshot rate:
/// its operator's file sets it (slice R1).
pub const RATE_IS_THE_FILES: &str =
    "A dedicated server's snapshot rate is set by its operator's `snapshot-rate`.";

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
        assert_eq!(config.snapshot_rate, 60);
        assert_eq!(config.ticks_per_snapshot(), 2);
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
            HostConfig {
                start: StartMode::King,
                ..base.clone()
            },
            HostConfig {
                house: Some("[100::]:0".parse().unwrap()),
                crown: CrownRule::FirstPlayer,
                ..base.clone()
            },
            HostConfig {
                house: Some("[100::]:0".parse().unwrap()),
                start: StartMode::King,
                ..base.clone()
            },
        ] {
            assert!(matches!(config.validate(), Err(HostError::Setting(_))));
        }
        // A game a player hosts, and a server whose first player is King.
        HostConfig {
            house: Some("[100::]:0".parse().unwrap()),
            start: StartMode::King,
            crown: CrownRule::FirstPlayer,
            ..base.clone()
        }
        .validate()
        .unwrap();
        HostConfig {
            crown: CrownRule::FirstPlayer,
            ..base.clone()
        }
        .validate()
        .unwrap();
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
    fn the_configurations_settings_are_checked() {
        use settings::number;
        let with = |values: &[(u8, u32)]| HostConfig {
            settings: values.to_vec(),
            ..HostConfig::new(build())
        };
        with(&[(number::MODE, 1), (number::KILL_LIMIT, 3)])
            .validate()
            .unwrap();
        with(&[(number::FRIENDLY_FIRE, 0), (number::VISIBILITY, 0)])
            .validate()
            .unwrap();
        let refused = |values: &[(u8, u32)]| match with(values).validate() {
            Err(HostError::Setting(text)) => text,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            refused(&[(number::KILL_LIMIT, 3)]),
            "kill-limit applies only in PvP (mode pvp)"
        );
        assert_eq!(
            refused(&[(number::LIVES, 11)]),
            "lives is 0 to 10 or unlimited"
        );
        assert_eq!(
            refused(&[(number::MAX_PLAYERS, 4)]),
            "max-players is set by its own field"
        );
        assert_eq!(
            refused(&[(number::RESPAWN, 1), (number::RESPAWN, 2)]),
            "respawn is given twice"
        );
        assert!(refused(&[(number::VISIBILITY, 2)]).contains("broadcast on"));
        HostConfig {
            listable: true,
            ..with(&[(number::VISIBILITY, 2)])
        }
        .validate()
        .unwrap();
        // Open planes follow the mode by default.
        assert!(!OpenPlanes::Friendly.allows_in(7, false, Mode::Coop));
        assert!(OpenPlanes::Friendly.allows_in(7, false, Mode::Pvp));
        assert!(!OpenPlanes::List(vec![1]).allows_in(7, false, Mode::Pvp));
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
