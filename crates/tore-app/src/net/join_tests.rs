//! Slice J5's acceptance: the game joins a game listed on the Internet Lobby
//! through the master, directly and through the relay, as the Internet Lobby
//! screen and its session do it. In real time on 127.0.0.1: the real master
//! (`tore_master::run::Running`, turned by the test), a hosting game's thread
//! listed on it (its rendezvous answers the master's Meets and Relay opens),
//! its King over the in-process link, the Internet Lobby screen on the
//! synthetic kit, and the game's own `NetSession` started from the screen's
//! join, on the synthetic import (no retail data).
//!
//! On one machine every direct path works, so the relay is tested with the
//! relay only (`TORE_JOIN_PATH=relay`, here set on the screen). No test
//! contacts the built-in master.
//!
//! ```sh
//! cargo test --locked -p tore-app join_tests -- --nocapture
//! ```

use super::*;
use crate::internet_screen::app::session_join;
use crate::internet_screen::{InternetScreen, Outcome};
use crate::net::hosting::{HostSetup, HostThread, Listing, Report, config};
use crate::net::options::HostOptions;
use std::net::Ipv4Addr;
use std::thread;
use std::time::Instant;
use tore_master::{Config, run::Running};
use tore_net::master::ListingState;
use tore_net::master::join::JoinConfig;
use tore_net::{LINK_ADDRESS, Listen};
use tore_session::bot::Bot;
use tore_session::{ClientPhase, OpenPlanes};
use tore_world::mission::{Skill, Start};
use tore_world::test_support::resources::{THEATER, resources};

/// How often the games take their turn.
const FRAME: Duration = Duration::from_millis(10);
/// The hosted game's name, as the Internet Lobby lists it.
const NAME: &str = "Host's game";

