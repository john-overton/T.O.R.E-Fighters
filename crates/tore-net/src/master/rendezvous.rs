//! A host's conversation with the master (slice I3): listing a game from its
//! game port ("Listing a game" in the architecture guide and the master
//! protocol).
//!
//! [`Rendezvous`] is a state machine like the transport's: it never reads a
//! clock or touches a socket. The host's loop reads its socket through
//! [`Rendezvous::over`], which takes the master's datagrams out of the game
//! port's stream ([`Routed`]), calls [`Rendezvous::update`] for its timers
//! and sends what [`Rendezvous::transmit`] gives on the same socket, so the
//! master sees the outside address of the very socket players join.
//! [`HostListing`] adds what needs the real world: the lookup of the
//! master's name on a thread, and the host's own addresses.
//!
//! What it does, with the master protocol's numbers:
//!
//! 1. With the master's address known it sends the mapping test's two
//!    Probes and a Register, at the same moment (*agent decision:* the
//!    listing does not wait for the test, whose result only the Report uses
//!    in stage I). A Challenge is answered with the Register again, carrying
//!    the cookie; Listed makes the game listed.
//! 2. A Heartbeat with the summary every heartbeat interval (30 seconds, or
//!    what Listed says), and one more when the summary changes, at most one
//!    every 5 seconds; a Keep when nothing else has gone out for the keep
//!    interval (15 seconds). No two of them closer than 2 seconds, the
//!    master's limit per listing (agent decision).
//! 3. An Unknown listing means register again at once: a restarted master
//!    lists the game again within one exchange.
//! 4. A request (Register or Heartbeat) unanswered is sent again every 3
//!    seconds (agent decision); no answer at all for 10 seconds means the
//!    master is silent, and the request is then repeated after 2, 4, 8 and
//!    up to 60 seconds, the next of the master's addresses each time.
//! 5. Unlisting, or stopping, sends Unregister three times at once (*agent
//!    decision:* at once, since the socket may close right after).
//!
//! 6. A Meet (slice J2) is answered with punches to the player's addresses
//!    from the game port and a Meet ack ([`meet`](super::meet)).
//! 7. A Relay open (slice J3) is acknowledged with the listing's token, and
//!    the channel's frames reach the transport as datagrams from its relayed
//!    address ([`relay`](super::relay)); the master's Relay close, or no
//!    frame for a minute, closes it here.
//!
//! The summary is the host's discovery answer without its nonce. The host's
//! loop offers it through [`Rendezvous::wants_summary`] and
//! [`Rendezvous::set_summary`], which the rendezvous asks for at most once a
//! second (agent decision), so building it costs nothing on most turns.

use std::collections::{BTreeSet, VecDeque};
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use super::candidate::{Candidate, MappingType, canonical};
use super::local::{
    MasterLookup, host_candidates, own_address_toward, parse_master, probe_address,
};
use super::meet::{MeetOutcome, Meets};
use super::packet::{
    Build, Heartbeat, Keep, ListingSummary, MasterPacket, MeetAck, Path, PortMapping, Probe,
    ProbePort, Register, Report, Role, Unregister,
};
use super::relay::{Channels, HostRelays, RelayCounters};
use super::routed::{MasterSide, Routed, is_relayed};
use super::{
    CHANGE_HEARTBEAT_DELAY, GOODBYE_COPIES, HEARTBEAT_INTERVAL, KEEP_INTERVAL,
    MAPPING_TEST_INTERVAL, MASTER_SILENT,
};
use crate::entropy::{Entropy, Rng};
use crate::packet::{Packet, Punch};
use crate::{Datagrams, LINK_ADDRESS, Transmit};

/// An unanswered Register or Heartbeat is sent again this often while the
/// master is not yet silent (agent decision).
pub const REQUEST_RETRY: Duration = Duration::from_secs(3);
/// The least time between two of a listing's Heartbeats and Keeps: the
/// master drops the second of two closer than this (agent decision, the
/// master protocol's limit).
pub const LISTING_GAP: Duration = Duration::from_secs(2);
/// The rendezvous asks the host's loop for the summary at most this often
/// (agent decision).
pub const SUMMARY_LOOK: Duration = Duration::from_secs(1);
/// The first wait after the master falls silent; each next one doubles.
pub const FIRST_BACKOFF: Duration = Duration::from_secs(2);
/// The longest wait between requests to a silent master.
pub const MAX_BACKOFF: Duration = Duration::from_secs(60);
/// The master's name is looked up again this often while listing.
pub const LOOKUP_INTERVAL: Duration = Duration::from_secs(600);
/// Probes of one mapping test are sent this many times at most while no
/// answer comes, [`REQUEST_RETRY`] apart (agent decision).
const PROBE_TRIES: u32 = 3;

