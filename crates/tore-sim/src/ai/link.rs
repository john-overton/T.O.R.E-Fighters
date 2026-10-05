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
//! The table is not state: it is rebuilt from the controllers every tick, so
//! exact checkpoints need not code it.

use super::mission::AiActor;
use super::targeting::Side;
use super::weapon_service::Phase;

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
}

impl Engagements {
    /// The table at the start of a decision loop: every actor's engagement as
    /// the previous tick left it.
    pub fn new(actors: &[AiActor]) -> Self {
        Self {
            rows: actors.iter().map(Row::of).collect(),
        }
    }

    /// Rewrite the row of the actor at `index`, right after it stepped.
    pub fn decided(&mut self, index: usize, actor: &AiActor) {
        debug_assert_eq!(self.rows[index].actor, actor.id(), "rows in actor order");
        self.rows[index] = Row::of(actor);
    }

    /// The target `actor` attacks, as the table holds it now.
    pub fn target(&self, actor: u32) -> Option<u32> {
        self.rows
            .iter()
            .find(|row| row.actor == actor)
            .and_then(|row| row.target)
    }

    /// The targets the other members of `actor`'s wing attack, in decision
    /// order, one entry per attacker.
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

    /// Compare the table's wing targets with the old scan at `index`'s turn.
    pub(crate) fn wing(actors: &[AiActor], index: usize, table: &[u32]) {
        let scanned = scanned_wing_targets(actors, index);
        assert_eq!(
            table,
            scanned.as_slice(),
            "engagement table differs from the live scan for actor {} (index {index})",
            actors[index].id()
        );
        count(|r| {
            r.wing_checks += 1;
            r.wing_attacking += u64::from(!scanned.is_empty());
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
