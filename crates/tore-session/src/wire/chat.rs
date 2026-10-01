//! Chat on the wire (slice EF6, protocol 4): a player's line to the host,
//! the host's delivered line, the limits the host holds a player to, and
//! the words and tones every end uses to show a line.
//!
//! The routing rules are in docs/ARCHITECTURE.md ("Chat") and the bytes in
//! docs/formats/net-protocol.md ("Messages as built"). Everything here is
//! plain data and rules; the host decides who hears a line
//! (`host::chat`) and the game draws it (`net/chat`).

use super::bits::{read_option, read_str, write_option, write_str};
use super::{WireError, WireResult};
use std::time::Duration;
use tore_codec::{BitReader, BitWriter};

pub use tore_formats::chat::{QuickMessage, Receiver};

/// The longest line a player types, characters (agent proposal, EF design;
/// retail's `CHAT.TXT` lines are cut at 50, which is also the longest quick
/// message).
pub const MAX_TEXT: usize = 80;
/// Lines a player may send in [`RATE_WINDOW`]; more are refused.
pub const RATE_LINES: usize = 5;
/// The window the rate limit counts in.
pub const RATE_WINDOW: Duration = Duration::from_secs(5);
/// The longest sound name a quick message carries, as `CHAT.TXT` allows.
pub const MAX_SOUND: usize = tore_formats::chat::MAX_SOUND;
/// The quick messages: numbers 1 to 12, the F1 to F12 keys.
pub const QUICK_MESSAGES: u8 = tore_formats::chat::MAX_LINES as u8;

/// Why a line is not sent, in the words the player is told. The host speaks
/// them as system lines; the client checks the first four before sending, so
/// a player is told at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Nothing but spaces.
    Empty,
    /// More than [`MAX_TEXT`] characters.
    TooLong,
    /// A character outside printable ASCII.
    Unprintable,
    /// A quick message number outside 1 to 12, or a sound name that is not
    /// one.
    BadQuick,
    /// Before flight only All works.
    OnlyAll,
    /// More than [`RATE_LINES`] lines in [`RATE_WINDOW`].
    TooFast,
    /// Target, and the sender has none designated.
    NoTarget,
    /// The game is not connected to a host (never sent by the host).
    NotConnected,
}

impl Refusal {
    /// The plain words.
    pub fn text(self) -> &'static str {
        match self {
            Self::Empty => "There is nothing to send.",
            Self::TooLong => "That line is too long: 80 characters at most.",
            Self::Unprintable => "Chat takes letters, digits and punctuation only.",
            Self::BadQuick => "There is no such quick message.",
            Self::OnlyAll => "Before flight you can only send to All.",
            Self::TooFast => "You are sending too fast: 5 lines in 5 seconds at most.",
            Self::NoTarget => "You have no target designated.",
            Self::NotConnected => "You are not connected to a game.",
        }
    }
}

/// What the sender is told when nobody else hears a line (agent decision on
/// the words).
pub const NO_ONE_HEARS: &str = "No one hears you.";

/// A line's text as it is sent: the spaces cut off both ends, and every
/// character printable ASCII (the retail fonts' range) within [`MAX_TEXT`].
pub fn clean_text(text: &str) -> Result<&str, Refusal> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Refusal::Empty);
    }
    if text.chars().count() > MAX_TEXT {
        return Err(Refusal::TooLong);
    }
    if !text.chars().all(printable) {
        return Err(Refusal::Unprintable);
    }
    Ok(text)
}

/// A character the chat takes: printable ASCII.
pub fn printable(c: char) -> bool {
    (' '..='~').contains(&c)
}

/// A quick message's line made fit to send: any character outside printable
/// ASCII (a code page 437 symbol in an installed file) becomes `?`.
pub fn plain(text: &str) -> String {
    text.chars()
        .map(|c| if printable(c) { c } else { '?' })
        .collect()
}

/// Whether `name` is a sound name as `CHAT.TXT` writes them: at most
/// [`MAX_SOUND`] characters of printable ASCII, no backslash, ending in
/// `.5K` or `.11K` (any case).
pub fn sound_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    name.len() <= MAX_SOUND
        && name.chars().all(|c| printable(c) && c != '\\')
        && ((upper.ends_with(".5K") && upper.len() > 3)
            || (upper.ends_with(".11K") && upper.len() > 4))
}

