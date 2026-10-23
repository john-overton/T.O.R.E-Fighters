//! Sides in a PvP lobby (lobby pass, slice L3; John, 2026-10-09): the colours
//! a slot and a player wear for their side, the "Blue 1 #2" wing words, which
//! slots the list shows, and the Bluefor and Redfor check boxes beside the
//! Slots heading with what each says and does. Plain data in and out, like
//! [`super::facts`], so every state is tested without a kit or a session.
//!
//! A player's side is the side of the slot they hold, and a player with no
//! slot has none: the lobby state's slots are the one source of truth, so a
//! box is checked exactly when the player holds a slot on its side, and the
//! list is filtered to the side the player holds.
use super::facts::Facts;
use tore_session::settings::{Sides, number, side_name};
use tore_session::wire::messages::{LobbySlot, LobbyState, Lock};
use tore_sim::ai::launch::Side;

/// The colours of the two sides (John's decision, 2026-10-09). Text on the
/// list's near-black well takes the light colours; a taken slot is a filled
/// bar in the strong colour with white text, which reads far better than
/// royal blue or bright red as text (plan 2.1).
pub mod tone {
    /// An open Bluefor slot, and a Bluefor player's name (retail menu palette
    /// entry 19).
    pub const LIGHT_BLUE: [u8; 3] = [134, 182, 223];
    /// An open Redfor slot, and a Redfor player's name (palette entry 56).
    pub const LIGHT_RED: [u8; 3] = [206, 113, 121];
    /// A taken Bluefor slot's bar: royal blue (palette entry 32).
    pub const ROYAL_BLUE: [u8; 3] = [36, 81, 186];
    /// A taken Redfor slot's bar: bright red (palette entry 62).
    pub const BRIGHT_RED: [u8; 3] = [210, 36, 40];
    /// The text on a taken slot's bar.
    pub const WHITE: [u8; 3] = [228, 230, 236];
    /// The text of a taken slot whose player is away (the AI flies it): the
    /// white, dimmed.
    pub const AWAY: [u8; 3] = [168, 176, 200];
}

/// The light colour of `side`: open slots, names and the box labels.
pub fn light(side: Side) -> [u8; 3] {
    match side {
        Side::Friendly => tone::LIGHT_BLUE,
        Side::Enemy => tone::LIGHT_RED,
    }
}

/// The strong colour of `side`: a taken slot's bar.
pub fn strong(side: Side) -> [u8; 3] {
    match side {
        Side::Friendly => tone::ROYAL_BLUE,
        Side::Enemy => tone::BRIGHT_RED,
    }
}

/// Both sides, Bluefor first (the order the mission builds them).
pub const SIDES: [Side; 2] = [Side::Friendly, Side::Enemy];

/// The host's refusal while Autobalance picks the sides, and the lobby's own
/// words for the same (`tore_session::host::sides::SIDES_BALANCED`, which the
/// host does not export).
pub const BALANCED_WORDS: &str = "Autobalance picks the sides.";
/// The host's refusal for a side fixed by lock sides in flight.
pub const LOCKED_WORDS: &str = "Sides are locked until the mission ends.";
/// A player flying cannot change slots.
const FLYING_WORDS: &str = "Leave your aircraft before you change your slot.";

/// The lobby is a PvP game (setting 1, the game type).
pub fn is_pvp(lobby: &LobbyState) -> bool {
    setting(lobby, number::MODE) == Some(1)
}

/// How players choose their side. Always free outside PvP, where the setting
/// does not apply.
pub fn rule(lobby: &LobbyState) -> Sides {
    if !is_pvp(lobby) {
        return Sides::Free;
    }
    match setting(lobby, number::LOCK_SIDES) {
        Some(0) => Sides::Free,
        Some(2) => Sides::Balanced,
        _ => Sides::Locked,
    }
}

fn setting(lobby: &LobbyState, number: u8) -> Option<u32> {
    lobby
        .settings
        .iter()
        .find(|(n, _)| *n == number)
        .map(|(_, v)| *v)
}

/// The side of the slot of `plane`, whoever holds it.
pub fn side_of_plane(lobby: &LobbyState, plane: u32) -> Option<Side> {
    lobby
        .slots
        .iter()
        .find(|s| s.plane == plane)
        .map(|s| s.wing.side)
}

/// The wing's name in PvP, which says the side in words as well as colour
/// (for colour-blind players; agent decision): "Blue 1 #2" and "Red 1 #2".
pub fn wing_label(slot: &LobbySlot) -> String {
    let side = match slot.wing.side {
        Side::Friendly => "Blue",
        Side::Enemy => "Red",
    };
    format!(
        "{side} {} #{}",
        slot.wing.display_number(),
        u32::from(slot.member) + 1
    )
}

