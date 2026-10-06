//! The host's journal (stage K; docs/ARCHITECTURE.md, "The journal: one door
//! into the world"). Slice K0 stepped the host's world through
//! [`crate::journal::apply_tick`]; slice K1 builds the door itself:
//!
//! - [`Driver`] owns the host's world. The rest of the host reads it
//!   (`Deref<Target = World>`) and changes it only through the driver: the
//!   changes before a step (the scoring switch, an `ai-slot` revival's store
//!   cut) are recorded and made at the next step, the step goes through
//!   `apply_tick`, and the drains every tick makes (score facts, combat's
//!   device notes) follow it at once, as a standby makes them.
//! - [`Journal`] keeps what the driver did as records for the standby stream
//!   (slice K3 drains them): Flight when a mission starts flying, Tick for
//!   every step, State for each part of the session's state that changed
//!   (`state.rs`), and Ended. It keeps them only while recording, so a host
//!   with no standby holds nothing.
//!
//! *Agent decision (K1):* a change before the step is checked at once (a
//! store cut the world would refuse is refused to the caller then) but made
//! only by the step's `apply_tick`, so a checkpoint taken between two ticks
//! never holds a change the next tick's record makes again.

use super::Host;
use crate::journal::{Change, FlightRecord, Part, StatePart, Tick, apply_tick, drain};
use crate::wire::messages::EndReason;
use std::collections::{BTreeMap, VecDeque};
use std::ops::Deref;
use tore_net::ConnectionId;
use tore_sim::combat::live::DeviceNote;
use tore_world::WorldResult;
use tore_world::score::Facts;
use tore_world::seats::PlaneId;
use tore_world::world::revive::RevivalWeapons;
use tore_world::world::{TickOutput, World};

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;

/// The most records the journal holds for a stream that does not drain
/// them: a minute of ticks with their parts (agent decision). Past it the
/// queue is dropped and [`Journal::lost`] says so; the stream starts its
/// standbys again from a checkpoint.
pub const MAX_QUEUED: usize = 2 * 60 * 120;

/// The one door into the host's world. See the module documentation.
pub(super) struct Driver {
    world: World,
    /// The changes made since the last step, in order: the next step's
    /// record carries them and its `apply_tick` makes them.
    changes: Vec<Change>,
    /// The score facts the last step left, until scoring takes them.
    facts: Facts,
    /// The data link journal's entries of the last step, until the seats
    /// take them (slice G7).
    link: Vec<tore_world::datalink::Entry>,
}

impl Deref for Driver {
    type Target = World;

    fn deref(&self) -> &World {
        &self.world
    }
}

/// Tests change the world by hand (a pilot killed, a kill booked) between
/// two ticks; such a change is not journaled, so a test that replays the
/// journal must make it on its twin too.
#[cfg(test)]
impl std::ops::DerefMut for Driver {
    fn deref_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

impl Driver {
    /// The door to a world just built.
    pub(super) fn new(world: World) -> Self {
        Self {
            world,
            changes: Vec::new(),
            facts: Facts::default(),
            link: Vec::new(),
        }
    }

    /// Scoring on or off ([`World::set_scoring`]), made at the next step.
    pub(super) fn set_scoring(&mut self, on: bool) {
        self.changes.push(Change::Scoring(on));
    }

    /// An `ai-slot` revival's cut of `plane`'s stores
    /// ([`World::cut_ai_stores`]), made at the next step, before the
    /// mission commands. Refused now when the world would refuse it, with
    /// the world's words.
    pub(super) fn cut_ai_stores(
        &mut self,
        plane: PlaneId,
        weapons: RevivalWeapons,
    ) -> WorldResult<()> {
        if weapons != RevivalWeapons::Missiles {
            // The world's own refusals (`World::cut_ai_stores`), checked
            // without changing it.
            let wings = self
                .world
                .ai_wings
                .as_ref()
                .ok_or("no AI flies this mission")?;
            if wings.configuration(plane.0).is_none() {
                return Err(format!("the AI has no configuration for plane {}", plane.0).into());
            }
            if wings.mission().actor(plane.0).is_none() {
                return Err(format!("the AI does not fly plane {}", plane.0).into());
            }
        }
        self.changes.push(Change::StoreCut { plane, weapons });
        Ok(())
    }

    /// Steps the world with `tick` (its mission commands and seat inputs;
    /// the driver adds the changes made since the last step) into `out`, and
    /// drains it. Returns the tick as the journal records it and combat's
    /// device notes, for the event tracker; the score facts wait for
    /// [`Self::take_facts`].
    pub(super) fn step(
        &mut self,
        mut tick: Tick,
        out: &mut TickOutput,
    ) -> WorldResult<(Tick, Vec<DeviceNote>)> {
        let mut changes = std::mem::take(&mut self.changes);
        changes.append(&mut tick.changes);
        tick.changes = changes;
        apply_tick(&mut self.world, &tick, out)?;
        let drained = drain(&mut self.world);
        self.facts = drained.facts;
        self.link = drained.link;
        Ok((tick, drained.notes))
    }

    /// The score facts of the last step, leaving none.
    pub(super) fn take_facts(&mut self) -> Facts {
        std::mem::take(&mut self.facts)
    }

    /// The data link journal's entries of the last step, leaving none.
    pub(super) fn take_link(&mut self) -> Vec<tore_world::datalink::Entry> {
        std::mem::take(&mut self.link)
    }

