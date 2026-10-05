//! The master on the network simulator, with scripted hosts and the browse
//! client (slice I2's acceptance in the architecture guide's slice table).

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use tore_net::master::browse::{BrowseEvent, Browser, BrowserConfig};
use tore_net::master::{
    Browse, Build, Candidate, CandidateKind, Details, Heartbeat, Introduce, Keep, Listed,
    ListingSummary, MappingType, MasterPacket, Path, PortMapping, Probe, ProbePort, Register,
    Report, Role, Unregister,
};
use tore_net::packet::DiscoverPhase;
use tore_net::sim::{Mapping, Prefix, RouterConfig, SimNetwork, SimSocket};
use tore_net::{Datagrams, Entropy};

use crate::master::{Master, MasterPort, Settings};

fn a(text: &str) -> SocketAddr {
    text.parse().unwrap()
}

const MAIN_V4: &str = "198.51.100.1:26901";
const MAIN_V6: &str = "[2001:db8:ffff::1]:26901";
const PROBE_V4: &str = "198.51.100.1:26902";
const PROBE_V6: &str = "[2001:db8:ffff::1]:26902";

/// A master on the simulator, on both ports and both families.
struct Rig {
    net: SimNetwork,
    master: Master,
    main: SimSocket,
    probe: SimSocket,
}

impl Rig {
    fn new(settings: Settings) -> Self {
        Self::with_network(SimNetwork::new(7), settings)
    }

    fn with_network(net: SimNetwork, settings: Settings) -> Self {
        net.set_now(Duration::from_secs(100));
        let main = net.bind_dual(a(MAIN_V4), a(MAIN_V6)).unwrap();
        let probe = net.bind_dual(a(PROBE_V4), a(PROBE_V6)).unwrap();
        Self {
            master: Master::new(settings, Entropy::Seeded(11), 0),
            net,
            main,
            probe,
        }
    }

    fn now(&self) -> Duration {
        self.net.now()
    }

    /// Reads both ports, runs the timers and sends the answers.
    fn turn(&mut self) {
        let now = self.net.now();
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
    }

    fn step(&mut self, dt: Duration) {
        self.net.advance(dt);
        self.turn();
    }

    /// Moves to `at`, a turn every `every`.
    fn run_until(&mut self, at: Duration, every: Duration) {
        while self.now() + every <= at {
            self.step(every);
        }
        if self.now() < at {
            let rest = at - self.now();
            self.step(rest);
        }
    }

    fn logs(&mut self) -> Vec<String> {
        std::iter::from_fn(|| self.master.poll_log()).collect()
    }
}

fn send(socket: &mut SimSocket, to: &str, packet: &MasterPacket) -> usize {
    let bytes = packet.encode().unwrap();
    socket.send_datagram(a(to), &bytes).unwrap();
    bytes.len()
}

/// Everything waiting at a socket, decoded, with its length.
fn received(socket: &mut SimSocket) -> Vec<(MasterPacket, usize)> {
    let mut buf = [0u8; 2_048];
    let mut got = Vec::new();
    while let Some((len, _)) = socket.recv_datagram(&mut buf).unwrap() {
        got.push((MasterPacket::decode(&buf[..len]).unwrap(), len));
    }
    got
}

fn build() -> Build {
    Build {
        protocol_version: 7,
        game_version: "0.1.3".into(),
        game_commit: "abc".into(),
        release: true,
    }
}

fn other_build() -> Build {
    Build {
        protocol_version: 7,
        game_version: "0.1.4-2-gdef".into(),
        game_commit: "def".into(),
        release: false,
    }
}

fn summary(name: &str, players: u8, phase: DiscoverPhase) -> ListingSummary {
    ListingSummary {
        protocol_version: 7,
        players,
        capacity: 8,
        phase,
        session_id: 1,
        game_version: "0.1.3".into(),
        game_commit: "abc".into(),
        name: name.into(),
        mission: "UKR, clear".into(),
        king: "Viper".into(),
        callsigns: vec!["Viper".into()],
        ..ListingSummary::default()
    }
}

/// A scripted host: a socket that lists a game.
struct Host {
    socket: SimSocket,
    nonce: u64,
    build: Build,
    candidates: Vec<Candidate>,
    listed: Option<Listed>,
}

impl Host {
    fn new(rig: &Rig, address: &str, nonce: u64) -> Self {
        Self::on(rig.net.bind(a(address)).unwrap(), nonce)
    }

    fn on(socket: SimSocket, nonce: u64) -> Self {
        Self {
            socket,
            nonce,
            build: build(),
            candidates: Vec::new(),
            listed: None,
        }
    }

    fn register(&self, cookie: u64, summary: &ListingSummary) -> MasterPacket {
        MasterPacket::Register(
            Register {
                nonce: self.nonce,
                cookie,
                build: self.build.clone(),
                dedicated: false,
                telemetry: false,
                install_id: 0,
                platform: 3,
                candidates: self.candidates.clone(),
                summary: summary.clone(),
            }
            .fit(),
        )
    }

