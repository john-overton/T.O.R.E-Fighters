//! The host session on the network simulator, with scripted test clients
//! made of `tore-net`'s client and the wire's [`ClientConnection`], over a
//! mission built from synthetic resources.

use super::*;
use crate::wire::connection::ClientConnection;
use crate::wire::connection::FlightOrder;
use crate::wire::entity::{EntityKey, EntityState};
use crate::wire::events::ReceivedEvent;
use crate::wire::inputs::{Command, NumberedCommand};
use crate::wire::messages::{Goodbye, LobbyState, Mission, Roster, TakePlane};
use crate::wire::names::{NameIndex, NameTable};
use crate::wire::own_state::OwnStateHeader;
use crate::wire::snapshot::ReceivedSnapshot;
use crate::wire::{SECTION_EVENTS, SECTION_SNAPSHOT};
use std::collections::HashMap;
use tore_formats::aircraft::AircraftId;
use tore_net::sim::{LinkConfig, SimNetwork, SimSocket};
use tore_net::{Client, ClientConfig, ClientEvent, Entropy, RefuseReason};
use tore_sim::flight::{PilotCommand, Switch};
use tore_sim::models::AircraftModel;
use tore_world::mission::{Skill, Start};
use tore_world::snapshot::RenderSnapshot;
use tore_world::test_support::resources::{THEATER, resources};

const MS: Duration = Duration::from_millis(1);

