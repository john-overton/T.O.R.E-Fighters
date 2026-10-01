//! The client session: a player's game joined to a host, with no window or
//! audio. See docs/ARCHITECTURE.md, "The client session"; the numbers are
//! docs/MULTIPLAYER.md, "Netcode numbers"; the bytes are
//! docs/formats/net-protocol.md.
//!
//! [`Client`] is driven like the host: the caller passes the time in
//! ([`Client::update`]), feeds it datagrams ([`Client::receive`]) and sends
//! what it gives ([`Client::poll_transmit`]). It reads no clock and touches
//! no socket itself.
//!
//! - **Joining.** It connects with a callsign, password and build. On the
//!   Mission message it builds the mission from its own import
//!   (`World::new(spec, resources, Seating::Open)`, never stepped: the
//!   terrain, the aircraft types, the ground objects and the roster), compares
//!   the content manifests and refuses on a difference, then asks for its
//!   plane. On Seated it decodes its plane's exact state.
//! - **Prediction** ([`prediction`]): each tick of its clock it steps its
//!   plane with the pilot's quantized input, and sends the inputs with their
//!   redundancy and the numbered commands.
//! - **Reconciliation**: it compares each snapshot's own state hash with its
//!   own at that tick and reports a mismatch; an exact state restarts the
//!   plane at its tick and steps the stored inputs again, and the drawn plane
//!   slides to the correction.
//! - **Clock steering** ([`clock`]): from the margins the host reports.
//! - **Interpolation** ([`interpolation`]): everything else, drawn in the past.
//! - **The frame's data** ([`ClientFrame`]): the own plane's presented
//!   flight, the picture, the newest cockpit readout and the events.
//! - **Diagnostics and capture** ([`diagnostics`], [`capture`]).

pub mod capture;
pub mod clock;
pub mod diagnostics;
pub mod interpolation;
pub mod prediction;
#[cfg(test)]
mod tests;

use crate::host::BuildId;
use crate::wire::connection::ClientConnection;
use crate::wire::entity::EntityKey;
use crate::wire::events::{ReceivedEvent, WireEvent};
use crate::wire::inputs::{Command, InputFrame, InputsSection, NumberedCommand, quantize_command};
use crate::wire::messages::{
    ContentRefused, Debrief, Message, Mission, MissionEnded, Ready, Roster, Seated,
};
use crate::wire::names::NameIndex;
use crate::wire::{
    PROTOCOL_VERSION, SECTION_EVENTS, SECTION_INPUTS, SECTION_OWN_STATE, SECTION_SNAPSHOT,
};
use capture::CaptureWriter;
use clock::{
    HIGH_LOSS, INTERPOLATION_MARGIN, INTERPOLATION_MARGIN_LOSSY, InputClock, LossWindow,
    RenderClock, TICKS_PER_SECOND,
};
use diagnostics::Diagnostics;
use interpolation::Interpolator;
use prediction::{Offset, Predictor, Restored, pose};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{self, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tore_formats::aircraft::AircraftId;
use tore_net::{CloseReason, Datagrams, DisconnectReason, Entropy, Event, Transmit};
use tore_sim::combat::blast::MarkKind;
use tore_sim::combat::live::{Configuration, EffectKind};
use tore_sim::flight::{self, PilotInput};
use tore_sim::models::AircraftModel;
use tore_world::WorldResult;
use tore_world::frame::{FlightFrame, ReadoutSlot};
use tore_world::mission::{LoadoutSpec, MissionSpec};
use tore_world::readout::CockpitReadout;
use tore_world::resources::{ResourceReads, ResourceSource};
use tore_world::seats::{PlaneId, SeatCommand, SeatId};
use tore_world::snapshot::{
    AircraftPose, Damage, Draw, EffectPose, Engine, MarkPose, PilotPose, RenderSnapshot,
};
use tore_world::world::{Seating, World};

/// Ticks between two input packets at 60 a second: the input margin is one
/// tick plus this, and this again while loss is high.
pub const INPUT_INTERVAL_TICKS: f64 = 2.;
/// The client sends at most one input packet this often (60 a second).
pub const INPUT_PACKET_INTERVAL: Duration = Duration::from_micros(16_666);
/// Input ticks in one packet at most (200 ms).
pub const INPUT_REDUNDANCY: u64 = 24;
/// The most ticks one update steps; past it the clock is beyond any honest
/// margin and the rest wait for the next update.
pub const MAX_TICKS_PER_UPDATE: u64 = 240;
/// A correction within this long of seating snaps.
pub const SEATING_SNAP: Duration = Duration::from_secs(1);
/// Events held for the caller at most; older ones are dropped.
const MAX_EVENTS: usize = 4096;

/// What a join needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientConfig {
    /// The host's address.
    pub server: SocketAddr,
    /// 1 to 15 printable ASCII characters; the host adds a suffix when it is
    /// taken.
    pub callsign: String,
    /// The server's password; empty for none.
    pub password: String,
    /// This game's build, which the host must match.
    pub build: BuildId,
    /// The plane to ask for (`--slot`), or `None` for the first free one.
    pub plane: Option<u32>,
    /// Where the join's randomness comes from: [`Entropy::System`] on real
    /// sockets, a seed in tests and the simulator.
    pub entropy: Entropy,
    /// The retail stall-speed switch is on for this process: a client
    /// refuses to join with it.
    pub retail_stall_speeds: bool,
}

impl ClientConfig {
    /// A join to `server` as `callsign` with this `build`: no password, any
    /// plane, system entropy.
    pub fn new(server: SocketAddr, callsign: &str, build: BuildId) -> Self {
        Self {
            server,
            callsign: callsign.to_owned(),
            password: String::new(),
            build,
            plane: None,
            entropy: Entropy::System,
            retail_stall_speeds: false,
        }
    }
}

/// Why a join could not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientError {
    /// The callsign, version, commit or password does not fit the protocol.
    Config(String),
    /// The retail stall-speed switch is on.
    RetailStallSpeeds,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(text) => f.write_str(text),
            Self::RetailStallSpeeds => f.write_str(
                "the retail stall-speed switch (--retail-stall-speeds or \
                 TORE_RETAIL_STALL_SPEEDS) is on; every machine in a session must fly one \
                 configuration, so a game with it cannot join",
            ),
        }
    }
}

impl std::error::Error for ClientError {}

/// What the pilot does this update: held controls and the commands given
/// since the last update.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Controls {
    /// Sticks and throttle, and the pilot commands given (gear, flaps,
    /// switches, throttle presses), in order.
    pub pilot: PilotInput,
    /// The trigger is held.
    pub trigger: bool,
    /// The scope controls.
    pub sensors: tore_sim::sensors::Controls,
    /// Seat commands given, in order.
    pub commands: Vec<SeatCommand>,
    /// What the view follows when it is not the own cockpit.
    pub view_subject: Option<EntityKey>,
}

impl Controls {
    /// Neutral controls for while a menu is up or the window lost focus:
    /// stick centred, throttle held, trigger released, the scope as it is.
    pub fn neutral(sensors: tore_sim::sensors::Controls) -> Self {
        Self {
            sensors,
            ..Self::default()
        }
    }
}

