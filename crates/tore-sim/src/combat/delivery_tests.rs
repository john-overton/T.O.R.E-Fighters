//! The delivery solution on its own (docs/spec/air-to-ground.md, "The HUD in
//! air-to-ground modes"; docs/formats/weapons.md, "Bomb sight").
use super::*;
use crate::attitude::Basis;
use crate::combat::ballistics::BombMotion;
use crate::sensors;

const KNOT: f64 = 1.687_8;

/// A Mk 82 as the record has it, with a 20,000 ft zone-1 range.
fn mk82() -> Weapon {
    let mut w = crate::combat::gunsight::tests::weapon();
    w.source = "MK82.JT".into();
    w.hud_name = "MK-82".into();
    w.flags = 0x22012;
    w.movement.minimum_speed = 0;
    w.movement.maximum_speed = 1466;
    w.movement.initial_speed = 0;
    w.movement.final_speed = 0;
    w.movement.launch_retard = 100;
    w.movement.remove_t = 240;
    w.seeker.zones[1].maximum_range = 20_000;
    w.damage.fuze_arm_t = 4;
    w.damage.fuze_radius = 300;
    w.damage.collateral_radius = 500;
    w
}

/// A LAU-61: a pod of 19 rockets, the speed command and the gravity flag.
fn lau61() -> Weapon {
    let mut w = crate::combat::gunsight::tests::weapon();
    w.source = "LAU61.JT".into();
    w.flags = 0x20846;
    w.burst.projectiles_in_pod = 19;
    w.movement.remove_t = 80;
    w.seeker.zones[1].maximum_range = 12_000;
    w
}

fn station(weapon: Weapon) -> Station {
    Station {
        weapon,
        mount: [0.; 3],
        count: 4,
        internal: false,
        rack: Default::default(),
    }
}

/// Heading north at `knots`, `altitude` ft up, pitched `dive` degrees.
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

const CLOCK: Clock = Clock {
    tick: 0,
    service: 2,
    remainder: 16,
};

fn solved(
    station: &Station,
    launcher: &Launcher,
    mode: DeliveryMode,
    target: Option<Vector>,
) -> Delivery {
    solve(
        Situation {
            index: 3,
            station,
            mount: station.mount,
            launcher,
            mode,
            target,
            target_velocity: [0.; 3],
            clock: CLOCK,
            falling: None,
            ballistic: false,
            beyond_life: false,
        },
        |_, _| 0.,
    )
    .expect("a bomb or rocket station")
}

#[test]
fn the_arc_is_whole_inside_four_tenths_and_a_bare_ring_from_the_maximum() {
    let r = 20_000.;
    assert_eq!(arc_steps(0., r), ARC_STEPS);
    assert_eq!(arc_steps(0.4 * r, r), ARC_STEPS);
    // Just past 0.4 R the arc loses its last step: glyph 0x2e.
    assert_eq!(arc_steps(0.4 * r + 1., r), 14);
    // One fourteenth of the remaining 0.6 R takes one more.
    assert_eq!(arc_steps(0.4 * r + 0.6 * r / 14. + 1., r), 13);
    // Just short of the maximum one step is left (0x21); from it, none (0x20).
    assert_eq!(arc_steps(r - 1., r), 1);
    assert_eq!(arc_steps(r, r), 0);
    assert_eq!(arc_steps(3. * r, r), 0);
    // No range: no arc.
    assert_eq!(arc_steps(10., 0.), 0);
    assert_eq!(arc_steps(f64::NAN, r), 0);
    // The steps never grow with range.
    let mut last = ARC_STEPS;
    for d in (0..25_000).step_by(37) {
        let steps = arc_steps(f64::from(d), r);
        assert!(steps <= last);
        last = steps;
    }
}

