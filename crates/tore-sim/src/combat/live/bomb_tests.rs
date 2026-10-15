//! Bombs and rocket pods in the live adapter (plan section 7.1, slice B1):
//! the bomb equation from a real release, the predictor, the scatter, packs
//! and pods. Numbers from docs/formats/weapons.md, "Bomb motion".
use super::tests::fixture;
use super::*;
use crate::combat::ballistics::{
    self, BOMB_SCATTER_DEGREES, BombMotion, ReleaseState, pack_offset, predict_impact,
};

const KNOT: f64 = 1.687_8;

/// A Mk 82 as the record has it: bomb flag, no gravity flag, 0 to 1,466 ft/s,
/// full launch retard, one round a call.
pub(super) fn mk82() -> Weapon {
    let mut w = fixture(false).own().configuration().stations[0]
        .weapon
        .clone();
    w.source = "MK82.JT".into();
    w.weight = 500;
    w.flags = 0x22012;
    w.movement.minimum_speed = 0;
    w.movement.maximum_speed = 1466;
    w.movement.initial_speed = 0;
    w.movement.final_speed = 0;
    w.movement.launch_retard = 100;
    w.movement.remove_t = 240;
    w.burst.actual_rounds_per_game = 1;
    w.burst.game_rounds_in_burst = 1;
    w.burst.game_burst_t = 0;
    w.damage.fuze_arm_t = 4;
    w.effects.crater_size = 3;
    w.seeker.signature = 0;
    w
}

/// A LAU-61 as the record has it: 19 rockets a pod, a burst of 2, repeat
/// flag 0x800 with a quarter-second burst time, gravity flag 0x4.
pub(super) fn lau61() -> Weapon {
    let mut w = fixture(false).own().configuration().stations[0]
        .weapon
        .clone();
    w.source = "LAU61.JT".into();
    w.flags = 0x20846;
    w.weight = 200;
    w.burst.projectiles_in_pod = 19;
    w.burst.actual_rounds_per_game = 1;
    w.burst.game_rounds_in_burst = 2;
    w.burst.game_burst_t = 1;
    w.movement.remove_t = 80;
    w.seeker.signature = 0;
    w
}

/// One external station carrying `count` of `weapon` at `mount`.
fn carrying(weapon: Weapon, count: u16, mount: Vector) -> State {
    let mut config = fixture(false).own().configuration().clone();
    config.stations = vec![Station {
        weapon,
        mount,
        count,
        internal: false,
        rack: crate::combat::hardpoints::Rack {
            // Released from the record's own point (a bay): these paths
            // start at the mount, whatever a pylon's rack would add.
            location: 3,
            ..Default::default()
        },
    }];
    State::new(config, true).unwrap()
}

/// An aircraft at `altitude` ft, heading north at `knots`, pitched `dive`
/// degrees (negative down), its velocity along its nose.
fn pass(altitude: f64, knots: f64, dive: f64) -> Launcher {
    let basis = Basis::new(0., dive.to_radians(), 0.);
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
        controls: sensors::Controls::default(),
    }
}

/// Far above and away, so nothing the aircraft drops meets it again.
fn away() -> Launcher {
    Launcher {
        position: [200_000., 40_000., -200_000.],
        ..pass(40_000., 450., 0.)
    }
}

fn step(s: &mut State, held: bool, launcher: Launcher) -> Vec<Event> {
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held,
            launcher,
        }],
        |_, _| 0.,
    )
}

/// Where every bomb burst on the ground, in order: its crater, which stays
/// for the mission (a burst's flash is gone in a few seconds).
fn ground_bursts(s: &State) -> Vec<Vector> {
    s.marks
        .iter()
        .filter(|m| matches!(m.kind, crate::combat::blast::MarkKind::Crater(_)))
        .map(|m| m.position)
        .collect()
}

