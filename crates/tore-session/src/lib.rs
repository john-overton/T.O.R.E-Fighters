//! The game's side of networking: the wire messages and the host session
//! now, and in later slices the client session (prediction, interpolation,
//! clock steering, readouts) and the headless bot client. See
//! docs/ARCHITECTURE.md, "Network sessions".
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

pub mod host;
pub mod wire;

pub use host::{
    AfterEnd, BuildId, CommandError, Host, HostConfig, HostError, HostLog, HostStatus, LeaveReason,
    OpenPlanes, Phase, PlayerStatus, StartMode,
};
