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
            && (!(-360..=360).contains(&heading) || !(0..=200).contains(&speed))
        {
            return Err(std::io::Error::other(format!(
                "mission wind heading must be -360..=360 degrees and speed 0..=200 ft/s (got {heading}, {speed})"
            )));
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
    fn signed_mission_metadata_loads_with_its_direction_and_speed() {
        for heading in [-360, -155, -76, -51, 0, 90, 360] {
            // Synthetic metadata exercises the same parse-to-weather boundary
            // as theater startup, without committing any retail mission bytes.
            let source = format!("textFormat\nmap TEST.T2\nwind {heading} 20\n");
            let metadata = tore_formats::theater::Environment::parse(source.as_bytes()).unwrap();
            let wind = Wind::resolve(metadata.wind, &mut NativeRng::seeded(1).unwrap()).unwrap();
            assert_eq!(wind.heading_binary, (heading * 182) as i16);
            assert_eq!(wind.speed_fps, 20);
            assert_eq!(wind.origin, Origin::Mission);
            let world = wind.world_fps();
            assert!((world[0].hypot(world[2]) - 20.).abs() < 1e-10);
            if heading == -76 {
                assert!(world[0] < 0. && world[2] > 0.);
            }
            if heading == -155 {
                assert!(world[0] < 0. && world[2] < 0.);
            }
        }
        let calm = Wind::resolve(Some([-155, 0]), &mut NativeRng::seeded(1).unwrap()).unwrap();
        assert_eq!(calm.world_fps(), [0.; 3]);
    }

    #[test]
    fn signed_headings_do_not_relax_other_wind_bounds() {
        for value in [
            [-361, 20],
            [361, 20],
            [i32::MIN, 20],
            [i32::MAX, 20],
            [-76, -1],
            [-76, 201],
        ] {
            let mut rng = NativeRng::seeded(1).unwrap();
            let before = rng.clone();
            let error = Wind::resolve(Some(value), &mut rng).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(&format!("got {}, {}", value[0], value[1]))
            );
            assert_eq!(rng, before);
        }
        for heading in [-360, 360] {
            assert!(
                Wind::resolve(Some([heading, 200]), &mut NativeRng::seeded(1).unwrap()).is_ok()
            );
        }
    }

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
