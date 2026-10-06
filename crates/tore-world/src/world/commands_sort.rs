//! The sort order (slice G3c, Alt+A): the lead hands each addressed wingman a
//! different bandit from the data link's picture. The guide is
//! `docs/DATALINK.md` ("Giving assignments"); the rule is
//! `tore_sim::datalink::sort`, the plan is [`DataLink::sort_plan`].
//!
//! A sort is carried out as one Engage order for each wingman, sent to that
//! wingman alone with the bandit it was dealt, so each takes it exactly as it
//! would take Engage my target (the same checks, the same reply from the first
//! wingman, one journal line each). The assignments are written as sorts. The
//! first assignment call plays at once as the order voice, as every order
//! call does; the rest are queued on the radio 3.5 seconds apart, one for each
//! AI wingman (a human wingman gets the cues of its assignment and no call:
//! that is stage F's order call to human wingmen).

use super::{Cue, TickOutput, World};
use crate::{
    ai_wings,
    comms::{Call, Hearer, journal},
    datalink::{
        SortPlan,
        calls::{self, Addressee, Geometry},
    },
    seats::{PlaneId, SeatId},
};
use tore_sim::ai::wing::PlayerOrder;

/// Seconds between the calls of one sort.
pub const CALL_SPACING_SECONDS: f64 = 3.5;

impl World {
    /// Alt+A from the seat `seat`, flying `plane` from `cockpit`. Only a
    /// plane leading its wing may order it; the wingmen are the ones the
    /// seat's recipient addresses, or the whole flight.
    pub(super) fn sort_order(
        &mut self,
        plane: PlaneId,
        seat: SeatId,
        cockpit: usize,
        out: &mut TickOutput,
    ) {
        let order = PlayerOrder::Sort;
        let tick = self.combat.state.tick();
        let now = tick as f64 / 120.;
        let recipient = self.roster.seat(seat).and_then(|s| s.wing_recipient);
        let designated = self
            .combat
            .state
            .view(self.cockpits[cockpit].plane.0)
            .and_then(|view| view.designated());
        let refused = |world: &mut Self, message: String, out: &mut TickOutput| {
            world
                .comms
                .record(journal::Entry::order_refused(now, order, message.clone()));
            out.cues.push(Cue::Message {
                seat,
                text: message.clone(),
            });
            out.orders.push(super::OrderReply {
                seat,
                order,
                outcome: super::OrderOutcome::Refused { message },
            });
        };
        let Some(wings) = self.ai_wings.as_ref() else {
            refused(self, "Wing order unavailable: no AI wing".into(), out);
            return;
        };
        if !wings.leads_wing(plane.0) {
            refused(
                self,
                "Wing order unavailable: you are not leading your wing".into(),
                out,
            );
            return;
        }
        let plan = self
            .datalink
            .sort_plan(plane.0, recipient, designated)
            .unwrap_or_default();
        if plan.given.is_empty() {
            let message = nothing_to_sort(&plan, recipient);
            refused(self, message, out);
            return;
        }
        // The sender's own flight, for the calls' words and its label.
        let sender = crate::radio_calls::members(&self.roster, self.ai_wings.as_ref(), |_| true)
            .into_iter()
            .find(|member| member.id == plane.0);
        let flight = sender.as_ref().map(|member| member.flight);
        let label = sender
            .as_ref()
            .map_or_else(|| "YOU".to_owned(), crate::radio_calls::label);
        let mut told = 0;
        let mut refused_by_wingman = 0;
        let mut calls = 0;
        let mut failure = None;
        for pick in &plan.given {
            let datalink = &self.datalink;
            let result = self.ai_wings.as_mut().map(|wings| {
                wings.command_linked(
                    plane.0,
                    PlayerOrder::EngageMyTarget,
                    Some(pick.target),
                    Some(pick.member),
                    None,
                    flight,
                    &|target| datalink.tracked(plane.0, target),
                )
            });
            let report = match result {
                Some(Ok(report)) => report,
                Some(Err(error)) => {
                    failure = Some(error.to_string());
                    break;
                }
                None => break,
            };
            if report.reached.is_empty() {
                refused_by_wingman += 1;
                continue;
            }
            told += 1;
            self.datalink
                .assign(tick, plane.0, order, &report.reached, Some(pick.target));
            if report.radio.is_empty() {
                continue;
            }
            if calls == 0 {
                // The first call is the order voice: played at once, cuts off
                // the wing lines still playing and holds the seat's channel.
                self.comms.cut_off(seat, now, journal::Reason::OrderVoice);
                self.comms.spoken(seat, now);
                out.cues.push(Cue::OrderVoice {
                    seat,
                    stems: report.radio,
                });
            } else if let Some(call) = self.sort_call(&label, pick.member, pick.plane, pick.target)
            {
                let cause = journal::Cause::Order {
                    order: PlayerOrder::EngageMyTarget,
                    selected: Some(pick.target),
                    target: Some(pick.target),
                };
                let call = call.after(CALL_SPACING_SECONDS * f64::from(calls)).because(
                    journal::Origin::of(journal::Source::Order, cause).by(ai_wings::PLAYER_ID),
                );
                self.comms.send(now, call, &[Hearer::seat(seat)]);
            }
            calls += 1;
        }
        let (message, failed) = match (failure, told) {
            (Some(error), _) => (error, true),
            (None, 0) => (
                "Wing order unavailable: no wingman could take the sort".into(),
                false,
            ),
            (None, _) => (sort_message(told, refused_by_wingman, &plan), false),
        };
        out.cues.push(Cue::Message {
            seat,
            text: message.clone(),
        });
        out.orders.push(super::OrderReply {
            seat,
            order,
            outcome: if failed {
                super::OrderOutcome::Failed { message }
            } else {
                super::OrderOutcome::Given { message }
            },
        });
    }

    /// The assignment call to wingman `member` ("Three, attack bandit, ...")
    /// as the lead speaks it on the radio, worded from where the wingman and
    /// the bandit fly now. `None` when either has gone from the mission.
    fn sort_call(&self, label: &str, member: u8, wingman: u32, target: u32) -> Option<Call> {
        let wings = self.ai_wings.as_ref()?;
        let from = wings.mission().actor(wingman)?.flight().position;
        let to = wings.mission().actor(target)?.flight().position;
        let words = calls::assignment_phrase(
            &self.phrases,
            Addressee::Wingman(member),
            Geometry::between(from, to),
        );
        Some(calls::assignment_call(label, words))
    }
}

/// Why a sort gave nobody a bandit.
fn nothing_to_sort(plan: &SortPlan, recipient: Option<u8>) -> String {
    if plan.skipped.is_empty() && plan.left.is_empty() {
        match recipient {
            Some(_) => "Wing order unavailable: no such wingman".into(),
            None => "Wing order unavailable: no wingmen".into(),
        }
    } else if plan.left.is_empty() {
        "Sort: every wingman is out of missiles, low on fuel or badly hurt".into()
    } else {
        "Sort: no other bandit in reach".into()
    }
}

/// The pilot's line for a sort that went out.
fn sort_message(told: usize, refused: usize, plan: &SortPlan) -> String {
    let mut message = format!("Sort: {told} assigned");
    if refused > 0 {
        message.push_str(&format!(", {refused} rejected"));
    }
    if !plan.skipped.is_empty() {
        message.push_str(&format!(", {} skipped", plan.skipped.len()));
    }
    if !plan.left.is_empty() {
        message.push_str(&format!(", {} without a bandit", plan.left.len()));
    }
    message
}
