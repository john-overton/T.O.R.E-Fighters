//! A networked flight in the game: joining from the command line, the
//! session's turn each frame, the flight it starts when the host seats the
//! player, what the screen presents from the client's frame (the events as
//! HUD lines, speech, sounds and rumble, and what the single-player tick
//! presenter does once a tick), and how the flight ends.
//!
//! The flight screen itself is `main.rs`'s: it draws a [`FlightFrame`], and
//! here the frame comes from the client session (`ClientFrame`) instead of
//! the world. The game keeps its own copy of the mission, never stepped, with
//! the player's plane taken as a handoff so every screen that reads a
//! cockpit finds one; each frame the cockpit is overwritten with the
//! client's prediction (agent decision).
use crate::{
    App, OWN, SEAT, Screen, TickPresenter,
    aircraft::Airframe,
    combat_view::CombatView,
    comms, flight, flight_views,
    frame::FlightFrame,
    net::{
        debrief,
        session::{EffectContext, GunContext, NetSession},
    },
    scenery, seats, show_selected_weapon_page,
};
use std::time::{Duration, Instant};
use tore_session::{
    ClientEvent, Controls,
    client::ended_text,
    wire::{
        events::{ReceivedEvent, WireEvent},
        names::NameIndex,
    },
};
use winit::event_loop::ActiveEventLoop;

/// The most client ticks a frame presents at tick rate (camera weather,
/// blackout, vapor, the situation music): a stall is not caught up on.
const MAX_PRESENTED_TICKS: u64 = 8;
/// How long a message that ends a session stays on the main menu.
const MESSAGE_SECONDS: u64 = 12;

/// The flight frame of the moment between redraws, for the commands that
/// read what the screen shows: the client's newest frame in a networked
/// flight, the world's otherwise.
pub(crate) fn current_frame<'a>(
    world: &'a crate::world::World,
    flight: &'a Option<NetFlight>,
    net: &'a Option<NetSession>,
) -> FlightFrame<'a> {
    match (flight.as_ref().and_then(|f| f.frame.as_ref()), net.as_ref()) {
        (Some(client), Some(net)) => client.flight_frame(
            [&net.effects.smoke, &net.effects.contrails],
            &net.effects.devices,
            || {
                world
                    .cockpit_readout(SEAT, crate::combat::launcher(&client.presented))
                    .expect("the seat flies a plane")
            },
        ),
        _ => crate::tick_frame(world, SEAT, &[]),
    }
}

/// The flight menu on screen: a session's has no time or mission-changing
/// rows ([`crate::flight_ui::session_menu`]).
pub(crate) fn menu<'a>(
    hornet: &'a Airframe,
    flight: &'a Option<NetFlight>,
) -> &'a [tore_formats::ui::MenuNode] {
    flight.as_ref().map_or(&hornet.flight_menu, |f| &f.menu)
}

/// What the app set aside to show a networked flight and puts back when it
/// ends, so single player starts from the state it left.
pub struct Stash {
    world: crate::world::World,
    combat_view: CombatView,
    hornet: Airframe,
    scenery: scenery::Scenery,
}

/// A networked flight on screen.
pub struct NetFlight {
    stash: Stash,
    /// The newest frame the client gave, which the screen draws.
    pub frame: Option<tore_session::ClientFrame>,
    /// The flight menu a session shows.
    pub menu: Vec<tore_formats::ui::MenuNode>,
    /// What the last frame showed, to see what changed.
    last_tick: u64,
    weather_steps: u64,
    was_dead: bool,
    was_crashed: bool,
    was_burning: bool,
}

