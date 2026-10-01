//! Chat in a networked flight (slice EF6): the `~` line, its keys, the
//! window of recent lines at the top left of the flight screen, and the
//! quick messages of `CHAT.TXT`. What the host does with a line is in
//! `tore_session::host` ("Chat"); the design and John's decisions are in
//! docs/ARCHITECTURE.md ("Chat") and docs/CONTROLS.md.
//!
//! - **The line.** `~` (the backtick key; the key is the one above Tab,
//!   whatever the layout prints on it) opens it while a networked flight
//!   shows. While it is open every held flight key is let go and the
//!   keyboard types into it; the joystick and the mouse still fly. Tab picks
//!   the receiver (All, Friendlies, Enemies, Wing, Target, round), Enter
//!   sends and closes it, Esc closes it without sending, Backspace deletes,
//!   and F1 to F12 send the matching line of `CHAT.TXT` to its own receiver
//!   (or the picked one). Closed, Enter designates and the F keys are the
//!   views, as in single player. Single player never opens it.
//! - **The window.** The last [`WINDOW_LINES`] lines at the top left, apart
//!   from the HUD's messages at the bottom, each in the colour of who sent
//!   it and to whom ([`colour`]): green for the player's own side, blue for
//!   a line to everyone from the player's side, red for the enemy's, pale
//!   grey for the host's own words. A line stays [`LINE_SECONDS`] and fades
//!   out over the last [`FADE_SECONDS`]; while the line is open the window
//!   keeps showing the last [`OPEN_LINES`] whatever their age.
//!
//! The text is the HUD's own font, drawn at the HUD's scale and filtered
//! like the HUD's messages, over a thin translucent dark band so any colour
//! reads on any sky or ground. Retail's `MPSTATUS` pane and `MPFONT` (the
//! menus' connected-state window, EF0) are not used: the pane is an opaque
//! 333 by 80 block that would hide a sixth of the top of the view (*agent
//! decision*).
use crate::flight_canvas::{FlightCanvas, HUD_SCALE};
use crate::widgets::tone;
use std::collections::VecDeque;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tore_formats::chat::QuickMessage;
use tore_formats::font::Font;
use tore_formats::text::GlyphCodes;
use tore_session::wire::chat::{
    ChatLine, ChatSend, MAX_TEXT, Receiver, Tone, next_receiver, printable, receiver_label,
};

/// Lines the window shows at once (after wrapping).
pub const WINDOW_LINES: usize = 6;
/// Lines the window shows while the line is open.
pub const OPEN_LINES: usize = 8;
/// How long a line stays (*agent decision*).
pub const LINE_SECONDS: f64 = 15.;
/// The last seconds of that, over which a line fades out.
pub const FADE_SECONDS: f64 = 3.;
/// Lines kept for the open line to show.
const HISTORY: usize = 64;
/// The window's width, in 640 by 480 layer units (the HUD's own).
const WIDTH: f64 = 320.;
/// The gap to the left and top edges, in layer units.
const MARGIN: f64 = 5.;
/// The translucent band behind the text.
const BACKING: f64 = 0.6;
/// The key hint under the open line.
const HINT: &str = "TAB RECEIVER  ENTER SEND  ESC CANCEL  F1-F12 QUICK";
/// What the open line's caret does each half second.
const BLINK: Duration = Duration::from_millis(500);

static QUICK: OnceLock<Vec<QuickMessage>> = OnceLock::new();

/// Reads `CHAT.TXT` from the pack's multiplayer resources, once, when the
/// game starts: the quick messages F1 to F12 send. A pack without it has
/// none (the F keys then say so).
pub fn load_quick_messages(multiplayer: &std::collections::BTreeMap<String, Vec<u8>>) {
    let lines = multiplayer
        .get(tore_import::selection::CHAT_RESOURCE)
        .map(|bytes| tore_formats::chat::parse(bytes))
        .unwrap_or_default();
    let _ = QUICK.set(lines);
}

/// The quick messages the game loaded.
pub fn quick_messages() -> &'static [QuickMessage] {
    QUICK.get().map_or(&[], Vec::as_slice)
}

