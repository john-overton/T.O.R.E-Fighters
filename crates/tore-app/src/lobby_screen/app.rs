//! The game's side of the lobby screen: opening it when a join or New starts
//! a session, feeding it the client session's lobby, carrying out its
//! requests, the Quick Mission creator in Accept mode and Load Ordnance for
//! the player's own slot, and closing it. `main.rs` only calls these; the
//! screen itself is [`super::LobbyScreen`].
//!
//! # The two pages
//!
//! Both are the single-player pages, opened over the lobby on the game's
//! `Screen::Quick` with a flag that changes only their labels and what they
//! allow (the flag is off in single player, so those draw and behave as
//! before):
//!
//! - **Mission...** (the King) opens the creator with `OK` reading **Accept**
//!   and Start locked to Airborne. Accept checks the draft against what a
//!   host takes ([`crate::quick_mission::QuickMission::lobby_spec`]), sends it
//!   with `change_mission` and waits: the next lobby state with a new mission
//!   number closes the page; the host's refusal (the build's own words) stays
//!   in the creator's notice. Cancel (or Esc) puts the draft back as it was
//!   and sends nothing.
//! - **Loadout** opens Load Ordnance for the aircraft of the slot the player
//!   holds, with **Accept** and **Cancel**, the mission's Guns only already
//!   applied, and Cheat loading refused on the page. Accept checks the
//!   loadout with the host's own rule
//!   ([`tore_world::mission::LoadoutSpec::check_for_plane`]) so a refusal
//!   shows on the page before anything is sent, then sends it. A host's
//!   refusal (the mission changed meanwhile) shows on the page too. The page
//!   closes when the host has taken it (the lobby shows the player armed) or,
//!   when nothing answers within a second, assuming it was taken.
use super::{LobbyScreen, Request};
use crate::aircraft_type::AircraftType;
use crate::menu::Action;
use crate::ordnance::Ordnance;
use crate::quick_mission::Saved;
use crate::{App, Screen};
use std::time::{Duration, Instant};
use tore_session::wire::chat::Receiver;
use tore_session::wire::messages::{LobbyPhase, kind};
use tore_sim::combat::loadout::Loadout;
use tore_world::mission::LoadoutSpec;

/// How long Accept waits on Load Ordnance for a refusal before it takes the
/// loadout as accepted (the host answers only a refusal).
const ASSUME_TAKEN: Duration = Duration::from_millis(1000);
/// The same wait when the lobby already showed the player armed (the
/// lobby's state will not change to say it was taken).
const TAKEN_AGAIN: Duration = Duration::from_millis(300);

/// The page open over the lobby.
enum Page {
    /// The creator. `sent` is the mission number the lobby had when Accept
    /// sent the mission; `saved` puts the draft back on Cancel.
    Creator {
        saved: Box<Saved>,
        sent: Option<u32>,
    },
    /// Load Ordnance for `plane` of mission `mission`. `shelved` is single
    /// player's own page, put back when this one closes.
    Loadout {
        plane: u32,
        mission: u32,
        shelved: Option<Box<Ordnance>>,
        sent: Option<Instant>,
    },
}

/// What the game keeps for the lobby.
#[derive(Default)]
pub struct Lobby {
    pub screen: Option<LobbyScreen>,
    page: Option<Page>,
    /// The loadout last chosen: its mission number, plane and page, so
    /// Loadout opens on it again.
    kept: Option<(u32, u32, Ordnance)>,
}

impl Lobby {}

impl App {
    /// Opens the lobby screen over the session a join or New just started.
    /// `label` names the game until its lobby state arrives.
    pub(crate) fn open_lobby(&mut self, label: &str, hosting: bool) {
        let Some(kit) = self.direct.kit() else {
            return;
        };
        self.lobby = Lobby {
            screen: Some(LobbyScreen::new(kit, label, hosting)),
            ..Lobby::default()
        };
        self.menu.state.cancel();
        self.mouse_look = None;
        if let Some(renderer) = &self.renderer {
            renderer.window.request_redraw();
        }
    }

    /// Closes the lobby (the session ended): any page goes, and single
    /// player's own page and creator come back as they were.
    pub(crate) fn close_lobby(&mut self) {
        self.close_page(false);
        self.lobby = Lobby::default();
    }

    /// A flight starts: a page open over the lobby is put away.
    pub(crate) fn close_lobby_page(&mut self) {
        self.close_page(false);
    }

    // ---- the turn ----

