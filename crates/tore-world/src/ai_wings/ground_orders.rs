//! A flight lead's orders onto the ground (slice ST-O of the AI strike round;
//! docs/spec/ai-strike.md, "Orders from the flight lead").
//!
//! The orders are Alt+E and Alt+R with a ground designation, Alt+F (retail's
//! "engage the IR/laser target"), Alt+W (retail's "engage every target of my
//! target's class") and the new "Lase my target". The world works out, from
//! the sender's cockpit, what the order can name ([`GroundAim`]); this module
//! decides what it means ([`resolve`]) and gives it to the wingmen
//! ([`AiWings::command_ground`]). It is a child of `orders` so that it shares
//! the order journal and the radio plumbing.
use super::*;
use crate::comms::journal::WingLine;
use crate::datalink::calls::Noun;
use tore_sim::ai::engagement::GroupObjective;
use tore_sim::ai::wing::AppliedSetting;

/// How far a class order looks for the designated target's kind, from that
/// target, feet (15 nm; fitted, agent): the wingmen take the nearest first.
pub const CLASS_SCAN_FT: f64 = 15. * 6_076.;
/// How many objects a class order names at most, the nearest to the
/// designated one first (fitted, agent: a structure class can hold hundreds).
pub const CLASS_ORDER_MAX: usize = 32;
/// A sight pin names the nearest hostile ground object within this distance
/// of the pinned point, feet (fitted, agent).
pub const PIN_RADIUS_FT: f64 = 1_000.;

/// A type's class word when it is a bomber (retail: 0x4000; fighters 0x8000).
pub const BOMBER_CLASS: u16 = 0x4000;

/// One ground object an order may name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundFact {
    pub id: u32,
    /// The combat side; `NO_SIDE` objects are not offered.
    pub side: Side,
    /// The class word (`sensors::track::class`).
    pub class: u16,
    pub position: [f64; 3],
}

/// What the sender's cockpit lets an order name, read by the world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroundAim {
    /// Every standing ground object with a side, ascending id.
    pub facts: Vec<GroundFact>,
    /// The object the targeting sight tracks, or its laser spot is on.
    pub sight: Option<u32>,
    /// The ground point the sight is pinned on, when it tracks no object.
    pub pin: Option<[f64; 3]>,
}

impl GroundAim {
    fn fact(&self, id: u32, hostile_to: Side) -> Option<&GroundFact> {
        self.facts
            .iter()
            .find(|f| f.id == id && f.side != hostile_to && f.side != live::NO_SIDE)
    }

    /// The nearest hostile object within [`PIN_RADIUS_FT`] of the pin.
    fn pinned(&self, hostile_to: Side) -> Option<&GroundFact> {
        let pin = self.pin?;
        self.facts
            .iter()
            .filter(|f| f.side != hostile_to && f.side != live::NO_SIDE)
            .map(|f| (horizontal(f.position, pin), f))
            .filter(|(d, _)| *d <= PIN_RADIUS_FT)
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)))
            .map(|(_, f)| f)
    }

    /// The members of `anchor`'s class within [`CLASS_SCAN_FT`] of it, the
    /// anchor first, then the nearest, at most [`CLASS_ORDER_MAX`], ascending
    /// id.
    fn class_of(&self, anchor: &GroundFact, hostile_to: Side) -> Vec<u32> {
        let mut near: Vec<(f64, u32)> = self
            .facts
            .iter()
            .filter(|f| f.side != hostile_to && f.side != live::NO_SIDE)
            .filter(|f| f.class & anchor.class != 0)
            .map(|f| (horizontal(f.position, anchor.position), f.id))
            .filter(|(d, id)| *d <= CLASS_SCAN_FT || *id == anchor.id)
            .collect();
        near.sort_by(|a, b| {
            (a.1 != anchor.id)
                .cmp(&(b.1 != anchor.id))
                .then(a.0.total_cmp(&b.0))
                .then(a.1.cmp(&b.1))
        });
        near.truncate(CLASS_ORDER_MAX);
        let mut ids: Vec<u32> = near.into_iter().map(|(_, id)| id).collect();
        ids.sort_unstable();
        ids
    }
}