/// The colour a line is drawn in: the message box's own set, so the lobby
/// and the flight agree.
pub fn colour(tone_of_line: Tone) -> [u8; 3] {
    match tone_of_line {
        Tone::Own => tone::OWN_SIDE,
        Tone::Everyone => tone::ALL,
        Tone::Enemy => tone::ENEMY,
        Tone::System => tone::SYSTEM,
    }
}

/// One line of the window.
#[derive(Clone, Debug)]
struct Entry {
    text: String,
    tone: Tone,
    at: Instant,
}

/// What a key did to the open line, for the game to carry out.
#[derive(Clone, Debug, PartialEq)]
pub enum Keyed {
    /// Nothing more to do (the text or receiver changed, or the key is not
    /// one the line takes).
    Nothing,
    /// The line closed without sending.
    Closed,
    /// Send this, and the line is closed.
    Send(ChatSend),
    /// The line stays open and the player is told why nothing was sent.
    Told(String),
}

/// The chat's state in a networked session: the window's lines and the open
/// line.
#[derive(Debug)]
pub struct Chat {
    entries: VecDeque<Entry>,
    /// The text typed while the line is open; `None` while it is closed.
    line: Option<String>,
    /// The receiver Tab last picked, kept between openings.
    receiver: Receiver,
    /// When the line opened, for the caret's blink.
    opened: Instant,
}

impl Default for Chat {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            line: None,
            receiver: Receiver::All,
            opened: Instant::now(),
        }
    }
}

impl Chat {
    pub fn is_open(&self) -> bool {
        self.line.is_some()
    }

    /// The receiver the line sends to.
    #[cfg(test)]
    pub fn receiver(&self) -> Receiver {
        self.receiver
    }

    /// The typed text (empty when closed).
    #[cfg(test)]
    pub fn text(&self) -> &str {
        self.line.as_deref().unwrap_or("")
    }

    /// Opens an empty line.
    pub fn open(&mut self, now: Instant) {
        self.line = Some(String::new());
        self.opened = now;
    }

    /// A line arrived (or the player's own went out): shown from `now`. A
    /// line to anyone but All needs the player in a flight, so the receiver
    /// the next opening starts on returns to All when the flight ends
    /// ([`Chat::flight_ended`]).
    pub fn push(&mut self, line: &ChatLine, now: Instant) {
        self.say(line.display(), line.tone(), now);
    }

    /// A line from the game itself, in the host's colour.
    pub fn system(&mut self, text: &str, now: Instant) {
        self.say(text.to_owned(), Tone::System, now);
    }

    fn say(&mut self, text: String, tone: Tone, now: Instant) {
        self.entries.push_back(Entry {
            text,
            tone,
            at: now,
        });
        while self.entries.len() > HISTORY {
            self.entries.pop_front();
        }
    }

    /// The flight is over: the line closes and the receiver is All again.
    pub fn flight_ended(&mut self) {
        self.line = None;
        self.receiver = Receiver::All;
    }

    /// A key press while the line is open. `name` is the flight's name for
    /// the key (`Enter`, `Tab`, `F3`, `a`...), `text` the text the key
    /// typed, `shift` whether Shift is down (Shift-Tab goes back a
    /// receiver), `typing` whether Ctrl, Alt and Super are all up (a chord
    /// types nothing).
    pub fn key(&mut self, name: &str, text: Option<&str>, shift: bool, typing: bool) -> Keyed {
        let Some(line) = &mut self.line else {
            return Keyed::Nothing;
        };
        match name {
            "Escape" => {
                self.line = None;
                Keyed::Closed
            }
            "Tab" => {
                // Shift-Tab goes back: four steps of five.
                let steps = if shift { 4 } else { 1 };
                for _ in 0..steps {
                    self.receiver = next_receiver(self.receiver, true);
                }
                Keyed::Nothing
            }
            "Enter" => {
                let typed = std::mem::take(line);
                self.line = None;
                if typed.trim().is_empty() {
                    Keyed::Closed
                } else {
                    Keyed::Send(ChatSend::typed(self.receiver, &typed))
                }
            }
            "Backspace" => {
                line.pop();
                Keyed::Nothing
            }
            f if quick_number(f).is_some() => {
                let number = quick_number(f).expect("a number");
                match quick_messages().get(usize::from(number) - 1) {
                    Some(quick) if !quick.text.is_empty() => {
                        let send = ChatSend::quick(number, quick, self.receiver);
                        self.line = None;
                        Keyed::Send(send)
                    }
                    _ => Keyed::Told(format!("There is no quick message on F{number}.")),
                }
            }
            _ => {
                if typing && let Some(text) = text {
                    for c in text.chars().filter(|c| printable(*c)) {
                        if line.chars().count() < MAX_TEXT {
                            line.push(c);
                        }
                    }
                }
                Keyed::Nothing
            }
        }
    }

