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
//!   the content manifests and tells the host when its import cannot play it.
//!   On Seated it decodes its plane's exact state.
//! - **The lobby** (slice EF4): the lobby's state as the host last sent it
//!   ([`Client::lobby`], [`ClientEvent::Lobby`]), and the calls a lobby
//!   screen makes: take or leave a slot, send a loadout, mark ready, and for
//!   the King change the mission, start, kick and end the mission. With
//!   [`ClientConfig::auto_ready`] (the default) the client takes its slot
//!   (the plane asked for, or the first free one) with the standard loadout
//!   and marks ready by itself whenever it is in the lobby, after a mission
//!   change and after each return, as a game with no lobby screen does.
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
//! - **Content** (stage L, [`content`]): its first message after the join
//!   is its content ([`ClientConfig::content`]); it keeps the host's Content
//!   gaps ([`Client::content_gaps`]) and words its own refusal by item.

mod away;
#[cfg(test)]
mod away_tests;
// Stage K's seams (slice K0): each later slice fills its own.
pub mod candidate;
pub mod capture;
#[cfg(test)]
mod chat_tests;
pub mod clock;
pub mod content;
pub mod convert;
#[cfg(test)]
mod convert_tests;
pub mod diagnostics;
pub mod interpolation;
#[cfg(test)]
mod lobby_tests;
#[cfg(test)]
mod matrix_tests;
pub mod migrate;
#[cfg(test)]
mod migrate_tests;
#[cfg(test)]
mod migration_seams_tests;
pub mod observe;
#[cfg(test)]
mod observe_tests;
#[cfg(test)]
mod phase2_seams_tests;
pub mod prediction;
#[cfg(test)]
mod radar_page_tests;
pub mod rejoin;
#[cfg(test)]
mod rejoin_tests;
#[cfg(test)]
mod relay_tests;
pub mod results;
pub mod revival;
#[cfg(test)]
mod revival_tests;
pub mod scores;
pub mod seen;
pub mod sight;
#[cfg(test)]
mod sight_tests;
#[cfg(test)]
mod stall_tests;
#[cfg(test)]
mod tests;

use crate::host::BuildId;
use crate::wire::chat::{ChatLine, ChatSend, Receiver, Refusal};
use crate::wire::connection::ClientConnection;
use crate::wire::connection::FlightOrder;
use crate::wire::entity::EntityKey;
use crate::wire::events::{ReceivedEvent, WireEvent};
use crate::wire::inputs::{Command, InputFrame, InputsSection, NumberedCommand, quantize_command};
use crate::wire::messages::{
    ContentRefused, Debrief, Goodbye, Kick, LobbyPhase, LobbyState, Lock, Message, Mission,
    MissionEnded, Observe, Observing, Results, Revival, Roster, Scores, Seated, SetReady,
    SettingsChange, Slot, SlotLock, SlotRequest, Spawned, TakePlane,
};
use crate::wire::names::NameIndex;
use crate::wire::own_state::OwnStateHeader;
use crate::wire::{
    PROTOCOL_VERSION, Platform, SECTION_EVENTS, SECTION_INPUTS, SECTION_OWN_STATE, SECTION_SNAPSHOT,
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
use tore_net::master::Path;
use tore_net::{CloseReason, Datagrams, DisconnectReason, Entropy, Event, Target, Transmit};
use tore_sim::combat::blast::MarkKind;
use tore_sim::combat::live::{Configuration, EffectKind};
use tore_sim::flight::{self, PilotInput};
use tore_sim::models::AircraftModel;
use tore_world::WorldResult;
use tore_world::frame::{FlightFrame, ReadoutSlot};
use tore_world::mission::{LoadoutSpec, MissionSpec};
use tore_world::readout::CockpitReadout;
use tore_world::resources::{Manifest, ResourceReads, ResourceSource};
use tore_world::seats::{PlaneId, SeatCommand, SeatId};
use tore_world::snapshot::{
    AircraftPose, Damage, Draw, EffectPose, Engine, MarkPose, PilotPose, RenderSnapshot,
};
use tore_world::world::plane::ExactState;
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
/// A prediction this many ticks behind its clock (a quarter of a second,
/// after a stall) takes the host's newest exact state ahead of it instead of
/// stepping the backlog.
pub const CATCH_UP_TICKS: u64 = 30;
/// A predicted clock less than this many ticks ahead of the host's newest
/// snapshot is behind the host (that snapshot is already in the past) and
/// jumps ahead.
pub const BEHIND_TICKS: f64 = 1.;
/// The longest an exact state waits while the host still repeats late
/// inputs, ticks (125 ms); see `Client::apply_own_states`.
pub const CORRECTION_HOLD_TICKS: u64 = 15;
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
    /// Takes a slot and marks ready by itself whenever it is in the lobby
    /// (the default): `plane`'s slot, or after a refusal the first free one,
    /// with the standard loadout. A lobby screen turns it off and calls
    /// [`Client::take_slot`], [`Client::set_ready`] and the rest itself.
    pub auto_ready: bool,
    /// The operating system this game runs on, which the host shows beside
    /// the callsign in the lobby: this build's own by default.
    pub platform: Platform,
    /// A join through the master (stage J, slice J2): every address the
    /// master gave for the host, raced at once, and the introduction whose
    /// punches add more. `None` joins `server` alone.
    pub race: Option<Race>,
    /// This game's content (stage L), sent as its first message after the
    /// join. `None` computes it from the resources at [`Client::connect`],
    /// with the source the pack's entry gives; the game passes the one it
    /// computed on a worker, with the source read from the import report too.
    pub content: Option<Arc<crate::host::content::GameContent>>,
    /// The rejoin token this game holds for the session it joins (stage K,
    /// slice K5), sent in the Challenge answer: the host admits it whatever
    /// the room and makes the connection that player again. `None` joins as
    /// a new player.
    pub token: Option<tore_net::Token>,
}

/// The host's addresses from the master's introduction, for a join that
/// races them ([`tore_net::Client::connect_any`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Race {
    /// The host's addresses and the path each stands for, the master's
    /// order.
    pub targets: Vec<Target>,
    /// The introduction id the host's punches carry.
    pub introduction: u64,
}

