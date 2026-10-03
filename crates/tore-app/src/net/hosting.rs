//! The host inside the game (slice EF3 of stage E): the hosting player's game
//! runs the stage D `tore_session::Host` on a thread of its own, with its
//! own fixed 120 Hz clock and its UDP socket on the game port, exactly as the
//! dedicated server runs it, and joins it as an ordinary client over the
//! in-process link (`tore_net::link`). See docs/ARCHITECTURE.md, "The host
//! inside the game (stage E)".
//!
//! The thread builds the `Host` itself, so the window never waits for the
//! mission to build, and then runs the dedicated server's loop: receive,
//! update, transmit, wait until the next wake (`tore_net::wait_until`, which
//! sleeps and then spins). The game talks to it through two channels:
//! [`Command`]s in and [`Report`]s out. Every choice here is an agent
//! decision unless it is credited.
//!
//! - **Stop.** [`Command::Stop`] is the hosting player leaving the game
//!   (EF4): a flying mission ends for everyone with "the host left the
//!   game" and their debriefs, every player is told the host left and is
//!   disconnected once that is acknowledged. The thread waits at most
//!   [`STOP_GRACE`] for that, then disconnects whoever is left with "server
//!   stopping", closes its socket and ends; the game waits at most
//!   [`JOIN_LIMIT`] for it. A game that drops its end of the channel stops
//!   the thread the same way.
//! - **The lobby** (EF4). The hosting player's own connection is the King
//!   ([`HostConfig::king`] is the link's address), so the King's verbs
//!   (change the mission, start, end the mission, kick) go over that
//!   connection as messages, as a remote King's would in phase 2, and not
//!   through [`Command`].
//! - **Panic.** A panic on the thread is caught: it tries once to disconnect
//!   every player with "server stopping" (best effort: the host may be
//!   broken), closes the socket and reports [`End::Panicked`], which the game
//!   shows as "The game you were hosting stopped: ...". A remote player the
//!   disconnect does not reach sees the client's own timeout message.
//! - **The port** is closed before the thread reports its end, so a new host
//!   on the same port binds at once.
//! - **On time on macOS** (EF-M). The thread holds an `NSProcessInfo`
//!   activity, latency-critical and user-initiated, from its start to its
//!   end, so App Nap does not slow a hidden hosting game, and once the
//!   mission is built it gives itself a Mach time-constraint policy for the
//!   120 Hz tick (`tore_realtime_native`), whose timers macOS does not
//!   coalesce. It logs once what took ("Host: macOS real-time scheduling
//!   on", or why not). Both do nothing on Linux and Windows.
use crate::net::{
    options::HostOptions,
    session::{Join, NetSession, Transport, build_id},
};
use std::{
    any::Any,
    collections::BTreeMap,
    net::SocketAddr,
    panic::{self, AssertUnwindSafe},
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tore_net::{
    LINK_ADDRESS, LinkEnd, Linked, Listen, MAX_NAP, RealClock, SPIN_MARGIN, ServerSocket,
    wait_until,
};
use tore_realtime_native::{Activity, real_time_thread, summary};
use tore_session::{
    AfterEnd, Host, HostConfig, HostLog, LeaveReason, OpenPlanes, Phase, StartMode,
    host::TICKS_PER_SECOND, wire::messages::EndReason,
};
use tore_world::mission::MissionSpec;

/// How long the thread waits, after a stop, for the players to acknowledge
/// their debriefs before it disconnects them anyway.
pub const STOP_GRACE: Duration = Duration::from_millis(1500);
/// How long the game waits for the thread to end after asking it to stop.
pub const JOIN_LIMIT: Duration = Duration::from_secs(3);
/// The thread's stack: a main thread's 8 MiB, as the dedicated server's host
/// has, not a spawned thread's 2 MiB.
const STACK: usize = 8 << 20;

/// What the thread builds the host from.
pub struct HostSetup {
    pub spec: MissionSpec,
    /// The import, which the host builds the mission from.
    pub resources: Arc<BTreeMap<String, Vec<u8>>>,
    pub config: HostConfig,
    /// Where the socket listens.
    pub listen: Listen,
    pub port: u16,
}

/// What the game asks of the thread. The lobby's verbs are the King's
/// messages over the game's own connection, not commands.
#[derive(Debug)]
pub enum Command {
    /// The hosting player leaves: end the game for everyone, politely, and
    /// stop.
    Stop,
    /// Panic on the thread, for the tests of the panic rule.
    #[cfg(test)]
    Panic,
}

/// What the thread tells the game, oldest first.
#[derive(Clone, Debug, PartialEq)]
pub enum Report {
    /// The mission is built and the host takes joins.
    Started { aircraft: usize, capacity: usize },
    /// The host's log: joins, refusals, seats, departures, the mission's
    /// start and end, overloads and faults.
    Log(HostLog),
    /// The host's phase changed.
    Phase(Phase),
    /// A socket error, noted and survived, as the dedicated server notes it.
    Note(String),
    /// The thread has ended, and why. Nothing follows.
    Ended(End),
}

/// Why the thread ended.
#[derive(Clone, Debug, PartialEq)]
pub enum End {
    /// The game asked it to stop.
    Stopped,
    /// The host stopped by itself: its mission ended.
    Finished,
    /// The mission could not be built, or the settings were refused.
    BuildFailed(String),
    /// The thread panicked, with the panic's message.
    Panicked(String),
}

impl End {
    /// What the hosting player is told when the end is not one their own
    /// session reports: a failed build or a panic.
    pub fn failure(&self) -> Option<String> {
        match self {
            Self::Stopped | Self::Finished => None,
            Self::BuildFailed(text) => Some(format!("The game could not be hosted: {text}")),
            Self::Panicked(text) => Some(format!("The game you were hosting stopped: {text}")),
        }
    }
}

/// The host thread, as the game holds it. Dropping it stops the thread.
pub struct HostThread {
    commands: Sender<Command>,
    reports: Receiver<Report>,
    handle: Option<JoinHandle<()>>,
    addresses: Vec<SocketAddr>,
    end: Option<End>,
}

impl HostThread {
    /// Binds the socket, so a port in use is refused at once, and starts the
    /// thread, which builds the host. Returns the game's end of the link.
    pub fn start(setup: HostSetup) -> Result<(Self, LinkEnd), String> {
        let HostSetup {
            spec,
            resources,
            config,
            listen,
            port,
        } = setup;
        let socket = ServerSocket::bind(listen, port).map_err(|error| {
            format!(
                "Cannot host on UDP port {port}: {error}. Is another game or server using it? \
                 Choose another with --port."
            )
        })?;
        let addresses = socket.local_addresses();
        let (host_end, game_end) = tore_net::link::pair();
        let (command_sender, commands) = mpsc::channel();
        let (report_sender, reports) = mpsc::channel();
        let transport = Linked::new(socket, host_end);
        let handle = thread::Builder::new()
            .name("tore-host".into())
            .stack_size(STACK)
            .spawn(move || run(spec, resources, config, transport, commands, report_sender))
            .map_err(|error| format!("Cannot start the host: {error}"))?;
        Ok((
            Self {
                commands: command_sender,
                reports,
                handle: Some(handle),
                addresses,
                end: None,
            },
            game_end,
        ))
    }

    /// The addresses the socket listens on.
    pub fn addresses(&self) -> &[SocketAddr] {
        &self.addresses
    }

    /// Sends `command`; false when the thread has ended.
    // The lobby (EF4) sends its verbs through this; until then only the
    // tests do.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn send(&self, command: Command) -> bool {
        self.commands.send(command).is_ok()
    }

    /// Every report since the last call, each also written to the game's
    /// log.
    pub fn poll(&mut self) -> Vec<Report> {
        let mut reports = Vec::new();
        loop {
            match self.reports.try_recv() {
                Ok(report) => {
                    log_report(&report);
                    if let Report::Ended(end) = &report {
                        self.end = Some(end.clone());
                    }
                    reports.push(report);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // A thread that ends always says why first; this is a
                    // thread that could not even do that.
                    if self.end.is_none() {
                        let end = End::Panicked("the host thread ended without a word".into());
                        let report = Report::Ended(end.clone());
                        log_report(&report);
                        self.end = Some(end);
                        reports.push(report);
                    }
                    break;
                }
            }
        }
        reports
    }

    /// Why the thread ended, once it has said.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn end(&self) -> Option<&End> {
        self.end.as_ref()
    }

    /// Asks the thread to stop and waits at most `limit` for it to end. True
    /// when it ended in time (or had ended already); otherwise it is left to
    /// finish on its own.
    pub fn stop(&mut self, limit: Duration) -> bool {
        let Some(handle) = self.handle.take() else {
            return true;
        };
        let _ = self.commands.send(Command::Stop);
        let deadline = Instant::now() + limit;
        while !handle.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(2));
        }
        let ended = handle.is_finished();
        if ended {
            let _ = handle.join();
        } else {
            log::warn!(
                "Host: the host thread did not stop within {} ms; leaving it to finish",
                limit.as_millis()
            );
        }
        self.poll();
        ended
    }
}

