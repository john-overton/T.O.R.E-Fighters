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
//! datagram from the real socket that claims it.
//!
//! **The relay (slice J3).** A channel's peer is a relayed address,
//! [`relayed_address`], `[100::1:HHHH:LLLL]:0` for channel `HHHHLLLL`. A
//! Relay frame of an open channel from the master reaches the transport as a
//! datagram from that address, and a datagram the transport sends there
//! leaves as a Relay frame to the master ([`super::relay`]). A frame of no
//! channel, or with another key, is dropped and counted, and so is a send to
//! a relayed address with no channel.
//!
//! The router reads into a buffer of its own, as long as the longest master
//! datagram, so a Relay frame (up to 1,215 bytes) is never cut by the
//! transport's buffer (1,201 bytes); what the transport gets is copied to
//! its buffer.

use std::io;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use super::packet::{MAX_MASTER_DATAGRAM, MasterKind, peek_kind};
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

/// The address that stands for relay channel `channel` on both ends:
/// `[100::1:HHHH:LLLL]:0` ("Relayed addresses" in the net protocol).
pub fn relayed_address(channel: u32) -> SocketAddr {
    let ip = Ipv6Addr::new(0x100, 0, 0, 0, 0, 1, (channel >> 16) as u16, channel as u16);
    SocketAddr::new(IpAddr::V6(ip), 0)
}

/// The channel a relayed address stands for, when it is one
/// ([`relayed_address`]'s inverse).
pub fn channel_of(address: SocketAddr) -> Option<u32> {
    let IpAddr::V6(ip) = address.ip() else {
        return None;
    };
    let s = ip.segments();
    (address.port() == 0 && s[..6] == [0x100, 0, 0, 0, 0, 1])
        .then(|| u32::from(s[6]) << 16 | u32::from(s[7]))
}

/// One side of a master's router: the host's [`Rendezvous`] or a joining
/// player's [`super::join::Joiner`]. Both routers ([`Routed`] and
/// [`super::join::JoinRouted`]) read and send through [`route_receive`] and
/// [`route_send`], so the master's packets, the relay's frames and the
/// relayed prefix are handled the same way on both (agent decision, J3: the
/// two routers J2 left side by side share their code here).
pub(super) trait MasterSide {
    /// True when `from` is one of the master's addresses, either port.
    fn is_master(&self, from: SocketAddr) -> bool;
    /// A master packet other than a Relay frame.
    fn take_master(&mut self, now: Duration, from: SocketAddr, datagram: &[u8]);
    /// The relay's channels.
    fn channels(&mut self) -> &mut super::relay::Channels;
    /// A real sender claimed an address in the relayed prefix.
    fn claimed(&mut self);
    /// The transport sent to a relayed address with no open channel.
    fn send_dropped(&mut self);
}

/// Reads one datagram for the transport from `socket` into `buf`: see the
/// module documentation. At most [`MAX_RECEIVE_BATCH`] datagrams are read in
/// one call, so a flood from the master's addresses cannot hold the loop.
pub(super) fn route_receive<D: Datagrams + ?Sized, S: MasterSide + ?Sized>(
    side: &mut S,
    socket: &mut D,
    buf: &mut [u8],
    now: Duration,
) -> io::Result<Option<(usize, SocketAddr)>> {
    let mut own = [0u8; MAX_MASTER_DATAGRAM + 1];
    for _ in 0..MAX_RECEIVE_BATCH {
        let Some((length, from)) = socket.recv_datagram(&mut own)? else {
            return Ok(None);
        };
        let datagram = &own[..length];
        if side.is_master(from) {
            if peek_kind(datagram) == Some(MasterKind::Relay) {
                if let Some((start, relayed)) = side.channels().unwrap_frame(now, from, datagram) {
                    // The game's datagram is the frame's tail.
                    let game = &datagram[start..];
                    let n = game.len().min(buf.len());
                    buf[..n].copy_from_slice(&game[..n]);
                    return Ok(Some((n, relayed)));
                }
            } else {
                side.take_master(now, from, datagram);
            }
        } else if is_relayed(from) {
            side.claimed();
        } else {
            let n = length.min(buf.len());
            buf[..n].copy_from_slice(&datagram[..n]);
            return Ok(Some((n, from)));
        }
    }
    Ok(None)
}

/// Sends one of the transport's datagrams: to a relayed address as a Relay
/// frame to the master, else unchanged.
pub(super) fn route_send<D: Datagrams + ?Sized, S: MasterSide + ?Sized>(
    side: &mut S,
    socket: &mut D,
    to: SocketAddr,
    datagram: &[u8],
    now: Duration,
) -> io::Result<()> {
    if !is_relayed(to) {
        return socket.send_datagram(to, datagram);
    }
    match side.channels().wrap(now, to, datagram) {
        Some((master, frame)) => socket.send_datagram(master, &frame),
        None => {
            side.send_dropped();
            Ok(())
        }
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
        route_send(self.rendezvous, self.socket, to, datagram, self.now)
    }

    fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        route_receive(self.rendezvous, self.socket, buf, self.now)
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

    #[test]
    fn a_channel_is_its_relayed_address_and_back() {
        let address = |text: &str| text.parse::<SocketAddr>().unwrap();
        assert_eq!(relayed_address(7), address("[100::1:0:7]:0"));
        assert_eq!(
            relayed_address(0xdead_beef),
            address("[100::1:dead:beef]:0")
        );
        for channel in [0, 1, 0xffff, 0x1_0000, u32::MAX] {
            let relayed = relayed_address(channel);
            assert!(is_relayed(relayed));
            assert_eq!(channel_of(relayed), Some(channel));
        }
        // Only the channels' own corner of the prefix, on port 0.
        assert_eq!(channel_of(address("[100::1:0:7]:1")), None);
        assert_eq!(channel_of(address("[100::2:0:7]:0")), None);
        assert_eq!(channel_of(LINK_ADDRESS), None);
        assert_eq!(channel_of(address("10.0.0.1:0")), None);
    }
}
