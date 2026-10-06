//! The players part of the session's state (stage K slice K1;
//! docs/ARCHITECTURE.md, "What moves with the host"): every connected player
//! by its join order, with what it is in the session (callsign, system,
//! path, lobby entry, crown and house, seat and plane, watching, content)
//! and none of what its connection holds.
//!
//! [`PlayerState`] is the part's unit: made from a connection's record
//! ([`PlayerState::of`]) and coded with every field of that record named, so
//! a field added to it fails to compile here until it is coded or skipped
//! with its class. A restore gives the players' states, which slice K4
//! holds absent until each player resumes and then turns into a record
//! ([`PlayerState::into_peer`]).

use super::super::state::{
    Clock, Result, Saving, load_address, load_duration, save_address, save_time,
};
use super::super::{Host, Peer, Stage};
use super::Entry;
use crate::host::content::PlayerContent;
use crate::wire::chat::RateLimit;
use crate::wire::connection::HostConnection;
use crate::wire::messages::{Goodbye, Importer};
use crate::wire::{Path, Platform};
use std::net::SocketAddr;
use std::time::Duration;
use tore_net::DisconnectReason;
use tore_sim::checkpoint::{Checkpoint, Loader, Saver, invalid};
use tore_world::seats::{PlaneId, SeatId};

tore_sim::checkpoint_enum!(crate::wire::messages::Build {
    Unknown = 0,
    V10 = 1,
    V102F = 2,
});

tore_sim::checkpoint_enum!(crate::wire::messages::ItemKind {
    Aircraft = 0,
    Theater = 1,
    Weapon = 2,
    Shared = 3,
});

tore_sim::checkpoint_struct!(Importer { version, commit });
tore_sim::checkpoint_struct!(PlayerContent {
    build,
    importer,
    items,
});

tore_sim::checkpoint_struct!(Entry {
    id,
    order,
    slot,
    loadout,
    ready,
    unable,
    unable_flight,
});

/// Where a player is; a closing deadline on the coding host's clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::host) enum StageState {
    Lobby,
    Taking {
        seat: SeatId,
        plane: PlaneId,
    },
    Seated,
    Leaving,
    Closing {
        deadline: Duration,
        reason: DisconnectReason,
    },
}

/// One player as the players part holds it.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::host) struct PlayerState {
    /// The address the host saw it at.
    pub address: SocketAddr,
    pub callsign: String,
    pub platform: Platform,
    pub path: Path,
    pub stage: StageState,
    pub seat: Option<SeatId>,
    pub plane: Option<PlaneId>,
    /// The mission ended while it was connected, and the host stops.
    pub ended: bool,
    /// Its lobby entry: its lobby id and join order, slot, loadout, ready
    /// mark and why it cannot fly.
    pub lobby: Entry,
    pub king: bool,
    pub house: bool,
    /// Why the host disconnects it, when it said goodbye.
    pub goodbye: Option<Goodbye>,
    /// It watches the flying mission; its game asks again when it resumes
    /// (docs/formats/net-protocol.md, "Resuming").
    pub watching: bool,
    /// What its Content said (stage L).
    pub content: Option<PlayerContent>,
}

impl PlayerState {
    /// The player of `peer`. Every field of the record is named.
    pub(in crate::host) fn of(peer: &Peer) -> Self {
        let Peer {
            // Connection state, a new host's own (docs/ARCHITECTURE.md, "Not
            // moved"): the wire's baselines and names, the input buffer, what
            // the game could not foresee, the exact states and their holds,
            // the ownship terms, the last picture, the flight number (a
            // resumed player starts a new flight), the lobby's send times,
            // the request and chat rates, the refusal last logged, and
            // whether the gaps went out (the new host sends them again).
            wire: _,
            inputs: _,
            unforeseen: _,
            last_own_state: _,
            holding_since: _,
            mismatch_answered: _,
            terms: _,
            picture: _,
            flight: _,
            lobby_stale: _,
            lobby_sent: _,
            requests: _,
            refusal_logged: _,
            chat_rate: _,
            gaps_sent: _,
            // Coded.
            address,
            callsign,
            platform,
            path,
            stage,
            seat,
            plane,
            ended,
            lobby,
            king,
            house,
            goodbye,
            watch,
            content,
        } = peer;
        Self {
            address: *address,
            callsign: callsign.clone(),
            platform: *platform,
            path: *path,
            stage: match *stage {
                Stage::Lobby => StageState::Lobby,
                Stage::Taking { seat, plane } => StageState::Taking { seat, plane },
                Stage::Seated => StageState::Seated,
                Stage::Leaving => StageState::Leaving,
                Stage::Closing { deadline, reason } => StageState::Closing { deadline, reason },
            },
            seat: *seat,
            plane: *plane,
            ended: *ended,
            lobby: lobby.clone(),
            king: *king,
            house: *house,
            goodbye: goodbye.clone(),
            watching: watch.is_some(),
            content: content.clone(),
        }
    }

