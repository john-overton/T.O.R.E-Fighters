//! `--connect HOST[:PORT]` with `--callsign`, `--slot` and `--password`
//! (docs/DEDICATED-SERVER.md, "Joining from the game").
//!
//! Parsing and checking are here and need no network: the host name is looked
//! up only when the game starts the session ([`ConnectOptions::resolve`]).
//! Every default is an agent decision.
use std::net::{SocketAddr, ToSocketAddrs};

/// The port a server listens on unless it is told another.
pub const DEFAULT_PORT: u16 = 26_900;
/// The longest callsign the wire carries.
pub const MAX_CALLSIGN: usize = 15;
/// The callsign a player gets who gives none. The server adds a suffix when
/// it is taken (agent decision).
pub const DEFAULT_CALLSIGN: &str = "Pilot";

/// What the player asked for on the command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectOptions {
    /// The server's host name or address, without a port.
    pub host: String,
    pub port: u16,
    /// 1 to 15 printable ASCII characters.
    pub callsign: String,
    /// The plane to take (`--slot`); `None` takes the first free friendly
    /// plane.
    pub slot: Option<u32>,
    /// Empty when the server has no password.
    pub password: String,
}

impl ConnectOptions {
    /// Checks `server` (`HOST` or `HOST:PORT`; `[::1]:26900` for an IPv6
    /// address with a port), the callsign, the slot and the password.
    pub fn new(
        server: &str,
        callsign: Option<&str>,
        slot: Option<&str>,
        password: Option<&str>,
    ) -> Result<Self, String> {
        let (host, port) = split_server(server)?;
        let callsign = callsign.unwrap_or(DEFAULT_CALLSIGN);
        if let Some(problem) = callsign_problem(callsign) {
            return Err(format!("--callsign {callsign:?}: {problem}"));
        }
        let slot = slot
            .map(|text| {
                text.parse::<u32>()
                    .map_err(|_| format!("--slot needs a plane number, not {text:?}"))
            })
            .transpose()?;
        let password = password.unwrap_or_default();
        if password.len() > 255 || password.chars().any(char::is_control) {
            return Err(
                "--password must be at most 255 characters with no control characters".into(),
            );
        }
        Ok(Self {
            host,
            port,
            callsign: callsign.to_owned(),
            slot,
            password: password.to_owned(),
        })
    }

    /// The server's address. A name is looked up now and the first address
    /// found is used (IPv4 before IPv6 when both exist).
    pub fn resolve(&self) -> Result<SocketAddr, String> {
        let mut found: Vec<SocketAddr> = (self.host.as_str(), self.port)
            .to_socket_addrs()
            .map_err(|error| format!("Cannot find the server {}: {error}", self.host))?
            .collect();
        found.sort_by_key(|address| address.is_ipv6());
        found
            .into_iter()
            .next()
            .ok_or_else(|| format!("Cannot find the server {}", self.host))
    }

    /// The server as the player typed it, for messages and logs.
    pub fn server(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

/// Why `callsign` is not allowed, or `None` when it is.
pub fn callsign_problem(callsign: &str) -> Option<&'static str> {
    if callsign.is_empty() {
        Some("a callsign cannot be empty")
    } else if callsign.chars().count() > MAX_CALLSIGN {
        Some("a callsign is at most 15 characters")
    } else if !callsign.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
        Some("a callsign uses printable ASCII characters only")
    } else if callsign.starts_with(' ') || callsign.ends_with(' ') {
        Some("a callsign cannot start or end with a space")
    } else {
        None
    }
}

/// `HOST`, `HOST:PORT`, `[V6]` or `[V6]:PORT` into a host and a port. A bare
/// IPv6 address (more than one colon, no brackets) has no port.
fn split_server(server: &str) -> Result<(String, u16), String> {
    let usage = "--connect needs HOST or HOST:PORT";
    let server = server.trim();
    if server.is_empty() {
        return Err(usage.into());
    }
    let port = |text: &str| -> Result<u16, String> {
        text.parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or_else(|| format!("--connect port {text:?} is not 1 to 65535"))
    };
    if let Some(rest) = server.strip_prefix('[') {
        let (host, tail) = rest.split_once(']').ok_or(usage)?;
        let port = match tail.strip_prefix(':') {
            Some(text) => port(text)?,
            None if tail.is_empty() => DEFAULT_PORT,
            None => return Err(usage.into()),
        };
        return Ok((host.to_owned(), port));
    }
    match server.matches(':').count() {
        0 => Ok((server.to_owned(), DEFAULT_PORT)),
        1 => {
            let (host, text) = server.split_once(':').expect("one colon");
            if host.is_empty() {
                return Err(usage.into());
            }
            Ok((host.to_owned(), port(text)?))
        }
        _ => Ok((server.to_owned(), DEFAULT_PORT)),
    }
}

