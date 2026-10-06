//! The client's side: joining a host, then one connection.
//!
//! A join goes to one address ([`Client::connect`]) or races several
//! ([`Client::connect_any`], stage J's slice J2): the same Connect request,
//! same nonce, goes to every address the master gave for a host, every
//! 250 ms, and the first Challenge or Refuse that carries the nonce chooses
//! the address. From then on the client is the ordinary client of that one
//! address. A host's Punch that carries the join's introduction id, from an
//! address the client was not told about, adds that address to the race
//! ("Joining from several addresses at once" in the net protocol).

use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use crate::connection::{
    CloseReason, Connection, ConnectionId, DisconnectReason, Event, RefuseReason, SendError, Stats,
};
use crate::entropy::{Entropy, Rng};
use crate::master::Path;
use crate::master::candidate::canonical;
use crate::master::rendezvous::path_of;
use crate::packet::{
    self, ChallengeAnswer, ConnectRequest, MAX_DATAGRAM, Packet, PacketKind, Token, valid_callsign,
};
use crate::platform::Platform;
use crate::{Counters, Datagrams, HANDSHAKE_GIVE_UP, HANDSHAKE_RETRY, MAX_SECTION_KIND, Transmit};

/// The client's settings for one join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    /// The protocol version, kept by the caller (`tore-session`).
    pub protocol_version: u16,
    /// This game's version string.
    pub game_version: String,
    /// The commit this build stamps.
    pub game_commit: String,
    /// 1 to 15 printable ASCII characters.
    pub callsign: String,
    /// The server's password; empty for none.
    pub password: String,
    /// The operating system the game runs on, which the host shows beside
    /// the callsign.
    pub platform: Platform,
    /// The highest section kind a Payload may carry (5 in protocol 1).
    pub max_section_kind: u8,
    /// Where the nonce comes from; [`Entropy::System`] on a real network.
    pub entropy: Entropy,
    /// The rejoin token for the session this join goes to, when the game
    /// holds one (stage K): it goes in the Challenge answer. A game sends it
    /// only to the session that issued it.
    pub token: Option<Token>,
}

impl ClientConfig {
    /// Defaults for a join: no password, empty build strings, this build's
    /// platform, section kinds up to 5, system entropy, no token.
    pub fn new(protocol_version: u16, callsign: &str) -> Self {
        Self {
            protocol_version,
            game_version: String::new(),
            game_commit: String::new(),
            callsign: callsign.to_owned(),
            password: String::new(),
            platform: Platform::current(),
            max_section_kind: MAX_SECTION_KIND,
            entropy: Entropy::System,
            token: None,
        }
    }
}

/// Why a join could not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// The callsign is not 1 to 15 printable ASCII characters.
    BadCallsign,
    /// A version, commit or password over 255 bytes.
    StringTooLong,
    /// No address to join, or more than [`MAX_TARGETS`].
    BadTargets,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BadCallsign => "a callsign is 1 to 15 printable ASCII characters",
            Self::StringTooLong => "a version, commit or password is over 255 bytes",
            Self::BadTargets => "a join tries 1 to 12 addresses",
        })
    }
}

/// The most addresses one join tries at once: the master's candidates (at
/// most 8) and those learned from the host's punches (agent decision: four
/// more, so a host behind a router that gives each destination a new port is
/// still found, and a flood of forged punches cannot grow the race).
pub const MAX_TARGETS: usize = 12;

/// One address a join tries, and how reaching the host there reads (the
/// path byte of the Challenge answer, "The path in the Challenge answer" in
/// the net protocol).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    /// The host's address.
    pub address: SocketAddr,
    /// The path a join that this address answers took.
    pub path: Path,
}

impl Target {
    /// A target with its path.
    pub fn new(address: SocketAddr, path: Path) -> Self {
        Self { address, path }
    }

