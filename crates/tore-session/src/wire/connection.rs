//! Each end's wire state for one connection, tied to the transport's packet
//! numbers.
//!
//! The host builds a snapshot packet's sections with
//! [`HostConnection::snapshot`], sends them, and tells the connection the
//! packet's sequence ([`HostConnection::sent`]); when the transport reports
//! that packet delivered or lost, [`HostConnection::delivered`] and
//! [`HostConnection::lost`] promote the baselines and acknowledge the events
//! it carried. The client checks each section before the transport accepts
//! the packet ([`ClientConnection::check`]) and then reads it.

use super::entity::Entity;
use super::events::{
    EventQueue, EventReceiver, EventsReport, EventsSection, ReceivedEvent, WireEvent,
};
use super::inputs::InputsSection;
use super::messages::Names;
use super::names::{NameTable, ReceivedNames};
use super::own_state::{self, OwnStateHeader, OwnStateReceiver, OwnStateSender};
use super::priority::Relevance;
use super::snapshot::{
    EntityReceiver, EntityReport, EntitySender, ReceivedSnapshot, SnapshotHeader, SnapshotSection,
};
use super::space::{self, Shares};
use super::{SECTION_EVENTS, SECTION_INPUTS, SECTION_OWN_STATE, SECTION_SNAPSHOT, WireResult};
use tore_sim::models::AircraftModel;
use tore_world::world::plane::ExactState;

/// One snapshot packet's sections and what they held.
#[derive(Clone, Debug)]
pub struct SnapshotPacket {
    pub snapshot: Vec<u8>,
    pub events: Option<Vec<u8>>,
    pub shares: Shares,
    pub entities: EntityReport,
    pub events_report: EventsReport,
}

impl SnapshotPacket {
    /// The sections as the transport's `send_payload` takes them.
    pub fn sections(&self) -> Vec<(u8, &[u8])> {
        let mut sections = vec![(SECTION_SNAPSHOT, self.snapshot.as_slice())];
        if let Some(events) = &self.events {
            sections.push((SECTION_EVENTS, events.as_slice()));
        }
        sections
    }

    /// Bytes of the packet before the transport adds messages: the payload
    /// header, the section headers and the bodies.
    pub fn bytes(&self) -> usize {
        tore_net::packet::PAYLOAD_HEADER_LEN
            + self
                .sections()
                .iter()
                .map(|(_, body)| tore_net::packet::SECTION_HEADER_LEN + body.len())
                .sum::<usize>()
    }
}

/// What is staged for the next packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Staged {
    None,
    Snapshot,
    OwnState,
}

/// The host's wire state for one connection.
#[derive(Clone, Debug)]
pub struct HostConnection {
    pub entities: EntitySender,
    pub events: EventQueue,
    pub own: OwnStateSender,
    pub names: NameTable,
    staged: Staged,
}

impl HostConnection {
    /// A new connection's state; the host sends a snapshot every
    /// `ticks_per_snapshot` ticks.
    pub fn new(ticks_per_snapshot: u32) -> Self {
        Self {
            entities: EntitySender::new(ticks_per_snapshot),
            events: EventQueue::new(),
            own: OwnStateSender::new(),
            names: NameTable::new(),
            staged: Staged::None,
        }
    }

    /// Queues an event that happened at `tick` for this connection.
    pub fn event(&mut self, tick: u32, event: &WireEvent) -> WireResult<u16> {
        self.events.push(tick, event)
    }

    /// Builds a snapshot packet: the Snapshot section with the entities
    /// chosen by priority within their share, and the Events section with as
    /// many unacknowledged events as the rest of the packet holds, keeping
    /// `messages` bytes for the reliable messages (256 during flight, 0 when
    /// the caller knows none are waiting).
    pub fn snapshot(
        &mut self,
        header: &SnapshotHeader,
        entities: &[(Entity, Relevance)],
        messages: usize,
    ) -> WireResult<SnapshotPacket> {
        self.discard();
        let shares = space::plan(0, self.events.waiting_bytes(), messages);
        let (snapshot, entity_report) = self.entities.build(header, entities, shares.entities)?;
        // The events take what the entities left, keeping the messages'
        // room; the oldest may use that room too, so a long one is never
        // starved (the transport does the same for a long message).
        let budget = space::events_room(snapshot.len(), messages);
        let room = space::events_room(snapshot.len(), 0);
        let (events, events_report) = self.events.build(header.tick, budget, room)?;
        self.staged = Staged::Snapshot;
        Ok(SnapshotPacket {
            snapshot,
            events,
            shares,
            entities: entity_report,
            events_report,
        })
    }

