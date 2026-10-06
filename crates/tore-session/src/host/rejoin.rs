//! Rejoin tokens and reservations (stage K; docs/ARCHITECTURE.md, "Rejoin
//! tokens and reservations"). Slice K5.
//!
//! - **Tokens.** Every player gets a 128-bit token when it joins, drawn from
//!   the standard library's randomly keyed hasher
//!   ([`tore_net::TokenSource`]) and sent in a Token message. A token is good
//!   in this session only, whichever host runs the session now, for 24 hours
//!   after its player was last connected (the host's clock, which a migration
//!   carries over). A kick voids it; leaving does not.
//! - **The gate.** A game that sends a token that works in its Challenge
//!   answer is admitted whatever the room ([`super::HostGate`]); a game that
//!   joined without one may send it afterwards in a Rejoin message. A token
//!   that no longer works is answered in words, and the game joins as a new
//!   player.
//! - **Rejoining.** The connection becomes that player again: its join order
//!   (so its scores and lives are its own), its callsign and lobby id when
//!   free, its slot and loadout in the lobby, its side under lock sides (the
//!   court keeps sides by callsign). A player still connected under the token
//!   (its game restarted before the host noticed) is replaced by the new
//!   connection.
//! - **Reservations.** One table, keyed by the player's join order, holds
//!   every aircraft the AI flies for a player: an away player's (slice F2-A,
//!   [`Reserved::away`], its game still connected) and a dropped player's (its
//!   connection ended while it flew, with no Leave and no kick). Nobody else
//!   takes a reserved aircraft, and its slot reads "AI (Viper away)". A
//!   returning player takes it back by Join (or Back); the King's Release or
//!   the mission's end frees it; an aircraft the AI loses while its player is
//!   away ends the reservation and brings the revival rules on return (lives
//!   and the delay; `host::away`, `host::revive`).
//!
//! The session part *rejoin* (`rejoin_state.rs`) moves the tokens and the
//! table with the host.

#[path = "rejoin_state.rs"]
mod state;
// The part's coders: `state.rs` encodes and restores the part with them.
#[cfg_attr(not(test), allow(unused_imports))]
pub(super) use state::{load_rejoin, save_rejoin};

use super::lobby::LobbyEvent;
use super::{ConnectionId, Host, LeaveReason, Life, Peer, SlotRequest, Stage, unique_callsign};
use crate::wire::messages::Message;
use crate::wire::migration::{TokenGrant, limits};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;
use tore_net::{DisconnectReason, Entropy, Token, TokenSource};
use tore_world::mission::LoadoutSpec;
use tore_world::seats::{PlaneId, SeatId};

/// A token's life after its player was last connected: 24 hours (John,
/// 2026-09-28).
pub const TOKEN_LIFE: Duration = Duration::from_secs(limits::TOKEN_LIFE_SECONDS as u64);

/// The most players' records a host keeps; past it the longest-gone go.
const MAX_RECORDS: usize = 1_024;

/// How long a record outlives its token, so a token that expired is still
/// told so in words rather than as an unknown one (agent decision).
const TOMBSTONE: Duration = Duration::from_secs(7 * 24 * 3600);

/// Why a token does not work, in words for the player.
pub const TOKEN_EXPIRED: &str =
    "Your rejoin token has expired: it lasts 24 hours after you were last in the game.";
pub const TOKEN_VOIDED: &str =
    "You were removed from this game, so your rejoin token no longer works.";
pub const TOKEN_UNKNOWN: &str =
    "This game does not know your rejoin token: it was issued by another game.";
pub const LOST_AWAY: &str = "Your aircraft was lost while you were away.";
pub const ALREADY_IN: &str = "You are already in the game.";
pub const NOT_RESERVED: &str = "No aircraft is kept for a player on that plane.";

/// Why a token is not good now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    /// More than 24 hours since its player was last connected.
    Expired,
    /// Its player was kicked.
    Voided,
    /// No player of this session holds it.
    Unknown,
}

impl Refusal {
    pub(super) fn words(self) -> &'static str {
        match self {
            Self::Expired => TOKEN_EXPIRED,
            Self::Voided => TOKEN_VOIDED,
            Self::Unknown => TOKEN_UNKNOWN,
        }
    }
}

