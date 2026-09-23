//! Pilot G effects: blackout, redout and high-G view shake. Behaviour and the
//! proposed (fitted) numbers: docs/spec/cheats.md#g-effects.
use crate::flight::DT;

/// Sustained positive G above this starts to black the view out.
pub const BLACKOUT_ONSET_G: f64 = 6.;
/// Seconds above the onset before blackout begins: 5 just over 6 G (John,
/// 2026-09-23), one second less per extra G (proposed), never under 1.
pub const BLACKOUT_DELAY_SECONDS: f64 = 5.;
pub const BLACKOUT_DELAY_PER_G: f64 = 1.;
pub const BLACKOUT_MINIMUM_DELAY_SECONDS: f64 = 1.;
/// Seconds from the first darkening to full blackout at 9 G; higher G is
/// proportionally faster.
pub const BLACKOUT_SECONDS_AT_9G: f64 = 5.;
/// Sustained negative G below this starts to red the view out.
pub const REDOUT_ONSET_G: f64 = -2.;
/// Seconds below the onset before redout begins (John, 2026-09-23).
pub const REDOUT_DELAY_SECONDS: f64 = 3.;
/// Seconds from the first reddening to full redout at -3.5 G.
pub const REDOUT_SECONDS_AT_MINUS_3_5G: f64 = 3.;
/// Seconds for vision to clear from full once G is back inside the limits.
pub const RECOVERY_SECONDS: f64 = 3.;
/// Shake starts at this G and reaches full strength at [`SHAKE_FULL_G`].
pub const SHAKE_ONSET_G: f64 = 6.;
pub const SHAKE_FULL_G: f64 = 9.;
/// Full-strength view shake, radians: 4 pixels at 640 by 480 with the default
/// 60 degree vertical view (240 times root 3 pixels per radian).
pub const SHAKE_RADIANS: f64 = 0.0096;

/// Vision loss from 0 (clear) to 1 (fully black or red). The pilot keeps
/// control throughout (John, 2026-09-23).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GEffects {
    pub blackout: f64,
    pub redout: f64,
    /// How much of the onset delay has been used, 0 to 1. It drains over the
    /// recovery time inside the limits, so a short unload does not reset it.
    blackout_strain: f64,
    redout_strain: f64,
}
/// Seconds of sustained `g` before blackout begins.
pub fn blackout_delay(g: f64) -> f64 {
    (BLACKOUT_DELAY_SECONDS - (g - BLACKOUT_ONSET_G) * BLACKOUT_DELAY_PER_G)
        .max(BLACKOUT_MINIMUM_DELAY_SECONDS)
}
impl GEffects {
    /// One fixed simulation step at load factor `g`. Disabled clears at once.
    pub fn step(&mut self, g: f64, enabled: bool) {
        if !enabled {
            *self = Self::default();
            return;
        }
        let recover = DT / RECOVERY_SECONDS;
        let rate_per_9g = 1. / (BLACKOUT_SECONDS_AT_9G * (9. - BLACKOUT_ONSET_G));
        if g > BLACKOUT_ONSET_G {
            if self.blackout_strain < 1. {
                self.blackout_strain += DT / blackout_delay(g);
            } else {
                self.blackout += (g - BLACKOUT_ONSET_G) * rate_per_9g * DT;
            }
        } else {
            self.blackout_strain -= recover;
            self.blackout -= recover;
        }
        let rate_per_negative = 1. / (REDOUT_SECONDS_AT_MINUS_3_5G * (REDOUT_ONSET_G - -3.5));
        if g < REDOUT_ONSET_G {
            if self.redout_strain < 1. {
                self.redout_strain += DT / REDOUT_DELAY_SECONDS;
            } else {
                self.redout += (REDOUT_ONSET_G - g) * rate_per_negative * DT;
            }
        } else {
            self.redout_strain -= recover;
            self.redout -= recover;
        }
        for v in [
            &mut self.blackout,
            &mut self.redout,
            &mut self.blackout_strain,
            &mut self.redout_strain,
        ] {
            *v = v.clamp(0., 1.);
        }
    }
    /// Darkening at a point `radius` from the view centre (0 centre, 1 corner):
    /// the edges go first, like tunnel vision, and all of it at full loss.
    pub fn coverage(level: f64, radius: f64) -> f64 {
        (level * 1.5 - 0.5 * (1. - radius.clamp(0., 1.))).clamp(0., 1.)
    }
}

/// View shake as [yaw, pitch] radians at simulation time `seconds`. Two
/// incommensurate smooth noise rates (13 and 17 Hz) keep it irregular.
pub fn shake(g: f64, seconds: f64) -> [f64; 2] {
    let x = ((g - SHAKE_ONSET_G) / (SHAKE_FULL_G - SHAKE_ONSET_G)).clamp(0., 1.);
    if x == 0. {
        return [0.; 2];
    }
    let strength = SHAKE_RADIANS * x * x * (3. - 2. * x);
    [0, 1].map(|axis| {
        let a = noise(seconds * 13., axis * 2);
        let b = noise(seconds * 17., axis * 2 + 1);
        strength * (0.6 * a + 0.4 * b)
    })
}

