//! `tore-bot`: headless bot players for a T.O.R.E server, for the network
//! tests and the LAN smoke test. Each bot is the client session with a
//! scripted pilot (`tore_session::bot`): it joins, takes its slot (`--slot`,
//! or the first free one) with the standard loadout and marks ready, flies
//! straight and level and turns, chases and fires at the other side, returns
//! to the lobby after each mission and readies again, follows the King's
//! mission changes, and after `--seconds` leaves the game (ending its flight
//! first and waiting for its debrief).
//!
//! ```text
//! tore-bot --connect HOST[:PORT] [--data-dir DIR] [--count N] [--callsign NAME]
//!          [--slot PLANE] [--seconds S] [--password TEXT]
//!          [--say SECONDS,RECEIVER,TEXT]... [--quick SECONDS,NUMBER]...
//! ```
//!
//! `--say` makes every bot send the text to the receiver (`all`,
//! `friendlies`, `enemies`, `wing` or `target`) that many seconds after it
//! starts, once it may (a line to anyone but all waits until it flies);
//! `--quick` sends `CHAT.TXT`'s line NUMBER (1 to 12) to the receiver the
//! line names, or all. Both may be given more than once. Every chat line a
//! bot receives is printed with its sender and receiver, the host's words
//! (no one hears you, a refusal) too.
//!
//! It prints one line per join, seating, debrief, lobby change and
//! departure, and each bot's figures every five seconds. It exits 0 when
//! every bot was seated, got a debrief and then left cleanly, or was told the
//! host left the game.

use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tore_net::{CloseReason, DisconnectReason, Entropy, RealClock, bind_udp};
use tore_session::bot::Bot;
use tore_session::client::ended_text;
use tore_session::wire::chat::Receiver;
use tore_session::wire::messages::{Goodbye, LobbyState};
use tore_session::{BuildId, Client, ClientConfig, ClientEvent, ClientPhase};

const USAGE: &str = "usage: tore-bot --connect HOST[:PORT] [--data-dir DIR] [--count N] \
[--callsign NAME] [--slot PLANE] [--seconds S] [--password TEXT] \
[--say SECONDS,RECEIVER,TEXT]... [--quick SECONDS,NUMBER]...";

/// How long a bot waits for its debrief and the disconnect after leaving.
const LEAVE_GRACE: Duration = Duration::from_secs(8);

struct Options {
    connect: SocketAddr,
    data_dir: Option<PathBuf>,
    count: usize,
    callsign: String,
    slot: Option<u32>,
    seconds: u64,
    password: String,
    /// `--say`: when, to whom, what.
    say: Vec<(Duration, Receiver, String)>,
    /// `--quick`: when, which line.
    quick: Vec<(Duration, u8)>,
}

fn receiver(word: &str) -> Result<Receiver, String> {
    Ok(match word.to_ascii_lowercase().as_str() {
        "all" => Receiver::All,
        "friendlies" => Receiver::Friendlies,
        "enemies" => Receiver::Enemies,
        "wing" => Receiver::Wing,
        "target" => Receiver::Target,
        _ => {
            return Err(format!(
                "{word:?} is not a receiver: all, friendlies, enemies, wing or target"
            ));
        }
    })
}

fn seconds(word: &str) -> Result<Duration, String> {
    word.parse::<f64>()
        .ok()
        .filter(|s| s.is_finite() && *s >= 0.)
        .map(Duration::from_secs_f64)
        .ok_or_else(|| format!("{word:?} is not a number of seconds"))
}

/// `--say SECONDS,RECEIVER,TEXT`.
fn say(value: &str) -> Result<(Duration, Receiver, String), String> {
    let mut parts = value.splitn(3, ',');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(at), Some(to), Some(text)) => Ok((seconds(at)?, receiver(to)?, text.to_owned())),
        _ => Err(format!("--say takes SECONDS,RECEIVER,TEXT\n{USAGE}")),
    }
}