fn host_address() -> SocketAddr {
    "10.0.0.1:26900".parse().unwrap()
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

fn build() -> BuildId {
    BuildId {
        version: "0.1.3-1-gtest".into(),
        commit: "test-commit".into(),
        release: false,
    }
}

fn config() -> HostConfig {
    HostConfig {
        entropy: Entropy::Seeded(11),
        ..HostConfig::new(build())
    }
}

fn model() -> AircraftModel {
    let map = resources();
    AircraftModel::for_aircraft(
        &tore_world::aircraft_type::AircraftType::load(&map, AircraftId::F18)
            .unwrap()
            .profile,
    )
    .unwrap()
}

/// A test client's scripted controls for a tick: a gentle weave, and the
/// trigger held for 30 ticks of every 240.
fn script(tick: u32) -> InputFrame {
    let t = f64::from(tick);
    InputFrame::of(
        &tore_sim::flight::PilotInput {
            pitch: (t * 0.02).sin() * 0.3,
            roll: (t * 0.013).cos() * 0.2,
            ..Default::default()
        },
        tick % 240 < 30,
        Default::default(),
    )
}

fn ready(mission: u32, plane: Option<u32>) -> Message {
    Message::TakePlane(TakePlane { mission, plane })
}

/// A minimal player's game: the transport's client and the wire's
/// connection state, with a scripted pilot.
struct TestClient {
    socket: SimSocket,
    client: Client,
    wire: Option<ClientConnection>,
    /// Ask for this plane (`Some(None)` for any) when the mission arrives.
    ready: Option<Option<u32>>,
    /// Send inputs while seated.
    flying: bool,
    /// Ticks the client runs ahead of the host.
    lead: u32,
    last_inputs: Option<Duration>,
    commands: Vec<NumberedCommand>,
    next_command: u16,
    model: AircraftModel,

    ticks_per_snapshot: u32,
    mission: Option<Mission>,
    roster: Option<Roster>,
    seated: Option<Seated>,
    seat_refused: Vec<String>,
    debrief: Option<Debrief>,
    ended: Option<MissionEnded>,
    closed: Option<CloseReason>,
    lobby: Option<LobbyState>,
    loadouts: Vec<Vec<(u32, tore_world::mission::LoadoutSpec)>>,
    refused: Vec<(u8, String)>,
    goodbye: Option<Goodbye>,
    /// Quit once the debrief is in.
    quitting: bool,
    snapshots: Vec<(SnapshotHeader, ReceivedSnapshot)>,
    own: Vec<(OwnStateHeader, ExactState)>,
    events: Vec<ReceivedEvent>,
    errors: Vec<String>,
    /// Every Scores message (phase 2, slice F2-S).
    scores: Vec<crate::wire::messages::Scores>,
    /// Every Results message (phase 2, slice F2-D).
    results: Vec<crate::wire::messages::Results>,
    /// Every Revival and Spawned message (phase 2, slice F2-V).
    revivals: Vec<crate::wire::messages::Revival>,
    spawned: Vec<crate::wire::messages::Spawned>,
    /// Every Observing message and Notice (phase 2, slice F2-A).
    observing: Vec<crate::wire::messages::Observing>,
    notices: Vec<String>,
    /// Every Token message (stage K, slice K5).
    tokens: Vec<crate::wire::migration::TokenGrant>,
}

impl TestClient {
    fn new(net: &SimNetwork, port: u16, configure: impl FnOnce(&mut ClientConfig)) -> Self {
        let address: SocketAddr = format!("10.0.0.2:{port}").parse().unwrap();
        let socket = net.bind(address).unwrap();
        let mut config = ClientConfig {
            game_version: build().version,
            game_commit: build().commit,
            entropy: Entropy::Seeded(u64::from(port)),
            ..ClientConfig::new(PROTOCOL_VERSION, "Viper")
        };
        configure(&mut config);
        let client = Client::connect(config, host_address(), net.now()).unwrap();
        Self {
            socket,
            client,
            wire: None,
            ready: Some(None),
            flying: true,
            lead: 6,
            last_inputs: None,
            commands: Vec::new(),
            next_command: 1,
            model: model(),
            ticks_per_snapshot: 4,
            mission: None,
            roster: None,
            seated: None,
            seat_refused: Vec::new(),
            debrief: None,
            ended: None,
            closed: None,
            lobby: None,
            loadouts: Vec::new(),
            refused: Vec::new(),
            goodbye: None,
            quitting: false,
            snapshots: Vec::new(),
            own: Vec::new(),
            events: Vec::new(),
            errors: Vec::new(),
            scores: Vec::new(),
            results: Vec::new(),
            revivals: Vec::new(),
            spawned: Vec::new(),
            observing: Vec::new(),
            notices: Vec::new(),
            tokens: Vec::new(),
        }
    }

    fn send(&mut self, message: &Message) {
        let body = message.encode().unwrap();
        self.client.send_message(message.kind(), &body).unwrap();
    }

    /// Gives a command, applied in the client's game at `tick`.
    fn command(&mut self, tick: u32, command: Command) {
        self.commands.push(NumberedCommand {
            number: self.next_command,
            tick,
            command,
        });
        self.next_command = self.next_command.wrapping_add(1);
    }

    /// Ends the flight and leaves the game once the debrief is in.
    fn leave(&mut self) {
        self.flying = false;
        self.quitting = true;
        self.send(&Message::Leave);
    }

    /// Ends the flight only: back in the lobby.
    fn leave_flight(&mut self) {
        self.flying = false;
        self.send(&Message::Leave);
    }

    /// The mission's number, as last sent.
    fn number(&self) -> u32 {
        self.mission.as_ref().map_or(0, |m| m.number)
    }

    /// Asks for a plane (stage D's Ready).
    fn take(&mut self, plane: Option<u32>) {
        let number = self.number();
        self.send(&ready(number, plane));
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
                    self.ticks_per_snapshot = u32::from(welcome.ticks_per_snapshot);
                    self.wire = Some(ClientConnection::new(self.ticks_per_snapshot));
                }
                ClientEvent::Closed(reason) => self.closed = Some(reason),
                ClientEvent::Connection(Event::Message { kind, body }) => {
                    match Message::decode(kind, &body) {
                        Ok(message) => self.message(message),
                        Err(error) => self.errors.push(format!("message: {error}")),
                    }
                }
                ClientEvent::Connection(Event::Payload { sections, .. }) => {
                    self.payload(sections);
                }
                ClientEvent::Connection(_) => {}
            }
        }
        if self.seated.is_some()
            && self.flying
            && self.closed.is_none()
            && self
                .last_inputs
                .is_none_or(|last| now - last >= Duration::from_millis(16))
        {
            self.send_inputs(now, host_tick);
        }
        let _ = self.client.transmit(&mut self.socket);
    }

    fn payload(&mut self, sections: Vec<tore_net::Section>) {
        let mut tick = None;
        let tps = self.ticks_per_snapshot;
        for section in sections {
            if let Some(flight) = ClientConnection::section_flight(section.kind, &section.body) {
                let current = self.wire.as_ref().and_then(|w| w.flight);
                match FlightOrder::of(flight, current) {
                    FlightOrder::Earlier => continue,
                    FlightOrder::Later => {
                        self.wire = Some(ClientConnection::for_flight(tps, flight));
                    }
                    FlightOrder::Same => {}
                }
            }
            let wire = self.wire.as_mut().unwrap();
            match section.kind {
                SECTION_SNAPSHOT => match wire.snapshot(&section.body) {
                    Ok((header, received)) => {
                        tick = Some(header.tick);
                        let applied = header.commands_applied;
                        self.commands.retain(|c| c.number > applied);
                        self.snapshots.push((header, received));
                    }
                    Err(error) => self.errors.push(format!("snapshot: {error}")),
                },
                SECTION_EVENTS => match wire.events(&section.body, tick.unwrap_or_default()) {
                    Ok(events) => self.events.extend(events),
                    Err(error) => self.errors.push(format!("events: {error}")),
                },
                SECTION_OWN_STATE => match wire.own_state(&section.body, &self.model) {
                    Ok(own) => self.own.push(own),
                    Err(error) => self.errors.push(format!("own state: {error}")),
                },
                other => self.errors.push(format!("section {other}")),
            }
        }
    }

    fn message(&mut self, message: Message) {
        match message {
            Message::Mission(mission) => {
                let first = self.mission.is_none();
                let number = mission.number;
                self.mission = Some(mission);
                if first && let Some(plane) = self.ready {
                    self.send(&ready(number, plane));
                }
            }
            Message::Roster(roster) => self.roster = Some(roster),
            Message::Seated(seated) => {
                let current = self.wire.as_ref().and_then(|w| w.flight);
                if FlightOrder::of(seated.flight, current) == FlightOrder::Later {
                    self.wire = Some(ClientConnection::for_flight(
                        self.ticks_per_snapshot,
                        seated.flight,
                    ));
                }
                self.roster = Some(seated.roster.clone());
                self.seated = Some(*seated);
            }
            Message::Lobby(lobby) => self.lobby = Some(*lobby),
            Message::Refused { request, reason } => self.refused.push((request, reason)),
            Message::Goodbye(goodbye) => self.goodbye = Some(goodbye),
            Message::FlightLoadouts(flight) => self.loadouts.push(flight.loadouts),
            Message::SeatRefused(reason) => self.seat_refused.push(reason),
            Message::Names(names) => {
                let wire = self.wire.as_mut().unwrap();
                match wire.names(&names) {
                    Ok(events) => self.events.extend(events),
                    Err(error) => self.errors.push(format!("names: {error}")),
                }
            }
            Message::Debrief(debrief) => {
                self.debrief = Some(*debrief);
                if self.quitting {
                    self.client.disconnect(DisconnectReason::Left);
                }
            }
            Message::MissionEnded(ended) => self.ended = Some(ended),
            Message::Notice(text) => self.notices.push(text),
            Message::Token(grant) => self.tokens.push(grant),
            Message::Scores(scores) => self.scores.push(*scores),
            Message::Results(results) => self.results.push(*results),
            Message::Revival(revival) => self.revivals.push(*revival),
            Message::Spawned(spawned) => self.spawned.push(*spawned),
            Message::Observing(observing) => self.observing.push(*observing),
            other => self.errors.push(format!("unexpected {other:?}")),
        }
    }

    /// Inputs up to `lead` ticks ahead of the host's next tick, repeating
    /// the last 24 ticks and every command not yet acknowledged.
    fn send_inputs(&mut self, now: Duration, host_tick: u64) {
        let seated = self.seated.as_ref().unwrap().tick;
        let newest = host_tick as u32 + self.lead;
        let oldest = newest.saturating_sub(23).max(seated + 1);
        if oldest > newest {
            return;
        }
        let section = InputsSection {
            flight: self.seated.as_ref().unwrap().flight,
            newest_tick: newest,
            frames: (oldest..=newest).map(script).collect(),
            view_offset: (self.lead + 12).min(255) as u8,
            interpolation_delay: 12,
            view_subject: None,
            mismatch: 0,
            // A command goes out once the client's game has reached its tick.
            commands: self
                .commands
                .iter()
                .filter(|c| c.tick <= newest)
                .take(64)
                .copied()
                .collect(),
        };
        let body = section.encode().unwrap();
        if self
            .client
            .send_payload(now, &[(SECTION_INPUTS, &body)])
            .is_ok()
        {
            self.last_inputs = Some(now);
        }
    }

    fn plane(&self) -> Option<PlaneId> {
        self.seated.as_ref().map(|s| PlaneId(s.plane))
    }

    fn name(&self, index: u16) -> String {
        self.wire
            .as_ref()
            .unwrap()
            .names
            .name(NameIndex(index))
            .unwrap()
            .to_owned()
    }
}

