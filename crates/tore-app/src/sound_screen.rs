//! The Sound/Music Prefs dialog, opened from Pref > Sound... in the main
//! menu and in the paused flight menu. It is drawn from the retail art:
//! `SNDPREF.PIC` carries the panels, labels and slider slots (the original
//! also redraws the slots from `SLIDETOP/MID/BOT.PIC` to erase the knob;
//! redrawing the whole picture each frame makes that unnecessary),
//! `SLIDERV.PIC` is the knob and `TOGGLE00..04.PIC` the channel swap lever. Behaviour, numbers and provenance: docs/spec/sound-prefs.md.
use crate::menu::{Canvas, Sprite, text_width};
use crate::sound_prefs::{MAX, Settings, Slider};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

type Rect = (i32, i32, i32, i32);

/// Where `SNDPREF.PIC` is drawn: centred across, a third of the way down.
const ORIGIN: (i32, i32) = (129, 34);
/// Each slider: its knob's left edge and the knob's top edge at MAX.
const KNOBS: [(Slider, i32, i32); 9] = [
    (Slider::Overall, 189, 123),
    (Slider::Engine, 251, 123),
    (Slider::WeaponLock, 295, 123),
    (Slider::Rwr, 339, 123),
    (Slider::StallWarn, 383, 123),
    (Slider::RadioMsg, 427, 123),
    (Slider::InFlightMusic, 172, 284),
    (Slider::OtherMusic, 230, 284),
    (Slider::StereoSeparation, 323, 270),
];
/// Knob travel from MAX to OFF, in pixels.
const TRAVEL: i32 = 49;
/// A slider's click area, from its knob's MAX position.
const TRACK_SIZE: (i32, i32) = (34, 79);
/// The swap lever's frames from YES (up) to NO (down), where they are drawn
/// and where a click flips it.
const TOGGLE_FRAMES: [&str; 5] = [
    "TOGGLE00.PIC",
    "TOGGLE01.PIC",
    "TOGGLE02.PIC",
    "TOGGLE03.PIC",
    "TOGGLE04.PIC",
];
const TOGGLE_AT: (i32, i32) = (453, 250);
const TOGGLE_HIT: Rect = (449, 250, 35, 48);
/// Each lever frame shows for 26/256 s while the lever moves.
const TOGGLE_FRAME: Duration = Duration::from_micros(101_563);
/// The lever sounds `&SWITCH.11K` as this frame shows.
const SWITCH_FRAME: usize = 2;
/// Cancel (SNDPREF.DLG action 2) and OK (action 1, the default): top-left
/// corners.
const CANCEL: (i32, i32) = (413, 330);
const OK: (i32, i32) = (413, 364);
/// Arrow keys and the wheel move a slider this many levels (opinionated).
const STEP: u8 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    Slider(usize),
    Swap,
    Cancel,
    Ok,
}
const FOCUS_ORDER: [Control; 12] = [
    Control::Slider(0),
    Control::Slider(1),
    Control::Slider(2),
    Control::Slider(3),
    Control::Slider(4),
    Control::Slider(5),
    Control::Slider(6),
    Control::Slider(7),
    Control::Slider(8),
    Control::Swap,
    Control::Cancel,
    Control::Ok,
];

/// What the app does after an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    /// Only the picture changed.
    Redraw,
    /// A level changed; the mixer follows [`Screen::heard`].
    Preview,
    /// OK: keep and save the draft.
    Save,
    /// Cancel or Escape: back to the settings the dialog opened with.
    Cancel,
}

/// The lever moving towards a setting: the frame showing and when it did.
#[derive(Clone, Copy, Debug)]
struct Lever {
    frame: usize,
    since: Instant,
    /// The switch sound is still to play on this move.
    sound: bool,
}

pub struct Screen {
    /// The settings shown; they take effect on OK.
    pub draft: Settings,
    /// The settings the dialog opened with, restored by Cancel.
    pub original: Settings,
    /// True over the paused flight rather than the main menu.
    pub in_flight: bool,
    hover: Option<Control>,
    pressed: Option<Control>,
    /// The slider being dragged and the knob-relative grab point.
    drag: Option<(usize, i32)>,
    focus: Option<Control>,
    lever: Lever,
    /// The lever reached its sound frame since the host last asked.
    switch_sound: bool,
}

