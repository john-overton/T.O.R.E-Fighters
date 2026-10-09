//! Taking a lineage's aircraft (the lobby pass's follow-up F1, John
//! 2026-10-09: "a late joiner can take over AI aircraft"; docs/ARCHITECTURE.md,
//! "Death, revival and lives").
//!
//! A slot names a plane the mission started with: the root of its lineage
//! (slice R1). Once that plane is lost and the AI has respawned it, the slot
//! is the lineage's, not the wreck's:
//!
//! - **Take.** A player who joins in flight with a slot (or asks for a plane)
//!   whose plane is lost takes the lineage's newest plane when the AI flies it
//!   for nobody ([`Holder::Ai`]). Every rule of an in-flight take still
//!   applies to that plane (the open planes, judged by the root; the King's
//!   slot locks, join in progress, lock sides and Autobalance; another
//!   player's slot or reservation).
//! - **Wait.** When the lineage is between respawns (its newest plane lost,
//!   the AI's, and a respawn due: AI respawn on and a life left), the player
//!   waits in the lobby, ready, with a notice of when, and takes the new plane
//!   the tick after the AI respawns it. Leave, unready or another slot ends the
//!   wait; a respawn that will no longer come ends it with the usual refusal.
//! - **The open planes.** `open-planes` lists roots: every plane of a listed
//!   lineage is open ([`Host::open`]).
//! - **The lead.** A player who takes a lineage plane that leads its flight
//!   owns the flight's lead by the lead hold's own rule (slice R2,
//!   `tore_world::world::lead_hold`).
//!
//! A player's own revival comes first: a player whose plane is lost (held,
//! or noted while it was away) flies again by the revival rules
//! (`host::revive`), never by taking the AI's respawn of its lineage.

use super::revive::Holder;
use super::{ConnectionId, Host, LobbyEvent, Stage, TICKS_PER_SECOND};
use crate::wire::messages::Message;
use tore_world::seats::{Pilot, PlaneId};

/// What a take of a lineage's plane flies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum LineageTake {
    /// The plane asked for, by the usual rules.
    AsAsked,
    /// The lineage's newest plane, which the AI flies for nobody.
    Head(PlaneId),
    /// The lineage waits for its respawn: the words say when.
    Wait(String),
}

