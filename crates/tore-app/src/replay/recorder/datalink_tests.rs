//! The data link's journal as recorded events, from synthetic entries.
use super::super::Recorder;
use super::*;
use tore_replay::vocab::field;
use tore_replay::{AircraftInfo, Side};
use tore_sim::ai::wing::PlayerOrder;
use tore_world::datalink::ClearReason;

fn roster() -> Vec<AircraftInfo> {
    [(0, "You"), (1, "Red two"), (3, "Enemy 1-1")]
        .into_iter()
        .map(|(id, label)| AircraftInfo {
            id,
            label: label.into(),
            side: Side::Friendly,
            ..AircraftInfo::default()
        })
        .collect()
}

fn every_entry() -> Vec<Entry> {
    vec![
        Entry::Member {
            tick: 0,
            plane: 1,
            radar: true,
        },
        Entry::Member {
            tick: 0,
            plane: 2,
            radar: false,
        },
        Entry::Lock {
            tick: 5,
            plane: 1,
            target: 3,
        },
        Entry::Assign {
            tick: 6,
            plane: 1,
            target: 3,
            by: 0,
            order: PlayerOrder::EngageMyTarget,
        },
        Entry::Acknowledge {
            tick: 7,
            plane: 1,
            target: 3,
        },
        Entry::SortWarning {
            tick: 8,
            plane: 0,
            other: 1,
            target: 3,
        },
        Entry::Unlock {
            tick: 9,
            plane: 1,
            target: 3,
        },
        Entry::Clear {
            tick: 10,
            plane: 1,
            target: 3,
            why: ClearReason::TargetLost,
        },
    ]
}

#[test]
fn each_entry_becomes_its_event_with_who_and_what() {
    let (mut recorder, _receiver) = Recorder::detached(64, &roster());
    recorder.datalink(&every_entry());
    let kinds: Vec<&str> = recorder.early.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(
        kinds,
        [
            kind::DATALINK_MEMBER,
            kind::DATALINK_MEMBER,
            kind::DATALINK_LOCK,
            kind::DATALINK_ASSIGN,
            kind::DATALINK_ACKNOWLEDGE,
            kind::DATALINK_SORT_WARNING,
            kind::DATALINK_UNLOCK,
            kind::DATALINK_CLEAR,
        ]
    );
    let early = &recorder.early;
    assert_eq!(early[0].subject, Some(1));
    assert_eq!(early[0].flag(field::RADAR), Some(true));
    assert_eq!(early[1].flag(field::RADAR), Some(false));
    assert_eq!((early[2].subject, early[2].object), (Some(1), Some(3)));
    // The lead gives, the wingman receives, the target is a field.
    let assign = &early[3];
    assert_eq!((assign.subject, assign.object), (Some(0), Some(1)));
    assert_eq!(assign.id(field::TARGET), Some(3));
    assert_eq!(assign.string(field::ORDER), Some("EngageMyTarget"));
    assert_eq!((early[4].subject, early[4].object), (Some(1), Some(3)));
    let warning = &early[5];
    assert_eq!((warning.subject, warning.object), (Some(0), Some(3)));
    assert_eq!(warning.id(field::OTHER), Some(1));
    assert_eq!((early[6].subject, early[6].object), (Some(1), Some(3)));
    assert_eq!(early[7].string(field::REASON), Some("target lost"));
}

#[test]
fn a_drain_notes_the_entries_the_journal_lost_once() {
    let (mut recorder, _receiver) = Recorder::detached(64, &roster());
    let entries = every_entry();
    recorder.datalink_drained(0, &entries);
    assert_eq!(recorder.early.len(), entries.len());
    // A host that skipped draining: the oldest are gone and a note says so.
    recorder.early.clear();
    recorder.datalink_drained(3, &entries[..1]);
    assert_eq!(recorder.early.len(), 2);
    assert_eq!(recorder.early[0].kind, kind::SYSTEM_NOTE);
    assert!(recorder.early[0].text.contains("3 entries were lost"));
    // The same loss is not noted twice; a new one notes only the difference.
    recorder.early.clear();
    recorder.datalink_drained(3, &[]);
    assert!(recorder.early.is_empty());
    recorder.datalink_drained(5, &[]);
    assert!(recorder.early[0].text.contains("2 entries were lost"));
}

#[test]
fn draining_a_link_takes_its_journal() {
    let (mut recorder, _receiver) = Recorder::detached(64, &roster());
    let mut link = DataLink::default();
    recorder.drain_datalink(&mut link);
    assert!(recorder.early.is_empty());
}
