//! Both ends' side of the relay (slice J3, "The relay" in the architecture
//! guide and "Relay" in the master protocol): the channels an end has open,
//! the Relay frames that carry the game's datagrams through the master, the
//! host's answer to a Relay open, and the socket wrapper a relayed game's
//! keepalive thread sends through.
//!
//! When a joining player's race finds no path (or the master's hint says so
//! at once), the player sends a Relay request; the master opens a channel,
//! sends the host a Relay open, and once the host acknowledges it, offers the
//! player the channel and its key. From then on each end's transport sees
//! the other at the channel's relayed address
//! ([`relayed_address`](super::routed::relayed_address)): a datagram it
//! sends there leaves as a Relay frame to the master, and a frame of the
//! channel from the master reaches it as a datagram from that address
//! ([`super::routed`]). The game's datagram inside a frame is unchanged.
//!
//! *Agent decisions:*
//!
//! - A frame counts only from the master's address the channel was opened
//!   by, with the channel's key; anything else is dropped and counted.
//! - An end forgets a channel it has neither sent nor received a frame on for
//!   [`FORGET_AFTER`] (twice the master's idle time, since the master's
//!   Relay close may be lost), and a host keeps at most [`MAX_CHANNELS`].
//! - A host acknowledges a Relay open only while it is listed (the ack
//!   carries its listing token), and acknowledges one repeated (its ack was
//!   lost) again, keeping the channel it has.
//! - The keepalive thread's wrapper ([`RelayFraming`]) never reads: the
//!   game's loop reads the socket.

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use super::candidate::canonical;
use super::packet::{
    CloseReason, MasterPacket, RELAY_HEADER_LEN, RelayClose, RelayFrame, RelayOpen, RelayOpenAck,
    RelayResult,
};
use super::routed::{channel_of, is_relayed, relayed_address};
use super::{GOODBYE_COPIES, RELAY_IDLE};
use crate::Datagrams;

/// The most channels a host keeps open (the master allows 30 per listing).
pub const MAX_CHANNELS: usize = 64;

/// An end forgets a channel with no frame either way for this long.
pub const FORGET_AFTER: Duration = Duration::from_secs(RELAY_IDLE.as_secs() * 2);

/// What a player reads when the master will not relay, unless the master
/// sent a text of its own.
pub fn refusal_text(result: RelayResult) -> &'static str {
    match result {
        RelayResult::Open => "Connected through the relay.",
        RelayResult::HostSilent => "The game's host did not answer the relay.",
        RelayResult::Full => "The Internet Lobby's relay is busy. Try again in a few minutes.",
        RelayResult::AllowanceSpent => "The Internet Lobby's relay is full for this month.",
        RelayResult::Off => "The Internet Lobby's relay is switched off.",
        RelayResult::TooMany => "Too many relayed games from this address at once.",
    }
}

/// Why a channel closed, as a player reads it.
pub fn close_text(reason: CloseReason) -> &'static str {
    match reason {
        CloseReason::Closed => "The relay was closed by the other end.",
        CloseReason::Idle => "The relay closed after 30 seconds with no traffic.",
        CloseReason::OverRate => "The relay closed: the game sent more than it allows.",
        CloseReason::AllowanceSpent => "The Internet Lobby's relay is full for this month.",
        CloseReason::Stopping => "The Internet Lobby is restarting; the relay closed.",
    }
}

/// What an end's relay counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RelayCounters {
    /// Channels opened.
    pub opened: u64,
    /// Channels closed: by the master, by this end, or forgotten when idle.
    pub closed: u64,
    /// Frames received from the master and handed to the transport.
    pub frames_in: u64,
    /// The game's bytes in those frames.
    pub bytes_in: u64,
    /// Frames sent to the master.
    pub frames_out: u64,
    /// The game's bytes in those frames.
    pub bytes_out: u64,
    /// Frames from the master of no channel open here, with another key, or
    /// from another of its addresses: dropped.
    pub frames_dropped: u64,
}

