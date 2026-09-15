//! FA 0x4b31f0: two ordered indexed haze remaps along one line of sight.
use super::{Environment, clamp_altitude};
use tore_formats::weather::Layer;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RayRemaps {
    pub view_layer: usize,
    pub view_density: i32,
    /// Applied first; None means the source identity table.
    pub target: Option<(usize, i32)>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ramp(pub [i32; 4]);
impl From<&Layer> for Ramp {
    fn from(l: &Layer) -> Self {
        Self([l.fog_near, l.fog_near_density, l.fog_far, l.fog_far_density])
    }
}
impl Ramp {
    fn restricted(self, view: Self) -> Self {
        Self([
            self.0[0].min(view.0[0]),
            self.0[1].max(view.0[1]),
            self.0[2].min(view.0[2]),
            self.0[3].max(view.0[3]),
        ])
    }
    pub fn density(self, distance: i32) -> i32 {
        let [near, a, far, b] = self.0;
        if distance <= near {
            a
        } else if distance >= far {
            b
        } else {
            a + ((i64::from(b - a) * i64::from(distance - near)) / i64::from(far - near)) as i32
        }
        .clamp(0, 256)
    }
}
impl Environment {
    /// `bias_f8` is an explicit presentation input, zero for normal views.
    /// Native signed WORD distance is retained. Distance products retain source wrapping DWORD arithmetic; no visibility-sensor claim.
    pub fn ray_remaps(
        &self,
        view_alt: f64,
        target_alt: f64,
        distance: f64,
        bias_f8: i32,
    ) -> Option<RayRemaps> {
        if !distance.is_finite() {
            return None;
        }
        let view = clamp_altitude(view_alt);
        let target = clamp_altitude(target_alt);
        let vi = self.active.iter().position(|l| l.covers_altitude(view))?;
        let ti = self.active.iter().position(|l| l.covers_altitude(target))?;
        let overlap = self.active.get(vi + 1).is_some_and(|l| l.low_feet <= view);
        let distance =
            ((((distance.max(0.) * 256.).floor() as i64).saturating_add(i64::from(bias_f8))).max(0)
                >> 16) as i16 as i32;
        let mut view_ramp = Ramp::from(&self.active[vi]);
        let mut target_ramp = Ramp::from(&self.active[ti]);
        if overlap {
            let blended = Ramp::from(&self.sample(view_alt)?);
            view_ramp = view_ramp.restricted(blended);
            target_ramp = target_ramp.restricted(blended);
        }
        let (vd, td) = if vi == ti {
            (distance, 0)
        } else if ti + 1 < vi || ti > vi + 1 || (overlap && ti < vi) {
            (65535, 65535)
        } else if vi > ti {
            let v = (view - self.active[ti].high_feet).wrapping_mul(distance) / (view - target);
            (v, distance - v)
        } else {
            let t = (target - self.active[vi].high_feet).wrapping_mul(distance) / (target - view);
            (distance - t, t)
        };
        Some(RayRemaps {
            view_layer: vi,
            view_density: view_ramp.density(vd),
            target: (td > 0).then(|| (ti, target_ramp.density(td))),
        })
    }
    pub fn remap_index(&self, ray: RayRemaps, mut index: u8) -> u8 {
        if let Some((layer, density)) = ray.target {
            index = self
                .configuration
                .shade_remap(self.active[layer].shade)
                .level(density)[usize::from(index)];
        }
        self.configuration
            .shade_remap(self.active[ray.view_layer].shade)
            .level(ray.view_density)[usize::from(index)]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Configuration;
    use tore_formats::weather::{Module, synthetic_module};
    fn environment() -> Environment {
        let mut m = Module::parse(&synthetic_module(3)).unwrap();
        for (i, l) in m.layers.iter_mut().enumerate() {
            l.start_seconds = 0;
            l.end_seconds = i32::MAX;
            l.low_feet = i as i32 * 1000;
            l.high_feet = if i == 2 {
                i32::MAX
            } else {
                (i as i32 + 1) * 1000
            };
            l.fog_near = 0;
            l.fog_far = 100;
            l.fog_near_density = 0;
            l.fog_far_density = 256;
        }
        Environment::new(Configuration::new(m, 12, 0, 0, None).unwrap())
    }
    #[test]
    fn distance_split_identity_and_distant_band_saturation() {
        let e = environment();
        let before = e.clone();
        assert_eq!(
            e.ray_remaps(500., 500., 25600., 0),
            Some(RayRemaps {
                view_layer: 0,
                view_density: 256,
                target: None
            })
        );
        assert_eq!(
            e.ray_remaps(500., 1500., 25600., 0),
            Some(RayRemaps {
                view_layer: 0,
                view_density: 128,
                target: Some((1, 128))
            })
        );
        assert_eq!(
            e.ray_remaps(1500., 500., 25600., 0),
            Some(RayRemaps {
                view_layer: 1,
                view_density: 128,
                target: Some((0, 128))
            })
        );
        assert_eq!(
            e.ray_remaps(500., 2500., 100., 0),
            Some(RayRemaps {
                view_layer: 0,
                view_density: 256,
                target: Some((2, 256))
            })
        );
        assert_eq!(e.ray_remaps(500., 500., 255., 0).unwrap().view_density, 0);
        assert_eq!(e.ray_remaps(500., 500., 255., 256).unwrap().view_density, 2);
        // Signed WORD distance wraps at 32,768 source distance units.
        assert_eq!(
            e.ray_remaps(500., 500., 8_388_608., 0)
                .unwrap()
                .view_density,
            0
        );
        assert!(e.ray_remaps(500., 500., f64::NAN, 0).is_none());
        assert_eq!(e, before);
    }
    #[test]
    fn overlap_restricts_both_ramps_and_remap_order_matters() {
        let mut e = environment();
        e.active[0].high_feet = 1500;
        e.active[1].fog_far_density = 128;
        let ray = e.ray_remaps(1250., 1700., 25600., 0).unwrap();
        assert_eq!(ray.view_layer, 0);
        assert!(ray.target.is_some());
        // Synthetic noncommuting tables distinguish target-then-view ordering.
        let mut a = e.configuration.module.shades[0].clone();
        a.color = [1, 1, 1];
        for table in &mut a.levels {
            for (i, v) in table.iter_mut().enumerate() {
                *v = i.saturating_add(1).min(255) as u8;
            }
        }
        let mut b = a.clone();
        b.color = [2, 2, 2];
        for table in &mut b.levels {
            for (i, v) in table.iter_mut().enumerate() {
                *v = (i / 2) as u8;
            }
        }
        e.configuration.module.shades = vec![a, b];
        e.active[0].shade = [1, 1, 1];
        e.active[1].shade = [2, 2, 2];
        assert_eq!(e.remap_index(ray, 10), 6);
    }
}
