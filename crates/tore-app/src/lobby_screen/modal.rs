//! The two small panels the lobby opens over itself: the King's Kick, which
//! asks for the reason the player will see, and the King's Leave, which asks
//! before it ends the game for everyone. One panel serves both: a title,
//! some lines of words, an optional edit line, and two buttons.
use crate::menu::{Canvas, text_width};
use crate::widgets::{
    Button, Filter, Focus, Kit, Outcome, Point, Rect, Route, TextField, Widget, draw_panel, fit,
};

/// The panel's place: centred over the lobby's panel.
const PANEL: Rect = (110, 150, 420, 180);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Id {
    Reason,
    Ok,
    Cancel,
}

/// What the screen does after an event reaches the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    None,
    /// The player chose the first button; the text is the reason line (empty
    /// when the panel has none).
    Ok(String),
    Cancel,
}

/// What a panel is for, so the screen knows what an answer means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// Kick this player (lobby id and callsign).
    Kick(u8, String),
    /// The King leaves and the game ends.
    Leave,
}

pub struct Modal {
    pub purpose: Purpose,
    title: String,
    lines: Vec<String>,
    reason: Option<TextField>,
    ok: Button,
    cancel: Button,
    focus: Focus<Id>,
}

/// The most characters of a kick reason (it travels in a goodbye).
pub const MAX_REASON: usize = 60;

impl Modal {
    /// The Kick panel for `callsign`: a reason line, Kick and Cancel.
    pub fn kick(id: u8, callsign: &str) -> Self {
        let field = TextField::line((PANEL.0 + 22, PANEL.1 + 84), 376, Filter::Text)
            .with_max(MAX_REASON)
            .with_hint("the reason the player sees (optional)");
        let mut focus = Focus::new(vec![Id::Reason, Id::Ok, Id::Cancel], None);
        focus.set(Id::Reason);
        Self {
            purpose: Purpose::Kick(id, callsign.to_owned()),
            title: format!("Kick {callsign}"),
            lines: vec![
                format!("{callsign} is removed from the game."),
                "Reason (the player sees it):".to_owned(),
            ],
            reason: Some(field),
            ok: Button::new("Kick", (PANEL.0 + 100, PANEL.1 + 132), 85).default_button(),
            cancel: Button::new("Cancel", (PANEL.0 + 235, PANEL.1 + 132), 85),
            focus,
        }
    }

    /// The King's Leave panel: a question and Leave and Cancel (Cancel has
    /// the keyboard, so a stray Enter does not end everyone's game).
    pub fn leave() -> Self {
        let mut focus = Focus::new(vec![Id::Ok, Id::Cancel], None);
        focus.set(Id::Cancel);
        Self {
            purpose: Purpose::Leave,
            title: "Leave the game".to_owned(),
            lines: vec![
                "Leaving ends the game for everyone.".to_owned(),
                "Leave?".to_owned(),
            ],
            reason: None,
            ok: Button::new("Leave", (PANEL.0 + 100, PANEL.1 + 120), 85),
            cancel: Button::new("Cancel", (PANEL.0 + 235, PANEL.1 + 120), 85).default_button(),
            focus,
        }
    }

    /// The panel's words and button labels, for tests.
    #[cfg(test)]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Text typed now goes to the reason line.
    pub fn typing(&self) -> bool {
        self.reason.is_some() && self.focus.is(Id::Reason)
    }

    pub fn reason_text(&self) -> &str {
        self.reason.as_ref().map_or("", TextField::text)
    }

    pub fn text_input(&mut self, text: &str) {
        if self.typing()
            && let Some(field) = &mut self.reason
        {
            field.text_input(text);
        }
    }

    fn accept(&self) -> Answer {
        Answer::Ok(self.reason_text().trim().to_owned())
    }

    pub fn key(&mut self, name: &str, shift: bool) -> Answer {
        if name == "Escape" {
            return Answer::Cancel;
        }
        match self.focus.key(name, shift, |_| true) {
            Route::Moved | Route::Ignored => Answer::None,
            Route::Default(_) => Answer::None,
            Route::Widget(Id::Reason) => match self.reason.as_mut().map(|f| f.key(name)) {
                Some(Outcome::Activated) => self.accept(),
                _ => Answer::None,
            },
            Route::Widget(Id::Ok) => {
                if self.ok.key(name) == Outcome::Activated {
                    self.accept()
                } else {
                    Answer::None
                }
            }
            Route::Widget(Id::Cancel) => {
                if self.cancel.key(name) == Outcome::Activated {
                    Answer::Cancel
                } else {
                    Answer::None
                }
            }
        }
    }

    pub fn moved(&mut self, point: Option<Point>) {
        self.ok.pointer_move(point);
        self.cancel.pointer_move(point);
    }

    pub fn button(&mut self, kit: &Kit, point: Option<Point>, pressed: bool) -> Answer {
        let Some(point) = point else {
            if !pressed {
                self.ok.release((-1, -1));
                self.cancel.release((-1, -1));
            }
            return Answer::None;
        };
        if pressed {
            if let Some(field) = &mut self.reason
                && field.hit(point)
            {
                self.focus.set(Id::Reason);
                field.press(point, kit);
            }
            self.ok.press(point);
            self.cancel.press(point);
            return Answer::None;
        }
        let ok = self.ok.release(point) == Outcome::Activated;
        let cancel = self.cancel.release(point) == Outcome::Activated;
        if ok {
            self.accept()
        } else if cancel {
            Answer::Cancel
        } else {
            Answer::None
        }
    }

    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit) {
        draw_panel(canvas, kit, PANEL);
        let font = kit.sprite("PANELFNT");
        canvas.text(
            font,
            &self.title,
            PANEL.0 + (PANEL.2 - text_width(font, &self.title)) / 2,
            PANEL.1 + 7,
            None,
        );
        canvas.outline(
            (PANEL.0 + 12, PANEL.1 + 24, PANEL.2 - 24, PANEL.3 - 36),
            [174, 174, 174, 255],
        );
        let mut y = PANEL.1 + 38;
        for line in &self.lines {
            let line = fit(font, line, PANEL.2 - 48);
            canvas.text(font, &line, PANEL.0 + 22, y, None);
            y += 18;
        }
        if let Some(field) = &self.reason {
            field.draw(canvas, kit, self.focus.is(Id::Reason));
        }
        self.ok.draw(canvas, kit, self.focus.marked(Id::Ok));
        self.cancel.draw(canvas, kit, self.focus.marked(Id::Cancel));
    }
}
