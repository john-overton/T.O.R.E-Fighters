//! Mission recordings: the bounded binary format, its reader, and the debug
//! log, summary, Tacview and comparison exports. No dependencies, no renderer
//! and no simulation types; the app converts its own state into this model.
//! Opinionated addition requested by John on 2026-09-26; see docs/REPLAYS.md.
//!
//! Write with [`Writer`], read with [`Recording`], export with [`export`].
//! Event kinds, field names, channels and units live in [`vocab`].

mod checksum;
mod chunk;
mod codec;
mod error;
mod events;
pub mod export;
mod format;
mod frames;
pub mod limits;
pub mod model;
mod predict;
mod reader;
mod rotors;
mod spawns;
mod strings;
mod tilts;
mod trees;
pub mod vocab;
mod writer;

pub use checksum::state_checksum;
pub use error::{Error, Result};
pub use model::*;
pub use predict::precision;
pub use reader::{
    CONTRAIL_PUFF_CAP, ChunkInfo, FrameIter, LONGEST_PUFF_TICKS, LiveEffect, LivePuff, Peek,
    Recording, SMOKE_PUFF_CAP, Spawns, TimedEvent,
};
pub use writer::{Writer, WriterOptions, partial_path};

/// The newest format version this build writes and reads. A recording is
/// written in the oldest version that holds it: version 3 only for a world
/// with a ground target, a redrawn airfield scene or surface units (see
/// [`World::needs_surface_format`]), version 2 for every other, byte for byte
/// as before.
pub const FORMAT_VERSION: u16 = 3;
/// The format version a recording is written in unless its world needs more:
/// the one from before the surface tracks.
pub const BASE_FORMAT_VERSION: u16 = 2;

#[cfg(test)]
mod tests {
    /// The app writes on its own thread and the viewer decodes on others.
    #[test]
    fn writer_and_recording_cross_threads() {
        fn send<T: Send>() {}
        fn share<T: Send + Sync>() {}
        send::<crate::Writer>();
        share::<crate::Recording>();
        share::<crate::Frame>();
        share::<crate::Header>();
    }
}
