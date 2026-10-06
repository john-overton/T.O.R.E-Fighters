//! Packets: the header, the checksum and the thirteen kinds.
//!
//! Every packet starts with a CRC-32 checksum (4 bytes, least significant
//! byte first) and a kind byte. The checksum covers a 10-byte protocol id that
//! is never sent, followed by every byte after the checksum. Connect request,
//! Refuse and the two discovery kinds use the fixed id `TORE-HELLO`, so a host
//! of any version can read and answer them; the other kinds use `TORE-NET`
//! and the protocol version (16 bits), so a packet from another program or
//! version fails its checksum.
//! See "Packets" and "Connecting" in
//! [`docs/formats/net-protocol.md`](../../../docs/formats/net-protocol.md).
//!
//! Every decoder is bounded and returns an error for any bytes; none panics.
//! Decoding is strict: a field outside its limits, trailing bytes, or padding
//! that is not zero rejects the packet.

use std::fmt;

use tore_codec::{BitReader, BitWriter, CodecError, Crc32};

use crate::master::Path;
use crate::platform::Platform;

/// The largest datagram either side sends or accepts.
pub const MAX_DATAGRAM: usize = 1200;
/// The exact size of a Connect request and a Challenge answer.
pub const PADDED_LEN: usize = 1000;
/// Checksum and kind.
pub const HEADER_LEN: usize = 5;
/// A Payload's header with the checksum and kind: 19 bytes.
pub const PAYLOAD_HEADER_LEN: usize = HEADER_LEN + 4 + 2 + 2 + 4 + 2;
/// A section's kind and length.
pub const SECTION_HEADER_LEN: usize = 3;
/// The longest refusal text, in bytes.
pub const MAX_REFUSE_TEXT: usize = 200;
/// The longest callsign, in characters (printable ASCII, so also bytes).
pub const MAX_CALLSIGN: usize = 15;
/// The ack delay value that says "nothing received yet": the ack and ack bits
/// mean nothing.
pub const ACK_DELAY_NONE: u16 = u16::MAX;
/// The largest ack delay that is a time: 65,534 units of 16 microseconds.
pub const ACK_DELAY_MAX: u16 = u16::MAX - 1;
/// One unit of the ack delay, in microseconds.
pub const ACK_DELAY_UNIT_MICROS: u64 = 16;
/// Section kind 1: reliable messages, this crate's own.
pub const SECTION_MESSAGES: u8 = 1;
/// The fixed protocol id of Connect request and Refuse.
pub const HELLO_ID: [u8; 10] = *b"TORE-HELLO";
/// The name part of the versioned protocol id.
pub const PROTOCOL_NAME: [u8; 8] = *b"TORE-NET";

/// The exact size of a Discover query: the longest answer it may be given.
pub const DISCOVER_LEN: usize = PADDED_LEN;
/// The longest game version or commit text a discover answer carries, in
/// bytes (longer is cut by [`DiscoverAnswer::fit`]).
pub const MAX_DISCOVER_BUILD: usize = 64;
/// The longest game name a discover answer carries, in bytes.
pub const MAX_DISCOVER_NAME: usize = 64;
/// The longest mission summary a discover answer carries, in bytes.
pub const MAX_DISCOVER_SUMMARY: usize = 200;

/// The thirteen packet kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PacketKind {
    /// Client to host, padded to 1,000 bytes, `TORE-HELLO` id.
    ConnectRequest = 1,
    /// Host to client: the cookie.
    Challenge = 2,
    /// Client to host: the cookie echoed, the callsign and password and the
    /// player's platform, padded.
    ChallengeAnswer = 3,
    /// Host to client: the connection id and the session's rates.
    Accepted = 4,
    /// Host to client, `TORE-HELLO` id: a reason code and text.
    Refuse = 5,
    /// Both ways once connected.
    Payload = 6,
    /// Both ways: the connection ends.
    Disconnect = 7,
    /// Anyone to a host, `TORE-HELLO` id, padded to 1,000 bytes: "who is
    /// hosting here?" (slice EF5).
    Discover = 8,
    /// Host to the asker, `TORE-HELLO` id, never longer than the query: the
    /// game's summary.
    DiscoverAnswer = 9,
    /// Client to host once connected, 9 bytes: "my game is stalled, not
    /// gone" (slice EF-K, protocol 5). Sent by the joined game's keepalive
    /// thread while its own loop is held up; the host only notes that it
    /// heard from the connection.
    Keepalive = 10,
    /// Host to a joining player, 13 bytes: the introduction id the master
    /// gave (stage J, slice J2, protocol 9). Going out, it opens the host's
    /// router for the player's address; a player that is joining with that
    /// introduction adds the sender's address to the ones it tries.
    Punch = 11,
    /// Game to game, 22 bytes (stage K, slice K0, protocol 13): does this
    /// game reach the other's joined socket, and how long is the round trip?
    /// Sent the other way first, it opens the sender's router, as a Punch
    /// does. Answered only for the answerer's own session.
    Reach = 12,
    /// Game to game, 22 bytes, never longer than the Reach it answers: the
    /// Reach's nonce, the session and whether the answerer hosts it now.
    ReachAnswer = 13,
}

impl PacketKind {
    /// The kind for a kind byte, if it is one.
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            1 => Self::ConnectRequest,
            2 => Self::Challenge,
            3 => Self::ChallengeAnswer,
            4 => Self::Accepted,
            5 => Self::Refuse,
            6 => Self::Payload,
            7 => Self::Disconnect,
            8 => Self::Discover,
            9 => Self::DiscoverAnswer,
            10 => Self::Keepalive,
            11 => Self::Punch,
            12 => Self::Reach,
            13 => Self::ReachAnswer,
            _ => return None,
        })
    }

    /// True for the kinds checked with the fixed `TORE-HELLO` id.
    pub fn uses_hello_id(self) -> bool {
        matches!(
            self,
            Self::ConnectRequest | Self::Refuse | Self::Discover | Self::DiscoverAnswer
        )
    }
}

/// The 10-byte protocol id a kind's checksum covers.
pub fn protocol_id(kind: PacketKind, version: u16) -> [u8; 10] {
    if kind.uses_hello_id() {
        return HELLO_ID;
    }
    let mut id = [0u8; 10];
    id[..8].copy_from_slice(&PROTOCOL_NAME);
    id[8..].copy_from_slice(&version.to_le_bytes());
    id
}

/// Why a datagram was not a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketError {
    /// Shorter than a header.
    TooShort,
    /// Longer than 1,200 bytes.
    TooLong,
    /// A kind byte that is not one of the thirteen.
    UnknownKind(u8),
    /// The checksum does not match: another program, another version or
    /// damage. Dropped silently.
    Checksum,
    /// The checksum matched but the fields break their rules.
    Malformed,
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => f.write_str("datagram shorter than a packet header"),
            Self::TooLong => f.write_str("datagram longer than 1,200 bytes"),
            Self::UnknownKind(kind) => write!(f, "unknown packet kind {kind}"),
            Self::Checksum => f.write_str("checksum mismatch"),
            Self::Malformed => f.write_str("malformed packet"),
        }
    }
}

impl std::error::Error for PacketError {}

impl From<CodecError> for PacketError {
    fn from(_: CodecError) -> Self {
        Self::Malformed
    }
}

/// Why a packet could not be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// The packet would pass 1,200 bytes (or 1,000 for a padded one).
    TooLarge,
    /// A string over 255 bytes, a refusal text over 200 bytes, or a callsign
    /// that is not 1 to 15 printable ASCII characters.
    BadString,
    /// A section of kind 0.
    BadSectionKind,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "packet too large",
            Self::BadString => "string too long or callsign invalid",
            Self::BadSectionKind => "section kind 0",
        })
    }
}