    /// Register, the Challenge, Register with its cookie: the Listed, or
    /// `None` when the master said nothing.
    fn list(&mut self, rig: &mut Rig, summary: &ListingSummary) -> Option<Listed> {
        let packet = self.register(0, summary);
        send(&mut self.socket, MAIN_V4, &packet);
        rig.turn();
        let got = received(&mut self.socket);
        let [(MasterPacket::Challenge(challenge), 23)] = got.as_slice() else {
            panic!("expected one Challenge, got {got:?}");
        };
        assert_eq!(challenge.nonce, self.nonce);
        let cookie = challenge.cookie;
        let packet = self.register(cookie, summary);
        send(&mut self.socket, MAIN_V4, &packet);
        rig.turn();
        let got = received(&mut self.socket);
        let listed = match got.as_slice() {
            [(MasterPacket::Listed(listed), len)] => {
                assert!(*len <= 53);
                Some(*listed)
            }
            [] => None,
            other => panic!("expected Listed, got {other:?}"),
        };
        self.listed = listed;
        listed
    }

    fn token(&self) -> u64 {
        self.listed.expect("listed").token
    }

    fn heartbeat(
        &mut self,
        rig: &mut Rig,
        summary: &ListingSummary,
        change: u16,
    ) -> Vec<(MasterPacket, usize)> {
        let beat = MasterPacket::Heartbeat(
            Heartbeat {
                token: self.token(),
                change,
                candidates: self.candidates.clone(),
                summary: summary.clone(),
            }
            .fit(),
        );
        send(&mut self.socket, MAIN_V4, &beat);
        rig.turn();
        received(&mut self.socket)
    }

    fn keep(&mut self, rig: &mut Rig) -> Vec<(MasterPacket, usize)> {
        let keep = MasterPacket::Keep(Keep {
            token: self.token(),
        });
        send(&mut self.socket, MAIN_V4, &keep);
        rig.turn();
        received(&mut self.socket)
    }
}

/// A browse client on a socket of its own.
struct Asker {
    socket: SimSocket,
    browser: Browser,
}

impl Asker {
    fn new(rig: &Rig, address: &str, other_builds: bool, full_games: bool) -> Self {
        Self {
            socket: rig.net.bind(a(address)).unwrap(),
            browser: Browser::new(
                BrowserConfig {
                    build: build(),
                    other_builds,
                    full_games,
                    entropy: Entropy::Seeded(99),
                },
                vec![a(MAIN_V4)],
            ),
        }
    }

    /// Refreshes the list, turning the master until the refresh ends.
    fn refresh(&mut self, rig: &mut Rig) -> Vec<BrowseEvent> {
        self.browser.refresh(rig.now());
        for _ in 0..100 {
            self.browser.transmit(&mut self.socket).unwrap();
            rig.step(Duration::from_millis(10));
            self.browser
                .receive_from(&mut self.socket, rig.now())
                .unwrap();
            self.browser.update(rig.now());
            if !self.browser.refreshing() {
                break;
            }
        }
        std::iter::from_fn(|| self.browser.poll_event()).collect()
    }
}

#[test]
fn no_listing_without_a_cookie_and_a_forged_source_gets_only_a_challenge() {
    let mut rig = Rig::new(Settings::default());
    let mut host = Host::new(&rig, "203.0.113.5:26900", 41);
    let game = summary("Friday night", 1, DiscoverPhase::Lobby);
    // No cookie, then a wrong one: a Challenge each, no listing.
    let mut cookie = 0;
    for claimed in [0, 12_345] {
        let packet = host.register(claimed, &game);
        send(&mut host.socket, MAIN_V4, &packet);
        rig.turn();
        let got = received(&mut host.socket);
        let [(MasterPacket::Challenge(challenge), 23)] = got.as_slice() else {
            panic!("{got:?}")
        };
        assert_eq!(challenge.nonce, 41);
        cookie = challenge.cookie;
        assert!(rig.master.listings().is_empty());
    }
    // A forged source, even one that copies the host's cookie, gets a
    // 23-byte Challenge sent to the address it claims, and nothing else.
    let forged = a("192.0.2.66:4000");
    rig.net.start_trace();
    for claimed in [0, cookie] {
        let bytes = host.register(claimed, &game).encode().unwrap();
        rig.net.inject(forged, a(MAIN_V4), &bytes);
        rig.turn();
    }
    let trace = rig.net.take_trace();
    assert_eq!(trace.len(), 2);
    for entry in &trace {
        assert_eq!(entry.to, forged);
        assert_eq!(entry.datagram.len(), 23);
        assert!(matches!(
            MasterPacket::decode(&entry.datagram).unwrap(),
            MasterPacket::Challenge(_)
        ));
    }
    assert!(rig.master.listings().is_empty());
    // The real host, with its cookie, is listed.
    let listed = host.list(&mut rig, &game).expect("listed");
    assert_eq!(listed.seen, a("203.0.113.5:26900"));
    assert_eq!(
        (listed.heartbeat_secs, listed.keep_secs, listed.expiry_secs),
        (30, 15, 90)
    );
    assert_eq!(rig.master.listings().len(), 1);
    assert_eq!(
        rig.logs(),
        [format!(
            "listed id={:016x} from=203.0.113.5:26900 name=\"Friday night\" listings=1",
            listed.listing_id
        )]
    );
}

