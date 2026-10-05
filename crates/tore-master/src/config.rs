//! The configuration file: one setting per line, a name and a value, `#` to
//! the end of the line is a comment. Every setting, default and range is in
//! docs/MASTER-SERVER.md, "The configuration file". An unknown name, a value
//! out of range and a name that appears twice are refused with the line, as
//! the dedicated server's file is.

use std::collections::BTreeSet;
use std::fmt;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tore_net::Listen;
use tore_net::master::{MASTER_PORT, PROBE_PORT};

use crate::limits::Rate;
use crate::master::Settings;

/// Every setting's name, in the guide's order.
pub const SETTINGS: [&str; 19] = [
    "listen",
    "port",
    "probe-port",
    "state-dir",
    "max-listings",
    "listings-per-source",
    "heartbeat",
    "keep",
    "expiry",
    "browse-rate",
    "introduce-rate",
    "answer-rate",
    "relay",
    "relay-channels",
    "relay-channels-per-source",
    "relay-rate",
    "relay-month-gb",
    "telemetry",
    "status-interval",
];

/// A master's configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Where both ports listen.
    pub listen: Listen,
    /// The main UDP port.
    pub port: u16,
    /// The second port of the mapping test; 0 turns the test off.
    pub probe_port: u16,
    /// Where statistics, telemetry counts and the relay's monthly figure are
    /// written.
    pub state_dir: PathBuf,
    /// Seconds between status lines; 0 for none.
    pub status_interval: u32,
    /// The master's numbers.
    pub settings: Settings,
}

