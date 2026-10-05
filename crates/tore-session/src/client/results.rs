//! The results a player's game keeps (stage F phase 2, slice F2-D;
//! docs/ARCHITECTURE.md, "The multiplayer debrief"): the Results message the
//! host sent at the mission's end, which the debrief screen adds its SCORES
//! and RESULTS pages from, and the words the log and the bots use for it.

use super::Client;
use crate::wire::messages::{ResultRow, ResultStatus, Results};

impl Client {
    /// The results of the mission that just ended, until the next mission
    /// or flight starts. Sent to every connection, so a player who flew,
    /// watched or waited in the lobby all have them.
    pub fn results(&self) -> Option<&Results> {
        self.results.as_deref()
    }
}

/// A status in a word.
pub fn status_word(status: ResultStatus) -> &'static str {
    match status {
        ResultStatus::Alive => "alive",
        ResultStatus::Ejected => "ejected",
        ResultStatus::Dead => "dead",
        ResultStatus::Retired => "retired",
    }
}

/// Everything a row's pilot launched and hit, over the three kinds.
pub fn shots(row: &ResultRow) -> (u32, u32) {
    let all = [row.air_to_air, row.gun, row.air_to_ground];
    (
        all.iter().map(|s| s.launched).sum(),
        all.iter().map(|s| s.hit).sum(),
    )
}

/// One line for a log or a bot's output: how many aircraft, how many the
/// players flew, then each plane with its pilot, fate and the aircraft it
/// shot down. In PvP the
/// winner follows, in the words of the final scores.
pub fn summary(results: &Results) -> String {
    let flown = results
        .rows
        .iter()
        .filter(|row| row.callsign.is_some())
        .count();
    let rows: Vec<String> = results
        .rows
        .iter()
        .map(|row| {
            format!(
                "{} {} {} {}k",
                row.plane,
                row.callsign.as_deref().unwrap_or("AI"),
                status_word(row.status),
                row.aircraft_kills
            )
        })
        .collect();
    let mut line = format!(
        "{} aircraft, {flown} flown by players: {}",
        results.rows.len(),
        rows.join(", ")
    );
    if let Some(winner) = results.scores.as_ref().and_then(super::scores::winner_text) {
        line.push_str(&format!("; {}", winner.to_lowercase()));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::messages::{EndReason, Shots};
    use tore_formats::aircraft::AircraftId;
    use tore_sim::ai::launch::{Side, WingId};

    fn row(plane: u32, callsign: Option<&str>, status: ResultStatus) -> ResultRow {
        ResultRow {
            plane,
            wing: WingId {
                side: Side::Friendly,
                index: 0,
            },
            member: plane as u8,
            aircraft: AircraftId::F18,
            callsign: callsign.map(str::to_owned),
            status,
            damage: 0,
            aircraft_kills: 1,
            other_kills: 1,
            friendly_fire: 0,
            air_to_air: Shots {
                launched: 4,
                hit: 1,
            },
            gun: Shots {
                launched: 100,
                hit: 7,
            },
            air_to_ground: Shots::default(),
        }
    }

    #[test]
    fn a_summary_names_each_plane_its_pilot_and_fate() {
        let results = Results {
            reason: EndReason::TimeLimit,
            rows: vec![
                row(0, Some("Viper"), ResultStatus::Dead),
                row(1, None, ResultStatus::Alive),
                row(2, Some("Hawk"), ResultStatus::Retired),
            ],
            scores: None,
        };
        assert_eq!(
            summary(&results),
            "3 aircraft, 2 flown by players: 0 Viper dead 1k, 1 AI alive 1k, 2 Hawk retired 1k"
        );
        assert_eq!(shots(&results.rows[0]), (104, 8));
    }
}

/// The real client against the host on the network simulator: the results
/// arrive with the mission's end, before the debrief and with the callsigns
/// of the players, are kept, and give way to the next flight.
#[cfg(test)]
mod rig_tests {
    use super::super::tests::{Rig, spec, weave};
    use crate::client::{ClientEvent, ClientPhase};
    use crate::host::{AfterEnd, HostConfig};
    use crate::wire::messages::EndReason;
    use std::time::Duration;
    use tore_net::sim::LinkConfig;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn the_client_keeps_the_results_until_the_next_flight() {
        let mut rig = Rig::with_config(
            spec(2, 2, 50),
            LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
            5,
            |config: &mut HostConfig| {
                config.time_limit = Some(Duration::from_secs(3));
                config.after_end = AfterEnd::Restart;
                config.restart_delay = Duration::from_secs(2);
            },
        );
        let viper = rig.join(|_| {}, Box::new(|now, _, _| weave(now.as_secs_f64())));
        let hawk = rig.join(
            |c| c.callsign = "Hawk".into(),
            Box::new(|now, _, _| weave(now.as_secs_f64())),
        );
        assert!(rig.players[viper].client.results().is_none());
        assert!(rig.run_until(Duration::from_secs(8), |r| {
            [viper, hawk]
                .iter()
                .all(|&p| r.players[p].client.results().is_some())
        }));
        let results = rig.players[viper].client.results().unwrap();
        assert_eq!(results.reason, EndReason::TimeLimit);
        // Four planes; the two players' callsigns are on their rows.
        assert_eq!(results.rows.len(), 4);
        let mut flown: Vec<&str> = results
            .rows
            .iter()
            .filter_map(|r| r.callsign.as_deref())
            .collect();
        flown.sort_unstable();
        assert_eq!(flown, ["Hawk", "Viper"]);
        // It came before the end and the debrief, as one event.
        let events = &rig.players[viper].events;
        let results_at = events
            .iter()
            .position(|e| matches!(e, ClientEvent::Results(_)))
            .unwrap();
        let ended_at = events
            .iter()
            .position(|e| matches!(e, ClientEvent::MissionEnded(_)));
        assert!(ended_at.is_none_or(|ended| results_at < ended));
        // The next flight starts afresh.
        assert!(rig.run_until(Duration::from_secs(6), |r| {
            r.players[viper].client.phase() == ClientPhase::Flying && r.host.world().tick() > 0
        }));
        assert!(rig.players[viper].client.results().is_none());
    }
}
