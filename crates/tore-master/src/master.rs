//! `Master`: the master server as a state machine. Like the transport it
//! never reads a clock or touches a socket: the caller passes the time in,
//! feeds it the datagrams each port receives, calls [`Master::update`] for
//! its timers and sends what [`Master::poll_transmit`] gives from the port it
//! names. The run loop ([`crate::run`]) does that on real sockets, and the
//! tests on the network simulator.
//!
//! Every answer the master sends is fitted to the request it answers and is
//! never longer than it ("Proving an address" in the master protocol), so the
//! master is never a reflector, whoever claims to have sent a request. Every
//! request is limited per source ([`crate::limits`]) and every answer
//! together by the answer rate.

use std::collections::VecDeque;
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use tore_net::master::candidate::canonical;
use tore_net::master::packet::{MAX_MASTER_DATAGRAM, peek_kind};
use tore_net::master::{
    CandidateKind, Challenge, CookieKey, Details, Heartbeat, HeartbeatAck, Introduce, Keep, Listed,
    ListingDetails, MasterDecodeError, MasterKind, MasterPacket, Probe, ProbeAnswer, ProbePort,
    Register, RelayFrame, RelayRequest, Report, SUPPORTED_VERSIONS, UnknownListing, Unregister,
    Unsupported,
};
use tore_net::{Datagrams, Entropy, MAX_RECEIVE_BATCH, SplitMix64};

use crate::browse;
use crate::introduce::Introductions;
use crate::limits::{Bucket, Limit, Rate, Rates, SourceKey, Sources, Taken};
use crate::listings::{BEAT_RATE, Gone, Listing, Listings, MOVE_INTERVAL};
use crate::probe::MappingTests;
use crate::relay::{RelaySettings, Relays};
use crate::telemetry::Telemetry;

/// Which of the master's two ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MasterPort {
    /// The main port (26901): everything.
    Main,
    /// The second port of the mapping test (26902): Probes only.
    Probe,
}

/// The master's numbers. [`Settings::default`] gives the defaults of the
/// operations guide's configuration table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Listings at once.
    pub max_listings: usize,
    /// Listings from one source.
    pub listings_per_source: u32,
    /// The heartbeat interval the master tells hosts.
    pub heartbeat: Duration,
    /// The keep interval it tells hosts.
    pub keep: Duration,
    /// A listing not heard from for this long is dropped.
    pub expiry: Duration,
    /// The rates per source.
    pub rates: Rates,
    /// Answers of every kind together, a second.
    pub answer_rate: u32,
    /// Sources remembered.
    pub max_sources: usize,
    /// Whether reports are counted.
    pub telemetry: bool,
    /// The relay's settings (slice J3).
    pub relay: RelaySettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_listings: 2_000,
            listings_per_source: 8,
            heartbeat: tore_net::master::HEARTBEAT_INTERVAL,
            keep: tore_net::master::KEEP_INTERVAL,
            expiry: tore_net::master::LISTING_EXPIRY,
            rates: Rates::default(),
            answer_rate: 5_000,
            max_sources: 65_536,
            telemetry: true,
            relay: RelaySettings::default(),
        }
    }
}

/// A datagram the master sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    /// The port it leaves from.
    pub port: MasterPort,
    /// Where to.
    pub to: SocketAddr,
    /// The bytes.
    pub datagram: Vec<u8>,
}