/// What the host keeps of one player, by join order, so its token can bring
/// it back.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Record {
    pub(super) token: Token,
    pub(super) callsign: String,
    pub(super) lobby_id: u8,
    /// The slot and loadout it held in the lobby when it was last connected.
    pub(super) slot: Option<PlaneId>,
    pub(super) loadout: Option<LoadoutSpec>,
    /// The host's clock when it was last connected; `None` while it is.
    pub(super) left: Option<Duration>,
    /// Its player was kicked: the token never works again.
    pub(super) voided: bool,
}

impl Record {
    /// Why the token does not work at `now`, if it does not.
    fn refusal(&self, now: Duration) -> Option<Refusal> {
        if self.voided {
            Some(Refusal::Voided)
        } else if self
            .left
            .is_some_and(|left| now.saturating_sub(left) > TOKEN_LIFE)
        {
            Some(Refusal::Expired)
        } else {
            None
        }
    }
}

/// A plane the AI flies for a player, kept for it. F2-A's away table and the
/// dropped players' are this one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Reserved {
    pub(super) plane: PlaneId,
    /// The player's lobby id and callsign, for the lobby state and the
    /// words of a refusal.
    pub(super) lobby_id: u8,
    pub(super) callsign: String,
    /// Back (or a returned player's Join) was accepted: the take is on its
    /// way.
    pub(super) returning: bool,
    /// The player is connected and away (slice F2-A): it takes the plane
    /// back by Back, and the lobby marks it away. A dropped player's is not.
    pub(super) away: bool,
}

/// The session's tokens and reservations.
#[derive(Debug)]
pub(super) struct Rejoin {
    /// Every player's record, by join order.
    pub(super) players: BTreeMap<u64, Record>,
    /// The planes the AI flies for players, by join order. It starts afresh
    /// with each mission.
    pub(super) reserved: BTreeMap<u64, Reserved>,
    /// The session's clock less the host's: a record's moments are on the
    /// session's clock, which a host that takes over carries on from the
    /// old host's reading (slice K4), so a token's 24 hours do not restart.
    pub(super) offset: Duration,
    source: TokenSource,
}

impl Rejoin {
    /// A host's tokens and reservations, its tokens drawn from `entropy`.
    pub(super) fn new(entropy: Entropy) -> Self {
        // A seeded host's tokens differ from its transport's own draws.
        let entropy = match entropy {
            Entropy::System => Entropy::System,
            Entropy::Seeded(seed) => Entropy::Seeded(seed ^ 0x704b_e45e_ed00),
        };
        Self {
            players: BTreeMap::new(),
            reserved: BTreeMap::new(),
            offset: Duration::ZERO,
            source: TokenSource::new(entropy),
        }
    }

    /// A record and a token for the player `order`, connected now.
    pub(super) fn issue(&mut self, order: u64, callsign: &str, lobby_id: u8) -> Token {
        let token = loop {
            let token = self.source.next_token();
            if token.0 != 0 && !self.players.values().any(|r| r.token == token) {
                break token;
            }
        };
        self.players.insert(
            order,
            Record {
                token,
                callsign: callsign.to_owned(),
                lobby_id,
                slot: None,
                loadout: None,
                left: None,
                voided: false,
            },
        );
        token
    }

    /// The player a token brings back at `now`, or why it does not.
    pub(super) fn claim(&self, token: Token, now: Duration) -> Result<u64, Refusal> {
        let (order, record) = self
            .players
            .iter()
            .find(|(_, record)| record.token == token)
            .ok_or(Refusal::Unknown)?;
        match record.refusal(now) {
            Some(why) => Err(why),
            None => Ok(*order),
        }
    }

    /// The tokens that work at `now`, for the transport's gate.
    pub(super) fn valid_tokens(&self, now: Duration) -> Vec<Token> {
        self.players
            .values()
            .filter(|record| record.refusal(now).is_none())
            .map(|record| record.token)
            .collect()
    }

    /// Whether the host still keeps the player `order` for its token: its
    /// scores stay while it does.
    pub(super) fn keeps(&self, order: u64, now: Duration) -> bool {
        self.players
            .get(&order)
            .is_some_and(|record| record.refusal(now).is_none())
    }

    /// The player a plane is kept for, and its reservation.
    pub(super) fn reserved_plane(&self, plane: PlaneId) -> Option<(u64, &Reserved)> {
        self.reserved
            .iter()
            .find(|(_, reserved)| reserved.plane == plane)
            .map(|(order, reserved)| (*order, reserved))
    }

