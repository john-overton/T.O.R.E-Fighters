//! What the AI reads from the flight data link ([architecture, "How the AI
//! reads the picture"](../../../../docs/ARCHITECTURE.md#how-the-ai-reads-the-picture)).
//!
//! Slice G1 holds the engagement table and the one engagement and lock rule.
//! The table replaces the live scan of other controllers that target choice
//! made (B41's wing attacker count) and the leader's target in
//! [`LeaderView`](super::controller::LeaderView), with identical results: it
//! is built from every actor at the start of the mission's decision loop and
//! each actor's row is rewritten right after that actor steps, so at any
//! actor's turn earlier actors show this tick's targets and later ones last
//! tick's, the AI's same-tick decision visibility (John, 2026-10-02).
//!
//! Slice G2 adds the humans. The world hands the AI each human's locked
//! target ([`LinkInput`]) before the step, and the table gets one row for each
//! human with a lock, in the human's own side and wing, after the actors' rows.
//! Humans do not decide in the loop, so their rows keep the value the world
//! read after combat for the whole step. An AI wingman then counts the player's
//! locked bandit among the targets its wing attacks (B41), and ranks it with
//! the same penalty as one a wingman attacks.
//!
//! Slice G3b lets an assignment reach the AI through the picture. The world
//! hands the AI, for each AI wingman holding an assignment, the freshest track
//! a flightmate reports of the assigned aircraft ([`Pursuit`]). While the
//! wingman's own sensors do not hold that aircraft, the mission builds a target
//! view from the track ([`Pursuit::view`], flagged `link_track`) and adds it to
//! the actor's targets: the wingman keeps the order, flies toward the track,
//! and fires only once its own sensors hold the aircraft.
//!
//! Slice G4 lets an AI lead use the picture. When a lead under loose control
//! commits to a new target it shares it with its idle wingmen, up to the
//! two-attacker allowance (B43), or, when it knows of another bandit and has
//! not sorted for thirty seconds, sorts: each fit wingman takes a different
//! bandit by [`crate::datalink::sort`]. Each wingman takes its target as the
//! controller takes any target order, the mission's own role unchanged, and
//! the mission writes a [`LinkEvent::Assign`] for the world to record and
//! voice. A wingman told by the world (`AiMission::yield_target`) that it
//! and a flightmate hold one bandit leaves it alone for ten seconds, if it has
//! another to take ([`Yield`]).
//!
//! Neither the table nor the input is state: the table is rebuilt from the
//! controllers every tick and the input is handed over again before every step
//! (and consumed by it), so exact checkpoints need not code them. What the
//! mission itself keeps (each flight's last sort, each actor's yields) is
//! coded in the mission's checkpoint.

use super::ScalarSpeed;
use super::controller::TargetView;
use super::mission::{AiActor, HumanMember, WorldObject};
use super::targeting::Side;
use super::weapon_service::Phase;
use super::wing::PlayerOrder;
use crate::datalink::sort::Bandit;

/// Simulation ticks per second, to age a track.
const TICKS_PER_SECOND: f64 = 120.;

/// The target `actor` attacks: its controller's target while it is alive. A
/// dead actor attacks nothing. The engagement table, the picture's AI
/// engagements and the situation music's "aiming at" all read this.
pub fn engagement_of(actor: &AiActor) -> Option<u32> {
    actor.controller().target().filter(|_| actor.alive())
}

/// The target `actor` holds a missile lock on: its engagement while its
/// weapon service is tracking or firing. The warning receiver's tones and the
/// picture's AI locks read this one rule.
pub fn lock_of(actor: &AiActor) -> Option<u32> {
    engagement_of(actor).filter(|_| {
        matches!(
            actor.controller().weapon_phase(),
            Phase::Tracking | Phase::Fire
        )
    })
}

/// What a human attacks: the target its sensors hold locked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HumanEngagement {
    /// The human-flown aircraft.
    pub plane: u32,
    /// The target it holds locked.
    pub target: u32,
}

/// What a flightmate's report gives an AI wingman to fly toward: the freshest
/// track in the picture of the aircraft the wingman was assigned (slice G3b).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pursuit {
    /// The AI wingman holding the assignment.
    pub receiver: u32,
    /// The assigned aircraft.
    pub target: u32,
    /// Where the reporter saw it, world feet, at `observed`.
    pub position: [f64; 3],
    /// Its ground-relative velocity then, feet per second.
    pub velocity: [f64; 3],
    /// The tick the reporter saw it.
    pub observed: u64,
}

