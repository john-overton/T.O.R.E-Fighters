//! The lead hold: a human who leads a flight keeps its lead while dead,
//! waiting to revive or flying back, and gets it back when a new plane of
//! theirs joins the flight. The lobby pass's slice R2 (John, 2026-10-09);
//! docs/ARCHITECTURE.md, "Lead succession", and docs/spec/ai.md, "Leader
//! succession".
//!
//! The host turns the hold on for a game whose `respawn` rule is not `none`
//! ([`super::MissionCommand::LeadHold`]); single player never does, and with
//! it off nothing here runs, so today's succession rule is unchanged there.
//!
//! - **Owners.** Each wing a human leads has an *owner*: the seat of that
//!   human ([`LeadOwner::Seat`]), or the plane the AI flies for an away or
//!   dropped player ([`LeadOwner::Away`]). A wing gains an owner when a human
//!   flies its leading plane (at the start, by a handoff, or as the successor
//!   of a lost AI lead), and when its AI lead is lost while the only humans
//!   in the flight wait to revive (the lowest of them).
//! - **Claims.** Before each AI step every owned wing's *claim* is handed to
//!   the AI mission ([`tore_sim::ai::mission::LeadClaim`]): the owner's
//!   current plane in the wing, or none. The mission crowns a claimed plane
//!   that flies and does not lead, and marks a successor chosen while the
//!   claim does not fly as a *stand-in* (`acting`), so every system that
//!   reads the leader works unchanged.
//! - **Who is in the flight.** A human whose current plane is in the wing:
//!   flying, or lost and held for them (in flight or in the lobby with a held
//!   seat). Order is the plane's member number, then the seat.
//! - **Leaving.** A GiveBack turns a seat owner into an away owner (the AI
//!   flies the same plane, which keeps the lead); taking that plane back, or
//!   reviving from it, makes the taker's seat the owner again. When the host
//!   says the owner has left the game ([`super::MissionCommand::LeadLeft`]),
//!   or the owner flies in another wing now (an `ai-slot` revival elsewhere),
//!   the lead goes to the next human in the flight, else the owner is
//!   cleared and the current leader leads as an ordinary lead: the AI loop.
//! - **The HUD lines** (`radio_calls.rs`): a human stand-in reads "You lead
//!   the flight until Blue one flies again.", an owner given the lead back
//!   "You lead your flight again.", in place of the "You're the Wingleader
//!   now" call. A new owner (the next human, or a lost human made owner who
//!   now flies) hears today's call.
//!
//! Everything here is mission state in the revival section of the exact
//! checkpoint, changed only by mission commands and the tick itself, so a
//! standby replaying the journal holds the same owners.

use super::{Cue, TickOutput, World};
use crate::{
    ai_wings::{ENEMY_SIDE, FRIENDLY_SIDE},
    radio_calls,
    seats::{Pilot, PlaneId, SeatId},
};
use tore_sim::ai::{
    launch::{Side, WingId},
    mission::LeadClaim,
};

/// Who holds a flight's lead under the lead hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeadOwner {
    /// The human of this seat: flying, or holding a lost plane.
    Seat(SeatId),
    /// The human whose plane this is, which the AI flies (or lost) for them
    /// while they are away or dropped.
    Away(PlaneId),
}

/// One owned wing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owned {
    pub wing: WingId,
    pub owner: LeadOwner,
    /// The owner's newest plane in the wing: the one a stand-in's line
    /// names.
    pub plane: PlaneId,
    /// The owner has led the wing since it became the owner: crowning it
    /// again gives the lead back ("You lead your flight again."), crowning
    /// it the first time makes it the new lead ("You're the Wingleader
    /// now").
    pub led: bool,
}

/// The lead hold's state: whether it is on, and every owned wing in the
/// order it gained its owner.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LeadHold {
    pub(super) on: bool,
    pub(super) owners: Vec<Owned>,
}

impl LeadHold {
    /// Whether the host turned the hold on.
    pub fn on(&self) -> bool {
        self.on
    }

    /// Every owned wing.
    pub fn owners(&self) -> &[Owned] {
        &self.owners
    }
}

/// The combat side a wing's side is in the AI mission.
fn mission_side(side: Side) -> tore_sim::ai::targeting::Side {
    match side {
        Side::Friendly => FRIENDLY_SIDE,
        Side::Enemy => ENEMY_SIDE,
    }
}

impl World {
    /// Whether the lead hold is on (the host's, for a game with revival).
    pub fn lead_hold(&self) -> bool {
        self.revival.lead_hold.on
    }

    /// Every owned wing (empty with the hold off).
    pub fn lead_owners(&self) -> &[Owned] {
        &self.revival.lead_hold.owners
    }

    /// Who owns `wing`'s lead, if anyone does.
    pub fn lead_owner(&self, wing: WingId) -> Option<LeadOwner> {
        self.owned(wing).map(|owned| owned.owner)
    }