/// What a host's rendezvous is set up with.
#[derive(Debug, Clone)]
pub struct HostRendezvous {
    /// The host's build, which the master filters by.
    pub build: Build,
    /// A dedicated server (`tore-server`), not a game that hosts.
    pub dedicated: bool,
    /// The anonymous install id when telemetry is on, else `None`: then the
    /// Register carries none and no Report is sent.
    pub install_id: Option<u64>,
    /// The host's platform code.
    pub platform: u8,
    /// The nonces' source: [`Entropy::System`] on a real network.
    pub entropy: Entropy,
}

/// Where the listing stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListingState {
    /// Not asked to list.
    Off,
    /// Asked to list; the master's address is not known yet.
    FindingMaster,
    /// Registering: no Listed yet.
    Registering,
    /// On the master's list.
    Listed {
        /// The public id browsers name the listing by.
        listing_id: u64,
        /// The host's address as the master saw it.
        seen: SocketAddr,
    },
    /// No answer from the master for 10 seconds; asking again with back-off.
    Silent,
    /// The master does not speak this build's master protocol: its text.
    Refused(String),
}

/// What the rendezvous tells the host's loop, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendezvousEvent {
    /// The game is listed, first or again (after a master restart or a
    /// silence).
    Listed {
        /// The public id.
        listing_id: u64,
        /// The host's address as the master saw it.
        seen: SocketAddr,
    },
    /// The master saw the host at another address (the router gave the port
    /// a new outside address).
    SeenChanged(SocketAddr),
    /// The game is no longer listed, as asked.
    Unlisted,
    /// The master has not answered for 10 seconds.
    MasterSilent,
    /// The master's name could not be looked up, with why.
    LookupFailed(String),
    /// The master does not speak this build's master protocol: its text.
    Refused(String),
    /// The mapping test's result: how the host's router maps the game port.
    MappingTested(MappingType),
}

/// What the rendezvous counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RendezvousCounters {
    /// Master packets sent.
    pub sent: u64,
    /// Datagrams from the master's addresses taken out of the stream.
    pub received: u64,
    /// Of those, ones that did not decode.
    pub malformed: u64,
    /// Good packets this side does not take, or not now (an old nonce, a
    /// token not ours).
    pub unexpected: u64,
    /// Relay opens acknowledged (slice J3), repeats included.
    pub relay_opens: u64,
    /// Relay closes from the master that closed a channel.
    pub relay_closes: u64,
    /// Meets acted on: punched and acknowledged (slice J2).
    pub meets: u64,
    /// Meets repeated for an introduction already met: acknowledged again.
    pub meets_again: u64,
    /// Meets dropped: over 10 a second, or while the game is not listed.
    pub meets_dropped: u64,
    /// Punches sent.
    pub punches: u64,
    /// Datagrams from a real socket claiming a relayed address, dropped.
    pub relayed_claims: u64,
    /// Sends by the transport to a relayed address with no open channel,
    /// dropped.
    pub relay_sends_dropped: u64,
}

/// The listing the master gave.
#[derive(Debug, Clone, Copy)]
struct Listing {
    listing_id: u64,
    token: u64,
    seen: SocketAddr,
}

/// The mapping test under way or done.
#[derive(Debug, Clone)]
struct Mapping {
    nonce: u64,
    main: Option<SocketAddr>,
    second: Option<SocketAddr>,
    /// The last complete test's verdict.
    result: MappingType,
    /// When this test's first Probes went out.
    started: Duration,
    /// When Probes go out next.
    next: Duration,
    tries: u32,
    /// This test's verdict is in.
    done: bool,
}

impl Mapping {
    fn new(result: MappingType, next: Duration) -> Self {
        Self {
            nonce: 0,
            main: None,
            second: None,
            result,
            started: next,
            next,
            tries: 0,
            done: false,
        }
    }
}

/// A host's side of the master: see the module documentation.
#[derive(Debug)]
pub struct Rendezvous {
    config: HostRendezvous,
    rng: Rng,
    /// The master's main addresses, canonical, IPv4 first.
    masters: Vec<SocketAddr>,
    /// The one listed with now.
    current: usize,
    candidates: Vec<Candidate>,
    wanted: bool,
    nonce: u64,
    listing: Option<Listing>,
    heartbeat_interval: Duration,
    keep_interval: Duration,
    summary: Option<ListingSummary>,
    /// Raised when the summary changes.
    change: u16,
    /// The summary changed since the last Register or Heartbeat.
    changed: bool,
    looked: Option<Duration>,
    last_heartbeat: Duration,
    last_listing_send: Option<Duration>,
    /// When the oldest unanswered request went out.
    asked_since: Option<Duration>,
    /// When the request goes out (again).
    next_ask: Duration,
    silent: bool,
    backoff: Duration,
    refused: Option<String>,
    mapping: Mapping,
    meets: Meets,
    /// The relay's channels to this host (slice J3).
    pub(super) relays: HostRelays,
    out: VecDeque<Transmit>,
    events: VecDeque<RendezvousEvent>,
    /// What the rendezvous counted.
    pub counters: RendezvousCounters,
}

