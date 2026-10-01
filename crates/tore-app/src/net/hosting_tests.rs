//! The host thread with the game's own client over the in-process link and a
//! remote bot over loopback UDP, in real time, on the synthetic import (no
//! retail data): the acceptance tests of slice EF3.
//!
//! The "game side" is a `tore_session::Client` flown by the scripted pilot
//! and pumped every 16 ms, as the game pumps its session once a frame at 60
//! frames a second; the remote player is the same on its own thread with its
//! own socket, pumped every millisecond or two, as `tore-bot` runs.
//!
//! ```sh
//! cargo test --locked -p tore-app hosting -- --nocapture
//! ```

use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tore_formats::aircraft::AircraftId;
use tore_net::{CloseReason, DisconnectReason, bind_udp};
use tore_session::bot::Bot;
use tore_session::wire::messages::Goodbye;
use tore_session::{Client, ClientConfig, ClientEvent, ClientPhase};
use tore_world::mission::{Skill, Start};
use tore_world::test_support::resources::{THEATER, resources};

/// How often the game side pumps its session: a 60 Hz frame.
const FRAME: Duration = Duration::from_millis(16);

/// The friendly flight of four (the hosting player, the remote player and
/// two AI wingmen) against two enemy AI, 5 nautical miles apart, as the
/// loopback test flies.
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

fn loopback() -> Listen {
    Listen::Address("127.0.0.1".parse().unwrap())
}

/// The hosted game's settings, as `tore-app --host` makes them.
fn hosted_config() -> HostConfig {
    config(&HostOptions {
        mission: "duel.txt".into(),
        spec: spec(),
        port: 0,
        name: "Host's game".into(),
        open_planes: OpenPlanes::Friendly,
        password: None,
        callsign: "Host".into(),
        slot: None,
    })
}

/// A host on a free loopback port, with the hosted game's settings.
fn start_host(port: u16) -> (HostThread, LinkEnd, SocketAddr) {
    let config = hosted_config();
    let (thread, link) = HostThread::start(HostSetup {
        spec: spec(),
        resources: import(),
        config,
        listen: loopback(),
        port,
    })
    .expect("the host starts");
    let address = thread.addresses()[0];
    (thread, link, address)
}

/// The hosting player's game: a client over the link, pumped once a frame.
struct Game {
    bot: Bot,
    link: LinkEnd,
    clock: RealClock,
    events: Vec<ClientEvent>,
    seated_tick: Option<u64>,
    closed: Option<CloseReason>,
}

impl Game {
    fn join(link: LinkEnd) -> Self {
        let clock = RealClock::new();
        let config = ClientConfig {
            entropy: tore_net::Entropy::System,
            ..ClientConfig::new(LINK_ADDRESS, "Host", build_id())
        };
        let client = Client::connect(config, import(), clock.now()).expect("a client");
        let mut bot = Bot::new(client);
        // The King starts each mission once everyone holding a slot is
        // ready, as `tore-app --host` does.
        bot.start_when_ready = true;
        Self {
            bot,
            link,
            clock,
            events: Vec::new(),
            seated_tick: None,
            closed: None,
        }
    }

    /// One frame's turn, as `NetSession::pump` and the frame take it.
    fn pump(&mut self) {
        let now = self.clock.now();
        let _ = self.bot.client.receive_from(now, &mut self.link);
        self.bot.update(now);
        let _ = self.bot.client.transmit(&mut self.link);
        while let Some(event) = self.bot.client.poll_event() {
            match &event {
                ClientEvent::Seated { tick, .. } => self.seated_tick = Some(u64::from(*tick)),
                ClientEvent::Closed(reason) => self.closed = Some(reason.clone()),
                _ => {}
            }
            self.events.push(event);
        }
    }

    /// Pumps once a frame for `time`.
    fn fly(&mut self, time: Duration) {
        let until = Instant::now() + time;
        while Instant::now() < until {
            self.pump();
            thread::sleep(FRAME);
        }
    }

