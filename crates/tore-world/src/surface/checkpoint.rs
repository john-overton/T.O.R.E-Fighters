//! The coders of the surface's changing state, coded in the combat section
//! beside `live::State` (docs/formats/checkpoint.md). The units' hit points
//! are their combat target rows, which `live::State`'s coder already carries
//! (template ids included); this codes the per-unit state the surface slices
//! add, with the digest of the surface it belongs to.
use super::{
    BatteryState, Engager, MountStock, RadarState, SurfaceState, SurfaceUnitState, UnitId,
    movement::{Halt, Mover},
    supply::Resupply,
    units::Seen,
};
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

impl Checkpoint for UnitId {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        self.0.save(s, base.map(|b| &b.0))
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(Self(u32::load(l, base.map(|b| &b.0))?))
    }
}

tore_sim::checkpoint_struct!(MountStock {
    loaded,
    reserve,
    ordinal,
});

tore_sim::checkpoint_enum!(Halt {
    Moving = 0,
    Arrived = 1,
    Destroyed = 2,
});

tore_sim::checkpoint_struct!(Mover {
    x,
    y,
    z,
    heading,
    pitch,
    bank,
    speed,
    leg,
    halt,
});

tore_sim::checkpoint_struct!(Resupply { rearm, refill });

tore_sim::checkpoint_struct!(Seen {
    target,
    tick,
    position,
    velocity,
});

tore_sim::checkpoint_struct!(Engager {
    controller,
    aim_error,
    holding,
    seen,
});

tore_sim::checkpoint_struct!(RadarState {
    on,
    last_hostile,
    shutdown_until,
});

tore_sim::checkpoint_struct!(BatteryState {
    controller,
    optical
});

tore_sim::checkpoint_struct!(SurfaceUnitState {
    id,
    mover,
    armed,
    engagers,
    mounts,
    radar,
    supply,
    resupply,
});

// The trace, locks, painting and places are rebuilt every surface tick.
tore_sim::checkpoint_struct!(SurfaceState {
    digest,
    units,
    batteries,
    rng,
    harm_rolled,
} skip {
    trace = Vec::new(),
    locks = Vec::new(),
    painting = Vec::new(),
    places = Default::default(),
});

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