/// Why this process may not join a server, or `None`. The retail stall-speed
/// switch changes the simulation for the whole process, so a client with it
/// on would predict a different aircraft than the host flies (the server
/// refuses it as well; John, 2026-09-30).
pub fn process_problem() -> Option<&'static str> {
    tore_sim::flight::retail_stall_speeds().then_some(
        "Cannot join a server with --retail-stall-speeds on: every machine in a multiplayer \
         flight must fly the same aircraft model. Start the game without it.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(server: &str) -> Result<ConnectOptions, String> {
        ConnectOptions::new(server, None, None, None)
    }

    #[test]
    fn the_port_defaults_and_a_given_one_is_checked() {
        let plain = options("192.168.1.20").unwrap();
        assert_eq!((plain.host.as_str(), plain.port), ("192.168.1.20", 26_900));
        let named = options("game.example.org:1234").unwrap();
        assert_eq!(
            (named.host.as_str(), named.port),
            ("game.example.org", 1_234)
        );
        assert_eq!(named.server(), "game.example.org:1234");
        for bad in [
            "",
            "   ",
            ":26900",
            "host:0",
            "host:70000",
            "host:abc",
            "[::1",
        ] {
            assert!(options(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn ipv6_addresses_have_brackets_for_a_port_and_none_without() {
        let bare = options("::1").unwrap();
        assert_eq!((bare.host.as_str(), bare.port), ("::1", 26_900));
        assert_eq!(bare.server(), "[::1]:26900");
        let bracketed = options("[fe80::1]:1234").unwrap();
        assert_eq!(
            (bracketed.host.as_str(), bracketed.port),
            ("fe80::1", 1_234)
        );
        assert_eq!(options("[::1]").unwrap().port, 26_900);
        assert!(options("[::1]x").is_err());
    }

    #[test]
    fn a_literal_address_resolves_without_a_lookup() {
        let address = options("127.0.0.1:4000").unwrap().resolve().unwrap();
        assert_eq!(address, "127.0.0.1:4000".parse().unwrap());
        assert_eq!(
            options("[::1]:4000").unwrap().resolve().unwrap(),
            "[::1]:4000".parse().unwrap()
        );
    }

    #[test]
    fn callsigns_are_one_to_fifteen_printable_ascii_characters() {
        let with = |callsign: &str| ConnectOptions::new("host", Some(callsign), None, None);
        assert_eq!(options("host").unwrap().callsign, "Pilot");
        assert_eq!(with("Viper").unwrap().callsign, "Viper");
        assert_eq!(with("Maverick 1").unwrap().callsign, "Maverick 1");
        assert!(with("123456789012345").is_ok());
        for bad in [
            "",
            "1234567890123456",
            "Vip\u{e9}r",
            "tab\t",
            " Viper",
            "Viper ",
        ] {
            assert!(with(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_slot_is_a_plane_number_and_the_password_is_kept_as_typed() {
        let joined = ConnectOptions::new("host", None, Some("7"), Some("open sesame")).unwrap();
        assert_eq!(
            (joined.slot, joined.password.as_str()),
            (Some(7), "open sesame")
        );
        assert_eq!(options("host").unwrap().slot, None);
        assert!(ConnectOptions::new("host", None, Some("-1"), None).is_err());
        assert!(ConnectOptions::new("host", None, Some("first"), None).is_err());
        assert!(ConnectOptions::new("host", None, None, Some("bad\npassword")).is_err());
    }

    #[test]
    fn the_retail_stall_switch_refuses_a_session() {
        // The switch is process-wide; only the environment form is read here,
        // so this test never changes what other tests see.
        if tore_sim::flight::retail_stall_speeds() {
            assert!(process_problem().is_some());
        } else {
            assert_eq!(process_problem(), None);
        }
    }
}
