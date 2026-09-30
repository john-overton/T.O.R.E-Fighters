//! One established connection: what both the client and the host keep once
//! the handshake is done.

use std::collections::VecDeque;
use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

use crate::packet::{
    self, ACK_DELAY_NONE, ACK_DELAY_UNIT_MICROS, Disconnect, MAX_DATAGRAM, Packet, PayloadHeader,
    SECTION_HEADER_LEN, SECTION_MESSAGES, Section,
};
use crate::reliable::{self, Receiver, Sender};
use crate::track::{
    AckOutcome, Arrival, BadWindow, LossWindow, RateWindow, ReceiveWindow, RoundTrip, SentLog,
    Spread,
};
use crate::{BAD_PACKET_LIMIT, KEEPALIVE_INTERVAL, MESSAGE_PACKET_INTERVAL, TIMEOUT, Transmit};

/// A connection's id: the random number the host gave in Accepted, which
/// every Payload carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionId(pub u32);

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}", self.0)
    }
}

/// Why a host refused a join. The codes are the wire's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefuseReason {
    /// 1: a different protocol version.
    ProtocolVersion,
    /// 2: a different game build.
    GameBuild,
    /// 3: the server is full.
    ServerFull,
    /// 4: the wrong password.
    WrongPassword,
    /// 5: the server is shutting down or restarting the mission.
    ShuttingDown,
    /// 6: this player was kicked.
    Kicked,
    /// A code this build does not know, from a newer host.
    Other(u8),
}

impl RefuseReason {
    /// The wire code.
    pub fn code(self) -> u8 {
        match self {
            Self::ProtocolVersion => 1,
            Self::GameBuild => 2,
            Self::ServerFull => 3,
            Self::WrongPassword => 4,
            Self::ShuttingDown => 5,
            Self::Kicked => 6,
            Self::Other(code) => code,
        }
    }

    /// The reason for a wire code.
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => Self::ProtocolVersion,
            2 => Self::GameBuild,
            3 => Self::ServerFull,
            4 => Self::WrongPassword,
            5 => Self::ShuttingDown,
            6 => Self::Kicked,
            other => Self::Other(other),
        }
    }
}

/// Why a connection ended. The codes are the wire's (agent decision: the
/// protocol names the reasons but not their numbers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisconnectReason {
    /// 1: the player left.
    Left,
    /// 2: 5 seconds without a valid packet.
    Timeout,
    /// 3: 50 bad packets within 5 seconds.
    BadPackets,
    /// 4: a message out of its window, or fragments that do not fit.
    ProtocolError,
    /// 5: the client's content differs from the host's.
    ContentMismatch,
    /// 6: the server is stopping.
    ServerStopping,
    /// 7: the host removed the player.
    Kicked,
    /// A code this build does not know.
    Other(u8),
}

impl DisconnectReason {
    /// The wire code.
    pub fn code(self) -> u8 {
        match self {
            Self::Left => 1,
            Self::Timeout => 2,
            Self::BadPackets => 3,
            Self::ProtocolError => 4,
            Self::ContentMismatch => 5,
            Self::ServerStopping => 6,
            Self::Kicked => 7,
            Self::Other(code) => code,
        }
    }

    /// The reason for a wire code.
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Left,
            2 => Self::Timeout,
            3 => Self::BadPackets,
            4 => Self::ProtocolError,
            5 => Self::ContentMismatch,
            6 => Self::ServerStopping,
            7 => Self::Kicked,
            other => Self::Other(other),
        }
    }
}

/// Why a join failed or a connection ended. Every connection that ends, and
/// every join that fails, produces exactly one event carrying one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseReason {
    /// The client heard nothing from the host for 10 seconds while joining.
    NoAnswer,
    /// The host refused the join.
    Refused {
        /// The reason code.
        reason: RefuseReason,
        /// The host's text for the player.
        text: String,
    },
    /// The connection ended.
    Disconnected {
        /// Why.
        reason: DisconnectReason,
        /// True when the other side said so in a Disconnect packet; false when
        /// this side decided (a timeout, bad packets, or the caller's call).
        by_peer: bool,
    },
    /// Host only: a new join from the same address and port replaced the
    /// connection.
    Replaced,
}

/// Something that happened on an established connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A reliable message, whole, in order.
    Message {
        /// The caller's kind byte.
        kind: u8,
        /// The body, reassembled when it was sent in fragments.
        body: Vec<u8>,
    },
    /// A Payload arrived carrying the caller's sections (kinds 2 and up), in
    /// packet order. Empty keepalives and message-only packets produce none.
    Payload {
        /// The packet's sequence.
        sequence: u16,
        /// The sections, in the order they were written.
        sections: Vec<Section>,
    },
    /// A Payload this side sent was acknowledged.
    Delivered {
        /// Its sequence.
        sequence: u16,
    },
    /// A Payload this side sent was judged lost: 33 newer ones were
    /// acknowledged without it.
    Lost {
        /// Its sequence.
        sequence: u16,
    },
}

