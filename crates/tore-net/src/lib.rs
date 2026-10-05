//! T.O.R.E's network transport: UDP packets, the connection handshake,
//! acknowledgements and round trip, reliable ordered messages, statistics,
//! and an in-process network simulator.
//!
//! This crate uses only the standard library and `tore-codec`, and knows
//! nothing of the game: the game's payloads are opaque sections. Beside the
//! transport it holds what a host's real-time loop shares between the
//! dedicated server and a game that hosts: the server's dual-stack sockets
//! ([`ServerSocket`]), the in-process [`link`] a hosting game flies through,
//! and the sleep-then-spin wait ([`wait_until`]); and, for a joined game, the
//! [`Keepalive`] thread that speaks for it while its loop is stalled. The wire
//! format is [`docs/formats/net-protocol.md`](../../../docs/formats/net-protocol.md),
//! from "Overview" to "Reliable messages", "Limits", "Versions" and
//! "Security".
//!
//! # Shape
//!
//! [`Server`] and [`Client`] are state machines that never touch a socket or
//! a clock. The caller passes the time in (`now`, a [`Duration`] since any
//! origin it likes), feeds them received datagrams, calls `update` often,
//! sends what `poll_transmit` gives and handles `poll_event`. The helpers
//! `receive_from` and `transmit` do the socket part for anything that is
//! [`Datagrams`]: a non-blocking [`std::net::UdpSocket`] (see [`bind_udp`]
//! and [`RealClock`]) or a [`sim::SimSocket`] on the [`sim::SimNetwork`],
//! which runs on a virtual clock, deterministically for a seed.
//!
//! ```
//! use std::time::Duration;
//! use tore_net::sim::{LinkConfig, SimNetwork};
//! use tore_net::{
//!     AcceptInfo, Client, ClientConfig, ClientEvent, ConnectDetails, Decision, Entropy, Server,
//!     ServerConfig,
//! };
//!
//! let net = SimNetwork::new(1);
//! net.set_default_link(LinkConfig::one_way(Duration::from_millis(20)));
//! let host_addr = "10.0.0.1:26900".parse().unwrap();
//! let mut host_socket = net.bind(host_addr).unwrap();
//! let mut client_socket = net.bind("10.0.0.2:40000".parse().unwrap()).unwrap();
//!
//! let mut host = Server::new(ServerConfig { entropy: Entropy::Seeded(2), ..ServerConfig::new(1) });
//! let mut gate = |_: &ConnectDetails| {
//!     Decision::Accept(AcceptInfo { session_id: 1, ticks_per_second: 120, ticks_per_snapshot: 4, host_tick: 0 })
//! };
//! let config = ClientConfig { entropy: Entropy::Seeded(3), ..ClientConfig::new(1, "Viper") };
//! let mut client = Client::connect(config, host_addr, net.now()).unwrap();
//!
//! let mut joined = false;
//! for _ in 0..200 {
//!     net.advance(Duration::from_millis(1));
//!     let now = net.now();
//!     host.receive_from(&mut host_socket, now, &mut gate).unwrap();
//!     host.update(now);
//!     host.transmit(&mut host_socket).unwrap();
//!     client.receive_from(&mut client_socket, now).unwrap();
//!     client.update(now);
//!     client.transmit(&mut client_socket).unwrap();
//!     while let Some(event) = client.poll_event() {
//!         joined |= matches!(event, ClientEvent::Connected(_));
//!     }
//! }
//! assert!(joined);
//! ```

use std::time::Duration;

pub mod keepalive;
pub mod link;
pub mod master;
pub mod packet;
pub mod platform;
pub mod reach;
pub mod sim;

mod client;
mod connection;
mod datagram;
mod entropy;
mod reliable;
mod server;
mod socket;
mod track;
mod wait;

