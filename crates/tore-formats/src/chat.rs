//! The retail quick-message file `CHAT.TXT`: up to twelve lines of
//! `receiver\text\sound`, one for each of the F1 to F12 keys while a player
//! types a message in multiplayer.
//!
//! Behaviour and evidence: `docs/spec/multiplayer.md` ("CHAT.TXT"). The file is
//! plain text with CRLF line ends and a Ctrl-Z end byte; it is not in any
//! archive, so the importer reads it as a loose file (installed game) or from
//! the disc's installer container and keeps its bytes in the pack under
//! `TORE_CHAT_V1`. This reader never fails: it takes what it can read, as the
//! retail reader does, and a file with nothing usable has no quick messages.

use crate::text::decode_cp437;

/// The most lines a file holds: F1 to F12.
pub const MAX_LINES: usize = 12;
/// Retail cuts a line at this many characters before looking at its fields.
pub const MAX_LINE: usize = 159;
/// Retail keeps the message text to this many characters.
pub const MAX_TEXT: usize = 50;
/// A last field is a sound name only when it is at most this long and ends in
/// `.5K` or `.11K` (any case).
pub const MAX_SOUND: usize = 12;

/// Who a line is sent to when its key is pressed. The five keywords are the
/// ones the retail program knows; the shipped file uses three of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Receiver {
    All,
    Friendlies,
    Enemies,
    Wing,
    Target,
}

impl Receiver {
    /// The keyword as written in the file, lower case.
    pub fn keyword(self) -> &'static str {
        match self {
            Receiver::All => "send to all",
            Receiver::Friendlies => "send to friendlies",
            Receiver::Enemies => "send to enemies",
            Receiver::Wing => "send to wing",
            Receiver::Target => "send to target",
        }
    }
    const ALL: [Receiver; 5] = [
        Receiver::All,
        Receiver::Friendlies,
        Receiver::Enemies,
        Receiver::Wing,
        Receiver::Target,
    ];
    fn from_field(field: &str) -> Option<Receiver> {
        let field = field.trim();
        Self::ALL
            .into_iter()
            .find(|receiver| field.eq_ignore_ascii_case(receiver.keyword()))
    }
}

/// One quick message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickMessage {
    /// The line's receiver, or `None` for a line without a keyword: it uses
    /// the receiver the player has currently picked.
    pub receiver: Option<Receiver>,
    /// The words sent, at most [`MAX_TEXT`] characters. Empty on a blank line
    /// (the slot is kept so F keys stay aligned with lines).
    pub text: String,
    /// The sound sent with the message, upper case (`^SHWTIME.5K`), or `None`
    /// for a line without a sound field.
    pub sound: Option<String>,
}

fn is_sound(field: &str) -> bool {
    field.len() <= MAX_SOUND && {
        let upper = field.to_ascii_uppercase();
        (upper.ends_with(".5K") && upper.len() > 3) || (upper.ends_with(".11K") && upper.len() > 4)
    }
}

fn parse_line(line: &str) -> QuickMessage {
    let line: String = line.chars().take(MAX_LINE).collect();
    let (receiver, rest) = match line.split_once('\\') {
        Some((first, rest)) => match Receiver::from_field(first) {
            Some(receiver) => (Some(receiver), rest),
            None => (None, line.as_str()),
        },
        None => (None, line.as_str()),
    };
    let (text, sound) = match rest.rsplit_once('\\') {
        Some((text, last)) if is_sound(last.trim()) => {
            (text, Some(last.trim().to_ascii_uppercase()))
        }
        _ => (rest, None),
    };
    QuickMessage {
        receiver,
        text: text.chars().take(MAX_TEXT).collect(),
        sound,
    }
}