/// A line the file cannot take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    /// The line, from 1.
    pub line: usize,
    /// Why.
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// The defaults; the state folder is `base`, the configuration file's
    /// folder.
    pub fn defaults(base: &Path) -> Self {
        Self {
            listen: Listen::Any,
            port: MASTER_PORT,
            probe_port: PROBE_PORT,
            state_dir: base.to_path_buf(),
            status_interval: 60,
            settings: Settings::default(),
        }
    }

    /// Reads a configuration file's text; a relative `state-dir` is taken
    /// against `base`.
    pub fn parse(text: &str, base: &Path) -> Result<Self, ConfigError> {
        let mut config = Self::defaults(base);
        let mut seen = BTreeSet::new();
        let mut last_line = 0;
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            last_line = line;
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
        config.check().map_err(|message| ConfigError {
            line: last_line,
            message,
        })?;
        Ok(config)
    }

    fn set(&mut self, name: &str, value: &str, base: &Path) -> Result<(), String> {
        let s = &mut self.settings;
        match name {
            "listen" => {
                self.listen =
                    if value == "any" {
                        Listen::Any
                    } else {
                        Listen::Address(value.parse::<IpAddr>().map_err(|_| {
                            format!("`listen` is `any` or one address, not `{value}`")
                        })?)
                    }
            }
            "port" => self.port = number(name, value, 1, 65_535)? as u16,
            "probe-port" => self.probe_port = number(name, value, 0, 65_535)? as u16,
            "state-dir" => self.state_dir = base.join(value),
            "max-listings" => s.max_listings = number(name, value, 1, 100_000)? as usize,
            "listings-per-source" => s.listings_per_source = number(name, value, 1, 1_000)?,
            "heartbeat" => s.heartbeat = seconds(name, value, 10, 120)?,
            "keep" => s.keep = seconds(name, value, 5, 60)?,
            "expiry" => s.expiry = seconds(name, value, 30, 255)?,
            "browse-rate" => {
                let rate = number(name, value, 1, 1_000)?;
                s.rates.query = Rate::per_second(rate, rate * 2);
            }
            "introduce-rate" => {
                let rate = number(name, value, 1, 100)?;
                s.rates.introduce = Rate::per_second(rate, rate);
            }
            "answer-rate" => s.answer_rate = number(name, value, 100, 100_000)?,
            "relay" => s.relay.on = switch(name, value)?,
            "relay-channels" => s.relay.channels = number(name, value, 0, 1_000)?,
            "relay-channels-per-source" => {
                s.relay.channels_per_source = number(name, value, 1, 30)?
            }
            "relay-rate" => s.relay.rate_kb = number(name, value, 8, 1_024)?,
            "relay-month-gb" => s.relay.month_gb = number(name, value, 0, 100_000)?,
            "telemetry" => s.telemetry = switch(name, value)?,
            "status-interval" => self.status_interval = number(name, value, 0, 3_600)?,
            _ => unreachable!("every name in SETTINGS is handled"),
        }
        Ok(())
    }

    /// The rules between settings.
    fn check(&self) -> Result<(), String> {
        let s = &self.settings;
        if s.expiry < s.heartbeat * 2 {
            return Err(format!(
                "`expiry` ({} s) must be at least twice `heartbeat` ({} s)",
                s.expiry.as_secs(),
                s.heartbeat.as_secs()
            ));
        }
        if self.probe_port != 0 && self.probe_port == self.port {
            return Err("`probe-port` must differ from `port` (or be 0)".into());
        }
        Ok(())
    }

    /// What the master will do, for `--check-config` and the start lines:
    /// the sockets, then every setting that differs from its default.
    pub fn describe(&self) -> Vec<String> {
        let listen = match self.listen {
            Listen::Any => "every IPv4 and IPv6 address".to_string(),
            Listen::Address(ip) => ip.to_string(),
        };
        let mut lines = vec![format!("listen {listen}, port {}", self.port)];
        lines.push(if self.probe_port == 0 {
            "probe port off: games get no router test from this master".into()
        } else {
            format!("probe port {}", self.probe_port)
        });
        lines.push(format!("state folder {}", self.state_dir.display()));
        lines.push(self.relay_line());
        let defaults = Self::defaults(&self.state_dir);
        let (s, d) = (&self.settings, &defaults.settings);
        let mut differ = Vec::new();
        let mut note = |name: &str, value: String, default: String| {
            if value != default {
                differ.push(format!("{name} {value}"));
            }
        };
        note(
            "max-listings",
            s.max_listings.to_string(),
            d.max_listings.to_string(),
        );
        note(
            "listings-per-source",
            s.listings_per_source.to_string(),
            d.listings_per_source.to_string(),
        );
        note("heartbeat", secs(s.heartbeat), secs(d.heartbeat));
        note("keep", secs(s.keep), secs(d.keep));
        note("expiry", secs(s.expiry), secs(d.expiry));
        note(
            "browse-rate",
            s.rates.query.count.to_string(),
            d.rates.query.count.to_string(),
        );
        note(
            "introduce-rate",
            s.rates.introduce.count.to_string(),
            d.rates.introduce.count.to_string(),
        );
        note(
            "answer-rate",
            s.answer_rate.to_string(),
            d.answer_rate.to_string(),
        );
        note("relay", on_off(s.relay.on), on_off(d.relay.on));
        note(
            "relay-channels",
            s.relay.channels.to_string(),
            d.relay.channels.to_string(),
        );
        note(
            "relay-channels-per-source",
            s.relay.channels_per_source.to_string(),
            d.relay.channels_per_source.to_string(),
        );
        note(
            "relay-rate",
            s.relay.rate_kb.to_string(),
            d.relay.rate_kb.to_string(),
        );
        note(
            "relay-month-gb",
            s.relay.month_gb.to_string(),
            d.relay.month_gb.to_string(),
        );
        note("telemetry", on_off(s.telemetry), on_off(d.telemetry));
        note(
            "status-interval",
            self.status_interval.to_string(),
            defaults.status_interval.to_string(),
        );
        if differ.is_empty() {
            lines.push("every other setting at its default".into());
        } else {
            lines.push(format!(
                "settings changed from the defaults: {}",
                differ.join(", ")
            ));
        }
        lines
    }
}

impl Config {
    /// What the relay will do, in one plain line: whether it is active, and
    /// its limits (John, 2026-10-05: say plainly that it is).
    pub fn relay_line(&self) -> String {
        let r = &self.settings.relay;
        if !r.on {
            return "relay OFF: players who cannot connect directly cannot join".into();
        }
        if r.channels == 0 {
            return "relay on but relay-channels is 0: every relay request is refused".into();
        }
        if r.month_gb == 0 {
            return "relay on but relay-month-gb is 0: every relay request is refused".into();
        }
        let refuse = u64::from(r.month_gb) * crate::relay::REFUSE_PERCENT / 100;
        format!(
            "relay ACTIVE: up to {} channels, {} per player address, {} KB/s each way per channel; \
             {} GB a month (new channels refused from {refuse} GB, open ones closed at {} GB)",
            r.channels, r.channels_per_source, r.rate_kb, r.month_gb, r.month_gb
        )
    }
}

fn secs(d: Duration) -> String {
    d.as_secs().to_string()
}

fn on_off(on: bool) -> String {
    if on { "on" } else { "off" }.into()
}

fn number(name: &str, value: &str, low: u32, high: u32) -> Result<u32, String> {
    let digits: String = value.chars().filter(|c| *c != ',' && *c != '_').collect();
    match digits.parse::<u32>() {
        Ok(n) if (low..=high).contains(&n) => Ok(n),
        _ => Err(format!(
            "`{name}` is a number from {low} to {high}, not `{value}`"
        )),
    }
}

