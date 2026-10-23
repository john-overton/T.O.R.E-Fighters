//! The scores part of the session's state (stage K slice K1;
//! docs/ARCHITECTURE.md, "What moves with the host"): the tallies by player
//! (join order) and side, who flew each plane last, the results' callsigns,
//! and where the Scores messages stand. Every field of [`Scoring`] is named,
//! so a field added to it fails to compile here until it is coded or
//! skipped with its class.

use super::super::state::{Result, load_option_side, save_option_side};
use super::{Player, Scoring, Tally};
use std::collections::BTreeMap;
use tore_sim::checkpoint::{Checkpoint, Loader, Saver};

tore_sim::checkpoint_struct!(Tally {
    kills,
    losses,
    damage,
});

fn save_player(s: &mut Saver, player: &Player) -> Result<()> {
    let Player { tally, side } = player;
    tally.save(s, None)?;
    save_option_side(s, *side);
    Ok(())
}

fn load_player(l: &mut Loader<'_>) -> Result<Player> {
    Ok(Player {
        tally: Tally::load(l, None)?,
        side: load_option_side(l)?,
    })
}

pub(in crate::host) fn save_scores(s: &mut Saver, scoring: &Scoring) -> Result<()> {
    let Scoring {
        // What each connection was last sent is not moved: the new host
        // sends Scores to everyone at once (docs/ARCHITECTURE.md).
        sent: _,
        accounts,
        members,
        ai,
        players,
        sides,
        flyers,
        callsigns,
        changed,
        listed,
        version,
        last_sent,
    } = scoring;
    save_accounts(s, accounts)?;
    members.save(s, None)?;
    ai.save(s, None)?;
    s.count(players.len());
    for (order, player) in players {
        order.save(s, None)?;
        save_player(s, player)?;
    }
    sides.save(s, None)?;
    flyers.save(s, None)?;
    callsigns.save(s, None)?;
    changed.save(s, None)?;
    s.count(listed.len());
    for (order, side, left) in listed {
        order.save(s, None)?;
        save_option_side(s, *side);
        left.save(s, None)?;
    }
    version.save(s, None)?;
    last_sent.save(s, None)
}

pub(in crate::host) fn load_scores(l: &mut Loader<'_>) -> Result<Scoring> {
    let accounts = load_accounts(l)?;
    let members = Checkpoint::load(l, None)?;
    let ai = Checkpoint::load(l, None)?;
    let mut players = BTreeMap::new();
    for _ in 0..l.count()? {
        let order = u64::load(l, None)?;
        if players.insert(order, load_player(l)?).is_some() {
            return tore_sim::checkpoint::invalid("a player's tally twice");
        }
    }
    let sides = Checkpoint::load(l, None)?;
    let flyers = Checkpoint::load(l, None)?;
    let callsigns = Checkpoint::load(l, None)?;
    let changed = bool::load(l, None)?;
    let mut listed = Vec::new();
    for _ in 0..l.count()? {
        listed.push((
            u64::load(l, None)?,
            load_option_side(l)?,
            bool::load(l, None)?,
        ));
    }
    Ok(Scoring {
        accounts,
        members,
        ai,
        players,
        sides,
        flyers,
        callsigns,
        changed,
        listed,
        version: u64::load(l, None)?,
        sent: BTreeMap::new(),
        last_sent: Checkpoint::load(l, None)?,
    })
}

fn save_accounts(s: &mut Saver, accounts: &super::super::flight_report::Accounts) -> Result<()> {
    use crate::wire::messages::{Debrief, Message};
    let super::super::flight_report::Accounts {
        totals,
        previous,
        ai,
        last_ai,
        side_hits,
    } = accounts;
    for entries in [
        totals.iter().map(|(k, v)| (*k, v)).collect::<Vec<_>>(),
        previous.iter().map(|(k, v)| (u64::from(*k), v)).collect(),
        ai.iter().map(|(k, v)| (u64::from(*k), v)).collect(),
    ] {
        s.count(entries.len());
        for (key, pilot) in entries {
            key.save(s, None)?;
            let bytes = Message::Debrief(Box::new(Debrief {
                player: pilot.clone(),
                ..Debrief::default()
            }))
            .encode()
            .map_err(|_| tore_sim::checkpoint::CheckpointError::Invalid("report".into()))?;
            bytes.save(s, None)?;
        }
    }
    last_ai.save(s, None)?;
    side_hits.save(s, None)
}

