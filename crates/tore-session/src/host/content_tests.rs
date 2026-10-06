//! Slice L3's tests (docs/ARCHITECTURE.md, "Compatibility"): the players'
//! content, the gaps and the words, on the network simulator with synthetic
//! imports, and the pure parts (the gaps, the message's size, the lines) on
//! their own.
//!
//! The synthetic import holds one aircraft (the F/A-18D), one theater (UKR)
//! and two weapons, so a player "lacking an aircraft" lacks the only one
//! (and with it the shared item, which needs an aircraft to probe); a third
//! weapon (`AIM9X.JT`) is added where a test needs one the standard load
//! does not read.

use super::content::*;
use super::*;
use crate::client::content as words;
use crate::client::{Client, ClientConfig, ClientEvent, ClientPhase, Controls};
use crate::wire::messages::{Build, ContentGaps, Gap, GapPlayer, ItemKind, kind};
use tore_formats::aircraft::AircraftId;
use tore_import::source::{self, Importer, Source};
use tore_net::Entropy;
use tore_net::sim::{LinkConfig, SimNetwork, SimSocket};
use tore_world::mission::{Skill, Start};
use tore_world::test_support::resources::{THEATER, loadable_missile, resources};

const MS: Duration = Duration::from_millis(1);
/// The third weapon some tests add.
const EXTRA: &str = "AIM9X.JT";
const HORNET: &str = "F/A-18D Hornet";

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

/// Friendly Wing 1 of `friendly` and the enemy's Wing 1 of `enemy`,
/// airborne, 20 nm apart.
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

/// The synthetic import with the third weapon.
fn with_extra() -> Resources {
    let mut map = resources();
    map.insert(EXTRA.into(), loadable_missile(EXTRA));
    map
}

/// `map` without `name`.
fn without(mut map: Resources, name: &str) -> Resources {
    assert!(map.remove(name).is_some(), "{name} is in the import");
    map
}

/// `map` with `name`'s last byte changed (the file still reads).
fn changed(mut map: Resources, name: &str) -> Resources {
    let bytes = map.get_mut(name).expect("in the import");
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    map
}

/// The content of `map` with a known source.
fn content_of(map: &Resources, build: source::Build, version: &str) -> Arc<GameContent> {
    Arc::new(GameContent::with_source(
        map,
        Source {
            build: Some(build),
            importer: Some(Importer {
                version: version.into(),
                commit: "0123456789abcdef".into(),
            }),
        },
    ))
}

struct Player {
    socket: SimSocket,
    client: Client,
    events: Vec<ClientEvent>,
}

/// A host and its players, each with its own import.
struct Rig {
    net: SimNetwork,
    host: Host,
    socket: SimSocket,
    players: Vec<Player>,
    logs: Vec<HostLog>,
    content_logs: Vec<ContentLog>,
    next_port: u16,
}

impl Rig {
    /// A host on `import` flying `spec`, configured further by `configure`.
    fn new(spec: MissionSpec, import: Resources, configure: impl FnOnce(&mut HostConfig)) -> Self {
        let net = SimNetwork::new(9);
        net.set_default_link(LinkConfig::for_round_trip(20 * MS, 0., 0., 0.));
        let socket = net.bind(host_address()).unwrap();
        let mut config = HostConfig {
            entropy: Entropy::Seeded(17),
            ..HostConfig::new(build())
        };
        configure(&mut config);
        let host = Host::new(spec, Arc::new(import), config).unwrap();
        Self {
            net,
            host,
            socket,
            players: Vec::new(),
            logs: Vec::new(),
            content_logs: Vec::new(),
            next_port: 40_000,
        }
    }

    /// A game a player hosts; the first to join is the house and the King.
    fn hosted(spec: MissionSpec, import: Resources) -> Self {
        Self::new(spec, import, |config| {
            config.house = Some("10.0.0.2:40000".parse().unwrap());
            config.crown = CrownRule::FirstPlayer;
            config.start = StartMode::King;
            config.restart_delay = Duration::ZERO;
            config.empty_timeout = Duration::ZERO;
        })
    }

