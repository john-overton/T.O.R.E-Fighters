//! Slice EF-K's acceptance: a game joined over loopback UDP whose loop
//! stalls is kept connected by its keepalive thread, for as long as the
//! bound allows, and only for that long; the hosting game's King, over the
//! in-process link, has no keepalive and is unaffected. In real time, on the
//! synthetic import (no retail data), with the host thread a hosted game
//! runs.
//!
//! Both games are pumped once a 16 ms frame, as the game pumps its session,
//! through [`KeptAlive`] exactly as [`NetSession::pump`] does.
//!
//! ```sh
//! cargo test --locked -p tore-app keepalive -- --nocapture
//! ```

use super::*;
use crate::net::hosting::{HostSetup, HostThread, Report, config};
use crate::net::options::HostOptions;
use std::thread;
use std::time::Instant;
use tore_formats::aircraft::AircraftId;
use tore_net::{CloseReason, DisconnectReason, LINK_ADDRESS, Listen, bind_udp};
use tore_session::bot::Bot;
use tore_session::{ClientPhase, HostLog, LeaveReason, OpenPlanes};
use tore_world::mission::{Skill, Start};
use tore_world::test_support::resources::{THEATER, resources};

/// How often each game pumps its session: a 60 Hz frame.
const FRAME: Duration = Duration::from_millis(16);

/// The hosting player, the joined player and two AI wingmen against two
/// enemy AI, 5 nautical miles apart, as the hosting tests fly.
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

fn import() -> Arc<BTreeMap<String, Vec<u8>>> {
    Arc::new(resources())
}

/// One game: a client with the scripted pilot, its transport and its
/// keepalive, pumped once a frame.
struct Side {
    bot: Bot,
    transport: Transport,
    clock: RealClock,
    kept: KeptAlive,
    events: Vec<ClientEvent>,
    seated: bool,
    closed: Option<CloseReason>,
}

impl Side {
    fn join(
        server: SocketAddr,
        transport: Transport,
        callsign: &str,
        kept: KeepaliveConfig,
    ) -> Self {
        let clock = RealClock::new();
        let config = ClientConfig {
            entropy: Entropy::System,
            ..ClientConfig::new(server, callsign, build_id())
        };
        let client = Client::connect(config, import(), clock.now()).expect("a client");
        Self {
            bot: Bot::new(client),
            transport,
            clock,
            kept: KeptAlive::new(kept),
            events: Vec::new(),
            seated: false,
            closed: None,
        }
    }

    /// One frame's turn, as `NetSession::pump` takes it.
    fn pump(&mut self) {
        let now = self.clock.now();
        let _ = self.bot.client.receive_from(now, &mut self.transport);
        self.bot.update(now);
        let _ = self.bot.client.transmit(&mut self.transport);
        self.kept.turned(&self.bot.client, &self.transport);
        while let Some(event) = self.bot.client.poll_event() {
            match &event {
                ClientEvent::Seated { .. } => self.seated = true,
                ClientEvent::Closed(reason) => self.closed = Some(reason.clone()),
                _ => {}
            }
            self.events.push(event);
        }
    }

    fn corrections(&self) -> usize {
        self.bot.client.corrections().len()
    }
}

/// A hosted game: the host thread, its King over the link and a guest over
/// loopback UDP.
struct Game {
    host: HostThread,
    king: Side,
    guest: Side,
    reports: Vec<(Instant, Report)>,
    /// The guest's game is stalled: it neither receives, steps nor sends.
    guest_stalled: bool,
}

impl Game {
    fn start(guest_keepalive: KeepaliveConfig) -> Self {
        let options = HostOptions {
            mission: "duel.txt".into(),
            spec: spec(),
            port: 0,
            name: "Host's game".into(),
            open_planes: OpenPlanes::Friendly,
            password: None,
            callsign: "Host".into(),
            slot: None,
        };
        let (host, link) = HostThread::start(HostSetup {
            spec: spec(),
            resources: import(),
            config: config(&options),
            listen: Listen::Address("127.0.0.1".parse().unwrap()),
            port: 0,
        })
        .expect("the host starts");
        let server = host.addresses()[0];
        let mut king = Side::join(
            LINK_ADDRESS,
            Transport::Link(link),
            "Host",
            KeepaliveConfig::default(),
        );
        // The King starts each mission once everyone holding a slot is
        // ready, as `tore-app --host` does.
        king.bot.start_when_ready = true;
        let socket = bind_udp("127.0.0.1:0".parse().unwrap()).expect("a socket");
        let guest = Side::join(server, Transport::Udp(socket), "Guest", guest_keepalive);
        Self {
            host,
            king,
            guest,
            reports: Vec::new(),
            guest_stalled: false,
        }
    }

    fn frame(&mut self) {
        let now = Instant::now();
        self.reports
            .extend(self.host.poll().into_iter().map(|report| (now, report)));
        self.king.pump();
        if !self.guest_stalled {
            self.guest.pump();
        }
        thread::sleep(FRAME);
    }

    fn run(&mut self, time: Duration) {
        let until = Instant::now() + time;
        while Instant::now() < until {
            self.frame();
        }
    }

    fn run_until(&mut self, limit: Duration, mut done: impl FnMut(&Self) -> bool) -> bool {
        let until = Instant::now() + limit;
        while Instant::now() < until {
            self.frame();
            if done(self) {
                return true;
            }
        }
        false
    }

    /// Both seated and flying a few seconds.
    fn fly_both(&mut self) {
        assert!(
            self.run_until(Duration::from_secs(20), |g| g.king.seated && g.guest.seated),
            "both seated: king {:?}, guest {:?}",
            self.king.events,
            self.guest.events
        );
        self.run(Duration::from_secs(3));
    }

