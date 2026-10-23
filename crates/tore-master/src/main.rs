//! `tore-master`: the master server. See docs/MASTER-SERVER.md.
//!
//! ```text
//! tore-master [--config FILE] [--check-config]
//! tore-master flood TARGET SECONDS [--rate N] [--ports N]
//! tore-master --version
//! ```

use std::io::{self, BufRead, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;

use tore_master::Config;
use tore_master::flood::{FloodOptions, flood};
use tore_master::run::{CONSOLE_HELP, Running, version};
use tore_net::Entropy;

const USAGE: &str = "usage: tore-master [--config FILE] [--check-config]
       tore-master flood TARGET SECONDS [--rate N] [--ports N]
       tore-master --version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    if args.first().map(String::as_str) == Some("flood") {
        return run_flood(&args[1..]);
    }
    let mut config_path: Option<PathBuf> = None;
    let mut check = false;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--config" => {
                config_path = Some(PathBuf::from(rest.next().ok_or("--config needs a file")?))
            }
            "--check-config" => check = true,
            "--version" => {
                println!("{}", version());
                return Ok(ExitCode::SUCCESS);
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(ExitCode::SUCCESS);
            }
            other => return Err(format!("unknown option `{other}`\n{USAGE}")),
        }
    }
    let config = match &config_path {
        Some(path) => {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let base = path.parent().unwrap_or(Path::new("."));
            let base = if base.as_os_str().is_empty() {
                Path::new(".")
            } else {
                base
            };
            Config::parse(&text, base).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => Config::defaults(Path::new(".")),
    };
    if check {
        println!("{}", version());
        for line in config.describe() {
            println!("{line}");
        }
        println!("The configuration is good.");
        return Ok(ExitCode::SUCCESS);
    }
    let (mut running, notes) =
        Running::bind(config, Entropy::System, true).map_err(|e| format!("cannot listen: {e}"))?;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let print = |out: &mut dyn Write, line: &str| {
        let _ = writeln!(out, "{line}");
    };
    for line in running.start_lines().iter().chain(&notes) {
        print(&mut out, line);
    }
    print(&mut out, &format!("Ready ({CONSOLE_HELP})"));
    let _ = out.flush();
    // The console: lines typed on standard input. Under systemd standard
    // input is empty, so the thread ends at once and only a signal stops the
    // master.
    let (send, console) = mpsc::channel();
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if send.send(line).is_err() {
                break;
            }
        }
    });
    let stop = AtomicBool::new(false);
    running
        .run(&stop, &console, &mut out)
        .map_err(|e| format!("the master stopped: {e}"))?;
    print(&mut out, "Stopped");
    Ok(ExitCode::SUCCESS)
}

fn run_flood(args: &[String]) -> Result<ExitCode, String> {
    let mut positional = Vec::new();
    let mut rate = None;
    let mut ports = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--rate" => {
                rate = Some(
                    rest.next()
                        .and_then(|v| v.parse::<u32>().ok())
                        .filter(|&r| (1..=100_000).contains(&r))
                        .ok_or("--rate is a number of datagrams a second, 1 to 100000")?,
                )
            }
            "--ports" => {
                ports = Some(
                    rest.next()
                        .and_then(|v| v.parse::<usize>().ok())
                        .filter(|&p| (1..=1_000).contains(&p))
                        .ok_or("--ports is a number from 1 to 1000")?,
                )
            }
            _ => positional.push(arg.clone()),
        }
    }
    let [target, seconds] = positional.as_slice() else {
        return Err(USAGE.into());
    };
    let target: SocketAddr = target
        .to_socket_addrs()
        .map_err(|e| format!("{target}: {e}"))?
        .next()
        .ok_or_else(|| format!("{target}: no address"))?;
    let seconds: f64 = seconds
        .parse()
        .ok()
        .filter(|s: &f64| (0.1..=3_600.0).contains(s))
        .ok_or("SECONDS is from 0.1 to 3600")?;
    let mut options = FloodOptions::new(target, seconds);
    if let Some(rate) = rate {
        options.rate = rate;
    }
    if let Some(ports) = ports {
        options.ports = ports;
    }
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let report = flood(options, &mut out).map_err(|e| format!("flood: {e}"))?;
    for line in report.lines(&options) {
        let _ = writeln!(out, "{line}");
    }
    Ok(if report.held() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}
