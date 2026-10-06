//! The game's side of the Internet Lobby screen: opening it, building its kit
//! once, routing the window's events to it and carrying out what it asks for
//! (join, host, leave), and the player's Report when a session joined through
//! it ends. `main.rs` only calls these; the screen itself is
//! [`super::InternetScreen`].
//!
//! The screen shares Direct Connection's pieces: the kit is the one Direct
//! Connection builds (`MODEM3`'s palette), kept in [`crate::direct_screen::app::Direct`]
//! for the life of the game, so a second visit to either opens at once. A join
//! or New that starts a session opens the lobby screen
//! ([`crate::lobby_screen`]) over this one, as it does over Direct
//! Connection, and these calls hand the window's events to whichever is up.
use super::{HostRequest, InternetJoin, InternetScreen, Outcome};
use crate::menu::Action;
use crate::net::{
    browse::MasterJoin,
    options::HostOptions,
    session::{Introduction, Join, MasterTransport, ThroughFacts, Transport},
    telemetry::{self, PlayerTally},
};
use crate::{App, Screen};
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use winit::event_loop::ActiveEventLoop;

use crate::widgets::Kit;

/// A session joined through the Internet Lobby whose Report is due when it
/// ends: the master to send it to, the install id to send it under and what
/// the session has counted.
struct Reporting {
    master: String,
    install_id: Option<u64>,
    tally: PlayerTally,
    /// The path the join took has been counted.
    said: bool,
    /// From pressing Join to the master's answer.
    asked: Duration,
    /// From the session's start to the host accepting the join, once it has.
    connected: Option<Duration>,
    /// The player's mapping type and the bytes relayed, as last seen.
    facts: ThroughFacts,
}

impl Reporting {
    /// The Report for the session that ended, under `install_id`: the
    /// tally's, with what the join through the master adds (slice J5): the
    /// time from asking for the introduction to the handshake's end (when it
    /// ended), the player's mapping type and the kilobytes relayed.
    fn report(&self, install_id: u64, version: &str) -> tore_net::master::Report {
        let mut report = self.tally.report(install_id, version);
        if let Some(connected) = self.connected {
            let tenths = (self.asked + connected).as_millis() / 100;
            report.connect_tenths = u8::try_from(tenths).unwrap_or(u8::MAX);
        }
        report.mapping = self.facts.mapping;
        report.relayed_kb = u32::try_from(self.facts.relayed_bytes / 1_000).unwrap_or(u32::MAX);
        // Stage K: the migrations the session resumed through or was lost to.
        report.migrations = self.facts.migrations;
        report.failed_migrations = self.facts.failed_migrations;
        report
    }
}

/// The session's join for a join the Internet Lobby started: the socket and
/// joiner the master's introduction ran on, the host's addresses to race and
/// the relay when the race finds nothing (slice J5). A join the screen starts
/// opens the lobby, which shows the joining, and a refusal ends it back on
/// the screen.
pub(crate) fn session_join(request: &InternetJoin, through: MasterJoin) -> Join {
    let (socket, joiner) = through.into_parts();
    let introduction = Introduction {
        race: request.race.clone(),
        relay_now: request.relay_now,
        path: request.path,
        asked: request.asked,
    };
    Join {
        server: request.join.address,
        transport: Transport::Internet(MasterTransport::new(socket, joiner, introduction)),
        callsign: request.join.callsign.clone(),
        slot: None,
        password: request.join.password.clone(),
        label: request.join.label.clone(),
        lobby: true,
    }
}

/// What the game keeps for the screen: the screen while it is open, the kit it
/// is being built with (the first visit of the game, when Direct Connection
/// has not built it yet), and the Report due.
#[derive(Default)]
pub struct Internet {
    pub screen: Option<InternetScreen>,
    building: Option<mpsc::Receiver<Result<Kit, String>>>,
    report: Option<Reporting>,
}

impl Internet {
    /// The kit is being built.
    pub fn is_building(&self) -> bool {
        self.building.is_some()
    }
}

impl App {
    /// The screen is open and showing: over Choose Activity.
    pub(crate) fn internet_open(&self) -> bool {
        self.screen == Screen::Main && self.internet.screen.is_some()
    }

