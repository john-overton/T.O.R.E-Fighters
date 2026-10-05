//! A joining player's side of the master (slice J2, "Joining through the
//! master" in the architecture guide): the mapping test, the introduction,
//! and the router of the socket the player joins with.
//!
//! [`Joiner`] is a state machine like the host's [`super::Rendezvous`]: it
//! never reads a clock or touches a socket. The player's game binds one
//! dual-stack socket (`ServerSocket::bind(Listen::Any, 0)`), gives the
//! joiner the master's addresses and its own candidates, and reads and
//! writes the socket through [`Joiner::over`], so the master sees the
//! outside address of the very socket the session then joins from:
//!
//! 1. The mapping test: one Probe to each master port, with the same nonce.
//!    The introduction waits for both answers, or for the first when it
//!    shows no translation, at most [`MAPPING_WAIT`] (agent decision: a
//!    missing second port only makes the type unknown).
//! 2. Introduce, padded to 1,000 bytes; the master's Challenge is answered
//!    with the Introduce again, carrying the cookie. An unanswered Introduce
//!    is sent again every [`INTRODUCE_RETRY`], the master's next address
//!    after [`TRIES_PER_ADDRESS`] tries, and after [`INTRODUCE_TRIES`] in
//!    all the master counts as silent (agent decisions).
//! 3. The Introduction: the host's addresses as [`Target`]s for
//!    [`crate::Client::connect_any`], each with the path a join there takes,
//!    the introduction id the host's punches carry, and the hint; or a
//!    refusal with its text.
//!
//! 4. The relay (slice J3), when the race finds no path in
//!    [`super::RACE_BEFORE_RELAY`] or the hint says so: [`Joiner::ask_relay`]
//!    sends a Relay request, again once after [`RELAY_REQUEST_RETRY`] (the
//!    master takes two a minute from one address), and the master's Relay
//!    offer opens the channel ([`JoinEvent::Relayed`]) or refuses it with
//!    its text. With no offer in [`RELAY_WAIT`] the master counts as silent
//!    (agent decisions). The game then joins the channel's relayed address
//!    with an ordinary `Client::connect`; [`Joiner::over`] frames its
//!    datagrams, and a stalled game's keepalive thread sends through
//!    [`Joiner::keepalive_socket`]. [`Joiner::close_relay`] closes the
//!    channel when the game connection ends.
//!
//! The race itself is the transport client's. What a player's router does
//! to the host's punches and the player's Connect requests is the
//! simulator's ([`crate::sim`]) to show.

use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use super::candidate::{Candidate, CandidateKind, MappingType, canonical};
use super::local::probe_address;
use super::packet::{
    Build, CloseReason, Hint, Introduce, IntroductionResult, MasterPacket, Path, Probe, ProbePort,
    RelayRequest, RelayResult,
};
use super::relay::{Channels, RelayCounters, RelayFraming, goodbye};
use super::routed::{MasterSide, relayed_address, route_receive, route_send};
use crate::client::Target;
use crate::entropy::{Entropy, Rng};
use crate::{Datagrams, Transmit};

/// The introduction waits this long at most for the mapping test's answers.
pub const MAPPING_WAIT: Duration = Duration::from_secs(1);
/// A Probe still unanswered is sent once more after this long.
pub const PROBE_RETRY: Duration = Duration::from_millis(500);
/// An unanswered Introduce is sent again this often.
pub const INTRODUCE_RETRY: Duration = Duration::from_secs(1);
/// Introduces sent to one of the master's addresses before the next.
pub const TRIES_PER_ADDRESS: u32 = 3;
/// Introduces sent in all before the master counts as silent.
pub const INTRODUCE_TRIES: u32 = 6;
/// An unanswered Relay request is sent once more after this long (the
/// master takes two a minute from one address).
pub const RELAY_REQUEST_RETRY: Duration = Duration::from_millis(1500);
/// With no Relay offer this long after the first request, the master counts
/// as silent. Long enough for the master's three Relay opens to the host
/// (750 ms) and its answer either way.
pub const RELAY_WAIT: Duration = Duration::from_secs(4);

