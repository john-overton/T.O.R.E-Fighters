//! Assignments: what a lead gave each wingman to attack, and when the
//! assignment ends (slice G3a; the guide is `docs/DATALINK.md`, "Giving
//! assignments").
//!
//! [`DataLink::assign`] is the one door. A lead's order comes in with the
//! members it reached and the target it named, and the table changes:
//!
//! - Engage my target and Engage from formation write one assignment for each
//!   member reached, replacing the one it had;
//! - the orders that give a wingman something else to do (Disengage, Protect
//!   me, Attack on contact, Bug out, Land, and an Approach, which sets its own
//!   target) clear the assignments of the members reached;
//! - every other order leaves the table alone.
//!
//! The rest of the clearing rules run every tick, at the end of
//! [`DataLink::after_ai`](super::DataLink::after_ai): the receiver or the
//! target is lost, or the lead changes. A member that locks its target marks
//! the assignment acknowledged.
//!
//! Delivery is not here: the AI still gets the target through
//! `TargetOrder::ConcreteTarget` (slice G3b widens it), and the call is worded
//! by [`calls`](super::calls).

use super::{Assignment, DataLink, Entry};
use crate::ai_wings::{AiWings, ENEMY_SIDE, FRIENDLY_SIDE};
use tore_sim::ai::{launch::Side, wing::PlayerOrder};

/// Why an assignment ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearReason {
    /// The lead ordered something else of the receiver: Disengage, Protect
    /// me, Attack on contact, Bug out, Land, an Approach.
    Order,
    /// The receiver was destroyed or is gone.
    ReceiverLost,
    /// The target was destroyed or is gone.
    TargetLost,
    /// The member that gave it no longer leads the flight.
    LeadChanged,
}

impl ClearReason {
    /// The word the probe prints and the recorder keeps.
    pub fn name(self) -> &'static str {
        match self {
            ClearReason::Order => "order",
            ClearReason::ReceiverLost => "receiver lost",
            ClearReason::TargetLost => "target lost",
            ClearReason::LeadChanged => "lead changed",
        }
    }
}

impl DataLink {
    /// A lead's order reached `receivers` (the AI wingmen that took it and
    /// the human wingmen it addressed), and named `target` when it names one.
    /// Writes or clears their assignments, journals each change, and returns
    /// the assignments written, one for each receiver, in the order given.
    ///
    /// `tick` is the combat tick the order was given on. A receiver the
    /// picture knows to be dead gets nothing.
    pub fn assign(
        &mut self,
        tick: u64,
        sender: u32,
        order: PlayerOrder,
        receivers: &[u32],
        target: Option<u32>,
    ) -> Vec<Assignment> {
        // A plane the picture has not met yet (an order on the first tick) is
        // taken as alive; one it knows to be dead gets nothing.
        let live = |link: &Self, plane: u32| link.member(plane).is_none_or(|m| m.alive);
        match order {
            PlayerOrder::EngageMyTarget | PlayerOrder::EngageFromFormation => {
                let Some(target) = target else {
                    return Vec::new();
                };
                let mut written = Vec::new();
                for &receiver in receivers {
                    if receiver == sender || !live(self, receiver) {
                        continue;
                    }
                    let assignment = Assignment {
                        target,
                        by: sender,
                        tick,
                        order,
                        acknowledged: false,
                    };
                    self.assignments.insert(receiver, assignment);
                    self.journal.push(Entry::Assign {
                        tick,
                        plane: receiver,
                        target,
                        by: sender,
                        order,
                    });
                    written.push(assignment);
                }
                written
            }
            PlayerOrder::Disengage
            | PlayerOrder::ProtectMe
            | PlayerOrder::AttackOnContact
            | PlayerOrder::BugOut
            | PlayerOrder::LandAtSelected
            | PlayerOrder::Approach(_) => {
                for &receiver in receivers {
                    self.clear(tick, receiver, ClearReason::Order);
                }
                Vec::new()
            }
            PlayerOrder::Break(_)
            | PlayerOrder::Formation(_)
            | PlayerOrder::Spacing
            | PlayerOrder::Stacking
            | PlayerOrder::ControlToggle => Vec::new(),
        }
    }

    /// The assignment `plane` holds.
    pub fn assignment(&self, plane: u32) -> Option<Assignment> {
        self.assignments.get(&plane).copied()
    }

    /// Ends `plane`'s assignment, if it has one, and journals why.
    fn clear(&mut self, tick: u64, plane: u32, why: ClearReason) {
        if let Some(assignment) = self.assignments.remove(&plane) {
            self.journal.push(Entry::Clear {
                tick,
                plane,
                target: assignment.target,
                why,
            });
        }
    }

