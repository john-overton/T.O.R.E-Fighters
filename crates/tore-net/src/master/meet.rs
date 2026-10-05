//! A host's side of introductions: the Meet a master sends when a player asks
//! to be introduced, the punches it asks for and the Meet ack (stage J's
//! slice J2, "Hole punching" in the architecture guide).
//!
//! Until J2 this is the dispatch only: the [`super::Rendezvous`] hands every
//! Meet here and it is dropped, counted, with no answer, so a master that
//! introduces a player to a host of this build gets nothing back and its
//! player's join falls back on the host's seen address.

use super::packet::Meet;

/// What a host does with a Meet. Before slice J2: nothing.
pub(super) fn dispatch(_meet: &Meet, dropped: &mut u64) {
    *dropped += 1;
}
