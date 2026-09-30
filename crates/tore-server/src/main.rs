//! `tore-server`: the dedicated server. No window, graphics or sound; see
//! docs/DEDICATED-SERVER.md for every option, setting and command.

mod app;
mod check;
mod clock;
mod config;
mod console;
mod host;
mod importing;
mod log;
mod options;
mod prepare;
mod report;
mod run;
mod socket;
mod wiring;

use std::process::ExitCode;

fn main() -> ExitCode {
    match app::run(std::env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(app::Failure::Usage(message)) => {
            eprintln!("tore-server: {message}");
            ExitCode::from(2)
        }
        Err(app::Failure::Refused(message)) => {
            eprintln!("tore-server: {message}");
            ExitCode::FAILURE
        }
    }
}
