//! Rejoin tokens and reservations (stage K; docs/ARCHITECTURE.md, "Rejoin
//! tokens and reservations"): a token for every player, the gate admitting
//! a token holder, a dropped player's plane reserved for it, the King's
//! Release and a late Rejoin. Slice K0 places the seam; slice K5 builds it.
//!
//! F2-A's idle reservation (`Host::idle`) and stage K's rejoin reservation
//! become one table in slice K5: until then [`Host::reserved_for`] reads the
//! idle one, the seam where both meet.

use super::{ConnectionId, Host, NOT_AVAILABLE};
use tore_net::Token;
use tore_world::seats::PlaneId;

/// The session's tokens and reservations (slice K5 fills it).
#[derive(Debug, Default)]
pub(super) struct Rejoin {}

impl Host {
    /// A player joined: slice K5 issues its token here.
    pub(super) fn rejoin_connected(&mut self, connection: ConnectionId, token: Option<Token>) {
        let _ = (connection, token, &self.rejoin);
    }

    /// The callsign of the player `plane` is reserved for, for the lobby
    /// state's slot: an away player's plane, which the AI flies for it
    /// (slice F2-A). Slice K5 adds a dropped player's here.
    pub(super) fn reserved_for(&self, plane: PlaneId) -> Option<String> {
        self.idle
            .reserved
            .values()
            .find(|reserved| reserved.plane == plane)
            .map(|reserved| reserved.callsign.clone())
    }

    /// The King's Release of a reservation (message 53). Not built yet:
    /// refused.
    pub(super) fn release_request(
        &mut self,
        connection: ConnectionId,
        plane: u32,
    ) -> Result<(), String> {
        let _ = (connection, plane);
        Err(NOT_AVAILABLE.into())
    }

    /// A late Rejoin with a token (message 54). Not built yet: refused.
    pub(super) fn rejoin_request(
        &mut self,
        connection: ConnectionId,
        token: Token,
    ) -> Result<(), String> {
        let _ = (connection, token);
        Err(NOT_AVAILABLE.into())
    }
}
