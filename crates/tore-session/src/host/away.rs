//! The AI flies an idle player's aircraft (stage F phase 2;
//! docs/ARCHITECTURE.md, "The AI flies an idle player's aircraft"): Away and
//! Back, the stall's count and the reservation.
//!
//! Slice F2-0 adds the hooks the host calls, each refusing or doing nothing;
//! slice F2-A fills them.

use super::{ConnectionId, Host, NOT_AVAILABLE};
use tore_world::world::MissionCommand;

impl Host {
    /// The player's game has been away for the `idle-ai` setting's seconds
    /// (message 35).
    pub(super) fn away_request(&mut self, _connection: ConnectionId) -> Result<(), String> {
        Err(NOT_AVAILABLE.into())
    }

    /// The player is back at the controls (message 36), answered by a
    /// Seated message or a refusal.
    pub(super) fn back_request(&mut self, _connection: ConnectionId) -> Result<(), String> {
        Err(NOT_AVAILABLE.into())
    }

    /// The tick's handoffs for away and returning players, added to
    /// `commands` before the step. Nothing yet.
    pub(super) fn away_commands(&mut self, _tick: u64, _commands: &mut Vec<MissionCommand>) {}
}
