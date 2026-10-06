//! The client's side of host selection (stage K; docs/ARCHITECTURE.md, "Host
//! selection"): the Candidate report and the CPU measure, the reach tests
//! and their report, the upload test. Slice K0 places the seam; slice K6
//! builds it.

use super::Client;
use crate::wire::messages::Message;

/// The client's host-selection state (slice K6 fills it).
#[derive(Debug, Default)]
pub(super) struct Candidacy {}

impl Client {
    /// The Candidate report and the tests' timers, from `Client::update`
    /// (slice K6).
    pub(super) fn candidate_update(&mut self, now: std::time::Duration) {
        let _ = (now, &self.candidacy);
    }

    /// A Reach test, Reach peers or Upload test from the host. Answered by
    /// slice K6.
    pub(super) fn candidate_message(&mut self, message: Message) {
        let _ = message;
    }
}
