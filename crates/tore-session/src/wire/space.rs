//! How a snapshot packet's 1,200 bytes are shared, so that the entities
//! always keep their place (net-protocol.md, "Snapshots").
//!
//! After the headers, the cockpit readout takes up to 200 bytes, the events
//! up to 150 and the reliable messages up to 256; the entities get the rest,
//! at least 540 bytes. A section that needs less than its share leaves the
//! rest to the entities. The events are written after the entities, so the
//! events may also use whatever the entities left, beyond their own share.
//! The transport fills the room left at the end with due messages, up to its
//! message budget (256 bytes during flight).

use tore_net::MAX_DATAGRAM;
use tore_net::packet::{PAYLOAD_HEADER_LEN, SECTION_HEADER_LEN};

/// The cockpit readout's share.
pub const READOUT_SHARE: usize = 200;
/// The events' share.
pub const EVENTS_SHARE: usize = 150;
/// The reliable messages' share during flight.
pub const MESSAGES_SHARE: usize = 256;
/// The snapshot header's most bytes, the readout's presence bit included.
pub const SNAPSHOT_HEADER_BYTES: usize = 21;
/// The entities' least share, the per-kind record counts included.
pub const ENTITIES_MIN: usize = MAX_DATAGRAM
    - PAYLOAD_HEADER_LEN
    - 3 * SECTION_HEADER_LEN
    - SNAPSHOT_HEADER_BYTES
    - READOUT_SHARE
    - EVENTS_SHARE
    - MESSAGES_SHARE;

/// The bytes each part of one snapshot packet may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shares {
    pub readout: usize,
    pub events: usize,
    pub messages: usize,
    /// The entity records and their counts.
    pub entities: usize,
}

/// The shares for a readout of `readout` bytes, `events` bytes of events
/// waiting and `messages` bytes of messages the caller keeps room for.
pub fn plan(readout: usize, events: usize, messages: usize) -> Shares {
    let readout = readout.min(READOUT_SHARE);
    let events = events.min(EVENTS_SHARE);
    let messages = messages.min(MESSAGES_SHARE);
    let fixed = PAYLOAD_HEADER_LEN + SECTION_HEADER_LEN + SNAPSHOT_HEADER_BYTES;
    let mut used = fixed + readout;
    if events > 0 {
        used += SECTION_HEADER_LEN + events;
    }
    if messages > 0 {
        used += SECTION_HEADER_LEN + messages;
    }
    Shares {
        readout,
        events,
        messages,
        entities: MAX_DATAGRAM - used,
    }
}

/// The room for the Events section's body once the Snapshot section of
/// `snapshot` bytes is written, keeping `messages` bytes (and their section
/// header) for the messages.
pub fn events_room(snapshot: usize, messages: usize) -> usize {
    let messages = messages.min(MESSAGES_SHARE);
    let mut used = PAYLOAD_HEADER_LEN + 2 * SECTION_HEADER_LEN + snapshot;
    if messages > 0 {
        used += SECTION_HEADER_LEN + messages;
    }
    MAX_DATAGRAM.saturating_sub(used)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entities_keep_at_least_their_share() {
        const { assert!(ENTITIES_MIN >= 540) };
        for readout in [0, 50, 200, 5_000] {
            for events in [0, 10, 150, 9_000] {
                for messages in [0, 256, 70_000] {
                    let shares = plan(readout, events, messages);
                    assert!(shares.entities >= ENTITIES_MIN);
                    assert!(shares.readout <= READOUT_SHARE);
                    assert!(shares.events <= EVENTS_SHARE);
                }
            }
        }
        assert_eq!(plan(200, 150, 256).entities, ENTITIES_MIN);
        // Nothing else waiting: the entities have the packet.
        assert_eq!(
            plan(0, 0, 0).entities,
            MAX_DATAGRAM - PAYLOAD_HEADER_LEN - SECTION_HEADER_LEN - SNAPSHOT_HEADER_BYTES
        );
    }
}
