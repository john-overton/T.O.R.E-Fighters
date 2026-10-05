//! The listing end to end (slice I3's acceptance): a host's rendezvous
//! (`tore_net::master::Rendezvous`) against the real master,
//! `tore_master::Master`, on the network simulator's virtual clock, browsed
//! by the Internet Lobby's own client (`tore_net::master::Browser`); and a
//! real `tore-server` with `broadcast on` against a real master on
//! 127.0.0.1.

use crate::{
    app::serve_with,
    clock::RealTimer,
    config::Listen,
    console::Command,
    log::Logger,
    options::Options,
    prepare::{
        self,
        tests::{MISSION, data_folder},
    },
    wiring,
};
use std::{
    fs,
    net::{SocketAddr, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::channel,
    },
    time::{Duration, Instant},
};
use tore_master::{Master, MasterPort, Settings};
use tore_net::master::browse::{Browser, BrowserConfig};
use tore_net::master::{
    Build, Candidate, CandidateKind, HostRendezvous, ListingState, ListingSummary, MappingType,
    Rendezvous, RendezvousEvent,
};
use tore_net::packet::DiscoverPhase;
use tore_net::sim::{LinkConfig, Prefix, RouterConfig, SimNetwork, SimSocket};
use tore_net::{Datagrams, Entropy, RealClock, bind_udp};

const MASTER: &str = "198.51.100.1:26901";
const PROBE: &str = "198.51.100.1:26902";
const BROWSER: &str = "192.0.2.50:5000";

fn address(text: &str) -> SocketAddr {
    text.parse().unwrap()
}

fn build() -> Build {
    Build {
        protocol_version: tore_session::wire::PROTOCOL_VERSION,
        game_version: "0.1.3".into(),
        game_commit: "abc".into(),
        release: false,
    }
}

fn summary(players: u8) -> ListingSummary {
    ListingSummary {
        protocol_version: tore_session::wire::PROTOCOL_VERSION,
        phase: DiscoverPhase::Lobby,
        players,
        capacity: 15,
        session_id: 7,
        game_version: "0.1.3".into(),
        game_commit: "abc".into(),
        name: "Friday night".into(),
        mission: "UKR, 2 v 2".into(),
        callsigns: (0..players).map(|n| format!("Pilot{n}")).collect(),
        ..ListingSummary::default()
    }
}

/// The master, a host and a browser on the simulator.
struct Rig {
    net: SimNetwork,
    master: Master,
    main: SimSocket,
    probe: SimSocket,
    host: SimSocket,
    rendezvous: Rendezvous,
    browser: Browser,
    browser_socket: SimSocket,
    players: u8,
    /// The host is still there.
    alive: bool,
    /// The master hears and answers.
    master_up: bool,
    /// Datagrams the host's transport would have read.
    transport: usize,
    events: Vec<RendezvousEvent>,
    next_refresh: Duration,
}

impl Rig {
    /// A host at `host` (behind a home router when `nat`), listed from the
    /// start.
    fn new(nat: bool) -> Self {
        let net = SimNetwork::new(9);
        net.set_default_link(LinkConfig::one_way(Duration::from_millis(25)));
        let host_address = if nat {
            net.add_router(RouterConfig::nat(
                "203.0.113.9".parse().unwrap(),
                "192.168.1.0/24".parse::<Prefix>().unwrap(),
            ))
            .unwrap();
            address("192.168.1.20:26900")
        } else {
            address("203.0.113.5:26900")
        };
        let mut rendezvous = Rendezvous::host(
            HostRendezvous {
                build: build(),
                dedicated: true,
                install_id: Some(0x5eed),
                platform: 3,
                entropy: Entropy::Seeded(4),
            },
            net.now(),
        );
        rendezvous.set_masters(
            vec![address(MASTER)],
            vec![Candidate::new(CandidateKind::Local, host_address)],
            net.now(),
        );
        rendezvous.set_listed(true, net.now());
        let browser = Browser::new(
            BrowserConfig {
                build: build(),
                other_builds: false,
                full_games: true,
                entropy: Entropy::Seeded(5),
            },
            vec![address(MASTER)],
        );
        Self {
            master: Master::new(Settings::default(), Entropy::Seeded(1), 0),
            main: net.bind(address(MASTER)).unwrap(),
            probe: net.bind(address(PROBE)).unwrap(),
            host: net.bind(host_address).unwrap(),
            browser_socket: net.bind(address(BROWSER)).unwrap(),
            net,
            rendezvous,
            browser,
            players: 1,
            alive: true,
            master_up: true,
            transport: 0,
            events: Vec::new(),
            next_refresh: Duration::ZERO,
        }
    }