/// Reads the quick messages of a `CHAT.TXT`: the lines before the Ctrl-Z end
/// byte (or the end of the file), at most twelve, in order. Line ends may be
/// CRLF or LF, and the last line needs none. A trailing run of blank lines is
/// dropped; a blank line in the middle keeps its slot.
pub fn parse(bytes: &[u8]) -> Vec<QuickMessage> {
    let end = bytes.iter().position(|b| *b == 0x1a).unwrap_or(bytes.len());
    let text = decode_cp437(&bytes[..end]);
    let mut messages: Vec<QuickMessage> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .take(MAX_LINES)
        .map(parse_line)
        .collect();
    while messages.last().is_some_and(|m| m.text.is_empty()) {
        messages.pop();
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(receiver: Option<Receiver>, text: &str, sound: Option<&str>) -> QuickMessage {
        QuickMessage {
            receiver,
            text: text.to_string(),
            sound: sound.map(str::to_string),
        }
    }

    /// The shipped file, as a synthetic copy of what the spec records.
    const SHIPPED: &str = "send to all\\Hurry up, I don't have all day\\^shwtime.5k\r\n\
send to all\\I don't like this\\^dntlike.5k\r\n\
send to all\\The worm has turned\\^worm.5k\r\n\
send to all\\Splash one bandit!\\^splbndt.5k\r\n\
send to friendlies\\Get this guy off me\\^offme.5k\r\n\
send to friendlies\\I'm going after him\\^igoaf.5k\r\n\
send to friendlies\\I'm taking damage\\^imdmge2.5k\r\n\
send to friendlies\\Who's side are you on?\\^whoside.5k\r\n\
send to target\\I've got a lock on you!\\&rwrlock.5k\r\n\
send to target\\Eat hot lead\\^hotlead.5k\r\n\
send to target\\Missile inbound! Break!\\^missbrk.5k\r\n\
send to target\\Eject! Eject! Eject!\\^ejectx3.5k\r\n\x1a\r\n\r\n";

    #[test]
    fn the_shipped_layout_gives_twelve_lines() {
        let messages = parse(SHIPPED.as_bytes());
        assert_eq!(messages.len(), 12);
        assert_eq!(
            messages[0],
            message(
                Some(Receiver::All),
                "Hurry up, I don't have all day",
                Some("^SHWTIME.5K")
            )
        );
        assert_eq!(messages[3].receiver, Some(Receiver::All));
        assert_eq!(messages[4].receiver, Some(Receiver::Friendlies));
        assert_eq!(messages[7].text, "Who's side are you on?");
        assert_eq!(messages[8].receiver, Some(Receiver::Target));
        assert_eq!(messages[8].sound.as_deref(), Some("&RWRLOCK.5K"));
        assert_eq!(messages[11].text, "Eject! Eject! Eject!");
        assert_eq!(messages[11].sound.as_deref(), Some("^EJECTX3.5K"));
    }

    #[test]
    fn receivers_match_without_regard_to_case_and_all_five_are_known() {
        let messages = parse(
            b"SEND TO ALL\\a\\^x.5K\r\nSend To Friendlies\\b\r\nsend to enemies\\c\r\nsend to wing\\d\r\nSEND TO TARGET\\e\r\n",
        );
        let receivers: Vec<_> = messages.iter().map(|m| m.receiver).collect();
        assert_eq!(
            receivers,
            [
                Some(Receiver::All),
                Some(Receiver::Friendlies),
                Some(Receiver::Enemies),
                Some(Receiver::Wing),
                Some(Receiver::Target)
            ]
        );
    }

    #[test]
    fn a_missing_sound_field_is_accepted() {
        let messages = parse(b"send to all\\No sound here\r\nJust words\r\n");
        assert_eq!(
            messages[0],
            message(Some(Receiver::All), "No sound here", None)
        );
        assert_eq!(messages[1], message(None, "Just words", None));
    }

    #[test]
    fn a_line_without_a_keyword_has_no_receiver() {
        let messages = parse(b"Cover me\\^cover.11k\r\n");
        assert_eq!(messages[0], message(None, "Cover me", Some("^COVER.11K")));
    }

    #[test]
    fn a_last_field_that_is_not_a_sound_stays_in_the_text() {
        // Not a sound extension, and a name longer than twelve characters.
        let messages = parse(b"send to all\\one\\two\r\nsend to all\\x\\^waytoolongname.5k\r\n");
        assert_eq!(messages[0].text, "one\\two");
        assert_eq!(messages[0].sound, None);
        assert_eq!(messages[1].text, "x\\^waytoolongname.5k");
        assert_eq!(messages[1].sound, None);
    }

    #[test]
    fn limits_cut_the_text_the_sound_and_the_line() {
        let long = "w".repeat(80);
        let line = format!("send to all\\{long}\\^a.5k\r\n");
        let messages = parse(line.as_bytes());
        assert_eq!(messages[0].text.len(), MAX_TEXT);
        assert_eq!(messages[0].sound.as_deref(), Some("^A.5K"));
        // Cut at 159 characters: the sound field is lost with the tail.
        let line = format!("send to all\\{}\\^a.5k\r\n", "w".repeat(150));
        let messages = parse(line.as_bytes());
        assert_eq!(messages[0].sound, None);
        assert_eq!(messages[0].text.len(), MAX_TEXT);
        // A twelve character sound is the longest accepted.
        let messages = parse(b"send to all\\x\\^abcdefgh.5k\r\nsend to all\\x\\^abcdefghi.5k\r\n");
        assert_eq!(messages[0].sound.as_deref(), Some("^ABCDEFGH.5K"));
        assert_eq!(messages[1].sound, None);
    }

    #[test]
    fn only_the_first_twelve_lines_count_and_ctrl_z_ends_the_file() {
        let mut text = String::new();
        for n in 0..15 {
            text.push_str(&format!("send to all\\line {n}\r\n"));
        }
        assert_eq!(parse(text.as_bytes()).len(), 12);
        let messages = parse(b"first\r\n\x1asecond\r\n");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "first");
    }

    #[test]
    fn line_ends_blank_lines_and_empty_input() {
        assert!(parse(b"").is_empty());
        assert!(parse(b"\x1a\r\n").is_empty());
        assert!(parse(b"\r\n\r\n").is_empty());
        let messages = parse(b"one\nsend to all\\\n\ntwo");
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[1].text, "");
        assert_eq!(messages[1].receiver, Some(Receiver::All));
        assert_eq!(messages[3].text, "two");
    }

    #[test]
    fn text_above_ascii_reads_as_the_dos_character_set() {
        let messages = parse(b"send to all\\Caf\x82\r\n");
        assert_eq!(messages[0].text, "Caf\u{e9}");
    }

    /// The retail file, when the install is present (the `gameassets` link or
    /// `TORE_GAME_DIR`); the test skips quietly otherwise.
    #[test]
    fn the_retail_file_gives_the_twelve_lines_of_the_spec() {
        let root = std::env::var_os("TORE_GAME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../gameassets/fighters-anthology")
            });
        let Ok(bytes) = std::fs::read(root.join("CHAT.TXT")) else {
            eprintln!("skipped: no retail CHAT.TXT");
            return;
        };
        let messages = parse(&bytes);
        let lines: Vec<_> = messages
            .iter()
            .map(|m| {
                (
                    m.receiver.map(Receiver::keyword),
                    m.text.as_str(),
                    m.sound.as_deref().unwrap_or(""),
                )
            })
            .collect();
        let all = Some("send to all");
        let friends = Some("send to friendlies");
        let target = Some("send to target");
        assert_eq!(
            lines,
            [
                (all, "Hurry up, I don't have all day", "^SHWTIME.5K"),
                (all, "I don't like this", "^DNTLIKE.5K"),
                (all, "The worm has turned", "^WORM.5K"),
                (all, "Splash one bandit!", "^SPLBNDT.5K"),
                (friends, "Get this guy off me", "^OFFME.5K"),
                (friends, "I'm going after him", "^IGOAF.5K"),
                (friends, "I'm taking damage", "^IMDMGE2.5K"),
                (friends, "Who's side are you on?", "^WHOSIDE.5K"),
                (target, "I've got a lock on you!", "&RWRLOCK.5K"),
                (target, "Eat hot lead", "^HOTLEAD.5K"),
                (target, "Missile inbound! Break!", "^MISSBRK.5K"),
                (target, "Eject! Eject! Eject!", "^EJECTX3.5K"),
            ]
        );
    }
}
