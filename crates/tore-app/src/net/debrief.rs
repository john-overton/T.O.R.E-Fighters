//! The debrief of a networked flight: the report the host sends when the
//! player leaves or the mission ends, in the form the debrief screen reads.
//! The host builds it from its own ledger (`tore_world::debrief::capture`);
//! the wire's [`Debrief`] mirrors [`Report`] field for field.
use crate::debrief::{Ended, Objective, Outcome, Pilot, Report, Status};
use tore_formats::aircraft::AircraftId;
use tore_session::client::results::{shots, status_word};
use tore_session::client::scores::{heading, ratio, side_name, winner_text};
use tore_session::settings::Fight;
use tore_session::wire::messages::{
    Debrief, DebriefObjective, DebriefPilot, EndReason, LobbyState, PilotStatus, ResultRow,
    Results, Scores,
};
use tore_sim::ai::launch::Side;

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

/// Rows (or players) to a page of the multiplayer pages.
pub const PER_PAGE: usize = 15;

/// Where each column of the SCORES and RESULTS pages starts, in the debrief
/// screen's pixels. The clipboard's text column runs from 294 to about 580.
const SCORE_COLUMNS: [i32; 7] = [294, 312, 374, 420, 458, 490, 538];
const RESULT_COLUMNS: [i32; 7] = [294, 324, 388, 434, 482, 520, 548];
/// The longest callsign a column shows before it is cut.
const SCORE_NAME: usize = 9;
const RESULT_NAME: usize = 9;

/// `text` cut to `limit` characters, with a dot where it was cut.
fn cut(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_owned()
    } else {
        text.chars()
            .take(limit.saturating_sub(1))
            .collect::<String>()
            + "."
    }
}

fn columns(xs: &[i32]) -> String {
    let xs: Vec<String> = xs.iter().map(i32::to_string).collect();
    format!(".columns {}", xs.join(" "))
}

/// A heading line in the clipboard markup, as the retail pages draw theirs.
fn title(text: &str) -> Vec<String> {
    [
        ".center",
        ".underline",
        ".bold",
        text,
        "..underline",
        "..bold",
        ".left",
    ]
    .map(str::to_owned)
    .to_vec()
}

fn bold(text: String) -> Vec<String> {
    vec![".bold".to_owned(), text, "..bold".to_owned()]
}

/// A side in a column: the board's words cut to fit beside the figures.
fn side_word(side: Side) -> &'static str {
    match side {
        Side::Friendly => "FRIEND",
        Side::Enemy => "ENEMY",
    }
}

/// The SCORES pages of a PvP mission's results: the winner, the players
/// ranked as the tally ranks them with their side, kills, losses, damage in
/// aircraft and ratio, fifteen to a page, and each side's totals when the
/// fight is by sides, on the last. Empty without scores (co-op).
pub fn scores_pages(scores: &Scores) -> Vec<Vec<String>> {
    let chunks: Vec<&[tore_session::wire::messages::PlayerScore]> = if scores.players.is_empty() {
        vec![&[]]
    } else {
        scores.players.chunks(PER_PAGE).collect()
    };
    let last = chunks.len() - 1;
    let mut rank = 0;
    chunks
        .iter()
        .enumerate()
        .map(|(page, players)| {
            let mut lines = title("SCORES");
            lines.push(columns(&SCORE_COLUMNS));
            if page > 0 {
                lines.extend([String::new(), String::new()]);
            } else {
                lines.push(match winner_text(scores) {
                    Some(winner) => format!("{winner}."),
                    None => "No winner was named.".to_owned(),
                });
                lines.push(String::new());
            }
            lines.extend(bold(heading(scores.tally).to_owned()));
            lines.extend(bold(
                ["", "PILOT", "SIDE", "KILLS", "LOST", "DAMAGE", "RATIO"].join("\t"),
            ));
            for player in *players {
                rank += 1;
                lines.push(
                    [
                        rank.to_string(),
                        cut(&player.callsign, SCORE_NAME),
                        player.side.map_or("-", side_word).to_owned(),
                        player.kills.to_string(),
                        player.losses.to_string(),
                        format!("{:.2}", f64::from(player.damage) / 1000.),
                        format!("{:.2}", ratio(player.kills, player.losses)),
                    ]
                    .join("\t"),
                );
            }
            if scores.players.is_empty() {
                lines.push("No players".to_owned());
            }
            if page == last && scores.fight == Fight::Sides {
                lines.push(String::new());
                for (side, tally) in [Side::Friendly, Side::Enemy].into_iter().zip(scores.sides) {
                    lines.push(
                        [
                            String::new(),
                            format!("{} SIDE", side_name(side).to_uppercase()),
                            String::new(),
                            tally.kills.to_string(),
                            tally.losses.to_string(),
                            format!("{:.2}", f64::from(tally.damage) / 1000.),
                            format!("{:.2}", ratio(tally.kills, tally.losses)),
                        ]
                        .join("\t"),
                    );
                }
            }
            lines
        })
        .collect()
}

/// An aircraft's short name for a column: "F/A-18D", "Rafale", "MiG-29".
fn short_name(aircraft: AircraftId) -> &'static str {
    aircraft.label().split(' ').next().unwrap_or("")
}

