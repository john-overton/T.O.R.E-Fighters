//! The coders of the route types an AI aircraft stores: a home position and
//! a waypoint's octant (docs/formats/checkpoint.md, stage H slice H4). The
//! rest of the module is pure functions and per-call inputs.

use super::{Octant, Position};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

crate::checkpoint_struct!(Position { x, z });

/// The sector, 0 to 7; a value above that in the bytes is refused.
impl Checkpoint for Octant {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        self.sector().save(s, None)
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let sector = u8::load(l, None)?;
        match Octant::new(sector) {
            Ok(octant) => Ok(octant),
            Err(_) => invalid(format!("octant {sector} is outside 0..8")),
        }
    }
}