impl Rendezvous {
    /// A host's rendezvous, not listing until [`Rendezvous::set_listed`].
    pub fn host(mut config: HostRendezvous, now: Duration) -> Self {
        // An install id of 0 means none.
        config.install_id = config.install_id.filter(|id| *id != 0);
        let mut rng = Rng::new(config.entropy);
        let nonce = rng.next_u64();
        Self {
            config,
            rng,
            masters: Vec::new(),
            current: 0,
            candidates: Vec::new(),
            wanted: false,
            nonce,
            listing: None,
            heartbeat_interval: HEARTBEAT_INTERVAL,
            keep_interval: KEEP_INTERVAL,
            summary: None,
            change: 0,
            changed: false,
            looked: None,
            last_heartbeat: now,
            last_listing_send: None,
            asked_since: None,
            next_ask: now,
            silent: false,
            backoff: FIRST_BACKOFF,
            refused: None,
            mapping: Mapping::new(MappingType::Unknown, now),
            meets: Meets::default(),
            relays: HostRelays::default(),
            out: VecDeque::new(),
            events: VecDeque::new(),
            counters: RendezvousCounters::default(),
        }
    }

    /// The relayed addresses of the relay's channels open to this host: one
    /// per relayed player (slice J3).
    pub fn relayed(&self) -> Vec<SocketAddr> {
        self.relays.channels.addresses()
    }

    /// What the relay's channels counted.
    pub fn relay_counters(&self) -> RelayCounters {
        self.relays.channels.counters
    }

    /// The host's connection at the relayed address `relayed` ended: its
    /// channel is closed, and the master told ([`GOODBYE_COPIES`] Relay
    /// closes). Nothing happens for an address with no channel. *Agent
    /// decision:* the host loops need not call it, since the master closes a
    /// channel idle for 30 seconds and the player's end closes its own.
    pub fn close_relayed(&mut self, relayed: SocketAddr) {
        for (to, packet) in self.relays.close_address(relayed) {
            self.send(to, &packet);
        }
    }

