//! The imported records copied into mission state, coded by value: weapon
//! records, ownship and AI configurations (docs/formats/checkpoint.md,
//! "Shared records"). Callers code them through [`super::Saver::shared`], so
//! equal copies cost one coding.
//!
//! Stage H slice H1 (records and sensors) fills these in; until then they
//! report themselves not covered.

use super::{Checkpoint, CheckpointError, Loader, Saver, not_covered};

impl Checkpoint for tore_formats::weapons::Weapon {
    fn save(&self, _: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        not_covered("tore_formats::weapons::Weapon")
    }
    fn load(_: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        not_covered("tore_formats::weapons::Weapon")
    }
}

impl Checkpoint for crate::combat::live::Configuration {
    fn save(&self, _: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        not_covered("combat::live::Configuration")
    }
    fn load(_: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        not_covered("combat::live::Configuration")
    }
}