/// Controls as the client steps and sends them: rounded to the wire's steps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sampled {
    pub frame: InputFrame,
    /// Pilot commands first, then seat commands, each in order.
    pub commands: Vec<Command>,
    pub view_subject: Option<EntityKey>,
}

impl Sampled {
    /// `controls` rounded as the host will step them.
    pub fn of(controls: &Controls) -> Self {
        Self {
            frame: InputFrame::of(&controls.pilot, controls.trigger, controls.sensors),
            commands: controls
                .pilot
                .commands
                .iter()
                .map(|&c| Command::Pilot(quantize_command(c)))
                .chain(controls.commands.iter().map(|&c| Command::Seat(c)))
                .collect(),
            view_subject: controls.view_subject,
        }
    }
}

/// Where the client is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientPhase {
    /// Handshaking.
    Connecting,
    /// Accepted; waiting for or loading the mission.
    Loading,
    /// Asked for a plane.
    Seating,
    /// Flying a plane.
    Flying,
    /// Sent Leave; waiting for the debrief and the disconnect.
    Leaving,
    /// Done; the [`ClientEvent::Closed`] event says why.
    Closed,
}

/// Something that happened to the session.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientEvent {
    /// The host accepted the join.
    Connected {
        session_id: u64,
        ticks_per_snapshot: u8,
        host_tick: u32,
    },
    /// The mission is built from the import and matches the host's.
    MissionLoaded,
    /// The import differs from the host's in these resources: the client
    /// told the host, which disconnects it.
    ContentRefused { names: Vec<String> },
    /// The mission could not be built from the import.
    MissionFailed(String),
    /// No plane: the reason. The client may ask again with [`Client::ready`].
    SeatRefused(String),
    /// The player has a plane.
    Seated { seat: u8, plane: u32, tick: u32 },
    /// The roster changed; read it with [`Client::roster`].
    Roster,
    /// A line from the server for the HUD.
    Notice(String),
    /// The player's debrief.
    Debrief(Box<Debrief>),
    /// The host ended the mission.
    MissionEnded(MissionEnded),
    /// The connection ended; nothing follows.
    Closed(CloseReason),
}

/// A plain-language line for why the connection ended.
pub fn describe(reason: &CloseReason) -> String {
    match reason {
        CloseReason::NoAnswer => "No answer from the server.".into(),
        CloseReason::Refused { text, .. } => text.clone(),
        CloseReason::Replaced => "A new join from this address replaced the connection.".into(),
        CloseReason::Disconnected { reason, by_peer } => {
            let why = match reason {
                DisconnectReason::Left => "the player left",
                DisconnectReason::Timeout => "no packets for 5 seconds",
                DisconnectReason::BadPackets => "too many bad packets",
                DisconnectReason::ProtocolError => "a protocol error",
                DisconnectReason::ContentMismatch => "the game data differs from the server's",
                DisconnectReason::ServerStopping => "the server is stopping",
                DisconnectReason::Kicked => "kicked by the server",
                DisconnectReason::Other(_) => "an unknown reason",
            };
            if *by_peer {
                format!("The server ended the connection: {why}.")
            } else {
                format!("The connection ended: {why}.")
            }
        }
    }
}

/// A correction of the own plane, for the figures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Correction {
    /// The host tick of the exact state.
    pub tick: u64,
    /// The predicted tick it was applied at.
    pub now: u64,
    /// How far the drawn plane moved at now.
    pub feet: f64,
    pub degrees: f64,
    /// Blended (true), or snapped or too small to show (false).
    pub shown: bool,
}

/// The client's figures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientStats {
    pub round_trip: Duration,
    /// Lost over judged of the client's packets, over 5 seconds.
    pub loss: Option<f64>,
    /// Snapshots missed over expected, over 10 seconds.
    pub snapshot_loss: f64,
    /// The arrival spread of snapshot packets.
    pub spread: Duration,
    /// The last input margin the host reported, ticks.
    pub input_margin: Option<i8>,
    pub interpolation_delay_ticks: f64,
    pub clock_rate: f64,
    pub clock_jumps: u64,
    pub bytes_up_per_second: u64,
    pub bytes_down_per_second: u64,
    pub predicted_tick: u64,
    pub render_tick: f64,
    pub snapshots: u64,
    pub own_states: u64,
    /// Snapshot hashes compared with the prediction, and those that differed.
    pub hashes_compared: u64,
    pub mismatches: u64,
    /// Exact states that changed the prediction: in all, blended, adopted.
    pub corrections: u64,
    pub corrections_shown: u64,
    pub adopted: u64,
    pub frames: u64,
    /// Entities drawn over all frames, and of them past their newest state.
    pub entity_frames: u64,
    pub extrapolated: u64,
    /// Ticks the host repeated the last input for, as its snapshots said.
    pub inputs_repeated: u64,
    pub input_packets: u64,
}

/// What the game draws this frame.
#[derive(Clone, Debug)]
pub struct ClientFrame {
    pub seat: SeatId,
    pub plane: PlaneId,
    /// The newest predicted tick.
    pub tick: u64,
    /// The predicted flight after `tick`, and at its start.
    pub flight: flight::State,
    pub previous: flight::State,
    /// The flight to draw: blended to this instant, plus what is left of
    /// the last correction's offset.
    pub presented: flight::State,
    /// Everything drawn at `render_tick`: the own plane as the player pose,
    /// every other aircraft and the ground objects as targets, projectiles,
    /// debris, pilots, and the effects and marks of the events.
    pub picture: RenderSnapshot,
    /// The host tick the picture shows.
    pub render_tick: f64,
    /// The newest cockpit readout received.
    pub readout: Option<CockpitReadout>,
    /// The plane's ownship configuration (its loadout).
    pub config: Arc<Configuration>,
    /// The events released since the last frame, in order: the seat's cues
    /// on arrival, mission-wide events once the picture reaches their tick.
    pub events: Vec<ReceivedEvent>,
}

impl PartialEq for ClientFrame {
    /// Equal frames, the configuration (mission data) aside.
    fn eq(&self, other: &Self) -> bool {
        self.seat == other.seat
            && self.plane == other.plane
            && self.tick == other.tick
            && self.flight == other.flight
            && self.previous == other.previous
            && self.presented == other.presented
            && self.picture == other.picture
            && self.render_tick.to_bits() == other.render_tick.to_bits()
            && self.readout == other.readout
            && self.events == other.events
    }
}

