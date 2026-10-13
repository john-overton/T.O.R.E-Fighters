//! A bot's keepalive thread (slice NET, 2026-10-12): the same thread the
//! game runs for a join over UDP (slice EF-K, `tore_net::Keepalive`), so a
//! bot whose loop is held up is not dropped by the host's 5-second silence
//! rule. The long hold-up is the mission build: the client builds the
//! mission's world inside `receive` when the host's Mission message
//! arrives, and on a loaded machine with a debug build that takes longer
//! than 5 seconds. Every bot of one process shares one loop, so one bot's
//! build holds up all of them; each has its own thread.
//!
//! The thread starts on the first turn after the host accepted the join,
//! before that turn's packets go out (so the host's answer, and the
//! mission with it, comes after the thread runs), learns of every turn, and
//! follows a rejoin or a race to a new host. A bot that hosts (its own
//! client over the in-process link) has none.

use std::net::SocketAddr;
use std::net::UdpSocket;
use tore_net::master::routed::is_relayed;

use tore_net::{Keepalive, KeepaliveConfig, ServerSocket};
use tore_session::{Client, ClientPhase};

use crate::hosting::Sock;
use tore_net::master::join::Joiner;

/// Where the bot's client sends from: what a keepalive thread needs a clone
/// of.
pub enum Source<'a> {
    /// A join by address (a standby's included): its UDP socket.
    Udp(&'a UdpSocket),
    /// A join through the Internet Lobby: its socket, and the joiner whose
    /// relay channel frames the keepalive for a relayed host.
    Lobby(&'a ServerSocket, &'a Joiner),
    /// Nothing to speak for: a bot that hosts, or no socket yet.
    None,
}

impl<'a> Source<'a> {
    /// The source for a bot with these parts.
    pub fn of(socket: Option<&'a Sock>, lobby: Option<(&'a ServerSocket, &'a Joiner)>) -> Self {
        match (lobby, socket) {
            (Some((socket, joiner)), _) => Self::Lobby(socket, joiner),
            (None, Some(Sock::Udp(socket))) => Self::Udp(socket),
            _ => Self::None,
        }
    }
}

/// The thread, when it runs, and what it speaks for.
#[derive(Default)]
pub struct Kept {
    thread: Option<Keepalive>,
    /// The host and the Keepalive packet the thread sends.
    speaks_for: Option<(SocketAddr, Vec<u8>)>,
    /// The thread could not be started; the bot carries on without it.
    failed: bool,
    /// Keepalives already reported.
    noted: u64,
}

impl Kept {
    /// After a turn of the bot's loop, before its packets go out. Returns
    /// the keepalives the thread sent since the last call that said so: the
    /// times the loop was held up for a second or more.
    pub fn turned(&mut self, client: &Client, from: Source<'_>) -> u64 {
        match client.phase() {
            ClientPhase::Connecting => return 0,
            ClientPhase::Closed => {
                self.thread = None;
                return 0;
            }
            _ => {}
        }
        let datagram = client.keepalive_datagram();
        let host = client.server();
        if matches!(from, Source::None) || datagram.is_none() {
            self.thread = None;
            self.speaks_for = None;
            return 0;
        }
        let moved = datagram.as_ref().is_some_and(|datagram| {
            self.speaks_for
                .as_ref()
                .is_none_or(|(to, sent)| *to != host || sent != datagram)
        });
        if moved {
            // A rejoin or a new host: the thread follows the connection.
            self.thread = None;
            self.speaks_for = None;
            self.failed = false;
        }
        if let Some(thread) = &self.thread {
            thread.turned();
            let sent = thread.sent();
            let new = sent.saturating_sub(self.noted);
            self.noted = sent;
            return new;
        }
        if self.failed {
            return 0;
        }
        let Some(datagram) = datagram else {
            return 0;
        };
        let config = KeepaliveConfig::default();
        let started = match from {
            Source::Udp(socket) => socket
                .try_clone()
                .and_then(|s| Keepalive::start(s, host, datagram.clone(), config)),
            Source::Lobby(socket, joiner) => socket.try_clone().and_then(|s| {
                if !is_relayed(host) {
                    return Keepalive::start(s, host, datagram.clone(), config);
                }
                match joiner.keepalive_socket(s) {
                    Some(framed) => Keepalive::start(framed, host, datagram.clone(), config),
                    None => Err(std::io::Error::other("the relay's channel is not open")),
                }
            }),
            Source::None => return 0,
        };
        match started {
            Ok(thread) => {
                self.thread = Some(thread);
                self.speaks_for = Some((host, datagram));
                self.noted = 0;
            }
            Err(error) => {
                self.failed = true;
                eprintln!("no keepalive thread ({error}); a held-up bot may be dropped");
            }
        }
        0
    }
}