#[test]
fn the_arc_range_is_retails_approximate_distance() {
    assert_eq!(retail_distance([0.; 3], [100., -40., 20.]), 100. + 60. / 4.);
    assert_eq!(retail_distance([5., 5., 5.], [5., 5., 5.]), 0.);
    // Never shorter than the true distance's largest axis.
    let d = retail_distance([0.; 3], [3_000., 4_000., 0.]);
    assert_eq!(d, 4_000. + 750.);
}

#[test]
fn ccip_is_the_bomb_predictors_mean_impact_with_its_glyph() {
    let s = station(mk82());
    let launcher = pass(5_000., 450., 0.);
    let d = solved(&s, &launcher, DeliveryMode::Ccip, None);
    assert_eq!(
        (d.station, d.mode, d.rocket),
        (3, DeliveryMode::Ccip, false)
    );
    let c = d.ccip.unwrap();
    let release = ReleaseState::from_station(&launcher, &s, 0);
    let impact = predict_impact(release, |_, _| 0.).unwrap();
    assert_eq!(c.impact, impact.position);
    assert_eq!(c.seconds, impact.seconds);
    // 5,000 ft: 17.7 s of fall, 13,400 ft of throw, 14,300 ft away: the
    // retail distance puts it short of 20,000 ft with part of the arc.
    assert!((c.seconds - (5_000_f64 / 16.).sqrt()).abs() < 1e-9);
    assert_eq!(c.range_ft, retail_distance(launcher.position, c.impact));
    assert!(c.in_range());
    assert_eq!(c.glyph(), 0x20 + arc_steps(c.range_ft, 20_000.));
    assert!(c.arc > 0 && c.arc < ARC_STEPS);
    assert!(d.level.is_none());
    assert!(d.warnings.is_empty());
}

#[test]
fn the_dot_trail_marks_each_quarter_second_of_the_fall() {
    let s = station(mk82());
    let launcher = pass(3_000., 400., -20.);
    let c = solved(&s, &launcher, DeliveryMode::Ccip, None)
        .ccip
        .unwrap();
    let dots: Vec<Vector> = c.trail().collect();
    assert_eq!(dots.len(), (c.seconds / TRAIL_SECONDS).floor() as usize + 1);
    assert_eq!(dots[0], c.release.position);
    let motion = BombMotion::new(c.release);
    for (k, dot) in dots.iter().enumerate() {
        assert_eq!(*dot, motion.position(k as f64 * 0.25));
    }
    assert!(dots.last().unwrap()[1] >= -1e-9);
}

/// [`solved`] over `ground`, against a target moving at `velocity`.
fn solved_over(
    station: &Station,
    launcher: &Launcher,
    target: Vector,
    velocity: Vector,
    ground: impl Fn(f64, f64) -> f64,
) -> Delivery {
    solve(
        Situation {
            index: 3,
            station,
            mount: station.mount,
            launcher,
            mode: DeliveryMode::Level,
            target: Some(target),
            target_velocity: velocity,
            clock: CLOCK,
            falling: None,
            ballistic: false,
            beyond_life: false,
        },
        ground,
    )
    .expect("a bomb station")
}

#[test]
fn the_level_t_brings_the_bomb_down_on_the_targets_ground() {
    // Slice ST-LV. Level at 5,000 ft and 450 kt over sea level; the target
    // stands on a 400 ft plateau, or in a 400 ft hollow. The T is due where
    // the bomb's mean path meets the target's ground, not the ground under
    // the aircraft.
    let s = station(mk82());
    let launcher = pass(5_000., 450., 0.);
    let release = ReleaseState::from_station(&launcher, &s, 0);
    for height in [400., -400.] {
        let ground = move |_: f64, z: f64| if z > 6_000. { height } else { 0. };
        let lands = ballistics::impact_at(release, height).unwrap().position;
        let target = [lands[0], height, lands[2]];
        let d = solved_over(&s, &launcher, target, [0.; 3], ground);
        // The pipper (slice ST-G) now meets the same ground the path does.
        let pipper = d.ccip.unwrap();
        assert!((pipper.impact[2] - lands[2]).abs() < 1e-6, "{height}");
        assert_eq!(pipper.impact[1], height);
        let l = d.level.unwrap();
        assert!(l.along_ft.abs() < 1e-6, "{height}: D {}", l.along_ft);
        assert!(l.cross_ft.abs() < 1e-6);
        assert!(l.due());
        // The same bomb over the plateau falls about 400 x 759 / 566 ft
        // short of where it would land at sea level, beyond it in the hollow.
        let sea = predict_impact(release, |_, _| 0.).unwrap().position;
        let gap = sea[2] - lands[2];
        assert!((gap.abs() - 520.).abs() < 60., "{height}: {gap}");
        assert_eq!(gap > 0., height > 0.);
    }
}

