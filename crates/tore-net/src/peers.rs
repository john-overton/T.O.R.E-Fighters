//! The peers router in front of a joined game's socket (stage K, slice K6;
//! docs/ARCHITECTURE.md, "Reaching the new host"), with its reach answerer
//! and its reach tests.
//!
//! The socket a game joined with is its future host socket, so other games
//! test whether they reach it. Everything that arrives on it goes through
//! [`Peers::route`] first:
//!
//! - a **Reach** for this game's session is answered here, at most 10 a
//!   second from one address and port (one player's socket), 160 a second
//!   from one IP address (a LAN party of 8 behind one router, each player
//!   testing every address of up to three candidates) and 200 a second in
//!   all, never with more bytes than it brought; while the game hosts, the
//!   host's transport answers it instead ([`Route::Host`]);
//! - a **Reach answer** to one of this game's own Reaches is taken here and
//!   timed;
//! - while the game is a player, what only a host receives (Connect
//!   requests, Challenge answers, Discover queries, Keepalives) is dropped,
//!   and everything else goes to the client ([`Route::Client`]);
//! - once the game hosts ([`Peers::set_hosting`]), everything goes to its
//!   host's transport, except datagrams from the old host's address, which
//!   still go to the client.
//!
//! The router also sends Reaches: [`Peers::open`] sends five 200 ms apart to
//! each of a list of players' addresses, which opens this game's router to
//! them (a candidate's half of a reach test); [`Peers::test`] sends five 200
//! ms apart to each address of up to three candidates and, a second after
//! the last, reports which address answered and the median round trip (a
//! player's half). An address a candidate's Reach came from is tried too, as
//! a race tries the port a host's punch came from.
//!
//! Like the transport, it is a state machine driven by the caller: feed
//! datagrams to [`Peers::route`], call [`Peers::update`] often, send what
//! [`Peers::poll_transmit`] gives (or [`Peers::transmit`]) and take
//! [`Peers::poll_finished`].

use crate::datagram::{Datagrams, Transmit};
use crate::entropy::{Entropy, Rng};
use crate::packet::{self, Packet, PacketKind, Reach, ReachAnswer, ReachRole};
use crate::{RATE_LIMIT_TOTAL, REACH_PER_ADDRESS};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

/// Reaches sent to each address of a test or an opening.
pub const REACH_TRIES: u32 = 5;
/// The time between two Reaches to one address.
pub const REACH_GAP: Duration = Duration::from_millis(200);
/// How long a test waits for answers after its last Reach (agent decision:
/// a round trip of up to a second still counts).
pub const REACH_WAIT: Duration = Duration::from_secs(1);
/// Reaches answered a second from one IP address, whatever their ports: a
/// LAN party of 8 behind one router sends up to 7 players' five Reaches to
/// each of four candidate addresses, 140 a second, and each player's own
/// socket is held to [`REACH_PER_ADDRESS`] besides (slice KP, agent
/// decision; the transport's own answerer keeps one limit for the IP).
pub const REACH_PER_IP: u32 = 160;
/// Candidates in one test at most.
pub const MAX_TARGETS: usize = 3;
/// Addresses of one candidate a test tries at most, the learned ones
/// included: the Reach report's index is 4 bits.
pub const MAX_TEST_ADDRESSES: usize = 16;
/// Addresses one opening sends to at most: 64 players of 8 addresses.
pub const MAX_OPEN_ADDRESSES: usize = 512;
/// Tests and openings running at once at most; a new one beyond replaces
/// the oldest (agent decision: the host runs one test every 10 seconds).
pub const MAX_RUNNING: usize = 4;

/// Where a datagram goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// To the game's client.
    Client,
    /// To the game's host transport (it hosts now).
    Host,
    /// Taken by the router: a Reach answered or refused, a Reach answer
    /// timed, or a datagram dropped.
    Taken,
}

/// What the router counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PeersCounters {
    /// Reaches answered.
    pub answered: u64,
    /// Reaches for another session, or while the game is in none.
    pub foreign: u64,
    /// Reaches over the rate limits.
    pub rate_limited: u64,
    /// Reach answers that match none of this game's Reaches.
    pub unexpected_answers: u64,
    /// Reaches and Reach answers that do not decode.
    pub malformed: u64,
    /// Datagrams only a host takes, dropped while the game is a player.
    pub dropped: u64,
    /// Reaches sent.
    pub sent: u64,
}

/// One candidate's result in a finished test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetResult {
    /// The candidate's lobby id.
    pub player: u8,
    /// The answering address (its index in the test's list, the learned
    /// addresses after the given ones) and the median round trip of its
    /// answers; `None` when no address answered.
    pub reached: Option<(u8, Duration)>,
}

