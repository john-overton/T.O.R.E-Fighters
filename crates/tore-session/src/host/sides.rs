//! The lobby pass's side requests (slice W0): a player asks for the first
//! free slot on Bluefor or Redfor ([`SlotRequest::Side`], the lobby's side
//! boxes), and the host answers with that slot or a refusal in words.
//!
//! A player's side is the side of the slot it holds; a player with no slot
//! has none. Nothing here keeps state: the lobby state's slots are the one
//! source of truth.
//!
//! Autobalance (setting 7's `balanced`) refuses a request for the other
//! side than the one the host gave the player; the balancing rule itself,
//! which seats players on a side, is slice A1's (`host/balance.rs`).
//!
//! [`SlotRequest::Side`]: crate::wire::messages::SlotRequest::Side

use super::{ConnectionId, Host, Life};
use crate::settings::{Mode, side_name};
use tore_sim::ai::launch::Side;
use tore_world::seats::{Pilot, PlaneId};

/// The refusal of a side request in a co-op game, where humans are always
/// on the friendly side.
pub const SIDES_PVP_ONLY: &str = "Sides are chosen only in a PvP game.";
/// The refusal of a side request while the host picks the sides.
pub const SIDES_BALANCED: &str = "Autobalance picks the sides.";

impl Host {
    /// The slot a side request asks for: the one held, when it is on that
    /// side already; otherwise the first slot of that side, in plane order,
    /// that `connection` may hold now. Refused in co-op, while Autobalance
    /// picks the sides, while the player holds a slot on the other side
    /// (it leaves that side first, D3), when lock sides fixed the player's
    /// side in flight, and when the side has no free slot.
    pub(super) fn side_slot(
        &self,
        connection: ConnectionId,
        side: Side,
        held: Option<PlaneId>,
    ) -> Result<PlaneId, String> {
        if self.settings.mode() != Mode::Pvp {
            return Err(SIDES_PVP_ONLY.into());
        }
        if let Some(why) = self.balance_side_refusal(connection, side) {
            return Err(why);
        }
        let slots = self.slots();
        if let Some(plane) = held
            && let Some(own) = slots.iter().find(|slot| slot.id == plane.0)
        {
            if own.wing.side == side {
                return Ok(plane);
            }
            return Err(format!("Leave {} first.", side_name(own.wing.side)));
        }
        let ours: Vec<PlaneId> = slots
            .iter()
            .filter(|slot| slot.wing.side == side)
            .map(|slot| PlaneId(slot.id))
            .collect();
        let Some(&first) = ours.first() else {
            return Err(format!(
                "{} has no slots players may take.",
                side_name(side)
            ));
        };
        // Lock sides in flight: the first plane flown this mission fixes the
        // side, so any plane of the side asked for answers.
        if let Some(why) = self.sides_refusal(connection, first) {
            return Err(why);
        }
        ours.into_iter()
            .find(|&plane| self.side_slot_free(connection, plane))
            .ok_or_else(|| format!("{} is full.", side_name(side)))
    }

    /// Whether `plane`'s slot is free to `connection`: nobody else holds it,
    /// the King's lock lets this player have it (a slot kept for the player
    /// is free to it alone), and, while the mission flies, the AI flies it
    /// for nobody.
    pub(super) fn side_slot_free(&self, connection: ConnectionId, plane: PlaneId) -> bool {
        if self.holder(plane, connection).is_some()
            || self.lock_refusal(connection, plane.0).is_some()
        {
            return false;
        }
        if !matches!(self.life, Life::Flying) {
            return true;
        }
        self.world
            .roster
            .plane(plane)
            .is_some_and(|entry| entry.pilot == Pilot::Ai)
            && !self.reserved(plane)
            && self.away_take_refusal(connection, plane).is_none()
    }

    /// Why Autobalance refuses `connection` choosing `side`, if it does:
    /// while the host picks the sides, any side but the one it gave the
    /// player (plan 6.3; the rule is in `host/balance.rs`). Its own side is
    /// never refused, so it may still move between its side's slots.
    /// [`Host::slot`]'s [`SlotRequest::Take`] asks it too, and so does a
    /// plane taken in flight ([`Host::king_take_refusal`]).
    ///
    /// [`SlotRequest::Take`]: crate::wire::messages::SlotRequest::Take
    pub(super) fn balance_side_refusal(
        &self,
        connection: ConnectionId,
        side: Side,
    ) -> Option<String> {
        self.balanced_side(connection)
            .is_some_and(|own| own != side)
            .then(|| SIDES_BALANCED.to_owned())
    }
}
