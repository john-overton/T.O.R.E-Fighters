//! The peers router (slice K6): routing, the reach answerer and its limits,
//! and reach tests along every row of the punching table
//! (docs/ARCHITECTURE.md, "Hole punching") on the NAT simulator: a
//! candidate behind the table's host router opens its router to a player
//! behind the table's player router, and the player reaches the candidate
//! exactly on the rows that punch.

use super::*;
use crate::packet::{ConnectRequest, Packet, Reach, ReachAnswer};
use crate::sim::nat::{Filtering, Forward, Mapping, Prefix, RouterConfig};
use crate::sim::{LinkConfig, SimNetwork, SimSocket};

const VERSION: u16 = 13;
const SESSION: u64 = 0x5e55;
const MS: Duration = Duration::from_millis(1);

fn a(text: &str) -> SocketAddr {
    text.parse().unwrap()
}

fn reach(session: u64, nonce: u64, from: u8) -> Vec<u8> {
    Packet::Reach(Reach {
        session_id: session,
        nonce,
        from,
    })
    .encode(VERSION)
    .unwrap()
}

fn peers(seed: u64, me: u8) -> Peers {
    let mut peers = Peers::new(VERSION, Entropy::Seeded(seed));
    peers.set_session(Some(SESSION), me);
    peers
}

#[test]
fn a_reach_for_the_session_is_answered_never_longer_and_others_are_counted() {
    let mut p = peers(1, 3);
    let asker = a("10.0.0.9:5000");
    let datagram = reach(SESSION, 77, 1);
    assert_eq!(p.route(MS, asker, &datagram), Route::Taken);
    let answer = p.poll_transmit().expect("an answer");
    assert_eq!(answer.to, asker);
    assert!(answer.datagram.len() <= datagram.len());
    let Ok(Packet::ReachAnswer(ReachAnswer {
        nonce,
        session_id,
        role,
    })) = Packet::decode(&answer.datagram, VERSION)
    else {
        panic!("a Reach answer");
    };
    assert_eq!(
        (nonce, session_id, role),
        (77, SESSION, ReachRole::NotHosting)
    );
    // Another session's Reach, and one to a game in no session, go
    // unanswered.
    assert_eq!(p.route(MS, asker, &reach(SESSION + 1, 1, 1)), Route::Taken);
    let mut none = Peers::new(VERSION, Entropy::Seeded(2));
    assert_eq!(none.route(MS, asker, &reach(SESSION, 1, 1)), Route::Taken);
    assert!(p.poll_transmit().is_none() && none.poll_transmit().is_none());
    assert_eq!(p.counters().foreign, 1);
    assert_eq!(none.counters().foreign, 1);
    // An answer to nothing this game sent is counted.
    let stray = Packet::ReachAnswer(ReachAnswer {
        nonce: 5,
        session_id: SESSION,
        role: ReachRole::Hosting,
    })
    .encode(VERSION)
    .unwrap();
    assert_eq!(p.route(MS, asker, &stray), Route::Taken);
    assert_eq!(p.counters().unexpected_answers, 1);
}

#[test]
fn reaches_are_answered_ten_a_second_from_one_address() {
    let mut p = peers(1, 0);
    let asker = a("10.0.0.9:5000");
    for nonce in 0..15 {
        p.route(
            Duration::from_millis(100 + nonce),
            asker,
            &reach(SESSION, nonce, 1),
        );
    }
    let answered = std::iter::from_fn(|| p.poll_transmit()).count();
    assert_eq!(answered, 10);
    assert_eq!(p.counters().rate_limited, 5);
    // Another address, and the next second, are answered.
    p.route(
        Duration::from_millis(200),
        a("10.0.0.8:5000"),
        &reach(SESSION, 1, 1),
    );
    p.route(Duration::from_millis(1_100), asker, &reach(SESSION, 2, 1));
    assert_eq!(std::iter::from_fn(|| p.poll_transmit()).count(), 2);
}

