//! The host session: one mission's `World` on a fixed 120 Hz clock, the
//! transport's server endpoint, and one record per connection (its seat, its
//! input buffer, its wire state and its event queue). See
//! docs/ARCHITECTURE.md, "The host session" and "Joining, leaving and the end
//! of a mission"; the dedicated server's rules are docs/DEDICATED-SERVER.md,
//! "The mission lifecycle".
//!
//! The host never reads a clock or touches a socket for the mission: the
//! caller passes the time in ([`Host::update`]), feeds it datagrams
//! ([`Host::receive`]) and sends what it gives ([`Host::poll_transmit`]).
//! Only the tick-cost statistic reads the process's clock, and nothing the
//! mission does depends on it.
//!
//! Each tick the host applies joins and departures as `MissionCommand::Take`
//! and `GiveBack`, takes every seat's input from its buffer, steps the world,
//! sorts the tick's output into each connection's events, notes what each
//! player's game could not foresee, and every few ticks sends each seated
//! player a snapshot and, when due, its plane's exact state.

pub mod config;
pub mod inputs;
mod sorting;
#[cfg(test)]
mod tests;

pub use config::{AfterEnd, BuildId, HostConfig, HostError, OpenPlanes, StartMode};
pub use sorting::{BURST_SLACK_TICKS, round_interval};

use crate::wire::connection::HostConnection;
use crate::wire::entity::{Entity, EntityKind};
use crate::wire::events::WireEvent;
use crate::wire::inputs::{InputFrame, InputsSection};
use crate::wire::messages::{
    self, Debrief, DebriefObjective, DebriefPilot, EndReason, Message, MissionEnded, PilotStatus,
    RosterPilot, RosterPlane, Seated,
};
use crate::wire::snapshot::SnapshotHeader;
use crate::wire::{PROTOCOL_VERSION, SECTION_INPUTS, SECTION_OWN_STATE, WireError, from_world};
use inputs::InputBuffer;
use sorting::{Timed, Tracker, Wide};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tore_net::{
    AcceptInfo, CloseReason, ConnectDetails, ConnectionId, Datagrams, Decision, DisconnectReason,
    Event, Gate, RefuseReason, Server, ServerConfig, ServerEvent, Transmit,
};
use tore_sim::ai::launch::Side;
use tore_sim::combat::live;
use tore_world::debrief;
use tore_world::mission::{LoadoutSpec, MissionSpec, StationLoad};
use tore_world::resources::{Manifest, ResourceReads};
use tore_world::seats::{Pilot, PlaneId, SeatId, SeatInput};
use tore_world::world::plane::{ExactState, OwnPlane, OwnshipTerms};
use tore_world::world::{Cue, MissionCommand, Seating, TickOutput, World};

/// Ticks a second, fixed.
pub const TICKS_PER_SECOND: u64 = 120;
/// Ticks the host runs at most in one update to catch up; beyond it the
/// backlog is dropped and noted as an overload. Simulated time is never
/// skipped: the mission runs slower than real time instead.
pub const MAX_CATCH_UP_TICKS: u64 = 30;
/// The Messages section's budget in a snapshot packet during flight.
pub const FLIGHT_MESSAGE_BUDGET: usize = 256;
/// A player's plane's exact state goes out at least this often (1 s).
pub const OWN_STATE_INTERVAL_TICKS: u64 = 120;
/// The longest the host holds a seat's exact states back for its Seated
/// message to be acknowledged, ticks (3 seconds): a lost fragment is sent
/// again after about a round trip, but a message queue that never empties
/// must not hold them for good.
pub const SEATED_HOLD_TICKS: u64 = 360;
/// How long a departing connection has for its last messages (the debrief,
/// Mission ended) to be acknowledged before the host disconnects it anyway.
pub const CLOSE_GRACE: Duration = Duration::from_secs(5);
/// The longest [`Host::next_wake`] when no tick is due: often enough for the
/// transport's keepalives and the lifecycle's timers.
pub const IDLE_WAKE: Duration = Duration::from_millis(10);
/// The seat ids a host gives out.
const SEAT_IDS: std::ops::RangeInclusive<u8> = 0..=254;

/// The time from tick 0 to tick `ticks` of a clock at 120 a second, to the
/// nanosecond (rounded up, so a tick is never due early).
fn ticks_time(ticks: u64) -> Duration {
    Duration::from_nanos(
        (u128::from(ticks) * 1_000_000_000).div_ceil(u128::from(TICKS_PER_SECOND)) as u64,
    )
}

/// Whole ticks of a 120 a second clock in `time`.
fn whole_ticks(time: Duration) -> u64 {
    (time.as_nanos() * u128::from(TICKS_PER_SECOND) / 1_000_000_000) as u64
}

/// Where the host's mission is in its lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Built, at tick 0, not flying and sending no snapshots, until the first
    /// player is seated or the console says `start now`.
    Waiting,
    /// Flying at 120 ticks a second.
    Flying,
    /// Ended: players get their debriefs and are disconnected. `next_in` is
    /// the time until the next mission, or `None` when the host stops after.
    Ended { next_in: Option<Duration> },
    /// Stopped: the caller may exit once it has transmitted.
    Stopped,
}

/// Why a player left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaveReason {
    /// The player ended the mission (Leave), or said so in its disconnect.
    Left,
    /// 5 seconds without a packet.
    Silent,
    /// The console's `kick`.
    Kicked,
    /// The mission ended.
    MissionEnded,
    /// A new join from the same address replaced the connection.
    Replaced,
    /// The connection ended for another reason.
    Disconnected(DisconnectReason),
}

/// Something for the console and the log, each with the host tick.
#[derive(Clone, Debug, PartialEq)]
pub enum HostLog {
    /// A join was accepted.
    Connected {
        tick: u64,
        address: SocketAddr,
        callsign: String,
    },
    /// A join was refused.
    Refused {
        tick: u64,
        address: SocketAddr,
        callsign: String,
        reason: String,
    },
    /// A player's content differs from the host's.
    ContentRefused {
        tick: u64,
        callsign: String,
        names: Vec<String>,
    },
    /// A player asked for a plane and did not get one.
    SeatRefused {
        tick: u64,
        callsign: String,
        reason: String,
    },
    /// A player took a plane.
    Seated {
        tick: u64,
        seat: u8,
        callsign: String,
        plane: u32,
    },
    /// A player left; its plane, if it had one, went back to the AI (or, when
    /// destroyed, stays as it is).
    Left {
        tick: u64,
        seat: Option<u8>,
        callsign: String,
        plane: Option<u32>,
        reason: LeaveReason,
    },
    /// The mission started flying.
    MissionStarted { tick: u64 },
    /// The mission ended.
    MissionEnded { tick: u64, reason: EndReason },
    /// A fresh copy of the mission was built.
    MissionRestarted { tick: u64 },
    /// The host fell behind real time by more than it catches up.
    Overloaded { tick: u64, ticks_behind: u64 },
    /// The mission could not go on (a step or a rebuild failed).
    Fault { tick: u64, text: String },
    /// The host stopped.
    Stopped { tick: u64 },
}

impl HostLog {
    /// The host tick of the entry.
    pub fn tick(&self) -> u64 {
        match self {
            Self::Connected { tick, .. }
            | Self::Refused { tick, .. }
            | Self::ContentRefused { tick, .. }
            | Self::SeatRefused { tick, .. }
            | Self::Seated { tick, .. }
            | Self::Left { tick, .. }
            | Self::MissionStarted { tick }
            | Self::MissionEnded { tick, .. }
            | Self::MissionRestarted { tick }
            | Self::Overloaded { tick, .. }
            | Self::Fault { tick, .. }
            | Self::Stopped { tick } => *tick,
        }
    }
}

/// Why a console command did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandError {
    /// No connected player has this seat.
    NoSuchSeat(u8),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchSeat(seat) => write!(f, "no player has seat {seat}"),
        }
    }
}

impl std::error::Error for CommandError {}