/// Smooth value noise in -1..1, deterministic in `t` and `stream`.
fn noise(t: f64, stream: u64) -> f64 {
    let cell = t.floor();
    let f = t - cell;
    let blend = f * f * (3. - 2. * f);
    let at = |n: f64| {
        let mut h = (n as i64 as u64) ^ stream.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        h = (h ^ (h >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
        h = (h ^ (h >> 33)).wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        h ^= h >> 33;
        (h >> 11) as f64 / (1u64 << 53) as f64 * 2. - 1.
    };
    at(cell) * (1. - blend) + at(cell + 1.) * blend
}

#[cfg(test)]
mod tests {
    use super::*;
    fn seconds_to_full(g: f64) -> f64 {
        let mut e = GEffects::default();
        let mut ticks = 0;
        while e.blackout < 1. && e.redout < 1. && ticks < 120 * 60 {
            e.step(g, true);
            ticks += 1;
        }
        ticks as f64 * DT
    }
    fn seconds_to_first(g: f64) -> f64 {
        let mut e = GEffects::default();
        let mut ticks = 0;
        while e.blackout == 0. && e.redout == 0. && ticks < 120 * 60 {
            e.step(g, true);
            ticks += 1;
        }
        ticks as f64 * DT
    }
    #[test]
    fn onset_waits_five_seconds_just_over_six_g_less_when_pulling_harder() {
        assert!((seconds_to_first(6.01) - 5.).abs() < 0.03);
        assert!((seconds_to_first(7.) - 4.).abs() < 0.03);
        assert!((seconds_to_first(9.) - 2.).abs() < 0.03);
        assert!((seconds_to_first(12.) - 1.).abs() < 0.03);
        assert!((seconds_to_first(-2.5) - 3.).abs() < 0.03);
        assert!((seconds_to_first(-6.) - 3.).abs() < 0.03);
        // Half the delay, a second's unload, then the rest is shorter than new.
        let mut e = GEffects::default();
        for _ in 0..300 {
            e.step(6.5, true);
        }
        for _ in 0..120 {
            e.step(1., true);
        }
        let mut ticks = 0;
        while e.blackout == 0. {
            e.step(6.5, true);
            ticks += 1;
        }
        let resumed = ticks as f64 * DT;
        assert!(resumed > 2.25 && resumed < 4.5);
    }
    #[test]
    fn blackout_and_redout_follow_the_proposed_timings() {
        assert!((seconds_to_full(9.) - 7.).abs() < 0.03);
        assert!(seconds_to_full(10.5) < 7.);
        assert!((seconds_to_full(-3.5) - 6.).abs() < 0.03);
        let mut e = GEffects::default();
        for _ in 0..1200 {
            e.step(5.9, true);
            e.step(-1.9, true);
        }
        assert_eq!(e, GEffects::default(), "inside the limits nothing happens");
    }
    #[test]
    fn vision_recovers_in_three_seconds_and_the_cheat_clears_it() {
        let mut e = GEffects {
            blackout: 1.,
            ..Default::default()
        };
        for _ in 0..(RECOVERY_SECONDS * 120.) as usize - 1 {
            e.step(1., true);
        }
        assert!(e.blackout > 0. && e.blackout < 0.01);
        e.step(1., true);
        e.step(1., true);
        assert_eq!(e.blackout, 0.);
        let mut e = GEffects {
            blackout: 0.5,
            redout: 0.5,
            ..Default::default()
        };
        e.step(9., false);
        assert_eq!(e, GEffects::default());
    }
    #[test]
    fn tunnel_vision_darkens_edges_first_and_everything_at_full_loss() {
        assert_eq!(GEffects::coverage(0., 1.), 0.);
        assert!(GEffects::coverage(0.3, 1.) > 0.);
        assert_eq!(GEffects::coverage(0.3, 0.), 0.);
        assert_eq!(GEffects::coverage(1., 0.), 1.);
    }
    #[test]
    fn shake_starts_at_six_g_grows_to_nine_and_is_deterministic() {
        let peak = |g: f64| {
            (0..1200)
                .map(|t| {
                    let [x, y] = shake(g, t as f64 * DT);
                    x.abs().max(y.abs())
                })
                .fold(0., f64::max)
        };
        assert_eq!(peak(6.), 0.);
        assert!(peak(7.) > 0.);
        assert!(peak(7.) < peak(9.));
        assert_eq!(peak(9.), peak(12.));
        assert!(peak(9.) <= SHAKE_RADIANS);
        assert_eq!(shake(8., 1.234), shake(8., 1.234));
    }
}
