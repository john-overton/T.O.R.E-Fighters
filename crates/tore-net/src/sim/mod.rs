//! The network simulator: in-process endpoints joined by links with seeded
//! latency, arrival spread, loss, duplication and burst loss, on a virtual
//! clock, with optional routers (address translation and firewalls) between
//! them.
//!
//! Nothing here reads a real clock or sleeps. The caller moves the clock with
//! [`SimNetwork::advance`] or [`SimNetwork::set_now`]; a datagram sent at
//! time t over a link arrives at t plus the link's latency plus a uniform
//! offset within its spread, so reordering follows from the spread. A whole
//! run is deterministic for a seed, as long as the caller sends in the same
//! order.
//!
//! Each datagram is decided in this order: burst state, loss, then (if it
//! survived) duplication, each copy with its own arrival time.
//!
//! # Routers
//!
//! [`SimNetwork::add_router`] puts a router ([`RouterConfig`], in
//! [`nat`]) in front of a prefix of addresses. A socket bound at an address
//! in that prefix is behind it; a router's outside address may itself be
//! behind another router (a home router behind a carrier's). A datagram
//! passes the routers on the sender's side as it is sent, each translating
//! its source, then crosses the link (keyed, as without routers, by the
//! sender's own address and the address it sent to), then passes the routers
//! on the receiver's side when it arrives, each checking its mappings and
//! filter at that moment. Sockets on the same side of a router reach each
//! other directly. Drops are counted per router and cause
//! ([`SimNetwork::router_stats`]).
//!
//! With no router added the simulator behaves exactly as it always has: the
//! same draws, the same arrivals, the same trace.

pub mod nat;

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::datagram::Datagrams;
use crate::entropy::SplitMix64;

pub use nat::{
    Filtering, Forward, Mapping, PortChoice, Prefix, PrefixParseError, RouterConfig, RouterId,
    RouterStats,
};
use nat::{Realm, Router};

/// Gilbert-Elliott burst loss: the link flips between a good state (the
/// link's normal loss) and a bad state (this loss), checked before each
/// datagram.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurstLoss {
    /// Chance per datagram of going from good to bad.
    pub enter: f64,
    /// Chance per datagram of going from bad back to good.
    pub leave: f64,
    /// Loss while bad.
    pub loss: f64,
}

/// One direction of a link.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkConfig {
    /// The one-way delay's middle.
    pub latency: Duration,
    /// Arrivals spread uniformly within plus or minus this of the latency.
    pub spread: Duration,
    /// Chance a datagram is lost, 0 to 1.
    pub loss: f64,
    /// Chance a surviving datagram arrives twice.
    pub duplicate: f64,
    /// Optional burst loss.
    pub burst: Option<BurstLoss>,
}

impl LinkConfig {
    /// A link that delivers everything at once.
    pub const PERFECT: Self = Self {
        latency: Duration::ZERO,
        spread: Duration::ZERO,
        loss: 0.0,
        duplicate: 0.0,
        burst: None,
    };

    /// A link that delivers everything after `latency`.
    pub fn one_way(latency: Duration) -> Self {
        Self {
            latency,
            ..Self::PERFECT
        }
    }

    /// One direction of a path with round trip `round_trip`, arrivals spread
    /// by `spread_fraction` of the one-way delay (0.1 for plus or minus 10
    /// percent), and the given loss and duplication.
    pub fn for_round_trip(
        round_trip: Duration,
        spread_fraction: f64,
        loss: f64,
        duplicate: f64,
    ) -> Self {
        let latency = round_trip / 2;
        Self {
            latency,
            spread: latency.mul_f64(spread_fraction),
            loss,
            duplicate,
            burst: None,
        }
    }
}

/// What one direction of a link did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkStats {
    /// Datagrams sent into it.
    pub sent: u64,
    /// Datagrams it lost.
    pub lost: u64,
    /// Extra copies it made.
    pub duplicated: u64,
    /// Datagrams dropped because nothing was bound at the destination when
    /// they were sent (behind a router: when they arrived).
    pub unbound: u64,
}

