//! The lobby screen (slice EF8): where the players of a game gather before a
//! mission, and where everyone returns after it. Built of the widget kit
//! ([`crate::widgets`]) on the Direct Connection screen's background and panel
//! (the look John approved on 2026-10-01); the design and the choices below
//! are in `docs/ARCHITECTURE.md`, "Lobby and hosting".
//!
//! The screen is plain state, as Direct Connection is: the game gives it the
//! lobby's state each frame ([`LobbyScreen::update`]) and its events (keys,
//! text, the pointer, the wheel), and it answers with a [`Request`] for the
//! game to send through the client session. It owns no session, so every
//! rule on it is tested without a window. What each button does, and who may
//! press it, is in [`facts`].
//!
//! # What is on it
//!
//! - **The head**: the game's name, the mission's summary and the start rule
//!   in plain words.
//! - **Slots**: every aircraft a player may take (wing, member, aircraft,
//!   who holds it, or AI). Click a free slot to take it; click one's own again
//!   to free it. A slot another player holds is dimmed and cannot be clicked.
//! - **Players**: callsigns, the King's crown, the house of the player whose
//!   machine runs the game, a tick when ready, a red mark and the reason
//!   when a player's game cannot play the mission, the platform's mark and,
//!   for a player reaching the host through the relay, the relay's mark; the
//!   selected player's line says how it connected (slice J6).
//! - **Messages and the chat line** (EF6's [`LobbyChat`]): the game's words
//!   and chat; Enter in the line sends to All.
//! - **Buttons**: Mission..., Players... and Fly (the King; Fly reads End
//!   Mission while the mission flies), Settings... (everyone; greyed rows
//!   unless the King's), Loadout (Watch while the mission flies), Ready (Join
//!   while the mission flies) and Leave. A dedicated server's lobby has no
//!   King, so it shows only Settings..., Loadout, Ready and Leave. A button
//!   that cannot be pressed says why when it is clicked.
//! - **Panels**: Settings... (four pages of the King's settings, see
//!   [`settings_panel`]), Players... (the King's Give crown and Kick for the
//!   player selected in Players, see [`players_panel`]), Kick (with the
//!   reason the player sees) and the King's Leave ("Leaving ends the game
//!   for everyone. Leave?").
//! - **Slot locks**: the King's right click on a slot closes it to the AI,
//!   opens it again, or, with a player selected in Players, keeps it for that
//!   player.
//!
//! # Keys
//!
//! Tab and Shift+Tab walk Slots, Players, Messages, the chat line and the
//! buttons; Enter on a slot clicks it, in the chat line sends (or, with the
//! line empty, presses the blue button: Fly when it can be pressed, else
//! Ready); Esc is Leave. Every control also works by the mouse.
pub mod app;
pub mod facts;
#[cfg(test)]
mod k7b_tests;
mod modal;
#[cfg(test)]
mod phase2_tests;
pub mod players_panel;
pub mod preview;
pub mod settings_panel;
#[cfg(test)]
mod tests;

use crate::menu::{Canvas, text_width};
use crate::net::lobby_chat::LobbyChat;
use crate::ui_text;
use crate::widgets::{
    Align, Backdrop, Background, Button, Column, Focus, Kit, List, Outcome as Wo, Pager, Point,
    Route, Widget, draw_panel, fit, inside,
};
use facts::{Buttons, DefaultButton, Facts, FlyAs, LoadoutAs, SlotClick};
use modal::{Answer, Modal, Purpose};
use players_panel::PlayersPanel;
use settings_panel::{Context as SettingsContext, Edit, SettingsPanel};
use std::sync::Arc;
use std::time::Instant;
use tore_session::wire::chat::ChatLine;
use tore_session::wire::messages::{ContentGaps, LobbyState, Lock, SettingsChange};
use tore_sim::cheats::Cheats;

/// The screen's own frame lines: the colour measured on John's screenshot.
const LINE: [u8; 4] = [174, 174, 174, 255];

/// The controls Tab visits, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Id {
    Slots,
    Players,
    Messages,
    Line,
    Mission,
    /// The Settings... button.
    Settings,
    /// The Players... button (the list is [`Id::Players`]).
    PlayersPanel,
    Loadout,
    Ready,
    Fly,
    Leave,
}

impl Id {
    /// How many controls Tab visits.
    const COUNT: usize = 11;
    /// The buttons, in the order of their places for the King.
    const BUTTONS: [Id; 7] = [
        Id::Mission,
        Id::Settings,
        Id::PlayersPanel,
        Id::Loadout,
        Id::Ready,
        Id::Fly,
        Id::Leave,
    ];
}

