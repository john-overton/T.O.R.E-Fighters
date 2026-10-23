//! Candidates and host selection (stage K, slice K6; docs/ARCHITECTURE.md,
//! "Host selection"): each game's Candidate report, the reach and upload
//! tests, eligibility and ranking, the succession, setting 21's pin and its
//! fallback, and the King's warnings.
//!
//! Only a game a player hosts selects: a dedicated server keeps the reports
//! it is sent and does nothing else (it never migrates).
//!
//! - **Reports** (message 40) are kept by the player's join order, the
//!   identity that survives a migration.
//! - **Reach tests** run for the three best candidates by the other
//!   measures, whenever a player joins or a report changes, at most one
//!   every 10 seconds: each candidate is sent Reach peers (open your router
//!   to these players) and every other direct player a Reach test (reach
//!   these candidates); each player's Reach report says whether it reached
//!   each candidate and the median round trip. A player that does not
//!   report within [`REACH_ROUND_LIMIT`] reached nobody.
//! - **Upload tests**, in the lobby only, one at a time: the best untested
//!   eligible candidate of the three best is asked for a 1-second burst of
//!   Filler at the rate the game needs ([`upload_need`]: 56 KB/s for every
//!   other player at the default 60 snapshots a second, [`need_per_player`],
//!   and each standby's [`standby_need`], warm and cold apart), and passes
//!   when 90 percent of it arrives.
//! - **The house** (the game that hosts now) cannot test its own upload in
//!   the lobby; it is judged from its flights instead: the share of its
//!   packets its direct players' games acknowledge (agent decision).
//! - **Eligible:** a game a player hosts; not relayed (John, 2026-09-28);
//!   its switch on; reached by every other direct player. **Ranked** by
//!   upload (a pass, a fail, untested), the median round trip in 20 ms
//!   steps, the router, the CPU in 0.5 ms steps, then the longest
//!   connected.
//! - **The calculated host** stays on the house while it passes; in the
//!   lobby, when it does not and another candidate does, the game moves to
//!   the best one passing (John, 2026-10-05). The move is slice K4's
//!   handover, called once the player to move to is ready as standby 1
//!   (slice KP; [`Host::host_move`] says who it is).
//! - **The pin** (setting 21): a player the King names. A relayed player,
//!   one who switched hosting off, or a pin on a dedicated server is
//!   refused; a pinned player who leaves falls back to calculated, with a
//!   line to the King.
//! - **Warnings** go to the King in Messages, once each while they hold.

#[cfg(test)]
#[path = "succession_tests.rs"]
mod tests;

#[path = "succession_state.rs"]
pub(super) mod state;

