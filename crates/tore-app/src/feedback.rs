//! The operator-enabled feedback form. Layout and controller keyboard are
//! opinionated host UI, sharing the controls editor's frame and buttons.
use crate::controls_editor::{self as style, Editor, Rect};
use tore_formats::font::Font;
use tore_replay::feedback::{Feedback, MAX_CHARS};
const FIELD: Rect = (16, 64, 608, 218);
const SUBMIT: Rect = (340, 446, 134, 26);
const CANCEL: Rect = (486, 446, 138, 26);
const KEYS: &str = "abcdefghijklmnopqrstuvwxyz0123456789 .,!?-";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    Submit,
    Cancel,
}
pub struct Form {
    pub flag: Feedback,
    caret: usize,
    focus: usize,
    pressed: Option<usize>,
}
impl Form {
    pub fn new(tick: u64) -> Self {
        Self {
            flag: Feedback {
                tick,
                opened_at: unix_ms(),
                ..Default::default()
            },
            caret: 0,
            focus: 0,
            pressed: None,
        }
    }
    pub fn text(&mut self, text: &str) {
        if self.focus != 0 {
            return;
        }
        let remaining = MAX_CHARS.saturating_sub(self.flag.text.chars().count());
        let add: String = text
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .take(remaining)
            .collect();
        self.flag.text.insert_str(self.caret, &add);
        self.caret += add.len();
    }
    pub fn key(&mut self, key: &str) -> Outcome {
        match key {
            "Escape" => return Outcome::Cancel,
            "Tab" => self.focus = (self.focus + 1) % 3,
            "Enter" if self.focus == 1 => {
                return if Feedback::valid_text(&self.flag.text) {
                    Outcome::Submit
                } else {
                    Outcome::None
                };
            }
            "Enter" if self.focus == 2 => return Outcome::Cancel,
            "Enter" => self.text("\n"),
            "ArrowLeft" if self.focus == 0 => {
                self.caret = self.flag.text[..self.caret]
                    .char_indices()
                    .last()
                    .map_or(0, |(i, _)| i)
            }
            "ArrowRight" if self.focus == 0 => {
                self.caret += self.flag.text[self.caret..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8)
            }
            "Home" => self.caret = 0,
            "End" => self.caret = self.flag.text.len(),
            "Backspace" if self.focus == 0 && self.caret > 0 => {
                let end = self.caret;
                self.key("ArrowLeft");
                self.flag.text.drain(self.caret..end);
            }
            "Delete" if self.focus == 0 && self.caret < self.flag.text.len() => {
                self.flag.text.remove(self.caret);
            }
            "ArrowUp" | "ArrowDown" => self.focus = (self.focus + 1) % 3,
            _ => {}
        }
        Outcome::None
    }
    /// A controller can enter text with the on-screen keyboard as well as
    /// choose Submit and Cancel. Row zero starts at the letter a.
    pub fn controller(&mut self, action: &str) -> Outcome {
        let count = KEYS.chars().count() + 1; // Backspace follows the letters.
        match action {
            "menu-back" => return Outcome::Cancel,
            "menu-accept" if self.focus >= 3 => {
                let index = self.focus - 3;
                let saved = self.focus;
                self.focus = 0;
                if let Some(c) = KEYS.chars().nth(index) {
                    self.text(&c.to_string());
                } else {
                    self.key("Backspace");
                }
                self.focus = saved;
            }
            "menu-accept" if self.focus == 0 => self.focus = 3,
            "menu-accept" => return self.key("Enter"),
            "menu-left" => {
                self.focus = if self.focus <= 3 {
                    3 + count - 1
                } else {
                    self.focus - 1
                }
            }
            "menu-right" => {
                self.focus = if self.focus < 3 || self.focus == 3 + count - 1 {
                    3
                } else {
                    self.focus + 1
                }
            }
            "menu-down" => {
                self.focus = if self.focus >= 3 && self.focus + 10 < 3 + count {
                    self.focus + 10
                } else if self.focus == 1 {
                    2
                } else if self.focus == 2 {
                    3
                } else {
                    1
                }
            }
            "menu-up" => {
                self.focus = if self.focus >= 13 {
                    self.focus - 10
                } else if self.focus >= 3 {
                    2
                } else {
                    3 + count - 1
                }
            }
            _ => {}
        }
        Outcome::None
    }
    fn controls() -> Vec<(usize, Rect, String)> {
        let mut out = vec![
            (0, FIELD, String::new()),
            (1, SUBMIT, "Submit".into()),
            (2, CANCEL, "Cancel".into()),
        ];
        for (i, c) in KEYS
            .chars()
            .map(|c| {
                if c == ' ' {
                    "Space".into()
                } else {
                    c.to_string()
                }
            })
            .chain(["Bksp".into()])
            .enumerate()
        {
            out.push((
                3 + i,
                (
                    16 + (i % 10) as i32 * 61,
                    310 + (i / 10) as i32 * 24,
                    57,
                    21,
                ),
                c,
            ));
        }
        out
    }
    pub fn pointer(&mut self, point: Option<(f64, f64)>, down: bool) -> Outcome {
        let hit = point.and_then(|p| {
            Self::controls()
                .iter()
                .find(|(_, r, _)| style::inside(p, *r))
                .map(|(i, _, _)| *i)
        });
        if down {
            self.pressed = hit;
            return Outcome::None;
        }
        if hit != self.pressed.take() {
            return Outcome::None;
        }
        if let Some(hit) = hit {
            self.focus = hit;
            if hit >= 3 {
                return self.controller("menu-accept");
            }
            if hit > 0 {
                return self.key("Enter");
            }
        }
        Outcome::None
    }
    pub fn draw(&self, pixels: &mut [u8], font: &Font) {
        style::title_bar(
            pixels,
            font,
            "FEEDBACK   |   Describe what happened",
            "Multiplayer flight continues",
        );
        Editor::text(
            pixels,
            font,
            (16, 34, 608, 24),
            style::TITLE,
            &format!(
                "Flag at {:02}:{:02} (tick {})",
                self.flag.tick / 7200,
                self.flag.tick / 120 % 60,
                self.flag.tick
            ),
            (16, 40),
        );
        crate::menu::Canvas(pixels).rect(FIELD, style::PANEL);
        let mut lines = vec![String::new()];
        let mut caret_line = 0;
        let mut caret_x = 0;
        for (offset, c) in self
            .flag
            .text
            .char_indices()
            .chain([(self.flag.text.len(), '\0')])
        {
            if style::text_width(font, lines.last().unwrap()) > 580 {
                lines.push(String::new());
            }
            if offset == self.caret {
                caret_line = lines.len() - 1;
                caret_x = style::text_width(font, lines.last().unwrap());
            }
            if c == '\n' {
                lines.push(String::new());
            } else if c != '\0' {
                lines.last_mut().unwrap().push(c);
            }
        }
        let start = caret_line.saturating_sub(11);
        for (i, line) in lines.iter().skip(start).take(12).enumerate() {
            Editor::text(
                pixels,
                font,
                FIELD,
                style::WHITE,
                line,
                (22, 72 + i as i32 * 17),
            );
        }
        if self.focus == 0 {
            crate::menu::Canvas(pixels).rect(
                (
                    22 + caret_x,
                    72 + (caret_line - start) as i32 * 17,
                    1,
                    font.height as i32,
                ),
                style::WHITE,
            );
        }
        Editor::text(
            pixels,
            font,
            (16, 286, 608, 20),
            style::TITLE,
            &format!(
                "{} / {MAX_CHARS} characters. Tab: buttons. Controller A: keyboard.",
                self.flag.text.chars().count()
            ),
            (16, 290),
        );
        for (id, r, label) in Self::controls().into_iter().filter(|(id, _, _)| *id > 0) {
            style::footer_button(pixels, font, r, &label, self.focus == id);
        }
    }
}
pub fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub fn enabled(lobby: Option<&tore_session::wire::messages::LobbyState>) -> bool {
    lobby.is_some_and(|l| l.feedback && l.host.is_none())
}
pub fn menu(tree: &mut [tore_formats::ui::MenuNode], enabled: bool) {
    if let Some(tab) = tree.first_mut() {
        tab.children.retain(|n| n.label != "Feedback...");
        if enabled {
            tab.children.push(tore_formats::ui::MenuNode {
                label: "Feedback...".into(),
                shortcut: String::new(),
                children: Vec::new(),
            });
        }
    }
}
impl crate::App {
    pub(crate) fn feedback_enabled(&self) -> bool {
        self.net_flight.is_some() && enabled(self.net.as_ref().and_then(|s| s.client.lobby()))
    }
    pub(crate) fn refresh_feedback(&mut self) {
        let enabled = self.feedback_enabled();
        if let Some(f) = &mut self.net_flight {
            menu(&mut f.menu, enabled);
        }
        if !enabled {
            self.feedback = None;
        }
    }
    pub(crate) fn feedback_result(&mut self, result: Outcome) {
        match result {
            Outcome::None => {}
            Outcome::Cancel => {
                self.feedback = None;
            }
            Outcome::Submit => {
                if self.feedback_enabled()
                    && let Some(form) = self.feedback.take()
                    && let Some(net) = &mut self.net
                {
                    let mut flag = form.flag;
                    flag.submitted_at = unix_ms();
                    net.client.send_feedback(flag);
                    self.flight_ui.message("Feedback sent to the server");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feedback_text_edits_utf8_and_bounds_submit_and_cancel() {
        let mut f = Form::new(120);
        f.text("aé🛩");
        f.key("ArrowLeft");
        f.key("Backspace");
        assert_eq!(f.flag.text, "a🛩");
        f.key("Delete");
        assert_eq!(f.flag.text, "a");
        f.key("Home");
        f.text("Ω");
        assert_eq!(f.flag.text, "Ωa");
        f.text(&"é".repeat(2000));
        assert_eq!(f.flag.text.chars().count(), 1000);
        f.key("Tab");
        assert_eq!(f.key("Enter"), Outcome::Submit);
        assert_eq!(f.flag.tick, 120);
        assert_eq!(f.key("Escape"), Outcome::Cancel);
        let mut empty = Form::new(0);
        empty.key("Tab");
        assert_eq!(empty.key("Enter"), Outcome::None);
    }
    #[test]
    fn feedback_controller_can_type_submit_and_cancel() {
        let mut f = Form::new(0);
        f.controller("menu-accept");
        f.controller("menu-accept");
        f.controller("menu-right");
        f.controller("menu-accept");
        assert_eq!(f.flag.text, "ab");
        for _ in 0..5 {
            f.controller("menu-down");
            if f.focus == 1 {
                break;
            }
        }
        assert_eq!(f.controller("menu-accept"), Outcome::Submit);
        assert_eq!(f.controller("menu-back"), Outcome::Cancel);
    }
    #[test]
    fn feedback_button_is_only_on_an_enabled_dedicated_server() {
        assert!(!enabled(None));
        let mut lobby = tore_session::wire::messages::LobbyState {
            feedback: false,
            name: String::new(),
            summary: String::new(),
            mission: 0,
            phase: tore_session::wire::messages::LobbyPhase::Flying,
            start: tore_session::wire::messages::StartRule::Flying,
            king: None,
            host: None,
            you: 0,
            players: vec![],
            slots: vec![],
            settings: vec![],
            mission_locked: false,
            force_seconds: None,
        };
        assert!(!enabled(Some(&lobby)));
        lobby.feedback = true;
        assert!(enabled(Some(&lobby)));
        lobby.host = Some(0);
        assert!(!enabled(Some(&lobby)));
        let mut tree = vec![tore_formats::ui::MenuNode {
            label: "?".into(),
            shortcut: String::new(),
            children: vec![],
        }];
        menu(&mut tree, false);
        assert!(tree[0].children.is_empty());
        menu(&mut tree, true);
        menu(&mut tree, true);
        assert_eq!(tree[0].children.len(), 1);
        menu(&mut tree, false);
        assert!(tree[0].children.is_empty());
    }
}
