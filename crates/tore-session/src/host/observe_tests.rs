//! Slice F2-O1's tests (docs/ARCHITECTURE.md, "The observer view"): the
//! observer stream on the network simulator. A raw observer (the transport's
//! client and the wire's connection state, so every snapshot and event can
//! be read as the host sent it) watches a mission flown by the AI and by
//! bots (the client session with the scripted pilot). Synthetic resources.

use super::*;
use crate::bot::Bot;
use crate::settings::number;
use crate::wire::connection::{ClientConnection, FlightOrder};
use crate::wire::entity::{EntityKey, EntityKind};
use crate::wire::events::ReceivedEvent;
use crate::wire::messages::{
    LobbyState, Mission, Observe, ObserverFlight, Observing, Subject, TakePlane,
};
use crate::wire::snapshot::ReceivedSnapshot;
use crate::wire::{SECTION_EVENTS, SECTION_SNAPSHOT};
use std::collections::HashMap;
use tore_formats::aircraft::AircraftId;
use tore_net::sim::{LinkConfig, SimNetwork, SimSocket};
use tore_net::{ClientEvent, Entropy};
use tore_world::mission::{Skill, Start};
use tore_world::test_support::resources::{THEATER, resources};

const MS: Duration = Duration::from_millis(1);

fn host_address() -> SocketAddr {
    "10.0.0.1:26900".parse().unwrap()
}

fn build() -> BuildId {
    BuildId {
        version: "0.1.3-1-gtest".into(),
        commit: "test-commit".into(),
        release: false,
    }
}

