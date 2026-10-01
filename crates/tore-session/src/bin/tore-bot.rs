//! `tore-bot`: headless bot players for a T.O.R.E server, for the network
//! tests and the LAN smoke test. Each bot is the client session with a
//! scripted pilot (`tore_session::bot`): it joins, takes a plane, flies
//! straight and level and turns, chases and fires at the other side, and
//! after `--seconds` ends the mission and waits for its debrief.
//!
//! ```text
//! tore-bot --connect HOST[:PORT] [--data-dir DIR] [--count N] [--callsign NAME]
//!          [--slot PLANE] [--seconds S] [--password TEXT]
//! ```
//!
//! It prints one line per join, seating, debrief and departure, and each
//! bot's figures every five seconds. It exits 0 when every bot was seated,
//! got its debrief and left cleanly.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tore_net::{CloseReason, DisconnectReason, Entropy, RealClock, bind_udp};
use tore_session::bot::Bot;
use tore_session::client::describe;
use tore_session::{BuildId, Client, ClientConfig, ClientEvent, ClientPhase};

const USAGE: &str = "usage: tore-bot --connect HOST[:PORT] [--data-dir DIR] [--count N] \
[--callsign NAME] [--slot PLANE] [--seconds S] [--password TEXT]";

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
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    let connect = connect.ok_or_else(|| format!("--connect is required\n{USAGE}"))?;
    let with_port = if connect.contains(':') && !connect.ends_with(']') {
        connect.clone()
    } else {
        format!("{connect}:{}", tore_net::DEFAULT_PORT)
    };
    options.connect = with_port
        .to_socket_addrs()
        .map_err(|error| format!("cannot resolve {connect}: {error}"))?
        .next()
        .ok_or_else(|| format!("cannot resolve {connect}"))?;
    Ok(options)
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
        bots.push(Running {
            name,
            socket,
            bot: Bot::new(client),
            seated: false,
            debrief: false,
            left_at: None,
            closed: None,
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
                    r.bot.client.leave(now);
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
                    ClientEvent::ContentRefused { names } => {
                        println!("{}: the game data differs: {}", r.name, names.join(", "));
                    }
                    ClientEvent::MissionFailed(text) => println!("{}: {text}", r.name),
                    ClientEvent::SeatRefused(text) => {
                        println!("{}: no plane: {text}; asking for any", r.name);
                        r.bot.client.ready(None);
                    }
                    ClientEvent::Seated { seat, plane, tick } => {
                        r.seated = true;
                        println!("{}: seat {seat}, plane {plane}, at tick {tick}", r.name);
                    }
                    ClientEvent::Notice(text) => println!("{}: {text}", r.name),
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
                        println!("{}: the mission ended ({:?})", r.name, ended.reason);
                    }
                    ClientEvent::Closed(reason) => {
                        println!("{}: {}", r.name, describe(&reason));
                        r.closed = Some(reason);
                    }
                    ClientEvent::Roster => {}
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
            && matches!(
                r.closed,
                Some(CloseReason::Disconnected {
                    reason: DisconnectReason::Left,
                    ..
                })
            )
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
}
