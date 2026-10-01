//! Looking for games on the local network (slice EF5): the search loop the
//! Direct Connection screen runs while it is open (EF7 draws it).
//!
//! Every [`ROUND`] the loop sends a Discover query to the limited broadcast
//! address on the game port, to this machine's loopback address and to its own
//! network address (so a game hosted here is always found, and found at the
//! address other machines would use), and collects the answers into a list of
//! games with the time each was last heard. A game not heard for
//! [`MISSED_ROUNDS`] rounds is dropped. [`Search::update`] never blocks: the
//! screen calls it every frame with the time, and takes the changes as
//! events. The packets are `tore_net::packet`'s kinds 8 and 9; the rules are
//! in docs/formats/net-protocol.md, "Discovery".
//!
//! IPv4 only. The standard library cannot list the machine's network
//! interfaces, so the broadcast leaves by the interface the system picks for
//! 255.255.255.255 (the default route's): a machine with several networks
//! finds the games on that one, and joining by address reaches the others.

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;
use tore_net::packet::{DiscoverAnswer, Packet};
use tore_net::{Datagrams, MAX_DATAGRAM, reach};
use tore_session::BuildId;

/// Time between queries (agent decision: John asked for one every two
/// seconds).
pub const ROUND: Duration = Duration::from_secs(2);
/// A game is dropped after this many rounds without an answer.
pub const MISSED_ROUNDS: u32 = 3;
/// Datagrams one [`Search::update`] reads at most.
const MAX_READ: usize = 256;

/// How a found game's build compares with this game's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compat {
    /// The same build: joining is possible.
    Same,
    /// The same network protocol but another build: the host will refuse the
    /// join and say which version it runs.
    OtherBuild,
    /// Another network protocol version: not joinable.
    OtherProtocol,
}

/// This game's build, to compare found games with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Own {
    pub protocol_version: u16,
    pub build: BuildId,
}

impl Own {
    /// This game: the protocol it speaks and the build it is.
    pub fn this_game() -> Self {
        Self {
            protocol_version: tore_session::wire::PROTOCOL_VERSION,
            build: crate::net::session::build_id(),
        }
    }

    /// How a host's answer compares. The host's own rule (commit equal, or a
    /// plain version equal on a tagged release) needs a release flag the
    /// answer does not carry, so a plain-version match counts as the same
    /// build here (agent decision); the host has the last word at the join.
    pub fn compare(&self, answer: &DiscoverAnswer) -> Compat {
        if answer.protocol_version != self.protocol_version {
            Compat::OtherProtocol
        } else if (answer.game_commit == self.build.commit && answer.game_commit != "unknown")
            || (answer.game_version == self.build.version && !answer.game_version.contains('-'))
        {
            Compat::Same
        } else {
            Compat::OtherBuild
        }
    }
}

/// A game found on the network, as the list shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    /// Where to join it: the address the answer came from, on the game port.
    pub address: SocketAddr,
    /// What the host said.
    pub answer: DiscoverAnswer,
    /// How its build compares with this game's.
    pub compat: Compat,
}

/// A change to the list of games.
#[derive(Clone, Debug, PartialEq)]
pub enum SearchEvent {
    /// A game appeared.
    Added(Game),
    /// A game's details changed: its players, phase, mission or address.
    Changed(Game),
    /// A game went away (not heard for [`MISSED_ROUNDS`] rounds).
    Dropped(Game),
}

struct Heard {
    answer: DiscoverAnswer,
    last_heard: Duration,
}

/// The games heard, by the address that answered, and what each last said.
/// One game can answer from several addresses (a host on this machine answers
/// from its network address and from loopback); the list shows it once, at
/// the best address: a network address before a loopback one, then the lowest.
#[derive(Default)]
pub struct GameList {
    heard: BTreeMap<SocketAddr, Heard>,
}

impl GameList {
    /// Notes an answer from `from` heard at `now`.
    pub fn hear(&mut self, now: Duration, from: SocketAddr, answer: DiscoverAnswer) {
        self.heard.insert(
            from,
            Heard {
                answer,
                last_heard: now,
            },
        );
    }

    /// Forgets the answers not heard for [`MISSED_ROUNDS`] rounds and a half
    /// (the half is slack for a late answer).
    pub fn expire(&mut self, now: Duration) {
        let limit = ROUND * MISSED_ROUNDS + ROUND / 2;
        self.heard
            .retain(|_, heard| now.saturating_sub(heard.last_heard) < limit);
    }

