//! The recordings folder: file names, the auto-delete settings, listing and
//! cleanup. Opinionated addition requested by John on 2026-09-26; the names,
//! defaults and safety rules below are agent decisions (2026-09-26).
//!
//! Recordings live in `replays/` under the app data directory and are named
//! `2026-09-26_1540_UKR_F18.tore-replay`: the UTC date and time the flight
//! started, the map and the player's aircraft. The game has no time-zone
//! data without a new dependency, so the time is UTC, like the recording's
//! header. Cleanup deletes only files it can prove are recordings it may
//! remove: a matching name, the format's magic bytes, not kept, not the
//! recording in progress and not a `.partial` file another game may still
//! be writing.
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Folder under the app data directory.
pub const FOLDER: &str = "replays";
/// Settings file beside the other settings files in the app data directory.
pub const SETTINGS_FILE: &str = "replays-v1.conf";
/// Recording file extension.
pub const EXTENSION: &str = "tore-replay";
const PARTIAL: &str = ".partial";
/// The format's first bytes.
const MAGIC: &[u8; 8] = b"TOREREPL";
/// A `.partial` file written this recently may belong to a game that is
/// still running, so cleanup leaves it alone. The writer flushes every
/// second, so a live file is never older than that.
const LIVE_PARTIAL: Duration = Duration::from_secs(10 * 60);
/// Recordings a settings file may mark as kept.
pub const MAX_KEPT: usize = 4096;

/// Which recordings auto-delete removes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// Keep only the newest recordings.
    KeepLast,
    /// Delete recordings older than a number of days.
    OlderThan,
}

/// Auto-delete settings, saved in `replays-v1.conf`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// On by default, so recordings never fill the disk unnoticed.
    pub auto_delete: bool,
    pub rule: Rule,
    /// Recordings kept by [`Rule::KeepLast`], default 20.
    pub keep_last: u32,
    /// Age limit for [`Rule::OlderThan`], default 30 days.
    pub older_than_days: u32,
    /// File names the player marked Keep; never deleted.
    pub kept: BTreeSet<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_delete: true,
            rule: Rule::KeepLast,
            keep_last: 20,
            older_than_days: 30,
            kept: BTreeSet::new(),
        }
    }
}

impl Settings {
    pub fn text(&self) -> String {
        let mut text = format!(
            "tore-replays 1\nauto-delete {}\nrule {}\nkeep-last {}\nolder-than-days {}\n",
            if self.auto_delete { "on" } else { "off" },
            match self.rule {
                Rule::KeepLast => "keep-last",
                Rule::OlderThan => "older-than",
            },
            self.keep_last,
            self.older_than_days
        );
        for name in &self.kept {
            text.push_str(&format!("keep {name}\n"));
        }
        text
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        if lines.next() != Some("tore-replays 1") {
            return Err("unsupported replay settings version".into());
        }
        let mut settings = Self::default();
        let mut seen = BTreeSet::new();
        for line in lines {
            let (key, value) = line
                .split_once(' ')
                .ok_or_else(|| format!("invalid replay setting {line:?}"))?;
            if key != "keep" && !seen.insert(key) {
                return Err(format!("replay setting {key} appears twice"));
            }
            let number = |low: u32, high: u32| -> Result<u32, String> {
                value
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (low..=high).contains(n))
                    .ok_or_else(|| format!("{key} needs {low} to {high}"))
            };
            match key {
                "auto-delete" => {
                    settings.auto_delete = match value {
                        "on" => true,
                        "off" => false,
                        _ => return Err("auto-delete needs on or off".into()),
                    }
                }
                "rule" => {
                    settings.rule = match value {
                        "keep-last" => Rule::KeepLast,
                        "older-than" => Rule::OlderThan,
                        _ => return Err("rule needs keep-last or older-than".into()),
                    }
                }
                "keep-last" => settings.keep_last = number(1, 10_000)?,
                "older-than-days" => settings.older_than_days = number(1, 36_500)?,
                "keep" => {
                    if !is_recording_name(value) {
                        return Err(format!("{value:?} is not a recording name"));
                    }
                    if settings.kept.len() >= MAX_KEPT {
                        return Err("too many kept recordings".into());
                    }
                    settings.kept.insert(value.to_owned());
                }
                _ => return Err(format!("unknown replay setting {key}")),
            }
        }
        Ok(settings)
    }
}

