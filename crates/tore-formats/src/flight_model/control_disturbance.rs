//! Native control-side offset state at 0x47bcb2..0x47c0a2 and selector 0x47af70.
//! Direction-code producers remain caller-owned. This is not a new buffet force law.
use super::{match_f24, stick_input};
use crate::{Result, invalid};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis {
    pub direction: i16,
    pub enabled: bool,
    pub offset_f8: i32,
    pub previous: i16,
    pub started: i32,
}
impl Default for Axis {
    fn default() -> Self {
        Self {
            direction: 0,
            enabled: false,
            offset_f8: 0,
            previous: -1,
            started: 0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub pitch: Axis,
    pub heading: Axis,
    pub quiet_since: i32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            pitch: Axis::default(),
            heading: Axis::default(),
            quiet_since: -1,
        }
    }
}
impl State {
    /// 0x47af70. Positive first selector is rejected while touching ground.
    pub fn select(&mut self, pitch: i16, heading: i16, touching: bool) {
        if pitch > 0 && touching {
            return;
        }
        for (a, code) in [(&mut self.pitch, pitch), (&mut self.heading, heading)] {
            a.enabled = code != 0 && (a.direction != 0 || code != -1);
            if a.direction != 0 || code != -1 {
                a.direction = code;
            }
        }
    }
    /// Native normal-control block. Returns offsets in compositor order [heading,pitch].
    #[allow(clippy::too_many_arguments)]
    pub fn advance(
        &mut self,
        now: i32,
        ticks: i16,
        throttle_f8: i32,
        speed_f8: i32,
        minimum_speed: i16,
        touching: bool,
        rate_shift: u8,
    ) -> Result<[i32; 2]> {
        if ticks < 0 {
            return Err(invalid("negative control disturbance time"));
        }
        let amount = (((throttle_f8 >> 8).wrapping_mul(100).wrapping_sub(3500) / 60).clamp(0, 100)
            * 110)
            / 100;
        let amount = if (speed_f8 >> 8) < minimum_speed as i32 || touching {
            0
        } else {
            amount
        };
        let shift = rate_shift & 31;
        for index in 0..2 {
            let mut cancel = false;
            let (a, negative, positive, divisor) = if index == 0 {
                (&mut self.pitch, 3, 4, 3)
            } else {
                (&mut self.heading, 1, 2, 2)
            };
            if a.enabled {
                if a.direction == negative || a.direction == positive {
                    let command = if a.direction == negative {
                        -amount
                    } else {
                        amount
                    };
                    a.offset_f8 = stick_input(
                        a.offset_f8,
                        75 * 256,
                        0,
                        -75 * 256,
                        25 >> shift,
                        38 >> shift,
                        command,
                        ticks,
                    )?;
                }
            } else {
                a.offset_f8 = match_f24(
                    a.offset_f8,
                    -(a.offset_f8 / divisor),
                    (40i32 >> shift) << 8,
                    ticks,
                );
            }
            if a.direction == 0 {
                a.previous = -1;
            } else if a.direction == negative || a.direction == positive {
                // Native TEST with 0xffffff00 is signed: negative values stay negative.
                let value = a.offset_f8 & !255;
                let crossed = if a.direction == negative {
                    value > 0
                } else {
                    value < 0
                };
                if crossed {
                    if a.previous == -1 {
                        a.previous = a.direction;
                        a.started = now;
                    } else if a.previous == a.direction && a.started.wrapping_add(64) < now {
                        cancel = true;
                    }
                }
            } else if a.direction == -1 {
                a.started = now;
            }
            if cancel {
                self.select(-1, 0, false);
            }
        }
        if self.pitch.offset_f8.wrapping_abs() < 512 && self.heading.offset_f8.wrapping_abs() < 512
        {
            if self.quiet_since == -1 {
                if (self.pitch.offset_f8 & !255) != 0 || (self.heading.offset_f8 & !255) != 0 {
                    self.quiet_since = now;
                }
            } else if self.quiet_since.wrapping_add(1280) < now {
                self.select(0, 0, false);
                self.quiet_since = -1;
            }
        } else {
            self.quiet_since = -1;
        }
        Ok([self.heading.offset_f8, self.pitch.offset_f8])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direction_selection_ground_inhibition_and_release() {
        let mut s = State::default();
        s.select(3, 2, true);
        assert_eq!(s, State::default());
        s.select(3, 2, false);
        let out = s
            .advance(0, 256, 95 * 256, 500 * 256, 300, false, 0)
            .unwrap();
        assert!(out[0] > 0 && out[1] < 0);
        s.select(0, 0, false);
        s.advance(1, 256, 0, 500 * 256, 300, false, 0).unwrap();
        assert!(s.pitch.offset_f8 >= 0 && s.heading.offset_f8 <= 0);
        let mut zero = State::default();
        zero.select(-1, -1, false);
        assert_eq!(zero, State::default());
    }
    #[test]
    fn strict_quiet_timeout_and_speed_gate() {
        let mut s = State {
            quiet_since: 0,
            ..Default::default()
        };
        s.select(3, 2, false);
        s.advance(1280, 0, 95 * 256, 100 * 256, 300, false, 0)
            .unwrap();
        assert!(s.pitch.enabled);
        s.advance(1281, 0, 95 * 256, 100 * 256, 300, false, 0)
            .unwrap();
        assert!(!s.pitch.enabled);
        assert_eq!((s.pitch.offset_f8, s.heading.offset_f8), (0, 0));
    }
}