/// Steps clear of the aircraft until no round is left, at most `seconds`.
fn fall(s: &mut State, seconds: f64) {
    for _ in 0..(seconds * 120.) as usize {
        if s.projectiles.is_empty() {
            return;
        }
        step(s, false, away());
    }
    panic!("rounds still flying after {seconds} s");
}

fn horizontal(a: Vector, b: Vector) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

#[test]
fn a_released_bomb_follows_the_shared_flight_model() {
    let mut s = carrying(mk82(), 4, [0.; 3]);
    let launcher = pass(5_000., 450., 0.);
    step(&mut s, true, launcher);
    assert_eq!(s.projectiles.len(), 1);
    let id = s.projectiles[0].id;
    let motion = s.bomb_motion(id).unwrap();
    // Retail release: 450 kt is 759 ft/s whole, at full retard, no push.
    let speed = motion.horizontal[0].hypot(motion.horizontal[1]);
    assert!(
        (motion.horizontal[0]
            .hypot(motion.horizontal[1])
            .hypot(motion.vertical)
            - 450. * KNOT)
            .abs()
            < 1e-9
    );
    assert!(speed > 759. * 3_f64.to_radians().cos() - 1e-9);
    assert_eq!(motion.origin, launcher.position);
    let heading = motion.horizontal[0].atan2(motion.horizontal[1]);
    for tick in 1..=600u64 {
        let p = &s.projectiles[0];
        let seconds = tick as f64 / 120.;
        let expected = motion.position(seconds);
        for (axis, want) in p.position.iter().zip(expected) {
            assert!((axis - want).abs() < 1e-6, "tick {tick}");
        }
        // y0 + vz s - 16 s².
        let height = motion.position(seconds)[1];
        assert!((p.position[1] - height).abs() < 1e-6);
        // Pitch follows the velocity; heading stays the release heading.
        let pitch = p.direction[1].atan2(p.direction[0].hypot(p.direction[2]));
        let velocity = motion.velocity(seconds);
        let falling = velocity[1].atan2(velocity[0].hypot(velocity[2]));
        assert!((pitch - falling).abs() < 1e-9);
        assert!((p.direction[0].atan2(p.direction[2]) - heading).abs() < 1e-9);
        step(&mut s, false, away());
    }
}

#[test]
fn mk82_from_5000_ft_level_at_450_kt_lands_where_the_numbers_say() {
    let w = mk82();
    let station = Station {
        weapon: w,
        mount: [0.; 3],
        count: 1,
        internal: false,
        rack: crate::combat::hardpoints::Rack {
            // Released from the record's own point (a bay): these paths
            // start at the mount, whatever a pylon's rack would add.
            location: 3,
            ..Default::default()
        },
    };
    let release = ReleaseState::from_station(&pass(5_000., 450., 0.), &station, 0);
    assert_eq!(release.speed(), 450. * KNOT);
    let impact = predict_impact(release, |_, _| 0.).unwrap();
    assert!(impact.seconds > (5_000_f64 / 16.).sqrt());
    assert!(impact.seconds < 20.);
    assert!(impact.position[2] > 12_000. && impact.position[2] < 13_417.);
    assert_eq!(impact.position[1], 0.);
}

