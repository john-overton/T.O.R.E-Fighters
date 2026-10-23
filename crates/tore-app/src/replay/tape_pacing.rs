//! The HUD camera's tape plays at 24 pictures a second, as a videotape
//! recording of a film-rate camera would (opinionated, John, 2026-10-19: "it
//! should run at 24 fps capped (because it's a VHS tape)").
//!
//! This is a presentation rate, not a playback rate: the replay clock keeps
//! running at the speed chosen and the controls, cursor and menus are drawn
//! every frame at the window's own rate. Only the tape layer (the world
//! picture, the HUD, the recorder's lines and the tape effects) is made 24
//! times a second and shown again in between.
//!
//! Nothing here reads or feeds the simulation or the recording.
use std::time::{Duration, Instant};

/// Pictures a second on the tape.
pub const TAPE_FPS: u32 = 24;

/// How long one tape picture is held.
pub const PICTURE: Duration = Duration::from_nanos(1_000_000_000 / TAPE_FPS as u64);

/// Decides which window frames make a new tape picture. Tape pictures fall
/// on a fixed 24 Hz grid from the first one, so the rhythm does not drift
/// with the window's frame times (at 60 frames a second it is the familiar
/// three-two pattern).
#[derive(Clone, Copy, Debug, Default)]
pub struct Pacer {
    /// Time of the first picture and the number of the last one made.
    last: Option<(Instant, u64)>,
}

impl Pacer {
    /// True when the frame at `now` should make a new tape picture. A frame
    /// that falls in the same 24 Hz slot as the last picture is a held one.
    /// The first call is always due, and so is the call after
    /// [`Pacer::restart`].
    pub fn due(&mut self, now: Instant) -> bool {
        let Some((origin, number)) = self.last else {
            self.last = Some((now, 0));
            return true;
        };
        let slot = (now.saturating_duration_since(origin).as_nanos() / PICTURE.as_nanos()) as u64;
        if slot == number {
            return false;
        }
        // A long gap (a stall, a hidden window) starts the grid again rather
        // than catching up with pictures nobody saw.
        if slot > number + 1 + u64::from(TAPE_FPS) {
            self.last = Some((now, 0));
        } else {
            self.last = Some((origin, slot));
        }
        true
    }

    /// The next frame makes a new picture: the view changed.
    pub fn restart(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(origin: Instant, ms: u64) -> Instant {
        origin + Duration::from_millis(ms)
    }

    #[test]
    fn the_first_frame_makes_a_picture_and_the_next_in_its_slot_hold_it() {
        let t0 = Instant::now();
        let mut pacer = Pacer::default();
        assert!(pacer.due(t0));
        assert!(!pacer.due(at(t0, 5)));
        assert!(!pacer.due(at(t0, 41)));
        assert!(pacer.due(at(t0, 42)));
        assert!(!pacer.due(at(t0, 60)));
    }

    #[test]
    fn sixty_frames_a_second_make_twenty_four_pictures() {
        let t0 = Instant::now();
        let mut pacer = Pacer::default();
        let made = (0..60)
            .filter(|n| pacer.due(t0 + Duration::from_nanos(n * 1_000_000_000 / 60)))
            .count();
        assert_eq!(made, 24);
    }

    #[test]
    fn two_hundred_frames_a_second_still_make_twenty_four() {
        let t0 = Instant::now();
        let mut pacer = Pacer::default();
        let made = (0..400)
            .filter(|n| pacer.due(t0 + Duration::from_micros(n * 5000)))
            .count();
        assert_eq!(made, 48);
    }

    #[test]
    fn slow_frames_make_a_picture_each() {
        let t0 = Instant::now();
        let mut pacer = Pacer::default();
        let made = (0..10)
            .filter(|n| pacer.due(t0 + Duration::from_millis(n * 100)))
            .count();
        assert_eq!(made, 10);
    }

    #[test]
    fn the_grid_does_not_drift_with_uneven_frames() {
        let t0 = Instant::now();
        let mut pacer = Pacer::default();
        assert!(pacer.due(t0));
        // Frames at 30 and 60 ms: the second is in the second slot (41.7 to
        // 83.3 ms), the first is still in the first.
        assert!(!pacer.due(at(t0, 30)));
        assert!(pacer.due(at(t0, 60)));
        assert!(!pacer.due(at(t0, 80)));
        assert!(pacer.due(at(t0, 84)));
    }

    #[test]
    fn restart_and_long_gaps_begin_again() {
        let t0 = Instant::now();
        let mut pacer = Pacer::default();
        assert!(pacer.due(t0));
        pacer.restart();
        assert!(pacer.due(at(t0, 1)));
        assert!(!pacer.due(at(t0, 10)));
        assert!(pacer.due(at(t0, 10_000)));
        assert!(!pacer.due(at(t0, 10_020)));
        assert!(pacer.due(at(t0, 10_045)));
    }
}
