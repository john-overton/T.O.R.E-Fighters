//! Slice F2-1's tests (docs/ARCHITECTURE.md, "The King, the crown and the
//! house" and "Slots, sides and joining"): the King's lobby on the network
//! simulator. A host and full client sessions, each driven through its
//! lobby calls as a lobby screen would; synthetic resources.

use super::*;
use crate::client::{Client, ClientConfig, ClientEvent, ClientPhase, Controls};
use crate::settings::{LoadoutRule, Mode, Visibility, number};
use crate::wire::messages::{Lock, PasswordChange, SettingsChange, Subject, kind};
use tore_formats::aircraft::AircraftId;
use tore_net::Entropy;
use tore_net::sim::{LinkConfig, SimNetwork, SimSocket};
use tore_sim::combat::live::FriendlyFire;
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

/// Friendly Wing 1 of `friendly` and the enemy's Wing 1 of `enemy`, 20 nm
/// apart, airborne: planes 0 to `friendly - 1` are friendly, the rest enemy.
fn spec(friendly: usize, enemy: usize) -> MissionSpec {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = friendly;
    spec.wings[3].count = enemy;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 20;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

/// One player's game.
struct Player {
    socket: SimSocket,
    client: Client,
    events: Vec<ClientEvent>,
}

/// A host and its players on one simulated network.
struct Rig {
    net: SimNetwork,
    host: Host,
    socket: SimSocket,
    players: Vec<Player>,
    logs: Vec<HostLog>,
    next_port: u16,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
}

impl Rig {
    fn new(spec: MissionSpec, configure: impl FnOnce(&mut HostConfig)) -> Self {
        let net = SimNetwork::new(5);
        net.set_default_link(LinkConfig::for_round_trip(20 * MS, 0., 0., 0.));
        let socket = net.bind(host_address()).unwrap();
        let resources = Arc::new(resources());
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
            resources,
        }
    }

    /// A game a player hosts: the first player to join is the house.
    fn hosted(spec: MissionSpec) -> Self {
        Self::new(spec, |config| {
            config.house = Some(Self::address(0));
            config.crown = CrownRule::FirstPlayer;
            config.start = StartMode::King;
            config.restart_delay = Duration::ZERO;
            config.empty_timeout = Duration::ZERO;
            config.listable = true;
        })
    }

    fn address(n: u16) -> SocketAddr {
        format!("10.0.0.2:{}", 40_000 + n).parse().unwrap()
    }

    /// A player who drives the lobby itself (`manual`), or takes a slot
    /// and readies by itself.
    fn join_with(&mut self, callsign: &str, configure: impl FnOnce(&mut ClientConfig)) -> usize {
        let address: SocketAddr = format!("10.0.0.2:{}", self.next_port).parse().unwrap();
        let socket = self.net.bind(address).unwrap();
        let mut config = ClientConfig {
            entropy: Entropy::Seeded(u64::from(self.next_port)),
            auto_ready: false,
            ..ClientConfig::new(host_address(), callsign, build())
        };
        configure(&mut config);
        self.next_port += 1;
        let client = Client::connect(config, Arc::clone(&self.resources), self.net.now()).unwrap();
        self.players.push(Player {
            socket,
            client,
            events: Vec::new(),
        });
        self.players.len() - 1
    }

    fn join(&mut self, callsign: &str) -> usize {
        self.join_with(callsign, |_| {})
    }

    /// One millisecond for everyone.
    fn step(&mut self) {
        self.net.advance(MS);
        let now = self.net.now();
        self.host.receive_from(now, &mut self.socket).unwrap();
        self.host.update(now);
        self.host.transmit(&mut self.socket).unwrap();
        while let Some(log) = self.host.poll_log() {
            self.logs.push(log);
        }
        for player in &mut self.players {
            player.client.receive_from(now, &mut player.socket).unwrap();
            player.client.update(now, &Controls::default());
            player.client.transmit(&mut player.socket).unwrap();
            while let Some(event) = player.client.poll_event() {
                player.events.push(event);
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

    /// Every player in `players` is in the lobby with the mission loaded
    /// and sees all of them.
    fn gather(&mut self, players: &[usize]) {
        let n = players.len();
        assert!(
            self.run_until(Duration::from_secs(3), |r| {
                players.iter().all(|&p| {
                    r.players[p].client.phase() == ClientPhase::Lobby
                        && r.lobby(p).is_some_and(|l| l.players.len() == n)
                })
            }),
            "the players gather"
        );
    }

    fn client(&mut self, player: usize) -> &mut Client {
        &mut self.players[player].client
    }

    fn lobby(&self, player: usize) -> Option<&LobbyState> {
        self.players[player].client.lobby()
    }

    fn id(&self, player: usize) -> u8 {
        self.lobby(player).expect("in the lobby").you
    }

    fn refusals(&self, player: usize) -> Vec<(u8, String)> {
        self.players[player]
            .events
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Refused { request, reason } => Some((*request, reason.clone())),
                ClientEvent::SeatRefused(reason) => Some((kind::SEAT_REFUSED, reason.clone())),
                _ => None,
            })
            .collect()
    }

    fn refused(&self, player: usize, request: u8, reason: &str) -> bool {
        self.refusals(player)
            .iter()
            .any(|(k, r)| *k == request && r == reason)
    }

    fn notices(&self, player: usize) -> Vec<String> {
        self.players[player]
            .events
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Notice(text) => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn change(&mut self, player: usize, values: &[(u8, u32)]) {
        self.client(player).change_settings(SettingsChange {
            values: values.to_vec(),
            ..SettingsChange::default()
        });
        self.run(Duration::from_millis(200));
    }

    fn lobby_events(&self) -> Vec<(String, LobbyEvent)> {
        self.logs
            .iter()
            .filter_map(|log| match log {
                HostLog::Lobby {
                    callsign, event, ..
                } => Some((callsign.clone(), event.clone())),
                _ => None,
            })
            .collect()
    }

    fn seated(&self, player: usize) -> bool {
        self.players[player].client.phase() == ClientPhase::Flying
    }
}

/// The synthetic F/A-18D's standard load.
fn standard() -> tore_world::mission::LoadoutSpec {
    let map = resources();
    let kind = tore_world::aircraft_type::AircraftType::load(&map, AircraftId::F18).unwrap();
    tore_world::mission::LoadoutSpec::of(
        &tore_sim::combat::loadout::Loadout::new(&kind.profile, |name| {
            map.get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
        })
        .unwrap(),
    )
}

#[test]
fn each_setting_the_king_changes_reaches_every_lobby_state() {
    let mut rig = Rig::hosted(spec(2, 2));
    let king = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[king, cobra]);
    let values = [
        (number::MODE, Mode::Pvp.value()),
        (number::MAX_PLAYERS, 12),
        (number::JOIN_IN_PROGRESS, 0),
        (number::VISIBILITY, Visibility::Hidden.value()),
        (number::FRIENDLY_FIRE, 0),
        (number::LOCK_SIDES, 0),
        (number::LOADOUTS, LoadoutRule::Any.value()),
        (number::RESPAWN, 1),
        (number::LIVES, 3),
        (number::REVIVE_DELAY, 120),
        (number::REVIVE_DISTANCE, 40),
        (number::REVIVE_WEAPONS, 3),
        (number::FIGHT, 1),
        (number::TALLY, 2),
        (number::TIME_LIMIT, 1_800),
        (number::KILL_LIMIT, 7),
        (number::KILL_OWNER, 2),
        (number::OBSERVER_DELAY, 60),
        (number::IDLE_AI, 120),
        (number::AI_RESPAWN, 0),
    ];
    let number_before = rig.host.mission_number();
    rig.client(king).change_settings(SettingsChange {
        values: values.to_vec(),
        name: Some("Night owls".into()),
        password: Some(PasswordChange::Set("pw".into())),
    });
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        [king, cobra]
            .iter()
            .all(|&p| r.lobby(p).is_some_and(|l| l.name == "Night owls"))
    }));
    assert!(rig.refusals(king).is_empty(), "{:?}", rig.refusals(king));
    let mut expected: Vec<(u8, u32)> = values.to_vec();
    expected.push((number::PASSWORD, 1));
    // Setting 21, the host, stays calculated (its pin is slice K6's), and
    // setting 22, the snapshot rate, stays at 60 (slice R1).
    expected.push((number::HOST, 0));
    expected.push((number::SNAPSHOT_RATE, 60));
    expected.push((number::AI, 1));
    expected.push((number::MIN_PLAYERS, 0));
    expected.sort_unstable();
    for p in [king, cobra] {
        let lobby = rig.lobby(p).unwrap();
        assert_eq!(lobby.settings, expected, "player {p}");
    }
    assert_eq!(rig.host.settings().password(), Some("pw"));
    // Friendly fire and the loadout rule travel in the mission, under the
    // same number: requests made for it still stand.
    assert_eq!(rig.host.mission_number(), number_before);
    for p in [king, cobra] {
        let spec = rig.players[p].client.spec().unwrap();
        assert!(!spec.friendly_fire && spec.cheat_loadouts, "player {p}");
    }
    let said = rig
        .lobby_events()
        .into_iter()
        .find_map(|(who, event)| match event {
            LobbyEvent::SettingsChanged(words) => Some((who, words)),
            _ => None,
        })
        .unwrap();
    assert_eq!(said.0, "Viper");
    assert!(
        said.1
            .starts_with("mode pvp, max-players 12, join-in-progress off")
            && said.1.ends_with("name \"Night owls\", password set"),
        "{}",
        said.1
    );
}