/// Friendly Wing 1 of `friendly` and the enemy's Wing 1 of `enemy`, the
/// enemy `separation_nm` ahead, airborne at 10,000 feet.
fn spec(friendly: usize, enemy: usize, separation_nm: u32) -> MissionSpec {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = friendly;
    spec.wings[3].count = enemy;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = separation_nm;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

/// 15 against 15: every wing of five, the enemy 10 nm ahead.
fn crowd() -> MissionSpec {
    let mut spec = spec(5, 5, 10);
    for wing in &mut spec.wings {
        wing.count = 5;
    }
    spec
}

/// One snapshot as the observer read it, with the host's tick when it
/// arrived.
struct Seen {
    arrived: u64,
    header: SnapshotHeader,
    snapshot: ReceivedSnapshot,
}

/// A raw observer's game.
struct Watcher {
    socket: SimSocket,
    client: tore_net::Client,
    wire: Option<ClientConnection>,
    tps: u32,
    mission: Option<Mission>,
    lobby: Option<LobbyState>,
    /// Watch with this camera as soon as the mission flies.
    watch: Option<Subject>,
    asked: bool,
    observing: Vec<Observing>,
    /// Every message's kind and the host's tick when it arrived, in
    /// arrival order.
    kinds: Vec<(u8, u64)>,
    refused: Vec<(u8, String)>,
    seated: Option<Seated>,
    seen: Vec<Seen>,
    events: Vec<(u64, ReceivedEvent)>,
    errors: Vec<String>,
}

impl Watcher {
    fn new(net: &SimNetwork, port: u16, watch: Option<Subject>) -> Self {
        let address: SocketAddr = format!("10.0.0.2:{port}").parse().unwrap();
        let socket = net.bind(address).unwrap();
        let config = tore_net::ClientConfig {
            game_version: build().version,
            game_commit: build().commit,
            entropy: Entropy::Seeded(u64::from(port)),
            ..tore_net::ClientConfig::new(PROTOCOL_VERSION, "Owl")
        };
        Self {
            socket,
            client: tore_net::Client::connect(config, host_address(), net.now()).unwrap(),
            wire: None,
            tps: 4,
            mission: None,
            lobby: None,
            watch,
            asked: false,
            observing: Vec::new(),
            kinds: Vec::new(),
            refused: Vec::new(),
            seated: None,
            seen: Vec::new(),
            events: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn send(&mut self, message: &Message) {
        let body = message.encode().unwrap();
        self.client.send_message(message.kind(), &body).unwrap();
    }

    fn started(&self) -> Option<&ObserverFlight> {
        match self.observing.last() {
            Some(Observing::Started(started)) => Some(started),
            _ => None,
        }
    }

    fn pump(&mut self, now: Duration, host_tick: u64) {
        let wire = &self.wire;
        let _ = self
            .client
            .receive_from_checked(&mut self.socket, now, &mut |kind, body| {
                wire.as_ref().is_none_or(|w| w.check(kind, body))
            });
        self.client.update(now);
        while let Some(event) = self.client.poll_event() {
            match event {
                ClientEvent::Connected(welcome) => {
                    self.tps = u32::from(welcome.ticks_per_snapshot);
                    self.wire = Some(ClientConnection::new(self.tps));
                }
                ClientEvent::Connection(Event::Message { kind, body }) => {
                    self.kinds.push((kind, host_tick));
                    match Message::decode(kind, &body) {
                        Ok(message) => self.message(message),
                        Err(error) => self.errors.push(format!("message: {error}")),
                    }
                }
                ClientEvent::Connection(Event::Payload { sections, .. }) => {
                    self.payload(sections, host_tick);
                }
                _ => {}
            }
        }
        if let Some(subject) = self.watch
            && !self.asked
            && self
                .lobby
                .as_ref()
                .is_some_and(|l| l.phase == LobbyPhase::Flying)
        {
            self.asked = true;
            self.send(&Message::Observe(Observe::Watch(subject)));
        }
        let _ = self.client.transmit(&mut self.socket);
    }

    fn payload(&mut self, sections: Vec<tore_net::Section>, host_tick: u64) {
        let mut tick = None;
        for section in sections {
            if let Some(flight) = ClientConnection::section_flight(section.kind, &section.body) {
                let current = self.wire.as_ref().and_then(|w| w.flight);
                match FlightOrder::of(flight, current) {
                    FlightOrder::Earlier => continue,
                    FlightOrder::Later => {
                        self.wire = Some(ClientConnection::for_flight(self.tps, flight));
                    }
                    FlightOrder::Same => {}
                }
            }
            let wire = self.wire.as_mut().unwrap();
            match section.kind {
                SECTION_SNAPSHOT => match wire.snapshot(&section.body) {
                    Ok((header, snapshot)) => {
                        tick = Some(header.tick);
                        self.seen.push(Seen {
                            arrived: host_tick,
                            header,
                            snapshot,
                        });
                    }
                    Err(error) => self.errors.push(format!("snapshot: {error}")),
                },
                SECTION_EVENTS => match wire.events(&section.body, tick.unwrap_or_default()) {
                    Ok(events) => self
                        .events
                        .extend(events.into_iter().map(|e| (host_tick, e))),
                    Err(error) => self.errors.push(format!("events: {error}")),
                },
                other => self.errors.push(format!("section {other}")),
            }
        }
    }

    fn message(&mut self, message: Message) {
        match message {
            Message::Mission(mission) => self.mission = Some(mission),
            Message::Lobby(lobby) => self.lobby = Some(*lobby),
            Message::Observing(observing) => {
                if let Observing::Started(started) = &*observing {
                    let current = self.wire.as_ref().and_then(|w| w.flight);
                    if FlightOrder::of(started.flight, current) == FlightOrder::Later {
                        self.wire = Some(ClientConnection::for_flight(self.tps, started.flight));
                    }
                }
                self.observing.push(*observing);
            }
            Message::Names(names) => {
                if let Some(wire) = self.wire.as_mut() {
                    match wire.names(&names) {
                        Ok(events) => self.events.extend(events.into_iter().map(|e| (0, e))),
                        Err(error) => self.errors.push(format!("names: {error}")),
                    }
                }
            }
            Message::Seated(seated) => {
                let current = self.wire.as_ref().and_then(|w| w.flight);
                if FlightOrder::of(seated.flight, current) == FlightOrder::Later {
                    self.wire = Some(ClientConnection::for_flight(self.tps, seated.flight));
                }
                self.seated = Some(*seated);
            }
            Message::Refused { request, reason } => self.refused.push((request, reason)),
            _ => {}
        }
    }

    /// Where the first message of `kind` came, and the last.
    fn first(&self, kind: u8) -> Option<usize> {
        self.kinds.iter().position(|(k, _)| *k == kind)
    }

    fn last(&self, kind: u8) -> Option<usize> {
        self.kinds.iter().rposition(|(k, _)| *k == kind)
    }

    /// The snapshots of the observer flight `flight`.
    fn flight_seen(&self, flight: u8) -> impl Iterator<Item = &Seen> {
        self.seen.iter().filter(move |s| s.header.flight == flight)
    }
}

/// A host, its bots and its raw observers on one simulated network, and
/// where every aircraft was after each host tick.
struct Rig {
    net: SimNetwork,
    host: Host,
    socket: SimSocket,
    bots: Vec<(SimSocket, Bot, Vec<crate::client::ClientEvent>)>,
    watchers: Vec<Watcher>,
    next_port: u16,
    truth: HashMap<(u64, u32), [f64; 3]>,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
}

impl Rig {
    /// A dedicated server's mission flying from the start, every plane open
    /// (so a fight between bots has room for observers: each connection
    /// counts against the open planes).
    fn new(spec: MissionSpec, configure: impl FnOnce(&mut HostConfig)) -> Self {
        let net = SimNetwork::new(7);
        net.set_default_link(LinkConfig::for_round_trip(40 * MS, 0., 0., 0.));
        let socket = net.bind(host_address()).unwrap();
        let resources = Arc::new(resources());
        let mut config = HostConfig {
            entropy: Entropy::Seeded(11),
            start: StartMode::Now,
            open_planes: OpenPlanes::All,
            ..HostConfig::new(build())
        };
        configure(&mut config);
        let host = Host::new(spec, Arc::clone(&resources), config).unwrap();
        Self {
            net,
            host,
            socket,
            bots: Vec::new(),
            watchers: Vec::new(),
            next_port: 40_000,
            truth: HashMap::new(),
            resources,
        }
    }

    /// PvP with the observers' delay.
    fn delayed(&mut self, seconds: u32) {
        self.host
            .settings
            .apply(&[(number::MODE, 1), (number::OBSERVER_DELAY, seconds)])
            .unwrap();
        assert_eq!(
            self.host.observer_delay_ticks(),
            u64::from(seconds) * TICKS_PER_SECOND
        );
    }

    fn bot(&mut self, plane: Option<u32>) -> usize {
        let address: SocketAddr = format!("10.0.0.2:{}", self.next_port).parse().unwrap();
        let socket = self.net.bind(address).unwrap();
        let config = crate::client::ClientConfig {
            entropy: Entropy::Seeded(u64::from(self.next_port)),
            plane,
            ..crate::client::ClientConfig::new(host_address(), "Bot", build())
        };
        self.next_port += 1;
        let client =
            crate::client::Client::connect(config, Arc::clone(&self.resources), self.net.now())
                .unwrap();
        self.bots.push((socket, Bot::new(client), Vec::new()));
        self.bots.len() - 1
    }

    fn watcher(&mut self, watch: Option<Subject>) -> usize {
        let watcher = Watcher::new(&self.net, self.next_port, watch);
        self.next_port += 1;
        self.watchers.push(watcher);
        self.watchers.len() - 1
    }

    fn step(&mut self) {
        self.net.advance(MS);
        let now = self.net.now();
        let before = self.host.world().tick();
        self.host.receive_from(now, &mut self.socket).unwrap();
        self.host.update(now);
        self.host.transmit(&mut self.socket).unwrap();
        while self.host.poll_log().is_some() {}
        let world = self.host.world();
        if world.tick() > before {
            let tick = world.tick() - 1;
            for cockpit in &world.cockpits {
                self.truth
                    .insert((tick, cockpit.plane.0), cockpit.flight.position);
            }
            for actor in world.ai_wings.iter().flat_map(|w| w.mission().actors()) {
                self.truth
                    .insert((tick, actor.id()), actor.flight().position);
            }
        }
        let tick = self.host.world().tick();
        for (socket, bot, events) in &mut self.bots {
            bot.client.receive_from(now, socket).unwrap();
            bot.update(now);
            bot.client.transmit(socket).unwrap();
            while let Some(event) = bot.client.poll_event() {
                events.push(event);
            }
        }
        for watcher in &mut self.watchers {
            watcher.pump(now, tick);
        }
    }

    fn run(&mut self, time: Duration) {
        let end = self.net.now() + time;
        while self.net.now() < end {
            self.step();
        }
    }

    fn run_until(&mut self, limit: Duration, mut done: impl FnMut(&Rig) -> bool) -> bool {
        let end = self.net.now() + limit;
        while self.net.now() < end {
            self.step();
            if done(self) {
                return true;
            }
        }
        false
    }

    /// The host's connection of watcher `w`.
    fn connection(&self, w: usize) -> ConnectionId {
        let port = self.watchers[w].socket.local_addr().port();
        *self
            .host
            .peers
            .iter()
            .find(|(_, peer)| peer.address.port() == port)
            .expect("the watcher is connected")
            .0
    }
}

/// The aircraft ids of `side` in the observer flight's roster.
fn side_planes(started: &ObserverFlight, friendly: bool) -> Vec<u32> {
    started
        .roster
        .planes
        .iter()
        .filter(|p| (p.wing.side == Side::Friendly) == friendly)
        .map(|p| p.id)
        .collect()
}

#[test]
fn an_observer_gets_entities_near_its_camera_every_snapshot_and_the_rest_twice_a_second() {
    // The AI alone: the enemy 50 nm from the friendly wing the camera
    // follows.
    let mut rig = Rig::new(spec(2, 2, 50), |_| {});
    let w = rig.watcher(Some(Subject::Aircraft(0)));
    assert!(
        rig.run_until(Duration::from_secs(5), |r| r.watchers[w]
            .started()
            .is_some()),
        "the observer flight starts"
    );
    let started = rig.watchers[w].started().unwrap().clone();
    assert_eq!(started.delay_seconds, 0);
    assert!(started.roster.planes.len() == 4);
    let from = rig.watchers[w].seen.len();
    rig.run(Duration::from_secs(4));
    let watcher = &rig.watchers[w];
    assert!(watcher.errors.is_empty(), "{:?}", watcher.errors);
    let seen: Vec<&Seen> = watcher.seen[from..].iter().collect();
    // 30 snapshots a second, each with no own plane, no readout and no
    // input figures, on the observer's lobby id's phase.
    assert!(
        (110..=125).contains(&seen.len()),
        "{} snapshots",
        seen.len()
    );
    let lobby = watcher.lobby.as_ref().unwrap();
    let phase = crate::wire::snapshot_phase(lobby.you, 4);
    for s in &seen {
        assert_eq!(s.header.flight, started.flight);
        assert_eq!(s.header.own_hash, None);
        assert!(s.snapshot.readout.is_none());
        assert_eq!(
            (
                s.header.input_received,
                s.header.input_margin,
                s.header.inputs_repeated,
                s.header.commands_applied
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(u64::from(s.header.tick) % 4, phase);
        assert!(s.snapshot.unresolved.is_empty());
    }
    let sent = |id: u32| {
        seen.iter()
            .filter(|s| {
                s.snapshot.updated.iter().any(|e| {
                    e.key()
                        == EntityKey {
                            kind: EntityKind::Aircraft,
                            id,
                        }
                })
            })
            .count()
    };
    for id in side_planes(&started, true) {
        assert!(
            sent(id) + 2 >= seen.len(),
            "friendly {id}, near the camera: {} of {}",
            sent(id),
            seen.len()
        );
    }
    for id in side_planes(&started, false) {
        assert!(
            (6..=10).contains(&sent(id)),
            "enemy {id}, 50 nm away: {} in 4 s",
            sent(id)
        );
    }
    // The lobby marks the observer.
    let me = lobby.players.iter().find(|p| p.id == lobby.you).unwrap();
    assert!(me.observing && !me.flying);
}

#[test]
fn a_point_camera_and_none_choose_what_is_near() {
    let mut rig = Rig::new(spec(2, 2, 50), |_| {});
    // A point far from both wings: everything is far.
    let w = rig.watcher(Some(Subject::Point([3_000_000, 10_000, -3_000_000])));
    assert!(rig.run_until(Duration::from_secs(5), |r| {
        r.watchers[w].started().is_some()
    }));
    // Let the first full records go by.
    rig.run(Duration::from_secs(1));
    let from = rig.watchers[w].seen.len();
    rig.run(Duration::from_secs(3));
    let records: usize = rig.watchers[w].seen[from..]
        .iter()
        .map(|s| s.snapshot.updated.len())
        .sum();
    // Four aircraft twice a second for 3 seconds.
    assert!((20..=28).contains(&records), "{records} records");

    // No subject: everything at the full rate.
    let id = rig.connection(w);
    rig.watchers[w].send(&Message::Observe(Observe::Watch(Subject::None)));
    rig.run(Duration::from_millis(600));
    assert_eq!(
        rig.host.peers[&id].watch.as_ref().unwrap().subject(),
        Subject::None
    );
    let from = rig.watchers[w].seen.len();
    rig.run(Duration::from_secs(2));
    let seen = &rig.watchers[w].seen[from..];
    assert!(
        seen.iter().all(|s| s.snapshot.updated.len() == 4),
        "every aircraft in every snapshot"
    );
}

#[test]
fn camera_changes_are_applied_at_most_twice_a_second() {
    let mut rig = Rig::new(spec(2, 2, 20), |_| {});
    let w = rig.watcher(Some(Subject::Aircraft(0)));
    assert!(rig.run_until(Duration::from_secs(5), |r| {
        r.watchers[w].started().is_some()
    }));
    rig.run(Duration::from_millis(600));
    let id = rig.connection(w);
    let subject = |rig: &Rig| rig.host.peers[&id].watch.as_ref().unwrap().subject();
    rig.watchers[w].send(&Message::Observe(Observe::Watch(Subject::Aircraft(1))));
    rig.run(Duration::from_millis(60));
    assert_eq!(subject(&rig), Subject::Aircraft(1));
    // Two more at once: the first waits, the second replaces it, and it is
    // applied half a second after the change before.
    rig.watchers[w].send(&Message::Observe(Observe::Watch(Subject::Aircraft(2))));
    rig.watchers[w].send(&Message::Observe(Observe::Watch(Subject::Aircraft(3))));
    rig.run(Duration::from_millis(100));
    assert_eq!(subject(&rig), Subject::Aircraft(1));
    rig.run(Duration::from_millis(500));
    assert_eq!(subject(&rig), Subject::Aircraft(3));
    assert!(rig.watchers[w].refused.is_empty());
}

#[test]
fn watching_is_refused_to_a_flying_player_before_the_mission_flies_and_for_no_plane() {
    // A server that waits for its first player: the lobby first.
    let mut rig = Rig::new(spec(2, 2, 20), |c| c.start = StartMode::FirstPlayer);
    let w = rig.watcher(None);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.watchers[w].lobby.is_some()));
    rig.watchers[w].send(&Message::Observe(Observe::Watch(Subject::None)));
    rig.run(Duration::from_millis(200));
    assert_eq!(
        rig.watchers[w].refused,
        [(
            kind::OBSERVE,
            "The mission is not flying; watch once it flies.".to_owned()
        )]
    );

    // Flying: a bot in plane 0 is refused, and a plane that is not there.
    let mut rig = Rig::new(spec(2, 2, 20), |_| {});
    let bot = rig.bot(Some(0));
    let w = rig.watcher(None);
    assert!(rig.run_until(Duration::from_secs(5), |r| {
        r.bots[bot].1.client.phase() == crate::client::ClientPhase::Flying
            && r.watchers[w].lobby.is_some()
    }));
    rig.bots[bot]
        .1
        .client
        .observe(Observe::Watch(Subject::Aircraft(1)));
    rig.watchers[w].send(&Message::Observe(Observe::Watch(Subject::Aircraft(99))));
    rig.run(Duration::from_millis(300));
    let refused: Vec<(u8, String)> = rig.bots[bot]
        .2
        .iter()
        .filter_map(|e| match e {
            crate::client::ClientEvent::Refused { request, reason } => {
                Some((*request, reason.clone()))
            }
            _ => None,
        })
        .collect();
    assert!(rig.bots[bot].1.client.watching().is_none());
    assert_eq!(
        refused,
        [(
            kind::OBSERVE,
            "Leave your aircraft before you watch.".to_owned()
        )]
    );
    assert_eq!(
        rig.watchers[w].refused,
        [(kind::OBSERVE, "There is no plane 99.".to_owned())]
    );
    // Stop with no watch does nothing.
    rig.watchers[w].send(&Message::Observe(Observe::Stop));
    rig.run(Duration::from_millis(200));
    assert!(rig.watchers[w].observing.is_empty());
    assert_eq!(rig.watchers[w].refused.len(), 1);
}

#[test]
fn an_observer_sees_the_human_flown_planes_and_the_mission_wide_events() {
    // Two bots in a fight 5 nm apart, firing; the observer follows plane 0.
    let mut rig = Rig::new(spec(2, 2, 5), |_| {});
    rig.bot(Some(0));
    rig.bot(Some(2));
    let w = rig.watcher(Some(Subject::Aircraft(0)));
    assert!(rig.run_until(Duration::from_secs(5), |r| {
        r.watchers[w].started().is_some()
    }));
    rig.run(Duration::from_secs(20));
    let watcher = &rig.watchers[w];
    assert!(watcher.errors.is_empty(), "{:?}", watcher.errors);
    let flight = watcher.started().unwrap().flight;
    // Plane 0 and plane 2 are flown by bots; every snapshot that carries
    // them shows them where the host had them at that tick.
    let mut checked = 0;
    for s in watcher.flight_seen(flight) {
        for e in &s.snapshot.updated {
            if e.state.kind() != EntityKind::Aircraft {
                continue;
            }
            let truth = rig.truth[&(u64::from(s.header.tick), e.id)];
            let at = e.state.motion().position_ft();
            let off = (0..3).map(|i| (at[i] - truth[i]).abs()).fold(0., f64::max);
            assert!(
                off < 0.05,
                "plane {} at tick {}: {off} ft",
                e.id,
                s.header.tick
            );
            checked += usize::from(e.id == 0 || e.id == 2);
        }
    }
    assert!(checked > 100, "{checked} records of the flown planes");
    // Mission-wide events reach the observer: launches or gun bursts and
    // sounds, never a seat's own.
    let wide = watcher
        .events
        .iter()
        .filter(|(_, e)| {
            matches!(
                e.event,
                WireEvent::GunBurst { .. } | WireEvent::Launch { .. } | WireEvent::Sound { .. }
            )
        })
        .count();
    assert!(wide > 0, "no mission-wide event in 20 s of a fight");
    assert!(watcher.events.iter().all(|(_, e)| !matches!(
        e.event,
        WireEvent::Message { .. }
            | WireEvent::Radio { .. }
            | WireEvent::Release { .. }
            | WireEvent::Feedback { .. }
            | WireEvent::YourAircraftExploded { .. }
            | WireEvent::Link(_)
    )));
}

#[test]
fn with_a_delay_nothing_newer_than_now_less_the_delay_leaves_the_host() {
    let mut rig = Rig::new(spec(2, 2, 5), |_| {});
    rig.delayed(10);
    rig.bot(Some(0));
    rig.bot(Some(2));
    // The observer asks two seconds in, before the ring holds 10 seconds.
    rig.run(Duration::from_secs(2));
    let w = rig.watcher(Some(Subject::Aircraft(0)));
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.watchers[w].started().is_some()
    }));
    let started = rig.watchers[w].started().unwrap().clone();
    assert_eq!(started.delay_seconds, 10);
    // The first snapshot shows the ring's first frame, ten seconds after it
    // was recorded.
    assert_eq!(started.tick, 0);
    // The scores of a tick reach the observer once its stream shows it.
    let id = rig.connection(w);
    let now_tick = rig.host.world().tick();
    rig.host
        .send_as_of(id, now_tick, Message::Notice("news".into()));
    // The fight's first 20 seconds, shown 10 seconds late.
    rig.run(Duration::from_secs(28));

    let watcher = &rig.watchers[w];
    assert!(watcher.errors.is_empty(), "{:?}", watcher.errors);
    let delay = 10 * TICKS_PER_SECOND;
    let seen: Vec<&Seen> = watcher.flight_seen(started.flight).collect();
    assert!(seen.len() > 200, "{} snapshots", seen.len());
    assert_eq!(seen[0].header.tick, started.tick);
    for s in &seen {
        let shown = u64::from(s.header.tick);
        assert_eq!(shown % 4, 0, "the ring's ticks");
        // Sent at the host's tick `shown + delay`, and arrived after it.
        assert!(
            shown + delay <= s.arrived,
            "tick {shown} arrived at {}",
            s.arrived
        );
        assert!(
            s.arrived <= shown + delay + 12,
            "tick {shown} arrived at {}",
            s.arrived
        );
        // What the host had then, not now.
        for e in s
            .snapshot
            .updated
            .iter()
            .filter(|e| e.state.kind() == EntityKind::Aircraft)
        {
            let truth = rig.truth[&(shown, e.id)];
            let at = e.state.motion().position_ft();
            let off = (0..3).map(|i| (at[i] - truth[i]).abs()).fold(0., f64::max);
            assert!(off < 0.05, "plane {} at tick {shown}: {off} ft", e.id);
        }
    }
    // Events too: each arrived at least the delay after its tick.
    let events: Vec<&(u64, ReceivedEvent)> = watcher
        .events
        .iter()
        .filter(|(arrived, _)| *arrived > 0)
        .collect();
    assert!(!events.is_empty());
    for (arrived, event) in events {
        assert!(
            u64::from(event.tick) + delay <= *arrived,
            "{:?} of tick {} arrived at {arrived}",
            event.event,
            event.tick
        );
    }
    // The held message came once the stream reached its tick.
    let notice = watcher.first(kind::NOTICE).expect("the notice arrived");
    assert!(watcher.kinds[notice].1 >= now_tick + delay);

    // The ring holds the delay's frames: 30 a second.
    let ring = rig.host.stream.ring().unwrap();
    assert!(
        (295..=302).contains(&ring.frames()),
        "{} frames",
        ring.frames()
    );
}