#[test]
fn a_listing_appears_in_the_next_browse_and_heartbeats_change_it() {
    let mut rig = Rig::new(Settings::default());
    let mut host = Host::new(&rig, "203.0.113.5:26900", 1);
    let mut asker = Asker::new(&rig, "192.0.2.50:40000", false, false);
    assert_eq!(
        asker.refresh(&mut rig),
        [BrowseEvent::Refreshed {
            matching: 0,
            shown: 0
        }]
    );
    let game = summary("Friday night", 1, DiscoverPhase::Lobby);
    let listed = host.list(&mut rig, &game).unwrap();
    let events = asker.refresh(&mut rig);
    let [
        BrowseEvent::Added(entry),
        BrowseEvent::Refreshed {
            matching: 1,
            shown: 1,
        },
    ] = events.as_slice()
    else {
        panic!("{events:?}")
    };
    assert_eq!(entry.listing_id, listed.listing_id);
    assert_eq!(entry.name, "Friday night");
    assert_eq!((entry.players, entry.capacity, entry.platform), (1, 8, 3));
    assert!(entry.other_build.is_none() && !entry.relay_likely);
    // A heartbeat with another player: acknowledged, and the next browse
    // shows it.
    rig.step(Duration::from_secs(5));
    let more = summary("Friday night", 2, DiscoverPhase::Flying);
    let got = host.heartbeat(&mut rig, &more, 1);
    let [(MasterPacket::HeartbeatAck(ack), len)] = got.as_slice() else {
        panic!("{got:?}")
    };
    assert!(*len <= 34);
    assert_eq!(
        (ack.listing_id, ack.seen),
        (listed.listing_id, a("203.0.113.5:26900"))
    );
    let events = asker.refresh(&mut rig);
    assert!(
        matches!(&events[0], BrowseEvent::Changed(e) if e.players == 2 && e.phase == DiscoverPhase::Flying),
        "{events:?}"
    );
    // Details carry the whole summary.
    asker.browser.ask_details(rig.now(), listed.listing_id);
    asker.browser.transmit(&mut asker.socket).unwrap();
    rig.step(Duration::from_millis(10));
    asker
        .browser
        .receive_from(&mut asker.socket, rig.now())
        .unwrap();
    let event = asker.browser.poll_event();
    assert!(
        matches!(&event, Some(BrowseEvent::Details { summary: Some(s), .. }) if *s == more),
        "{event:?}"
    );
}

#[test]
fn a_silent_listing_expires_at_90_seconds_and_unregister_is_at_once() {
    let mut rig = Rig::new(Settings::default());
    let mut quiet = Host::new(&rig, "203.0.113.5:26900", 1);
    let mut beating = Host::new(&rig, "203.0.113.6:26900", 2);
    let mut leaving = Host::new(&rig, "203.0.113.7:26900", 3);
    let game = summary("Game", 1, DiscoverPhase::Lobby);
    let start = rig.now();
    let silent = quiet.list(&mut rig, &game).unwrap();
    beating.list(&mut rig, &game).unwrap();
    leaving.list(&mut rig, &game).unwrap();
    rig.logs();
    // Unregister, three times as a host sends it: gone at once, no answer.
    let token = leaving.token();
    for _ in 0..3 {
        send(
            &mut leaving.socket,
            MAIN_V4,
            &MasterPacket::Unregister(Unregister { token }),
        );
    }
    rig.turn();
    assert!(received(&mut leaving.socket).is_empty());
    assert_eq!(rig.master.listings().len(), 2);
    let logs = rig.logs();
    assert_eq!(logs.len(), 1);
    assert!(
        logs[0].ends_with("reason=unregistered listings=2"),
        "{logs:?}"
    );
    // The beating host keeps every 15 s and beats every 30 s.
    let mut next_keep = start + Duration::from_secs(15);
    let mut next_beat = start + Duration::from_secs(30);
    let end = start + Duration::from_secs(200);
    while rig.now() < end {
        let at = next_keep.min(next_beat).min(end);
        rig.run_until(at, Duration::from_millis(250));
        if rig.now() == next_beat {
            assert_eq!(beating.heartbeat(&mut rig, &game, 0).len(), 1);
            next_beat += Duration::from_secs(30);
        }
        if rig.now() == next_keep {
            assert!(beating.keep(&mut rig).is_empty());
            next_keep += Duration::from_secs(15);
        }
        let listed = rig.master.listings().get(silent.listing_id).is_some();
        assert_eq!(
            listed,
            rig.now() < start + Duration::from_secs(90),
            "at {:?}",
            rig.now() - start
        );
    }
    assert_eq!(rig.master.listings().len(), 1);
    let logs = rig.logs();
    assert_eq!(logs.len(), 1);
    assert!(
        logs[0].starts_with(&format!(
            "unlisted id={:016x} from=203.0.113.5:26900 name=\"Game\" reason=expired",
            silent.listing_id
        )),
        "{logs:?}"
    );
    // A Keep with a token the master forgot gets Unknown listing, 15 bytes.
    let got = quiet.keep(&mut rig);
    assert!(
        matches!(got.as_slice(), [(MasterPacket::UnknownListing(u), 15)] if u.token == silent.token),
        "{got:?}"
    );
}

