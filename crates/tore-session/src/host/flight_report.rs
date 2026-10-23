//! Multiplayer accounting and the ongoing report saved by each client.
//! Totals belong to a connection's stable join order, across all its lives.
use super::{Host, Stage, debrief_message, debrief_pilot};
use crate::wire::messages::{Debrief, DebriefPilot, EndReason, FlightReport, Message};
use std::collections::{BTreeMap, BTreeSet};
use tore_net::ConnectionId;
use tore_sim::combat::ledger::Tally;
use tore_world::seats::{Pilot, PlaneId};

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Accounts {
    pub totals: BTreeMap<u64, DebriefPilot>,
    pub previous: BTreeMap<u32, DebriefPilot>,
    pub ai: BTreeMap<u32, DebriefPilot>,
    pub last_ai: BTreeSet<u32>,
    pub side_hits: [u32; 2],
}

pub(super) fn hits(pilot: &DebriefPilot) -> u32 {
    [
        pilot.air_to_air,
        pilot.air_to_ground,
        pilot.gun,
        pilot.bombs,
    ]
    .iter()
    .fold(0u32, |sum, tally| sum.saturating_add(tally.hit))
}

/// Apply a counter's signed change. Spoof-to-hit corrections can decrease
/// a counter, so adding only positive deltas would double count outcomes.
fn change(total: &mut u32, now: u32, before: u32) {
    *total = total.saturating_add(now).saturating_sub(before);
}

fn tally(total: &mut Tally, now: Tally, before: Tally) {
    change(&mut total.launched, now.launched, before.launched);
    change(&mut total.hit, now.hit, before.hit);
    change(&mut total.damage, now.damage, before.damage);
    change(&mut total.missed, now.missed, before.missed);
    change(&mut total.spoofed, now.spoofed, before.spoofed);
    change(&mut total.jammed, now.jammed, before.jammed);
}

fn counters(total: &mut DebriefPilot, now: &DebriefPilot, before: &DebriefPilot) {
    for ((total, now), before) in total.kills.iter_mut().zip(now.kills).zip(before.kills) {
        change(total, now, before);
    }
    change(
        &mut total.friendly_fire,
        now.friendly_fire,
        before.friendly_fire,
    );
    tally(&mut total.air_to_air, now.air_to_air, before.air_to_air);
    tally(
        &mut total.air_to_ground,
        now.air_to_ground,
        before.air_to_ground,
    );
    tally(&mut total.gun, now.gun, before.gun);
    tally(&mut total.bombs, now.bombs, before.bombs);
    tally(&mut total.enemy_aam, now.enemy_aam, before.enemy_aam);
    tally(&mut total.enemy_sam, now.enemy_sam, before.enemy_sam);
    tally(&mut total.enemy_gun, now.enemy_gun, before.enemy_gun);
    tally(&mut total.enemy_aaa, now.enemy_aaa, before.enemy_aaa);
}

impl Host {
    /// Called before ownership changes and before reading a report. Lost
    /// aircraft keep credit for late weapons; aircraft returned to AI do not.
    pub(super) fn report_update(&mut self) {
        for (row, pilot) in tore_world::debrief::pilot_results(&self.world) {
            let now = debrief_pilot(&pilot);
            let before = self
                .score
                .accounts
                .previous
                .insert(row.plane.0, now.clone());
            let before = before.unwrap_or_default();
            let side = usize::from(row.slot.wing.side.is_enemy());
            change(
                &mut self.score.accounts.side_hits[side],
                hits(&now),
                hits(&before),
            );
            self.score.changed |= hits(&now) != hits(&before);
            let ai = match self.world.roster.plane(row.plane).map(|p| p.pilot) {
                Some(Pilot::Ai) => {
                    self.score.accounts.last_ai.insert(row.plane.0);
                    true
                }
                Some(_) => {
                    self.score.accounts.last_ai.remove(&row.plane.0);
                    false
                }
                None => self.score.accounts.last_ai.contains(&row.plane.0),
            };
            let total = if ai || !self.score.flyers.contains_key(&row.plane) {
                let root = self.world.revival.root_of(row.plane).0;
                self.score.accounts.ai.entry(root).or_default()
            } else {
                let order = self.score.flyers[&row.plane];
                self.score.accounts.totals.entry(order).or_default()
            };
            counters(total, &now, &before);
        }
    }

    /// Seating starts at this aircraft's current counters, excluding any
    /// earlier pilot's work. It never clears this player's mission totals.
    pub(super) fn report_seated(&mut self, connection: ConnectionId, plane: PlaneId) {
        let order = self.peers[&connection].lobby.order;
        self.score.accounts.totals.entry(order).or_default();
        if let Some((_, pilot)) = tore_world::debrief::pilot_results(&self.world)
            .into_iter()
            .find(|(row, _)| row.plane == plane)
        {
            self.score
                .accounts
                .previous
                .insert(plane.0, debrief_pilot(&pilot));
        }
        self.score.accounts.last_ai.remove(&plane.0);
        self.score.flyers.insert(plane, order);
    }

    pub(super) fn player_report(&self, connection: ConnectionId) -> Option<Debrief> {
        let peer = self.peers.get(&connection)?;
        let mut report = debrief_message(&tore_world::debrief::capture(&self.world, peer.seat?)?);
        if let Some(total) = self.score.accounts.totals.get(&peer.lobby.order) {
            // Physical status describes the latest life; counters describe
            // the player. The wingman and objective rules stay unchanged.
            let current = report.player.clone();
            counters(&mut report.player, total, &current);
            report.success &= report.player.friendly_fire == 0;
        }
        Some(report)
    }

    pub(super) fn send_flight_report(&mut self, connection: ConnectionId) {
        let Some(debrief) = self.player_report(connection) else {
            return;
        };
        let Some(plane) = self.peers[&connection].plane else {
            return;
        };
        let mut results = self.results(EndReason::EveryoneLeft);
        // Co-op also retains the live scores for the partial report.
        results.scores = Some(self.final_scores(EndReason::EveryoneLeft));
        self.send(
            connection,
            &Message::FlightReport(Box::new(FlightReport {
                tick: self.world.tick() as u32,
                plane: plane.0,
                debrief,
                results,
            })),
        );
    }

    pub(super) fn report_tick(&mut self, tick: u64) {
        if !tick.is_multiple_of(super::TICKS_PER_SECOND) {
            return;
        }
        let ids: Vec<_> = self
            .peers
            .iter()
            .filter(|(_, p)| p.stage == Stage::Seated)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.send_flight_report(id);
        }
    }
}