/// What the player asked the game to do. The game sends it through the
/// client session (or opens a page, or ends the session).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Take this plane's slot.
    Take(u32),
    /// Free the slot held.
    LeaveSlot,
    /// Ready, or not ready.
    SetReady(bool),
    /// The King's Fly.
    Start,
    /// The King ends the flying mission for everyone.
    EndMission,
    Kick {
        player: u8,
        reason: String,
    },
    /// The King changes settings, the name or the password (the Settings
    /// panel); the host checks it whole.
    Settings(SettingsChange),
    /// The King sends the lobby mission again with these cheats (the
    /// Settings panel's Realism page).
    Cheats(Cheats),
    /// The King gives the crown to this lobby id.
    PassCrown(u8),
    /// The King opens, closes or reserves a slot (a right click on it).
    Lock {
        plane: u32,
        lock: Lock,
    },
    /// Watch the flying mission with no plane (slice F2-O1's stream; the
    /// observer screen is slice F2-O2's).
    Watch,
    /// Stop watching.
    StopWatch,
    /// Open the Quick Mission creator in Accept mode (the King).
    Mission,
    /// Open Load Ordnance for the player's own slot.
    Loadout,
    /// A chat line to All.
    Chat(String),
    /// The King frees the aircraft kept for a player, this plane (stage K,
    /// message 53).
    Release(u32),
    /// Leave the game: back to Direct Connection. The King has been asked.
    Leave,
}

const BUTTON_Y: i32 = 419;
const BUTTON_W: i32 = 75;
/// The seven places of the button row: 75 wide on a 79 pitch across the
/// 549 wide row.
const SLOT_X: [i32; 7] = [45, 124, 203, 282, 361, 440, 519];

/// What Players... is for (stage K).
enum PanelTarget {
    Player((u8, String)),
    Slot((u32, String)),
}

/// The slots list's pager: its rocker, the PREV and NEXT labels and the page
/// box, to the right of the list (the Players box starts at 399).
const SLOTS_PAGER: Pager = Pager {
    rocker: (364, 183),
    prev: (336, 184),
    next: (336, 207),
    page_label: (336, 226),
    counter_box: (336, 240),
};

/// The screen.
pub struct LobbyScreen {
    kit: Arc<Kit>,
    /// What the game is called until its lobby state arrives.
    label: String,
    /// This player's game runs the host: leaving ends the game.
    hosting: bool,
    slots: List,
    players: List,
    /// Messages and the chat line (slice EF6).
    pub chat: LobbyChat,
    mission: Button,
    settings: Button,
    players_button: Button,
    loadout: Button,
    ready: Button,
    fly: Button,
    leave: Button,
    focus: Focus<Id>,
    /// Which list the player last picked from, Slots or Players: what
    /// Players... acts on when both have a selection (stage K).
    picked: Id,
    modal: Option<Modal>,
    /// The Settings... panel, while open.
    settings_panel: Option<SettingsPanel>,
    /// The Players... panel, while open.
    players_panel: Option<PlayersPanel>,
    /// The lobby mission's cheats, for the Settings panel's Realism page.
    cheats: Option<Cheats>,
    state: Option<LobbyState>,
    unable: Option<String>,
    /// The host's newest Content gaps (stage L), for the differs lines.
    gaps: Option<ContentGaps>,
    /// Which players' items Messages has said how they differ.
    notes: facts::GapNotes,
    facts: Facts,
    buttons: Buttons,
    default: Option<DefaultButton>,
    pointer: Option<Point>,
    /// A button that cannot be pressed was pressed on: its release says why.
    blocked: Option<Id>,
    backdrop: std::cell::RefCell<Option<Backdrop>>,
    /// The screen's own frame cost, logged every 300 frames when
    /// `TORE_DIRECT_TIMING` is set (the Direct Connection screen's switch).
    timing: Option<crate::direct_screen::Timing>,
}

impl LobbyScreen {
    /// A lobby screen on `kit` (in `MODEM3`'s palette). `label` names the
    /// game while the lobby's state is on its way; `hosting` says this game
    /// runs the host, so the player is the King and Leave asks first.
    pub fn new(kit: Arc<Kit>, label: &str, hosting: bool) -> Self {
        let slot_columns = vec![
            Column {
                x: 0,
                width: 12,
                align: Align::Centre,
            },
            Column {
                x: 14,
                width: 54,
                align: Align::Left,
            },
            Column {
                x: 70,
                width: 84,
                align: Align::Left,
            },
            Column {
                x: 156,
                width: 100,
                align: Align::Left,
            },
            Column {
                x: 258,
                width: 12,
                align: Align::Centre,
            },
        ];
        // The crown, the house, the ready tick, the platform and the relay
        // mark (slice J6), 12 pixels wide at a pitch of 13, then the name and
        // the state.
        let player_columns = vec![
            Column {
                x: 0,
                width: 12,
                align: Align::Centre,
            },
            Column {
                x: 13,
                width: 12,
                align: Align::Centre,
            },
            Column {
                x: 26,
                width: 12,
                align: Align::Centre,
            },
            Column {
                x: 39,
                width: 12,
                align: Align::Centre,
            },
            Column {
                x: 52,
                width: 12,
                align: Align::Centre,
            },
            Column {
                x: 66,
                width: 60,
                align: Align::Left,
            },
            Column {
                x: 128,
                width: 40,
                align: Align::Left,
            },
        ];
        let order = vec![
            Id::Slots,
            Id::Players,
            Id::Messages,
            Id::Line,
            Id::Mission,
            Id::Settings,
            Id::PlayersPanel,
            Id::Loadout,
            Id::Ready,
            Id::Fly,
            Id::Leave,
        ];
        let mut focus = Focus::new(order, None);
        // Typing starts in the chat line.
        focus.set(Id::Line);
        let button =
            |label: &str, place: usize| Button::new(label, (SLOT_X[place], BUTTON_Y), BUTTON_W);
        let facts = Facts::of(None, None);
        let buttons = facts::buttons(&facts, false);
        let mut screen = Self {
            kit,
            label: label.to_owned(),
            hosting,
            slots: List::new((45, 168), 286, 5)
                .with_pager(SLOTS_PAGER)
                .with_columns(slot_columns),
            players: List::new((404, 168), 186, 5).with_columns(player_columns),
            chat: LobbyChat::new((45, 294, 549, 78), (45, 377), 549),
            mission: button("Mission...", 0),
            settings: button("Settings...", 1),
            players_button: button("Players...", 2),
            loadout: button("Loadout", 3),
            ready: button("Ready", 4),
            fly: button("Fly", 5),
            leave: button("Leave", 6),
            focus,
            picked: Id::Players,
            modal: None,
            settings_panel: None,
            players_panel: None,
            cheats: None,
            state: None,
            unable: None,
            gaps: None,
            notes: Default::default(),
            facts,
            buttons,
            default: None,
            pointer: None,
            blocked: None,
            backdrop: Default::default(),
            timing: std::env::var_os("TORE_DIRECT_TIMING")
                .map(|_| crate::direct_screen::Timing::new("Lobby")),
        };
        screen.refresh();
        screen
    }