    /// The games, one for each session, by name and then address.
    pub fn games(&self, own: &Own) -> Vec<Game> {
        let mut by_session: BTreeMap<u64, (SocketAddr, &DiscoverAnswer)> = BTreeMap::new();
        for (address, heard) in &self.heard {
            let better = |current: &SocketAddr| {
                let rank = |a: &SocketAddr| (a.ip().is_loopback(), *a);
                rank(address) < rank(current)
            };
            match by_session.get(&heard.answer.session_id) {
                Some((current, _)) if !better(current) => {}
                _ => {
                    by_session.insert(heard.answer.session_id, (*address, &heard.answer));
                }
            }
        }
        let mut games: Vec<Game> = by_session
            .into_values()
            .map(|(address, answer)| Game {
                address,
                compat: own.compare(answer),
                answer: answer.clone(),
            })
            .collect();
        games.sort_by(|a, b| (&a.answer.name, a.address).cmp(&(&b.answer.name, b.address)));
        games
    }
}

/// The changes from `old` to `new`, by session.
fn changes(old: &[Game], new: &[Game]) -> Vec<SearchEvent> {
    let mut events = Vec::new();
    for game in new {
        match old
            .iter()
            .find(|o| o.answer.session_id == game.answer.session_id)
        {
            None => events.push(SearchEvent::Added(game.clone())),
            Some(before) if before != game => events.push(SearchEvent::Changed(game.clone())),
            Some(_) => {}
        }
    }
    for game in old {
        if !new
            .iter()
            .any(|n| n.answer.session_id == game.answer.session_id)
        {
            events.push(SearchEvent::Dropped(game.clone()));
        }
    }
    events
}

/// The search: a socket, the places to ask and the games found.
pub struct Search<D: Datagrams = UdpSocket> {
    socket: D,
    targets: Vec<SocketAddr>,
    nonce: u64,
    query: Vec<u8>,
    own: Own,
    next_round: Duration,
    list: GameList,
    view: Vec<Game>,
    events: VecDeque<SearchEvent>,
}

impl Search<UdpSocket> {
    /// Starts looking for games on UDP `port` at `now`: a socket with
    /// broadcast on, and the first query ready to go out at the first
    /// [`Search::update`]. The loop runs only while it exists: dropping it
    /// stops the search.
    pub fn start(port: u16, own: Own, now: Duration) -> io::Result<Self> {
        let socket = tore_net::bind_udp((Ipv4Addr::UNSPECIFIED, 0).into())?;
        socket.set_broadcast(true)?;
        Self::with(socket, targets(port), own, reach::random_nonce(), now)
    }
}

/// Where a query goes: the limited broadcast address, this machine's network
/// address when it has one, and loopback, all on `port`.
pub fn targets(port: u16) -> Vec<SocketAddr> {
    let mut targets: Vec<SocketAddr> = vec![(Ipv4Addr::BROADCAST, port).into()];
    if let Some(lan) = own_network_address() {
        targets.push((lan, port).into());
    }
    targets.push((Ipv4Addr::LOCALHOST, port).into());
    targets
}

/// The IPv4 address this machine uses to reach the network, if it has one:
/// the address a UDP socket takes when it is pointed at a far address (no
/// packet is sent).
pub fn own_network_address() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() && !ip.is_loopback() => Some(ip),
        _ => None,
    }
}

impl<D: Datagrams> Search<D> {
    /// A search over any datagram socket, asking each of `targets` with
    /// `nonce`.
    pub fn with(
        socket: D,
        targets: Vec<SocketAddr>,
        own: Own,
        nonce: u64,
        now: Duration,
    ) -> io::Result<Self> {
        let query = Packet::Discover(tore_net::packet::Discover {
            protocol_version: own.protocol_version,
            nonce,
        })
        .encode(own.protocol_version)
        .map_err(io::Error::other)?;
        Ok(Self {
            socket,
            targets,
            nonce,
            query,
            own,
            next_round: now,
            list: GameList::default(),
            view: Vec::new(),
            events: VecDeque::new(),
        })
    }