    /// The socket as the transport should see it, for one receive or one
    /// transmit.
    pub fn over<'a, D: Datagrams + ?Sized>(
        &'a mut self,
        socket: &'a mut D,
        now: Duration,
    ) -> Routed<'a, D> {
        Routed {
            rendezvous: self,
            socket,
            now,
        }
    }

    /// The master's main addresses (IPv4 first, as a lookup gives them) and
    /// the host's own candidates. A listing made with an address no longer
    /// among them is made again with the first.
    pub fn set_masters(
        &mut self,
        masters: Vec<SocketAddr>,
        candidates: Vec<Candidate>,
        now: Duration,
    ) {
        let mut unique: Vec<SocketAddr> = Vec::new();
        for master in masters.into_iter().map(canonical) {
            if !unique.contains(&master) {
                unique.push(master);
            }
        }
        let before = self.masters.get(self.current).copied();
        self.candidates = candidates;
        self.masters = unique;
        self.current = before
            .and_then(|before| self.masters.iter().position(|m| *m == before))
            .unwrap_or(0);
        if before.is_some() && before != self.masters.get(self.current).copied() {
            self.listing = None;
            self.asked_since = None;
            self.next_ask = now;
        }
        if self.masters.is_empty() {
            return;
        }
        if before.is_none() {
            self.next_ask = self.next_ask.max(now);
            self.mapping = Mapping::new(self.mapping.result, now);
        }
    }

    /// The master's main address the listing goes to, once known.
    pub fn master(&self) -> Option<SocketAddr> {
        self.masters.get(self.current).copied()
    }

    /// True when `from` is one of the master's addresses, either port.
    pub fn is_master(&self, from: SocketAddr) -> bool {
        let from = canonical(from);
        self.masters
            .iter()
            .any(|m| *m == from || probe_address(*m) == Some(from))
    }

    /// Lists the game, or takes it off the list (three Unregisters).
    pub fn set_listed(&mut self, listed: bool, now: Duration) {
        if listed == self.wanted {
            return;
        }
        self.wanted = listed;
        if listed {
            self.nonce = self.rng.next_u64();
            self.asked_since = None;
            self.next_ask = now;
            self.silent = false;
            self.backoff = FIRST_BACKOFF;
            self.mapping = Mapping::new(self.mapping.result, now);
        } else {
            self.unregister();
            self.asked_since = None;
            self.events.push_back(RendezvousEvent::Unlisted);
        }
    }

    /// Whether the game is to be listed.
    pub fn listed_wanted(&self) -> bool {
        self.wanted
    }

    /// Takes the game off the list, as the host stops: three Unregisters to
    /// send before the socket closes.
    pub fn stop(&mut self, now: Duration) {
        self.set_listed(false, now);
    }

    /// Where the listing stands.
    pub fn state(&self) -> ListingState {
        if let Some(text) = &self.refused {
            return ListingState::Refused(text.clone());
        }
        if !self.wanted {
            return ListingState::Off;
        }
        if self.masters.is_empty() {
            return ListingState::FindingMaster;
        }
        if self.silent {
            return ListingState::Silent;
        }
        match self.listing {
            Some(listing) => ListingState::Listed {
                listing_id: listing.listing_id,
                seen: listing.seen,
            },
            None => ListingState::Registering,
        }
    }

    /// The mapping test's result so far.
    pub fn mapping(&self) -> MappingType {
        self.mapping.result
    }

    /// True when the rendezvous wants a fresh summary from the host's loop
    /// ([`Rendezvous::set_summary`]): at most once a second while listing.
    pub fn wants_summary(&self, now: Duration) -> bool {
        self.wanted && self.looked.is_none_or(|at| now >= at + SUMMARY_LOOK)
    }

    /// The host's summary: its discovery answer without the nonce. A change
    /// raises the change counter and sends a Heartbeat within 5 seconds.
    pub fn set_summary(&mut self, now: Duration, summary: ListingSummary) {
        self.looked = Some(now);
        if self.summary.as_ref() != Some(&summary) {
            if self.summary.is_some() {
                self.change = self.change.wrapping_add(1);
                self.changed = true;
            }
            self.summary = Some(summary);
        }
    }

    /// Queues a telemetry Report for the master. Nothing is sent without an
    /// install id (telemetry off) or a master address.
    pub fn report(&mut self, mut report: Report) {
        let (Some(install_id), Some(master)) = (self.config.install_id, self.master()) else {
            return;
        };
        report.install_id = install_id;
        self.send(master, &MasterPacket::Report(report));
    }

    /// The next event, oldest first.
    pub fn poll_event(&mut self) -> Option<RendezvousEvent> {
        self.events.pop_front()
    }

    /// The next master datagram to send.
    pub fn poll_transmit(&mut self) -> Option<Transmit> {
        self.out.pop_front()
    }

    /// Sends every queued master datagram on `socket`, carrying on past
    /// failures; returns the first error.
    pub fn transmit<D: Datagrams + ?Sized>(&mut self, socket: &mut D) -> io::Result<()> {
        crate::datagram::transmit_all(&mut self.out, socket)
    }

    /// A datagram from one of the master's addresses.
    pub fn receive(&mut self, now: Duration, from: SocketAddr, datagram: &[u8]) {
        self.counters.received += 1;
        let Ok(packet) = MasterPacket::decode(datagram) else {
            self.counters.malformed += 1;
            return;
        };
        let from = canonical(from);
        match packet {
            MasterPacket::Challenge(challenge) => {
                if challenge.nonce != self.nonce || self.listing.is_some() || !self.wanted {
                    self.counters.unexpected += 1;
                    return;
                }
                self.heard(now, true);
                self.send_register(challenge.cookie, now);
                // The Register with the cookie is a request of its own.
                self.asked_since = Some(now);
                self.next_ask = now + REQUEST_RETRY;
            }
            MasterPacket::Listed(listed) => {
                if listed.nonce != self.nonce || !self.wanted {
                    self.counters.unexpected += 1;
                    return;
                }
                self.heard(now, true);
                let again = self
                    .listing
                    .is_some_and(|l| l.listing_id == listed.listing_id && l.token == listed.token);
                let seconds = |secs: u8, default: Duration| {
                    if secs == 0 {
                        default
                    } else {
                        Duration::from_secs(u64::from(secs))
                    }
                };
                self.heartbeat_interval = seconds(listed.heartbeat_secs, HEARTBEAT_INTERVAL);
                self.keep_interval = seconds(listed.keep_secs, KEEP_INTERVAL);
                if !again {
                    self.listing = Some(Listing {
                        listing_id: listed.listing_id,
                        token: listed.token,
                        seen: listed.seen,
                    });
                    self.last_heartbeat = now;
                    self.events.push_back(RendezvousEvent::Listed {
                        listing_id: listed.listing_id,
                        seen: listed.seen,
                    });
                }
            }
            MasterPacket::HeartbeatAck(ack) => {
                let Some(listing) = self.listing.as_mut() else {
                    self.counters.unexpected += 1;
                    return;
                };
                if ack.listing_id != listing.listing_id {
                    self.counters.unexpected += 1;
                    return;
                }
                if ack.seen != listing.seen {
                    listing.seen = ack.seen;
                    self.events
                        .push_back(RendezvousEvent::SeenChanged(ack.seen));
                }
                self.heard(now, true);
            }
            MasterPacket::UnknownListing(unknown) => {
                if self.listing.is_none_or(|l| l.token != unknown.token) {
                    self.counters.unexpected += 1;
                    return;
                }
                // The master restarted, or dropped the listing: register again
                // at once.
                self.heard(now, true);
                self.listing = None;
                self.nonce = self.rng.next_u64();
                self.next_ask = now;
            }
            MasterPacket::Unsupported(unsupported) => {
                self.heard(now, true);
                if self.refused.is_none() {
                    let text = if unsupported.text.is_empty() {
                        "The Internet Lobby does not support this version of the game.".to_owned()
                    } else {
                        unsupported.text
                    };
                    self.events
                        .push_back(RendezvousEvent::Refused(text.clone()));
                    self.refused = Some(text);
                }
                self.listing = None;
            }
            MasterPacket::ProbeAnswer(answer) => {
                if answer.nonce != self.mapping.nonce {
                    self.counters.unexpected += 1;
                    return;
                }
                self.heard(now, false);
                match answer.port {
                    ProbePort::Main => self.mapping.main = Some(answer.seen),
                    ProbePort::Second => self.mapping.second = Some(answer.seen),
                }
                self.judge_mapping(from);
            }
            MasterPacket::Meet(meet) => {
                self.heard(now, false);
                self.meet(now, from, &meet);
            }
            MasterPacket::RelayOpen(open) => {
                self.heard(now, false);
                let token = self
                    .listing
                    .filter(|_| self.wanted)
                    .map(|listing| listing.token);
                if let Some(ack) = self.relays.open(now, from, &open, token) {
                    self.counters.relay_opens += 1;
                    self.send(from, &ack);
                }
            }
            MasterPacket::RelayClose(close) => {
                self.heard(now, false);
                if self.relays.close(&close) {
                    self.counters.relay_closes += 1;
                } else {
                    self.counters.unexpected += 1;
                }
            }
            _ => self.counters.unexpected += 1,
        }
    }

    /// A Meet: punches for the player's addresses and an ack to the master,
    /// as [`Meets`] decides. Only a listed game has the token an ack needs.
    fn meet(&mut self, now: Duration, from: SocketAddr, meet: &super::packet::Meet) {
        let Some(listing) = self.listing.filter(|_| self.wanted) else {
            self.counters.meets_dropped += 1;
            return;
        };
        match self.meets.receive(now, meet) {
            MeetOutcome::Punching => self.counters.meets += 1,
            MeetOutcome::Again => self.counters.meets_again += 1,
            MeetOutcome::OverLimit => {
                self.counters.meets_dropped += 1;
                return;
            }
        }
        self.send_punches(now);
        let ack = MasterPacket::MeetAck(MeetAck {
            token: listing.token,
            introduction_id: meet.introduction_id,
        });
        self.send(from, &ack);
    }

    /// The punches due: transport packets of the game's protocol version,
    /// sent beside the master's datagrams on the game port.
    fn send_punches(&mut self, now: Duration) {
        let version = self.config.build.protocol_version;
        for (to, introduction) in self.meets.due(now) {
            if let Ok(datagram) = Packet::Punch(Punch { introduction }).encode(version) {
                self.counters.punches += 1;
                self.out.push_back(Transmit { to, datagram });
            }
        }
    }

    /// Runs the timers: requests and their retries, heartbeats, keeps, the
    /// mapping test, silence, and a Meet's punches.
    pub fn update(&mut self, now: Duration) {
        if self.meets.punching() {
            self.send_punches(now);
        }
        self.relays.channels.forget_idle(now);
        if !self.wanted || self.refused.is_some() || self.masters.is_empty() {
            return;
        }
        if let Some(since) = self.asked_since
            && !self.silent
            && now >= since + MASTER_SILENT
        {
            self.silent = true;
            self.events.push_back(RendezvousEvent::MasterSilent);
            // The next try goes to the master's next address, if it has one.
            self.current = (self.current + 1) % self.masters.len();
            if self.listing.is_none() {
                self.nonce = self.rng.next_u64();
            }
            self.next_ask = now + self.backoff;
        }
        self.probe_if_due(now);
        let retry_due = self.asked_since.is_some() && now >= self.next_ask;
        match self.listing {
            None => {
                if self.summary.is_some() && now >= self.next_ask {
                    self.ask(now);
                }
            }
            Some(_) => {
                let gap_ok = self
                    .last_listing_send
                    .is_none_or(|at| now >= at + LISTING_GAP);
                let heartbeat_due = now >= self.last_heartbeat + self.heartbeat_interval
                    || (self.changed && now >= self.last_heartbeat + CHANGE_HEARTBEAT_DELAY);
                if gap_ok && (retry_due || (self.asked_since.is_none() && heartbeat_due)) {
                    self.ask(now);
                } else if gap_ok
                    && self.asked_since.is_none()
                    && self
                        .last_listing_send
                        .is_none_or(|at| now >= at + self.keep_interval)
                    // A Heartbeat due within the gap keeps the mapping open
                    // itself, and a Keep now would hold it back.
                    && now + LISTING_GAP < self.last_heartbeat + self.heartbeat_interval
                {
                    self.send_keep(now);
                }
            }
        }
    }

    /// Sends the request of the moment (a Register before Listed, a
    /// Heartbeat after) and sets its retry.
    fn ask(&mut self, now: Duration) {
        if self.listing.is_some() {
            self.send_heartbeat(now);
        } else {
            self.send_register(0, now);
        }
        self.asked_since.get_or_insert(now);
        self.next_ask = if self.silent {
            let at = now + self.backoff;
            self.backoff = (self.backoff * 2).min(MAX_BACKOFF);
            at
        } else {
            now + REQUEST_RETRY
        };
    }

    /// A packet from the master: it is not silent. `answer` when the packet
    /// answers the request under way.
    fn heard(&mut self, now: Duration, answer: bool) {
        if answer {
            self.asked_since = None;
            self.next_ask = now;
        }
        if self.silent {
            self.silent = false;
            self.backoff = FIRST_BACKOFF;
        }
    }

    fn send(&mut self, to: SocketAddr, packet: &MasterPacket) {
        match packet.encode() {
            Ok(datagram) => {
                self.counters.sent += 1;
                self.out.push_back(Transmit { to, datagram });
            }
            // Only a summary that breaks its limits could fail, and fit()
            // cuts it first; nothing is sent rather than a broken packet.
            Err(_) => self.counters.unexpected += 1,
        }
    }

    fn send_register(&mut self, cookie: u64, now: Duration) {
        let (Some(master), Some(summary)) = (self.master(), self.summary.clone()) else {
            return;
        };
        let register = Register {
            nonce: self.nonce,
            cookie,
            build: self.config.build.clone(),
            dedicated: self.config.dedicated,
            telemetry: self.config.install_id.is_some(),
            install_id: self.config.install_id.unwrap_or(0),
            platform: self.config.platform,
            candidates: self.candidates.clone(),
            summary,
        }
        .fit();
        self.send(master, &MasterPacket::Register(register));
        self.changed = false;
        self.last_heartbeat = now;
        self.last_listing_send = Some(now);
    }

    fn send_heartbeat(&mut self, now: Duration) {
        let (Some(master), Some(listing), Some(summary)) =
            (self.master(), self.listing, self.summary.clone())
        else {
            return;
        };
        let heartbeat = Heartbeat {
            token: listing.token,
            change: self.change,
            candidates: self.candidates.clone(),
            summary,
        }
        .fit();
        self.send(master, &MasterPacket::Heartbeat(heartbeat));
        self.changed = false;
        self.last_heartbeat = now;
        self.last_listing_send = Some(now);
    }

    fn send_keep(&mut self, now: Duration) {
        let (Some(master), Some(listing)) = (self.master(), self.listing) else {
            return;
        };
        self.send(
            master,
            &MasterPacket::Keep(Keep {
                token: listing.token,
            }),
        );
        self.last_listing_send = Some(now);
    }

    fn unregister(&mut self) {
        let (Some(master), Some(listing)) = (self.master(), self.listing.take()) else {
            return;
        };
        for _ in 0..GOODBYE_COPIES {
            self.send(
                master,
                &MasterPacket::Unregister(Unregister {
                    token: listing.token,
                }),
            );
        }
    }

    /// Sends the mapping test's two Probes when they are due: when listing
    /// starts, again while an answer is missing (three tries,
    /// [`REQUEST_RETRY`] apart), and every 10 minutes.
    fn probe_if_due(&mut self, now: Duration) {
        if now < self.mapping.next {
            return;
        }
        let Some(master) = self.master() else {
            return;
        };
        if self.mapping.done || self.mapping.tries >= PROBE_TRIES {
            self.mapping = Mapping::new(self.mapping.result, now);
        }
        if self.mapping.tries == 0 {
            self.mapping.nonce = self.rng.next_u64();
            self.mapping.started = now;
        }
        let probe = MasterPacket::Probe(Probe {
            nonce: self.mapping.nonce,
        });
        if self.mapping.main.is_none() {
            self.send(master, &probe);
        }
        if let Some(second) = probe_address(master)
            && self.mapping.second.is_none()
        {
            self.send(second, &probe);
        }
        self.mapping.tries += 1;
        self.mapping.next = if self.mapping.tries < PROBE_TRIES {
            now + REQUEST_RETRY
        } else {
            self.mapping.started + MAPPING_TEST_INTERVAL
        };
    }

    /// The test's verdict once both answers are in (or the main one shows no
    /// translation).
    fn judge_mapping(&mut self, from: SocketAddr) {
        let Some(main) = self.mapping.main else {
            return;
        };
        let second = self.mapping.second;
        let own = self
            .candidates
            .iter()
            .map(|c| c.address)
            .find(|a| a.is_ipv4() == from.is_ipv4());
        let result = MappingType::from_probes(own, main, second);
        if self.mapping.done || (second.is_none() && result != MappingType::NoTranslation) {
            return;
        }
        self.mapping.done = true;
        self.mapping.result = result;
        self.mapping.next = self.mapping.started + MAPPING_TEST_INTERVAL;
        self.events
            .push_back(RendezvousEvent::MappingTested(result));
    }
}

