//! Reports into daily counts ("Reports" in the master protocol; what is kept
//! is the operations guide's "What the master keeps").
//!
//! A Report adds to the day's counts and is then forgotten. No address is
//! ever kept with them. Distinct installs are counted through a salted hash
//! of the install id: the salt is drawn each day and never written down, so
//! an install cannot be followed from one day to the next, and the set of
//! hashes is dropped with the day.
//!
//! The counts are names and numbers (`reports 12`, `path.relay 3`), so the
//! day's file is one `name<TAB>number` line each and a master that restarts
//! during a day reads it back and carries on. Installs counted before the
//! restart cannot be told from those after it, so a restart can count an
//! install twice that day (agent decision: the figure is a daily estimate).

use std::collections::hash_map::RandomState;
use std::collections::{BTreeMap, HashSet};
use std::hash::{BuildHasher, DefaultHasher, Hash, Hasher};

use tore_net::Entropy;
use tore_net::SplitMix64;
use tore_net::master::{MappingType, Path, PortMapping, Report, Role};

/// Distinct installs remembered in a day at most; past it the figure stops
/// rising (agent decision: about 2 MB).
pub const MAX_INSTALLS: usize = 262_144;
/// Distinct game versions counted by name in a day; the rest count as
/// `version.other`.
pub const MAX_VERSIONS: usize = 64;
/// Reports from the master's own load tool carry this game version and are
/// not counted.
pub const FLOOD_VERSION: &str = "flood";

#[derive(Debug, Clone)]
enum Salt {
    System(RandomState),
    Seeded(u64),
}

impl Salt {
    fn hash(&self, install: u64) -> u64 {
        match self {
            Self::System(state) => state.hash_one(install),
            Self::Seeded(key) => {
                let mut hasher = DefaultHasher::new();
                (key, install).hash(&mut hasher);
                hasher.finish()
            }
        }
    }
}

/// One day's counts.
#[derive(Debug, Clone)]
pub struct Telemetry {
    day: i64,
    entropy: Entropy,
    rng: SplitMix64,
    salt: Salt,
    installs: HashSet<u64>,
    /// Installs already in the day's file when the master started.
    installs_before: u64,
    counts: BTreeMap<String, u64>,
}

impl Telemetry {
    /// An empty day `day` (days since 1970-01-01, UTC).
    pub fn new(day: i64, entropy: Entropy) -> Self {
        let mut rng = SplitMix64::new(match entropy {
            Entropy::Seeded(seed) => seed,
            Entropy::System => 0,
        });
        let salt = Self::draw(entropy, &mut rng);
        Self {
            day,
            entropy,
            rng,
            salt,
            installs: HashSet::new(),
            installs_before: 0,
            counts: BTreeMap::new(),
        }
    }

    fn draw(entropy: Entropy, rng: &mut SplitMix64) -> Salt {
        match entropy {
            Entropy::System => Salt::System(RandomState::new()),
            Entropy::Seeded(_) => Salt::Seeded(rng.next_u64()),
        }
    }

    /// The day being counted.
    pub fn day(&self) -> i64 {
        self.day
    }

    /// Carries on from the day's file, `name<TAB>number` lines. Lines that do
    /// not read are skipped.
    pub fn resume(&mut self, text: &str) {
        for line in text.lines() {
            let Some((name, number)) = line.split_once('\t') else {
                continue;
            };
            let Ok(number) = number.trim().parse::<u64>() else {
                continue;
            };
            if name == "installs" {
                self.installs_before = number;
            } else {
                *self.counts.entry(name.to_owned()).or_default() += number;
            }
        }
    }

