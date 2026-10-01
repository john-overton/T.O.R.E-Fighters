//! The text field: an editable line with a caret that moves, a filter and a
//! maximum length, a hint when empty and sideways scrolling.
use super::{
    Kit, Outcome, Point, Rect, Widget,
    draw::{focus_mark, text_clipped},
    inside,
};
use crate::menu::Canvas;
use std::cell::Cell;
use std::net::{Ipv4Addr, Ipv6Addr};
use tore_formats::text::GlyphCodes;

/// A taller box's height, and the text's inset from the box's sides.
pub const LINE_HEIGHT: i32 = 18;
const PAD: i32 = 3;

/// Which characters a field takes, and how many.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    /// A callsign: printable ASCII (letters, digits, punctuation and
    /// spaces), at most 15 characters, as `net/options.rs` checks it. A field
    /// cannot refuse a leading or trailing space while the player is still
    /// typing; the screen asks `callsign_problem` before it proceeds.
    Callsign,
    /// A host name, an IPv4 address or an IPv6 address in brackets, with an
    /// optional `:port`: letters, digits and `. - _ : [ ]`, at most 255
    /// characters (*agent decision*: the longest host name is 253).
    Address,
    /// A port number: digits, at most five.
    Port,
    /// Any printable ASCII (a password), at most 255 characters.
    Text,
}

impl Filter {
    pub fn allows(self, c: char) -> bool {
        match self {
            Filter::Callsign | Filter::Text => c.is_ascii_graphic() || c == ' ',
            Filter::Address => c.is_ascii_alphanumeric() || ".-_:[]".contains(c),
            Filter::Port => c.is_ascii_digit(),
        }
    }
    pub fn max_len(self) -> usize {
        match self {
            Filter::Callsign => 15,
            Filter::Address | Filter::Text => 255,
            Filter::Port => 5,
        }
    }
}

/// One editable line: NEWNET's flat grey recessed box, typed in `PANELFNT`
/// (John, 2026-10-01: every field a player types in is this plain box; the
/// retail edit control's red gradient and wide-spaced `WHEELFNT` are not
/// used). [`TextField::bar`] is the 13 pixel high bar NEWNET shows for the
/// callsign; [`TextField::line`] is the same box taller, for the fields that
/// are the point of their screen (Connect to, the chat line, Options).
///
/// Keys it takes: Left, Right, Home, End, Backspace, Delete; Enter answers
/// `Activated`. Typed text comes from the window's text events
/// ([`TextField::text_input`]). A click puts the caret between the characters
/// nearest the pointer. The screen owns Tab, Esc and the focus. No paste (it
/// would need the clipboard).
#[derive(Clone, Debug)]
pub struct TextField {
    rect: Rect,
    filter: Filter,
    max: usize,
    hint: String,
    text: String,
    caret: usize,
    /// The first character shown; settled by the next draw or click.
    first: Cell<usize>,
    enabled: bool,
    /// Draws a `*` for every character (a password).
    masked: bool,
}

impl TextField {
    /// NEWNET's flat bar of `width` by 13 at `at` (the callsign is 139 wide).
    pub fn bar(at: Point, width: i32, filter: Filter) -> Self {
        Self::new((at.0, at.1, width, 13), filter)
    }
    /// The same box `width` by `LINE_HEIGHT` (18) at `at`.
    pub fn line(at: Point, width: i32, filter: Filter) -> Self {
        Self::new((at.0, at.1, width, LINE_HEIGHT), filter)
    }
    fn new(rect: Rect, filter: Filter) -> Self {
        Self {
            rect,
            filter,
            max: filter.max_len(),
            hint: String::new(),
            text: String::new(),
            caret: 0,
            first: Cell::new(0),
            enabled: true,
            masked: false,
        }
    }
    /// Shows a `*` for every character typed, as a password field does. The
    /// text is still what [`TextField::text`] returns.
    pub fn masked(mut self) -> Self {
        self.masked = true;
        self
    }
    /// A tighter limit than the filter's.
    pub fn with_max(mut self, max: usize) -> Self {
        self.max = max.min(self.filter.max_len());
        self.set_text(&self.text.clone());
        self
    }
    /// Words shown in the field while it is empty.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }
    #[cfg(test)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    /// The caret's place, in characters from the start.
    #[cfg(test)]
    pub fn caret(&self) -> usize {
        self.caret
    }
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
    /// Replaces the text (kept to the filter and the limit) and puts the
    /// caret at its end.
    pub fn set_text(&mut self, text: &str) {
        self.text = text
            .chars()
            .filter(|c| self.filter.allows(*c))
            .take(self.max)
            .collect();
        self.caret = self.text.len();
        self.first.set(0);
    }

