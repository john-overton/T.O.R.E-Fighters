//! Source cloud repeats and launch defaults; pure queries cannot consume RNG.
use tore_formats::{Result, flight_model::clock_rng::NativeRng, weather::clouds::Layout};

pub fn generated_altitude(condition: usize, rng: &mut NativeRng) -> Result<i32> {
    let choice = crate::environment::CONDITIONS
        .get(condition)
        .ok_or_else(|| std::io::Error::other("cloud weather choice"))?;
    Ok(if choice.scattered_clouds && rng.chance(50)? {
        7000 + rng.below(13000)?
    } else {
        0
    })
}
/// FA 0x4a8090/0x4a8130: repeat over a 4x4 supercell at detail >=2,
/// choose the nearest periodic representative with strict half-period tests.
/// View-dependent frustum optimizations are replaced by GPU clipping.
pub fn centers(layout: &Layout, eye: [f64; 3], altitude: i32) -> Vec<([f64; 3], i16)> {
    if layout.validate().is_err()
        || altitude <= 0
        || eye.iter().any(|v| !v.is_finite() || v.abs() > 8_000_000.)
    {
        return Vec::new();
    }
    let count = 1i64 << layout.subdivisions;
    let period = i64::from(layout.period_f8) * count;
    let cell = |v: f64| ((v * 256.).floor() as i64) & !(period - 1);
    let wrap = |offset: i64, eye: f64| {
        let mut v = cell(eye) + offset;
        let delta = (eye * 256.).floor() as i64 - v;
        if delta > period / 2 {
            v += period;
        } else if delta < -(period / 2) {
            v -= period;
        }
        v as f64 / 256.
    };
    let mut out = Vec::new();
    for x in 0..count {
        for z in 0..count {
            for p in &layout.patches {
                out.push((
                    [
                        wrap(i64::from(p.x_f8) + x * i64::from(layout.period_f8), eye[0]),
                        f64::from(altitude),
                        wrap(i64::from(p.z_f8) + z * i64::from(layout.period_f8), eye[2]),
                    ],
                    p.yaw,
                ));
            }
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::weather::clouds::Patch;
    #[test]
    fn repeated_clouds_are_query_stable_and_wrap_at_far_boundary() {
        let layout = Layout {
            period_f8: 1024,
            subdivisions: 2,
            patches: vec![Patch {
                mask: 1,
                x_f8: 256,
                z_f8: 512,
                yaw: 0,
            }],
        };
        let a = centers(&layout, [0., 0., 0.], 7000);
        assert_eq!(a.len(), 16);
        assert_eq!(a, centers(&layout, [0., 1000., 0.], 7000));
        let b = centers(&layout, [16., 0., 16.], 7000);
        for (a, b) in a.iter().zip(b) {
            assert_eq!(b.0, [a.0[0] + 16., 7000., a.0[2] + 16.]);
        }
        assert!(centers(&layout, [0.; 3], 0).is_empty());
        let single = Layout {
            subdivisions: 0,
            ..layout.clone()
        };
        assert_eq!(centers(&single, [3., 0., 0.], 7000)[0].0[0], 1.);
        assert_eq!(centers(&single, [3. + 1. / 256., 0., 0.], 7000)[0].0[0], 5.);
        let invalid = Layout {
            subdivisions: 255,
            ..layout.clone()
        };
        assert!(centers(&invalid, [0.; 3], 7000).is_empty());
        let mut saw = [false; 2];
        for seed in 1..80 {
            let mut rng = NativeRng::seeded(seed).unwrap();
            let alt = generated_altitude(0, &mut rng).unwrap();
            assert!(alt == 0 || (7000..20000).contains(&alt));
            saw[usize::from(alt != 0)] = true;
        }
        assert_eq!(saw, [true, true]);
        for condition in [1, 2, 5] {
            let mut rng = NativeRng::seeded(1).unwrap();
            let before = rng.clone();
            assert_eq!(generated_altitude(condition, &mut rng).unwrap(), 0);
            assert_eq!(rng, before);
        }
    }
}
