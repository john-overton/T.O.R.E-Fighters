//! The check box: retail's lamp, `CHECK00` (off) to `CHECK06` (on), lit
//! through the middle frames.
use super::{Kit, Outcome, Point, Rect, Widget, draw::focus_mark, inside};
use crate::menu::Canvas;
use crate::rocker::Rocker;
use crate::ui_text;
use std::time::Instant;

/// The box's hit size: the DLG's 19 by 19 (the art is 28 to 33 square with
/// its shadow).
const HIT: i32 = 19;
/// The frame the lamp is lit from and the last frame.
const ON: usize = 6;
/// How bright a disabled lamp is drawn when the box dims it.
const DISABLED_GAIN: f32 = 0.5;

/// A check box at `at` with an optional label to its right in `PANELFNT`.
///
/// Frames (EF0, `FA.EXE` `0x48b320`): `CHECK00` is off at rest and `CHECK06`
/// on at rest. Turning on plays `CHECK01` to `CHECK06`, turning off plays
/// `CHECK05` back to `CHECK00`, one frame per 40 ms (the rocker's step, fitted:
/// retail waits a screen update). With the animation off ([`CheckBox::
/// animated`]) only `CHECK00` and `CHECK06` show. A click toggles on release
/// over the box; Space and Enter toggle it while it has the focus.
#[derive(Clone, Debug)]
pub struct CheckBox {
    at: Point,
    label: String,
    checked: bool,
    frame: usize,
    next: Instant,
    animate: bool,
    pressed: bool,
    enabled: bool,
    /// Draw the lamp dimmer while the box is disabled (lobby pass L3's side
    /// boxes); off, a disabled box keeps its art (the connection screens').
    dim_when_disabled: bool,
}

impl CheckBox {
    pub fn new(at: Point, label: impl Into<String>, checked: bool) -> Self {
        Self {
            at,
            label: label.into(),
            checked,
            frame: if checked { ON } else { 0 },
            next: Instant::now(),
            animate: true,
            pressed: false,
            enabled: true,
            dim_when_disabled: false,
        }
    }
    /// A disabled box draws its lamp at half brightness.
    pub fn dimmed_when_disabled(mut self) -> Self {
        self.dim_when_disabled = true;
        self
    }
    /// Turns the frame-by-frame lamp off or on (on by default).
    #[cfg(test)]
    pub fn animated(mut self, animate: bool) -> Self {
        self.animate = animate;
        self
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    pub fn checked(&self) -> bool {
        self.checked
    }
    /// The frame drawn now, 0 to 6.
    #[cfg(test)]
    pub fn frame(&self) -> usize {
        self.frame
    }
    /// Sets the box without a sound or a lamp animation (loading a setting).
    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
        self.frame = if checked { ON } else { 0 };
    }
    /// Turns the box over, starting the lamp's walk through the frames.
    pub fn toggle(&mut self, now: Instant) -> Outcome {
        if !self.enabled {
            return Outcome::None;
        }
        self.checked = !self.checked;
        if self.animate {
            // The lamp starts at once on its first frame: 01 going on, 05
            // going off; a toggle in the middle of a walk turns it round.
            if self.frame == 0 && self.checked {
                self.frame = 1;
            } else if self.frame == ON && !self.checked {
                self.frame = ON - 1;
            }
            self.next = now + Rocker::FRAME;
        } else {
            self.frame = if self.checked { ON } else { 0 };
        }
        Outcome::Changed
    }

    pub fn press(&mut self, point: Point) -> Outcome {
        if self.hit(point) {
            self.pressed = true;
        }
        Outcome::None
    }
    /// The mouse went up: toggles when it went down on the box and is still
    /// over it.
    pub fn release(&mut self, point: Point, now: Instant) -> Outcome {
        let fired = self.pressed && self.hit(point);
        self.pressed = false;
        if fired {
            self.toggle(now)
        } else {
            Outcome::None
        }
    }
    pub fn key(&mut self, name: &str, now: Instant) -> Outcome {
        if matches!(name, "Space" | " " | "Enter") {
            self.toggle(now)
        } else {
            Outcome::None
        }
    }
    /// Walks the lamp one frame per 40 ms toward its rest frame; true while
    /// it is still moving.
    pub fn advance(&mut self, now: Instant) -> bool {
        let rest = if self.checked { ON } else { 0 };
        while self.frame != rest && now >= self.next {
            if self.frame < rest {
                self.frame += 1;
            } else {
                self.frame -= 1;
            }
            self.next += Rocker::FRAME;
        }
        self.frame != rest
    }

    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit, focused: bool) {
        let sprite = kit.sprite(&format!("CHECK0{}", self.frame));
        let gain = if self.dim_when_disabled && !self.enabled {
            DISABLED_GAIN
        } else {
            1.0
        };
        canvas.blit(sprite, self.at, 0, sprite.width, gain);
        if !self.label.is_empty() {
            let font = kit.sprite("PANELFNT");
            let font = if self.enabled {
                font
            } else {
                kit.sprite("PANELFND")
            };
            ui_text::text(
                canvas,
                kit,
                font,
                &self.label,
                (self.at.0 + HIT + 10, self.at.1 + 5),
                None,
                None,
            );
        }
        if focused && self.enabled {
            let (x, y, w, h) = self.bounds();
            focus_mark(canvas, (x - 2, y - 2, w + 4, h + 4));
        }
    }
    /// The width of the box and its label, for placing what follows it.
    #[cfg(test)]
    pub fn width(&self, kit: &Kit) -> i32 {
        HIT + 10 + crate::menu::text_width(kit.sprite("PANELFNT"), &self.label)
    }
}

