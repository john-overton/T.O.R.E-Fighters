//! The punching table on the network simulator (slice J2's acceptance,
//! "Hole punching" in the architecture guide): the real master, a host that
//! lists itself through its rendezvous and answers Meets with punches, and a
//! player that asks for an introduction and races the host's addresses with
//! `Client::connect_any`, each behind the routers of one row.
//!
//! Every link has a 100 ms round trip. A row that punches connects within
//! 1.5 seconds of the player starting its join, along the row's path. A row
//! whose expected path is the relay finds no direct path within the 3
//! seconds the game races before it asks for the relay (slice J3 then
//! carries it).

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use tore_master::{Master, MasterPort, Settings};
use tore_net::master::join::{JoinConfig, JoinEvent, Joiner};
use tore_net::master::{
    Build, Candidate, CandidateKind, Hint, HostRendezvous, ListingState, ListingSummary, Path,
    RACE_BEFORE_RELAY, Rendezvous,
};
use tore_net::packet::DiscoverPhase;
use tore_net::sim::{
    Filtering, Forward, LinkConfig, Mapping, Prefix, RouterConfig, RouterId, SimNetwork, SimSocket,
};
use tore_net::{
    AcceptInfo, Client, ClientConfig, ClientState, ConnectDetails, Decision, Entropy, Server,
    ServerConfig, ServerEvent,
};

const VERSION: u16 = 9;
const MAIN_V4: &str = "198.51.100.1:26901";
const MAIN_V6: &str = "[2001:db8:ffff::1]:26901";
const PROBE_V4: &str = "198.51.100.1:26902";
const PROBE_V6: &str = "[2001:db8:ffff::1]:26902";
const STEP: Duration = Duration::from_millis(5);

fn a(text: &str) -> SocketAddr {
    text.parse().unwrap()
}

fn ip(text: &str) -> IpAddr {
    text.parse().unwrap()
}