/// Why a send failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    /// The connection is not established (joining, or ended).
    NotConnected,
    /// No connection has that id.
    UnknownConnection,
    /// A message over 64 KB.
    MessageTooLarge,
    /// 8,192 messages already queued.
    QueueFull,
    /// The sections do not fit one 1,200-byte packet.
    PacketTooLarge,
    /// A section kind the caller may not send: 0, 1 (the crate's own) or
    /// beyond the configured highest kind.
    BadSectionKind(u8),
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConnected => f.write_str("not connected"),
            Self::UnknownConnection => f.write_str("no such connection"),
            Self::MessageTooLarge => f.write_str("message over 64 KB"),
            Self::QueueFull => f.write_str("message queue full"),
            Self::PacketTooLarge => f.write_str("sections exceed one packet"),
            Self::BadSectionKind(kind) => write!(f, "section kind {kind} not allowed"),
        }
    }
}

impl std::error::Error for SendError {}

/// A connection's statistics, as of the last time the endpoint was given the
/// time.
#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    /// The smoothed round trip (gain 1/8).
    pub round_trip: Duration,
    /// Its smoothed mean deviation (gain 1/4).
    pub round_trip_deviation: Duration,
    /// False until the first sample: the round trip is then an assumed 250 ms.
    pub round_trip_measured: bool,
    /// Lost over lost plus delivered for this side's packets, over the last
    /// 5 seconds; `None` before any was judged.
    pub loss: Option<f64>,
    /// Arrival spread of the times the caller notes (RFC 3550 style).
    pub spread: Duration,
    /// Datagram bytes sent in the last second (UDP payload, no IP headers).
    pub bytes_sent_per_second: u64,
    /// Datagram bytes received in the last second.
    pub bytes_received_per_second: u64,
    /// Packets sent in the last second.
    pub packets_sent_per_second: u32,
    /// Valid packets received in the last second.
    pub packets_received_per_second: u32,
    /// Bad packets since the connection started.
    pub bad_packets: u64,
    /// Bad packets in the last 5 seconds (50 end the connection).
    pub bad_packets_recent: u32,
    /// Reliable messages queued and unacknowledged, fragments counted each.
    pub messages_queued: usize,
    /// Time since the last valid packet.
    pub since_last_received: Duration,
}

/// The state of an established connection.
#[derive(Debug)]
pub(crate) struct Connection {
    pub id: u32,
    pub peer: SocketAddr,
    version: u16,
    max_section_kind: u8,
    recv: ReceiveWindow,
    sent: SentLog,
    round_trip: RoundTrip,
    loss: LossWindow,
    sent_rate: RateWindow,
    recv_rate: RateWindow,
    spread: Spread,
    bad: BadWindow,
    bad_total: u64,
    sender: Sender,
    receiver: Receiver,
    last_received: Duration,
    last_sent: Option<Duration>,
    now: Duration,
    message_budget: usize,
    pub events: VecDeque<Event>,
    pub closed: Option<CloseReason>,
}

/// What the receiving code decided about a Payload.
enum Verdict {
    Apply,
    Drop,
    Bad,
    ProtocolError,
}

impl Connection {
    pub(crate) fn new(
        id: u32,
        peer: SocketAddr,
        version: u16,
        max_section_kind: u8,
        now: Duration,
        round_trip: Option<Duration>,
    ) -> Self {
        Self {
            id,
            peer,
            version,
            max_section_kind,
            recv: ReceiveWindow::default(),
            sent: SentLog::default(),
            round_trip: RoundTrip::new(round_trip),
            loss: LossWindow::default(),
            sent_rate: RateWindow::default(),
            recv_rate: RateWindow::default(),
            spread: Spread::default(),
            bad: BadWindow::default(),
            bad_total: 0,
            sender: Sender::default(),
            receiver: Receiver::default(),
            last_received: now,
            last_sent: None,
            now,
            message_budget: MAX_DATAGRAM,
            events: VecDeque::new(),
            closed: None,
        }
    }

    pub(crate) fn set_message_budget(&mut self, bytes: usize) {
        self.message_budget = bytes;
    }

    pub(crate) fn note_arrival(&mut self, sent: Duration, received: Duration) {
        self.spread.note(sent, received);
    }

