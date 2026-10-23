//! The game's rejoin tokens (stage K, slice K7b; docs/ARCHITECTURE.md,
//! "Rejoin tokens and reservations").
//!
//! The host gives every player a token when it joins (message 39). A token is
//! good only in the session that issued it, whichever machine hosts that
//! session now, and for 24 hours after its player was last connected (John,
//! 2026-09-28). The game keeps the tokens in its data folder so a player who
//! was dropped, or whose game stopped, can come back to the game and its
//! reserved aircraft: the screens mark such a game "Rejoin", and a join to it
//! sends the token by itself.
//!
//! One small text file, `rejoin-v1.conf`, beside the other preference files,
//! read and written as they are:
//!
//! ```text
//! tore-rejoin 1
//! token SESSION TOKEN LIFE LAST_SEEN ADDRESS CALLSIGN
//! ```
//!
//! `SESSION` is 16 hex digits, `TOKEN` 32, `LIFE` the seconds the host keeps
//! the token after the player was last connected, `LAST_SEEN` seconds since
//! the Unix epoch, `ADDRESS` the host's address when the game joined (`-` for
//! a game this game hosted) and `CALLSIGN` the rest of the line. The newest
//! entry is first. At most [`MAX_TOKENS`] are kept (*agent decision*, the
//! design's number); an expired one is dropped whenever the file is read or
//! written; a file that cannot be read is read as empty, with a line in the
//! log. The one-line [`FileStore`](tore_session::client::rejoin::FileStore)
//! is for bots and tests.

use std::path::{Path, PathBuf};
use tore_net::Token;
use tore_session::client::rejoin::{KeptToken, TokenStore, unix_now};

/// The file's name in the data folder.
pub const FILE: &str = "rejoin-v1.conf";
/// The most tokens kept.
pub const MAX_TOKENS: usize = 32;
/// The largest file read, a guard against a file that is not ours.
const MAX_FILE: usize = 16 * 1024;

/// One kept token and where the game joined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub kept: KeptToken,
    /// The host's address as the game joined it; empty for a game this game
    /// hosted.
    pub address: String,
}

/// The tokens in the file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Store {
    /// Newest first.
    pub entries: Vec<Entry>,
}

impl Store {
    /// The file's place in `data`.
    pub fn path(data: &Path) -> PathBuf {
        data.join(FILE)
    }

    /// The tokens kept in `data` that have not expired at `now_unix`.
    pub fn load(data: &Path, now_unix: u64) -> Self {
        let text = match crate::preferences::read(&Self::path(data)) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(error) => {
                log::warn!("Rejoin tokens not loaded: {error}");
                return Self::default();
            }
        };
        let mut store = Self::parse(&text).unwrap_or_else(|error| {
            log::warn!("Rejoin tokens not loaded: {error}; starting with none");
            Self::default()
        });
        store.drop_expired(now_unix);
        store
    }

    /// Writes the file.
    pub fn save(&self, data: &Path) -> std::io::Result<()> {
        crate::preferences::write(&Self::path(data), &self.text())
    }

    /// The token for the session `session_id`, when one is kept.
    pub fn for_session(&self, session_id: u64) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|entry| entry.kept.session_id == session_id)
    }

    /// The token kept for the host at `address`, when one is: what a join to
    /// an address typed (or a game found there) sends.
    pub fn for_address(&self, address: &str) -> Option<&Entry> {
        (!address.is_empty())
            .then(|| self.entries.iter().find(|entry| entry.address == address))
            .flatten()
    }

    /// Puts `kept` first, replacing the entry of its session; the oldest go
    /// past [`MAX_TOKENS`].
    pub fn keep(&mut self, kept: KeptToken, address: &str) {
        self.entries
            .retain(|entry| entry.kept.session_id != kept.session_id);
        // A game that was joined at this address before, in a session that
        // has gone, is not the game any more.
        if !address.is_empty() {
            self.entries.retain(|entry| entry.address != address);
        }
        self.entries.insert(
            0,
            Entry {
                kept,
                address: address.to_owned(),
            },
        );
        self.entries.truncate(MAX_TOKENS);
    }

    /// Drops the tokens the host would no longer honour at `now_unix`.
    pub fn drop_expired(&mut self, now_unix: u64) {
        self.entries.retain(|entry| !entry.kept.expired(now_unix));
    }

    /// The file's text.
    pub fn text(&self) -> String {
        let mut text = String::from("tore-rejoin 1\n");
        for entry in &self.entries {
            let kept = &entry.kept;
            text += &format!(
                "token {:016x} {:032x} {} {} {} {}\n",
                kept.session_id,
                kept.token.0,
                kept.life_seconds,
                kept.last_seen_unix,
                if entry.address.is_empty() {
                    "-"
                } else {
                    &entry.address
                },
                kept.callsign
            );
        }
        text
    }

    /// Reads the file's text; the first thing wrong is the error.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > MAX_FILE {
            return Err("the file is too large".into());
        }
        let mut lines = text.lines();
        if lines.next() != Some("tore-rejoin 1") {
            return Err("unsupported version".into());
        }
        let mut store = Self::default();
        for line in lines {
            let entry =
                Self::parse_entry(line).ok_or_else(|| format!("`{line}` is not a token"))?;
            store.entries.push(entry);
        }
        if store.entries.len() > MAX_TOKENS {
            return Err(format!("more than {MAX_TOKENS} tokens"));
        }
        Ok(store)
    }

    fn parse_entry(line: &str) -> Option<Entry> {
        let mut words = line.splitn(7, ' ');
        if words.next()? != "token" {
            return None;
        }
        let session_id = u64::from_str_radix(words.next()?, 16).ok()?;
        let token = u128::from_str_radix(words.next()?, 16).ok()?;
        let life_seconds = words.next()?.parse().ok()?;
        let last_seen_unix = words.next()?.parse().ok()?;
        let address = words.next()?;
        let callsign = words.next()?;
        if callsign.is_empty() || callsign.chars().any(char::is_control) {
            return None;
        }
        Some(Entry {
            kept: KeptToken {
                token: Token(token),
                session_id,
                callsign: callsign.to_owned(),
                life_seconds,
                last_seen_unix,
            },
            address: if address == "-" {
                String::new()
            } else {
                address.to_owned()
            },
        })
    }
}