#[test]
fn the_expiry_is_exact_on_the_virtual_clock() {
    let mut rig = Rig::new(Settings::default());
    let mut host = Host::new(&rig, "203.0.113.5:26900", 1);
    let listed = host
        .list(&mut rig, &summary("Game", 1, DiscoverPhase::Lobby))
        .unwrap();
    let start = rig.now();
    rig.run_until(
        start + Duration::from_millis(89_999),
        Duration::from_secs(1),
    );
    assert!(rig.master.listings().get(listed.listing_id).is_some());
    rig.step(Duration::from_millis(1));
    assert!(rig.master.listings().get(listed.listing_id).is_none());
}

#[test]
fn pages_list_every_match_once_filtered_by_build_and_fullness_in_order() {
    let mut rig = Rig::new(Settings::default());
    let phases = [
        DiscoverPhase::Flying,
        DiscoverPhase::Lobby,
        DiscoverPhase::Closed,
    ];
    let mut expected = Vec::new();
    for n in 0..45u8 {
        let address = format!("203.0.{}.{}:26900", 100 + n / 8, n % 8 + 1);
        let mut host = Host::new(&rig, &address, u64::from(n) + 1);
        // Long names, so the list takes several pages.
        let name = format!("{:02} {}", n % 7, "x".repeat(55));
        let mut game = summary(&name, n % 5, phases[usize::from(n) % 3]);
        game.full = n % 9 == 0;
        if n % 4 == 0 {
            host.build = other_build();
        }
        let listed = host.list(&mut rig, &game).unwrap();
        expected.push((listed.listing_id, game, n % 4 == 0));
    }
    let rank = |p: DiscoverPhase| match p {
        DiscoverPhase::Lobby => 0,
        DiscoverPhase::Flying => 1,
        DiscoverPhase::Closed => 2,
    };
    expected.sort_by(|(ia, a, _), (ib, b, _)| {
        rank(a.phase)
            .cmp(&rank(b.phase))
            .then(b.players.cmp(&a.players))
            .then(a.name.cmp(&b.name))
            .then(ia.cmp(ib))
    });
    for (other_builds, full_games) in [(false, false), (true, false), (false, true), (true, true)] {
        let browses_before = rig.master.counters().browses;
        let mut asker = Asker::new(&rig, "192.0.2.50:40000", other_builds, full_games);
        let events = asker.refresh(&mut rig);
        let want: Vec<u64> = expected
            .iter()
            .filter(|(_, game, other)| (!other || other_builds) && (!game.full || full_games))
            .map(|(id, _, _)| *id)
            .collect();
        let got: Vec<u64> = asker.browser.games().iter().map(|e| e.listing_id).collect();
        assert_eq!(got, want, "filters {other_builds} {full_games}");
        assert!(matches!(
            events.last(),
            Some(BrowseEvent::Refreshed { matching, shown })
                if usize::from(*matching) == want.len() && *shown == want.len()
        ));
        // Several pages, each answer within its Browse.
        assert!(rig.master.counters().browses - browses_before >= 2);
        for entry in asker.browser.games() {
            let other = expected
                .iter()
                .find(|(id, _, _)| *id == entry.listing_id)
                .unwrap()
                .2;
            assert_eq!(entry.other_build.is_some(), other);
            if other {
                assert_eq!(entry.other_build.as_deref(), Some("0.1.3"));
            }
        }
    }
}

#[test]
fn register_again_replace_and_move() {
    let mut rig = Rig::new(Settings::default());
    let game = summary("Game", 1, DiscoverPhase::Lobby);
    let mut host = Host::new(&rig, "203.0.113.5:26900", 1);
    let first = host.list(&mut rig, &game).unwrap();
    // The same nonce again: the same Listed.
    assert_eq!(host.list(&mut rig, &game).unwrap(), first);
    // Another nonce from the same address: the game restarted.
    host.nonce = 2;
    let second = host.list(&mut rig, &game).unwrap();
    assert_ne!(second.listing_id, first.listing_id);
    assert_eq!(rig.master.listings().len(), 1);
    let logs = rig.logs();
    assert!(
        logs[1].contains("reason=replaced by a new registration"),
        "{logs:?}"
    );
    // The router gives the game port a new outside address: a Keep from
    // there moves the listing, at most once a minute.
    let moved = rig.net.bind(a("203.0.113.5:31000")).unwrap();
    let old = std::mem::replace(&mut host.socket, moved);
    rig.step(Duration::from_secs(3));
    host.keep(&mut rig);
    let listing = rig.master.listings().get(second.listing_id).unwrap();
    assert_eq!(listing.address, a("203.0.113.5:31000"));
    assert_eq!(
        rig.logs(),
        [format!(
            "moved id={:016x} from=203.0.113.5:26900 to=203.0.113.5:31000",
            second.listing_id
        )]
    );
    host.socket = old;
    rig.step(Duration::from_secs(3));
    host.keep(&mut rig);
    let listing = rig.master.listings().get(second.listing_id).unwrap();
    assert_eq!(
        listing.address,
        a("203.0.113.5:31000"),
        "a second move within a minute"
    );
}