    /// Typed text, from the window's text event for a key press. Characters
    /// the filter refuses (control characters, Tab, Enter's carriage return)
    /// are dropped; the rest go in at the caret until the field is full.
    pub fn text_input(&mut self, typed: &str) -> Outcome {
        let mut changed = false;
        for c in typed.chars() {
            if self.text.len() >= self.max {
                break;
            }
            if self.filter.allows(c) {
                self.text.insert(self.caret, c);
                self.caret += 1;
                changed = true;
            }
        }
        if changed {
            Outcome::Changed
        } else {
            Outcome::None
        }
    }

    /// A key by the window's name for it ("ArrowLeft", "Backspace"...).
    pub fn key(&mut self, name: &str) -> Outcome {
        if !self.enabled {
            return Outcome::None;
        }
        match name {
            "ArrowLeft" => self.caret = self.caret.saturating_sub(1),
            "ArrowRight" => self.caret = (self.caret + 1).min(self.text.len()),
            "Home" => self.caret = 0,
            "End" => self.caret = self.text.len(),
            "Backspace" if self.caret > 0 => {
                self.caret -= 1;
                self.text.remove(self.caret);
                return Outcome::Changed;
            }
            "Delete" if self.caret < self.text.len() => {
                self.text.remove(self.caret);
                return Outcome::Changed;
            }
            "Enter" => return Outcome::Activated,
            _ => {}
        }
        Outcome::None
    }

    fn font<'k>(&self, kit: &'k Kit) -> &'k crate::menu::Sprite {
        kit.sprite("PANELFNT")
    }
    /// The text area: left, width.
    fn inner(&self) -> (i32, i32) {
        (self.rect.0 + PAD, self.rect.2 - 2 * PAD)
    }
    fn widths(&self, kit: &Kit) -> Vec<i32> {
        let font = self.font(kit);
        self.text
            .glyph_codes()
            .map(|c| font.glyphs[c as usize][1] as i32)
            .collect()
    }
    /// Settles which character is first so that the caret shows, and returns
    /// it with the character widths.
    fn view(&self, kit: &Kit) -> (usize, Vec<i32>) {
        let widths = self.widths(kit);
        let (_, room) = self.inner();
        let span = |from: usize, to: usize| -> i32 { widths[from..to].iter().sum() };
        let mut first = self.first.get().min(self.text.len());
        if self.caret < first {
            first = self.caret;
        }
        while first < self.caret && span(first, self.caret) > room {
            first += 1;
        }
        // No empty space at the right while characters hide at the left.
        while first > 0 && span(first - 1, widths.len()) <= room {
            first -= 1;
        }
        self.first.set(first);
        (first, widths)
    }

    /// The mouse went down at `point`: a hit puts the caret between the two
    /// characters nearest it. The screen gives the field the focus.
    pub fn press(&mut self, point: Point, kit: &Kit) -> Outcome {
        if !self.hit(point) {
            return Outcome::None;
        }
        let (first, widths) = self.view(kit);
        let (left, _) = self.inner();
        let mut edge = left;
        let mut best = (i32::MAX, first);
        for (i, w) in widths.iter().enumerate().skip(first) {
            let distance = (point.0 - edge).abs();
            if distance < best.0 {
                best = (distance, i);
            }
            edge += w;
        }
        if (point.0 - edge).abs() < best.0 {
            best = (0, widths.len());
        }
        self.caret = best.1;
        Outcome::None
    }

    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit, focused: bool) {
        let (x, y, w, h) = self.rect;
        let font = self.font(kit);
        let (first, widths) = self.view(kit);
        let (left, room) = self.inner();
        let clip = (left, y, room, h);
        canvas.rect((x, y, w, h), [97, 97, 97, 255]);
        // The text sits in the middle of the box (NEWNET's 13 pixel bar puts
        // it 2 pixels down).
        let top = y + 2 + (h - 13) / 2;
        let (caret_top, caret_height) = (top - 1, 12);
        if self.text.is_empty() {
            if !self.hint.is_empty() {
                text_clipped(canvas, font, &self.hint, (left, top), clip, Some([150; 3]));
            }
        } else {
            let shown: String = if self.masked {
                "*".repeat(self.text.chars().count().saturating_sub(first))
            } else {
                self.text.chars().skip(first).collect()
            };
            let tint = (!self.enabled).then_some([120; 3]);
            text_clipped(canvas, font, &shown, (left, top), clip, tint);
        }
        if focused && self.enabled {
            let before: i32 = widths[first.min(self.caret)..self.caret].iter().sum();
            canvas.rect((left + before, caret_top, 1, caret_height), [255; 4]);
            focus_mark(canvas, (x - 2, y - 2, w + 4, h + 4));
        }
    }
}

