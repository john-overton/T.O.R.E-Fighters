//! The client's side of host migration (stage K; docs/ARCHITECTURE.md, "On
//! the client"), built by slice K4: the succession, detection, the race to
//! the new host, Resume, Resumed and Backlog, and the 10 seconds of inputs
//! kept for it. The standby stream's records are kept for the game's
//! standby (slice K0).
//!
//! - **The succession** (message 45) names the ready standbys and their
//!   addresses; a client keeps the newest.
//! - **Detection.** [`DETECT_SILENCE`] without a packet from the host, or a
//!   Host moving (51), starts the **race**: from the same socket, Connect
//!   requests to every address of every standby but this game's own, every
//!   250 ms, with the rejoin token in the Challenge answer. A relayed game
//!   races its relayed address again. A client with no ready standby to race
//!   waits out the transport's 5 seconds as before. The old host answering
//!   again before anyone else stops the race.
//! - **The prediction never stops**: the own aircraft flies on throughout,
//!   and the client keeps [`KEPT_TICKS`] of its predicted ticks (controls,
//!   commands and view) beside the predictor's 2 seconds.
//! - **Resuming.** Accepted must carry the session's id (any other ends the
//!   race for that address). The client sends Resume; Resumed gives a new
//!   flight and the plane's exact state as tick T begins. The client replays
//!   its kept ticks from T over that state, with the commands the host has
//!   not applied, and compares the result with its prediction: equal (the
//!   rule on one class), nothing changes; otherwise it takes the replay, as
//!   any correction. Then Backlog sends those ticks and commands, numbered
//!   afresh; the wire starts afresh with the new flight, and the clocks and
//!   the prediction stay.
//! - **Giving up.** With no new host [`GIVE_UP`] after the loss the
//!   connection ends: "No other game could take over. The host left the
//!   game."
//! - **A game that takes over** joins its own new host over the in-process
//!   link ([`Client::host_here`]) and tells the old host on the connection
//!   it still holds (Taken over, 52); an old host's own game races the
//!   new host when its host steps down ([`Client::move_to`]).

use super::prediction::Predictor;
use super::{Client, ClientEvent, ClientPhase, Correction, SEATING_SNAP, prediction};
use crate::host::Present;
use crate::standby::takeover::{DETECT_SILENCE, GIVE_UP, due};
use crate::wire::PROTOCOL_VERSION;
use crate::wire::inputs::{Command, InputFrame, NumberedCommand};
use crate::wire::messages::{Message, StandbyMark};
use crate::wire::migration::{
    Backlog, BacklogCommand, BacklogTick, HostMoving, Resume, Resumed, ResumedFlight, Succession,
    TakenOver, limits,
};
use std::collections::{BTreeSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tore_net::master::Path;
use tore_net::master::candidate::canonical;
use tore_net::{CloseReason, DisconnectReason, Entropy, Target, Transmit};
use tore_world::seats::SeatId;
use tore_world::world::plane::ExactState;

/// Predicted ticks a client keeps for its backlog: 10 seconds.
pub const KEPT_TICKS: usize = limits::BACKLOG_TICKS;
/// How long a game that took over keeps its connection to the old host, for
/// Taken over to get through.
pub const OLD_CONNECTION_GRACE: Duration = Duration::from_secs(2);

/// The words a player reads during a migration (John, 2026-10-05).
pub mod words {
    /// The host is lost and the game races its standbys.
    pub fn lost(callsign: &str) -> String {
        format!("Lost contact with the host. Moving the game to {callsign}...")
    }

    /// The game resumed with its new host.
    pub fn moved(callsign: &str) -> String {
        format!("The game moved to {callsign}.")
    }

    /// No new host answered.
    pub const GAVE_UP: &str = "No other game could take over. The host left the game.";
}

/// Where a migration is, for the screens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrationState {
    /// Joined to a host that answers.
    Steady,
    /// The host was lost at `since`: racing the succession, `to` the
    /// standby expected to take over.
    Racing { since: Duration, to: String },
    /// Joined to the new host, waiting for Resumed.
    Resuming,
}

/// What this game's migrations came to: telemetry's counts and the
/// figures of the tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MigrationCounts {
    /// Migrations resumed through.
    pub resumed: u64,
    /// Migrations the session was lost to.
    pub failed: u64,
    /// Resumes whose replay differed from the prediction: the own plane
    /// was corrected.
    pub corrected: u64,
}