    // ---- what the game reads ----

    #[cfg(test)]
    pub fn facts(&self) -> &Facts {
        &self.facts
    }
    #[cfg(test)]
    pub fn buttons(&self) -> Buttons {
        self.buttons
    }
    /// A panel (Kick, or the King's Leave) is open.
    #[cfg(test)]
    pub fn modal_open(&self) -> bool {
        self.modal.is_some()
    }
    /// The Settings... panel, when open.
    #[cfg(test)]
    pub fn settings_open(&self) -> Option<&SettingsPanel> {
        self.settings_panel.as_ref()
    }
    /// The Players... panel, when open.
    #[cfg(test)]
    pub fn players_open(&self) -> Option<&PlayersPanel> {
        self.players_panel.as_ref()
    }
    /// A Settings or Players panel is open, or the Kick and Leave panels.
    fn overlaid(&self) -> bool {
        self.modal.is_some() || self.settings_panel.is_some() || self.players_panel.is_some()
    }
    /// What the screen shows to be read: the game's name.
    pub fn game_name(&self) -> &str {
        self.state.as_ref().map_or(&self.label, |s| &s.name)
    }
    /// True when typed text goes to a field.
    pub fn typing(&self) -> bool {
        if let Some(modal) = &self.modal {
            return modal.typing();
        }
        if let Some(panel) = &self.settings_panel {
            return panel.typing();
        }
        if self.players_panel.is_some() {
            return false;
        }
        self.focus.is(Id::Line)
    }

    /// The host refused a settings or mission change the panel asked for:
    /// its words go in the panel, else in Messages. True when a panel took
    /// them.
    pub fn refused_in_panel(&mut self, reason: &str) -> bool {
        match &mut self.settings_panel {
            Some(panel) => {
                panel.refused(reason);
                true
            }
            None => false,
        }
    }

    /// The lobby mission's cheats, for the Settings panel's Realism page; the
    /// game gives them each frame.
    pub fn set_cheats(&mut self, cheats: Option<Cheats>) {
        if cheats != self.cheats {
            self.cheats = cheats;
            self.sync_panel();
        }
    }

    /// The Settings panel reads the lobby's state again.
    fn sync_panel(&mut self) {
        if let (Some(panel), Some(state)) = (&mut self.settings_panel, &self.state) {
            panel.set_context(SettingsContext::of(state, self.cheats));
        }
    }
    /// The lines in Messages, oldest first.
    #[cfg(test)]
    pub fn message_lines(&self) -> Vec<String> {
        self.chat
            .messages
            .lines()
            .map(|(text, _)| text.to_owned())
            .collect()
    }
    /// The button Enter presses now.
    #[cfg(test)]
    pub fn default_button(&self) -> Option<DefaultButton> {
        self.default
    }
    #[cfg(test)]
    pub fn typed(&self) -> &str {
        self.chat.field.text()
    }
    /// The plane of the slot the selection is on, for tests.
    #[cfg(test)]
    pub fn selected_slot(&self) -> Option<u32> {
        self.selected_plane()
    }

    // ---- words ----

    /// Adds a line to Messages in the game's own colour.
    pub fn say(&mut self, text: &str) {
        log::info!("Lobby: {text}");
        self.chat.system(&self.kit, text);
    }

    /// A chat line from the session.
    pub fn chat_line(&mut self, line: &ChatLine) {
        self.chat.push(&self.kit, line);
    }

    // ---- the turn ----