impl Pursuit {
    /// The target view a wingman flies by while its own sensors do not hold
    /// the aircraft: the reported position carried forward at the reported
    /// velocity to `tick`, the heading, pitch and speed that velocity gives,
    /// and the aircraft's type from `object`. Flagged `link_track`, so the
    /// weapons never see it. `None` when the aircraft is no living hostile
    /// aircraft of `side` any more.
    pub fn view(
        &self,
        tick: u64,
        side: Side,
        object: &WorldObject,
        seeker_eligible: bool,
        wing_attackers: u32,
    ) -> Option<TargetView> {
        if object.id != self.target
            || object.side == side
            || !object.is_aircraft
            || !object.alive
            || object.destroyed
        {
            return None;
        }
        let age = tick.saturating_sub(self.observed) as f64 / TICKS_PER_SECOND;
        let position = std::array::from_fn(|axis| self.position[axis] + self.velocity[axis] * age);
        let [vx, vy, vz] = self.velocity;
        let level = vx.hypot(vz);
        Some(TargetView {
            id: self.target,
            side: object.side,
            position,
            heading_deg: vx.atan2(vz).to_degrees().rem_euclid(360.),
            pitch_deg: vy.atan2(level).to_degrees(),
            speed: ScalarSpeed(level.hypot(vy)),
            maximum_speed: object.maximum_speed,
            is_aircraft: true,
            is_fighter: object.is_fighter,
            human_controlled: object.human_controlled,
            valid: true,
            type_allowed: true,
            seeker_eligible,
            wing_attackers,
            terrain_blocked: false,
            sensor_supported: false,
            link_track: true,
        })
    }
}

/// The hostile aircraft a side's flights know of, as the picture last had
/// them carried forward to the tick it was read: what an AI lead sorts from
/// (slice G4).
#[derive(Clone, Debug, PartialEq)]
pub struct SideBandits {
    pub side: Side,
    /// In target id order, without any the picture knows to be dead.
    pub bandits: Vec<Bandit>,
}

/// What a flight's last published picture says of one member's state, as a
/// lead would know it (slice G4). A member the picture has no row for is
/// taken to be fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemberState {
    pub plane: u32,
    /// No missile and no gun rounds left.
    pub winchester: bool,
    /// Bingo fuel or worse.
    pub bingo: bool,
    /// Half its hit points or less.
    pub heavy_damage: bool,
}

impl MemberState {
    /// Whether an AI lead leaves this member out of a share or a sort.
    pub fn skipped(&self) -> bool {
        self.winchester || self.bingo || self.heavy_damage
    }
}

/// What an AI lead knows of a human flying in its wing (slice G11): whether
/// the human is free for a shared target, and what it was last told to
/// attack. The human's side, wing and place come from the mission's own
/// [`HumanMember`] list; its position and whether it is airborne from the
/// world snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HumanWingman {
    pub plane: u32,
    /// The aircraft the human holds an assignment on, if it holds one.
    pub assigned: Option<u32>,
    /// A share may go to the human: it holds no radar lock and no assignment
    /// younger than the sort interval (agent decision, slice G11).
    pub idle: bool,
}

/// What the world hands the AI before a step: the humans' locked targets
/// (slice G2), in plane id order, the pursuits of the assigned AI wingmen
/// (slice G3b), in receiver order, and for the AI leads' shares and sorts
/// (slice G4) the bandits each side knows and each member's state, and
/// (slice G11) what each living human wingman holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinkInput {
    pub humans: Vec<HumanEngagement>,
    pub pursuits: Vec<Pursuit>,
    pub bandits: Vec<SideBandits>,
    pub states: Vec<MemberState>,
    pub wingmen: Vec<HumanWingman>,
}

impl LinkInput {
    /// The pursuit of `receiver`, if the picture gives it one.
    pub fn pursuit_of(&self, receiver: u32) -> Option<&Pursuit> {
        self.pursuits.iter().find(|p| p.receiver == receiver)
    }

