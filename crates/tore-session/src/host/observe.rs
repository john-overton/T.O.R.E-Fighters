//! Observers on the host (stage F phase 2; docs/ARCHITECTURE.md, "The
//! observer view"): Observe and Observing, the observer flight's snapshots
//! and the delay ring.
//!
//! Slice F2-0 adds the hooks the host calls, each refusing or doing nothing;
//! slice F2-O1 fills them.

use super::{ConnectionId, Host, NOT_AVAILABLE};
use crate::wire::messages::Observe;
use std::time::Duration;

impl Host {
    /// A connection starts or stops watching, or moves its camera (message
    /// 33).
    pub(super) fn observe_request(
        &mut self,
        _connection: ConnectionId,
        _observe: Observe,
    ) -> Result<(), String> {
        Err(NOT_AVAILABLE.into())
    }

    /// After the seated players' snapshots: the observers'. Nothing yet.
    pub(super) fn observe_tick(&mut self, _tick: u64, _now: Duration) {}
}
