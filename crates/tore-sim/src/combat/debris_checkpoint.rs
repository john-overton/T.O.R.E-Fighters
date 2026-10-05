//! The coders of falling debris (docs/formats/checkpoint.md, stage H slice
//! H3b): a detached piece of a destroyed aircraft, with the attitude its
//! tumble has reached. The list that holds the pieces is `live::State`'s.

use super::Piece;

crate::checkpoint_struct!(Piece {
    owner,
    variant,
    position,
    velocity,
    basis,
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attitude::Basis;
    use crate::checkpoint::{Models, round_trip};

    /// A piece that has fallen and tumbled for a while.
    fn piece(owner: u32, variant: usize) -> Piece {
        let mut piece = Piece::new(
            owner,
            variant,
            [1500.5, 9000., -300.25],
            [80., 10., 300.],
            Basis::new(0.3, -0.2, 1.1),
            [3., 0.5, 20.],
        );
        let ground = |x: f64, _| x * 0.1;
        for _ in 0..200 {
            assert!(piece.step(&ground).is_none());
        }
        piece
    }

    #[test]
    fn a_tumbling_piece_round_trips_and_lands_where_the_original_does() {
        let models = Models::default();
        let ground = |x: f64, _| x * 0.1;
        for (owner, variant) in [(0, 0), (7, 1), (u32::MAX, 0)] {
            let mut original = piece(owner, variant);
            let mut copy = round_trip(&original, &models).unwrap();
            assert_eq!(copy, original);
            for tick in 0..2400 {
                let (a, b) = (original.step(&ground), copy.step(&ground));
                assert_eq!(a, b, "tick {tick}");
                assert_eq!(copy, original, "tick {tick}");
                if a.is_some() {
                    break;
                }
            }
        }
    }
}
