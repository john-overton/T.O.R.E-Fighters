//! The Direct Connection screen's Options panel: the port, the password and
//! the name of the game being hosted, and the retail quick messages read only
//! (slice EF7). Built of the widget kit and drawn over the screen, which stops
//! listening while it is up.
use crate::menu::{Canvas, text_width};
use crate::widgets::{
    Button, Filter, Focus, Kit, MessageBox, Outcome, Point, Rect, Route, TextField, Widget,
    draw_panel, tone,
};
use tore_formats::chat::{QuickMessage, Receiver};

/// The panel's rectangle: a sibling of the screen's panel, smaller, centred.
const PANEL: Rect = (80, 100, 480, 360);
const LABEL_X: i32 = 115;
const FIELD_X: i32 = 240;
/// The longest game name: the dedicated server's limit.
const MAX_NAME: usize = crate::net::options::MAX_NAME;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Id {
    Port,
    Password,
    Name,
    Quick,
    Ok,
    Cancel,
}

/// What the panel's values are when the player accepts them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Values {
    pub port: u16,
    /// The password to send and to host with, empty for none. Never stored.
    pub password: String,
    /// The game's name when hosting; `None` for "CALLSIGN's game".
    pub name: Option<String>,
}

/// What the screen does after an event reaches the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    None,
    Ok(Values),
    Cancel,
}

pub struct OptionsPanel {
    port: TextField,
    password: TextField,
    name: TextField,
    pub(super) quick: MessageBox,
    ok: Button,
    cancel: Button,
    focus: Focus<Id>,
    error: Option<&'static str>,
}

impl OptionsPanel {
    pub fn new(
        kit: &Kit,
        port: u16,
        password: &str,
        name: Option<&str>,
        default_name: &str,
        quick: &[QuickMessage],
    ) -> Self {
        let mut port_field = TextField::line((FIELD_X, 139), 60, Filter::Port);
        port_field.set_text(&port.to_string());
        let mut password_field = TextField::line((FIELD_X, 169), 216, Filter::Text)
            .masked()
            .with_hint("none");
        password_field.set_text(password);
        let mut name_field = TextField::line((FIELD_X, 199), 216, Filter::Text)
            .with_max(MAX_NAME)
            .with_hint(default_name.to_owned());
        name_field.set_text(name.unwrap_or(""));
        let mut messages = MessageBox::new((LABEL_X, 262, 410, 128));
        if quick.iter().all(|m| m.text.is_empty()) {
            messages.push(kit, "No quick messages were imported.", tone::SYSTEM);
        }
        for (i, message) in quick.iter().enumerate().filter(|(_, m)| !m.text.is_empty()) {
            let to = match message.receiver {
                Some(Receiver::All) => "to all",
                Some(Receiver::Friendlies) => "to friendlies",
                Some(Receiver::Enemies) => "to enemies",
                Some(Receiver::Wing) => "to wing",
                Some(Receiver::Target) => "to target",
                None => "to the chosen receiver",
            };
            messages.push(
                kit,
                &format!("F{}  {}  ({to})", i + 1, message.text),
                tone::SYSTEM,
            );
        }
        let mut focus = Focus::new(
            vec![
                Id::Port,
                Id::Password,
                Id::Name,
                Id::Quick,
                Id::Ok,
                Id::Cancel,
            ],
            None,
        );
        focus.set(Id::Port);
        Self {
            port: port_field,
            password: password_field,
            name: name_field,
            quick: messages,
            ok: Button::new("OK", (190, 408), 85).default_button(),
            cancel: Button::new("Cancel", (330, 408), 85),
            focus,
            error: None,
        }
    }

    /// True while a text field has the keyboard, so typed text is for it.
    pub fn typing(&self) -> bool {
        matches!(
            self.focus.current(),
            Some(Id::Port | Id::Password | Id::Name)
        )
    }