impl std::error::Error for EncodeError {}

/// Connect request: the first packet of a join.
///
/// The protocol version and the nonce come first and never move, so a host of
/// any version can read them and refuse with the client's nonce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectRequest {
    /// The client's protocol version.
    pub protocol_version: u16,
    /// The client's random nonce for this join.
    pub nonce: u64,
    /// The client's game version string. Empty when decoded from a request
    /// of another protocol version, whose layout past the nonce is unknown.
    pub game_version: String,
    /// The commit the client's build stamps. Empty as above.
    pub game_commit: String,
}

/// Challenge: the host's cookie. 21 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Challenge {
    /// The nonce of the request it answers.
    pub nonce: u64,
    /// The keyed hash the client must echo.
    pub cookie: u64,
}

/// Challenge answer: the cookie echoed with the player's details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeAnswer {
    /// The nonce of the join.
    pub nonce: u64,
    /// The cookie from the Challenge.
    pub cookie: u64,
    /// 1 to 15 printable ASCII characters.
    pub callsign: String,
    /// May be empty.
    pub password: String,
    /// Repeated from the request: the host keeps nothing between the two.
    pub game_version: String,
    /// Repeated from the request.
    pub game_commit: String,
    /// The operating system the player's game runs on (protocol 7).
    pub platform: Platform,
    /// How the player reached the host (protocol 9): the byte after the
    /// platform, the master's path codes. A host takes a relayed address as
    /// the relay whatever this says.
    pub path: Path,
    /// The player's rejoin token for this session, when its game holds one
    /// (stage K, protocol 13): a byte after the path, 0 none or 1 a token,
    /// then the 128-bit token. The transport reads nothing into it.
    pub token: Option<Token>,
}

/// A player's rejoin token (stage K): 128 bits drawn from the operating
/// system's randomness ([`crate::TokenSource`]), good only in the session
/// that issued it. Its `Debug` shows it whole: a log that prints one is the
/// host's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Token(pub u128);

impl Token {
    /// Writes the token's 128 bits, the low 64 first.
    pub fn write(self, w: &mut BitWriter) {
        w.write_bits(self.0 as u64, 64).ok();
        w.write_bits((self.0 >> 64) as u64, 64).ok();
    }

    /// Reads what [`Token::write`] wrote.
    pub fn read(r: &mut BitReader<'_>) -> Result<Self, CodecError> {
        let low = r.read_bits(64)?;
        let high = r.read_bits(64)?;
        Ok(Self(u128::from(low) | u128::from(high) << 64))
    }
}

/// Reach: does this game reach the other's joined socket? 22 bytes (stage
/// K, protocol 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reach {
    /// The session both games are in; another session's game drops it.
    pub session_id: u64,
    /// The sender's random nonce, which the answer repeats.
    pub nonce: u64,
    /// The sender's lobby id.
    pub from: u8,
}

/// What an answering game is in the session (stage K).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReachRole {
    /// 0: a player's game, not hosting.
    NotHosting = 0,
    /// 1: the game hosts this session now.
    Hosting = 1,
}

impl ReachRole {
    /// The role of a wire code; `None` for any other.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::NotHosting),
            1 => Some(Self::Hosting),
            _ => None,
        }
    }
}

/// Reach answer: the Reach's nonce, the session and the answerer's role. 22
/// bytes, never longer than the Reach (stage K, protocol 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReachAnswer {
    /// The nonce of the Reach it answers.
    pub nonce: u64,
    /// The answerer's session.
    pub session_id: u64,
    /// Whether the answerer hosts the session now.
    pub role: ReachRole,
}

/// Accepted: the join succeeded. 31 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accepted {
    /// The nonce of the join, so a stranger cannot forge an acceptance.
    pub nonce: u64,
    /// The connection id every Payload carries.
    pub connection: u32,
    /// The host's session id.
    pub session_id: u64,
    /// Simulation ticks per second (120).
    pub ticks_per_second: u8,
    /// Ticks between snapshots (4 by default).
    pub ticks_per_snapshot: u8,
    /// The host's tick when it accepted.
    pub host_tick: u32,
}

/// Refuse: the join failed, with a reason code and a text for the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refuse {
    /// The nonce of the join, so a stranger cannot refuse on the host's
    /// behalf.
    pub nonce: u64,
    /// The reason code.
    pub reason: u8,
    /// Up to 200 bytes of UTF-8.
    pub text: String,
}

/// Disconnect: the connection ends. Sent three times. 10 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disconnect {
    /// The connection it ends.
    pub connection: u32,
    /// The reason code.
    pub reason: u8,
}

/// Keepalive: the connection is alive though its game's loop is stalled.
/// 9 bytes, smaller than the empty Payload it stands in for, and never
/// answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keepalive {
    /// The connection id from Accepted: the same identity every Payload
    /// carries, accepted only from the connection's own address.
    pub connection: u32,
}

/// Punch: the host's packet to a player the master introduced (stage J,
/// protocol 9). 13 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Punch {
    /// The introduction id the master gave both ends.
    pub introduction: u64,
}

/// Discover query: "who is hosting here?" The protocol version and the nonce
/// come first and never move, so a host of any build reads them. 1,000 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Discover {
    /// The asker's protocol version.
    pub protocol_version: u16,
    /// The asker's random nonce, which the answer repeats.
    pub nonce: u64,
}

/// Where a game is, as a game list shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiscoverPhase {
    /// In the lobby: the mission is chosen and not flying.
    Lobby = 0,
    /// The mission is flying.
    Flying = 1,
    /// The mission has ended or the host is stopping: joins are refused.
    Closed = 2,
}

impl DiscoverPhase {
    fn from_bits(bits: u64) -> Option<Self> {
        Some(match bits {
            0 => Self::Lobby,
            1 => Self::Flying,
            2 => Self::Closed,
            _ => return None,
        })
    }
}

/// Discover answer: a game's summary for the list. Its layout never changes
/// under this kind, so a later build's host is still read and shown as
/// another version. Never longer than the query it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverAnswer {
    /// The nonce of the query it answers.
    pub nonce: u64,
    /// The host's protocol version.
    pub protocol_version: u16,
    /// The host's game version and commit.
    pub game_version: String,
    /// The commit its build stamps.
    pub game_commit: String,
    /// The host's session id, which names one game across addresses.
    pub session_id: u64,
    /// The game's name.
    pub name: String,
    /// The mission's one-line summary.
    pub summary: String,
    /// Players connected.
    pub players: u8,
    /// Players the game seats at most.
    pub capacity: u8,
    /// A password is needed to join.
    pub password: bool,
    /// No place for another player.
    pub full: bool,
    /// Lobby, flying or closed.
    pub phase: DiscoverPhase,
    /// The King's callsign; empty when the game has no King (a dedicated
    /// server).
    pub king: String,
    /// The players' callsigns, as many as fit the query's length.
    pub callsigns: Vec<String>,
    /// `callsigns` leaves players out.
    pub truncated: bool,
}