#[test]
fn retard_deploys_after_release_and_all_bombs_inherit_aircraft_velocity() {
    let mut snakeye = mk82();
    snakeye.movement.launch_retard = 66;
    let mut cluster = mk82();
    cluster.flags = 0x82a052;
    cluster.movement.launch_retard = 90;
    cluster.movement.maximum_speed = 2933;
    let at = |w: &Weapon, knots: f64| {
        let station = Station {
            weapon: w.clone(),
            mount: [0.; 3],
            count: 1,
            internal: false,
            rack: crate::combat::hardpoints::Rack {
                // Released from the record's own point (a bay): these paths
                // start at the mount, whatever a pylon's rack would add.
                location: 3,
                ..Default::default()
            },
        };
        ReleaseState::from_station(&pass(5_000., knots, 0.), &station, 0)
    };
    // 759 x 66 / 100, whole feet a second.
    assert_eq!(at(&snakeye, 450.).speed(), 450. * KNOT);
    let slick = predict_impact(at(&mk82(), 450.), |_, _| 0.).unwrap();
    let retarded = predict_impact(at(&snakeye, 450.), |_, _| 0.).unwrap();
    // Same fall, a shorter throw: no drag after release.
    assert!(retarded.seconds > slick.seconds);
    assert!(retarded.position[2] < slick.position[2] * 0.75);
    // 2,000 kt: slick bombs stop at 1,466 ft/s, cluster bombs at 2,933.
    assert_eq!(at(&mk82(), 2_000.).speed(), 2_000. * KNOT);
    assert_eq!(at(&cluster, 2_000.).speed(), 2_000. * KNOT);
    assert_eq!(at(&cluster, 450.).speed(), 450. * KNOT);
    // The cluster records' 0x40 speed command does not touch the path.
    let mut s = carrying(cluster, 1, [0.; 3]);
    step(&mut s, true, pass(5_000., 450., 0.));
    let motion = s.bomb_motion(s.projectiles[0].id).unwrap();
    for _ in 0..240 {
        step(&mut s, false, away());
    }
    let expected = motion.position(241. / 120.);
    assert!((0..3).all(|i| (s.projectiles[0].position[i] - expected[i]).abs() < 1e-6));
}

#[test]
fn the_mean_path_lands_on_the_predicted_point_within_a_foot() {
    // Scatter off and flat ground: the real bomb, flown tick by tick through
    // combat to its ground burst, against the predictor.
    for knots in [250., 450., 650.] {
        for altitude in [800., 5_000., 15_000.] {
            for dive in [0., -10., -30., -45., 10.] {
                let mut s = carrying(mk82(), 1, [3., -2., 1.]);
                let launcher = pass(altitude, knots, dive);
                step(&mut s, true, launcher);
                let station = s.own().configuration().stations[0].clone();
                let release = ReleaseState::from_station(&launcher, &station, 0);
                let id = s.projectiles[0].id;
                // Undo the scatter: the bomb flies its mean path.
                s.bombs.insert(id, BombMotion::new(release));
                fall(&mut s, 60.);
                let predicted = predict_impact(release, |_, _| 0.).unwrap();
                let burst = *ground_bursts(&s).last().expect("a ground burst");
                let miss = horizontal(burst, predicted.position);
                assert!(
                    miss < 1.,
                    "{knots} kt, {altitude} ft, {dive} deg: missed the prediction by {miss} ft"
                );
            }
        }
    }
}

#[test]
fn every_bomb_scatters_within_one_and_a_half_degrees_and_lands_inside_the_predicted_spread() {
    let mut s = carrying(mk82(), 200, [0.; 3]);
    let launcher = pass(5_000., 450., -20.);
    let station = s.own().configuration().stations[0].clone();
    let mean = ReleaseState::from_station(&launcher, &station, 0);
    let limit = BOMB_SCATTER_DEGREES.to_radians() + 1e-9;
    // The farthest the predictor puts a bomb from the mean point with the
    // heading and pitch errors at their limits.
    let centre = predict_impact(mean, |_, _| 0.).unwrap().position;
    let mut spread: f64 = 0.;
    for h in [-1., -0.5, 0., 0.5, 1.] {
        for p in [-1., -0.5, 0., 0.5, 1.] {
            let corner = mean.turned(h * limit, p * limit);
            let at = predict_impact(corner, |_, _| 0.).unwrap().position;
            spread = spread.max(horizontal(at, centre));
        }
    }
    let mut errors = Vec::new();
    for _ in 0..40 {
        step(&mut s, true, launcher);
        let id = s.projectiles.last().unwrap().id;
        let motion = s.bomb_motion(id).unwrap();
        let release = ReleaseState {
            body: motion.body,
            position: motion.origin,
            velocity: motion.velocity(0.),
        };
        let heading = release.heading() - mean.heading();
        let pitch = release.pitch() - mean.pitch();
        assert!(
            heading.abs() <= limit && pitch.abs() <= limit,
            "{heading} {pitch}"
        );
        assert!((release.speed() - mean.speed()).abs() < 1e-9);
        errors.push((heading, pitch));
        step(&mut s, false, launcher);
    }
    // They differ bomb to bomb and reach toward both limits.
    let largest = errors
        .iter()
        .map(|e| e.0.abs().max(e.1.abs()))
        .fold(0., f64::max);
    assert!(largest > limit * 0.8, "{largest}");
    assert!(errors.windows(2).all(|pair| pair[0] != pair[1]));
    fall(&mut s, 60.);
    let bursts = ground_bursts(&s);
    assert_eq!(bursts.len(), 40);
    for burst in bursts {
        assert!(
            horizontal(burst, centre) <= spread + 1.,
            "{burst:?} {centre:?} {spread}"
        );
    }
}

