//! The coders of smoke and contrail puffs and their outlets.
//!
//! Stage H slice H3b (combat effects) fills this in; until then it reports itself not
//! covered (docs/ARCHITECTURE.md, "How stage H lands").

use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, not_covered};

impl Checkpoint for super::Smoke {
    fn save(&self, _: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        not_covered("combat::smoke::Smoke")
    }
    fn load(_: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        not_covered("combat::smoke::Smoke")
    }
}
