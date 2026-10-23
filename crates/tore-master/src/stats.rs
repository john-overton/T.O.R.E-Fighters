//! The status line and the minute table ("Logs and statistics" in the
//! operations guide).
//!
//! The status line goes to standard output every `status-interval` seconds;
//! the same counts go to `state-dir/stats/YYYY-MM-DD.tsv` once a minute. The
//! rates and the counts of drops and invalid datagrams are over the time
//! since the previous line; the rest are as they stand.

use std::time::Duration;

use crate::master::{Counters, Master};

/// One status line's numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Status {
    /// Listings now.
    pub listings: usize,
    /// Sources remembered now.
    pub sources: usize,
    /// Browse and Details answered a second, over the interval.
    pub browse_per_second: f64,
    /// Introductions a minute, over the interval (stage J).
    pub introductions_per_minute: f64,
    /// Introductions that ended punched, over the interval (stage J).
    pub punched: u64,
    /// Relay channels opened, over the interval (stage J).
    pub relayed: u64,
    /// Relay channels open now (stage J).
    pub channels: usize,
    /// Bytes relayed out this month (stage J).
    pub relay_month_bytes: u64,
    /// Requests and answers dropped for a limit, over the interval.
    pub dropped_limit: u64,
    /// Datagrams that were not a master packet this master answers: invalid,
    /// malformed, an unsupported version or an unexpected kind.
    pub invalid: u64,
    /// Bytes received, over the interval.
    pub bytes_in: u64,
    /// Bytes sent, over the interval.
    pub bytes_out: u64,
}

impl Status {
    /// The status line.
    pub fn line(&self) -> String {
        format!(
            "status listings={} sources={} browse/s={:.1} introductions/min={:.0} punched={} relayed={} channels={} relay-month={} dropped(limit)={} invalid={} in={} out={}",
            self.listings,
            self.sources,
            self.browse_per_second,
            self.introductions_per_minute,
            self.punched,
            self.relayed,
            self.channels,
            bytes(self.relay_month_bytes),
            self.dropped_limit,
            self.invalid,
            bytes(self.bytes_in),
            bytes(self.bytes_out),
        )
    }

    /// The minute table's header.
    pub const TSV_HEADER: &'static str = "time\tlistings\tsources\tbrowse_per_s\tintroductions_per_min\tpunched\trelayed\tchannels\trelay_month_bytes\tdropped_limit\tinvalid\tbytes_in\tbytes_out";

    /// One row of the minute table, at `time` (`HH:MM:SS`, UTC).
    pub fn tsv_row(&self, time: &str) -> String {
        format!(
            "{time}\t{}\t{}\t{:.2}\t{:.1}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.listings,
            self.sources,
            self.browse_per_second,
            self.introductions_per_minute,
            self.punched,
            self.relayed,
            self.channels,
            self.relay_month_bytes,
            self.dropped_limit,
            self.invalid,
            self.bytes_in,
            self.bytes_out,
        )
    }
}

/// A size in bytes, kilobytes, megabytes or gigabytes.
pub fn bytes(n: u64) -> String {
    if n < 10_000 {
        format!("{n}B")
    } else if n < 10_000_000 {
        format!("{:.1}KB", n as f64 / 1e3)
    } else if n < 10_000_000_000 {
        format!("{:.1}MB", n as f64 / 1e6)
    } else {
        format!("{:.1}GB", n as f64 / 1e9)
    }
}

/// The counters at the previous line, to take differences from.
#[derive(Debug, Clone, Copy, Default)]
pub struct Interval {
    last: Counters,
    introductions: u64,
    relayed: u64,
    at: Duration,
}

impl Interval {
    /// Starts measuring at `now`.
    pub fn new(master: &Master, now: Duration) -> Self {
        Self {
            last: master.counters(),
            introductions: master.introductions().introduced,
            relayed: master.relays().counters.opened,
            at: now,
        }
    }

    /// The status since the previous call (or the start), and starts the next
    /// interval.
    pub fn take(&mut self, master: &Master, now: Duration) -> Status {
        let c = master.counters();
        let l = self.last;
        let seconds = now.saturating_sub(self.at).as_secs_f64().max(1e-3);
        let introductions = master.introductions().introduced;
        let relayed = master.relays().counters.opened;
        let status = Status {
            listings: master.listings().len(),
            sources: master.sources(),
            browse_per_second: (c.browses + c.details - l.browses - l.details) as f64 / seconds,
            introductions_per_minute: (introductions - self.introductions) as f64 * 60.0 / seconds,
            punched: 0,
            relayed: relayed - self.relayed,
            channels: master.relays().channels(),
            relay_month_bytes: master.relays().month_bytes(),
            dropped_limit: c.dropped_limit + c.dropped_answers
                - l.dropped_limit
                - l.dropped_answers,
            invalid: (c.invalid + c.malformed + c.unsupported + c.unexpected)
                - (l.invalid + l.malformed + l.unsupported + l.unexpected),
            bytes_in: c.bytes_in - l.bytes_in,
            bytes_out: c.bytes_out - l.bytes_out,
        };
        *self = Self {
            last: c,
            introductions,
            relayed,
            at: now,
        };
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_has_the_guides_fields() {
        let status = Status {
            listings: 41,
            sources: 318,
            browse_per_second: 2.4,
            introductions_per_minute: 7.0,
            punched: 5,
            relayed: 2,
            channels: 3,
            relay_month_bytes: 12_700_000_000,
            dropped_limit: 0,
            invalid: 4,
            bytes_in: 123_456,
            bytes_out: 789,
        };
        assert_eq!(
            status.line(),
            "status listings=41 sources=318 browse/s=2.4 introductions/min=7 punched=5 relayed=2 channels=3 relay-month=12.7GB dropped(limit)=0 invalid=4 in=123.5KB out=789B"
        );
        assert_eq!(
            status.tsv_row("12:00:00").split('\t').count(),
            Status::TSV_HEADER.split('\t').count()
        );
    }
}
