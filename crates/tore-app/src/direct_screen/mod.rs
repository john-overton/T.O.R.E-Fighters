//! The Direct Connection screen (slice EF7): where a player finds a game on
//! the local network, joins one by address, or starts one of their own, until
//! the lobby screen (EF8) takes over the second half. Built of the widget kit
//! ([`crate::widgets`]) at NEWNET's rectangles on `MODEM3` under `NETIPX3`'s
//! title bar, the look John approved on 2026-10-01; the design and the choices
//! below are in `docs/ARCHITECTURE.md`, "Lobby and hosting".
//!
//! The screen is plain state: the game gives it events (keys, text, the
//! pointer, the wheel) and a turn each frame ([`DirectScreen::update`]), and it
//! answers with an [`Outcome`] for the game to carry out (leave the screen,
//! join, host, end the session). It never opens a window, a session or a host
//! itself, so every rule on it is tested without a display. What it does own
//! is the search for games ([`Search`]), which runs only while it is open and
//! not in a session, and the address lookup ([`Lookup`]) of a typed address.
//!
//! # What the buttons and keys do
//!
//! - **New** hosts a game (a [`HostRequest`]) once the callsign is checked.
//! - **Join** joins the selected game, or the typed address through the
//!   lookup, which tries every address the name gives.
//! - **Options** opens the panel of [`options`]; **Cancel** (or Esc) stops a
//!   lookup, leaves a session, or leaves the screen, in that order.
//! - **Enter** presses the default button: Join once a game is selected or an
//!   address is typed, New otherwise (*agent decision*). Tab and Shift+Tab walk
//!   the controls; every control also works by mouse.
//! - In **Connect to**, Up and Down (or the wheel) step through the addresses
//!   joined before (*agent decision*).
use crate::menu::{Canvas, text_width};
use crate::net::{
    lookup::{Lookup, Progress},
    options::{DEFAULT_CALLSIGN, callsign_problem},
    search::{Compat, Game, Own, Search, SearchEvent},
    settings::Remembered,
};
use crate::widgets::{
    Align, Background, Button, Cell, CheckBox, Column, Filter, Focus, Icon, Kit, List, MessageBox,
    Outcome as Wo, Pager, Point, Route, Row, TextField, Widget as _, draw_panel, fit, tone,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tore_formats::chat::QuickMessage;
use tore_net::packet::DiscoverPhase;

pub mod app;
mod options;
pub mod preview;
#[cfg(test)]
mod tests;

use options::{Answer, OptionsPanel};

/// The screen's own frame lines: the colour measured on John's screenshot.
const LINE: [u8; 4] = [174, 174, 174, 255];

/// The controls Tab visits, in order. The Players list shows the selected
/// game's players and is not one of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Id {
    Callsign,
    Address,
    Full,
    Games,
    Messages,
    New,
    Join,
    Options,
    Cancel,
}

/// What Join goes to when both a game is selected and an address is typed:
/// whichever the player touched last (*agent decision*).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Typed,
    Game,
}

/// A join the game should start: the address is known (a found game, or the
/// address a lookup reached).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinRequest {
    pub address: SocketAddr,
    /// What the player sees it called: the game's name or the typed address.
    pub label: String,
    pub callsign: String,
    /// Empty for none.
    pub password: String,
}

/// A game the player asked to host. The game builds it from the Quick
/// Mission creator's mission until the lobby screen exists (EF8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostRequest {
    pub callsign: String,
    pub name: String,
    pub port: u16,
    pub password: Option<String>,
}

/// What the game does after an event reaches the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing; the screen redraws every frame while it is up.
    None,
    /// Leave the screen, back to Choose Activity. The search stops with it.
    Close,
    Join(JoinRequest),
    Host(HostRequest),
    /// Leave the game session this screen started (Cancel while joined).
    EndSession,
}

