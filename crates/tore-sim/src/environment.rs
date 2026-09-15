//! Renderer-independent environment evolution. Immutable validated configuration,
//! caller-owned clock state and pure spatial queries: sampling for a mirror, a
//! camera panel or another aircraft never advances weather.
use tore_formats::Result;
use tore_formats::flight_model::clock_rng::FixedClock;
use tore_formats::weather::{Layer, Module};

/// Native simulation clock resolution (0x486bf0 scales elapsed time by 256).
pub const TICKS_PER_SECOND: i64 = 256;
pub const SECONDS_PER_DAY: i64 = 86_400;
/// `_WRWeatherEffects` selector 0 drives the visibility distance at 0x48d98d.
pub const VISIBILITY: usize = 0;

/// Validated launch environment. Construction resolves every source field once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Configuration {
    module: Module,
    start_seconds: i32,
    parameter: i32,
}

impl Configuration {
    /// `hour`/`minute` come from the mission `time` line; `parameter` from `layer`.
    pub fn new(module: Module, hour: i32, minute: i32, parameter: i32) -> Result<Self> {
        if !(0..24).contains(&hour) || !(0..60).contains(&minute) {
            return Err(std::io::Error::other("mission time outside one day"));
        }
        if !(0..=255).contains(&parameter) {
            return Err(std::io::Error::other("layer parameter outside byte range"));
        }
        // Native _TIMEInit at 0x486a34 computes (hour * 60 + minute) * 60.
        Ok(Self {
            module,
            start_seconds: (hour * 60 + minute) * 60,
            parameter,
        })
    }

    pub fn layers(&self) -> &[Layer] {
        &self.module.layers
    }

    pub fn start_seconds(&self) -> i32 {
        self.start_seconds
    }

    /// The mission `layer` parameter. Its full native semantics remain unverified;
    /// it is carried, not interpreted.
    pub fn parameter(&self) -> i32 {
        self.parameter
    }

    pub fn palette(&self, index: usize) -> Result<[[u8; 3]; 256]> {
        self.module.palette(index)
    }
}

/// One resolved environment instant at one altitude. Pure data; no renderer types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    /// Index into `Configuration::layers`.
    pub layer: usize,
    /// Record flag bits reaching `_currentLayer` (0x4b3526). Bit 0x40 is night hazing.
    pub flags: u16,
    /// True when a further record also covers this altitude (`_WRGetLayer` out flag).
    pub overlapped: bool,
}

impl Sample {
    /// `_currentLayer & 0x40` sets `_nightHazing` at 0x4b353c and suppresses
    /// wing vapor at 0x49fdad.
    pub fn night_hazing(self) -> bool {
        self.flags & tore_formats::weather::NIGHT_HAZING != 0
    }
}

/// Authoritative weather state. Advanced only by the host fixed-tick service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    configuration: Configuration,
    clock: FixedClock,
    ticks: i64,
    active: Vec<usize>,
    active_seconds: i32,
}

impl Environment {
    pub fn new(configuration: Configuration) -> Self {
        let mut environment = Self {
            configuration,
            clock: FixedClock::default(),
            ticks: 0,
            active: Vec::new(),
            active_seconds: -1,
        };
        environment.select();
        environment
    }

    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    /// Exactly one host 120 Hz tick. Pausing means not calling this method;
    /// there is no elapsed wall-time catch-up.
    pub fn step(&mut self) {
        self.ticks += i64::from(self.clock.advance(false));
        self.select();
    }

    /// Native ticks since launch at 256 units per second.
    pub fn ticks(&self) -> i64 {
        self.ticks
    }

    /// Time of day in seconds. The native path samples whole seconds from
    /// `currentTicks >> 8` and lags one weather update (0x486b7d); this authored
    /// clock drops the lag rather than reproducing an update-order artifact.
    /// The native 16-bit elapsed word wrap at 65,536 seconds is deliberately absent.
    pub fn seconds_of_day(&self) -> i32 {
        let elapsed = self.ticks / TICKS_PER_SECOND;
        ((i64::from(self.configuration.start_seconds) + elapsed).rem_euclid(SECONDS_PER_DAY)) as i32
    }

    /// Records whose inclusive time window covers the current second, in table order.
    pub fn active(&self) -> &[usize] {
        &self.active
    }

    /// Pure query. `_WRGetLayer@8` truncates altitude toward zero and clamps at 0,
    /// then takes the first active record whose inclusive altitude band contains it.
    pub fn sample(&self, altitude_feet: f64) -> Option<Sample> {
        let feet = clamp_altitude(altitude_feet);
        let layers = self.configuration.layers();
        let mut found = None;
        for (position, index) in self.active.iter().enumerate() {
            if layers[*index].covers_altitude(feet) {
                found = Some((position, *index));
                break;
            }
        }
        let (position, index) = found?;
        let overlapped = self.active[position + 1..]
            .iter()
            .any(|i| layers[*i].covers_altitude(feet));
        Some(Sample {
            layer: index,
            flags: layers[index].flags,
            overlapped,
        })
    }