impl Screen {
    pub fn new(settings: Settings, in_flight: bool) -> Self {
        Self {
            draft: settings,
            original: settings,
            in_flight,
            hover: None,
            pressed: None,
            drag: None,
            focus: None,
            lever: Lever {
                frame: lever_frame(settings.swap),
                since: Instant::now(),
                sound: false,
            },
            switch_sound: false,
        }
    }
    pub fn cancel_press(&mut self) {
        self.pressed = None;
        self.drag = None;
    }
    /// What the mixer plays while the dialog is open: the settings it opened
    /// with, except Other music, which follows its knob at once.
    pub fn heard(&self) -> Settings {
        let mut heard = self.original;
        heard.set(Slider::OtherMusic, self.draft.level(Slider::OtherMusic));
        heard
    }
    /// True once each time the lever reaches the frame that sounds it.
    pub fn take_switch_sound(&mut self) -> bool {
        std::mem::take(&mut self.switch_sound)
    }

    // ---- geometry -------------------------------------------------------

    /// Knob offset below MAX for a level; the inverse of [`level_at`].
    fn offset(level: u8) -> i32 {
        ((i32::from(MAX) - i32::from(level)) * TRAVEL + 50) / 100
    }
    fn knob(&self, index: usize) -> (i32, i32) {
        let (slider, x, top) = KNOBS[index];
        (x, top + Self::offset(self.draft.level(slider)))
    }
    fn track(index: usize) -> Rect {
        let (_, x, top) = KNOBS[index];
        (x, top, TRACK_SIZE.0, TRACK_SIZE.1)
    }
    fn button_width(sprites: Option<&BTreeMap<String, Sprite>>, label: &str) -> i32 {
        sprites
            .and_then(|s| s.get("QUICKFONT"))
            .map_or(0, |font| text_width(font, label) + 24)
            .max(61)
    }
    fn button_rect((x, y): (i32, i32), width: i32) -> Rect {
        (x, y, width - 5, 20)
    }
    fn hit(&self, point: Option<(f64, f64)>) -> Option<Control> {
        let (x, y) = point?;
        let inside = |(rx, ry, rw, rh): Rect| {
            x >= f64::from(rx)
                && y >= f64::from(ry)
                && x < f64::from(rx + rw)
                && y < f64::from(ry + rh)
        };
        if inside(Self::button_rect(
            CANCEL,
            Self::button_width(None, "Cancel"),
        )) {
            return Some(Control::Cancel);
        }
        if inside(Self::button_rect(OK, Self::button_width(None, "OK"))) {
            return Some(Control::Ok);
        }
        if inside(TOGGLE_HIT) {
            return Some(Control::Swap);
        }
        (0..KNOBS.len())
            .find(|i| inside(Self::track(*i)))
            .map(Control::Slider)
    }

    // ---- input ----------------------------------------------------------

