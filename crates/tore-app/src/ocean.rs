//! Ocean presentation: user-requested short ripples and distance-dependent filtering.
use crate::AppResult;

#[derive(Clone, Copy)]
pub struct Motion {
    enabled: bool,
    phase: Option<f32>,
    pub environment_reflection: f32,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            enabled: true,
            phase: None,
            environment_reflection: 0.3,
        }
    }
}
impl Motion {
    pub fn from_environment() -> AppResult<Self> {
        let mut motion = Self::parse(
            std::env::var("TORE_OCEAN_MOTION").ok().as_deref(),
            std::env::var("TORE_OCEAN_PHASE").ok().as_deref(),
        )?;
        if let Ok(value) = std::env::var("TORE_WATER_ENV_REFLECTION") {
            let peak: f32 = value.parse()?;
            if !peak.is_finite() || !(0.0..=1.0).contains(&peak) {
                return Err("TORE_WATER_ENV_REFLECTION must be finite in 0..1".into());
            }
            motion.environment_reflection = peak;
        }
        Ok(motion)
    }
    fn parse(enabled: Option<&str>, phase: Option<&str>) -> AppResult<Self> {
        let enabled = match enabled {
            None | Some("1") => true,
            Some("0") => false,
            _ => return Err("TORE_OCEAN_MOTION must be 0 or 1".into()),
        };
        let phase = phase.map(str::parse::<f32>).transpose()?;
        if phase.is_some_and(|v| !v.is_finite() || !(0.0..120.0).contains(&v)) {
            return Err("TORE_OCEAN_PHASE must be finite seconds in 0..120".into());
        }
        Ok(Self {
            enabled,
            phase,
            environment_reflection: 0.3,
        })
    }
    /// Bounded phase shared by every camera. 120 seconds contains whole periods
    /// of the fitted ripple animation.
    pub fn seconds(&self, ticks: i64) -> f32 {
        self.phase
            .unwrap_or_else(|| ticks.rem_euclid(120 * 256) as f32 / 256.)
    }
    pub fn uniform(&self, ticks: i64, ocean_decks: [bool; 2], pixel_angle: f32) -> [f32; 4] {
        [
            self.seconds(ticks),
            f32::from(self.enabled && ocean_decks[0]),
            f32::from(self.enabled && ocean_decks[1]),
            pixel_angle,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phase_is_shared_repeatable_bounded_and_wraps_without_losing_ticks() {
        let m = Motion::default();
        assert_eq!(m.seconds(64), 0.25);
        assert_eq!(m.seconds(120 * 256 + 64), m.seconds(64));
        let ticks = i64::MAX - 127;
        assert!((0.0..120.).contains(&m.seconds(ticks)));
        let paused = m.uniform(8192, [false, true], 0.002);
        for _ in 0..10 {
            assert_eq!(m.uniform(8192, [false, true], 0.002), paused);
        }
        assert_eq!(m.seconds(0), 0.);
        assert_ne!(m.seconds(8192), m.seconds(8194));
    }
    #[test]
    fn controls_reject_bad_values_and_isolate_sky_and_static_mode() {
        for value in ["NaN", "inf", "-1", "120", "nonsense"] {
            assert!(Motion::parse(None, Some(value)).is_err());
        }
        assert!(Motion::parse(Some("2"), None).is_err());
        let m = Motion::parse(None, Some("3.5")).unwrap();
        assert_eq!(m.uniform(0, [false, true], 0.002), [3.5, 0., 1., 0.002]);
        assert_eq!(m.seconds(9999), 3.5);
        let m = Motion::parse(Some("0"), None).unwrap();
        assert_eq!(m.uniform(0, [true, true], 0.002), [0., 0., 0., 0.002]);
    }
}