fn spec() -> MissionSpec {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 4;
    spec.wings[3].count = 2;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 5;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

fn import() -> Arc<BTreeMap<String, Vec<u8>>> {
    Arc::new(resources())
}

/// The real master on 127.0.0.1, on ports of its own, with its relay on or
/// off, turned by the test.
struct Master {
    running: Running,
    main: SocketAddr,
}

impl Master {
    fn start(relay: bool) -> Self {
        let mut config = Config::defaults(std::path::Path::new("."));
        config.listen = Listen::Address(Ipv4Addr::LOCALHOST.into());
        config.port = 0;
        config.probe_port = 0;
        config.status_interval = 0;
        config.settings.relay.on = relay;
        let (running, _) = Running::bind(config, Entropy::System, false).expect("a master");
        let main = running.main_addresses()[0];
        Self { running, main }
    }

    fn turn(&mut self) {
        let mut out = Vec::new();
        let _ = self.running.turn(&mut out);
    }
}

/// The master, a hosting game listed on it with its King, and a player at
/// the Internet Lobby screen, all turned once a frame.
struct Game {
    master: Master,
    host: Option<HostThread>,
    king: Bot,
    king_link: LinkEnd,
    king_clock: RealClock,
    reports: Vec<Report>,
    screen: InternetScreen,
    /// The join the screen asked for, once it did.
    request: Option<crate::internet_screen::InternetJoin>,
    session: Option<NetSession>,
    /// The session's events, and its notices (the Messages lines) in order.
    seen: Vec<ClientEvent>,
    notices: Vec<String>,
    /// The player's game is stalled: its session takes no turn.
    stalled: bool,
    data: PathBuf,
}

impl Game {
    fn start(name: &str, path: JoinPath, relay: bool) -> Self {
        let master = Master::start(relay);
        let options = HostOptions {
            mission: "duel.txt".into(),
            spec: spec(),
            port: 0,
            name: NAME.into(),
            open_planes: OpenPlanes::Friendly,
            password: None,
            callsign: "Host".into(),
            slot: None,
            listing: None,
        };
        let (host, king_link) = HostThread::start_listed(
            HostSetup {
                spec: spec(),
                resources: import(),
                config: config(&options),
                listen: Listen::Address(Ipv4Addr::LOCALHOST.into()),
                port: 0,
            },
            Some(Listing {
                master: master.main.to_string(),
                listed: true,
                install_id: None,
            }),
        )
        .expect("the host starts");
        let king_clock = RealClock::new();
        let king_config = ClientConfig {
            entropy: Entropy::System,
            ..ClientConfig::new(LINK_ADDRESS, "Host", build_id())
        };
        let client = Client::connect(king_config, import(), king_clock.now()).expect("a client");
        let mut king = Bot::new(client);
        king.start_when_ready = true;
        let screen = InternetScreen::for_join_tests(&master.main.to_string(), "Viper", path);
        let data = std::env::temp_dir().join(format!("tore-join-{name}-{}", std::process::id()));
        Self {
            master,
            host: Some(host),
            king,
            king_link,
            king_clock,
            reports: Vec::new(),
            screen,
            request: None,
            session: None,
            seen: Vec::new(),
            notices: Vec::new(),
            stalled: false,
            data,
        }
    }

    fn frame(&mut self) {
        self.master.turn();
        if let Some(host) = &mut self.host {
            self.reports.extend(host.poll());
        }
        let now = self.king_clock.now();
        let _ = self.king.client.receive_from(now, &mut self.king_link);
        self.king.update(now);
        let _ = self.king.client.transmit(&mut self.king_link);
        while self.king.client.poll_event().is_some() {}
        if let Some(session) = &mut self.session
            && !self.stalled
        {
            session.pump(&Controls::neutral(Default::default()));
            for event in session.take_events() {
                if let ClientEvent::Notice(text) = &event {
                    self.notices.push(text.clone());
                }
                self.seen.push(event);
            }
        }
        thread::sleep(FRAME);
    }

    fn run_until(&mut self, limit: Duration, mut done: impl FnMut(&mut Self) -> bool) -> bool {
        let until = Instant::now() + limit;
        while Instant::now() < until {
            self.frame();
            if done(self) {
                return true;
            }
        }
        false
    }

    fn run_for(&mut self, time: Duration) {
        self.run_until(time, |_| false);
    }

    fn session(&self) -> &NetSession {
        self.session.as_ref().expect("a session")
    }

    fn session_mut(&mut self) -> &mut NetSession {
        self.session.as_mut().expect("a session")
    }

    /// The host is listed, and the screen lists its game.
    fn listed(&mut self) {
        assert!(
            self.run_until(Duration::from_secs(10), |g| g
                .reports
                .iter()
                .any(|r| matches!(r, Report::Listing(ListingState::Listed { .. })))),
            "the host is listed: {:?}",
            self.reports
        );
        assert!(
            self.run_until(Duration::from_secs(10), |g| {
                g.screen.update(false);
                !g.screen.listed_names().is_empty()
            }),
            "the screen lists the game: {:?}",
            self.screen.message_lines()
        );
        assert_eq!(self.screen.listed_names(), [NAME]);
    }

    /// Join on the listed game: the master's introduction, then the session
    /// the game starts from it, as `App::internet_join` starts it.
    fn join(&mut self) {
        self.listed();
        assert_eq!(self.screen.join_row(0), Outcome::None);
        assert!(
            self.run_until(Duration::from_secs(10), |g| {
                if let Outcome::Join(request) = g.screen.update(false) {
                    g.request = Some(*request);
                }
                g.request.is_some()
            }),
            "introduced: {:?}",
            self.screen.message_lines()
        );
        let request = self.request.as_ref().expect("a join");
        let through = self.screen.take_through().expect("the introduced socket");
        let join = session_join(request, through);
        let session = NetSession::start(join, import(), &self.data, None).expect("a session");
        self.session = Some(session);
    }

    /// Takes a plane and readies, as the lobby screen does: the King's game
    /// starts the mission and the player flies.
    fn fly(&mut self) {
        // The mission loads first (the King flies already, alone).
        assert!(
            self.run_until(Duration::from_secs(10), |g| g.session().client.phase()
                == ClientPhase::Lobby),
            "the mission loaded: {:?}",
            self.seen
        );
        self.session_mut().client.take_any_slot();
        assert!(
            self.run_until(Duration::from_secs(10), |g| g
                .session()
                .client
                .lobby()
                .and_then(|l| l.me())
                .is_some_and(|m| m.slot.is_some())),
            "a slot: {:?}",
            self.session().client.lobby()
        );
        self.session_mut().client.set_ready(true);
        assert!(
            self.run_until(Duration::from_secs(20), |g| {
                g.session().client.seat().is_some()
                    && g.session().client.phase() == ClientPhase::Flying
            }),
            "seated and flying: {:?}, {:?}",
            self.session().client.phase(),
            self.seen
        );
        self.run_for(Duration::from_secs(2));
        assert_eq!(self.session().client.phase(), ClientPhase::Flying);
    }

    /// The player leaves as the game's End does: a disconnect and a flush.
    fn leave(&mut self) {
        let session = self.session_mut();
        let now = session.now();
        session.client.disconnect(now);
        session.flush();
    }

    fn closed(&self) -> Option<&CloseReason> {
        self.seen.iter().find_map(|e| match e {
            ClientEvent::Closed(reason) => Some(reason),
            _ => None,
        })
    }

    fn joiner(&self) -> &Joiner {
        match &self.session().socket {
            Transport::Internet(t) => &t.joiner,
            _ => panic!("a join through the master"),
        }
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data);
    }
}