#[test]
fn a_player_drops_what_only_a_host_takes_and_a_host_takes_everything() {
    let mut p = peers(1, 0);
    let from = a("10.0.0.9:5000");
    let request = Packet::ConnectRequest(ConnectRequest {
        protocol_version: VERSION,
        nonce: 1,
        game_version: "0.1.3".into(),
        game_commit: "abc".into(),
    })
    .encode(VERSION)
    .unwrap();
    assert_eq!(p.route(MS, from, &request), Route::Taken);
    assert_eq!(p.counters().dropped, 1);
    assert_eq!(p.route(MS, from, b"anything else"), Route::Client);
    let old_host = a("10.0.0.1:26900");
    p.set_hosting(true, Some(old_host));
    assert_eq!(p.route(MS, from, &request), Route::Host);
    assert_eq!(p.route(MS, from, &reach(SESSION, 1, 1)), Route::Host);
    assert_eq!(p.route(MS, old_host, b"anything"), Route::Client);
    assert!(p.poll_transmit().is_none(), "the host's transport answers");
}

/// Two games on one link: the player's test of the candidate reports the
/// address that answered and the median round trip.
#[test]
fn a_test_reports_the_answering_address_and_the_median_round_trip() {
    let net = SimNetwork::new(5);
    net.set_default_link(LinkConfig::one_way(Duration::from_millis(30)));
    let c_addr = a("10.0.0.3:26900");
    let p_addr = a("10.0.0.4:40000");
    let mut c = Game::new(&net, c_addr, 3, 11);
    let mut p = Game::new(&net, p_addr, 4, 12);
    let dead = a("10.0.0.99:1");
    p.peers.test(
        net.now(),
        9,
        &[
            TestTarget {
                player: 3,
                addresses: vec![dead, c_addr],
            },
            TestTarget {
                player: 7,
                addresses: vec![dead],
            },
        ],
    );
    let finished = run(&net, &mut [&mut c, &mut p], Duration::from_secs(3))
        .pop()
        .expect("the test ended");
    assert_eq!(finished.key, 9);
    let reached = finished.results[0].reached.expect("reached");
    assert_eq!(reached.0, 1, "the second address answered");
    assert!(
        reached.1 >= Duration::from_millis(60) && reached.1 <= Duration::from_millis(62),
        "{reached:?}"
    );
    assert_eq!(finished.results[1].reached, None);
}

/// One game: its socket and its router.
struct Game {
    socket: SimSocket,
    peers: Peers,
    finished: Vec<Finished>,
}

impl Game {
    fn new(net: &SimNetwork, address: SocketAddr, me: u8, seed: u64) -> Self {
        Self::on(net.bind(address).unwrap(), me, seed)
    }

    fn on(socket: SimSocket, me: u8, seed: u64) -> Self {
        Self {
            socket,
            peers: peers(seed, me),
            finished: Vec::new(),
        }
    }

    fn step(&mut self, now: Duration) {
        let mut buf = [0u8; 1500];
        while let Ok(Some((len, from))) = self.socket.recv_datagram(&mut buf) {
            self.peers.route(now, from, &buf[..len]);
        }
        self.peers.update(now);
        self.peers.transmit(&mut self.socket).unwrap();
        while let Some(f) = self.peers.poll_finished() {
            self.finished.push(f);
        }
    }
}

/// Runs the games 1 ms at a time; the last game's finished tests.
fn run(net: &SimNetwork, games: &mut [&mut Game], time: Duration) -> Vec<Finished> {
    let end = net.now() + time;
    while net.now() < end {
        net.advance(MS);
        let now = net.now();
        for game in games.iter_mut() {
            game.step(now);
        }
    }
    std::mem::take(&mut games.last_mut().unwrap().finished)
}

// ----- The punching table -------------------------------------------------

const EIM: Mapping = Mapping::EndpointIndependent;
const SYMMETRIC: Mapping = Mapping::AddressAndPortDependent;
const OPEN: Filtering = Filtering::EndpointIndependent;
const ADF: Filtering = Filtering::AddressDependent;
const APDF: Filtering = Filtering::AddressAndPortDependent;

fn prefix(text: &str) -> Prefix {
    text.parse().unwrap()
}

fn router(outside: &str, inside: &str, mapping: Mapping, filtering: Filtering) -> RouterConfig {
    RouterConfig {
        mapping,
        filtering,
        ..RouterConfig::nat(outside.parse().unwrap(), prefix(inside))
    }
}