#[test]
fn a_non_king_and_a_wrong_phase_are_refused() {
    let mut rig = Rig::hosted(spec(2, 2));
    let king = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[king, cobra]);
    rig.change(cobra, &[(number::IDLE_AI, 60)]);
    assert!(rig.refused(cobra, kind::SETTINGS, "Only the King may do that."));
    // The registry's and the host's own words, and nothing applied.
    rig.change(king, &[(number::IDLE_AI, 60), (number::LIVES, 11)]);
    assert!(rig.refused(king, kind::SETTINGS, "lives is 0 to 10 or unlimited."));
    assert_eq!(rig.host.settings().idle_ai_seconds(), Some(300));
    rig.change(king, &[(number::KILL_LIMIT, 3)]);
    assert!(rig.refused(king, kind::SETTINGS, "kill-limit applies only in PvP."));
    rig.change(king, &[(number::MAX_PLAYERS, 1)]);
    assert!(rig.refused(
        king,
        kind::SETTINGS,
        "2 players are connected: the limit cannot be lower."
    ));
    rig.change(king, &[(number::MAX_PLAYERS, 31)]);
    assert!(rig.refused(king, kind::SETTINGS, "max-players is 1 to 30."));

    // In flight: the lobby's settings wait, the game's change at once.
    rig.client(king).take_slot(0);
    rig.run(Duration::from_millis(200));
    rig.client(king).set_ready(true);
    rig.run(Duration::from_millis(200));
    rig.client(king).start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(king)));
    rig.change(king, &[(number::FRIENDLY_FIRE, 0)]);
    assert!(rig.refused(king, kind::SETTINGS, super::king::IN_THE_LOBBY));
    rig.change(
        king,
        &[(number::IDLE_AI, 60), (number::JOIN_IN_PROGRESS, 0)],
    );
    assert_eq!(rig.host.settings().idle_ai_seconds(), Some(60));
    assert!(!rig.host.settings().join_in_progress());
    assert!(rig.host.settings().friendly_fire());
}

#[test]
fn pvp_opens_both_sides_and_a_mode_change_frees_what_is_no_longer_a_slot() {
    let mut rig = Rig::hosted(spec(2, 2));
    let king = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[king, cobra]);
    assert_eq!(rig.lobby(king).unwrap().slots.len(), 2, "co-op: friendly");
    rig.client(cobra).take_slot(2);
    rig.run(Duration::from_millis(200));
    assert!(rig.refused(cobra, kind::SLOT, "Plane 2 is not a slot players may take."));

    rig.change(king, &[(number::MODE, Mode::Pvp.value())]);
    let slots: Vec<u32> = rig
        .lobby(cobra)
        .unwrap()
        .slots
        .iter()
        .map(|s| s.plane)
        .collect();
    assert_eq!(slots, [0, 1, 2, 3], "PvP: both sides");
    assert_eq!(rig.host.status(rig.net.now()).capacity, 4);
    rig.client(cobra).take_slot(2);
    rig.client(cobra).set_ready(true);
    rig.client(king).take_slot(0);
    rig.client(king).set_ready(true);
    rig.run(Duration::from_millis(300));
    assert_eq!(rig.lobby(king).unwrap().me().unwrap().slot, Some(0));

    // Back to co-op: Cobra's enemy slot goes, and Cobra is told; the
    // King's ready mark is cleared as the mode's change clears them all.
    rig.change(king, &[(number::MODE, Mode::Coop.value())]);
    let me = rig.lobby(cobra).unwrap().me().unwrap().clone();
    assert_eq!(me.slot, None);
    assert!(
        rig.notices(cobra)
            .iter()
            .any(|n| n.contains("Plane 2 is no longer a slot")),
        "{:?}",
        rig.notices(cobra)
    );
    assert!(!rig.lobby(king).unwrap().me().unwrap().ready);

    // PvP again, and both sides fly.
    rig.change(king, &[(number::MODE, Mode::Pvp.value())]);
    rig.client(cobra).take_slot(3);
    rig.client(cobra).set_ready(true);
    rig.client(king).set_ready(true);
    rig.run(Duration::from_millis(300));
    rig.client(king).start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(king)
        && r.seated(cobra)));
    assert_eq!(rig.players[cobra].client.seat().unwrap().1, PlaneId(3));
}