#[test]
fn a_missions_first_bomb_takes_its_own_scatter_and_the_mean_has_no_bias() {
    // Slice ST-LV. Nothing rolls combat's random stream before a quiet
    // mission's first bomb, so every first bomb used to take the same error
    // (1.25 degrees left, 1.31 low: some 330 ft short from a 5,000 ft level
    // pass, at every skill and on every target). Released on different ticks,
    // first bombs now scatter differently, both ways, about the mean.
    let launcher = pass(5_000., 450., 0.);
    let limit = BOMB_SCATTER_DEGREES.to_radians() + 1e-9;
    let mut errors = Vec::new();
    for wait in 0..64 {
        let mut s = carrying(mk82(), 1, [0.; 3]);
        for _ in 0..wait {
            step(&mut s, false, launcher);
        }
        step(&mut s, true, launcher);
        let station = s.own().configuration().stations[0].clone();
        let mean = ReleaseState::from_station(&launcher, &station, 0);
        let motion = s.bomb_motion(s.projectiles[0].id).unwrap();
        let release = ReleaseState {
            body: motion.body,
            position: motion.origin,
            velocity: motion.velocity(0.),
        };
        let error = (
            release.heading() - mean.heading(),
            release.pitch() - mean.pitch(),
        );
        assert!(
            error.0.abs() <= limit && error.1.abs() <= limit,
            "{error:?}"
        );
        errors.push(error);
    }
    let mut distinct = errors.clone();
    distinct.sort_by(|a, b| a.partial_cmp(b).unwrap());
    distinct.dedup();
    assert!(distinct.len() >= 60, "{} distinct of 64", distinct.len());
    let n = errors.len() as f64;
    let mean_heading = errors.iter().map(|e| e.0).sum::<f64>() / n;
    let mean_pitch = errors.iter().map(|e| e.1).sum::<f64>() / n;
    // A uniform error of 1.5 degrees has a mean of 64 within about 0.11
    // degrees of zero (one sigma); the old fixed draw sat at -1.25 and -1.31.
    assert!(
        mean_heading.to_degrees().abs() < 0.35,
        "{}",
        mean_heading.to_degrees()
    );
    assert!(
        mean_pitch.to_degrees().abs() < 0.35,
        "{}",
        mean_pitch.to_degrees()
    );
    for pick in [|e: &(f64, f64)| e.0, |e: &(f64, f64)| e.1] {
        assert!(errors.iter().any(|e| pick(e) > limit * 0.5));
        assert!(errors.iter().any(|e| pick(e) < -limit * 0.5));
    }
}

#[test]
fn the_same_release_scatters_the_same_way_every_run() {
    let run = || {
        let mut s = carrying(mk82(), 20, [0.; 3]);
        for _ in 0..5 {
            step(&mut s, true, pass(3_000., 400., -10.));
            step(&mut s, false, pass(3_000., 400., -10.));
        }
        s.bombs.clone()
    };
    assert_eq!(run(), run());
}

