//! The coder of the revival bookkeeping (the revival section;
//! docs/formats/checkpoint.md). Slice F2-V.
//!
//! `World::revival` is the [`Book`]: each abandoned plane with the tick it
//! was abandoned and the tick its wreck came to rest (which decide what a
//! revival may retire), the retired planes' roster entries (the results keep
//! a row for each), the planes revivals added (which the mission identity
//! leaves out), and each added plane's lineage root (slice R1's AI respawn).
//! The lists keep their order: the oldest wreck is the first retired.

use super::{Book, LostPlane};
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

tore_sim::checkpoint_struct!(LostPlane {
    plane,
    abandoned,
    resting_since,
});

impl Checkpoint for Book {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let Book {
            lost,
            retired,
            added,
            roots,
        } = self;
        lost.save(s, None)?;
        retired.save(s, None)?;
        added.save(s, None)?;
        roots.save(s, None)
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let book = Book {
            lost: Checkpoint::load(l, None)?,
            retired: Checkpoint::load(l, None)?,
            added: Checkpoint::load(l, None)?,
            roots: Checkpoint::load(l, None)?,
        };
        // A plane is abandoned once and retired once, and a retired plane is
        // no longer abandoned: damaged bytes that break this are refused.
        let mut lost: Vec<_> = book.lost.iter().map(|l| l.plane).collect();
        let mut retired: Vec<_> = book.retired.iter().map(|p| p.id).collect();
        let mut added = book.added.clone();
        for list in [&mut lost, &mut retired, &mut added] {
            let count = list.len();
            list.sort();
            list.dedup();
            if list.len() != count {
                return invalid("the revival book names a plane twice in one list");
            }
        }
        if lost
            .iter()
            .any(|plane| retired.binary_search(plane).is_ok())
        {
            return invalid("the revival book has a plane both abandoned and retired");
        }
        if book
            .retired
            .iter()
            .any(|p| p.pilot != crate::seats::Pilot::Lost)
        {
            return invalid("the revival book retired a plane someone flies");
        }
        // A root belongs to an added plane, and is itself one the mission
        // started with (slice R1).
        if book.roots.iter().any(|(plane, root)| {
            added.binary_search(plane).is_err() || added.binary_search(root).is_ok()
        }) {
            return invalid("the revival book roots a lineage in an added plane");
        }
        Ok(book)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seats::{Pilot, Plane, PlaneId, Slot};
    use tore_sim::ai::launch::{Side, WingId};
    use tore_sim::checkpoint::{Models, from_bytes, round_trip, to_bytes};

    /// A book that has lived: two wrecks abandoned (one at rest), one
    /// retired, and three planes added by revivals, one of them since lost.
    fn lived_in() -> Book {
        Book {
            lost: vec![
                LostPlane {
                    plane: PlaneId(0),
                    abandoned: 240,
                    resting_since: Some(1_812),
                },
                LostPlane {
                    plane: PlaneId(13),
                    abandoned: 9_001,
                    resting_since: None,
                },
            ],
            retired: vec![Plane {
                id: PlaneId(4),
                slot: Slot {
                    wing: WingId {
                        side: Side::Enemy,
                        index: 2,
                    },
                    member: 3,
                },
                pilot: Pilot::Lost,
            }],
            added: vec![PlaneId(12), PlaneId(13), PlaneId(14)],
            roots: [(12, 0), (13, 0), (14, 4)]
                .map(|(plane, root)| (PlaneId(plane), PlaneId(root)))
                .into(),
        }
    }

    #[test]
    fn a_book_with_abandoned_and_retired_planes_round_trips() {
        let book = lived_in();
        assert_eq!(round_trip(&book, &Models::default()).unwrap(), book);
        let empty = Book::default();
        assert_eq!(round_trip(&empty, &Models::default()).unwrap(), empty);
    }

    #[test]
    fn a_book_that_breaks_its_rules_is_refused() {
        let models = Models::default();
        let refused = |book: &Book, why: &str| {
            let coded = to_bytes(book, &models).unwrap();
            assert!(from_bytes::<Book>(&coded, &models).is_err(), "{why}");
        };
        let mut book = lived_in();
        book.lost.push(book.lost[0]);
        refused(&book, "abandoned twice");
        let mut book = lived_in();
        book.lost[1].plane = PlaneId(4);
        refused(&book, "abandoned and retired");
        let mut book = lived_in();
        book.retired[0].pilot = Pilot::Ai;
        refused(&book, "a retired plane with a pilot");
        let mut book = lived_in();
        book.added.push(PlaneId(12));
        refused(&book, "added twice");
        let mut book = lived_in();
        book.roots.insert(PlaneId(11), PlaneId(0));
        refused(&book, "a root for a plane never added");
        let mut book = lived_in();
        book.roots.insert(PlaneId(14), PlaneId(12));
        refused(&book, "an added plane as a root");
    }

    #[test]
    fn damaged_book_bytes_are_refused_without_a_panic() {
        let models = Models::default();
        let coded = to_bytes(&lived_in(), &models).unwrap();
        for cut in 0..coded.body.len() {
            let mut shorter = coded.clone();
            shorter.body.truncate(cut);
            assert!(
                from_bytes::<Book>(&shorter, &models).is_err(),
                "cut at {cut}"
            );
        }
        for bit in 0..coded.body.len() * 8 {
            let mut flipped = coded.clone();
            flipped.body[bit / 8] ^= 1 << (bit % 8);
            let _ = from_bytes::<Book>(&flipped, &models);
        }
    }
}
