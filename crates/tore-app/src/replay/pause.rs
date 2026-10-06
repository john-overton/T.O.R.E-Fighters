//! The replay viewer's Escape menu: flight's paused menu (`pause_menu.rs`)
//! over a tree built from the imported `FMENUD.MNU` rows that mean
//! something in a replay (`?`, Control and Pref), plus four authored rows. Opening it pauses
//! playback; closing it plays on as before. Opinionated addition requested
//! by John on 2026-09-28; the authored row labels, the title and the help
//! text are agent choices (2026-09-28). See docs/REPLAYS.md.
use crate::pause_menu::{Event, Look, PauseMenu};
use crate::replay::clock::Clock;
use crate::replay::viewer::Ui;
use tore_formats::ui::MenuNode;

/// The `?` row that returns to the Replays screen, in place of retail End
/// mission. Label chosen by John on 2026-09-28.
pub const END_REPLAY: &str = "End Replay";
/// The same row in the live view of a mission being flown (an observer's):
/// it stops the watch and returns to the lobby.
pub const STOP_WATCHING: &str = "Stop Watching";
/// The away player's rows (slice F2-O4, John 2026-10-06), in place of Stop
/// Watching: back into the player's own reserved aircraft, a new aircraft
/// when none is left, and out of the game.
pub const TAKE_BACK_FLIGHT: &str = "Take Back Flight";
pub const SPAWN: &str = "Spawn in Aircraft";
pub const LEAVE_GAME: &str = "Leave Game";
/// What the Leave Game row says beside it for a host that has no ready
/// standby, and once the first choice has asked to confirm.
pub const ENDS_THE_GAME: &str = "Ends the game for everyone";
pub const CONFIRM_LEAVE: &str = "Choose again to end it";
/// The retail `?` row that quits the game, shown as Exit to Desktop.
pub const EXIT: &str = "Exit to Windows";
/// Authored Pref rows: the same switches as the N, T, R and C keys.
pub const NAME_LABELS: &str = "Name labels?";
pub const MISSION_TIMER: &str = "Mission timer?";
pub const TRAILS: &str = "Flight path trails?";
pub const COMMS: &str = "Comms panel?";
/// The retail tab that opens the controls screen, as it does in flight.
pub const CONTROL: &str = "Control";
/// The retail Pref rows a replay keeps, in retail order.
const PREF_ROWS: [&str; 3] = ["Graphics...", "Sound...", "Time"];
/// The retail Time rows a replay keeps, in retail order.
const TIME_ROWS: [&str; 6] = ["Paused", "Slow-motion", "1x", "2x", "4x", "8x"];

/// The bottom buttons, left to right.
const BUTTONS: [&str; 2] = ["Resume replay", "Keyboard shortcuts"];

/// How the replay presents the shared menu.
pub const LOOK: Look = Look {
    title: "REPLAY PAUSED",
    buttons: &BUTTONS,
    help: help_lines,
};

/// The replay keyboard help, from the key table in docs/REPLAYS.md.
fn help_lines(_: &[MenuNode]) -> Vec<String> {
    [
        "REPLAY KEYBOARD COMMANDS - Esc returns",
        "Space: play or pause",
        "J / K / L: backwards / pause / forwards; again for twice the speed",
        "Up / Down: next faster / slower speed in the same direction",
        "Left / Right: 5 seconds back / forward; one tick while paused",
        "Shift-Left / Shift-Right: 30 seconds back / forward",
        "Home / End: start / end | PageUp / PageDown: previous / next marker",
        "Tab / Shift-Tab: next / previous aircraft",
        "F1 to F10, F12: the flight views, on the selected aircraft",
        "Alt / Ctrl + view: from its target / its newest missile",
        "O / Shift-O: object view, look at the next / previous object",
        "Keypad 5 or Shift-/: recenter | + / -: zoom",
        "Backquote: follow drone, then free drone, then back",
        "W A S D, E / Q: move the drone, climb / descend; Shift 4x faster",
        "Mouse wheel: scroll a panel or menu; drone speed or zoom",
        "Right-drag: look around, or turn the drone",
        "N: name labels | T: mission timer | C: Comms panel",
        "I / F / G: AI thinking / telemetry / missile guidance panels",
        "M: right-click menu on the selected aircraft | X: close every panel",
        "Right-click: the menu on the object under the pointer",
        "R: trails on or off | Shift-R: next trail length",
        "H: hide or show the interface | P: save the view as a PNG",
        "Esc: this menu; Esc again resumes | Alt-F4: exit to the desktop",
        "The keys are built in; docs/REPLAYS.md lists them all",
    ]
    .map(String::from)
    .to_vec()
}

