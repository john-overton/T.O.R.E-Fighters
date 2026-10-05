//! `--connect HOST[:PORT]` with `--callsign`, `--slot` and `--password`
//! (docs/DEDICATED-SERVER.md, "Joining from the game"), and `--host
//! MISSION_FILE` with `--port`, `--name`, `--open-planes`, `--list`,
//! `--master` and the same three ("Hosting from the game").
//!
//! Parsing and checking are here and need no network: the host name is looked
//! up only when the game starts the session ([`ConnectOptions::resolve`]).
//! Every default is an agent decision.
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tore_session::OpenPlanes;

use crate::net::hosting::Listing;
use tore_world::mission::MissionSpec;

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

    /// Every address the server's name gives, IPv4 first, looked up now.
    pub fn addresses(&self) -> Result<Vec<SocketAddr>, String> {
        let found = tore_net::reach::resolve(&self.host, self.port)
            .map_err(|error| format!("Cannot find the server {}: {error}", self.host))?;
        if found.is_empty() {
            return Err(format!("Cannot find the server {}", self.host));
        }
        Ok(found)
    }

    /// The server's address. A name is looked up now; when it gives several
    /// addresses each is tried in turn (IPv4 first, three seconds each) and
    /// the first that answers the handshake is used, else the first one, so
    /// the join reports the silence itself. One address is used without a
    /// test. This waits, so the game's screens use `lookup::Lookup` on a
    /// thread instead; the command line is the caller that may wait.
    pub fn resolve(&self) -> Result<SocketAddr, String> {
        let found = self.addresses()?;
        if found.len() == 1 {
            return Ok(found[0]);
        }
        for address in &found {
            let reach = tore_net::reach::probe(
                *address,
                tore_session::wire::PROTOCOL_VERSION,
                tore_net::reach::PROBE_TIMEOUT,
                &|| false,
            );
            if matches!(
                reach,
                Ok(tore_net::reach::Reach::Answered | tore_net::reach::Reach::Refused(_))
            ) {
                return Ok(*address);
            }
            log::info!("Network: no answer from {address}, trying the next address");
        }
        Ok(found[0])
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

/// The longest game name, in characters: the dedicated server's limit.
pub const MAX_NAME: usize = 60;
/// Plane numbers run from 0 to this, less one: six wings of five.
pub const MAX_PLANES: u32 = 30;

/// What the player asked for to host a game from the command line.
#[derive(Clone, Debug, PartialEq)]
pub struct HostOptions {
    /// The mission file, read and parsed.
    pub mission: PathBuf,
    pub spec: MissionSpec,
    /// The UDP port to listen on, on every address.
    pub port: u16,
    /// The game's name, shown to joining players.
    pub name: String,
    /// Which planes players may take.
    pub open_planes: OpenPlanes,
    /// The hosting player's callsign and plane.
    pub callsign: String,
    pub slot: Option<u32>,
    /// The password joining players must give, `None` for none.
    pub password: Option<String>,
    /// Listed on the Internet Lobby (`--list`), `None` for a game that never
    /// talks to the master.
    pub listing: Option<Listing>,
}

impl HostOptions {
    /// Reads and checks the mission file and the options, as the dedicated
    /// server reads its own: a file that cannot be read or a line it does not
    /// take is refused with the file's name and the line.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mission: &Path,
        port: Option<&str>,
        name: Option<&str>,
        open_planes: Option<&str>,
        callsign: Option<&str>,
        slot: Option<&str>,
        password: Option<&str>,
    ) -> Result<Self, String> {
        let joining = ConnectOptions::new("localhost", callsign, slot, password)?;
        let port = match port {
            Some(text) => text
                .parse::<u16>()
                .ok()
                .filter(|port| *port != 0)
                .ok_or_else(|| format!("--port must be a number from 1 to 65535, not {text:?}"))?,
            None => DEFAULT_PORT,
        };
        let name = match name {
            Some(name) => {
                if name.is_empty()
                    || name.chars().count() > MAX_NAME
                    || name.chars().any(char::is_control)
                {
                    return Err(format!(
                        "--name must be 1 to {MAX_NAME} printable characters"
                    ));
                }
                name.to_owned()
            }
            None => format!("{}'s game", joining.callsign),
        };
        let open_planes = match open_planes {
            Some(text) => parse_open_planes(text)?,
            None => OpenPlanes::Friendly,
        };
        let spec = load_mission(mission)?;
        Ok(Self {
            mission: mission.to_owned(),
            spec,
            port,
            name,
            open_planes,
            callsign: joining.callsign,
            slot: joining.slot,
            password: Some(joining.password).filter(|p| !p.is_empty()),
            listing: None,
        })
    }
}