    /// Draws the window and, when open, the line, on the flight view's
    /// overlay.
    pub fn draw(&self, canvas: &mut FlightCanvas, font: &Font, now: Instant) {
        let open = self.is_open();
        let [w, h] = canvas.size.map(f64::from);
        let layer = (w / 640.).min(h / 480.);
        let scale = layer * HUD_SCALE;
        let room = (WIDTH * layer / scale) as usize;
        let mut lines: Vec<(String, [u8; 3], f64)> = Vec::new();
        let keep = if open { OPEN_LINES } else { WINDOW_LINES };
        for entry in &self.entries {
            let age = now.saturating_duration_since(entry.at).as_secs_f64();
            let alpha = if open {
                1.
            } else if age >= LINE_SECONDS {
                continue;
            } else {
                ((LINE_SECONDS - age) / FADE_SECONDS).min(1.)
            };
            for piece in wrap(font, &entry.text, room) {
                lines.push((piece, colour(entry.tone), alpha));
            }
        }
        let skip = lines.len().saturating_sub(keep);
        lines.drain(..skip);
        if let Some(text) = &self.line {
            let caret = if (now.saturating_duration_since(self.opened).as_millis()
                / BLINK.as_millis())
            .is_multiple_of(2)
            {
                "_"
            } else {
                " "
            };
            let tone = if self.receiver == Receiver::All {
                Tone::Everyone
            } else {
                Tone::Own
            };
            // The line keeps its tail in view when it runs long.
            let prompt = format!("SEND TO {}: {text}{caret}", receiver_label(self.receiver));
            let mut shown = wrap(font, &prompt, room);
            let keep_from = shown.len().saturating_sub(2);
            lines.extend(
                shown
                    .drain(keep_from..)
                    .map(|piece| (piece, colour(tone), 1.)),
            );
            lines.push((HINT.to_owned(), [170, 170, 170], 1.));
        }
        if lines.is_empty() {
            return;
        }
        draw_lines(canvas, font, &lines, layer, scale);
    }
}

/// The quick message a key name asks for: `F1` to `F12` are 1 to 12.
fn quick_number(name: &str) -> Option<u8> {
    name.strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|n| (1..=12).contains(n))
}

/// Whether `event` is the chat key: the key above Tab (physical Backquote),
/// or a `` ` `` or `~` the layout puts elsewhere, with no Ctrl, Alt or
/// Super held.
pub fn is_chat_key(
    physical: winit::keyboard::PhysicalKey,
    name: &str,
    modifiers: winit::keyboard::ModifiersState,
) -> bool {
    use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
    let chord = ModifiersState::CONTROL | ModifiersState::ALT | ModifiersState::SUPER;
    !modifiers.intersects(chord)
        && (physical == PhysicalKey::Code(KeyCode::Backquote) || name == "`" || name == "~")
}

fn text_width(font: &Font, text: &str) -> usize {
    text.glyph_codes()
        .map(|ch| font.glyphs[ch as usize].advance)
        .sum()
}

/// Breaks `text` into lines no wider than `width` font pixels, at spaces (a
/// word wider than the window breaks where it must).
fn wrap(font: &Font, text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split(' ') {
        let current = lines.last().expect("a line");
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if text_width(font, &candidate) <= width {
            *lines.last_mut().expect("a line") = candidate;
            continue;
        }
        if !current.is_empty() {
            lines.push(String::new());
        }
        for c in word.chars() {
            let current = lines.last_mut().expect("a line");
            let mut grown = current.clone();
            grown.push(c);
            if text_width(font, &grown) > width && !current.is_empty() {
                lines.push(c.to_string());
            } else {
                *current = grown;
            }
        }
    }
    lines
}

