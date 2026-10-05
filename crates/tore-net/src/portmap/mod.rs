//! Asking the router to forward the game port: UPnP (an SSDP search, then
//! the Internet Gateway Device's SOAP actions over HTTP), NAT-PMP
//! (RFC 6886) and PCP (RFC 6887), on the standard library alone.
//!
//! [`PortMapper::map`] asks all three at once and keeps the first that
//! works; it blocks for at most five seconds ([`BUDGET`]), so a hosting game
//! runs it on a thread of its own. The mapping is renewed at half its lease
//! ([`PortMapper::next_renewal`], [`PortMapper::renew`]) and removed when
//! hosting stops ([`PortMapper::remove`]); nothing is removed on drop, and a
//! mapping left behind lapses with its lease. A router whose outside address
//! is itself private or a carrier's is behind a second router: its mapping
//! is removed and the answer says so. The design is
//! [`docs/ARCHITECTURE.md`, "Port mapping"](../../../../docs/ARCHITECTURE.md#port-mapping).
//!
//! Tests never ask a real router: the [`fake`] gateways answer on loopback,
//! and every target ([`MapperConfig::ssdp`], [`MapperConfig::gateway`],
//! [`MapperConfig::gateway_v6`]) is set to them.

pub mod gateway;
mod http;
mod igd;
pub mod keeper;
mod natpmp;
mod pcp;
mod ssdp;
mod xml;

pub use ssdp::SSDP_ADDRESS;

/// Fake gateways on loopback for tests, here and in the crates that host a
/// game: a UPnP device with its SSDP responder, and a PCP and NAT-PMP
/// gateway.
pub mod fake {
    pub use super::igd::{FakeUpnp, FakeUpnpConfig, FakeUpnpMapping};
    pub use super::pcp::{FakeGateway, FakeGatewayConfig, FakeMapping};
}

use crate::Entropy;
use crate::entropy::Rng;
use http::{HttpError, Url};
use igd::SoapError;
use std::fmt;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// The PCP and NAT-PMP port on a gateway.
pub const GATEWAY_PORT: u16 = 5351;
/// The longest [`PortMapper::map`] or [`PortMapper::renew`] blocks.
pub const BUDGET: Duration = Duration::from_secs(5);
/// How long the SSDP search waits for a gateway to answer.
pub const SSDP_WINDOW: Duration = Duration::from_secs(2);
/// The lease asked for.
pub const LEASE: Duration = Duration::from_secs(3600);
/// The longest [`PortMapper::remove`] blocks (agent decision).
pub const REMOVE_BUDGET: Duration = Duration::from_secs(2);
/// After a conflict on the game port, UPnP tries this many next ports.
pub const NEXT_PORTS: u16 = 4;
/// How a mapping is named in the router's table.
pub const DESCRIPTION: &str = "T.O.R.E-Fighters";

/// The HTTP User-Agent.
const AGENT: &str = "T.O.R.E-Fighters UPnP/1.1";
/// The longest a blocking read waits before it looks at the stop flag again.
const SLICE: Duration = Duration::from_millis(50);
/// PCP and NAT-PMP repeat a request after 250 ms, then 500 ms, 1 s and so on
/// (RFC 6886's schedule; agent decision for PCP too, so both fit the budget).
const FIRST_RETRY: Duration = Duration::from_millis(250);
/// The SSDP search is sent again once, after a second without an answer.
const SSDP_RESEND: Duration = Duration::from_secs(1);
/// The most devices one search follows.
const MAX_DEVICES: usize = 4;
/// When two protocols both map the port, the one that loses removes its
/// mapping within this.
const LOSER_REMOVE: Duration = Duration::from_secs(1);

/// A stop flag that is never set, for calls that only end on time.
static NEVER: AtomicBool = AtomicBool::new(false);

/// A way of asking a router to forward a port.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Protocol {
    Upnp,
    NatPmp,
    Pcp,
}

impl Protocol {
    /// Its name as players read it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Upnp => "UPnP",
            Self::NatPmp => "NAT-PMP",
            Self::Pcp => "PCP",
        }
    }

    /// The telemetry report's "Port mapping" value for a mapping made this
    /// way (docs/formats/master-protocol.md, "Reports").
    pub fn telemetry(self) -> u8 {
        match self {
            Self::Upnp => 1,
            Self::NatPmp => 2,
            Self::Pcp => 3,
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What to map and whom to ask.
#[derive(Clone, Debug)]
pub struct MapperConfig {
    /// The game port: the port inside, and the outside port asked for.
    pub port: u16,
    /// Ask by UPnP.
    pub upnp: bool,
    /// Ask by PCP, and by NAT-PMP when the gateway answers that it does not
    /// speak PCP.
    pub pcp: bool,
    /// This machine's global IPv6 address, for a PCP mapping that opens the
    /// router's IPv6 firewall to the game port; `None` asks nothing for IPv6.
    pub ipv6: Option<Ipv6Addr>,
    /// Where the SSDP search goes: [`SSDP_ADDRESS`], or a fake's address.
    pub ssdp: SocketAddr,
    /// The PCP and NAT-PMP gateway; `None` finds it: the system's default
    /// route, else the address that answered the SSDP search, else the local
    /// network's address ending in 1, on [`GATEWAY_PORT`].
    pub gateway: Option<SocketAddr>,
    /// The PCP gateway for IPv6; `None` finds the system's default IPv6
    /// router.
    pub gateway_v6: Option<SocketAddr>,
    /// The lease asked for.
    pub lease: Duration,
    /// The longest `map` and `renew` block.
    pub budget: Duration,
    /// How long the SSDP search waits.
    pub ssdp_window: Duration,
    /// The longest `remove` blocks.
    pub remove_budget: Duration,
    /// The mapping's name in the router's table.
    pub description: String,
    /// Where PCP's nonces come from.
    pub entropy: Entropy,
}

impl MapperConfig {
    /// Forwarding `port` on the real router, by every protocol, with the
    /// standard times.
    pub fn new(port: u16) -> Self {
        Self {
            port,
            upnp: true,
            pcp: true,
            ipv6: None,
            ssdp: SSDP_ADDRESS,
            gateway: None,
            gateway_v6: None,
            lease: LEASE,
            budget: BUDGET,
            ssdp_window: SSDP_WINDOW,
            remove_budget: REMOVE_BUDGET,
            description: DESCRIPTION.to_owned(),
            entropy: Entropy::System,
        }
    }
}

/// How a mapping is renewed and removed.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Route {
    Upnp {
        service: igd::Service,
        client: Ipv4Addr,
    },
    NatPmp {
        gateway: SocketAddr,
    },
    Pcp {
        server: SocketAddr,
        client: IpAddr,
        nonce: [u8; 12],
    },
}

/// A port the router forwards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mapping {
    /// How it was made.
    pub protocol: Protocol,
    /// The address and port players reach from outside: the game's Mapped
    /// candidate. For IPv6, the machine's own address.
    pub outside: SocketAddr,
    /// The game port inside.
    pub inside_port: u16,
    /// The lease granted; zero for a mapping with no end (an old UPnP device
    /// that takes no other).
    pub lease: Duration,
    /// When it was made or last renewed.
    pub made: Instant,
    route: Route,
}

impl Mapping {
    /// When to renew it: at half its lease; never for one with no end.
    pub fn renew_at(&self) -> Option<Instant> {
        (!self.lease.is_zero()).then(|| self.made + self.lease / 2)
    }
}