use super::{ConnectionId, Host, Life, Peer, Stage};
use crate::settings::{CALCULATED_HOST, number};
use crate::wire::messages::Message;
use crate::wire::migration::{
    CandidateReport, ReachPeers, ReachReport, ReachTarget, ReachTest,
    Succession as SuccessionMessage, Successor, UploadTest, limits,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Duration;
use tore_net::master::Path;
use tore_net::master::candidate::{Candidate, CandidateKind, MappingType};

/// At most one reach test this often.
pub const REACH_INTERVAL: Duration = Duration::from_secs(10);
/// A reach test's players report within this long, or reached nobody: the
/// test's 1.8 seconds, a round trip and a margin (agent decision).
pub const REACH_ROUND_LIMIT: Duration = Duration::from_secs(4);
/// Candidates a reach test or the upload tests try.
pub const TESTED_CANDIDATES: usize = 3;
/// An upload test's length.
pub const UPLOAD_LENGTH_MS: u16 = 1_000;
/// After a burst's last byte could have left, the host counts what arrives
/// for a round trip and this long more (agent decision).
pub const UPLOAD_GRACE: Duration = Duration::from_millis(500);
/// The time between two upload tests (agent decision).
pub const UPLOAD_GAP: Duration = Duration::from_secs(2);
/// The upload need for every other player at 30 snapshots a second, bytes
/// a second: the stage D peak per player (docs/baselines/net-2026-09-30.md).
pub const NEED_PER_PLAYER: u32 = 28_000;

/// The upload need for every other player at `snapshot_rate` snapshots a
/// second, bytes a second: [`NEED_PER_PLAYER`] scaled by the rate over 30,
/// 56 KB/s at the default 60 (slice D12). *Agent decision:* scaled by the
/// rate, not fitted: a snapshot's bytes do not depend on the rate (the
/// 15 against 15 mission's mean packet was 388 bytes at both), and the
/// measured download per player grew 1.75 to 1.94 times from 30 and twice a
/// second to 60 and 4, its busiest second 1.56 to 1.76 times
/// (docs/baselines/net-rates-2026-10-06.md), so the doubled need keeps
/// the old one's place between a player's average and its busiest second,
/// at most about an eighth higher than a fit would put it.
pub fn need_per_player(snapshot_rate: u32) -> u32 {
    NEED_PER_PLAYER * snapshot_rate.clamp(1, 120) / 30
}
/// The upload a warm standby needs besides its humans' share, bytes a
/// second: the state parts, the Checks and the transport's framing, at
/// three humans (slice KP, from protocol 14's figures in
/// docs/baselines/standby-stream-2026-10-05.md).
pub const WARM_STANDBY_BASE: u32 = 3_000;
/// The upload a warm standby needs for each human flying, bytes a second:
/// 3.4 to 5.6 bytes a seat a tick at 120 ticks a second is 410 to 670, and
/// 12.8 KB/s of stream (16.2 KB/s on the wire) with 30 humans, 3.9 KB/s on
/// the wire with 3; the fit through both ends with a margin of 15 percent
/// (slice KP). *Fitted.*
pub const WARM_STANDBY_PER_HUMAN: u32 = 520;
/// What a cold standby's checkpoints need besides the journal, bytes a
/// second: the real-data 15 against 15 mission's stream averaged 64 KB/s
/// (slice K3) and 70.6 KB/s (slice KP, 83.5 KB/s on the wire, its first
/// minutes 87 KB/s) on four humans, its checkpoints of 0.52 to 1.1 MB each
/// taking 8 to 11 seconds. The need is the wire average with a little over,
/// 80 KB/s: the stream's pace holds it under the 1 Mbit/s line, and the
/// transport queues across the busy seconds (slice KP). *Fitted.*
pub const COLD_STANDBY_CHECKPOINTS: u32 = 80_000;
/// A candidate passes the upload test when this share of the burst arrives,
/// per mille.
pub const UPLOAD_PASS: u16 = 900;
/// Round trips rank in steps of this many milliseconds.
pub const ROUND_TRIP_STEP_MS: u32 = 20;
/// The CPU measure ranks in steps of this many microseconds.
pub const CPU_STEP_MICROS: u32 = 500;
/// Half of one core at 120 ticks a second: the busiest minute's mean tick
/// must stay under it, microseconds.
pub const CPU_BUDGET_MICROS: u32 = 4_166;
/// The busiest minute's mean tick over the CPU measure's (the first 240
/// ticks, all AI), per mille: 0.81 to 0.85 on the real-data 15 against 15
/// mission, whose first minute is its busiest, so the measure itself
/// slightly overstates it; the highest is kept
/// (docs/baselines/host-selection-2026-10-05.md). *Fitted.* A measure up
/// to 4.9 ms a tick passes.
pub const CPU_BUSY_PER_MILLE: u32 = 850;
/// The house's flight figure: one sample a second, the last this many.
pub const HOUSE_SAMPLES: usize = 10;

/// The upload one standby needs, bytes a second, with `humans` flying: a
/// warm one the journal's share of each human, a cold one that and its
/// checkpoints besides.
pub fn standby_need(humans: usize, cold: bool) -> u32 {
    let journal = WARM_STANDBY_BASE + WARM_STANDBY_PER_HUMAN * humans.min(64) as u32;
    if cold {
        journal + COLD_STANDBY_CHECKPOINTS
    } else {
        journal
    }
}

/// The upload a host needs, bytes a second, for `players` players (itself
/// included) at `snapshot_rate` snapshots a second: [`need_per_player`] for
/// every other player, and for each standby, of
/// which there are up to two, [`standby_need`]: `cold_standbys` of them
/// cold (the others warm).
pub fn upload_need(players: usize, cold_standbys: usize, snapshot_rate: u32) -> u32 {
    let others = players.saturating_sub(1);
    let standbys = others.min(limits::STANDBYS);
    let cold = cold_standbys.min(standbys);
    need_per_player(snapshot_rate) * others as u32
        + (standbys - cold) as u32 * standby_need(players, false)
        + cold as u32 * standby_need(players, true)
}

/// How open a router is, best first: no translation or a mapped port, one
/// outside port for every destination, a new one for each, unknown.
pub fn router_rank(report: &CandidateReport) -> u8 {
    let mapped = report
        .candidates
        .iter()
        .any(|c| c.kind == CandidateKind::Mapped);
    match report.mapping {
        _ if mapped => 0,
        MappingType::NoTranslation => 0,
        MappingType::SamePort => 1,
        MappingType::PortPerDestination => 2,
        MappingType::Unknown => 3,
    }
}

/// The busiest minute's predicted mean tick for a CPU measure, as a share
/// of [`CPU_BUDGET_MICROS`], percent.
pub fn cpu_percent(cpu_micros: u32) -> u32 {
    (u64::from(cpu_micros) * u64::from(CPU_BUSY_PER_MILLE) / 10 / u64::from(CPU_BUDGET_MICROS))
        as u32
}

/// An upload figure: the players it was measured for, and the share of
/// what they need that arrived, per mille.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Upload {
    pub players: u16,
    pub per_mille: u16,
}

impl Upload {
    fn passed(self) -> bool {
        self.per_mille >= UPLOAD_PASS
    }
}

/// What the host knows of one player as a candidate.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Measure {
    pub report: Option<CandidateReport>,
    /// By the tester's join order: its median round trip to this candidate
    /// in milliseconds, or `None` when it did not reach it.
    pub reached_by: BTreeMap<u64, Option<u16>>,
    pub upload: Option<Upload>,
}

/// A reach test under way.
#[derive(Debug)]
struct ReachRound {
    test: u16,
    started: Duration,
    /// The testers yet to report, by join order, with the candidates each
    /// was given.
    waiting: BTreeMap<u64, Vec<u64>>,
}

/// An upload test under way.
#[derive(Debug)]
struct UploadRun {
    order: u64,
    connection: ConnectionId,
    /// Filler counts until then.
    ends: Duration,
    expected: u64,
    received: u64,
    players: u16,
}