impl App {
    /// Starts joining the server the command line named.
    pub(crate) fn start_session(&mut self, options: crate::net::options::ConnectOptions) {
        let data = match crate::assets::data_directory() {
            Ok(data) => data,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let server = options.server();
        crate::net::settings::remember_join(&data, &options);
        match crate::net::session::Join::connect(&options) {
            Ok(join) => {
                self.start_join(join, &server);
            }
            Err(error) => self.message(error),
        }
    }

    /// Starts the session over `join`, naming `server` to the player. False
    /// (with the reason shown) when it could not start.
    pub(crate) fn start_join(&mut self, join: crate::net::session::Join, server: &str) -> bool {
        let data = match crate::assets::data_directory() {
            Ok(data) => data,
            Err(error) => {
                self.error = Some(error);
                return false;
            }
        };
        match NetSession::start(
            join,
            std::sync::Arc::clone(&self.theater_resources),
            &data,
            self.replay_library.as_ref(),
        ) {
            Ok(session) => {
                self.net = Some(session);
                self.message(format!("Joining {server}..."));
                true
            }
            Err(error) => {
                self.message(error);
                false
            }
        }
    }

    /// A message for the player where they are: a HUD line in flight, the
    /// menu's message line otherwise, which stays long enough to read.
    pub(crate) fn message(&mut self, text: impl Into<String>) {
        let text = text.into();
        log::info!("Network: {text}");
        if self.screen == Screen::Flight {
            self.flight_ui.message(text);
        } else if let Some(screen) = &mut self.direct.screen {
            // The Direct Connection screen is where the player is looking.
            screen.say(&text);
        } else {
            self.menu.state.toast = Some((
                text,
                Instant::now() + Duration::from_secs(MESSAGE_SECONDS - 3),
            ));
        }
    }

    /// The session's turn: controls in, the network pumped, what happened
    /// handled, and, once flying, the frame the screen draws and what that
    /// frame presents. Called at the start of every redraw.
    pub(crate) fn net_tick(&mut self, event_loop: &ActiveEventLoop) {
        if self.net.is_none() {
            return;
        }
        let flying = self.net_flight.is_some() && self.screen == Screen::Flight;
        let sensors = self.instruments.controls();
        let mut controls = if flying && !self.flight_ui.frozen() && self.focused {
            self.session_controls(sensors)
        } else {
            Controls::neutral(sensors)
        };
        if flying {
            controls.commands = std::mem::take(&mut self.seat_commands);
        }
        let session = self.net.as_mut().expect("a session");
        session.pump(&controls);
        if let Some(failure) = session.take_host_failure() {
            self.net_ending = Some(failure);
            self.end_session(event_loop);
            return;
        }
        for event in session.take_events() {
            if !self.net_event(event) {
                self.end_session(event_loop);
                return;
            }
        }
        // A game hosted from the command line starts each mission as soon as
        // everyone holding a slot is ready and its player has closed the
        // debrief.
        let reading = self.quick.debrief.is_some();
        if let Some(session) = &mut self.net {
            session.auto_start(!reading && self.net_flight.is_none());
        }
        if self.net_flight.is_none() && !self.begin_session_flight(event_loop) {
            return;
        }
        if self.net_flight.is_some() && self.screen == Screen::Flight {
            self.net_present();
        }
    }

    /// The pilot's controls for this frame from the keyboard, the mouse and
    /// the controllers, as a tick of single player takes them.
    fn session_controls(&mut self, sensors: tore_sim::sensors::Controls) -> Controls {
        let frame = self.net_flight.as_ref().and_then(|f| f.frame.as_ref());
        if let Some(service) = frame.and_then(|f| f.readout.as_ref()?.airport.service.as_ref()) {
            self.instruments.navigation.refresh(
                &self.world.terrain.airport_scene,
                service,
                self.world.cockpits[OWN].flight.position,
            );
            for button in std::mem::take(&mut self.instruments.navigation.pending) {
                if let Some(id) = self.instruments.navigation.control(button) {
                    crate::queue_command(
                        &mut self.seat_commands,
                        seats::SeatCommand::Airport(crate::world::AirportInput::Command(
                            tore_sim::airport::Command::SelectAirport(id),
                        )),
                    );
                }
            }
        }
        let throttle = self.world.cockpits[OWN].flight.throttle;
        let (pilot, _) = self.input.frame(&self.camera.keys, throttle);
        Controls {
            pilot,
            trigger: self.input.resolver.held("fire"),
            sensors,
            commands: Vec::new(),
            view_subject: None,
        }
    }

    /// One session event. `false` ends the session.
    fn net_event(&mut self, event: ClientEvent) -> bool {
        match event {
            ClientEvent::Connected { .. } => {
                if let Some(screen) = &mut self.direct.screen {
                    screen.say("Connected. Loading the game's mission...");
                }
            }
            ClientEvent::MissionLoaded => {
                let session = self.net.as_mut().expect("a session");
                match session.take_built() {
                    Some(Ok(built)) => self.net_built = Some(built),
                    Some(Err(error)) => {
                        self.net_ending = Some(format!("The mission could not be built: {error}"));
                        return false;
                    }
                    None => {}
                }
                if self.net_flight.is_none() {
                    self.message("Mission loaded; taking a plane...");
                }
            }
            ClientEvent::ContentRefused { names, reason } => {
                log::warn!("Network: {reason} ({})", names.join(", "));
                // With no lobby screen to wait in, the game leaves.
                self.net_ending = Some(reason);
                return false;
            }
            ClientEvent::MissionFailed(text) => {
                self.net_ending = Some(format!("The mission could not be built: {text}"));
                return false;
            }
            ClientEvent::SeatRefused(text) => {
                // The client asks for any free plane by itself.
                self.message(format!("No plane: {text}"));
            }
            ClientEvent::Seated { plane, .. } => {
                log::info!("Network: seated in plane {plane}");
            }
            ClientEvent::Roster => {}
            ClientEvent::Notice(text) => self.message(text),
            ClientEvent::Debrief(debrief) => {
                // A player leaving the game sees it when the session ends; one
                // who stays sees it now, back in the lobby.
                let leaving = self.net.as_ref().is_some_and(|s| s.left_at.is_some());
                if !leaving {
                    if let Some(session) = &mut self.net {
                        session.debrief = None;
                    }
                    self.show_net_debrief(&debrief);
                }
            }
            ClientEvent::MissionEnded(ended) => {
                let text = ended_text(&ended);
                self.end_net_flight();
                self.message(text);
            }
            ClientEvent::Lobby => {
                if let Some(line) = self.net.as_mut().and_then(|s| s.lobby_change()) {
                    log::info!("Network: lobby: {line}");
                }
            }
            ClientEvent::Refused { reason, .. } => self.message(reason),
            ClientEvent::Goodbye(_) => {}
            ClientEvent::Closed(reason) => {
                let left = self.net.as_ref().is_some_and(|s| s.left_at.is_some());
                let text = self
                    .net
                    .as_ref()
                    .map(|s| s.client.close_text(&reason))
                    .unwrap_or_default();
                self.net_ending = Some(if left { String::new() } else { text });
                return false;
            }
        }
        true
    }

    /// The debrief the host sent, on its screen, which closes to the main
    /// menu.
    fn show_net_debrief(&mut self, debrief: &tore_session::wire::messages::Debrief) {
        match crate::debrief::Debrief::new(debrief::report(debrief), &self.theater_resources, None)
        {
            Ok(screen) => {
                self.quick.debrief = Some(screen);
                self.quick.debrief_to_menu = true;
                self.screen = Screen::Quick;
                if let Some(renderer) = &self.renderer {
                    renderer.window.set_title("T.O.R.E-Fighters - Debrief");
                    renderer.window.request_redraw();
                }
            }
            Err(error) => self.message(error.to_string()),
        }
    }

    /// The flight is over but the session goes on (the mission ended, back
    /// in the lobby): the flight is put away and single player's state put
    /// back, as when a session ends.
    fn end_net_flight(&mut self) {
        let Some(flight) = self.net_flight.take() else {
            return;
        };
        let stash = flight.stash;
        self.world = stash.world;
        self.combat_view = stash.combat_view;
        self.hornet = stash.hornet;
        self.scenery = stash.scenery;
        if let Some(renderer) = &mut self.renderer {
            renderer.set_scenery(&self.scenery);
            renderer.prepare_aircraft(&self.hornet);
            renderer.combat(&Default::default());
            renderer
                .window
                .set_title("T.O.R.E-Fighters - Choose Activity");
        }
        self.flight_ui.reset_for_flight();
        self.camera.keys.clear();
        self.seat_commands.clear();
        self.input.context(true, self.focused);
        self.screen = Screen::Main;
        if let Some(audio) = &self.audio {
            audio.scene(crate::audio::music::Scene::Main);
        }
    }

    /// Starts the flight once the host has seated the player and the mission
    /// is built. `false` when the session ended instead.
    fn begin_session_flight(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(session) = &self.net else {
            return true;
        };
        let Some((_, plane)) = session.client.seat() else {
            return true;
        };
        let Some(built) = self.net_built.take() else {
            return true;
        };
        let plane = plane.0;
        let crate::net::session::Built { mut world, models } = built;
        let aircraft = match world.ai_wings.as_ref().and_then(|wings| wings.slot(plane)) {
            Some(slot) => slot.aircraft,
            None => {
                self.net_ending = Some(format!(
                    "The server seated plane {plane}, which this mission does not have"
                ));
                self.end_session(event_loop);
                return false;
            }
        };
        let resources = std::sync::Arc::clone(&self.theater_resources);
        let result = (|| -> crate::AppResult<_> {
            world
                .take_plane(SEAT, crate::seats::PlaneId(plane))
                .map_err(|error| error.to_string())?;
            let hornet = Airframe::load(&resources, aircraft)?;
            let view = CombatView::with_models(&world.combat, plane, &resources, models)
                .map_err(|error| error.to_string())?;
            let scenery = scenery::Scenery::build(&resources, &world.terrain)?;
            Ok((hornet, view, scenery))
        })();
        let (hornet, view, scenery) = match result {
            Ok(parts) => parts,
            Err(error) => {
                self.net_ending = Some(format!("The flight could not start: {error}"));
                self.end_session(event_loop);
                return false;
            }
        };
        // Single player's state waits aside.
        let stash = Stash {
            world: std::mem::replace(&mut self.world, world),
            combat_view: std::mem::replace(&mut self.combat_view, view),
            hornet: std::mem::replace(&mut self.hornet, hornet),
            scenery: std::mem::replace(&mut self.scenery, scenery),
        };
        if let Some(renderer) = &mut self.renderer {
            renderer.set_scenery(&self.scenery);
            renderer.prepare_aircraft(&self.hornet);
        }
        if let Some(audio) = &self.audio {
            audio.restart_flight();
        }
        self.input.context(true, self.focused);
        self.seat_commands.clear();
        self.cheats_sent = None;
        self.instruments.navigation = crate::navigation::Navigation::default();
        self.scenery.reset_presentations();
        self.flight_music = crate::flight_music::Observer::new();
        self.rwr_warnings = Default::default();
        self.reset_vapor();
        self.g_effects = Default::default();
        self.flight_clock.remainder = 0.;
        self.flight_view = 0;
        self.view_rig = flight_views::Rig::for_plane(plane);
        let saved = crate::preferences::Preferences::capture(
            &self.flight_ui,
            &self.instruments,
            self.fullscreen_preference,
        );
        self.flight_ui.reset_for_flight();
        self.live_debug.reset();
        saved.apply(&mut self.flight_ui, &mut self.instruments);
        self.flight_ui.enter_session();
        self.screen = Screen::Flight;
        self.camera.keys.clear();
        self.quick.cancel();
        self.frame_time = Instant::now();
        if let Some(renderer) = &self.renderer {
            renderer.window.set_title(&format!(
                "T.O.R.E-Fighters - {} Multiplayer",
                self.hornet.profile.id.label()
            ));
        }
        if let Some(audio) = &self.audio {
            audio.scene(crate::audio::music::Scene::Score(0));
        }
        self.net_flight = Some(NetFlight {
            stash,
            frame: None,
            menu: crate::flight_ui::session_menu(&self.hornet.flight_menu),
            last_tick: 0,
            weather_steps: 0,
            was_dead: false,
            was_crashed: false,
            was_burning: false,
        });
        true
    }

    /// This frame's client frame, its effects, and everything it presents.
    fn net_present(&mut self) {
        let Some(session) = self.net.as_mut() else {
            return;
        };
        let Some(mut frame) = session.frame() else {
            return;
        };
        session.step_guns(
            &mut frame,
            &GunContext {
                terrain: &self.world.terrain,
                configurations: self.world.combat.dummy_configurations(),
            },
        );
        let flight = self.net_flight.as_mut().expect("a networked flight");
        let ticks = frame
            .tick
            .saturating_sub(flight.last_tick)
            .min(MAX_PRESENTED_TICKS)
            .max(u64::from(flight.last_tick == 0));
        flight.last_tick = frame.tick;

        // The screens that read a cockpit read the client's prediction.
        let cockpit = &mut self.world.cockpits[OWN];
        cockpit.previous_flight.clone_from(&frame.previous);
        cockpit.flight.clone_from(&frame.flight);
        if let Some(readout) = &frame.readout {
            cockpit.airport_nav_mode = readout.airport.nav_mode;
            if let Some(service) = &readout.airport.service {
                cockpit.airport_service.clone_from(service);
            }
        }
        // Ground objects the host has destroyed stop standing.
        for pose in frame
            .picture
            .targets
            .iter()
            .filter(|pose| !pose.airborne && pose.damage.hp <= 0)
        {
            if let Some(target) = self
                .world
                .combat
                .state
                .targets
                .iter_mut()
                .find(|target| target.id == pose.id)
            {
                target.hp = 0;
            }
        }
        self.combat_view.show_picture(frame.picture.clone());

        // The weather clock follows the host's tick.
        let behind = frame.tick.saturating_sub(flight.weather_steps);
        let bulk = behind.saturating_sub(ticks);
        for _ in 0..bulk {
            self.world.terrain.weather.step();
        }
        flight.weather_steps += bulk;

        // Smoke, contrails, chaff and flares.
        {
            let models: Vec<&Airframe> = self
                .combat_view
                .models
                .iter()
                .chain(std::iter::once(&self.hornet))
                .collect();
            session.step_effects(
                frame.tick,
                &frame.picture,
                &EffectContext {
                    terrain: &self.world.terrain,
                    models: &models,
                    resources: &self.theater_resources,
                    sortie: self.world.combat.contrail_sortie(),
                },
            );
        }

        // What the frame shows, presented.
        let session = self.net.as_ref().expect("a session");
        let effects = &session.effects;
        let world = &self.world;
        let presented = frame.flight_frame(
            [&effects.smoke, &effects.contrails],
            &effects.devices,
            || {
                let launcher = crate::combat::launcher(&frame.presented);
                world
                    .cockpit_readout(SEAT, launcher)
                    .expect("the seat flies a plane")
            },
        );
        let name = |index: NameIndex| session.client.name(index).map(str::to_owned);
        let mut presenter = TickPresenter {
            world: &mut self.world,
            scenery: &mut self.scenery,
            flight_ui: &mut self.flight_ui,
            input: &mut self.input,
            audio: self.audio.as_ref(),
            recorder: None,
            instruments: &mut self.instruments,
            flight_view: &mut self.flight_view,
            view_rig: &mut self.view_rig,
            hornet: &self.hornet,
            combat_view: &self.combat_view,
            head_look: self.head_look,
            g_effects: &mut self.g_effects,
            perf_blackout: self.performance.veil_level(),
            vapor: &mut self.vapor,
            flight_music: &mut self.flight_music,
            rwr_warnings: &mut self.rwr_warnings,
        };
        let mut state = Seen {
            was_dead: flight.was_dead,
            was_crashed: flight.was_crashed,
            was_burning: flight.was_burning,
        };
        presenter.present_net(&presented, &frame.events, &name, ticks, &mut state);
        flight.was_dead = state.was_dead;
        flight.was_crashed = state.was_crashed;
        flight.was_burning = state.was_burning;
        drop(presented);
        flight.frame = Some(frame);
    }

    /// End Mission: the hosting player ends the mission for everyone; any
    /// other leaves the game with its debrief.
    pub(crate) fn leave_session(&mut self) {
        let hosting = self.net.as_ref().is_some_and(NetSession::hosting);
        if let Some(session) = &mut self.net {
            session.leave();
        }
        self.message(if hosting {
            "Ending the mission for everyone..."
        } else {
            "Leaving the mission..."
        });
    }

    /// The session is over: its files are done, the flight is put away, and
    /// the player sees the debrief the host sent, or the reason in plain
    /// words on the main menu.
    pub(crate) fn end_session(&mut self, event_loop: &ActiveEventLoop) {
        let reason = self.net_ending.take().unwrap_or_default();
        let debrief = self.net.as_mut().and_then(|session| session.debrief.take());
        let capture = self.net.as_ref().and_then(|s| s.capture.clone());
        if let Some(session) = &mut self.net {
            let now = session.now();
            session.client.disconnect(now);
            session.flush();
        }
        self.net = None;
        self.net_built = None;
        if let Some(flight) = self.net_flight.take() {
            let stash = flight.stash;
            self.world = stash.world;
            self.combat_view = stash.combat_view;
            self.hornet = stash.hornet;
            self.scenery = stash.scenery;
            if let Some(renderer) = &mut self.renderer {
                renderer.set_scenery(&self.scenery);
                renderer.prepare_aircraft(&self.hornet);
                renderer.combat(&Default::default());
            }
            self.flight_ui.reset_for_flight();
            self.camera.keys.clear();
            self.seat_commands.clear();
            self.input.context(true, self.focused);
            self.screen = Screen::Main;
            if let Some(audio) = &self.audio {
                audio.scene(crate::audio::music::Scene::Main);
            }
        }
        if let Some(path) = capture {
            // The finished capture counts in the replays' pruning from now.
            log::info!("Network capture: {}", path.display());
        }
        if let Some(report) = debrief.as_ref().map(debrief::report) {
            match crate::debrief::Debrief::new(report, &self.theater_resources, None) {
                Ok(screen) => {
                    self.quick.debrief = Some(screen);
                    self.quick.debrief_to_menu = true;
                    self.screen = Screen::Quick;
                }
                Err(error) => self.message(error.to_string()),
            }
        }
        if !reason.is_empty() {
            self.message(reason);
        }
        if let Some(renderer) = &self.renderer {
            renderer.window.set_title(&match self.screen {
                Screen::Quick => "T.O.R.E-Fighters - Debrief".to_owned(),
                _ => "T.O.R.E-Fighters - Choose Activity".to_owned(),
            });
            renderer.window.request_redraw();
        }
        let _ = event_loop;
    }
}

/// What the presenter saw last frame, to tell what changed.
pub struct Seen {
    pub was_dead: bool,
    pub was_crashed: bool,
    pub was_burning: bool,
}

impl TickPresenter<'_> {
    /// Presents one networked frame as single player presents ticks: the
    /// events the host sent as HUD lines, speech, sounds and rumble, and
    /// `ticks` steps of what follows the flight (camera weather, blackout,
    /// vapor, the situation music and the warning tones).
    pub(crate) fn present_net(
        &mut self,
        frame: &FlightFrame,
        events: &[ReceivedEvent],
        name: &dyn Fn(NameIndex) -> Option<String>,
        ticks: u64,
        seen: &mut Seen,
    ) {
        let plane = frame.plane.0;
        let mut weapon_cycled = false;
        let mut releases: Vec<String> = Vec::new();
        let mut emissions: Vec<tore_sim::acoustics::Emission> = Vec::new();
        for received in events {
            match &received.event {
                WireEvent::Message { text } => self.flight_ui.message(text.clone()),
                WireEvent::Radio {
                    route,
                    label,
                    text,
                    stems,
                    ..
                } => {
                    let stems: Vec<String> = stems.iter().filter_map(|stem| name(*stem)).collect();
                    match route {
                        comms::Route::Radio | comms::Route::Airport => {
                            self.flight_ui.message(format!("{label}: '{text}'"));
                            if let Some(audio) = self.audio {
                                if *route == comms::Route::Airport {
                                    audio.airport_speech(&stems);
                                } else {
                                    audio.speech(&stems);
                                }
                            }
                        }
                        comms::Route::Direct => {
                            if let (Some(audio), Some(stem)) = (self.audio, stems.first()) {
                                audio.direct_voice(stem);
                            }
                        }
                    }
                }
                WireEvent::Tower { stem } => {
                    if let Some(audio) = self.audio {
                        match stem.and_then(name) {
                            Some(stem) => audio.airport_radio(&[stem.as_str()]),
                            None => audio.cancel_airport_radio(),
                        }
                    }
                }
                WireEvent::OrderVoice { stems } => {
                    if let Some(audio) = self.audio {
                        let stems: Vec<String> =
                            stems.iter().filter_map(|stem| name(*stem)).collect();
                        let refs: Vec<&str> = stems.iter().map(String::as_str).collect();
                        audio.radio(&refs, true);
                    }
                }
                WireEvent::OrderReply { .. } => {}
                WireEvent::WeaponCycled => weapon_cycled = true,
                WireEvent::Release { sound, .. } => {
                    if let Some(sound) = name(*sound) {
                        releases.push(sound);
                    }
                }
                WireEvent::Feedback { rumble } => self.input.feedback(rumble.event()),
                WireEvent::YourAircraftExploded { on_impact } => {
                    self.flight_ui.message(if *on_impact {
                        "Your aircraft exploded on impact"
                    } else {
                        "Your aircraft exploded"
                    });
                }
                WireEvent::WingEjection {
                    message, friendly, ..
                } => {
                    self.flight_ui.message(message.clone());
                    if *friendly && let Some(audio) = &self.audio {
                        audio.wingman_ejected();
                    }
                }
                WireEvent::Sound {
                    kind,
                    position,
                    arrived,
                    from,
                } => emissions.push(tore_sim::acoustics::Emission {
                    kind: *kind,
                    position: position
                        .map(|v| v as f64 * tore_session::wire::entity::POSITION_STEP),
                    arrived: *arrived,
                    own: *from == Some(plane),
                }),
                // Their effects are in the picture, the regenerated devices
                // and the view rig, or arrive as the entities they make.
                WireEvent::Effect { .. }
                | WireEvent::Mark { .. }
                | WireEvent::GroundDestroyed { .. }
                | WireEvent::Countermeasure { .. }
                | WireEvent::Launch { .. }
                | WireEvent::GunBurst { .. } => {}
            }
        }
        if weapon_cycled {
            show_selected_weapon_page(frame, &self.world.combat, self.instruments);
        }

        let flight = frame.flight;
        let dead = flight.systems.pilot.dead || flight.escape.is_some();
        if let Some(view) = self.flight_ui.pilot_death_view(seen.was_dead, dead) {
            *self.flight_view = view;
            self.view_rig.select(flight_views::Reference::Player);
        }
        seen.was_dead = dead;

        // The tick-rate presentation of the flight.
        self.flown_net(frame, ticks);
        let danger = tore_sim::ejection::assess(flight, |x, z| {
            f64::from(self.world.terrain.height(x as f32, z as f32))
        })
        .is_some();
        if let Some(audio) = self.audio {
            audio.ejection(frame.previous, flight, danger);
        }
        if let Some(audio) = self.audio {
            for _ in 0..ticks {
                let music = self.flight_music.step(frame, &[], &self.world.terrain);
                audio.situation(&music.inputs, music.now);
                audio.rwr(self.rwr_warnings.step(
                    frame.readout.tick,
                    crate::rwr_tone::inbound(&frame.readout, flight.position),
                    &frame.readout.rwr.locks,
                    flight.escape.is_some() || flight.systems.pilot.dead,
                ));
            }
            audio.seeker(frame.readout.seeker.tone);
        }
        if let Some(audio) = self.audio {
            let scene =
                flight_views::Scene::from_frame(frame, flight, self.world.ai_wings.as_ref());
            let listener_camera = self
                .view_rig
                .clone()
                .camera(
                    *self.flight_view,
                    &scene,
                    self.hornet
                        .camera(flight, *self.flight_view, Default::default()),
                    crate::look::combine(
                        self.flight_ui.look,
                        self.head_look,
                        matches!(*self.flight_view, 1 | 2),
                    ),
                    self.flight_ui.zoom,
                )
                .unwrap_or_else(|_| self.hornet.camera(flight, 0, Default::default()));
            let basis = tore_sim::attitude::Basis::new(
                f64::from(listener_camera.yaw),
                f64::from(listener_camera.pitch),
                -f64::from(listener_camera.roll),
            );
            let models: Vec<_> = self
                .combat_view
                .models
                .iter()
                .map(|model| &*model.kind)
                .chain([&**self.hornet])
                .map(|model| {
                    (
                        model.profile.id,
                        crate::audio::EngineSounds::of(&model.profile),
                    )
                })
                .collect();
            let loops = crate::audio::loop_sources(frame.picture, &models);
            let sources = crate::audio::snapshot_sources(frame.picture);
            let own_releases: Vec<&str> = releases.iter().map(String::as_str).collect();
            for step in 0..ticks.max(1) {
                let first = step == 0;
                audio.spatial_tick(
                    tore_sim::acoustics::Listener {
                        position: listener_camera.position,
                        right: basis.right,
                        view: *self.flight_view,
                        external: !self.view_rig.cockpit(*self.flight_view),
                        own: Some(plane),
                    },
                    &sources,
                    if first { &emissions } else { &[] },
                    if first { &own_releases } else { &[] },
                    flight.position,
                    Some((flight.position, flight.velocity)),
                    &loops,
                );
            }
        }
        if flight.crashed && !seen.was_crashed && flight.escape.is_none() {
            self.input.feedback(tore_input::FeedbackEvent::Crash);
        }
        seen.was_crashed = flight.crashed;
        let burning = flight.afterburner_active();
        if burning && !seen.was_burning {
            self.input
                .feedback(tore_input::FeedbackEvent::AfterburnerEngaged);
        }
        seen.was_burning = burning;
        self.input.afterburner_feedback(burning);
        self.input.feedback_tick();
    }

