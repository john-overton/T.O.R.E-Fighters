//! The coders of combat in the world (the combat section): the wrapper's triggers, poses, contrails and render history, restored in place around `live::State`.
//!
//! Stage H slice H3b (combat effects and wrapper) fills this in; until then it reports itself not
//! covered (docs/ARCHITECTURE.md, "How stage H lands").

use tore_sim::checkpoint::{CheckpointError, InPlace, Loader, Saver, not_covered};

impl InPlace for super::Combat {
    fn save_in_place(&self, _: &mut Saver) -> Result<(), CheckpointError> {
        not_covered("combat::Combat")
    }
    fn restore_in_place(&mut self, _: &mut Loader<'_>) -> Result<(), CheckpointError> {
        not_covered("combat::Combat")
    }
}
