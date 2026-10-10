//! The coders of the surface's changing state, coded in the combat section
//! beside `live::State` (docs/formats/checkpoint.md). The units' hit points
//! are their combat target rows, which `live::State`'s coder already carries
//! (template ids included); this codes the per-unit state the surface slices
//! add, with the digest of the surface it belongs to.
use super::{SurfaceState, SurfaceUnitState, UnitId};
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

impl Checkpoint for UnitId {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        self.0.save(s, base.map(|b| &b.0))
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(Self(u32::load(l, base.map(|b| &b.0))?))
    }
}

tore_sim::checkpoint_struct!(SurfaceUnitState { id });

tore_sim::checkpoint_struct!(SurfaceState { digest, units });

impl SurfaceState {
    /// Refuses a decoded state that belongs to another surface than
    /// `fresh`, the state of the world it restores over: a different digest,
    /// or units out of the surface's order.
    pub(crate) fn check_against(&self, fresh: &SurfaceState) -> Result<(), CheckpointError> {
        if self.digest != fresh.digest {
            return invalid(format!(
                "the checkpoint's surface {:#018x} is not this mission's {:#018x}",
                self.digest, fresh.digest
            ));
        }
        if self.units.len() != fresh.units.len()
            || self
                .units
                .iter()
                .zip(&fresh.units)
                .any(|(a, b)| a.id != b.id)
        {
            return invalid("the checkpoint's surface units are not this mission's");
        }
        Ok(())
    }
}
