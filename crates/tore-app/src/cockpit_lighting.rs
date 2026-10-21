//! Local presentation choices. Player-visible rules: docs/spec/cockpit-lighting.md.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lighting {
    pub colour: u8,
    pub instruments: u8,
    pub flood: u8,
}
impl Default for Lighting {
    fn default() -> Self {
        Self {
            colour: 0,
            instruments: 4,
            flood: 3,
        }
    }
}
impl Lighting {
    pub const COLOURS: [&str; 4] = ["GREEN", "AMBER", "WHITE", "CYAN"];
    pub fn instrument_gain(self) -> f32 {
        f32::from(self.instruments.clamp(1, 5)) * 0.25
    }
    /// Green retains the complete existing weather-resolved HUD ink.
    pub fn hud(self, green: [u8; 3], brightness: i16) -> [u8; 3] {
        let base = match self.colour {
            1 => [255, 176, 48],
            2 => [220, 240, 255],
            3 => [64, 220, 255],
            _ => return green,
        };
        let mut palette = [[0; 3]; 256];
        palette[40] = base.map(|c| ((c as u16 * 63 + 127) / 255) as u8);
        tore_formats::weather::palette::apply_hud_brightness(&mut palette, brightness)
            .expect("validated brightness");
        palette[40].map(|c| ((u16::from(c) * 255 + 31) / 63) as u8)
    }
    pub fn flood_strength(self, ambient: f32) -> f32 {
        let level = [0., 0.18, 0.36, 0.60, 0.82, 1.][usize::from(self.flood.min(5))];
        let dark = 1. - ambient.clamp(0., 1.);
        level * (1. - 0.97 * (1. - dark.powi(2))) * (1. + 0.5 * dark)
    }
    pub fn step(&mut self, flood: bool, delta: i8) {
        let level = if flood {
            &mut self.flood
        } else {
            &mut self.instruments
        };
        *level = (i16::from(*level) + i16::from(delta)).clamp(if flood { 0 } else { 1 }, 5) as u8;
    }
}

/// Developer capture override: instrument level,flood level,colour index,HUD amount.
pub fn preview(text: &str) -> Result<(Lighting, i16), String> {
    let values = text
        .split(',')
        .map(str::parse::<i16>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "invalid cockpit lighting preview")?;
    let [instruments, flood, colour, brightness] = values.as_slice() else {
        return Err("cockpit lighting preview needs instruments,flood,colour,brightness".into());
    };
    if !(1..=5).contains(instruments)
        || !(0..=5).contains(flood)
        || !(0..=3).contains(colour)
        || !(-256..=256).contains(brightness)
    {
        return Err("cockpit lighting preview outside bounds".into());
    }
    Ok((
        Lighting {
            instruments: *instruments as u8,
            flood: *flood as u8,
            colour: *colour as u8,
        },
        *brightness,
    ))
}

/// Fitted exposure of flat cockpit art to the same sky light as world surfaces.
pub fn ambient(
    world: &crate::terrain::Terrain,
    scenery: &crate::scenery::Scenery,
    altitude: f64,
) -> f32 {
    let (sun, moon) = crate::surface_lighting::sky_directions(world, scenery, altitude);
    let radius = scenery
        .celestial
        .as_ref()
        .and_then(|c| c.solid_sun_radius())
        .unwrap_or(0.5_f32.to_radians());
    let (_, visible) = crate::surface_lighting::visible_sun(sun, radius);
    let transmission = if scenery.smooth_weather {
        let records = world.weather.configuration().layers();
        let shares = crate::sky_mix::band_shares(records, world.weather.visual_seconds_of_day());
        // Use the same bands as the world shader, retaining fractional record weights.
        let bands = shares.iter().map(|band| {
            let mut values = [0.; 4];
            for &(index, share) in band {
                let layer = &records[index];
                for (out, value) in values.iter_mut().zip([
                    layer.low_feet as f32,
                    layer.high_feet as f32,
                    layer.fog_far_density as f32 / 256.,
                    layer.fog_far as f32 * 256.,
                ]) {
                    *out += value * share as f32;
                }
            }
            values
        });
        sunlight_transmission(bands, altitude as f32, sun[1])
    } else {
        1.
    };
    let value = exposure(sun[1], moon[1], visible, transmission);
    // Capture diagnostics are opt-in with the existing lighting preview override.
    if std::env::var_os("TORE_COCKPIT_LIGHTING").is_some() {
        static REPORT: std::sync::Once = std::sync::Once::new();
        REPORT.call_once(|| println!(
            "Cockpit ambient: sun={:.3} deg, moon-light-y={:.5}, transmission={:.5}, paint={:.5}",
            sun[1].clamp(-1., 1.).asin().to_degrees(), moon[1], transmission, value,
        ));
    }
    value
}

