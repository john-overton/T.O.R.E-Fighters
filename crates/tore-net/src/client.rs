//! The client's side: joining a host, then one connection.

use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use crate::connection::{
    CloseReason, Connection, ConnectionId, DisconnectReason, Event, RefuseReason, SendError, Stats,
};
use crate::entropy::{Entropy, Rng};
use crate::packet::{
    self, ChallengeAnswer, ConnectRequest, MAX_DATAGRAM, Packet, PacketKind, valid_callsign,
};
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
    /// The highest section kind a Payload may carry (5 in protocol 1).
    pub max_section_kind: u8,
    /// Where the nonce comes from; [`Entropy::System`] on a real network.
    pub entropy: Entropy,
}

impl ClientConfig {
    /// Defaults for a join: no password, empty build strings, section kinds
    /// up to 5, system entropy.
    pub fn new(protocol_version: u16, callsign: &str) -> Self {
        Self {
            protocol_version,
            game_version: String::new(),
            game_commit: String::new(),
            callsign: callsign.to_owned(),
            password: String::new(),
            max_section_kind: MAX_SECTION_KIND,
            entropy: Entropy::System,
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
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BadCallsign => "a callsign is 1 to 15 printable ASCII characters",
            Self::StringTooLong => "a version, commit or password is over 255 bytes",
        })
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
    pub fn connect(
        config: ClientConfig,
        server: SocketAddr,
        now: Duration,
    ) -> Result<Self, ConfigError> {
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
            server,
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
        client.out.push_back(Transmit {
            to: server,
            datagram: client.request.clone(),
        });
        Ok(client)
    }

    /// The host's address.
    pub fn server(&self) -> SocketAddr {
        self.server
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
                    self.out.push_back(Transmit {
                        to: self.server,
                        datagram: self.request.clone(),
                    });
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

    /// Takes one datagram; anything not from the host is dropped.
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
        if from != self.server {
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
        match kind {
            PacketKind::Challenge => self.on_challenge(now, body),
            PacketKind::Accepted => self.on_accepted(now, body),
            PacketKind::Refuse => self.on_refuse(body),
            PacketKind::Payload => self.on_payload(now, datagram.len(), body, check),
            PacketKind::Disconnect => self.on_disconnect(body),
            PacketKind::ConnectRequest
            | PacketKind::ChallengeAnswer
            | PacketKind::Discover
            | PacketKind::DiscoverAnswer => {
                self.counters.unexpected += 1;
            }
        }
        self.collect();
    }

    fn on_challenge(&mut self, now: Duration, body: &[u8]) {
        let Ok(challenge) = packet::decode_challenge(body) else {
            self.counters.malformed += 1;
            return;
        };
        if !matches!(self.state, State::Requesting { .. }) || challenge.nonce != self.nonce {
            self.counters.unexpected += 1;
            return;
        }
        let answer = Packet::ChallengeAnswer(ChallengeAnswer {
            nonce: self.nonce,
            cookie: challenge.cookie,
            callsign: self.config.callsign.clone(),
            password: self.config.password.clone(),
            game_version: self.config.game_version.clone(),
            game_commit: self.config.game_commit.clone(),
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

    fn on_refuse(&mut self, body: &[u8]) {
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
