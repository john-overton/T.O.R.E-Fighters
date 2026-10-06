//! Candidates and host selection (stage K; docs/ARCHITECTURE.md, "Host
//! selection"): each game's Candidate report, the reach and upload tests,
//! eligibility and ranking, the succession, setting 21's pin and the King's
//! warnings. Slice K0 places the seam; slice K6 builds it.

use super::{ConnectionId, Host, NOT_AVAILABLE};
use crate::settings::CALCULATED_HOST;
use crate::wire::migration::{CandidateReport, ReachReport};
use std::time::Duration;

/// What the host knows of each candidate (slice K6 fills it).
#[derive(Debug, Default)]
pub(super) struct Succession {
    /// Filler sections' bytes received in all (protocol 13): an upload
    /// test's measure, counted and otherwise ignored.
    pub(super) filler_bytes: u64,
}

impl Host {
    /// A player's Candidate report (message 40). Not built yet: refused.
    pub(super) fn candidate_report(
        &mut self,
        connection: ConnectionId,
        report: &CandidateReport,
    ) -> Result<(), String> {
        let _ = (connection, report);
        Err(NOT_AVAILABLE.into())
    }

    /// A player's Reach report (message 43). Not built yet: refused.
    pub(super) fn reach_report(
        &mut self,
        connection: ConnectionId,
        report: &ReachReport,
    ) -> Result<(), String> {
        let _ = (connection, report);
        Err(NOT_AVAILABLE.into())
    }

    /// The timers of host selection, from `Host::update` (slice K6).
    pub(super) fn succession_update(&mut self, now: Duration) {
        let _ = now;
    }

    /// Why the King's pin of setting 21 is refused: until slice K6, any
    /// pin.
    pub(super) fn host_pin_refusal(&self, value: u32) -> Result<(), String> {
        if value == CALCULATED_HOST {
            Ok(())
        } else {
            Err(NOT_AVAILABLE.into())
        }
    }

    /// A Filler section from `connection`: counted, nothing more until the
    /// upload test (slice K6).
    pub(super) fn filler_received(&mut self, connection: ConnectionId, bytes: usize) {
        let _ = connection;
        self.succession.filler_bytes += bytes as u64;
    }
}