impl DiscoverAnswer {
    /// The answer with its texts cut to their limits and as many callsigns
    /// as keep the packet within `max_len` bytes; `truncated` says when some
    /// are left out. The fixed part always fits 1,000 bytes; for a smaller
    /// `max_len` that cannot hold it, [`Packet::encode_within`] says so.
    pub fn fit(mut self, max_len: usize) -> Self {
        self.game_version = cut(&self.game_version, MAX_DISCOVER_BUILD);
        self.game_commit = cut(&self.game_commit, MAX_DISCOVER_BUILD);
        self.name = cut(&self.name, MAX_DISCOVER_NAME);
        self.summary = cut(&self.summary, MAX_DISCOVER_SUMMARY);
        self.king = cut(&self.king, MAX_CALLSIGN);
        let listed = self.callsigns.len();
        self.callsigns.retain(|c| valid_callsign(c));
        self.callsigns.truncate(usize::from(u8::MAX));
        self.truncated |= self.callsigns.len() < listed;
        let mut size = self.fixed_len();
        let mut keep = 0;
        for callsign in &self.callsigns {
            if size + 1 + callsign.len() > max_len {
                break;
            }
            size += 1 + callsign.len();
            keep += 1;
        }
        if keep < self.callsigns.len() {
            self.callsigns.truncate(keep);
            self.truncated = true;
        }
        self
    }

    /// The packet's length without the callsigns.
    fn fixed_len(&self) -> usize {
        // Header 5, nonce 8, protocol 2, flags 1, players 1, capacity 1,
        // session 8, five strings with their length bytes, the count.
        5 + 8
            + 2
            + 1
            + 1
            + 1
            + 8
            + 5
            + self.game_version.len()
            + self.game_commit.len()
            + self.name.len()
            + self.summary.len()
            + self.king.len()
            + 1
    }
}

/// `text` cut to at most `max` bytes at a character boundary.
fn cut(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// The fixed part of a Payload after the checksum and kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadHeader {
    /// The connection id from Accepted.
    pub connection: u32,
    /// This packet's sequence in its direction.
    pub sequence: u16,
    /// The newest sequence received from the other side.
    pub ack: u16,
    /// Bit i set: sequence `ack - 1 - i` was received.
    pub ack_bits: u32,
    /// Time from receiving `ack` to sending this, in 16-microsecond units;
    /// [`ACK_DELAY_NONE`] when nothing has been received yet.
    pub ack_delay: u16,
}

/// One section of a Payload: a kind and an opaque body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// 1 is this crate's Messages; 2 and up are the caller's.
    pub kind: u8,
    /// The body bytes.
    pub body: Vec<u8>,
}

/// Any packet, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet {
    /// Kind 1.
    ConnectRequest(ConnectRequest),
    /// Kind 2.
    Challenge(Challenge),
    /// Kind 3.
    ChallengeAnswer(ChallengeAnswer),
    /// Kind 4.
    Accepted(Accepted),
    /// Kind 5.
    Refuse(Refuse),
    /// Kind 6, with its sections in order.
    Payload(PayloadHeader, Vec<Section>),
    /// Kind 7.
    Disconnect(Disconnect),
    /// Kind 8.
    Discover(Discover),
    /// Kind 9.
    DiscoverAnswer(DiscoverAnswer),
    /// Kind 10.
    Keepalive(Keepalive),
    /// Kind 11.
    Punch(Punch),
    /// Kind 12.
    Reach(Reach),
    /// Kind 13.
    ReachAnswer(ReachAnswer),
}

impl Packet {
    /// This packet's kind.
    pub fn kind(&self) -> PacketKind {
        match self {
            Self::ConnectRequest(_) => PacketKind::ConnectRequest,
            Self::Challenge(_) => PacketKind::Challenge,
            Self::ChallengeAnswer(_) => PacketKind::ChallengeAnswer,
            Self::Accepted(_) => PacketKind::Accepted,
            Self::Refuse(_) => PacketKind::Refuse,
            Self::Payload(..) => PacketKind::Payload,
            Self::Disconnect(_) => PacketKind::Disconnect,
            Self::Discover(_) => PacketKind::Discover,
            Self::DiscoverAnswer(_) => PacketKind::DiscoverAnswer,
            Self::Keepalive(_) => PacketKind::Keepalive,
            Self::Punch(_) => PacketKind::Punch,
            Self::Reach(_) => PacketKind::Reach,
            Self::ReachAnswer(_) => PacketKind::ReachAnswer,
        }
    }

    /// Writes the packet like [`Packet::encode`], and refuses one longer than
    /// `max_len` bytes ([`EncodeError::TooLarge`]): how a host keeps an answer
    /// within the query it answers.
    pub fn encode_within(&self, version: u16, max_len: usize) -> Result<Vec<u8>, EncodeError> {
        let bytes = self.encode(version)?;
        if bytes.len() > max_len {
            return Err(EncodeError::TooLarge);
        }
        Ok(bytes)
    }

    /// Writes the packet with its checksum for protocol `version`.
    pub fn encode(&self, version: u16) -> Result<Vec<u8>, EncodeError> {
        let mut w = start(self.kind());
        match self {
            Self::ConnectRequest(p) => {
                w.write_bits(u64::from(p.protocol_version), 16).ok();
                w.write_bits(p.nonce, 64).ok();
                put_str(&mut w, &p.game_version)?;
                put_str(&mut w, &p.game_commit)?;
                pad(&mut w)?;
            }
            Self::Challenge(p) => {
                w.write_bits(p.nonce, 64).ok();
                w.write_bits(p.cookie, 64).ok();
            }
            Self::ChallengeAnswer(p) => {
                if !valid_callsign(&p.callsign) {
                    return Err(EncodeError::BadString);
                }
                w.write_bits(p.nonce, 64).ok();
                w.write_bits(p.cookie, 64).ok();
                put_str(&mut w, &p.callsign)?;
                put_str(&mut w, &p.password)?;
                put_str(&mut w, &p.game_version)?;
                put_str(&mut w, &p.game_commit)?;
                w.write_bits(u64::from(p.platform.code()), 8).ok();
                w.write_bits(u64::from(p.path.code()), 8).ok();
                match p.token {
                    None => {
                        w.write_bits(0, 8).ok();
                    }
                    Some(token) => {
                        w.write_bits(1, 8).ok();
                        token.write(&mut w);
                    }
                }
                pad(&mut w)?;
            }
            Self::Accepted(p) => {
                w.write_bits(p.nonce, 64).ok();
                w.write_bits(u64::from(p.connection), 32).ok();
                w.write_bits(p.session_id, 64).ok();
                w.write_bits(u64::from(p.ticks_per_second), 8).ok();
                w.write_bits(u64::from(p.ticks_per_snapshot), 8).ok();
                w.write_bits(u64::from(p.host_tick), 32).ok();
            }
            Self::Refuse(p) => {
                if p.text.len() > MAX_REFUSE_TEXT {
                    return Err(EncodeError::BadString);
                }
                w.write_bits(p.nonce, 64).ok();
                w.write_bits(u64::from(p.reason), 8).ok();
                put_str(&mut w, &p.text)?;
            }
            Self::Payload(header, sections) => {
                let refs: Vec<(u8, &[u8])> = sections
                    .iter()
                    .map(|s| (s.kind, s.body.as_slice()))
                    .collect();
                return write_payload(version, header, &refs);
            }
            Self::Disconnect(p) => {
                w.write_bits(u64::from(p.connection), 32).ok();
                w.write_bits(u64::from(p.reason), 8).ok();
            }
            Self::Discover(p) => {
                w.write_bits(u64::from(p.protocol_version), 16).ok();
                w.write_bits(p.nonce, 64).ok();
                pad(&mut w)?;
            }
            Self::DiscoverAnswer(p) => {
                if p.callsigns.len() > usize::from(u8::MAX)
                    || p.callsigns.iter().any(|c| !valid_callsign(c))
                {
                    return Err(EncodeError::BadString);
                }
                w.write_bits(p.nonce, 64).ok();
                w.write_bits(u64::from(p.protocol_version), 16).ok();
                let flags = u64::from(p.password)
                    | u64::from(p.full) << 1
                    | u64::from(p.truncated) << 2
                    | (p.phase as u64) << 3;
                w.write_bits(flags, 8).ok();
                w.write_bits(u64::from(p.players), 8).ok();
                w.write_bits(u64::from(p.capacity), 8).ok();
                w.write_bits(p.session_id, 64).ok();
                put_str(&mut w, &p.game_version)?;
                put_str(&mut w, &p.game_commit)?;
                put_str(&mut w, &p.name)?;
                put_str(&mut w, &p.summary)?;
                put_str(&mut w, &p.king)?;
                w.write_bits(p.callsigns.len() as u64, 8).ok();
                for callsign in &p.callsigns {
                    put_str(&mut w, callsign)?;
                }
            }
            Self::Keepalive(p) => {
                w.write_bits(u64::from(p.connection), 32).ok();
            }
            Self::Punch(p) => {
                w.write_bits(p.introduction, 64).ok();
            }
            Self::Reach(p) => {
                w.write_bits(p.session_id, 64).ok();
                w.write_bits(p.nonce, 64).ok();
                w.write_bits(u64::from(p.from), 8).ok();
            }
            Self::ReachAnswer(p) => {
                w.write_bits(p.nonce, 64).ok();
                w.write_bits(p.session_id, 64).ok();
                w.write_bits(p.role as u64, 8).ok();
            }
        }
        seal(w.finish(), version)
    }

