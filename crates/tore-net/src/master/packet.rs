//! The master protocol's packets: the header, the checksum and the 26 kinds
//! ("Packets" and the sections after it in the master protocol).
//!
//! Every packet starts with a CRC-32 checksum (4 bytes, least significant
//! byte first) over the 11 bytes `TORE-MASTER` followed by every byte after
//! the checksum, then the kind (8 bits) and the master protocol version (16
//! bits): 7 bytes. The fields follow bit-packed, least significant bit first,
//! with no alignment between them; strings are a length byte and UTF-8.
//!
//! Every decoder is bounded and returns an error for any bytes; none panics.
//! Decoding is strict, so every packet has exactly one encoding: a field
//! outside its limits, a code the protocol does not name, a reserved bit
//! that is set, trailing bytes, or padding that is not zero rejects it.
//! Requests an unproven sender may make (Register, Browse, Details, Probe,
//! Introduce) are padded with zeros to a fixed length that is at least as
//! long as any answer to them, and must be exactly that long.

use std::fmt;
use std::net::SocketAddr;

use tore_codec::{BitReader, BitWriter, CodecError, Crc32};

use super::candidate::{self, Candidate, MappingType};
use super::{MASTER_VERSION, SUPPORTED_VERSIONS};
use crate::MAX_DATAGRAM;
use crate::packet::{
    DiscoverAnswer, DiscoverPhase, MAX_CALLSIGN, MAX_DISCOVER_BUILD, MAX_DISCOVER_NAME,
    MAX_DISCOVER_SUMMARY, MAX_REFUSE_TEXT, valid_callsign,
};

/// The protocol id every master packet's checksum covers. It is never sent.
pub const MASTER_ID: [u8; 11] = *b"TORE-MASTER";
/// Checksum, kind and version.
pub const HEADER_LEN: usize = 7;
/// The longest master datagram: a relayed game datagram of 1,200 bytes and
/// its frame fit, and so does the smallest IPv6 path (1,280 bytes less 48 of
/// IPv6 and UDP headers).
pub const MAX_MASTER_DATAGRAM: usize = 1232;
/// The exact length of a Register.
pub const REGISTER_LEN: usize = 1200;
/// The exact length of a Browse.
pub const BROWSE_LEN: usize = 1200;
/// The exact length of a Details.
pub const DETAILS_LEN: usize = 1000;
/// The exact length of an Introduce.
pub const INTRODUCE_LEN: usize = 1000;
/// The exact length of a Probe.
pub const PROBE_LEN: usize = 64;
/// A Relay frame's length before the game datagram it carries: 15 bytes.
pub const RELAY_HEADER_LEN: usize = HEADER_LEN + 4 + 4;
/// The longest game datagram a Relay frame carries.
pub const MAX_RELAYED: usize = MAX_DATAGRAM;
// The longest game datagram and its frame fit the longest master datagram.
const _: () = assert!(RELAY_HEADER_LEN + MAX_RELAYED <= MAX_MASTER_DATAGRAM);
/// The longest text of an Unsupported, in bytes.
pub const MAX_UNSUPPORTED_TEXT: usize = 100;
/// The longest refusal text of an Introduction or a Relay offer, in bytes:
/// the game's own refusal limit (agent decision).
pub const MAX_TEXT: usize = MAX_REFUSE_TEXT;
/// The longest game version or commit in a [`Build`], in bytes.
pub const MAX_BUILD_TEXT: usize = MAX_DISCOVER_BUILD;
/// The most entries a Page carries (its count is 8 bits).
pub const MAX_PAGE_ENTRIES: usize = 255;
/// The highest platform code a Page entry carries (3 bits).
pub const MAX_PAGE_PLATFORM: u8 = 7;

codes! {
    /// The 26 kinds.
    MasterKind: 8 bits {
        /// Master to anyone: the cookie that proves an address. 23 bytes.
        Challenge = 1,
        /// Master to anyone: the sender's version is not supported. Its
        /// layout never changes, so it is read whatever its version.
        Unsupported = 2,
        /// Host to master, padded to 1,200 bytes: list my game.
        Register = 3,
        /// Master to host: the listing id and token.
        Listed = 4,
        /// Host to master: the listing's summary, every 30 seconds.
        Heartbeat = 5,
        /// Master to host.
        HeartbeatAck = 6,
        /// Host to master, 15 bytes: keeps the router's mapping open.
        Keep = 7,
        /// Master to host, 15 bytes: register again.
        UnknownListing = 8,
        /// Host to master, 15 bytes: remove my listing.
        Unregister = 9,
        /// Anyone to master, padded to 1,200 bytes: a page of the list.
        Browse = 10,
        /// Master to asker.
        Page = 11,
        /// Anyone to master, padded to 1,000 bytes: one listing's summary.
        Details = 12,
        /// Master to asker.
        ListingDetails = 13,
        /// Anyone to both master ports, padded to 64 bytes: the mapping test.
        Probe = 14,
        /// Master to prober: the address it saw.
        ProbeAnswer = 15,
        /// Player to master, padded to 1,000 bytes: introduce me to a host.
        Introduce = 16,
        /// Master to player: the host's addresses.
        Introduction = 17,
        /// Master to host: a player's addresses.
        Meet = 18,
        /// Host to master.
        MeetAck = 19,
        /// Player to master: open a relay channel.
        RelayRequest = 20,
        /// Master to player: the channel and its key, or why not.
        RelayOffer = 21,
        /// Master to host: a channel opens.
        RelayOpen = 22,
        /// Host to master.
        RelayOpenAck = 23,
        /// Both ends to master, master to the other end: one game datagram.
        Relay = 24,
        /// Either end or the master: a channel closes.
        RelayClose = 25,
        /// Game to master: anonymous telemetry.
        Report = 26,
    }
}

impl MasterKind {
    /// The exact length of a padded request; `None` for the kinds that are
    /// not padded.
    pub const fn padded_len(self) -> Option<usize> {
        match self {
            Self::Register => Some(REGISTER_LEN),
            Self::Browse => Some(BROWSE_LEN),
            Self::Details => Some(DETAILS_LEN),
            Self::Introduce => Some(INTRODUCE_LEN),
            Self::Probe => Some(PROBE_LEN),
            _ => None,
        }
    }
}

codes! {
    /// Which master port a Probe answer came from.
    ProbePort: 8 bits {
        /// The main port (26901).
        Main = 0,
        /// The second port (26902).
        Second = 1,
    }
}

codes! {
    /// An Introduction's result.
    IntroductionResult: 8 bits {
        /// Introduced: the host's addresses follow.
        Introduced = 0,
        /// No such listing.
        NoListing = 1,
        /// The game is of another build.
        OtherBuild = 2,
        /// The game is full.
        Full = 3,
        /// The player has too many introductions under way.
        TooMany = 4,
    }
}

codes! {
    /// An Introduction's hint.
    Hint: 8 bits {
        /// Race the candidates, then ask for the relay.
        Race = 0,
        /// Ask for the relay at once: both routers map a port per
        /// destination, and the host has no other way in.
        RelayNow = 1,
    }
}

codes! {
    /// A Relay offer's result.
    RelayResult: 8 bits {
        /// The channel is open.
        Open = 0,
        /// The host did not answer.
        HostSilent = 1,
        /// The relay is full.
        Full = 2,
        /// The relay's monthly allowance is spent.
        AllowanceSpent = 3,
        /// The relay is switched off.
        Off = 4,
        /// Too many channels from this address.
        TooMany = 5,
    }
}

codes! {
    /// Why a relay channel closed.
    CloseReason: 8 bits {
        /// Closed by an end.
        Closed = 0,
        /// No frame either way for 30 seconds.
        Idle = 1,
        /// Over the channel's rate.
        OverRate = 2,
        /// The month's allowance is spent.
        AllowanceSpent = 3,
        /// The master is stopping.
        Stopping = 4,
    }
}

codes! {
    /// Who sent a Report.
    Role: 2 bits {
        /// A joining player.
        Player = 0,
        /// A game that hosted.
        HostingGame = 1,
        /// A dedicated server.
        DedicatedServer = 2,
    }
}

codes! {
    /// How a player reached its host: the Report's codes, and the game's path
    /// codes in the Challenge answer.
    Path: 3 bits {
        /// The local network.
        LocalNetwork = 0,
        /// A typed address.
        ByAddress = 1,
        /// A router's mapped port.
        MappedPort = 2,
        /// IPv6.
        Ipv6 = 3,
        /// The address the master saw: punched, or forwarded by hand.
        Punched = 4,
        /// The relay.
        Relay = 5,
    }
}

codes! {
    /// What the game's port mapping did.
    PortMapping: 3 bits {
        /// Not tried.
        NotTried = 0,
        /// Mapped with UPnP.
        Upnp = 1,
        /// Mapped with NAT-PMP.
        NatPmp = 2,
        /// Mapped with PCP.
        Pcp = 3,
        /// Tried and failed.
        Failed = 4,
        /// Mapped, but behind a second router.
        SecondRouter = 5,
    }
}

/// Why a datagram was not a master packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasterDecodeError {
    /// Shorter than a header.
    TooShort,
    /// Longer than 1,232 bytes.
    TooLong,
    /// The checksum does not match: another program, or damage. Dropped
    /// silently.
    Checksum,
    /// A good checksum, but a master protocol version outside
    /// [`SUPPORTED_VERSIONS`]: a master answers [`Unsupported`] when that is
    /// no longer than the request. `kind` is the kind byte, unread.
    Unsupported {
        /// The sender's version.
        version: u16,
        /// The kind byte.
        kind: u8,
    },
    /// A kind byte this version does not name.
    UnknownKind(u8),
    /// The checksum matched but the fields break their rules.
    Malformed,
}

impl fmt::Display for MasterDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => f.write_str("datagram shorter than a master packet header"),
            Self::TooLong => f.write_str("datagram longer than 1,232 bytes"),
            Self::Checksum => f.write_str("master checksum mismatch"),
            Self::Unsupported { version, kind } => {
                write!(
                    f,
                    "master protocol version {version} not supported (kind {kind})"
                )
            }
            Self::UnknownKind(kind) => write!(f, "unknown master packet kind {kind}"),
            Self::Malformed => f.write_str("malformed master packet"),
        }
    }
}

impl std::error::Error for MasterDecodeError {}

impl From<CodecError> for MasterDecodeError {
    fn from(_: CodecError) -> Self {
        Self::Malformed
    }
}

/// Why a master packet could not be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasterEncodeError {
    /// Longer than its padded length, 1,232 bytes, or the length allowed.
    TooLarge,
    /// A field outside its limits: a text too long, a callsign that is not 1
    /// to 15 printable ASCII characters, more than 8 candidates or a Seen
    /// one in a sender's own list, more than 255 Page entries or a platform
    /// code over 7 in one, an install id that breaks its rule, a relayed
    /// datagram empty or over 1,200 bytes.
    BadField,
}

impl fmt::Display for MasterEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "master packet too large",
            Self::BadField => "master packet field outside its limits",
        })
    }
}

impl std::error::Error for MasterEncodeError {}

/// The game's build, as the master filters by it: the same three facts the
/// game's build match rule reads.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Build {
    /// The game's protocol version.
    pub protocol_version: u16,
    /// The game version, at most 64 bytes.
    pub game_version: String,
    /// The commit the build stamps, at most 64 bytes.
    pub game_commit: String,
    /// A tagged release build.
    pub release: bool,
}

/// A listed game's summary: the game's discovery answer without its nonce,
/// so the Internet Lobby and Direct Connection show the same facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingSummary {
    /// The host's protocol version.
    pub protocol_version: u16,
    /// A password is needed to join.
    pub password: bool,
    /// No place for another player.
    pub full: bool,
    /// `callsigns` leaves players out.
    pub truncated: bool,
    /// Lobby, flying or closed.
    pub phase: DiscoverPhase,
    /// Players connected.
    pub players: u8,
    /// Players the game seats at most.
    pub capacity: u8,
    /// The host's session id.
    pub session_id: u64,
    /// The game version, at most 64 bytes.
    pub game_version: String,
    /// The commit its build stamps, at most 64 bytes.
    pub game_commit: String,
    /// The game's name, at most 64 bytes.
    pub name: String,
    /// The mission's one-line summary, at most 200 bytes.
    pub mission: String,
    /// The King's callsign, at most 15 bytes; empty on a dedicated server.
    pub king: String,
    /// The players' callsigns, as many as fit.
    pub callsigns: Vec<String>,
}

impl Default for ListingSummary {
    fn default() -> Self {
        Self {
            protocol_version: 0,
            password: false,
            full: false,
            truncated: false,
            phase: DiscoverPhase::Lobby,
            players: 0,
            capacity: 0,
            session_id: 0,
            game_version: String::new(),
            game_commit: String::new(),
            name: String::new(),
            mission: String::new(),
            king: String::new(),
            callsigns: Vec::new(),
        }
    }
}

impl From<DiscoverAnswer> for ListingSummary {
    fn from(answer: DiscoverAnswer) -> Self {
        Self {
            protocol_version: answer.protocol_version,
            password: answer.password,
            full: answer.full,
            truncated: answer.truncated,
            phase: answer.phase,
            players: answer.players,
            capacity: answer.capacity,
            session_id: answer.session_id,
            game_version: answer.game_version,
            game_commit: answer.game_commit,
            name: answer.name,
            mission: answer.summary,
            king: answer.king,
            callsigns: answer.callsigns,
        }
    }
}

impl ListingSummary {
    /// The discovery answer this summary is, with `nonce`.
    pub fn to_discover_answer(&self, nonce: u64) -> DiscoverAnswer {
        DiscoverAnswer {
            nonce,
            protocol_version: self.protocol_version,
            game_version: self.game_version.clone(),
            game_commit: self.game_commit.clone(),
            session_id: self.session_id,
            name: self.name.clone(),
            summary: self.mission.clone(),
            players: self.players,
            capacity: self.capacity,
            password: self.password,
            full: self.full,
            phase: self.phase,
            king: self.king.clone(),
            callsigns: self.callsigns.clone(),
            truncated: self.truncated,
        }
    }