    fn step(&mut self) {
        self.net.advance(Duration::from_millis(10));
        let now = self.net.now();
        if self.master_up {
            self.master
                .receive_from(now, MasterPort::Main, &mut self.main)
                .unwrap();
            self.master
                .receive_from(now, MasterPort::Probe, &mut self.probe)
                .unwrap();
            self.master.update(now);
            self.master
                .transmit(&mut self.main, Some(&mut self.probe))
                .unwrap();
        } else {
            let mut buf = [0u8; 2048];
            while self.main.recv_datagram(&mut buf).unwrap().is_some() {}
            while self.probe.recv_datagram(&mut buf).unwrap().is_some() {}
        }
        while self.master.poll_log().is_some() {}
        if self.alive {
            let mut buf = [0u8; 2048];
            let mut routed = self.rendezvous.over(&mut self.host, now);
            while routed.recv_datagram(&mut buf).unwrap().is_some() {
                self.transport += 1;
            }
            if self.rendezvous.wants_summary(now) {
                self.rendezvous.set_summary(now, summary(self.players));
            }
            self.rendezvous.update(now);
            self.rendezvous.transmit(&mut self.host).unwrap();
            while let Some(event) = self.rendezvous.poll_event() {
                self.events.push(event);
            }
        }
        if now >= self.next_refresh {
            self.browser.refresh(now);
            self.next_refresh = now + Duration::from_millis(500);
        }
        self.browser
            .receive_from(&mut self.browser_socket, now)
            .unwrap();
        self.browser.update(now);
        self.browser.transmit(&mut self.browser_socket).unwrap();
        while self.browser.poll_event().is_some() {}
    }

    fn run(&mut self, seconds: f64) {
        for _ in 0..(seconds * 100.0).round() as u64 {
            self.step();
        }
    }

    /// Runs until `done`, at most `seconds`; how long it took.
    fn until(&mut self, seconds: f64, done: impl Fn(&Self) -> bool) -> Option<f64> {
        let start = self.net.now();
        for _ in 0..(seconds * 100.0).round() as u64 {
            self.step();
            if done(self) {
                return Some((self.net.now() - start).as_secs_f64());
            }
        }
        None
    }

    fn listed_players(&self) -> Option<u8> {
        self.master
            .listings()
            .iter()
            .next()
            .map(|l| l.summary.players)
    }

    fn browsed_players(&self) -> Option<u8> {
        self.browser.games().first().map(|g| g.players)
    }
}

#[test]
fn a_host_is_listed_within_its_first_exchange_and_a_browser_sees_it() {
    let mut rig = Rig::new(false);
    // Register, Challenge, Register with the cookie: two round trips of 50 ms.
    let took = rig
        .until(1.0, |rig| rig.master.listings().len() == 1)
        .expect("listed");
    assert!(took <= 0.15, "{took}");
    // The next Browse lists it, with its name and players.
    rig.until(1.0, |rig| !rig.browser.games().is_empty())
        .expect("browsed");
    let game = &rig.browser.games()[0];
    assert_eq!((game.name.as_str(), game.players), ("Friday night", 1));
    assert!(game.dedicated);
    rig.run(1.0);
    assert!(matches!(
        rig.rendezvous.state(),
        ListingState::Listed { seen, .. } if seen == address("203.0.113.5:26900")
    ));
    // The master's datagrams never reached the transport.
    assert_eq!(rig.transport, 0);
    assert_eq!(rig.rendezvous.counters.malformed, 0);
    assert_eq!(rig.rendezvous.counters.unexpected, 0);
}

