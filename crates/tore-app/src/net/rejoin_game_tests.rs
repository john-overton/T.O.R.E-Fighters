//! The game keeps its rejoin token and sends it again (stage K, slice K7b):
//! a joined `NetSession` takes the token the host grants into the data
//! folder's `rejoin-v1.conf`, and a later session to the same host comes
//! back as the same player, whether the token is found by the host's address
//! (it goes in the Challenge answer) or only by the session's id (it goes in
//! a Rejoin once the host has accepted the join). Loopback, a hosted game on
//! the synthetic import, no window.
//!
//! ```sh
//! cargo test --locked -p tore-app rejoin_game -- --nocapture
//! ```

use super::*;
use crate::net::hosting::{HostSetup, HostThread, config};
use crate::net::options::HostOptions;
use crate::net::rejoin_store::Store;
use std::thread;
use std::time::Instant;
use tore_net::{Listen, bind_udp};
use tore_session::OpenPlanes;
use tore_session::bot::Bot;
use tore_session::client::rejoin::{KeptToken, unix_now};
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

/// A scratch data folder, removed when it goes.
struct Data(PathBuf);

impl Data {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("tore-rejoin-game-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A hosted game: its King (a bot over the link) and the host's address.
struct Hosted {
    host: HostThread,
    king: Bot,
    link: LinkEnd,
    clock: RealClock,
    server: SocketAddr,
}

impl Hosted {
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
            listing: None,
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
        let clock = RealClock::new();
        let king_config = ClientConfig {
            entropy: Entropy::System,
            ..ClientConfig::new(LINK_ADDRESS, "Host", build_id())
        };
        let client = Client::connect(king_config, import(), clock.now()).expect("a client");
        Self {
            host,
            king: Bot::new(client),
            link,
            clock,
            server,
        }
    }

    /// A joiner's session on loopback in lobby mode, keeping its tokens in
    /// `data`.
    fn join(&self, data: &Data, callsign: &str) -> NetSession {
        NetSession::start(
            Join {
                server: self.server,
                transport: Transport::Udp(
                    bind_udp("127.0.0.1:0".parse().unwrap()).expect("socket"),
                ),
                callsign: callsign.into(),
                slot: None,
                password: String::new(),
                label: "rejoin game test".into(),
                lobby: true,
                token: None,
            },
            import(),
            &data.0,
            None,
        )
        .expect("a session")
    }

    /// One frame: the host, its King and each session.
    fn frame(&mut self, sessions: &mut [&mut NetSession]) -> Vec<ClientEvent> {
        let _ = self.host.poll();
        let now = self.clock.now();
        let _ = self.king.client.receive_from(now, &mut self.link);
        self.king.update(now);
        let _ = self.king.client.transmit(&mut self.link);
        while self.king.client.poll_event().is_some() {}
        let mut events = Vec::new();
        for session in sessions {
            session.pump(&Controls::neutral(Default::default()));
            events.extend(session.take_events());
        }
        thread::sleep(FRAME);
        events
    }

    fn run_until(
        &mut self,
        sessions: &mut [&mut NetSession],
        limit: Duration,
        mut done: impl FnMut(&[&mut NetSession], &[ClientEvent]) -> bool,
    ) -> bool {
        let until = Instant::now() + limit;
        let mut seen = Vec::new();
        while Instant::now() < until {
            seen.extend(self.frame(sessions));
            if done(sessions, &seen) {
                return true;
            }
        }
        false
    }
}

fn welcomed(events: &[ClientEvent]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, ClientEvent::Notice(text) if text.starts_with("Welcome back, ")))
}

/// The first join leaves its token in the data folder, kept by the host's
/// address; a second session of the same data folder to the same address
/// sends it in the Challenge answer and is welcomed back.
#[test]
fn a_token_kept_by_address_brings_the_player_back() {
    let data = Data::new("address");
    let mut hosted = Hosted::start();
    let mut first = hosted.join(&data, "Viper");
    assert!(
        hosted.run_until(&mut [&mut first], Duration::from_secs(10), |s, _| s[0]
            .client
            .token()
            .is_some()),
        "the host grants a token"
    );
    let kept = Store::load(&data.0, unix_now());
    let entry = kept.entries.first().expect("the token is in the file");
    assert_eq!(entry.address, hosted.server.to_string());
    assert_eq!(entry.kept.callsign, "Viper");
    assert_eq!(
        Some(&entry.kept),
        first.client.token(),
        "the file holds what the client holds"
    );
    // The game stops: the host sees it drop, the token stays good.
    drop(first);
    hosted.run_until(&mut [], Duration::from_secs(1), |_, _| false);

    let mut second = hosted.join(&data, "Viper");
    assert!(
        hosted.run_until(&mut [&mut second], Duration::from_secs(10), |_, seen| {
            welcomed(seen)
        }),
        "the player is welcomed back"
    );
    assert_eq!(
        second.client.token().map(|t| t.token),
        Some(entry.kept.token),
        "it is the same token: the same player"
    );
}