    /// The screen's turn each frame: the lobby's state taken in (the changes
    /// said in Messages, the rows and buttons rebuilt only when it changed),
    /// and the widgets animated. `unable` is the client's word on whether its
    /// game plays the mission. Never blocks.
    pub fn update(&mut self, lobby: Option<&LobbyState>, unable: Option<&str>) {
        let started = self.timing.as_ref().map(|_| Instant::now());
        self.turn(lobby, unable);
        if let (Some(timing), Some(started)) = (&self.timing, started) {
            timing.update.set(timing.update.get() + started.elapsed());
        }
    }

    /// The host's Content gaps as the client last had them (stage L): what
    /// Messages says about a player's items, once for each player (see
    /// [`facts::GapNotes`]). Called each frame after [`LobbyScreen::update`].
    pub fn set_gaps(&mut self, gaps: Option<&ContentGaps>) {
        if gaps != self.gaps.as_ref() {
            self.gaps = gaps.cloned();
        }
        let lines = match &self.state {
            Some(state) => self.notes.lines(state, self.gaps.as_ref()),
            None => return,
        };
        for line in lines {
            self.say(&line);
        }
    }

    fn turn(&mut self, lobby: Option<&LobbyState>, unable: Option<&str>) {
        if lobby != self.state.as_ref() || unable != self.unable.as_deref() {
            self.take(lobby, unable);
        }
        let at = Instant::now();
        self.slots.advance(at);
        self.players.advance(at);
    }

    fn take(&mut self, lobby: Option<&LobbyState>, unable: Option<&str>) {
        if let Some(new) = lobby {
            for line in facts::change_lines(self.state.as_ref(), new) {
                self.say(&line);
            }
        }
        // A player whose own game cannot play it is told once, in its own
        // game's words (the second person); when only the host has said so,
        // in the host's words. Both read as they are (stage L).
        let host_said = |state: Option<&LobbyState>| {
            state
                .and_then(LobbyState::me)
                .and_then(|me| me.unable.clone())
        };
        if let Some(why) = unable {
            if unable != self.unable.as_deref() {
                self.say(why);
            }
        } else if let Some(why) = host_said(lobby)
            && host_said(self.state.as_ref()).as_ref() != Some(&why)
        {
            self.say(&why);
        }
        self.state = lobby.cloned();
        self.unable = unable.map(str::to_owned);
        if let Some(state) = &self.state {
            self.slots.set_rows(facts::slot_rows(state));
            self.players.set_rows(facts::player_rows(state));
        } else {
            self.slots.set_rows(Vec::new());
            self.players.set_rows(Vec::new());
        }
        self.refresh();
        self.sync_panel();
        // The Players panel is for a player who is still there, and only the
        // King's.
        if let Some(panel) = &self.players_panel
            && (!self.facts.king
                || self.state.as_ref().is_none_or(|s| {
                    if panel.slot_only() {
                        // A reserved plane stays until it is released, taken
                        // by its player or the mission ends.
                        !s.slots.iter().any(|slot| {
                            slot.holder.is_none()
                                && slot.reserved.is_some()
                                && Some(slot.plane) == panel.release_plane()
                        })
                    } else {
                        s.player(panel.player()).is_none()
                    }
                }))
        {
            self.players_panel = None;
        }
    }

    /// The facts, the button states, their labels and the blue button, from
    /// what the screen holds now.
    fn refresh(&mut self) {
        self.facts = Facts::of(self.state.as_ref(), self.unable.as_deref());
        self.buttons = facts::buttons(&self.facts, self.panel_target().is_some());
        self.default = facts::default_button(&self.facts, &self.buttons);
        let b = self.buttons;
        self.mission.set_enabled(b.mission.is_enabled());
        self.settings.set_enabled(b.settings.is_enabled());
        self.players_button.set_enabled(b.players.is_enabled());
        self.loadout.set_enabled(b.loadout.is_enabled());
        self.loadout.set_label(match b.loadout_as {
            LoadoutAs::Loadout => "Loadout",
            LoadoutAs::Watch => "Watch",
            LoadoutAs::StopWatch => "Stop Watch",
        });
        self.ready.set_enabled(b.ready.is_enabled());
        self.fly.set_enabled(b.fly.is_enabled());
        self.ready.set_label(facts::ready_label(&self.facts));
        self.fly.set_label(match b.fly_as {
            FlyAs::Fly => "Fly",
            FlyAs::EndMission => "End Mission",
        });
        self.fly
            .set_default(self.default == Some(DefaultButton::Fly));
        self.ready
            .set_default(self.default == Some(DefaultButton::Ready));
        // The buttons shown fill the row from the left for the King and from
        // the right for the others, so Leave is always in the last place.
        let places: &[(Id, usize)] = if b.mission.is_shown() {
            &[
                (Id::Mission, 0),
                (Id::Settings, 1),
                (Id::PlayersPanel, 2),
                (Id::Loadout, 3),
                (Id::Ready, 4),
                (Id::Fly, 5),
                (Id::Leave, 6),
            ]
        } else {
            &[
                (Id::Settings, 3),
                (Id::Loadout, 4),
                (Id::Ready, 5),
                (Id::Leave, 6),
            ]
        };
        for (id, place) in places {
            self.button_mut(*id).place((SLOT_X[*place], BUTTON_Y));
        }
    }

