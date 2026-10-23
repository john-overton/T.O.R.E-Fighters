//! The client's diagnostics log (docs/ARCHITECTURE.md, "Recordings and
//! diagnostics"): tab-separated lines, a header first, then once a second the
//! connection's figures, and a line for every join, seating, refusal and
//! drop. The game writes it to `logs/net-<date>.tsv` in the data folder.
//!
//! Every line starts with the seconds since the client started (three
//! decimals) and its kind. A `stats` line's fields follow [`STATS_FIELDS`];
//! the counts are those of the second since the line before.

use super::ClientStats;
use std::io::Write;
use std::time::Duration;

/// The fields of a `stats` line after the time and the kind.
pub const STATS_FIELDS: [&str; 17] = [
    "round_trip_ms",
    "loss_percent",
    "snapshot_loss_percent",
    "snapshot_spread_ms",
    "input_margin_ticks",
    "interpolation_delay_ms",
    "clock_rate",
    "corrections",
    "corrections_shown",
    "mismatches",
    "frames",
    "extrapolated",
    "inputs_repeated",
    "bytes_up_per_second",
    "bytes_down_per_second",
    "predicted_tick",
    "render_tick",
];

/// The writer and its pace.
pub struct Diagnostics {
    out: Box<dyn Write>,
    next: Option<Duration>,
    last: ClientStats,
    failed: bool,
}

impl Diagnostics {
    /// A log on `out`, starting with its header line.
    pub fn new(out: Box<dyn Write>) -> Self {
        let mut d = Self {
            out,
            next: None,
            last: ClientStats::default(),
            failed: false,
        };
        let mut header = vec!["seconds", "kind"];
        header.extend(STATS_FIELDS);
        d.write(&header.join("\t"));
        d
    }

    fn write(&mut self, line: &str) {
        if !self.failed && writeln!(self.out, "{line}").is_err() {
            // A log that cannot be written stops; the session goes on.
            self.failed = true;
        }
    }

    /// An event line.
    pub fn line(&mut self, now: Duration, kind: &str, fields: &[&str]) {
        let mut line = format!("{:.3}\t{kind}", now.as_secs_f64());
        for field in fields {
            line.push('\t');
            // A field never breaks the line or the columns.
            line.extend(field.chars().map(|c| if c.is_control() { ' ' } else { c }));
        }
        self.write(&line);
    }

    /// Whether a `stats` line is due at `now`.
    pub fn due(&self, now: Duration) -> bool {
        self.next.is_none_or(|next| now >= next)
    }

    /// The `stats` line for the second ending at `now`.
    pub fn second(&mut self, now: Duration, stats: &ClientStats) {
        let next = self.next.get_or_insert(now);
        while *next <= now {
            *next += Duration::from_secs(1);
        }
        let last = std::mem::replace(&mut self.last, stats.clone());
        let ms = |d: Duration| format!("{:.1}", d.as_secs_f64() * 1000.);
        let fields = [
            ms(stats.round_trip),
            stats
                .loss
                .map_or_else(|| "-".into(), |l| format!("{:.2}", l * 100.)),
            format!("{:.2}", stats.snapshot_loss * 100.),
            ms(stats.spread),
            stats
                .input_margin
                .map_or_else(|| "-".into(), |m| m.to_string()),
            format!("{:.1}", stats.interpolation_delay_ticks * 1000. / 120.),
            format!("{:.4}", stats.clock_rate),
            (stats.corrections - last.corrections).to_string(),
            (stats.corrections_shown - last.corrections_shown).to_string(),
            (stats.mismatches - last.mismatches).to_string(),
            (stats.frames - last.frames).to_string(),
            (stats.extrapolated - last.extrapolated).to_string(),
            (stats.inputs_repeated - last.inputs_repeated).to_string(),
            stats.bytes_up_per_second.to_string(),
            stats.bytes_down_per_second.to_string(),
            stats.predicted_tick.to_string(),
            format!("{:.2}", stats.render_tick),
        ];
        let fields: Vec<&str> = fields.iter().map(String::as_str).collect();
        self.line(now, "stats", &fields);
        let _ = self.out.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);
    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn lines_are_tab_separated_with_a_header_and_one_stats_line_a_second() {
        let shared = Shared::default();
        let mut d = Diagnostics::new(Box::new(shared.clone()));
        d.line(
            Duration::from_millis(1500),
            "refused",
            &["Wrong\tpassword."],
        );
        for ms in (0..3000).step_by(10) {
            let now = Duration::from_millis(ms);
            if d.due(now) {
                d.second(now, &ClientStats::default());
            }
        }
        let text = String::from_utf8(shared.0.lock().unwrap().clone()).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0].split('\t').count(), 2 + STATS_FIELDS.len());
        assert_eq!(lines[1], "1.500\trefused\tWrong password.");
        let stats: Vec<&&str> = lines.iter().filter(|l| l.contains("\tstats\t")).collect();
        assert_eq!(stats.len(), 3);
        assert!(
            stats
                .iter()
                .all(|l| l.split('\t').count() == 2 + STATS_FIELDS.len())
        );
    }
}
