//! The network simulator: in-process endpoints joined by links with seeded
//! latency, arrival spread, loss, duplication and burst loss, on a virtual
//! clock.
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

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::datagram::Datagrams;
use crate::entropy::SplitMix64;

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
    /// they were sent.
    pub unbound: u64,
}

/// One datagram's fate, for the trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    /// When it was sent.
    pub at: Duration,
    /// From where.
    pub from: SocketAddr,
    /// To where.
    pub to: SocketAddr,
    /// The bytes.
    pub datagram: Vec<u8>,
    /// Copies put on their way: 0 lost, 1 normal, 2 duplicated.
    pub copies: u8,
    /// When each copy arrives. A receiver takes datagrams in arrival order,
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

#[derive(Debug, Default)]
struct LinkState {
    config: Option<LinkConfig>,
    bad: bool,
    stats: LinkStats,
}

#[derive(Debug)]
struct Inner {
    now: Duration,
    rng: SplitMix64,
    default_link: LinkConfig,
    links: BTreeMap<(SocketAddr, SocketAddr), LinkState>,
    inboxes: BTreeMap<SocketAddr, BinaryHeap<Reverse<InFlight>>>,
    order: u64,
    trace: Option<Vec<TraceEntry>>,
}

impl Inner {
    fn send(&mut self, from: SocketAddr, to: SocketAddr, datagram: &[u8]) {
        let now = self.now;
        let default = self.default_link;
        let link = self.links.entry((from, to)).or_default();
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
            let flight = InFlight {
                arrival,
                order: self.order,
                from,
                datagram: datagram.to_vec(),
            };
            match self.inboxes.get_mut(&to) {
                Some(inbox) => inbox.push(Reverse(flight)),
                None => {
                    if let Some(link) = self.links.get_mut(&(from, to)) {
                        link.stats.unbound += 1;
                    }
                }
            }
        }
        if let Some(trace) = self.trace.as_mut() {
            trace.push(TraceEntry {
                at: now,
                from,
                to,
                datagram: datagram.to_vec(),
                copies,
                arrivals,
            });
        }
    }

    fn recv(&mut self, at: SocketAddr, buf: &mut [u8]) -> Option<(usize, SocketAddr)> {
        let now = self.now;
        let inbox = self.inboxes.get_mut(&at)?;
        if inbox.peek().is_none_or(|Reverse(f)| f.arrival > now) {
            return None;
        }
        let Reverse(flight) = inbox.pop()?;
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
                rng: SplitMix64::new(seed),
                default_link: LinkConfig::PERFECT,
                links: BTreeMap::new(),
                inboxes: BTreeMap::new(),
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

    /// The link from `from` to `to`.
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

    /// A socket at `address`; an address already bound is
    /// [`io::ErrorKind::AddrInUse`]. Dropping the socket unbinds it, and
    /// datagrams still on their way to it are lost.
    pub fn bind(&self, address: SocketAddr) -> io::Result<SimSocket> {
        let mut inner = lock(&self.inner);
        if inner.inboxes.contains_key(&address) {
            return Err(io::Error::new(io::ErrorKind::AddrInUse, "address in use"));
        }
        inner.inboxes.insert(address, BinaryHeap::new());
        Ok(SimSocket {
            inner: Arc::clone(&self.inner),
            address,
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

    /// The earliest time a datagram on its way arrives, if any is.
    pub fn next_arrival(&self) -> Option<Duration> {
        lock(&self.inner)
            .inboxes
            .values()
            .filter_map(|inbox| inbox.peek().map(|Reverse(f)| f.arrival))
            .min()
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
    /// `from`, past every link rule. For tests that forge packets.
    pub fn inject(&self, from: SocketAddr, to: SocketAddr, datagram: &[u8]) {
        let mut inner = lock(&self.inner);
        inner.order += 1;
        let flight = InFlight {
            arrival: inner.now,
            order: inner.order,
            from,
            datagram: datagram.to_vec(),
        };
        if let Some(inbox) = inner.inboxes.get_mut(&to) {
            inbox.push(Reverse(flight));
        }
    }
}

/// An endpoint on a [`SimNetwork`].
#[derive(Debug)]
pub struct SimSocket {
    inner: Arc<Mutex<Inner>>,
    address: SocketAddr,
}

impl SimSocket {
    /// The socket's address.
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }
}

impl Datagrams for SimSocket {
    fn send_datagram(&mut self, to: SocketAddr, datagram: &[u8]) -> io::Result<()> {
        lock(&self.inner).send(self.address, to, datagram);
        Ok(())
    }

    fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        Ok(lock(&self.inner).recv(self.address, buf))
    }
}

impl Drop for SimSocket {
    fn drop(&mut self) {
        lock(&self.inner).inboxes.remove(&self.address);
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
}