/// One predicted tick kept for a backlog.
#[derive(Clone, Debug, PartialEq)]
struct KeptTick {
    tick: u64,
    frame: InputFrame,
    /// The host tick the screen showed, as ticks before this one, and the
    /// interpolation delay.
    view_offset: u8,
    delay: u8,
}

/// A race to a new host.
struct Race {
    net: tore_net::Client,
    /// Each target's standby (its lobby id, when the succession names it),
    /// for the words.
    owners: Vec<(SocketAddr, Option<u8>)>,
    /// This game took over and joins its own host.
    own: bool,
    /// Started by the host's silence: the old host answering again stops it.
    by_silence: bool,
}

/// The connection to the old host, kept while it tells it something.
struct Old {
    net: tore_net::Client,
    /// Disconnected (moved to the new host) by then at the latest.
    until: Duration,
    disconnected: bool,
}

/// The migration's state on the client, and the standby records waiting
/// for the game's standby (slice K2 reads them).
#[derive(Default)]
pub(super) struct Migration {
    /// Standby records as they arrived, in order, unread.
    records: VecDeque<Vec<u8>>,
    /// The newest Succession.
    succession: Option<Succession>,
    /// The newest snapshot tick and when it arrived: the old host's clock.
    newest: Option<(Duration, u32)>,
    kept: VecDeque<KeptTick>,
    race: Option<Race>,
    /// When the host was lost, while a migration runs.
    lost_at: Option<Duration>,
    /// The standby expected to take over, for the words.
    to: String,
    /// The old connection closed while racing: why, for when the race
    /// gives up.
    lost_reason: Option<CloseReason>,
    /// Addresses whose host was another session's.
    foreign: BTreeSet<SocketAddr>,
    old: Option<Old>,
    /// The host hands over to this standby.
    moving: Option<HostMoving>,
    /// Joined to a new host since the loss, and the standby that hosts.
    found: Option<String>,
    /// The game said its host moved ([`Client::move_to`]).
    moved: bool,
    /// Races started, for each race's own seed.
    races: u64,
    /// What the migrations came to.
    counts: MigrationCounts,
}

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migration")
            .field("records", &self.records.len())
            .field("succession", &self.succession)
            .field("lost_at", &self.lost_at)
            .field("racing", &self.race.is_some())
            .finish_non_exhaustive()
    }
}

impl Client {
    /// A message of stage K's migration from the host: Succession, Standby
    /// record, Resumed or Host moving.
    pub(super) fn migrate_message(&mut self, message: Message) {
        match message {
            Message::StandbyRecord(record) => self.migration.records.push_back(record),
            Message::Succession(succession) => {
                self.migration.succession =
                    (!succession.standbys.is_empty()).then_some(*succession);
            }
            Message::HostMoving(moving) => {
                self.log(
                    "migrate",
                    &[
                        "host-moving",
                        &moving.standby.to_string(),
                        &moving.last_tick.to_string(),
                    ],
                );
                let me = self.lobby.as_ref().map(|l| l.you);
                self.migration.moving = Some(moving);
                if me != Some(moving.standby) {
                    let now = self.now;
                    self.lose_host(now);
                }
            }
            Message::Resumed(resumed) => self.resumed(*resumed),
            _ => {}
        }
    }

    /// The standby stream's records since the last call, in the order the
    /// host sent them (stage K): what the game's standby replays. The client
    /// keeps them until they are taken, never reading them.
    pub fn take_standby_records(&mut self) -> Vec<Vec<u8>> {
        self.migration.records.drain(..).collect()
    }

    /// The newest Succession: the ready standbys and their addresses.
    pub fn succession(&self) -> Option<&Succession> {
        self.migration.succession.as_ref()
    }

    /// This game's role in the succession: first, second or none.
    pub fn standby_role(&self) -> StandbyMark {
        let Some(me) = self.lobby.as_ref().map(|l| l.you) else {
            return StandbyMark::None;
        };
        match self
            .migration
            .succession
            .as_ref()
            .and_then(|s| s.standbys.iter().position(|s| s.player == me))
        {
            Some(0) => StandbyMark::First,
            Some(_) => StandbyMark::Second,
            None => StandbyMark::None,
        }
    }

