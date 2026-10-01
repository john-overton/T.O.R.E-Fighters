//! The game's side of the Direct Connection screen: opening it, building its
//! kit once, routing the window's events to it and carrying out what it asks
//! for (join, host, leave). `main.rs` only calls these; the screen itself is
//! [`super::DirectScreen`].
use super::{DirectScreen, HostRequest, JoinRequest, Outcome};
use crate::menu::Action;
use crate::net::{options::HostOptions, session::Join};
use crate::{App, Screen};
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use winit::event_loop::ActiveEventLoop;

use crate::widgets::Kit;

/// What the game keeps for the screen (*agent decision*, EF7): the screen
/// while it is open, the kit it is made of once it has been built (kept for
/// the life of the game, so a second visit opens at once), and the thread
/// building it the first time. While the kit builds (about 150 ms in a
/// release build) the player sees Choose Activity with "Opening Direct
/// Connection..." on its message line, and nothing blocks.
#[derive(Default)]
pub struct Direct {
    pub screen: Option<DirectScreen>,
    kit: Option<Arc<Kit>>,
    building: Option<mpsc::Receiver<Result<Kit, String>>>,
}

impl Direct {
    /// The kit is being built.
    pub fn is_building(&self) -> bool {
        self.building.is_some()
    }
}

impl App {
    /// The screen is open and showing: over Choose Activity.
    pub(crate) fn direct_open(&self) -> bool {
        self.screen == Screen::Main && self.direct.screen.is_some()
    }

    /// The Multi menu's Direct Connection row.
    pub(crate) fn open_direct(&mut self) {
        if self.direct.screen.is_some() || self.direct.building.is_some() {
            return;
        }
        self.menu.state.cancel();
        if self.direct.kit.is_some() {
            self.make_direct_screen();
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
                self.direct.building = Some(receive);
                self.menu.state.toast = Some((
                    "Opening Direct Connection...".to_owned(),
                    std::time::Instant::now() + std::time::Duration::from_secs(30),
                ));
            }
            Err(error) => self.message(format!("Cannot open Direct Connection: {error}")),
        }
    }

    fn make_direct_screen(&mut self) {
        let Some(kit) = self.direct.kit.clone() else {
            return;
        };
        let data: Option<PathBuf> = crate::assets::data_directory().ok();
        self.direct.screen = Some(DirectScreen::new(
            kit,
            self.menu.kit_source.quick_messages(),
            data,
        ));
        self.menu.state.cancel();
        self.mouse_look = None;
        if let Some(renderer) = &self.renderer {
            renderer.window.request_redraw();
        }
    }

    /// The game's turn for the screen, before a frame is drawn: the kit's
    /// build taken when it is done, the screen's own turn, and what it asks
    /// for.
    pub(crate) fn direct_tick(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(receive) = &self.direct.building {
            match receive.try_recv() {
                Ok(Ok(kit)) => {
                    self.direct.building = None;
                    self.direct.kit = Some(Arc::new(kit));
                    self.make_direct_screen();
                }
                Ok(Err(error)) => {
                    self.direct.building = None;
                    self.menu.state.toast = None;
                    self.message(format!("Cannot open Direct Connection: {error}"));
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.direct.building = None;
                    self.message("Cannot open Direct Connection: the pieces did not build.");
                }
            }
        }
        let session = self.net.is_some();
        // Flying, or under a debrief with no session, the screen has no turn.
        if self.screen == Screen::Flight || (self.screen != Screen::Main && !session) {
            return;
        }
        let Some(screen) = &mut self.direct.screen else {
            return;
        };
        let outcome = screen.update(session);
        let action = self.direct_outcome(outcome);
        if action != Action::None {
            self.action(event_loop, action);
        }
    }

    /// Carries out what the screen asked for.
    pub(crate) fn direct_outcome(&mut self, outcome: Outcome) -> Action {
        match outcome {
            Outcome::None => Action::None,
            Outcome::Close => Action::DirectClose,
            Outcome::EndSession => Action::DirectLeave,
            Outcome::Join(request) => {
                self.direct_join(request);
                Action::Click
            }
            Outcome::Host(request) => {
                self.direct_host(request);
                Action::Click
            }
        }
    }

    fn direct_join(&mut self, request: JoinRequest) {
        let JoinRequest {
            address,
            label,
            callsign,
            password,
        } = request;
        match Join::to(address, &callsign, &password, &label) {
            Ok(join) => {
                // `start_join` says "Joining ..." and any failure in Messages.
                self.start_join(join, &label);
            }
            Err(error) => self.message(error),
        }
    }

    /// New: hosts the Quick Mission creator's current mission (until the
    /// lobby screen of EF8 builds one), as `--host` does.
    fn direct_host(&mut self, request: HostRequest) {
        let HostRequest {
            callsign,
            name,
            port,
            password,
        } = request;
        let spec = match self.quick.unsupported() {
            Some(problem) => Err(problem),
            None => self.quick.mission_spec(),
        };
        let spec = match spec {
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
        };
        match self.begin_hosting(options) {
            // The first mission starts by itself; after it the game waits in
            // the lobby, so the player can come back here and leave.
            Ok(()) => {
                if let Some(session) = &mut self.net {
                    session.auto_restart = false;
                }
                self.message(
                    "The mission starts once everyone holding a plane is ready, and the game waits here after it. Leave closes the game.",
                );
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
    pub(crate) fn direct_key(&mut self, name: &str, text: Option<&str>) -> Action {
        let shift = self.modifiers.shift_key();
        let typed = text
            .filter(|_| !self.modifiers.control_key() && !self.modifiers.alt_key())
            .filter(|text| text.chars().any(|c| !c.is_control()));
        let Some(screen) = &mut self.direct.screen else {
            return Action::None;
        };
        if let Some(text) = typed
            && screen.typing()
        {
            screen.text_input(text);
            return Action::None;
        }
        let outcome = screen.key(name, shift);
        self.direct_outcome(outcome)
    }

    /// The left mouse button on the open screen.
    pub(crate) fn direct_button(&mut self, pressed: bool) -> Action {
        let Some(screen) = &mut self.direct.screen else {
            return Action::None;
        };
        let outcome = screen.button(pressed);
        self.direct_outcome(outcome)
    }

    /// Leaves the screen.
    pub(crate) fn close_direct(&mut self) {
        // Dropping the screen stops its search and any lookup.
        self.direct.screen = None;
        self.menu.state.cancel();
    }

    /// Cancel on the screen with a session running: leaves the game.
    pub(crate) fn leave_direct_session(&mut self, event_loop: &ActiveEventLoop) {
        if self.net.is_some() {
            self.net_ending = Some("You left the game.".into());
            self.end_session(event_loop);
        }
    }
}