#[derive(Debug, Clone, Copy)]
struct Channel {
    key: u32,
    master: SocketAddr,
    /// The last frame either way.
    heard: Duration,
}

/// The channels an end has open, by channel number.
#[derive(Debug, Clone, Default)]
pub struct Channels {
    open: HashMap<u32, Channel>,
    /// What the channels counted.
    pub counters: RelayCounters,
}

impl Channels {
    /// Opens `channel` with `key` from the master at `master`; an open one is
    /// kept as it is. False when [`MAX_CHANNELS`] are open already.
    pub fn open(&mut self, channel: u32, key: u32, master: SocketAddr, now: Duration) -> bool {
        if let Some(open) = self.open.get(&channel) {
            return open.key == key;
        }
        if self.open.len() >= MAX_CHANNELS {
            return false;
        }
        self.open.insert(
            channel,
            Channel {
                key,
                master: canonical(master),
                heard: now,
            },
        );
        self.counters.opened += 1;
        true
    }

    /// Closes `channel` when `key` is its key; its master's address.
    pub fn close(&mut self, channel: u32, key: u32) -> Option<SocketAddr> {
        if self.open.get(&channel)?.key != key {
            return None;
        }
        self.counters.closed += 1;
        self.open.remove(&channel).map(|c| c.master)
    }

    /// The key and the master's address of an open channel.
    pub fn get(&self, channel: u32) -> Option<(u32, SocketAddr)> {
        self.open.get(&channel).map(|c| (c.key, c.master))
    }

    /// The relayed addresses of the channels open.
    pub fn addresses(&self) -> Vec<SocketAddr> {
        let mut channels: Vec<u32> = self.open.keys().copied().collect();
        channels.sort_unstable();
        channels.into_iter().map(relayed_address).collect()
    }

    /// How many channels are open.
    pub fn len(&self) -> usize {
        self.open.len()
    }

    /// True when none is.
    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// A Relay frame from `from`, one of the master's addresses: the start
    /// of the game's datagram in it and the channel's relayed address, or
    /// `None` when it is dropped (counted).
    pub fn unwrap_frame(
        &mut self,
        now: Duration,
        from: SocketAddr,
        frame: &[u8],
    ) -> Option<(usize, SocketAddr)> {
        let opened = RelayFrame::open(frame).ok().and_then(|f| {
            let channel = self.open.get_mut(&f.channel)?;
            (channel.key == f.key && channel.master == canonical(from)).then(|| {
                channel.heard = now;
                (f.channel, f.datagram.len())
            })
        });
        let Some((channel, length)) = opened else {
            self.counters.frames_dropped += 1;
            return None;
        };
        self.counters.frames_in += 1;
        self.counters.bytes_in += length as u64;
        Some((RELAY_HEADER_LEN, relayed_address(channel)))
    }

    /// A datagram the transport sends to `to`: the master's address and the
    /// Relay frame to send there, or `None` when `to` is no open channel's.
    pub fn wrap(
        &mut self,
        now: Duration,
        to: SocketAddr,
        datagram: &[u8],
    ) -> Option<(SocketAddr, Vec<u8>)> {
        let number = channel_of(to)?;
        let channel = self.open.get_mut(&number)?;
        let frame = RelayFrame {
            channel: number,
            key: channel.key,
            datagram,
        }
        .encode()
        .ok()?;
        channel.heard = now;
        self.counters.frames_out += 1;
        self.counters.bytes_out += datagram.len() as u64;
        Some((channel.master, frame))
    }

    /// Forgets the channels with no frame either way for [`FORGET_AFTER`];
    /// their numbers.
    pub fn forget_idle(&mut self, now: Duration) -> Vec<u32> {
        let mut idle: Vec<u32> = self
            .open
            .iter()
            .filter(|(_, c)| now.saturating_sub(c.heard) >= FORGET_AFTER)
            .map(|(n, _)| *n)
            .collect();
        idle.sort_unstable();
        for n in &idle {
            self.open.remove(n);
            self.counters.closed += 1;
        }
        idle
    }