    /// The accepted values, or the reason they are not (shown on the panel).
    fn accept(&mut self) -> Answer {
        let port = self
            .port
            .text()
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0);
        let Some(port) = port else {
            self.error = Some("The port must be a number from 1 to 65535.");
            self.focus.set(Id::Port);
            return Answer::None;
        };
        let name = self.name.text().trim();
        Answer::Ok(Values {
            port,
            password: self.password.text().to_owned(),
            name: (!name.is_empty()).then(|| name.to_owned()),
        })
    }

    pub fn text_input(&mut self, text: &str) {
        let outcome = match self.focus.current() {
            Some(Id::Port) => self.port.text_input(text),
            Some(Id::Password) => self.password.text_input(text),
            Some(Id::Name) => self.name.text_input(text),
            _ => Outcome::None,
        };
        if outcome != Outcome::None {
            self.error = None;
        }
    }

    pub fn key(&mut self, name: &str, shift: bool) -> Answer {
        if name == "Escape" {
            return Answer::Cancel;
        }
        let usable = |_: Id| true;
        match self.focus.key(name, shift, usable) {
            Route::Moved => Answer::None,
            Route::Ignored | Route::Default(_) => {
                if name == "Enter" {
                    self.accept()
                } else {
                    Answer::None
                }
            }
            Route::Widget(id) => {
                let outcome = match id {
                    Id::Port => self.port.key(name),
                    Id::Password => self.password.key(name),
                    Id::Name => self.name.key(name),
                    Id::Quick => {
                        self.quick.key(name);
                        if name == "Enter" {
                            Outcome::Activated
                        } else {
                            Outcome::None
                        }
                    }
                    Id::Ok => self.ok.key(name),
                    Id::Cancel => return self.cancel_key(name),
                };
                match (id, outcome) {
                    (Id::Ok, Outcome::Activated) => self.accept(),
                    (Id::Port | Id::Password | Id::Name | Id::Quick, Outcome::Activated) => {
                        self.accept()
                    }
                    (_, Outcome::Changed) => {
                        self.error = None;
                        Answer::None
                    }
                    _ => Answer::None,
                }
            }
        }
    }

    fn cancel_key(&mut self, name: &str) -> Answer {
        if self.cancel.key(name) == Outcome::Activated {
            Answer::Cancel
        } else {
            Answer::None
        }
    }

    pub fn moved(&mut self, point: Option<Point>) {
        self.ok.pointer_move(point);
        self.cancel.pointer_move(point);
    }

    pub fn wheel(&mut self, notches: i32, point: Option<Point>) {
        if point.is_some_and(|p| self.quick.hit(p)) {
            self.quick.wheel(notches);
        }
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
            for (id, field) in [
                (Id::Port, &mut self.port),
                (Id::Password, &mut self.password),
                (Id::Name, &mut self.name),
            ] {
                if field.hit(point) {
                    self.focus.set(id);
                    field.press(point, kit);
                }
            }
            if self.quick.hit(point) {
                self.focus.set(Id::Quick);
                self.quick.press(point);
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
        let dim = kit.sprite("PANELFND");
        let title = "Options";
        canvas.text(
            font,
            title,
            PANEL.0 + (PANEL.2 - text_width(font, title)) / 2,
            PANEL.1 + 7,
            None,
        );
        canvas.outline((100, 120, 440, 320), [174, 174, 174, 255]);
        canvas.text(font, "Port:", LABEL_X, 142, None);
        canvas.text(font, "Password:", LABEL_X, 172, None);
        canvas.text(font, "Game name:", LABEL_X, 202, None);
        canvas.text(
            dim,
            "The password is sent when you join and used when you host. It is never saved.",
            LABEL_X,
            230,
            None,
        );
        canvas.text(
            font,
            "Quick messages (read only): F1 to F12 while typing a message in flight",
            LABEL_X,
            248,
            None,
        );
        let marked = |id: Id| self.focus.marked(id);
        self.port.draw(canvas, kit, self.focus.is(Id::Port));
        self.password.draw(canvas, kit, self.focus.is(Id::Password));
        self.name.draw(canvas, kit, self.focus.is(Id::Name));
        self.quick.draw(canvas, kit, marked(Id::Quick));
        if let Some(error) = self.error {
            canvas.text(font, error, LABEL_X, 396, Some(tone::ENEMY));
        }
        self.ok.draw(canvas, kit, marked(Id::Ok));
        self.cancel.draw(canvas, kit, marked(Id::Cancel));
    }
}