/// What the host's world held at a snapshot tick for one plane's seat.
struct Expected {
    entities: HashMap<EntityKey, Entity>,
    exact: ExactState,
    readout: crate::wire::readout::QReadout,
}

/// The host, its socket, the test clients and, when watching, what the
/// host's world held at each snapshot tick for each plane a human flies.
struct Rig {
    net: SimNetwork,
    host: Host,
    socket: SimSocket,
    clients: Vec<TestClient>,
    logs: Vec<HostLog>,
    next_port: u16,
    watch: bool,
    expected: HashMap<(u64, u32), Expected>,
    names: NameTable,
    pictures: HashMap<u32, RenderSnapshot>,
}

impl Rig {
    fn new(spec: MissionSpec, config: HostConfig, link: LinkConfig) -> Self {
        let net = SimNetwork::new(5);
        net.set_default_link(link);
        let socket = net.bind(host_address()).unwrap();
        let host = Host::new(spec, Arc::new(resources()), config).unwrap();
        Self {
            net,
            host,
            socket,
            clients: Vec::new(),
            logs: Vec::new(),
            next_port: 40_000,
            watch: false,
            expected: HashMap::new(),
            names: NameTable::new(),
            pictures: HashMap::new(),
        }
    }

    fn join(&mut self, configure: impl FnOnce(&mut ClientConfig)) -> usize {
        let client = TestClient::new(&self.net, self.next_port, configure);
        self.next_port += 1;
        self.clients.push(client);
        self.clients.len() - 1
    }

    /// One millisecond for everyone.
    fn step(&mut self) {
        self.net.advance(MS);
        let now = self.net.now();
        let before = self.host.world().tick();
        self.host.receive_from(now, &mut self.socket).unwrap();
        self.host.update(now);
        self.host.transmit(&mut self.socket).unwrap();
        while let Some(log) = self.host.poll_log() {
            self.logs.push(log);
        }
        let after = self.host.world().tick();
        if self.watch && after == before + 1 {
            self.record(before);
        }
        for client in &mut self.clients {
            client.pump(now, after);
        }
    }