impl ClientConfig {
    /// A join to `server` as `callsign` with this `build`: no password, any
    /// plane, system entropy, this build's platform.
    pub fn new(server: SocketAddr, callsign: &str, build: BuildId) -> Self {
        Self {
            server,
            callsign: callsign.to_owned(),
            password: String::new(),
            build,
            plane: None,
            entropy: Entropy::System,
            retail_stall_speeds: false,
            auto_ready: true,
            platform: Platform::current(),
            race: None,
            content: None,
            token: None,
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
    /// The AC-130 gunsight's slew, [x, y], right and up positive, -127 to
    /// 127 (protocol 21).
    pub sight: [i8; 2],
    /// The target camera's zoom step, 1 to 6; 0 means the default step.
    pub sight_zoom: u8,
    /// Seat commands given, in order.
    pub commands: Vec<SeatCommand>,
    /// What the view follows when it is not the own cockpit.
    pub view_subject: Option<EntityKey>,
}

impl Controls {
    /// Neutral controls for while a menu is up or the window lost focus:
    /// stick centred, throttle held, trigger released, no sight slew, the
    /// scope as it is. The caller keeps the sight's zoom step.
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
            frame: InputFrame::of(&controls.pilot, controls.trigger, controls.sensors)
                .with_sight(controls.sight, controls.sight_zoom),
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
    /// In the lobby, the mission loaded (or refused): taking a slot, arming,
    /// marking ready, or waiting for the next mission.
    Lobby,
    /// Asked for a plane.
    Seating,
    /// Flying a plane.
    Flying,
    /// Sent Leave; waiting for the debrief, then back in the lobby.
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
    /// The mission is built from the import and matches the host's: the
    /// first one, the King's change, or a flight's start.
    MissionLoaded,
    /// The import differs from the host's in these resources, or cannot
    /// build the mission (`reason`): the client told the host and stays in
    /// the lobby, marked unable, until the mission changes.
    ContentRefused { names: Vec<String>, reason: String },
    /// The mission text could not be read: the client leaves.
    MissionFailed(String),
    /// The lobby changed; read it with [`Client::lobby`].
    Lobby,
    /// A lobby request was refused: the request's message kind
    /// ([`crate::wire::messages::kind`]) and why.
    Refused { request: u8, reason: String },
    /// The host is about to disconnect the player, and why; the
    /// [`ClientEvent::Closed`] that follows reads with [`Client::close_text`].
    Goodbye(Goodbye),
    /// No plane: the reason. The client may ask again with [`Client::ready`].
    SeatRefused(String),
    /// The player has a plane.
    Seated { seat: u8, plane: u32, tick: u32 },
    /// The roster changed; read it with [`Client::roster`].
    Roster,
    /// A line from the server for the HUD.
    Notice(String),
    /// A chat line (protocol 4): another player's, the player's own sent
    /// back, or the host's words (a refusal, or that no one heard).
    Chat(ChatLine),
    /// The player's debrief.
    Debrief(Box<Debrief>),
    /// The host ended the mission.
    MissionEnded(MissionEnded),
    /// The connection ended; nothing follows.
    Closed(CloseReason),
    // Stage F phase 2 (protocol 8). The client passes these on; the slices
    // that build each part act on them.
    /// The player's plane is lost: whether and when it may fly again
    /// ([`Client::revive`], [`Client::revival`]; slice F2-V).
    Revival(Box<Revival>),
    /// A revival's new plane, which the client has added to its copy of the
    /// mission ([`Client::spawned`]; slice F2-V).
    Spawned(Box<Spawned>),
    /// The scores changed (slice F2-S); the newest are also kept
    /// ([`Client::scores`]).
    Scores(Box<Scores>),
    /// Every plane's results at the mission's end (slice F2-D).
    Results(Box<Results>),
    /// The observer flight starts or ends (slice F2-O1).
    Observing(Box<Observing>),
}

/// The plain words for a mission's end, for the player.
pub fn ended_text(ended: &MissionEnded) -> String {
    use crate::wire::messages::EndReason;
    let why = match ended.reason {
        EndReason::EveryoneLeft => "Mission ended: everyone left",
        EndReason::TimeLimit => "Mission ended: the time limit",
        EndReason::ServerStopping => "Mission ended: the server is stopping",
        EndReason::EndedByServer => "Mission ended by the host",
        EndReason::HostLeft => "Mission ended: the host left the game",
        EndReason::KillLimit => "Mission ended: the kill limit",
    };
    match ended.next_in_seconds {
        Some(0) | None => format!("{why}."),
        Some(seconds) => format!("{why}; the next starts in {seconds} seconds."),
    }
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
                DisconnectReason::MovedToNewHost => "the player moved to the game's new host",
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
    /// Exact states taken to catch up a prediction far behind its clock
    /// (after a long stall), not counted as corrections.
    pub catch_ups: u64,
    /// Times the predicted clock was found behind the host's newest
    /// snapshot and jumped ahead.
    pub behind: u64,
    pub frames: u64,
    /// Entities drawn over all frames, and of them past their newest state.
    pub entity_frames: u64,
    pub extrapolated: u64,
    /// Of those, the ones drawn with an extra delay (sent twice a second),
    /// and of them the ones past their newest state.
    pub far_frames: u64,
    pub far_extrapolated: u64,
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
    /// The newest cockpit readout received, its contacts' bearings,
    /// elevations and distances worked out around `presented`.
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

/// What a mission's build is compared with.
#[derive(Clone, Copy)]
enum Check<'a> {
    /// The host's whole manifest: the Mission message's.
    Whole(&'a Manifest),
    /// Only these entries: what a flight's loadouts add.
    Added(&'a Manifest),
    /// Nothing: the lobby's mission again, checked before.
    None,
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
    /// The AC-130 gunsight's look, turned ahead of the host.
    sight: sight::SightPrediction,
    seated_at: Duration,
    seated_tick: u64,
    /// Initial forecast span: the host has no controls for its elapsed part.
    bootstrap_until: u64,
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
    /// Exact states received and read, applied once the prediction has
    /// stepped to the update's time: after a stall the queued ones then
    /// meet a prediction that has caught up instead of being adopted one by
    /// one ahead of it.
    pending_own: Vec<(OwnStateHeader, ExactState)>,
    /// The inputs the newest snapshot said the host repeated.
    repeats_reported: u8,
    /// The snapshot tick a held exact state began waiting at.
    holding_since: Option<u64>,
    /// The newest input tick whose margin the clock has been given.
    margin_input: u32,
    /// The lobby as the host last sent it.
    lobby: Option<LobbyState>,
    /// The newest scores of the mission flying (slice F2-S).
    scores: Option<scores::Kept>,
    /// The results of the mission that ended (slice F2-D).
    results: Option<Box<Results>>,
    /// The newest Revival since the player's plane was lost, and the
    /// revivals' new planes of the mission (slice F2-V, [`revival`]).
    revival: Option<revival::Kept>,
    spawned: Vec<Spawned>,
    /// The newest Mission message's number, and the number of the newest
    /// mission built and matched.
    number: Option<u32>,
    loaded: Option<u32>,
    /// Why this import cannot play the mission, when it cannot, and
    /// whether only a flight's build (its loadouts) failed.
    unable: Option<String>,
    unable_flight: bool,
    /// Why the host said it disconnects the player.
    goodbye: Option<Goodbye>,
    /// The automatic ready ([`ClientConfig::auto_ready`]): a Take plane is
    /// on its way; the plane to ask for (`None` after a refusal: any); a
    /// refused request waits for the lobby to change before asking again.
    auto_pending: bool,
    auto_plane: Option<u32>,
    auto_wait: bool,
    /// The player is leaving the game: disconnect once the debrief is in.
    quit_after_debrief: bool,
    /// The clock was set from the host's first margin since seating.
    settled: bool,
    upstream: LossWindow,
    downstream: LossWindow,
    stats: ClientStats,
    corrections: Vec<Correction>,
    diagnostics: Option<Diagnostics>,
    capture: Option<CaptureWriter>,
    /// Watching the flying mission (stage F phase 2, [`observe`]).
    watching: Option<observe::Watching>,
    /// The AI flies the plane while the player is away (stage F phase 2,
    /// [`away`]).
    away: away::Away,
    /// What the session was given, kept for converting a capture into a
    /// replay ([`seen`]); off in a game.
    observed: Option<seen::Observed>,
    /// Stage K's migration: the standby records passed on, and what slice
    /// K4 adds.
    migration: migrate::Migration,
    /// The rejoin token the host granted, and where it is kept (stage K,
    /// slice K5).
    rejoin: rejoin::Kept,
    /// Host selection: the Candidate report, the tests (slice K6).
    candidacy: candidate::Candidacy,
    /// This game's content, and the host's newest Content gaps (stage L).
    content: Arc<crate::host::content::GameContent>,
    gaps: Option<crate::wire::messages::ContentGaps>,
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
        let net_config = tore_net::ClientConfig {
            game_version: config.build.version.clone(),
            game_commit: config.build.commit.clone(),
            password: config.password.clone(),
            platform: config.platform,
            token: config.token,
            entropy: Entropy::Seeded(seed),
            max_section_kind: crate::wire::SECTION_FILLER,
            ..tore_net::ClientConfig::new(PROTOCOL_VERSION, &config.callsign)
        };
        let net = match &config.race {
            Some(race) => tore_net::Client::connect_any(
                net_config,
                &race.targets,
                Some(race.introduction),
                now,
            ),
            None => tore_net::Client::connect(net_config, config.server, now),
        }
        .map_err(|error| ClientError::Config(error.to_string()))?;
        let content = config
            .content
            .clone()
            .unwrap_or_else(|| crate::host::content::GameContent::shared(&resources));
        Ok(Client {
            content,
            gaps: None,
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
            pending_own: Vec::new(),
            repeats_reported: 0,
            holding_since: None,
            margin_input: 0,
            lobby: None,
            scores: None,
            results: None,
            revival: None,
            spawned: Vec::new(),
            number: None,
            loaded: None,
            unable: None,
            unable_flight: false,
            goodbye: None,
            auto_pending: false,
            auto_plane: None,
            auto_wait: false,
            quit_after_debrief: false,
            settled: false,
            upstream: LossWindow::default(),
            downstream: LossWindow::default(),
            stats: ClientStats::default(),
            corrections: Vec::new(),
            diagnostics: None,
            capture: None,
            watching: None,
            away: away::Away::default(),
            observed: None,
            migration: migrate::Migration::default(),
            rejoin: rejoin::Kept::default(),
            candidacy: candidate::Candidacy::default(),
            now,
        })
        .map(|mut client| {
            client.auto_plane = client.config.plane;
            client
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

    /// Keeps what the session is given ([`seen::Observed`]), from here
    /// on, for converting a capture into a replay. Set it right after the
    /// client starts.
    pub fn start_observing(&mut self) {
        self.observed.get_or_insert_with(seen::Observed::default);
    }

    /// Ends the observation and hands over what was kept: the flight in
    /// progress is closed at the client's time.
    pub fn finish_observing(&mut self) -> Option<seen::Observed> {
        self.flush_observed();
        let names = self.wire.as_ref().map(|wire| wire.names.clone());
        let now = self.now;
        let mut observed = self.observed.take()?;
        observed.end_flight(now, names.as_ref());
        Some(observed)
    }

    /// Moves the predictor's trace to the observer.
    fn flush_observed(&mut self) {
        if let (Some(observed), Some(seat)) = (&mut self.observed, &mut self.seat) {
            observed.trace(seat.predictor.take_trace());
        }
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
        // Stage K: the race to a new host and the old connection take their
        // own datagrams (slice K4).
        if self.migrate_receive(now, from, datagram) {
            return;
        }
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
        // Stage K: detection, the race and the resume (slice K4).
        self.migrate_update(now);
        self.pending.extend(sampled.commands);
        self.view_subject = sampled.view_subject;
        self.observe_update(now);
        if self.phase == ClientPhase::Flying {
            self.fly(now, sampled.frame);
            self.apply_own_states();
            self.flush_observed();
        }
        // Stage K: the predicted ticks kept for a backlog (slice K4).
        self.migrate_note_flight();
        self.auto_ready();
        self.candidate_update(now);
        let margin = self.interpolation_margin(now);
        self.render_clock.advance(now, margin);
        let lossy = self.downstream.loss(now) > HIGH_LOSS;
        self.interp.advance(now, lossy);
        self.diagnose(now);
    }

    /// The next datagram to send, oldest first.
    pub fn poll_transmit(&mut self) -> Option<Transmit> {
        // Stage K: the race's and the old connection's first (slice K4).
        self.migrate_poll_transmit()
            .or_else(|| self.net.poll_transmit())
    }

    /// Sends every queued datagram on `socket`.
    pub fn transmit<D: Datagrams + ?Sized>(&mut self, socket: &mut D) -> io::Result<()> {
        while let Some(t) = self.migrate_poll_transmit() {
            socket.send_datagram(t.to, &t.datagram)?;
        }
        self.net.transmit(socket)
    }

    /// The transport's Keepalive packet for this connection, once joined
    /// (slice EF-K): what the game's keepalive thread ([`tore_net::Keepalive`])
    /// sends for it while the game's loop is stalled. `None` while joining and
    /// once the connection has closed.
    pub fn keepalive_datagram(&self) -> Option<Vec<u8>> {
        self.net.keepalive_datagram()
    }

    /// The host's address: the one the race chose, for a join through the
    /// master.
    pub fn server(&self) -> SocketAddr {
        self.net.server()
    }

    /// How the player reached the host (stage J): the path of the address
    /// joined, which the Challenge answer told the host.
    pub fn path(&self) -> Path {
        self.net.path()
    }

    /// True once the join's address is settled: given one, or the race's
    /// first answer chose it.
    pub fn chosen(&self) -> bool {
        self.net.chosen()
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

    /// Takes a plane at once: in the lobby `plane`'s slot (or the first
    /// free one) and ready; in flight, that plane (or any) now. Stage D's
    /// call, for a game with no lobby screen.
    pub fn ready(&mut self, plane: Option<u32>) {
        let now = self.now;
        self.request(
            now,
            Message::TakePlane(TakePlane {
                mission: self.number.unwrap_or(0),
                plane,
            }),
        );
    }

    /// The player ends its flight: Leave goes to the host, which sends the
    /// debrief, gives the plane back to the AI and keeps the player in the
    /// lobby, still connected.
    pub fn leave(&mut self, now: Duration) {
        if let Some(capture) = &mut self.capture {
            capture.leave(now);
        }
        self.now = self.now.max(now);
        if matches!(self.phase, ClientPhase::Seating | ClientPhase::Flying) {
            self.send(&Message::Leave);
            self.phase = ClientPhase::Leaving;
            self.net.update(self.now);
            self.pump();
        }
    }

    /// The player leaves the game: a flying player ends its flight and the
    /// client quits once the debrief is in (the caller gives it a few
    /// seconds); anywhere else it quits at once.
    pub fn leave_game(&mut self, now: Duration) {
        if let Some(capture) = &mut self.capture {
            capture.leave_game(now);
        }
        self.now = self.now.max(now);
        match self.phase {
            ClientPhase::Seating | ClientPhase::Flying | ClientPhase::Leaving => {
                self.quit_after_debrief = true;
                if self.phase != ClientPhase::Leaving {
                    self.send(&Message::Leave);
                    self.phase = ClientPhase::Leaving;
                }
                self.net.update(self.now);
                self.pump();
            }
            ClientPhase::Closed => {}
            _ => {
                self.net.disconnect(DisconnectReason::Left);
                self.pump();
            }
        }
    }

    // ----- The lobby -----------------------------------------------------

    /// Ticks between the host's snapshots to this player: the host's
    /// Accepted packet gives them at the join, and each flight starts from
    /// the snapshot rate in the lobby's settings (slice R1).
    pub fn ticks_per_snapshot(&self) -> u32 {
        self.ticks_per_snapshot
    }

    /// The tick of the newest snapshot of the flight, once one has come.
    pub fn snapshot_tick(&self) -> Option<u32> {
        self.snapshot_tick
    }

    /// The lobby as the host last sent it; `None` before the first.
    pub fn lobby(&self) -> Option<&LobbyState> {
        self.lobby.as_ref()
    }

    /// Why this game's import cannot play the lobby's mission, when it
    /// cannot (the King sees it in the lobby too).
    pub fn unable(&self) -> Option<&str> {
        self.unable.as_deref()
    }

    /// This game's content (stage L): what it sent the host.
    pub fn content(&self) -> &crate::host::content::GameContent {
        &self.content
    }

    /// The host's newest Content gaps (stage L): the items not every
    /// player can use; `None` before the first arrives.
    pub fn content_gaps(&self) -> Option<&crate::wire::messages::ContentGaps> {
        self.gaps.as_ref()
    }

    /// Why a choice of the item of `kind` and `key` would be refused, in the
    /// host's words, when it is in a gap now ([`content::gap_refusal`]).
    pub fn gap_refusal(&self, kind: crate::wire::messages::ItemKind, key: &str) -> Option<String> {
        content::gap_refusal(self.gaps.as_ref()?, self.lobby.as_ref(), kind, key)
    }

    /// Hold `plane`'s slot. The answer is the next lobby state, or a
    /// [`ClientEvent::Refused`].
    pub fn take_slot(&mut self, plane: u32) {
        self.slot_request(SlotRequest::Take(plane));
    }

    /// Hold the first free slot.
    pub fn take_any_slot(&mut self) {
        self.slot_request(SlotRequest::Any);
    }

    /// Hold the first free slot on `side` (a PvP lobby's Bluefor and Redfor
    /// boxes), or keep the slot held there. The host refuses it in co-op,
    /// under Autobalance, while the player holds a slot on the other side,
    /// when lock sides fixed the side in flight, and when the side is full.
    pub fn take_side_slot(&mut self, side: tore_sim::ai::launch::Side) {
        self.slot_request(SlotRequest::Side(side));
    }

    /// Hold no slot.
    pub fn leave_slot(&mut self) {
        self.slot_request(SlotRequest::Leave);
    }

    fn slot_request(&mut self, request: SlotRequest) {
        let now = self.now;
        self.request(
            now,
            Message::Slot(Slot {
                mission: self.number.unwrap_or(0),
                request,
            }),
        );
    }

    /// The loadout for the slot this player holds (`None`: the aircraft's
    /// standard load). The host checks it with the Load Ordnance page's rules
    /// and refuses one they do not allow, with the reason.
    pub fn send_loadout(&mut self, loadout: Option<LoadoutSpec>) {
        let Some(plane) = self
            .lobby
            .as_ref()
            .and_then(|l| l.me())
            .and_then(|me| me.slot)
        else {
            self.event(ClientEvent::Refused {
                request: crate::wire::messages::kind::LOADOUT,
                reason: "Take a slot first.".into(),
            });
            return;
        };
        let now = self.now;
        self.request(
            now,
            Message::Loadout(Box::new(crate::wire::messages::Loadout {
                mission: self.number.unwrap_or(0),
                plane,
                loadout,
            })),
        );
    }

    /// Ready, or not. In flight, a player holding a slot who gets ready
    /// takes that plane.
    pub fn set_ready(&mut self, ready: bool) {
        let now = self.now;
        self.request(
            now,
            Message::SetReady(SetReady {
                mission: self.number.unwrap_or(0),
                ready,
            }),
        );
    }

    /// The King's new mission for everyone, in the lobby.
    pub fn change_mission(&mut self, spec: &MissionSpec) {
        let now = self.now;
        self.request(now, Message::ChangeMission(spec.to_text()));
    }

    /// The King starts the mission: accepted when every player holding a
    /// slot is ready.
    pub fn start_mission(&mut self) {
        let now = self.now;
        self.request(now, Message::Start);
    }

    /// The King removes the player with lobby id `player`, who is told
    /// `reason`.
    pub fn kick(&mut self, player: u8, reason: &str) {
        let now = self.now;
        self.request(
            now,
            Message::Kick(Kick {
                player,
                reason: reason.to_owned(),
            }),
        );
    }

    /// The King ends the mission for everyone: every player gets the
    /// debrief and the lobby returns.
    pub fn end_mission(&mut self) {
        let now = self.now;
        self.request(now, Message::EndMission);
    }

    // ----- Stage F phase 2 (protocol 8) ---------------------------------

    /// The King gives the crown to the player with lobby id `player`.
    pub fn pass_crown(&mut self, player: u8) {
        let now = self.now;
        self.request(now, Message::PassCrown(player));
    }

    /// The King changes the lobby's settings, all or none
    /// ([`crate::settings`]).
    pub fn change_settings(&mut self, change: SettingsChange) {
        let now = self.now;
        self.request(now, Message::Settings(Box::new(change)));
    }

    /// The King opens, closes or reserves `plane`'s slot.
    pub fn lock_slot(&mut self, plane: u32, lock: Lock) {
        let now = self.now;
        self.request(
            now,
            Message::SlotLock(Box::new(SlotLock {
                mission: self.number.unwrap_or(0),
                plane,
                lock,
            })),
        );
    }

    /// Fly again after a loss, by the respawn rule: the answer is a
    /// [`ClientEvent::Seated`] or a [`ClientEvent::Refused`].
    pub fn revive(&mut self) {
        let now = self.now;
        self.request(
            now,
            Message::Revive {
                mission: self.number.unwrap_or(0),
            },
        );
    }

    /// Start or stop watching the flying mission, or move the camera
    /// ([`Client::watch`], [`Client::stop_watching`]).
    pub fn observe(&mut self, observe: Observe) {
        match observe {
            Observe::Watch(subject) => self.watch(subject),
            Observe::Stop => self.stop_watching(),
        }
    }

    /// Sends a lobby request (any message a player's game sends but Leave),
    /// recorded in the capture so a replay sends it again.
    pub fn request(&mut self, now: Duration, message: Message) {
        if !message.from_player() || matches!(message, Message::Leave) {
            return;
        }
        if let Some(capture) = &mut self.capture
            && let Ok(body) = message.encode()
        {
            capture.request(now, message.kind(), &body);
        }
        self.now = self.now.max(now);
        self.send_request(message);
    }

    /// Sends a lobby request, not recorded: the automatic ready's, which a
    /// replay makes again by itself.
    fn send_request(&mut self, message: Message) {
        if matches!(self.phase, ClientPhase::Connecting | ClientPhase::Closed) {
            return;
        }
        if let Message::TakePlane(_) = message
            && matches!(self.phase, ClientPhase::Lobby | ClientPhase::Loading)
            && self
                .lobby
                .as_ref()
                .is_some_and(|l| l.phase == LobbyPhase::Flying)
        {
            self.phase = ClientPhase::Seating;
        }
        self.send(&message);
    }

    // ----- Chat ----------------------------------------------------------

    /// Sends a typed line to `receiver`. The host routes it ([`crate::wire::chat`]);
    /// the answers are [`ClientEvent::Chat`]s: the line back as sent, and
    /// the host's words if it is refused or no one hears it. A line the
    /// rules refuse before it is sent (nothing but spaces, too long, not
    /// printable ASCII, anyone but All before flight) comes back as the
    /// refusal, for the caller to show.
    pub fn chat(&mut self, receiver: Receiver, text: &str) -> Result<(), Refusal> {
        self.chat_send(ChatSend::typed(receiver, text))
    }

    /// Sends one of `CHAT.TXT`'s lines (`number` 1 to 12, the F key) with
    /// its sound, to the line's own receiver or `picked`, the receiver the
    /// player has chosen.
    pub fn chat_quick(
        &mut self,
        number: u8,
        line: &tore_formats::chat::QuickMessage,
        picked: Receiver,
    ) -> Result<(), Refusal> {
        self.chat_send(ChatSend::quick(number, line, picked))
    }

    /// Sends a chat line, checked first by the rules the host holds the
    /// player to (apart from the rate, which only the host counts).
    pub fn chat_send(&mut self, send: ChatSend) -> Result<(), Refusal> {
        if matches!(self.phase, ClientPhase::Connecting | ClientPhase::Closed) {
            return Err(Refusal::NotConnected);
        }
        let send = send.checked()?;
        if send.receiver != Receiver::All && self.phase != ClientPhase::Flying {
            return Err(Refusal::OnlyAll);
        }
        let now = self.now;
        self.request(now, Message::ChatSend(send));
        Ok(())
    }

    /// Why the host said goodbye, when it did.
    pub fn goodbye(&self) -> Option<&Goodbye> {
        self.goodbye.as_ref()
    }

    /// The plain words for why the connection ended: the host's goodbye
    /// when it said one ("The host left the game"), else [`describe`]. A
    /// player who was removed is told by whom: the King, or the server when
    /// the game has no King (the lobby state says: a dedicated server's
    /// `king` is empty).
    pub fn close_text(&self, reason: &CloseReason) -> String {
        let remover = if self.lobby.as_ref().is_some_and(|l| l.king.is_none()) {
            "The server"
        } else {
            "The King"
        };
        match (&self.goodbye, reason) {
            (Some(Goodbye::HostLeft), CloseReason::Disconnected { .. }) => {
                "The host left the game.".into()
            }
            (Some(Goodbye::Kicked(why)), CloseReason::Disconnected { .. }) if why.is_empty() => {
                format!("{remover} removed you from the game.")
            }
            (Some(Goodbye::Kicked(why)), CloseReason::Disconnected { .. }) => {
                format!("{remover} removed you from the game: {why}")
            }
            _ => describe(reason),
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

    /// The AC-130 gunsight's look as the client turns it, while seated.
    pub fn sight_prediction(&self) -> Option<&sight::SightPrediction> {
        self.seat.as_ref().map(|s| &s.sight)
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
        let lossy = self.downstream.loss(now) > HIGH_LOSS;
        self.interp.advance(now, lossy);
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
        self.stats.far_frames += drawn.far as u64;
        self.stats.far_extrapolated += drawn.far_extrapolated as u64;

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
                crew: false,
            })
            .chain(drawn.pilots)
            .collect();
        let mut picture = RenderSnapshot {
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
        // The newest readout, its contacts placed around the drawn plane.
        let mut readout = self.wire.as_ref().and_then(|wire| {
            wire.cockpit_readout(&presented, Some(&mission.world.terrain.airport_scene))
                .and_then(Result::ok)
        });
        let seat_id = seat.seat;
        let predicted_tick = seat.predictor.tick();
        let own_flight = own.flight.clone();
        let own_previous = own.previous_flight.clone();
        // The gunsight's look: the client's own turn, corrected to the
        // host's (gunsight plan 2.13).
        if let Some(seat) = self.seat.as_mut()
            && let Some(gunsight) = readout.as_mut().and_then(|r| {
                let tick = r.tick;
                r.gunsight.as_mut().map(|g| (tick, g))
            })
        {
            let (tick, gunsight) = gunsight;
            let corrected = seat.sight.correct(
                tick,
                gunsight,
                seat.predictor.history(),
                &seat.predictor.plane().flight,
            );
            if corrected == sight::Corrected::Snapped
                && let Some(d) = &mut self.diagnostics
            {
                d.line(now, "sight-snapped", &[&tick.to_string()]);
            }
            gunsight.look = seat.sight.presented();
        }
        if let Some(readout) = &readout {
            presented.gun_aim = std::array::from_fn(|mount| {
                [
                    readout.stores.gun_aim[mount * 2],
                    readout.stores.gun_aim[mount * 2 + 1],
                ]
            });
            presented.gun_group = readout.stores.gun_group;
            if let Some(devices) = &mut picture.player.devices {
                devices[15..21].copy_from_slice(&readout.stores.gun_aim);
                devices[21] = f64::from(readout.stores.gun_group);
            }
        }
        Some(ClientFrame {
            seat: seat_id,
            plane: PlaneId(plane),
            tick: predicted_tick,
            flight: own_flight,
            previous: own_previous,
            presented,
            picture,
            render_tick: render,
            readout,
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
                    self.rejoin_session(welcome.session_id);
                    self.log(
                        "joined",
                        &[
                            &format!("{:016x}", welcome.session_id),
                            &welcome.host_tick.to_string(),
                        ],
                    );
                    // Stage L: the content first, before anything else.
                    if let Some(content) = self.content.message() {
                        self.send(&Message::Content(Box::new(content)));
                    }
                    self.event(ClientEvent::Connected {
                        session_id: welcome.session_id,
                        ticks_per_snapshot: welcome.ticks_per_snapshot,
                        host_tick: welcome.host_tick,
                    });
                }
                tore_net::ClientEvent::Closed(reason) => {
                    // Stage K: kept quiet while a migration races (K4).
                    if self.migrate_closed(&reason) {
                        continue;
                    }
                    self.phase = ClientPhase::Closed;
                    self.rejoin_closed();
                    let kind = match reason {
                        CloseReason::Refused { .. } => "refused",
                        _ => "closed",
                    };
                    // The log says what the screen says.
                    let text = self.close_text(&reason);
                    self.log(kind, &[&text]);
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
                    // Only a host hears keepalives.
                    Event::Stalled | Event::Resumed { .. } => {}
                },
            }
        }
    }

    fn message(&mut self, message: Message) {
        // Stage F phase 2: the idle rule reads what it needs first.
        self.away_message(&message);
        match message {
            Message::Mission(mission) => {
                self.scores = None;
                self.results = None;
                self.revival = None;
                self.spawned.clear();
                self.mission_arrived(mission);
            }
            Message::Roster(roster) => {
                if let Some(observed) = &mut self.observed {
                    observed.roster(&roster);
                }
                self.roster = Some(roster);
                self.event(ClientEvent::Roster);
            }
            Message::Seated(seated) => self.seated(*seated),
            Message::SeatRefused(reason) => {
                if self.phase == ClientPhase::Seating {
                    self.phase = ClientPhase::Lobby;
                }
                self.log("seat-refused", &[&reason]);
                self.auto_refused();
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
            Message::ChatLine(line) => {
                self.log("chat", &[&line.log_text()]);
                self.event(ClientEvent::Chat(line));
            }
            Message::Debrief(debrief) => {
                self.log("debrief", &[]);
                self.event(ClientEvent::Debrief(debrief));
                // The flight is over for this player: back in the lobby.
                if self.phase == ClientPhase::Leaving {
                    self.end_flight();
                }
                if self.quit_after_debrief {
                    self.net.disconnect(DisconnectReason::Left);
                }
            }
            Message::MissionEnded(ended) => {
                self.log("mission-ended", &[&format!("{:?}", ended.reason)]);
                self.revival = None;
                self.end_flight();
                self.event(ClientEvent::MissionEnded(ended));
                // Only the flight's build failed here: back in the lobby the
                // lobby's mission, which this import plays, is built again.
                if ended.next_in_seconds.is_some()
                    && std::mem::take(&mut self.unable_flight)
                    && let (Some(mut spec), Some(number)) = (self.spec.clone(), self.number)
                {
                    self.unable = None;
                    spec.plane_loadouts.clear();
                    self.build_mission(spec, number, Check::None);
                }
            }
            Message::Lobby(lobby) => {
                if lobby.me().is_some_and(|me| me.ready || me.flying) {
                    self.auto_pending = false;
                }
                if self.lobby.as_ref() != Some(&*lobby) {
                    self.auto_wait = false;
                }
                self.lobby = Some(*lobby);
                self.event(ClientEvent::Lobby);
            }
            Message::Refused { request, reason } => {
                self.log("refused", &[&request.to_string(), &reason]);
                if request == crate::wire::messages::kind::OBSERVE {
                    self.watch_refused();
                }
                self.auto_refused();
                self.event(ClientEvent::Refused { request, reason });
            }
            Message::Goodbye(goodbye) => {
                self.goodbye = Some(goodbye.clone());
                self.event(ClientEvent::Goodbye(goodbye));
            }
            Message::FlightLoadouts(loadouts) => {
                // A mission starts flying: its scores and revivals start
                // afresh.
                self.scores = None;
                self.results = None;
                self.revival = None;
                self.spawned.clear();
                self.flight_loadouts(loadouts);
            }
            // Stage F phase 2.
            Message::Revival(revival) => self.revival_message(revival),
            Message::Spawned(spawned) => self.spawned_message(spawned),
            Message::Scores(scores) => {
                self.log("scores", &[&scores::summary(&scores)]);
                self.scores = Some(scores::Kept {
                    scores: (*scores).clone(),
                    received: self.now,
                });
                self.event(ClientEvent::Scores(scores));
            }
            Message::Results(results) => {
                self.log("results", &[&results::summary(&results)]);
                self.results = Some(results.clone());
                self.event(ClientEvent::Results(results));
            }
            Message::Observing(observing) => self.observing_message(*observing),
            // Stage K (protocol 13): each later slice's module acts on its
            // own; slice K0 passes the standby records on.
            Message::Token(grant) => self.rejoin_token(grant),
            message @ (Message::ReachTest(_) | Message::ReachPeers(_) | Message::UploadTest(_)) => {
                self.candidate_message(message);
            }
            message @ (Message::Succession(_)
            | Message::StandbyRecord(_)
            | Message::Resumed(_)
            | Message::HostMoving(_)) => self.migrate_message(message),
            // Stage L: what not everyone can use. Read with
            // `content_gaps`; the lobby's lines change with it.
            Message::ContentGaps(gaps) => {
                self.log("content-gaps", &[&gaps.gaps.len().to_string()]);
                self.gaps = Some(*gaps);
                if self.lobby.is_some() {
                    self.event(ClientEvent::Lobby);
                }
            }
            // Client-to-host messages from the host break the protocol.
            _ => self.net.disconnect(DisconnectReason::ProtocolError),
        }
    }

    /// The player's flight is over (it left, or the mission ended): no
    /// plane, back in the lobby.
    fn end_flight(&mut self) {
        if matches!(self.phase, ClientPhase::Closed | ClientPhase::Connecting) {
            return;
        }
        self.flush_observed();
        if self.observed.is_some() {
            let names = self.wire.as_ref().map(|wire| wire.names.clone());
            let now = self.now;
            if let Some(observed) = &mut self.observed {
                observed.end_flight(now, names.as_ref());
            }
        }
        self.seat = None;
        self.pending_own.clear();
        self.holding_since = None;
        self.early.clear();
        self.auto_pending = false;
        if self.phase != ClientPhase::Loading {
            self.phase = ClientPhase::Lobby;
        }
    }

    /// A lobby request of the automatic ready was refused: ask for any slot
    /// next time, once the lobby has changed.
    fn auto_refused(&mut self) {
        if self.auto_pending {
            self.auto_pending = false;
            if self.auto_plane.is_none() {
                self.auto_wait = true;
            }
            self.auto_plane = None;
        }
    }

    /// The automatic ready ([`ClientConfig::auto_ready`]): in the lobby with
    /// the mission loaded and not ready, take the slot asked for (or any)
    /// and mark ready.
    fn auto_ready(&mut self) {
        if !self.config.auto_ready
            || self.auto_pending
            || self.auto_wait
            || self.quit_after_debrief
            || self.unable.is_some()
            || self.watching.is_some()
            || self.phase != ClientPhase::Lobby
        {
            return;
        }
        let Some(lobby) = &self.lobby else {
            return;
        };
        if self.loaded != Some(lobby.mission)
            || self.number != Some(lobby.mission)
            || lobby.phase == LobbyPhase::Ended
            || lobby.me().is_none_or(|me| me.ready || me.flying)
        {
            return;
        }
        let plane = self.auto_plane.or(lobby.me().and_then(|me| me.slot));
        self.auto_pending = true;
        let mission = lobby.mission;
        self.send_request(Message::TakePlane(TakePlane { mission, plane }));
    }

    /// The mission: built from the import and its manifest compared. A
    /// player whose import cannot play it tells the host and stays in the
    /// lobby, marked unable; one whose mission text cannot be read leaves.
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
        self.number = Some(mission.number);
        self.auto_pending = false;
        self.auto_wait = false;
        self.log("mission", &[&mission.host_tick.to_string()]);
        self.unable_flight = false;
        self.build_mission(spec, mission.number, Check::Whole(&mission.manifest));
    }

    /// The flight starts with these planes' loadouts: the lobby's mission
    /// is built again with them, and the resources they add (a loadout's
    /// other weapons) are compared with the host's; the rest was checked
    /// in the lobby.
    fn flight_loadouts(&mut self, flight: crate::wire::messages::FlightLoadouts) {
        let (Some(mut spec), Some(number)) = (self.spec.clone(), self.number) else {
            return;
        };
        if self.unable.is_some() {
            return;
        }
        spec.plane_loadouts = flight.loadouts.into_iter().collect();
        self.build_mission(spec, number, Check::Added(&flight.manifest));
    }

    /// Builds `spec` from the import, comparing the manifest with the
    /// host's as `check` says.
    fn build_mission(&mut self, spec: MissionSpec, number: u32, check: Check<'_>) {
        let reads = ResourceReads::new(&self.resources);
        let built = match &mut self.builder {
            Some(builder) => builder(&spec, &reads),
            None => World::new(&spec, &reads, Seating::Open),
        };
        let ours = reads.manifest();
        let differences = match check {
            Check::Whole(manifest) => ours.differences(manifest),
            Check::Added(manifest) => manifest
                .entries
                .iter()
                .filter(|entry| {
                    ours.entries
                        .iter()
                        .find(|e| e.name == entry.name)
                        .is_none_or(|e| e.hash != entry.hash)
                })
                .map(|entry| entry.name.clone())
                .collect(),
            Check::None => Vec::new(),
        };
        let flight = matches!(check, Check::Added(_));
        let refusal = if !differences.is_empty() {
            // Stage L: worded by the item the differing files belong to.
            Some(content::refusal(
                &self.content,
                &self.resources,
                &spec,
                &differences,
                &self.config.build.version,
            ))
        } else {
            built
                .as_ref()
                .err()
                .map(|error| format!("Your game cannot build this mission: {error}"))
        };
        if let Some(reason) = refusal {
            self.log("content-refused", &[&differences.join(" "), &reason]);
            self.send(&Message::ContentRefused(ContentRefused {
                mission: number,
                names: differences.clone(),
                reason: reason.clone(),
                flight,
            }));
            self.unable = Some(reason.clone());
            self.unable_flight = flight;
            self.mission = None;
            self.loaded = None;
            self.spec = Some(spec);
            self.end_flight();
            if self.phase == ClientPhase::Loading {
                self.phase = ClientPhase::Lobby;
            }
            self.event(ClientEvent::ContentRefused {
                names: differences,
                reason,
            });
            return;
        }
        let world = built.expect("a build that did not fail");
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
        self.unable = None;
        self.loaded = Some(number);
        if self.phase == ClientPhase::Loading {
            self.phase = ClientPhase::Lobby;
        }
        self.event(ClientEvent::MissionLoaded);
    }

    /// The plane is the player's: its exact state decoded, the prediction
    /// started and the clock set ahead of the host.
    fn seated(&mut self, seated: Seated) {
        let current = self.wire.as_ref().and_then(|wire| wire.flight);
        if FlightOrder::of(seated.flight, current) == FlightOrder::Later {
            self.begin_flight(seated.flight);
        }
        let Some(mission) = &self.mission else {
            // The host seated a player whose import could not build the
            // mission; its refusal is on the way and the host will take the
            // plane back.
            self.log("seat-failed", &["the mission is not loaded"]);
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
        let seen_sample = self.observed.is_some().then(|| {
            seen::own_sample(
                u64::from(seated.tick),
                plane,
                &state.flight,
                &config,
                state.terms.as_ref(),
                &mission.world.terrain,
                None,
            )
        });
        let mut predictor = Predictor::new(
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
        if let (Some(observed), Some(sample)) = (&mut self.observed, seen_sample) {
            predictor.trace_on();
            observed.ensure_flight(now, seated.flight);
            observed.seated(
                seen::SeatSeen {
                    seat: seated.seat,
                    plane,
                    tick: seated.tick,
                },
                &seated.roster,
                sample,
            );
        }
        // Ahead of the host by a round trip and the margin. Fill this initial
        // forecast with neutral input, which is what the host uses before it
        // hears from us. Fresh controls and commands start on the next tick,
        // so they never rewrite the elapsed part of the seating interval.
        let round_trip = self.net.stats().map_or(Duration::ZERO, |s| s.round_trip);
        let lead = clock::ticks_of(round_trip) + self.input_margin_target(now) + 1.;
        self.input_clock
            .reset(now, f64::from(seated.tick) + lead.ceil());
        let mut offset = Offset::default();
        offset.clear(now.as_secs_f64());
        self.roster = Some(seated.roster.clone());
        // A new plane: a revival's wait is over.
        self.revival = None;
        self.seat = Some(Seat {
            seat: SeatId(seated.seat),
            plane,
            model,
            predictor,
            offset,
            sight: sight::SightPrediction::default(),
            seated_at: now,
            seated_tick: u64::from(seated.tick),
            bootstrap_until: u64::from(seated.tick) + lead.ceil() as u64,
            loadout: seated.loadout.clone(),
        });
        self.unacked.clear();
        self.pending.clear();
        self.next_command = 1;
        self.input_acked = seated.tick;
        self.sent_tick = u64::from(seated.tick);
        self.mismatch = 0;
        self.settled = false;
        self.margin_input = 0;
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

    /// The snapshot rate the lobby's settings carry becomes this flight's
    /// ticks per snapshot (slice R1): the host's Accepted packet gave the rate
    /// of the day the player joined, and the King may have turned it in the
    /// lobby since. Settings that change in the lobby cannot change while a
    /// flight runs, so what the lobby says as a flight starts is the host's.
    fn follow_snapshot_rate(&mut self) {
        let rate = self.lobby.as_ref().and_then(|lobby| {
            lobby
                .settings
                .iter()
                .find(|(n, _)| *n == crate::settings::number::SNAPSHOT_RATE)
                .map(|&(_, rate)| rate)
        });
        if let Some(rate) = rate.filter(|rate| (1..=120).contains(rate) && 120 % rate == 0) {
            self.ticks_per_snapshot = 120 / rate;
        }
    }

    /// A new flight of the connection starts (its Seated message, or a
    /// section of it that came first): the wire's baselines, events and
    /// names start afresh, as the host's did, and so do the picture's
    /// clocks and what it held.
    fn begin_flight(&mut self, flight: u8) {
        self.follow_snapshot_rate();
        if self.observed.is_some() {
            let names = self.wire.as_ref().map(|wire| wire.names.clone());
            let now = self.now;
            self.flush_observed();
            if let Some(observed) = &mut self.observed {
                observed.begin_flight(now, flight, names.as_ref());
            }
        }
        self.wire = Some(ClientConnection::for_flight(
            self.ticks_per_snapshot,
            flight,
        ));
        self.interp = Interpolator::new(self.ticks_per_snapshot);
        self.render_clock = RenderClock::new();
        self.held.clear();
        self.effects.clear();
        self.marks.clear();
        self.destroyed.clear();
        self.snapshot_tick = None;
        self.pending_own.clear();
        self.holding_since = None;
        self.early.retain(|body| {
            crate::wire::connection::own_state_header(body).is_ok_and(|h| h.flight == flight)
        });
    }

    fn payload(&mut self, sections: Vec<tore_net::Section>) {
        let mut tick = self.snapshot_tick.unwrap_or(0);
        // The flight of the packet's snapshot, which its events share.
        let mut stale = false;
        for section in sections {
            if let Some(flight) = ClientConnection::section_flight(section.kind, &section.body) {
                let current = self.wire.as_ref().and_then(|wire| wire.flight);
                match FlightOrder::of(flight, current) {
                    FlightOrder::Earlier => {
                        // An earlier flight's, come late: nothing of it
                        // applies now.
                        stale |= section.kind == SECTION_SNAPSHOT;
                        continue;
                    }
                    FlightOrder::Later => self.begin_flight(flight),
                    FlightOrder::Same => {}
                }
            }
            match section.kind {
                SECTION_SNAPSHOT => {
                    if let Some(t) = self.snapshot(&section.body) {
                        tick = t;
                    }
                }
                SECTION_EVENTS if stale => {}
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
        if let Some(observed) = &mut self.observed {
            observed.snapshot(now, &received);
        }
        self.interp.receive(&received);
        self.input_acked = self.input_acked.max(header.input_received);
        let applied = header.commands_applied;
        self.unacked
            .retain(|c| tore_net::sequence_newer(c.number, applied));
        self.stats.inputs_repeated += u64::from(header.inputs_repeated);
        if self.snapshot_tick == Some(tick) {
            self.repeats_reported = header.inputs_repeated;
        }
        self.stats.input_margin = Some(header.input_margin);
        if let Some(seat) = &mut self.seat {
            let round_trip = self.net.stats().map_or(Duration::ZERO, |s| s.round_trip);
            // Before the host has had an input from this seat its margin
            // means nothing; and a snapshot that has had no new input since
            // the last only repeats the last figure, which says nothing of
            // inputs that are not arriving (EF4 follow-up: a client behind
            // the host sends none, and a stale margin kept it there).
            if u64::from(header.input_received) > seat.seated_tick
                && header.input_received > self.margin_input
            {
                self.margin_input = header.input_received;
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
        if self.seat.is_none() {
            // The host sends its exact states as soon as it has seated the
            // player, and they can overtake the long Seated message. The
            // transport has acknowledged this one, so the host may code the
            // next against it: keep it, in order, for when the seat arrives.
            self.early.push(body.to_vec());
            return;
        }
        let (Some(wire), Some(seat)) = (self.wire.as_mut(), self.seat.as_mut()) else {
            return;
        };
        // Read now, so the baselines stay in step with the host's; applied
        // once the prediction has stepped to the update's time.
        match wire.own_state(body, &seat.model) {
            Ok(read) => {
                self.stats.own_states += 1;
                self.pending_own.push(read);
            }
            Err(_) => self.net.disconnect(DisconnectReason::ProtocolError),
        }
    }

    /// Applies the exact states received since the last update, oldest
    /// first, to a prediction that has stepped to now. *Agent decision
    /// (EF4):* applied after the update's steps rather than on arrival, so
    /// the states that queue up while the game stalls meet a prediction
    /// that has caught up; on arrival each was a tick the client had not
    /// reached, and was adopted as it stood, one correction a snapshot.
    ///
    /// Only the newest is applied: restoring it steps the stored ticks after
    /// it again, so the older ones would only be corrected over and over.
    /// While the host still reports repeating this player's late inputs (a
    /// stall's backlog arriving), the newest waits, at most
    /// [`CORRECTION_HOLD_TICKS`], so one stretch of lateness costs one
    /// correction rather than one a snapshot.
    fn apply_own_states(&mut self) {
        let Some(newest) = std::mem::take(&mut self.pending_own)
            .into_iter()
            .max_by_key(|(header, _)| header.tick)
        else {
            return;
        };
        // The hold is counted from when it began, whatever arrives meanwhile
        // (EF4 review): late inputs that last for seconds still cost a
        // correction every 125 ms, never one large one at the end.
        let snapshot = self.snapshot_tick.map_or(0, u64::from);
        if self.repeats_reported > 0 {
            let since = *self.holding_since.get_or_insert(snapshot);
            if snapshot < since + CORRECTION_HOLD_TICKS {
                self.pending_own.push(newest);
                return;
            }
        }
        self.holding_since = None;
        self.apply_own_state(newest.0, newest.1);
    }

    /// Takes the host's exact state of a tick the prediction never reached
    /// (it fell far behind): not a correction of anything predicted, so it
    /// counts as a catch-up, and the drawn plane goes there with it.
    fn catch_up(&mut self, header: OwnStateHeader, state: ExactState) {
        let now = self.now;
        let (Some(seat), Some(mission)) = (self.seat.as_mut(), self.mission.as_ref()) else {
            return;
        };
        match seat
            .predictor
            .restore(u64::from(header.tick), state, &mission.world.terrain)
        {
            Ok(_) => {
                seat.offset.clear(now.as_secs_f64());
                self.stats.catch_ups += 1;
            }
            Err(error) => {
                self.log("prediction-failed", &[&error.to_string()]);
                self.net.disconnect(DisconnectReason::Other(0));
            }
        }
    }

    fn apply_own_state(&mut self, header: OwnStateHeader, state: ExactState) {
        let now = self.now;
        let (Some(seat), Some(mission)) = (self.seat.as_mut(), self.mission.as_ref()) else {
            return;
        };
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
        if let Some(observed) = &mut self.observed {
            observed.event(self.now, &event);
        }
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
            | WireEvent::YourAircraftExploded { .. }
            | WireEvent::Link(_) => {
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
        // Behind the host's own newest snapshot (a starved or stalled game):
        // every input would come late and none would be sent, so no margin
        // would ever say so. The clock jumps ahead of the host as seating
        // sets it, by a round trip and the margin (EF4 follow-up, agent
        // decision).
        if let Some(newest) = self.snapshot_tick
            && self.seat.is_some()
            && self.input_clock.position() < f64::from(newest) + BEHIND_TICKS
        {
            let round_trip = self.net.stats().map_or(Duration::ZERO, |s| s.round_trip);
            let lead = clock::ticks_of(round_trip) + target + 1.;
            self.input_clock.jump_to((f64::from(newest) + lead).ceil());
            self.settled = true;
            self.stats.behind += 1;
        }
        let due = self.input_clock.position().floor().max(0.) as u64;
        // Far behind (a long stall): rather than step the whole backlog, take
        // the host's newest exact state ahead of the prediction as it is,
        // one correction, and step only what is left (agent decision, EF4
        // review: a slow machine could otherwise take seconds to step it,
        // and be corrected all the while).
        if let Some(at) = self.seat.as_ref().map(|seat| seat.predictor.tick())
            && due.saturating_sub(at) > CATCH_UP_TICKS
            && let Some(index) = self
                .pending_own
                .iter()
                .enumerate()
                .filter(|(_, (header, _))| u64::from(header.tick) > at)
                .max_by_key(|(_, (header, _))| header.tick)
                .map(|(index, _)| index)
        {
            let (header, state) = self.pending_own.swap_remove(index);
            self.pending_own.retain(|(h, _)| h.tick > header.tick);
            self.holding_since = None;
            self.catch_up(header, state);
        }
        let (Some(seat), Some(mission)) = (self.seat.as_mut(), self.mission.as_ref()) else {
            return;
        };
        // Ticks the host has stepped already without this player's input
        // (the game stalled): it repeated the last controls with no
        // commands, so the prediction does the same and stays the host's
        // (agent decision, EF4). The commands wait for the next tick.
        let host_stepped = self.snapshot_tick.map_or(0, u64::from);
        let host_had = u64::from(self.input_acked);
        let mut steps = 0;
        while seat.predictor.tick() < due && steps < MAX_TICKS_PER_UPDATE {
            let tick = seat.predictor.tick() + 1;
            // The host repeats the controls of the newest tick it had.
            let repeated = seat
                .predictor
                .history()
                .iter()
                .rev()
                .find(|r| r.tick <= host_had)
                .or(seat.predictor.history().back())
                .map(|r| r.frame);
            let catch_up = if tick <= seat.bootstrap_until {
                Some(InputFrame::default())
            } else if tick <= host_stepped && tick > host_had {
                repeated
            } else {
                None
            };
            if let Some(last) = catch_up {
                if let Err(error) = seat
                    .predictor
                    .step(last, Vec::new(), &mission.world.terrain)
                {
                    let text = error.to_string();
                    if let Some(d) = &mut self.diagnostics {
                        d.line(now, "prediction-failed", &[&text]);
                    }
                    self.net.disconnect(DisconnectReason::Other(0));
                    return;
                }
                seat.sight
                    .step(tick, &last, &[], &seat.predictor.plane().flight);
                steps += 1;
                continue;
            }
            let commands: Vec<Command> = std::mem::take(&mut self.pending);
            for command in &commands {
                self.unacked.push_back(NumberedCommand {
                    number: self.next_command,
                    tick: tick as u32,
                    command: *command,
                });
                self.next_command = self.next_command.wrapping_add(1);
            }
            let stepped = commands.clone();
            if let Err(error) = seat.predictor.step(frame, commands, &mission.world.terrain) {
                let text = error.to_string();
                if let Some(d) = &mut self.diagnostics {
                    d.line(now, "prediction-failed", &[&text]);
                }
                self.net.disconnect(DisconnectReason::Other(0));
                return;
            }
            seat.sight
                .step(tick, &frame, &stepped, &seat.predictor.plane().flight);
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
        // Ticks the host has stepped already would come too late, and their
        // margins would read as a long delay (agent decision, EF4).
        let oldest = (u64::from(self.input_acked) + 1)
            .max(newest.saturating_sub(INPUT_REDUNDANCY - 1))
            .max(first)
            .max(self.snapshot_tick.map_or(0, |tick| u64::from(tick) + 1));
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
            flight: self.wire.as_ref().and_then(|wire| wire.flight).unwrap_or(0),
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
            rotor: flight.lift_controls.drive.rotor_speed,
            rotor_turns: tore_world::snapshot::rotor_pose(flight).0,
            rotor_tilt: tore_world::snapshot::rotor_pose(flight).1,
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
