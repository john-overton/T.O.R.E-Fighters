//! A listing and its relay channels follow a migrated host (stage K, slice
//! K8's acceptance in the architecture guide's slice table), on the network
//! simulator with the real master.
//!
//! A host lists its game; one player joins it by an introduction (a direct,
//! punched path) and one through the relay. The host is lost, and two
//! seconds later another game takes the mission over: it is handed the old
//! host's listing part (as bytes, as the standby stream carries it) and
//! heartbeats with the listing's token from its own game port. The listing
//! keeps its id and is found at the new address; the relayed player's new
//! handshake over its old channel reaches the new host; nothing more goes to
//! the old address. A second migration within the minute waits out the
//! master's once-a-minute rule, then moves too.
//!
//! Every link is 20 ms one way.

use std::net::SocketAddr;
use std::time::Duration;

use tore_master::{Master, MasterPort, Settings};
use tore_net::master::join::{JoinConfig, JoinEvent, Joiner};
use tore_net::master::rendezvous::ListingPart;
use tore_net::master::{
    Browse, Build, Candidate, CandidateKind, HostRendezvous, ListingState, ListingSummary,
    MasterPacket, Path, Rendezvous, RendezvousEvent, channel_of, is_relayed,
};
use tore_net::packet::DiscoverPhase;
use tore_net::sim::{LinkConfig, SimNetwork, SimSocket};
use tore_net::{
    AcceptInfo, Client, ClientConfig, ClientState, ConnectDetails, Datagrams, Decision, Entropy,
    Server, ServerConfig, ServerEvent,
};

const VERSION: u16 = 13;
const MAIN: &str = "198.51.100.1:26901";
const PROBE: &str = "198.51.100.1:26902";
const OLD_HOST: &str = "203.0.113.10:26900";
const NEW_HOST: &str = "203.0.113.30:26900";
const THIRD_HOST: &str = "203.0.113.40:26900";
const PUNCHED: &str = "192.0.2.40:40000";
const RELAYED: &str = "192.0.2.50:40000";
const BROWSER: &str = "192.0.2.99:5000";
const STEP: Duration = Duration::from_millis(5);
const LINK: Duration = Duration::from_millis(20);

fn a(text: &str) -> SocketAddr {
    text.parse().unwrap()
}

fn build() -> Build {
    Build {
        protocol_version: VERSION,
        game_version: "0.1.3".into(),
        game_commit: "abc".into(),
        release: true,
    }
}

fn host_config(seed: u64) -> HostRendezvous {
    HostRendezvous {
        build: build(),
        dedicated: false,
        install_id: None,
        platform: 3,
        entropy: Entropy::Seeded(seed),
    }
}

fn summary() -> ListingSummary {
    ListingSummary {
        protocol_version: VERSION,
        players: 3,
        capacity: 8,
        phase: DiscoverPhase::Flying,
        game_version: "0.1.3".into(),
        game_commit: "abc".into(),
        name: "Friday night".into(),
        ..ListingSummary::default()
    }
}

fn gate(_: &ConnectDetails) -> Decision {
    Decision::Accept(AcceptInfo {
        session_id: 1,
        ticks_per_second: 120,
        ticks_per_snapshot: 4,
        host_tick: 0,
    })
}

/// A hosting game: its transport and its listing on one socket.
struct Host {
    socket: SimSocket,
    server: Server,
    rendezvous: Rendezvous,
    /// Who connected, and along which path.
    connected: Vec<ConnectDetails>,
    listed: Vec<RendezvousEvent>,
    /// Stopped: the game is lost and its socket only listens.
    lost: bool,
    /// What reached the socket once lost: when, and from where.
    heard_after_loss: Vec<(Duration, SocketAddr)>,
}

impl Host {
    fn new(socket: SimSocket, rendezvous: Rendezvous, seed: u64) -> Self {
        Self {
            socket,
            server: Server::new(ServerConfig {
                entropy: Entropy::Seeded(seed),
                ..ServerConfig::new(VERSION)
            }),
            rendezvous,
            connected: Vec::new(),
            listed: Vec::new(),
            lost: false,
            heard_after_loss: Vec::new(),
        }
    }

