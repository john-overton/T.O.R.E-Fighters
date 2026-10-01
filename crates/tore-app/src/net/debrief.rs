//! The debrief of a networked flight: the report the host sends when the
//! player leaves or the mission ends, in the form the debrief screen reads.
//! The host builds it from its own ledger (`tore_world::debrief::capture`);
//! the wire's [`Debrief`] mirrors [`Report`] field for field.
use crate::debrief::{Objective, Outcome, Pilot, Report, Status};
use tore_session::wire::messages::{Debrief, DebriefObjective, DebriefPilot, PilotStatus};

/// The report a received debrief stands for.
pub fn report(debrief: &Debrief) -> Report {
    Report {
        outcome: if debrief.success {
            Outcome::Success
        } else {
            Outcome::Failure
        },
        objectives: debrief
            .objectives
            .iter()
            .map(|objective| match *objective {
                DebriefObjective::Destroy { destroyed, total } => {
                    Objective::Destroy { destroyed, total }
                }
                DebriefObjective::Protect { protected, total } => {
                    Objective::Protect { protected, total }
                }
            })
            .collect(),
        elapsed_seconds: debrief.elapsed_seconds,
        player: pilot(&debrief.player),
        wingman: debrief.wingman.as_ref().map(pilot),
    }
}

fn pilot(pilot: &DebriefPilot) -> Pilot {
    Pilot {
        status: match pilot.status {
            PilotStatus::Alive => Status::Alive,
            PilotStatus::Ejected => Status::Ejected,
            PilotStatus::Dead => Status::Dead,
        },
        damage: pilot.damage,
        landing_grade: pilot.landing_grade,
        // The two losses the evaluator names; any other text a host sends
        // names no loss the page knows, so it shows none.
        cause: match pilot.cause.as_deref() {
            Some("overspeed") => Some("overspeed"),
            Some("out of bounds") => Some("out of bounds"),
            _ => None,
        },
        kills: pilot.kills,
        friendly_fire: pilot.friendly_fire,
        air_to_air: pilot.air_to_air,
        air_to_ground: pilot.air_to_ground,
        gun: pilot.gun,
        bombs: pilot.bombs,
        enemy_aam: pilot.enemy_aam,
        enemy_sam: pilot.enemy_sam,
        enemy_gun: pilot.enemy_gun,
        enemy_aaa: pilot.enemy_aaa,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::combat::ledger::Tally;

    fn full() -> Report {
        let tally = |n: u32| Tally {
            launched: n,
            hit: n / 2,
            damage: n * 3,
            missed: n / 4,
            spoofed: n / 5,
            jammed: n / 6,
        };
        let pilot = |seed: u32, cause| Pilot {
            status: [Status::Alive, Status::Ejected, Status::Dead][seed as usize % 3],
            damage: f64::from(seed) / 10.,
            landing_grade: Some(seed * 7),
            cause,
            kills: std::array::from_fn(|i| seed + i as u32),
            friendly_fire: seed,
            air_to_air: tally(seed * 8),
            air_to_ground: tally(seed * 4),
            gun: tally(seed * 12),
            bombs: tally(seed),
            enemy_aam: tally(seed * 2),
            enemy_sam: tally(seed * 3),
            enemy_gun: tally(seed * 5),
            enemy_aaa: tally(seed * 9),
        };
        Report {
            outcome: Outcome::Success,
            objectives: vec![
                Objective::Destroy {
                    destroyed: 2,
                    total: 3,
                },
                Objective::Protect {
                    protected: 1,
                    total: 1,
                },
            ],
            elapsed_seconds: 1_234,
            player: pilot(1, Some("overspeed")),
            wingman: Some(pilot(2, Some("out of bounds"))),
        }
    }

    #[test]
    fn a_report_survives_the_wire_in_both_directions() {
        for report_in in [full(), Report::sample(), Report::default()] {
            let wire = tore_session::host::debrief_message(&report_in);
            assert_eq!(report(&wire), report_in);
        }
    }

    #[test]
    fn a_loss_the_page_does_not_know_shows_none() {
        let mut wire = tore_session::host::debrief_message(&full());
        wire.player.cause = Some("gremlins".into());
        assert_eq!(report(&wire).player.cause, None);
    }
}