#[test]
fn the_level_t_leads_a_moving_target_by_the_fall() {
    // Slice ST-LV: a column driving at 50 ft/s toward the aircraft is due
    // where it will be when the bomb lands, not where it is now.
    let s = station(mk82());
    let launcher = pass(5_000., 450., 0.);
    let release = ReleaseState::from_station(&launcher, &s, 0);
    let lands = predict_impact(release, |_, _| 0.).unwrap();
    let velocity = [0., 0., -50.];
    let now = [
        lands.position[0],
        0.,
        lands.position[2] + 50. * lands.seconds,
    ];
    let l = solved_over(&s, &launcher, now, velocity, |_, _| 0.)
        .level
        .unwrap();
    assert!(l.along_ft.abs() < 1e-6, "D {}", l.along_ft);
    assert!(l.due());
    assert!((l.target[2] - lands.position[2]).abs() < 1e-6);
    // The column now under the impact point is overflown by the time it lands.
    let under = [lands.position[0], 0., lands.position[2]];
    let l = solved_over(&s, &launcher, under, velocity, |_, _| 0.)
        .level
        .unwrap();
    assert!((l.along_ft + 50. * lands.seconds).abs() < 1e-6);
    // A standing target is not led.
    let l = solved_over(&s, &launcher, under, [0.; 3], |_, _| 0.)
        .level
        .unwrap();
    assert!(l.along_ft.abs() < 1e-6);
    assert_eq!(l.target, under);
}

#[test]
fn level_errors_are_signed_along_and_across_the_impact_track() {
    let s = station(mk82());
    let launcher = pass(5_000., 450., 0.);
    let c = solved(&s, &launcher, DeliveryMode::Ccip, None)
        .ccip
        .unwrap();
    // Heading north (+z): right is +x.
    let ahead = [c.impact[0], 0., c.impact[2] + 3_000.];
    let d = solved(&s, &launcher, DeliveryMode::Level, Some(ahead));
    let l = d.level.unwrap();
    assert!((l.along_ft - 3_000.).abs() < 1e-6);
    assert!(l.cross_ft.abs() < 1e-6);
    let speed = 450. * KNOT;
    assert!((l.ground_speed_fps - speed).abs() < 1e-9);
    assert!((l.release_seconds - 3_000. / speed).abs() < 1e-9);
    assert!(l.steering_rad.abs() < 1e-9);
    assert!(!l.overflown());
    assert!(!l.due());
    assert_eq!(l.radius_ft, 500.);
    assert!((l.marker_fraction() - l.release_seconds / MARKER_SECONDS).abs() < 1e-12);
    // Off to the right: positive cross-track error, a right turn on the stem.
    let right = [c.impact[0] + 400., 0., c.impact[2] + 3_000.];
    let l = solved(&s, &launcher, DeliveryMode::Level, Some(right))
        .level
        .unwrap();
    assert!((l.cross_ft - 400.).abs() < 1e-6);
    assert!(l.steering_rad > 0.);
    let left = [c.impact[0] - 400., 0., c.impact[2] + 3_000.];
    let l = solved(&s, &launcher, DeliveryMode::Level, Some(left))
        .level
        .unwrap();
    assert!((l.cross_ft + 400.).abs() < 1e-6);
    assert!(l.steering_rad < 0.);
    // Far out: the marker is at the top and the stem dashed.
    let far = [c.impact[0], 0., c.impact[2] + 61. * speed];
    let l = solved(&s, &launcher, DeliveryMode::Level, Some(far))
        .level
        .unwrap();
    assert!(l.far());
    assert_eq!(l.marker_fraction(), 1.);
}