fn load_accounts(l: &mut Loader<'_>) -> Result<super::super::flight_report::Accounts> {
    use crate::wire::messages::{Message, kind};
    let mut accounts = super::super::flight_report::Accounts::default();
    for section in 0..3 {
        for _ in 0..l.count()? {
            let key = u64::load(l, None)?;
            let bytes = Vec::<u8>::load(l, None)?;
            let Ok(Message::Debrief(report)) = Message::decode(kind::DEBRIEF, &bytes) else {
                return tore_sim::checkpoint::invalid("report");
            };
            if section != 0 {
                let Ok(key) = u32::try_from(key) else {
                    return tore_sim::checkpoint::invalid("plane");
                };
                if section == 1 {
                    accounts.previous.insert(key, report.player);
                } else {
                    accounts.ai.insert(key, report.player);
                }
            } else {
                accounts.totals.insert(key, report.player);
            }
        }
    }
    accounts.last_ai = Checkpoint::load(l, None)?;
    accounts.side_hits = Checkpoint::load(l, None)?;
    Ok(accounts)
}

#[cfg(test)]
mod tests {
    use super::super::super::state::{from_bytes, to_bytes};
    use super::*;
    use tore_sim::ai::launch::Side;
    use tore_world::seats::PlaneId;

    /// Scores with every field filled restore whole, but for what each
    /// connection was last sent, which is not moved.
    #[test]
    fn full_scores_restore_but_what_was_sent() {
        let tally = |kills, losses, damage| Tally {
            kills,
            losses,
            damage,
        };
        let pilot = crate::wire::messages::DebriefPilot {
            kills: [3, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            air_to_air: tore_sim::combat::ledger::Tally {
                launched: 5,
                hit: 3,
                damage: 250,
                ..Default::default()
            },
            ..Default::default()
        };
        let scoring = Scoring {
            accounts: super::super::super::flight_report::Accounts {
                totals: BTreeMap::from([(1, pilot.clone())]),
                previous: BTreeMap::from([(0, pilot.clone())]),
                ai: BTreeMap::from([(3, pilot)]),
                last_ai: std::collections::BTreeSet::from([3]),
                side_hits: [3, 3],
            },
            ai: BTreeMap::from([(tore_world::seats::PlaneId(3), tally(2, 1, 0.75))]),
            members: BTreeMap::from([(1, (0, "Viper".into())), (4, (1, "Hawk".into()))]),
            players: BTreeMap::from([
                (
                    1,
                    Player {
                        tally: tally(2, 1, 1.75),
                        side: Some(Side::Friendly),
                    },
                ),
                (
                    4,
                    Player {
                        tally: tally(0, 0, 0.),
                        side: None,
                    },
                ),
            ]),
            sides: [tally(2, 1, 1.75), tally(1, 2, 0.5)],
            flyers: BTreeMap::from([(PlaneId(0), 1), (PlaneId(5), 4)]),
            callsigns: BTreeMap::from([(PlaneId(0), "Viper".to_owned())]),
            changed: true,
            listed: vec![(1, Some(Side::Friendly), false), (4, None, true)],
            version: 9,
            sent: BTreeMap::from([(tore_net::ConnectionId(3), 8)]),
            last_sent: Some(1_440),
        };
        let bytes = to_bytes(|s| save_scores(s, &scoring)).unwrap();
        let restored = from_bytes(&bytes, load_scores).unwrap();
        assert_eq!(restored.players, scoring.players);
        assert_eq!(restored.members, scoring.members);
        assert_eq!(restored.ai, scoring.ai);
        assert_eq!(restored.accounts, scoring.accounts);
        assert_eq!(restored.sides, scoring.sides);
        assert_eq!(restored.flyers, scoring.flyers);
        assert_eq!(restored.callsigns, scoring.callsigns);
        assert_eq!(restored.listed, scoring.listed);
        assert_eq!(
            (restored.changed, restored.version, restored.last_sent),
            (true, 9, Some(1_440))
        );
        assert!(restored.sent.is_empty());
        assert_eq!(to_bytes(|s| save_scores(s, &restored)).unwrap(), bytes);
        for cut in 0..bytes.len() {
            let _ = from_bytes(&bytes[..cut], load_scores);
        }
    }
}
