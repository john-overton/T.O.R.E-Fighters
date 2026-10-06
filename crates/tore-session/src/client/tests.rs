//! The client session against a `Host` on the network simulator, over a
//! mission built from synthetic resources.

use super::*;
use crate::host::{Host, HostConfig, HostLog};
use std::collections::HashMap;
use tore_net::sim::{LinkConfig, SimNetwork, SimSocket};
use tore_world::mission::{Skill, Start};
use tore_world::test_support::resources::{THEATER, resources};

const MS: Duration = Duration::from_millis(1);

pub(super) fn host_address() -> SocketAddr {
    "10.0.0.1:26900".parse().unwrap()
}

pub(super) fn build() -> BuildId {
    BuildId {
        version: "0.1.3-1-gtest".into(),
        commit: "test-commit".into(),
        release: false,
    }
}

/// Friendly Wing 1 of `friendly` and the enemy's Wing 1 of `enemy`, the
/// enemy `separation_nm` ahead, airborne at 10,000 feet.
pub(super) fn spec(friendly: usize, enemy: usize, separation_nm: u32) -> MissionSpec {
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

/// A gentle weave with the throttle held: the plane stays airborne.
pub(super) fn weave(t: f64) -> Controls {
    Controls {
        pilot: PilotInput {
            pitch: (t * 0.7).sin() * 0.15,
            roll: (t * 0.45).cos() * 0.2,
            ..PilotInput::default()
        },
        ..Controls::default()
    }
}

type Script = Box<dyn FnMut(Duration, &Client, Option<&RenderSnapshot>) -> Controls>;

/// One player's game on the simulator.
pub(super) struct Player {
    pub socket: SimSocket,
    pub client: Client,
    pub script: Script,
    pub events: Vec<ClientEvent>,
    pub frames: Vec<ClientFrame>,
    pub keep_frames: bool,
    pub frame_every: Duration,
    pub last_frame: Option<Duration>,
    pub digests: Vec<u64>,
    pub picture: Option<RenderSnapshot>,
    /// Its game has stalled: nothing is received, stepped or sent.
    pub stalled: bool,
}

/// A host and its players on one simulated network.
pub(super) struct Rig {
    pub net: SimNetwork,
    pub host: Host,
    pub socket: SimSocket,
    pub players: Vec<Player>,
    pub logs: Vec<HostLog>,
    pub next_port: u16,
    /// Each aircraft's position after each host tick, when watching.
    pub watch: bool,
    pub truth: HashMap<(u64, u32), [f64; 3]>,
    pub resources: Arc<BTreeMap<String, Vec<u8>>>,
}

impl Rig {
    pub fn new(spec: MissionSpec, link: LinkConfig, seed: u64) -> Self {
        Self::with_config(spec, link, seed, |_| {})
    }

    /// [`Rig::new`] with the host's settings changed by `configure`.
    pub fn with_config(
        spec: MissionSpec,
        link: LinkConfig,
        seed: u64,
        configure: impl FnOnce(&mut HostConfig),
    ) -> Self {
        Self::with_import(spec, link, seed, resources(), configure)
    }

    /// [`Rig::with_config`] with the host's and the players' import.
    pub fn with_import(
        spec: MissionSpec,
        link: LinkConfig,
        seed: u64,
        import: BTreeMap<String, Vec<u8>>,
        configure: impl FnOnce(&mut HostConfig),
    ) -> Self {
        let net = SimNetwork::new(seed);
        net.set_default_link(link);
        let socket = net.bind(host_address()).unwrap();
        let resources = Arc::new(import);
        let mut config = HostConfig {
            entropy: Entropy::Seeded(11),
            ..HostConfig::new(build())
        };
        configure(&mut config);
        let host = Host::new(spec, Arc::clone(&resources), config).unwrap();
        Self {
            net,
            host,
            socket,
            players: Vec::new(),
            logs: Vec::new(),
            next_port: 40_000,
            watch: false,
            truth: HashMap::new(),
            resources,
        }
    }

    pub fn join(&mut self, configure: impl FnOnce(&mut ClientConfig), script: Script) -> usize {
        let address: SocketAddr = format!("10.0.0.2:{}", self.next_port).parse().unwrap();
        let socket = self.net.bind(address).unwrap();
        let mut config = ClientConfig {
            entropy: Entropy::Seeded(u64::from(self.next_port)),
            ..ClientConfig::new(host_address(), "Viper", build())
        };
        configure(&mut config);
        self.next_port += 1;
        let client = Client::connect(config, Arc::clone(&self.resources), self.net.now()).unwrap();
        self.players.push(Player {
            socket,
            client,
            script,
            events: Vec::new(),
            frames: Vec::new(),
            keep_frames: false,
            frame_every: Duration::from_millis(16),
            last_frame: None,
            digests: Vec::new(),
            picture: None,
            stalled: false,
        });
        self.players.len() - 1
    }

    /// The address the `n`th player joins from.
    pub fn player_address(n: usize) -> SocketAddr {
        format!("10.0.0.2:{}", 40_000 + n).parse().unwrap()
    }

    /// One millisecond for everyone.
    pub fn step(&mut self) {
        self.net.advance(MS);
        let now = self.net.now();
        let before = self.host.world().tick();
        self.host.receive_from(now, &mut self.socket).unwrap();
        self.host.update(now);
        self.host.transmit(&mut self.socket).unwrap();
        while let Some(log) = self.host.poll_log() {
            self.logs.push(log);
        }
        if self.watch && self.host.world().tick() > before {
            self.record();
        }
        for player in &mut self.players {
            if player.stalled {
                continue;
            }
            player.client.receive_from(now, &mut player.socket).unwrap();
            let controls = (player.script)(now, &player.client, player.picture.as_ref());
            player.client.update(now, &controls);
            if player
                .last_frame
                .is_none_or(|last| now - last >= player.frame_every)
            {
                player.last_frame = Some(now);
                if let Some(frame) = player.client.frame(now) {
                    player.digests.push(frame.digest());
                    player.picture = Some(frame.picture.clone());
                    if player.keep_frames {
                        player.frames.push(frame);
                    }
                }
            }
            player.client.transmit(&mut player.socket).unwrap();
            while let Some(event) = player.client.poll_event() {
                player.events.push(event);
            }
        }
    }

    /// Where every aircraft is after the host's last tick.
    fn record(&mut self) {
        let world = self.host.world();
        let tick = world.tick() - 1;
        for cockpit in &world.cockpits {
            self.truth
                .insert((tick, cockpit.plane.0), cockpit.flight.position);
        }
        if let Some(wings) = &world.ai_wings {
            for actor in wings.mission().actors() {
                self.truth
                    .insert((tick, actor.id()), actor.flight().position);
            }
        }
    }

    pub fn run_until(&mut self, limit: Duration, mut done: impl FnMut(&Rig) -> bool) -> bool {
        let end = self.net.now() + limit;
        while self.net.now() < end {
            self.step();
            if done(self) {
                return true;
            }
        }
        false
    }

    pub fn run(&mut self, time: Duration) {
        let end = self.net.now() + time;
        while self.net.now() < end {
            self.step();
        }
    }

    pub fn seated(&self, player: usize) -> bool {
        self.players[player].client.phase() == ClientPhase::Flying
    }

    pub fn closed(&self, player: usize) -> bool {
        self.players[player].client.phase() == ClientPhase::Closed
    }

    /// The host's position of `plane` at fractional tick `tick`.
    pub fn truth_at(&self, plane: u32, tick: f64) -> Option<[f64; 3]> {
        let t0 = tick.floor() as u64;
        let a = self.truth.get(&(t0, plane))?;
        let b = self.truth.get(&(t0 + 1, plane)).unwrap_or(a);
        let s = tick - tick.floor();
        Some(std::array::from_fn(|i| a[i] + (b[i] - a[i]) * s))
    }
}

fn weave_script() -> Script {
    Box::new(|now, _, _| weave(now.as_secs_f64()))
}

/// The bot's pilot with no picture: straight and level and turns, never
/// firing.
pub(super) fn level_script() -> Script {
    let mut pilot = crate::bot::ScriptedPilot::new();
    Box::new(move |now, client, _| match client.prediction() {
        Some(prediction) => pilot.controls(now, &prediction.plane().flight, None, &|_| false),
        None => Controls::default(),
    })
}

/// The bot's pilot, chasing and firing at the other side.
pub(super) fn bot_script() -> Script {
    let mut pilot = crate::bot::ScriptedPilot::new();
    Box::new(move |now, client, picture| match client.prediction() {
        Some(prediction) => {
            let enemies = crate::bot::enemies(client);
            pilot.controls(now, &prediction.plane().flight, picture, &|id| {
                enemies.contains(&id)
            })
        }
        None => Controls::default(),
    })
}

#[test]
fn the_weather_reading_follows_the_host_tick() {
    let mut rig = Rig::new(spec(1, 1, 20), LinkConfig::PERFECT, 1);
    let start = rig
        .host
        .world()
        .terrain
        .weather
        .configuration()
        .start_seconds();
    rig.host.start_now();
    let mut checked = 0;
    for _ in 0..3000 {
        let before = rig.host.world().tick();
        rig.step();
        let world = rig.host.world();
        if world.tick() > before {
            let reading = tore_world::world::plane::WeatherReading::of(&world.terrain.weather);
            assert_eq!(prediction::weather_at(start, world.tick() - 1), reading);
            checked += 1;
        }
    }
    assert!(checked > 300);
}

#[test]
fn a_client_joins_flies_and_leaves_with_its_debrief() {
    let mut rig = Rig::new(
        spec(2, 2, 20),
        LinkConfig::for_round_trip(40 * MS, 0.1, 0., 0.),
        3,
    );
    let player = rig.join(|_| {}, weave_script());
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(player)));
    rig.run(Duration::from_secs(3));
    let now = rig.net.now();
    rig.players[player].client.leave_game(now);
    assert!(rig.run_until(Duration::from_secs(7), |r| r.closed(player)));
    let p = &rig.players[player];
    assert!(
        p.events
            .iter()
            .any(|e| matches!(e, ClientEvent::Debrief(_))),
        "{:?}",
        p.events
    );
    assert!(p.events.iter().any(|e| matches!(
        e,
        ClientEvent::Closed(CloseReason::Disconnected {
            reason: DisconnectReason::Left,
            by_peer: false
        })
    )));
    assert!(!p.digests.is_empty());
}