fn seconds(name: &str, value: &str, low: u32, high: u32) -> Result<Duration, String> {
    number(name, value, low, high).map(|n| Duration::from_secs(u64::from(n)))
}

fn switch(name: &str, value: &str) -> Result<bool, String> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(format!("`{name}` is `on` or `off`, not `{value}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn base() -> PathBuf {
        PathBuf::from("/etc/tore-master")
    }

    #[test]
    fn an_empty_file_gives_the_defaults() {
        let config = Config::parse("# nothing\n\n", &base()).unwrap();
        assert_eq!(config, Config::defaults(&base()));
        assert_eq!(config.port, 26901);
        assert_eq!(config.probe_port, 26902);
        assert_eq!(config.settings.max_listings, 2_000);
        assert_eq!(config.settings.relay.month_gb, 800);
        assert_eq!(
            config.describe().last().unwrap(),
            "every other setting at its default"
        );
    }

    #[test]
    fn the_guides_examples_read() {
        let public = "# /etc/tore-master/master.conf\nlisten any\nport 26901\nprobe-port 26902\nstate-dir /var/lib/tore-master\nrelay-month-gb 800\n";
        let config = Config::parse(public, &base()).unwrap();
        assert_eq!(config.state_dir, PathBuf::from("/var/lib/tore-master"));
        let local = "listen 127.0.0.1\nport 26911\nprobe-port 26912\n";
        let config = Config::parse(local, &base()).unwrap();
        assert_eq!(config.listen, Listen::Address(Ipv4Addr::LOCALHOST.into()));
        assert_eq!((config.port, config.probe_port), (26911, 26912));
    }

    #[test]
    fn every_setting_reads() {
        let text = "listen ::1\nport 1000\nprobe-port 0\nstate-dir state\nmax-listings 100,000\nlistings-per-source 3\nheartbeat 60\nkeep 20\nexpiry 255\nbrowse-rate 5\nintroduce-rate 2\nanswer-rate 100\nrelay off\nrelay-channels 0\nrelay-channels-per-source 30\nrelay-rate 8\nrelay-month-gb 0\ntelemetry off\nstatus-interval 0\n";
        let config = Config::parse(text, &base()).unwrap();
        let s = &config.settings;
        assert_eq!(config.state_dir, base().join("state"));
        assert_eq!(s.max_listings, 100_000);
        assert_eq!(s.rates.query, Rate::per_second(5, 10));
        assert_eq!(s.rates.introduce, Rate::per_second(2, 2));
        assert!(!s.relay.on && !s.telemetry);
        assert_eq!(config.status_interval, 0);
        let described = config.describe().join("\n");
        assert!(described.contains("relay OFF"), "{described}");
        assert!(described.contains("probe port off"));
        assert!(described.contains("telemetry off"));
    }

    #[test]
    fn the_relay_line_says_plainly_whether_it_is_active() {
        let config = Config::parse("", &base()).unwrap();
        assert_eq!(
            config.relay_line(),
            "relay ACTIVE: up to 64 channels, 2 per player address, 64 KB/s each way per channel; \
             800 GB a month (new channels refused from 760 GB, open ones closed at 800 GB)"
        );
        assert!(config.describe().contains(&config.relay_line()));
        for (text, words) in [
            ("relay off\n", "relay OFF"),
            ("relay-channels 0\n", "relay-channels is 0"),
            ("relay-month-gb 0\n", "relay-month-gb is 0"),
        ] {
            let line = Config::parse(text, &base()).unwrap().relay_line();
            assert!(line.contains(words), "{text}: {line}");
        }
    }

    #[test]
    fn bad_lines_are_refused_with_their_number() {
        for (text, line, words) in [
            ("port 26901\nsize 3\n", 2, "not a setting"),
            ("port 1\nport 2\n", 2, "appears twice"),
            ("port\n", 1, "needs a value"),
            ("port 0\n", 1, "from 1 to 65535"),
            ("expiry 600\n", 1, "from 30 to 255"),
            ("relay maybe\n", 1, "`on` or `off`"),
            ("listen everywhere\n", 1, "`any` or one address"),
            ("heartbeat 60\nexpiry 90\n", 2, "at least twice"),
            ("port 5\nprobe-port 5\n", 2, "must differ"),
        ] {
            let error = Config::parse(text, &base()).unwrap_err();
            assert_eq!(error.line, line, "{text}");
            assert!(error.message.contains(words), "{text}: {error}");
        }
    }
}