    /// The lobby's turn each frame, before a frame is drawn.
    pub(crate) fn lobby_tick(&mut self) {
        if self.lobby.screen.is_none() {
            return;
        }
        let Some(session) = &mut self.net else {
            self.close_lobby();
            return;
        };
        // What the router did about the game port (slice J4b), in Messages.
        let notes = session
            .hosting
            .as_mut()
            .map(|hosting| hosting.take_notes())
            .unwrap_or_default();
        let client = &session.client;
        if let Some(screen) = &mut self.lobby.screen {
            for note in &notes {
                screen.say(note);
            }
            screen.update(client.lobby(), client.unable());
        }
        // The pages are the only reason to look further.
        if self.lobby.page.is_none() {
            return;
        }
        let Some(lobby) = client.lobby().cloned() else {
            return;
        };
        match &mut self.lobby.page {
            Some(Page::Creator {
                sent: Some(number), ..
            }) if lobby.mission != *number => {
                // The host took the mission: back to the lobby.
                self.close_page(true);
            }
            Some(Page::Loadout {
                plane,
                mission,
                sent,
                ..
            }) => {
                let holds = lobby.me().and_then(|m| m.slot);
                let armed = lobby.me().is_some_and(|m| m.loadout);
                let (plane, mission) = (*plane, *mission);
                let closed = self.quick.ordnance.as_ref().is_none_or(|o| !o.visible);
                if lobby.phase != LobbyPhase::Lobby
                    || holds != Some(plane)
                    || lobby.mission != mission
                {
                    self.close_page(false);
                    if let Some(screen) = &mut self.lobby.screen {
                        screen.say("The mission or your slot changed; choose your loadout again.");
                    }
                } else if closed {
                    self.close_page(false);
                } else if sent.is_some_and(|at| {
                    at.elapsed() >= if armed { TAKEN_AGAIN } else { ASSUME_TAKEN }
                }) {
                    self.close_page(true);
                    if let Some(screen) = &mut self.lobby.screen {
                        screen.say("Loadout sent.");
                    }
                }
            }
            _ => {}
        }
    }

    // ---- events from the session ----

    /// The host refused a lobby request: on the page that asked, else in
    /// Messages.
    pub(crate) fn lobby_refused(&mut self, request: u8, reason: &str) {
        match (&mut self.lobby.page, request) {
            (Some(Page::Creator { sent, .. }), kind::CHANGE_MISSION) => {
                *sent = None;
                self.quick.notice = Some(reason.to_owned());
                return;
            }
            (Some(Page::Loadout { sent, .. }), kind::LOADOUT) => {
                *sent = None;
                if let Some(o) = self.quick.ordnance.as_mut() {
                    o.message = Some(reason.to_owned());
                }
                return;
            }
            _ => {}
        }
        if let Some(screen) = &mut self.lobby.screen {
            screen.say(reason);
        }
    }

    // ---- requests ----

    /// Carries out what the screen asked for.
    pub(crate) fn lobby_outcome(&mut self, request: Option<Request>) -> Action {
        let Some(request) = request else {
            return Action::None;
        };
        let Some(session) = &mut self.net else {
            return Action::None;
        };
        let client = &mut session.client;
        match request {
            Request::Take(plane) => client.take_slot(plane),
            Request::LeaveSlot => client.leave_slot(),
            Request::SetReady(ready) => client.set_ready(ready),
            Request::Start => client.start_mission(),
            Request::EndMission => client.end_mission(),
            Request::Kick { player, reason } => {
                let name = client
                    .lobby()
                    .and_then(|l| l.player(player))
                    .map(|p| p.callsign.clone());
                client.kick(player, &reason);
                if let (Some(name), Some(screen)) = (name, &mut self.lobby.screen) {
                    screen.say(&format!("Kicked {name}."));
                }
            }
            Request::Chat(text) => {
                if let Err(refusal) = client.chat(Receiver::All, &text)
                    && let Some(screen) = &mut self.lobby.screen
                {
                    screen.say(refusal.text());
                }
            }
            Request::Mission => {
                self.open_creator();
            }
            Request::Loadout => {
                self.open_loadout();
            }
            Request::Leave => return Action::DirectLeave,
        }
        Action::Click
    }

    // ---- the pages ----

    fn say(&mut self, text: &str) {
        if let Some(screen) = &mut self.lobby.screen {
            screen.say(text);
        }
    }

    fn open_creator(&mut self) {
        if self.lobby.page.is_some() {
            return;
        }
        self.lobby.page = Some(Page::Creator {
            saved: Box::new(self.quick.save()),
            sent: None,
        });
        self.quick.enter_lobby();
        self.screen = Screen::Quick;
        self.menu.state.cancel();
    }

