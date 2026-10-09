//! The revivals part of the session's state (stage K slice K1;
//! docs/ARCHITECTURE.md, "What moves with the host"): lives and losses by
//! player, the seats held for players whose plane is lost and the revivals
//! asked for, both by player (join order) rather than connection, the sides'
//! starts and the planes revivals added, and (slice R1) the AI lineages'
//! respawns and every lineage's original spawn. Every field of [`Revivals`] is
//! named, so a field added to it fails to compile here until it is coded or
//! skipped with its class.

use super::super::state::{Restoring, Result, Saving, load_side, save_side};
use super::{Lineage, Pending, Player, Revivals};
use crate::wire::messages::Spawned;
use std::collections::BTreeMap;
use tore_net::ConnectionId;
use tore_sim::ai::launch::WingId;
use tore_sim::checkpoint::{Checkpoint, Loader, Saver, invalid};

tore_sim::checkpoint_struct!(Player { used, lost });

tore_sim::checkpoint_struct!(Lineage { used, lost, told });

fn save_pending(s: &mut Saver, pending: Pending) -> Result<()> {
    match pending {
        Pending::Revive { seat } => {
            s.writer().write_varint(0);
            seat.save(s, None)
        }
        Pending::AiSlot { seat, plane } => {
            s.writer().write_varint(1);
            seat.save(s, None)?;
            plane.save(s, None)
        }
    }
}

fn load_pending(l: &mut Loader<'_>) -> Result<Pending> {
    Ok(match l.reader().read_varint()? {
        0 => Pending::Revive {
            seat: Checkpoint::load(l, None)?,
        },
        1 => Pending::AiSlot {
            seat: Checkpoint::load(l, None)?,
            plane: Checkpoint::load(l, None)?,
        },
        other => return invalid(format!("a revival asked for has no variant {other}")),
    })
}

fn save_spawned(s: &mut Saver, spawned: &Spawned) -> Result<()> {
    let Spawned {
        plane,
        tick,
        wing,
        member,
        aircraft,
        spawn,
    } = spawned;
    let WingId { side, index } = wing;
    plane.save(s, None)?;
    tick.save(s, None)?;
    save_side(s, *side);
    index.save(s, None)?;
    member.save(s, None)?;
    aircraft.save(s, None)?;
    spawn.save(s, None)
}

fn load_spawned(l: &mut Loader<'_>) -> Result<Spawned> {
    Ok(Spawned {
        plane: u32::load(l, None)?,
        tick: u32::load(l, None)?,
        wing: WingId {
            side: load_side(l)?,
            index: u8::load(l, None)?,
        },
        member: u8::load(l, None)?,
        aircraft: Checkpoint::load(l, None)?,
        spawn: Checkpoint::load(l, None)?,
    })
}

/// A map by connection, coded by join order: the entries in join order.
fn by_order<T: Copy>(
    saving: &Saving<'_>,
    map: impl Iterator<Item = (ConnectionId, T)>,
) -> BTreeMap<u64, T> {
    map.filter_map(|(connection, value)| Some((saving.order(connection)?, value)))
        .collect()
}

pub(in crate::host) fn save_revivals(
    s: &mut Saver,
    saving: &Saving<'_>,
    revivals: &Revivals,
) -> Result<()> {
    let Revivals {
        // Scratch: the revivals one tick's commands make, filled before the
        // step and taken after it in the same tick (`revive_after`), so it
        // is empty between ticks, where parts are coded.
        making: _,
        // Scratch as `making` is: the AI respawns of one tick (slice R1).
        respawning: _,
        players,
        held,
        pending,
        waiting,
        starts,
        spawned,
        lineages,
        origins,
    } = revivals;
    players.save(s, None)?;
    by_order(saving, held.iter().map(|(c, seat)| (*c, *seat))).save(s, None)?;
    let pending = by_order(saving, pending.iter().map(|(c, p)| (*c, *p)));
    s.count(pending.len());
    for (order, pending) in pending {
        order.save(s, None)?;
        save_pending(s, pending)?;
    }
    let waiting: Vec<u64> = waiting.iter().filter_map(|c| saving.order(*c)).collect();
    waiting.save(s, None)?;
    starts.save(s, None)?;
    s.count(spawned.len());
    for spawned in spawned {
        save_spawned(s, spawned)?;
    }
    lineages.save(s, None)?;
    origins.save(s, None)
}