/// One datagram's fate, for the trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    /// When it was sent.
    pub at: Duration,
    /// From where: the sending socket's own address.
    pub from: SocketAddr,
    /// To where.
    pub to: SocketAddr,
    /// The address it left the sender's side from, after the routers there
    /// translated it: `from` itself when no router stands in front of the
    /// sender, `None` when one of them dropped it.
    pub sent_as: Option<SocketAddr>,
    /// The bytes.
    pub datagram: Vec<u8>,
    /// Copies put on their way: 0 lost (or dropped by a router on the way
    /// out), 1 normal, 2 duplicated.
    pub copies: u8,
    /// When each copy arrives (behind a router, at the receiver's routers,
    /// which may still drop it). A receiver takes datagrams in arrival order,
    /// ties in send order.
    pub arrivals: Vec<Duration>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct InFlight {
    arrival: Duration,
    order: u64,
    from: SocketAddr,
    datagram: Vec<u8>,
}

/// A datagram on its way to a router's outside address (or, for a
/// firewall, to an address behind it), decided when it arrives.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ToRouter {
    arrival: Duration,
    order: u64,
    router: RouterId,
    /// The source as the router sees it.
    from: SocketAddr,
    to: SocketAddr,
    /// The sending socket's own address, for the link's count.
    sender: SocketAddr,
    datagram: Vec<u8>,
}

/// Where a datagram meets its receiver's side.
enum Meet {
    /// A socket's inbox, in this realm.
    Socket(Realm),
    /// A router that takes it in when it arrives.
    Router(RouterId),
}

/// A bound address: the realm it is in, and the address.
type End = (Realm, SocketAddr);

#[derive(Debug, Default)]
struct LinkState {
    config: Option<LinkConfig>,
    bad: bool,
    stats: LinkStats,
}

#[derive(Debug)]
struct Inner {
    now: Duration,
    seed: u64,
    rng: SplitMix64,
    default_link: LinkConfig,
    links: BTreeMap<(SocketAddr, SocketAddr), LinkState>,
    inboxes: BTreeMap<End, BinaryHeap<Reverse<InFlight>>>,
    routers: Vec<Router>,
    to_routers: BinaryHeap<Reverse<ToRouter>>,
    order: u64,
    trace: Option<Vec<TraceEntry>>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.to_string())
}

impl Inner {
    /// The router in `realm` that takes datagrams for `ip`, if any.
    fn claimant(&self, realm: Realm, ip: IpAddr) -> Option<RouterId> {
        self.routers
            .iter()
            .position(|r| r.parent == realm && r.claims(ip))
            .map(RouterId)
    }

    /// The realm an address is in: behind the router whose inside prefix
    /// holds it most narrowly, else the open network.
    fn place(&self, ip: IpAddr) -> io::Result<Realm> {
        let mut best: Option<(u8, RouterId)> = None;
        let mut tied = false;
        for (i, router) in self.routers.iter().enumerate() {
            let inside = router.config.inside;
            if !inside.contains(ip) {
                continue;
            }
            match best {
                Some((bits, _)) if bits > inside.bits() => {}
                Some((bits, _)) if bits == inside.bits() => tied = true,
                _ => {
                    best = Some((inside.bits(), RouterId(i)));
                    tied = false;
                }
            }
        }
        if tied {
            return Err(invalid(
                "the address is behind two routers with the same prefix: bind it behind one",
            ));
        }
        Ok(best.map(|(_, id)| id))
    }