    /// Closes every channel: each one's Relay close, [`GOODBYE_COPIES`]
    /// times, to its master, as `(to, packet)`.
    pub fn close_all(&mut self) -> Vec<(SocketAddr, MasterPacket)> {
        let mut channels: Vec<(u32, Channel)> = self.open.drain().collect();
        channels.sort_unstable_by_key(|(n, _)| *n);
        self.counters.closed += channels.len() as u64;
        channels
            .into_iter()
            .flat_map(|(n, c)| goodbye(n, c.key).into_iter().map(move |p| (c.master, p)))
            .collect()
    }
}

/// An end's Relay close for `channel`, [`GOODBYE_COPIES`] copies.
pub fn goodbye(channel: u32, key: u32) -> Vec<MasterPacket> {
    (0..GOODBYE_COPIES)
        .map(|_| {
            MasterPacket::RelayClose(RelayClose {
                channel,
                key,
                reason: CloseReason::Closed,
            })
        })
        .collect()
}

/// A host's side of the relay: its channels, and its answers to the
/// master's Relay open and Relay close.
#[derive(Debug, Clone, Default)]
pub struct HostRelays {
    /// The channels open.
    pub channels: Channels,
    /// Relay opens refused: not listed, or [`MAX_CHANNELS`] open.
    pub refused: u64,
}

impl HostRelays {
    /// A Relay open from the master at `from`: the Relay open ack to send
    /// back, or `None` when the host is not listed (`token` is `None`) or
    /// has no room.
    pub fn open(
        &mut self,
        now: Duration,
        from: SocketAddr,
        open: &RelayOpen,
        token: Option<u64>,
    ) -> Option<MasterPacket> {
        let Some(token) = token else {
            self.refused += 1;
            return None;
        };
        if !self.channels.open(open.channel, open.key, from, now) {
            self.refused += 1;
            return None;
        }
        Some(MasterPacket::RelayOpenAck(RelayOpenAck {
            token,
            channel: open.channel,
        }))
    }

    /// A Relay close from the master: the channel goes, when its key is
    /// right. True when it went.
    pub fn close(&mut self, close: &RelayClose) -> bool {
        self.channels.close(close.channel, close.key).is_some()
    }

    /// Asks the master to close the channel of `relayed` (the host's
    /// connection there ended): the Relay closes to send, as `(to, packet)`.
    pub fn close_address(&mut self, relayed: SocketAddr) -> Vec<(SocketAddr, MasterPacket)> {
        let Some(channel) = channel_of(relayed) else {
            return Vec::new();
        };
        let Some((key, _)) = self.channels.get(channel) else {
            return Vec::new();
        };
        let Some(master) = self.channels.close(channel, key) else {
            return Vec::new();
        };
        goodbye(channel, key)
            .into_iter()
            .map(|packet| (master, packet))
            .collect()
    }
}

/// A socket that sends one channel's datagrams as Relay frames to the
/// master: what a relayed game hands its keepalive thread
/// ([`crate::Keepalive::start`]), wrapped around a clone of its socket
/// ([`crate::ServerSocket::try_clone`]), so a stalled game's keepalives
/// reach the host through the relay as its own packets do.
///
/// A send to the channel's relayed address goes to the master as a frame;
/// a send to another relayed address is dropped; any other send goes out
/// unchanged. It never reads: the game's loop reads the socket.
#[derive(Debug)]
pub struct RelayFraming<S> {
    socket: S,
    master: SocketAddr,
    channel: u32,
    key: u32,
}

impl<S> RelayFraming<S> {
    /// Frames for `channel` with `key`, sent to the master at `master` on
    /// `socket`.
    pub fn new(socket: S, master: SocketAddr, channel: u32, key: u32) -> Self {
        Self {
            socket,
            master,
            channel,
            key,
        }
    }

    /// The channel's relayed address: where the transport's packets for it
    /// go.
    pub fn address(&self) -> SocketAddr {
        relayed_address(self.channel)
    }
}