impl fmt::Display for Mapping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.outside {
            SocketAddr::V4(outside) => write!(
                f,
                "Your router forwards UDP port {} ({}). Friends can join at {outside}.",
                outside.port(),
                self.protocol
            ),
            SocketAddr::V6(outside) => write!(
                f,
                "Your router lets players in on UDP port {} over IPv6 ({}).",
                outside.port(),
                self.protocol
            ),
        }
    }
}

/// Why no mapping was made. Its text is the line the player reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapError {
    /// Nothing answered: no UPnP device, no PCP or NAT-PMP gateway.
    NoAnswer,
    /// The router said no.
    Refused { protocol: Protocol, reason: String },
    /// Another machine already has every outside port tried.
    PortsTaken {
        protocol: Protocol,
        first: u16,
        last: u16,
    },
    /// The router's outside address is private or a carrier's: it is behind
    /// another router, and its mapping (if one was made) has been removed.
    SecondRouter {
        protocol: Protocol,
        outside: Ipv4Addr,
    },
    /// The router has no outside address (it is not connected).
    NoOutsideAddress { protocol: Protocol },
    /// Something else went wrong on this machine or in the answer.
    Failed(String),
}

impl MapError {
    /// How much the error says, for choosing the one to show when every
    /// protocol failed (agent decision): a second router, then taken ports,
    /// no outside address, a refusal, another failure, silence.
    fn weight(&self) -> u8 {
        match self {
            Self::NoAnswer => 0,
            Self::Failed(_) => 1,
            Self::Refused { .. } => 2,
            Self::NoOutsideAddress { .. } => 3,
            Self::PortsTaken { .. } => 4,
            Self::SecondRouter { .. } => 5,
        }
    }

    fn more_telling(self, other: MapError) -> MapError {
        if other.weight() > self.weight() {
            other
        } else {
            self
        }
    }
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAnswer => {
                f.write_str("Your router did not answer a request to forward the port (UPnP, NAT-PMP or PCP).")
            }
            Self::Refused { protocol, reason } => {
                write!(f, "Your router refused to forward the port ({protocol}: {reason}).")
            }
            Self::PortsTaken { protocol, first, last } if first == last => {
                write!(f, "Your router already forwards UDP port {first} to another machine ({protocol}).")
            }
            Self::PortsTaken { protocol, first, last } => write!(
                f,
                "Your router already forwards UDP ports {first} to {last} to another machine ({protocol})."
            ),
            Self::SecondRouter { .. } => f.write_str(
                "Your router is behind another one, so the port could not be opened to the internet.",
            ),
            Self::NoOutsideAddress { .. } => {
                f.write_str("Your router has no internet address, so the port could not be opened.")
            }
            Self::Failed(reason) => write!(f, "The port could not be forwarded: {reason}."),
        }
    }
}

impl std::error::Error for MapError {}

/// What a `map` or `renew` came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapReport {
    /// The IPv4 mapping, or why there is none.
    pub ipv4: Result<Mapping, MapError>,
    /// The IPv6 one, when [`MapperConfig::ipv6`] asked for it.
    pub ipv6: Option<Result<Mapping, MapError>>,
}

impl MapReport {
    /// The telemetry report's "Port mapping" value: 1 UPnP, 2 NAT-PMP,
    /// 3 PCP, 4 tried and failed, 5 behind a second router.
    pub fn telemetry(&self) -> u8 {
        match &self.ipv4 {
            Ok(mapping) => mapping.protocol.telemetry(),
            Err(MapError::SecondRouter { .. }) => 5,
            Err(_) => 4,
        }
    }
}

/// Maps the game port on the router, keeps it renewed and removes it. Every
/// call blocks; run it on a thread of its own.
#[derive(Debug)]
pub struct PortMapper {
    config: MapperConfig,
    /// PCP's nonces, one for each family, drawn once: a gateway knows a
    /// mapping by its nonce and refuses another one for the same port until
    /// the old mapping lapses, so asking again after a failed renewal must
    /// use the same (agent decision).
    nonce4: [u8; 12],
    nonce6: [u8; 12],
    ipv4: Option<Mapping>,
    ipv6: Option<Mapping>,
}

impl PortMapper {
    /// A mapper that holds nothing yet.
    pub fn new(config: MapperConfig) -> Self {
        let mut rng = Rng::new(config.entropy);
        let mut nonce = || {
            let mut nonce = [0u8; 12];
            nonce[..8].copy_from_slice(&rng.next_u64().to_le_bytes());
            nonce[8..].copy_from_slice(&rng.next_u64().to_le_bytes()[..4]);
            nonce
        };
        let (nonce4, nonce6) = (nonce(), nonce());
        Self {
            config,
            nonce4,
            nonce6,
            ipv4: None,
            ipv6: None,
        }
    }

    /// Its configuration.
    pub fn config(&self) -> &MapperConfig {
        &self.config
    }

    /// The IPv4 mapping it holds.
    pub fn mapping(&self) -> Option<&Mapping> {
        self.ipv4.as_ref()
    }

    /// The IPv6 mapping it holds.
    pub fn ipv6_mapping(&self) -> Option<&Mapping> {
        self.ipv6.as_ref()
    }

    /// When the next held mapping is due for renewal.
    pub fn next_renewal(&self) -> Option<Instant> {
        [&self.ipv4, &self.ipv6]
            .into_iter()
            .flatten()
            .filter_map(Mapping::renew_at)
            .min()
    }

    /// Asks for every mapping it does not hold yet, by every protocol at
    /// once, for at most [`MapperConfig::budget`].
    pub fn map(&mut self) -> MapReport {
        let until = Instant::now() + self.config.budget;
        self.map_until(until)
    }

    /// Renews every mapping it holds by the protocol that made it, and asks
    /// again from the start for any that fails and any it does not hold, all
    /// within [`MapperConfig::budget`].
    pub fn renew(&mut self) -> MapReport {
        let until = Instant::now() + self.config.budget;
        if let Some(mapping) = self.ipv4.take() {
            self.ipv4 = refresh(&mapping, &self.config, until).ok();
        }
        if let Some(mapping) = self.ipv6.take() {
            self.ipv6 = refresh(&mapping, &self.config, until).ok();
        }
        self.map_until(until)
    }

    /// Removes every mapping it holds, within [`MapperConfig::remove_budget`].
    /// They are forgotten either way; one that could not be removed lapses
    /// with its lease.
    pub fn remove(&mut self) -> Result<(), MapError> {
        let until = Instant::now() + self.config.remove_budget;
        let mut result = Ok(());
        for mapping in [self.ipv4.take(), self.ipv6.take()].into_iter().flatten() {
            if let Err(error) = remove_mapping(&mapping, until)
                && result.is_ok()
            {
                result = Err(error);
            }
        }
        result
    }