    pub(crate) fn stats(&self) -> Stats {
        let now = self.now;
        let (bytes_sent, packets_sent) = self.sent_rate.per_second(now);
        let (bytes_received, packets_received) = self.recv_rate.per_second(now);
        Stats {
            round_trip: self.round_trip.mean(),
            round_trip_deviation: self.round_trip.deviation(),
            round_trip_measured: self.round_trip.measured(),
            loss: self.loss.ratio(now),
            spread: self.spread.value(),
            bytes_sent_per_second: bytes_sent,
            bytes_received_per_second: bytes_received,
            packets_sent_per_second: packets_sent,
            packets_received_per_second: packets_received,
            bad_packets: self.bad_total,
            bad_packets_recent: self.bad.recent(now) as u32,
            messages_queued: self.sender.queued(),
            since_last_received: now.saturating_sub(self.last_received),
        }
    }

    pub(crate) fn send_message(&mut self, kind: u8, body: &[u8]) -> Result<(), SendError> {
        if self.closed.is_some() {
            return Err(SendError::NotConnected);
        }
        self.sender.push(kind, body)
    }

    /// Sends a Payload with the caller's sections and whatever messages are
    /// due and fit. Returns its sequence.
    pub(crate) fn send_payload(
        &mut self,
        now: Duration,
        sections: &[(u8, &[u8])],
        out: &mut VecDeque<Transmit>,
    ) -> Result<u16, SendError> {
        if self.closed.is_some() {
            return Err(SendError::NotConnected);
        }
        self.now = now;
        let mut size = packet::PAYLOAD_HEADER_LEN;
        for (kind, body) in sections {
            if *kind <= SECTION_MESSAGES || *kind > self.max_section_kind {
                return Err(SendError::BadSectionKind(*kind));
            }
            size += SECTION_HEADER_LEN + body.len();
        }
        if size > MAX_DATAGRAM {
            return Err(SendError::PacketTooLarge);
        }
        let interval = reliable::resend_interval(self.round_trip.mean());
        let messages = self
            .sender
            .select(now, interval, MAX_DATAGRAM - size, self.message_budget);
        let (ack, ack_bits, ack_delay) = self.recv.ack_fields(now);
        let header = PayloadHeader {
            connection: self.id,
            sequence: self.sent.next_sequence(),
            ack,
            ack_bits,
            ack_delay,
        };
        let mut all: Vec<(u8, &[u8])> = sections.to_vec();
        let (message_body, message_ids) = match &messages {
            Some((body, ids)) => (body.as_slice(), ids.clone()),
            None => (&[][..], Vec::new()),
        };
        if messages.is_some() {
            all.push((SECTION_MESSAGES, message_body));
        }
        let datagram = packet::write_payload(self.version, &header, &all)
            .map_err(|_| SendError::PacketTooLarge)?;
        let sequence = header.sequence;
        if let Some(evicted) = self.sent.record(now, message_ids) {
            self.loss.push(now, true);
            self.events.push_back(Event::Lost { sequence: evicted });
        }
        self.last_sent = Some(now);
        self.sent_rate.push(now, datagram.len());
        out.push_back(Transmit {
            to: self.peer,
            datagram,
        });
        Ok(sequence)
    }

    /// Takes a Payload whose checksum and connection id are good.
    pub(crate) fn receive(
        &mut self,
        now: Duration,
        header: PayloadHeader,
        sections: Result<Vec<Section>, packet::PacketError>,
        datagram_len: usize,
        check: &mut dyn FnMut(u8, &[u8]) -> bool,
        out: &mut VecDeque<Transmit>,
    ) {
        if self.closed.is_some() {
            return;
        }
        self.now = now;
        let Ok(sections) = sections else {
            self.bad_packet(now, out);
            return;
        };
        let (verdict, records) = self.judge(&header, &sections, check);
        match verdict {
            Verdict::Drop => {
                self.last_received = now;
                return;
            }
            Verdict::Bad => {
                self.bad_packet(now, out);
                return;
            }
            Verdict::ProtocolError => {
                self.close(DisconnectReason::ProtocolError, out);
                return;
            }
            Verdict::Apply => {}
        }
        self.recv.record(header.sequence, now);
        self.last_received = now;
        self.recv_rate.push(now, datagram_len);
        if header.ack_delay != ACK_DELAY_NONE {
            let delay = Duration::from_micros(u64::from(header.ack_delay) * ACK_DELAY_UNIT_MICROS);
            let outcome = self.sent.apply(now, header.ack, header.ack_bits, delay);
            self.take_ack(now, outcome);
        }
        let mut ready = Vec::new();
        for record in records {
            if self.receiver.accept(record, &mut ready).is_err() {
                self.close(DisconnectReason::ProtocolError, out);
                return;
            }
        }
        for (kind, body) in ready {
            self.events.push_back(Event::Message { kind, body });
        }
        let caller: Vec<Section> = sections
            .into_iter()
            .filter(|s| s.kind != SECTION_MESSAGES)
            .collect();
        if !caller.is_empty() {
            self.events.push_back(Event::Payload {
                sequence: header.sequence,
                sections: caller,
            });
        }
    }