/// What a join asks the master for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinConfig {
    /// The player's build: the master introduces only to a host of the same.
    pub build: Build,
    /// The listing chosen in the Internet Lobby.
    pub listing_id: u64,
    /// The nonces' source: [`Entropy::System`] on a real network.
    pub entropy: Entropy,
}

/// The master's introduction, as the race needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Introduced {
    /// The id the host's punches carry.
    pub introduction_id: u64,
    /// Race the addresses, or ask for the relay at once.
    pub hint: Hint,
    /// The player's own address as the master saw it.
    pub seen: SocketAddr,
    /// How the host's router maps its game port.
    pub host_mapping: MappingType,
    /// The host's addresses with their paths, in the master's order (seen,
    /// mapped, global IPv6, local).
    pub targets: Vec<Target>,
}

/// What the joiner tells the player's game, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinEvent {
    /// The mapping test's result: how the player's router maps the socket.
    MappingTested(MappingType),
    /// The master introduced the player to the host.
    Introduced(Introduced),
    /// The master would not introduce the player: why, and its text.
    Refused {
        /// The Introduction's result.
        result: IntroductionResult,
        /// The plain reason the player reads.
        text: String,
    },
    /// The master did not answer the Introduce.
    MasterSilent,
    /// The master does not speak this build's master protocol: its text.
    Unsupported(String),
    /// The relay's channel is open: join the host at this relayed address
    /// (slice J3).
    Relayed {
        /// The channel's relayed address, the host as the transport sees it.
        address: SocketAddr,
    },
    /// The master would not relay: why, and its text.
    RelayRefused {
        /// The Relay offer's result.
        result: RelayResult,
        /// The plain reason the player reads.
        text: String,
    },
    /// The master did not answer the Relay request.
    RelaySilent,
    /// The master closed the channel (idle, over its rate, the allowance
    /// spent, the master stopping): the game connection through it is lost.
    RelayClosed(CloseReason),
}

/// Where a join's relay stands (slice J3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayState {
    /// Not asked for.
    None,
    /// Asked for; no offer yet.
    Asking,
    /// The channel is open.
    Open {
        /// The channel.
        channel: u32,
    },
    /// Refused, silent or closed.
    Ended,
}

/// Where the join stands with the master.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinState {
    /// The master's address is not known yet.
    FindingMaster,
    /// The mapping test is under way.
    Testing,
    /// Asking for the introduction.
    Introducing,
    /// Introduced: the race is the client's.
    Introduced,
    /// Refused, silent or unsupported: the join through the master ended.
    Ended,
}

/// What the joiner counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JoinCounters {
    /// Master packets sent.
    pub sent: u64,
    /// Datagrams from the master's addresses taken out of the stream.
    pub received: u64,
    /// Of those, ones that did not decode.
    pub malformed: u64,
    /// Good packets this side does not take, or not now.
    pub unexpected: u64,
    /// Datagrams from a real socket claiming a relayed address, dropped.
    pub relayed_claims: u64,
    /// Sends by the transport to a relayed address with no open channel,
    /// dropped.
    pub relay_sends_dropped: u64,
    /// Relay requests sent.
    pub relay_requests: u64,
}

/// A joining player's side of the master: see the module documentation.
#[derive(Debug)]
pub struct Joiner {
    config: JoinConfig,
    rng: Rng,
    masters: Vec<SocketAddr>,
    current: usize,
    candidates: Vec<Candidate>,
    state: JoinState,
    probe_nonce: u64,
    probed_at: Duration,
    probe_retried: bool,
    main: Option<SocketAddr>,
    second: Option<SocketAddr>,
    mapping: MappingType,
    nonce: u64,
    cookie: u64,
    next_introduce: Duration,
    tries: u32,
    introduced: Option<Introduced>,
    relay: RelayState,
    /// The master's address the relay was asked of.
    relay_master: Option<SocketAddr>,
    relay_asked: Duration,
    relay_requests: u32,
    channels: Channels,
    out: VecDeque<Transmit>,
    events: VecDeque<JoinEvent>,
    /// What the joiner counted.
    pub counters: JoinCounters,
}

