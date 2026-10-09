//! The highest the ground reaches over each square of a host's height grid,
//! so a sight line wholly above it can skip sampling the ground.
//!
//! Agent decision, 2026-10-09: with thousands of rounds in the air, every AI
//! aircraft checks the sight line to every tracer it sees against the ground,
//! sampling it nine times. Most of those lines run far above any hill or
//! runway. This grid answers that case from a few squares. It is a shortcut
//! only: it says "clear" just when no sample could be at or below the ground,
//! so every answer is the one the samples would give.

use crate::attitude::Vector;

/// Height above the highest ground a sight line must keep to count as clear,
/// in feet: far more than any rounding of the heights or the samples.
const MARGIN_FT: f64 = 1.;

#[derive(Clone, Debug, PartialEq)]
pub struct GroundCeiling {
    /// Feet between neighbouring grid points along x and z.
    cell_feet: f64,
    /// Squares along x and along z.
    squares: [usize; 2],
    /// The highest the ground reaches over each square, row by row.
    highest: Vec<f64>,
}

impl GroundCeiling {
    /// From a grid of `cols` by `rows` points `cell_feet` apart, starting at
    /// the origin, whose ground blends the heights at each square's corners
    /// (never above the highest of them) and holds the edge squares' heights
    /// beyond the grid. `height(col, row)` is the height at a grid point.
    pub fn from_grid(
        cols: usize,
        rows: usize,
        cell_feet: f64,
        height: impl Fn(usize, usize) -> f64,
    ) -> Self {
        let squares = [cols.saturating_sub(1).max(1), rows.saturating_sub(1).max(1)];
        let point = |col: usize, row: usize| {
            height(
                col.min(cols.saturating_sub(1)),
                row.min(rows.saturating_sub(1)),
            )
        };
        let mut highest = Vec::with_capacity(squares[0] * squares[1]);
        for row in 0..squares[1] {
            for col in 0..squares[0] {
                // NaN never wins `max`; a NaN height is never above a sample.
                highest.push(
                    point(col, row)
                        .max(point(col + 1, row))
                        .max(point(col, row + 1))
                        .max(point(col + 1, row + 1)),
                );
            }
        }
        Self {
            cell_feet,
            squares,
            highest,
        }
    }

    /// Raises every square the horizontal rectangle from `lo` to `hi` (x, z)
    /// can touch to at least `height`: a runway, a deck or anything else
    /// standing on the grid.
    pub fn raise(&mut self, lo: [f64; 2], hi: [f64; 2], height: f64) {
        let Some(([x0, x1], [z0, z1])) = self.range(lo, hi) else {
            return;
        };
        for row in z0..=z1 {
            for col in x0..=x1 {
                let square = &mut self.highest[row * self.squares[0] + col];
                *square = square.max(height);
            }
        }
    }

    /// Whether the segment from `a` to `b` stays above the highest ground of
    /// every square it can be over, so no point of it is at or below the
    /// ground. `false` means only "not known": sample it.
    pub fn clear_above(&self, a: Vector, b: Vector) -> bool {
        let floor = a[1].min(b[1]);
        let Some(([x0, x1], [z0, z1])) = self.range(
            [a[0].min(b[0]), a[2].min(b[2])],
            [a[0].max(b[0]), a[2].max(b[2])],
        ) else {
            return false;
        };
        if !floor.is_finite() {
            return false;
        }
        (z0..=z1).all(|row| {
            self.highest[row * self.squares[0] + x0..=row * self.squares[0] + x1]
                .iter()
                .all(|&high| floor > high + MARGIN_FT)
        })
    }

    /// The squares a rectangle can be over, one square wider on every side
    /// than its corners say (a height lookup may round a point across a
    /// square's edge), clamped to the grid as the lookups clamp points.
    /// `None` when a value is not finite.
    fn range(&self, lo: [f64; 2], hi: [f64; 2]) -> Option<([usize; 2], [usize; 2])> {
        if !lo.iter().chain(&hi).all(|v| v.is_finite())
            || !self.cell_feet.is_finite()
            || self.cell_feet <= 0.
        {
            return None;
        }
        let index = |value: f64, axis: usize| {
            let last = self.squares[axis] - 1;
            let square = (value / self.cell_feet).floor();
            if square <= 0. {
                0
            } else if square >= last as f64 {
                last
            } else {
                square as usize
            }
        };
        let span = |axis: usize| {
            [
                index(lo[axis], axis).saturating_sub(1),
                (index(hi[axis], axis) + 1).min(self.squares[axis] - 1),
            ]
        };
        Some((span(0), span(1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> GroundCeiling {
        // 4 by 3 points 1,000 ft apart; a 900 ft hill at (2, 1).
        GroundCeiling::from_grid(
            4,
            3,
            1000.,
            |col, row| {
                if (col, row) == (2, 1) { 900. } else { 100. }
            },
        )
    }

    #[test]
    fn a_line_above_the_hill_is_clear_and_one_through_it_is_not() {
        let g = grid();
        assert!(g.clear_above([0., 902., 0.], [3000., 950., 2000.]));
        assert!(!g.clear_above([0., 900.5, 0.], [3000., 950., 2000.]));
        // Far from the hill, a low line over flat ground is clear only
        // where the hill's squares (widened by one) are out of reach.
        let wide =
            GroundCeiling::from_grid(20, 3, 1000., |col, _| if col == 2 { 900. } else { 100. });
        assert!(wide.clear_above([10_000., 200., 500.], [15_000., 200., 900.]));
        assert!(!wide.clear_above([1_000., 200., 500.], [1_500., 200., 900.]));
    }

    #[test]
    fn beyond_the_grid_counts_its_edge_squares() {
        let g = grid();
        assert!(!g.clear_above([-50_000., 500., -50_000.], [50_000., 500., 50_000.]));
        assert!(g.clear_above([-50_000., 1000., -50_000.], [50_000., 1000., 50_000.]));
    }

    #[test]
    fn a_raised_rectangle_and_odd_values_are_never_clear() {
        let mut g = grid();
        g.raise([0., 0.], [10., 10.], 5000.);
        assert!(!g.clear_above([0., 1000., 0.], [3000., 1000., 2000.]));
        assert!(!g.clear_above([f64::NAN, 1000., 0.], [3000., 1000., 2000.]));
        assert!(!g.clear_above([0., f64::NAN, 0.], [3000., 1000., 2000.]));
    }
}