impl MasterSide for Rendezvous {
    fn is_master(&self, from: SocketAddr) -> bool {
        Rendezvous::is_master(self, from)
    }

    fn take_master(&mut self, now: Duration, from: SocketAddr, datagram: &[u8]) {
        self.receive(now, from, datagram);
    }

    fn channels(&mut self) -> &mut Channels {
        &mut self.relays.channels
    }

    fn claimed(&mut self) {
        self.counters.relayed_claims += 1;
    }

    fn send_dropped(&mut self) {
        self.counters.relay_sends_dropped += 1;
    }
}

/// How a remote player reached the host, as far as a host in stage I can
/// tell from the player's address: the relay's prefix, a private or
/// link-local address (the local network), else a typed or found address.
/// *Agent decision:* stage J's path in the Challenge answer replaces this.
pub fn path_of(address: SocketAddr) -> Path {
    if is_relayed(address) {
        return Path::Relay;
    }
    let local = match canonical(address).ip() {
        IpAddr::V4(ip) => ip.is_private() || ip.is_link_local() || ip.is_loopback(),
        IpAddr::V6(ip) => {
            let first = ip.segments()[0];
            ip.is_loopback() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
        }
    };
    if local {
        Path::LocalNetwork
    } else {
        Path::ByAddress
    }
}