    fn button_mut(&mut self, id: Id) -> &mut Button {
        match id {
            Id::Mission => &mut self.mission,
            Id::Settings => &mut self.settings,
            Id::PlayersPanel => &mut self.players_button,
            Id::Loadout => &mut self.loadout,
            Id::Ready => &mut self.ready,
            Id::Fly => &mut self.fly,
            _ => &mut self.leave,
        }
    }

    fn button_ref(&self, id: Id) -> &Button {
        match id {
            Id::Mission => &self.mission,
            Id::Settings => &self.settings,
            Id::PlayersPanel => &self.players_button,
            Id::Loadout => &self.loadout,
            Id::Ready => &self.ready,
            Id::Fly => &self.fly,
            _ => &self.leave,
        }
    }

    /// For each control, indexed by [`Id`], whether Tab may stop on it.
    fn usable(&self) -> [bool; Id::COUNT] {
        let mut ok = [true; Id::COUNT];
        for id in Id::BUTTONS {
            ok[id as usize] = self.shown(id) && Widget::enabled(self.button_ref(id));
        }
        ok
    }

    /// The buttons drawn now, which are the ones the pointer can reach.
    fn shown_buttons(&self) -> Vec<Id> {
        Id::BUTTONS
            .into_iter()
            .filter(|id| self.shown(*id))
            .collect()
    }

    /// Which buttons are drawn, and so can be reached.
    fn shown(&self, id: Id) -> bool {
        match id {
            Id::Mission | Id::PlayersPanel | Id::Fly => self.buttons.mission.is_shown(),
            _ => true,
        }
    }

    /// The plane of the slot selected in Slots.
    fn selected_plane(&self) -> Option<u32> {
        self.slots.selected_row()?.key.parse().ok()
    }

    /// What Players... acts on (stage K): the reserved slot selected in Slots
    /// when Slots has the focus or no other player is selected, else the
    /// player selected in Players.
    fn panel_target(&self) -> Option<PanelTarget> {
        let slot = self.reserved_target();
        let player = self.player_target();
        match (player, slot) {
            (Some(_), Some(slot)) if self.picked == Id::Slots => Some(PanelTarget::Slot(slot)),
            (Some(player), _) => Some(PanelTarget::Player(player)),
            (None, Some(slot)) => Some(PanelTarget::Slot(slot)),
            (None, None) => None,
        }
    }

    /// The slot selected in Slots when the AI flies it for a player who is
    /// not in the game: its plane and the callsign it is kept for.
    fn reserved_target(&self) -> Option<(u32, String)> {
        let state = self.state.as_ref()?;
        let plane = self.selected_plane()?;
        let slot = state.slots.iter().find(|s| s.plane == plane)?;
        match (&slot.holder, &slot.reserved) {
            (None, Some(callsign)) => Some((plane, callsign.clone())),
            _ => None,
        }
    }

    /// The player selected in Players that the King may act on with
    /// Players...: anyone but themself.
    fn player_target(&self) -> Option<(u8, String)> {
        let state = self.state.as_ref()?;
        let id: u8 = self.players.selected_row()?.key.parse().ok()?;
        if id == state.you {
            return None;
        }
        Some((id, state.player(id)?.callsign.clone()))
    }

    // ---- what the buttons do ----

    fn press(&mut self, id: Id) -> Option<Request> {
        if !self.shown(id) {
            return None;
        }
        let enabled = match id {
            Id::Mission => self.buttons.mission.is_enabled(),
            Id::Settings => self.buttons.settings.is_enabled(),
            Id::PlayersPanel => self.buttons.players.is_enabled(),
            Id::Loadout => self.buttons.loadout.is_enabled(),
            Id::Ready => self.buttons.ready.is_enabled(),
            Id::Fly => self.buttons.fly.is_enabled(),
            _ => true,
        };
        if !enabled {
            if let Some(why) =
                facts::disabled_reason(&self.facts, id, self.panel_target().is_some())
            {
                self.say(&why);
            }
            return None;
        }
        match id {
            Id::Mission => Some(Request::Mission),
            Id::Settings => {
                let state = self.state.as_ref()?;
                self.settings_panel =
                    Some(SettingsPanel::new(SettingsContext::of(state, self.cheats)));
                None
            }
            Id::PlayersPanel => {
                let state = self.state.as_ref()?;
                self.players_panel = Some(match self.panel_target()? {
                    PanelTarget::Player((player, callsign)) => {
                        // The aircraft the AI flies for a player who is away.
                        let release = state.player(player).filter(|p| p.away).and_then(|p| p.slot);
                        PlayersPanel::new(player, &callsign, state.host == Some(player))
                            .with_release(release)
                    }
                    PanelTarget::Slot((plane, callsign)) => {
                        PlayersPanel::for_slot(plane, &callsign)
                    }
                });
                None
            }
            Id::Loadout => Some(match self.buttons.loadout_as {
                LoadoutAs::Loadout => Request::Loadout,
                LoadoutAs::Watch => Request::Watch,
                LoadoutAs::StopWatch => Request::StopWatch,
            }),
            Id::Ready => {
                let ready = !self.facts.ready;
                if ready && !self.facts.armed {
                    self.say(
                        if self.facts.phase == tore_session::wire::messages::LobbyPhase::Flying {
                            "Joining with the aircraft's standard stores."
                        } else {
                            "Ready with the standard stores. Press Loadout first to choose others."
                        },
                    );
                }
                Some(Request::SetReady(ready))
            }
            Id::Fly => Some(match self.buttons.fly_as {
                FlyAs::Fly => Request::Start,
                FlyAs::EndMission => Request::EndMission,
            }),
            Id::Leave => self.leave_pressed(),
            _ => None,
        }
    }

