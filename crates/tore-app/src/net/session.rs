//! The game's end of a networked flight: the client session, its socket and
//! files, the mission the drawn half is built from, and the plain data the
//! flight screen takes from them each frame. Windows, sound and the screens
//! stay in `main.rs`; this file holds what needs no `App`.
//!
//! The mission is built twice before the player is asked for a plane: once
//! by the client session (`World::new`, open seating, which is what the
//! manifest check reads) and once here with the game's own loaders, so the
//! drawn model of every aircraft type loads the way the Quick Mission
//! creator loads them and `CombatView` can draw it. The second world is the
//! app's copy of the mission: never stepped, it gives the screens the
//! terrain, the roster and the aircraft types (agent decision).
//!
//! A join through the Internet Lobby ([`Transport::Internet`]) carries on
//! here after the master's introduction (slice J5): the race of the host's
//! addresses, the relay when none answers, the 15-second give-up and a line
//! in Messages for each step ([`MasterTransport`]; `docs/ARCHITECTURE.md`,
//! "Joining through the master").
use crate::{
    aircraft::Airframe,
    aircraft_type::AircraftType,
    net::{
        files::{self, DatedLog},
        guns::{self, Guns},
        hosting::{HostThread, Report},
        options::ConnectOptions,
    },
    regen::{self, DeviceRelease, Effects, Motor},
    replay::library::Library,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io::{self, BufWriter},
    net::{SocketAddr, UdpSocket},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tore_formats::aircraft::AircraftId;
use tore_net::master::join::{JoinEvent, Joiner, RelayState};
use tore_net::master::{
    JOIN_GIVE_UP, MappingType, Path as JoinedPath, RACE_BEFORE_RELAY, is_relayed,
};
use tore_net::{
    CloseReason, Datagrams, Entropy, Keepalive, KeepaliveConfig, LinkEnd, RealClock, ServerSocket,
};
use tore_session::{
    BuildId, Client, ClientConfig, ClientEvent, ClientFrame, ClientPhase, Controls,
    wire::events::WireEvent,
};
use tore_sim::attitude::{Basis, Vector};
use tore_sim::combat::countermeasures::Release;
use tore_sim::combat::live::EffectKind;
use tore_world::snapshot::RenderSnapshot;
use tore_world::{
    WorldResult,
    mission::MissionSpec,
    resources::ResourceSource,
    world::{Hooks, Seating, World},
};

/// What the game's own build of the mission leaves for the screens.
pub struct Built {
    pub world: World,
    /// The drawn model of each aircraft type, in the order the world holds
    /// the types (`CombatView` draws them in that order).
    pub models: Vec<Airframe>,
}

/// The build this game is, which the host's must match.
pub fn build_id() -> BuildId {
    BuildId {
        version: crate::version::version().to_owned(),
        commit: crate::version::commit().to_owned(),
        // The same rule as the server and the bot: a stamped tag is a
        // release build.
        release: option_env!("TORE_BUILD_VERSION").is_some(),
    }
}

/// Builds the mission of `spec` with the game's loaders: the drawn model of
/// every aircraft type loads beside its simulation half.
fn build_mission(spec: &MissionSpec, resources: &BTreeMap<String, Vec<u8>>) -> WorldResult<Built> {
    let mut models = Vec::new();
    let mut load = |id| -> WorldResult<Arc<AircraftType>> {
        let model = Airframe::load(resources, id)?;
        let kind = Arc::clone(&model.kind);
        models.push(model);
        Ok(kind)
    };
    let built = World::build(
        spec,
        resources,
        Seating::Open,
        &mut Hooks {
            player: None,
            load: Some(&mut load),
            weapon_label: None,
        },
    )?;
    Ok(Built {
        world: built.world,
        models,
    })
}

/// Where the second build of the mission is left for the game.
type Sink = Rc<RefCell<Option<WorldResult<Built>>>>;

/// What carries the session's datagrams: a UDP socket to a server, the
/// in-process link to the host this game runs itself, or the socket a join
/// through the master was introduced on.
pub enum Transport {
    Udp(UdpSocket),
    Link(LinkEnd),
    /// A join through the Internet Lobby (slices I4 and J5, with J2's joiner
    /// and J3's relay).
    Internet(Box<MasterTransport>),
}

/// How a join through the Internet Lobby may reach the host, as `tore-bot
/// --path` says it: race the host's addresses and ask for the relay when none
/// answers (the default), race only, or the relay only. *Agent decision
/// (J5):* the game reads it from the environment variable `TORE_JOIN_PATH`
/// (`auto`, `direct` or `relay`), for tests on one machine, where every
/// direct path works; a player never meets it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum JoinPath {
    #[default]
    Auto,
    Direct,
    Relay,
}

impl JoinPath {
    /// The environment variable the game reads it from.
    pub const VARIABLE: &str = "TORE_JOIN_PATH";

    /// `auto`, `direct` or `relay`.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Some(Self::Auto),
            "direct" => Some(Self::Direct),
            "relay" => Some(Self::Relay),
            _ => None,
        }
    }

    /// The path `TORE_JOIN_PATH` names; `Auto` when it is not set, or (said
    /// in the log) not one of the three.
    pub fn from_env() -> Self {
        let Ok(text) = std::env::var(Self::VARIABLE) else {
            return Self::Auto;
        };
        Self::parse(&text).unwrap_or_else(|| {
            log::warn!(
                "{}={text:?} is not auto, direct or relay; joining as auto",
                Self::VARIABLE
            );
            Self::Auto
        })
    }
}

