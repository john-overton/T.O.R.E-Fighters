//! The Snapshot section (host to client) and the per-connection baselines.
//!
//! See net-protocol.md, "Snapshots". The section is the header, the cockpit
//! readout's slot, then the entity records kind by kind (aircraft,
//! projectiles, debris, pilots), each kind's count first and its records in id
//! order.
//!
//! **Baselines are per entity.** [`EntitySender`] (the host, per connection)
//! remembers the quantized state it put in each packet; when the transport
//! reports a packet delivered, those states become each entity's
//! acknowledged baseline. A record codes against its entity's newest
//! acknowledged state if that is at most [`MAX_BASELINE_BACK`] (127)
//! snapshots old, else in full.
//! [`EntityReceiver`] (the client) keeps every entity's received states,
//! so it can decode each packet whatever happened to the others.
//!
//! *Agent decision:* a record names its baseline by how many snapshots back
//! it was (`tick - back × ticks per snapshot`), not by transport packets,
//! since the host sends other packets between snapshots (own state, messages)
//! and cannot know their numbers before it sends them.

use super::bits::{self, read_uladder, write_uladder};
use super::entity::{
    self, Delta, Entity, EntityKey, EntityKind, EntityState, read_delta, read_full, write_delta,
    write_full,
};
use super::priority::Relevance;
use super::readout::ReadoutRaw;
use super::{WireError, WireResult};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use tore_codec::{BitReader, BitWriter};

/// Bits of a record's (and the cockpit readout's) baseline field: snapshots
/// back to the acknowledged state it codes against, 0 for none. 7 since
/// protocol 16 (slice B2), 5 before.
pub const BASELINE_BACK_BITS: u32 = 7;
/// The most snapshots back a baseline may be: 127, 2.1 seconds at 60 a
/// second (31 before protocol 16, half a second at 60 a second).
pub const MAX_BASELINE_BACK: u32 = (1 << BASELINE_BACK_BITS) - 1;
/// Snapshots behind the newest a packet may arrive and still be decoded.
const LATE_SNAPSHOTS: u32 = 32;
/// Snapshots of states the client keeps per entity, and of readouts: a
/// packet 32 behind the newest can still arrive, and its records reach 127
/// further back.
pub const HISTORY_SNAPSHOTS: u32 = LATE_SNAPSHOTS + MAX_BASELINE_BACK + 1;
/// Snapshot packets the host remembers per connection while it waits to hear
/// of them: one acknowledged as late as the window allows is still of use.
pub(crate) const PENDING_PACKETS: usize = MAX_BASELINE_BACK as usize + 1;
/// Id differences between records of a kind: zero, 4 bits, 10 bits, or a
/// varint.
const ID_LADDER: [u32; 3] = [0, 4, 10];

/// The snapshot header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotHeader {
    /// The connection's flight the snapshot belongs to: the number the
    /// Seated message gave (protocol 3). Each flight's sections code against
    /// that flight's baselines only, so a client drops those of an earlier
    /// flight that arrive late.
    pub flight: u8,
    /// The host tick the snapshot shows, after that tick's step.
    pub tick: u32,
    /// The newest input tick received from this player.
    pub input_received: u32,
    /// Over the inputs received since the last snapshot, the fewest ticks by
    /// which one arrived before the host needed it; negative when late.
    pub input_margin: i8,
    /// Ticks since the last snapshot for which the host repeated the last
    /// input.
    pub inputs_repeated: u8,
    /// The highest command number applied, all before it applied too.
    pub commands_applied: u16,
    /// FNV-1a 64 of the player's plane's exact state at `tick`; `None`
    /// before seating.
    pub own_hash: Option<u64>,
}

impl SnapshotHeader {
    fn write(&self, w: &mut BitWriter) {
        let _ = w.write_bits(u64::from(self.flight), 8);
        let _ = w.write_bits(u64::from(self.tick), 32);
        let _ = w.write_bits(u64::from(self.input_received), 32);
        let _ = w.write_signed(i64::from(self.input_margin), 8);
        let _ = w.write_bits(u64::from(self.inputs_repeated), 8);
        let _ = w.write_bits(u64::from(self.commands_applied), 16);
        bits::write_option(w, self.own_hash, |w, hash| {
            let _ = w.write_bits(hash, 64);
        });
    }