    fn map_until(&mut self, until: Instant) -> MapReport {
        let (nonce4, nonce6) = (self.nonce4, self.nonce6);
        let config = &self.config;
        let want4 = self.ipv4.is_none();
        let want6 = config.ipv6.filter(|_| self.ipv6.is_none());
        let (v4, v6) = thread::scope(|scope| {
            let v6 =
                want6.map(|address| scope.spawn(move || map_ipv6(config, address, nonce6, until)));
            let v4 = want4.then(|| map_ipv4(config, nonce4, until));
            let v6 = v6.map(|worker| {
                worker.join().unwrap_or_else(|_| {
                    Err(MapError::Failed("the IPv6 request stopped".to_owned()))
                })
            });
            (v4, v6)
        });
        let ipv4 = match v4 {
            Some(Ok(mapping)) => {
                self.ipv4 = Some(mapping.clone());
                Ok(mapping)
            }
            Some(Err(error)) => Err(error),
            None => self.ipv4.clone().ok_or(MapError::NoAnswer),
        };
        let ipv6 = match v6 {
            Some(Ok(mapping)) => {
                self.ipv6 = Some(mapping.clone());
                Some(Ok(mapping))
            }
            Some(Err(error)) => Some(Err(error)),
            None => self.ipv6.clone().map(Ok),
        };
        MapReport { ipv4, ipv6 }
    }
}

// ---------------------------------------------------------------------------
// Waiting on a UDP socket
// ---------------------------------------------------------------------------

/// Why an exchange over UDP gave nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Silence {
    NoAnswer,
    Stopped,
    Io(io::ErrorKind),
}

/// Errors that say nothing about the socket: a timeout, a signal, Windows
/// reporting an earlier datagram's peer unreachable, or a datagram longer
/// than the buffer (WSAEMSGSIZE).
fn transient(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock
            | io::ErrorKind::TimedOut
            | io::ErrorKind::Interrupted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionRefused
    ) || (cfg!(windows) && error.raw_os_error() == Some(10040))
}

/// Waits for one datagram until `until`, looking at `stop` at least every
/// [`SLICE`]. `None` when the time is up or `stop` is set.
fn receive(
    socket: &UdpSocket,
    buf: &mut [u8],
    until: Instant,
    stop: &AtomicBool,
) -> io::Result<Option<(usize, SocketAddr)>> {
    loop {
        if stop.load(Ordering::Acquire) {
            return Ok(None);
        }
        let Some(left) = until
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
        else {
            return Ok(None);
        };
        socket.set_read_timeout(Some(left.min(SLICE)))?;
        match socket.recv_from(buf) {
            Ok(received) => return Ok(Some(received)),
            Err(error) if transient(&error) => {}
            Err(error) => return Err(error),
        }
    }
}

/// Sends `request` to `server` and again after 250 ms, 500 ms, 1 s, 2 s and
/// so on, until `accept` takes a datagram from `server`, `until` passes or
/// `stop` is set. Datagrams from anywhere else are ignored.
fn exchange<T>(
    socket: &UdpSocket,
    server: SocketAddr,
    request: &[u8],
    until: Instant,
    stop: &AtomicBool,
    mut accept: impl FnMut(&[u8]) -> Option<T>,
) -> Result<T, Silence> {
    let mut wait = FIRST_RETRY;
    let mut next_send = Instant::now();
    let mut buf = [0u8; 1504];
    loop {
        if stop.load(Ordering::Acquire) {
            return Err(Silence::Stopped);
        }
        let now = Instant::now();
        if now >= until {
            return Err(Silence::NoAnswer);
        }
        if now >= next_send {
            match socket.send_to(request, server) {
                Ok(_) => {}
                Err(error) if transient(&error) => {}
                Err(error) => return Err(Silence::Io(error.kind())),
            }
            next_send = now + wait;
            wait = wait.saturating_mul(2);
        }
        let received = receive(socket, &mut buf, next_send.min(until), stop)
            .map_err(|error| Silence::Io(error.kind()))?;
        if let Some((len, from)) = received
            && from.ip() == server.ip()
            && from.port() == server.port()
            && let Some(value) = accept(&buf[..len])
        {
            return Ok(value);
        }
    }
}

// ---------------------------------------------------------------------------
// The race between protocols
// ---------------------------------------------------------------------------

/// How one protocol's attempt ended.
enum Outcome {
    /// It mapped the port first.
    Won(Mapping),
    /// Another protocol was first; anything this one made is removed.
    Lost,
    Failed(MapError),
}

fn lease_seconds(lease: Duration) -> u32 {
    u32::try_from(lease.as_secs()).unwrap_or(u32::MAX)
}

/// The problem with an outside address, if it is not one the internet can
/// reach.
fn outside_problem(protocol: Protocol, outside: Ipv4Addr) -> Option<MapError> {
    if gateway::is_no_address(outside) {
        Some(MapError::NoOutsideAddress { protocol })
    } else if gateway::is_inner_address(outside) {
        Some(MapError::SecondRouter { protocol, outside })
    } else {
        None
    }
}

/// Claims the race for `mapping`, or removes it when another protocol was
/// first.
fn settle(mapping: Mapping, race: &AtomicBool, config: &MapperConfig) -> Outcome {
    if race
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        Outcome::Won(mapping)
    } else {
        let _ = remove_mapping(
            &mapping,
            Instant::now() + config.remove_budget.min(LOSER_REMOVE),
        );
        Outcome::Lost
    }
}

fn map_ipv4(config: &MapperConfig, nonce: [u8; 12], until: Instant) -> Result<Mapping, MapError> {
    let race = AtomicBool::new(false);
    let (found_tx, found_rx) = mpsc::channel::<Ipv4Addr>();
    let outcomes = thread::scope(|scope| {
        let race = &race;
        let upnp = config
            .upnp
            .then(|| scope.spawn(move || upnp_worker(config, until, race, &found_tx)));
        let pcp = config
            .pcp
            .then(|| scope.spawn(move || pcp_worker(config, nonce, until, race, &found_rx)));
        [upnp, pcp].map(|worker| {
            worker.map(|worker| {
                worker.join().unwrap_or_else(|_| {
                    Outcome::Failed(MapError::Failed("a request stopped".to_owned()))
                })
            })
        })
    });
    let mut error = MapError::NoAnswer;
    for outcome in outcomes.into_iter().flatten() {
        match outcome {
            Outcome::Won(mapping) => return Ok(mapping),
            Outcome::Lost => {}
            Outcome::Failed(failure) => error = error.more_telling(failure),
        }
    }
    Err(error)
}

fn http_failure(error: HttpError) -> MapError {
    match error {
        HttpError::Connect(_) | HttpError::TimedOut | HttpError::Closed | HttpError::Stopped => {
            MapError::NoAnswer
        }
        other => MapError::Failed(format!("UPnP: {other}")),
    }
}

fn soap_failure(error: SoapError) -> MapError {
    match error {
        SoapError::Http(error) => http_failure(error),
        SoapError::Upnp { .. } | SoapError::Status(_) => MapError::Refused {
            protocol: Protocol::Upnp,
            reason: error.to_string(),
        },
        SoapError::Malformed => {
            MapError::Failed("UPnP: the router's answer could not be read".to_owned())
        }
    }
}

