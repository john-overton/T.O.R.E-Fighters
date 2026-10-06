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

/// A late input explains a mismatch or a correction within this many ticks
/// of the host's clock after it (one second, as in the network matrix).
const LATE_INPUT_TICKS: u64 = 120;

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
        listing: None,
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
    /// The predicted tick at each frame in which the host said it had
    /// repeated this player's input (an input that arrived late), and at
    /// each frame in which a snapshot's hash differed from the prediction.
    repeats: Vec<u64>,
    mismatch_ticks: Vec<u64>,
    counts: (u64, u64),
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
            repeats: Vec::new(),
            mismatch_ticks: Vec::new(),
            counts: (0, 0),
        }
    }

    /// One frame's turn, as `NetSession::pump` and the frame take it.
    fn pump(&mut self) {
        let now = self.clock.now();
        let _ = self.bot.client.receive_from(now, &mut self.link);
        self.bot.update(now);
        let _ = self.bot.client.transmit(&mut self.link);
        let stats = self.bot.client.clone_stats();
        let tick = self.bot.client.prediction().map_or(0, |p| p.tick());
        if stats.inputs_repeated > self.counts.0 {
            self.repeats.push(tick);
        }
        if stats.mismatches > self.counts.1 {
            self.mismatch_ticks.push(tick);
        }
        self.counts = (stats.inputs_repeated, stats.mismatches);
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

    /// Whether the host repeated this player's input in the second before
    /// the predicted tick `at`: a late input explains a mismatch or a
    /// correction then, as the network matrix's rule has it.
    fn late_input_before(&self, at: u64) -> bool {
        self.repeats
            .iter()
            .any(|&r| r <= at + 2 && at.saturating_sub(r) < LATE_INPUT_TICKS)
    }
}

/// What the remote player saw.
#[derive(Default)]
struct Remote {
    events: Vec<ClientEvent>,
    seated: bool,
    debrief: bool,
    closed: Option<CloseReason>,
    /// When the guest's loop found new snapshots, the newest frame's tick
    /// then, and how many snapshots it had received in all.
    snapshots: Vec<(Instant, u64, u64)>,
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
            .find(|(when, ..)| *when <= at)
            .map_or(0, |(_, tick, _)| *tick)
    }

    /// Between the first time the guest found a snapshot at or after `from`
    /// and the last at or before `to`: how long, the ticks its clock ran,
    /// and the snapshots received. Both ends are the guest's own readings,
    /// so a guest that wakes late (a starved runner) measures its rates
    /// right, where its ticks at `from` and `to` would be short by however
    /// late it woke.
    fn within(&self, from: Instant, to: Instant) -> (Duration, u64, u64) {
        let inside: Vec<_> = self
            .snapshots
            .iter()
            .filter(|(when, ..)| *when >= from && *when <= to)
            .collect();
        match (inside.first(), inside.last()) {
            (Some(first), Some(last)) => (last.0 - first.0, last.1 - first.1, last.2 - first.2),
            _ => (Duration::ZERO, 0, 0),
        }
    }
}

/// The longest wait for the guest's snapshots while the game side stalls,
/// in the default suite. A snapshot leaves every 33 ms; the strict bound is
/// 150 ms. The guest's loop asks to sleep at most 2 ms, but on the macOS CI
/// runners a sleep can last 140 ms and, with the whole test suite beside it,
/// longer (a gap of 195 ms in an 8-second stall on macos-15-intel). A host
/// held up by the stalled game would leave a gap as long as the stall.
const LENIENT_GAP: Duration = Duration::from_secs(1);