/// What a join through the master brings to the session besides its socket
/// and joiner: the race, the master's hint, the path allowed and the time
/// already spent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Introduction {
    /// The host's addresses to race, and the introduction's id.
    pub race: tore_session::client::Race,
    /// The master said only the relay reaches the host.
    pub relay_now: bool,
    /// How the join may reach the host.
    pub path: JoinPath,
    /// From pressing Join to the introduction; the whole join gives up
    /// [`JOIN_GIVE_UP`] after the press.
    pub asked: Duration,
}

/// The socket a join through the master was introduced on, read and written
/// through its joiner so the master's datagrams never reach the transport,
/// the race the client runs from it, and the relay when the race finds
/// nothing (slice J5; "Joining through the master" in the architecture
/// guide). [`NetSession::pump`] gives it its turn.
pub struct MasterTransport {
    socket: ServerSocket,
    joiner: Joiner,
    /// The session clock at the last turn: the router's and the joiner's
    /// time (agent decision: one clock for the session, the race and the
    /// relay).
    now: Duration,
    race: Option<tore_session::client::Race>,
    relay_now: bool,
    path: JoinPath,
    asked: Duration,
    /// When the race started, and when the whole join gives up, on the
    /// session clock.
    raced: Duration,
    give_up: Duration,
    /// The relay was asked for (at most once).
    relay_asked: bool,
}

impl MasterTransport {
    /// The introduced socket and joiner, and what the introduction said.
    pub fn new(socket: ServerSocket, joiner: Joiner, introduction: Introduction) -> Box<Self> {
        let Introduction {
            race,
            relay_now,
            path,
            asked,
        } = introduction;
        Box::new(Self {
            socket,
            joiner,
            now: Duration::ZERO,
            race: Some(race),
            relay_now,
            path,
            asked,
            raced: Duration::ZERO,
            give_up: JOIN_GIVE_UP.saturating_sub(asked),
            relay_asked: false,
        })
    }

    /// The race starts at `now` on the session clock.
    fn begin(&mut self, now: Duration) {
        self.now = now;
        self.raced = now;
        self.give_up = now + JOIN_GIVE_UP.saturating_sub(self.asked);
    }

    /// Why to ask for the relay now, as Messages says it, while no address
    /// has answered (`chosen` false): at once when only the relay is allowed
    /// or the master's hint says so, after [`RACE_BEFORE_RELAY`] otherwise,
    /// never for a join that races only or has asked already.
    fn relay_reason(&self, now: Duration, chosen: bool) -> Option<&'static str> {
        if self.relay_asked || chosen {
            return None;
        }
        match self.path {
            JoinPath::Direct => None,
            JoinPath::Relay => Some("Asking for the relay..."),
            JoinPath::Auto if self.relay_now => Some(
                "The Internet Lobby says only the relay reaches this game; asking for the relay...",
            ),
            JoinPath::Auto if now >= self.raced + RACE_BEFORE_RELAY => {
                Some("No direct path; asking for the relay...")
            }
            JoinPath::Auto => None,
        }
    }

    /// Why a join that never connected ended, for `label`.
    fn no_answer_text(&self, label: &str) -> String {
        if self.relay_asked {
            format!("Could not reach '{label}' directly or through the relay.")
        } else {
            format!("No answer from '{label}' at any of its addresses.")
        }
    }

    /// The player's own mapping type and the game bytes relayed each way,
    /// for the player's Report.
    fn facts(&self) -> ThroughFacts {
        let relay = self.joiner.relay_counters();
        ThroughFacts {
            mapping: self.joiner.mapping(),
            relayed_bytes: relay.bytes_in + relay.bytes_out,
        }
    }
}

/// What a join through the Internet Lobby tells the player's Report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThroughFacts {
    /// How the player's router maps the joining socket.
    pub mapping: MappingType,
    /// The game's bytes through the relay, both ways.
    pub relayed_bytes: u64,
}

impl Default for ThroughFacts {
    fn default() -> Self {
        Self {
            mapping: MappingType::Unknown,
            relayed_bytes: 0,
        }
    }
}

impl Datagrams for Transport {
    fn send_datagram(&mut self, to: SocketAddr, datagram: &[u8]) -> io::Result<()> {
        match self {
            Self::Udp(socket) => socket.send_datagram(to, datagram),
            Self::Link(link) => link.send_datagram(to, datagram),
            Self::Internet(t) => {
                // The relay only: nothing goes to the host's own addresses,
                // so the race cannot win on a machine where every path works.
                if t.path == JoinPath::Relay && !is_relayed(to) {
                    return Ok(());
                }
                let now = t.now;
                t.joiner
                    .over(&mut t.socket, now)
                    .send_datagram(to, datagram)
            }
        }
    }

    fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        match self {
            Self::Udp(socket) => socket.recv_datagram(buf),
            Self::Link(link) => link.recv_datagram(buf),
            Self::Internet(t) => {
                let now = t.now;
                t.joiner.over(&mut t.socket, now).recv_datagram(buf)
            }
        }
    }
}