#[test]
fn closed_and_reserved_slots_bind_holders_takers_and_the_capacity() {
    let mut rig = Rig::hosted(spec(4, 1));
    let king = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[king, cobra]);
    rig.client(king).take_slot(0);
    rig.client(cobra).take_slot(1);
    rig.run(Duration::from_millis(200));
    assert_eq!(rig.host.status(rig.net.now()).capacity, 4);

    // Closing Cobra's slot frees it and tells Cobra; nobody takes it.
    rig.client(king).lock_slot(1, Lock::Closed);
    rig.run(Duration::from_millis(300));
    assert_eq!(rig.lobby(cobra).unwrap().me().unwrap().slot, None);
    assert!(
        rig.notices(cobra)
            .contains(&"The King closed plane 1's slot: the AI flies it.".to_owned())
    );
    assert_eq!(rig.lobby(cobra).unwrap().slots[1].lock, Lock::Closed);
    assert_eq!(
        rig.host.status(rig.net.now()).capacity,
        3,
        "a closed slot seats nobody"
    );
    rig.client(cobra).take_slot(1);
    rig.client(king).take_slot(1);
    rig.run(Duration::from_millis(200));
    for p in [king, cobra] {
        assert!(rig.refused(p, kind::SLOT, "Plane 1 is closed: the AI flies it."));
    }

    // Reserved for Hawk: Cobra is refused, Hawk takes it.
    rig.client(king).lock_slot(2, Lock::Reserved("Hawk".into()));
    rig.run(Duration::from_millis(200));
    rig.client(cobra).take_slot(2);
    rig.run(Duration::from_millis(200));
    assert!(rig.refused(cobra, kind::SLOT, "Plane 2 is kept for Hawk."));
    let hawk = rig.join("Hawk");
    rig.gather(&[king, cobra, hawk]);
    rig.client(hawk).take_slot(2);
    rig.run(Duration::from_millis(200));
    assert_eq!(rig.lobby(hawk).unwrap().me().unwrap().slot, Some(2));
    assert_eq!(
        rig.lobby(king).unwrap().slots[2].lock,
        Lock::Reserved("Hawk".into())
    );
    // Any free slot skips the locked ones.
    rig.client(cobra).take_any_slot();
    rig.run(Duration::from_millis(400));
    assert_eq!(rig.lobby(cobra).unwrap().me().unwrap().slot, Some(3));

    // A player's lock is the King's alone; a stale one names the change.
    rig.client(cobra).lock_slot(1, Lock::Open);
    rig.run(Duration::from_millis(200));
    assert!(rig.refused(cobra, kind::SLOT_LOCK, "Only the King may do that."));
    rig.client(king).lock_slot(1, Lock::Open);
    rig.run(Duration::from_millis(200));
    assert_eq!(rig.lobby(cobra).unwrap().slots[1].lock, Lock::Open);
    let kept = rig.lobby_events().iter().any(|(who, event)| {
        who == "Viper"
            && *event
                == LobbyEvent::SlotLocked {
                    plane: 2,
                    lock: Lock::Reserved("Hawk".into()),
                }
    });
    assert!(kept);
}

#[test]
fn join_in_progress_off_refuses_every_seating_after_the_first_tick() {
    let mut rig = Rig::new(spec(3, 1), |config| {
        config.settings = vec![(number::JOIN_IN_PROGRESS, 0)];
    });
    // The first player's ready starts the mission and seats it at tick 0.
    let first = rig.join_with("Viper", |c| c.auto_ready = true);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(first)));
    rig.run(Duration::from_millis(500));
    let late = rig.join_with("Cobra", |c| c.auto_ready = true);
    rig.run(Duration::from_secs(2));
    assert!(!rig.seated(late));
    assert!(
        rig.refused(late, kind::SEAT_REFUSED, super::king::NO_NEW_PILOTS),
        "{:?}",
        rig.refusals(late)
    );
    // The first player, too, once it has left its plane.
    let now = rig.net.now();
    rig.client(first).leave(now);
    rig.run(Duration::from_secs(1));
    rig.client(first).ready(Some(0));
    rig.run(Duration::from_millis(500));
    assert!(rig.refused(first, kind::SEAT_REFUSED, super::king::NO_NEW_PILOTS));
}

#[test]
fn lock_sides_keeps_a_player_on_the_side_it_first_flew() {
    let mut rig = Rig::new(spec(2, 2), |config| {
        // PvP's default locks the sides.
        config.settings = vec![(number::MODE, Mode::Pvp.value())];
    });
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    rig.client(viper).ready(Some(0));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let now = rig.net.now();
    rig.client(viper).leave(now);
    rig.run(Duration::from_secs(1));
    rig.client(viper).ready(Some(2));
    rig.run(Duration::from_millis(500));
    assert!(
        rig.refused(viper, kind::SEAT_REFUSED, super::king::SIDES_LOCKED),
        "{:?}",
        rig.refusals(viper)
    );
    // Its own side's other plane is fine.
    rig.client(viper).ready(Some(1));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(viper)));
    assert_eq!(rig.players[viper].client.seat().unwrap().1, PlaneId(1));

    // Without the lock, the other side is open.
    let mut rig = Rig::new(spec(2, 2), |config| {
        config.settings = vec![(number::MODE, Mode::Pvp.value()), (number::LOCK_SIDES, 0)];
    });
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    rig.client(viper).ready(Some(0));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let now = rig.net.now();
    rig.client(viper).leave(now);
    rig.run(Duration::from_secs(1));
    rig.client(viper).ready(Some(2));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(viper)));
    assert_eq!(rig.players[viper].client.seat().unwrap().1, PlaneId(2));
}

#[test]
fn the_loadout_rule_allows_a_cheat_loading_and_drops_it_when_it_turns_back() {
    let mut rig = Rig::hosted(spec(2, 1));
    let king = rig.join("Viper");
    rig.gather(&[king]);
    rig.client(king).take_slot(0);
    rig.run(Duration::from_millis(200));
    let mut cheat = standard();
    cheat.cheat = true;
    rig.client(king).send_loadout(Some(cheat.clone()));
    rig.run(Duration::from_millis(200));
    assert!(rig.refused(
        king,
        kind::LOADOUT,
        "Cheat loading is not allowed in a multiplayer game."
    ));
    rig.change(king, &[(number::LOADOUTS, LoadoutRule::Any.value())]);
    rig.client(king).send_loadout(Some(cheat.clone()));
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        r.lobby(king)
            .is_some_and(|l| l.me().is_some_and(|m| m.loadout))
    }));
    // Back to own: the cheat loading goes, and the player is told.
    rig.change(king, &[(number::LOADOUTS, LoadoutRule::Own.value())]);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        r.lobby(king)
            .is_some_and(|l| l.me().is_some_and(|m| !m.loadout))
    }));
    assert!(
        rig.notices(king)
            .iter()
            .any(|n| n.starts_with("Your loadout for plane 0 does not fit")),
        "{:?}",
        rig.notices(king)
    );
    // Under any, the cheat loading flies: the host's and the player's
    // builds of the flight take it.
    rig.change(king, &[(number::LOADOUTS, LoadoutRule::Any.value())]);
    rig.client(king).send_loadout(Some(cheat));
    rig.run(Duration::from_millis(200));
    rig.client(king).set_ready(true);
    rig.run(Duration::from_millis(200));
    rig.client(king).start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(king)));
    assert!(rig.host.spec().cheat_loadouts);
    assert!(rig.players[king].client.unable().is_none());
}

