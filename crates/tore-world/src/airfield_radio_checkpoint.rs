//! The coders of the tower conversation of each cockpit and of the AI wingmen's airfield reports (the wing status section).
//!
//! Stage H slice H7 (radio) fills this in; until then it reports itself not
//! covered (docs/ARCHITECTURE.md, "How stage H lands").

use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, not_covered};

impl Checkpoint for super::AirfieldRadio {
    fn save(&self, _: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        not_covered("airfield_radio::AirfieldRadio")
    }
    fn load(_: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        not_covered("airfield_radio::AirfieldRadio")
    }
}

impl Checkpoint for super::WingStatus {
    fn save(&self, _: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        not_covered("airfield_radio::WingStatus")
    }
    fn load(_: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        not_covered("airfield_radio::WingStatus")
    }
}