    /// Pumps until `done` or `limit`; true when done.
    fn fly_until(&mut self, limit: Duration, mut done: impl FnMut(&Self) -> bool) -> bool {
        let until = Instant::now() + limit;
        while Instant::now() < until {
            self.pump();
            if done(self) {
                return true;
            }
            thread::sleep(FRAME);
        }
        false
    }

    /// Corrections of the own plane after the second of seating, which snaps.
    fn corrections_after_seating(&self) -> Vec<tore_session::client::Correction> {
        let seated = self.seated_tick.expect("seated");
        self.bot
            .client
            .corrections()
            .iter()
            .filter(|c| c.tick >= seated + 120)
            .copied()
            .collect()
    }
}

/// What the remote player saw.
#[derive(Default)]
struct Remote {
    events: Vec<ClientEvent>,
    seated: bool,
    debrief: bool,
    closed: Option<CloseReason>,
    /// When each new snapshot arrived, and the newest frame's tick then.
    snapshots: Vec<(Instant, u64)>,
    corrections: u64,
    /// The host said it left the game, and the words the player was shown.
    host_left: bool,
    close_text: Option<String>,
}

impl Remote {
    fn refused(&self) -> bool {
        self.events.iter().any(|e| {
            matches!(
                e,
                ClientEvent::SeatRefused(_)
                    | ClientEvent::ContentRefused { .. }
                    | ClientEvent::MissionFailed(_)
            )
        })
    }

    /// The longest time between two snapshots within `from` to `to`.
    fn longest_gap(&self, from: Instant, to: Instant) -> Duration {
        self.snapshots
            .windows(2)
            .filter(|pair| pair[1].0 >= from && pair[0].0 <= to)
            .map(|pair| pair[1].0 - pair[0].0)
            .max()
            .unwrap_or(Duration::MAX)
    }

    /// The predicted tick at the newest snapshot at or before `at`.
    fn tick_at(&self, at: Instant) -> u64 {
        self.snapshots
            .iter()
            .rev()
            .find(|(when, _)| *when <= at)
            .map_or(0, |(_, tick)| *tick)
    }
}

/// A remote player over UDP on its own thread: joins `server`, flies, leaves
/// after `leave_after` (if given), and returns when its connection closes or
/// `limit` passes.
fn remote(
    server: SocketAddr,
    leave_after: Option<Duration>,
    limit: Duration,
    seated: Arc<AtomicBool>,
) -> thread::JoinHandle<Remote> {
    thread::spawn(move || {
        let clock = RealClock::new();
        let mut socket = bind_udp("127.0.0.1:0".parse().unwrap()).expect("a socket");
        let config = ClientConfig {
            entropy: tore_net::Entropy::System,
            ..ClientConfig::new(server, "Guest", build_id())
        };
        let client = Client::connect(config, import(), clock.now()).expect("a client");
        let mut bot = Bot::new(client);
        let mut seen = Remote::default();
        let mut left = false;
        let mut count = 0;
        let mut tick = 0;
        while clock.now() < limit && seen.closed.is_none() {
            let now = clock.now();
            let _ = bot.client.receive_from(now, &mut socket);
            if leave_after.is_some_and(|at| now >= at) && !left {
                left = true;
                bot.client.leave_game(now);
            }
            if let Some(frame) = bot.update(now) {
                tick = frame.tick;
            }
            let _ = bot.client.transmit(&mut socket);
            let stats = bot.client.clone_stats();
            if stats.snapshots > count {
                count = stats.snapshots;
                seen.snapshots.push((Instant::now(), tick));
            }
            seen.corrections = stats.corrections;
            while let Some(event) = bot.client.poll_event() {
                match &event {
                    ClientEvent::Seated { .. } => {
                        seen.seated = true;
                        seated.store(true, Ordering::SeqCst);
                    }
                    ClientEvent::Debrief(_) => seen.debrief = true,
                    ClientEvent::Goodbye(goodbye) => {
                        seen.host_left = *goodbye == Goodbye::HostLeft;
                    }
                    ClientEvent::Closed(reason) => {
                        seen.close_text = Some(bot.client.close_text(reason));
                        seen.closed = Some(reason.clone());
                    }
                    _ => {}
                }
                seen.events.push(event);
            }
            thread::sleep(
                bot.client
                    .next_wake(now)
                    .clamp(Duration::from_micros(200), Duration::from_millis(2)),
            );
        }
        seen
    })
}

