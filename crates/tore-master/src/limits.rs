//! The master's limits ("Limits" in the master protocol): token buckets per
//! source and in all, and the table of sources with its bound.
//!
//! A **source** is an IPv4 address or an IPv6 /64 network: one home is given
//! a whole /64, so counting single IPv6 addresses would let one home pass any
//! limit. The table remembers at most [`Settings::max_sources`] of them and
//! forgets the least recently heard first.
//!
//! Each bucket is a generic cell rate algorithm: it keeps the time at which
//! it will be empty again, so it needs no refill step, takes integer time
//! only, and is the same for the same inputs on every system.
//!
//! [`Settings::max_sources`]: crate::master::Settings::max_sources

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use tore_net::master::candidate::canonical;

/// An IPv4 address, or the first 64 bits of an IPv6 address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SourceKey {
    /// One IPv4 address.
    V4(u32),
    /// One IPv6 /64 network.
    V6(u64),
}

impl SourceKey {
    /// The source an address counts against. An IPv4-mapped IPv6 address
    /// counts as its IPv4 address.
    pub fn of(address: SocketAddr) -> Self {
        match canonical(address).ip() {
            IpAddr::V4(v4) => Self::V4(u32::from(v4)),
            IpAddr::V6(v6) => Self::V6((u128::from(v6) >> 64) as u64),
        }
    }
}

impl fmt::Display for SourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::V4(v4) => write!(f, "{}", Ipv4Addr::from(v4)),
            Self::V6(prefix) => write!(f, "{}/64", Ipv6Addr::from(u128::from(prefix) << 64)),
        }
    }
}

/// A rate: `count` in `per`, with bursts of `burst`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    /// How many.
    pub count: u32,
    /// In how long.
    pub per: Duration,
    /// How many at once, after a quiet spell.
    pub burst: u32,
}

impl Rate {
    /// `count` a second, bursts of `burst`.
    pub const fn per_second(count: u32, burst: u32) -> Self {
        Self {
            count,
            per: Duration::from_secs(1),
            burst,
        }
    }

    /// `count` a minute, bursts of `burst`.
    pub const fn per_minute(count: u32, burst: u32) -> Self {
        Self {
            count,
            per: Duration::from_secs(60),
            burst,
        }
    }

    /// The time one request takes up.
    fn interval(&self) -> Duration {
        self.per / self.count.max(1)
    }

    /// How far ahead of now a bucket may run before it refuses.
    fn tolerance(&self) -> Duration {
        self.interval() * self.burst.max(1).saturating_sub(1)
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = if self.per == Duration::from_secs(60) {
            "a minute"
        } else {
            "a second"
        };
        write!(f, "{} {unit}, bursts of {}", self.count, self.burst)
    }
}

/// One token bucket (a generic cell rate algorithm).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Bucket {
    /// When the bucket is empty again.
    full_until: Duration,
}

impl Bucket {
    /// Takes one request at `now`: false when it is over `rate`.
    pub fn take(&mut self, now: Duration, rate: Rate) -> bool {
        let start = self.full_until.max(now);
        if start - now > rate.tolerance() {
            return false;
        }
        self.full_until = start + rate.interval();
        true
    }

    /// True when the bucket has used at most half its burst: a source that
    /// asks now and then rather than as fast as it may.
    pub fn quiet(&self, now: Duration, rate: Rate) -> bool {
        self.full_until.saturating_sub(now) <= (rate.tolerance() + rate.interval()) / 2
    }
}

/// What a source is limited in, one bucket each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// Register, with and without a cookie.
    Register,
    /// Browse and Details, and the small answers to strangers (Unknown
    /// listing, Unsupported).
    Query,
    /// Probe, on either port.
    Probe,
    /// Introduce (stage J).
    Introduce,
    /// Relay request (stage J).
    RelayRequest,
    /// Report.
    Report,
}

impl Limit {
    /// Every limit, in table order.
    pub const ALL: [Self; 6] = [
        Self::Register,
        Self::Query,
        Self::Probe,
        Self::Introduce,
        Self::RelayRequest,
        Self::Report,
    ];