/// What the master has done since it started.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counters {
    /// Datagrams received.
    pub received: u64,
    /// Bytes received.
    pub bytes_in: u64,
    /// Datagrams sent.
    pub sent: u64,
    /// Bytes sent.
    pub bytes_out: u64,
    /// Too short, too long, a failed checksum or an unknown kind: dropped
    /// silently.
    pub invalid: u64,
    /// A good checksum but broken fields.
    pub malformed: u64,
    /// A master protocol version this master does not speak.
    pub unsupported: u64,
    /// A kind the master never receives (its own answers), or a packet
    /// other than a Probe on the second port.
    pub unexpected: u64,
    /// Requests over a per-source or per-listing limit.
    pub dropped_limit: u64,
    /// Answers over the answer rate.
    pub dropped_answers: u64,
    /// Registers with a good cookie refused for want of room (8 per source,
    /// 2,000 in all), answered with nothing.
    pub refused_room: u64,
    /// Challenges sent.
    pub challenges: u64,
    /// Browse requests answered.
    pub browses: u64,
    /// Details requests answered.
    pub details: u64,
    /// Probes answered.
    pub probes: u64,
    /// Heartbeats taken.
    pub heartbeats: u64,
    /// Keeps taken.
    pub keeps: u64,
    /// Heartbeats, Keeps and Unregisters with a token the master does not
    /// know.
    pub unknown_tokens: u64,
    /// Listings made.
    pub listed: u64,
    /// Listings removed, for any reason.
    pub unlisted: u64,
    /// Reports counted.
    pub reports: u64,
    /// Reports not counted (telemetry off, or the load tool's).
    pub reports_ignored: u64,
}

/// A generator for listing ids and tokens: unpredictable on a real network,
/// repeatable for a seed in tests.
#[derive(Debug, Clone)]
enum Ids {
    System(u64),
    Seeded(SplitMix64),
}

impl Ids {
    fn new(entropy: Entropy) -> Self {
        match entropy {
            Entropy::System => Self::System(0),
            Entropy::Seeded(seed) => Self::Seeded(SplitMix64::new(seed ^ 0x6d61_7374_6572)),
        }
    }

    fn next(&mut self) -> u64 {
        match self {
            Self::System(counter) => {
                *counter = counter.wrapping_add(1);
                RandomState::new().hash_one(*counter)
            }
            Self::Seeded(rng) => rng.next_u64(),
        }
    }
}

/// The master server.
#[derive(Debug)]
pub struct Master {
    settings: Settings,
    key: CookieKey,
    ids: Ids,
    sources: Sources,
    answers: Bucket,
    listings: Listings,
    tests: MappingTests,
    introductions: Introductions,
    relays: Relays,
    telemetry: Telemetry,
    counters: Counters,
    out: VecDeque<Outgoing>,
    log: VecDeque<String>,
}

impl Master {
    /// A master with `settings`, its secrets drawn from `entropy`
    /// ([`Entropy::System`] on a real network), counting telemetry for day
    /// `day` (days since 1970-01-01, UTC).
    pub fn new(settings: Settings, entropy: Entropy, day: i64) -> Self {
        Self {
            key: CookieKey::new(entropy),
            ids: Ids::new(entropy),
            sources: Sources::new(settings.rates, settings.max_sources),
            answers: Bucket::default(),
            listings: Listings::default(),
            tests: MappingTests::default(),
            introductions: Introductions::new(entropy),
            relays: Relays::new(settings.relay, entropy),
            telemetry: Telemetry::new(day, entropy),
            counters: Counters::default(),
            out: VecDeque::new(),
            log: VecDeque::new(),
            settings,
        }
    }

    /// The settings in force.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// What it has done.
    pub fn counters(&self) -> Counters {
        self.counters
    }

    /// The listings.
    pub fn listings(&self) -> &Listings {
        &self.listings
    }

    /// Sources remembered.
    pub fn sources(&self) -> usize {
        self.sources.len()
    }

    /// Introductions (stage J).
    pub fn introductions(&self) -> &Introductions {
        &self.introductions
    }

    /// The relay (stage J).
    pub fn relays(&self) -> &Relays {
        &self.relays
    }

    /// The relay, to resume or roll its month's figure.
    pub fn relays_mut(&mut self) -> &mut Relays {
        &mut self.relays
    }

    /// The master is stopping: every relay channel is closed and both its
    /// ends told (Relay close, reason 4), to send before the sockets close.
    pub fn stop(&mut self) {
        self.relays.stop();
        self.send_relays();
    }