/// Draws `lines` (text, colour, opacity) from the top left of the view over
/// a translucent band, in the HUD's font at its scale, each glyph pixel a
/// box that gives every screen pixel the share it covers.
fn draw_lines(
    canvas: &mut FlightCanvas,
    font: &Font,
    lines: &[(String, [u8; 3], f64)],
    layer: f64,
    scale: f64,
) {
    let line_height = (font.height + 1) as f64 * scale;
    let pad = 2. * layer;
    let (left, top) = (MARGIN * layer, MARGIN * layer);
    let widest = lines
        .iter()
        .map(|(text, _, _)| text_width(font, text))
        .max()
        .unwrap_or(0) as f64
        * scale;
    let band = (
        (left - pad).max(0.),
        (top - pad).max(0.),
        left + widest + pad,
        top + lines.len() as f64 * line_height + pad,
    );
    // The band follows the brightest line, so a fading window fades whole.
    let strongest = lines.iter().map(|l| l.2).fold(0., f64::max);
    darken(canvas, band, BACKING * strongest);
    let rows = line_height.ceil() as usize + 2;
    let mut cover = vec![0f64; widest.ceil() as usize * rows + 2 * rows];
    for (i, (text, colour, alpha)) in lines.iter().enumerate() {
        // The line's own box, whole pixels: its top row is `y0`, as wide as
        // the line.
        let y = top + i as f64 * line_height;
        let (x0, y0) = (left.floor(), y.floor());
        let columns = (text_width(font, text) as f64 * scale).ceil() as usize + 2;
        cover[..columns * rows].fill(0.);
        let mut x = left - x0;
        let dy0 = y - y0;
        for ch in text.glyph_codes() {
            let glyph = &font.glyphs[ch as usize];
            for &(gx, gy) in &glyph.pixels {
                let (l, u) = (x + gx as f64 * scale, dy0 + gy as f64 * scale);
                let (r, d) = (l + scale, u + scale);
                for row in u.floor() as usize..(d.ceil() as usize).min(rows) {
                    let dy = d.min(row as f64 + 1.) - u.max(row as f64);
                    for column in l.floor() as usize..(r.ceil() as usize).min(columns) {
                        let dx = r.min(column as f64 + 1.) - l.max(column as f64);
                        cover[row * columns + column] += dx * dy;
                    }
                }
            }
            x += glyph.advance as f64 * scale;
        }
        for (at, amount) in cover[..columns * rows].iter().enumerate() {
            if *amount > 0. {
                let (column, row) = (at % columns, at / columns);
                canvas.blend(
                    x0 as i32 + column as i32,
                    y0 as i32 + row as i32,
                    *colour,
                    amount.min(1.) * alpha,
                );
            }
        }
    }
}

/// Darkens the rectangle `(left, top, right, bottom)` of the overlay by
/// `alpha` of black: an integer "over" per pixel, which an empty overlay
/// pixel takes as plain translucent black.
fn darken(canvas: &mut FlightCanvas, (left, top, right, bottom): (f64, f64, f64, f64), alpha: f64) {
    let [w, h] = canvas.size.map(|n| n as i32);
    let (x0, x1) = (
        (left.floor() as i32).clamp(0, w),
        (right.ceil() as i32).clamp(0, w),
    );
    let (y0, y1) = (
        (top.floor() as i32).clamp(0, h),
        (bottom.ceil() as i32).clamp(0, h),
    );
    let a = (alpha.clamp(0., 1.) * 255.).round() as u32;
    if a == 0 {
        return;
    }
    for y in y0..y1 {
        let row = (y * w) as usize * 4;
        for pixel in canvas.pixels[row + x0 as usize * 4..row + x1 as usize * 4].chunks_exact_mut(4)
        {
            if pixel[3] == 0 {
                pixel.copy_from_slice(&[0, 0, 0, a as u8]);
            } else {
                // Straight-alpha over black: the colour dims by what the
                // band does not let through, the alpha grows.
                let under = u32::from(pixel[3]) * (255 - a) / 255;
                let out = a + under;
                for c in &mut pixel[..3] {
                    *c = (u32::from(*c) * under / out.max(1)) as u8;
                }
                pixel[3] = out.min(255) as u8;
            }
        }
    }
}

