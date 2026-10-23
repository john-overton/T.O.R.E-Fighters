//! The real loop: the master on two UDP ports.
//!
//! One thread. Each turn reads at most 1,024 datagrams from each port, hands
//! each to [`Master::receive`], calls [`Master::update`], sends what the
//! master queued from the port it names, and writes the log lines, the
//! status line and the state files when they are due. A turn that had nothing
//! to do sleeps a millisecond while the master is busy (it read a datagram or
//! held a relay channel within the last second), so a datagram or relay frame
//! waits at most that millisecond; once idle for a second it sleeps ten
//! milliseconds and runs its timers every 25 ([`Pacer`]), so the first
//! datagram after a quiet spell waits at most ten, well inside a client's
//! 250 ms handshake retry (optimization run S2).
//!
//! The loop stops when its stop flag is set: by the console's `quit`, or by
//! a test. The standard library cannot catch Ctrl+C or SIGTERM without
//! unsafe code, which the workspace forbids, so those end the process at
//! once (agent decision): the telemetry counts and the minute table are
//! written every minute, so a stop loses at most a minute of them.

use std::io::{self, Write};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use tore_net::{Entropy, RealClock, ServerSocket};

use crate::config::Config;
use crate::log::{self, StateFiles};
use crate::master::{Master, MasterPort};
use crate::stats::Interval;

/// The minute table and the telemetry file are written this often.
pub const FILE_INTERVAL: Duration = Duration::from_secs(60);
/// A turn with nothing to do sleeps this long while the master is busy: it
/// read a datagram or held a relay channel within [`BUSY_WINDOW`].
pub const BUSY_SLEEP: Duration = Duration::from_millis(1);
/// A turn with nothing to do sleeps this long once the master has been idle
/// for [`BUSY_WINDOW`]: the first datagram after a quiet spell waits at most
/// this long, well inside a client's 250 ms handshake retry.
pub const IDLE_SLEEP: Duration = Duration::from_millis(10);
/// How long after a datagram or an open relay channel the master counts as
/// busy.
pub const BUSY_WINDOW: Duration = Duration::from_secs(1);
/// While idle, the timers (listing expiry, introductions, relay checks), the
/// status line and the file writes run this often rather than each turn.
pub const IDLE_HOUSEKEEPING: Duration = Duration::from_millis(25);

/// How briskly the loop turns: every millisecond while busy, every ten and
/// with housekeeping every 25 when idle. A pure value on the master's clock
/// so it can be tested.
#[derive(Clone, Copy, Debug)]
pub struct Pacer {
    last_busy: Option<Duration>,
    next_housekeeping: Duration,
}

impl Default for Pacer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pacer {
    pub const fn new() -> Self {
        Self {
            last_busy: None,
            next_housekeeping: Duration::ZERO,
        }
    }

    /// Notes a turn at `now`: `busy` when it read a datagram or a relay
    /// channel is open.
    pub fn note(&mut self, now: Duration, busy: bool) {
        if busy {
            self.last_busy = Some(now);
        }
    }

    /// Whether the master was busy within [`BUSY_WINDOW`] of `now`.
    pub fn busy(&self, now: Duration) -> bool {
        self.last_busy
            .is_some_and(|at| now.saturating_sub(at) < BUSY_WINDOW)
    }

    /// Whether this turn runs the timers and the rest: always when it read
    /// something or the master is busy, else every [`IDLE_HOUSEKEEPING`].
    pub fn housekeeping_due(&self, now: Duration, read: bool) -> bool {
        read || self.busy(now) || now >= self.next_housekeeping
    }

    /// Notes that housekeeping ran at `now`.
    pub fn housekept(&mut self, now: Duration) {
        self.next_housekeeping = now + IDLE_HOUSEKEEPING;
    }

    /// How long a turn at `now` that read nothing sleeps.
    pub fn sleep(&self, now: Duration) -> Duration {
        if self.busy(now) {
            BUSY_SLEEP
        } else {
            IDLE_SLEEP
        }
    }
}

/// The build: the version and the commit it was built from.
pub fn version() -> String {
    format!(
        "tore-master {} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("TORE_BUILD_COMMIT")
    )
}