#[test]
fn a_pack_releases_all_its_bombs_in_one_tick_for_one_round() {
    let mut w = mk82();
    w.source = "MK82AIR.JT".into();
    w.burst.game_rounds_in_burst = 7;
    w.burst.game_burst_t = 4;
    w.movement.launch_retard = 66;
    let mount = [-6., -3., 2.];
    let mut s = carrying(w, 2, mount);
    let launcher = pass(2_000., 450., 0.);
    let events = step(&mut s, true, launcher);
    assert_eq!(s.projectiles.len(), 7);
    assert_eq!(s.own().rounds(0), 1, "one round for the whole pack");
    let fired = events
        .iter()
        .filter(|e| matches!(e, Event::Fired { .. }))
        .count();
    assert_eq!(fired, 7);
    let expected_x = [0., -2., 2., -4., 4., -6., 6.];
    let expected_y = [0., -2., -6., -12., -20., -30., -42.];
    let mut velocities = Vec::new();
    for (n, p) in s.projectiles.iter().enumerate() {
        assert_eq!(pack_offset(n), [expected_x[n], expected_y[n]]);
        let motion = s.bomb_motion(p.id).unwrap();
        let station: Vector = std::array::from_fn(|i| {
            launcher.position[i]
                + launcher.basis.right[i] * mount[0]
                + launcher.basis.up[i] * mount[1]
                + launcher.basis.forward[i] * mount[2]
        });
        let offset: Vector = std::array::from_fn(|i| motion.origin[i] - station[i]);
        let side = dot(offset, launcher.basis.right);
        assert!((side - expected_x[n]).abs() < 1e-9, "bomb {n}: {offset:?}");
        assert!(
            (offset[1] - expected_y[n]).abs() < 1e-9,
            "bomb {n}: {offset:?}"
        );
        // 66 percent of 759 ft/s, each with its own scatter.
        assert!(
            (motion
                .velocity(0.)
                .iter()
                .map(|v| v * v)
                .sum::<f64>()
                .sqrt()
                - launcher.speed_fps)
                .abs()
                < 1e-9
        );
        velocities.push(motion.velocity(0.));
    }
    for (a, b) in velocities.iter().zip(velocities.iter().skip(1)) {
        assert_ne!(a, b);
    }
    // Holding the trigger does not release the second pack: no repeat flag.
    for _ in 0..240 {
        step(&mut s, true, away());
    }
    assert_eq!(s.own().rounds(0), 1);
    // A new press does.
    step(&mut s, false, away());
    step(&mut s, true, away());
    assert_eq!(s.own().rounds(0), 0);
}

#[test]
fn a_bomb_pack_survives_a_checkpoint_mid_fall_and_lands_identically() {
    use crate::checkpoint::{Models, from_bytes, to_bytes};
    let mut w = mk82();
    w.burst.game_rounds_in_burst = 6;
    let mut s = carrying(w, 3, [0.; 3]);
    step(&mut s, true, pass(4_000., 420., -15.));
    for _ in 0..90 {
        step(&mut s, false, away());
    }
    assert_eq!(s.bombs.len(), 6);
    let models = Models::default();
    let mut copy: State = from_bytes(&to_bytes(&s, &models).unwrap(), &models).unwrap();
    assert_eq!(copy.bombs, s.bombs);
    fall(&mut s, 60.);
    fall(&mut copy, 60.);
    assert_eq!(ground_bursts(&s), ground_bursts(&copy));
    assert_eq!(ground_bursts(&s).len(), 6);
    assert!(s.bombs.is_empty() && copy.bombs.is_empty());
}