fn row(label: &str, shortcut: &str) -> MenuNode {
    MenuNode {
        label: label.into(),
        shortcut: shortcut.into(),
        children: vec![],
    }
}

/// The replay's menu tree from the imported flight menu: its `?` tab with
/// End Replay and Exit to Windows, its Control tab, which opens the
/// controls screen instead of showing rows, and its Pref tab with Graphics...,
/// Sound... and Time (Paused to 8x), then the four authored rows. Retail
/// labels come from `imported`; any it lacks are authored, so an empty
/// tree gives the whole menu. Shortcuts are dropped except Alt-F4, since a
/// replay's keys are its own.
pub fn tree(imported: &[MenuNode]) -> Vec<MenuNode> {
    let tab = |label: &str| imported.iter().find(|n| n.label == label);
    let kept = |node: Option<&MenuNode>, wanted: &[&str]| -> Vec<MenuNode> {
        wanted
            .iter()
            .map(|label| {
                node.and_then(|n| n.children.iter().find(|c| c.label == *label))
                    .map_or_else(
                        || row(label, ""),
                        |c| MenuNode {
                            shortcut: String::new(),
                            children: Vec::new(),
                            ..c.clone()
                        },
                    )
            })
            .collect()
    };
    let question = tab("?");
    let exit = question
        .and_then(|n| n.children.iter().find(|c| c.label == EXIT))
        .map_or_else(|| row(EXIT, "Alt-F4"), MenuNode::clone);
    let pref = tab("Pref");
    let mut prefs = kept(pref, &PREF_ROWS);
    let time = pref.and_then(|n| n.children.iter().find(|c| c.label == "Time"));
    if let Some(node) = prefs.iter_mut().find(|n| n.label == "Time") {
        node.children = kept(time, &TIME_ROWS);
    }
    prefs.extend([NAME_LABELS, MISSION_TIMER, TRAILS, COMMS].map(|label| row(label, "")));
    vec![
        MenuNode {
            label: question.map_or("?", |n| &n.label).into(),
            shortcut: String::new(),
            children: vec![row(END_REPLAY, ""), exit],
        },
        MenuNode {
            label: tab(CONTROL).map_or(CONTROL, |n| &n.label).into(),
            shortcut: String::new(),
            children: Vec::new(),
        },
        MenuNode {
            label: pref.map_or("Pref", |n| &n.label).into(),
            shortcut: String::new(),
            children: prefs,
        },
    ]
}

/// The On/Off readout of an authored row, from the interface switches.
pub fn state(ui: &Ui, label: &str) -> Option<&'static str> {
    let on = match label {
        NAME_LABELS => ui.labels,
        MISSION_TIMER => ui.timer,
        TRAILS => ui.trails,
        COMMS => ui.comms,
        _ => return None,
    };
    Some(if on { "On" } else { "Off" })
}

/// Where `--replay-menu` opens the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// The `?` tab.
    Question,
    Pref,
    /// Pref > Time.
    Time,
    /// The keyboard help.
    Help,
    /// Pref > Graphics..., with the app opening that screen.
    Graphics,
    /// Pref > Sound..., with the app opening that screen.
    Sound,
    /// The Control tab, with the app opening the controls screen.
    Controls,
}

impl Start {
    pub fn parse(name: &str) -> Result<Self, String> {
        Ok(match name {
            "?" => Self::Question,
            "pref" => Self::Pref,
            "time" => Self::Time,
            "help" => Self::Help,
            "graphics" => Self::Graphics,
            "sound" => Self::Sound,
            "controls" => Self::Controls,
            other => {
                return Err(format!(
                    "unknown replay menu page {other:?}: use ?, pref, time, help, graphics, sound or controls"
                ));
            }
        })
    }
}

/// A replay interface part an authored row switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Labels,
    Timer,
    Trails,
    Comms,
}

