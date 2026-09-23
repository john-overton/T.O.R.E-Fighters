//! Opinionated host diagnostics, specified in docs/spec/startup-diagnostics.md.
use std::backtrace::Backtrace;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const ROUTINE_LIMIT: usize = 5 * 1024 * 1024;
const RECORD_LIMIT: usize = 16 * 1024;
const FATAL_LIMIT: usize = 256 * 1024;
const KEEP: usize = 5;
static LOGGER: OnceLock<Logger> = OnceLock::new();
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
static PANIC_ACTIVE: AtomicBool = AtomicBool::new(false);
static PANIC_SUMMARY: Mutex<Option<String>> = Mutex::new(None);

struct Output {
    file: Option<File>,
    bytes: usize,
    suppressed: bool,
}
struct Logger {
    start: Instant,
    path: Option<PathBuf>,
    fatal_path: Option<PathBuf>,
    output: Mutex<Output>,
    stage: Mutex<Option<(&'static str, Instant)>>,
    fatal_bytes: AtomicUsize,
}

fn timestamp() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}Z", elapsed.as_secs(), elapsed.subsec_millis())
}
fn bounded(text: &str, limit: usize) -> &str {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
fn stderr(text: &str) {
    let _ = writeln!(io::stderr().lock(), "{text}");
}
fn preferred_directory(get: impl Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = get("TORE_LOG_DIR") {
        return Some(path);
    }
    if let Some(path) = get("TORE_DATA_DIR") {
        return Some(path.join("logs"));
    }
    #[cfg(target_os = "windows")]
    return get("LOCALAPPDATA").map(|p| p.join("T.O.R.E-Fighters/logs"));
    #[cfg(target_os = "macos")]
    return get("HOME").map(|p| p.join("Library/Logs/T.O.R.E-Fighters"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    get("XDG_STATE_HOME")
        .or_else(|| get("HOME").map(|p| p.join(".local/state")))
        .map(|p| p.join("T.O.R.E-Fighters/logs"))
}
fn maintenance_lock(directory: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(".tore-maintenance.lock"))?;
    for attempt in 0..5 {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if attempt < 4 => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(io::Error::other(error)),
        }
    }
    Err(io::Error::other("diagnostics maintenance lock unavailable"))
}
fn open_session(directory: &Path) -> io::Result<(PathBuf, File)> {
    let directory = std::path::absolute(directory)?;
    fs::create_dir_all(&directory)?;
    let _maintenance = maintenance_lock(&directory)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for counter in 0..64 {
        let path = directory.join(format!(
            "tore-{stamp:039}-{}-{counter}.log",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                file.try_lock().map_err(io::Error::other)?;
                return Ok((path, file));
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "session names exhausted",
    ))
}
fn retain(directory: &Path, suffix: &str) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_file())
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with("tore-") && name.ends_with(suffix))
        })
        .map(|entry| entry.path())
        .filter(|path| {
            let active = if suffix == ".fatal.txt" {
                path.with_file_name(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .replace(".fatal.txt", ".log"),
                )
            } else {
                path.clone()
            };
            match OpenOptions::new().write(true).open(active) {
                Ok(file) => file.try_lock().is_ok(),
                Err(error) => error.kind() == io::ErrorKind::NotFound,
            }
        })
        .collect();
    files.sort_unstable();
    let excess = files.len().saturating_sub(KEEP);
    for file in files.into_iter().take(excess) {
        let _ = fs::remove_file(file);
    }
}
impl Logger {
    fn write(&self, level: log::Level, message: &str) {
        self.write_record(level, message, level <= log::Level::Warn, false);
    }
    fn write_record(&self, level: log::Level, message: &str, mirror: bool, file_only: bool) {
        let line = format!(
            "{} +{:.3}s {level} {}\n",
            timestamp(),
            self.start.elapsed().as_secs_f64(),
            bounded(message, RECORD_LIMIT)
        );
        // A backend formatter or panic hook may reenter logging. Never wait on it.
        let Ok(mut output) = self.output.try_lock() else {
            return;
        };
        if output.suppressed {
            return;
        }
        if output.bytes + line.len() > ROUTINE_LIMIT - 128 {
            output.suppressed = true;
            if let Some(file) = output.file.as_mut() {
                let _ = file.write_all(b"Routine log limit reached; further routine output suppressed. Fatal reports remain separate.\n");
                let _ = file.flush();
            }
            return;
        }
        output.bytes += line.len();
        if mirror {
            stderr(line.trim_end());
        }
        if let Some(file) = output.file.as_mut() {
            if file
                .write_all(line.as_bytes())
                .and_then(|()| file.flush())
                .is_ok()
            {
                return;
            }
            output.file = None;
            stderr("T.O.R.E: session log write failed; using stderr.");
        }
        if !mirror && !file_only {
            stderr(line.trim_end());
        }
    }
    fn stage_name(&self) -> &'static str {
        self.stage
            .try_lock()
            .ok()
            .and_then(|s| s.as_ref().map(|(name, _)| *name))
            .unwrap_or("startup or running (stage unavailable)")
    }
}
impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
            || (metadata.target().starts_with("tore") && metadata.level() <= log::Level::Info)
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            self.write(
                record.level(),
                &format!("{}: {}", record.target(), record.args()),
            );
        }
    }
    fn flush(&self) {
        if let Ok(mut output) = self.output.try_lock()
            && let Some(file) = output.file.as_mut()
        {
            let _ = file.flush();
        }
    }
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        if PANIC_ACTIVE.swap(true, Ordering::SeqCst) {
            stderr("T.O.R.E: concurrent or recursive panic; additional reporting skipped.");
            return;
        }
        let reason = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        let thread = std::thread::current();
        let summary = format!(
            "Rust panic: {}\nThread: {}\nLocation: {}",
            bounded(reason, RECORD_LIMIT),
            thread.name().unwrap_or("unnamed"),
            info.location()
                .map(|p| p.to_string())
                .unwrap_or_else(|| "unknown".into())
        );
        if let Ok(mut saved) = PANIC_SUMMARY.try_lock() {
            *saved = Some(summary.clone());
        }
        fatal(&format!(
            "{summary}\nBacktrace:\n{}",
            Backtrace::force_capture()
        ));
        PANIC_ACTIVE.store(false, Ordering::SeqCst);
    }));
}