    /// The lever is moving: the original takes no input until it stops.
    fn busy(&self) -> bool {
        self.lever.frame != lever_frame(self.draft.swap)
    }
    /// The pointer moved; a held knob follows it.
    pub fn moved(&mut self, point: Option<(f64, f64)>) -> Outcome {
        if let (Some((index, grab)), Some((_, y))) = (self.drag, point) {
            let (_, _, top) = KNOBS[index];
            return self.set(index, level_at(y as i32 - grab - top));
        }
        let hover = self.hit(point);
        if hover != self.hover {
            self.hover = hover;
            return Outcome::Redraw;
        }
        Outcome::None
    }
    /// The left button went down or up at `point`.
    pub fn button(&mut self, point: Option<(f64, f64)>, pressed: bool) -> Outcome {
        self.hover = self.hit(point);
        if !pressed {
            let was = self.pressed.take();
            if self.drag.take().is_some() {
                return Outcome::Redraw;
            }
            return match was.filter(|c| Some(*c) == self.hover) {
                Some(Control::Ok) => Outcome::Save,
                Some(Control::Cancel) => self.restore(),
                _ => Outcome::Redraw,
            };
        }
        if self.busy() {
            return Outcome::None;
        }
        self.focus = None;
        self.pressed = self.hover;
        match self.hover {
            Some(Control::Slider(index)) => {
                let (_, y) = point.expect("hit needs a point");
                let (_, top) = self.knob(index);
                // Grabbing the knob keeps its grip and its level until it
                // moves; a press elsewhere on the track puts the knob's
                // centre under the pointer, then drags.
                if (top..top + 30).contains(&(y as i32)) {
                    self.drag = Some((index, y as i32 - top));
                    return Outcome::Redraw;
                }
                self.drag = Some((index, 15));
                let (_, _, max) = KNOBS[index];
                self.set(index, level_at(y as i32 - 15 - max))
            }
            Some(Control::Swap) => self.flip(),
            Some(_) => Outcome::Redraw,
            None => Outcome::None,
        }
    }
    /// The wheel over a slider moves it (opinionated addition).
    pub fn wheel(&mut self, notches: i32) -> Outcome {
        let Some(Control::Slider(index)) = self.hover.or(self.focus) else {
            return Outcome::None;
        };
        self.nudge(index, notches)
    }
    /// Enter is OK and Escape is Cancel, as in the original. Focus, arrows
    /// and Space are opinionated additions for keyboard and controller play.
    pub fn key(&mut self, key: &str, shift: bool) -> Outcome {
        if self.busy() && key != "Escape" {
            return Outcome::None;
        }
        match key {
            "Escape" => self.restore(),
            "Enter" => match self.focus {
                Some(Control::Cancel) => self.restore(),
                Some(Control::Swap) => self.flip(),
                _ => Outcome::Save,
            },
            "Space" | " " => match self.focus {
                Some(Control::Swap) => self.flip(),
                Some(Control::Cancel) => self.restore(),
                Some(Control::Ok) => Outcome::Save,
                _ => Outcome::None,
            },
            "Tab" | "ArrowLeft" | "ArrowRight" => {
                let back = (key == "Tab" && shift) || key == "ArrowLeft";
                let n = FOCUS_ORDER.len();
                let at = self
                    .focus
                    .and_then(|f| FOCUS_ORDER.iter().position(|c| *c == f));
                let next = match (at, back) {
                    (None, false) => 0,
                    (None, true) => n - 1,
                    (Some(i), false) => (i + 1) % n,
                    (Some(i), true) => (i + n - 1) % n,
                };
                self.focus = Some(FOCUS_ORDER[next]);
                Outcome::Redraw
            }
            "ArrowUp" | "ArrowDown" | "PageUp" | "PageDown" | "Home" | "End" => {
                let Some(Control::Slider(index)) = self.focus else {
                    if self.focus.is_none() {
                        self.focus = Some(FOCUS_ORDER[0]);
                        return Outcome::Redraw;
                    }
                    return Outcome::None;
                };
                match key {
                    "Home" => self.set(index, MAX),
                    "End" => self.set(index, 0),
                    "ArrowUp" => self.nudge(index, 1),
                    "ArrowDown" => self.nudge(index, -1),
                    "PageUp" => self.nudge(index, 4),
                    _ => self.nudge(index, -4),
                }
            }
            _ => Outcome::None,
        }
    }

    // ---- editing --------------------------------------------------------

    fn set(&mut self, index: usize, level: u8) -> Outcome {
        let slider = KNOBS[index].0;
        if self.draft.level(slider) == level {
            return Outcome::Redraw;
        }
        self.draft.set(slider, level);
        Outcome::Preview
    }
    fn nudge(&mut self, index: usize, steps: i32) -> Outcome {
        let level = i32::from(self.draft.level(KNOBS[index].0));
        let level = (level + steps * i32::from(STEP)).clamp(0, i32::from(MAX));
        self.set(index, level as u8)
    }
    fn flip(&mut self) -> Outcome {
        self.draft.swap = !self.draft.swap;
        // The lever leaves its end position at once: 03 towards YES, 01
        // towards NO.
        self.lever = Lever {
            frame: if self.draft.swap { 3 } else { 1 },
            since: Instant::now(),
            sound: true,
        };
        Outcome::Redraw
    }
    fn restore(&mut self) -> Outcome {
        self.draft = self.original;
        Outcome::Cancel
    }

    // ---- drawing --------------------------------------------------------

