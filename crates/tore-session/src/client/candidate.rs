//! The client's side of host selection (stage K, slice K6; docs/ARCHITECTURE.md,
//! "Host selection"): the Candidate report and the CPU measure, the reach
//! tests and their report, the upload test.
//!
//! - **The report** (message 40) goes to the host once the lobby arrives and
//!   again whenever it changes: the player's "Let my game take over hosting"
//!   switch, this build's class, the joined socket's candidates and the
//!   router's mapping type as the game knows them
//!   ([`Client::set_candidate`]), and the CPU measure.
//! - **The CPU measure**, when the game asks for it
//!   ([`CandidateSettings::measure_cpu`]): on a thread of its own the lobby's
//!   mission is built once more and that throwaway copy, all AI, steps 240
//!   ticks (2 seconds of mission); the mean cost of a tick goes in the next
//!   report, with the mission's number. Again whenever the lobby's mission
//!   changes.
//! - **Reach tests.** A Reach test or Reach peers from the host is work for
//!   the game's peers router ([`tore_net::peers::Peers`]), which sits in front
//!   of the joined socket: [`Client::drive_peers`] hands it over and sends
//!   the Reach report when the router's test ends.
//! - **The upload test.** An Upload test asks for a paced burst of Filler
//!   sections at a rate for a length: each update sends what is due since
//!   the burst began, in whole packets of [`FILLER_PACKET`] bytes of filler
//!   (the last may be shorter), so the host can measure how much of it
//!   arrives.

use super::{Client, ClientPhase};
use crate::wire::SECTION_FILLER;
use crate::wire::messages::Message;
use crate::wire::migration::{
    CandidateReport, Processor, ReachReport, ReachResult, Reached, UploadTest, limits,
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};
use tore_net::master::candidate::{Candidate, CandidateKind, MappingType};
use tore_net::peers::{Peers, TestTarget};
use tore_world::mission::MissionSpec;
use tore_world::resources::ResourceReads;
use tore_world::world::{Seating, TickOutput, World};

/// Ticks the CPU measure steps: 2 seconds of mission.
pub const CPU_MEASURE_TICKS: u64 = 240;
/// Filler bytes in one upload test packet at most: with the Payload's
/// header, its section's and room for due messages, under 1,200 bytes.
pub const FILLER_PACKET: usize = 1_100;
/// Reach work kept for a router at most; older work is dropped when no
/// router takes it.
const MAX_WORK: usize = 8;

/// What the game tells the host about itself as a host. Without a call to
/// [`Client::set_candidate`] the client reports the defaults: it may host,
/// no candidates, its router's mapping unknown, and no CPU measure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateSettings {
    /// The player's "Let my game take over hosting" switch (on by default).
    pub may_host: bool,
    /// The joined socket's Local, Mapped and Global IPv6 candidates (at
    /// most three; a Seen candidate is the host's to add, and is dropped).
    pub candidates: Vec<Candidate>,
    /// The router's mapping type from the master's test.
    pub mapping: MappingType,
    /// Measure the CPU on a thread of its own. Off by default, so a test
    /// rig or a crowd of bots does not step a mission each.
    pub measure_cpu: bool,
}

impl Default for CandidateSettings {
    fn default() -> Self {
        Self {
            may_host: true,
            candidates: Vec::new(),
            mapping: MappingType::Unknown,
            measure_cpu: false,
        }
    }
}

/// A burst of Filler under way.
#[derive(Debug)]
struct Burst {
    started: Duration,
    rate: u64,
    length: Duration,
    sent: u64,
}

/// Work for the peers router, waiting for [`Client::drive_peers`].
#[derive(Debug)]
enum Work {
    Test {
        test: u16,
        targets: Vec<TestTarget>,
    },
    Open {
        test: u16,
        addresses: Vec<std::net::SocketAddr>,
    },
}