#[test]
fn the_cue_is_due_at_the_target_inside_the_radius_and_overflown_after() {
    let s = station(mk82());
    let launcher = pass(5_000., 450., 0.);
    let c = solved(&s, &launcher, DeliveryMode::Ccip, None)
        .ccip
        .unwrap();
    let tick = 450. * KNOT / 120.;
    let at = |along: f64, across: f64| {
        let target = [c.impact[0] + across, 0., c.impact[2] + along];
        solved(&s, &launcher, DeliveryMode::Level, Some(target))
    };
    // Half a tick before the cue: not yet.
    assert_eq!(at(tick / 2., 0.).level_trigger(3), Some(false));
    // On it, or up to the window past it: release.
    assert_eq!(at(0., 0.).level_trigger(3), Some(true));
    assert_eq!(at(-2. * tick, 499.).level_trigger(3), Some(true));
    // Outside the radius: the held trigger waits in vain.
    assert_eq!(at(0., 501.).level_trigger(3), Some(false));
    // Past the window LVL gives the trigger back: a press releases at once.
    let past = at(-4. * tick, 0.);
    assert_eq!(past.level_trigger(3), None);
    assert!(past.level.unwrap().overflown());
    assert_eq!(past.warnings, [Warning::Overfly]);
    assert_eq!(past.warning(), Some(Warning::Overfly));
    // The cue only holds its own station, and CCIP never holds it.
    assert_eq!(at(0., 0.).level_trigger(2), None);
    let ccip = solved(&s, &launcher, DeliveryMode::Ccip, Some(c.impact));
    assert_eq!(ccip.level_trigger(3), None);
    // LVL without a target is the plain press too.
    assert_eq!(
        solved(&s, &launcher, DeliveryMode::Level, None).level_trigger(3),
        None
    );
}

#[test]
fn low_warns_of_a_fall_shorter_than_the_fuze_arm_time() {
    let mut w = mk82();
    // Two seconds to arm.
    w.damage.fuze_arm_t = 8;
    w.damage.collateral_radius = 0;
    let s = station(w);
    // 50 ft: under two seconds of fall.
    let low = solved(&s, &pass(50., 450., 0.), DeliveryMode::Ccip, None);
    assert!(low.ccip.unwrap().seconds < 2.);
    assert_eq!(low.warnings, [Warning::Low]);
    // 100 ft: 2.5 s, armed.
    let high = solved(&s, &pass(100., 450., 0.), DeliveryMode::Ccip, None);
    assert!(high.ccip.unwrap().seconds > 2.);
    assert!(high.warnings.is_empty());
}

#[test]
fn frag_warns_when_the_burst_reaches_the_aircraft_flying_on() {
    // A slick bomb from a level pass lands under the aircraft: 300 ft up is
    // inside a 500 ft collateral radius, 1,000 ft is not.
    let s = station(mk82());
    let low = solved(&s, &pass(300., 450., 0.), DeliveryMode::Ccip, None);
    assert_eq!(low.warnings, [Warning::Frag]);
    let high = solved(&s, &pass(1_000., 450., 0.), DeliveryMode::Ccip, None);
    assert!(high.warnings.is_empty());
    // A dive is taken to be pulled out at the release height: from 400 ft
    // the burst reaches the aircraft, from 2,500 ft it does not, though
    // flying the dive on would end in the crater.
    let dive = solved(&s, &pass(400., 450., -30.), DeliveryMode::Ccip, None);
    assert!(dive.warnings.contains(&Warning::Frag));
    let dive = solved(&s, &pass(2_500., 450., -30.), DeliveryMode::Ccip, None);
    assert!(dive.warnings.is_empty());
    // A climb keeps climbing.
    let climb = pass(300., 450., 20.);
    let up = flown(&climb, 2.);
    assert!((up[1] - (300. + 2. * climb.velocity[1])).abs() < 1e-9);
    let down = flown(&pass(300., 450., -20.), 2.);
    assert_eq!(down[1], 300.);
}