    /// The host's state at snapshot tick `tick` for every plane a human
    /// flies, built the way the host builds a snapshot.
    fn record(&mut self, tick: u64) {
        // Each seat's snapshots come at its own phase of the interval.
        let tps = self.host.config().ticks_per_snapshot();
        let planes: Vec<u32> = self
            .host
            .peers
            .values()
            .filter(|p| p.stage == Stage::Seated)
            .filter_map(|p| Some((p.seat?, p.plane?)))
            .filter(|(seat, _)| tick % u64::from(tps) == crate::wire::snapshot_phase(seat.0, tps))
            .map(|(_, plane)| plane.0)
            .collect();
        for plane in planes {
            let world = self.host.world();
            let picture = from_world::seat_picture(world, PlaneId(plane)).unwrap();
            let entities =
                from_world::entities(&picture, self.pictures.get(&plane), plane, &mut self.names)
                    .unwrap();
            let cockpit = world.cockpits.iter().find(|c| c.plane.0 == plane).unwrap();
            let terms = self
                .host
                .out
                .terms
                .iter()
                .find(|(p, _)| p.0 == plane)
                .map(|(_, t)| *t);
            let exact = ExactState::of(&OwnPlane::of(cockpit), terms.as_ref());
            let readout = world
                .combat
                .cockpit_readout(
                    plane,
                    tore_world::combat::launcher(&cockpit.flight),
                    world.ai_wings.as_ref(),
                    Some(cockpit),
                )
                .unwrap();
            self.expected.insert(
                (tick, plane),
                Expected {
                    entities: entities.into_iter().map(|e| (e.key(), e)).collect(),
                    exact,
                    readout: crate::wire::readout::QReadout::of(&readout, tick as u32),
                },
            );
            self.pictures.insert(plane, picture);
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

    fn run(&mut self, time: Duration) {
        let end = self.net.now() + time;
        while self.net.now() < end {
            self.step();
        }
    }

    fn faults(&self) -> Vec<&HostLog> {
        self.logs
            .iter()
            .filter(|log| matches!(log, HostLog::Fault { .. }))
            .collect()
    }

    fn seated(&self, client: usize) -> bool {
        self.clients[client].seated.is_some()
    }

    fn closed(&self, client: usize) -> bool {
        self.clients[client].closed.is_some()
    }
}

/// An entity with a projectile's weapon and shape named rather than
/// numbered, since each end's name table numbers them.
fn named(
    entity: &Entity,
    name: impl Fn(u16) -> String,
) -> (Entity, Option<(String, Option<String>)>) {
    let mut plain = *entity;
    let mut names = None;
    if let EntityState::Projectile(p) = &mut plain.state {
        names = Some((name(p.weapon.0), p.shape.map(|s| name(s.0))));
        p.weapon = NameIndex::default();
        p.shape = None;
    }
    (plain, names)
}

#[test]
fn callsigns_get_a_suffix_within_fifteen_characters() {
    let taken = ["Viper", "Viper_2", "ABCDEFGHIJKLMNO"];
    assert_eq!(unique_callsign("Hornet", taken.iter().copied()), "Hornet");
    assert_eq!(unique_callsign("Viper", taken.iter().copied()), "Viper_3");
    assert_eq!(
        unique_callsign("ABCDEFGHIJKLMNO", taken.iter().copied()),
        "ABCDEFGHIJKLM_2"
    );
}

#[test]
fn a_waiting_mission_holds_at_tick_0_until_the_first_player_is_seated() {
    let mut rig = Rig::new(spec(2, 2, 20), config(), LinkConfig::one_way(10 * MS));
    let client = rig.join(|_| {});
    rig.clients[client].ready = None;
    rig.run(Duration::from_secs(1));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    // The lobby: the player, no slot held, the friendly planes as slots and
    // no King on a server.
    let lobby = rig.clients[client]
        .lobby
        .clone()
        .expect("the lobby's state");
    assert_eq!(lobby.phase, LobbyPhase::Lobby);
    assert_eq!(lobby.start, StartRule::FirstReady);
    assert_eq!(lobby.king, None);
    assert_eq!(lobby.players.len(), 1);
    assert_eq!(lobby.me().unwrap().callsign, "Viper");
    assert_eq!(
        lobby.slots.iter().map(|s| s.plane).collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(rig.host.world().tick(), 0);
    let c = &rig.clients[client];
    assert!(c.mission.is_some() && c.roster.is_some());
    assert!(
        c.snapshots.is_empty(),
        "no snapshots before the mission flies"
    );
    let mission = c.mission.as_ref().unwrap();
    assert_eq!(mission.host_tick, 0);
    assert_eq!(mission.spec().unwrap(), *rig.host.spec());
    assert_eq!(mission.manifest, *rig.host.manifest());
    assert!(!mission.manifest.entries.is_empty());
    assert_eq!(
        mission.contrail_sortie,
        rig.host.world().combat.contrail_sortie()
    );

    rig.clients[client].take(None);
    assert!(rig.run_until(Duration::from_secs(1), |r| r.seated(client)));
    assert_eq!(rig.host.phase(), Phase::Flying);
    let seated = rig.clients[client].seated.clone().unwrap();
    assert_eq!(
        (seated.seat, seated.plane),
        (0, 0),
        "Friendly Wing 1's lead first"
    );
    assert!(rig.logs.contains(&HostLog::MissionStarted { tick: 0 }));
}

#[test]
fn start_now_flies_with_nobody_connected() {
    let mut rig = Rig::new(
        spec(2, 2, 20),
        HostConfig {
            start: StartMode::Now,
            ..config()
        },
        LinkConfig::PERFECT,
    );
    rig.run(Duration::from_millis(500));
    // 500 ms at 120 a second, the first due at once.
    assert_eq!(rig.host.world().tick(), 60);
    assert!(rig.host.world().cockpits.is_empty());
    let status = rig.host.status(rig.net.now());
    assert_eq!(status.phase, Phase::Flying);
    assert_eq!(status.players, 0);
    assert_eq!(status.aircraft, 4);
    assert_eq!(status.capacity, 2, "the friendly planes");
    assert!(status.tick_cost_mean > Duration::ZERO);
    assert!(status.load > 0.);
}

#[test]
fn a_slow_host_catches_up_30_ticks_then_notes_an_overload() {
    let mut host = Host::new(
        spec(1, 1, 20),
        Arc::new(resources()),
        HostConfig {
            start: StartMode::Now,
            ..config()
        },
    )
    .unwrap();
    host.update(Duration::from_secs(1));
    assert_eq!(host.world().tick(), 1);
    // Half a second later: 60 ticks owed, 30 run.
    host.update(Duration::from_millis(1_500));
    assert_eq!(host.world().tick(), 31);
    let overload = std::iter::from_fn(|| host.poll_log())
        .find(|log| matches!(log, HostLog::Overloaded { .. }));
    assert_eq!(
        overload,
        Some(HostLog::Overloaded {
            tick: 31,
            ticks_behind: 30
        })
    );
    // The clock goes on from there: the next tick a tick's time later.
    assert!(host.next_wake(Duration::from_millis(1_500)) > Duration::ZERO);
    host.update(Duration::from_millis(1_500) + Duration::from_micros(8_400));
    assert_eq!(host.world().tick(), 32);
    assert_eq!(host.status(Duration::from_secs(2)).overloads, 1);
}

#[test]
fn a_seated_client_rebuilds_the_hosts_entities_and_own_state() {
    for (loss, duplicate) in [(0., 0.), (0.05, 0.05)] {
        let mut rig = Rig::new(
            spec(3, 3, 2),
            config(),
            LinkConfig::for_round_trip(Duration::from_millis(80), 0.2, loss, duplicate),
        );
        rig.watch = true;
        let client = rig.join(|_| {});
        assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(client)));
        rig.run(Duration::from_secs(4));
        let c = &rig.clients[client];
        assert!(c.errors.is_empty(), "{:?}", c.errors);
        let plane = c.plane().unwrap().0;

        // The seated state decodes with the plane's model.
        let seated = c.seated.as_ref().unwrap();
        seated.exact_state(&c.model).unwrap();
        assert!(!seated.loadout.stations.is_empty());

        assert!(c.snapshots.len() > 60, "{} snapshots", c.snapshots.len());
        let host_name = |i: u16| rig.names.name(NameIndex(i)).unwrap().to_owned();
        let mut checked = 0;
        for (header, received) in &c.snapshots {
            assert_eq!(
                u64::from(header.tick) % 4,
                crate::wire::snapshot_phase(seated.seat, 4)
            );
            assert!(received.unresolved.is_empty());
            let expected = &rig.expected[&(u64::from(header.tick), plane)];
            assert_eq!(header.own_hash, Some(expected.exact.hash().unwrap()));
            for entity in &received.updated {
                let want = &expected.entities[&entity.key()];
                assert_eq!(named(entity, |i| c.name(i)), named(want, host_name));
                checked += 1;
            }
            for key in &received.removed {
                assert!(!expected.entities.contains_key(key));
            }
        }
        assert!(checked > 200, "{checked} entity states checked");
        // Every snapshot carries the seat's cockpit readout, and once the
        // first second has brought it across the client holds the host's
        // readout of that tick exactly.
        let mut readouts = 0;
        for (index, (header, received)) in c.snapshots.iter().enumerate() {
            assert!(!received.readout_unresolved);
            let readout = received.readout.as_ref().expect("a readout");
            let expected = &rig.expected[&(u64::from(header.tick), plane)];
            if index >= 30 {
                assert_eq!(*readout, expected.readout, "tick {}", header.tick);
                readouts += 1;
            }
        }
        assert!(readouts > 30, "{readouts} readouts checked");
        // And it reads back as a cockpit readout for the client's frame.
        let scene = &rig.host.world().terrain.airport_scene;
        let own = &rig
            .host
            .world()
            .cockpits
            .iter()
            .find(|c| c.plane.0 == plane)
            .unwrap()
            .flight;
        let readout = c
            .wire
            .as_ref()
            .unwrap()
            .cockpit_readout(own, Some(scene))
            .unwrap()
            .unwrap();
        assert_eq!(readout.plane, plane);
        // The exact states decode to the host's, bit for bit.
        assert!(!c.own.is_empty());
        for (header, state) in &c.own {
            let expected = &rig.expected[&(u64::from(header.tick), plane)];
            assert_eq!(*state, expected.exact);
            assert_eq!(state.hash().unwrap(), expected.exact.hash().unwrap());
        }
        // At least once a second, and more often when the host repeated an
        // input or the plane fired.
        let ticks: Vec<u32> = c.own.iter().map(|(h, _)| h.tick).collect();
        if loss == 0. {
            assert!(ticks.windows(2).all(|w| w[1] - w[0] <= 120), "{ticks:?}");
        }
        assert!(rig.faults().is_empty());
    }
}

#[test]
fn clients_join_fly_and_leave_a_hundred_times() {
    let mut rig = Rig::new(
        spec(2, 2, 20),
        config(),
        LinkConfig::for_round_trip(Duration::from_millis(40), 0.1, 0.0, 0.0),
    );
    for round in 0..100u32 {
        // Every third round asks for the lead's wingman by number.
        let wanted = (round % 3 == 2).then_some(1);
        let client = rig.join(|c| c.callsign = format!("Pilot{round}"));
        rig.clients[client].ready = Some(wanted);
        assert!(
            rig.run_until(Duration::from_secs(3), |r| r.seated(client)),
            "round {round}: seated"
        );
        let plane = rig.clients[client].plane().unwrap();
        assert_eq!(plane.0, wanted.unwrap_or(0), "round {round}");
        let seat = SeatId(rig.clients[client].seated.as_ref().unwrap().seat);
        assert_eq!(
            rig.host.world().roster.plane(plane).unwrap().pilot,
            Pilot::Human(seat)
        );
        rig.run(Duration::from_millis(250));
        rig.clients[client].leave();
        assert!(
            rig.run_until(Duration::from_secs(6), |r| r.closed(client)),
            "round {round}: closed"
        );
        let c = rig.clients.remove(client);
        assert!(c.errors.is_empty(), "round {round}: {:?}", c.errors);
        assert!(c.debrief.is_some(), "round {round}: debrief");
        assert!(!c.snapshots.is_empty(), "round {round}: snapshots");
        assert_eq!(
            c.closed,
            Some(CloseReason::Disconnected {
                reason: DisconnectReason::Left,
                by_peer: false
            }),
            "round {round}"
        );
        assert_eq!(
            rig.host.world().roster.plane(plane).unwrap().pilot,
            Pilot::Ai,
            "round {round}: the plane went back to the AI"
        );
    }
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
    // The last one's goodbye reaches the host.
    rig.run(Duration::from_millis(100));
    // Each left its flight, was back in the lobby, then left the game.
    let count = |f: &dyn Fn(&HostLog) -> bool| rig.logs.iter().filter(|l| f(l)).count();
    assert_eq!(
        count(&|l| matches!(
            l,
            HostLog::Lobby {
                event: LobbyEvent::BackInLobby,
                ..
            }
        )),
        100
    );
    assert_eq!(
        count(&|l| matches!(
            l,
            HostLog::Left {
                reason: LeaveReason::Left,
                ..
            }
        )),
        100
    );
    assert!(rig.host.world().cockpits.is_empty());
}

#[test]
fn two_players_fly_together_and_see_each_other() {
    let mut rig = Rig::new(spec(2, 2, 20), config(), LinkConfig::one_way(20 * MS));
    let a = rig.join(|c| c.callsign = "Viper".into());
    let b = rig.join(|c| c.callsign = "Viper".into());
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(a) && r.seated(b)));
    rig.run(Duration::from_secs(1));
    let (pa, pb) = (
        rig.clients[a].plane().unwrap(),
        rig.clients[b].plane().unwrap(),
    );
    assert_ne!(pa, pb);
    let roster = rig.clients[a].roster.clone().unwrap();
    let humans: Vec<&RosterPilot> = roster
        .planes
        .iter()
        .map(|p| &p.pilot)
        .filter(|p| matches!(p, RosterPilot::Human { .. }))
        .collect();
    assert_eq!(humans.len(), 2);
    assert!(
        humans
            .iter()
            .any(|p| matches!(p, RosterPilot::Human { callsign, .. } if callsign == "Viper_2"))
    );
    // Each one's snapshots carry the other's plane as an aircraft.
    for (me, other) in [(a, pb), (b, pa)] {
        let wire = rig.clients[me].wire.as_ref().unwrap();
        let key = EntityKey {
            kind: EntityKind::Aircraft,
            id: other.0,
        };
        assert!(wire.entities.latest(key).is_some());
    }
    let players = rig.host.players();
    assert_eq!(players.len(), 2);
    assert!(
        players
            .iter()
            .all(|p| p.seat.is_some() && p.input_margin_ticks.is_some())
    );
    assert!(
        players
            .iter()
            .all(|p| p.round_trip > Duration::from_millis(30))
    );
}