    /// The bandits `side` knows of.
    pub fn bandits_of(&self, side: Side) -> &[Bandit] {
        self.bandits
            .iter()
            .find(|known| known.side == side)
            .map_or(&[], |known| known.bandits.as_slice())
    }

    /// What the picture holds of the human `plane` for its AI lead's shares
    /// and sorts (slice G11).
    pub fn wingman_of(&self, plane: u32) -> Option<&HumanWingman> {
        self.wingmen.iter().find(|w| w.plane == plane)
    }

    /// The state of `plane` as its flight's picture last had it.
    pub fn state_of(&self, plane: u32) -> Option<&MemberState> {
        self.states.iter().find(|state| state.plane == plane)
    }
}

/// What an AI lead did with the picture this tick (slice G4), for the world
/// to record and voice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LinkEvent {
    /// A lead gave `receiver` a target: its own, shared under loose control
    /// ([`PlayerOrder::EngageMyTarget`]), or one dealt by a sort
    /// ([`PlayerOrder::Sort`]). The receiver took it before this was written.
    Assign {
        lead: u32,
        receiver: u32,
        target: u32,
        order: PlayerOrder,
    },
    /// `actor` leaves `target` alone for [`YIELD_TICKS`] when it has another
    /// target to take, because a flightmate it was not sorted with locked the
    /// same aircraft.
    Yield { actor: u32, target: u32 },
}

/// How long an AI member leaves a bandit alone after yielding it: ten
/// seconds at 120 Hz.
pub const YIELD_TICKS: u64 = 10 * 120;

/// The least time between two sorts by one flight's AI lead: thirty seconds.
pub const SORT_INTERVAL_TICKS: u64 = 30 * 120;

/// One bandit an actor has agreed to leave alone for a while.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Yield {
    pub target: u32,
    /// The mission tick the yield ends.
    pub until: u64,
    /// The `Yield` event has been written.
    pub announced: bool,
}

/// The tick each flight last sorted, by side and wing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortStamp {
    pub side: Side,
    pub wing: u8,
    pub tick: u64,
}

/// One actor's row: who it is, its wing, and what it attacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Row {
    actor: u32,
    side: Side,
    wing: u8,
    target: Option<u32>,
}

impl Row {
    fn of(actor: &AiActor) -> Self {
        let identity = actor.identity();
        Self {
            actor: actor.id(),
            side: identity.side,
            wing: identity.wing,
            target: engagement_of(actor),
        }
    }
}

/// Who attacks what among the AI, kept in decision order: one row per actor,
/// in the mission's actor order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Engagements {
    rows: Vec<Row>,
    /// How many leading rows are actors; the rest are humans.
    actors: usize,
}

impl Engagements {
    /// The table at the start of a decision loop: every actor's engagement as
    /// the previous tick left it.
    pub fn new(actors: &[AiActor]) -> Self {
        Self {
            rows: actors.iter().map(Row::of).collect(),
            actors: actors.len(),
        }
    }

    /// Add a row for each human in `input` that belongs to a wing, after the
    /// actors' rows. A plane the mission also holds an actor for keeps its
    /// actor row, so a handoff never counts a plane twice.
    pub fn with_humans(mut self, humans: &[HumanMember], input: &LinkInput) -> Self {
        let actors = self.actors;
        for engagement in &input.humans {
            let Some(member) = humans.iter().find(|h| h.id == engagement.plane) else {
                continue;
            };
            if self.rows[..actors]
                .iter()
                .any(|r| r.actor == engagement.plane)
            {
                continue;
            }
            self.rows.push(Row {
                actor: engagement.plane,
                side: member.side,
                wing: member.wing,
                target: Some(engagement.target),
            });
        }
        self
    }

    /// Rewrite the row of the actor at `index`, right after it stepped.
    pub fn decided(&mut self, index: usize, actor: &AiActor) {
        debug_assert_eq!(self.rows[index].actor, actor.id(), "rows in actor order");
        self.rows[index] = Row::of(actor);
    }

    /// The targets the humans of a wing hold locked, in plane id order (slice
    /// G2): the table's last rows, for the audit.
    #[doc(hidden)]
    pub fn human_wing_targets(&self, side: Side, wing: u8) -> Vec<u32> {
        self.rows[self.actors..]
            .iter()
            .filter(|row| row.side == side && row.wing == wing)
            .filter_map(|row| row.target)
            .collect()
    }