/// A join through the master (slice J2): the race of the host's addresses
/// chooses the one that answers, the client is seated along its path, and
/// its capture, which names the race, replays the same.
#[test]
fn a_raced_join_is_seated_along_the_answering_path_and_its_capture_replays() {
    let mut rig = Rig::new(
        spec(1, 1, 20),
        LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
        5,
    );
    let race = Race {
        targets: vec![
            Target::new("10.0.0.99:26900".parse().unwrap(), Path::LocalNetwork),
            Target::new(host_address(), Path::Punched),
        ],
        introduction: 0x51,
    };
    let player = rig.join(|c| c.race = Some(race), level_script());
    let capture = Shared::default();
    rig.players[player]
        .client
        .set_capture(Box::new(capture.clone()));
    assert!(!rig.players[player].client.chosen());
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(player)));
    let client = &rig.players[player].client;
    assert!(client.chosen());
    assert_eq!(
        (client.server(), client.path()),
        (host_address(), Path::Punched)
    );
    rig.run(Duration::from_secs(2));
    let bytes = capture.0.lock().unwrap().clone();
    let replayed = capture::replay(&bytes, Arc::clone(&rig.resources), &mut |_| {}).unwrap();
    assert!(replayed.identical, "the replay sent the same");
    assert!(replayed.frames > 0);
}

