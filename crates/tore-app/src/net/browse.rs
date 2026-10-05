//! Looking for games on the Internet Lobby (slice I4): the browse loop the
//! Internet Lobby screen runs while it is open, and `tore-app --browse`.
//!
//! [`Browse`] is the screen's `net::search::Search` for the master. It owns a
//! socket of its own (one UDP port, IPv4 and IPv6, the way a joining player's
//! socket will be in stage J), the master's name looked up on a thread
//! ([`tore_net::master::local::MasterLookup`]) and the `tore_net::master::Browser`
//! state machine. The screen calls [`Browse::update`] every frame with the
//! time and takes the news; nothing here blocks a frame.
//!
//! - The list is asked for as soon as the master's name is known and every
//!   [`REFRESH`] after that (and when [`Browse::refresh`] says so); the
//!   selected game's details every [`DETAILS`].
//! - A master whose name cannot be looked up is tried again after
//!   [`REFRESH`], as a silent one is.
//! - **Join** asks the master to introduce the player to a listed game
//!   ([`Browse::introduce`]) and takes the host's address from the answer.
//!   *Agent decision (slice I4):* a Page and a game's Details carry no
//!   address, so the design's "Join goes straight to the address the master
//!   saw" needs the master's Introduction (kind 17), which slice J2 makes the
//!   master answer. Until then a master drops the request and the join says
//!   so; once J2 is in, the first host address (the one the master saw) is
//!   joined as stage I wants, and stage J5 replaces this with the race.
//!
//! The wire is `docs/formats/master-protocol.md`, "Browsing" and
//! "Introductions".

use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tore_net::master::browse::{BrowseEvent, Browser, BrowserConfig};
use tore_net::master::candidate::canonical;
use tore_net::master::local::{MasterLookup, parse_master};
use tore_net::master::{
    Build, CandidateKind, Introduce, Introduction, IntroductionResult, ListingSummary, MappingType,
    MasterPacket, PageEntry, packet::MAX_MASTER_DATAGRAM,
};
use tore_net::{Datagrams, Entropy, Listen, ServerSocket};

/// The list is asked for again this often while the screen is open (the
/// design's 15 seconds).
pub const REFRESH: Duration = Duration::from_secs(15);
/// The selected game's details are asked for this often (5 seconds).
pub const DETAILS: Duration = Duration::from_secs(5);
/// An introduction request with no answer is sent again after this long...
const INTRODUCE_RETRY: Duration = Duration::from_secs(1);
/// ...this many times in all, then the master counts as silent.
const INTRODUCE_TRIES: u32 = 3;
/// Datagrams one update reads at most.
const MAX_READ: usize = 256;

/// This game's build, as the master filters by it.
pub fn build() -> Build {
    let id = crate::net::session::build_id();
    Build {
        protocol_version: tore_session::wire::PROTOCOL_VERSION,
        game_version: id.version,
        game_commit: id.commit,
        release: id.release,
    }
}

/// What the loop tells the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum News {
    /// The master's name was looked up: how many addresses it gave.
    Resolved(usize),
    /// The master's name could not be looked up (the reason), or its address
    /// is not one. The loop tries again after [`REFRESH`].
    CannotResolve(String),
    /// What the browse client said: games added, changed and dropped, a
    /// finished refresh, a game's details, a silent master.
    Browse(BrowseEvent),
    /// The master introduced the player to a game: its listing and the
    /// address to join.
    Introduced {
        /// The listing asked for.
        listing_id: u64,
        /// Where the host is: the address the master saw, or the first it
        /// gave.
        address: SocketAddr,
    },
    /// An introduction was refused, or not answered: the line for Messages.
    NotIntroduced {
        /// The listing asked for.
        listing_id: u64,
        /// What to tell the player.
        text: String,
    },
}

/// An introduction under way: the request, and the answer's wait.
#[derive(Clone, Debug)]
struct Introducing {
    nonce: u64,
    cookie: u64,
    listing_id: u64,
    sent: Duration,
    tries: u32,
}

/// The browse loop on a socket of its own. Generic over the datagram socket
/// so a test can stand a simulated one in.
pub struct Browse<D: Datagrams = ServerSocket> {
    socket: D,
    browser: Browser,
    build: Build,
    host: String,
    port: u16,
    lookup: Option<MasterLookup>,
    masters: Vec<SocketAddr>,
    /// When the list is asked for next; `None` until the master is known.
    next_refresh: Option<Duration>,
    /// When the lookup is tried again after a failure.
    retry_lookup: Option<Duration>,
    details: Option<(u64, Duration)>,
    introducing: Option<Introducing>,
    events: VecDeque<News>,
}