    fn open_loadout(&mut self) {
        if self.lobby.page.is_some() {
            return;
        }
        let Some(lobby) = self.net.as_ref().and_then(|s| s.client.lobby()).cloned() else {
            return;
        };
        let Some(plane) = lobby.me().and_then(|m| m.slot) else {
            self.say("Take a slot first.");
            return;
        };
        let Some(slot) = lobby.slots.iter().find(|s| s.plane == plane) else {
            return;
        };
        let aircraft = slot.aircraft;
        let guns_only = self
            .net
            .as_ref()
            .and_then(|s| s.client.spec())
            .is_some_and(|spec| spec.guns_only);
        let kept = self.lobby.kept.take().filter(|(mission, p, o)| {
            *mission == lobby.mission && *p == plane && o.loadout.aircraft == aircraft
        });
        let mut page = match kept {
            Some((_, _, page)) => page,
            None => {
                let built = (|| -> crate::AppResult<Ordnance> {
                    let kind = AircraftType::load(&*self.theater_resources, aircraft)?;
                    let load = Loadout::new(&kind.profile, |name| {
                        self.theater_resources.get(name).cloned().ok_or_else(|| {
                            std::io::Error::other(format!("missing loadout resource {name}"))
                        })
                    })?;
                    Ordnance::new(load, &self.theater_resources)
                })();
                match built {
                    Ok(page) => page,
                    Err(error) => {
                        self.say(&format!("Cannot open Load Ordnance: {error}"));
                        return;
                    }
                }
            }
        };
        page.lobby = true;
        page.visible = true;
        if guns_only {
            page.loadout.restrict_to_guns();
        }
        page.message = guns_only.then(|| {
            "Guns only: this mission allows the gun alone. Accept sends your loadout to the lobby."
                .to_owned()
        });
        let shelved = self.quick.ordnance.replace(page).map(Box::new);
        self.lobby.page = Some(Page::Loadout {
            plane,
            mission: lobby.mission,
            shelved,
            sent: None,
        });
        self.screen = Screen::Quick;
        self.menu.state.cancel();
    }

    /// Closes the open page and returns to the lobby. `keep` says the
    /// creator's draft stays (Accept took it); otherwise it is put back as
    /// it was. The loadout page is kept for the next visit either way.
    fn close_page(&mut self, keep: bool) {
        let Some(page) = self.lobby.page.take() else {
            return;
        };
        match page {
            Page::Creator { saved, .. } => {
                if !keep {
                    self.quick.restore(*saved);
                }
                self.quick.leave_lobby();
            }
            Page::Loadout {
                plane,
                mission,
                shelved,
                ..
            } => {
                if let Some(mut page) =
                    std::mem::replace(&mut self.quick.ordnance, shelved.map(|page| *page))
                {
                    page.visible = false;
                    page.message = None;
                    self.lobby.kept = Some((mission, plane, page));
                }
            }
        }
        if self.screen == Screen::Quick {
            self.screen = Screen::Main;
        }
        if let Some(renderer) = &self.renderer {
            renderer.window.request_redraw();
        }
    }

    /// An action on the creator or Load Ordnance while a lobby page is open:
    /// Accept and Cancel are the lobby's. Returns the action the rest of
    /// `main.rs` goes on with.
    pub(crate) fn lobby_page_action(&mut self, action: Action) -> Action {
        match (&self.lobby.page, &action) {
            (Some(Page::Creator { .. }), Action::Mission) => {
                self.accept_mission();
                Action::Click
            }
            (Some(Page::Creator { .. }), Action::Back) => {
                self.close_page(false);
                Action::Click
            }
            (Some(Page::Loadout { .. }), Action::MissionFly) => {
                self.accept_loadout();
                Action::Click
            }
            _ => action,
        }
    }

    /// Accept in the creator: the draft, checked against what a host takes,
    /// goes to the King's client.
    fn accept_mission(&mut self) {
        let spec = match self.quick.lobby_spec() {
            Ok(spec) => spec,
            Err(problem) => {
                self.quick.notice = Some(problem);
                return;
            }
        };
        let Some(session) = &mut self.net else {
            return;
        };
        let number = session.client.lobby().map(|l| l.mission);
        session.client.change_mission(&spec);
        if let Some(Page::Creator { sent, .. }) = &mut self.lobby.page {
            *sent = number;
        }
        self.quick.notice = Some("Sending the mission to the lobby...".into());
    }

    /// Accept on Load Ordnance: the loadout checked with the host's own rule
    /// (its words go on the page), then sent.
    fn accept_loadout(&mut self) {
        let Some(Page::Loadout { plane, .. }) = &self.lobby.page else {
            return;
        };
        let plane = *plane;
        let Some(page) = self.quick.ordnance.as_mut() else {
            return;
        };
        let spec = LoadoutSpec::of(&page.loadout);
        let aircraft = page.loadout.aircraft;
        let Some(session) = &mut self.net else {
            return;
        };
        let guns_only = session.client.spec().is_some_and(|s| s.guns_only);
        let checked = AircraftType::load(&*self.theater_resources, aircraft)
            .map_err(|e| e.to_string())
            .and_then(|kind| {
                spec.check_for_plane(&kind.profile, &*self.theater_resources, guns_only)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            });
        if let Err(reason) = checked {
            page.message = Some(reason);
            return;
        }
        session.client.send_loadout(Some(spec));
        page.message = Some("Sending your loadout...".into());
        if let Some(Page::Loadout { sent, .. }) = &mut self.lobby.page {
            *sent = Some(Instant::now());
        }
        let _ = plane;
    }
}