    /// Decides a Payload's fate without changing any state.
    fn judge(
        &self,
        header: &PayloadHeader,
        sections: &[Section],
        check: &mut dyn FnMut(u8, &[u8]) -> bool,
    ) -> (Verdict, Vec<reliable::Record>) {
        let mut records = None;
        for section in sections {
            if section.kind > self.max_section_kind {
                return (Verdict::Bad, Vec::new());
            }
            if section.kind == SECTION_MESSAGES {
                if records.is_some() {
                    return (Verdict::Bad, Vec::new());
                }
                match reliable::decode(&section.body) {
                    Some(decoded) => records = Some(decoded),
                    None => return (Verdict::Bad, Vec::new()),
                }
            }
        }
        if header.ack_delay != ACK_DELAY_NONE && !self.sent.ack_is_sane(header.ack) {
            return (Verdict::Bad, Vec::new());
        }
        if self.recv.arrival(header.sequence) != Arrival::New {
            return (Verdict::Drop, Vec::new());
        }
        let records = records.unwrap_or_default();
        if self.receiver.check(&records).is_err() {
            return (Verdict::ProtocolError, Vec::new());
        }
        for section in sections {
            if section.kind != SECTION_MESSAGES && !check(section.kind, &section.body) {
                return (Verdict::Bad, Vec::new());
            }
        }
        (Verdict::Apply, records)
    }

    fn take_ack(&mut self, now: Duration, outcome: AckOutcome) {
        if let Some(sample) = outcome.sample {
            self.round_trip.sample(sample);
        }
        for (sequence, ids) in outcome.delivered {
            for id in ids {
                self.sender.ack(id);
            }
            self.loss.push(now, false);
            self.events.push_back(Event::Delivered { sequence });
        }
        for sequence in outcome.lost {
            self.loss.push(now, true);
            self.events.push_back(Event::Lost { sequence });
        }
    }

    /// Counts a bad packet; 50 within 5 seconds end the connection.
    pub(crate) fn bad_packet(&mut self, now: Duration, out: &mut VecDeque<Transmit>) {
        self.now = now;
        self.bad_total += 1;
        if self.bad.push(now) >= BAD_PACKET_LIMIT {
            self.close(DisconnectReason::BadPackets, out);
        }
    }

    /// Timeouts, keepalives and due messages. Due messages go out in a
    /// packet of their own at most every 8.3 ms; the caller's packets carry
    /// them too.
    pub(crate) fn update(&mut self, now: Duration, out: &mut VecDeque<Transmit>) {
        if self.closed.is_some() {
            return;
        }
        self.now = now;
        if now.saturating_sub(self.last_received) >= TIMEOUT {
            self.close(DisconnectReason::Timeout, out);
            return;
        }
        let since = self.last_sent.map(|sent| now.saturating_sub(sent));
        let keepalive = since.is_none_or(|since| since >= KEEPALIVE_INTERVAL);
        let paced = since.is_none_or(|since| since >= MESSAGE_PACKET_INTERVAL);
        let interval = reliable::resend_interval(self.round_trip.mean());
        if keepalive || (paced && self.sender.has_due(now, interval)) {
            // An empty section list always fits.
            let _ = self.send_payload(now, &[], out);
        }
    }

    /// Ends the connection from this side: Disconnect sent three times.
    pub(crate) fn close(&mut self, reason: DisconnectReason, out: &mut VecDeque<Transmit>) {
        if self.closed.is_some() {
            return;
        }
        let packet = Packet::Disconnect(Disconnect {
            connection: self.id,
            reason: reason.code(),
        });
        if let Ok(datagram) = packet.encode(self.version) {
            for _ in 0..3 {
                out.push_back(Transmit {
                    to: self.peer,
                    datagram: datagram.clone(),
                });
            }
        }
        self.closed = Some(CloseReason::Disconnected {
            reason,
            by_peer: false,
        });
    }

    /// The other side sent Disconnect.
    pub(crate) fn peer_closed(&mut self, reason: DisconnectReason) {
        if self.closed.is_none() {
            self.closed = Some(CloseReason::Disconnected {
                reason,
                by_peer: true,
            });
        }
    }
}