    /// When the host logged the guest's departure, and why.
    fn guest_left(&self) -> Option<(Instant, LeaveReason)> {
        self.reports.iter().find_map(|(at, report)| match report {
            Report::Log(HostLog::Left {
                callsign, reason, ..
            }) if callsign == "Guest" => Some((*at, *reason)),
            _ => None,
        })
    }
}

/// Acceptance: the joined game stalls for 15 seconds. Its keepalive speaks
/// about once a second, the host keeps it, its plane flies on with the held
/// controls, and when it resumes it catches up and settles with no lasting
/// corrections. The hosting game's King, which has no keepalive, flies on
/// unaffected and still reigns.
#[test]
fn a_joined_game_stalled_for_fifteen_seconds_is_kept_and_recovers() {
    let mut game = Game::start(KeepaliveConfig::default());
    game.fly_both();
    assert!(game.guest.kept.running(), "the guest's keepalive runs");
    assert!(!game.king.kept.running(), "the King over the link has none");
    let king_before = game.king.corrections();
    let repeated_before = game.guest.bot.client.clone_stats().inputs_repeated;

    let stall = Instant::now();
    game.guest_stalled = true;
    game.run(Duration::from_secs(15));
    game.guest_stalled = false;
    let sent = game.guest.kept.sent();
    let king_during = game.king.corrections() - king_before;
    assert!(
        game.guest_left().is_none(),
        "dropped: {:?}",
        game.guest_left()
    );
    // One a second from the first quiet second on; a loaded machine may
    // delay a few, never by seconds.
    assert!((10..=16).contains(&sent), "{sent} keepalives in 15 s");

    game.run(Duration::from_secs(4));
    let after = game.guest.corrections();
    let stats = game.guest.bot.client.clone_stats();
    eprintln!(
        "guest after a 15 s stall, {:?} after it began: {sent} keepalives, {} corrections \
         since: {:?}\nstats: {stats:#?}\nKing: {king_during} corrections during the stall",
        stall.elapsed(),
        after,
        game.guest.bot.client.corrections()
    );
    assert!(game.guest.closed.is_none(), "{:?}", game.guest.closed);
    assert_eq!(game.guest.bot.client.phase(), ClientPhase::Flying);
    // The host flew the plane on with the held controls through the stall.
    // The client only learns that from the snapshots it read, and a socket
    // buffer holds a few seconds of them (the system drops the rest), so the
    // count it reports is well short of the stall's 1,800 ticks.
    let repeated = stats.inputs_repeated - repeated_before;
    assert!(repeated >= 120, "{repeated} ticks repeated");
    // It takes the host's newest state rather than step 15 seconds of
    // backlog (a catch-up, not a correction) ...
    assert!(stats.catch_ups >= 1, "{stats:#?}");
    // ... and then settles: two seconds with the prediction the host's at
    // every snapshot.
    let mut quiet_since = (Instant::now(), after);
    let settled = game.run_until(Duration::from_secs(12), |g| {
        let count = g.guest.corrections();
        if count != quiet_since.1 {
            quiet_since = (Instant::now(), count);
        }
        quiet_since.0.elapsed() >= Duration::from_secs(2)
    });
    assert!(
        settled,
        "still corrected: {:?}",
        &game.guest.bot.client.corrections()[after..]
    );
    assert!(game.guest_left().is_none(), "{:?}", game.guest_left());

    // The King flew on and still reigns; nobody was dropped.
    assert!(game.king.closed.is_none(), "{:?}", game.king.closed);
    assert!(!game.king.kept.running());
    let lobby = game.king.bot.client.lobby().expect("the lobby").clone();
    assert!(lobby.is_king());
    assert_eq!(lobby.players.len(), 2, "{lobby:?}");
    assert!(lobby.players.iter().all(|p| p.flying), "{lobby:?}");
    assert!(game.host.stop(crate::net::hosting::JOIN_LIMIT));
}

/// Acceptance: a joined game stalled for longer than the bound is dropped
/// as before, 5 seconds after its last keepalive, and learns it on resuming.
/// The bound is 3 seconds here (60 in the game) so the test is short.
#[test]
fn a_joined_game_stalled_past_the_bound_is_dropped() {
    let bound = Duration::from_secs(3);
    let mut game = Game::start(KeepaliveConfig {
        bound,
        ..KeepaliveConfig::default()
    });
    game.fly_both();
    let stall = Instant::now();
    game.guest_stalled = true;
    assert!(
        game.run_until(Duration::from_secs(14), |g| g.guest_left().is_some()),
        "never dropped"
    );
    let (at, reason) = game.guest_left().unwrap();
    let after = at - stall;
    let sent = game.guest.kept.sent();
    eprintln!("dropped {after:?} after the stall began, after {sent} keepalives");
    assert_eq!(reason, LeaveReason::Silent);
    // Kept past the plain 5 seconds by the keepalives within the bound,
    // dropped 5 seconds after the last.
    assert!((1..=3).contains(&sent), "{sent} keepalives");
    assert!(
        after >= Duration::from_millis(6500) && after <= bound + Duration::from_secs(7),
        "dropped {after:?} after the stall began"
    );
    game.run(Duration::from_secs(2));
    game.guest_stalled = false;
    assert!(game.run_until(Duration::from_secs(8), |g| g.guest.closed.is_some()));
    assert!(
        matches!(
            game.guest.closed,
            Some(CloseReason::Disconnected {
                reason: DisconnectReason::Timeout,
                ..
            })
        ),
        "{:?}",
        game.guest.closed
    );
    // The closed connection stops the thread.
    game.frame();
    assert!(!game.guest.kept.running());
    assert!(game.king.closed.is_none(), "{:?}", game.king.closed);
    assert!(game.host.stop(crate::net::hosting::JOIN_LIMIT));
}