impl Widget for TextField {
    fn bounds(&self) -> Rect {
        self.rect
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn hit(&self, point: Point) -> bool {
        self.enabled && inside(self.rect, point)
    }
}

/// Why `text` is not an address the Direct Connection screen can join, in
/// plain words, or `None` when it is one. See [`parse_address`].
#[cfg(test)]
pub fn address_problem(text: &str) -> Option<&'static str> {
    parse_address(text).err()
}

/// Splits what the player typed into a host and an optional port. Accepts a
/// host name (`games.example.org`), an IPv4 address (`192.168.1.20`), either
/// with `:port`, and an IPv6 address in brackets (`[::1]`, `[::1]:26900`); a
/// bare IPv6 address (more than one colon, no brackets) has no port. The host
/// is not looked up here: names are resolved off the screen's thread (EF5).
/// The port, when given, is 1 to 65535.
pub fn parse_address(text: &str) -> Result<(String, Option<u16>), &'static str> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Type the game's address.");
    }
    let port = |digits: &str| -> Result<Option<u16>, &'static str> {
        digits
            .parse::<u16>()
            .ok()
            .filter(|p| *p != 0)
            .map(Some)
            .ok_or("The port must be a number from 1 to 65535.")
    };
    if let Some(rest) = text.strip_prefix('[') {
        let (host, tail) = rest
            .split_once(']')
            .ok_or("An IPv6 address needs its closing bracket.")?;
        host.parse::<Ipv6Addr>()
            .map_err(|_| "That is not a valid IPv6 address.")?;
        let port = match tail.strip_prefix(':') {
            Some(digits) => port(digits)?,
            None if tail.is_empty() => None,
            None => return Err("Put a port after the bracket as :port."),
        };
        return Ok((host.to_owned(), port));
    }
    if text.matches(':').count() > 1 {
        text.parse::<Ipv6Addr>()
            .map_err(|_| "Put an IPv6 address in brackets, like [::1].")?;
        return Ok((text.to_owned(), None));
    }
    let (host, port) = match text.split_once(':') {
        Some((host, digits)) => (host, port(digits)?),
        None => (text, None),
    };
    if host.is_empty() {
        return Err("Type the game's address before the port.");
    }
    if host.chars().all(|c| c.is_ascii_digit() || c == '.') {
        host.parse::<Ipv4Addr>()
            .map_err(|_| "That is not a valid IPv4 address.")?;
        return Ok((host.to_owned(), port));
    }
    if host.len() > 253 {
        return Err("A host name is at most 253 characters.");
    }
    for label in host.split('.') {
        let valid = (1..=63).contains(&label.len())
            && label
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            && !label.starts_with('-')
            && !label.ends_with('-');
        if !valid {
            return Err("That is not a valid host name.");
        }
    }
    Ok((host.to_owned(), port))
}

#[cfg(test)]
mod tests {
    use super::super::test_kit::{at, blank, kit};
    use super::*;

    fn edit(filter: Filter) -> TextField {
        TextField::line((100, 100), 31, filter)
    }
    fn type_in(field: &mut TextField, text: &str) {
        field.text_input(text);
    }