    /// The day's telemetry counts.
    pub fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }

    /// The day's telemetry counts, to resume or roll over.
    pub fn telemetry_mut(&mut self) -> &mut Telemetry {
        &mut self.telemetry
    }

    /// The next datagram to send.
    pub fn poll_transmit(&mut self) -> Option<Outgoing> {
        let out = self.out.pop_front()?;
        self.counters.sent += 1;
        self.counters.bytes_out += out.datagram.len() as u64;
        Some(out)
    }

    /// The next notable event for the log: a listing made, moved or removed,
    /// a source over a limit, a relay channel opened or closed, the relay's
    /// allowance.
    pub fn poll_log(&mut self) -> Option<String> {
        self.log.pop_front().or_else(|| self.relays.poll_log())
    }

    /// Timers: listings that expire, Meets due again, introductions to
    /// forget.
    pub fn update(&mut self, now: Duration) {
        for listing in self.listings.expire(now, self.settings.expiry) {
            self.unlisted(&listing, Gone::Expired);
        }
        self.introductions.update(now);
        self.send_meets();
        self.relays.update(now);
        self.send_relays();
    }

    /// Queues what the relay wants sent from the main port: forwarded
    /// frames, Relay opens, offers and closes. None of them is an answer
    /// fitted to a request (agent decision, as the Meets): each goes to an
    /// address proven by a listing or an introduction.
    fn send_relays(&mut self) {
        while let Some((to, datagram)) = self.relays.poll_send() {
            self.out.push_back(Outgoing {
                port: MasterPort::Main,
                to,
                datagram,
            });
        }
    }

    /// Queues the Meets the introductions want sent, from the main port. A
    /// Meet goes to a host whose address its listing proves, on behalf of a
    /// player whose address its cookie proves, so it is not an answer and
    /// is not fitted to a request (agent decision).
    fn send_meets(&mut self) {
        while let Some((to, datagram)) = self.introductions.poll_send() {
            self.out.push_back(Outgoing {
                port: MasterPort::Main,
                to,
                datagram,
            });
        }
    }

    /// Reads up to [`MAX_RECEIVE_BATCH`] waiting datagrams from the socket of
    /// `port`. Returns how many it read.
    pub fn receive_from<D: Datagrams + ?Sized>(
        &mut self,
        now: Duration,
        port: MasterPort,
        socket: &mut D,
    ) -> io::Result<usize> {
        // One byte over the longest datagram, so a longer one is seen as such.
        let mut buf = [0u8; MAX_MASTER_DATAGRAM + 1];
        let mut read = 0;
        while read < MAX_RECEIVE_BATCH {
            let Some((len, from)) = socket.recv_datagram(&mut buf)? else {
                break;
            };
            read += 1;
            self.receive(now, port, from, &buf[..len]);
        }
        Ok(read)
    }

    /// Sends everything queued, each from its port's socket; what is for the
    /// second port is dropped when there is none. Carries on past failures
    /// and returns the first.
    pub fn transmit<D: Datagrams + ?Sized>(
        &mut self,
        main: &mut D,
        mut probe: Option<&mut D>,
    ) -> io::Result<()> {
        let mut first_error = None;
        while let Some(out) = self.poll_transmit() {
            let socket: &mut D = match out.port {
                MasterPort::Main => main,
                MasterPort::Probe => match probe.as_deref_mut() {
                    Some(probe) => probe,
                    None => continue,
                },
            };
            if let Err(error) = socket.send_datagram(out.to, &out.datagram) {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// One datagram that arrived at `port` from `from`.
    pub fn receive(&mut self, now: Duration, port: MasterPort, from: SocketAddr, bytes: &[u8]) {
        let from = canonical(from);
        self.counters.received += 1;
        self.counters.bytes_in += bytes.len() as u64;
        // Relay frames take the fast path: read in place, never copied.
        if port == MasterPort::Main && peek_kind(bytes) == Some(MasterKind::Relay) {
            match RelayFrame::open(bytes) {
                Ok(frame) => {
                    self.relays.frame(now, from, &frame, bytes);
                    self.send_relays();
                }
                Err(error) => self.refuse(now, port, from, bytes.len(), error),
            }
            return;
        }
        let packet = match MasterPacket::decode(bytes) {
            Ok(packet) => packet,
            Err(error) => return self.refuse(now, port, from, bytes.len(), error),
        };
        let len = bytes.len();
        match (port, packet) {
            (_, MasterPacket::Probe(probe)) => self.probe(now, port, from, probe, len),
            (MasterPort::Probe, _) => self.counters.unexpected += 1,
            (_, MasterPacket::Register(register)) => self.register(now, from, register, len),
            (_, MasterPacket::Heartbeat(beat)) => self.heartbeat(now, from, beat, len),
            (_, MasterPacket::Keep(Keep { token })) => self.keep(now, from, token, len),
            (_, MasterPacket::Unregister(Unregister { token })) => self.unregister(token),
            (_, MasterPacket::Browse(request)) => self.browse(now, from, request, len),
            (_, MasterPacket::Details(request)) => self.details(now, from, request, len),
            (_, MasterPacket::Report(report)) => self.report(now, from, report),
            (_, MasterPacket::Introduce(request)) => self.introduce(now, from, request, len),
            (_, MasterPacket::MeetAck(ack)) => {
                self.introductions.meet_ack(now, from, &ack, &self.listings)
            }
            (_, MasterPacket::RelayRequest(request)) => self.relay_request(now, from, request),
            (_, MasterPacket::RelayOpenAck(ack)) => {
                self.relays.open_ack(now, from, &ack, &self.listings);
                self.send_relays();
            }
            (_, MasterPacket::RelayClose(close)) => {
                self.relays.close(now, from, &close);
                self.send_relays();
            }
            // The master's own kinds: nobody sends them to it.
            (_, _) => self.counters.unexpected += 1,
        }
    }

    /// A datagram that did not decode.
    fn refuse(
        &mut self,
        now: Duration,
        port: MasterPort,
        from: SocketAddr,
        len: usize,
        error: MasterDecodeError,
    ) {
        match error {
            MasterDecodeError::Malformed => self.counters.malformed += 1,
            MasterDecodeError::Unsupported { version, .. } => {
                self.counters.unsupported += 1;
                let Some(quiet) = self.limit(now, from, Limit::Query) else {
                    return;
                };
                let text = if version < *SUPPORTED_VERSIONS.start() {
                    "This game is older than the Internet Lobby supports. Update the game to see internet games."
                } else {
                    "This game is newer than the Internet Lobby. Internet games return once the lobby is updated."
                };
                let answer = MasterPacket::Unsupported(Unsupported {
                    lowest: *SUPPORTED_VERSIONS.start(),
                    highest: *SUPPORTED_VERSIONS.end(),
                    text: text.into(),
                });
                self.answer_in(now, port, from, version, &answer, len, quiet);
            }
            _ => self.counters.invalid += 1,
        }
    }

    /// Takes one request of `limit` from `from`'s source: `Some(quiet)`
    /// within the limit, `None` over it (counted, and logged once a minute).
    fn limit(&mut self, now: Duration, from: SocketAddr, limit: Limit) -> Option<bool> {
        let source = SourceKey::of(from);
        match self.sources.take(source, limit, now) {
            Taken::Yes { quiet } => Some(quiet),
            Taken::Over { log } => {
                self.counters.dropped_limit += 1;
                if log {
                    let rate = self.sources.rates().of(limit);
                    self.log.push_back(format!(
                        "limit source={source} over={} ({rate})",
                        limit.name()
                    ));
                }
                None
            }
        }
    }

    /// Queues an answer in the master's version from the main port.
    fn answer(
        &mut self,
        now: Duration,
        to: SocketAddr,
        packet: &MasterPacket,
        request_len: usize,
        quiet: bool,
    ) {
        let version = tore_net::master::MASTER_VERSION;
        self.answer_in(
            now,
            MasterPort::Main,
            to,
            version,
            packet,
            request_len,
            quiet,
        );
    }

    /// Queues an answer, never longer than the request it answers, within the
    /// answer rate. While more than half the second's answers are spent, only
    /// quiet sources (those that have used at most half their own burst) are
    /// answered, so a flood is dropped before a player who asks now and then
    /// (agent decision: the busiest sources are dropped first).
    #[allow(clippy::too_many_arguments)]
    fn answer_in(
        &mut self,
        now: Duration,
        port: MasterPort,
        to: SocketAddr,
        version: u16,
        packet: &MasterPacket,
        request_len: usize,
        quiet: bool,
    ) {
        let rate = Rate::per_second(self.settings.answer_rate, self.settings.answer_rate);
        if (!quiet && !self.answers.quiet(now, rate)) || !self.answers.take(now, rate) {
            self.counters.dropped_answers += 1;
            return;
        }
        match packet.encode_within(version, request_len) {
            Ok(datagram) => self.out.push_back(Outgoing { port, to, datagram }),
            // Never for a request that decoded: every answer is fitted.
            Err(_) => self.counters.dropped_answers += 1,
        }
    }

    fn register(&mut self, now: Duration, from: SocketAddr, register: Register, len: usize) {
        let Some(quiet) = self.limit(now, from, Limit::Register) else {
            return;
        };
        if !self.key.check(from, register.nonce, register.cookie, now) {
            self.counters.challenges += 1;
            let challenge = MasterPacket::Challenge(Challenge {
                nonce: register.nonce,
                cookie: self.key.cookie(from, register.nonce, now),
            });
            self.answer(now, from, &challenge, len, quiet);
            return;
        }
        let expiry = self.settings.expiry;
        if let Some(existing) = self.listings.at_address(from) {
            let id = existing.id;
            if existing.nonce == register.nonce {
                // The same game again: the same Listed, and the newest facts.
                let mapping = self.mapping_of(now, from, &register.candidates);
                let listing = self
                    .listings
                    .update(id, expiry, |listing| {
                        listing.heard = now;
                        listing.build = register.build;
                        listing.dedicated = register.dedicated;
                        listing.platform = register.platform;
                        listing.candidates = register.candidates;
                        listing.summary = register.summary;
                        listing.mapping = mapping;
                        listing.clone()
                    })
                    .expect("the listing at the address exists");
                let listed = self.listed(&listing, register.nonce, from);
                self.answer(now, from, &listed, len, true);
                return;
            }
            if let Some(old) = self.listings.remove(id, expiry) {
                self.unlisted(&old, Gone::Replaced);
            }
        }
        let source = SourceKey::of(from);
        if self.listings.of_source(source) >= self.settings.listings_per_source
            || self.listings.len() >= self.settings.max_listings
        {
            self.counters.refused_room += 1;
            return;
        }
        let id = self.fresh_number();
        let token = self.fresh_number();
        let mapping = self.mapping_of(now, from, &register.candidates);
        let listing = Listing {
            id,
            token,
            nonce: register.nonce,
            address: from,
            source,
            build: register.build,
            dedicated: register.dedicated,
            platform: register.platform,
            candidates: register.candidates,
            summary: register.summary,
            change: 0,
            mapping,
            heard: now,
            moved: None,
            beats: Bucket::default(),
        };
        let listed = self.listed(&listing, register.nonce, from);
        self.counters.listed += 1;
        self.log.push_back(format!(
            "listed id={id:016x} from={from} name={:?} listings={}",
            listing.summary.name,
            self.listings.len() + 1
        ));
        self.listings.insert(listing, expiry);
        self.answer(now, from, &listed, len, true);
    }

    fn listed(&self, listing: &Listing, nonce: u64, seen: SocketAddr) -> MasterPacket {
        let secs = |d: Duration| d.as_secs().min(255) as u8;
        MasterPacket::Listed(Listed {
            nonce,
            listing_id: listing.id,
            token: listing.token,
            seen,
            heartbeat_secs: secs(self.settings.heartbeat),
            keep_secs: secs(self.settings.keep),
            expiry_secs: secs(self.settings.expiry),
        })
    }

    /// A number no listing uses as its id or token, never 0.
    fn fresh_number(&mut self) -> u64 {
        loop {
            let n = self.ids.next();
            if !self.listings.number_taken(n) {
                return n;
            }
        }
    }

    fn mapping_of(
        &mut self,
        now: Duration,
        from: SocketAddr,
        candidates: &[tore_net::master::Candidate],
    ) -> tore_net::master::MappingType {
        let own = candidates
            .iter()
            .filter(|c| matches!(c.kind, CandidateKind::Local | CandidateKind::GlobalIpv6))
            .map(|c| canonical(c.address));
        self.tests.mapping_of(now, from, own)
    }

    fn unlisted(&mut self, listing: &Listing, why: Gone) {
        self.counters.unlisted += 1;
        self.log.push_back(format!(
            "unlisted id={:016x} from={} name={:?} reason={} listings={}",
            listing.id,
            listing.address,
            listing.summary.name,
            why.text(),
            self.listings.len()
        ));
    }

    /// A Heartbeat or Keep with a known token from `from`: the per-listing
    /// rate, and a move to a new address at most once a minute, which takes
    /// the listing's relay channels with it (stage K, slice K8). Returns the
    /// listing's id when it may go on.
    fn beat(&mut self, now: Duration, from: SocketAddr, token: u64, len: usize) -> Option<u64> {
        let Some(id) = self.listings.id_of_token(token) else {
            self.counters.unknown_tokens += 1;
            if let Some(quiet) = self.limit(now, from, Limit::Query) {
                let unknown = MasterPacket::UnknownListing(UnknownListing { token });
                self.answer(now, from, &unknown, len, quiet);
            }
            return None;
        };
        let expiry = self.settings.expiry;
        let listing = self.listings.get(id)?;
        let moving = listing.address != from;
        if moving
            && listing
                .moved
                .is_some_and(|at| now.saturating_sub(at) < MOVE_INTERVAL)
        {
            self.counters.dropped_limit += 1;
            return None;
        }
        let allowed = self
            .listings
            .update(id, expiry, |listing| listing.beats.take(now, BEAT_RATE))?;
        if !allowed {
            self.counters.dropped_limit += 1;
            return None;
        }
        if moving {
            if let Some(other) = self.listings.at_address(from).map(|l| l.id)
                && let Some(old) = self.listings.remove(other, expiry)
            {
                self.unlisted(&old, Gone::Displaced);
            }
            let old_address = self.listings.update(id, expiry, |listing| {
                let old = listing.address;
                listing.address = from;
                listing.source = SourceKey::of(from);
                listing.moved = Some(now);
                old
            })?;
            self.log
                .push_back(format!("moved id={id:016x} from={old_address} to={from}"));
            // Stage K (slice K8): the listing's relay channels follow it, so
            // a migrated game's relayed players reach the new host.
            self.relays.move_host(id, from);
        }
        self.listings
            .update(id, expiry, |listing| listing.heard = now);
        Some(id)
    }

    fn heartbeat(&mut self, now: Duration, from: SocketAddr, beat: Heartbeat, len: usize) {
        let Some(id) = self.beat(now, from, beat.token, len) else {
            return;
        };
        self.counters.heartbeats += 1;
        let mapping = self.mapping_of(now, from, &beat.candidates);
        let expiry = self.settings.expiry;
        self.listings.update(id, expiry, |listing| {
            listing.change = beat.change;
            listing.candidates = beat.candidates;
            listing.summary = beat.summary;
            listing.mapping = mapping;
        });
        let ack = MasterPacket::HeartbeatAck(HeartbeatAck {
            listing_id: id,
            seen: from,
        });
        self.answer(now, from, &ack, len, true);
    }

    fn keep(&mut self, now: Duration, from: SocketAddr, token: u64, len: usize) {
        if self.beat(now, from, token, len).is_some() {
            self.counters.keeps += 1;
        }
    }

    /// Unregister: the listing goes at once; never answered.
    fn unregister(&mut self, token: u64) {
        let Some(id) = self.listings.id_of_token(token) else {
            self.counters.unknown_tokens += 1;
            return;
        };
        if let Some(listing) = self.listings.remove(id, self.settings.expiry) {
            self.unlisted(&listing, Gone::Unregistered);
        }
    }

    fn browse(
        &mut self,
        now: Duration,
        from: SocketAddr,
        request: tore_net::master::Browse,
        len: usize,
    ) {
        let Some(quiet) = self.limit(now, from, Limit::Query) else {
            return;
        };
        self.counters.browses += 1;
        let page = MasterPacket::Page(browse::page(&self.listings, &request, len));
        self.answer(now, from, &page, len, quiet);
    }

    fn details(&mut self, now: Duration, from: SocketAddr, request: Details, len: usize) {
        let Some(quiet) = self.limit(now, from, Limit::Query) else {
            return;
        };
        self.counters.details += 1;
        let details = ListingDetails {
            nonce: request.nonce,
            listing_id: request.listing_id,
            summary: self
                .listings
                .get(request.listing_id)
                .map(|l| l.summary.clone()),
        }
        .fit(len);
        self.answer(
            now,
            from,
            &MasterPacket::ListingDetails(details),
            len,
            quiet,
        );
    }

    fn probe(
        &mut self,
        now: Duration,
        port: MasterPort,
        from: SocketAddr,
        probe: Probe,
        len: usize,
    ) {
        let Some(quiet) = self.limit(now, from, Limit::Probe) else {
            return;
        };
        self.counters.probes += 1;
        let which = match port {
            MasterPort::Main => ProbePort::Main,
            MasterPort::Probe => ProbePort::Second,
        };
        self.tests.probed(now, which, from, probe.nonce);
        let answer = MasterPacket::ProbeAnswer(ProbeAnswer {
            nonce: probe.nonce,
            port: which,
            seen: from,
        });
        let version = tore_net::master::MASTER_VERSION;
        self.answer_in(now, port, from, version, &answer, len, quiet);
    }

    /// Introduce: the source's limit, the cookie (a Challenge of 23 bytes
    /// for an unproven sender, never more), then the introduction and its
    /// Meet.
    fn introduce(&mut self, now: Duration, from: SocketAddr, request: Introduce, len: usize) {
        let Some(quiet) = self.limit(now, from, Limit::Introduce) else {
            return;
        };
        if !self.key.check(from, request.nonce, request.cookie, now) {
            self.counters.challenges += 1;
            let challenge = MasterPacket::Challenge(Challenge {
                nonce: request.nonce,
                cookie: self.key.cookie(from, request.nonce, now),
            });
            self.answer(now, from, &challenge, len, quiet);
            return;
        }
        let introduction = self
            .introductions
            .introduce(now, from, &request, &self.listings);
        if let Some(introduction) = introduction {
            let packet = MasterPacket::Introduction(introduction);
            self.answer(now, from, &packet, len, true);
        }
        self.send_meets();
    }

    /// A Relay request: the source's limit (2 a minute), then the
    /// introduction it names, then the relay's own rules.
    fn relay_request(&mut self, now: Duration, from: SocketAddr, request: RelayRequest) {
        if self.limit(now, from, Limit::RelayRequest).is_none() {
            return;
        }
        // The host end is the listing's address now: a listing that moved
        // since the introduction (stage K, slice K8) is relayed to its new
        // host.
        let ends = self
            .introductions
            .ends(request.introduction_id)
            .map(|mut ends| {
                if let Some(listing) = self.listings.get(ends.listing_id) {
                    ends.host = listing.address;
                }
                ends
            });
        self.relays.request(now, from, &request, ends);
        self.send_relays();
    }

    fn report(&mut self, now: Duration, from: SocketAddr, report: Report) {
        if self.limit(now, from, Limit::Report).is_none() {
            return;
        }
        if self.settings.telemetry && self.telemetry.record(&report) {
            self.counters.reports += 1;
        } else {
            self.counters.reports_ignored += 1;
        }
    }
}
