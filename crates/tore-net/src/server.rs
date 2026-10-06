//! The host's side: the stateless handshake and every connection.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use crate::connection::{
    CloseReason, Connection, ConnectionId, DisconnectReason, Event, RefuseReason, SendError, Stats,
};
use crate::entropy::{CookieKey, Entropy, Rng};
use crate::master::Path;
use crate::master::routed::is_relayed;
use crate::packet::{
    self, Accepted, Challenge, Discover, DiscoverAnswer, MAX_DATAGRAM, MAX_REFUSE_TEXT, Packet,
    PacketKind, ReachAnswer, ReachRole, Refuse, Token,
};
use crate::platform::Platform;
use crate::{
    COOKIE_SLOT, Counters, Datagrams, MAX_SECTION_KIND, RATE_LIMIT_PER_ADDRESS, RATE_LIMIT_TOTAL,
    REACH_PER_ADDRESS, REACH_PER_IP, Transmit,
};

/// The host's settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// The protocol version, kept by the caller (`tore-session`).
    pub protocol_version: u16,
    /// Connections at most; a join beyond it is refused as "server full"
    /// before the gate is asked.
    pub max_connections: usize,
    /// The highest section kind a Payload may carry (5 in protocol 1).
    pub max_section_kind: u8,
    /// Where secrets come from; [`Entropy::System`] on a real network.
    pub entropy: Entropy,
}

impl ServerConfig {
    /// Defaults for `protocol_version`: 30 connections, section kinds up to
    /// 5, system entropy.
    pub fn new(protocol_version: u16) -> Self {
        Self {
            protocol_version,
            max_connections: 30,
            max_section_kind: MAX_SECTION_KIND,
            entropy: Entropy::System,
        }
    }
}

/// What a client told the host when it asked to join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectDetails {
    /// The address and port it joins from.
    pub address: SocketAddr,
    /// Its protocol version (equal to the host's by the time the gate sees
    /// it).
    pub protocol_version: u16,
    /// Its game version string.
    pub game_version: String,
    /// The commit its build stamps.
    pub game_commit: String,
    /// 1 to 15 printable ASCII characters. Making it unique (`Viper_2`) is
    /// the caller's job.
    pub callsign: String,
    /// The password it gave; may be empty.
    pub password: String,
    /// The operating system its game runs on, as it says.
    pub platform: Platform,
    /// How it reached the host (protocol 9): what its Challenge answer says,
    /// or the relay for a relayed address whatever the answer says.
    pub path: Path,
    /// Its rejoin token for this session, when its game holds one (stage K,
    /// protocol 13). The transport reads nothing into it: the gate does.
    pub token: Option<Token>,
}

/// What the host tells an accepted client, besides its connection id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptInfo {
    /// The session's id.
    pub session_id: u64,
    /// Ticks per second (120).
    pub ticks_per_second: u8,
    /// Ticks between snapshots.
    pub ticks_per_snapshot: u8,
    /// The host's tick now.
    pub host_tick: u32,
}

/// The gate's answer to a join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Let the client in.
    Accept(AcceptInfo),
    /// Turn it away with a reason and a text of up to 200 bytes (longer
    /// text is cut at a character boundary).
    Refuse {
        /// The reason code.
        reason: RefuseReason,
        /// The text the player sees.
        text: String,
    },
}

/// The caller's decisions during [`Server::receive`]: whether to accept a
/// join, and whether a Payload's own sections pass their checks.
///
/// A closure `FnMut(&ConnectDetails) -> Decision` is a gate that accepts
/// every section.
pub trait Gate {
    /// Accept or refuse a join whose cookie is good. Called once per new
    /// connection; a repeated answer gets the same Accepted without asking.
    fn accept(&mut self, details: &ConnectDetails) -> Decision;

    /// Checks one of the caller's sections (kind 2 and up) before anything in
    /// the packet is applied. False drops the whole packet, unacknowledged,
    /// and counts it as bad. It must not change the caller's state: the
    /// section arrives again as an [`Event::Payload`].
    fn check_section(&mut self, connection: ConnectionId, kind: u8, body: &[u8]) -> bool {
        let _ = (connection, kind, body);
        true
    }
}

impl<F: FnMut(&ConnectDetails) -> Decision> Gate for F {
    fn accept(&mut self, details: &ConnectDetails) -> Decision {
        self(details)
    }
}

/// Something that happened on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    /// A join was accepted.
    Connected {
        /// The new connection.
        connection: ConnectionId,
        /// What the client sent.
        details: ConnectDetails,
    },
    /// A connection ended. Every connection ends with exactly one of these.
    Closed {
        /// The connection.
        connection: ConnectionId,
        /// Its address.
        address: SocketAddr,
        /// Why.
        reason: CloseReason,
    },
    /// Something on an established connection.
    Connection {
        /// The connection.
        connection: ConnectionId,
        /// What.
        event: Event,
    },
    /// Someone asked who is hosting here (slice EF5). The caller answers with
    /// [`Server::answer_discover`], or ignores it. The query has passed the
    /// rate limit.
    Discover {
        /// Who asked.
        from: SocketAddr,
        /// What it carries.
        query: Discover,
    },
}

struct Entry {
    connection: Connection,
    nonce: u64,
    accepted: Vec<u8>,
}

