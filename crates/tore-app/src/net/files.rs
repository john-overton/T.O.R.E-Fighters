//! The files a networked flight keeps (docs/ARCHITECTURE.md, "Recordings and
//! diagnostics"):
//!
//! - the **diagnostics log**, `logs/net-<date>.tsv` in the data folder, one
//!   tab-separated line per second of the session plus every join, seat,
//!   refusal and drop; a new file starts at UTC midnight, and the lines of
//!   one day's sessions follow each other in it;
//! - the **capture**, `replays/<date>_<time>_NET_<host>.tore-capture`: every
//!   packet the client received and every input it sent.
//!
//! A networked flight records no mission replay (John, 2026-09-30). Both
//! files are kept and pruned by the replays' own auto-delete settings
//! (`replays-v1.conf`): the captures as a list of their own, and the logs as
//! one more list counted in days. The date is UTC because the game has no
//! time-zone data (as the replays and the server's log). Every choice here
//! not named as John's is an agent decision.
use crate::replay::library::{self, Cleanup, Kind, Library, Rule, Settings};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The folder of the diagnostics logs, in the data folder.
pub const LOG_FOLDER: &str = "logs";
const LOG_PREFIX: &str = "net-";
const LOG_EXTENSION: &str = "tsv";
/// A capture file's extension.
pub const CAPTURE_EXTENSION: &str = "tore-capture";
/// A capture file's first bytes, which the client session writes
/// (docs/formats/net-protocol.md, "Captures").
pub const CAPTURE_MAGIC: &[u8; 8] = tore_session::capture::MAGIC;
/// The most a diagnostics line may be before the log drops the rest of it, so
/// a writer that never ends a line cannot grow the buffer without end.
const MAX_LINE: usize = 4096;

const CAPTURE: Kind = Kind {
    extension: CAPTURE_EXTENSION,
    magic: CAPTURE_MAGIC,
    skip_recent: true,
};

/// `net-2026-09-30.tsv`, the log of the UTC day `time` falls in.
pub fn log_name(time: SystemTime) -> String {
    let [year, month, day, ..] = library::utc(time);
    format!("{LOG_PREFIX}{year:04}-{month:02}-{day:02}.{LOG_EXTENSION}")
}

/// The seconds since 1970 that start the day a log name holds, `None` for a
/// name that is not a net log's.
fn log_day(name: &str) -> Option<i64> {
    let date = name
        .strip_prefix(LOG_PREFIX)?
        .strip_suffix(&format!(".{LOG_EXTENSION}"))?;
    let bytes = date.as_bytes();
    if date.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let number = |text: &str| -> Option<i64> {
        text.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| text.parse().ok())?
    };
    let (year, month, day) = (
        number(&date[..4])?,
        number(&date[5..7])?,
        number(&date[8..])?,
    );
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days from the civil date, as `library` counts them.
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400)
}

/// The diagnostics log as a writer for the client session. Whole lines go to
/// the file of the day they are written on, which is created when the first
/// line arrives, so a session that writes nothing leaves no file. A failure
/// to write is remembered and reported once through `log`, and then lines
/// are dropped: diagnostics never end a flight.
pub struct DatedLog {
    folder: PathBuf,
    clock: Box<dyn Fn() -> SystemTime + Send>,
    pending: Vec<u8>,
    open: Option<(String, File)>,
    complained: bool,
}

impl DatedLog {
    /// A log writing into `folder` (`logs` of the data folder).
    pub fn new(folder: PathBuf) -> Self {
        Self::with_clock(folder, Box::new(SystemTime::now))
    }

    pub fn with_clock(folder: PathBuf, clock: Box<dyn Fn() -> SystemTime + Send>) -> Self {
        Self {
            folder,
            clock,
            pending: Vec::new(),
            open: None,
            complained: false,
        }
    }

    fn append(&mut self, line: &[u8]) -> io::Result<()> {
        let name = log_name((self.clock)());
        if self.open.as_ref().is_none_or(|(open, _)| *open != name) {
            fs::create_dir_all(&self.folder)?;
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.folder.join(&name))?;
            self.open = Some((name, file));
        }
        let (_, file) = self.open.as_mut().expect("just opened");
        file.write_all(line)?;
        file.flush()
    }
}

