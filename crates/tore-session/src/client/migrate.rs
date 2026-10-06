//! The client's side of host migration (stage K; docs/ARCHITECTURE.md, "On
//! the client"): the succession, detection, the race to the new host,
//! Resume, Resumed and Backlog, and the 10 seconds of inputs kept for it.
//! Slice K0 places the seam and passes the standby stream's records on;
//! slice K4 builds the rest.

use super::Client;
use crate::wire::messages::Message;
use std::collections::VecDeque;

/// The migration's state on the client (slice K4 fills it), and the standby
/// records waiting for the game's standby (slice K2 reads them).
#[derive(Debug, Default)]
pub(super) struct Migration {
    /// Standby records as they arrived, in order, unread.
    records: VecDeque<Vec<u8>>,
}

impl Client {
    /// A message of stage K's migration from the host: Succession, Standby
    /// record, Resumed or Host moving. A standby record is kept for
    /// [`Client::take_standby_records`]; the rest wait for slice K4.
    pub(super) fn migrate_message(&mut self, message: Message) {
        if let Message::StandbyRecord(record) = message {
            self.migration.records.push_back(record);
        }
    }

    /// The standby stream's records since the last call, in the order the
    /// host sent them (stage K): what the game's standby replays. The client
    /// keeps them until they are taken, never reading them.
    pub fn take_standby_records(&mut self) -> Vec<Vec<u8>> {
        self.migration.records.drain(..).collect()
    }
}