/// What the away player's observer menu offers now (slice F2-O4), from the
/// game's state: it is rebuilt when this changes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AwayRows {
    /// The player's own reserved aircraft is alive and the AI flies it: the
    /// first row is Take Back Flight. Else no aircraft is left (the AI lost
    /// it, or the King released it) and the first row is Spawn in Aircraft.
    pub own_aircraft: bool,
    /// Why Spawn in Aircraft is dimmed, when the revival rules allow nothing
    /// now (no revival, no lives left, the delay not over, no room).
    pub spawn_blocked: Option<String>,
    /// Leaving ends the game for everyone: the player hosts it and no
    /// standby is ready to take it over. The row asks to be chosen twice.
    pub leave_ends_game: bool,
}

impl AwayRows {
    /// The rows, with the words beside them. A dimmed row's reason is its
    /// readout.
    pub fn rows(&self, confirming: bool) -> Vec<MenuNode> {
        let mut rows = Vec::new();
        if self.own_aircraft {
            rows.push(row(TAKE_BACK_FLIGHT, ""));
        } else {
            rows.push(row(SPAWN, self.spawn_blocked.as_deref().unwrap_or("")));
        }
        rows.push(row(
            LEAVE_GAME,
            match (self.leave_ends_game, confirming) {
                (false, _) => "",
                (true, false) => ENDS_THE_GAME,
                (true, true) => CONFIRM_LEAVE,
            },
        ));
        rows
    }

    /// The labels drawn dim: Spawn in Aircraft when the rules allow nothing.
    pub fn dimmed(&self) -> Vec<String> {
        if !self.own_aircraft && self.spawn_blocked.is_some() {
            vec![SPAWN.to_owned()]
        } else {
            Vec::new()
        }
    }
}

/// What a key or click in the menu asks of the viewer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    None,
    /// The menu moved: a click sound.
    Click,
    /// The away player takes its own aircraft back (slice F2-O4).
    TakeBack,
    /// The away player flies again in a new aircraft, by the revival rules.
    Spawn,
    /// Spawn in Aircraft was chosen while dimmed: the reason is said.
    SpawnBlocked,
    /// The away player leaves the game.
    LeaveGame,
    /// Back to the Replays screen, or from a live view to the lobby.
    Leave,
    /// Quit the game.
    Exit,
    Graphics,
    Sound,
    /// Open the controls screen.
    Controls,
    /// Switch an interface part.
    Toggle(Part),
}

/// The open menu and the play state it paused.
pub struct Menu {
    pub widget: PauseMenu,
    pub tree: Vec<MenuNode>,
    /// While open: whether playback goes on when it closes.
    open: Option<bool>,
    /// The playhead was at the live edge when the menu opened, and returns
    /// there when it closes (a live view's paused time is not worth keeping).
    was_live: bool,
    /// The away player's rows, when this is the menu of its own aircraft's
    /// observer screen (slice F2-O4).
    away: Option<AwayRows>,
    /// Leave Game was chosen once and waits for a second choice.
    confirming: bool,
}

impl Menu {
    pub fn new(imported: &[MenuNode]) -> Self {
        Self {
            widget: PauseMenu::default(),
            tree: tree(imported),
            open: None,
            was_live: false,
            away: None,
            confirming: false,
        }
    }