    /// An address typed or found on the local network: the relay for a
    /// relayed address, the local network for a private, link-local or
    /// loopback one, else by address.
    pub fn typed(address: SocketAddr) -> Self {
        Self::new(address, path_of(address))
    }
}

impl std::error::Error for ConfigError {}

/// What the host said when it accepted the join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Welcome {
    /// This connection's id.
    pub connection: ConnectionId,
    /// The host's session id.
    pub session_id: u64,
    /// Ticks per second (120).
    pub ticks_per_second: u8,
    /// Ticks between snapshots.
    pub ticks_per_snapshot: u8,
    /// The host's tick when it accepted.
    pub host_tick: u32,
}

/// Something that happened on the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientEvent {
    /// The host accepted the join.
    Connected(Welcome),
    /// The join failed or the connection ended; nothing follows.
    Closed(CloseReason),
    /// Something on the established connection.
    Connection(Event),
}

/// Where the client is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    /// Handshaking.
    Connecting,
    /// Joined.
    Connected,
    /// Finished; see the [`ClientEvent::Closed`] event for why.
    Closed,
}

enum State {
    Requesting { last_sent: Duration },
    Answering { last_sent: Duration, sends: u32 },
    Connected(Box<Connection>),
    Closed,
}

/// One join to a host, and then the connection.
///
/// Driven by the caller like [`crate::Server`]: feed it datagrams from the
/// host, call [`Client::update`] often, send what [`Client::poll_transmit`]
/// gives and handle [`Client::poll_event`].
pub struct Client {
    config: ClientConfig,
    server: SocketAddr,
    /// The addresses raced, until the first answer chooses one.
    targets: Vec<Target>,
    /// An address is chosen: `server` is the host.
    chosen: bool,
    /// The path the chosen address stands for.
    path: Path,
    /// The introduction the master made for this join, whose punches teach
    /// the race new addresses.
    introduction: Option<u64>,
    nonce: u64,
    started: Duration,
    request: Vec<u8>,
    answer: Vec<u8>,
    state: State,
    welcome: Option<Welcome>,
    out: VecDeque<Transmit>,
    events: VecDeque<ClientEvent>,
    counters: Counters,
}

impl Client {
    /// Starts joining `server` at `now`: the Connect request is queued at
    /// once and repeated every 250 ms, and the join gives up after 10 seconds.
    /// The path is the address's own ([`Target::typed`]).
    pub fn connect(
        config: ClientConfig,
        server: SocketAddr,
        now: Duration,
    ) -> Result<Self, ConfigError> {
        Self::connect_any(config, &[Target::typed(server)], None, now)
    }

    /// Starts joining one host at several addresses at once (stage J): the
    /// same Connect request goes to every target every 250 ms, and the first
    /// Challenge or Refuse with the join's nonce chooses its target. A
    /// Punch carrying `introduction` from an address not among the targets
    /// adds it, as [`Path::Punched`]. The join gives up after 10 seconds.
    /// Targets repeated (by their canonical address) count once. One target
    /// and no introduction is [`Client::connect`]: chosen from the start.
    pub fn connect_any(
        config: ClientConfig,
        targets: &[Target],
        introduction: Option<u64>,
        now: Duration,
    ) -> Result<Self, ConfigError> {
        let mut unique: Vec<Target> = Vec::new();
        for target in targets {
            if !unique
                .iter()
                .any(|t| canonical(t.address) == canonical(target.address))
            {
                unique.push(*target);
            }
        }
        let Some(first) = unique.first().copied() else {
            return Err(ConfigError::BadTargets);
        };
        if unique.len() > MAX_TARGETS {
            return Err(ConfigError::BadTargets);
        }
        if !valid_callsign(&config.callsign) {
            return Err(ConfigError::BadCallsign);
        }
        let limit = tore_codec::text::MAX_STRING_BYTES;
        if [&config.game_version, &config.game_commit, &config.password]
            .iter()
            .any(|s| s.len() > limit)
        {
            return Err(ConfigError::StringTooLong);
        }
        let nonce = Rng::new(config.entropy).next_u64();
        let request = Packet::ConnectRequest(ConnectRequest {
            protocol_version: config.protocol_version,
            nonce,
            game_version: config.game_version.clone(),
            game_commit: config.game_commit.clone(),
        })
        .encode(config.protocol_version)
        .map_err(|_| ConfigError::StringTooLong)?;
        let mut client = Self {
            config,
            server: first.address,
            chosen: unique.len() == 1 && introduction.is_none(),
            path: first.path,
            targets: unique,
            introduction,
            nonce,
            started: now,
            request,
            answer: Vec::new(),
            state: State::Requesting { last_sent: now },
            welcome: None,
            out: VecDeque::new(),
            events: VecDeque::new(),
            counters: Counters::default(),
        };
        client.send_requests();
        Ok(client)
    }