/// A finished reach test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    /// The caller's key for the test (the host's test id).
    pub key: u16,
    /// One per candidate, in the test's order.
    pub results: Vec<TargetResult>,
}

/// A candidate to test: its lobby id and its addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestTarget {
    pub player: u8,
    pub addresses: Vec<SocketAddr>,
}

#[derive(Debug)]
struct Target {
    player: u8,
    addresses: Vec<SocketAddr>,
    /// The round trips of each address's answers.
    answers: Vec<Vec<Duration>>,
}

#[derive(Debug)]
struct Test {
    key: u16,
    targets: Vec<Target>,
    /// When the test reports: a second after its last Reach.
    ends: Duration,
}

/// A Reach due to go out.
#[derive(Debug, Clone, Copy)]
struct Due {
    at: Duration,
    to: SocketAddr,
    /// The test and the target and address it serves; `None` for an
    /// opening.
    probe: Option<(u16, usize, usize)>,
}

/// Where an outstanding nonce came from.
#[derive(Debug, Clone, Copy)]
struct Sent {
    at: Duration,
    probe: Option<(u16, usize, usize)>,
}

/// The Reach answerer's rate limits: each address and port, each IP address,
/// and all of them together, a second at a time.
#[derive(Debug, Default)]
struct Limiter {
    second: u64,
    total: u32,
    per_address: HashMap<SocketAddr, u32>,
    per_ip: HashMap<IpAddr, u32>,
}

impl Limiter {
    fn allow(&mut self, now: Duration, from: SocketAddr) -> bool {
        let second = now.as_secs();
        if second != self.second {
            self.second = second;
            self.total = 0;
            self.per_address.clear();
            self.per_ip.clear();
        }
        if self.total >= RATE_LIMIT_TOTAL {
            return false;
        }
        if self
            .per_address
            .get(&from)
            .is_some_and(|&n| n >= REACH_PER_ADDRESS)
            || self
                .per_ip
                .get(&from.ip())
                .is_some_and(|&n| n >= REACH_PER_IP)
        {
            return false;
        }
        *self.per_address.entry(from).or_insert(0) += 1;
        *self.per_ip.entry(from.ip()).or_insert(0) += 1;
        self.total += 1;
        true
    }
}

/// The peers router. See the module documentation.
#[derive(Debug)]
pub struct Peers {
    version: u16,
    rng: Rng,
    session: Option<u64>,
    me: u8,
    hosting: bool,
    old_host: Option<SocketAddr>,
    limiter: Limiter,
    due: Vec<Due>,
    sent: HashMap<u64, Sent>,
    tests: Vec<Test>,
    /// Openings running: their key and when their last Reach goes.
    openings: Vec<(u16, Duration)>,
    finished: VecDeque<Finished>,
    out: VecDeque<Transmit>,
    counters: PeersCounters,
}

impl Peers {
    /// A router for a game of `protocol_version`, its nonces from
    /// `entropy`; in no session until [`Peers::set_session`].
    pub fn new(protocol_version: u16, entropy: Entropy) -> Self {
        Self {
            version: protocol_version,
            rng: Rng::new(entropy),
            session: None,
            me: 0,
            hosting: false,
            old_host: None,
            limiter: Limiter::default(),
            due: Vec::new(),
            sent: HashMap::new(),
            tests: Vec::new(),
            openings: Vec::new(),
            finished: VecDeque::new(),
            out: VecDeque::new(),
            counters: PeersCounters::default(),
        }
    }

    /// The session this game is in and its lobby id there: Reaches for it
    /// are answered, and this game's own carry the id. `None` answers none.
    pub fn set_session(&mut self, session: Option<u64>, lobby_id: u8) {
        self.session = session;
        self.me = lobby_id;
    }

    /// The session, when in one.
    pub fn session(&self) -> Option<u64> {
        self.session
    }

    /// The game hosts its session now: datagrams go to its host's transport,
    /// but those from `old_host` (the game's client's old host) still go to
    /// the client.
    pub fn set_hosting(&mut self, hosting: bool, old_host: Option<SocketAddr>) {
        self.hosting = hosting;
        self.old_host = old_host;
    }

    /// Whether the game hosts.
    pub fn hosting(&self) -> bool {
        self.hosting
    }

    /// What the router counted.
    pub fn counters(&self) -> &PeersCounters {
        &self.counters
    }