fn horizontal(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

/// What an order means for the wingmen.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Plan {
    /// An air order, as it always was: the order the wingmen receive (an
    /// Alt+F onto an aircraft is "engage my target"; an Alt+F or Alt+W onto
    /// nothing, or onto an aircraft, is attack on contact) and the aircraft
    /// it names. A refusal for no target is the air path's.
    Air {
        order: PlayerOrder,
        selected: Option<u32>,
    },
    /// Attack one ground object.
    Ground {
        target: u32,
        position: [f64; 3],
        noun: Noun,
    },
    /// Attack every object of a class, nearest first.
    Class { targets: Vec<u32>, noun: Noun },
    /// Lase one ground object.
    Lase { target: u32 },
    /// "Lase my target" and no ground object named.
    NoTarget,
}

/// Reads `order`. `selected` is the sender's designation, `hostile_air` says
/// whether an id is a living aircraft of the other side, and `side` is the
/// sender's side.
pub(super) fn resolve(
    order: PlayerOrder,
    selected: Option<u32>,
    ground: &GroundAim,
    side: Side,
    hostile_air: &dyn Fn(u32) -> bool,
) -> Plan {
    let ground_plan = |fact: &GroundFact| Plan::Ground {
        target: fact.id,
        position: fact.position,
        noun: Noun::of_class(fact.class),
    };
    // The designation first, else the sight's pin (Alt+E and Alt+R: "the
    // seat's ground designation, else the sight pin").
    let designated = || {
        selected
            .and_then(|id| ground.fact(id, side))
            .or_else(|| ground.pinned(side))
    };
    match order {
        PlayerOrder::EngageMyTarget | PlayerOrder::EngageFromFormation => {
            if selected.is_some_and(hostile_air) {
                return Plan::Air { order, selected };
            }
            match designated() {
                Some(fact) => ground_plan(fact),
                None => Plan::Air { order, selected },
            }
        }
        PlayerOrder::EngageIrLaser => {
            // The contact the sight holds: its track or laser spot, else its
            // pin. With none, the wingmen are freed (retail).
            if let Some(id) = ground.sight {
                if hostile_air(id) {
                    return Plan::Air {
                        order: PlayerOrder::EngageMyTarget,
                        selected: Some(id),
                    };
                }
                if let Some(fact) = ground.fact(id, side) {
                    return ground_plan(fact);
                }
            }
            match ground.pinned(side) {
                Some(fact) => ground_plan(fact),
                None => Plan::Air {
                    order: PlayerOrder::AttackOnContact,
                    selected: None,
                },
            }
        }
        PlayerOrder::EngageClass => {
            if selected.is_some_and(hostile_air) {
                return Plan::Air {
                    order: PlayerOrder::AttackOnContact,
                    selected: None,
                };
            }
            match designated() {
                Some(anchor) => Plan::Class {
                    targets: ground.class_of(anchor, side),
                    noun: Noun::of_class(anchor.class),
                },
                None => Plan::Air {
                    order: PlayerOrder::AttackOnContact,
                    selected: None,
                },
            }
        }
        PlayerOrder::LaseMyTarget => {
            let sighted = ground.sight.and_then(|id| ground.fact(id, side));
            match designated().or(sighted) {
                Some(fact) => Plan::Lase { target: fact.id },
                None => Plan::NoTarget,
            }
        }
        other => Plan::Air {
            order: other,
            selected,
        },
    }
}

/// What `command_linked` has worked out before it hands an order onto the
/// ground to [`AiWings::command_ground`].
pub(super) struct Call {
    pub sender: u32,
    /// The order the player gave.
    pub order: PlayerOrder,
    /// The sender's designation, as given.
    pub selected: Option<u32>,
    pub recipient: Option<u8>,
    pub plan: Plan,
    /// The addressed living wingmen that still answer, lowest member first.
    pub members: Vec<(u8, u32)>,
    /// The first wingman of the wing, addressed or not.
    pub first: Option<u32>,
    /// The human wingmen the order addressed.
    pub humans: Vec<(u8, u32)>,
    /// The answers so far (wingmen that bugged out).
    pub answers: Vec<Answer>,
    pub bugged_out: usize,
    /// The radio flight number of the sender's flight (0 is Red).
    pub flight: Option<u8>,
}