    #[test]
    fn typing_inserts_at_the_caret_and_the_caret_moves() {
        let mut f = edit(Filter::Text);
        type_in(&mut f, "abcd");
        assert_eq!((f.text(), f.caret()), ("abcd", 4));
        f.key("ArrowLeft");
        f.key("ArrowLeft");
        type_in(&mut f, "XY");
        assert_eq!((f.text(), f.caret()), ("abXYcd", 4));
        f.key("Home");
        assert_eq!(f.caret(), 0);
        f.key("ArrowLeft");
        assert_eq!(f.caret(), 0, "Left stops at the start");
        f.key("End");
        assert_eq!(f.caret(), 6);
        f.key("ArrowRight");
        assert_eq!(f.caret(), 6, "Right stops at the end");
    }

    #[test]
    fn backspace_and_delete_remove_around_the_caret() {
        let mut f = edit(Filter::Text);
        type_in(&mut f, "abcd");
        f.key("ArrowLeft");
        assert_eq!(f.key("Backspace"), Outcome::Changed);
        assert_eq!((f.text(), f.caret()), ("abd", 2));
        assert_eq!(f.key("Delete"), Outcome::Changed);
        assert_eq!((f.text(), f.caret()), ("ab", 2));
        assert_eq!(f.key("Delete"), Outcome::None, "nothing after the caret");
        f.key("Home");
        assert_eq!(f.key("Backspace"), Outcome::None, "nothing before it");
        assert_eq!(f.text(), "ab");
    }

    #[test]
    fn enter_activates_and_other_keys_are_the_screens() {
        let mut f = edit(Filter::Text);
        assert_eq!(f.key("Enter"), Outcome::Activated);
        assert_eq!(f.key("Escape"), Outcome::None);
        assert_eq!(f.key("Tab"), Outcome::None);
        assert!(f.is_empty());
    }

    #[test]
    fn callsign_filter_takes_printable_ascii_up_to_15() {
        let mut f = TextField::bar((0, 0), 139, Filter::Callsign);
        assert_eq!(f.text_input("Maverick\r\n\t\u{7f}é"), Outcome::Changed);
        assert_eq!(
            f.text(),
            "Maverick",
            "control and non-ASCII characters are dropped"
        );
        assert_eq!(f.text_input("0123456789ABCDEFG"), Outcome::Changed);
        assert_eq!(f.text().len(), 15, "at most 15 characters");
        assert_eq!(
            f.text_input("x"),
            Outcome::None,
            "a full field takes nothing"
        );
        // Whatever the field lets through, the game's own check agrees except
        // for the leading and trailing spaces the screen still has to refuse.
        for typed in ["Pilot", "Bat man", "A-1_2~", "x"] {
            let mut f = TextField::bar((0, 0), 139, Filter::Callsign);
            f.text_input(typed);
            assert_eq!(f.text(), typed);
            assert_eq!(crate::net::options::callsign_problem(f.text()), None);
        }
        let mut f = TextField::bar((0, 0), 139, Filter::Callsign);
        f.text_input(" a");
        assert!(crate::net::options::callsign_problem(f.text()).is_some());
    }

    #[test]
    fn address_and_port_filters() {
        let mut f = edit(Filter::Address);
        f.text_input("[::1]:26900 games.example-1.org_x /?#");
        assert_eq!(f.text(), "[::1]:26900games.example-1.org_x");
        let mut p = edit(Filter::Port);
        p.text_input("26a9x00123");
        assert_eq!(p.text(), "26900", "digits only, five at most");
        let mut small = edit(Filter::Text).with_max(3);
        small.text_input("abcdef");
        assert_eq!(small.text(), "abc");
        small.set_text("zzzzzz");
        assert_eq!(small.text(), "zzz");
    }

