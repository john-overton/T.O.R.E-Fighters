//! The Internet Lobby screen (slice I4): where a player finds a game listed on
//! the master server, joins one, or lists one of their own. It is the Direct
//! Connection screen's sibling: the same `NETIPX3` background, panel, widgets
//! and rectangles ([`crate::widgets`]), with the title bar lettered INTERNET
//! LOBBY. The design and the choices below are in `docs/ARCHITECTURE.md`,
//! "Master server and connectivity", "The Internet Lobby screen".
//!
//! Like [`crate::direct_screen::DirectScreen`] the screen is plain state: the
//! game gives it events (keys, text, the pointer, the wheel) and a turn each
//! frame ([`InternetScreen::update`]), and it answers with an [`Outcome`] for
//! the game to carry out (leave the screen, join, host, end the session). It
//! never opens a window, a session or a host itself, so every rule on it is
//! tested without a display. What it owns is the browse of the master
//! ([`Browse`]), which runs only while it is open and no session is.
//!
//! # What the buttons and keys do
//!
//! - **New** hosts a game listed on the Internet Lobby (a [`HostRequest`]),
//!   once the callsign is checked.
//! - **Join** joins the selected game: it asks the master to introduce the
//!   player ([`Browse::introduce`]) and, when the master gives the host's
//!   address, starts the join. Enter presses it when a game is selected, New
//!   otherwise (*agent decision*, as on Direct Connection).
//! - **Refresh** (or F5) asks for the list again now (*agent decision*).
//! - **Options** opens the panel of [`options`]; **Cancel** (or Esc) stops an
//!   introduction, leaves a session, or leaves the screen, in that order.
//! - Tab and Shift+Tab walk the controls; every control also works by mouse.
//!
//! # The list (*agent decisions*)
//!
//! The master's order, with the games that cannot be joined (another
//! version, full, closing) after those that can, each group in the master's
//! order. A small mark at the row's end says the master expects the relay.
//! The list is asked for when the screen opens and every 15 seconds, the
//! selected game's details every 5.
use crate::direct_screen::{JoinRequest, Timing};
use crate::menu::{Canvas, text_width};
use crate::net::{
    browse::{Browse, News},
    hosting::Listing,
    options::{DEFAULT_CALLSIGN, callsign_problem},
    settings::Remembered,
    telemetry,
};
use crate::ui_text;
use crate::widgets::{
    Align, Backdrop, Background, Button, Cell, CheckBox, Column, Filter, Focus, Icon, Kit, List,
    MessageBox, Outcome as Wo, Pager, Point, Route, Row, TextField, Widget as _, draw_panel, fit,
    tone,
};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tore_net::master::{ListingSummary, PageEntry, browse::BrowseEvent};
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
    Full,
    Other,
    Games,
    Messages,
    New,
    Join,
    Refresh,
    Options,
    Cancel,
}

/// A join the game should start: the host's address is known (the master
/// gave it), with what the report on the session needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InternetJoin {
    /// The address, name, callsign and password, as Direct Connection's join.
    pub join: JoinRequest,
    /// From pressing Join to the master's answer, for the player's Report.
    pub asked: Duration,
    /// The master that introduced the player, for the player's Report.
    pub master: String,
    /// The install id to report under, when statistics are on and the
    /// one-time notice has been shown; `None` sends no report.
    pub install_id: Option<u64>,
}

/// A game the player asked to host, listed on the Internet Lobby. The game
/// builds it from the Quick Mission creator's mission until the lobby
/// screen's own mission choice (EF8) is used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostRequest {
    pub callsign: String,
    pub name: String,
    pub port: u16,
    pub password: Option<String>,
    /// The master, and the install id when statistics are on.
    pub listing: Listing,
}

/// What the game does after an event reaches the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing; the screen redraws every frame while it is up.
    None,
    /// Leave the screen, back to Choose Activity. The browse stops with it.
    Close,
    Join(Box<InternetJoin>),
    Host(Box<HostRequest>),
    /// Leave the game session this screen started (Cancel while joined).
    EndSession,
}

/// A join under way: the master has been asked to introduce the player.
#[derive(Clone, Debug)]
struct Joining {
    listing_id: u64,
    name: String,
    asked: Duration,
}

