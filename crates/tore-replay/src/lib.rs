//! Mission recordings: the bounded binary format, its reader, and the debug
//! log, summary, Tacview and comparison exports. No dependencies, no renderer
//! and no simulation types; the app converts its own state into this model.
//! Opinionated addition requested by John on 2026-09-26; see docs/REPLAYS.md.
//!
//! Write with [`Writer`], read with [`Recording`], export with [`export`].
//! Event kinds, field names, channels and units live in [`vocab`].

mod checksum;
mod chunk;
mod city;
mod codec;
pub mod display;
mod display_detail;
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
pub mod stores;
mod strings;
mod tilts;
mod trees;
pub mod vocab;
mod writer;

pub use checksum::state_checksum;
pub use city::CityTrack;
pub use display::{HudSample, SightSample, Track as DisplayTrack};
pub use error::{Error, Result};
pub use model::*;
pub use predict::precision;
pub use reader::{
    CONTRAIL_PUFF_CAP, ChunkInfo, FrameIter, LONGEST_PUFF_TICKS, LiveEffect, LivePuff, Peek,
    Recording, SMOKE_PUFF_CAP, Spawns, TimedEvent,
};
pub use writer::{Writer, WriterOptions, partial_path};

/// The newest readable format. Live recorders request version 4 for 24 Hz
/// display samples. Synthetic and legacy writers can still request 2 or 3.
pub const FORMAT_VERSION: u16 = 4;
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