    /// Its width on the wire, in bits.
    pub fn bits(&self) -> usize {
        let texts = [
            &self.game_version,
            &self.game_commit,
            &self.name,
            &self.mission,
            &self.king,
        ];
        16 + 8
            + 8
            + 8
            + 64
            + texts.iter().map(|t| 8 + 8 * t.len()).sum::<usize>()
            + 8
            + self
                .callsigns
                .iter()
                .map(|c| 8 + 8 * c.len())
                .sum::<usize>()
    }

    /// The texts cut to their limits, the callsigns that are not valid
    /// dropped and the list cut to 255, `truncated` set when any were.
    fn cut(mut self) -> Self {
        self.game_version = cut(&self.game_version, MAX_DISCOVER_BUILD);
        self.game_commit = cut(&self.game_commit, MAX_DISCOVER_BUILD);
        self.name = cut(&self.name, MAX_DISCOVER_NAME);
        self.mission = cut(&self.mission, MAX_DISCOVER_SUMMARY);
        self.king = cut(&self.king, MAX_CALLSIGN);
        let listed = self.callsigns.len();
        self.callsigns.retain(|c| valid_callsign(c));
        self.callsigns.truncate(usize::from(u8::MAX));
        self.truncated |= self.callsigns.len() < listed;
        self
    }

    /// Keeps as many callsigns, from the start, as take at most
    /// `available_bits`; `truncated` says when some are left out.
    fn keep_callsigns(mut self, mut available_bits: usize) -> Self {
        let mut keep = 0;
        for callsign in &self.callsigns {
            let bits = 8 + 8 * callsign.len();
            if bits > available_bits {
                break;
            }
            available_bits -= bits;
            keep += 1;
        }
        if keep < self.callsigns.len() {
            self.callsigns.truncate(keep);
            self.truncated = true;
        }
        self
    }

    fn write(&self, w: &mut BitWriter) -> Result<(), MasterEncodeError> {
        if self.callsigns.len() > usize::from(u8::MAX)
            || self.callsigns.iter().any(|c| !valid_callsign(c))
        {
            return Err(MasterEncodeError::BadField);
        }
        put(w, u64::from(self.protocol_version), 16);
        let flags = u64::from(self.password)
            | u64::from(self.full) << 1
            | u64::from(self.truncated) << 2
            | (self.phase as u64) << 3;
        put(w, flags, 8);
        put(w, u64::from(self.players), 8);
        put(w, u64::from(self.capacity), 8);
        put(w, self.session_id, 64);
        put_text(w, &self.game_version, MAX_DISCOVER_BUILD)?;
        put_text(w, &self.game_commit, MAX_DISCOVER_BUILD)?;
        put_text(w, &self.name, MAX_DISCOVER_NAME)?;
        put_text(w, &self.mission, MAX_DISCOVER_SUMMARY)?;
        put_text(w, &self.king, MAX_CALLSIGN)?;
        put(w, self.callsigns.len() as u64, 8);
        for callsign in &self.callsigns {
            put_text(w, callsign, MAX_CALLSIGN)?;
        }
        Ok(())
    }

    fn read(r: &mut BitReader<'_>) -> Result<Self, MasterDecodeError> {
        let protocol_version = get(r, 16)? as u16;
        let flags = get(r, 8)?;
        if flags >> 5 != 0 {
            return Err(MasterDecodeError::Malformed);
        }
        let phase = phase_of((flags >> 3) & 3)?;
        let players = get(r, 8)? as u8;
        let capacity = get(r, 8)? as u8;
        let session_id = get(r, 64)?;
        let game_version = get_text(r, MAX_DISCOVER_BUILD)?;
        let game_commit = get_text(r, MAX_DISCOVER_BUILD)?;
        let name = get_text(r, MAX_DISCOVER_NAME)?;
        let mission = get_text(r, MAX_DISCOVER_SUMMARY)?;
        let king = get_text(r, MAX_CALLSIGN)?;
        let count = get(r, 8)? as usize;
        let mut callsigns = Vec::with_capacity(count);
        for _ in 0..count {
            let callsign = r.read_str()?;
            if !valid_callsign(&callsign) {
                return Err(MasterDecodeError::Malformed);
            }
            callsigns.push(callsign);
        }
        Ok(Self {
            protocol_version,
            password: flags & 1 != 0,
            full: flags & 2 != 0,
            truncated: flags & 4 != 0,
            phase,
            players,
            capacity,
            session_id,
            game_version,
            game_commit,
            name,
            mission,
            king,
            callsigns,
        })
    }
}

/// Challenge (kind 1): the cookie a sender repeats its request with. 23
/// bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Challenge {
    /// The request's nonce.
    pub nonce: u64,
    /// The cookie.
    pub cookie: u64,
}

/// Unsupported (kind 2): the master does not speak the sender's version.
/// Sent in the sender's version, and only when no longer than the packet it
/// answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    /// The lowest version the master supports.
    pub lowest: u16,
    /// The highest.
    pub highest: u16,
    /// A plain text for the player, at most 100 bytes.
    pub text: String,
}

/// Register (kind 3), padded to 1,200 bytes: list my game. Call
/// [`Register::fit`] before sending, so the summary fits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Register {
    /// The host's random nonce, kept for this listing.
    pub nonce: u64,
    /// 0 the first time; the Challenge's cookie after.
    pub cookie: u64,
    /// The host's build.
    pub build: Build,
    /// A dedicated server.
    pub dedicated: bool,
    /// Telemetry is on: then `install_id` is not 0, else it is 0.
    pub telemetry: bool,
    /// The anonymous install id when telemetry is on, else 0.
    pub install_id: u64,
    /// The platform code, as the Challenge answer codes it. Kept as the raw
    /// code so a master reads a later build that names a platform this one
    /// does not ([`crate::Platform::from_code`] reads it).
    pub platform: u8,
    /// Local, Mapped and Global IPv6, as the host knows them; never Seen.
    pub candidates: Vec<Candidate>,
    /// The lobby's summary.
    pub summary: ListingSummary,
}

impl Register {
    /// The summary cut to fit the packet: its texts to their limits, then
    /// callsigns from the end, with the truncated flag set.
    pub fn fit(self) -> Self {
        fit_ending(
            self,
            REGISTER_LEN,
            |p| &mut p.summary,
            MasterPacket::Register,
        )
    }
}

/// Listed (kind 4): the listing exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listed {
    /// The Register's nonce.
    pub nonce: u64,
    /// Public: what browsers name the listing by.
    pub listing_id: u64,
    /// Secret: what the host proves itself with.
    pub token: u64,
    /// The host's address as the master saw it.
    pub seen: SocketAddr,
    /// Seconds between Heartbeats (30).
    pub heartbeat_secs: u8,
    /// Seconds between Keeps (15).
    pub keep_secs: u8,
    /// Seconds of silence after which the listing is dropped (90).
    pub expiry_secs: u8,
}

/// Heartbeat (kind 5): the listing's summary. Not padded. Call
/// [`Heartbeat::fit`] before sending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heartbeat {
    /// The listing's token.
    pub token: u64,
    /// Raised when the summary changes.
    pub change: u16,
    /// As in Register.
    pub candidates: Vec<Candidate>,
    /// The lobby's summary.
    pub summary: ListingSummary,
}

impl Heartbeat {
    /// The summary cut to fit the longest master datagram, as
    /// [`Register::fit`].
    pub fn fit(self) -> Self {
        fit_ending(
            self,
            MAX_MASTER_DATAGRAM,
            |p| &mut p.summary,
            MasterPacket::Heartbeat,
        )
    }
}

/// Heartbeat ack (kind 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeartbeatAck {
    /// The listing's public id.
    pub listing_id: u64,
    /// The host's address as the master saw it.
    pub seen: SocketAddr,
}

/// Keep (kind 7): keeps the router's mapping open. 15 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keep {
    /// The listing's token.
    pub token: u64,
}

/// Unknown listing (kind 8): register again. 15 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownListing {
    /// The token that was not known.
    pub token: u64,
}

/// Unregister (kind 9): remove the listing at once. 15 bytes, sent three
/// times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unregister {
    /// The listing's token.
    pub token: u64,
}

/// Browse (kind 10), padded to 1,200 bytes: a page of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Browse {
    /// The asker's random nonce.
    pub nonce: u64,
    /// The asker's build.
    pub build: Build,
    /// Include games of other builds.
    pub other_builds: bool,
    /// Include full games.
    pub full_games: bool,
    /// 0 for the first page; then the Page's next cursor.
    pub cursor: u32,
}

/// One game in a Page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageEntry {
    /// What Details and Introduce name.
    pub listing_id: u64,
    /// A password is needed.
    pub password: bool,
    /// No place for another player.
    pub full: bool,
    /// A dedicated server.
    pub dedicated: bool,
    /// The game version of a game of another build, at most 64 bytes; `None`
    /// for the asker's own build.
    pub other_build: Option<String>,
    /// The relay is likely needed ([`candidate::relay_likely`]).
    pub relay_likely: bool,
    /// Lobby, flying or closed.
    pub phase: DiscoverPhase,
    /// Players connected.
    pub players: u8,
    /// Players the game seats at most.
    pub capacity: u8,
    /// The host's platform code, at most 7.
    pub platform: u8,
    /// The game's name, at most 64 bytes.
    pub name: String,
}

impl PageEntry {
    /// Its width on the wire, in bits.
    pub fn bits(&self) -> usize {
        64 + 8
            + 2
            + 8
            + 8
            + 3
            + self.other_build.as_ref().map_or(0, |v| 8 + 8 * v.len())
            + 8
            + 8 * self.name.len()
    }
}

/// Page (kind 11): listings, never longer than the Browse. Call
/// [`Page::fit`], then set `next_cursor` from the entries it kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// The Browse's nonce.
    pub nonce: u64,
    /// How many listings match (the master stops counting at 200).
    pub matching: u16,
    /// The next page's cursor; 0 on the last page.
    pub next_cursor: u32,
    /// The listings, in the master's order.
    pub entries: Vec<PageEntry>,
}

impl Page {
    /// The bits of a Page with no entries.
    const BASE_BITS: usize = HEADER_LEN * 8 + 64 + 16 + 32 + 8;

    /// The page cut to `max_len` bytes: each entry's texts to their limits,
    /// then as many entries from the start as fit (and at most 255). The
    /// master then sets `next_cursor` from how many it kept.
    pub fn fit(mut self, max_len: usize) -> Self {
        let mut available = (max_len * 8).saturating_sub(Self::BASE_BITS);
        let mut keep = 0;
        for entry in self.entries.iter_mut().take(MAX_PAGE_ENTRIES) {
            entry.name = cut(&entry.name, MAX_DISCOVER_NAME);
            if let Some(version) = &mut entry.other_build {
                *version = cut(version, MAX_BUILD_TEXT);
            }
            let bits = entry.bits();
            if bits > available {
                break;
            }
            available -= bits;
            keep += 1;
        }
        self.entries.truncate(keep);
        self
    }
}

/// Details (kind 12), padded to 1,000 bytes: one listing's summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Details {
    /// The asker's random nonce.
    pub nonce: u64,
    /// The listing.
    pub listing_id: u64,
}

/// Listing details (kind 13): never longer than the Details. Call
/// [`ListingDetails::fit`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingDetails {
    /// The Details' nonce.
    pub nonce: u64,
    /// The listing asked for.
    pub listing_id: u64,
    /// Its summary; `None` when there is no such listing.
    pub summary: Option<ListingSummary>,
}

impl ListingDetails {
    /// The summary cut to fit `max_len` bytes (the request's length), as
    /// [`Register::fit`].
    pub fn fit(self, max_len: usize) -> Self {
        let Self {
            nonce,
            listing_id,
            summary,
        } = self;
        let Some(summary) = summary else {
            return Self {
                nonce,
                listing_id,
                summary: None,
            };
        };
        #[derive(Clone)]
        struct Found(u64, u64, ListingSummary);
        let found = fit_ending(
            Found(nonce, listing_id, summary),
            max_len,
            |f| &mut f.2,
            |f| {
                MasterPacket::ListingDetails(ListingDetails {
                    nonce: f.0,
                    listing_id: f.1,
                    summary: Some(f.2),
                })
            },
        );
        Self {
            nonce,
            listing_id,
            summary: Some(found.2),
        }
    }
}

/// Probe (kind 14), padded to 64 bytes: the mapping test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe {
    /// The prober's random nonce.
    pub nonce: u64,
}

/// Probe answer (kind 15): at most 35 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeAnswer {
    /// The Probe's nonce.
    pub nonce: u64,
    /// The port the Probe arrived at.
    pub port: ProbePort,
    /// The prober's address as the master saw it.
    pub seen: SocketAddr,
}

/// Introduce (kind 16), padded to 1,000 bytes: introduce me to a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Introduce {
    /// The player's random nonce.
    pub nonce: u64,
    /// 0 the first time; the Challenge's cookie after.
    pub cookie: u64,
    /// The game chosen.
    pub listing_id: u64,
    /// The player's build.
    pub build: Build,
    /// From the player's mapping test.
    pub mapping: MappingType,
    /// Local and Global IPv6, as the player knows them; never Seen.
    pub candidates: Vec<Candidate>,
}

/// Introduction (kind 17): the host's addresses, or why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Introduction {
    /// The Introduce's nonce.
    pub nonce: u64,
    /// Introduced, or why not.
    pub result: IntroductionResult,
    /// Names this introduction; the host's Punch packets carry it.
    pub introduction_id: u64,
    /// Race first, or ask for the relay at once.
    pub hint: Hint,
    /// The player's address as the master saw it.
    pub seen: SocketAddr,
    /// The host's mapping type.
    pub host_mapping: MappingType,
    /// Seen first, then Mapped, Global IPv6 and Local.
    pub host_candidates: Vec<Candidate>,
    /// For a refusal, the plain reason the player reads; at most 200 bytes.
    pub text: String,
}