#[test]
fn a_silent_player_is_dropped_and_its_plane_goes_back_without_a_debrief() {
    let mut rig = Rig::new(spec(2, 2, 20), config(), LinkConfig::one_way(10 * MS));
    let client = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(client)));
    let plane = rig.clients[client].plane().unwrap();
    // The player's game goes silent.
    let silent = rig.clients.remove(client);
    assert!(rig.run_until(Duration::from_secs(7), |r| {
        r.logs.iter().any(|l| matches!(l, HostLog::Left { .. }))
    }));
    assert!(rig.logs.iter().any(|l| matches!(
        l,
        HostLog::Left {
            reason: LeaveReason::Silent,
            plane: Some(0),
            ..
        }
    )));
    rig.run(Duration::from_millis(50));
    assert_eq!(
        rig.host.world().roster.plane(plane).unwrap().pilot,
        Pilot::Ai
    );
    assert!(silent.debrief.is_none());
}

#[test]
fn a_kicked_player_loses_its_plane_with_no_debrief() {
    let mut rig = Rig::new(spec(2, 2, 20), config(), LinkConfig::one_way(10 * MS));
    let client = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(client)));
    assert_eq!(rig.host.kick(7), Err(CommandError::NoSuchSeat(7)));
    rig.host.kick(0).unwrap();
    assert!(rig.run_until(Duration::from_secs(1), |r| r.closed(client)));
    let c = &rig.clients[client];
    assert_eq!(
        c.closed,
        Some(CloseReason::Disconnected {
            reason: DisconnectReason::Kicked,
            by_peer: true
        })
    );
    assert!(c.debrief.is_none());
    rig.run(Duration::from_millis(20));
    assert_eq!(
        rig.host.world().roster.plane(PlaneId(0)).unwrap().pilot,
        Pilot::Ai
    );
    assert!(rig.logs.iter().any(|l| matches!(
        l,
        HostLog::Left {
            reason: LeaveReason::Kicked,
            ..
        }
    )));
}