    /// A player on `import`, driving the lobby itself unless `configure`
    /// says otherwise.
    fn join(
        &mut self,
        callsign: &str,
        import: Resources,
        configure: impl FnOnce(&mut ClientConfig),
    ) -> usize {
        let address: SocketAddr = format!("10.0.0.2:{}", self.next_port).parse().unwrap();
        let socket = self.net.bind(address).unwrap();
        let mut config = ClientConfig {
            entropy: Entropy::Seeded(u64::from(self.next_port)),
            auto_ready: false,
            ..ClientConfig::new(host_address(), callsign, build())
        };
        configure(&mut config);
        self.next_port += 1;
        let client = Client::connect(config, Arc::new(import), self.net.now()).unwrap();
        self.players.push(Player {
            socket,
            client,
            events: Vec::new(),
        });
        self.players.len() - 1
    }

    fn step(&mut self) {
        self.net.advance(MS);
        let now = self.net.now();
        self.host.receive_from(now, &mut self.socket).unwrap();
        self.host.update(now);
        self.host.transmit(&mut self.socket).unwrap();
        while let Some(log) = self.host.poll_log() {
            self.logs.push(log);
        }
        while let Some(log) = self.host.poll_content_log() {
            self.content_logs.push(log);
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

    /// Every player in `players` is in the lobby, sees all of them, and has
    /// the gaps (the synthetic import's build is unknown, so the lobby's
    /// builds say nothing here); then the lobby settles.
    fn gather(&mut self, players: &[usize]) {
        let n = players.len();
        assert!(
            self.run_until(Duration::from_secs(3), |r| {
                players.iter().all(|&p| {
                    let client = &r.players[p].client;
                    client.phase() == ClientPhase::Lobby
                        && client.content_gaps().is_some()
                        && client.lobby().is_some_and(|l| l.players.len() == n)
                })
            }),
            "the players gather"
        );
        self.run(Duration::from_millis(500));
    }

    fn client(&mut self, player: usize) -> &mut Client {
        &mut self.players[player].client
    }

    fn gaps(&self, player: usize) -> Vec<Gap> {
        self.players[player]
            .client
            .content_gaps()
            .map(|g| g.gaps.clone())
            .unwrap_or_default()
    }

    fn refused(&self, player: usize, request: u8) -> Vec<String> {
        self.players[player]
            .events
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Refused { request: r, reason } if *r == request => {
                    Some(reason.clone())
                }
                _ => None,
            })
            .collect()
    }

    fn unable_of(&self, viewer: usize, callsign: &str) -> Option<String> {
        self.players[viewer]
            .client
            .lobby()?
            .players
            .iter()
            .find(|p| p.callsign == callsign)?
            .unable
            .clone()
    }

    fn content_lines(&self, kind: ContentLogKind) -> Vec<String> {
        self.content_logs
            .iter()
            .filter(|l| l.kind == kind)
            .map(|l| l.text.clone())
            .collect()
    }
}

/// A loadout of the F/A-18D with the third weapon on its missile station.
fn extra_loadout(import: &Resources) -> LoadoutSpec {
    let kind = tore_world::aircraft_type::AircraftType::load(import, AircraftId::F18).unwrap();
    let base = tore_sim::combat::loadout::Loadout::new(&kind.profile, |name| {
        import
            .get(name)
            .cloned()
            .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
    })
    .unwrap();
    let mut load = LoadoutSpec::of(&base);
    let missile = tore_formats::weapons::Weapon::parse(EXTRA, &import[EXTRA]).unwrap();
    let capacity = base.capacity(1, &missile) as u16;
    load.stations[1].weapon = EXTRA.into();
    load.stations[1].count = capacity;
    load.stations[1].quantity = capacity;
    load
}

// ----- On the simulator ------------------------------------------------------

