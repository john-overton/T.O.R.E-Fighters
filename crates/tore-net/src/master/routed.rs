//! The router in front of the transport (slice I3): one socket, two
//! protocols ("One socket, two protocols" in the architecture guide).
//!
//! A host's game port carries its players' traffic and the master's.
//! [`Routed`] is the socket as the transport sees it for one receive or
//! transmit: every datagram from one of the master's addresses (either of its
//! ports, either family) goes to the [`Rendezvous`] and never to the
//! transport; a datagram from a real socket that claims an address in the
//! relayed prefix `100::/64` is dropped and counted, as [`crate::Linked`]
//! drops a claim of [`LINK_ADDRESS`]; everything else passes unchanged.
//!
//! [`LINK_ADDRESS`] itself, `[100::]:0`, is the in-process link's peer and is
//! the one address in the prefix that passes: the hosting game reads its
//! `Linked` socket through this router, and `Linked` has already dropped any
//! datagram from the real socket that claims it. Relayed addresses
//! (`[100::1:HHHH:LLLL]:0`, the relay's channels) are stage J's slice J3;
//! until then a send to one is dropped and counted.

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use super::rendezvous::Rendezvous;
use crate::{Datagrams, LINK_ADDRESS, MAX_RECEIVE_BATCH};

/// True when `address` is in the relayed prefix `100::/64` and is not the
/// in-process link's own address: an address only the relay hands out, which
/// no real sender may claim.
pub fn is_relayed(address: SocketAddr) -> bool {
    match address.ip() {
        IpAddr::V6(ip) => {
            let segments = ip.segments();
            segments[..4] == [0x100, 0, 0, 0] && address != LINK_ADDRESS
        }
        IpAddr::V4(_) => false,
    }
}

/// A socket seen through a [`Rendezvous`]: see the module documentation.
/// Made with [`Rendezvous::over`] for one receive or one transmit.
pub struct Routed<'a, D: ?Sized> {
    pub(super) rendezvous: &'a mut Rendezvous,
    pub(super) socket: &'a mut D,
    pub(super) now: Duration,
}

impl<D: Datagrams + ?Sized> Datagrams for Routed<'_, D> {
    fn send_datagram(&mut self, to: SocketAddr, datagram: &[u8]) -> io::Result<()> {
        if is_relayed(to) {
            // Relay frames are slice J3's; no channel is open before it.
            self.rendezvous.counters.relay_sends_dropped += 1;
            return Ok(());
        }
        self.socket.send_datagram(to, datagram)
    }

    fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        // A flood from the master's addresses cannot hold the loop: after a
        // batch of them the transport is told nothing is waiting this turn.
        for _ in 0..MAX_RECEIVE_BATCH {
            let Some((length, from)) = self.socket.recv_datagram(buf)? else {
                return Ok(None);
            };
            if self.rendezvous.is_master(from) {
                self.rendezvous.receive(self.now, from, &buf[..length]);
            } else if is_relayed(from) {
                self.rendezvous.counters.relayed_claims += 1;
            } else {
                return Ok(Some((length, from)));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_relayed_prefix_is_100_slash_64_less_the_link() {
        let address = |text: &str| text.parse::<SocketAddr>().unwrap();
        assert!(is_relayed(address("[100::1:0:7]:0")));
        assert!(is_relayed(address("[100::ffff:ffff:ffff:ffff]:5")));
        assert!(is_relayed(address("[100::]:1")));
        assert!(!is_relayed(LINK_ADDRESS));
        assert!(!is_relayed(address("[100:0:0:1::]:0")));
        assert!(!is_relayed(address("[2001:db8::1]:26900")));
        assert!(!is_relayed(address("10.0.0.1:0")));
    }
}
