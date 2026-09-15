//! FA 0x4aacf0 / 0x4c942c background bands, normal full-detail view.
//! Screen rasterization is a host projection adaptation; no Earth-curvature dip.
use tore_formats::weather::Layer;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Horizon {
    pub upper_band: bool,
    pub lower_band: bool,
    pub lower_extent: i32,
}
impl Horizon {
    pub fn new(layer: &Layer, altitude: f64) -> Self {
        let altitude = super::clamp_altitude(altitude);
        let sky = !layer.decks[0].name.is_empty();
        let above_sky = sky && altitude >= layer.decks[0].altitude_feet;
        let ocean = !layer.decks[1].name.is_empty() && altitude > layer.decks[1].altitude_feet;
        Self {
            upper_band: !sky || above_sky,
            lower_band: !above_sky && !ocean,
            lower_extent: (130 * i64::from(altitude) / 15000).clamp(10, 130) as i32,
        }
    }
    pub fn flags(self) -> u8 {
        u8::from(self.upper_band) | (u8::from(self.lower_band) << 1)
    }
}

/// FA 0x4aad7e: integer degrees classify inversion; native clip dimensions
/// choose the solid lower horizon offset. Units are added to the Q15 view
/// plane's half-sized constant, not feet or an Earth-curvature horizon dip.
pub fn lower_solid_offset(size: [u32; 2], roll: i16) -> i16 {
    let inverted = !(-90..=90).contains(&(roll / 182));
    if size[0] >= 300 && size[1] >= 190 {
        if inverted { 90 } else { 20 }
    } else if size[0] >= 200 && size[1] >= 100 {
        if inverted { 200 } else { 0 }
    } else if inverted {
        300
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solid_horizon_uses_clip_size_and_integer_roll_thresholds() {
        assert_eq!(lower_solid_offset([640, 480], 0), 20);
        assert_eq!(lower_solid_offset([640, 480], 16561), 20);
        assert_eq!(lower_solid_offset([640, 480], 16562), 90);
        assert_eq!(lower_solid_offset([640, 480], -16562), 90);
        assert_eq!(lower_solid_offset([299, 190], 20000), 200);
        assert_eq!(lower_solid_offset([300, 189], 20000), 200);
        assert_eq!(lower_solid_offset([200, 100], 0), 0);
        assert_eq!(lower_solid_offset([199, 100], 20000), 300);
    }
    #[test]
    fn deck_crossings_and_band_extent_follow_source_boundaries() {
        let mut layer =
            tore_formats::weather::Module::parse(&tore_formats::weather::synthetic_module(1))
                .unwrap()
                .layers
                .remove(0);
        layer.decks[0].name.clear();
        layer.decks[1].name.clear();
        let low = Horizon::new(&layer, 0.);
        assert_eq!(low.flags(), 3);
        assert_eq!(low.lower_extent, 10);
        assert_eq!(Horizon::new(&layer, 7500.).lower_extent, 65);
        assert_eq!(Horizon::new(&layer, 15000.).lower_extent, 130);
        layer.decks[0].name = "SYNTH.PIC".into();
        layer.decks[0].altitude_feet = 75000;
        assert_eq!(Horizon::new(&layer, 74999.).flags(), 2);
        assert_eq!(Horizon::new(&layer, 75000.).flags(), 1);
        layer.decks[1].name = "WATER.PIC".into();
        assert_eq!(Horizon::new(&layer, 1.).flags(), 0);
        assert_eq!(Horizon::new(&layer, 0.).flags(), 2);
    }
}