fn prefix(text: &str) -> Prefix {
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

/// A home router: how it maps and filters.
fn router(outside: &str, inside: &str, mapping: Mapping, filtering: Filtering) -> RouterConfig {
    RouterConfig {
        mapping,
        filtering,
        ..RouterConfig::nat(ip(outside), prefix(inside))
    }
}

const EIM: Mapping = Mapping::EndpointIndependent;
const SYMMETRIC: Mapping = Mapping::AddressAndPortDependent;
const OPEN: Filtering = Filtering::EndpointIndependent;
const ADF: Filtering = Filtering::AddressDependent;
const APDF: Filtering = Filtering::AddressAndPortDependent;

/// Where one end's socket sits.
struct End {
    v4: &'static str,
    v6: Option<&'static str>,
    /// Its own candidates beyond the Local IPv4 (a Mapped or Global IPv6).
    extra: Vec<Candidate>,
}

impl End {
    fn v4(address: &'static str) -> Self {
        Self {
            v4: address,
            v6: None,
            extra: Vec::new(),
        }
    }

    fn bind(&self, net: &SimNetwork, behind: Option<RouterId>) -> SimSocket {
        match (self.v6, behind) {
            (Some(v6), _) => net.bind_dual(a(self.v4), a(v6)).unwrap(),
            (None, Some(router)) => net.bind_behind(router, a(self.v4)).unwrap(),
            (None, None) => net.bind(a(self.v4)).unwrap(),
        }
    }

    fn candidates(&self) -> Vec<Candidate> {
        let mut list = vec![Candidate::new(CandidateKind::Local, a(self.v4))];
        if let Some(v6) = self.v6 {
            list.push(Candidate::new(CandidateKind::GlobalIpv6, a(v6)));
        }
        list.extend(self.extra.iter().copied());
        list
    }
}

/// How a row ended.
#[derive(Debug)]
struct Outcome {
    /// The path of the address the race chose, or `None` when no direct
    /// path answered within the race's 3 seconds.
    path: Option<Path>,
    /// From the player starting its join to connected.
    took: Option<Duration>,
    hint: Hint,
    /// The address the race chose.
    chosen: Option<SocketAddr>,
    /// The host's address as the master saw it.
    seen: SocketAddr,
    /// The path the host's transport was told.
    host_path: Option<Path>,
    punches: u64,
}

struct Row {
    net: SimNetwork,
    host: End,
    host_behind: Option<RouterId>,
    player: End,
    player_behind: Option<RouterId>,
}

impl Row {
    fn new() -> Self {
        let net = SimNetwork::new(17);
        net.set_default_link(LinkConfig::one_way(Duration::from_millis(50)));
        Self {
            net,
            host: End::v4("192.168.1.10:26900"),
            host_behind: None,
            player: End::v4("192.168.2.20:40000"),
            player_behind: None,
        }
    }

    fn add(&self, config: RouterConfig) -> RouterId {
        self.net.add_router(config).unwrap()
    }

    /// Lists the host, introduces the player and runs the race.
    fn run(self) -> Outcome {
        let net = self.net;
        net.set_now(Duration::from_secs(100));
        let mut master = Master::new(Settings::default(), Entropy::Seeded(11), 0);
        let mut main = net.bind_dual(a(MAIN_V4), a(MAIN_V6)).unwrap();
        let mut probe = net.bind_dual(a(PROBE_V4), a(PROBE_V6)).unwrap();
        let masters = vec![a(MAIN_V4), a(MAIN_V6)];

        let mut host_socket = self.host.bind(&net, self.host_behind);
        let mut server = Server::new(ServerConfig {
            entropy: Entropy::Seeded(2),
            ..ServerConfig::new(VERSION)
        });
        let mut rendezvous = Rendezvous::host(
            HostRendezvous {
                build: build(),
                dedicated: true,
                install_id: None,
                platform: 3,
                entropy: Entropy::Seeded(3),
            },
            net.now(),
        );
        rendezvous.set_masters(masters.clone(), self.host.candidates(), net.now());
        rendezvous.set_listed(true, net.now());
        let summary = ListingSummary {
            protocol_version: VERSION,
            players: 0,
            capacity: 8,
            phase: DiscoverPhase::Lobby,
            game_version: "0.1.3".into(),
            game_commit: "abc".into(),
            name: "Friday night".into(),
            ..ListingSummary::default()
        };
        let mut gate = |_: &ConnectDetails| {
            Decision::Accept(AcceptInfo {
                session_id: 1,
                ticks_per_second: 120,
                ticks_per_snapshot: 4,
                host_tick: 0,
            })
        };
        let mut host_path = None;

        let mut player_socket = self.player.bind(&net, self.player_behind);
        let mut joiner: Option<Joiner> = None;
        let mut client: Option<Client> = None;
        let mut started = None;
        let mut raced = None;
        let mut connected = None;
        let mut hint = Hint::Race;
        let mut listing_id = None;

        let deadline = net.now() + Duration::from_secs(20);
        while net.now() < deadline {
            net.advance(STEP);
            let now = net.now();
            master
                .receive_from(now, MasterPort::Main, &mut main)
                .unwrap();
            master
                .receive_from(now, MasterPort::Probe, &mut probe)
                .unwrap();
            master.update(now);
            master.transmit(&mut main, Some(&mut probe)).unwrap();

            server
                .receive_from(&mut rendezvous.over(&mut host_socket, now), now, &mut gate)
                .unwrap();
            server.update(now);
            if rendezvous.wants_summary(now) {
                rendezvous.set_summary(now, summary.clone());
            }
            rendezvous.update(now);
            server
                .transmit(&mut rendezvous.over(&mut host_socket, now))
                .unwrap();
            rendezvous.transmit(&mut host_socket).unwrap();
            while let Some(event) = server.poll_event() {
                if let ServerEvent::Connected { details, .. } = event {
                    host_path = Some(details.path);
                }
            }
            if listing_id.is_none()
                && let ListingState::Listed { listing_id: id, .. } = rendezvous.state()
            {
                listing_id = Some(id);
                // The player starts a moment after the host is listed and its
                // mapping test is in.
                started = Some(now + Duration::from_millis(500));
            }

            let Some(start) = started else {
                continue;
            };
            if joiner.is_none() && now >= start {
                let mut j = Joiner::new(
                    JoinConfig {
                        build: build(),
                        listing_id: listing_id.unwrap(),
                        entropy: Entropy::Seeded(5),
                    },
                    now,
                );
                j.set_masters(masters.clone(), self.player.candidates(), now);
                joiner = Some(j);
            }
            let Some(j) = joiner.as_mut() else {
                continue;
            };
            match client.as_mut() {
                Some(c) => {
                    c.receive_from(&mut j.over(&mut player_socket, now), now)
                        .unwrap();
                }
                None => {
                    // Only the master's datagrams matter before the race.
                    let mut routed = j.over(&mut player_socket, now);
                    let mut buf = [0u8; 2048];
                    while tore_net::Datagrams::recv_datagram(&mut routed, &mut buf)
                        .unwrap()
                        .is_some()
                    {}
                }
            }
            j.update(now);
            while let Some(event) = j.poll_event() {
                match event {
                    JoinEvent::Introduced(introduced) => {
                        hint = introduced.hint;
                        raced = Some(now);
                        let config = ClientConfig {
                            entropy: Entropy::Seeded(6),
                            ..ClientConfig::new(VERSION, "Viper")
                        };
                        client = Some(
                            Client::connect_any(
                                config,
                                &introduced.targets,
                                Some(introduced.introduction_id),
                                now,
                            )
                            .unwrap(),
                        );
                    }
                    JoinEvent::MappingTested(_) => {}
                    other => panic!("the join through the master failed: {other:?}"),
                }
            }
            if let Some(c) = client.as_mut() {
                c.update(now);
                c.transmit(&mut j.over(&mut player_socket, now)).unwrap();
                if connected.is_none() && c.state() == ClientState::Connected {
                    connected = Some(now);
                }
            }
            j.transmit(&mut player_socket).unwrap();
            let race_over = raced.is_some_and(|at| now >= at + RACE_BEFORE_RELAY);
            if (connected.is_some() && host_path.is_some()) || race_over {
                break;
            }
        }
        let seen = match rendezvous.state() {
            ListingState::Listed { seen, .. } => seen,
            other => panic!("the host is not listed: {other:?}"),
        };
        let client = client.expect("the master introduced the player");
        let chosen_and_connected = connected.is_some();
        Outcome {
            path: chosen_and_connected.then(|| client.path()),
            took: connected.map(|at| at - started.unwrap()),
            hint,
            chosen: chosen_and_connected.then(|| client.server()),
            seen,
            host_path,
            punches: rendezvous.counters.punches,
        }
    }
}

/// A row that punches: connected along `path` within 1.5 seconds.
fn punched(outcome: &Outcome, path: Path) {
    assert_eq!(outcome.path, Some(path), "{outcome:?}");
    assert_eq!(outcome.host_path, Some(path), "{outcome:?}");
    assert!(
        outcome.took.unwrap() <= Duration::from_millis(1500),
        "{outcome:?}"
    );
    assert!(outcome.punches > 0, "{outcome:?}");
}

/// A row whose path is the relay: no direct path in the race's 3 seconds.
fn relay(outcome: &Outcome) {
    assert_eq!(outcome.path, None, "{outcome:?}");
}

#[test]
fn a_host_with_no_router_is_reached_by_any_player() {
    let mut row = Row::new();
    row.host = End::v4("198.51.100.10:26900");
    row.add(router("203.0.113.20", "192.168.2.0/24", SYMMETRIC, APDF));
    let outcome = row.run();
    punched(&outcome, Path::Punched);
    assert_eq!(outcome.chosen, Some(a("198.51.100.10:26900")));
}

#[test]
fn a_mapped_port_is_reached_as_the_mapped_port() {
    let mut row = Row::new();
    let mut home = router("203.0.113.10", "192.168.1.0/24", EIM, APDF);
    home.forwards.push(Forward {
        outside_port: 26900,
        inside: a("192.168.1.10:26900"),
    });
    row.add(home);
    row.host.extra = vec![Candidate::new(
        CandidateKind::Mapped,
        a("203.0.113.10:26900"),
    )];
    row.add(router("203.0.113.20", "192.168.2.0/24", SYMMETRIC, APDF));
    let outcome = row.run();
    punched(&outcome, Path::MappedPort);
    assert_eq!(outcome.seen, a("203.0.113.10:26900"));
}

#[test]
fn two_routers_that_keep_one_port_punch_whatever_they_filter() {
    let row = Row::new();
    row.add(router("203.0.113.10", "192.168.1.0/24", EIM, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", EIM, APDF));
    punched(&row.run(), Path::Punched);
}

#[test]
fn a_host_filtering_by_address_only_punches_to_a_symmetric_player() {
    for filtering in [ADF, OPEN] {
        let row = Row::new();
        row.add(router("203.0.113.10", "192.168.1.0/24", EIM, filtering));
        row.add(router("203.0.113.20", "192.168.2.0/24", SYMMETRIC, APDF));
        punched(&row.run(), Path::Punched);
    }
}

#[test]
fn a_host_filtering_by_port_and_a_symmetric_player_need_the_relay() {
    let row = Row::new();
    row.add(router("203.0.113.10", "192.168.1.0/24", EIM, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", SYMMETRIC, APDF));
    let outcome = row.run();
    relay(&outcome);
    assert_eq!(outcome.hint, Hint::Race);
}

#[test]
fn a_symmetric_host_is_found_at_the_port_its_punch_came_from() {
    for filtering in [ADF, OPEN] {
        let row = Row::new();
        row.add(router("203.0.113.10", "192.168.1.0/24", SYMMETRIC, APDF));
        row.add(router("203.0.113.20", "192.168.2.0/24", EIM, filtering));
        let outcome = row.run();
        punched(&outcome, Path::Punched);
        // Not the port the master saw: the one the host's router gave its
        // punches to the player.
        let chosen = outcome.chosen.unwrap();
        assert_eq!(chosen.ip(), outcome.seen.ip());
        assert_ne!(chosen.port(), outcome.seen.port(), "{outcome:?}");
    }
}

#[test]
fn a_symmetric_host_and_a_player_filtering_by_port_need_the_relay() {
    let row = Row::new();
    row.add(router("203.0.113.10", "192.168.1.0/24", SYMMETRIC, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", EIM, APDF));
    relay(&row.run());
}

#[test]
fn two_symmetric_routers_are_told_to_ask_for_the_relay_at_once() {
    let row = Row::new();
    row.add(router("203.0.113.10", "192.168.1.0/24", SYMMETRIC, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", SYMMETRIC, APDF));
    let outcome = row.run();
    relay(&outcome);
    assert_eq!(outcome.hint, Hint::RelayNow);
}

#[test]
fn a_host_behind_two_routers_that_keep_one_port_punches() {
    let row = Row::new();
    row.add(router("203.0.113.40", "10.0.0.0/8", EIM, APDF));
    row.add(router("10.0.0.2", "192.168.1.0/24", EIM, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", EIM, APDF));
    let outcome = row.run();
    punched(&outcome, Path::Punched);
    assert_eq!(outcome.seen.ip(), ip("203.0.113.40"));
}

#[test]
fn a_host_and_a_player_at_one_home_meet_on_the_local_network() {
    let mut row = Row::new();
    row.add(router("203.0.113.10", "192.168.1.0/24", EIM, APDF));
    row.player = End::v4("192.168.1.20:40000");
    // The home network is faster than the way out and back in.
    row.net.set_link_both(
        a("192.168.1.10:26900"),
        a("192.168.1.20:40000"),
        LinkConfig::one_way(Duration::from_millis(1)),
    );
    let outcome = row.run();
    punched(&outcome, Path::LocalNetwork);
    assert_eq!(outcome.chosen, Some(a("192.168.1.10:26900")));
}

#[test]
fn a_carrier_router_with_no_ipv6_and_a_player_filtering_by_port_need_the_relay() {
    let row = Row::new();
    row.add(router("203.0.113.50", "100.64.0.0/10", SYMMETRIC, APDF));
    row.add(router("100.64.0.5", "192.168.1.0/24", EIM, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", EIM, APDF));
    let outcome = row.run();
    relay(&outcome);
    assert_eq!(outcome.hint, Hint::Race);
}

#[test]
fn ipv6_behind_two_firewalls_is_punched_through_both() {
    let mut row = Row::new();
    // IPv4 is no way in (a symmetric host, a player filtering by port), so
    // only IPv6 can connect.
    row.add(router("203.0.113.10", "192.168.1.0/24", SYMMETRIC, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", EIM, APDF));
    row.add(RouterConfig::firewall(prefix("2001:db8:1::/48")));
    row.add(RouterConfig::firewall(prefix("2001:db8:2::/48")));
    row.host.v6 = Some("[2001:db8:1::10]:26900");
    row.player.v6 = Some("[2001:db8:2::20]:40000");
    let outcome = row.run();
    punched(&outcome, Path::Ipv6);
    assert_eq!(outcome.chosen, Some(a("[2001:db8:1::10]:26900")));
}