impl AiWings {
    /// The order, taken by each addressed wingman that can take it: its
    /// ground picture is given the target (or the class), it keeps its
    /// formation settings and rejoins when the order is done. A wingman with
    /// no usable surface store (or, for "Lase my target", no designator)
    /// refuses; with none of them willing the lead hears one say so.
    pub(super) fn command_ground(&mut self, call: Call) -> WorldResult<OrderReport> {
        let Call {
            sender,
            order,
            selected,
            recipient,
            plan,
            members,
            first,
            humans,
            mut answers,
            bugged_out,
            flight,
        } = call;
        let lase = matches!(plan, Plan::Lase { .. });
        let named = match &plan {
            Plan::Ground { target, .. } | Plan::Lase { target } => Some(*target),
            Plan::Class { targets, .. } => targets.first().copied(),
            Plan::Air { .. } | Plan::NoTarget => None,
        };
        let cause = Cause::Order {
            order,
            selected,
            target: named,
        };
        let (mut applied, mut rejected, mut no_stores, mut no_designators) = (0, 0, 0, 0);
        let mut reached = Vec::new();
        let mut replied = None;
        let mut refuser = None;
        for (member, id) in &members {
            let actor = self.mission.actor(*id).unwrap();
            let answer = |result, side| Answer {
                recipient: *id,
                member: *member,
                result,
                side,
            };
            if let Some(why) = actor.ground_order_refusal() {
                rejected += 1;
                answers.push(answer(
                    Answered::Receiver(ReceiverOutcome::Rejected(why)),
                    Vec::new(),
                ));
                continue;
            }
            let ability = self.capability(*id);
            if lase && !ability.designator {
                no_designators += 1;
                refuser.get_or_insert((*id, WingLine::NoDesignator));
                answers.push(answer(Answered::NoDesignator, Vec::new()));
                continue;
            }
            if !lase && !ability.strike {
                no_stores += 1;
                refuser.get_or_insert((*id, WingLine::Winchester));
                answers.push(answer(Answered::NoSurfaceStore, Vec::new()));
                continue;
            }
            // The silent control side effect, as the air orders have it.
            let (control, _, _) = actor.controller().wing_settings();
            let old = control.unwrap_or(WingControl::Loose);
            let new = wing::apply_control_side_effect(order, old).map_err(|e| e.to_string())?;
            let mut side = Vec::new();
            if new != old {
                let outcome = self
                    .mission
                    .order(*id, WingRequest::WingControl(new))
                    .unwrap()
                    .map_err(|e| e.to_string())?;
                side.push(("wing control change", outcome));
            }
            let actor = self.mission.actor_mut(*id).unwrap();
            match &plan {
                Plan::Ground { target, .. } => actor.order_strike(*target),
                Plan::Class { targets, .. } => actor.order_strike_class(targets.clone()),
                Plan::Lase { target } => actor.order_lase(*target),
                Plan::Air { .. } | Plan::NoTarget => {
                    unreachable!("an air order is not a ground call")
                }
            }
            // The lase order is flown as slice ST-L's duty: the offset orbit
            // with the spot on the target until it dies, another order or a
            // threat ends it (ST-INT). `capability` checked `can_lase`.
            if let Plan::Lase { target } = &plan {
                self.order_lase(*id, *target, tore_sim::ai::strike::LaseUntil::TargetDies);
            }
            answers.push(answer(
                Answered::Receiver(ReceiverOutcome::Applied(AppliedSetting::TargetOrder {
                    deadline: None,
                })),
                side,
            ));
            reached.push(*id);
            applied += 1;
            // The first wingman answers two seconds later, over the radio.
            if Some(*id) == first {
                replied = Some(if lase {
                    self.chat(Chatter::Line {
                        speaker: *id,
                        line: WingLine::Lasing,
                    });
                    Reply::Replied {
                        by: *id,
                        reply: WingReply::Line(WingLine::Lasing),
                    }
                } else {
                    let aircraft = self.engaged(*id, named);
                    Reply::Replied {
                        by: *id,
                        reply: WingReply::Engage { aircraft },
                    }
                });
            }
        }
        // Nobody took it: the first that refused says why.
        if applied == 0
            && let Some((speaker, line)) = refuser
        {
            self.chat(Chatter::Line { speaker, line });
            replied = Some(Reply::Replied {
                by: speaker,
                reply: WingReply::Line(line),
            });
        }
        for (member, id) in &humans {
            reached.push(*id);
            answers.push(Answer {
                recipient: *id,
                member: *member,
                result: Answered::Human,
                side: Vec::new(),
            });
        }
        // The lead's call: where the object is for an order onto one, "Attack
        // SAMs" for a class, nothing for lasing (no recording says it).
        let radio = match (&plan, members.first()) {
            (_, None) => Vec::new(),
            (Plan::Ground { position, noun, .. }, Some(&(_, near))) => {
                let from = self.mission.actor(near).map(|a| a.flight().position);
                match from {
                    Some(from) => {
                        let addressee = match recipient {
                            Some(member) => calls::Addressee::Wingman(member),
                            None => calls::Addressee::Flight(flight.unwrap_or(0)),
                        };
                        calls::assignment_stems_of(
                            &comms::Phrases::new(),
                            addressee,
                            calls::Geometry::between(from, *position),
                            *noun,
                        )
                    }
                    None => Vec::new(),
                }
            }
            (Plan::Class { noun, .. }, _) => calls::class_stems(*noun),
            _ => Vec::new(),
        };
        let refused = rejected + no_stores + no_designators;
        let message = if applied == 0 && rejected == 0 && no_stores > 0 {
            "Wing order unavailable: no usable surface store".to_owned()
        } else if applied == 0 && rejected == 0 && no_designators > 0 {
            "Wing order unavailable: no designator aboard".to_owned()
        } else {
            let mut message = format!(
                "{}: {applied} applied, {refused} rejected, 0 without motion",
                order_label(order)
            );
            if no_stores > 0 {
                message.push_str(&format!(", {no_stores} with no usable surface store"));
            }
            if no_designators > 0 {
                message.push_str(&format!(", {no_designators} with no designator"));
            }
            message
        };
        let mut message = message;
        if bugged_out > 0 {
            message.push_str(&format!(", {bugged_out} bugged out"));
        }
        if !humans.is_empty() {
            message.push_str(&format!(", {} flown by a human", humans.len()));
        }
        let report = OrderReport {
            message,
            radio,
            reached,
            target: None,
            humans: humans.iter().map(|(_, id)| *id).collect(),
            ground: true,
        };
        let reply = replied.unwrap_or_else(|| no_reply(order, first, &answers));
        self.journal_order(
            sender,
            cause,
            recipient,
            &report,
            Outcome::Answered { answers, reply },
        );
        Ok(report)
    }

