//! Taking over and resuming (stage K; docs/ARCHITECTURE.md, "Losing the
//! host"): `Host::resume` from a standby's world and parts, the players
//! absent until they resume, the resume window and the fast-forward, Resume,
//! Resumed and Backlog, Taken over and the handover. Slice K0 places the
//! seam; slice K4 builds it.

use super::{ConnectionId, Host, NOT_AVAILABLE};
use crate::wire::migration::{Backlog, Resume, TakenOver};

/// The migration's state on the host: the absent players and the resume
/// window (slice K4 fills it).
#[derive(Debug, Default)]
pub(super) struct Resuming {}

impl Host {
    /// A player resumes with this host (message 48). Not built yet: refused.
    pub(super) fn resume_request(
        &mut self,
        connection: ConnectionId,
        resume: Resume,
    ) -> Result<(), String> {
        let _ = (connection, resume, &self.resuming);
        Err(NOT_AVAILABLE.into())
    }

    /// A resumed player's backlog (message 50). Not built yet: refused.
    pub(super) fn backlog(
        &mut self,
        connection: ConnectionId,
        backlog: &Backlog,
    ) -> Result<(), String> {
        let _ = (connection, backlog);
        Err(NOT_AVAILABLE.into())
    }

    /// A new host says it has taken over (message 52). Not built yet:
    /// refused.
    pub(super) fn taken_over(
        &mut self,
        connection: ConnectionId,
        taken: TakenOver,
    ) -> Result<(), String> {
        let _ = (connection, taken);
        Err(NOT_AVAILABLE.into())
    }
}
