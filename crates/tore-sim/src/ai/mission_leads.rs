//! What an AI lead does with the flight data link, and what its wingmen do
//! when a flightmate holds their bandit (slice G4; the guide is
//! `docs/DATALINK.md`, "Giving assignments", and the design is
//! `docs/ARCHITECTURE.md`, "Flight data link").
//!
//! A lead whose target changes this tick ([`AiMission::lead_commits`], run
//! after every actor has decided, as the automatic wing requests are) either
//!
//! - **sorts**: when the flight has not sorted for thirty seconds and the side
//!   knows another bandit within reach of the lead, each fit wingman takes a
//!   different one by [`crate::datalink::sort`], or
//! - **shares**: its target goes to the wingmen that have none, up to the
//!   two-attacker allowance (B43, [`wing::share_targets`]).
//!
//! Both only under loose control, which is the rule B43 gives the share, and
//! only to AI wingmen that fly the mission's free engagement (agent decisions).
//! A wingman takes the target as the controller takes any target order and
//! keeps its own mission role, so once the target is gone it chooses again as
//! it did before; the mission's role-bound wingmen (escort, patrol, intercept
//! and the like) are left alone because their role already says what they may
//! attack. Each target given is a [`LinkEvent::Assign`] for the world to
//! record and voice.
//!
//! A yield ([`AiMission::yield_target`]) is the other half: the world finds
//! two flightmates locked on one aircraft that the lead did not mean, and the
//! member with the higher number, if an AI, leaves that aircraft alone for ten
//! seconds when it has another target to take. A human is never moved.

use super::{AiActor, AiMission, MissionOutput};
use crate::ai::engagement::{Role, Stance};
use crate::ai::link::{
    Engagements, LinkEvent, LinkInput, MemberState, SORT_INTERVAL_TICKS, SortStamp, YIELD_TICKS,
    Yield,
};
use crate::ai::targeting::Side;
use crate::ai::wing::{self, AttackerCap, PlayerOrder, TargetId, TargetOrder, WingRequest};
use crate::datalink::sort::{self, Wingman};

impl AiActor {
    /// Whether this actor takes a target a lead shares or sorts to it: a
    /// living, released AI aircraft in free flight that flies the mission's
    /// free engagement, so any hostile aircraft is a legal choice for it.
    pub(super) fn accepts_shared(&self) -> bool {
        self.alive
            && !self.dummy
            && !self.neutral
            && !self.bugged_out
            && self.damage_return.is_none()
            && self.landing_order.is_none()
            && self.airfield.is_none()
            && self.flight.escape.is_none()
            && self.assignment.stance == Stance::EngageAssigned
            && self.assignment.role == Role::FreeEngagement
    }

    /// Whether this actor, as a lead, may give its wingmen targets: alive,
    /// released, flying, and on loose control.
    fn may_lead_shares(&self, default_control: wing::WingControl) -> bool {
        let control = self.controller.wing_settings().0.unwrap_or(default_control);
        self.alive
            && !self.dummy
            && !self.neutral
            && !self.bugged_out
            && self.damage_return.is_none()
            && self.landing_order.is_none()
            && self.airfield.is_none()
            && self.flight.escape.is_none()
            && wing::leader_shares_target(control)
    }

    /// Takes `target` as a lead's order: the controller's target order, the
    /// mission's role untouched. `false` when the actor does not accept it.
    fn take_shared_target(&mut self, target: u32, tick: u64) -> bool {
        if !self.accepts_shared() {
            return false;
        }
        self.controller
            .prepare_order(self.flight.yaw.to_degrees(), self.speed_limits());
        let request = WingRequest::TargetAssignment(TargetOrder::ConcreteTarget(TargetId(target)));
        matches!(
            self.controller.receive_order(request, tick),
            Ok(wing::ReceiverOutcome::Applied(_) | wing::ReceiverOutcome::MotionInstalled(_))
        )
    }

    /// The bandits this actor has agreed to leave alone, with when each
    /// lapses.
    pub fn yields(&self) -> &[Yield] {
        &self.yields
    }
}