    /// The connection record of a player that resumes, its moments moved by
    /// `clock` onto the resuming host's, its connection state fresh as a
    /// join's; it watches again only when it asks again.
    pub(in crate::host) fn into_peer(self, clock: Clock, ticks_per_snapshot: u32) -> Peer {
        let PlayerState {
            address,
            callsign,
            platform,
            path,
            stage,
            seat,
            plane,
            ended,
            lobby,
            king,
            house,
            goodbye,
            watching: _,
            content,
        } = self;
        Peer {
            address,
            callsign,
            platform,
            path,
            stage: match stage {
                StageState::Lobby => Stage::Lobby,
                StageState::Taking { seat, plane } => Stage::Taking { seat, plane },
                StageState::Seated => Stage::Seated,
                StageState::Leaving => Stage::Leaving,
                StageState::Closing { deadline, reason } => Stage::Closing {
                    deadline: clock.moved(deadline),
                    reason,
                },
            },
            seat,
            plane,
            wire: HostConnection::new(ticks_per_snapshot),
            inputs: super::super::InputBuffer::new(),
            unforeseen: false,
            last_own_state: 0,
            holding_since: None,
            mismatch_answered: 0,
            terms: None,
            picture: None,
            ended,
            lobby,
            flight: 0,
            king,
            house,
            goodbye,
            lobby_stale: true,
            lobby_sent: None,
            requests: (Duration::ZERO, 0),
            refusal_logged: None,
            chat_rate: RateLimit::default(),
            watch: None,
            content,
            gaps_sent: false,
        }
    }
}

fn save_stage(s: &mut Saver, stage: StageState) -> Result<()> {
    match stage {
        StageState::Lobby => s.writer().write_varint(0),
        StageState::Taking { seat, plane } => {
            s.writer().write_varint(1);
            seat.save(s, None)?;
            plane.save(s, None)?;
        }
        StageState::Seated => s.writer().write_varint(2),
        StageState::Leaving => s.writer().write_varint(3),
        StageState::Closing { deadline, reason } => {
            s.writer().write_varint(4);
            save_time(s, deadline)?;
            reason.code().save(s, None)?;
        }
    }
    Ok(())
}

fn load_stage(l: &mut Loader<'_>) -> Result<StageState> {
    Ok(match l.reader().read_varint()? {
        0 => StageState::Lobby,
        1 => StageState::Taking {
            seat: Checkpoint::load(l, None)?,
            plane: Checkpoint::load(l, None)?,
        },
        2 => StageState::Seated,
        3 => StageState::Leaving,
        4 => StageState::Closing {
            deadline: load_duration(l)?,
            reason: DisconnectReason::from_code(u8::load(l, None)?),
        },
        other => return invalid(format!("a player has no stage {other}")),
    })
}

fn save_goodbye(s: &mut Saver, goodbye: &Option<Goodbye>) -> Result<()> {
    match goodbye {
        None => s.writer().write_varint(0),
        Some(Goodbye::Kicked(words)) => {
            s.writer().write_varint(1);
            words.save(s, None)?;
        }
        Some(Goodbye::HostLeft) => s.writer().write_varint(2),
    }
    Ok(())
}

fn load_goodbye(l: &mut Loader<'_>) -> Result<Option<Goodbye>> {
    Ok(match l.reader().read_varint()? {
        0 => None,
        1 => Some(Goodbye::Kicked(String::load(l, None)?)),
        2 => Some(Goodbye::HostLeft),
        other => return invalid(format!("a goodbye has no variant {other}")),
    })
}

fn save_player(s: &mut Saver, player: &PlayerState) -> Result<()> {
    let PlayerState {
        address,
        callsign,
        platform,
        path,
        stage,
        seat,
        plane,
        ended,
        lobby,
        king,
        house,
        goodbye,
        watching,
        content,
    } = player;
    save_address(s, *address)?;
    callsign.save(s, None)?;
    platform.code().save(s, None)?;
    path.code().save(s, None)?;
    save_stage(s, *stage)?;
    seat.save(s, None)?;
    plane.save(s, None)?;
    ended.save(s, None)?;
    lobby.save(s, None)?;
    king.save(s, None)?;
    house.save(s, None)?;
    save_goodbye(s, goodbye)?;
    watching.save(s, None)?;
    content.save(s, None)
}

fn load_player(l: &mut Loader<'_>) -> Result<PlayerState> {
    Ok(PlayerState {
        address: load_address(l)?,
        callsign: String::load(l, None)?,
        platform: Platform::from_code(u8::load(l, None)?)
            .ok_or_else(|| tore_sim::checkpoint::CheckpointError::Invalid("platform".into()))?,
        path: Path::from_code(u64::from(u8::load(l, None)?))
            .ok_or_else(|| tore_sim::checkpoint::CheckpointError::Invalid("path".into()))?,
        stage: load_stage(l)?,
        seat: Checkpoint::load(l, None)?,
        plane: Checkpoint::load(l, None)?,
        ended: bool::load(l, None)?,
        lobby: Entry::load(l, None)?,
        king: bool::load(l, None)?,
        house: bool::load(l, None)?,
        goodbye: load_goodbye(l)?,
        watching: bool::load(l, None)?,
        content: Checkpoint::load(l, None)?,
    })
}