#[test]
fn a_player_lacking_an_aircraft_is_a_gap_refused_unable_and_gone_when_it_leaves() {
    let mut rig = Rig::hosted(spec(2, 1), resources());
    let king = rig.join("Viper", resources(), |_| {});
    let hawk = rig.join("Hawk", without(resources(), "F18.PT"), |_| {});
    rig.gather(&[king, hawk]);
    let hawk_id = rig.players[hawk].client.lobby().unwrap().you;

    // Every player gets the gap: the aircraft, and the shared item (with no
    // aircraft Hawk's import cannot probe it).
    for player in [king, hawk] {
        let gaps = rig.gaps(player);
        let keys: Vec<(ItemKind, &str)> = gaps.iter().map(|g| (g.kind, g.key.as_str())).collect();
        assert_eq!(
            keys,
            [(ItemKind::Aircraft, "F18.PT"), (ItemKind::Shared, "")],
            "player {player}"
        );
        assert_eq!(gaps[0].label, HORNET);
        assert_eq!(
            gaps[0].players,
            [GapPlayer {
                id: hawk_id,
                differs: false
            }]
        );
    }
    // Hawk is unable, in the third person for everyone and in the second
    // person on its own screen.
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.unable_of(king, "Hawk").is_some()
    }));
    assert_eq!(
        rig.unable_of(king, "Hawk").as_deref(),
        Some("Hawk's game has no F/A-18D Hornet, which this mission flies.")
    );
    let own = rig.players[hawk].client.unable().unwrap().to_owned();
    assert!(
        own.starts_with("Your game has no F/A-18D Hornet, which this mission flies.")
            && own.ends_with("Re-import Fighters Anthology (Pref, Re-import) to add it."),
        "{own}"
    );
    // The King's mission with the aircraft is refused with the words; the
    // King's game words it the same from the gaps.
    let refusal = "Not everyone can fly the F/A-18D Hornet: Hawk's game has no F/A-18D Hornet.";
    assert_eq!(
        rig.players[king]
            .client
            .gap_refusal(ItemKind::Aircraft, "F18.PT")
            .as_deref(),
        Some(refusal)
    );
    rig.client(king).change_mission(&spec(3, 1));
    rig.run(Duration::from_millis(300));
    assert_eq!(rig.refused(king, kind::CHANGE_MISSION), [refusal]);
    assert_eq!(rig.host.spec().wings[0].count, 2, "the mission stays");
    // The log names the player's content and the gap.
    let players = rig.content_lines(ContentLogKind::Player);
    assert!(
        players.iter().any(|l| l
            == "content Hawk: an unknown Fighters Anthology build, imported by an unknown \
                T.O.R.E; lacks aircraft F18.PT, shared data"),
        "{players:?}"
    );
    assert!(
        players.iter().any(
            |l| l.starts_with("content Viper: ") && l.ends_with("; the same items as the host")
        ),
        "{players:?}"
    );
    assert_eq!(
        rig.content_lines(ContentLogKind::Gaps),
        ["gaps: aircraft F18.PT (Hawk lacks it); shared data (Hawk lacks it)"]
    );

    // Hawk leaves: the gap closes, everyone left is told, and the change
    // goes through.
    let now = rig.net.now();
    rig.client(hawk).leave_game(now);
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[king]
            .client
            .content_gaps()
            .is_some_and(|g| g.gaps.is_empty())
    }));
    assert_eq!(
        rig.content_lines(ContentLogKind::Gaps)
            .last()
            .map(String::as_str),
        Some("gaps: none")
    );
    rig.client(king).change_mission(&spec(3, 1));
    rig.run(Duration::from_millis(300));
    assert_eq!(rig.refused(king, kind::CHANGE_MISSION).len(), 1);
    assert_eq!(rig.host.spec().wings[0].count, 3);
}

#[test]
fn a_loadout_with_a_weapon_a_player_lacks_or_has_differently_is_refused() {
    for differs in [false, true] {
        let import = with_extra();
        let label = label_of(ItemKind::Weapon, EXTRA, &import).unwrap();
        let mut rig = Rig::hosted(spec(2, 1), import.clone());
        let king = rig.join("Viper", import.clone(), |_| {});
        let theirs = if differs {
            changed(import.clone(), EXTRA)
        } else {
            without(import.clone(), EXTRA)
        };
        let hawk = rig.join("Hawk", theirs, |_| {});
        rig.gather(&[king, hawk]);
        // The weapon is a gap; nothing the mission reads is, so Hawk can
        // fly it.
        let gaps = rig.gaps(king);
        assert_eq!(gaps.len(), 1, "{gaps:?}");
        assert_eq!(
            (gaps[0].kind, gaps[0].key.as_str()),
            (ItemKind::Weapon, EXTRA)
        );
        assert_eq!(gaps[0].players[0].differs, differs);
        rig.run(Duration::from_millis(500));
        assert!(rig.unable_of(king, "Hawk").is_none());
        rig.client(king).take_slot(0);
        rig.run(Duration::from_millis(300));
        rig.client(king).send_loadout(Some(extra_loadout(&import)));
        rig.run(Duration::from_millis(300));
        let expected = if differs {
            format!("Not everyone has the {label}: Hawk's {label} differs from the host's.")
        } else {
            format!("Not everyone has the {label}: Hawk's game has no {label}.")
        };
        assert_eq!(
            rig.refused(king, kind::LOADOUT),
            std::slice::from_ref(&expected)
        );
        assert_eq!(
            rig.players[king]
                .client
                .gap_refusal(ItemKind::Weapon, EXTRA)
                .as_deref(),
            Some(expected.as_str())
        );
        // The standard load is not refused.
        rig.client(king).send_loadout(None);
        rig.run(Duration::from_millis(300));
        assert_eq!(rig.refused(king, kind::LOADOUT).len(), 1);
    }
}

