//! The client's side of rejoin (stage K; docs/ARCHITECTURE.md, "Rejoin
//! tokens and reservations"). Slice K5.
//!
//! The host grants every player a token when it joins (message 39). The
//! client keeps it ([`Client::token`]) and hands it to the game's
//! [`TokenStore`], which keeps it where the game wants (the game's data
//! folder, `rejoin-v1.conf`, is slice K7b's; a bot's file is
//! [`FileStore`]). A game sends a token it holds for the session it is
//! joining in its Challenge answer ([`super::ClientConfig::token`]), or, when
//! it learned the session only from Accepted, afterwards in a Rejoin message
//! ([`Client::rejoin`]). The host answers a token that no longer works in
//! words (a Notice, or a refusal of the Rejoin) and the game plays on as a
//! new player.

use super::Client;
use crate::wire::messages::Message;
use crate::wire::migration::TokenGrant;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tore_net::Token;

/// A token a game holds, with what it needs to use it later.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptToken {
    pub token: Token,
    /// The session that issued it: a token is good in that session only.
    pub session_id: u64,
    /// The callsign the game joined with.
    pub callsign: String,
    /// Seconds the host keeps it after the player was last connected.
    pub life_seconds: u32,
    /// When the game last held it connected, seconds since the Unix epoch.
    pub last_seen_unix: u64,
}

impl KeptToken {
    /// Whether the host would no longer honor it at `now_unix`.
    pub fn expired(&self, now_unix: u64) -> bool {
        now_unix.saturating_sub(self.last_seen_unix) > u64::from(self.life_seconds)
    }
}

/// Where a game keeps its tokens. Called when the host grants one and when
/// the connection ends (the last time it was connected).
pub trait TokenStore: Send {
    fn keep(&mut self, kept: &KeptToken);
}

/// Seconds since the Unix epoch, 0 if the clock is before it.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// One token in a file of its own, for a bot or a test: a single line
/// `rejoin-v1 SESSION TOKEN LIFE LAST_SEEN CALLSIGN` (the session as 16 hex
/// digits, the token as 32, the times in decimal), written whole each time.
#[derive(Clone, Debug)]
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The token in the file, if there is one that has not expired.
    pub fn load(path: &Path) -> Option<KeptToken> {
        let text = std::fs::read_to_string(path).ok()?;
        let kept = parse(text.trim())?;
        (!kept.expired(unix_now())).then_some(kept)
    }
}

impl TokenStore for FileStore {
    fn keep(&mut self, kept: &KeptToken) {
        // A failure to write is the game's to ignore: it plays on, and the
        // player loses only the way back.
        let _: io::Result<()> = std::fs::write(&self.path, format!("{}\n", line(kept)));
    }
}

/// The file's line for `kept`.
pub fn line(kept: &KeptToken) -> String {
    format!(
        "rejoin-v1 {:016x} {:032x} {} {} {}",
        kept.session_id, kept.token.0, kept.life_seconds, kept.last_seen_unix, kept.callsign
    )
}

/// The token a file's line holds.
pub fn parse(text: &str) -> Option<KeptToken> {
    let mut words = text.splitn(6, ' ');
    if words.next()? != "rejoin-v1" {
        return None;
    }
    let session_id = u64::from_str_radix(words.next()?, 16).ok()?;
    let token = u128::from_str_radix(words.next()?, 16).ok()?;
    Some(KeptToken {
        token: Token(token),
        session_id,
        life_seconds: words.next()?.parse().ok()?,
        last_seen_unix: words.next()?.parse().ok()?,
        callsign: words.next()?.to_owned(),
    })
}

/// What the client keeps of its token.
#[derive(Default)]
pub(super) struct Kept {
    pub(super) session_id: u64,
    pub(super) kept: Option<KeptToken>,
    pub(super) store: Option<Box<dyn TokenStore>>,
}