    fn owned(&self, wing: WingId) -> Option<&Owned> {
        self.revival
            .lead_hold
            .owners
            .iter()
            .find(|owned| owned.wing == wing)
    }

    /// Whether the AI mission marks `wing`'s current leader as a stand-in
    /// for its owner.
    pub fn lead_acting(&self, wing: WingId) -> bool {
        self.ai_wings.as_ref().is_some_and(|wings| {
            wings
                .mission()
                .wing_acting(mission_side(wing.side), wing.index)
        })
    }

    /// The hold on or off ([`super::MissionCommand::LeadHold`]). Off clears
    /// every owner, so today's rule holds from the next step.
    pub(super) fn set_lead_hold(&mut self, on: bool) {
        let hold = &mut self.revival.lead_hold;
        hold.on = on;
        if !on {
            hold.owners.clear();
        }
    }

    /// The humans in `wing`'s flight, in order: (member, seat, plane) for
    /// each seat whose current plane is in the wing, flying or lost.
    fn flight_humans(&self, wing: WingId) -> Vec<(u8, SeatId, PlaneId)> {
        let mut humans: Vec<(u8, SeatId, PlaneId)> = self
            .roster
            .seats()
            .iter()
            .filter_map(|seat| {
                let plane = self.roster.plane(seat.plane?)?;
                (plane.slot.wing == wing && plane.pilot == Pilot::Human(seat.id)).then_some((
                    plane.slot.member,
                    seat.id,
                    plane.id,
                ))
            })
            .collect();
        humans.sort();
        humans
    }

    /// `seat`'s current plane, if it is in `wing`.
    fn seat_plane_in(&self, seat: SeatId, wing: WingId) -> Option<PlaneId> {
        let plane = self.roster.plane(self.roster.seat(seat)?.plane?)?;
        (plane.slot.wing == wing).then_some(plane.id)
    }

    /// The owner of `wing` has left its flight: the lead goes to the next
    /// human in the flight (other than a leaving seat), else the wing has no
    /// owner and its current leader leads as an ordinary one.
    fn pass_lead(&mut self, wing: WingId, leaving: LeadOwner) {
        let next = self
            .flight_humans(wing)
            .into_iter()
            .find(|(_, seat, _)| leaving != LeadOwner::Seat(*seat));
        let owners = &mut self.revival.lead_hold.owners;
        let Some(index) = owners.iter().position(|owned| owned.wing == wing) else {
            return;
        };
        match next {
            Some((_, seat, plane)) => {
                owners[index] = Owned {
                    wing,
                    owner: LeadOwner::Seat(seat),
                    plane,
                    led: false,
                }
            }
            None => {
                owners.remove(index);
            }
        }
    }

    /// The host says `owner` has left the game
    /// ([`super::MissionCommand::LeadLeft`]): every wing it owns passes on.
    pub(super) fn lead_left(&mut self, owner: LeadOwner) {
        let wings: Vec<WingId> = self
            .lead_owners()
            .iter()
            .filter(|owned| owned.owner == owner)
            .map(|owned| owned.wing)
            .collect();
        for wing in wings {
            self.pass_lead(wing, owner);
        }
    }

    /// `seat` gave `plane` back to the AI (leaving, dropping or going away):
    /// an owner of `plane`'s wing becomes an away owner of `plane`, which
    /// keeps the lead it has.
    pub(super) fn lead_given_back(&mut self, seat: SeatId, plane: PlaneId) {
        let Some(wing) = self.roster.plane(plane).map(|p| p.slot.wing) else {
            return;
        };
        for owned in &mut self.revival.lead_hold.owners {
            if owned.wing == wing && owned.owner == LeadOwner::Seat(seat) {
                owned.owner = LeadOwner::Away(plane);
                owned.plane = plane;
            }
        }
    }

    /// `seat` took `plane`, or revived from it while it was lost (a player
    /// back from away): an away owner of `plane` is that seat again.
    pub(super) fn lead_taken(&mut self, seat: SeatId, plane: PlaneId) {
        for owned in &mut self.revival.lead_hold.owners {
            if owned.owner == LeadOwner::Away(plane) {
                owned.owner = LeadOwner::Seat(seat);
            }
        }
    }

    /// Before the AI step: an owner flying in another wing has left its
    /// flight, and every owned wing's claim goes to the AI mission. With the
    /// hold off the mission is given no claims (and only cleared if it had
    /// some), so single player's step is today's.
    pub(super) fn lead_before_ai(&mut self) {
        if !self.lead_hold() {
            if let Some(wings) = self.ai_wings.as_mut()
                && !wings.mission().lead_claims().is_empty()
            {
                wings.set_lead_claims(Vec::new());
            }
            return;
        }
        let elsewhere: Vec<(WingId, LeadOwner)> = self
            .lead_owners()
            .iter()
            .filter_map(|owned| {
                let LeadOwner::Seat(seat) = owned.owner else {
                    return None;
                };
                let plane = self.roster.plane(self.roster.seat(seat)?.plane?)?;
                (plane.slot.wing != owned.wing).then_some((owned.wing, owned.owner))
            })
            .collect();
        for (wing, owner) in elsewhere {
            self.pass_lead(wing, owner);
        }
        let claims: Vec<LeadClaim> = self
            .lead_owners()
            .iter()
            .map(|owned| LeadClaim {
                side: mission_side(owned.wing.side),
                wing: owned.wing.index,
                plane: match owned.owner {
                    LeadOwner::Seat(seat) => self.seat_plane_in(seat, owned.wing),
                    LeadOwner::Away(plane) => self
                        .roster
                        .plane(plane)
                        .filter(|p| p.slot.wing == owned.wing)
                        .map(|p| p.id),
                }
                .map(|plane| plane.0),
                fresh: !owned.led,
            })
            .collect();
        if let Some(wings) = self.ai_wings.as_mut() {
            wings.set_lead_claims(claims);
        }
    }