    /// Its name in the log.
    pub fn name(self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::Query => "browse",
            Self::Probe => "probe",
            Self::Introduce => "introduce",
            Self::RelayRequest => "relay request",
            Self::Report => "report",
        }
    }
}

/// The rates of each [`Limit`] per source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rates {
    /// Register: 10 a minute.
    pub register: Rate,
    /// Browse and Details: 20 a second, bursts of 40.
    pub query: Rate,
    /// Probe: 4 a second.
    pub probe: Rate,
    /// Introduce: 4 a second (and 30 a minute, stage J).
    pub introduce: Rate,
    /// Relay request: 2 a minute.
    pub relay_request: Rate,
    /// Report: 1 a minute, bursts of 5.
    pub report: Rate,
}

impl Default for Rates {
    fn default() -> Self {
        Self {
            register: Rate::per_minute(10, 10),
            query: Rate::per_second(20, 40),
            probe: Rate::per_second(4, 4),
            introduce: Rate::per_second(4, 4),
            relay_request: Rate::per_minute(2, 2),
            report: Rate::per_minute(1, 5),
        }
    }
}

impl Rates {
    /// The rate of one limit.
    pub fn of(&self, limit: Limit) -> Rate {
        match limit {
            Limit::Register => self.register,
            Limit::Query => self.query,
            Limit::Probe => self.probe,
            Limit::Introduce => self.introduce,
            Limit::RelayRequest => self.relay_request,
            Limit::Report => self.report,
        }
    }
}

/// What the master remembers of one source.
#[derive(Debug, Clone, Default)]
struct Source {
    buckets: [Bucket; 6],
    /// The order it was last heard in, the table's key for forgetting.
    heard: u64,
    /// When the log last said this source was over a limit.
    logged: Option<Duration>,
}

/// The table of sources: every per-source bucket, bounded.
#[derive(Debug, Clone)]
pub struct Sources {
    rates: Rates,
    max: usize,
    table: HashMap<SourceKey, Source>,
    by_heard: BTreeMap<u64, SourceKey>,
    order: u64,
    forgotten: u64,
}

/// The answer to [`Sources::take`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taken {
    /// Within the limit; `quiet` when the source has used at most half its
    /// burst.
    Yes {
        /// The source asks now and then.
        quiet: bool,
    },
    /// Over it. `log` is true at most once a minute per source.
    Over {
        /// Say so in the log.
        log: bool,
    },
}

impl Sources {
    /// An empty table that remembers at most `max` sources.
    pub fn new(rates: Rates, max: usize) -> Self {
        Self {
            rates,
            max: max.max(1),
            table: HashMap::new(),
            by_heard: BTreeMap::new(),
            order: 0,
            forgotten: 0,
        }
    }

    /// The rates in force.
    pub fn rates(&self) -> &Rates {
        &self.rates
    }

    /// How many sources are remembered.
    pub fn len(&self) -> usize {
        self.table.len()
    }

    /// True when none is.
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    /// How many sources were forgotten to keep within the bound.
    pub fn forgotten(&self) -> u64 {
        self.forgotten
    }

