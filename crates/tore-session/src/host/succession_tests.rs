//! Slice K6's acceptance (docs/ARCHITECTURE.md, "Host selection"): a game a
//! player hosts and its players' full client sessions on the network
//! simulator, each game with its peers router in front of its joined
//! socket, as a game has. Ranking by each measure in turn; a relayed
//! player never chosen and never pinnable; reach tests along the rows of
//! the punching table, where the eligible rows are the punching rows; an
//! upload test on a link throttled to half the need fails and at the full
//! need passes; the warnings' words; the pin and its fallback; the bytes of
//! a 30-player reach test bounded. Synthetic resources.

use super::state::CandidatesPart;
use super::*;
use crate::client::candidate::CandidateSettings;
use crate::client::{Client, ClientConfig, ClientEvent, Controls, Race};
use crate::host::{AfterEnd, BuildId, CrownRule, HostConfig, StartMode};
use crate::wire::PROTOCOL_VERSION;
use crate::wire::messages::{SettingsChange, kind};
use crate::wire::migration::{ReachResult, Reached};
use std::net::SocketAddr;
use std::sync::Arc;
use tore_formats::aircraft::AircraftId;
use tore_net::peers::{Peers, Route};
use tore_net::sim::nat::{Filtering, Forward, Mapping, Prefix, RouterConfig};
use tore_net::sim::{LinkConfig, SimNetwork, SimSocket};
use tore_net::{Datagrams, Entropy, Target};
use tore_world::mission::{MissionSpec, Skill, Start};
use tore_world::test_support::resources::{THEATER, resources};

const MS: Duration = Duration::from_millis(1);
const HOST: &str = "198.51.100.1:26900";
const HOUSE: &str = "198.51.100.5:40000";

fn a(text: &str) -> SocketAddr {
    text.parse().unwrap()
}

fn build() -> BuildId {
    BuildId {
        version: "0.1.3-1-gtest".into(),
        commit: "test-commit".into(),
        release: false,
    }
}