/// The host's figures for a status line.
#[derive(Clone, Debug, PartialEq)]
pub struct HostStatus {
    pub phase: Phase,
    /// The next tick the mission steps.
    pub tick: u64,
    /// Simulated time flown.
    pub mission_time: Duration,
    /// Players connected.
    pub players: usize,
    /// Players the host can seat: the lesser of the maximum and the open
    /// planes.
    pub capacity: usize,
    /// Aircraft in the mission.
    pub aircraft: usize,
    /// Mean and longest cost of a tick since the previous status.
    pub tick_cost_mean: Duration,
    pub tick_cost_max: Duration,
    /// The mean tick cost as a fraction of one core at 120 ticks a second
    /// (0.11 is 11 percent).
    pub load: f64,
    /// Overloads since the host started.
    pub overloads: u64,
    /// Bytes a second the host sends and receives, over every connection.
    pub bytes_up_per_second: u64,
    pub bytes_down_per_second: u64,
}

/// One player's figures for the `players` command and the log.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerStatus {
    pub seat: Option<u8>,
    pub callsign: String,
    pub address: SocketAddr,
    pub plane: Option<u32>,
    pub round_trip: Duration,
    /// Lost over sent, 0 to 1, of the host's packets; `None` before any was
    /// judged.
    pub loss: Option<f64>,
    /// The arrival spread of the player's input packets.
    pub spread: Duration,
    /// Ticks by which the player's inputs arrived before the host needed
    /// them, as last reported; `None` before seating.
    pub input_margin_ticks: Option<i32>,
    /// Ticks the host repeated the player's last input for.
    pub inputs_repeated: u64,
    /// Bytes a second the host sends the player, and receives from it.
    pub bytes_up_per_second: u64,
    pub bytes_down_per_second: u64,
}

/// Where a connection is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// Accepted and sent the mission: loading it, then asking for a plane.
    Loading,
    /// Takes `plane` from `seat` at the next tick.
    Taking { seat: SeatId, plane: PlaneId },
    /// Flies its plane.
    Seated,
    /// Left: its debrief is sent and its plane goes back at the next tick.
    Leaving,
    /// Done: disconnected with `reason` once its messages are acknowledged,
    /// or at `deadline`.
    Closing {
        deadline: Duration,
        reason: DisconnectReason,
    },
}

/// One connection.
struct Peer {
    address: SocketAddr,
    callsign: String,
    stage: Stage,
    /// The seat and plane while seated (and leaving).
    seat: Option<SeatId>,
    plane: Option<PlaneId>,
    wire: HostConnection,
    inputs: InputBuffer,
    /// This player's game cannot have foreseen its plane's state exactly
    /// since the last exact state went out.
    unforeseen: bool,
    /// The tick of the last exact state sent.
    last_own_state: u64,
    /// Seated was sent at this tick and its acknowledgement is awaited: no
    /// exact state goes out until it is (or [`SEATED_HOLD_TICKS`] pass), so
    /// none overtakes the Seated message.
    holding_since: Option<u64>,
    /// The newest mismatch already answered with an exact state.
    mismatch_answered: u32,
    /// The ownship terms the plane took last tick.
    terms: Option<OwnshipTerms>,
    /// The seat's picture at its last snapshot, for debris and pilots'
    /// velocities.
    picture: Option<tore_world::snapshot::RenderSnapshot>,
    /// The mission ended while it was connected.
    ended: bool,
}

/// The lifecycle's own state.
#[derive(Clone, Copy, Debug)]
enum Life {
    Waiting,
    Flying,
    Ended {
        /// When the next mission starts; `None` when the host stops.
        next_at: Option<Duration>,
        /// When the host stops regardless of connections still closing.
        stop_at: Duration,
    },
    Stopped,
}

/// Tick costs since the last status.
#[derive(Clone, Copy, Debug, Default)]
struct Costs {
    ticks: u64,
    total: Duration,
    max: Duration,
}

/// The host session. See the module documentation.
pub struct Host {
    config: HostConfig,
    spec: MissionSpec,
    spec_text: String,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    world: World,
    manifest: Manifest,
    server: Server,
    session_id: u64,
    peers: BTreeMap<ConnectionId, Peer>,
    /// Seats whose plane could not go back to the AI (destroyed, or its pilot
    /// gone) when their player left: the host flies them with neutral input,
    /// keeping the departed player's callsign for the roster.
    orphans: BTreeMap<SeatId, String>,
    /// Seats whose plane goes back to the AI at the next tick.
    gives: Vec<(SeatId, String)>,
    tracker: Tracker,
    life: Life,
    now: Duration,
    /// When tick 0 of the flying clock was due, and the ticks run since.
    origin: Option<Duration>,
    ticks_run: u64,
    ever_seated: bool,
    empty_since: Option<Duration>,
    roster_dirty: bool,
    logs: VecDeque<HostLog>,
    costs: Costs,
    overloads: u64,
    out: TickOutput,
    /// What each seat's game could not foresee, by tick (tests only).
    #[cfg(test)]
    unforeseen_log: Vec<(u64, SeatId, Unforeseen)>,
}

/// Why a seat's game could not foresee its plane's state at a tick, for the
/// network matrix's figures.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unforeseen {
    /// An input was late and repeated, or a command applied at another tick.
    LateInput,
    /// A change of the ownship terms (a release, fuel or stores) or an event
    /// about the plane (a hit, a blast, damage).
    Event,
}

fn build_world(
    spec: &MissionSpec,
    resources: &BTreeMap<String, Vec<u8>>,
) -> Result<(World, Manifest), HostError> {
    let reads = ResourceReads::new(resources);
    let world = World::new(spec, &reads, Seating::Open)
        .map_err(|error| HostError::Mission(error.to_string()))?;
    Ok((world, reads.manifest()))
}

fn session_id(entropy: tore_net::Entropy) -> u64 {
    use std::hash::BuildHasher;
    match entropy {
        tore_net::Entropy::System => std::collections::hash_map::RandomState::new().hash_one(0u8),
        tore_net::Entropy::Seeded(seed) => tore_net::SplitMix64::new(seed ^ 0x5e55_1011).next_u64(),
    }
}

/// `callsign` made unique among `taken`: `Viper`, then `Viper_2`, `Viper_3`,
/// the name shortened first so the whole stays within 15 characters.
pub fn unique_callsign<'a>(callsign: &str, taken: impl Iterator<Item = &'a str> + Clone) -> String {
    if !taken.clone().any(|t| t == callsign) {
        return callsign.to_owned();
    }
    for n in 2.. {
        let suffix = format!("_{n}");
        let keep = 15usize.saturating_sub(suffix.len());
        let base: String = callsign.chars().take(keep).collect();
        let candidate = format!("{base}{suffix}");
        if !taken.clone().any(|t| t == candidate) {
            return candidate;
        }
    }
    unreachable!("some suffix is free")
}

/// The wire's debrief for a seat's report.
pub fn debrief_message(report: &debrief::Report) -> Debrief {
    let pilot = |p: &debrief::Pilot| DebriefPilot {
        status: match p.status {
            debrief::Status::Alive => PilotStatus::Alive,
            debrief::Status::Ejected => PilotStatus::Ejected,
            debrief::Status::Dead => PilotStatus::Dead,
        },
        damage: p.damage,
        landing_grade: p.landing_grade,
        cause: p.cause.map(str::to_owned),
        kills: p.kills,
        friendly_fire: p.friendly_fire,
        air_to_air: p.air_to_air,
        air_to_ground: p.air_to_ground,
        gun: p.gun,
        bombs: p.bombs,
        enemy_aam: p.enemy_aam,
        enemy_sam: p.enemy_sam,
        enemy_gun: p.enemy_gun,
        enemy_aaa: p.enemy_aaa,
    };
    Debrief {
        success: report.outcome == debrief::Outcome::Success,
        objectives: report
            .objectives
            .iter()
            .map(|objective| match *objective {
                debrief::Objective::Destroy { destroyed, total } => {
                    DebriefObjective::Destroy { destroyed, total }
                }
                debrief::Objective::Protect { protected, total } => {
                    DebriefObjective::Protect { protected, total }
                }
            })
            .collect(),
        elapsed_seconds: report.elapsed_seconds,
        player: pilot(&report.player),
        wingman: report.wingman.as_ref().map(pilot),
    }
}