/// One RESULTS row: the wing and member, the callsign or AI, the aircraft,
/// its fate, aircraft shot down (as the scores count them; the row's other
/// kills are not on the page), hit percentage over every shot and damage.
fn result_line(row: &ResultRow) -> String {
    let (launched, hit) = shots(row);
    let hits = if launched == 0 {
        "-".to_owned()
    } else {
        format!(
            "{}%",
            u64::from(hit.min(launched)) * 100 / u64::from(launched)
        )
    };
    let kills = row.aircraft_kills;
    [
        format!(
            "{}-{}",
            u32::from(row.wing.index) + 1,
            u32::from(row.member) + 1
        ),
        row.callsign
            .as_deref()
            .map_or_else(|| "AI".to_owned(), |name| cut(name, RESULT_NAME)),
        short_name(row.aircraft).to_owned(),
        {
            let mut status = status_word(row.status).to_owned();
            status[..1].make_ascii_uppercase();
            status
        },
        if kills == 0 {
            "-".to_owned()
        } else {
            kills.to_string()
        },
        hits,
        format!("{}%", row.damage / 10),
    ]
    .join("\t")
}

/// The RESULTS pages: every aircraft of the mission, human or AI, retired
/// ones included, a side to a page (the friendly side first) and fifteen to a
/// page, in the order of the rows (wing, then member). Agent decision: a page
/// never mixes the sides, so its heading names its side.
pub fn results_pages(results: &Results) -> Vec<Vec<String>> {
    let mut pages = Vec::new();
    for side in [Side::Friendly, Side::Enemy] {
        let rows: Vec<&ResultRow> = results
            .rows
            .iter()
            .filter(|row| row.wing.side == side)
            .collect();
        for chunk in rows.chunks(PER_PAGE) {
            let mut lines = title(&format!(
                "RESULTS : {} SIDE",
                side_name(side).to_uppercase()
            ));
            lines.push(columns(&RESULT_COLUMNS));
            lines.push(String::new());
            lines.extend(bold(
                ["WING", "PILOT", "TYPE", "STATUS", "KILLS", "HIT", "DMG"].join("\t"),
            ));
            lines.extend(chunk.iter().map(|row| result_line(row)));
            pages.push(lines);
        }
    }
    pages
}

/// The pages a networked debrief turns in after its first: SCORES (PvP
/// only), then RESULTS.
pub fn extra_pages(results: &Results) -> Vec<Vec<String>> {
    let mut pages = results
        .scores
        .as_ref()
        .map(scores_pages)
        .unwrap_or_default();
    pages.extend(results_pages(results));
    pages
}