    /// Adds a report. Returns false for one that is not counted (the load
    /// tool's).
    pub fn record(&mut self, report: &Report) -> bool {
        if report.game_version == FLOOD_VERSION {
            return false;
        }
        if self.installs.len() < MAX_INSTALLS {
            self.installs.insert(self.salt.hash(report.install_id));
        }
        self.add("reports", 1);
        let role = match report.role {
            Role::Player => "player",
            Role::HostingGame => "hosting-game",
            Role::DedicatedServer => "dedicated-server",
        };
        self.add(&format!("role.{role}"), 1);
        self.add(&format!("role.{role}.minutes"), u64::from(report.minutes));
        self.add(
            &format!(
                "minutes.{}",
                bucket(u64::from(report.minutes), &MINUTE_BUCKETS)
            ),
            1,
        );
        self.add(
            &format!(
                "humans.{}",
                bucket(u64::from(report.humans), &HUMAN_BUCKETS)
            ),
            1,
        );
        let version = self.version_name(&report.game_version);
        self.add(&format!("version.{version}"), 1);
        self.add(&format!("platform.{}", report.platform), 1);
        self.add(&format!("mapping.{}", mapping_name(report.mapping)), 1);
        self.add(
            &format!("port-mapping.{}", port_mapping_name(report.port_mapping)),
            1,
        );
        if report.role == Role::Player {
            let path = path_name(report.path);
            self.add(&format!("path.{path}"), 1);
            self.add(
                &format!(
                    "connect.{path}.{}",
                    bucket(u64::from(report.connect_tenths), &CONNECT_BUCKETS)
                ),
                1,
            );
            self.add("relayed-kb", u64::from(report.relayed_kb));
        } else {
            for (path, count) in Path::ALL.iter().zip(report.players_by_path) {
                self.add(
                    &format!("hosted-path.{}", path_name(*path)),
                    u64::from(count),
                );
            }
        }
        self.add("migrations", u64::from(report.migrations));
        self.add("migrations-failed", u64::from(report.failed_migrations));
        true
    }

    /// Distinct installs counted today.
    pub fn installs(&self) -> u64 {
        self.installs_before + self.installs.len() as u64
    }

    /// The day's counts as the file holds them, `installs` first.
    pub fn to_tsv(&self) -> String {
        let mut text = format!("installs\t{}\n", self.installs());
        for (name, number) in &self.counts {
            text.push_str(&format!("{name}\t{number}\n"));
        }
        text
    }

    /// Starts day `day`: a new salt, no counts. Returns the finished day's
    /// file text, or `None` when `day` is the day being counted.
    pub fn roll(&mut self, day: i64) -> Option<String> {
        if day == self.day {
            return None;
        }
        let finished = self.to_tsv();
        self.day = day;
        self.salt = Self::draw(self.entropy, &mut self.rng);
        self.installs.clear();
        self.installs_before = 0;
        self.counts.clear();
        Some(finished)
    }

    fn add(&mut self, name: &str, by: u64) {
        let count = self.counts.entry(name.to_owned()).or_default();
        *count = count.saturating_add(by);
    }

    /// A version as a count's name: printable, no spaces or tabs; past
    /// [`MAX_VERSIONS`] new names count as `other`.
    fn version_name(&self, version: &str) -> String {
        let clean: String = version
            .chars()
            .map(|c| if c.is_ascii_graphic() { c } else { '_' })
            .collect();
        let clean = if clean.is_empty() {
            "none".into()
        } else {
            clean
        };
        let known = self.counts.contains_key(&format!("version.{clean}"));
        let versions = self
            .counts
            .keys()
            .filter(|k| k.starts_with("version."))
            .count();
        if known || versions < MAX_VERSIONS {
            clean
        } else {
            "other".into()
        }
    }
}

const MINUTE_BUCKETS: [(u64, &str); 5] = [
    (5, "under-5"),
    (15, "5-15"),
    (30, "15-30"),
    (60, "30-60"),
    (u64::MAX, "60-up"),
];
const HUMAN_BUCKETS: [(u64, &str); 6] = [
    (2, "1"),
    (3, "2"),
    (5, "3-4"),
    (9, "5-8"),
    (17, "9-16"),
    (u64::MAX, "17-up"),
];
const CONNECT_BUCKETS: [(u64, &str); 5] = [
    (5, "under-0.5s"),
    (10, "0.5-1s"),
    (30, "1-3s"),
    (50, "3-5s"),
    (u64::MAX, "5s-up"),
];