/// The game's token store for one session's client: the host grants a token
/// (and the connection ends) and the file in the data folder takes it, with
/// the address the game joined.
#[derive(Clone, Debug)]
pub struct GameStore {
    data: PathBuf,
    address: String,
}

impl GameStore {
    /// A store writing into `data`, for a client that joined the host at
    /// `address` (empty for a game this game hosts).
    pub fn new(data: &Path, address: &str) -> Self {
        Self {
            data: data.to_owned(),
            address: address.to_owned(),
        }
    }
}

impl TokenStore for GameStore {
    fn keep(&mut self, kept: &KeptToken) {
        let mut store = Store::load(&self.data, unix_now());
        store.keep(kept.clone(), &self.address);
        // A file that cannot be written costs the player only the way back.
        if let Err(error) = store.save(&self.data) {
            log::warn!("Rejoin tokens not saved: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept(session: u64, last_seen: u64, callsign: &str) -> KeptToken {
        KeptToken {
            token: Token((u128::from(session) << 64) | 0xabcd),
            session_id: session,
            callsign: callsign.to_owned(),
            life_seconds: 86_400,
            last_seen_unix: last_seen,
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tore-rejoin-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_store_round_trips_through_its_text() {
        let mut store = Store::default();
        store.keep(kept(1, 1_000, "Viper"), "192.168.1.20:26900");
        store.keep(kept(2, 2_000, "Hawk Eye"), "");
        let again = Store::parse(&store.text()).unwrap();
        assert_eq!(again, store);
        assert_eq!(again.entries[0].kept.session_id, 2, "the newest is first");
        assert_eq!(
            again.entries[0].address, "",
            "a hosted game keeps no address"
        );
        assert_eq!(again.entries[1].kept.callsign, "Viper");
    }

    #[test]
    fn a_session_keeps_one_token_and_an_address_one_session() {
        let mut store = Store::default();
        store.keep(kept(1, 1_000, "Viper"), "10.0.0.1:26900");
        store.keep(kept(1, 1_500, "Viper"), "10.0.0.1:26900");
        assert_eq!(store.entries.len(), 1, "the same session is one entry");
        assert_eq!(store.entries[0].kept.last_seen_unix, 1_500);
        store.keep(kept(2, 2_000, "Viper"), "10.0.0.1:26900");
        assert_eq!(store.entries.len(), 1, "a new session at that address");
        assert_eq!(store.entries[0].kept.session_id, 2);
        assert!(store.for_session(1).is_none());
        assert!(store.for_address("10.0.0.1:26900").is_some());
        assert!(store.for_address("").is_none(), "no address finds none");
    }

    #[test]
    fn at_most_thirty_two_are_kept_and_the_oldest_go() {
        let mut store = Store::default();
        for session in 1..=40 {
            store.keep(kept(session, 1_000, "Viper"), "");
        }
        assert_eq!(store.entries.len(), MAX_TOKENS);
        assert!(store.for_session(40).is_some());
        assert!(store.for_session(8).is_none(), "the oldest eight went");
        assert!(store.for_session(9).is_some());
    }

    #[test]
    fn a_token_expires_twenty_four_hours_after_its_player_was_last_seen() {
        let mut store = Store::default();
        store.keep(kept(1, 1_000, "Viper"), "");
        store.keep(kept(2, 90_000, "Hawk"), "");
        store.drop_expired(1_000 + 86_400);
        assert_eq!(store.entries.len(), 2, "exactly 24 hours is still good");
        store.drop_expired(1_000 + 86_401);
        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries[0].kept.callsign, "Hawk");
    }

    #[test]
    fn a_game_store_writes_the_file_and_a_load_drops_expired_tokens() {
        let dir = scratch("game");
        let now = unix_now();
        let mut game = GameStore::new(&dir, "10.0.0.2:26900");
        game.keep(&kept(7, now, "Viper"));
        let mut other = GameStore::new(&dir, "");
        other.keep(&kept(8, now - 100_000, "Hawk"));
        let loaded = Store::load(&dir, now);
        assert_eq!(loaded.entries.len(), 1, "Hawk's token is a day old");
        let entry = loaded.for_session(7).expect("Viper's token");
        assert_eq!(entry.address, "10.0.0.2:26900");
        assert_eq!(entry.kept.token, kept(7, now, "Viper").token);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_or_damaged_file_is_no_tokens() {
        let dir = scratch("damaged");
        assert_eq!(Store::load(&dir, 0), Store::default());
        std::fs::write(Store::path(&dir), "tore-rejoin 1\ntoken zzz\n").unwrap();
        assert_eq!(Store::load(&dir, 0), Store::default());
        std::fs::write(Store::path(&dir), "something else\n").unwrap();
        assert_eq!(Store::load(&dir, 0), Store::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_that_is_too_large_or_not_ours_is_refused() {
        assert!(Store::parse(&"x".repeat(MAX_FILE + 1)).is_err());
        let line = |n: u64| format!("token {n:016x} {n:032x} 86400 1 - Viper\n");
        let many: String = (1..=33).map(line).collect();
        assert!(Store::parse(&format!("tore-rejoin 1\n{many}")).is_err());
        assert!(Store::parse("tore-rejoin 2\n").is_err());
    }
}
