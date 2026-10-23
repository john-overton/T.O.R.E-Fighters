//! The results at a mission's end (stage F phase 2; docs/ARCHITECTURE.md,
//! "The multiplayer debrief"): every plane's row, with its pilot and, in PvP,
//! the final scores, sent to every connection.
//!
//! The mission core reads each plane's row (`tore_world::debrief::results`:
//! status, damage, kills, friendly fire, shots and hits, retired planes
//! included); the host adds who flew it. A plane's callsign is its last human
//! pilot's, kept even after that player leaves ([`Callsigns`]); a plane only
//! the AI ever flew has none.

use super::{ConnectionId, Host, Life, Stage};
use crate::settings::Mode;
use crate::wire::messages::{EndReason, Message, ResultRow, ResultStatus, Results, Shots};
use std::collections::BTreeMap;
use tore_sim::combat::ledger::Tally;
use tore_world::debrief::{PlaneResult, RowStatus};
use tore_world::seats::PlaneId;

/// The most rows a Results message holds (the wire's limit): far more than
/// a mission's planes, retired ones included, ever come to.
const ROWS_LIMIT: usize = 1_024;

/// The callsign of the last human who flew each plane this mission.
pub(super) type Callsigns = BTreeMap<PlaneId, String>;

fn shots(tally: Tally) -> Shots {
    Shots {
        launched: tally.launched,
        hit: tally.hit,
    }
}

/// A plane's result as the wire carries it.
pub(super) fn row(result: &PlaneResult, callsign: Option<String>) -> ResultRow {
    ResultRow {
        plane: result.plane.0,
        wing: result.slot.wing,
        member: result.slot.member,
        aircraft: result.aircraft,
        callsign,
        status: match result.status {
            RowStatus::Alive => ResultStatus::Alive,
            RowStatus::Ejected => ResultStatus::Ejected,
            RowStatus::Dead => ResultStatus::Dead,
            RowStatus::Retired => ResultStatus::Retired,
        },
        damage: (result.damage * 1000.).round().clamp(0., 1000.) as u16,
        aircraft_kills: result.aircraft_kills,
        other_kills: result.other_kills,
        friendly_fire: result.friendly_fire,
        air_to_air: shots(result.air_to_air),
        gun: shots(result.gun),
        air_to_ground: shots(result.air_to_ground),
    }
}

impl Host {
    /// Remembers who flies each plane now, for the results: called each tick
    /// the mission flies (and once more when it ends).
    pub(super) fn results_note_flyers(&mut self) {
        for peer in self.peers.values() {
            if let Some(plane) = peer.plane
                && self.score.callsigns.get(&plane) != Some(&peer.callsign)
            {
                self.score.callsigns.insert(plane, peer.callsign.clone());
            }
        }
    }

    /// The results as the mission ends for `reason`: a row for every plane,
    /// and the final scores in PvP.
    pub(super) fn results(&mut self, reason: EndReason) -> Results {
        self.results_note_flyers();
        let rows = tore_world::debrief::results(&self.world)
            .iter()
            .map(|result| row(result, self.score.callsigns.get(&result.plane).cloned()))
            .take(ROWS_LIMIT)
            .collect();
        Results {
            reason,
            rows,
            scores: (self.settings.mode() == Mode::Pvp).then(|| self.final_scores(reason)),
        }
    }

    /// At the mission's end, before Mission ended: sends Results to every
    /// connection, the observers and the players in the lobby included.
    pub(super) fn send_results(&mut self, reason: EndReason) {
        if !matches!(self.life, Life::Flying) {
            return;
        }
        let message = Message::Results(Box::new(self.results(reason)));
        let ids: Vec<ConnectionId> = self
            .peers
            .iter()
            .filter(|(_, peer)| !matches!(peer.stage, Stage::Closing { .. }))
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.send(id, &message);
        }
    }
}