    fn bind_end(&mut self, end: End) -> io::Result<()> {
        if self.inboxes.contains_key(&end) {
            return Err(io::Error::new(io::ErrorKind::AddrInUse, "address in use"));
        }
        if self.claimant(end.0, end.1.ip()).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                "the address is a router's own",
            ));
        }
        self.inboxes.insert(end, BinaryHeap::new());
        Ok(())
    }

    fn add_router(&mut self, config: RouterConfig) -> io::Result<RouterId> {
        let id = RouterId(self.routers.len());
        let parent = match config.behind {
            Some(behind) => {
                let Some(router) = self.routers.get(behind.0) else {
                    return Err(invalid("no such router to stand behind"));
                };
                let place = config.outside.unwrap_or(config.inside.address());
                if !router.config.inside.contains(place) {
                    return Err(invalid(
                        "the router's outside address is not behind the router it stands behind",
                    ));
                }
                Some(behind)
            }
            None => self.place(config.outside.unwrap_or(config.inside.address()))?,
        };
        if let Some(outside) = config.outside {
            if config.inside.contains(outside) {
                return Err(invalid("the router's outside address is inside it"));
            }
            if self.claimant(parent, outside).is_some() {
                return Err(invalid("another router has that outside address"));
            }
            if self
                .inboxes
                .keys()
                .any(|(realm, addr)| *realm == parent && addr.ip() == outside)
            {
                return Err(invalid("a socket is bound at the router's outside address"));
            }
        } else if self.routers.iter().any(|r| {
            r.parent == parent
                && (r.claims(config.inside.address())
                    || r.config.outside.is_some_and(|o| config.inside.contains(o)))
        }) {
            return Err(invalid("another router already takes those addresses"));
        }
        if config
            .forwards
            .iter()
            .any(|f| !config.inside.contains(f.inside.ip()))
        {
            return Err(invalid("a forward leads to an address outside the router"));
        }
        if self
            .inboxes
            .keys()
            .any(|(realm, addr)| *realm == parent && config.inside.contains(addr.ip()))
        {
            return Err(invalid(
                "sockets are already bound in the router's prefix: add routers before binding",
            ));
        }
        let seed = self
            .seed
            .wrapping_add(0xD1B5_4A32_D192_ED03_u64.wrapping_mul(id.0 as u64 + 1));
        self.routers.push(Router::new(config, parent, seed));
        Ok(id)
    }

    /// Takes a datagram from `from` in `realm` out through the routers on
    /// the sender's side: where it meets the receiver's side, and the source
    /// it has there. `None` when a router dropped it.
    fn route_out(
        &mut self,
        realm: Realm,
        from: SocketAddr,
        to: SocketAddr,
    ) -> Option<(Meet, SocketAddr)> {
        let (mut realm, mut from) = (realm, from);
        loop {
            if let Some(router) = self.claimant(realm, to.ip()) {
                return Some((Meet::Router(router), from));
            }
            let Some(id) = realm else {
                return Some((Meet::Socket(None), from));
            };
            let router = &mut self.routers[id.0];
            if router.config.inside.contains(to.ip()) {
                return Some((Meet::Socket(realm), from));
            }
            from = router.outbound(self.now, from, to).ok()?;
            realm = router.parent;
        }
    }

    fn send(&mut self, end: End, to: SocketAddr, datagram: &[u8]) {
        self.settle();
        let now = self.now;
        let (realm, sender) = end;
        let routed = self.route_out(realm, sender, to);
        let Some((meet, sent_as)) = routed else {
            if let Some(trace) = self.trace.as_mut() {
                trace.push(TraceEntry {
                    at: now,
                    from: sender,
                    to,
                    sent_as: None,
                    datagram: datagram.to_vec(),
                    copies: 0,
                    arrivals: Vec::new(),
                });
            }
            return;
        };
        let default = self.default_link;
        let link = self.links.entry((sender, to)).or_default();
        let config = link.config.unwrap_or(default);
        link.stats.sent += 1;
        let mut loss = config.loss;
        if let Some(burst) = config.burst {
            let flip = if link.bad { burst.leave } else { burst.enter };
            if self.rng.chance(flip) {
                link.bad = !link.bad;
            }
            if link.bad {
                loss = burst.loss;
            }
        }
        let copies: u8 = if self.rng.chance(loss) {
            link.stats.lost += 1;
            0
        } else if self.rng.chance(config.duplicate) {
            link.stats.duplicated += 1;
            2
        } else {
            1
        };
        let mut arrivals = Vec::new();
        for _ in 0..copies {
            let spread = config.spread.as_micros() as i128;
            let offset = if spread > 0 {
                (self.rng.below((2 * spread + 1) as u64) as i128) - spread
            } else {
                0
            };
            let micros = (now + config.latency).as_micros() as i128 + offset;
            let arrival = Duration::from_micros(micros.max(now.as_micros() as i128) as u64);
            arrivals.push(arrival);
            self.order += 1;
            match meet {
                Meet::Router(router) => self.to_routers.push(Reverse(ToRouter {
                    arrival,
                    order: self.order,
                    router,
                    from: sent_as,
                    to,
                    sender,
                    datagram: datagram.to_vec(),
                })),
                Meet::Socket(realm) => {
                    let flight = InFlight {
                        arrival,
                        order: self.order,
                        from: sent_as,
                        datagram: datagram.to_vec(),
                    };
                    match self.inboxes.get_mut(&(realm, to)) {
                        Some(inbox) => inbox.push(Reverse(flight)),
                        None => {
                            if let Some(link) = self.links.get_mut(&(sender, to)) {
                                link.stats.unbound += 1;
                            }
                        }
                    }
                }
            }
        }
        if let Some(trace) = self.trace.as_mut() {
            trace.push(TraceEntry {
                at: now,
                from: sender,
                to,
                sent_as: Some(sent_as),
                datagram: datagram.to_vec(),
                copies,
                arrivals,
            });
        }
    }

    /// Takes every datagram that has reached a router by now in through the
    /// routers on the receiver's side, each at its own arrival time, into
    /// the receiving socket's inbox. Called before anything sends or reads,
    /// so a router decides each arrival with the state it had at that moment.
    fn settle(&mut self) {
        while self
            .to_routers
            .peek()
            .is_some_and(|Reverse(f)| f.arrival <= self.now)
        {
            let Some(Reverse(flight)) = self.to_routers.pop() else {
                break;
            };
            let mut router = flight.router;
            let mut to = flight.to;
            let end = loop {
                match self.routers[router.0].inbound(flight.arrival, flight.from, to) {
                    Err(_) => break None,
                    Ok(inside) => {
                        to = inside;
                        match self.claimant(Some(router), inside.ip()) {
                            Some(next) => router = next,
                            None => break Some((Some(router), inside)),
                        }
                    }
                }
            };
            let Some(end) = end else { continue };
            match self.inboxes.get_mut(&end) {
                Some(inbox) => inbox.push(Reverse(InFlight {
                    arrival: flight.arrival,
                    order: flight.order,
                    from: flight.from,
                    datagram: flight.datagram,
                })),
                None => {
                    if let Some(link) = self.links.get_mut(&(flight.sender, flight.to)) {
                        link.stats.unbound += 1;
                    }
                }
            }
        }
    }

    fn recv(&mut self, ends: &[End], buf: &mut [u8]) -> Option<(usize, SocketAddr)> {
        self.settle();
        let now = self.now;
        let (_, end) = ends
            .iter()
            .filter_map(|end| {
                let Reverse(head) = self.inboxes.get(end)?.peek()?;
                (head.arrival <= now).then_some(((head.arrival, head.order), *end))
            })
            .min()?;
        let Reverse(flight) = self.inboxes.get_mut(&end)?.pop()?;
        let len = flight.datagram.len().min(buf.len());
        buf[..len].copy_from_slice(&flight.datagram[..len]);
        Some((len, flight.from))
    }
}