/// `lines` holds `expected` in that order (other lines may come between).
fn in_order(lines: &[String], expected: &[&str]) -> bool {
    let mut at = 0;
    for line in lines {
        if at < expected.len() && line == expected[at] {
            at += 1;
        }
    }
    at == expected.len()
}

/// Join on a listed game reaches its host directly: the race's first
/// answer, the punched path on this machine, a plane and a flight; the
/// Messages lines in order; the path in the net log; no relay asked for.
#[test]
fn a_listed_game_is_joined_directly_through_the_masters_introduction_and_flown() {
    let mut game = Game::start("direct", JoinPath::Auto, true);
    game.join();
    let request = game.request.clone().expect("a join");
    assert_eq!(request.path, JoinPath::Auto);
    assert!(!request.relay_now);
    assert!(
        game.run_until(Duration::from_secs(5), |g| g
            .session()
            .connected_after()
            .is_some()),
        "connected: {:?}",
        game.seen
    );
    // On this machine the master sees the host at its own address: the race
    // reads it as a punched hole.
    assert_eq!(game.session().client.path(), JoinedPath::Punched);
    assert!(game.session().client.chosen());
    game.fly();
    let screen = game.screen.message_lines();
    assert!(
        in_order(
            &screen,
            &[
                "Asking the Internet Lobby to introduce you to 'Host's game'...",
                "Trying 1 address for 'Host's game'...",
            ]
        ),
        "{screen:?}"
    );
    assert_eq!(
        game.notices.first().map(String::as_str),
        Some("Connected directly (punched through)."),
        "{:?}",
        game.notices
    );
    assert!(
        !game.notices.iter().any(|n| n.contains("relay")),
        "{:?}",
        game.notices
    );
    assert_eq!(game.joiner().relay_state(), RelayState::None);
    let facts = game
        .session()
        .through_facts()
        .expect("a join through the master");
    assert_eq!(facts.relayed_bytes, 0);
    // The net log names the path.
    let logs = std::fs::read_dir(game.data.join(files::LOG_FOLDER))
        .expect("the log folder")
        .filter_map(|e| std::fs::read_to_string(e.ok()?.path()).ok())
        .collect::<String>();
    assert!(
        logs.lines()
            .any(|l| l.contains("\tpath\tpunched\t127.0.0.1:")),
        "{logs}"
    );
    game.leave();
    game.run_for(Duration::from_millis(300));
    game.host.take();
}