#[test]
fn eight_listings_from_one_source_then_nothing() {
    let mut settings = Settings::default();
    // Registers are limited too (10 a minute); this test is about room.
    settings.rates.register = crate::limits::Rate::per_minute(100, 100);
    let mut rig = Rig::new(settings);
    let game = summary("Game", 1, DiscoverPhase::Lobby);
    for port in 0..8u16 {
        let mut host = Host::new(&rig, &format!("203.0.113.5:{}", 27_000 + port), 10);
        assert!(host.list(&mut rig, &game).is_some());
    }
    let mut ninth = Host::new(&rig, "203.0.113.5:28000", 10);
    assert_eq!(ninth.list(&mut rig, &game), None);
    assert_eq!(rig.master.counters().refused_room, 1);
    let mut elsewhere = Host::new(&rig, "203.0.113.6:28000", 10);
    assert!(elsewhere.list(&mut rig, &game).is_some());
    // And 2,000 in all, or the setting.
    let mut small = Rig::new(Settings {
        max_listings: 2,
        ..Settings::default()
    });
    for n in 1..=3 {
        let mut host = Host::new(&small, &format!("203.0.113.{n}:26900"), 1);
        assert_eq!(host.list(&mut small, &game).is_some(), n <= 2);
    }
}

#[test]
fn heartbeats_and_keeps_are_limited_per_listing() {
    let mut rig = Rig::new(Settings::default());
    let game = summary("Game", 1, DiscoverPhase::Lobby);
    let mut host = Host::new(&rig, "203.0.113.5:26900", 1);
    host.list(&mut rig, &game).unwrap();
    // Two at once pass (a Keep and a Heartbeat falling due together).
    assert_eq!(host.heartbeat(&mut rig, &game, 0).len(), 1);
    host.keep(&mut rig);
    assert!(host.heartbeat(&mut rig, &game, 0).is_empty());
    rig.step(Duration::from_secs(2));
    assert_eq!(host.heartbeat(&mut rig, &game, 0).len(), 1);
    assert_eq!(rig.master.counters().heartbeats, 2);
    assert_eq!(rig.master.counters().keeps, 1);
}

#[test]
fn ipv6_sources_count_by_their_64_network() {
    let mut rig = Rig::new(Settings::default());
    let game = summary("Game", 1, DiscoverPhase::Lobby);
    let mut listed = 0;
    for n in 1..=9u64 {
        // Every one from another address of one home's /64.
        let socket = rig
            .net
            .bind(a(&format!("[2001:db8:1:2::{n:x}]:26900")))
            .unwrap();
        let mut host = Host::on(socket, n);
        let packet = host.register(0, &game);
        send(&mut host.socket, MAIN_V6, &packet);
        rig.turn();
        let got = received(&mut host.socket);
        let Some((MasterPacket::Challenge(c), _)) = got.first() else {
            panic!("{got:?}")
        };
        let cookie = c.cookie;
        let packet = host.register(cookie, &game);
        send(&mut host.socket, MAIN_V6, &packet);
        rig.turn();
        if matches!(
            received(&mut host.socket).first(),
            Some((MasterPacket::Listed(_), _))
        ) {
            listed += 1;
        }
        rig.step(Duration::from_secs(7));
    }
    assert_eq!(listed, 8);
    assert_eq!(rig.master.counters().refused_room, 1);
    // Fifty askers from another /64 at the same moment share its burst of 40.
    let browse = MasterPacket::Browse(Browse {
        nonce: 1,
        build: build(),
        other_builds: false,
        full_games: false,
        cursor: 0,
    })
    .encode()
    .unwrap();
    rig.net.start_trace();
    for n in 1..=50u16 {
        let from = SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 9, 9, n, 0, 0, 1)),
            5_000,
        );
        rig.net.inject(from, a(MAIN_V6), &browse);
    }
    rig.turn();
    assert_eq!(rig.net.take_trace().len(), 40);
    assert_eq!(rig.master.sources(), 2);
}