/// The transport's gate: the build, the password, the capacity and whether
/// the host takes joins at all.
struct HostGate {
    build: BuildId,
    password: Option<String>,
    /// Why joins are refused now, when they are.
    closed: Option<String>,
    players: usize,
    capacity: usize,
    accept: AcceptInfo,
    tick: u64,
    logs: Vec<HostLog>,
}

impl Gate for HostGate {
    fn accept(&mut self, details: &ConnectDetails) -> Decision {
        let refusal = if let Some(text) = &self.closed {
            Some((RefuseReason::ShuttingDown, text.clone()))
        } else if !self
            .build
            .matches(&details.game_version, &details.game_commit)
        {
            Some((
                RefuseReason::GameBuild,
                format!(
                    "The server runs version {} (commit {}); your game is {} (commit {}).",
                    self.build.version,
                    self.build.commit,
                    details.game_version,
                    details.game_commit
                ),
            ))
        } else if self
            .password
            .as_ref()
            .is_some_and(|password| *password != details.password)
        {
            Some((RefuseReason::WrongPassword, "Wrong password.".into()))
        } else if self.players >= self.capacity {
            Some((
                RefuseReason::ServerFull,
                format!("The server is full ({} players).", self.capacity),
            ))
        } else {
            None
        };
        match refusal {
            Some((reason, text)) => {
                self.logs.push(HostLog::Refused {
                    tick: self.tick,
                    address: details.address,
                    callsign: details.callsign.clone(),
                    reason: text.clone(),
                });
                Decision::Refuse { reason, text }
            }
            None => {
                self.players += 1;
                Decision::Accept(self.accept)
            }
        }
    }

    fn check_section(&mut self, _: ConnectionId, kind: u8, body: &[u8]) -> bool {
        HostConnection::check(kind, body)
    }
}

impl Host {
    /// A host for the mission `spec`, built from the imported `resources`,
    /// with `config`. Refuses a setting out of range, the retail stall-speed
    /// switch, and a mission the import cannot build or an open mission
    /// refuses.
    pub fn new(
        spec: MissionSpec,
        resources: Arc<BTreeMap<String, Vec<u8>>>,
        config: HostConfig,
    ) -> Result<Host, HostError> {
        config.validate()?;
        let (world, manifest) = build_world(&spec, &resources)?;
        let server = Server::new(ServerConfig {
            protocol_version: PROTOCOL_VERSION,
            max_connections: config.max_players,
            max_section_kind: tore_net::MAX_SECTION_KIND,
            entropy: config.entropy,
        });
        let tracker = Tracker::new(&world);
        let life = match config.start {
            StartMode::FirstPlayer => Life::Waiting,
            StartMode::Now => Life::Flying,
        };
        let mut host = Host {
            session_id: session_id(config.entropy),
            spec_text: spec.to_text(),
            spec,
            resources,
            world,
            manifest,
            server,
            peers: BTreeMap::new(),
            orphans: BTreeMap::new(),
            gives: Vec::new(),
            tracker,
            life,
            now: Duration::ZERO,
            origin: None,
            ticks_run: 0,
            ever_seated: false,
            empty_since: None,
            roster_dirty: false,
            logs: VecDeque::new(),
            costs: Costs::default(),
            overloads: 0,
            #[cfg(test)]
            unforeseen_log: Vec::new(),
            out: TickOutput::default(),
            config,
        };
        if matches!(host.life, Life::Flying) {
            host.log(HostLog::MissionStarted { tick: 0 });
        }
        Ok(host)
    }

    // ----- Driving -----------------------------------------------------

    /// Takes one datagram that arrived at `now` from `from`.
    pub fn receive(&mut self, now: Duration, from: SocketAddr, datagram: &[u8]) {
        self.now = self.now.max(now);
        let mut gate = self.gate();
        self.server.receive(now, from, datagram, &mut gate);
        let logs = std::mem::take(&mut gate.logs);
        self.logs.extend(logs);
        self.pump();
    }

    /// Reads every datagram waiting on `socket` (at most 1,024) and takes
    /// each, at `now`.
    pub fn receive_from<D: Datagrams + ?Sized>(
        &mut self,
        now: Duration,
        socket: &mut D,
    ) -> io::Result<()> {
        self.now = self.now.max(now);
        let mut gate = self.gate();
        let result = self.server.receive_from(socket, now, &mut gate).map(|_| ());
        let logs = std::mem::take(&mut gate.logs);
        self.logs.extend(logs);
        self.pump();
        result
    }

    /// Runs whatever is due at `now`: the ticks the clock owes (at most 30,
    /// then an overload note), each seated player's snapshots, the
    /// transport's timeouts and keepalives, and the lifecycle's timers.
    pub fn update(&mut self, now: Duration) {
        self.now = self.now.max(now);
        let now = self.now;
        self.server.update(now);
        self.pump();
        match self.life {
            Life::Flying => self.fly(now),
            Life::Ended { next_at, stop_at } => {
                if let Some(next_at) = next_at {
                    if now >= next_at {
                        self.next_mission();
                    }
                } else if self.peers.is_empty() || now >= stop_at {
                    self.stop();
                }
            }
            Life::Waiting | Life::Stopped => {}
        }
        self.close_departed(now);
        self.pump();
        if matches!(self.life, Life::Flying) {
            self.check_lifecycle(now);
        }
    }

    /// The next datagram to send, oldest first.
    pub fn poll_transmit(&mut self) -> Option<Transmit> {
        self.server.poll_transmit()
    }

    /// Sends every queued datagram on `socket`.
    pub fn transmit<D: Datagrams + ?Sized>(&mut self, socket: &mut D) -> io::Result<()> {
        self.server.transmit(socket)
    }

    /// How long from `now` until [`Host::update`] has something to do: the
    /// next tick while flying, else at most [`IDLE_WAKE`].
    pub fn next_wake(&self, now: Duration) -> Duration {
        match (self.life, self.origin) {
            (Life::Flying, Some(origin)) => {
                let due = origin + ticks_time(self.ticks_run);
                due.saturating_sub(now).min(IDLE_WAKE)
            }
            (Life::Flying, None) => Duration::ZERO,
            _ => IDLE_WAKE,
        }
    }

    /// The next entry for the console and the log, oldest first.
    pub fn poll_log(&mut self) -> Option<HostLog> {
        self.logs.pop_front()
    }

    // ----- Console -----------------------------------------------------

    /// The console's `start now`: a waiting mission starts flying.
    pub fn start_now(&mut self) {
        if matches!(self.life, Life::Waiting) {
            self.start_flying();
        }
    }

    /// The console's `kick SEAT`: the player's plane goes back to the AI at
    /// the next tick and the player is disconnected, with no debrief.
    pub fn kick(&mut self, seat: u8) -> Result<(), CommandError> {
        let connection = self
            .peers
            .iter()
            .find(|(_, peer)| {
                peer.seat == Some(SeatId(seat))
                    || matches!(peer.stage, Stage::Taking { seat: s, .. } if s == SeatId(seat))
            })
            .map(|(id, _)| *id)
            .ok_or(CommandError::NoSuchSeat(seat))?;
        self.server.disconnect(connection, DisconnectReason::Kicked);
        self.pump();
        Ok(())
    }

    /// The console's `end`: every seated player gets "Mission ended" and
    /// their debrief and is disconnected; the next mission starts after the
    /// restart delay, or the host stops, as configured.
    pub fn end(&mut self) {
        let next = match self.config.after_end {
            AfterEnd::Restart => Some(self.config.restart_delay),
            AfterEnd::Quit => None,
        };
        self.end_mission(EndReason::EndedByServer, next);
    }