#[test]
fn a_different_import_is_refused_with_the_names_that_differ() {
    let mut rig = Rig::new(spec(1, 1, 20), LinkConfig::PERFECT, 4);
    let mut other = resources();
    let name = other.keys().find(|k| k.ends_with(".PT")).unwrap().clone();
    other.get_mut(&name).unwrap().push(0);
    rig.resources = Arc::new(other);
    let player = rig.join(|_| {}, weave_script());
    // The player is told why and stays in the lobby, marked unable.
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[player]
            .client
            .lobby()
            .and_then(|l| l.me())
            .is_some_and(|me| me.unable.is_some())
    }));
    rig.run(Duration::from_secs(1));
    let p = &rig.players[player];
    assert!(
        p.events.iter().any(|e| matches!(
            e,
            // Stage L: worded by the item the file belongs to (the changed
            // profile no longer loads, so the import lacks the aircraft).
            ClientEvent::ContentRefused { names, reason }
                if names.contains(&name) && reason.contains("F/A-18D Hornet")
        )),
        "{:?}",
        p.events
    );
    assert_eq!(p.client.phase(), ClientPhase::Lobby);
    assert!(p.client.unable().is_some());
    assert_eq!(rig.host.phase(), crate::host::Phase::Lobby);
}

#[test]
fn initial_stick_and_commands_do_not_rewrite_ticks_before_the_host_can_receive_them() {
    for rtt_ms in [60, 120] {
        let mut rig = Rig::new(
            spec(1, 1, 20),
            LinkConfig::for_round_trip(rtt_ms * MS, 0., 0., 0.),
            5,
        );
        let mut commanded = false;
        let player = rig.join(
            |_| {},
            Box::new(move |_, client, _| {
                let mut controls = Controls {
                    pilot: PilotInput {
                        roll: 0.4,
                        yaw: 0.3,
                        ..Default::default()
                    },
                    ..Controls::default()
                };
                if client.phase() == ClientPhase::Flying && !commanded {
                    controls.pilot.commands.push(tore_input::PilotCommand::Set(
                        tore_input::Switch::Airbrake,
                        true,
                    ));
                    commanded = true;
                }
                controls
            }),
        );
        assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(player)));
        rig.run(Duration::from_secs(3));
        let client = &rig.players[player].client;
        let stats = client.clone_stats();
        assert!(stats.hashes_compared > 60);
        assert_eq!(
            stats.mismatches,
            0,
            "rtt={rtt_ms}: {:?}",
            client.corrections()
        );
        assert_eq!(stats.corrections, 0);
        let flight = &client.prediction().unwrap().plane().flight;
        assert!(
            flight.aileron > 0.3 && flight.rudder > 0.2,
            "live inputs were lost"
        );
        assert!(
            flight.brake_out,
            "the first command was lost during bootstrap"
        );
        assert!(
            client.unacked.is_empty(),
            "the command was not acknowledged"
        );
    }
}

