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
/// Diagnostic nearest-copy query. Rendering uses `centers_for_view`.
pub fn centers(layout: &Layout, eye: [f64; 3], altitude: i32) -> Vec<([f64; 3], i16)> {
    centers_for_view(
        layout,
        eye,
        altitude,
        View {
            heading: 0,
            pitch: -16380,
            detail: 2,
            radius_feet: 0,
        },
    )
}

/// FA 0x4a9660 publishes these binary-angle view sectors; 0x4a8130
/// relocates copies wholly behind them. Roll does not enter this query.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub heading: i16,
    pub pitch: i16,
    pub detail: u8,
    pub radius_feet: i32,
}
pub fn centers_for_view(
    layout: &Layout,
    eye: [f64; 3],
    altitude: i32,
    view: View,
) -> Vec<([f64; 3], i16)> {
    if layout.validate().is_err()
        || view.detail > 2
        || !(0..=1_000_000).contains(&view.radius_feet)
        || altitude <= 0
        || eye.iter().any(|v| !v.is_finite() || v.abs() > 8_000_000.)
    {
        return Vec::new();
    }
    let count = 1i64
        << if view.detail >= 2 {
            layout.subdivisions
        } else {
            0
        };
    let period = i64::from(layout.period_f8) * count;
    let cell = |v: f64| ((v * 256.).floor() as i64) & !(period - 1);
    let h = view.heading;
    let sectors = [
        ((5460..=27300).contains(&h), (-27300..=-5460).contains(&h)),
        (
            (-10920..=10920).contains(&h),
            !(-21839..=21839).contains(&h),
        ),
    ];
    let wrap = |offset: i64, eye: f64, sector: (bool, bool)| {
        let mut v = cell(eye) + offset;
        let delta = (eye * 256.).floor() as i64 - v;
        if delta > period / 2 {
            v += period;
        } else if delta < -(period / 2) {
            v -= period;
        }
        let eye = (eye * 256.).floor() as i64;
        let radius = i64::from(view.radius_feet) * 256;
        if view.pitch >= -8190 {
            if sector.0 && v + radius < eye {
                v += period;
            } else if sector.1 && v - radius > eye {
                v -= period;
            }
        }
        v as f64 / 256.
    };
    let mut out = Vec::new();
    for x in 0..count {
        for z in 0..count {
            for p in &layout.patches {
                out.push((
                    [
                        wrap(
                            i64::from(p.x_f8) + x * i64::from(layout.period_f8),
                            eye[0],
                            sectors[0],
                        ),
                        f64::from(altitude),
                        wrap(
                            i64::from(p.z_f8) + z * i64::from(layout.period_f8),
                            eye[2],
                            sectors[1],
                        ),
                    ],
                    p.yaw,
                ));
            }
        }
    }
    out
}
/// FA 0x4d057c rejects shapes before frustum clipping when any camera-relative
/// component cannot fit the source signed coordinate after the SH scale shift.
/// This is an observable distance gate, not replaceable by GPU far-plane clipping.
pub fn within_shape_range(eye: [f64; 3], center: [f64; 3], exponent: u16) -> bool {
    (8..=20).contains(&exponent)
        && eye.into_iter().zip(center).all(|(e, c)| {
            e.is_finite()
                && c.is_finite()
                && e.abs() <= 8_000_000.
                && c.abs() <= 8_000_000.
                && (((e * 256.).floor() as i64 - (c * 256.).floor() as i64) >> exponent).abs()
                    < 32767
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::weather::clouds::Patch;
    #[test]
    fn source_shape_range_retains_signed_shift_boundaries() {
        assert!(within_shape_range([0.; 3], [131064., 0., 0.], 10));
        assert!(!within_shape_range(
            [0.; 3],
            [131064. + 1. / 256., 0., 0.],
            10
        ));
        assert!(within_shape_range(
            [0.; 3],
            [-131068. + 1. / 256., 0., 0.],
            10
        ));
        assert!(!within_shape_range([0.; 3], [-131068., 0., 0.], 10));
        assert!(!within_shape_range([0.; 3], [0., 131068., 0.], 10));
        assert!(!within_shape_range([f64::NAN; 3], [0.; 3], 10));
    }
    #[test]
    fn detail_and_forward_sectors_preserve_bounds_and_downward_view() {
        let layout = Layout {
            period_f8: 4096,
            subdivisions: 2,
            patches: vec![Patch {
                mask: 3,
                x_f8: 256,
                z_f8: 256,
                yaw: 123,
            }],
        };
        let mut view = View {
            heading: 0,
            pitch: 0,
            detail: 0,
            radius_feet: 1,
        };
        let query = |v| centers_for_view(&layout, [4., 0., 4.], 7000, v);
        assert_eq!(query(view), vec![([1., 7000., 17.], 123)]);
        view.detail = 1;
        assert_eq!(query(view), vec![([1., 7000., 17.], 123)]);
        view.detail = 2;
        assert_eq!(query(view).len(), 16);
        view.detail = 0;
        for (heading, expected) in [
            (5459, [1., 7000., 17.]),
            (5460, [17., 7000., 17.]),
            (10920, [17., 7000., 17.]),
            (10921, [17., 7000., 1.]),
            (27300, [17., 7000., 1.]),
            (27301, [1., 7000., 1.]),
        ] {
            view.heading = heading;
            assert_eq!(query(view)[0].0, expected);
        }
        view.heading = 8190;
        view.pitch = -8190;
        assert_eq!(query(view)[0].0, [17., 7000., 17.]);
        view.pitch = -8191;
        assert_eq!(query(view)[0].0, [1., 7000., 1.]);
        view.pitch = 0;
        view.radius_feet = 3;
        assert_eq!(query(view)[0].0, [1., 7000., 1.]); // tangent, strict test
        view.radius_feet = 0;
        view.heading = -8190;
        assert_eq!(
            centers_for_view(&layout, [0.; 3], 7000, view)[0].0,
            [-15., 7000., 1.]
        );
        view.heading = -27300;
        assert_eq!(
            centers_for_view(&layout, [0.; 3], 7000, view)[0].0,
            [-15., 7000., -15.]
        );
        view.heading = -27301;
        assert_eq!(
            centers_for_view(&layout, [0.; 3], 7000, view)[0].0,
            [1., 7000., -15.]
        );
        assert_eq!(query(view), query(view));
    }
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