    /// The console's `restart`: ends the mission and starts it again at
    /// once.
    pub fn restart(&mut self) {
        self.end_mission(EndReason::EndedByServer, Some(Duration::ZERO));
        self.next_mission();
    }

    /// The console's `quit`: disconnects every player with "server stopping"
    /// and stops.
    pub fn stop(&mut self) {
        if matches!(self.life, Life::Stopped) {
            return;
        }
        for peer in self.peers.values_mut() {
            peer.ended = true;
        }
        self.server.disconnect_all(DisconnectReason::ServerStopping);
        self.pump();
        self.life = Life::Stopped;
        let tick = self.world.tick();
        self.log(HostLog::Stopped { tick });
    }

    /// Where the mission is in its lifecycle.
    pub fn phase(&self) -> Phase {
        match self.life {
            Life::Waiting => Phase::Waiting,
            Life::Flying => Phase::Flying,
            Life::Ended { next_at, .. } => Phase::Ended {
                next_in: next_at.map(|at| at.saturating_sub(self.now)),
            },
            Life::Stopped => Phase::Stopped,
        }
    }

    // ----- Statistics ----------------------------------------------------

    /// The host's figures now; the tick costs cover the ticks since the
    /// previous call.
    pub fn status(&mut self, now: Duration) -> HostStatus {
        self.now = self.now.max(now);
        let costs = std::mem::take(&mut self.costs);
        let mean = if costs.ticks == 0 {
            Duration::ZERO
        } else {
            costs.total / costs.ticks as u32
        };
        let (mut up, mut down) = (0, 0);
        for id in self.peers.keys() {
            if let Some(stats) = self.server.stats(*id) {
                up += stats.bytes_sent_per_second;
                down += stats.bytes_received_per_second;
            }
        }
        HostStatus {
            phase: self.phase(),
            tick: self.world.tick(),
            mission_time: ticks_time(self.world.tick()),
            players: self.peers.len(),
            capacity: self.capacity(),
            aircraft: self.world.roster.planes().len(),
            tick_cost_mean: mean,
            tick_cost_max: costs.max,
            load: mean.as_secs_f64() * TICKS_PER_SECOND as f64,
            overloads: self.overloads,
            bytes_up_per_second: up,
            bytes_down_per_second: down,
        }
    }

    /// Every connected player's figures.
    pub fn players(&self) -> Vec<PlayerStatus> {
        self.peers
            .iter()
            .map(|(id, peer)| {
                let stats = self.server.stats(*id);
                let seated = peer.seat.is_some();
                PlayerStatus {
                    seat: peer.seat.map(|s| s.0),
                    callsign: peer.callsign.clone(),
                    address: peer.address,
                    plane: peer.plane.map(|p| p.0),
                    round_trip: stats.as_ref().map_or(Duration::ZERO, |s| s.round_trip),
                    loss: stats.as_ref().and_then(|s| s.loss),
                    spread: stats.as_ref().map_or(Duration::ZERO, |s| s.spread),
                    input_margin_ticks: seated.then(|| i32::from(peer.inputs.margin())),
                    inputs_repeated: peer.inputs.repeats_total(),
                    bytes_up_per_second: stats.as_ref().map_or(0, |s| s.bytes_sent_per_second),
                    bytes_down_per_second: stats
                        .as_ref()
                        .map_or(0, |s| s.bytes_received_per_second),
                }
            })
            .collect()
    }

    /// What each seat's game could not foresee, as (tick, seat, why).
    #[cfg(test)]
    pub(crate) fn unforeseen_log(&self) -> &[(u64, SeatId, Unforeseen)] {
        &self.unforeseen_log
    }

    /// The mission's world.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// The mission.
    pub fn spec(&self) -> &MissionSpec {
        &self.spec
    }

    /// The content manifest: every resource the mission's build read.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The settings.
    pub fn config(&self) -> &HostConfig {
        &self.config
    }

    /// The transport's counters of datagrams dropped before a connection.
    pub fn counters(&self) -> &tore_net::Counters {
        self.server.counters()
    }

    // ----- Joins and messages --------------------------------------------

    fn log(&mut self, entry: HostLog) {
        self.logs.push_back(entry);
    }

    /// Players the host can seat: the lesser of the maximum and the open
    /// planes.
    fn capacity(&self) -> usize {
        let open = self
            .world
            .roster
            .planes()
            .iter()
            .filter(|plane| self.open(plane.id))
            .count();
        open.min(self.config.max_players)
    }

    fn open(&self, plane: PlaneId) -> bool {
        self.world.roster.plane(plane).is_some_and(|p| {
            self.config
                .open_planes
                .allows(plane.0, p.slot.wing.side == Side::Friendly)
        })
    }

    fn gate(&self) -> HostGate {
        let closed = match self.life {
            Life::Waiting | Life::Flying => None,
            Life::Ended {
                next_at: Some(at), ..
            } => Some(format!(
                "The mission has ended; the next starts in {} seconds.",
                at.saturating_sub(self.now).as_secs()
            )),
            Life::Ended { next_at: None, .. } | Life::Stopped => {
                Some("The server is stopping.".into())
            }
        };
        HostGate {
            build: self.config.build.clone(),
            password: self.config.password.clone(),
            closed,
            // A departing connection's place is free already.
            players: self
                .peers
                .values()
                .filter(|peer| !matches!(peer.stage, Stage::Closing { .. }))
                .count(),
            capacity: self.capacity(),
            accept: AcceptInfo {
                session_id: self.session_id,
                ticks_per_second: TICKS_PER_SECOND as u8,
                ticks_per_snapshot: self.config.ticks_per_snapshot() as u8,
                host_tick: self.world.tick() as u32,
            },
            tick: self.world.tick(),
            logs: Vec::new(),
        }
    }

    /// Queues `message` to `connection`; a queue the transport refuses ends
    /// the connection.
    fn send(&mut self, connection: ConnectionId, message: &Message) {
        let sent = message.encode().map_err(|_| ()).and_then(|body| {
            self.server
                .send_message(connection, message.kind(), &body)
                .map_err(|_| ())
        });
        if sent.is_err() {
            self.server
                .disconnect(connection, DisconnectReason::ProtocolError);
        }
    }

    /// Handles every transport event.
    fn pump(&mut self) {
        while let Some(event) = self.server.poll_event() {
            match event {
                ServerEvent::Connected {
                    connection,
                    details,
                } => self.connected(connection, details),
                ServerEvent::Closed {
                    connection, reason, ..
                } => self.closed(connection, reason),
                ServerEvent::Connection { connection, event } => match event {
                    Event::Message { kind, body } => self.message(connection, kind, &body),
                    Event::Payload { sections, .. } => {
                        for section in sections {
                            if section.kind == SECTION_INPUTS {
                                self.inputs(connection, &section.body);
                            }
                        }
                    }
                    Event::Delivered { sequence } => {
                        if let Some(peer) = self.peers.get_mut(&connection) {
                            peer.wire.delivered(sequence);
                        }
                    }
                    Event::Lost { sequence } => {
                        if let Some(peer) = self.peers.get_mut(&connection) {
                            peer.wire.lost(sequence);
                        }
                    }
                },
            }
        }
    }

    fn connected(&mut self, connection: ConnectionId, details: ConnectDetails) {
        let callsign = unique_callsign(
            &details.callsign,
            self.peers.values().map(|p| p.callsign.as_str()),
        );
        self.peers.insert(
            connection,
            Peer {
                address: details.address,
                callsign: callsign.clone(),
                stage: Stage::Loading,
                seat: None,
                plane: None,
                wire: HostConnection::new(self.config.ticks_per_snapshot()),
                inputs: InputBuffer::new(),
                unforeseen: false,
                last_own_state: 0,
                holding_since: None,
                mismatch_answered: 0,
                terms: None,
                picture: None,
                ended: false,
            },
        );
        let tick = self.world.tick();
        self.log(HostLog::Connected {
            tick,
            address: details.address,
            callsign,
        });
        let mission = Message::Mission(messages::Mission {
            spec: self.spec_text.clone(),
            manifest: self.manifest.clone(),
            host_tick: tick as u32,
            contrail_sortie: self.world.combat.contrail_sortie(),
        });
        self.send(connection, &mission);
        let roster = Message::Roster(self.roster());
        self.send(connection, &roster);
    }