/// The host's candidates and their measures.
#[derive(Debug, Default)]
pub(super) struct Succession {
    /// Filler sections' bytes received in all (protocol 13).
    pub(super) filler_bytes: u64,
    /// By join order.
    pub(super) measures: BTreeMap<u64, Measure>,
    next_test: u16,
    reach: Option<ReachRound>,
    last_reach: Option<Duration>,
    /// A player joined or a report changed since the last reach test.
    reach_due: bool,
    upload: Option<UploadRun>,
    last_upload: Option<Duration>,
    /// The house's figure from its flights, and its samples.
    pub(super) house_upload: Option<Upload>,
    house_samples: VecDeque<f64>,
    last_sample: Option<Duration>,
    /// The players present at the last update, by join order.
    present: BTreeSet<u64>,
    /// The pinned player as last seen: its lobby id and callsign.
    pinned: Option<(u8, String)>,
    /// The warnings the King holds now.
    warned: BTreeSet<String>,
    /// The game should move to this player in the lobby (slice K4 hands
    /// over); by join order.
    move_to: Option<u64>,
    /// The last Succession sent.
    sent: Option<SuccessionMessage>,
}

/// The words of the King's warnings and refusals (agent decisions where the
/// design gives none).
pub mod words {
    /// No other game can take over hosting (the design's words).
    pub const ALONE: &str = "No other game can take over hosting: if you leave, the game ends.";

    /// A relayed player pinned (the design's words).
    pub fn relayed(callsign: &str) -> String {
        format!("{callsign} connects through the relay and cannot host.")
    }

    /// No machine passed the upload test (the design's words). `lower` is
    /// the highest snapshot rate below the one in force that the best
    /// machine's figure would fit (slice R1).
    pub fn upload(players: usize, best: &str, percent: u32, lower: Option<u32>) -> String {
        format!(
            "No machine here passed the test for {players} players: {best}'s carried {percent} percent of what they need. {}Fewer players, or a dedicated server, will fly better.",
            rate_hint(lower)
        )
    }

    /// The pinned host did not pass the upload test.
    pub fn pinned_upload(players: usize, pinned: &str, percent: u32, lower: Option<u32>) -> String {
        format!(
            "{pinned}'s machine, the pinned host, carried {percent} percent of what {players} players need. {}Fewer players, or a dedicated server, will fly better.",
            rate_hint(lower)
        )
    }

    /// The sentence that says a lower snapshot rate would fit (slice R1;
    /// *agent decision* on the words).
    fn rate_hint(lower: Option<u32>) -> String {
        match lower {
            Some(rate) => format!(
                "At {rate} snapshots a second it would fit: turn the snapshot rate down in the lobby's Settings. "
            ),
            None => String::new(),
        }
    }

    /// No machine passed the CPU test.
    pub fn cpu(best: &str, percent: u32) -> String {
        format!(
            "No machine here passed the processor test for this mission: {best}'s needs {percent} percent of the time it has. Fewer aircraft, or a dedicated server, will fly better."
        )
    }

    /// The pinned host did not pass the CPU test.
    pub fn pinned_cpu(pinned: &str, percent: u32) -> String {
        format!(
            "{pinned}'s machine, the pinned host, needs {percent} percent of the time this mission has. Fewer aircraft, or a dedicated server, will fly better."
        )
    }

    /// The pinned player left.
    pub fn pin_left(callsign: &str) -> String {
        format!("{callsign}, the pinned host, left: the host is calculated again.")
    }

    /// A pinned player turned hosting off.
    pub fn switched_off(callsign: &str) -> String {
        format!("{callsign} has turned off \"Let my game take over hosting\".")
    }

    /// A pin on a dedicated server.
    pub const DEDICATED: &str = "A dedicated server always hosts its own game.";

    /// A pin naming nobody.
    pub fn no_player(id: u32) -> String {
        format!("No player has the lobby id {id}.")
    }

    /// The line when the game moves in the lobby (the design's words).
    pub fn moved(callsign: &str, players: usize) -> String {
        format!("The game moved to {callsign}'s machine, which can carry {players} players.")
    }
}

impl Host {
    // ----- Requests ----------------------------------------------------

    /// A player's Candidate report (message 40): kept by its join order.
    pub(super) fn candidate_report(
        &mut self,
        connection: ConnectionId,
        report: &CandidateReport,
    ) -> Result<(), String> {
        let Some(order) = self.join_order_of(connection) else {
            return Ok(());
        };
        let measure = self.succession.measures.entry(order).or_default();
        if measure.report.as_ref() != Some(report) {
            let addresses = measure
                .report
                .as_ref()
                .is_none_or(|r| r.candidates != report.candidates);
            if addresses {
                // Its addresses changed: what players reached is stale.
                measure.reached_by.clear();
            }
            measure.report = Some(report.clone());
            self.succession.reach_due |= addresses || report.may_host;
        }
        Ok(())
    }

    /// A player's Reach report (message 43): its reach of each candidate of
    /// the running test. A report for another test is ignored.
    pub(super) fn reach_report(
        &mut self,
        connection: ConnectionId,
        report: &ReachReport,
    ) -> Result<(), String> {
        let Some(tester) = self.join_order_of(connection) else {
            return Ok(());
        };
        let ids: BTreeMap<u8, u64> = self
            .peers
            .values()
            .map(|p| (p.lobby.id, p.lobby.order))
            .collect();
        let Some(round) = self
            .succession
            .reach
            .as_mut()
            .filter(|round| round.test == report.test)
        else {
            return Ok(());
        };
        let Some(given) = round.waiting.remove(&tester) else {
            return Ok(());
        };
        for candidate in given {
            let reached = report
                .results
                .iter()
                .find(|r| ids.get(&r.player) == Some(&candidate))
                .and_then(|r| r.reached)
                .map(|r| r.round_trip_ms);
            if let Some(measure) = self.succession.measures.get_mut(&candidate) {
                measure.reached_by.insert(tester, reached);
            }
        }
        Ok(())
    }

