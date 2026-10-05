//! Scoring on the host (stage F phase 2; docs/ARCHITECTURE.md, "Scoring"):
//! the tallies by player and side, the limits and the Scores message.
//!
//! The mission core records the facts (`tore_world::score`); the host turns
//! the recording on when a mission starts flying, drains the facts every
//! tick and keeps the tallies:
//!
//! - **Kills**: an opponent's aircraft lost and credited to a plane a player
//!   flies at the kill's tick, or flew last when the plane itself is lost (a
//!   late missile). A human's plane lost with its pilot aboard counts two, any
//!   other aircraft one; ground objects, the AI's kills and kills of one's own
//!   side's aircraft count nothing.
//! - **Damage**: the fractions of an opponent aircraft's full hit points a
//!   player's hits took, so a whole aircraft is 1.0.
//! - **Losses**: a player's plane lost by any cause.
//! - **Ratio**: kills over losses, or kills alone with no loss.
//!
//! Opponents are the other side's aircraft under `sides`; under
//! `free-for-all` they are also the other humans of one's own side (the AI of
//! one's own side never counts). Each side's tally is its players' (those
//! who left included). Players are tallied by their connection, so a lobby
//! id given out again starts afresh; a player who leaves leaves the list.
//!
//! In PvP a kill limit ends the mission ([`EndReason::KillLimit`]) at the
//! tick the kills of every player together (`total`), of one side (`side`)
//! or of one player (`player`) reach it. A mission ended by a kill or time
//! limit in PvP names the winner by the tally: a side under `sides`, a
//! player under `free-for-all`, a draw when the best are level. In co-op the
//! scoring settings other than the time limit do not apply: no kill limit,
//! no winner, the fight by sides.
//!
//! **Scores** (message 31) go to every flying player and every observer
//! whenever the tallies or the players change, at most once a second
//! ([`SCORES_INTERVAL_TICKS`]), to a newly seated player or a new observer at
//! the next of those chances, and to every connection with the mission's end.
//! An observer watching with a delay gets them as its stream reaches their
//! tick (`send_as_of`).

use super::{ConnectionId, Host, Life, Stage, TICKS_PER_SECOND};
use crate::settings::{Fight, KillOwner, Mode, ScoreTally};
use crate::wire::messages::{EndReason, Message, PlayerScore, Scores, SideScore, Winner};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use tore_sim::ai::launch::Side;
use tore_world::score::{Fact, Flown, Victim};
use tore_world::seats::{Pilot, PlaneId, SeatId};
use tore_world::world::TickOutput;

/// The fewest ticks between two Scores messages: one second.
pub const SCORES_INTERVAL_TICKS: u64 = TICKS_PER_SECOND;
/// The most players a Scores message lists (the wire's limit).
const LISTED_PLAYERS: usize = 64;

/// One player's or one side's tally.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Tally {
    pub kills: u32,
    pub losses: u32,
    /// Damage to opponents, in aircraft.
    pub damage: f64,
}

impl Tally {
    /// Kills over losses, or kills alone with no loss.
    pub fn ratio(&self) -> f64 {
        if self.losses == 0 {
            f64::from(self.kills)
        } else {
            f64::from(self.kills) / f64::from(self.losses)
        }
    }

    /// The figure `tally` ranks by.
    pub fn value(&self, tally: ScoreTally) -> f64 {
        match tally {
            ScoreTally::Kills => f64::from(self.kills),
            ScoreTally::Damage => self.damage,
            ScoreTally::Ratio => self.ratio(),
        }
    }

    /// Ranks `self` before `other` by `tally`, then by kills, then by
    /// damage.
    fn rank(&self, other: &Self, tally: ScoreTally) -> Ordering {
        other
            .value(tally)
            .total_cmp(&self.value(tally))
            .then(other.kills.cmp(&self.kills))
            .then(other.damage.total_cmp(&self.damage))
    }

    fn wire(&self) -> SideScore {
        SideScore {
            kills: self.kills,
            losses: self.losses,
            damage: thousandths(self.damage),
        }
    }
}

/// Aircraft as thousandths, as the wire carries damage.
fn thousandths(aircraft: f64) -> u32 {
    (aircraft * 1000.).round().clamp(0., f64::from(u32::MAX)) as u32
}