    /// Leave, and Esc: the King is asked first, since the game ends for
    /// everyone.
    fn leave_pressed(&mut self) -> Option<Request> {
        if self.facts.king || (self.hosting && !self.facts.connected) {
            self.modal = Some(Modal::leave());
            None
        } else {
            Some(Request::Leave)
        }
    }

    fn press_default(&mut self) -> Option<Request> {
        match self.default {
            Some(DefaultButton::Fly) => self.press(Id::Fly),
            Some(DefaultButton::Ready) => self.press(Id::Ready),
            None => None,
        }
    }

    /// A click, or Enter, on the slot of `plane`.
    fn slot_clicked(&mut self, plane: u32) -> Option<Request> {
        let state = self.state.as_ref()?;
        match facts::slot_click(state, &self.facts, plane) {
            SlotClick::Take(plane) => Some(Request::Take(plane)),
            SlotClick::Leave => Some(Request::LeaveSlot),
            SlotClick::Refused(why) => {
                self.say(&why);
                None
            }
        }
    }

    fn modal_answer(&mut self, answer: Answer) -> Option<Request> {
        match answer {
            Answer::None => None,
            Answer::Cancel => {
                self.modal = None;
                None
            }
            Answer::Ok(reason) => {
                let modal = self.modal.take()?;
                match modal.purpose {
                    Purpose::Kick(player, _) => Some(Request::Kick { player, reason }),
                    Purpose::Leave => Some(Request::Leave),
                }
            }
        }
    }

    /// What the Settings panel answered.
    fn settings_answer(&mut self, answer: settings_panel::Answer) -> Option<Request> {
        match answer {
            settings_panel::Answer::None => None,
            settings_panel::Answer::Close => {
                self.settings_panel = None;
                None
            }
            settings_panel::Answer::Edit(Edit::Settings(change)) => Some(Request::Settings(change)),
            settings_panel::Answer::Edit(Edit::Cheats(cheats)) => Some(Request::Cheats(cheats)),
        }
    }

    /// What the Players panel answered.
    fn players_answer(&mut self, answer: players_panel::Answer) -> Option<Request> {
        match answer {
            players_panel::Answer::None => None,
            players_panel::Answer::Close => {
                self.players_panel = None;
                None
            }
            players_panel::Answer::Crown(player) => {
                self.players_panel = None;
                Some(Request::PassCrown(player))
            }
            players_panel::Answer::Release(plane) => {
                self.players_panel = None;
                Some(Request::Release(plane))
            }
            players_panel::Answer::Kick(player) => {
                let callsign = self
                    .players_panel
                    .take()
                    .map(|panel| panel.callsign().to_owned())?;
                self.modal = Some(Modal::kick(player, &callsign));
                None
            }
        }
    }

    // ---- events ----

    /// A key press by the window's name for it.
    pub fn key(&mut self, name: &str, shift: bool) -> Option<Request> {
        if let Some(modal) = &mut self.modal {
            let answer = modal.key(name, shift);
            return self.modal_answer(answer);
        }
        if let Some(panel) = &mut self.settings_panel {
            let answer = panel.key(name, shift);
            return self.settings_answer(answer);
        }
        if let Some(panel) = &mut self.players_panel {
            let answer = panel.key(name, shift);
            return self.players_answer(answer);
        }
        if name == "Escape" {
            return self.leave_pressed();
        }
        let ok = self.usable();
        let usable = |id: Id| ok[id as usize];
        match self.focus.key(name, shift, usable) {
            Route::Moved => None,
            Route::Ignored | Route::Default(_) => {
                if name == "Enter" {
                    self.press_default()
                } else {
                    None
                }
            }
            Route::Widget(id) => match id {
                Id::Slots => match self.slots.key(name) {
                    Wo::Changed => {
                        self.picked = Id::Slots;
                        self.refresh();
                        None
                    }
                    Wo::Activated => {
                        let plane = self.selected_plane()?;
                        self.slot_clicked(plane)
                    }
                    _ => None,
                },
                Id::Players => {
                    if self.players.key(name) == Wo::Changed {
                        self.picked = Id::Players;
                        self.refresh();
                    }
                    None
                }
                Id::Messages => {
                    if name == "Enter" {
                        self.press_default()
                    } else {
                        self.chat.messages.key(name);
                        None
                    }
                }
                Id::Line => match self.chat.key(name) {
                    Wo::Activated => match self.chat.take_text() {
                        Some(text) => Some(Request::Chat(text)),
                        None => self.press_default(),
                    },
                    _ => None,
                },
                button => {
                    if self.shown(button) && self.button_mut(button).key(name) == Wo::Activated {
                        self.press(button)
                    } else {
                        None
                    }
                }
            },
        }
    }