/// Connect requests and answers answered per address and in all, per whole
/// second of the host's clock.
#[derive(Debug, Default)]
struct RateLimiter {
    second: u64,
    total: u32,
    per_address: HashMap<IpAddr, u32>,
}

impl RateLimiter {
    fn allow(&mut self, now: Duration, address: IpAddr) -> bool {
        let second = now.as_secs();
        if second != self.second {
            self.second = second;
            self.total = 0;
            self.per_address.clear();
        }
        if self.total >= RATE_LIMIT_TOTAL {
            return false;
        }
        let count = self.per_address.entry(address).or_insert(0);
        if *count >= RATE_LIMIT_PER_ADDRESS {
            return false;
        }
        *count += 1;
        self.total += 1;
        true
    }
}

/// The Reach answerer's rate limits, a whole second of the caller's clock at
/// a time: each address and port ([`REACH_PER_ADDRESS`]), each IP address
/// ([`REACH_PER_IP`]) and all together ([`RATE_LIMIT_TOTAL`]). A host's
/// transport and the peers router keep one each (slice KP).
#[derive(Debug, Default)]
pub(crate) struct ReachLimiter {
    second: u64,
    total: u32,
    per_address: HashMap<SocketAddr, u32>,
    per_ip: HashMap<IpAddr, u32>,
}

impl ReachLimiter {
    pub(crate) fn allow(&mut self, now: Duration, from: SocketAddr) -> bool {
        let second = now.as_secs();
        if second != self.second {
            self.second = second;
            self.total = 0;
            self.per_address.clear();
            self.per_ip.clear();
        }
        if self.total >= RATE_LIMIT_TOTAL {
            return false;
        }
        if self
            .per_address
            .get(&from)
            .is_some_and(|&n| n >= REACH_PER_ADDRESS)
            || self
                .per_ip
                .get(&from.ip())
                .is_some_and(|&n| n >= REACH_PER_IP)
        {
            return false;
        }
        *self.per_address.entry(from).or_insert(0) += 1;
        *self.per_ip.entry(from.ip()).or_insert(0) += 1;
        self.total += 1;
        true
    }
}

/// The host: answers joins without keeping state until a client proves its
/// address, then keeps one connection per client.
///
/// It is driven entirely by the caller, which passes the time in: feed it
/// datagrams with [`Server::receive`], call [`Server::update`] often (every
/// tick), send what [`Server::poll_transmit`] gives, and handle
/// [`Server::poll_event`].
pub struct Server {
    config: ServerConfig,
    rng: Rng,
    cookie_key: CookieKey,
    entries: BTreeMap<SocketAddr, Entry>,
    ids: HashMap<u32, SocketAddr>,
    limiter: RateLimiter,
    /// Discover queries have a limiter of their own, with the same limits, so
    /// a flood of them cannot use up the joins' allowance (agent decision).
    discover_limiter: RateLimiter,
    /// Reach packets have a limiter of their own too: 10 a second from one
    /// address and port, 160 from one IP address (stage K, slice KP).
    reach_limiter: ReachLimiter,
    /// The session a Reach must name to be answered, as hosting it (role 1);
    /// `None` answers none (stage K, slice K0).
    reach_session: Option<u64>,
    out: VecDeque<Transmit>,
    events: VecDeque<ServerEvent>,
    counters: Counters,
}

impl Server {
    /// A host with no connections.
    pub fn new(config: ServerConfig) -> Self {
        let mut rng = Rng::new(config.entropy);
        let cookie_key = CookieKey::new(config.entropy, &mut rng);
        Self {
            config,
            rng,
            cookie_key,
            entries: BTreeMap::new(),
            ids: HashMap::new(),
            limiter: RateLimiter::default(),
            discover_limiter: RateLimiter::default(),
            reach_limiter: ReachLimiter::default(),
            reach_session: None,
            out: VecDeque::new(),
            events: VecDeque::new(),
            counters: Counters::default(),
        }
    }

    /// The settings.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Answers a Reach that names `session` as the game that hosts it (role
    /// 1), at most 10 a second from one address; `None` answers none. A host
    /// sets its own session's id (stage K): a returning old host, or a
    /// client, asks a standby this way whether it hosts now.
    pub fn set_reach_session(&mut self, session: Option<u64>) {
        self.reach_session = session;
    }

    /// Datagrams dropped before reaching a connection, by cause.
    pub fn counters(&self) -> &Counters {
        &self.counters
    }