/// Flies one client for `seconds` on a clean link and checks that, after
/// seating settles, the prediction equals the host at every snapshot.
fn prediction_matches_the_host(seconds: u64) {
    let mut rig = Rig::new(
        spec(2, 2, 20),
        LinkConfig::for_round_trip(60 * MS, 0., 0., 0.),
        5,
    );
    let player = rig.join(|_| {}, level_script());
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(player)));
    rig.run(Duration::from_secs(3));
    let settled = rig.players[player].client.stats();
    rig.run(Duration::from_secs(seconds));
    let stats = rig.players[player].client.stats();
    eprintln!("settled after seating: {settled:#?}\nend: {stats:#?}");
    eprintln!(
        "corrections: {:?}",
        rig.players[player].client.corrections()
    );
    let compared = stats.hashes_compared - settled.hashes_compared;
    assert!(
        compared as f64 >= seconds as f64 * 30. * 0.95,
        "{compared} hashes compared in {seconds} s"
    );
    assert_eq!(stats.mismatches, settled.mismatches, "{stats:#?}");
    assert_eq!(stats.corrections, settled.corrections, "{stats:#?}");
    assert_eq!(stats.inputs_repeated, settled.inputs_repeated, "{stats:#?}");
    assert_eq!(stats.clock_jumps, settled.clock_jumps, "{stats:#?}");
    let margin = stats.input_margin.unwrap();
    assert!((2..=6).contains(&margin), "input margin {margin}");
}

