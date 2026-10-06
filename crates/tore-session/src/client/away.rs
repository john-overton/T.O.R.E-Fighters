//! The game's side of the idle aircraft (stage F phase 2; docs/ARCHITECTURE.md,
//! "The AI flies an idle player's aircraft"). Slice F2-A.
//!
//! The game says when it has been away for the King's `idle-ai` seconds
//! ([`Client::away`], message 35) and when the player is back at the controls
//! ([`Client::back`], message 36); the host decides. When the host gives the
//! plane to the AI (because the game asked, or because it heard nothing from
//! it for those seconds) it starts the connection's observer flight with its
//! own plane as the subject: an Observing message that reaches a flying
//! game ends its flight, and [`Client::ai_flies`] names the plane until the
//! game has it back (a Seated message, after which the host ends the
//! observer flight) or the host keeps it no longer (the lobby state's away
//! mark goes, or the mission ends).

use super::{Client, ClientPhase};
use crate::wire::messages::{Message, Observing, kind};

/// Where the game stands with the idle rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Away {
    /// Away was sent and not yet answered.
    asked: bool,
    /// The AI flies this plane, kept for the player.
    plane: Option<u32>,
    /// Back was sent and not yet answered.
    back: bool,
}

impl Client {
    /// The game has been away for the `idle-ai` setting's seconds: the host
    /// gives the plane to the AI at its next tick. Sent once, while flying.
    pub fn away(&mut self) {
        if self.away.asked || self.away.plane.is_some() || self.phase != ClientPhase::Flying {
            return;
        }
        self.away.asked = true;
        let now = self.now;
        self.request(now, Message::Away);
    }

    /// The player is back at the controls: the answer is a
    /// [`super::ClientEvent::Seated`] (the plane again, as the AI left it)
    /// or a [`super::ClientEvent::Refused`]. Sent once, while away or
    /// asking to be.
    pub fn back(&mut self) {
        if self.away.back || !(self.away.asked || self.away.plane.is_some()) {
            return;
        }
        self.away.asked = false;
        self.away.back = true;
        let now = self.now;
        self.request(now, Message::Back);
    }

    /// The plane the AI flies for the player while it is away.
    pub fn ai_flies(&self) -> Option<u32> {
        self.away.plane
    }

    /// Whether Away was sent and the host has not yet answered it.
    pub fn away_asked(&self) -> bool {
        self.away.asked
    }

    /// Before each host message is handled: what it says of the idle rule.
    pub(super) fn away_message(&mut self, message: &Message) {
        match message {
            // The host gave the flying player's plane to the AI: the flight
            // ends, and the observer flight that follows shows the plane.
            Message::Observing(observing) => match observing.as_ref() {
                Observing::Started(_) => {
                    if self.phase == ClientPhase::Flying
                        && let Some(seat) = &self.seat
                    {
                        let plane = seat.plane;
                        self.away = Away {
                            plane: Some(plane),
                            ..Away::default()
                        };
                        self.log("away", &[&plane.to_string()]);
                        self.end_flight();
                    }
                }
                // Before the Seated message of the plane taken back, and at
                // the mission's end.
                Observing::Ended => self.away = Away::default(),
            },
            Message::Refused { request, .. } if *request == kind::AWAY => self.away.asked = false,
            Message::Refused { request, .. } if *request == kind::BACK => self.away.back = false,
            Message::Seated(_) => self.away = Away::default(),
            // The host keeps the plane no longer (the AI lost it, or the
            // player left it): every lobby state after the handoff marks the
            // player away while it does.
            Message::Lobby(lobby) => {
                if self.away.plane.is_some() && lobby.me().is_some_and(|me| !me.away) {
                    self.log("away", &["ended"]);
                    self.away = Away::default();
                }
            }
            Message::Mission(_) | Message::MissionEnded(_) => self.away = Away::default(),
            _ => {}
        }
    }
}