impl Browse {
    /// A browse of the master at `master` (`HOST` or `HOST:PORT`) on a new
    /// socket. Refused at once when the address is not one.
    pub fn start(master: &str, other_builds: bool, full_games: bool) -> Result<Self, String> {
        let socket = ServerSocket::bind(Listen::Any, 0)
            .map_err(|error| format!("Cannot open a socket for the Internet Lobby: {error}"))?;
        Self::with(socket, master, other_builds, full_games, Entropy::System)
    }
}

impl<D: Datagrams> Browse<D> {
    /// A browse on `socket`, looking the master up now.
    pub fn with(
        socket: D,
        master: &str,
        other_builds: bool,
        full_games: bool,
        entropy: Entropy,
    ) -> Result<Self, String> {
        if !crate::net::settings::master_ok(master) {
            return Err(format!(
                "the master address {master:?} is not a host name or address, with a port if it is not {}",
                tore_net::master::MASTER_PORT
            ));
        }
        let (host, port) = parse_master(master)?;
        let build = build();
        let browser = Browser::new(
            BrowserConfig {
                build: build.clone(),
                other_builds,
                full_games,
                entropy,
            },
            Vec::new(),
        );
        let mut browse = Self {
            socket,
            browser,
            build,
            host,
            port,
            lookup: None,
            masters: Vec::new(),
            next_refresh: None,
            retry_lookup: None,
            details: None,
            introducing: None,
            events: VecDeque::new(),
        };
        browse.look_up();
        Ok(browse)
    }

    fn look_up(&mut self) {
        self.lookup = Some(MasterLookup::start(&self.host, self.port));
        self.retry_lookup = None;
    }

    fn found(&mut self, masters: Vec<SocketAddr>, now: Duration) {
        self.events.push_back(News::Resolved(masters.len()));
        self.masters = masters.iter().copied().map(canonical).collect();
        self.browser.set_masters(masters);
        self.browser.refresh(now);
        self.next_refresh = Some(now + REFRESH);
    }