    /// The changes waiting for the next step.
    #[cfg(test)]
    pub(super) fn pending(&self) -> &[Change] {
        &self.changes
    }
}

/// One record of the host's journal, in the order the host made them.
#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))] // Slice K3 drains the journal.
pub enum JournalRecord {
    /// A mission started flying: its world is built fresh from the flight's
    /// spec text, at tick 0.
    Flight(FlightRecord),
    /// One step, with its changes before the step.
    Tick(Tick),
    /// A part of the session's state changed; it holds after `tick`.
    State(StatePart),
    /// The mission ended: the world is dropped.
    Ended(EndReason),
}

/// The journal's records on their way to the standby stream, and what the
/// state parts last coded to.
#[derive(Debug, Default)]
pub(super) struct Journal {
    /// Records are kept (slice K3 turns it on while it has a standby).
    recording: bool,
    /// The records not yet drained, oldest first.
    queue: VecDeque<JournalRecord>,
    /// The queue passed [`MAX_QUEUED`] and was dropped since the last drain.
    lost: bool,
    /// Each part's FNV-1a 64 as it last went into the queue: a part whose
    /// coding differs has changed.
    sent: BTreeMap<Part, u64>,
    /// Every connection's join order this mission, kept after it leaves,
    /// so state that still names a departed connection codes (`state.rs`).
    pub(super) orders: BTreeMap<ConnectionId, u64>,
}

impl Journal {
    fn push(&mut self, record: JournalRecord) {
        if !self.recording {
            return;
        }
        if self.queue.len() >= MAX_QUEUED {
            self.queue.clear();
            self.sent.clear();
            self.lost = true;
        }
        self.queue.push_back(record);
    }
}

impl Host {
    /// Keeps the journal's records from now on, or stops and drops them
    /// (slice K3, while it has a standby). Turned on, every part goes out
    /// again with the next records.
    #[cfg_attr(not(test), allow(dead_code))] // Slice K3 drains the journal.
    pub(crate) fn record_journal(&mut self, on: bool) {
        self.journal.recording = on;
        self.journal.sent.clear();
        if !on {
            self.journal.queue.clear();
            self.journal.lost = false;
        }
    }

    /// The records made since the last call, oldest first, and whether some
    /// were dropped before them ([`MAX_QUEUED`]).
    #[cfg_attr(not(test), allow(dead_code))] // Slice K3 drains the journal.
    pub(crate) fn drain_journal(&mut self) -> (Vec<JournalRecord>, bool) {
        let lost = std::mem::take(&mut self.journal.lost);
        (self.journal.queue.drain(..).collect(), lost)
    }

    /// A mission started flying on the world the driver holds now: the
    /// Flight record, and a fresh list of join orders.
    pub(super) fn journal_flight(&mut self) {
        self.journal.orders = self
            .peers
            .iter()
            .map(|(id, peer)| (*id, peer.lobby.order))
            .collect();
        let record = FlightRecord {
            mission: self.number,
            spec_hash: tore_codec::fnv1a64(self.spec_text.as_bytes()),
            identity: self.world.mission_identity(),
        };
        self.journal.push(JournalRecord::Flight(record));
    }

    /// The tick the host just stepped, as the journal records it.
    pub(super) fn journal_ticked(&mut self, tick: Tick) {
        self.journal.push(JournalRecord::Tick(tick));
    }

    /// The mission ended for `reason`.
    pub(super) fn journal_ended(&mut self, reason: EndReason) {
        self.journal.push(JournalRecord::Ended(reason));
    }

    /// Every part whose coding changed since it last went into the queue,
    /// after the last tick stepped: after each tick in flight, and on each
    /// update otherwise. Nothing while not recording.
    pub(super) fn journal_parts(&mut self) {
        if !self.journal.recording {
            return;
        }
        for (id, peer) in &self.peers {
            self.journal.orders.insert(*id, peer.lobby.order);
        }
        // The tick after which the parts hold: the last stepped (0 before
        // any).
        let tick = u32::try_from(self.world.tick().saturating_sub(1)).unwrap_or(u32::MAX);
        for part in super::state::JOURNALED {
            let (bytes, hash) = match self.encode_part_hashed(part) {
                Ok(coded) => coded,
                Err(error) => {
                    // A part that does not code is a fault in its coder: it is
                    // logged once until the error changes, and left out of the
                    // stream.
                    let text = format!("state part {part:?}: {error}");
                    let hash = tore_codec::fnv1a64(text.as_bytes());
                    if self.journal.sent.insert(part, hash) != Some(hash) {
                        let tick = self.world.tick();
                        self.log(super::HostLog::Fault { tick, text });
                    }
                    continue;
                }
            };
            if self.journal.sent.get(&part) == Some(&hash) {
                continue;
            }
            self.journal.sent.insert(part, hash);
            self.journal
                .push(JournalRecord::State(StatePart { part, tick, bytes }));
        }
    }

    /// Keeps every tick the host steps from now on, for a test's twin.
    #[cfg(test)]
    pub(crate) fn keep_journal(&mut self) {
        self.record_journal(true);
    }

    /// The ticks kept since [`Host::keep_journal`], leaving none.
    #[cfg(test)]
    pub(crate) fn take_journal(&mut self) -> Vec<Tick> {
        self.drain_journal()
            .0
            .into_iter()
            .filter_map(|record| match record {
                JournalRecord::Tick(tick) => Some(tick),
                _ => None,
            })
            .collect()
    }

    /// The flying mission's spec text, which a twin is built from.
    #[cfg(test)]
    pub(crate) fn flight_spec_text(&self) -> &str {
        &self.spec_text
    }
}
