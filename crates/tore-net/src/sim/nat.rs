//! Routers for the network simulator: address translation (NAT) and
//! firewalls, with the behaviours of RFC 4787.
//!
//! A router stands between the addresses behind it (its inside prefix) and
//! the network outside it. On the way out it gives each inside socket an
//! outside address and port (a *mapping*) and remembers whom it sent to; on
//! the way in it lets a datagram through only to a live mapping and only from
//! a sender its filtering allows. A mapping lives while the inside socket
//! sends: incoming traffic never refreshes it. A router without an outside
//! address translates nothing and only filters: an IPv6 firewall.
//!
//! The routing itself (which router a datagram passes, and when) is in the
//! simulator's [`SimNetwork`](super::SimNetwork); this module holds each
//! router's settings and state.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;

use crate::entropy::SplitMix64;

/// The lowest outside port a router hands out by itself.
pub const FIRST_DYNAMIC_PORT: u16 = 1024;

/// A router's default idle time: a mapping nobody sends through for this
/// long is gone. RFC 4787 asks for at least two minutes (agent decision: the
/// default is that minimum; tests set shorter ones).
pub const DEFAULT_IDLE: Duration = Duration::from_secs(120);

/// An address prefix, such as `192.168.1.0/24`: the addresses behind a
/// router.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Prefix {
    address: IpAddr,
    bits: u8,
}

impl Prefix {
    /// The prefix of `bits` leading bits of `address` (host bits cleared);
    /// `None` when `bits` is longer than the address.
    pub fn new(address: IpAddr, bits: u8) -> Option<Self> {
        let max = if address.is_ipv4() { 32 } else { 128 };
        (bits <= max).then(|| Self {
            address: mask(address, bits),
            bits,
        })
    }

    /// The prefix's first address.
    pub fn address(&self) -> IpAddr {
        self.address
    }

    /// How many leading bits it fixes.
    pub fn bits(&self) -> u8 {
        self.bits
    }

    /// Whether `ip` is in it (an address of the other family never is).
    pub fn contains(&self, ip: IpAddr) -> bool {
        ip.is_ipv4() == self.address.is_ipv4() && mask(ip, self.bits) == self.address
    }
}

fn mask(ip: IpAddr, bits: u8) -> IpAddr {
    match ip {
        IpAddr::V4(v4) => {
            let m = if bits == 0 {
                0
            } else {
                u32::MAX << (32 - u32::from(bits))
            };
            IpAddr::V4(Ipv4Addr::from(u32::from(v4) & m))
        }
        IpAddr::V6(v6) => {
            let m = if bits == 0 {
                0
            } else {
                u128::MAX << (128 - u32::from(bits))
            };
            IpAddr::V6(Ipv6Addr::from(u128::from(v6) & m))
        }
    }
}

impl fmt::Display for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.address, self.bits)
    }
}

/// A prefix that would not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixParseError;

impl fmt::Display for PrefixParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not an address prefix such as 192.168.1.0/24")
    }
}

impl std::error::Error for PrefixParseError {}

impl FromStr for Prefix {
    type Err = PrefixParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (address, bits) = s.split_once('/').ok_or(PrefixParseError)?;
        let address: IpAddr = address.parse().map_err(|_| PrefixParseError)?;
        let bits: u8 = bits.parse().map_err(|_| PrefixParseError)?;
        Self::new(address, bits).ok_or(PrefixParseError)
    }
}

/// How a router picks the outside port of a new mapping (RFC 4787's
/// mapping behaviour).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mapping {
    /// One outside port for an inside socket, whoever it sends to
    /// ("endpoint-independent"; what hole punching needs).
    EndpointIndependent,
    /// A new outside port for each destination address.
    AddressDependent,
    /// A new outside port for each destination address and port
    /// ("symmetric").
    AddressAndPortDependent,
}

/// Which senders a router lets in to a mapping (RFC 4787's filtering
/// behaviour).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filtering {
    /// Anyone (a "full cone").
    EndpointIndependent,
    /// Only addresses the mapping has sent to, from any port.
    AddressDependent,
    /// Only the addresses and ports the mapping has sent to.
    AddressAndPortDependent,
}

/// Which outside port a router gives a new mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortChoice {
    /// The inside socket's own port when it is free, else the next free one
    /// above it.
    Preserve,
    /// The next free port after the one it handed out last, from 1,024 up,
    /// wrapping.
    Next,
    /// A free port drawn at random (seeded by the network's seed and the
    /// router's number), from 1,024 up.
    Random,
}

/// A static forward: the router's outside port that always leads to one
/// inside socket, open to anyone, never expiring. A port forwarded by hand
/// or by UPnP, NAT-PMP or PCP. The inside socket also sends from it: its
/// datagrams leave from the forwarded port, whoever they go to (agent
/// decision, as routers with a port mapping do).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Forward {
    /// The outside port. A firewall's forward opens the inside socket's own
    /// address, so its outside port should be the inside port.
    pub outside_port: u16,
    /// The inside socket.
    pub inside: SocketAddr,
}

/// A router's number on its network, from
/// [`SimNetwork::add_router`](super::SimNetwork::add_router).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RouterId(pub(crate) usize);

/// A router's settings.
#[derive(Debug, Clone, PartialEq)]
pub struct RouterConfig {
    /// Its outside address, on the network outside it; `None` for a
    /// firewall that translates nothing and only filters.
    pub outside: Option<IpAddr>,
    /// The addresses behind it.
    pub inside: Prefix,
    /// How it maps (ignored by a firewall, whose mapping is the inside
    /// socket's own address).
    pub mapping: Mapping,
    /// How it filters.
    pub filtering: Filtering,
    /// How it picks outside ports (ignored by a firewall).
    pub ports: PortChoice,
    /// How long a mapping lasts after the inside socket last sent through it.
    pub idle: Duration,
    /// Whether a datagram from inside to its own outside address loops back
    /// in (hairpinning); without it such a datagram is dropped. The looped
    /// datagram comes from the sender's outside address and passes the
    /// filter like any other (agent decision).
    pub hairpin: bool,
    /// Static forwards.
    pub forwards: Vec<Forward>,
    /// The router it stands behind. `None` places it by its outside address
    /// (a firewall: by its inside prefix): behind the router whose inside
    /// prefix holds that address most narrowly, else on the open network.
    /// Needed only when two routers' inside prefixes are the same, as two
    /// homes on `192.168.1.0/24` behind one carrier.
    pub behind: Option<RouterId>,
}