    /// Typed text for the field that has the keyboard.
    pub fn text_input(&mut self, text: &str) {
        if let Some(modal) = &mut self.modal {
            modal.text_input(text);
            return;
        }
        if let Some(panel) = &mut self.settings_panel {
            panel.text_input(text);
            return;
        }
        if self.players_panel.is_some() {
            return;
        }
        if self.focus.is(Id::Line) {
            self.chat.text_input(text);
        }
    }

    /// The pointer moved (canvas pixels), or left the canvas.
    pub fn moved(&mut self, point: Option<(f64, f64)>) {
        self.pointer = point.map(|(x, y)| (x as i32, y as i32));
        let point = self.pointer;
        if let Some(modal) = &mut self.modal {
            modal.moved(point);
            return;
        }
        if let Some(panel) = &mut self.settings_panel {
            panel.moved(point);
            return;
        }
        if let Some(panel) = &mut self.players_panel {
            panel.moved(point);
            return;
        }
        for id in Id::BUTTONS {
            self.button_mut(id).pointer_move(point);
        }
        if let Some(p) = self.pointer {
            self.chat.messages.drag(p);
        }
    }

    /// The left mouse button went down or up at the pointer.
    pub fn button(&mut self, pressed: bool) -> Option<Request> {
        let point = self.pointer;
        if let Some(modal) = &mut self.modal {
            let answer = modal.button(&self.kit, point, pressed);
            return self.modal_answer(answer);
        }
        if let Some(panel) = &mut self.settings_panel {
            let answer = panel.button(&self.kit, point, pressed, false);
            return self.settings_answer(answer);
        }
        if let Some(panel) = &mut self.players_panel {
            let answer = panel.button(point, pressed);
            return self.players_answer(answer);
        }
        let now = Instant::now();
        let Some(p) = point else {
            if !pressed {
                self.cancel_press();
            }
            return None;
        };
        if pressed {
            return self.pressed_at(p, now);
        }
        // Released.
        self.slots.release(now);
        self.chat.messages.release();
        let mut fired = None;
        let blocked = self.blocked.take();
        for id in self.shown_buttons() {
            let button = self.button_mut(id);
            let bounds = button.bounds();
            if button.release(p) == Wo::Activated || (blocked == Some(id) && inside(bounds, p)) {
                // A click on a button that cannot be pressed says why
                // ([`LobbyScreen::press`]).
                fired = Some(id);
            }
        }
        match fired {
            Some(id) if self.shown(id) => {
                self.focus.set(id);
                self.press(id)
            }
            _ => None,
        }
    }

    fn pressed_at(&mut self, p: Point, now: Instant) -> Option<Request> {
        let mut request = None;
        if self.chat.field.hit(p) {
            self.focus.set(Id::Line);
            self.chat.field.press(p, &self.kit);
        }
        if self.slots.hit(p) {
            self.focus.set(Id::Slots);
            self.picked = Id::Slots;
        }
        // The rocker is outside the rows' rectangle: the list sorts out what
        // it was pressed on. A row clicked is a slot clicked, once (a
        // double-click's second press is not another click).
        let row = self.slots.row_at(p);
        let outcome = self.slots.press(p, now);
        self.refresh();
        if let Some(index) = row
            && outcome != Wo::Activated
            && let Some(plane) = self
                .slots
                .rows()
                .get(index)
                .and_then(|row| row.key.parse().ok())
        {
            request = self.slot_clicked(plane);
        }
        if self.players.hit(p) {
            self.focus.set(Id::Players);
            self.picked = Id::Players;
        }
        if self.players.press(p, now) == Wo::Changed {
            self.refresh();
        }
        if self.chat.messages.hit(p) {
            self.focus.set(Id::Messages);
        }
        self.chat.messages.press(p);
        for id in self.shown_buttons() {
            let button = self.button_mut(id);
            button.press(p);
            if !Widget::enabled(button) && inside(button.bounds(), p) {
                self.blocked = Some(id);
            }
        }
        request
    }

    /// Lets go of anything held (the window lost focus or was resized).
    pub fn cancel_press(&mut self) {
        let off = (-1, -1);
        self.slots.release(Instant::now());
        self.chat.messages.release();
        for id in Id::BUTTONS {
            let button = self.button_mut(id);
            button.release(off);
            button.pointer_move(None);
        }
        self.pointer = None;
        self.blocked = None;
    }

    /// A wheel step over the pointer's place. Positive is up.
    pub fn wheel(&mut self, notches: i32) {
        let Some(p) = self.pointer else {
            return;
        };
        if self.overlaid() {
            return;
        }
        if self.chat.messages.hit(p) {
            self.chat.messages.wheel(notches);
        } else if self.players.hit(p) {
            if self.players.wheel(notches) == Wo::Changed {
                self.refresh();
            }
        } else if self.slots.hit(p) {
            self.slots.wheel(notches);
            self.refresh();
        }
    }

