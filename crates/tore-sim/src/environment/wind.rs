//! Mission wind resolution. Native heading units are retained at construction.
use tore_formats::{Result, flight_model::clock_rng::NativeRng};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Mission,
    /// Native default draw bounds/order with a dedicated authored launch stream.
    Generated,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wind {
    pub heading_binary: i16,
    pub speed_fps: i32,
    pub origin: Origin,
}
impl Wind {
    /// FA 0x4808fb..0x48091c initializes heading then speed before mission parsing;
    /// 0x481e70..0x481e80 replaces both when a wind line exists. Explicit calm
    /// stays calm. Always draw the defaults, preserving initialization order.
    pub fn resolve(mission: Option<[i32; 2]>, rng: &mut NativeRng) -> Result<Self> {
        if let Some([heading, speed]) = mission
            && (!(0..=360).contains(&heading) || !(0..=200).contains(&speed))
        {
            return Err(std::io::Error::other("mission wind outside source range"));
        }
        let heading_binary = rng.below(0xfff0)? as i16;
        let speed_fps = rng.below(0x16)? + 7;
        Ok(match mission {
            Some([heading, speed]) => Self {
                heading_binary: (heading * 182) as i16,
                speed_fps: speed,
                origin: Origin::Mission,
            },
            None => Self {
                heading_binary,
                speed_fps,
                origin: Origin::Generated,
            },
        })
    }
    /// Native position arithmetic drifts toward this heading; source meteorological
    /// naming remains unverified. Float sin/cos is the existing host adaptation.
    pub fn world_fps(self) -> [f64; 3] {
        let radians = f64::from(self.heading_binary) * std::f64::consts::TAU / 65536.;
        let speed = f64::from(self.speed_fps);
        [speed * radians.sin(), 0., speed * radians.cos()]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_have_source_bounds_draw_order_and_explicit_calm_wins() {
        for seed in [1, 2, 97, 1000] {
            let mut rng = NativeRng::seeded(seed).unwrap();
            let mut expected = rng.clone();
            let heading = expected.below(0xfff0).unwrap() as i16;
            let speed = expected.below(22).unwrap() + 7;
            let wind = Wind::resolve(None, &mut rng).unwrap();
            assert_eq!((wind.heading_binary, wind.speed_fps), (heading, speed));
            assert!((7..=28).contains(&speed));
            assert_eq!(rng, expected);
            let calm = Wind::resolve(Some([90, 0]), &mut NativeRng::seeded(seed).unwrap()).unwrap();
            assert_eq!(calm.world_fps(), [0.; 3]);
            assert_eq!(calm.origin, Origin::Mission);
        }
    }
}