fn spec() -> MissionSpec {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 4;
    spec.wings[3].count = 2;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 20;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

/// A token bucket on a game's sends: a link that carries `rate` bytes a
/// second (the simulator's links have no rate of their own).
struct Throttle {
    rate: f64,
    tokens: f64,
    at: Duration,
}

impl Throttle {
    fn new(rate: u32) -> Self {
        Self {
            rate: f64::from(rate),
            tokens: 0.,
            at: Duration::ZERO,
        }
    }

    fn allow(&mut self, now: Duration, bytes: usize) -> bool {
        let burst = 2_400.;
        self.tokens = (self.tokens + (now - self.at).as_secs_f64() * self.rate).min(burst);
        self.at = now;
        if self.tokens >= bytes as f64 {
            self.tokens -= bytes as f64;
            true
        } else {
            false
        }
    }
}

/// One player's game: its joined socket, its peers router and its client.
struct Game {
    socket: SimSocket,
    peers: Peers,
    client: Client,
    events: Vec<ClientEvent>,
    throttle: Option<Throttle>,
    gone: bool,
}

/// A host and its players' games on one simulated network.
struct Rig {
    net: SimNetwork,
    host: Host,
    socket: SimSocket,
    games: Vec<Game>,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
}

impl Rig {
    /// A game a player hosts, the house at [`HOUSE`]; `net` already holds
    /// its routers.
    fn on(net: SimNetwork) -> Self {
        Self::with(net, |config| {
            config.house = Some(a(HOUSE));
            config.crown = CrownRule::FirstPlayer;
            config.start = StartMode::King;
        })
    }

    fn with(net: SimNetwork, configure: impl FnOnce(&mut HostConfig)) -> Self {
        let socket = net.bind(a(HOST)).unwrap();
        let resources = Arc::new(resources());
        let mut config = HostConfig {
            entropy: Entropy::Seeded(11),
            after_end: AfterEnd::Restart,
            ..HostConfig::new(build())
        };
        configure(&mut config);
        let host = Host::new(spec(), Arc::clone(&resources), config).unwrap();
        Self {
            net,
            host,
            socket,
            games: Vec::new(),
            resources,
        }
    }

    fn new() -> Self {
        let net = SimNetwork::new(7);
        net.set_default_link(LinkConfig::one_way(Duration::from_millis(20)));
        Self::on(net)
    }

    /// A game joining from `socket` as `callsign`; through the relay when
    /// `relayed`.
    fn join(
        &mut self,
        socket: SimSocket,
        callsign: &str,
        relayed: bool,
        settings: CandidateSettings,
    ) -> usize {
        let seed = 100 + self.games.len() as u64;
        let mut config = ClientConfig {
            entropy: Entropy::Seeded(seed),
            auto_ready: false,
            ..ClientConfig::new(a(HOST), callsign, build())
        };
        if relayed {
            config.race = Some(Race {
                targets: vec![Target::new(a(HOST), Path::Relay)],
                introduction: 1,
            });
        }
        let mut client =
            Client::connect(config, Arc::clone(&self.resources), self.net.now()).unwrap();
        client.set_candidate(settings);
        self.games.push(Game {
            socket,
            peers: Peers::new(PROTOCOL_VERSION, Entropy::Seeded(seed)),
            client,
            events: Vec::new(),
            throttle: None,
            gone: false,
        });
        self.games.len() - 1
    }

    /// A game on the open network at `address`, its Local candidate its
    /// own address.
    fn join_at(&mut self, address: &str, callsign: &str) -> usize {
        let socket = self.net.bind(a(address)).unwrap();
        let settings = CandidateSettings {
            candidates: vec![Candidate::new(CandidateKind::Local, a(address))],
            ..CandidateSettings::default()
        };
        self.join(socket, callsign, false, settings)
    }

    /// One millisecond for everyone.
    fn step(&mut self) {
        self.net.advance(MS);
        let now = self.net.now();
        self.host.receive_from(now, &mut self.socket).unwrap();
        self.host.update(now);
        self.host.transmit(&mut self.socket).unwrap();
        while self.host.poll_log().is_some() {}
        let mut buf = vec![0u8; 2_048];
        for game in self.games.iter_mut().filter(|g| !g.gone) {
            while let Ok(Some((len, from))) = game.socket.recv_datagram(&mut buf) {
                if game.peers.route(now, from, &buf[..len]) == Route::Client {
                    game.client.receive(now, from, &buf[..len]);
                }
            }
            game.client.update(now, &Controls::default());
            game.client.drive_peers(now, &mut game.peers);
            game.peers.update(now);
            game.peers.transmit(&mut game.socket).unwrap();
            while let Some(t) = game.client.poll_transmit() {
                if game
                    .throttle
                    .as_mut()
                    .is_none_or(|throttle| throttle.allow(now, t.datagram.len()))
                {
                    game.socket.send_datagram(t.to, &t.datagram).unwrap();
                }
            }
            while let Some(event) = game.client.poll_event() {
                game.events.push(event);
            }
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

    /// The `n`th game's lobby id and join order.
    fn id(&self, n: usize) -> u8 {
        self.games[n].client.lobby().expect("in the lobby").you
    }

    fn order(&self, n: usize) -> u64 {
        let id = self.id(n);
        self.host
            .peers
            .values()
            .find(|p| p.lobby.id == id)
            .expect("connected")
            .lobby
            .order
    }

    /// Every game in the lobby with every report in and a reach test done
    /// since the last one arrived.
    fn settle(&mut self) {
        let games = self.games.len();
        assert!(
            self.run_until(Duration::from_secs(30), |r| {
                r.host
                    .succession
                    .measures
                    .values()
                    .filter(|m| m.report.is_some())
                    .count()
                    == games
                    && r.host.succession.reach.is_none()
                    && !r.host.succession.reach_due
            }),
            "every report and a reach test"
        );
    }

    fn notices(&self, n: usize) -> Vec<String> {
        self.games[n]
            .events
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Notice(text) => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn refusals(&self, n: usize) -> Vec<(u8, String)> {
        self.games[n]
            .events
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Refused { request, reason } => Some((*request, reason.clone())),
                _ => None,
            })
            .collect()
    }

    fn pin(&mut self, value: u32) {
        self.games[0].client.change_settings(SettingsChange {
            values: vec![(number::HOST, value)],
            ..SettingsChange::default()
        });
    }

    /// The candidates the host ranks, as game indices.
    fn ranked(&self) -> Vec<usize> {
        self.host
            .ranked_candidates()
            .into_iter()
            .map(|order| {
                (0..self.games.len())
                    .find(|&n| self.order(n) == order)
                    .unwrap()
            })
            .collect()
    }
}

/// The house and three direct players, settled.
fn four() -> Rig {
    let mut rig = Rig::new();
    rig.join_at(HOUSE, "Viper");
    rig.join_at("198.51.100.11:40000", "Alpha");
    rig.join_at("198.51.100.12:40000", "Bravo");
    rig.join_at("198.51.100.13:40000", "Charlie");
    rig.settle();
    rig
}

#[test]
fn every_player_reports_and_reaches_every_candidate() {
    let rig = four();
    let house = rig.order(0);
    for n in 1..4 {
        let measure = &rig.host.succession.measures[&rig.order(n)];
        assert_eq!(
            measure.report.as_ref(),
            Some(&rig.games[n].client.candidate_report())
        );
        // Every other direct player reached it, at the links' 40 ms.
        let trips: Vec<u16> = measure.reached_by.values().map(|r| r.unwrap()).collect();
        assert_eq!(trips.len(), 2, "{measure:?}");
        assert!(trips.iter().all(|&ms| (40..=45).contains(&ms)), "{trips:?}");
        assert!(!measure.reached_by.contains_key(&house));
    }
    assert_eq!(rig.ranked(), vec![1, 2, 3]);
    assert_eq!(
        rig.host.standby_choice().len(),
        2,
        "the two best are the standbys"
    );
}

fn m(rig: &mut Rig, order: u64) -> &mut Measure {
    rig.host.succession.measures.get_mut(&order).unwrap()
}

#[test]
fn candidates_rank_by_upload_then_round_trip_then_router_then_cpu_then_join_order() {
    let mut rig = four();
    let (a1, b2, c3) = (rig.order(1), rig.order(2), rig.order(3));
    let measures = &mut rig.host.succession.measures;
    let pass = Some(Upload {
        players: 4,
        per_mille: 1_000,
    });
    for order in [a1, b2, c3] {
        let m = measures.get_mut(&order).unwrap();
        m.upload = pass;
        for trip in m.reached_by.values_mut() {
            *trip = Some(50);
        }
        let report = m.report.as_mut().unwrap();
        report.mapping = MappingType::Unknown;
        report.cpu_micros = 2_000;
    }
    // Everything equal: the longest connected first.
    assert_eq!(rig.ranked(), vec![1, 2, 3]);
    // The CPU in 0.5 ms steps: 1.6 ms beats 2.0, 1.9 does not.
    m(&mut rig, b2).report.as_mut().unwrap().cpu_micros = 2_400;
    assert_eq!(rig.ranked(), vec![1, 2, 3], "the same step");
    m(&mut rig, c3).report.as_mut().unwrap().cpu_micros = 1_400;
    assert_eq!(rig.ranked(), vec![3, 1, 2]);
    // The router before the CPU.
    m(&mut rig, b2).report.as_mut().unwrap().mapping = MappingType::SamePort;
    assert_eq!(rig.ranked(), vec![2, 3, 1]);
    // A mapped port counts as no translation.
    let mapped = Candidate::new(CandidateKind::Mapped, a("203.0.113.1:26900"));
    m(&mut rig, a1)
        .report
        .as_mut()
        .unwrap()
        .candidates
        .push(mapped);
    assert_eq!(rig.ranked(), vec![1, 2, 3]);
    // The median round trip, in 20 ms steps, before the router.
    for trip in m(&mut rig, c3).reached_by.values_mut() {
        *trip = Some(21);
    }
    for trip in m(&mut rig, b2).reached_by.values_mut() {
        *trip = Some(39);
    }
    assert_eq!(rig.ranked(), vec![2, 3, 1], "21 and 39 ms are one step");
    for trip in m(&mut rig, c3).reached_by.values_mut() {
        *trip = Some(19);
    }
    assert_eq!(rig.ranked(), vec![3, 2, 1]);
    // The upload before everything: a pass, then a fail, then untested.
    m(&mut rig, c3).upload = None;
    m(&mut rig, b2).upload = Some(Upload {
        players: 4,
        per_mille: 899,
    });
    assert_eq!(rig.ranked(), vec![1, 2, 3]);
}

#[test]
fn a_relayed_player_is_never_chosen_and_cannot_be_pinned() {
    let mut rig = Rig::new();
    rig.join_at(HOUSE, "Viper");
    let socket = rig.net.bind(a("198.51.100.20:40000")).unwrap();
    let hawk = rig.join(socket, "Hawk", true, CandidateSettings::default());
    let delta = rig.join_at("198.51.100.21:40000", "Delta");
    rig.settle();
    let hawk_order = rig.order(hawk);
    // The best measures do not make it a candidate.
    let measure = rig.host.succession.measures.get_mut(&hawk_order).unwrap();
    measure.upload = Some(Upload {
        players: 3,
        per_mille: 1_000,
    });
    measure.report.as_mut().unwrap().mapping = MappingType::NoTranslation;
    assert_eq!(rig.ranked(), vec![delta]);
    assert!(
        !rig.host.succession.measures[&rig.order(delta)]
            .reached_by
            .contains_key(&hawk_order),
        "a relayed player's reach does not count"
    );
    let id = rig.id(hawk);
    rig.pin(1 + u32::from(id));
    rig.run(Duration::from_millis(300));
    assert!(
        rig.refusals(0).contains(&(
            kind::SETTINGS,
            "Hawk connects through the relay and cannot host.".into()
        )),
        "{:?}",
        rig.refusals(0)
    );
    assert_eq!(rig.host.settings.pinned_host(), None);
}

#[test]
fn the_pin_moves_the_host_and_falls_back_to_calculated_when_its_player_leaves() {
    let mut rig = Rig::new();
    rig.join_at(HOUSE, "Viper");
    let delta = rig.join_at("198.51.100.21:40000", "Delta");
    rig.settle();
    assert_eq!(rig.host.host_move(), None, "the house passes: it stays");
    let id = rig.id(delta);
    rig.pin(1 + u32::from(id));
    rig.run(Duration::from_millis(300));
    assert!(rig.refusals(0).is_empty(), "{:?}", rig.refusals(0));
    assert_eq!(rig.host.host_move(), Some(id), "a pin moves the host");
    let lobby = rig.games[0].client.lobby().unwrap();
    assert!(lobby.settings.contains(&(number::HOST, 1 + u32::from(id))));
    // Pinning the house stays.
    rig.pin(1 + u32::from(rig.id(0)));
    rig.run(Duration::from_millis(300));
    assert_eq!(rig.host.host_move(), None);
    rig.pin(1 + u32::from(id));
    rig.run(Duration::from_millis(300));
    let now = rig.net.now();
    rig.games[delta].client.disconnect(now);
    rig.run(Duration::from_millis(300));
    rig.games[delta].gone = true;
    rig.run(Duration::from_millis(500));
    assert_eq!(rig.host.settings.pinned_host(), None);
    assert_eq!(rig.host.host_move(), None);
    assert!(
        rig.notices(0)
            .contains(&"Delta, the pinned host, left: the host is calculated again.".into()),
        "{:?}",
        rig.notices(0)
    );
    let lobby = rig.games[0].client.lobby().unwrap();
    assert!(lobby.settings.contains(&(number::HOST, 0)));
}

#[test]
fn a_pin_of_nobody_or_of_a_player_who_switched_hosting_off_is_refused() {
    let mut rig = Rig::new();
    rig.join_at(HOUSE, "Viper");
    let socket = rig.net.bind(a("198.51.100.21:40000")).unwrap();
    let off = CandidateSettings {
        may_host: false,
        ..CandidateSettings::default()
    };
    let delta = rig.join(socket, "Delta", false, off);
    rig.settle();
    rig.pin(1 + u32::from(rig.id(delta)));
    rig.pin(1 + 200);
    rig.run(Duration::from_millis(300));
    let refused: Vec<String> = rig.refusals(0).into_iter().map(|(_, r)| r).collect();
    assert_eq!(
        refused,
        vec![
            "Delta has turned off \"Let my game take over hosting\".".to_owned(),
            "No player has the lobby id 200.".to_owned(),
        ]
    );
    assert!(rig.host.ranked_candidates().is_empty());
}

#[test]
fn a_dedicated_server_refuses_a_pin_and_selects_nothing() {
    let net = SimNetwork::new(7);
    net.set_default_link(LinkConfig::one_way(Duration::from_millis(20)));
    let mut rig = Rig::with(net, |config| {
        config.crown = CrownRule::FirstPlayer;
        config.start = StartMode::FirstPlayer;
    });
    rig.join_at("198.51.100.20:40000", "Viper");
    rig.join_at("198.51.100.21:40000", "Delta");
    rig.run(Duration::from_secs(3));
    rig.pin(1 + u32::from(rig.id(1)));
    rig.run(Duration::from_millis(300));
    assert_eq!(
        rig.refusals(0),
        vec![(
            kind::SETTINGS,
            "A dedicated server always hosts its own game.".into()
        )]
    );
    // It keeps the reports and tests nothing.
    assert_eq!(rig.host.succession.measures.len(), 2);
    assert!(rig.host.succession.last_reach.is_none());
    assert!(rig.host.ranked_candidates().is_empty());
}

#[test]
fn the_king_reads_that_no_other_game_can_take_over() {
    let mut rig = Rig::new();
    rig.join_at(HOUSE, "Viper");
    let socket = rig.net.bind(a("198.51.100.20:40000")).unwrap();
    rig.join(socket, "Hawk", true, CandidateSettings::default());
    rig.run(Duration::from_secs(3));
    assert_eq!(
        rig.notices(0),
        vec!["No other game can take over hosting: if you leave, the game ends.".to_owned()],
        "said once"
    );
    // A direct player who may host joins: the warning ends, and comes back
    // only when it holds again.
    rig.join_at("198.51.100.21:40000", "Delta");
    rig.settle();
    assert!(!rig.host.succession.warned.contains(words::ALONE));
}

#[test]
fn the_calculated_host_stays_while_it_passes_and_moves_when_it_fails_and_another_passes() {
    let mut rig = Rig::new();
    rig.join_at(HOUSE, "Viper");
    let delta = rig.join_at("198.51.100.21:40000", "Delta");
    rig.settle();
    // Delta's upload test runs and passes on the open link.
    assert!(rig.run_until(Duration::from_secs(10), |r| {
        r.host.succession.measures[&r.order(delta)].upload.is_some()
    }));
    let upload = rig.host.succession.measures[&rig.order(delta)]
        .upload
        .unwrap();
    assert!(upload.passed() && upload.players == 2, "{upload:?}");
    assert_eq!(rig.host.host_move(), None, "the house passes");
    // Its flights say the house carried 70 percent: the game moves.
    rig.host.succession.house_upload = Some(Upload {
        players: 2,
        per_mille: 700,
    });
    rig.run(Duration::from_millis(50));
    assert_eq!(rig.host.host_move(), Some(rig.id(delta)));
    assert!(rig.notices(0).is_empty(), "no warning: another passes");
    rig.host.succession.house_upload = Some(Upload {
        players: 2,
        per_mille: 950,
    });
    rig.run(Duration::from_millis(50));
    assert_eq!(rig.host.host_move(), None);
}

#[test]
fn an_upload_test_on_a_link_at_half_the_need_fails_and_at_the_full_need_passes() {
    for (share, passes) in [(0.5, false), (1.05, true)] {
        let mut rig = Rig::new();
        rig.join_at(HOUSE, "Viper");
        let delta = rig.join_at("198.51.100.21:40000", "Delta");
        rig.settle();
        // 32 KB/s for two players: one other player and one warm standby.
        // The full need is the filler's rate plus its packets' headers.
        let need = upload_need(2, 0);
        assert_eq!(need, 28_000 + 3_000 + 2 * 520);
        rig.games[delta].throttle = Some(Throttle::new((f64::from(need) * share) as u32));
        assert!(rig.run_until(Duration::from_secs(10), |r| {
            r.host.succession.measures[&r.order(delta)].upload.is_some()
        }));
        let upload = rig.host.succession.measures[&rig.order(delta)]
            .upload
            .unwrap();
        println!(
            "a link at {share} of the need carried {} per mille",
            upload.per_mille
        );
        assert_eq!(upload.passed(), passes, "{share}: {upload:?}");
        if !passes {
            assert!((400..=560).contains(&upload.per_mille), "{upload:?}");
            assert!(!rig.host.host_passes_for(rig.order(delta), 2));
            // The house fails too: the King reads the best machine's share.
            rig.host.succession.house_upload = Some(Upload {
                players: 2,
                per_mille: 700,
            });
            rig.run(Duration::from_millis(50));
            assert_eq!(rig.host.host_move(), None, "nowhere better to go");
            assert_eq!(
                rig.notices(0),
                vec![
                    "No machine here passed the test for 2 players: Viper's carried 70 percent of what they need. Fewer players, or a dedicated server, will fly better."
                        .to_owned()
                ]
            );
        }
    }
}

#[test]
fn the_cpu_and_pinned_host_warnings_read_as_designed() {
    let mut rig = Rig::new();
    rig.join_at(HOUSE, "Viper");
    let delta = rig.join_at("198.51.100.21:40000", "Delta");
    rig.settle();
    assert!(rig.run_until(Duration::from_secs(10), |r| {
        r.host.succession.measures[&r.order(delta)].upload.is_some()
    }));
    // The house steps the mission at 10 ms a tick: well past half a core,
    // and Delta reported no measure, so it passes and the game moves.
    let house = rig.order(0);
    let slow = |rig: &mut Rig, order: u64, micros: u32| {
        let measures = &mut rig.host.succession.measures;
        measures
            .get_mut(&order)
            .unwrap()
            .report
            .as_mut()
            .unwrap()
            .cpu_micros = micros;
    };
    slow(&mut rig, house, 10_000);
    rig.run(Duration::from_millis(50));
    assert_eq!(rig.host.host_move(), Some(rig.id(delta)));
    // Delta is slow too: no machine passes, and the King reads why.
    let delta_order = rig.order(delta);
    slow(&mut rig, delta_order, 9_000);
    rig.run(Duration::from_millis(50));
    assert_eq!(rig.host.host_move(), None);
    let percent = cpu_percent(10_000);
    assert_eq!(
        rig.notices(0),
        vec![format!(
            "No machine here passed the processor test for this mission: Viper's needs {percent} percent of the time it has. Fewer aircraft, or a dedicated server, will fly better."
        )]
    );
    // Pinned, a failing upload reads as the pinned host's.
    slow(&mut rig, delta_order, 1_000);
    rig.host
        .succession
        .measures
        .get_mut(&delta_order)
        .unwrap()
        .upload = Some(Upload {
        players: 2,
        per_mille: 420,
    });
    rig.pin(1 + u32::from(rig.id(delta)));
    rig.run(Duration::from_millis(300));
    assert!(
        rig.notices(0).contains(
            &"Delta's machine, the pinned host, carried 42 percent of what 2 players need. Fewer players, or a dedicated server, will fly better."
                .to_owned()
        ),
        "{:?}",
        rig.notices(0)
    );
    assert_eq!(
        words::pinned_cpu("Delta", 130),
        "Delta's machine, the pinned host, needs 130 percent of the time this mission has. Fewer aircraft, or a dedicated server, will fly better."
    );
    assert_eq!(
        words::moved("Hawk", 12),
        "The game moved to Hawk's machine, which can carry 12 players."
    );
}

#[test]
fn the_need_the_router_and_the_cpu_share_follow_the_design() {
    assert_eq!(upload_need(1, 0), 0);
    assert_eq!(upload_need(1, 2), 0, "no other player, no standby");
    assert_eq!(upload_need(2, 0), 28_000 + 3_000 + 2 * 520);
    assert_eq!(upload_need(12, 0), 28_000 * 11 + 2 * (3_000 + 12 * 520));
    // A cold standby adds its checkpoints; one other player has one standby
    // at most, and a third cold standby is not a role.
    assert_eq!(upload_need(2, 1) - upload_need(2, 0), 70_000);
    assert_eq!(upload_need(2, 2), upload_need(2, 1));
    assert_eq!(upload_need(12, 2) - upload_need(12, 0), 2 * 70_000);
    // The need follows the measured streams (docs/baselines/
    // standby-stream-2026-10-05.md): what the host's transport sent a
    // standby, protocol 14, on the simulator.
    assert!(standby_need(3, false) >= 3_900, "3 humans, warm: 3.9 KB/s");
    assert!(
        standby_need(30, false) >= 16_200,
        "30 humans, warm: 16.2 KB/s"
    );
    assert!(
        standby_need(4, true) >= 72_000,
        "real 15 v 15, cold: 72 KB/s"
    );
    assert!(
        standby_need(30, true) >= 34_600,
        "30 humans, cold: 34.6 KB/s"
    );
    assert!(
        standby_need(30, false) > 12_800 && standby_need(30, false) < 30_000,
        "above the old 10 KB/s, not far above the 12.8 KB/s measured"
    );
    let report = |mapping, mapped: bool| CandidateReport {
        may_host: true,
        platform: crate::wire::Platform::Linux,
        processor: crate::wire::migration::Processor::X86_64,
        candidates: if mapped {
            vec![Candidate::new(
                CandidateKind::Mapped,
                a("203.0.113.1:26900"),
            )]
        } else {
            Vec::new()
        },
        mapping,
        cpu_micros: 0,
        cpu_mission: 0,
    };
    assert_eq!(router_rank(&report(MappingType::NoTranslation, false)), 0);
    assert_eq!(
        router_rank(&report(MappingType::PortPerDestination, true)),
        0
    );
    assert_eq!(router_rank(&report(MappingType::SamePort, false)), 1);
    assert_eq!(
        router_rank(&report(MappingType::PortPerDestination, false)),
        2
    );
    assert_eq!(router_rank(&report(MappingType::Unknown, false)), 3);
    assert!(cpu_percent(CPU_BUDGET_MICROS * 1_000 / CPU_BUSY_PER_MILLE) <= 100);
    assert!(cpu_percent(CPU_BUDGET_MICROS * 1_000 / CPU_BUSY_PER_MILLE + 50) > 100);
}

/// A candidate's upload test asks for the cold standbys it would have: the
/// other players that may host and are of another class, at most two.
#[test]
fn the_upload_need_counts_the_cold_standbys_a_candidate_would_have() {
    let mut rig = four();
    let order = |n: usize, rig: &Rig| rig.order(n);
    let (a1, b2, c3) = (order(1, &rig), order(2, &rig), order(3, &rig));
    let other = crate::wire::Platform::ALL
        .into_iter()
        .find(|p| *p != crate::wire::Platform::current() && *p != crate::wire::Platform::Unknown)
        .unwrap();
    assert_eq!(rig.host.cold_standbys_for(a1), 0, "one class: all warm");
    // Bravo is another class: cold for Alpha's game, and Alpha for Bravo's.
    m(&mut rig, b2).report.as_mut().unwrap().platform = other;
    assert_eq!(rig.host.cold_standbys_for(a1), 1);
    assert_eq!(rig.host.cold_standbys_for(c3), 1);
    // Bravo's game sees every other as cold, but only two roles.
    assert_eq!(rig.host.cold_standbys_for(b2), 2);
    // A player who switched hosting off is no standby.
    m(&mut rig, c3).report.as_mut().unwrap().may_host = false;
    assert_eq!(rig.host.cold_standbys_for(a1), 1);
    m(&mut rig, a1).report.as_mut().unwrap().platform = other;
    assert_eq!(
        rig.host.cold_standbys_for(b2),
        1,
        "Alpha is now of its class"
    );
    assert_eq!(
        rig.host.cold_standbys_for(1_000),
        0,
        "a game with no report"
    );
}

#[test]
fn the_candidates_part_round_trips_and_restores_into_a_fresh_host() {
    let mut rig = four();
    rig.host.succession.house_upload = Some(Upload {
        players: 4,
        per_mille: 930,
    });
    let order = rig.order(2);
    rig.host.succession.measures.get_mut(&order).unwrap().upload = Some(Upload {
        players: 4,
        per_mille: 870,
    });
    let part = rig.host.candidates_part();
    let bytes = part.encode().unwrap();
    assert_eq!(CandidatesPart::decode(&bytes).unwrap(), part);
    assert!(CandidatesPart::decode(&bytes[..bytes.len() - 1]).is_err());
    let restored = part.restore().unwrap();
    assert_eq!(restored.players.len(), 4);
    for (order, (seen, measure)) in &restored.players {
        let peer = rig
            .host
            .peers
            .values()
            .find(|p| p.lobby.order == *order)
            .unwrap();
        assert_eq!(*seen, peer.address);
        assert_eq!(measure, &rig.host.succession.measures[order]);
    }
    let mut fresh = Rig::new();
    fresh.host.restore_candidates(&restored);
    assert_eq!(fresh.host.succession.measures, rig.host.succession.measures);
    assert_eq!(fresh.host.succession.house_upload, restored.house_upload);
}

#[test]
fn the_succession_lists_each_ready_standby_with_its_addresses() {
    let mut rig = four();
    let choice = rig.host.standby_choice();
    let ready: Vec<(ConnectionId, bool)> = vec![(choice[0], true), (choice[1], false)];
    rig.host.send_succession(&ready);
    rig.host.send_succession(&ready);
    rig.run(Duration::from_millis(200));
    let message = rig.host.succession_message(&ready);
    assert_eq!(message.standbys.len(), 2);
    let first = &message.standbys[0];
    assert_eq!(first.player, rig.id(1));
    assert!(first.warm && !message.standbys[1].warm);
    assert_eq!(
        first.addresses,
        vec![
            Candidate::new(CandidateKind::Seen, a("198.51.100.11:40000")),
            Candidate::new(CandidateKind::Local, a("198.51.100.11:40000")),
        ]
    );
}

#[test]
fn a_30_player_reach_test_is_bounded() {
    // The worst case: 29 direct players and three candidates, every player
    // with eight IPv6 addresses.
    let addresses: Vec<SocketAddr> = (0..8)
        .map(|n| a(&format!("[2001:db8::{n}]:4000{n}")))
        .collect();
    let target = |player: u8| ReachTarget {
        player,
        addresses: addresses.clone(),
    };
    let encoded = |message: Message| message.encode().unwrap().len();
    let peers = encoded(Message::ReachPeers(Box::new(ReachPeers {
        test: 1,
        players: (0..28).map(target).collect(),
    })));
    let test = encoded(Message::ReachTest(Box::new(ReachTest {
        test: 1,
        candidates: (0..3).map(target).collect(),
    })));
    let report = encoded(Message::ReachReport(Box::new(ReachReport {
        test: 1,
        results: (0..3)
            .map(|player| ReachResult {
                player,
                reached: Some(Reached {
                    address: 7,
                    round_trip_ms: 40,
                }),
            })
            .collect(),
    })));
    // A Reach and its answer: 22 bytes each, five to every address.
    let reach = 22 * 2 * tore_net::peers::REACH_TRIES as usize;
    let candidate_sends = 28 * 8 * reach;
    let tester_sends = 3 * 8 * reach;
    let host = 3 * peers + 29 * test;
    let whole = host + 29 * report + 3 * candidate_sends + 29 * tester_sends;
    println!(
        "Reach peers {peers} B, Reach test {test} B, Reach report {report} B; \
         a candidate's Reaches and answers {candidate_sends} B, a player's {tester_sends} B; \
         the host sends {host} B; the whole test {whole} B"
    );
    assert!(
        peers <= 4_608 && test <= 512 && report <= 16,
        "{peers} {test} {report}"
    );
    // The host sends about 25 KB, a candidate's Reaches and their answers
    // stay under 64 KB in the test's second, and the whole test, once
    // every 10 seconds at most, under 400 KB across 30 machines.
    assert!(host <= 32_768, "{host}");
    assert!(candidate_sends + 29 * reach <= 65_536, "{candidate_sends}");
    assert!(whole <= 400_000, "{whole}");
}

// ----- The punching table, end to end -------------------------------------

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

/// A row: the candidate (the table's host) and the player behind their
/// routers, the house and the host on the open network.
struct Row {
    net: SimNetwork,
    candidate: (&'static str, Option<&'static str>, Option<&'static str>),
    player: (&'static str, Option<&'static str>),
}

impl Row {
    fn new() -> Self {
        let net = SimNetwork::new(17);
        net.set_default_link(LinkConfig::one_way(Duration::from_millis(50)));
        Self {
            net,
            candidate: ("192.168.1.10:26900", None, None),
            player: ("192.168.2.20:40000", None),
        }
    }

    fn add(&self, config: RouterConfig) {
        self.net.add_router(config).unwrap();
    }

    fn socket(net: &SimNetwork, v4: &str, v6: Option<&str>) -> SimSocket {
        match v6 {
            Some(v6) => net.bind_dual(a(v4), a(v6)).unwrap(),
            None => net.bind(a(v4)).unwrap(),
        }
    }

    /// Whether the candidate is eligible once the reach test has run.
    fn eligible(self) -> bool {
        let (c_v4, c_v6, c_mapped) = self.candidate;
        let (p_v4, p_v6) = self.player;
        let c_socket = Self::socket(&self.net, c_v4, c_v6);
        let p_socket = Self::socket(&self.net, p_v4, p_v6);
        let mut rig = Rig::on(self.net);
        rig.join_at(HOUSE, "Viper");
        let mut candidates = vec![Candidate::new(CandidateKind::Local, a(c_v4))];
        candidates.extend(c_mapped.map(|m| Candidate::new(CandidateKind::Mapped, a(m))));
        candidates.extend(c_v6.map(|v6| Candidate::new(CandidateKind::GlobalIpv6, a(v6))));
        let c = rig.join(
            c_socket,
            "Cobra",
            false,
            CandidateSettings {
                candidates,
                ..CandidateSettings::default()
            },
        );
        let mut candidates = vec![Candidate::new(CandidateKind::Local, a(p_v4))];
        candidates.extend(p_v6.map(|v6| Candidate::new(CandidateKind::GlobalIpv6, a(v6))));
        rig.join(
            p_socket,
            "Puma",
            false,
            CandidateSettings {
                candidates,
                ..CandidateSettings::default()
            },
        );
        rig.settle();
        let order = rig.order(c);
        let reached = rig.host.succession.measures[&order].reached_by.clone();
        assert_eq!(reached.len(), 1, "the player reported: {reached:?}");
        rig.host.ranked_candidates().contains(&order)
    }
}

#[test]
fn the_eligible_rows_of_the_punching_table_are_the_punching_rows() {
    // A candidate with no router.
    let mut row = Row::new();
    row.candidate.0 = "198.51.100.10:26900";
    row.add(router("203.0.113.20", "192.168.2.0/24", SYMMETRIC, APDF));
    assert!(row.eligible(), "no router");
    // A mapped port.
    let mut row = Row::new();
    let mut home = router("203.0.113.10", "192.168.1.0/24", EIM, APDF);
    home.forwards.push(Forward {
        outside_port: 26900,
        inside: a("192.168.1.10:26900"),
    });
    row.add(home);
    row.candidate.2 = Some("203.0.113.10:26900");
    row.add(router("203.0.113.20", "192.168.2.0/24", SYMMETRIC, APDF));
    assert!(row.eligible(), "a mapped port");
    // The rows of two home routers.
    let homes = [
        (EIM, APDF, EIM, APDF, true),
        (EIM, ADF, SYMMETRIC, APDF, true),
        (EIM, OPEN, SYMMETRIC, APDF, true),
        (EIM, APDF, SYMMETRIC, APDF, false),
        (SYMMETRIC, APDF, EIM, ADF, true),
        (SYMMETRIC, APDF, EIM, OPEN, true),
        (SYMMETRIC, APDF, EIM, APDF, false),
        (SYMMETRIC, APDF, SYMMETRIC, APDF, false),
    ];
    for (cm, cf, pm, pf, expected) in homes {
        let row = Row::new();
        row.add(router("203.0.113.10", "192.168.1.0/24", cm, cf));
        row.add(router("203.0.113.20", "192.168.2.0/24", pm, pf));
        assert_eq!(row.eligible(), expected, "{cm:?} {cf:?} / {pm:?} {pf:?}");
    }
    // Two routers in front of the candidate, each one port for every
    // destination.
    let row = Row::new();
    row.add(router("203.0.113.40", "10.0.0.0/8", EIM, APDF));
    row.add(router("10.0.0.2", "192.168.1.0/24", EIM, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", EIM, APDF));
    assert!(row.eligible(), "two routers");
    // One home.
    let mut row = Row::new();
    row.add(router("203.0.113.10", "192.168.1.0/24", EIM, APDF));
    row.player.0 = "192.168.1.20:40000";
    row.net.set_link_both(
        a("192.168.1.10:26900"),
        a("192.168.1.20:40000"),
        LinkConfig::one_way(Duration::from_millis(1)),
    );
    assert!(row.eligible(), "one home");
    // A carrier's router and no IPv6.
    let row = Row::new();
    row.add(router("203.0.113.50", "100.64.0.0/10", SYMMETRIC, APDF));
    row.add(router("100.64.0.5", "192.168.1.0/24", EIM, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", EIM, APDF));
    assert!(!row.eligible(), "a carrier's router");
    // IPv6 through two firewalls.
    let mut row = Row::new();
    row.add(router("203.0.113.10", "192.168.1.0/24", SYMMETRIC, APDF));
    row.add(router("203.0.113.20", "192.168.2.0/24", EIM, APDF));
    row.add(RouterConfig::firewall(prefix("2001:db8:1::/48")));
    row.add(RouterConfig::firewall(prefix("2001:db8:2::/48")));
    row.candidate.1 = Some("[2001:db8:1::10]:26900");
    row.player.1 = Some("[2001:db8:2::20]:40000");
    assert!(row.eligible(), "IPv6");
}

// ----- The CPU threshold, fitted on real data ------------------------------

/// The real-data 15 against 15 mission of stage H's checkpoint baseline:
/// five F/A-18Ds in each of three wings against five MiG-29s in each of
/// three, 10 nm apart at 10,000 feet, all AI.
fn crowd() -> MissionSpec {
    let mut spec = MissionSpec::new("UKR", AircraftId::F18);
    for (index, wing) in spec.wings.iter_mut().enumerate() {
        wing.count = 5;
        wing.skill = Skill::Average;
        if index >= 3 {
            wing.aircraft = AircraftId::Mig29;
        }
    }
    spec.separation_nm = 10;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

/// The CPU measure (the first 240 ticks) against the busiest minute of five
/// on the real-data 15 against 15 mission: the ratio [`CPU_BUSY_PER_MILLE`]
/// is fitted from (docs/baselines/host-selection-2026-10-05.md). Run in
/// release, several times:
///
/// ```sh
/// TORE_DATA_DIR=$PWD/.local/DATA cargo test --release --locked \
///     -p tore-session --lib cpu_measure_against_the_busiest_minute -- --ignored --nocapture
/// ```
#[test]
#[ignore = "reads a real import through TORE_DATA_DIR and flies 5 minutes; run by hand in release"]
fn cpu_measure_against_the_busiest_minute() {
    use tore_world::resources::ResourceReads;
    use tore_world::world::{Seating, TickOutput, World};
    let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
    let resources = tore_import::load(&directory).expect("an imported pack");
    let spec = crowd();
    for run in 0..3 {
        let measure = crate::client::candidate::measure_cpu(&spec, &resources, 240).unwrap();
        let reads = ResourceReads::new(&resources);
        let mut world = World::new(&spec, &reads, Seating::Open).unwrap();
        let mut out = TickOutput::default();
        let mut minutes = Vec::new();
        for _ in 0..5 {
            let started = std::time::Instant::now();
            for _ in 0..120 * 60 {
                world.step(&[], &mut out).unwrap();
            }
            minutes.push(started.elapsed().as_micros() as f64 / 7_200.);
        }
        let busiest = minutes.iter().copied().fold(0., f64::max);
        println!(
            "run {run}: measure {measure} us a tick; minutes {:?} us; busiest over measure {:.3}",
            minutes.iter().map(|m| m.round()).collect::<Vec<_>>(),
            busiest / f64::from(measure)
        );
    }
}
