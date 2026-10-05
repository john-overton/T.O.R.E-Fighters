//! Relay channels, keys, rates, idle, the monthly allowance and its file
//! ("Relay" in the master protocol, "The relay's monthly allowance" in the
//! operations guide).
//!
//! Slice J3 builds them. Until then (slice I2) the dispatch exists and every
//! Relay request, Relay open ack, Relay frame and Relay close is dropped and
//! counted. The relay's settings are read and checked already, so a
//! configuration written now keeps working.

use std::net::SocketAddr;
use std::time::Duration;

use tore_net::master::{RelayClose, RelayFrame, RelayOpenAck, RelayRequest};

/// The relay's settings ("The configuration file" in the operations guide).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelaySettings {
    /// Whether to relay at all.
    pub on: bool,
    /// Relayed pairs at once.
    pub channels: u32,
    /// Relayed pairs one player's address may have.
    pub channels_per_source: u32,
    /// Each channel's limit, each way, in KB/s.
    pub rate_kb: u32,
    /// Relayed gigabytes sent out each calendar month (UTC); new channels
    /// are refused at 95 percent of it (John, 2026-10-05).
    pub month_gb: u32,
}

impl Default for RelaySettings {
    fn default() -> Self {
        Self {
            on: true,
            channels: 64,
            channels_per_source: 2,
            rate_kb: 64,
            month_gb: 800,
        }
    }
}

/// The relay: no channels until slice J3.
#[derive(Debug, Clone, Default)]
pub struct Relays {
    /// Packets dropped because the relay is not built, every kind together.
    pub dropped: u64,
}

impl Relays {
    /// A player's Relay request.
    pub fn request(&mut self, now: Duration, from: SocketAddr, packet: &RelayRequest) {
        let _ = (now, from, packet);
        self.dropped += 1;
    }

    /// A host's Relay open ack.
    pub fn open_ack(&mut self, now: Duration, from: SocketAddr, packet: &RelayOpenAck) {
        let _ = (now, from, packet);
        self.dropped += 1;
    }

    /// A Relay frame, read in place.
    pub fn frame(&mut self, now: Duration, from: SocketAddr, frame: &RelayFrame<'_>) {
        let _ = (now, from, frame);
        self.dropped += 1;
    }

    /// A Relay close from an end.
    pub fn close(&mut self, now: Duration, from: SocketAddr, packet: &RelayClose) {
        let _ = (now, from, packet);
        self.dropped += 1;
    }

    /// Idle channels, rates, the allowance.
    pub fn update(&mut self, now: Duration) {
        let _ = now;
    }

    /// Channels open.
    pub fn channels(&self) -> usize {
        0
    }

    /// Bytes relayed out this calendar month.
    pub fn month_bytes(&self) -> u64 {
        0
    }
}