/// The keepalive thread of a game joined over UDP (slice EF-K): while the
/// game's loop is stalled (a window dragged on Windows, a long frame, a
/// screenshot), it tells the host once a second that the connection is
/// alive, for at most a minute. It starts once the host has accepted the
/// join, learns of every turn of the game's loop, and stops when the
/// connection closes or the session is dropped. The hosting game's own
/// connection, over the in-process link, has none: the host never drops it
/// for silence (EF4).
pub(crate) struct KeptAlive {
    config: KeepaliveConfig,
    thread: Option<Keepalive>,
    /// The thread could not be started; the game carries on without it.
    failed: bool,
    /// Keepalives already noted in the log.
    noted: u64,
}

impl KeptAlive {
    pub(crate) fn new(config: KeepaliveConfig) -> Self {
        Self {
            config,
            thread: None,
            failed: false,
            noted: 0,
        }
    }

    /// After a turn of the game's loop, which has just sent what it had.
    pub(crate) fn turned(&mut self, client: &Client, transport: &Transport) {
        match client.phase() {
            tore_session::ClientPhase::Connecting => {}
            tore_session::ClientPhase::Closed => self.thread = None,
            _ => match &self.thread {
                Some(thread) => {
                    thread.turned();
                    let sent = thread.sent();
                    if sent > self.noted {
                        log::info!(
                            "Network: the game was held up; its keepalive kept the connection \
                             ({} keepalives, {} in all)",
                            sent - self.noted,
                            sent
                        );
                        self.noted = sent;
                    }
                }
                None if !self.failed => self.start(client, transport),
                None => {}
            },
        }
    }

    fn start(&mut self, client: &Client, transport: &Transport) {
        // Over the link (the hosting game's own connection) there is none.
        if matches!(transport, Transport::Link(_)) {
            return;
        }
        let Some(datagram) = client.keepalive_datagram() else {
            return;
        };
        let (host, config) = (client.server(), self.config);
        let started = match transport {
            Transport::Udp(socket) => socket
                .try_clone()
                .and_then(|socket| Keepalive::start(socket, host, datagram, config)),
            // A join through the master: a clone of its socket, and through
            // the relay one that frames for the channel (slice J5, with J3's
            // `RelayFraming`), so a stalled relayed game stays connected
            // (John, 2026-10-05).
            Transport::Internet(t) => t.socket.try_clone().and_then(|socket| {
                if !is_relayed(host) {
                    return Keepalive::start(socket, host, datagram, config);
                }
                match t.joiner.keepalive_socket(socket) {
                    Some(framed) => Keepalive::start(framed, host, datagram, config),
                    None => Err(io::Error::other("the relay's channel is not open")),
                }
            }),
            Transport::Link(_) => return,
        };
        match started {
            Ok(thread) => {
                self.thread = Some(thread);
                self.noted = 0;
            }
            Err(error) => {
                self.failed = true;
                log::warn!("Network: no keepalive thread ({error}); a stalled game may be dropped");
            }
        }
    }

    /// Whether the thread runs.
    #[cfg(test)]
    pub(crate) fn running(&self) -> bool {
        self.thread.is_some()
    }

    /// Keepalives the thread has sent.
    #[cfg(test)]
    pub(crate) fn sent(&self) -> u64 {
        self.thread.as_ref().map_or(0, Keepalive::sent)
    }
}

/// Where and how a session joins: the host's address, the transport that
/// reaches it, and what the player gives.
pub struct Join {
    pub server: SocketAddr,
    pub transport: Transport,
    pub callsign: String,
    /// The plane to ask for; `None` for the first free one.
    pub slot: Option<u32>,
    /// Empty when the host has no password.
    pub password: String,
    /// The host as the capture's file name shows it.
    pub label: String,
    /// A lobby screen drives the session (EF8): the client does not take a
    /// slot or ready by itself, the King's game does not start missions by
    /// itself, and a joiner's End Mission returns it to the lobby instead of
    /// leaving the game. False for `--connect` and `--host`.
    pub lobby: bool,
}

impl Join {
    /// A join to the server `options` name: the name looked up, a socket
    /// opened.
    pub fn connect(options: &ConnectOptions) -> Result<Self, String> {
        let server = options.resolve()?;
        let mut join = Self::to(server, &options.callsign, &options.password, &options.host)?;
        join.slot = options.slot;
        Ok(join)
    }

    /// A join to an address already known (a game the Direct Connection
    /// screen found, or an address its lookup reached): a socket opened,
    /// nothing looked up.
    pub fn to(
        server: SocketAddr,
        callsign: &str,
        password: &str,
        label: &str,
    ) -> Result<Self, String> {
        let local: SocketAddr = if server.is_ipv4() {
            ([0, 0, 0, 0], 0).into()
        } else {
            "[::]:0".parse().expect("an address")
        };
        let socket =
            tore_net::bind_udp(local).map_err(|error| format!("Cannot open a socket: {error}"))?;
        Ok(Self {
            server,
            transport: Transport::Udp(socket),
            callsign: callsign.to_owned(),
            slot: None,
            password: password.to_owned(),
            label: label.to_owned(),
            lobby: false,
        })
    }
}