impl crate::App {
    /// The flight's key routing for chat, called before the flight takes a
    /// key. Returns true when chat took the key and the flight must not see
    /// it.
    ///
    /// Closed, `~` opens the line while a networked flight shows (no menu or
    /// map up, the window focused): every held flight key is let go, as a
    /// menu does. Open, every key press is the line's (releases go on to
    /// the flight, which only lets go of what was held).
    pub(crate) fn chat_key(&mut self, event: &crate::input_script::KeyInput, name: &str) -> bool {
        if self.screen != crate::Screen::Flight || self.net_flight.is_none() {
            return false;
        }
        let Some(session) = self.net.as_mut() else {
            return false;
        };
        let now = Instant::now();
        if !session.chat.is_open() {
            if event.pressed
                && !event.repeat
                && self.focused
                && !self.flight_ui.menu
                && !self.flight_ui.map.open
                && is_chat_key(event.physical, name, self.modifiers)
            {
                session.chat.open(now);
                self.release_flight_keys();
                return true;
            }
            return false;
        }
        if !event.pressed {
            return false;
        }
        let m = self.modifiers;
        let typing = !(m.control_key() || m.alt_key() || m.super_key());
        let keyed = session
            .chat
            .key(name, event.text.as_deref(), m.shift_key(), typing);
        match keyed {
            Keyed::Nothing | Keyed::Closed => {}
            Keyed::Told(text) => session.chat.system(&text, now),
            Keyed::Send(send) => {
                if let Err(refusal) = session.client.chat_send(send) {
                    // A line the game itself refuses (it should not come to
                    // this: the line takes only what the rules allow).
                    session.chat.system(refusal.text(), now);
                }
            }
        }
        true
    }

    /// Lets go of every held flight key and the trigger, so a key held when
    /// the chat line opens does not go on flying the plane.
    fn release_flight_keys(&mut self) {
        self.camera.keys.clear();
        self.release_trigger();
        self.instruments.cancel_press();
        self.input.release_keys();
    }

    /// A chat line from the session: kept for the window (and the log), and
    /// its quick-message sound played for those who hear it.
    pub(crate) fn chat_line(&mut self, line: ChatLine) {
        log::info!("Network: chat: {}", line.log_text());
        if let Some(sound) = line.sound.as_deref()
            && let Some(audio) = &self.audio
        {
            audio.effect(sound);
        }
        if let Some(session) = &mut self.net {
            session.chat.push(&line, Instant::now());
        }
    }
}

