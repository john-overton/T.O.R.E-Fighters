//! The coders of the cockpits section: every human-flown plane's `Cockpit`
//! (its flight through `tore_sim::checkpoint::save_flight`, turbulence and
//! its stream, airport service, NAV mode, message clocks, tower radio, crew
//! voice and result tracker), rebuilt as a list, since handoffs add and
//! remove cockpits.
//!
//! Stage H slice H2 (world shell) fills this in; until then it reports itself
//! not covered (docs/ARCHITECTURE.md, "How stage H lands").

use super::World;
use tore_sim::checkpoint::{CheckpointError, Loader, Saver, not_covered};

/// Writes every cockpit, in plane order.
pub(crate) fn save_cockpits(_world: &World, _s: &mut Saver) -> Result<(), CheckpointError> {
    not_covered("world::Cockpit")
}

/// Replaces the world's cockpits with the ones [`save_cockpits`] wrote.
pub(crate) fn restore_cockpits(
    _world: &mut World,
    _l: &mut Loader<'_>,
) -> Result<(), CheckpointError> {
    not_covered("world::Cockpit")
}