    /// The Connect request to the chosen address, or to every target while
    /// none is chosen.
    fn send_requests(&mut self) {
        if self.chosen {
            self.out.push_back(Transmit {
                to: self.server,
                datagram: self.request.clone(),
            });
            return;
        }
        for target in &self.targets {
            self.out.push_back(Transmit {
                to: target.address,
                datagram: self.request.clone(),
            });
        }
    }

    /// The host's address: the chosen one, or while racing the first target.
    pub fn server(&self) -> SocketAddr {
        self.server
    }

    /// The path of the chosen address (while racing, the first target's).
    pub fn path(&self) -> Path {
        self.path
    }

    /// True once one address is the host's: given one, or chosen by the
    /// race's first answer.
    pub fn chosen(&self) -> bool {
        self.chosen
    }

    /// The addresses the join tries or tried, those learned from punches
    /// last.
    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    /// Where the join is.
    pub fn state(&self) -> ClientState {
        match self.state {
            State::Requesting { .. } | State::Answering { .. } => ClientState::Connecting,
            State::Connected(_) => ClientState::Connected,
            State::Closed => ClientState::Closed,
        }
    }

    /// What the host said when it accepted, once it has.
    pub fn welcome(&self) -> Option<&Welcome> {
        self.welcome.as_ref()
    }

    /// The connection's statistics, once joined.
    pub fn stats(&self) -> Option<Stats> {
        match &self.state {
            State::Connected(connection) => Some(connection.stats()),
            _ => None,
        }
    }

    /// The Keepalive packet of this connection, once joined: what a
    /// [`crate::Keepalive`] thread sends for the game while its loop is
    /// stalled. `None` while joining and once closed.
    pub fn keepalive_datagram(&self) -> Option<Vec<u8>> {
        let State::Connected(connection) = &self.state else {
            return None;
        };
        Packet::Keepalive(packet::Keepalive {
            connection: connection.id,
        })
        .encode(self.config.protocol_version)
        .ok()
    }

    /// Datagrams dropped before reaching the connection, by cause.
    pub fn counters(&self) -> &Counters {
        &self.counters
    }

    /// Caps the Messages section of each packet (the first due message is
    /// always sent if it fits). The default is the whole packet.
    pub fn set_message_budget(&mut self, bytes: usize) {
        if let State::Connected(connection) = &mut self.state {
            connection.set_message_budget(bytes);
        }
    }

    /// Notes when a packet that arrived at `received` was sent, in the
    /// sender's time (for example the tick it carries), for the arrival
    /// spread statistic.
    pub fn note_arrival(&mut self, sent: Duration, received: Duration) {
        if let State::Connected(connection) = &mut self.state {
            connection.note_arrival(sent, received);
        }
    }

    /// The next event, oldest first.
    pub fn poll_event(&mut self) -> Option<ClientEvent> {
        self.events.pop_front()
    }

    /// The next datagram to send, oldest first.
    pub fn poll_transmit(&mut self) -> Option<Transmit> {
        self.out.pop_front()
    }