/// The screen.
pub struct InternetScreen {
    kit: Arc<Kit>,
    background: Background,
    callsign: TextField,
    full: CheckBox,
    other: CheckBox,
    games: List,
    players: List,
    messages: MessageBox,
    new: Button,
    join: Button,
    refresh: Button,
    options_button: Button,
    cancel: Button,
    focus: Focus<Id>,
    settings: Remembered,
    /// Where the settings are kept; `None` keeps nothing (tests, previews).
    data: Option<PathBuf>,
    /// The password to send or host with, from Options. Never stored.
    password: String,
    panel: Option<OptionsPanel>,
    /// The games the master listed, before the list filters them.
    entries: Vec<PageEntry>,
    /// What the master said about the games asked about, by listing.
    details: BTreeMap<u64, ListingSummary>,
    browse: Option<Browse>,
    /// The master the browse runs on, to tell when Options changed it.
    browsed: String,
    /// A browse that could not start is not tried again until something
    /// changes (another master, a session that ended).
    browse_failed: bool,
    /// The count of games last said in Messages, and whether the master's
    /// silence or failed lookup was.
    said_count: Option<usize>,
    said_trouble: bool,
    joining: Option<Joining>,
    /// A session this screen's join or host started is running.
    session: bool,
    /// The browse's clock.
    clock: tore_net::RealClock,
    pointer: Option<Point>,
    /// The backdrop, drawn the first time the screen is.
    backdrop: std::cell::RefCell<Option<Backdrop>>,
    /// The screen's own frame cost, logged every 300 frames when
    /// `TORE_DIRECT_TIMING` is set.
    timing: Option<Timing>,
}