/// A master on its sockets.
pub struct Running {
    master: Master,
    main: ServerSocket,
    probe: Option<ServerSocket>,
    clock: RealClock,
    config: Config,
    files: Option<StateFiles>,
    status: Interval,
    minute: Interval,
    asked: Interval,
    next_status: Option<Duration>,
    next_files: Duration,
    pacer: Pacer,
}

impl Running {
    /// Binds both ports and makes the master. With `files`, the state folder
    /// is opened (and today's telemetry resumed); a folder that cannot be
    /// written is reported in the returned lines and the master runs without
    /// it.
    pub fn bind(config: Config, entropy: Entropy, files: bool) -> io::Result<(Self, Vec<String>)> {
        let mut notes = Vec::new();
        let main = ServerSocket::bind(config.listen, config.port)?;
        let probe = match config.probe_port {
            0 => None,
            port => Some(ServerSocket::bind(config.listen, port)?),
        };
        let today = log::day_of(log::unix_seconds());
        let month = log::month_of(today);
        let mut master = Master::new(config.settings.clone(), entropy, today);
        master.relays_mut().roll_month(month);
        let files = if files {
            match StateFiles::open(&config.state_dir) {
                Ok(files) => {
                    if let Some(text) = files.read_telemetry(today) {
                        master.telemetry_mut().resume(&text);
                        notes.push("telemetry: carrying on with today's counts".into());
                    }
                    let relayed = files.read_relay_month(month).unwrap_or(0);
                    master.relays_mut().resume_month(month, relayed);
                    notes.push(format!(
                        "relay this month ({:04}-{:02}): {} of {} GB relayed",
                        month.0,
                        month.1,
                        crate::stats::bytes(relayed),
                        config.settings.relay.month_gb
                    ));
                    Some(files)
                }
                Err(error) => {
                    notes.push(format!(
                        "state folder {} cannot be written ({error}): no statistics or telemetry files",
                        config.state_dir.display()
                    ));
                    None
                }
            }
        } else {
            None
        };
        let clock = RealClock::new();
        let now = clock.now();
        let status = Interval::new(&master, now);
        let next_status = (config.status_interval > 0)
            .then(|| now + Duration::from_secs(u64::from(config.status_interval)));
        Ok((
            Self {
                status,
                minute: status,
                asked: status,
                master,
                main,
                probe,
                clock,
                config,
                files,
                next_status,
                next_files: now + FILE_INTERVAL,
                pacer: Pacer::new(),
            },
            notes,
        ))
    }

    /// The main port's addresses.
    pub fn main_addresses(&self) -> Vec<SocketAddr> {
        self.main.local_addresses()
    }

    /// The probe port's addresses (none when it is off).
    pub fn probe_addresses(&self) -> Vec<SocketAddr> {
        self.probe
            .as_ref()
            .map(ServerSocket::local_addresses)
            .unwrap_or_default()
    }

    /// The master.
    pub fn master(&self) -> &Master {
        &self.master
    }

    /// The start lines: the build, the sockets, the settings.
    pub fn start_lines(&self) -> Vec<String> {
        let mut lines = vec![version()];
        let list = |addresses: Vec<SocketAddr>| {
            addresses
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        lines.push(format!("main port on {}", list(self.main_addresses())));
        if self.probe.is_some() {
            lines.push(format!("probe port on {}", list(self.probe_addresses())));
        }
        lines.extend(self.config.describe().into_iter().skip(2));
        lines
    }

    /// The status line now, over the time since the console last asked
    /// (the periodic line keeps its own interval).
    pub fn status_line(&mut self) -> String {
        let now = self.clock.now();
        self.asked.take(&self.master, now).line()
    }

    /// One listing line each: id, address, name, players, phase.
    pub fn listing_lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .master
            .listings()
            .iter()
            .map(|l| {
                format!(
                    "listing id={:016x} from={} name={:?} players={}/{} phase={:?}",
                    l.id,
                    l.address,
                    l.summary.name,
                    l.summary.players,
                    l.summary.capacity,
                    l.summary.phase
                )
            })
            .collect();
        lines.sort();
        lines.push(format!("listings={}", self.master.listings().len()));
        lines
    }

    /// Reads what is waiting on both ports. Returns how many datagrams.
    fn receive(&mut self, now: Duration) -> io::Result<usize> {
        let mut read = self
            .master
            .receive_from(now, MasterPort::Main, &mut self.main)?;
        if let Some(probe) = &mut self.probe {
            read += self.master.receive_from(now, MasterPort::Probe, probe)?;
        }
        Ok(read)
    }