/// A token the game holds for the host's session but not for the address it
/// joined at is sent in a Rejoin once the host has accepted the join.
#[test]
fn a_token_found_only_by_session_is_sent_after_the_join() {
    let data = Data::new("session");
    let mut hosted = Hosted::start();
    let mut first = hosted.join(&data, "Viper");
    assert!(
        hosted.run_until(&mut [&mut first], Duration::from_secs(10), |s, _| s[0]
            .client
            .token()
            .is_some()),
        "the host grants a token"
    );
    let granted: KeptToken = first.client.token().cloned().expect("a token");
    // The game stops without a goodbye (nothing here sends one), so the
    // host still lists the player as connected when the same game joins
    // again: the new connection replaces the old.
    drop(first);
    hosted.run_until(&mut [], Duration::from_secs(1), |_, _| false);
    // The same token, kept for another address (the game was found another
    // way): only its session names the host.
    let mut store = Store::default();
    store.keep(granted.clone(), "203.0.113.9:26900");
    store.save(&data.0).unwrap();
    assert!(store.for_address(&hosted.server.to_string()).is_none());

    let mut second = hosted.join(&data, "Viper");
    assert!(
        hosted.run_until(&mut [&mut second], Duration::from_secs(10), |_, seen| {
            welcomed(seen)
        }),
        "the player is welcomed back after the join"
    );
}

/// The token the host grants a newly joined game can reach the client with
/// the Accepted, before the game reads that the join was accepted, and the
/// client's store then holds the new token for the host's session in place
/// of the one the game came with. The Rejoin must still send the token the
/// game held when it started (B8: a macOS CI run lost the player this way).
/// Simulated exactly, with no timing: the file is rewritten as the grant
/// rewrites it, before the session has read anything.
#[test]
fn a_token_the_host_grants_first_does_not_replace_the_one_to_send() {
    let data = Data::new("granted-first");
    let mut hosted = Hosted::start();
    let mut first = hosted.join(&data, "Viper");
    assert!(
        hosted.run_until(&mut [&mut first], Duration::from_secs(10), |s, _| s[0]
            .client
            .token()
            .is_some()),
        "the host grants a token"
    );
    let granted: KeptToken = first.client.token().cloned().expect("a token");
    drop(first);
    hosted.run_until(&mut [], Duration::from_secs(1), |_, _| false);
    let mut store = Store::default();
    store.keep(granted.clone(), "203.0.113.9:26900");
    store.save(&data.0).unwrap();

    let mut second = hosted.join(&data, "Viper");
    // The grant of the new connection, as the client's store keeps it: a
    // token of the host's session that is not the player's own.
    let mut file = Store::load(&data.0, unix_now());
    file.keep(
        KeptToken {
            token: tore_net::Token(granted.token.0 ^ 0xffff),
            ..granted.clone()
        },
        &hosted.server.to_string(),
    );
    file.save(&data.0).unwrap();
    assert!(
        hosted.run_until(&mut [&mut second], Duration::from_secs(10), |_, seen| {
            welcomed(seen)
        }),
        "the player is welcomed back with the token it held at the start"
    );
}

/// A game with no token joins as a new player, and keeps the token it is
/// given.
#[test]
fn a_game_with_no_token_joins_as_a_new_player() {
    let data = Data::new("none");
    let mut hosted = Hosted::start();
    let mut joiner = hosted.join(&data, "Viper");
    let mut greeted = false;
    assert!(
        hosted.run_until(&mut [&mut joiner], Duration::from_secs(10), |s, events| {
            greeted |= welcomed(events);
            s[0].client.token().is_some()
        }),
        "a token is granted"
    );
    assert!(!greeted, "nobody welcomed a new player back");
    assert_eq!(Store::load(&data.0, unix_now()).entries.len(), 1);
}
