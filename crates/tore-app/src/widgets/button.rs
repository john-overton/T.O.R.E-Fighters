//! Retail buttons: the green action button, the blue default button, and
//! their grey disabled copies.
use super::{Kit, Outcome, Point, Rect, Widget, draw::focus_mark, inside};
use crate::menu::{Canvas, text_width};
use crate::ui_text;

/// The cap's columns the default button uses, from its left (a navy outline
/// line, a grey border, and two clear columns that carry the top and bottom
/// lines across), and from its right (the face's own left rim: a grey border
/// and a dark navy line). The columns between hold the striped box and are
/// not drawn.
const CAP_OUTER: usize = 4;
const CAP_RIM: usize = 2;
const CAP_WIDTH: usize = 20;
/// How far left of the default button's face its outline reaches: the outer
/// columns run up to the face's rim, so the navy line stands six pixels from the
/// face, four from the rim's edge, about as far as the top line's end is
/// beyond the shadow on the right.
const OUTLINE_LEFT: i32 = (CAP_OUTER + CAP_RIM) as i32;

/// A button placed where its dialog places it: `at` is the dialog's position
/// plus the origin (NEWNET's New is at (106, 419)) and `width` the dialog's
/// width (85 on NEWNET), shadow included, as `Canvas::action_button` takes
/// them. The default button is drawn three pixels higher, with the navy
/// outline its pieces carry on top, as `action_button` draws it. Retail adds a
/// 20 pixel cap to its left, a striped box in a frame; here only the cap's
/// outer columns ([`CAP_OUTER`], the outline's left side) and its last two (the
/// face's left rim) are drawn, the outline reaching [`OUTLINE_LEFT`] pixels
/// left of the face, and the striped box is gone (John, 2026-10-05,
/// *opinionated*, requested). He had the outline taken out as well, and had it
/// put back the same day, then asked for the rim and for the outline to stand
/// three or four pixels off the button.
///
/// Reuses `Canvas::button_style` for the pieces. The label is set in the
/// retail button fonts (`FONTACT` on the green face, `FONTDFT` on the blue
/// one, their dim copies when disabled) at the offsets EF0 measured on John's
/// screenshot, not in the Noto `QUICKFONT` that `action_button` uses
/// (*agent decision*: the existing screens' button label font is a
/// workaround for the menu's flat font; the pieces are the retail look).
///
/// States: normal, hover (the face 8 percent brighter), pressed (83 percent
/// and one pixel down, as the menu's buttons), disabled (the grey `ACTIOD0`
/// or `ACTDFD0` pieces and the dim font; ignores everything) and focused (the
/// kit's dotted mark, drawn only when the screen says the button has the
/// keyboard focus).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Button {
    label: String,
    at: Point,
    width: i32,
    default: bool,
    enabled: bool,
    hover: bool,
    pressed: bool,
}