impl Drop for HostThread {
    fn drop(&mut self) {
        self.stop(JOIN_LIMIT);
    }
}

/// The thread: serve until stopped, catching a panic.
fn run(
    spec: MissionSpec,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    config: HostConfig,
    mut transport: Linked<ServerSocket>,
    commands: Receiver<Command>,
    reports: Sender<Report>,
) {
    let mut host: Option<Host> = None;
    let served = panic::catch_unwind(AssertUnwindSafe(|| {
        serve(
            &mut host,
            spec,
            resources,
            config,
            &mut transport,
            &commands,
            &reports,
        )
    }));
    let end = match served {
        Ok(end) => end,
        Err(payload) => {
            if let Some(host) = host.as_mut() {
                // Best effort: the host may be broken, so a second panic is
                // caught and ignored.
                let _ = panic::catch_unwind(AssertUnwindSafe(|| {
                    host.stop();
                    let _ = host.transmit(&mut transport);
                }));
            }
            End::Panicked(panic_text(payload.as_ref()))
        }
    };
    // The port is free before the game hears the end.
    drop(host);
    drop(transport);
    let _ = reports.send(Report::Ended(end));
}

fn panic_text(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "an unknown error".into())
}

/// Builds the host and runs the dedicated server's loop until the host stops
/// or the game stops it.
fn serve(
    slot: &mut Option<Host>,
    spec: MissionSpec,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    config: HostConfig,
    transport: &mut Linked<ServerSocket>,
    commands: &Receiver<Command>,
    reports: &Sender<Report>,
) -> End {
    let mut clock = RealClock::new();
    // Held until the thread stops hosting, a panic included.
    let activity = Activity::begin("Hosting a T.O.R.E-Fighters game");
    let host = match Host::new(spec, resources, config) {
        Ok(host) => host,
        Err(error) => return End::BuildFailed(error.to_string()),
    };
    let planes = host.world().roster.planes().len() as u32;
    if let OpenPlanes::List(list) = &host.config().open_planes
        && let Some(plane) = list.iter().find(|plane| **plane >= planes)
    {
        return End::BuildFailed(format!(
            "--open-planes names plane {plane}, but the mission has planes 0 to {}",
            planes.saturating_sub(1)
        ));
    }
    let host = slot.insert(host);
    // After the build: a real-time thread that runs long without blocking is
    // demoted for a while.
    let tick = Duration::from_nanos(1_000_000_000 / TICKS_PER_SECOND);
    if let Some(line) = summary(&real_time_thread(tick), activity.outcome()) {
        log::info!("Host: {line}");
    }
    let capacity = host.status(clock.now()).capacity;
    let _ = reports.send(Report::Started {
        aircraft: planes as usize,
        capacity,
    });
    let mut phase = None;
    let mut stop_by: Option<Duration> = None;
    let measure = std::env::var_os("TORE_PERF_HOST").is_some();
    let mut report_at = clock.now() + Duration::from_secs(1);
    loop {
        let now = clock.now();
        if let Err(error) = host.receive_from(now, transport) {
            let _ = reports.send(Report::Note(format!("receive failed: {error}")));
        }
        host.update(now);
        if let Err(error) = host.transmit(transport) {
            let _ = reports.send(Report::Note(format!("send failed: {error}")));
        }
        forward(host, reports, &mut phase);
        if measure && now >= report_at {
            let status = host.status(now);
            println!(
                "Host performance: tick={} players={} mean_ms={:.3} max_ms={:.3} overloads={}",
                status.tick,
                status.players,
                status.tick_cost_mean.as_secs_f64() * 1000.,
                status.tick_cost_max.as_secs_f64() * 1000.,
                status.overloads,
            );
            report_at = now + Duration::from_secs(1);
        }
        if host.phase() == Phase::Stopped {
            return if stop_by.is_some() {
                End::Stopped
            } else {
                End::Finished
            };
        }
        let mut stop = false;
        loop {
            match commands.try_recv() {
                Ok(Command::Stop) => stop = true,
                #[cfg(test)]
                Ok(Command::Panic) => panic!("a test asked the host thread to panic"),
                Err(TryRecvError::Empty) => break,
                // The game has gone without a word: stop as it would.
                Err(TryRecvError::Disconnected) => {
                    stop = true;
                    break;
                }
            }
        }
        if stop && stop_by.is_none() {
            host.host_left();
            stop_by = Some(now + STOP_GRACE);
            let _ = host.transmit(transport);
            forward(host, reports, &mut phase);
        }
        if stop_by.is_some_and(|by| now >= by) {
            host.stop();
            let _ = host.transmit(transport);
            forward(host, reports, &mut phase);
            return End::Stopped;
        }
        let wake = (now + host.next_wake(now)).min(now + MAX_NAP);
        wait_until(&mut clock, wake, SPIN_MARGIN);
    }
}