/// The receiver's name as lines show it.
pub fn receiver_label(receiver: Receiver) -> &'static str {
    match receiver {
        Receiver::All => "ALL",
        Receiver::Friendlies => "FRIENDLIES",
        Receiver::Enemies => "ENEMIES",
        Receiver::Wing => "WING",
        Receiver::Target => "TARGET",
    }
}

/// The receiver after `receiver` in Tab's order: All, Friendlies, Enemies,
/// Wing, Target, and round. `flying` is false in the lobby, where only All
/// is sent to (a Tab stays on All).
pub fn next_receiver(receiver: Receiver, flying: bool) -> Receiver {
    if !flying {
        return Receiver::All;
    }
    match receiver {
        Receiver::All => Receiver::Friendlies,
        Receiver::Friendlies => Receiver::Enemies,
        Receiver::Enemies => Receiver::Wing,
        Receiver::Wing => Receiver::Target,
        Receiver::Target => Receiver::All,
    }
}

fn receiver_code(receiver: Receiver) -> u64 {
    match receiver {
        Receiver::All => 0,
        Receiver::Friendlies => 1,
        Receiver::Enemies => 2,
        Receiver::Wing => 3,
        Receiver::Target => 4,
    }
}

fn read_receiver(r: &mut BitReader<'_>) -> WireResult<Receiver> {
    Ok(match r.read_bits(3)? {
        0 => Receiver::All,
        1 => Receiver::Friendlies,
        2 => Receiver::Enemies,
        3 => Receiver::Wing,
        4 => Receiver::Target,
        _ => return Err(WireError::Invalid("chat receiver")),
    })
}

/// A quick message's number and sound, sent with its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quick {
    /// 1 to 12: the F key.
    pub number: u8,
    /// The sound the receivers hear, as `CHAT.TXT` names it (upper case).
    pub sound: Option<String>,
}

/// A player's line (client to host).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatSend {
    pub receiver: Receiver,
    pub text: String,
    /// Set when the line is one of `CHAT.TXT`'s.
    pub quick: Option<Quick>,
}

impl ChatSend {
    /// A typed line.
    pub fn typed(receiver: Receiver, text: &str) -> Self {
        Self {
            receiver,
            text: text.to_owned(),
            quick: None,
        }
    }

    /// `CHAT.TXT`'s line number `number` (1 to 12, the F key) as a line to
    /// send: to the line's own receiver, or `picked` (the receiver the
    /// player has chosen) when the line names none.
    pub fn quick(number: u8, line: &QuickMessage, picked: Receiver) -> Self {
        Self {
            receiver: line.receiver.unwrap_or(picked),
            text: plain(&line.text),
            quick: Some(Quick {
                number,
                sound: line.sound.clone(),
            }),
        }
    }

    /// The body's bits.
    pub fn write(&self, w: &mut BitWriter) {
        let _ = w.write_bits(receiver_code(self.receiver), 3);
        write_str(w, &self.text);
        write_option(w, self.quick.as_ref(), |w, quick| {
            let _ = w.write_bits(u64::from(quick.number), 4);
            write_option(w, quick.sound.as_deref(), write_str);
        });
    }

    /// Reads the body's bits.
    pub fn read(r: &mut BitReader<'_>) -> WireResult<Self> {
        let receiver = read_receiver(r)?;
        let text = read_str(r)?;
        let quick = read_option(r, |r| {
            Ok(Quick {
                number: r.read_bits(4)? as u8,
                sound: read_option(r, read_str)?,
            })
        })?;
        Ok(Self {
            receiver,
            text,
            quick,
        })
    }

    /// The line checked by the rules both ends hold: the text clean, a quick
    /// message's number and sound sane. Returns the line to send (text cut
    /// of its spaces).
    pub fn checked(&self) -> Result<Self, Refusal> {
        let text = clean_text(&self.text)?;
        if let Some(quick) = &self.quick
            && (!(1..=QUICK_MESSAGES).contains(&quick.number)
                || quick.sound.as_deref().is_some_and(|s| !sound_name(s)))
        {
            return Err(Refusal::BadQuick);
        }
        Ok(Self {
            receiver: self.receiver,
            text: text.to_owned(),
            quick: self.quick.clone(),
        })
    }
}

/// Where the sender stands to the one who reads the line: the host works it
/// out for each receiver from the sides of their planes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// One of them has no plane (the lobby), so no side.
    Neutral,
    /// The same side as the reader.
    Own,
    /// The other side.
    Enemy,
}

