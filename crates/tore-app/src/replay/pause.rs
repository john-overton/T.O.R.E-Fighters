//! The replay viewer's Escape menu: flight's paused menu (`pause_menu.rs`)
//! over a tree built from the imported `FMENUD.MNU` rows that mean
//! something in a replay, plus four authored rows. Opening it pauses
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
/// The retail `?` row that quits the game, shown as Exit to Desktop.
pub const EXIT: &str = "Exit to Windows";
/// Authored Pref rows: the same switches as the N, T, R and C keys.
pub const NAME_LABELS: &str = "Name labels?";
pub const MISSION_TIMER: &str = "Mission timer?";
pub const TRAILS: &str = "Flight path trails?";
pub const COMMS: &str = "Comms panel?";
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
/// End Replay and Exit to Windows, and its Pref tab with Graphics...,
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
            other => {
                return Err(format!(
                    "unknown replay menu page {other:?}: use ?, pref, time, help, graphics or sound"
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

/// What a key or click in the menu asks of the viewer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    None,
    /// The menu moved: a click sound.
    Click,
    /// Back to the Replays screen.
    Leave,
    /// Quit the game.
    Exit,
    Graphics,
    Sound,
    /// Switch an interface part.
    Toggle(Part),
}

/// The open menu and the play state it paused.
pub struct Menu {
    pub widget: PauseMenu,
    pub tree: Vec<MenuNode>,
    /// While open: whether playback goes on when it closes.
    open: Option<bool>,
}

impl Menu {
    pub fn new(imported: &[MenuNode]) -> Self {
        Self {
            widget: PauseMenu::default(),
            tree: tree(imported),
            open: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Opens the menu over the view and pauses playback, noting whether it
    /// was playing.
    pub fn open(&mut self, clock: &mut Clock) {
        if self.open.is_none() {
            self.open = Some(!clock.paused());
            clock.pause();
        }
    }

    /// Opens the menu at `start`, for captures.
    pub fn open_at(&mut self, start: Start, clock: &mut Clock) {
        self.open(clock);
        match start {
            Start::Question => self.widget.show_tab(0),
            Start::Help => self.widget.open_help(),
            Start::Pref | Start::Time | Start::Graphics | Start::Sound => {
                self.widget.show_tab(1);
                let row = match start {
                    Start::Time => "Time",
                    Start::Sound => "Sound...",
                    _ => "Graphics...",
                };
                if let Some(at) = self.tree[1].children.iter().position(|n| n.label == row) {
                    self.widget.focus_row(&self.tree, at);
                    if start == Start::Time {
                        self.widget.enter(at);
                    }
                }
            }
        }
    }

    /// Closes the menu; playback goes on in the speed and direction it had,
    /// or stays paused if it was.
    pub fn close(&mut self, clock: &mut Clock) {
        if let Some(play) = self.open.take()
            && play
            && clock.paused()
        {
            clock.toggle();
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
        match self.widget.key(name, &self.tree) {
            Event::Select(index) => self.select(index, clock),
            _ => Choice::None,
        }
    }

    /// The left button went down or up at `point` in the 640x480 layer.
    pub fn pointer(&mut self, point: Option<(f64, f64)>, down: bool, clock: &mut Clock) -> Choice {
        match self.widget.pointer(&self.tree, &LOOK, point, down) {
            Event::None | Event::Switched(_) => Choice::None,
            Event::Click => Choice::Click,
            Event::Tab(index) => {
                self.widget.show_tab(index);
                Choice::Click
            }
            Event::Select(index) => self.select(index, clock),
            Event::Button(index) => self.activate(BUTTONS[index], clock),
        }
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
            END_REPLAY => Choice::Leave,
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
            ["?|", "Pref|"].map(String::from).to_vec(),
            "only the two tabs, in retail order"
        );
        assert_eq!(
            labels(&t[0].children),
            ["End Replay|", "Exit to Windows|Alt-F4"]
                .map(String::from)
                .to_vec()
        );
        assert_eq!(
            labels(&t[1].children),
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
            labels(&t[1].children[2].children),
            ["Paused|", "Slow-motion|", "1x|", "2x|", "4x|", "8x|"]
                .map(String::from)
                .to_vec()
        );
        // Without imported data the same menu is authored.
        assert_eq!(format!("{:?}", tree(&[])), format!("{t:?}"));
    }

    #[test]
    fn start_pages_parse() {
        assert_eq!(Start::parse("?"), Ok(Start::Question));
        assert_eq!(Start::parse("time"), Ok(Start::Time));
        assert!(Start::parse("cheat").is_err());
    }
}