/// A host with `config`, and a client configured by `configure` that tries
/// to join it for up to two seconds.
fn try_join(config: HostConfig, configure: impl FnOnce(&mut ClientConfig)) -> (Rig, usize) {
    let mut rig = Rig::new(spec(2, 2, 20), config, LinkConfig::one_way(5 * MS));
    let client = rig.join(configure);
    rig.run_until(Duration::from_secs(2), |r| {
        r.closed(client) || r.seated(client)
    });
    (rig, client)
}

fn refused(rig: &Rig, client: usize) -> (RefuseReason, String) {
    match &rig.clients[client].closed {
        Some(CloseReason::Refused { reason, text }) => (*reason, text.clone()),
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn refusals_reach_the_client_with_their_reason() {
    let locked = HostConfig {
        password: Some("secret".into()),
        ..config()
    };
    // A wrong password, then the right one.
    let (rig, client) = try_join(locked.clone(), |c| c.password = "guess".into());
    assert_eq!(refused(&rig, client).0, RefuseReason::WrongPassword);
    assert!(
        rig.logs
            .iter()
            .any(|l| matches!(l, HostLog::Refused { .. }))
    );
    let (rig, client) = try_join(locked, |c| c.password = "secret".into());
    assert!(rig.seated(client), "the right password joins");

    // A different build.
    let (rig, client) = try_join(config(), |c| {
        c.game_version = "0.1.2".into();
        c.game_commit = "other".into();
    });
    let (reason, text) = refused(&rig, client);
    assert_eq!(reason, RefuseReason::GameBuild);
    assert!(text.contains("0.1.3-1-gtest"), "{text}");

    // A game of protocol 7, before stage F phase 2's messages, is refused
    // by its version, naming both, before it can send an answer.
    let (rig, client) = try_join(config(), |c| c.protocol_version = 7);
    let (reason, text) = refused(&rig, client);
    assert_eq!(reason, RefuseReason::ProtocolVersion);
    assert!(
        text.contains(&format!("protocol version {PROTOCOL_VERSION}"))
            && text.contains("uses version 7"),
        "{text}"
    );
    assert!(
        !rig.logs
            .iter()
            .any(|l| matches!(l, HostLog::Connected { .. }))
    );

    // Full: one player at most.
    let (mut rig, _) = try_join(
        HostConfig {
            max_players: 1,
            ..config()
        },
        |_| {},
    );
    let second = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.closed(second)));
    assert_eq!(refused(&rig, second).0, RefuseReason::ServerFull);

    // Content: the client reports a difference and stays in the lobby,
    // marked unable, and may not take a plane.
    let mut rig = Rig::new(spec(2, 2, 20), config(), LinkConfig::one_way(5 * MS));
    let client = rig.join(|_| {});
    rig.clients[client].ready = None;
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.clients[client].mission.is_some()
    }));
    let mission = rig.clients[client].number();
    rig.clients[client].send(&Message::ContentRefused(
        crate::wire::messages::ContentRefused {
            mission,
            names: vec!["F18.PT".into()],
            reason: "Your game data differs from the host's in 1 file(s), such as F18.PT.".into(),
            flight: false,
        },
    ));
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.clients[client]
            .lobby
            .as_ref()
            .and_then(|l| l.me())
            .is_some_and(|me| me.unable.is_some())
    }));
    rig.clients[client].take(None);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        !r.clients[client].seat_refused.is_empty()
    }));
    assert!(rig.clients[client].seat_refused[0].contains("cannot play"));
    assert!(!rig.closed(client));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    assert!(rig.logs.iter().any(|l| matches!(
        l,
        HostLog::ContentRefused { names, .. } if names == &["F18.PT".to_string()]
    )));
}

#[test]
fn a_taken_or_closed_plane_is_refused_and_the_player_may_ask_again() {
    let mut rig = Rig::new(spec(2, 2, 20), config(), LinkConfig::one_way(5 * MS));
    let first = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(first)));
    let second = rig.join(|_| {});
    rig.clients[second].ready = Some(Some(0));
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        !r.clients[second].seat_refused.is_empty()
    }));
    assert!(rig.clients[second].seat_refused[0].contains("another player"));
    // An enemy plane is not open by default.
    rig.clients[second].take(Some(2));
    assert!(rig.run_until(
        Duration::from_secs(1),
        |r| r.clients[second].seat_refused.len() == 2
    ));
    assert!(rig.clients[second].seat_refused[1].contains("not open"));
    rig.clients[second].take(None);
    assert!(rig.run_until(Duration::from_secs(1), |r| r.seated(second)));
    assert_eq!(rig.clients[second].plane(), Some(PlaneId(1)));
    // Both friendly planes are flown: the host is full.
    let third = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.closed(third)));
    assert_eq!(refused(&rig, third).0, RefuseReason::ServerFull);
}

#[test]
fn a_command_repeated_in_many_packets_toggles_once() {
    let mut rig = Rig::new(
        spec(1, 1, 20),
        config(),
        LinkConfig::for_round_trip(Duration::from_millis(60), 0.3, 0.1, 0.3),
    );
    let client = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(client)));
    let gear = |rig: &Rig| rig.host.world().cockpits[0].flight.gear_down;
    let before = gear(&rig);
    let tick = rig.host.world().tick() as u32 + 30;
    rig.clients[client].command(tick, Command::Pilot(PilotCommand::Toggle(Switch::Gear)));
    rig.run(Duration::from_secs(2));
    assert_ne!(gear(&rig), before, "toggled");
    assert!(rig.clients[client].commands.is_empty(), "acknowledged");
    rig.run(Duration::from_secs(1));
    assert_ne!(gear(&rig), before, "and only once");
    // A second toggle, given for a tick already stepped, applies at the next
    // tick and the exact state goes out.
    let own_states = rig.clients[client].own.len();
    let late = rig.host.world().tick() as u32 - 10;
    rig.clients[client].command(late, Command::Pilot(PilotCommand::Toggle(Switch::Gear)));
    rig.run(Duration::from_secs(1));
    assert_eq!(gear(&rig), before);
    assert!(rig.clients[client].own.len() > own_states);
}