    /// One turn of the loop. Returns true when it received anything.
    pub fn turn(&mut self, out: &mut dyn Write) -> io::Result<bool> {
        let now = self.clock.now();
        let read = self.receive(now)?;
        self.housekeeping(now, out)?;
        Ok(read > 0)
    }

    /// One paced turn of [`Running::run`]: reads, runs the timers, the sends,
    /// the status line and the file writes when due (every turn while busy,
    /// else every [`IDLE_HOUSEKEEPING`]), and returns how long to sleep: none
    /// after a turn that read something.
    pub fn step(&mut self, out: &mut dyn Write) -> io::Result<Duration> {
        let now = self.clock.now();
        let read = self.receive(now)?;
        let busy = read > 0 || self.master.relays().channels() > 0;
        self.pacer.note(now, busy);
        if self.pacer.housekeeping_due(now, read > 0) {
            self.housekeeping(now, out)?;
            self.pacer.housekept(now);
        }
        Ok(if read > 0 {
            Duration::ZERO
        } else {
            self.pacer.sleep(now)
        })
    }

    /// The timers, the sends, the log, the status line and the files.
    fn housekeeping(&mut self, now: Duration, out: &mut dyn Write) -> io::Result<()> {
        self.master.update(now);
        // A send that fails (a peer's network unreachable) is the network's
        // loss, not the master's: it carries on.
        let _ = self.master.transmit(&mut self.main, self.probe.as_mut());
        while let Some(line) = self.master.poll_log() {
            writeln!(out, "{line}")?;
        }
        if self.next_status.is_some_and(|at| now >= at) {
            let line = self.status.take(&self.master, now).line();
            writeln!(out, "{line}")?;
            self.next_status =
                Some(now + Duration::from_secs(u64::from(self.config.status_interval)));
        }
        if now >= self.next_files {
            self.next_files = now + FILE_INTERVAL;
            self.write_files(now, out)?;
        }
        out.flush()
    }

    /// The minute table's row, the telemetry file, the day's change.
    fn write_files(&mut self, now: Duration, out: &mut dyn Write) -> io::Result<()> {
        let status = self.minute.take(&self.master, now);
        let unix = log::unix_seconds();
        let today = log::day_of(unix);
        let Some(files) = &self.files else {
            return Ok(());
        };
        let mut problems = Vec::new();
        if let Err(error) = files.append_stats(unix, &status) {
            problems.push(format!("statistics file: {error}"));
        }
        // The relay's month figure: a finished month's last figure, then
        // the current one.
        let relays = self.master.relays_mut();
        if let Some((finished, bytes)) = relays.roll_month(log::month_of(today))
            && let Err(error) = files.write_relay_month(finished, bytes)
        {
            problems.push(format!("relay file: {error}"));
        }
        let relays = self.master.relays();
        if let Some(month) = relays.month()
            && let Err(error) = files.write_relay_month(month, relays.month_bytes())
        {
            problems.push(format!("relay file: {error}"));
        }
        let telemetry = self.master.telemetry_mut();
        let counted = telemetry.day();
        if let Some(finished) = telemetry.roll(today) {
            if let Err(error) = files.write_telemetry(counted, &finished) {
                problems.push(format!("telemetry file: {error}"));
            }
            match files.prune_stats(today) {
                Ok(0) => {}
                Ok(n) => writeln!(out, "deleted {n} statistics files older than 90 days")?,
                Err(error) => problems.push(format!("statistics files: {error}")),
            }
        }
        let text = self.master.telemetry().to_tsv();
        if let Err(error) = files.write_telemetry(today, &text) {
            problems.push(format!("telemetry file: {error}"));
        }
        for problem in problems {
            writeln!(out, "{problem}")?;
        }
        Ok(())
    }