#[test]
fn friendly_fire_off_reaches_the_flown_mission_on_the_host_and_the_player() {
    let mut rig = Rig::hosted(spec(2, 1));
    let king = rig.join("Viper");
    rig.gather(&[king]);
    rig.change(king, &[(number::FRIENDLY_FIRE, 0)]);
    assert_eq!(
        rig.host.world().combat.state.friendly_fire,
        FriendlyFire::Off
    );
    rig.client(king).take_slot(0);
    rig.run(Duration::from_millis(200));
    rig.client(king).set_ready(true);
    rig.run(Duration::from_millis(200));
    rig.client(king).start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(king)));
    rig.run(Duration::from_millis(500));
    assert_eq!(
        rig.host.world().combat.state.friendly_fire,
        FriendlyFire::Off
    );
    let copy = rig.players[king].client.mission().unwrap();
    assert_eq!(copy.combat.state.friendly_fire, FriendlyFire::Off);
    // The King's own mission text cannot turn it back on.
    rig.client(king).end_mission();
    rig.run(Duration::from_secs(1));
    let mut text = rig.host.spec().clone();
    text.friendly_fire = true;
    rig.client(king).change_mission(&text);
    rig.run(Duration::from_millis(300));
    assert!(!rig.host.spec().friendly_fire);
}

#[test]
fn the_crown_is_passed_used_and_passed_on_when_the_king_leaves() {
    let mut rig = Rig::hosted(spec(3, 1));
    let viper = rig.join("Viper");
    let cobra = rig.join("Cobra");
    let hawk = rig.join("Hawk");
    rig.gather(&[viper, cobra, hawk]);
    let (viper_id, cobra_id) = (rig.id(viper), rig.id(cobra));
    let lobby = rig.lobby(hawk).unwrap();
    assert_eq!((lobby.king, lobby.host), (Some(viper_id), Some(viper_id)));

    rig.client(viper).pass_crown(viper_id);
    rig.run(Duration::from_millis(200));
    assert!(rig.refused(viper, kind::PASS_CROWN, "You wear the crown already."));
    rig.client(viper).pass_crown(cobra_id);
    assert!(rig.run_until(Duration::from_secs(1), |r| {
        r.lobby(hawk).is_some_and(|l| l.king == Some(cobra_id))
    }));
    // The house stays the house.
    assert_eq!(rig.lobby(hawk).unwrap().host, Some(viper_id));
    // The new King rules; the old one is refused.
    rig.change(viper, &[(number::IDLE_AI, 120)]);
    assert!(rig.refused(viper, kind::SETTINGS, "Only the King may do that."));
    rig.change(cobra, &[(number::IDLE_AI, 120)]);
    assert_eq!(rig.host.settings().idle_ai_seconds(), Some(120));
    // The house cannot be kicked: its leaving would end the game.
    rig.client(cobra).kick(viper_id, "bye");
    rig.run(Duration::from_millis(200));
    assert!(rig.refused(
        cobra,
        kind::KICK,
        "The house runs the game; it cannot be kicked."
    ));

    // The King leaves: the crown passes to the longest-connected, the
    // house, and the game goes on.
    let now = rig.net.now();
    rig.client(cobra).leave_game(now);
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.lobby(hawk)
            .is_some_and(|l| l.king == Some(viper_id) && l.players.len() == 2)
    }));
    assert_eq!(rig.host.phase(), Phase::Lobby);
    assert!(
        rig.lobby_events()
            .iter()
            .any(|(who, e)| who == "Viper" && *e == LobbyEvent::Crowned)
    );
    assert!(
        rig.lobby_events()
            .iter()
            .any(|(who, e)| { who == "Viper" && *e == LobbyEvent::CrownPassed("Cobra".into()) })
    );
}

#[test]
fn the_houses_leaving_ends_the_game_and_a_kings_does_not() {
    let mut rig = Rig::hosted(spec(3, 1));
    let viper = rig.join("Viper");
    let cobra = rig.join("Cobra");
    let hawk = rig.join("Hawk");
    rig.gather(&[viper, cobra, hawk]);
    let cobra_id = rig.id(cobra);
    rig.client(viper).pass_crown(cobra_id);
    rig.run(Duration::from_millis(300));
    // The house leaves without the crown: the game is over all the same.
    let now = rig.net.now();
    rig.client(viper).leave_game(now);
    assert!(rig.run_until(Duration::from_secs(8), |r| {
        [cobra, hawk]
            .iter()
            .all(|&p| r.players[p].client.phase() == ClientPhase::Closed)
    }));
    for p in [cobra, hawk] {
        assert_eq!(
            rig.players[p].client.goodbye(),
            Some(&messages::Goodbye::HostLeft)
        );
    }
}

#[test]
fn a_servers_first_player_is_king_and_its_start_rule_gives_way_to_the_king() {
    // A server with no King: as built.
    let mut rig = Rig::new(spec(3, 1), |_| {});
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    assert_eq!(rig.lobby(viper).unwrap().king, None);
    rig.change(viper, &[(number::IDLE_AI, 120)]);
    assert!(rig.refused(viper, kind::SETTINGS, "Only the King may do that."));

    // `king first-player`: the first to join wears the crown, and the
    // server waits for the King's start while a King is connected.
    let mut rig = Rig::new(spec(3, 1), |config| config.crown = CrownRule::FirstPlayer);
    let viper = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[viper, cobra]);
    let lobby = rig.lobby(cobra).unwrap();
    assert_eq!(lobby.king, Some(rig.id(viper)));
    assert_eq!(lobby.host, None, "a server has no house");
    assert_eq!(lobby.start, messages::StartRule::King);
    rig.client(cobra).take_slot(1);
    rig.run(Duration::from_millis(200));
    rig.client(cobra).set_ready(true);
    rig.run(Duration::from_millis(500));
    assert_eq!(
        rig.host.phase(),
        Phase::Lobby,
        "a ready player no longer starts it"
    );
    rig.change(viper, &[(number::MODE, Mode::Pvp.value())]);
    assert_eq!(rig.host.settings().mode(), Mode::Pvp);
    rig.client(cobra).set_ready(true);
    rig.run(Duration::from_millis(300));
    rig.client(viper).start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(cobra)));

    // The King leaves: Cobra wears the crown.
    let now = rig.net.now();
    rig.client(viper).leave_game(now);
    let cobra_id = rig.id(cobra);
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.lobby(cobra).is_some_and(|l| l.king == Some(cobra_id))
    }));
}

#[test]
fn a_locked_servers_king_starts_kicks_and_passes_but_changes_nothing() {
    let mut rig = Rig::new(spec(3, 1), |config| {
        config.crown = CrownRule::FirstPlayer;
        config.mission_locked = true;
    });
    let viper = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[viper, cobra]);
    rig.change(viper, &[(number::IDLE_AI, 120)]);
    assert!(rig.refused(viper, kind::SETTINGS, super::king::MISSION_LOCKED));
    let spec = rig.host.spec().clone();
    rig.client(viper).change_mission(&spec);
    rig.run(Duration::from_millis(200));
    assert!(rig.refused(viper, kind::CHANGE_MISSION, super::king::MISSION_LOCKED));
    rig.client(viper).lock_slot(2, Lock::Closed);
    let cobra_id = rig.id(cobra);
    rig.client(viper).pass_crown(cobra_id);
    rig.run(Duration::from_millis(300));
    assert_eq!(rig.lobby(viper).unwrap().king, Some(rig.id(cobra)));
    assert_eq!(rig.lobby(viper).unwrap().slots[2].lock, Lock::Closed);
}