#[test]
fn the_stream_stops_when_the_observer_takes_a_plane_and_at_the_end() {
    let mut rig = Rig::new(spec(2, 2, 20), |c| c.after_end = AfterEnd::Quit);
    let w = rig.watcher(Some(Subject::Aircraft(1)));
    let v = rig.watcher(Some(Subject::None));
    assert!(rig.run_until(Duration::from_secs(5), |r| {
        r.watchers[w].started().is_some() && r.watchers[v].started().is_some()
    }));
    rig.run(Duration::from_secs(1));
    // Taking a plane: Observing's end comes before the Seated message.
    let number = rig.watchers[w].mission.as_ref().unwrap().number;
    rig.watchers[w].send(&Message::TakePlane(TakePlane {
        mission: number,
        plane: Some(0),
    }));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.watchers[w].seated.is_some()));
    let watcher = &rig.watchers[w];
    let ended = watcher.last(kind::OBSERVING).expect("Observing's end");
    assert!(ended < watcher.first(kind::SEATED).unwrap());
    assert_eq!(rig.watchers[w].observing.last(), Some(&Observing::Ended));
    let flight = rig.watchers[w].started().map(|s| s.flight);
    assert!(flight.is_none(), "the last Observing ended the watch");
    let seated_flight = rig.watchers[w].seated.as_ref().unwrap().flight;
    rig.run(Duration::from_millis(500));
    assert!(
        rig.watchers[w]
            .seen
            .iter()
            .rev()
            .take(5)
            .all(|s| s.header.flight == seated_flight),
        "snapshots of the seat's flight"
    );

    // The end: Observing's end before Mission ended.
    rig.host.end();
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.watchers[v].first(kind::MISSION_ENDED).is_some()
    }));
    let watcher = &rig.watchers[v];
    assert!(watcher.last(kind::OBSERVING).unwrap() < watcher.first(kind::MISSION_ENDED).unwrap());
    assert_eq!(rig.watchers[v].observing.last(), Some(&Observing::Ended));
    assert!(rig.host.stream.ring().is_none());
}

