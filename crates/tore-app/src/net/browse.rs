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
//! - **Join** is [`MasterJoin`]: a socket and a `tore_net::master::join::Joiner`
//!   of its own that run the mapping test and ask the master to introduce the
//!   player, then hand both to the session, which races the host's addresses
//!   from that very socket while the host punches back (slice J2). The
//!   socket is the session's, not the browse's, because the host's punches
//!   open the router's mapping for the address the master saw the Introduce
//!   come from.
//!
//! The wire is `docs/formats/master-protocol.md`, "Browsing" and
//! "Introductions".

use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tore_net::master::browse::{BrowseEvent, Browser, BrowserConfig};
use tore_net::master::candidate::canonical;
use tore_net::master::join::{JoinConfig, JoinEvent, Joiner};
use tore_net::master::local::{MasterLookup, host_candidates, own_address_toward, parse_master};
use tore_net::master::{Build, ListingSummary, PageEntry, packet::MAX_MASTER_DATAGRAM};
use tore_net::{Datagrams, Entropy, Listen, ServerSocket};

/// The list is asked for again this often while the screen is open (the
/// design's 15 seconds).
pub const REFRESH: Duration = Duration::from_secs(15);
/// The selected game's details are asked for this often (5 seconds).
pub const DETAILS: Duration = Duration::from_secs(5);
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
}

/// The browse loop on a socket of its own. Generic over the datagram socket
/// so a test can stand a simulated one in.
pub struct Browse<D: Datagrams = ServerSocket> {
    socket: D,
    browser: Browser,
    host: String,
    port: u16,
    lookup: Option<MasterLookup>,
    masters: Vec<SocketAddr>,
    /// When the list is asked for next; `None` until the master is known.
    next_refresh: Option<Duration>,
    /// When the lookup is tried again after a failure.
    retry_lookup: Option<Duration>,
    details: Option<(u64, Duration)>,
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
            host,
            port,
            lookup: None,
            masters: Vec::new(),
            next_refresh: None,
            retry_lookup: None,
            details: None,
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

    /// The master's addresses, once its name is known (IPv4 first).
    pub fn masters(&self) -> &[SocketAddr] {
        &self.masters
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
            self.browser.receive(now, from, &buf[..len]);
        }
    }

    /// The next news.
    pub fn poll_news(&mut self) -> Option<News> {
        self.events.pop_front()
    }
}

/// A join through the master under way (slice J2's `Joiner` on a socket of
/// its own): the mapping test, then the introduction. The screen turns it
/// every frame with [`MasterJoin::update`]; on [`JoinEvent::Introduced`] the
/// socket and the joiner go to the session ([`MasterJoin::into_parts`]),
/// which races the host's addresses from the same socket.
pub struct MasterJoin {
    socket: ServerSocket,
    joiner: Joiner,
}

impl MasterJoin {
    /// A join of `listing_id` on the master at `masters` (as the browse
    /// found them), from a new dual-stack socket whose port is the one the
    /// master will see and the host will punch.
    pub fn start(masters: &[SocketAddr], listing_id: u64, now: Duration) -> Result<Self, String> {
        if masters.is_empty() {
            return Err("the Internet Lobby's address is not known yet".into());
        }
        let socket = ServerSocket::bind(Listen::Any, 0)
            .map_err(|error| format!("Cannot open a socket to join with: {error}"))?;
        let port = socket.local_addresses().first().map_or(0, SocketAddr::port);
        let mut joiner = Joiner::new(
            JoinConfig {
                build: build(),
                listing_id,
                entropy: Entropy::System,
            },
            now,
        );
        joiner.set_masters(
            masters.to_vec(),
            host_candidates(masters, port, own_address_toward),
            now,
        );
        Ok(Self { socket, joiner })
    }

    /// The loop's turn: the master's datagrams read (anything else before the
    /// race is dropped), the timers, the datagrams sent; what happened.
    pub fn update(&mut self, now: Duration) -> Vec<JoinEvent> {
        let mut buf = [0u8; tore_net::MAX_DATAGRAM + 1];
        {
            let mut routed = self.joiner.over(&mut self.socket, now);
            while let Ok(Some(_)) = routed.recv_datagram(&mut buf) {}
        }
        self.joiner.update(now);
        if let Err(error) = self.joiner.transmit(&mut self.socket) {
            log::info!("Internet Lobby: send failed: {error}");
        }
        std::iter::from_fn(|| self.joiner.poll_event()).collect()
    }

    /// The socket and the joiner, for the session that races the host.
    pub fn into_parts(self) -> (ServerSocket, Joiner) {
        (self.socket, self.joiner)
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
    use std::time::Instant;
    use tore_master::{Config, run::Running};
    use tore_net::master::{MasterPacket, Register};
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
}

#[cfg(test)]
mod tests {
    use super::testing::LoopbackMaster;
    use super::*;
    use std::time::Instant;

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

    #[test]
    fn a_join_through_the_master_is_introduced_to_the_host_from_its_own_socket() {
        let mut master = LoopbackMaster::start();
        let (id, host) = master.list("Friday night", 1, 8, false);
        let masters = [master.main];
        let clock = Instant::now();
        let mut join = MasterJoin::start(&masters, id, clock.elapsed()).unwrap();
        let mut introduced = None;
        let mut mapping = None;
        master.pump(|| {
            for event in join.update(clock.elapsed()) {
                match event {
                    JoinEvent::MappingTested(m) => mapping = Some(m),
                    JoinEvent::Introduced(i) => introduced = Some(i),
                    other => panic!("{other:?}"),
                }
            }
            introduced.is_some()
        });
        let introduced = introduced.expect("an introduction");
        // On this machine the master sees the host at its own address.
        assert_eq!(
            introduced.targets[0].address,
            host.local_addr().unwrap(),
            "{introduced:?}"
        );
        assert!(mapping.is_some());
        // The socket the master saw is the one handed on.
        let (socket, _joiner) = join.into_parts();
        assert_eq!(introduced.seen.port(), socket.local_addresses()[0].port());
    }

    #[test]
    fn a_game_the_master_does_not_list_is_refused_in_its_words_and_a_dead_master_goes_silent() {
        let mut master = LoopbackMaster::start();
        let clock = Instant::now();
        let mut join = MasterJoin::start(&[master.main], 99, clock.elapsed()).unwrap();
        let mut refused = None;
        master.pump(|| {
            for event in join.update(clock.elapsed()) {
                if let JoinEvent::Refused { text, .. } = event {
                    refused = Some(text);
                }
            }
            refused.is_some()
        });
        assert_eq!(refused.as_deref(), Some("That game is no longer listed."));
        // A master nothing listens on: on virtual time, silent after six tries.
        let dead = address("127.0.0.1:9");
        let mut join = MasterJoin::start(&[dead], 1, Duration::ZERO).unwrap();
        let mut silent = false;
        for step in 1..=12 {
            for event in join.update(Duration::from_millis(1_100) * step) {
                silent |= event == JoinEvent::MasterSilent;
            }
        }
        assert!(silent);
        assert!(MasterJoin::start(&[], 1, Duration::ZERO).is_err());
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