/// UTC calendar time for `time`: year, month, day, hour, minute, second.
pub fn utc(time: SystemTime) -> [i64; 6] {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    [year, month, day, rest / 3_600, rest / 60 % 60, rest % 60]
}

/// Seconds since 1970 for a UTC calendar date and time.
fn epoch_seconds([year, month, day, hour, minute, second]: [i64; 6]) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    days * 86_400 + hour * 3_600 + minute * 60 + second
}

/// `2026-09-26T15:40:00Z`, as a recording's header keeps its start.
pub fn utc_text(time: SystemTime) -> String {
    let [y, mo, d, h, mi, s] = utc(time);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// `2026-09-26_1540`, the start of a recording's file name.
fn stamp(time: SystemTime) -> String {
    let [y, mo, d, h, mi, _] = utc(time);
    format!("{y:04}-{mo:02}-{d:02}_{h:02}{mi:02}")
}

/// Upper-case letters and digits only, at most 16, so a name part never
/// holds a path separator or a character some file systems refuse.
fn name_part(text: &str) -> String {
    let part: String = text
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(16)
        .collect::<String>()
        .to_ascii_uppercase();
    if part.is_empty() { "X".into() } else { part }
}

fn all(text: &str, test: impl Fn(char) -> bool) -> bool {
    !text.is_empty() && text.chars().all(test)
}

/// The start time a recording name holds, in seconds since 1970, when the
/// name has the recording pattern: `YYYY-MM-DD_HHMM_MAP_AIRCRAFT`, an
/// optional `-N` collision suffix, and the extension.
fn name_start(name: &str) -> Option<i64> {
    order_key(name).map(|(start, _)| start)
}

/// How recordings sort, newest last: the start time a recording name holds
/// (seconds since 1970) and its collision number, 1 for a name without a
/// `-N` suffix, so a second flight in the same minute (`-2`) is the newer
/// one. `None` when the name does not have the recording pattern.
pub fn order_key(name: &str) -> Option<(i64, u32)> {
    let stem = name.strip_suffix(&format!(".{EXTENSION}"))?;
    let (stem, suffix) = match stem.rsplit_once('-') {
        Some((head, n)) if head.len() > 10 && all(n, |c| c.is_ascii_digit()) && n.len() <= 4 => {
            (head, Some(n))
        }
        _ => (stem, None),
    };
    if suffix.is_some_and(|n| n.starts_with('0')) {
        return None;
    }
    let collision = match suffix {
        Some(n) => n.parse::<u32>().ok()?,
        None => 1,
    };
    let mut parts = stem.split('_');
    let (date, time, map, aircraft) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let upper = |c: char| c.is_ascii_uppercase() || c.is_ascii_digit();
    if !all(map, upper) || !all(aircraft, upper) || map.len() > 16 || aircraft.len() > 16 {
        return None;
    }
    let digits = |text: &str| all(text, |c| c.is_ascii_digit());
    let bytes = date.as_bytes();
    if date.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' || time.len() != 4 {
        return None;
    }
    let (year, month, day) = (&date[..4], &date[5..7], &date[8..]);
    if ![year, month, day, time].into_iter().all(digits) {
        return None;
    }
    let number = |text: &str| text.parse::<i64>().ok();
    let (month, day) = (number(month)?, number(day)?);
    let (hour, minute) = (number(&time[..2])?, number(&time[2..])?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    Some((
        epoch_seconds([number(year)?, month, day, hour, minute, 0]),
        collision,
    ))
}

/// Whether `name` has the recording file name pattern.
pub fn is_recording_name(name: &str) -> bool {
    name_start(name).is_some()
}

/// One file in the recordings folder.
#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    /// The recording's name, without `.partial`.
    pub name: String,
    pub bytes: u64,
    /// Still being written, or cut short by a crash.
    pub partial: bool,
    pub kept: bool,
    /// What the header and footer say, or why the file could not be read.
    pub peek: Result<tore_replay::Peek, String>,
}