    /// Builds an Own state section for the player's plane's `state` at
    /// `tick`, for a packet of its own.
    pub fn own_state(&mut self, tick: u32, state: &ExactState) -> WireResult<Vec<u8>> {
        self.discard();
        let bytes = self.own.build(tick, state)?;
        self.staged = Staged::OwnState;
        Ok(bytes)
    }

    /// The staged packet went out numbered `sequence`.
    pub fn sent(&mut self, sequence: u16) {
        match self.staged {
            Staged::Snapshot => {
                self.entities.sent(sequence);
                self.events.sent(sequence);
            }
            Staged::OwnState => self.own.sent(sequence),
            Staged::None => {}
        }
        self.staged = Staged::None;
    }

    /// The staged packet was not sent.
    pub fn discard(&mut self) {
        match self.staged {
            Staged::Snapshot => {
                self.entities.discard();
                self.events.discard();
            }
            Staged::OwnState => self.own.discard(),
            Staged::None => {}
        }
        self.staged = Staged::None;
    }

    /// The transport reported the packet numbered `sequence` delivered.
    pub fn delivered(&mut self, sequence: u16) {
        self.entities.delivered(sequence);
        self.events.delivered(sequence);
        self.own.delivered(sequence);
    }

    /// The transport reported the packet numbered `sequence` lost.
    pub fn lost(&mut self, sequence: u16) {
        self.entities.lost(sequence);
        self.events.lost(sequence);
        self.own.lost(sequence);
    }

    /// Checks an Inputs section before the transport accepts its packet.
    pub fn check(kind: u8, body: &[u8]) -> bool {
        kind == SECTION_INPUTS && InputsSection::decode(body).is_ok()
    }
}

/// The client's wire state for its connection.
#[derive(Clone, Debug)]
pub struct ClientConnection {
    pub entities: EntityReceiver,
    pub events: EventReceiver,
    pub own: OwnStateReceiver,
    pub names: ReceivedNames,
}

impl ClientConnection {
    /// A new connection's state for a host that sends a snapshot every
    /// `ticks_per_snapshot` ticks.
    pub fn new(ticks_per_snapshot: u32) -> Self {
        Self {
            entities: EntityReceiver::new(ticks_per_snapshot),
            events: EventReceiver::new(),
            own: OwnStateReceiver::new(),
            names: ReceivedNames::new(),
        }
    }

    /// Checks one of a packet's sections before the transport accepts it:
    /// every section must read, and an own state's baseline must be one the
    /// client has. It changes nothing.
    pub fn check(&self, kind: u8, body: &[u8]) -> bool {
        match kind {
            SECTION_SNAPSHOT => SnapshotSection::decode(body).is_ok(),
            SECTION_EVENTS => EventsSection::decode(body).is_ok(),
            SECTION_OWN_STATE => self.own.baseline(body).is_ok(),
            _ => false,
        }
    }

    /// Reads a Snapshot section.
    pub fn snapshot(&mut self, body: &[u8]) -> WireResult<(SnapshotHeader, ReceivedSnapshot)> {
        let section = SnapshotSection::decode(body)?;
        let received = self.entities.receive(&section);
        Ok((section.header, received))
    }

    /// Reads an Events section that came with the snapshot of `tick`: the
    /// new events, once each, ready to present.
    pub fn events(&mut self, body: &[u8], tick: u32) -> WireResult<Vec<ReceivedEvent>> {
        let section = EventsSection::decode(body)?;
        Ok(self.events.receive(&section, tick, self.names.len()))
    }

    /// Takes a Names message: events held for these names are ready now.
    pub fn names(&mut self, names: &Names) -> WireResult<Vec<ReceivedEvent>> {
        self.names.apply(names)?;
        Ok(self.events.names_arrived(self.names.len()))
    }

    /// Reads an Own state section with the plane's aircraft `model`.
    pub fn own_state(
        &mut self,
        body: &[u8],
        model: &AircraftModel,
    ) -> WireResult<(OwnStateHeader, ExactState)> {
        self.own.receive(body, model)
    }
}

/// Reads an Own state section's header without a model or a baseline.
pub fn own_state_header(body: &[u8]) -> WireResult<OwnStateHeader> {
    own_state::OwnStateHeader::peek(body)
}
