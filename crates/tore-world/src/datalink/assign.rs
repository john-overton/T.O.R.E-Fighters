//! Assignments: what a lead gave each wingman to attack, and when the
//! assignment ends (slice G3a; the guide is `docs/DATALINK.md`, "Giving
//! assignments").
//!
//! [`DataLink::assign`] is the one door. A lead's order comes in with the
//! members it reached and the target it named, and the table changes:
//!
//! - Engage my target and Engage from formation write one assignment for each
//!   member reached, replacing the one it had;
//! - Sort writes the same assignment for the one wingman it gives a bandit
//!   to, with the order recorded as the sort ([`DataLink::sort_plan`] decides
//!   who gets which bandit);
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
//! `TargetOrder::ConcreteTarget`, and the call is worded by
//! [`calls`](super::calls). Slice G3b adds the picture's side of delivery: a
//! wingman that cannot see its assigned aircraft is given the freshest track
//! a flightmate reports of it ([`DataLink::pursuits`]), and an order is taken
//! when the picture holds a track of its target ([`DataLink::tracked`]).

use super::{Assignment, Damage, DataLink, Entry, Fuel, MemberStatus, Track, Weapons};
use crate::ai_wings::{AiWings, ENEMY_SIDE, FRIENDLY_SIDE};
use std::collections::BTreeMap;
use tore_sim::{
    ai::{
        launch::Side,
        link::{MemberState, Pursuit, SideBandits},
        wing::PlayerOrder,
    },
    datalink::sort::{self, Bandit, Wingman},
};

/// Ticks in a second, to carry a track forward.
const TICKS_PER_SECOND: f64 = 120.;

/// One wingman's share of a sort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortPick {
    pub plane: u32,
    /// Its place in the flight from zero.
    pub member: u8,
    pub target: u32,
}

/// What a sort decided: who attacks which bandit, who was left out for their
/// state and who had no bandit left to take. Planes, in member order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SortPlan {
    pub given: Vec<SortPick>,
    pub skipped: Vec<u32>,
    pub left: Vec<u32>,
}

