//! The guided weapons' records, glide, cones and reach on their own (slice
//! W1): retail records from docs/formats/missiles.md, "Laser and designator"
//! and "Optical".
use super::*;
use crate::combat::live::guided_tests::{at2, gbu10, gbu29};
use crate::sensors::Controls;

const KNOT: f64 = 1.687_8;

fn level(altitude: f64, knots: f64) -> Launcher {
    let basis = Basis::new(0., 0., 0.);
    let speed = knots * KNOT;
    Launcher {
        position: [0., altitude, 0.],
        basis,
        speed_fps: speed,
        velocity: basis.forward.map(|v| v * speed),
        bay_ready: true,
        radar_power: true,
        radar: true,
        jammer: false,
        alive: true,
        body_present: true,
        controls: Controls::default(),
    }
}

#[test]
fn the_kinds_come_from_the_signature_and_the_type_flags() {
    assert_eq!(kind(&gbu10()), Some(Guidance::Laser));
    assert_eq!(kind(&gbu29()), Some(Guidance::Coordinate));
    assert_eq!(kind(&at2()), Some(Guidance::Command));
    // AS-14: the laser bits on a powered, cruising missile.
    let mut as14 = at2();
    as14.flags = 0x2a56f;
    as14.seeker.signature = 1;
    assert_eq!(kind(&as14), Some(Guidance::Laser));
    assert!(!is_glide_bomb(&as14));
    assert!(is_glide_bomb(&gbu10()) && is_glide_bomb(&gbu29()));
    // A Mk 82 (bomb flag 0x10), an AS-7 (radar, support 0x200) and an
    // unguided rocket are not these weapons.
    let mut mk82 = gbu29();
    mk82.flags = 0x22012;
    assert_eq!(kind(&mk82), None);
    let mut as7 = at2();
    as7.flags = 0x2236f;
    as7.seeker.signature = 3;
    assert_eq!(kind(&as7), None);
    let mut rocket = at2();
    rocket.flags &= !0x1;
    assert_eq!(kind(&rocket), None);
}

#[test]
fn a_guided_bomb_leaves_along_the_nose_at_ninety_percent_and_glides_to_278() {
    let launcher = level(10_000., 450.);
    let v = release_velocity(&gbu10(), &launcher);
    // 450 kt is 759 ft/s whole; retard 90 gives 683.
    assert_eq!(v, [0., 0., 683.]);
    // Straight on, the glide covers its 60 s at 14 ft/s² down to 278 ft/s:
    // 405 / 14 = 28.9 s slowing over 13,900 ft, then 278 ft/s.
    let motion = Motion::launch(&gbu10(), v, 10_000.);
    let slowing = 405. / 14.;
    let slowed = (683. + 278.) / 2. * slowing;
    let ahead = |d: f64| [0., 10_000., d];
    let from = [0., 10_000., 0.];
    let t = time_to(
        &gbu10(),
        motion,
        from,
        [0., 0., 1.],
        ahead(slowed + HIT_FT),
        0,
    )
    .unwrap();
    assert!((t - slowing).abs() < 0.01, "{t}");
    let far = slowed + 278. * (60. - slowing);
    assert!(time_to(&gbu10(), motion, from, [0., 0., 1.], ahead(far - 200.), 0).is_some());
    assert!(time_to(&gbu10(), motion, from, [0., 0., 1.], ahead(far + 200.), 0).is_none());
}

#[test]
fn the_cone_is_45_degrees_each_way_about_the_nose() {
    let w = gbu10();
    let basis = Basis::new(0., 0., 0.);
    let at = |degrees: f64| {
        let r = degrees.to_radians();
        [0., -1_000. * r.sin(), 1_000. * r.cos()]
    };
    assert!(in_cone(&w, [0.; 3], basis, at(44.9)));
    assert!(!in_cone(&w, [0.; 3], basis, at(45.1)));
    // Zone 1's 30,000 ft at release, halved by cloud for a laser weapon.
    let far = [0., -10_000., 25_000.];
    assert!(in_release_zone(&w, [0.; 3], basis, far, 1.));
    assert!(!in_release_zone(&w, [0.; 3], basis, far, 0.5));
}

#[test]
fn the_reach_starts_at_the_cone_and_ends_where_the_glide_runs_out() {
    let w = gbu10();
    let launcher = level(8_000., 450.);
    let reach = reach(&w, &launcher, 0., 1.).unwrap();
    // Level, the 45 degree cone starts at the height.
    assert!((reach.minimum_ft - 8_000.).abs() < 10., "{reach:?}");
    // The far edge: a point just inside is reached, just beyond is not.
    let motion = Motion::launch(&w, release_velocity(&w, &launcher), 8_000.);
    let point = |range: f64| [0., 0., range];
    let fly = |range: f64| time_to(&w, motion, launcher.position, [0., 0., 1.], point(range), 0);
    assert!(fly(reach.maximum_ft - 50.).is_some());
    assert!(fly(reach.maximum_ft + 50.).is_none());
    assert!(reach.contains(12_000.) && !reach.contains(5_000.));
    assert_eq!(reach.fraction(reach.minimum_ft), 0.);
    assert_eq!(reach.fraction(reach.maximum_ft), 1.);
}

#[test]
fn the_reach_shrinks_with_height_and_grows_with_speed() {
    // The guided glide has no fall (retail: no gravity flag), so its slant
    // reach is fixed by its speeds and life: a higher release spends more of
    // it going down, and the far edge comes in, while the cone's near edge
    // goes out. A faster release reaches farther.
    let w = gbu10();
    let mut last: Option<Reach> = None;
    for altitude in [2_000., 5_000., 8_000., 11_000., 14_000.] {
        let reach = reach(&w, &level(altitude, 450.), 0., 1.).unwrap();
        if let Some(last) = last {
            assert!(
                reach.maximum_ft <= last.maximum_ft,
                "{altitude}: {reach:?} {last:?}"
            );
            assert!(reach.minimum_ft >= last.minimum_ft);
        }
        last = Some(reach);
    }
    let slow = reach(&w, &level(8_000., 350.), 0., 1.).unwrap();
    let fast = reach(&w, &level(8_000., 550.), 0., 1.).unwrap();
    assert!(fast.maximum_ft > slow.maximum_ft);
    // Too high for the glide to reach anything inside the cone: no reach.
    assert!(reach(&w, &level(20_000., 350.), 0., 1.).is_none());
}

#[test]
fn a_missile_reach_is_its_zone_with_no_fly_out() {
    let mut as14 = at2();
    as14.flags = 0x2a56f;
    as14.seeker.signature = 1;
    as14.seeker.zones[1].maximum_range = 40_000;
    let reach = reach(&as14, &level(3_000., 400.), 0., 1.).unwrap();
    assert!((reach.maximum_ft - (40_000_f64.powi(2) - 3_000_f64.powi(2)).sqrt()).abs() < 1.);
    // Only the cone keeps the nearest points out (45 degrees down).
    assert!((reach.minimum_ft - 3_000.).abs() < 20., "{reach:?}");
}