/// Join on a listed game with the relay only: the session asks the master
/// for the relay, joins the channel's relayed address with a new client,
/// flies, stays connected through a stall by keepalives framed for the
/// channel, and closes the channel when it leaves.
#[test]
fn a_listed_game_is_joined_through_the_relay_flown_and_kept_through_a_stall() {
    let mut game = Game::start("relay", JoinPath::Relay, true);
    game.join();
    assert!(
        game.run_until(Duration::from_secs(8), |g| g
            .session()
            .connected_after()
            .is_some()),
        "connected through the relay: {:?}",
        game.seen
    );
    assert_eq!(game.session().client.path(), JoinedPath::Relay);
    assert!(is_relayed(game.session().client.server()));
    assert!(matches!(
        game.joiner().relay_state(),
        RelayState::Open { .. }
    ));
    game.fly();
    let screen = game.screen.message_lines();
    assert!(
        in_order(
            &screen,
            &[
                "Asking the Internet Lobby to introduce you to 'Host's game'...",
                "Joining 'Host's game' through the relay only (TORE_JOIN_PATH=relay)...",
            ]
        ),
        "{screen:?}"
    );
    assert!(
        game.notices.starts_with(&[
            "Asking for the relay...".to_owned(),
            "The relay is open; joining through it...".to_owned(),
            "Connected through the relay.".to_owned(),
        ]),
        "{:?}",
        game.notices
    );
    // The relay carried the game both ways.
    let facts = game
        .session()
        .through_facts()
        .expect("a join through the master");
    assert!(facts.relayed_bytes > 10_000, "{facts:?}");
    // The keepalive thread runs, framing for the channel.
    assert!(game.session().kept.running());
    // A stall longer than the host's 5-second timeout: the framed
    // keepalives hold the connection.
    game.stalled = true;
    game.run_for(Duration::from_secs(7));
    game.stalled = false;
    game.run_for(Duration::from_secs(2));
    assert!(game.closed().is_none(), "{:?}", game.seen);
    assert_eq!(game.session().client.phase(), ClientPhase::Flying);
    assert!(
        game.session().kept.sent() >= 3,
        "{}",
        game.session().kept.sent()
    );
    // Leaving closes the channel.
    game.leave();
    assert_eq!(game.joiner().relay_state(), RelayState::Ended);
    game.run_for(Duration::from_millis(300));
    game.host.take();
}

/// A master whose relay is switched off refuses the relay: the session
/// ends with the master's own words, and nothing joined.
#[test]
fn a_refused_relay_ends_the_join_with_the_masters_words() {
    let mut game = Game::start("refused", JoinPath::Relay, false);
    game.join();
    assert!(
        game.run_until(Duration::from_secs(8), |g| g.closed().is_some()),
        "the join ends: {:?}",
        game.seen
    );
    let reason = game.closed().cloned().expect("closed");
    assert_eq!(
        game.session().close_text(&reason),
        "The Internet Lobby's relay is switched off."
    );
    assert_eq!(game.notices, ["Asking for the relay..."]);
    assert!(game.session().connected_after().is_none());
    game.host.take();
}

/// A game that stopped after the screen listed it: Join is refused by the
/// master in a plain line on the screen, and no session starts.
#[test]
fn a_refused_introduction_is_a_plain_line_on_the_screen() {
    let mut game = Game::start("gone", JoinPath::Auto, true);
    game.listed();
    // The host stops: it unregisters at once.
    game.host.take();
    game.run_for(Duration::from_millis(300));
    assert_eq!(game.screen.join_row(0), Outcome::None);
    assert!(
        game.run_until(Duration::from_secs(10), |g| {
            assert_eq!(g.screen.update(false), Outcome::None);
            g.screen
                .message_lines()
                .last()
                .is_some_and(|l| l == "That game is no longer listed.")
        }),
        "{:?}",
        game.screen.message_lines()
    );
    assert!(game.screen.take_through().is_none());
}

/// A transport of a join through the master, on its own socket, for the
/// rules that need no host.
fn transport(path: JoinPath, relay_now: bool, asked: Duration) -> Box<MasterTransport> {
    let socket = ServerSocket::bind(Listen::Address(Ipv4Addr::LOCALHOST.into()), 0).unwrap();
    let joiner = Joiner::new(
        JoinConfig {
            build: tore_net::master::Build {
                protocol_version: 1,
                game_version: "test".into(),
                game_commit: "test".into(),
                release: false,
            },
            listing_id: 1,
            entropy: Entropy::Seeded(1),
        },
        Duration::ZERO,
    );
    let race = tore_session::client::Race {
        targets: vec![tore_net::Target::typed("127.0.0.1:9".parse().unwrap())],
        introduction: 7,
    };
    MasterTransport::new(
        socket,
        joiner,
        Introduction {
            race,
            relay_now,
            path,
            asked,
        },
    )
}

