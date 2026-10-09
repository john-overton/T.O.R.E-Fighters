//! The scrolling message box: wrapped, coloured lines, newest at the bottom,
//! with a way back through what scrolled off.
use super::{
    Kit, Outcome, Point, Rect, Widget,
    draw::focus_mark,
    scroll_bar::{self, Press, ScrollBar, View},
};
use crate::menu::{Canvas, text_width};
use crate::ui_text;
use std::collections::VecDeque;
use tore_formats::text::GlyphCodes;

/// The colours lines are drawn in on the panel's grey (*agent decision*:
/// the design's three, green for the player's own side, blue for a line to
/// everyone from the player's side, red from the enemy side, lightened to read
/// on the dark grey, and a pale grey for the game's own words).
pub mod tone {
    /// What the game says: joins, refusals, status.
    pub const SYSTEM: [u8; 3] = [214, 214, 208];
    /// Chat to the player's side, wing or target.
    pub const OWN_SIDE: [u8; 3] = [116, 232, 124];
    /// Chat to everyone from the player's side.
    pub const ALL: [u8; 3] = [128, 176, 255];
    /// Chat from the enemy side.
    pub const ENEMY: [u8; 3] = [255, 118, 104];
}

/// Lines kept unless the screen says otherwise (visual lines, after
/// wrapping): enough to scroll back through a long lobby.
pub const DEFAULT_CAP: usize = 200;
/// `PANELFNT`'s line height; the box holds its height less 8, over this.
const LINE: i32 = 10;
/// Inner margin.
const PAD: i32 = 4;
/// The scroll bar's art sits flush right inside the box's one pixel frame; the
/// text stops 3 pixels short of it (the plan's 549 wide box has 507 for text).
const BAR_GAP: i32 = 3;
/// Lines the wheel moves per notch.
const WHEEL_LINES: i32 = 3;

#[derive(Clone, Debug)]
struct Line {
    text: String,
    colour: [u8; 3],
}

/// NEWNET's Messages area is 549 by 88: eight lines of 10 pixels in
/// `PANELFNT` and 8 over ([`MessageBox::newnet`]). A line longer than the box
/// wraps at spaces (a word longer than the box breaks where it must), and
/// the wrapped part is indented two spaces. The newest line is the last on
/// the box; fewer lines than the box holds start at its top (*agent
/// decisions*).
///
/// Scrolling back (*agent decision*): the wheel moves three lines a notch,
/// PageUp and PageDown a page less a line, Up and Down a line, Home and End
/// to the oldest and newest, while the box has the focus (or under the
/// pointer for the wheel). The retail scroll bar ([`ScrollBar`], the Sound
/// Prefs slider's red knob in its grey track, 34 wide) runs down the right
/// edge inside the frame and shows the place in the kept lines when they do
/// not all fit; a click above or below its knob pages back or forward (a page
/// less a line) and the knob can be dragged ([`MessageBox::drag`], with
/// [`MessageBox::release`] to let go). The track is always drawn, so the text
/// does not move when the bar is needed. A view scrolled back stays on the
/// same words as new lines arrive; at the bottom it follows them.
#[derive(Clone, Debug)]
pub struct MessageBox {
    rect: Rect,
    lines: VecDeque<Line>,
    cap: usize,
    /// How many lines the view is scrolled back from the newest.
    back: usize,
    bar: ScrollBar,
    enabled: bool,
}

impl MessageBox {
    pub fn new(rect: Rect) -> Self {
        let (x, y, w, h) = rect;
        Self {
            rect,
            lines: VecDeque::new(),
            cap: DEFAULT_CAP,
            back: 0,
            bar: ScrollBar::new((x + w - 1 - scroll_bar::WIDTH, y + 1), h - 2),
            enabled: true,
        }
    }
    /// NEWNET's Messages area: (45, 319), 549 by 88.
    pub fn newnet() -> Self {
        Self::new((45, 319, 549, 88))
    }
    /// A different number of kept lines (at least a screenful).
    #[cfg(test)]
    pub fn with_cap(mut self, cap: usize) -> Self {
        self.cap = cap.max(self.visible());
        self
    }