/// The client's host-selection state.
#[derive(Debug, Default)]
pub(super) struct Candidacy {
    settings: CandidateSettings,
    /// The last report sent.
    sent: Option<CandidateReport>,
    /// The newest CPU measure: the mission's number and microseconds a tick.
    cpu: Option<(u32, u32)>,
    /// A measure running for a mission's number.
    measuring: Option<(u32, Receiver<u32>)>,
    work: VecDeque<Work>,
    /// Tests handed to the router, by their ids.
    running: BTreeMap<u16, ()>,
    burst: Option<Burst>,
}

impl Client {
    /// What the game tells the host about itself as a host (slice K6): the
    /// switch, the joined socket's candidates, the router's mapping type,
    /// whether to measure the CPU. A changed report goes out at the next
    /// update.
    pub fn set_candidate(&mut self, mut settings: CandidateSettings) {
        settings
            .candidates
            .retain(|c| c.kind != CandidateKind::Seen);
        settings.candidates.truncate(limits::OWN_CANDIDATES);
        self.candidacy.settings = settings;
    }

    /// The Candidate report as it stands (slice K6).
    pub fn candidate_report(&self) -> CandidateReport {
        let settings = &self.candidacy.settings;
        let (cpu_mission, cpu_micros) = self.candidacy.cpu.unwrap_or((0, 0));
        CandidateReport {
            may_host: settings.may_host,
            platform: self.config.platform,
            processor: Processor::current(),
            candidates: settings.candidates.clone(),
            mapping: settings.mapping,
            cpu_micros,
            cpu_mission,
        }
    }

    /// A Reach test, Reach peers or Upload test from the host.
    pub(super) fn candidate_message(&mut self, message: Message) {
        match message {
            Message::ReachTest(test) => {
                let targets = test
                    .candidates
                    .iter()
                    .map(|c| TestTarget {
                        player: c.player,
                        addresses: c.addresses.clone(),
                    })
                    .collect();
                self.candidacy.work.push_back(Work::Test {
                    test: test.test,
                    targets,
                });
            }
            Message::ReachPeers(peers) => {
                let addresses = peers
                    .players
                    .iter()
                    .flat_map(|p| p.addresses.iter().copied())
                    .collect();
                self.candidacy.work.push_back(Work::Open {
                    test: peers.test,
                    addresses,
                });
            }
            Message::UploadTest(UploadTest {
                rate, length_ms, ..
            }) => {
                self.candidacy.burst = Some(Burst {
                    started: self.now,
                    rate: u64::from(rate),
                    length: Duration::from_millis(u64::from(length_ms.min(limits::UPLOAD_MS))),
                    sent: 0,
                });
            }
            _ => {}
        }
        while self.candidacy.work.len() > MAX_WORK {
            self.candidacy.work.pop_front();
        }
    }

    /// The report when it changed, the CPU measure, and the upload burst's
    /// due packets (called from [`Client::update`]).
    pub(super) fn candidate_update(&mut self, now: Duration) {
        if matches!(self.phase, ClientPhase::Connecting | ClientPhase::Closed) {
            return;
        }
        self.cpu_measure();
        if self.lobby.is_some() {
            let report = self.candidate_report();
            if self.candidacy.sent.as_ref() != Some(&report) {
                self.send(&Message::Candidate(Box::new(report.clone())));
                self.candidacy.sent = Some(report);
            }
        }
        self.burst(now);
    }