    /// A Filler section from `connection`: counted, and measured when an
    /// upload test of that player runs.
    pub(super) fn filler_received(&mut self, connection: ConnectionId, bytes: usize) {
        self.succession.filler_bytes += bytes as u64;
        let now = self.now;
        if let Some(run) = &mut self.succession.upload
            && run.connection == connection
            && now <= run.ends
        {
            run.received += bytes as u64;
        }
    }

    /// Why the King's pin `value` of setting 21 is refused, if it is: a
    /// dedicated server, nobody with that lobby id, a relayed player, or one
    /// who switched hosting off.
    pub(super) fn host_pin_refusal(&self, value: u32) -> Result<(), String> {
        // Calculated, or the pin as it stands (a screen that sends every
        // setting back), is never refused.
        let unchanged = self.settings.pinned_host().map(|id| u32::from(id) + 1) == Some(value);
        if value == CALCULATED_HOST || unchanged {
            return Ok(());
        }
        if self.config.house.is_none() {
            return Err(words::DEDICATED.into());
        }
        let id = value - 1;
        let Some(peer) = self
            .live_peers()
            .map(|(_, p)| p)
            .find(|p| u32::from(p.lobby.id) == id)
        else {
            return Err(words::no_player(id));
        };
        if peer.path == Path::Relay {
            return Err(words::relayed(&peer.callsign));
        }
        let switched_off = self
            .candidate_of(peer.lobby.order)
            .is_some_and(|r| !r.may_host);
        if switched_off && !peer.house {
            return Err(words::switched_off(&peer.callsign));
        }
        Ok(())
    }

    // ----- What the rest of the host reads ------------------------------

    /// The player the game should move to in the lobby, by lobby id: the
    /// pinned player, or the best candidate passing when the house does not
    /// (slice K4 hands over to it). `None` stays.
    #[allow(dead_code)] // Slices K3 and K4 call it; until then the tests do.
    pub(crate) fn host_move(&self) -> Option<u8> {
        let order = self.succession.move_to?;
        self.peer_of_order(order).map(|(_, p)| p.lobby.id)
    }

    /// The standbys the host should keep, best first: the eligible
    /// candidates other than the house, at most two (slice K3 appoints
    /// them).
    #[allow(dead_code)] // Slices K3 and K4 call it; until then the tests do.
    pub(crate) fn standby_choice(&self) -> Vec<ConnectionId> {
        let mut ranked = self.ranked_candidates();
        ranked.truncate(limits::STANDBYS);
        ranked
            .into_iter()
            .filter_map(|order| self.peer_of_order(order).map(|(c, _)| c))
            .collect()
    }

    /// Every eligible candidate other than the house, best first, by join
    /// order.
    pub(super) fn ranked_candidates(&self) -> Vec<u64> {
        let mut eligible: Vec<u64> = self
            .live_peers()
            .filter(|(_, p)| !p.house)
            .map(|(_, p)| p.lobby.order)
            .filter(|&order| self.may_take_over(order))
            .collect();
        eligible.sort_by_key(|&order| self.candidate_rank(order));
        eligible
    }

    /// The Succession message for `ready` standbys (in order, each warm or
    /// cold): each with the address the host sees it at and its candidates.
    pub(crate) fn succession_message(&self, ready: &[(ConnectionId, bool)]) -> SuccessionMessage {
        let standbys = ready
            .iter()
            .take(limits::STANDBYS)
            .filter_map(|&(connection, warm)| {
                let peer = self.peers.get(&connection)?;
                let mut addresses = vec![Candidate::new(CandidateKind::Seen, peer.address)];
                if let Some(report) = self.candidate_of(peer.lobby.order) {
                    addresses.extend(report.candidates.iter().copied());
                }
                addresses.truncate(limits::ADDRESSES);
                Some(Successor {
                    player: peer.lobby.id,
                    warm,
                    addresses,
                })
            })
            .collect();
        SuccessionMessage { standbys }
    }

    /// Sends every player the Succession for `ready` when it changed (slice
    /// K3 calls it as standbys become ready or leave).
    pub(crate) fn send_succession(&mut self, ready: &[(ConnectionId, bool)]) {
        let message = self.succession_message(ready);
        if self.succession.sent.as_ref() == Some(&message) {
            return;
        }
        self.succession.sent = Some(message.clone());
        let to: Vec<ConnectionId> = self.live_peers().map(|(c, _)| c).collect();
        let message = Message::Succession(Box::new(message));
        for connection in to {
            self.send(connection, &message);
        }
    }

    /// A player who joined or resumed after the last Succession went out is
    /// sent it, or it could not follow a migration (slice K10: found in
    /// `net-reach-upload`, where a late joiner dropped after a handover).
    pub(super) fn succession_connected(&mut self, connection: ConnectionId) {
        if !self.peers.contains_key(&connection) {
            return;
        }
        let Some(sent) = self
            .succession
            .sent
            .as_ref()
            .filter(|s| !s.standbys.is_empty())
        else {
            return;
        };
        let message = Message::Succession(Box::new(sent.clone()));
        self.send(connection, &message);
    }

    // ----- The timers --------------------------------------------------

