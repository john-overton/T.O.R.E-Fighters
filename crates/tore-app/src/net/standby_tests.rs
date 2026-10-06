//! Slice K7a's acceptance (docs/ARCHITECTURE.md, "Host migration and
//! rejoin", row K7a): in real time on loopback, on the synthetic import, a
//! hosting game (the host thread and its own player over the link), a
//! joined game that may host (its standby warm, on the same machine) and a
//! bot's game that may not, each a whole [`NetSession`] pumped once a 16 ms
//! frame as the game pumps it.
//!
//! - The host thread vanishes in flight without a word: the joined game
//!   takes the game over on the socket it joined with, and the bot and the
//!   joined game's own player fly on within 5 seconds.
//! - The hosting game leaves: its thread hands the game over and ends, and
//!   the joined game hosts within a second or so.
//!
//! ```sh
//! cargo test --locked -p tore-app net::standby -- --nocapture
//! ```

use super::*;
use crate::net::hosting::{Command, End, HostSetup, HostThread, config};
use crate::net::options::HostOptions;
use crate::net::session::{Join, NetSession, Transport};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tore_formats::aircraft::AircraftId;
use tore_net::{LINK_ADDRESS, Listen};
use tore_session::client::migrate::{MigrationState, words};
use tore_session::{ClientEvent, ClientPhase, Controls, OpenPlanes};
use tore_world::mission::{Skill, Start};
use tore_world::test_support::resources::{THEATER, resources};

/// How often each game pumps its session: a 60 Hz frame.
const FRAME: Duration = Duration::from_millis(16);

