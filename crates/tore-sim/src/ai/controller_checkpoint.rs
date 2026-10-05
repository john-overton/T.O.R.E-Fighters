//! The coders of an AI pilot's controller and everything it owns.
//!
//! Stage H slice H5 (AI controller) fills this in; until then it reports itself not
//! covered (docs/ARCHITECTURE.md, "How stage H lands").

use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, not_covered};

impl Checkpoint for super::Controller {
    fn save(&self, _: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        not_covered("ai::controller::Controller")
    }
    fn load(_: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        not_covered("ai::controller::Controller")
    }
}