fn smooth(low: f32, high: f32, value: f32) -> f32 {
    let t = ((value - low) / (high - low)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// CPU form of surface_lighting.wgsl sunlight_transmission, for flat art.
fn sunlight_transmission(bands: impl Iterator<Item = [f32; 4]>, altitude: f32, sun_y: f32) -> f32 {
    let mut path = 0.;
    for [low, high, density, far] in bands {
        let dense = smooth(0.9, 1., density) * (1. - smooth(8000., 16000., far));
        path += (high - altitude.max(low)).max(0.) * dense / sun_y.max(0.02);
    }
    (-path / 600.).exp()
}

fn exposure(sun_y: f32, moon_y: f32, visible: f32, transmission: f32) -> f32 {
    let night = crate::night_glow::night_factor(sun_y);
    let daylight = (0.28 + 0.90 * sun_y.max(0.) * visible * transmission)
        / (0.28 + 0.90 * 25_f32.to_radians().sin());
    ((1. - night) * daylight + night * (0.065 + 0.055 * moon_y.max(0.))).clamp(0., 1.)
}

/// Light non-emissive bezel paint from its original colour. Displays bypass this.
pub fn surface_pixel(pixel: &mut [u8], ambient: f32, flood: f32) {
    if pixel[3] == 0 {
        return;
    }
    let red = f32::from(pixel[0]);
    pixel[0] = (red * ambient + (22. + red * 0.35) * flood).min(255.) as u8;
    pixel[1] = (f32::from(pixel[1]) * ambient * (1. - 0.12 * flood)) as u8;
    pixel[2] = (f32::from(pixel[2]) * ambient * (1. - 0.16 * flood)) as u8;
}

/// Generated bezel legends retain a subtle, dimmable backlight in darkness.
pub fn legend_ink(original: [u8; 3], ambient: f32, flood: f32, gain: f32) -> [u8; 4] {
    let mut pixel = [original[0], original[1], original[2], 255];
    surface_pixel(&mut pixel, ambient, flood);
    for (channel, original) in pixel[..3].iter_mut().zip(original) {
        let backlight = f32::from(original) * 0.18 * (1. - ambient);
        *channel = (f32::from(*channel).max(backlight) * gain).min(255.) as u8;
    }
    pixel
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn world_sky_blend_dims_paint_continuously_and_moon_keeps_faint_shapes() {
        assert_eq!(exposure(1., 0., 1., 1.), 1.);
        assert_eq!(exposure(0.5, 0., 1., 1.), 1.);
        let sunset = exposure(0., 0., 0.5, 1.);
        assert!(sunset > 0.3 && sunset < 0.4);
        assert!((exposure(-0.2, 0., 0., 1.) - 0.065).abs() < 1e-6);
        assert!((exposure(-0.2, 1., 0., 1.) - 0.12).abs() < 1e-6);
        assert!(
            exposure(1., 0., 1., 0.) < 0.43,
            "overcast blocks direct sun"
        );
        let mut previous = exposure(-0.3, 0.5, 0., 1.);
        for i in 1..=10000 {
            let sun = -0.3 + i as f32 * 0.0001;
            let (_, disc) =
                crate::surface_lighting::visible_sun([0., sun, 1.], 0.5_f32.to_radians());
            let now = exposure(sun, 0.5, disc, 1.);
            assert!(
                (now - previous).abs() < 0.001,
                "light discontinuity at {sun}"
            );
            assert!(now >= previous - 1e-6, "dawn must brighten monotonically");
            previous = now;
        }
    }
    #[test]
    fn dense_cloud_path_matches_the_world_transmission_and_clears_above_cloud() {
        let band = [1000., 2200., 1., 6000.];
        let at = |height, sun| sunlight_transmission(std::iter::once(band), height, sun);
        assert!((at(1600., 1.) - (-1_f32).exp()).abs() < 1e-6);
        assert!((at(1600., 0.5) - (-2_f32).exp()).abs() < 1e-6);
        assert_eq!(at(2300., 0.5), 1.);
        assert_eq!(
            sunlight_transmission(std::iter::once([1000., 2200., 0.5, 6000.]), 1600., 1.),
            1.
        );
    }
    #[test]
    fn flood_reflects_original_paint_even_when_ambient_is_almost_black() {
        let original = [100, 120, 140, 255];
        let mut day = original;
        surface_pixel(&mut day, 1., 0.);
        assert_eq!(day, original);
        let mut night = original;
        surface_pixel(&mut night, 0.065, 0.);
        assert_eq!(night, [6, 7, 9, 255]);
        let mut flood = original;
        surface_pixel(&mut flood, 0.065, 0.6);
        assert!(flood[0] > 35 && flood[1] < 10 && flood[2] < 10);
        assert_eq!(flood[3], 255);
        let mut clear = [0; 4];
        surface_pixel(&mut clear, 0.065, 1.);
        assert_eq!(clear, [0; 4]);
        // Preserve the previous default flood's noon arithmetic exactly.
        assert_eq!(Lighting::default().flood_strength(1.), 0.6 * (1. - 0.97));
    }
    #[test]
    fn generated_legends_have_subtle_dimmable_backlighting() {
        assert_eq!(legend_ink([200; 3], 1., 0., 1.), [200, 200, 200, 255]);
        let night = legend_ink([200; 3], 0.065, 0., 1.);
        assert_eq!(night, [33, 33, 33, 255]);
        let dim = legend_ink([200; 3], 0.065, 0., 0.25);
        assert_eq!(dim, [8, 8, 8, 255]);
    }
    #[test]
    fn preview_is_bounded() {
        assert_eq!(preview("4,3,0,0").unwrap(), (Lighting::default(), 0));
        for bad in [
            "0,3,0,0",
            "4,6,0,0",
            "4,3,4,0",
            "4,3,0,257",
            "4,3,0",
            "a,3,0,0",
        ] {
            assert!(preview(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn steps_saturate_independently_and_default_is_identity() {
        let mut light = Lighting::default();
        assert_eq!(light.instrument_gain(), 1.);
        assert_eq!(light.hud([22, 155, 4], 0), [22, 155, 4]);
        for _ in 0..20 {
            light.step(false, -1);
        }
        assert_eq!((light.instruments, light.flood), (1, 3));
        for _ in 0..20 {
            light.step(true, -1);
        }
        assert_eq!(light.flood_strength(-1.), 0.);
        for _ in 0..20 {
            light.step(true, 1);
            light.step(false, 1);
        }
        assert_eq!((light.instruments, light.flood), (5, 5));
        assert!((light.flood_strength(1.) - 0.03).abs() < 1e-6);
        assert_eq!(light.flood_strength(-1.), 1.5);
        for colour in 1..4 {
            light.colour = colour;
            assert_eq!(light.hud([0; 3], -256), [0; 3]);
            assert_eq!(light.hud([0; 3], 256), [255; 3]);
        }
    }
}