    fn turn(&mut self, now: Duration) {
        if self.lost {
            let mut buf = [0u8; 2048];
            while let Some((_, from)) = self.socket.recv_datagram(&mut buf).unwrap() {
                self.heard_after_loss.push((now, from));
            }
            return;
        }
        let mut gate = gate;
        self.server
            .receive_from(
                &mut self.rendezvous.over(&mut self.socket, now),
                now,
                &mut gate,
            )
            .unwrap();
        self.server.update(now);
        if self.rendezvous.wants_summary(now) {
            self.rendezvous.set_summary(now, summary());
        }
        self.rendezvous.update(now);
        self.server
            .transmit(&mut self.rendezvous.over(&mut self.socket, now))
            .unwrap();
        self.rendezvous.transmit(&mut self.socket).unwrap();
        while let Some(event) = self.server.poll_event() {
            if let ServerEvent::Connected { details, .. } = event {
                self.connected.push(details);
            }
        }
        while let Some(event) = self.rendezvous.poll_event() {
            self.listed.push(event);
        }
    }

    fn listing_id(&self) -> Option<u64> {
        match self.rendezvous.state() {
            ListingState::Listed { listing_id, .. } => Some(listing_id),
            _ => None,
        }
    }
}

/// A player: its socket, its join through the master and its connection.
struct Player {
    socket: SimSocket,
    joiner: Joiner,
    client: Option<Client>,
    /// Asks for the relay at once rather than racing the host's addresses.
    relay: bool,
    events: Vec<JoinEvent>,
    seed: u64,
}

impl Player {
    fn new(net: &SimNetwork, address: &str, listing_id: u64, relay: bool, seed: u64) -> Self {
        Self {
            socket: net.bind(a(address)).unwrap(),
            joiner: joiner(listing_id, address, seed, net.now()),
            client: None,
            relay,
            events: Vec::new(),
            seed,
        }
    }

    fn config(&self) -> ClientConfig {
        ClientConfig {
            entropy: Entropy::Seeded(self.seed),
            ..ClientConfig::new(VERSION, if self.relay { "Hawk" } else { "Viper" })
        }
    }

    fn turn(&mut self, now: Duration) {
        match self.client.as_mut() {
            Some(client) => {
                client
                    .receive_from(&mut self.joiner.over(&mut self.socket, now), now)
                    .unwrap();
            }
            None => {
                let mut routed = self.joiner.over(&mut self.socket, now);
                let mut buf = [0u8; 2048];
                while routed.recv_datagram(&mut buf).unwrap().is_some() {}
            }
        }
        self.joiner.update(now);
        while let Some(event) = self.joiner.poll_event() {
            match &event {
                JoinEvent::Introduced(_) if self.relay => {
                    assert!(self.joiner.ask_relay(now));
                }
                JoinEvent::Introduced(introduced) => {
                    let client = Client::connect_any(
                        self.config(),
                        &introduced.targets,
                        Some(introduced.introduction_id),
                        now,
                    )
                    .unwrap();
                    self.client = Some(client);
                }
                JoinEvent::Relayed { address } => {
                    self.client = Some(Client::connect(self.config(), *address, now).unwrap());
                }
                JoinEvent::MappingTested(_) => {}
                other => panic!("the join through the master failed: {other:?}"),
            }
            self.events.push(event);
        }
        if let Some(client) = self.client.as_mut() {
            client.update(now);
            client
                .transmit(&mut self.joiner.over(&mut self.socket, now))
                .unwrap();
        }
        self.joiner.transmit(&mut self.socket).unwrap();
    }

    fn connected(&self) -> bool {
        self.client
            .as_ref()
            .is_some_and(|c| c.state() == ClientState::Connected)
    }
}

fn joiner(listing_id: u64, address: &str, seed: u64, now: Duration) -> Joiner {
    let mut joiner = Joiner::new(
        JoinConfig {
            build: build(),
            listing_id,
            entropy: Entropy::Seeded(seed),
        },
        now,
    );
    joiner.set_masters(
        vec![a(MAIN)],
        vec![Candidate::new(CandidateKind::Local, a(address))],
        now,
    );
    joiner
}