    /// The master's address text, as the player gave it.
    pub fn master_text(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    /// Changes the filters; the next refresh uses them. Asks again now.
    pub fn set_filters(&mut self, other_builds: bool, full_games: bool, now: Duration) {
        self.browser.set_filters(other_builds, full_games);
        self.refresh(now);
    }

    /// Asks for the list again now.
    pub fn refresh(&mut self, now: Duration) {
        if self.next_refresh.is_some() {
            self.browser.refresh(now);
            self.next_refresh = Some(now + REFRESH);
        } else if self.lookup.is_none() {
            // The name could not be looked up: try that again.
            self.look_up();
        }
    }

    /// The games the last refresh listed, in the master's order.
    pub fn games(&self) -> &[PageEntry] {
        self.browser.games()
    }

    /// The details of `listing_id` are wanted now and every [`DETAILS`]
    /// after, until another game is named or `None` stops them.
    pub fn watch(&mut self, listing_id: Option<u64>, now: Duration) {
        match listing_id {
            None => self.details = None,
            Some(id) => {
                if self.details.is_some_and(|(watching, _)| watching == id) {
                    return;
                }
                self.details = Some((id, now));
                if self.next_refresh.is_some() {
                    self.browser.ask_details(now, id);
                }
            }
        }
    }

    /// Asks the master to introduce the player to `listing_id`. One at a
    /// time: a new one replaces the one under way.
    pub fn introduce(&mut self, listing_id: u64, now: Duration) {
        self.introducing = Some(Introducing {
            nonce: tore_net::reach::random_nonce(),
            cookie: 0,
            listing_id,
            sent: now,
            tries: 0,
        });
        self.send_introduce(now);
    }

    /// Stops an introduction under way.
    pub fn cancel_introduction(&mut self) {
        self.introducing = None;
    }

    /// An introduction is under way.
    #[cfg(test)]
    pub fn introducing(&self) -> bool {
        self.introducing.is_some()
    }

    fn send_introduce(&mut self, now: Duration) {
        let Some(&to) = self.masters.first() else {
            return;
        };
        let Some(asking) = &mut self.introducing else {
            return;
        };
        asking.sent = now;
        asking.tries += 1;
        let request = MasterPacket::Introduce(Introduce {
            nonce: asking.nonce,
            cookie: asking.cookie,
            listing_id: asking.listing_id,
            build: self.build.clone(),
            mapping: MappingType::Unknown,
            candidates: Vec::new(),
        });
        if let Ok(datagram) = request.encode()
            && let Err(error) = self.socket.send_datagram(to, &datagram)
        {
            log::info!("Internet Lobby: send failed: {error}");
        }
    }

    /// The loop's turn: the lookup read, the list asked for when it is time,
    /// datagrams read and sent, retries made. Never blocks.
    pub fn update(&mut self, now: Duration) {
        self.poll_lookup(now);
        if let Some(at) = self.next_refresh
            && now >= at
        {
            self.browser.refresh(now);
            self.next_refresh = Some(now + REFRESH);
        }
        if let Some((id, at)) = self.details
            && now.saturating_sub(at) >= DETAILS
            && self.next_refresh.is_some()
        {
            self.details = Some((id, now));
            self.browser.ask_details(now, id);
        }
        self.read(now);
        self.browser.update(now);
        self.retry_introduce(now);
        while let Some(event) = self.browser.poll_event() {
            self.events.push_back(News::Browse(event));
        }
        if let Err(error) = self.browser.transmit(&mut self.socket) {
            log::info!("Internet Lobby: send failed: {error}");
        }
    }

    fn poll_lookup(&mut self, now: Duration) {
        if let Some(lookup) = &mut self.lookup
            && let Some(answer) = lookup.poll()
        {
            self.lookup = None;
            match answer {
                Ok(addresses) => self.found(addresses, now),
                Err(error) => {
                    self.events.push_back(News::CannotResolve(error));
                    self.retry_lookup = Some(now + REFRESH);
                }
            }
        }
        if let Some(at) = self.retry_lookup
            && now >= at
            && self.lookup.is_none()
        {
            self.look_up();
        }
    }

    fn read(&mut self, now: Duration) {
        let mut buf = [0u8; MAX_MASTER_DATAGRAM + 1];
        for _ in 0..MAX_READ {
            let (len, from) = match self.socket.recv_datagram(&mut buf) {
                Ok(Some(received)) => received,
                Ok(None) => break,
                Err(error) => {
                    log::info!("Internet Lobby: receive failed: {error}");
                    break;
                }
            };
            let datagram = &buf[..len];
            if !self.masters.contains(&canonical(from)) {
                continue;
            }
            match MasterPacket::decode(datagram) {
                Ok(MasterPacket::Challenge(challenge)) => {
                    if let Some(asking) = &mut self.introducing
                        && asking.nonce == challenge.nonce
                        && asking.cookie == 0
                    {
                        asking.cookie = challenge.cookie;
                        asking.tries = 0;
                        self.send_introduce(now);
                    }
                }
                Ok(MasterPacket::Introduction(answer)) => self.introduction(answer),
                _ => {
                    self.browser.receive(now, from, datagram);
                }
            }
        }
    }

    fn introduction(&mut self, answer: Introduction) {
        let Some(asking) = &self.introducing else {
            return;
        };
        if asking.nonce != answer.nonce {
            return;
        }
        let listing_id = asking.listing_id;
        self.introducing = None;
        let refused = |text: &str| News::NotIntroduced {
            listing_id,
            text: text.to_owned(),
        };
        let news = match answer.result {
            IntroductionResult::Introduced => {
                let seen = answer
                    .host_candidates
                    .iter()
                    .find(|c| c.kind == CandidateKind::Seen)
                    .or_else(|| answer.host_candidates.first());
                match seen {
                    Some(candidate) => News::Introduced {
                        listing_id,
                        address: candidate.address,
                    },
                    None => refused("The Internet Lobby gave no address for that game."),
                }
            }
            _ if !answer.text.is_empty() => refused(&answer.text),
            IntroductionResult::NoListing => refused("That game is no longer listed."),
            IntroductionResult::OtherBuild => {
                refused("That game runs another version and cannot be joined.")
            }
            IntroductionResult::Full => refused("That game is full."),
            IntroductionResult::TooMany => {
                refused("Too many joins are under way from here; try again in a moment.")
            }
        };
        self.events.push_back(news);
    }

    fn retry_introduce(&mut self, now: Duration) {
        let Some(asking) = &self.introducing else {
            return;
        };
        if now.saturating_sub(asking.sent) < INTRODUCE_RETRY {
            return;
        }
        if asking.tries >= INTRODUCE_TRIES || self.masters.is_empty() {
            let listing_id = asking.listing_id;
            self.introducing = None;
            self.events.push_back(News::NotIntroduced {
                listing_id,
                text: "The Internet Lobby did not introduce you to that game. Joining through it needs a master that does so; try Refresh, or join by address in Direct Connection.".into(),
            });
        } else {
            self.send_introduce(now);
        }
    }

    /// The next news.
    pub fn poll_news(&mut self) -> Option<News> {
        self.events.pop_front()
    }
}

/// One listed game as `--browse` prints it and the log says it: no address
/// (the master gives none until a join), the facts a player reads.
pub fn entry_line(entry: &PageEntry, summary: Option<&ListingSummary>) -> String {
    let phase = match entry.phase {
        tore_net::packet::DiscoverPhase::Lobby => "lobby",
        tore_net::packet::DiscoverPhase::Flying => "flying",
        tore_net::packet::DiscoverPhase::Closed => "closed",
    };
    let build = match &entry.other_build {
        None => "this build".to_owned(),
        Some(version) => format!("version {version}"),
    };
    let mut line = format!(
        "{}  {}/{} players, {phase}, {}, {}, {build}",
        quote(&entry.name),
        entry.players,
        entry.capacity,
        if entry.password { "password" } else { "open" },
        if entry.full { "full" } else { "not full" },
    );
    if entry.dedicated {
        line += ", dedicated server";
    }
    if entry.relay_likely {
        line += ", relay likely";
    }
    if let Some(summary) = summary {
        line += &format!(
            "; mission {}; king {}; players {}",
            quote(&summary.mission),
            if summary.king.is_empty() {
                "-"
            } else {
                &summary.king
            },
            if summary.callsigns.is_empty() {
                "-".to_owned()
            } else {
                summary.callsigns.join(", ")
            }
        );
    }
    line
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "'"))
}

