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

/// One recovered weather choice. `0x4f0670` and `0x4f0688` are parallel tables
/// indexed by the mission's `layer` parameter, which `0x495fbf` writes out as
/// the second value on the `layer` line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Condition {
    pub layer: &'static str,
    pub seconds_of_day: i32,
    /// 0x42a8df: only these three picks can also carry a scattered cloud deck,
    /// and then only half the time, between 7,000 and 20,000 feet.
    pub scattered_clouds: bool,
}

/// The six choices the source weather table offers, in its own order.
pub const CONDITIONS: [Condition; 6] = [
    Condition {
        layer: "DAY2",
        seconds_of_day: 43_200,
        scattered_clouds: true,
    },
    Condition {
        layer: "CLOUD1",
        seconds_of_day: 43_200,
        scattered_clouds: false,
    },
    Condition {
        layer: "FOG1",
        seconds_of_day: 43_200,
        scattered_clouds: false,
    },
    Condition {
        layer: "DAY2",
        seconds_of_day: 25_260,
        scattered_clouds: true,
    },
    Condition {
        layer: "DAY2",
        seconds_of_day: 68_460,
        scattered_clouds: true,
    },
    Condition {
        layer: "DAY2",
        seconds_of_day: 0,
        scattered_clouds: false,
    },
];

/// 0x42a80c: a theater whose map name begins with one of these letters uses the
/// matching per-theater module variant; every other theater uses the plain one.
const VARIANTS: [char; 5] = ['B', 'E', 'F', 'T', 'V'];

/// The `.LAY` resource one condition selects for one theater map name.
pub fn layer_resource(condition: usize, map: &str) -> Result<String> {
    let choice = CONDITIONS
        .get(condition)
        .ok_or_else(|| std::io::Error::other("weather condition outside the source table"))?;
    // The native skip covers generated campaign prefixes.
    let letter = map
        .chars()
        .find(|c| *c != '~' && *c != '$')
        .ok_or_else(|| std::io::Error::other("theater map name is empty"))?
        .to_ascii_uppercase();
    let suffix = if VARIANTS.contains(&letter) {
        letter.to_string()
    } else {
        String::new()
    };
    Ok(format!("{}{suffix}.LAY", choice.layer))
}

/// Validated launch environment. Construction resolves every source field once.
#[derive(Clone, Debug, PartialEq)]
pub struct Configuration {
    module: Module,
    start_seconds: i32,
    parameter: i32,
    wind: [f64; 3],
}

impl Configuration {
    /// `hour`/`minute` come from the mission `time` line; `parameter` from
    /// `layer`; `wind` from the `wind` line, unresolved and in source units.
    pub fn new(
        module: Module,
        hour: i32,
        minute: i32,
        parameter: i32,
        wind: Option<[i32; 2]>,
    ) -> Result<Self> {
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
            wind: resolve_wind(wind)?,
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

    pub fn base_palette(&self) -> &[[u8; 3]; 256] {
        &self.module.base
    }

    /// Steady horizontal wind in world feet per second, X east and Z north.
    pub fn wind_world_fps(&self) -> [f64; 3] {
        self.wind
    }
}

/// One resolved environment instant at one altitude: a record blended across
/// every overlap window that applies. Pure data; no renderer types.
pub type Sample = Layer;

/// Authoritative weather state. Advanced only by the host fixed-tick service.
#[derive(Clone, Debug, PartialEq)]
pub struct Environment {
    configuration: Configuration,
    clock: FixedClock,
    ticks: i64,
    active: Vec<Layer>,
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

    /// The time-resolved active list, already collapsed across dawn and dusk
    /// windows. This is the equivalent of native `curLayers`.
    pub fn active(&self) -> &[Layer] {
        &self.active
    }

    /// Pure query. `_WRGetLayer@8` truncates altitude and clamps it at zero;
    /// `0x4b3be0` then blends every active record whose band contains it.
    pub fn sample(&self, altitude_feet: f64) -> Option<Sample> {
        let feet = clamp_altitude(altitude_feet);
        let mut result: Option<Layer> = None;
        for layer in &self.active {
            if !layer.covers_altitude(feet) {
                continue;
            }
            match &mut result {
                // 0x4b3c37: position walks from this record's floor to the
                // previous record's ceiling, so bands cross over their overlap.
                Some(previous) => {
                    let mut next = layer.clone();
                    next.apply_altitude_haze(feet);
                    let span = previous.high_feet.saturating_sub(layer.low_feet);
                    previous.blend(&next, feet.saturating_sub(layer.low_feet), span);
                }
                None => {
                    let mut first = layer.clone();
                    first.apply_altitude_haze(feet);
                    result = Some(first);
                }
            }
        }
        result
    }

    /// The 256-entry palette for one altitude, expanded over the module base.
    pub fn palette(&self, altitude_feet: f64) -> Option<[[u8; 3]; 256]> {
        let layer = self.sample(altitude_feet)?;
        Some(tore_formats::weather::expand(
            self.configuration.base_palette(),
            &layer,
        ))
    }

    /// Effect strength across an altitude span, matching `_WRWeatherEffects`
    /// at 0x4b4720: the span minimum, capped at 100, or 100 with no active record.
    pub fn effect(&self, selector: usize, low_feet: f64, high_feet: f64) -> Result<u8> {
        let (low, high) = (clamp_altitude(low_feet), clamp_altitude(high_feet));
        let (low, high) = (low.min(high), low.max(high));
        let mut strength = 100;
        for layer in &self.active {
            if layer.high_feet >= low && layer.low_feet <= high {
                strength = strength.min(layer.effect(selector)?);
            }
        }
        Ok(strength)
    }