impl ClientFrame {
    /// The flight frame the screens draw, with the smoke, contrails, chaff
    /// and flares the game regenerates. Its readout is the newest received,
    /// or the empty one before any arrives (`empty`).
    pub fn flight_frame<'a>(
        &'a self,
        smoke: [&'a tore_sim::combat::smoke::Smoke; 2],
        devices: &'a tore_sim::combat::countermeasures::Devices,
        empty: impl FnOnce() -> CockpitReadout,
    ) -> FlightFrame<'a> {
        FlightFrame {
            seat: self.seat,
            plane: self.plane,
            flight: &self.flight,
            previous: &self.previous,
            presented: std::borrow::Cow::Borrowed(&self.presented),
            picture: &self.picture,
            smoke,
            devices,
            config: &self.config,
            readout: ReadoutSlot::ready(self.readout.clone().unwrap_or_else(empty)),
            tick_cues: &[],
        }
    }

    /// A digest of the frame, for comparing runs.
    pub fn digest(&self) -> u64 {
        struct Fnv(u64);
        impl std::fmt::Write for Fnv {
            fn write_str(&mut self, s: &str) -> std::fmt::Result {
                for b in s.bytes() {
                    self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
                }
                Ok(())
            }
        }
        let mut h = Fnv(0xcbf2_9ce4_8422_2325);
        let _ = std::fmt::Write::write_fmt(
            &mut h,
            format_args!(
                "{:?}{:?}{}{:?}{:?}{:?}{:?}{}{:?}{:?}",
                self.seat,
                self.plane,
                self.tick,
                self.flight,
                self.previous,
                self.presented,
                self.picture,
                self.render_tick.to_bits(),
                self.readout,
                self.events
            ),
        );
        h.0
    }
}

/// Builds the static mission; the default is `World::new` with open seating.
pub type MissionBuilder = Box<dyn FnMut(&MissionSpec, &dyn ResourceSource) -> WorldResult<World>>;

/// The mission as the client's import builds it.
struct Loaded {
    world: World,
    /// Ground objects as drawn, standing.
    ground: Vec<AircraftPose>,
    models: Vec<AircraftId>,
    start_seconds: i32,
}

/// The player's seat.
struct Seat {
    seat: SeatId,
    plane: u32,
    model: AircraftModel,
    predictor: Predictor,
    offset: Offset,
    seated_at: Duration,
    seated_tick: u64,
    loadout: LoadoutSpec,
}

/// An effect from an event, aged against the drawn time.
#[derive(Clone, Copy, Debug)]
struct Shown {
    tick: u32,
    kind: EffectKind,
    position: [f64; 3],
    ticks: u16,
    blast: Option<u8>,
}

/// The client session. See the module documentation.
pub struct Client {
    config: ClientConfig,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    seed: u64,
    started: Duration,
    net: tore_net::Client,
    wire: Option<ClientConnection>,
    ticks_per_snapshot: u32,
    phase: ClientPhase,
    builder: Option<MissionBuilder>,
    mission: Option<Loaded>,
    spec: Option<MissionSpec>,
    roster: Option<Roster>,
    seat: Option<Seat>,
    input_clock: InputClock,
    render_clock: RenderClock,
    interp: Interpolator,
    events: VecDeque<ClientEvent>,
    released: Vec<ReceivedEvent>,
    held: VecDeque<ReceivedEvent>,
    effects: Vec<Shown>,
    marks: Vec<(u32, MarkKind, [f64; 3])>,
    destroyed: BTreeMap<u32, u32>,
    pending: Vec<Command>,
    view_subject: Option<EntityKey>,
    unacked: VecDeque<NumberedCommand>,
    next_command: u16,
    input_acked: u32,
    last_sent: Option<Duration>,
    sent_tick: u64,
    mismatch: u32,
    snapshot_tick: Option<u32>,
    /// Own state sections that arrived before the Seated message they
    /// follow, read once it has.
    early: Vec<Vec<u8>>,
    /// The clock was set from the host's first margin since seating.
    settled: bool,
    readout: Option<CockpitReadout>,
    upstream: LossWindow,
    downstream: LossWindow,
    stats: ClientStats,
    corrections: Vec<Correction>,
    diagnostics: Option<Diagnostics>,
    capture: Option<CaptureWriter>,
    now: Duration,
}

/// The seed a join's randomness comes from: drawn from the system for
/// [`Entropy::System`], so a capture can repeat the join exactly.
fn seed_of(entropy: Entropy) -> u64 {
    use std::hash::BuildHasher;
    match entropy {
        Entropy::System => std::collections::hash_map::RandomState::new().hash_one(0x5eed_u32),
        Entropy::Seeded(seed) => seed,
    }
}

fn ticks_time(ticks: u64) -> Duration {
    Duration::from_nanos((u128::from(ticks) * 1_000_000_000 / 120) as u64)
}

impl Client {
    /// Starts joining `config.server` at `now`, with the game data
    /// `resources` the mission will be built from. Refuses a callsign or
    /// strings the protocol cannot carry, and the retail stall-speed switch.
    pub fn connect(
        config: ClientConfig,
        resources: Arc<BTreeMap<String, Vec<u8>>>,
        now: Duration,
    ) -> Result<Client, ClientError> {
        let seed = seed_of(config.entropy);
        Self::start(config, resources, now, seed)
    }

    pub(crate) fn start(
        config: ClientConfig,
        resources: Arc<BTreeMap<String, Vec<u8>>>,
        now: Duration,
        seed: u64,
    ) -> Result<Client, ClientError> {
        if config.retail_stall_speeds || tore_sim::flight::retail_stall_speeds() {
            return Err(ClientError::RetailStallSpeeds);
        }
        let net = tore_net::Client::connect(
            tore_net::ClientConfig {
                game_version: config.build.version.clone(),
                game_commit: config.build.commit.clone(),
                password: config.password.clone(),
                entropy: Entropy::Seeded(seed),
                ..tore_net::ClientConfig::new(PROTOCOL_VERSION, &config.callsign)
            },
            config.server,
            now,
        )
        .map_err(|error| ClientError::Config(error.to_string()))?;
        Ok(Client {
            config,
            resources,
            seed,
            started: now,
            net,
            wire: None,
            ticks_per_snapshot: 4,
            phase: ClientPhase::Connecting,
            builder: None,
            mission: None,
            spec: None,
            roster: None,
            seat: None,
            input_clock: InputClock::default(),
            render_clock: RenderClock::new(),
            interp: Interpolator::new(4),
            events: VecDeque::new(),
            released: Vec::new(),
            held: VecDeque::new(),
            effects: Vec::new(),
            marks: Vec::new(),
            destroyed: BTreeMap::new(),
            pending: Vec::new(),
            view_subject: None,
            unacked: VecDeque::new(),
            next_command: 1,
            input_acked: 0,
            last_sent: None,
            sent_tick: 0,
            mismatch: 0,
            snapshot_tick: None,
            early: Vec::new(),
            settled: false,
            readout: None,
            upstream: LossWindow::default(),
            downstream: LossWindow::default(),
            stats: ClientStats::default(),
            corrections: Vec::new(),
            diagnostics: None,
            capture: None,
            now,
        })
    }

    // ----- Settings ------------------------------------------------------

    /// Writes the diagnostics log to `out`: a header, a line a second and
    /// every join, seating, refusal and drop ([`diagnostics`]).
    pub fn set_diagnostics(&mut self, out: Box<dyn Write>) {
        let mut diagnostics = Diagnostics::new(out);
        diagnostics.line(
            self.now,
            "connect",
            &[&self.config.server.to_string(), &self.config.callsign],
        );
        self.diagnostics = Some(diagnostics);
    }

