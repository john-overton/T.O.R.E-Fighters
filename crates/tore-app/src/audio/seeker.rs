//! Temporary fitted cue, not a claimed retail sample mapping.
#[derive(Default)]
pub struct Tone {
    pub target: f64,
    pub gain: f64,
    pub phase: f64,
    pub ground: bool,
}
impl Tone {
    pub fn sample(&mut self, rate: f64, audible: bool) -> f32 {
        if !audible {
            return 0.;
        }
        // Full-scale 0.1 second linear slew, independent of device sample rate.
        self.gain += (self.target - self.gain).clamp(-10. / rate, 10. / rate);
        self.phase = (self.phase + 1. / rate).fract();
        let wave = if self.ground {
            (self.phase * 660. * std::f64::consts::TAU).sin()
        } else {
            (self.phase * 110. * std::f64::consts::TAU).sin()
                * (0.65 + 0.35 * (self.phase * 37. * std::f64::consts::TAU).sin())
        };
        (wave * self.gain) as f32
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn envelope_pause_and_timbre_are_deterministic() {
        for rate in [8000., 48000.] {
            let mut tone = Tone {
                target: 1.,
                ..Default::default()
            };
            for _ in 0..(rate * 0.1) as usize {
                tone.sample(rate, true);
            }
            assert!((tone.gain - 1.).abs() < 1e-9);
            let phase = tone.phase;
            assert_eq!(tone.sample(rate, false), 0.);
            assert_eq!(tone.phase, phase);
            tone.target = 0.;
            for _ in 0..(rate * 0.1) as usize {
                tone.sample(rate, true);
            }
            assert!(tone.gain < 1e-9);
        }
        let mut air = Tone {
            target: 1.,
            ..Default::default()
        };
        let mut ground = Tone {
            target: 1.,
            ground: true,
            ..Default::default()
        };
        assert_ne!(air.sample(48000., true), ground.sample(48000., true));
    }
}