#[test]
fn a_player_whose_aircraft_differs_is_unable_with_the_item_named() {
    // The aircraft's sensor file differs: the aircraft still loads, with
    // another digest.
    let mut rig = Rig::hosted(spec(2, 1), resources());
    let king = rig.join("Viper", resources(), |_| {});
    let hawk = rig.join("Hawk", changed(resources(), "F18R.SEE"), |_| {});
    rig.gather(&[king, hawk]);
    let gaps = rig.gaps(king);
    assert_eq!(gaps.len(), 1, "{gaps:?}");
    assert_eq!(
        (gaps[0].kind, gaps[0].players[0].differs),
        (ItemKind::Aircraft, true)
    );
    assert!(rig.run_until(Duration::from_secs(2), |r| {
        r.unable_of(king, "Hawk").is_some()
    }));
    assert_eq!(
        rig.unable_of(king, "Hawk").as_deref(),
        Some("Hawk's F/A-18D Hornet differs from the host's.")
    );
    assert_eq!(
        rig.players[hawk].client.unable(),
        Some("Your F/A-18D Hornet's data differs from the host's.")
    );
    rig.client(king).change_mission(&spec(3, 1));
    rig.run(Duration::from_millis(300));
    assert_eq!(
        rig.refused(king, kind::CHANGE_MISSION),
        [
            "Not everyone can fly the F/A-18D Hornet: Hawk's F/A-18D Hornet differs from the \
             host's."
        ]
    );
}

#[test]
fn a_servers_own_mission_is_never_refused_and_a_player_who_cannot_fly_it_is_unable() {
    // A dedicated server whose mission flies the aircraft Hawk lacks: the
    // server starts, Hawk joins and is unable, Viper flies.
    let mut rig = Rig::new(spec(2, 1), resources(), |_| {});
    let hawk = rig.join("Hawk", without(resources(), "F18.PT"), |_| {});
    let viper = rig.join("Viper", resources(), |config| config.auto_ready = true);
    assert!(rig.run_until(Duration::from_secs(5), |r| {
        r.players[viper].client.seat().is_some() && r.unable_of(viper, "Hawk").is_some()
    }));
    assert_eq!(
        rig.unable_of(viper, "Hawk").as_deref(),
        Some("Hawk's game has no F/A-18D Hornet, which this mission flies.")
    );
    assert!(rig.players[hawk].client.seat().is_none());
    assert_eq!(rig.players[hawk].client.phase(), ClientPhase::Lobby);
    assert_eq!(rig.host.gaps().len(), 2);
}