/// What a host counts for its Report at a session's end ("Reports" in the
/// master protocol): how long, the most humans at once, and its remote
/// players by their path. The hosting player's own connection over the
/// in-process link is a human but has no path.
#[derive(Debug, Clone)]
pub struct HostTally {
    started: Duration,
    present: BTreeSet<SocketAddr>,
    counted: BTreeSet<SocketAddr>,
    most: usize,
    by_path: [u8; 6],
}

impl HostTally {
    /// A session that starts at `now`.
    pub fn new(now: Duration) -> Self {
        Self {
            started: now,
            present: BTreeSet::new(),
            counted: BTreeSet::new(),
            most: 0,
            by_path: [0; 6],
        }
    }

    /// The players connected now, by their addresses: the host's loop
    /// gives them after each join or departure.
    pub fn present(&mut self, addresses: impl IntoIterator<Item = SocketAddr>) {
        self.present = addresses.into_iter().collect();
        self.most = self.most.max(self.present.len());
        for &address in &self.present {
            if address != LINK_ADDRESS && self.counted.insert(address) {
                let slot = &mut self.by_path[usize::from(path_of(address).code())];
                *slot = slot.saturating_add(1);
            }
        }
    }

    /// True when anyone joined the session.
    pub fn anyone(&self) -> bool {
        self.most > 0
    }