/// The window and the open line on the flight view, for a networked flight
/// (`session` is `None` otherwise).
pub(crate) fn draw_window(
    session: Option<&crate::net::session::NetSession>,
    canvas: &mut FlightCanvas,
    font: &Font,
) {
    if let Some(session) = session {
        session.chat.draw(canvas, font, Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_session::wire::chat::{ChatFrom, Standing};

    fn font() -> Font {
        Font {
            height: 8,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 6,
                    pixels: vec![(0, 0), (1, 0), (0, 1), (1, 1)],
                })
                .collect(),
        }
    }

    fn line(standing: Standing, receiver: Receiver, you: bool, text: &str) -> ChatLine {
        ChatLine {
            from: ChatFrom::Player {
                callsign: "Viper".into(),
                standing,
                you,
            },
            receiver,
            text: text.into(),
            sound: None,
        }
    }

    fn typed(chat: &mut Chat, text: &str) {
        for c in text.chars() {
            chat.key(&c.to_string(), Some(&c.to_string()), false, true);
        }
    }

    #[test]
    fn the_line_types_sends_to_the_picked_receiver_and_closes() {
        let mut chat = Chat::default();
        let now = Instant::now();
        assert!(!chat.is_open());
        assert_eq!(chat.key("a", Some("a"), false, true), Keyed::Nothing);
        chat.open(now);
        assert!(chat.is_open());
        typed(&mut chat, "Hi there");
        assert_eq!(chat.text(), "Hi there");
        chat.key("Backspace", None, false, true);
        assert_eq!(chat.text(), "Hi ther");
        // Tab goes All, Friendlies, Enemies, Wing, Target and round.
        let mut seen = Vec::new();
        for _ in 0..5 {
            chat.key("Tab", Some("\t"), false, true);
            seen.push(chat.receiver());
        }
        assert_eq!(
            seen,
            [
                Receiver::Friendlies,
                Receiver::Enemies,
                Receiver::Wing,
                Receiver::Target,
                Receiver::All
            ]
        );
        chat.key("Tab", None, true, true);
        assert_eq!(chat.receiver(), Receiver::Target, "Shift-Tab goes back");
        chat.key("Tab", None, true, true);
        chat.key("Tab", None, true, true);
        assert_eq!(chat.receiver(), Receiver::Enemies);
        assert_eq!(
            chat.key("Enter", Some("\r"), false, true),
            Keyed::Send(ChatSend::typed(Receiver::Enemies, "Hi ther"))
        );
        assert!(!chat.is_open());
        assert_eq!(chat.receiver(), Receiver::Enemies, "kept for the next line");
        chat.flight_ended();
        assert_eq!(chat.receiver(), Receiver::All);
    }

    #[test]
    fn esc_closes_without_sending_and_an_empty_enter_sends_nothing() {
        let mut chat = Chat::default();
        chat.open(Instant::now());
        typed(&mut chat, "never mind");
        assert_eq!(chat.key("Escape", None, false, true), Keyed::Closed);
        assert!(!chat.is_open());
        chat.open(Instant::now());
        assert_eq!(chat.text(), "", "a new line starts empty");
        typed(&mut chat, "   ");
        assert_eq!(chat.key("Enter", None, false, true), Keyed::Closed);
    }

    #[test]
    fn the_line_takes_printable_ascii_up_to_80_characters_and_no_chords() {
        let mut chat = Chat::default();
        chat.open(Instant::now());
        chat.key("é", Some("\u{e9}\u{7}x"), false, true);
        assert_eq!(chat.text(), "x", "only printable ASCII goes in");
        chat.key("c", Some("\u{3}"), false, false);
        chat.key("c", Some("c"), false, false);
        assert_eq!(chat.text(), "x", "Ctrl, Alt and Super types nothing");
        typed(&mut chat, &"y".repeat(100));
        assert_eq!(chat.text().chars().count(), MAX_TEXT);
        // Backtick and tilde are ordinary characters in an open line.
        chat.key("Backspace", None, false, true);
        chat.key("`", Some("~"), true, true);
        assert!(chat.text().ends_with('~'));
    }

    #[test]
    fn f_keys_send_the_matching_quick_message_only_while_the_line_is_open() {
        // The shipped file's shape, as synthetic text.
        let mut multiplayer = std::collections::BTreeMap::new();
        multiplayer.insert(
            tore_import::selection::CHAT_RESOURCE.to_owned(),
            b"send to all\\Hurry up\\^shwtime.5k\r\nsend to friendlies\\Get this guy off me\\^offme.5k\r\nI have no receiver\r\n".to_vec(),
        );
        load_quick_messages(&multiplayer);
        let mut chat = Chat::default();
        // Closed, the F keys are not the line's.
        assert_eq!(chat.key("F2", None, false, true), Keyed::Nothing);
        chat.open(Instant::now());
        match chat.key("F2", None, false, true) {
            Keyed::Send(send) => {
                assert_eq!(send.receiver, Receiver::Friendlies, "the line's own");
                assert_eq!(send.text, "Get this guy off me");
                let quick = send.quick.expect("a quick message");
                assert_eq!(quick.number, 2);
                assert_eq!(quick.sound.as_deref(), Some("^OFFME.5K"));
            }
            other => panic!("{other:?}"),
        }
        assert!(!chat.is_open(), "a quick message closes the line");
        // A line with no receiver goes to the one picked.
        chat.open(Instant::now());
        chat.key("Tab", None, false, true);
        chat.key("Tab", None, false, true);
        match chat.key("F3", None, false, true) {
            Keyed::Send(send) => assert_eq!(send.receiver, Receiver::Enemies),
            other => panic!("{other:?}"),
        }
        chat.open(Instant::now());
        assert_eq!(
            chat.key("F9", None, false, true),
            Keyed::Told("There is no quick message on F9.".into())
        );
        assert!(chat.is_open(), "the line stays open");
        assert_eq!(quick_number("F12"), Some(12));
        assert_eq!(quick_number("F13"), None);
        assert_eq!(quick_number("Fx"), None);
    }

    #[test]
    fn the_chat_key_is_the_one_above_tab_with_no_chord() {
        use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
        let none = ModifiersState::empty();
        let back = PhysicalKey::Code(KeyCode::Backquote);
        let other = PhysicalKey::Code(KeyCode::KeyA);
        assert!(is_chat_key(back, "`", none));
        assert!(is_chat_key(back, "~", ModifiersState::SHIFT));
        assert!(is_chat_key(back, "", none), "whatever the layout prints");
        assert!(is_chat_key(other, "~", ModifiersState::SHIFT));
        assert!(!is_chat_key(other, "a", none));
        assert!(!is_chat_key(back, "`", ModifiersState::CONTROL));
        assert!(!is_chat_key(back, "`", ModifiersState::ALT));
        assert!(!is_chat_key(back, "`", ModifiersState::SUPER));
    }

    fn canvas() -> FlightCanvas {
        let mut canvas = FlightCanvas::default();
        canvas.size = [1280, 960];
        canvas.pixels = vec![0; 1280 * 960 * 4];
        canvas
    }

    /// The colours drawn, by the strongest pixel of each colour.
    fn colours_drawn(canvas: &FlightCanvas) -> Vec<[u8; 3]> {
        let mut seen = Vec::new();
        for p in canvas.pixels.chunks_exact(4) {
            if p[3] > 200 && p[..3] != [0, 0, 0] && !seen.contains(&[p[0], p[1], p[2]]) {
                seen.push([p[0], p[1], p[2]]);
            }
        }
        seen
    }

    #[test]
    fn the_window_shows_lines_in_each_colour_then_fades_and_drops_them() {
        let mut chat = Chat::default();
        let font = font();
        let t0 = Instant::now();
        chat.push(&line(Standing::Own, Receiver::Wing, false, "own"), t0);
        chat.push(&line(Standing::Own, Receiver::All, false, "everyone"), t0);
        chat.push(&line(Standing::Enemy, Receiver::All, false, "enemy"), t0);
        chat.system("No one hears you.", t0);
        let mut c = canvas();
        chat.draw(&mut c, &font, t0 + Duration::from_secs(1));
        let drawn = colours_drawn(&c);
        for want in [tone::OWN_SIDE, tone::ALL, tone::ENEMY, tone::SYSTEM] {
            assert!(drawn.contains(&want), "{want:?} in {drawn:?}");
        }
        // The band behind is translucent black at the top left.
        let corner = &c.pixels[(8 * 1280 + 8) * 4..(8 * 1280 + 8) * 4 + 4];
        assert!(corner[3] > 0 && corner[3] < 255, "{corner:?}");
        // Nothing past the window's width: its right half is empty.
        assert!(
            c.pixels
                .chunks_exact(4)
                .enumerate()
                .all(|(i, p)| p[3] == 0 || (i % 1280) < 700),
            "the window stays at the left"
        );
        // Inside the last three seconds it fades.
        let mut faded = canvas();
        chat.draw(
            &mut faded,
            &font,
            t0 + Duration::from_secs_f64(LINE_SECONDS - 1.5),
        );
        let peak = |c: &FlightCanvas| c.pixels.chunks_exact(4).map(|p| p[3]).max().unwrap();
        assert!(peak(&faded) < peak(&c) && peak(&faded) > 0);
        // And past it, the window is gone.
        let mut gone = canvas();
        chat.draw(
            &mut gone,
            &font,
            t0 + Duration::from_secs_f64(LINE_SECONDS + 0.1),
        );
        assert_eq!(peak(&gone), 0);
        // The open line shows old lines again, and what is typed.
        chat.open(t0);
        typed(&mut chat, "hello");
        let mut open = canvas();
        chat.draw(&mut open, &font, t0 + Duration::from_secs(60));
        assert!(peak(&open) > 0, "an open line brings the lines back");
        let lit = |c: &FlightCanvas| c.pixels.chunks_exact(4).filter(|p| p[3] > 0).count();
        let mut closed_again = canvas();
        chat.key("Escape", None, false, true);
        chat.draw(&mut closed_again, &font, t0 + Duration::from_secs(60));
        assert!(lit(&open) > lit(&closed_again));
    }

    #[test]
    fn only_the_last_lines_show_and_long_ones_wrap_inside_the_window() {
        let mut chat = Chat::default();
        let font = font();
        let t0 = Instant::now();
        for n in 0..20 {
            chat.push(
                &line(Standing::Own, Receiver::All, false, &format!("line {n}")),
                t0,
            );
        }
        let mut c = canvas();
        chat.draw(&mut c, &font, t0);
        let rows_lit = |c: &FlightCanvas| {
            (0..960)
                .filter(|y| (0..1280).any(|x| c.pixels[(y * 1280 + x) * 4 + 3] > 200))
                .count()
        };
        let six = rows_lit(&c);
        let mut one = canvas();
        let mut chat1 = Chat::default();
        chat1.push(&line(Standing::Own, Receiver::All, false, "line 0"), t0);
        chat1.draw(&mut one, &font, t0);
        assert!(six > 4 * rows_lit(&one), "six lines against one");
        assert!(six < 8 * rows_lit(&one));
        // A line of 80 characters wraps inside the window's width.
        let words = vec!["word"; 16].join(" ");
        let pieces = wrap(&font, &words, 300);
        assert!(pieces.len() > 1);
        assert!(pieces.iter().all(|p| text_width(&font, p) <= 300));
        // A word wider than the window breaks.
        let long = "x".repeat(200);
        assert!(
            wrap(&font, &long, 300)
                .iter()
                .all(|p| text_width(&font, p) <= 300)
        );
    }

    /// The window's frame cost with the real HUD font, closed (six lines)
    /// and open, at three window sizes, from the imported pack:
    ///
    /// ```text
    /// TORE_DATA_DIR=... cargo test --release -p tore-app --locked \
    ///     net::chat::tests::time_chat_draw -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "timing; needs an imported data profile (TORE_DATA_DIR)"]
    fn time_chat_draw() {
        let dir = crate::assets::data_directory().expect("data directory");
        let assets = crate::assets::Assets::load(&dir).expect("imported pack");
        let font = Font::parse(&assets.theater_resources["HUD11.FNT"]).expect("HUD11.FNT");
        let t0 = Instant::now();
        let mut chat = Chat::default();
        for (standing, receiver, text) in [
            (
                Standing::Own,
                Receiver::All,
                "VIPER TO ALL: ANYONE SEE THAT BANDIT AT TWO O'CLOCK HIGH",
            ),
            (
                Standing::Own,
                Receiver::Wing,
                "COBRA TO WING: ON YOUR WING, BREAKING LEFT NOW",
            ),
            (
                Standing::Enemy,
                Receiver::All,
                "RAVEN TO ALL: YOU WILL NOT MAKE IT HOME TONIGHT",
            ),
            (
                Standing::Own,
                Receiver::Friendlies,
                "HAWK TO FRIENDLIES: I AM TAKING DAMAGE, RTB",
            ),
            (
                Standing::Own,
                Receiver::All,
                "VIPER TO ALL: WORM HAS TURNED",
            ),
            (
                Standing::Own,
                Receiver::All,
                "VIPER TO ALL: SPLASH ONE BANDIT!",
            ),
        ] {
            chat.push(&line(standing, receiver, false, text), t0);
        }
        chat.system("No one hears you.", t0);
        for [w, h] in [[1280u32, 960], [1920, 1080], [3840, 2160]] {
            let mut canvas = FlightCanvas::default();
            canvas.size = [w, h];
            canvas.pixels = vec![0; (w * h * 4) as usize];
            for open in [false, true] {
                if open {
                    chat.open(t0);
                    typed(&mut chat, "ready when you are");
                } else if chat.is_open() {
                    chat.key("Escape", None, false, true);
                }
                for _ in 0..20 {
                    chat.draw(&mut canvas, &font, t0);
                }
                let runs = 500;
                let started = Instant::now();
                for _ in 0..runs {
                    canvas.pixels.fill(0);
                    chat.draw(&mut canvas, &font, t0);
                }
                let each = started.elapsed() / runs;
                // The clear is the benchmark's own: take it out.
                let started = Instant::now();
                for _ in 0..runs {
                    canvas.pixels.fill(0);
                }
                let clear = started.elapsed() / runs;
                println!(
                    "{w}x{h} {}: {:?} a frame",
                    if open { "line open" } else { "window only" },
                    each.saturating_sub(clear)
                );
            }
        }
    }
}
