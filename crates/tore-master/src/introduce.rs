//! Introductions, Meets and their retries, hints ("Introductions" in the
//! master protocol).
//!
//! Slice J2 builds them. Until then (slice I2) the dispatch exists and every
//! Introduce and Meet ack is dropped and counted, so the status line shows
//! that players asked.

use std::net::SocketAddr;
use std::time::Duration;

use tore_net::master::{Introduce, MeetAck};

/// Introductions under way: none until slice J2.
#[derive(Debug, Clone, Default)]
pub struct Introductions {
    /// Introduce packets dropped because introductions are not built.
    pub dropped_introduce: u64,
    /// Meet acks dropped.
    pub dropped_meet_ack: u64,
    /// Introductions made (the status line's `introductions/min`).
    pub introduced: u64,
}

impl Introductions {
    /// An Introduce from `from`, `len` bytes long.
    pub fn introduce(&mut self, now: Duration, from: SocketAddr, packet: &Introduce, len: usize) {
        let _ = (now, from, packet, len);
        self.dropped_introduce += 1;
    }

    /// A host's Meet ack.
    pub fn meet_ack(&mut self, now: Duration, from: SocketAddr, packet: &MeetAck) {
        let _ = (now, from, packet);
        self.dropped_meet_ack += 1;
    }

    /// Retries and forgetting.
    pub fn update(&mut self, now: Duration) {
        let _ = now;
    }

    /// Introductions under way.
    pub fn under_way(&self) -> usize {
        0
    }
}