    /// The session's Report, its install id filled in by
    /// [`Rendezvous::report`].
    pub fn report(
        &self,
        now: Duration,
        role: Role,
        game_version: &str,
        platform: u8,
        mapping: MappingType,
    ) -> Report {
        let mut version = game_version.to_owned();
        while version.len() > super::packet::MAX_BUILD_TEXT {
            version.pop();
        }
        Report {
            install_id: 0,
            role,
            game_version: version,
            platform,
            minutes: u16::try_from(now.saturating_sub(self.started).as_secs() / 60)
                .unwrap_or(u16::MAX),
            humans: u8::try_from(self.most).unwrap_or(u8::MAX),
            path: Path::LocalNetwork,
            connect_tenths: 0,
            mapping,
            port_mapping: PortMapping::NotTried,
            relayed_kb: 0,
            players_by_path: self.by_path,
            migrations: 0,
            failed_migrations: 0,
        }
    }
}

/// A host's listing with what needs the real world: the master's name looked
/// up on a thread (again every 10 minutes and after the master falls
/// silent), and the host's own addresses toward it. The host's loop changes
/// by a line or two:
///
/// ```ignore
/// host.receive_from(now, &mut listing.over(&mut socket, now))?;
/// host.update(now);
/// listing.update(now, || host.discover_answer(0).into());
/// host.transmit(&mut listing.over(&mut socket, now))?;
/// listing.transmit(&mut socket)?;
/// ```
#[derive(Debug)]
pub struct HostListing {
    rendezvous: Rendezvous,
    host: String,
    port: u16,
    game_port: u16,
    lookup: Option<MasterLookup>,
    next_lookup: Duration,
    lookup_backoff: Duration,
    was_silent: bool,
}

