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
//! tore-bot (--connect HOST[:PORT] | --master ADDRESS --listing NAME [--path auto|direct|relay])
//!          [--data-dir DIR] [--count N] [--callsign NAME]
//!          [--slot PLANE] [--seconds S] [--password TEXT] [--capture FILE]
//!          [--token-file FILE]
//!          [--say SECONDS,RECEIVER,TEXT]... [--quick SECONDS,NUMBER]...
//!          [--observe PLANE|none] [--king NAME=VALUE[,NAME=VALUE]...]
//!          [--revive SECONDS] [--away SECONDS,FOR]
//!          [--order SECONDS,NAME]... [--reply SECONDS,KIND]...
//!          [--drop-resource NAME]... [--expect-unable]
//! tore-bot --content-report [--data-dir DIR] [--drop-resource NAME]...
//! ```
//!
//! `--master` and `--listing` join through the Internet Lobby (stage J,
//! slice J2) as the game's Join on a listed game does: the bot asks the
//! master at ADDRESS (port 26901 unless given) for its list, takes the first
//! game named NAME, and each bot runs the mapping test, asks for an
//! introduction from a dual-stack socket of its own and races every address
//! the master gives for the host while the host punches back. It prints
//! each step and the path the join took. `--path auto` (the default) asks
//! the master for the relay (slice J3) when 3 seconds pass without a direct
//! path, or at once when the master's hint says only the relay reaches the
//! game, and joins through the relay if the race has still found nothing
//! when the channel opens. `--path direct` races only. `--path relay` asks
//! for the relay at once and never races, for tests on one machine, where
//! every direct path works. Tests point `--master` at a loopback
//! `tore-master`, never the public one.
//!
//! `--capture FILE` writes each bot's capture (everything the client was
//! given, docs/formats/net-protocol.md, "Captures") to FILE, or FILE-1,
//! FILE-2 and so on with `--count`; the game's `--convert-capture` turns it
//! into a replay.
//!
//! `--token-file FILE` makes each bot keep its rejoin token in FILE (FILE-1,
//! FILE-2 and so on with `--count`; stage K, slice K5) as a game keeps its
//! tokens, and join with the one already in it, if it has not expired: a bot
//! killed in flight and started again with the same file is the same player
//! again and takes back the aircraft kept for it. It prints "rejoining with
//! its token" when it sends one, and the host's welcome back (a Notice).
//!
//! `--say` makes every bot send the text to the receiver (`all`,
//! `friendlies`, `enemies`, `wing` or `target`) that many seconds after it
//! starts, once it may (a line to anyone but all waits until it flies);
//! `--quick` sends `CHAT.TXT`'s line NUMBER (1 to 12) to the receiver the
//! line names, or all. Both may be given more than once. Every chat line a
//! bot receives is printed with its sender and receiver, the host's words
//! (no one hears you, a refusal) too.
//!
//! `--observe` makes every bot an observer (stage F phase 2): it takes no
//! plane and, whenever the mission flies, watches it with the camera on
//! PLANE (or on nothing, `none`), printing when its observer flight starts
//! and ends and, with its figures, the observer frames drawn and the
//! aircraft in the last. An observer succeeds when it watched, drew frames
//! with aircraft in them and left cleanly.
//!
//! `--king` makes a bot that wears the crown (stage F phase 2, slice F2-1: a
//! game it hosts, or a server with `king first-player`) change the King's
//! settings once, by their registry names and values (`mode=pvp,kill-limit=3`,
//! numbers in the wire's units), and start each mission once every player
//! holding a slot is ready. Every bot prints when it wears the crown and the
//! settings whenever they change (the values apart from co-op's defaults).
//!
//! `--revive` makes every bot eject SECONDS after it is first seated and,
//! once the host's Revival says it may, fly again (stage F phase 2's
//! revival): it prints the ejection, each Revival's words, every Spawned
//! plane and the new seating, and succeeds only if it was seated again
//! after ejecting.
//!
//! `--away` makes every bot's game say it is away SECONDS after it is first
//! seated, as a game left at its menu for the King's `idle-ai` seconds does
//! (stage F phase 2, slice F2-A): the AI flies its plane, kept for it, while
//! it watches; FOR seconds later it says it is back and flies on in the same
//! plane. It prints when the AI takes the plane and when it asks for it
//! back, and succeeds only if it was seated again in that plane.
//!
//! `--order` and `--reply` (stage F phase 2, slice F2-R) give a seat command
//! once, SECONDS after the bot is first seated: `--order` is a wing order from
//! a bot that leads its flight (`break-left`, `break-right`, `break-high`,
//! `break-low`, `steady`, `attack-bandits`, `protect-me`, `disengage` or
//! `bug-out`), `--reply` a wingman's reply key (`engaging`, `winchester`,
//! `bingo` or `help`). Every bot prints each radio line it hears ("Bot: radio:
//! Red one: 'Break left'") and each HUD line it reads ("Bot: line: You lead
//! this flight."), so a test can follow an order and its reply from one bot to
//! the other. Put the lead and the wingman in the same wing with `--slot`.
//!
//! `--order SECONDS,sort` (slice G7) is the lead's sort (Alt+A): each
//! wingman, human or AI, is given a bandit of the side's picture by data
//! link. Every bot prints the data link's changes about its flight that the
//! host sent ("Bot: link: plane 0 assigned plane 1 bandit 7 (Sort)") and
//! each change of its own assignment in its readout ("Bot: link: assigned:
//! bandit 7 by plane 0") and of its flightmates' assignments in its readout's
//! marks ("Bot: link: marked: bandit 7 assigned to 3", member numbers from
//! one).
//!
//! Stage L (docs/ARCHITECTURE.md, "Compatibility"): each bot sends the
//! content of its import when it joins, and prints the host's gaps whenever
//! they change ("Bot: gaps: aircraft SU27.PT (Hawk lacks it)", or "gaps:
//! none") and, once for each player whose items differ from the host's, the
//! lobby's Messages line ("Bot: Hawk's game differs from the host's: no
//! Su-27."). A Fighters Anthology build is not reported (John, 2026-10-06).
//! `--drop-resource NAME` (a test aid, any number of times) removes a
//! resource from the bot's loaded import before anything reads it, so a test
//! can play a player whose import lacks an aircraft. `--content-report`
//! prints the import's content (its source, its counts and one line per item
//! with its digest, the same lines as `tore-server --check`) and exits
//! without joining. `--expect-unable` makes a bot succeed only if its import
//! cannot play the mission: it is told why, takes no plane and leaves
//! cleanly (agent decision, for the battery's `net-content-missing`).
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
use tore_net::master::browse::{BrowseEvent, Browser, BrowserConfig};
use tore_net::master::join::{JoinConfig, JoinEvent, Joiner};
use tore_net::master::local::{host_candidates, own_address_toward, parse_master};
use tore_net::master::{Build, Hint, Path, RACE_BEFORE_RELAY};
use tore_net::{
    CloseReason, Datagrams, DisconnectReason, Entropy, Listen, RealClock, ServerSocket, bind_udp,
};
use tore_session::bot::Bot;
use tore_session::client::content::differs_line;
use tore_session::client::rejoin::FileStore;
use tore_session::client::{Race, ended_text};
use tore_session::host::content::{GameContent, gaps_line, report_lines};
use tore_session::settings::{self, Mode, Store};
use tore_session::wire::chat::Receiver;
use tore_session::wire::messages::{Goodbye, LobbyState, Observing, SettingsChange, Subject};
use tore_session::{BuildId, Client, ClientConfig, ClientEvent, ClientPhase};
use tore_world::seats::SeatCommand;