    /// The right mouse button went down or up at the pointer. Over the
    /// Settings panel it turns a row back; over a slot it is the King's lock
    /// (open, closed, or kept for the player selected in Players).
    pub fn right_button(&mut self, pressed: bool) -> Option<Request> {
        let point = self.pointer;
        if self.modal.is_some() || self.players_panel.is_some() {
            return None;
        }
        if let Some(panel) = &mut self.settings_panel {
            let answer = panel.button(&self.kit, point, pressed, true);
            return self.settings_answer(answer);
        }
        let p = point?;
        if !pressed || !self.facts.king {
            return None;
        }
        let index = self.slots.row_at(p)?;
        let plane: u32 = self.slots.rows().get(index)?.key.parse().ok()?;
        let state = self.state.as_ref()?;
        let slot = state.slots.iter().find(|s| s.plane == plane)?;
        let selected = self
            .players
            .selected_row()
            .and_then(|row| row.key.parse().ok())
            .and_then(|id: u8| state.player(id))
            .map(|p| p.callsign.as_str());
        let lock = facts::lock_click(slot, selected);
        self.slots.select(index);
        self.refresh();
        Some(Request::Lock { plane, lock })
    }

    // ---- drawing ----

    /// The part of the screen that never changes: the composed background,
    /// the panel, the title, the frame lines, the headings and the boxes the
    /// lists sit in. Drawn once and kept.
    fn draw_backdrop(&self, canvas: &mut Canvas) {
        let kit = &*self.kit;
        Background::direct_connection().draw(canvas, kit);
        draw_panel(canvas, kit, (10, 80, 619, 395));
        let font = kit.sprite("PANELFNT");
        let title = "Lobby";
        ui_text::text(
            canvas,
            kit,
            font,
            title,
            (10 + (619 - text_width(font, title)) / 2, 87),
            None,
            None,
        );
        canvas.outline((30, 100, 579, 355), LINE);
        for (label, x, y) in [
            ("Slots", 45, 152),
            ("Players", 400, 152),
            ("Messages", 45, 282),
        ] {
            ui_text::text(canvas, kit, font, label, (x, y), None, None);
        }
        canvas.outline((40, 164, 355, 97), LINE);
        canvas.rect((400, 165, 194, 95), [81, 81, 81, 255]);
        canvas.outline((399, 164, 196, 97), LINE);
    }

    /// Draws the whole screen onto the 640 by 480 canvas.
    pub fn draw(&self, canvas: &mut Canvas) {
        let started = self.timing.as_ref().map(|_| Instant::now());
        self.draw_frame(canvas);
        if let (Some(timing), Some(started)) = (&self.timing, started) {
            timing.drew(started.elapsed());
        }
    }

    fn draw_frame(&self, canvas: &mut Canvas) {
        let kit = &*self.kit;
        {
            let mut cached = self.backdrop.borrow_mut();
            if !cached.as_ref().is_some_and(Backdrop::is_for_now) {
                *cached = Some(Backdrop::new(|canvas| self.draw_backdrop(canvas)));
            }
            if let Some(backdrop) = cached.as_ref() {
                backdrop.put(canvas);
            }
        }
        let font = kit.sprite("PANELFNT");
        let dim = kit.sprite("PANELFND");
        let text = ui_text::text;
        let game = format!("Game: {}", self.game_name());
        text(
            canvas,
            kit,
            font,
            &fit(font, &game, 549),
            (45, 102),
            None,
            None,
        );
        match &self.state {
            Some(state) => {
                let mission = format!("Mission: {}", state.summary);
                text(
                    canvas,
                    kit,
                    font,
                    &fit(font, &mission, 549),
                    (45, 115),
                    None,
                    None,
                );
                let rule = fit(dim, &facts::rule_text(state), 549);
                text(canvas, kit, dim, &rule, (45, 128), None, None);
                // The King's settings in words (slice F2-L).
                if let Some(summary) = facts::settings_summary(state) {
                    let summary = fit(font, &format!("Rules: {summary}"), 549);
                    text(canvas, kit, font, &summary, (45, 141), None, None);
                }
            }
            None => {
                text(
                    canvas,
                    kit,
                    dim,
                    "Waiting for the game's lobby...",
                    (45, 115),
                    None,
                    None,
                );
            }
        }
        let hint = self
            .state
            .as_ref()
            .zip(self.players.selected_row())
            .and_then(|(state, row)| facts::player_detail(state, row.key.parse().ok()?))
            .unwrap_or_else(|| facts::hint(&self.facts));
        text(
            canvas,
            kit,
            font,
            &fit(font, &hint, 549),
            (45, 266),
            None,
            None,
        );
        let marked = |id| self.focus.marked(id);
        self.slots.draw(canvas, kit, marked(Id::Slots));
        self.players.draw(canvas, kit, marked(Id::Players));
        self.chat
            .draw(canvas, kit, marked(Id::Messages), self.focus.is(Id::Line));
        for id in Id::BUTTONS {
            if self.shown(id) {
                self.button_ref(id).draw(canvas, kit, marked(id));
            }
        }
        if let Some(panel) = &self.settings_panel {
            panel.draw(canvas, kit);
        }
        if let Some(panel) = &self.players_panel {
            panel.draw(canvas, kit);
        }
        if let Some(modal) = &self.modal {
            modal.draw(canvas, kit);
        }
    }
}