pub(in crate::host) fn load_revivals(
    l: &mut Loader<'_>,
    restoring: &mut Restoring<'_>,
) -> Result<Revivals> {
    let players = Checkpoint::load(l, None)?;
    let held: BTreeMap<u64, tore_world::seats::SeatId> = Checkpoint::load(l, None)?;
    let held = held
        .into_iter()
        .map(|(order, seat)| (restoring.connection(order), seat))
        .collect();
    let mut pending = BTreeMap::new();
    for _ in 0..l.count()? {
        let order = u64::load(l, None)?;
        let wanted = load_pending(l)?;
        pending.insert(restoring.connection(order), wanted);
    }
    let waiting: Vec<u64> = Checkpoint::load(l, None)?;
    let waiting = waiting
        .into_iter()
        .map(|order| restoring.connection(order))
        .collect();
    let starts = Checkpoint::load(l, None)?;
    let mut spawned = Vec::new();
    for _ in 0..l.count()? {
        spawned.push(load_spawned(l)?);
    }
    let lineages = Checkpoint::load(l, None)?;
    let origins = Checkpoint::load(l, None)?;
    Ok(Revivals {
        players,
        held,
        pending,
        making: Vec::new(),
        waiting,
        starts,
        spawned,
        lineages,
        origins,
        respawning: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::state::{from_bytes, to_bytes};
    use super::*;
    use tore_formats::aircraft::AircraftId;
    use tore_sim::ai::launch::Side;
    use tore_world::mission::{LoadoutSpec, StationLoad};
    use tore_world::seats::{PlaneId, SeatId};
    use tore_world::world::revive::Spawn;

    /// Revivals with every field filled code by join order and restore
    /// under the connections given for the orders; a connection with no
    /// order is left out.
    #[test]
    fn full_revivals_restore_by_join_order() {
        let c = ConnectionId;
        let revivals = Revivals {
            players: BTreeMap::from([
                (
                    1,
                    Player {
                        used: 2,
                        lost: Some((PlaneId(3), 840)),
                    },
                ),
                (2, Player::default()),
            ]),
            held: BTreeMap::from([(c(7), SeatId(1)), (c(99), SeatId(4))]),
            pending: BTreeMap::from([
                (c(7), Pending::Revive { seat: SeatId(1) }),
                (
                    c(8),
                    Pending::AiSlot {
                        seat: SeatId(2),
                        plane: PlaneId(5),
                    },
                ),
            ]),
            making: Vec::new(),
            respawning: Vec::new(),
            lineages: BTreeMap::from([
                (
                    PlaneId(2),
                    Lineage {
                        used: 3,
                        lost: Some(1_200),
                        told: true,
                    },
                ),
                (PlaneId(5), Lineage::default()),
            ]),
            origins: BTreeMap::from([
                (PlaneId(0), ([1.5, 20_000., -3e5], 0.25)),
                (PlaneId(2), ([-4096., 20_000., 1.2e5], -3.0)),
            ]),
            waiting: vec![c(8), c(7)],
            starts: [Some([1., -2.5, 3e5]), None],
            spawned: vec![Spawned {
                plane: 11,
                tick: 900,
                wing: WingId {
                    side: Side::Enemy,
                    index: 2,
                },
                member: 3,
                aircraft: AircraftId::Mig29,
                spawn: Spawn {
                    position: [10., 20., 30.],
                    heading_rad: 1.25,
                    speed_fps: 640.,
                    loadout: LoadoutSpec {
                        tanks: None,
                        fuel_lbs: 9_000.,
                        cheat: false,
                        stations: vec![StationLoad {
                            weapon: "AA10.JT".into(),
                            count: 2,
                            quantity: 2,
                        }],
                    },
                },
            }],
        };
        // Connections 7 and 8 joined first and second; 99 is forgotten.
        let orders = |connection: ConnectionId| match connection.0 {
            7 => Some(1),
            8 => Some(2),
            _ => None,
        };
        let bytes = to_bytes(|s| save_revivals(s, &Saving::new(&orders), &revivals)).unwrap();
        let mut ids = |order: u64| ConnectionId(100 + order as u32);
        let mut restoring = Restoring::new(&mut ids);
        let restored = from_bytes(&bytes, |l| load_revivals(l, &mut restoring)).unwrap();
        assert_eq!(
            restored.held,
            BTreeMap::from([(c(101), SeatId(1))]),
            "by order, the forgotten connection left out"
        );
        assert_eq!(
            restored.pending,
            BTreeMap::from([
                (c(101), Pending::Revive { seat: SeatId(1) }),
                (
                    c(102),
                    Pending::AiSlot {
                        seat: SeatId(2),
                        plane: PlaneId(5)
                    }
                ),
            ])
        );
        assert_eq!(restored.waiting, [c(102), c(101)]);
        assert_eq!(restored.starts, revivals.starts);
        assert_eq!(restored.spawned, revivals.spawned);
        assert_eq!(restored.players[&1].used, 2);
        assert_eq!(restored.players[&1].lost, Some((PlaneId(3), 840)));
        assert_eq!(restored.lineages, revivals.lineages);
        assert_eq!(restored.origins, revivals.origins);
        let back = |connection: ConnectionId| Some(u64::from(connection.0 - 100));
        assert_eq!(
            to_bytes(|s| save_revivals(s, &Saving::new(&back), &restored)).unwrap(),
            bytes
        );
        for cut in 0..bytes.len() {
            let mut ids = |order: u64| ConnectionId(order as u32);
            let mut restoring = Restoring::new(&mut ids);
            let _ = from_bytes(&bytes[..cut], |l| load_revivals(l, &mut restoring));
        }
    }
}