fn upnp_worker(
    config: &MapperConfig,
    until: Instant,
    race: &AtomicBool,
    found: &mpsc::Sender<Ipv4Addr>,
) -> Outcome {
    let socket = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
        Ok(socket) => socket,
        Err(error) => return Outcome::Failed(MapError::Failed(format!("UPnP: {error}"))),
    };
    // The search must not leave the local network.
    let _ = socket.set_multicast_ttl_v4(2);
    let search = |socket: &UdpSocket| {
        for target in ssdp::SEARCH_TARGETS {
            // A network with nowhere to send it has no device to answer.
            let _ = socket.send_to(
                ssdp::search_request(config.ssdp, target).as_bytes(),
                config.ssdp,
            );
        }
    };
    search(&socket);
    let started = Instant::now();
    let window_end = (started + config.ssdp_window).min(until);
    let mut resend_at = Some(started + SSDP_RESEND);
    let mut tried: Vec<String> = Vec::new();
    let mut error = MapError::NoAnswer;
    let mut buf = [0u8; ssdp::MAX_ANSWER + 1];
    loop {
        let wait_until = resend_at.map_or(window_end, |at| at.min(window_end));
        let received = match receive(&socket, &mut buf, wait_until, race) {
            Ok(received) => received,
            Err(failure) => {
                return Outcome::Failed(
                    error.more_telling(MapError::Failed(format!("UPnP: {failure}"))),
                );
            }
        };
        if race.load(Ordering::Acquire) {
            return Outcome::Lost;
        }
        let Some((len, from)) = received else {
            if Instant::now() >= window_end {
                break;
            }
            if resend_at.take().is_some() {
                search(&socket);
            }
            continue;
        };
        let Some(answer) = ssdp::parse_answer(&buf[..len]) else {
            continue;
        };
        if tried.len() >= MAX_DEVICES || tried.contains(&answer.location) {
            continue;
        }
        tried.push(answer.location.clone());
        // The description must come from the device that answered (agent
        // decision): an answer cannot send this side to another machine.
        let Some(location) = Url::parse(&answer.location).filter(|url| url.host == from.ip())
        else {
            continue;
        };
        if let IpAddr::V4(ip) = from.ip() {
            let _ = found.send(ip);
        }
        resend_at = None;
        match upnp_device(config, &location, until, race) {
            Outcome::Failed(failure) => error = error.more_telling(failure),
            other => return other,
        }
    }
    Outcome::Failed(error)
}

fn upnp_device(
    config: &MapperConfig,
    location: &Url,
    until: Instant,
    race: &AtomicBool,
) -> Outcome {
    let description = match http::request(location, "GET", &[], &[], until, race) {
        Ok(exchange) if exchange.response.status == 200 => exchange.response.body,
        Ok(exchange) => {
            return Outcome::Failed(MapError::Failed(format!(
                "UPnP: the router's description gave HTTP status {}",
                exchange.response.status
            )));
        }
        Err(HttpError::Stopped) => return Outcome::Lost,
        Err(error) => return Outcome::Failed(http_failure(error)),
    };
    let services = igd::services(&String::from_utf8_lossy(&description), location);
    if services.is_empty() {
        return Outcome::Failed(MapError::Failed(
            "UPnP: the router offers no internet connection service".to_owned(),
        ));
    }
    let mut error = MapError::NoAnswer;
    for service in &services {
        if race.load(Ordering::Acquire) {
            return Outcome::Lost;
        }
        let (outside, local) = match igd::external_address(service, until, race) {
            Ok(found) => found,
            Err(SoapError::Http(HttpError::Stopped)) => return Outcome::Lost,
            Err(failure) => {
                error = error.more_telling(soap_failure(failure));
                continue;
            }
        };
        let IpAddr::V4(client) = local.ip() else {
            continue;
        };
        // Checked before anything is mapped, so there is nothing to remove
        // (agent decision).
        if let Some(problem) = outside_problem(Protocol::Upnp, outside) {
            return Outcome::Failed(problem);
        }
        return upnp_add(config, service, outside, client, until, race);
    }
    Outcome::Failed(error)
}

/// `AddPortMapping` on the game port, then on the next four after a
/// conflict, with a lease of 0 for a device that takes no other.
fn upnp_add(
    config: &MapperConfig,
    service: &igd::Service,
    outside: Ipv4Addr,
    client: Ipv4Addr,
    until: Instant,
    race: &AtomicBool,
) -> Outcome {
    let first = config.port;
    let last = first.saturating_add(NEXT_PORTS);
    let mut port = first;
    let mut lease = lease_seconds(config.lease);
    loop {
        if race.load(Ordering::Acquire) {
            return Outcome::Lost;
        }
        let request = igd::AddMapping {
            external_port: port,
            internal_port: config.port,
            client,
            lease,
            description: &config.description,
        };
        match igd::add_port_mapping(service, &request, until, race) {
            Ok(()) => {
                let mapping = Mapping {
                    protocol: Protocol::Upnp,
                    outside: SocketAddr::new(outside.into(), port),
                    inside_port: config.port,
                    lease: Duration::from_secs(lease.into()),
                    made: Instant::now(),
                    route: Route::Upnp {
                        service: service.clone(),
                        client,
                    },
                };
                return settle(mapping, race, config);
            }
            Err(SoapError::Upnp {
                code: igd::ONLY_PERMANENT_LEASES,
                ..
            }) if lease != 0 => lease = 0,
            Err(SoapError::Upnp {
                code: igd::CONFLICT,
                ..
            }) if port < last => port += 1,
            Err(SoapError::Upnp {
                code: igd::CONFLICT,
                ..
            }) => {
                return Outcome::Failed(MapError::PortsTaken {
                    protocol: Protocol::Upnp,
                    first,
                    last: port,
                });
            }
            Err(SoapError::Upnp {
                code: igd::SAME_PORTS_REQUIRED,
                ..
            }) if port > first => {
                return Outcome::Failed(MapError::PortsTaken {
                    protocol: Protocol::Upnp,
                    first,
                    last: port - 1,
                });
            }
            Err(SoapError::Http(HttpError::Stopped)) => return Outcome::Lost,
            Err(failure) => return Outcome::Failed(soap_failure(failure)),
        }
    }
}

/// Where to ask by PCP and NAT-PMP.
fn pcp_server(
    config: &MapperConfig,
    found: &mpsc::Receiver<Ipv4Addr>,
    until: Instant,
) -> Option<SocketAddr> {
    if let Some(server) = config.gateway {
        return Some(server);
    }
    if let Some(ip) = gateway::default_gateway() {
        return Some((ip, GATEWAY_PORT).into());
    }
    let now = Instant::now();
    let wait = (now + config.ssdp_window)
        .min(until)
        .saturating_duration_since(now);
    if let Ok(ip) = found.recv_timeout(wait) {
        return Some((ip, GATEWAY_PORT).into());
    }
    match gateway::local_address_toward((Ipv4Addr::new(192, 0, 2, 1), 9).into())? {
        IpAddr::V4(local) if !local.is_loopback() => {
            Some((gateway::guess_gateway(local), GATEWAY_PORT).into())
        }
        _ => None,
    }
}

fn pcp_failure(error: pcp::Error) -> MapError {
    match error {
        pcp::Error::Silence(Silence::NoAnswer | Silence::Stopped) => MapError::NoAnswer,
        pcp::Error::Silence(Silence::Io(kind)) => MapError::Failed(format!("PCP: {kind}")),
        pcp::Error::Refused(code) => MapError::Refused {
            protocol: Protocol::Pcp,
            reason: pcp::result_text(code).to_owned(),
        },
    }
}

fn natpmp_failure(error: natpmp::Error) -> MapError {
    match error {
        natpmp::Error::Silence(Silence::NoAnswer | Silence::Stopped) => MapError::NoAnswer,
        natpmp::Error::Silence(Silence::Io(kind)) => MapError::Failed(format!("NAT-PMP: {kind}")),
        natpmp::Error::Refused(code) => MapError::Refused {
            protocol: Protocol::NatPmp,
            reason: natpmp::result_text(code).to_owned(),
        },
    }
}