/// Slice F2-V replaced the departed players' orphans: a lost plane whose
/// player leaves the game is abandoned to the mission (host/revive_tests.rs
/// has the rest).
#[test]
fn a_player_whose_plane_cannot_go_back_leaves_it_where_it_is() {
    let mut rig = Rig::new(spec(2, 2, 20), config(), LinkConfig::one_way(5 * MS));
    let client = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(client)));
    // The pilot is lost (the synthetic aircraft has no ejection seat, so
    // the test kills the pilot outright).
    rig.host.world.cockpits[0].flight.systems.pilot.dead = true;
    rig.run(Duration::from_millis(100));
    rig.clients[client].leave();
    assert!(rig.run_until(Duration::from_secs(6), |r| r.closed(client)));
    let debrief = rig.clients[client].debrief.clone().unwrap();
    assert_eq!(debrief.player.status, PilotStatus::Dead);
    // The plane is abandoned to the mission once its player has gone, and
    // flies on with nobody.
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        r.host.world().roster.plane(PlaneId(0)).unwrap().pilot == Pilot::Lost
    }));
    assert!(
        rig.host
            .world()
            .cockpits
            .iter()
            .any(|c| c.plane == PlaneId(0))
    );
    rig.run(Duration::from_millis(500));
    // A new player gets the free seat and another plane.
    let next = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(next)));
    let seated = rig.clients[next].seated.clone().unwrap();
    assert_eq!((seated.seat, seated.plane), (0, 1));
    let lost = seated
        .roster
        .planes
        .iter()
        .find(|p| p.id == 0)
        .unwrap()
        .pilot
        .clone();
    assert_eq!(lost, RosterPilot::Ai, "the roster names nobody for it");
    // Asking for the lost plane says why, not that someone flies it.
    let third = rig.join(|_| {});
    rig.clients[third].ready = Some(Some(0));
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        !r.clients[third].seat_refused.is_empty()
    }));
    assert_eq!(
        rig.clients[third].seat_refused[0],
        "Plane 0 is destroyed or has lost its pilot."
    );
    assert!(rig.faults().is_empty(), "{:?}", rig.faults());
}

#[test]
fn the_time_limit_ends_the_mission_with_debriefs_and_it_starts_again() {
    let mut rig = Rig::new(
        spec(2, 2, 20),
        HostConfig {
            time_limit: Some(Duration::from_secs(2)),
            restart_delay: Duration::from_secs(1),
            ..config()
        },
        LinkConfig::one_way(5 * MS),
    );
    let client = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.clients[client].debrief.is_some()
    }));
    let c = &rig.clients[client];
    assert_eq!(
        c.ended,
        Some(MissionEnded {
            reason: EndReason::TimeLimit,
            next_in_seconds: Some(1)
        })
    );
    // The player stays connected, back in the lobby, its slot kept.
    assert!(!rig.closed(client));
    assert!(matches!(
        rig.host.phase(),
        Phase::Ended { next_in: Some(_) }
    ));
    assert!(
        rig.logs.contains(&HostLog::MissionEnded {
            reason: EndReason::TimeLimit,
            tick: 240
        }),
        "{:?}",
        rig.logs
    );
    // While ended, a join is refused as "shutting down".
    let late = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_millis(500), |r| r.closed(late)));
    assert_eq!(refused(&rig, late).0, RefuseReason::ShuttingDown);
    // The fresh mission waits in the lobby for its first ready player.
    assert!(rig.run_until(Duration::from_secs(2), |r| r.host.phase() == Phase::Lobby));
    assert_eq!(rig.host.world().tick(), 0);
    assert!(
        rig.logs
            .iter()
            .any(|l| matches!(l, HostLog::MissionRestarted { .. }))
    );
    assert!(rig.run_until(Duration::from_millis(200), |r| {
        r.clients[client]
            .lobby
            .as_ref()
            .is_some_and(|l| l.phase == LobbyPhase::Lobby && l.me().unwrap().slot == Some(0))
    }));
    let me = rig.clients[client]
        .lobby
        .as_ref()
        .unwrap()
        .me()
        .unwrap()
        .clone();
    assert!(!me.ready && !me.flying, "{me:?}");
    // The same player flies the next mission, a new flight of its
    // connection.
    rig.clients[client].take(None);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.clients[client]
            .seated
            .as_ref()
            .is_some_and(|s| s.flight == 2)
    }));
    let again = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(again)));
    assert!(
        rig.clients[client].errors.is_empty(),
        "{:?}",
        rig.clients[client].errors
    );
}

#[test]
fn an_empty_mission_ends_after_the_empty_timeout() {
    let mut rig = Rig::new(
        spec(2, 2, 20),
        HostConfig {
            empty_timeout: Duration::from_secs(1),
            after_end: AfterEnd::Quit,
            ..config()
        },
        LinkConfig::one_way(5 * MS),
    );
    let client = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(client)));
    rig.clients[client].leave();
    assert!(rig.run_until(Duration::from_secs(2), |r| r.closed(client)));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.host.phase() == Phase::Stopped));
    // The player left at about tick 4; the mission flew a second more.
    let left = rig.logs.iter().find_map(|l| match l {
        HostLog::Left { tick, .. } => Some(*tick),
        _ => None,
    });
    let ended = rig.logs.iter().find_map(|l| match l {
        HostLog::MissionEnded { reason, tick } => Some((*reason, *tick)),
        _ => None,
    });
    let (reason, tick) = ended.unwrap();
    assert_eq!(reason, EndReason::EveryoneLeft);
    assert!(
        (120..140).contains(&(tick - left.unwrap() + 12)),
        "{:?}",
        rig.logs
    );
    assert!(
        rig.logs
            .iter()
            .any(|l| matches!(l, HostLog::Stopped { .. }))
    );
}