#[test]
fn a_locked_server_tells_every_lobby_so_and_a_hosted_game_never_does() {
    let mut rig = Rig::new(spec(3, 1), |config| {
        config.crown = CrownRule::FirstPlayer;
        config.mission_locked = true;
    });
    let viper = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[viper, cobra]);
    assert!(rig.lobby(viper).unwrap().mission_locked, "the King's lobby");
    assert!(rig.lobby(cobra).unwrap().mission_locked, "a joiner's lobby");
    // An open server and a game a player hosts say nothing of the kind.
    let mut rig = Rig::new(spec(3, 1), |config| config.crown = CrownRule::FirstPlayer);
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    assert!(!rig.lobby(viper).unwrap().mission_locked);
    let mut rig = Rig::hosted(spec(3, 1));
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    assert!(!rig.lobby(viper).unwrap().mission_locked);
}

#[test]
fn a_crowned_server_left_empty_goes_back_to_its_file() {
    let mut rig = Rig::new(spec(2, 2), |config| {
        config.crown = CrownRule::FirstPlayer;
        config.empty_timeout = Duration::from_secs(1);
        config.time_limit = Some(Duration::from_secs(45 * 60));
    });
    let file = rig.host.spec().clone();
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    rig.change(
        viper,
        &[
            (number::MODE, Mode::Pvp.value()),
            (number::FRIENDLY_FIRE, 0),
        ],
    );
    rig.client(viper).lock_slot(3, Lock::Closed);
    rig.run(Duration::from_millis(200));
    assert_eq!(rig.host.settings().mode(), Mode::Pvp);
    assert!(!rig.host.spec().friendly_fire);
    let now = rig.net.now();
    rig.client(viper).leave_game(now);
    rig.run(Duration::from_secs(3));
    assert_eq!(rig.host.settings().mode(), Mode::Coop);
    assert_eq!(
        rig.host.settings().time_limit_seconds(),
        Some(2_700),
        "the file's limit"
    );
    assert_eq!(*rig.host.spec(), file);
    assert!(
        rig.lobby_events()
            .iter()
            .any(|(_, e)| *e == LobbyEvent::BackToFile)
    );
    // The next player finds the file's mission, with no locks.
    let cobra = rig.join("Cobra");
    rig.gather(&[cobra]);
    let lobby = rig.lobby(cobra).unwrap();
    assert_eq!(lobby.king, Some(rig.id(cobra)));
    assert!(lobby.slots.iter().all(|s| s.lock == Lock::Open));
}

#[test]
fn the_password_the_limit_and_the_visibility_apply_to_the_next_joins() {
    let mut rig = Rig::hosted(spec(3, 1));
    let king = rig.join("Viper");
    rig.gather(&[king]);
    rig.client(king).change_settings(SettingsChange {
        values: vec![(number::MAX_PLAYERS, 2)],
        name: None,
        password: Some(PasswordChange::Set("swordfish".into())),
    });
    rig.run(Duration::from_millis(200));
    assert_eq!(rig.host.status(rig.net.now()).capacity, 2);
    // Without the password: refused; with it: in.
    let stranger = rig.join("Stranger");
    rig.run(Duration::from_secs(1));
    assert!(matches!(
        rig.players[stranger].client.phase(),
        ClientPhase::Closed
    ));
    assert!(rig.logs.iter().any(|log| matches!(
        log,
        HostLog::Refused { callsign, reason, .. } if callsign == "Stranger" && reason == "Wrong password."
    )));
    let cobra = rig.join_with("Cobra", |c| c.password = "swordfish".into());
    rig.gather(&[king, cobra]);
    // Full at two: an observer counts against the room (agent decision,
    // F2-O1 note 9), so a full game takes none.
    let owl = rig.join_with("Owl", |c| c.password = "swordfish".into());
    rig.run(Duration::from_secs(1));
    assert_eq!(rig.players[owl].client.phase(), ClientPhase::Closed);
    assert!(rig.logs.iter().any(|log| matches!(
        log,
        HostLog::Refused { callsign, reason, .. }
            if callsign == "Owl" && reason == "The server is full (2 players)."
    )));

    // Hidden answers no search; local does.
    let asker: SocketAddr = "10.0.9.9:5000".parse().unwrap();
    let query = tore_net::packet::Packet::Discover(tore_net::packet::Discover {
        protocol_version: PROTOCOL_VERSION,
        nonce: 7,
    })
    .encode(PROTOCOL_VERSION)
    .unwrap();
    let answered = |rig: &mut Rig| {
        let now = rig.net.now();
        rig.host.receive(now, asker, &query);
        let mut answered = false;
        while let Some(sent) = rig.host.poll_transmit() {
            answered |= sent.to == asker;
        }
        answered
    };
    assert!(answered(&mut rig));
    rig.change(king, &[(number::VISIBILITY, Visibility::Hidden.value())]);
    assert!(!answered(&mut rig));
    rig.change(king, &[(number::VISIBILITY, Visibility::Local.value())]);
    assert!(answered(&mut rig));
    // The answer carries the King's name and the lock.
    let answer = rig.host.discover_answer(1);
    assert!(answer.password && answer.full);
}

#[test]
fn a_server_refuses_public_and_a_hosted_game_takes_it() {
    let mut rig = Rig::new(spec(2, 1), |config| config.crown = CrownRule::FirstPlayer);
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    rig.change(viper, &[(number::VISIBILITY, Visibility::Public.value())]);
    assert!(rig.refused(viper, kind::SETTINGS, super::config::PUBLIC_IS_BROADCAST));
    let mut rig = Rig::hosted(spec(2, 1));
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    rig.change(viper, &[(number::VISIBILITY, Visibility::Public.value())]);
    assert_eq!(rig.host.settings().visibility(), Visibility::Public);
}

#[test]
fn watching_is_logged_when_it_starts_and_stops() {
    let mut rig = Rig::new(spec(2, 1), |config| config.start = StartMode::Now);
    let owl = rig.join("Owl");
    rig.gather(&[owl]);
    rig.client(owl).watch(Subject::Aircraft(0));
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.lobby_events()
            .iter()
            .any(|(who, e)| who == "Owl" && *e == LobbyEvent::Watching(true))
    }));
    rig.client(owl).stop_watching();
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.lobby_events()
            .iter()
            .any(|(who, e)| who == "Owl" && *e == LobbyEvent::Watching(false))
    }));
}