/// The screen.
pub struct DirectScreen {
    kit: Arc<Kit>,
    background: Background,
    callsign: TextField,
    address: TextField,
    full: CheckBox,
    games: List,
    players: List,
    messages: MessageBox,
    new: Button,
    join: Button,
    options_button: Button,
    cancel: Button,
    focus: Focus<Id>,
    target: Target,
    /// Where Up and Down in Connect to are among the remembered addresses.
    recent: Option<usize>,
    settings: Remembered,
    /// Where the settings are kept; `None` keeps nothing (tests, previews).
    data: Option<PathBuf>,
    /// The password to send or host with, from Options. Never stored.
    password: String,
    quick: Vec<QuickMessage>,
    panel: Option<OptionsPanel>,
    /// The games the search holds, before the list filters them.
    found: Vec<Game>,
    search: Option<Search>,
    /// A search that could not start is not tried again until something
    /// changes (a new port, a session that ended).
    search_failed: bool,
    lookup: Option<Lookup>,
    /// What the lookup was started for, as the player typed it.
    looking_for: String,
    /// A session this screen's join or host started is running.
    session: bool,
    /// The search's clock.
    clock: tore_net::RealClock,
    pointer: Option<Point>,
    /// The port the search was asked for (kept to tell what changed).
    searched_port: u16,
    /// The backdrop, drawn the first time the screen is.
    backdrop: std::cell::OnceCell<Vec<u8>>,
}

impl DirectScreen {
    /// A screen on `kit` (in `MODEM3`'s palette). `data` is where the
    /// remembered settings are read from and written to; `quick` the retail
    /// quick messages Options shows.
    pub fn new(kit: Arc<Kit>, quick: Vec<QuickMessage>, data: Option<PathBuf>) -> Self {
        let settings = data.as_deref().map(Remembered::load).unwrap_or_default();
        let games_columns = vec![
            Column {
                x: 0,
                width: 11,
                align: Align::Centre,
            },
            Column {
                x: 14,
                width: 84,
                align: Align::Left,
            },
            Column {
                x: 100,
                width: 22,
                align: Align::Right,
            },
            Column {
                x: 126,
                width: 46,
                align: Align::Left,
            },
        ];
        let players_columns = vec![
            Column {
                x: 0,
                width: 11,
                align: Align::Centre,
            },
            Column {
                x: 14,
                width: 210,
                align: Align::Left,
            },
        ];
        let mut callsign =
            TextField::bar((88, 108), 139, Filter::Callsign).with_hint("your callsign");
        if let Some(name) = &settings.callsign {
            callsign.set_text(name);
        }
        let mut full = CheckBox::new((330, 130), "Show full games", settings.show_full);
        full.set_checked(settings.show_full);
        let mut focus = Focus::new(
            vec![
                Id::Callsign,
                Id::Address,
                Id::Full,
                Id::Games,
                Id::Messages,
                Id::New,
                Id::Join,
                Id::Options,
                Id::Cancel,
            ],
            None,
        );
        // A first visit starts in the callsign, which Join and New ask for.
        if settings.callsign.is_none() {
            focus.set(Id::Callsign);
        }
        let mut screen = Self {
            kit,
            background: Background::direct_connection(),
            callsign,
            address: TextField::edit((110, 132), 20, Filter::Address).with_hint("host or address"),
            full,
            games: List::new((48, 185), 200, 4)
                .with_pager(Pager::NEWNET)
                .with_columns(games_columns),
            players: List::new((346, 185), 242, 5).with_columns(players_columns),
            messages: MessageBox::newnet(),
            new: Button::new("New", (106, 419), 85).default_button(),
            join: Button::new("Join", (229, 419), 85),
            options_button: Button::new("Options", (352, 419), 85),
            cancel: Button::new("Cancel", (475, 419), 85),
            focus,
            target: Target::Typed,
            recent: None,
            settings,
            data,
            password: String::new(),
            quick,
            panel: None,
            found: Vec::new(),
            search: None,
            search_failed: false,
            lookup: None,
            looking_for: String::new(),
            session: false,
            clock: tore_net::RealClock::new(),
            pointer: None,
            searched_port: 0,
            backdrop: std::cell::OnceCell::new(),
        };
        if !screen.settings.addresses.is_empty() {
            screen.say("Up and Down in Connect to pick an address you joined before.");
        }
        screen
    }

    // ---- state the game reads ----

