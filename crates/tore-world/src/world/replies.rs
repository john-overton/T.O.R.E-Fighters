//! A human wingman's replies and requests to its flight, and the order call a
//! human lead makes to its human wingmen. Stage F phase 2, slice F2-R; see
//! docs/ARCHITECTURE.md, "Orders to human wingmen, and their replies".
//!
//! The four kinds of [`crate::seats::SeatCommand::WingReply`] are a call from
//! the seat's plane to its flight: every human of the flight hears it as a
//! radio line ("Red two: Winchester"), the speaker hears its own as `YOU`, and
//! the AI does nothing with it. A plane that leads its flight has no one to
//! answer. A human lead's Alt-key orders, which the AI wingmen act on, reach
//! the human wingmen the order addressed as the lead's own call, with the
//! lead's own recording and text, on each wingman's channel.

use std::collections::BTreeSet;

use super::*;
use crate::{
    ai_wings::{Member, OrderReport},
    comms::{
        Hearer, Kind, Phrase, Phrases, Route,
        journal::{Audience, Cause, Entry, Origin, Outcome, Reason, Source},
    },
    datalink::calls::{self, Addressee, Geometry},
    radio_calls::{self, Listener, Scene},
};
use tore_sim::ai::wing::PlayerOrder;

/// Seconds a seat waits between two reply calls, so a held key cannot flood
/// the flight's radio (agent decision).
pub const REPLY_GAP_S: f64 = 2.;
/// The cooldown key of [`REPLY_GAP_S`], a rule of the radio's.
const REPLY_KEY: &str = radio_calls::REPLY;

/// What a wingman tells its flight. The wire codes it in 2 bits, in this
/// order (docs/formats/net-protocol.md, "Changed messages").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reply {
    /// "Engaging" (Alt+Shift+E, retail recording `^ENGAGE`).
    Engaging,
    /// "Winchester": out of weapons (Alt+Shift+W, text only: no retail
    /// recording says it).
    Winchester,
    /// "Bingo fuel" (Alt+Shift+B, `^BINGO`).
    BingoFuel,
    /// "Need help" (Alt+Shift+H, the retail recording whose phrase asks for
    /// help).
    NeedHelp,
}

impl Reply {
    /// Every kind, in wire order.
    pub const ALL: [Reply; 4] = [
        Reply::Engaging,
        Reply::Winchester,
        Reply::BingoFuel,
        Reply::NeedHelp,
    ];

    /// The line a flight's humans read, after the speaker's place
    /// ("Two: Winchester").
    pub fn text(self) -> &'static str {
        match self {
            Reply::Engaging => "Engaging",
            Reply::Winchester => "Winchester",
            Reply::BingoFuel => "Bingo fuel",
            Reply::NeedHelp => "Need help",
        }
    }

    /// The retail recording that says it, if there is one. Winchester has
    /// none, so it is text only. Need help is `^OFFME`, "Get this guy off
    /// me", the recording that asks for help; `^CLRMY6`, "Clear my six", is
    /// the lead's Protect me order and is not a wingman's to say (agent
    /// decision).
    pub fn stem(self) -> Option<&'static str> {
        match self {
            Reply::Engaging => Some("^ENGAGE"),
            Reply::Winchester => None,
            Reply::BingoFuel => Some("^BINGO"),
            Reply::NeedHelp => Some("^OFFME"),
        }
    }

    /// The call's words: the recording's imported text with the recording, or
    /// [`Self::text`] where the text was not imported, so the line is never
    /// empty.
    pub fn phrase(self, phrases: &Phrases) -> Phrase {
        let Some(stem) = self.stem() else {
            return Phrase::default().raw(self.text(), None);
        };
        let mut phrase = Phrase::stem(phrases, stem);
        if phrase.text.is_empty() {
            phrase.text = self.text().to_owned();
        }
        phrase
    }
}

