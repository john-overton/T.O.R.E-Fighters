//! The coders of the mission ledger: launches in flight, aims waiting for
//! their round, tallies, credited kills and the last shooter to hurt each
//! target (docs/formats/checkpoint.md).
//!
//! Everything the debrief and the score read is coded. The one field skipped
//! is `outcomes`, a why-record: the host drains it into the tick output at
//! the end of every step, and nothing in flight reads it back.

use super::{Key, Kill, Ledger, ShotKind, Tally};
use std::collections::VecDeque;

crate::checkpoint_enum!(ShotKind {
    AirToAir = 0,
    AirToGround = 1,
    Gun = 2,
    Bomb = 3,
    Other = 4,
});

crate::checkpoint_struct!(Key { owner, aim, kind });

crate::checkpoint_struct!(Tally {
    launched,
    hit,
    damage,
    missed,
    spoofed,
    jammed,
});

crate::checkpoint_struct!(Kill {
    owner,
    victim,
    category,
    aircraft,
});

crate::checkpoint_struct!(Ledger {
    open,
    decoyed,
    aims,
    tallies,
    kills,
    last_hit,
    uncredited,
} skip {
    // Why-record: drained into the tick output every step, never read back.
    outcomes = VecDeque::new(),
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::{Models, from_bytes, round_trip, to_bytes};
    use crate::combat::ledger::Resolution;

    fn busy() -> Ledger {
        let mut ledger = Ledger::default();
        ledger.aim(11, 4);
        ledger.launch(1, 0, Some(7), ShotKind::AirToAir);
        ledger.launch(2, 0, Some(7), ShotKind::AirToAir);
        ledger.launch(3, 5, None, ShotKind::Gun);
        ledger.launch(4, 5, Some(0), ShotKind::Bomb);
        ledger.launch(5, 6, None, ShotKind::AirToGround);
        ledger.launch(6, 6, None, ShotKind::Other);
        ledger.resolve(1, Resolution::Spoofed);
        ledger.resolve(2, Resolution::Hit(120));
        ledger.resolve(3, Resolution::Missed);
        ledger.resolve(4, Resolution::Jammed);
        let kill = Kill {
            owner: 0,
            victim: 7,
            category: 0x2000,
            aircraft: true,
        };
        ledger.damaged(kill);
        ledger.kill(kill);
        ledger.damaged(Kill {
            owner: 5,
            victim: 9,
            category: 0x100,
            aircraft: false,
        });
        ledger.lose_without_credit(12);
        ledger
    }

    #[test]
    fn a_ledger_round_trips_without_its_why_record() {
        let models = Models::default();
        let mut ledger = busy();
        let outcomes = ledger.take_outcomes();
        assert_eq!(outcomes.len(), 4, "the fixture resolved four shots");
        let copy = round_trip(&ledger, &models).unwrap();
        assert_eq!(copy, ledger);
        // Open rounds, aims, tallies, kills and credit all came back.
        assert_eq!(copy.kills(), ledger.kills());
        assert_eq!(copy.credit(9), ledger.credit(9));
        assert_eq!(copy.credit(12), None);
        assert_eq!(
            copy.total(|k| k.kind == ShotKind::AirToAir),
            ledger.total(|k| k.kind == ShotKind::AirToAir)
        );
        // The open rounds still resolve once after a restore.
        let mut a = ledger.clone();
        let mut b = copy;
        for ledger in [&mut a, &mut b] {
            ledger.resolve(5, Resolution::Hit(30));
            ledger.resolve(5, Resolution::Missed);
            // The spoofed missile is remembered and strikes late.
            ledger.resolve(1, Resolution::Hit(60));
            ledger.launch(11, 0, None, ShotKind::Gun);
        }
        assert_eq!(a, b);
        assert_eq!(a.take_outcomes(), b.take_outcomes());
    }

    #[test]
    fn the_outcomes_are_not_coded() {
        let models = Models::default();
        let with = busy();
        let mut without = with.clone();
        without.take_outcomes();
        assert_eq!(
            to_bytes(&with, &models).unwrap(),
            to_bytes(&without, &models).unwrap()
        );
        let copy = round_trip(&with, &models).unwrap();
        assert_ne!(copy, with, "the restored ledger holds no outcomes");
        let mut copy = copy;
        assert!(copy.take_outcomes().is_empty());
    }

    #[test]
    fn a_cut_ledger_is_refused() {
        let models = Models::default();
        let coded = to_bytes(&busy(), &models).unwrap();
        for cut in 0..coded.body.len() {
            let mut short = coded.clone();
            short.body.truncate(cut);
            assert!(from_bytes::<Ledger>(&short, &models).is_err(), "cut {cut}");
        }
    }
}