/// A joined (or joining) session.
pub struct NetSession {
    pub client: Client,
    socket: Transport,
    /// Speaks for the connection while the game's loop is stalled.
    kept: KeptAlive,
    clock: RealClock,
    sink: Sink,
    /// The mission the host last sent, to build the game's copy of it again
    /// for a plane taken a second time in the same running mission.
    spec: Rc<RefCell<Option<MissionSpec>>>,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    /// Chaff, flares, smoke and contrails, stepped once per client tick.
    pub effects: Effects,
    /// The client tick the effects have been stepped to.
    effects_tick: u64,
    /// Releases waiting for the next effects step.
    releases: Vec<DeviceRelease>,
    /// The gun rounds the host does not send.
    guns: Guns,
    /// The player's trigger, kept as the host will read it.
    fire: guns::Fire,
    /// Whether it was held at the last turn.
    trigger: bool,
    /// Each weapon record's motor, parsed once.
    motors: RefCell<BTreeMap<String, Option<Motor>>>,
    /// Session events not yet taken by the game.
    events: Vec<ClientEvent>,
    /// When the player asked to leave.
    pub left_at: Option<Duration>,
    /// The capture file being written, kept out of the pruning.
    pub capture: Option<PathBuf>,
    /// The debrief the host sent, when it has.
    pub debrief: Option<tore_session::wire::messages::Debrief>,
    /// Why the host last ended the mission, until the debrief that follows
    /// it is shown or the player is seated again.
    pub ended: Option<tore_session::wire::messages::EndReason>,
    /// The host this game runs, when it hosts the session; dropping the
    /// session stops it.
    pub hosting: Option<HostThread>,
    /// Why the host this game runs ended without the session ending: its
    /// mission could not be built, or it panicked.
    host_failure: Option<String>,
    /// The lobby state the King's automatic start was last asked for.
    started_for: Option<tore_session::wire::messages::LobbyState>,
    /// A lobby screen drives the session (EF8, [`Join::lobby`]).
    pub lobby_screen: bool,
    /// The lobby's last line in the log.
    lobby_line: Option<String>,
    /// The chat window's lines and the open line (slice EF6).
    pub chat: crate::net::chat::Chat,
    /// What joining again needs: a join through the master joins the relay's
    /// address with a new client when the race finds nothing (slice J5).
    again: Again,
    /// The diagnostics log, shared with the client so the game can add the
    /// path line.
    log: Rc<RefCell<DatedLog>>,
    /// The session clock when the host accepted the join.
    connected_at: Option<Duration>,
    /// Why the game ended a join through the master itself (it gave up, or
    /// the relay was refused or closed): the words the player reads instead
    /// of the client's.
    ending: Option<String>,
}

/// What a session keeps to start its client again (slice J5).
struct Again {
    /// The client's settings as the session started it.
    config: ClientConfig,
    replays: Option<Library>,
    label: String,
}

/// Writes into the diagnostics log the client holds, so the game can add a
/// line of its own.
struct SharedLog(Rc<RefCell<DatedLog>>);

impl io::Write for SharedLog {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.borrow_mut().flush()
    }
}

/// A path as the net log and the game's log say it (as `tore-bot` says it).
pub fn path_words(path: JoinedPath) -> &'static str {
    match path {
        JoinedPath::LocalNetwork => "local network",
        JoinedPath::ByAddress => "by address",
        JoinedPath::MappedPort => "mapped port",
        JoinedPath::Ipv6 => "IPv6",
        JoinedPath::Punched => "punched",
        JoinedPath::Relay => "relay",
    }
}

/// Gives a newly started `client` its capture (a new file beside the
/// replays, when there are replays), the diagnostics log and the game's
/// mission builder. The capture's path, when one was made.
fn equip(
    client: &mut Client,
    again: &Again,
    log: &Rc<RefCell<DatedLog>>,
    sink: &Sink,
    spec: &Rc<RefCell<Option<MissionSpec>>>,
    resources: &Arc<BTreeMap<String, Vec<u8>>>,
) -> Option<PathBuf> {
    let mut capture = None;
    if let Some(library) = &again.replays {
        match files::create_capture(library, SystemTime::now(), &again.label) {
            Ok((path, file)) => {
                client.set_capture(Box::new(BufWriter::new(file)));
                capture = Some(path);
            }
            Err(error) => log::warn!("Network capture not written: {error}"),
        }
    }
    client.set_diagnostics(Box::new(SharedLog(Rc::clone(log))));
    let (map, kept, spec_for_rebuild) = (Arc::clone(resources), Rc::clone(sink), Rc::clone(spec));
    client.set_mission_builder(Box::new(move |spec, reads: &dyn ResourceSource| {
        *spec_for_rebuild.borrow_mut() = Some(spec.clone());
        // The game's own build for the screens, then the session's: a
        // failure of either is the mission's.
        *kept.borrow_mut() = Some(build_mission(spec, &map));
        // Read every resource the plain build reads, through `reads`, so
        // the manifest the host compares is the simulation's own.
        World::new(spec, reads, Seating::Open)
    }));
    capture
}

/// How long after Leave the game waits for the debrief and the disconnect
/// before it quits the connection itself.
pub const LEAVE_GRACE: Duration = Duration::from_secs(8);

impl NetSession {
    /// The import the session's mission is built from, which its capture
    /// converts with.
    pub fn resources(&self) -> Arc<BTreeMap<String, Vec<u8>>> {
        Arc::clone(&self.resources)
    }

