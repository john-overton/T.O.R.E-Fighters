//! The program: options, then import, check or run.

use std::sync::mpsc::Receiver;
use tore_net::ServerSocket;

use crate::{
    check,
    clock::{RealTimer, Timer},
    config::StartMode,
    console,
    host::{Host, HostSetup},
    importing,
    log::Logger,
    options::{Options, USAGE},
    prepare::{self, Prepared},
    run::{Ended, Loop},
    wiring,
};
/// The build's version string, as the game reports it.
pub fn version() -> &'static str {
    option_env!("TORE_BUILD_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// Whether this is a tagged release build, which the join gate compares by
/// version instead of by commit. The rule is that the build script's stamped
/// tag, `TORE_BUILD_VERSION`, was set at compile time: the same test as the
/// game's `version::version()` uses to choose the tag over the crate version.
/// The game's client (slice D8) must use the same rule. `tore_session::BuildId::matches`
/// decides what a release flag means: a release host also takes a game of the
/// same plain version, and any host takes the same commit.
pub fn is_release() -> bool {
    option_env!("TORE_BUILD_VERSION").is_some()
}

/// The build's commit, stamped by the build script.
pub fn commit() -> &'static str {
    env!("TORE_BUILD_COMMIT")
}

fn target() -> &'static str {
    env!("TORE_BUILD_TARGET")
}

/// Prints a line, ignoring a closed pipe (`tore-server --check | head`).
fn say(text: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stdout(), "{text}");
}

/// Why the program stopped without running.
pub enum Failure {
    /// The command line is wrong; exit code 2.
    Usage(String),
    /// Anything else that stops a start; exit code 1.
    Refused(String),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Refused(message)
    }
}

/// Runs the program with the arguments after its name.
pub fn run(args: impl IntoIterator<Item = String>) -> Result<(), Failure> {
    let options = Options::parse(args).map_err(Failure::Usage)?;
    if options.help {
        say(USAGE);
        return Ok(());
    }
    if options.version {
        say(&format!("T.O.R.E server v{} ({})", version(), commit()));
        return Ok(());
    }
    prepare::refuse_retail_stall_speeds(&options, std::env::var_os("TORE_RETAIL_STALL_SPEEDS"))?;
    tore_import::set_log(|level, text| match level {
        tore_import::Level::Info => say(text),
        tore_import::Level::Warn => eprintln!("warning: {text}"),
    });
    let data_dir = match &options.data_dir {
        Some(dir) => dir.clone(),
        None => tore_import::data_directory().map_err(|error| error.to_string())?,
    };

    if let Some(folder) = &options.import {
        let lines = importing::import(folder, &data_dir, &mut say)?;
        for line in lines {
            say(&line);
        }
        return Ok(());
    }

    let prepared = prepare::prepare(&options, &data_dir)?;
    if options.check {
        for line in check::report(&prepared) {
            say(&line);
        }
        return Ok(());
    }
    serve(prepared).map_err(Failure::Refused)
}

/// Opens the port, starts the host and runs until the console quits or the
/// last mission ends.
fn serve(prepared: Prepared) -> Result<(), String> {
    let data_dir = prepared.data_dir.clone();
    let logger = Logger::new(data_dir.join("logs"), Box::new(std::io::stdout()));
    serve_with(
        prepared,
        wiring::start_host,
        console::spawn_stdin(),
        &mut RealTimer::new(),
        logger,
    )
}