/// `--quick SECONDS,NUMBER`.
fn quick(value: &str) -> Result<(Duration, u8), String> {
    let (at, number) = value
        .split_once(',')
        .ok_or_else(|| format!("--quick takes SECONDS,NUMBER\n{USAGE}"))?;
    let number = number
        .parse()
        .ok()
        .filter(|n| (1..=12).contains(n))
        .ok_or("--quick's number is 1 to 12")?;
    Ok((seconds(at)?, number))
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut connect = None;
    let mut options = Options {
        connect: SocketAddr::from(([127, 0, 0, 1], tore_net::DEFAULT_PORT)),
        data_dir: None,
        count: 1,
        callsign: "Bot".into(),
        slot: None,
        seconds: 60,
        password: String::new(),
        say: Vec::new(),
        quick: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--connect" => connect = Some(value()?),
            "--data-dir" => options.data_dir = Some(PathBuf::from(value()?)),
            "--count" => {
                options.count = value()?
                    .parse()
                    .ok()
                    .filter(|n| (1..=30).contains(n))
                    .ok_or("--count is 1 to 30")?;
            }
            "--callsign" => options.callsign = value()?,
            "--slot" => {
                options.slot = Some(value()?.parse().map_err(|_| "--slot is a plane number")?)
            }
            "--seconds" => {
                options.seconds = value()?
                    .parse()
                    .map_err(|_| "--seconds is a whole number")?;
            }
            "--password" => options.password = value()?,
            "--say" => options.say.push(say(&value()?)?),
            "--quick" => options.quick.push(quick(&value()?)?),
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    let connect = connect.ok_or_else(|| format!("--connect is required\n{USAGE}"))?;
    options.connect = reach(&connect)?;
    Ok(options)
}

/// The address to join for what `--connect` said: every address the name
/// gives is tried in turn (IPv4 first, three seconds each) and the first that
/// answers the handshake is used, else the first; one address is used as it
/// is. `HOST`, `HOST:PORT`, `[V6]` and `[V6]:PORT` are read.
fn reach(connect: &str) -> Result<SocketAddr, String> {
    let (host, port) = tore_net::reach::split_address(connect, tore_net::DEFAULT_PORT)
        .map_err(|error| format!("--connect {connect:?} {error}"))?;
    let found = tore_net::reach::resolve(&host, port)
        .map_err(|error| format!("cannot resolve {connect}: {error}"))?;
    let first = *found
        .first()
        .ok_or_else(|| format!("cannot resolve {connect}"))?;
    if found.len() == 1 {
        return Ok(first);
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
        eprintln!("no answer from {address}, trying the next address");
    }
    Ok(first)
}

fn load(data_dir: Option<PathBuf>) -> Result<tore_import::Resources, String> {
    let dir = match data_dir {
        Some(dir) => dir,
        None => tore_import::data_directory().map_err(|error| error.to_string())?,
    };
    tore_import::load_with(&dir, &|resources| tore_import::check_markers(resources))
        .map(|loaded| loaded.resources)
        .map_err(|error| {
            format!(
                "There is no usable import in {} ({error}). Import Fighters Anthology with the game first, or point --data-dir at its data folder.",
                dir.display()
            )
        })
}

fn build() -> BuildId {
    BuildId {
        version: option_env!("TORE_BUILD_VERSION")
            .unwrap_or(env!("CARGO_PKG_VERSION"))
            .to_owned(),
        commit: env!("TORE_BUILD_COMMIT").to_owned(),
        // The same rule as the game and the server: a stamped tag is a
        // release build.
        release: option_env!("TORE_BUILD_VERSION").is_some(),
    }
}

/// One bot and how it is doing.
struct Running {
    name: String,
    socket: UdpSocket,
    bot: Bot,
    seated: bool,
    debrief: bool,
    left_at: Option<Duration>,
    closed: Option<CloseReason>,
    /// The last lobby line printed.
    lobby: Option<String>,
    /// The host said it left the game.
    host_left: bool,
}

/// The lobby in one line: the phase, and each player's slot and marks.
fn lobby_line(lobby: &LobbyState) -> String {
    let players: Vec<String> = lobby
        .players
        .iter()
        .map(|p| {
            let mut text = p.callsign.clone();
            if lobby.king == Some(p.id) {
                text.push_str(" (King)");
            }
            match p.slot {
                Some(plane) => text.push_str(&format!(" plane {plane}")),
                None => text.push_str(" no slot"),
            }
            if p.flying {
                text.push_str(" flying");
            } else if p.ready {
                text.push_str(" ready");
            }
            if p.unable.is_some() {
                text.push_str(" unable");
            }
            text
        })
        .collect();
    format!(
        "{:?}, mission {}: {}",
        lobby.phase,
        lobby.mission,
        players.join("; ")
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse(&args) {
        Ok(options) => options,
        Err(text) => {
            eprintln!("{text}");
            return ExitCode::from(2);
        }
    };
    let resources = match load(options.data_dir.clone()) {
        Ok(resources) => Arc::new(resources),
        Err(text) => {
            eprintln!("{text}");
            return ExitCode::from(2);
        }
    };
    let clock = RealClock::new();
    let local: SocketAddr = if options.connect.is_ipv4() {
        SocketAddr::from(([0, 0, 0, 0], 0))
    } else {
        "[::]:0".parse().expect("an address")
    };
    let lines = resources
        .get(tore_import::selection::CHAT_RESOURCE)
        .map(|bytes| tore_formats::chat::parse(bytes))
        .unwrap_or_default();
    let mut bots = Vec::new();
    for i in 0..options.count {
        let name = if options.count == 1 {
            options.callsign.clone()
        } else {
            let suffix = format!("{}", i + 1);
            let keep = 15usize.saturating_sub(suffix.len());
            format!(
                "{}{suffix}",
                options.callsign.chars().take(keep).collect::<String>()
            )
        };
        let socket = match bind_udp(local) {
            Ok(socket) => socket,
            Err(error) => {
                eprintln!("cannot open a socket: {error}");
                return ExitCode::from(2);
            }
        };
        let config = ClientConfig {
            password: options.password.clone(),
            plane: options.slot.map(|slot| slot + i as u32),
            entropy: Entropy::System,
            ..ClientConfig::new(options.connect, &name, build())
        };
        let client = match Client::connect(config, Arc::clone(&resources), clock.now()) {
            Ok(client) => client,
            Err(error) => {
                eprintln!("{name}: {error}");
                return ExitCode::from(2);
            }
        };
        let mut bot = Bot::new(client);
        for (at, to, text) in &options.say {
            bot.say_at(*at, *to, text);
        }
        for (at, number) in &options.quick {
            bot.quick_at(*at, *number, &lines);
        }
        bots.push(Running {
            name,
            socket,
            bot,
            seated: false,
            debrief: false,
            left_at: None,
            closed: None,
            lobby: None,
            host_left: false,
        });
    }
    let end = Duration::from_secs(options.seconds);
    let mut next_report = Duration::from_secs(5);
    loop {
        let now = clock.now();
        for r in &mut bots {
            if r.closed.is_some() {
                continue;
            }
            let _ = r.bot.client.receive_from(now, &mut r.socket);
            if now >= end && r.left_at.is_none() {
                r.left_at = Some(now);
                if r.bot.client.phase() == ClientPhase::Connecting {
                    r.bot.client.disconnect(now);
                } else {
                    r.bot.client.leave_game(now);
                }
            }
            if r.left_at
                .is_some_and(|at| now.saturating_sub(at) > LEAVE_GRACE)
            {
                r.bot.client.disconnect(now);
            }
            r.bot.update(now);
            let _ = r.bot.client.transmit(&mut r.socket);
            while let Some(event) = r.bot.client.poll_event() {
                match event {
                    ClientEvent::Connected { .. } => println!("{}: joined", r.name),
                    ClientEvent::MissionLoaded => println!("{}: mission loaded", r.name),
                    ClientEvent::ContentRefused { names, reason } => {
                        println!("{}: {reason} ({})", r.name, names.join(", "));
                    }
                    ClientEvent::MissionFailed(text) => println!("{}: {text}", r.name),
                    ClientEvent::SeatRefused(text) => {
                        println!("{}: no plane: {text}; asking for any", r.name);
                    }
                    ClientEvent::Refused { reason, .. } => {
                        println!("{}: refused: {reason}", r.name);
                    }
                    ClientEvent::Lobby => {
                        if let Some(lobby) = r.bot.client.lobby() {
                            let line = lobby_line(lobby);
                            if r.lobby.as_deref() != Some(line.as_str()) {
                                println!("{}: lobby: {line}", r.name);
                                r.lobby = Some(line);
                            }
                        }
                    }
                    ClientEvent::Goodbye(_) => {}
                    ClientEvent::Seated { seat, plane, tick } => {
                        r.seated = true;
                        println!("{}: seat {seat}, plane {plane}, at tick {tick}", r.name);
                    }
                    ClientEvent::Notice(text) => println!("{}: {text}", r.name),
                    ClientEvent::Chat(line) => {
                        let heard = line
                            .sound
                            .as_deref()
                            .map_or(String::new(), |s| format!(" [{s}]"));
                        println!("{}: chat: {}{heard}", r.name, line.log_text());
                    }
                    ClientEvent::Debrief(debrief) => {
                        r.debrief = true;
                        println!(
                            "{}: debrief: {}, {} kills, {} seconds",
                            r.name,
                            if debrief.success {
                                "success"
                            } else {
                                "failure"
                            },
                            debrief.player.kills.iter().sum::<u32>(),
                            debrief.elapsed_seconds
                        );
                    }
                    ClientEvent::MissionEnded(ended) => {
                        println!("{}: {}", r.name, ended_text(&ended));
                    }
                    ClientEvent::Closed(reason) => {
                        println!("{}: {}", r.name, r.bot.client.close_text(&reason));
                        r.host_left = r.bot.client.goodbye() == Some(&Goodbye::HostLeft);
                        r.closed = Some(reason);
                    }
                    ClientEvent::Roster => {}
                    // Stage F phase 2: each slice's bot option reports its
                    // own.
                    ClientEvent::Revival(_)
                    | ClientEvent::Spawned(_)
                    | ClientEvent::Scores(_)
                    | ClientEvent::Results(_)
                    | ClientEvent::Observing(_) => {}
                }
            }
        }
        if now >= next_report {
            next_report += Duration::from_secs(5);
            for r in &mut bots {
                if r.closed.is_some() {
                    continue;
                }
                let s = r.bot.client.stats();
                println!(
                    "{}: round trip {:.0} ms, loss {:.1}%, margin {}, delay {:.0} ms, \
                     corrections {}, mismatches {}, extrapolated {}, repeated {}, \
                     bursts {}, up {} B/s, down {} B/s",
                    r.name,
                    s.round_trip.as_secs_f64() * 1000.,
                    s.loss.unwrap_or(0.) * 100.,
                    s.input_margin.map_or("-".into(), |m| m.to_string()),
                    s.interpolation_delay_ticks * 1000. / 120.,
                    s.corrections,
                    s.mismatches,
                    s.extrapolated,
                    s.inputs_repeated,
                    r.bot.pilot.bursts,
                    s.bytes_up_per_second,
                    s.bytes_down_per_second
                );
            }
        }
        if bots.iter().all(|r| r.closed.is_some()) {
            break;
        }
        let wake = bots
            .iter()
            .filter(|r| r.closed.is_none())
            .map(|r| r.bot.client.next_wake(now))
            .min()
            .unwrap_or(Duration::from_millis(1))
            .clamp(Duration::from_micros(200), Duration::from_millis(2));
        std::thread::sleep(wake);
    }
    let clean = bots.iter().all(|r| {
        r.seated
            && r.debrief
            && (r.host_left
                || matches!(
                    r.closed,
                    Some(CloseReason::Disconnected {
                        reason: DisconnectReason::Left,
                        ..
                    })
                ))
    });
    if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn options_parse_with_their_defaults() {
        let o = parse(&args("--connect 127.0.0.1")).unwrap();
        assert_eq!(o.connect, "127.0.0.1:26900".parse().unwrap());
        assert_eq!((o.count, o.seconds, o.slot), (1, 60, None));
        let o = parse(&args(
            "--connect 127.0.0.1:4000 --count 2 --callsign Viper --slot 3 --seconds 5",
        ))
        .unwrap();
        assert_eq!(o.connect.port(), 4000);
        assert_eq!((o.count, o.seconds, o.slot), (2, 5, Some(3)));
        assert_eq!(o.callsign, "Viper");
        assert!(parse(&args("--count 2")).is_err());
        assert!(parse(&args("--connect 127.0.0.1 --count 0")).is_err());
        assert!(parse(&args("--connect 127.0.0.1 --bogus")).is_err());
    }

    #[test]
    fn chat_options_parse_and_refuse_what_they_cannot_send() {
        let o = parse(&args(
            "--connect 127.0.0.1 --say 5,friendlies,Hello,_there --quick 7.5,12 --say 1,ALL,x",
        ))
        .unwrap();
        assert_eq!(
            o.say[0],
            (
                Duration::from_secs(5),
                Receiver::Friendlies,
                "Hello,_there".to_owned()
            )
        );
        assert_eq!(o.say[1].1, Receiver::All);
        assert_eq!(o.quick, [(Duration::from_millis(7500), 12)]);
        for bad in [
            "--say 5,friends,Hi",
            "--say Hi",
            "--say x,all,Hi",
            "--quick 5,13",
            "--quick 5",
        ] {
            assert!(
                parse(&args(&format!("--connect 127.0.0.1 {bad}"))).is_err(),
                "{bad}"
            );
        }
    }
}
