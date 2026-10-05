//! A host's side of the relay: the Relay open the master sends, its ack, the
//! channels' relayed addresses, Relay frames and Relay close (stage J's slice
//! J3, "The relay" in the architecture guide).
//!
//! Until J3 this is the dispatch only: the [`super::Rendezvous`] hands every
//! relay packet here and it is dropped, counted, with no answer, so the
//! master sees the host never acknowledge a channel and tells the player so.

use super::packet::MasterPacket;

/// What a host does with a relay packet (Relay open, a Relay frame, Relay
/// close). Before slice J3: nothing.
pub(super) fn dispatch(_packet: &MasterPacket, dropped: &mut u64) {
    *dropped += 1;
}