    /// Drops what has expired and, past the cap, the longest-gone records.
    pub(super) fn purge(&mut self, now: Duration) {
        self.players.retain(|_, record| {
            record
                .left
                .is_none_or(|left| now.saturating_sub(left) <= TOMBSTONE)
        });
        if self.players.len() > MAX_RECORDS {
            let mut gone: Vec<(Duration, u64)> = self
                .players
                .iter()
                .filter_map(|(order, record)| Some((record.left?, *order)))
                .collect();
            gone.sort();
            let excess = self.players.len() - MAX_RECORDS;
            for (_, order) in gone.into_iter().take(excess) {
                self.players.remove(&order);
            }
        }
    }
}

/// What a connection that ends leaves the table of reservations to decide.
struct Gone {
    order: u64,
    stage: Stage,
    seat: Option<SeatId>,
    plane: Option<PlaneId>,
    lobby_id: u8,
    callsign: String,
}

impl Gone {
    fn of(peer: &Peer) -> Self {
        Self {
            order: peer.lobby.order,
            stage: peer.stage,
            seat: peer.seat,
            plane: peer.plane,
            lobby_id: peer.lobby.id,
            callsign: peer.callsign.clone(),
        }
    }
}

impl Host {
    /// The session's clock for token expiry: the host's, moved by the offset
    /// a migration sets so that it carries on from the old host's.
    pub(super) fn token_clock(&self) -> Duration {
        self.now + self.rejoin.offset
    }

    /// The connection of the player `order`, when it has one that is not
    /// closing.
    pub(super) fn connection_of(&self, order: u64) -> Option<ConnectionId> {
        self.peers
            .iter()
            .find(|(_, peer)| {
                peer.lobby.order == order && !matches!(peer.stage, Stage::Closing { .. })
            })
            .map(|(id, _)| *id)
    }

    /// A player joined (the hook after the transport's `Connected`): a game
    /// with a token that works becomes that player again, any other gets a
    /// token of its own, and a token that does not work is answered in
    /// words.
    pub(super) fn rejoin_connected(&mut self, connection: ConnectionId, token: Option<Token>) {
        if !self.peers.contains_key(&connection) {
            return;
        }
        let now = self.token_clock();
        self.rejoin.purge(now);
        let mut why = None;
        if let Some(token) = token {
            match self.rejoin.claim(token, now) {
                Ok(order) => {
                    self.restore(connection, order);
                    return;
                }
                Err(refusal) => why = Some(refusal),
            }
        }
        self.issue_token(connection);
        if let Some(refusal) = why {
            self.send(
                connection,
                &Message::Notice(format!("{} You join as a new player.", refusal.words())),
            );
        }
    }

    /// Draws a token for the new player on `connection` and sends it.
    fn issue_token(&mut self, connection: ConnectionId) {
        let Some(peer) = self.peers.get(&connection) else {
            return;
        };
        let (order, callsign, id) = (peer.lobby.order, peer.callsign.clone(), peer.lobby.id);
        let token = self.rejoin.issue(order, &callsign, id);
        self.send_token(connection, token);
    }

    fn send_token(&mut self, connection: ConnectionId, token: Token) {
        self.send(
            connection,
            &Message::Token(TokenGrant {
                token,
                life_seconds: limits::TOKEN_LIFE_SECONDS,
            }),
        );
    }

