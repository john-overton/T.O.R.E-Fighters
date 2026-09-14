//! FA clock arithmetic and shuffled Park-Miller generator. No wall-clock/global RNG.
use crate::{Result, invalid};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeRng {
    seed: i32,
    shuffle_value: i32,
    table: [i32; 32],
}
impl NativeRng {
    pub fn seeded(seed: i32) -> Result<Self> {
        if !(1..i32::MAX).contains(&seed) {
            return Err(invalid("native RNG seed outside reviewed domain"));
        }
        Ok(Self {
            seed: -seed,
            shuffle_value: 0,
            table: [0; 32],
        })
    }
    fn step(seed: i32) -> i32 {
        let quotient = seed / -127773;
        let value = seed
            .wrapping_mul(16807)
            .wrapping_add(quotient.wrapping_mul(i32::MAX));
        if value < 0 {
            value.wrapping_add(i32::MAX)
        } else {
            value
        }
    }
    /// FA 0x4561d0; nonpositive bound returns zero without consuming state.
    pub fn below(&mut self, bound: i32) -> Result<i32> {
        if bound <= 0 {
            return Ok(0);
        }
        if self.seed <= 0 || self.shuffle_value == 0 {
            self.seed = if self.seed <= -1 {
                self.seed.wrapping_neg()
            } else {
                1
            };
            for j in (0..40).rev() {
                self.seed = Self::step(self.seed);
                if j < 32 {
                    self.table[j] = self.seed;
                }
            }
            self.shuffle_value = self.table[0];
        }
        self.seed = Self::step(self.seed);
        let index = (self.shuffle_value / 67108864) as usize;
        if index >= 32 {
            return Err(invalid("native RNG shuffle index outside table"));
        }
        self.shuffle_value = self.table[index];
        self.table[index] = self.seed;
        Ok(self.shuffle_value % bound)
    }
}
/// FA 0x486bf0 normal high-resolution clock conversion; unsigned counter domain.
pub fn counter_ticks(elapsed: u64, frequency: u64) -> Result<u32> {
    if frequency == 0 {
        return Err(invalid("zero timer frequency"));
    }
    Ok((elapsed.wrapping_shl(8) / frequency) as u32)
}
/// FA TIMEUpdate 0x486aa0, after platform wait. Supported source shift domain +/-15.
/// Pause skips simulation time; native display elapsed is a separate clock.
pub fn frame_ticks(
    raw_elapsed: i16,
    time_shift: i16,
    paused: bool,
    four_thirds: bool,
) -> Result<i16> {
    if paused || time_shift == i16::MAX {
        return Ok(0);
    }
    if !(-15..=15).contains(&time_shift) {
        return Err(invalid("native time shift outside reviewed domain"));
    }
    let mut scaled = if time_shift > 0 {
        raw_elapsed.wrapping_shl(time_shift as u32)
    } else {
        raw_elapsed >> (time_shift.wrapping_neg() as u32)
    };
    if four_thirds {
        scaled = (scaled as i32 * 4 / 3) as i16;
    }
    Ok(scaled.clamp(5, 128))
}
/// FA 0x462930: per-object elapsed is a wrapping WORD, minimum two units.
pub fn service_ticks(current: i32, last_object_tick: i16) -> i16 {
    (current as i16).wrapping_sub(last_object_tick).max(2)
}
/// Authored bridge for future 120 Hz integration, NOT the retail frame scheduler.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FixedClock {
    remainder: u16,
}
impl FixedClock {
    pub fn advance(&mut self, paused: bool) -> i16 {
        if paused {
            return 0;
        }
        self.remainder += 256;
        let elapsed = self.remainder / 120;
        self.remainder %= 120;
        elapsed as i16
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rng_matches_modular_reference_and_replays() {
        // Independent wide modular arithmetic, same specified shuffle schedule.
        let mut state = 1i64;
        let mut table = [0i64; 32];
        for j in (0..40).rev() {
            state = state * 16807 % 2147483647;
            if j < 32 {
                table[j] = state;
            }
        }
        let mut value = table[0];
        let mut rng = NativeRng::seeded(1).unwrap();
        for _ in 0..100 {
            state = state * 16807 % 2147483647;
            let index = (value / 67108864) as usize;
            value = table[index];
            table[index] = state;
            assert_eq!(rng.below(65536).unwrap(), (value % 65536) as i32);
        }
        let mut replay = rng.clone();
        for _ in 0..50 {
            assert_eq!(rng.below(256).unwrap(), replay.below(256).unwrap());
        }
        let saved = rng.clone();
        assert_eq!(rng.below(0).unwrap(), 0);
        assert_eq!(saved, rng);
    }
    #[test]
    fn clock_pause_caps_word_wrap_and_fractional_remainder() {
        assert_eq!(counter_ticks(1_000_000, 1_000_000).unwrap(), 256);
        assert_eq!(frame_ticks(6, 1, false, false).unwrap(), 12);
        assert_eq!(frame_ticks(6, 0, false, true).unwrap(), 8);
        assert_eq!(frame_ticks(1, 0, false, false).unwrap(), 5);
        assert_eq!(frame_ticks(1000, 0, false, false).unwrap(), 128);
        assert_eq!(service_ticks(65538, -2), 4);
        let mut c = FixedClock::default();
        let mut total = 0;
        for _ in 0..120 {
            total += c.advance(false) as i32;
            let before = c;
            assert_eq!(c.advance(true), 0);
            assert_eq!(c, before);
        }
        assert_eq!(total, 256);
    }
}
