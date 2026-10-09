//! The AC-130 gunsight's slew command: the keyboard and button holds and
//! the analog axes, combined once per tick into the seat input's
//! `sight: [i8; 2]` (right and up positive).
//!
//! The ramp is the tap-then-rate idea of the trim keys ([`crate::trim_keys`]):
//! a held key or button starts at a quarter rate for fine nudges and goes to
//! full rate after a quarter second. Opinionated (John, 2026-10-09); the
//! 0.25 s and quarter-rate numbers are fitted (agent). The host turns the
//! deflection into degrees using the zoom step, so single player and
//! multiplayer slew alike.

/// Ticks a held direction runs at [`FINE_SCALE`] before the full rate: a
/// quarter second at the fixed 120 Hz.
pub const FINE_TICKS: u32 = 30;
/// The deflection share of the first [`FINE_TICKS`] of a hold.
pub const FINE_SCALE: f64 = 0.25;
/// Full deflection in the seat input.
pub const FULL: i8 = 127;
/// Zoom steps of the sight, 1 (widest) to 6 (narrowest).
pub const ZOOM_STEPS: u8 = 6;
/// The default zoom step, the plan's 7.5 degree field of view.
pub const ZOOM_DEFAULT: u8 = 3;

/// A direction of the held slew, in [`SightSlew::step`]'s order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Debug, Default)]
pub struct SightSlew {
    held: [u32; 4],
}
impl SightSlew {
    /// One tick. `holds` is whether each of left, right, up and down is
    /// held; `analog` is the `sight-x` and `sight-y` axes. The result is
    /// each axis's deflection, -1 to 1.
    pub fn step(&mut self, holds: [bool; 4], analog: [f64; 2]) -> [f64; 2] {
        let mut ramped = [0.; 4];
        for (i, on) in holds.into_iter().enumerate() {
            if on {
                let ticks = self.held[i];
                self.held[i] = ticks.saturating_add(1);
                ramped[i] = if ticks < FINE_TICKS { FINE_SCALE } else { 1. };
            } else {
                self.held[i] = 0;
            }
        }
        let axis = |analog: f64, negative: f64, positive: f64| {
            let analog = if analog.is_finite() { analog } else { 0. };
            (analog + positive - negative).clamp(-1., 1.)
        };
        [
            axis(
                analog[0],
                ramped[Hold::Left as usize],
                ramped[Hold::Right as usize],
            ),
            axis(
                analog[1],
                ramped[Hold::Down as usize],
                ramped[Hold::Up as usize],
            ),
        ]
    }
    /// Forget any hold in progress (a pause, a menu, a lost window).
    pub fn release(&mut self) {
        self.held = [0; 4];
    }
}

/// A deflection as the seat input carries it: -127 to 127.
pub fn deflection(value: [f64; 2]) -> [i8; 2] {
    value.map(|v| {
        if v.is_finite() {
            (v.clamp(-1., 1.) * f64::from(FULL)).round() as i8
        } else {
            0
        }
    })
}

/// The zoom step after `delta` presses of zoom in (positive) or out
/// (negative), kept within 1 to [`ZOOM_STEPS`].
pub fn zoom_step(step: u8, delta: i8) -> u8 {
    (i16::from(step) + i16::from(delta)).clamp(1, i16::from(ZOOM_STEPS)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_key_nudges_for_a_quarter_second_then_runs_full() {
        let mut slew = SightSlew::default();
        let hold = [false, true, false, false];
        for _ in 0..FINE_TICKS {
            assert_eq!(slew.step(hold, [0.; 2]), [FINE_SCALE, 0.]);
        }
        assert_eq!(slew.step(hold, [0.; 2]), [1., 0.]);
        assert_eq!(slew.step(hold, [0.; 2]), [1., 0.]);
        // Letting go and pressing again starts the nudge over.
        assert_eq!(slew.step([false; 4], [0.; 2]), [0.; 2]);
        assert_eq!(slew.step(hold, [0.; 2]), [FINE_SCALE, 0.]);
    }

    #[test]
    fn opposite_holds_cancel_and_directions_have_their_signs() {
        let mut slew = SightSlew::default();
        assert_eq!(slew.step([true, true, false, false], [0.; 2]), [0.; 2]);
        let mut slew = SightSlew::default();
        let [x, y] = slew.step([true, false, true, false], [0.; 2]);
        assert!(x < 0. && y > 0., "left is negative x, up is positive y");
        let [x, y] = slew.step([false, true, false, true], [0.; 2]);
        assert!(x > 0. && y < 0.);
    }

    #[test]
    fn an_analog_axis_is_proportional_and_adds_to_the_holds() {
        let mut slew = SightSlew::default();
        assert_eq!(slew.step([false; 4], [0.4, -0.7]), [0.4, -0.7]);
        let [x, _] = slew.step([false, true, false, false], [0.9, 0.]);
        assert_eq!(x, 1., "the sum is clamped to full deflection");
        assert_eq!(slew.step([false; 4], [f64::NAN, 0.]), [0.; 2]);
    }

    #[test]
    fn deflection_quantizes_symmetrically_and_clamps() {
        assert_eq!(deflection([1., -1.]), [127, -127]);
        assert_eq!(deflection([0., 0.5]), [0, 64]);
        assert_eq!(deflection([2., f64::NAN]), [127, 0]);
        assert_eq!(deflection([FINE_SCALE, 0.]), [32, 0]);
    }

    #[test]
    fn the_zoom_step_stops_at_each_end_of_the_ladder() {
        assert_eq!(zoom_step(ZOOM_DEFAULT, 1), 4);
        assert_eq!(zoom_step(6, 1), 6);
        assert_eq!(zoom_step(1, -1), 1);
        assert_eq!(zoom_step(3, -1), 2);
    }
}