/// Meet (kind 18): a player's addresses, to the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meet {
    /// The introduction.
    pub introduction_id: u64,
    /// The player's mapping type.
    pub mapping: MappingType,
    /// Seen first, then Global IPv6 and Local.
    pub candidates: Vec<Candidate>,
}

/// Meet ack (kind 19). 23 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeetAck {
    /// The host's listing token.
    pub token: u64,
    /// The introduction.
    pub introduction_id: u64,
}

/// Relay request (kind 20): open a channel. 23 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayRequest {
    /// The Introduce's nonce.
    pub nonce: u64,
    /// The introduction.
    pub introduction_id: u64,
}

/// Relay offer (kind 21): the channel and its key, or why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayOffer {
    /// The Introduce's nonce.
    pub nonce: u64,
    /// The introduction.
    pub introduction_id: u64,
    /// Open, or why not.
    pub result: RelayResult,
    /// The channel.
    pub channel: u32,
    /// The channel's key.
    pub key: u32,
    /// For a refusal, the plain reason; at most 200 bytes.
    pub text: String,
}

/// Relay open (kind 22): a channel opens, to the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayOpen {
    /// The introduction.
    pub introduction_id: u64,
    /// The channel.
    pub channel: u32,
    /// The channel's key.
    pub key: u32,
    /// The player's address as the master saw it.
    pub player: SocketAddr,
}

/// Relay open ack (kind 23). 19 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayOpenAck {
    /// The host's listing token.
    pub token: u64,
    /// The channel.
    pub channel: u32,
}

/// Relay (kind 24): one game datagram through a channel. 15 bytes of
/// overhead. [`RelayFrame`] reads one without copying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relay {
    /// The channel.
    pub channel: u32,
    /// The channel's key.
    pub key: u32,
    /// The game's datagram, 1 to 1,200 bytes, unchanged.
    pub datagram: Vec<u8>,
}

/// Relay close (kind 25): a channel closes. 16 bytes, sent three times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayClose {
    /// The channel.
    pub channel: u32,
    /// The channel's key.
    pub key: u32,
    /// Why.
    pub reason: CloseReason,
}

/// Report (kind 26): anonymous telemetry. Never answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The anonymous install id; never 0.
    pub install_id: u64,
    /// Who sends it.
    pub role: Role,
    /// The game version, at most 64 bytes.
    pub game_version: String,
    /// The platform code, kept raw as in [`Register`].
    pub platform: u8,
    /// How long the session lasted.
    pub minutes: u16,
    /// The most humans in it at once.
    pub humans: u8,
    /// How a player connected.
    pub path: Path,
    /// From asking for the introduction to the handshake's end, in tenths of
    /// a second (at most 25.5 s).
    pub connect_tenths: u8,
    /// The game's own mapping type.
    pub mapping: MappingType,
    /// What the game's port mapping did.
    pub port_mapping: PortMapping,
    /// A player's traffic through the relay, in kilobytes.
    pub relayed_kb: u32,
    /// A host's or server's players by their path, in [`Path`] code order.
    pub players_by_path: [u8; 6],
    /// Host migrations in the session (stage K; 0 until then).
    pub migrations: u8,
    /// How many of them failed.
    pub failed_migrations: u8,
}

/// A Relay frame read in place: the master forwards it unchanged, and a
/// game's router hands the datagram to its transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayFrame<'a> {
    /// The channel.
    pub channel: u32,
    /// The channel's key.
    pub key: u32,
    /// The game's datagram, 1 to 1,200 bytes.
    pub datagram: &'a [u8],
}

impl<'a> RelayFrame<'a> {
    /// Checks a datagram as [`MasterPacket::decode`] does and reads it as a
    /// Relay frame, without copying the game's datagram. A good packet of
    /// another kind is [`MasterDecodeError::Malformed`]; look at
    /// [`peek_kind`] first.
    pub fn open(frame: &'a [u8]) -> Result<Self, MasterDecodeError> {
        let (kind, version, body) = open(frame)?;
        if !SUPPORTED_VERSIONS.contains(&version) {
            return Err(MasterDecodeError::Unsupported { version, kind });
        }
        if kind != MasterKind::Relay.code() {
            return Err(MasterDecodeError::Malformed);
        }
        let datagram = &frame[RELAY_HEADER_LEN.min(frame.len())..];
        if body.len() < 8 || !(1..=MAX_RELAYED).contains(&datagram.len()) {
            return Err(MasterDecodeError::Malformed);
        }
        Ok(Self {
            channel: u32::from_le_bytes([body[0], body[1], body[2], body[3]]),
            key: u32::from_le_bytes([body[4], body[5], body[6], body[7]]),
            datagram,
        })
    }

    /// Writes the frame, as [`MasterPacket::encode`] writes a [`Relay`].
    pub fn encode(&self) -> Result<Vec<u8>, MasterEncodeError> {
        if !(1..=MAX_RELAYED).contains(&self.datagram.len()) {
            return Err(MasterEncodeError::BadField);
        }
        let mut bytes = Vec::with_capacity(RELAY_HEADER_LEN + self.datagram.len());
        bytes.extend_from_slice(&[0; 4]);
        bytes.push(MasterKind::Relay.code());
        bytes.extend_from_slice(&MASTER_VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.channel.to_le_bytes());
        bytes.extend_from_slice(&self.key.to_le_bytes());
        bytes.extend_from_slice(self.datagram);
        seal(&mut bytes);
        Ok(bytes)
    }
}

/// The kind a datagram claims, without checking anything else: how a router
/// picks the Relay fast path before decoding.
pub fn peek_kind(datagram: &[u8]) -> Option<MasterKind> {
    datagram
        .get(4)
        .and_then(|&kind| MasterKind::from_code(u64::from(kind)))
}

/// Any master packet, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum MasterPacket {
    Challenge(Challenge),
    Unsupported(Unsupported),
    Register(Register),
    Listed(Listed),
    Heartbeat(Heartbeat),
    HeartbeatAck(HeartbeatAck),
    Keep(Keep),
    UnknownListing(UnknownListing),
    Unregister(Unregister),
    Browse(Browse),
    Page(Page),
    Details(Details),
    ListingDetails(ListingDetails),
    Probe(Probe),
    ProbeAnswer(ProbeAnswer),
    Introduce(Introduce),
    Introduction(Introduction),
    Meet(Meet),
    MeetAck(MeetAck),
    RelayRequest(RelayRequest),
    RelayOffer(RelayOffer),
    RelayOpen(RelayOpen),
    RelayOpenAck(RelayOpenAck),
    Relay(Relay),
    RelayClose(RelayClose),
    Report(Report),
}

impl MasterPacket {
    /// This packet's kind.
    pub fn kind(&self) -> MasterKind {
        match self {
            Self::Challenge(_) => MasterKind::Challenge,
            Self::Unsupported(_) => MasterKind::Unsupported,
            Self::Register(_) => MasterKind::Register,
            Self::Listed(_) => MasterKind::Listed,
            Self::Heartbeat(_) => MasterKind::Heartbeat,
            Self::HeartbeatAck(_) => MasterKind::HeartbeatAck,
            Self::Keep(_) => MasterKind::Keep,
            Self::UnknownListing(_) => MasterKind::UnknownListing,
            Self::Unregister(_) => MasterKind::Unregister,
            Self::Browse(_) => MasterKind::Browse,
            Self::Page(_) => MasterKind::Page,
            Self::Details(_) => MasterKind::Details,
            Self::ListingDetails(_) => MasterKind::ListingDetails,
            Self::Probe(_) => MasterKind::Probe,
            Self::ProbeAnswer(_) => MasterKind::ProbeAnswer,
            Self::Introduce(_) => MasterKind::Introduce,
            Self::Introduction(_) => MasterKind::Introduction,
            Self::Meet(_) => MasterKind::Meet,
            Self::MeetAck(_) => MasterKind::MeetAck,
            Self::RelayRequest(_) => MasterKind::RelayRequest,
            Self::RelayOffer(_) => MasterKind::RelayOffer,
            Self::RelayOpen(_) => MasterKind::RelayOpen,
            Self::RelayOpenAck(_) => MasterKind::RelayOpenAck,
            Self::Relay(_) => MasterKind::Relay,
            Self::RelayClose(_) => MasterKind::RelayClose,
            Self::Report(_) => MasterKind::Report,
        }
    }

    /// Writes the packet with its checksum, in [`MASTER_VERSION`], padded
    /// when its kind is.
    pub fn encode(&self) -> Result<Vec<u8>, MasterEncodeError> {
        self.encode_in(MASTER_VERSION)
    }

    /// Writes the packet with `version` in its header. A master answers in
    /// the version the request came in; with one version that is
    /// [`MASTER_VERSION`], apart from an [`Unsupported`], whose layout no
    /// version changes.
    pub fn encode_in(&self, version: u16) -> Result<Vec<u8>, MasterEncodeError> {
        let mut bytes = self.write(version)?.finish();
        let len = self.kind().padded_len().unwrap_or(bytes.len());
        if bytes.len() > len || len > MAX_MASTER_DATAGRAM {
            return Err(MasterEncodeError::TooLarge);
        }
        bytes.resize(len, 0);
        seal(&mut bytes);
        Ok(bytes)
    }

    /// Writes the packet as [`MasterPacket::encode_in`], and refuses one
    /// longer than `max_len` bytes ([`MasterEncodeError::TooLarge`]): how the
    /// master keeps an answer within the request it answers.
    pub fn encode_within(
        &self,
        version: u16,
        max_len: usize,
    ) -> Result<Vec<u8>, MasterEncodeError> {
        let bytes = self.encode_in(version)?;
        if bytes.len() > max_len {
            return Err(MasterEncodeError::TooLarge);
        }
        Ok(bytes)
    }

    /// The packet's length in bits before any padding, the header included.
    pub fn content_bits(&self) -> Result<usize, MasterEncodeError> {
        Ok(self.write(MASTER_VERSION)?.bit_len())
    }

    /// The header and the fields, unpadded and unsealed.
    fn write(&self, version: u16) -> Result<BitWriter, MasterEncodeError> {
        let mut w = BitWriter::with_capacity(64);
        put(&mut w, 0, 32);
        put(&mut w, u64::from(self.kind().code()), 8);
        put(&mut w, u64::from(version), 16);
        self.write_body(&mut w)?;
        Ok(w)
    }

    fn write_body(&self, w: &mut BitWriter) -> Result<(), MasterEncodeError> {
        match self {
            Self::Challenge(p) => {
                put(w, p.nonce, 64);
                put(w, p.cookie, 64);
            }
            Self::Unsupported(p) => {
                put(w, u64::from(p.lowest), 16);
                put(w, u64::from(p.highest), 16);
                put_text(w, &p.text, MAX_UNSUPPORTED_TEXT)?;
            }
            Self::Register(p) => {
                if p.telemetry != (p.install_id != 0) {
                    return Err(MasterEncodeError::BadField);
                }
                put(w, p.nonce, 64);
                put(w, p.cookie, 64);
                write_build(w, &p.build)?;
                put(w, u64::from(p.dedicated) | u64::from(p.telemetry) << 1, 8);
                put(w, p.install_id, 64);
                put(w, u64::from(p.platform), 8);
                candidate::write_list(w, &p.candidates, true)?;
                p.summary.write(w)?;
            }
            Self::Listed(p) => {
                put(w, p.nonce, 64);
                put(w, p.listing_id, 64);
                put(w, p.token, 64);
                candidate::write_address(w, p.seen);
                put(w, u64::from(p.heartbeat_secs), 8);
                put(w, u64::from(p.keep_secs), 8);
                put(w, u64::from(p.expiry_secs), 8);
            }
            Self::Heartbeat(p) => {
                put(w, p.token, 64);
                put(w, u64::from(p.change), 16);
                candidate::write_list(w, &p.candidates, true)?;
                p.summary.write(w)?;
            }
            Self::HeartbeatAck(p) => {
                put(w, p.listing_id, 64);
                candidate::write_address(w, p.seen);
            }
            Self::Keep(Keep { token })
            | Self::UnknownListing(UnknownListing { token })
            | Self::Unregister(Unregister { token }) => put(w, *token, 64),
            Self::Browse(p) => {
                put(w, p.nonce, 64);
                write_build(w, &p.build)?;
                put(
                    w,
                    u64::from(p.other_builds) | u64::from(p.full_games) << 1,
                    8,
                );
                put(w, u64::from(p.cursor), 32);
            }
            Self::Page(p) => {
                if p.entries.len() > MAX_PAGE_ENTRIES {
                    return Err(MasterEncodeError::BadField);
                }
                put(w, p.nonce, 64);
                put(w, u64::from(p.matching), 16);
                put(w, u64::from(p.next_cursor), 32);
                put(w, p.entries.len() as u64, 8);
                for entry in &p.entries {
                    write_entry(w, entry)?;
                }
            }
            Self::Details(p) => {
                put(w, p.nonce, 64);
                put(w, p.listing_id, 64);
            }
            Self::ListingDetails(p) => {
                put(w, p.nonce, 64);
                put(w, p.listing_id, 64);
                put(w, u64::from(p.summary.is_some()), 8);
                if let Some(summary) = &p.summary {
                    summary.write(w)?;
                }
            }
            Self::Probe(p) => put(w, p.nonce, 64),
            Self::ProbeAnswer(p) => {
                put(w, p.nonce, 64);
                put(w, u64::from(p.port.code()), ProbePort::BITS);
                candidate::write_address(w, p.seen);
            }
            Self::Introduce(p) => {
                put(w, p.nonce, 64);
                put(w, p.cookie, 64);
                put(w, p.listing_id, 64);
                write_build(w, &p.build)?;
                put(w, u64::from(p.mapping.code()), 8);
                candidate::write_list(w, &p.candidates, true)?;
            }
            Self::Introduction(p) => {
                put(w, p.nonce, 64);
                put(w, u64::from(p.result.code()), IntroductionResult::BITS);
                put(w, p.introduction_id, 64);
                put(w, u64::from(p.hint.code()), Hint::BITS);
                candidate::write_address(w, p.seen);
                put(w, u64::from(p.host_mapping.code()), 8);
                candidate::write_list(w, &p.host_candidates, false)?;
                put_text(w, &p.text, MAX_TEXT)?;
            }
            Self::Meet(p) => {
                put(w, p.introduction_id, 64);
                put(w, u64::from(p.mapping.code()), 8);
                candidate::write_list(w, &p.candidates, false)?;
            }
            Self::MeetAck(p) => {
                put(w, p.token, 64);
                put(w, p.introduction_id, 64);
            }
            Self::RelayRequest(p) => {
                put(w, p.nonce, 64);
                put(w, p.introduction_id, 64);
            }
            Self::RelayOffer(p) => {
                put(w, p.nonce, 64);
                put(w, p.introduction_id, 64);
                put(w, u64::from(p.result.code()), RelayResult::BITS);
                put(w, u64::from(p.channel), 32);
                put(w, u64::from(p.key), 32);
                put_text(w, &p.text, MAX_TEXT)?;
            }
            Self::RelayOpen(p) => {
                put(w, p.introduction_id, 64);
                put(w, u64::from(p.channel), 32);
                put(w, u64::from(p.key), 32);
                candidate::write_address(w, p.player);
            }
            Self::RelayOpenAck(p) => {
                put(w, p.token, 64);
                put(w, u64::from(p.channel), 32);
            }
            Self::Relay(p) => {
                if !(1..=MAX_RELAYED).contains(&p.datagram.len()) {
                    return Err(MasterEncodeError::BadField);
                }
                put(w, u64::from(p.channel), 32);
                put(w, u64::from(p.key), 32);
                w.write_bytes(&p.datagram);
            }
            Self::RelayClose(p) => {
                put(w, u64::from(p.channel), 32);
                put(w, u64::from(p.key), 32);
                put(w, u64::from(p.reason.code()), CloseReason::BITS);
            }
            Self::Report(p) => {
                if p.install_id == 0 {
                    return Err(MasterEncodeError::BadField);
                }
                put(w, p.install_id, 64);
                put(w, u64::from(p.role.code()), Role::BITS);
                put_text(w, &p.game_version, MAX_BUILD_TEXT)?;
                put(w, u64::from(p.platform), 8);
                put(w, u64::from(p.minutes), 16);
                put(w, u64::from(p.humans), 8);
                put(w, u64::from(p.path.code()), Path::BITS);
                put(w, u64::from(p.connect_tenths), 8);
                put(w, u64::from(p.mapping.code()), MappingType::BITS);
                put(w, u64::from(p.port_mapping.code()), PortMapping::BITS);
                put(w, u64::from(p.relayed_kb), 32);
                for count in p.players_by_path {
                    put(w, u64::from(count), 8);
                }
                put(w, u64::from(p.migrations), 8);
                put(w, u64::from(p.failed_migrations), 8);
            }
        }
        Ok(())
    }