/// "0:45".
fn clock(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

impl Host {
    /// The root of `plane`'s lineage: the plane the mission started with that
    /// it continues, or `plane` itself.
    pub(super) fn root_of(&self, plane: PlaneId) -> PlaneId {
        self.world.revival.root_of(plane)
    }

    /// What a take of `wanted` flies in flight: `wanted` itself, the newest
    /// plane of its lineage when `wanted` is lost and the AI flies the newest
    /// for nobody, or a wait for the lineage's respawn.
    pub(super) fn lineage_take(&self, wanted: PlaneId) -> LineageTake {
        if self.world.roster.plane(wanted).is_none() && !self.retired(wanted) {
            return LineageTake::AsAsked;
        }
        let root = self.root_of(wanted);
        let head = self.world.lineage_head(root);
        if self.lineage_holder(head) != Holder::Ai {
            return LineageTake::AsAsked;
        }
        if self.world.head_lost(head) {
            return match self.respawn_due(root) {
                Some(words) => LineageTake::Wait(words),
                None => LineageTake::AsAsked,
            };
        }
        let ai_flies = self
            .world
            .roster
            .plane(head)
            .is_some_and(|plane| plane.pilot == Pilot::Ai);
        if head != wanted && ai_flies {
            LineageTake::Head(head)
        } else {
            LineageTake::AsAsked
        }
    }

    /// Whether `plane` was retired from the mission.
    fn retired(&self, plane: PlaneId) -> bool {
        self.world
            .revival
            .retired()
            .iter()
            .any(|entry| entry.id == plane)
    }

    /// When the AI respawns `root`'s lost lineage, in words, if it will: AI
    /// respawn on and a life left.
    fn respawn_due(&self, root: PlaneId) -> Option<String> {
        if !self.ai_respawns() {
            return None;
        }
        let lineage = self.revival.lineages.get(&root);
        let used = lineage.map_or(0, |l| l.used);
        if self.settings.lives().is_some_and(|lives| used >= lives) {
            return None;
        }
        let delay = u64::from(self.settings.revive_delay_seconds()) * TICKS_PER_SECOND;
        let since = lineage
            .and_then(|l| l.lost)
            .unwrap_or_else(|| self.world.tick());
        let wait = (since + delay).saturating_sub(self.world.tick());
        Some(if wait == 0 {
            format!(
                "Plane {} is about to fly again: you take it when it does.",
                root.0
            )
        } else {
            format!(
                "Plane {} flies again in {}: you take it then.",
                root.0,
                clock(wait.div_ceil(TICKS_PER_SECOND))
            )
        })
    }

    /// Why `connection` may not wait for `root`'s respawn, if it may not: the
    /// rules an in-flight take of the lineage's plane would meet, but for the
    /// plane flying.
    pub(super) fn lineage_wait_refusal(
        &self,
        connection: ConnectionId,
        root: PlaneId,
    ) -> Option<String> {
        if !self.open(root) {
            return Some(format!("Plane {} is not open to players.", root.0));
        }
        if let Some(why) = self.away_take_refusal(connection, root) {
            return Some(why);
        }
        if let Some(other) = self.holder(root, connection) {
            return Some(format!(
                "{} holds plane {}.",
                self.peers[&other].callsign, root.0
            ));
        }
        self.king_take_refusal(connection, root)
            .or_else(|| self.revive_take_refusal(connection, root))
    }

    /// `connection` waits for `root`'s lineage to respawn: ready in the
    /// lobby, told when, and seated in the new plane after it appears.
    pub(super) fn await_respawn(&mut self, connection: ConnectionId, root: PlaneId, words: String) {
        let Some(peer) = self.peers.get_mut(&connection) else {
            return;
        };
        peer.lobby.ready = true;
        let callsign = peer.callsign.clone();
        let is_slot = self.slots().iter().any(|slot| slot.id == root.0);
        if let Some(peer) = self.peers.get_mut(&connection)
            && is_slot
            && peer.lobby.slot != Some(root)
        {
            peer.lobby.release();
            peer.lobby.slot = Some(root);
            peer.lobby.ready = true;
        }
        let new = self.revival.awaiting.insert(connection, root) != Some(root);
        self.lobby_dirty = true;
        self.send(connection, &Message::Notice(words));
        if new {
            self.lobby_log(
                callsign,
                LobbyEvent::Revival(format!("waits to take plane {} when it respawns", root.0)),
            );
        }
    }

    /// The player gives up its wait (Leave, or no longer ready): it stays in
    /// the lobby.
    pub(super) fn end_wait(&mut self, connection: ConnectionId) -> bool {
        self.revival.awaiting.remove(&connection).is_some()
    }

    /// After the tick's respawns: every player waiting for a lineage whose
    /// respawn has come takes it now (at the next tick, as any Join), and a
    /// wait that no longer applies ends: the player left, is no longer ready
    /// or holds another slot, or the respawn will not come (the take's
    /// refusal says why).
    pub(super) fn awaiting_takes(&mut self) {
        let waiting: Vec<(ConnectionId, PlaneId)> = self
            .revival
            .awaiting
            .iter()
            .map(|(connection, root)| (*connection, *root))
            .collect();
        for (connection, root) in waiting {
            let still = self.peers.get(&connection).is_some_and(|peer| {
                peer.stage == Stage::Lobby
                    && peer.lobby.ready
                    && peer.lobby.slot.is_none_or(|slot| slot == root)
            });
            if !still {
                self.revival.awaiting.remove(&connection);
                continue;
            }
            if matches!(self.lineage_take(root), LineageTake::Wait(_)) {
                continue;
            }
            self.revival.awaiting.remove(&connection);
            self.take_now(connection, Some(root.0));
        }
    }
}