/// A simulated network. Cloning gives another handle to the same network.
#[derive(Debug, Clone)]
pub struct SimNetwork {
    inner: Arc<Mutex<Inner>>,
}

fn lock(inner: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl SimNetwork {
    /// A network at time zero where every link is perfect until configured.
    pub fn new(seed: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                now: Duration::ZERO,
                seed,
                rng: SplitMix64::new(seed),
                default_link: LinkConfig::PERFECT,
                links: BTreeMap::new(),
                inboxes: BTreeMap::new(),
                routers: Vec::new(),
                to_routers: BinaryHeap::new(),
                order: 0,
                trace: None,
            })),
        }
    }

    /// The link every direction uses unless [`Self::set_link`] says
    /// otherwise.
    pub fn set_default_link(&self, config: LinkConfig) {
        lock(&self.inner).default_link = config;
    }

    /// The link from `from` to `to`: the sending socket's own address and
    /// the address it sends to, whatever routers stand between.
    pub fn set_link(&self, from: SocketAddr, to: SocketAddr, config: LinkConfig) {
        lock(&self.inner)
            .links
            .entry((from, to))
            .or_default()
            .config = Some(config);
    }

    /// The same link both ways between `a` and `b`.
    pub fn set_link_both(&self, a: SocketAddr, b: SocketAddr, config: LinkConfig) {
        self.set_link(a, b, config);
        self.set_link(b, a, config);
    }

    /// Adds a router. Add routers before binding sockets behind them: a
    /// router whose prefix holds an address already bound on its side is
    /// refused, as are an outside address another router or a socket has, an
    /// outside address inside its own prefix, a forward to an address outside
    /// it, and a placement two routers tie for.
    pub fn add_router(&self, config: RouterConfig) -> io::Result<RouterId> {
        lock(&self.inner).add_router(config)
    }

    /// What a router has done.
    pub fn router_stats(&self, router: RouterId) -> RouterStats {
        lock(&self.inner)
            .routers
            .get(router.0)
            .map(|r| r.stats)
            .unwrap_or_default()
    }

    /// The outside address a router's live mapping (or forward) gives the
    /// inside socket `inside` toward `to`, if it has one now.
    pub fn mapped(
        &self,
        router: RouterId,
        inside: SocketAddr,
        to: SocketAddr,
    ) -> Option<SocketAddr> {
        let inner = lock(&self.inner);
        inner.routers.get(router.0)?.mapped(inner.now, inside, to)
    }

    /// A socket at `address`, behind whichever routers hold it in their
    /// prefixes; an address already bound is [`io::ErrorKind::AddrInUse`].
    /// Dropping the socket unbinds it, and datagrams still on their way to
    /// it are lost.
    pub fn bind(&self, address: SocketAddr) -> io::Result<SimSocket> {
        let mut inner = lock(&self.inner);
        let realm = inner.place(address.ip())?;
        inner.bind_end((realm, address))?;
        Ok(SimSocket {
            inner: Arc::clone(&self.inner),
            ends: vec![(realm, address)],
        })
    }

    /// A socket at `address` behind `router`, for when two routers' prefixes
    /// are the same.
    pub fn bind_behind(&self, router: RouterId, address: SocketAddr) -> io::Result<SimSocket> {
        let mut inner = lock(&self.inner);
        let Some(r) = inner.routers.get(router.0) else {
            return Err(invalid("no such router"));
        };
        if !r.config.inside.contains(address.ip()) {
            return Err(invalid("the address is not behind that router"));
        }
        let end = (Some(router), address);
        inner.bind_end(end)?;
        Ok(SimSocket {
            inner: Arc::clone(&self.inner),
            ends: vec![end],
        })
    }

    /// One socket at an IPv4 and an IPv6 address, as a dual-stack socket on
    /// one port: it sends from the address of the destination's family and
    /// receives at both, in arrival order.
    pub fn bind_dual(&self, v4: SocketAddr, v6: SocketAddr) -> io::Result<SimSocket> {
        if !v4.is_ipv4() || !v6.is_ipv6() {
            return Err(invalid("a dual socket needs an IPv4 and an IPv6 address"));
        }
        let mut inner = lock(&self.inner);
        let ends = vec![(inner.place(v4.ip())?, v4), (inner.place(v6.ip())?, v6)];
        inner.bind_end(ends[0])?;
        if let Err(e) = inner.bind_end(ends[1]) {
            inner.inboxes.remove(&ends[0]);
            return Err(e);
        }
        Ok(SimSocket {
            inner: Arc::clone(&self.inner),
            ends,
        })
    }

    /// The virtual time.
    pub fn now(&self) -> Duration {
        lock(&self.inner).now
    }

    /// Moves the clock to `now`; it never goes back.
    pub fn set_now(&self, now: Duration) {
        let mut inner = lock(&self.inner);
        inner.now = inner.now.max(now);
    }

    /// Moves the clock forward by `step`.
    pub fn advance(&self, step: Duration) {
        let mut inner = lock(&self.inner);
        inner.now += step;
    }

    /// The earliest time a datagram on its way arrives, if any is (behind a
    /// router, possibly one the router then drops).
    pub fn next_arrival(&self) -> Option<Duration> {
        let mut inner = lock(&self.inner);
        inner.settle();
        let at_sockets = inner
            .inboxes
            .values()
            .filter_map(|inbox| inbox.peek().map(|Reverse(f)| f.arrival))
            .min();
        let at_routers = inner.to_routers.peek().map(|Reverse(f)| f.arrival);
        at_sockets.into_iter().chain(at_routers).min()
    }

    /// What the link from `from` to `to` has done.
    pub fn link_stats(&self, from: SocketAddr, to: SocketAddr) -> LinkStats {
        lock(&self.inner)
            .links
            .get(&(from, to))
            .map(|l| l.stats)
            .unwrap_or_default()
    }

    /// Starts recording every datagram sent (and clears any record).
    pub fn start_trace(&self) {
        lock(&self.inner).trace = Some(Vec::new());
    }

    /// The record so far, leaving recording on.
    pub fn take_trace(&self) -> Vec<TraceEntry> {
        lock(&self.inner)
            .trace
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// Puts a datagram straight into `to`'s inbox, arriving now, as if from
    /// `from`, past every link and router rule. For tests that forge packets.
    pub fn inject(&self, from: SocketAddr, to: SocketAddr, datagram: &[u8]) {
        let mut inner = lock(&self.inner);
        let Ok(realm) = inner.place(to.ip()) else {
            return;
        };
        inner.order += 1;
        let flight = InFlight {
            arrival: inner.now,
            order: inner.order,
            from,
            datagram: datagram.to_vec(),
        };
        if let Some(inbox) = inner.inboxes.get_mut(&(realm, to)) {
            inbox.push(Reverse(flight));
        }
    }
}

