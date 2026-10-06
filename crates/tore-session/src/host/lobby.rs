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
    /// Only the flight's build failed (the loadouts' weapons): cleared when
    /// the lobby returns.
    pub unable_flight: bool,
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
            unable_flight: false,
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
    /// The player sent more lobby requests in a second than the host
    /// answers; the rest of that second's are dropped.
    TooManyRequests,
    /// The player wears the crown now: it joined first, or the King left
    /// (stage F phase 2, slice F2-1).
    Crowned,
    /// The King gave the crown to this player.
    CrownPassed(String),
    /// The King changed the settings, in words ("mode pvp, kill-limit 5").
    SettingsChanged(String),
    /// The King locked a slot, or opened it.
    SlotLocked {
        plane: u32,
        lock: crate::wire::messages::Lock,
    },
    /// The player started watching the flying mission, or stopped (slice
    /// F2-O1's observers).
    Watching(bool),
    /// A crowned dedicated server, empty for its empty timeout, went back
    /// to its file's mission and settings.
    BackToFile,
    /// The player is away and the AI flies its plane, kept for it: its game
    /// said so, or sent nothing for the `idle-ai` seconds (`stalled`; slice
    /// F2-A).
    Away { plane: u32, stalled: bool },
    /// The away player is back: it takes its plane from the AI.
    Back { plane: u32 },
    /// The away player's plane is no longer kept for it: the AI lost it
    /// (`lost`), or the player left it to the AI.
    AwayEnded { plane: u32, lost: bool },
    /// The player's connection ended while it flew: the AI flies `plane`,
    /// kept for it (slice K5).
    Dropped { plane: u32 },
    /// The player came back with its token; `plane` is the aircraft kept for
    /// it, if one is.
    Rejoined { plane: Option<u32> },
    /// The King released the aircraft kept for the player.
    Released { plane: u32 },
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
            Self::TooManyRequests => {
                f.write_str("sent too many lobby requests; the rest of this second's are dropped")
            }
            Self::Crowned => f.write_str("wears the crown"),
            Self::CrownPassed(to) => write!(f, "passed the crown to {to}"),
            Self::SettingsChanged(words) => write!(f, "changed the settings: {words}"),
            Self::SlotLocked { plane, lock } => {
                use crate::wire::messages::Lock;
                match lock {
                    Lock::Open => write!(f, "opened the slot of plane {plane}"),
                    Lock::Closed => write!(f, "closed the slot of plane {plane}"),
                    Lock::Reserved(callsign) => {
                        write!(f, "kept the slot of plane {plane} for {callsign}")
                    }
                }
            }
            Self::Watching(true) => f.write_str("is watching the mission"),
            Self::Watching(false) => f.write_str("stopped watching"),
            Self::BackToFile => f.write_str(
                "has been empty for its empty timeout: back to its file's mission and settings",
            ),
            Self::Away {
                plane,
                stalled: false,
            } => write!(f, "is away: the AI flies plane {plane}"),
            Self::Away {
                plane,
                stalled: true,
            } => write!(
                f,
                "is away (its game sends nothing): the AI flies plane {plane}"
            ),
            Self::Back { plane } => write!(f, "is back: takes plane {plane} from the AI"),
            Self::AwayEnded { plane, lost: true } => {
                write!(f, "lost plane {plane} while the AI flew it")
            }
            Self::AwayEnded { plane, lost: false } => {
                write!(f, "left plane {plane} to the AI")
            }
            Self::Dropped { plane } => {
                write!(f, "dropped out: the AI flies plane {plane}, kept for it")
            }
            Self::Rejoined { plane: Some(plane) } => {
                write!(f, "rejoined with its token: plane {plane} is waiting")
            }
            Self::Rejoined { plane: None } => f.write_str("rejoined with its token"),
            Self::Released { plane } => {
                write!(f, "had plane {plane} released to the AI by the King")
            }
        }
    }
}

// Stage K: the players part of the session's state (slice K1).
#[path = "lobby_state.rs"]
pub(super) mod state;