/// Reads and parses a mission file; an error names the file and its line.
pub fn load_mission(path: &Path) -> Result<MissionSpec, String> {
    let text = tore_import::files::read(path)
        .map_err(|error| format!("Cannot read the mission file {}: {error}", path.display()))?;
    MissionSpec::from_text(&text).map_err(|error| format!("{}: {error}", path.display()))
}

/// `friendly`, `all` or plane numbers separated by commas or spaces, as the
/// dedicated server's `open-planes` setting takes them.
pub fn parse_open_planes(value: &str) -> Result<OpenPlanes, String> {
    match value.trim() {
        "friendly" => Ok(OpenPlanes::Friendly),
        "all" => Ok(OpenPlanes::All),
        list => {
            let mut planes = Vec::new();
            for word in list.split([',', ' ', '\t']).filter(|w| !w.is_empty()) {
                let plane = word.parse::<u32>().map_err(|_| {
                    format!(
                        "--open-planes must be `friendly`, `all` or plane numbers, not `{word}`"
                    )
                })?;
                if plane >= MAX_PLANES {
                    return Err(format!(
                        "--open-planes plane {plane} is out of range: a mission has planes 0 to {}",
                        MAX_PLANES - 1
                    ));
                }
                if planes.contains(&plane) {
                    return Err(format!("--open-planes lists plane {plane} twice"));
                }
                planes.push(plane);
            }
            if planes.is_empty() {
                return Err("--open-planes needs `friendly`, `all` or plane numbers".into());
            }
            Ok(OpenPlanes::List(planes))
        }
    }
}

/// What the command line asked the game to do with a session.
#[derive(Clone, Debug, PartialEq)]
pub enum Session {
    /// `--connect`: join a server.
    Join(ConnectOptions),
    /// `--host`: host a game and fly in it.
    Host(Box<HostOptions>),
}

/// The session options as the command line gave them, before they are
/// checked together.
#[derive(Clone, Debug, Default)]
pub struct SessionArgs {
    pub connect: Option<String>,
    pub host: Option<PathBuf>,
    pub callsign: Option<String>,
    pub slot: Option<String>,
    pub password: Option<String>,
    pub port: Option<String>,
    pub name: Option<String>,
    pub open_planes: Option<String>,
    /// `--list`: list the hosted game on the Internet Lobby.
    pub list: bool,
    /// `--master ADDRESS`: the master server `--list` lists on.
    pub master: Option<String>,
}

impl SessionArgs {
    /// The session the options ask for, checked. `other_mode` is true when
    /// the command line also asks for a capture, probe, recording or the
    /// replay viewer, which a session cannot combine with.
    pub fn session(self, other_mode: bool) -> Result<Option<Session>, String> {
        let Self {
            connect,
            host,
            callsign,
            slot,
            password,
            port,
            name,
            open_planes,
            list,
            master,
        } = self;
        if master.is_some() && !list {
            return Err("--master goes with --list".into());
        }
        if let Some(master) = &master {
            tore_net::master::local::parse_master(master)
                .map_err(|error| format!("--master {error}"))?;
        }
        let hosting_only = port.is_some() || name.is_some() || open_planes.is_some() || list;
        let session = match (connect, host) {
            (Some(_), Some(_)) => {
                return Err("--connect and --host cannot be used together".into());
            }
            (Some(server), None) => {
                if hosting_only {
                    return Err("--port, --name, --open-planes and --list go with --host".into());
                }
                if other_mode {
                    return Err("--connect joins a server and cannot combine with captures, probes, recordings or the replay viewer".into());
                }
                Session::Join(ConnectOptions::new(
                    &server,
                    callsign.as_deref(),
                    slot.as_deref(),
                    password.as_deref(),
                )?)
            }
            (None, Some(mission)) => {
                if other_mode {
                    return Err("--host hosts a game and cannot combine with captures, probes, recordings or the replay viewer".into());
                }
                let mut options = HostOptions::new(
                    &mission,
                    port.as_deref(),
                    name.as_deref(),
                    open_planes.as_deref(),
                    callsign.as_deref(),
                    slot.as_deref(),
                    password.as_deref(),
                )?;
                options.listing = list.then(|| Listing::listed(master.as_deref()));
                Session::Host(Box::new(options))
            }
            (None, None) => {
                if hosting_only {
                    return Err("--port, --name, --open-planes and --list go with --host".into());
                }
                if callsign.is_some() || slot.is_some() || password.is_some() {
                    return Err(
                        "--callsign, --slot and --password go with --connect or --host".into(),
                    );
                }
                return Ok(None);
            }
        };
        if let Some(problem) = process_problem() {
            return Err(problem.into());
        }
        Ok(Some(session))
    }
}