#[test]
fn a_bomb_placed_in_flight_falls_from_where_it_is() {
    let mut s = carrying(mk82(), 1, [0.; 3]);
    let w = mk82();
    let release = ReleaseState::onto([500., 20., 900.], [0., -1., 1.], 600., 1_000.);
    s.projectiles.push(Projectile {
        id: 0x00F0_0000,
        owner: 99,
        weapon: Some(w),
        guidance: None,
        motion: None,
        guidance_ticks: None,
        age: 0,
        incoming: None,
        station: 0,
        position: release.position,
        previous: release.position,
        direction: unit(release.velocity),
        speed_f8: (600. * 256.) as i32,
        launched_t: 0,
        target: None,
        fall: FallState::default(),
        gun_round: None,
        tracer: false,
    });
    for _ in 0..(10 * 120) {
        if s.projectiles.is_empty() {
            break;
        }
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: away(),
            }],
            |_, _| 20.,
        );
    }
    let burst = *ground_bursts(&s).last().unwrap();
    assert!(horizontal(burst, [500., 20., 900.]) < 1., "{burst:?}");
}

#[test]
fn a_rocket_pod_holds_its_rockets_and_fires_one_a_call() {
    let mut s = carrying(lau61(), 2, [0.; 3]);
    // Two pods hold 38 rockets, and weigh two pods.
    assert_eq!(s.own().rounds(0), 38);
    assert!((s.own().payload_lbs() - 400.).abs() < 1e-9);
    let launcher = pass(3_000., 400., -10.);
    // A tap fires one rocket and debits one.
    step(&mut s, true, launcher);
    step(&mut s, false, launcher);
    assert_eq!(s.own().rounds(0), 37);
    assert_eq!(s.projectiles.len(), 1);
    assert!((s.own().payload_lbs() - 200. * 37. / 19.).abs() < 1e-9);
    // Rockets keep their own motion: no bomb path.
    assert!(s.bombs.is_empty());
    // Held, the repeat flag fires one every quarter second: 4 a second.
    for _ in 0..120 {
        step(&mut s, true, launcher);
    }
    let held = 37 - s.own().rounds(0);
    assert!((4..=5).contains(&held), "{held} rockets in a second held");
    // Held on, both pods run dry and the station stops.
    for _ in 0..(120 * 12) {
        step(&mut s, true, launcher);
    }
    assert_eq!(s.own().rounds(0), 0);
    assert_eq!(s.own().release_readiness, Readiness::Empty);
}

#[test]
fn a_pod_without_the_repeat_flag_fires_once_a_press() {
    let mut lau10 = lau61();
    lau10.source = "LAU10.JT".into();
    lau10.flags = 0x20046;
    lau10.burst.projectiles_in_pod = 4;
    lau10.burst.game_rounds_in_burst = 1;
    let mut s = carrying(lau10, 2, [0.; 3]);
    assert_eq!(s.own().rounds(0), 8);
    let launcher = pass(3_000., 400., -10.);
    for _ in 0..240 {
        step(&mut s, true, launcher);
    }
    assert_eq!(s.own().rounds(0), 7);
    step(&mut s, false, launcher);
    step(&mut s, true, launcher);
    assert_eq!(s.own().rounds(0), 6);
}

#[test]
fn guided_bombs_keep_their_own_path() {
    // A GBU-10 has neither the bomb flag nor the gravity flag: slice W1's.
    let mut w = mk82();
    w.source = "GBU10.JT".into();
    w.flags = 0x2a543;
    w.seeker.signature = 0;
    assert!(!ballistics::is_bomb(&w));
    let mut s = carrying(w, 1, [0.; 3]);
    step(&mut s, true, pass(5_000., 450., 0.));
    assert!(s.bombs.is_empty());
    let before = s.projectiles[0].position;
    step(&mut s, false, away());
    // Straight along the nose, as before this slice.
    assert!((s.projectiles[0].position[1] - before[1]).abs() < 1e-9);
}