    /// How long the host has been silent, while joined.
    pub fn host_silence(&self) -> Option<Duration> {
        self.net.stats().map(|stats| stats.since_last_received)
    }

    /// Where a migration is, for the screens.
    pub fn migration(&self) -> MigrationState {
        match (self.migration.lost_at, &self.migration.race) {
            (Some(since), Some(_)) => MigrationState::Racing {
                since,
                to: self.migration.to.clone(),
            },
            (Some(_), None) => MigrationState::Resuming,
            (None, _) => MigrationState::Steady,
        }
    }

    /// What this game's migrations came to (telemetry's counts).
    pub fn migration_counts(&self) -> MigrationCounts {
        self.migration.counts
    }

    /// The old host's clock at `now` as this game estimates it: the next
    /// tick after the newest snapshot, carried forward from its arrival by
    /// the time since and half a round trip. What a standby that takes over
    /// steps to ([`crate::host::Present`]).
    pub fn present(&self, now: Duration) -> Option<Present> {
        let (at, tick) = self.migration.newest?;
        let half_trip = self
            .net_stats_round_trip()
            .map_or(0., |rtt| rtt.as_secs_f64() / 2.);
        let ahead =
            (now.saturating_sub(at).as_secs_f64() + half_trip) * super::clock::TICKS_PER_SECOND;
        Some(Present {
            at: now,
            tick: f64::from(tick) + 1. + ahead,
        })
    }

    fn net_stats_round_trip(&self) -> Option<Duration> {
        let net = self
            .migration
            .old
            .as_ref()
            .map_or(&self.net, |old| &old.net);
        net.stats().map(|stats| stats.round_trip)
    }

    /// Whether this game's standby should take the game over now
    /// ([`due`]): `ready` when the standby holds a world the journal
    /// continues from, `handover` when it holds the host's Handover record
    /// (the host's Host moving names it too, but only the record says the
    /// standby holds every tick the host stepped).
    pub fn takeover_due(&self, ready: bool, handover: bool) -> bool {
        let silence = self.host_silence().unwrap_or(Duration::ZERO);
        let answered =
            self.migration.found.is_some() || self.migration.race.as_ref().is_some_and(|r| r.own);
        due(self.standby_role(), silence, handover, answered, ready)
    }

    /// The old host's address while this game keeps its connection to it:
    /// what the game's peers router still hands the client.
    pub fn old_host(&self) -> Option<SocketAddr> {
        self.migration.old.as_ref().map(|old| old.net.server())
    }

    /// This game took the game over at tick `tick` (slice K4): it tells the
    /// old host so on the connection it holds (Taken over), and joins its
    /// own new host at `link` (the in-process link) to resume like every
    /// other player.
    pub fn host_here(&mut self, now: Duration, link: SocketAddr, tick: u64) {
        if let Some(me) = self.lobby.as_ref().map(|l| l.you) {
            self.send(&Message::TakenOver(TakenOver {
                new_host: me,
                tick: u32::try_from(tick).unwrap_or(u32::MAX),
            }));
        }
        if self.migration.lost_at.is_none() {
            self.migration.lost_at = Some(now);
        }
        self.migration.to = self.config.callsign.clone();
        self.migration.race = None;
        self.log("migrate", &["host-here", &tick.to_string()]);
        self.start_race(now, vec![(Target::typed(link), None)], true);
    }

    /// This game's host stepped down: another game hosts the session at one
    /// of `addresses` (the one that answered the old host's question, say).
    /// The client races them now, as after a loss.
    pub fn move_to(&mut self, now: Duration, addresses: &[SocketAddr]) {
        let owner = |address: SocketAddr| {
            self.migration.succession.as_ref().and_then(|s| {
                s.standbys
                    .iter()
                    .find(|s| s.addresses.iter().any(|c| c.address == address))
                    .map(|s| s.player)
            })
        };
        let targets: Vec<(Target, Option<u8>)> = addresses
            .iter()
            .take(tore_net::MAX_TARGETS)
            .map(|&address| (Target::typed(address), owner(address)))
            .collect();
        if targets.is_empty() {
            return;
        }
        let to = targets
            .iter()
            .find_map(|(_, id)| *id)
            .map_or_else(|| "another game".to_owned(), |id| self.callsign_of(id));
        if self.migration.lost_at.is_none() {
            self.migration.lost_at = Some(now);
            self.log("migrate", &["host-moved", &to]);
        }
        self.migration.to = to;
        self.migration.race = None;
        self.migration.moved = true;
        self.start_race(now, targets, false);
    }