/// A mission's results for the tests and the renders: `per_side` aircraft
/// on each side in wings of five (so up to three wings), the first `humans`
/// of each side flown by players with long and short callsigns, a mix of
/// fates and some kills, and a PvP set of final scores.
#[cfg(test)]
pub(crate) fn sample_results(per_side: usize, humans: usize) -> Results {
    use tore_session::settings::{KillOwner, ScoreTally};
    use tore_session::wire::messages::{
        EndReason, PlayerScore, ResultStatus, Shots, SideScore, Winner,
    };
    use tore_sim::ai::launch::WingId;
    let names = [
        "Viper",
        "Cobra",
        "Hawk",
        "Maverick",
        "Goose",
        "Iceman",
        "Slider",
        "Hollywood",
        "Wolfman",
        "Jester",
        "Merlin",
        "Sundown",
        "Stinger",
        "Chipper",
        "Lonestar",
        "ABCDEFGHIJKLMNO",
    ];
    let mut rows = Vec::new();
    let mut players = Vec::new();
    for (s, side) in [Side::Friendly, Side::Enemy].into_iter().enumerate() {
        for n in 0..per_side {
            let plane = (s * per_side + n) as u32;
            let flown = n < humans;
            let status = [
                ResultStatus::Alive,
                ResultStatus::Ejected,
                ResultStatus::Dead,
                ResultStatus::Retired,
            ][n % 4];
            rows.push(ResultRow {
                plane,
                wing: WingId {
                    side,
                    index: (n / 5) as u8,
                },
                member: (n % 5) as u8,
                aircraft: [
                    AircraftId::F18,
                    AircraftId::Mig29,
                    AircraftId::Rafale,
                    AircraftId::Su27,
                ][n % 4],
                callsign: flown.then(|| names[(s * humans + n) % names.len()].to_owned()),
                status,
                damage: [0, 1000, 640, 1000][n % 4],
                aircraft_kills: (n % 3) as u32,
                other_kills: (n % 2) as u32,
                friendly_fire: 0,
                air_to_air: Shots {
                    launched: 8 * (n as u32 % 3),
                    hit: 3 * (n as u32 % 3),
                },
                gun: Shots {
                    launched: 120 * (n as u32 % 2),
                    hit: 17 * (n as u32 % 2),
                },
                air_to_ground: Shots::default(),
            });
            if flown {
                players.push(PlayerScore {
                    id: plane as u8,
                    callsign: names[(s * humans + n) % names.len()].to_owned(),
                    side: Some(side),
                    kills: (n % 3) as u32 + 1,
                    losses: (n % 2) as u32,
                    damage: 1250 * (n as u32 + 1),
                });
            }
        }
    }
    Results {
        reason: EndReason::KillLimit,
        rows,
        scores: Some(Scores {
            tally: ScoreTally::Kills,
            fight: Fight::Sides,
            seconds_left: Some(120),
            kill_limit: 5,
            kill_owner: KillOwner::Side,
            players,
            sides: [
                SideScore {
                    kills: 5,
                    losses: 2,
                    damage: 6_500,
                },
                SideScore {
                    kills: 2,
                    losses: 5,
                    damage: 3_250,
                },
            ],
            winner: Winner::Side(Side::Friendly),
        }),
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
            mission_locked: false,
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

    #[test]
    fn a_result_row_reads_as_one_line_of_cells() {
        let results = sample_results(15, 2);
        let line = |plane: u32| {
            let row = results.rows.iter().find(|r| r.plane == plane).unwrap();
            result_line(row)
        };
        // A player's plane: wing 1 member 1, alive, nothing hit or killed.
        assert_eq!(line(0), "1-1\tViper\tF/A-18D\tAlive\t-\t-\t0%");
        // Ejected, two kills, 3 of 8 missiles and 17 of 120 rounds: 20 of 128.
        assert_eq!(line(1), "1-2\tCobra\tMiG-29\tEjected\t1\t15%\t100%");
        // An AI plane has no callsign; its damage shows in whole percent.
        assert_eq!(line(2), "1-3\tAI\tRafale\tDead\t2\t37%\t64%");
        assert_eq!(line(3).split('\t').nth(3), Some("Retired"));
        // A long callsign is cut to its column.
        let mut row = results.rows[0].clone();
        row.callsign = Some("ABCDEFGHIJKLMNO".into());
        assert_eq!(result_line(&row).split('\t').nth(1), Some("ABCDEFGH."));
    }

    #[test]
    fn results_pages_hold_every_aircraft_a_side_to_a_page() {
        let results = sample_results(15, 6);
        let pages = results_pages(&results);
        assert_eq!(pages.len(), 2);
        // Fifteen rows and the column heads each, six tabs to a line.
        for (page, side) in pages.iter().zip(["FRIENDLY", "ENEMY"]) {
            assert!(page.contains(&format!("RESULTS : {side} SIDE")));
            assert_eq!(
                page.iter().filter(|l| l.matches('\t').count() == 6).count(),
                16
            );
        }
        // Retired planes are listed too.
        assert!(pages[0].iter().any(|l| l.contains("Retired")));
        // No aircraft, no pages; a mission of one side alone has one.
        let mut none = results.clone();
        none.rows.clear();
        assert!(results_pages(&none).is_empty());
        none.rows = results
            .rows
            .iter()
            .filter(|r| r.wing.side == Side::Enemy)
            .take(3)
            .cloned()
            .collect();
        assert_eq!(results_pages(&none).len(), 1);
    }

    #[test]
    fn the_scores_page_names_the_winner_ranks_the_players_and_totals_the_sides() {
        let results = sample_results(15, 3);
        let pages = scores_pages(results.scores.as_ref().unwrap());
        assert_eq!(pages.len(), 1);
        let page = &pages[0];
        assert!(page.contains(&"The friendly side wins.".to_string()));
        assert!(page.contains(&"PLAYERS RANKED BY KILLS".to_string()));
        // Rank, pilot, side, kills, lost, damage in aircraft, ratio.
        assert!(page.contains(&"1\tViper\tFRIEND\t1\t0\t1.25\t1.00".to_string()));
        assert!(page.contains(&"4\tMaverick\tENEMY\t1\t0\t1.25\t1.00".to_string()));
        assert!(page.contains(&"\tFRIENDLY SIDE\t\t5\t2\t6.50\t2.50".to_string()));
        assert!(page.contains(&"\tENEMY SIDE\t\t2\t5\t3.25\t0.40".to_string()));
        // A mission ended with no winner says so; a free-for-all has no
        // side totals.
        let mut scores = results.scores.clone().unwrap();
        scores.winner = tore_session::wire::messages::Winner::NoneYet;
        scores.fight = Fight::FreeForAll;
        let page = &scores_pages(&scores)[0];
        assert!(page.contains(&"No winner was named.".to_string()));
        assert!(!page.iter().any(|l| l.contains(" SIDE\t")));
        // Nobody flew: the page says so.
        scores.players.clear();
        assert!(scores_pages(&scores)[0].contains(&"No players".to_string()));
    }

    #[test]
    fn co_op_results_add_only_the_results_pages() {
        let mut results = sample_results(4, 2);
        let with_scores = extra_pages(&results).len();
        results.scores = None;
        let co_op = extra_pages(&results);
        assert_eq!(with_scores, co_op.len() + 1);
        assert!(co_op.iter().all(|p| !p.contains(&"SCORES".to_string())));
        assert!(co_op[0].contains(&"RESULTS : FRIENDLY SIDE".to_string()));
    }
}
