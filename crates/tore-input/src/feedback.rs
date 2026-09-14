//! Authored tactile cues from confirmed simulation events, never from raw fire keys.
//! Independent of flight state, native devices, wall time and replay authority.
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub enum FeedbackEvent {
    GunFired,
    MissileLaunched,
    BombReleased,
    RocketLaunched,
    /// Actual turbulence producer supplies normalized severity, not steady wind or G-load.
    Turbulence {
        intensity: f64,
    },
    AfterburnerEngaged,
    Damage,
    Crash,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FeedbackUpdate {
    Pulse {
        strong: f64,
        weak: f64,
        duration: Duration,
    },
    Stop,
}
#[derive(Clone, Copy, Debug, Default)]
struct Slot {
    remaining: u16,
    cooldown: u16,
    strong: f64,
    weak: f64,
}
/// Eight fixed effect slots; overlapping motor strengths use max, never summation.
/// Call event() for confirmed events, then tick() exactly once per 120 Hz tick.
#[derive(Clone, Debug)]
pub struct FeedbackMixer {
    slots: [Slot; 8],
    quiet_ticks: u16,
    dirty: bool,
    playing: bool,
    afterburner: bool,
}
impl Default for FeedbackMixer {
    fn default() -> Self {
        Self {
            slots: [Slot::default(); 8],
            quiet_ticks: 6,
            dirty: false,
            playing: false,
            afterburner: false,
        }
    }
}
impl FeedbackMixer {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn afterburner(&self) -> bool {
        self.afterburner
    }
    /// A quiet low-frequency bed with finite native leases; no infinite hardware effect.
    pub fn set_afterburner(&mut self, active: bool) {
        if self.afterburner != active {
            self.afterburner = active;
            if !active {
                self.slots[5].remaining = 0;
            }
            self.dirty = true;
        }
    }
    /// Returns false for a suppressed repeat or invalid/zero turbulence sample.
    pub fn event(&mut self, event: FeedbackEvent) -> bool {
        // Durations/cooldowns are simulation ticks. Six ticks = 50 ms.
        let (index, duration, cooldown, strong, weak) = match event {
            FeedbackEvent::GunFired => (0, 8, 6, 0.08, 0.16),
            FeedbackEvent::MissileLaunched => (1, 18, 12, 0.18, 0.10),
            FeedbackEvent::BombReleased => (2, 12, 12, 0.12, 0.07),
            FeedbackEvent::RocketLaunched => (3, 10, 6, 0.10, 0.16),
            FeedbackEvent::Turbulence { intensity } => {
                if !intensity.is_finite() || intensity <= 0. {
                    return false;
                }
                let intensity = intensity.min(1.);
                (4, 15, 12, 0.10 * intensity, 0.06 * intensity)
            }
            FeedbackEvent::AfterburnerEngaged => (5, 18, 60, 0.06, 0.10),
            FeedbackEvent::Damage => (6, 20, 12, 0.25, 0.18),
            FeedbackEvent::Crash => (7, 22, 120, 0.35, 0.20),
        };
        let slot = &mut self.slots[index];
        if slot.cooldown != 0 {
            return false;
        }
        *slot = Slot {
            remaining: duration,
            cooldown,
            strong,
            weak,
        };
        self.dirty = true;
        true
    }

    pub fn tick(&mut self) -> Option<FeedbackUpdate> {
        // Limit native requests to 20 Hz, including simultaneous/repeated events.
        if self.afterburner && self.quiet_ticks >= 60 {
            self.dirty = true;
        }
        let output = if self.dirty && self.quiet_ticks >= 6 {
            let mut remaining = if self.afterburner { 90 } else { 0 };
            let (mut strong, mut weak) = if self.afterburner {
                (0.035_f64, 0.01_f64)
            } else {
                (0., 0.)
            };
            for slot in &self.slots {
                if slot.remaining != 0 {
                    remaining = remaining.max(slot.remaining);
                    strong = strong.max(slot.strong);
                    weak = weak.max(slot.weak);
                }
            }
            self.dirty = false;
            self.quiet_ticks = 0;
            if remaining != 0 {
                self.playing = true;
                Some(FeedbackUpdate::Pulse {
                    strong,
                    weak,
                    duration: Duration::from_micros(
                        (u64::from(remaining) * 1_000_000).div_ceil(120),
                    ),
                })
            } else if std::mem::take(&mut self.playing) {
                Some(FeedbackUpdate::Stop)
            } else {
                None
            }
        } else {
            None
        };
        for slot in &mut self.slots {
            if slot.remaining == 1 {
                self.dirty = true;
            }
            slot.remaining = slot.remaining.saturating_sub(1);
            slot.cooldown = slot.cooldown.saturating_sub(1);
        }
        self.quiet_ticks = self.quiet_ticks.saturating_add(1);
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn afterburner_bed_renews_finite_leases_and_stops_on_disengagement() {
        let mut m = FeedbackMixer::default();
        m.set_afterburner(true);
        let mut last = 0;
        for tick in 0..360 {
            if let Some(update) = m.tick() {
                match update {
                    FeedbackUpdate::Pulse {
                        strong,
                        weak,
                        duration,
                    } => {
                        assert_eq!((strong, weak), (0.035, 0.01));
                        assert_eq!(duration, Duration::from_millis(750));
                        assert!(tick - last <= 60);
                        last = tick;
                    }
                    FeedbackUpdate::Stop => panic!("active bed stopped"),
                }
            }
        }
        m.set_afterburner(false);
        assert!((0..7).any(|_| m.tick() == Some(FeedbackUpdate::Stop)));
        m.set_afterburner(true);
        m.clear();
        assert!(!m.afterburner());
        assert_eq!(m.tick(), None);
    }
    #[test]
    fn overlap_is_capped_and_expiry_returns_to_silence() {
        let mut m = FeedbackMixer::default();
        m.event(FeedbackEvent::Crash);
        m.event(FeedbackEvent::GunFired);
        assert!(matches!(
            m.tick(),
            Some(FeedbackUpdate::Pulse {
                strong: 0.35,
                weak: 0.20,
                ..
            })
        ));
        let mut stopped = false;
        for _ in 0..40 {
            stopped |= m.tick() == Some(FeedbackUpdate::Stop);
        }
        assert!(stopped);
        assert_eq!(m.tick(), None);
    }
    #[test]
    fn rapid_fire_is_bounded_and_cannot_leave_a_held_effect() {
        let mut m = FeedbackMixer::default();
        let mut pulses = 0;
        for _ in 0..120 {
            m.event(FeedbackEvent::GunFired);
            m.event(FeedbackEvent::RocketLaunched);
            if let Some(FeedbackUpdate::Pulse {
                strong,
                weak,
                duration,
            }) = m.tick()
            {
                pulses += 1;
                assert!(strong <= 0.35 && weak <= 0.20);
                assert!(duration <= Duration::from_millis(184));
            }
        }
        assert_eq!(pulses, 20);
        assert!((0..40).any(|_| m.tick() == Some(FeedbackUpdate::Stop)));
    }
    #[test]
    fn interruption_discards_pending_impulses_and_cooldowns() {
        let mut m = FeedbackMixer::default();
        m.event(FeedbackEvent::AfterburnerEngaged);
        m.tick();
        m.event(FeedbackEvent::BombReleased);
        m.clear();
        for _ in 0..120 {
            assert_eq!(m.tick(), None);
        }
        assert!(m.event(FeedbackEvent::AfterburnerEngaged));
    }
    #[test]
    fn turbulence_is_explicit_finite_and_scaled() {
        let mut m = FeedbackMixer::default();
        for intensity in [f64::NAN, f64::INFINITY, -1., 0.] {
            assert!(!m.event(FeedbackEvent::Turbulence { intensity }));
        }
        assert_eq!(m.tick(), None);
        m.event(FeedbackEvent::Turbulence { intensity: 0.5 });
        assert!(matches!(
            m.tick(),
            Some(FeedbackUpdate::Pulse {
                strong: 0.05,
                weak: 0.03,
                ..
            })
        ));
    }
    #[test]
    fn distinct_event_impulses_and_repeat_cooldowns() {
        for event in [
            FeedbackEvent::GunFired,
            FeedbackEvent::MissileLaunched,
            FeedbackEvent::BombReleased,
            FeedbackEvent::RocketLaunched,
            FeedbackEvent::AfterburnerEngaged,
            FeedbackEvent::Damage,
            FeedbackEvent::Crash,
        ] {
            let mut m = FeedbackMixer::default();
            assert!(m.event(event));
            assert!(!m.event(event));
            assert!(matches!(m.tick(), Some(FeedbackUpdate::Pulse { .. })));
        }
    }
}
