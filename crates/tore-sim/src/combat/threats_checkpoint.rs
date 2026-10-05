//! The coders of a missile-warning service's tracked threats.
//!
//! Stage H slice H1 (records and sensors) fills this in; until then it reports itself not
//! covered (docs/ARCHITECTURE.md, "How stage H lands").

use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, not_covered};

impl Checkpoint for super::ThreatService {
    fn save(&self, _: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        not_covered("combat::threats::ThreatService")
    }
    fn load(_: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        not_covered("combat::threats::ThreatService")
    }
}