impl AiMission {
    /// The AI leads that changed target this tick give their wingmen targets
    /// (see the module). `commits` are the actor indices of the leads, in
    /// actor order; `engagements` is the decision-order table after the loop.
    pub(super) fn lead_commits(
        &mut self,
        commits: &[usize],
        link: &LinkInput,
        engagements: &Engagements,
        output: &mut MissionOutput,
    ) {
        let tick = self.tick;
        for &index in commits {
            let lead = &self.actors[index];
            let Some(target) = lead.controller.target() else {
                continue;
            };
            if !lead.may_lead_shares(self.wing_control)
                || self
                    .external_leader(lead.identity.side, lead.identity.wing)
                    .is_some()
            {
                continue;
            }
            let (lead_id, side, wing_index, position) = (
                lead.id(),
                lead.identity.side,
                lead.identity.wing,
                lead.flight.position,
            );
            // The wingmen that take a target, in member order.
            let mut wingmen: Vec<usize> = (0..self.actors.len())
                .filter(|&i| {
                    let a = &self.actors[i];
                    i != index
                        && a.identity.side == side
                        && a.identity.wing == wing_index
                        && a.accepts_shared()
                })
                .collect();
            wingmen.sort_by_key(|&i| (self.actors[i].identity.member, self.actors[i].id()));
            if wingmen.is_empty() {
                continue;
            }
            let fit = |actors: &[AiActor], i: usize| {
                !link
                    .state_of(actors[i].id())
                    .is_some_and(MemberState::skipped)
            };
            if self.sort_due(side, wing_index, tick) {
                let rows: Vec<Wingman> = wingmen
                    .iter()
                    .map(|&i| {
                        let actor = &self.actors[i];
                        let state = link.state_of(actor.id());
                        Wingman {
                            id: actor.id(),
                            member: actor.identity.member,
                            position: actor.flight.position,
                            winchester: state.is_some_and(|s| s.winchester),
                            bingo: state.is_some_and(|s| s.bingo),
                            heavy_damage: state.is_some_and(|s| s.heavy_damage),
                        }
                    })
                    .collect();
                let sorted = sort::sort(position, Some(target), link.bandits_of(side), &rows);
                if !sorted.given.is_empty() {
                    self.stamp_sort(side, wing_index, tick);
                    for (receiver, bandit) in sorted.given {
                        let already = self
                            .actor(receiver)
                            .is_some_and(|a| a.controller.target() == Some(bandit));
                        if !already && self.give_target(receiver, bandit) {
                            output.link.push(LinkEvent::Assign {
                                lead: lead_id,
                                receiver,
                                target: bandit,
                                order: PlayerOrder::Sort,
                            });
                        }
                    }
                    continue;
                }
            }
            // The share: the idle, fit wingmen take the lead's target, up to
            // the allowance, the lead counted among the attackers.
            let attacking = 1 + engagements
                .wing_targets(lead_id, side, wing_index)
                .iter()
                .filter(|id| **id == target)
                .count() as u32;
            let idle: Vec<u32> = wingmen
                .iter()
                .filter(|&&i| self.actors[i].controller.target().is_none() && fit(&self.actors, i))
                .map(|&i| self.actors[i].id())
                .collect();
            let shared = wing::share_targets(AttackerCap::Two, attacking, idle.len() as u32);
            for receiver in idle.into_iter().take(shared.assigned as usize) {
                if self.give_target(receiver, target) {
                    output.link.push(LinkEvent::Assign {
                        lead: lead_id,
                        receiver,
                        target,
                        order: PlayerOrder::EngageMyTarget,
                    });
                }
            }
        }
    }

    /// Gives `receiver` `target` as a lead's order.
    fn give_target(&mut self, receiver: u32, target: u32) -> bool {
        let tick = self.tick;
        self.actor_mut(receiver)
            .is_some_and(|actor| actor.take_shared_target(target, tick))
    }

    /// Whether the flight may sort now: it never has, or the last sort is
    /// thirty seconds old.
    pub(super) fn sort_due(&self, side: Side, wing: u8, tick: u64) -> bool {
        self.sort_clock
            .iter()
            .find(|stamp| stamp.side == side && stamp.wing == wing)
            .is_none_or(|stamp| tick >= stamp.tick + SORT_INTERVAL_TICKS)
    }

    pub(super) fn stamp_sort(&mut self, side: Side, wing: u8, tick: u64) {
        match self
            .sort_clock
            .iter_mut()
            .find(|stamp| stamp.side == side && stamp.wing == wing)
        {
            Some(stamp) => stamp.tick = tick,
            None => self.sort_clock.push(SortStamp { side, wing, tick }),
        }
    }

    /// The tick `side`'s wing `wing` last sorted, if it has.
    pub fn last_sort(&self, side: Side, wing: u8) -> Option<u64> {
        self.sort_clock
            .iter()
            .find(|stamp| stamp.side == side && stamp.wing == wing)
            .map(|stamp| stamp.tick)
    }

    /// The world found `actor` and a flightmate locked on `target` that the
    /// lead did not mean: `actor` leaves it alone for ten seconds, if it has
    /// another target to take. `false` for an aircraft that is no living AI
    /// actor (a human is never moved).
    pub fn yield_target(&mut self, actor: u32, target: u32) -> bool {
        let until = self.tick + YIELD_TICKS;
        let Some(actor) = self.actor_mut(actor).filter(|a| a.alive && !a.dummy) else {
            return false;
        };
        match actor.yields.iter_mut().find(|y| y.target == target) {
            Some(held) => held.until = until,
            None => actor.yields.push(Yield {
                target,
                until,
                announced: false,
            }),
        }
        true
    }
}