    /// What is due at `now`: players gone and joined, the pin's fallback,
    /// the reach and upload tests, the house's flight figure, the lobby's
    /// move and the King's warnings. Called from [`Host::update`].
    pub(super) fn succession_update(&mut self, now: Duration) {
        // A dedicated server only forgets the players who left.
        self.note_candidates();
        if self.config.house.is_none() {
            return;
        }
        let lobby = matches!(self.life, Life::Lobby);
        if !lobby && !matches!(self.life, Life::Flying) {
            return;
        }
        self.pin_update();
        self.reach_update(now);
        if lobby {
            self.upload_update(now);
            self.decide_host();
        } else {
            self.succession.upload = None;
            self.house_sample(now);
        }
    }

    /// Forgets players who left and notes joins (a reach test is due).
    fn note_candidates(&mut self) {
        let present: BTreeSet<u64> = self.live_peers().map(|(_, p)| p.lobby.order).collect();
        if present == self.succession.present {
            return;
        }
        let joined = present
            .difference(&self.succession.present)
            .next()
            .is_some();
        let gone: Vec<u64> = self
            .succession
            .present
            .difference(&present)
            .copied()
            .collect();
        for order in &gone {
            self.succession.measures.remove(order);
            for measure in self.succession.measures.values_mut() {
                measure.reached_by.remove(order);
            }
            if self.succession.move_to == Some(*order) {
                self.succession.move_to = None;
            }
            if let Some(round) = &mut self.succession.reach {
                round.waiting.remove(order);
                for given in round.waiting.values_mut() {
                    given.retain(|c| c != order);
                }
            }
            if self
                .succession
                .upload
                .as_ref()
                .is_some_and(|run| run.order == *order)
            {
                self.succession.upload = None;
            }
        }
        self.succession.reach_due |= joined;
        self.succession.present = present;
    }

    /// Remembers the pinned player, and falls back to calculated when it
    /// has left.
    fn pin_update(&mut self) {
        let Some(id) = self.settings.pinned_host() else {
            self.succession.pinned = None;
            return;
        };
        let found = self
            .live_peers()
            .map(|(_, p)| p)
            .find(|p| p.lobby.id == id)
            .map(|p| p.callsign.clone());
        match found {
            Some(callsign) => self.succession.pinned = Some((id, callsign)),
            None => {
                let callsign = self
                    .succession
                    .pinned
                    .take()
                    .filter(|(pinned, _)| *pinned == id)
                    .map_or_else(|| format!("Player {id}"), |(_, callsign)| callsign);
                if self
                    .settings
                    .apply(&[(number::HOST, CALCULATED_HOST)])
                    .is_ok()
                {
                    self.lobby_dirty = true;
                    self.tell_king_line(words::pin_left(&callsign));
                }
            }
        }
    }

    /// Starts a reach test when one is due, and ends the one running when
    /// every player reported or its time is up.
    fn reach_update(&mut self, now: Duration) {
        if let Some(round) = &self.succession.reach
            && (round.waiting.is_empty() || now.saturating_sub(round.started) >= REACH_ROUND_LIMIT)
        {
            let round = self.succession.reach.take().expect("a round");
            for (tester, given) in round.waiting {
                for candidate in given {
                    if let Some(measure) = self.succession.measures.get_mut(&candidate) {
                        measure.reached_by.insert(tester, None);
                    }
                }
            }
        }
        if self.succession.reach.is_some()
            || !self.succession.reach_due
            || self
                .succession
                .last_reach
                .is_some_and(|last| now.saturating_sub(last) < REACH_INTERVAL)
        {
            return;
        }
        self.succession.reach_due = false;
        let mut candidates: Vec<u64> = self
            .reach_testers()
            .map(|(_, p)| p.lobby.order)
            .filter(|&order| self.candidate_of(order).is_some_and(|r| r.may_host))
            .collect();
        candidates.sort_by_key(|&order| self.reach_test_key(order));
        candidates.truncate(TESTED_CANDIDATES);
        let testers: Vec<(ConnectionId, u64)> = self
            .reach_testers()
            .map(|(c, p)| (c, p.lobby.order))
            .collect();
        let test = self.succession.next_test;
        let mut waiting = BTreeMap::new();
        let mut tests = Vec::new();
        for &(connection, tester) in &testers {
            let given: Vec<u64> = candidates
                .iter()
                .copied()
                .filter(|&c| c != tester)
                .collect();
            if given.is_empty() {
                continue;
            }
            let targets = given.iter().filter_map(|&c| self.reach_target(c)).collect();
            tests.push((
                connection,
                ReachTest {
                    test,
                    candidates: targets,
                },
            ));
            waiting.insert(tester, given);
        }
        if waiting.is_empty() {
            return;
        }
        self.succession.next_test = test.wrapping_add(1);
        self.succession.last_reach = Some(now);
        let mut opens = Vec::new();
        for &candidate in &candidates {
            let Some((connection, _)) = self.peer_of_order(candidate) else {
                continue;
            };
            let players = testers
                .iter()
                .filter(|&&(_, t)| t != candidate)
                .filter_map(|&(_, t)| self.reach_target(t))
                .take(limits::REACH_PEERS)
                .collect();
            opens.push((connection, ReachPeers { test, players }));
        }
        // Each candidate opens its router as the players start to reach it.
        for (connection, peers) in opens {
            self.send(connection, &Message::ReachPeers(Box::new(peers)));
        }
        for (connection, reach) in tests {
            self.send(connection, &Message::ReachTest(Box::new(reach)));
        }
        self.succession.reach = Some(ReachRound {
            test,
            started: now,
            waiting,
        });
    }

