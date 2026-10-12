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

/// A sight with the aim `range` feet out, the reach 8,000 to 19,000 ft, and
/// the T at `steering` radians and 760 ft/s.
fn tee(range: f64, steering: f64, in_reach: bool) -> GuidedSight {
    GuidedSight {
        kind: Guidance::Coordinate,
        aim: Some([0., 0., range]),
        buddy: false,
        ground_range_ft: Some(range),
        slant_range_ft: Some(range),
        reach: Some(Reach {
            minimum_ft: 8_000.,
            maximum_ft: 19_000.,
        }),
        in_reach,
        advisory: (!in_reach).then_some(Advisory::OutOfReach),
        level: Some(GuidedLevel {
            steering_rad: steering,
            ground_speed_fps: 760.,
        }),
    }
}

#[test]
fn the_bar_stands_two_fifths_in_from_the_far_edge() {
    let sight = tee(30_000., 0., false);
    // 19,000 less 0.4 of the 11,000 ft reach.
    assert!((sight.release_range_ft().unwrap() - 14_600.).abs() < 1e-6);
    assert!((sight.along_ft().unwrap() - 15_400.).abs() < 1e-6);
    assert!((sight.release_seconds().unwrap() - 15_400. / 760.).abs() < 1e-9);
    // The marker rides at the top while 15 s or more away, the stem is dashed
    // beyond a minute.
    assert_eq!(sight.marker_fraction(), 1.);
    assert!(!sight.far());
    assert!(tee(70_000., 0., false).far());
    let near = tee(14_600. + 760. * 7.5, 0., true);
    assert!((near.marker_fraction() - 0.5).abs() < 1e-9);
}

#[test]
fn a_held_trigger_waits_for_the_bar_then_releases_inside_the_window() {
    let tick = 760. / 120.;
    // Far out, in the reach but before the bar, or short of it: waits.
    assert_eq!(tee(30_000., 0., false).level_trigger(), Some(false));
    assert_eq!(tee(16_000., 0., true).level_trigger(), Some(false));
    // On the bar and up to three ticks past it: releases.
    assert_eq!(tee(14_600., 0., true).level_trigger(), Some(true));
    assert_eq!(
        tee(14_600. - 2. * tick, 0., true).level_trigger(),
        Some(true)
    );
    // Past the window: a plain press again; nearer than the reach: overflown.
    assert_eq!(tee(14_600. - 4. * tick, 0., true).level_trigger(), None);
    assert_eq!(tee(7_000., 0., false).level_trigger(), None);
    assert!(tee(7_000., 0., false).overflown());
    // Behind the aircraft is overflown whatever the range says.
    assert!(tee(16_000., 3., true).overflown());
}

#[test]
fn the_bar_releases_only_with_the_aim_in_the_reach_and_near_the_track() {
    // Off the track by 8 degrees releases, by 12 degrees waits on the stem.
    assert_eq!(
        tee(14_600., 8f64.to_radians(), true).level_trigger(),
        Some(true)
    );
    assert_eq!(
        tee(14_600., -12f64.to_radians(), true).level_trigger(),
        Some(false)
    );
    // The aim outside the reach at the bar (out of the cone) does not release.
    assert_eq!(tee(14_600., 0., false).level_trigger(), Some(false));
    // No level, no hold.
    let mut plain = tee(14_600., 0., true);
    plain.level = None;
    assert_eq!(plain.level_trigger(), None);
    assert_eq!(plain.release_seconds(), None);
}

#[test]
fn the_steering_is_the_turn_from_the_track_to_the_aim() {
    let l = level(8_000., 450.);
    let to = |x: f64, z: f64| steering_to(&l, [x, 0., z]).unwrap();
    assert!(to(0., 10_000.).abs() < 1e-12);
    assert!((to(10_000., 10_000.) - std::f64::consts::FRAC_PI_4).abs() < 1e-9);
    assert!((to(-10_000., 10_000.) + std::f64::consts::FRAC_PI_4).abs() < 1e-9);
    assert!((to(0., -10_000.).abs() - std::f64::consts::PI).abs() < 1e-9);
}

/// One guiding command missile flown as live flight flies it, with the
/// terrain-aware time to impact worked out every tick (`every`) or as live
/// flight now does, every `SECONDS_REFRESH_TICKS` with a count-down between
/// (review H1). Returns the time spent on those times, the ticks flown and
/// every time.
fn guiding_cost(
    ground: &dyn Fn(f64, f64) -> f64,
    every: bool,
) -> (std::time::Duration, u64, Vec<Option<f64>>) {
    use crate::combat::live::SECONDS_REFRESH_TICKS;
    let mut w = at2();
    w.movement.remove_t = 240;
    let m = w.movement;
    let mut motion = Motion::launch(&w, [0., 0., 1000.], 1500.);
    let mut position = [0., 1500., 0.];
    let mut direction = [0., 0., 1.];
    let point = [0., ground(0., 40_000.), 40_000.];
    let mut spent = std::time::Duration::ZERO;
    let mut seconds: Vec<Option<f64>> = Vec::new();
    for age in 0..u64::from(m.remove_t) * 30 {
        let aim = motion.aim_over_terrain(position, point, ground);
        let desired = missiles::commanded_heading(
            direction,
            motion.path(),
            unit(missiles::sub(aim, position)),
        );
        let next = missiles::steer(&m, age, direction, desired);
        motion.turn(direction, next);
        direction = next;
        let start = std::time::Instant::now();
        let refresh = every || age == 0 || age.is_multiple_of(SECONDS_REFRESH_TICKS);
        let time = if refresh {
            time_to_with_terrain(&w, motion, position, direction, point, age, Some(ground))
        } else {
            seconds.last().copied().flatten().map(|s| (s - DT).max(0.))
        };
        spent += start.elapsed();
        seconds.push(time);
        let delta = motion.step(&m, age, direction);
        for (p, d) in position.iter_mut().zip(delta) {
            *p += d;
        }
        if missiles::length(missiles::sub(point, position)) <= 25.
            || position[1] <= ground(position[0], position[2])
        {
            return (spent, age + 1, seconds);
        }
    }
    (spent, u64::from(m.remove_t) * 30, seconds)
}

/// Review H1 timing probe: `cargo test --release -p tore-sim
/// guiding_time_to_impact_cost -- --ignored --nocapture`.
#[test]
#[ignore = "timing probe"]
fn guiding_time_to_impact_cost() {
    let hills = |x: f64, z: f64| 600. + 500. * (x / 2500.).sin() * (z / 3100.).sin();
    let ridge = |_: f64, z: f64| (1. - ((z - 20_000.) / 3000.).abs()).max(0.) * 2200.;
    type Ground<'a> = &'a dyn Fn(f64, f64) -> f64;
    let cases: [(&str, Ground); 3] = [("flat", &|_, _| 0.), ("ridge", &ridge), ("hills", &hills)];
    for (name, ground) in cases {
        for every in [true, false] {
            let (spent, ticks, seconds) = guiding_cost(ground, every);
            let flown = ticks as f64 * DT;
            let worst = seconds
                .iter()
                .enumerate()
                .filter_map(|(i, s)| s.map(|s| (s - (flown - i as f64 * DT)).abs()))
                .fold(0., f64::max);
            println!(
                "h1 {name} {}: {ticks} ticks, {:.1} us per tick, worst error {worst:.4} s",
                if every { "every tick" } else { "refreshed" },
                spent.as_secs_f64() * 1e6 / ticks as f64,
            );
        }
    }
}
