//! The coders of a sensor suite's state: selection, contacts, strobes, plots and trails.
//!
//! Stage H slice H1 (records and sensors) fills this in; until then it reports itself not
//! covered (docs/ARCHITECTURE.md, "How stage H lands").

use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, not_covered};

impl Checkpoint for super::Sensors {
    fn save(&self, _: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        not_covered("sensors::Sensors")
    }
    fn load(_: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        not_covered("sensors::Sensors")
    }
}
