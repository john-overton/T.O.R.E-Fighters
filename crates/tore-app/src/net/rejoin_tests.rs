//! A player who ends its own flight (the others fly on) and presses Join
//! again is seated again, any number of times, and the game has a built
//! mission for each seating (slice EF-F). The game's own `NetSession` joins
//! a hosted game over loopback UDP in lobby mode, as the lobby screen does,
//! on the synthetic import (no retail data; the windowed run in
//! `.local/mp-notes/stage-ef/eff/` flies it with the real one).
//!
//! ```sh
//! cargo test --locked -p tore-app rejoin -- --nocapture
//! ```

use super::*;
use crate::net::hosting::{HostSetup, HostThread, config};
use crate::net::options::HostOptions;
use std::thread;
use std::time::Instant;
use tore_net::{LINK_ADDRESS, Listen, bind_udp};
use tore_session::bot::Bot;
use tore_session::{ClientPhase, OpenPlanes};
use tore_world::mission::{Skill, Start};
use tore_world::test_support::resources::{THEATER, resources};

const FRAME: Duration = Duration::from_millis(16);

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

/// The hosting game's King (over the link) and a joined `NetSession` in
/// lobby mode, both pumped once a frame.
struct Game {
    host: HostThread,
    king: Bot,
    king_link: LinkEnd,
    king_clock: RealClock,
    joiner: NetSession,
    /// The joiner's session events, kept.
    seen: Vec<ClientEvent>,
}

impl Game {
    fn start() -> Self {
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
        let king_clock = RealClock::new();
        let king_config = ClientConfig {
            entropy: Entropy::System,
            ..ClientConfig::new(LINK_ADDRESS, "Host", build_id())
        };
        let client = Client::connect(king_config, import(), king_clock.now()).expect("a client");
        let mut king = Bot::new(client);
        king.start_when_ready = true;
        let data = std::env::temp_dir().join(format!("tore-rejoin-{}", std::process::id()));
        let joiner = NetSession::start(
            Join {
                server,
                transport: Transport::Udp(
                    bind_udp("127.0.0.1:0".parse().unwrap()).expect("socket"),
                ),
                callsign: "Joiner".into(),
                slot: None,
                password: String::new(),
                label: "rejoin test".into(),
                lobby: true,
            },
            import(),
            &data,
            None,
        )
        .expect("a session");
        Self {
            host,
            king,
            king_link: link,
            king_clock,
            joiner,
            seen: Vec::new(),
        }
    }

    fn frame(&mut self) {
        let _ = self.host.poll();
        let now = self.king_clock.now();
        let _ = self.king.client.receive_from(now, &mut self.king_link);
        self.king.update(now);
        let _ = self.king.client.transmit(&mut self.king_link);
        while self.king.client.poll_event().is_some() {}
        self.joiner.pump(&Controls::neutral(Default::default()));
        self.seen.extend(self.joiner.take_events());
        thread::sleep(FRAME);
    }

    fn run_until(&mut self, limit: Duration, mut done: impl FnMut(&mut Self) -> bool) -> bool {
        let until = Instant::now() + limit;
        while Instant::now() < until {
            self.frame();
            if done(self) {
                return true;
            }
        }
        false
    }
}

/// The game takes the first seating's build, then builds the mission again
/// for each later seating in the same running mission: twice over.
#[test]
fn a_player_who_ends_its_own_flight_is_seated_and_built_for_again_twice() {
    let mut game = Game::start();
    // The King flies alone while the joiner, in the lobby, holds no slot.
    assert!(
        game.run_until(Duration::from_secs(20), |g| {
            g.joiner
                .client
                .lobby()
                .is_some_and(|l| l.phase == tore_session::wire::messages::LobbyPhase::Flying)
        }),
        "the King's flight is under way: {:?}",
        game.joiner.client.lobby()
    );
    // The mission's first build is the one the host's load gave.
    // (The synthetic import has no drawn aircraft models, so the game's own
    // build fails with "unreviewed ... device layout"; what the test checks
    // is that a build is attempted, and kept for, each seating.)
    assert!(
        game.joiner.take_built().is_some(),
        "the load's build: {:?}",
        game.seen
    );
    for round in 1..=3 {
        // Join, as the lobby does while the others fly: a slot, then Ready.
        if game
            .joiner
            .client
            .lobby()
            .and_then(|l| l.me())
            .and_then(|m| m.slot)
            .is_none()
        {
            game.joiner.client.take_any_slot();
            assert!(
                game.run_until(Duration::from_secs(5), |g| {
                    g.joiner
                        .client
                        .lobby()
                        .and_then(|l| l.me())
                        .and_then(|m| m.slot)
                        .is_some()
                }),
                "round {round}: a slot"
            );
        }
        game.joiner.client.set_ready(true);
        assert!(
            game.run_until(Duration::from_secs(15), |g| g
                .joiner
                .client
                .seat()
                .is_some()),
            "round {round}: seated: {:?}",
            game.joiner.client.lobby()
        );
        // What the game does at a seating with no build left from a load.
        assert!(game.joiner.take_built().is_none(), "round {round}");
        match game.joiner.rebuild() {
            Some(Ok(built)) => assert!(!built.models.is_empty(), "round {round}"),
            Some(Err(_)) => {}
            None => panic!("round {round}: no mission to build again"),
        }
        game.run_until(Duration::from_secs(2), |_| false);
        // End its own flight: back in the lobby, still connected.
        game.joiner.leave();
        assert!(
            game.run_until(Duration::from_secs(10), |g| {
                g.joiner.client.seat().is_none() && g.joiner.client.phase() == ClientPhase::Lobby
            }),
            "round {round}: back in the lobby: phase {:?}",
            game.joiner.client.phase()
        );
    }
    drop(game.host);
}