    /// A player as a reach target: its lobby id, the address the host sees
    /// it at and its own candidates (agent decision: all of them, so a
    /// candidate opens its firewall to a player's IPv6 address too).
    fn reach_target(&self, order: u64) -> Option<ReachTarget> {
        let (_, peer) = self.peer_of_order(order)?;
        let mut addresses = vec![peer.address];
        if let Some(report) = self.candidate_of(order) {
            for candidate in &report.candidates {
                if !addresses.contains(&candidate.address) {
                    addresses.push(candidate.address);
                }
            }
        }
        addresses.truncate(limits::ADDRESSES);
        Some(ReachTarget {
            player: peer.lobby.id,
            addresses,
        })
    }

    /// The King changed the snapshot rate (slice R1): every upload figure
    /// measured what the old rate needs, so they are forgotten and the tests
    /// run again at the new one.
    pub(super) fn rate_changed(&mut self) {
        for measure in self.succession.measures.values_mut() {
            measure.upload = None;
        }
        self.succession.upload = None;
        self.succession.house_upload = None;
        self.succession.house_samples.clear();
    }

    /// Ends the running upload test when its time is up, and starts the next
    /// one due.
    fn upload_update(&mut self, now: Duration) {
        if let Some(run) = &self.succession.upload {
            if now < run.ends {
                return;
            }
            let run = self.succession.upload.take().expect("a run");
            let per_mille = (run.received * 1_000 / run.expected.max(1)).min(1_000) as u16;
            if let Some(measure) = self.succession.measures.get_mut(&run.order) {
                measure.upload = Some(Upload {
                    players: run.players,
                    per_mille,
                });
            }
            self.succession.last_upload = Some(now);
        }
        if self
            .succession
            .last_upload
            .is_some_and(|last| now.saturating_sub(last) < UPLOAD_GAP)
        {
            return;
        }
        let players = self.live_peers().count();
        let mut best: Vec<u64> = self
            .live_peers()
            .filter(|(_, p)| !p.house)
            .map(|(_, p)| p.lobby.order)
            .filter(|&order| self.may_take_over(order))
            .collect();
        best.sort_by_key(|&order| {
            let (_, rtt, router, cpu, order) = self.candidate_rank(order);
            (rtt, router, cpu, order)
        });
        best.truncate(TESTED_CANDIDATES);
        let Some(order) = best.into_iter().find(|&order| {
            self.succession
                .measures
                .get(&order)
                .is_none_or(|m| m.upload.is_none_or(|u| usize::from(u.players) < players))
        }) else {
            return;
        };
        let Some((connection, _)) = self.peer_of_order(order) else {
            return;
        };
        let rate = upload_need(
            players,
            self.cold_standbys_for(order),
            self.settings.snapshot_rate(),
        );
        let round_trip = self
            .server
            .stats(connection)
            .map_or(Duration::ZERO, |s| s.round_trip);
        let length = Duration::from_millis(u64::from(UPLOAD_LENGTH_MS));
        let test = self.succession.next_test;
        self.succession.next_test = test.wrapping_add(1);
        self.succession.upload = Some(UploadRun {
            order,
            connection,
            ends: now + length + round_trip + UPLOAD_GRACE,
            expected: u64::from(rate) * u64::from(UPLOAD_LENGTH_MS) / 1_000,
            received: 0,
            players: players.min(usize::from(u16::MAX)) as u16,
        });
        self.send(
            connection,
            &Message::UploadTest(UploadTest {
                test,
                rate,
                length_ms: UPLOAD_LENGTH_MS,
            }),
        );
    }

    /// One sample a second of the share of the house's packets its direct
    /// players' games acknowledge (the median player's), while flying.
    fn house_sample(&mut self, now: Duration) {
        if self
            .succession
            .last_sample
            .is_some_and(|last| now.saturating_sub(last) < Duration::from_secs(1))
        {
            return;
        }
        self.succession.last_sample = Some(now);
        let mut delivered: Vec<f64> = self
            .reach_testers()
            .filter_map(|(c, _)| self.server.stats(c)?.loss)
            .map(|loss| 1. - loss)
            .collect();
        if delivered.is_empty() {
            return;
        }
        delivered.sort_by(f64::total_cmp);
        let median = delivered[(delivered.len() - 1) / 2];
        let samples = &mut self.succession.house_samples;
        samples.push_back(median);
        while samples.len() > HOUSE_SAMPLES {
            samples.pop_front();
        }
        if samples.len() >= HOUSE_SAMPLES / 2 {
            let mean = samples.iter().sum::<f64>() / samples.len() as f64;
            let players = self.live_peers().count().min(usize::from(u16::MAX)) as u16;
            self.succession.house_upload = Some(Upload {
                players,
                per_mille: (mean * 1_000.).round().clamp(0., 1_000.) as u16,
            });
        }
    }

    /// Carries out the lobby's move (slice KP, with slice K4's handover):
    /// the player to move to becomes standby 1 and, once it is ready, the
    /// game is handed to it with one call to [`Host::hand_over`], and the
    /// King reads the line in Messages. Nothing happens while the standbys
    /// are off, the player is not ready or the game has been handed over.
    fn carry_out_move(&mut self, players: usize) {
        if self.resume_handed() {
            return;
        }
        let Some(order) = self.succession.move_to else {
            return;
        };
        if !self.make_first_standby(order) {
            return;
        }
        let Some((connection, peer)) = self.peer_of_order(order) else {
            return;
        };
        if self.ready_standbys().first().map(|&(c, _)| c) != Some(connection) {
            return;
        }
        let line = words::moved(&peer.callsign, players);
        self.tell_king_line(line);
        let _ = self.hand_over();
    }