    fn closed(&mut self, connection: ConnectionId, reason: CloseReason) {
        let Some(peer) = self.peers.remove(&connection) else {
            return;
        };
        let reason = if peer.ended {
            LeaveReason::MissionEnded
        } else {
            match reason {
                CloseReason::Replaced => LeaveReason::Replaced,
                CloseReason::Disconnected { reason, .. } => match reason {
                    DisconnectReason::Left => LeaveReason::Left,
                    DisconnectReason::Timeout => LeaveReason::Silent,
                    DisconnectReason::Kicked => LeaveReason::Kicked,
                    other => LeaveReason::Disconnected(other),
                },
                CloseReason::NoAnswer | CloseReason::Refused { .. } => {
                    LeaveReason::Disconnected(DisconnectReason::Other(0))
                }
            }
        };
        // A seated player's plane goes back to the AI at the next tick, with
        // no debrief; a leaving one's is on its way already.
        if peer.stage == Stage::Seated
            && let Some(seat) = peer.seat
        {
            self.gives.push((seat, peer.callsign.clone()));
        }
        let (seat, plane) = (peer.seat, peer.plane);
        let tick = self.world.tick();
        self.log(HostLog::Left {
            tick,
            seat: seat.map(|s| s.0),
            callsign: peer.callsign,
            plane: plane.map(|p| p.0),
            reason,
        });
    }

    fn message(&mut self, connection: ConnectionId, kind: u8, body: &[u8]) {
        let Ok(message) = Message::decode(kind, body) else {
            self.server
                .disconnect(connection, DisconnectReason::ProtocolError);
            return;
        };
        match message {
            Message::Ready(ready) => self.ready(connection, ready.plane),
            Message::Leave => self.leave(connection),
            Message::ContentRefused(refused) => {
                let callsign = self
                    .peers
                    .get(&connection)
                    .map(|p| p.callsign.clone())
                    .unwrap_or_default();
                let tick = self.world.tick();
                self.log(HostLog::ContentRefused {
                    tick,
                    callsign,
                    names: refused.names,
                });
                self.server
                    .disconnect(connection, DisconnectReason::ContentMismatch);
            }
            // Host-to-client messages from a client break the protocol.
            _ => self
                .server
                .disconnect(connection, DisconnectReason::ProtocolError),
        }
    }

    fn inputs(&mut self, connection: ConnectionId, body: &[u8]) {
        let next = self.world.tick();
        let Some(peer) = self.peers.get_mut(&connection) else {
            return;
        };
        if !matches!(peer.stage, Stage::Seated) {
            return;
        }
        let Ok(section) = InputsSection::decode(body) else {
            return;
        };
        let refused = peer.inputs.receive(&section, next).is_err();
        // The arrival spread, in the player's own time: the tick it sampled.
        let sent = ticks_time(u64::from(section.newest_tick));
        self.server.note_arrival(connection, sent, self.now);
        if refused {
            self.server
                .disconnect(connection, DisconnectReason::ProtocolError);
        }
    }

    /// Whether some other connection already takes or flies `plane`.
    fn reserved(&self, plane: PlaneId) -> bool {
        self.peers.values().any(|peer| {
            matches!(peer.stage, Stage::Taking { plane: p, .. } if p == plane)
                || (peer.plane == Some(plane) && !matches!(peer.stage, Stage::Closing { .. }))
        })
    }

    /// The lowest seat id no connection, pending take or departed player's
    /// plane holds.
    fn free_seat(&self) -> Option<SeatId> {
        let used: BTreeSet<SeatId> = self
            .peers
            .values()
            .filter_map(|peer| match peer.stage {
                Stage::Taking { seat, .. } => Some(seat),
                _ => peer.seat,
            })
            .chain(self.orphans.keys().copied())
            .chain(self.gives.iter().map(|(seat, _)| *seat))
            .collect();
        SEAT_IDS.map(SeatId).find(|seat| !used.contains(seat))
    }

    /// Why `plane` cannot be taken by `seat` now, if it cannot.
    fn refusal(&self, seat: SeatId, plane: PlaneId) -> Option<String> {
        let Some(entry) = self.world.roster.plane(plane) else {
            return Some(format!("There is no plane {}.", plane.0));
        };
        if !self.open(plane) {
            return Some(format!("Plane {} is not open to players.", plane.0));
        }
        if entry.pilot != Pilot::Ai || self.reserved(plane) {
            return Some(format!("Plane {} is flown by another player.", plane.0));
        }
        self.world
            .can_take(seat, plane)
            .err()
            .map(|_| format!("Plane {} is destroyed or has lost its pilot.", plane.0))
    }

    fn ready(&mut self, connection: ConnectionId, wanted: Option<u32>) {
        if !matches!(self.life, Life::Waiting | Life::Flying) {
            return;
        }
        let Some(peer) = self.peers.get(&connection) else {
            return;
        };
        if peer.stage != Stage::Loading {
            return;
        }
        let callsign = peer.callsign.clone();
        let Some(seat) = self.free_seat() else {
            self.refuse_seat(connection, callsign, "No seat is free.".into());
            return;
        };
        let chosen = match wanted {
            Some(plane) => match self.refusal(seat, PlaneId(plane)) {
                None => Ok(PlaneId(plane)),
                Some(reason) => Err(reason),
            },
            None => {
                // Friendly Wing 1's lead first: the roster is in wing order.
                let planes: Vec<(PlaneId, bool)> = self
                    .world
                    .roster
                    .planes()
                    .iter()
                    .map(|p| (p.id, p.slot.wing.side == Side::Friendly))
                    .collect();
                planes
                    .iter()
                    .filter(|(_, friendly)| *friendly)
                    .chain(planes.iter().filter(|(_, friendly)| !*friendly))
                    .map(|(id, _)| *id)
                    .find(|plane| self.refusal(seat, *plane).is_none())
                    .ok_or_else(|| "No plane is free.".to_string())
            }
        };
        match chosen {
            Ok(plane) => {
                if let Some(peer) = self.peers.get_mut(&connection) {
                    peer.stage = Stage::Taking { seat, plane };
                }
                if matches!(self.life, Life::Waiting) {
                    self.start_flying();
                }
            }
            Err(reason) => self.refuse_seat(connection, callsign, reason),
        }
    }

    fn refuse_seat(&mut self, connection: ConnectionId, callsign: String, reason: String) {
        self.send(connection, &Message::SeatRefused(reason.clone()));
        let tick = self.world.tick();
        self.log(HostLog::SeatRefused {
            tick,
            callsign,
            reason,
        });
    }

    /// The player ends the mission: its debrief now, its plane back to the
    /// AI at the next tick, then the disconnect.
    fn leave(&mut self, connection: ConnectionId) {
        let Some(peer) = self.peers.get(&connection) else {
            return;
        };
        match peer.stage {
            Stage::Seated => {
                let seat = peer.seat.expect("a seated player has a seat");
                let callsign = peer.callsign.clone();
                if let Some(report) = debrief::capture(&self.world, seat) {
                    let message = Message::Debrief(Box::new(debrief_message(&report)));
                    self.send(connection, &message);
                }
                self.gives.push((seat, callsign));
                if let Some(peer) = self.peers.get_mut(&connection) {
                    peer.stage = Stage::Leaving;
                }
            }
            Stage::Loading | Stage::Taking { .. } => {
                if let Some(peer) = self.peers.get_mut(&connection) {
                    peer.stage = Stage::Closing {
                        deadline: self.now + CLOSE_GRACE,
                        reason: DisconnectReason::Left,
                    };
                }
            }
            Stage::Leaving | Stage::Closing { .. } => {}
        }
    }

