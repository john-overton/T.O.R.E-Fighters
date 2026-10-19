//! The AI test settings (slice S7b of the optimization run): two switches
//! for measuring CPU use and AI behaviour. The defaults (slice S9) are the
//! 120 Hz observation rate and lean sensors, which decide exactly as full
//! sensors do (S7b measured the same checksum at every rate). Nothing about
//! them is checkpointed, because they are host setup like the ground
//! ceiling: the host sets them before every step.
//!
//! # AI observation rate
//!
//! An AI aircraft's observation (its sensors, its lookout over the world, the
//! received emitters and the awareness memory they feed) runs once every
//! `divisor` ticks instead of every tick: 120 Hz is a divisor of 1 (today),
//! 60 Hz of 2, 40 Hz of 3, 30 Hz of 4 and 20 Hz of 6. Aircraft are staggered
//! by their place in the mission, `(tick + index) % divisor == 0`, so each
//! tick carries the same share of the work whatever the aircraft count. The
//! controller, the flight model, the missile threat service and the incoming
//! fire service stay at 120 Hz.
//!
//! What a tick between two observations holds and what it refreshes:
//!
//! - **Held:** the awareness memory (the targets the controller and the
//!   engagement policy choose from, with the positions they were seen at),
//!   the sensors' contacts, selection and track, and the received emitters.
//!   All of them are already part of a checkpoint, and the phase is a pure
//!   function of the mission tick and the actor's index, so a checkpoint
//!   taken between two observations resumes exactly.
//! - **Refreshed every tick:** the lookout (the aircraft's own pose and the
//!   scan sector, with the attention point of the last observation), so the
//!   missile threat reaction and incoming fire see from where the aircraft
//!   is now and keep their 120 Hz reaction; the decision frame's terrain
//!   flag per held target (it depends on the aircraft's own position); the
//!   wing attackers count; held targets that died since the last
//!   observation are dropped.
//! - **Timing kept:** the sensors count a step as `divisor` ticks, so a radar
//!   track takes the same half second to acquire, a plot goes stale and a
//!   trail is sampled on the same clock.
//!
//! # Lean sensors
//!
//! The AI never reads an aircraft's own visual channel list or its map list
//! (the only reader is the player's wing order check, which lean sensors
//! answers from the awareness memory instead), and reads the received emitter
//! list only to name the launcher of an electronically supported missile
//! warning. With lean sensors the visual channel is not worked out, and the
//! received emitters are built when such a warning needs them. A checkpoint
//! then holds an empty visual list, map list and emitter list.
//!
//! Both are for testing: changing a default is a simulation change that
//! needs new guard references and goldens.

/// The observation rates the settings offer, in hertz. The first is today's.
pub const OBSERVATION_RATES_HZ: [u32; 5] = [120, 60, 40, 30, 20];

/// The AI test settings. [`Default`] is 120 Hz with lean sensors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiTestSettings {
    divisor: u32,
    /// AI aircraft skip the visual channel and build the received-radar list
    /// only when a warning needs it.
    pub lean_sensors: bool,
}

impl Default for AiTestSettings {
    fn default() -> Self {
        Self {
            divisor: 1,
            lean_sensors: true,
        }
    }
}

impl AiTestSettings {
    /// 120 Hz with the full sensors (the visual channel, map list and
    /// received emitters worked out every observation): what the AI did
    /// before lean sensors became the default.
    pub fn full() -> Self {
        Self::default().with_lean_sensors(false)
    }

    /// Observation every `divisor` ticks, `divisor` being 1 to 6 (the rates
    /// of [`OBSERVATION_RATES_HZ`] are the divisors that divide a second of
    /// ticks and the sensors' 60 tick history mark).
    pub fn with_divisor(mut self, divisor: u32) -> Self {
        self.divisor = divisor.clamp(1, 6);
        self
    }

    /// `hz` being one of [`OBSERVATION_RATES_HZ`]; `None` for any other.
    pub fn with_observation_hz(self, hz: u32) -> Option<Self> {
        OBSERVATION_RATES_HZ
            .contains(&hz)
            .then(|| self.with_divisor(super::TICKS_PER_SECOND as u32 / hz))
    }