    #[test]
    fn addresses_parse_into_host_and_port() {
        let ok = |t: &str| parse_address(t).unwrap();
        assert_eq!(ok("games.example.org"), ("games.example.org".into(), None));
        assert_eq!(
            ok("games.example.org:26901"),
            ("games.example.org".into(), Some(26901))
        );
        assert_eq!(ok("192.168.1.20"), ("192.168.1.20".into(), None));
        assert_eq!(ok("192.168.1.20:1"), ("192.168.1.20".into(), Some(1)));
        assert_eq!(ok("localhost"), ("localhost".into(), None));
        assert_eq!(ok("[::1]"), ("::1".into(), None));
        assert_eq!(ok("[fe80::2]:26900"), ("fe80::2".into(), Some(26900)));
        assert_eq!(
            ok("::1"),
            ("::1".into(), None),
            "a bare IPv6 address has no port"
        );
        for bad in [
            "",
            "  ",
            ":26900",
            "host:",
            "host:0",
            "host:65536",
            "host:abc",
            "[::1",
            "[::1]x",
            "[nope]",
            "1.2.3",
            "999.1.1.1",
            "-bad.example.org",
            "bad-.org",
            "a..b",
            "a:b:c",
        ] {
            assert!(parse_address(bad).is_err(), "{bad:?} should be refused");
        }
        assert_eq!(
            address_problem("host:99999"),
            Some("The port must be a number from 1 to 65535.")
        );
        assert_eq!(address_problem("host"), None);
    }

    #[test]
    fn a_wide_text_scrolls_to_keep_the_caret_in_view() {
        let kit = kit();
        let mut f = edit(Filter::Text); // five 10 pixel characters show
        type_in(&mut f, "abcdefghijkl");
        let (first, _) = f.view(&kit);
        assert_eq!(first, 7, "the caret at the end sits at the right edge");
        f.key("Home");
        assert_eq!(f.view(&kit).0, 0, "Home scrolls back to the start");
        // Moving right inside the view does not scroll; past it does.
        for _ in 0..5 {
            f.key("ArrowRight");
        }
        assert_eq!(f.view(&kit).0, 0);
        f.key("ArrowRight");
        assert_eq!(f.view(&kit).0, 1);
        // Moving left within the view keeps it where it is.
        f.key("ArrowLeft");
        f.key("ArrowLeft");
        assert_eq!(f.view(&kit).0, 1);
        // Deleting leaves no blank space on the right.
        f.key("End");
        for _ in 0..8 {
            f.key("Backspace");
        }
        assert_eq!(f.text(), "abcd");
        assert_eq!(f.view(&kit).0, 0);
    }

    #[test]
    fn a_click_puts_the_caret_between_the_nearest_characters() {
        let kit = kit();
        let mut f = edit(Filter::Text);
        type_in(&mut f, "abcde");
        // The text starts at x 103 and each character is 5 wide.
        f.press((103 + 11, 105), &kit);
        assert_eq!(f.caret(), 2);
        f.press((103 + 13, 105), &kit);
        assert_eq!(f.caret(), 3);
        f.press((130, 105), &kit);
        assert_eq!(f.caret(), 5, "past the text is the end");
        f.press((100, 105), &kit);
        assert_eq!(f.caret(), 0);
        f.press((300, 300), &kit);
        assert_eq!(f.caret(), 0, "a click elsewhere changes nothing");
    }

    #[test]
    fn it_draws_a_hint_when_empty_and_a_caret_when_focused() {
        let kit = kit();
        let mut f = edit(Filter::Text).with_hint("host");
        let mut pixels = blank();
        f.draw(&mut Canvas(&mut pixels), &kit, false);
        // The hint is the font's white in the hint grey, on the field.
        assert_eq!(at(&pixels, 105, 106), [150; 3]);
        let mut pixels = blank();
        type_in(&mut f, "ab");
        f.draw(&mut Canvas(&mut pixels), &kit, true);
        assert_eq!(at(&pixels, 105, 106), [255; 3], "typed text is not dimmed");
        // The caret is a white line at x + 3 + 5 * 2, from 3 below the top.
        assert_eq!(at(&pixels, 113, 103), [255; 3]);
        assert_eq!(at(&pixels, 113, 114), [255; 3]);
        // A plain grey box, not the retail edit control (the box is 18 high).
        assert_eq!(at(&pixels, 101, 112), [97; 3]);
        assert_eq!(at(&pixels, 101, 117), [97; 3]);
    }

    #[test]
    fn a_disabled_field_ignores_keys_and_clicks() {
        let kit = kit();
        let mut f = edit(Filter::Text);
        type_in(&mut f, "ab");
        f.set_enabled(false);
        assert_eq!(f.key("Backspace"), Outcome::None);
        assert!(!f.hit((110, 110)));
        f.press((110, 110), &kit);
        assert_eq!(f.text(), "ab");
    }

    use crate::menu::Canvas;
}