    /// Routes a datagram for the race or the old connection (the hook at
    /// the start of `receive`). Returns whether it took it.
    pub(super) fn migrate_receive(
        &mut self,
        now: Duration,
        from: SocketAddr,
        datagram: &[u8],
    ) -> bool {
        if let Some(race) = &mut self.migration.race {
            let ours = race
                .net
                .targets()
                .iter()
                .any(|t| canonical(t.address) == canonical(from));
            if ours {
                race.net.receive(now, from, datagram);
                self.migrate_pump(now);
                return true;
            }
        }
        if let Some(old) = &mut self.migration.old
            && old.net.server() == from
            && from != self.net.server()
        {
            old.net.receive(now, from, datagram);
            while old.net.poll_event().is_some() {}
            return true;
        }
        false
    }

    /// The race's and the old connection's datagrams first (the hook in
    /// `poll_transmit`).
    pub(super) fn migrate_poll_transmit(&mut self) -> Option<Transmit> {
        if let Some(t) = self
            .migration
            .race
            .as_mut()
            .and_then(|race| race.net.poll_transmit())
        {
            return Some(t);
        }
        let old = self.migration.old.as_mut()?;
        let t = old.net.poll_transmit();
        if t.is_none() && old.disconnected {
            self.migration.old = None;
        }
        t
    }

    /// The old connection closed while a migration runs (the hook in
    /// `pump`): kept quiet until the race succeeds or gives up. Returns
    /// whether it was.
    pub(super) fn migrate_closed(&mut self, reason: &CloseReason) -> bool {
        if self.migration.lost_at.is_some() && self.migration.found.is_none() {
            self.migration.lost_reason = Some(reason.clone());
            return true;
        }
        false
    }

    /// Notes the predicted ticks stepped since the last call, for the
    /// backlog (the hook after the prediction steps).
    pub(super) fn migrate_note_flight(&mut self) {
        let Some(seat) = &self.seat else {
            self.migration.kept.clear();
            return;
        };
        let render = self.render_clock.render();
        let delay = self.render_clock.delay().round().clamp(0., 63.) as u8;
        let kept = &mut self.migration.kept;
        let history = seat.predictor.history();
        let last = kept.back().map(|k| k.tick);
        // A gap (the prediction adopted a state ahead of it, or the kept
        // ticks were cleared) starts the kept run again.
        if let (Some(last), Some(first)) = (last, history.front().map(|r| r.tick))
            && (first > last + 1 || seat.predictor.tick() < last)
        {
            kept.clear();
        }
        let last = kept.back().map(|k| k.tick);
        for record in history.iter().filter(|r| last.is_none_or(|l| r.tick > l)) {
            let view_offset = render.map_or(0, |render| {
                (record.tick as f64 - render.floor()).clamp(0., 255.) as u8
            });
            kept.push_back(KeptTick {
                tick: record.tick,
                frame: record.frame,
                view_offset,
                delay,
            });
        }
        while kept.len() > KEPT_TICKS {
            kept.pop_front();
        }
    }