    fn read(r: &mut BitReader<'_>) -> WireResult<Self> {
        Ok(Self {
            flight: r.read_bits(8)? as u8,
            tick: r.read_bits(32)? as u32,
            input_received: r.read_bits(32)? as u32,
            input_margin: r.read_signed(8)? as i8,
            inputs_repeated: r.read_bits(8)? as u8,
            commands_applied: r.read_bits(16)? as u16,
            own_hash: bits::read_option(r, |r| Ok(r.read_bits(64)?))?,
        })
    }
}

/// One record as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordBody {
    /// The entity is gone.
    Removed,
    /// Every field, with nothing to go on.
    Full(EntityState),
    /// Against the entity's state `back` snapshots before this one.
    Delta { back: u8, delta: Delta },
}

/// One entity's record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub key: EntityKey,
    pub body: RecordBody,
}

/// A Snapshot section as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotSection {
    pub header: SnapshotHeader,
    /// The cockpit readout's record, when the player has a plane.
    pub readout: Option<ReadoutRaw>,
    /// Records in kind order, then id order.
    pub records: Vec<Record>,
}

/// What a record says, for writing.
enum Body<'a> {
    Removed,
    Full(&'a EntityState),
    Delta {
        back: u8,
        state: &'a EntityState,
        base: &'a EntityState,
        ticks: u32,
    },
}

fn write_body(w: &mut BitWriter, body: &Body<'_>) -> WireResult<()> {
    match body {
        Body::Removed => w.write_bool(true),
        Body::Full(state) => {
            w.write_bool(false);
            let _ = w.write_bits(0, BASELINE_BACK_BITS);
            write_full(w, state)?;
        }
        Body::Delta {
            back,
            state,
            base,
            ticks,
        } => {
            w.write_bool(false);
            let _ = w.write_bits(u64::from(*back), BASELINE_BACK_BITS);
            write_delta(w, state, base, *ticks)?;
        }
    }
    Ok(())
}

fn id_bits_at_most(id: u32) -> usize {
    let mut w = BitWriter::new();
    write_uladder(&mut w, u64::from(id), &ID_LADDER);
    w.bit_len()
}

/// Writes the header, an empty readout slot and `records` (already in kind
/// and id order, each body's bits written) into a section.
pub(crate) fn write_section(
    header: &SnapshotHeader,
    readout: Option<&BitWriter>,
    records: &[(EntityKey, BitWriter)],
) -> WireResult<Vec<u8>> {
    let mut w = BitWriter::with_capacity(1_200);
    header.write(&mut w);
    // The cockpit readout: a presence bit, then its record.
    w.write_bool(readout.is_some());
    if let Some(readout) = readout {
        bits::append(&mut w, readout);
    }
    write_entities(&mut w, records)?;
    Ok(bits::finish(w))
}

/// The bits the entity part of a section takes: the counts and `records`.
pub(crate) fn entity_bits(records: &[(EntityKey, BitWriter)]) -> WireResult<usize> {
    let mut w = BitWriter::new();
    write_entities(&mut w, records)?;
    Ok(w.bit_len())
}

fn write_entities(w: &mut BitWriter, records: &[(EntityKey, BitWriter)]) -> WireResult<()> {
    for kind in EntityKind::ALL {
        let of_kind: Vec<&(EntityKey, BitWriter)> =
            records.iter().filter(|(key, _)| key.kind == kind).collect();
        entity::check_count(kind, of_kind.len())?;
        bits::write_count(w, of_kind.len());
        let mut previous: Option<u32> = None;
        for (key, body) in of_kind {
            let delta = match previous {
                None => u64::from(key.id),
                Some(p) if key.id > p => u64::from(key.id - p - 1),
                Some(_) => return Err(WireError::Invalid("records out of id order")),
            };
            write_uladder(w, delta, &ID_LADDER);
            bits::append(w, body);
            previous = Some(key.id);
        }
    }
    Ok(())
}

