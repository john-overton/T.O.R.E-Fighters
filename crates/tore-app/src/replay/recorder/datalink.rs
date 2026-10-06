//! The data link's journal as recorded events: each plane that joined the
//! picture, each radar lock taken and let go, each assignment given, cleared
//! and acknowledged, and each sort warning a human heard. The journal is
//! write-only (`tore_world::datalink::Journal`); this only reads what the
//! host drained. The events' layout is an agent decision (slice G9,
//! 2026-10-05); see docs/REPLAYS.md ("Data link events").

use super::Recorder;
use tore_replay::{
    Event, Value,
    vocab::{field, kind},
};
use tore_world::datalink::{DataLink, Entry};

/// One journal entry as the event it records.
pub(super) fn event(entry: &Entry) -> Event {
    match *entry {
        Entry::Member { plane, radar, .. } => Event::new(kind::DATALINK_MEMBER)
            .with_subject(plane)
            .with(field::RADAR, radar)
            .with_text(if radar {
                "joined the data link, with a radar"
            } else {
                "joined the data link, with no radar"
            }),
        Entry::Lock { plane, target, .. } => Event::new(kind::DATALINK_LOCK)
            .with_subject(plane)
            .with_object(target)
            .with_text("locked"),
        Entry::Unlock { plane, target, .. } => Event::new(kind::DATALINK_UNLOCK)
            .with_subject(plane)
            .with_object(target)
            .with_text("let go of its lock"),
        Entry::Assign {
            plane,
            target,
            by,
            order,
            ..
        } => Event::new(kind::DATALINK_ASSIGN)
            .with_subject(by)
            .with_object(plane)
            .with(field::TARGET, Value::Id(target))
            .with(field::ORDER, format!("{order:?}")),
        Entry::Clear {
            plane, target, why, ..
        } => Event::new(kind::DATALINK_CLEAR)
            .with_subject(plane)
            .with_object(target)
            .with(field::REASON, why.name())
            .with_text("assignment ended"),
        Entry::Acknowledge { plane, target, .. } => Event::new(kind::DATALINK_ACKNOWLEDGE)
            .with_subject(plane)
            .with_object(target)
            .with_text("locked its assigned target"),
        Entry::SortWarning {
            plane,
            other,
            target,
            ..
        } => Event::new(kind::DATALINK_SORT_WARNING)
            .with_subject(plane)
            .with_object(target)
            .with(field::OTHER, Value::Id(other))
            .with_text("told a flightmate holds the same lock"),
    }
}

impl Recorder {
    /// Drains the data link's journal for this tick, write-only. Entries the
    /// journal's bound threw away since the last drain are noted.
    pub fn drain_datalink(&mut self, link: &mut DataLink) {
        let lost = link.journal_lost();
        self.datalink_drained(lost, &link.take_journal());
    }

    /// What a drain found: `lost` is the journal's count of entries it ever
    /// dropped, `entries` what it held.
    pub fn datalink_drained(&mut self, lost: u64, entries: &[Entry]) {
        if lost > self.why.datalink_lost {
            self.note(Event::new(kind::SYSTEM_NOTE).with_text(format!(
                "the data link journal was full; {} entries were lost",
                lost - self.why.datalink_lost
            )));
            self.why.datalink_lost = lost;
        }
        self.datalink(entries);
    }

    /// Journal entries drained elsewhere, without the loss count.
    pub fn datalink(&mut self, entries: &[Entry]) {
        for entry in entries {
            self.note(event(entry));
        }
    }
}

#[cfg(test)]
#[path = "datalink_tests.rs"]
mod tests;