#[test]
fn an_observer_who_stops_is_back_in_the_lobby() {
    let mut rig = Rig::new(spec(2, 2, 20), |_| {});
    let w = rig.watcher(Some(Subject::Aircraft(0)));
    assert!(rig.run_until(Duration::from_secs(5), |r| {
        r.watchers[w].started().is_some()
    }));
    rig.run(Duration::from_millis(500));
    rig.watchers[w].send(&Message::Observe(Observe::Stop));
    rig.run(Duration::from_millis(1500));
    let watcher = &rig.watchers[w];
    assert_eq!(watcher.observing.last(), Some(&Observing::Ended));
    let lobby = watcher.lobby.as_ref().unwrap();
    assert!(!lobby.players.iter().any(|p| p.observing));
    let flight = watcher.observing.iter().find_map(|o| match o {
        Observing::Started(s) => Some(s.flight),
        Observing::Ended => None,
    });
    let last = watcher.seen.last().unwrap().arrived;
    let after = rig.host.world().tick();
    assert!(after > last + 100, "no snapshot in the last second");
    assert!(watcher.seen.iter().all(|s| Some(s.header.flight) == flight));
}

/// Bytes a second the host sends `w`, and the ring's bytes, for the
/// slice's figures.
fn figures(rig: &mut Rig, w: usize) -> (u64, usize, usize) {
    let id = rig.connection(w);
    let up = rig
        .host
        .players()
        .into_iter()
        .find(|p| p.id == rig.host.peers[&id].lobby.id)
        .unwrap()
        .bytes_up_per_second;
    let ring = rig.host.stream.ring();
    (
        up,
        ring.map_or(0, Ring::bytes),
        ring.map_or(0, Ring::frames),
    )
}