    /// Starts the CPU measure for the lobby's mission when it is new, and
    /// takes a finished one.
    fn cpu_measure(&mut self) {
        if let Some((mission, receiver)) = &self.candidacy.measuring {
            match receiver.try_recv() {
                Ok(micros) => {
                    self.candidacy.cpu = Some((*mission, micros.max(1)));
                    self.candidacy.measuring = None;
                }
                Err(TryRecvError::Disconnected) => self.candidacy.measuring = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        if !self.candidacy.settings.measure_cpu
            || self.phase != ClientPhase::Lobby
            || self.candidacy.measuring.is_some()
        {
            return;
        }
        let (Some(number), Some(spec)) = (self.number, &self.spec) else {
            return;
        };
        if self.candidacy.cpu.is_some_and(|(n, _)| n == number) {
            return;
        }
        let spec = spec.clone();
        let resources = Arc::clone(&self.resources);
        let (sender, receiver) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("tore-cpu-measure".into())
            .spawn(move || {
                if let Some(micros) = measure_cpu(&spec, &resources, CPU_MEASURE_TICKS) {
                    let _ = sender.send(micros);
                }
            });
        match spawned {
            Ok(_) => self.candidacy.measuring = Some((number, receiver)),
            // No thread: no measure for this mission, rather than trying at
            // every update.
            Err(_) => self.candidacy.cpu = Some((number, 0)),
        }
    }

    /// Sends the burst's packets due at `now`.
    fn burst(&mut self, now: Duration) {
        let Some(burst) = &mut self.candidacy.burst else {
            return;
        };
        let elapsed = now.saturating_sub(burst.started).min(burst.length);
        let total = burst.rate * burst.length.as_millis() as u64 / 1_000;
        let due = (burst.rate * elapsed.as_micros() as u64 / 1_000_000).min(total);
        let zeros = [0u8; FILLER_PACKET];
        // Whole packets as they fall due, and what is left at the end.
        let ending = elapsed >= burst.length;
        while burst.sent < due && (ending || due - burst.sent >= FILLER_PACKET as u64) {
            let size = (due - burst.sent).min(FILLER_PACKET as u64) as usize;
            if self
                .net
                .send_payload(now, &[(SECTION_FILLER, &zeros[..size])])
                .is_err()
            {
                break;
            }
            burst.sent += size as u64;
        }
        if burst.sent >= total || ending {
            self.candidacy.burst = None;
        }
    }

    /// Hands the host's reach work to the game's peers router in front of
    /// the joined socket, and sends the Reach report of each test it
    /// finished (slice K6). The game calls it after each update; until the
    /// game hosts, the router answers Reaches for this session as this
    /// player.
    pub fn drive_peers(&mut self, now: Duration, peers: &mut Peers) {
        if !peers.hosting() {
            let session = self.net.welcome().map(|w| w.session_id);
            let me = self.lobby.as_ref().map_or(0, |l| l.you);
            peers.set_session(session.filter(|_| self.lobby.is_some()), me);
        }
        while let Some(work) = self.candidacy.work.pop_front() {
            match work {
                Work::Test { test, targets } => {
                    peers.test(now, test, &targets);
                    self.candidacy.running.insert(test, ());
                }
                Work::Open { test, addresses } => peers.open(now, test, &addresses),
            }
        }
        while let Some(finished) = peers.poll_finished() {
            if self.candidacy.running.remove(&finished.key).is_none() {
                continue;
            }
            let results = finished
                .results
                .iter()
                .map(|r| ReachResult {
                    player: r.player,
                    reached: r.reached.map(|(address, round_trip)| Reached {
                        address: address.min(15),
                        round_trip_ms: round_trip.as_millis().min(u128::from(u16::MAX)) as u16,
                    }),
                })
                .collect();
            if !matches!(self.phase, ClientPhase::Connecting | ClientPhase::Closed) {
                self.send(&Message::ReachReport(Box::new(ReachReport {
                    test: finished.key,
                    results,
                })));
            }
        }
    }
}

/// The CPU measure: builds `spec` from `resources` and steps it `ticks`
/// ticks with nobody seated (all AI); the mean wall-clock cost of a tick in
/// microseconds, at least 1. `None` when the mission does not build or a
/// tick fails.
pub fn measure_cpu(
    spec: &MissionSpec,
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
    ticks: u64,
) -> Option<u32> {
    let reads = ResourceReads::new(resources);
    let mut world = World::new(spec, &reads, Seating::Open).ok()?;
    let mut out = TickOutput::default();
    let started = Instant::now();
    for _ in 0..ticks {
        world.step(&[], &mut out).ok()?;
    }
    let micros = started.elapsed().as_micros() / u128::from(ticks.max(1));
    Some(micros.clamp(1, u128::from(u32::MAX)) as u32)
}

#[cfg(test)]
#[path = "candidate_tests.rs"]
mod tests;