/// The transport target a host candidate stands for: the path a join that
/// it answers took ("The path in the Challenge answer" in the net protocol).
pub fn target_of(candidate: &Candidate) -> Target {
    let path = match candidate.kind {
        CandidateKind::Seen => Path::Punched,
        CandidateKind::Mapped => Path::MappedPort,
        CandidateKind::GlobalIpv6 => Path::Ipv6,
        CandidateKind::Local => Path::LocalNetwork,
    };
    Target::new(candidate.address, path)
}

/// The targets of an Introduction's host candidates, in its order, each
/// address once (the first). A seen address that is also one of the host's
/// own (its Mapped or Global IPv6 candidate) takes that candidate's path:
/// the game can tell a mapped port or an address without translation from a
/// punched hole then (agent decision).
pub fn targets_of(candidates: &[Candidate]) -> Vec<Target> {
    let mut targets: Vec<Target> = Vec::new();
    for candidate in candidates {
        let address = canonical(candidate.address);
        if targets.iter().any(|t| canonical(t.address) == address) {
            continue;
        }
        let own = candidates.iter().find(|c| {
            candidate.kind == CandidateKind::Seen
                && matches!(c.kind, CandidateKind::Mapped | CandidateKind::GlobalIpv6)
                && canonical(c.address) == address
        });
        targets.push(target_of(own.unwrap_or(candidate)));
    }
    targets
}

impl Joiner {
    /// A join of the listing in `config`, waiting for the master's address.
    pub fn new(config: JoinConfig, now: Duration) -> Self {
        let mut rng = Rng::new(config.entropy);
        let probe_nonce = rng.next_u64();
        let nonce = rng.next_u64();
        Self {
            config,
            rng,
            masters: Vec::new(),
            current: 0,
            candidates: Vec::new(),
            state: JoinState::FindingMaster,
            probe_nonce,
            probed_at: now,
            probe_retried: false,
            main: None,
            second: None,
            mapping: MappingType::Unknown,
            nonce,
            cookie: 0,
            next_introduce: now,
            tries: 0,
            introduced: None,
            relay: RelayState::None,
            relay_master: None,
            relay_asked: now,
            relay_requests: 0,
            channels: Channels::default(),
            out: VecDeque::new(),
            events: VecDeque::new(),
            counters: JoinCounters::default(),
        }
    }

