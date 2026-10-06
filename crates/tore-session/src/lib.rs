//! The game's side of networking: the wire messages, the host session, the
//! client session (prediction, interpolation, clock steering, readouts) and
//! the headless bot. See docs/ARCHITECTURE.md, "Network sessions".
//!
//! [`wire`] knows the game's messages and their bytes
//! ([`docs/formats/net-protocol.md`](../../../docs/formats/net-protocol.md),
//! from "Inputs" on) and the per-connection bookkeeping both ends need: the
//! acknowledged baselines, the priorities, the event queue and the name
//! table. It reads no clock and touches no socket or `World` step; the host
//! and client sessions drive it.
//!
//! [`host`] is the host session: one mission's `World` on a fixed 120 Hz
//! clock the caller drives with the time, the transport's server endpoint,
//! joins, each seat's input buffer, the tick's sorting into events, the
//! snapshots and the mission's lifecycle ([`Host`]).
//!
//! [`client`] is the client session, a player's game joined to a host with
//! no window or audio ([`Client`]): it predicts its own plane, draws the rest
//! in the past, and hands the game a frame each render, with a diagnostics
//! log and a capture that replays offline. [`bot`] flies it with a scripted
//! pilot, as the `tore-bot` program does.
//!
//! [`settings`] is the King's settings' registry and the host's store of
//! their values (stage F phase 2).
//!
//! [`journal`] is stage K's one door into a host's world: each tick's
//! changes, which the host steps with and a standby replays, and the
//! standby stream's records. [`standby`] is the standby's side (slice K2):
//! the state machine that replays the stream and its worker thread.

pub mod bot;
pub mod client;
#[cfg(feature = "test-support")]
pub mod fixture;
pub mod host;
pub mod journal;
#[cfg(test)]
mod journal_tests;
pub mod settings;
pub mod standby;
pub mod wire;

pub use client::capture;
pub use client::{
    Client, ClientConfig, ClientError, ClientEvent, ClientFrame, ClientPhase, ClientStats, Controls,
};
pub use host::{
    AfterEnd, BuildId, CommandError, CrownRule, Host, HostConfig, HostError, HostLog, HostStatus,
    LeaveReason, OpenPlanes, Phase, PlayerStatus, StartMode,
};
