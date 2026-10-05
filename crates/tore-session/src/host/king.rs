//! The King's lobby (stage F phase 2; docs/ARCHITECTURE.md, "The King, the
//! crown and the house" and "Slots, sides and joining"): passing the crown,
//! the King's settings and slot locks, and the King's rules on taking a
//! plane.
//!
//! Slice F2-0 adds the hooks the host calls, each refusing or doing nothing;
//! slice F2-1 fills them.

use super::{ConnectionId, Host, NOT_AVAILABLE};
use crate::wire::messages::{Lock, SettingsChange};
use tore_world::seats::PlaneId;

impl Host {
    /// The King gives the crown to the player with lobby id `player`
    /// (message 23). The caller has checked that `connection` is the King.
    pub(super) fn pass_crown(
        &mut self,
        _connection: ConnectionId,
        _player: u8,
    ) -> Result<(), String> {
        Err(NOT_AVAILABLE.into())
    }

    /// The King changes the lobby's settings, all or none (message 24). The
    /// caller has checked that `connection` is the King.
    pub(super) fn change_settings(
        &mut self,
        _connection: ConnectionId,
        _change: &SettingsChange,
    ) -> Result<(), String> {
        Err(NOT_AVAILABLE.into())
    }

    /// The King opens, closes or reserves `plane`'s slot (message 27). The
    /// caller has checked that `connection` is the King and the mission's
    /// number.
    pub(super) fn lock_slot(
        &mut self,
        _connection: ConnectionId,
        _plane: u32,
        _lock: &Lock,
    ) -> Result<(), String> {
        Err(NOT_AVAILABLE.into())
    }

    /// Why the King's rules refuse `connection` taking `plane` now, if they
    /// do: join in progress, lock sides, the slot's lock. Nothing yet.
    pub(super) fn king_take_refusal(
        &self,
        _connection: ConnectionId,
        _plane: PlaneId,
    ) -> Option<String> {
        None
    }
}