    /// The master's main addresses (IPv4 first, as a lookup gives them) and
    /// the player's own candidates (Local and Global IPv6, with the socket's
    /// port). The first time, the mapping test starts.
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
        self.masters = unique;
        self.current = 0;
        self.candidates = candidates
            .into_iter()
            .filter(|c| matches!(c.kind, CandidateKind::Local | CandidateKind::GlobalIpv6))
            .collect();
        if self.state == JoinState::FindingMaster && !self.masters.is_empty() {
            self.state = JoinState::Testing;
            self.probed_at = now;
            self.send_probes();
        }
    }

    /// The master's main address asked now, once known.
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

    /// Where the join stands.
    pub fn state(&self) -> JoinState {
        self.state
    }

    /// The mapping test's result so far.
    pub fn mapping(&self) -> MappingType {
        self.mapping
    }

    /// The introduction, once the master made it.
    pub fn introduced(&self) -> Option<&Introduced> {
        self.introduced.as_ref()
    }

    /// Asks the master for the relay (slice J3): after the race's 3 seconds
    /// with no path, or at once on the hint. False when there is no
    /// introduction to name, or the relay was asked for already.
    pub fn ask_relay(&mut self, now: Duration) -> bool {
        if self.introduced.is_none() || self.relay != RelayState::None {
            return false;
        }
        let Some(master) = self.master() else {
            return false;
        };
        self.relay = RelayState::Asking;
        self.relay_master = Some(master);
        self.relay_asked = now;
        self.send_relay_request();
        true
    }

    /// Where the relay stands.
    pub fn relay_state(&self) -> RelayState {
        self.relay
    }

    /// The channel's relayed address once it is open: the host as the
    /// transport sees it.
    pub fn relayed(&self) -> Option<SocketAddr> {
        match self.relay {
            RelayState::Open { channel } => Some(relayed_address(channel)),
            _ => None,
        }
    }

    /// What the relay's channel counted.
    pub fn relay_counters(&self) -> RelayCounters {
        self.channels.counters
    }

    /// The game connection through the relay ended: the channel is closed
    /// and the master told ([`super::GOODBYE_COPIES`] Relay closes).
    pub fn close_relay(&mut self) {
        let RelayState::Open { channel } = self.relay else {
            return;
        };
        self.relay = RelayState::Ended;
        if let Some((key, master)) = self.channels.get(channel)
            && self.channels.close(channel, key).is_some()
        {
            for packet in goodbye(channel, key) {
                self.send(master, &packet);
            }
        }
    }

    /// A clone of the joining socket wrapped so that what the game's
    /// keepalive thread sends to the relayed host goes out as a Relay frame:
    /// `None` until the channel is open.
    pub fn keepalive_socket<S>(&self, socket: S) -> Option<RelayFraming<S>> {
        let RelayState::Open { channel } = self.relay else {
            return None;
        };
        let (key, master) = self.channels.get(channel)?;
        Some(RelayFraming::new(socket, master, channel, key))
    }

    fn send_relay_request(&mut self) {
        let (Some(master), Some(introduced)) = (self.relay_master, self.introduced.as_ref()) else {
            return;
        };
        let request = MasterPacket::RelayRequest(RelayRequest {
            nonce: self.nonce,
            introduction_id: introduced.introduction_id,
        });
        self.send(master, &request);
        self.relay_requests += 1;
        self.counters.relay_requests += 1;
    }

    /// The socket as the transport should see it, for one receive or one
    /// transmit: the master's datagrams go to the joiner, never to the
    /// transport.
    pub fn over<'a, D: Datagrams + ?Sized>(
        &'a mut self,
        socket: &'a mut D,
        now: Duration,
    ) -> JoinRouted<'a, D> {
        JoinRouted {
            joiner: self,
            socket,
            now,
        }
    }

    /// The next event, oldest first.
    pub fn poll_event(&mut self) -> Option<JoinEvent> {
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

    /// Timers: the mapping test's wait, the Introduce's retries, the Relay
    /// request's retry and wait.
    pub fn update(&mut self, now: Duration) {
        if self.relay == RelayState::Asking {
            let waited = now.saturating_sub(self.relay_asked);
            if waited >= RELAY_WAIT {
                self.relay = RelayState::Ended;
                self.events.push_back(JoinEvent::RelaySilent);
            } else if waited >= RELAY_REQUEST_RETRY && self.relay_requests < 2 {
                self.send_relay_request();
            }
        }
        match self.state {
            JoinState::Testing => {
                let waited = now.saturating_sub(self.probed_at);
                if waited >= MAPPING_WAIT {
                    self.judge();
                } else if waited >= PROBE_RETRY && !self.probe_retried {
                    self.probe_retried = true;
                    self.send_probes();
                }
            }
            JoinState::Introducing if now >= self.next_introduce => {
                if self.tries >= INTRODUCE_TRIES {
                    self.state = JoinState::Ended;
                    self.events.push_back(JoinEvent::MasterSilent);
                    return;
                }
                if self.tries > 0 && self.tries.is_multiple_of(TRIES_PER_ADDRESS) {
                    // The master's next address; a cookie is for one address.
                    self.current = (self.current + 1) % self.masters.len().max(1);
                    self.cookie = 0;
                }
                self.send_introduce(now);
            }
            _ => {}
        }
        if self.state == JoinState::Introducing && self.tries == 0 {
            self.send_introduce(now);
        }
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
            MasterPacket::ProbeAnswer(answer) => {
                if answer.nonce != self.probe_nonce || self.state != JoinState::Testing {
                    self.counters.unexpected += 1;
                    return;
                }
                match answer.port {
                    ProbePort::Main => self.main = Some(answer.seen),
                    ProbePort::Second => self.second = Some(answer.seen),
                }
                let own = self.own_toward(from);
                let result = self
                    .main
                    .map(|main| MappingType::from_probes(own, main, self.second));
                if result == Some(MappingType::NoTranslation) || self.second.is_some() {
                    self.judge();
                }
                // An answer means the master is there: introduce at once.
                if self.state == JoinState::Introducing {
                    self.send_introduce(now);
                }
            }
            MasterPacket::Challenge(challenge) => {
                if challenge.nonce != self.nonce || self.state != JoinState::Introducing {
                    self.counters.unexpected += 1;
                    return;
                }
                self.cookie = challenge.cookie;
                self.send_introduce(now);
            }
            MasterPacket::Introduction(introduction) => {
                if introduction.nonce != self.nonce || self.state != JoinState::Introducing {
                    self.counters.unexpected += 1;
                    return;
                }
                if introduction.result != IntroductionResult::Introduced {
                    self.state = JoinState::Ended;
                    self.events.push_back(JoinEvent::Refused {
                        result: introduction.result,
                        text: if introduction.text.is_empty() {
                            refusal_text(introduction.result).to_owned()
                        } else {
                            introduction.text
                        },
                    });
                    return;
                }
                let introduced = Introduced {
                    introduction_id: introduction.introduction_id,
                    hint: introduction.hint,
                    seen: introduction.seen,
                    host_mapping: introduction.host_mapping,
                    targets: targets_of(&introduction.host_candidates),
                };
                self.state = JoinState::Introduced;
                self.introduced = Some(introduced.clone());
                self.events.push_back(JoinEvent::Introduced(introduced));
            }
            MasterPacket::RelayOffer(offer) => {
                let ours = self.relay == RelayState::Asking
                    && offer.nonce == self.nonce
                    && self
                        .introduced
                        .as_ref()
                        .is_some_and(|i| i.introduction_id == offer.introduction_id);
                if !ours {
                    self.counters.unexpected += 1;
                    return;
                }
                if offer.result == RelayResult::Open
                    && self.channels.open(offer.channel, offer.key, from, now)
                {
                    self.relay = RelayState::Open {
                        channel: offer.channel,
                    };
                    self.events.push_back(JoinEvent::Relayed {
                        address: relayed_address(offer.channel),
                    });
                } else {
                    self.relay = RelayState::Ended;
                    let result = offer.result;
                    let text = if offer.text.is_empty() || result == RelayResult::Open {
                        super::relay::refusal_text(result).to_owned()
                    } else {
                        offer.text
                    };
                    self.events
                        .push_back(JoinEvent::RelayRefused { result, text });
                }
            }
            MasterPacket::RelayClose(close) => {
                let RelayState::Open { channel } = self.relay else {
                    self.counters.unexpected += 1;
                    return;
                };
                if close.channel != channel || self.channels.close(channel, close.key).is_none() {
                    self.counters.unexpected += 1;
                    return;
                }
                self.relay = RelayState::Ended;
                self.events.push_back(JoinEvent::RelayClosed(close.reason));
            }
            MasterPacket::Unsupported(unsupported) => {
                if self.state == JoinState::Ended || self.state == JoinState::Introduced {
                    self.counters.unexpected += 1;
                    return;
                }
                self.state = JoinState::Ended;
                let text = if unsupported.text.is_empty() {
                    "The Internet Lobby does not support this version of the game.".to_owned()
                } else {
                    unsupported.text
                };
                self.events.push_back(JoinEvent::Unsupported(text));
            }
            _ => self.counters.unexpected += 1,
        }
    }

    /// The player's own address of `toward`'s family, when it knows it.
    fn own_toward(&self, toward: SocketAddr) -> Option<SocketAddr> {
        self.candidates
            .iter()
            .map(|c| c.address)
            .find(|a| a.is_ipv4() == toward.is_ipv4())
    }

    /// The mapping test is over: both answers are in, the first shows no
    /// translation, or the wait is up.
    fn judge(&mut self) {
        if self.state != JoinState::Testing {
            return;
        }
        let Some(master) = self.master() else {
            return;
        };
        self.mapping = match self.main {
            Some(main) => MappingType::from_probes(self.own_toward(master), main, self.second),
            None => MappingType::Unknown,
        };
        self.events
            .push_back(JoinEvent::MappingTested(self.mapping));
        self.state = JoinState::Introducing;
    }

    fn send(&mut self, to: SocketAddr, packet: &MasterPacket) {
        if let Ok(datagram) = packet.encode() {
            self.counters.sent += 1;
            self.out.push_back(Transmit { to, datagram });
        }
    }

    fn send_probes(&mut self) {
        let Some(master) = self.master() else {
            return;
        };
        let probe = MasterPacket::Probe(Probe {
            nonce: self.probe_nonce,
        });
        if self.main.is_none() {
            self.send(master, &probe);
        }
        if let Some(second) = probe_address(master)
            && self.second.is_none()
        {
            self.send(second, &probe);
        }
    }

    fn send_introduce(&mut self, now: Duration) {
        let Some(master) = self.master() else {
            return;
        };
        let introduce = MasterPacket::Introduce(Introduce {
            nonce: self.nonce,
            cookie: self.cookie,
            listing_id: self.config.listing_id,
            build: self.config.build.clone(),
            mapping: self.mapping,
            candidates: self.candidates.clone(),
        });
        self.send(master, &introduce);
        self.tries += 1;
        self.next_introduce = now + INTRODUCE_RETRY;
    }

    /// A fresh nonce for asking again (a new join of the same listing).
    pub fn restart(&mut self, now: Duration) {
        self.nonce = self.rng.next_u64();
        self.probe_nonce = self.rng.next_u64();
        self.cookie = 0;
        self.tries = 0;
        self.main = None;
        self.second = None;
        self.introduced = None;
        self.probe_retried = false;
        self.close_relay();
        self.relay = RelayState::None;
        self.relay_master = None;
        self.relay_requests = 0;
        self.state = JoinState::FindingMaster;
        if !self.masters.is_empty() {
            self.state = JoinState::Testing;
            self.probed_at = now;
            self.send_probes();
        }
    }
}