    /// The clearing rules that watch the mission, and the acknowledgement:
    /// run at the end of the AI's half of the tick, when every member's
    /// `alive` and lock are this tick's.
    pub(super) fn settle(&mut self, tick: u64, wings: Option<&AiWings>) {
        let planes: Vec<u32> = self.assignments.keys().copied().collect();
        for plane in planes {
            let Some(assignment) = self.assignments.get(&plane).copied() else {
                continue;
            };
            let alive = |link: &Self, id: u32| link.member(id).is_some_and(|m| m.alive);
            let why = if !alive(self, plane) {
                Some(ClearReason::ReceiverLost)
            } else if !alive(self, assignment.target) {
                Some(ClearReason::TargetLost)
            } else if self.lead_changed(assignment.by, plane, wings) {
                Some(ClearReason::LeadChanged)
            } else {
                None
            };
            if let Some(why) = why {
                self.clear(tick, plane, why);
                continue;
            }
            let locked = self
                .lock(plane)
                .is_some_and(|lock| lock.target == assignment.target);
            if locked && !assignment.acknowledged {
                if let Some(held) = self.assignments.get_mut(&plane) {
                    held.acknowledged = true;
                }
                self.journal.push(Entry::Acknowledge {
                    tick,
                    plane,
                    target: assignment.target,
                });
            }
        }
    }