    /// Steps the lever's animation; true while it still moves.
    pub fn animate(&mut self) -> bool {
        let target = lever_frame(self.draft.swap);
        let now = Instant::now();
        loop {
            if self.lever.sound && self.lever.frame == SWITCH_FRAME {
                self.lever.sound = false;
                self.switch_sound = true;
            }
            if self.lever.frame == target || now - self.lever.since < TOGGLE_FRAME {
                break;
            }
            self.lever.frame = if self.lever.frame < target {
                self.lever.frame + 1
            } else {
                self.lever.frame - 1
            };
            self.lever.since += TOGGLE_FRAME;
        }
        self.lever.frame != target
    }
    pub fn draw(&self, pixels: &mut [u8], sprites: &BTreeMap<String, Sprite>) {
        let mut c = Canvas(pixels);
        let art = &sprites["SNDPREF.PIC"];
        c.blit(art, ORIGIN, 0, art.width, 1.);
        let knob = &sprites["SLIDERV.PIC"];
        for index in 0..KNOBS.len() {
            let (kx, ky) = self.knob(index);
            c.blit(knob, (kx, ky), 0, knob.width, 1.);
            if self.focus == Some(Control::Slider(index)) {
                c.outline(
                    (
                        kx - 2,
                        ky - 2,
                        knob.width as i32 + 4,
                        knob.height as i32 + 4,
                    ),
                    FOCUS,
                );
            }
        }
        let lever = &sprites[TOGGLE_FRAMES[self.lever.frame]];
        c.blit(lever, TOGGLE_AT, 0, lever.width, 1.);
        if self.focus == Some(Control::Swap) {
            let (x, y, w, h) = TOGGLE_HIT;
            c.outline((x - 2, y - 2, w + 4, h + 4), FOCUS);
        }
        let down = |control| self.pressed == Some(control) && self.hover == Some(control);
        for (control, label, (x, y), default) in [
            (Control::Cancel, "Cancel", CANCEL, false),
            (Control::Ok, "OK", OK, true),
        ] {
            let width = Self::button_width(Some(sprites), label);
            let rect = c.action_button(sprites, label, (x, y, width), default, down(control));
            if self.focus == Some(control) {
                c.outline(rect, FOCUS);
            }
        }
    }
}

/// Keyboard focus outline, the main menu's colour.
const FOCUS: [u8; 4] = [204, 225, 205, 255];

/// The level for a knob `offset` pixels below MAX, as the original rounds
/// it: only 50 levels can be reached by dragging.
fn level_at(offset: i32) -> u8 {
    let d = offset.clamp(0, TRAVEL);
    (i32::from(MAX) - (100 * d + 48) / 49) as u8
}