impl Widget for CheckBox {
    fn bounds(&self) -> Rect {
        (self.at.0, self.at.1, HIT, HIT)
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn hit(&self, point: Point) -> bool {
        self.enabled && inside(self.bounds(), point)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_kit::{at, blank, kit, start, tone_of};
    use super::*;

    fn walk(b: &mut CheckBox, t0: Instant) -> Vec<usize> {
        let mut frames = vec![b.frame()];
        let mut step = 1;
        while b.advance(t0 + Rocker::FRAME * step) {
            frames.push(b.frame());
            step += 1;
        }
        frames.push(b.frame());
        frames.dedup();
        frames
    }

    #[test]
    fn it_rests_on_frame_zero_off_and_six_on() {
        assert_eq!(CheckBox::new((0, 0), "", false).frame(), 0);
        assert_eq!(CheckBox::new((0, 0), "", true).frame(), 6);
    }

    #[test]
    fn turning_on_plays_one_to_six_and_off_plays_five_to_zero() {
        let t0 = start();
        let mut b = CheckBox::new((10, 10), "", false);
        assert_eq!(b.toggle(t0), Outcome::Changed);
        assert!(b.checked());
        assert_eq!(walk(&mut b, t0), [1, 2, 3, 4, 5, 6]);
        assert_eq!(b.toggle(t0), Outcome::Changed);
        assert!(!b.checked());
        assert_eq!(walk(&mut b, t0), [5, 4, 3, 2, 1, 0]);
    }

    #[test]
    fn one_frame_per_forty_milliseconds() {
        let t0 = start();
        let mut b = CheckBox::new((10, 10), "", false);
        b.toggle(t0);
        assert_eq!(b.frame(), 1);
        assert!(b.advance(t0 + Duration::from_millis(39)));
        assert_eq!(b.frame(), 1);
        assert!(b.advance(t0 + Duration::from_millis(41)));
        assert_eq!(b.frame(), 2);
        assert!(!b.advance(t0 + Duration::from_millis(1000)));
        assert_eq!(b.frame(), 6);
    }

    #[test]
    fn a_toggle_in_mid_walk_turns_the_lamp_round() {
        let t0 = start();
        let mut b = CheckBox::new((10, 10), "", false);
        b.toggle(t0);
        b.advance(t0 + Rocker::FRAME * 2);
        assert_eq!(b.frame(), 3);
        b.toggle(t0 + Rocker::FRAME * 2);
        assert_eq!(walk(&mut b, t0 + Rocker::FRAME * 2).first(), Some(&3));
        assert_eq!(b.frame(), 0);
    }

    #[test]
    fn without_the_animation_only_the_rest_frames_show() {
        let t0 = start();
        let mut b = CheckBox::new((10, 10), "", false).animated(false);
        b.toggle(t0);
        assert_eq!((b.frame(), b.advance(t0)), (6, false));
        b.toggle(t0);
        assert_eq!((b.frame(), b.advance(t0)), (0, false));
    }

    #[test]
    fn a_click_toggles_on_release_over_the_box() {
        let t0 = start();
        let mut b = CheckBox::new((10, 10), "", false);
        assert_eq!(b.press((12, 12)), Outcome::None);
        assert_eq!(b.release((12, 12), t0), Outcome::Changed);
        assert!(b.checked());
        b.press((12, 12));
        assert_eq!(b.release((200, 200), t0), Outcome::None, "dragged off");
        assert!(b.checked());
        assert_eq!(b.press((40, 40)), Outcome::None);
        assert_eq!(
            b.release((40, 40), t0),
            Outcome::None,
            "outside the 19 pixel box"
        );
        assert!(b.hit((28, 28)) && !b.hit((29, 29)));
    }

    #[test]
    fn keys_and_a_disabled_box() {
        let t0 = start();
        let mut b = CheckBox::new((10, 10), "", false);
        assert_eq!(b.key("Space", t0), Outcome::Changed);
        assert_eq!(b.key("Enter", t0), Outcome::Changed);
        assert!(!b.checked());
        assert_eq!(b.key("ArrowLeft", t0), Outcome::None);
        b.set_enabled(false);
        assert_eq!(b.key("Space", t0), Outcome::None);
        assert_eq!(b.press((12, 12)), Outcome::None);
        assert!(!b.checked());
    }

    #[test]
    fn it_draws_the_frame_it_is_on() {
        let kit = kit();
        let t0 = start();
        let mut b = CheckBox::new((10, 10), "Full", false);
        let mut pixels = blank();
        b.draw(&mut Canvas(&mut pixels), &kit, false);
        assert_eq!(at(&pixels, 12, 12), tone_of(&kit, "CHECK00"));
        b.toggle(t0);
        b.draw(&mut Canvas(&mut pixels), &kit, false);
        assert_eq!(at(&pixels, 12, 12), tone_of(&kit, "CHECK01"));
        b.advance(t0 + Duration::from_secs(1));
        b.draw(&mut Canvas(&mut pixels), &kit, false);
        assert_eq!(at(&pixels, 12, 12), tone_of(&kit, "CHECK06"));
        // The label sits to the right, 10 pixels clear of the box.
        assert_eq!(at(&pixels, 10 + 19 + 10 + 1, 10 + 5 + 1), [255; 3]);
        assert_eq!(b.width(&kit), 19 + 10 + 4 * 5);
    }

    use crate::menu::Canvas;
    use std::time::Duration;
}
