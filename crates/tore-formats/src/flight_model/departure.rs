//! Stall/spin components read from FA 0x47b250..0x47ba27 and 0x47cc70..0x47cea2.
//! Callers supply envelope/VTOL gates, random tie-breaks and later movement stages.
use super::{div32, integration::service_delta, match_f24, mul_div};
use crate::{Result, invalid};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepartureProfile {
    pub warning_delay: i16,
    pub stall_delay: i16,
    pub severity: i16,
    pub pitch_down: i16,
    pub spin_entry: i16,
    pub spin_exit: i16,
    pub spin_yaw: [i16; 2],
    pub spin_aoa: [i16; 2],
    pub spin_bank: [i16; 2],
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum DepartureMode {
    #[default]
    Normal = 0,
    Warning = 1,
    Stalled = 2,
    Spinning = 3,
    ExtendedWarning = 4,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StallState {
    pub mode: DepartureMode,
    pub elapsed: i16,
}
impl StallState {
    /// Branch-local transition logic; spin entry is tested BEFORE native dispatch.
    /// `below_stall` is 0x47cc70's result, not merely AoA above a chosen angle.
    /// `initial_stall` is the current-G envelope class==1 AND below_stall.
    /// Source PT flag 0x400 enables the additional warning stage.
    pub fn advance(
        &mut self,
        p: &DepartureProfile,
        below_stall: bool,
        initial_stall: bool,
        extended_warning: bool,
        on_ground: bool,
        ticks: i16,
    ) -> Result<()> {
        if ticks < 0 {
            return Err(invalid("negative departure elapsed time"));
        }
        if on_ground {
            self.mode = DepartureMode::Normal;
        }
        match self.mode {
            DepartureMode::Normal => {
                if initial_stall {
                    self.mode = DepartureMode::Warning;
                    self.elapsed = 0;
                }
            }
            DepartureMode::Warning | DepartureMode::ExtendedWarning => {
                if !below_stall {
                    self.mode = DepartureMode::Normal;
                } else {
                    self.add_elapsed(ticks);
                    if self.elapsed >= p.warning_delay {
                        self.mode = if self.mode == DepartureMode::Warning && extended_warning {
                            DepartureMode::ExtendedWarning
                        } else {
                            DepartureMode::Stalled
                        };
                        self.elapsed = 0;
                    }
                }
            }
            DepartureMode::Stalled => {
                self.add_elapsed(ticks);
                if self.elapsed >= p.stall_delay && !below_stall {
                    self.mode = DepartureMode::Normal;
                }
            }
            DepartureMode::Spinning => {}
        }
        Ok(())
    }
    fn add_elapsed(&mut self, ticks: i16) {
        // Source tests before addition, so the timer can overshoot 0x1400.
        if self.elapsed < 0x1400 {
            self.elapsed = self.elapsed.wrapping_add(ticks);
        }
    }
}
/// FA 0x47cc70: uses floor(G) clamped to 0..2 for the stall predicate.
pub fn stall_envelope_g(g_f8: i32) -> i32 {
    (g_f8 >> 8).clamp(0, 2)
}
/// FA 0x47b287: severity ramp and deep-speed-deficit multiplier, fixed8.
pub fn stall_severity(
    profile: &DepartureProfile,
    elapsed: i16,
    speed_fps: i32,
    stall_fps: i32,
) -> Result<i32> {
    if stall_fps <= 0 {
        return Err(invalid("nonpositive native stall speed"));
    }
    let base = ((profile.severity as i32).wrapping_mul(elapsed as i32) / 1024).min(256);
    let deficit = div32(
        stall_fps
            .wrapping_sub(speed_fps.min(stall_fps))
            .wrapping_shl(9),
        stall_fps,
    )?;
    Ok(if deficit >= 256 {
        (base.wrapping_mul(deficit) / 256).min(256)
    } else {
        base
    })
}
/// FA 0x47b37e..0x47b3dd. Preserve distinct roll/rudder vs pitch divisors.
pub fn stall_authority(severity_f8: i32, controls: [i32; 3], lift_f8: i32) -> ([i32; 3], i32) {
    let remaining = 256i32.wrapping_sub(severity_f8);
    let authority = remaining.max(150);
    (
        [
            controls[0].wrapping_mul(authority) / 1024,
            controls[1].wrapping_mul(authority) / 256,
            controls[2].wrapping_mul(authority) / 1024,
        ],
        lift_f8.wrapping_mul(remaining) / 256,
    )
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpinInput {
    pub pitch_stick: i32,
    pub rudder: i32,
    pub throttle_f8: i32,
    pub speed_f8: i32,
    pub clean_stall_fps: i32,
    pub thrust_vector_f8: i32,
    pub inhibited: bool,
}
/// FA 0x47cd70. Only the exactly level/stationary tie consumes an RNG choice.
pub fn spin_direction(roll_rate_f8: i32, body_roll_pa: i16, random_choice: bool) -> i8 {
    if roll_rate_f8 > 0 {
        1
    } else if roll_rate_f8 < 0 || body_roll_pa > 0 || (body_roll_pa == 0 && random_choice) {
        -1
    } else {
        1
    }
}
/// FA 0x47ccb0. Direction supplied by spin_direction; no hidden RNG in the kernel.
pub fn spin_entry(
    p: &DepartureProfile,
    mode: DepartureMode,
    input: SpinInput,
    direction: i8,
) -> bool {
    if input.inhibited
        || !matches!(mode, DepartureMode::Warning | DepartureMode::Stalled)
        || p.spin_entry == 2
        || input.thrust_vector_f8 <= -45 * 256
        || ![-1, 1].contains(&direction)
    {
        return false;
    }
    let (rudder, pitch) = if p.spin_entry == 1 {
        (240, 128)
    } else {
        (120, 0)
    };
    input.rudder * direction as i32 >= rudder && input.pitch_stick > pitch
}
/// FA 0x47cdb0. `locked` is the latched cp flag 0x02000000.
pub fn spin_recovery(p: &DepartureProfile, input: SpinInput, direction: i8, locked: bool) -> bool {
    if locked || (input.speed_f8 >> 8) <= input.clean_stall_fps.wrapping_add(10) {
        return false;
    }
    if ![-1, 1].contains(&direction) || input.rudder * direction as i32 > -200 {
        return false;
    }
    if p.spin_exit == -2 {
        input.pitch_stick < 0
    } else {
        input.pitch_stick < -100 && input.throttle_f8 >= 50 * 256
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpinState {
    pub intensity_f8: i32,
    pub direction: i8,
    pub recovery_elapsed: i16,
    pub recovery_locked: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpinMotion {
    pub body_rates_f8: [i32; 3],
    pub movement_pitch_f8: i32,
    pub movement_roll_f8: i32,
    pub speed_f8: i32,
    pub bank_offset_f8: i32,
    pub aoa_offset_f8: i32,
    pub slip_offset_f8: i32,
}
impl SpinState {
    pub fn entered(direction: i8, recovery_locked: bool) -> Result<Self> {
        if ![-1, 1].contains(&direction) {
            return Err(invalid("spin direction must be signed unit"));
        }
        Ok(Self {
            direction,
            recovery_locked,
            ..Self::default()
        })
    }
    /// FA's spin branch only. Returns recovery completion; caller clears departure mode.
    /// Later force integration and MovePlane still run and are NOT included here.
    pub fn advance(
        &mut self,
        motion: &mut SpinMotion,
        p: &DepartureProfile,
        input: SpinInput,
        ticks: i16,
    ) -> Result<bool> {
        if ticks < 0
            || ![-1, 1].contains(&self.direction)
            || !(0..=25600).contains(&self.intensity_f8)
        {
            return Err(invalid("invalid spin state/time"));
        }
        let command = input.rudder * self.direction as i32;
        if command >= 200 {
            self.intensity_f8 = match_f24(self.intensity_f8, 100 * 256, 25 * 256, ticks);
        } else if command <= -200 {
            self.intensity_f8 = match_f24(self.intensity_f8, 0, 25 * 256, ticks);
        }
        for rate in &mut motion.body_rates_f8 {
            *rate = match_f24(*rate, 0, 90 * 256, ticks);
        }
        motion.movement_pitch_f8 = match_f24(motion.movement_pitch_f8, -88 * 256, 40 * 256, ticks);
        let blend = |ends: [i16; 2]| -> Result<i32> {
            Ok(ends[0] as i32
                + mul_div(
                    ends[1] as i32 - ends[0] as i32,
                    self.intensity_f8,
                    100 * 256,
                )?)
        };
        let yaw = blend(p.spin_yaw)? * self.direction as i32 * 256;
        motion.movement_roll_f8 = motion
            .movement_roll_f8
            .wrapping_add(service_delta(yaw, ticks));
        // Native single-turn canonicalization, 0x4768f0; ordinary tick domain.
        if motion.movement_roll_f8 > 180 * 256 {
            motion.movement_roll_f8 -= 360 * 256;
        }
        if motion.movement_roll_f8 < -180 * 256 {
            motion.movement_roll_f8 += 360 * 256;
        }
        motion.speed_f8 = match_f24(
            motion.speed_f8,
            input.clean_stall_fps.wrapping_add(110).wrapping_shl(8),
            50 * 256,
            ticks,
        );
        motion.bank_offset_f8 = match_f24(
            motion.bank_offset_f8,
            -blend(p.spin_bank)? * self.direction as i32 * 256,
            40 * 256,
            ticks,
        );
        motion.aoa_offset_f8 = match_f24(
            motion.aoa_offset_f8,
            blend(p.spin_aoa)? * 256,
            40 * 256,
            ticks,
        );
        motion.slip_offset_f8 = match_f24(motion.slip_offset_f8, 0, 40 * 256, ticks);
        if (0..=100).contains(&p.spin_exit) && (self.intensity_f8 >> 8) >= p.spin_exit as i32 {
            self.recovery_locked = true;
        }
        // Native recovery checks speed AFTER the spin branch's speed slew.
        let recovery_input = SpinInput {
            speed_f8: motion.speed_f8,
            ..input
        };
        if spin_recovery(p, recovery_input, self.direction, self.recovery_locked) {
            self.recovery_elapsed = self.recovery_elapsed.wrapping_add(ticks);
            Ok(self.recovery_elapsed as i32 >= if p.spin_exit < -1 { 256 } else { 768 })
        } else {
            self.recovery_elapsed = 0;
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const P: DepartureProfile = DepartureProfile {
        warning_delay: 512,
        stall_delay: 512,
        severity: 256,
        pitch_down: 30,
        spin_entry: 0,
        spin_exit: -1,
        spin_yaw: [10, 60],
        spin_aoa: [30, 70],
        spin_bank: [5, 20],
    };
    fn input() -> SpinInput {
        SpinInput {
            pitch_stick: 256,
            rudder: 256,
            throttle_f8: 100 * 256,
            speed_f8: 350 * 256,
            clean_stall_fps: 200,
            thrust_vector_f8: 0,
            inhibited: false,
        }
    }
    #[test]
    fn warning_delay_hysteresis_and_extended_stage() {
        let mut s = StallState::default();
        s.advance(&P, true, true, false, false, 256).unwrap();
        assert_eq!(
            s,
            StallState {
                mode: DepartureMode::Warning,
                elapsed: 0
            }
        );
        s.advance(&P, true, true, false, false, 256).unwrap();
        assert_eq!(s.mode, DepartureMode::Warning);
        s.advance(&P, true, true, true, false, 256).unwrap();
        assert_eq!(s.mode, DepartureMode::ExtendedWarning);
        s.advance(&P, true, true, true, false, 512).unwrap();
        assert_eq!(s.mode, DepartureMode::Stalled);
        s.advance(&P, false, false, false, false, 256).unwrap();
        assert_eq!(s.mode, DepartureMode::Stalled);
        s.advance(&P, false, false, false, false, 256).unwrap();
        assert_eq!(s.mode, DepartureMode::Normal);
        assert_eq!(stall_envelope_g(-1), 0);
        assert_eq!(stall_envelope_g(9 * 256), 2);
    }
    #[test]
    fn severity_ramp_deep_stall_and_distinct_authority() {
        assert_eq!(stall_severity(&P, 256, 190, 200).unwrap(), 64);
        assert_eq!(stall_severity(&P, 256, 0, 200).unwrap(), 128);
        assert_eq!(stall_severity(&P, 2048, 190, 200).unwrap(), 256);
        assert_eq!(stall_authority(256, [256; 3], 256), ([37, 150, 37], 0));
        assert!(stall_severity(&P, 1, 0, 0).is_err());
    }
    #[test]
    fn entry_thresholds_and_rng_tie_are_explicit() {
        assert!(spin_entry(&P, DepartureMode::Warning, input(), 1));
        assert!(!spin_entry(&P, DepartureMode::Normal, input(), 1));
        let mut p = P;
        p.spin_entry = 2;
        assert!(!spin_entry(&p, DepartureMode::Stalled, input(), 1));
        p.spin_entry = 1;
        let mut i = input();
        i.pitch_stick = 128;
        assert!(!spin_entry(&p, DepartureMode::Warning, i, 1));
        i.pitch_stick = 129;
        i.rudder = 239;
        assert!(!spin_entry(&p, DepartureMode::Warning, i, 1));
        i.rudder = 240;
        assert!(spin_entry(&p, DepartureMode::Warning, i, 1));
        assert_eq!(spin_direction(0, 0, false), 1);
        assert_eq!(spin_direction(0, 0, true), -1);
    }
    #[test]
    fn mirrored_spin_recovery_timer_and_latched_lock() {
        let mut outcomes = vec![];
        for direction in [-1, 1] {
            let mut state = SpinState::entered(direction, false).unwrap();
            let mut motion = SpinMotion {
                speed_f8: 350 * 256,
                ..SpinMotion::default()
            };
            let mut i = input();
            i.rudder *= direction as i32;
            state.advance(&mut motion, &P, i, 256).unwrap();
            assert_eq!(state.intensity_f8, 25 * 256);
            assert_eq!(motion.movement_pitch_f8, -40 * 256);
            outcomes.push(motion.movement_roll_f8);
            i.rudder = -200 * direction as i32;
            i.pitch_stick = -101;
            assert!(!state.advance(&mut motion, &P, i, 256).unwrap());
            i.pitch_stick = 0;
            state.advance(&mut motion, &P, i, 256).unwrap();
            assert_eq!(state.recovery_elapsed, 0);
            i.pitch_stick = -101;
            for _ in 0..2 {
                assert!(!state.advance(&mut motion, &P, i, 256).unwrap());
            }
            assert!(state.advance(&mut motion, &P, i, 256).unwrap());
            let mut p = P;
            p.spin_exit = 0;
            assert!(!state.advance(&mut motion, &p, i, 256).unwrap());
            assert!(state.recovery_locked);
        }
        assert_eq!(outcomes[0], -outcomes[1]);
        let mut p = P;
        p.spin_exit = -2;
        let mut i = input();
        i.rudder = -200;
        i.pitch_stick = -1;
        i.throttle_f8 = 0;
        assert!(spin_recovery(&p, i, 1, false));
        i.speed_f8 = 210 * 256;
        assert!(!spin_recovery(&p, i, 1, false));
    }
}