#[test]
fn a_new_bomb_starts_in_level_only_with_a_ground_target_near_level() {
    use DeliveryMode::*;
    assert_eq!(DeliveryMode::default(), Ccip);
    assert_eq!(DeliveryMode::chosen(true, 0.), Level);
    assert_eq!(DeliveryMode::chosen(true, 9.9_f64.to_radians()), Level);
    assert_eq!(DeliveryMode::chosen(true, -10_f64.to_radians()), Level);
    assert_eq!(DeliveryMode::chosen(true, -10.1_f64.to_radians()), Ccip);
    assert_eq!(DeliveryMode::chosen(false, 0.), Ccip);
    assert_eq!(Ccip.toggled(), Level);
    assert_eq!(Level.toggled(), Ccip);
    assert_eq!((Ccip.label(), Level.label()), ("CCIP", "LVL"));
    assert_eq!(
        [Warning::Low, Warning::Frag, Warning::Overfly].map(Warning::label),
        ["LOW", "FRAG", "OVERFLY"]
    );
}

#[test]
fn rockets_get_a_pipper_and_never_the_level_release() {
    let s = station(lau61());
    let launcher = pass(1_500., 400., -10.);
    let d = solved(&s, &launcher, DeliveryMode::Level, Some([0., 0., 5_000.]));
    assert!(d.rocket);
    assert_eq!(d.mode, DeliveryMode::Ccip);
    assert!(d.level.is_none());
    let c = d.ccip.unwrap();
    // Down range, short of where the bore meets the ground, and under 20 s.
    let bore = 1_500. / 10_f64.to_radians().tan();
    assert!(c.impact[2] > 0. && c.impact[2] < bore + 1.);
    assert!(c.seconds > 0. && c.seconds < 20.);
    assert!(c.impact[1].abs() < 1.);
    // Guided records and guns are not this sight's.
    let mut guided = lau61();
    guided.seeker.signature = 2;
    let solved_guided = solve(
        Situation {
            index: 0,
            station: &station(guided),
            mount: [0.; 3],
            launcher: &launcher,
            mode: DeliveryMode::Ccip,
            target: None,
            target_velocity: [0.; 3],
            clock: CLOCK,
            falling: None,
            ballistic: false,
            beyond_life: false,
        },
        |_, _| 0.,
    );
    assert!(solved_guided.is_none());
}

/// The human's solution: the rocket pipper carries on past the lifetime.
fn solved_beyond(station: &Station, launcher: &Launcher) -> Option<Delivery> {
    solve(
        Situation {
            index: 0,
            station,
            mount: station.mount,
            launcher,
            mode: DeliveryMode::Ccip,
            target: None,
            target_velocity: [0.; 3],
            clock: CLOCK,
            falling: None,
            ballistic: false,
            beyond_life: true,
        },
        |_, _| 0.,
    )
}

#[test]
fn the_ais_rocket_gate_still_sees_no_impact_where_the_life_ends_first() {
    // Climbing away, or level too high: the 20 s life ends first, and the AI's
    // solution (no continuation) has no pipper, as before the human's
    // pipper was made permanent.
    let s = station(lau61());
    for launcher in [pass(1_500., 400., 30.), pass(3_000., 400., 0.)] {
        let d = solved(&s, &launcher, DeliveryMode::Ccip, None);
        assert!(d.ccip.is_none());
        assert!(d.warnings.is_empty());
    }
}