#[test]
fn summary_changes_reach_a_browser_within_5_seconds() {
    let mut rig = Rig::new(false);
    rig.run(12.0);
    for players in [2u8, 3, 5, 4] {
        rig.players = players;
        let took = rig
            .until(10.0, |rig| rig.listed_players() == Some(players))
            .expect("the change reaches the master");
        assert!(took <= 5.0, "{players}: {took}");
        rig.until(2.0, |rig| rig.browsed_players() == Some(players))
            .expect("and the browser");
    }
}

#[test]
fn heartbeats_and_keeps_stay_within_the_masters_limits_for_ten_minutes() {
    let mut rig = Rig::new(true);
    for players in 1..=10u8 {
        rig.players = players;
        rig.run(60.0);
    }
    assert_eq!(rig.master.counters().dropped_limit, 0);
    assert_eq!(rig.master.listings().len(), 1);
    assert!(!rig.events.contains(&RendezvousEvent::MasterSilent));
    let listed = rig
        .events
        .iter()
        .filter(|e| matches!(e, RendezvousEvent::Listed { .. }))
        .count();
    assert_eq!(listed, 1, "{:?}", rig.events);
    // The mapping test paired its two probes: a home router keeps one port.
    assert_eq!(rig.rendezvous.mapping(), MappingType::SamePort);
    assert_eq!(
        rig.master.listings().iter().next().unwrap().mapping,
        MappingType::SamePort
    );
}

#[test]
fn a_vanished_host_is_gone_from_the_list_within_90_seconds() {
    let mut rig = Rig::new(false);
    rig.run(20.0);
    rig.alive = false;
    let took = rig
        .until(120.0, |rig| rig.master.listings().is_empty())
        .expect("expired");
    assert!(took <= 90.0, "{took}");
    rig.until(2.0, |rig| rig.browser.games().is_empty())
        .expect("and the browser drops it");
}

#[test]
fn a_master_restart_is_healed_within_one_heartbeat() {
    let mut rig = Rig::new(false);
    rig.run(10.0);
    rig.master = Master::new(Settings::default(), Entropy::Seeded(2), 0);
    let took = rig
        .until(40.0, |rig| rig.master.listings().len() == 1)
        .expect("listed again");
    assert!(took <= 30.0, "{took}");
}

#[test]
fn a_master_that_goes_quiet_is_asked_at_most_once_a_second_and_relists_the_game() {
    let mut rig = Rig::new(false);
    rig.run(5.0);
    rig.master_up = false;
    rig.net.start_trace();
    rig.run(200.0);
    let sends: Vec<f64> = rig
        .net
        .take_trace()
        .iter()
        .filter(|t| t.from == address("203.0.113.5:26900") && t.to == address(MASTER))
        .map(|t| t.at.as_secs_f64())
        .collect();
    // Keeps and requests alike, never two in a second.
    assert!(sends.windows(2).all(|p| p[1] - p[0] >= 1.0), "{sends:?}");
    assert!(rig.events.contains(&RendezvousEvent::MasterSilent));
    // A new master is up again with nothing listed.
    rig.master = Master::new(Settings::default(), Entropy::Seeded(3), 0);
    rig.master_up = true;
    rig.until(61.0, |rig| rig.master.listings().len() == 1)
        .expect("listed again");
}

#[test]
fn unlisting_takes_the_game_off_at_once() {
    let mut rig = Rig::new(false);
    rig.run(2.0);
    assert_eq!(rig.master.listings().len(), 1);
    let now = rig.net.now();
    rig.rendezvous.set_listed(false, now);
    rig.until(0.2, |rig| rig.master.listings().is_empty())
        .expect("removed at once");
}