    /// The Multi menu's Internet Lobby row.
    pub(crate) fn open_internet(&mut self) {
        if self.internet.screen.is_some() || self.internet.building.is_some() {
            return;
        }
        self.menu.state.cancel();
        // Stage L: this game's content is worked out on a worker, once, so a
        // join or a host does not wait for it.
        crate::net::session::start_content(&self.theater_resources);
        if self.direct.kit().is_some() {
            self.make_internet_screen();
            return;
        }
        let source = Arc::clone(&self.menu.kit_source);
        let (send, receive) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("tore-kit".into())
            .spawn(move || {
                let _ = send.send(source.build("MODEM3").map_err(|error| error.to_string()));
            });
        match spawned {
            Ok(_) => {
                self.internet.building = Some(receive);
                self.menu.state.toast = Some((
                    "Opening the Internet Lobby...".to_owned(),
                    std::time::Instant::now() + Duration::from_secs(30),
                ));
            }
            Err(error) => self.message(format!("Cannot open the Internet Lobby: {error}")),
        }
    }

    fn make_internet_screen(&mut self) {
        let Some(kit) = self.direct.kit() else {
            return;
        };
        let data: Option<PathBuf> = crate::assets::data_directory().ok();
        self.internet.screen = Some(InternetScreen::new(kit, data));
        self.menu.state.cancel();
        self.mouse_look = None;
        if let Some(renderer) = &self.renderer {
            renderer.window.request_redraw();
        }
    }

    /// The game's turn for the screen, before a frame is drawn: the kit's
    /// build taken when it is done, the screen's own turn, what it asks for,
    /// and the Report of a session that ended.
    pub(crate) fn internet_tick(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(receive) = &self.internet.building {
            match receive.try_recv() {
                Ok(Ok(kit)) => {
                    self.internet.building = None;
                    self.direct.keep_kit(Arc::new(kit));
                    self.make_internet_screen();
                }
                Ok(Err(error)) => {
                    self.internet.building = None;
                    self.menu.state.toast = None;
                    self.message(format!("Cannot open the Internet Lobby: {error}"));
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.internet.building = None;
                    self.message("Cannot open the Internet Lobby: the pieces did not build.");
                }
            }
        }
        self.report_tick();
        let session = self.net.is_some();
        // Flying, or under a debrief with no session, the screen has no turn.
        if self.screen == Screen::Flight || (self.screen != Screen::Main && !session) {
            return;
        }
        let Some(screen) = &mut self.internet.screen else {
            return;
        };
        let outcome = screen.update(session);
        let action = self.internet_outcome(outcome);
        if action != Action::None {
            self.action(event_loop, action);
        }
    }

    /// Counts a joined session's humans, the path it took and what its join
    /// through the master saw, and sends its Report once it ends. The session
    /// itself says in Messages how it connected (slice J5).
    fn report_tick(&mut self) {
        let Some(report) = &mut self.internet.report else {
            return;
        };
        if let Some(session) = &self.net {
            if let Some(facts) = session.through_facts() {
                report.facts = facts;
            }
            report.connected = report.connected.or(session.connected_after());
            if let Some(lobby) = session.client.lobby() {
                report.tally.present(lobby.players.len());
                if !report.said {
                    report.said = true;
                    report.tally.set_path(session.client.path());
                }
            }
            return;
        }
        // The session is over: a report only for one that reached its lobby,
        // and only while statistics are on.
        if let Some(report) = self.internet.report.take()
            && report.tally.joined()
            && let Some(install_id) = report.install_id
        {
            let version = crate::version::version();
            telemetry::send(&report.master, report.report(install_id, version));
        }
    }

    /// Carries out what the screen asked for.
    pub(crate) fn internet_outcome(&mut self, outcome: Outcome) -> Action {
        match outcome {
            Outcome::None => Action::None,
            Outcome::Close => Action::InternetClose,
            Outcome::EndSession => Action::DirectLeave,
            Outcome::Join(request) => {
                self.internet_join(*request);
                Action::Click
            }
            Outcome::Host(request) => {
                self.internet_host(*request);
                Action::Click
            }
        }
    }

