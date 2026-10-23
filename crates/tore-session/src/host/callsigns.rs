//! The players' callsigns in the mission core (the lobby pass's follow-up
//! F1; docs/ARCHITECTURE.md, "Lead succession"): before a tick's commands
//! seat a player (a take, an `ai-slot` revival's take, a revival), the host
//! names the seat's player to the world with
//! [`MissionCommand::Callsign`], when the world does not already know that
//! name for the seat. Seats are reused by the next player, so the name goes
//! with each seating. The world keeps the name on every plane the player
//! flies, which is how the lead hold's HUD line names an owner ("You lead the
//! flight until Viper flies again."). The command is journalled with the
//! tick's others, so a standby knows the same names.

use super::{ConnectionId, Host, Stage};
use tore_world::seats::SeatId;
use tore_world::world::MissionCommand;

impl Host {
    /// The connection the tick's commands seat in `seat`: a player taking a
    /// plane, or one a revival seats.
    fn seating(&self, seat: SeatId) -> Option<ConnectionId> {
        self.peers
            .iter()
            .find(|(_, peer)| matches!(peer.stage, Stage::Taking { seat: s, .. } if s == seat))
            .map(|(id, _)| *id)
            .or_else(|| self.revival.making_connection(seat))
    }

    /// Names the player of every seat `commands` seat, ahead of the command
    /// that seats it, when the world does not know the name yet.
    pub(super) fn callsign_commands(&self, commands: &mut Vec<MissionCommand>) {
        let mut named: Vec<SeatId> = Vec::new();
        let mut out = Vec::with_capacity(commands.len());
        for command in std::mem::take(commands) {
            let seated = match command {
                MissionCommand::Take { seat, .. }
                | MissionCommand::Revive { seat, .. }
                | MissionCommand::ReviveLost { seat, .. } => Some(seat),
                _ => None,
            };
            if let Some(seat) = seated
                && !named.contains(&seat)
                && let Some(callsign) = self
                    .seating(seat)
                    .and_then(|connection| self.peers.get(&connection))
                    .map(|peer| peer.callsign.clone())
                && self.world.roster.seat_callsign(seat) != Some(callsign.as_str())
            {
                named.push(seat);
                out.push(MissionCommand::Callsign { seat, callsign });
            }
            out.push(command);
        }
        *commands = out;
    }
}