#[test]
fn the_console_ends_restarts_and_stops_the_mission() {
    let mut rig = Rig::new(spec(2, 2, 20), config(), LinkConfig::one_way(5 * MS));
    let a = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(a)));
    rig.run(Duration::from_millis(200));
    rig.host.end();
    assert!(rig.run_until(Duration::from_secs(2), |r| r.clients[a].debrief.is_some()));
    assert_eq!(
        rig.clients[a].ended,
        Some(MissionEnded {
            reason: EndReason::EndedByServer,
            next_in_seconds: Some(30)
        })
    );
    assert!(matches!(rig.host.phase(), Phase::Ended { .. }));

    // Restart starts the fresh mission at once, back in the lobby with the
    // player still connected.
    rig.host.restart();
    assert_eq!(rig.host.phase(), Phase::Lobby);
    assert_eq!(rig.host.world().tick(), 0);
    let b = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(b)));
    rig.clients[b].debrief = None;
    rig.host.restart();
    assert!(rig.run_until(Duration::from_secs(2), |r| r.clients[b].debrief.is_some()));
    assert_eq!(
        rig.clients[b].ended.map(|e| e.next_in_seconds),
        Some(Some(0))
    );
    assert!(!rig.closed(a) && !rig.closed(b));

    // Quit: everyone is told the server is stopping. (The two slots are
    // held, so one player gives its up first.)
    rig.clients[a].client.disconnect(DisconnectReason::Left);
    assert!(rig.run_until(Duration::from_secs(1), |r| r.closed(a)));
    let c = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(c)));
    rig.host.stop();
    assert_eq!(rig.host.phase(), Phase::Stopped);
    assert!(rig.run_until(Duration::from_secs(1), |r| r.closed(c)));
    assert_eq!(
        rig.clients[c].closed,
        Some(CloseReason::Disconnected {
            reason: DisconnectReason::ServerStopping,
            by_peer: true
        })
    );
    assert!(rig.faults().is_empty());
}

#[test]
fn every_player_hears_of_bursts_and_only_its_own_rumbles() {
    let mut rig = Rig::new(spec(2, 2, 20), config(), LinkConfig::one_way(5 * MS));
    let a = rig.join(|_| {});
    let b = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(a) && r.seated(b)));
    // The script holds the trigger for 30 ticks of every 240.
    rig.run(Duration::from_secs(5));
    let shooter = rig.clients[a].plane().unwrap().0;
    for me in [a, b] {
        let bursts: Vec<&WireEvent> = rig.clients[me]
            .events
            .iter()
            .map(|e| &e.event)
            .filter(|e| matches!(e, WireEvent::GunBurst { shooter: s, .. } if *s == shooter))
            .collect();
        assert!(
            bursts
                .iter()
                .any(|e| matches!(e, WireEvent::GunBurst { length: None, .. })),
            "a burst starts"
        );
        assert!(
            bursts
                .iter()
                .any(|e| matches!(e, WireEvent::GunBurst { length: Some(n), .. } if *n > 1)),
            "and ends with its length: {bursts:?}"
        );
    }
    // The seat's own gun rumbles; the synthetic gun has no fire sound, so
    // no release event comes with it.
    let rumbles = rig.clients[a]
        .events
        .iter()
        .filter(|e| {
            matches!(
                e.event,
                WireEvent::Feedback {
                    rumble: crate::wire::events::Rumble::GunFired
                }
            )
        })
        .count();
    assert!(rumbles > 0, "the seat's own gun rumbles");
}

#[test]
fn each_seats_snapshots_have_their_own_phase_of_the_interval() {
    let mut rig = Rig::new(spec(5, 2, 20), config(), LinkConfig::one_way(5 * MS));
    let clients: Vec<usize> = (0..5).map(|_| rig.join(|_| {})).collect();
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        clients.iter().all(|&c| r.seated(c))
    }));
    for &c in &clients {
        rig.clients[c].snapshots.clear();
    }
    rig.run(Duration::from_secs(3));
    let mut phases = Vec::new();
    for &c in &clients {
        let client = &rig.clients[c];
        let seat = client.seated.as_ref().unwrap().seat;
        let phase = crate::wire::snapshot_phase(seat, 4);
        // 30 a second, every one at this seat's phase of the four ticks.
        assert!(client.snapshots.len() >= 85, "{}", client.snapshots.len());
        for (header, _) in &client.snapshots {
            assert_eq!(u64::from(header.tick) % 4, phase, "seat {seat}");
        }
        phases.push(phase);
    }
    // Five seats over four phases: every phase is used, and none holds more
    // than two seats.
    for phase in 0..4 {
        let n = phases.iter().filter(|&&p| p == phase).count();
        assert!((1..=2).contains(&n), "phase {phase}: {n} seats");
    }
}

#[test]
fn a_player_who_leaves_its_flight_stays_and_flies_again_as_a_new_flight() {
    let mut rig = Rig::new(spec(2, 2, 20), config(), LinkConfig::one_way(10 * MS));
    let a = rig.join(|_| {});
    let b = rig.join(|_| {});
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(a) && r.seated(b)));
    rig.run(Duration::from_millis(500));
    let plane = rig.clients[a].plane().unwrap();
    rig.clients[a].leave_flight();
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.clients[a].debrief.is_some()
            && r.clients[a]
                .lobby
                .as_ref()
                .and_then(|l| l.me())
                .is_some_and(|me| !me.flying && me.slot == Some(plane.0))
    }));
    assert!(!rig.closed(a));
    assert_eq!(
        rig.host.phase(),
        Phase::Flying,
        "the mission flies on for b"
    );
    assert_eq!(
        rig.host.world().roster.plane(plane).unwrap().pilot,
        Pilot::Ai
    );
    // Back in flight: the same plane, a new flight of the connection whose
    // sections code against nothing of the last.
    let snapshots = rig.clients[a].snapshots.len();
    rig.clients[a].flying = true;
    rig.clients[a].take(None);
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.clients[a].seated.as_ref().is_some_and(|s| s.flight == 2)
    }));
    assert_eq!(rig.clients[a].plane(), Some(plane), "its slot's plane");
    rig.run(Duration::from_secs(1));
    let c = &rig.clients[a];
    assert!(c.snapshots.len() > snapshots + 20);
    assert!(c.snapshots[snapshots..].iter().all(|(h, _)| h.flight == 2));
    assert!(c.errors.is_empty(), "{:?}", c.errors);
    assert!(rig.faults().is_empty());
    // A player who leaves only its flight never leaves the game.
    rig.clients[b].leave_flight();
    rig.run(Duration::from_secs(1));
    assert!(!rig.closed(b));
}

#[path = "worker_tests.rs"]
mod worker_tests;

#[path = "score_tests.rs"]
mod score_tests;

#[path = "revive_tests.rs"]
mod revive_tests;

#[path = "results_tests.rs"]
mod results_tests;

#[path = "away_tests.rs"]
mod away_tests;

#[path = "path_tests.rs"]
mod path_tests;

#[path = "rejoin_tests.rs"]
mod rejoin_tests;