    /// Gives the AI aircraft of a Quick Mission their air-to-ground roles
    /// (slice ST-O; docs/spec/ai-strike.md, "Who strikes"). A friendly wing
    /// other than the player's whose objective is Strike strikes. With the
    /// default objective (Inherit) and a ground target set, a wing of
    /// bomber-class aircraft (retail's rule: the type's class word 0x4000,
    /// where the fighters are 0x8000) strikes too, as retail sends them at
    /// the target; everything else flies CAP, the player's own wing always
    /// (its wingmen strike on orders only) and every Redfor wing (Quick
    /// Mission gives Redfor no air-to-ground objectives).
    pub fn apply_strike_roles(&mut self, objectives: &[GroupObjective; 6], ground_target: bool) {
        use tore_sim::ai::engagement::StrikeRole;
        let roles: Vec<(u32, StrikeRole)> = self
            .slots
            .iter()
            .filter(|slot| slot.side == tore_sim::ai::launch::Side::Friendly)
            .filter(|slot| slot.wing_number > 1)
            .filter_map(|slot| {
                let objective = objectives[usize::from(slot.wing_number) - 1];
                let bomber = self
                    .configs
                    .get(&slot.id)
                    .is_some_and(|c| c.target_category & BOMBER_CLASS != 0);
                let strike = match objective {
                    GroupObjective::Strike => true,
                    GroupObjective::Inherit => ground_target && bomber,
                    _ => false,
                };
                strike.then_some((slot.id, StrikeRole::Strike))
            })
            .collect();
        for (id, role) in roles {
            self.set_strike_role(id, role);
        }
    }

    /// "Lase my target" with nothing to lase: refused before any wingman
    /// took it, and the first wingman says so.
    pub(super) fn refuse_no_ground_target(
        &mut self,
        sender: u32,
        order: PlayerOrder,
        selected: Option<u32>,
        recipient: Option<u8>,
        first: Option<u32>,
    ) -> OrderReport {
        let report = OrderReport {
            message: "Wing order unavailable: no ground target designated".into(),
            ..OrderReport::default()
        };
        if let Some(speaker) = first {
            self.chat(Chatter::Line {
                speaker,
                line: WingLine::NoTarget,
            });
        }
        let cause = Cause::Order {
            order,
            selected,
            target: None,
        };
        self.journal_order(
            sender,
            cause,
            recipient,
            &report,
            Outcome::Refused(Reason::NoGroundTarget),
        );
        report
    }
}

#[cfg(test)]
#[path = "ground_orders_tests.rs"]
mod tests;