use super::observe::Ring;

#[test]
fn the_crowds_bandwidth_and_the_rings_memory_are_measured() {
    // 15 against 15 with a 10-second delay: what an observer is sent, and
    // what the ring holds per second of delay.
    let mut rig = Rig::new(crowd(), |_| {});
    rig.delayed(10);
    let w = rig.watcher(Some(Subject::Aircraft(0)));
    rig.run(Duration::from_secs(14));
    let (up, bytes, frames) = figures(&mut rig, w);
    let per_frame = bytes / frames.max(1);
    let at_60 = per_frame * 60 * 30;
    eprintln!(
        "observer: {up} B/s; ring: {frames} frames, {bytes} bytes, {per_frame} a frame, \
         {:.1} MB at a 60-second delay",
        at_60 as f64 / 1e6
    );
    assert!(up > 0 && up < 40_000, "{up} B/s");
    assert!(at_60 < 16_000_000, "{at_60} bytes at 60 s");
}

#[test]
#[ignore = "slow (a minute of a 30-aircraft mission, about 20 s): the full suite runs it"]
fn the_ring_at_a_60_second_delay_for_30_aircraft() {
    let mut rig = Rig::new(crowd(), |_| {});
    rig.delayed(60);
    let w = rig.watcher(Some(Subject::Aircraft(0)));
    rig.run(Duration::from_secs(64));
    let (up, bytes, frames) = figures(&mut rig, w);
    eprintln!("observer: {up} B/s; ring: {frames} frames, {bytes} bytes");
    assert!((1795..=1802).contains(&frames), "{frames} frames");
    assert!(bytes < 16_000_000, "{bytes} bytes");
    assert!(rig.watchers[w].seen.len() > 60);
}