    /// Writes the capture to `out` ([`capture`]). Set it right after
    /// [`Client::connect`], before any other call, for a complete capture.
    pub fn set_capture(&mut self, out: Box<dyn Write>) {
        let mut writer = CaptureWriter::new(out);
        writer.header(&self.config, self.seed, self.started);
        self.capture = Some(writer);
    }

    /// Builds the mission with `builder` instead of `World::new` with open
    /// seating; the game passes one with its drawn-model loader.
    pub fn set_mission_builder(&mut self, builder: MissionBuilder) {
        self.builder = Some(builder);
    }

    // ----- Driving -------------------------------------------------------

    /// Takes one datagram that arrived at `now` from `from`.
    pub fn receive(&mut self, now: Duration, from: SocketAddr, datagram: &[u8]) {
        if let Some(capture) = &mut self.capture {
            capture.receive(now, from, datagram);
        }
        self.now = self.now.max(now);
        let wire = &self.wire;
        let early = &self.early;
        let seated = self.seat.is_some();
        self.net
            .receive_checked(now, from, datagram, &mut |kind, body| {
                let Some(wire) = wire else {
                    return false;
                };
                if kind == SECTION_OWN_STATE && !seated {
                    return early_own_state_fits(early, body);
                }
                wire.check(kind, body)
            });
        self.pump();
    }

    /// Reads every datagram waiting on `socket` (at most 1,024) and takes
    /// each, at `now`.
    pub fn receive_from<D: Datagrams + ?Sized>(
        &mut self,
        now: Duration,
        socket: &mut D,
    ) -> io::Result<()> {
        let mut buf = [0u8; tore_net::MAX_DATAGRAM + 1];
        for _ in 0..tore_net::MAX_RECEIVE_BATCH {
            let Some((len, from)) = socket.recv_datagram(&mut buf)? else {
                break;
            };
            self.receive(now, from, &buf[..len]);
        }
        Ok(())
    }

    /// Runs what is due at `now` with the pilot's `controls`: the transport's
    /// timers, every predicted tick the clock has reached (the commands in
    /// `controls` go to the first, or wait for the next update that steps
    /// one), and the inputs.
    pub fn update(&mut self, now: Duration, controls: &Controls) {
        self.update_sampled(now, Sampled::of(controls));
    }

    pub(crate) fn update_sampled(&mut self, now: Duration, sampled: Sampled) {
        if let Some(capture) = &mut self.capture {
            capture.update(now, &sampled);
        }
        self.now = self.now.max(now);
        let now = self.now;
        self.net.update(now);
        self.pump();
        self.pending.extend(sampled.commands);
        self.view_subject = sampled.view_subject;
        if self.phase == ClientPhase::Flying {
            self.fly(now, sampled.frame);
        }
        let margin = self.interpolation_margin(now);
        self.render_clock.advance(now, margin);
        self.interp.advance(now);
        self.diagnose(now);
    }

    /// The next datagram to send, oldest first.
    pub fn poll_transmit(&mut self) -> Option<Transmit> {
        self.net.poll_transmit()
    }

    /// Sends every queued datagram on `socket`.
    pub fn transmit<D: Datagrams + ?Sized>(&mut self, socket: &mut D) -> io::Result<()> {
        self.net.transmit(socket)
    }

    /// How long from `now` until the next predicted tick is due, at most
    /// 10 ms.
    pub fn next_wake(&self, now: Duration) -> Duration {
        let idle = Duration::from_millis(10);
        if self.phase != ClientPhase::Flying {
            return idle;
        }
        let Some(seat) = &self.seat else {
            return idle;
        };
        let due = (seat.predictor.tick() + 1) as f64 - self.input_clock.position();
        let wait =
            Duration::from_secs_f64((due.max(0.) / TICKS_PER_SECOND) / self.input_clock.rate());
        wait.min(idle)
            .min(idle.saturating_sub(now.saturating_sub(self.now)))
    }

    /// The next session event, oldest first.
    pub fn poll_event(&mut self) -> Option<ClientEvent> {
        self.events.pop_front()
    }

    /// Asks for a plane again after a refusal: `plane`, or any.
    pub fn ready(&mut self, plane: Option<u32>) {
        if self.phase == ClientPhase::Seating || self.phase == ClientPhase::Loading {
            self.send(&Message::Ready(Ready { plane }));
            self.phase = ClientPhase::Seating;
        }
    }

    /// The player ends the mission: Leave goes to the host, which sends the
    /// debrief and then disconnects.
    pub fn leave(&mut self, now: Duration) {
        if let Some(capture) = &mut self.capture {
            capture.leave(now);
        }
        self.now = self.now.max(now);
        if matches!(
            self.phase,
            ClientPhase::Loading | ClientPhase::Seating | ClientPhase::Flying
        ) {
            self.send(&Message::Leave);
            self.phase = ClientPhase::Leaving;
            self.net.update(self.now);
            self.pump();
        }
    }

    /// Quits at once: Disconnect goes to the host three times.
    pub fn disconnect(&mut self, now: Duration) {
        if let Some(capture) = &mut self.capture {
            capture.disconnect(now);
        }
        self.now = self.now.max(now);
        self.net.disconnect(DisconnectReason::Left);
        self.pump();
    }

    // ----- Reading -------------------------------------------------------

    /// Where the session is.
    pub fn phase(&self) -> ClientPhase {
        self.phase
    }

    /// The mission as the import built it, once loaded: never stepped.
    pub fn mission(&self) -> Option<&World> {
        self.mission.as_ref().map(|m| &m.world)
    }

    /// The mission's spec, once received.
    pub fn spec(&self) -> Option<&MissionSpec> {
        self.spec.as_ref()
    }

    /// The roster as the host last sent it.
    pub fn roster(&self) -> Option<&Roster> {
        self.roster.as_ref()
    }

    /// The seat and plane, once seated.
    pub fn seat(&self) -> Option<(SeatId, PlaneId)> {
        self.seat.as_ref().map(|s| (s.seat, PlaneId(s.plane)))
    }

    /// The plane's loadout as the host seated it.
    pub fn loadout(&self) -> Option<&LoadoutSpec> {
        self.seat.as_ref().map(|s| &s.loadout)
    }

    /// The name a snapshot or an event numbers.
    pub fn name(&self, index: NameIndex) -> Option<&str> {
        self.wire.as_ref()?.names.name(index)
    }

    /// The client's figures now.
    pub fn stats(&mut self) -> ClientStats {
        let now = self.now;
        let mut stats = self.stats.clone();
        if let Some(net) = self.net.stats() {
            stats.round_trip = net.round_trip;
            stats.loss = net.loss;
            stats.spread = net.spread;
            stats.bytes_up_per_second = net.bytes_sent_per_second;
            stats.bytes_down_per_second = net.bytes_received_per_second;
        }
        stats.snapshot_loss = self.downstream.loss(now);
        stats.interpolation_delay_ticks = self.render_clock.delay();
        stats.clock_rate = self.input_clock.rate();
        stats.clock_jumps = self.input_clock.jumps();
        stats.predicted_tick = self.seat.as_ref().map_or(0, |s| s.predictor.tick());
        stats.render_tick = self.render_clock.render().unwrap_or(0.);
        stats
    }