/// One player's part of the scores.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Player {
    pub tally: Tally,
    /// The side of the plane it flew last this mission.
    pub side: Option<Side>,
}

/// The host's scoring for the mission flying: reset when a mission starts.
#[derive(Clone, Debug, Default)]
pub(super) struct Scoring {
    /// By the player's connection (its `Entry::order`, which no later
    /// connection reuses).
    players: BTreeMap<u64, Player>,
    /// The friendly side's, then the enemy side's.
    sides: [Tally; 2],
    /// The connection that flew each plane last, for a lost plane's late
    /// kills.
    flyers: BTreeMap<PlaneId, u64>,
    /// The callsign of the last human who flew each plane, for the results
    /// (`results.rs`), kept after the player leaves.
    pub(super) callsigns: super::results::Callsigns,
    /// The tallies changed since Scores last went out.
    changed: bool,
    /// The players and sides the last Scores listed.
    listed: Vec<(u64, Option<Side>)>,
    /// Raised each time the scores change; each connection's last.
    version: u64,
    sent: BTreeMap<ConnectionId, u64>,
    /// The tick Scores last went out.
    last_sent: Option<u64>,
}

impl Scoring {
    #[cfg(test)]
    pub(super) fn player(&self, order: u64) -> Option<&Player> {
        self.players.get(&order)
    }

    #[cfg(test)]
    pub(super) fn side(&self, side: Side) -> Tally {
        self.sides[side_index(side)]
    }
}

fn side_index(side: Side) -> usize {
    match side {
        Side::Friendly => 0,
        Side::Enemy => 1,
    }
}

impl Host {
    /// A mission starts flying: fresh tallies, and the mission core records
    /// score facts.
    pub(super) fn score_start(&mut self) {
        self.score = Scoring::default();
        self.world.set_scoring(true);
    }

    /// After the step: drains the tick's score facts into the tallies, ends
    /// the mission at a kill limit, and sends Scores when they change.
    pub(super) fn score_tick(&mut self, tick: u64, _out: &TickOutput) {
        let facts = self.world.take_score_facts();
        // Who flies what now: each seat's connection, and each plane's last.
        let mut seats: BTreeMap<SeatId, u64> = BTreeMap::new();
        for peer in self.peers.values() {
            if let Some(seat) = peer.seat {
                seats.insert(seat, peer.lobby.order);
            }
            if let Some(plane) = peer.plane {
                self.score.flyers.insert(plane, peer.lobby.order);
                let side = self.side_of(plane);
                self.score.players.entry(peer.lobby.order).or_default().side = side;
            }
        }
        self.results_note_flyers();
        for fact in facts.facts {
            self.tally(fact, &seats);
        }
        let connected: Vec<u64> = self.peers.values().map(|p| p.lobby.order).collect();
        self.score
            .players
            .retain(|order, _| connected.contains(order));
        if self.kill_limit_reached() {
            let next = self.after_end();
            self.end_mission(EndReason::KillLimit, next);
            return;
        }
        self.send_scores(tick);
    }

