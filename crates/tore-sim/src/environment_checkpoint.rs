//! The coders of the weather clock: its ticks, remainder, fog selection and random stream. The configuration is mission setup.
//!
//! Stage H slice H2 (world shell) fills this in; until then it reports itself not
//! covered (docs/ARCHITECTURE.md, "How stage H lands").

use crate::checkpoint::{CheckpointError, InPlace, Loader, Saver, not_covered};

impl InPlace for super::Environment {
    fn save_in_place(&self, _: &mut Saver) -> Result<(), CheckpointError> {
        not_covered("environment::Environment")
    }
    fn restore_in_place(&mut self, _: &mut Loader<'_>) -> Result<(), CheckpointError> {
        not_covered("environment::Environment")
    }
}