    /// [`Client::stats`] without moving the loss window on.
    pub fn clone_stats(&self) -> ClientStats {
        let mut stats = self.stats.clone();
        if let Some(net) = self.net.stats() {
            stats.round_trip = net.round_trip;
            stats.loss = net.loss;
            stats.spread = net.spread;
        }
        stats.interpolation_delay_ticks = self.render_clock.delay();
        stats.clock_rate = self.input_clock.rate();
        stats.clock_jumps = self.input_clock.jumps();
        stats
    }

    /// Every correction of the own plane so far.
    pub fn corrections(&self) -> &[Correction] {
        &self.corrections
    }

    /// The prediction's exact state and tick, once seated.
    pub fn prediction(&self) -> Option<&Predictor> {
        self.seat.as_ref().map(|s| &s.predictor)
    }

    /// The entities' extra delays and the like, for tests.
    pub fn interpolator(&self) -> &Interpolator {
        &self.interp
    }

    /// The host tick the picture shows now.
    pub fn render_tick(&self) -> Option<f64> {
        self.render_clock.render()
    }

    // ----- The frame -----------------------------------------------------

    /// What the game draws at `now`: `None` until seated. The events since
    /// the last frame come with it.
    pub fn frame(&mut self, now: Duration) -> Option<ClientFrame> {
        if let Some(capture) = &mut self.capture {
            capture.frame(now);
        }
        self.now = self.now.max(now);
        let now = self.now;
        let margin = self.interpolation_margin(now);
        self.render_clock.advance(now, margin);
        self.interp.advance(now);
        let render = self.render_clock.render();
        if let Some(render) = render {
            while self
                .held
                .front()
                .is_some_and(|e| f64::from(e.tick) <= render)
            {
                let event = self.held.pop_front().expect("an event");
                self.released.push(event);
            }
        }
        let seat = self.seat.as_ref()?;
        let render = render.unwrap_or(seat.predictor.tick() as f64);
        let plane = seat.plane;
        let wire = &self.wire;
        let name = |index: NameIndex| {
            wire.as_ref()
                .and_then(|w| w.names.name(index))
                .unwrap_or_default()
                .to_owned()
        };
        let drawn = self.interp.draw(render, plane, &name);
        self.stats.frames += 1;
        self.stats.entity_frames += drawn.entities as u64;
        self.stats.extrapolated += drawn.extrapolated as u64;

        let seat = self.seat.as_ref()?;
        let own = seat.predictor.plane();
        let fraction = (self.input_clock.position() - seat.predictor.tick() as f64).clamp(0., 1.);
        let mut presented = own.flight.presented(&own.previous_flight, fraction);
        seat.offset.apply(&mut presented, now.as_secs_f64());
        let config = Arc::clone(seat.predictor.config());
        let terms = seat.predictor.terms().copied();
        let player = player_pose(plane, &presented, &config, terms.as_ref());
        let mission = self.mission.as_ref()?;
        let mut targets = drawn.aircraft;
        targets.extend(mission.ground.iter().map(|pose| {
            let mut pose = pose.clone();
            if self
                .destroyed
                .get(&pose.id)
                .is_some_and(|tick| f64::from(*tick) <= render)
            {
                pose.damage.hp = 0;
                pose.crashed = true;
            }
            pose
        }));
        self.effects
            .retain(|e| f64::from(e.tick) + f64::from(e.ticks) > render);
        let effects = self
            .effects
            .iter()
            .filter(|e| f64::from(e.tick) <= render)
            .map(|e| EffectPose {
                kind: e.kind,
                position: e.position,
                ticks: (f64::from(e.ticks) - (render - f64::from(e.tick))).max(0.) as u16,
                blast: e.blast,
            })
            .collect();
        let marks = self
            .marks
            .iter()
            .filter(|(tick, ..)| f64::from(*tick) <= render)
            .map(|(tick, kind, position)| MarkPose {
                kind: *kind,
                position: *position,
                age: (render - f64::from(*tick)).max(0.) as u64,
                strength: 1.,
            })
            .collect();
        let pilots = presented
            .escape
            .iter()
            .map(|escape| PilotPose {
                owner: plane,
                position: escape.position,
                heading: escape.heading,
                phase: escape.phase,
            })
            .chain(drawn.pilots)
            .collect();
        let picture = RenderSnapshot {
            tick: render.max(0.) as u64,
            player,
            targets,
            projectiles: drawn.projectiles,
            effects,
            marks,
            debris: drawn.debris,
            pilots,
            models: mission.models.clone(),
        };
        Some(ClientFrame {
            seat: seat.seat,
            plane: PlaneId(plane),
            tick: seat.predictor.tick(),
            flight: own.flight.clone(),
            previous: own.previous_flight.clone(),
            presented,
            picture,
            render_tick: render,
            readout: self.readout.clone(),
            config,
            events: std::mem::take(&mut self.released),
        })
    }

    // ----- Inside --------------------------------------------------------

    fn interpolation_margin(&mut self, now: Duration) -> f64 {
        if self.downstream.loss(now) > HIGH_LOSS {
            INTERPOLATION_MARGIN_LOSSY
        } else {
            INTERPOLATION_MARGIN
        }
    }

    fn input_margin_target(&mut self, now: Duration) -> f64 {
        let lossy = self.upstream.loss(now) > HIGH_LOSS;
        1. + INPUT_INTERVAL_TICKS + if lossy { INPUT_INTERVAL_TICKS } else { 0. }
    }

    fn send(&mut self, message: &Message) {
        let sent = message
            .encode()
            .ok()
            .and_then(|body| self.net.send_message(message.kind(), &body).ok());
        if sent.is_none() {
            self.net.disconnect(DisconnectReason::ProtocolError);
        }
    }

    fn event(&mut self, event: ClientEvent) {
        self.events.push_back(event);
        while self.events.len() > MAX_EVENTS {
            self.events.pop_front();
        }
    }

    fn log(&mut self, kind: &str, fields: &[&str]) {
        if let Some(d) = &mut self.diagnostics {
            d.line(self.now, kind, fields);
        }
    }

