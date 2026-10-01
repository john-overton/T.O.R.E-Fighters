//! The scrolling message box: wrapped, coloured lines, newest at the bottom,
//! with a way back through what scrolled off.
use super::{
    Kit, Outcome, Point, Rect, Widget,
    draw::{focus_mark, text_clipped},
    inside,
};
use crate::menu::{Canvas, text_width};
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
/// Inner margin, and the scroll bar's width and gap.
const PAD: i32 = 4;
const BAR: i32 = 4;
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
/// pointer for the wheel). A thin bar on the right edge shows the place in the
/// kept lines when they do not all fit, and a click above or below its thumb
/// pages back or forward. A view scrolled back stays on the same words as new
/// lines arrive; at the bottom it follows them.
#[derive(Clone, Debug)]
pub struct MessageBox {
    rect: Rect,
    lines: VecDeque<Line>,
    cap: usize,
    /// How many lines the view is scrolled back from the newest.
    back: usize,
    enabled: bool,
}

impl MessageBox {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            lines: VecDeque::new(),
            cap: DEFAULT_CAP,
            back: 0,
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
        self.rect.2 - 2 * PAD - BAR - 2
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
    /// The mouse went down: a click on the scroll bar above its thumb pages
    /// back, below it forward.
    pub fn press(&mut self, point: Point) -> Outcome {
        if !self.enabled || self.max_back() == 0 {
            return Outcome::None;
        }
        let (track, thumb) = self.bar();
        if !inside(track, point) {
            return Outcome::None;
        }
        let page = self.visible().saturating_sub(1).max(1) as i32;
        if point.1 < thumb.1 {
            self.scroll(page)
        } else if point.1 >= thumb.1 + thumb.3 {
            self.scroll(-page)
        } else {
            Outcome::None
        }
    }

    /// The scroll bar's track and thumb.
    fn bar(&self) -> (Rect, Rect) {
        let (x, y, w, h) = self.rect;
        let track = (x + w - PAD - BAR, y + PAD, BAR, h - 2 * PAD);
        let total = self.lines.len().max(1) as i32;
        let shown = self.visible() as i32;
        let height = (track.3 * shown / total).clamp(6, track.3);
        let travel = track.3 - height;
        let room = self.max_back().max(1) as i32;
        // Scrolled to the newest, the thumb is at the bottom.
        let top = track.1 + travel - travel * self.back as i32 / room;
        (track, (track.0, top, BAR, height))
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
            text_clipped(
                canvas,
                font,
                text,
                (x + PAD, y + PAD + i as i32 * LINE),
                clip,
                Some(colour),
            );
        }
        if self.max_back() > 0 {
            let (track, thumb) = self.bar();
            canvas.rect(track, [58, 58, 58, 255]);
            canvas.rect(thumb, [168, 168, 168, 255]);
        }
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
    use super::super::test_kit::{at, blank, kit};
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
        let (track, thumb) = b.bar();
        assert_eq!(
            thumb.1 + thumb.3,
            track.1 + track.3,
            "the thumb rests at the bottom"
        );
        let top = (track.0 + 1, track.1);
        assert_eq!(b.press(top), Outcome::Changed);
        assert_eq!(b.scrolled_back(), 3);
        let (_, thumb) = b.bar();
        assert!(thumb.1 > track.1 && thumb.1 + thumb.3 < track.1 + track.3);
        let below = (track.0 + 1, track.1 + track.3 - 1);
        assert_eq!(b.press(below), Outcome::Changed);
        assert_eq!(b.scrolled_back(), 0);
        assert_eq!(
            b.press((thumb.0 - 20, thumb.1)),
            Outcome::None,
            "off the bar"
        );
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