fn pcp_mapping(
    answer: &pcp::MapAnswer,
    inside_port: u16,
    server: SocketAddr,
    client: IpAddr,
    nonce: [u8; 12],
) -> Mapping {
    Mapping {
        protocol: Protocol::Pcp,
        outside: SocketAddr::new(answer.external_ip, answer.external_port),
        inside_port,
        lease: Duration::from_secs(answer.lifetime.into()),
        made: Instant::now(),
        route: Route::Pcp {
            server,
            client,
            nonce,
        },
    }
}

fn pcp_worker(
    config: &MapperConfig,
    nonce: [u8; 12],
    until: Instant,
    race: &AtomicBool,
    found: &mpsc::Receiver<Ipv4Addr>,
) -> Outcome {
    let Some(server) = pcp_server(config, found, until) else {
        return Outcome::Failed(MapError::NoAnswer);
    };
    if race.load(Ordering::Acquire) {
        return Outcome::Lost;
    }
    let Some(client) = gateway::local_address_toward(server) else {
        return Outcome::Failed(MapError::NoAnswer);
    };
    let no_preference = match client {
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    };
    let request = pcp::MapRequest {
        lifetime: lease_seconds(config.lease),
        client,
        nonce,
        internal_port: config.port,
        external_port: config.port,
        external_ip: no_preference,
    };
    match pcp::request(server, &request, until, race) {
        Ok(pcp::Reply::Mapped(answer)) => {
            let mapping = pcp_mapping(&answer, config.port, server, client, nonce);
            let problem = match answer.external_ip {
                IpAddr::V4(outside) => outside_problem(Protocol::Pcp, outside),
                IpAddr::V6(_) => Some(MapError::Failed(
                    "PCP: the router gave an IPv6 address for an IPv4 mapping".to_owned(),
                )),
            };
            if let Some(problem) = problem {
                let _ = remove_mapping(
                    &mapping,
                    Instant::now() + config.remove_budget.min(LOSER_REMOVE),
                );
                return Outcome::Failed(problem);
            }
            settle(mapping, race, config)
        }
        Ok(pcp::Reply::NatPmpOnly) => natpmp_worker(config, server, until, race),
        Err(pcp::Error::Silence(Silence::Stopped)) => Outcome::Lost,
        Err(error) => Outcome::Failed(pcp_failure(error)),
    }
}

fn natpmp_worker(
    config: &MapperConfig,
    gateway: SocketAddr,
    until: Instant,
    race: &AtomicBool,
) -> Outcome {
    let outside = match natpmp::public_address(gateway, until, race) {
        Ok(outside) => outside,
        Err(natpmp::Error::Silence(Silence::Stopped)) => return Outcome::Lost,
        Err(error) => return Outcome::Failed(natpmp_failure(error)),
    };
    // Checked before anything is mapped, as for UPnP.
    if let Some(problem) = outside_problem(Protocol::NatPmp, outside) {
        return Outcome::Failed(problem);
    }
    match natpmp::map_udp(
        gateway,
        config.port,
        config.port,
        lease_seconds(config.lease),
        until,
        race,
    ) {
        Ok((0, _)) => Outcome::Failed(MapError::Failed(
            "NAT-PMP: the router gave no outside port".to_owned(),
        )),
        Ok((external, lifetime)) => {
            let mapping = Mapping {
                protocol: Protocol::NatPmp,
                outside: SocketAddr::new(outside.into(), external),
                inside_port: config.port,
                lease: Duration::from_secs(lifetime.into()),
                made: Instant::now(),
                route: Route::NatPmp { gateway },
            };
            settle(mapping, race, config)
        }
        Err(natpmp::Error::Silence(Silence::Stopped)) => Outcome::Lost,
        Err(error) => Outcome::Failed(natpmp_failure(error)),
    }
}

/// PCP's MAP for this machine's global IPv6 address: where the router allows
/// it, its IPv6 firewall lets players in on the game port.
fn map_ipv6(
    config: &MapperConfig,
    address: Ipv6Addr,
    nonce: [u8; 12],
    until: Instant,
) -> Result<Mapping, MapError> {
    let server = match config.gateway_v6 {
        Some(server) => server,
        None => {
            let (router, scope) = gateway::default_gateway_v6().ok_or(MapError::NoAnswer)?;
            SocketAddr::V6(SocketAddrV6::new(router, GATEWAY_PORT, 0, scope))
        }
    };
    let request = pcp::MapRequest {
        lifetime: lease_seconds(config.lease),
        client: IpAddr::V6(address),
        nonce,
        internal_port: config.port,
        external_port: config.port,
        external_ip: IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    };
    match pcp::request(server, &request, until, &NEVER) {
        Ok(pcp::Reply::Mapped(answer)) => Ok(pcp_mapping(
            &answer,
            config.port,
            server,
            request.client,
            nonce,
        )),
        Ok(pcp::Reply::NatPmpOnly) => Err(MapError::Refused {
            protocol: Protocol::Pcp,
            reason: "the router speaks only NAT-PMP, which has no IPv6".to_owned(),
        }),
        Err(error) => Err(pcp_failure(error)),
    }
}

/// Asks again for a mapping by the protocol that made it.
fn refresh(mapping: &Mapping, config: &MapperConfig, until: Instant) -> Result<Mapping, MapError> {
    match &mapping.route {
        Route::Upnp { service, client } => {
            let request = igd::AddMapping {
                external_port: mapping.outside.port(),
                internal_port: mapping.inside_port,
                client: *client,
                lease: lease_seconds(mapping.lease),
                description: &config.description,
            };
            igd::add_port_mapping(service, &request, until, &NEVER).map_err(soap_failure)?;
            Ok(Mapping {
                made: Instant::now(),
                ..mapping.clone()
            })
        }
        Route::NatPmp { gateway } => {
            let (external, lifetime) = natpmp::map_udp(
                *gateway,
                mapping.inside_port,
                mapping.outside.port(),
                lease_seconds(config.lease),
                until,
                &NEVER,
            )
            .map_err(natpmp_failure)?;
            Ok(Mapping {
                outside: SocketAddr::new(mapping.outside.ip(), external),
                lease: Duration::from_secs(lifetime.into()),
                made: Instant::now(),
                ..mapping.clone()
            })
        }
        Route::Pcp {
            server,
            client,
            nonce,
        } => {
            // The same nonce: the gateway knows the mapping by it.
            let request = pcp::MapRequest {
                lifetime: lease_seconds(config.lease),
                client: *client,
                nonce: *nonce,
                internal_port: mapping.inside_port,
                external_port: mapping.outside.port(),
                external_ip: mapping.outside.ip(),
            };
            match pcp::request(*server, &request, until, &NEVER) {
                Ok(pcp::Reply::Mapped(answer)) => Ok(pcp_mapping(
                    &answer,
                    mapping.inside_port,
                    *server,
                    *client,
                    *nonce,
                )),
                Ok(pcp::Reply::NatPmpOnly) => Err(MapError::NoAnswer),
                Err(error) => Err(pcp_failure(error)),
            }
        }
    }
}