impl Button {
    pub fn new(label: impl Into<String>, at: Point, width: i32) -> Self {
        Self {
            label: label.into(),
            at,
            width,
            default: false,
            enabled: true,
            hover: false,
            pressed: false,
        }
    }
    /// Makes it the screen's default button: blue, with the striped cap.
    pub fn default_button(mut self) -> Self {
        self.default = true;
        self
    }
    #[cfg(test)]
    pub fn is_default(&self) -> bool {
        self.default
    }
    /// Makes the button the screen's default one, or takes that away: a
    /// screen whose Enter goes to a different button as the player works
    /// (Direct Connection's New or Join) moves the blue face between them.
    pub fn set_default(&mut self, default: bool) {
        self.default = default;
    }
    #[cfg(test)]
    pub fn label(&self) -> &str {
        &self.label
    }
    /// Moves the button (a screen whose row of buttons changes with who the
    /// player is, the lobby).
    pub fn place(&mut self, at: Point) {
        self.at = at;
    }
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.hover = false;
            self.pressed = false;
        }
    }
    #[cfg(test)]
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// The pointer moved (`None` when it left the canvas). True when the
    /// hover look changed.
    pub fn pointer_move(&mut self, point: Option<Point>) -> bool {
        let over = point.is_some_and(|p| self.hit(p));
        std::mem::replace(&mut self.hover, over) != over
    }
    /// The mouse went down at `point`: a hit presses the button.
    pub fn press(&mut self, point: Point) -> Outcome {
        if self.hit(point) {
            self.pressed = true;
        }
        Outcome::None
    }
    /// The mouse went up: `Activated` when the button was pressed and the
    /// pointer is still on it.
    pub fn release(&mut self, point: Point) -> Outcome {
        let fired = self.pressed && self.hit(point);
        self.pressed = false;
        if fired {
            Outcome::Activated
        } else {
            Outcome::None
        }
    }
    /// A key while the button has the focus: Enter or Space presses it.
    pub fn key(&mut self, name: &str) -> Outcome {
        if self.enabled && matches!(name, "Enter" | "Space" | " ") {
            Outcome::Activated
        } else {
            Outcome::None
        }
    }

    /// The rectangle of the face as drawn.
    fn face(&self) -> Rect {
        let up = if self.default { 3 } else { 0 };
        (self.at.0, self.at.1 - up, self.width - 5, 27)
    }

    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit, focused: bool) {
        let (x, y) = self.at;
        let top = y - if self.default { 3 } else { 0 };
        let shift = i32::from(self.pressed && self.enabled);
        let gain = if !self.enabled {
            1.0
        } else if self.pressed {
            0.83
        } else if self.hover {
            1.08
        } else {
            1.0
        };
        if self.default {
            let cap = kit.sprite(if self.enabled { "ACTDFLT" } else { "ACTDFLD" });
            let left = x + shift - OUTLINE_LEFT;
            canvas.blit(cap, (left, top + shift), 0, CAP_OUTER, 1.0);
            canvas.blit(
                cap,
                (x + shift - CAP_RIM as i32, top + shift),
                CAP_WIDTH - CAP_RIM,
                CAP_RIM,
                1.0,
            );
        }
        let prefix = match (self.default, self.enabled) {
            (false, true) => "ACTION0",
            (false, false) => "ACTIOD0",
            (true, true) => "ACTDFT0",
            (true, false) => "ACTDFD0",
        };
        canvas.button_style(
            kit.sprites(),
            "",
            (x + shift, top + shift, self.width),
            gain,
            prefix,
        );
        // EF0: label x = x + (73 - width) / 2 for an 85 wide button, one
        // pixel further left on the default button; the glyph strip's top is
        // 6 below the face on a plain button and 10 on the default (whose
        // face is drawn 3 higher).
        let font = kit.sprite(match (self.default, self.enabled) {
            (false, true) => "FONTACT",
            (false, false) => "FONTACD",
            (true, true) => "FONTDFT",
            (true, false) => "FONTDFD",
        });
        let tw = text_width(font, &self.label);
        let lx = x + (self.width - 12 - tw) / 2 - i32::from(self.default);
        let ly = top + if self.default { 10 } else { 6 };
        ui_text::text(
            canvas,
            kit,
            font,
            &self.label,
            (lx + shift, ly + shift),
            None,
            None,
        );
        if focused && self.enabled {
            let (fx, fy, fw, fh) = self.face();
            let marker = if self.default { OUTLINE_LEFT } else { 0 };
            focus_mark(canvas, (fx - 2 - marker, fy - 2, fw + marker - 2, fh - 2));
        }
    }
}

