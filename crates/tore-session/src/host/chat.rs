//! Chat on the host (slice EF6, protocol 4): the routing of a player's line
//! to those who hear it, the limits, and what the sender is told. The rules
//! are in docs/ARCHITECTURE.md ("Chat") and docs/formats/net-protocol.md.
//!
//! - **All** goes to everyone connected, in the lobby or flying, seated or
//!   not.
//! - **Friendlies** and **Enemies** go to the players flying a plane of the
//!   sender's side, or the other side.
//! - **Wing** goes to the players flying in the sender's wing.
//! - **Target** goes to the human flying the sender's designated target.
//!   An AI-flown target, a target that is not an aircraft and no target at
//!   all hear nothing.
//! - A player with no plane (in the lobby, whether or not the mission
//!   flies) sends to All only, and hears only All. Observers, when phase 2
//!   makes them, are refused here.
//!
//! The sender is sent its own line back, so it sees what went out, and when
//! no one else hears it, the host's words "No one hears you.".

use super::{Host, HostLog, Life, Message, Peer, Stage};
use crate::wire::chat::{ChatFrom, ChatLine, ChatSend, NO_ONE_HEARS, Receiver, Refusal, Standing};
use tore_net::ConnectionId;
use tore_sim::ai::launch::WingId;
use tore_sim::combat::missiles::TargetRole;
use tore_world::seats::{Pilot, PlaneId, SeatId};

/// Where a flying sender is: its plane and wing (whose side is the
/// sender's).
#[derive(Clone, Copy, Debug)]
struct Place {
    plane: PlaneId,
    wing: WingId,
}

impl Host {
    /// A player's chat line: checked, routed, delivered, logged.
    pub(super) fn chat(&mut self, connection: ConnectionId, send: ChatSend) {
        let send = match send.checked() {
            Ok(send) => send,
            // A line of nothing is dropped without a word.
            Err(Refusal::Empty) => return,
            Err(refusal) => return self.chat_refused(connection, refusal),
        };
        let place = self.place_of(connection);
        if place.is_none() && send.receiver != Receiver::All {
            return self.chat_refused(connection, Refusal::OnlyAll);
        }
        let hearers = match self.hearers(connection, place, send.receiver) {
            Ok(hearers) => hearers,
            Err(refusal) => return self.chat_refused(connection, refusal),
        };
        let now = self.now;
        let Some(peer) = self.peers.get_mut(&connection) else {
            return;
        };
        if !peer.chat_rate.allow(now) {
            return self.chat_refused(connection, Refusal::TooFast);
        }
        let callsign = peer.callsign.clone();
        let sound = send.quick.and_then(|quick| quick.sound);
        // The sender sees its own line (retail's `YOU TO ALL`), and does
        // not hear its own quick message's sound (agent decision).
        let own = ChatLine {
            from: ChatFrom::Player {
                callsign: callsign.clone(),
                standing: if place.is_some() {
                    Standing::Own
                } else {
                    Standing::Neutral
                },
                you: true,
            },
            receiver: send.receiver,
            text: send.text.clone(),
            sound: None,
        };
        self.send(connection, &Message::ChatLine(own));
        let heard = hearers.len();
        for hearer in hearers {
            let standing = match (place, self.place_of(hearer)) {
                (Some(from), Some(to)) if from.wing.side == to.wing.side => Standing::Own,
                (Some(_), Some(_)) => Standing::Enemy,
                _ => Standing::Neutral,
            };
            let line = ChatLine {
                from: ChatFrom::Player {
                    callsign: callsign.clone(),
                    standing,
                    you: false,
                },
                receiver: send.receiver,
                text: send.text.clone(),
                sound: sound.clone(),
            };
            self.send(hearer, &Message::ChatLine(line));
        }
        let tick = self.world.tick();
        self.log(HostLog::Chat {
            tick,
            callsign,
            receiver: send.receiver,
            text: send.text,
            heard,
        });
        if heard == 0 {
            self.say(connection, NO_ONE_HEARS);
        }
    }

    /// Tells one player something, as a system line.
    fn say(&mut self, connection: ConnectionId, text: &str) {
        self.send(connection, &Message::ChatLine(ChatLine::system(text)));
    }

    /// Refuses a line: the reason is said to the sender and noted in the
    /// log (once a second at most for the same reason).
    fn chat_refused(&mut self, connection: ConnectionId, refusal: Refusal) {
        self.say(connection, refusal.text());
        self.log_refusal(connection, "a chat line", refusal.text().to_owned());
    }

    /// Where a player flies, when it does: seated, in a plane the roster
    /// knows.
    fn place_of(&self, connection: ConnectionId) -> Option<Place> {
        let peer = self.peers.get(&connection)?;
        flying(peer)?;
        let plane = peer.plane?;
        let wing = self.world.roster.plane(plane)?.slot.wing;
        Some(Place { plane, wing })
    }

    /// The connections that hear a line from `sender` (never the sender
    /// itself), or why the line cannot go.
    fn hearers(
        &self,
        sender: ConnectionId,
        place: Option<Place>,
        receiver: Receiver,
    ) -> Result<Vec<ConnectionId>, Refusal> {
        let target = match (receiver, place) {
            (Receiver::Target, Some(place)) => self.designated_seat(place.plane)?,
            _ => None,
        };
        let observing = place.is_none() && matches!(self.life, Life::Flying);
        let mut hearers = Vec::new();
        for (&connection, peer) in &self.peers {
            if connection == sender || matches!(peer.stage, Stage::Closing { .. }) {
                continue;
            }
            let hears = match receiver {
                // An observer's line stays among observers while the mission
                // flies; everyone else's All reaches everyone.
                Receiver::All => !(observing && self.place_of(connection).is_some()),
                _ => {
                    // Everything but All goes to those flying, and the
                    // sender flies (a lobby sender was refused before).
                    let (Some(sender), Some(theirs)) = (place, self.place_of(connection)) else {
                        continue;
                    };
                    match receiver {
                        Receiver::All => true,
                        Receiver::Friendlies => theirs.wing.side == sender.wing.side,
                        Receiver::Enemies => theirs.wing.side != sender.wing.side,
                        Receiver::Wing => theirs.wing == sender.wing,
                        Receiver::Target => target.is_some() && peer.seat == target,
                    }
                }
            };
            if hears {
                hearers.push(connection);
            }
        }
        Ok(hearers)
    }

    /// The seat flying the aircraft `plane` has designated: `None` when
    /// an AI flies it, a refusal when `plane` has no target or it is not
    /// an aircraft.
    fn designated_seat(&self, plane: PlaneId) -> Result<Option<SeatId>, Refusal> {
        let id = self.designated_aircraft(plane).ok_or(Refusal::NoTarget)?;
        Ok(
            match self.world.roster.plane(PlaneId(id)).map(|p| p.pilot) {
                Some(Pilot::Human(seat)) => Some(seat),
                _ => None,
            },
        )
    }

    /// The aircraft `plane` has designated, when it has and the target is an
    /// aircraft (a ground target has no pilot to hear).
    fn designated_aircraft(&self, plane: PlaneId) -> Option<u32> {
        // The synthetic test import's radar sees nothing, so tests choose
        // the designation.
        #[cfg(test)]
        if let Some(id) = self.test_designations.get(&plane) {
            return Some(*id);
        }
        let view = self.world.combat.state.view(plane.0)?;
        let target = view.contact(view.designated()?)?;
        (target.role == TargetRole::Aircraft).then_some(target.id)
    }
}

/// `Some` when the player flies a plane now.
fn flying(peer: &Peer) -> Option<()> {
    (peer.stage == Stage::Seated).then_some(())
}