/// What a cleanup did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cleanup {
    pub deleted: Vec<PathBuf>,
    /// Files it wanted to delete but could not, with the reason.
    pub failed: Vec<(PathBuf, String)>,
}

/// The recordings folder of one app data directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
    data: PathBuf,
}

impl Library {
    pub fn new(data: &Path) -> Self {
        Self {
            data: data.to_path_buf(),
        }
    }

    pub fn folder(&self) -> PathBuf {
        self.data.join(FOLDER)
    }

    pub fn settings_path(&self) -> PathBuf {
        self.data.join(SETTINGS_FILE)
    }

    /// The saved settings; a missing file gives the defaults, and a damaged
    /// one the defaults with a warning in the session log.
    pub fn settings(&self) -> Settings {
        match crate::preferences::read(&self.settings_path()) {
            Ok(text) => Settings::parse(&text).unwrap_or_else(|error| {
                log::warn!("Replays: {SETTINGS_FILE} ignored: {error}");
                Settings::default()
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(error) => {
                log::warn!("Replays: {SETTINGS_FILE} unreadable: {error}");
                Settings::default()
            }
        }
    }

    pub fn save_settings(&self, settings: &Settings) -> std::io::Result<()> {
        crate::preferences::write(&self.settings_path(), &settings.text())
    }

    /// A new recording's path for a flight that started at `start`, on map
    /// `map` in `aircraft`, creating the folder. Neither the name nor its
    /// `.partial` form exists yet; a clash adds `-2`, `-3` and so on.
    pub fn new_path(
        &self,
        start: SystemTime,
        map: &str,
        aircraft: &str,
    ) -> std::io::Result<PathBuf> {
        let folder = self.folder();
        std::fs::create_dir_all(&folder)?;
        let stem = format!(
            "{}_{}_{}",
            stamp(start),
            name_part(map),
            name_part(aircraft)
        );
        for n in 1..10_000 {
            let name = if n == 1 {
                format!("{stem}.{EXTENSION}")
            } else {
                format!("{stem}-{n}.{EXTENSION}")
            };
            let path = folder.join(&name);
            if !path.exists() && !tore_replay::partial_path(&path).exists() {
                return Ok(path);
            }
        }
        Err(std::io::Error::other(
            "too many recordings share one start minute",
        ))
    }

    /// Every recording in the folder, finished or not, newest first (see
    /// [`order_key`]). Files that are not recordings are left out.
    pub fn list(&self) -> Vec<Entry> {
        let kept = self.settings().kept;
        let Ok(dir) = std::fs::read_dir(self.folder()) else {
            return Vec::new();
        };
        let mut entries: Vec<((i64, u32), Entry)> = dir
            .flatten()
            .filter_map(|file| {
                let path = file.path();
                let file_name = file.file_name().into_string().ok()?;
                let (name, partial) = match file_name.strip_suffix(PARTIAL) {
                    Some(name) => (name.to_owned(), true),
                    None => (file_name, false),
                };
                let start = order_key(&name)?;
                if !has_magic(&path) {
                    return None;
                }
                let bytes = file.metadata().map_or(0, |m| m.len());
                let peek = tore_replay::Recording::peek(&path).map_err(|e| e.to_string());
                Some((
                    start,
                    Entry {
                        kept: kept.contains(&name),
                        path,
                        name,
                        bytes,
                        partial,
                        peek,
                    },
                ))
            })
            .collect();
        entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.name.cmp(&a.1.name)));
        entries.into_iter().map(|(_, entry)| entry).collect()
    }

