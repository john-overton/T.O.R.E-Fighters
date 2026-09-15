//! Authored presentation interpolation of source colors. Never advances the clock,
//! callbacks or RNG; native quantized environment queries remain unchanged.
use super::Environment;
use tore_formats::weather::{ALTITUDE_HAZE, Layer};

#[derive(Clone)]
struct Band {
    layer: Layer,
    colors: [[f64; 3]; 256],
    tint: [f64; 3],
}
fn mix(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
impl Band {
    fn new(layer: &Layer, base: &[[u8; 3]; 256]) -> Self {
        let mut colors = base.map(|c| c.map(f64::from));
        for (out, c) in colors[192..224].iter_mut().zip(layer.terrain) {
            *out = c.map(f64::from);
        }
        for (out, c) in colors[224..255].iter_mut().zip(layer.sky) {
            *out = c.map(f64::from);
        }
        Self {
            layer: layer.clone(),
            colors,
            tint: layer.tint.map(f64::from),
        }
    }
    fn blend(&mut self, other: &Self, t: f64) {
        let t = t.clamp(0., 1.);
        for (a, b) in self
            .colors
            .iter_mut()
            .flatten()
            .zip(other.colors.iter().flatten())
        {
            *a = mix(*a, *b, t);
        }
        self.tint = std::array::from_fn(|i| mix(self.tint[i], other.tint[i], t));
        // Metadata retains the bounded source grammar; color precision is separate.
        self.layer
            .blend(&other.layer, (t * 1_000_000.).round() as i32, 1_000_000);
    }
    fn altitude_haze(&mut self, altitude: f64) {
        let l = &self.layer;
        if l.flags & ALTITUDE_HAZE == 0 {
            return;
        }
        let above = (altitude - f64::from(l.low_feet)) / 256.;
        let amount = if above <= f64::from(l.haze_low) {
            f64::from(l.haze_low_blend)
        } else if above >= f64::from(l.haze_high) {
            f64::from(l.haze_high_blend)
        } else {
            mix(
                f64::from(l.haze_low_blend),
                f64::from(l.haze_high_blend),
                (above - f64::from(l.haze_low)) / f64::from(l.haze_high - l.haze_low),
            )
        }
        .clamp(0., 256.)
            / 256.;
        for i in 192..255 {
            let weight = if i < 224 {
                amount
            } else if i >= 240 {
                amount * (i - 239) as f64 / 15.
            } else {
                0.
            };
            for k in 0..3 {
                self.colors[i][k] = mix(self.colors[i][k], self.tint[k], weight);
            }
        }
    }
}

pub struct Sample {
    pub bands: Vec<Layer>,
    pub layer: Layer,
    colors: [[f64; 3]; 256],
    tint: [f64; 3],
}
impl Sample {
    /// Float color interpolation ends at the normal eight-bit GPU palette upload.
    pub fn palette(&self, tint: f64, sun: f64) -> [[u8; 3]; 256] {
        self.palette_with_prefix(None, tint, sun)
    }
    /// Preserve private cockpit colors after source HUD brightness, before effects.
    pub fn palette_with_prefix(
        &self,
        prefix: Option<&[[u8; 3]; 64]>,
        tint: f64,
        sun: f64,
    ) -> [[u8; 3]; 256] {
        let mut colors = self.colors;
        if let Some(prefix) = prefix {
            for (out, color) in colors[..64].iter_mut().zip(prefix) {
                *out = color.map(f64::from);
            }
        }
        for (i, color) in colors[..255].iter_mut().enumerate() {
            for (k, c) in color.iter_mut().enumerate() {
                *c = mix(*c, 63., sun.clamp(0., 255.) / 256.);
                let amount = if i >= 64 {
                    tint
                } else if (47..=60).contains(&i) {
                    tint.min(92.)
                } else {
                    0.
                };
                *c = mix(*c, self.tint[k], amount.clamp(0., 255.) / 256.);
            }
        }
        colors.map(|c| c.map(|v| (v * 255. / 63.).round().clamp(0., 255.) as u8))
    }
}
impl Environment {
    /// Current fractional simulation time, including wrap; no wall-time input.
    pub fn visual_seconds_of_day(&self) -> f64 {
        (f64::from(self.configuration.start_seconds) + self.ticks as f64 / 256.).rem_euclid(86400.)
    }
    /// Reevaluate color blends at this tick rather than holding the ten-second
    /// native selection result. Mutable source records are only read here.
    pub fn visual_sample(&self, altitude: f64) -> Option<Sample> {
        self.visual_sample_at(altitude, self.visual_seconds_of_day())
    }
    fn visual_sample_at(&self, altitude: f64, seconds: f64) -> Option<Sample> {
        let altitude = if altitude.is_finite() {
            altitude.max(0.)
        } else {
            0.
        };
        let mut bands: Vec<Band> = Vec::new();
        for record in &self.records {
            if !record.covers_time(seconds.floor() as i32) {
                continue;
            }
            let next = Band::new(record, self.configuration.base_palette());
            if let Some(previous) = bands.last_mut()
                && previous.layer.low_feet == record.low_feet
            {
                let span = f64::from(previous.layer.end_seconds - record.start_seconds);
                let t = if span > 0. {
                    (seconds - f64::from(record.start_seconds)) / span
                } else {
                    1.
                };
                previous.blend(&next, t);
            } else {
                bands.push(next);
            }
        }
        let active = bands.iter().map(|b| b.layer.clone()).collect();
        let mut result: Option<Band> = None;
        for mut band in bands {
            if !band.layer.covers_altitude(super::clamp_altitude(altitude)) {
                continue;
            }
            band.altitude_haze(altitude);
            if let Some(previous) = &mut result {
                let span = f64::from(previous.layer.high_feet - band.layer.low_feet);
                let t = if span > 0. {
                    (altitude - f64::from(band.layer.low_feet)) / span
                } else {
                    1.
                };
                previous.blend(&band, t);
            } else {
                result = Some(band);
            }
        }
        result.map(|b| Sample {
            bands: active,
            layer: b.layer,
            colors: b.colors,
            tint: b.tint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Configuration;
    use tore_formats::weather::Module;
    #[test]
    fn visual_queries_interpolate_fractional_time_without_advancing_native_state() {
        let mut m = Module::parse(&tore_formats::weather::synthetic_module(1)).unwrap();
        let mut a = m.layers[0].clone();
        a.flags = 0;
        a.start_seconds = 0;
        a.end_seconds = 120;
        a.low_feet = 0;
        a.high_feet = 10000;
        a.sky = [[0; 3]; 31];
        let mut b = a.clone();
        b.start_seconds = 60;
        b.end_seconds = 86400;
        b.sky = [[60; 3]; 31];
        m.layers = vec![a, b];
        let e = Environment::new(Configuration::new(m, 0, 1, 0, None).unwrap());
        let before = e.clone();
        let a = e.visual_sample_at(0., 70.).unwrap();
        let b = e.visual_sample_at(0., 70.25).unwrap();
        assert!((a.colors[224][0] - 10.).abs() < 1e-9);
        assert!((b.colors[224][0] - 10.25).abs() < 1e-9);
        assert_eq!(
            e, before,
            "queries must not consume callback/RNG/clock state"
        );
        assert_eq!(e.visual_sample_at(0., 60.).unwrap().colors[224], [0.; 3]);
        assert_eq!(e.visual_sample_at(0., 120.).unwrap().colors[224], [60.; 3]);
    }
    #[test]
    fn altitude_color_is_continuous_across_native_256_foot_steps() {
        let mut m = Module::parse(&tore_formats::weather::synthetic_module(1)).unwrap();
        let l = &mut m.layers[0];
        l.flags = ALTITUDE_HAZE;
        l.low_feet = 0;
        l.high_feet = 10000;
        l.start_seconds = 0;
        l.end_seconds = 86400;
        l.haze_low = 0;
        l.haze_high = 10;
        l.haze_low_blend = 0;
        l.haze_high_blend = 256;
        l.terrain = [[0; 3]; 32];
        l.tint = [60; 3];
        let e = Environment::new(Configuration::new(m, 0, 0, 0, None).unwrap());
        let before = e.clone();
        let below = e.visual_sample(255.9).unwrap().colors[192][0];
        let above = e.visual_sample(256.1).unwrap().colors[192][0];
        assert!(above > below && above - below < 0.01);
        assert_eq!(e, before);
    }

    #[test]
    fn fractional_clock_wrap_and_private_palette_keep_cutout_and_effect_order() {
        let m = Module::parse(&tore_formats::weather::synthetic_module(1)).unwrap();
        let mut e = Environment::new(Configuration::new(m, 23, 59, 0, None).unwrap());
        e.ticks = 60 * 256 + 64;
        assert_eq!(e.visual_seconds_of_day(), 0.25);
        let mut s = e.visual_sample_at(0., 0.).unwrap();
        s.tint = [0.; 3];
        let prefix = [[31; 3]; 64];
        let p = s.palette_with_prefix(Some(&prefix), 255., 128.);
        assert_eq!(p[40], [190; 3], "HUD brightens before uncapped world tint");
        assert!(
            p[47][0] < p[40][0],
            "cockpit tint applies only to source range"
        );
        assert_eq!(p[255], s.palette(0., 0.)[255], "cutout must not whiten");
    }
}