/// Whether `slot` is free to the player the lobby was sent to: nobody holds
/// it, the AI is not kept flying it for someone else and the King's lock lets
/// this player have it. A slot kept for the reader is free to the reader.
pub fn free_for_reader(lobby: &LobbyState, slot: &LobbySlot) -> bool {
    let mine = |callsign: &str| lobby.me().is_some_and(|m| m.callsign == callsign);
    slot.holder.is_none()
        && slot.reserved.as_deref().is_none_or(mine)
        && match &slot.lock {
            Lock::Open => true,
            Lock::Closed => false,
            Lock::Reserved(callsign) => mine(callsign),
        }
}

/// What a click on a side box asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoxClick {
    /// Take the first free slot of the side (`SlotRequest::Side`).
    Join(Side),
    /// Free the slot held, which leaves the side.
    Leave,
    /// The reason in words, for Messages.
    Refused(String),
}

/// One side box: its label with the slots taken, whether it is checked and
/// lit, and what a click does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideBox {
    pub side: Side,
    /// "Bluefor 3/5": the side's name and how many of its slots the reader
    /// cannot take (held, closed, or kept for another player), of all its
    /// slots (agent proposal, so that "full" shows: "5/5").
    pub label: String,
    /// The reader holds a slot on this side.
    pub checked: bool,
    /// The box can do something now; a greyed box only says why it cannot.
    pub lit: bool,
    pub click: BoxClick,
}

/// The side boxes for the player the lobby was sent to, `None` in a co-op
/// game, where humans are always Bluefor. The states are the plan's table
/// (section 5.4) and the decisions of 2026-10-09 (D3, D6).
pub fn side_boxes(lobby: &LobbyState, facts: &Facts) -> Option<[SideBox; 2]> {
    if !facts.pvp {
        return None;
    }
    Some(SIDES.map(|side| {
        let on_side = lobby.slots.iter().filter(|s| s.wing.side == side);
        let total = on_side.clone().count();
        let free = on_side.filter(|s| free_for_reader(lobby, s)).count();
        let (lit, click) = box_state(side, total, free, facts);
        SideBox {
            side,
            label: format!("{} {}/{}", side_name(side), total - free, total),
            checked: facts.side == Some(side),
            lit,
            click,
        }
    }))
}

fn box_state(side: Side, total: usize, free: usize, facts: &Facts) -> (bool, BoxClick) {
    let refused = |why: &str| (false, BoxClick::Refused(why.to_owned()));
    // A game that cannot play the mission sees both sides, boxes greyed.
    if let Some(why) = &facts.unable {
        return refused(why);
    }
    // The host picks the sides: the assigned side is checked, both greyed.
    if facts.sides == Sides::Balanced {
        return refused(BALANCED_WORDS);
    }
    // Locked sides while flying: the side flown stays until the mission ends.
    if facts.sides == Sides::Locked && facts.flying {
        return refused(LOCKED_WORDS);
    }
    if facts.flying {
        return refused(FLYING_WORDS);
    }
    match facts.side {
        // Your side: unchecking leaves it.
        Some(own) if own == side => (true, BoxClick::Leave),
        // The other side, while you are on one (D3: leave, then join).
        Some(own) => refused(&format!("Uncheck {} first.", side_name(own))),
        None if free > 0 => (true, BoxClick::Join(side)),
        None if total == 0 => refused(&format!(
            "{} has no slots players may take.",
            side_name(side)
        )),
        None => refused(&format!("{} is full.", side_name(side))),
    }
}

/// A short note beside the boxes when the player cannot choose a side, widest
/// wording first; the screen draws the first that fits.
pub fn note(facts: &Facts) -> &'static [&'static str] {
    if !facts.pvp || facts.unable.is_some() {
        return &[];
    }
    match facts.sides {
        Sides::Balanced => &["Balanced by the host", "Balanced"],
        Sides::Locked if facts.flying => &["Locked for this mission", "Locked"],
        _ => &[],
    }
}

/// The side whose slots the Slots list shows: the player's own, once they
/// hold a slot; both sides with none (or outside PvP).
pub fn shown_side(lobby: &LobbyState) -> Option<Side> {
    if !is_pvp(lobby) {
        return None;
    }
    lobby
        .me()
        .and_then(|m| m.slot)
        .and_then(|plane| side_of_plane(lobby, plane))
}