    /// Applies the auto-delete rule, if it is on, at time `now`. `protect`
    /// lists recordings that must survive whatever the rule says, such as the
    /// one being written.
    pub fn cleanup(&self, settings: &Settings, now: SystemTime, protect: &[&Path]) -> Cleanup {
        let mut result = Cleanup::default();
        for path in self.plan(settings, now, protect) {
            match std::fs::remove_file(&path) {
                Ok(()) => result.deleted.push(path),
                Err(error) => result.failed.push((path, error.to_string())),
            }
        }
        result
    }

    /// The recordings [`Library::cleanup`] would delete now, deleting
    /// nothing: the Replays screen shows how many.
    pub fn plan(&self, settings: &Settings, now: SystemTime, protect: &[&Path]) -> Vec<PathBuf> {
        if !settings.auto_delete {
            return Vec::new();
        }
        let Ok(dir) = std::fs::read_dir(self.folder()) else {
            return Vec::new();
        };
        let protected: BTreeSet<PathBuf> = protect
            .iter()
            .flat_map(|path| [path.to_path_buf(), tore_replay::partial_path(path)])
            .collect();
        let now_s = now
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let mut candidates: Vec<((i64, u32), String, PathBuf)> = dir
            .flatten()
            .filter_map(|file| {
                let path = file.path();
                let file_name = file.file_name().into_string().ok()?;
                let (name, partial) = match file_name.strip_suffix(PARTIAL) {
                    Some(name) => (name.to_owned(), true),
                    None => (file_name, false),
                };
                let start = order_key(&name)?;
                if settings.kept.contains(&name)
                    || protected.contains(&path)
                    || !file.file_type().is_ok_and(|t| t.is_file())
                    || !has_magic(&path)
                {
                    return None;
                }
                if partial {
                    let recent = file
                        .metadata()
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|modified| now.duration_since(modified).ok())
                        .is_none_or(|age| age < LIVE_PARTIAL);
                    if recent {
                        return None;
                    }
                }
                Some((start, name, path))
            })
            .collect();
        // Newest first.
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
        match settings.rule {
            Rule::KeepLast => candidates
                .into_iter()
                .skip(settings.keep_last as usize)
                .map(|(_, _, path)| path)
                .collect(),
            Rule::OlderThan => {
                let limit = now_s - i64::from(settings.older_than_days) * 86_400;
                candidates
                    .into_iter()
                    .filter(|((start, _), _, _)| *start < limit)
                    .map(|(_, _, path)| path)
                    .collect()
            }
        }
    }
}

