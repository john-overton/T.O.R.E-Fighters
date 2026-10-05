//! The debrief of a networked flight: the report the host sends when the
//! player leaves or the mission ends, in the form the debrief screen reads.
//! The host builds it from its own ledger (`tore_world::debrief::capture`);
//! the wire's [`Debrief`] mirrors [`Report`] field for field.
use crate::debrief::{Ended, Objective, Outcome, Pilot, Report, Status};
use tore_session::wire::messages::{
    Debrief, DebriefObjective, DebriefPilot, EndReason, LobbyState, PilotStatus,
};

/// Who ended the flight the debrief reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    /// The player left its own flight.
    Left,
    /// The host ended the mission, for this reason.
    Host(EndReason),
}

/// What a networked debrief says when the mission's objectives did not
/// decide it, or `None` when they did and the result stands (agent
/// decision, EF-F). The host's report has only success or not: a mission
/// somebody ended before its objectives were met would read "MISSION
/// FAILURE / You failed this Quick Mission.", which is not what happened.
/// The result stands when the objectives were met, or when the flight was
/// lost: the player's pilot dead or ejected, or a friendly objective
/// destroyed. Anything else was ended, and the page says by whom. `lobby`
/// tells a game with a King from a dedicated server (which has none).
pub fn ending(debrief: &Debrief, cause: Cause, lobby: Option<&LobbyState>) -> Option<Ended> {
    let lost = debrief.player.status != PilotStatus::Alive
        || debrief.objectives.iter().any(|objective| {
            matches!(*objective, DebriefObjective::Protect { protected, total } if protected < total)
        });
    if debrief.success || lost {
        return None;
    }
    let king = lobby.is_some_and(|l| l.king.is_some());
    let you_king = lobby.is_some_and(LobbyState::is_king);
    let sentence = match cause {
        Cause::Left => "You left the mission.",
        Cause::Host(EndReason::EndedByServer) if you_king => "You ended the mission for everyone.",
        Cause::Host(EndReason::EndedByServer) if king => "The King ended the mission.",
        Cause::Host(EndReason::EndedByServer) => "The server ended the mission.",
        Cause::Host(EndReason::TimeLimit) => "The time limit ended the mission.",
        Cause::Host(EndReason::EveryoneLeft) => "Everyone left the mission.",
        Cause::Host(EndReason::ServerStopping) if king => "The host is stopping the game.",
        Cause::Host(EndReason::ServerStopping) => "The server is stopping.",
        Cause::Host(EndReason::HostLeft) => "The host left the game.",
        Cause::Host(EndReason::KillLimit) => "The kill limit ended the mission.",
    };
    Some(Ended {
        title: "MISSION ENDED".into(),
        sentence: sentence.into(),
    })
}

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

    fn wire(success: bool, status: PilotStatus, objectives: Vec<DebriefObjective>) -> Debrief {
        Debrief {
            success,
            objectives,
            player: DebriefPilot {
                status,
                ..DebriefPilot::default()
            },
            ..Debrief::default()
        }
    }

    fn lobby(king: Option<u8>, you: u8) -> LobbyState {
        LobbyState {
            name: String::new(),
            summary: String::new(),
            mission: 1,
            phase: tore_session::wire::messages::LobbyPhase::Flying,
            start: tore_session::wire::messages::StartRule::King,
            king,
            host: king,
            you,
            players: Vec::new(),
            slots: Vec::new(),
            settings: Vec::new(),
        }
    }

    fn says(ended: Option<Ended>) -> Option<(String, String)> {
        ended.map(|e| (e.title, e.sentence))
    }

    #[test]
    fn a_mission_nobody_decided_says_who_ended_it() {
        let open = wire(
            false,
            PilotStatus::Alive,
            vec![DebriefObjective::Destroy {
                destroyed: 1,
                total: 3,
            }],
        );
        let with_king = lobby(Some(0), 1);
        let king_self = lobby(Some(1), 1);
        let server = lobby(None, 1);
        let by = |cause, lobby: &LobbyState| says(ending(&open, cause, Some(lobby))).unwrap().1;
        assert_eq!(
            says(ending(&open, Cause::Left, Some(&with_king)))
                .unwrap()
                .0,
            "MISSION ENDED"
        );
        assert_eq!(by(Cause::Left, &with_king), "You left the mission.");
        let host = |reason| Cause::Host(reason);
        assert_eq!(
            by(host(EndReason::EndedByServer), &with_king),
            "The King ended the mission."
        );
        assert_eq!(
            by(host(EndReason::EndedByServer), &king_self),
            "You ended the mission for everyone."
        );
        assert_eq!(
            by(host(EndReason::EndedByServer), &server),
            "The server ended the mission."
        );
        assert_eq!(
            by(host(EndReason::TimeLimit), &server),
            "The time limit ended the mission."
        );
        assert_eq!(
            by(host(EndReason::HostLeft), &with_king),
            "The host left the game."
        );
        assert_eq!(
            by(host(EndReason::ServerStopping), &server),
            "The server is stopping."
        );
        // No lobby at all (a session that ended): the server's wording.
        assert_eq!(
            says(ending(&open, host(EndReason::EndedByServer), None))
                .unwrap()
                .1,
            "The server ended the mission."
        );
    }

    #[test]
    fn a_mission_its_objectives_decided_keeps_its_result() {
        let king = lobby(Some(0), 1);
        let cause = Cause::Host(EndReason::EndedByServer);
        // Success stands.
        let won = wire(true, PilotStatus::Alive, vec![]);
        assert_eq!(ending(&won, cause, Some(&king)), None);
        // A lost pilot stands as a failure, ejected or dead.
        for status in [PilotStatus::Dead, PilotStatus::Ejected] {
            let lost = wire(false, status, vec![]);
            assert_eq!(ending(&lost, Cause::Left, Some(&king)), None, "{status:?}");
        }
        // So does a friendly objective destroyed.
        let guarded = wire(
            false,
            PilotStatus::Alive,
            vec![DebriefObjective::Protect {
                protected: 0,
                total: 1,
            }],
        );
        assert_eq!(ending(&guarded, cause, Some(&king)), None);
        // A friendly objective still standing is not a failure.
        let standing = wire(
            false,
            PilotStatus::Alive,
            vec![DebriefObjective::Protect {
                protected: 1,
                total: 1,
            }],
        );
        assert!(ending(&standing, cause, Some(&king)).is_some());
    }
}