/// Sends the host's new log entries and its phase when it changed.
fn forward(host: &mut Host, reports: &Sender<Report>, phase: &mut Option<Phase>) {
    while let Some(entry) = host.poll_log() {
        let _ = reports.send(Report::Log(entry));
    }
    let now = host.phase();
    // The seconds to the next mission count down; only the kind of phase is
    // news.
    let kind = |phase: Phase| std::mem::discriminant(&phase);
    if phase.is_none_or(|before| kind(before) != kind(now)) {
        *phase = Some(now);
        let _ = reports.send(Report::Phase(now));
    }
}

/// The settings of a game a player hosts (EF4). *Agent decisions:* the
/// server guide's defaults, except that the hosting player's own connection
/// is the King, who starts each mission (`StartMode::King`); a mission's
/// end returns everyone to the lobby at once (`after-end restart` with no
/// delay); and a mission nobody flies any more ends at once (no empty
/// timeout), so the lobby returns when the last player leaves the flight.
pub fn config(options: &HostOptions) -> HostConfig {
    let mut config = HostConfig::new(build_id());
    config.name = options.name.clone();
    config.password = options.password.clone();
    config.open_planes = options.open_planes.clone();
    config.king = Some(LINK_ADDRESS);
    config.start = StartMode::King;
    config.after_end = AfterEnd::Restart;
    config.restart_delay = Duration::ZERO;
    config.empty_timeout = Duration::ZERO;
    config.retail_stall_speeds = tore_sim::flight::retail_stall_speeds();
    config
}