impl<S: Datagrams> Datagrams for RelayFraming<S> {
    fn send_datagram(&mut self, to: SocketAddr, datagram: &[u8]) -> io::Result<()> {
        if channel_of(to) == Some(self.channel) {
            let frame = RelayFrame {
                channel: self.channel,
                key: self.key,
                datagram,
            }
            .encode();
            return match frame {
                Ok(frame) => self.socket.send_datagram(self.master, &frame),
                // Only an empty or oversize datagram, which no transport
                // sends.
                Err(_) => Ok(()),
            };
        }
        if is_relayed(to) {
            return Ok(());
        }
        self.socket.send_datagram(to, datagram)
    }

    fn recv_datagram(&mut self, _buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master::packet::peek_kind;
    use crate::master::{MasterKind, Relay};

    fn a(text: &str) -> SocketAddr {
        text.parse().unwrap()
    }

    const MASTER: &str = "198.51.100.1:26901";
    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn a_frame_of_an_open_channel_unwraps_and_wraps() {
        let mut channels = Channels::default();
        assert!(channels.open(7, 99, a(MASTER), Duration::ZERO));
        let frame = RelayFrame {
            channel: 7,
            key: 99,
            datagram: &[1, 2, 3],
        }
        .encode()
        .unwrap();
        let (start, from) = channels.unwrap_frame(MS, a(MASTER), &frame).unwrap();
        assert_eq!(
            (&frame[start..], from),
            (&[1u8, 2, 3][..], relayed_address(7))
        );
        // The way back.
        let (to, back) = channels.wrap(MS, relayed_address(7), &[4, 5]).unwrap();
        assert_eq!(to, a(MASTER));
        assert_eq!(
            MasterPacket::decode(&back).unwrap(),
            MasterPacket::Relay(Relay {
                channel: 7,
                key: 99,
                datagram: vec![4, 5]
            })
        );
        assert_eq!(channels.counters.frames_in, 1);
        assert_eq!(channels.counters.frames_out, 1);
        assert_eq!(
            (channels.counters.bytes_in, channels.counters.bytes_out),
            (3, 2)
        );
    }

    #[test]
    fn a_wrong_key_another_channel_or_another_master_address_is_dropped() {
        let mut channels = Channels::default();
        channels.open(7, 99, a(MASTER), Duration::ZERO);
        let frame = |channel, key| {
            RelayFrame {
                channel,
                key,
                datagram: &[1],
            }
            .encode()
            .unwrap()
        };
        assert!(
            channels
                .unwrap_frame(MS, a(MASTER), &frame(7, 98))
                .is_none()
        );
        assert!(
            channels
                .unwrap_frame(MS, a(MASTER), &frame(8, 99))
                .is_none()
        );
        assert!(
            channels
                .unwrap_frame(MS, a("198.51.100.1:26902"), &frame(7, 99))
                .is_none()
        );
        assert!(channels.unwrap_frame(MS, a(MASTER), &[0; 20]).is_none());
        assert_eq!(channels.counters.frames_dropped, 4);
        assert!(channels.wrap(MS, relayed_address(8), &[1]).is_none());
        // An IPv4 master seen through a dual-stack socket is the same.
        assert!(
            channels
                .unwrap_frame(MS, a("[::ffff:198.51.100.1]:26901"), &frame(7, 99))
                .is_some()
        );
    }

    #[test]
    fn a_host_acknowledges_an_open_only_while_listed_and_closes_by_key() {
        let mut host = HostRelays::default();
        let open = RelayOpen {
            introduction_id: 5,
            channel: 7,
            key: 99,
            player: a("203.0.113.9:40000"),
        };
        assert_eq!(host.open(MS, a(MASTER), &open, None), None);
        assert_eq!(host.refused, 1);
        let ack = host.open(MS, a(MASTER), &open, Some(42)).unwrap();
        assert_eq!(
            ack,
            MasterPacket::RelayOpenAck(RelayOpenAck {
                token: 42,
                channel: 7
            })
        );
        // Again (the ack was lost): acknowledged again, one channel.
        assert_eq!(host.open(MS, a(MASTER), &open, Some(42)), Some(ack));
        assert_eq!(host.channels.len(), 1);
        let wrong = RelayClose {
            channel: 7,
            key: 1,
            reason: CloseReason::Idle,
        };
        assert!(!host.close(&wrong));
        assert!(host.close(&RelayClose { key: 99, ..wrong }));
        assert!(host.channels.is_empty());
    }

    #[test]
    fn a_host_closes_a_channel_by_its_address_and_forgets_idle_ones() {
        let mut host = HostRelays::default();
        let open = |channel| RelayOpen {
            introduction_id: 5,
            channel,
            key: 99,
            player: a("203.0.113.9:40000"),
        };
        host.open(Duration::ZERO, a(MASTER), &open(1), Some(42));
        host.open(Duration::ZERO, a(MASTER), &open(2), Some(42));
        let closes = host.close_address(relayed_address(1));
        assert_eq!(closes.len(), GOODBYE_COPIES as usize);
        assert!(closes.iter().all(|(to, p)| *to == a(MASTER)
            && *p
                == MasterPacket::RelayClose(RelayClose {
                    channel: 1,
                    key: 99,
                    reason: CloseReason::Closed
                })));
        assert!(host.close_address(relayed_address(1)).is_empty());
        // Channel 2 hears a frame at 10 s, so it is forgotten 60 s later.
        let frame = RelayFrame {
            channel: 2,
            key: 99,
            datagram: &[1],
        }
        .encode()
        .unwrap();
        let ten = Duration::from_secs(10);
        host.channels.unwrap_frame(ten, a(MASTER), &frame);
        assert!(
            host.channels
                .forget_idle(ten + FORGET_AFTER - MS)
                .is_empty()
        );
        assert_eq!(host.channels.forget_idle(ten + FORGET_AFTER), [2]);
    }

    #[test]
    fn a_host_keeps_at_most_64_channels() {
        let mut host = HostRelays::default();
        for channel in 0..MAX_CHANNELS as u32 + 3 {
            let open = RelayOpen {
                introduction_id: 5,
                channel,
                key: 1,
                player: a("203.0.113.9:40000"),
            };
            host.open(Duration::ZERO, a(MASTER), &open, Some(42));
        }
        assert_eq!(host.channels.len(), MAX_CHANNELS);
        assert_eq!(host.refused, 3);
    }

    /// A socket that keeps what is sent.
    #[derive(Default)]
    struct Sent(Vec<(SocketAddr, Vec<u8>)>);

    impl Datagrams for Sent {
        fn send_datagram(&mut self, to: SocketAddr, datagram: &[u8]) -> io::Result<()> {
            self.0.push((to, datagram.to_vec()));
            Ok(())
        }

        fn recv_datagram(&mut self, _: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
            Ok(None)
        }
    }

    #[test]
    fn the_keepalive_wrapper_frames_for_its_channel_only() {
        let mut framing = RelayFraming::new(Sent::default(), a(MASTER), 7, 99);
        assert_eq!(framing.address(), relayed_address(7));
        framing.send_datagram(relayed_address(7), &[9; 9]).unwrap();
        framing.send_datagram(relayed_address(8), &[9; 9]).unwrap();
        framing.send_datagram(a("203.0.113.5:26900"), &[1]).unwrap();
        let mut buf = [0u8; 16];
        assert!(framing.recv_datagram(&mut buf).unwrap().is_none());
        let sent = framing.socket.0;
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].0, a(MASTER));
        assert_eq!(peek_kind(&sent[0].1), Some(MasterKind::Relay));
        let frame = RelayFrame::open(&sent[0].1).unwrap();
        assert_eq!(
            (frame.channel, frame.key, frame.datagram),
            (7, 99, &[9u8; 9][..])
        );
        assert_eq!(sent[1], (a("203.0.113.5:26900"), vec![1]));
    }
}