    /// Takes one datagram that arrived at `now` from `from`, and says where
    /// it goes.
    pub fn route(&mut self, now: Duration, from: SocketAddr, datagram: &[u8]) -> Route {
        let onward = if self.hosting && Some(from) != self.old_host {
            Route::Host
        } else {
            Route::Client
        };
        let Ok((kind, body)) = packet::open(datagram, self.version) else {
            return onward;
        };
        match kind {
            PacketKind::Reach if onward == Route::Host => Route::Host,
            PacketKind::Reach => {
                self.on_reach(now, from, body);
                Route::Taken
            }
            PacketKind::ReachAnswer => {
                self.on_answer(now, body);
                Route::Taken
            }
            PacketKind::ConnectRequest
            | PacketKind::ChallengeAnswer
            | PacketKind::Discover
            | PacketKind::Keepalive
                if onward == Route::Client =>
            {
                self.counters.dropped += 1;
                Route::Taken
            }
            _ => onward,
        }
    }

    fn on_reach(&mut self, now: Duration, from: SocketAddr, body: &[u8]) {
        let Ok(reach) = packet::decode_reach(body) else {
            self.counters.malformed += 1;
            return;
        };
        if self.session != Some(reach.session_id) {
            self.counters.foreign += 1;
            return;
        }
        // A candidate's Reach teaches a test running against it the
        // address its router gave it towards this game.
        self.learn(now, reach.from, from);
        if !self.limiter.allow(now, from) {
            self.counters.rate_limited += 1;
            return;
        }
        self.counters.answered += 1;
        self.send(
            from,
            &Packet::ReachAnswer(ReachAnswer {
                nonce: reach.nonce,
                session_id: reach.session_id,
                role: ReachRole::NotHosting,
            }),
        );
    }

    fn on_answer(&mut self, now: Duration, body: &[u8]) {
        let Ok(answer) = packet::decode_reach_answer(body) else {
            self.counters.malformed += 1;
            return;
        };
        let Some(sent) = self
            .sent
            .remove(&answer.nonce)
            .filter(|_| self.session == Some(answer.session_id))
        else {
            self.counters.unexpected_answers += 1;
            return;
        };
        let Some((key, target, address)) = sent.probe else {
            return;
        };
        if let Some(test) = self.tests.iter_mut().find(|t| t.key == key)
            && let Some(answers) = test
                .targets
                .get_mut(target)
                .and_then(|t| t.answers.get_mut(address))
        {
            answers.push(now.saturating_sub(sent.at));
        }
    }

    /// Adds `address` to the running tests' candidate `player`, when it is
    /// new and there is room, and starts trying it.
    fn learn(&mut self, now: Duration, player: u8, address: SocketAddr) {
        let mut added = Vec::new();
        for test in &mut self.tests {
            for (index, target) in test.targets.iter_mut().enumerate() {
                if target.player != player
                    || target.addresses.contains(&address)
                    || target.addresses.len() >= MAX_TEST_ADDRESSES
                {
                    continue;
                }
                target.addresses.push(address);
                target.answers.push(Vec::new());
                added.push((test.key, index, target.addresses.len() - 1));
                let last = now + REACH_GAP * (REACH_TRIES - 1);
                test.ends = test.ends.max(last + REACH_WAIT);
            }
        }
        for (key, target, index) in added {
            self.schedule(now, address, Some((key, target, index)));
        }
    }

    fn schedule(&mut self, now: Duration, to: SocketAddr, probe: Option<(u16, usize, usize)>) {
        for n in 0..REACH_TRIES {
            self.due.push(Due {
                at: now + REACH_GAP * n,
                to,
                probe,
            });
        }
    }

    /// Opens this game's router to `addresses` (the players of a Reach
    /// peers, under `key`): five Reaches to each, 200 ms apart. Their
    /// answers are taken and not reported. At most [`MAX_OPEN_ADDRESSES`];
    /// the rest are ignored.
    pub fn open(&mut self, now: Duration, key: u16, addresses: &[SocketAddr]) {
        let mut seen = Vec::new();
        for &address in addresses.iter().take(MAX_OPEN_ADDRESSES) {
            if !seen.contains(&address) {
                seen.push(address);
                self.schedule(now, address, None);
            }
        }
        self.openings.retain(|(k, _)| *k != key);
        self.openings
            .push((key, now + REACH_GAP * (REACH_TRIES - 1)));
        self.trim();
    }

    /// Tests `targets` (a Reach test's candidates, under `key`): five
    /// Reaches to each address, 200 ms apart; a second after the last, a
    /// [`Finished`] with each candidate's answering address and median round
    /// trip. At most [`MAX_TARGETS`] candidates of
    /// [`MAX_TEST_ADDRESSES`] addresses; the rest are ignored. A test under a
    /// key already running replaces it.
    pub fn test(&mut self, now: Duration, key: u16, targets: &[TestTarget]) {
        self.drop_test(key);
        let targets: Vec<Target> = targets
            .iter()
            .take(MAX_TARGETS)
            .map(|t| {
                let mut addresses: Vec<SocketAddr> = Vec::new();
                for &address in t.addresses.iter().take(MAX_TEST_ADDRESSES) {
                    if !addresses.contains(&address) {
                        addresses.push(address);
                    }
                }
                Target {
                    player: t.player,
                    answers: vec![Vec::new(); addresses.len()],
                    addresses,
                }
            })
            .collect();
        for (index, target) in targets.iter().enumerate() {
            for (n, &address) in target.addresses.iter().enumerate() {
                self.schedule(now, address, Some((key, index, n)));
            }
        }
        self.tests.push(Test {
            key,
            targets,
            ends: now + REACH_GAP * (REACH_TRIES - 1) + REACH_WAIT,
        });
        self.trim();
    }

