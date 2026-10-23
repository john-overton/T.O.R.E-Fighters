//! The data link's journal: what changed in the picture at once, with when.
//!
//! Like the communication journal it is write-only: no rule reads an entry
//! back, building one draws no random number, and the picture is the same
//! whether or not anything drains it. The host drains it once a tick, for the
//! recorder (slice G9) and the probe's `data link:` lines. A host that never
//! drains keeps the newest [`CAPACITY`] entries and counts the rest in
//! [`Journal::lost`].

use super::ClearReason;
use std::collections::VecDeque;
use tore_sim::ai::wing::PlayerOrder;

/// Entries kept between drains.
pub const CAPACITY: usize = 1024;

/// One change of the picture. Slice G3a added the assignment entries and slice
/// G9 the sort warning, for the recorder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A plane joined the picture, with whether its aircraft has a radar (the
    /// first tick it is seen).
    Member { tick: u64, plane: u32, radar: bool },
    /// A member took a lock on a target.
    Lock { tick: u64, plane: u32, target: u32 },
    /// A member let go of a lock (the target, the member or the lock is gone).
    Unlock { tick: u64, plane: u32, target: u32 },
    /// A lead gave `plane` a target (`by` is the lead). The words of the call
    /// are in the communication journal's entry for the order.
    Assign {
        tick: u64,
        plane: u32,
        target: u32,
        by: u32,
        order: PlayerOrder,
    },
    /// `plane`'s assignment ended.
    Clear {
        tick: u64,
        plane: u32,
        target: u32,
        why: ClearReason,
    },
    /// `plane` locked the target it was assigned (the first time).
    Acknowledge { tick: u64, plane: u32, target: u32 },
    /// A human flying `plane` was told that its flightmate `other` holds a
    /// lock on `target`, the aircraft it holds (the warning's HUD line and
    /// beep are cues, so this entry is only for the recording).
    SortWarning {
        tick: u64,
        plane: u32,
        other: u32,
        target: u32,
    },
}

/// A bounded list of entries waiting for the host.
#[derive(Clone, Debug, Default)]
pub struct Journal {
    entries: VecDeque<Entry>,
    lost: u64,
}

impl Journal {
    pub fn push(&mut self, entry: Entry) {
        if self.entries.len() >= CAPACITY {
            self.entries.pop_front();
            self.lost += 1;
        }
        self.entries.push_back(entry);
    }

    /// Takes every entry waiting, oldest first.
    pub fn take(&mut self) -> Vec<Entry> {
        self.entries.drain(..).collect()
    }

    /// Entries waiting.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries dropped because nobody drained them.
    pub fn lost(&self) -> u64 {
        self.lost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock(tick: u64) -> Entry {
        Entry::Lock {
            tick,
            plane: 1,
            target: 2,
        }
    }

    #[test]
    fn it_keeps_entries_in_order_and_drains_them() {
        let mut journal = Journal::default();
        journal.push(lock(1));
        journal.push(lock(2));
        assert_eq!(journal.len(), 2);
        assert_eq!(journal.take(), [lock(1), lock(2)]);
        assert!(journal.is_empty());
    }

    #[test]
    fn a_host_that_never_drains_keeps_the_newest_entries() {
        let mut journal = Journal::default();
        for tick in 0..CAPACITY as u64 + 10 {
            journal.push(lock(tick));
        }
        assert_eq!(journal.len(), CAPACITY);
        assert_eq!(journal.lost(), 10);
        assert_eq!(journal.take()[0], lock(10));
    }
}