    /// How many lines the box shows at once.
    pub fn visible(&self) -> usize {
        ((self.rect.3 - 2 * PAD) / LINE).max(1) as usize
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.lines.len()
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
    /// The kept lines, oldest first, as wrapped.
    #[cfg(test)]
    pub fn lines(&self) -> impl Iterator<Item = (&str, [u8; 3])> {
        self.lines.iter().map(|l| (l.text.as_str(), l.colour))
    }
    /// The lines on view, top to bottom.
    pub fn view(&self) -> Vec<(&str, [u8; 3])> {
        let end = self.lines.len() - self.back;
        let start = end.saturating_sub(self.visible());
        self.lines
            .range(start..end)
            .map(|l| (l.text.as_str(), l.colour))
            .collect()
    }
    /// How far the view is scrolled back, in lines (0 at the newest).
    #[cfg(test)]
    pub fn scrolled_back(&self) -> usize {
        self.back
    }
    fn max_back(&self) -> usize {
        self.lines.len().saturating_sub(self.visible())
    }
    fn text_room(&self) -> i32 {
        self.rect.2 - PAD - 1 - scroll_bar::WIDTH - BAR_GAP
    }
    /// What the scroll bar is asked about. The knob's line is the first one on
    /// view, so at the newest the knob is at the bottom.
    fn view_of_bar(&self) -> View {
        View {
            total: self.lines.len(),
            visible: self.visible(),
            first: self.max_back() - self.back,
        }
    }

    /// Adds a line, wrapped to the box in `PANELFNT`, in `colour`.
    pub fn push(&mut self, kit: &Kit, text: &str, colour: [u8; 3]) {
        let before = self.lines.len();
        for piece in wrap(kit, text, self.text_room()) {
            self.lines.push_back(Line {
                text: piece,
                colour,
            });
        }
        let added = self.lines.len() - before;
        if self.back > 0 {
            self.back += added;
        }
        while self.lines.len() > self.cap {
            self.lines.pop_front();
        }
        self.back = self.back.min(self.max_back());
    }

    /// Scrolls the view: positive goes back in time, negative toward the
    /// newest. `Changed` when it moved.
    pub fn scroll(&mut self, lines: i32) -> Outcome {
        let to = (self.back as i64 + i64::from(lines)).clamp(0, self.max_back() as i64) as usize;
        if std::mem::replace(&mut self.back, to) == to {
            Outcome::None
        } else {
            Outcome::Changed
        }
    }
    /// A wheel step: positive is up, back in time.
    pub fn wheel(&mut self, notches: i32) -> Outcome {
        if self.enabled {
            self.scroll(notches * WHEEL_LINES)
        } else {
            Outcome::None
        }
    }
    /// A key while the box has the focus.
    pub fn key(&mut self, name: &str) -> Outcome {
        if !self.enabled {
            return Outcome::None;
        }
        let page = self.visible().saturating_sub(1).max(1) as i32;
        match name {
            "ArrowUp" => self.scroll(1),
            "ArrowDown" => self.scroll(-1),
            "PageUp" => self.scroll(page),
            "PageDown" => self.scroll(-page),
            "Home" => self.scroll(i32::MAX / 2),
            "End" => self.scroll(i32::MIN / 2),
            _ => Outcome::None,
        }
    }
    /// The mouse went down: the knob is grabbed, and a click on the track
    /// above it pages back, below it forward.
    pub fn press(&mut self, point: Point) -> Outcome {
        if !self.enabled {
            return Outcome::None;
        }
        let page = self.visible().saturating_sub(1).max(1) as i32;
        let view = self.view_of_bar();
        match self.bar.press(point, view) {
            Press::Up => self.scroll(page),
            Press::Down => self.scroll(-page),
            Press::Grab | Press::Miss => Outcome::None,
        }
    }
    /// The pointer moved to `point`: a held knob scrolls the view. Call it on
    /// every pointer move; it does nothing when no knob is held.
    pub fn drag(&mut self, point: Point) -> Outcome {
        let view = self.view_of_bar();
        match self.bar.drag(point, view) {
            Some(first) => {
                let to = self.max_back() - first;
                if std::mem::replace(&mut self.back, to) == to {
                    Outcome::None
                } else {
                    Outcome::Changed
                }
            }
            None => Outcome::None,
        }
    }
    /// The mouse went up, or the window lost it: the knob is let go.
    pub fn release(&mut self) {
        self.bar.release();
    }
    /// True while the scroll bar's knob is held.
    #[cfg(test)]
    pub fn dragging(&self) -> bool {
        self.bar.dragging()
    }
    /// True over the scroll bar's art, which sits inside the box.
    #[cfg(test)]
    pub fn on_bar(&self, point: Point) -> bool {
        self.bar.hit(point)
    }

    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit, focused: bool) {
        let (x, y, w, h) = self.rect;
        // Measured on John's screenshot (EF0): a flat grey 81 inside a one
        // pixel frame of the panel's line colour.
        canvas.rect((x, y, w, h), [81, 81, 81, 255]);
        canvas.outline((x, y, w, h), [174, 174, 174, 255]);
        let font = kit.sprite("PANELFNT");
        let clip = (x + 1, y + 1, w - 2, h - 2);
        for (i, (text, colour)) in self.view().into_iter().enumerate() {
            ui_text::text(
                canvas,
                kit,
                font,
                text,
                (x + PAD, y + PAD + i as i32 * LINE),
                Some(clip),
                Some(colour),
            );
        }
        self.bar.draw(canvas, kit, self.view_of_bar());
        if focused && self.enabled {
            focus_mark(canvas, (x - 3, y - 3, w + 6, h + 6));
        }
    }
}

