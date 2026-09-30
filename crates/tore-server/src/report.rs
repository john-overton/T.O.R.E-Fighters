//! The lines the server prints: the status line, the players table and each
//! player's figures for the log.

use crate::host::{PlayerFigures, Status};

/// `HH:MM:SS` of mission time for a tick count at 120 ticks a second.
pub fn mission_clock(tick: u64) -> String {
    let seconds = tick / 120;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3_600,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// Kilobytes (1,000 bytes) a second, rounded to a whole number.
fn rate(bytes_per_second: f64) -> String {
    format!("{:.0} KB/s", bytes_per_second / 1_000.0)
}

/// The status line, for example
/// `00:12:30 tick 90000 players 2/15 aircraft 30 load 11% (1.1 ms a tick) up 64 KB/s down 7 KB/s`.
pub fn status_line(status: &Status) -> String {
    format!(
        "{} tick {} players {}/{} aircraft {} load {:.0}% ({:.1} ms a tick) up {} down {}",
        mission_clock(status.tick),
        status.tick,
        status.players,
        status.capacity,
        status.aircraft,
        status.load_percent,
        status.tick_ms,
        rate(status.up_bytes_per_second),
        rate(status.down_bytes_per_second),
    )
}

fn plane(figures: &PlayerFigures) -> String {
    figures
        .plane
        .map_or_else(|| "none".into(), |plane| plane.to_string())
}

fn seat(figures: &PlayerFigures) -> String {
    figures
        .seat
        .map_or_else(|| "-".into(), |seat| seat.to_string())
}

/// A figure that may not be known yet.
fn optional(value: Option<f64>, unit: &str) -> String {
    value.map_or_else(|| "-".into(), |v| format!("{v:.1}{unit}"))
}

/// The `players` command's answer.
pub fn players_table(players: &[PlayerFigures]) -> Vec<String> {
    if players.is_empty() {
        return vec!["No players connected".into()];
    }
    let mut lines = vec![format!(
        "{:<4} {:<15} {:<5} {:>8} {:>7} {:>9} {:>9}  {}",
        "seat", "callsign", "plane", "rtt", "loss", "margin", "repeated", "address"
    )];
    for p in players {
        lines.push(format!(
            "{:<4} {:<15} {:<5} {:>5.0} ms {:>7} {:>9} {:>9}  {}",
            seat(p),
            p.callsign,
            plane(p),
            p.round_trip_ms,
            optional(p.loss_percent, "%"),
            optional(p.input_margin_ticks, " tk"),
            p.inputs_repeated,
            p.address
        ));
    }
    lines
}

/// One player's line in the once-a-minute log: the figures a player's game
/// writes to its own diagnostics log.
pub fn figures_line(p: &PlayerFigures) -> String {
    format!(
        "figures seat {} {} plane {}: round trip {:.0} ms, loss {}, arrival spread {:.1} ms, input margin {}, inputs repeated {}, sending {} and receiving {}",
        seat(p),
        p.callsign,
        plane(p),
        p.round_trip_ms,
        optional(p.loss_percent, "%"),
        p.arrival_spread_ms,
        optional(p.input_margin_ticks, " ticks"),
        p.inputs_repeated,
        rate(p.bytes_up_per_second),
        rate(p.bytes_down_per_second)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_line_is_the_guides_example() {
        let status = Status {
            tick: 90_000,
            players: 2,
            capacity: 15,
            aircraft: 30,
            load_percent: 11.0,
            tick_ms: 1.1,
            up_bytes_per_second: 64_000.0,
            down_bytes_per_second: 7_000.0,
        };
        assert_eq!(
            status_line(&status),
            "00:12:30 tick 90000 players 2/15 aircraft 30 load 11% (1.1 ms a tick) up 64 KB/s down 7 KB/s"
        );
    }

    #[test]
    fn the_mission_clock_runs_at_120_ticks_a_second() {
        assert_eq!(mission_clock(0), "00:00:00");
        assert_eq!(mission_clock(119), "00:00:00");
        assert_eq!(mission_clock(120), "00:00:01");
        assert_eq!(mission_clock(120 * 3_661), "01:01:01");
    }

    #[test]
    fn the_players_table_and_figures_name_every_figure() {
        let player = PlayerFigures {
            seat: Some(1),
            callsign: "Viper".into(),
            address: "127.0.0.1:5000".into(),
            plane: Some(3),
            round_trip_ms: 42.4,
            loss_percent: Some(0.25),
            arrival_spread_ms: 3.0,
            input_margin_ticks: Some(2.5),
            inputs_repeated: 7,
            bytes_up_per_second: 21_400.0,
            bytes_down_per_second: 3_200.0,
        };
        let table = players_table(std::slice::from_ref(&player));
        assert_eq!(table.len(), 2);
        assert!(table[0].contains("callsign") && table[0].contains("repeated"));
        assert!(table[1].contains("Viper") && table[1].contains("42 ms"));
        assert!(table[1].contains("127.0.0.1:5000"));
        let line = figures_line(&player);
        for needle in [
            "seat 1 Viper plane 3",
            "round trip 42 ms",
            "loss 0.2%",
            "arrival spread 3.0 ms",
            "input margin 2.5 ticks",
            "inputs repeated 7",
            "sending 21 KB/s",
            "receiving 3 KB/s",
        ] {
            assert!(line.contains(needle), "{needle} in {line}");
        }
        assert_eq!(players_table(&[]), vec!["No players connected"]);
    }

    #[test]
    fn a_connection_with_no_seat_or_figures_yet_shows_dashes() {
        let player = PlayerFigures {
            callsign: "Newcomer".into(),
            ..Default::default()
        };
        let line = figures_line(&player);
        assert!(line.contains("seat - Newcomer plane none"), "{line}");
        assert!(line.contains("loss -, "), "{line}");
        assert!(line.contains("input margin -,"), "{line}");
        assert!(players_table(&[player])[1].starts_with("-    Newcomer"));
    }
}