    /// Effect strength across an altitude span, matching `_WRWeatherEffects`
    /// at 0x4b4720: the span minimum, capped at 100, or 100 with no active record.
    pub fn effect(&self, selector: usize, low_feet: f64, high_feet: f64) -> Result<u8> {
        let (low, high) = (clamp_altitude(low_feet), clamp_altitude(high_feet));
        let (low, high) = (low.min(high), low.max(high));
        let layers = self.configuration.layers();
        let mut strength = 100;
        for index in &self.active {
            let layer = &layers[*index];
            if layer.high_feet >= low && layer.low_feet <= high {
                strength = strength.min(layer.effect(selector)?);
            }
        }
        Ok(strength)
    }

    /// Rescans the record table whenever the whole second changed.
    fn select(&mut self) {
        let seconds = self.seconds_of_day();
        if seconds == self.active_seconds {
            return;
        }
        self.active_seconds = seconds;
        self.active.clear();
        for (index, layer) in self.configuration.layers().iter().enumerate() {
            if layer.covers_time(seconds) {
                self.active.push(index);
            }
        }
    }
}

/// `sar ecx, 8` then a negative clamp at 0x4b3195. Non-finite altitudes clamp low.
fn clamp_altitude(feet: f64) -> i32 {
    if !feet.is_finite() {
        return 0;
    }
    feet.floor().clamp(0., f64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configuration(records: usize) -> Configuration {
        let module = Module::parse(&tore_formats::weather::synthetic_module(records)).unwrap();
        Configuration::new(module, 0, 0, 0).unwrap()
    }

    #[test]
    fn clock_advances_at_the_native_rate_and_wraps_one_day() {
        let mut e = Environment::new(configuration(4));
        for _ in 0..120 {
            e.step();
        }
        assert_eq!(e.ticks(), 256);
        assert_eq!(e.seconds_of_day(), 1);
        let module = Module::parse(&tore_formats::weather::synthetic_module(1)).unwrap();
        let mut e = Environment::new(Configuration::new(module, 23, 59, 0).unwrap());
        for _ in 0..120 * 61 {
            e.step();
        }
        assert_eq!(e.seconds_of_day(), 1);
    }

    #[test]
    fn selection_follows_the_clock_and_sampling_is_pure() {
        let mut e = Environment::new(configuration(4));
        assert_eq!(e.active(), [0]);
        for _ in 0..120 * 3600 {
            e.step();
        }
        assert_eq!(e.active(), [1]);
        let before = e.clone();
        for altitude in [-5., 0., 1000., 99_999., f64::NAN] {
            let _ = e.sample(altitude);
            let _ = e.effect(VISIBILITY, 0., altitude);
        }
        assert_eq!(e, before, "queries must not advance environment state");
    }

    #[test]
    fn identical_ticks_produce_identical_state_at_any_query_order() {
        let run = |queries: bool| {
            let mut e = Environment::new(configuration(4));
            for tick in 0..120 * 7200 {
                e.step();
                if queries && tick % 7 == 0 {
                    let _ = e.sample(f64::from(tick % 40_000));
                }
            }
            e
        };
        assert_eq!(run(false), run(true));
    }

    #[test]
    fn altitude_query_matches_the_native_band_and_effect_minimum() {
        let e = Environment::new(configuration(2));
        let sample = e.sample(500.).unwrap();
        assert_eq!(sample.layer, 0);
        assert!(!sample.overlapped);
        assert!(!sample.night_hazing());
        assert!(e.sample(100_001.).is_none());
        // Fixture record 0 stores 80 and 40 in the first two effect bytes.
        assert_eq!(e.effect(VISIBILITY, 0., 1000.).unwrap(), 80);
        assert_eq!(e.effect(1, 0., 1000.).unwrap(), 40);
        assert_eq!(e.effect(2, 0., 1000.).unwrap(), 0);
        assert!(e.effect(tore_formats::weather::EFFECTS, 0., 1.).is_err());
    }

    #[test]
    fn invalid_launch_times_and_parameters_are_rejected() {
        let module = Module::parse(&tore_formats::weather::synthetic_module(1)).unwrap();
        for (hour, minute, parameter) in [(24, 0, 0), (-1, 0, 0), (0, 60, 0), (0, 0, 256)] {
            assert!(Configuration::new(module.clone(), hour, minute, parameter).is_err());
        }
        assert!(Configuration::new(module, 23, 59, 255).is_ok());
    }
}