/// Where one end's joined socket sits, and the candidates its Candidate
/// report would carry.
struct End {
    v4: &'static str,
    v6: Option<&'static str>,
    mapped: Option<&'static str>,
}

impl End {
    fn v4(address: &'static str) -> Self {
        Self {
            v4: address,
            v6: None,
            mapped: None,
        }
    }

    fn bind(&self, net: &SimNetwork) -> SimSocket {
        match self.v6 {
            Some(v6) => net.bind_dual(a(self.v4), a(v6)).unwrap(),
            None => net.bind(a(self.v4)).unwrap(),
        }
    }

    fn candidates(&self) -> Vec<SocketAddr> {
        let mut list = vec![a(self.v4)];
        list.extend(self.mapped.map(a));
        list.extend(self.v6.map(a));
        list
    }
}

/// One row: a candidate (the table's host) and a player, each joined to a
/// public host that sees them at the address their router gives it.
struct Row {
    net: SimNetwork,
    candidate: End,
    player: End,
}

impl Row {
    fn new() -> Self {
        let net = SimNetwork::new(17);
        net.set_default_link(LinkConfig::one_way(Duration::from_millis(50)));
        Self {
            net,
            candidate: End::v4("192.168.1.10:26900"),
            player: End::v4("192.168.2.20:40000"),
        }
    }

    fn add(&self, config: RouterConfig) {
        self.net.add_router(config).unwrap();
    }

    /// The host's Reach peers and Reach test, as the host sends them: the
    /// candidate opens its router to the player's seen address and
    /// candidates, and the player tests the candidate's. True when the
    /// player reached the candidate.
    fn reached(self) -> bool {
        let net = self.net;
        let mut host = net.bind(a("198.51.100.1:26900")).unwrap();
        let mut c = Game::on(self.candidate.bind(&net), 1, 21);
        let mut p = Game::on(self.player.bind(&net), 2, 22);
        // Each game's joined socket has spoken to the host: the host sees
        // it at its router's mapping towards the host.
        let hello = |socket: &mut SimSocket| socket.send_datagram(a("198.51.100.1:26900"), b"hi");
        hello(&mut c.socket).unwrap();
        hello(&mut p.socket).unwrap();
        net.advance(Duration::from_millis(100));
        let mut seen = Vec::new();
        let mut buf = [0u8; 64];
        while let Ok(Some((_, from))) = host.recv_datagram(&mut buf) {
            seen.push(from);
        }
        assert_eq!(seen.len(), 2, "both reached the host");
        let (c_seen, p_seen) = (seen[0], seen[1]);
        let mut to_player = vec![p_seen];
        to_player.extend(self.player.candidates());
        let mut to_candidate = vec![c_seen];
        to_candidate.extend(self.candidate.candidates());
        let now = net.now();
        c.peers.open(now, 1, &to_player);
        p.peers.test(
            now,
            1,
            &[TestTarget {
                player: 1,
                addresses: to_candidate,
            }],
        );
        let finished = run(&net, &mut [&mut c, &mut p], Duration::from_secs(3));
        assert_eq!(finished.len(), 1);
        finished[0].results[0].reached.is_some()
    }
}

#[test]
fn a_candidate_with_no_router_or_a_mapped_port_is_reached_by_any_player() {
    let mut row = Row::new();
    row.candidate = End::v4("198.51.100.10:26900");
    row.add(router("203.0.113.20", "192.168.2.0/24", SYMMETRIC, APDF));
    assert!(row.reached());

    let mut row = Row::new();
    let mut home = router("203.0.113.10", "192.168.1.0/24", EIM, APDF);
    home.forwards.push(Forward {
        outside_port: 26900,
        inside: a("192.168.1.10:26900"),
    });
    row.add(home);
    row.candidate.mapped = Some("203.0.113.10:26900");
    row.add(router("203.0.113.20", "192.168.2.0/24", SYMMETRIC, APDF));
    assert!(row.reached());
}