    /// Starts the join over `join`'s transport. The mission is built when
    /// the host sends it.
    pub fn start(
        join: Join,
        resources: Arc<BTreeMap<String, Vec<u8>>>,
        data: &Path,
        replays: Option<&Library>,
    ) -> Result<Self, String> {
        let Join {
            server,
            transport: mut socket,
            callsign,
            slot,
            password,
            label,
            lobby,
        } = join;
        let clock = RealClock::new();
        let now = clock.now();
        // A join through the master races the host's addresses.
        let race = match &mut socket {
            Transport::Internet(t) => {
                t.begin(now);
                t.race.take()
            }
            _ => None,
        };
        let config = ClientConfig {
            race,
            password,
            plane: slot,
            entropy: Entropy::System,
            retail_stall_speeds: tore_sim::flight::retail_stall_speeds(),
            auto_ready: !lobby,
            ..ClientConfig::new(server, &callsign, build_id())
        };
        let mut client = Client::connect(config.clone(), Arc::clone(&resources), now)
            .map_err(|error| error.to_string())?;

        // The files: the log, and a capture beside the replays, pruned by the
        // replays' own rule before a new one is made.
        if let Some(library) = replays {
            let settings = library.settings();
            let cleanup = files::prune(library, data, &settings, SystemTime::now(), &[]);
            for (path, error) in &cleanup.failed {
                log::warn!(
                    "Network files: could not remove {}: {error}",
                    path.display()
                );
            }
        }
        let again = Again {
            config,
            replays: replays.cloned(),
            label,
        };
        let log = Rc::new(RefCell::new(DatedLog::new(data.join(files::LOG_FOLDER))));
        let sink: Sink = Rc::new(RefCell::new(None));
        let spec_kept: Rc<RefCell<Option<MissionSpec>>> = Rc::new(RefCell::new(None));
        let capture = equip(&mut client, &again, &log, &sink, &spec_kept, &resources);
        Ok(Self {
            client,
            socket,
            kept: KeptAlive::new(KeepaliveConfig::default()),
            clock,
            sink,
            spec: spec_kept,
            resources,
            effects: Effects::default(),
            effects_tick: 0,
            releases: Vec::new(),
            guns: Guns::default(),
            fire: guns::Fire::default(),
            trigger: false,
            motors: RefCell::new(BTreeMap::new()),
            events: Vec::new(),
            left_at: None,
            capture,
            debrief: None,
            ended: None,
            hosting: None,
            host_failure: None,
            started_for: None,
            lobby_screen: lobby,
            lobby_line: None,
            chat: Default::default(),
            again,
            log,
            connected_at: None,
            ending: None,
        })
    }

    /// Whether this game hosts the session: its player is the King.
    pub fn hosting(&self) -> bool {
        self.hosting.is_some()
    }

    /// The King's start for a game hosted from the command line (agent
    /// decision): as soon as every player holding a slot is ready, when
    /// `allowed` (the hosting player is not reading a debrief), once for each
    /// lobby state. A game with a lobby screen never starts by itself: its
    /// King presses Fly.
    pub fn auto_start(&mut self, allowed: bool) {
        if !allowed || self.hosting.is_none() || self.lobby_screen {
            return;
        }
        let Some(lobby) = self.client.lobby() else {
            return;
        };
        if lobby.is_king()
            && lobby.phase == tore_session::wire::messages::LobbyPhase::Lobby
            && lobby.all_ready()
            && self.started_for.as_ref() != Some(lobby)
        {
            self.started_for = Some(lobby.clone());
            log::info!("Network: starting the mission; every player holding a slot is ready");
            self.client.start_mission();
        }
    }

    /// The lobby in one line for the log, when it changed since the last.
    pub fn lobby_change(&mut self) -> Option<String> {
        let lobby = self.client.lobby()?;
        let players: Vec<String> = lobby
            .players
            .iter()
            .map(|p| {
                let mut text = p.callsign.clone();
                if lobby.king == Some(p.id) {
                    text.push_str(" (King)");
                }
                match p.slot {
                    Some(plane) => text.push_str(&format!(" plane {plane}")),
                    None => text.push_str(" no slot"),
                }
                if p.loadout {
                    text.push_str(" armed");
                }
                if p.flying {
                    text.push_str(" flying");
                } else if p.ready {
                    text.push_str(" ready");
                }
                if let Some(why) = &p.unable {
                    text.push_str(&format!(" unable ({why})"));
                }
                text
            })
            .collect();
        let line = format!(
            "{:?}, mission {} ({}): {}",
            lobby.phase,
            lobby.mission,
            lobby.summary,
            players.join("; ")
        );
        if self.lobby_line.as_ref() == Some(&line) {
            return None;
        }
        self.lobby_line = Some(line.clone());
        Some(line)
    }

    /// Sends what the client has queued (a Disconnect, say) without turning
    /// the session over.
    pub fn flush(&mut self) {
        let _ = self.client.transmit(&mut self.socket);
        if let Transport::Internet(t) = &mut self.socket {
            // A connection that ended closes its relay channel, after its
            // last datagrams went through it.
            if self.client.phase() == ClientPhase::Closed {
                t.joiner.close_relay();
            }
            let _ = t.joiner.transmit(&mut t.socket);
        }
    }

    /// The words for why the session ended: the game's own when it ended a
    /// join through the master itself, a plainer one for a join through the
    /// master that no address answered, else the client's.
    pub fn close_text(&self, reason: &CloseReason) -> String {
        if let Some(text) = &self.ending {
            return text.clone();
        }
        if let (Transport::Internet(t), CloseReason::NoAnswer) = (&self.socket, reason) {
            return t.no_answer_text(&self.again.label);
        }
        self.client.close_text(reason)
    }

    /// What a join through the Internet Lobby tells the player's Report;
    /// `None` for any other join.
    pub fn through_facts(&self) -> Option<ThroughFacts> {
        match &self.socket {
            Transport::Internet(t) => Some(t.facts()),
            _ => None,
        }
    }