/// The players part: a count, then each player in join order.
pub(in crate::host) fn save_players(s: &mut Saver, _: &Saving<'_>, host: &Host) -> Result<()> {
    let mut players: Vec<PlayerState> = host.peers.values().map(PlayerState::of).collect();
    players.sort_by_key(|player| player.lobby.order);
    s.count(players.len());
    for player in &players {
        save_player(s, player)?;
    }
    Ok(())
}

/// The players a players part holds, in join order.
pub(in crate::host) fn load_players(bytes: &[u8]) -> Result<Vec<PlayerState>> {
    super::super::state::from_bytes(bytes, |l| {
        let count = l.count()?;
        let mut players: Vec<PlayerState> = Vec::with_capacity(count);
        for _ in 0..count {
            let player = load_player(l)?;
            if players
                .last()
                .is_some_and(|last| last.lobby.order >= player.lobby.order)
            {
                return invalid("players out of join order");
            }
            players.push(player);
        }
        Ok(players)
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::state::to_bytes;
    use super::*;
    use crate::wire::messages::{Build, ItemKind};
    use std::collections::BTreeMap;
    use tore_world::mission::{LoadoutSpec, StationLoad};

    fn player(order: u64) -> PlayerState {
        let mut lobby = Entry::new(order as u8 + 2, order);
        lobby.slot = Some(PlaneId(3));
        lobby.loadout = Some(LoadoutSpec {
            fuel_lbs: 7_500.5,
            cheat: true,
            stations: vec![StationLoad {
                weapon: "AIM9X.JT".into(),
                count: 1,
                quantity: 2,
            }],
        });
        lobby.ready = true;
        lobby.unable = Some("lacks the F-22".into());
        lobby.unable_flight = true;
        PlayerState {
            address: "[2001:db8::7]:26900".parse().unwrap(),
            callsign: "Viper_2".into(),
            platform: Platform::MacOs,
            path: Path::Relay,
            stage: StageState::Closing {
                deadline: Duration::from_millis(61_250),
                reason: DisconnectReason::Kicked,
            },
            seat: Some(SeatId(4)),
            plane: Some(PlaneId(3)),
            ended: true,
            lobby,
            king: true,
            house: false,
            goodbye: Some(Goodbye::Kicked("Bye.".into())),
            watching: true,
            content: Some(PlayerContent {
                build: Build::V102F,
                importer: Some(Importer {
                    version: "0.1.3".into(),
                    commit: "abc".into(),
                }),
                items: BTreeMap::from([
                    ((ItemKind::Aircraft, "F18.PT".to_owned()), 17),
                    ((ItemKind::Shared, String::new()), 4),
                ]),
            }),
        }
    }

    /// Players with every field filled, and every stage, restore as they
    /// were; damaged bytes are refused, never a panic.
    #[test]
    fn full_players_restore_in_join_order() {
        let mut players = vec![player(1), player(2)];
        players[1].address = "10.0.0.2:40001".parse().unwrap();
        players[1].goodbye = Some(Goodbye::HostLeft);
        players[1].content = None;
        for (n, stage) in [
            StageState::Lobby,
            StageState::Taking {
                seat: SeatId(1),
                plane: PlaneId(2),
            },
            StageState::Seated,
            StageState::Leaving,
        ]
        .into_iter()
        .enumerate()
        {
            let mut more = player(3 + n as u64);
            more.stage = stage;
            more.goodbye = None;
            players.push(more);
        }
        let bytes = to_bytes(|s| {
            s.count(players.len());
            players.iter().try_for_each(|p| save_player(s, p))
        })
        .unwrap();
        assert_eq!(load_players(&bytes).unwrap(), players);
        for cut in 0..bytes.len() {
            let _ = load_players(&bytes[..cut]);
        }
        // Out of join order is refused.
        players.swap(0, 1);
        let bytes = to_bytes(|s| {
            s.count(players.len());
            players.iter().try_for_each(|p| save_player(s, p))
        })
        .unwrap();
        assert!(load_players(&bytes).is_err());
    }

    /// A player made into a connection's record again keeps what the part
    /// holds, its closing deadline moved onto the new clock, and starts its
    /// connection state afresh.
    #[test]
    fn a_player_becomes_a_record_on_the_new_clock() {
        let state = player(1);
        let clock = Clock {
            old: Duration::from_secs(60),
            new: Duration::from_secs(5),
        };
        let peer = state.clone().into_peer(clock, 4);
        assert_eq!(
            peer.stage,
            Stage::Closing {
                deadline: Duration::from_millis(6_250),
                reason: DisconnectReason::Kicked
            }
        );
        assert!(peer.watch.is_none(), "it asks to watch again");
        let mut again = PlayerState::of(&peer);
        again.stage = state.stage;
        again.watching = true;
        assert_eq!(again, state);
    }
}
