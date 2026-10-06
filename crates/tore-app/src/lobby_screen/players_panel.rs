//! The Players... panel of the lobby screen (stage F phase 2, slice F2-L):
//! what the King does to the player selected in Players. Give the crown
//! passes the King's role on (the host asks nothing more of the new King);
//! Kick opens EF8's reason panel. The house, whose leaving would end the
//! game, cannot be kicked: the host says so, and the panel does not offer
//! it. Release (stage K, slice K7b) frees the aircraft the AI flies for a
//! player who is away, or who dropped and is not in the game any more: the
//! panel then stands for the reserved slot, not a player.
use crate::menu::{Canvas, text_width};
use crate::ui_text;
use crate::widgets::{Button, Focus, Kit, Outcome, Point, Rect, Route, draw_panel, fit};

/// The panel's place: centred over the lobby's panel.
const PANEL: Rect = (110, 150, 420, 180);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Id {
    Crown,
    Kick,
    Release,
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
    /// Free the aircraft kept for a player: this plane (stage K).
    Release(u32),
    Close,
}

pub struct PlayersPanel {
    player: u8,
    callsign: String,
    /// The player is the house: it cannot be kicked.
    house: bool,
    /// The aircraft the AI flies for the player, which Release frees.
    release_plane: Option<u32>,
    /// The panel is for a reserved slot whose player is not in the game: no
    /// crown, no kick.
    slot_only: bool,
    lines: Vec<String>,
    crown: Button,
    kick: Button,
    release: Button,
    close: Button,
    focus: Focus<Id>,
}

impl PlayersPanel {
    /// The panel for `callsign`, lobby id `player`. `house` says the player's
    /// machine runs the game.
    pub fn new(player: u8, callsign: &str, house: bool) -> Self {
        let mut focus = Focus::new(vec![Id::Crown, Id::Kick, Id::Release, Id::Close], None);
        focus.set(Id::Close);
        let y = PANEL.1 + 132;
        let mut kick = Button::new("Kick...", (PANEL.0 + 128, y), 80);
        kick.set_enabled(!house);
        let mut release = Button::new("Release", (PANEL.0 + 218, y), 85);
        release.set_enabled(false);
        Self {
            player,
            callsign: callsign.to_owned(),
            house,
            release_plane: None,
            slot_only: false,
            lines: vec![
                format!("{callsign} is selected."),
                if house {
                    format!("{callsign}'s game runs this game, so {callsign} cannot be kicked.")
                } else {
                    format!("Give {callsign} the crown, or remove {callsign} from the game.")
                },
            ],
            crown: Button::new("Give crown", (PANEL.0 + 18, y), 100),
            kick,
            release,
            close: Button::new("Close", (PANEL.0 + 313, y), 80).default_button(),
            focus,
        }
    }

    /// The aircraft the AI flies for this connected player, kept for it
    /// while it is away: Release frees it (stage K, message 53).
    pub fn with_release(mut self, plane: Option<u32>) -> Self {
        self.release_plane = plane;
        self.release.set_enabled(plane.is_some());
        if let Some(plane) = plane {
            self.lines.push(format!(
                "Plane {plane} is kept for {}. Release frees it for another player.",
                self.callsign
            ));
        }
        self
    }

    /// The panel for a plane kept for a player who is not in the game (it
    /// dropped): only Release applies.
    pub fn for_slot(plane: u32, callsign: &str) -> Self {
        let mut panel = Self::new(u8::MAX, callsign, false).with_release(Some(plane));
        panel.slot_only = true;
        panel.crown.set_enabled(false);
        panel.kick.set_enabled(false);
        panel.lines = vec![
            format!("Plane {plane} is kept for {callsign}, who is not in the game."),
            "The AI flies it. Release frees it for another player.".to_owned(),
        ];
        panel
    }

    /// The panel is for a reserved slot, not a player.
    pub fn slot_only(&self) -> bool {
        self.slot_only
    }

    /// The plane Release frees, when it can.
    pub fn release_plane(&self) -> Option<u32> {
        self.release_plane
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
            Id::Crown if !self.slot_only => Answer::Crown(self.player),
            Id::Crown => Answer::None,
            Id::Kick if !self.house && !self.slot_only => Answer::Kick(self.player),
            Id::Kick => Answer::None,
            Id::Release => self.release_plane.map_or(Answer::None, Answer::Release),
            Id::Close => Answer::Close,
        }
    }

    pub fn key(&mut self, name: &str, shift: bool) -> Answer {
        if name == "Escape" {
            return Answer::Close;
        }
        let kick_ok = !self.house && !self.slot_only;
        let crown_ok = !self.slot_only;
        let release_ok = self.release_plane.is_some();
        let usable = |id: Id| match id {
            Id::Kick => kick_ok,
            Id::Crown => crown_ok,
            Id::Release => release_ok,
            Id::Close => true,
        };
        match self.focus.key(name, shift, usable) {
            Route::Moved | Route::Ignored | Route::Default(_) => Answer::None,
            Route::Widget(id) => {
                let fired = match id {
                    Id::Crown => self.crown.key(name),
                    Id::Kick => self.kick.key(name),
                    Id::Release => self.release.key(name),
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
        self.release.pointer_move(point);
        self.close.pointer_move(point);
    }

    pub fn button(&mut self, point: Option<Point>, pressed: bool) -> Answer {
        let Some(point) = point else {
            if !pressed {
                for button in [
                    &mut self.crown,
                    &mut self.kick,
                    &mut self.release,
                    &mut self.close,
                ] {
                    button.release((-1, -1));
                }
            }
            return Answer::None;
        };
        if pressed {
            self.crown.press(point);
            self.kick.press(point);
            self.release.press(point);
            self.close.press(point);
            return Answer::None;
        }
        let crown = self.crown.release(point) == Outcome::Activated;
        let kick = self.kick.release(point) == Outcome::Activated;
        let release = self.release.release(point) == Outcome::Activated;
        let close = self.close.release(point) == Outcome::Activated;
        if crown {
            self.press_id(Id::Crown)
        } else if kick {
            self.press_id(Id::Kick)
        } else if release {
            self.press_id(Id::Release)
        } else if close {
            Answer::Close
        } else {
            Answer::None
        }
    }

    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit) {
        draw_panel(canvas, kit, PANEL);
        let font = kit.sprite("PANELFNT");
        let title = if self.slot_only {
            format!("Reserved aircraft: {}", self.callsign)
        } else {
            format!("Players: {}", self.callsign)
        };
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
        self.release
            .draw(canvas, kit, self.focus.marked(Id::Release));
        self.close.draw(canvas, kit, self.focus.marked(Id::Close));
    }
}