/// The name of the first bucket whose bound is over `value`.
fn bucket(value: u64, buckets: &[(u64, &'static str)]) -> &'static str {
    buckets
        .iter()
        .find(|(below, _)| value < *below)
        .map_or("other", |(_, name)| name)
}

fn path_name(path: Path) -> &'static str {
    match path {
        Path::LocalNetwork => "local",
        Path::ByAddress => "address",
        Path::MappedPort => "mapped",
        Path::Ipv6 => "ipv6",
        Path::Punched => "punched",
        Path::Relay => "relay",
    }
}

fn mapping_name(mapping: MappingType) -> &'static str {
    match mapping {
        MappingType::Unknown => "unknown",
        MappingType::NoTranslation => "none",
        MappingType::SamePort => "same-port",
        MappingType::PortPerDestination => "port-per-destination",
    }
}

fn port_mapping_name(mapping: PortMapping) -> &'static str {
    match mapping {
        PortMapping::NotTried => "not-tried",
        PortMapping::Upnp => "upnp",
        PortMapping::NatPmp => "nat-pmp",
        PortMapping::Pcp => "pcp",
        PortMapping::Failed => "failed",
        PortMapping::SecondRouter => "second-router",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn report(install_id: u64, role: Role) -> Report {
        Report {
            install_id,
            role,
            game_version: "0.1.3".into(),
            platform: 3,
            minutes: 20,
            humans: 4,
            path: Path::Relay,
            connect_tenths: 35,
            mapping: MappingType::SamePort,
            port_mapping: PortMapping::Upnp,
            relayed_kb: 1_000,
            players_by_path: [1, 0, 2, 0, 3, 1],
            migrations: 1,
            failed_migrations: 0,
        }
    }

    #[test]
    fn reports_become_counts_and_installs_are_counted_once() {
        let mut t = Telemetry::new(20_000, Entropy::Seeded(1));
        assert!(t.record(&report(5, Role::Player)));
        assert!(t.record(&report(5, Role::Player)));
        assert!(t.record(&report(6, Role::HostingGame)));
        let mut flood = report(7, Role::Player);
        flood.game_version = FLOOD_VERSION.into();
        assert!(!t.record(&flood));
        let text = t.to_tsv();
        for line in [
            "installs\t2",
            "reports\t3",
            "role.player\t2",
            "role.player.minutes\t40",
            "role.hosting-game\t1",
            "minutes.15-30\t3",
            "humans.3-4\t3",
            "path.relay\t2",
            "connect.relay.3-5s\t2",
            "relayed-kb\t2000",
            "hosted-path.local\t1",
            "hosted-path.ipv6\t0",
            "hosted-path.punched\t3",
            "version.0.1.3\t3",
            "platform.3\t3",
            "mapping.same-port\t3",
            "port-mapping.upnp\t3",
            "migrations\t3",
        ] {
            assert!(
                text.lines().any(|l| l == line),
                "{line} missing from\n{text}"
            );
        }
        assert!(!text.contains("flood"));
    }

    #[test]
    fn a_new_day_starts_empty_with_a_new_salt_and_a_restart_resumes() {
        let mut t = Telemetry::new(1, Entropy::Seeded(2));
        t.record(&report(5, Role::Player));
        let first_hash = t.salt.hash(5);
        assert_eq!(t.roll(1), None);
        let finished = t.roll(2).unwrap();
        assert!(finished.starts_with("installs\t1\nconnect."));
        assert_ne!(t.salt.hash(5), first_hash);
        assert_eq!(t.to_tsv(), "installs\t0\n");
        let mut resumed = Telemetry::new(1, Entropy::Seeded(3));
        resumed.resume(&finished);
        resumed.record(&report(9, Role::Player));
        assert_eq!(resumed.installs(), 2);
        assert!(resumed.to_tsv().contains("reports\t2\n"));
    }

    #[test]
    fn version_names_are_clean_and_bounded() {
        let mut t = Telemetry::new(1, Entropy::Seeded(2));
        let mut r = report(1, Role::Player);
        r.game_version = "0.1\t3 x".into();
        t.record(&r);
        assert!(t.to_tsv().contains("version.0.1_3_x\t1"));
        for n in 0..MAX_VERSIONS + 3 {
            r.game_version = format!("v{n}");
            t.record(&r);
        }
        assert!(t.to_tsv().contains("version.other\t"));
    }
}
