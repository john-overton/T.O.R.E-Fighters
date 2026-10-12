//! The bomb equation on its own (docs/formats/weapons.md, "Bomb motion").
use super::*;

#[test]
fn the_pack_stagger_steps_out_two_feet_a_bomb() {
    let offsets: Vec<[f64; 2]> = (0..7).map(pack_offset).collect();
    assert_eq!(
        offsets,
        [
            [0., 0.],
            [-2., -2.],
            [2., -6.],
            [-4., -12.],
            [4., -20.],
            [-6., -30.],
            [6., -42.],
        ]
    );
}

#[test]
fn the_scatter_is_273_units_either_way_from_the_sim_stream() {
    // 1.5 degrees of 65,536 units a turn (retail's 3 degrees was 546).
    assert_eq!(scatter_units(), 273);
    let unit_angle = std::f64::consts::TAU / 65_536.;
    let mut asked = Vec::new();
    let (heading, pitch) = scatter(|bound| {
        asked.push(bound);
        if asked.len() == 1 { 0 } else { bound - 1 }
    });
    assert_eq!(asked, [547, 547]);
    assert!((heading + 273. * unit_angle).abs() < 1e-12);
    assert!((pitch - 273. * unit_angle).abs() < 1e-12);
    assert!(heading.abs() <= BOMB_SCATTER_DEGREES.to_radians());
}

#[test]
fn a_level_release_falls_sixteen_s_squared() {
    let motion = BombMotion::new(ReleaseState::new([10., 5_000., -20.], [0., 0., 1.], 759.));
    for s in [0., 1., 2.5, 10.] {
        let p = motion.position(s);
        assert!((p[1] - (5_000. - 16. * s * s)).abs() < 1e-9);
        assert!((p[2] - (-20. + 759. * s)).abs() < 1e-9);
        assert_eq!(p[0], 10.);
    }
    let t = motion.fall_time(0.).unwrap();
    assert!((t - (5_000_f64 / 16.).sqrt()).abs() < 1e-12);
    // Past the predicted ground it keeps going at the same rate.
    assert!((motion.position(t + 1.)[2] - motion.position(t)[2] - 759.).abs() < 1e-9);
}

#[test]
fn a_dive_release_uses_retails_fall_time() {
    let pitch = -30_f64.to_radians();
    let release = ReleaseState::new([0., 4_000., 0.], [0., pitch.sin(), pitch.cos()], 760.);
    let motion = BombMotion::new(release);
    let vz = 760. * pitch.sin();
    let expected = (vz + (vz * vz + 64. * 4_000.).sqrt()) / 32.;
    let t = motion.fall_time(0.).unwrap();
    assert!((t - expected).abs() < 1e-12);
    assert!(motion.position(t)[1].abs() < 1e-9);
    // Its pitch is its velocity's, steepening as it falls.
    let d0 = motion.direction(0.);
    let d1 = motion.direction(t);
    assert!((d0[1] - pitch.sin()).abs() < 1e-12);
    assert!(d1[1] < d0[1]);
    assert_eq!(d1[0], 0.);
}

#[test]
fn retails_predictor_reads_the_ground_under_the_release_point() {
    let release = ReleaseState::new([0., 3_000., 0.], [0., 0., 1.], 500.);
    // Ground 1,000 ft high under the release, 0 everywhere else.
    let ground = |x: f64, z: f64| if x.hypot(z) < 1. { 1_000. } else { 0. };
    let impact = predict_impact_flat(release, ground).unwrap();
    let t = (2_000_f64 / 16.).sqrt();
    assert!((impact.seconds - t).abs() < 1e-12);
    assert_eq!(impact.position[1], 1_000.);
    assert!((impact.position[2] - 500. * t).abs() < 1e-9);
    // Under the ground with nowhere to fall to: no impact.
    let low = ReleaseState::new([0., 10., 0.], [0., 0., 1.], 500.);
    assert!(predict_impact_flat(low, |_, _| 20.).is_none());
    assert!(predict_impact(low, |_, _| 20.).is_none());
}

/// A level run at 5,000 ft over a plateau: flat ground (0 ft) under the
/// release, then ground at `height` from z = 2,000 ft on.
fn plateau(height: f64) -> impl Fn(f64, f64) -> f64 {
    move |_, z| if z >= 2_000. { height } else { 0. }
}