    /// The migration's timers at `now` (the hook after the transport's):
    /// the snapshot clock, detection, the race, the old connection and
    /// giving up.
    pub(super) fn migrate_update(&mut self, now: Duration) {
        if let Some(newest) = self.snapshot_tick
            && self.migration.newest.is_none_or(|(_, tick)| tick < newest)
        {
            self.migration.newest = Some((now, newest));
        }
        if let Some(old) = &mut self.migration.old {
            old.net.update(now);
            while old.net.poll_event().is_some() {}
            let through = old
                .net
                .stats()
                .is_none_or(|stats| stats.messages_queued == 0);
            if !old.disconnected && (through || now >= old.until) {
                old.net.disconnect(DisconnectReason::MovedToNewHost);
                old.disconnected = true;
            }
        }
        if matches!(self.phase, ClientPhase::Connecting | ClientPhase::Closed) {
            return;
        }
        let silence = self.host_silence().unwrap_or(Duration::ZERO);
        match self.migration.lost_at {
            None if silence >= DETECT_SILENCE => self.lose_host(now),
            // The old host answers again before anyone else: the race stops.
            Some(_)
                if silence < DETECT_SILENCE
                    && self.migration.found.is_none()
                    && self.migration.race.as_ref().is_some_and(|r| r.by_silence)
                    && self.migration.lost_reason.is_none() =>
            {
                self.log("migrate", &["old-host-back"]);
                self.migration.race = None;
                self.migration.lost_at = None;
            }
            _ => {}
        }
        if let Some(race) = &mut self.migration.race {
            race.net.update(now);
        }
        self.migrate_pump(now);
        if let Some(lost) = self.migration.lost_at
            && self.migration.found.is_none()
            && now.saturating_sub(lost) >= GIVE_UP
        {
            self.give_up();
        }
    }

    /// The host is lost (or hands over): race every standby's addresses but
    /// this game's own. Nothing to race leaves the transport's timeout to
    /// end the connection, as before stage K.
    fn lose_host(&mut self, now: Duration) {
        if self.migration.lost_at.is_some() && self.migration.race.is_some() {
            return;
        }
        let me = self.lobby.as_ref().map(|l| l.you);
        let mut targets: Vec<(Target, Option<u8>)> = Vec::new();
        if self.net.path() == Path::Relay {
            // A relayed game keeps its channel: the master forwards to the
            // new host once the listing has moved.
            let to = self
                .migration
                .succession
                .as_ref()
                .and_then(|s| s.standbys.iter().find(|s| Some(s.player) != me))
                .map(|s| s.player);
            targets.push((Target::new(self.net.server(), Path::Relay), to));
        } else if let Some(succession) = &self.migration.succession {
            for standby in succession.standbys.iter().filter(|s| Some(s.player) != me) {
                for candidate in &standby.addresses {
                    let address = candidate.address;
                    if self.migration.foreign.contains(&canonical(address))
                        || targets.len() >= tore_net::MAX_TARGETS
                    {
                        continue;
                    }
                    targets.push((Target::typed(address), Some(standby.player)));
                }
            }
        }
        if targets.is_empty() {
            return;
        }
        // The words name the standby expected to take over: the one the host
        // hands over to, else the succession's first, which is this game
        // when it stands by first (its own address is not in the race).
        let first = self
            .migration
            .moving
            .map(|m| m.standby)
            .or_else(|| {
                self.migration
                    .succession
                    .as_ref()
                    .and_then(|s| s.standbys.first())
                    .map(|s| s.player)
            })
            .or(targets[0].1);
        let to = first.map_or_else(|| "another game".to_owned(), |id| self.callsign_of(id));
        let fresh = self.migration.lost_at.is_none();
        self.migration.lost_at.get_or_insert(now);
        self.migration.to = to.clone();
        if fresh {
            self.log("migrate", &["host-lost", &to]);
            self.event(ClientEvent::Notice(words::lost(&to)));
        }
        self.start_race(now, targets, false);
    }

    /// The callsign of the player with lobby id `id`, as the lobby last
    /// named it.
    fn callsign_of(&self, id: u8) -> String {
        self.lobby
            .as_ref()
            .and_then(|l| l.players.iter().find(|p| p.id == id))
            .map_or_else(|| format!("player {id}"), |p| p.callsign.clone())
    }

    /// Starts a race to `targets` with this game's token.
    fn start_race(&mut self, now: Duration, targets: Vec<(Target, Option<u8>)>, own: bool) {
        self.migration.races += 1;
        let token = self
            .rejoin
            .kept
            .as_ref()
            .filter(|kept| kept.session_id == self.rejoin.session_id)
            .map(|kept| kept.token);
        let config = tore_net::ClientConfig {
            game_version: self.config.build.version.clone(),
            game_commit: self.config.build.commit.clone(),
            password: self.config.password.clone(),
            platform: self.config.platform,
            token,
            entropy: Entropy::Seeded(self.seed ^ 0x6d69_6772_6174_6500 ^ self.migration.races),
            max_section_kind: crate::wire::SECTION_FILLER,
            ..tore_net::ClientConfig::new(PROTOCOL_VERSION, &self.config.callsign)
        };
        let list: Vec<Target> = targets.iter().map(|(t, _)| *t).collect();
        let owners = targets.iter().map(|(t, id)| (t.address, *id)).collect();
        match tore_net::Client::connect_any(config, &list, None, now) {
            Ok(net) => {
                let by_silence = !own && self.migration.moving.is_none() && !self.migration.moved;
                self.migration.race = Some(Race {
                    net,
                    owners,
                    own,
                    by_silence,
                });
            }
            Err(error) => self.log("migrate", &["race-failed", &error.to_string()]),
        }
    }