    /// Checks and decodes a datagram. An [`Unsupported`] is read whatever
    /// its version; any other kind in a version outside
    /// [`SUPPORTED_VERSIONS`] is [`MasterDecodeError::Unsupported`].
    pub fn decode(datagram: &[u8]) -> Result<Self, MasterDecodeError> {
        let (kind_byte, version, body) = open(datagram)?;
        let unsupported = kind_byte == MasterKind::Unsupported.code();
        if !unsupported && !SUPPORTED_VERSIONS.contains(&version) {
            return Err(MasterDecodeError::Unsupported {
                version,
                kind: kind_byte,
            });
        }
        let kind = MasterKind::from_code(u64::from(kind_byte))
            .ok_or(MasterDecodeError::UnknownKind(kind_byte))?;
        if let Some(len) = kind.padded_len()
            && datagram.len() != len
        {
            return Err(MasterDecodeError::Malformed);
        }
        if kind == MasterKind::Relay {
            let frame = RelayFrame::open(datagram)?;
            return Ok(Self::Relay(Relay {
                channel: frame.channel,
                key: frame.key,
                datagram: frame.datagram.to_vec(),
            }));
        }
        let mut r = BitReader::new(body);
        let packet = Self::read_body(kind, &mut r)?;
        let rest_is_padding = r.only_zero_padding_left();
        let ends = kind.padded_len().is_some() || r.bits_remaining() < 8;
        if !(rest_is_padding && ends) {
            return Err(MasterDecodeError::Malformed);
        }
        Ok(packet)
    }

    fn read_body(kind: MasterKind, r: &mut BitReader<'_>) -> Result<Self, MasterDecodeError> {
        Ok(match kind {
            MasterKind::Challenge => Self::Challenge(Challenge {
                nonce: get(r, 64)?,
                cookie: get(r, 64)?,
            }),
            MasterKind::Unsupported => Self::Unsupported(Unsupported {
                lowest: get(r, 16)? as u16,
                highest: get(r, 16)? as u16,
                text: get_text(r, MAX_UNSUPPORTED_TEXT)?,
            }),
            MasterKind::Register => {
                let nonce = get(r, 64)?;
                let cookie = get(r, 64)?;
                let build = read_build(r)?;
                let flags = get(r, 8)?;
                if flags >> 2 != 0 {
                    return Err(MasterDecodeError::Malformed);
                }
                let telemetry = flags & 2 != 0;
                let install_id = get(r, 64)?;
                if telemetry != (install_id != 0) {
                    return Err(MasterDecodeError::Malformed);
                }
                Self::Register(Register {
                    nonce,
                    cookie,
                    build,
                    dedicated: flags & 1 != 0,
                    telemetry,
                    install_id,
                    platform: get(r, 8)? as u8,
                    candidates: candidate::read_list(r, true)?,
                    summary: ListingSummary::read(r)?,
                })
            }
            MasterKind::Listed => Self::Listed(Listed {
                nonce: get(r, 64)?,
                listing_id: get(r, 64)?,
                token: get(r, 64)?,
                seen: candidate::read_address(r)?,
                heartbeat_secs: get(r, 8)? as u8,
                keep_secs: get(r, 8)? as u8,
                expiry_secs: get(r, 8)? as u8,
            }),
            MasterKind::Heartbeat => Self::Heartbeat(Heartbeat {
                token: get(r, 64)?,
                change: get(r, 16)? as u16,
                candidates: candidate::read_list(r, true)?,
                summary: ListingSummary::read(r)?,
            }),
            MasterKind::HeartbeatAck => Self::HeartbeatAck(HeartbeatAck {
                listing_id: get(r, 64)?,
                seen: candidate::read_address(r)?,
            }),
            MasterKind::Keep => Self::Keep(Keep { token: get(r, 64)? }),
            MasterKind::UnknownListing => {
                Self::UnknownListing(UnknownListing { token: get(r, 64)? })
            }
            MasterKind::Unregister => Self::Unregister(Unregister { token: get(r, 64)? }),
            MasterKind::Browse => {
                let nonce = get(r, 64)?;
                let build = read_build(r)?;
                let filters = get(r, 8)?;
                if filters >> 2 != 0 {
                    return Err(MasterDecodeError::Malformed);
                }
                Self::Browse(Browse {
                    nonce,
                    build,
                    other_builds: filters & 1 != 0,
                    full_games: filters & 2 != 0,
                    cursor: get(r, 32)? as u32,
                })
            }
            MasterKind::Page => {
                let nonce = get(r, 64)?;
                let matching = get(r, 16)? as u16;
                let next_cursor = get(r, 32)? as u32;
                let count = get(r, 8)? as usize;
                let mut entries = Vec::with_capacity(count);
                for _ in 0..count {
                    entries.push(read_entry(r)?);
                }
                Self::Page(Page {
                    nonce,
                    matching,
                    next_cursor,
                    entries,
                })
            }
            MasterKind::Details => Self::Details(Details {
                nonce: get(r, 64)?,
                listing_id: get(r, 64)?,
            }),
            MasterKind::ListingDetails => {
                let nonce = get(r, 64)?;
                let listing_id = get(r, 64)?;
                let found = get(r, 8)?;
                let summary = match found {
                    0 => None,
                    1 => Some(ListingSummary::read(r)?),
                    _ => return Err(MasterDecodeError::Malformed),
                };
                Self::ListingDetails(ListingDetails {
                    nonce,
                    listing_id,
                    summary,
                })
            }
            MasterKind::Probe => Self::Probe(Probe { nonce: get(r, 64)? }),
            MasterKind::ProbeAnswer => Self::ProbeAnswer(ProbeAnswer {
                nonce: get(r, 64)?,
                port: code(r, ProbePort::BITS, ProbePort::from_code)?,
                seen: candidate::read_address(r)?,
            }),
            MasterKind::Introduce => Self::Introduce(Introduce {
                nonce: get(r, 64)?,
                cookie: get(r, 64)?,
                listing_id: get(r, 64)?,
                build: read_build(r)?,
                mapping: code(r, 8, MappingType::from_code)?,
                candidates: candidate::read_list(r, true)?,
            }),
            MasterKind::Introduction => Self::Introduction(Introduction {
                nonce: get(r, 64)?,
                result: code(r, IntroductionResult::BITS, IntroductionResult::from_code)?,
                introduction_id: get(r, 64)?,
                hint: code(r, Hint::BITS, Hint::from_code)?,
                seen: candidate::read_address(r)?,
                host_mapping: code(r, 8, MappingType::from_code)?,
                host_candidates: candidate::read_list(r, false)?,
                text: get_text(r, MAX_TEXT)?,
            }),
            MasterKind::Meet => Self::Meet(Meet {
                introduction_id: get(r, 64)?,
                mapping: code(r, 8, MappingType::from_code)?,
                candidates: candidate::read_list(r, false)?,
            }),
            MasterKind::MeetAck => Self::MeetAck(MeetAck {
                token: get(r, 64)?,
                introduction_id: get(r, 64)?,
            }),
            MasterKind::RelayRequest => Self::RelayRequest(RelayRequest {
                nonce: get(r, 64)?,
                introduction_id: get(r, 64)?,
            }),
            MasterKind::RelayOffer => Self::RelayOffer(RelayOffer {
                nonce: get(r, 64)?,
                introduction_id: get(r, 64)?,
                result: code(r, RelayResult::BITS, RelayResult::from_code)?,
                channel: get(r, 32)? as u32,
                key: get(r, 32)? as u32,
                text: get_text(r, MAX_TEXT)?,
            }),
            MasterKind::RelayOpen => Self::RelayOpen(RelayOpen {
                introduction_id: get(r, 64)?,
                channel: get(r, 32)? as u32,
                key: get(r, 32)? as u32,
                player: candidate::read_address(r)?,
            }),
            MasterKind::RelayOpenAck => Self::RelayOpenAck(RelayOpenAck {
                token: get(r, 64)?,
                channel: get(r, 32)? as u32,
            }),
            // Read in place by `RelayFrame::open`.
            MasterKind::Relay => return Err(MasterDecodeError::Malformed),
            MasterKind::RelayClose => Self::RelayClose(RelayClose {
                channel: get(r, 32)? as u32,
                key: get(r, 32)? as u32,
                reason: code(r, CloseReason::BITS, CloseReason::from_code)?,
            }),
            MasterKind::Report => {
                let install_id = get(r, 64)?;
                if install_id == 0 {
                    return Err(MasterDecodeError::Malformed);
                }
                let role = code(r, Role::BITS, Role::from_code)?;
                let game_version = get_text(r, MAX_BUILD_TEXT)?;
                let platform = get(r, 8)? as u8;
                let minutes = get(r, 16)? as u16;
                let humans = get(r, 8)? as u8;
                let path = code(r, Path::BITS, Path::from_code)?;
                let connect_tenths = get(r, 8)? as u8;
                let mapping = code(r, MappingType::BITS, MappingType::from_code)?;
                let port_mapping = code(r, PortMapping::BITS, PortMapping::from_code)?;
                let relayed_kb = get(r, 32)? as u32;
                let mut players_by_path = [0u8; 6];
                for count in &mut players_by_path {
                    *count = get(r, 8)? as u8;
                }
                Self::Report(Report {
                    install_id,
                    role,
                    game_version,
                    platform,
                    minutes,
                    humans,
                    path,
                    connect_tenths,
                    mapping,
                    port_mapping,
                    relayed_kb,
                    players_by_path,
                    migrations: get(r, 8)? as u8,
                    failed_migrations: get(r, 8)? as u8,
                })
            }
        })
    }
}

/// The checksum of a master packet: CRC-32 over `TORE-MASTER`, then `rest`,
/// which starts at the kind byte.
pub fn checksum(rest: &[u8]) -> u32 {
    let mut crc = Crc32::new();
    crc.update(&MASTER_ID);
    crc.update(rest);
    crc.finalize()
}

/// Fills in the checksum of a packet whose first four bytes are reserved for
/// it.
fn seal(bytes: &mut [u8]) {
    let crc = checksum(&bytes[4..]);
    bytes[..4].copy_from_slice(&crc.to_le_bytes());
}

/// Checks a datagram's size and checksum. Returns the kind byte, the
/// version and the bytes after the header.
fn open(datagram: &[u8]) -> Result<(u8, u16, &[u8]), MasterDecodeError> {
    if datagram.len() < HEADER_LEN {
        return Err(MasterDecodeError::TooShort);
    }
    if datagram.len() > MAX_MASTER_DATAGRAM {
        return Err(MasterDecodeError::TooLong);
    }
    let sent = u32::from_le_bytes([datagram[0], datagram[1], datagram[2], datagram[3]]);
    if sent != checksum(&datagram[4..]) {
        return Err(MasterDecodeError::Checksum);
    }
    let version = u16::from_le_bytes([datagram[5], datagram[6]]);
    Ok((datagram[4], version, &datagram[HEADER_LEN..]))
}