    /// The search's turn at `now`: sends the round's queries when one is due,
    /// reads the answers waiting, forgets games not heard for a while, and
    /// queues an event for each change. Never blocks.
    pub fn update(&mut self, now: Duration) {
        if now >= self.next_round {
            // Catch up in one step after a long pause; never send twice.
            self.next_round = now + ROUND;
            for target in &self.targets {
                // A send the network refuses (no route, a full buffer) is
                // the same as a query nobody answers.
                let _ = self.socket.send_datagram(*target, &self.query);
            }
        }
        let mut buf = [0u8; MAX_DATAGRAM + 1];
        for _ in 0..MAX_READ {
            let Ok(Some((len, from))) = self.socket.recv_datagram(&mut buf) else {
                break;
            };
            if let Ok(Packet::DiscoverAnswer(answer)) =
                Packet::decode(&buf[..len], self.own.protocol_version)
                && answer.nonce == self.nonce
            {
                let from = SocketAddr::new(from.ip().to_canonical(), from.port());
                self.list.hear(now, from, answer);
            }
        }
        self.list.expire(now);
        let view = self.list.games(&self.own);
        self.events.extend(changes(&self.view, &view));
        self.view = view;
    }

    /// The next change to the list, oldest first.
    pub fn poll_event(&mut self) -> Option<SearchEvent> {
        self.events.pop_front()
    }

    /// The games now, by name.
    pub fn games(&self) -> &[Game] {
        &self.view
    }

    /// When the next round of queries goes out.
    pub fn next_round(&self) -> Duration {
        self.next_round
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use tore_net::packet::DiscoverPhase;

    const V: u16 = 3;

    fn own() -> Own {
        Own {
            protocol_version: V,
            build: BuildId {
                version: "0.1.3-2-gabc".into(),
                commit: "abc".into(),
                release: false,
            },
        }
    }

    fn answer(nonce: u64, session: u64, name: &str) -> DiscoverAnswer {
        DiscoverAnswer {
            nonce,
            protocol_version: V,
            game_version: "0.1.3-2-gabc".into(),
            game_commit: "abc".into(),
            session_id: session,
            name: name.into(),
            summary: "UKR".into(),
            players: 1,
            capacity: 4,
            password: false,
            full: false,
            phase: DiscoverPhase::Lobby,
            king: "Viper".into(),
            callsigns: vec!["Viper".into()],
            truncated: false,
        }
    }

    fn bytes(answer: &DiscoverAnswer) -> Vec<u8> {
        Packet::DiscoverAnswer(answer.clone()).encode(V).unwrap()
    }

    /// A socket that records what is sent and hands out what is queued.
    #[derive(Clone, Default)]
    struct Fake {
        sent: Rc<RefCell<Vec<(SocketAddr, Vec<u8>)>>>,
        inbox: Rc<RefCell<VecDeque<(Vec<u8>, SocketAddr)>>>,
    }

    impl Datagrams for Fake {
        fn send_datagram(&mut self, to: SocketAddr, datagram: &[u8]) -> io::Result<()> {
            self.sent.borrow_mut().push((to, datagram.to_vec()));
            Ok(())
        }

        fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
            Ok(self.inbox.borrow_mut().pop_front().map(|(bytes, from)| {
                buf[..bytes.len()].copy_from_slice(&bytes);
                (bytes.len(), from)
            }))
        }
    }

    fn at(secs: f64) -> Duration {
        Duration::from_secs_f64(secs)
    }

    fn rig() -> (Search<Fake>, Fake) {
        let fake = Fake::default();
        let search =
            Search::with(fake.clone(), targets(26_900), own(), 0xABCD, Duration::ZERO).unwrap();
        (search, fake)
    }

    fn hear(fake: &Fake, from: &str, answer: &DiscoverAnswer) {
        fake.inbox
            .borrow_mut()
            .push_back((bytes(answer), from.parse().unwrap()));
    }

    #[test]
    fn a_query_goes_to_every_target_every_two_seconds() {
        let (mut search, fake) = rig();
        search.update(at(0.0));
        let first = fake.sent.borrow().len();
        assert_eq!(first, search.targets.len());
        assert!(first >= 2, "broadcast and loopback at least");
        {
            let sent = fake.sent.borrow();
            assert!(
                sent.iter()
                    .any(|(to, _)| *to == "255.255.255.255:26900".parse().unwrap())
            );
            assert!(
                sent.iter()
                    .any(|(to, _)| *to == "127.0.0.1:26900".parse().unwrap())
            );
            // The query is the padded packet with this search's nonce.
            let Ok(Packet::Discover(query)) = Packet::decode(&sent[0].1, V) else {
                panic!("not a query")
            };
            assert_eq!((query.nonce, query.protocol_version), (0xABCD, V));
            assert_eq!(sent[0].1.len(), tore_net::packet::DISCOVER_LEN);
        }
        search.update(at(1.9));
        assert_eq!(fake.sent.borrow().len(), first);
        search.update(at(2.0));
        assert_eq!(fake.sent.borrow().len(), 2 * first);
        // A long pause sends one round, not a burst.
        search.update(at(60.0));
        assert_eq!(fake.sent.borrow().len(), 3 * first);
    }