#[test]
fn probes_answer_on_both_ports_and_a_host_behind_a_symmetric_router_is_marked() {
    let net = SimNetwork::new(3);
    // One home router that keeps one outside port for every destination, and
    // one that maps a port per destination ("symmetric").
    let open = net
        .add_router(RouterConfig::nat(
            "203.0.113.20".parse().unwrap(),
            Prefix::new("192.168.1.0".parse().unwrap(), 24).unwrap(),
        ))
        .unwrap();
    let mut symmetric_config = RouterConfig::nat(
        "203.0.113.21".parse().unwrap(),
        Prefix::new("192.168.2.0".parse().unwrap(), 24).unwrap(),
    );
    symmetric_config.mapping = Mapping::AddressAndPortDependent;
    let symmetric = net.add_router(symmetric_config).unwrap();
    let mut rig = Rig::with_network(net, Settings::default());
    let mut verdicts = Vec::new();
    let mut hosts = Vec::new();
    for (router, inside) in [
        (open, "192.168.1.10:26900"),
        (symmetric, "192.168.2.10:26900"),
    ] {
        let socket = rig.net.bind_behind(router, a(inside)).unwrap();
        let mut host = Host::on(socket, 5);
        host.candidates = vec![Candidate::new(CandidateKind::Local, a(inside))];
        // The mapping test: one nonce to both ports.
        send(
            &mut host.socket,
            MAIN_V4,
            &MasterPacket::Probe(Probe { nonce: 77 }),
        );
        send(
            &mut host.socket,
            PROBE_V4,
            &MasterPacket::Probe(Probe { nonce: 77 }),
        );
        rig.turn();
        let answers = received(&mut host.socket);
        assert_eq!(answers.len(), 2);
        let mut seen = BTreeMap::new();
        for (packet, len) in answers {
            let MasterPacket::ProbeAnswer(answer) = packet else {
                panic!("{packet:?}")
            };
            assert!(len <= 35);
            assert_eq!(answer.nonce, 77);
            seen.insert(answer.port == ProbePort::Main, answer.seen);
        }
        verdicts.push(MappingType::from_probes(
            Some(a(inside)),
            seen[&true],
            Some(seen[&false]),
        ));
        let listed = host
            .list(&mut rig, &summary("Game", 1, DiscoverPhase::Lobby))
            .unwrap();
        hosts.push(listed.listing_id);
    }
    assert_eq!(
        verdicts,
        [MappingType::SamePort, MappingType::PortPerDestination]
    );
    let mut asker = Asker::new(&rig, "192.0.2.50:40000", false, false);
    asker.refresh(&mut rig);
    let mark = |id: u64| {
        asker
            .browser
            .games()
            .iter()
            .find(|g| g.listing_id == id)
            .unwrap()
            .relay_likely
    };
    assert!(!mark(hosts[0]));
    assert!(mark(hosts[1]));
}

#[test]
fn unsupported_versions_are_answered_only_within_the_request() {
    let mut rig = Rig::new(Settings::default());
    let mut asker = rig.net.bind(a("192.0.2.50:40000")).unwrap();
    let browse = MasterPacket::Browse(Browse {
        nonce: 1,
        build: build(),
        other_builds: false,
        full_games: false,
        cursor: 0,
    });
    asker
        .send_datagram(a(MAIN_V4), &browse.encode_in(9).unwrap())
        .unwrap();
    // A 64-byte Probe is shorter than any Unsupported: nothing.
    let probe = MasterPacket::Probe(Probe { nonce: 2 })
        .encode_in(9)
        .unwrap();
    asker.send_datagram(a(MAIN_V4), &probe).unwrap();
    rig.turn();
    let got = received(&mut asker);
    let [(MasterPacket::Unsupported(answer), len)] = got.as_slice() else {
        panic!("{got:?}")
    };
    assert!(*len <= 1_200);
    assert_eq!((answer.lowest, answer.highest), (1, 1));
    assert!(answer.text.contains("newer"));
    assert_eq!(rig.master.counters().unsupported, 2);
}

#[test]
fn garbage_and_the_masters_own_kinds_get_nothing() {
    let mut rig = Rig::new(Settings::default());
    let mut sender = rig.net.bind(a("192.0.2.50:40000")).unwrap();
    sender.send_datagram(a(MAIN_V4), b"hello").unwrap();
    sender.send_datagram(a(MAIN_V4), &[0u8; 1_300]).unwrap();
    let challenge = MasterPacket::Challenge(tore_net::master::Challenge {
        nonce: 1,
        cookie: 2,
    });
    send(&mut sender, MAIN_V4, &challenge);
    // Anything but a Probe on the second port.
    let browse = MasterPacket::Browse(Browse {
        nonce: 1,
        build: build(),
        other_builds: false,
        full_games: false,
        cursor: 0,
    });
    send(&mut sender, PROBE_V4, &browse);
    let mut bad = MasterPacket::Keep(Keep { token: 1 }).encode().unwrap();
    bad.push(0xff);
    // The checksum covers the extra byte, so only the fields are wrong.
    let crc = tore_net::master::packet::checksum(&bad[4..]);
    bad[..4].copy_from_slice(&crc.to_le_bytes());
    sender.send_datagram(a(MAIN_V4), &bad).unwrap();
    rig.turn();
    assert!(received(&mut sender).is_empty());
    let c = rig.master.counters();
    assert_eq!((c.invalid, c.unexpected, c.malformed), (2, 2, 1));
}

