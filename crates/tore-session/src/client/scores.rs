//! The scores a player's game keeps (stage F phase 2, slice F2-S;
//! docs/ARCHITECTURE.md, "Scoring"): the newest Scores message the host
//! sent, with the time left counted down from its arrival, and the words the
//! score board and the bots use for them.

use super::Client;
use crate::settings::{KillOwner, ScoreTally};
use crate::wire::messages::{Scores, Winner};
use std::time::Duration;
use tore_sim::ai::launch::Side;

/// The newest scores and when they arrived (the client's clock).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Kept {
    pub scores: Scores,
    pub received: Duration,
}

impl Client {
    /// The newest scores the host sent for the mission flying, if any yet.
    pub fn scores(&self) -> Option<&Scores> {
        self.scores.as_ref().map(|kept| &kept.scores)
    }

    /// The seconds left of the mission's time limit: the scores' figure,
    /// counted down since they arrived. `None` without a time limit or
    /// scores.
    pub fn seconds_left(&self) -> Option<u32> {
        let kept = self.scores.as_ref()?;
        let gone = self.now.saturating_sub(kept.received).as_secs();
        kept.scores
            .seconds_left
            .map(|left| left.saturating_sub(u32::try_from(gone).unwrap_or(u32::MAX)))
    }
}

/// The score board's heading for a tally, retail's three.
pub fn heading(tally: ScoreTally) -> &'static str {
    match tally {
        ScoreTally::Kills => "PLAYERS RANKED BY KILLS",
        ScoreTally::Ratio => "PLAYERS RANKED BY KILL RATIO",
        ScoreTally::Damage => "PLAYERS RANKED BY TOTAL DAMAGE",
    }
}

/// A side's name in a sentence.
pub fn side_name(side: Side) -> &'static str {
    match side {
        Side::Friendly => "friendly",
        Side::Enemy => "enemy",
    }
}

/// Kills over losses, or kills alone with no loss, as the host ranks them.
pub fn ratio(kills: u32, losses: u32) -> f64 {
    if losses == 0 {
        f64::from(kills)
    } else {
        f64::from(kills) / f64::from(losses)
    }
}