#[test]
fn a_human_rockets_pipper_is_on_the_ground_past_the_lifetime() {
    let s = station(lau61());
    // Level at 3,000 ft: the rocket sinks 1,500 ft in its 20 s, so the AI's
    // fly-out has no impact; the continuation brings it down on the ground.
    let level = pass(3_000., 400., 0.);
    assert!(rocket_impact(&s.weapon, &level, s.mount, CLOCK, &|_, _| 0.).is_none());
    let c = solved_beyond(&s, &level).unwrap().ccip.unwrap();
    assert!(c.impact[1].abs() < 5., "{}", c.impact[1]);
    assert!(c.seconds > 20., "{}", c.seconds);
    assert!(c.impact[2] > 0.);
    // A steeper pass reaches the same ground inside the life, and the
    // continuation agrees with the in-life solution exactly there.
    let dive = pass(1_500., 400., -10.);
    let inside = rocket_impact(&s.weapon, &dive, s.mount, CLOCK, &|_, _| 0.).unwrap();
    let beyond = rocket_impact_beyond(&s.weapon, &dive, s.mount, CLOCK, &|_, _| 0.).unwrap();
    assert_eq!(inside.1, beyond.1);
    // Nose above the horizon: no ground, so the pipper marks where the round
    // stands when its life ends, 20 s out and above the aircraft.
    let climb = pass(1_500., 400., 30.);
    let c = solved_beyond(&s, &climb).unwrap().ccip.unwrap();
    assert!((c.seconds - 20.).abs() < 0.1, "{}", c.seconds);
    assert!(c.impact[1] > 1_500. && c.impact[2] > 10_000.);
    assert!(!c.in_range());
}

#[test]
fn the_continuation_follows_rising_ground() {
    // A ridge 2,000 ft up, 20,000 ft out, stops a level pass the flat ground
    // would not.
    let s = station(lau61());
    let level = pass(3_000., 400., 0.);
    let ridge = |_: f64, z: f64| if z > 20_000. { 2_000. } else { 0. };
    let flat = rocket_impact_beyond(&s.weapon, &level, s.mount, CLOCK, &|_, _| 0.).unwrap();
    let hit = rocket_impact_beyond(&s.weapon, &level, s.mount, CLOCK, &ridge).unwrap();
    assert!(
        hit.1.seconds < flat.1.seconds,
        "{} {}",
        hit.1.seconds,
        flat.1.seconds
    );
    // It lands on the face of the ridge, at its foot (the step is a tenth of a second).
    assert!(hit.1.position[1] <= 2_000. && (hit.1.position[2] - 20_000.).abs() < 400.);
}

#[test]
fn reach_finds_the_interval_about_the_throw_the_fly_out_accepts() {
    let release = ReleaseState::new([0., 15_000., 0.], [0., 0., 1.], 760.);
    let throw = 760. * (15_000_f64 / 16.).sqrt();
    let mut calls = 0;
    let reach = Reach::search(release, 0., 200_000., |range| {
        calls += 1;
        (throw - 4_000.0..=throw + 9_000.).contains(&range)
    })
    .unwrap();
    assert!((reach.minimum_ft - (throw - 4_000.)).abs() <= Reach::TOLERANCE_FT);
    assert!((reach.maximum_ft - (throw + 9_000.)).abs() <= Reach::TOLERANCE_FT);
    assert!(reach.minimum_ft >= throw - 4_000. && reach.maximum_ft <= throw + 9_000.);
    assert!(calls < 60, "{calls} fly-outs");
    assert!(reach.contains(throw));
    assert!(!reach.contains(throw + 10_000.));
    assert_eq!(reach.fraction(reach.minimum_ft), 0.);
    assert_eq!(reach.fraction(reach.maximum_ft), 1.);
    assert_eq!(reach.fraction(1e9), 1.);
    // Bounded by the limit and by zero.
    let wide = Reach::search(release, 0., 30_000., |_| true).unwrap();
    assert_eq!((wide.minimum_ft, wide.maximum_ft), (0., 30_000.));
    // The unguided throw missed: no reach.
    assert!(Reach::search(release, 0., 200_000., |range| range > throw + 1.).is_none());
    // The same fly-out gives the same reach.
    let again = Reach::search(release, 0., 200_000., |range| {
        (throw - 4_000.0..=throw + 9_000.).contains(&range)
    });
    assert_eq!(again, Some(reach));
}