    /// From the session's start to the host accepting the join, once it has.
    pub fn connected_after(&self) -> Option<Duration> {
        self.connected_at
    }

    /// The join through the master's turn, after the client's (slice J5):
    /// the joiner's timers and the master's answers; the relay asked for
    /// when the race finds nothing in time, the client started again on the
    /// relay's address when its channel opens, the relay closed when it is
    /// not wanted or the connection ended; the whole join given up after
    /// [`JOIN_GIVE_UP`]. Each step a player should read is a
    /// [`ClientEvent::Notice`] (agent decision: the session's own notices
    /// reach Messages the way the host's do).
    fn through(&mut self, now: Duration) {
        let Transport::Internet(t) = &mut self.socket else {
            return;
        };
        let label = &self.again.label;
        let phase = self.client.phase();
        let joining = phase == ClientPhase::Connecting;
        let mut notices = Vec::new();
        let mut ending = None;
        let mut relayed = None;
        t.joiner.update(now);
        while let Some(event) = t.joiner.poll_event() {
            match event {
                JoinEvent::Relayed { address } => {
                    if joining && !self.client.chosen() {
                        relayed = Some(address);
                    } else {
                        log::info!(
                            "Network: the relay opened after {label} answered directly; closing it"
                        );
                        t.joiner.close_relay();
                    }
                }
                JoinEvent::RelayRefused { text, .. } => {
                    log::info!("Network: the relay was refused: {text}");
                    if !joining {
                        continue;
                    }
                    if t.path == JoinPath::Relay {
                        ending = Some(text);
                    } else {
                        notices.push(text);
                    }
                }
                JoinEvent::RelaySilent => {
                    let text = "The Internet Lobby did not answer the relay request.".to_owned();
                    if !joining {
                        continue;
                    }
                    if t.path == JoinPath::Relay {
                        ending = Some(text);
                    } else {
                        notices.push(text);
                    }
                }
                JoinEvent::RelayClosed(reason) => {
                    let text = tore_net::master::relay::close_text(reason).to_owned();
                    log::info!("Network: {text}");
                    // The game connection through the relay is lost with it.
                    if is_relayed(self.client.server()) && phase != ClientPhase::Closed {
                        ending = Some(text);
                    }
                }
                // The introduction's events came before the session.
                other => log::info!("Network: the Internet Lobby said {other:?} after the join"),
            }
        }
        if ending.is_none()
            && joining
            && let Some(why) = t.relay_reason(now, self.client.chosen())
        {
            t.relay_asked = true;
            if t.joiner.ask_relay(now) {
                notices.push(why.to_owned());
            } else {
                log::info!("Network: the relay could not be asked for");
            }
        }
        if ending.is_none() && joining && now >= t.give_up {
            ending = Some(t.no_answer_text(label));
        }
        if phase == ClientPhase::Closed && t.joiner.relay_state() != RelayState::Ended {
            t.joiner.close_relay();
        }
        let _ = t.joiner.transmit(&mut t.socket);
        self.events
            .extend(notices.into_iter().map(ClientEvent::Notice));
        if let Some(text) = ending {
            self.ending = Some(text);
            self.client.disconnect(now);
            let _ = self.client.transmit(&mut self.socket);
        } else if let Some(address) = relayed {
            self.join_relayed(address, now);
        }
    }

    /// The race found nothing and the relay's channel is open: the same
    /// join, to the channel's relayed address, with a new client (as
    /// `tore-bot` does). The race's capture, which holds no game, is
    /// removed and a new one started (agent decision).
    fn join_relayed(&mut self, address: SocketAddr, now: Duration) {
        let config = ClientConfig {
            server: address,
            race: None,
            ..self.again.config.clone()
        };
        let client = match Client::connect(config, Arc::clone(&self.resources), now) {
            Ok(client) => client,
            Err(error) => {
                self.ending = Some(error.to_string());
                self.client.disconnect(now);
                return;
            }
        };
        drop(std::mem::replace(&mut self.client, client));
        if let Some(raced) = self.capture.take()
            && let Err(error) = std::fs::remove_file(&raced)
        {
            log::warn!(
                "Network files: could not remove the race's capture {}: {error}",
                raced.display()
            );
        }
        self.capture = equip(
            &mut self.client,
            &self.again,
            &self.log,
            &self.sink,
            &self.spec,
            &self.resources,
        );
        log::info!(
            "Network: the relay is open; joining {} through it at {address}",
            self.again.label
        );
        self.events.push(ClientEvent::Notice(
            "The relay is open; joining through it...".to_owned(),
        ));
        let _ = self.client.transmit(&mut self.socket);
    }

    /// The host accepted the join at `now`: the path it took goes to the net
    /// log and the game's log, and, for a join through the Internet Lobby,
    /// to Messages ("Connected through the relay.").
    fn connected(&mut self, now: Duration) {
        self.connected_at = Some(now);
        if matches!(self.socket, Transport::Link(_)) {
            return;
        }
        let path = self.client.path();
        let server = self.client.server();
        let words = path_words(path);
        let line = format!("{:.3}\tpath\t{words}\t{server}\n", now.as_secs_f64());
        let _ = io::Write::write_all(&mut *self.log.borrow_mut(), line.as_bytes());
        log::info!("Network: joined {server}, path {words}");
        if matches!(self.socket, Transport::Internet(_)) {
            self.events.push(ClientEvent::Notice(
                crate::net::telemetry::path_line(path).to_owned(),
            ));
        }
    }

