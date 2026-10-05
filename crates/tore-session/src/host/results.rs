//! The results at a mission's end (stage F phase 2; docs/ARCHITECTURE.md,
//! "The multiplayer debrief"): every plane's row, with its pilots and the
//! scores, sent to every connection.
//!
//! Slice F2-0 adds the hook the end of a mission calls, which does nothing;
//! slice F2-D fills it.

use super::Host;
use crate::wire::messages::EndReason;

impl Host {
    /// At the mission's end, before Mission ended: sends Results. Nothing
    /// yet.
    pub(super) fn send_results(&mut self, _reason: EndReason) {}
}