impl Widget for Button {
    /// The face and, for the default button, its outline to the left.
    fn bounds(&self) -> Rect {
        let marker = if self.default { OUTLINE_LEFT } else { 0 };
        (
            self.at.0 - marker,
            self.at.1 - 3,
            self.width + marker - 5,
            27,
        )
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
    use super::super::test_kit::{at, blank, kit, tone_of};
    use super::*;

    fn join() -> Button {
        Button::new("Join", (229, 419), 85)
    }
    fn new() -> Button {
        Button::new("New", (106, 419), 85).default_button()
    }

    #[test]
    fn a_click_that_starts_and_ends_on_the_button_activates_it() {
        let mut b = join();
        assert_eq!(b.press((240, 425)), Outcome::None);
        assert!(b.is_pressed());
        assert_eq!(b.release((240, 425)), Outcome::Activated);
        assert!(!b.is_pressed());
        b.press((240, 425));
        assert_eq!(b.release((10, 10)), Outcome::None, "released elsewhere");
        assert_eq!(b.release((240, 425)), Outcome::None, "never pressed");
        b.press((10, 10));
        assert!(!b.is_pressed(), "a press elsewhere does not press it");
    }

    #[test]
    fn enter_and_space_press_it_while_focused() {
        let mut b = join();
        assert_eq!(b.key("Enter"), Outcome::Activated);
        assert_eq!(b.key("Space"), Outcome::Activated);
        assert_eq!(b.key("ArrowLeft"), Outcome::None);
    }

    #[test]
    fn a_disabled_button_ignores_everything() {
        let mut b = join();
        b.set_enabled(false);
        assert_eq!(b.press((240, 425)), Outcome::None);
        assert!(!b.is_pressed());
        assert_eq!(b.release((240, 425)), Outcome::None);
        assert_eq!(b.key("Enter"), Outcome::None);
        assert!(!b.hit((240, 425)));
        assert!(!b.pointer_move(Some((240, 425))));
        // Disabling mid-press lets go.
        let mut b = join();
        b.press((240, 425));
        b.set_enabled(false);
        assert!(!b.is_pressed());
    }

    #[test]
    fn hover_changes_are_reported_once() {
        let mut b = join();
        assert!(b.pointer_move(Some((240, 425))));
        assert!(!b.pointer_move(Some((241, 425))));
        assert!(b.pointer_move(None));
    }

    #[test]
    fn the_default_buttons_hit_area_includes_its_outline() {
        assert_eq!(new().bounds(), (106 - 6, 419 - 3, 85 + 6 - 5, 27));
        assert_eq!(join().bounds(), (229, 419 - 3, 80, 27));
        assert!(new().hit((101, 425)));
        assert!(!new().hit((90, 425)), "where the striped cap was");
        assert!(!join().hit((225, 425)));
    }

    fn drawn(button: &Button, focused: bool) -> Vec<u8> {
        let mut pixels = blank();
        button.draw(&mut Canvas(&mut pixels), &kit(), focused);
        pixels
    }

    #[test]
    fn the_four_looks_use_their_own_pieces() {
        let kit = kit();
        // Face pixels in the middle piece (past the 24 pixel left cap).
        let face = |b: &Button, x: i32, y: i32| at(&drawn(b, false), x, y);
        let mut b = join();
        assert_eq!(face(&b, 229 + 53, 419 + 15), tone_of(&kit, "ACTION0M"));
        b.set_enabled(false);
        assert_eq!(face(&b, 229 + 53, 419 + 15), tone_of(&kit, "ACTIOD0M"));
        let mut d = new();
        // The default button is drawn 3 higher.
        assert_eq!(face(&d, 106 + 53, 416 + 15), tone_of(&kit, "ACTDFT0M"));
        d.set_enabled(false);
        assert_eq!(face(&d, 106 + 53, 416 + 15), tone_of(&kit, "ACTDFD0M"));
    }

    #[test]
    fn the_default_button_has_its_outline_all_round_and_no_striped_box() {
        let kit = kit();
        for (enabled, cap, piece) in [
            (true, "ACTDFLT", "ACTDFT0M"),
            (false, "ACTDFLD", "ACTDFD0M"),
        ] {
            let mut d = new();
            d.set_enabled(enabled);
            let pixels = drawn(&d, false);
            // The pieces' own top rows, the outline, are drawn: the first
            // row is 3 above where a plain button's face starts.
            for x in 106 + 24..106 + 85 - 29 {
                assert_eq!(at(&pixels, x, 416), tone_of(&kit, piece), "row at {x}");
            }
            // The cap's four outer columns, 6 pixels left of the face down to
            // 3, and its last two, the face's rim, 2 pixels left of it: one
            // run up to the face.
            for x in 106 - 6..106 {
                assert_eq!(
                    at(&pixels, x, 416 + 5),
                    tone_of(&kit, cap),
                    "cap column at {x}"
                );
            }
            // Beyond them, where the striped box is, the canvas shows.
            for x in 106 - 20..106 - 6 {
                for y in 416..443 {
                    assert_eq!(at(&pixels, x, y), [0; 3], "stray pixel at {x}, {y}");
                }
            }
        }
    }

    #[test]
    fn the_default_button_is_drawn_whole_three_pixels_higher() {
        // 33 rows of pieces from 3 above a plain button's top.
        let (plain, default) = (drawn(&join(), false), drawn(&new(), false));
        let rows = |pixels: &[u8], x: i32| {
            let lit = |y: &i32| at(pixels, x, *y) != [0; 3];
            let first = (0..crate::menu::HEIGHT as i32).find(lit);
            let last = (0..crate::menu::HEIGHT as i32).rev().find(lit);
            (first, last)
        };
        assert_eq!(rows(&plain, 229 + 53), (Some(419), Some(419 + 29)));
        assert_eq!(rows(&default, 106 + 53), (Some(416), Some(416 + 32)));
    }

    #[test]
    fn labels_use_the_retail_button_fonts() {
        // FONTACT is 7 pixels a glyph here: "Join" is 28 wide, so the label
        // starts at x + (85 - 12 - 28) / 2 = x + 22 and 6 below the face top.
        let b = join();
        let pixels = drawn(&b, false);
        assert_eq!(at(&pixels, 229 + 22, 419 + 6), [255; 3]);
        assert_ne!(at(&pixels, 229 + 21, 419 + 6), [255; 3]);
        // The default button's label is a pixel further left and one lower.
        let pixels = drawn(&new(), false);
        let x = 106 + (85 - 12 - 21) / 2 - 1;
        assert_eq!(at(&pixels, x, 419 + 7), [255; 3]);
        assert_ne!(at(&pixels, x - 1, 419 + 7), [255; 3]);
    }

    #[test]
    fn pressed_darkens_and_moves_it_down_one_pixel() {
        let kit = kit();
        let mut b = join();
        b.press((240, 425));
        let pixels = drawn(&b, false);
        let [r, g, bl] = tone_of(&kit, "ACTION0M");
        let dark = |c: u8| (c as f32 * 0.83) as u8;
        let got = at(&pixels, 229 + 53, 419 + 16);
        for (a, e) in got.iter().zip([dark(r), dark(g), dark(bl)]) {
            assert!((*a as i32 - e as i32).abs() <= 1, "{got:?}");
        }
        assert_eq!(
            at(&pixels, 229 + 53, 419),
            [0; 3],
            "the top row is no longer covered"
        );
    }

    #[test]
    fn the_focus_mark_is_dotted_and_only_when_asked_and_enabled() {
        let b = join();
        let plain = drawn(&b, false);
        let marked = drawn(&b, true);
        let lit = |p: &[u8]| {
            p.chunks_exact(4)
                .filter(|px| px[..3] == [204, 225, 205])
                .count()
        };
        assert_eq!(lit(&plain), 0);
        assert!(lit(&marked) > 40, "a dotted rectangle round the face");
        let mut off = join();
        off.set_enabled(false);
        assert_eq!(lit(&drawn(&off, true)), 0);
    }

    use crate::menu::Canvas;
}