#[test]
fn release_sound_is_shared_by_every_bomb_class_except_novelty() {
    for name in [
        "MK82.JT",
        "MK82P3.JT",
        "MK82HD.JT",
        "MK82P.JT",
        "MK82AIR.JT",
        "MK84.JT",
        "CBU87.JT",
        "CBU89.JT",
        "MK20.JT",
        "RBK250.JT",
        "RBK500.JT",
        "FAB250.JT",
        "FAB500.JT",
        "FAB1000.JT",
    ] {
        let mut w = mk82();
        w.source = name.into();
        w.fire_sound = Some("&bomb2.11k".into());
        assert_eq!(crate::combat::release_sound(&w), Some("&bomb.11k"));
    }
    for name in [
        "GBU10.JT",
        "GBU10A.JT",
        "PAVEWAY.JT",
        "PAVEWA3.JT",
        "GBU28.JT",
        "GBU29.JT",
        "GBU29P.JT",
        "GBU30.JT",
    ] {
        let mut w = super::guided_tests::gbu10();
        w.source = name.into();
        assert_eq!(crate::combat::release_sound(&w), Some("&bomb.11k"));
    }
    for name in ["~BLDR.JT", "~MOOSE.JT"] {
        let mut w = mk82();
        w.source = name.into();
        w.fire_sound = Some("&mothbf.11k".into());
        assert_eq!(crate::combat::release_sound(&w), Some("&mothbf.11k"));
    }
    let mut rocket = lau61();
    rocket.fire_sound = Some("rocket".into());
    assert_eq!(crate::combat::release_sound(&rocket), Some("rocket"));
}

#[test]
fn fitted_bomb_flight_acceptance_measurements() {
    use crate::combat::bomb_flight::{Body, DT, Flight};
    let mut gbu = super::guided_tests::gbu10();
    gbu.weight = 2_000;
    let mut air = mk82();
    air.source = "MK82AIR.JT".into();
    air.movement.launch_retard = 66;
    for (w, height, range) in [
        (gbu, 20_000., Some(24_000.)),
        (mk82(), 20_000., None),
        (air, 500., None),
    ] {
        let body = Body::of(&w);
        let mut flight = Flight {
            position: [0., height, 0.],
            velocity: [0., 0., 450. * KNOT],
        };
        let target = range.map(|z| [0., 0., z]);
        let predicted = target.and_then(|point| {
            crate::combat::guided::time_to(
                &w,
                missiles::Motion::launch(&w, flight.velocity, height),
                flight.position,
                [0., 0., 1.],
                point,
                0,
            )
        });
        let mut tick = 0;
        while flight.position[1] > 0. && tick < 36_000 {
            flight.step(body, tick, target);
            tick += 1;
        }
        let speed = missiles::length(flight.velocity) / KNOT;
        println!(
            "bomb-acceptance {} release-kt=450 arrival-kt={speed:.3} seconds={:.5} range-ft={:.3} miss-ft={:.3}",
            w.source,
            tick as f64 * DT,
            flight.position[2],
            range.map_or(0., |r| (r - flight.position[2]).abs())
        );
        assert!(tick < 36_000);
        if w.source == "GBU10.JT" {
            assert!(speed > 300., "{speed}");
            assert!((predicted.unwrap() - tick as f64 * DT).abs() <= DT);
            assert!((flight.position[2] - range.unwrap()).abs() < 25.);
        } else if height == 500. {
            assert!(flight.position[2] < 450. * KNOT * tick as f64 * DT - 500.);
        } else {
            assert!(speed > 450.);
            assert!(flight.position[2] > 20_000. && flight.position[2] < 30_000.);
        }
    }
}

#[test]
fn bombs_follow_aircraft_velocity_even_when_the_nose_is_pitched_up() {
    let mut launcher = pass(5_000., 450., 0.);
    launcher.basis = Basis::new(0., 20_f64.to_radians(), 0.);
    let s = carrying(mk82(), 1, [0.; 3]);
    let release = ReleaseState::from_station(&launcher, &s.own().configuration().stations[0], 0);
    assert_eq!(release.velocity, launcher.velocity);
    assert_eq!(
        crate::combat::guided::release_velocity(&super::guided_tests::gbu10(), &launcher),
        launcher.velocity
    );
}