#[test]
fn players_of_two_builds_with_the_same_items_read_the_same_line_and_fly() {
    let import = resources();
    let mut rig = Rig::new(spec(2, 1), import.clone(), |config| {
        config.content = Some(content_of(&import, source::Build::V102F, "0.1.4"));
    });
    let old = rig.join("Hawk", import.clone(), |config| {
        config.auto_ready = true;
        config.content = Some(content_of(&import, source::Build::Disc10, "0.1.4"));
    });
    let new = rig.join("Viper", import.clone(), |config| {
        config.auto_ready = true;
        config.content = Some(content_of(&import, source::Build::V102F, "0.1.4"));
    });
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        [old, new].iter().all(|&p| {
            let client = &r.players[p].client;
            client.content_gaps().is_some()
                && client.lobby().is_some_and(|l| {
                    l.players.len() == 2 && l.players.iter().all(|p| p.build != Build::Unknown)
                })
        })
    }));
    assert!(rig.gaps(old).is_empty());
    let gaps = rig.players[old].client.content_gaps().unwrap().clone();
    assert_eq!(gaps.host_build, Build::V102F);
    let lobby = rig.players[old].client.lobby().unwrap().clone();
    let me = lobby.me().unwrap();
    assert_eq!(me.build, Build::V10);
    assert_eq!(
        words::joined_line(me, true, &gaps).as_deref(),
        Some(
            "You imported Fighters Anthology 1.0; the host, 1.02F. Every aircraft, weapon and \
             theater is the same."
        )
    );
    let viper_view = rig.players[new].client.lobby().unwrap().clone();
    let hawk = viper_view
        .players
        .iter()
        .find(|p| p.callsign == "Hawk")
        .unwrap();
    assert_eq!(
        words::joined_line(hawk, false, &gaps).as_deref(),
        Some(
            "Hawk imported Fighters Anthology 1.0; the host, 1.02F. Every aircraft, weapon and \
             theater is the same."
        )
    );
    assert_eq!(
        words::joined_line(viper_view.me().unwrap(), true, &gaps),
        None
    );
    assert!(words::hint_line(hawk).starts_with("Hawk: Fighters Anthology 1.0, on "));
    // Both fly.
    assert!(rig.run_until(Duration::from_secs(5), |r| {
        r.players[old].client.seat().is_some() && r.players[new].client.seat().is_some()
    }));
    assert!(
        rig.content_lines(ContentLogKind::Player)
            .iter()
            .any(|l| l.starts_with(
                "content Hawk: Fighters Anthology 1.0, imported by T.O.R.E 0.1.4 (01234567)"
            ))
    );
    assert!(rig.content_lines(ContentLogKind::Gaps).is_empty());
    assert!(
        rig.logs
            .iter()
            .all(|l| !matches!(l, HostLog::ContentRefused { .. }))
    );
}

// ----- The pure parts --------------------------------------------------------

#[test]
fn the_content_message_codes_the_items_for_the_wire() {
    let map = with_extra();
    let content = content_of(&map, source::Build::Disc10, "0.1.4");
    let message = content.message().unwrap();
    assert_eq!(message.build, Build::V10);
    assert_eq!(message.importer.as_ref().unwrap().version, "0.1.4");
    let keys: Vec<(ItemKind, &str)> = message
        .items
        .iter()
        .map(|i| (i.kind, i.key.as_str()))
        .collect();
    assert_eq!(
        keys,
        [
            (ItemKind::Aircraft, "F18.PT"),
            (ItemKind::Theater, "UKR"),
            (ItemKind::Weapon, "AIM9M.JT"),
            (ItemKind::Weapon, EXTRA),
            (ItemKind::Weapon, "M61.JT"),
            (ItemKind::Shared, ""),
        ]
    );
    // It is a message the wire takes.
    crate::wire::messages::Message::Content(Box::new(message))
        .encode()
        .unwrap();
    // A long importer is cut to the wire's 64 bytes.
    let long = GameContent {
        source: Source {
            build: None,
            importer: Some(Importer {
                version: "9".repeat(100),
                commit: "c".repeat(100),
            }),
        },
        ..(*content).clone()
    };
    assert_eq!(long.importer().unwrap().version.len(), 64);
    // An import with no item at all sends nothing.
    assert!(GameContent::of(&Resources::new()).message().is_none());
    // The words' labels.
    let labels = content.labels(&map);
    assert_eq!(labels.get(ItemKind::Aircraft, "F18.PT"), Some(HORNET));
    assert_eq!(labels.get(ItemKind::Shared, ""), Some(SHARED_LABEL));
    assert!(labels.get(ItemKind::Theater, "UKR").is_some());
    assert_eq!(labels.or_key(ItemKind::Aircraft, "SU27.PT"), "SU27.PT");
}

#[test]
fn shared_computes_once_for_resources_that_live() {
    let map = Arc::new(resources());
    let a = GameContent::shared(&map);
    let b = GameContent::shared(&map);
    assert!(Arc::ptr_eq(&a, &b));
    let other = Arc::new(resources());
    let c = GameContent::shared(&other);
    assert!(!Arc::ptr_eq(&a, &c));
    assert_eq!(*a, *c);
}

/// A gap as kind, key, whether the host lacks it, and (id, differs) of
/// each player.
type GapSummary<'a> = (ItemKind, &'a str, bool, Vec<(u8, bool)>);