    #[test]
    fn answers_are_added_refreshed_changed_and_dropped() {
        let (mut search, fake) = rig();
        search.update(at(0.0));
        let mut found = answer(0xABCD, 7, "Friday night");
        hear(&fake, "192.168.1.20:26900", &found);
        search.update(at(0.1));
        assert!(matches!(
            search.poll_event(),
            Some(SearchEvent::Added(game)) if game.answer.name == "Friday night"
                && game.address == "192.168.1.20:26900".parse().unwrap()
                && game.compat == Compat::Same
        ));
        assert!(search.poll_event().is_none());
        // The same answer again changes nothing: no event.
        hear(&fake, "192.168.1.20:26900", &found);
        search.update(at(2.1));
        assert!(search.poll_event().is_none());
        // A change of players is an event.
        found.players = 3;
        hear(&fake, "192.168.1.20:26900", &found);
        search.update(at(4.1));
        assert!(matches!(
            search.poll_event(),
            Some(SearchEvent::Changed(game)) if game.answer.players == 3
        ));
        assert_eq!(search.games().len(), 1);
        // Heard last at 4.1: still listed at 11.0, dropped by 11.2 (three
        // rounds and a half after).
        search.update(at(10.9));
        assert_eq!(search.games().len(), 1);
        search.update(at(11.2));
        assert!(matches!(search.poll_event(), Some(SearchEvent::Dropped(_))));
        assert!(search.games().is_empty());
    }

    #[test]
    fn an_answer_to_another_search_or_a_stranger_is_ignored() {
        let (mut search, fake) = rig();
        search.update(at(0.0));
        hear(
            &fake,
            "192.168.1.20:26900",
            &answer(0x1111, 7, "Wrong nonce"),
        );
        fake.inbox
            .borrow_mut()
            .push_back((vec![1, 2, 3], "10.0.0.1:1".parse().unwrap()));
        fake.inbox.borrow_mut().push_back((
            Packet::Challenge(tore_net::packet::Challenge {
                nonce: 0xABCD,
                cookie: 1,
            })
            .encode(V)
            .unwrap(),
            "10.0.0.1:1".parse().unwrap(),
        ));
        search.update(at(0.1));
        assert!(search.games().is_empty() && search.poll_event().is_none());
    }

    #[test]
    fn one_game_answering_from_two_addresses_is_listed_once_at_the_network_address() {
        let (mut search, fake) = rig();
        search.update(at(0.0));
        let found = answer(0xABCD, 7, "Mine");
        hear(&fake, "127.0.0.1:26900", &found);
        search.update(at(0.1));
        assert_eq!(
            search.games()[0].address,
            "127.0.0.1:26900".parse().unwrap()
        );
        hear(&fake, "192.168.1.115:26900", &found);
        search.update(at(0.2));
        assert_eq!(search.games().len(), 1);
        assert_eq!(
            search.games()[0].address,
            "192.168.1.115:26900".parse().unwrap()
        );
        assert!(matches!(search.poll_event(), Some(SearchEvent::Added(_))));
        assert!(matches!(search.poll_event(), Some(SearchEvent::Changed(_))));
        // Another game is another entry.
        hear(&fake, "192.168.1.30:26900", &answer(0xABCD, 8, "Theirs"));
        search.update(at(0.3));
        assert_eq!(search.games().len(), 2);
        assert_eq!(search.games()[0].answer.name, "Mine");
    }

    #[test]
    fn a_different_build_or_protocol_is_listed_as_such() {
        let (mut search, fake) = rig();
        search.update(at(0.0));
        let mut other_build = answer(0xABCD, 1, "Other build");
        other_build.game_version = "0.1.4-1-gdef".into();
        other_build.game_commit = "def".into();
        let mut other_protocol = answer(0xABCD, 2, "Other protocol");
        other_protocol.protocol_version = V + 1;
        let mut tagged = answer(0xABCD, 3, "Tagged");
        tagged.game_commit = "other".into();
        tagged.game_version = "0.1.3".into();
        let mut local = own();
        local.build.version = "0.1.3".into();
        for (found, expect) in [
            (&other_build, Compat::OtherBuild),
            (&other_protocol, Compat::OtherProtocol),
        ] {
            assert_eq!(own().compare(found), expect);
        }
        assert_eq!(local.compare(&tagged), Compat::Same);
        for (i, found) in [&other_build, &other_protocol].into_iter().enumerate() {
            hear(&fake, &format!("10.0.0.{}:26900", i + 1), found);
        }
        search.update(at(0.1));
        let compat: Vec<_> = search.games().iter().map(|g| g.compat).collect();
        assert_eq!(compat, [Compat::OtherBuild, Compat::OtherProtocol]);
    }