fn report(install_id: u64) -> MasterPacket {
    MasterPacket::Report(Report {
        install_id,
        role: Role::Player,
        game_version: "0.1.3".into(),
        platform: 3,
        minutes: 12,
        humans: 2,
        path: Path::Punched,
        connect_tenths: 4,
        mapping: MappingType::SamePort,
        port_mapping: PortMapping::NotTried,
        relayed_kb: 0,
        players_by_path: [0; 6],
        migrations: 0,
        failed_migrations: 0,
    })
}

#[test]
fn reports_are_counted_without_answers_and_limited() {
    let mut rig = Rig::new(Settings::default());
    let mut game = rig.net.bind(a("192.0.2.50:40000")).unwrap();
    for n in 1..=7 {
        send(&mut game, MAIN_V4, &report(n));
    }
    rig.turn();
    assert!(received(&mut game).is_empty());
    // A burst of five a source, then one a minute.
    assert_eq!(rig.master.counters().reports, 5);
    let counts = rig.master.telemetry().to_tsv();
    assert!(counts.contains("installs\t5\n") && counts.contains("path.punched\t5\n"));
    // With telemetry off nothing is counted.
    let mut off = Rig::new(Settings {
        telemetry: false,
        ..Settings::default()
    });
    let mut game = off.net.bind(a("192.0.2.50:40000")).unwrap();
    send(&mut game, MAIN_V4, &report(1));
    off.turn();
    assert_eq!(off.master.counters().reports_ignored, 1);
    assert_eq!(off.master.telemetry().to_tsv(), "installs\t0\n");
}

#[test]
fn an_introduce_is_challenged_and_the_relay_dropped_and_counted_until_slice_j3() {
    let mut rig = Rig::new(Settings::default());
    let mut player = rig.net.bind(a("192.0.2.50:40000")).unwrap();
    let introduce = MasterPacket::Introduce(Introduce {
        nonce: 1,
        cookie: 0,
        listing_id: 5,
        build: build(),
        mapping: MappingType::Unknown,
        candidates: Vec::new(),
    });
    send(&mut player, MAIN_V4, &introduce);
    let frame = tore_net::master::RelayFrame {
        channel: 1,
        key: 2,
        datagram: &[1, 2, 3],
    }
    .encode()
    .unwrap();
    player.send_datagram(a(MAIN_V4), &frame).unwrap();
    rig.turn();
    // Slice J2: the Introduce without a cookie gets the Challenge alone.
    let got = received(&mut player);
    assert!(
        matches!(got[..], [(MasterPacket::Challenge(_), 23)]),
        "{got:?}"
    );
    assert_eq!(rig.master.introductions().under_way(), 0);
    assert_eq!(rig.master.relays().dropped, 1);
}

/// What a flooding source sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flood {
    Register,
    Browse,
    Details,
    Probe,
    Keep,
    Heartbeat,
    Report,
    Introduce,
    Garbage,
}

const FLOOD_KINDS: [Flood; 9] = [
    Flood::Register,
    Flood::Browse,
    Flood::Details,
    Flood::Probe,
    Flood::Keep,
    Flood::Heartbeat,
    Flood::Report,
    Flood::Introduce,
    Flood::Garbage,
];

fn flood_datagram(kind: Flood, n: u64) -> Vec<u8> {
    let game = summary("flood", 1, DiscoverPhase::Lobby);
    let packet = match kind {
        Flood::Register => MasterPacket::Register(Register {
            nonce: n,
            cookie: n % 3,
            build: build(),
            dedicated: false,
            telemetry: false,
            install_id: 0,
            platform: 3,
            candidates: Vec::new(),
            summary: game,
        }),
        Flood::Browse => MasterPacket::Browse(Browse {
            nonce: n,
            build: build(),
            other_builds: true,
            full_games: true,
            cursor: 0,
        }),
        Flood::Details => MasterPacket::Details(Details {
            nonce: n,
            listing_id: n,
        }),
        Flood::Probe => MasterPacket::Probe(Probe { nonce: n }),
        Flood::Keep => MasterPacket::Keep(Keep { token: n }),
        Flood::Heartbeat => MasterPacket::Heartbeat(Heartbeat {
            token: n,
            change: 0,
            candidates: Vec::new(),
            summary: game,
        }),
        Flood::Report => report(n | 1),
        Flood::Introduce => MasterPacket::Introduce(Introduce {
            nonce: n,
            cookie: 0,
            listing_id: n,
            build: build(),
            mapping: MappingType::Unknown,
            candidates: Vec::new(),
        }),
        Flood::Garbage => return vec![n as u8; 40 + (n % 900) as usize],
    };
    packet.encode().unwrap()
}

