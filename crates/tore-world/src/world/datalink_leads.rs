//! What the world does with an AI lead's data link work (slice G4): the
//! calls it makes when it gives a wingman a target, and the yields it hands
//! the AI when two flightmates lock one aircraft.
//!
//! The assignment itself is recorded by `DataLink::after_ai` (it comes out of
//! the mission's [`LinkEvent`](tore_sim::ai::link::LinkEvent)s); the call is
//! made here, at the end of the AI's half of the tick, so it is queued in the
//! tick, as the wing's chatter is.
//!
//! The call is the assignment call of a human lead ("Two, attack bandit,
//! bearing 270, 15 miles, angels 20"), in the lead's label, said on the
//! flight's net: the seats of the lead's flight hear it, worded from where
//! the receiver flies, and every seat of the side's other flights that
//! monitors the battle net (slice G8) hears it too; a call nobody hears is journaled as unheard like
//! every other call of an AI flight. One lead's calls of a tick follow each
//! other 3.5 seconds apart, as a sort's do.

use super::World;
use crate::{
    comms::{
        Hearer,
        journal::{Audience, Cause, Entry, Origin, Outcome, Reason, Source},
    },
    datalink::{
        LeadAssignment,
        calls::{self, Addressee, Geometry},
    },
    radio_calls,
};

/// Seconds between the calls one lead makes in a tick.
const CALL_SPACING_SECONDS: f64 = 3.5;

impl World {
    /// Makes the call for each target an AI lead gave this tick, in order.
    pub(super) fn voice_lead_assignments(&mut self, given: &[LeadAssignment]) {
        if given.is_empty() {
            return;
        }
        let now = self.combat.state.tick() as f64 / 120.;
        let members = radio_calls::members(&self.roster, self.ai_wings.as_ref(), |plane| {
            self.cockpits
                .iter()
                .position(|cockpit| cockpit.plane == plane)
                .is_some_and(|cockpit| self.cockpit_alive(cockpit))
        });
        // The battle net's hearers (slice G8) are worked out from the radio's
        // own listener rule.
        let listeners: Vec<radio_calls::Listener> = self
            .cockpits
            .iter()
            .enumerate()
            .filter_map(|(index, cockpit)| {
                let seat = self.roster.seat(self.roster.seat_of(cockpit.plane)?)?;
                let member = members.iter().find(|m| m.id == cockpit.plane.0)?;
                Some(radio_calls::Listener {
                    seat: seat.id,
                    plane: cockpit.plane.0,
                    flight: member.flight,
                    enemy: member.enemy,
                    alive: self.cockpit_alive(index),
                    position: cockpit.flight.position,
                    crew: seat.crew,
                })
            })
            .collect();
        let leaders = radio_calls::leaders(&self.roster, &members, self.ai_wings.as_ref());
        let mut spoken: std::collections::BTreeMap<u32, u32> = Default::default();
        for assignment in given {
            let Some(lead) = members.iter().find(|m| m.id == assignment.lead) else {
                continue;
            };
            let Some(receiver) = self.datalink.member(assignment.receiver) else {
                continue;
            };
            let (Some(from), Some(to)) = (
                self.position_of(assignment.receiver),
                self.position_of(assignment.target),
            ) else {
                continue;
            };
            let words = calls::assignment_phrase(
                &self.phrases,
                Addressee::Wingman(receiver.member),
                Geometry::between(from, to),
            );
            let label = radio_calls::label(lead);
            let origin = Origin::of(
                Source::Radio,
                Cause::NewTarget {
                    target: assignment.target,
                },
            )
            .by(assignment.lead)
            .to(Audience::Flight);
            // The seats of the lead's flight that are listening.
            let mut hearers = Vec::new();
            let mut unheard = None;
            for cockpit in 0..self.cockpits.len() {
                let plane = self.cockpits[cockpit].plane;
                let Some(seat) = self.roster.seat_of(plane) else {
                    continue;
                };
                let Some(listener) = members.iter().find(|m| m.id == plane.0) else {
                    continue;
                };
                if listener.enemy != lead.enemy {
                    unheard.get_or_insert(Reason::EnemyFlight);
                } else if listener.flight != lead.flight {
                    unheard.get_or_insert(Reason::OtherFlight);
                } else if !self.cockpit_alive(cockpit) {
                    unheard.get_or_insert(Reason::PlayerDown);
                } else {
                    hearers.push(Hearer::named(seat, label.clone()));
                }
            }
            // The leads of the side's other flights that monitor the battle
            // net hear it too, with the flight colour in front.
            hearers.extend(radio_calls::battle_hearers(
                &self.comms,
                &members,
                &leaders,
                &listeners,
                assignment.lead,
                &|_| words.clone(),
            ));
            if hearers.is_empty() {
                self.comms.record(
                    Entry::note(
                        now,
                        label,
                        origin,
                        Outcome::Unheard(unheard.unwrap_or(Reason::NoRadioIdentity)),
                    )
                    .with_text(words.text)
                    .with_stems(words.stems)
                    .with_kind(crate::comms::Route::Radio, crate::comms::Kind::Important),
                );
                continue;
            }
            let said = spoken.entry(assignment.lead).or_insert(0u32);
            let delay = CALL_SPACING_SECONDS * f64::from(*said);
            *said += 1;
            let call = calls::assignment_call(label, words)
                .after(delay)
                .because(origin);
            self.comms.send(now, call, &hearers);
        }
    }

    /// Where the aircraft `plane` flies now, feet: an AI actor's, a
    /// human-flown plane's, or a combat target's.
    fn position_of(&self, plane: u32) -> Option<[f64; 3]> {
        self.ai_wings
            .as_ref()
            .and_then(|wings| wings.mission().actor(plane))
            .map(|actor| actor.flight().position)
            .or_else(|| {
                self.cockpits
                    .iter()
                    .find(|cockpit| cockpit.plane.0 == plane)
                    .map(|cockpit| cockpit.flight.position)
            })
            .or_else(|| {
                self.combat
                    .state
                    .targets
                    .iter()
                    .find(|target| target.id == plane)
                    .map(|target| target.position)
            })
    }

    /// Hands the AI the yields a tick's new pairs of locks asked for.
    pub(super) fn apply_yields(&mut self, yields: &[(u32, u32)]) {
        if let Some(wings) = self.ai_wings.as_mut() {
            for &(actor, target) in yields {
                wings.yield_target(actor, target);
            }
        }
    }
}