    /// Disconnects every departing connection whose last messages are
    /// acknowledged, or whose grace has run out.
    fn close_departed(&mut self, now: Duration) {
        let done: Vec<(ConnectionId, DisconnectReason)> = self
            .peers
            .iter()
            .filter_map(|(id, peer)| match peer.stage {
                Stage::Closing { deadline, reason } => {
                    let acknowledged = self
                        .server
                        .stats(*id)
                        .is_none_or(|stats| stats.messages_queued == 0);
                    (acknowledged || now >= deadline).then_some((*id, reason))
                }
                _ => None,
            })
            .collect();
        for (id, reason) in done {
            self.server.disconnect(id, reason);
        }
    }

    /// The roster message: every plane with its pilot.
    fn roster(&self) -> messages::Roster {
        let callsign = |seat: SeatId| {
            self.peers
                .values()
                .find(|peer| {
                    peer.seat == Some(seat)
                        || matches!(peer.stage, Stage::Taking { seat: s, .. } if s == seat)
                })
                .map(|peer| peer.callsign.clone())
                .or_else(|| self.orphans.get(&seat).cloned())
                .unwrap_or_default()
        };
        messages::Roster {
            planes: self
                .world
                .roster
                .planes()
                .iter()
                .filter_map(|plane| {
                    Some(RosterPlane {
                        id: plane.id.0,
                        wing: plane.slot.wing,
                        member: plane.slot.member,
                        aircraft: aircraft_of(&self.world, plane.id)?,
                        pilot: match plane.pilot {
                            Pilot::Ai => RosterPilot::Ai,
                            Pilot::Human(seat) => RosterPilot::Human {
                                seat: seat.0,
                                callsign: callsign(seat),
                            },
                        },
                    })
                })
                .collect(),
        }
    }

    fn broadcast_roster(&mut self) {
        let roster = Message::Roster(self.roster());
        let to: Vec<ConnectionId> = self
            .peers
            .iter()
            .filter(|(_, peer)| !matches!(peer.stage, Stage::Closing { .. }))
            .map(|(id, _)| *id)
            .collect();
        for id in to {
            self.send(id, &roster);
        }
    }

    // ----- The clock and the tick ----------------------------------------

    fn start_flying(&mut self) {
        self.life = Life::Flying;
        self.origin = None;
        self.ticks_run = 0;
        let tick = self.world.tick();
        self.log(HostLog::MissionStarted { tick });
    }

    /// Runs the ticks the clock owes at `now`.
    fn fly(&mut self, now: Duration) {
        let origin = *self.origin.get_or_insert(now);
        let owed = whole_ticks(now.saturating_sub(origin)) + 1;
        let behind = owed.saturating_sub(self.ticks_run);
        let run = behind.min(MAX_CATCH_UP_TICKS);
        for _ in 0..run {
            if !matches!(self.life, Life::Flying) {
                return;
            }
            self.ticks_run += 1;
            self.tick(now);
        }
        if behind > MAX_CATCH_UP_TICKS {
            let dropped = behind - MAX_CATCH_UP_TICKS;
            // The clock starts again from here: the backlog is not run.
            self.origin = Some(origin + ticks_time(dropped));
            self.overloads += 1;
            let tick = self.world.tick();
            self.log(HostLog::Overloaded {
                tick,
                ticks_behind: dropped,
            });
        }
    }

    /// One tick: handoffs, inputs, the step, the sorting, the snapshots.
    fn tick(&mut self, now: Duration) {
        let started = Instant::now();
        let tick = self.world.tick();
        let mut commands = Vec::new();

        // Planes going back to the AI; a plane that cannot (destroyed, or its
        // pilot gone) stays with its seat, flown with neutral input.
        let mut given = Vec::new();
        for (seat, callsign) in std::mem::take(&mut self.gives) {
            if self
                .world
                .roster
                .seat(seat)
                .is_none_or(|s| s.plane.is_none())
            {
                // Nothing to give back; a leaving player is done all the same.
                given.push(seat);
                continue;
            }
            if self.world.can_give_back(seat).is_ok() {
                commands.push(MissionCommand::GiveBack { seat });
                given.push(seat);
            } else {
                self.orphans.insert(seat, callsign);
                given.push(seat);
            }
        }

        // Planes taken this tick.
        let mut takes = Vec::new();
        let taking: Vec<(ConnectionId, SeatId, PlaneId)> = self
            .peers
            .iter()
            .filter_map(|(id, peer)| match peer.stage {
                Stage::Taking { seat, plane } => Some((*id, seat, plane)),
                _ => None,
            })
            .collect();
        for (connection, seat, plane) in taking {
            if self.world.can_take(seat, plane).is_ok() {
                commands.push(MissionCommand::Take { seat, plane });
                takes.push((connection, seat, plane));
            } else {
                let callsign = self.peers[&connection].callsign.clone();
                if let Some(peer) = self.peers.get_mut(&connection) {
                    peer.stage = Stage::Loading;
                }
                self.refuse_seat(
                    connection,
                    callsign,
                    format!("Plane {} is destroyed or has lost its pilot.", plane.0),
                );
            }
        }

        // Every flying seat's input.
        let mut inputs: Vec<SeatInput> = Vec::new();
        for peer in self.peers.values_mut() {
            if peer.stage != Stage::Seated {
                continue;
            }
            let Some(seat) = peer.seat else { continue };
            let (input, taken) = peer.inputs.take(seat, tick);
            peer.unforeseen |= taken.repeated || taken.command_moved;
            #[cfg(test)]
            if taken.repeated || taken.command_moved {
                self.unforeseen_log
                    .push((tick, seat, Unforeseen::LateInput));
            }
            inputs.push(input);
        }
        for &(_, seat, _) in &takes {
            inputs.push(InputFrame::default().seat_input(seat, tick, &[], None));
        }
        for &seat in self.orphans.keys() {
            if !inputs.iter().any(|input| input.seat == seat) {
                inputs.push(InputFrame::default().seat_input(seat, tick, &[], None));
            }
        }

        let mut out = std::mem::take(&mut self.out);
        if let Err(error) = self
            .world
            .step_with(&commands, &inputs, &mut out, |_, _| Ok(()))
        {
            self.out = out;
            self.fault(format!("tick {tick}: {error}"));
            return;
        }

        if !given.is_empty() || !takes.is_empty() {
            self.roster_dirty = true;
        }
        // Players whose plane went back are done: disconnect them once their
        // debrief is acknowledged.
        for seat in &given {
            for peer in self.peers.values_mut() {
                if peer.seat == Some(*seat) && peer.stage == Stage::Leaving {
                    peer.stage = Stage::Closing {
                        deadline: now + CLOSE_GRACE,
                        reason: DisconnectReason::Left,
                    };
                }
            }
        }
        let wide = self.tracker.sort(&mut self.world, &out, tick);
        for (connection, seat, plane) in takes {
            self.seated(connection, seat, plane, tick, &out);
        }
        self.sort_seats(tick, &out, &wide);
        if std::mem::take(&mut self.roster_dirty) {
            self.broadcast_roster();
        }
        self.snapshots(tick, now, &out);
        self.out = out;
        let cost = started.elapsed();
        self.costs.ticks += 1;
        self.costs.total += cost;
        self.costs.max = self.costs.max.max(cost);
    }

