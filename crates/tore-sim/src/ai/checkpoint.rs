//! The coder of the AI's random stream, whose state is private to this
//! module (docs/formats/checkpoint.md). The other AI coders sit beside their
//! modules (`<module>_checkpoint.rs`).

use super::{DecisionRandom, DrawLog};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver};

/// The generator's one word of state. The draw log is a why-record: written
/// for the replay debug panels, never read by a decision, and outside
/// equality, so a restored stream starts with an empty log.
impl Checkpoint for DecisionRandom {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        let DecisionRandom { state, log: _ } = self;
        state.save(s, base.map(|b| &b.state))
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(DecisionRandom {
            state: Checkpoint::load(l, base.map(|b| &b.state))?,
            log: DrawLog::default(),
        })
    }
}