impl SnapshotSection {
    /// Reads a section. Records against a baseline are parsed without it;
    /// [`EntityReceiver::receive`] applies them.
    pub fn decode(bytes: &[u8]) -> WireResult<Self> {
        let mut r = BitReader::new(bytes);
        let header = SnapshotHeader::read(&mut r)?;
        let readout = if r.read_bool()? {
            Some(super::readout::read_record(&mut r)?)
        } else {
            None
        };
        let mut records = Vec::new();
        for kind in EntityKind::ALL {
            let count = bits::read_count(&mut r, kind.limit(), "records")?;
            let mut previous: Option<u32> = None;
            for _ in 0..count {
                let delta = read_uladder(&mut r, &ID_LADDER)?;
                let id = match previous {
                    None => delta,
                    Some(p) => u64::from(p) + delta + 1,
                };
                let id = u32::try_from(id).map_err(|_| WireError::Invalid("entity id"))?;
                previous = Some(id);
                let key = EntityKey { kind, id };
                let body = if r.read_bool()? {
                    RecordBody::Removed
                } else {
                    match r.read_bits(BASELINE_BACK_BITS)? as u8 {
                        0 => RecordBody::Full(read_full(&mut r, kind)?),
                        back => RecordBody::Delta {
                            back,
                            delta: read_delta(&mut r, kind)?,
                        },
                    }
                };
                records.push(Record { key, body });
            }
        }
        bits::end(&mut r)?;
        Ok(Self {
            header,
            readout,
            records,
        })
    }

    /// Writes a section of full records and removals, with no baselines:
    /// for tests and tools. The host writes through [`EntitySender`].
    pub fn encode_full(
        header: &SnapshotHeader,
        entities: &[Entity],
        removed: &[EntityKey],
    ) -> WireResult<Vec<u8>> {
        let mut records: Vec<(EntityKey, BitWriter)> = Vec::new();
        for entity in entities {
            let mut w = BitWriter::new();
            write_body(&mut w, &Body::Full(&entity.state))?;
            records.push((entity.key(), w));
        }
        for key in removed {
            let mut w = BitWriter::new();
            write_body(&mut w, &Body::Removed)?;
            records.push((*key, w));
        }
        records.sort_by_key(|(key, _)| *key);
        write_section(header, None, &records)
    }
}

/// What one snapshot's entity part held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntityReport {
    /// Records of present entities.
    pub sent: usize,
    /// Of those, full records.
    pub full: usize,
    /// Removal records.
    pub removed: usize,
    /// Due entities that did not fit and wait.
    pub waiting: usize,
    /// Present entities not yet due (the far band between its sends).
    pub not_due: usize,
    /// Bytes of the whole section.
    pub bytes: usize,
}

#[derive(Clone, Debug)]
struct SentPacket {
    sequence: Option<u16>,
    tick: u32,
    records: Vec<(EntityKey, Option<EntityState>)>,
}

/// The host's entity bookkeeping for one connection: acknowledged baselines,
/// priorities, pending removals and the packets not yet heard of.
#[derive(Clone, Debug)]
pub struct EntitySender {
    ticks_per_snapshot: u32,
    snapshot_rate: u32,
    acked: HashMap<EntityKey, (u32, EntityState)>,
    priority: HashMap<EntityKey, u32>,
    removals: BTreeSet<EntityKey>,
    known: HashSet<EntityKey>,
    packets: VecDeque<SentPacket>,
}

impl EntitySender {
    /// Bookkeeping for a connection whose host sends a snapshot every
    /// `ticks_per_snapshot` ticks of 120 a second.
    pub fn new(ticks_per_snapshot: u32) -> Self {
        let ticks_per_snapshot = ticks_per_snapshot.clamp(1, 120);
        Self {
            ticks_per_snapshot,
            snapshot_rate: (120 / ticks_per_snapshot).max(1),
            acked: HashMap::new(),
            priority: HashMap::new(),
            removals: BTreeSet::new(),
            known: HashSet::new(),
            packets: VecDeque::new(),
        }
    }