/// Writes a report to the game's log.
fn log_report(report: &Report) {
    match report {
        Report::Started { aircraft, capacity } => log::info!(
            "Host: the mission is built ({aircraft} aircraft, {capacity} players at most); taking joins"
        ),
        Report::Log(entry) => log::info!("Host: {}", log_line(entry)),
        Report::Phase(phase) => log::info!("Host: phase {phase:?}"),
        Report::Note(text) => log::warn!("Host: {text}"),
        Report::Ended(end) => match end {
            End::Stopped => log::info!("Host: stopped"),
            End::Finished => log::info!("Host: the mission ended; the host has stopped"),
            End::BuildFailed(text) => log::warn!("Host: could not start: {text}"),
            End::Panicked(text) => log::error!("Host: the host thread panicked: {text}"),
        },
    }
}

/// A host log entry as a line of the game's log, in the dedicated server's
/// words; the hosting player's own connection is "this game".
pub fn log_line(entry: &HostLog) -> String {
    let who = |address: &SocketAddr| {
        if *address == LINK_ADDRESS {
            "this game".to_owned()
        } else {
            address.to_string()
        }
    };
    let text = match entry {
        HostLog::Connected {
            address, callsign, ..
        } => format!("{} joined as {callsign}", who(address)),
        HostLog::Refused {
            address,
            callsign,
            reason,
            ..
        } if callsign.is_empty() => format!("{} refused: {reason}", who(address)),
        HostLog::Refused {
            address,
            callsign,
            reason,
            ..
        } => format!("{} ({callsign}) refused: {reason}", who(address)),
        HostLog::ContentRefused {
            callsign, names, ..
        } => format!("{callsign} refused: content mismatch: {}", names.join(", ")),
        HostLog::SeatRefused {
            callsign, reason, ..
        } => format!("{callsign} was refused a plane: {reason}"),
        HostLog::Seated {
            seat,
            callsign,
            plane,
            ..
        } => format!("seat {seat} {callsign} took plane {plane}"),
        HostLog::Left {
            seat,
            callsign,
            plane,
            reason,
            ..
        } => {
            let seat = seat.map_or_else(String::new, |seat| format!("seat {seat} "));
            let plane = plane.map_or_else(String::new, |plane| format!(" (plane {plane})"));
            let why = match reason {
                LeaveReason::Left => "left".to_owned(),
                LeaveReason::Silent => "no packet for 5 seconds".to_owned(),
                LeaveReason::Kicked => "kicked".to_owned(),
                LeaveReason::MissionEnded => "the mission ended".to_owned(),
                LeaveReason::HostLeft => "the host left the game".to_owned(),
                LeaveReason::Replaced => {
                    "replaced by a new connection from the same address".to_owned()
                }
                LeaveReason::Disconnected(why) => format!("disconnected ({why:?})"),
            };
            format!("{seat}{callsign}{plane} left: {why}")
        }
        HostLog::MissionStarted { .. } => "mission started".to_owned(),
        HostLog::MissionEnded { reason, .. } => format!(
            "mission ended: {}",
            match reason {
                EndReason::EveryoneLeft => "everyone left",
                EndReason::TimeLimit => "the time limit",
                EndReason::ServerStopping => "the host is stopping",
                EndReason::EndedByServer => "ended by the host",
                EndReason::HostLeft => "the host left the game",
            }
        ),
        HostLog::MissionRestarted { .. } => "mission restarted".to_owned(),
        HostLog::Overloaded { ticks_behind, .. } => {
            format!("overloaded: the tick loop is {ticks_behind} ticks behind real time")
        }
        HostLog::Fault { text, .. } => format!("fault: {text}"),
        HostLog::Stopped { .. } => "stopped".to_owned(),
        HostLog::Lobby {
            callsign, event, ..
        } => format!("lobby: {callsign} {event}"),
        HostLog::Chat {
            callsign,
            receiver,
            text,
            heard,
            ..
        } => format!(
            "chat: {callsign} to {} ({heard} heard): {text}",
            tore_session::wire::chat::receiver_label(*receiver).to_ascii_lowercase()
        ),
        HostLog::Stalled { .. } | HostLog::Resumed { .. } => entry.stall_text().unwrap_or_default(),
    };
    format!("tick {}: {text}", entry.tick())
}