/// Minutes and seconds: "9:05".
pub fn clock(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// The kill limit in words, when there is one: "First side to 5 kills".
pub fn limit_text(scores: &Scores) -> Option<String> {
    let limit = scores.kill_limit;
    if limit == 0 {
        return None;
    }
    let kills = if limit == 1 { "kill" } else { "kills" };
    Some(match scores.kill_owner {
        KillOwner::Total => format!("Ends at {limit} {kills} in all"),
        KillOwner::Side => format!("First side to {limit} {kills}"),
        KillOwner::Player => format!("First player to {limit} {kills}"),
    })
}

/// Who won, in words, once the host has said: "The friendly side wins",
/// "Hawk wins", "A draw".
pub fn winner_text(scores: &Scores) -> Option<String> {
    match scores.winner {
        Winner::NoneYet => None,
        Winner::Side(side) => Some(format!("The {} side wins", side_name(side))),
        Winner::Player(id) => Some(scores.players.iter().find(|p| p.id == id).map_or_else(
            || "A player who left wins".to_owned(),
            |p| format!("{} wins", p.callsign),
        )),
        Winner::Draw => Some("A draw".to_owned()),
    }
}

/// One line for a log or a bot's output: the heading, each player in order
/// with its side, kills, losses and damage, the sides, the time left and the
/// winner.
pub fn summary(scores: &Scores) -> String {
    let players: Vec<String> = scores
        .players
        .iter()
        .enumerate()
        .map(|(n, p)| {
            format!(
                "{} {}{} {}/{} {:.2}",
                n + 1,
                p.callsign,
                p.side
                    .map_or(String::new(), |s| format!(" ({})", side_name(s))),
                p.kills,
                p.losses,
                f64::from(p.damage) / 1000.
            )
        })
        .collect();
    let [friendly, enemy] = scores.sides;
    let mut line = format!(
        "{}: {}; sides {}/{} to {}/{}",
        heading(scores.tally).to_lowercase(),
        if players.is_empty() {
            "nobody".to_owned()
        } else {
            players.join(", ")
        },
        friendly.kills,
        friendly.losses,
        enemy.kills,
        enemy.losses,
    );
    if let Some(left) = scores.seconds_left {
        line.push_str(&format!("; {} left", clock(left)));
    }
    if let Some(limit) = limit_text(scores) {
        line.push_str(&format!("; {}", limit.to_lowercase()));
    }
    if let Some(winner) = winner_text(scores) {
        line.push_str(&format!("; {}", winner.to_lowercase()));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Fight;
    use crate::wire::messages::{PlayerScore, SideScore};

    fn scores() -> Scores {
        Scores {
            tally: ScoreTally::Kills,
            fight: Fight::Sides,
            seconds_left: Some(545),
            kill_limit: 5,
            kill_owner: KillOwner::Side,
            players: vec![
                PlayerScore {
                    id: 2,
                    callsign: "Hawk".into(),
                    side: Some(Side::Enemy),
                    kills: 3,
                    losses: 1,
                    damage: 2_250,
                },
                PlayerScore {
                    id: 0,
                    callsign: "Viper".into(),
                    side: None,
                    kills: 0,
                    losses: 0,
                    damage: 0,
                },
            ],
            sides: [
                SideScore::default(),
                SideScore {
                    kills: 3,
                    losses: 1,
                    damage: 2_250,
                },
            ],
            winner: Winner::NoneYet,
        }
    }

    #[test]
    fn the_words_for_scores() {
        let mut scores = scores();
        assert_eq!(
            summary(&scores),
            "players ranked by kills: 1 Hawk (enemy) 3/1 2.25, 2 Viper 0/0 0.00; \
             sides 0/0 to 3/1; 9:05 left; first side to 5 kills"
        );
        assert_eq!(heading(ScoreTally::Ratio), "PLAYERS RANKED BY KILL RATIO");
        assert_eq!(
            heading(ScoreTally::Damage),
            "PLAYERS RANKED BY TOTAL DAMAGE"
        );
        scores.kill_limit = 1;
        scores.kill_owner = KillOwner::Total;
        assert_eq!(limit_text(&scores).unwrap(), "Ends at 1 kill in all");
        scores.kill_owner = KillOwner::Player;
        assert_eq!(limit_text(&scores).unwrap(), "First player to 1 kill");
        scores.kill_limit = 0;
        assert_eq!(limit_text(&scores), None);
        for (winner, text) in [
            (Winner::Side(Side::Friendly), "The friendly side wins"),
            (Winner::Player(2), "Hawk wins"),
            (Winner::Player(9), "A player who left wins"),
            (Winner::Draw, "A draw"),
        ] {
            scores.winner = winner;
            assert_eq!(winner_text(&scores).unwrap(), text);
        }
        assert_eq!(ratio(3, 0), 3.);
        assert_eq!(ratio(3, 2), 1.5);
        assert_eq!(clock(60), "1:00");
    }
}

/// The real client against the host on the network simulator: the scores
/// arrive, are kept, count their time down and start afresh with the next
/// flight.
#[cfg(test)]
mod rig_tests {
    use super::super::tests::{Rig, spec, weave};
    use super::*;
    use crate::client::{ClientEvent, ClientPhase};
    use crate::host::{AfterEnd, HostConfig};
    use tore_net::sim::LinkConfig;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn the_client_keeps_the_newest_scores_and_counts_the_time_down() {
        let mut rig = Rig::with_config(
            spec(2, 2, 50),
            LinkConfig::for_round_trip(40 * MS, 0., 0., 0.),
            5,
            |config: &mut HostConfig| {
                config.time_limit = Some(Duration::from_secs(6));
                config.after_end = AfterEnd::Restart;
                config.restart_delay = Duration::from_secs(1);
            },
        );
        let viper = rig.join(|_| {}, Box::new(|now, _, _| weave(now.as_secs_f64())));
        let hawk = rig.join(
            |c| c.callsign = "Hawk".into(),
            Box::new(|now, _, _| weave(now.as_secs_f64())),
        );
        assert!(rig.run_until(Duration::from_secs(4), |r| {
            [viper, hawk]
                .iter()
                .all(|&p| r.players[p].client.scores().is_some())
        }));
        let client = &rig.players[viper].client;
        let scores = client.scores().unwrap();
        let mut callsigns: Vec<&str> = scores.players.iter().map(|p| p.callsign.as_str()).collect();
        callsigns.sort_unstable();
        assert_eq!(callsigns, ["Hawk", "Viper"]);
        assert_eq!(scores.winner, Winner::NoneYet);
        let left = client.seconds_left().unwrap();
        assert!(left <= 6, "{left}");
        // Nothing changes, so nothing more arrives: the client counts down.
        rig.run(Duration::from_millis(2000));
        let later = rig.players[viper].client.seconds_left().unwrap();
        assert!(
            later + 2 <= left && later + 3 >= left,
            "{left} then {later}"
        );
        // The time limit ends the mission; the final scores came with it.
        assert!(rig.run_until(Duration::from_secs(6), |r| {
            r.players[viper]
                .events
                .iter()
                .any(|e| matches!(e, ClientEvent::MissionEnded(_)))
        }));
        let finals = rig.players[viper]
            .events
            .iter()
            .filter(|e| matches!(e, ClientEvent::Scores(_)))
            .count();
        assert!(finals >= 2, "the first scores and the final ones");
        assert_eq!(rig.players[viper].client.seconds_left(), Some(0));
        // The next flight starts its scores afresh.
        assert!(rig.run_until(Duration::from_secs(4), |r| {
            r.players[viper].client.phase() == ClientPhase::Flying && r.host.world().tick() > 0
        }));
        assert!(
            rig.players[viper]
                .client
                .scores()
                .is_none_or(|s| s.seconds_left.is_some_and(|left| left > 3))
        );
    }
}
