//! The data link's journal as recorded events: each plane that joined the
//! picture, each radar lock taken and let go, each assignment given, cleared
//! and acknowledged, and each sort warning a human heard. The journal is
//! write-only (`tore_world::datalink::Journal`); this only reads what the
//! host drained. The events' layout is an agent decision (slice G9,
//! 2026-10-05); see docs/REPLAYS.md ("Data link events").

use super::Recorder;
use tore_replay::{Event, vocab::kind};
use tore_world::datalink::{DataLink, Entry};

/// One journal entry as the event it records: the mapping the capture
/// conversion uses too, so a recorded and a converted flight agree (slice G7
/// moved it to `tore_session::client::convert::datalink_event`).
pub(super) fn event(entry: &Entry) -> Event {
    tore_session::client::convert::datalink_event(entry)
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