/// The port `--find-games` searches: `--port`, else the default. Any other
/// session option is refused, since `--find-games` joins and hosts nothing.
pub fn find_games_port(args: &mut SessionArgs) -> Result<u16, String> {
    only_port(args, "--find-games", "lists games and exits")
}

/// The port `--map-port` forwards (slice J4b): `--port`, else the default.
/// Any other session option is refused.
pub fn map_port_port(args: &mut SessionArgs) -> Result<u16, String> {
    only_port(args, "--map-port", "forwards the port and exits")
}

/// The port a command that joins and hosts nothing uses: only `--port` goes
/// with `flag`.
fn only_port(args: &mut SessionArgs, flag: &str, what: &str) -> Result<u16, String> {
    let other = args.connect.is_some()
        || args.host.is_some()
        || args.callsign.is_some()
        || args.slot.is_some()
        || args.password.is_some()
        || args.name.is_some()
        || args.open_planes.is_some()
        || args.list
        || args.master.is_some();
    if other {
        return Err(format!("{flag} {what}; only --port goes with it"));
    }
    match args.port.take() {
        Some(text) => text
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or_else(|| format!("--port must be a number from 1 to 65535, not {text:?}")),
        None => Ok(DEFAULT_PORT),
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

/// `HOST`, `HOST:PORT`, `[V6]` or `[V6]:PORT` into a host and a port (the
/// reading is `tore_net::reach::split_address`, shared with the bot and the
/// screen). A bare IPv6 address has no port.
pub fn split_server(server: &str) -> Result<(String, u16), String> {
    tore_net::reach::split_address(server, DEFAULT_PORT)
        .map_err(|error| format!("--connect {error}"))
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

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tore-host-options-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const MISSION: &str = "tore-mission 1\ntheater UKR\nstart airborne 10000\nseparation-nm 2\nwing friendly 1 F18.PT 2 average\nwing enemy 1 F18.PT 2 average\n";

    #[test]
    fn hosting_reads_the_mission_file_and_takes_the_servers_defaults() {
        let dir = scratch("good");
        let path = dir.join("duel.txt");
        std::fs::write(&path, MISSION).unwrap();
        let host = HostOptions::new(&path, None, None, None, Some("Viper"), None, None).unwrap();
        assert_eq!(host.spec.theater, "UKR");
        assert_eq!((host.port, host.name.as_str()), (26_900, "Viper's game"));
        assert_eq!(host.open_planes, OpenPlanes::Friendly);
        assert_eq!((host.slot, host.password.as_deref()), (None, None));
        let host = HostOptions::new(
            &path,
            Some("27000"),
            Some("Friday night"),
            Some("0, 1 3"),
            None,
            Some("1"),
            Some("open sesame"),
        )
        .unwrap();
        assert_eq!((host.port, host.name.as_str()), (27_000, "Friday night"));
        assert_eq!(host.open_planes, OpenPlanes::List(vec![0, 1, 3]));
        assert_eq!(host.callsign, "Pilot");
        assert_eq!(
            (host.slot, host.password.as_deref()),
            (Some(1), Some("open sesame"))
        );
        assert_eq!(
            HostOptions::new(&path, None, None, Some("all"), None, None, None)
                .unwrap()
                .open_planes,
            OpenPlanes::All
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hosting_refuses_a_bad_mission_file_and_bad_options_plainly() {
        let dir = scratch("bad");
        let path = dir.join("mission.txt");
        std::fs::write(&path, "tore-mission 1\ntheater UKR\ntheater MOON\n").unwrap();
        let host =
            |port, name, planes| HostOptions::new(&path, port, name, planes, None, None, None);
        let error = host(None, None, None).unwrap_err();
        assert!(error.contains("mission.txt: line 3"), "{error}");
        let missing = HostOptions::new(&dir.join("none.txt"), None, None, None, None, None, None);
        assert!(
            missing
                .unwrap_err()
                .contains("Cannot read the mission file")
        );
        std::fs::write(&path, MISSION).unwrap();
        for (port, name, planes, needle) in [
            (Some("0"), None, None, "1 to 65535"),
            (Some("x"), None, None, "1 to 65535"),
            (None, Some(""), None, "printable"),
            (None, Some("tab\there"), None, "printable"),
            (None, None, Some("some"), "`some`"),
            (None, None, Some("30"), "out of range"),
            (None, None, Some("2,2"), "twice"),
            (None, None, Some(" , "), "plane numbers"),
        ] {
            let error = host(port, name, planes).unwrap_err();
            assert!(error.contains(needle), "{needle} in {error}");
        }
        let long = "x".repeat(MAX_NAME + 1);
        assert!(host(None, Some(&long), None).is_err());
        assert!(HostOptions::new(&path, None, None, None, Some(" bad"), None, None).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_session_options_go_together_and_with_nothing_else() {
        let dir = scratch("args");
        let path = dir.join("mission.txt");
        std::fs::write(&path, MISSION).unwrap();
        let args = |connect: Option<&str>, host: bool| SessionArgs {
            connect: connect.map(str::to_owned),
            host: host.then(|| path.clone()),
            ..SessionArgs::default()
        };
        assert_eq!(SessionArgs::default().session(false), Ok(None));
        let error = |args: SessionArgs, other| args.session(other).unwrap_err();
        assert!(error(args(Some("h"), true), false).contains("cannot be used together"));
        assert!(error(args(Some("h"), false), true).contains("--connect joins a server"));
        assert!(error(args(None, true), true).contains("--host hosts a game"));
        let lonely = SessionArgs {
            callsign: Some("Viper".into()),
            ..SessionArgs::default()
        };
        assert!(error(lonely, false).contains("go with --connect or --host"));
        let port = |connect, host| SessionArgs {
            port: Some("27000".into()),
            ..args(connect, host)
        };
        assert!(error(port(Some("h"), false), false).contains("go with --host"));
        assert!(error(port(None, false), false).contains("go with --host"));
        let listed = |connect, host, master: Option<&str>| SessionArgs {
            list: true,
            master: master.map(str::to_owned),
            ..args(connect, host)
        };
        assert!(error(listed(Some("h"), false, None), false).contains("--list go with --host"));
        assert!(error(listed(None, false, None), false).contains("--list go with --host"));
        let unlisted_master = SessionArgs {
            master: Some("127.0.0.1:26911".into()),
            ..args(None, true)
        };
        assert!(error(unlisted_master, false).contains("--master goes with --list"));
        assert!(error(listed(None, true, Some("host:0")), false).contains("--master"));
        if process_problem().is_none() {
            assert!(matches!(
                args(Some("h:4000"), false).session(false),
                Ok(Some(Session::Join(options))) if options.port == 4000
            ));
            assert!(matches!(
                port(None, true).session(false),
                Ok(Some(Session::Host(options))) if options.port == 27_000 && options.listing.is_none()
            ));
            let Ok(Some(Session::Host(options))) =
                listed(None, true, Some("127.0.0.1:26911")).session(false)
            else {
                panic!("a listed host");
            };
            assert_eq!(
                options.listing,
                Some(Listing {
                    master: "127.0.0.1:26911".into(),
                    listed: true,
                    install_id: None,
                })
            );
            let Ok(Some(Session::Host(options))) = listed(None, true, None).session(false) else {
                panic!("a listed host");
            };
            assert_eq!(
                options.listing.unwrap().master,
                tore_net::master::DEFAULT_MASTER
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn find_games_takes_a_port_and_nothing_else() {
        let mut args = SessionArgs::default();
        assert_eq!(find_games_port(&mut args), Ok(26_900));
        args.port = Some("27000".into());
        assert_eq!(find_games_port(&mut args), Ok(27_000));
        assert!(args.port.is_none());
        for bad in ["0", "x", "70000"] {
            args.port = Some(bad.into());
            assert!(find_games_port(&mut args).is_err(), "{bad}");
        }
        for args in [
            SessionArgs {
                connect: Some("h".into()),
                ..SessionArgs::default()
            },
            SessionArgs {
                callsign: Some("Viper".into()),
                ..SessionArgs::default()
            },
            SessionArgs {
                name: Some("n".into()),
                ..SessionArgs::default()
            },
            SessionArgs {
                list: true,
                ..SessionArgs::default()
            },
        ] {
            let mut args = args;
            assert!(
                find_games_port(&mut args)
                    .unwrap_err()
                    .contains("only --port")
            );
        }
    }

    #[test]
    fn map_port_takes_a_port_and_nothing_else() {
        let mut args = SessionArgs::default();
        assert_eq!(map_port_port(&mut args), Ok(26_900));
        args.port = Some("27001".into());
        assert_eq!(map_port_port(&mut args), Ok(27_001));
        args.port = Some("0".into());
        assert!(map_port_port(&mut args).is_err());
        let mut args = SessionArgs {
            host: Some("m.txt".into()),
            ..SessionArgs::default()
        };
        assert_eq!(
            map_port_port(&mut args).unwrap_err(),
            "--map-port forwards the port and exits; only --port goes with it"
        );
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