#[test]
fn the_pipper_finds_where_the_path_meets_the_terrain_not_the_ground_under_the_aircraft() {
    // John, 2026-10-11: realistic CCIP. Level at 5,000 ft and 600 ft/s, the
    // fall to sea level is 17.7 s and 10,600 ft downrange.
    let release = ReleaseState::new([0., 5_000., 0.], [0., 0., 1.], 600.);
    let flat = predict_impact_flat(release, |_, _| 0.).unwrap();
    // Over flat ground nothing changes, to the bit.
    assert_eq!(predict_impact(release, |_, _| 0.), Some(flat));
    // Target ground 400 ft high: the path reaches it sooner and shorter
    // than retail's flat fall to 0 ft says.
    let ground = plateau(400.);
    let retail = predict_impact_flat(release, &ground).unwrap();
    assert_eq!(retail, flat, "retail reads 0 ft under the release");
    let real = predict_impact(release, ground).unwrap();
    assert_eq!(real.position[1], 400.);
    assert!(real.seconds < flat.seconds);
    assert!(
        real.position[2] < flat.position[2] - 400.,
        "{} vs {}",
        real.position[2],
        flat.position[2]
    );
    // It is the very point a real bomb reaches: the parabola at that time.
    let motion = BombMotion::new(release);
    let at = motion.position(real.seconds);
    assert!((at[2] - real.position[2]).abs() < 1e-6);
    assert!((at[1] - 400.).abs() < 1e-6, "{at:?}");
}

#[test]
fn the_pipper_falls_on_into_a_hollow_beyond_the_release_ground() {
    // Ground 600 ft under the sea-level plain beyond z = 2,000 ft...
    let release = ReleaseState::new([0., 5_000., 0.], [0., 0., 1.], 600.);
    let hollow = |_: f64, z: f64| if z >= 2_000. { -600. } else { 0. };
    let flat = predict_impact_flat(release, hollow).unwrap();
    let real = predict_impact(release, hollow).unwrap();
    assert_eq!(real.position[1], -600.);
    assert!(real.seconds > flat.seconds);
    assert!(real.position[2] > flat.position[2] + 400.);
    let at = BombMotion::new(release).position(real.seconds);
    assert!((at[1] + 600.).abs() < 1e-6);
}

#[test]
fn a_ridge_between_the_aircraft_and_the_target_stops_the_bomb_on_its_face() {
    // A 1,500 ft ridge at z = 6,000 to 7,000 ft, sea level elsewhere. A
    // bomb released level at 2,000 ft and 700 ft/s is 825 ft up when it
    // reaches the ridge, so it meets the face.
    let release = ReleaseState::new([0., 2_000., 0.], [0., 0., 1.], 700.);
    let ridge = |_: f64, z: f64| {
        if (6_000. ..7_000.).contains(&z) {
            1_500.
        } else {
            0.
        }
    };
    let flat = predict_impact_flat(release, ridge).unwrap();
    let real = predict_impact(release, ridge).unwrap();
    // Retail's fall carries it past the ridge to the plain; the bomb in fact
    // reaches the ridge first.
    assert!(flat.position[2] > 7_000.);
    assert_eq!(real.position[1], 1_500.);
    assert!((6_000. ..7_000.).contains(&real.position[2]), "{real:?}");
    assert!(real.seconds < flat.seconds);
}

#[test]
fn turning_a_release_keeps_its_speed_and_clamps_the_pitch() {
    let release = ReleaseState::new([0.; 3], [1., 0., 1.], 600.);
    let turned = release.turned(0.1, -0.05);
    assert!((turned.speed() - 600.).abs() < 1e-9);
    assert!((turned.heading() - release.heading() - 0.1).abs() < 1e-12);
    assert!((turned.pitch() + 0.05).abs() < 1e-12);
    let down = ReleaseState::new([0.; 3], [0., -1., 0.], 600.).turned(0., -0.1);
    assert!(down.pitch() >= -PITCH_LIMIT - 1e-12);
}

#[test]
fn onto_puts_the_mean_path_on_the_target() {
    let target = [1_000., 50., -300.];
    let release = ReleaseState::onto(target, [1., -1., 0.], 700., 1_500.);
    assert_eq!(release.position[1], 1_550.);
    let impact = predict_impact(release, |_, _| 50.).unwrap();
    assert!((impact.position[0] - target[0]).abs() < 1e-6);
    assert!((impact.position[2] - target[2]).abs() < 1e-6);
}

#[test]
fn packs_release_their_burst_and_pods_one_round() {
    let w = |flags: u32, pod: i16, burst: u8| {
        let mut w = crate::combat::gunsight::tests::weapon();
        w.flags = flags;
        w.burst.projectiles_in_pod = pod;
        w.burst.game_rounds_in_burst = burst;
        w
    };
    assert_eq!(release_count(&w(0x22012, 1, 7)), 7);
    assert_eq!(release_count(&w(0x22012, 1, 1)), 1);
    assert_eq!(release_count(&w(0x20846, 19, 2)), 1);
    assert!(is_pod(&w(0x20046, 4, 1)));
    assert!(!is_pod(&w(0x80, 1200, 2)));
    assert!(is_bomb(&w(0x82a052, 1, 1)));
    assert!(!is_bomb(&w(0x2a543, 1, 1)));
}
