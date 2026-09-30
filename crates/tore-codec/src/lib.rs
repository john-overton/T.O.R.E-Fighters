//! Small, general encoding tools for T.O.R.E's network and checkpoint formats.
//!
//! This crate uses only the standard library and knows nothing about the game.
//! It is shared by the wire protocol
//! ([`docs/formats/net-protocol.md`](../../../docs/formats/net-protocol.md)),
//! the exact own-plane coder (stage D4) and the stage H checkpoints.
//!
//! What each part is for:
//!
//! - [`bits`]: [`BitWriter`] and [`BitReader`], which pack fields least
//!   significant bit first into bytes in order, exactly as the protocol's
//!   "Overview" says. Every read is bounded and returns a [`CodecError`]
//!   rather than panicking, whatever the bytes.
//! - [`ints`]: variable-length integers (7-bit groups with a continuation
//!   bit, zigzag for signed values) and the bucketed signed coding for small
//!   residuals.
//! - [`quant`]: quantizers that turn a float into a whole number of steps and
//!   back, angles as a fraction of a turn, and the unit-range codings.
//! - [`xor`]: exact floats and integers against a baseline, using the
//!   exclusive-or scheme of the Gorilla time-series paper.
//! - [`text`]: short length-prefixed UTF-8 strings.
//! - [`hash`]: FNV-1a 64 and CRC-32 (IEEE), one-shot and incremental.
//!
//! Decoding is strict: every value has one encoding and the readers reject
//! any other (over-long integers, padded bucket choices, unused escape
//! forms). The writers only produce the canonical form.

pub mod bits;
pub mod hash;
pub mod ints;
pub mod quant;
pub mod text;
pub mod xor;

mod error;

pub use bits::{BitReader, BitWriter};
pub use error::CodecError;
pub use hash::{Crc32, Fnv1a64, crc32, fnv1a64};
