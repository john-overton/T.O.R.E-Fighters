//! Scoring on the host (stage F phase 2; docs/ARCHITECTURE.md, "Scoring"):
//! the tallies by player and side, the limits and the Scores message.
//!
//! Slice F2-0 adds the hook the tick calls, which does nothing; slice F2-S
//! fills it.

use super::Host;
use tore_world::world::TickOutput;

impl Host {
    /// After the step: drains the tick's score facts into the tallies, ends
    /// the mission at a kill limit, and sends Scores when they change.
    /// Nothing yet.
    pub(super) fn score_tick(&mut self, _tick: u64, _out: &TickOutput) {}
}