    /// Checks and decodes a datagram for protocol `version`.
    pub fn decode(datagram: &[u8], version: u16) -> Result<Self, PacketError> {
        let (kind, body) = open(datagram, version)?;
        Ok(match kind {
            PacketKind::ConnectRequest => {
                Self::ConnectRequest(decode_connect_request(datagram.len(), body, version)?)
            }
            PacketKind::Challenge => Self::Challenge(decode_challenge(body)?),
            PacketKind::ChallengeAnswer => {
                Self::ChallengeAnswer(decode_challenge_answer(datagram.len(), body)?)
            }
            PacketKind::Accepted => Self::Accepted(decode_accepted(body)?),
            PacketKind::Refuse => Self::Refuse(decode_refuse(body)?),
            PacketKind::Payload => {
                let (header, rest) = decode_payload_header(body)?;
                Self::Payload(header, decode_sections(rest)?)
            }
            PacketKind::Disconnect => Self::Disconnect(decode_disconnect(body)?),
            PacketKind::Discover => Self::Discover(decode_discover(datagram.len(), body, version)?),
            PacketKind::DiscoverAnswer => Self::DiscoverAnswer(decode_discover_answer(body)?),
            PacketKind::Keepalive => Self::Keepalive(decode_keepalive(body)?),
            PacketKind::Punch => Self::Punch(decode_punch(body)?),
            PacketKind::Reach => Self::Reach(decode_reach(body)?),
            PacketKind::ReachAnswer => Self::ReachAnswer(decode_reach_answer(body)?),
        })
    }
}

/// True for 1 to 15 printable ASCII characters (space to tilde).
pub fn valid_callsign(callsign: &str) -> bool {
    (1..=MAX_CALLSIGN).contains(&callsign.len())
        && callsign.bytes().all(|b| (0x20..=0x7E).contains(&b))
}

fn start(kind: PacketKind) -> BitWriter {
    let mut w = BitWriter::with_capacity(64);
    w.write_bits(0, 32).ok();
    w.write_bits(kind as u64, 8).ok();
    w
}

fn put_str(w: &mut BitWriter, text: &str) -> Result<(), EncodeError> {
    w.write_str(text).map_err(|_| EncodeError::BadString)
}

fn pad(w: &mut BitWriter) -> Result<(), EncodeError> {
    let len = w.byte_len();
    if len > PADDED_LEN {
        return Err(EncodeError::TooLarge);
    }
    w.write_bytes(&vec![0u8; PADDED_LEN - len]);
    Ok(())
}

/// Fills in the checksum of a packet whose first four bytes are reserved for
/// it.
fn seal(mut bytes: Vec<u8>, version: u16) -> Result<Vec<u8>, EncodeError> {
    if bytes.len() > MAX_DATAGRAM {
        return Err(EncodeError::TooLarge);
    }
    let kind = bytes
        .get(4)
        .copied()
        .and_then(PacketKind::from_u8)
        .ok_or(EncodeError::TooLarge)?;
    let crc = checksum(kind, version, &bytes[4..]);
    bytes[..4].copy_from_slice(&crc.to_le_bytes());
    Ok(bytes)
}

/// The checksum of a packet: CRC-32 over the protocol id, then `rest`, which
/// starts at the kind byte.
pub fn checksum(kind: PacketKind, version: u16, rest: &[u8]) -> u32 {
    let mut crc = Crc32::new();
    crc.update(&protocol_id(kind, version));
    crc.update(rest);
    crc.finalize()
}

/// Checks a datagram's size, kind and checksum. Returns the kind and the
/// bytes after the kind byte.
pub fn open(datagram: &[u8], version: u16) -> Result<(PacketKind, &[u8]), PacketError> {
    if datagram.len() < HEADER_LEN {
        return Err(PacketError::TooShort);
    }
    if datagram.len() > MAX_DATAGRAM {
        return Err(PacketError::TooLong);
    }
    let kind = PacketKind::from_u8(datagram[4]).ok_or(PacketError::UnknownKind(datagram[4]))?;
    let sent = u32::from_le_bytes([datagram[0], datagram[1], datagram[2], datagram[3]]);
    if sent != checksum(kind, version, &datagram[4..]) {
        return Err(PacketError::Checksum);
    }
    Ok((kind, &datagram[HEADER_LEN..]))
}

/// Requires the reader to be at the end, with nothing left over.
fn end(r: &BitReader<'_>) -> Result<(), PacketError> {
    if r.bits_remaining() == 0 {
        Ok(())
    } else {
        Err(PacketError::Malformed)
    }
}

/// Requires the rest to be zero padding.
fn padding(r: &BitReader<'_>) -> Result<(), PacketError> {
    if r.only_zero_padding_left() {
        Ok(())
    } else {
        Err(PacketError::Malformed)
    }
}

fn u16_of(r: &mut BitReader<'_>) -> Result<u16, PacketError> {
    Ok(r.read_bits(16)? as u16)
}

fn u32_of(r: &mut BitReader<'_>) -> Result<u32, PacketError> {
    Ok(r.read_bits(32)? as u32)
}

fn u8_of(r: &mut BitReader<'_>) -> Result<u8, PacketError> {
    Ok(r.read_bits(8)? as u8)
}

/// Decodes a Connect request's body. `len` is the whole datagram's length,
/// which must be exactly 1,000. A request of another protocol version is read
/// only up to its nonce.
pub fn decode_connect_request(
    len: usize,
    body: &[u8],
    version: u16,
) -> Result<ConnectRequest, PacketError> {
    if len != PADDED_LEN {
        return Err(PacketError::Malformed);
    }
    let mut r = BitReader::new(body);
    let protocol_version = u16_of(&mut r)?;
    let nonce = r.read_bits(64)?;
    if protocol_version != version {
        return Ok(ConnectRequest {
            protocol_version,
            nonce,
            game_version: String::new(),
            game_commit: String::new(),
        });
    }
    let game_version = r.read_str()?;
    let game_commit = r.read_str()?;
    padding(&r)?;
    Ok(ConnectRequest {
        protocol_version,
        nonce,
        game_version,
        game_commit,
    })
}