impl Write for DatedLog {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for &byte in bytes {
            if self.pending.len() < MAX_LINE {
                self.pending.push(byte);
            }
            if byte == b'\n' {
                let line = std::mem::take(&mut self.pending);
                if let Err(error) = self.append(&line)
                    && !std::mem::replace(&mut self.complained, true)
                {
                    log::warn!("Network diagnostics log: {error}; later lines are dropped");
                }
            }
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// `2026-09-30_1540_NET_HOST`, the start of a capture's file name: the UTC
/// date and time the session began and the server as it was typed (letters
/// and digits only, at most 16, like a replay's map).
pub fn capture_stem(start: SystemTime, host: &str) -> String {
    format!("{}_NET_{}", library::stamp(start), library::name_part(host))
}

/// A new capture file for a session that started at `start` with `host`, in
/// the recordings folder, created and open for writing. A clash with an
/// earlier session in the same minute adds `-2`, `-3` and so on.
pub fn create_capture(
    library: &Library,
    start: SystemTime,
    host: &str,
) -> io::Result<(PathBuf, File)> {
    let folder = library.folder();
    fs::create_dir_all(&folder)?;
    let stem = capture_stem(start, host);
    for n in 1..10_000 {
        let name = if n == 1 {
            format!("{stem}.{CAPTURE_EXTENSION}")
        } else {
            format!("{stem}-{n}.{CAPTURE_EXTENSION}")
        };
        let path = folder.join(&name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("too many captures share one start minute"))
}

/// Applies the replays' auto-delete rule, if it is on, to the captures and
/// the diagnostics logs: with "keep the last N" the newest N captures and the
/// newest N days of logs stay, with "older than D days" nothing newer does.
/// `protect` is the capture being written; a capture or log changed within
/// the last ten minutes is never removed, as it may belong to a game still
/// running.
pub fn prune(
    library: &Library,
    data: &Path,
    settings: &Settings,
    now: SystemTime,
    protect: &[&Path],
) -> Cleanup {
    let mut result = Cleanup::default();
    let mut remove = |path: PathBuf| match fs::remove_file(&path) {
        Ok(()) => result.deleted.push(path),
        Err(error) => result.failed.push((path, error.to_string())),
    };
    for path in library.plan_files(&CAPTURE, settings, now, protect) {
        remove(path);
    }
    for path in plan_logs(&data.join(LOG_FOLDER), settings, now) {
        remove(path);
    }
    result
}

/// The diagnostics logs the rule would remove, oldest day first.
fn plan_logs(folder: &Path, settings: &Settings, now: SystemTime) -> Vec<PathBuf> {
    if !settings.auto_delete {
        return Vec::new();
    }
    let Ok(dir) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let now_s = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    let mut logs: Vec<(i64, PathBuf)> = dir
        .flatten()
        .filter(|file| file.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|file| {
            let day = log_day(file.file_name().to_str()?)?;
            let recent = file
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_none_or(|age| age.as_secs() < 600);
            (!recent).then(|| (day, file.path()))
        })
        .collect();
    // Newest day first.
    logs.sort_by(|a, b| b.cmp(a));
    match settings.rule {
        Rule::KeepLast => logs
            .into_iter()
            .skip(settings.keep_last as usize)
            .map(|(_, path)| path)
            .collect(),
        Rule::OlderThan => {
            let limit = now_s - i64::from(settings.older_than_days) * 86_400;
            logs.into_iter()
                .filter(|(day, _)| *day < limit)
                .map(|(_, path)| path)
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::tests::TempDir;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// 2026-09-30 12:34:56 UTC.
    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }
    const NOON: u64 = 1_790_771_696;

    #[test]
    fn a_log_name_holds_the_utc_day() {
        assert_eq!(log_name(at(NOON)), "net-2026-09-30.tsv");
        assert_eq!(log_name(at(NOON + 86_400 * 2)), "net-2026-10-02.tsv");
        assert_eq!(log_day("net-1970-01-02.tsv"), Some(86_400));
        assert_eq!(log_day("net-2026-13-01.tsv"), None);
        assert_eq!(log_day("server-2026-09-30.log"), None);
        assert_eq!(log_day("net-2026-09-30.txt"), None);
        assert_eq!(log_day("net-2026-0a-30.tsv"), None);
    }

    #[test]
    fn lines_go_to_the_file_of_the_day_and_a_quiet_session_leaves_none() {
        let dir = TempDir::new("net-log");
        let folder = dir.path().join("logs");
        let now = Arc::new(Mutex::new(NOON));
        let clock = Arc::clone(&now);
        let mut log =
            DatedLog::with_clock(folder.clone(), Box::new(move || at(*clock.lock().unwrap())));
        assert!(!folder.exists());
        // A line written in pieces lands whole.
        log.write_all(b"1\tjoin").unwrap();
        log.write_all(b"\tviper\n2\tseat\n").unwrap();
        *now.lock().unwrap() = NOON + 86_400;
        writeln!(log, "3\tleft").unwrap();
        assert_eq!(
            fs::read_to_string(folder.join("net-2026-09-30.tsv")).unwrap(),
            "1\tjoin\tviper\n2\tseat\n"
        );
        assert_eq!(
            fs::read_to_string(folder.join("net-2026-10-01.tsv")).unwrap(),
            "3\tleft\n"
        );
        // A second session on the same day follows the first.
        let mut again = DatedLog::with_clock(folder.clone(), Box::new(|| at(NOON + 86_400)));
        writeln!(again, "4\tjoin").unwrap();
        assert_eq!(
            fs::read_to_string(folder.join("net-2026-10-01.tsv")).unwrap(),
            "3\tleft\n4\tjoin\n"
        );
    }

    #[test]
    fn a_log_that_cannot_be_written_drops_its_lines_without_failing() {
        let dir = TempDir::new("net-log-blocked");
        // The folder's place holds a file, so it can never be created.
        let blocked = dir.path().join("logs");
        fs::write(&blocked, b"in the way").unwrap();
        let mut log = DatedLog::with_clock(blocked, Box::new(|| at(NOON)));
        assert!(writeln!(log, "1\tjoin").is_ok());
        assert!(writeln!(log, "2\tseat").is_ok());
    }

    #[test]
    fn an_endless_line_is_cut_instead_of_growing_without_end() {
        let dir = TempDir::new("net-log-long");
        let folder = dir.path().join("logs");
        let mut log = DatedLog::with_clock(folder.clone(), Box::new(|| at(NOON)));
        log.write_all(&vec![b'x'; 3 * MAX_LINE]).unwrap();
        assert!(log.pending.len() <= MAX_LINE);
        log.write_all(b"\n").unwrap();
        let line = fs::read(folder.join("net-2026-09-30.tsv")).unwrap();
        assert_eq!(line.len(), MAX_LINE);
    }

    #[test]
    fn captures_are_named_by_start_and_server_and_never_collide() {
        let dir = TempDir::new("net-capture");
        let library = Library::new(dir.path());
        assert_eq!(
            capture_stem(at(NOON), "game.example.org"),
            "2026-09-30_1234_NET_GAMEEXAMPLEORG"
        );
        let (first, _) = create_capture(&library, at(NOON), "192.168.1.20").unwrap();
        let (second, _) = create_capture(&library, at(NOON + 2), "192.168.1.20").unwrap();
        assert!(first.ends_with("replays/2026-09-30_1234_NET_192168120.tore-capture"));
        assert!(second.ends_with("replays/2026-09-30_1234_NET_192168120-2.tore-capture"));
        // The names follow the replays' pattern, newest last, collisions too.
        let key = |path: &Path| {
            library::order_key_for(
                path.file_name().and_then(|n| n.to_str()).unwrap(),
                CAPTURE_EXTENSION,
            )
        };
        assert!(key(&first).unwrap() < key(&second).unwrap());
        // A replay's extension is not a capture's.
        assert_eq!(
            library::order_key_for("2026-09-30_1234_NET_X.tore-replay", CAPTURE_EXTENSION),
            None
        );
    }

    /// Makes `path` look last written at `time`.
    fn touched(path: &Path, time: SystemTime) {
        OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(time)
            .unwrap();
    }

    /// A capture file with a start day, its first bytes, written an hour ago.
    fn capture(library: &Library, day: u32, magic: &[u8; 8]) -> PathBuf {
        let folder = library.folder();
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join(format!("2026-09-{day:02}_1200_NET_HOST.tore-capture"));
        let mut bytes = magic.to_vec();
        bytes.extend_from_slice(b" body");
        fs::write(&path, bytes).unwrap();
        touched(&path, at(NOON - 3_600));
        path
    }

    fn net_log(dir: &Path, day: u32) -> PathBuf {
        let folder = dir.join(LOG_FOLDER);
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join(format!("net-2026-09-{day:02}.tsv"));
        fs::write(&path, "x\n").unwrap();
        touched(&path, at(NOON - 3_600));
        path
    }

    #[test]
    fn pruning_follows_the_replay_rules_and_leaves_what_it_cannot_vouch_for() {
        let dir = TempDir::new("net-prune");
        let library = Library::new(dir.path());
        let captures: Vec<PathBuf> = (1..=5)
            .map(|d| capture(&library, d, CAPTURE_MAGIC))
            .collect();
        let foreign = capture(&library, 6, b"NOTCAPTU");
        // A replay in the same folder is the replays' business.
        let replay = library.folder().join("2026-09-01_1200_UKR_F18.tore-replay");
        fs::write(&replay, b"TOREREPL body").unwrap();
        touched(&replay, at(NOON - 3_600));
        let logs: Vec<PathBuf> = (1..=5).map(|d| net_log(dir.path(), d)).collect();
        let server_log = dir.path().join(LOG_FOLDER).join("server-2026-09-01.log");
        fs::write(&server_log, "x\n").unwrap();
        touched(&server_log, at(NOON - 3_600));
        // One written a minute ago may be a running game's.
        let running = capture(&library, 7, CAPTURE_MAGIC);
        touched(&running, at(NOON - 60));

        let mut settings = Settings {
            keep_last: 2,
            ..Settings::default()
        };
        // The oldest capture is the one being written: protected.
        let protect = [captures[0].as_path()];
        let done = prune(&library, dir.path(), &settings, at(NOON), &protect);
        // Captures are counted among themselves, newest first. The running
        // one (7) and the protected one (1) are never judged, so of 5, 4, 3
        // and 2 the newest two stay and 3 and 2 go.
        assert!(!captures[1].exists() && !captures[2].exists());
        assert!(captures[0].exists() && captures[3].exists() && captures[4].exists());
        assert!(running.exists());
        assert!(foreign.exists() && replay.exists() && server_log.exists());
        // Logs: the newest two days stay.
        assert!(!logs[0].exists() && !logs[1].exists() && !logs[2].exists());
        assert!(logs[3].exists() && logs[4].exists());
        assert_eq!(done.deleted.len(), 5);
        assert!(done.failed.is_empty());

        // Off, nothing goes.
        settings.auto_delete = false;
        let none = prune(&library, dir.path(), &settings, at(NOON), &[]);
        assert!(none.deleted.is_empty());
    }

    #[test]
    fn the_older_than_rule_counts_days_from_the_name() {
        let dir = TempDir::new("net-prune-age");
        let library = Library::new(dir.path());
        let old = capture(&library, 1, CAPTURE_MAGIC);
        let new = capture(&library, 28, CAPTURE_MAGIC);
        let (old_log, new_log) = (net_log(dir.path(), 1), net_log(dir.path(), 28));
        let settings = Settings {
            rule: Rule::OlderThan,
            older_than_days: 10,
            ..Settings::default()
        };
        let done = prune(&library, dir.path(), &settings, at(NOON), &[]);
        assert_eq!(done.deleted.len(), 2);
        assert!(!old.exists() && !old_log.exists());
        assert!(new.exists() && new_log.exists());
    }
}