    /// Stops the test under `key`, if one runs, reporting nothing.
    fn drop_test(&mut self, key: u16) {
        self.tests.retain(|t| t.key != key);
        self.due
            .retain(|d| d.probe.is_none_or(|(k, _, _)| k != key));
        self.sent
            .retain(|_, s| s.probe.is_none_or(|(k, _, _)| k != key));
    }

    /// Keeps at most [`MAX_RUNNING`] tests and openings, the newest.
    fn trim(&mut self) {
        while self.tests.len() + self.openings.len() > MAX_RUNNING {
            let oldest_test = self.tests.first().map(|t| t.ends);
            let oldest_opening = self.openings.first().map(|o| o.1);
            match (oldest_test, oldest_opening) {
                (Some(t), Some(o)) if o <= t => self.drop_opening(),
                (None, Some(_)) => self.drop_opening(),
                _ => {
                    let key = self.tests[0].key;
                    self.drop_test(key);
                }
            }
        }
    }

    fn drop_opening(&mut self) {
        self.openings.remove(0);
        // Its Reaches still due go: they only open the router.
    }

    /// Sends the Reaches due at `now` and reports the tests that ended.
    pub fn update(&mut self, now: Duration) {
        let mut due = Vec::new();
        self.due.retain(|d| {
            if d.at <= now {
                due.push(*d);
                false
            } else {
                true
            }
        });
        if let Some(session) = self.session {
            for d in due {
                let nonce = self.rng.next_u64();
                self.sent.insert(
                    nonce,
                    Sent {
                        at: now,
                        probe: d.probe,
                    },
                );
                self.counters.sent += 1;
                self.send(
                    d.to,
                    &Packet::Reach(Reach {
                        session_id: session,
                        nonce,
                        from: self.me,
                    }),
                );
            }
        }
        self.openings.retain(|(_, last)| *last > now);
        let (ended, running): (Vec<Test>, Vec<Test>) = std::mem::take(&mut self.tests)
            .into_iter()
            .partition(|t| t.ends <= now);
        self.tests = running;
        for test in ended {
            self.sent
                .retain(|_, s| s.probe.is_none_or(|(k, _, _)| k != test.key));
            self.finished.push_back(Finished {
                key: test.key,
                results: test.targets.iter().map(result).collect(),
            });
        }
        // Reaches unanswered well after the wait are forgotten.
        let keep = REACH_GAP * REACH_TRIES + REACH_WAIT;
        self.sent.retain(|_, s| now.saturating_sub(s.at) <= keep);
    }

    /// The next finished test, oldest first.
    pub fn poll_finished(&mut self) -> Option<Finished> {
        self.finished.pop_front()
    }

    /// The next datagram to send.
    pub fn poll_transmit(&mut self) -> Option<Transmit> {
        self.out.pop_front()
    }

    /// Sends every queued datagram on `socket`.
    pub fn transmit<D: Datagrams + ?Sized>(&mut self, socket: &mut D) -> io::Result<()> {
        while let Some(t) = self.out.pop_front() {
            socket.send_datagram(t.to, &t.datagram)?;
        }
        Ok(())
    }

    fn send(&mut self, to: SocketAddr, packet: &Packet) {
        if let Ok(datagram) = packet.encode(self.version) {
            self.out.push_back(Transmit { to, datagram });
        }
    }
}

/// A finished test's result for one candidate: the address with the most
/// answers (the lowest median round trip between equals, then the first),
/// with its median.
fn result(target: &Target) -> TargetResult {
    let best = target
        .answers
        .iter()
        .enumerate()
        .filter(|(_, answers)| !answers.is_empty())
        .map(|(index, answers)| (index, answers.len(), median(answers)))
        .min_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0)));
    TargetResult {
        player: target.player,
        reached: best.map(|(index, _, median)| (index as u8, median)),
    }
}

/// The median of `times` (the lower of the middle two); zero for none.
pub fn median(times: &[Duration]) -> Duration {
    let mut sorted = times.to_vec();
    sorted.sort();
    sorted
        .get(sorted.len().saturating_sub(1) / 2)
        .copied()
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "peers_tests.rs"]
mod tests;