    /// The race's events: a new host found, or an address that failed.
    fn migrate_pump(&mut self, now: Duration) {
        loop {
            let Some(race) = &mut self.migration.race else {
                return;
            };
            let Some(event) = race.net.poll_event() else {
                return;
            };
            match event {
                tore_net::ClientEvent::Connected(welcome) => {
                    if welcome.session_id != self.rejoin.session_id {
                        // Another session's host there: not this one.
                        let server = race.net.server();
                        self.migration.foreign.insert(canonical(server));
                        self.log("migrate", &["foreign", &server.to_string()]);
                        self.migration.race = None;
                        self.lose_host(now);
                        return;
                    }
                    self.switch(now);
                }
                tore_net::ClientEvent::Closed(reason) => {
                    let own = race.own;
                    self.log("migrate", &["race-ended", &super::describe(&reason)]);
                    self.migration.race = None;
                    if own {
                        return;
                    }
                    if self
                        .migration
                        .lost_at
                        .is_some_and(|lost| now.saturating_sub(lost) < GIVE_UP)
                    {
                        self.lose_host(now);
                    }
                    return;
                }
                tore_net::ClientEvent::Connection(_) => {}
            }
        }
    }

    /// The race found the new host: its connection is the client's now, the
    /// old one tells the old host it moved, and Resume goes out.
    fn switch(&mut self, now: Duration) {
        let Some(race) = self.migration.race.take() else {
            return;
        };
        let server = race.net.server();
        let owner = race
            .owners
            .iter()
            .find(|(address, _)| canonical(*address) == canonical(server))
            .and_then(|(_, id)| *id);
        let to = if race.own {
            self.config.callsign.clone()
        } else {
            owner.map_or_else(|| self.migration.to.clone(), |id| self.callsign_of(id))
        };
        let old = std::mem::replace(&mut self.net, race.net);
        // The old connection: a game that took over keeps it a moment for
        // Taken over; any other tells the old host at once that it moved.
        let mut old = Old {
            net: old,
            until: now
                + if race.own {
                    OLD_CONNECTION_GRACE
                } else {
                    Duration::ZERO
                },
            disconnected: false,
        };
        if !race.own {
            old.net.disconnect(DisconnectReason::MovedToNewHost);
            old.disconnected = true;
        }
        self.migration.old = Some(old);
        self.migration.found = Some(to.clone());
        self.migration.succession = None;
        self.migration.lost_reason = None;
        let after = self
            .migration
            .lost_at
            .map_or(Duration::ZERO, |lost| now.saturating_sub(lost));
        self.log(
            "migrate",
            &[
                "joined",
                &server.to_string(),
                &format!("{:.0}ms", after.as_secs_f64() * 1000.),
            ],
        );
        let flight = self.wire.as_ref().and_then(|wire| wire.flight).unwrap_or(0);
        // A new connection's wire: nothing of the old one's baselines, names
        // or events applies to the new host's sections.
        self.wire = Some(crate::wire::connection::ClientConnection::new(
            self.ticks_per_snapshot,
        ));
        let newest_tick = match (&self.seat, self.phase) {
            (Some(seat), ClientPhase::Flying) => u32::try_from(seat.predictor.tick()).unwrap_or(0),
            _ => 0,
        };
        let mission_hash = self
            .spec
            .as_ref()
            .map_or(0, |spec| tore_codec::fnv1a64(spec.to_text().as_bytes()));
        let resume = Resume {
            flight,
            newest_tick,
            mission: self.loaded.unwrap_or(0),
            mission_hash,
            watching: self.watching.is_some(),
        };
        self.send(&Message::Resume(resume));
    }