/// Waits for `flag` for at most `limit`, pumping the game meanwhile.
fn wait_for(game: &mut Game, flag: &AtomicBool, limit: Duration) -> bool {
    game.fly_until(limit, |_| flag.load(Ordering::SeqCst))
}

/// Every report so far, and whether any is a fault or a refusal.
fn faults(reports: &[Report]) -> Vec<&Report> {
    reports
        .iter()
        .filter(|r| {
            matches!(
                r,
                Report::Log(
                    HostLog::Fault { .. }
                        | HostLog::Refused { .. }
                        | HostLog::ContentRefused { .. }
                        | HostLog::SeatRefused { .. }
                )
            )
        })
        .collect()
}

fn server_stopping(reason: &Option<CloseReason>) -> bool {
    matches!(
        reason,
        Some(CloseReason::Disconnected {
            reason: DisconnectReason::ServerStopping,
            by_peer: true
        })
    )
}

/// The port binds again at once.
fn port_is_free(address: SocketAddr) -> bool {
    ServerSocket::bind(loopback(), address.port()).is_ok()
}

/// Acceptance: a hosted mission with the hosting player and a remote bot flies
/// 30 seconds with no correction of the hosting player's plane after seating,
/// and the bot flies its whole flight and leaves with its debrief.
#[test]
fn a_hosted_mission_flies_with_no_correction_of_the_hosting_players_plane() {
    let seconds = std::env::var("TORE_HOSTED_SECONDS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(30u64);
    let (mut thread, link, server) = start_host(0);
    let mut game = Game::join(link);
    assert!(
        game.fly_until(Duration::from_secs(10), |g| g.seated_tick.is_some()),
        "the hosting player is seated: {:?}",
        game.events
    );
    let seated = Arc::new(AtomicBool::new(false));
    let flight = Duration::from_secs(seconds);
    let started = Instant::now();
    let guest = remote(
        server,
        Some(flight + Duration::from_secs(2)),
        flight + Duration::from_secs(20),
        Arc::clone(&seated),
    );
    assert!(wait_for(&mut game, &seated, Duration::from_secs(10)));
    game.fly(flight.saturating_sub(started.elapsed()) + Duration::from_secs(2));
    // A little more while the guest leaves.
    game.fly(Duration::from_millis(500));
    let guest = guest.join().expect("the guest's thread");
    let stats = game.bot.client.clone_stats();
    let after = game.corrections_after_seating();
    let mut reports = thread.poll();
    eprintln!(
        "hosting player: {:#?}\nall corrections: {:?}",
        stats,
        game.bot.client.corrections()
    );
    assert_eq!(game.bot.client.phase(), ClientPhase::Flying);
    assert!(after.is_empty(), "corrections after seating: {after:?}");
    assert_eq!(stats.mismatches, 0, "{stats:#?}");
    assert!(
        stats.hashes_compared as f64 >= seconds as f64 * 30. * 0.9,
        "{} hashes compared",
        stats.hashes_compared
    );
    assert!(guest.seated && guest.debrief, "{:?}", guest.events);
    assert!(!guest.refused(), "{:?}", guest.events);
    assert!(
        matches!(
            guest.closed,
            Some(CloseReason::Disconnected {
                reason: DisconnectReason::Left,
                ..
            })
        ),
        "{:?}",
        guest.closed
    );
    assert!(thread.stop(JOIN_LIMIT));
    reports.extend(thread.poll());
    assert_eq!(thread.end(), Some(&End::Stopped));
    assert!(faults(&reports).is_empty(), "{reports:#?}");
}

/// Acceptance: the game side stops pumping for 2 seconds. The host ticks on,
/// the remote player's snapshots keep coming, and the hosting player flies on
/// afterwards.
#[test]
fn a_two_second_window_stall_stalls_nobody() {
    let (thread, link, server) = start_host(0);
    let mut game = Game::join(link);
    assert!(game.fly_until(Duration::from_secs(10), |g| g.seated_tick.is_some()));
    let seated = Arc::new(AtomicBool::new(false));
    let guest = remote(
        server,
        Some(Duration::from_secs(12)),
        Duration::from_secs(30),
        Arc::clone(&seated),
    );
    assert!(wait_for(&mut game, &seated, Duration::from_secs(10)));
    game.fly(Duration::from_secs(3));
    let before = game.bot.client.corrections().len();
    let stall = Instant::now();
    thread::sleep(Duration::from_secs(2));
    let resumed = Instant::now();
    game.fly(Duration::from_secs(3));
    let after_recovery = game.bot.client.corrections().len();
    game.fly(Duration::from_secs(2));
    let guest = guest.join().expect("the guest's thread");
    let corrections = game.bot.client.corrections();
    eprintln!(
        "hosting player: {} corrections before the stall, {} in the 3 seconds after, {} in the 2 \
         seconds after that: {:?}\nstats: {:#?}",
        before,
        after_recovery - before,
        corrections.len() - after_recovery,
        &corrections[before..],
        game.bot.client.clone_stats()
    );
    let gap = guest.longest_gap(stall, resumed);
    let ticks = guest.tick_at(resumed) - guest.tick_at(stall);
    eprintln!("guest: longest snapshot gap in the stall {gap:?}, ticks in it {ticks}");
    // The host ticked on: about 240 ticks in 2 seconds.
    assert!((216..=264).contains(&ticks), "{ticks} ticks in the stall");
    // A snapshot every 33 ms; a few intervals at most between two.
    assert!(gap <= Duration::from_millis(150), "a gap of {gap:?}");
    assert!(guest.seated && guest.debrief && !guest.refused());
    assert!(matches!(
        guest.closed,
        Some(CloseReason::Disconnected {
            reason: DisconnectReason::Left,
            ..
        })
    ));
    // The hosting player flies on, and recovers from the stall with at most
    // one correction (EF4: before, every snapshot of the next two seconds
    // was adopted 4 ticks ahead of the prediction).
    assert_eq!(game.bot.client.phase(), ClientPhase::Flying);
    assert!(
        after_recovery - before <= 1,
        "corrections after the stall: {:?}",
        &corrections[before..]
    );
    assert_eq!(corrections.len(), after_recovery, "{corrections:?}");
    drop(thread);
}

/// The hosting player's window is held still for 8 seconds (a drag, a
/// resize, a long load): the game side stops pumping. Its connection is not
/// dropped for the silence, the host and a remote bot fly on with no drop,
/// and afterwards the hosting player recovers and still holds the crown: its
/// End mission ends the mission for everyone.
#[test]
fn an_eight_second_window_stall_drops_nobody_and_the_king_still_reigns() {
    let (thread, link, server) = start_host(0);
    let mut game = Game::join(link);
    assert!(game.fly_until(Duration::from_secs(10), |g| g.seated_tick.is_some()));
    let seated = Arc::new(AtomicBool::new(false));
    let guest = remote(server, None, Duration::from_secs(40), Arc::clone(&seated));
    assert!(wait_for(&mut game, &seated, Duration::from_secs(10)));
    game.fly(Duration::from_secs(3));
    let before = game.bot.client.corrections().len();
    let stall = Instant::now();
    thread::sleep(Duration::from_secs(8));
    let resumed = Instant::now();
    game.fly(Duration::from_secs(4));
    let after = game.bot.client.corrections().len();
    let stats = game.bot.client.clone_stats();
    eprintln!(
        "hosting player after an 8 s stall: {} corrections: {:?}\nstats: {stats:#?}",
        after - before,
        &game.bot.client.corrections()[before..]
    );
    assert!(game.closed.is_none(), "{:?}", game.closed);
    assert_eq!(game.bot.client.phase(), ClientPhase::Flying);
    // It recovers as the stall fix allows: the client takes the host's
    // newest state rather than step the 8-second backlog (a catch-up, not a
    // correction); a tick or two late on resuming then costs a few
    // corrections, one every 125 ms at most while the host still repeats,
    // and then none. On an idle machine they are a few thousandths of a
    // foot; with the whole test suite running beside this real-time test,
    // the debug client is starved for a while and its sizes say little, so
    // they are printed rather than judged.
    assert!(stats.catch_ups >= 1, "{stats:#?}");
    // And then it settles: two seconds with the prediction the host's at
    // every snapshot (on an idle machine at once; a starved one takes a few
    // seconds of clock steering).
    let mut quiet_since = (Instant::now(), game.bot.client.corrections().len());
    let settled = game.fly_until(Duration::from_secs(12), |g| {
        let count = g.bot.client.corrections().len();
        if count != quiet_since.1 {
            quiet_since = (Instant::now(), count);
        }
        quiet_since.0.elapsed() >= Duration::from_secs(2)
    });
    assert!(
        settled,
        "still corrected: {:?}",
        &game.bot.client.corrections()[after..]
    );
    let lobby = game.bot.client.lobby().expect("the lobby").clone();
    assert!(lobby.is_king());
    assert_eq!(lobby.players.len(), 2, "{lobby:?}");
    assert!(lobby.players.iter().all(|p| p.flying), "{lobby:?}");
    // The King's verbs still work: End mission returns both to the lobby.
    game.bot.client.end_mission();
    assert!(game.fly_until(Duration::from_secs(5), |g| {
        g.events
            .iter()
            .any(|e| matches!(e, ClientEvent::Debrief(_)))
    }));
    assert_eq!(game.bot.client.phase(), ClientPhase::Lobby);
    game.fly(Duration::from_secs(1));
    drop(thread);
    let guest = guest.join().expect("the guest's thread");
    let gap = guest.longest_gap(stall, resumed);
    let ticks = guest.tick_at(resumed) - guest.tick_at(stall);
    eprintln!("guest: longest snapshot gap in the stall {gap:?}, ticks in it {ticks}");
    assert!((912..=1008).contains(&ticks), "{ticks} ticks in the stall");
    assert!(gap <= Duration::from_millis(150), "a gap of {gap:?}");
    assert!(guest.seated && !guest.refused(), "{:?}", guest.events);
    assert!(
        guest.events.iter().any(|e| matches!(
            e,
            ClientEvent::MissionEnded(ended) if ended.reason == EndReason::EndedByServer
        )),
        "the King's End mission reached the guest: {:?}",
        guest.events
    );
    assert!(guest.host_left, "{:?}", guest.events);
}

/// Acceptance (EF4): the hosting player, the King, ends the mission; both
/// players get their debriefs and are back in the lobby, still connected;
/// the remote player readies again by itself, the King starts again, and
/// both fly a second mission.
#[test]
fn the_king_ends_the_mission_and_everyone_flies_again() {
    let (mut thread, link, server) = start_host(0);
    let mut game = Game::join(link);
    assert!(game.fly_until(Duration::from_secs(10), |g| g.seated_tick.is_some()));
    let seated = Arc::new(AtomicBool::new(false));
    let guest = remote(server, None, Duration::from_secs(30), Arc::clone(&seated));
    assert!(wait_for(&mut game, &seated, Duration::from_secs(10)));
    game.fly(Duration::from_secs(1));
    game.bot.client.end_mission();
    assert!(game.fly_until(Duration::from_secs(5), |g| {
        g.events
            .iter()
            .any(|e| matches!(e, ClientEvent::Debrief(_)))
    }));
    assert_eq!(game.bot.client.phase(), ClientPhase::Lobby);
    // The King's game starts again once everyone holding a slot is ready.
    game.seated_tick = None;
    assert!(
        game.fly_until(Duration::from_secs(10), |g| g.seated_tick.is_some()),
        "{:?}",
        game.bot.client.lobby()
    );
    // Lobby states reach a flying player at most once a second.
    game.fly(Duration::from_millis(1500));
    let lobby = game.bot.client.lobby().expect("the lobby").clone();
    assert_eq!(
        lobby.phase,
        tore_session::wire::messages::LobbyPhase::Flying
    );
    assert_eq!(lobby.players.len(), 2, "{lobby:?}");
    assert!(
        game.bot
            .client
            .lobby()
            .unwrap()
            .players
            .iter()
            .all(|p| p.flying),
        "{:?}",
        game.bot.client.lobby()
    );
    assert!(thread.stop(JOIN_LIMIT));
    let guest = guest.join().expect("the guest's thread");
    let seatings = guest
        .events
        .iter()
        .filter(|e| matches!(e, ClientEvent::Seated { .. }))
        .count();
    assert_eq!(seatings, 2, "{:?}", guest.events);
    let debriefs = guest
        .events
        .iter()
        .filter(|e| matches!(e, ClientEvent::Debrief(_)))
        .count();
    assert_eq!(
        debriefs, 2,
        "the end, then the host leaving: {:?}",
        guest.events
    );
    assert!(server_stopping(&guest.closed), "{:?}", guest.closed);
    assert!(guest.host_left, "{:?}", guest.events);
}

/// Acceptance: the hosting player leaves the game; the host ends it for
/// everyone. The remote player gets "Mission ended" (the host left), its
/// debrief and "The host left the game"; the thread ends in time and the
/// port binds again.
#[test]
fn the_hosting_player_leaving_ends_the_game_cleanly() {
    let (mut thread, link, server) = start_host(0);
    let mut game = Game::join(link);
    assert!(game.fly_until(Duration::from_secs(10), |g| g.seated_tick.is_some()));
    let seated = Arc::new(AtomicBool::new(false));
    let guest = remote(server, None, Duration::from_secs(30), Arc::clone(&seated));
    assert!(wait_for(&mut game, &seated, Duration::from_secs(10)));
    game.fly(Duration::from_secs(1));
    let now = game.clock.now();
    game.bot.client.leave_game(now);
    assert!(game.fly_until(Duration::from_secs(5), |g| g.closed.is_some()));
    assert!(
        game.events
            .iter()
            .any(|e| matches!(e, ClientEvent::Debrief(_)))
    );
    let asked = Instant::now();
    assert!(thread.stop(JOIN_LIMIT));
    let took = asked.elapsed();
    let guest = guest.join().expect("the guest's thread");
    eprintln!("leave: the host stopped in {took:?}");
    assert!(took < STOP_GRACE + Duration::from_millis(500), "{took:?}");
    assert_eq!(thread.end(), Some(&End::Stopped));
    assert!(
        guest.events.iter().any(|e| matches!(
            e,
            ClientEvent::MissionEnded(ended) if ended.reason == EndReason::HostLeft
        )),
        "{:?}",
        guest.events
    );
    assert!(guest.debrief, "{:?}", guest.events);
    assert!(server_stopping(&guest.closed), "{:?}", guest.closed);
    assert!(guest.host_left, "{:?}", guest.events);
    assert_eq!(guest.close_text.as_deref(), Some("The host left the game."));
    assert!(port_is_free(server));
}

/// Acceptance: the hosting player quits (the window closes): the game says
/// goodbye on its own connection and drops the host, which stops it.
#[test]
fn quitting_the_hosting_game_ends_the_game_cleanly() {
    let (thread, link, server) = start_host(0);
    let mut game = Game::join(link);
    assert!(game.fly_until(Duration::from_secs(10), |g| g.seated_tick.is_some()));
    let seated = Arc::new(AtomicBool::new(false));
    let guest = remote(server, None, Duration::from_secs(30), Arc::clone(&seated));
    assert!(wait_for(&mut game, &seated, Duration::from_secs(10)));
    game.fly(Duration::from_secs(1));
    // As `exiting` does: a polite disconnect, flushed, then the session goes.
    let now = game.clock.now();
    game.bot.client.disconnect(now);
    let _ = game.bot.client.transmit(&mut game.link);
    let asked = Instant::now();
    drop(thread);
    let took = asked.elapsed();
    let guest = guest.join().expect("the guest's thread");
    eprintln!("quit: the host stopped in {took:?}");
    assert!(took < STOP_GRACE + Duration::from_millis(500), "{took:?}");
    assert!(guest.debrief, "{:?}", guest.events);
    assert!(server_stopping(&guest.closed), "{:?}", guest.closed);
    assert!(guest.host_left, "{:?}", guest.events);
    assert!(port_is_free(server));
}

/// Acceptance: a panic on the host thread ends the hosted game with a plain
/// message for the hosting player; the remote player is told the server is
/// stopping; the port binds again.
#[test]
fn a_panic_on_the_host_thread_ends_the_game_with_a_plain_message() {
    let (mut thread, link, server) = start_host(0);
    let mut game = Game::join(link);
    assert!(game.fly_until(Duration::from_secs(10), |g| g.seated_tick.is_some()));
    let seated = Arc::new(AtomicBool::new(false));
    let guest = remote(server, None, Duration::from_secs(30), Arc::clone(&seated));
    assert!(wait_for(&mut game, &seated, Duration::from_secs(10)));
    game.fly(Duration::from_secs(1));
    let asked = Instant::now();
    assert!(thread.send(Command::Panic));
    let mut failure = None;
    while failure.is_none() && asked.elapsed() < Duration::from_secs(2) {
        failure = thread.poll().into_iter().find_map(|report| match report {
            Report::Ended(end) => end.failure(),
            _ => None,
        });
        thread::sleep(Duration::from_millis(5));
    }
    let took = asked.elapsed();
    eprintln!("panic: reported in {took:?}: {failure:?}");
    assert_eq!(
        failure.as_deref(),
        Some("The game you were hosting stopped: a test asked the host thread to panic")
    );
    assert!(took < Duration::from_millis(500), "{took:?}");
    assert!(port_is_free(server));
    assert!(thread.stop(JOIN_LIMIT));
    let guest = guest.join().expect("the guest's thread");
    assert!(server_stopping(&guest.closed), "{:?}", guest.closed);
    // The hosting player's own client hears the same, over the link.
    assert!(game.fly_until(Duration::from_secs(2), |g| g.closed.is_some()));
    assert!(server_stopping(&game.closed), "{:?}", game.closed);
}

/// A mission the import cannot build is the thread's end, not a hang.
#[test]
fn a_mission_the_import_cannot_build_is_reported() {
    let mut spec = spec();
    spec.wings[3].aircraft = AircraftId::Su27;
    let (mut thread, _link) = HostThread::start(HostSetup {
        spec,
        resources: import(),
        config: HostConfig::new(build_id()),
        listen: loopback(),
        port: 0,
    })
    .unwrap();
    let address = thread.addresses()[0];
    let started = Instant::now();
    while thread.end().is_none() && started.elapsed() < Duration::from_secs(10) {
        thread.poll();
        thread::sleep(Duration::from_millis(5));
    }
    let failure = thread.end().and_then(End::failure).expect("a failure");
    assert!(
        failure.starts_with("The game could not be hosted: the mission cannot be built"),
        "{failure}"
    );
    assert!(port_is_free(address));
}

/// A port in use is refused at once, with what to do.
#[test]
fn a_port_in_use_is_refused_at_once() {
    let (_thread, _link, server) = start_host(0);
    let error = HostThread::start(HostSetup {
        spec: spec(),
        resources: import(),
        config: HostConfig::new(build_id()),
        listen: loopback(),
        port: server.port(),
    })
    .err()
    .expect("refused");
    assert!(error.starts_with("Cannot host on UDP port"), "{error}");
    assert!(error.contains("--port"), "{error}");
}

/// The log line for the hosting player's own connection names it.
#[test]
fn the_log_names_the_hosting_players_own_connection() {
    let line = log_line(&HostLog::Connected {
        tick: 0,
        address: LINK_ADDRESS,
        callsign: "Viper".into(),
    });
    assert_eq!(line, "tick 0: this game joined as Viper");
    let line = log_line(&HostLog::Left {
        tick: 240,
        seat: Some(1),
        callsign: "Cobra".into(),
        plane: Some(1),
        reason: LeaveReason::Silent,
    });
    assert_eq!(
        line,
        "tick 240: seat 1 Cobra (plane 1) left: no packet for 5 seconds"
    );
}