/// Fits the summary that ends a packet to `max_len` bytes: its texts cut,
/// then as many callsigns as fit. `summary` reaches the summary inside the
/// packet; `wrap` makes the packet to measure.
fn fit_ending<T: Clone>(
    mut packet: T,
    max_len: usize,
    summary: fn(&mut T) -> &mut ListingSummary,
    wrap: fn(T) -> MasterPacket,
) -> T {
    let cut = std::mem::take(summary(&mut packet)).cut();
    *summary(&mut packet) = ListingSummary {
        callsigns: Vec::new(),
        ..cut.clone()
    };
    // The packet with the summary's fixed part: what the callsigns have
    // left. A packet that does not encode keeps every callsign, and its
    // encoder says why.
    let Ok(bits) = wrap(packet.clone()).content_bits() else {
        *summary(&mut packet) = cut;
        return packet;
    };
    *summary(&mut packet) = cut.keep_callsigns((max_len * 8).saturating_sub(bits));
    packet
}

fn write_build(w: &mut BitWriter, build: &Build) -> Result<(), MasterEncodeError> {
    put(w, u64::from(build.protocol_version), 16);
    put_text(w, &build.game_version, MAX_BUILD_TEXT)?;
    put_text(w, &build.game_commit, MAX_BUILD_TEXT)?;
    put(w, u64::from(build.release), 8);
    Ok(())
}

fn read_build(r: &mut BitReader<'_>) -> Result<Build, MasterDecodeError> {
    let protocol_version = get(r, 16)? as u16;
    let game_version = get_text(r, MAX_BUILD_TEXT)?;
    let game_commit = get_text(r, MAX_BUILD_TEXT)?;
    let release = match get(r, 8)? {
        0 => false,
        1 => true,
        _ => return Err(MasterDecodeError::Malformed),
    };
    Ok(Build {
        protocol_version,
        game_version,
        game_commit,
        release,
    })
}

fn write_entry(w: &mut BitWriter, entry: &PageEntry) -> Result<(), MasterEncodeError> {
    if entry.platform > MAX_PAGE_PLATFORM {
        return Err(MasterEncodeError::BadField);
    }
    put(w, entry.listing_id, 64);
    let flags = u64::from(entry.password)
        | u64::from(entry.full) << 1
        | u64::from(entry.dedicated) << 2
        | u64::from(entry.other_build.is_some()) << 3
        | u64::from(entry.relay_likely) << 4;
    put(w, flags, 8);
    put(w, entry.phase as u64, 2);
    put(w, u64::from(entry.players), 8);
    put(w, u64::from(entry.capacity), 8);
    put(w, u64::from(entry.platform), 3);
    if let Some(version) = &entry.other_build {
        put_text(w, version, MAX_BUILD_TEXT)?;
    }
    put_text(w, &entry.name, MAX_DISCOVER_NAME)
}

fn read_entry(r: &mut BitReader<'_>) -> Result<PageEntry, MasterDecodeError> {
    let listing_id = get(r, 64)?;
    let flags = get(r, 8)?;
    if flags >> 5 != 0 {
        return Err(MasterDecodeError::Malformed);
    }
    let phase = phase_of(get(r, 2)?)?;
    let players = get(r, 8)? as u8;
    let capacity = get(r, 8)? as u8;
    let platform = get(r, 3)? as u8;
    let other_build = if flags & 8 != 0 {
        Some(get_text(r, MAX_BUILD_TEXT)?)
    } else {
        None
    };
    Ok(PageEntry {
        listing_id,
        password: flags & 1 != 0,
        full: flags & 2 != 0,
        dedicated: flags & 4 != 0,
        other_build,
        relay_likely: flags & 16 != 0,
        phase,
        players,
        capacity,
        platform,
        name: get_text(r, MAX_DISCOVER_NAME)?,
    })
}

fn phase_of(bits: u64) -> Result<DiscoverPhase, MasterDecodeError> {
    match bits {
        0 => Ok(DiscoverPhase::Lobby),
        1 => Ok(DiscoverPhase::Flying),
        2 => Ok(DiscoverPhase::Closed),
        _ => Err(MasterDecodeError::Malformed),
    }
}

/// Writes a field whose value the caller has bounded to its width.
fn put(w: &mut BitWriter, value: u64, bits: u32) {
    w.write_bits(value, bits).ok();
}

fn put_text(w: &mut BitWriter, text: &str, max: usize) -> Result<(), MasterEncodeError> {
    if text.len() > max {
        return Err(MasterEncodeError::BadField);
    }
    w.write_str(text).map_err(|_| MasterEncodeError::BadField)
}

fn get(r: &mut BitReader<'_>, bits: u32) -> Result<u64, MasterDecodeError> {
    Ok(r.read_bits(bits)?)
}

fn get_text(r: &mut BitReader<'_>, max: usize) -> Result<String, MasterDecodeError> {
    let text = r.read_str()?;
    if text.len() > max {
        return Err(MasterDecodeError::Malformed);
    }
    Ok(text)
}