    /// The new connection becomes the player `order` again.
    fn restore(&mut self, connection: ConnectionId, order: u64) {
        let Some(record) = self.rejoin.players.get(&order).cloned() else {
            return;
        };
        let Some(me) = self.peers.get(&connection) else {
            return;
        };
        let fresh = me.lobby.order;
        // Its game restarted before the host noticed the old connection
        // went: the old one goes as a drop would, and the crown moves.
        let crown = match self.connection_of(order).filter(|old| *old != connection) {
            Some(old) => self.supersede(old),
            None => false,
        };
        let (callsigns, ids): (Vec<String>, BTreeSet<u8>) = {
            let others: Vec<&Peer> = self
                .peers
                .iter()
                .filter(|(id, peer)| **id != connection && peer.lobby.order != order)
                .map(|(_, peer)| peer)
                .collect();
            (
                others.iter().map(|peer| peer.callsign.clone()).collect(),
                others.iter().map(|peer| peer.lobby.id).collect(),
            )
        };
        let callsign = unique_callsign(&record.callsign, callsigns.iter().map(String::as_str));
        let Some(peer) = self.peers.get_mut(&connection) else {
            return;
        };
        let id = if ids.contains(&record.lobby_id) {
            peer.lobby.id
        } else {
            record.lobby_id
        };
        peer.lobby.order = order;
        peer.lobby.id = id;
        peer.callsign = callsign.clone();
        if crown {
            peer.king = true;
        }
        // The record it was given at this join is not needed.
        if fresh != order {
            self.rejoin.players.remove(&fresh);
        }
        if let Some(record) = self.rejoin.players.get_mut(&order) {
            record.left = None;
            record.callsign = callsign.clone();
            record.lobby_id = id;
        }
        // The lobby keeps its place: the slot it held, with its loadout,
        // when the slot is still free and its rules allow it.
        if matches!(self.life, Life::Lobby)
            && let Some(plane) = record.slot
            && self.slot(connection, SlotRequest::Take(plane.0)).is_ok()
            && let Some(peer) = self.peers.get_mut(&connection)
        {
            peer.lobby.loadout = record.loadout.clone();
        }
        self.send_token(connection, record.token);
        // Welcome back, with what waits for it.
        let reserved = self.rejoin.reserved.get(&order).map(|r| r.plane);
        let words = if reserved.is_some() {
            format!("Welcome back, {callsign}: your aircraft is waiting.")
        } else {
            format!("Welcome back, {callsign}.")
        };
        self.send(connection, &Message::Notice(words));
        if reserved.is_none() {
            self.rejoin_lost_notice(connection, order);
        }
        self.lobby_log(
            callsign,
            LobbyEvent::Rejoined {
                plane: reserved.map(|p| p.0),
            },
        );
        self.roster_dirty = true;
        self.lobby_dirty = true;
    }

    /// A returning player whose aircraft the AI lost while it was away is
    /// told, with the revival rules' words (lives and the delay).
    fn rejoin_lost_notice(&mut self, connection: ConnectionId, order: u64) {
        if !matches!(self.life, Life::Flying) || !self.revival_lost(order) {
            return;
        }
        self.send(connection, &Message::Notice(LOST_AWAY.into()));
        let revival = self.revival_message(order);
        self.send(connection, &Message::Revival(Box::new(revival)));
    }

    /// The old connection of a player whose game came back before the host
    /// noticed it went: it leaves as a drop would, its plane kept for the
    /// player. Returns whether it wore the crown, which moves to the new
    /// connection.
    fn supersede(&mut self, old: ConnectionId) -> bool {
        let now = self.now;
        let Some(peer) = self.peers.get(&old) else {
            return false;
        };
        let gone = Gone::of(peer);
        self.reserve(gone);
        let mut crown = false;
        if let Some(peer) = self.peers.get_mut(&old) {
            if peer.stage == Stage::Seated
                && let Some(seat) = peer.seat
            {
                self.gives.push((seat, peer.callsign.clone()));
            }
            // It closes at the next update: its drop was handled here.
            peer.stage = Stage::Closing {
                deadline: now,
                reason: DisconnectReason::Left,
            };
            crown = std::mem::take(&mut peer.king);
        }
        self.lobby_dirty = true;
        crown
    }

    /// The plane of a seated player whose connection ends is kept for it, if
    /// the AI can fly it. A player still connected and away keeps its
    /// reservation, now a dropped player's.
    fn reserve(&mut self, gone: Gone) {
        let Gone {
            order,
            stage,
            seat,
            plane,
            lobby_id,
            callsign,
        } = gone;
        if let Some(reserved) = self.rejoin.reserved.get_mut(&order) {
            reserved.away = false;
            reserved.returning = false;
            self.lobby_dirty = true;
            return;
        }
        if !matches!(self.life, Life::Flying) {
            return;
        }
        let (Stage::Seated, Some(seat), Some(plane)) = (stage, seat, plane) else {
            return;
        };
        // A lost plane cannot go to the AI: the revival rules have it.
        if self.world.can_give_back(seat).is_err() {
            return;
        }
        self.rejoin.reserved.insert(
            order,
            Reserved {
                plane,
                lobby_id,
                callsign: callsign.clone(),
                returning: false,
                away: false,
            },
        );
        self.lobby_log(callsign, LobbyEvent::Dropped { plane: plane.0 });
        self.lobby_dirty = true;
    }