/// What the guest saw of the host while the game side stalled from `stall`
/// to `resumed`: the host ticked on and sent its snapshots throughout.
///
/// Strict (a machine whose sleeps are accurate): `ticks` in the stall by
/// the guest's newest snapshot at each end, and no gap over 150 ms. Lenient
/// (any runner): rates over what the guest saw, which its own late wakes do
/// not bias, a 120 Hz clock within a tenth and 24 of the 30 snapshots a
/// second, and no gap over [`LENIENT_GAP`].
fn assert_host_flew_on(
    guest: &Remote,
    stall: Instant,
    resumed: Instant,
    ticks: std::ops::RangeInclusive<u64>,
    strict: bool,
) {
    let gap = guest.longest_gap(stall, resumed);
    let in_stall = guest.tick_at(resumed) - guest.tick_at(stall);
    let (span, span_ticks, snapshots) = guest.within(stall, resumed);
    let seconds = span.as_secs_f64();
    eprintln!(
        "guest: longest snapshot gap in the stall {gap:?}, ticks in it {in_stall}; \
         over {span:?} it saw {span_ticks} ticks and {snapshots} snapshots"
    );
    if strict {
        assert!(ticks.contains(&in_stall), "{in_stall} ticks in the stall");
        assert!(gap <= Duration::from_millis(150), "a gap of {gap:?}");
        return;
    }
    let stall_seconds = (resumed - stall).as_secs_f64();
    assert!(
        seconds >= stall_seconds / 2.,
        "snapshots seen over {span:?} of the {stall_seconds} s stall"
    );
    let rate = span_ticks as f64 / seconds;
    assert!((108. ..=132.).contains(&rate), "{rate} ticks a second");
    let rate = snapshots as f64 / seconds;
    assert!(rate >= 24., "{rate} snapshots a second");
    assert!(gap <= LENIENT_GAP, "a gap of {gap:?}");
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
                seen.snapshots.push((Instant::now(), tick, count));
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
/// 30 seconds with no correction of the hosting player's plane after seating
/// that a late input does not explain, and the bot flies its whole flight and
/// leaves with its debrief.
///
/// A late input (one the host had to repeat) is the only cause allowed, and
/// at most one tick in twenty may have one. On a quiet machine there is none
/// after seating, but the macOS CI runners sleep 66 to 74 ms when asked for
/// 16 and up to 140 ms (slice EF-X), so this harness's game pumps a few
/// times a second there, its inputs arrive late now and then, and the host's
/// repeats of them differ from what the game predicted by a few millionths
/// of a foot. The strict form, with no correction and no mismatch at all, is
/// [`a_hosted_mission_flies_with_no_correction_at_all`].
#[test]
fn a_hosted_mission_flies_with_no_correction_of_the_hosting_players_plane() {
    hosted_mission(false);
}

/// The acceptance in its strict form, for a quiet machine whose sleeps are
/// close to what they ask (the Linux CI runner, the development machine):
/// no correction after seating and no mismatch, whatever the cause. Run by
/// `network.yml` on Linux, or with `--ignored`.
#[test]
#[ignore = "strict: needs a machine whose sleeps are accurate"]
fn a_hosted_mission_flies_with_no_correction_at_all() {
    hosted_mission(true);
}

fn hosted_mission(strict: bool) {
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
    if strict {
        assert!(after.is_empty(), "corrections after seating: {after:?}");
        assert_eq!(stats.mismatches, 0, "{stats:#?}");
    }
    let unexplained: Vec<_> = after
        .iter()
        .filter(|c| !game.late_input_before(c.now))
        .collect();
    assert!(
        unexplained.is_empty(),
        "corrections after seating with no late input before them: {unexplained:?}\n\
         late inputs at predicted ticks {:?}",
        game.repeats
    );
    // A mismatch with no late input before it is allowed once, as long as
    // no correction lacks one (above): the host's next exact state then
    // found the prediction right, and the player saw nothing. The 32-bit Windows
    // runner had one such mismatch in 975 compared (run 36944204775), not
    // seen elsewhere; the strict form, run on Linux by `network.yml`,
    // allows none. A prediction that drifts from the host's mismatches over
    // and over and fails here.
    let unexplained: Vec<_> = game
        .mismatch_ticks
        .iter()
        .filter(|&&t| !game.late_input_before(t))
        .collect();
    assert!(
        unexplained.len() <= 1,
        "mismatches with no late input before them at predicted ticks {unexplained:?}\n\
         late inputs at {:?}",
        game.repeats
    );
    // A clock that kept the inputs late would repeat many.
    let ticks = stats.hashes_compared * 4;
    assert!(
        stats.inputs_repeated * 20 <= ticks,
        "{} of about {ticks} ticks repeated the input",
        stats.inputs_repeated
    );
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
/// afterwards, recovering with at most one correction that a late input
/// does not explain.
///
/// On the macOS CI runners this harness's game and guest sleep up to 140 ms
/// when they ask for 16 or 2 (slice EF-X), so the game's inputs come late
/// now and then after the stall, and the host's repeats of them cost a
/// correction or two of a thousandth of a foot; and the guest, waking late,
/// sees longer gaps. This form allows both ([`assert_host_flew_on`]). The
/// rule that a stall costs at most one correction is held exactly by the
/// network simulator's `a_stalled_game_recovers_with_at_most_one_correction`
/// (tore-session), and in real time by the strict form,
/// [`a_two_second_window_stall_stalls_nobody_strictly`].
#[test]
fn a_two_second_window_stall_stalls_nobody() {
    two_second_stall(false);
}

/// The 2-second stall in its strict form, for a machine whose sleeps are
/// accurate: at most one correction after the stall, none once recovered,
/// 216 to 264 ticks in the stall and no snapshot gap over 150 ms. Run by
/// `network.yml` on Linux, or with `--ignored`.
#[test]
#[ignore = "strict: needs a machine whose sleeps are accurate"]
fn a_two_second_window_stall_stalls_nobody_strictly() {
    two_second_stall(true);
}

fn two_second_stall(strict: bool) {
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
    // The host ticked on (about 240 ticks in 2 seconds) and sent a snapshot
    // every 33 ms.
    assert_host_flew_on(&guest, stall, resumed, 216..=264, strict);
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
    if strict {
        assert!(
            after_recovery - before <= 1,
            "corrections after the stall: {:?}",
            &corrections[before..]
        );
        assert_eq!(corrections.len(), after_recovery, "{corrections:?}");
    } else {
        // Beyond the stall's own, only corrections a late input explains.
        let unexplained: Vec<_> = corrections[before..]
            .iter()
            .filter(|c| !game.late_input_before(c.now))
            .collect();
        assert!(
            unexplained.len() <= 1,
            "corrections after the stall with no late input before them: {unexplained:?}\n\
             late inputs at predicted ticks {:?}",
            game.repeats
        );
        let late: Vec<_> = corrections[after_recovery..]
            .iter()
            .filter(|c| !game.late_input_before(c.now))
            .collect();
        assert!(late.is_empty(), "corrections once recovered: {late:?}");
    }
    drop(thread);
}

/// The hosting player's window is held still for 8 seconds (a drag, a
/// resize, a long load): the game side stops pumping. Its connection is not
/// dropped for the silence, the host and a remote bot fly on with no drop,
/// and afterwards the hosting player recovers and still holds the crown: its
/// End mission ends the mission for everyone.
///
/// The guest's view of the host in the stall is judged leniently here
/// ([`assert_host_flew_on`]): on macos-15-intel the guest once woke to a
/// 195 ms gap. The strict form is
/// [`an_eight_second_window_stall_drops_nobody_and_the_king_still_reigns_strictly`].
#[test]
fn an_eight_second_window_stall_drops_nobody_and_the_king_still_reigns() {
    eight_second_stall(false);
}

/// The 8-second stall in its strict form, for a machine whose sleeps are
/// accurate: 912 to 1008 ticks in the stall and no snapshot gap over
/// 150 ms. Run by `network.yml` on Linux, or with `--ignored`.
#[test]
#[ignore = "strict: needs a machine whose sleeps are accurate"]
fn an_eight_second_window_stall_drops_nobody_and_the_king_still_reigns_strictly() {
    eight_second_stall(true);
}

fn eight_second_stall(strict: bool) {
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
    // every snapshot (on an idle machine at once; a starved one may jump its
    // clock ahead of the host a few times first). A client stuck behind the
    // host, corrected at every own state, never settles and fails here.
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
    assert_host_flew_on(&guest, stall, resumed, 912..=1008, strict);
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
    // The hosting player's game joins over the in-process link like any
    // other and names its own platform; the remote one over UDP names this
    // build's too.
    assert!(
        lobby
            .players
            .iter()
            .all(|p| p.platform == tore_session::wire::Platform::current()),
        "{lobby:?}"
    );
    assert_eq!(lobby.me().map(|me| me.callsign.as_str()), Some("Host"));
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
        path: tore_session::wire::Path::LocalNetwork,
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
    // A stalled game, and its return (EF-K follow-up).
    let line = log_line(&HostLog::Stalled {
        tick: 300,
        seat: Some(2),
        callsign: "Viper".into(),
    });
    assert_eq!(line, "tick 300: seat 2 Viper: game stalled, flying neutral");
    let line = log_line(&HostLog::Resumed {
        tick: 1188,
        seat: Some(2),
        callsign: "Viper".into(),
        stalled_for: Duration::from_millis(7_420),
    });
    assert_eq!(line, "tick 1188: seat 2 Viper: game back after 7.4 s");
    let line = log_line(&HostLog::Stalled {
        tick: 300,
        seat: None,
        callsign: "Cobra".into(),
    });
    assert_eq!(line, "tick 300: Cobra: game stalled");
}

/// A game this one hosts is found by the search loop (slice EF5) at its
/// game port, with the hosting player listed as the King: the answer comes
/// from the host thread's own socket while the game's session is joined over
/// the link.
#[test]
fn a_hosted_game_is_found_by_the_search_loop() {
    use crate::net::search::{Compat, Own, Search, SearchEvent};
    use tore_net::packet::DiscoverPhase;
    let (_thread, link, address) = start_host(0);
    let mut game = Game::join(link);
    let socket = bind_udp("0.0.0.0:0".parse().unwrap()).expect("a socket");
    let mut search = Search::with(
        socket,
        vec![address],
        Own::this_game(),
        tore_net::reach::random_nonce(),
        Duration::ZERO,
    )
    .expect("a search");
    let clock = RealClock::new();
    let started = Instant::now();
    let mut added = None;
    let mut last = None;
    while started.elapsed() < Duration::from_secs(8) {
        game.pump();
        search.update(clock.now());
        while let Some(event) = search.poll_event() {
            if let SearchEvent::Added(found) | SearchEvent::Changed(found) = event {
                added.get_or_insert(started.elapsed());
                last = Some(found);
            }
        }
        // Done once the hosting player is listed.
        if last.as_ref().is_some_and(|g| g.answer.players == 1) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let found = last.expect("the hosted game is found");
    eprintln!("found after {:?}: {:?}", added.unwrap(), found.answer);
    assert_eq!(found.address, address);
    assert_eq!(found.compat, Compat::Same);
    assert_eq!(found.answer.name, "Host's game");
    // The King starts the mission as soon as the hosting player is ready.
    assert_ne!(found.answer.phase, DiscoverPhase::Closed);
    assert_eq!(found.answer.callsigns, ["Host"]);
    assert_eq!(found.answer.king, "Host");
    assert_eq!((found.answer.players, found.answer.capacity), (1, 4));
    assert!(
        found.answer.summary.starts_with("UKR"),
        "{}",
        found.answer.summary
    );
}

/// The search holds the game port while it lives, so the screen must drop it
/// before this game hosts on the port; with the search gone the host binds at
/// once. The other way round the search falls back to another port and
/// still finds the game (slice EF5).
#[test]
fn the_search_and_the_host_share_the_game_port_one_after_the_other() {
    use crate::net::search::{Own, Search};
    let port = bind_udp("0.0.0.0:0".parse().unwrap())
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let start = |listen| {
        HostThread::start(HostSetup {
            spec: spec(),
            resources: import(),
            config: hosted_config(),
            listen,
            port,
        })
    };
    let search = Search::start(port, Own::this_game(), Duration::ZERO).unwrap();
    assert!(search.on_game_port());
    let refused = start(Listen::Any).err().expect("the port is held");
    assert!(refused.starts_with("Cannot host on UDP port"), "{refused}");
    drop(search);
    let (_thread, _link) = start(Listen::Any).expect("the host binds once the search is gone");
    // With the host up, a new search takes another port, and says so.
    let mut search = Search::start(port, Own::this_game(), Duration::ZERO).unwrap();
    assert!(!search.on_game_port());
    let clock = RealClock::new();
    let started = Instant::now();
    while search.games().is_empty() && started.elapsed() < Duration::from_secs(5) {
        search.update(clock.now());
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(search.games().len(), 1, "found through the unicast targets");
}

/// A master on loopback that answers just enough of the protocol to list a
/// game (Challenge, then Listed) and counts what a host sends it, by kind.
/// The end-to-end tests use the real master (`tore-master`); this checks the
/// hosting thread's side alone.
struct LoopbackMaster {
    address: SocketAddr,
    kinds: Arc<std::sync::Mutex<Vec<tore_net::master::MasterKind>>>,
    /// The candidates of every Register and Heartbeat, oldest first.
    candidates: Arc<std::sync::Mutex<Vec<Vec<tore_net::master::Candidate>>>>,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl LoopbackMaster {
    fn start() -> Self {
        use tore_net::master::{Challenge, CookieKey, Listed, MasterPacket};
        let socket = bind_udp("127.0.0.1:0".parse().unwrap()).unwrap();
        let address = socket.local_addr().unwrap();
        let kinds = Arc::new(std::sync::Mutex::new(Vec::new()));
        let candidates = Arc::new(std::sync::Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = thread::spawn({
            let kinds = Arc::clone(&kinds);
            let candidates = Arc::clone(&candidates);
            let stop = Arc::clone(&stop);
            move || {
                let key = CookieKey::new(tore_net::Entropy::System);
                let clock = RealClock::new();
                let mut buf = [0u8; 2048];
                while !stop.load(Ordering::Relaxed) {
                    let Ok((length, from)) = socket.recv_from(&mut buf) else {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    };
                    let Ok(packet) = MasterPacket::decode(&buf[..length]) else {
                        continue;
                    };
                    kinds.lock().unwrap().push(packet.kind());
                    match &packet {
                        MasterPacket::Register(r) => {
                            candidates.lock().unwrap().push(r.candidates.clone());
                        }
                        MasterPacket::Heartbeat(h) => {
                            candidates.lock().unwrap().push(h.candidates.clone());
                        }
                        _ => {}
                    }
                    let now = clock.now();
                    let answer = match packet {
                        MasterPacket::Register(r) if key.check(from, r.nonce, r.cookie, now) => {
                            MasterPacket::Listed(Listed {
                                nonce: r.nonce,
                                listing_id: 1,
                                token: 2,
                                seen: from,
                                heartbeat_secs: 30,
                                keep_secs: 15,
                                expiry_secs: 90,
                            })
                        }
                        MasterPacket::Register(r) => MasterPacket::Challenge(Challenge {
                            nonce: r.nonce,
                            cookie: key.cookie(from, r.nonce, now),
                        }),
                        _ => continue,
                    };
                    let _ = socket.send_to(&answer.encode().unwrap(), from);
                }
            }
        });
        Self {
            address,
            kinds,
            candidates,
            stop,
            handle: Some(handle),
        }
    }

    /// Whether any Register or Heartbeat so far carried `candidate`.
    fn saw_candidate(&self, candidate: tore_net::master::Candidate) -> bool {
        self.candidates
            .lock()
            .unwrap()
            .iter()
            .any(|list| list.contains(&candidate))
    }

    fn count(&self, kind: tore_net::master::MasterKind) -> usize {
        self.kinds
            .lock()
            .unwrap()
            .iter()
            .filter(|k| **k == kind)
            .count()
    }
}

impl Drop for LoopbackMaster {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Waits up to `limit` for `done`, polling the thread's reports.
fn wait_for_reports(
    thread: &mut HostThread,
    reports: &mut Vec<Report>,
    limit: Duration,
    done: impl Fn(&[Report]) -> bool,
) -> bool {
    let started = Instant::now();
    while started.elapsed() < limit {
        reports.extend(thread.poll());
        if done(reports) {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    false
}

/// A game hosted with a listing registers with the master from its game
/// port, reports where the listing stands, takes it off with `SetListed`
/// (three Unregisters) and lists it again; a game hosted without one never
/// talks to the master.
#[test]
fn a_listed_host_registers_reports_and_unlists_on_command_and_on_stop() {
    use tore_net::master::{ListingState, MasterKind};
    let master = LoopbackMaster::start();
    let (mut thread, _link) = HostThread::start_listed(
        HostSetup {
            spec: spec(),
            resources: import(),
            config: hosted_config(),
            listen: loopback(),
            port: 0,
        },
        Some(Listing::listed(Some(&master.address.to_string()))),
    )
    .expect("the host starts");
    let game_port = thread.addresses()[0];
    let mut reports = Vec::new();
    let listed = |reports: &[Report]| {
        reports
            .iter()
            .filter(|r| matches!(r, Report::Listing(ListingState::Listed { .. })))
            .count()
    };
    assert!(
        wait_for_reports(
            &mut thread,
            &mut reports,
            Duration::from_secs(10),
            |r| listed(r) == 1
        ),
        "{reports:?}"
    );
    // The master saw the game port itself.
    assert!(reports.iter().any(|r| matches!(
        r,
        Report::Listing(ListingState::Listed { seen, .. }) if *seen == game_port
    )));
    assert_eq!(master.count(MasterKind::Register), 2);
    assert!(thread.send(Command::SetListed(false)));
    assert!(wait_for_reports(
        &mut thread,
        &mut reports,
        Duration::from_secs(5),
        |r| r
            .iter()
            .any(|r| matches!(r, Report::Listing(ListingState::Off)))
    ));
    let started = Instant::now();
    while master.count(MasterKind::Unregister) < 3 && started.elapsed() < Duration::from_secs(5) {
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(master.count(MasterKind::Unregister), 3);
    assert!(thread.send(Command::SetListed(true)));
    assert!(wait_for_reports(
        &mut thread,
        &mut reports,
        Duration::from_secs(10),
        |r| listed(r) == 2
    ));
    assert!(thread.stop(JOIN_LIMIT));
    let started = Instant::now();
    while master.count(MasterKind::Unregister) < 6 && started.elapsed() < Duration::from_secs(5) {
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(master.count(MasterKind::Unregister), 6);
    assert!(port_is_free(game_port));

    // Without a listing, nothing goes to any master, and SetListed is a
    // note in the log only.
    let quiet = LoopbackMaster::start();
    let (mut thread, _link, _) = start_host(0);
    assert!(thread.send(Command::SetListed(true)));
    let mut reports = Vec::new();
    wait_for_reports(
        &mut thread,
        &mut reports,
        Duration::from_millis(500),
        |_| false,
    );
    assert!(!reports.iter().any(|r| matches!(r, Report::Listing(_))));
    assert_eq!(quiet.kinds.lock().unwrap().len(), 0);
}

/// A master address that cannot be read refuses the start at once.
#[test]
fn a_bad_master_address_refuses_the_start() {
    let error = HostThread::start_listed(
        HostSetup {
            spec: spec(),
            resources: import(),
            config: hosted_config(),
            listen: loopback(),
            port: 0,
        },
        Some(Listing::listed(Some("host:0"))),
    )
    .err()
    .expect("refused");
    assert!(error.starts_with("Cannot list the game:"), "{error}");
}

/// The King's Visibility (slice F2-1) lists a hosted game and takes it off,
/// as `SetListed` does: public registers with the master, local unlists it
/// (three Unregisters).
#[test]
fn the_kings_visibility_lists_the_game_and_takes_it_off() {
    use tore_net::master::{ListingState, MasterKind};
    use tore_session::settings::{Visibility, number};
    use tore_session::wire::messages::SettingsChange;
    let master = LoopbackMaster::start();
    let (mut thread, link) = HostThread::start_listed(
        HostSetup {
            spec: spec(),
            resources: import(),
            config: hosted_config(),
            listen: loopback(),
            port: 0,
        },
        Some(Listing {
            master: master.address.to_string(),
            listed: false,
            install_id: None,
        }),
    )
    .expect("the host starts");
    let mut game = Game::join(link);
    assert!(game.fly_until(Duration::from_secs(5), |g| {
        g.bot.client.lobby().is_some_and(|l| l.is_king())
    }));
    let mut reports = Vec::new();
    let set = |game: &mut Game, visibility: Visibility| {
        game.bot.client.change_settings(SettingsChange {
            values: vec![(number::VISIBILITY, visibility.value())],
            ..SettingsChange::default()
        });
    };
    let until = |game: &mut Game,
                 thread: &mut HostThread,
                 reports: &mut Vec<Report>,
                 done: &dyn Fn(&[Report]) -> bool| {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(10) {
            game.pump();
            reports.extend(thread.poll());
            if done(reports) {
                return true;
            }
            thread::sleep(FRAME);
        }
        false
    };
    let listed = |reports: &[Report]| {
        reports
            .iter()
            .any(|r| matches!(r, Report::Listing(ListingState::Listed { .. })))
    };
    // Local, as it starts: nothing goes to the master.
    game.fly(Duration::from_millis(300));
    assert_eq!(master.count(MasterKind::Register), 0);
    set(&mut game, Visibility::Public);
    assert!(
        until(&mut game, &mut thread, &mut reports, &listed),
        "{reports:?}"
    );
    set(&mut game, Visibility::Local);
    assert!(until(
        &mut game,
        &mut thread,
        &mut reports,
        &|r: &[Report]| {
            r.iter()
                .rev()
                .find_map(|r| match r {
                    Report::Listing(state) => Some(*state == ListingState::Off),
                    _ => None,
                })
                .unwrap_or(false)
        }
    ));
    let started = Instant::now();
    while master.count(MasterKind::Unregister) < 3 && started.elapsed() < Duration::from_secs(5) {
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(master.count(MasterKind::Unregister), 3);
    assert!(thread.stop(JOIN_LIMIT));
}

/// Slice J4b: a hosting game with port mapping on asks the router (here the
/// fakes on loopback, never a real one) from the moment its socket is bound,
/// tells the game what the router did, gives the listing the Mapped
/// candidate, and removes the mapping before it reports its end; with
/// mapping off, the router hears nothing.
#[test]
fn a_hosting_game_maps_its_port_lists_the_mapped_candidate_and_removes_it_on_stop() {
    use tore_net::master::{Candidate, CandidateKind, ListingState};
    use tore_net::portmap::MapperConfig;
    use tore_net::portmap::fake::{FakeGateway, FakeGatewayConfig};
    let router = FakeGateway::start("127.0.0.1:0".parse().unwrap(), FakeGatewayConfig::default())
        .expect("a fake router");
    let master = LoopbackMaster::start();
    let mapper = MapperConfig {
        upnp: false,
        gateway: Some(router.address()),
        gateway_v6: Some("[::1]:9".parse().unwrap()),
        entropy: tore_net::Entropy::Seeded(5),
        ..MapperConfig::new(0)
    };
    let (mut thread, _link) = HostThread::start_forwarded(
        HostSetup {
            spec: spec(),
            resources: import(),
            config: hosted_config(),
            listen: loopback(),
            port: 0,
        },
        Some(Listing::listed(Some(&master.address.to_string()))),
        Some(Forward::with_config(mapper)),
    )
    .expect("the host starts");
    let game_port = thread.addresses()[0].port();
    let outside = SocketAddr::new("203.0.113.5".parse().unwrap(), game_port);
    let mut reports = Vec::new();
    // The game is told, in plain words, and the lobby's lines wait for it.
    assert!(
        wait_for_reports(&mut thread, &mut reports, Duration::from_secs(10), |r| r
            .iter()
            .any(|r| matches!(r, Report::Forward(_)))),
        "{reports:?}"
    );
    assert_eq!(
        thread.take_notes(),
        [format!(
            "Your router forwards UDP port {game_port} (PCP). Friends can join at {outside}."
        )]
    );
    assert!(thread.take_notes().is_empty());
    let held = router.mappings();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].internal_port, game_port);
    // The master's listing carries the Mapped candidate (the Register or a
    // Heartbeat within the change delay).
    let mapped = Candidate::new(CandidateKind::Mapped, outside);
    let started = Instant::now();
    while !master.saw_candidate(mapped) && started.elapsed() < Duration::from_secs(12) {
        reports.extend(thread.poll());
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        master.saw_candidate(mapped),
        "{:?} {reports:?}",
        master.candidates.lock().unwrap()
    );
    assert!(wait_for_reports(
        &mut thread,
        &mut reports,
        Duration::from_secs(10),
        |r| r
            .iter()
            .any(|r| matches!(r, Report::Listing(ListingState::Listed { .. })))
    ));
    // Stopping removes the mapping before the thread reports its end.
    assert!(thread.stop(JOIN_LIMIT));
    assert!(router.mappings().is_empty());
    assert_eq!(thread.end(), Some(&End::Stopped));

    // Switched off: the router hears nothing.
    let silent = FakeGateway::start("127.0.0.1:0".parse().unwrap(), FakeGatewayConfig::default())
        .expect("a fake router");
    let (mut thread, _link, _) = start_host(0);
    let mut reports = Vec::new();
    wait_for_reports(
        &mut thread,
        &mut reports,
        Duration::from_millis(700),
        |_| false,
    );
    assert!(!reports.iter().any(|r| matches!(r, Report::Forward(_))));
    assert_eq!(silent.pcp_requests() + silent.natpmp_requests(), 0);
    assert!(thread.take_notes().is_empty());
}

/// A game that could not open the port says so plainly and lists no Mapped
/// candidate; the host keeps running.
#[test]
fn a_router_that_refuses_is_told_to_the_game_and_the_host_runs_on() {
    use tore_net::portmap::MapperConfig;
    use tore_net::portmap::fake::{FakeGateway, FakeGatewayConfig};
    let router = FakeGateway::start(
        "127.0.0.1:0".parse().unwrap(),
        FakeGatewayConfig {
            refuse: Some(2),
            ..FakeGatewayConfig::default()
        },
    )
    .expect("a fake router");
    let mapper = MapperConfig {
        upnp: false,
        gateway: Some(router.address()),
        gateway_v6: Some("[::1]:9".parse().unwrap()),
        entropy: tore_net::Entropy::Seeded(5),
        ..MapperConfig::new(0)
    };
    let (mut thread, _link) = HostThread::start_forwarded(
        HostSetup {
            spec: spec(),
            resources: import(),
            config: hosted_config(),
            listen: loopback(),
            port: 0,
        },
        None,
        Some(Forward::with_config(mapper)),
    )
    .expect("the host starts");
    let mut reports = Vec::new();
    assert!(wait_for_reports(
        &mut thread,
        &mut reports,
        Duration::from_secs(10),
        |r| r.iter().any(|r| matches!(r, Report::Forward(_)))
    ));
    let notes = thread.take_notes();
    assert_eq!(notes.len(), 1);
    assert!(
        notes[0].starts_with("Your router refused to forward the port"),
        "{notes:?}"
    );
    assert!(notes[0].ends_with("may need the relay."), "{notes:?}");
    assert!(reports.iter().any(|r| matches!(r, Report::Started { .. })));
    assert!(thread.stop(JOIN_LIMIT));
}