/// Reads a code field of `bits` bits; an unnamed code is malformed.
fn code<T>(
    r: &mut BitReader<'_>,
    bits: u32,
    from: fn(u64) -> Option<T>,
) -> Result<T, MasterDecodeError> {
    from(get(r, bits)?).ok_or(MasterDecodeError::Malformed)
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

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use super::*;
    use crate::SplitMix64;
    use crate::master::candidate::CandidateKind;
    use crate::master::{MAX_CANDIDATES, relay_likely};

    fn addr(text: &str) -> SocketAddr {
        text.parse().unwrap()
    }

    fn build() -> Build {
        Build {
            protocol_version: 7,
            game_version: "0.1.3".into(),
            game_commit: "fb9c2ec".into(),
            release: true,
        }
    }

    fn summary(callsigns: &[&str]) -> ListingSummary {
        ListingSummary {
            protocol_version: 7,
            password: true,
            full: false,
            truncated: false,
            phase: DiscoverPhase::Lobby,
            players: callsigns.len() as u8,
            capacity: 8,
            session_id: 0x1122_3344_5566_7788,
            game_version: "0.1.3".into(),
            game_commit: "fb9c2ec".into(),
            name: "Friday night".into(),
            mission: "UKR, clear, airborne at 20000 ft: F/A-18D Hornet x4 against MiG-29 x4".into(),
            king: callsigns.first().copied().unwrap_or_default().into(),
            callsigns: callsigns.iter().map(|c| (*c).to_owned()).collect(),
        }
    }

    fn host_candidates() -> Vec<Candidate> {
        vec![
            Candidate::new(CandidateKind::Local, addr("192.168.1.20:26900")),
            Candidate::new(CandidateKind::Mapped, addr("203.0.113.5:26900")),
            Candidate::new(CandidateKind::GlobalIpv6, addr("[2001:db8::20]:26900")),
        ]
    }

    /// The largest lobby: 30 callsigns of 15 characters and every text over
    /// its limit, so fitting cuts them.
    fn biggest_summary() -> ListingSummary {
        let callsigns: Vec<String> = (0..30).map(|i| format!("Callsign_{i:06}")).collect();
        ListingSummary {
            game_version: "9".repeat(300),
            game_commit: "f".repeat(300),
            name: "n".repeat(300),
            mission: "s\u{e9}".repeat(300),
            king: callsigns[0].clone(),
            players: 30,
            capacity: 30,
            callsigns,
            ..summary(&[])
        }
    }

    /// One or more samples of every kind, named for the golden file.
    fn samples() -> Vec<(&'static str, MasterPacket)> {
        use MasterPacket as P;
        vec![
            (
                "challenge",
                P::Challenge(Challenge {
                    nonce: 1,
                    cookie: u64::MAX,
                }),
            ),
            (
                "unsupported",
                P::Unsupported(Unsupported {
                    lowest: 2,
                    highest: 3,
                    text: "This game is older than the Internet Lobby supports.".into(),
                }),
            ),
            (
                "register",
                P::Register(
                    Register {
                        nonce: 0x0123_4567_89AB_CDEF,
                        cookie: 0xFEDC_BA98_7654_3210,
                        build: build(),
                        dedicated: false,
                        telemetry: true,
                        install_id: 0x5EED_0000_0000_0001,
                        platform: 3,
                        candidates: host_candidates(),
                        summary: summary(&["Viper", "Maverick 1"]),
                    }
                    .fit(),
                ),
            ),
            (
                "listed",
                P::Listed(Listed {
                    nonce: 0x0123_4567_89AB_CDEF,
                    listing_id: 42,
                    token: 0xA5A5_A5A5_A5A5_A5A5,
                    seen: addr("203.0.113.5:26900"),
                    heartbeat_secs: 30,
                    keep_secs: 15,
                    expiry_secs: 90,
                }),
            ),
            (
                "heartbeat",
                P::Heartbeat(
                    Heartbeat {
                        token: 0xA5A5_A5A5_A5A5_A5A5,
                        change: 3,
                        candidates: host_candidates(),
                        summary: summary(&["Viper"]),
                    }
                    .fit(),
                ),
            ),
            (
                "heartbeat-ack",
                P::HeartbeatAck(HeartbeatAck {
                    listing_id: 42,
                    seen: addr("[2001:db8::20]:26900"),
                }),
            ),
            ("keep", P::Keep(Keep { token: 7 })),
            (
                "unknown-listing",
                P::UnknownListing(UnknownListing { token: 8 }),
            ),
            ("unregister", P::Unregister(Unregister { token: 9 })),
            (
                "browse",
                P::Browse(Browse {
                    nonce: 10,
                    build: build(),
                    other_builds: true,
                    full_games: false,
                    cursor: 0,
                }),
            ),
            (
                "page",
                P::Page(Page {
                    nonce: 10,
                    matching: 2,
                    next_cursor: 0,
                    entries: vec![
                        PageEntry {
                            listing_id: 42,
                            password: true,
                            full: false,
                            dedicated: false,
                            other_build: None,
                            relay_likely: false,
                            phase: DiscoverPhase::Lobby,
                            players: 2,
                            capacity: 8,
                            platform: 3,
                            name: "Friday night".into(),
                        },
                        PageEntry {
                            listing_id: 43,
                            password: false,
                            full: true,
                            dedicated: true,
                            other_build: Some("0.1.2".into()),
                            relay_likely: true,
                            phase: DiscoverPhase::Flying,
                            players: 30,
                            capacity: 30,
                            platform: 1,
                            name: "Red Flag".into(),
                        },
                    ],
                }),
            ),
            (
                "details",
                P::Details(Details {
                    nonce: 11,
                    listing_id: 42,
                }),
            ),
            (
                "listing-details",
                P::ListingDetails(
                    ListingDetails {
                        nonce: 11,
                        listing_id: 42,
                        summary: Some(summary(&["Viper", "Maverick 1"])),
                    }
                    .fit(DETAILS_LEN),
                ),
            ),
            (
                "listing-details-gone",
                P::ListingDetails(ListingDetails {
                    nonce: 12,
                    listing_id: 44,
                    summary: None,
                }),
            ),
            ("probe", P::Probe(Probe { nonce: 13 })),
            (
                "probe-answer",
                P::ProbeAnswer(ProbeAnswer {
                    nonce: 13,
                    port: ProbePort::Second,
                    seen: addr("203.0.113.5:31000"),
                }),
            ),
            (
                "introduce",
                P::Introduce(Introduce {
                    nonce: 14,
                    cookie: 0,
                    listing_id: 42,
                    build: build(),
                    mapping: MappingType::SamePort,
                    candidates: vec![
                        Candidate::new(CandidateKind::Local, addr("10.0.0.7:50000")),
                        Candidate::new(CandidateKind::GlobalIpv6, addr("[2001:db8::7]:50000")),
                    ],
                }),
            ),
            (
                "introduction",
                P::Introduction(Introduction {
                    nonce: 14,
                    result: IntroductionResult::Introduced,
                    introduction_id: 0x1D1D_1D1D_1D1D_1D1D,
                    hint: Hint::Race,
                    seen: addr("198.51.100.9:50000"),
                    host_mapping: MappingType::PortPerDestination,
                    host_candidates: {
                        let mut list = vec![Candidate::new(
                            CandidateKind::Seen,
                            addr("203.0.113.5:26900"),
                        )];
                        list.extend(host_candidates());
                        list
                    },
                    text: String::new(),
                }),
            ),
            (
                "introduction-refused",
                P::Introduction(Introduction {
                    nonce: 15,
                    result: IntroductionResult::Full,
                    introduction_id: 0,
                    hint: Hint::Race,
                    seen: addr("198.51.100.9:50000"),
                    host_mapping: MappingType::Unknown,
                    host_candidates: Vec::new(),
                    text: "The game is full.".into(),
                }),
            ),
            (
                "meet",
                P::Meet(Meet {
                    introduction_id: 0x1D1D_1D1D_1D1D_1D1D,
                    mapping: MappingType::SamePort,
                    candidates: vec![
                        Candidate::new(CandidateKind::Seen, addr("198.51.100.9:50000")),
                        Candidate::new(CandidateKind::Local, addr("10.0.0.7:50000")),
                    ],
                }),
            ),
            (
                "meet-ack",
                P::MeetAck(MeetAck {
                    token: 0xA5A5_A5A5_A5A5_A5A5,
                    introduction_id: 0x1D1D_1D1D_1D1D_1D1D,
                }),
            ),
            (
                "relay-request",
                P::RelayRequest(RelayRequest {
                    nonce: 14,
                    introduction_id: 0x1D1D_1D1D_1D1D_1D1D,
                }),
            ),
            (
                "relay-offer",
                P::RelayOffer(RelayOffer {
                    nonce: 14,
                    introduction_id: 0x1D1D_1D1D_1D1D_1D1D,
                    result: RelayResult::Open,
                    channel: 0xC0DE,
                    key: 0xBEEF_CAFE,
                    text: String::new(),
                }),
            ),
            (
                "relay-open",
                P::RelayOpen(RelayOpen {
                    introduction_id: 0x1D1D_1D1D_1D1D_1D1D,
                    channel: 0xC0DE,
                    key: 0xBEEF_CAFE,
                    player: addr("198.51.100.9:50000"),
                }),
            ),
            (
                "relay-open-ack",
                P::RelayOpenAck(RelayOpenAck {
                    token: 0xA5A5_A5A5_A5A5_A5A5,
                    channel: 0xC0DE,
                }),
            ),
            (
                "relay",
                P::Relay(Relay {
                    channel: 0xC0DE,
                    key: 0xBEEF_CAFE,
                    datagram: vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
                }),
            ),
            (
                "relay-close",
                P::RelayClose(RelayClose {
                    channel: 0xC0DE,
                    key: 0xBEEF_CAFE,
                    reason: CloseReason::Idle,
                }),
            ),
            (
                "report",
                P::Report(Report {
                    install_id: 0x5EED_0000_0000_0001,
                    role: Role::Player,
                    game_version: "0.1.3".into(),
                    platform: 2,
                    minutes: 47,
                    humans: 6,
                    path: Path::Punched,
                    connect_tenths: 12,
                    mapping: MappingType::SamePort,
                    port_mapping: PortMapping::Upnp,
                    relayed_kb: 0,
                    players_by_path: [0, 0, 0, 0, 0, 0],
                    migrations: 0,
                    failed_migrations: 0,
                }),
            ),
        ]
    }

    /// Rewrites the checksum so a damaged packet reaches the decoders.
    fn reseal(bytes: &mut [u8]) {
        if bytes.len() >= 4 {
            let crc = checksum(&bytes[4..]);
            bytes[..4].copy_from_slice(&crc.to_le_bytes());
        }
    }

    #[test]
    fn every_kind_has_a_sample_that_round_trips() {
        let samples = samples();
        for kind in MasterKind::ALL {
            assert!(samples.iter().any(|(_, p)| p.kind() == *kind), "{kind:?}");
        }
        for (name, packet) in &samples {
            let bytes = packet.encode().unwrap();
            assert!(bytes.len() <= MAX_MASTER_DATAGRAM, "{name}");
            if let Some(len) = packet.kind().padded_len() {
                assert_eq!(bytes.len(), len, "{name}");
            }
            assert_eq!(&MasterPacket::decode(&bytes).unwrap(), packet, "{name}");
            assert_eq!(peek_kind(&bytes), Some(packet.kind()));
        }
    }

    #[test]
    fn sizes_match_the_spec() {
        let len = |p: MasterPacket| p.encode().unwrap().len();
        let v6 = addr("[2001:db8::1]:65535");
        assert_eq!(
            len(MasterPacket::Challenge(Challenge {
                nonce: 0,
                cookie: 0
            })),
            23
        );
        for token in [0, u64::MAX] {
            assert_eq!(len(MasterPacket::Keep(Keep { token })), 15);
            assert_eq!(
                len(MasterPacket::UnknownListing(UnknownListing { token })),
                15
            );
            assert_eq!(len(MasterPacket::Unregister(Unregister { token })), 15);
        }
        // The largest of each small answer: an IPv6 address.
        let listed = Listed {
            nonce: 1,
            listing_id: 2,
            token: 3,
            seen: v6,
            heartbeat_secs: 30,
            keep_secs: 15,
            expiry_secs: 90,
        };
        assert_eq!(len(MasterPacket::Listed(listed)), 53);
        let ack = HeartbeatAck {
            listing_id: 1,
            seen: v6,
        };
        assert_eq!(len(MasterPacket::HeartbeatAck(ack)), 34);
        let probe = ProbeAnswer {
            nonce: 1,
            port: ProbePort::Main,
            seen: v6,
        };
        assert_eq!(len(MasterPacket::ProbeAnswer(probe)), 35);
        let meet_ack = MeetAck {
            token: 1,
            introduction_id: 2,
        };
        assert_eq!(len(MasterPacket::MeetAck(meet_ack)), 23);
        let request = RelayRequest {
            nonce: 1,
            introduction_id: 2,
        };
        assert_eq!(len(MasterPacket::RelayRequest(request)), 23);
        let open_ack = RelayOpenAck {
            token: 1,
            channel: 2,
        };
        assert_eq!(len(MasterPacket::RelayOpenAck(open_ack)), 19);
        let close = RelayClose {
            channel: 1,
            key: 2,
            reason: CloseReason::Stopping,
        };
        assert_eq!(len(MasterPacket::RelayClose(close)), 16);
        // A frame is the game's datagram and 15 bytes, which fit the
        // longest master datagram with the longest game datagram.
        let relay = Relay {
            channel: 1,
            key: 2,
            datagram: vec![0xAB; MAX_RELAYED],
        };
        assert_eq!(
            len(MasterPacket::Relay(relay)),
            RELAY_HEADER_LEN + MAX_RELAYED
        );
        assert_eq!(RELAY_HEADER_LEN, 15);
    }

    #[test]
    fn content_bits_count_every_field() {
        for (name, packet) in samples() {
            let bits = packet.content_bits().unwrap();
            let bytes = packet.encode().unwrap();
            match packet.kind().padded_len() {
                Some(len) => assert!(bits.div_ceil(8) <= len, "{name}"),
                None => assert_eq!(bits.div_ceil(8), bytes.len(), "{name}"),
            }
        }
        // The summary's and an entry's own counts agree with the encoder.
        for s in [
            summary(&[]),
            summary(&["Viper", "Maverick 1"]),
            biggest_summary().cut(),
        ] {
            let mut w = BitWriter::new();
            s.write(&mut w).unwrap();
            assert_eq!(w.bit_len(), s.bits());
        }
        let Some((_, MasterPacket::Page(page))) = samples().into_iter().find(|(n, _)| *n == "page")
        else {
            panic!("no page")
        };
        for entry in &page.entries {
            let mut w = BitWriter::new();
            write_entry(&mut w, entry).unwrap();
            assert_eq!(w.bit_len(), entry.bits());
        }
        let empty = Page {
            entries: Vec::new(),
            ..page
        };
        assert_eq!(
            MasterPacket::Page(empty).content_bits().unwrap(),
            Page::BASE_BITS
        );
    }

    #[test]
    fn a_summary_at_every_limit_with_30_callsigns_fits_each_packet_that_carries_it() {
        let biggest = biggest_summary();
        // Checks the packet fits and round trips; gives its length before
        // any padding, and the summary it carries.
        let check = |packet: MasterPacket, max: usize| -> (usize, ListingSummary) {
            let bytes = packet.encode().unwrap();
            assert!(bytes.len() <= max, "{} bytes over {max}", bytes.len());
            let back = MasterPacket::decode(&bytes).unwrap();
            assert_eq!(back, packet);
            let content = packet.content_bits().unwrap().div_ceil(8);
            let summary = match back {
                MasterPacket::Register(p) => p.summary,
                MasterPacket::Heartbeat(p) => p.summary,
                MasterPacket::ListingDetails(p) => p.summary.unwrap(),
                _ => unreachable!(),
            };
            (content, summary)
        };
        let register = |candidates: Vec<Candidate>| Register {
            nonce: 1,
            cookie: 2,
            build: Build {
                protocol_version: 7,
                game_version: "9".repeat(MAX_BUILD_TEXT),
                game_commit: "f".repeat(MAX_BUILD_TEXT),
                release: false,
            },
            dedicated: true,
            telemetry: true,
            install_id: 3,
            platform: 3,
            candidates,
            summary: biggest.clone(),
        };
        // With the three candidates a host lists, every callsign fits and
        // only the texts are cut.
        let fitted = [
            check(
                MasterPacket::Register(register(host_candidates()).fit()),
                REGISTER_LEN,
            ),
            check(
                MasterPacket::Heartbeat(
                    Heartbeat {
                        token: 1,
                        change: 2,
                        candidates: host_candidates(),
                        summary: biggest.clone(),
                    }
                    .fit(),
                ),
                MAX_MASTER_DATAGRAM,
            ),
            check(
                MasterPacket::ListingDetails(
                    ListingDetails {
                        nonce: 1,
                        listing_id: 2,
                        summary: Some(biggest.clone()),
                    }
                    .fit(DETAILS_LEN),
                ),
                DETAILS_LEN,
            ),
        ];
        // The lengths master-protocol.md gives ("The wire as built").
        let lengths: Vec<usize> = fitted.iter().map(|(len, _)| *len).collect();
        assert_eq!(lengths, [1103, 954, 929]);
        for (_, s) in fitted {
            assert_eq!(s.callsigns.len(), 30);
            assert!(!s.truncated);
            assert_eq!(s.game_version.len(), MAX_DISCOVER_BUILD);
            assert_eq!(s.game_commit.len(), MAX_DISCOVER_BUILD);
            assert_eq!(s.name.len(), MAX_DISCOVER_NAME);
            assert!(s.mission.len() <= MAX_DISCOVER_SUMMARY && s.mission.len() >= 199);
            assert_eq!(s.king, "Callsign_000000");
        }
        // With eight IPv6 candidates, the most a list holds, the Register's
        // callsigns are cut from the end and the cut is flagged.
        let eight = vec![
            Candidate::new(CandidateKind::GlobalIpv6, addr("[2001:db8::1]:1"));
            MAX_CANDIDATES
        ];
        let (_, s) = check(MasterPacket::Register(register(eight).fit()), REGISTER_LEN);
        assert!(s.truncated && s.callsigns.len() < 30 && !s.callsigns.is_empty());
        assert_eq!(s.callsigns[0], "Callsign_000000");
        // A smaller Details still gets an answer within it.
        for max in [500, 600, 800] {
            let details = ListingDetails {
                nonce: 1,
                listing_id: 2,
                summary: Some(biggest.clone()),
            }
            .fit(max);
            let (_, s) = check(MasterPacket::ListingDetails(details), max);
            assert!(s.truncated && s.callsigns.len() < 30);
        }
        // Callsigns that are not valid are dropped, and flagged.
        let mut odd = summary(&["Viper", "Maverick 1"]);
        odd.callsigns.insert(1, "tab\there".into());
        let s = ListingDetails {
            nonce: 1,
            listing_id: 2,
            summary: Some(odd),
        }
        .fit(DETAILS_LEN);
        let s = s.summary.unwrap();
        assert_eq!(s.callsigns, ["Viper", "Maverick 1"]);
        assert!(s.truncated);
    }

    #[test]
    fn no_answer_to_an_unproven_sender_is_longer_than_its_request() {
        let v6 = addr("[2001:db8::ffff]:65535");
        let challenge = MasterPacket::Challenge(Challenge {
            nonce: u64::MAX,
            cookie: u64::MAX,
        });
        let unknown = MasterPacket::UnknownListing(UnknownListing { token: u64::MAX });
        // The largest Page: 255 entries of the longest texts, fitted.
        let entry = PageEntry {
            listing_id: u64::MAX,
            password: true,
            full: true,
            dedicated: true,
            other_build: Some("v".repeat(300)),
            relay_likely: true,
            phase: DiscoverPhase::Closed,
            players: 255,
            capacity: 255,
            platform: MAX_PAGE_PLATFORM,
            name: "n".repeat(300),
        };
        let page = Page {
            nonce: u64::MAX,
            matching: MAX_BROWSE_MATCHES_FOR_TESTS,
            next_cursor: u32::MAX,
            entries: vec![entry; 300],
        }
        .fit(BROWSE_LEN);
        assert!(!page.entries.is_empty());
        let details = ListingDetails {
            nonce: u64::MAX,
            listing_id: u64::MAX,
            summary: Some(biggest_summary()),
        }
        .fit(DETAILS_LEN);
        let probe = MasterPacket::ProbeAnswer(ProbeAnswer {
            nonce: u64::MAX,
            port: ProbePort::Second,
            seen: v6,
        });
        let smallest_heartbeat = MasterPacket::Heartbeat(Heartbeat {
            token: 0,
            change: 0,
            candidates: Vec::new(),
            summary: ListingSummary::default(),
        });
        let longest = |p: &MasterPacket| p.encode().unwrap().len();
        let heartbeat_len = longest(&smallest_heartbeat);
        assert_eq!(heartbeat_len, 37);
        for (request, request_len, answer_len) in [
            (MasterKind::Register, REGISTER_LEN, longest(&challenge)),
            (
                MasterKind::Browse,
                BROWSE_LEN,
                longest(&MasterPacket::Page(page)),
            ),
            (
                MasterKind::Details,
                DETAILS_LEN,
                longest(&MasterPacket::ListingDetails(details)),
            ),
            (MasterKind::Probe, PROBE_LEN, longest(&probe)),
            (MasterKind::Introduce, INTRODUCE_LEN, longest(&challenge)),
            (MasterKind::Heartbeat, heartbeat_len, longest(&unknown)),
            (MasterKind::Keep, 15, longest(&unknown)),
            (MasterKind::Unregister, 15, longest(&unknown)),
        ] {
            assert!(
                answer_len <= request_len,
                "{request:?}: {answer_len} > {request_len}"
            );
            assert_eq!(request.padded_len().unwrap_or(request_len), request_len);
        }
        // An Unsupported with the longest text goes only where it fits.
        let unsupported = MasterPacket::Unsupported(Unsupported {
            lowest: 1,
            highest: u16::MAX,
            text: "x".repeat(MAX_UNSUPPORTED_TEXT),
        });
        assert_eq!(longest(&unsupported), 7 + 4 + 1 + MAX_UNSUPPORTED_TEXT);
        assert!(unsupported.encode_within(9, DETAILS_LEN).is_ok());
        assert_eq!(
            unsupported.encode_within(9, PROBE_LEN),
            Err(MasterEncodeError::TooLarge)
        );
    }

    /// The master's cap on matching listings, as a Page's 16-bit count.
    const MAX_BROWSE_MATCHES_FOR_TESTS: u16 = crate::master::MAX_BROWSE_MATCHES;

    #[test]
    fn a_page_keeps_its_entries_in_order_within_its_length() {
        let entry = |i: u64, name_len: usize| PageEntry {
            listing_id: i,
            password: false,
            full: false,
            dedicated: false,
            other_build: None,
            relay_likely: false,
            phase: DiscoverPhase::Lobby,
            players: 1,
            capacity: 8,
            platform: 3,
            name: "g".repeat(name_len),
        };
        let page = |entries| Page {
            nonce: 1,
            matching: 200,
            next_cursor: 0,
            entries,
        };
        // Short names: all 20 fit a 1,200-byte Browse.
        let fitted = page((0..20).map(|i| entry(i, 12)).collect()).fit(BROWSE_LEN);
        assert_eq!(fitted.entries.len(), 20);
        // Names at their limit: 10 or more still fit, and the rest are left
        // for the next page in order.
        let fitted = page((0..40).map(|i| entry(i, 64)).collect()).fit(BROWSE_LEN);
        assert!(
            (10..40).contains(&fitted.entries.len()),
            "{}",
            fitted.entries.len()
        );
        assert!(
            fitted
                .entries
                .iter()
                .enumerate()
                .all(|(i, e)| e.listing_id == i as u64)
        );
        assert!(MasterPacket::Page(fitted).encode().unwrap().len() <= BROWSE_LEN);
        // Never more than 255, and a platform code over 7 does not encode.
        let fitted = page((0..300).map(|i| entry(i, 0)).collect()).fit(usize::MAX / 16);
        assert_eq!(fitted.entries.len(), MAX_PAGE_ENTRIES);
        let mut bad = page(vec![entry(0, 1)]);
        bad.entries[0].platform = 8;
        assert_eq!(
            MasterPacket::Page(bad).encode(),
            Err(MasterEncodeError::BadField)
        );
        assert!(relay_likely(MappingType::PortPerDestination, &[]));
    }

    #[test]
    fn padded_requests_must_be_exactly_their_length_with_zero_padding() {
        for (name, packet) in samples() {
            let Some(len) = packet.kind().padded_len() else {
                continue;
            };
            let bytes = packet.encode().unwrap();
            let mut short = bytes[..len - 1].to_vec();
            reseal(&mut short);
            assert_eq!(
                MasterPacket::decode(&short),
                Err(MasterDecodeError::Malformed),
                "{name}"
            );
            let mut long = bytes.clone();
            long.push(0);
            reseal(&mut long);
            assert_eq!(
                MasterPacket::decode(&long),
                Err(MasterDecodeError::Malformed),
                "{name}"
            );
            let mut dirty = bytes.clone();
            dirty[len - 1] = 1;
            reseal(&mut dirty);
            assert_eq!(
                MasterPacket::decode(&dirty),
                Err(MasterDecodeError::Malformed),
                "{name}"
            );
        }
    }

    #[test]
    fn unpadded_packets_refuse_trailing_bytes_and_stray_bits() {
        for (name, packet) in samples() {
            if packet.kind().padded_len().is_some() || packet.kind() == MasterKind::Relay {
                continue;
            }
            let bytes = packet.encode().unwrap();
            let mut long = bytes.clone();
            long.push(0);
            reseal(&mut long);
            assert_eq!(
                MasterPacket::decode(&long),
                Err(MasterDecodeError::Malformed),
                "{name}"
            );
            // A set bit in the last byte's unused bits, where there are any.
            let bits = packet.content_bits().unwrap();
            if bits % 8 != 0 {
                let mut stray = bytes.clone();
                *stray.last_mut().unwrap() |= 0x80;
                reseal(&mut stray);
                assert!(MasterPacket::decode(&stray).is_err(), "{name}");
            }
        }
    }

    #[test]
    fn versions_are_checked_before_kinds_and_unsupported_reads_in_any() {
        let all = samples();
        let keep = &all[6].1;
        let unsupported = &all[1].1;
        assert_eq!(keep.kind(), MasterKind::Keep);
        for version in [0, 2, u16::MAX] {
            let bytes = keep.encode_in(version).unwrap();
            assert_eq!(
                MasterPacket::decode(&bytes),
                Err(MasterDecodeError::Unsupported {
                    version,
                    kind: MasterKind::Keep.code()
                })
            );
            // A kind this version does not name is still an unsupported
            // version first: a later version may name it.
            let mut later = bytes.clone();
            later[4] = 200;
            reseal(&mut later);
            assert_eq!(
                MasterPacket::decode(&later),
                Err(MasterDecodeError::Unsupported { version, kind: 200 })
            );
            let bytes = unsupported.encode_in(version).unwrap();
            assert_eq!(&MasterPacket::decode(&bytes).unwrap(), unsupported);
        }
        for kind in [0, 27, 255] {
            let mut bytes = keep.encode().unwrap();
            bytes[4] = kind;
            reseal(&mut bytes);
            assert_eq!(
                MasterPacket::decode(&bytes),
                Err(MasterDecodeError::UnknownKind(kind))
            );
        }
        // Any other program's datagram, the game's own packets included,
        // fails the checksum.
        let game = crate::packet::Packet::Keepalive(crate::packet::Keepalive { connection: 1 })
            .encode(7)
            .unwrap();
        let mut padded = game.clone();
        padded.resize(15, 0);
        for datagram in [&game[..], &padded[..]] {
            assert!(matches!(
                MasterPacket::decode(datagram),
                Err(MasterDecodeError::Checksum | MasterDecodeError::TooShort)
            ));
        }
        assert_eq!(
            MasterPacket::decode(&[0; 6]),
            Err(MasterDecodeError::TooShort)
        );
        assert_eq!(
            MasterPacket::decode(&[0; MAX_MASTER_DATAGRAM + 1]),
            Err(MasterDecodeError::TooLong)
        );
    }

    #[test]
    fn fields_outside_their_rules_neither_encode_nor_decode() {
        let samples = samples();
        let find = |name: &str| samples.iter().find(|(n, _)| *n == name).unwrap().1.clone();
        let MasterPacket::Register(register) = find("register") else {
            panic!()
        };
        // The install id goes with the telemetry switch.
        let off = Register {
            telemetry: false,
            ..register.clone()
        };
        assert_eq!(
            MasterPacket::Register(off).encode(),
            Err(MasterEncodeError::BadField)
        );
        let zero = Register {
            install_id: 0,
            ..register.clone()
        };
        assert_eq!(
            MasterPacket::Register(zero).encode(),
            Err(MasterEncodeError::BadField)
        );
        let quiet = Register {
            telemetry: false,
            install_id: 0,
            ..register.clone()
        };
        assert!(MasterPacket::Register(quiet).encode().is_ok());
        // A sender never lists a Seen candidate.
        let mut seen = register.clone();
        seen.candidates
            .push(Candidate::new(CandidateKind::Seen, addr("1.2.3.4:5")));
        assert_eq!(
            MasterPacket::Register(seen).encode(),
            Err(MasterEncodeError::BadField)
        );
        // A text over its limit.
        let mut long = register.clone();
        long.build.game_commit = "f".repeat(MAX_BUILD_TEXT + 1);
        assert_eq!(
            MasterPacket::Register(long).encode(),
            Err(MasterEncodeError::BadField)
        );
        let refused = Introduction {
            text: "x".repeat(MAX_TEXT + 1),
            ..match find("introduction-refused") {
                MasterPacket::Introduction(p) => p,
                _ => panic!(),
            }
        };
        assert_eq!(
            MasterPacket::Introduction(refused).encode(),
            Err(MasterEncodeError::BadField)
        );
        // A report needs an install id; a relay frame needs a datagram.
        let MasterPacket::Report(report) = find("report") else {
            panic!()
        };
        let anonymous = Report {
            install_id: 0,
            ..report
        };
        assert_eq!(
            MasterPacket::Report(anonymous).encode(),
            Err(MasterEncodeError::BadField)
        );
        for datagram in [vec![], vec![0; MAX_RELAYED + 1]] {
            let frame = Relay {
                channel: 1,
                key: 2,
                datagram,
            };
            assert_eq!(
                MasterPacket::Relay(frame.clone()).encode(),
                Err(MasterEncodeError::BadField)
            );
            let view = RelayFrame {
                channel: 1,
                key: 2,
                datagram: &frame.datagram,
            };
            assert_eq!(view.encode(), Err(MasterEncodeError::BadField));
        }
        // Codes the protocol does not name: a result byte, a phase of 3.
        let mut bytes = find("relay-offer").encode().unwrap();
        bytes[HEADER_LEN + 16] = 6;
        reseal(&mut bytes);
        assert_eq!(
            MasterPacket::decode(&bytes),
            Err(MasterDecodeError::Malformed)
        );
        let mut bytes = find("heartbeat").encode().unwrap();
        let summary_at = HEADER_LEN * 8 + 64 + 16 + candidate::list_bits(&host_candidates());
        let flags_at = summary_at + 16;
        let phase_bit = flags_at + 3;
        for bit in [phase_bit, phase_bit + 1] {
            bytes[bit / 8] |= 1 << (bit % 8);
        }
        reseal(&mut bytes);
        assert_eq!(
            MasterPacket::decode(&bytes),
            Err(MasterDecodeError::Malformed)
        );
    }

    #[test]
    fn a_relay_frame_reads_in_place() {
        let datagram: Vec<u8> = (0..=255).collect();
        let view = RelayFrame {
            channel: 0xC0DE,
            key: 0xBEEF_CAFE,
            datagram: &datagram,
        };
        let bytes = view.encode().unwrap();
        assert_eq!(bytes.len(), RELAY_HEADER_LEN + datagram.len());
        assert_eq!(&bytes[RELAY_HEADER_LEN..], &datagram[..]);
        assert_eq!(RelayFrame::open(&bytes).unwrap(), view);
        assert_eq!(peek_kind(&bytes), Some(MasterKind::Relay));
        let packet = MasterPacket::Relay(Relay {
            channel: 0xC0DE,
            key: 0xBEEF_CAFE,
            datagram: datagram.clone(),
        });
        assert_eq!(packet.encode().unwrap(), bytes);
        assert_eq!(MasterPacket::decode(&bytes).unwrap(), packet);
        // Another kind, a damaged frame, a frame with no datagram.
        let keep = MasterPacket::Keep(Keep { token: 1 }).encode().unwrap();
        assert_eq!(RelayFrame::open(&keep), Err(MasterDecodeError::Malformed));
        let mut damaged = bytes.clone();
        damaged[20] ^= 1;
        assert_eq!(RelayFrame::open(&damaged), Err(MasterDecodeError::Checksum));
        let mut empty = bytes[..RELAY_HEADER_LEN].to_vec();
        reseal(&mut empty);
        assert_eq!(RelayFrame::open(&empty), Err(MasterDecodeError::Malformed));
        assert_eq!(peek_kind(&[]), None);
    }

    #[test]
    fn a_summary_is_the_discovery_answer_without_its_nonce() {
        let answer = summary(&["Viper", "Maverick 1"]).to_discover_answer(99);
        assert_eq!(answer.nonce, 99);
        assert_eq!(
            ListingSummary::from(answer.clone()),
            summary(&["Viper", "Maverick 1"])
        );
        // The same bytes as the answer's after its header and nonce.
        let discover = crate::packet::Packet::DiscoverAnswer(answer)
            .encode(7)
            .unwrap();
        let mut w = BitWriter::new();
        summary(&["Viper", "Maverick 1"]).write(&mut w).unwrap();
        assert_eq!(w.finish(), discover[5 + 8..]);
    }

    // Seeded random packets of every kind.

    struct Gen(SplitMix64);

    impl Gen {
        fn below(&mut self, n: u64) -> u64 {
            self.0.below(n)
        }

        fn bits(&mut self, bits: u32) -> u64 {
            let v = self.0.next_u64();
            if bits == 64 { v } else { v & ((1 << bits) - 1) }
        }

        fn flag(&mut self) -> bool {
            self.below(2) == 1
        }

        fn pick<T: Copy>(&mut self, all: &[T]) -> T {
            all[self.below(all.len() as u64) as usize]
        }

        /// Up to `max` bytes of mixed one-, two- and three-byte characters.
        fn text(&mut self, max: usize) -> String {
            let target = match self.below(4) {
                0 => 0,
                1 => max,
                _ => self.below(max as u64 + 1) as usize,
            };
            let mut text = String::new();
            loop {
                let c = match self.below(8) {
                    0 => '\u{e9}',
                    1 => '\u{65e5}',
                    _ => char::from(0x20 + self.below(95) as u8),
                };
                if text.len() + c.len_utf8() > target {
                    return text;
                }
                text.push(c);
            }
        }

        fn callsign(&mut self) -> String {
            let len = 1 + self.below(MAX_CALLSIGN as u64);
            (0..len)
                .map(|_| char::from(0x20 + self.below(95) as u8))
                .collect()
        }

        fn address(&mut self) -> SocketAddr {
            let ip = if self.flag() {
                IpAddr::V4(Ipv4Addr::from(self.bits(32) as u32))
            } else {
                let mut ip =
                    Ipv6Addr::from(u128::from(self.bits(64)) << 64 | u128::from(self.bits(64)));
                if ip.to_ipv4_mapped().is_some() {
                    ip = Ipv6Addr::LOCALHOST;
                }
                IpAddr::V6(ip)
            };
            SocketAddr::new(ip, self.bits(16) as u16)
        }

        fn candidates(&mut self, sender: bool) -> Vec<Candidate> {
            let count = self.below(MAX_CANDIDATES as u64 + 1);
            (0..count)
                .map(|_| {
                    let kind = loop {
                        let kind = self.pick(CandidateKind::ALL);
                        if !(sender && kind == CandidateKind::Seen) {
                            break kind;
                        }
                    };
                    Candidate::new(kind, self.address())
                })
                .collect()
        }

        fn build(&mut self) -> Build {
            Build {
                protocol_version: self.bits(16) as u16,
                game_version: self.text(MAX_BUILD_TEXT),
                game_commit: self.text(MAX_BUILD_TEXT),
                release: self.flag(),
            }
        }

        fn summary(&mut self) -> ListingSummary {
            let count = self.below(31);
            ListingSummary {
                protocol_version: self.bits(16) as u16,
                password: self.flag(),
                full: self.flag(),
                truncated: self.flag(),
                phase: self.pick(&[
                    DiscoverPhase::Lobby,
                    DiscoverPhase::Flying,
                    DiscoverPhase::Closed,
                ]),
                players: self.bits(8) as u8,
                capacity: self.bits(8) as u8,
                session_id: self.bits(64),
                // Over the limits at times: fitting cuts them.
                game_version: self.text(80),
                game_commit: self.text(80),
                name: self.text(80),
                mission: self.text(250),
                king: if self.flag() {
                    self.callsign()
                } else {
                    String::new()
                },
                callsigns: (0..count).map(|_| self.callsign()).collect(),
            }
        }

        fn entry(&mut self) -> PageEntry {
            PageEntry {
                listing_id: self.bits(64),
                password: self.flag(),
                full: self.flag(),
                dedicated: self.flag(),
                other_build: if self.flag() {
                    Some(self.text(MAX_BUILD_TEXT))
                } else {
                    None
                },
                relay_likely: self.flag(),
                phase: self.pick(&[
                    DiscoverPhase::Lobby,
                    DiscoverPhase::Flying,
                    DiscoverPhase::Closed,
                ]),
                players: self.bits(8) as u8,
                capacity: self.bits(8) as u8,
                platform: self.bits(3) as u8,
                name: self.text(MAX_DISCOVER_NAME),
            }
        }

        fn packet(&mut self, kind: MasterKind) -> MasterPacket {
            use MasterPacket as P;
            match kind {
                MasterKind::Challenge => P::Challenge(Challenge {
                    nonce: self.bits(64),
                    cookie: self.bits(64),
                }),
                MasterKind::Unsupported => P::Unsupported(Unsupported {
                    lowest: self.bits(16) as u16,
                    highest: self.bits(16) as u16,
                    text: self.text(MAX_UNSUPPORTED_TEXT),
                }),
                MasterKind::Register => {
                    let telemetry = self.flag();
                    P::Register(
                        Register {
                            nonce: self.bits(64),
                            cookie: self.bits(64),
                            build: self.build(),
                            dedicated: self.flag(),
                            telemetry,
                            install_id: if telemetry { self.bits(64).max(1) } else { 0 },
                            platform: self.bits(8) as u8,
                            candidates: self.candidates(true),
                            summary: self.summary(),
                        }
                        .fit(),
                    )
                }
                MasterKind::Listed => P::Listed(Listed {
                    nonce: self.bits(64),
                    listing_id: self.bits(64),
                    token: self.bits(64),
                    seen: self.address(),
                    heartbeat_secs: self.bits(8) as u8,
                    keep_secs: self.bits(8) as u8,
                    expiry_secs: self.bits(8) as u8,
                }),
                MasterKind::Heartbeat => P::Heartbeat(
                    Heartbeat {
                        token: self.bits(64),
                        change: self.bits(16) as u16,
                        candidates: self.candidates(true),
                        summary: self.summary(),
                    }
                    .fit(),
                ),
                MasterKind::HeartbeatAck => P::HeartbeatAck(HeartbeatAck {
                    listing_id: self.bits(64),
                    seen: self.address(),
                }),
                MasterKind::Keep => P::Keep(Keep {
                    token: self.bits(64),
                }),
                MasterKind::UnknownListing => P::UnknownListing(UnknownListing {
                    token: self.bits(64),
                }),
                MasterKind::Unregister => P::Unregister(Unregister {
                    token: self.bits(64),
                }),
                MasterKind::Browse => P::Browse(Browse {
                    nonce: self.bits(64),
                    build: self.build(),
                    other_builds: self.flag(),
                    full_games: self.flag(),
                    cursor: self.bits(32) as u32,
                }),
                MasterKind::Page => {
                    let count = self.below(30);
                    P::Page(
                        Page {
                            nonce: self.bits(64),
                            matching: self.bits(16) as u16,
                            next_cursor: self.bits(32) as u32,
                            entries: (0..count).map(|_| self.entry()).collect(),
                        }
                        .fit(BROWSE_LEN),
                    )
                }
                MasterKind::Details => P::Details(Details {
                    nonce: self.bits(64),
                    listing_id: self.bits(64),
                }),
                MasterKind::ListingDetails => P::ListingDetails(
                    ListingDetails {
                        nonce: self.bits(64),
                        listing_id: self.bits(64),
                        summary: if self.flag() {
                            Some(self.summary())
                        } else {
                            None
                        },
                    }
                    .fit(DETAILS_LEN),
                ),
                MasterKind::Probe => P::Probe(Probe {
                    nonce: self.bits(64),
                }),
                MasterKind::ProbeAnswer => P::ProbeAnswer(ProbeAnswer {
                    nonce: self.bits(64),
                    port: self.pick(ProbePort::ALL),
                    seen: self.address(),
                }),
                MasterKind::Introduce => P::Introduce(Introduce {
                    nonce: self.bits(64),
                    cookie: self.bits(64),
                    listing_id: self.bits(64),
                    build: self.build(),
                    mapping: self.pick(MappingType::ALL),
                    candidates: self.candidates(true),
                }),
                MasterKind::Introduction => P::Introduction(Introduction {
                    nonce: self.bits(64),
                    result: self.pick(IntroductionResult::ALL),
                    introduction_id: self.bits(64),
                    hint: self.pick(Hint::ALL),
                    seen: self.address(),
                    host_mapping: self.pick(MappingType::ALL),
                    host_candidates: self.candidates(false),
                    text: self.text(MAX_TEXT),
                }),
                MasterKind::Meet => P::Meet(Meet {
                    introduction_id: self.bits(64),
                    mapping: self.pick(MappingType::ALL),
                    candidates: self.candidates(false),
                }),
                MasterKind::MeetAck => P::MeetAck(MeetAck {
                    token: self.bits(64),
                    introduction_id: self.bits(64),
                }),
                MasterKind::RelayRequest => P::RelayRequest(RelayRequest {
                    nonce: self.bits(64),
                    introduction_id: self.bits(64),
                }),
                MasterKind::RelayOffer => P::RelayOffer(RelayOffer {
                    nonce: self.bits(64),
                    introduction_id: self.bits(64),
                    result: self.pick(RelayResult::ALL),
                    channel: self.bits(32) as u32,
                    key: self.bits(32) as u32,
                    text: self.text(MAX_TEXT),
                }),
                MasterKind::RelayOpen => P::RelayOpen(RelayOpen {
                    introduction_id: self.bits(64),
                    channel: self.bits(32) as u32,
                    key: self.bits(32) as u32,
                    player: self.address(),
                }),
                MasterKind::RelayOpenAck => P::RelayOpenAck(RelayOpenAck {
                    token: self.bits(64),
                    channel: self.bits(32) as u32,
                }),
                MasterKind::Relay => {
                    let len = 1 + self.below(MAX_RELAYED as u64) as usize;
                    P::Relay(Relay {
                        channel: self.bits(32) as u32,
                        key: self.bits(32) as u32,
                        datagram: (0..len).map(|_| self.bits(8) as u8).collect(),
                    })
                }
                MasterKind::RelayClose => P::RelayClose(RelayClose {
                    channel: self.bits(32) as u32,
                    key: self.bits(32) as u32,
                    reason: self.pick(CloseReason::ALL),
                }),
                MasterKind::Report => P::Report(Report {
                    install_id: self.bits(64).max(1),
                    role: self.pick(Role::ALL),
                    game_version: self.text(MAX_BUILD_TEXT),
                    platform: self.bits(8) as u8,
                    minutes: self.bits(16) as u16,
                    humans: self.bits(8) as u8,
                    path: self.pick(Path::ALL),
                    connect_tenths: self.bits(8) as u8,
                    mapping: self.pick(MappingType::ALL),
                    port_mapping: self.pick(PortMapping::ALL),
                    relayed_kb: self.bits(32) as u32,
                    players_by_path: std::array::from_fn(|_| self.bits(8) as u8),
                    migrations: self.bits(8) as u8,
                    failed_migrations: self.bits(8) as u8,
                }),
            }
        }
    }

    #[test]
    fn seeded_random_packets_of_every_kind_round_trip() {
        let mut g = Gen(SplitMix64::new(0x4D41_5354_4552));
        for round in 0..300 {
            for kind in MasterKind::ALL {
                let packet = g.packet(*kind);
                let bytes = packet
                    .encode()
                    .unwrap_or_else(|e| panic!("round {round}: {kind:?} did not encode: {e}"));
                assert!(bytes.len() <= MAX_MASTER_DATAGRAM);
                assert_eq!(
                    MasterPacket::decode(&bytes).unwrap(),
                    packet,
                    "round {round}"
                );
            }
        }
    }

    #[test]
    fn a_hundred_thousand_fuzzed_datagrams_never_panic_and_only_canonical_ones_decode() {
        let mut g = Gen(SplitMix64::new(0xF022));
        let mut corpus: Vec<Vec<u8>> = samples().iter().map(|(_, p)| p.encode().unwrap()).collect();
        for _ in 0..4 {
            for kind in MasterKind::ALL {
                corpus.push(g.packet(*kind).encode().unwrap());
            }
        }
        let mut decoded = 0;
        for _ in 0..100_000 {
            let mut bytes = if g.below(4) == 0 {
                // Random bytes behind a plausible header.
                let len = g.below(MAX_MASTER_DATAGRAM as u64 + 40) as usize;
                let mut bytes: Vec<u8> = (0..len).map(|_| g.bits(8) as u8).collect();
                if bytes.len() >= HEADER_LEN && g.below(5) != 0 {
                    bytes[4] = 1 + g.below(MasterKind::ALL.len() as u64) as u8;
                    bytes[5..7].copy_from_slice(&MASTER_VERSION.to_le_bytes());
                }
                bytes
            } else {
                // A good packet, damaged a little.
                let mut bytes = corpus[g.below(corpus.len() as u64) as usize].clone();
                for _ in 0..1 + g.below(4) {
                    match g.below(6) {
                        0 if !bytes.is_empty() => {
                            let at = g.below(bytes.len() as u64) as usize;
                            bytes[at] ^= 1 << g.below(8);
                        }
                        1 if !bytes.is_empty() => {
                            let at = g.below(bytes.len() as u64) as usize;
                            bytes[at] = g.bits(8) as u8;
                        }
                        2 => {
                            let len = g.below(bytes.len() as u64 + 1) as usize;
                            bytes.truncate(len);
                        }
                        3 => {
                            for _ in 0..1 + g.below(8) {
                                bytes.push(if g.flag() { 0 } else { g.bits(8) as u8 });
                            }
                        }
                        4 if bytes.len() > HEADER_LEN => {
                            // A length or count byte stretched.
                            let at =
                                HEADER_LEN + g.below((bytes.len() - HEADER_LEN) as u64) as usize;
                            bytes[at] = g.pick(&[0, 1, 8, 9, 15, 16, 64, 65, 200, 201, 255]);
                        }
                        _ => {}
                    }
                }
                bytes
            };
            if g.below(10) != 0 {
                reseal(&mut bytes);
            }
            let Ok(packet) = MasterPacket::decode(&bytes) else {
                continue;
            };
            decoded += 1;
            // Strict decoding: what decodes is exactly what it encodes to.
            let version = u16::from_le_bytes([bytes[5], bytes[6]]);
            assert_eq!(packet.encode_in(version).unwrap(), bytes, "{packet:?}");
        }
        // The mutations reach the decoders: some decode, most do not.
        assert!(decoded > 1_000 && decoded < 90_000, "{decoded} decoded");
    }

    /// The encodings of every sample, one line each: the master protocol's
    /// bytes. Bytes that change need a new master protocol version. Refresh
    /// with `TORE_UPDATE_MASTER_GOLDEN=1 cargo test --locked -p tore-net
    /// master_golden`; under the same version that only adds lines.
    #[test]
    fn master_golden() {
        let hex = |bytes: &[u8]| {
            bytes.iter().fold(String::new(), |mut out, b| {
                let _ = write!(out, "{b:02x}");
                out
            })
        };
        let mut text = String::from(
            "# T.O.R.E master protocol golden: samples of every kind, sealed under the TORE-MASTER id.\n\
             # A padded request's line holds its length and its bytes up to the padding.\n",
        );
        let _ = writeln!(text, "master-version {MASTER_VERSION}");
        for (name, packet) in samples() {
            let bytes = packet.encode().unwrap();
            let shown = packet.content_bits().unwrap().div_ceil(8);
            let _ = writeln!(text, "{name} {} {}", bytes.len(), hex(&bytes[..shown]));
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("master-golden.txt");
        // A Windows checkout may turn the file's line ends into CR LF.
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        let version_line = format!("master-version {MASTER_VERSION}\n");
        if std::env::var_os("TORE_UPDATE_MASTER_GOLDEN").is_some() {
            if committed.contains(&version_line) {
                for line in committed.lines().filter(|l| !l.starts_with('#')) {
                    assert!(
                        text.lines().any(|l| l == line),
                        "the master encodings changed under version {MASTER_VERSION}: raise \
                         MASTER_VERSION first ({line})"
                    );
                }
            }
            std::fs::write(&path, &text).unwrap();
            return;
        }
        assert_eq!(
            text, committed,
            "the master protocol's bytes differ from master-golden.txt: raise MASTER_VERSION, \
             then TORE_UPDATE_MASTER_GOLDEN=1 cargo test --locked -p tore-net master_golden"
        );
        // And the committed bytes decode back to the samples.
        let samples = samples();
        for line in committed
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("master-"))
        {
            let mut parts = line.split(' ');
            let name = parts.next().unwrap();
            let len: usize = parts.next().unwrap().parse().unwrap();
            let shown = parts.next().unwrap();
            let mut bytes: Vec<u8> = (0..shown.len() / 2)
                .map(|i| u8::from_str_radix(&shown[2 * i..2 * i + 2], 16).unwrap())
                .collect();
            bytes.resize(len, 0);
            let packet = &samples.iter().find(|(n, _)| *n == name).unwrap().1;
            assert_eq!(&MasterPacket::decode(&bytes).unwrap(), packet, "{name}");
        }
    }
}
