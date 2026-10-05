//! The coders of the AI wings (the AI wings section), restored in place around the AI mission.
//!
//! Stage H slice H6 (AI wings) fills this in; until then it reports itself not
//! covered (docs/ARCHITECTURE.md, "How stage H lands").

use tore_sim::checkpoint::{CheckpointError, InPlace, Loader, Saver, not_covered};

impl InPlace for super::AiWings {
    fn save_in_place(&self, _: &mut Saver) -> Result<(), CheckpointError> {
        not_covered("ai_wings::AiWings")
    }
    fn restore_in_place(&mut self, _: &mut Loader<'_>) -> Result<(), CheckpointError> {
        not_covered("ai_wings::AiWings")
    }
}