    /// After the AI step: owners from the tick's lead changes and the
    /// wings' leaders, each owner's newest plane and whether it has led, and
    /// the HUD lines of a stand-in and of an owner given the lead back.
    pub(super) fn lead_after_ai(&mut self, out: &mut TickOutput) {
        if !self.lead_hold() {
            return;
        }
        let Some(wings) = self.ai_wings.as_ref() else {
            return;
        };
        let mission = wings.mission();
        let wing_of = |side: tore_sim::ai::targeting::Side, index: u8| WingId {
            side: if side == ENEMY_SIDE {
                Side::Enemy
            } else {
                Side::Friendly
            },
            index,
        };
        let changes = wings.last_output().leadership.clone();
        // Every wing of the mission, in plane order.
        let mut all: Vec<WingId> = Vec::new();
        for plane in self.roster.planes() {
            if !all.contains(&plane.slot.wing) {
                all.push(plane.slot.wing);
            }
        }
        let leaders: Vec<(WingId, Option<u32>)> = all
            .iter()
            .map(|wing| {
                (
                    *wing,
                    mission.wing_leader(mission_side(wing.side), wing.index),
                )
            })
            .collect();
        // An AI lead lost while every human in the flight waits to revive:
        // the lowest of them owns the lead from now on.
        for change in &changes {
            let wing = wing_of(change.side, change.wing);
            let human_led = self.roster.seat_of(PlaneId(change.leader)).is_some();
            if self.owned(wing).is_some() || human_led {
                continue;
            }
            if let Some(&(_, seat, plane)) = self.flight_humans(wing).first() {
                self.revival.lead_hold.owners.push(Owned {
                    wing,
                    owner: LeadOwner::Seat(seat),
                    plane,
                    led: false,
                });
            }
        }
        // A human flies a wing's leading plane and nobody owns the wing: the
        // human does, from the start, a handoff or a succession.
        for &(wing, leader) in &leaders {
            if self.owned(wing).is_some() {
                continue;
            }
            let Some(leader) = leader.map(PlaneId) else {
                continue;
            };
            if let Some(seat) = self.roster.seat_of(leader) {
                self.revival.lead_hold.owners.push(Owned {
                    wing,
                    owner: LeadOwner::Seat(seat),
                    plane: leader,
                    led: true,
                });
            }
        }
        // Each owner's newest plane, and whether it has led.
        let current: Vec<Option<PlaneId>> = self
            .lead_owners()
            .iter()
            .map(|owned| match owned.owner {
                LeadOwner::Seat(seat) => self.seat_plane_in(seat, owned.wing),
                LeadOwner::Away(plane) => Some(plane),
            })
            .collect();
        for (owned, plane) in self.revival.lead_hold.owners.iter_mut().zip(current) {
            let Some(plane) = plane else { continue };
            owned.plane = plane;
            let leads = leaders
                .iter()
                .any(|(wing, leader)| *wing == owned.wing && *leader == Some(plane.0));
            owned.led |= leads;
        }
        // The HUD lines: the stand-in's and the owner's given back.
        if changes.iter().all(|c| !c.acting && !c.reclaimed) {
            return;
        }
        let members = radio_calls::members(&self.roster, self.ai_wings.as_ref(), |_| true);
        for change in &changes {
            let Some(seat) = self.roster.seat_of(PlaneId(change.leader)) else {
                continue;
            };
            let text = if change.reclaimed {
                radio_calls::LEAD_AGAIN.to_owned()
            } else if change.acting {
                let owner = self
                    .owned(wing_of(change.side, change.wing))
                    .and_then(|owned| members.iter().find(|m| m.id == owned.plane.0))
                    .map(radio_calls::label);
                radio_calls::stand_in_line(owner.as_deref())
            } else {
                continue;
            };
            out.cues.push(Cue::Message { seat, text });
        }
    }
}

// Exact checkpoints: the lead hold rides in the revival section.
#[path = "lead_hold_checkpoint.rs"]
mod checkpoint;

// The world tests of the lead hold's state machine (slice R2).
#[cfg(test)]
#[path = "lead_hold_tests.rs"]
mod lead_hold_tests;