pub use client::{Client, ClientConfig, ClientEvent, ClientState, ConfigError, Welcome};
pub use connection::{
    CloseReason, ConnectionId, DisconnectReason, Event, RefuseReason, SendError, Stats,
};
pub use datagram::{Datagrams, RealClock, Transmit, bind_udp};
pub use entropy::{Entropy, SplitMix64};
pub use keepalive::{Keepalive, KeepaliveConfig};
pub use link::{LINK_ADDRESS, LinkEnd, Linked};
pub use packet::{MAX_DATAGRAM, Section};
pub use platform::Platform;
pub use reliable::{MAX_MESSAGE_BODY, MAX_MESSAGE_LEN, MAX_QUEUED_MESSAGES, MESSAGE_WINDOW};
pub use server::{AcceptInfo, ConnectDetails, Decision, Gate, Server, ServerConfig, ServerEvent};
pub use socket::{Listen, ServerSocket};
pub use wait::{MAX_NAP, SPIN_MARGIN, Sleep, wait_until};

/// The default server port (a setting).
pub const DEFAULT_PORT: u16 = 26900;
/// The highest section kind in protocol 1: Messages, Inputs, Snapshot,
/// Events, Own state.
pub const MAX_SECTION_KIND: u8 = 5;
/// A joining client repeats its current step this often.
pub const HANDSHAKE_RETRY: Duration = Duration::from_millis(250);
/// A joining client gives up after this long ("no answer from the server").
pub const HANDSHAKE_GIVE_UP: Duration = Duration::from_secs(10);
/// A connection ends after this long without a valid packet.
pub const TIMEOUT: Duration = Duration::from_secs(5);
/// Each side's transport sends a packet at least this often (10 a second)
/// while its caller drives it; a stalled game's [`Keepalive`] thread sends
/// one a second instead.
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_millis(100);
/// `update` sends a packet of due messages alone at most this often (120 a
/// second), so a burst of messages does not flood the link and a packet is
/// rarely overtaken by 32 newer ones (agent decision).
pub const MESSAGE_PACKET_INTERVAL: Duration = Duration::from_micros(8_333);
/// Bad packets within 5 seconds that end a connection.
pub const BAD_PACKET_LIMIT: usize = 50;
/// The cookie's time slot; the current and the previous slot are accepted.
pub const COOKIE_SLOT: Duration = Duration::from_secs(10);
/// Connect requests and answers answered per second from one IP address.
pub const RATE_LIMIT_PER_ADDRESS: u32 = 20;
/// Connect requests and answers answered per second in all.
pub const RATE_LIMIT_TOTAL: u32 = 200;
/// Datagrams one `receive_from` call takes at most, so a flood cannot hold
/// the caller's loop.
pub const MAX_RECEIVE_BATCH: usize = 1024;

/// Datagrams an endpoint dropped before they reached a connection, by cause.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counters {
    /// Too short, too long, an unknown kind or a failed checksum: from
    /// another program, another version or damaged. Dropped silently.
    pub invalid: u64,
    /// A good checksum but broken fields (outside any connection's count).
    pub malformed: u64,
    /// A kind this side never receives, or a handshake packet out of turn.
    pub unexpected: u64,
    /// From an address with no connection (or, on the client, not the host).
    pub unknown_address: u64,
    /// A connection id that is not the address's current one.
    pub stale: u64,
    /// Handshake packets over the rate limits.
    pub rate_limited: u64,
    /// Challenge answers whose cookie is wrong or expired.
    pub bad_cookie: u64,
}

/// True when sequence `a` is newer than `b`: `a - b` modulo 65,536 is 1 to
/// 32,767.
pub fn sequence_newer(a: u16, b: u16) -> bool {
    let d = a.wrapping_sub(b);
    d != 0 && d < 0x8000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequences_compare_the_wrapping_way() {
        assert!(sequence_newer(1, 0));
        assert!(sequence_newer(0, 65_535));
        assert!(sequence_newer(32_767, 0));
        assert!(!sequence_newer(32_768, 0));
        assert!(!sequence_newer(0, 0));
        assert!(!sequence_newer(0, 1));
    }
}