    /// Lean sensors on or off.
    pub fn with_lean_sensors(mut self, lean: bool) -> Self {
        self.lean_sensors = lean;
        self
    }

    /// Ticks between two observations of one aircraft.
    pub fn divisor(&self) -> u32 {
        self.divisor
    }

    /// The observation rate in hertz.
    pub fn observation_hz(&self) -> u32 {
        super::TICKS_PER_SECOND as u32 / self.divisor
    }

    /// Whether these are today's behaviour.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The next rate of [`OBSERVATION_RATES_HZ`], wrapping to 120.
    pub fn next_rate(self) -> Self {
        let at = OBSERVATION_RATES_HZ
            .iter()
            .position(|hz| *hz == self.observation_hz())
            .unwrap_or(0);
        let hz = OBSERVATION_RATES_HZ[(at + 1) % OBSERVATION_RATES_HZ.len()];
        self.with_observation_hz(hz).expect("a listed rate")
    }

    /// `observation_hz` and `lean_sensors` as the command line gives them
    /// (`None` is the default of each): `None` for a rate that is not one of
    /// [`OBSERVATION_RATES_HZ`].
    pub fn from_flags(observation_hz: Option<u32>, lean_sensors: Option<bool>) -> Option<Self> {
        let settings = match lean_sensors {
            Some(lean) => Self::default().with_lean_sensors(lean),
            None => Self::default(),
        };
        match observation_hz {
            Some(hz) => settings.with_observation_hz(hz),
            None => Some(settings),
        }
    }

    /// The value of `--ai-lean-sensors`: `on` or `off`.
    pub fn parse_on_off(text: &str) -> Option<bool> {
        match text {
            "on" => Some(true),
            "off" => Some(false),
            _ => None,
        }
    }

    /// A line for logs and the probe's output: `120 Hz, full sensors`.
    pub fn describe(&self) -> String {
        format!(
            "{} Hz, {} sensors",
            self.observation_hz(),
            if self.lean_sensors { "lean" } else { "full" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_120_hz_with_lean_sensors() {
        let settings = AiTestSettings::default();
        assert_eq!(settings.divisor(), 1);
        assert_eq!(settings.observation_hz(), 120);
        assert!(settings.lean_sensors);
        assert!(settings.is_default());
        let full = AiTestSettings::full();
        assert!(!full.lean_sensors && !full.is_default());
        assert_eq!(full.observation_hz(), 120);
        assert_eq!(full.describe(), "120 Hz, full sensors");
        assert_eq!(settings.describe(), "120 Hz, lean sensors");
    }

    #[test]
    fn the_lean_flag_has_an_on_and_an_off_form() {
        assert_eq!(AiTestSettings::parse_on_off("on"), Some(true));
        assert_eq!(AiTestSettings::parse_on_off("off"), Some(false));
        assert_eq!(AiTestSettings::parse_on_off("yes"), None);
        assert!(AiTestSettings::from_flags(None, None).unwrap().is_default());
        assert_eq!(
            AiTestSettings::from_flags(None, Some(false)),
            Some(AiTestSettings::full())
        );
        assert!(
            AiTestSettings::from_flags(Some(120), Some(true))
                .unwrap()
                .is_default()
        );
    }

    #[test]
    fn every_offered_rate_is_a_whole_divisor_of_the_tick_rate() {
        for hz in OBSERVATION_RATES_HZ {
            let settings = AiTestSettings::default().with_observation_hz(hz).unwrap();
            assert_eq!(settings.observation_hz(), hz);
            assert_eq!(120 % settings.divisor(), 0);
            assert_eq!(60 % settings.divisor(), 0);
        }
        assert_eq!(AiTestSettings::default().with_observation_hz(90), None);
        assert_eq!(AiTestSettings::from_flags(Some(25), None), None);
    }

    #[test]
    fn the_rate_cycles_through_the_list_and_back() {
        let mut settings = AiTestSettings::default();
        let mut seen = vec![settings.observation_hz()];
        for _ in 0..OBSERVATION_RATES_HZ.len() {
            settings = settings.next_rate();
            seen.push(settings.observation_hz());
        }
        assert_eq!(seen, [120, 60, 40, 30, 20, 120]);
    }
}
