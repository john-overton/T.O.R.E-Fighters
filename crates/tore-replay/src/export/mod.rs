//! Exports, all written to any `std::io::Write`:
//!
//! - [`jsonl`]: the machine-readable debug log, one JSON object per line.
//! - [`summary`]: the plain-English mission summary.
//! - [`anomaly`]: anomaly flags, used by both of the above.
//! - [`acmi`]: Tacview ACMI 2.2 text.
//! - [`diff`]: comparing two recordings.

pub mod acmi;
pub mod anomaly;
pub mod diff;
mod json;
pub mod jsonl;
pub mod summary;
mod text;

pub use acmi::{
    AcmiOptions, AcmiStats, THEATER_ANCHORS, TheaterAnchor, theater_anchor, write_acmi,
};
pub use anomaly::{Anomaly, Thresholds, detect};
pub use diff::{CompareOptions, Comparison, compare, write_diff};
pub use jsonl::{JsonlOptions, JsonlStats, write_jsonl};
pub use summary::{SummaryOptions, write_summary};