/// Who a delivered line is from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatFrom {
    /// The host, to the one player: a refusal, or that nobody heard.
    System,
    /// A player. `you` is the reader's own line, which the host sends back
    /// so the sender sees what went out.
    Player {
        callsign: String,
        standing: Standing,
        you: bool,
    },
}

/// A line the host delivers (host to client).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatLine {
    pub from: ChatFrom,
    pub receiver: Receiver,
    pub text: String,
    /// A quick message's sound, for those who hear it (not the sender).
    pub sound: Option<String>,
}

/// The colour class of a line in the chat window (John, 2026-10-01).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Green: from the reader's own side, to the side, the wing or the
    /// reader's target.
    Own,
    /// Blue: to everyone, from the reader's side or from the lobby.
    Everyone,
    /// Red: from the other side.
    Enemy,
    /// A colour of its own: the host's words.
    System,
}

impl ChatLine {
    /// A line from the host.
    pub fn system(text: &str) -> Self {
        Self {
            from: ChatFrom::System,
            receiver: Receiver::All,
            text: text.to_owned(),
            sound: None,
        }
    }

    /// The colour class.
    pub fn tone(&self) -> Tone {
        match &self.from {
            ChatFrom::System => Tone::System,
            ChatFrom::Player {
                standing: Standing::Enemy,
                ..
            } => Tone::Enemy,
            ChatFrom::Player { .. } if self.receiver == Receiver::All => Tone::Everyone,
            ChatFrom::Player { .. } => Tone::Own,
        }
    }

    /// The line as the window shows it: `YOU TO ALL: text` for the reader's
    /// own, `VIPER TO WING: text` for another's, `VIPER TO YOU: text` for a
    /// line to the reader's own plane as its target, and the text alone for
    /// the host's. Retail's `YOU TO ALL` forms, with the sender's callsign
    /// in place of `YOU`
    /// ([the spec](../../../../docs/spec/multiplayer.md#in-flight-s)).
    pub fn display(&self) -> String {
        match &self.from {
            ChatFrom::System => self.text.clone(),
            ChatFrom::Player { you: true, .. } => {
                format!("YOU TO {}: {}", receiver_label(self.receiver), self.text)
            }
            ChatFrom::Player { callsign, .. } => {
                let to = if self.receiver == Receiver::Target {
                    "YOU"
                } else {
                    receiver_label(self.receiver)
                };
                format!("{callsign} TO {to}: {}", self.text)
            }
        }
    }

    /// The line for a log: sender, receiver and text.
    pub fn log_text(&self) -> String {
        match &self.from {
            ChatFrom::System => format!("(host) {}", self.text),
            ChatFrom::Player { callsign, .. } => format!(
                "{callsign} to {}: {}",
                receiver_label(self.receiver).to_ascii_lowercase(),
                self.text
            ),
        }
    }

    /// The body's bits.
    pub fn write(&self, w: &mut BitWriter) {
        match &self.from {
            ChatFrom::System => {
                w.write_bool(false);
                write_str(w, &self.text);
            }
            ChatFrom::Player {
                callsign,
                standing,
                you,
            } => {
                w.write_bool(true);
                write_str(w, callsign);
                let _ = w.write_bits(
                    match standing {
                        Standing::Neutral => 0,
                        Standing::Own => 1,
                        Standing::Enemy => 2,
                    },
                    2,
                );
                w.write_bool(*you);
                let _ = w.write_bits(receiver_code(self.receiver), 3);
                write_str(w, &self.text);
                write_option(w, self.sound.as_deref(), write_str);
            }
        }
    }

    /// Reads the body's bits.
    pub fn read(r: &mut BitReader<'_>) -> WireResult<Self> {
        if !r.read_bool()? {
            return Ok(Self::system(&read_str(r)?));
        }
        let callsign = read_str(r)?;
        let standing = match r.read_bits(2)? {
            0 => Standing::Neutral,
            1 => Standing::Own,
            2 => Standing::Enemy,
            _ => return Err(WireError::Invalid("chat standing")),
        };
        let you = r.read_bool()?;
        let receiver = read_receiver(r)?;
        Ok(Self {
            from: ChatFrom::Player {
                callsign,
                standing,
                you,
            },
            receiver,
            text: read_str(r)?,
            sound: read_option(r, read_str)?,
        })
    }
}

/// Counts a player's lines against [`RATE_LINES`] in [`RATE_WINDOW`].
#[derive(Clone, Debug, Default)]
pub struct RateLimit {
    sent: std::collections::VecDeque<Duration>,
}