/// A master on real loopback sockets, on a thread of its own: the main port
/// and the main port + 1.
struct LoopbackMaster {
    port: u16,
    names: Arc<Mutex<Vec<String>>>,
    lines: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl LoopbackMaster {
    fn start() -> Self {
        let (main, probe) = loop {
            let main = bind_udp("127.0.0.1:0".parse().unwrap()).unwrap();
            let port = main.local_addr().unwrap().port();
            if port == u16::MAX {
                continue;
            }
            if let Ok(probe) = bind_udp(format!("127.0.0.1:{}", port + 1).parse().unwrap()) {
                break (main, probe);
            }
        };
        let port = main.local_addr().unwrap().port();
        let names = Arc::new(Mutex::new(Vec::new()));
        let lines = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = std::thread::spawn({
            let (names, lines, stop) = (names.clone(), lines.clone(), stop.clone());
            move || {
                let (mut main, mut probe): (UdpSocket, UdpSocket) = (main, probe);
                let mut master = Master::new(Settings::default(), Entropy::System, 0);
                let clock = RealClock::new();
                while !stop.load(Ordering::Relaxed) {
                    let now = clock.now();
                    let _ = master.receive_from(now, MasterPort::Main, &mut main);
                    let _ = master.receive_from(now, MasterPort::Probe, &mut probe);
                    master.update(now);
                    let _ = master.transmit(&mut main, Some(&mut probe));
                    while let Some(line) = master.poll_log() {
                        lines.lock().unwrap().push(line);
                    }
                    *names.lock().unwrap() = master
                        .listings()
                        .iter()
                        .map(|l| l.summary.name.clone())
                        .collect();
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        });
        Self {
            port,
            names,
            lines,
            stop,
            handle: Some(handle),
        }
    }

    fn names(&self) -> Vec<String> {
        self.names.lock().unwrap().clone()
    }

    fn wait(&self, what: &str, done: impl Fn(&[String]) -> bool) {
        let start = Instant::now();
        while !done(&self.names()) {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "timed out waiting for {what}: {:?}",
                self.lines.lock().unwrap()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for LoopbackMaster {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The real server with `broadcast on` lists itself on a master on
/// 127.0.0.1, `broadcast off` and `on` take it off and back, and `quit`
/// takes it off before the server stops.
#[test]
fn a_broadcasting_server_lists_itself_and_quitting_removes_it() {
    let master = LoopbackMaster::start();
    let dir = data_folder("listing", true);
    fs::write(dir.join("mission.txt"), MISSION).unwrap();
    let (console, commands) = channel();
    let server_dir = dir.clone();
    let master_text = format!("127.0.0.1:{}", master.port);
    let server = std::thread::spawn(move || {
        let mut prepared = prepare::prepare(&Options::default(), &server_dir).unwrap();
        prepared.config.address = Listen::Address("127.0.0.1".parse().unwrap());
        prepared.config.port = 0;
        prepared.config.status_interval_seconds = 0;
        prepared.config.name = "Night Owls".into();
        prepared.config.broadcast = true;
        prepared.config.master = master_text;
        let log = Logger::new(server_dir.join("logs"), Box::new(std::io::sink()));
        serve_with(
            prepared,
            wiring::start_host,
            commands,
            &mut RealTimer::new(),
            log,
        )
    });
    master.wait("the listing", |names| names == ["Night Owls"]);
    console.send(Command::Broadcast(false)).unwrap();
    master.wait("the listing to go", |names| names.is_empty());
    console.send(Command::Broadcast(true)).unwrap();
    master.wait("the listing again", |names| names.len() == 1);
    console.send(Command::Status).unwrap();
    console.send(Command::Quit).unwrap();
    assert_eq!(server.join().expect("the server thread"), Ok(()));
    master.wait("the listing to go at quit", |names| names.is_empty());
    let lines = master.lines.lock().unwrap().clone();
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.starts_with("unlisted") && l.contains("reason=unregistered"))
            .count(),
        2,
        "{lines:?}"
    );
    let mut log = String::new();
    for entry in fs::read_dir(dir.join("logs")).unwrap().flatten() {
        log += &fs::read_to_string(entry.path()).unwrap();
    }
    for needle in [
        "Broadcast: on, to the Internet Lobby at 127.0.0.1:",
        "Broadcasting: listed on the Internet Lobby (127.0.0.1:",
        "console: broadcast off",
        "Broadcasting stopped: off the Internet Lobby",
        "console: broadcast on",
        "Stopped",
    ] {
        assert!(log.contains(needle), "{needle} in\n{log}");
    }
    // Telemetry is on by default: the server kept an install id.
    assert!(dir.join(crate::app::INSTALL_ID_FILE).exists());
    let _ = fs::remove_dir_all(dir);
}

/// Slice J4b: the test builds of the server never choose the real router,
/// whatever `port-mapping` says.
#[test]
fn a_test_build_never_chooses_the_real_router() {
    assert!(wiring::mapping_choice(true).is_none());
    assert!(wiring::mapping_choice(false).is_none());
}

/// Slice J4b: a server with `port-mapping on` asks the router (a fake
/// gateway on loopback here) when it starts, logs what the router did in the
/// operator's words, and removes the mapping when it stops.
#[test]
fn a_server_with_port_mapping_forwards_its_port_and_removes_it_at_quit() {
    use tore_net::portmap::MapperConfig;
    use tore_net::portmap::fake::{FakeGateway, FakeGatewayConfig};
    use tore_net::portmap::keeper::KeeperConfig;
    let router = Arc::new(
        FakeGateway::start("127.0.0.1:0".parse().unwrap(), FakeGatewayConfig::default())
            .expect("a fake router"),
    );
    let dir = data_folder("portmap", true);
    fs::write(dir.join("mission.txt"), MISSION).unwrap();
    let (console, commands) = channel();
    let server_dir = dir.clone();
    let mapper = KeeperConfig::with_config(MapperConfig {
        upnp: false,
        gateway: Some(router.address()),
        gateway_v6: Some("[::1]:9".parse().unwrap()),
        entropy: Entropy::Seeded(3),
        ..MapperConfig::new(0)
    });
    let server = std::thread::spawn(move || {
        let mut prepared = prepare::prepare(&Options::default(), &server_dir).unwrap();
        prepared.config.address = Listen::Address("127.0.0.1".parse().unwrap());
        prepared.config.port = 0;
        prepared.config.status_interval_seconds = 0;
        prepared.config.port_mapping = true;
        let log = Logger::new(server_dir.join("logs"), Box::new(std::io::sink()));
        serve_with(
            prepared,
            move |setup| wiring::start_host_with(setup, Some(mapper)),
            commands,
            &mut RealTimer::new(),
            log,
        )
    });
    let start = Instant::now();
    while router.mappings().is_empty() {
        assert!(start.elapsed() < Duration::from_secs(10), "no mapping");
        std::thread::sleep(Duration::from_millis(5));
    }
    let held = router.mappings();
    assert_eq!(held.len(), 1);
    let port = held[0].internal_port;
    // The news reaches the log on a later poll.
    std::thread::sleep(Duration::from_millis(300));
    console.send(Command::Quit).unwrap();
    assert_eq!(server.join().expect("the server thread"), Ok(()));
    assert!(router.mappings().is_empty(), "removed at quit");
    let mut log = String::new();
    for entry in fs::read_dir(dir.join("logs")).unwrap().flatten() {
        log += &fs::read_to_string(entry.path()).unwrap();
    }
    let line = format!(
        "Port mapping: Your router forwards UDP port {port} (PCP). Friends can join at 203.0.113.5:{port}."
    );
    assert!(log.contains(&line), "{line} in\n{log}");
    let _ = fs::remove_dir_all(dir);
}