/// An endpoint on a [`SimNetwork`].
#[derive(Debug)]
pub struct SimSocket {
    inner: Arc<Mutex<Inner>>,
    ends: Vec<End>,
}

impl SimSocket {
    /// The socket's address (a dual socket's IPv4 one).
    pub fn local_addr(&self) -> SocketAddr {
        self.ends[0].1
    }

    /// Every address the socket is bound at.
    pub fn local_addrs(&self) -> Vec<SocketAddr> {
        self.ends.iter().map(|(_, a)| *a).collect()
    }
}

impl Datagrams for SimSocket {
    fn send_datagram(&mut self, to: SocketAddr, datagram: &[u8]) -> io::Result<()> {
        let end = self
            .ends
            .iter()
            .find(|(_, a)| a.is_ipv4() == to.is_ipv4())
            .unwrap_or(&self.ends[0]);
        lock(&self.inner).send(*end, to, datagram);
        Ok(())
    }

    fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        Ok(lock(&self.inner).recv(&self.ends, buf))
    }
}

impl Drop for SimSocket {
    fn drop(&mut self) {
        let mut inner = lock(&self.inner);
        for end in &self.ends {
            inner.inboxes.remove(end);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(port: u16) -> SocketAddr {
        SocketAddr::from(([10, 0, 0, 1], port))
    }

    fn drain(socket: &mut SimSocket) -> Vec<Vec<u8>> {
        let mut buf = [0u8; 64];
        let mut got = Vec::new();
        while let Some((len, _)) = socket.recv_datagram(&mut buf).unwrap() {
            got.push(buf[..len].to_vec());
        }
        got
    }

    #[test]
    fn latency_holds_datagrams_until_due() {
        let net = SimNetwork::new(1);
        net.set_default_link(LinkConfig::one_way(Duration::from_millis(50)));
        let mut a = net.bind(addr(1)).unwrap();
        let mut b = net.bind(addr(2)).unwrap();
        assert!(net.bind(addr(2)).is_err());
        a.send_datagram(addr(2), b"hi").unwrap();
        net.advance(Duration::from_millis(49));
        assert!(drain(&mut b).is_empty());
        assert_eq!(net.next_arrival(), Some(Duration::from_millis(50)));
        net.advance(Duration::from_millis(1));
        assert_eq!(drain(&mut b), vec![b"hi".to_vec()]);
    }

    #[test]
    fn loss_duplication_and_spread_match_their_settings() {
        let net = SimNetwork::new(7);
        let config = LinkConfig {
            latency: Duration::from_millis(100),
            spread: Duration::from_millis(10),
            loss: 0.05,
            duplicate: 0.01,
            burst: None,
        };
        net.set_default_link(config);
        let mut a = net.bind(addr(1)).unwrap();
        let mut b = net.bind(addr(2)).unwrap();
        let n = 100_000u32;
        for i in 0..n {
            a.send_datagram(addr(2), &i.to_le_bytes()).unwrap();
        }
        let stats = net.link_stats(addr(1), addr(2));
        assert_eq!(stats.sent, u64::from(n));
        let loss = stats.lost as f64 / f64::from(n);
        let dup = stats.duplicated as f64 / f64::from(n);
        assert!((loss - 0.05).abs() < 0.003, "loss {loss}");
        assert!((dup - 0.0095).abs() < 0.002, "duplication {dup}");
        net.advance(Duration::from_millis(89));
        assert!(drain(&mut b).is_empty());
        net.advance(Duration::from_millis(21));
        let got = drain(&mut b);
        assert_eq!(got.len() as u64, stats.sent - stats.lost + stats.duplicated);
        // Reordered: arrival order is not send order.
        let order: Vec<u32> = got
            .iter()
            .map(|d| u32::from_le_bytes([d[0], d[1], d[2], d[3]]))
            .collect();
        assert!(order.windows(2).any(|w| w[1] < w[0]));
    }

    #[test]
    fn burst_loss_clusters() {
        let net = SimNetwork::new(3);
        net.set_default_link(LinkConfig {
            burst: Some(BurstLoss {
                enter: 0.01,
                leave: 0.2,
                loss: 1.0,
            }),
            ..LinkConfig::PERFECT
        });
        net.start_trace();
        let mut a = net.bind(addr(1)).unwrap();
        let _b = net.bind(addr(2)).unwrap();
        for _ in 0..20_000 {
            a.send_datagram(addr(2), b"x").unwrap();
        }
        let fates: Vec<bool> = net.take_trace().iter().map(|t| t.copies == 0).collect();
        let lost = fates.iter().filter(|l| **l).count();
        let runs = fates.windows(2).filter(|w| w[0] && !w[1]).count();
        // About 1/21 of datagrams lost, in runs averaging 5.
        assert!(lost > 500 && lost < 1500, "lost {lost}");
        assert!(lost as f64 / runs as f64 > 3.0);
    }

    #[test]
    fn a_seed_repeats_exactly() {
        let run = |seed| {
            let net = SimNetwork::new(seed);
            net.set_default_link(LinkConfig::for_round_trip(
                Duration::from_millis(300),
                0.1,
                0.05,
                0.01,
            ));
            net.start_trace();
            let mut a = net.bind(addr(1)).unwrap();
            for i in 0..1000u32 {
                a.send_datagram(addr(2), &i.to_le_bytes()).unwrap();
            }
            net.take_trace()
        };
        assert_eq!(run(5), run(5));
        assert_ne!(run(5), run(6));
    }

    #[test]
    fn unbound_destinations_swallow_datagrams() {
        let net = SimNetwork::new(1);
        let mut a = net.bind(addr(1)).unwrap();
        let b = net.bind(addr(2)).unwrap();
        drop(b);
        a.send_datagram(addr(2), b"x").unwrap();
        assert_eq!(net.link_stats(addr(1), addr(2)).unbound, 1);
        let mut b = net.bind(addr(2)).unwrap();
        assert!(drain(&mut b).is_empty());
    }

    fn fnv(hash: &mut u64, bytes: &[u8]) {
        for b in bytes {
            *hash ^= u64::from(*b);
            *hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }

    /// With no router the simulator gives what it gave before routers
    /// existed: the fingerprint was taken from the code before stage J
    /// (`bbca6ab8`), over everything received, the trace and the counts.
    #[test]
    fn without_routers_a_seed_gives_what_it_always_gave() {
        let net = SimNetwork::new(11);
        net.set_default_link(LinkConfig {
            latency: Duration::from_millis(40),
            spread: Duration::from_millis(8),
            loss: 0.05,
            duplicate: 0.02,
            burst: Some(BurstLoss {
                enter: 0.02,
                leave: 0.3,
                loss: 0.8,
            }),
        });
        let a_addr: SocketAddr = "10.0.0.1:26900".parse().unwrap();
        let b_addr: SocketAddr = "10.0.0.2:40000".parse().unwrap();
        let nowhere: SocketAddr = "10.0.0.9:1".parse().unwrap();
        net.set_link(
            a_addr,
            b_addr,
            LinkConfig::for_round_trip(Duration::from_millis(150), 0.1, 0.02, 0.01),
        );
        net.start_trace();
        let mut a = net.bind(a_addr).unwrap();
        let mut b = net.bind(b_addr).unwrap();
        let mut hash = 0xCBF2_9CE4_8422_2325u64;
        let mut buf = [0u8; 64];
        for i in 0..2000u32 {
            a.send_datagram(b_addr, &i.to_le_bytes()).unwrap();
            if i % 3 == 0 {
                b.send_datagram(a_addr, &i.to_be_bytes()).unwrap();
            }
            if i % 7 == 0 {
                b.send_datagram(nowhere, b"lost").unwrap();
            }
            net.advance(Duration::from_millis(1));
            for s in [&mut a, &mut b] {
                while let Some((len, from)) = s.recv_datagram(&mut buf).unwrap() {
                    fnv(&mut hash, from.to_string().as_bytes());
                    fnv(&mut hash, &buf[..len]);
                    fnv(&mut hash, &net.now().as_micros().to_le_bytes());
                }
            }
        }
        for t in net.take_trace() {
            assert_eq!(t.sent_as, Some(t.from));
            fnv(&mut hash, &t.at.as_micros().to_le_bytes());
            fnv(&mut hash, t.from.to_string().as_bytes());
            fnv(&mut hash, t.to.to_string().as_bytes());
            fnv(&mut hash, &t.datagram);
            fnv(&mut hash, &[t.copies]);
            for at in t.arrivals {
                fnv(&mut hash, &at.as_micros().to_le_bytes());
            }
        }
        assert_eq!(hash, 0xe8bc_faef_0ea5_f693);
        assert_eq!(
            net.link_stats(a_addr, b_addr),
            LinkStats {
                sent: 2000,
                lost: 28,
                duplicated: 19,
                unbound: 0
            }
        );
        assert_eq!(
            net.link_stats(b_addr, nowhere),
            LinkStats {
                sent: 286,
                lost: 34,
                duplicated: 5,
                unbound: 257
            }
        );
    }
}