#[test]
fn reach_grows_with_release_height_for_a_glide_that_does() {
    // A synthetic glide (W1 brings the real one): the bomb makes good four
    // times its height beyond the throw and half of it short.
    let reach_from = |height: f64| {
        let release = ReleaseState::new([0., height, 0.], [0., 0., 1.], 700.);
        let throw = 700. * (height / 16.).sqrt();
        Reach::search(release, 0., 500_000., |range| {
            (throw - height / 2.0..=throw + 4. * height).contains(&range)
        })
        .unwrap()
    };
    let low = reach_from(5_000.);
    let high = reach_from(20_000.);
    assert!(high.maximum_ft > low.maximum_ft);
    assert!(high.maximum_ft - high.minimum_ft > low.maximum_ft - low.minimum_ft);
}

#[test]
fn the_mode_round_trips_in_a_checkpoint() {
    use crate::checkpoint::{Models, from_bytes, to_bytes};
    for mode in [DeliveryMode::Ccip, DeliveryMode::Level] {
        let models = Models::default();
        let coded = to_bytes(&mode, &models).unwrap();
        let back: DeliveryMode = from_bytes(&coded, &models).unwrap();
        assert_eq!(back, mode);
    }
}

#[test]
fn the_ccip_pipper_lays_on_a_target_on_a_rise_or_in_a_hollow() {
    // John, 2026-10-11: realistic CCIP. Retail works the fall out over the
    // ground under the aircraft, so a pipper laid on a target 400 ft up
    // bombed it about 520 ft short and one in a hollow as far long. The
    // pipper now finds where the path meets the terrain: the bomb it shows
    // is the bomb that lands (here with no scatter).
    let s = station(mk82());
    let launcher = pass(5_000., 450., 0.);
    let release = ReleaseState::from_station(&launcher, &s, 0);
    for height in [400., -400., 0.] {
        let ground = move |_: f64, z: f64| if z > 6_000. { height } else { 0. };
        let d = solve(
            Situation {
                index: 3,
                station: &s,
                mount: s.mount,
                launcher: &launcher,
                mode: DeliveryMode::Ccip,
                target: None,
                target_velocity: [0.; 3],
                clock: CLOCK,
                falling: None,
                ballistic: false,
                beyond_life: false,
            },
            ground,
        )
        .expect("a bomb station");
        let pipper = d.ccip.unwrap();
        let lands = ballistics::impact_at(release, height).unwrap();
        assert!(
            (pipper.impact[2] - lands.position[2]).abs() < 1e-6,
            "{height}"
        );
        assert!((pipper.seconds - lands.seconds).abs() < 1e-9);
        assert_eq!(pipper.impact[1], height);
        // The AI's CCIP cue is the pipper's point; in LVL it is the path
        // brought down to the target's ground, the same point here.
        let cue = d.cue(height).unwrap();
        assert_eq!(cue.position, pipper.impact);
        // Retail's reading would have missed by the slope's offset.
        let retail = ballistics::predict_impact_flat(release, ground).unwrap();
        let miss = (retail.position[2] - lands.position[2]).abs();
        assert_eq!(miss > 300., height != 0., "{height}: {miss}");
    }
}