impl RouterConfig {
    /// A typical home router: one outside port per inside socket, kept equal
    /// to the inside port when free, letting in only the addresses and ports
    /// it has sent to, idle after two minutes, with hairpinning.
    pub fn nat(outside: IpAddr, inside: Prefix) -> Self {
        Self {
            outside: Some(outside),
            inside,
            mapping: Mapping::EndpointIndependent,
            filtering: Filtering::AddressAndPortDependent,
            ports: PortChoice::Preserve,
            idle: DEFAULT_IDLE,
            hairpin: true,
            forwards: Vec::new(),
            behind: None,
        }
    }

    /// A stateful firewall (as in front of an IPv6 network): no translation,
    /// letting in only the addresses and ports each inside socket has sent
    /// to, idle after two minutes.
    pub fn firewall(inside: Prefix) -> Self {
        Self {
            outside: None,
            hairpin: false,
            ..Self::nat(inside.address(), inside)
        }
    }
}

/// What one router did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RouterStats {
    /// Datagrams it passed from inside to outside.
    pub sent_out: u64,
    /// Datagrams it let in.
    pub let_in: u64,
    /// Datagrams it looped back from inside to inside (counted in neither
    /// of the above).
    pub hairpinned: u64,
    /// Mappings it made.
    pub mappings: u64,
    /// Arrivals dropped for want of a mapping or forward at their port (or,
    /// on the way out, of a free port).
    pub no_mapping: u64,
    /// Arrivals dropped because the mapping had not sent to the sender.
    pub filtered: u64,
    /// Arrivals dropped because their mapping had gone idle.
    pub expired: u64,
    /// Datagrams to its own outside address dropped because it does not
    /// hairpin.
    pub no_hairpin: u64,
}

/// Why a router dropped a datagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dropped {
    NoMapping,
    Filtered,
    Expired,
    NoHairpin,
}

/// Where a router stands: behind another router, or (`None`) on the open
/// network.
pub(crate) type Realm = Option<RouterId>;

/// A mapping's key: the inside socket and, for mappings that depend on the
/// destination, the destination (its port 0 for an address-dependent one).
type BindKey = (SocketAddr, Option<SocketAddr>);

#[derive(Debug)]
struct Binding {
    outside: SocketAddr,
    last_out: Duration,
    sent_to: BTreeSet<SocketAddr>,
}

/// One router's settings and state.
#[derive(Debug)]
pub(crate) struct Router {
    pub(crate) config: RouterConfig,
    pub(crate) parent: Realm,
    pub(crate) stats: RouterStats,
    bindings: BTreeMap<BindKey, Binding>,
    by_outside: BTreeMap<SocketAddr, BindKey>,
    next_port: u16,
    rng: SplitMix64,
}

impl Router {
    pub(crate) fn new(config: RouterConfig, parent: Realm, seed: u64) -> Self {
        Self {
            config,
            parent,
            stats: RouterStats::default(),
            bindings: BTreeMap::new(),
            by_outside: BTreeMap::new(),
            next_port: FIRST_DYNAMIC_PORT,
            rng: SplitMix64::new(seed),
        }
    }

    /// Whether a datagram to `ip`, on the network outside this router, is
    /// this router's to take in: its outside address, or for a firewall any
    /// address behind it.
    pub(crate) fn claims(&self, ip: IpAddr) -> bool {
        match self.config.outside {
            Some(outside) => ip == outside,
            None => self.config.inside.contains(ip),
        }
    }

    /// The outside address a forward or mapping of `inside` at `port` has.
    fn outside_of(&self, inside: SocketAddr, port: u16) -> SocketAddr {
        SocketAddr::new(self.config.outside.unwrap_or(inside.ip()), port)
    }

    fn forward_from(&self, inside: SocketAddr) -> Option<&Forward> {
        self.config.forwards.iter().find(|f| f.inside == inside)
    }

    fn forward_to(&self, outside: SocketAddr) -> Option<&Forward> {
        self.config
            .forwards
            .iter()
            .find(|f| self.outside_of(f.inside, f.outside_port) == outside)
    }

    fn expired(&self, binding: &Binding, at: Duration) -> bool {
        at.saturating_sub(binding.last_out) >= self.config.idle
    }

    fn remove(&mut self, key: BindKey) {
        if let Some(binding) = self.bindings.remove(&key) {
            self.by_outside.remove(&binding.outside);
        }
    }

    fn sweep(&mut self, now: Duration) {
        let gone: Vec<BindKey> = self
            .bindings
            .iter()
            .filter(|(_, b)| self.expired(b, now))
            .map(|(k, _)| *k)
            .collect();
        for key in gone {
            self.remove(key);
        }
    }

    fn port_free(&self, inside: SocketAddr, port: u16) -> bool {
        port >= FIRST_DYNAMIC_PORT
            && !self.by_outside.contains_key(&self.outside_of(inside, port))
            && !self.config.forwards.iter().any(|f| f.outside_port == port)
    }

    /// The first free port from `start` up, wrapping; `None` when every
    /// dynamic port is taken.
    fn scan(&self, inside: SocketAddr, start: u16) -> Option<u16> {
        let span = u32::from(u16::MAX - FIRST_DYNAMIC_PORT) + 1;
        let start = u32::from(start.max(FIRST_DYNAMIC_PORT) - FIRST_DYNAMIC_PORT);
        (0..span)
            .map(|i| FIRST_DYNAMIC_PORT + ((start + i) % span) as u16)
            .find(|&port| self.port_free(inside, port))
    }