/// The lever frame at rest for a setting: up is YES.
fn lever_frame(swap: bool) -> usize {
    if swap { 0 } else { TOGGLE_FRAMES.len() - 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn centre(r: Rect) -> Option<(f64, f64)> {
        Some((f64::from(r.0 + r.2 / 2), f64::from(r.1 + r.3 / 2)))
    }
    fn knob_centre(s: &Screen, index: usize) -> Option<(f64, f64)> {
        let (x, y) = s.knob(index);
        centre((x, y, 26, 30))
    }
    #[test]
    fn levels_follow_the_original_pixel_mapping() {
        assert_eq!(level_at(0), 100);
        assert_eq!(level_at(1), 97);
        assert_eq!(level_at(49), 0);
        assert_eq!(level_at(80), 0);
        assert_eq!(level_at(-5), 100);
        for d in 0..=TRAVEL {
            assert_eq!(Screen::offset(level_at(d)), d, "{d}");
        }
        // The defaults sit between reachable levels.
        assert_eq!(Screen::offset(80), 10);
    }
    #[test]
    fn dragging_a_knob_sets_its_level_and_cancel_restores() {
        let mut s = Screen::new(Settings::default(), false);
        let knob = knob_centre(&s, 1);
        assert_eq!(
            s.button(knob, true),
            Outcome::Redraw,
            "grabbing does not move it"
        );
        let (x, y) = knob.unwrap();
        assert_eq!(s.moved(Some((x, y + 200.))), Outcome::Preview);
        assert_eq!(s.draft.level(Slider::Engine), 0, "clamped at OFF");
        s.moved(Some((x, y - 200.)));
        assert_eq!(s.draft.level(Slider::Engine), MAX);
        assert_eq!(s.button(Some((x, y)), false), Outcome::Redraw);
        assert_ne!(s.moved(Some((x, y + 200.))), Outcome::Preview, "released");
        assert_eq!(s.draft.level(Slider::Engine), MAX);
        assert_eq!(s.key("Escape", false), Outcome::Cancel);
        assert_eq!(s.draft, Settings::default());
    }
    #[test]
    fn only_other_music_is_heard_before_ok() {
        let mut s = Screen::new(Settings::default(), false);
        s.draft.set(Slider::Engine, 0);
        s.draft.set(Slider::OtherMusic, 10);
        let heard = s.heard();
        assert_eq!(heard.level(Slider::Engine), 80);
        assert_eq!(heard.level(Slider::OtherMusic), 10);
    }
    #[test]
    fn clicking_the_track_centres_the_knob_under_the_pointer() {
        let mut s = Screen::new(Settings::default(), false);
        let (x, y, _, h) = Screen::track(0);
        let point = Some((f64::from(x + 5), f64::from(y + h - 1)));
        assert_eq!(s.button(point, true), Outcome::Preview);
        assert_eq!(s.draft.level(Slider::Overall), 0);
    }
    #[test]
    fn lever_flips_animates_sounds_once_and_blocks_input_meanwhile() {
        let mut s = Screen::new(Settings::default(), false);
        assert_eq!(s.lever.frame, 4, "NO is down");
        let at = centre(TOGGLE_HIT);
        assert_eq!(s.button(at, true), Outcome::Redraw);
        assert!(s.draft.swap);
        assert_eq!(s.lever.frame, 3);
        assert_eq!(s.button(at, false), Outcome::Redraw);
        assert_eq!(s.button(at, true), Outcome::None, "busy while it moves");
        s.lever.since -= TOGGLE_FRAME;
        assert!(s.animate());
        assert_eq!(s.lever.frame, 2);
        assert!(s.take_switch_sound());
        assert!(!s.take_switch_sound());
        s.lever.since -= TOGGLE_FRAME * 5;
        assert!(!s.animate());
        assert_eq!(s.lever.frame, 0);
        assert!(!s.take_switch_sound(), "one sound per flip");
    }
    #[test]
    fn ok_needs_press_and_release_on_it() {
        let mut s = Screen::new(Settings::default(), false);
        let ok = centre(Screen::button_rect(OK, 61));
        let cancel = centre(Screen::button_rect(CANCEL, 61));
        s.button(ok, true);
        assert_eq!(s.button(cancel, false), Outcome::Redraw);
        s.button(ok, true);
        assert_eq!(s.button(ok, false), Outcome::Save);
    }
    #[test]
    fn keyboard_moves_focus_and_levels() {
        let mut s = Screen::new(Settings::default(), false);
        assert_eq!(s.key("ArrowDown", false), Outcome::Redraw, "first focus");
        assert_eq!(s.key("ArrowDown", false), Outcome::Preview);
        assert_eq!(s.draft.level(Slider::Overall), 75 - STEP);
        s.key("End", false);
        assert_eq!(s.draft.level(Slider::Overall), 0);
        for _ in 0..9 {
            s.key("Tab", false);
        }
        assert_eq!(s.focus, Some(Control::Swap));
        assert_eq!(s.key("Space", false), Outcome::Redraw);
        assert!(s.draft.swap);
        assert_eq!(s.key("Enter", false), Outcome::None, "busy while it moves");
        s.lever.since -= TOGGLE_FRAME * 5;
        s.animate();
        s.key("Tab", false);
        s.key("Tab", false);
        assert_eq!(s.key("Enter", false), Outcome::Save);
    }
    #[test]
    fn wheel_moves_the_hovered_slider() {
        let mut s = Screen::new(Settings::default(), false);
        s.moved(centre(Screen::track(8)));
        assert_eq!(s.wheel(-2), Outcome::Preview);
        assert_eq!(s.draft.level(Slider::StereoSeparation), 80 - 2 * STEP);
    }
}
