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
    pub fn flood_strength(self, sun_height: f32) -> f32 {
        let day = ((sun_height + 0.05) / 0.4).clamp(0., 1.);
        let day = day * day * (3. - 2. * day);
        f32::from(self.flood.min(5)) / 5. * (1. - 0.97 * day)
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

/// Reflected red light on a bezel. Screen pixels never use this operation.
pub fn flood_pixel(pixel: &mut [u8], strength: f32) {
    let red = f32::from(pixel[0]);
    pixel[0] = (red + (22. + red * 0.35) * strength).min(255.) as u8;
    pixel[1] = (f32::from(pixel[1]) * (1. - 0.12 * strength)) as u8;
    pixel[2] = (f32::from(pixel[2]) * (1. - 0.16 * strength)) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(light.flood_strength(-1.), 1.);
        for colour in 1..4 {
            light.colour = colour;
            assert_eq!(light.hud([0; 3], -256), [0; 3]);
            assert_eq!(light.hud([0; 3], 256), [255; 3]);
        }
    }
}