    /// Handles every transport event.
    fn pump(&mut self) {
        while let Some(event) = self.net.poll_event() {
            match event {
                tore_net::ClientEvent::Connected(welcome) => {
                    self.ticks_per_snapshot = u32::from(welcome.ticks_per_snapshot).max(1);
                    self.wire = Some(ClientConnection::new(self.ticks_per_snapshot));
                    self.interp = Interpolator::new(self.ticks_per_snapshot);
                    self.phase = ClientPhase::Loading;
                    self.log(
                        "joined",
                        &[
                            &format!("{:016x}", welcome.session_id),
                            &welcome.host_tick.to_string(),
                        ],
                    );
                    self.event(ClientEvent::Connected {
                        session_id: welcome.session_id,
                        ticks_per_snapshot: welcome.ticks_per_snapshot,
                        host_tick: welcome.host_tick,
                    });
                }
                tore_net::ClientEvent::Closed(reason) => {
                    self.phase = ClientPhase::Closed;
                    let kind = match reason {
                        CloseReason::Refused { .. } => "refused",
                        _ => "closed",
                    };
                    self.log(kind, &[&describe(&reason)]);
                    self.event(ClientEvent::Closed(reason));
                }
                tore_net::ClientEvent::Connection(event) => match event {
                    Event::Message { kind, body } => match Message::decode(kind, &body) {
                        Ok(message) => self.message(message),
                        Err(_) => self.net.disconnect(DisconnectReason::ProtocolError),
                    },
                    Event::Payload { sections, .. } => self.payload(sections),
                    Event::Delivered { .. } => self.upstream.note(self.now, 0, 1),
                    Event::Lost { .. } => self.upstream.note(self.now, 1, 0),
                },
            }
        }
    }

    fn message(&mut self, message: Message) {
        match message {
            Message::Mission(mission) => self.mission_arrived(mission),
            Message::Roster(roster) => {
                self.roster = Some(roster);
                self.event(ClientEvent::Roster);
            }
            Message::Seated(seated) => self.seated(*seated),
            Message::SeatRefused(reason) => {
                self.phase = ClientPhase::Seating;
                self.log("seat-refused", &[&reason]);
                self.event(ClientEvent::SeatRefused(reason));
            }
            Message::Names(names) => {
                let ready = self
                    .wire
                    .as_mut()
                    .map(|wire| wire.names(&names))
                    .transpose();
                match ready {
                    Ok(events) => {
                        for event in events.into_iter().flatten() {
                            self.route(event);
                        }
                    }
                    Err(_) => self.net.disconnect(DisconnectReason::ProtocolError),
                }
            }
            Message::Notice(text) => self.event(ClientEvent::Notice(text)),
            Message::Debrief(debrief) => {
                self.log("debrief", &[]);
                self.event(ClientEvent::Debrief(debrief));
            }
            Message::MissionEnded(ended) => {
                self.log("mission-ended", &[&format!("{:?}", ended.reason)]);
                self.event(ClientEvent::MissionEnded(ended));
            }
            // Client-to-host messages from the host break the protocol.
            Message::ContentRefused(_) | Message::Ready(_) | Message::Leave => {
                self.net.disconnect(DisconnectReason::ProtocolError);
            }
        }
    }

    /// The mission: built from the import, its manifest compared, then the
    /// plane asked for.
    fn mission_arrived(&mut self, mission: Mission) {
        let spec = match mission.spec() {
            Ok(spec) => spec,
            Err(error) => {
                self.log("mission-failed", &[&error.to_string()]);
                self.event(ClientEvent::MissionFailed(error.to_string()));
                self.net.disconnect(DisconnectReason::ProtocolError);
                return;
            }
        };
        let reads = ResourceReads::new(&self.resources);
        let built = match &mut self.builder {
            Some(builder) => builder(&spec, &reads),
            None => World::new(&spec, &reads, Seating::Open),
        };
        let differences = reads.manifest().differences(&mission.manifest);
        if !differences.is_empty() {
            self.log("content-refused", &[&differences.join(" ")]);
            self.send(&Message::ContentRefused(ContentRefused {
                names: differences.clone(),
            }));
            self.event(ClientEvent::ContentRefused { names: differences });
            return;
        }
        let world = match built {
            Ok(world) => world,
            Err(error) => {
                self.log("mission-failed", &[&error.to_string()]);
                self.event(ClientEvent::MissionFailed(error.to_string()));
                self.net.disconnect(DisconnectReason::Other(0));
                return;
            }
        };
        let ground = ground_poses(&world);
        let mut models: Vec<AircraftId> = world
            .combat
            .dummy_types()
            .iter()
            .map(|t| t.profile.id)
            .collect();
        models.dedup();
        let start_seconds = world.terrain.weather.configuration().start_seconds();
        self.mission = Some(Loaded {
            world,
            ground,
            models,
            start_seconds,
        });
        self.spec = Some(spec);
        self.log("mission", &[&mission.host_tick.to_string()]);
        self.event(ClientEvent::MissionLoaded);
        self.phase = ClientPhase::Seating;
        self.send(&Message::Ready(Ready {
            plane: self.config.plane,
        }));
    }

    /// The plane is the player's: its exact state decoded, the prediction
    /// started and the clock set ahead of the host.
    fn seated(&mut self, seated: Seated) {
        let Some(mission) = &self.mission else {
            self.net.disconnect(DisconnectReason::ProtocolError);
            return;
        };
        let world = &mission.world;
        let plane = seated.plane;
        let aircraft = seated
            .roster
            .planes
            .iter()
            .find(|p| p.id == plane)
            .map(|p| p.aircraft)
            .or_else(|| {
                world
                    .ai_wings
                    .as_ref()
                    .and_then(|w| w.slot(plane))
                    .map(|s| s.aircraft)
            });
        let kit = aircraft.and_then(|aircraft| {
            let kind = world
                .combat
                .dummy_types()
                .iter()
                .find(|kind| kind.profile.id == aircraft)?;
            let config = world
                .ai_wings
                .as_ref()
                .and_then(|w| w.configuration(plane))
                .or_else(|| {
                    world
                        .combat
                        .dummy_configurations()
                        .iter()
                        .find(|c| c.aircraft == aircraft)
                })?
                .clone();
            let model = AircraftModel::for_aircraft(&kind.profile).ok()?;
            Some((model, config))
        });
        let Some((model, config)) = kit else {
            self.log("seat-failed", &[&format!("no aircraft for plane {plane}")]);
            self.net.disconnect(DisconnectReason::ProtocolError);
            return;
        };
        let state = match seated.exact_state(&model) {
            Ok(state) => state,
            Err(error) => {
                self.log("seat-failed", &[&error.to_string()]);
                self.net.disconnect(DisconnectReason::ProtocolError);
                return;
            }
        };
        let destroyed: BTreeSet<u32> = seated.destroyed.iter().copied().collect();
        let standing: BTreeSet<u32> = mission
            .ground
            .iter()
            .map(|pose| pose.id)
            .filter(|id| !destroyed.contains(id))
            .collect();
        for &id in &destroyed {
            self.destroyed.insert(id, 0);
        }
        let predictor = Predictor::new(
            SeatId(seated.seat),
            plane,
            u64::from(seated.tick),
            state,
            Arc::new(config),
            u64::from(self.ticks_per_snapshot),
            mission.start_seconds,
            standing,
        );
        let now = self.now;
        // Ahead of the host by a round trip and the margin: the inputs of the
        // ticks before that reach it late, and the exact state that answers
        // them snaps.
        let round_trip = self.net.stats().map_or(Duration::ZERO, |s| s.round_trip);
        let lead = clock::ticks_of(round_trip) + self.input_margin_target(now) + 1.;
        self.input_clock
            .reset(now, f64::from(seated.tick) + lead.ceil());
        let mut offset = Offset::default();
        offset.clear(now.as_secs_f64());
        self.roster = Some(seated.roster.clone());
        self.seat = Some(Seat {
            seat: SeatId(seated.seat),
            plane,
            model,
            predictor,
            offset,
            seated_at: now,
            seated_tick: u64::from(seated.tick),
            loadout: seated.loadout.clone(),
        });
        self.unacked.clear();
        self.pending.clear();
        self.next_command = 1;
        self.input_acked = seated.tick;
        self.sent_tick = u64::from(seated.tick);
        self.mismatch = 0;
        self.settled = false;
        self.phase = ClientPhase::Flying;
        self.log(
            "seated",
            &[
                &seated.seat.to_string(),
                &plane.to_string(),
                &seated.tick.to_string(),
            ],
        );
        self.event(ClientEvent::Seated {
            seat: seated.seat,
            plane,
            tick: seated.tick,
        });
        self.event(ClientEvent::Roster);
        for body in std::mem::take(&mut self.early) {
            self.own_state(&body);
        }
    }