    /// Takes one request of `limit` from `source` at `now`. The source is
    /// remembered (and heard now); the least recently heard is forgotten
    /// when the table is full.
    pub fn take(&mut self, source: SourceKey, limit: Limit, now: Duration) -> Taken {
        let rate = self.rates.of(limit);
        self.order += 1;
        let order = self.order;
        let entry = match self.table.get_mut(&source) {
            Some(entry) => {
                self.by_heard.remove(&entry.heard);
                entry
            }
            None => {
                if self.table.len() >= self.max
                    && let Some((_, oldest)) = self.by_heard.pop_first()
                {
                    self.table.remove(&oldest);
                    self.forgotten += 1;
                }
                self.table.entry(source).or_default()
            }
        };
        entry.heard = order;
        self.by_heard.insert(order, source);
        let bucket = &mut entry.buckets[limit as usize];
        if bucket.take(now, rate) {
            Taken::Yes {
                quiet: bucket.quiet(now, rate),
            }
        } else {
            let log = entry
                .logged
                .is_none_or(|at| now.saturating_sub(at) >= Duration::from_secs(60));
            if log {
                entry.logged = Some(now);
            }
            Taken::Over { log }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_bucket_takes_its_burst_then_its_rate() {
        let rate = Rate::per_second(4, 4);
        let mut bucket = Bucket::default();
        let start = Duration::from_secs(10);
        for _ in 0..4 {
            assert!(bucket.take(start, rate));
        }
        assert!(!bucket.take(start, rate));
        assert!(!bucket.take(start + ms(249), rate));
        assert!(bucket.take(start + ms(250), rate));
        assert!(!bucket.take(start + ms(250), rate));
        // After a quiet second the whole burst is back.
        let later = start + Duration::from_secs(5);
        for _ in 0..4 {
            assert!(bucket.take(later, rate));
        }
        assert!(!bucket.take(later, rate));
    }

    #[test]
    fn a_minute_rate_with_a_burst() {
        let rate = Rate::per_minute(1, 5);
        let mut bucket = Bucket::default();
        let t = Duration::from_secs(100);
        for _ in 0..5 {
            assert!(bucket.take(t, rate));
        }
        assert!(!bucket.take(t + Duration::from_secs(59), rate));
        assert!(bucket.take(t + Duration::from_secs(60), rate));
    }

    #[test]
    fn quiet_means_at_most_half_the_burst_used() {
        let rate = Rate::per_second(20, 40);
        let mut bucket = Bucket::default();
        let t = Duration::from_secs(1);
        assert!(bucket.take(t, rate));
        assert!(bucket.quiet(t, rate));
        for _ in 0..30 {
            bucket.take(t, rate);
        }
        assert!(!bucket.quiet(t, rate));
    }

    #[test]
    fn ipv6_sources_count_by_their_64_network() {
        let a: SocketAddr = "[2001:db8:1:2::5]:1000".parse().unwrap();
        let b: SocketAddr = "[2001:db8:1:2:ffff::9]:2000".parse().unwrap();
        let c: SocketAddr = "[2001:db8:1:3::5]:1000".parse().unwrap();
        assert_eq!(SourceKey::of(a), SourceKey::of(b));
        assert_ne!(SourceKey::of(a), SourceKey::of(c));
        assert_eq!(SourceKey::of(a).to_string(), "2001:db8:1:2::/64");
        let v4: SocketAddr = "203.0.113.5:9".parse().unwrap();
        let mapped: SocketAddr = "[::ffff:203.0.113.5]:10".parse().unwrap();
        assert_eq!(SourceKey::of(v4), SourceKey::of(mapped));
        assert_eq!(SourceKey::of(v4).to_string(), "203.0.113.5");
    }

    #[test]
    fn the_table_forgets_the_least_recently_heard() {
        let mut sources = Sources::new(Rates::default(), 3);
        let t = Duration::ZERO;
        for n in 1..=3 {
            sources.take(SourceKey::V4(n), Limit::Probe, t);
        }
        // Source 1 is heard again, so 2 is the oldest.
        sources.take(SourceKey::V4(1), Limit::Probe, t);
        sources.take(SourceKey::V4(4), Limit::Probe, t);
        assert_eq!(sources.len(), 3);
        assert_eq!(sources.forgotten(), 1);
        assert!(sources.table.contains_key(&SourceKey::V4(1)));
        assert!(!sources.table.contains_key(&SourceKey::V4(2)));
    }

    #[test]
    fn over_a_limit_is_logged_once_a_minute() {
        let mut sources = Sources::new(Rates::default(), 10);
        let key = SourceKey::V4(7);
        let t = Duration::from_secs(5);
        for _ in 0..4 {
            assert!(matches!(
                sources.take(key, Limit::Probe, t),
                Taken::Yes { .. }
            ));
        }
        assert_eq!(
            sources.take(key, Limit::Probe, t),
            Taken::Over { log: true }
        );
        assert_eq!(
            sources.take(key, Limit::Probe, t),
            Taken::Over { log: false }
        );
        let later = t + Duration::from_secs(60);
        for _ in 0..4 {
            sources.take(key, Limit::Probe, later);
        }
        assert_eq!(
            sources.take(key, Limit::Probe, later),
            Taken::Over { log: true }
        );
    }
}