/// Three of ours (the hosting player, the joined player and the bot) and an
/// AI wingman against two enemy AI 5 nautical miles ahead.
fn spec() -> MissionSpec {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 4;
    spec.wings[3].count = 2;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 5;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

/// A data folder of the test's own, removed when dropped.
struct Data(std::path::PathBuf);

impl Data {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "tore-k7a-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One game: its session, what it saw and when its snapshots came.
struct Game {
    name: &'static str,
    session: NetSession,
    events: Vec<(Instant, ClientEvent)>,
    snapshots: Vec<Instant>,
    seen: u64,
    /// The game is gone: nothing of it runs.
    gone: bool,
}

impl Game {
    fn new(name: &'static str, session: NetSession) -> Self {
        Self {
            name,
            session,
            events: Vec::new(),
            snapshots: Vec::new(),
            seen: 0,
            gone: false,
        }
    }

    fn pump(&mut self) {
        if self.gone {
            return;
        }
        self.session.pump(&Controls::default());
        // A game started from the command line: its King starts each
        // mission once every player holding a slot is ready.
        self.session.auto_start(true);
        let _ = self.session.frame();
        let now = Instant::now();
        for event in self.session.take_events() {
            self.events.push((now, event));
        }
        let seen = self.session.client.clone_stats().snapshots;
        if seen > self.seen {
            self.seen = seen;
            self.snapshots.push(now);
        }
    }

    fn flying(&self) -> bool {
        self.session.client.phase() == ClientPhase::Flying
    }

    /// The first snapshot after `at`.
    fn snapshot_after(&self, at: Instant) -> Option<Instant> {
        self.snapshots.iter().copied().find(|t| *t > at)
    }

    fn notices(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|(_, e)| match e {
                ClientEvent::Notice(text) => Some(text.clone()),
                _ => None,
            })
            .collect()
    }
}

/// The three games, joined and flying, the joined game standing by first
/// and ready.
struct Rig {
    games: Vec<Game>,
    _data: Data,
}

const HOST: usize = 0;
const VIPER: usize = 1;
const COBRA: usize = 2;

impl Rig {
    fn start() -> Self {
        let data = Data::new();
        let resources = Arc::new(resources());
        let options = HostOptions {
            mission: "duel.txt".into(),
            spec: spec(),
            port: 0,
            name: "Lead's game".into(),
            open_planes: OpenPlanes::Friendly,
            password: None,
            callsign: "Lead".into(),
            slot: Some(0),
            listing: None,
        };
        // The players hold their sticks still: the AI's idle rule (F2-A)
        // must not take their planes.
        let mut config = config(&options);
        config
            .settings
            .push((tore_session::settings::number::IDLE_AI, 0));
        let (thread, link) = HostThread::start(HostSetup {
            spec: spec(),
            resources: Arc::clone(&resources),
            config,
            listen: Listen::Address("127.0.0.1".parse().unwrap()),
            port: 0,
        })
        .expect("the host starts");
        let server = thread.addresses()[0];
        let join = Join {
            server: LINK_ADDRESS,
            transport: Transport::Link(link),
            callsign: "Lead".into(),
            slot: Some(0),
            password: String::new(),
            label: "hosted".into(),
            lobby: false,
            token: None,
        };
        let mut host = NetSession::start(join, Arc::clone(&resources), &data.0, None).unwrap();
        host.hosting = Some(thread);
        let joined = |callsign: &str, slot: u32| {
            let mut join = Join::to_from(server, callsign, "", "test", 0).unwrap();
            join.slot = Some(slot);
            NetSession::start(join, Arc::clone(&resources), &data.0, None).unwrap()
        };
        let viper = joined("Viper", 1);
        let mut cobra = joined("Cobra", 2);
        // The bot's game may not host: Viper is the only candidate.
        cobra.set_may_host(false);
        let mut rig = Self {
            games: vec![
                Game::new("Lead", host),
                Game::new("Viper", viper),
                Game::new("Cobra", cobra),
            ],
            _data: data,
        };
        assert!(
            rig.run_until(Duration::from_secs(60), |r| {
                r.games.iter().all(Game::flying)
                    && r.games[VIPER].session.standing_by()
                    && r.games[VIPER].session.client.standby_role()
                        == tore_session::wire::messages::StandbyMark::First
                    && r.games[COBRA]
                        .session
                        .client
                        .succession()
                        .is_some_and(|s| !s.standbys.is_empty())
            }),
            "every game flies and Viper stands by, ready: {}",
            rig.state()
        );
        // A few seconds of flight with the standby stepping along.
        rig.run(Duration::from_secs(2));
        rig
    }

    fn state(&self) -> String {
        self.games
            .iter()
            .map(|g| {
                format!(
                    "{}: {:?}, role {:?}, succession {:?}, standing by {}, hosting {}",
                    g.name,
                    g.session.client.phase(),
                    g.session.client.standby_role(),
                    g.session.client.succession().map(|s| s.standbys.len()),
                    g.session.standing_by(),
                    g.session.hosting(),
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn frame(&mut self) {
        for game in &mut self.games {
            game.pump();
        }
        std::thread::sleep(FRAME);
    }

    fn run(&mut self, time: Duration) {
        let end = Instant::now() + time;
        while Instant::now() < end {
            self.frame();
        }
    }

    fn run_until(&mut self, limit: Duration, mut done: impl FnMut(&Self) -> bool) -> bool {
        let end = Instant::now() + limit;
        while Instant::now() < end {
            self.frame();
            if done(self) {
                return true;
            }
        }
        false
    }

    /// Viper hosts and both remaining players fly with snapshots again
    /// after `lost`, within `within` of it: when each got its first.
    fn back_after(&mut self, lost: Instant, within: Duration) -> [Duration; 2] {
        // Snapshots already on their way when the host was lost do not
        // count.
        let from = lost + Duration::from_millis(200);
        let back = self.run_until(within.saturating_sub(lost.elapsed()), |r| {
            [VIPER, COBRA].iter().all(|&g| {
                r.games[g].flying()
                    && r.games[g].session.client.migration() == MigrationState::Steady
                    && r.games[g].snapshot_after(from).is_some()
            })
        });
        if !back {
            fn dump(dir: &std::path::Path) {
                for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        dump(&path);
                    } else if let Ok(text) = std::fs::read_to_string(&path) {
                        for line in text.lines() {
                            if line.contains("standby")
                                || line.contains("host")
                                || line.contains("migrate")
                            {
                                eprintln!("{}: {line}", path.display());
                            }
                        }
                    }
                }
            }
            dump(&self._data.0);
            for g in [VIPER, COBRA] {
                for (at, event) in &self.games[g].events {
                    if *at > lost {
                        eprintln!("{} +{:?}: {event:?}", self.games[g].name, *at - lost);
                    }
                }
            }
        }
        assert!(
            back,
            "both players fly on with snapshots within {within:?}: {}",
            self.state()
        );
        [VIPER, COBRA].map(|g| self.games[g].snapshot_after(from).unwrap() - lost)
    }
}

#[test]
fn a_host_that_vanishes_in_flight_is_taken_over_by_the_joined_game() {
    let mut rig = Rig::start();
    let thread = rig.games[HOST].session.hosting.as_ref().unwrap();
    assert!(thread.send(Command::Vanish));
    let lost = Instant::now();
    // The machine is gone: its game runs no more.
    rig.games[HOST].gone = true;
    let [viper, cobra] = rig.back_after(lost, Duration::from_secs(5));
    eprintln!("snapshots again after the loss: Viper {viper:?}, Cobra {cobra:?}");
    assert!(rig.games[VIPER].session.hosting(), "Viper hosts");
    let notices = rig.games[COBRA].notices();
    assert!(
        notices.contains(&words::lost("Viper")) && notices.contains(&words::moved("Viper")),
        "Cobra read the migration's lines: {notices:?}"
    );
    // The game flies on at the new host, the players' planes theirs.
    rig.run(Duration::from_secs(2));
    assert!(rig.games[VIPER].flying() && rig.games[COBRA].flying());
    for game in [VIPER, COBRA] {
        assert_eq!(
            rig.games[game].session.client.migration_counts().resumed,
            1,
            "{} resumed once",
            rig.games[game].name
        );
    }
}

#[test]
fn a_hosting_game_that_leaves_hands_the_game_over() {
    let mut rig = Rig::start();
    let thread = rig.games[HOST].session.hosting.as_ref().unwrap();
    assert!(thread.send(Command::Stop));
    let left = Instant::now();
    // The hosting player has left: its game no longer pumps, but its
    // thread runs on until the game is handed over.
    rig.games[HOST].gone = true;
    let hosts = rig.run_until(Duration::from_secs(3), |r| r.games[VIPER].session.hosting());
    assert!(hosts, "Viper takes the game over: {}", rig.state());
    let taken = left.elapsed();
    let [viper, cobra] = rig.back_after(left, Duration::from_secs(5));
    eprintln!(
        "handed over: Viper hosts after {taken:?}; snapshots again after Viper {viper:?}, Cobra {cobra:?}"
    );
    assert!(taken < Duration::from_millis(1_500), "took {taken:?}");
    let host = rig.games[HOST].session.hosting.as_mut().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while host.end().is_none() && Instant::now() < deadline {
        host.poll();
        std::thread::sleep(FRAME);
    }
    assert_eq!(host.end(), Some(&End::HandedOver));
}

#[test]
fn an_old_host_that_comes_back_steps_down_and_flies_on_as_a_player() {
    let mut rig = Rig::start();
    let thread = rig.games[HOST].session.hosting.as_ref().unwrap();
    // Lead's machine is cut off the network for 4 seconds; its game, and
    // its own player over the link, run on.
    assert!(thread.send(Command::Cut(Duration::from_secs(4))));
    let cut = Instant::now();
    let [viper, cobra] = rig.back_after(cut, Duration::from_secs(5));
    eprintln!("snapshots again after the cut: Viper {viper:?}, Cobra {cobra:?}");
    assert!(rig.games[VIPER].session.hosting(), "Viper hosts");
    // Back on the network, Lead's host asks standby 1, hears it hosts now,
    // steps down, and Lead's game resumes with Viper's like any other.
    let back = rig.run_until(Duration::from_secs(10), |r| {
        let lead = &r.games[HOST];
        !lead.session.hosting()
            && lead.session.client.server() != LINK_ADDRESS
            && lead.session.client.migration() == MigrationState::Steady
            && lead.session.client.migration_counts().resumed == 1
    });
    assert!(back, "Lead resumes with Viper: {}", rig.state());
    eprintln!("Lead resumed with Viper after {:?}", cut.elapsed());
    // Lead flies its own plane on, with Viper's snapshots.
    let resumed = Instant::now();
    rig.run(Duration::from_secs(1));
    assert!(rig.games[VIPER].session.hosting());
    assert!(rig.games[HOST].flying(), "Lead flies on: {}", rig.state());
    assert!(rig.games[HOST].snapshot_after(resumed).is_some());
}

#[test]
fn the_migration_lines_name_what_happened() {
    use tore_session::host::ResumeNote;
    assert_eq!(
        resume_line(&ResumeNote::Dropped {
            callsign: "Hawk".into()
        }),
        "Hawk never resumed: dropped, its plane kept for it"
    );
    assert_eq!(
        note_line(&Note::Handover { last_tick: 900 }),
        "the host hands over after tick 900"
    );
    assert_eq!(figures_line(&[]), "standbys: none");
    // The spec hash is the FNV-1a 64 a Flight record carries.
    assert_eq!(spec_hash(""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(spec_hash("a"), 0xaf63_dc4c_8601_ec8c);
}