    /// Queues a reliable message; allowed only once joined.
    pub fn send_message(&mut self, kind: u8, body: &[u8]) -> Result<(), SendError> {
        match &mut self.state {
            State::Connected(connection) => connection.send_message(kind, body),
            _ => Err(SendError::NotConnected),
        }
    }

    /// Sends a Payload with the caller's sections (kinds 2 and up) and
    /// whatever reliable messages are due and fit. Returns its sequence.
    pub fn send_payload(
        &mut self,
        now: Duration,
        sections: &[(u8, &[u8])],
    ) -> Result<u16, SendError> {
        let result = match &mut self.state {
            State::Connected(connection) => connection.send_payload(now, sections, &mut self.out),
            _ => Err(SendError::NotConnected),
        };
        self.collect();
        result
    }

    /// Leaves: Disconnect is sent three times while connected; a join in
    /// progress just stops. Either way a [`ClientEvent::Closed`] follows.
    pub fn disconnect(&mut self, reason: DisconnectReason) {
        match &mut self.state {
            State::Connected(connection) => connection.close(reason, &mut self.out),
            State::Requesting { .. } | State::Answering { .. } => {
                self.close(CloseReason::Disconnected {
                    reason,
                    by_peer: false,
                });
            }
            State::Closed => {}
        }
        self.collect();
    }

    fn close(&mut self, reason: CloseReason) {
        self.state = State::Closed;
        self.events.push_back(ClientEvent::Closed(reason));
    }

    /// Handshake retries and give-up, then the connection's timeouts,
    /// keepalives and due messages.
    pub fn update(&mut self, now: Duration) {
        let joining = matches!(
            self.state,
            State::Requesting { .. } | State::Answering { .. }
        );
        if joining && now.saturating_sub(self.started) >= HANDSHAKE_GIVE_UP {
            self.close(CloseReason::NoAnswer);
            return;
        }
        match &mut self.state {
            State::Requesting { last_sent } => {
                if now.saturating_sub(*last_sent) >= HANDSHAKE_RETRY {
                    *last_sent = now;
                    self.send_requests();
                }
            }
            State::Answering { last_sent, sends } => {
                if now.saturating_sub(*last_sent) >= HANDSHAKE_RETRY {
                    *last_sent = now;
                    *sends += 1;
                    self.out.push_back(Transmit {
                        to: self.server,
                        datagram: self.answer.clone(),
                    });
                }
            }
            State::Connected(connection) => connection.update(now, &mut self.out),
            State::Closed => {}
        }
        self.collect();
    }

    /// Takes one datagram; anything not from the host (or, while racing,
    /// from a target, or a Punch) is dropped.
    pub fn receive(&mut self, now: Duration, from: SocketAddr, datagram: &[u8]) {
        self.receive_checked(now, from, datagram, &mut |_, _| true);
    }

    /// Takes one datagram, asking `check` about each of the caller's
    /// sections (kinds 2 and up) before anything in the packet is applied.
    /// False drops the whole packet, unacknowledged, and counts it as bad;
    /// `check` must not change the caller's state, since the sections arrive
    /// again as an [`Event::Payload`].
    pub fn receive_checked(
        &mut self,
        now: Duration,
        from: SocketAddr,
        datagram: &[u8],
        check: &mut dyn FnMut(u8, &[u8]) -> bool,
    ) {
        // A dual-stack socket names an IPv4 sender by its IPv4-mapped IPv6
        // address: both forms are the same sender.
        let from = canonical(from);
        if self.chosen && from != canonical(self.server) {
            self.counters.unknown_address += 1;
            return;
        }
        let (kind, body) = match packet::open(datagram, self.config.protocol_version) {
            Ok(opened) => opened,
            Err(_) => {
                self.counters.invalid += 1;
                return;
            }
        };
        if kind == PacketKind::Punch {
            self.on_punch(from, body);
            return;
        }
        let target = self
            .targets
            .iter()
            .position(|t| canonical(t.address) == from);
        if !self.chosen && target.is_none() {
            self.counters.unknown_address += 1;
            return;
        }
        match kind {
            PacketKind::Challenge => self.on_challenge(now, body, target),
            PacketKind::Accepted => self.on_accepted(now, body),
            PacketKind::Refuse => self.on_refuse(body, target),
            PacketKind::Payload => self.on_payload(now, datagram.len(), body, check),
            PacketKind::Disconnect => self.on_disconnect(body),
            PacketKind::ConnectRequest
            | PacketKind::ChallengeAnswer
            | PacketKind::Discover
            | PacketKind::DiscoverAnswer
            | PacketKind::Keepalive
            | PacketKind::Punch
            | PacketKind::Reach
            | PacketKind::ReachAnswer => {
                // A game's peers router (stage K) takes Reach packets before
                // the client sees any.
                self.counters.unexpected += 1;
            }
        }
        self.collect();
    }