struct Rig {
    net: SimNetwork,
    master: Master,
    main: SimSocket,
    probe: SimSocket,
    browser: SimSocket,
    hosts: Vec<Host>,
    players: Vec<Player>,
    /// The master's log lines, with when.
    log: Vec<(Duration, String)>,
}

impl Rig {
    fn new() -> Self {
        let net = SimNetwork::new(29);
        net.set_default_link(LinkConfig::one_way(LINK));
        net.set_now(Duration::from_secs(100));
        let main = net.bind(a(MAIN)).unwrap();
        let probe = net.bind(a(PROBE)).unwrap();
        let browser = net.bind(a(BROWSER)).unwrap();
        Self {
            master: Master::new(Settings::default(), Entropy::Seeded(11), 0),
            main,
            probe,
            browser,
            hosts: Vec::new(),
            players: Vec::new(),
            log: Vec::new(),
            net,
        }
    }

    fn now(&self) -> Duration {
        self.net.now()
    }

    fn step(&mut self) {
        self.net.advance(STEP);
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
        while let Some(line) = self.master.poll_log() {
            self.log.push((now, line));
        }
        for host in &mut self.hosts {
            host.turn(now);
        }
        for player in &mut self.players {
            player.turn(now);
        }
    }

    fn run_until(&mut self, limit: Duration, mut done: impl FnMut(&Rig) -> bool) -> bool {
        let end = self.now() + limit;
        while self.now() < end {
            self.step();
            if done(self) {
                return true;
            }
        }
        false
    }

    fn run(&mut self, time: Duration) {
        let end = self.now() + time;
        while self.now() < end {
            self.step();
        }
    }

    /// When the master logged a line that starts with `prefix`, the last
    /// time.
    fn logged(&self, prefix: &str) -> Option<(Duration, String)> {
        self.log
            .iter()
            .rev()
            .find(|(_, line)| line.starts_with(prefix))
            .cloned()
    }

    /// The listing ids and names a fresh Browse finds.
    fn browse(&mut self) -> Vec<(u64, u8)> {
        let request = MasterPacket::Browse(Browse {
            nonce: 77,
            build: build(),
            other_builds: false,
            full_games: false,
            cursor: 0,
        })
        .encode()
        .unwrap();
        self.browser.send_datagram(a(MAIN), &request).unwrap();
        self.run(LINK * 3);
        let mut buf = [0u8; 2048];
        let (length, _) = self
            .browser
            .recv_datagram(&mut buf)
            .unwrap()
            .expect("a Page");
        let MasterPacket::Page(page) = MasterPacket::decode(&buf[..length]).unwrap() else {
            panic!("not a Page");
        };
        page.entries
            .iter()
            .map(|e| (e.listing_id, e.players))
            .collect()
    }
}

/// The game that takes the mission over: handed `part` as the stream's
/// bytes, it resumes the listing on its own socket at `address`.
fn take_over(net: &SimNetwork, part: &ListingPart, address: &str, seed: u64) -> Host {
    let bytes = part.encode();
    let part = ListingPart::decode(&bytes).expect("the part's bytes decode");
    let rendezvous = Rendezvous::resume(
        host_config(seed),
        &part,
        vec![Candidate::new(CandidateKind::Local, a(address))],
        net.now(),
    );
    Host::new(net.bind(a(address)).unwrap(), rendezvous, seed + 100)
}