    /// Rescans the record table whenever the whole second changed. Adjacent
    /// records sharing a floor collapse into one blended record, exactly as
    /// 0x4b37ab does, which is how dawn and dusk cross over.
    fn select(&mut self) {
        let seconds = self.seconds_of_day();
        if seconds == self.active_seconds {
            return;
        }
        self.active_seconds = seconds;
        self.active.clear();
        for layer in self.configuration.layers() {
            if !layer.covers_time(seconds) {
                continue;
            }
            if let Some(previous) = self.active.last_mut()
                && previous.low_feet == layer.low_feet
            {
                // Native word arithmetic scales both sides by four before blending.
                let position = (seconds - layer.start_seconds) / 4;
                let span = (previous.end_seconds.saturating_sub(layer.start_seconds)) / 4;
                previous.blend(layer, position, span);
                continue;
            }
            self.active.push(layer.clone());
        }
    }
}

/// The mission `wind` line is a compass heading in whole degrees and a speed in
/// feet per second: `0x481e70` multiplies the heading by 182 into a binary angle
/// and stores the speed unscaled, and `0x476f3d` then advances position by
/// `speed * ticks` rotated by that angle. `_Rotate2@8` turns (0, d) into
/// `(d sin h, d cos h)`, so heading zero is north and ninety is east.
///
/// Whether the value names the direction the wind blows towards or comes from
/// is UNRESOLVED; this reproduces the arithmetic, which drifts an aircraft
/// towards the stated heading.
fn resolve_wind(wind: Option<[i32; 2]>) -> Result<[f64; 3]> {
    let Some([heading, speed]) = wind else {
        return Ok([0.; 3]);
    };
    if !(0..=360).contains(&heading) || !(0..=200).contains(&speed) {
        return Err(std::io::Error::other("mission wind outside source range"));
    }
    // The native conversion truncates into a 16-bit binary angle; keep that.
    let binary = f64::from((heading * 182) as i16);
    let radians = binary * std::f64::consts::TAU / 65536.;
    let speed = f64::from(speed);
    Ok([speed * radians.sin(), 0., speed * radians.cos()])
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
        Configuration::new(module, 0, 0, 0, None).unwrap()
    }

    #[test]
    fn conditions_select_the_recovered_module_and_time() {
        assert_eq!(layer_resource(0, "UKR.T2").unwrap(), "DAY2.LAY");
        assert_eq!(layer_resource(1, "UKR.T2").unwrap(), "CLOUD1.LAY");
        assert_eq!(layer_resource(2, "UKR.T2").unwrap(), "FOG1.LAY");
        // Theater variants, including a generated campaign prefix.
        assert_eq!(layer_resource(0, "EGY.T2").unwrap(), "DAY2E.LAY");
        assert_eq!(layer_resource(0, "~VIET6.T2").unwrap(), "DAY2V.LAY");
        assert_eq!(layer_resource(5, "TAI.T2").unwrap(), "DAY2T.LAY");
        assert!(layer_resource(6, "UKR.T2").is_err());
        assert!(layer_resource(0, "").is_err());
        // Dawn, sunset and night are the day module at different times.
        assert_eq!(CONDITIONS[3].seconds_of_day, 25_260);
        assert_eq!(CONDITIONS[4].seconds_of_day, 68_460);
        assert_eq!(CONDITIONS[5].seconds_of_day, 0);
        assert!(!CONDITIONS[5].scattered_clouds);
    }

    #[test]
    fn mission_wind_resolves_to_world_feet_per_second() {
        let module = Module::parse(&tore_formats::weather::synthetic_module(1)).unwrap();
        let of = |h, s| {
            Configuration::new(module.clone(), 12, 0, 0, Some([h, s]))
                .unwrap()
                .wind_world_fps()
        };
        // Heading zero is north and ninety is east, at the stated speed.
        let north = of(0, 10);
        assert!(north[2] > 9.99 && north[0].abs() < 1e-9 && north[1] == 0.);
        let east = of(90, 10);
        assert!(east[0] > 9.98 && east[2].abs() < 0.02, "{east:?}");
        // The documented Ukraine mission value.
        let ukraine = of(160, 7);
        assert!((ukraine[0] * ukraine[0] + ukraine[2] * ukraine[2]).sqrt() - 7. < 1e-6);
        assert!(
            ukraine[0] > 0. && ukraine[2] < 0.,
            "from the north-east: {ukraine:?}"
        );
        assert_eq!(
            Configuration::new(module, 12, 0, 0, None)
                .unwrap()
                .wind_world_fps(),
            [0.; 3]
        );
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
        let mut e = Environment::new(Configuration::new(module, 23, 59, 0, None).unwrap());
        for _ in 0..120 * 61 {
            e.step();
        }
        assert_eq!(e.seconds_of_day(), 1);
    }

    #[test]
    fn selection_follows_the_clock_and_sampling_is_pure() {
        let mut e = Environment::new(configuration(4));
        assert_eq!(e.active().len(), 1);
        assert_eq!(e.active()[0].start_seconds, 0);
        for _ in 0..120 * 3600 {
            e.step();
        }
        assert_eq!(e.active().len(), 1);
        assert_eq!(e.active()[0].start_seconds, 3600);
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
        assert_eq!(sample.start_seconds, 0);
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
            assert!(Configuration::new(module.clone(), hour, minute, parameter, None).is_err());
        }
        for wind in [[-1, 7], [361, 7], [160, -1], [160, 201]] {
            assert!(Configuration::new(module.clone(), 12, 0, 0, Some(wind)).is_err());
        }
        assert!(Configuration::new(module, 23, 59, 255, None).is_ok());
    }
}