    fn payload(&mut self, sections: Vec<tore_net::Section>) {
        let mut tick = self.snapshot_tick.unwrap_or(0);
        for section in sections {
            match section.kind {
                SECTION_SNAPSHOT => {
                    if let Some(t) = self.snapshot(&section.body) {
                        tick = t;
                    }
                }
                SECTION_EVENTS => {
                    let Some(wire) = self.wire.as_mut() else {
                        continue;
                    };
                    match wire.events(&section.body, tick) {
                        Ok(events) => {
                            for event in events {
                                self.route(event);
                            }
                        }
                        Err(_) => self.net.disconnect(DisconnectReason::ProtocolError),
                    }
                }
                SECTION_OWN_STATE => self.own_state(&section.body),
                _ => {}
            }
        }
    }

    /// A Snapshot section; returns its tick.
    fn snapshot(&mut self, body: &[u8]) -> Option<u32> {
        let now = self.now;
        let wire = self.wire.as_mut()?;
        let (header, received) = match wire.snapshot(body) {
            Ok(read) => read,
            Err(_) => {
                self.net.disconnect(DisconnectReason::ProtocolError);
                return None;
            }
        };
        let tick = header.tick;
        self.stats.snapshots += 1;
        self.net.note_arrival(ticks_time(u64::from(tick)), now);
        match self.snapshot_tick {
            Some(newest) if tick > newest => {
                let step = self.ticks_per_snapshot;
                let missed = (tick - newest) / step - 1;
                self.downstream.note(now, missed, 1);
                self.snapshot_tick = Some(tick);
            }
            Some(_) => self.downstream.note(now, 0, 0),
            None => self.snapshot_tick = Some(tick),
        }
        self.render_clock.snapshot(now, tick);
        self.interp.receive(&received);
        self.input_acked = self.input_acked.max(header.input_received);
        let applied = header.commands_applied;
        self.unacked
            .retain(|c| tore_net::sequence_newer(c.number, applied));
        self.stats.inputs_repeated += u64::from(header.inputs_repeated);
        self.stats.input_margin = Some(header.input_margin);
        if let Some(seat) = &mut self.seat {
            let round_trip = self.net.stats().map_or(Duration::ZERO, |s| s.round_trip);
            // Before the host has had an input from this seat its margin
            // means nothing.
            if u64::from(header.input_received) > seat.seated_tick {
                self.input_clock
                    .margin(now, f64::from(header.input_margin), round_trip);
            }
            if let Some(hash) = header.own_hash
                && let Some(ours) = seat.predictor.hash_at(u64::from(tick))
            {
                self.stats.hashes_compared += 1;
                if ours != hash {
                    self.stats.mismatches += 1;
                    self.mismatch = self.mismatch.max(tick);
                }
            }
        }
        Some(tick)
    }

    /// An Own state section: the host's exact state of the plane.
    fn own_state(&mut self, body: &[u8]) {
        let now = self.now;
        if self.seat.is_none() {
            // The host sends its exact states as soon as it has seated the
            // player, and they can overtake the long Seated message. The
            // transport has acknowledged this one, so the host may code the
            // next against it: keep it, in order, for when the seat arrives.
            self.early.push(body.to_vec());
            return;
        }
        let (Some(wire), Some(seat), Some(mission)) = (
            self.wire.as_mut(),
            self.seat.as_mut(),
            self.mission.as_ref(),
        ) else {
            return;
        };
        let (header, state) = match wire.own_state(body, &seat.model) {
            Ok(read) => read,
            Err(_) => {
                self.net.disconnect(DisconnectReason::ProtocolError);
                return;
            }
        };
        self.stats.own_states += 1;
        let before = pose(&seat.predictor.plane().flight);
        let at = seat.predictor.tick();
        let restored =
            seat.predictor
                .restore(u64::from(header.tick), state, &mission.world.terrain);
        let seconds = now.as_secs_f64();
        match restored {
            Ok(Restored::Same) | Ok(Restored::Stale) => {}
            Ok(Restored::Corrected { .. }) | Ok(Restored::Adopted) => {
                let after = pose(&seat.predictor.plane().flight);
                let (feet, degrees) = prediction::difference(&before, &after);
                let adopted = matches!(restored, Ok(Restored::Adopted));
                let shown = if adopted || now.saturating_sub(seat.seated_at) < SEATING_SNAP {
                    seat.offset.clear(seconds);
                    false
                } else {
                    seat.offset.correct(&before, &after, seconds)
                };
                self.stats.corrections += 1;
                self.stats.corrections_shown += u64::from(shown);
                self.stats.adopted += u64::from(adopted);
                self.corrections.push(Correction {
                    tick: u64::from(header.tick),
                    now: at,
                    feet,
                    degrees,
                    shown,
                });
            }
            Err(error) => {
                self.log("prediction-failed", &[&error.to_string()]);
                self.net.disconnect(DisconnectReason::Other(0));
            }
        }
    }

    /// An event from the host: the seat's cues are released at once, the
    /// mission-wide ones when the picture reaches their tick.
    fn route(&mut self, event: ReceivedEvent) {
        let position = |p: &[i64; 3]| p.map(|q| q as f64 * crate::wire::entity::POSITION_STEP);
        match &event.event {
            WireEvent::Message { .. }
            | WireEvent::Radio { .. }
            | WireEvent::Tower { .. }
            | WireEvent::OrderVoice { .. }
            | WireEvent::OrderReply { .. }
            | WireEvent::WeaponCycled
            | WireEvent::Release { .. }
            | WireEvent::Feedback { .. }
            | WireEvent::YourAircraftExploded { .. } => {
                self.released.push(event);
                if self.released.len() > MAX_EVENTS {
                    self.released.remove(0);
                }
                return;
            }
            WireEvent::Effect {
                kind,
                position: at,
                ticks,
                blast,
            } => self.effects.push(Shown {
                tick: event.tick,
                kind: *kind,
                position: position(at),
                ticks: *ticks,
                blast: *blast,
            }),
            WireEvent::Mark { kind, position: at } => {
                self.marks.push((event.tick, *kind, position(at)));
            }
            WireEvent::GroundDestroyed { object } => {
                self.destroyed.entry(*object).or_insert(event.tick);
                if let Some(seat) = &mut self.seat {
                    seat.predictor.destroyed(*object);
                }
            }
            WireEvent::Launch { .. }
            | WireEvent::WingEjection { .. }
            | WireEvent::Countermeasure { .. }
            | WireEvent::GunBurst { .. }
            | WireEvent::Sound { .. } => {}
        }
        // Held in tick order (events arrive in number order, which is
        // nearly tick order).
        let at = self
            .held
            .iter()
            .rposition(|e| e.tick <= event.tick)
            .map_or(0, |i| i + 1);
        self.held.insert(at, event);
        while self.held.len() > MAX_EVENTS {
            self.held.pop_front();
        }
    }