/// Decodes a Discover query's body. `len` is the whole datagram's length,
/// which must be exactly 1,000. The padding must be zero when the asker's
/// protocol version is `version`; a query of another version may use it.
pub fn decode_discover(len: usize, body: &[u8], version: u16) -> Result<Discover, PacketError> {
    if len != DISCOVER_LEN {
        return Err(PacketError::Malformed);
    }
    let mut r = BitReader::new(body);
    let protocol_version = u16_of(&mut r)?;
    let nonce = r.read_bits(64)?;
    if protocol_version == version {
        padding(&r)?;
    }
    Ok(Discover {
        protocol_version,
        nonce,
    })
}

/// Decodes a Discover answer's body.
pub fn decode_discover_answer(body: &[u8]) -> Result<DiscoverAnswer, PacketError> {
    let mut r = BitReader::new(body);
    let nonce = r.read_bits(64)?;
    let protocol_version = u16_of(&mut r)?;
    let flags = r.read_bits(8)?;
    if flags >> 5 != 0 {
        return Err(PacketError::Malformed);
    }
    let phase = DiscoverPhase::from_bits((flags >> 3) & 3).ok_or(PacketError::Malformed)?;
    let players = u8_of(&mut r)?;
    let capacity = u8_of(&mut r)?;
    let session_id = r.read_bits(64)?;
    let game_version = r.read_str()?;
    let game_commit = r.read_str()?;
    let name = r.read_str()?;
    let summary = r.read_str()?;
    let king = r.read_str()?;
    let count = usize::from(u8_of(&mut r)?);
    let mut callsigns = Vec::with_capacity(count);
    for _ in 0..count {
        let callsign = r.read_str()?;
        if !valid_callsign(&callsign) {
            return Err(PacketError::Malformed);
        }
        callsigns.push(callsign);
    }
    end(&r)?;
    Ok(DiscoverAnswer {
        nonce,
        protocol_version,
        game_version,
        game_commit,
        session_id,
        name,
        summary,
        players,
        capacity,
        password: flags & 1 != 0,
        full: flags & 2 != 0,
        phase,
        king,
        callsigns,
        truncated: flags & 4 != 0,
    })
}

/// Decodes a Challenge's body.
pub fn decode_challenge(body: &[u8]) -> Result<Challenge, PacketError> {
    let mut r = BitReader::new(body);
    let nonce = r.read_bits(64)?;
    let cookie = r.read_bits(64)?;
    end(&r)?;
    Ok(Challenge { nonce, cookie })
}

/// Decodes a Challenge answer's body. `len` is the whole datagram's length,
/// which must be exactly 1,000. A platform, path or token code the protocol
/// does not name is malformed.
pub fn decode_challenge_answer(len: usize, body: &[u8]) -> Result<ChallengeAnswer, PacketError> {
    if len != PADDED_LEN {
        return Err(PacketError::Malformed);
    }
    let mut r = BitReader::new(body);
    let nonce = r.read_bits(64)?;
    let cookie = r.read_bits(64)?;
    let callsign = r.read_str()?;
    if !valid_callsign(&callsign) {
        return Err(PacketError::Malformed);
    }
    let password = r.read_str()?;
    let game_version = r.read_str()?;
    let game_commit = r.read_str()?;
    let platform = Platform::from_code(u8_of(&mut r)?).ok_or(PacketError::Malformed)?;
    let path = Path::from_code(u64::from(u8_of(&mut r)?)).ok_or(PacketError::Malformed)?;
    let token = match u8_of(&mut r)? {
        0 => None,
        1 => Some(Token::read(&mut r)?),
        _ => return Err(PacketError::Malformed),
    };
    padding(&r)?;
    Ok(ChallengeAnswer {
        nonce,
        cookie,
        callsign,
        password,
        game_version,
        game_commit,
        platform,
        path,
        token,
    })
}

/// Decodes an Accepted's body.
pub fn decode_accepted(body: &[u8]) -> Result<Accepted, PacketError> {
    let mut r = BitReader::new(body);
    let accepted = Accepted {
        nonce: r.read_bits(64)?,
        connection: u32_of(&mut r)?,
        session_id: r.read_bits(64)?,
        ticks_per_second: u8_of(&mut r)?,
        ticks_per_snapshot: u8_of(&mut r)?,
        host_tick: u32_of(&mut r)?,
    };
    end(&r)?;
    Ok(accepted)
}

/// Decodes a Refuse's body.
pub fn decode_refuse(body: &[u8]) -> Result<Refuse, PacketError> {
    let mut r = BitReader::new(body);
    let nonce = r.read_bits(64)?;
    let reason = u8_of(&mut r)?;
    let text = r.read_str()?;
    if text.len() > MAX_REFUSE_TEXT {
        return Err(PacketError::Malformed);
    }
    end(&r)?;
    Ok(Refuse {
        nonce,
        reason,
        text,
    })
}

/// Decodes a Disconnect's body.
pub fn decode_disconnect(body: &[u8]) -> Result<Disconnect, PacketError> {
    let mut r = BitReader::new(body);
    let connection = u32_of(&mut r)?;
    let reason = u8_of(&mut r)?;
    end(&r)?;
    Ok(Disconnect { connection, reason })
}

/// Decodes a Keepalive's body.
pub fn decode_keepalive(body: &[u8]) -> Result<Keepalive, PacketError> {
    let mut r = BitReader::new(body);
    let connection = u32_of(&mut r)?;
    end(&r)?;
    Ok(Keepalive { connection })
}

/// Decodes a Punch's body.
pub fn decode_punch(body: &[u8]) -> Result<Punch, PacketError> {
    let mut r = BitReader::new(body);
    let introduction = r.read_bits(64)?;
    end(&r)?;
    Ok(Punch { introduction })
}

/// Decodes a Reach's body.
pub fn decode_reach(body: &[u8]) -> Result<Reach, PacketError> {
    let mut r = BitReader::new(body);
    let reach = Reach {
        session_id: r.read_bits(64)?,
        nonce: r.read_bits(64)?,
        from: u8_of(&mut r)?,
    };
    end(&r)?;
    Ok(reach)
}

/// Decodes a Reach answer's body; a role the protocol does not name is
/// malformed.
pub fn decode_reach_answer(body: &[u8]) -> Result<ReachAnswer, PacketError> {
    let mut r = BitReader::new(body);
    let nonce = r.read_bits(64)?;
    let session_id = r.read_bits(64)?;
    let role = ReachRole::from_code(u8_of(&mut r)?).ok_or(PacketError::Malformed)?;
    end(&r)?;
    Ok(ReachAnswer {
        nonce,
        session_id,
        role,
    })
}

/// Decodes a Payload's fixed header; returns it and the section bytes.
pub fn decode_payload_header(body: &[u8]) -> Result<(PayloadHeader, &[u8]), PacketError> {
    let fixed = PAYLOAD_HEADER_LEN - HEADER_LEN;
    if body.len() < fixed {
        return Err(PacketError::Malformed);
    }
    let mut r = BitReader::new(&body[..fixed]);
    let header = PayloadHeader {
        connection: u32_of(&mut r)?,
        sequence: u16_of(&mut r)?,
        ack: u16_of(&mut r)?,
        ack_bits: u32_of(&mut r)?,
        ack_delay: u16_of(&mut r)?,
    };
    Ok((header, &body[fixed..]))
}

/// Splits a Payload's section bytes into sections. Kind 0, or a length that
/// runs past the packet, is malformed. Which kinds are known is the caller's
/// check.
pub fn decode_sections(mut rest: &[u8]) -> Result<Vec<Section>, PacketError> {
    let mut sections = Vec::new();
    while !rest.is_empty() {
        if rest.len() < SECTION_HEADER_LEN {
            return Err(PacketError::Malformed);
        }
        let kind = rest[0];
        let len = usize::from(u16::from_le_bytes([rest[1], rest[2]]));
        if kind == 0 || rest.len() - SECTION_HEADER_LEN < len {
            return Err(PacketError::Malformed);
        }
        let body = &rest[SECTION_HEADER_LEN..SECTION_HEADER_LEN + len];
        sections.push(Section {
            kind,
            body: body.to_vec(),
        });
        rest = &rest[SECTION_HEADER_LEN + len..];
    }
    Ok(sections)
}