    /// A take went through at `tick`: the player gets its seat, its plane's
    /// exact state, its loadout, the roster and the destroyed ground objects,
    /// and the mission as it stands is queued for its first snapshot.
    fn seated(
        &mut self,
        connection: ConnectionId,
        seat: SeatId,
        plane: PlaneId,
        tick: u64,
        out: &TickOutput,
    ) {
        let Some(cockpit) = self.world.cockpits.iter().find(|c| c.plane == plane) else {
            return;
        };
        let terms = out.terms.iter().find(|(p, _)| *p == plane).map(|(_, t)| *t);
        let exact = ExactState::of(&OwnPlane::of(cockpit), terms.as_ref());
        let Ok(exact_bytes) = exact.encode(None) else {
            self.fault(format!("tick {tick}: plane {} has no exact state", plane.0));
            return;
        };
        let loadout = self
            .world
            .combat
            .state
            .ownship(plane.0)
            .map(|own| LoadoutSpec {
                fuel_lbs: cockpit.flight.fuel,
                cheat: false,
                stations: own
                    .configuration()
                    .stations
                    .iter()
                    .zip(&own.ammo)
                    .map(|(station, &quantity)| StationLoad {
                        weapon: station.weapon.source.clone(),
                        count: station.count,
                        quantity,
                    })
                    .collect(),
            })
            .unwrap_or(LoadoutSpec {
                fuel_lbs: cockpit.flight.fuel,
                cheat: false,
                stations: Vec::new(),
            });
        let Some(peer) = self.peers.get_mut(&connection) else {
            return;
        };
        peer.stage = Stage::Seated;
        peer.seat = Some(seat);
        peer.plane = Some(plane);
        peer.inputs = InputBuffer::new();
        peer.unforeseen = false;
        peer.last_own_state = tick;
        peer.holding_since = Some(tick);
        peer.terms = terms;
        peer.picture = None;
        let callsign = peer.callsign.clone();
        for Timed { tick, event } in Tracker::standing(&self.world, tick) {
            queue(peer, tick, &event);
        }
        self.server
            .set_message_budget(connection, FLIGHT_MESSAGE_BUDGET);
        self.ever_seated = true;
        self.empty_since = None;
        let seated = Message::Seated(Box::new(Seated {
            seat: seat.0,
            plane: plane.0,
            tick: tick as u32,
            exact: exact_bytes,
            loadout,
            roster: self.roster(),
            destroyed: self.tracker.destroyed().iter().copied().collect(),
        }));
        self.send(connection, &seated);
        self.log(HostLog::Seated {
            tick,
            seat: seat.0,
            callsign,
            plane: plane.0,
        });
    }

    /// Queues the tick's events for every seated player and notes what each
    /// one's game could not foresee.
    fn sort_seats(&mut self, tick: u64, out: &TickOutput, wide: &[Timed]) {
        let mut behind = Vec::new();
        for (id, peer) in self.peers.iter_mut() {
            if peer.stage != Stage::Seated {
                continue;
            }
            let (Some(seat), Some(plane)) = (peer.seat, peer.plane) else {
                continue;
            };
            let mut events: Vec<WireEvent> = Vec::new();
            let mut failed = false;
            for cue in &out.cues {
                // The seat's own explosion is its own event.
                if let Cue::Message { seat: s, text } = cue
                    && *s == seat
                    && let Some(on_impact) = exploded(text)
                {
                    events.push(WireEvent::YourAircraftExploded { on_impact });
                    continue;
                }
                match from_world::cue_event(cue, seat, &mut peer.wire.names) {
                    Ok(Some(event)) => events.push(event),
                    Ok(None) => {}
                    Err(_) => failed = true,
                }
            }
            for release in out.releases.iter().filter(|r| r.seat == seat) {
                match from_world::release_event(release, &mut peer.wire.names) {
                    Ok(event) => events.push(event),
                    Err(_) => failed = true,
                }
            }
            for reply in out.orders.iter().filter(|r| r.seat == seat) {
                events.push(from_world::order_event(reply));
            }
            for event in &events {
                failed |= peer.wire.event(tick as u32, event).is_err();
            }
            for timed in wide {
                failed |= !queue(peer, timed.tick, &timed.event);
            }
            if failed {
                behind.push(*id);
            }
            // What the player's game cannot foresee about its own plane.
            let terms = out.terms.iter().find(|(p, _)| *p == plane).map(|(_, t)| *t);
            let changed =
                terms != peer.terms || out.events.iter().any(|event| about(event, plane.0));
            peer.unforeseen |= changed;
            peer.terms = terms;
            #[cfg(test)]
            if changed {
                self.unforeseen_log.push((tick, seat, Unforeseen::Event));
            }
        }
        for id in behind {
            self.server.disconnect(id, DisconnectReason::ProtocolError);
        }
    }

    /// Each seated player's snapshot of `tick`, and its plane's exact state
    /// when due.
    /// The snapshots due at `tick`: each seat's has its own phase within the
    /// interval ([`wire::snapshot_phase`]), so the cost spreads over the
    /// interval's ticks instead of landing on one.
    fn snapshots(&mut self, tick: u64, now: Duration, out: &TickOutput) {
        let tps = self.config.ticks_per_snapshot();
        let ids: Vec<ConnectionId> = self
            .peers
            .iter()
            .filter(|(_, peer)| {
                peer.stage == Stage::Seated
                    && peer.seat.is_some_and(|seat| {
                        tick % u64::from(tps) == crate::wire::snapshot_phase(seat.0, tps)
                    })
            })
            .map(|(id, _)| *id)
            .collect();
        if ids.is_empty() {
            return;
        }
        // Who each side's sensors track: every human-flown plane's contacts.
        let mut tracked: BTreeMap<bool, BTreeSet<u32>> = BTreeMap::new();
        for own in self.world.combat.state.ownships() {
            let Some(entry) = self.world.roster.plane(PlaneId(own.aircraft)) else {
                continue;
            };
            let set = tracked
                .entry(entry.slot.wing.side == Side::Friendly)
                .or_default();
            set.extend(own.sensors.contacts().iter().map(|c| c.id));
            set.extend(own.sensors.visual().iter().map(|c| c.id));
        }
        let mut failed = Vec::new();
        for id in ids {
            if self.snapshot(id, tick, now, out, &tracked).is_err() {
                failed.push(id);
            }
        }
        for id in failed {
            self.server.disconnect(id, DisconnectReason::ProtocolError);
        }
    }

    fn snapshot(
        &mut self,
        connection: ConnectionId,
        tick: u64,
        now: Duration,
        out: &TickOutput,
        tracked: &BTreeMap<bool, BTreeSet<u32>>,
    ) -> Result<(), WireError> {
        let world = &self.world;
        let Some(peer) = self.peers.get_mut(&connection) else {
            return Ok(());
        };
        let Some(plane) = peer.plane else {
            return Ok(());
        };
        let Some(cockpit) = world.cockpits.iter().find(|c| c.plane == plane) else {
            return Ok(());
        };
        let Some(picture) = from_world::seat_picture(world, plane) else {
            return Ok(());
        };
        let slot = world.roster.plane(plane).map(|p| p.slot);
        let flight: BTreeSet<u32> = world
            .roster
            .planes()
            .iter()
            .filter(|p| Some(p.slot.wing) == slot.map(|s| s.wing))
            .map(|p| p.id.0)
            .collect();
        let side_tracks = slot.and_then(|s| tracked.get(&(s.wing.side == Side::Friendly)));
        let own = cockpit.flight.position;
        let entities: Vec<(Entity, _)> = from_world::entities(
            &picture,
            peer.picture.as_ref(),
            plane.0,
            &mut peer.wire.names,
        )?
        .into_iter()
        .map(|entity| {
            let mut relevance = from_world::distance_relevance(&entity, own, plane.0);
            let aircraft = entity.state.kind() == EntityKind::Aircraft;
            relevance.own_flight = aircraft && flight.contains(&entity.id);
            relevance.friendly_tracked =
                aircraft && side_tracks.is_some_and(|t| t.contains(&entity.id));
            relevance.viewed = peer.inputs.view_subject == Some(entity.key());
            (entity, relevance)
        })
        .collect();
        let terms = out.terms.iter().find(|(p, _)| *p == plane).map(|(_, t)| *t);
        let exact = ExactState::of(&OwnPlane::of(cockpit), terms.as_ref());
        let (input_margin, inputs_repeated) = peer.inputs.report();
        let header = SnapshotHeader {
            tick: tick as u32,
            input_received: peer.inputs.newest(),
            input_margin,
            inputs_repeated,
            commands_applied: peer.inputs.applied(),
            own_hash: Some(exact.hash()?),
        };
        // New names go first, in a message the packet can carry.
        if let Some(names) = peer.wire.names.take_new() {
            let message = Message::Names(names);
            let body = message.encode()?;
            if self
                .server
                .send_message(connection, message.kind(), &body)
                .is_err()
            {
                return Err(WireError::Invalid("message queue"));
            }
        }
        let messages = self
            .server
            .messages_due_bytes(connection, now)
            .min(FLIGHT_MESSAGE_BUDGET);
        // The seat's cockpit readout, from the tick's flight.
        let readout = world.combat.cockpit_readout(
            plane.0,
            tore_world::combat::launcher(&cockpit.flight),
            world.ai_wings.as_ref(),
            Some(cockpit),
        );
        let packet =
            peer.wire
                .snapshot_with_readout(&header, &entities, readout.as_ref(), messages)?;
        match self
            .server
            .send_payload(now, connection, &packet.sections())
        {
            Ok(sequence) => peer.wire.sent(sequence),
            Err(_) => peer.wire.discard(),
        }
        peer.picture = Some(picture);

        // The Seated message comes first: an exact state is held until it
        // is acknowledged (every reliable message acknowledged, since the
        // transport reports no more than the count), and then goes out at
        // once if one was due meanwhile.
        if let Some(since) = peer.holding_since {
            let acknowledged = self
                .server
                .stats(connection)
                .is_some_and(|stats| stats.messages_queued == 0);
            if acknowledged || tick.saturating_sub(since) >= SEATED_HOLD_TICKS {
                peer.holding_since = None;
            }
        }
        let mismatch = peer.inputs.mismatch > peer.mismatch_answered;
        let due = peer.holding_since.is_none()
            && (peer.unforeseen
                || mismatch
                || tick.saturating_sub(peer.last_own_state) >= OWN_STATE_INTERVAL_TICKS);
        if due {
            let bytes = peer.wire.own_state(tick as u32, &exact)?;
            match self
                .server
                .send_payload(now, connection, &[(SECTION_OWN_STATE, &bytes)])
            {
                Ok(sequence) => {
                    peer.wire.sent(sequence);
                    peer.unforeseen = false;
                    peer.last_own_state = tick;
                    peer.mismatch_answered = peer.inputs.mismatch;
                }
                Err(_) => peer.wire.discard(),
            }
        }
        Ok(())
    }

