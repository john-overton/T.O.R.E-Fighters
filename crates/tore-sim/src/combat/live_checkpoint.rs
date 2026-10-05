//! The coders of combat's state: ownships, targets, projectiles, effects, the ledger, the rewind history, random streams and the combat tick.
//!
//! Stage H slice H3a (combat core) fills this in; until then it reports itself not
//! covered (docs/ARCHITECTURE.md, "How stage H lands").

use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, not_covered};

impl Checkpoint for super::State {
    fn save(&self, _: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        not_covered("combat::live::State")
    }
    fn load(_: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        not_covered("combat::live::State")
    }
}