    /// The race's first answer with the nonce chooses its target.
    fn choose(&mut self, target: Option<usize>) {
        if self.chosen {
            return;
        }
        if let Some(target) = target.and_then(|i| self.targets.get(i)) {
            self.server = target.address;
            self.path = target.path;
            self.chosen = true;
        }
    }

    /// A host's Punch: with this join's introduction id, from an address the
    /// race does not try yet, it adds that address (the host's router gave
    /// the player another outside port than the master). Punches are
    /// expected while racing and for a moment after; one with another id,
    /// or to a join with no introduction, is counted as unexpected.
    fn on_punch(&mut self, from: SocketAddr, body: &[u8]) {
        let Ok(punch) = packet::decode_punch(body) else {
            self.counters.malformed += 1;
            return;
        };
        if self.introduction != Some(punch.introduction) {
            self.counters.unexpected += 1;
            return;
        }
        let racing = !self.chosen && matches!(self.state, State::Requesting { .. });
        let known = self.targets.iter().any(|t| canonical(t.address) == from);
        if racing && !known && self.targets.len() < MAX_TARGETS {
            self.targets.push(Target::new(from, Path::Punched));
            self.out.push_back(Transmit {
                to: from,
                datagram: self.request.clone(),
            });
        }
    }

    fn on_challenge(&mut self, now: Duration, body: &[u8], target: Option<usize>) {
        let Ok(challenge) = packet::decode_challenge(body) else {
            self.counters.malformed += 1;
            return;
        };
        if !matches!(self.state, State::Requesting { .. }) || challenge.nonce != self.nonce {
            self.counters.unexpected += 1;
            return;
        }
        self.choose(target);
        let answer = Packet::ChallengeAnswer(ChallengeAnswer {
            nonce: self.nonce,
            cookie: challenge.cookie,
            callsign: self.config.callsign.clone(),
            password: self.config.password.clone(),
            game_version: self.config.game_version.clone(),
            game_commit: self.config.game_commit.clone(),
            platform: self.config.platform,
            path: self.path,
            token: self.config.token,
        });
        let Ok(answer) = answer.encode(self.config.protocol_version) else {
            return;
        };
        self.answer = answer;
        self.out.push_back(Transmit {
            to: self.server,
            datagram: self.answer.clone(),
        });
        self.state = State::Answering {
            last_sent: now,
            sends: 1,
        };
    }

    fn on_accepted(&mut self, now: Duration, body: &[u8]) {
        let Ok(accepted) = packet::decode_accepted(body) else {
            self.counters.malformed += 1;
            return;
        };
        let State::Answering { last_sent, sends } = self.state else {
            self.counters.unexpected += 1;
            return;
        };
        if accepted.nonce != self.nonce {
            self.counters.unexpected += 1;
            return;
        }
        // One answer sent: the wait for Accepted is a fair first round trip.
        let first_sample = (sends == 1).then(|| now.saturating_sub(last_sent));
        let connection = Connection::new(
            accepted.connection,
            self.server,
            self.config.protocol_version,
            self.config.max_section_kind,
            now,
            first_sample,
        );
        let welcome = Welcome {
            connection: ConnectionId(accepted.connection),
            session_id: accepted.session_id,
            ticks_per_second: accepted.ticks_per_second,
            ticks_per_snapshot: accepted.ticks_per_snapshot,
            host_tick: accepted.host_tick,
        };
        self.welcome = Some(welcome);
        self.state = State::Connected(Box::new(connection));
        self.events.push_back(ClientEvent::Connected(welcome));
    }