    // ----- The lifecycle ------------------------------------------------

    /// The time limit and the empty timeout.
    fn check_lifecycle(&mut self, now: Duration) {
        let seated = self
            .peers
            .values()
            .any(|p| matches!(p.stage, Stage::Seated | Stage::Taking { .. }));
        if seated {
            self.empty_since = None;
        } else if self.ever_seated && self.empty_since.is_none() {
            self.empty_since = Some(now);
        }
        let next = match self.config.after_end {
            AfterEnd::Restart => Some(self.config.restart_delay),
            AfterEnd::Quit => None,
        };
        let flown = ticks_time(self.world.tick());
        if self.config.time_limit.is_some_and(|limit| flown >= limit) {
            self.end_mission(EndReason::TimeLimit, next);
        } else if self
            .empty_since
            .is_some_and(|since| now.saturating_sub(since) >= self.config.empty_timeout)
        {
            self.end_mission(EndReason::EveryoneLeft, next);
        }
    }

    /// Ends the mission: "Mission ended" to every player, and each seated
    /// player's debrief; each is disconnected once they are acknowledged.
    /// `next` is the delay to the next mission, `None` to stop.
    fn end_mission(&mut self, reason: EndReason, next: Option<Duration>) {
        if matches!(self.life, Life::Ended { .. } | Life::Stopped) {
            return;
        }
        let now = self.now;
        let ended = Message::MissionEnded(MissionEnded {
            reason,
            next_in_seconds: next.map(|d| d.as_secs().min(u64::from(u32::MAX)) as u32),
        });
        let ids: Vec<ConnectionId> = self.peers.keys().copied().collect();
        for id in ids {
            let peer = &self.peers[&id];
            if matches!(peer.stage, Stage::Closing { .. }) {
                continue;
            }
            let report = match peer.stage {
                Stage::Seated => peer
                    .seat
                    .and_then(|seat| debrief::capture(&self.world, seat)),
                _ => None,
            };
            self.send(id, &ended);
            if let Some(report) = report {
                self.send(id, &Message::Debrief(Box::new(debrief_message(&report))));
            }
            if let Some(peer) = self.peers.get_mut(&id) {
                peer.ended = true;
                peer.stage = Stage::Closing {
                    deadline: now + CLOSE_GRACE,
                    reason: DisconnectReason::ServerStopping,
                };
            }
        }
        self.gives.clear();
        self.life = Life::Ended {
            next_at: next.map(|delay| now + delay),
            stop_at: now + CLOSE_GRACE,
        };
        let tick = self.world.tick();
        self.log(HostLog::MissionEnded { tick, reason });
    }

    /// A fresh copy of the mission, from its spec.
    fn next_mission(&mut self) {
        match build_world(&self.spec, &self.resources) {
            Ok((world, manifest)) => {
                self.world = world;
                self.manifest = manifest;
                self.tracker = Tracker::new(&self.world);
                self.orphans.clear();
                self.gives.clear();
                self.origin = None;
                self.ticks_run = 0;
                self.ever_seated = false;
                self.empty_since = None;
                self.log(HostLog::MissionRestarted { tick: 0 });
                match self.config.start {
                    StartMode::FirstPlayer => self.life = Life::Waiting,
                    StartMode::Now => self.start_flying(),
                }
            }
            Err(error) => {
                let tick = self.world.tick();
                self.log(HostLog::Fault {
                    tick,
                    text: error.to_string(),
                });
                self.stop();
            }
        }
    }

    /// The mission cannot go on: it ends as the operator's end would.
    fn fault(&mut self, text: String) {
        let tick = self.world.tick();
        self.log(HostLog::Fault { tick, text });
        let next = match self.config.after_end {
            AfterEnd::Restart => Some(self.config.restart_delay),
            AfterEnd::Quit => None,
        };
        self.end_mission(EndReason::EndedByServer, next);
    }
}

/// Queues a mission-wide event for `peer`, numbering its names in the
/// connection's table; false when the connection is too far behind.
fn queue(peer: &mut Peer, tick: u64, event: &Wide) -> bool {
    let event = match event {
        Wide::Event(event) => event.clone(),
        Wide::Launch {
            shooter,
            projectile,
            weapon,
        } => match peer.wire.names.intern(weapon) {
            Ok(weapon) => WireEvent::Launch {
                shooter: *shooter,
                projectile: *projectile,
                weapon,
            },
            Err(_) => return false,
        },
    };
    peer.wire.event(tick as u32, &event).is_ok()
}

/// The explosion lines the tick gives a seat whose own plane exploded.
fn exploded(text: &str) -> Option<bool> {
    match text {
        "Your aircraft exploded on impact" => Some(true),
        "Your aircraft exploded" => Some(false),
        _ => None,
    }
}

/// Whether a combat event did something to `plane` that its player's game
/// cannot foresee: a hit, damage, a blast, its destruction or a release.
fn about(event: &live::Event, plane: u32) -> bool {
    use live::Event as E;
    match event {
        E::Fired { aircraft, .. }
        | E::OwnshipDamaged { aircraft, .. }
        | E::SubsystemDamaged { aircraft, .. }
        | E::OwnshipDestroyed { aircraft }
        | E::PilotKilled { aircraft }
        | E::OwnshipGroundImpact { aircraft } => *aircraft == plane,
        E::Jolt(jolt) => jolt.target == plane,
        E::Airburst(id) => *id == plane,
        _ => false,
    }
}

/// The aircraft that flies as `plane`.
fn aircraft_of(world: &World, plane: PlaneId) -> Option<tore_formats::aircraft::AircraftId> {
    world
        .ai_wings
        .as_ref()
        .and_then(|wings| wings.slot(plane.0))
        .map(|slot| slot.aircraft)
        .or_else(|| {
            world
                .combat
                .state
                .ownship(plane.0)
                .map(|own| own.configuration().aircraft)
        })
}
