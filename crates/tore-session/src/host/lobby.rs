//! The lobby's bookkeeping on the host (slice EF4): what each connected
//! player holds between flights, and the lobby's log entries. The rules are
//! in docs/ARCHITECTURE.md, "The lobby", and the dedicated server's in
//! docs/DEDICATED-SERVER.md, "The lobby".

use std::fmt;
use tore_world::mission::LoadoutSpec;
use tore_world::seats::PlaneId;

/// One player's place in the lobby.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Entry {
    /// The player's lobby id, kept while connected: the lowest free.
    pub id: u8,
    /// When the player connected, for the lobby's order.
    pub order: u64,
    /// The plane of the slot the player holds.
    pub slot: Option<PlaneId>,
    /// The loadout chosen for that slot; `None` is the standard load.
    pub loadout: Option<LoadoutSpec>,
    /// Ready to fly.
    pub ready: bool,
    /// Why the player's import cannot play the mission, when it cannot.
    pub unable: Option<String>,
}

impl Entry {
    pub fn new(id: u8, order: u64) -> Self {
        Self {
            id,
            order,
            slot: None,
            loadout: None,
            ready: false,
            unable: None,
        }
    }

    /// Lets go of the slot, its loadout and the ready mark.
    pub fn release(&mut self) {
        self.slot = None;
        self.loadout = None;
        self.ready = false;
    }
}

/// Something that happened in the lobby, for the log.
#[derive(Clone, Debug, PartialEq)]
pub enum LobbyEvent {
    /// The player holds this slot now, or none.
    Slot(Option<u32>),
    /// The player chose a loadout for its slot's plane (`own`), or went back
    /// to the standard one.
    Loadout { plane: u32, own: bool },
    /// The player is ready, or not.
    Ready(bool),
    /// The player's import cannot play the mission.
    Unable(String),
    /// A request was refused: the request and why.
    Refused {
        request: &'static str,
        reason: String,
    },
    /// The King changed the mission.
    MissionChanged { number: u32, summary: String },
    /// The King removed the player, with the King's words.
    Kicked(String),
    /// The player left the flight and is back in the lobby.
    BackInLobby,
}

impl fmt::Display for LobbyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Slot(Some(plane)) => write!(f, "took the slot of plane {plane}"),
            Self::Slot(None) => f.write_str("left its slot"),
            Self::Loadout { plane, own: true } => write!(f, "armed plane {plane}"),
            Self::Loadout { plane, own: false } => {
                write!(f, "chose the standard load for plane {plane}")
            }
            Self::Ready(true) => f.write_str("is ready"),
            Self::Ready(false) => f.write_str("is not ready"),
            Self::Unable(reason) => write!(f, "cannot play the mission: {reason}"),
            Self::Refused { request, reason } => write!(f, "was refused {request}: {reason}"),
            Self::MissionChanged { number, summary } => {
                write!(f, "changed the mission (number {number}): {summary}")
            }
            Self::Kicked(reason) if reason.is_empty() => f.write_str("was kicked"),
            Self::Kicked(reason) => write!(f, "was kicked: {reason}"),
            Self::BackInLobby => f.write_str("is back in the lobby"),
        }
    }
}