    /// Whether `giver` no longer leads `receiver`'s flight: it is gone, or the
    /// mission names another leader. Where the mission names none (no AI
    /// wings, or before it has stepped) the giver is taken to lead on.
    fn lead_changed(&self, giver: u32, receiver: u32, wings: Option<&AiWings>) -> bool {
        if !self.member(giver).is_some_and(|m| m.alive) {
            return true;
        }
        let Some(flight) = self.member(receiver).map(|m| m.flight) else {
            return false;
        };
        let side = match flight.side {
            Side::Friendly => FRIENDLY_SIDE,
            Side::Enemy => ENEMY_SIDE,
        };
        wings
            .and_then(|wings| wings.mission().wing_leader(side, flight.index))
            .is_some_and(|leader| leader != giver)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datalink::{Lock, Member};
    use tore_sim::ai::{
        launch::WingId,
        wing::{Formation, PlayerApproach, PlayerBreak},
    };

    const FLIGHT: WingId = WingId {
        side: Side::Friendly,
        index: 0,
    };
    const ENEMY: WingId = WingId {
        side: Side::Enemy,
        index: 0,
    };

    fn member(plane: u32, flight: WingId, alive: bool) -> Member {
        Member {
            plane,
            flight,
            member: plane as u8,
            aircraft: None,
            radar: true,
            human: plane == 0,
            alive,
            position: [0.; 3],
        }
    }

    /// Lead 0, wingmen 1 and 2, and enemy aircraft 10 and 11.
    fn link() -> DataLink {
        DataLink {
            members: vec![
                member(0, FLIGHT, true),
                member(1, FLIGHT, true),
                member(2, FLIGHT, true),
                member(10, ENEMY, true),
                member(11, ENEMY, true),
            ],
            ..DataLink::default()
        }
    }

    fn engage(link: &mut DataLink, tick: u64, receivers: &[u32], target: u32) -> Vec<Assignment> {
        link.assign(
            tick,
            0,
            PlayerOrder::EngageMyTarget,
            receivers,
            Some(target),
        )
    }

    #[test]
    fn engage_orders_write_one_assignment_for_each_receiver() {
        for order in [
            PlayerOrder::EngageMyTarget,
            PlayerOrder::EngageFromFormation,
        ] {
            let mut link = link();
            let written = link.assign(40, 0, order, &[1, 2], Some(10));
            assert_eq!(written.len(), 2);
            for (plane, assignment) in [(1, written[0]), (2, written[1])] {
                assert_eq!(link.assignment(plane), Some(assignment));
                assert_eq!(assignment.target, 10);
                assert_eq!(assignment.by, 0);
                assert_eq!(assignment.tick, 40);
                assert_eq!(assignment.order, order);
                assert!(!assignment.acknowledged);
            }
            assert_eq!(link.assignments().len(), 2);
            let journal = link.take_journal();
            assert_eq!(journal.len(), 2);
            assert!(matches!(
                journal[0],
                Entry::Assign {
                    tick: 40,
                    plane: 1,
                    target: 10,
                    by: 0,
                    ..
                }
            ));
        }
    }

    #[test]
    fn an_engage_order_without_a_target_or_for_a_dead_receiver_writes_nothing() {
        let mut link = link();
        assert!(
            link.assign(1, 0, PlayerOrder::EngageMyTarget, &[1], None)
                .is_empty()
        );
        link.members[2].alive = false;
        let written = engage(&mut link, 2, &[1, 2], 10);
        assert_eq!(written.len(), 1, "the dead wingman gets none");
        assert!(link.assignment(2).is_none());
        // A plane the picture has not met yet is taken as alive.
        let written = engage(&mut link, 3, &[99], 10);
        assert_eq!(written.len(), 1);
    }

    #[test]
    fn a_new_target_order_replaces_the_assignment() {
        let mut link = link();
        engage(&mut link, 1, &[1], 10);
        engage(&mut link, 5, &[1], 11);
        let held = link.assignment(1).unwrap();
        assert_eq!((held.target, held.tick), (11, 5));
        assert_eq!(link.assignments().len(), 1);
    }

    #[test]
    fn each_order_that_gives_the_wingman_something_else_clears_it() {
        for order in [
            PlayerOrder::Disengage,
            PlayerOrder::ProtectMe,
            PlayerOrder::AttackOnContact,
            PlayerOrder::BugOut,
            PlayerOrder::LandAtSelected,
            PlayerOrder::Approach(PlayerApproach::Left),
        ] {
            let mut link = link();
            engage(&mut link, 1, &[1, 2], 10);
            link.take_journal();
            // Only the member the order reached loses its assignment.
            link.assign(9, 0, order, &[1], Some(10));
            assert!(link.assignment(1).is_none(), "{order:?}");
            assert!(link.assignment(2).is_some(), "{order:?}");
            assert_eq!(
                link.take_journal(),
                [Entry::Clear {
                    tick: 9,
                    plane: 1,
                    target: 10,
                    why: ClearReason::Order
                }],
                "{order:?}"
            );
        }
    }

    #[test]
    fn the_other_orders_leave_the_table_alone() {
        for order in [
            PlayerOrder::Break(PlayerBreak::Left),
            PlayerOrder::Formation(Formation::Echelon),
            PlayerOrder::Spacing,
            PlayerOrder::Stacking,
            PlayerOrder::ControlToggle,
        ] {
            let mut link = link();
            engage(&mut link, 1, &[1, 2], 10);
            link.take_journal();
            assert!(link.assign(9, 0, order, &[1, 2], None).is_empty());
            assert_eq!(link.assignments().len(), 2, "{order:?}");
            assert!(link.take_journal().is_empty(), "{order:?}");
        }
        // Clearing a wingman with nothing assigned says nothing.
        let mut link = link();
        link.assign(9, 0, PlayerOrder::Disengage, &[1], None);
        assert!(link.take_journal().is_empty());
    }

    #[test]
    fn a_lost_receiver_or_target_ends_the_assignment() {
        let mut link = link();
        engage(&mut link, 1, &[1, 2], 10);
        link.take_journal();
        link.members[1].alive = false;
        link.settle(7, None);
        assert!(link.assignment(1).is_none());
        assert!(link.assignment(2).is_some());
        assert_eq!(
            link.take_journal(),
            [Entry::Clear {
                tick: 7,
                plane: 1,
                target: 10,
                why: ClearReason::ReceiverLost
            }]
        );
        link.members[3].alive = false;
        link.settle(8, None);
        assert!(link.assignments().is_empty());
        assert_eq!(
            link.take_journal(),
            [Entry::Clear {
                tick: 8,
                plane: 2,
                target: 10,
                why: ClearReason::TargetLost
            }]
        );
    }

    #[test]
    fn a_lead_that_is_gone_ends_its_assignments() {
        let mut link = link();
        engage(&mut link, 1, &[1, 2], 10);
        link.take_journal();
        link.settle(3, None);
        assert_eq!(link.assignments().len(), 2, "the lead leads on");
        link.members[0].alive = false;
        link.settle(4, None);
        assert!(link.assignments().is_empty());
        let why: Vec<_> = link
            .take_journal()
            .into_iter()
            .map(|e| match e {
                Entry::Clear { why, .. } => why,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(why, [ClearReason::LeadChanged; 2]);
    }

    #[test]
    fn locking_the_target_acknowledges_once() {
        let mut link = link();
        engage(&mut link, 1, &[1, 2], 10);
        link.take_journal();
        // A lock on something else is not an acknowledgement.
        link.locks.insert(
            1,
            Lock {
                target: 11,
                since: 2,
            },
        );
        link.settle(2, None);
        assert!(!link.assignment(1).unwrap().acknowledged);
        link.locks.insert(
            1,
            Lock {
                target: 10,
                since: 3,
            },
        );
        link.settle(3, None);
        assert!(link.assignment(1).unwrap().acknowledged);
        assert!(!link.assignment(2).unwrap().acknowledged);
        link.settle(4, None);
        assert_eq!(
            link.take_journal(),
            [Entry::Acknowledge {
                tick: 3,
                plane: 1,
                target: 10
            }],
            "once"
        );
        // A fresh assignment starts unacknowledged again.
        engage(&mut link, 6, &[1], 10);
        assert!(!link.assignment(1).unwrap().acknowledged);
    }
}