    fn allocate(&mut self, inside: SocketAddr) -> Option<u16> {
        if self.config.outside.is_none() {
            return Some(inside.port());
        }
        let port = match self.config.ports {
            PortChoice::Preserve => {
                if self.port_free(inside, inside.port()) {
                    Some(inside.port())
                } else {
                    self.scan(inside, inside.port().saturating_add(1))
                }
            }
            PortChoice::Next => self.scan(inside, self.next_port),
            PortChoice::Random => {
                let span = u64::from(u16::MAX - FIRST_DYNAMIC_PORT) + 1;
                let drawn = FIRST_DYNAMIC_PORT + self.rng.below(span) as u16;
                self.scan(inside, drawn)
            }
        }?;
        self.next_port = port.checked_add(1).unwrap_or(FIRST_DYNAMIC_PORT);
        Some(port)
    }

    fn key(&self, inside: SocketAddr, to: SocketAddr) -> BindKey {
        if self.config.outside.is_none() {
            return (inside, None);
        }
        match self.config.mapping {
            Mapping::EndpointIndependent => (inside, None),
            Mapping::AddressDependent => (inside, Some(SocketAddr::new(to.ip(), 0))),
            Mapping::AddressAndPortDependent => (inside, Some(to)),
        }
    }

    /// A datagram from the inside socket `from` to `to` leaves through this
    /// router at `now`: the address it leaves from.
    pub(crate) fn outbound(
        &mut self,
        now: Duration,
        from: SocketAddr,
        to: SocketAddr,
    ) -> Result<SocketAddr, Dropped> {
        let hairpin = self.config.outside == Some(to.ip());
        if hairpin && !self.config.hairpin {
            self.stats.no_hairpin += 1;
            return Err(Dropped::NoHairpin);
        }
        let leaves_from = if let Some(forward) = self.forward_from(from) {
            self.outside_of(from, forward.outside_port)
        } else {
            let key = self.key(from, to);
            if self
                .bindings
                .get(&key)
                .is_some_and(|b| self.expired(b, now))
            {
                self.remove(key);
            }
            if !self.bindings.contains_key(&key) {
                self.sweep(now);
                let Some(port) = self.allocate(from) else {
                    self.stats.no_mapping += 1;
                    return Err(Dropped::NoMapping);
                };
                let outside = self.outside_of(from, port);
                self.bindings.insert(
                    key,
                    Binding {
                        outside,
                        last_out: now,
                        sent_to: BTreeSet::new(),
                    },
                );
                self.by_outside.insert(outside, key);
                self.stats.mappings += 1;
            }
            let binding = self.bindings.get_mut(&key).expect("just made");
            binding.last_out = now;
            binding.sent_to.insert(to);
            binding.outside
        };
        if hairpin {
            self.stats.hairpinned += 1;
        } else {
            self.stats.sent_out += 1;
        }
        Ok(leaves_from)
    }

    /// A datagram from `from` reaches this router for `to` at `at`: the
    /// inside socket it goes to.
    pub(crate) fn inbound(
        &mut self,
        at: Duration,
        from: SocketAddr,
        to: SocketAddr,
    ) -> Result<SocketAddr, Dropped> {
        let looped = self.config.outside == Some(from.ip());
        if let Some(forward) = self.forward_to(to) {
            let inside = forward.inside;
            if !looped {
                self.stats.let_in += 1;
            }
            return Ok(inside);
        }
        let Some(&key) = self.by_outside.get(&to) else {
            self.stats.no_mapping += 1;
            return Err(Dropped::NoMapping);
        };
        let binding = &self.bindings[&key];
        if self.expired(binding, at) {
            self.remove(key);
            self.stats.expired += 1;
            return Err(Dropped::Expired);
        }
        let allowed = match self.config.filtering {
            Filtering::EndpointIndependent => true,
            Filtering::AddressDependent => binding.sent_to.iter().any(|s| s.ip() == from.ip()),
            Filtering::AddressAndPortDependent => binding.sent_to.contains(&from),
        };
        if !allowed {
            self.stats.filtered += 1;
            return Err(Dropped::Filtered);
        }
        if !looped {
            self.stats.let_in += 1;
        }
        Ok(key.0)
    }

