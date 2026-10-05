//! The coders of craters and fires and the blast random stream
//! (docs/formats/checkpoint.md, stage H slice H3b).
//!
//! A mark is presentation the next tick reads back: its ticks run down, and a
//! fire's strength follows them. The variety rolls' stream decides the next
//! explosion's look, so it is mutable state too.

use super::{Mark, MarkKind, Rolls};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

// A crater by its original size, or a fire. Written by hand so a new variant
// fails to compile.
impl Checkpoint for MarkKind {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Self::Crater(size) => {
                s.writer().write_varint(0);
                size.save(s, None)?;
            }
            Self::Fire => s.writer().write_varint(1),
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        match l.reader().read_varint()? {
            0 => Ok(Self::Crater(u8::load(l, None)?)),
            1 => Ok(Self::Fire),
            other => invalid(format!("a mark kind has no variant {other}")),
        }
    }
}

crate::checkpoint_struct!(Mark {
    position,
    kind,
    ticks,
    born,
    serial,
});

// The xorshift state of the explosion variety rolls.
crate::checkpoint_tuple!(Rolls(state));

#[cfg(test)]
mod tests {
    use super::super::{FOREVER, vary};
    use super::*;
    use crate::checkpoint::{Coded, Models, from_bytes, round_trip};

    #[test]
    fn marks_of_every_kind_round_trip() {
        let models = Models::default();
        let marks = [
            Mark {
                position: [1234.5, 20.25, -987.0],
                kind: MarkKind::Crater(18),
                ticks: FOREVER,
                born: 77,
                serial: 3,
            },
            Mark {
                position: [-0.0, f64::NAN, f64::INFINITY],
                kind: MarkKind::Fire,
                ticks: 15 * 60 * 120 - 5,
                born: u64::MAX,
                serial: u64::MAX - 1,
            },
            Mark {
                position: [0.; 3],
                kind: MarkKind::Crater(0),
                ticks: 0,
                born: 0,
                serial: 0,
            },
        ];
        for mark in &marks {
            let copy = round_trip(mark, &models).unwrap();
            // NaN is not equal to itself: compare the bits.
            assert_eq!(copy.kind, mark.kind);
            assert_eq!(
                copy.position.map(f64::to_bits),
                mark.position.map(f64::to_bits)
            );
            assert_eq!(
                (copy.ticks, copy.born, copy.serial),
                (mark.ticks, mark.born, mark.serial)
            );
            // A fire's strength reads the restored ticks.
            assert_eq!(copy.strength(), mark.strength());
        }
    }

    #[test]
    fn a_stream_restored_mid_sequence_draws_the_same_varieties() {
        let models = Models::default();
        let mut rolls = Rolls::default();
        for _ in 0..1234 {
            rolls.roll(100);
        }
        let mut copy = round_trip(&rolls, &models).unwrap();
        assert_eq!(copy, rolls);
        for kind in [15, 18, 21, 30, 35, 17].repeat(500) {
            assert_eq!(vary(kind, &mut copy), vary(kind, &mut rolls));
        }
        assert_eq!(copy, rolls);
    }

    #[test]
    fn an_unknown_mark_kind_is_refused() {
        let models = Models::default();
        let mut s = Saver::with_models(models.clone());
        s.writer().write_varint(2);
        let coded = Coded {
            body: s.finish_section(),
            records: Vec::new(),
        };
        assert!(from_bytes::<MarkKind>(&coded, &models).is_err());
    }
}