    /// A connection ended (the hook in `Host::closed`, `peer` already out of
    /// the table): the player's token starts its 24 hours, or is voided by a
    /// kick, and a plane it was flying is kept for it.
    pub(super) fn rejoin_left(&mut self, peer: &Peer, reason: LeaveReason) {
        let order = peer.lobby.order;
        // Its game came back first: the new connection is the player now.
        if self.peers.values().any(|other| other.lobby.order == order) {
            return;
        }
        let now = self.token_clock();
        if let Some(record) = self.rejoin.players.get_mut(&order) {
            record.left = Some(now);
            record.slot = peer.lobby.slot;
            record.loadout = peer.lobby.loadout.clone();
            record.lobby_id = peer.lobby.id;
            record.callsign = peer.callsign.clone();
            if reason == LeaveReason::Kicked {
                record.voided = true;
            }
        }
        match reason {
            // Removed, or the game is over: nothing is kept.
            LeaveReason::Kicked | LeaveReason::HostLeft | LeaveReason::MissionEnded => {
                self.rejoin.reserved.remove(&order);
                self.lobby_dirty = true;
            }
            // An away player whose game says goodbye has left on purpose
            // (its flight ended when the plane went to the AI): the plane
            // stays the AI's.
            LeaveReason::Left if self.rejoin.reserved.get(&order).is_some_and(|r| r.away) => {
                self.rejoin.reserved.remove(&order);
                self.lobby_dirty = true;
            }
            // A drop (silence, a crash, quitting without ending the
            // flight): the plane is kept for the player.
            LeaveReason::Left
            | LeaveReason::Silent
            | LeaveReason::Replaced
            | LeaveReason::Disconnected(_) => self.reserve(Gone::of(peer)),
        }
    }

    /// The callsign of the player `plane` is reserved for, for the lobby
    /// state's slot: an away player's plane, which the AI flies for it
    /// (slice F2-A), or a dropped player's (slice K5).
    pub(super) fn reserved_for(&self, plane: PlaneId) -> Option<String> {
        self.rejoin
            .reserved_plane(plane)
            .map(|(_, reserved)| reserved.callsign.clone())
    }

    /// The King's Release of a reservation (message 53): the aircraft is the
    /// AI's again, free for anyone. A player who comes back finds no plane
    /// and takes a free slot by the usual rules.
    pub(super) fn release_request(
        &mut self,
        _connection: ConnectionId,
        plane: u32,
    ) -> Result<(), String> {
        let Some((order, _)) = self.rejoin.reserved_plane(PlaneId(plane)) else {
            return Err(NOT_RESERVED.into());
        };
        let Some(reserved) = self.rejoin.reserved.remove(&order) else {
            return Err(NOT_RESERVED.into());
        };
        // A player still here is told, as when the AI loses its aircraft.
        if let Some(connection) = self.connection_of(order) {
            self.send(
                connection,
                &Message::Notice("The King released your aircraft to the AI.".into()),
            );
        }
        self.lobby_log(reserved.callsign, LobbyEvent::Released { plane });
        self.lobby_dirty = true;
        Ok(())
    }

    /// A late Rejoin with a token (message 54), from a game that joined
    /// without sending it: the player becomes the token's player again, or
    /// is told why not.
    pub(super) fn rejoin_request(
        &mut self,
        connection: ConnectionId,
        token: Token,
    ) -> Result<(), String> {
        let now = self.token_clock();
        let peer = self.peers.get(&connection).ok_or("")?;
        // Its own token again changes nothing.
        if self
            .rejoin
            .players
            .get(&peer.lobby.order)
            .is_some_and(|record| record.token == token)
        {
            return Ok(());
        }
        // A game that has flown or holds a place in the lobby is a player
        // already.
        if peer.stage != Stage::Lobby || peer.flight != 0 || peer.lobby.slot.is_some() {
            return Err(ALREADY_IN.into());
        }
        let order = self
            .rejoin
            .claim(token, now)
            .map_err(|refusal| refusal.words().to_owned())?;
        self.restore(connection, order);
        Ok(())
    }

    /// Moves the session's clock `by` on, as if that long had passed (tests
    /// of the token's 24 hours).
    #[cfg(test)]
    pub(crate) fn advance_token_clock_for_test(&mut self, by: Duration) {
        self.rejoin.offset += by;
    }

    /// The session's id, for tests.
    #[cfg(test)]
    pub(crate) fn session_id_for_test(&self) -> u64 {
        self.session_id
    }

    /// How many connections the host holds, for tests.
    #[cfg(test)]
    pub(crate) fn peers_for_test(&self) -> usize {
        self.peers.len()
    }

    /// The token of the player with `callsign`, for tests.
    #[cfg(test)]
    pub(crate) fn token_of_for_test(&self, callsign: &str) -> Option<Token> {
        self.rejoin
            .players
            .values()
            .find(|record| record.callsign == callsign)
            .map(|record| record.token)
    }
}