    /// The lobby's move and the King's warnings.
    fn decide_host(&mut self) {
        let players = self.live_peers().count();
        let house = self
            .live_peers()
            .find(|(_, p)| p.house)
            .map(|(_, p)| p.lobby.order);
        let pinned = self
            .settings
            .pinned_host()
            .and_then(|id| self.live_peers().find(|(_, p)| p.lobby.id == id))
            .map(|(_, p)| p.lobby.order);
        let host_passes = house.is_none_or(|h| self.host_passes_for(h, players));
        let passing: Vec<u64> = self
            .ranked_candidates()
            .into_iter()
            .filter(|&order| self.host_passes_for(order, players))
            .collect();
        let move_to = match pinned {
            Some(order) if Some(order) == house => None,
            Some(order) => Some(order),
            None if host_passes => None,
            None => passing.first().copied(),
        };
        self.succession.move_to = move_to;
        self.carry_out_move(players);
        let mut warnings = BTreeSet::new();
        if let Some(host) = pinned.or(house)
            && !self.host_passes_for(host, players)
            && (pinned.is_some() || passing.is_empty())
        {
            let callsign = self
                .peer_of_order(host)
                .map(|(_, p)| p.callsign.clone())
                .unwrap_or_default();
            if let Some(upload) = self.upload_figure(host).filter(|u| !u.passed()) {
                let percent = u32::from(upload.per_mille) / 10;
                warnings.insert(match pinned {
                    Some(_) => words::pinned_upload(
                        players,
                        &callsign,
                        percent,
                        self.lower_rate_that_fits(host, upload),
                    ),
                    None => {
                        let (best, percent, lower) = self.best_upload_figure(players).unwrap_or((
                            callsign.clone(),
                            percent,
                            None,
                        ));
                        words::upload(players, &best, percent, lower)
                    }
                });
            }
            if let Some(cpu) = self.cpu_figure(host).filter(|&cpu| cpu_percent(cpu) > 100) {
                warnings.insert(match pinned {
                    Some(_) => words::pinned_cpu(&callsign, cpu_percent(cpu)),
                    None => words::cpu(&callsign, cpu_percent(cpu)),
                });
            }
        }
        // Said once every other game has reported (a relayed one never
        // counts) and the reach tests are done.
        let others = self.live_peers().any(|(_, p)| !p.house);
        let reported = self.live_peers().all(|(_, p)| {
            p.house || p.path == Path::Relay || self.candidate_of(p.lobby.order).is_some()
        });
        if others
            && reported
            && self.succession.reach.is_none()
            && !self.succession.reach_due
            && self.ranked_candidates().is_empty()
        {
            warnings.insert(words::ALONE.to_owned());
        }
        let new: Vec<String> = warnings
            .difference(&self.succession.warned)
            .cloned()
            .collect();
        self.succession.warned = warnings;
        for line in new {
            self.tell_king_line(line);
        }
    }

    // ----- Measures ----------------------------------------------------

    /// Whether `order` may take over hosting: in a game a player hosts, not
    /// relayed, its switch on, and reached by every other direct player.
    /// The house is eligible when it is direct.
    fn may_take_over(&self, order: u64) -> bool {
        let Some((_, peer)) = self.peer_of_order(order) else {
            return false;
        };
        if self.config.house.is_none() || peer.path == Path::Relay {
            return false;
        }
        if peer.house {
            return true;
        }
        if !self.candidate_of(order).is_some_and(|r| r.may_host) {
            return false;
        }
        let reached_by = self.succession.measures.get(&order).map(|m| &m.reached_by);
        self.reach_testers()
            .map(|(_, p)| p.lobby.order)
            .filter(|&tester| tester != order)
            .all(|tester| {
                reached_by
                    .and_then(|r| r.get(&tester))
                    .is_some_and(Option::is_some)
            })
    }

    /// Whether a standby already appointed (slice K3) may stay one: as
    /// [`Host::may_take_over`], but a player not yet in a reach test does not
    /// count against it, only one whose test found it unreachable. A player
    /// joining does not dismiss the standbys until the next reach test.
    pub(super) fn may_stay_standby(&self, order: u64) -> bool {
        let Some((_, peer)) = self.peer_of_order(order) else {
            return false;
        };
        if self.config.house.is_none() || peer.path == Path::Relay || peer.house {
            return false;
        }
        if !self.candidate_of(order).is_some_and(|r| r.may_host) {
            return false;
        }
        let reached_by = self.succession.measures.get(&order).map(|m| &m.reached_by);
        self.reach_testers()
            .map(|(_, p)| p.lobby.order)
            .filter(|&tester| tester != order)
            .all(|tester| {
                reached_by
                    .and_then(|r| r.get(&tester))
                    .is_none_or(Option::is_some)
            })
    }

    /// Whether `order` passes for `players`: eligible, its upload passed for
    /// at least that many (the house: unless its flights say otherwise), and
    /// its CPU measure within the budget (or not measured).
    fn host_passes_for(&self, order: u64, players: usize) -> bool {
        let Some((_, peer)) = self.peer_of_order(order) else {
            return false;
        };
        if !self.may_take_over(order) {
            return false;
        }
        let upload = match self.upload_figure(order) {
            Some(upload) if peer.house => upload.passed(),
            Some(upload) => upload.passed() && usize::from(upload.players) >= players,
            None => peer.house,
        };
        upload
            && self
                .cpu_figure(order)
                .is_none_or(|cpu| cpu_percent(cpu) <= 100)
    }

