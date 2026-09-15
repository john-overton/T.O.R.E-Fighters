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

#[cfg(test)]
mod tests {
    use super::*;
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
