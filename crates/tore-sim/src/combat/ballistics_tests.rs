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
fn the_scatter_is_546_units_either_way_from_the_sim_stream() {
    // 3 degrees of 65,536 units a turn.
    assert_eq!(scatter_units(), 546);
    let unit_angle = std::f64::consts::TAU / 65_536.;
    let mut asked = Vec::new();
    let (heading, pitch) = scatter(|bound| {
        asked.push(bound);
        if asked.len() == 1 { 0 } else { bound - 1 }
    });
    assert_eq!(asked, [1093, 1093]);
    assert!((heading + 546. * unit_angle).abs() < 1e-12);
    assert!((pitch - 546. * unit_angle).abs() < 1e-12);
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
fn the_predictor_reads_the_ground_under_the_release_point() {
    let release = ReleaseState::new([0., 3_000., 0.], [0., 0., 1.], 500.);
    // Ground 1,000 ft high under the release, 0 everywhere else.
    let ground = |x: f64, z: f64| if x.hypot(z) < 1. { 1_000. } else { 0. };
    let impact = predict_impact(release, ground).unwrap();
    let t = (2_000_f64 / 16.).sqrt();
    assert!((impact.seconds - t).abs() < 1e-12);
    assert_eq!(impact.position[1], 1_000.);
    assert!((impact.position[2] - 500. * t).abs() < 1e-9);
    // Under the ground with nowhere to fall to: no impact.
    let low = ReleaseState::new([0., 10., 0.], [0., 0., 1.], 500.);
    assert!(predict_impact(low, |_, _| 20.).is_none());
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