    /// The session clock now.
    pub fn now(&self) -> Duration {
        self.clock.now()
    }

    /// Receives, runs what is due with `controls`, and sends. The session's
    /// events collect for [`NetSession::take_events`].
    pub fn pump(&mut self, controls: &Controls) {
        if let Some(hosting) = &mut self.hosting {
            for report in hosting.poll() {
                if let Report::Ended(end) = report
                    && let Some(text) = end.failure()
                {
                    self.host_failure.get_or_insert(text);
                }
            }
        }
        let now = self.clock.now();
        if let Transport::Internet(t) = &mut self.socket {
            t.now = now;
        }
        self.trigger = self.fire.turn(&controls.commands, controls.trigger);
        let _ = self.client.receive_from(now, &mut self.socket);
        self.client.update(now, controls);
        let _ = self.client.transmit(&mut self.socket);
        // A join through the master: the race, the relay, the give-up. It
        // may start the client again, before its events are read.
        self.through(now);
        self.kept.turned(&self.client, &self.socket);
        while let Some(event) = self.client.poll_event() {
            let connected = matches!(event, ClientEvent::Connected { .. });
            if let ClientEvent::Debrief(debrief) = &event {
                self.debrief = Some((**debrief).clone());
            }
            self.events.push(event);
            if connected {
                self.connected(now);
            }
        }
        // A player who left waits only so long for the host to answer.
        if self
            .left_at
            .is_some_and(|at| now.saturating_sub(at) > LEAVE_GRACE)
        {
            self.client.disconnect(now);
        }
    }

    /// How long until the session next needs a turn, at most 10 ms.
    pub fn next_wake(&self) -> Duration {
        self.client.next_wake(self.clock.now())
    }

    /// The plain message for a host this game runs that failed (its
    /// mission could not be built, or it panicked), once.
    pub fn take_host_failure(&mut self) -> Option<String> {
        self.host_failure.take()
    }

    /// The session events since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<ClientEvent> {
        std::mem::take(&mut self.events)
    }

    /// The game's build of the mission, once the host has sent it. `Some`
    /// once; an `Err` is the build's failure.
    pub fn take_built(&mut self) -> Option<WorldResult<Built>> {
        self.sink.borrow_mut().take()
    }

    /// The game's build of the running mission again, for a player who left
    /// its flight and takes a plane in the same mission once more (the
    /// first seating took the build [`NetSession::take_built`] gave).
    /// `None` until the host has sent a mission.
    pub fn rebuild(&mut self) -> Option<WorldResult<Built>> {
        let spec = self.spec.borrow().clone()?;
        Some(build_mission(&spec, &self.resources))
    }

    /// End Mission in flight. The hosting player, the King, ends the
    /// mission for everyone, who return to the lobby with their debriefs
    /// (and the King's game starts the next once the debrief is closed);
    /// any other player leaves the game, with its debrief, since a game with
    /// no lobby screen has nowhere else to go (agent decisions, EF4). With a
    /// lobby screen (EF8) any other player returns to the lobby, its debrief
    /// shown, while the others fly on.
    pub fn leave(&mut self) {
        if self.hosting.is_some() {
            self.client.end_mission();
            return;
        }
        if self.lobby_screen {
            // Back to the lobby while the others fly on (EF8).
            let now = self.clock.now();
            self.client.leave(now);
            return;
        }
        if self.left_at.is_none() {
            let now = self.clock.now();
            self.left_at = Some(now);
            self.client.leave_game(now);
        }
    }

    /// The client's frame for this render, with the countermeasure releases
    /// its events brought (kept for [`NetSession::step_effects`]). `None`
    /// until the player has a plane.
    pub fn frame(&mut self) -> Option<ClientFrame> {
        let frame = self.client.frame(self.clock.now())?;
        for received in &frame.events {
            if let WireEvent::Countermeasure {
                aircraft,
                flare,
                position,
                velocity,
                attitude,
                number,
                ..
            } = &received.event
            {
                self.releases.push(release_of(
                    *aircraft,
                    *flare,
                    position,
                    velocity,
                    attitude,
                    *number,
                    received.tick,
                ));
            }
        }
        Some(frame)
    }

    /// Adds the gun rounds the host does not send to `frame`'s picture: the
    /// player's own from its trigger and its predicted aircraft, and other
    /// aircraft's from the host's gun burst events.
    pub fn step_guns(&mut self, frame: &mut ClientFrame, around: &GunContext<'_>) {
        let ground = |x: f64, z: f64| f64::from(around.terrain.height(x as f32, z as f32));
        let stations = |id: AircraftId| -> &[tore_sim::combat::live::Station] {
            around
                .configurations
                .iter()
                .find(|config| config.aircraft == id)
                .map_or(&[][..], |config| config.stations.as_slice())
        };
        let inputs = guns::Inputs {
            tick: frame.tick,
            render_tick: frame.render_tick,
            trigger: self.trigger,
            plane: frame.plane.0,
            own: crate::combat::launcher(&frame.presented),
            stores: frame.readout.as_ref().map(guns::Stores::of),
            config: &frame.config,
            events: &frame.events,
        };
        let mut picture = std::mem::take(&mut frame.picture);
        self.guns.step(
            &inputs,
            &guns::Around {
                ground: &ground,
                stations: &stations,
            },
            &mut picture,
        );
        frame.picture = picture;
    }

    /// Steps the regenerated effects to client tick `to`, one step per tick
    /// since the last call (at most [`MAX_EFFECT_TICKS`]), each over
    /// `picture`, the newest the screen has; the releases the frames brought
    /// go with the first step.
    pub fn step_effects(&mut self, to: u64, picture: &RenderSnapshot, around: &EffectContext<'_>) {
        let behind = to.saturating_sub(self.effects_tick);
        // A long gap (a stall, or the first frame) is not caught up on.
        let steps = behind.clamp(1, MAX_EFFECT_TICKS);
        self.effects_tick = to;
        let outlets = |id: AircraftId| -> &[Vector] {
            around
                .models
                .iter()
                .find(|model| model.profile.id == id)
                .map_or(&[][..], |model| &model.kind.contrail_offsets)
        };
        let motor = |name: &str| -> Option<Motor> {
            self.motors
                .borrow_mut()
                .entry(name.to_owned())
                .or_insert_with(|| {
                    let bytes = around.resources.get(name)?;
                    Motor::of(&tore_formats::weapons::Weapon::parse(name, bytes).ok()?)
                })
                .to_owned()
        };
        let surroundings = regen::Surroundings {
            terrain: around.terrain,
            outlets: &outlets,
            motor: &motor,
            sortie: around.sortie,
        };
        for step in 0..steps {
            let releases = if step == 0 {
                std::mem::take(&mut self.releases)
            } else {
                Vec::new()
            };
            self.effects.step(picture, &surroundings, &releases);
        }
    }
}