pub fn init() {
    if LOGGER.get().is_some() {
        return;
    }
    install_panic_hook();
    let preferred = preferred_directory(|key| std::env::var_os(key).map(PathBuf::from));
    let mut failures = Vec::new();
    let mut opened = None;
    for directory in preferred.into_iter().chain(std::iter::once(
        std::env::temp_dir().join("T.O.R.E-Fighters/logs"),
    )) {
        match open_session(&directory) {
            Ok(session) => {
                opened = Some(session);
                break;
            }
            Err(error) => failures.push(format!(
                "Log directory {} unavailable: {error}",
                directory.display()
            )),
        }
    }
    let (path, file) = match opened {
        Some((path, file)) => (Some(path), Some(file)),
        None => (None, None),
    };
    let fatal_path = path.as_ref().map(|p| p.with_extension("fatal.txt"));
    let logger = Logger {
        start: Instant::now(),
        path,
        fatal_path,
        output: Mutex::new(Output {
            file,
            bytes: 0,
            suppressed: false,
        }),
        stage: Mutex::new(None),
        fatal_bytes: AtomicUsize::new(0),
    };
    if LOGGER.set(logger).is_err() {
        return;
    }
    let Some(logger) = LOGGER.get() else { return };
    let _ = log::set_logger(logger);
    log::set_max_level(log::LevelFilter::Info);
    logger.write(log::Level::Info, &format!("Session started: version={} commit={} target={} OS={} architecture={} executable={:?} working_directory={:?} log={:?}", crate::version::version(), crate::version::commit(), crate::version::target(), std::env::consts::OS, std::env::consts::ARCH, std::env::current_exe(), std::env::current_dir(), logger.path));
    for failure in failures {
        logger.write(log::Level::Warn, &failure);
    }
    match crate::assets::data_directory()
        .and_then(|path| std::path::absolute(path).map_err(Into::into))
    {
        Ok(path) => {
            logger.write(
                log::Level::Info,
                &format!("Data directory: {}", path.display()),
            );
            let _ = DATA_DIR.set(path);
        }
        Err(error) => logger.write(
            log::Level::Warn,
            &format!("Data directory unavailable: {error}"),
        ),
    }
    if let Some(directory) = logger.path.as_ref().and_then(|p| p.parent())
        && let Ok(_maintenance) = maintenance_lock(directory)
    {
        retain(directory, ".log");
        retain(directory, ".fatal.txt");
    }
}
pub fn log_path() -> Option<PathBuf> {
    LOGGER.get().and_then(|logger| logger.path.clone())
}
pub fn stage(name: &'static str) {
    if let Some(logger) = LOGGER.get() {
        if let Ok(mut stage) = logger.stage.try_lock() {
            *stage = Some((name, Instant::now()));
        }
        logger.write(log::Level::Info, &format!("Stage started: {name}"));
    }
}
pub fn stage_done() {
    if let Some(logger) = LOGGER.get() {
        let stage = logger.stage.try_lock().ok().and_then(|mut s| s.take());
        if let Some((name, started)) = stage {
            logger.write(
                log::Level::Info,
                &format!(
                    "Stage completed: {name} ({:.3}s)",
                    started.elapsed().as_secs_f64()
                ),
            );
        }
    }
}
pub fn finish_success() {
    stage_done();
    if let Some(logger) = LOGGER.get() {
        logger.write(log::Level::Info, "Session completed successfully");
        if let Ok(mut output) = logger.output.try_lock() {
            output.file.take();
        }
        if let Some(directory) = logger.path.as_ref().and_then(|path| path.parent())
            && let Ok(_maintenance) = maintenance_lock(directory)
        {
            retain(directory, ".log");
            retain(directory, ".fatal.txt");
        }
    }
}
fn fatal(reason: &str) -> String {
    let Some(logger) = LOGGER.get() else {
        let report = format!(
            "{} T.O.R.E-Fighters {} commit={} target={}\nStage: diagnostics initialization\n{}\nNo diagnostic log could be saved.",
            timestamp(),
            crate::version::version(),
            crate::version::commit(),
            crate::version::target(),
            bounded(reason, 32 * 1024)
        );
        stderr(&report);
        let _ = tore_diagnostics_native::event_error(&report);
        return "No diagnostic log could be saved.".into();
    };
    let report = format!(
        "{}\nT.O.R.E-Fighters {} commit={} target={}\nStage: {}\n{}\nSession log: {}\n",
        timestamp(),
        crate::version::version(),
        crate::version::commit(),
        crate::version::target(),
        logger.stage_name(),
        bounded(reason, 32 * 1024),
        logger
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "unavailable".into())
    );
    logger.write_record(log::Level::Error, &report, false, true);
    stderr(&report);
    let previous = logger
        .fatal_bytes
        .fetch_add(report.len(), Ordering::Relaxed);
    let saved = previous.saturating_add(report.len()) <= FATAL_LIMIT
        && logger.fatal_path.as_ref().is_some_and(|path| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut file| {
                    file.write_all(report.as_bytes())?;
                    file.sync_data()
                })
                .is_ok()
        });
    let mut location = if saved {
        format!(
            "Fatal report: {}",
            logger
                .fatal_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        )
    } else {
        "Fatal report could not be saved; see stderr and the session log if available.".into()
    };
    if let Some(directory) = DATA_DIR.get() {
        let path = directory.join("last-error.txt");
        // The compatibility summary uses the same atomic replacement as settings,
        // so concurrent launches cannot leave a partially overwritten report.
        match crate::preferences::write(&path, &format!("{report}{location}\n")) {
            Ok(()) if !saved => {
                location = format!(
                    "Error summary: {}\nThe separate fatal log could not be saved.",
                    path.display()
                );
            }
            Ok(()) => {}
            Err(error) => logger.write(
                log::Level::Warn,
                &format!("Could not save {}: {error}", path.display()),
            ),
        }
    }
    let event = format!(
        "T.O.R.E-Fighters {} commit={} target={}\nStage: {}\n{}\nSession log: {}\nReason: {}",
        crate::version::version(),
        crate::version::commit(),
        crate::version::target(),
        logger.stage_name(),
        location,
        logger
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "unavailable".into()),
        bounded(reason, 4096)
    );
    let _ = tore_diagnostics_native::event_error(&event);
    location
}
fn display(summary: &str, location: &str, interactive: bool) {
    if interactive
        && std::env::var_os("TORE_NO_ERROR_DIALOG").as_deref() != Some(std::ffi::OsStr::new("1"))
    {
        let stage = LOGGER.get().map(Logger::stage_name).unwrap_or("startup");
        let message = format!(
            "T.O.R.E-Fighters failed during {stage}.\n\n{}\n\n{location}",
            bounded(summary, 4096)
        );
        if let Err(error) = tore_diagnostics_native::show_error("T.O.R.E-Fighters error", &message)
        {
            stderr(&format!("Error display unavailable: {error}"));
        }
    }
}
pub fn report_error(error: &dyn std::error::Error, interactive: bool) {
    let mut summary = format!("{error}");
    let mut source = error.source();
    for _ in 0..16 {
        let Some(cause) = source else { break };
        summary.push_str(&format!(
            "\nCaused by: {}",
            bounded(&cause.to_string(), RECORD_LIMIT)
        ));
        source = cause.source();
    }
    let location = fatal(&summary);
    display(&summary, &location, interactive);
}
pub fn report_panic(interactive: bool) {
    let summary = PANIC_SUMMARY
        .try_lock()
        .ok()
        .and_then(|s| s.clone())
        .unwrap_or_else(|| "Rust panic. See the fatal report for available details.".into());
    let location = LOGGER
        .get()
        .and_then(|l| l.fatal_path.as_ref())
        .filter(|p| p.is_file())
        .map(|p| format!("Fatal report: {}", p.display()))
        .unwrap_or_else(|| "Fatal report could not be saved; see stderr.".into());
    display(&summary, &location, interactive);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tore-diagnostics-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn override_and_profile_precedence() {
        assert_eq!(
            preferred_directory(|key| match key {
                "TORE_LOG_DIR" => Some("logs".into()),
                "TORE_DATA_DIR" => Some("profile".into()),
                _ => None,
            }),
            Some("logs".into())
        );
        assert_eq!(
            preferred_directory(|key| (key == "TORE_DATA_DIR").then(|| "profile".into())),
            Some(PathBuf::from("profile/logs"))
        );
    }
    #[test]
    fn unique_sessions_and_retention_preserve_active_and_unrelated_files() {
        let dir = directory();
        let (first, a) = open_session(&dir).unwrap();
        let (second, b) = open_session(&dir).unwrap();
        assert_ne!(first, second);
        for i in 0..8 {
            fs::write(dir.join(format!("tore-{i:02}.log")), "test").unwrap();
        }
        fs::write(dir.join("unrelated.log"), "keep").unwrap();
        retain(&dir, ".log");
        assert!(first.exists() && second.exists() && dir.join("unrelated.log").exists());
        assert!(!dir.join("tore-02.log").exists());
        assert!(dir.join("tore-03.log").exists());
        drop((a, b));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn routine_limit_is_bounded_and_does_not_block_reentrant_logging() {
        let dir = directory();
        let (path, file) = open_session(&dir).unwrap();
        let logger = Logger {
            start: Instant::now(),
            path: Some(path.clone()),
            fatal_path: None,
            output: Mutex::new(Output {
                file: Some(file),
                bytes: 0,
                suppressed: false,
            }),
            stage: Mutex::new(None),
            fatal_bytes: AtomicUsize::new(0),
        };
        {
            let _guard = logger.output.lock().unwrap();
            logger.write(log::Level::Error, "reentrant");
        }
        for _ in 0..400 {
            logger.write(log::Level::Info, &"x".repeat(RECORD_LIMIT * 2));
        }
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.len() <= ROUTINE_LIMIT);
        assert_eq!(
            contents
                .matches("further routine output suppressed")
                .count(),
            1
        );
        drop(logger);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn closed_session_paths_remain_stable_and_live_sessions_survive() {
        let dir = directory();
        let (closed, file) = open_session(&dir).unwrap();
        drop(file);
        let (live, live_file) = open_session(&dir).unwrap();
        retain(&dir, ".log");
        assert!(closed.exists());
        assert!(live.exists());
        assert!(closed.is_absolute() && live.is_absolute());
        drop(live_file);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn directory_that_is_a_file_is_a_recoverable_error() {
        let dir = directory();
        let blocked = dir.join("blocked");
        fs::write(&blocked, "occupied").unwrap();
        assert!(open_session(&blocked).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn contended_maintenance_returns_without_waiting_for_owner() {
        let dir = directory();
        let lock = maintenance_lock(&dir).unwrap();
        let started = Instant::now();
        assert!(open_session(&dir).is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        drop(lock);
        assert!(open_session(&dir).is_ok());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn bounded_records_keep_unicode_valid() {
        assert_eq!(bounded("aéz", 2), "a");
    }
}