    /// The new host's answer (message 49).
    fn resumed(&mut self, resumed: Resumed) {
        let found = self.migration.found.take().unwrap_or_default();
        // The new host's lobby names its house when it has come.
        let to = self
            .lobby
            .as_ref()
            .and_then(|l| l.host)
            .filter(|_| self.migration.moved || found == "another game")
            .map_or(found, |id| self.callsign_of(id));
        let lost_at = self.migration.lost_at.take();
        self.migration.moving = None;
        self.migration.moved = false;
        match resumed {
            Resumed::NotFlying => {
                if self.seat.is_some() {
                    self.end_flight();
                }
                // An observer's watch starts again.
                if let Some(watching) = self.watching.take() {
                    self.watch(watching.subject);
                }
            }
            Resumed::Flying(flight) => self.resume_flight(flight),
        }
        if lost_at.is_some() {
            self.migration.counts.resumed += 1;
            let now = self.now;
            let after = lost_at.map_or(Duration::ZERO, |lost| now.saturating_sub(lost));
            self.log(
                "migrate",
                &[
                    "resumed",
                    &to,
                    &format!("{:.0}ms", after.as_secs_f64() * 1000.),
                ],
            );
            if !to.is_empty() && to != self.config.callsign {
                self.event(ClientEvent::Notice(words::moved(&to)));
            }
        }
    }

