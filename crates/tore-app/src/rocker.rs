//! The retail vertical rocker (PREV/NEXT, fuel): five imported frames,
//! tilted while pressed. Retail frame order: docs/formats/debrief.md.
use std::time::{Duration, Instant};

/// A vertical rocker's five ROCKER0n frames: 2 is level, 0 and 4 hold the
/// top (previous) and bottom (next) halves down. Retail steps one frame at a
/// time toward the pose and acts on press.
#[derive(Clone, Copy, Debug)]
pub struct Rocker {
    frame: usize,
    target: usize,
    /// The mouse still holds the rocker down.
    held: bool,
    next: Instant,
}
impl Rocker {
    pub const LEVEL: usize = 2;
    /// Fitted: retail waits one screen update per frame.
    pub const FRAME: Duration = Duration::from_millis(40);
    /// The sprite for the current frame.
    pub fn sprite(&self) -> String {
        format!("ROCKER0{}.PIC", self.frame)
    }
    pub fn held(&self) -> bool {
        self.held
    }
    pub fn new() -> Self {
        Self {
            frame: Self::LEVEL,
            target: Self::LEVEL,
            held: false,
            next: Instant::now(),
        }
    }
    pub fn push(&mut self, forward: bool, held: bool, now: Instant) {
        if self.frame == self.target {
            self.next = now + Self::FRAME;
        }
        self.target = if forward { 4 } else { 0 };
        self.held = held;
    }
    pub fn release(&mut self, now: Instant) {
        if self.frame == self.target {
            self.next = now + Self::FRAME;
        }
        self.held = false;
        self.target = Self::LEVEL;
    }
    /// Steps toward the pose; a push that is not held springs back once it
    /// lands. Returns true while still moving.
    pub fn advance(&mut self, now: Instant) -> bool {
        while self.frame != self.target && now >= self.next {
            self.frame = if self.frame < self.target {
                self.frame + 1
            } else {
                self.frame - 1
            };
            self.next += Self::FRAME;
            if self.frame == self.target && !self.held && self.target != Self::LEVEL {
                self.target = Self::LEVEL;
            }
        }
        self.frame != self.target
    }
}
impl Default for Rocker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rocker_tilts_one_frame_at_a_time_holds_and_springs_back() {
        let start = Instant::now();
        let at = |frames: u32| start + Rocker::FRAME * frames;
        let mut rocker = Rocker::new();
        assert_eq!(rocker.frame, Rocker::LEVEL);
        // Held on the bottom half: 3, then 4, then it stays down.
        rocker.push(true, true, start);
        assert!(rocker.advance(at(1)));
        assert_eq!(rocker.frame, 3);
        assert!(!rocker.advance(at(2)));
        assert_eq!(rocker.frame, 4);
        assert!(!rocker.advance(at(9)));
        assert_eq!(rocker.frame, 4);
        // Released: back through 3 to level.
        rocker.release(at(9));
        rocker.advance(at(10));
        assert_eq!(rocker.frame, 3);
        assert!(!rocker.advance(at(11)));
        assert_eq!(rocker.frame, Rocker::LEVEL);
        // A key or clipboard click taps the top half and springs back.
        rocker.push(false, false, at(20));
        assert!(rocker.advance(at(22)));
        assert_eq!(rocker.frame, 0);
        assert!(!rocker.advance(at(24)));
        assert_eq!(rocker.frame, Rocker::LEVEL);
    }
}