/// When the relay is asked for: at once with the relay only or on the
/// master's hint, after 3 seconds of a race with no answer, never for a
/// race only, never once an address answered, and once.
#[test]
fn the_relay_is_asked_for_after_three_seconds_or_at_once_and_only_once() {
    let start = Duration::from_secs(100);
    let at = |s: f64| start + Duration::from_secs_f64(s);
    let mut auto = transport(JoinPath::Auto, false, Duration::from_secs(1));
    auto.begin(start);
    assert_eq!(auto.relay_reason(at(0.0), false), None);
    assert_eq!(auto.relay_reason(at(2.9), false), None);
    assert_eq!(
        auto.relay_reason(at(3.0), false),
        Some("No direct path; asking for the relay...")
    );
    assert_eq!(auto.relay_reason(at(3.0), true), None);
    auto.relay_asked = true;
    assert_eq!(auto.relay_reason(at(5.0), false), None);
    let mut hinted = transport(JoinPath::Auto, true, Duration::ZERO);
    hinted.begin(start);
    assert!(
        hinted
            .relay_reason(at(0.0), false)
            .is_some_and(|why| why.starts_with("The Internet Lobby says only the relay"))
    );
    let mut relay = transport(JoinPath::Relay, false, Duration::ZERO);
    relay.begin(start);
    assert_eq!(
        relay.relay_reason(at(0.0), false),
        Some("Asking for the relay...")
    );
    let mut direct = transport(JoinPath::Direct, true, Duration::ZERO);
    direct.begin(start);
    assert_eq!(direct.relay_reason(at(10.0), false), None);
}

/// The whole join gives up 15 seconds after Join was pressed: the time the
/// introduction took counts. A join that never connected says why plainly.
#[test]
fn the_join_gives_up_fifteen_seconds_after_join_was_pressed() {
    let start = Duration::from_secs(50);
    let mut t = transport(JoinPath::Auto, false, Duration::from_millis(1_500));
    t.begin(start);
    assert_eq!(t.give_up, start + Duration::from_millis(13_500));
    assert_eq!(
        t.no_answer_text("Friday night"),
        "No answer from 'Friday night' at any of its addresses."
    );
    t.relay_asked = true;
    assert_eq!(
        t.no_answer_text("Friday night"),
        "Could not reach 'Friday night' directly or through the relay."
    );
    // An introduction that took longer than the whole join leaves nothing.
    let mut late = transport(JoinPath::Auto, false, Duration::from_secs(20));
    late.begin(start);
    assert_eq!(late.give_up, start);
}

/// With the relay only, nothing goes to the host's own addresses: the race
/// cannot win on a machine where every path works.
#[test]
fn the_relay_only_sends_nothing_to_the_hosts_own_addresses() {
    let listener = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let to = listener.local_addr().unwrap();
    let mut relay = Transport::Internet(transport(JoinPath::Relay, false, Duration::ZERO));
    relay.send_datagram(to, b"hello").unwrap();
    let mut auto = Transport::Internet(transport(JoinPath::Auto, false, Duration::ZERO));
    auto.send_datagram(to, b"hello").unwrap();
    thread::sleep(Duration::from_millis(50));
    let mut buf = [0u8; 16];
    let mut got = 0;
    while listener.recv_from(&mut buf).is_ok() {
        got += 1;
    }
    assert_eq!(got, 1, "only the join that may race sends to the host");
}

#[test]
fn the_join_path_reads_auto_direct_or_relay() {
    assert_eq!(JoinPath::parse("relay"), Some(JoinPath::Relay));
    assert_eq!(JoinPath::parse(" Direct "), Some(JoinPath::Direct));
    assert_eq!(JoinPath::parse("auto"), Some(JoinPath::Auto));
    assert_eq!(JoinPath::parse(""), Some(JoinPath::Auto));
    assert_eq!(JoinPath::parse("ipx"), None);
    assert_eq!(JoinPath::default(), JoinPath::Auto);
}