    fn internet_join(&mut self, request: InternetJoin) {
        let Some(through) = self
            .internet
            .screen
            .as_mut()
            .and_then(|screen| screen.take_through())
        else {
            self.message("Cannot join: the Internet Lobby's introduction was lost.");
            return;
        };
        let join = session_join(&request, through);
        let label = request.join.label.clone();
        // `start_join` says "Joining ..." and any failure in Messages.
        if self.start_join(join, &label) {
            self.open_lobby(&label, false);
            self.internet.report = Some(Reporting {
                master: request.master,
                install_id: request.install_id,
                tally: PlayerTally::begin(request.join.address, request.asked),
                said: false,
                asked: request.asked,
                connected: None,
                facts: ThroughFacts::default(),
            });
        }
    }

    /// New: hosts the Quick Mission creator's current mission, airborne,
    /// listed on the Internet Lobby, and opens the lobby as King (EF8).
    fn internet_host(&mut self, request: HostRequest) {
        let HostRequest {
            callsign,
            name,
            port,
            password,
            listing,
        } = request;
        let label = name.clone();
        let spec = match self.quick.lobby_spec() {
            Ok(spec) => spec,
            Err(problem) => {
                self.message(format!("Cannot host this mission: {problem}"));
                return;
            }
        };
        let options = HostOptions {
            mission: PathBuf::from("the Quick Mission creator"),
            spec,
            port,
            name,
            open_planes: tore_session::OpenPlanes::Friendly,
            callsign,
            slot: None,
            password,
            listing: Some(listing),
        };
        match self.begin_hosting(options, true) {
            // The lobby opens; its King presses Fly when everyone is ready.
            Ok(()) => {
                if self.net.is_some() {
                    self.open_lobby(&label, true);
                }
            }
            Err(error) => {
                // The command line's hint is about --port; the screen has
                // Options.
                let error = error.replace(
                    "Choose another with --port.",
                    "Choose another port in Options.",
                );
                self.message(error);
            }
        }
    }

    /// A key press for the open screen, with the text the key typed.
    pub(crate) fn internet_key(&mut self, name: &str, text: Option<&str>) -> Action {
        // The lobby over this screen takes the keys first.
        if self.lobby.screen.is_some() {
            return self.direct_key(name, text);
        }
        let shift = self.modifiers.shift_key();
        let typed = text
            .filter(|_| !self.modifiers.control_key() && !self.modifiers.alt_key())
            .filter(|text| text.chars().any(|c| !c.is_control()));
        let Some(screen) = &mut self.internet.screen else {
            return Action::None;
        };
        if let Some(text) = typed
            && screen.typing()
        {
            screen.text_input(text);
            return Action::None;
        }
        let outcome = screen.key(name, shift);
        self.internet_outcome(outcome)
    }

    /// The left mouse button on the open screen.
    pub(crate) fn internet_button(&mut self, pressed: bool) -> Action {
        if self.lobby.screen.is_some() {
            return self.direct_button(pressed);
        }
        let Some(screen) = &mut self.internet.screen else {
            return Action::None;
        };
        let outcome = screen.button(pressed);
        self.internet_outcome(outcome)
    }

    /// A wheel step over the open screen.
    pub(crate) fn internet_wheel(&mut self, notches: i32) {
        if self.lobby.screen.is_some() {
            self.direct_wheel(notches);
        } else if let Some(screen) = &mut self.internet.screen {
            screen.wheel(notches);
        }
    }

    /// Lets go of anything held on the screen (resize, lost focus).
    pub(crate) fn internet_cancel_press(&mut self) {
        if let Some(screen) = &mut self.internet.screen {
            screen.cancel_press();
        }
    }

    /// Leaves the screen.
    pub(crate) fn close_internet(&mut self) {
        // Dropping the screen stops its browse and any introduction.
        self.internet.screen = None;
        self.menu.state.cancel();
    }
}

/// The pointer moved over the screen that is up (canvas pixels), or left
/// the canvas: the lobby over Direct Connection or this screen takes it
/// first.
pub(crate) fn pointer_moved(
    lobby: &mut crate::lobby_screen::app::Lobby,
    internet: &mut Internet,
    point: Option<(f64, f64)>,
) {
    if let Some(screen) = &mut lobby.screen {
        screen.moved(point);
    } else if let Some(screen) = &mut internet.screen {
        screen.moved(point);
    }
}

/// Draws the screen when it is up; false when it is not.
pub(crate) fn draw_screen(internet: &Internet, canvas: &mut crate::menu::Canvas) -> bool {
    match &internet.screen {
        Some(screen) => {
            screen.draw(canvas);
            true
        }
        None => false,
    }
}