/// [`serve`] with the host's builder, the console, the clock and the log
/// given, so a test can drive the whole start-up with a scripted host.
pub fn serve_with(
    prepared: Prepared,
    start_host: impl FnOnce(HostSetup) -> Result<Box<dyn Host>, String>,
    commands: Receiver<console::Command>,
    timer: &mut dyn Timer,
    mut log: Logger,
) -> Result<(), String> {
    let Prepared {
        config,
        data_dir,
        spec,
        resources,
        world,
        ..
    } = prepared;
    let aircraft = world.roster.planes().len();
    drop(world);

    let socket = ServerSocket::bind(config.address, config.port).map_err(|error| {
        format!(
            "Cannot listen on {}: {error}. Is another server using the port? Change it with the `port` setting or --port.",
            config.listen_description()
        )
    })?;
    let listening: Vec<String> = socket
        .local_addresses()
        .iter()
        .map(|address| address.to_string())
        .collect();
    let start_lines = [
        format!(
            "T.O.R.E server v{} (commit {}, {})",
            version(),
            commit().chars().take(9).collect::<String>(),
            target()
        ),
        format!("Server name: {}", config.name),
        format!("Data folder: {}", data_dir.display()),
        check::mission_summary(&spec, aircraft),
        format!("Listening on UDP {}", listening.join(" and ")),
    ];
    let (status_interval, start_mode) = (config.status_interval_seconds, config.start);

    let mut host = start_host(HostSetup {
        config,
        spec,
        resources,
        socket,
        version: version().to_owned(),
        commit: commit().to_owned(),
    })?;
    for line in &start_lines {
        log.log(timer.unix_seconds(), line);
    }
    log.log(
        timer.unix_seconds(),
        match start_mode {
            StartMode::FirstPlayer => "Waiting for players",
            StartMode::Now => "Flying: the mission started without waiting",
        },
    );
    match Loop::new(timer, &mut log, &commands, status_interval).run(host.as_mut()) {
        Ended::Quit | Ended::Finished => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        clock::fake::FakeTimer,
        config::Listen,
        console::Command,
        host::scripted::ScriptedHost,
        log::tests::Shared,
        prepare::tests::{MISSION, data_folder},
    };
    use std::{fs, sync::mpsc::channel};

    #[test]
    fn the_start_lines_the_log_and_quit_with_a_scripted_host() {
        let dir = data_folder("app-serve", true);
        fs::write(dir.join("mission.txt"), MISSION).unwrap();
        let mut prepared = prepare::prepare(&Options::default(), &dir).unwrap();
        prepared.config.address = Listen::Address("127.0.0.1".parse().unwrap());
        prepared.config.port = 0;
        let (sender, commands) = channel();
        sender.send(Command::Quit).unwrap();
        let echo = Shared::default();
        let logger = Logger::new(dir.join("logs"), Box::new(echo.clone()));
        let mut timer = FakeTimer {
            unix: 1_790_771_696,
            ..Default::default()
        };
        let mut received = None;
        serve_with(
            prepared,
            |setup| {
                received = Some((
                    setup.config.clone(),
                    setup.spec.clone(),
                    setup.socket.local_addresses(),
                ));
                Ok(Box::new(ScriptedHost::default()))
            },
            commands,
            &mut timer,
            logger,
        )
        .unwrap();
        let text = echo.text();
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[0].starts_with(&format!("T.O.R.E server v{}", version())),
            "{text}"
        );
        assert_eq!(lines[1], "Server name: T.O.R.E server");
        assert!(lines[2].starts_with("Data folder: "));
        assert!(
            lines[3].starts_with("Mission: UKR (clear), airborne at 10000 ft"),
            "{text}"
        );
        assert!(
            lines[4].starts_with("Listening on UDP 127.0.0.1:"),
            "{text}"
        );
        assert_eq!(lines[5], "Waiting for players");
        assert!(text.contains("console: quit"));
        assert!(lines.last().unwrap().ends_with("Stopped"));
        let (config, spec, sockets) = received.unwrap();
        assert_eq!(config.snapshot_rate, 30);
        assert_eq!(spec.theater, "UKR");
        assert_eq!(sockets.len(), 1);
        let file = fs::read_to_string(dir.join("logs/server-2026-09-30.log")).unwrap();
        assert!(file.contains("12:34:56 Waiting for players"), "{file}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_host_that_cannot_start_stops_the_server_before_it_announces_anything() {
        let dir = data_folder("app-nohost", true);
        fs::write(dir.join("mission.txt"), MISSION).unwrap();
        let mut prepared = prepare::prepare(&Options::default(), &dir).unwrap();
        prepared.config.address = Listen::Address("127.0.0.1".parse().unwrap());
        prepared.config.port = 0;
        let (_keep, commands) = channel();
        let echo = Shared::default();
        let logger = Logger::new(dir.join("logs"), Box::new(echo.clone()));
        let error = serve_with(
            prepared,
            |_| Err("no host".to_owned()),
            commands,
            &mut FakeTimer::default(),
            logger,
        )
        .unwrap_err();
        assert_eq!(error, "no host");
        assert_eq!(echo.text(), "");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_taken_port_is_refused_with_the_setting_to_change() {
        let dir = data_folder("app-busy", true);
        fs::write(dir.join("mission.txt"), MISSION).unwrap();
        let taken = ServerSocket::bind(Listen::Address("127.0.0.1".parse().unwrap()), 0).unwrap();
        let mut prepared = prepare::prepare(&Options::default(), &dir).unwrap();
        prepared.config.address = Listen::Address("127.0.0.1".parse().unwrap());
        prepared.config.port = taken.local_addresses()[0].port();
        let (_keep, commands) = channel();
        let logger = Logger::new(dir.join("logs"), Box::new(Shared::default()));
        let error = serve_with(
            prepared,
            |_| Err("unreachable".to_owned()),
            commands,
            &mut FakeTimer::default(),
            logger,
        )
        .unwrap_err();
        assert!(
            error.starts_with("Cannot listen on UDP 127.0.0.1:"),
            "{error}"
        );
        assert!(error.contains("`port` setting"), "{error}");
        let _ = fs::remove_dir_all(dir);
    }
}