#[test]
fn two_routers_that_keep_one_port_reach_whatever_they_filter() {
    for (c, p) in [(APDF, APDF), (OPEN, APDF), (APDF, ADF)] {
        let row = Row::new();
        row.add(router("203.0.113.10", "192.168.1.0/24", EIM, c));
        row.add(router("203.0.113.20", "192.168.2.0/24", EIM, p));
        assert!(row.reached(), "{c:?} {p:?}");
    }
}

#[test]
fn a_candidate_filtering_by_address_only_reaches_a_symmetric_player() {
    for filtering in [ADF, OPEN] {
        let row = Row::new();
        row.add(router("203.0.113.10", "192.168.1.0/24", EIM, filtering));
        row.add(router("203.0.113.20", "192.168.2.0/24", SYMMETRIC, APDF));
        assert!(row.reached(), "{filtering:?}");
    }
}

#[test]
fn a_candidate_filtering_by_port_and_a_symmetric_player_do_not_reach() {
    let row = Row::new();
    row.add(router("203.0.113.10", "192.168.1.0/24", EIM, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", SYMMETRIC, APDF));
    assert!(!row.reached());
}

#[test]
fn a_symmetric_candidate_is_reached_at_the_port_its_reach_came_from() {
    for filtering in [ADF, OPEN] {
        let row = Row::new();
        row.add(router("203.0.113.10", "192.168.1.0/24", SYMMETRIC, APDF));
        row.add(router("203.0.113.20", "192.168.2.0/24", EIM, filtering));
        assert!(row.reached(), "{filtering:?}");
    }
}

#[test]
fn a_symmetric_candidate_and_a_player_filtering_by_port_do_not_reach() {
    let row = Row::new();
    row.add(router("203.0.113.10", "192.168.1.0/24", SYMMETRIC, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", EIM, APDF));
    assert!(!row.reached());
}

#[test]
fn two_symmetric_routers_do_not_reach() {
    let row = Row::new();
    row.add(router("203.0.113.10", "192.168.1.0/24", SYMMETRIC, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", SYMMETRIC, APDF));
    assert!(!row.reached());
}

#[test]
fn a_candidate_behind_two_routers_that_keep_one_port_is_reached() {
    let row = Row::new();
    row.add(router("203.0.113.40", "10.0.0.0/8", EIM, APDF));
    row.add(router("10.0.0.2", "192.168.1.0/24", EIM, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", EIM, APDF));
    assert!(row.reached());
}

#[test]
fn a_candidate_and_a_player_at_one_home_reach_on_the_local_network() {
    let mut row = Row::new();
    row.add(router("203.0.113.10", "192.168.1.0/24", EIM, APDF));
    row.player = End::v4("192.168.1.20:40000");
    row.net.set_link_both(
        a("192.168.1.10:26900"),
        a("192.168.1.20:40000"),
        LinkConfig::one_way(Duration::from_millis(1)),
    );
    assert!(row.reached());
}

#[test]
fn a_carrier_router_with_no_ipv6_and_a_player_filtering_by_port_do_not_reach() {
    let row = Row::new();
    row.add(router("203.0.113.50", "100.64.0.0/10", SYMMETRIC, APDF));
    row.add(router("100.64.0.5", "192.168.1.0/24", EIM, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", EIM, APDF));
    assert!(!row.reached());
}

#[test]
fn ipv6_behind_two_firewalls_is_reached_through_both() {
    let mut row = Row::new();
    // IPv4 is no way in, so only IPv6 can reach.
    row.add(router("203.0.113.10", "192.168.1.0/24", SYMMETRIC, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", EIM, APDF));
    row.add(RouterConfig::firewall(prefix("2001:db8:1::/48")));
    row.add(RouterConfig::firewall(prefix("2001:db8:2::/48")));
    row.candidate.v6 = Some("[2001:db8:1::10]:26900");
    row.player.v6 = Some("[2001:db8:2::20]:40000");
    assert!(row.reached());
}

#[test]
fn the_median_is_the_lower_middle() {
    let ms = Duration::from_millis;
    assert_eq!(median(&[]), Duration::ZERO);
    assert_eq!(median(&[ms(5)]), ms(5));
    assert_eq!(median(&[ms(9), ms(1), ms(5)]), ms(5));
    assert_eq!(median(&[ms(9), ms(1), ms(5), ms(7)]), ms(5));
}