/// The master `--browse` asks: `--master`, else the one remembered from the
/// Internet Lobby's Options, else the built-in one. Any other session option
/// is refused, since `--browse` joins and hosts nothing.
pub fn browse_master(
    args: &mut crate::net::options::SessionArgs,
    remembered: Option<&str>,
) -> Result<String, String> {
    let other = args.connect.is_some()
        || args.host.is_some()
        || args.callsign.is_some()
        || args.slot.is_some()
        || args.password.is_some()
        || args.port.is_some()
        || args.name.is_some()
        || args.open_planes.is_some()
        || args.list;
    if other {
        return Err("--browse lists games and exits; only --master goes with it".into());
    }
    let master = args
        .master
        .take()
        .or_else(|| remembered.map(str::to_owned))
        .unwrap_or_else(|| tore_net::master::DEFAULT_MASTER.to_owned());
    if !crate::net::settings::master_ok(&master) {
        return Err(format!("--master {master:?} is not a host name or address"));
    }
    Ok(master)
}

/// `tore-app --browse SECONDS [--master ADDRESS]`: lists the Internet
/// Lobby's games for that long, prints each (with its details) and exits, as
/// `--find-games` does for the local network. A master that does not answer
/// is an error.
pub fn browse_games(seconds: f64, master: &str, other_builds: bool) -> io::Result<()> {
    use std::io::Write;
    use std::time::Instant;
    let clock = Instant::now();
    let now = || clock.elapsed();
    let mut browse = Browse::start(master, other_builds, true).map_err(io::Error::other)?;
    let mut out = io::stdout().lock();
    let end = Duration::from_secs_f64(seconds);
    let mut details: Vec<(u64, Option<ListingSummary>)> = Vec::new();
    let mut silent = false;
    let mut unresolved = None;
    let mut asked: Option<u64> = None;
    while now() < end {
        browse.update(now());
        while let Some(news) = browse.poll_news() {
            match news {
                News::Browse(BrowseEvent::Details {
                    listing_id,
                    summary,
                }) => {
                    details.retain(|(id, _)| *id != listing_id);
                    details.push((listing_id, summary));
                    if asked == Some(listing_id) {
                        asked = None;
                    }
                }
                News::Browse(BrowseEvent::Silent) => silent = true,
                News::Browse(BrowseEvent::Refreshed { .. }) => silent = false,
                News::Browse(BrowseEvent::Unsupported { text }) => {
                    writeln!(out, "The master does not take this game: {text}")?;
                }
                News::CannotResolve(error) => unresolved = Some(error),
                News::Resolved(_) => unresolved = None,
                _ => {}
            }
        }
        // One details question at a time, for each game not yet asked about.
        if asked.is_none()
            && let Some(entry) = browse
                .games()
                .iter()
                .find(|g| !details.iter().any(|(id, _)| *id == g.listing_id))
        {
            asked = Some(entry.listing_id);
            browse.watch(Some(entry.listing_id), now());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let games = browse.games().to_vec();
    if games.is_empty() {
        if let Some(error) = unresolved {
            return Err(io::Error::other(format!(
                "cannot find the Internet Lobby at {}: {error}",
                browse.master_text()
            )));
        }
        if silent {
            return Err(io::Error::other(format!(
                "the Internet Lobby at {} does not answer",
                browse.master_text()
            )));
        }
        writeln!(out, "No games listed.")?;
        return Ok(());
    }
    for entry in &games {
        let summary = details
            .iter()
            .find(|(id, _)| *id == entry.listing_id)
            .and_then(|(_, summary)| summary.as_ref());
        writeln!(out, "{}", entry_line(entry, summary))?;
    }
    writeln!(
        out,
        "{} game{} listed.",
        games.len(),
        if games.len() == 1 { "" } else { "s" }
    )?;
    Ok(())
}

/// A master and a game to list on it, on this machine, for the tests of
/// this module and of the Internet Lobby screen.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use std::net::UdpSocket;
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::thread::JoinHandle;
    use std::time::Instant;
    use tore_master::{Config, run::Running};
    use tore_net::master::{Candidate, Challenge, Hint, Introduction, MasterPacket, Register};
    use tore_net::packet::DiscoverPhase;

    /// The real master, `tore_master::run::Running`, on 127.0.0.1 and a
    /// port of its own, turned by hand.
    pub struct LoopbackMaster {
        running: Running,
        /// Where it listens.
        pub main: SocketAddr,
    }

    impl LoopbackMaster {
        pub fn start() -> Self {
            let mut config = Config::defaults(Path::new("."));
            config.listen = Listen::Address(std::net::Ipv4Addr::LOCALHOST.into());
            config.port = 0;
            config.probe_port = 0;
            let (running, _) = Running::bind(config, Entropy::System, false).unwrap();
            let main = running.main_addresses()[0];
            Self { running, main }
        }

        /// The master's address as a player types it.
        pub fn text(&self) -> String {
            self.main.to_string()
        }

        pub fn turn(&mut self) {
            let mut out = Vec::new();
            self.running.turn(&mut out).unwrap();
        }

        /// Turns the master until `done` says so, or five seconds pass.
        pub fn pump(&mut self, mut done: impl FnMut() -> bool) -> bool {
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(5) {
                self.turn();
                if done() {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            false
        }

        /// A host lists a game of this game's build: its Register, the
        /// Challenge, the Register again, the Listed. The socket stays the
        /// host's (the listing lives while it does, and the master is not
        /// turned past the test's 90 seconds).
        pub fn list(
            &mut self,
            name: &str,
            players: u8,
            capacity: u8,
            full: bool,
        ) -> (u64, UdpSocket) {
            let host = UdpSocket::bind("127.0.0.1:0").unwrap();
            host.set_nonblocking(true).unwrap();
            let build = build();
            let summary = ListingSummary {
                protocol_version: build.protocol_version,
                full,
                phase: DiscoverPhase::Lobby,
                players,
                capacity,
                session_id: 7,
                game_version: build.game_version.clone(),
                game_commit: build.game_commit.clone(),
                name: name.to_owned(),
                mission: format!("{name}: KOLA, clear"),
                king: "Iceman".into(),
                callsigns: (0..players)
                    .map(|n| {
                        if n == 0 {
                            "Iceman".into()
                        } else {
                            format!("Pilot{n}")
                        }
                    })
                    .collect(),
                ..ListingSummary::default()
            };
            let register = |cookie| {
                MasterPacket::Register(Register {
                    nonce: 5,
                    cookie,
                    build: build.clone(),
                    dedicated: false,
                    telemetry: false,
                    install_id: 0,
                    platform: 3,
                    candidates: Vec::new(),
                    summary: summary.clone(),
                })
                .encode()
                .unwrap()
            };
            let answer = |host: &UdpSocket, master: &mut Self| {
                let mut got = None;
                assert!(master.pump(|| {
                    let mut buf = [0u8; 1_500];
                    if let Ok((len, _)) = host.recv_from(&mut buf) {
                        got = MasterPacket::decode(&buf[..len]).ok();
                    }
                    got.is_some()
                }));
                got.unwrap()
            };
            host.send_to(&register(0), self.main).unwrap();
            let MasterPacket::Challenge(challenge) = answer(&host, self) else {
                panic!("no challenge");
            };
            host.send_to(&register(challenge.cookie), self.main)
                .unwrap();
            let MasterPacket::Listed(listed) = answer(&host, self) else {
                panic!("not listed");
            };
            (listed.listing_id, host)
        }
    }

    /// A master that answers an Introduce the way a test says (the real
    /// master drops it until slice J2), on a thread of its own.
    pub struct ScriptedMaster {
        /// Where it listens.
        pub address: SocketAddr,
        /// Introduce requests received.
        pub requests: Arc<AtomicUsize>,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    /// What a scripted master answers with.
    pub struct Script {
        pub result: IntroductionResult,
        pub candidates: Vec<Candidate>,
        pub text: &'static str,
        /// Answer the Challenge's repeat; false is a master that never
        /// introduces.
        pub answer: bool,
    }

    impl ScriptedMaster {
        pub fn start(script: Script) -> Self {
            let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
            socket
                .set_read_timeout(Some(Duration::from_millis(10)))
                .unwrap();
            let address = socket.local_addr().unwrap();
            let requests = Arc::new(AtomicUsize::new(0));
            let stop = Arc::new(AtomicBool::new(false));
            let (counted, flag) = (Arc::clone(&requests), Arc::clone(&stop));
            let thread = std::thread::spawn(move || {
                let mut buf = [0u8; 1_500];
                while !flag.load(Ordering::Relaxed) {
                    let Ok((len, from)) = socket.recv_from(&mut buf) else {
                        continue;
                    };
                    let Ok(MasterPacket::Introduce(request)) = MasterPacket::decode(&buf[..len])
                    else {
                        continue;
                    };
                    counted.fetch_add(1, Ordering::Relaxed);
                    let reply = if request.cookie == 0 {
                        MasterPacket::Challenge(Challenge {
                            nonce: request.nonce,
                            cookie: 0xC00C1E,
                        })
                    } else if request.cookie == 0xC00C1E && script.answer {
                        MasterPacket::Introduction(Introduction {
                            nonce: request.nonce,
                            result: script.result,
                            introduction_id: 77,
                            hint: Hint::Race,
                            seen: from,
                            host_mapping: MappingType::Unknown,
                            host_candidates: script.candidates.clone(),
                            text: script.text.to_owned(),
                        })
                    } else {
                        continue;
                    };
                    let _ = socket.send_to(&reply.encode().unwrap(), from);
                }
            });
            Self {
                address,
                requests,
                stop,
                thread: Some(thread),
            }
        }
    }

    impl Drop for ScriptedMaster {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{LoopbackMaster, Script, ScriptedMaster};
    use super::*;
    use std::time::Instant;
    use tore_net::master::{Candidate, CandidateKind};

    fn address(text: &str) -> SocketAddr {
        text.parse().unwrap()
    }

    /// Updates `browse` on the real clock while the master turns, until
    /// `done`, or five seconds pass; the news it gave.
    fn run(
        master: &mut LoopbackMaster,
        browse: &mut Browse,
        mut done: impl FnMut(&Browse, &[News]) -> bool,
    ) -> Vec<News> {
        let clock = Instant::now();
        let mut news = Vec::new();
        master.pump(|| {
            browse.update(clock.elapsed());
            while let Some(item) = browse.poll_news() {
                news.push(item);
            }
            done(browse, &news)
        });
        news
    }

    #[test]
    fn a_browse_lists_a_masters_games_and_asks_for_the_details_of_one() {
        let mut master = LoopbackMaster::start();
        let (id, _host) = master.list("Friday night", 3, 8, false);
        let mut browse = Browse::start(&master.text(), false, false).unwrap();
        let news = run(&mut master, &mut browse, |_, news| {
            news.iter()
                .any(|n| matches!(n, News::Browse(BrowseEvent::Refreshed { .. })))
        });
        assert!(news.contains(&News::Resolved(1)));
        assert!(
            news.iter().any(|n| matches!(n, News::Browse(BrowseEvent::Added(e)) if e.listing_id == id && e.name == "Friday night")),
            "{news:?}"
        );
        assert!(
            news.iter().any(|n| matches!(
                n,
                News::Browse(BrowseEvent::Refreshed {
                    matching: 1,
                    shown: 1
                })
            )),
            "{news:?}"
        );
        assert_eq!(browse.games().len(), 1);
        assert_eq!(browse.games()[0].players, 3);
        // The details of the game come when it is watched.
        browse.watch(Some(id), Duration::ZERO);
        let news = run(&mut master, &mut browse, |_, news| {
            news.iter()
                .any(|n| matches!(n, News::Browse(BrowseEvent::Details { .. })))
        });
        let summary = news
            .iter()
            .find_map(|n| match n {
                News::Browse(BrowseEvent::Details {
                    summary: Some(summary),
                    ..
                }) => Some(summary),
                _ => None,
            })
            .expect("the details");
        assert_eq!(summary.king, "Iceman");
        assert_eq!(summary.callsigns.len(), 3);
        assert!(summary.mission.contains("KOLA"));
    }

    #[test]
    fn the_filters_reach_the_master_and_a_filter_change_asks_again_at_once() {
        let mut master = LoopbackMaster::start();
        let (_a, _host_a) = master.list("Room to spare", 1, 8, false);
        let (_b, _host_b) = master.list("Packed", 8, 8, true);
        let mut browse = Browse::start(&master.text(), false, false).unwrap();
        run(&mut master, &mut browse, |b, _| b.games().len() == 1);
        assert_eq!(browse.games().len(), 1);
        assert_eq!(browse.games()[0].name, "Room to spare");
        browse.set_filters(false, true, Duration::from_secs(1));
        run(&mut master, &mut browse, |b, _| b.games().len() == 2);
        assert_eq!(browse.games().len(), 2);
    }

    #[test]
    fn a_master_that_cannot_be_found_is_told_once_and_looked_up_again_later() {
        // The reserved .invalid name never resolves (RFC 6761).
        let mut browse = Browse::start("master.invalid:26901", false, false).unwrap();
        let clock = Instant::now();
        let mut news = Vec::new();
        while clock.elapsed() < Duration::from_secs(10) {
            browse.update(clock.elapsed());
            while let Some(item) = browse.poll_news() {
                news.push(item);
            }
            if !news.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            matches!(news.as_slice(), [News::CannotResolve(_)]),
            "{news:?}"
        );
        assert!(browse.games().is_empty());
        // Nothing is looked up again until the refresh interval has passed;
        // then the lookup starts again.
        let at = clock.elapsed();
        browse.update(at + REFRESH / 2);
        assert!(browse.lookup.is_none());
        browse.update(at + REFRESH + Duration::from_secs(1));
        assert!(browse.lookup.is_some());
        // Refresh asks for the lookup at once when it is not running.
        browse.lookup = None;
        browse.retry_lookup = None;
        browse.refresh(at);
        assert!(browse.lookup.is_some());
    }

    #[test]
    fn a_master_address_that_is_not_one_is_refused_at_once() {
        for bad in ["", "two words", "host:0", "[::1"] {
            assert!(Browse::start(bad, false, false).is_err(), "{bad:?}");
        }
    }

    fn seen_and_local() -> Vec<Candidate> {
        vec![
            Candidate::new(CandidateKind::Seen, address("203.0.113.9:40000")),
            Candidate::new(CandidateKind::Local, address("192.168.1.20:26900")),
        ]
    }

    /// A browse on a scripted master, and the news after asking for an
    /// introduction to listing 5.
    fn introduce(script: Script) -> (Vec<News>, usize) {
        let master = ScriptedMaster::start(script);
        let mut browse = Browse::start(&master.address.to_string(), false, false).unwrap();
        let clock = Instant::now();
        browse.introduce(5, clock.elapsed());
        // Held until the master is known: sent once the lookup is read.
        assert!(browse.introducing());
        let mut news = Vec::new();
        while clock.elapsed() < Duration::from_secs(8) {
            browse.update(clock.elapsed());
            while let Some(item) = browse.poll_news() {
                if matches!(item, News::Introduced { .. } | News::NotIntroduced { .. }) {
                    news.push(item);
                }
            }
            if !news.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let requests = master.requests.load(std::sync::atomic::Ordering::Relaxed);
        (news, requests)
    }

    #[test]
    fn an_introduction_gives_the_address_the_master_saw_for_the_host() {
        let (news, requests) = introduce(Script {
            result: IntroductionResult::Introduced,
            candidates: seen_and_local(),
            text: "",
            answer: true,
        });
        assert_eq!(
            news,
            [News::Introduced {
                listing_id: 5,
                address: address("203.0.113.9:40000")
            }]
        );
        // The request, then its repeat with the Challenge's cookie.
        assert_eq!(requests, 2);
        // With no Seen candidate the first one is used.
        let (news, _) = introduce(Script {
            result: IntroductionResult::Introduced,
            candidates: seen_and_local()[1..].to_vec(),
            text: "",
            answer: true,
        });
        assert_eq!(
            news,
            [News::Introduced {
                listing_id: 5,
                address: address("192.168.1.20:26900")
            }]
        );
        // No address at all is a plain refusal, not a join to nowhere.
        let (news, _) = introduce(Script {
            result: IntroductionResult::Introduced,
            candidates: Vec::new(),
            text: "",
            answer: true,
        });
        assert!(
            matches!(news.as_slice(), [News::NotIntroduced { listing_id: 5, text }] if text.contains("no address"))
        );
    }

    #[test]
    fn a_refused_introduction_says_why_in_plain_words() {
        for (result, text, wanted) in [
            (IntroductionResult::NoListing, "", "no longer listed"),
            (IntroductionResult::OtherBuild, "", "another version"),
            (IntroductionResult::Full, "", "full"),
            (IntroductionResult::TooMany, "", "Too many"),
            // The master's own words win.
            (
                IntroductionResult::Full,
                "Try the other server.",
                "Try the other server.",
            ),
        ] {
            let (news, _) = introduce(Script {
                result,
                candidates: Vec::new(),
                text,
                answer: true,
            });
            assert!(
                matches!(news.as_slice(), [News::NotIntroduced { listing_id: 5, text }] if text.contains(wanted)),
                "{result:?}: {news:?}"
            );
        }
    }

    #[test]
    fn a_master_that_never_introduces_is_given_up_on_after_three_tries() {
        let master = ScriptedMaster::start(Script {
            result: IntroductionResult::Introduced,
            candidates: Vec::new(),
            text: "",
            answer: false,
        });
        let mut browse = Browse::start(&master.address.to_string(), false, false).unwrap();
        // The lookup of a literal address ends at once; wait for it.
        let clock = Instant::now();
        while browse.masters.is_empty() && clock.elapsed() < Duration::from_secs(5) {
            browse.update(clock.elapsed());
            std::thread::sleep(Duration::from_millis(2));
        }
        // From here the time is the test's: one second a step.
        let mut now = Duration::from_secs(100);
        browse.introduce(5, now);
        let mut given_up = None;
        for _ in 0..8 {
            now += Duration::from_millis(1_100);
            browse.update(now);
            while let Some(item) = browse.poll_news() {
                if let News::NotIntroduced { text, .. } = item {
                    given_up = Some(text);
                }
            }
            if given_up.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        let text = given_up.expect("the join gives up");
        assert!(text.contains("did not introduce"), "{text}");
        assert!(!browse.introducing());
        // The Challenge was answered, then no more than two repeats.
        let requests = master.requests.load(std::sync::atomic::Ordering::Relaxed);
        assert!((3..=6).contains(&requests), "{requests} requests");
    }

    #[test]
    fn a_datagram_that_is_not_from_the_master_is_ignored() {
        let master = ScriptedMaster::start(Script {
            result: IntroductionResult::Introduced,
            candidates: seen_and_local(),
            text: "",
            answer: false,
        });
        let mut browse = Browse::start(&master.address.to_string(), false, false).unwrap();
        let clock = Instant::now();
        while browse.masters.is_empty() {
            browse.update(clock.elapsed());
            std::thread::sleep(Duration::from_millis(2));
        }
        browse.introduce(5, clock.elapsed());
        let nonce = browse.introducing.as_ref().unwrap().nonce;
        let forged = MasterPacket::Introduction(Introduction {
            nonce,
            result: IntroductionResult::Introduced,
            introduction_id: 1,
            hint: tore_net::master::Hint::Race,
            seen: address("198.51.100.1:1"),
            host_mapping: MappingType::Unknown,
            host_candidates: seen_and_local(),
            text: String::new(),
        })
        .encode()
        .unwrap();
        // Sent to the browse's own socket by someone else.
        let target = browse
            .socket
            .local_addresses()
            .into_iter()
            .find(|a| a.is_ipv4());
        let stranger = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        if let Some(target) = target {
            let to = address(&format!("127.0.0.1:{}", target.port()));
            stranger.send_to(&forged, to).unwrap();
        }
        std::thread::sleep(Duration::from_millis(50));
        browse.update(clock.elapsed());
        assert!(
            browse
                .poll_news()
                .is_none_or(|n| !matches!(n, News::Introduced { .. }))
        );
        assert!(browse.introducing(), "still waiting for the real master");
    }

    #[test]
    fn a_listed_game_reads_as_one_line_with_its_details() {
        let entry = PageEntry {
            listing_id: 9,
            password: true,
            full: false,
            dedicated: true,
            other_build: None,
            relay_likely: true,
            phase: tore_net::packet::DiscoverPhase::Flying,
            players: 2,
            capacity: 8,
            platform: 3,
            name: "Say \"hi\"".into(),
        };
        let bare = entry_line(&entry, None);
        assert_eq!(
            bare,
            "\"Say 'hi'\"  2/8 players, flying, password, not full, this build, dedicated server, relay likely"
        );
        let summary = ListingSummary {
            mission: "UKR, clear".into(),
            king: "Maverick".into(),
            callsigns: vec!["Maverick".into(), "Viper".into()],
            ..ListingSummary::default()
        };
        let full = entry_line(&entry, Some(&summary));
        assert!(full.ends_with("; mission \"UKR, clear\"; king Maverick; players Maverick, Viper"));
        let other = PageEntry {
            other_build: Some("0.1.2".into()),
            ..entry
        };
        assert!(entry_line(&other, None).contains("version 0.1.2"));
    }

    #[test]
    fn browse_takes_only_a_master_and_falls_back_to_the_remembered_one() {
        use crate::net::options::SessionArgs;
        let args = |master: Option<&str>| SessionArgs {
            master: master.map(str::to_owned),
            ..SessionArgs::default()
        };
        assert_eq!(
            browse_master(&mut args(Some("m.example.org:1")), Some("kept:2")).unwrap(),
            "m.example.org:1"
        );
        assert_eq!(
            browse_master(&mut args(None), Some("kept:2")).unwrap(),
            "kept:2"
        );
        assert_eq!(
            browse_master(&mut args(None), None).unwrap(),
            tore_net::master::DEFAULT_MASTER
        );
        assert!(browse_master(&mut args(Some("two words")), None).is_err());
        let mut with_port = args(None);
        with_port.port = Some("1".into());
        assert!(
            browse_master(&mut with_port, None)
                .unwrap_err()
                .contains("only --master")
        );
        let mut listing = args(None);
        listing.list = true;
        assert!(browse_master(&mut listing, None).is_err());
    }
}