    /// A host on loopback, on a thread of its own, answering queries from a
    /// fixed game summary until told to stop.
    #[test]
    fn a_real_host_on_loopback_is_found_over_real_sockets() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        use tore_net::{
            ConnectDetails, Decision, Entropy, RefuseReason, Server, ServerConfig, ServerEvent,
        };
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        let address = socket.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            let mut server = Server::new(ServerConfig {
                entropy: Entropy::Seeded(1),
                ..ServerConfig::new(V)
            });
            let mut gate = |_: &ConnectDetails| Decision::Refuse {
                reason: RefuseReason::ShuttingDown,
                text: String::new(),
            };
            let started = std::time::Instant::now();
            let mut buf = [0u8; 1201];
            while !flag.load(Ordering::Relaxed) {
                if let Ok((len, from)) = socket.recv_from(&mut buf) {
                    server.receive(started.elapsed(), from, &buf[..len], &mut gate);
                }
                while let Some(ServerEvent::Discover { from, query }) = server.poll_event() {
                    server.answer_discover(from, answer(query.nonce, 99, "Loopback game"));
                }
                while let Some(transmit) = server.poll_transmit() {
                    socket.send_to(&transmit.datagram, transmit.to).unwrap();
                }
            }
        });
        let socket = tore_net::bind_udp((Ipv4Addr::UNSPECIFIED, 0).into()).unwrap();
        socket.set_broadcast(true).unwrap();
        let mut search =
            Search::with(socket, vec![address], own(), 0x5EED, Duration::ZERO).unwrap();
        let started = std::time::Instant::now();
        while search.games().is_empty() && started.elapsed() < Duration::from_secs(3) {
            search.update(started.elapsed());
            std::thread::sleep(Duration::from_millis(5));
        }
        stop.store(true, Ordering::Relaxed);
        thread.join().unwrap();
        assert_eq!(search.games().len(), 1);
        assert_eq!(search.games()[0].address, address);
        assert_eq!(search.games()[0].answer.name, "Loopback game");
    }

    /// Looks for games on this machine's network for a while and prints what
    /// it hears and when: the on-machine check of slice EF5. Run with a host
    /// or a server up:
    ///
    /// ```sh
    /// TORE_FIND_PORT=26900 TORE_FIND_SECONDS=8 cargo test --locked -p tore-app \
    ///   find_games_on_this_machine -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "looks at the real network; run by hand"]
    fn find_games_on_this_machine() {
        let port = std::env::var("TORE_FIND_PORT")
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or(tore_net::DEFAULT_PORT);
        let seconds: f64 = std::env::var("TORE_FIND_SECONDS")
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or(6.0);
        // TORE_FIND_TARGET=ADDRESS asks that one address alone.
        let mut search = match std::env::var("TORE_FIND_TARGET") {
            Ok(address) => {
                let socket = tore_net::bind_udp((Ipv4Addr::UNSPECIFIED, 0).into()).unwrap();
                socket.set_broadcast(true).unwrap();
                let target: SocketAddr = format!("{address}:{port}").parse().unwrap();
                eprintln!("asking [{target}]");
                Search::with(
                    socket,
                    vec![target],
                    Own::this_game(),
                    reach::random_nonce(),
                    Duration::ZERO,
                )
                .unwrap()
            }
            Err(_) => {
                eprintln!("asking {:?}", targets(port));
                Search::start(port, Own::this_game(), Duration::ZERO).unwrap()
            }
        };
        let clock = tore_net::RealClock::new();
        while clock.now().as_secs_f64() < seconds {
            search.update(clock.now());
            while let Some(event) = search.poll_event() {
                eprintln!("{:>7.1} ms  {event:?}", clock.now().as_secs_f64() * 1000.0);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        for game in search.games() {
            eprintln!(
                "listed: {} at {} ({:?}, {}/{} players, {:?}, king {:?})",
                game.answer.name,
                game.address,
                game.compat,
                game.answer.players,
                game.answer.capacity,
                game.answer.phase,
                game.answer.king
            );
        }
    }
}