impl InternetScreen {
    /// A screen on `kit` (in `MODEM3`'s palette). `data` is where the
    /// remembered settings are read from and written to.
    pub fn new(kit: Arc<Kit>, data: Option<PathBuf>) -> Self {
        let settings = data.as_deref().map(Remembered::load).unwrap_or_default();
        let games_columns = vec![
            Column {
                x: 0,
                width: 12,
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
                width: 36,
                align: Align::Left,
            },
            Column {
                x: 164,
                width: 12,
                align: Align::Centre,
            },
        ];
        let players_columns = vec![
            Column {
                x: 0,
                width: 12,
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
        let mut full = CheckBox::new((330, 106), "Show full games", settings.show_full);
        full.set_checked(settings.show_full);
        let mut other = CheckBox::new((330, 134), "Show other versions", settings.show_other);
        other.set_checked(settings.show_other);
        let mut focus = Focus::new(
            vec![
                Id::Callsign,
                Id::Full,
                Id::Other,
                Id::Games,
                Id::Messages,
                Id::New,
                Id::Join,
                Id::Refresh,
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
            background: Background::internet_lobby(),
            callsign,
            full,
            other,
            games: List::new((48, 185), 200, 4)
                .with_pager(Pager::NEWNET)
                .with_columns(games_columns),
            players: List::new((346, 185), 242, 5).with_columns(players_columns),
            messages: MessageBox::newnet(),
            new: Button::new("New", (45, 419), 85).default_button(),
            join: Button::new("Join", (161, 419), 85),
            refresh: Button::new("Refresh", (277, 419), 85),
            options_button: Button::new("Options", (393, 419), 85),
            cancel: Button::new("Cancel", (509, 419), 85),
            focus,
            settings,
            data,
            password: String::new(),
            panel: None,
            entries: Vec::new(),
            details: BTreeMap::new(),
            browse: None,
            browsed: String::new(),
            browse_failed: false,
            said_count: None,
            said_trouble: false,
            joining: None,
            session: false,
            clock: tore_net::RealClock::new(),
            pointer: None,
            backdrop: Default::default(),
            timing: std::env::var_os("TORE_DIRECT_TIMING").map(|_| Timing::new("Internet Lobby")),
        };
        screen.notice();
        screen
    }

    /// The one-time line about the statistics, the first time the screen
    /// opens while they are on. Written down so it is not said again.
    fn notice(&mut self) {
        if self.settings.telemetry && !self.settings.telemetry_notice {
            self.say(telemetry::NOTICE);
            self.settings.telemetry_notice = true;
            self.save();
        }
    }

    // ---- state the game reads ----

    /// A browse is running.
    #[cfg(test)]
    pub fn browsing(&self) -> bool {
        self.browse.is_some()
    }
    /// A join is waiting for the master's introduction.
    #[cfg(test)]
    pub fn joining(&self) -> bool {
        self.joining.is_some()
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
    /// The names of the games in the list, in order, for the tests.
    #[cfg(test)]
    pub fn listed_names(&self) -> Vec<String> {
        self.games
            .rows()
            .iter()
            .filter_map(|row| match row.cells.get(1) {
                Some(Cell::Text(text)) => Some(text.clone()),
                _ => None,
            })
            .collect()
    }
    /// True when text typed now goes to a field.
    pub fn typing(&self) -> bool {
        match &self.panel {
            Some(panel) => panel.typing(),
            None => !self.busy() && matches!(self.focus.current(), Some(Id::Callsign)),
        }
    }

    /// Something is going on that the player may stop: an introduction, or
    /// a session.
    fn busy(&self) -> bool {
        self.joining.is_some() || self.session
    }

    fn selected_entry(&self) -> Option<&PageEntry> {
        let key = &self.games.selected_row()?.key;
        self.entries
            .iter()
            .find(|entry| entry.listing_id.to_string() == *key)
    }

    fn join_is_default(&self) -> bool {
        !self.busy() && self.selected_entry().is_some()
    }

    /// The master as the player knows it: theirs from Options, or the
    /// built-in one.
    fn master_text(&self) -> String {
        self.settings
            .master
            .clone()
            .unwrap_or_else(|| tore_net::master::DEFAULT_MASTER.to_owned())
    }

    // ---- messages ----

    /// Adds a line to Messages in the game's own colour.
    pub fn say(&mut self, text: &str) {
        log::info!("Internet Lobby: {text}");
        self.messages.push(&self.kit, text, tone::SYSTEM);
    }

    // ---- the turn ----

    /// The screen's turn each frame: the browse started, stopped and read,
    /// the lists refreshed and the widgets animated. `session` says whether
    /// the game has a session open. Never blocks.
    pub fn update(&mut self, session: bool) -> Outcome {
        let now = self.clock.now();
        self.update_at(now, session)
    }

    /// [`InternetScreen::update`] at a given time on the browse's clock.
    pub fn update_at(&mut self, now: Duration, session: bool) -> Outcome {
        let started = self.timing.as_ref().map(|_| Instant::now());
        let outcome = self.turn(now, session);
        if let (Some(timing), Some(started)) = (&self.timing, started) {
            timing.update.set(timing.update.get() + started.elapsed());
        }
        outcome
    }

    fn turn(&mut self, now: Duration, session: bool) -> Outcome {
        if self.session && !session {
            // The session ended (the game said why in Messages).
            self.browse_failed = false;
        }
        self.session = session;
        self.sync_browse();
        let outcome = self.poll_browse(now);
        let at = Instant::now();
        self.games.advance(at);
        self.players.advance(at);
        self.full.advance(at);
        self.other.advance(at);
        if let Some(panel) = &mut self.panel {
            panel.advance();
        }
        self.refresh_buttons();
        outcome
    }

    /// The browse runs while the screen is open and no session is: started
    /// here, stopped by a session or another master.
    fn sync_browse(&mut self) {
        if self.session {
            if self.browse.take().is_some() {
                log::info!("Internet Lobby: browse stopped for the session");
                self.entries.clear();
                self.details.clear();
                self.rebuild_games();
            }
            return;
        }
        if self.browse.is_some() && self.browsed != self.master_text() {
            self.browse = None;
            self.entries.clear();
            self.details.clear();
            self.said_count = None;
            self.said_trouble = false;
            self.browse_failed = false;
            self.rebuild_games();
        }
        if self.browse.is_none() && !self.browse_failed && self.panel.is_none() {
            self.start_browse();
        }
    }

    /// Starts the browse on the master, saying in Messages what it asks.
    pub fn start_browse(&mut self) {
        let master = self.master_text();
        match Browse::start(&master, self.other.checked(), self.full.checked()) {
            Ok(browse) => {
                self.say(&format!(
                    "Asking the Internet Lobby at {} for games...",
                    browse.master_text()
                ));
                self.browsed = master;
                self.browse = Some(browse);
                self.said_count = None;
                self.said_trouble = false;
            }
            Err(error) => {
                self.browse_failed = true;
                self.say(&format!("Cannot ask the Internet Lobby: {error}"));
            }
        }
    }

    /// Takes the browse's news into the lists and Messages; an introduction
    /// that arrived is a join.
    fn poll_browse(&mut self, now: Duration) -> Outcome {
        let Some(browse) = &mut self.browse else {
            return Outcome::None;
        };
        browse.update(now);
        let mut news = Vec::new();
        while let Some(item) = browse.poll_news() {
            news.push(item);
        }
        let mut outcome = Outcome::None;
        let mut changed = false;
        for item in news {
            match item {
                News::Resolved(_) => {}
                News::CannotResolve(error) => {
                    if !self.said_trouble {
                        self.said_trouble = true;
                        let master = self
                            .browse
                            .as_ref()
                            .map(Browse::master_text)
                            .unwrap_or_default();
                        self.say(&format!(
                            "Cannot find the Internet Lobby at {master}: {error}. Direct Connection still works."
                        ));
                    }
                }
                News::Browse(event) => match event {
                    BrowseEvent::Added(_) | BrowseEvent::Changed(_) | BrowseEvent::Dropped(_) => {
                        changed = true
                    }
                    BrowseEvent::Refreshed { shown, .. } => {
                        changed = true;
                        self.said_trouble = false;
                        self.refreshed(shown);
                    }
                    BrowseEvent::Details {
                        listing_id,
                        summary,
                    } => {
                        match summary {
                            Some(summary) => {
                                self.details.insert(listing_id, summary);
                            }
                            None => {
                                self.details.remove(&listing_id);
                            }
                        }
                        self.rebuild_players();
                    }
                    BrowseEvent::Silent => {
                        if !self.said_trouble {
                            self.said_trouble = true;
                            let master = self
                                .browse
                                .as_ref()
                                .map(Browse::master_text)
                                .unwrap_or_default();
                            self.say(&format!(
                                "The Internet Lobby at {master} does not answer. Direct Connection still works."
                            ));
                        }
                    }
                    BrowseEvent::Unsupported { text } => {
                        self.say(&format!(
                            "The Internet Lobby does not take this game: {text}"
                        ));
                    }
                },
                News::Introduced {
                    listing_id,
                    address,
                } => {
                    if let Some(joining) = self.joining.take()
                        && joining.listing_id == listing_id
                    {
                        outcome = self.start_join(joining, address, now);
                    }
                }
                News::NotIntroduced { listing_id, text } => {
                    if self
                        .joining
                        .as_ref()
                        .is_some_and(|joining| joining.listing_id == listing_id)
                    {
                        self.joining = None;
                        self.say(&text);
                    }
                }
            }
        }
        if changed && let Some(browse) = &self.browse {
            self.entries = browse.games().to_vec();
            self.details
                .retain(|id, _| self.entries.iter().any(|e| e.listing_id == *id));
            self.rebuild_games();
        }
        outcome
    }

    /// A refresh ended with `shown` games: said when the count is new.
    fn refreshed(&mut self, shown: usize) {
        if self.said_count == Some(shown) {
            return;
        }
        self.said_count = Some(shown);
        let line = match shown {
            0 => "No games are listed on the Internet Lobby right now. New lists one of yours."
                .to_owned(),
            1 => "1 game is listed on the Internet Lobby.".to_owned(),
            n => format!("{n} games are listed on the Internet Lobby."),
        };
        self.say(&line);
    }

    fn refresh_buttons(&mut self) {
        let busy = self.busy();
        let join_default = self.join_is_default();
        self.join.set_default(join_default);
        self.new.set_default(!join_default);
        for button in [
            &mut self.new,
            &mut self.join,
            &mut self.refresh,
            &mut self.options_button,
        ] {
            button.set_enabled(!busy);
        }
        self.cancel
            .set_label(if self.session { "Leave" } else { "Cancel" });
        self.full.set_enabled(!busy);
        self.other.set_enabled(!busy);
        self.games.set_enabled(!busy);
    }

    // ---- the lists ----

    /// Puts the listed games in the list: full games only when "Show full
    /// games" is on and games of other versions only when "Show other
    /// versions" is, those that cannot be joined after those that can. The
    /// selection stays on its game.
    fn rebuild_games(&mut self) {
        let show_full = self.full.checked();
        let show_other = self.other.checked();
        let mut rows: Vec<(bool, Row)> = self
            .entries
            .iter()
            .filter(|entry| show_full || !entry.full || entry.other_build.is_some())
            .filter(|entry| show_other || entry.other_build.is_none())
            .map(|entry| (joinable(entry), game_row(entry)))
            .collect();
        // Stable: each group keeps the master's order.
        rows.sort_by_key(|(joinable, _)| !*joinable);
        self.games
            .set_rows(rows.into_iter().map(|(_, row)| row).collect());
        self.rebuild_players();
    }

    fn rebuild_players(&mut self) {
        let rows = match self.selected_entry().map(|e| e.listing_id) {
            None => Vec::new(),
            Some(id) => match self.details.get(&id) {
                None => Vec::new(),
                Some(summary) => {
                    let mut rows: Vec<Row> = summary
                        .callsigns
                        .iter()
                        .map(|name| {
                            let king = !summary.king.is_empty() && *name == summary.king;
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
                    let more = usize::from(summary.players).saturating_sub(rows.len());
                    if more > 0 {
                        rows.push(Row::text(format!("and {more} more")).dimmed());
                    }
                    rows
                }
            },
        };
        self.players.set_rows(rows);
    }

    /// The selected game's mission, or why it cannot be joined, for the line
    /// under the Games box.
    fn selection_line(&self) -> Option<String> {
        let entry = self.selected_entry()?;
        if let Some(version) = &entry.other_build {
            return Some(format!(
                "{} runs another version ({version}) and cannot be joined.",
                entry.name
            ));
        }
        let mission = self
            .details
            .get(&entry.listing_id)
            .map(|summary| format!("Mission: {}", summary.mission));
        match (mission, entry.relay_likely) {
            (Some(mission), true) => Some(format!("{mission} (may need the relay)")),
            (Some(mission), false) => Some(mission),
            (None, true) => Some("This game's router may need the relay.".into()),
            (None, false) => None,
        }
    }

    /// The player picked a game: its players show, its details are asked.
    fn game_selected(&mut self) {
        let id = self.selected_entry().map(|e| e.listing_id);
        let now = self.clock.now();
        if let Some(browse) = &mut self.browse {
            browse.watch(id, now);
        }
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

    /// Writes the callsign, the port, the check boxes and the Options' choices
    /// down, as far as they are valid.
    pub fn save(&mut self) {
        let text = self.callsign.text().to_owned();
        self.settings.remember_callsign(&text);
        self.settings.show_full = self.full.checked();
        self.settings.show_other = self.other.checked();
        if let Some(data) = &self.data
            && let Err(error) = self.settings.save(data)
        {
            log::warn!("Network settings not saved: {error}");
        }
    }

    /// The install id this game reports under, or none: statistics on, the
    /// notice shown, and a folder to keep the id in.
    fn install_id(&self) -> Option<u64> {
        let data = self.data.as_deref()?;
        telemetry::install_id(
            data,
            self.settings.telemetry && self.settings.telemetry_notice,
        )
    }

    fn press(&mut self, id: Id) -> Outcome {
        match id {
            Id::New => self.new_game(),
            Id::Join => self.join_game(),
            Id::Refresh => {
                self.ask_again();
                Outcome::None
            }
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

    /// Refresh: the list is asked for now. A browse that never started (the
    /// master's name could not be looked up, or the socket) starts again.
    fn ask_again(&mut self) {
        if self.busy() {
            return;
        }
        let now = self.clock.now();
        match &mut self.browse {
            Some(browse) => {
                browse.refresh(now);
                self.said_trouble = false;
                self.say("Asking the Internet Lobby for the list...");
            }
            None => {
                self.browse_failed = false;
                self.start_browse();
            }
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
        let listing = Listing {
            master: self.master_text(),
            listed: true,
            install_id: self.install_id(),
        };
        Outcome::Host(Box::new(HostRequest {
            callsign,
            name,
            port: self.settings.port,
            password: Some(self.password.clone()).filter(|p| !p.is_empty()),
            listing,
        }))
    }

    fn join_game(&mut self) -> Outcome {
        if self.busy() {
            return Outcome::None;
        }
        if self.checked_callsign().is_none() {
            return Outcome::None;
        }
        let Some(entry) = self.selected_entry() else {
            self.say("Pick a game in the list first.");
            self.focus.set(Id::Games);
            return Outcome::None;
        };
        let (name, listing_id) = (entry.name.clone(), entry.listing_id);
        let problem = if let Some(version) = &entry.other_build {
            Some(format!(
                "{name} runs another version ({version}); it cannot be joined."
            ))
        } else if entry.phase == DiscoverPhase::Closed {
            Some(format!("{name} is closing; try again soon."))
        } else if entry.full {
            Some(format!("{name} is full."))
        } else {
            None
        };
        if let Some(problem) = problem {
            self.say(&problem);
            return Outcome::None;
        }
        let Some(browse) = &mut self.browse else {
            self.say(
                "The Internet Lobby cannot be reached, so there is nobody to ask. Try Refresh.",
            );
            return Outcome::None;
        };
        let now = self.clock.now();
        browse.introduce(listing_id, now);
        self.say(&format!(
            "Asking the Internet Lobby to introduce you to '{name}'..."
        ));
        self.save();
        self.joining = Some(Joining {
            listing_id,
            name,
            asked: now,
        });
        Outcome::None
    }

    /// The master gave the host's address: the join starts.
    fn start_join(&mut self, joining: Joining, address: SocketAddr, now: Duration) -> Outcome {
        let callsign = self.callsign.text().to_owned();
        self.say(&format!(
            "Attempting connection to '{}' at {address}...",
            joining.name
        ));
        // The join takes over from the browse.
        self.browse = None;
        Outcome::Join(Box::new(InternetJoin {
            join: JoinRequest {
                address,
                label: joining.name,
                callsign,
                password: self.password.clone(),
            },
            asked: now.saturating_sub(joining.asked),
            master: self.master_text(),
            install_id: self.install_id(),
        }))
    }

    fn cancel_or_leave(&mut self) -> Outcome {
        if self.joining.take().is_some() {
            if let Some(browse) = &mut self.browse {
                browse.cancel_introduction();
            }
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
            &self.settings,
            &self.password,
            &default_name,
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
                self.settings.master = values.master;
                self.settings.port_forward = values.port_forward;
                let was_on = self.settings.telemetry;
                self.settings.telemetry = values.telemetry;
                if !values.telemetry {
                    // Off deletes the id; on again draws a new one.
                    if let Some(data) = &self.data {
                        telemetry::install_id(data, false);
                    }
                    self.settings.telemetry_notice = false;
                } else if !was_on {
                    // Turned back on by the player: they know.
                    self.settings.telemetry_notice = true;
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
        if name == "F5" {
            self.ask_again();
            return Outcome::None;
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
            Route::Widget(Id::Callsign | Id::Full | Id::Other | Id::Games) if busy => Outcome::None,
            Route::Widget(id) => match id {
                Id::Callsign => match self.callsign.key(name) {
                    Wo::Activated => self.press_default(),
                    _ => Outcome::None,
                },
                Id::Full => {
                    if self.full.key(name, now) == Wo::Changed {
                        self.filters_changed();
                    }
                    Outcome::None
                }
                Id::Other => {
                    if self.other.key(name, now) == Wo::Changed {
                        self.filters_changed();
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
                Id::New | Id::Join | Id::Refresh | Id::Options | Id::Cancel => {
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

    /// A check box turned: the list is filtered again, the choice saved and
    /// the master asked with the new filters.
    fn filters_changed(&mut self) {
        self.rebuild_games();
        self.save();
        let now = self.clock.now();
        let (other, full) = (self.other.checked(), self.full.checked());
        if let Some(browse) = &mut self.browse {
            browse.set_filters(other, full, now);
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
        if let Some(Id::Callsign) = self.focus.current() {
            self.callsign.text_input(text);
        }
    }

    fn button_mut(&mut self, id: Id) -> &mut Button {
        match id {
            Id::New => &mut self.new,
            Id::Join => &mut self.join,
            Id::Refresh => &mut self.refresh,
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
            &mut self.refresh,
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
                self.cancel_press();
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
                if self.full.hit(p) {
                    self.focus.set(Id::Full);
                    self.full.press(p);
                }
                if self.other.hit(p) {
                    self.focus.set(Id::Other);
                    self.other.press(p);
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
                &mut self.refresh,
                &mut self.options_button,
                &mut self.cancel,
            ] {
                button.press(p);
            }
            return Outcome::None;
        }
        // Released.
        self.games.release(now);
        if self.full.release(p, now) == Wo::Changed || self.other.release(p, now) == Wo::Changed {
            self.filters_changed();
        }
        let mut fired = None;
        for (id, button) in [
            (Id::New, &mut self.new),
            (Id::Join, &mut self.join),
            (Id::Refresh, &mut self.refresh),
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

    /// Lets go of anything held (the window lost focus or was resized).
    pub fn cancel_press(&mut self) {
        let off = (-1, -1);
        let now = Instant::now();
        self.games.release(now);
        self.full.release(off, now);
        self.other.release(off, now);
        for button in [
            &mut self.new,
            &mut self.join,
            &mut self.refresh,
            &mut self.options_button,
            &mut self.cancel,
        ] {
            button.release(off);
            button.pointer_move(None);
        }
        self.pointer = None;
    }

    /// A wheel step over the pointer's place: the list, the players or the
    /// messages. Positive is up.
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
        } else if !self.busy() && self.games.hit(p) && self.games.wheel(notches) == Wo::Changed {
            self.game_selected();
        }
    }

    // ---- drawing ----

    /// The part of the screen that never changes: the composed background,
    /// the panel, the title, the frame lines, the headings and the boxes the
    /// lists sit in. Drawn once and kept (as Direct Connection's is).
    fn draw_backdrop(&self, canvas: &mut Canvas) {
        let kit = &*self.kit;
        self.background.draw(canvas, kit);
        draw_panel(canvas, kit, (10, 80, 619, 395));
        let font = kit.sprite("PANELFNT");
        let title = "Internet Lobby";
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
            ("Callsign:", 45, 110),
            ("Games", 45, 165),
            ("Players", 340, 165),
            ("Messages", 45, 304),
        ] {
            ui_text::text(canvas, kit, font, label, (x, y), None, None);
        }
        canvas.outline((45, 180, 269, 105), LINE);
        canvas.rect((341, 181, 252, 103), [81, 81, 81, 255]);
        canvas.outline((340, 180, 254, 105), LINE);
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
        // Which master the list comes from, on the line under the callsign.
        let master = format!("Games listed by {}", self.master_text());
        let master = fit(dim, &master, 270);
        ui_text::text(canvas, kit, dim, &master, (45, 139), None, None);
        if let Some(line) = self.selection_line() {
            let line = fit(font, &line, 549);
            ui_text::text(canvas, kit, font, &line, (45, 290), None, None);
        }
        let marked = |id| self.focus.marked(id);
        self.callsign.draw(canvas, kit, self.focus.is(Id::Callsign));
        self.full.draw(canvas, kit, marked(Id::Full));
        self.other.draw(canvas, kit, marked(Id::Other));
        self.players.draw(canvas, kit, false);
        self.games.draw(canvas, kit, marked(Id::Games));
        self.messages.draw(canvas, kit, marked(Id::Messages));
        self.new.draw(canvas, kit, marked(Id::New));
        self.join.draw(canvas, kit, marked(Id::Join));
        self.refresh.draw(canvas, kit, marked(Id::Refresh));
        self.options_button.draw(canvas, kit, marked(Id::Options));
        self.cancel.draw(canvas, kit, marked(Id::Cancel));
        if let Some(panel) = &self.panel {
            panel.draw(canvas, kit);
        }
    }
}

/// A game the player can join: this version, with room, not closing.
fn joinable(entry: &PageEntry) -> bool {
    entry.other_build.is_none() && !entry.full && entry.phase != DiscoverPhase::Closed
}

/// One listed game as a row of the Games list: the lock, the name, players
/// and capacity, lobby or flying, and a mark when the master expects the
/// relay (a game of another version shows its version instead and is dimmed,
/// like a full or closing one).
fn game_row(entry: &PageEntry) -> Row {
    let state = match &entry.other_build {
        None => match entry.phase {
            DiscoverPhase::Lobby => "Lobby".to_owned(),
            DiscoverPhase::Flying => "Flying".to_owned(),
            DiscoverPhase::Closed => "Closed".to_owned(),
        },
        Some(version) => format!("v{version}"),
    };
    let row = Row::new(
        entry.listing_id.to_string(),
        vec![
            if entry.password {
                Cell::Icon(Icon::Lock)
            } else {
                Cell::Empty
            },
            Cell::Text(entry.name.clone()),
            Cell::Text(format!("{}/{}", entry.players, entry.capacity)),
            Cell::Text(state),
            if entry.relay_likely {
                Cell::Text("R".to_owned())
            } else {
                Cell::Empty
            },
        ],
    );
    if joinable(entry) { row } else { row.dimmed() }
}