    /// `flown` for a networked frame: the camera slots' weather, the view
    /// rig, blackout and redout, the wing vapor and the control sounds,
    /// stepped `ticks` times where single player steps them once a tick.
    fn flown_net(&mut self, frame: &FlightFrame, ticks: u64) {
        let flight = frame.flight;
        let speed = flight.speed;
        let scene = flight_views::Scene::from_frame(frame, flight, self.world.ai_wings.as_ref());
        let weather_view = self
            .view_rig
            .camera(
                *self.flight_view,
                &scene,
                self.hornet
                    .camera(flight, *self.flight_view, Default::default()),
                crate::look::combine(
                    self.flight_ui.look,
                    self.head_look,
                    matches!(*self.flight_view, 1 | 2),
                ),
                self.flight_ui.zoom,
            )
            .unwrap_or_else(|_| self.hornet.camera(flight, 0, Default::default()));
        self.view_rig.observe(&scene);
        let other = self
            .view_rig
            .other_camera(
                &scene,
                self.hornet
                    .camera(flight, self.view_rig.other_view(), Default::default()),
            )
            .ok();
        let target = self
            .combat_view
            .target_camera(&self.world.combat, &frame.readout, flight);
        let mirror = crate::mirrors::camera(flight);
        let panel = self.hornet.panel_camera(flight, 2);
        for _ in 0..ticks {
            self.world.terrain.weather.step();
            self.scenery
                .step_view_weather(&self.world.terrain, &weather_view, speed);
            self.scenery
                .step_view_weather(&self.world.terrain, &mirror, speed);
            self.scenery
                .step_view_weather(&self.world.terrain, &panel, speed);
            for camera in other.iter().chain(target.iter()) {
                self.scenery
                    .step_view_weather(&self.world.terrain, camera, speed);
            }
            self.g_effects.step(
                flight.g,
                !self.flight_ui.cheats.no_g_effects && !flight.crashed && flight.native.is_none(),
            );
            if let Some(level) = self.perf_blackout {
                self.g_effects.blackout = level;
            }
            if let Some(points) = self.hornet.streamer_points(flight) {
                self.vapor.step(self.world.terrain.weather.ticks(), points);
            }
        }
        if let Some(audio) = self.audio {
            audio.controls(frame.previous, flight);
        }
        let _ = flight::DT;
    }
}