/// Writes a sealed Payload from a header and borrowed sections.
pub fn write_payload(
    version: u16,
    header: &PayloadHeader,
    sections: &[(u8, &[u8])],
) -> Result<Vec<u8>, EncodeError> {
    let size = PAYLOAD_HEADER_LEN
        + sections
            .iter()
            .map(|(_, body)| SECTION_HEADER_LEN + body.len())
            .sum::<usize>();
    if size > MAX_DATAGRAM {
        return Err(EncodeError::TooLarge);
    }
    let mut w = start(PacketKind::Payload);
    w.write_bits(u64::from(header.connection), 32).ok();
    w.write_bits(u64::from(header.sequence), 16).ok();
    w.write_bits(u64::from(header.ack), 16).ok();
    w.write_bits(u64::from(header.ack_bits), 32).ok();
    w.write_bits(u64::from(header.ack_delay), 16).ok();
    for (kind, body) in sections {
        if *kind == 0 {
            return Err(EncodeError::BadSectionKind);
        }
        w.write_bits(u64::from(*kind), 8).ok();
        w.write_bits(body.len() as u64, 16).ok();
        w.write_bytes(body);
    }
    seal(w.finish(), version)
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: u16 = 3;

    fn samples() -> Vec<Packet> {
        vec![
            Packet::ConnectRequest(ConnectRequest {
                protocol_version: V,
                nonce: 0x0123_4567_89AB_CDEF,
                game_version: "0.1.3".into(),
                game_commit: "fb9c2ec".into(),
            }),
            Packet::Challenge(Challenge {
                nonce: 1,
                cookie: u64::MAX,
            }),
            Packet::ChallengeAnswer(ChallengeAnswer {
                nonce: 2,
                cookie: 3,
                callsign: "Viper".into(),
                password: "secret".into(),
                game_version: "0.1.3".into(),
                game_commit: "fb9c2ec".into(),
                platform: Platform::MacOs,
                path: Path::Punched,
                token: Some(Token(0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210)),
            }),
            Packet::Accepted(Accepted {
                nonce: 4,
                connection: 0xDEAD_BEEF,
                session_id: 5,
                ticks_per_second: 120,
                ticks_per_snapshot: 4,
                host_tick: 123_456,
            }),
            Packet::Refuse(Refuse {
                nonce: 6,
                reason: 4,
                text: "Wrong password.".into(),
            }),
            Packet::Payload(
                PayloadHeader {
                    connection: 7,
                    sequence: 65_535,
                    ack: 12,
                    ack_bits: 0x8000_0001,
                    ack_delay: ACK_DELAY_NONE,
                },
                vec![
                    Section {
                        kind: 2,
                        body: vec![1, 2, 3],
                    },
                    Section {
                        kind: 5,
                        body: vec![],
                    },
                ],
            ),
            Packet::Disconnect(Disconnect {
                connection: 8,
                reason: 2,
            }),
            Packet::Discover(Discover {
                protocol_version: V,
                nonce: 0xFEDC_BA98_7654_3210,
            }),
            Packet::DiscoverAnswer(answer(&["Viper", "Maverick 1"])),
            Packet::Keepalive(Keepalive {
                connection: 0xDEAD_BEEF,
            }),
            Packet::Punch(Punch {
                introduction: 0x0123_4567_89AB_CDEF,
            }),
            Packet::Reach(Reach {
                session_id: 0x1122_3344_5566_7788,
                nonce: 0x99AA_BBCC_DDEE_FF00,
                from: 7,
            }),
            Packet::ReachAnswer(ReachAnswer {
                nonce: 0x99AA_BBCC_DDEE_FF00,
                session_id: 0x1122_3344_5566_7788,
                role: ReachRole::Hosting,
            }),
        ]
    }

    fn answer(callsigns: &[&str]) -> DiscoverAnswer {
        DiscoverAnswer {
            nonce: 0xFEDC_BA98_7654_3210,
            protocol_version: V,
            game_version: "0.1.3".into(),
            game_commit: "fb9c2ec".into(),
            session_id: 0x1122_3344_5566_7788,
            name: "Friday night".into(),
            summary:
                "UKR, clear, airborne at 20000 ft: F/A-18D Hornet x4 against MiG-29 Fulcrum-C x4"
                    .into(),
            players: callsigns.len() as u8,
            capacity: 8,
            password: true,
            full: false,
            phase: DiscoverPhase::Lobby,
            king: callsigns.first().copied().unwrap_or_default().into(),
            callsigns: callsigns.iter().map(|c| (*c).to_owned()).collect(),
            truncated: false,
        }
    }

    #[test]
    fn every_kind_round_trips() {
        for packet in samples() {
            let bytes = packet.encode(V).unwrap();
            assert!(bytes.len() <= MAX_DATAGRAM);
            assert_eq!(Packet::decode(&bytes, V).unwrap(), packet, "{packet:?}");
        }
    }

    #[test]
    fn sizes_match_the_spec() {
        let sizes: Vec<usize> = samples()
            .iter()
            .map(|p| p.encode(V).unwrap().len())
            .collect();
        // Request and answer padded; Challenge 21; Accepted 31; Disconnect 10;
        // the Payload is its 19-byte header plus two sections.
        assert_eq!(sizes[0], PADDED_LEN);
        assert_eq!(sizes[1], 21);
        assert_eq!(sizes[2], PADDED_LEN);
        assert_eq!(sizes[3], 31);
        assert_eq!(sizes[5], PAYLOAD_HEADER_LEN + 3 + 3 + 3);
        assert_eq!(sizes[6], 10);
        // The query is padded to the longest answer it may be given.
        assert_eq!(sizes[7], DISCOVER_LEN);
        assert!(sizes[8] < DISCOVER_LEN);
        // A keepalive is smaller than the empty Payload it stands in for.
        assert_eq!(sizes[9], 9);
        assert!(sizes[9] < PAYLOAD_HEADER_LEN);
        // A punch is its header and the introduction id.
        assert_eq!(sizes[10], 13);
        // A Reach and its answer are 22 bytes each: the answer is never
        // longer than the Reach.
        assert_eq!(sizes[11], 22);
        assert_eq!(sizes[12], 22);
    }

    #[test]
    fn a_reach_answer_names_a_known_role_and_both_are_exact() {
        for role in [ReachRole::NotHosting, ReachRole::Hosting] {
            let answer = Packet::ReachAnswer(ReachAnswer {
                nonce: 1,
                session_id: 2,
                role,
            });
            let bytes = answer.encode(V).unwrap();
            assert_eq!(Packet::decode(&bytes, V).unwrap(), answer);
        }
        // Role 2 and up name nothing.
        let mut bytes = samples()[12].encode(V).unwrap();
        for code in [2, 0x80, u8::MAX] {
            bytes[21] = code;
            let crc = checksum(PacketKind::ReachAnswer, V, &bytes[4..]);
            bytes[..4].copy_from_slice(&crc.to_le_bytes());
            assert_eq!(Packet::decode(&bytes, V), Err(PacketError::Malformed));
        }
        // Shorter or longer bodies are malformed.
        assert_eq!(decode_reach(&[0; 16]), Err(PacketError::Malformed));
        assert_eq!(decode_reach(&[0; 18]), Err(PacketError::Malformed));
        assert_eq!(decode_reach_answer(&[0; 18]), Err(PacketError::Malformed));
        assert_eq!(
            decode_reach(&[1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3]),
            Ok(Reach {
                session_id: 1,
                nonce: 2,
                from: 3
            })
        );
    }

    #[test]
    fn the_answer_carries_a_token_or_none_and_refuses_another_code() {
        let Packet::ChallengeAnswer(base) = samples()[2].clone() else {
            panic!("not an answer")
        };
        // The token byte follows the path byte.
        let at = 5
            + 8
            + 8
            + [
                &base.callsign,
                &base.password,
                &base.game_version,
                &base.game_commit,
            ]
            .iter()
            .map(|s| 1 + s.len())
            .sum::<usize>()
            + 2;
        for token in [None, Some(Token(0)), Some(Token(u128::MAX)), base.token] {
            let answer = ChallengeAnswer {
                token,
                ..base.clone()
            };
            let bytes = Packet::ChallengeAnswer(answer.clone()).encode(V).unwrap();
            assert_eq!(bytes.len(), PADDED_LEN);
            assert_eq!(bytes[at], u8::from(token.is_some()));
            assert_eq!(
                Packet::decode(&bytes, V).unwrap(),
                Packet::ChallengeAnswer(answer)
            );
        }
        // The token's low byte comes first.
        let bytes = samples()[2].encode(V).unwrap();
        assert_eq!(bytes[at + 1], 0x10);
        assert_eq!(bytes[at + 16], 0x01);
        let mut bytes = Packet::ChallengeAnswer(ChallengeAnswer {
            token: None,
            ..base
        })
        .encode(V)
        .unwrap();
        for code in [2, 0x80, u8::MAX] {
            bytes[at] = code;
            let crc = checksum(PacketKind::ChallengeAnswer, V, &bytes[4..]);
            bytes[..4].copy_from_slice(&crc.to_le_bytes());
            assert_eq!(Packet::decode(&bytes, V), Err(PacketError::Malformed));
        }
    }

    #[test]
    fn the_answers_fixed_part_is_counted_exactly() {
        let bare = answer(&[]);
        let size = Packet::DiscoverAnswer(bare.clone())
            .encode(V)
            .unwrap()
            .len();
        assert_eq!(size, bare.fixed_len());
        let two = answer(&["Viper", "Maverick 1"]);
        let size = Packet::DiscoverAnswer(two.clone()).encode(V).unwrap().len();
        assert_eq!(size, two.fixed_len() + 6 + 11);
    }

    #[test]
    fn no_answer_is_longer_than_its_query_for_the_largest_lobby() {
        // 30 players with 15-character callsigns, the longest texts.
        let callsigns: Vec<String> = (0..30).map(|i| format!("Callsign_{i:06}")).collect();
        assert!(callsigns.iter().all(|c| c.len() == MAX_CALLSIGN));
        let mut biggest = answer(&[]);
        biggest.game_version = "9".repeat(300);
        biggest.game_commit = "f".repeat(300);
        biggest.name = "n".repeat(300);
        biggest.summary = "s\u{e9}".repeat(300);
        biggest.king = callsigns[0].clone();
        biggest.players = 30;
        biggest.callsigns = callsigns;
        let fitted = biggest.clone().fit(DISCOVER_LEN);
        let bytes = Packet::DiscoverAnswer(fitted.clone())
            .encode_within(V, DISCOVER_LEN)
            .unwrap();
        assert!(bytes.len() <= DISCOVER_LEN, "{} bytes", bytes.len());
        // Everything fits: the cuts keep the texts at their limits and the
        // whole player list in.
        assert_eq!(fitted.callsigns.len(), 30);
        assert!(!fitted.truncated);
        assert_eq!(fitted.name.len(), MAX_DISCOVER_NAME);
        assert!(fitted.summary.len() <= MAX_DISCOVER_SUMMARY);
        assert_eq!(fitted.game_version.len(), MAX_DISCOVER_BUILD);
        // A shorter allowance drops callsigns from the end and says so.
        for max in [500, 700, 800] {
            let small = biggest.clone().fit(max);
            let bytes = Packet::DiscoverAnswer(small.clone())
                .encode_within(V, max)
                .unwrap();
            assert!(bytes.len() <= max);
            assert!(small.truncated && small.callsigns.len() < 30);
            assert_eq!(small.callsigns[0], "Callsign_000000");
            let Packet::DiscoverAnswer(back) = Packet::decode(&bytes, V).unwrap() else {
                panic!("not an answer")
            };
            assert_eq!(back, small);
        }
        // An answer that is too long is refused, never sent.
        assert_eq!(
            Packet::DiscoverAnswer(fitted).encode_within(V, 100),
            Err(EncodeError::TooLarge)
        );
    }

    #[test]
    fn a_query_is_exactly_the_padded_length_with_zero_padding() {
        let query = samples()[7].encode(V).unwrap();
        assert_eq!(query.len(), DISCOVER_LEN);
        assert!(Packet::decode(&query[..DISCOVER_LEN - 1], V).is_err());
        let mut long = query.clone();
        long.push(0);
        let crc = checksum(PacketKind::Discover, V, &long[4..]);
        long[..4].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(Packet::decode(&long, V), Err(PacketError::Malformed));
        // Padding of the asker's own version must be zero; another version
        // may use it, and is still read to its nonce.
        let mut bad = query.clone();
        bad[DISCOVER_LEN - 1] = 1;
        let crc = checksum(PacketKind::Discover, V, &bad[4..]);
        bad[..4].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(Packet::decode(&bad, V), Err(PacketError::Malformed));
        assert!(matches!(
            Packet::decode(&bad, V + 1),
            Ok(Packet::Discover(d)) if d.nonce == 0xFEDC_BA98_7654_3210 && d.protocol_version == V
        ));
    }

    #[test]
    fn an_answer_with_trailing_bytes_or_reserved_flags_is_malformed() {
        let mut bytes = samples()[8].encode(V).unwrap();
        bytes.push(0);
        let crc = checksum(PacketKind::DiscoverAnswer, V, &bytes[4..]);
        bytes[..4].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(Packet::decode(&bytes, V), Err(PacketError::Malformed));
        let mut bytes = samples()[8].encode(V).unwrap();
        bytes[5 + 8 + 2] |= 0x20;
        let crc = checksum(PacketKind::DiscoverAnswer, V, &bytes[4..]);
        bytes[..4].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(Packet::decode(&bytes, V), Err(PacketError::Malformed));
        // A phase of 3 does not exist.
        let mut bytes = samples()[8].encode(V).unwrap();
        bytes[5 + 8 + 2] |= 3 << 3;
        let crc = checksum(PacketKind::DiscoverAnswer, V, &bytes[4..]);
        bytes[..4].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(Packet::decode(&bytes, V), Err(PacketError::Malformed));
    }

    /// The discovery packets' encodings, one line each: their layout never
    /// changes under their kinds (a later layout is a later kind), so they
    /// have a golden file of their own, outside the protocol version's. Refresh
    /// it with `TORE_UPDATE_DISCOVER_GOLDEN=1 cargo test -p tore-net
    /// discover_golden`, only when adding a line.
    #[test]
    fn discover_golden() {
        use std::fmt::Write as _;
        let hex = |bytes: &[u8]| {
            bytes.iter().fold(String::new(), |mut out, b| {
                let _ = write!(out, "{b:02x}");
                out
            })
        };
        let mut text = String::from(
            "# T.O.R.E discovery golden: kinds 8 and 9, checked with the TORE-HELLO id.\n\
             # Version-free: these bytes never change.\n",
        );
        let samples = samples();
        for (name, packet) in [
            ("discover", &samples[7]),
            ("answer", &samples[8]),
            ("answer-empty", &Packet::DiscoverAnswer(answer(&[]))),
        ] {
            // The query is zeros after its first 15 bytes; the line keeps the
            // head and the length.
            let bytes = packet.encode(V).unwrap();
            let shown = if name == "discover" {
                &bytes[..15]
            } else {
                &bytes[..]
            };
            let _ = writeln!(text, "{name} {} {}", bytes.len(), hex(shown));
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("discover-golden.txt");
        if std::env::var_os("TORE_UPDATE_DISCOVER_GOLDEN").is_some() {
            std::fs::write(&path, &text).unwrap();
            return;
        }
        // A Windows checkout may turn the file's line ends into CR LF.
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert_eq!(
            text, committed,
            "the discovery encodings changed: they are version-free, so add a new kind instead"
        );
        // And the committed bytes decode back to the samples.
        for (name, packet) in [("discover", &samples[7]), ("answer", &samples[8])] {
            let line = committed
                .lines()
                .find(|l| l.starts_with(name) && l[name.len()..].starts_with(' '))
                .unwrap();
            let encoded = packet.encode(V).unwrap();
            assert_eq!(line.split(' ').nth(1).unwrap(), encoded.len().to_string());
        }
    }

    #[test]
    fn the_answer_carries_every_platform_and_refuses_an_unknown_code() {
        let Packet::ChallengeAnswer(base) = samples()[2].clone() else {
            panic!("not an answer")
        };
        // Header 5, nonce 8, cookie 8, then four strings with their length
        // bytes; the platform byte follows them.
        let at = 5
            + 8
            + 8
            + [
                &base.callsign,
                &base.password,
                &base.game_version,
                &base.game_commit,
            ]
            .iter()
            .map(|s| 1 + s.len())
            .sum::<usize>();
        for platform in Platform::ALL {
            let answer = ChallengeAnswer {
                platform,
                ..base.clone()
            };
            let bytes = Packet::ChallengeAnswer(answer.clone()).encode(V).unwrap();
            assert_eq!(bytes.len(), PADDED_LEN);
            assert_eq!(bytes[at], platform.code());
            assert_eq!(
                Packet::decode(&bytes, V).unwrap(),
                Packet::ChallengeAnswer(answer)
            );
        }
        let mut bytes = samples()[2].encode(V).unwrap();
        for code in [Platform::MAX_CODE + 1, 7, u8::MAX] {
            bytes[at] = code;
            let crc = checksum(PacketKind::ChallengeAnswer, V, &bytes[4..]);
            bytes[..4].copy_from_slice(&crc.to_le_bytes());
            assert_eq!(Packet::decode(&bytes, V), Err(PacketError::Malformed));
        }
    }

    #[test]
    fn another_version_fails_the_checksum_except_hello_kinds() {
        for packet in samples() {
            let bytes = packet.encode(V).unwrap();
            let other = Packet::decode(&bytes, V + 1);
            match packet.kind() {
                PacketKind::Refuse | PacketKind::Discover | PacketKind::DiscoverAnswer => {
                    assert_eq!(other.unwrap(), packet)
                }
                PacketKind::ConnectRequest => {
                    // Read up to the nonce so the host can refuse by version.
                    let Packet::ConnectRequest(req) = other.unwrap() else {
                        panic!("not a request")
                    };
                    assert_eq!(req.protocol_version, V);
                    assert_eq!(req.nonce, 0x0123_4567_89AB_CDEF);
                    assert!(req.game_version.is_empty());
                }
                _ => assert_eq!(other, Err(PacketError::Checksum)),
            }
        }
    }

    #[test]
    fn checksum_catches_any_single_bit_flip() {
        for packet in samples() {
            let bytes = packet.encode(V).unwrap();
            for bit in 0..bytes.len().min(64) * 8 {
                let mut bad = bytes.clone();
                bad[bit / 8] ^= 1 << (bit % 8);
                assert!(Packet::decode(&bad, V).is_err());
            }
        }
    }

    #[test]
    fn strict_decoding_rejects_extras() {
        // Nonzero padding in a request.
        let mut bytes = samples()[0].encode(V).unwrap();
        bytes[999] = 1;
        let kind = PacketKind::ConnectRequest;
        let crc = checksum(kind, V, &bytes[4..]);
        bytes[..4].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(Packet::decode(&bytes, V), Err(PacketError::Malformed));
        // A short request.
        let short = &samples()[0].encode(V).unwrap()[..999];
        assert!(Packet::decode(short, V).is_err());
        // Bad callsigns do not encode.
        for callsign in ["", "0123456789ABCDEF", "tab\there", "é"] {
            let answer = Packet::ChallengeAnswer(ChallengeAnswer {
                nonce: 0,
                cookie: 0,
                callsign: callsign.into(),
                password: String::new(),
                game_version: String::new(),
                game_commit: String::new(),
                platform: Platform::Unknown,
                path: Path::ByAddress,
                token: None,
            });
            assert_eq!(answer.encode(V), Err(EncodeError::BadString));
        }
        // A section running past the end.
        assert_eq!(
            decode_sections(&[2, 5, 0, 1, 2]),
            Err(PacketError::Malformed)
        );
        assert_eq!(decode_sections(&[0, 0, 0]), Err(PacketError::Malformed));
        assert_eq!(decode_sections(&[2, 0]), Err(PacketError::Malformed));
        assert_eq!(decode_sections(&[]), Ok(vec![]));
        // A keepalive is exactly its connection id: shorter or longer is
        // malformed.
        assert_eq!(decode_keepalive(&[1, 2, 3]), Err(PacketError::Malformed));
        assert_eq!(
            decode_keepalive(&[1, 2, 3, 4, 0]),
            Err(PacketError::Malformed)
        );
        assert_eq!(
            decode_keepalive(&[0xEF, 0xBE, 0xAD, 0xDE]),
            Ok(Keepalive {
                connection: 0xDEAD_BEEF
            })
        );
        // A punch is exactly its introduction id.
        assert_eq!(decode_punch(&[1; 7]), Err(PacketError::Malformed));
        assert_eq!(decode_punch(&[1; 9]), Err(PacketError::Malformed));
        assert_eq!(
            decode_punch(&[1, 0, 0, 0, 0, 0, 0, 0]),
            Ok(Punch { introduction: 1 })
        );
    }

    #[test]
    fn the_answer_carries_every_path_and_refuses_an_unknown_code() {
        let Packet::ChallengeAnswer(base) = samples()[2].clone() else {
            panic!("not an answer")
        };
        // The path byte follows the platform byte.
        let at = 5
            + 8
            + 8
            + [
                &base.callsign,
                &base.password,
                &base.game_version,
                &base.game_commit,
            ]
            .iter()
            .map(|s| 1 + s.len())
            .sum::<usize>()
            + 1;
        for path in Path::ALL {
            let answer = ChallengeAnswer {
                path: *path,
                ..base.clone()
            };
            let bytes = Packet::ChallengeAnswer(answer.clone()).encode(V).unwrap();
            assert_eq!(bytes.len(), PADDED_LEN);
            assert_eq!(bytes[at], path.code());
            assert_eq!(
                Packet::decode(&bytes, V).unwrap(),
                Packet::ChallengeAnswer(answer)
            );
        }
        let mut bytes = samples()[2].encode(V).unwrap();
        for code in [6, 7, u8::MAX] {
            bytes[at] = code;
            let crc = checksum(PacketKind::ChallengeAnswer, V, &bytes[4..]);
            bytes[..4].copy_from_slice(&crc.to_le_bytes());
            assert_eq!(Packet::decode(&bytes, V), Err(PacketError::Malformed));
        }
    }
}
