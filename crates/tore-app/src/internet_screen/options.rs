//! The Internet Lobby screen's Options panel (slice I4): the port, the
//! password and the game name (shared with Direct Connection), the master's
//! address, "Forward the game port on my router" and "Send anonymous
//! statistics". Built of the widget kit and drawn over the screen, which
//! stops asking the master while it is up.
//!
//! The password is never kept (as on Direct Connection). The two switches
//! and the master's address are kept in `network-v1.conf` beside the rest
//! ([`crate::net::settings::Remembered`]). The port-forward switch only keeps
//! the choice until slice J4b builds the mapper that reads it.
use crate::menu::{Canvas, text_width};
use crate::net::settings::{Remembered, master_ok};
use crate::ui_text;
use crate::widgets::{
    Button, CheckBox, Filter, Focus, Kit, Outcome, Point, Rect, Route, TextField, Widget,
    draw_panel, tone,
};
use std::time::Instant;

/// The panel's rectangle: a sibling of the screen's panel, smaller, centred.
const PANEL: Rect = (80, 100, 480, 360);
const LABEL_X: i32 = 115;
const FIELD_X: i32 = 240;
/// The longest game name: the dedicated server's limit.
const MAX_NAME: usize = crate::net::options::MAX_NAME;
/// The longest master address typed.
const MAX_MASTER: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Id {
    Port,
    Password,
    Name,
    Master,
    Forward,
    Statistics,
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
    /// The master's address; `None` for the built-in one.
    pub master: Option<String>,
    pub port_forward: bool,
    pub telemetry: bool,
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
    master: TextField,
    forward: CheckBox,
    statistics: CheckBox,
    ok: Button,
    cancel: Button,
    focus: Focus<Id>,
    error: Option<&'static str>,
}

