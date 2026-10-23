//! One feedback flag per JSON line, for analysis without opening the viewer.
use super::json::string;
use crate::Recording;
use std::io::{self, Write};
pub fn write(recording: &Recording, file: &str, mut out: impl Write) -> io::Result<()> {
    for f in recording.feedback() {
        writeln!(
            out,
            "{{\"file\":{},\"tick\":{},\"mission_time\":{},\"wall_clock_time_ms\":{},\"submit_tick\":{},\"submitted_at_ms\":{},\"player\":{},\"seat\":{},\"aircraft\":{},\"theater\":{},\"text\":{}}}",
            string(file),
            f.tick,
            f.tick as f64 / 120.,
            f.opened_at,
            f.submit_tick,
            f.submitted_at,
            string(&f.player),
            f.seat,
            string(&f.aircraft),
            string(&recording.header().world.theater),
            string(&f.text)
        )?;
    }
    Ok(())
}