    /// Closes the relay's channels (both ends told) and writes the
    /// telemetry counts and the relay's month figure, as the master stops.
    pub fn finish(&mut self, out: &mut dyn Write) -> io::Result<()> {
        self.master.stop();
        let _ = self.master.transmit(&mut self.main, self.probe.as_mut());
        while let Some(line) = self.master.poll_log() {
            writeln!(out, "{line}")?;
        }
        if let Some(files) = &self.files {
            let telemetry = self.master.telemetry();
            if let Err(error) = files.write_telemetry(telemetry.day(), &telemetry.to_tsv()) {
                writeln!(out, "telemetry file: {error}")?;
            }
            let relays = self.master.relays();
            if let Some(month) = relays.month()
                && let Err(error) = files.write_relay_month(month, relays.month_bytes())
            {
                writeln!(out, "relay file: {error}")?;
            }
        }
        Ok(())
    }

    /// Turns until `stop` is set or the console says `quit`, sleeping after
    /// a turn that read nothing (a millisecond while busy, ten once idle for
    /// a second; see [`Pacer`]). `console` brings the lines typed on
    /// standard input.
    pub fn run(
        &mut self,
        stop: &AtomicBool,
        console: &Receiver<String>,
        out: &mut dyn Write,
    ) -> io::Result<()> {
        while !stop.load(Ordering::Relaxed) {
            while let Ok(line) = console.try_recv() {
                for answer in self.command(line.trim()) {
                    writeln!(out, "{answer}")?;
                }
                if line.trim() == "quit" {
                    stop.store(true, Ordering::Relaxed);
                }
            }
            let sleep = self.step(out)?;
            if !sleep.is_zero() {
                std::thread::sleep(sleep);
            }
        }
        self.finish(out)
    }

    /// The console's answer to one command.
    pub fn command(&mut self, command: &str) -> Vec<String> {
        match command {
            "" => Vec::new(),
            "status" => vec![self.status_line()],
            "listings" => self.listing_lines(),
            "quit" => vec!["Stopping".into()],
            "help" => vec![CONSOLE_HELP.into()],
            other => vec![format!("unknown command `{other}`; {CONSOLE_HELP}")],
        }
    }
}

/// The console's commands.
pub const CONSOLE_HELP: &str =
    "commands: status (the status line now), listings (every listing), quit";

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_fresh_master_is_idle_and_does_its_first_housekeeping() {
        let pacer = Pacer::new();
        assert!(!pacer.busy(ms(0)));
        assert_eq!(pacer.sleep(ms(0)), IDLE_SLEEP);
        assert!(pacer.housekeeping_due(ms(0), false));
    }

    #[test]
    fn traffic_makes_the_master_busy_for_a_second() {
        let mut pacer = Pacer::new();
        pacer.note(ms(5_000), true);
        assert_eq!(pacer.sleep(ms(5_001)), BUSY_SLEEP);
        assert_eq!(pacer.sleep(ms(5_999)), BUSY_SLEEP);
        assert_eq!(pacer.sleep(ms(6_000)), IDLE_SLEEP);
        // A quiet turn does not extend it.
        pacer.note(ms(5_500), false);
        assert_eq!(pacer.sleep(ms(6_000)), IDLE_SLEEP);
        // An open relay channel keeps it busy turn after turn.
        for t in (6_000..9_000).step_by(100) {
            pacer.note(ms(t), true);
            assert_eq!(pacer.sleep(ms(t)), BUSY_SLEEP);
        }
    }

    #[test]
    fn idle_housekeeping_runs_every_twenty_five_milliseconds() {
        let mut pacer = Pacer::new();
        pacer.note(ms(0), true);
        // Busy: every turn.
        assert!(pacer.housekeeping_due(ms(100), false));
        pacer.housekept(ms(100));
        assert!(pacer.housekeeping_due(ms(101), false));
        // Idle: only when 25 ms have passed, or a datagram was read.
        let idle = ms(10_000);
        pacer.housekept(idle);
        assert!(!pacer.housekeeping_due(idle + ms(10), false));
        assert!(!pacer.housekeeping_due(idle + ms(24), false));
        assert!(pacer.housekeeping_due(idle + ms(25), false));
        assert!(pacer.housekeeping_due(idle + ms(1), true));
    }

    #[test]
    fn the_first_datagram_after_a_quiet_spell_waits_at_most_the_idle_sleep() {
        // The handshake retries after 250 ms; the idle sleep is well inside.
        assert!(IDLE_SLEEP <= ms(10));
        assert!(IDLE_SLEEP * 10 <= ms(250));
        assert!(IDLE_HOUSEKEEPING <= ms(50));
    }
}