/// Whether the file starts with the recording format's magic bytes.
fn has_magic(path: &Path) -> bool {
    let mut head = [0; 8];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut head))
        .is_ok()
        && &head == MAGIC
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::tests::TempDir;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn utc_calendar_times_round_trip() {
        for (seconds, text) in [
            (0, "1970-01-01T00:00:00Z"),
            (951_782_400, "2000-02-29T00:00:00Z"),
            (1_709_164_800, "2024-02-29T00:00:00Z"),
            (1_790_437_200, "2026-09-26T15:40:00Z"),
            (4_102_444_799, "2099-12-31T23:59:59Z"),
        ] {
            assert_eq!(utc_text(at(seconds)), text);
            assert_eq!(epoch_seconds(utc(at(seconds))), seconds as i64);
        }
        assert_eq!(stamp(at(1_790_437_200)), "2026-09-26_1540");
    }

    #[test]
    fn names_follow_the_pattern_and_never_collide() {
        let dir = TempDir::new("library-names");
        let library = Library::new(dir.path());
        let start = at(1_790_437_200);
        let first = library.new_path(start, "~UKR1", "f/a-xx").unwrap();
        assert_eq!(
            first.file_name().unwrap(),
            "2026-09-26_1540_UKR1_FAXX.tore-replay"
        );
        std::fs::write(&first, b"x").unwrap();
        let second = library.new_path(start, "~UKR1", "f/a-xx").unwrap();
        assert_eq!(
            second.file_name().unwrap(),
            "2026-09-26_1540_UKR1_FAXX-2.tore-replay"
        );
        // A recording in progress holds its name too.
        std::fs::write(tore_replay::partial_path(&second), b"x").unwrap();
        let third = library.new_path(start, "~UKR1", "f/a-xx").unwrap();
        assert!(third.ends_with("2026-09-26_1540_UKR1_FAXX-3.tore-replay"));
        for name in [
            "2026-09-26_1540_UKR_F18.tore-replay",
            "2026-09-26_1540_UKR_F18-12.tore-replay",
            "1999-12-31_2359_KURILE_FAXX.tore-replay",
        ] {
            assert!(is_recording_name(name), "{name}");
        }
        for name in [
            "2026-09-26_1540_UKR_F18.tore-replay.partial",
            "2026-09-26_1540_ukr_F18.tore-replay",
            "2026-13-26_1540_UKR_F18.tore-replay",
            "2026-09-26_2460_UKR_F18.tore-replay",
            "2026-09-26_1540_UKR_F18-0.tore-replay",
            "2026-09-26_1540_UKR.tore-replay",
            "2026-09-26_1540_UKR_F18_X.tore-replay",
            "notes.txt",
            "../2026-09-26_1540_UKR_F18.tore-replay",
        ] {
            assert!(!is_recording_name(name), "{name}");
        }
    }

    #[test]
    fn a_later_flight_in_the_same_minute_sorts_as_newer() {
        let key = |name: &str| order_key(name).unwrap();
        let base = key("2026-09-26_1540_UKR_F18.tore-replay");
        assert_eq!(base.1, 1);
        assert!(key("2026-09-26_1540_UKR_F18-2.tore-replay") > base);
        assert!(
            key("2026-09-26_1540_UKR_F18-10.tore-replay")
                > key("2026-09-26_1540_UKR_F18-2.tore-replay")
        );
        assert!(
            key("2026-09-26_1541_UKR_F18.tore-replay")
                > key("2026-09-26_1540_UKR_F18-10.tore-replay")
        );
        assert_eq!(order_key("2026-09-26_1540_UKR_F18-0.tore-replay"), None);
    }

    #[test]
    fn settings_round_trip_and_refuse_nonsense() {
        let mut settings = Settings {
            auto_delete: false,
            rule: Rule::OlderThan,
            keep_last: 5,
            older_than_days: 7,
            kept: BTreeSet::new(),
        };
        settings
            .kept
            .insert("2026-09-26_1540_UKR_F18.tore-replay".into());
        assert_eq!(Settings::parse(&settings.text()), Ok(settings.clone()));
        assert_eq!(
            Settings::parse(&Settings::default().text()),
            Ok(Settings::default())
        );
        for bad in [
            "",
            "tore-replays 2\n",
            "tore-replays 1\nauto-delete maybe\n",
            "tore-replays 1\nkeep-last 0\n",
            "tore-replays 1\nolder-than-days x\n",
            "tore-replays 1\nkeep ../../etc/passwd\n",
            "tore-replays 1\nrule keep-last\nrule older-than\n",
            "tore-replays 1\ncolour blue\n",
        ] {
            assert!(Settings::parse(bad).is_err(), "{bad:?}");
        }
        let dir = TempDir::new("library-settings");
        let library = Library::new(dir.path());
        assert_eq!(library.settings(), Settings::default());
        library.save_settings(&settings).unwrap();
        assert_eq!(library.settings(), settings);
        std::fs::write(library.settings_path(), "garbage").unwrap();
        assert_eq!(library.settings(), Settings::default());
    }

    /// Writes a minimal real recording at `name` in the folder.
    fn recording(library: &Library, name: &str) -> PathBuf {
        let path = library.folder().join(name);
        let writer = tore_replay::Writer::create(&path, &tore_replay::Header::default()).unwrap();
        writer.finish(&tore_replay::Footer::default()).unwrap()
    }

    #[test]
    fn cleanup_deletes_only_what_the_rule_allows() {
        let dir = TempDir::new("library-cleanup");
        let library = Library::new(dir.path());
        std::fs::create_dir_all(library.folder()).unwrap();
        let day = 86_400;
        let now = at(1_790_437_200);
        let names: Vec<String> = (1..=6)
            .map(|n| {
                let [y, mo, d, ..] = utc(at(1_790_437_200 - n * 10 * day));
                format!("{y:04}-{mo:02}-{d:02}_1200_UKR_F18.tore-replay")
            })
            .collect();
        for name in &names {
            recording(&library, name);
        }
        // Kept, in progress, a foreign file with a recording's name, a
        // recording with a foreign name, and two unfinished recordings: one
        // from a crash long ago and one another game may be writing.
        let mut settings = Settings {
            keep_last: 2,
            ..Settings::default()
        };
        settings.kept.insert(names[5].clone());
        let current = library.folder().join("2026-09-26_1539_UKR_F18.tore-replay");
        let writer =
            tore_replay::Writer::create(&current, &tore_replay::Header::default()).unwrap();
        let foreign = library.folder().join("2020-01-01_0000_UKR_F18.tore-replay");
        std::fs::write(&foreign, b"not a recording").unwrap();
        let renamed = library.folder().join("my favourite.tore-replay");
        std::fs::copy(library.folder().join(&names[0]), &renamed).unwrap();
        let crashed = library.folder().join(format!("{}.partial", names[4]));
        std::fs::copy(library.folder().join(&names[0]), &crashed).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&crashed)
            .unwrap()
            .set_modified(at(1_790_437_200 - 60 * day))
            .unwrap();
        let running = library
            .folder()
            .join("2026-09-26_1530_UKR_F18.tore-replay.partial");
        std::fs::copy(library.folder().join(&names[0]), &running).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&running)
            .unwrap()
            .set_modified(at(1_790_437_200 - 60))
            .unwrap();
        let listed: Vec<String> = library.list().into_iter().map(|e| e.name).collect();
        assert_eq!(listed.len(), 9, "{listed:?}");
        assert!(
            !listed
                .iter()
                .any(|n| n.starts_with("2020") || n.starts_with("my "))
        );

        // Off: nothing happens.
        let off = Settings {
            auto_delete: false,
            ..settings.clone()
        };
        assert_eq!(library.cleanup(&off, now, &[&current]), Cleanup::default());
        // Keep the newest two of the recordings cleanup may touch: the
        // running game's file, the current one and the kept one never count.
        let result = library.cleanup(&settings, now, &[&current]);
        let deleted: BTreeSet<PathBuf> = result.deleted.iter().cloned().collect();
        let expected: BTreeSet<PathBuf> = [&names[2], &names[3], &names[4]]
            .into_iter()
            .map(|n| library.folder().join(n))
            .chain([crashed.clone()])
            .collect();
        assert_eq!(deleted, expected);
        assert!(result.failed.is_empty());
        for survivor in [
            &foreign,
            &renamed,
            &running,
            &tore_replay::partial_path(&current),
        ] {
            assert!(survivor.exists(), "{survivor:?}");
        }
        assert!(library.folder().join(&names[5]).exists());
        // Older than 15 days: only the second-newest remaining goes.
        let settings = Settings {
            rule: Rule::OlderThan,
            older_than_days: 15,
            ..settings
        };
        let result = library.cleanup(&settings, now, &[&current]);
        assert_eq!(result.deleted, [library.folder().join(&names[1])]);
        assert!(library.folder().join(&names[0]).exists());
        drop(writer);
    }
}
