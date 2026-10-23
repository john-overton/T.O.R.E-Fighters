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
fn drag_delays_the_fall_and_reduces_horizontal_speed() {
    let release = ReleaseState::new([10., 5_000., -20.], [0., 0., 1.], 759.);
    let motion = BombMotion::new(release);
    let t = motion.fall_time(0.).unwrap();
    assert!(t > (5_000_f64 / 16.).sqrt());
    assert!(t < 20.);
    assert!(motion.velocity(t)[2] < 759.);
    assert!(motion.position(t)[1].abs() < 1e-6);
}

#[test]
fn prediction_matches_tick_integration_across_release_conditions() {
    for height in [500., 5_000., 20_000.] {
        for knots in [250., 450., 600.] {
            for dive in [0_f64, 30.] {
                for retarder in [0., 2.4, 5.5] {
                    let pitch = -dive.to_radians();
                    let mut release = ReleaseState::new(
                        [10., height, -20.],
                        [0., pitch.sin(), pitch.cos()],
                        knots * 1.6878,
                    );
                    release.body.retarder = retarder;
                    let impact = predict_impact(release, |_, _| 0.).unwrap();
                    let mut flight = Flight {
                        position: release.position,
                        velocity: release.velocity,
                    };
                    let mut tick = 0;
                    while flight.position[1] > 0. {
                        flight.step(release.body, tick, None);
                        tick += 1;
                    }
                    assert!((tick as f64 * DT - impact.seconds).abs() <= DT);
                    let miss = (flight.position[0] - impact.position[0])
                        .hypot(flight.position[2] - impact.position[2]);
                    assert!(
                        miss <= release.speed() * DT + 0.01,
                        "{height} {knots} {dive} {retarder}: {miss}"
                    );
                    println!(
                        "bomb-flight height={height} kt={knots} dive={dive} retard={retarder} time={:.5} tick-miss={miss:.4} arrival-kt={:.2}",
                        impact.seconds,
                        flight.velocity.iter().map(|v| v * v).sum::<f64>().sqrt() / 1.6878
                    );
                }
            }
        }
    }
}

#[test]
fn retails_predictor_reads_the_ground_under_the_release_point() {
    let release = ReleaseState::new([0., 3_000., 0.], [0., 0., 1.], 500.);
    // Ground 1,000 ft high under the release, 0 everywhere else.
    let ground = |x: f64, z: f64| if x.hypot(z) < 1. { 1_000. } else { 0. };
    let impact = predict_impact_flat(release, ground).unwrap();
    let t = BombMotion::new(release).fall_time(1_000.).unwrap();
    assert!((impact.seconds - t).abs() < 1e-12);
    assert_eq!(impact.position[1], 1_000.);
    assert!((impact.position[2] - BombMotion::new(release).position(t)[2]).abs() < 1e-9);
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

/// A fall's cursor (optimization slice S1) answers bit for bit what the stored
/// motion answers flying again from its release: over random releases
/// (heights, speeds, climbs and dives, bodies and drag), random ages walked
/// forward with random strides, some steps back, part ticks, stored motions
/// already advanced, and fall times to random ground heights, including
/// heights the path already went below and heights above the release.
#[test]
fn a_fall_cursor_matches_the_stored_motion_bit_for_bit() {
    let mut seed = 0x51_7cc1_b727_220a_u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let bits = |f: Flight| {
        [f.position, f.velocity]
            .concat()
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>()
    };
    let mut checks = 0;
    let mut fallbacks = 0;
    let mut impacts = 0;
    let mut searched_on = 0;
    for case in 0..64 {
        let height = 100. + next() * 25_000.;
        let pitch = (next() - 0.6) * 1.4;
        let heading = next() * std::f64::consts::TAU;
        let mut release = ReleaseState::new(
            [next() * 1e5 - 5e4, height, next() * 1e5 - 5e4],
            [
                pitch.cos() * heading.sin(),
                pitch.sin(),
                pitch.cos() * heading.cos(),
            ],
            50. + next() * 1_500.,
        );
        release.body = Body {
            mass: (50. + next() * 2_500.) / 32.,
            area: 0.05 + next() * 0.4,
            retarder: [0., 0., 2.4, 5.5][case % 4],
            steering: 0,
            guidance_seconds: 0.,
        };
        let mut stored = BombMotion::new(release);
        // A stored motion that was itself advanced starts its paths there.
        if case % 8 == 7 {
            stored.advance((next() * 300.) as u64);
        }
        let mut cursor = FallCursor::new(stored);
        let start = if case % 3 == 0 {
            (next() * 50.) as u64
        } else {
            0
        };
        let end = 200 + (next() * 1_300.) as u64;
        let mut age = start;
        while age < end {
            // Mostly forward, now and then back (a restore, a replay seek).
            if next() < 0.03 {
                age = age.saturating_sub((next() * 200.) as u64).max(start);
            } else {
                age += 1 + (next() * 16.) as u64;
            }
            let seconds =
                (age - start) as f64 / 120. + if case % 5 == 4 { next() / 120. } else { 0. };
            let got = cursor.at(&stored, seconds);
            assert_eq!(
                bits(got),
                bits(stored.at(seconds)),
                "case {case} age {age} seconds {seconds}"
            );
            assert_eq!(
                got.position.map(f64::to_bits),
                stored.position(seconds).map(f64::to_bits)
            );
            assert_eq!(
                unit(got.velocity).map(f64::to_bits),
                stored.direction(seconds).map(f64::to_bits)
            );
            let lowest = cursor.lowest;
            let ground = match (next() * 4.) as u32 {
                0 => height + 1. + next() * 500.,
                1 if lowest.is_finite() => lowest + (next() - 0.5) * 40.,
                _ => got.position[1] - next() * 4_000.,
            };
            if lowest - ground <= 0. || lowest.is_nan() {
                fallbacks += 1;
            }
            let expected = stored.fall_time(ground);
            impacts += usize::from(expected.is_some());
            // Found by searching on from the cursor, not from the release.
            searched_on += usize::from(
                expected.is_some()
                    && stored.tick == 0
                    && lowest - ground > 0.
                    && lowest.is_finite(),
            );
            assert_eq!(
                cursor.fall_time(&stored, ground).map(f64::to_bits),
                expected.map(f64::to_bits),
                "case {case} age {age} ground {ground}"
            );
            checks += 1;
        }
    }
    assert!(checks > 3_000, "{checks}");
    assert!(fallbacks > 50, "{fallbacks}");
    assert!(impacts > 1_000, "{impacts}");
    assert!(searched_on > 500, "{searched_on}");
}

/// The cursors drop out of `Debug`, so they never tell two states apart.
#[test]
fn fall_cursors_are_transient() {
    let release = ReleaseState::new([0., 5_000., 0.], [0., 0., 1.], 700.);
    let stored = BombMotion::new(release);
    let mut cursors = FallCursors::default();
    cursors.follow(3, &stored).at(&stored, 2.);
    assert_eq!(format!("{cursors:?}"), "FallCursors");
    // Another fall under the same number starts its cursor again.
    let other = BombMotion::new(ReleaseState::new([0., 5_000., 0.], [0., 0., 1.], 701.));
    assert!(!cursors.follow(3, &other).follows(&stored));
    assert_eq!(cursors.follow(3, &other).motion.tick, 0);
    cursors.retain(|id| id != 3);
    assert!(cursors.is_empty());
}
