//! The host's journal (stage K; docs/ARCHITECTURE.md, "The journal: one door
//! into the world"). Slice K0 steps the host's world through
//! [`crate::journal::apply_tick`] from the start, so the host and a standby
//! share one code path, and hands each tick's [`Tick`] here after the step;
//! slice K1 builds the `Driver` that owns the world, the Flight, Tick and
//! Ended records, and the queue the standby stream drains.

use super::Host;
use crate::journal::Tick;

/// The journal's records on their way to the standby stream (slice K1 fills
/// it; slice K3 drains it).
#[derive(Debug, Default)]
pub(super) struct Journal {
    /// The ticks the host stepped, kept whole when a test asks
    /// ([`Host::keep_journal`]).
    #[cfg(test)]
    pub(super) kept: Option<Vec<Tick>>,
}

impl Host {
    /// The tick the host just stepped, as the journal records it. Nothing is
    /// recorded yet (slice K1).
    pub(super) fn journal_ticked(&mut self, tick: Tick) {
        #[cfg(test)]
        if let Some(kept) = &mut self.journal.kept {
            kept.push(tick);
            return;
        }
        let _ = (&self.journal, tick);
    }

    /// Keeps every tick the host steps from now on, for a test's twin.
    #[cfg(test)]
    pub(crate) fn keep_journal(&mut self) {
        self.journal.kept = Some(Vec::new());
    }

    /// The ticks kept since [`Host::keep_journal`], leaving none.
    #[cfg(test)]
    pub(crate) fn take_journal(&mut self) -> Vec<Tick> {
        self.journal
            .kept
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// The flying mission's spec text, which a twin is built from.
    #[cfg(test)]
    pub(crate) fn flight_spec_text(&self) -> &str {
        &self.spec_text
    }
}