    /// Builds the Snapshot section for `header`: `entities` are everything
    /// the player's game should draw at `header.tick` (every other aircraft,
    /// missile, debris piece and pilot) with each one's relevance, and
    /// `budget` is the entities' share of the packet in bytes (see
    /// [`super::space`]). The section is staged: call [`Self::sent`] with the
    /// packet's sequence once it is sent, or [`Self::discard`].
    pub fn build(
        &mut self,
        header: &SnapshotHeader,
        entities: &[(Entity, Relevance)],
        budget: usize,
    ) -> WireResult<(Vec<u8>, EntityReport)> {
        let (records, mut report) = self.select(header, entities, budget)?;
        let bytes = write_section(header, None, &records)?;
        report.bytes = bytes.len();
        Ok((bytes, report))
    }

    /// Chooses the records of [`Self::build`] and stages them, leaving the
    /// section to be written with a cockpit readout beside them.
    pub(crate) fn select(
        &mut self,
        header: &SnapshotHeader,
        entities: &[(Entity, Relevance)],
        budget: usize,
    ) -> WireResult<(Vec<(EntityKey, BitWriter)>, EntityReport)> {
        self.discard();
        let tick = header.tick;
        let mut present: HashMap<EntityKey, (&EntityState, &Relevance)> = HashMap::new();
        for (entity, relevance) in entities {
            if present
                .insert(entity.key(), (&entity.state, relevance))
                .is_some()
            {
                return Err(WireError::Invalid("two entities with one key"));
            }
        }
        // Whatever the client may know that is gone is removed until the
        // removal is acknowledged; whatever came back is sent in full.
        for key in self.known.iter() {
            if !present.contains_key(key) {
                self.removals.insert(*key);
            }
        }
        for key in &self.removals {
            self.acked.remove(key);
            self.priority.remove(key);
        }
        self.removals.retain(|key| !present.contains_key(key));
        self.priority.retain(|key, _| present.contains_key(key));

        // Priorities grow by each band's weight; a new entity is due at once.
        let threshold = self.snapshot_rate;
        for (key, (_, relevance)) in &present {
            let weight = relevance.weight(key.kind, threshold);
            self.priority
                .entry(*key)
                .and_modify(|p| *p = p.saturating_add(weight))
                .or_insert(threshold);
        }
        let mut forced: Vec<EntityKey> = Vec::new();
        let mut due: Vec<(u32, EntityKey)> = Vec::new();
        let mut not_due = 0;
        for (key, (_, relevance)) in &present {
            let priority = self.priority[key];
            if relevance.forced(key.kind) {
                forced.push(*key);
            } else if priority >= threshold {
                due.push((priority, *key));
            } else {
                not_due += 1;
            }
        }
        forced.sort();
        due.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

        // Choose records until the share is full: the counts' bytes first.
        let mut left = budget
            .saturating_mul(8)
            .saturating_sub(EntityKind::ALL.len() * 16);
        let mut chosen: Vec<(EntityKey, BitWriter, Option<EntityState>, bool)> = Vec::new();
        let mut counts = [0usize; 4];
        let mut waiting = 0;
        let order = forced
            .iter()
            .map(|key| (*key, false))
            .chain(self.removals.iter().map(|key| (*key, true)))
            .chain(due.iter().map(|(_, key)| (*key, false)))
            .collect::<Vec<_>>();
        for (key, removal) in order {
            let kind = usize::from(key.kind.code());
            if counts[kind] >= key.kind.limit() {
                waiting += 1;
                continue;
            }
            let mut w = BitWriter::new();
            let (state, full) = if removal {
                write_body(&mut w, &Body::Removed)?;
                (None, false)
            } else {
                let state = present[&key].0;
                let base = self.acked.get(&key).and_then(|(base_tick, base)| {
                    let ticks = tick.checked_sub(*base_tick)?;
                    let back = ticks / self.ticks_per_snapshot;
                    (ticks > 0
                        && ticks % self.ticks_per_snapshot == 0
                        && back <= MAX_BASELINE_BACK
                        && base.same_identity(state))
                    .then_some((back as u8, base, ticks))
                });
                match base {
                    Some((back, base, ticks)) => {
                        write_body(
                            &mut w,
                            &Body::Delta {
                                back,
                                state,
                                base,
                                ticks,
                            },
                        )?;
                        (Some(*state), false)
                    }
                    None => {
                        write_body(&mut w, &Body::Full(state))?;
                        (Some(*state), true)
                    }
                }
            };
            let size = id_bits_at_most(key.id) + w.bit_len();
            if size > left {
                waiting += 1;
                continue;
            }
            left -= size;
            counts[kind] += 1;
            chosen.push((key, w, state, full));
        }
        chosen.sort_by_key(|(key, ..)| *key);
        let records: Vec<(EntityKey, BitWriter)> = chosen
            .iter()
            .map(|(key, w, ..)| (*key, w.clone()))
            .collect();
        let mut report = EntityReport {
            waiting,
            not_due,
            ..EntityReport::default()
        };
        let mut sent = Vec::with_capacity(chosen.len());
        for (key, _, state, full) in chosen {
            match state {
                Some(state) => {
                    report.sent += 1;
                    report.full += usize::from(full);
                    self.priority.insert(key, 0);
                    self.known.insert(key);
                    sent.push((key, Some(state)));
                }
                None => {
                    report.removed += 1;
                    sent.push((key, None));
                }
            }
        }
        self.packets.push_back(SentPacket {
            sequence: None,
            tick,
            records: sent,
        });
        Ok((records, report))
    }

