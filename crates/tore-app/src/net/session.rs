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
use tore_net::{Datagrams, Entropy, LinkEnd, RealClock};
use tore_session::{
    BuildId, Client, ClientConfig, ClientEvent, ClientFrame, Controls, wire::events::WireEvent,
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

/// What carries the session's datagrams: a UDP socket to a server, or the
/// in-process link to the host this game runs itself.
pub enum Transport {
    Udp(UdpSocket),
    Link(LinkEnd),
}

impl Datagrams for Transport {
    fn send_datagram(&mut self, to: SocketAddr, datagram: &[u8]) -> io::Result<()> {
        match self {
            Self::Udp(socket) => socket.send_datagram(to, datagram),
            Self::Link(link) => link.send_datagram(to, datagram),
        }
    }

    fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        match self {
            Self::Udp(socket) => socket.recv_datagram(buf),
            Self::Link(link) => link.recv_datagram(buf),
        }
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
}

impl Join {
    /// A join to the server `options` name: the name looked up, a socket
    /// opened.
    pub fn connect(options: &ConnectOptions) -> Result<Self, String> {
        let server = options.resolve()?;
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
            callsign: options.callsign.clone(),
            slot: options.slot,
            password: options.password.clone(),
            label: options.host.clone(),
        })
    }
}

/// A joined (or joining) session.
pub struct NetSession {
    pub client: Client,
    socket: Transport,
    clock: RealClock,
    sink: Sink,
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
    /// The host this game runs, when it hosts the session; dropping the
    /// session stops it.
    pub hosting: Option<HostThread>,
    /// Why the host this game runs ended without the session ending: its
    /// mission could not be built, or it panicked.
    host_failure: Option<String>,
    /// The lobby state the King's automatic start was last asked for.
    started_for: Option<tore_session::wire::messages::LobbyState>,
    /// The lobby's last line in the log.
    lobby_line: Option<String>,
}

/// How long after Leave the game waits for the debrief and the disconnect
/// before it quits the connection itself.
pub const LEAVE_GRACE: Duration = Duration::from_secs(8);

impl NetSession {
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
            transport: socket,
            callsign,
            slot,
            password,
            label,
        } = join;
        let clock = RealClock::new();
        let config = ClientConfig {
            password,
            plane: slot,
            entropy: Entropy::System,
            retail_stall_speeds: tore_sim::flight::retail_stall_speeds(),
            ..ClientConfig::new(server, &callsign, build_id())
        };
        let mut client = Client::connect(config, Arc::clone(&resources), clock.now())
            .map_err(|error| error.to_string())?;

        // The files: the log, and a capture beside the replays, pruned by the
        // replays' own rule before a new one is made.
        let mut capture = None;
        if let Some(library) = replays {
            let now = SystemTime::now();
            let settings = library.settings();
            let cleanup = files::prune(library, data, &settings, now, &[]);
            for (path, error) in &cleanup.failed {
                log::warn!(
                    "Network files: could not remove {}: {error}",
                    path.display()
                );
            }
            match files::create_capture(library, now, &label) {
                Ok((path, file)) => {
                    client.set_capture(Box::new(BufWriter::new(file)));
                    capture = Some(path);
                }
                Err(error) => log::warn!("Network capture not written: {error}"),
            }
        }
        client.set_diagnostics(Box::new(DatedLog::new(data.join(files::LOG_FOLDER))));

        let sink: Sink = Rc::new(RefCell::new(None));
        let (map, kept) = (Arc::clone(&resources), Rc::clone(&sink));
        client.set_mission_builder(Box::new(move |spec, reads: &dyn ResourceSource| {
            // The game's own build for the screens, then the session's: a
            // failure of either is the mission's.
            *kept.borrow_mut() = Some(build_mission(spec, &map));
            // Read every resource the plain build reads, through `reads`, so
            // the manifest the host compares is the simulation's own.
            World::new(spec, reads, Seating::Open)
        }));
        Ok(Self {
            client,
            socket,
            clock,
            sink,
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
            hosting: None,
            host_failure: None,
            started_for: None,
            lobby_line: None,
        })
    }

    /// Whether this game hosts the session: its player is the King.
    pub fn hosting(&self) -> bool {
        self.hosting.is_some()
    }

    /// The King's start for a game hosted from the command line (agent
    /// decision, until the lobby screen's Fly button, EF8): as soon as
    /// every player holding a slot is ready, when `allowed` (the hosting
    /// player is not reading a debrief), once for each lobby state.
    pub fn auto_start(&mut self, allowed: bool) {
        if !allowed || self.hosting.is_none() {
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
        self.trigger = self.fire.turn(&controls.commands, controls.trigger);
        let _ = self.client.receive_from(now, &mut self.socket);
        self.client.update(now, controls);
        let _ = self.client.transmit(&mut self.socket);
        while let Some(event) = self.client.poll_event() {
            if let ClientEvent::Debrief(debrief) = &event {
                self.debrief = Some((**debrief).clone());
            }
            self.events.push(event);
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

    /// End Mission in flight. The hosting player, the King, ends the
    /// mission for everyone, who return to the lobby with their debriefs
    /// (and the King's game starts the next once the debrief is closed);
    /// any other player leaves the game, with its debrief, since a game with
    /// no lobby screen has nowhere else to go (agent decisions, EF4).
    pub fn leave(&mut self) {
        if self.hosting.is_some() {
            self.client.end_mission();
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