impl HostListing {
    /// A host's listing on `game_port`, with the master at `master` (a name
    /// or an address, port 26901 when none is given), not listed until
    /// [`HostListing::set_listed`].
    pub fn new(
        master: &str,
        config: HostRendezvous,
        game_port: u16,
        now: Duration,
    ) -> Result<Self, String> {
        let (host, port) = parse_master(master)?;
        Ok(Self {
            rendezvous: Rendezvous::host(config, now),
            host,
            port,
            game_port,
            lookup: None,
            next_lookup: now,
            lookup_backoff: FIRST_BACKOFF,
            was_silent: false,
        })
    }

    /// The master as given, for messages: `HOST:PORT`.
    pub fn master_text(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    /// The state machine inside.
    pub fn rendezvous(&self) -> &Rendezvous {
        &self.rendezvous
    }

    /// The state machine inside, to change.
    pub fn rendezvous_mut(&mut self) -> &mut Rendezvous {
        &mut self.rendezvous
    }

    /// See [`Rendezvous::over`].
    pub fn over<'a, D: Datagrams + ?Sized>(
        &'a mut self,
        socket: &'a mut D,
        now: Duration,
    ) -> Routed<'a, D> {
        self.rendezvous.over(socket, now)
    }

    /// See [`Rendezvous::set_listed`].
    pub fn set_listed(&mut self, listed: bool, now: Duration) {
        self.rendezvous.set_listed(listed, now);
    }

    /// The lookup, the summary when it is wanted, and the rendezvous's
    /// timers. `summary` is called at most once a second.
    pub fn update(&mut self, now: Duration, summary: impl FnOnce() -> ListingSummary) {
        if let Some(lookup) = self.lookup.as_mut()
            && let Some(answer) = lookup.poll()
        {
            self.lookup = None;
            match answer {
                Ok(masters) => {
                    let candidates = host_candidates(&masters, self.game_port, own_address_toward);
                    self.rendezvous.set_masters(masters, candidates, now);
                    self.next_lookup = now + LOOKUP_INTERVAL;
                    self.lookup_backoff = FIRST_BACKOFF;
                }
                Err(error) => {
                    self.rendezvous
                        .events
                        .push_back(RendezvousEvent::LookupFailed(error));
                    self.next_lookup = now + self.lookup_backoff;
                    self.lookup_backoff = (self.lookup_backoff * 2).min(MAX_BACKOFF);
                }
            }
        }
        let silent = self.rendezvous.state() == ListingState::Silent;
        if silent && !self.was_silent {
            // The master may have moved: look its name up again now.
            self.next_lookup = now;
        }
        self.was_silent = silent;
        if self.rendezvous.listed_wanted() && self.lookup.is_none() && now >= self.next_lookup {
            self.lookup = Some(MasterLookup::start(&self.host, self.port));
            self.next_lookup = now + LOOKUP_INTERVAL;
        }
        if self.rendezvous.wants_summary(now) {
            self.rendezvous.set_summary(now, summary());
        }
        self.rendezvous.update(now);
    }

    /// See [`Rendezvous::transmit`].
    pub fn transmit<D: Datagrams + ?Sized>(&mut self, socket: &mut D) -> io::Result<()> {
        self.rendezvous.transmit(socket)
    }

    /// See [`Rendezvous::poll_event`].
    pub fn poll_event(&mut self) -> Option<RendezvousEvent> {
        self.rendezvous.poll_event()
    }

    /// See [`Rendezvous::state`].
    pub fn state(&self) -> ListingState {
        self.rendezvous.state()
    }

    /// See [`Rendezvous::stop`].
    pub fn stop(&mut self, now: Duration) {
        self.rendezvous.stop(now);
    }
}

/// Where the listing stands, as a player reads it (the words of the
/// architecture guide's "Listing a game"), for example `Listed on the
/// Internet Lobby, seen at 203.0.113.5:26900.`
pub fn state_text(state: &ListingState) -> String {
    match state {
        ListingState::Off => "Not listed on the Internet Lobby.".into(),
        ListingState::FindingMaster => "Looking up the Internet Lobby...".into(),
        ListingState::Registering => "Listing the game on the Internet Lobby...".into(),
        ListingState::Listed { seen, .. } => {
            format!("Listed on the Internet Lobby, seen at {seen}.")
        }
        ListingState::Silent => "The Internet Lobby does not answer, so the game is not \
                                 listed. Players can still join by address."
            .into(),
        ListingState::Refused(text) => format!("The Internet Lobby refused the listing: {text}"),
    }
}

#[cfg(test)]
#[path = "rendezvous_tests.rs"]
mod tests;