    /// The staged section went out in the packet numbered `sequence`.
    pub fn sent(&mut self, sequence: u16) {
        if let Some(packet) = self.packets.back_mut()
            && packet.sequence.is_none()
        {
            packet.sequence = Some(sequence);
        }
        while self.packets.len() > PENDING_PACKETS {
            self.packets.pop_front();
        }
    }

    /// The staged section was not sent.
    pub fn discard(&mut self) {
        if self.packets.back().is_some_and(|p| p.sequence.is_none()) {
            self.packets.pop_back();
        }
    }

    /// The packet numbered `sequence` was delivered: its states become
    /// baselines and its removals are done.
    pub fn delivered(&mut self, sequence: u16) {
        let Some(index) = self
            .packets
            .iter()
            .position(|p| p.sequence == Some(sequence))
        else {
            return;
        };
        let Some(packet) = self.packets.remove(index) else {
            return;
        };
        for (key, state) in packet.records {
            match state {
                Some(state) => {
                    if self.removals.contains(&key) {
                        continue;
                    }
                    let newer = self
                        .acked
                        .get(&key)
                        .is_none_or(|(tick, _)| *tick < packet.tick);
                    if newer {
                        self.acked.insert(key, (packet.tick, state));
                    }
                }
                None => {
                    if self.removals.remove(&key) {
                        self.known.remove(&key);
                    }
                }
            }
        }
    }

    /// The packet numbered `sequence` was lost.
    pub fn lost(&mut self, sequence: u16) {
        self.packets.retain(|p| p.sequence != Some(sequence));
    }

    /// Entities whose removal the client has not acknowledged.
    pub fn removals_pending(&self) -> usize {
        self.removals.len()
    }

    /// The newest acknowledged state of `key`, with its tick.
    pub fn acknowledged(&self, key: EntityKey) -> Option<(u32, &EntityState)> {
        self.acked.get(&key).map(|(tick, state)| (*tick, state))
    }
}

#[derive(Clone, Debug, Default)]
struct Track {
    /// Received states by tick, oldest first.
    states: VecDeque<(u32, EntityState)>,
    /// The tick of the newest removal record received.
    removed: Option<u32>,
}

/// What one snapshot told the client about its entities.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReceivedSnapshot {
    pub tick: u32,
    /// Every entity whose state the snapshot carried, resolved.
    pub updated: Vec<Entity>,
    /// Entities the snapshot removed.
    pub removed: Vec<EntityKey>,
    /// Records whose baseline the client does not have, or which did not
    /// apply to it: skipped. The host codes only against acknowledged
    /// states, so this stays empty unless something is wrong.
    pub unresolved: Vec<EntityKey>,
    /// The seat's cockpit readout, when the snapshot carried one and its
    /// baseline was held ([`super::connection::ClientConnection::snapshot`]
    /// fills it).
    pub readout: Option<super::readout::QReadout>,
    /// The snapshot carried a readout whose baseline the client lacked.
    pub readout_unresolved: bool,
}

/// The client's entity states: what each snapshot said about each entity,
/// kept for [`HISTORY_SNAPSHOTS`] snapshots.
#[derive(Clone, Debug)]
pub struct EntityReceiver {
    ticks_per_snapshot: u32,
    tracks: HashMap<EntityKey, Track>,
    newest: Option<u32>,
}