#[test]
fn the_prediction_equals_the_host_at_every_snapshot() {
    prediction_matches_the_host(20);
}

/// The acceptance's five minutes; run with `--ignored`.
#[test]
#[ignore = "five minutes of simulated flight"]
fn the_prediction_equals_the_host_for_five_minutes() {
    prediction_matches_the_host(300);
}

/// A writer the test keeps a handle on.
#[derive(Clone, Default)]
pub(super) struct Shared(pub(super) Arc<std::sync::Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// What a fight measured for one bot.
#[derive(Debug, Default)]
struct Measured {
    frames: u64,
    /// Other aircraft drawn, and of them within 1, 3 and 10 ft of the host.
    drawn: u64,
    within: [u64; 3],
    worst_ft: f64,
    /// Aircraft frames drawn with an extra delay (sent twice a second).
    far: u64,
}

/// Two bots fight AI enemies for `seconds` against a host at `round_trip`
/// with `loss` each way (1 percent duplicated, arrivals spread by 10 percent
/// of the one-way delay), then leave; the first bot's capture replays
/// offline into the same frames.
fn fight(seconds: u64, round_trip: Duration, loss: f64) {
    let link = LinkConfig::for_round_trip(round_trip, 0.1, loss, 0.01);
    let mut rig = Rig::new(spec(2, 2, 2), link, 21);
    rig.watch = true;
    let capture = Shared::default();
    let a = rig.join(|c| c.callsign = "Alpha".into(), bot_script());
    rig.players[a].client.set_capture(Box::new(capture.clone()));
    let diagnostics = Shared::default();
    rig.players[a]
        .client
        .set_diagnostics(Box::new(diagnostics.clone()));
    let b = rig.join(|c| c.callsign = "Bravo".into(), bot_script());
    assert!(rig.run_until(Duration::from_secs(5), |r| r.seated(a) && r.seated(b)));
    let mut measured = [Measured::default(), Measured::default()];
    let end = rig.net.now() + Duration::from_secs(seconds);
    let mut counted = [0usize; 2];
    while rig.net.now() < end {
        rig.step();
        for (i, m) in measured.iter_mut().enumerate() {
            let player = &rig.players[i];
            if player.digests.len() == counted[i] {
                continue;
            }
            counted[i] = player.digests.len();
            let Some(picture) = &player.picture else {
                continue;
            };
            // No picture of the others before the first snapshot arrives.
            let Some(render) = player.client.render_tick() else {
                continue;
            };
            m.frames += 1;
            for pose in picture.targets.iter().filter(|p| p.aircraft.is_some()) {
                // Each is drawn at the render time less its own extra delay:
                // compared with the host at that time.
                let key = EntityKey {
                    kind: crate::wire::entity::EntityKind::Aircraft,
                    id: pose.id,
                };
                let extra = player.client.interpolator().extra(key).unwrap_or(0.);
                m.far += u64::from(extra > 0.);
                let Some(truth) = rig.truth_at(pose.id, render - extra) else {
                    continue;
                };
                let error = (0..3)
                    .map(|k| (pose.position[k] - truth[k]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                m.drawn += 1;
                for (n, limit) in [1., 3., 10.].iter().enumerate() {
                    m.within[n] += u64::from(error <= *limit);
                }
                m.worst_ft = m.worst_ft.max(error);
            }
        }
    }
    let ticks = seconds * 120;
    let flying: Vec<ClientStats> = rig.players.iter_mut().map(|p| p.client.stats()).collect();
    for i in [a, b] {
        // The newest readout is the seat's own plane's, recent.
        let now = rig.net.now();
        let frame = rig.players[i].client.frame(now).unwrap();
        let readout = frame.readout.as_ref().expect("a readout");
        assert_eq!(readout.plane, frame.plane.0);
        assert!(
            readout.tick as f64 > frame.render_tick - 30.,
            "{}",
            readout.tick
        );
        assert_eq!(readout.stores.ammo.len(), frame.config.stations.len());
        rig.players[i].digests.push(frame.digest());
    }
    for i in [a, b] {
        let now = rig.net.now();
        rig.players[i].client.leave_game(now);
    }
    assert!(rig.run_until(Duration::from_secs(8), |r| r.closed(a) && r.closed(b)));
    for (i, m) in measured.iter().enumerate() {
        let p = &rig.players[i];
        let stats = p.client.clone_stats();
        let corrections = p.client.corrections();
        let shown = corrections.iter().filter(|c| c.shown).count();
        let under_foot = corrections.iter().filter(|c| c.feet < 1.).count();
        let pct = |n: u64, of: u64| 100. * n as f64 / of.max(1) as f64;
        eprintln!("bot {i}: {:#?}", flying[i]);
        eprintln!(
            "  corrections {} (shown {shown}, under 1 ft {under_foot}); {:.2} percent of {} \
             snapshots needed a visible correction",
            corrections.len(),
            pct(shown as u64, stats.snapshots),
            stats.snapshots
        );
        eprintln!(
            "  other aircraft drawn {} ({} far): within 1 ft {:.2} percent, 3 ft {:.2}, \
             10 ft {:.2}; worst {:.2} ft",
            m.drawn,
            m.far,
            pct(m.within[0], m.drawn),
            pct(m.within[1], m.drawn),
            pct(m.within[2], m.drawn),
            m.worst_ft
        );
        eprintln!(
            "  extrapolated {:.3} percent of entity frames; far ones {} of {}",
            pct(stats.extrapolated, stats.entity_frames),
            stats.far_extrapolated,
            stats.far_frames
        );
        eprintln!(
            "  inputs repeated {:.3} percent of {ticks} ticks",
            pct(stats.inputs_repeated, ticks)
        );
        assert!(
            p.events
                .iter()
                .any(|e| matches!(e, ClientEvent::Debrief(_))),
            "bot {i} got its debrief"
        );
        assert!(p.events.iter().any(|e| matches!(
            e,
            ClientEvent::Closed(CloseReason::Disconnected {
                reason: DisconnectReason::Left,
                by_peer: false
            })
        )));
        assert!(m.drawn > 0);
    }
    let log = String::from_utf8(diagnostics.0.lock().unwrap().clone()).unwrap();
    assert!(log.lines().filter(|l| l.contains("\tstats\t")).count() as u64 >= seconds);
    assert!(log.contains("\tseated\t"));

    // The capture replays offline into the same frames.
    let bytes = capture.0.lock().unwrap().clone();
    let mut digests = Vec::new();
    let replayed = capture::replay(&bytes, Arc::clone(&rig.resources), &mut |frame| {
        digests.push(frame.digest());
    })
    .unwrap();
    assert!(
        replayed.identical,
        "the replayed client sent the same inputs"
    );
    assert_eq!(digests, rig.players[a].digests, "the same frames");
    eprintln!(
        "capture: {} bytes, {} records, {} frames",
        bytes.len(),
        replayed.records,
        replayed.frames
    );
}

#[test]
fn two_bots_fight_and_a_capture_replays_into_the_same_frames() {
    fight(20, Duration::from_millis(150), 0.02);
}

/// The acceptance's five minutes at 150 ms and 2 percent loss; run with
/// `--ignored`.
#[test]
#[ignore = "five minutes of simulated flight"]
fn two_bots_fight_for_five_minutes() {
    fight(300, Duration::from_millis(150), 0.02);
}

/// A bot joins a host over real UDP sockets on 127.0.0.1, with the real
/// clock and system entropy, flies for `seconds` after it is seated and
/// leaves with its debrief. The snapshot rate is judged over the seated
/// time only: joining and building the mission take longer on a slow
/// machine (macOS CI runners took more than half a second), and that time is
/// not the flight's (slice EF-X).
fn real_udp(seconds: u64) {
    let resources = Arc::new(resources());
    let mut host = Host::new(
        spec(2, 2, 20),
        Arc::clone(&resources),
        HostConfig::new(build()),
    )
    .unwrap();
    let mut host_socket = tore_net::bind_udp("127.0.0.1:0".parse().unwrap()).unwrap();
    let address = host_socket.local_addr().unwrap();
    let mut socket = tore_net::bind_udp("127.0.0.1:0".parse().unwrap()).unwrap();
    let clock = tore_net::RealClock::new();
    let client = Client::connect(
        ClientConfig::new(address, "Viper", build()),
        resources,
        clock.now(),
    )
    .unwrap();
    let mut bot = crate::bot::Bot::new(client);
    let mut events = Vec::new();
    let mut left = false;
    // When the bot was seated and when it left, with the snapshot counts then.
    let mut seated_at: Option<(Duration, u64)> = None;
    let mut left_at: Option<(Duration, u64)> = None;
    let deadline = Duration::from_secs(seconds + 10);
    while clock.now() < deadline {
        let now = clock.now();
        host.receive_from(now, &mut host_socket).unwrap();
        host.update(now);
        host.transmit(&mut host_socket).unwrap();
        bot.client.receive_from(now, &mut socket).unwrap();
        if !left && bot.client.phase() == ClientPhase::Flying {
            let flown = seated_at.is_some_and(|(at, _)| now >= at + Duration::from_secs(seconds));
            if flown {
                bot.client.leave_game(now);
                left = true;
                left_at = Some((now, bot.client.clone_stats().snapshots));
            }
        }
        bot.update(now);
        bot.client.transmit(&mut socket).unwrap();
        events.extend(std::iter::from_fn(|| bot.client.poll_event()));
        if seated_at.is_none()
            && events
                .iter()
                .any(|e| matches!(e, ClientEvent::Seated { .. }))
        {
            seated_at = Some((now, bot.client.clone_stats().snapshots));
        }
        if bot.client.phase() == ClientPhase::Closed {
            break;
        }
        std::thread::sleep(
            host.next_wake(now)
                .min(bot.client.next_wake(now))
                .max(Duration::from_micros(100)),
        );
    }
    let stats = bot.client.clone_stats();
    eprintln!("real UDP: {stats:#?}");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ClientEvent::Seated { .. }))
    );
    assert!(
        events.iter().any(|e| matches!(e, ClientEvent::Debrief(_))),
        "{events:?}"
    );
    assert_eq!(bot.client.phase(), ClientPhase::Closed);
    // Thirty snapshots a second while seated, give or take a slow machine.
    let ((from, first), (to, last)) = (seated_at.unwrap(), left_at.expect("the bot left"));
    let flown = (to - from).as_secs_f64();
    eprintln!(
        "seated at {from:?}, {} snapshots in {flown:.2} s seated",
        last - first
    );
    assert!(flown >= seconds as f64);
    assert!(
        (last - first) as f64 > flown * 30. * 0.8,
        "{} snapshots in {flown:.2} s",
        last - first
    );
    assert!(bot.frames > 0);
}