impl RateLimit {
    /// Takes a line sent at `now`: `true` when it is within the limit (and
    /// counted), `false` when it is the sixth in the window (not counted).
    pub fn allow(&mut self, now: Duration) -> bool {
        while self
            .sent
            .front()
            .is_some_and(|at| now.saturating_sub(*at) >= RATE_WINDOW)
        {
            self.sent.pop_front();
        }
        if self.sent.len() >= RATE_LINES {
            return false;
        }
        self.sent.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_trimmed_printable_and_at_most_80_characters() {
        assert_eq!(clean_text("  hello there ").unwrap(), "hello there");
        assert_eq!(clean_text("   "), Err(Refusal::Empty));
        assert_eq!(clean_text(&"x".repeat(80)).unwrap().len(), 80);
        assert_eq!(clean_text(&"x".repeat(81)), Err(Refusal::TooLong));
        assert_eq!(clean_text("tab\there"), Err(Refusal::Unprintable));
        assert_eq!(clean_text("caf\u{e9}"), Err(Refusal::Unprintable));
        assert_eq!(clean_text("tilde ~ and `"), Ok("tilde ~ and `"));
    }

    #[test]
    fn a_quick_message_is_checked_for_its_number_and_sound() {
        let send = |number, sound: Option<&str>| ChatSend {
            receiver: Receiver::All,
            text: "Hurry up".into(),
            quick: Some(Quick {
                number,
                sound: sound.map(str::to_owned),
            }),
        };
        assert!(send(1, Some("^SHWTIME.5K")).checked().is_ok());
        assert!(send(12, None).checked().is_ok());
        assert_eq!(send(0, None).checked(), Err(Refusal::BadQuick));
        assert_eq!(send(13, None).checked(), Err(Refusal::BadQuick));
        assert_eq!(send(1, Some("..\\x.5K")).checked(), Err(Refusal::BadQuick));
        assert_eq!(
            send(1, Some("^TOOLONGNAME.5K")).checked(),
            Err(Refusal::BadQuick)
        );
        assert_eq!(send(1, Some("^X.WAV")).checked(), Err(Refusal::BadQuick));
        assert_eq!(plain("a\u{263a}b"), "a?b");
    }

    #[test]
    fn tab_goes_round_the_receivers_and_stays_on_all_before_flight() {
        let mut r = Receiver::All;
        let mut seen = Vec::new();
        for _ in 0..5 {
            r = next_receiver(r, true);
            seen.push(r);
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
        assert_eq!(next_receiver(Receiver::All, false), Receiver::All);
    }

    #[test]
    fn five_lines_in_five_seconds_and_no_more() {
        let mut limit = RateLimit::default();
        let s = Duration::from_secs;
        for i in 0..5 {
            assert!(limit.allow(s(10) + Duration::from_millis(i * 100)), "{i}");
        }
        assert!(!limit.allow(s(11)), "the sixth in the window");
        assert!(!limit.allow(s(14)), "still inside the window");
        assert!(limit.allow(s(15)), "the first has left the window");
        assert!(limit.allow(s(15) + Duration::from_millis(100)));
    }

    fn line(standing: Standing, receiver: Receiver, you: bool) -> ChatLine {
        ChatLine {
            from: ChatFrom::Player {
                callsign: "Viper".into(),
                standing,
                you,
            },
            receiver,
            text: "Break left".into(),
            sound: None,
        }
    }

    #[test]
    fn lines_have_retails_names_and_johns_colours() {
        let own = line(Standing::Own, Receiver::Wing, false);
        assert_eq!(own.display(), "Viper TO WING: Break left");
        assert_eq!(own.tone(), Tone::Own);
        let all = line(Standing::Own, Receiver::All, true);
        assert_eq!(all.display(), "YOU TO ALL: Break left");
        assert_eq!(all.tone(), Tone::Everyone);
        let target = line(Standing::Own, Receiver::Target, false);
        assert_eq!(target.display(), "Viper TO YOU: Break left");
        let enemy = line(Standing::Enemy, Receiver::All, false);
        assert_eq!(enemy.tone(), Tone::Enemy, "the enemy's wins over All");
        assert_eq!(
            line(Standing::Neutral, Receiver::All, false).tone(),
            Tone::Everyone
        );
        assert_eq!(ChatLine::system("No one hears you.").tone(), Tone::System);
        assert_eq!(
            ChatLine::system("No one hears you.").display(),
            "No one hears you."
        );
    }
}