    /// The outside address the live mapping of `inside` toward `to` has at
    /// `now`, if there is one.
    pub(crate) fn mapped(
        &self,
        now: Duration,
        inside: SocketAddr,
        to: SocketAddr,
    ) -> Option<SocketAddr> {
        if let Some(forward) = self.forward_from(inside) {
            return Some(self.outside_of(inside, forward.outside_port));
        }
        self.bindings
            .get(&self.key(inside, to))
            .filter(|b| !self.expired(b, now))
            .map(|b| b.outside)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datagram::Datagrams;
    use crate::sim::{LinkConfig, SimNetwork, SimSocket};

    const MS: Duration = Duration::from_millis(1);

    fn a(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn p(s: &str) -> Prefix {
        s.parse().unwrap()
    }

    /// Everything waiting at a socket: the sender as seen, and the bytes.
    fn drain(socket: &mut SimSocket) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut buf = [0u8; 64];
        let mut got = Vec::new();
        while let Some((len, from)) = socket.recv_datagram(&mut buf).unwrap() {
            got.push((from, buf[..len].to_vec()));
        }
        got
    }

    /// The address each datagram waiting at `socket` came from.
    fn senders(socket: &mut SimSocket) -> Vec<SocketAddr> {
        drain(socket).into_iter().map(|(from, _)| from).collect()
    }

    const HOME_OUT: &str = "203.0.113.10";
    const PLAYER: &str = "192.168.1.10:40000";
    const MASTER: &str = "198.51.100.1:26911";
    const PROBE: &str = "198.51.100.1:26912";
    const OTHER: &str = "198.51.100.2:26911";

    /// A network with one home router configured by `tune`, the player
    /// behind it and three servers outside.
    fn home(
        tune: impl FnOnce(&mut RouterConfig),
    ) -> (SimNetwork, RouterId, SimSocket, [SimSocket; 3]) {
        let net = SimNetwork::new(1);
        let mut config = RouterConfig::nat(ip(HOME_OUT), p("192.168.1.0/24"));
        tune(&mut config);
        let router = net.add_router(config).unwrap();
        let player = net.bind(a(PLAYER)).unwrap();
        let servers = [MASTER, PROBE, OTHER].map(|s| net.bind(a(s)).unwrap());
        (net, router, player, servers)
    }

    #[test]
    fn prefixes_parse_mask_and_contain() {
        let prefix = p("192.168.1.77/24");
        assert_eq!(prefix.to_string(), "192.168.1.0/24");
        assert!(prefix.contains(ip("192.168.1.200")));
        assert!(!prefix.contains(ip("192.168.2.1")));
        assert!(!prefix.contains(ip("::1")));
        assert!(p("2001:db8:1::/48").contains(ip("2001:db8:1:ff::1")));
        assert!(p("0.0.0.0/0").contains(ip("8.8.8.8")));
        assert!("10.0.0.0/33".parse::<Prefix>().is_err());
        assert!("10.0.0.0".parse::<Prefix>().is_err());
    }

    #[test]
    fn endpoint_independent_mapping_keeps_one_outside_port() {
        let (net, router, mut player, [mut master, mut probe, mut other]) = home(|c| {
            c.mapping = Mapping::EndpointIndependent;
        });
        for to in [MASTER, PROBE, OTHER] {
            player.send_datagram(a(to), b"probe").unwrap();
        }
        let seen: Vec<SocketAddr> = [&mut master, &mut probe, &mut other]
            .into_iter()
            .flat_map(senders)
            .collect();
        // Port preservation keeps the inside port, too.
        assert_eq!(seen, vec![a("203.0.113.10:40000"); 3]);
        assert_eq!(
            net.mapped(router, a(PLAYER), a(OTHER)),
            Some(a("203.0.113.10:40000"))
        );
        assert_eq!(net.router_stats(router).mappings, 1);
        assert_eq!(net.router_stats(router).sent_out, 3);
    }

    #[test]
    fn address_dependent_mapping_takes_a_port_per_address() {
        let (net, router, mut player, [mut master, mut probe, mut other]) = home(|c| {
            c.mapping = Mapping::AddressDependent;
        });
        for to in [MASTER, PROBE, OTHER] {
            player.send_datagram(a(to), b"probe").unwrap();
        }
        let m = senders(&mut master);
        let p2 = senders(&mut probe);
        let o = senders(&mut other);
        // The two master ports share an address, so they share a mapping.
        assert_eq!(m, p2);
        assert_ne!(m, o);
        assert_eq!(m[0].ip(), ip(HOME_OUT));
        assert_eq!(net.router_stats(router).mappings, 2);
    }

    #[test]
    fn address_and_port_dependent_mapping_takes_a_port_per_destination() {
        let (net, router, mut player, [mut master, mut probe, mut other]) = home(|c| {
            c.mapping = Mapping::AddressAndPortDependent;
        });
        for to in [MASTER, PROBE, OTHER, MASTER] {
            player.send_datagram(a(to), b"probe").unwrap();
        }
        let m = senders(&mut master);
        let p2 = senders(&mut probe);
        let o = senders(&mut other);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0], m[1], "the same destination keeps its mapping");
        assert_ne!(m[0], p2[0]);
        assert_ne!(m[0], o[0]);
        assert_ne!(p2[0], o[0]);
        assert_eq!(net.router_stats(router).mappings, 3);
    }

    /// The player sends to the master only; then the master's other port,
    /// another address and the master itself answer.
    fn answers(filtering: Filtering) -> (Vec<Vec<u8>>, RouterStats) {
        let (net, router, mut player, [mut master, mut probe, mut other]) =
            home(|c| c.filtering = filtering);
        player.send_datagram(a(MASTER), b"hello").unwrap();
        let outside = senders(&mut master)[0];
        master.send_datagram(outside, b"master").unwrap();
        probe.send_datagram(outside, b"probe port").unwrap();
        other.send_datagram(outside, b"stranger").unwrap();
        let got = drain(&mut player).into_iter().map(|(_, d)| d).collect();
        (got, net.router_stats(router))
    }

    #[test]
    fn endpoint_independent_filtering_lets_anyone_in() {
        let (got, stats) = answers(Filtering::EndpointIndependent);
        assert_eq!(got, [&b"master"[..], b"probe port", b"stranger"]);
        assert_eq!(stats.let_in, 3);
        assert_eq!(stats.filtered, 0);
    }

    #[test]
    fn address_dependent_filtering_lets_in_any_port_of_an_address_sent_to() {
        let (got, stats) = answers(Filtering::AddressDependent);
        assert_eq!(got, [&b"master"[..], b"probe port"]);
        assert_eq!(stats.filtered, 1);
    }

    #[test]
    fn address_and_port_dependent_filtering_lets_in_only_what_was_sent_to() {
        let (got, stats) = answers(Filtering::AddressAndPortDependent);
        assert_eq!(got, [&b"master"[..]]);
        assert_eq!(stats.filtered, 2);
    }

    #[test]
    fn nothing_gets_in_without_a_mapping() {
        let (net, router, mut player, [mut master, _, _]) = home(|c| {
            c.filtering = Filtering::EndpointIndependent;
        });
        master
            .send_datagram(a("203.0.113.10:40000"), b"unasked")
            .unwrap();
        assert!(drain(&mut player).is_empty());
        assert_eq!(net.router_stats(router).no_mapping, 1);
    }

    /// Two inside sockets on the same port, each sending to the master: the
    /// outside ports the master sees.
    fn two_on_one_port(ports: PortChoice, seed: u64) -> Vec<u16> {
        let net = SimNetwork::new(seed);
        let mut config = RouterConfig::nat(ip(HOME_OUT), p("192.168.1.0/24"));
        config.ports = ports;
        net.add_router(config).unwrap();
        let mut master = net.bind(a(MASTER)).unwrap();
        let mut first = net.bind(a("192.168.1.10:26900")).unwrap();
        let mut second = net.bind(a("192.168.1.11:26900")).unwrap();
        first.send_datagram(a(MASTER), b"1").unwrap();
        second.send_datagram(a(MASTER), b"2").unwrap();
        first.send_datagram(a(MASTER), b"1 again").unwrap();
        senders(&mut master).into_iter().map(|s| s.port()).collect()
    }

    #[test]
    fn port_preservation_keeps_the_inside_port_when_it_is_free() {
        assert_eq!(
            two_on_one_port(PortChoice::Preserve, 1),
            [26_900, 26_901, 26_900]
        );
    }

    #[test]
    fn sequential_ports_count_up_from_the_first_dynamic_port() {
        assert_eq!(two_on_one_port(PortChoice::Next, 1), [1024, 1025, 1024]);
    }

    #[test]
    fn random_ports_follow_the_seed() {
        let one = two_on_one_port(PortChoice::Random, 5);
        assert_eq!(one, two_on_one_port(PortChoice::Random, 5));
        assert_ne!(one, two_on_one_port(PortChoice::Random, 6));
        assert_eq!(one[0], one[2]);
        assert_ne!(one[0], one[1]);
        assert!(one.iter().all(|&port| port >= FIRST_DYNAMIC_PORT));
    }

    #[test]
    fn a_forwarded_port_is_never_handed_out() {
        let net = SimNetwork::new(1);
        let mut config = RouterConfig::nat(ip(HOME_OUT), p("192.168.1.0/24"));
        config.forwards.push(Forward {
            outside_port: 40_000,
            inside: a("192.168.1.20:26900"),
        });
        net.add_router(config).unwrap();
        let mut master = net.bind(a(MASTER)).unwrap();
        let mut player = net.bind(a(PLAYER)).unwrap();
        player.send_datagram(a(MASTER), b"x").unwrap();
        assert_eq!(senders(&mut master), [a("203.0.113.10:40001")]);
    }

    #[test]
    fn a_mapping_lasts_while_the_inside_sends_and_incoming_traffic_does_not_keep_it() {
        let (net, router, mut player, [mut master, _, _]) = home(|c| {
            c.idle = Duration::from_secs(10);
            c.ports = PortChoice::Next;
        });
        player.send_datagram(a(MASTER), b"out at 0").unwrap();
        let outside = senders(&mut master)[0];
        // Incoming traffic every second does not refresh it...
        for _ in 0..9 {
            net.advance(Duration::from_secs(1));
            master.send_datagram(outside, b"in").unwrap();
        }
        assert_eq!(drain(&mut player).len(), 9);
        net.advance(Duration::from_secs(1));
        master.send_datagram(outside, b"in at 10").unwrap();
        assert!(drain(&mut player).is_empty());
        let stats = net.router_stats(router);
        assert_eq!(stats.expired, 1);
        assert_eq!(net.mapped(router, a(PLAYER), a(MASTER)), None);
        // ...outgoing traffic does: a new mapping, sent through at 10, 19 and
        // 28, is still open at 37.
        player.send_datagram(a(MASTER), b"out at 10").unwrap();
        let fresh = senders(&mut master)[0];
        assert_ne!(fresh, outside, "the expired port went back to the pool");
        for _ in 0..2 {
            net.advance(Duration::from_secs(9));
            player.send_datagram(a(MASTER), b"keep").unwrap();
        }
        net.advance(Duration::from_secs(9));
        master.send_datagram(fresh, b"in at 37").unwrap();
        assert_eq!(drain(&mut player).len(), 1);
        assert_eq!(net.router_stats(router).mappings, 2);
    }

    #[test]
    fn a_router_decides_an_arrival_when_it_arrives() {
        // Two homes punch toward each other at the same moment over a 100 ms
        // round trip. Each router has sent to the other before the other's
        // datagram reaches it, so both get through: the filter is checked at
        // arrival, not when the datagram was sent.
        let net = SimNetwork::new(1);
        net.set_default_link(LinkConfig::one_way(50 * MS));
        let ra = net
            .add_router(RouterConfig::nat(ip("203.0.113.1"), p("192.168.1.0/24")))
            .unwrap();
        let rb = net
            .add_router(RouterConfig::nat(ip("203.0.113.2"), p("192.168.2.0/24")))
            .unwrap();
        let mut pa = net.bind(a("192.168.1.10:26900")).unwrap();
        let mut pb = net.bind(a("192.168.2.10:40000")).unwrap();
        // b is a moment late: a's punch is on its way before b has sent.
        pa.send_datagram(a("203.0.113.2:40000"), b"from a").unwrap();
        net.advance(10 * MS);
        pb.send_datagram(a("203.0.113.1:26900"), b"from b").unwrap();
        net.advance(39 * MS);
        assert!(drain(&mut pb).is_empty());
        net.advance(MS);
        assert_eq!(
            drain(&mut pb),
            [(a("203.0.113.1:26900"), b"from a".to_vec())]
        );
        net.advance(10 * MS);
        assert_eq!(
            drain(&mut pa),
            [(a("203.0.113.2:40000"), b"from b".to_vec())]
        );
        assert_eq!(
            net.router_stats(ra).filtered + net.router_stats(rb).filtered,
            0
        );

        // Had b sent later than a's punch arrived, its router would have
        // dropped the punch.
        let net = SimNetwork::new(1);
        net.set_default_link(LinkConfig::one_way(50 * MS));
        net.add_router(RouterConfig::nat(ip("203.0.113.1"), p("192.168.1.0/24")))
            .unwrap();
        let rb = net
            .add_router(RouterConfig::nat(ip("203.0.113.2"), p("192.168.2.0/24")))
            .unwrap();
        let mut pa = net.bind(a("192.168.1.10:26900")).unwrap();
        let mut pb = net.bind(a("192.168.2.10:40000")).unwrap();
        pa.send_datagram(a("203.0.113.2:40000"), b"from a").unwrap();
        net.advance(60 * MS);
        pb.send_datagram(a("203.0.113.1:26900"), b"from b").unwrap();
        assert!(drain(&mut pb).is_empty());
        assert_eq!(net.router_stats(rb).no_mapping, 1);
        net.advance(50 * MS);
        assert_eq!(drain(&mut pa).len(), 1);
        assert_eq!(net.next_arrival(), None);
    }

    /// Two sockets behind one router: the second sends to the first's
    /// outside address.
    fn hairpin(on: bool) -> (Vec<(SocketAddr, Vec<u8>)>, RouterStats) {
        let net = SimNetwork::new(1);
        let mut config = RouterConfig::nat(ip(HOME_OUT), p("192.168.1.0/24"));
        config.hairpin = on;
        config.filtering = Filtering::EndpointIndependent;
        let router = net.add_router(config).unwrap();
        let mut master = net.bind(a(MASTER)).unwrap();
        let mut host = net.bind(a("192.168.1.10:26900")).unwrap();
        let mut player = net.bind(a("192.168.1.11:40000")).unwrap();
        host.send_datagram(a(MASTER), b"register").unwrap();
        let host_outside = senders(&mut master)[0];
        player.send_datagram(host_outside, b"looped").unwrap();
        // The local address works either way, without the router.
        player
            .send_datagram(a("192.168.1.10:26900"), b"local")
            .unwrap();
        (drain(&mut host), net.router_stats(router))
    }

    #[test]
    fn hairpinning_loops_back_from_the_senders_outside_address() {
        let (got, stats) = hairpin(true);
        assert_eq!(
            got,
            [
                (a("203.0.113.10:40000"), b"looped".to_vec()),
                (a("192.168.1.11:40000"), b"local".to_vec()),
            ]
        );
        assert_eq!(stats.hairpinned, 1);
        assert_eq!(stats.sent_out, 1);
        assert_eq!(stats.let_in, 0);
    }

    #[test]
    fn without_hairpinning_the_outside_address_is_a_dead_end() {
        let (got, stats) = hairpin(false);
        assert_eq!(got, [(a("192.168.1.11:40000"), b"local".to_vec())]);
        assert_eq!(stats.no_hairpin, 1);
        assert_eq!(stats.hairpinned, 0);
    }

    #[test]
    fn a_router_behind_a_router_translates_twice() {
        // A home router behind a carrier's: the carrier maps per destination
        // and picks random ports.
        let net = SimNetwork::new(3);
        let mut carrier = RouterConfig::nat(ip("203.0.113.50"), p("100.64.0.0/10"));
        carrier.mapping = Mapping::AddressAndPortDependent;
        carrier.ports = PortChoice::Random;
        let carrier = net.add_router(carrier).unwrap();
        let home = net
            .add_router(RouterConfig::nat(ip("100.64.7.9"), p("192.168.1.0/24")))
            .unwrap();
        let mut player = net.bind(a(PLAYER)).unwrap();
        let mut master = net.bind(a(MASTER)).unwrap();
        let mut probe = net.bind(a(PROBE)).unwrap();
        player.send_datagram(a(MASTER), b"probe").unwrap();
        player.send_datagram(a(PROBE), b"probe").unwrap();
        let m = senders(&mut master)[0];
        let p2 = senders(&mut probe)[0];
        assert_eq!(m.ip(), ip("203.0.113.50"));
        assert_eq!(p2.ip(), ip("203.0.113.50"));
        assert_ne!(m.port(), p2.port(), "the carrier maps per destination");
        assert_eq!(
            net.mapped(home, a(PLAYER), a(MASTER)),
            Some(a("100.64.7.9:40000"))
        );
        assert_eq!(
            net.mapped(carrier, a("100.64.7.9:40000"), a(MASTER)),
            Some(m)
        );
        // The answer comes back in through both.
        master.send_datagram(m, b"answer").unwrap();
        assert_eq!(drain(&mut player), [(a(MASTER), b"answer".to_vec())]);
        assert_eq!(net.router_stats(carrier).let_in, 1);
        assert_eq!(net.router_stats(home).let_in, 1);
        // The master's answer to the probe's mapping, from the wrong port,
        // stops at the carrier.
        master.send_datagram(p2, b"wrong port").unwrap();
        assert!(drain(&mut player).is_empty());
        assert_eq!(net.router_stats(carrier).filtered, 1);
        assert_eq!(net.router_stats(home).let_in, 1);
    }

    #[test]
    fn two_homes_on_the_same_prefix_behind_one_carrier() {
        let net = SimNetwork::new(1);
        let carrier = net
            .add_router(RouterConfig::nat(ip("203.0.113.50"), p("100.64.0.0/10")))
            .unwrap();
        let mut first = RouterConfig::nat(ip("100.64.0.1"), p("192.168.1.0/24"));
        first.behind = Some(carrier);
        let mut second = RouterConfig::nat(ip("100.64.0.2"), p("192.168.1.0/24"));
        second.filtering = Filtering::EndpointIndependent;
        let first = net.add_router(first).unwrap();
        let second = net.add_router(second).unwrap();
        // An address both prefixes hold needs saying which.
        assert_eq!(
            net.bind(a(PLAYER)).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        let mut one = net.bind_behind(first, a(PLAYER)).unwrap();
        let mut two = net.bind_behind(second, a(PLAYER)).unwrap();
        // The first home reaches the second through their routers alone; the
        // carrier is not crossed.
        one.send_datagram(a("100.64.0.2:40000"), b"neighbour")
            .unwrap();
        assert!(drain(&mut one).is_empty());
        let mut buf = [0u8; 16];
        // Unsolicited: the second home has no mapping at 40000 yet.
        assert!(two.recv_datagram(&mut buf).unwrap().is_none());
        two.send_datagram(a(MASTER), b"open").unwrap();
        one.send_datagram(a("100.64.0.2:40000"), b"neighbour")
            .unwrap();
        assert_eq!(
            drain(&mut two),
            [(a("100.64.0.1:40000"), b"neighbour".to_vec())]
        );
        assert_eq!(net.router_stats(carrier).sent_out, 1);
        assert_eq!(net.router_stats(second).no_mapping, 1);
    }

    #[test]
    fn an_ipv6_firewall_translates_nothing_and_filters_by_what_was_sent() {
        let net = SimNetwork::new(1);
        let host_side = net
            .add_router(RouterConfig::firewall(p("2001:db8:1::/64")))
            .unwrap();
        let player_side = net
            .add_router(RouterConfig::firewall(p("2001:db8:2::/64")))
            .unwrap();
        let mut host = net.bind(a("[2001:db8:1::10]:26900")).unwrap();
        let mut player = net.bind(a("[2001:db8:2::20]:40000")).unwrap();
        let mut neighbour = net.bind(a("[2001:db8:2::21]:40000")).unwrap();
        // The player's first datagram opens its own firewall but stops at the
        // host's.
        player
            .send_datagram(a("[2001:db8:1::10]:26900"), b"connect")
            .unwrap();
        assert!(drain(&mut host).is_empty());
        assert_eq!(net.router_stats(host_side).no_mapping, 1);
        // The host's punch opens the host's firewall and passes the player's.
        host.send_datagram(a("[2001:db8:2::20]:40000"), b"punch")
            .unwrap();
        assert_eq!(
            drain(&mut player),
            [(a("[2001:db8:1::10]:26900"), b"punch".to_vec())]
        );
        player
            .send_datagram(a("[2001:db8:1::10]:26900"), b"connect")
            .unwrap();
        assert_eq!(
            drain(&mut host),
            [(a("[2001:db8:2::20]:40000"), b"connect".to_vec())],
            "no translation: the host sees the player's own address"
        );
        // Neighbours behind one firewall reach each other directly.
        neighbour
            .send_datagram(a("[2001:db8:2::20]:40000"), b"hi")
            .unwrap();
        assert_eq!(drain(&mut player).len(), 1);
        assert_eq!(net.router_stats(player_side).sent_out, 2);
        assert_eq!(net.router_stats(host_side).let_in, 1);
    }

    #[test]
    fn a_static_forward_lets_anyone_in_and_carries_the_inside_sockets_traffic_out() {
        let net = SimNetwork::new(1);
        let mut config = RouterConfig::nat(ip(HOME_OUT), p("192.168.1.0/24"));
        config.idle = Duration::from_secs(5);
        config.forwards.push(Forward {
            outside_port: 26_900,
            inside: a("192.168.1.10:26900"),
        });
        config.ports = PortChoice::Next;
        let router = net.add_router(config).unwrap();
        let mut host = net.bind(a("192.168.1.10:26900")).unwrap();
        let mut master = net.bind(a(MASTER)).unwrap();
        let mut stranger = net.bind(a(OTHER)).unwrap();
        stranger
            .send_datagram(a("203.0.113.10:26900"), b"join")
            .unwrap();
        assert_eq!(drain(&mut host), [(a(OTHER), b"join".to_vec())]);
        host.send_datagram(a(MASTER), b"register").unwrap();
        assert_eq!(senders(&mut master), [a("203.0.113.10:26900")]);
        // It never expires.
        net.advance(Duration::from_secs(600));
        stranger
            .send_datagram(a("203.0.113.10:26900"), b"later")
            .unwrap();
        assert_eq!(drain(&mut host).len(), 1);
        assert_eq!(net.router_stats(router).mappings, 0);
        // A firewall's forward opens the inside address itself.
        let net = SimNetwork::new(1);
        let mut wall = RouterConfig::firewall(p("2001:db8:1::/64"));
        wall.forwards.push(Forward {
            outside_port: 26_900,
            inside: a("[2001:db8:1::10]:26900"),
        });
        net.add_router(wall).unwrap();
        let mut host = net.bind(a("[2001:db8:1::10]:26900")).unwrap();
        let mut stranger = net.bind(a("[2001:db8:9::1]:5000")).unwrap();
        stranger
            .send_datagram(a("[2001:db8:1::10]:26900"), b"join")
            .unwrap();
        assert_eq!(drain(&mut host).len(), 1);
    }

    #[test]
    fn a_dual_socket_sends_from_the_family_of_the_destination() {
        let net = SimNetwork::new(1);
        net.add_router(RouterConfig::nat(ip(HOME_OUT), p("192.168.1.0/24")))
            .unwrap();
        net.add_router(RouterConfig::firewall(p("2001:db8:1::/64")))
            .unwrap();
        let mut player = net
            .bind_dual(a(PLAYER), a("[2001:db8:1::10]:40000"))
            .unwrap();
        assert_eq!(
            player.local_addrs(),
            [a(PLAYER), a("[2001:db8:1::10]:40000")]
        );
        let mut v4 = net.bind(a(MASTER)).unwrap();
        let mut v6 = net.bind(a("[2001:db8:ff::1]:26911")).unwrap();
        player.send_datagram(a(MASTER), b"4").unwrap();
        player
            .send_datagram(a("[2001:db8:ff::1]:26911"), b"6")
            .unwrap();
        assert_eq!(senders(&mut v4), [a("203.0.113.10:40000")]);
        assert_eq!(senders(&mut v6), [a("[2001:db8:1::10]:40000")]);
        v6.send_datagram(a("[2001:db8:1::10]:40000"), b"six")
            .unwrap();
        v4.send_datagram(a("203.0.113.10:40000"), b"four").unwrap();
        let got: Vec<Vec<u8>> = drain(&mut player).into_iter().map(|(_, d)| d).collect();
        assert_eq!(got, [b"six".to_vec(), b"four".to_vec()], "in arrival order");
        drop(player);
        assert!(net.bind(a(PLAYER)).is_ok());
        assert!(net.bind(a("[2001:db8:1::10]:40000")).is_ok());
    }

    #[test]
    fn links_apply_across_routers_by_the_senders_own_address() {
        let (net, _, mut player, [mut master, _, _]) = home(|_| {});
        net.set_link(a(PLAYER), a(MASTER), LinkConfig::one_way(30 * MS));
        net.set_link(
            a(MASTER),
            a("203.0.113.10:40000"),
            LinkConfig::one_way(70 * MS),
        );
        player.send_datagram(a(MASTER), b"out").unwrap();
        net.advance(29 * MS);
        assert!(drain(&mut master).is_empty());
        net.advance(MS);
        assert_eq!(drain(&mut master).len(), 1);
        master
            .send_datagram(a("203.0.113.10:40000"), b"back")
            .unwrap();
        assert_eq!(net.next_arrival(), Some(100 * MS));
        net.advance(69 * MS);
        assert!(drain(&mut player).is_empty());
        net.advance(MS);
        assert_eq!(drain(&mut player).len(), 1);
        assert_eq!(net.link_stats(a(PLAYER), a(MASTER)).sent, 1);
        assert_eq!(net.link_stats(a(MASTER), a("203.0.113.10:40000")).sent, 1);
    }

    #[test]
    fn misplaced_routers_and_sockets_are_refused() {
        let net = SimNetwork::new(1);
        let early = net.bind(a(PLAYER)).unwrap();
        let late = RouterConfig::nat(ip(HOME_OUT), p("192.168.1.0/24"));
        assert!(
            net.add_router(late.clone()).is_err(),
            "a socket is already behind it"
        );
        drop(early);
        net.add_router(late.clone()).unwrap();
        assert!(
            net.add_router(late).is_err(),
            "the same outside address twice"
        );
        assert!(
            net.bind(a("203.0.113.10:1")).is_err(),
            "the router's own address"
        );
        let inside_out = RouterConfig::nat(ip("10.0.0.1"), p("10.0.0.0/8"));
        assert!(net.add_router(inside_out).is_err());
        let mut far = RouterConfig::nat(ip("203.0.113.11"), p("10.0.0.0/8"));
        far.forwards.push(Forward {
            outside_port: 1,
            inside: a("172.16.0.1:1"),
        });
        assert!(net.add_router(far).is_err(), "a forward outside the router");
        let mut lost = RouterConfig::nat(ip("203.0.113.12"), p("10.0.0.0/8"));
        lost.behind = Some(RouterId(99));
        assert!(net.add_router(lost).is_err());
        assert!(net.bind_behind(RouterId(0), a("10.0.0.1:1")).is_err());
    }

    /// A scripted exchange through a carrier and two homes, with loss,
    /// spread and random ports: the trace, the routers' counts and what the
    /// sockets received.
    fn nat_run(seed: u64) -> (Vec<crate::sim::TraceEntry>, Vec<RouterStats>, Vec<Vec<u8>>) {
        let net = SimNetwork::new(seed);
        net.set_default_link(LinkConfig::for_round_trip(100 * MS, 0.2, 0.1, 0.05));
        let mut carrier = RouterConfig::nat(ip("203.0.113.50"), p("100.64.0.0/10"));
        carrier.mapping = Mapping::AddressAndPortDependent;
        carrier.ports = PortChoice::Random;
        let mut home = RouterConfig::nat(ip("100.64.0.1"), p("192.168.1.0/24"));
        home.ports = PortChoice::Random;
        home.idle = Duration::from_millis(400);
        let mut other = RouterConfig::nat(ip("203.0.113.60"), p("192.168.2.0/24"));
        other.ports = PortChoice::Random;
        let routers = [carrier, home, other].map(|c| net.add_router(c).unwrap());
        net.start_trace();
        let mut player = net.bind(a(PLAYER)).unwrap();
        let mut host = net.bind(a("192.168.2.5:26900")).unwrap();
        let mut master = net.bind(a(MASTER)).unwrap();
        let mut got = Vec::new();
        let mut buf = [0u8; 64];
        for i in 0..400u32 {
            if i % 50 == 0 {
                player.send_datagram(a(MASTER), &i.to_le_bytes()).unwrap();
                host.send_datagram(a(MASTER), &i.to_be_bytes()).unwrap();
            }
            while let Some((len, from)) = master.recv_datagram(&mut buf).unwrap() {
                master.send_datagram(from, &buf[..len]).unwrap();
                got.push(from.to_string().into_bytes());
            }
            for s in [&mut player, &mut host] {
                while let Some((len, _)) = s.recv_datagram(&mut buf).unwrap() {
                    got.push(buf[..len].to_vec());
                }
            }
            net.advance(5 * MS);
        }
        let stats = routers.iter().map(|r| net.router_stats(*r)).collect();
        (net.take_trace(), stats, got)
    }

    #[test]
    fn the_same_seed_gives_the_same_trace_through_routers() {
        let one = nat_run(9);
        assert_eq!(one, nat_run(9));
        assert_ne!(one.0, nat_run(10).0);
        // The trace shows each datagram's translated source.
        assert!(
            one.0
                .iter()
                .any(|t| t.sent_as.is_some_and(|s| s.ip() == ip("203.0.113.50")))
        );
        assert!(one.1[1].sent_out > 0 && one.1[0].let_in > 0);
    }
}