/// What a player reads when the master refuses an introduction without a
/// text of its own.
pub fn refusal_text(result: IntroductionResult) -> &'static str {
    match result {
        IntroductionResult::Introduced => "Introduced.",
        IntroductionResult::NoListing => "That game is no longer listed.",
        IntroductionResult::OtherBuild => "That game runs another version of T.O.R.E.",
        IntroductionResult::Full => "That game is full.",
        IntroductionResult::TooMany => {
            "Too many joins at once from this address. Wait a minute and try again."
        }
    }
}

/// The joining socket seen through a [`Joiner`]: every datagram from one of
/// the master's addresses goes to the joiner and never to the transport,
/// but a Relay frame of the open channel, which reaches the transport as a
/// datagram from the channel's relayed address; a datagram from a real
/// socket that claims an address in the relayed prefix is dropped and
/// counted; a send to the relayed address goes to the master as a frame;
/// everything else passes unchanged. Made with [`Joiner::over`] for one
/// receive or one transmit. *Agent decisions:* a router of its own beside
/// the host's [`super::Routed`] (J2), the two sharing their code
/// ([`super::routed`], J3).
pub struct JoinRouted<'a, D: ?Sized> {
    joiner: &'a mut Joiner,
    socket: &'a mut D,
    now: Duration,
}

impl<D: Datagrams + ?Sized> Datagrams for JoinRouted<'_, D> {
    fn send_datagram(&mut self, to: SocketAddr, datagram: &[u8]) -> io::Result<()> {
        route_send(self.joiner, self.socket, to, datagram, self.now)
    }

    fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        route_receive(self.joiner, self.socket, buf, self.now)
    }
}

impl MasterSide for Joiner {
    fn is_master(&self, from: SocketAddr) -> bool {
        Joiner::is_master(self, from)
    }

    fn take_master(&mut self, now: Duration, from: SocketAddr, datagram: &[u8]) {
        self.receive(now, from, datagram);
    }

    fn channels(&mut self) -> &mut Channels {
        &mut self.channels
    }

    fn claimed(&mut self) {
        self.counters.relayed_claims += 1;
    }

    fn send_dropped(&mut self) {
        self.counters.relay_sends_dropped += 1;
    }
}

#[cfg(test)]
#[path = "join_tests.rs"]
mod tests;