/// A target an AI lead gave one of its wingmen (slice G4), the order being
/// the share ([`PlayerOrder::EngageMyTarget`]) or a sort
/// ([`PlayerOrder::Sort`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeadAssignment {
    pub lead: u32,
    pub receiver: u32,
    pub target: u32,
    pub order: PlayerOrder,
}

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
            PlayerOrder::EngageMyTarget | PlayerOrder::EngageFromFormation | PlayerOrder::Sort => {
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

    /// Every hostile aircraft the pictures of `side`'s flights hold, one for
    /// each (the freshest report, as [`Self::track_on`] picks it), in target
    /// id order.
    pub fn side_tracks(&self, side: Side) -> Vec<Track> {
        let mut best: BTreeMap<u32, Track> = BTreeMap::new();
        for picture in self.pictures.iter().filter(|p| p.flight.side == side) {
            for track in &picture.tracks {
                if best
                    .get(&track.target)
                    .is_none_or(|old| track.observed > old.observed)
                {
                    best.insert(track.target, *track);
                }
            }
        }
        best.into_values().collect()
    }

    /// What a sort by `sender` hands out (slice G3c): the bandits the side's
    /// pictures hold, carried forward to the tick of the last observation at
    /// their own velocity and without any known to be dead, given to the
    /// sender's living flightmates (the one `recipient` names, from zero, or
    /// every one) by [`tore_sim::datalink::sort`]. `lead_target` is the
    /// aircraft the sender keeps, which is left out. Nothing is written.
    /// `None` when the picture does not know the sender.
    pub fn sort_plan(
        &self,
        sender: u32,
        recipient: Option<u8>,
        lead_target: Option<u32>,
    ) -> Option<SortPlan> {
        let lead = self.member(sender)?;
        let known = self.known_bandits(lead.flight.side);
        let wingmen: Vec<Wingman> = self
            .members
            .iter()
            .filter(|m| {
                m.flight == lead.flight
                    && m.alive
                    && m.plane != sender
                    && recipient.is_none_or(|wanted| m.member == wanted)
            })
            .map(|m| {
                let status = self.status_of(m.plane);
                Wingman {
                    id: m.plane,
                    member: m.member,
                    position: m.position,
                    winchester: status.is_some_and(|s| s.weapons == Weapons::Winchester),
                    bingo: status.is_some_and(|s| s.fuel >= Fuel::Bingo),
                    heavy_damage: status.is_some_and(|s| s.damage == Damage::Heavy),
                }
            })
            .collect();
        let sorted = sort::sort(lead.position, lead_target, &known, &wingmen);
        let member_of = |plane: u32| self.member(plane).map_or(0, |m| m.member);
        Some(SortPlan {
            given: sorted
                .given
                .into_iter()
                .map(|(plane, target)| SortPick {
                    plane,
                    member: member_of(plane),
                    target,
                })
                .collect(),
            skipped: sorted.skipped,
            left: sorted.left,
        })
    }

    /// The hostile aircraft `side`'s pictures hold, each carried forward at
    /// its own velocity to the tick of the last observation and without any
    /// known to be dead, in target id order: what a sort deals out.
    pub fn known_bandits(&self, side: Side) -> Vec<Bandit> {
        self.side_tracks(side)
            .into_iter()
            .filter(|track| self.member(track.target).is_none_or(|m| m.alive))
            .map(|track| {
                let age = self.tick.saturating_sub(track.observed) as f64 / TICKS_PER_SECOND;
                Bandit {
                    id: track.target,
                    position: std::array::from_fn(|axis| {
                        track.position[axis] + track.velocity[axis] * age
                    }),
                }
            })
            .collect()
    }

    /// What the AI leads read of the picture (slice G4): each side's known
    /// bandits and every member's last published state.
    pub fn lead_input(&self) -> (Vec<SideBandits>, Vec<MemberState>) {
        let bandits = [(Side::Friendly, FRIENDLY_SIDE), (Side::Enemy, ENEMY_SIDE)]
            .into_iter()
            .map(|(side, ai_side)| SideBandits {
                side: ai_side,
                bandits: self.known_bandits(side),
            })
            .filter(|known| !known.bandits.is_empty())
            .collect();
        let states = self
            .pictures
            .iter()
            .flat_map(|picture| picture.status.iter())
            .map(|status| MemberState {
                plane: status.plane,
                winchester: status.weapons == Weapons::Winchester,
                bingo: status.fuel >= Fuel::Bingo,
                heavy_damage: status.damage == Damage::Heavy,
            })
            .collect();
        (bandits, states)
    }

    /// The state `plane` last published to its flight's picture.
    fn status_of(&self, plane: u32) -> Option<MemberStatus> {
        let flight = self.member(plane)?.flight;
        self.picture(flight)?
            .status
            .iter()
            .find(|status| status.plane == plane)
            .copied()
    }

    /// The assignment `plane` holds.
    pub fn assignment(&self, plane: u32) -> Option<Assignment> {
        self.assignments.get(&plane).copied()
    }

    /// The freshest track of `target` in the published pictures of `side`'s
    /// flights: the latest observation wins, and a tie keeps the flight that
    /// comes first (friendly flights by wing number).
    pub fn track_on(&self, side: Side, target: u32) -> Option<Track> {
        let mut best: Option<Track> = None;
        for picture in self.pictures.iter().filter(|p| p.flight.side == side) {
            for track in picture.tracks.iter().filter(|t| t.target == target) {
                if best.is_none_or(|old| track.observed > old.observed) {
                    best = Some(*track);
                }
            }
        }
        best
    }

    /// Whether the pictures of the side of the flight `sender` flies in hold
    /// a track of `target`: a flightmate reports the aircraft, so a wingman
    /// that cannot see it may still be sent after it (slice G3b).
    pub fn tracked(&self, sender: u32, target: u32) -> bool {
        self.member(sender)
            .is_some_and(|m| self.track_on(m.flight.side, target).is_some())
    }

    /// What each assigned AI wingman flies toward while its own sensors do
    /// not hold its assigned aircraft: the freshest track of the aircraft in
    /// the pictures of the wingman's side, in receiver order. An assignment
    /// the picture holds no track for gives its wingman nothing to fly by.
    pub fn pursuits(&self) -> Vec<Pursuit> {
        self.assignments
            .iter()
            .filter_map(|(&receiver, assignment)| {
                let member = self.member(receiver).filter(|m| m.alive && !m.human)?;
                let track = self.track_on(member.flight.side, assignment.target)?;
                Some(Pursuit {
                    receiver,
                    target: assignment.target,
                    position: track.position,
                    velocity: track.velocity,
                    observed: track.observed,
                })
            })
            .collect()
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

    // Slice G3c: the sort.

    const NM: f64 = tore_sim::sensors::FEET_PER_NAUTICAL_MILE;

    /// Lead 0 at the origin, wingmen 1 and 2 (member 1 and 2) 3 nm either side
    /// of it, and three enemy aircraft the friendly picture tracks: 10 to the
    /// east, 11 to the west, 12 far to the north.
    fn sorting() -> DataLink {
        let mut link = link();
        link.members.push(member(12, ENEMY, true));
        link.members[1].position = [3. * NM, 0., 0.];
        link.members[2].position = [-3. * NM, 0., 0.];
        let tracks = vec![
            track(0, 10, 90, [15. * NM, 0., 10. * NM]),
            track(0, 11, 90, [-15. * NM, 0., 10. * NM]),
            track(0, 12, 90, [0., 0., 30. * NM]),
        ];
        link.pictures = vec![picture(FLIGHT, tracks)];
        link.tick = 90;
        link
    }

    fn picks(plan: &SortPlan) -> Vec<(u32, u32)> {
        plan.given.iter().map(|p| (p.plane, p.target)).collect()
    }

    #[test]
    fn a_sort_gives_each_wingman_the_bandit_nearest_it_in_member_order() {
        let link = sorting();
        let plan = link.sort_plan(0, None, None).unwrap();
        // Wingman 1 is east of the lead and takes the eastern bandit; wingman
        // 2 takes the western one.
        assert_eq!(picks(&plan), [(1, 10), (2, 11)]);
        assert_eq!(plan.given[0].member, 1);
        assert!(plan.skipped.is_empty() && plan.left.is_empty());
    }

    #[test]
    fn the_lead_keeps_its_designation_and_one_wingman_can_be_addressed() {
        let link = sorting();
        let plan = link.sort_plan(0, None, Some(10)).unwrap();
        assert_eq!(picks(&plan), [(1, 11), (2, 12)]);
        let plan = link.sort_plan(0, Some(2), None).unwrap();
        assert_eq!(picks(&plan), [(2, 11)]);
        // The picture does not know a plane that is not a member.
        assert!(link.sort_plan(99, None, None).is_none());
    }

    #[test]
    fn wingmen_known_to_be_spent_low_or_hurt_are_skipped_and_the_dead_are_not_dealt() {
        let mut link = sorting();
        let status = |plane, weapons, fuel, damage| MemberStatus {
            plane,
            weapons,
            fuel,
            damage,
        };
        link.pictures[0].status = vec![
            status(1, Weapons::Winchester, Fuel::Normal, Damage::None),
            status(2, Weapons::Missiles, Fuel::Normal, Damage::None),
        ];
        let plan = link.sort_plan(0, None, None).unwrap();
        assert_eq!(plan.skipped, [1]);
        assert_eq!(picks(&plan), [(2, 11)]);
        for (fuel, damage) in [(Fuel::Bingo, Damage::None), (Fuel::Normal, Damage::Heavy)] {
            link.pictures[0].status[0] = status(1, Weapons::Missiles, fuel, damage);
            assert_eq!(link.sort_plan(0, None, None).unwrap().skipped, [1]);
        }
        // Joker fuel and light damage are fit to fight.
        link.pictures[0].status[0] = status(1, Weapons::GunsOnly, Fuel::Joker, Damage::Light);
        assert!(link.sort_plan(0, None, None).unwrap().skipped.is_empty());
        // A bandit that has died since the picture was published is not dealt,
        // and a dead wingman gets nothing.
        link.members[3].alive = false;
        link.members[2].alive = false;
        let plan = link.sort_plan(0, None, None).unwrap();
        assert_eq!(picks(&plan), [(1, 11)]);
    }

    #[test]
    fn a_track_is_carried_forward_at_its_velocity_to_the_tick_of_the_sort() {
        let mut link = sorting();
        // Bandit 10 at 39 nm out, flying east at 1,000 ft/s: twenty seconds
        // later it is more than 40 nm out and is left alone.
        link.pictures[0].tracks[0] = Track {
            velocity: [1_000., 0., 0.],
            ..track(0, 10, 90, [30. * NM, 0., 25. * NM])
        };
        link.pictures[0].tracks.truncate(1);
        link.tick = 90;
        assert_eq!(picks(&link.sort_plan(0, None, None).unwrap()).len(), 2);
        link.tick = 90 + 120 * 20;
        assert!(link.sort_plan(0, None, None).unwrap().given.is_empty());
    }

    #[test]
    fn a_sort_writes_one_assignment_for_each_wingman_that_names_the_sort() {
        let mut link = sorting();
        let plan = link.sort_plan(0, None, None).unwrap();
        for pick in &plan.given {
            let written = link.assign(95, 0, PlayerOrder::Sort, &[pick.plane], Some(pick.target));
            assert_eq!(written.len(), 1);
            assert_eq!(written[0].order, PlayerOrder::Sort);
        }
        assert_eq!(link.assignment(1).unwrap().target, 10);
        assert_eq!(link.assignment(2).unwrap().target, 11);
        assert_eq!(link.take_journal().len(), 2);
        // A later Engage replaces a sort's assignment, and a Disengage clears it.
        engage(&mut link, 99, &[1], 12);
        assert_eq!(
            link.assignment(1).unwrap().order,
            PlayerOrder::EngageMyTarget
        );
        link.assign(100, 0, PlayerOrder::Disengage, &[2], None);
        assert!(link.assignment(2).is_none());
    }

    // Slice G4: what the AI leads read of the picture.

    #[test]
    fn the_leads_input_holds_each_sides_known_bandits_and_every_members_state() {
        let mut link = sorting();
        link.pictures[0].status = vec![
            MemberStatus {
                plane: 1,
                weapons: Weapons::Winchester,
                fuel: Fuel::Normal,
                damage: Damage::None,
            },
            MemberStatus {
                plane: 2,
                weapons: Weapons::Missiles,
                fuel: Fuel::Bingo,
                damage: Damage::Heavy,
            },
        ];
        link.pictures
            .push(picture(ENEMY, vec![track(10, 1, 90, [7. * NM, 0., 0.])]));
        let (bandits, states) = link.lead_input();
        // Friendly aircraft 10, 11 and 12 are the friendly side's bandits; the
        // enemy side knows friendly plane 1.
        assert_eq!(bandits.len(), 2);
        assert_eq!(
            bandits[0].bandits.iter().map(|b| b.id).collect::<Vec<_>>(),
            [10, 11, 12]
        );
        assert_eq!(bandits[0].side, crate::ai_wings::FRIENDLY_SIDE);
        assert_eq!(bandits[1].side, crate::ai_wings::ENEMY_SIDE);
        assert_eq!(bandits[1].bandits[0].id, 1);
        assert_eq!(states.len(), 2);
        assert!(states[0].winchester && !states[0].bingo && !states[0].heavy_damage);
        assert!(!states[1].winchester && states[1].bingo && states[1].heavy_damage);
        assert!(states[0].skipped() && states[1].skipped());
        // A side that knows nothing has no row, and the dead are not bandits.
        link.members[3].alive = false;
        let (bandits, _) = link.lead_input();
        assert_eq!(
            bandits[0].bandits.iter().map(|b| b.id).collect::<Vec<_>>(),
            [11, 12]
        );
        link.pictures.truncate(1);
        assert_eq!(link.lead_input().0.len(), 1);
    }

    #[test]
    fn the_known_bandits_are_the_same_the_sort_deals_from() {
        let link = sorting();
        let known = link.known_bandits(Side::Friendly);
        assert_eq!(known.len(), 3);
        // Carried forward at 10 ft/s east to the tick of the picture.
        assert_eq!(known[0].position, [15. * NM, 0., 10. * NM]);
        assert!(link.known_bandits(Side::Enemy).is_empty());
    }

    // Slice G3b: the picture's side of an assignment reaching the AI.

    fn track(reporter: u32, target: u32, observed: u64, position: [f64; 3]) -> Track {
        Track {
            reporter,
            target,
            position,
            velocity: [10., 0., 0.],
            channel: tore_sim::sensors::Channel::Radar,
            observed,
        }
    }

    fn picture(flight: WingId, tracks: Vec<Track>) -> crate::datalink::FlightPicture {
        crate::datalink::FlightPicture {
            flight,
            tick: 90,
            tracks,
            status: Vec::new(),
        }
    }

    /// The friendly flight's picture holds target 10, seen by plane 0 at tick
    /// 60 and by plane 2 at tick 90 (the fresher report); the enemy flight's
    /// picture holds a track of friendly plane 1.
    fn pictured() -> DataLink {
        let mut link = link();
        link.pictures = vec![
            picture(
                FLIGHT,
                vec![
                    track(0, 10, 60, [1., 2., 3.]),
                    track(2, 10, 90, [4., 5., 6.]),
                ],
            ),
            picture(ENEMY, vec![track(10, 1, 90, [7., 8., 9.])]),
        ];
        link
    }

    #[test]
    fn the_freshest_track_of_an_aircraft_in_the_sides_pictures_is_the_one_flown() {
        let link = pictured();
        let fresh = link.track_on(Side::Friendly, 10).unwrap();
        assert_eq!((fresh.reporter, fresh.observed), (2, 90));
        // Only the side's own pictures count: friendly plane 1 is the enemy's
        // track, not the friendly flight's.
        assert!(link.track_on(Side::Friendly, 1).is_none());
        assert!(link.track_on(Side::Enemy, 1).is_some());
        assert!(link.track_on(Side::Friendly, 11).is_none());
    }

    #[test]
    fn an_order_is_taken_on_a_flightmates_track_of_its_target() {
        let link = pictured();
        assert!(link.tracked(0, 10), "the lead's side holds the track");
        assert!(!link.tracked(0, 11), "nobody tracks 11");
        assert!(!link.tracked(99, 10), "an unknown sender");
        // The enemy lead is on the other side, and its picture does not hold 10.
        assert!(!link.tracked(10, 10));
    }

    #[test]
    fn an_assigned_ai_wingman_is_given_the_freshest_track_of_its_target() {
        let mut link = pictured();
        engage(&mut link, 91, &[1, 2], 10);
        let pursuits = link.pursuits();
        assert_eq!(pursuits.len(), 2);
        for (pursuit, receiver) in pursuits.iter().zip([1, 2]) {
            assert_eq!(pursuit.receiver, receiver);
            assert_eq!(pursuit.target, 10);
            assert_eq!(pursuit.position, [4., 5., 6.]);
            assert_eq!(pursuit.velocity, [10., 0., 0.]);
            assert_eq!(pursuit.observed, 90);
        }
    }

    #[test]
    fn no_track_dead_receivers_and_humans_get_no_pursuit() {
        let mut link = pictured();
        // Plane 11 is tracked by nobody: its assignment gives nothing to fly by.
        engage(&mut link, 91, &[1], 11);
        assert!(link.pursuits().is_empty());
        // A human receiver gets the cues, not a pursuit.
        engage(&mut link, 91, &[1], 10);
        link.assignments.insert(
            0,
            Assignment {
                target: 10,
                by: 2,
                tick: 91,
                order: PlayerOrder::EngageMyTarget,
                acknowledged: false,
            },
        );
        let planes: Vec<u32> = link.pursuits().iter().map(|p| p.receiver).collect();
        assert_eq!(planes, [1]);
        // A dead receiver gets none.
        link.members[1].alive = false;
        assert!(link.pursuits().is_empty());
    }
}