const USAGE: &str = "usage: tore-bot (--connect HOST[:PORT] | --master ADDRESS --listing NAME \
[--path auto|direct|relay]) [--data-dir DIR] [--count N] \
[--callsign NAME] [--slot PLANE] [--seconds S] [--password TEXT] [--capture FILE] \
[--token-file FILE] [--say SECONDS,RECEIVER,TEXT]... [--quick SECONDS,NUMBER]... [--observe PLANE|none] \
[--king NAME=VALUE[,NAME=VALUE]...] [--revive SECONDS] [--away SECONDS,FOR] \
[--order SECONDS,NAME]... [--reply SECONDS,KIND]... \
[--drop-resource NAME]... [--expect-unable]\n       tore-bot --content-report [--data-dir DIR] \
[--drop-resource NAME]...";

/// How long the bot looks for the listing on the master's list.
const FIND_LISTING: Duration = Duration::from_secs(10);

/// How a join through the master may go (`--path`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JoinPath {
    /// Race the host's addresses; the relay when the race finds nothing.
    Auto,
    /// Race only.
    Direct,
    /// The relay at once, never racing.
    Relay,
}

/// Where the bots join: an address, or a listing through the master.
#[derive(Clone, Debug, PartialEq, Eq)]
enum JoinBy {
    Connect(SocketAddr),
    Master {
        master: String,
        listing: String,
        path: JoinPath,
    },
}

/// How long a bot waits for its debrief and the disconnect after leaving.
const LEAVE_GRACE: Duration = Duration::from_secs(8);