impl OptionsPanel {
    /// The panel with the screen's remembered choices; `password` is this
    /// run's, `default_name` what a game is called when no name is typed.
    pub fn new(settings: &Remembered, password: &str, default_name: &str) -> Self {
        let mut port_field = TextField::line((FIELD_X, 139), 60, Filter::Port);
        port_field.set_text(&settings.port.to_string());
        let mut password_field = TextField::line((FIELD_X, 169), 216, Filter::Text)
            .masked()
            .with_hint("none");
        password_field.set_text(password);
        let mut name_field = TextField::line((FIELD_X, 199), 216, Filter::Text)
            .with_max(MAX_NAME)
            .with_hint(default_name.to_owned());
        name_field.set_text(settings.game_name.as_deref().unwrap_or(""));
        let mut master_field = TextField::line((FIELD_X, 229), 216, Filter::Text)
            .with_max(MAX_MASTER)
            .with_hint(format!("built in: {}", tore_net::master::DEFAULT_MASTER));
        master_field.set_text(settings.master.as_deref().unwrap_or(""));
        let mut focus = Focus::new(
            vec![
                Id::Port,
                Id::Password,
                Id::Name,
                Id::Master,
                Id::Forward,
                Id::Statistics,
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
            master: master_field,
            forward: CheckBox::new(
                (LABEL_X, 258),
                "Forward the game port on my router",
                settings.port_forward,
            ),
            statistics: CheckBox::new(
                (LABEL_X, 292),
                "Send anonymous statistics",
                settings.telemetry,
            ),
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
            Some(Id::Port | Id::Password | Id::Name | Id::Master)
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
        let master = self.master.text().trim();
        if !master.is_empty() && !master_ok(master) {
            self.error =
                Some("The master must be a name or address, then a port if it is not 26901.");
            self.focus.set(Id::Master);
            return Answer::None;
        }
        let name = self.name.text().trim();
        Answer::Ok(Values {
            port,
            password: self.password.text().to_owned(),
            name: (!name.is_empty()).then(|| name.to_owned()),
            master: (!master.is_empty()).then(|| master.to_owned()),
            port_forward: self.forward.checked(),
            telemetry: self.statistics.checked(),
        })
    }

    pub fn text_input(&mut self, text: &str) {
        let outcome = match self.focus.current() {
            Some(Id::Port) => self.port.text_input(text),
            Some(Id::Password) => self.password.text_input(text),
            Some(Id::Name) => self.name.text_input(text),
            Some(Id::Master) => self.master.text_input(text),
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
        let now = Instant::now();
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
                    Id::Master => self.master.key(name),
                    // Enter turns a check box; it does not accept the panel.
                    Id::Forward => {
                        let outcome = self.forward.key(name, now);
                        return self.turned(outcome);
                    }
                    Id::Statistics => {
                        let outcome = self.statistics.key(name, now);
                        return self.turned(outcome);
                    }
                    Id::Ok => self.ok.key(name),
                    Id::Cancel => return self.cancel_key(name),
                };
                match (id, outcome) {
                    (Id::Ok, Outcome::Activated) => self.accept(),
                    (Id::Port | Id::Password | Id::Name | Id::Master, Outcome::Activated) => {
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

    fn turned(&mut self, outcome: Outcome) -> Answer {
        if outcome == Outcome::Changed {
            self.error = None;
        }
        Answer::None
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

    /// The panel has nothing to scroll.
    pub fn wheel(&mut self, _notches: i32, _point: Option<Point>) {}

    pub fn button(&mut self, kit: &Kit, point: Option<Point>, pressed: bool) -> Answer {
        let now = Instant::now();
        let Some(point) = point else {
            if !pressed {
                self.ok.release((-1, -1));
                self.cancel.release((-1, -1));
                self.forward.release((-1, -1), now);
                self.statistics.release((-1, -1), now);
            }
            return Answer::None;
        };
        if pressed {
            for (id, field) in [
                (Id::Port, &mut self.port),
                (Id::Password, &mut self.password),
                (Id::Name, &mut self.name),
                (Id::Master, &mut self.master),
            ] {
                if field.hit(point) {
                    self.focus.set(id);
                    field.press(point, kit);
                }
            }
            if self.forward.hit(point) {
                self.focus.set(Id::Forward);
                self.forward.press(point);
            }
            if self.statistics.hit(point) {
                self.focus.set(Id::Statistics);
                self.statistics.press(point);
            }
            self.ok.press(point);
            self.cancel.press(point);
            return Answer::None;
        }
        self.forward.release(point, now);
        self.statistics.release(point, now);
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

    /// Walks the check boxes' lamps; the screen calls it every frame.
    pub fn advance(&mut self) {
        let now = Instant::now();
        self.forward.advance(now);
        self.statistics.advance(now);
    }

    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit) {
        draw_panel(canvas, kit, PANEL);
        let font = kit.sprite("PANELFNT");
        let dim = kit.sprite("PANELFND");
        let title = "Options";
        let text = ui_text::text;
        text(
            canvas,
            kit,
            font,
            title,
            (
                PANEL.0 + (PANEL.2 - text_width(font, title)) / 2,
                PANEL.1 + 7,
            ),
            None,
            None,
        );
        canvas.outline((100, 120, 440, 320), [174, 174, 174, 255]);
        text(canvas, kit, font, "Port:", (LABEL_X, 142), None, None);
        text(canvas, kit, font, "Password:", (LABEL_X, 172), None, None);
        text(canvas, kit, font, "Game name:", (LABEL_X, 202), None, None);
        text(canvas, kit, font, "Master:", (LABEL_X, 232), None, None);
        for (line, y) in [
            (
                "The password is sent when you join and used when you host. It is never saved.",
                330,
            ),
            (
                "Statistics: an anonymous id, how long you played, how many players, how you",
                348,
            ),
            (
                "connected. No names, no addresses. The README says what is sent.",
                364,
            ),
        ] {
            text(canvas, kit, dim, line, (LABEL_X, y), None, None);
        }
        let marked = |id: Id| self.focus.marked(id);
        self.port.draw(canvas, kit, self.focus.is(Id::Port));
        self.password.draw(canvas, kit, self.focus.is(Id::Password));
        self.name.draw(canvas, kit, self.focus.is(Id::Name));
        self.master.draw(canvas, kit, self.focus.is(Id::Master));
        self.forward.draw(canvas, kit, marked(Id::Forward));
        self.statistics.draw(canvas, kit, marked(Id::Statistics));
        if let Some(error) = self.error {
            text(
                canvas,
                kit,
                font,
                error,
                (LABEL_X, 388),
                None,
                Some(tone::ENEMY),
            );
        }
        self.ok.draw(canvas, kit, marked(Id::Ok));
        self.cancel.draw(canvas, kit, marked(Id::Cancel));
    }
}
