//! The client's side of host selection (stage K; docs/ARCHITECTURE.md, "Host
//! selection"): the Candidate report and the CPU measure, the reach tests
//! and their report, the upload test. Slice K0 places the seam; slice K6
//! builds it.

use super::Client;
use crate::wire::messages::Message;

impl Client {
    /// A Reach test, Reach peers or Upload test from the host. Answered by
    /// slice K6.
    pub(super) fn candidate_message(&mut self, message: Message) {
        let _ = message;
    }
}
