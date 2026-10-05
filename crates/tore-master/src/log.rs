//! The files the master writes in its state folder, and the UTC dates they
//! are named by ("Logs and statistics" in the operations guide):
//!
//! - `stats/YYYY-MM-DD.tsv`: the minute table, kept 90 days, then deleted by
//!   the master;
//! - `telemetry/YYYY-MM-DD.tsv`: the day's counts, rewritten every minute and
//!   when the master stops, kept until deleted by hand.
//!
//! The standard library has no time zones, so dates are UTC, as the server's
//! log is.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::stats::Status;

/// The minute table's files are deleted after this many days.
pub const STATS_KEPT_DAYS: i64 = 90;

/// Seconds since 1970-01-01 UTC, now.
pub fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Days since 1970-01-01 for a Unix time.
pub fn day_of(unix_seconds: u64) -> i64 {
    (unix_seconds / 86_400) as i64
}

/// Days since 1970-01-01 to a proleptic Gregorian (year, month, day): the
/// dedicated server's `civil_from_days`.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    } as u32;
    (year + i64::from(month <= 2), month, day)
}

/// `YYYY-MM-DD` for a day.
pub fn date(day: i64) -> String {
    let (year, month, day) = civil_from_days(day);
    format!("{year:04}-{month:02}-{day:02}")
}

/// `HH:MM:SS` UTC for a Unix time.
pub fn time_of_day(unix_seconds: u64) -> String {
    let s = unix_seconds % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3_600, s / 60 % 60, s % 60)
}

/// The state folder's files.
#[derive(Debug, Clone)]
pub struct StateFiles {
    dir: PathBuf,
}

impl StateFiles {
    /// The files under `dir`, making `stats/` and `telemetry/` there.
    pub fn open(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir.join("stats"))?;
        fs::create_dir_all(dir.join("telemetry"))?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// The minute table of a day.
    pub fn stats_path(&self, day: i64) -> PathBuf {
        self.dir.join("stats").join(format!("{}.tsv", date(day)))
    }

    /// The telemetry counts of a day.
    pub fn telemetry_path(&self, day: i64) -> PathBuf {
        self.dir
            .join("telemetry")
            .join(format!("{}.tsv", date(day)))
    }

    /// Appends a row to the day's minute table, with the header first in a
    /// new file.
    pub fn append_stats(&self, unix_seconds: u64, status: &Status) -> io::Result<()> {
        let path = self.stats_path(day_of(unix_seconds));
        let new = !path.exists();
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        if new {
            writeln!(file, "{}", Status::TSV_HEADER)?;
        }
        writeln!(file, "{}", status.tsv_row(&time_of_day(unix_seconds)))
    }

    /// The day's telemetry counts written so far, if any.
    pub fn read_telemetry(&self, day: i64) -> Option<String> {
        fs::read_to_string(self.telemetry_path(day)).ok()
    }

    /// Replaces the day's telemetry counts, through a file beside it so a
    /// crash never leaves half a file.
    pub fn write_telemetry(&self, day: i64, text: &str) -> io::Result<()> {
        let path = self.telemetry_path(day);
        let partial = path.with_extension("tsv.partial");
        fs::write(&partial, text)?;
        fs::rename(&partial, &path)
    }

    /// Deletes the minute tables older than [`STATS_KEPT_DAYS`] before
    /// `today`. Only files named as the master names them are touched.
    pub fn prune_stats(&self, today: i64) -> io::Result<usize> {
        let mut deleted = 0;
        for entry in fs::read_dir(self.dir.join("stats"))? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(day) = name.to_str().and_then(day_of_file_name) else {
                continue;
            };
            if today - day > STATS_KEPT_DAYS {
                fs::remove_file(entry.path())?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }
}

/// The day a `YYYY-MM-DD.tsv` name stands for.
fn day_of_file_name(name: &str) -> Option<i64> {
    let stem = name.strip_suffix(".tsv")?;
    let mut parts = stem.split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d): (i64, u32, u32) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    // Search near the year's start: a name is a real date or nothing.
    let guess = (y - 1970) * 365 + (y - 1969) / 4 - 1;
    (guess - 2..guess + 370).find(|&day| civil_from_days(day) == (y, m, d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_match_known_days() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(20_366), "2025-10-05");
        assert_eq!(date(20_731), "2026-10-05");
        assert_eq!(time_of_day(20_731 * 86_400 + 3_661), "01:01:01");
        assert_eq!(day_of_file_name("2026-10-05.tsv"), Some(20_731));
        assert_eq!(day_of_file_name("1970-01-01.tsv"), Some(0));
        assert_eq!(day_of_file_name("2026-02-30.tsv"), None);
        assert_eq!(day_of_file_name("notes.tsv"), None);
    }

    #[test]
    fn files_are_written_resumed_and_pruned() {
        let dir = std::env::temp_dir().join(format!(
            "tore-master-log-test-{}-{}",
            std::process::id(),
            unix_seconds()
        ));
        let files = StateFiles::open(&dir).unwrap();
        let today = 20_731;
        let status = Status {
            listings: 1,
            sources: 2,
            browse_per_second: 0.0,
            introductions_per_minute: 0.0,
            punched: 0,
            relayed: 0,
            channels: 0,
            relay_month_bytes: 0,
            dropped_limit: 0,
            invalid: 0,
            bytes_in: 0,
            bytes_out: 0,
        };
        let at = today as u64 * 86_400 + 60;
        files.append_stats(at, &status).unwrap();
        files.append_stats(at + 60, &status).unwrap();
        let table = fs::read_to_string(files.stats_path(today)).unwrap();
        assert_eq!(table.lines().count(), 3);
        assert!(table.starts_with("time\tlistings"));
        assert!(
            table
                .lines()
                .nth(2)
                .unwrap()
                .starts_with("00:02:00\t1\t2\t")
        );
        files.write_telemetry(today, "installs\t1\n").unwrap();
        assert_eq!(files.read_telemetry(today).unwrap(), "installs\t1\n");
        // An old table goes; a recent one and a stranger stay.
        fs::write(files.stats_path(today - 91), "old").unwrap();
        fs::write(files.stats_path(today - 90), "kept").unwrap();
        fs::write(dir.join("stats").join("notes.txt"), "mine").unwrap();
        assert_eq!(files.prune_stats(today).unwrap(), 1);
        assert!(!files.stats_path(today - 91).exists());
        assert!(files.stats_path(today - 90).exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