    /// The mission ends: every connection gets the final Scores, with the
    /// winner when a limit ended a PvP mission.
    pub(super) fn score_end(&mut self, reason: EndReason) {
        if !matches!(self.life, Life::Flying) {
            return;
        }
        let scores = self.final_scores(reason);
        let message = Message::Scores(Box::new(scores));
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

    /// The scores as the mission ends for `reason`, with its winner.
    pub(super) fn final_scores(&self, reason: EndReason) -> Scores {
        let winner = match reason {
            EndReason::KillLimit | EndReason::TimeLimit if self.settings.mode() == Mode::Pvp => {
                self.winner()
            }
            _ => Winner::NoneYet,
        };
        self.scores(winner)
    }

    /// The side `plane` flies for.
    fn side_of(&self, plane: PlaneId) -> Option<Side> {
        self.world.roster.plane(plane).map(|p| p.slot.wing.side)
    }

    /// The fight in force: by sides in co-op, whatever the setting says.
    pub(super) fn fight(&self) -> Fight {
        match self.settings.mode() {
            Mode::Coop => Fight::Sides,
            Mode::Pvp => self.settings.fight(),
        }
    }

    /// The kill limit in force: none in co-op.
    pub(super) fn kill_limit(&self) -> Option<u32> {
        match self.settings.mode() {
            Mode::Coop => None,
            Mode::Pvp => self.settings.kill_limit(),
        }
    }

    /// Whether `victim` is an opponent of `shooter`'s plane: on the other
    /// side, or, in a free-for-all, another human's plane on the same side.
    pub(super) fn opponents(&self, shooter: PlaneId, victim: Flown) -> bool {
        if shooter == victim.plane {
            return false;
        }
        let (Some(ours), Some(theirs)) = (self.side_of(shooter), self.side_of(victim.plane)) else {
            return false;
        };
        ours != theirs
            || (self.fight() == Fight::FreeForAll && matches!(victim.pilot, Pilot::Human(_)))
    }

    /// The connection a fact credits `shooter` to: its human pilot at the
    /// fact's tick, or, for a plane lost and abandoned, the last player who
    /// flew it. The AI is credited to nobody.
    fn credited(&self, shooter: Flown, seats: &BTreeMap<SeatId, u64>) -> Option<u64> {
        match shooter.pilot {
            Pilot::Ai => None,
            Pilot::Human(seat) => seats
                .get(&seat)
                .or_else(|| self.score.flyers.get(&shooter.plane))
                .copied(),
            Pilot::Lost => self.score.flyers.get(&shooter.plane).copied(),
        }
    }

    /// Counts one fact.
    pub(super) fn tally(&mut self, fact: Fact, seats: &BTreeMap<SeatId, u64>) {
        // The connection credited (none when it has left: its side still
        // counts), the shooter's side and the victim; `None` when the fact
        // counts for nobody.
        let credit = |host: &Self, shooter: Option<Flown>, victim: &Victim| {
            let (shooter, flown) = (shooter?, victim.flown?);
            if shooter.pilot == Pilot::Ai
                || !victim.aircraft
                || !host.opponents(shooter.plane, flown)
            {
                return None;
            }
            let side = host.side_of(shooter.plane)?;
            Some((host.credited(shooter, seats), side, flown))
        };
        let changed = match fact {
            Fact::Kill {
                shooter,
                victim,
                pilot_aboard,
            } => credit(self, shooter, &victim).map(|(player, side, flown)| {
                let kills = if pilot_aboard && matches!(flown.pilot, Pilot::Human(_)) {
                    2
                } else {
                    1
                };
                self.score.sides[side_index(side)].kills += kills;
                if let Some(player) = player {
                    self.score.players.entry(player).or_default().tally.kills += kills;
                }
            }),
            Fact::Damage {
                shooter,
                victim,
                fraction,
            } => credit(self, shooter, &victim).map(|(player, side, _)| {
                self.score.sides[side_index(side)].damage += fraction;
                if let Some(player) = player {
                    self.score.players.entry(player).or_default().tally.damage += fraction;
                }
            }),
            Fact::Loss { plane, seat } => self.side_of(plane).map(|side| {
                self.score.sides[side_index(side)].losses += 1;
                let player = seats
                    .get(&seat)
                    .or_else(|| self.score.flyers.get(&plane))
                    .copied();
                if let Some(player) = player {
                    self.score.players.entry(player).or_default().tally.losses += 1;
                }
            }),
        };
        self.score.changed |= changed.is_some();
    }

    /// Whether the kill limit in force is reached.
    fn kill_limit_reached(&self) -> bool {
        let Some(limit) = self.kill_limit() else {
            return false;
        };
        let [friendly, enemy] = self.score.sides;
        match self.settings.kill_owner() {
            KillOwner::Total => friendly.kills + enemy.kills >= limit,
            KillOwner::Side => friendly.kills >= limit || enemy.kills >= limit,
            KillOwner::Player => self.score.players.values().any(|p| p.tally.kills >= limit),
        }
    }

    /// The winner by the tally: the better side under `sides`, the best
    /// player under `free-for-all`, a draw when the best are level.
    fn winner(&self) -> Winner {
        let tally = self.settings.tally();
        match self.fight() {
            Fight::Sides => {
                let [friendly, enemy] = self.score.sides.map(|side| side.value(tally));
                match friendly.total_cmp(&enemy) {
                    Ordering::Greater => Winner::Side(Side::Friendly),
                    Ordering::Less => Winner::Side(Side::Enemy),
                    Ordering::Equal => Winner::Draw,
                }
            }
            Fight::FreeForAll => {
                let ranked = self.ranked();
                match ranked.as_slice() {
                    [(best, first), rest @ ..]
                        if rest.first().is_none_or(|(_, second)| {
                            first.tally.value(tally) > second.tally.value(tally)
                        }) =>
                    {
                        Winner::Player(best.lobby.id)
                    }
                    _ => Winner::Draw,
                }
            }
        }
    }

    /// The connected players with their tallies, best first by the tally,
    /// then in lobby order.
    fn ranked(&self) -> Vec<(&super::Peer, Player)> {
        let tally = self.settings.tally();
        let mut players: Vec<(&super::Peer, Player)> = self
            .peers
            .values()
            .filter(|peer| !matches!(peer.stage, Stage::Closing { .. }))
            .map(|peer| {
                let player = self
                    .score
                    .players
                    .get(&peer.lobby.order)
                    .copied()
                    .unwrap_or_default();
                (peer, player)
            })
            .collect();
        players.sort_by(|(a, pa), (b, pb)| {
            pa.tally
                .rank(&pb.tally, tally)
                .then(a.lobby.order.cmp(&b.lobby.order))
        });
        players
    }

    /// The Scores message as things stand, with `winner`.
    fn scores(&self, winner: Winner) -> Scores {
        let ticks = self.world.tick();
        let seconds_left = self.settings.time_limit_seconds().map(|limit| {
            let left = (u64::from(limit) * TICKS_PER_SECOND).saturating_sub(ticks);
            u32::try_from(left.div_ceil(TICKS_PER_SECOND)).unwrap_or(u32::MAX)
        });
        Scores {
            tally: self.settings.tally(),
            fight: self.fight(),
            seconds_left,
            kill_limit: self
                .kill_limit()
                .map_or(0, |limit| u8::try_from(limit.min(15)).unwrap_or(15)),
            kill_owner: self.settings.kill_owner(),
            players: self
                .ranked()
                .into_iter()
                .take(LISTED_PLAYERS)
                .map(|(peer, player)| PlayerScore {
                    id: peer.lobby.id,
                    callsign: peer.callsign.clone(),
                    side: player.side,
                    kills: player.tally.kills,
                    losses: player.tally.losses,
                    damage: thousandths(player.tally.damage),
                })
                .collect(),
            sides: self.score.sides.map(|side| side.wire()),
            winner,
        }
    }

    /// Sends Scores to each flying player that has not had the newest, at
    /// most once a second.
    fn send_scores(&mut self, tick: u64) {
        if self
            .score
            .last_sent
            .is_some_and(|at| tick < at + SCORES_INTERVAL_TICKS)
        {
            return;
        }
        let listed: Vec<(u64, Option<Side>)> = self
            .ranked()
            .iter()
            .map(|(peer, player)| (peer.lobby.order, player.side))
            .collect();
        if std::mem::take(&mut self.score.changed) || listed != self.score.listed {
            self.score.version += 1;
            self.score.listed = listed;
        }
        let version = self.score.version;
        self.score.sent.retain(|id, _| self.peers.contains_key(id));
        let to: Vec<ConnectionId> = self
            .peers
            .iter()
            .filter(|(id, peer)| {
                (peer.stage == Stage::Seated || peer.watch.is_some())
                    && self.score.sent.get(id) != Some(&version)
            })
            .map(|(id, _)| *id)
            .collect();
        if to.is_empty() {
            return;
        }
        let message = Message::Scores(Box::new(self.scores(Winner::NoneYet)));
        for id in to {
            // An observer watching with a delay gets them once its stream
            // shows this tick (slice F2-O1's `send_as_of`).
            self.send_as_of(id, tick, message.clone());
            self.score.sent.insert(id, version);
        }
        self.score.last_sent = Some(tick);
    }
}
