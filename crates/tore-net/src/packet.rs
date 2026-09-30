//! Packets: the header, the checksum and the seven kinds.
//!
//! Every packet starts with a CRC-32 checksum (4 bytes, least significant
//! byte first) and a kind byte. The checksum covers a 10-byte protocol id that
//! is never sent, followed by every byte after the checksum. Connect request
//! and Refuse use the fixed id `TORE-HELLO`, so a host of any version can read
//! and answer them; the other kinds use `TORE-NET` and the protocol version
//! (16 bits), so a packet from another program or version fails its checksum.
//! See "Packets" and "Connecting" in
//! [`docs/formats/net-protocol.md`](../../../docs/formats/net-protocol.md).
//!
//! Every decoder is bounded and returns an error for any bytes; none panics.
//! Decoding is strict: a field outside its limits, trailing bytes, or padding
//! that is not zero rejects the packet.

use std::fmt;

use tore_codec::{BitReader, BitWriter, CodecError, Crc32};

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

/// The seven packet kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PacketKind {
    /// Client to host, padded to 1,000 bytes, `TORE-HELLO` id.
    ConnectRequest = 1,
    /// Host to client: the cookie.
    Challenge = 2,
    /// Client to host: the cookie echoed, the callsign and password, padded.
    ChallengeAnswer = 3,
    /// Host to client: the connection id and the session's rates.
    Accepted = 4,
    /// Host to client, `TORE-HELLO` id: a reason code and text.
    Refuse = 5,
    /// Both ways once connected.
    Payload = 6,
    /// Both ways: the connection ends.
    Disconnect = 7,
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
            _ => return None,
        })
    }

    /// True for the two kinds checked with the fixed `TORE-HELLO` id.
    pub fn uses_hello_id(self) -> bool {
        matches!(self, Self::ConnectRequest | Self::Refuse)
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
    /// A kind byte that is not one of the seven.
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
        }
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

/// Decodes a Challenge's body.
pub fn decode_challenge(body: &[u8]) -> Result<Challenge, PacketError> {
    let mut r = BitReader::new(body);
    let nonce = r.read_bits(64)?;
    let cookie = r.read_bits(64)?;
    end(&r)?;
    Ok(Challenge { nonce, cookie })
}

/// Decodes a Challenge answer's body. `len` is the whole datagram's length,
/// which must be exactly 1,000.
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
    padding(&r)?;
    Ok(ChallengeAnswer {
        nonce,
        cookie,
        callsign,
        password,
        game_version,
        game_commit,
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
        ]
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
    }

    #[test]
    fn another_version_fails_the_checksum_except_hello_kinds() {
        for packet in samples() {
            let bytes = packet.encode(V).unwrap();
            let other = Packet::decode(&bytes, V + 1);
            match packet.kind() {
                PacketKind::Refuse => assert_eq!(other.unwrap(), packet),
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
    }
}