    fn on_refuse(&mut self, body: &[u8], target: Option<usize>) {
        let Ok(refuse) = packet::decode_refuse(body) else {
            self.counters.malformed += 1;
            return;
        };
        let joining = matches!(
            self.state,
            State::Requesting { .. } | State::Answering { .. }
        );
        if !joining || refuse.nonce != self.nonce {
            self.counters.unexpected += 1;
            return;
        }
        self.choose(target);
        self.close(CloseReason::Refused {
            reason: RefuseReason::from_code(refuse.reason),
            text: refuse.text,
        });
    }

    fn on_payload(
        &mut self,
        now: Duration,
        len: usize,
        body: &[u8],
        check: &mut dyn FnMut(u8, &[u8]) -> bool,
    ) {
        let State::Connected(connection) = &mut self.state else {
            self.counters.unexpected += 1;
            return;
        };
        let Ok((header, rest)) = packet::decode_payload_header(body) else {
            self.counters.malformed += 1;
            return;
        };
        if header.connection != connection.id {
            self.counters.stale += 1;
            return;
        }
        let sections = packet::decode_sections(rest);
        connection.receive(now, header, sections, len, check, &mut self.out);
    }

    fn on_disconnect(&mut self, body: &[u8]) {
        let Ok(disconnect) = packet::decode_disconnect(body) else {
            self.counters.malformed += 1;
            return;
        };
        let State::Connected(connection) = &mut self.state else {
            self.counters.unexpected += 1;
            return;
        };
        if disconnect.connection != connection.id {
            self.counters.stale += 1;
            return;
        }
        connection.peer_closed(DisconnectReason::from_code(disconnect.reason));
    }

    fn collect(&mut self) {
        let State::Connected(connection) = &mut self.state else {
            return;
        };
        while let Some(event) = connection.events.pop_front() {
            self.events.push_back(ClientEvent::Connection(event));
        }
        if let Some(reason) = connection.closed.clone() {
            self.close(reason);
        }
    }

    /// Reads every datagram waiting on `socket` (at most 1,024 per call) and
    /// takes each.
    pub fn receive_from<D: Datagrams + ?Sized>(
        &mut self,
        socket: &mut D,
        now: Duration,
    ) -> io::Result<usize> {
        self.receive_from_checked(socket, now, &mut |_, _| true)
    }

    /// [`Client::receive_from`] with a section check, as in
    /// [`Client::receive_checked`].
    pub fn receive_from_checked<D: Datagrams + ?Sized>(
        &mut self,
        socket: &mut D,
        now: Duration,
        check: &mut dyn FnMut(u8, &[u8]) -> bool,
    ) -> io::Result<usize> {
        let mut buf = [0u8; MAX_DATAGRAM + 1];
        let mut count = 0;
        while count < crate::MAX_RECEIVE_BATCH {
            let Some((len, from)) = socket.recv_datagram(&mut buf)? else {
                break;
            };
            self.receive_checked(now, from, &buf[..len], check);
            count += 1;
        }
        Ok(count)
    }

