//! The game's wire messages: the sections a Payload carries (Inputs,
//! Snapshot, Events, Own state) and the reliable message bodies, with the
//! bookkeeping each end keeps per connection.
//!
//! The specification is [`docs/formats/net-protocol.md`](../../../../docs/formats/net-protocol.md)
//! from "Inputs" on; where the build settled a detail the design left open,
//! that document says so. Everything here is plain data and pure functions of
//! it: no clock, socket or `World` step.
//!
//! - [`inputs`]: the Inputs section, the controls' quantization and the
//!   numbered commands.
//! - [`entity`]: the entities a snapshot carries, their quantized states and
//!   their records against a baseline.
//! - [`snapshot`]: the Snapshot section, its header, and the per-connection
//!   baselines: [`snapshot::EntitySender`] on the host and
//!   [`snapshot::EntityReceiver`] on the client.
//! - [`priority`]: the relevance bands and the priority accumulator.
//! - [`readout`]: the cockpit readout inside a snapshot, against the one
//!   the client acknowledged.
//! - [`space`]: how a snapshot packet's 1,200 bytes are shared.
//! - [`events`]: the Events section, the host's queue and the client's
//!   receiver.
//! - [`names`]: the name table the Names message fills.
//! - [`own_state`]: the Own state section, the exact state of the player's
//!   plane against an acknowledged one.
//! - [`messages`]: the reliable message bodies.
//! - [`connection`]: the host's and the client's per-connection wire state,
//!   which tie the parts above to the transport's packet numbers.
//! - [`from_world`]: filling the wire's plain data from `tore-world`'s.
//!
//! Every decoder is bounded: it checks each count and length against the
//! protocol's limits before reading on, and returns a [`WireError`] for any
//! bytes, never panicking.

pub mod connection;
pub mod entity;
pub mod events;
pub mod from_world;
pub mod inputs;
pub mod messages;
pub mod names;
pub mod own_state;
pub mod priority;
pub mod readout;
pub mod snapshot;
pub mod space;

mod bits;
mod flat;

#[cfg(test)]
mod fuzz_tests;
#[cfg(test)]
mod golden_tests;
#[cfg(test)]
mod lossy_tests;
#[cfg(test)]
mod readout_tests;
#[cfg(test)]
mod round_trip_tests;
#[cfg(test)]
pub(crate) mod samples;

use std::fmt;
use tore_codec::CodecError;

/// The protocol version: one number for every byte of the protocol, the
/// transport's included. Any change to the bytes raises it; the wire golden
/// test fails until it is raised and the committed copy refreshed
/// (`TORE_UPDATE_WIRE_GOLDEN=1`).
pub const PROTOCOL_VERSION: u16 = 2;

/// Section kinds after the transport's own Messages (kind 1).
/// The tick of each interval at which a seat's snapshots are built: ticks
/// where the tick modulo `ticks_per_snapshot` is this phase. Seats are spread
/// over the interval by their numbers, so a host with many players never
/// builds every snapshot on one tick (D10 follow-up, agent decision). The
/// client keeps the own-state hash at the same ticks.
pub fn snapshot_phase(seat: u8, ticks_per_snapshot: u32) -> u64 {
    u64::from(seat) % u64::from(ticks_per_snapshot.max(1))
}

pub const SECTION_INPUTS: u8 = 2;
/// The Snapshot section.
pub const SECTION_SNAPSHOT: u8 = 3;
/// The Events section.
pub const SECTION_EVENTS: u8 = 4;
/// The Own state section.
pub const SECTION_OWN_STATE: u8 = 5;

/// Why a section or a message could not be written or read.
#[derive(Debug, Clone, PartialEq)]
pub enum WireError {
    /// The bits themselves: the data ended early, a field was out of its
    /// width, or a form the writer never produces.
    Codec(CodecError),
    /// A count over the protocol's limit.
    TooMany {
        /// What was counted.
        what: &'static str,
        /// The limit.
        limit: usize,
    },
    /// A value the protocol does not allow, named.
    Invalid(&'static str),
    /// Bits other than zero padding after the body.
    Trailing,
    /// The own plane's exact state could not be coded or rebuilt.
    Exact(String),
    /// More unacknowledged events than the protocol allows: the connection is
    /// too far behind and ends.
    TooFarBehind,
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "{error}"),
            Self::TooMany { what, limit } => write!(f, "more than {limit} {what}"),
            Self::Invalid(what) => write!(f, "invalid {what}"),
            Self::Trailing => f.write_str("data after the end of the body"),
            Self::Exact(error) => write!(f, "own state: {error}"),
            Self::TooFarBehind => f.write_str("too many unacknowledged events"),
        }
    }
}

impl std::error::Error for WireError {}

impl From<CodecError> for WireError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

impl From<tore_sim::flight::exact::ExactError> for WireError {
    fn from(error: tore_sim::flight::exact::ExactError) -> Self {
        Self::Exact(format!("{error:?}"))
    }
}

/// A result of the wire.
pub type WireResult<T> = Result<T, WireError>;

/// The protocol's limits (net-protocol.md, "Limits") that the game's
/// sections check.
pub mod limits {
    /// Aircraft records in one snapshot.
    pub const AIRCRAFT: usize = 64;
    /// Projectile records in one snapshot.
    pub const PROJECTILES: usize = 256;
    /// Debris records in one snapshot.
    pub const DEBRIS: usize = 256;
    /// Ejected pilot records in one snapshot.
    pub const PILOTS: usize = 64;
    /// Entries of a connection's name table.
    pub const NAMES: usize = 4096;
    /// Input ticks in one Inputs section.
    pub const INPUT_TICKS: usize = 24;
    /// Commands in one Inputs section.
    pub const COMMANDS: usize = 64;
    /// Unacknowledged events per connection.
    pub const EVENTS: usize = 1024;
    /// Seats a host has.
    pub const SEATS: usize = 30;
    /// A reassembled reliable message, bytes.
    pub const MESSAGE: usize = 65_536;
    /// Recording stems in one radio call or order voice.
    pub const STEMS: usize = 32;
}