impl crate::App {
    /// Starts hosting the game the command line described: the host thread,
    /// then this game's own join over the in-process link.
    pub(crate) fn start_hosting(&mut self, options: HostOptions) {
        if let Err(error) = self.begin_hosting(options, false) {
            self.message(error);
        }
    }

    /// Starts hosting `options`, naming why it could not (a port in use, a
    /// mission that does not build) in plain words. The Direct Connection
    /// screen's New calls this (with `lobby`, so its lobby screen drives the
    /// game, [`Join::lobby`]) and shows the reason in its Messages.
    pub(crate) fn begin_hosting(
        &mut self,
        options: HostOptions,
        lobby: bool,
    ) -> Result<(), String> {
        let data = match crate::assets::data_directory() {
            Ok(data) => data,
            Err(error) => {
                self.error = Some(error);
                return Ok(());
            }
        };
        crate::net::settings::remember_host(&data, &options);
        let resources = Arc::clone(&self.theater_resources);
        let (thread, link) = HostThread::start(HostSetup {
            spec: options.spec.clone(),
            resources: Arc::clone(&resources),
            config: config(&options),
            listen: Listen::Any,
            port: options.port,
        })?;
        let listening: Vec<String> = thread.addresses().iter().map(ToString::to_string).collect();
        log::info!(
            "Host: hosting {} from {} on UDP {}",
            options.name,
            options.mission.display(),
            listening.join(" and ")
        );
        let join = Join {
            server: LINK_ADDRESS,
            transport: Transport::Link(link),
            callsign: options.callsign.clone(),
            slot: options.slot,
            password: options.password.clone().unwrap_or_default(),
            label: "hosted".into(),
            lobby,
        };
        // Dropping the thread, on an error, stops it.
        let mut session = NetSession::start(join, resources, &data, self.replay_library.as_ref())?;
        session.hosting = Some(thread);
        self.net = Some(session);
        self.message(format!(
            "Hosting {} on UDP port {}...",
            options.name, options.port
        ));
        Ok(())
    }
}

#[cfg(test)]
#[path = "hosting_tests.rs"]
mod tests;