    /// Steps every predicted tick the clock has reached and sends the inputs.
    fn fly(&mut self, now: Duration, frame: InputFrame) {
        self.input_clock.advance(now);
        let target = self.input_margin_target(now);
        if self.settled {
            self.input_clock.steer(now, target);
        } else if self.input_clock.settle(now, target).is_some()
            || self.input_clock.smallest_margin(now).is_some()
        {
            self.settled = true;
        }
        let (Some(seat), Some(mission)) = (self.seat.as_mut(), self.mission.as_ref()) else {
            return;
        };
        let due = self.input_clock.position().floor().max(0.) as u64;
        let mut steps = 0;
        while seat.predictor.tick() < due && steps < MAX_TICKS_PER_UPDATE {
            let tick = seat.predictor.tick() + 1;
            let commands: Vec<Command> = std::mem::take(&mut self.pending);
            for command in &commands {
                self.unacked.push_back(NumberedCommand {
                    number: self.next_command,
                    tick: tick as u32,
                    command: *command,
                });
                self.next_command = self.next_command.wrapping_add(1);
            }
            if let Err(error) = seat.predictor.step(frame, commands, &mission.world.terrain) {
                let text = error.to_string();
                if let Some(d) = &mut self.diagnostics {
                    d.line(now, "prediction-failed", &[&text]);
                }
                self.net.disconnect(DisconnectReason::Other(0));
                return;
            }
            steps += 1;
        }
        self.send_inputs(now);
    }

    /// One input packet, when a tick was stepped since the last and the
    /// packet interval has passed.
    fn send_inputs(&mut self, now: Duration) {
        let Some(seat) = &self.seat else {
            return;
        };
        let newest = seat.predictor.tick();
        if newest <= self.sent_tick
            || self
                .last_sent
                .is_some_and(|last| now.saturating_sub(last) < INPUT_PACKET_INTERVAL)
        {
            return;
        }
        let history = seat.predictor.history();
        let Some(first) = history.front().map(|r| r.tick) else {
            return;
        };
        let oldest = (u64::from(self.input_acked) + 1)
            .max(newest.saturating_sub(INPUT_REDUNDANCY - 1))
            .max(first);
        let frames: Vec<InputFrame> = history
            .iter()
            .filter(|r| r.tick >= oldest && r.tick <= newest)
            .map(|r| r.frame)
            .collect();
        if frames.is_empty() {
            return;
        }
        let view = self.render_clock.render().map_or(0, |render| {
            (newest as f64 - render.floor()).clamp(0., 255.) as u8
        });
        let section = InputsSection {
            newest_tick: newest as u32,
            frames,
            view_offset: view,
            interpolation_delay: self.render_clock.delay().round().clamp(0., 63.) as u8,
            view_subject: self.view_subject,
            mismatch: self.mismatch,
            commands: self
                .unacked
                .iter()
                .filter(|c| u64::from(c.tick) <= newest)
                .take(crate::wire::limits::COMMANDS)
                .copied()
                .collect(),
        };
        let Ok(body) = section.encode() else {
            return;
        };
        if self
            .net
            .send_payload(now, &[(SECTION_INPUTS, &body)])
            .is_ok()
        {
            self.last_sent = Some(now);
            self.sent_tick = newest;
            self.stats.input_packets += 1;
            if let Some(capture) = &mut self.capture {
                capture.sent(now, &body);
            }
        }
        self.pump();
    }

    fn diagnose(&mut self, now: Duration) {
        if self.diagnostics.as_ref().is_none_or(|d| !d.due(now)) {
            return;
        }
        let stats = self.stats();
        if let Some(d) = &mut self.diagnostics {
            d.second(now, &stats);
        }
    }
}

/// Whether an own state section that came before the Seated message can be
/// kept: its baseline, if it has one, is one kept already (at most 64).
fn early_own_state_fits(early: &[Vec<u8>], body: &[u8]) -> bool {
    let Ok(header) = crate::wire::connection::own_state_header(body) else {
        return false;
    };
    early.len() < 64
        && header.baseline().is_none_or(|number| {
            early.iter().any(|kept| {
                crate::wire::connection::own_state_header(kept).is_ok_and(|h| h.number == number)
            })
        })
}

/// The own plane as the picture's player pose, as combat draws it.
fn player_pose(
    plane: u32,
    flight: &flight::State,
    config: &Configuration,
    terms: Option<&tore_world::world::plane::OwnshipTerms>,
) -> AircraftPose {
    let hp = terms.map_or(config.damage_capacity, |t| t.hp);
    AircraftPose {
        id: plane,
        aircraft: Some(config.aircraft),
        draw: Draw::Ownship,
        position: flight.position,
        attitude: [flight.yaw, flight.pitch, flight.bank],
        velocity: flight.velocity,
        devices: Some(tore_world::snapshot::devices(flight)),
        engine: Engine {
            lit: flight.engine && flight.fuel > 0.,
            afterburner: flight.afterburner_active(),
            rates: flight.auxiliary_rates,
            flame: flight.afterburner_active() && flight.escape.is_none() && hp > 0,
        },
        damage: Damage {
            hp,
            initial_hp: config.damage_capacity,
            sections: terms.map_or([0; tore_sim::combat::live::DAMAGE_SECTIONS], |t| {
                t.damage_amounts
            }),
            structural: terms.and_then(|t| t.damage_section),
        },
        airborne: true,
        wreck: flight.wreck.as_ref().map(|w| w.phase),
        crashed: flight.crashed,
    }
}

/// The mission's ground objects as drawn, standing.
fn ground_poses(world: &World) -> Vec<AircraftPose> {
    world
        .combat
        .state
        .targets
        .iter()
        .filter(|t| t.aircraft.is_none())
        .map(|t| AircraftPose {
            id: t.id,
            aircraft: None,
            draw: Draw::Hidden,
            position: t.position,
            attitude: tore_world::combat::target_pose(t, world.combat.ai_poses),
            velocity: t.velocity,
            devices: None,
            engine: Engine {
                lit: true,
                ..Engine::default()
            },
            damage: Damage {
                hp: t.hp,
                initial_hp: t.initial_hp,
                sections: t.localized_damage.amounts,
                structural: t.localized_damage.structural_section,
            },
            airborne: t.airborne,
            wreck: None,
            crashed: t.hp <= 0,
        })
        .collect()
}
