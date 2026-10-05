//! The real loop: the master on two UDP ports.
//!
//! One thread. Each turn reads at most 1,024 datagrams from each port, hands
//! each to [`Master::receive`], calls [`Master::update`], sends what the
//! master queued from the port it names, and writes the log lines, the
//! status line and the state files when they are due. A turn that had nothing
//! to do sleeps a millisecond (agent decision in the design, measured in I2:
//! the loop then costs well under 1 percent of a core when idle, and a
//! datagram waits at most that millisecond).
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
/// A turn with nothing to do sleeps this long.
pub const IDLE_SLEEP: Duration = Duration::from_millis(1);

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

    /// One turn of the loop. Returns true when it received anything.
    pub fn turn(&mut self, out: &mut dyn Write) -> io::Result<bool> {
        let now = self.clock.now();
        let mut read = self
            .master
            .receive_from(now, MasterPort::Main, &mut self.main)?;
        if let Some(probe) = &mut self.probe {
            read += self.master.receive_from(now, MasterPort::Probe, probe)?;
        }
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
        out.flush()?;
        Ok(read > 0)
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

    /// Turns until `stop` is set or the console says `quit`, sleeping a
    /// millisecond after an idle turn. `console` brings the lines typed on
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
            if !self.turn(out)? {
                std::thread::sleep(IDLE_SLEEP);
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