    /// Sends every queued datagram. Keeps going past a failed send and
    /// returns the first error.
    pub fn transmit<D: Datagrams + ?Sized>(&mut self, socket: &mut D) -> io::Result<()> {
        crate::datagram::transmit_all(&mut self.out, socket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::Punch;
    use crate::server::{AcceptInfo, ConnectDetails, Decision, Server, ServerConfig, ServerEvent};
    use crate::sim::SimNetwork;

    const V: u16 = 9;

    fn a(text: &str) -> SocketAddr {
        text.parse().unwrap()
    }

    fn config() -> ClientConfig {
        ClientConfig {
            entropy: Entropy::Seeded(5),
            ..ClientConfig::new(V, "Viper")
        }
    }

    fn accept(_: &ConnectDetails) -> Decision {
        Decision::Accept(AcceptInfo {
            session_id: 1,
            ticks_per_second: 120,
            ticks_per_snapshot: 4,
            host_tick: 0,
        })
    }

    #[test]
    fn the_first_answer_chooses_the_address_and_its_path() {
        let net = SimNetwork::new(3);
        let host_address = a("198.51.100.7:26900");
        let mut host_socket = net.bind(host_address).unwrap();
        let mut socket = net.bind(a("203.0.113.9:40000")).unwrap();
        let mut host = Server::new(ServerConfig {
            entropy: Entropy::Seeded(4),
            ..ServerConfig::new(V)
        });
        let targets = [
            Target::new(a("10.9.9.9:26900"), Path::LocalNetwork),
            Target::new(host_address, Path::Punched),
            Target::new(a("[::ffff:198.51.100.7]:26900"), Path::ByAddress),
        ];
        let mut client = Client::connect_any(config(), &targets, Some(42), net.now()).unwrap();
        // The repeated address counts once; the first is the server until
        // the race chooses.
        assert_eq!(client.targets().len(), 2);
        assert!(!client.chosen());
        let mut details = None;
        for _ in 0..200 {
            net.advance(Duration::from_millis(5));
            let now = net.now();
            host.receive_from(&mut host_socket, now, &mut accept)
                .unwrap();
            host.update(now);
            host.transmit(&mut host_socket).unwrap();
            while let Some(event) = host.poll_event() {
                if let ServerEvent::Connected { details: d, .. } = event {
                    details = Some(d);
                }
            }
            client.receive_from(&mut socket, now).unwrap();
            client.update(now);
            client.transmit(&mut socket).unwrap();
        }
        assert_eq!(client.state(), ClientState::Connected);
        assert!(client.chosen());
        assert_eq!(
            (client.server(), client.path()),
            (host_address, Path::Punched)
        );
        assert_eq!(details.unwrap().path, Path::Punched);
        // The first round of requests went to both targets.
        assert!(
            net.link_stats(a("203.0.113.9:40000"), a("10.9.9.9:26900"))
                .sent
                >= 1
        );
        // From now on another target is an unknown address.
        let before = client.counters().unknown_address;
        let stray = Packet::Challenge(packet::Challenge {
            nonce: client.nonce,
            cookie: 1,
        })
        .encode(V)
        .unwrap();
        client.receive(net.now(), a("10.9.9.9:26900"), &stray);
        assert_eq!(client.counters().unknown_address, before + 1);
    }

    #[test]
    fn a_punch_with_the_introduction_adds_its_address_and_others_are_counted() {
        let now = Duration::from_secs(1);
        let mut client = Client::connect_any(
            config(),
            &[Target::new(a("198.51.100.7:26900"), Path::Punched)],
            Some(42),
            now,
        )
        .unwrap();
        while client.poll_transmit().is_some() {}
        let punch = |id| Packet::Punch(Punch { introduction: id }).encode(V).unwrap();
        // Another id: counted, nothing learned.
        client.receive(now, a("198.51.100.7:31000"), &punch(41));
        assert_eq!(client.counters().unexpected, 1);
        assert_eq!(client.targets().len(), 1);
        // The join's id from a new port: learned, asked at once; seen as an
        // IPv4-mapped address it is the same sender.
        client.receive(now, a("[::ffff:198.51.100.7]:31000"), &punch(42));
        let learned = client.targets()[1];
        assert_eq!(
            (learned.address, learned.path),
            (a("198.51.100.7:31000"), Path::Punched)
        );
        let sent = client
            .poll_transmit()
            .expect("a request to the new address");
        assert_eq!(sent.to, a("198.51.100.7:31000"));
        assert_eq!(sent.datagram.len(), packet::PADDED_LEN);
        // Again from the same address: nothing new.
        client.receive(now, a("198.51.100.7:31000"), &punch(42));
        assert!(client.poll_transmit().is_none());
        // Every retry goes to every address.
        client.update(now + HANDSHAKE_RETRY);
        let tos: Vec<SocketAddr> = std::iter::from_fn(|| client.poll_transmit())
            .map(|t| t.to)
            .collect();
        assert_eq!(tos, [a("198.51.100.7:26900"), a("198.51.100.7:31000")]);
        // The race grows to MAX_TARGETS at most.
        for port in 0..20 {
            client.receive(
                now,
                SocketAddr::new(a("198.51.100.9:1").ip(), 2000 + port),
                &punch(42),
            );
        }
        assert_eq!(client.targets().len(), MAX_TARGETS);
        // A join with no introduction learns nothing from punches.
        let mut plain = Client::connect(config(), a("198.51.100.7:26900"), now).unwrap();
        plain.receive(now, a("198.51.100.7:26900"), &punch(42));
        assert_eq!(plain.counters().unexpected, 1);
        assert_eq!(plain.targets().len(), 1);
    }

    #[test]
    fn a_token_goes_in_the_answer_and_reach_packets_are_counted() {
        let now = Duration::from_secs(1);
        let host = a("198.51.100.7:26900");
        let token = Token(0x5555_u128 << 64 | 0xAAAA);
        let mut client = Client::connect(
            ClientConfig {
                token: Some(token),
                ..config()
            },
            host,
            now,
        )
        .unwrap();
        while client.poll_transmit().is_some() {}
        let challenge = Packet::Challenge(packet::Challenge {
            nonce: client.nonce,
            cookie: 9,
        })
        .encode(V)
        .unwrap();
        client.receive(now, host, &challenge);
        let sent = client.poll_transmit().expect("the answer");
        let Ok(Packet::ChallengeAnswer(answer)) = Packet::decode(&sent.datagram, V) else {
            panic!("not an answer")
        };
        assert_eq!(answer.token, Some(token));
        // Reach and Reach answer are a peers router's, never the client's.
        for packet in [
            Packet::Reach(packet::Reach {
                session_id: 1,
                nonce: 2,
                from: 3,
            }),
            Packet::ReachAnswer(packet::ReachAnswer {
                nonce: 2,
                session_id: 1,
                role: crate::ReachRole::Hosting,
            }),
        ] {
            client.receive(now, host, &packet.encode(V).unwrap());
        }
        assert_eq!(client.counters().unexpected, 2);
        assert!(client.poll_transmit().is_none());
    }

    #[test]
    fn a_join_tries_one_to_twelve_addresses() {
        let now = Duration::ZERO;
        assert_eq!(
            Client::connect_any(config(), &[], None, now).err(),
            Some(ConfigError::BadTargets)
        );
        let many: Vec<Target> = (0..13)
            .map(|i| Target::typed(SocketAddr::new(a("198.51.100.1:1").ip(), 100 + i)))
            .collect();
        assert_eq!(
            Client::connect_any(config(), &many, None, now).err(),
            Some(ConfigError::BadTargets)
        );
        assert!(Client::connect_any(config(), &many[..12], None, now).is_ok());
        // A typed address's path is its own.
        assert_eq!(
            Target::typed(a("192.168.1.5:26900")).path,
            Path::LocalNetwork
        );
        assert_eq!(Target::typed(a("198.51.100.1:26900")).path, Path::ByAddress);
        assert_eq!(Target::typed(a("[100::1:0:5]:0")).path, Path::Relay);
    }
}