impl Widget for MessageBox {
    fn bounds(&self) -> Rect {
        self.rect
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
}

/// `text` in lines no wider than `room` pixels of `PANELFNT`, broken at
/// spaces; later lines start with two spaces.
fn wrap(kit: &Kit, text: &str, room: i32) -> Vec<String> {
    let font = kit.sprite("PANELFNT");
    let width = |s: &str| text_width(font, s);
    let glyph = |c: char| -> i32 {
        let code = std::iter::once(c)
            .collect::<String>()
            .glyph_codes()
            .next()
            .unwrap_or(b'?');
        font.glyphs[code as usize][1] as i32
    };
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut used = 0;
    let space = glyph(' ');
    for word in text.split(' ') {
        let indent = if lines.is_empty() { "" } else { "  " };
        let word_width = width(word);
        let needs_space = !line.is_empty() && line != indent;
        let extra = if needs_space { space } else { 0 };
        if used + extra + word_width <= room {
            if needs_space {
                line.push(' ');
            }
            line.push_str(word);
            used += extra + word_width;
            continue;
        }
        if !line.is_empty() && line != indent {
            lines.push(std::mem::take(&mut line));
        }
        let indent = if lines.is_empty() { "" } else { "  " };
        line = indent.to_owned();
        used = width(indent);
        // A word wider than a whole line breaks where it must.
        for c in word.chars() {
            let w = glyph(c);
            if used + w > room && !line.trim().is_empty() {
                lines.push(std::mem::take(&mut line));
                line = "  ".to_owned();
                used = width("  ");
            }
            line.push(c);
            used += w;
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::super::test_kit::{at, blank, kit, tone_of};
    use super::*;

    /// A box 100 wide (86 pixels of text: 17 five pixel characters) with four
    /// lines on view.
    fn small() -> MessageBox {
        MessageBox::new((20, 20, 100, 48))
    }
    fn texts(b: &MessageBox) -> Vec<String> {
        b.view().into_iter().map(|(t, _)| t.to_owned()).collect()
    }

    #[test]
    fn newnet_holds_eight_lines() {
        let b = MessageBox::newnet();
        assert_eq!((b.visible(), b.bounds()), (8, (45, 319, 549, 88)));
        assert!(b.is_empty());
        assert!(b.view().is_empty());
    }

    #[test]
    fn lines_wrap_at_spaces_and_the_rest_is_indented() {
        let kit = kit();
        let mut b = small();
        let room = b.text_room();
        // 5 pixel glyphs: the room fits this many characters.
        let fits = (room / 5) as usize;
        let words: Vec<String> = (0..6).map(|i| format!("w{i}word")).collect();
        let text = words.join(" ");
        b.push(&kit, &text, tone::ALL);
        assert!(b.len() > 1, "{room} pixels fit {fits} characters");
        for (line, colour) in b.lines() {
            assert!(line.len() <= fits, "{line:?} is too wide");
            assert_eq!(colour, tone::ALL, "each wrapped line keeps the colour");
        }
        let lines: Vec<&str> = b.lines().map(|(t, _)| t).collect();
        assert!(!lines[0].starts_with(' '));
        assert!(lines[1..].iter().all(|l| l.starts_with("  ")));
        // No word is lost or split, and the order is kept.
        let joined: Vec<&str> = lines.iter().flat_map(|l| l.split_whitespace()).collect();
        assert_eq!(joined, words.iter().map(String::as_str).collect::<Vec<_>>());
    }

    #[test]
    fn a_word_wider_than_the_box_breaks_where_it_must() {
        let kit = kit();
        let mut b = small();
        b.push(&kit, &"x".repeat(40), tone::SYSTEM);
        assert!(b.len() >= 3);
        let total: usize = b.lines().map(|(l, _)| l.trim_start().len()).sum();
        assert_eq!(total, 40);
        let fits = (b.text_room() / 5) as usize;
        assert!(b.lines().all(|(l, _)| l.len() <= fits));
    }

    #[test]
    fn empty_and_short_lines_are_kept() {
        let kit = kit();
        let mut b = small();
        b.push(&kit, "", tone::SYSTEM);
        b.push(&kit, "ok", tone::ENEMY);
        assert_eq!(b.len(), 2);
        assert_eq!(texts(&b), ["", "ok"]);
        assert_eq!(b.view()[1].1, tone::ENEMY);
    }

    #[test]
    fn the_newest_line_is_last_and_old_ones_scroll_off_the_top() {
        let kit = kit();
        let mut b = small();
        for i in 0..6 {
            b.push(&kit, &format!("line {i}"), tone::SYSTEM);
        }
        assert_eq!(b.visible(), 4);
        assert_eq!(texts(&b), ["line 2", "line 3", "line 4", "line 5"]);
    }

    #[test]
    fn the_cap_drops_the_oldest_lines() {
        let kit = kit();
        let mut b = small().with_cap(5);
        for i in 0..9 {
            b.push(&kit, &format!("l{i}"), tone::SYSTEM);
        }
        let all: Vec<&str> = b.lines().map(|(t, _)| t).collect();
        assert_eq!(all, ["l4", "l5", "l6", "l7", "l8"]);
        assert_eq!(small().with_cap(1).cap, 4, "never less than a screenful");
    }

    #[test]
    fn scrolling_back_with_wheel_and_keys() {
        let kit = kit();
        let mut b = small();
        for i in 0..10 {
            b.push(&kit, &format!("l{i}"), tone::SYSTEM);
        }
        assert_eq!(b.scrolled_back(), 0);
        assert_eq!(b.key("ArrowDown"), Outcome::None, "already at the newest");
        assert_eq!(b.wheel(1), Outcome::Changed);
        assert_eq!(b.scrolled_back(), 3);
        assert_eq!(texts(&b), ["l3", "l4", "l5", "l6"]);
        assert_eq!(b.key("PageUp"), Outcome::Changed);
        assert_eq!(
            b.scrolled_back(),
            6,
            "ten lines, four on view: at most six back"
        );
        assert_eq!(b.key("ArrowUp"), Outcome::None, "clamped at the oldest");
        assert_eq!(texts(&b), ["l0", "l1", "l2", "l3"]);
        assert_eq!(b.key("End"), Outcome::Changed);
        assert_eq!(b.scrolled_back(), 0);
        b.key("Home");
        assert_eq!(b.scrolled_back(), 6);
        b.key("ArrowDown");
        assert_eq!(b.scrolled_back(), 5);
        b.wheel(-1);
        assert_eq!(b.scrolled_back(), 2);
        b.key("PageDown");
        assert_eq!(b.scrolled_back(), 0);
        assert_eq!(b.wheel(-5), Outcome::None);
    }

    #[test]
    fn a_scrolled_view_stays_on_its_words_but_the_newest_view_follows() {
        let kit = kit();
        let mut b = small();
        for i in 0..10 {
            b.push(&kit, &format!("l{i}"), tone::SYSTEM);
        }
        b.wheel(1);
        let before = texts(&b);
        b.push(&kit, "l10", tone::SYSTEM);
        assert_eq!(texts(&b), before, "reading back is not disturbed");
        assert_eq!(b.scrolled_back(), 4);
        b.key("End");
        b.push(&kit, "l11", tone::SYSTEM);
        assert_eq!(texts(&b).last().unwrap(), "l11");
        // Eviction at the cap while scrolled back never leaves the view out of range.
        let mut b = small().with_cap(6);
        for i in 0..6 {
            b.push(&kit, &format!("l{i}"), tone::SYSTEM);
        }
        b.key("Home");
        for i in 6..20 {
            b.push(&kit, &format!("l{i}"), tone::SYSTEM);
        }
        assert!(b.scrolled_back() <= 2);
        assert_eq!(texts(&b).len(), 4);
    }

    #[test]
    fn nothing_scrolls_when_everything_fits() {
        let kit = kit();
        let mut b = small();
        b.push(&kit, "one", tone::SYSTEM);
        assert_eq!(b.wheel(3), Outcome::None);
        assert_eq!(b.key("PageUp"), Outcome::None);
        assert_eq!(
            b.press((b.rect.0 + b.rect.2 - 5, b.rect.1 + 5)),
            Outcome::None
        );
    }

    #[test]
    fn a_click_on_the_bar_pages_toward_the_click() {
        let kit = kit();
        let mut b = small();
        for i in 0..20 {
            b.push(&kit, &format!("l{i}"), tone::SYSTEM);
        }
        // The bar is inside the frame, flush right: x 85 to 119, y 21 to 67.
        assert_eq!(b.bar.bounds(), (85, 21, 34, 46));
        let knob = b.bar.knob(b.view_of_bar()).unwrap();
        assert_eq!(knob.1 + knob.3, 21 + 46 - 5, "the knob rests at the bottom");
        assert_eq!(b.press((86, 22)), Outcome::Changed);
        assert_eq!(b.scrolled_back(), 3, "a page less a line");
        assert!(!b.dragging());
        let knob = b.bar.knob(b.view_of_bar()).unwrap();
        assert!(knob.1 < 32, "the knob moved up");
        assert_eq!(b.press((86, 66)), Outcome::Changed);
        assert_eq!(b.scrolled_back(), 0);
        assert_eq!(b.press((84, 40)), Outcome::None, "off the bar");
        assert_eq!(b.press((90, 10)), Outcome::None, "off the bar");
        assert!(b.on_bar((100, 40)) && !b.on_bar((84, 40)));
    }

    #[test]
    fn the_knob_drags_the_view_and_lets_go() {
        let kit = kit();
        let mut b = small();
        for i in 0..20 {
            b.push(&kit, &format!("l{i}"), tone::SYSTEM);
        }
        assert_eq!(b.drag((100, 0)), Outcome::None, "nothing is held");
        // Grab the knob at the bottom, 8 pixels below its top.
        assert_eq!(b.press((100, 40)), Outcome::None);
        assert!(b.dragging());
        assert_eq!(b.drag((100, 40)), Outcome::None, "not moved yet");
        // Up past the top: the oldest lines.
        assert_eq!(b.drag((300, 0)), Outcome::Changed);
        assert_eq!(b.scrolled_back(), 16);
        assert_eq!(texts(&b), ["l0", "l1", "l2", "l3"]);
        // Part of the way down: the view follows the knob, not the pointer's
        // distance from the knob's first place.
        assert_eq!(b.drag((100, 40 - 5 + 2)), Outcome::Changed);
        assert!(b.scrolled_back() < 16 && b.scrolled_back() > 0);
        b.release();
        assert!(!b.dragging());
        let held = b.scrolled_back();
        assert_eq!(b.drag((100, 70)), Outcome::None);
        assert_eq!(b.scrolled_back(), held);
        // A press on the track holds nothing.
        b.press((86, 22));
        assert!(!b.dragging());
    }

    #[test]
    fn the_bar_does_not_move_the_text_and_nothing_to_scroll_has_no_knob() {
        let kit = kit();
        let mut b = MessageBox::newnet();
        // 549 wide: the bar's 34 pixels, the margin and the gap leave 507.
        assert_eq!(b.text_room(), 507);
        assert_eq!(b.bar.bounds(), (45 + 549 - 35, 320, 34, 86));
        b.push(&kit, "one", tone::SYSTEM);
        assert_eq!(b.bar.knob(b.view_of_bar()), None);
        let mut pixels = blank();
        b.draw(&mut Canvas(&mut pixels), &kit, false);
        // The empty track is there, with no knob.
        assert_eq!(at(&pixels, 45 + 549 - 34, 330), tone_of(&kit, "SLIDEMID"));
        assert_ne!(at(&pixels, 45 + 549 - 30, 330), tone_of(&kit, "SLIDERV"));
        for i in 0..30 {
            b.push(&kit, &format!("l{i}"), tone::SYSTEM);
        }
        let mut pixels = blank();
        b.draw(&mut Canvas(&mut pixels), &kit, false);
        // The knob, 6 in, at the bottom of its travel (86 - 41 = 45).
        let knob = b.bar.knob(b.view_of_bar()).unwrap();
        assert_eq!(knob, (45 + 549 - 35 + 6, 320 + 6 + 45, 26, 30));
        assert_eq!(
            at(&pixels, knob.0 + 1, knob.1 + 1),
            tone_of(&kit, "SLIDERV")
        );
    }

    #[test]
    fn the_knob_follows_the_view_up_and_down() {
        let kit = kit();
        let mut b = small();
        for i in 0..20 {
            b.push(&kit, &format!("l{i}"), tone::SYSTEM);
        }
        let low = b.bar.knob_top(b.view_of_bar()).unwrap();
        b.key("Home");
        let high = b.bar.knob_top(b.view_of_bar()).unwrap();
        assert!(high < low);
        assert_eq!(high, 21 + 6, "the oldest lines: the knob at the top");
        b.key("End");
        assert_eq!(b.bar.knob_top(b.view_of_bar()), Some(low));
    }

    #[test]
    fn it_draws_each_line_in_its_colour() {
        let kit = kit();
        let mut b = MessageBox::newnet();
        b.push(&kit, "hello", tone::ALL);
        b.push(&kit, "again", tone::ENEMY);
        let mut pixels = blank();
        b.draw(&mut Canvas(&mut pixels), &kit, false);
        // The synthetic font is white, so a lit pixel is the colour itself.
        assert_eq!(at(&pixels, 45 + PAD + 1, 319 + PAD + 1), tone::ALL);
        assert_eq!(at(&pixels, 45 + PAD + 1, 319 + PAD + 10 + 1), tone::ENEMY);
        assert_eq!(at(&pixels, 300, 319 + 60), [81; 3], "the flat box");
        assert_eq!(at(&pixels, 45, 319), [174; 3], "its frame");
    }

    use crate::menu::Canvas;
}
