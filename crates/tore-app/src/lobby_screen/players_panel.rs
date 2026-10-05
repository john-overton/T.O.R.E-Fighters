//! The Players... panel of the lobby screen (stage F phase 2, slice F2-L):
//! what the King does to the player selected in Players. Give the crown
//! passes the King's role on (the host asks nothing more of the new King);
//! Kick opens EF8's reason panel. The house, whose leaving would end the
//! game, cannot be kicked: the host says so, and the panel does not offer
//! it.
use crate::menu::{Canvas, text_width};
use crate::ui_text;
use crate::widgets::{Button, Focus, Kit, Outcome, Point, Rect, Route, draw_panel, fit};

/// The panel's place: centred over the lobby's panel.
const PANEL: Rect = (110, 150, 420, 180);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Id {
    Crown,
    Kick,
    Close,
}

/// What the screen does after an event reaches the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    None,
    /// Give the crown to this lobby id.
    Crown(u8),
    /// Open the Kick panel for this lobby id.
    Kick(u8),
    Close,
}

pub struct PlayersPanel {
    player: u8,
    callsign: String,
    /// The player is the house: it cannot be kicked.
    house: bool,
    lines: Vec<String>,
    crown: Button,
    kick: Button,
    close: Button,
    focus: Focus<Id>,
}

impl PlayersPanel {
    /// The panel for `callsign`, lobby id `player`. `house` says the player's
    /// machine runs the game.
    pub fn new(player: u8, callsign: &str, house: bool) -> Self {
        let mut focus = Focus::new(vec![Id::Crown, Id::Kick, Id::Close], None);
        focus.set(Id::Close);
        let y = PANEL.1 + 132;
        let mut kick = Button::new("Kick...", (PANEL.0 + 160, y), 85);
        kick.set_enabled(!house);
        Self {
            player,
            callsign: callsign.to_owned(),
            house,
            lines: vec![
                format!("{callsign} is selected."),
                if house {
                    format!("{callsign}'s game runs this game, so {callsign} cannot be kicked.")
                } else {
                    format!("Give {callsign} the crown, or remove {callsign} from the game.")
                },
            ],
            crown: Button::new("Give crown", (PANEL.0 + 40, y), 105),
            kick,
            close: Button::new("Close", (PANEL.0 + 260, y), 85).default_button(),
            focus,
        }
    }

    pub fn player(&self) -> u8 {
        self.player
    }

    pub fn callsign(&self) -> &str {
        &self.callsign
    }

    #[cfg(test)]
    pub fn house(&self) -> bool {
        self.house
    }

    #[cfg(test)]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    #[cfg(test)]
    pub fn kick_enabled(&self) -> bool {
        crate::widgets::Widget::enabled(&self.kick)
    }

    fn press_id(&self, id: Id) -> Answer {
        match id {
            Id::Crown => Answer::Crown(self.player),
            Id::Kick if !self.house => Answer::Kick(self.player),
            Id::Kick => Answer::None,
            Id::Close => Answer::Close,
        }
    }

    pub fn key(&mut self, name: &str, shift: bool) -> Answer {
        if name == "Escape" {
            return Answer::Close;
        }
        let kick_ok = !self.house;
        match self.focus.key(name, shift, |id| id != Id::Kick || kick_ok) {
            Route::Moved | Route::Ignored | Route::Default(_) => Answer::None,
            Route::Widget(id) => {
                let fired = match id {
                    Id::Crown => self.crown.key(name),
                    Id::Kick => self.kick.key(name),
                    Id::Close => self.close.key(name),
                };
                if fired == Outcome::Activated {
                    self.press_id(id)
                } else {
                    Answer::None
                }
            }
        }
    }

    pub fn moved(&mut self, point: Option<Point>) {
        self.crown.pointer_move(point);
        self.kick.pointer_move(point);
        self.close.pointer_move(point);
    }

    pub fn button(&mut self, point: Option<Point>, pressed: bool) -> Answer {
        let Some(point) = point else {
            if !pressed {
                for button in [&mut self.crown, &mut self.kick, &mut self.close] {
                    button.release((-1, -1));
                }
            }
            return Answer::None;
        };
        if pressed {
            self.crown.press(point);
            self.kick.press(point);
            self.close.press(point);
            return Answer::None;
        }
        let crown = self.crown.release(point) == Outcome::Activated;
        let kick = self.kick.release(point) == Outcome::Activated;
        let close = self.close.release(point) == Outcome::Activated;
        if crown {
            self.press_id(Id::Crown)
        } else if kick {
            self.press_id(Id::Kick)
        } else if close {
            Answer::Close
        } else {
            Answer::None
        }
    }

    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit) {
        draw_panel(canvas, kit, PANEL);
        let font = kit.sprite("PANELFNT");
        let title = format!("Players: {}", self.callsign);
        let title = fit(font, &title, PANEL.2 - 48);
        ui_text::text(
            canvas,
            kit,
            font,
            &title,
            (
                PANEL.0 + (PANEL.2 - text_width(font, &title)) / 2,
                PANEL.1 + 7,
            ),
            None,
            None,
        );
        canvas.outline(
            (PANEL.0 + 12, PANEL.1 + 24, PANEL.2 - 24, PANEL.3 - 36),
            [174, 174, 174, 255],
        );
        let mut y = PANEL.1 + 38;
        for line in &self.lines {
            let line = fit(font, line, PANEL.2 - 48);
            ui_text::text(canvas, kit, font, &line, (PANEL.0 + 22, y), None, None);
            y += 18;
        }
        self.crown.draw(canvas, kit, self.focus.marked(Id::Crown));
        self.kick.draw(canvas, kit, self.focus.marked(Id::Kick));
        self.close.draw(canvas, kit, self.focus.marked(Id::Close));
    }
}