#[test]
fn the_listing_and_its_relay_channel_follow_the_host_to_another_game() {
    let mut rig = Rig::new();
    // The old host lists its game.
    let mut rendezvous = Rendezvous::host(host_config(3), rig.now());
    rendezvous.set_masters(
        vec![a(MAIN)],
        vec![Candidate::new(CandidateKind::Local, a(OLD_HOST))],
        rig.now(),
    );
    rendezvous.set_listed(true, rig.now());
    let socket = rig.net.bind(a(OLD_HOST)).unwrap();
    rig.hosts.push(Host::new(socket, rendezvous, 2));
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.hosts[0].listing_id().is_some()
    }));
    let listing_id = rig.hosts[0].listing_id().unwrap();

    // A punched player and a relayed one join it.
    let punched = Player::new(&rig.net, PUNCHED, listing_id, false, 5);
    let relayed = Player::new(&rig.net, RELAYED, listing_id, true, 6);
    rig.players.push(punched);
    rig.players.push(relayed);
    assert!(
        rig.run_until(Duration::from_secs(5), |r| r
            .players
            .iter()
            .all(Player::connected)),
        "{:?} {:?}",
        rig.players[0].events,
        rig.players[1].events
    );
    let paths: Vec<Path> = rig.hosts[0].connected.iter().map(|d| d.path).collect();
    assert_eq!(paths, [Path::Punched, Path::Relay]);
    let channel_address = rig.players[1].joiner.relayed().unwrap();
    assert!(is_relayed(channel_address));
    assert_eq!(rig.hosts[0].rendezvous.relayed(), [channel_address]);
    rig.run(Duration::from_secs(1));

    // The old host's listing part, as the standby stream carries it.
    let mut part = rig.hosts[0].rendezvous.listing_part().unwrap();
    part.master_name = MAIN.into();
    assert_eq!(part.listing_id, listing_id);
    assert_eq!(part.master, a(MAIN));
    assert_eq!(
        part.channels.iter().map(|c| c.channel).collect::<Vec<_>>(),
        [channel_of(channel_address).unwrap()]
    );

    // The old host is lost. The punched player's connection goes with it;
    // the relayed player's game keeps its channel and its connection's
    // keepalives still go through it.
    rig.hosts[0].lost = true;
    rig.players[0].client = None;
    let lost_at = rig.now();
    rig.run(Duration::from_secs(2));
    let frames_before_move = rig.hosts[0]
        .heard_after_loss
        .iter()
        .filter(|(_, from)| *from == a(MAIN))
        .count();
    assert!(
        frames_before_move > 0,
        "the relayed player's frames reached the dead host until the move"
    );
    assert_eq!(
        rig.master.listings().get(listing_id).unwrap().address,
        a(OLD_HOST)
    );

    // Two seconds on, another game takes over with the part.
    let new_host = take_over(&rig.net, &part, NEW_HOST, 13);
    rig.hosts.push(new_host);
    assert_eq!(rig.hosts[1].rendezvous.state(), ListingState::Registering);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.hosts[1].listing_id().is_some()
    }));
    let moved_at = rig
        .logged("moved id=")
        .expect("the master moved the listing")
        .0;
    assert!(moved_at - lost_at < Duration::from_millis(2_100));
    // The listing keeps its id, at the new address, and its channel went
    // with it.
    assert_eq!(rig.hosts[1].listing_id(), Some(listing_id));
    let listed: Vec<_> = rig.hosts[1]
        .listed
        .iter()
        .filter(|e| !matches!(e, RendezvousEvent::MappingTested(_)))
        .collect();
    assert_eq!(
        listed,
        [&RendezvousEvent::Listed {
            listing_id,
            seen: a(NEW_HOST)
        }]
    );
    assert_eq!(
        rig.master.listings().get(listing_id).unwrap().address,
        a(NEW_HOST)
    );
    let (_, line) = rig.logged("relay moved ").expect("the channel moved");
    assert_eq!(
        line,
        format!("relay moved listing={listing_id:016x} from={OLD_HOST} to={NEW_HOST} channels=1")
    );
    assert_eq!(rig.master.relays().counters.moved, 1);
    assert_eq!(rig.master.relays().channels(), 1);

    // The next browse shows one game, the same listing.
    assert_eq!(rig.browse(), [(listing_id, 3)]);

    // The relayed player's game joins again over its old channel, from the
    // same socket, and its handshake reaches the new host.
    let now = rig.now();
    let player = &mut rig.players[1];
    let target = player.joiner.relay_target().expect("the channel is kept");
    assert_eq!(
        (target.address, target.path),
        (channel_address, Path::Relay)
    );
    player.seed = 16;
    player.client = Some(Client::connect_any(player.config(), &[target], None, now).unwrap());
    assert!(rig.run_until(Duration::from_secs(2), |r| r.players[1].connected()));
    let joined = rig.hosts[1].connected.last().expect("the relayed player");
    assert_eq!(
        (joined.address, joined.path, joined.callsign.as_str()),
        (channel_address, Path::Relay, "Hawk")
    );

    // The punched player's new introduction finds the new host first.
    let now = rig.now();
    let player = &mut rig.players[0];
    player.joiner = joiner(listing_id, PUNCHED, 25, now);
    player.seed = 15;
    assert!(rig.run_until(Duration::from_secs(3), |r| r.players[0].connected()));
    let introduced = rig.players[0]
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            JoinEvent::Introduced(introduced) => Some(introduced.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(introduced.targets[0].address, a(NEW_HOST));
    assert_eq!(
        rig.players[0].client.as_ref().unwrap().server(),
        a(NEW_HOST)
    );
    assert_eq!(
        rig.hosts[1].connected.last().map(|d| d.path),
        Some(Path::Punched)
    );

    // Nothing from the master reached the old address once the move was
    // made (what was already on the link aside).
    let late: Vec<_> = rig.hosts[0]
        .heard_after_loss
        .iter()
        .filter(|(at, from)| *from == a(MAIN) && *at > moved_at + LINK + STEP)
        .collect();
    assert!(late.is_empty(), "{late:?}");
}

