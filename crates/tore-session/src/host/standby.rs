//! The host's standby stream (stage K; docs/ARCHITECTURE.md, "Standbys"):
//! appointing and dismissing up to two standbys, the records out with the
//! snapshots, the checks and the checkpoints, the pacing and each standby's
//! status. Slice K0 places the seam; slice K3 builds it.

use super::{ConnectionId, Host, NOT_AVAILABLE};
use crate::wire::migration::StandbyStatus;

/// The host's standbys and their streams (slice K3 fills it).
#[derive(Debug, Default)]
pub(super) struct Standbys {}

impl Host {
    /// A standby's status (message 47). Not built yet: refused.
    pub(super) fn standby_status(
        &mut self,
        connection: ConnectionId,
        status: StandbyStatus,
    ) -> Result<(), String> {
        let _ = (connection, status, &self.standbys);
        Err(NOT_AVAILABLE.into())
    }
}
