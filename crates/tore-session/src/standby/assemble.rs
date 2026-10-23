//! A checkpoint's assembly from its chunks (slice K2; docs/ARCHITECTURE.md,
//! "Standbys", and the records in docs/formats/net-protocol.md, "The standby
//! stream"): a Checkpoint begin, then its chunks between other records, each
//! at most 4 KB. The whole is checked before a standby keeps it: the
//! container's CRC-32, its tick against the begin's and its mission against
//! the standby's own build.
//!
//! *Agent decisions (K2):* the host cuts a checkpoint into chunks of exactly
//! [`CHUNK_BYTES`] but the last, so the begin's chunk count must be the
//! length's; chunks may come in any order, each once; a checkpoint is at most
//! [`MAX_CHECKPOINT_BYTES`]. Any break of these is damage, and the standby
//! asks for the checkpoint again.

use crate::journal::{CheckpointBegin, CheckpointChunk};
use crate::wire::migration::limits::CHUNK_BYTES;

/// The longest checkpoint a standby assembles: 16 times the 1.0 MB measured
/// for a 15 against 15 furball (agent decision, K2).
pub const MAX_CHECKPOINT_BYTES: usize = 16 << 20;

/// Why a checkpoint was not kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Damaged or out of order: the standby asks for it again.
    Damaged(String),
    /// Built for another mission than the standby's own build: the standby
    /// reports itself behind and the host dismisses it.
    Foreign { theirs: u64, ours: u64 },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Damaged(why) => write!(f, "damaged checkpoint: {why}"),
            Self::Foreign { theirs, ours } => write!(
                f,
                "a checkpoint of another mission: identity {theirs:016x}, ours {ours:016x}"
            ),
        }
    }
}

fn damaged<T>(why: impl Into<String>) -> Result<T, Refusal> {
    Err(Refusal::Damaged(why.into()))
}

/// A checkpoint being assembled.
#[derive(Debug)]
pub struct Assembly {
    begin: CheckpointBegin,
    chunks: Vec<Option<Vec<u8>>>,
    received: usize,
}

impl Assembly {
    /// Starts assembling the checkpoint `begin` announces.
    pub fn begin(begin: CheckpointBegin) -> Result<Self, Refusal> {
        let length = begin.length as usize;
        if length == 0 || length > MAX_CHECKPOINT_BYTES {
            return damaged(format!("a checkpoint of {length} bytes"));
        }
        let chunks = length.div_ceil(CHUNK_BYTES);
        if usize::from(begin.chunks) != chunks {
            return damaged(format!(
                "{} chunks for {length} bytes, not {chunks}",
                begin.chunks
            ));
        }
        Ok(Self {
            begin,
            chunks: vec![None; chunks],
            received: 0,
        })
    }

    /// The begin record this assembly follows.
    pub fn tick(&self) -> u64 {
        u64::from(self.begin.tick)
    }

    /// Adds a chunk. Returns the checkpoint's bytes once every chunk is in,
    /// not yet checked ([`check`]).
    pub fn add(&mut self, chunk: CheckpointChunk) -> Result<Option<Vec<u8>>, Refusal> {
        let index = usize::from(chunk.index);
        let count = self.chunks.len();
        let Some(slot) = self.chunks.get_mut(index) else {
            return damaged(format!("chunk {index} of {count}"));
        };
        if slot.is_some() {
            return damaged(format!("chunk {index} twice"));
        }
        let length = self.begin.length as usize;
        let expected = if index + 1 == count {
            length - index * CHUNK_BYTES
        } else {
            CHUNK_BYTES
        };
        if chunk.bytes.len() != expected {
            return damaged(format!(
                "chunk {index} of {} bytes, not {expected}",
                chunk.bytes.len()
            ));
        }
        *slot = Some(chunk.bytes);
        self.received += 1;
        if self.received < count {
            return Ok(None);
        }
        let mut bytes = Vec::with_capacity(length);
        for chunk in self.chunks.drain(..) {
            bytes.extend(chunk.unwrap_or_default());
        }
        Ok(Some(bytes))
    }
}

/// Checks a whole checkpoint's container without restoring it: the CRC-32,
/// the framing, its tick against the begin's `tick` and its mission identity
/// against `identity`, the standby's own build's.
pub fn check(bytes: &[u8], tick: u64, identity: u64) -> Result<(), Refusal> {
    let layout = match tore_world::checkpoint::layout(bytes) {
        Ok(layout) => layout,
        Err(error) => return damaged(error.to_string()),
    };
    if layout.tick != tick {
        return damaged(format!(
            "the checkpoint holds tick {}, its begin said {tick}",
            layout.tick
        ));
    }
    if layout.identity != identity {
        return Err(Refusal::Foreign {
            theirs: layout.identity,
            ours: identity,
        });
    }
    Ok(())
}

/// Cuts a checkpoint into its begin and chunks, as the host sends it: the
/// writer's side of [`Assembly`], for slice K3 and the tests.
pub fn cut(tick: u64, bytes: &[u8]) -> (CheckpointBegin, Vec<CheckpointChunk>) {
    let chunks: Vec<CheckpointChunk> = bytes
        .chunks(CHUNK_BYTES)
        .enumerate()
        .map(|(index, bytes)| CheckpointChunk {
            index: index as u16,
            bytes: bytes.to_vec(),
        })
        .collect();
    let begin = CheckpointBegin {
        tick: tick as u32,
        length: bytes.len() as u32,
        chunks: chunks.len() as u16,
    };
    (begin, chunks)
}