/// Removes a mapping by the protocol that made it.
fn remove_mapping(mapping: &Mapping, until: Instant) -> Result<(), MapError> {
    match &mapping.route {
        Route::Upnp { service, .. } => {
            igd::delete_port_mapping(service, mapping.outside.port(), until, &NEVER)
                .map_err(soap_failure)
        }
        Route::NatPmp { gateway } => {
            natpmp::map_udp(*gateway, mapping.inside_port, 0, 0, until, &NEVER)
                .map(|_| ())
                .map_err(natpmp_failure)
        }
        Route::Pcp {
            server,
            client,
            nonce,
        } => {
            let request = pcp::MapRequest {
                lifetime: 0,
                client: *client,
                nonce: *nonce,
                internal_port: mapping.inside_port,
                external_port: mapping.outside.port(),
                external_ip: mapping.outside.ip(),
            };
            match pcp::request(*server, &request, until, &NEVER) {
                Ok(pcp::Reply::Mapped(_)) => Ok(()),
                Ok(pcp::Reply::NatPmpOnly) => Err(MapError::NoAnswer),
                Err(error) => Err(pcp_failure(error)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Every test asks the fakes on loopback, never a real router: the
    //! configuration below sets every target, and turns off a protocol whose
    //! fake is absent.

    use super::fake::{FakeGateway, FakeGatewayConfig, FakeUpnp, FakeUpnpConfig};
    use super::*;
    use std::net::TcpListener;

    const PORT: u16 = 26900;
    const OUTSIDE: Ipv4Addr = Ipv4Addr::new(203, 0, 113, 5);

    fn config(upnp: Option<&FakeUpnp>, gateway: Option<&FakeGateway>) -> MapperConfig {
        MapperConfig {
            upnp: upnp.is_some(),
            pcp: gateway.is_some(),
            // Loopback's discard port when there is no fake: never the
            // multicast group.
            ssdp: upnp.map_or((Ipv4Addr::LOCALHOST, 9).into(), FakeUpnp::ssdp_address),
            gateway: Some(gateway.map_or((Ipv4Addr::LOCALHOST, 9).into(), FakeGateway::address)),
            gateway_v6: Some((Ipv6Addr::LOCALHOST, 9).into()),
            entropy: Entropy::Seeded(7),
            ..MapperConfig::new(PORT)
        }
    }

    fn upnp(change: impl FnOnce(&mut FakeUpnpConfig)) -> FakeUpnp {
        let mut config = FakeUpnpConfig::default();
        change(&mut config);
        FakeUpnp::start(config).unwrap()
    }

    fn gateway(change: impl FnOnce(&mut FakeGatewayConfig)) -> FakeGateway {
        let mut config = FakeGatewayConfig::default();
        change(&mut config);
        FakeGateway::start((Ipv4Addr::LOCALHOST, 0).into(), config).unwrap()
    }

    fn mapped(report: &MapReport) -> &Mapping {
        report
            .ipv4
            .as_ref()
            .unwrap_or_else(|error| panic!("{error:?}: {error}"))
    }

    #[test]
    fn upnp_maps_on_an_igd_2_and_removes() {
        let device = upnp(|_| {});
        let mut mapper = PortMapper::new(config(Some(&device), None));
        let report = mapper.map();
        let mapping = mapped(&report).clone();
        assert_eq!(mapping.protocol, Protocol::Upnp);
        assert_eq!(mapping.outside, SocketAddr::new(OUTSIDE.into(), PORT));
        assert_eq!(mapping.lease, LEASE);
        assert_eq!(mapping.renew_at(), Some(mapping.made + LEASE / 2));
        assert_eq!(mapper.next_renewal(), mapping.renew_at());
        assert_eq!(report.telemetry(), 1);
        assert_eq!(report.ipv6, None);
        assert_eq!(
            mapping.to_string(),
            "Your router forwards UDP port 26900 (UPnP). Friends can join at 203.0.113.5:26900."
        );
        let held = device.mappings();
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].external_port, PORT);
        assert_eq!(held[0].internal_port, PORT);
        assert_eq!(held[0].client, "127.0.0.1");
        assert_eq!(held[0].lease, 3600);
        assert_eq!(held[0].description, DESCRIPTION);
        // The WANIPConnection:2 is asked, never the PPP connection listed first.
        assert_eq!(
            device.actions(),
            [
                "/ctl/IPConn GetExternalIPAddress",
                "/ctl/IPConn AddPortMapping"
            ]
        );
        assert!(device.searches() >= 1);

        assert_eq!(mapper.remove(), Ok(()));
        assert!(device.mappings().is_empty());
        assert_eq!(
            device.actions().last().map(String::as_str),
            Some("/ctl/IPConn DeletePortMapping")
        );
        assert_eq!(mapper.mapping(), None);
        assert_eq!(mapper.next_renewal(), None);
    }

    #[test]
    fn upnp_on_an_igd_1_with_chunked_answers() {
        let device = upnp(|c| {
            c.version = 1;
            c.chunked = true;
        });
        let mut mapper = PortMapper::new(config(Some(&device), None));
        let report = mapper.map();
        assert_eq!(mapped(&report).outside.port(), PORT);
        assert_eq!(device.mappings().len(), 1);
        assert_eq!(mapper.remove(), Ok(()));
        assert!(device.mappings().is_empty());
    }

    #[test]
    fn upnp_over_a_ppp_connection() {
        let device = upnp(|c| c.ppp_only = true);
        let mut mapper = PortMapper::new(config(Some(&device), None));
        assert_eq!(mapped(&mapper.map()).protocol, Protocol::Upnp);
        assert_eq!(device.actions()[1], "/ctl/PPPConn AddPortMapping");
    }

    #[test]
    fn a_taken_port_moves_to_the_next() {
        let device = upnp(|c| c.taken = vec![PORT, PORT + 1]);
        let mut mapper = PortMapper::new(config(Some(&device), None));
        let report = mapper.map();
        assert_eq!(mapped(&report).outside.port(), PORT + 2);
        assert_eq!(mapped(&report).inside_port, PORT);
        let held = device.mappings();
        assert_eq!(
            (held[0].external_port, held[0].internal_port),
            (PORT + 2, PORT)
        );
        assert_eq!(mapper.remove(), Ok(()));
        assert!(device.mappings().is_empty());
    }

    #[test]
    fn every_port_taken_is_reported() {
        let device = upnp(|c| c.taken = (PORT..=PORT + NEXT_PORTS).collect());
        let mut mapper = PortMapper::new(config(Some(&device), None));
        let report = mapper.map();
        assert_eq!(
            report.ipv4,
            Err(MapError::PortsTaken {
                protocol: Protocol::Upnp,
                first: PORT,
                last: PORT + 4
            })
        );
        assert_eq!(report.telemetry(), 4);
        assert!(device.mappings().is_empty());
        assert_eq!(device.actions().len(), 6, "one address and five ports");

        let strict = upnp(|c| {
            c.taken = vec![PORT];
            c.same_ports_only = true;
        });
        let report = PortMapper::new(config(Some(&strict), None)).map();
        assert_eq!(
            report.ipv4,
            Err(MapError::PortsTaken {
                protocol: Protocol::Upnp,
                first: PORT,
                last: PORT
            })
        );
    }

    #[test]
    fn a_device_that_takes_only_a_lease_of_zero() {
        let device = upnp(|c| c.permanent_only = true);
        let mut mapper = PortMapper::new(config(Some(&device), None));
        let mapping = mapped(&mapper.map()).clone();
        assert_eq!(mapping.lease, Duration::ZERO);
        assert_eq!(mapping.renew_at(), None);
        assert_eq!(mapper.next_renewal(), None);
        assert_eq!(device.mappings()[0].lease, 0);
        assert_eq!(mapper.remove(), Ok(()));
        assert!(device.mappings().is_empty());
    }

    #[test]
    fn a_upnp_refusal_is_reported() {
        let device = upnp(|c| c.refuse = Some(igd::NOT_AUTHORIZED));
        let report = PortMapper::new(config(Some(&device), None)).map();
        let Err(MapError::Refused {
            protocol: Protocol::Upnp,
            reason,
        }) = &report.ipv4
        else {
            panic!("{report:?}");
        };
        assert_eq!(reason, "error 606, Refused");
        assert_eq!(
            report.ipv4.unwrap_err().to_string(),
            "Your router refused to forward the port (UPnP: error 606, Refused)."
        );
    }

    #[test]
    fn upnp_behind_a_second_router_maps_nothing() {
        for (outside, expected) in [
            ("192.168.0.2", Some(Ipv4Addr::new(192, 168, 0, 2))),
            ("100.64.1.1", Some(Ipv4Addr::new(100, 64, 1, 1))),
            ("0.0.0.0", None),
        ] {
            let device = upnp(|c| c.outside = outside.parse().unwrap());
            let report = PortMapper::new(config(Some(&device), None)).map();
            match expected {
                Some(inner) => {
                    assert_eq!(
                        report.ipv4,
                        Err(MapError::SecondRouter {
                            protocol: Protocol::Upnp,
                            outside: inner
                        })
                    );
                    assert_eq!(report.telemetry(), 5);
                    assert_eq!(
                        report.ipv4.unwrap_err().to_string(),
                        "Your router is behind another one, so the port could not be opened to the internet."
                    );
                }
                None => assert_eq!(
                    report.ipv4,
                    Err(MapError::NoOutsideAddress {
                        protocol: Protocol::Upnp
                    })
                ),
            }
            assert!(device.mappings().is_empty());
            assert_eq!(device.actions(), ["/ctl/IPConn GetExternalIPAddress"]);
        }
    }

    #[test]
    fn pcp_maps_renews_with_the_same_nonce_and_removes() {
        let router = gateway(|c| c.max_lifetime = Some(120));
        let mut mapper = PortMapper::new(config(None, Some(&router)));
        let report = mapper.map();
        let mapping = mapped(&report).clone();
        assert_eq!(mapping.protocol, Protocol::Pcp);
        assert_eq!(mapping.outside, SocketAddr::new(OUTSIDE.into(), PORT));
        assert_eq!(mapping.lease, Duration::from_secs(120));
        assert_eq!(
            mapper.next_renewal(),
            Some(mapping.made + Duration::from_secs(60))
        );
        assert_eq!(report.telemetry(), 3);
        let held = router.mappings();
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].client, IpAddr::V4(Ipv4Addr::LOCALHOST));
        let nonce = held[0].nonce;

        // The fake refuses a known mapping asked with another nonce.
        let renewed = mapper.renew();
        assert_eq!(mapped(&renewed).outside, mapping.outside);
        assert!(mapped(&renewed).made >= mapping.made);
        assert_eq!(router.mappings().len(), 1);
        assert_eq!(router.mappings()[0].nonce, nonce);
        assert_eq!(router.pcp_requests(), 2);

        assert_eq!(mapper.remove(), Ok(()));
        assert!(router.mappings().is_empty());
        assert_eq!(router.natpmp_requests(), 0);
    }

    #[test]
    fn pcp_answers_with_another_nonce_or_from_elsewhere_are_ignored() {
        let router = gateway(|c| {
            c.wrong_nonce_first = true;
            c.decoy_first = true;
            c.assign_port = Some(40_000);
        });
        let mut mapper = PortMapper::new(config(None, Some(&router)));
        let report = mapper.map();
        assert_eq!(
            mapped(&report).outside,
            SocketAddr::new(OUTSIDE.into(), 40_000)
        );
        assert_eq!(mapper.remove(), Ok(()));
        assert!(router.mappings().is_empty());
    }

    #[test]
    fn a_natpmp_gateway_takes_over_when_pcp_is_refused() {
        let router = gateway(|c| {
            c.pcp = false;
            c.decoy_first = true;
        });
        let mut mapper = PortMapper::new(config(None, Some(&router)));
        let report = mapper.map();
        let mapping = mapped(&report).clone();
        assert_eq!(mapping.protocol, Protocol::NatPmp);
        assert_eq!(mapping.outside, SocketAddr::new(OUTSIDE.into(), PORT));
        assert_eq!(mapping.lease, LEASE);
        assert_eq!(report.telemetry(), 2);
        assert_eq!(router.pcp_requests(), 1);
        assert_eq!(router.natpmp_requests(), 2, "the address, then the mapping");
        assert_eq!(router.mappings()[0].protocol, Protocol::NatPmp);

        assert_eq!(mapped(&mapper.renew()).outside, mapping.outside);
        assert_eq!(router.natpmp_requests(), 3);
        assert_eq!(router.mappings().len(), 1);

        assert_eq!(mapper.remove(), Ok(()));
        assert!(router.mappings().is_empty());
    }

    #[test]
    fn pcp_and_natpmp_behind_a_second_router() {
        let router = gateway(|c| c.outside = Ipv4Addr::new(10, 0, 0, 2));
        let report = PortMapper::new(config(None, Some(&router))).map();
        assert_eq!(
            report.ipv4,
            Err(MapError::SecondRouter {
                protocol: Protocol::Pcp,
                outside: Ipv4Addr::new(10, 0, 0, 2)
            })
        );
        assert!(
            router.mappings().is_empty(),
            "the inner router's mapping is removed"
        );
        assert_eq!(router.pcp_requests(), 2, "the mapping and its removal");

        let router = gateway(|c| {
            c.pcp = false;
            c.outside = Ipv4Addr::new(100, 64, 0, 9);
        });
        let report = PortMapper::new(config(None, Some(&router))).map();
        assert_eq!(
            report.ipv4,
            Err(MapError::SecondRouter {
                protocol: Protocol::NatPmp,
                outside: Ipv4Addr::new(100, 64, 0, 9)
            })
        );
        assert!(router.mappings().is_empty());
        assert_eq!(router.natpmp_requests(), 1, "only the address is asked");
    }

    #[test]
    fn pcp_and_natpmp_refusals_are_reported() {
        let router = gateway(|c| c.refuse = Some(pcp::NOT_AUTHORIZED));
        let report = PortMapper::new(config(None, Some(&router))).map();
        assert_eq!(
            report.ipv4,
            Err(MapError::Refused {
                protocol: Protocol::Pcp,
                reason: "not authorized".into()
            })
        );
        let router = gateway(|c| {
            c.pcp = false;
            c.refuse = Some(3);
        });
        let report = PortMapper::new(config(None, Some(&router))).map();
        assert_eq!(
            report.ipv4,
            Err(MapError::Refused {
                protocol: Protocol::NatPmp,
                reason: "network failure".into()
            })
        );
    }

    #[test]
    fn with_both_gateways_one_mapping_remains() {
        for seed in 0..3 {
            let device = upnp(|_| {});
            let router = gateway(|_| {});
            let mut mapper = PortMapper::new(MapperConfig {
                entropy: Entropy::Seeded(seed),
                ..config(Some(&device), Some(&router))
            });
            let report = mapper.map();
            let protocol = mapped(&report).protocol;
            let (upnp_held, pcp_held) = (device.mappings().len(), router.mappings().len());
            assert_eq!(upnp_held + pcp_held, 1, "{protocol:?}");
            assert_eq!(protocol == Protocol::Upnp, upnp_held == 1);
            assert_eq!(mapper.remove(), Ok(()));
            assert!(device.mappings().is_empty() && router.mappings().is_empty());
        }
    }

    #[test]
    fn silent_fakes_end_within_the_budget() {
        let device = upnp(|c| c.silent_ssdp = true);
        let router = gateway(|c| c.silent = true);
        let started = Instant::now();
        let report = PortMapper::new(config(Some(&device), Some(&router))).map();
        let took = started.elapsed();
        assert_eq!(report.ipv4, Err(MapError::NoAnswer));
        assert_eq!(report.telemetry(), 4);
        assert!(took >= BUDGET - Duration::from_millis(100), "{took:?}");
        assert!(took <= BUDGET + Duration::from_millis(500), "{took:?}");
        assert_eq!(device.searches(), 4, "two searches, sent again once");
        assert_eq!(
            router.pcp_requests(),
            5,
            "at 0, 0.25, 0.75, 1.75 and 3.75 s"
        );
    }

    #[test]
    fn a_silent_device_ends_within_the_budget() {
        let device = upnp(|c| c.silent_http = true);
        let budget = Duration::from_millis(1200);
        let started = Instant::now();
        let report = PortMapper::new(MapperConfig {
            budget,
            ..config(Some(&device), None)
        })
        .map();
        let took = started.elapsed();
        assert_eq!(report.ipv4, Err(MapError::NoAnswer));
        assert!(took <= budget + Duration::from_millis(400), "{took:?}");
    }

    #[test]
    fn renewing_and_removing_end_on_time_with_a_silent_gateway() {
        let router = gateway(|_| {});
        let budget = Duration::from_millis(900);
        let remove_budget = Duration::from_millis(500);
        let mut mapper = PortMapper::new(MapperConfig {
            budget,
            remove_budget,
            ..config(None, Some(&router))
        });
        mapped(&mapper.map());
        router.set_silent(true);

        let started = Instant::now();
        let report = mapper.renew();
        let took = started.elapsed();
        assert_eq!(report.ipv4, Err(MapError::NoAnswer));
        assert!(took <= budget + Duration::from_millis(300), "{took:?}");
        assert_eq!(mapper.mapping(), None);

        router.set_silent(false);
        mapped(&mapper.map());
        router.set_silent(true);
        let started = Instant::now();
        assert_eq!(mapper.remove(), Err(MapError::NoAnswer));
        let took = started.elapsed();
        assert!(
            took <= remove_budget + Duration::from_millis(300),
            "{took:?}"
        );
        assert_eq!(mapper.mapping(), None);
    }

    #[test]
    fn a_failed_renewal_maps_again() {
        let device = upnp(|_| {});
        let mut mapper = PortMapper::new(config(Some(&device), None));
        mapped(&mapper.map());
        device.configure(|c| c.refuse = Some(501));
        let report = mapper.renew();
        assert!(
            matches!(
                report.ipv4,
                Err(MapError::Refused {
                    protocol: Protocol::Upnp,
                    ..
                })
            ),
            "{report:?}"
        );
        assert_eq!(mapper.mapping(), None);
        device.configure(|c| c.refuse = None);
        let report = mapper.renew();
        assert_eq!(mapped(&report).outside.port(), PORT);
        assert_eq!(mapper.remove(), Ok(()));
        assert!(device.mappings().is_empty());
    }

    #[test]
    fn an_answer_naming_another_host_is_not_followed() {
        let Ok(elsewhere) = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)) else {
            return; // No IPv6 loopback on this machine.
        };
        elsewhere.set_nonblocking(true).unwrap();
        let responder = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let location = format!("http://{}/rootDesc.xml", elsewhere.local_addr().unwrap());
        let ssdp = responder.local_addr().unwrap();
        let answering = thread::spawn(move || {
            responder
                .set_read_timeout(Some(Duration::from_millis(600)))
                .unwrap();
            let mut buf = [0u8; 2048];
            while let Ok((len, from)) = responder.recv_from(&mut buf) {
                if let Some(answer) = ssdp::fake_answer(&buf[..len], &location) {
                    let _ = responder.send_to(answer.as_bytes(), from);
                }
            }
        });
        let budget = Duration::from_millis(800);
        let mapper_config = MapperConfig {
            upnp: true,
            ssdp,
            budget,
            ssdp_window: budget,
            ..config(None, None)
        };
        let report = PortMapper::new(MapperConfig {
            pcp: false,
            ..mapper_config
        })
        .map();
        assert_eq!(report.ipv4, Err(MapError::NoAnswer));
        assert!(
            elsewhere.accept().is_err(),
            "nothing connected to the other host"
        );
        answering.join().unwrap();
    }

    #[test]
    fn pcp_opens_the_ipv6_firewall() {
        let Ok(router) = FakeGateway::start(
            (Ipv6Addr::LOCALHOST, 0).into(),
            FakeGatewayConfig::default(),
        ) else {
            return; // No IPv6 loopback on this machine.
        };
        let mut mapper = PortMapper::new(MapperConfig {
            ipv6: Some(Ipv6Addr::LOCALHOST),
            gateway_v6: Some(router.address()),
            ..config(None, None)
        });
        let report = mapper.map();
        assert_eq!(report.ipv4, Err(MapError::NoAnswer), "IPv4 asked no one");
        let mapping = report.ipv6.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(mapping.protocol, Protocol::Pcp);
        assert_eq!(
            mapping.outside,
            SocketAddr::new(Ipv6Addr::LOCALHOST.into(), PORT)
        );
        assert_eq!(
            mapping.to_string(),
            "Your router lets players in on UDP port 26900 over IPv6 (PCP)."
        );
        assert_eq!(router.mappings()[0].client, IpAddr::V6(Ipv6Addr::LOCALHOST));
        assert!(mapper.ipv6_mapping().is_some());
        assert_eq!(mapper.remove(), Ok(()));
        assert!(router.mappings().is_empty());

        let natpmp_only = FakeGateway::start(
            (Ipv6Addr::LOCALHOST, 0).into(),
            FakeGatewayConfig {
                pcp: false,
                ..FakeGatewayConfig::default()
            },
        )
        .unwrap();
        let report = PortMapper::new(MapperConfig {
            ipv6: Some(Ipv6Addr::LOCALHOST),
            gateway_v6: Some(natpmp_only.address()),
            ..config(None, None)
        })
        .map();
        assert!(matches!(
            report.ipv6,
            Some(Err(MapError::Refused {
                protocol: Protocol::Pcp,
                ..
            }))
        ));
    }

    #[test]
    fn errors_pick_the_most_telling() {
        let second = MapError::SecondRouter {
            protocol: Protocol::Pcp,
            outside: Ipv4Addr::new(10, 0, 0, 1),
        };
        assert_eq!(MapError::NoAnswer.more_telling(second.clone()), second);
        assert_eq!(second.clone().more_telling(MapError::NoAnswer), second);
        let refused = MapError::Refused {
            protocol: Protocol::Upnp,
            reason: "x".into(),
        };
        assert_eq!(
            MapError::Failed("y".into()).more_telling(refused.clone()),
            refused
        );
        assert_eq!(
            MapError::PortsTaken {
                protocol: Protocol::Upnp,
                first: 1,
                last: 5
            }
            .to_string(),
            "Your router already forwards UDP ports 1 to 5 to another machine (UPnP)."
        );
    }
}