fn player(items: &[(ItemKind, &str, u64)]) -> PlayerContent {
    PlayerContent {
        build: Build::V102F,
        importer: None,
        items: items
            .iter()
            .map(|(kind, key, digest)| ((*kind, key.to_string()), *digest))
            .collect(),
    }
}

#[test]
fn gaps_name_who_lacks_or_differs_and_what_the_host_lacks() {
    let host: BTreeMap<ItemKey, u64> = [
        ((ItemKind::Aircraft, "F18.PT".to_string()), 1),
        ((ItemKind::Weapon, "AIM9X.JT".to_string()), 2),
        ((ItemKind::Shared, String::new()), 3),
    ]
    .into_iter()
    .collect();
    let hawk = player(&[
        (ItemKind::Aircraft, "F18.PT", 1),
        (ItemKind::Weapon, "AIM9X.JT", 9),
        (ItemKind::Aircraft, "SU27.PT", 5),
        (ItemKind::Shared, "", 3),
    ]);
    let viper = player(&[(ItemKind::Aircraft, "F18.PT", 1), (ItemKind::Shared, "", 3)]);
    let counted = [
        Counted {
            id: 2,
            callsign: "Hawk",
            content: &hawk,
        },
        Counted {
            id: 5,
            callsign: "Viper",
            content: &viper,
        },
    ];
    let found = gaps(&host, &Labels::default(), &counted);
    let summary: Vec<GapSummary<'_>> = found
        .iter()
        .map(|g| {
            (
                g.kind,
                g.key.as_str(),
                g.host_lacks,
                g.players.iter().map(|p| (p.id, p.differs)).collect(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            // Only Hawk has it: the host lacks it, and Viper too.
            (ItemKind::Aircraft, "SU27.PT", true, vec![(5, false)]),
            (
                ItemKind::Weapon,
                "AIM9X.JT",
                false,
                vec![(2, true), (5, false)]
            ),
        ]
    );
    let name = |id: u8| {
        if id == 2 {
            "Hawk".to_string()
        } else {
            "Viper".to_string()
        }
    };
    assert_eq!(
        not_everyone(&found[1], "AIM-9X", &name, None),
        "Not everyone has the AIM-9X: Viper's game has no AIM-9X; Hawk's AIM-9X differs from \
         the host's."
    );
    assert_eq!(
        not_everyone(&found[0], "SU27.PT", &name, None),
        "Not everyone can fly the SU27.PT: the server has no SU27.PT; Viper's game has no \
         SU27.PT."
    );
    assert_eq!(
        whose(&found[0], "SU27.PT", &name, Some("Ace")),
        "Ace's game has no SU27.PT; Viper's game has no SU27.PT"
    );
    assert_eq!(
        gaps_line(&found, &name, 0),
        "gaps: aircraft SU27.PT (the host lacks it; Viper lacks it); weapon AIM9X.JT (Viper \
         lacks it; Hawk differs)"
    );
    let theater = Gap {
        kind: ItemKind::Theater,
        key: "TVIET".into(),
        label: "Vietnam".into(),
        host_lacks: false,
        players: (1..=5).map(|id| GapPlayer { id, differs: false }).collect(),
    };
    let callsigns = |id: u8| ["", "Ace", "Bat", "Cat", "Dog", "Eel"][usize::from(id)].to_string();
    assert_eq!(
        not_everyone(&theater, "Vietnam", &callsigns, None),
        "Not everyone has Vietnam: the games of Ace, Bat, Cat and 2 more have no Vietnam."
    );
}

#[test]
fn the_unable_words_name_the_item_or_the_files() {
    let host: BTreeMap<ItemKey, u64> = [
        ((ItemKind::Aircraft, "F18.PT".to_string()), 1),
        ((ItemKind::Theater, THEATER.to_string()), 2),
        ((ItemKind::Shared, String::new()), 3),
    ]
    .into_iter()
    .collect();
    let labels = GameContent::of(&resources()).labels(&resources());
    let mission = spec(2, 1);
    let lacks = player(&[(ItemKind::Theater, THEATER, 2)]);
    assert_eq!(
        unable_words("Hawk", Some(&lacks), &host, &labels, &mission, &[]),
        "Hawk's game has no F/A-18D Hornet, which this mission flies."
    );
    let differs = player(&[
        (ItemKind::Aircraft, "F18.PT", 1),
        (ItemKind::Theater, THEATER, 7),
    ]);
    let theater = labels.or_key(ItemKind::Theater, THEATER);
    assert_eq!(
        unable_words("Hawk", Some(&differs), &host, &labels, &mission, &[]),
        format!("Hawk's {theater} differs from the host's.")
    );
    let same = player(&[
        (ItemKind::Aircraft, "F18.PT", 1),
        (ItemKind::Theater, THEATER, 2),
    ]);
    let names = vec!["CRATER.SH".to_string(), "A.SH".into(), "B.SH".into()];
    assert_eq!(
        unable_words("Hawk", Some(&same), &host, &labels, &mission, &names),
        "Hawk's game data differs from the host's in 3 files, such as CRATER.SH."
    );
    assert_eq!(
        unable_words("Hawk", None, &host, &labels, &mission, &names[..1]),
        "Hawk's game data differs from the host's in 1 file, CRATER.SH."
    );
    assert_eq!(
        unable_words("Hawk", Some(&same), &host, &labels, &mission, &[]),
        "Hawk's game cannot build this mission."
    );
}

#[test]
fn the_players_own_words_name_the_item_its_import_and_what_to_do() {
    let mission = spec(2, 1);
    // Lacking the aircraft, from an import an earlier T.O.R.E made.
    let lacking = without(resources(), "F18.PT");
    let own = content_of(&lacking, source::Build::V102F, "0.1.2");
    assert_eq!(
        words::refusal(&own, &lacking, &mission, &["F18.PT".into()], "0.1.3"),
        "Your game has no F/A-18D Hornet, which this mission flies. Your import was made by an \
         earlier T.O.R.E (0.1.2). Re-import Fighters Anthology (Pref, Re-import) to add it."
    );
    let current = content_of(&lacking, source::Build::V102F, "0.1.3");
    assert_eq!(
        words::refusal(&current, &lacking, &mission, &["F18.PT".into()], "0.1.3"),
        "Your game has no F/A-18D Hornet, which this mission flies. Re-import Fighters \
         Anthology (Pref, Re-import) to add it."
    );
    // A file of the aircraft differs.
    let map = changed(resources(), "F18R.SEE");
    let own = content_of(&map, source::Build::V102F, "0.1.3");
    assert_eq!(
        words::refusal(&own, &map, &mission, &["F18R.SEE".into()], "0.1.3"),
        "Your F/A-18D Hornet's data differs from the host's."
    );
    // A file no mission item reads: the shared data.
    let names = vec![
        "TORE_RADIO_^BANDIT".to_string(),
        "TORE_RADIO_^BREAK1".into(),
    ];
    assert_eq!(
        words::refusal(&own, &map, &mission, &names, "0.1.3"),
        "Your game's shared flight data differs from the host's (2 files, such as \
         TORE_RADIO_^BANDIT). Re-import Fighters Anthology with this version of T.O.R.E."
    );
    // Earlier or not.
    let importer = |version: &str| crate::wire::messages::Importer {
        version: version.into(),
        commit: "c".into(),
    };
    assert!(words::made_earlier(None, "0.1.3"));
    assert!(words::made_earlier(
        Some(&importer("0.1.2")),
        "0.1.3-4-gabc"
    ));
    assert!(!words::made_earlier(
        Some(&importer("0.1.3-2-gdef")),
        "0.1.3"
    ));
    assert!(!words::made_earlier(Some(&importer("0.2.0")), "0.1.3"));
    assert!(!words::made_earlier(Some(&importer("dev")), "0.1.3"));
}

#[test]
fn the_joined_line_names_the_difference_or_the_unknown_build() {
    let player = |build: Build| crate::wire::messages::LobbyPlayer {
        id: 3,
        callsign: "Hawk".into(),
        slot: None,
        ready: false,
        loadout: false,
        flying: false,
        observing: false,
        away: false,
        unable: None,
        platform: crate::wire::Platform::Linux,
        path: crate::wire::Path::LocalNetwork,
        build,
        standby: crate::wire::messages::StandbyMark::None,
    };
    let gaps = |list: Vec<Gap>| ContentGaps {
        host_build: Build::V102F,
        host_importer: None,
        gaps: list,
    };
    let gap = |kind, key: &str, label: &str, differs| Gap {
        kind,
        key: key.into(),
        label: label.into(),
        host_lacks: false,
        players: vec![GapPlayer { id: 3, differs }],
    };
    let both = gaps(vec![
        gap(ItemKind::Aircraft, "SU27.PT", "Su-27", false),
        gap(ItemKind::Weapon, "AIM9X.JT", "AIM-9X", true),
    ]);
    assert_eq!(
        words::joined_line(&player(Build::V10), false, &both).as_deref(),
        Some("Hawk's game differs from the host's: no Su-27 and a different AIM-9X.")
    );
    assert_eq!(
        words::joined_line(&player(Build::V10), true, &both).as_deref(),
        Some("Your game differs from the host's: no Su-27 and a different AIM-9X.")
    );
    assert_eq!(
        words::joined_line(&player(Build::Unknown), false, &gaps(vec![])).as_deref(),
        Some("Hawk's import does not say which Fighters Anthology build it came from.")
    );
    assert_eq!(
        words::joined_line(&player(Build::V102F), false, &gaps(vec![])),
        None
    );
    assert_eq!(
        words::hint_line(&player(Build::V10)),
        "Hawk: Fighters Anthology 1.0, on Linux."
    );
}

#[test]
fn the_gaps_message_keeps_inside_one_message() {
    let content = content_of(&resources(), source::Build::V102F, "0.1.4");
    // 1,024 gaps with the longest key and label, each naming 30 players
    // (the most a host seats): far over 64 KB. 342 weapons, 341 aircraft
    // and 341 theaters.
    let players: Vec<GapPlayer> = (0..30).map(|id| GapPlayer { id, differs: true }).collect();
    let kinds = [ItemKind::Weapon, ItemKind::Aircraft, ItemKind::Theater];
    let mut all: Vec<Gap> = (0..1_024)
        .map(|n| Gap {
            kind: kinds[n % 3],
            key: format!("{n:0>32}"),
            label: "L".repeat(64),
            host_lacks: false,
            players: players.clone(),
        })
        .collect();
    all.sort_by(|a, b| (a.kind, &a.key).cmp(&(b.kind, &b.key)));
    let (kept, left_out) = message(&content, &all);
    let body = crate::wire::messages::Message::ContentGaps(Box::new(kept.clone()))
        .encode()
        .unwrap();
    assert!(body.len() <= MESSAGE_LIMIT, "{}", body.len());
    assert!(left_out > 0);
    assert_eq!(kept.gaps.len() + left_out, all.len());
    // Every aircraft is kept before any theater, and theaters before
    // weapons; the message is in Content's order.
    let count = |kind: ItemKind| kept.gaps.iter().filter(|g| g.kind == kind).count();
    assert_eq!(count(ItemKind::Aircraft), 341);
    assert!(count(ItemKind::Theater) > 0);
    assert_eq!(count(ItemKind::Weapon), 0);
    assert!(
        kept.gaps
            .windows(2)
            .all(|w| (w[0].kind, &w[0].key) < (w[1].kind, &w[1].key))
    );
    // A short list goes whole.
    let few: Vec<Gap> = ["A.JT", "B.JT", "C.JT"]
        .iter()
        .map(|key| Gap {
            kind: ItemKind::Weapon,
            key: (*key).into(),
            label: String::new(),
            host_lacks: true,
            players: Vec::new(),
        })
        .collect();
    let (whole, none) = message(&content, &few);
    assert_eq!((whole.gaps.len(), none), (3, 0));
    assert!(gaps_line(&all, &|id| format!("P{id}"), left_out).contains("left out"));
}

#[test]
fn lists_files_and_the_log_queue() {
    let names = |n: usize| -> Vec<String> { (0..n).map(|i| format!("P{i}")).collect() };
    assert_eq!(list(&names(1)), "P0");
    assert_eq!(list(&names(2)), "P0 and P1");
    assert_eq!(list(&names(3)), "P0, P1 and P2");
    assert_eq!(list(&names(5)), "P0, P1, P2 and 2 more");
    assert_eq!(files_phrase(&names(1)), "1 file, P0");
    let mut logs = ContentLogs::default();
    for tick in 0..300 {
        logs.push(ContentLog {
            tick,
            kind: ContentLogKind::Gaps,
            text: String::new(),
        });
    }
    assert_eq!(logs.pop().map(|l| l.tick), Some(44));
    assert_eq!(host_name(None), "the server");
    assert_eq!(wire_key(ItemKind::Shared, "shared"), "");
    assert_eq!(world_key(ItemKind::Shared, ""), "shared");
}