/// John, 2026-10-06 (Q56), slice R1: a game a player hosts has a lobby
/// setting for the snapshot rate, 60 (default), 30 or 20, and the flight
/// that starts follows it on the host and on every player.
#[test]
fn the_king_turns_the_snapshot_rate_in_the_lobby_and_the_flight_follows_it() {
    let mut rig = Rig::hosted(spec(2, 1));
    let viper = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[viper, cobra]);
    assert_eq!(rig.host.snapshot_rate(), 60);
    assert_eq!(rig.client(cobra).ticks_per_snapshot(), 2);

    // A rate off the King's list is refused with the list.
    rig.change(viper, &[(number::SNAPSHOT_RATE, 24)]);
    assert!(rig.refused(
        viper,
        kind::SETTINGS,
        "snapshot-rate is 60 a second, 30 a second or 20 a second."
    ));
    assert_eq!(rig.host.snapshot_rate(), 60);

    // 30 applies, every lobby shows it, and a player's flight starts from it
    // though its Accepted packet said 2 ticks.
    rig.change(viper, &[(number::SNAPSHOT_RATE, 30)]);
    assert_eq!(rig.host.snapshot_rate(), 30);
    assert_eq!(rig.host.ticks_per_snapshot(), 4);
    for p in [viper, cobra] {
        let shown = rig.lobby(p).unwrap().settings.clone();
        assert!(shown.contains(&(number::SNAPSHOT_RATE, 30)), "{shown:?}");
    }
    assert_eq!(
        rig.client(cobra).ticks_per_snapshot(),
        2,
        "not in the lobby"
    );
    rig.client(cobra).take_slot(1);
    rig.run(Duration::from_millis(200));
    rig.client(cobra).set_ready(true);
    rig.run(Duration::from_millis(300));
    rig.client(viper).start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(cobra)));
    assert_eq!(rig.client(cobra).ticks_per_snapshot(), 4);

    // Snapshots come every fourth tick, 30 a second.
    let mut ticks: Vec<u32> = Vec::new();
    let end = rig.net.now() + Duration::from_secs(2);
    while rig.net.now() < end {
        rig.step();
        if let Some(tick) = rig.client(cobra).snapshot_tick()
            && ticks.last() != Some(&tick)
        {
            ticks.push(tick);
        }
    }
    assert!(ticks.len() >= 55, "{} snapshots in 2 seconds", ticks.len());
    assert!(
        ticks.windows(2).all(|pair| pair[1] - pair[0] == 4),
        "every fourth tick: {ticks:?}"
    );

    // Not in flight: the rate is a lobby setting.
    rig.change(viper, &[(number::SNAPSHOT_RATE, 20)]);
    assert!(rig.refused(viper, kind::SETTINGS, super::king::IN_THE_LOBBY));
    assert_eq!(rig.host.snapshot_rate(), 30);
}

/// A mode change resets what the King changes in the lobby, but not the
/// snapshot rate, which belongs to the game (agent decision, slice R1).
#[test]
fn a_new_mode_keeps_the_snapshot_rate() {
    let mut rig = Rig::hosted(spec(2, 1));
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    rig.change(viper, &[(number::SNAPSHOT_RATE, 20)]);
    rig.change(viper, &[(number::MODE, Mode::Pvp.value())]);
    assert_eq!(rig.host.settings().mode(), Mode::Pvp);
    assert_eq!(rig.host.snapshot_rate(), 20);
}

/// A dedicated server's snapshot rate is its file's: the King may not turn
/// it, and the lobby state carries it as given, whatever the rate.
#[test]
fn a_dedicated_servers_rate_is_its_files_and_the_king_may_not_turn_it() {
    let mut rig = Rig::new(spec(2, 1), |config| {
        config.crown = CrownRule::FirstPlayer;
        config.snapshot_rate = 24;
    });
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    assert_eq!(rig.host.snapshot_rate(), 24);
    assert_eq!(rig.host.ticks_per_snapshot(), 5);
    assert!(
        rig.lobby(viper)
            .unwrap()
            .settings
            .contains(&(number::SNAPSHOT_RATE, 24))
    );
    rig.change(viper, &[(number::SNAPSHOT_RATE, 30)]);
    assert!(rig.refused(viper, kind::SETTINGS, super::config::RATE_IS_THE_FILES));
    assert_eq!(rig.host.snapshot_rate(), 24);
    // The mode may change on it and the rate stays.
    rig.change(viper, &[(number::MODE, Mode::Pvp.value())]);
    assert_eq!(rig.host.snapshot_rate(), 24);
}

// The lobby pass's side requests (slice W0) drive this file's rig.
#[path = "sides_tests.rs"]
mod sides_tests;

// The lobby pass's Autobalance (slice A1) drives this file's rig.
#[path = "balance_tests.rs"]
mod balance_tests;

#[test]
fn each_human_flight_shares_its_load_on_host_clients_and_ai_respawn() {
    let mut mission = spec(4, 3);
    // An authored wingman load is never overwritten.
    let mut authored = standard();
    authored.stations[1].quantity = 1;
    mission.plane_loadouts.insert(3, authored.clone());
    // Host::new accepts authored plane loads, unlike changing the lobby mission.
    let mut rig = Rig::hosted(mission);
    let blue = rig.join("Viper");
    let red = rig.join("Cobra");
    rig.gather(&[blue, red]);
    rig.change(
        blue,
        &[
            (number::MODE, Mode::Pvp.value()),
            (number::LOCK_SIDES, 0),
            (number::RESPAWN, crate::settings::Respawn::Revive.value()),
            (number::REVIVE_DELAY, 0),
        ],
    );
    let mut blue_load = standard();
    blue_load.stations[1].quantity = 0;
    blue_load.fuel_lbs *= 0.5;
    let mut red_load = standard();
    red_load.stations[0].quantity = 123;
    for (client, plane, load) in [(blue, 0, blue_load.clone()), (red, 4, red_load.clone())] {
        rig.client(client).take_slot(plane);
        assert!(rig.run_until(Duration::from_secs(2), |r| {
            r.lobby(client)
                .is_some_and(|l| l.me().is_some_and(|p| p.slot == Some(plane)))
        }));
        rig.client(client).send_loadout(Some(load));
        rig.run(Duration::from_millis(200));
        assert!(
            rig.refusals(client).is_empty(),
            "{:?}",
            rig.refusals(client)
        );
        assert_eq!(rig.lobby(client).unwrap().me().unwrap().slot, Some(plane));
        rig.client(client).set_ready(true);
    }
    rig.run(Duration::from_millis(200));
    rig.client(blue).start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(blue) && r.seated(red)));
    for world in std::iter::once(rig.host.world())
        .chain(rig.players.iter().map(|p| p.client.mission().unwrap()))
    {
        let loads = &world.setup.ai.as_ref().unwrap().loadouts;
        for id in [0, 1, 2] {
            assert_eq!(tore_world::mission::LoadoutSpec::of(&loads[&id]), blue_load);
        }
        assert_eq!(tore_world::mission::LoadoutSpec::of(&loads[&3]), authored);
        for id in [4, 5, 6] {
            assert_eq!(tore_world::mission::LoadoutSpec::of(&loads[&id]), red_load);
        }
    }
    rig.host
        .world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == 1)
        .unwrap()
        .hp = 0;
    assert!(rig.run_until(
        Duration::from_secs(3),
        |r| r.host.world.lineage_head(PlaneId(1)) != PlaneId(1)
    ));
    let revived = rig.host.world.lineage_head(PlaneId(1));
    rig.run(Duration::from_millis(200));
    for world in std::iter::once(rig.host.world())
        .chain(rig.players.iter().map(|p| p.client.mission().unwrap()))
    {
        let actor = world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actor(revived.0)
            .unwrap();
        assert_eq!(
            actor.stations()[1].store.rounds,
            tore_sim::ai::weapon_service::Rounds::Finite(0)
        );
    }
}