impl EntityReceiver {
    /// The client's copy for a host that sends a snapshot every
    /// `ticks_per_snapshot` ticks.
    pub fn new(ticks_per_snapshot: u32) -> Self {
        Self {
            ticks_per_snapshot: ticks_per_snapshot.clamp(1, 120),
            tracks: HashMap::new(),
            newest: None,
        }
    }

    /// Takes a decoded section, in whatever order sections arrive.
    pub fn receive(&mut self, section: &SnapshotSection) -> ReceivedSnapshot {
        let tick = section.header.tick;
        let mut out = ReceivedSnapshot {
            tick,
            ..ReceivedSnapshot::default()
        };
        for record in &section.records {
            let key = record.key;
            let state = match &record.body {
                RecordBody::Removed => {
                    let track = self.tracks.entry(key).or_default();
                    track.removed = Some(track.removed.map_or(tick, |t| t.max(tick)));
                    out.removed.push(key);
                    continue;
                }
                RecordBody::Full(state) => Ok(*state),
                RecordBody::Delta { back, delta } => {
                    let ticks = u32::from(*back) * self.ticks_per_snapshot;
                    match tick
                        .checked_sub(ticks)
                        .and_then(|base_tick| self.state(key, base_tick))
                    {
                        Some(base) => delta.apply(base, ticks),
                        None => Err(WireError::Invalid("missing baseline")),
                    }
                }
            };
            match state {
                Ok(state) => {
                    let track = self.tracks.entry(key).or_default();
                    match track.states.iter().position(|(t, _)| *t >= tick) {
                        Some(i) if track.states[i].0 == tick => track.states[i].1 = state,
                        Some(i) => track.states.insert(i, (tick, state)),
                        None => track.states.push_back((tick, state)),
                    }
                    out.updated.push(Entity { id: key.id, state });
                }
                Err(_) => out.unresolved.push(key),
            }
        }
        if self.newest.is_none_or(|newest| tick > newest) {
            self.newest = Some(tick);
            self.prune(tick);
        }
        out
    }

    fn prune(&mut self, newest: u32) {
        let keep = newest.saturating_sub(HISTORY_SNAPSHOTS * self.ticks_per_snapshot);
        self.tracks.retain(|_, track| {
            while track.states.len() > 1 && track.states.front().is_some_and(|(t, _)| *t < keep) {
                track.states.pop_front();
            }
            let old_state = track.states.back().is_none_or(|(t, _)| *t < keep);
            let old_removal = track.removed.is_none_or(|t| t < keep);
            !(old_state && old_removal)
        });
    }

    /// The state `key` had in the snapshot of `tick`, if one was received.
    pub fn state(&self, key: EntityKey, tick: u32) -> Option<&EntityState> {
        let track = self.tracks.get(&key)?;
        // The states are in tick order and up to 160 deep (slice B2).
        let index = track.states.binary_search_by_key(&tick, |(t, _)| *t).ok()?;
        Some(&track.states[index].1)
    }

    /// The newest state received for `key`, with its tick, unless a newer
    /// removal was received.
    pub fn latest(&self, key: EntityKey) -> Option<(u32, &EntityState)> {
        let track = self.tracks.get(&key)?;
        let (tick, state) = track.states.back()?;
        if track.removed.is_some_and(|removed| removed >= *tick) {
            return None;
        }
        Some((*tick, state))
    }

    /// Every entity the client knows, with its newest state, in key order.
    pub fn entities(&self) -> Vec<(EntityKey, u32, EntityState)> {
        let mut out: Vec<_> = self
            .tracks
            .keys()
            .filter_map(|key| self.latest(*key).map(|(tick, state)| (*key, tick, *state)))
            .collect();
        out.sort_by_key(|(key, ..)| *key);
        out
    }

    /// The received states of `key`, oldest first: what interpolation reads.
    pub fn history(&self, key: EntityKey) -> impl Iterator<Item = (u32, &EntityState)> {
        self.tracks
            .get(&key)
            .into_iter()
            .flat_map(|track| track.states.iter().map(|(tick, state)| (*tick, state)))
    }
}