#[test]
fn a_second_migration_within_the_minute_waits_for_it() {
    let mut rig = Rig::new();
    let mut rendezvous = Rendezvous::host(host_config(3), rig.now());
    rendezvous.set_masters(
        vec![a(MAIN)],
        vec![Candidate::new(CandidateKind::Local, a(OLD_HOST))],
        rig.now(),
    );
    rendezvous.set_listed(true, rig.now());
    let socket = rig.net.bind(a(OLD_HOST)).unwrap();
    rig.hosts.push(Host::new(socket, rendezvous, 2));
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.hosts[0].listing_id().is_some()
    }));
    let listing_id = rig.hosts[0].listing_id().unwrap();

    // The first migration moves the listing at once.
    let mut part = rig.hosts[0].rendezvous.listing_part().unwrap();
    part.master_name = MAIN.into();
    rig.hosts[0].lost = true;
    let new_host = take_over(&rig.net, &part, NEW_HOST, 13);
    rig.hosts.push(new_host);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.hosts[1].listing_id().is_some()
    }));
    let first_move = rig.logged("moved id=").unwrap().0;

    // Ten seconds on the new host is lost too, and a third game takes over.
    rig.run(Duration::from_secs(10));
    let mut part = rig.hosts[1].rendezvous.listing_part().unwrap();
    part.master_name = MAIN.into();
    rig.hosts[1].lost = true;
    let third = take_over(&rig.net, &part, THIRD_HOST, 23);
    rig.hosts.push(third);
    let dropped = rig.master.counters().dropped_limit;
    // Within the minute the listing stays at the dead address, and the
    // third game keeps asking.
    let moved = rig.run_until(Duration::from_secs(70), |r| {
        r.master.listings().get(listing_id).unwrap().address == a(THIRD_HOST)
    });
    assert!(moved, "the listing moved once the minute was up");
    let second_move = rig.now() - first_move;
    assert!(
        (Duration::from_secs(60)..=Duration::from_secs(64)).contains(&second_move),
        "{second_move:?}"
    );
    assert!(rig.master.counters().dropped_limit > dropped + 10);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        r.hosts[2].listing_id().is_some()
    }));
    assert_eq!(rig.hosts[2].listing_id(), Some(listing_id));
    assert_eq!(
        rig.hosts[2].rendezvous.state(),
        ListingState::Listed {
            listing_id,
            seen: a(THIRD_HOST)
        }
    );
    assert_eq!(rig.browse().len(), 1);
}