    /// The menu of a live view: End Replay reads Stop Watching.
    pub fn watching(imported: &[MenuNode]) -> Self {
        let mut menu = Self::new(imported);
        for node in menu.tree.iter_mut().flat_map(|tab| tab.children.iter_mut()) {
            if node.label == END_REPLAY {
                node.label = STOP_WATCHING.into();
            }
        }
        menu
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The menu of the observer screen of an away player's own aircraft:
    /// the first rows are `rows`' in place of Stop Watching. Called each
    /// frame; the tree changes only when the rows do, and the focus stays
    /// on its row.
    pub fn set_away(&mut self, rows: Option<AwayRows>) {
        if self.away == rows {
            return;
        }
        let on_leave = rows.as_ref().is_some_and(|r| !r.leave_ends_game);
        if on_leave {
            self.confirming = false;
        }
        self.away = rows;
        let Some(tab) = self.tree.first_mut() else {
            return;
        };
        // The first tab holds the away rows, then the retail rows after
        // End Replay / Stop Watching, which they replace.
        let rest: Vec<MenuNode> = tab
            .children
            .iter()
            .filter(|n| {
                !matches!(
                    n.label.as_str(),
                    END_REPLAY | STOP_WATCHING | TAKE_BACK_FLIGHT | SPAWN | LEAVE_GAME
                )
            })
            .cloned()
            .collect();
        tab.children = match &self.away {
            Some(away) => {
                let mut rows = away.rows(self.confirming);
                rows.extend(rest);
                rows
            }
            None => {
                let mut rows = vec![row(STOP_WATCHING, "")];
                rows.extend(rest);
                rows
            }
        };
        self.widget.dim = self.away.as_ref().map(AwayRows::dimmed).unwrap_or_default();
        let len = tab.children.len();
        if self.widget.root == 0 && self.widget.path.is_empty() && self.widget.focus >= len {
            self.widget.focus = 0;
        }
    }

    /// The away rows now, if this menu has them.
    pub fn away(&self) -> Option<&AwayRows> {
        self.away.as_ref()
    }

    /// Opens the menu over the view and pauses playback, noting whether it
    /// was playing.
    pub fn open(&mut self, clock: &mut Clock) {
        if self.open.is_none() {
            self.was_live = clock.following();
            self.open = Some(!clock.paused());
            clock.pause();
        }
    }

    /// Opens the menu at `start`, for captures.
    pub fn open_at(&mut self, start: Start, clock: &mut Clock) {
        self.open(clock);
        let pref = self.tree.len() - 1;
        debug_assert!(self.is_control(pref - 1));
        match start {
            Start::Question => self.widget.show_tab(0),
            Start::Help => self.widget.open_help(),
            Start::Controls => self.widget.show_tab(0),
            Start::Pref | Start::Time | Start::Graphics | Start::Sound => {
                self.widget.show_tab(pref);
                let row = match start {
                    Start::Time => "Time",
                    Start::Sound => "Sound...",
                    _ => "Graphics...",
                };
                if let Some(at) = self.tree[pref].children.iter().position(|n| n.label == row) {
                    self.widget.focus_row(&self.tree, at);
                    if start == Start::Time {
                        self.widget.enter(at);
                    }
                }
            }
        }
    }

    /// Forgets a first choice of Leave Game that asked to confirm.
    fn disarm(&mut self) {
        if std::mem::take(&mut self.confirming) {
            let rows = self.away.take();
            self.set_away(rows);
        }
    }

    /// Closes the menu; playback goes on in the speed and direction it had,
    /// or stays paused if it was.
    pub fn close(&mut self, clock: &mut Clock) {
        self.disarm();
        if let Some(play) = self.open.take()
            && play
            && clock.paused()
        {
            clock.toggle();
            if std::mem::take(&mut self.was_live) {
                clock.end();
            }
        }
    }

    /// A key pressed while the menu is open.
    pub fn key(&mut self, name: &str, clock: &mut Clock) -> Choice {
        if name == "Escape" {
            if !self.widget.back() {
                self.close(clock);
            }
            return Choice::None;
        }
        let before = self.widget.root;
        match self.widget.key(name, &self.tree) {
            Event::Select(index) => self.select(index, clock),
            Event::Switched(index) if self.is_control(index) => {
                // The arrows step past Control to the next tab the same
                // way, so the menu is there when the controls screen closes
                // and the next arrow does not open it again.
                let n = self.tree.len();
                let right = index == (before + 1) % n;
                self.widget.show_tab(if right {
                    (index + 1) % n
                } else {
                    (index + n - 1) % n
                });
                Choice::Controls
            }
            _ => Choice::None,
        }
    }

    /// The left button went down or up at `point` in the 640x480 layer.
    pub fn pointer(&mut self, point: Option<(f64, f64)>, down: bool, clock: &mut Clock) -> Choice {
        match self.widget.pointer(&self.tree, &LOOK, point, down) {
            Event::None | Event::Switched(_) => Choice::None,
            Event::Click => Choice::Click,
            Event::Tab(index) if self.is_control(index) => Choice::Controls,
            Event::Tab(index) => {
                self.widget.show_tab(index);
                Choice::Click
            }
            Event::Select(index) => self.select(index, clock),
            Event::Button(index) => self.activate(BUTTONS[index], clock),
        }
    }

    /// Whether tab `index` is Control, which opens the controls screen.
    fn is_control(&self, index: usize) -> bool {
        self.tree
            .get(index)
            .is_some_and(|tab| tab.children.is_empty())
    }

    fn select(&mut self, index: usize, clock: &mut Clock) -> Choice {
        let Some(node) = self.widget.focus_row(&self.tree, index) else {
            return Choice::None;
        };
        if node.children.is_empty() {
            let label = node.label.clone();
            self.activate(&label, clock)
        } else {
            self.widget.enter(index);
            Choice::Click
        }
    }

    /// What a leaf row or bottom button does. The Time rows set the clock
    /// as flight's set the time scale: the menu stays open and playback
    /// runs at that speed, forwards, once it closes; Paused closes the menu
    /// and leaves the replay paused if it was playing, playing if it was
    /// paused.
    pub fn activate(&mut self, label: &str, clock: &mut Clock) -> Choice {
        let speed = match label {
            "Slow-motion" => Some(0.5),
            "1x" => Some(1.),
            "2x" => Some(2.),
            "4x" => Some(4.),
            "8x" => Some(8.),
            _ => None,
        };
        if let Some(speed) = speed {
            clock.set_speed(speed);
            clock.pause();
            if let Some(play) = &mut self.open {
                *play = true;
            }
            return Choice::Click;
        }
        match label {
            "Resume replay" => {
                self.close(clock);
                Choice::Click
            }
            "Keyboard shortcuts" => {
                self.widget.open_help();
                Choice::Click
            }
            "Paused" => {
                if let Some(play) = &mut self.open {
                    *play = !*play;
                }
                self.close(clock);
                Choice::Click
            }
            END_REPLAY | STOP_WATCHING => Choice::Leave,
            TAKE_BACK_FLIGHT => {
                self.close(clock);
                Choice::TakeBack
            }
            SPAWN
                if self
                    .away
                    .as_ref()
                    .is_some_and(|a| a.spawn_blocked.is_some()) =>
            {
                Choice::SpawnBlocked
            }
            SPAWN => {
                self.close(clock);
                Choice::Spawn
            }
            LEAVE_GAME
                if self.away.as_ref().is_some_and(|a| a.leave_ends_game) && !self.confirming =>
            {
                self.confirming = true;
                let rows = self.away.take();
                self.set_away(rows);
                Choice::Click
            }
            LEAVE_GAME => Choice::LeaveGame,
            EXIT => Choice::Exit,
            "Graphics..." => Choice::Graphics,
            "Sound..." => Choice::Sound,
            NAME_LABELS => Choice::Toggle(Part::Labels),
            MISSION_TIMER => Choice::Toggle(Part::Timer),
            TRAILS => Choice::Toggle(Part::Trails),
            COMMS => Choice::Toggle(Part::Comms),
            _ => Choice::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(label: &str, shortcut: &str, children: Vec<MenuNode>) -> MenuNode {
        MenuNode {
            label: label.into(),
            shortcut: shortcut.into(),
            children,
        }
    }

    #[test]
    fn the_tree_keeps_the_imported_rows_a_replay_uses() {
        let imported = vec![
            node(
                "?",
                "",
                vec![
                    row("End mission", "Ctrl-Q"),
                    row("Exit to Windows", "Alt-F4"),
                ],
            ),
            node("Control", "", vec![row("Keyboard", "")]),
            node(
                "Pref",
                "",
                vec![
                    row("Graphics...", ""),
                    row("Sound...", ""),
                    node(
                        "Time",
                        "",
                        vec![
                            row("Paused", "Ctrl-P"),
                            row("Slow-motion", "C"),
                            row("1x", "C"),
                            row("2x", "C"),
                            row("4x", "C"),
                            row("8x", "C"),
                        ],
                    ),
                    row("Accelerated time?", ""),
                    row("Dim HUD", ""),
                    row("Show cockpit?", ""),
                ],
            ),
            node("View", "", vec![row("Front", "F1")]),
        ];
        let t = tree(&imported);
        let labels = |rows: &[MenuNode]| -> Vec<String> {
            rows.iter()
                .map(|n| format!("{}|{}", n.label, n.shortcut))
                .collect()
        };
        assert_eq!(
            labels(&t),
            ["?|", "Control|", "Pref|"].map(String::from).to_vec(),
            "only these three tabs, in retail order"
        );
        assert!(
            t[1].children.is_empty(),
            "Control opens the controls screen"
        );
        assert_eq!(
            labels(&t[0].children),
            ["End Replay|", "Exit to Windows|Alt-F4"]
                .map(String::from)
                .to_vec()
        );
        assert_eq!(
            labels(&t[2].children),
            [
                "Graphics...|",
                "Sound...|",
                "Time|",
                "Name labels?|",
                "Mission timer?|",
                "Flight path trails?|",
                "Comms panel?|"
            ]
            .map(String::from)
            .to_vec()
        );
        assert_eq!(
            labels(&t[2].children[2].children),
            ["Paused|", "Slow-motion|", "1x|", "2x|", "4x|", "8x|"]
                .map(String::from)
                .to_vec()
        );
        // Without imported data the same menu is authored.
        assert_eq!(format!("{:?}", tree(&[])), format!("{t:?}"));
    }

    #[test]
    fn control_opens_the_controls_screen_and_the_arrows_step_past_it() {
        let mut clock = Clock::new(0, 1_000);
        let mut m = Menu::new(&[]);
        m.open(&mut clock);
        // Right from ? opens it and leaves the menu on Pref; Left from Pref
        // opens it and leaves the menu on ?.
        assert_eq!(m.key("ArrowRight", &mut clock), Choice::Controls);
        assert_eq!(m.widget.root, 2);
        assert_eq!(m.key("ArrowLeft", &mut clock), Choice::Controls);
        assert_eq!(m.widget.root, 0);
        // Rows still work on either side.
        m.key("ArrowLeft", &mut clock);
        assert_eq!(m.widget.root, 2);
        assert_eq!(m.key("Enter", &mut clock), Choice::Graphics);
        assert!(m.is_open());
    }

    /// The first tab's rows as the menu shows them: label and readout, with
    /// a star on a dimmed one.
    fn shown(menu: &Menu) -> Vec<String> {
        let rows: Vec<String> = menu
            .widget
            .controls(&menu.tree, &[], &|_| None)
            .into_iter()
            .filter(|(id, _, _)| *id < 100)
            .zip(&menu.tree[0].children)
            .map(|((_, _, text), node)| {
                let star = if menu.widget.dim.contains(&node.label) {
                    "*"
                } else {
                    ""
                };
                format!("{}{star}", text.trim_end())
            })
            .collect();
        rows
    }

    fn away_menu(rows: AwayRows) -> (Menu, Clock) {
        let mut menu = Menu::watching(&[]);
        menu.set_away(Some(rows));
        (menu, Clock::new(0, 1_000))
    }

    fn choose(menu: &mut Menu, clock: &mut Clock, row: &str) -> Choice {
        menu.open(clock);
        menu.activate(row, clock)
    }

    /// The away player's menu in each state it has (slice F2-O4): its own
    /// aircraft alive, lost with the rules allowing a new one, lost with
    /// them allowing nothing, a host with a ready standby, a host with none,
    /// and a game that only joined (a dedicated server's, a friend's).
    #[test]
    fn the_away_menu_snapshots_each_state() {
        let alive = AwayRows {
            own_aircraft: true,
            ..AwayRows::default()
        };
        assert_eq!(
            shown(&away_menu(alive.clone()).0),
            ["Take Back Flight", "Leave Game", "Exit to Desktop  Alt-F4"]
        );
        let lost = AwayRows::default();
        assert_eq!(
            shown(&away_menu(lost.clone()).0),
            ["Spawn in Aircraft", "Leave Game", "Exit to Desktop  Alt-F4"]
        );
        let blocked = AwayRows {
            spawn_blocked: Some("No lives left".into()),
            ..AwayRows::default()
        };
        assert_eq!(
            shown(&away_menu(blocked).0),
            [
                "Spawn in Aircraft  No lives left*",
                "Leave Game",
                "Exit to Desktop  Alt-F4"
            ]
        );
        // A host with a ready standby hands the game over: no confirm line.
        // A host with none says leaving ends it for everyone.
        let ends = AwayRows {
            own_aircraft: true,
            leave_ends_game: true,
            ..AwayRows::default()
        };
        assert_eq!(
            shown(&away_menu(ends).0),
            [
                "Take Back Flight",
                "Leave Game  Ends the game for everyone",
                "Exit to Desktop  Alt-F4"
            ]
        );
        // The ordinary observer keeps Stop Watching.
        assert_eq!(
            shown(&Menu::watching(&[])),
            ["Stop Watching", "Exit to Desktop  Alt-F4"]
        );
    }

    #[test]
    fn each_away_row_does_its_own_thing() {
        let (mut menu, mut clock) = away_menu(AwayRows {
            own_aircraft: true,
            ..AwayRows::default()
        });
        assert_eq!(
            choose(&mut menu, &mut clock, TAKE_BACK_FLIGHT),
            Choice::TakeBack
        );
        assert!(!menu.is_open(), "the menu closes as the aircraft is taken");
        // A joined game (a dedicated server's) leaves at once, no confirm.
        assert_eq!(choose(&mut menu, &mut clock, LEAVE_GAME), Choice::LeaveGame);
        // Lost, and the rules allow a new aircraft.
        let (mut menu, mut clock) = away_menu(AwayRows::default());
        assert_eq!(choose(&mut menu, &mut clock, SPAWN), Choice::Spawn);
        // Lost, and the rules allow nothing: the row answers with its reason.
        let (mut menu, mut clock) = away_menu(AwayRows {
            spawn_blocked: Some("No revival in this game".into()),
            ..AwayRows::default()
        });
        assert_eq!(choose(&mut menu, &mut clock, SPAWN), Choice::SpawnBlocked);
        assert!(menu.is_open(), "the menu stays up");
        assert!(menu.widget.dim.contains(&SPAWN.to_owned()));
    }

    #[test]
    fn a_host_with_no_standby_confirms_before_it_ends_the_game() {
        let (mut menu, mut clock) = away_menu(AwayRows {
            own_aircraft: true,
            leave_ends_game: true,
            ..AwayRows::default()
        });
        assert_eq!(choose(&mut menu, &mut clock, LEAVE_GAME), Choice::Click);
        assert_eq!(
            shown(&menu)[1],
            "Leave Game  Choose again to end it".to_owned()
        );
        assert_eq!(menu.activate(LEAVE_GAME, &mut clock), Choice::LeaveGame);
        // Closing the menu forgets the first choice.
        let (mut menu, mut clock) = away_menu(AwayRows {
            own_aircraft: true,
            leave_ends_game: true,
            ..AwayRows::default()
        });
        choose(&mut menu, &mut clock, LEAVE_GAME);
        menu.close(&mut clock);
        assert_eq!(shown(&menu)[1], "Leave Game  Ends the game for everyone");
        assert_eq!(choose(&mut menu, &mut clock, LEAVE_GAME), Choice::Click);
        // A standby that became ready in the meantime: no confirm needed.
        menu.set_away(Some(AwayRows {
            own_aircraft: true,
            ..AwayRows::default()
        }));
        assert_eq!(menu.activate(LEAVE_GAME, &mut clock), Choice::LeaveGame);
    }

    #[test]
    fn the_rows_follow_the_game_as_its_state_changes() {
        let (mut menu, _) = away_menu(AwayRows {
            own_aircraft: true,
            ..AwayRows::default()
        });
        menu.widget.focus = 1;
        // The AI lost the aircraft: Take Back Flight gives way to Spawn.
        menu.set_away(Some(AwayRows::default()));
        assert_eq!(menu.tree[0].children[0].label, SPAWN);
        assert_eq!(menu.tree[0].children[1].label, LEAVE_GAME);
        assert_eq!(menu.widget.focus, 1, "the focus stays on its row");
        // Unchanged rows rebuild nothing.
        let before = format!("{:?}", menu.tree);
        menu.set_away(Some(AwayRows::default()));
        assert_eq!(before, format!("{:?}", menu.tree));
        // Back to an observer's: Stop Watching returns.
        menu.set_away(None);
        assert_eq!(menu.tree[0].children[0].label, STOP_WATCHING);
    }

    #[test]
    fn start_pages_parse() {
        assert_eq!(Start::parse("?"), Ok(Start::Question));
        assert_eq!(Start::parse("time"), Ok(Start::Time));
        assert_eq!(Start::parse("controls"), Ok(Start::Controls));
        assert!(Start::parse("cheat").is_err());
    }
}