struct Options {
    target: JoinBy,
    data_dir: Option<PathBuf>,
    count: usize,
    callsign: String,
    slot: Option<u32>,
    seconds: u64,
    password: String,
    /// `--capture`: where the capture of each bot goes.
    capture: Option<PathBuf>,
    /// `--token-file`: where each bot keeps its rejoin token.
    token_file: Option<PathBuf>,
    /// `--say`: when, to whom, what.
    say: Vec<(Duration, Receiver, String)>,
    /// `--quick`: when, which line.
    quick: Vec<(Duration, u8)>,
    /// `--observe`: watch instead of flying, the camera on this subject.
    observe: Option<Subject>,
    /// `--king`: as the King, these settings, then the start.
    king: Option<Vec<(u8, u32)>>,
    /// `--revive`: eject this long after the first seating and fly again.
    revive: Option<Duration>,
    /// `--away`: away this long after the first seating, back the second
    /// span after the AI took the plane.
    away: Option<(Duration, Duration)>,
    /// `--order` and `--reply`: when after the first seating, and what.
    commands: Vec<(Duration, SeatCommand)>,
    /// `--drop-resource`: names removed from the loaded import.
    drop: Vec<String>,
    /// `--content-report`: print the content and exit.
    content_report: bool,
    /// `--expect-unable`: succeed only as a player who cannot fly the
    /// mission.
    expect_unable: bool,
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

/// `--king NAME=VALUE[,NAME=VALUE]...`: settings by their registry names,
/// each value a word of the setting's or a number in the wire's unit.
fn king(value: &str) -> Result<Vec<(u8, u32)>, String> {
    value
        .split(',')
        .filter(|item| !item.is_empty())
        .map(|item| {
            let (name, value) = item
                .split_once('=')
                .ok_or_else(|| format!("--king takes NAME=VALUE pairs, not {item:?}"))?;
            let setting = settings::by_name(name)
                .filter(|s| s.number != settings::number::PASSWORD)
                .ok_or_else(|| format!("--king: {name:?} is not a setting"))?;
            let value = setting
                .parse(value)
                .ok_or_else(|| format!("--king: {} is {}", setting.name, setting.values_text()))?;
            Ok((setting.number, value))
        })
        .collect()
}

/// The settings apart from co-op's defaults, in words; "co-op's defaults"
/// when there are none.
fn settings_line(values: &[(u8, u32)]) -> String {
    let defaults = Store::defaults(Mode::Coop).lobby_list();
    let changed: Vec<(u8, u32)> = values
        .iter()
        .copied()
        .filter(|value| !defaults.contains(value))
        .collect();
    if changed.is_empty() {
        "co-op's defaults".into()
    } else {
        settings::words(&changed)
    }
}

/// `--away SECONDS,FOR`.
fn away(value: &str) -> Result<(Duration, Duration), String> {
    let (at, span) = value
        .split_once(',')
        .ok_or_else(|| format!("--away takes SECONDS,FOR\n{USAGE}"))?;
    Ok((seconds(at)?, seconds(span)?))
}

/// `--order SECONDS,NAME`: a wing order, given once.
fn order(value: &str) -> Result<(Duration, SeatCommand), String> {
    use tore_sim::ai::wing::{PlayerBreak, PlayerOrder};
    let (at, name) = value
        .split_once(',')
        .ok_or_else(|| format!("--order takes SECONDS,NAME\n{USAGE}"))?;
    let order = match name {
        "break-left" => PlayerOrder::Break(PlayerBreak::Left),
        "break-right" => PlayerOrder::Break(PlayerBreak::Right),
        "break-high" => PlayerOrder::Break(PlayerBreak::High),
        "break-low" => PlayerOrder::Break(PlayerBreak::Low),
        "steady" => PlayerOrder::Break(PlayerBreak::Straight),
        "attack-bandits" => PlayerOrder::AttackOnContact,
        "protect-me" => PlayerOrder::ProtectMe,
        "disengage" => PlayerOrder::Disengage,
        "bug-out" => PlayerOrder::BugOut,
        "sort" => PlayerOrder::Sort,
        other => {
            return Err(format!(
                "--order {other:?} is break-left, break-right, break-high, break-low, steady, \
                 attack-bandits, protect-me, disengage, bug-out or sort"
            ));
        }
    };
    Ok((seconds(at)?, SeatCommand::WingOrder(order)))
}

/// `--reply SECONDS,KIND`: a wingman's reply key, pressed once.
fn reply(value: &str) -> Result<(Duration, SeatCommand), String> {
    use tore_world::world::replies::Reply;
    let (at, kind) = value
        .split_once(',')
        .ok_or_else(|| format!("--reply takes SECONDS,KIND\n{USAGE}"))?;
    let reply = match kind {
        "engaging" => Reply::Engaging,
        "winchester" => Reply::Winchester,
        "bingo" => Reply::BingoFuel,
        "help" => Reply::NeedHelp,
        other => {
            return Err(format!(
                "--reply {other:?} is engaging, winchester, bingo or help"
            ));
        }
    };
    Ok((seconds(at)?, SeatCommand::WingReply(reply)))
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
    let mut master = None;
    let mut listing = None;
    let mut path = JoinPath::Auto;
    let mut options = Options {
        target: JoinBy::Connect(SocketAddr::from(([127, 0, 0, 1], tore_net::DEFAULT_PORT))),
        data_dir: None,
        count: 1,
        callsign: "Bot".into(),
        slot: None,
        seconds: 60,
        password: String::new(),
        capture: None,
        token_file: None,
        say: Vec::new(),
        quick: Vec::new(),
        observe: None,
        king: None,
        revive: None,
        away: None,
        commands: Vec::new(),
        drop: Vec::new(),
        content_report: false,
        expect_unable: false,
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
            "--master" => master = Some(value()?),
            "--listing" => listing = Some(value()?),
            "--path" => {
                path = match value()?.as_str() {
                    "auto" => JoinPath::Auto,
                    "direct" => JoinPath::Direct,
                    "relay" => JoinPath::Relay,
                    other => {
                        return Err(format!("--path is auto, direct or relay, not {other:?}"));
                    }
                }
            }
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
            "--capture" => options.capture = Some(PathBuf::from(value()?)),
            "--token-file" => options.token_file = Some(PathBuf::from(value()?)),
            "--say" => options.say.push(say(&value()?)?),
            "--quick" => options.quick.push(quick(&value()?)?),
            "--observe" => {
                let value = value()?;
                options.observe = Some(match value.as_str() {
                    "none" => Subject::None,
                    plane => Subject::Aircraft(
                        plane
                            .parse()
                            .map_err(|_| "--observe is a plane number or none")?,
                    ),
                });
            }
            "--king" => options.king = Some(king(&value()?)?),
            "--revive" => options.revive = Some(seconds(&value()?)?),
            "--away" => options.away = Some(away(&value()?)?),
            "--order" => options.commands.push(order(&value()?)?),
            "--reply" => options.commands.push(reply(&value()?)?),
            "--drop-resource" => options.drop.push(value()?),
            "--content-report" => options.content_report = true,
            "--expect-unable" => options.expect_unable = true,
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    if options.content_report {
        // A report joins nothing.
        if connect.is_some() || master.is_some() || listing.is_some() {
            return Err(format!("--content-report joins nothing\n{USAGE}"));
        }
        return Ok(options);
    }
    options.target = match (connect, master, listing) {
        (Some(connect), None, None) => JoinBy::Connect(reach(&connect)?),
        (None, Some(master), Some(listing)) => {
            parse_master(&master)?;
            JoinBy::Master {
                master,
                listing,
                path,
            }
        }
        (None, Some(_), None) => return Err(format!("--master needs --listing\n{USAGE}")),
        (None, None, Some(_)) => return Err(format!("--listing needs --master\n{USAGE}")),
        (Some(_), _, _) => {
            return Err(format!(
                "--connect joins by address; --master and --listing join through the \
                 Internet Lobby: give one or the other\n{USAGE}"
            ));
        }
        (None, None, None) => {
            return Err(format!("--connect or --master is required\n{USAGE}"));
        }
    };
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

/// The import in the data folder, less the `--drop-resource` names, and
/// its content (stage L), read with the import report beside the pack.
fn load(
    data_dir: Option<PathBuf>,
    drop: &[String],
) -> Result<(tore_import::Resources, GameContent), String> {
    let dir = match data_dir {
        Some(dir) => dir,
        None => tore_import::data_directory().map_err(|error| error.to_string())?,
    };
    let mut resources = tore_import::load_with(&dir, &|resources| {
        tore_import::check_markers(resources)
    })
    .map(|loaded| loaded.resources)
    .map_err(|error| {
        format!(
            "There is no usable import in {} ({error}). Import Fighters Anthology with the game first, or point --data-dir at its data folder.",
            dir.display()
        )
    })?;
    for name in drop {
        if resources.remove(name).is_none() {
            return Err(format!("--drop-resource: {name} is not in the import"));
        }
    }
    let content = GameContent::read(&dir, &resources);
    Ok((resources, content))
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

/// The master protocol's build: the same three facts as the game's.
fn master_build(build: &BuildId) -> Build {
    Build {
        protocol_version: tore_session::wire::PROTOCOL_VERSION,
        game_version: build.version.clone(),
        game_commit: build.commit.clone(),
        release: build.release,
    }
}

/// The master's addresses, IPv4 first.
fn masters(master: &str) -> Result<Vec<SocketAddr>, String> {
    let (host, port) = parse_master(master)?;
    let found = tore_net::reach::resolve(&host, port)
        .map_err(|error| format!("cannot resolve the master {master}: {error}"))?;
    if found.is_empty() {
        return Err(format!("cannot resolve the master {master}"));
    }
    Ok(found)
}

/// The id of the first game named `name` on the master's list, asking again
/// each second for up to ten.
fn find_listing(
    masters: &[SocketAddr],
    name: &str,
    build: &BuildId,
    clock: &RealClock,
) -> Result<u64, String> {
    let local: SocketAddr = if masters[0].is_ipv4() {
        SocketAddr::from(([0, 0, 0, 0], 0))
    } else {
        "[::]:0".parse().expect("an address")
    };
    let mut socket = bind_udp(local).map_err(|error| format!("cannot open a socket: {error}"))?;
    let mut browser = Browser::new(
        BrowserConfig {
            build: master_build(build),
            other_builds: false,
            full_games: true,
            entropy: Entropy::System,
        },
        masters.to_vec(),
    );
    let start = clock.now();
    browser.refresh(start);
    let mut again = None;
    loop {
        let now = clock.now();
        if now.saturating_sub(start) > FIND_LISTING {
            return Err(format!("no game named {name:?} on the Internet Lobby"));
        }
        if again.is_some_and(|at| now >= at) {
            again = None;
            browser.refresh(now);
        }
        let _ = browser.transmit(&mut socket);
        let _ = browser.receive_from(&mut socket, now);
        browser.update(now);
        while let Some(event) = browser.poll_event() {
            match event {
                BrowseEvent::Refreshed { .. } => {
                    if let Some(game) = browser.games().iter().find(|g| g.name == name) {
                        return Ok(game.listing_id);
                    }
                    again = Some(now + Duration::from_secs(1));
                }
                BrowseEvent::Silent => {
                    return Err("The Internet Lobby does not answer.".into());
                }
                BrowseEvent::Unsupported { text } => return Err(text),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// A path as the bot's lines say it.
fn path_words(path: Path) -> &'static str {
    match path {
        Path::LocalNetwork => "local network",
        Path::ByAddress => "by address",
        Path::MappedPort => "mapped port",
        Path::Ipv6 => "IPv6",
        Path::Punched => "punched",
        Path::Relay => "relay",
    }
}

/// A bot's join through the master, before and during the race.
struct Through {
    socket: ServerSocket,
    joiner: Joiner,
    path: JoinPath,
    /// When the race started.
    raced: Option<Duration>,
    /// The relay was asked for.
    relay_asked: bool,
}

/// One bot and how it is doing.
struct Running {
    name: String,
    /// The socket of a join by address.
    socket: Option<UdpSocket>,
    /// The join through the master.
    through: Option<Through>,
    /// The client's settings, kept until the master's introduction for a
    /// join through it.
    config: ClientConfig,
    /// Where this bot's capture goes (`--capture`).
    capture: Option<PathBuf>,
    /// Where this bot keeps its rejoin token (`--token-file`).
    token_file: Option<PathBuf>,
    /// The bot, once its client has started.
    bot: Option<Bot>,
    /// Why a join through the master ended before the race.
    failed: Option<String>,
    seated: bool,
    debrief: bool,
    left_at: Option<Duration>,
    closed: Option<CloseReason>,
    /// The last lobby line printed.
    lobby: Option<String>,
    /// The host said it left the game.
    host_left: bool,
    /// An observer's flight started.
    observed: bool,
    /// Seatings, the plane of the last, and the ejection printed
    /// (`--revive`).
    seatings: u32,
    last_plane: Option<u32>,
    ejection_told: bool,
    /// The plane the AI flew while it was away, and Back told (`--away`).
    away_plane: Option<u32>,
    back_told: bool,
    /// It wears the crown, and has sent `--king`'s settings.
    crowned: bool,
    settings_sent: bool,
    /// The settings last printed.
    settings: Option<Vec<(u8, u32)>>,
    /// Stage L: the gaps line last printed, the players whose differs line
    /// has been printed, and whether it was told it cannot fly the mission.
    gaps: Option<String>,
    told: std::collections::BTreeSet<u8>,
    unable: bool,
}

impl Running {
    /// Starts the client and the bot with the options' chat lines and
    /// watch.
    fn start(
        &mut self,
        options: &Options,
        resources: &Arc<tore_import::Resources>,
        lines: &[tore_session::wire::chat::QuickMessage],
        now: Duration,
    ) -> Result<(), ExitCode> {
        let client = match Client::connect(self.config.clone(), Arc::clone(resources), now) {
            Ok(client) => client,
            Err(error) => {
                eprintln!("{}: {error}", self.name);
                return Err(ExitCode::from(2));
            }
        };
        let mut client = client;
        if let Some(path) = &self.token_file {
            client.set_token_store(Box::new(FileStore::new(path)));
        }
        if let Some(path) = &self.capture {
            match std::fs::File::create(path) {
                Ok(file) => client.set_capture(Box::new(std::io::BufWriter::new(file))),
                Err(error) => {
                    eprintln!(
                        "{}: cannot write the capture {}: {error}",
                        self.name,
                        path.display()
                    );
                    return Err(ExitCode::from(2));
                }
            }
        }
        let mut bot = Bot::new(client);
        for (at, to, text) in &options.say {
            bot.say_at(*at, *to, text);
        }
        for (at, number) in &options.quick {
            bot.quick_at(*at, *number, lines);
        }
        if let Some(subject) = options.observe {
            bot.watch(subject);
        }
        if let Some(after) = options.revive {
            bot.revive_after(after);
        }
        if let Some((after, span)) = options.away {
            bot.away_after(after, span);
        }
        for (after, command) in &options.commands {
            bot.send_at(*after, *command);
        }
        self.bot = Some(bot);
        Ok(())
    }

    /// A join through the master: the joiner's datagrams and timers, its
    /// steps printed, the client started on the introduction, and the
    /// relay asked for and joined (slice J3).
    fn join_through(
        &mut self,
        now: Duration,
        options: &Options,
        resources: &Arc<tore_import::Resources>,
        lines: &[tore_session::wire::chat::QuickMessage],
    ) -> Result<(), ExitCode> {
        let name = self.name.clone();
        let Some(t) = self.through.as_mut() else {
            return Ok(());
        };
        if self.bot.is_none() {
            // Before the race only the master's datagrams matter.
            let mut buf = [0u8; tore_net::MAX_DATAGRAM + 1];
            let mut routed = t.joiner.over(&mut t.socket, now);
            while let Ok(Some(_)) = routed.recv_datagram(&mut buf) {}
        }
        t.joiner.update(now);
        let path = t.path;
        let mut race = None;
        let mut relay_now = None;
        let mut relayed = None;
        while let Some(event) = t.joiner.poll_event() {
            match event {
                JoinEvent::MappingTested(mapping) => {
                    println!("{name}: mapping test: {mapping:?}");
                }
                JoinEvent::Introduced(introduced) => {
                    let count = introduced.targets.len();
                    let plural = if count == 1 { "" } else { "es" };
                    if path == JoinPath::Relay {
                        println!("{name}: introduced; not racing its {count} address{plural}");
                        relay_now = Some("--path relay");
                    } else {
                        println!("{name}: introduced; trying {count} address{plural}...");
                        if introduced.hint == Hint::RelayNow && path == JoinPath::Auto {
                            relay_now =
                                Some("the Internet Lobby says only the relay reaches this game");
                        }
                        t.raced = Some(now);
                        race = Some(Race {
                            targets: introduced.targets,
                            introduction: introduced.introduction_id,
                        });
                    }
                }
                JoinEvent::Refused { text, .. } | JoinEvent::Unsupported(text) => {
                    println!("{name}: {text}");
                    self.failed = Some(text);
                }
                JoinEvent::MasterSilent => {
                    let text = "The Internet Lobby does not answer.".to_owned();
                    println!("{name}: {text}");
                    self.failed = Some(text);
                }
                JoinEvent::Relayed { address } => {
                    println!("{name}: the relay is open; joining through it");
                    relayed = Some(address);
                }
                JoinEvent::RelayRefused { text, .. } => {
                    println!("{name}: {text}");
                    if path == JoinPath::Relay {
                        self.failed = Some(text);
                    }
                }
                JoinEvent::RelaySilent => {
                    let text = "The Internet Lobby did not answer the relay request.".to_owned();
                    println!("{name}: {text}");
                    if path == JoinPath::Relay {
                        self.failed = Some(text);
                    }
                }
                JoinEvent::RelayClosed(reason) => {
                    println!("{name}: {}", tore_net::master::relay::close_text(reason));
                }
            }
        }
        let race_over = t.raced.is_some_and(|at| now >= at + RACE_BEFORE_RELAY);
        if let Some(race) = race {
            self.config.server = race.targets[0].address;
            self.config.race = Some(race);
            self.start(options, resources, lines, now)?;
        }
        let racing = self.bot.as_ref().is_some_and(|b| !b.client.chosen());
        if let Some(why) = relay_now {
            self.ask_relay(now, why);
        } else if path == JoinPath::Auto && race_over && racing {
            self.ask_relay(now, "no direct path in 3 seconds");
        }
        if let Some(address) = relayed {
            let racing = self.bot.as_ref().is_none_or(|b| !b.client.chosen());
            if racing {
                // The race found nothing: the same join, to the channel's
                // relayed address.
                self.config.server = address;
                self.config.race = None;
                self.start(options, resources, lines, now)?;
            } else if let Some(t) = self.through.as_mut() {
                println!("{name}: the race won after all; closing the relay");
                t.joiner.close_relay();
            }
        }
        if let Some(t) = self.through.as_mut() {
            let _ = t.joiner.transmit(&mut t.socket);
        }
        Ok(())
    }

    /// Asks the master for the relay, once, with a line saying why.
    fn ask_relay(&mut self, now: Duration, why: &str) {
        let name = self.name.clone();
        let Some(t) = self.through.as_mut() else {
            return;
        };
        if t.relay_asked || t.path == JoinPath::Direct {
            return;
        }
        t.relay_asked = true;
        if t.joiner.ask_relay(now) {
            println!("{name}: {why}; asking for the relay...");
        }
    }
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
            } else if p.observing {
                text.push_str(" observing");
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

/// Up to five names, then how many more.
fn names_text(names: &[String]) -> String {
    let shown: Vec<&str> = names.iter().take(5).map(String::as_str).collect();
    match names.len().saturating_sub(5) {
        0 => shown.join(", "),
        more => format!("{} and {more} more", shown.join(", ")),
    }
}

/// Stage L: prints the gaps when they change and, once for each player a gap
/// names, the Messages line about how its items differ from the host's. (A
/// player's Fighters Anthology build is no longer reported: John, 2026-10-06.)
fn content_lines(
    name: &str,
    shown: &mut Option<String>,
    told: &mut std::collections::BTreeSet<u8>,
    client: &Client,
) {
    let (Some(gaps), Some(lobby)) = (client.content_gaps(), client.lobby()) else {
        return;
    };
    let callsign = |id: u8| {
        lobby
            .player(id)
            .map_or_else(|| format!("player {id}"), |p| p.callsign.clone())
    };
    let line = gaps_line(&gaps.gaps, &callsign, 0);
    if shown.as_deref() != Some(line.as_str()) {
        println!("{name}: {line}");
        *shown = Some(line);
    }
    for player in &lobby.players {
        let you = player.id == lobby.you;
        if told.contains(&player.id) {
            continue;
        }
        if let Some(text) = differs_line(player, you, gaps) {
            told.insert(player.id);
            println!("{name}: {text}");
        }
    }
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
    let (resources, content) = match load(options.data_dir.clone(), &options.drop) {
        Ok((resources, content)) => (Arc::new(resources), Arc::new(content)),
        Err(text) => {
            eprintln!("{text}");
            return ExitCode::from(2);
        }
    };
    if options.content_report {
        for line in report_lines(&content) {
            println!("{line}");
        }
        return ExitCode::SUCCESS;
    }
    for name in &options.drop {
        println!("dropped {name} from the import");
    }
    println!(
        "content: {}, {} items",
        content.source.describe(),
        content.content.items().len()
    );
    let clock = RealClock::new();
    let lines = resources
        .get(tore_import::selection::CHAT_RESOURCE)
        .map(|bytes| tore_formats::chat::parse(bytes))
        .unwrap_or_default();
    // A join through the master finds its listing first.
    let found = match &options.target {
        JoinBy::Connect(_) => None,
        JoinBy::Master {
            master, listing, ..
        } => {
            let found = masters(master).and_then(|masters| {
                let id = find_listing(&masters, listing, &build(), &clock)?;
                Ok((masters, id))
            });
            match found {
                Ok((masters, id)) => {
                    println!("found {listing:?} on the Internet Lobby at {master}");
                    Some((masters, id))
                }
                Err(text) => {
                    eprintln!("{text}");
                    return ExitCode::FAILURE;
                }
            }
        }
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
        let server = match &options.target {
            JoinBy::Connect(address) => *address,
            JoinBy::Master { .. } => SocketAddr::from(([0, 0, 0, 0], 0)),
        };
        // A token kept from an earlier run that has not expired is sent in
        // the Challenge answer (stage K, slice K5).
        let token_path = options.token_file.as_ref().map(|base| {
            if options.count == 1 {
                base.clone()
            } else {
                let mut name = base.clone().into_os_string();
                name.push(format!("-{}", i + 1));
                PathBuf::from(name)
            }
        });
        let kept = token_path.as_deref().and_then(FileStore::load);
        if let Some(kept) = &kept {
            println!(
                "{name}: rejoining with its token (session {:016x})",
                kept.session_id
            );
        }
        let config = ClientConfig {
            token: kept.map(|kept| kept.token),
            password: options.password.clone(),
            plane: options.slot.map(|slot| slot + i as u32),
            entropy: Entropy::System,
            auto_ready: options.observe.is_none(),
            content: Some(Arc::clone(&content)),
            ..ClientConfig::new(server, &name, build())
        };
        let mut running = Running {
            name: name.clone(),
            socket: None,
            through: None,
            config,
            capture: options.capture.as_ref().map(|base| {
                if options.count == 1 {
                    base.clone()
                } else {
                    let mut name = base.clone().into_os_string();
                    name.push(format!("-{}", i + 1));
                    PathBuf::from(name)
                }
            }),
            token_file: options.token_file.as_ref().map(|base| {
                if options.count == 1 {
                    base.clone()
                } else {
                    let mut name = base.clone().into_os_string();
                    name.push(format!("-{}", i + 1));
                    PathBuf::from(name)
                }
            }),
            bot: None,
            failed: None,
            seated: false,
            debrief: false,
            left_at: None,
            closed: None,
            lobby: None,
            host_left: false,
            observed: false,
            seatings: 0,
            last_plane: None,
            ejection_told: false,
            away_plane: None,
            back_told: false,
            crowned: false,
            settings_sent: false,
            settings: None,
            gaps: None,
            told: std::collections::BTreeSet::new(),
            unable: false,
        };
        match (&options.target, &found) {
            (JoinBy::Master { path, .. }, Some((masters, listing_id))) => {
                // One dual-stack socket: its IPv4 and IPv6 candidates and
                // its Connect requests all leave from one port.
                let socket = match ServerSocket::bind(Listen::Any, 0) {
                    Ok(socket) => socket,
                    Err(error) => {
                        eprintln!("cannot open a socket: {error}");
                        return ExitCode::from(2);
                    }
                };
                let port = socket.local_addresses().first().map_or(0, SocketAddr::port);
                let now = clock.now();
                let mut joiner = Joiner::new(
                    JoinConfig {
                        build: master_build(&build()),
                        listing_id: *listing_id,
                        entropy: Entropy::System,
                    },
                    now,
                );
                joiner.set_masters(
                    masters.clone(),
                    host_candidates(masters, port, own_address_toward),
                    now,
                );
                println!("{name}: asking the Internet Lobby for an introduction...");
                running.through = Some(Through {
                    socket,
                    joiner,
                    path: *path,
                    raced: None,
                    relay_asked: false,
                });
            }
            _ => {
                let local: SocketAddr = if server.is_ipv4() {
                    SocketAddr::from(([0, 0, 0, 0], 0))
                } else {
                    "[::]:0".parse().expect("an address")
                };
                running.socket = match bind_udp(local) {
                    Ok(socket) => Some(socket),
                    Err(error) => {
                        eprintln!("cannot open a socket: {error}");
                        return ExitCode::from(2);
                    }
                };
                if let Err(code) = running.start(&options, &resources, &lines, clock.now()) {
                    return code;
                }
            }
        }
        bots.push(running);
    }
    let end = Duration::from_secs(options.seconds);
    let mut next_report = Duration::from_secs(5);
    loop {
        let now = clock.now();
        for r in &mut bots {
            if r.closed.is_some() || r.failed.is_some() {
                continue;
            }
            if let Err(code) = r.join_through(now, &options, &resources, &lines) {
                return code;
            }
            if r.failed.is_some() {
                continue;
            }
            let Some(bot) = r.bot.as_mut() else {
                if now >= end {
                    let text = "the join through the Internet Lobby did not finish in time";
                    println!("{}: {text}", r.name);
                    r.failed = Some(text.into());
                }
                continue;
            };
            match (&mut r.through, &mut r.socket) {
                (Some(t), _) => {
                    let _ = bot
                        .client
                        .receive_from(now, &mut t.joiner.over(&mut t.socket, now));
                }
                (None, Some(socket)) => {
                    let _ = bot.client.receive_from(now, socket);
                }
                (None, None) => {}
            }
            if now >= end && r.left_at.is_none() {
                r.left_at = Some(now);
                if bot.client.phase() == ClientPhase::Connecting {
                    bot.client.disconnect(now);
                } else {
                    bot.client.leave_game(now);
                }
            }
            if r.left_at
                .is_some_and(|at| now.saturating_sub(at) > LEAVE_GRACE)
            {
                bot.client.disconnect(now);
            }
            bot.update(now);
            if bot.ejected && !r.ejection_told {
                r.ejection_told = true;
                println!("{}: ejected", r.name);
            }
            if let Some(plane) = bot.client.ai_flies()
                && r.away_plane.is_none()
            {
                r.away_plane = Some(plane);
                println!("{}: away: the AI flies plane {plane}", r.name);
            }
            if bot.came_back && !r.back_told {
                r.back_told = true;
                println!("{}: back at the controls", r.name);
            }
            match (&mut r.through, &mut r.socket) {
                (Some(t), _) => {
                    let _ = bot.client.transmit(&mut t.joiner.over(&mut t.socket, now));
                    let _ = t.joiner.transmit(&mut t.socket);
                }
                (None, Some(socket)) => {
                    let _ = bot.client.transmit(socket);
                }
                (None, None) => {}
            }
            for line in std::mem::take(&mut bot.radio_heard) {
                println!("{}: radio: {line}", r.name);
            }
            for line in std::mem::take(&mut bot.lines_read) {
                println!("{}: line: {line}", r.name);
            }
            for line in std::mem::take(&mut bot.link_heard) {
                println!("{}: link: {line}", r.name);
            }
            while let Some(event) = bot.client.poll_event() {
                match event {
                    ClientEvent::Connected { .. } if r.through.is_some() => println!(
                        "{}: joined through the Internet Lobby, path {}",
                        r.name,
                        path_words(bot.client.path())
                    ),
                    ClientEvent::Connected { .. } => println!("{}: joined", r.name),
                    ClientEvent::MissionLoaded => println!("{}: mission loaded", r.name),
                    ClientEvent::ContentRefused { names, reason } => {
                        r.unable = true;
                        println!("{}: {reason} ({})", r.name, names_text(&names));
                    }
                    ClientEvent::MissionFailed(text) => println!("{}: {text}", r.name),
                    ClientEvent::SeatRefused(text) => {
                        println!("{}: no plane: {text}; asking for any", r.name);
                    }
                    ClientEvent::Refused { reason, .. } => {
                        println!("{}: refused: {reason}", r.name);
                    }
                    ClientEvent::Lobby => {
                        content_lines(&r.name, &mut r.gaps, &mut r.told, &bot.client);
                        if let Some(lobby) = bot.client.lobby() {
                            let line = lobby_line(lobby);
                            if r.lobby.as_deref() != Some(line.as_str()) {
                                println!("{}: lobby: {line}", r.name);
                                r.lobby = Some(line);
                            }
                            if r.settings.as_ref() != Some(&lobby.settings) {
                                println!(
                                    "{}: settings: {}",
                                    r.name,
                                    settings_line(&lobby.settings)
                                );
                                r.settings = Some(lobby.settings.clone());
                            }
                            let king = lobby.is_king();
                            let in_lobby =
                                lobby.phase == tore_session::wire::messages::LobbyPhase::Lobby;
                            let stand = options.king.as_ref().is_some_and(|values| {
                                values.iter().all(|v| lobby.settings.contains(v))
                            });
                            if king != r.crowned {
                                r.crowned = king;
                                if king {
                                    println!("{}: wears the crown", r.name);
                                }
                            }
                            if let Some(values) = &options.king
                                && king
                                && in_lobby
                                && !r.settings_sent
                            {
                                r.settings_sent = true;
                                println!(
                                    "{}: as the King, changing the settings: {}",
                                    r.name,
                                    settings::words(values)
                                );
                                bot.client.change_settings(SettingsChange {
                                    values: values.clone(),
                                    ..SettingsChange::default()
                                });
                            }
                            // The King's start once the settings stand and
                            // everyone holding a slot is ready (not before,
                            // so a change that clears the ready marks does
                            // not meet a start already on its way).
                            bot.start_when_ready = r.settings_sent && stand;
                        }
                    }
                    ClientEvent::Goodbye(_) => {}
                    ClientEvent::Seated { seat, plane, tick } => {
                        r.seated = true;
                        r.seatings += 1;
                        r.last_plane = Some(plane);
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
                        println!("{}: {}", r.name, bot.client.close_text(&reason));
                        // The game connection through the relay ended: the
                        // channel goes too.
                        if let Some(t) = r.through.as_mut() {
                            t.joiner.close_relay();
                            let _ = t.joiner.transmit(&mut t.socket);
                        }
                        r.host_left = bot.client.goodbye() == Some(&Goodbye::HostLeft);
                        r.closed = Some(reason);
                    }
                    ClientEvent::Roster => {}
                    ClientEvent::Observing(observing) => match *observing {
                        Observing::Started(started) => {
                            r.observed = true;
                            println!(
                                "{}: observing from tick {}, {} s behind",
                                r.name, started.tick, started.delay_seconds
                            );
                        }
                        Observing::Ended => println!("{}: observing ended", r.name),
                    },
                    // Stage F phase 2: each slice's bot option reports its
                    // own.
                    ClientEvent::Scores(scores) => println!(
                        "{}: scores: {}",
                        r.name,
                        tore_session::client::scores::summary(&scores)
                    ),
                    ClientEvent::Revival(_) => println!(
                        "{}: revival: {}",
                        r.name,
                        bot.client.revival_prompt().unwrap_or_default()
                    ),
                    ClientEvent::Spawned(spawned) => println!(
                        "{}: spawned plane {} in {:?} wing {}, member {}",
                        r.name,
                        spawned.plane,
                        spawned.wing.side,
                        spawned.wing.index + 1,
                        spawned.member + 1
                    ),
                    ClientEvent::Results(results) => println!(
                        "{}: results: {}",
                        r.name,
                        tore_session::client::results::summary(&results)
                    ),
                }
            }
        }
        if now >= next_report {
            next_report += Duration::from_secs(5);
            for r in &mut bots {
                let Some(bot) = r.bot.as_mut().filter(|_| r.closed.is_none()) else {
                    continue;
                };
                let s = bot.client.stats();
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
                    bot.pilot.bursts,
                    s.bytes_up_per_second,
                    s.bytes_down_per_second
                );
                if r.observed {
                    println!(
                        "{}: watching: frames {}, aircraft {}",
                        r.name, bot.watched, bot.watched_aircraft
                    );
                }
            }
        }
        if bots
            .iter()
            .all(|r| r.closed.is_some() || r.failed.is_some())
        {
            break;
        }
        let wake = bots
            .iter()
            .filter(|r| r.closed.is_none())
            .filter_map(|r| r.bot.as_ref())
            .map(|bot| bot.client.next_wake(now))
            .min()
            .unwrap_or(Duration::from_millis(1))
            .clamp(Duration::from_micros(200), Duration::from_millis(2));
        std::thread::sleep(wake);
    }
    let clean = bots.iter().all(|r| {
        let done = if options.expect_unable {
            r.unable && !r.seated
        } else if options.observe.is_some() {
            r.observed && r.bot.as_ref().is_some_and(|bot| bot.watched > 0)
        } else if options.away.is_some() {
            r.seated
                && r.debrief
                && r.seatings >= 2
                && r.away_plane.is_some()
                && r.last_plane == r.away_plane
        } else if options.revive.is_some() {
            r.seated
                && r.debrief
                && r.bot.as_ref().is_some_and(|bot| bot.ejected)
                && r.seatings >= 2
        } else {
            r.seated && r.debrief
        };
        done && r.failed.is_none()
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
        assert_eq!(
            o.target,
            JoinBy::Connect("127.0.0.1:26900".parse().unwrap())
        );
        assert_eq!((o.count, o.seconds, o.slot), (1, 60, None));
        let o = parse(&args(
            "--connect 127.0.0.1:4000 --count 2 --callsign Viper --slot 3 --seconds 5",
        ))
        .unwrap();
        assert!(matches!(o.target, JoinBy::Connect(a) if a.port() == 4000));
        assert_eq!((o.count, o.seconds, o.slot), (2, 5, Some(3)));
        assert_eq!(o.callsign, "Viper");
        assert!(parse(&args("--count 2")).is_err());
        assert!(parse(&args("--connect 127.0.0.1 --count 0")).is_err());
        assert!(parse(&args("--connect 127.0.0.1 --bogus")).is_err());
    }

    #[test]
    fn content_options_parse_and_a_report_joins_nothing() {
        let o = parse(&args(
            "--connect 127.0.0.1 --drop-resource SU27.PT --drop-resource AIM9X.JT --expect-unable",
        ))
        .unwrap();
        assert_eq!(o.drop, ["SU27.PT", "AIM9X.JT"]);
        assert!(o.expect_unable && !o.content_report);
        let o = parse(&args(
            "--content-report --data-dir /tmp/x --drop-resource SU27.PT",
        ))
        .unwrap();
        assert!(o.content_report);
        assert_eq!(o.drop, ["SU27.PT"]);
        assert!(
            parse(&args("--content-report --connect 127.0.0.1"))
                .err()
                .unwrap()
                .contains("joins nothing")
        );
        assert!(parse(&args("--connect 127.0.0.1 --drop-resource")).is_err());
        assert_eq!(names_text(&["A".into(), "B".into()]), "A, B");
        let many: Vec<String> = (0..8).map(|n| n.to_string()).collect();
        assert_eq!(names_text(&many), "0, 1, 2, 3, 4 and 3 more");
    }

    #[test]
    fn a_join_through_the_master_needs_a_listing_and_no_address() {
        let o = parse(&args("--master 127.0.0.1:26911 --listing Friday")).unwrap();
        assert_eq!(
            o.target,
            JoinBy::Master {
                master: "127.0.0.1:26911".into(),
                listing: "Friday".into(),
                path: JoinPath::Auto,
            }
        );
        let o = parse(&args("--master 127.0.0.1 --listing x --path direct")).unwrap();
        assert!(matches!(
            o.target,
            JoinBy::Master {
                path: JoinPath::Direct,
                ..
            }
        ));
        let o = parse(&args("--master 127.0.0.1 --listing x --path relay")).unwrap();
        assert!(matches!(
            o.target,
            JoinBy::Master {
                path: JoinPath::Relay,
                ..
            }
        ));
        for bad in [
            "--master 127.0.0.1",
            "--listing Friday",
            "--connect 127.0.0.1 --master 127.0.0.1 --listing x",
            "--master 127.0.0.1 --listing x --path sideways",
            "--master host:0 --listing x",
            "",
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn order_and_reply_give_a_seat_command_after_the_first_seating() {
        use tore_sim::ai::wing::{PlayerBreak, PlayerOrder};
        use tore_world::world::replies::Reply;
        let o = parse(&args(
            "--connect 127.0.0.1 --order 12,break-left --order 20.5,attack-bandits \
             --reply 15,winchester --reply 30,help",
        ))
        .unwrap();
        let at = |s: f64| Duration::from_secs_f64(s);
        assert_eq!(
            o.commands,
            [
                (
                    at(12.),
                    SeatCommand::WingOrder(PlayerOrder::Break(PlayerBreak::Left))
                ),
                (
                    at(20.5),
                    SeatCommand::WingOrder(PlayerOrder::AttackOnContact)
                ),
                (at(15.), SeatCommand::WingReply(Reply::Winchester)),
                (at(30.), SeatCommand::WingReply(Reply::NeedHelp)),
            ]
        );
        // The lead's sort (slice G7).
        assert_eq!(
            parse(&args("--connect 127.0.0.1 --order 9,sort"))
                .unwrap()
                .commands,
            [(at(9.), SeatCommand::WingOrder(PlayerOrder::Sort))]
        );
        assert!(
            parse(&args("--connect 127.0.0.1"))
                .unwrap()
                .commands
                .is_empty()
        );
        for bad in [
            "--order 5",
            "--order 5,fly-away",
            "--order x,break-left",
            "--reply 5",
            "--reply 5,hello",
            "--reply -1,engaging",
        ] {
            assert!(
                parse(&args(&format!("--connect 127.0.0.1 {bad}"))).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn king_takes_settings_by_name_and_refuses_what_is_not_one() {
        use tore_session::settings::number;
        let o = parse(&args(
            "--connect 127.0.0.1 --king mode=pvp,kill-limit=3,observer-delay=10,lives=unlimited",
        ))
        .unwrap();
        assert_eq!(
            o.king.unwrap(),
            [
                (number::MODE, 1),
                (number::KILL_LIMIT, 3),
                (number::OBSERVER_DELAY, 10),
                (number::LIVES, settings::UNLIMITED_LIVES),
            ]
        );
        assert_eq!(parse(&args("--connect 127.0.0.1")).unwrap().king, None);
        for bad in [
            "--king mode",
            "--king moods=pvp",
            "--king kill-limit=4",
            "--king password=1",
        ] {
            assert!(
                parse(&args(&format!("--connect 127.0.0.1 {bad}"))).is_err(),
                "{bad}"
            );
        }
        assert_eq!(
            settings_line(&Store::defaults(Mode::Coop).lobby_list()),
            "co-op's defaults"
        );
        let mut pvp = Store::defaults(Mode::Coop);
        pvp.apply(&[(number::MODE, 1), (number::KILL_LIMIT, 3)])
            .unwrap();
        assert!(settings_line(&pvp.lobby_list()).starts_with("mode pvp, "));
    }

    #[test]
    fn revive_takes_seconds() {
        let o = parse(&args("--connect 127.0.0.1 --revive 12.5")).unwrap();
        assert_eq!(o.revive, Some(Duration::from_millis(12_500)));
        assert_eq!(parse(&args("--connect 127.0.0.1")).unwrap().revive, None);
        assert!(parse(&args("--connect 127.0.0.1 --revive")).is_err());
        assert!(parse(&args("--connect 127.0.0.1 --revive soon")).is_err());
    }

    #[test]
    fn away_takes_two_spans_of_seconds() {
        let o = parse(&args("--connect 127.0.0.1 --away 8,5.5")).unwrap();
        assert_eq!(
            o.away,
            Some((Duration::from_secs(8), Duration::from_millis(5_500)))
        );
        assert_eq!(parse(&args("--connect 127.0.0.1")).unwrap().away, None);
        for bad in ["--away", "--away 8", "--away soon,5", "--away 8,-1"] {
            assert!(
                parse(&args(&format!("--connect 127.0.0.1 {bad}"))).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn observe_takes_a_plane_or_none() {
        let o = parse(&args("--connect 127.0.0.1 --observe 3")).unwrap();
        assert_eq!(o.observe, Some(Subject::Aircraft(3)));
        let o = parse(&args("--connect 127.0.0.1 --observe none")).unwrap();
        assert_eq!(o.observe, Some(Subject::None));
        assert_eq!(parse(&args("--connect 127.0.0.1")).unwrap().observe, None);
        assert!(parse(&args("--connect 127.0.0.1 --observe")).is_err());
        assert!(parse(&args("--connect 127.0.0.1 --observe me")).is_err());
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