    /// Resumed with a plane: the new flight's wire, the replay from T
    /// compared with the prediction, and the backlog.
    fn resume_flight(&mut self, resumed: ResumedFlight) {
        let tick = u64::from(resumed.tick);
        let backlog = |ticks: Vec<BacklogTick>, commands: Vec<BacklogCommand>| {
            Message::Backlog(Box::new(Backlog {
                flight: resumed.flight,
                first_tick: resumed.tick,
                ticks,
                commands,
            }))
        };
        self.begin_flight(resumed.flight);
        for &object in &resumed.destroyed {
            self.destroyed.insert(object, 0);
        }
        let mismatch = self
            .mission
            .as_ref()
            .and_then(|mission| super::surface_mismatch(&mission.world, resumed.surface_digest));
        if let Some(reason) = mismatch {
            let message = backlog(Vec::new(), Vec::new());
            self.send(&message);
            self.refuse_surface(reason);
            return;
        }
        let (Some(seat), Some(mission)) = (self.seat.as_mut(), self.mission.as_ref()) else {
            // Not flying here: an empty backlog ends the host's wait.
            let message = backlog(Vec::new(), Vec::new());
            self.send(&message);
            return;
        };
        for &object in &resumed.destroyed {
            seat.predictor.destroyed(object);
        }
        let state = match ExactState::decode(&resumed.exact, None, &seat.model) {
            Ok(state) => state,
            Err(error) => {
                self.log("migrate", &["resume-failed", &error.to_string()]);
                self.net.disconnect(DisconnectReason::ProtocolError);
                return;
            }
        };
        let newest = seat.predictor.tick();
        // The kept ticks from T on, unbroken.
        let ticks: Vec<KeptTick> = {
            let from_t: Vec<&KeptTick> = self
                .migration
                .kept
                .iter()
                .filter(|k| k.tick >= tick && k.tick <= newest)
                .collect();
            let unbroken = from_t.first().is_some_and(|k| k.tick == tick)
                && from_t.len() as u64 == newest.saturating_sub(tick) + 1;
            if unbroken {
                from_t
                    .into_iter()
                    .take(limits::BACKLOG_TICKS)
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            }
        };
        // The commands the old host had not applied, at their ticks (at T at
        // the earliest), numbered afresh for the new flight.
        let applied = resumed.last_command;
        let last = tick + ticks.len().saturating_sub(1) as u64;
        let unapplied: Vec<NumberedCommand> = if ticks.is_empty() {
            Vec::new()
        } else {
            self.unacked
                .iter()
                .filter(|c| tore_net::sequence_newer(c.number, applied))
                .take(limits::BACKLOG_COMMANDS)
                .enumerate()
                .map(|(index, c)| NumberedCommand {
                    number: u16::try_from(index + 1).unwrap_or(u16::MAX),
                    tick: u32::try_from(u64::from(c.tick).clamp(tick, last)).unwrap_or(u32::MAX),
                    command: c.command,
                })
                .collect()
        };
        // The replay: the host's state as T begins, stepped through the kept
        // ticks with the commands the host will apply.
        let standing: BTreeSet<u32> = mission
            .ground
            .iter()
            .map(|pose| pose.id)
            .filter(|id| !self.destroyed.contains_key(id))
            .collect();
        let mut replay = Predictor::new(
            SeatId(resumed.seat),
            resumed.plane,
            tick.saturating_sub(1),
            state,
            Arc::clone(seat.predictor.config()),
            u64::from(self.ticks_per_snapshot),
            mission.start_seconds,
            standing,
        );
        for kept in &ticks {
            let commands: Vec<Command> = unapplied
                .iter()
                .filter(|c| u64::from(c.tick) == kept.tick)
                .map(|c| c.command)
                .collect();
            if let Err(error) = replay.step(kept.frame, commands, &mission.world.terrain) {
                self.log("migrate", &["replay-failed", &error.to_string()]);
                self.net.disconnect(DisconnectReason::Other(0));
                return;
            }
        }
        let same = replay.tick() == newest
            && seat.plane == resumed.plane
            && seat.seat.0 == resumed.seat
            && matches!(
                (replay.exact().hash(), seat.predictor.exact().hash()),
                (Ok(a), Ok(b)) if a == b
            );
        let now = self.now;
        let mut corrected = None;
        if !same {
            // As any correction: the plane restarts from the host's state
            // and the drawn one slides to it.
            let before = prediction::pose(&seat.predictor.plane().flight);
            let after = prediction::pose(&replay.plane().flight);
            let (feet, degrees) = prediction::difference(&before, &after);
            let seconds = now.as_secs_f64();
            let shown = if now.saturating_sub(seat.seated_at) < SEATING_SNAP {
                seat.offset.clear(seconds);
                false
            } else {
                seat.offset.correct(&before, &after, seconds)
            };
            if self.observed.is_some() {
                replay.trace_on();
            }
            seat.seat = SeatId(resumed.seat);
            seat.plane = resumed.plane;
            seat.predictor = replay;
            self.stats.corrections += 1;
            self.stats.corrections_shown += u64::from(shown);
            self.corrections.push(Correction {
                tick: tick.saturating_sub(1),
                now: newest,
                feet,
                degrees,
                shown,
            });
            corrected = Some(feet);
            self.migration.counts.corrected += 1;
        }
        let newest = seat.predictor.tick();
        seat.seated_tick = newest;
        if let Some(feet) = corrected {
            self.log("migrate", &["corrected", &format!("{feet:.1}ft")]);
        }
        let message = backlog(
            ticks
                .iter()
                .map(|k| BacklogTick {
                    frame: k.frame,
                    view_offset: k.view_offset,
                    interpolation_delay: k.delay,
                })
                .collect(),
            unapplied
                .iter()
                .map(|c| BacklogCommand {
                    offset: u32::try_from(u64::from(c.tick) - tick).unwrap_or(0),
                    command: c.command,
                })
                .collect(),
        );
        self.send(&message);
        self.next_command = u16::try_from(unapplied.len() + 1).unwrap_or(u16::MAX);
        self.unacked = unapplied.into_iter().collect();
        self.input_acked = u32::try_from(newest).unwrap_or(u32::MAX);
        self.sent_tick = newest;
        self.mismatch = 0;
        self.margin_input = 0;
        self.repeats_reported = 0;
        self.phase = ClientPhase::Flying;
    }

    /// No new host answered: the session is over.
    fn give_up(&mut self) {
        self.log("migrate", &["gave-up"]);
        self.migration.race = None;
        self.migration.lost_at = None;
        self.migration.counts.failed += 1;
        self.event(ClientEvent::Notice(words::GAVE_UP.into()));
        match self.migration.lost_reason.take() {
            // The old connection ended while the race ran: it ends now.
            Some(reason) => {
                if self.phase != ClientPhase::Closed {
                    self.phase = ClientPhase::Closed;
                    self.rejoin_closed();
                    self.event(ClientEvent::Closed(reason));
                }
            }
            // Still open (its host gone quiet): it closes as usual.
            None => self.net.disconnect(DisconnectReason::Timeout),
        }
    }
}
