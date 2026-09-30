//! The game's side of networking: the wire messages now, and in later slices
//! the host session (clock, inputs, snapshots, joins), the client session
//! (prediction, interpolation, clock steering, readouts) and the headless bot
//! client. See docs/ARCHITECTURE.md, "Network sessions".
//!
//! [`wire`] knows the game's messages and their bytes
//! ([`docs/formats/net-protocol.md`](../../../docs/formats/net-protocol.md),
//! from "Inputs" on) and the per-connection bookkeeping both ends need: the
//! acknowledged baselines, the priorities, the event queue and the name
//! table. It reads no clock and touches no socket or `World` step; the host
//! and client sessions drive it.

pub mod wire;