impl World {
    /// The radio's view of the mission as [`World::step_radio`] builds it: the
    /// planes with a radio name, the human-flown ones as listeners, and the
    /// leader of each flight.
    pub(super) fn radio_parties(&self) -> (Vec<Member>, Vec<Listener>, Vec<(u8, u32)>) {
        let members = radio_calls::members(&self.roster, self.ai_wings.as_ref(), |plane| {
            self.cockpits
                .iter()
                .position(|cockpit| cockpit.plane == plane)
                .is_some_and(|cockpit| self.cockpit_alive(cockpit))
        });
        let listeners = self
            .cockpits
            .iter()
            .enumerate()
            .filter_map(|(index, cockpit)| {
                let seat = self.roster.seat(self.roster.seat_of(cockpit.plane)?)?;
                let member = members.iter().find(|m| m.id == cockpit.plane.0)?;
                Some(Listener {
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
        (members, listeners, leaders)
    }

    /// Alt+Shift+E, W, B or H from the seat `seat` flying `plane` in
    /// `cockpit`: the call to the flight, or a line saying why not. The plane
    /// that leads its flight has no one to answer ("You lead this flight."),
    /// a plane that is down says nothing, and a seat may call once in
    /// [`REPLY_GAP_S`]. Radio silence drops the call for each seat that has it
    /// on, as any routine chatter; a speaker under silence is told the call
    /// went out.
    pub(super) fn wing_reply(
        &mut self,
        plane: PlaneId,
        seat: SeatId,
        cockpit: usize,
        reply: Reply,
        out: &mut TickOutput,
    ) {
        let now = self.combat.state.tick() as f64 / 120.;
        let pilot = &self.cockpits[cockpit].flight.systems.pilot;
        if !self.cockpit_alive(cockpit) || pilot.dead || pilot.ejected {
            let why = Refusal {
                reason: Reason::AircraftLost,
                line: "No reply: your aircraft is down",
            };
            self.reply_refused(now, plane, seat, reply, why, out);
            return;
        }
        let (members, listeners, leaders) = self.radio_parties();
        let flight = members.iter().find(|m| m.id == plane.0).map(|m| m.flight);
        // A plane the radio does not name has no flight to call.
        let leads =
            flight.is_none_or(|flight| leaders.iter().any(|&(f, p)| f == flight && p == plane.0));
        if leads {
            let why = Refusal {
                reason: Reason::Text("the speaker leads its flight".into()),
                line: "You lead this flight.",
            };
            self.reply_refused(now, plane, seat, reply, why, out);
            return;
        }
        if !self.comms.seat_cooldown(seat, REPLY_KEY, now, REPLY_GAP_S) {
            let why = Refusal {
                reason: Reason::Cooldown {
                    key: REPLY_KEY,
                    seconds: REPLY_GAP_S,
                    remaining: self.comms.seat_remaining(seat, REPLY_KEY, now),
                },
                line: "Reply: wait a moment",
            };
            self.reply_refused(now, plane, seat, reply, why, out);
            return;
        }
        let friendlies = BTreeSet::new();
        let scene = Scene {
            now,
            phrases: &self.phrases,
            listeners: &listeners,
            members: &members,
            leaders: &leaders,
            targets: &self.combat.state.targets,
            friendlies: &friendlies,
        };
        self.radio.reply(&mut self.comms, &scene, plane.0, reply);
        if self.comms.radio_silence(seat) {
            out.cues.push(Cue::Message {
                seat,
                text: format!("{}: sent (radio silence is on)", reply.text()),
            });
        }
    }

    /// A reply that was not made: the line the pilot reads and the journal's
    /// note of why.
    fn reply_refused(
        &mut self,
        now: f64,
        plane: PlaneId,
        seat: SeatId,
        reply: Reply,
        why: Refusal,
        out: &mut TickOutput,
    ) {
        self.comms.record(
            Entry::note(
                now,
                "YOU",
                Origin::of(Source::Reply, Cause::Replied(reply))
                    .by(plane.0)
                    .to(Audience::Flight),
                Outcome::Refused(why.reason),
            )
            .with_text(reply.text())
            .with_kind(Route::Radio, Kind::Chatter),
        );
        out.cues.push(Cue::Message {
            seat,
            text: why.line.into(),
        });
    }

    /// The lead's order as a human wingman hears it (slice F2-R): one call, in
    /// the lead's label and with its own recordings and text, on the channel of
    /// every living human wingman the order addressed, and on nobody else's.
    /// An attack order is worded from each wingman's own place, as the AI
    /// wingman's call is ([`calls::hearers`]); "Attack bandits" and the
    /// formation, break and other orders are the same words for all, and the
    /// ones with no recording (bug out, land) are text only. An order is never
    /// silenced.
    pub(super) fn call_human_wingmen(
        &mut self,
        plane: PlaneId,
        order: PlayerOrder,
        recipient: Option<u8>,
        report: &OrderReport,
    ) {
        if report.humans.is_empty() {
            return;
        }
        let now = self.combat.state.tick() as f64 / 120.;
        let (members, listeners, _) = self.radio_parties();
        let Some(sender) = members.iter().find(|m| m.id == plane.0) else {
            return;
        };
        let label = radio_calls::label(sender);
        let heard: Vec<&Listener> = listeners
            .iter()
            .filter(|l| l.alive && report.humans.contains(&l.plane))
            .collect();
        if heard.is_empty() {
            return;
        }
        let hearers = match (attack_by_name(order), self.target_place(report.target)) {
            (true, Some(at)) => {
                let listeners: Vec<calls::Listener> = heard
                    .iter()
                    .map(|l| calls::Listener {
                        seat: l.seat,
                        label: label.clone(),
                        position: l.position,
                    })
                    .collect();
                calls::hearers(
                    &self.phrases,
                    addressee(recipient, sender.flight),
                    at,
                    &listeners,
                )
            }
            _ => {
                let words = self.order_words(order, report);
                heard
                    .iter()
                    .map(|l| Hearer::named(l.seat, label.clone()).saying(words.clone()))
                    .collect()
            }
        };
        let Some(first) = hearers.first().and_then(|h| h.words.clone()) else {
            return;
        };
        let cause = Cause::Order {
            order,
            selected: report.target,
            target: report.target,
        };
        let call = calls::assignment_call(label, first).because(
            Origin::of(Source::Order, cause)
                .by(plane.0)
                .to(Audience::Wing { member: recipient }),
        );
        self.comms.send(now, call, &hearers);
    }

    /// The lead's voice for an attack order that only human wingmen took: the
    /// call as the first of them hears it, since no AI wingman's place can
    /// word it. Empty when the order is not an attack by name.
    pub(super) fn human_attack_stems(
        &self,
        plane: PlaneId,
        order: PlayerOrder,
        recipient: Option<u8>,
        report: &OrderReport,
    ) -> Vec<&'static str> {
        if !attack_by_name(order) || report.humans.is_empty() {
            return Vec::new();
        }
        let (members, listeners, _) = self.radio_parties();
        let (Some(sender), Some(at)) = (
            members.iter().find(|m| m.id == plane.0),
            self.target_place(report.target),
        ) else {
            return Vec::new();
        };
        // The report lists the human wingmen lowest member first.
        let Some(first) = report
            .humans
            .iter()
            .find_map(|id| listeners.iter().find(|l| l.plane == *id))
        else {
            return Vec::new();
        };
        calls::assignment_stems(
            &Phrases::new(),
            addressee(recipient, sender.flight),
            Geometry::between(first.position, at),
        )
    }

    /// Where the aircraft an attack order named is now.
    fn target_place(&self, target: Option<u32>) -> Option<[f64; 3]> {
        let target = target?;
        self.ai_wings
            .as_ref()?
            .mission()
            .actor(target)
            .map(|actor| actor.flight().position)
    }

    /// The words of an order that is the same for every human wingman: the
    /// order's recordings with their text, else its name.
    fn order_words(&self, order: PlayerOrder, report: &OrderReport) -> Phrase {
        if order == PlayerOrder::AttackOnContact {
            return calls::blanket_phrase();
        }
        let mut phrase = if attack_by_name(order) {
            Phrase::default()
        } else {
            report.radio.iter().fold(Phrase::default(), |phrase, stem| {
                phrase.then(&self.phrases, stem)
            })
        };
        if phrase.text.is_empty() {
            phrase.text = crate::ai_wings::order_label(order).to_owned();
        }
        phrase
    }
}

/// Why a reply was not made: the journal's reason and the pilot's line.
struct Refusal {
    reason: Reason,
    line: &'static str,
}

/// Whether `order` names a target by place, which each hearer words for itself.
fn attack_by_name(order: PlayerOrder) -> bool {
    matches!(
        order,
        PlayerOrder::EngageMyTarget | PlayerOrder::EngageFromFormation
    )
}

/// Who an order is addressed to: the wingman it names, or the whole flight.
fn addressee(recipient: Option<u8>, flight: u8) -> Addressee {
    recipient.map_or(Addressee::Flight(flight), Addressee::Wingman)
}