    /// The target `actor` attacks, as the table holds it now.
    pub fn target(&self, actor: u32) -> Option<u32> {
        self.rows
            .iter()
            .find(|row| row.actor == actor)
            .and_then(|row| row.target)
    }

    /// The targets the other members of `actor`'s wing attack, one entry per
    /// attacker: the actors' in decision order, then the humans' locks.
    pub fn wing_targets(&self, actor: u32, side: Side, wing: u8) -> Vec<u32> {
        self.rows
            .iter()
            .filter(|row| row.actor != actor && row.side == side && row.wing == wing)
            .filter_map(|row| row.target)
            .collect()
    }
}

/// A check of the engagement table against a copy of the live scans it
/// replaced, run at every actor's turn while a test has started it. Off, it
/// costs one thread-local read per actor step and changes nothing. The AI's
/// decision loop runs on the calling thread (workers only prepare
/// observations), so the switch is per thread.
#[doc(hidden)]
pub mod audit {
    use std::cell::Cell;

    use super::super::mission::AiActor;

    thread_local! {
        static REPORT: Cell<Option<Report>> = const { Cell::new(None) };
    }

    /// What one audit saw.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Report {
        /// Actor turns whose wing attacker list was compared.
        pub wing_checks: u64,
        /// Of those, turns where some other member of the wing attacked.
        pub wing_attacking: u64,
        /// Of those, turns where a human of the wing held a lock.
        pub human_attacking: u64,
        /// Wingmen's turns whose leader target was compared.
        pub leader_checks: u64,
        /// Of those, turns where the leader had a target.
        pub leader_targets: u64,
    }

    /// Start auditing on this thread, from an empty report.
    pub fn start() {
        REPORT.with(|r| r.set(Some(Report::default())));
    }

    /// Stop auditing on this thread and return what it saw.
    pub fn finish() -> Report {
        REPORT.with(|r| r.take()).unwrap_or_default()
    }

    pub(crate) fn active() -> bool {
        REPORT.with(|r| r.get().is_some())
    }

    fn count(update: impl FnOnce(&mut Report)) {
        REPORT.with(|r| {
            if let Some(mut report) = r.get() {
                update(&mut report);
                r.set(Some(report));
            }
        });
    }

    /// The live scan `AiMission::step_actor` made before slice G1, kept as it
    /// was (through the actor's accessors): the targets of the other living actors of the same side and
    /// wing, in actor order.
    fn scanned_wing_targets(actors: &[AiActor], index: usize) -> Vec<u32> {
        let actor_id = actors[index].id();
        let identity = *actors[index].identity();
        actors
            .iter()
            .filter(|a| {
                a.alive()
                    && a.id() != actor_id
                    && a.identity().side == identity.side
                    && a.identity().wing == identity.wing
            })
            .filter_map(|a| a.controller().target())
            .collect()
    }

    /// Compare the table's wing targets with the old scan at `index`'s turn,
    /// followed by the targets `humans` hold in the actor's wing (slice G2: the
    /// humans' rows come after the actors').
    pub(crate) fn wing(actors: &[AiActor], index: usize, table: &[u32], humans: &[u32]) {
        let mut scanned = scanned_wing_targets(actors, index);
        let actors_attacking = !scanned.is_empty();
        scanned.extend_from_slice(humans);
        assert_eq!(
            table,
            scanned.as_slice(),
            "engagement table differs from the live scan for actor {} (index {index})",
            actors[index].id()
        );
        count(|r| {
            r.wing_checks += 1;
            r.wing_attacking += u64::from(actors_attacking);
            r.human_attacking += u64::from(!humans.is_empty());
        });
    }

    /// Compare the table's target for an AI leader with the old read of the
    /// leader's controller in `leader_view`.
    pub(crate) fn leader(leader: &AiActor, table: Option<u32>) {
        let scanned = leader.controller().target();
        assert_eq!(
            table,
            scanned,
            "engagement table differs from the leader's controller for leader {}",
            leader.id()
        );
        count(|r| {
            r.leader_checks += 1;
            r.leader_targets += u64::from(scanned.is_some());
        });
    }
}
