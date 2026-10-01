//! What the multiplayer screens remember between sessions (slice EF5): the
//! player's callsign, the last few addresses joined (most recent first), the
//! port and the last game name hosted. One small text file in the data folder
//! beside the other preference files (`network-v1.conf`), read and written as
//! they are. `--connect` and `--host` remember too.
//!
//! The password is never kept: the file is plain text, and a password for one
//! game is a thing to type again (agent decision).
//!
//! ```text
//! tore-network 1
//! callsign Viper
//! port 26900
//! game-name Friday night
//! address 192.168.1.20:26900
//! address game.example.org:26900
//! ```
//!
//! A line is a key, a space and the rest of the line. A key may not repeat
//! except `address`. A file that is malformed is read as the defaults, with a
//! line in the log, and replaced the next time something is remembered.

use crate::net::options::{
    ConnectOptions, DEFAULT_CALLSIGN, DEFAULT_PORT, HostOptions, MAX_NAME, callsign_problem,
};
use std::path::{Path, PathBuf};

/// The file's name in the data folder.
pub const FILE: &str = "network-v1.conf";
/// Addresses kept, most recent first.
pub const MAX_ADDRESSES: usize = 8;
/// The longest address kept, in bytes.
const MAX_ADDRESS: usize = 255;

/// The remembered choices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remembered {
    /// The player's callsign, when one was chosen.
    pub callsign: Option<String>,
    /// The game port, for hosting and for an address typed without one.
    pub port: u16,
    /// The last game name hosted.
    pub game_name: Option<String>,
    /// The addresses last joined, most recent first.
    pub addresses: Vec<String>,
}

impl Default for Remembered {
    fn default() -> Self {
        Self {
            callsign: None,
            port: DEFAULT_PORT,
            game_name: None,
            addresses: Vec::new(),
        }
    }
}

impl Remembered {
    /// The file's place in `data`.
    pub fn path(data: &Path) -> PathBuf {
        data.join(FILE)
    }

    /// The remembered choices in `data`; none when the file is missing, and
    /// the defaults (with a line in the log) when it cannot be read.
    pub fn load(data: &Path) -> Self {
        match crate::preferences::read(&Self::path(data)) {
            Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
                log::warn!("Network settings not loaded: {error}; using the defaults");
                Self::default()
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                log::warn!("Network settings not loaded: {error}; using the defaults");
                Self::default()
            }
        }
    }

    /// Writes the file.
    pub fn save(&self, data: &Path) -> std::io::Result<()> {
        crate::preferences::write(&Self::path(data), &self.text())
    }

    /// Puts `address` first, as typed and trimmed, and drops the oldest past
    /// [`MAX_ADDRESSES`]. An address already kept (in any letter case) moves
    /// to the front instead of repeating. Text that is not an address is not
    /// kept.
    pub fn remember_address(&mut self, address: &str) {
        let address = address.trim();
        if address.is_empty()
            || address.len() > MAX_ADDRESS
            || address.chars().any(char::is_control)
            || tore_net::reach::split_address(address, DEFAULT_PORT).is_err()
        {
            return;
        }
        self.addresses
            .retain(|kept| !kept.eq_ignore_ascii_case(address));
        self.addresses.insert(0, address.to_owned());
        self.addresses.truncate(MAX_ADDRESSES);
    }

    /// Keeps the callsign when it is a valid one.
    pub fn remember_callsign(&mut self, callsign: &str) {
        if callsign_problem(callsign).is_none() {
            self.callsign = Some(callsign.to_owned());
        }
    }

    /// Keeps the game name when it is a valid one.
    pub fn remember_game_name(&mut self, name: &str) {
        if name_ok(name) {
            self.game_name = Some(name.to_owned());
        }
    }

    /// The file's text.
    pub fn text(&self) -> String {
        let mut text = String::from("tore-network 1\n");
        if let Some(callsign) = &self.callsign {
            text += &format!("callsign {callsign}\n");
        }
        text += &format!("port {}\n", self.port);
        if let Some(name) = &self.game_name {
            text += &format!("game-name {name}\n");
        }
        for address in &self.addresses {
            text += &format!("address {address}\n");
        }
        text
    }

    /// Reads the file's text; the first thing wrong is the error.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 8192 {
            return Err("the file is too large".into());
        }
        let mut lines = text.lines();
        if lines.next() != Some("tore-network 1") {
            return Err("unsupported version".into());
        }
        let mut found = Self::default();
        let mut seen: Vec<&str> = Vec::new();
        for line in lines {
            let (key, value) = line
                .split_once(' ')
                .ok_or_else(|| format!("`{line}` is not a key and a value"))?;
            if key != "address" {
                if seen.contains(&key) {
                    return Err(format!("`{key}` appears twice"));
                }
                seen.push(key);
            }
            match key {
                "callsign" => {
                    if let Some(problem) = callsign_problem(value) {
                        return Err(format!("callsign: {problem}"));
                    }
                    found.callsign = Some(value.to_owned());
                }
                "port" => {
                    found.port = value
                        .parse::<u16>()
                        .ok()
                        .filter(|port| *port != 0)
                        .ok_or_else(|| format!("port `{value}` is not 1 to 65535"))?;
                }
                "game-name" => {
                    if !name_ok(value) {
                        return Err(format!(
                            "game-name must be 1 to {MAX_NAME} printable characters"
                        ));
                    }
                    found.game_name = Some(value.to_owned());
                }
                "address" => {
                    let before = found.addresses.len();
                    // Kept in the file's order, which is most recent first.
                    if value.is_empty()
                        || value.len() > MAX_ADDRESS
                        || value.chars().any(char::is_control)
                        || tore_net::reach::split_address(value, DEFAULT_PORT).is_err()
                    {
                        return Err(format!("`{value}` is not an address"));
                    }
                    if found.addresses.len() >= MAX_ADDRESSES {
                        return Err(format!("more than {MAX_ADDRESSES} addresses"));
                    }
                    found.addresses.push(value.to_owned());
                    debug_assert_eq!(found.addresses.len(), before + 1);
                }
                other => return Err(format!("unknown setting `{other}`")),
            }
        }
        Ok(found)
    }
}