    /// A search is running.
    #[cfg(test)]
    pub fn searching(&self) -> bool {
        self.search.is_some()
    }
    /// The search holds the game port itself (false after a fallback, or
    /// when none runs).
    #[cfg(test)]
    pub fn search_on_game_port(&self) -> bool {
        self.search.as_ref().is_some_and(Search::on_game_port)
    }
    /// An address is being looked up or tried.
    #[cfg(test)]
    pub fn looking_up(&self) -> bool {
        self.lookup.is_some()
    }
    /// The Options panel is open.
    #[cfg(test)]
    pub fn options_open(&self) -> bool {
        self.panel.is_some()
    }
    /// A session the screen started (joined or hosting) is running.
    #[cfg(test)]
    pub fn in_session(&self) -> bool {
        self.session
    }
    /// The callsign as typed.
    #[cfg(test)]
    pub fn callsign_text(&self) -> &str {
        self.callsign.text()
    }
    /// The address as typed.
    #[cfg(test)]
    pub fn address_text(&self) -> &str {
        self.address.text()
    }
    /// The lines in Messages, oldest first.
    #[cfg(test)]
    pub fn message_lines(&self) -> Vec<String> {
        self.messages
            .lines()
            .map(|(text, _)| text.to_owned())
            .collect()
    }
    /// The button Enter presses now.
    #[cfg(test)]
    pub fn default_button(&self) -> &'static str {
        if self.join_is_default() {
            "Join"
        } else {
            "New"
        }
    }
    /// True when text typed now goes to a field.
    pub fn typing(&self) -> bool {
        match &self.panel {
            Some(panel) => panel.typing(),
            None => {
                !self.busy() && matches!(self.focus.current(), Some(Id::Callsign | Id::Address))
            }
        }
    }

    /// Something is going on that the player may stop: a lookup, or a
    /// session.
    fn busy(&self) -> bool {
        self.lookup.is_some() || self.session
    }

    fn selected_game(&self) -> Option<&Game> {
        let key = &self.games.selected_row()?.key;
        self.found
            .iter()
            .find(|game| game.answer.session_id.to_string() == *key)
    }

    fn join_is_default(&self) -> bool {
        !self.busy()
            && (self.selected_game().is_some()
                || (!self.address.is_empty() && self.target == Target::Typed))
    }

    // ---- messages ----

    /// Adds a line to Messages in the game's own colour.
    pub fn say(&mut self, text: &str) {
        log::info!("Direct Connection: {text}");
        self.messages.push(&self.kit, text, tone::SYSTEM);
    }

    // ---- the turn ----

    /// The screen's turn each frame: the search started, stopped and read, the
    /// lookup's progress, the lists refreshed and the widgets animated.
    /// `session` says whether the game has a session open. Never blocks.
    pub fn update(&mut self, session: bool) -> Outcome {
        let now = self.clock.now();
        self.update_at(now, session)
    }

    /// [`DirectScreen::update`] at a given time on the search's clock.
    pub fn update_at(&mut self, now: Duration, session: bool) -> Outcome {
        if self.session && !session {
            // The session ended (the game said why in Messages).
            self.search_failed = false;
        }
        self.session = session;
        self.sync_search(now);
        let outcome = self.poll_lookup();
        self.poll_search(now);
        let at = Instant::now();
        self.games.advance(at);
        self.players.advance(at);
        self.full.advance(at);
        self.refresh_buttons();
        outcome
    }

    /// The search runs while the screen is open and no session is: started
    /// here, stopped by a session or a change of port.
    fn sync_search(&mut self, now: Duration) {
        if self.session {
            if self.search.take().is_some() {
                log::info!("Direct Connection: search stopped for the session");
                self.found.clear();
                self.rebuild_games();
            }
            return;
        }
        if self.search.is_some() && self.searched_port != self.settings.port {
            self.search = None;
            self.found.clear();
            self.rebuild_games();
        }
        if self.search.is_none() && !self.search_failed && self.panel.is_none() {
            self.start_search(now);
        }
    }

    /// Starts the search on the remembered port, saying in Messages how it
    /// went (EF5: it takes the game port when it is free, another port when
    /// it is not).
    pub fn start_search(&mut self, now: Duration) {
        let port = self.settings.port;
        self.searched_port = port;
        match Search::start(port, Own::this_game(), now) {
            Ok(search) => {
                if search.on_game_port() {
                    self.say(&format!(
                        "Searching for games on the local network (UDP port {port})..."
                    ));
                } else {
                    let local = search
                        .local_port()
                        .map_or_else(String::new, |p| format!(" {p}"));
                    self.say(&format!(
                        "Searching for games on the local network. UDP port {port} is busy on this machine (is a game or a server running?), so the search listens on port{local}; other computers may not answer."
                    ));
                }
                self.search = Some(search);
            }
            Err(error) => {
                self.search_failed = true;
                self.say(&format!("Cannot search the network: {error}"));
            }
        }
    }

    fn poll_search(&mut self, now: Duration) {
        let Some(search) = &mut self.search else {
            return;
        };
        search.update(now);
        let mut changed = false;
        let mut lines = Vec::new();
        while let Some(event) = search.poll_event() {
            changed = true;
            match event {
                SearchEvent::Added(game) => {
                    lines.push(format!("Found {} at {}.", game.answer.name, game.address))
                }
                SearchEvent::Dropped(game) => {
                    lines.push(format!("{} is no longer on the network.", game.answer.name));
                }
                SearchEvent::Changed(_) => {}
            }
        }
        if changed {
            self.found = search.games().to_vec();
            for line in lines {
                self.say(&line);
            }
            self.rebuild_games();
        }
    }

    /// Takes a lookup's progress into Messages; a reached address is a join.
    fn poll_lookup(&mut self) -> Outcome {
        let mut outcome = Outcome::None;
        let mut done = false;
        let mut lines = Vec::new();
        if let Some(lookup) = &mut self.lookup {
            while let Some(progress) = lookup.poll() {
                lines.push(progress.to_string());
                match progress {
                    Progress::Reached(address) => {
                        outcome = Outcome::Join(JoinRequest {
                            address,
                            label: self.looking_for.clone(),
                            callsign: self.callsign.text().to_owned(),
                            password: self.password.clone(),
                        });
                        done = true;
                    }
                    Progress::Refused(..) | Progress::Failed(_) => done = true,
                    _ => {}
                }
            }
        }
        for line in lines {
            self.say(&line);
        }
        if done {
            self.lookup = None;
            if matches!(outcome, Outcome::Join(_)) {
                self.remember_typed();
                self.search = None;
            }
        }
        outcome
    }

    fn refresh_buttons(&mut self) {
        let busy = self.busy();
        let join_default = self.join_is_default();
        self.join.set_default(join_default);
        self.new.set_default(!join_default);
        for button in [&mut self.new, &mut self.join, &mut self.options_button] {
            button.set_enabled(!busy);
        }
        self.cancel
            .set_label(if self.session { "Leave" } else { "Cancel" });
        self.full.set_enabled(!busy);
        self.games.set_enabled(!busy);
    }

    // ---- the lists ----

    /// Puts the found games in the list: full games only when "Show full
    /// games" is on, a game from another build dimmed and shown by its
    /// version. The selection stays on its game.
    fn rebuild_games(&mut self) {
        let show_full = self.full.checked();
        let rows = self
            .found
            .iter()
            .filter(|game| show_full || !game.answer.full || game.compat != Compat::Same)
            .map(game_row)
            .collect();
        self.games.set_rows(rows);
        self.rebuild_players();
    }

    fn rebuild_players(&mut self) {
        let rows = match self.selected_game() {
            None => Vec::new(),
            Some(game) => {
                let answer = &game.answer;
                let mut rows: Vec<Row> = answer
                    .callsigns
                    .iter()
                    .map(|name| {
                        let king = !answer.king.is_empty() && *name == answer.king;
                        Row::new(
                            name.clone(),
                            vec![
                                if king {
                                    Cell::Icon(Icon::Crown)
                                } else {
                                    Cell::Empty
                                },
                                Cell::Text(name.clone()),
                            ],
                        )
                    })
                    .collect();
                let more = usize::from(answer.players).saturating_sub(rows.len());
                if more > 0 {
                    rows.push(Row::text(format!("and {more} more")).dimmed());
                }
                rows
            }
        };
        self.players.set_rows(rows);
    }

    /// The selected game's mission, or why it cannot be joined, for the line
    /// under the Games box.
    fn selection_line(&self) -> Option<String> {
        let game = self.selected_game()?;
        let a = &game.answer;
        Some(match game.compat {
            Compat::Same => format!("Mission: {}", a.summary),
            _ => format!(
                "{} runs another version ({} {}) and cannot be joined.",
                a.name,
                a.game_version,
                a.game_commit.chars().take(9).collect::<String>()
            ),
        })
    }

    /// The player picked a game: it is the target, and its players show.
    fn game_selected(&mut self) {
        self.target = Target::Game;
        self.rebuild_players();
    }

    // ---- actions ----

    /// The callsign, checked: the reason in Messages when it is not usable.
    fn checked_callsign(&mut self) -> Option<String> {
        let text = self.callsign.text().to_owned();
        let problem = if text.is_empty() {
            Some("Type your callsign first.".to_owned())
        } else {
            callsign_problem(&text)
                .map(|problem| format!("That callsign cannot be used: {problem}."))
        };
        match problem {
            None => Some(text),
            Some(problem) => {
                self.say(&problem);
                self.focus.set(Id::Callsign);
                None
            }
        }
    }

    /// Writes the callsign, the port and "Show full games" down (and the
    /// game name Options set), as far as they are valid.
    pub fn save(&mut self) {
        let text = self.callsign.text().to_owned();
        self.settings.remember_callsign(&text);
        self.settings.show_full = self.full.checked();
        if let Some(data) = &self.data
            && let Err(error) = self.settings.save(data)
        {
            log::warn!("Network settings not saved: {error}");
        }
    }

    fn remember_typed(&mut self) {
        let (host, port) = match crate::widgets::parse_address(&self.looking_for) {
            Ok(parsed) => parsed,
            Err(_) => return,
        };
        let port = port.unwrap_or(self.settings.port);
        let text = if host.contains(':') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        };
        self.settings.remember_address(&text);
        self.save();
    }

    fn press(&mut self, id: Id) -> Outcome {
        match id {
            Id::New => self.new_game(),
            Id::Join => self.join_game(),
            Id::Options => {
                self.open_options();
                Outcome::None
            }
            Id::Cancel => self.cancel_or_leave(),
            _ => Outcome::None,
        }
    }

    fn press_default(&mut self) -> Outcome {
        if self.busy() {
            return Outcome::None;
        }
        if self.join_is_default() {
            self.join_game()
        } else {
            self.new_game()
        }
    }

    fn new_game(&mut self) -> Outcome {
        if self.busy() {
            return Outcome::None;
        }
        let Some(callsign) = self.checked_callsign() else {
            return Outcome::None;
        };
        let name = self
            .settings
            .game_name
            .clone()
            .unwrap_or_else(|| format!("{callsign}'s game"));
        self.save();
        // The search holds the game port; the host needs it (EF5's note).
        if self.search.take().is_some() {
            log::info!("Direct Connection: search stopped before hosting");
        }
        Outcome::Host(HostRequest {
            callsign,
            name,
            port: self.settings.port,
            password: Some(self.password.clone()).filter(|p| !p.is_empty()),
        })
    }

    fn join_game(&mut self) -> Outcome {
        if self.busy() {
            return Outcome::None;
        }
        let Some(callsign) = self.checked_callsign() else {
            return Outcome::None;
        };
        let use_game = match (self.selected_game().is_some(), self.address.is_empty()) {
            (true, true) => true,
            (true, false) => self.target == Target::Game,
            (false, _) => false,
        };
        if use_game {
            return self.join_found(callsign);
        }
        self.join_typed()
    }

    fn join_found(&mut self, callsign: String) -> Outcome {
        let Some(game) = self.selected_game() else {
            return Outcome::None;
        };
        let (name, address) = (game.answer.name.clone(), game.address);
        let problem = match (game.compat, game.answer.phase, game.answer.full) {
            (Compat::OtherBuild | Compat::OtherProtocol, ..) => Some(format!(
                "{name} runs another version ({} {}); it cannot be joined.",
                game.answer.game_version,
                game.answer.game_commit.chars().take(9).collect::<String>()
            )),
            (_, DiscoverPhase::Closed, _) => Some(format!("{name} is closing; try again soon.")),
            (_, _, true) => Some(format!("{name} is full.")),
            _ => None,
        };
        if let Some(problem) = problem {
            self.say(&problem);
            return Outcome::None;
        }
        self.say(&format!(
            "Attempting connection to '{name}' at {address}..."
        ));
        self.save();
        // The join takes over from the search.
        self.search = None;
        Outcome::Join(JoinRequest {
            address,
            label: name,
            callsign,
            password: self.password.clone(),
        })
    }

    fn join_typed(&mut self) -> Outcome {
        let text = self.address.text().trim().to_owned();
        if text.is_empty() {
            self.say("Pick a game in the list, or type an address in Connect to.");
            self.focus.set(Id::Address);
            return Outcome::None;
        }
        let (host, port) = match crate::widgets::parse_address(&text) {
            Ok(parsed) => parsed,
            Err(problem) => {
                self.say(problem);
                self.focus.set(Id::Address);
                return Outcome::None;
            }
        };
        // An address typed without a port uses the one in Options.
        let port = port.unwrap_or(self.settings.port);
        let with_port = if host.contains(':') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        };
        match Lookup::start(&with_port, tore_session::wire::PROTOCOL_VERSION) {
            Ok(lookup) => {
                self.lookup = Some(lookup);
                self.looking_for = with_port;
                self.save();
            }
            Err(problem) => {
                self.say(&problem);
                self.focus.set(Id::Address);
            }
        }
        Outcome::None
    }

    fn cancel_or_leave(&mut self) -> Outcome {
        if let Some(mut lookup) = self.lookup.take() {
            lookup.cancel();
            self.say("Cancelled.");
            return Outcome::None;
        }
        if self.session {
            return Outcome::EndSession;
        }
        self.save();
        Outcome::Close
    }

    fn open_options(&mut self) {
        let default_name = format!(
            "{}'s game",
            if self.callsign.is_empty() {
                DEFAULT_CALLSIGN
            } else {
                self.callsign.text()
            }
        );
        self.panel = Some(OptionsPanel::new(
            &self.kit,
            self.settings.port,
            &self.password,
            self.settings.game_name.as_deref(),
            &default_name,
            &self.quick,
        ));
    }

    fn options_answer(&mut self, answer: Answer) {
        match answer {
            Answer::None => {}
            Answer::Cancel => self.panel = None,
            Answer::Ok(values) => {
                self.panel = None;
                self.password = values.password;
                self.settings.port = values.port;
                match values.name {
                    Some(name) => self.settings.remember_game_name(&name),
                    None => self.settings.game_name = None,
                }
                self.save();
            }
        }
    }

    // ---- events ----

    /// A key press by the window's name for it.
    pub fn key(&mut self, name: &str, shift: bool) -> Outcome {
        if let Some(panel) = &mut self.panel {
            let answer = panel.key(name, shift);
            self.options_answer(answer);
            return Outcome::None;
        }
        if name == "Escape" {
            return self.cancel_or_leave();
        }
        let busy = self.busy();
        let usable = |id: Id| match id {
            Id::Cancel | Id::Messages => true,
            _ => !busy,
        };
        let now = Instant::now();
        match self.focus.key(name, shift, usable) {
            Route::Moved => Outcome::None,
            Route::Ignored | Route::Default(_) => {
                if name == "Enter" {
                    self.press_default()
                } else {
                    Outcome::None
                }
            }
            // The fields do not take keys while something is going on.
            Route::Widget(Id::Callsign | Id::Address | Id::Full | Id::Games) if busy => {
                Outcome::None
            }
            Route::Widget(id) => match id {
                Id::Callsign => match self.callsign.key(name) {
                    Wo::Activated => self.press_default(),
                    _ => Outcome::None,
                },
                Id::Address => {
                    if matches!(name, "ArrowUp" | "ArrowDown") {
                        self.step_recent(name == "ArrowUp");
                        return Outcome::None;
                    }
                    match self.address.key(name) {
                        Wo::Activated => self.press_default(),
                        Wo::Changed => {
                            self.address_edited();
                            Outcome::None
                        }
                        _ => Outcome::None,
                    }
                }
                Id::Full => {
                    if self.full.key(name, now) == Wo::Changed {
                        self.rebuild_games();
                        self.save();
                    }
                    Outcome::None
                }
                Id::Games => match self.games.key(name) {
                    Wo::Activated => self.press_default(),
                    Wo::Changed => {
                        self.game_selected();
                        Outcome::None
                    }
                    _ => {
                        if name == "Enter" {
                            self.press_default()
                        } else {
                            Outcome::None
                        }
                    }
                },
                Id::Messages => {
                    if name == "Enter" {
                        self.press_default()
                    } else {
                        self.messages.key(name);
                        Outcome::None
                    }
                }
                Id::New | Id::Join | Id::Options | Id::Cancel => {
                    let button = self.button_mut(id);
                    if button.key(name) == Wo::Activated {
                        self.press(id)
                    } else {
                        Outcome::None
                    }
                }
            },
        }
    }

    /// Typed text for the field that has the keyboard.
    pub fn text_input(&mut self, text: &str) {
        if let Some(panel) = &mut self.panel {
            panel.text_input(text);
            return;
        }
        if self.busy() {
            return;
        }
        match self.focus.current() {
            Some(Id::Callsign) => {
                self.callsign.text_input(text);
            }
            Some(Id::Address) => {
                if self.address.text_input(text) == Wo::Changed {
                    self.address_edited();
                }
            }
            _ => {}
        }
    }

    /// The typed address changed: it is the target, and no game is chosen.
    fn address_edited(&mut self) {
        self.target = Target::Typed;
        self.recent = None;
        self.games.clear_selection();
        self.rebuild_players();
    }

    /// Up (older) or Down (newer) in Connect to: the addresses joined before.
    fn step_recent(&mut self, older: bool) {
        let kept = &self.settings.addresses;
        if kept.is_empty() {
            return;
        }
        let at = match (self.recent, older) {
            (None, true) => Some(0),
            (None, false) => None,
            (Some(i), true) => Some((i + 1).min(kept.len() - 1)),
            (Some(0), false) => None,
            (Some(i), false) => Some(i - 1),
        };
        self.recent = at;
        let text = at.map_or_else(String::new, |i| kept[i].clone());
        self.address.set_text(&text);
        self.target = Target::Typed;
        self.games.clear_selection();
        self.rebuild_players();
    }

    fn button_mut(&mut self, id: Id) -> &mut Button {
        match id {
            Id::New => &mut self.new,
            Id::Join => &mut self.join,
            Id::Options => &mut self.options_button,
            _ => &mut self.cancel,
        }
    }

    /// The pointer moved (canvas pixels), or left the canvas.
    pub fn moved(&mut self, point: Option<(f64, f64)>) {
        self.pointer = point.map(|(x, y)| (x as i32, y as i32));
        let point = self.pointer;
        if let Some(panel) = &mut self.panel {
            panel.moved(point);
            return;
        }
        for button in [
            &mut self.new,
            &mut self.join,
            &mut self.options_button,
            &mut self.cancel,
        ] {
            button.pointer_move(point);
        }
    }

    /// The left mouse button went down or up at the pointer.
    pub fn button(&mut self, pressed: bool) -> Outcome {
        let point = self.pointer;
        if let Some(panel) = &mut self.panel {
            let answer = panel.button(&self.kit, point, pressed);
            self.options_answer(answer);
            return Outcome::None;
        }
        let now = Instant::now();
        let Some(p) = point else {
            if !pressed {
                self.release_all();
            }
            return Outcome::None;
        };
        let busy = self.busy();
        if pressed {
            if !busy {
                if self.callsign.hit(p) {
                    self.focus.set(Id::Callsign);
                    self.callsign.press(p, &self.kit);
                }
                if self.address.hit(p) {
                    self.focus.set(Id::Address);
                    self.address.press(p, &self.kit);
                }
                if self.full.hit(p) {
                    self.focus.set(Id::Full);
                    self.full.press(p);
                }
                if self.games.hit(p) {
                    self.focus.set(Id::Games);
                }
                // The rocker is outside the rows' rectangle: the list sorts
                // out what it was pressed on.
                match self.games.press(p, now) {
                    Wo::Changed => self.game_selected(),
                    Wo::Activated => {
                        self.game_selected();
                        return self.press_default();
                    }
                    Wo::None => {}
                }
            }
            if self.messages.hit(p) {
                self.focus.set(Id::Messages);
            }
            self.messages.press(p);
            for button in [
                &mut self.new,
                &mut self.join,
                &mut self.options_button,
                &mut self.cancel,
            ] {
                button.press(p);
            }
            return Outcome::None;
        }
        // Released.
        self.games.release(now);
        if self.full.release(p, now) == Wo::Changed {
            self.rebuild_games();
            self.save();
        }
        let mut fired = None;
        for (id, button) in [
            (Id::New, &mut self.new),
            (Id::Join, &mut self.join),
            (Id::Options, &mut self.options_button),
            (Id::Cancel, &mut self.cancel),
        ] {
            if button.release(p) == Wo::Activated {
                fired = Some(id);
            }
        }
        match fired {
            Some(id) => {
                self.focus.set(id);
                self.press(id)
            }
            None => Outcome::None,
        }
    }

    fn release_all(&mut self) {
        self.cancel_press();
    }

    /// Lets go of anything held (the window lost focus or was resized).
    pub fn cancel_press(&mut self) {
        let off = (-1, -1);
        let now = Instant::now();
        self.games.release(now);
        self.full.release(off, now);
        for button in [
            &mut self.new,
            &mut self.join,
            &mut self.options_button,
            &mut self.cancel,
        ] {
            button.release(off);
            button.pointer_move(None);
        }
        self.pointer = None;
    }

    /// A wheel step over the pointer's place: the list, the players, the
    /// messages or the address field's history. Positive is up.
    pub fn wheel(&mut self, notches: i32) {
        let point = self.pointer;
        if let Some(panel) = &mut self.panel {
            panel.wheel(notches, point);
            return;
        }
        let Some(p) = point else {
            return;
        };
        if self.messages.hit(p) {
            self.messages.wheel(notches);
        } else if self.players.hit(p) {
            self.players.wheel(notches);
        } else if !self.busy() && self.games.hit(p) {
            if self.games.wheel(notches) == Wo::Changed {
                self.game_selected();
            }
        } else if !self.busy() && self.address.hit(p) {
            self.step_recent(notches > 0);
        }
    }

    // ---- drawing ----

    /// The part of the screen that never changes: the composed background,
    /// the panel, the title, the frame lines, the headings and the boxes the
    /// lists sit in. Drawn once and kept (*agent decision*: about 0.45 ms of
    /// the screen's 0.75 ms a frame in a release build).
    fn draw_backdrop(&self, canvas: &mut Canvas) {
        let kit = &*self.kit;
        self.background.draw(canvas, kit);
        draw_panel(canvas, kit, (10, 80, 619, 395));
        let font = kit.sprite("PANELFNT");
        let title = "TCP/IP Network connection";
        canvas.text(
            font,
            title,
            10 + (619 - text_width(font, title)) / 2,
            87,
            None,
        );
        canvas.outline((30, 100, 579, 355), LINE);
        for (label, x, y) in [
            ("Callsign:", 45, 110),
            ("Connect to:", 45, 139),
            ("Games", 45, 165),
            ("Players", 340, 165),
            ("Messages", 45, 304),
        ] {
            canvas.text(font, label, x, y, None);
        }
        canvas.outline((45, 180, 269, 105), LINE);
        canvas.rect((341, 181, 252, 103), [81, 81, 81, 255]);
        canvas.outline((340, 180, 254, 105), LINE);
    }

    /// Draws the whole screen onto the 640 by 480 canvas.
    pub fn draw(&self, canvas: &mut Canvas) {
        let kit = &*self.kit;
        let backdrop = self.backdrop.get_or_init(|| {
            let mut pixels = vec![0u8; crate::menu::WIDTH * crate::menu::HEIGHT * 4];
            self.draw_backdrop(&mut Canvas(&mut pixels));
            pixels
        });
        canvas.0[..backdrop.len()].copy_from_slice(backdrop);
        let font = kit.sprite("PANELFNT");
        if let Some(line) = self.selection_line() {
            let line = fit(font, &line, 549);
            canvas.text(font, &line, 45, 290, None);
        }
        let marked = |id| self.focus.marked(id);
        self.callsign.draw(canvas, kit, self.focus.is(Id::Callsign));
        self.address.draw(canvas, kit, self.focus.is(Id::Address));
        self.full.draw(canvas, kit, marked(Id::Full));
        self.players.draw(canvas, kit, false);
        self.games.draw(canvas, kit, marked(Id::Games));
        self.messages.draw(canvas, kit, marked(Id::Messages));
        self.new.draw(canvas, kit, marked(Id::New));
        self.join.draw(canvas, kit, marked(Id::Join));
        self.options_button.draw(canvas, kit, marked(Id::Options));
        self.cancel.draw(canvas, kit, marked(Id::Cancel));
        if let Some(panel) = &self.panel {
            panel.draw(canvas, kit);
        }
    }
}

/// One found game as a row of the Games list: the lock, the name, players and
/// capacity, and lobby or flying (a game from another build shows its
/// version instead and is dimmed, like a full or closing one).
fn game_row(game: &Game) -> Row {
    let a = &game.answer;
    let state = match game.compat {
        Compat::Same => match a.phase {
            DiscoverPhase::Lobby => "Lobby".to_owned(),
            DiscoverPhase::Flying => "Flying".to_owned(),
            DiscoverPhase::Closed => "Closed".to_owned(),
        },
        Compat::OtherBuild | Compat::OtherProtocol => format!("v{}", a.game_version),
    };
    let row = Row::new(
        a.session_id.to_string(),
        vec![
            if a.password {
                Cell::Icon(Icon::Lock)
            } else {
                Cell::Empty
            },
            Cell::Text(a.name.clone()),
            Cell::Text(format!("{}/{}", a.players, a.capacity)),
            Cell::Text(state),
        ],
    );
    if game.compat != Compat::Same || a.full || a.phase == DiscoverPhase::Closed {
        row.dimmed()
    } else {
        row
    }
}
