//! The game's side of the Direct Connection screen: opening it, building its
//! kit once, routing the window's events to it and carrying out what it asks
//! for (join, host, leave). `main.rs` only calls these; the screen itself is
//! [`super::DirectScreen`]. A join or New that starts a session opens the
//! lobby screen ([`crate::lobby_screen`]) over it, and these calls hand the
//! window's events to whichever is up.
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
    /// The kit the screens are made of, once it has been built.
    pub fn kit(&self) -> Option<Arc<Kit>> {
        self.kit.clone()
    }
    /// The kit is being built.
    pub fn is_building(&self) -> bool {
        self.building.is_some()
    }
    /// Keeps a kit another screen built (the Internet Lobby, I4), when none
    /// is kept: the lobby screen opens over either and takes it from here.
    pub fn keep_kit(&mut self, kit: Arc<Kit>) {
        if self.kit.is_none() {
            self.kit = Some(kit);
        }
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
        // Stage L: this game's content is worked out on a worker, once, so a
        // join or a host does not wait for it.
        crate::net::session::start_content(&self.theater_resources);
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
        self.lobby_tick();
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
                // A join the screen starts opens the lobby, which shows the
                // joining and a refusal ends it back here.
                let mut join = join;
                join.lobby = true;
                if self.start_join(join, &label) {
                    self.open_lobby(&label, false);
                }
            }
            Err(error) => self.message(error),
        }
    }

    /// New: hosts the Quick Mission creator's current mission, airborne, and
    /// opens the lobby as King (EF8).
    fn direct_host(&mut self, request: HostRequest) {
        let HostRequest {
            callsign,
            name,
            port,
            password,
        } = request;
        let label = name.clone();
        let spec = self.quick.lobby_spec();
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
            listing: None,
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
    pub(crate) fn direct_key(&mut self, name: &str, text: Option<&str>) -> Action {
        let shift = self.modifiers.shift_key();
        let typed = text
            .filter(|_| !self.modifiers.control_key() && !self.modifiers.alt_key())
            .filter(|text| text.chars().any(|c| !c.is_control()));
        if let Some(screen) = &mut self.lobby.screen {
            if let Some(text) = typed
                && screen.typing()
            {
                screen.text_input(text);
                return Action::None;
            }
            let request = screen.key(name, shift);
            return self.lobby_outcome(request);
        }
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
        if let Some(screen) = &mut self.lobby.screen {
            let request = screen.button(pressed);
            return self.lobby_outcome(request);
        }
        let Some(screen) = &mut self.direct.screen else {
            return Action::None;
        };
        let outcome = screen.button(pressed);
        self.direct_outcome(outcome)
    }

    /// A wheel step over the open screen.
    pub(crate) fn direct_wheel(&mut self, notches: i32) {
        if let Some(screen) = &mut self.lobby.screen {
            screen.wheel(notches);
        } else if let Some(screen) = &mut self.direct.screen {
            screen.wheel(notches);
        }
    }

    /// Lets go of anything held on the screens (resize, lost focus).
    pub(crate) fn direct_cancel_press(&mut self) {
        if let Some(screen) = &mut self.lobby.screen {
            screen.cancel_press();
        }
        if let Some(screen) = &mut self.direct.screen {
            screen.cancel_press();
        }
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

/// The pointer moved over the screen that is up (canvas pixels), or left
/// the canvas. A function of the two fields, so the window event handler can
/// call it while it holds the renderer.
pub(crate) fn pointer_moved(
    lobby: &mut crate::lobby_screen::app::Lobby,
    direct: &mut Direct,
    point: Option<(f64, f64)>,
) {
    if let Some(screen) = &mut lobby.screen {
        screen.moved(point);
    } else if let Some(screen) = &mut direct.screen {
        screen.moved(point);
    }
}

/// Draws the screen that is up (the lobby over Direct Connection); false
/// when neither is.
pub(crate) fn draw_screen(
    lobby: &crate::lobby_screen::app::Lobby,
    direct: &Direct,
    canvas: &mut crate::menu::Canvas,
) -> bool {
    if let Some(screen) = &lobby.screen {
        screen.draw(canvas);
        true
    } else if let Some(screen) = &direct.screen {
        screen.draw(canvas);
        true
    } else {
        false
    }
}
