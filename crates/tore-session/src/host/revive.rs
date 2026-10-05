//! Death and revival on the host (stage F phase 2; docs/ARCHITECTURE.md,
//! "Death, revival and lives"): the respawn rule, lives and the delay, and
//! the tick's Abandon and Revive commands.
//!
//! Slice F2-0 adds the hooks the host calls, each refusing or doing nothing;
//! slice F2-V fills them.

use super::{ConnectionId, Host, NOT_AVAILABLE};
use tore_world::seats::PlaneId;
use tore_world::world::MissionCommand;

impl Host {
    /// The player asks to fly again after a loss (message 28), answered by
    /// a Seated message or a refusal. The caller has checked the mission's
    /// number.
    pub(super) fn revive_request(&mut self, _connection: ConnectionId) -> Result<(), String> {
        Err(NOT_AVAILABLE.into())
    }

    /// Why the revival rules refuse `connection` taking `plane` now, if they
    /// do: a player whose plane was lost joins only by those rules. Nothing
    /// yet.
    pub(super) fn revive_take_refusal(
        &self,
        _connection: ConnectionId,
        _plane: PlaneId,
    ) -> Option<String> {
        None
    }

    /// The tick's revivals and abandoned planes, added to `commands` before
    /// the step. Nothing yet.
    pub(super) fn revive_commands(&mut self, _tick: u64, _commands: &mut Vec<MissionCommand>) {}
}