#[test]
fn ai_off_waits_for_full_ready_slots_and_despawns_unheld_enemy_aircraft() {
    let mut rig = Rig::hosted(spec(2, 2));
    let king = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[king, cobra]);
    rig.change(king, &[(number::AI, 0)]);
    rig.client(king).take_slot(0);
    rig.client(king).set_ready(true);
    rig.run(300 * MS);
    assert!(rig.host.ai_wait_reason().unwrap().contains("1 more"));
    rig.host.start_now();
    assert!(matches!(rig.host.life, Life::Lobby));
    rig.client(cobra).take_slot(1);
    rig.client(cobra).set_ready(true);
    rig.run(300 * MS);
    assert_eq!(rig.host.ai_wait_reason(), None);
    rig.client(king).start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(king)
        && r.seated(cobra)));
    assert!(rig.host.world.combat.state.targets.is_empty());
    assert_eq!(
        rig.host
            .world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actors()
            .len(),
        0
    );
    assert_eq!(rig.host.world.cockpits.len(), 2);
}

#[test]
fn ai_off_departure_reopens_slot_and_a_late_join_uses_a_fresh_plane() {
    let mut rig = Rig::hosted(spec(2, 0));
    let king = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[king, cobra]);
    rig.change(king, &[(number::AI, 0)]);
    rig.client(king).take_slot(0);
    rig.client(king).set_ready(true);
    rig.client(cobra).take_slot(1);
    rig.client(cobra).set_ready(true);
    rig.run(300 * MS);
    rig.client(king).start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(king)
        && r.seated(cobra)));
    let now = rig.net.now();
    rig.client(cobra).leave_game(now);
    rig.run(Duration::from_secs(1));
    // The log says the departed player's aircraft was removed, as a note.
    assert!(
        rig.logs.iter().any(|log| matches!(
            log,
            HostLog::Note { text, .. } if text == "Cobra left: plane 1 removed (AI is off)"
        )),
        "{:?}",
        rig.logs
    );
    assert!(
        rig.host.world.combat.state.targets.is_empty(),
        "targets: {:?}, actors: {}, cockpits: {}",
        rig.host
            .world
            .combat
            .state
            .targets
            .iter()
            .map(|t| (t.id, t.hp))
            .collect::<Vec<_>>(),
        rig.host
            .world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actors()
            .len(),
        rig.host.world.cockpits.len()
    );
    assert_eq!(rig.host.world.cockpits.len(), 1);
    let late = rig.join("Falcon");
    rig.run(Duration::from_secs(1));
    rig.client(late).take_slot(1);
    rig.client(late).set_ready(true);
    assert!(
        rig.run_until(Duration::from_secs(3), |r| r.seated(late)),
        "{:?}",
        rig.refusals(late)
    );
    assert!(rig.players[late].client.seat().unwrap().1.0 > 1);
    let order = rig
        .host
        .peers
        .values()
        .find(|p| p.callsign == "Falcon")
        .unwrap()
        .lobby
        .order;
    assert_eq!(rig.host.revival.players[&order].used, 0);
}

#[test]
fn a_player_choice_is_built_as_the_chosen_aircraft_on_host_and_peers() {
    let mut mission = spec(2, 1);
    mission.player_choice[0] = true;
    let mut rig = Rig::hosted(mission.clone());
    // Use the existing, correctly identified synthetic gunship fixture.
    let data = tore_world::test_support::resources::gunship_resources();
    rig.resources = Arc::new(data);
    rig.host = Host::new(mission, Arc::clone(&rig.resources), rig.host.config.clone()).unwrap();
    let king = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[king, cobra]);
    rig.change(
        king,
        &[
            (number::RESPAWN, crate::settings::Respawn::Revive.value()),
            (number::AI_RESPAWN, 1),
            (number::IDLE_AI, 60),
            (number::REVIVE_DELAY, 0),
        ],
    );
    rig.client(king).take_slot(0);
    rig.client(cobra).take_slot(1);
    rig.run(200 * MS);
    rig.client(king)
        .send_aircraft_loadout(Some(AircraftId::Ac130), None);
    rig.client(cobra)
        .send_aircraft_loadout(Some(AircraftId::Ac130), None);
    rig.run(200 * MS);
    assert!(rig.refusals(cobra).is_empty(), "{:?}", rig.refusals(cobra));
    assert_eq!(
        rig.host.spec.plane_aircraft.get(&1),
        Some(&AircraftId::Ac130)
    );
    rig.client(king).set_ready(true);
    rig.client(cobra).set_ready(true);
    rig.run(200 * MS);
    rig.client(king).start_mission();
    assert!(
        rig.run_until(Duration::from_secs(3), |r| r.seated(king)
            && r.seated(cobra)),
        "{:?}",
        rig.logs
    );
    assert_eq!(
        rig.host.world.aircraft_of(PlaneId(1)),
        Some(AircraftId::Ac130)
    );
    for player in [king, cobra] {
        assert_eq!(
            rig.players[player]
                .client
                .spec()
                .unwrap()
                .plane_aircraft
                .get(&1),
            Some(&AircraftId::Ac130)
        );
    }
    rig.client(cobra).away();
    rig.run(300 * MS);
    assert_eq!(
        rig.host.world.roster.plane(PlaneId(1)).unwrap().pilot,
        Pilot::Ai
    );
    assert!(
        !rig.host.world.head_lost(PlaneId(1)),
        "going away keeps its selected plane"
    );
    assert_eq!(
        rig.host.world.aircraft_of(PlaneId(1)),
        Some(AircraftId::Ac130)
    );
    rig.client(cobra).back();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(cobra)));
    let now = rig.net.now();
    rig.client(cobra).leave_game(now);
    assert!(
        rig.run_until(Duration::from_secs(3), |r| {
            let head = r.host.world.lineage_head(PlaneId(1));
            head != PlaneId(1) && r.host.world.aircraft_of(head) == Some(AircraftId::F18)
        }),
        "a Choice wing whose lead also chose AC-130 must return to the authored F/A-18D AI default"
    );
}