/// Remembers a join the command line made: its address and, when one was
/// given, its callsign. A file that cannot be written is a line in the log.
pub fn remember_join(data: &Path, options: &ConnectOptions) {
    let mut kept = Remembered::load(data);
    kept.remember_address(&options.server());
    if options.callsign != DEFAULT_CALLSIGN {
        kept.remember_callsign(&options.callsign);
    }
    if let Err(error) = kept.save(data) {
        log::warn!("Network settings not saved: {error}");
    }
}

/// Remembers a game the command line hosts: its port and name and, when one
/// was given, the callsign. The password is not kept.
pub fn remember_host(data: &Path, options: &HostOptions) {
    let mut kept = Remembered::load(data);
    kept.port = options.port;
    kept.remember_game_name(&options.name);
    if options.callsign != DEFAULT_CALLSIGN {
        kept.remember_callsign(&options.callsign);
    }
    if let Err(error) = kept.save(data) {
        log::warn!("Network settings not saved: {error}");
    }
}

fn name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= MAX_NAME
        && !name.chars().any(char::is_control)
        && name == name.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tore-net-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn what_is_remembered_is_read_back() {
        let dir = scratch("round");
        assert_eq!(Remembered::load(&dir), Remembered::default());
        let mut kept = Remembered::default();
        kept.remember_callsign("Viper");
        kept.remember_game_name("Friday night");
        kept.port = 27_000;
        kept.remember_address("192.168.1.20");
        kept.remember_address("[::1]:4000");
        kept.remember_address("game.example.org:1234");
        kept.save(&dir).unwrap();
        let back = Remembered::load(&dir);
        assert_eq!(back, kept);
        assert_eq!(
            back.addresses,
            ["game.example.org:1234", "[::1]:4000", "192.168.1.20"]
        );
        let text = std::fs::read_to_string(Remembered::path(&dir)).unwrap();
        assert!(text.starts_with("tore-network 1\ncallsign Viper\nport 27000\n"));
        assert!(!text.contains("password"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn addresses_are_most_recent_first_without_repeats_and_capped() {
        let mut kept = Remembered::default();
        for i in 0..12 {
            kept.remember_address(&format!("10.0.0.{i}"));
        }
        assert_eq!(kept.addresses.len(), MAX_ADDRESSES);
        assert_eq!(kept.addresses[0], "10.0.0.11");
        assert_eq!(kept.addresses[MAX_ADDRESSES - 1], "10.0.0.4");
        // A repeat (in any case) moves to the front.
        kept.remember_address("10.0.0.6");
        assert_eq!(kept.addresses[0], "10.0.0.6");
        assert_eq!(kept.addresses.len(), MAX_ADDRESSES);
        kept.remember_address("Game.Example.org");
        kept.remember_address("game.example.ORG");
        assert_eq!(
            kept.addresses
                .iter()
                .filter(|a| a.eq_ignore_ascii_case("game.example.org"))
                .count(),
            1
        );
        assert_eq!(kept.addresses[0], "game.example.ORG");
        // Nonsense is not kept.
        let before = kept.clone();
        for bad in [
            "",
            "   ",
            "host:0",
            "[::1",
            "bad\naddress",
            &"x".repeat(300),
        ] {
            kept.remember_address(bad);
        }
        assert_eq!(kept, before);
        // A callsign or name that would be refused is not kept either.
        kept.remember_callsign("bad\u{e9}");
        kept.remember_game_name(" padded ");
        assert_eq!((kept.callsign, kept.game_name), (None, None));
    }

    #[test]
    fn the_command_line_remembers_what_it_joined_and_hosted() {
        let dir = scratch("cli");
        let join =
            ConnectOptions::new("game.example.org:1234", Some("Viper"), None, Some("pw")).unwrap();
        remember_join(&dir, &join);
        let plain = ConnectOptions::new("192.168.1.20", None, None, None).unwrap();
        remember_join(&dir, &plain);
        let kept = Remembered::load(&dir);
        // The default callsign is not a choice, so Viper stays; the address
        // list is most recent first; the password is nowhere.
        assert_eq!(kept.callsign.as_deref(), Some("Viper"));
        assert_eq!(
            kept.addresses,
            ["192.168.1.20:26900", "game.example.org:1234"]
        );
        let text = std::fs::read_to_string(Remembered::path(&dir)).unwrap();
        assert!(!text.contains("pw"));
        let mission = dir.join("m.txt");
        std::fs::write(
            &mission,
            "tore-mission 1\ntheater UKR\nstart airborne 10000\nseparation-nm 2\nwing friendly 1 F18.PT 2 average\nwing enemy 1 F18.PT 2 average\n",
        )
        .unwrap();
        let host = HostOptions::new(
            &mission,
            Some("27000"),
            Some("Friday night"),
            None,
            Some("Maverick"),
            None,
            Some("secret"),
        )
        .unwrap();
        remember_host(&dir, &host);
        let kept = Remembered::load(&dir);
        assert_eq!(
            (
                kept.port,
                kept.game_name.as_deref(),
                kept.callsign.as_deref()
            ),
            (27_000, Some("Friday night"), Some("Maverick"))
        );
        assert_eq!(kept.addresses.len(), 2);
        assert!(
            !std::fs::read_to_string(Remembered::path(&dir))
                .unwrap()
                .contains("secret")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_broken_file_is_the_defaults() {
        let good = "tore-network 1\ncallsign Viper\nport 26900\naddress a:1\n";
        assert!(Remembered::parse(good).is_ok());
        for bad in [
            "tore-network 2\n",
            "",
            "tore-network 1\nport 0\n",
            "tore-network 1\nport x\n",
            "tore-network 1\nport 5\nport 6\n",
            "tore-network 1\ncallsign \n",
            "tore-network 1\ncallsign Vip\u{e9}r\n",
            "tore-network 1\nfrobnicate 1\n",
            "tore-network 1\npassword hunter2\n",
            "tore-network 1\naddress :5\n",
            "tore-network 1\njunk\n",
        ] {
            assert!(Remembered::parse(bad).is_err(), "{bad:?}");
        }
        let nine: String = (0..9).map(|i| format!("address h{i}\n")).collect();
        assert!(Remembered::parse(&format!("tore-network 1\n{nine}")).is_err());
        let dir = scratch("broken");
        std::fs::write(Remembered::path(&dir), "not a settings file").unwrap();
        assert_eq!(Remembered::load(&dir), Remembered::default());
        let _ = std::fs::remove_dir_all(dir);
    }
}