impl Client {
    /// The token the host granted this player, once it has.
    pub fn token(&self) -> Option<&KeptToken> {
        self.rejoin.kept.as_ref()
    }

    /// Hands every token the host grants from now on to `store` (and the
    /// one already granted).
    pub fn set_token_store(&mut self, store: Box<dyn TokenStore>) {
        self.rejoin.store = Some(store);
        if let (Some(kept), Some(store)) = (&self.rejoin.kept, &mut self.rejoin.store) {
            store.keep(kept);
        }
    }

    /// Sends a Rejoin: this game joined without sending its token but holds
    /// one for this session. The host answers with the player's own
    /// identity back (a Token and a Notice) or a refusal in words
    /// ([`super::ClientEvent::Refused`]).
    pub fn rejoin(&mut self, token: Token) {
        let now = self.now;
        self.request(now, Message::Rejoin(token));
    }

    /// The host accepted the join: the session's id, for the token.
    pub(super) fn rejoin_session(&mut self, session_id: u64) {
        self.rejoin.session_id = session_id;
    }

    /// The host granted this player's token (message 39).
    pub(super) fn rejoin_token(&mut self, grant: TokenGrant) {
        let kept = KeptToken {
            token: grant.token,
            session_id: self.rejoin.session_id,
            callsign: self.config.callsign.clone(),
            life_seconds: grant.life_seconds,
            last_seen_unix: unix_now(),
        };
        self.log(
            "token",
            &[
                &format!("{:016x}", kept.session_id),
                &grant.life_seconds.to_string(),
            ],
        );
        if let Some(store) = &mut self.rejoin.store {
            store.keep(&kept);
        }
        self.rejoin.kept = Some(kept);
    }

    /// The connection ended: the token's 24 hours start from now.
    pub(super) fn rejoin_closed(&mut self) {
        if let Some(kept) = &mut self.rejoin.kept {
            kept.last_seen_unix = unix_now();
            if let Some(store) = &mut self.rejoin.store {
                store.keep(kept);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept() -> KeptToken {
        KeptToken {
            token: Token(0x0123_4567_89ab_cdef_0011_2233_4455_6677),
            session_id: 0xdead_beef_0000_0042,
            callsign: "Viper 2".into(),
            life_seconds: 86_400,
            last_seen_unix: 1_790_000_000,
        }
    }

    #[test]
    fn a_tokens_line_round_trips_with_a_callsign_that_has_a_space() {
        let kept = kept();
        assert_eq!(parse(&line(&kept)), Some(kept));
    }

    #[test]
    fn a_damaged_line_is_not_a_token() {
        for text in [
            "",
            "rejoin-v2 0 0 1 1 x",
            "rejoin-v1 zz 00 1 1 x",
            "rejoin-v1 0000000000000001 00000000000000000000000000000001 x 1 x",
            "rejoin-v1 0000000000000001 00000000000000000000000000000001 1 1",
        ] {
            assert_eq!(parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_token_expires_24_hours_after_it_was_last_seen() {
        let kept = kept();
        assert!(!kept.expired(kept.last_seen_unix + 86_400));
        assert!(kept.expired(kept.last_seen_unix + 86_401));
        assert!(!kept.expired(kept.last_seen_unix.saturating_sub(5)));
    }

    #[test]
    fn a_file_store_keeps_one_token_and_loads_it_unless_it_expired() {
        let dir = std::env::temp_dir().join(format!("tore-k5-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("token.txt");
        let mut store = FileStore::new(&path);
        let mut fresh = kept();
        fresh.last_seen_unix = unix_now();
        store.keep(&fresh);
        assert_eq!(FileStore::load(&path), Some(fresh.clone()));
        let mut old = fresh;
        old.last_seen_unix = unix_now().saturating_sub(90_000);
        store.keep(&old);
        assert_eq!(FileStore::load(&path), None);
        assert_eq!(FileStore::load(&dir.join("missing")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