    /// The ranking key: upload (a pass, a fail, untested), the median round
    /// trip in 20 ms steps, the router, the CPU in 0.5 ms steps, the join
    /// order. Lower is better.
    pub(super) fn candidate_rank(&self, order: u64) -> (u8, u32, u8, u32, u64) {
        let measure = self.succession.measures.get(&order);
        let upload = match measure.and_then(|m| m.upload) {
            Some(upload) if upload.passed() => 0,
            Some(_) => 1,
            None => 2,
        };
        let mut trips: Vec<u16> = measure
            .map(|m| m.reached_by.values().flatten().copied().collect())
            .unwrap_or_default();
        trips.sort_unstable();
        let rtt = trips
            .get(trips.len().saturating_sub(1) / 2)
            .map_or(u32::MAX, |&ms| u32::from(ms) / ROUND_TRIP_STEP_MS);
        let report = measure.and_then(|m| m.report.as_ref());
        let router = report.map_or(3, router_rank);
        let cpu = report
            .map(|r| r.cpu_micros)
            .filter(|&micros| micros > 0)
            .map_or(u32::MAX, |micros| micros / CPU_STEP_MICROS);
        (upload, rtt, router, cpu, order)
    }

    /// The order of the reach tests' candidates: by the measures other than
    /// the round trip.
    fn reach_test_key(&self, order: u64) -> (u8, u8, u32, u64) {
        let (upload, _, router, cpu, order) = self.candidate_rank(order);
        (upload, router, cpu, order)
    }

    /// How many standbys `order`'s game would have as the host that are cold
    /// (slice KP, agent decision): the worst case, the other direct players
    /// who may host and whose class (platform and processor) is not its
    /// own, as many as there are standby roles. Which of the others stand
    /// by is only known when the game moves.
    fn cold_standbys_for(&self, order: u64) -> usize {
        let Some(class) = self.candidate_of(order).map(|r| (r.platform, r.processor)) else {
            return 0;
        };
        self.live_peers()
            .filter(|(_, p)| p.lobby.order != order && p.path != Path::Relay)
            .filter_map(|(_, p)| self.candidate_of(p.lobby.order))
            .filter(|r| r.may_host && (r.platform, r.processor) != class)
            .count()
            .min(limits::STANDBYS)
    }

    /// The machine that carried the most of `players`' need, its share in
    /// percent, and the lower snapshot rate its figure would fit.
    fn best_upload_figure(&self, players: usize) -> Option<(String, u32, Option<u32>)> {
        self.live_peers()
            .filter_map(|(_, p)| {
                let upload = self.upload_figure(p.lobby.order)?;
                (p.house || usize::from(upload.players) >= players).then(|| {
                    (
                        p.callsign.clone(),
                        u32::from(upload.per_mille) / 10,
                        self.lower_rate_that_fits(p.lobby.order, upload),
                    )
                })
            })
            .max_by_key(|(_, percent, _)| *percent)
    }

    /// The highest snapshot rate the King offers below the one in force that
    /// `upload`, measured at the one in force, would fit for the players it
    /// was measured for (slice R1): what arrived of the need at this rate
    /// against the need at the lower one, which counts the standbys' share
    /// as it is, so it is no more hopeful than the real need.
    fn lower_rate_that_fits(&self, order: u64, upload: Upload) -> Option<u32> {
        let rate = self.settings.snapshot_rate();
        let players = usize::from(upload.players);
        let cold = self.cold_standbys_for(order);
        let carried = u64::from(upload_need(players, cold, rate)) * u64::from(upload.per_mille);
        crate::settings::KING_SNAPSHOT_RATES
            .into_iter()
            .filter(|&lower| lower < rate)
            .find(|&lower| {
                carried >= u64::from(upload_need(players, cold, lower)) * u64::from(UPLOAD_PASS)
            })
    }

    fn upload_figure(&self, order: u64) -> Option<Upload> {
        let (_, peer) = self.peer_of_order(order)?;
        if peer.house {
            return self.succession.house_upload;
        }
        self.succession.measures.get(&order)?.upload
    }

    fn cpu_figure(&self, order: u64) -> Option<u32> {
        self.candidate_of(order)
            .map(|r| r.cpu_micros)
            .filter(|&micros| micros > 0)
    }

    pub(super) fn candidate_of(&self, order: u64) -> Option<&CandidateReport> {
        self.succession.measures.get(&order)?.report.as_ref()
    }

    // ----- Players -----------------------------------------------------

    /// Connections not closing.
    fn live_peers(&self) -> impl Iterator<Item = (ConnectionId, &Peer)> {
        self.peers
            .iter()
            .filter(|(_, p)| !matches!(p.stage, Stage::Closing { .. }))
            .map(|(c, p)| (*c, p))
    }

    /// The players whose reach counts: direct (not relayed), not the house.
    fn reach_testers(&self) -> impl Iterator<Item = (ConnectionId, &Peer)> {
        self.live_peers()
            .filter(|(_, p)| !p.house && p.path != Path::Relay)
    }

    fn join_order_of(&self, connection: ConnectionId) -> Option<u64> {
        self.peers.get(&connection).map(|p| p.lobby.order)
    }

    fn peer_of_order(&self, order: u64) -> Option<(ConnectionId, &Peer)> {
        self.live_peers().find(|(_, p)| p.lobby.order == order)
    }

    /// A line in the King's Messages.
    fn tell_king_line(&mut self, line: String) {
        let king = self.live_peers().find(|(_, p)| p.king).map(|(c, _)| c);
        if let Some(king) = king {
            self.send(king, &Message::Notice(line));
        }
    }
}