#[test]
fn a_bot_flies_over_real_udp_on_this_machine() {
    real_udp(3);
}

/// The acceptance's minute; run with `--ignored`.
#[test]
#[ignore = "a minute of real time"]
fn a_bot_flies_a_minute_over_real_udp() {
    real_udp(60);
}

/// The host holds a seat's exact states until its Seated message is
/// acknowledged, so none arrives before the Seated message does, even on a
/// slow lossy link (the client keeps early ones, but need not).
#[test]
fn no_exact_state_arrives_before_the_seated_message() {
    for (seed, round_trip, loss) in [(31, 300, 0.05), (32, 150, 0.05), (33, 300, 0.02)] {
        let link = LinkConfig::for_round_trip(round_trip * MS, 0.1, loss, 0.01);
        let mut rig = Rig::new(spec(2, 2, 2), link, seed);
        let a = rig.join(|c| c.callsign = "Alpha".into(), bot_script());
        let b = rig.join(|c| c.callsign = "Bravo".into(), bot_script());
        let mut early = 0;
        let end = rig.net.now() + Duration::from_secs(12);
        while rig.net.now() < end {
            rig.step();
            for i in [a, b] {
                early += rig.players[i].client.early.len();
            }
        }
        assert!(rig.seated(a) && rig.seated(b), "seated (seed {seed})");
        assert_eq!(early, 0, "seed {seed}: own states arrived before Seated");
    }
}