#[test]
fn choice_revive_loads_a_new_type_on_host_and_both_clients() {
    let mut mission = spec(3, 1);
    mission.player_choice[0] = true;
    let mut rig = Rig::hosted(mission.clone());
    rig.resources = Arc::new(tore_world::test_support::resources::gunship_resources());
    rig.host = Host::new(mission, Arc::clone(&rig.resources), rig.host.config.clone()).unwrap();
    let viper = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[viper, cobra]);
    rig.change(
        viper,
        &[
            (number::RESPAWN, crate::settings::Respawn::Revive.value()),
            (number::REVIVE_DELAY, 0),
        ],
    );
    for (player, slot) in [(viper, 0), (cobra, 1)] {
        rig.client(player).take_slot(slot);
        rig.client(player).set_ready(true);
    }
    rig.run(300 * MS);
    rig.client(viper).start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)
        && r.seated(cobra)));
    rig.host
        .world_for_test()
        .cockpits
        .iter_mut()
        .find(|c| c.plane == PlaneId(0))
        .unwrap()
        .flight
        .systems
        .pilot
        .dead = true;
    rig.run(300 * MS);
    let kind =
        tore_world::aircraft_type::AircraftType::load(&*rig.resources, AircraftId::Ac130).unwrap();
    let mut load = tore_sim::combat::loadout::Loadout::new(&kind.profile, |name| {
        Ok(rig.resources[name].clone())
    })
    .unwrap();
    load.quantities[0] /= 2;
    let chosen = tore_world::mission::LoadoutSpec::of(&load);
    // A malformed request must not stage either half of the selection.
    let connection = *rig
        .host
        .peers
        .iter()
        .find(|(_, p)| p.callsign == "Viper")
        .unwrap()
        .0;
    let previous = rig.host.peers[&connection].lobby.loadout.clone();
    let mut invalid = chosen.clone();
    invalid.stations.clear();
    assert!(
        rig.host
            .revive_selected(connection, Some(AircraftId::Ac130), Some(invalid))
            .is_err()
    );
    assert!(!rig.host.revival.pending.contains_key(&connection));
    assert!(!rig.host.revival.choices.contains_key(&PlaneId(0)));
    assert_eq!(rig.host.peers[&connection].lobby.loadout, previous);
    rig.client(viper)
        .revive_choice(Some(AircraftId::Ac130), Some(chosen.clone()));
    assert!(
        rig.run_until(Duration::from_secs(3), |r| r.players[viper]
            .client
            .seat()
            .is_some_and(|(_, p)| p != PlaneId(0))),
        "refused {:?}, logs {:?}, pending {:?}",
        rig.refusals(viper),
        rig.logs,
        rig.host.revival.pending
    );
    rig.run(300 * MS);
    let new = rig.players[viper].client.seat().unwrap().1;
    assert_eq!(rig.host.world.aircraft_of(new), Some(AircraftId::Ac130));
    for player in [viper, cobra] {
        let client = &rig.players[player].client;
        assert_eq!(
            client.mission().unwrap().aircraft_of(new),
            Some(AircraftId::Ac130)
        );
        let spawned = client.spawned().iter().find(|s| s.plane == new.0).unwrap();
        assert_eq!(spawned.spawn.loadout.stations, chosen.stations);
    }
    let checksum = |player: usize| {
        tore_codec::hash::fnv1a64(
            &rig.players[player]
                .client
                .mission()
                .unwrap()
                .checkpoint()
                .unwrap(),
        )
    };
    assert_eq!(
        checksum(viper),
        checksum(cobra),
        "both clients built the accepted runtime type and loadout identically"
    );
    // A later entrant without the runtime type stays unable in the lobby.
    let mut missing = (*rig.resources).clone();
    missing.remove("AC130.PT");
    rig.resources = Arc::new(missing);
    let late = rig.join("Missing");
    rig.run(Duration::from_secs(1));
    assert!(
        rig.lobby(late)
            .and_then(|l| l.me())
            .and_then(|p| p.unable.as_ref())
            .is_some()
    );
}

#[test]
fn choice_late_join_replaces_ai_and_departure_returns_the_default() {
    let mut mission = spec(2, 1);
    mission.player_choice[0] = true;
    let data = tore_world::test_support::resources::gunship_resources();
    let kind = tore_world::aircraft_type::AircraftType::load(&data, AircraftId::F18).unwrap();
    let mut load =
        tore_sim::combat::loadout::Loadout::new(&kind.profile, |name| Ok(data[name].clone()))
            .unwrap();
    load.quantities[0] /= 2;
    let authored = tore_world::mission::LoadoutSpec::of(&load);
    mission.plane_loadouts.insert(1, authored.clone());

    let mut rig = Rig::hosted(mission.clone());
    rig.resources = Arc::new(tore_world::test_support::resources::gunship_resources());
    rig.host = Host::new(mission, Arc::clone(&rig.resources), rig.host.config.clone()).unwrap();
    let king = rig.join("Viper");
    rig.gather(&[king]);
    rig.change(
        king,
        &[
            (number::RESPAWN, crate::settings::Respawn::Revive.value()),
            (number::AI_RESPAWN, 1),
            (number::IDLE_AI, 60),
            (number::REVIVE_DELAY, 0),
        ],
    );
    rig.client(king).take_slot(0);
    rig.client(king).set_ready(true);
    rig.run(300 * MS);
    rig.client(king).start_mission();
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(king)));
    let old_pose = rig.host.world.plane_pose(PlaneId(1)).unwrap();
    let late = rig.join("Cobra");
    rig.run(Duration::from_secs(1));
    rig.client(late).take_slot(1);
    rig.run(200 * MS);
    rig.client(late)
        .send_aircraft_loadout(Some(AircraftId::Ac130), None);
    rig.run(200 * MS);
    rig.client(late).set_ready(true);
    assert!(
        rig.run_until(Duration::from_secs(3), |r| r.seated(late)),
        "{:?}",
        rig.refusals(late)
    );
    let new = rig.players[late].client.seat().unwrap().1;
    assert_ne!(new, PlaneId(1));
    assert_eq!(rig.host.world.aircraft_of(new), Some(AircraftId::Ac130));
    assert!(
        rig.host
            .world
            .ai_wings
            .as_ref()
            .unwrap()
            .mission()
            .actor(1)
            .is_none()
    );
    assert_ne!(rig.host.world.plane_pose(new).unwrap().0, old_pose.0);
    rig.client(late).away();
    rig.run(300 * MS);
    assert!(!rig.host.world.head_lost(new));
    assert_eq!(rig.host.world.roster.plane(new).unwrap().pilot, Pilot::Ai);
    let now = rig.net.now();
    rig.client(late).leave_game(now);
    assert!(
        rig.run_until(Duration::from_secs(3), |r| {
            let head = r.host.world.lineage_head(PlaneId(1));
            head != new
                && head != PlaneId(1)
                && r.host.world.aircraft_of(head) == Some(AircraftId::F18)
        }),
        "{:?}",
        rig.logs
    );
    let head = rig.host.world.lineage_head(PlaneId(1));
    let returned = rig
        .host
        .revival
        .spawned
        .iter()
        .find(|spawned| spawned.plane == head.0)
        .unwrap();
    assert_eq!(
        returned.spawn.loadout.stations, authored.stations,
        "a matching authored AI load survives Choice departure"
    );
}

#[test]
fn the_king_cannot_change_the_minimum_players_and_a_mode_change_keeps_it() {
    let mut rig = Rig::new(spec(2, 0), |config| {
        config.settings = vec![(number::MIN_PLAYERS, 2)];
        config.crown = CrownRule::FirstPlayer;
        config.start = StartMode::FirstPlayer;
    });
    let king = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[king, cobra]);
    assert_eq!(rig.host.settings().min_players(), 2);
    rig.change(king, &[(number::MIN_PLAYERS, 1)]);
    rig.run(300 * MS);
    assert_eq!(rig.host.settings().min_players(), 2);
    assert!(
        rig.refusals(king)
            .iter()
            .any(|r| r.1.contains("min-players")),
        "{:?}",
        rig.refusals(king)
    );
    // A mode change keeps the operator's minimum.
    rig.change(king, &[(number::MODE, 1)]);
    rig.run(300 * MS);
    assert_eq!(rig.host.settings().mode(), Mode::Pvp);
    assert_eq!(rig.host.settings().min_players(), 2);
}