    /// The established connections, in address order.
    pub fn connections(&self) -> impl Iterator<Item = ConnectionId> + '_ {
        self.entries.values().map(|e| ConnectionId(e.connection.id))
    }

    /// A connection's address.
    pub fn address(&self, connection: ConnectionId) -> Option<SocketAddr> {
        self.ids.get(&connection.0).copied()
    }

    /// A connection's statistics.
    pub fn stats(&self, connection: ConnectionId) -> Option<Stats> {
        self.entry(connection).map(|e| e.connection.stats())
    }

    /// Caps the Messages section of each packet to this connection (the
    /// first due message is always sent if it fits the packet). The default
    /// is the whole packet; 256 during flight.
    pub fn set_message_budget(&mut self, connection: ConnectionId, bytes: usize) {
        if let Some(e) = self.entry_mut(connection) {
            e.connection.set_message_budget(bytes);
        }
    }

    /// Exempts a connection from the 5-second silence timeout, or not. The
    /// caller decides which: a game that hosts exempts its own player's
    /// connection over the in-process link, whose silence is the game's
    /// window being held (dragged, resized, loading) rather than a lost
    /// player; if that game goes away, the host goes with it.
    pub fn set_silence_exempt(&mut self, connection: ConnectionId, exempt: bool) {
        if let Some(e) = self.entry_mut(connection) {
            e.connection.set_silence_exempt(exempt);
        }
    }

    /// Notes when a packet that arrived at `received` was sent, in the
    /// sender's time (for example the tick it carries), for the arrival
    /// spread statistic.
    pub fn note_arrival(&mut self, connection: ConnectionId, sent: Duration, received: Duration) {
        if let Some(e) = self.entry_mut(connection) {
            e.connection.note_arrival(sent, received);
        }
    }

    fn entry(&self, connection: ConnectionId) -> Option<&Entry> {
        self.entries.get(self.ids.get(&connection.0)?)
    }

    fn entry_mut(&mut self, connection: ConnectionId) -> Option<&mut Entry> {
        let address = *self.ids.get(&connection.0)?;
        self.entries.get_mut(&address)
    }

    /// The next event, oldest first.
    pub fn poll_event(&mut self) -> Option<ServerEvent> {
        self.events.pop_front()
    }

    /// The next datagram to send, oldest first.
    pub fn poll_transmit(&mut self) -> Option<Transmit> {
        self.out.pop_front()
    }

    /// The bytes a packet to `connection` sent at `now` would give its
    /// Messages section, section header included, to carry every reliable
    /// message due then (never sent, or due to be sent again), before the
    /// message budget caps it; 0 when none is due or there is no such
    /// connection. A caller that shares a packet between its own sections and
    /// the messages keeps this much room, capped at the budget, instead of
    /// always reserving the budget.
    pub fn messages_due_bytes(&self, connection: ConnectionId, now: Duration) -> usize {
        self.entry(connection)
            .map_or(0, |entry| entry.connection.messages_due_bytes(now))
    }

    /// Queues a reliable message.
    pub fn send_message(
        &mut self,
        connection: ConnectionId,
        kind: u8,
        body: &[u8],
    ) -> Result<(), SendError> {
        let entry = self
            .entry_mut(connection)
            .ok_or(SendError::UnknownConnection)?;
        entry.connection.send_message(kind, body)
    }

    /// Sends a Payload with the caller's sections (kinds 2 and up) and
    /// whatever reliable messages are due and fit. Returns its sequence, which
    /// a later [`Event::Delivered`] or [`Event::Lost`] names.
    pub fn send_payload(
        &mut self,
        now: Duration,
        connection: ConnectionId,
        sections: &[(u8, &[u8])],
    ) -> Result<u16, SendError> {
        let address = *self
            .ids
            .get(&connection.0)
            .ok_or(SendError::UnknownConnection)?;
        let entry = self
            .entries
            .get_mut(&address)
            .ok_or(SendError::UnknownConnection)?;
        let result = entry.connection.send_payload(now, sections, &mut self.out);
        self.collect(address);
        result
    }

    /// Ends a connection from the host's side.
    pub fn disconnect(&mut self, connection: ConnectionId, reason: DisconnectReason) {
        let Some(address) = self.ids.get(&connection.0).copied() else {
            return;
        };
        if let Some(entry) = self.entries.get_mut(&address) {
            entry.connection.close(reason, &mut self.out);
        }
        self.collect(address);
    }

    /// Ends every connection, for example with
    /// [`DisconnectReason::ServerStopping`].
    pub fn disconnect_all(&mut self, reason: DisconnectReason) {
        let ids: Vec<ConnectionId> = self.connections().collect();
        for id in ids {
            self.disconnect(id, reason);
        }
    }

    /// Timeouts, keepalives and due messages for every connection.
    pub fn update(&mut self, now: Duration) {
        let addresses: Vec<SocketAddr> = self.entries.keys().copied().collect();
        for address in addresses {
            if let Some(entry) = self.entries.get_mut(&address) {
                entry.connection.update(now, &mut self.out);
            }
            self.collect(address);
        }
    }

    /// Takes one datagram from `from`.
    pub fn receive<G: Gate + ?Sized>(
        &mut self,
        now: Duration,
        from: SocketAddr,
        datagram: &[u8],
        gate: &mut G,
    ) {
        let (kind, body) = match packet::open(datagram, self.config.protocol_version) {
            Ok(opened) => opened,
            Err(_) => {
                self.counters.invalid += 1;
                return;
            }
        };
        match kind {
            PacketKind::ConnectRequest => self.on_request(now, from, datagram.len(), body),
            PacketKind::ChallengeAnswer => self.on_answer(now, from, datagram.len(), body, gate),
            PacketKind::Payload => self.on_payload(now, from, datagram.len(), body, gate),
            PacketKind::Disconnect => self.on_disconnect(from, body),
            PacketKind::Discover => self.on_discover(now, from, datagram.len(), body),
            PacketKind::Keepalive => self.on_keepalive(now, from, body),
            PacketKind::Reach => self.on_reach(now, from, body),
            PacketKind::Challenge
            | PacketKind::Accepted
            | PacketKind::Refuse
            | PacketKind::DiscoverAnswer
            | PacketKind::Punch
            | PacketKind::ReachAnswer => {
                self.counters.unexpected += 1;
            }
        }
    }

    /// A Reach (stage K): answered as the host of the session it names,
    /// never longer than itself. One for another session, or to a host that
    /// answers none, is counted as unexpected; one past 10 a second from its
    /// address as rate limited.
    fn on_reach(&mut self, now: Duration, from: SocketAddr, body: &[u8]) {
        let Ok(reach) = packet::decode_reach(body) else {
            self.counters.malformed += 1;
            return;
        };
        if self.reach_session != Some(reach.session_id) {
            self.counters.unexpected += 1;
            return;
        }
        if !self.reach_limiter.allow(now, from) {
            self.counters.rate_limited += 1;
            return;
        }
        self.send(
            from,
            &Packet::ReachAnswer(ReachAnswer {
                nonce: reach.nonce,
                session_id: reach.session_id,
                role: ReachRole::Hosting,
            }),
        );
    }

    fn on_discover(&mut self, now: Duration, from: SocketAddr, len: usize, body: &[u8]) {
        let Ok(query) = packet::decode_discover(len, body, self.config.protocol_version) else {
            self.counters.malformed += 1;
            return;
        };
        if !self.discover_limiter.allow(now, from.ip()) {
            self.counters.rate_limited += 1;
            return;
        }
        self.events.push_back(ServerEvent::Discover { from, query });
    }

    /// Queues the answer to a [`ServerEvent::Discover`] for `to`: the answer
    /// is fitted to the query's length ([`packet::DISCOVER_LEN`]), so it is
    /// never longer than the query, whatever the game holds.
    pub fn answer_discover(&mut self, to: SocketAddr, answer: DiscoverAnswer) {
        let answer = answer.fit(packet::DISCOVER_LEN);
        match Packet::DiscoverAnswer(answer)
            .encode_within(self.config.protocol_version, packet::DISCOVER_LEN)
        {
            Ok(datagram) => self.out.push_back(Transmit { to, datagram }),
            Err(_) => self.counters.unexpected += 1,
        }
    }

    fn send(&mut self, to: SocketAddr, packet: &Packet) {
        match packet.encode(self.config.protocol_version) {
            Ok(datagram) => self.out.push_back(Transmit { to, datagram }),
            Err(_) => self.counters.unexpected += 1,
        }
    }

    fn refuse(&mut self, to: SocketAddr, nonce: u64, reason: RefuseReason, text: &str) {
        let refuse = Packet::Refuse(Refuse {
            nonce,
            reason: reason.code(),
            text: truncate(text, MAX_REFUSE_TEXT).to_owned(),
        });
        self.send(to, &refuse);
    }

    fn slot(now: Duration) -> u64 {
        now.as_secs() / COOKIE_SLOT.as_secs()
    }

    fn on_request(&mut self, now: Duration, from: SocketAddr, len: usize, body: &[u8]) {
        let version = self.config.protocol_version;
        let Ok(request) = packet::decode_connect_request(len, body, version) else {
            self.counters.malformed += 1;
            return;
        };
        if !self.limiter.allow(now, from.ip()) {
            self.counters.rate_limited += 1;
            return;
        }
        if request.protocol_version != version {
            let text = format!(
                "This server uses network protocol version {version}, and your game uses version {}. Both must be the same.",
                request.protocol_version
            );
            self.refuse(from, request.nonce, RefuseReason::ProtocolVersion, &text);
            return;
        }
        let cookie = self.cookie_key.cookie(from, request.nonce, Self::slot(now));
        self.send(
            from,
            &Packet::Challenge(Challenge {
                nonce: request.nonce,
                cookie,
            }),
        );
    }

    fn on_answer<G: Gate + ?Sized>(
        &mut self,
        now: Duration,
        from: SocketAddr,
        len: usize,
        body: &[u8],
        gate: &mut G,
    ) {
        let Ok(answer) = packet::decode_challenge_answer(len, body) else {
            self.counters.malformed += 1;
            return;
        };
        if !self.limiter.allow(now, from.ip()) {
            self.counters.rate_limited += 1;
            return;
        }
        let slot = Self::slot(now);
        let good = [slot, slot.wrapping_sub(1)]
            .iter()
            .any(|&s| self.cookie_key.cookie(from, answer.nonce, s) == answer.cookie);
        if !good {
            self.counters.bad_cookie += 1;
            return;
        }
        let replacing = match self.entries.get(&from) {
            Some(entry) if entry.nonce == answer.nonce => {
                let datagram = entry.accepted.clone();
                self.out.push_back(Transmit { to: from, datagram });
                return;
            }
            Some(_) => true,
            None => false,
        };
        let others = self.entries.len() - usize::from(replacing);
        if others >= self.config.max_connections {
            let text = format!(
                "The server is full ({} players).",
                self.config.max_connections
            );
            self.refuse(from, answer.nonce, RefuseReason::ServerFull, &text);
            return;
        }
        let details = ConnectDetails {
            address: from,
            protocol_version: self.config.protocol_version,
            game_version: answer.game_version,
            game_commit: answer.game_commit,
            callsign: answer.callsign,
            password: answer.password,
            platform: answer.platform,
            path: if is_relayed(from) {
                Path::Relay
            } else {
                answer.path
            },
            token: answer.token,
        };
        let info = match gate.accept(&details) {
            Decision::Accept(info) => info,
            Decision::Refuse { reason, text } => {
                self.refuse(from, answer.nonce, reason, &text);
                return;
            }
        };
        if replacing && let Some(old) = self.entries.remove(&from) {
            self.ids.remove(&old.connection.id);
            self.events.push_back(ServerEvent::Closed {
                connection: ConnectionId(old.connection.id),
                address: from,
                reason: CloseReason::Replaced,
            });
        }
        let id = self.new_connection_id();
        let accepted = Packet::Accepted(Accepted {
            nonce: answer.nonce,
            connection: id,
            session_id: info.session_id,
            ticks_per_second: info.ticks_per_second,
            ticks_per_snapshot: info.ticks_per_snapshot,
            host_tick: info.host_tick,
        });
        let Ok(accepted) = accepted.encode(self.config.protocol_version) else {
            return;
        };
        self.out.push_back(Transmit {
            to: from,
            datagram: accepted.clone(),
        });
        let connection = Connection::new(
            id,
            from,
            self.config.protocol_version,
            self.config.max_section_kind,
            now,
            None,
        );
        self.entries.insert(
            from,
            Entry {
                connection,
                nonce: answer.nonce,
                accepted,
            },
        );
        self.ids.insert(id, from);
        self.events.push_back(ServerEvent::Connected {
            connection: ConnectionId(id),
            details,
        });
    }

    fn new_connection_id(&mut self) -> u32 {
        loop {
            let id = self.rng.next_u64() as u32;
            if id != 0 && !self.ids.contains_key(&id) {
                return id;
            }
        }
    }

    fn on_payload<G: Gate + ?Sized>(
        &mut self,
        now: Duration,
        from: SocketAddr,
        len: usize,
        body: &[u8],
        gate: &mut G,
    ) {
        let Ok((header, rest)) = packet::decode_payload_header(body) else {
            self.counters.malformed += 1;
            return;
        };
        let Some(entry) = self.entries.get_mut(&from) else {
            self.counters.unknown_address += 1;
            return;
        };
        if entry.connection.id != header.connection {
            self.counters.stale += 1;
            return;
        }
        let id = ConnectionId(header.connection);
        let sections = packet::decode_sections(rest);
        let mut check = |kind: u8, body: &[u8]| gate.check_section(id, kind, body);
        entry
            .connection
            .receive(now, header, sections, len, &mut check, &mut self.out);
        self.collect(from);
    }

    fn on_disconnect(&mut self, from: SocketAddr, body: &[u8]) {
        let Ok(disconnect) = packet::decode_disconnect(body) else {
            self.counters.malformed += 1;
            return;
        };
        let Some(entry) = self.entries.get_mut(&from) else {
            self.counters.unknown_address += 1;
            return;
        };
        if entry.connection.id != disconnect.connection {
            self.counters.stale += 1;
            return;
        }
        entry
            .connection
            .peer_closed(DisconnectReason::from_code(disconnect.reason));
        self.collect(from);
    }

    /// A joined game's keepalive thread speaking while the game is stalled
    /// (slice EF-K). It counts only from the connection's own address with
    /// its own id, exactly as a Payload must, and only as hearing from the
    /// connection: nothing is acknowledged, applied or answered.
    fn on_keepalive(&mut self, now: Duration, from: SocketAddr, body: &[u8]) {
        let Ok(keepalive) = packet::decode_keepalive(body) else {
            self.counters.malformed += 1;
            return;
        };
        let Some(entry) = self.entries.get_mut(&from) else {
            self.counters.unknown_address += 1;
            return;
        };
        if entry.connection.id != keepalive.connection {
            self.counters.stale += 1;
            return;
        }
        entry.connection.kept_alive(now);
        self.collect(from);
    }

    /// Moves a connection's events out and removes it once closed.
    fn collect(&mut self, address: SocketAddr) {
        let Some(entry) = self.entries.get_mut(&address) else {
            return;
        };
        let connection = ConnectionId(entry.connection.id);
        while let Some(event) = entry.connection.events.pop_front() {
            self.events
                .push_back(ServerEvent::Connection { connection, event });
        }
        if let Some(reason) = entry.connection.closed.clone() {
            self.entries.remove(&address);
            self.ids.remove(&connection.0);
            self.events.push_back(ServerEvent::Closed {
                connection,
                address,
                reason,
            });
        }
    }

    /// Reads every datagram waiting on `socket` (at most 1,024 per call) and
    /// takes each.
    pub fn receive_from<D: Datagrams + ?Sized, G: Gate + ?Sized>(
        &mut self,
        socket: &mut D,
        now: Duration,
        gate: &mut G,
    ) -> io::Result<usize> {
        let mut buf = [0u8; MAX_DATAGRAM + 1];
        let mut count = 0;
        while count < crate::MAX_RECEIVE_BATCH {
            let Some((len, from)) = socket.recv_datagram(&mut buf)? else {
                break;
            };
            self.receive(now, from, &buf[..len], gate);
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

/// `text` cut to at most `max` bytes at a character boundary.
fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{Discover, DiscoverPhase};

    const V: u16 = 3;

    fn host() -> Server {
        Server::new(ServerConfig {
            entropy: Entropy::Seeded(1),
            ..ServerConfig::new(V)
        })
    }

    fn accept_all(_: &ConnectDetails) -> Decision {
        Decision::Refuse {
            reason: RefuseReason::ShuttingDown,
            text: String::new(),
        }
    }

    fn query(version: u16, nonce: u64) -> Vec<u8> {
        Packet::Discover(Discover {
            protocol_version: version,
            nonce,
        })
        .encode(version)
        .unwrap()
    }

    fn answer(nonce: u64, callsigns: usize) -> DiscoverAnswer {
        DiscoverAnswer {
            nonce,
            protocol_version: V,
            game_version: "0.1.3".into(),
            game_commit: "abc".into(),
            session_id: 7,
            name: "Game".into(),
            summary: "UKR".into(),
            players: callsigns as u8,
            capacity: 30,
            password: false,
            full: false,
            phase: DiscoverPhase::Lobby,
            king: String::new(),
            callsigns: (0..callsigns).map(|i| format!("Callsign_{i:06}")).collect(),
            truncated: false,
        }
    }

    #[test]
    fn a_query_is_handed_up_and_its_answer_is_never_longer() {
        let mut server = host();
        let asker: SocketAddr = "10.0.0.9:40000".parse().unwrap();
        let now = Duration::from_secs(5);
        server.receive(now, asker, &query(V, 42), &mut accept_all);
        let Some(ServerEvent::Discover { from, query }) = server.poll_event() else {
            panic!("no query event")
        };
        assert_eq!((from, query.nonce, query.protocol_version), (asker, 42, V));
        // The largest lobby: every callsign at its longest.
        server.answer_discover(from, answer(42, 30));
        let sent = server.poll_transmit().expect("an answer");
        assert_eq!(sent.to, asker);
        assert!(sent.datagram.len() <= packet::DISCOVER_LEN);
        let Ok(Packet::DiscoverAnswer(back)) = Packet::decode(&sent.datagram, V) else {
            panic!("not an answer")
        };
        assert_eq!((back.nonce, back.callsigns.len()), (42, 30));
        // An answer from a game of any size is cut to fit rather than sent long.
        let mut huge = answer(42, 0);
        huge.name = "n".repeat(1000);
        huge.callsigns = vec!["x".repeat(15); 255];
        server.answer_discover(from, huge);
        let sent = server.poll_transmit().expect("an answer");
        assert!(sent.datagram.len() <= packet::DISCOVER_LEN);
        let Ok(Packet::DiscoverAnswer(back)) = Packet::decode(&sent.datagram, V) else {
            panic!("not an answer")
        };
        assert!(back.truncated && back.callsigns.len() < 255);
    }

    #[test]
    fn a_query_of_another_version_is_still_handed_up() {
        let mut server = host();
        let asker: SocketAddr = "10.0.0.9:40000".parse().unwrap();
        // A later build's query: its own checksum covers the hello id, so it
        // passes, and its padding may carry fields this build does not know.
        let mut later = query(V + 1, 9);
        later[600] = 0x55;
        let crc = packet::checksum(PacketKind::Discover, V, &later[4..]);
        later[..4].copy_from_slice(&crc.to_le_bytes());
        server.receive(Duration::ZERO, asker, &later, &mut accept_all);
        assert!(matches!(
            server.poll_event(),
            Some(ServerEvent::Discover { query, .. }) if query.protocol_version == V + 1
        ));
    }

    #[test]
    fn a_host_that_does_not_know_a_kind_drops_it_silently() {
        // What a host of an earlier build does with a discover query: its
        // kind byte is one it has no name for. The same path drops any kind
        // a later build adds.
        let mut server = host();
        let asker: SocketAddr = "10.0.0.9:40000".parse().unwrap();
        let mut unknown = query(V, 1);
        unknown[4] = 11;
        let crc = packet::checksum(PacketKind::Discover, V, &unknown[4..]);
        unknown[..4].copy_from_slice(&crc.to_le_bytes());
        server.receive(Duration::ZERO, asker, &unknown, &mut accept_all);
        assert!(server.poll_event().is_none() && server.poll_transmit().is_none());
        assert_eq!(server.counters().invalid, 1);
        // A host that ignores the event (an old caller) sends nothing.
        server.receive(Duration::ZERO, asker, &query(V, 1), &mut accept_all);
        let _ = server.poll_event();
        assert!(server.poll_transmit().is_none());
    }

    #[test]
    fn queries_are_limited_per_address_apart_from_joins() {
        let mut server = host();
        let asker: SocketAddr = "10.0.0.9:40000".parse().unwrap();
        let now = Duration::from_millis(2500);
        for i in 0..30 {
            server.receive(now, asker, &query(V, i), &mut accept_all);
        }
        let mut events = 0;
        while server.poll_event().is_some() {
            events += 1;
        }
        assert_eq!(events, RATE_LIMIT_PER_ADDRESS);
        assert_eq!(server.counters().rate_limited, 30 - u64::from(events));
        // The joins' allowance is untouched by the queries.
        let request = Packet::ConnectRequest(packet::ConnectRequest {
            protocol_version: V,
            nonce: 5,
            game_version: "0.1.3".into(),
            game_commit: "abc".into(),
        })
        .encode(V)
        .unwrap();
        server.receive(now, asker, &request, &mut accept_all);
        assert!(matches!(
            server
                .poll_transmit()
                .map(|t| Packet::decode(&t.datagram, V)),
            Some(Ok(Packet::Challenge(_)))
        ));
        // A new second allows more.
        server.receive(
            now + Duration::from_secs(1),
            asker,
            &query(V, 1),
            &mut accept_all,
        );
        assert!(server.poll_event().is_some());
    }

    #[test]
    fn a_query_of_the_wrong_size_is_malformed() {
        let mut server = host();
        let asker: SocketAddr = "10.0.0.9:40000".parse().unwrap();
        let mut short = query(V, 1);
        short.truncate(999);
        server.receive(Duration::ZERO, asker, &short, &mut accept_all);
        assert!(server.poll_event().is_none());
        assert_eq!(server.counters().invalid + server.counters().malformed, 1);
    }

    #[test]
    fn the_path_is_the_answers_and_a_relayed_address_is_always_the_relay() {
        let accept = |_: &ConnectDetails| {
            Decision::Accept(AcceptInfo {
                session_id: 1,
                ticks_per_second: 120,
                ticks_per_snapshot: 4,
                host_tick: 0,
            })
        };
        for (from, said, kept) in [
            ("203.0.113.9:40000", Path::Punched, Path::Punched),
            ("203.0.113.9:40001", Path::Ipv6, Path::Ipv6),
            ("[100::1:0:7]:0", Path::LocalNetwork, Path::Relay),
        ] {
            let mut server = host();
            let from: SocketAddr = from.parse().unwrap();
            let now = Duration::from_secs(3);
            let request = Packet::ConnectRequest(packet::ConnectRequest {
                protocol_version: V,
                nonce: 5,
                game_version: String::new(),
                game_commit: String::new(),
            })
            .encode(V)
            .unwrap();
            server.receive(now, from, &request, &mut accept.clone());
            let Ok(Packet::Challenge(challenge)) =
                Packet::decode(&server.poll_transmit().unwrap().datagram, V)
            else {
                panic!("no challenge")
            };
            let answer = Packet::ChallengeAnswer(packet::ChallengeAnswer {
                nonce: 5,
                cookie: challenge.cookie,
                callsign: "Viper".into(),
                password: String::new(),
                game_version: String::new(),
                game_commit: String::new(),
                platform: Platform::Linux,
                path: said,
                token: None,
            })
            .encode(V)
            .unwrap();
            server.receive(now, from, &answer, &mut accept.clone());
            let Some(ServerEvent::Connected { details, .. }) = server.poll_event() else {
                panic!("not connected")
            };
            assert_eq!(details.path, kept);
        }
    }

    fn reach(session_id: u64, nonce: u64) -> Vec<u8> {
        Packet::Reach(packet::Reach {
            session_id,
            nonce,
            from: 3,
        })
        .encode(V)
        .unwrap()
    }

    #[test]
    fn a_host_answers_a_reach_for_its_own_session_only_and_never_longer() {
        let mut server = host();
        let asker: SocketAddr = "203.0.113.9:40000".parse().unwrap();
        let now = Duration::from_secs(2);
        // A host that answers no session counts it.
        server.receive(now, asker, &reach(77, 1), &mut accept_all);
        assert!(server.poll_transmit().is_none());
        assert_eq!(server.counters().unexpected, 1);
        server.set_reach_session(Some(77));
        let sent = reach(77, 5);
        server.receive(now, asker, &sent, &mut accept_all);
        let answer = server.poll_transmit().expect("an answer");
        assert_eq!(answer.to, asker);
        assert!(answer.datagram.len() <= sent.len());
        assert_eq!(
            Packet::decode(&answer.datagram, V),
            Ok(Packet::ReachAnswer(packet::ReachAnswer {
                nonce: 5,
                session_id: 77,
                role: ReachRole::Hosting,
            }))
        );
        // Another session's Reach is counted and dropped; so is an answer.
        server.receive(now, asker, &reach(78, 6), &mut accept_all);
        let stray = Packet::ReachAnswer(packet::ReachAnswer {
            nonce: 5,
            session_id: 77,
            role: ReachRole::NotHosting,
        })
        .encode(V)
        .unwrap();
        server.receive(now, asker, &stray, &mut accept_all);
        assert!(server.poll_transmit().is_none());
        assert_eq!(server.counters().unexpected, 3);
        // No connection, no event: a Reach leaves nothing behind.
        assert!(server.poll_event().is_none());
        assert_eq!(server.connections().count(), 0);
    }

    #[test]
    fn reaches_from_one_ip_address_are_answered_up_to_its_cap() {
        let mut server = host();
        server.set_reach_session(Some(9));
        let now = Duration::from_millis(3_500);
        // 20 ports of one IP address, ten Reaches each: the IP address's 160.
        for port in 0..20u16 {
            let asker: SocketAddr = format!("203.0.113.9:{}", 40_000 + port).parse().unwrap();
            for nonce in 0..10 {
                server.receive(now, asker, &reach(9, nonce), &mut accept_all);
            }
        }
        let answers = std::iter::from_fn(|| server.poll_transmit()).count();
        assert_eq!(answers, REACH_PER_IP as usize);
        assert_eq!(
            server.counters().rate_limited,
            200 - u64::from(REACH_PER_IP)
        );
        // Another IP address is still answered, and the next second starts
        // afresh.
        let other: SocketAddr = "203.0.113.10:40000".parse().unwrap();
        server.receive(now, other, &reach(9, 1), &mut accept_all);
        server.receive(
            now + Duration::from_secs(1),
            "203.0.113.9:40000".parse().unwrap(),
            &reach(9, 2),
            &mut accept_all,
        );
        assert_eq!(std::iter::from_fn(|| server.poll_transmit()).count(), 2);
    }

    #[test]
    fn reaches_are_answered_ten_a_second_from_one_address() {
        let mut server = host();
        server.set_reach_session(Some(9));
        let asker: SocketAddr = "203.0.113.9:40000".parse().unwrap();
        let now = Duration::from_millis(3_500);
        for nonce in 0..25 {
            server.receive(now, asker, &reach(9, nonce), &mut accept_all);
        }
        let answers = std::iter::from_fn(|| server.poll_transmit()).count();
        assert_eq!(answers, REACH_PER_ADDRESS as usize);
        assert_eq!(
            server.counters().rate_limited,
            25 - u64::from(REACH_PER_ADDRESS)
        );
        // Another address has its own ten, and joins keep their allowance.
        let other: SocketAddr = "203.0.113.10:40000".parse().unwrap();
        server.receive(now, other, &reach(9, 1), &mut accept_all);
        assert!(server.poll_transmit().is_some());
        let request = Packet::ConnectRequest(packet::ConnectRequest {
            protocol_version: V,
            nonce: 5,
            game_version: String::new(),
            game_commit: String::new(),
        })
        .encode(V)
        .unwrap();
        server.receive(now, asker, &request, &mut accept_all);
        assert!(matches!(
            server
                .poll_transmit()
                .map(|t| Packet::decode(&t.datagram, V)),
            Some(Ok(Packet::Challenge(_)))
        ));
        // The next second allows more.
        server.receive(
            now + Duration::from_secs(1),
            asker,
            &reach(9, 99),
            &mut accept_all,
        );
        assert!(server.poll_transmit().is_some());
    }

    #[test]
    fn the_token_in_the_answer_reaches_the_gate() {
        let mut seen = Vec::new();
        for token in [None, Some(Token(0xABCD_u128 << 100 | 7))] {
            let mut server = host();
            let from: SocketAddr = "203.0.113.9:40000".parse().unwrap();
            let now = Duration::from_secs(3);
            let request = Packet::ConnectRequest(packet::ConnectRequest {
                protocol_version: V,
                nonce: 5,
                game_version: String::new(),
                game_commit: String::new(),
            })
            .encode(V)
            .unwrap();
            let mut gate = |details: &ConnectDetails| {
                seen.push(details.token);
                Decision::Accept(AcceptInfo {
                    session_id: 1,
                    ticks_per_second: 120,
                    ticks_per_snapshot: 4,
                    host_tick: 0,
                })
            };
            server.receive(now, from, &request, &mut gate);
            let Ok(Packet::Challenge(challenge)) =
                Packet::decode(&server.poll_transmit().unwrap().datagram, V)
            else {
                panic!("no challenge")
            };
            let answer = Packet::ChallengeAnswer(packet::ChallengeAnswer {
                nonce: 5,
                cookie: challenge.cookie,
                callsign: "Viper".into(),
                password: String::new(),
                game_version: String::new(),
                game_commit: String::new(),
                platform: Platform::Linux,
                path: Path::ByAddress,
                token,
            })
            .encode(V)
            .unwrap();
            server.receive(now, from, &answer, &mut gate);
            let Some(ServerEvent::Connected { details, .. }) = server.poll_event() else {
                panic!("not connected")
            };
            assert_eq!(details.token, token);
        }
        assert_eq!(seen, [None, Some(Token(0xABCD_u128 << 100 | 7))]);
    }

    #[test]
    fn a_host_counts_a_punch_as_unexpected() {
        let mut server = host();
        let punch = Packet::Punch(packet::Punch { introduction: 1 })
            .encode(V)
            .unwrap();
        server.receive(
            Duration::ZERO,
            "203.0.113.9:1".parse().unwrap(),
            &punch,
            &mut accept_all,
        );
        assert_eq!(server.counters().unexpected, 1);
        assert!(server.poll_transmit().is_none());
    }

    #[test]
    fn truncate_respects_characters() {
        assert_eq!(truncate("abc", 5), "abc");
        assert_eq!(truncate("abcdef", 3), "abc");
        assert_eq!(truncate("aé", 2), "a");
    }

    #[test]
    fn limiter_caps_per_address_and_in_all() {
        let mut limiter = RateLimiter::default();
        let a: IpAddr = "10.0.0.1".parse().unwrap();
        let now = Duration::from_millis(1500);
        let allowed = (0..30).filter(|_| limiter.allow(now, a)).count();
        assert_eq!(allowed, RATE_LIMIT_PER_ADDRESS as usize);
        let mut total = allowed;
        for i in 0..250u32 {
            let ip = IpAddr::from([10, 1, (i >> 8) as u8, i as u8]);
            total += usize::from(limiter.allow(now, ip));
        }
        assert_eq!(total, RATE_LIMIT_TOTAL as usize);
        assert!(limiter.allow(Duration::from_millis(2000), a));
    }
}
