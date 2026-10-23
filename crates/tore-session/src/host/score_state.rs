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
        players,
        sides,
        flyers,
        callsigns,
        changed,
        listed,
        version,
        last_sent,
    } = scoring;
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
    for (order, side) in listed {
        order.save(s, None)?;
        save_option_side(s, *side);
    }
    version.save(s, None)?;
    last_sent.save(s, None)
}

pub(in crate::host) fn load_scores(l: &mut Loader<'_>) -> Result<Scoring> {
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
        listed.push((u64::load(l, None)?, load_option_side(l)?));
    }
    Ok(Scoring {
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
        let scoring = Scoring {
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
            listed: vec![(1, Some(Side::Friendly)), (4, None)],
            version: 9,
            sent: BTreeMap::from([(tore_net::ConnectionId(3), 8)]),
            last_sent: Some(1_440),
        };
        let bytes = to_bytes(|s| save_scores(s, &scoring)).unwrap();
        let restored = from_bytes(&bytes, load_scores).unwrap();
        assert_eq!(restored.players, scoring.players);
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