/// The most answers a source of `kind` may get in `seconds`: its burst and
/// its rate (the answer-rate cap aside).
fn flood_allowance(kind: Flood, seconds: f64) -> f64 {
    match kind {
        Flood::Register => 10.0 + seconds / 6.0,
        Flood::Browse | Flood::Details | Flood::Keep | Flood::Heartbeat => 40.0 + 20.0 * seconds,
        // An Introduce without a cookie gets its 23-byte Challenge (J2).
        Flood::Probe | Flood::Introduce => 4.0 + 4.0 * seconds,
        Flood::Report | Flood::Garbage => 0.0,
    }
}

#[test]
fn a_flood_from_1000_sources_gets_no_more_than_it_sent_and_a_browser_is_still_answered() {
    // An answer rate the flood exceeds, so the total cap is tested too.
    let answer_rate = 1_000;
    let mut rig = Rig::new(Settings {
        answer_rate,
        ..Settings::default()
    });
    let mut host = Host::new(&rig, "192.0.2.10:26900", 1);
    let game = summary("Real game", 2, DiscoverPhase::Lobby);
    let listed = host.list(&mut rig, &game).unwrap();
    let mut asker = Asker::new(&rig, "192.0.2.50:40000", false, false);
    asker.browser.refresh(rig.now());
    // 1,000 sources, each sending one kind 50 times a second for 4 seconds.
    let sources: Vec<(SocketAddr, Flood)> = (0..1_000u32)
        .map(|i| {
            let ip = Ipv4Addr::from(0x0a00_0000 + i * 7 + 1);
            (
                SocketAddr::new(ip.into(), 50_000),
                FLOOD_KINDS[i as usize % FLOOD_KINDS.len()],
            )
        })
        .collect();
    let templates: Vec<Vec<Vec<u8>>> = FLOOD_KINDS
        .iter()
        .map(|kind| (1..=8).map(|n| flood_datagram(*kind, n)).collect())
        .collect();
    let mut sent: BTreeMap<SocketAddr, u64> = BTreeMap::new();
    let mut answered: BTreeMap<SocketAddr, (u64, u64)> = BTreeMap::new();
    let mut browses = 0u32;
    let mut refreshed = 0u32;
    let mut total = 0u64;
    let start = rig.now();
    let seconds = 4.0;
    rig.net.start_trace();
    let mut round = 0u64;
    while rig.now() < start + Duration::from_secs_f64(seconds) {
        for (i, (from, kind)) in sources.iter().enumerate() {
            let k = i % FLOOD_KINDS.len();
            let datagram = &templates[k][(round as usize + i) % 8];
            let to = if *kind == Flood::Probe && i % 2 == 0 {
                PROBE_V4
            } else {
                MAIN_V4
            };
            rig.net.inject(*from, a(to), datagram);
            *sent.entry(*from).or_default() += datagram.len() as u64;
        }
        round += 1;
        // The proper browser asks again every second.
        if round.is_multiple_of(50) {
            asker.browser.refresh(rig.now());
        }
        asker.browser.transmit(&mut asker.socket).unwrap();
        rig.step(Duration::from_millis(20));
        asker
            .browser
            .receive_from(&mut asker.socket, rig.now())
            .unwrap();
        asker.browser.update(rig.now());
        while let Some(event) = asker.browser.poll_event() {
            if let BrowseEvent::Refreshed { .. } = event {
                refreshed += 1;
            }
        }
        for entry in rig.net.take_trace() {
            if entry.from.port() == 26901 || entry.from.port() == 26902 {
                total += 1;
                let got = answered.entry(entry.to).or_default();
                got.0 += 1;
                got.1 += entry.datagram.len() as u64;
            } else if entry.from == a("192.0.2.50:40000") {
                browses += 1;
            }
        }
    }
    // Every flooding source got at most what it sent, and within its limit.
    for (from, kind) in &sources {
        let (count, bytes) = answered.get(from).copied().unwrap_or_default();
        assert!(
            bytes <= sent[from],
            "{from} ({kind:?}) got {bytes} for {}",
            sent[from]
        );
        assert!(
            count as f64 <= flood_allowance(*kind, seconds),
            "{from} ({kind:?}) got {count} answers"
        );
    }
    // The total held its rate (a second's burst and the rate after).
    assert!(
        total as f64 <= f64::from(answer_rate) * (1.0 + seconds),
        "{total} answers"
    );
    let counters = rig.master.counters();
    assert!(counters.dropped_answers > 0, "the cap was reached");
    assert!(counters.dropped_limit > 0);
    // The proper browser was answered every time it asked, and sees the game.
    assert!(browses >= 4, "{browses}");
    assert_eq!(
        refreshed, browses,
        "{refreshed} refreshes of {browses} browses"
    );
    assert_eq!(
        asker
            .browser
            .games()
            .iter()
            .map(|g| g.listing_id)
            .collect::<Vec<_>>(),
        [listed.listing_id]
    );
    // And the host is still listed and acknowledged.
    assert_eq!(host.heartbeat(&mut rig, &game, 0).len(), 1);
    let over_logged = rig
        .logs()
        .iter()
        .filter(|l| l.starts_with("limit source=10."))
        .count();
    assert!(over_logged > 0);
}
