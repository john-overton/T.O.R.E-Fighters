//! The client's mission record survives a revival, a leave and a broken link.
//! Reliable ongoing reports supply counters the render snapshots do not have.
use crate::wire::events::{ReceivedEvent, WireEvent};
use crate::wire::messages::{FlightReport, ResultStatus};
use tore_formats::aircraft::AircraftId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sortie {
    pub plane: u32,
    pub aircraft: AircraftId,
    pub started: u32,
    pub ended: Option<(u32, ResultStatus)>,
}

#[derive(Clone, Debug, Default)]
pub struct Record {
    pub finished: bool,
    pub debrief_seen: bool,
    pub latest: Option<FlightReport>,
    pub sorties: Vec<Sortie>,
    pub events: Vec<(u32, String)>,
    /// Newest received simulation tick, never wall time during an outage.
    pub tick: u32,
}

impl Record {
    pub(super) fn seated(&mut self, plane: u32, aircraft: AircraftId, tick: u32) {
        self.tick = self.tick.max(tick);
        if self.sorties.last().is_some_and(|s| s.plane == plane) {
            return;
        }
        if self.sorties.len() < 1024 {
            self.sorties.push(Sortie {
                plane,
                aircraft,
                started: tick,
                ended: None,
            });
        }
    }

    pub(super) fn receive(&mut self, report: FlightReport) {
        if self
            .latest
            .as_ref()
            .is_some_and(|old| old.tick > report.tick)
        {
            return;
        }
        self.tick = self.tick.max(report.tick);
        for sortie in &mut self.sorties {
            if sortie.ended.is_none()
                && let Some(row) = report.results.rows.iter().find(|r| r.plane == sortie.plane)
                && row.status != ResultStatus::Alive
            {
                sortie.ended = Some((report.tick, row.status));
            }
        }
        self.latest = Some(report);
    }

    pub(super) fn event(&mut self, event: &ReceivedEvent) {
        if self.sorties.is_empty() {
            return;
        }
        let text = match &event.event {
            WireEvent::Message { text } => text.clone(),
            WireEvent::Radio {
                label,
                text,
                important: true,
                ..
            } => format!("{label}: {text}"),
            _ => return,
        };
        // Bound a long-running server's local history. Totals and sorties
        // remain independent of these most recent mission messages.
        if self.events.len() == 4096 {
            self.events.remove(0);
        }
        self.events.push((event.tick, text));
    }
}

impl super::Client {
    pub fn flight_record(&self) -> &Record {
        &self.flight_record
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::tests::{Rig, level_script, spec};
    use crate::client::{ClientEvent, ClientPhase};
    use crate::settings::{Respawn, number};
    use std::time::Duration;
    use tore_net::sim::LinkConfig;
    use tore_sim::combat::ledger::{Kill, Resolution, ShotKind};

    fn flown() -> (Rig, usize) {
        let mut rig = Rig::new(
            spec(2, 4, 50),
            LinkConfig::one_way(Duration::from_millis(5)),
            13,
        );
        rig.host
            .settings_for_test()
            .apply(&[
                (number::RESPAWN, Respawn::Revive.value()),
                (number::LIVES, 255),
                (number::AI_RESPAWN, 0),
            ])
            .unwrap();
        let player = rig.join(|c| c.plane = Some(0), level_script());
        assert!(rig.run_until(Duration::from_secs(5), |r| r.seated(player)));
        for life in 0..3 {
            let plane = rig.players[player].client.seat().unwrap().1;
            let world = rig.host.world_for_test();
            let victim = 3 + life;
            world
                .combat
                .state
                .ledger
                .launch(100 + life, plane.0, Some(victim), ShotKind::AirToAir);
            world
                .combat
                .state
                .ledger
                .resolve(100 + life, Resolution::Hit(100));
            world.combat.state.ledger.kill(Kill {
                owner: plane.0,
                victim,
                category: 0x8000,
                aircraft: true,
            });
            world
                .combat
                .state
                .targets
                .iter_mut()
                .find(|t| t.id == victim)
                .unwrap()
                .hp = 0;
            rig.run(Duration::from_millis(1300));
            if life < 2 {
                rig.host
                    .world_for_test()
                    .cockpits
                    .iter_mut()
                    .find(|c| c.plane == plane)
                    .unwrap()
                    .flight
                    .systems
                    .pilot
                    .dead = true;
                assert!(rig.run_until(Duration::from_secs(2), |r| {
                    r.players[player].client.revival().is_some()
                }));
                rig.players[player].client.revive();
                assert!(rig.run_until(Duration::from_secs(3), |r| {
                    r.players[player]
                        .client
                        .seat()
                        .is_some_and(|(_, p)| p != plane)
                }));
            }
        }
        (rig, player)
    }

    fn check(record: &Record) {
        let latest = record.latest.as_ref().unwrap();
        assert_eq!(latest.debrief.player.kills[0], 3);
        assert_eq!(latest.debrief.player.air_to_air.hit, 3);
        assert_eq!(latest.debrief.player.air_to_air.launched, 3);
        assert_eq!(record.sorties.len(), 3);
        assert!(record.sorties[..2].iter().all(|s| s.ended.is_some()));
        assert!(record.sorties[2].ended.is_none());
        assert_eq!(latest.results.scores.as_ref().unwrap().players[0].losses, 2);
    }

    #[test]
    fn early_leave_keeps_three_lives_without_final_results() {
        let (mut rig, player) = flown();
        rig.players[player].client.leave_game(rig.net.now());
        assert!(
            rig.run_until(Duration::from_secs(3), |r| r.players[player].client.phase()
                == ClientPhase::Closed)
        );
        let client = &rig.players[player].client;
        assert!(client.results().is_none());
        check(client.flight_record());
        assert!(
            rig.players[player]
                .events
                .iter()
                .any(|e| matches!(e, ClientEvent::Debrief(d) if d.player.kills[0] == 3))
        );
    }

    #[test]
    fn a_dropped_connection_keeps_the_received_report() {
        let (mut rig, player) = flown();
        rig.net.set_default_link(LinkConfig {
            loss: 1.,
            ..LinkConfig::one_way(Duration::ZERO)
        });
        assert!(rig.run_until(
            Duration::from_secs(40),
            |r| r.players[player].client.phase() == ClientPhase::Closed
        ));
        assert!(
            !rig.players[player]
                .events
                .iter()
                .any(|e| matches!(e, ClientEvent::Debrief(_)))
        );
        check(rig.players[player].client.flight_record());
    }

    #[test]
    fn normal_end_still_delivers_full_results_and_cumulative_debrief() {
        let (mut rig, player) = flown();
        rig.host.end();
        rig.run(Duration::from_secs(1));
        let client = &rig.players[player].client;
        let results = client.results().unwrap();
        assert_eq!(results.rows.len(), 8);
        assert_eq!(
            results.scores.as_ref().unwrap().winner,
            crate::wire::messages::Winner::NoneYet
        );
        assert!(rig.players[player].events.iter().any(|e| matches!(e, ClientEvent::Debrief(d) if d.player.kills[0] == 3 && d.player.air_to_air.hit == 3)));
        check(client.flight_record());
    }
}
