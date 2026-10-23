//! The coder of an AI pilot's ejection monitor (docs/formats/checkpoint.md,
//! stage H slice H4): how long the danger has lasted and the pilot's own
//! random stream, which decides each check.

use super::Monitor;

crate::checkpoint_struct!(Monitor { ticks, random });