/// What the gun rounds need that the frame does not say.
pub struct GunContext<'a> {
    pub terrain: &'a crate::terrain::Terrain,
    /// The mission's usual loadout of each aircraft type.
    pub configurations: &'a [tore_sim::combat::live::Configuration],
}

/// What the regenerated effects need that the picture does not say.
pub struct EffectContext<'a> {
    pub terrain: &'a crate::terrain::Terrain,
    /// The drawn model of every aircraft type of the mission, the player's
    /// included: their engine outlets place the contrails.
    pub models: &'a [&'a Airframe],
    pub resources: &'a BTreeMap<String, Vec<u8>>,
    pub sortie: u64,
}

/// The most effect steps one frame takes, so a stall cannot become a long
/// burst of work.
const MAX_EFFECT_TICKS: u64 = 24;

/// A wire countermeasure release as the effects fly it.
fn release_of(
    aircraft: u32,
    flare: bool,
    position: &[i64; 3],
    velocity: &[i64; 3],
    attitude: &[u16; 3],
    number: u64,
    tick: u32,
) -> DeviceRelease {
    use tore_session::wire::entity::{POSITION_STEP, VELOCITY_STEP, radians};
    let [yaw, pitch, bank] = attitude.map(radians);
    DeviceRelease {
        owner: aircraft,
        kind: if flare {
            EffectKind::Flare
        } else {
            EffectKind::Chaff
        },
        release: Release {
            position: position.map(|v| v as f64 * POSITION_STEP),
            velocity: velocity.map(|v| v as f64 * VELOCITY_STEP),
            basis: Basis::new(yaw, pitch, bank),
        },
        number,
        tick: u64::from(tick),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::combat::live::DeviceRelease as Combat;

    /// What combat released, sent by the host and read back here, is the
    /// release combat made to within the wire's steps.
    #[test]
    fn a_countermeasure_release_comes_back_as_combat_made_it() {
        let made = Combat {
            owner: 3,
            kind: EffectKind::Flare,
            release: Release {
                position: [12_345.5, 20_000.25, -4_321.125],
                velocity: [310.5, -12.25, 480.75],
                basis: Basis::new(1.2, -0.3, 0.45),
            },
            number: 7,
            tick: 900,
            left: Some(12),
        };
        let WireEvent::Countermeasure {
            aircraft,
            flare,
            position,
            velocity,
            attitude,
            number,
            ..
        } = tore_session::wire::from_world::countermeasure_event(&made)
        else {
            panic!("a countermeasure event");
        };
        let read = release_of(
            aircraft, flare, &position, &velocity, &attitude, number, 900,
        );
        assert_eq!(
            (read.owner, read.kind, read.number),
            (3, EffectKind::Flare, 7)
        );
        for i in 0..3 {
            assert!((read.release.position[i] - made.release.position[i]).abs() < 1. / 32.);
            assert!((read.release.velocity[i] - made.release.velocity[i]).abs() < 1. / 64.);
            // A turn in 65,536 steps: about a ten-thousandth of a radian.
            for (a, b) in [
                (read.release.basis.forward[i], made.release.basis.forward[i]),
                (read.release.basis.up[i], made.release.basis.up[i]),
            ] {
                assert!((a - b).abs() < 2e-4, "{a} {b}");
            }
        }
        let chaff = Combat {
            kind: EffectKind::Chaff,
            ..made
        };
        let WireEvent::Countermeasure { flare, .. } =
            tore_session::wire::from_world::countermeasure_event(&chaff)
        else {
            panic!("a countermeasure event");
        };
        assert!(!flare);
    }
}

#[cfg(test)]
#[path = "keepalive_tests.rs"]
mod keepalive_tests;

#[cfg(test)]
#[path = "rejoin_tests.rs"]
mod rejoin_tests;

#[cfg(test)]
#[path = "join_tests.rs"]
mod join_tests;