#[test]
fn per_store_bomb_flight_measurements() {
    use crate::combat::bomb_flight::{Body, DT, Flight};
    // Synthetic records with the reviewed mass, pack count and class metadata.
    let stores = [
        ("CBU87.JT", 0x82a052, 500, 90, 1),
        ("CBU89.JT", 0x82a052, 700, 90, 1),
        ("FAB1000.JT", 0x22012, 2272, 100, 1),
        ("FAB250.JT", 0x22012, 550, 100, 1),
        ("FAB500.JT", 0x22012, 1093, 100, 1),
        ("GBU10.JT", 0x2a543, 2000, 90, 1),
        ("GBU10A.JT", 0x2a543, 2000, 90, 1),
        ("GBU28.JT", 0x2a543, 4500, 90, 1),
        ("GBU29.JT", 0x2a043, 2000, 90, 1),
        ("GBU29P.JT", 0x2a043, 2000, 90, 1),
        ("GBU30.JT", 0x2a043, 1000, 90, 1),
        ("MK20.JT", 0x82a052, 400, 90, 1),
        ("MK82.JT", 0x22012, 500, 100, 1),
        ("MK82AIR.JT", 0x22012, 3500, 66, 7),
        ("MK82HD.JT", 0x22012, 500, 66, 1),
        ("MK82P.JT", 0x22012, 3000, 66, 6),
        ("MK82P3.JT", 0x22012, 1500, 100, 3),
        ("MK84.JT", 0x22012, 2000, 100, 1),
        ("PAVEWA3.JT", 0x2a543, 2000, 90, 1),
        ("PAVEWAY.JT", 0x2a543, 2000, 90, 1),
        ("RBK250.JT", 0x82a052, 500, 90, 1),
        ("RBK500.JT", 0x82a052, 700, 90, 1),
        ("~BLDR.JT", 0x22012, 300, 100, 1),
        ("~MOOSE.JT", 0x22012, 300, 100, 1),
    ];
    for (name, flags, weight, retard, count) in stores {
        let guided = flags & 0x10 == 0;
        let mut w = if guided {
            super::guided_tests::gbu10()
        } else {
            mk82()
        };
        w.source = name.into();
        w.flags = flags;
        w.weight = weight;
        w.movement.launch_retard = retard;
        w.burst.game_rounds_in_burst = count;
        if name.starts_with("GBU29") || name == "GBU30.JT" {
            w.seeker.signature = 0;
        }
        let body = Body::of(&w);
        let mut flight = Flight {
            position: [0., 20_000., 0.],
            velocity: [0., 0., 450. * KNOT],
        };
        let mut tick = 0;
        while flight.position[1] > 0. && tick < 36_000 {
            flight.step(body, tick, guided.then_some([0., 0., 24_000.]));
            tick += 1;
        }
        assert!(tick < 36_000);
        println!(
            "store-flight {name} release-kt=450 arrival-kt={:.3} seconds={:.5} range-ft={:.3}",
            missiles::length(flight.velocity) / KNOT,
            tick as f64 * DT,
            flight.position[2]
        );
    }
}

#[test]
fn a_retarded_bomb_keeps_falling_past_the_record_lifetime() {
    let mut w = mk82();
    w.source = "MK82AIR.JT".into();
    w.movement.launch_retard = 66;
    w.movement.remove_t = 4;
    let mut s = carrying(w, 1, [0.; 3]);
    step(&mut s, true, pass(20_000., 450., 0.));
    for _ in 0..240 {
        step(&mut s, false, away());
    }
    assert_eq!(s.projectiles.len(), 1);
    fall(&mut s, 100.);
    assert!(s.projectiles.is_empty());
    assert_eq!(ground_bursts(&s).len(), 1);
}
