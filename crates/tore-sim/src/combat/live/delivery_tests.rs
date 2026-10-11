//! The bomb and rocket sights in the live adapter (slice H1): the solution
//! every step, Ctrl+M, the mode a new bomb starts in, the level release's
//! held trigger and the rocket pipper against the flown rocket
//! (docs/spec/air-to-ground.md, "The HUD in air-to-ground modes").
use super::tests::fixture;
use super::*;
use crate::combat::ballistics::{ReleaseState, predict_impact};
use crate::combat::delivery::{DeliveryMode, RELEASE_WINDOW_TICKS, Warning};
use crate::models::strike_class::StrikeClass;

const KNOT: f64 = 1.687_8;
const TARGET: u32 = 0x4000_0000;

/// A Mk 82: bomb flag, full launch retard, a 300 ft fuze and 500 ft splash.
fn mk82() -> Weapon {
    let mut w = fixture(false).own().configuration().stations[0]
        .weapon
        .clone();
    w.source = "MK82.JT".into();
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
    w.damage.fuze_radius = 300;
    w.damage.collateral_radius = 500;
    w.seeker.signature = 0;
    w.seeker.zones[1].maximum_range = 20_000;
    w
}

/// A LAU-61 as B1's tests have it.
fn lau61() -> Weapon {
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
    w.seeker.zones[1].maximum_range = 12_000;
    w
}

/// A gun on station 0 and `weapon` on station 1.
fn carrying(weapon: Weapon, count: u16) -> State {
    carrying_as(weapon, count, StrikeClass::Fighter)
}

/// [`carrying`] for an aircraft of the given bombing class.
fn carrying_as(weapon: Weapon, count: u16, class: StrikeClass) -> State {
    let mut config = fixture(false).own().configuration().clone();
    config.strike_class = class;
    let gun = config.stations[0].clone();
    config.stations = vec![
        gun,
        Station {
            weapon,
            mount: [0.; 3],
            count,
            internal: false,
        },
    ];
    let mut s = State::new(config, true).unwrap();
    s.own_mut().selected = 1;
    s
}

/// Heading north at `knots`, pitched `dive` degrees, at `[0, altitude, z]`.
fn pass(altitude: f64, z: f64, knots: f64, dive: f64) -> Launcher {
    let basis = Basis::new(0., dive.to_radians(), 0.);
    let speed = knots * KNOT;
    Launcher {
        position: [0., altitude, z],
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

fn away() -> Launcher {
    pass(40_000., -400_000., 450., 0.)
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

/// A ground target at `(x, z)`, designated as Ctrl+X does under Easy
/// targeting, which keeps it while it stands.
fn designate_ground(s: &mut State, x: f64, z: f64) {
    let bounds = crate::airport::OrientedBox {
        center: [x, 6., z],
        half: [10., 6., 20.],
        heading: 0.,
        pitch: 0.,
        bank: 0.,
    };
    s.add_ground_target(TARGET, bounds, 1_000, 0x100, NO_SIDE)
        .unwrap();
    s.cheats.easy_targeting = true;
    s.own_mut().sensors.select_ground(TARGET);
}

fn bombs(s: &State) -> usize {
    s.projectiles.iter().filter(|p| p.station == 1).count()
}

#[test]
fn the_sight_is_worked_out_every_step_for_the_selected_bomb() {
    let mut s = carrying(mk82(), 4);
    let launcher = pass(5_000., 0., 450., 0.);
    step(&mut s, false, launcher);
    let d = s.own().delivery.clone().expect("a bomb is selected");
    assert_eq!(
        (d.station, d.mode, d.rocket),
        (1, DeliveryMode::Ccip, false)
    );
    let c = d.ccip.unwrap();
    let release = ReleaseState::from_station(&launcher, &s.own().configuration().stations[1], 0);
    assert_eq!(c.release, release);
    assert_eq!(
        c.impact,
        predict_impact(release, |_, _| 0.).unwrap().position
    );
    // The gun has no bomb sight; NAV has none at all.
    s.own_mut().selected = 0;
    step(&mut s, false, launcher);
    assert!(s.own().delivery.is_none());
    s.command(0, Command::SelectNav, launcher);
    step(&mut s, false, launcher);
    assert!(s.own().delivery.is_none());
}

#[test]
fn ctrl_m_switches_a_bombs_sight_and_a_new_bomb_picks_its_mode() {
    let mut s = carrying(mk82(), 4);
    let level = pass(5_000., 0., 450., 0.);
    s.command(0, Command::ToggleSeekerMode, level);
    assert_eq!(s.own().delivery_mode, DeliveryMode::Level);
    // The missile's launch mode is not touched.
    assert_eq!(s.own().launch_mode, LaunchMode::Cued);
    s.command(0, Command::ToggleSeekerMode, level);
    assert_eq!(s.own().delivery_mode, DeliveryMode::Ccip);
    // Selecting the bomb with a ground target designated in level flight
    // starts it in LVL; in a 30 degree dive, in CCIP.
    designate_ground(&mut s, 0., 20_000.);
    for (launcher, mode) in [
        (level, DeliveryMode::Level),
        (pass(5_000., 0., 450., -30.), DeliveryMode::Ccip),
    ] {
        s.command(0, Command::SelectNav, launcher);
        s.command(0, Command::NextSelection, launcher);
        s.command(0, Command::NextSelection, launcher);
        assert_eq!(s.own().selected, 1);
        assert_eq!(s.own().delivery_mode, mode);
    }
    // Without a ground target a new bomb starts in CCIP.
    s.own_mut().sensors.clear_selection();
    s.command(0, Command::SelectNav, level);
    s.command(0, Command::NextSelection, level);
    s.command(0, Command::NextSelection, level);
    assert_eq!(s.own().delivery_mode, DeliveryMode::Ccip);
}

/// A Heavy's bombs default to LVL whatever the attitude, with or without a
/// target (retail heavies have no CCIP); the other classes keep the H1
/// rule; Ctrl+M still switches a Heavy to CCIP (docs/spec/ai-strike.md,
/// "Bombing class").
#[test]
fn a_heavy_selects_lvl_whatever_its_attitude_and_every_class_keeps_both_modes() {
    let level = pass(5_000., 0., 450., 0.);
    let dive = pass(5_000., 0., 450., -30.);
    let select = |s: &mut State, launcher: Launcher| {
        s.command(0, Command::SelectNav, launcher);
        s.command(0, Command::NextSelection, launcher);
        s.command(0, Command::NextSelection, launcher);
        assert_eq!(s.own().selected, 1);
        s.own().delivery_mode
    };
    // A Heavy starts in LVL, with no ground target and in a dive.
    let mut heavy = carrying_as(mk82(), 4, StrikeClass::Heavy);
    assert_eq!(heavy.own().delivery_mode, DeliveryMode::Level);
    assert_eq!(select(&mut heavy, dive), DeliveryMode::Level);
    assert_eq!(select(&mut heavy, level), DeliveryMode::Level);
    designate_ground(&mut heavy, 0., 20_000.);
    assert_eq!(select(&mut heavy, dive), DeliveryMode::Level);
    // Both modes stay: Ctrl+M takes it to CCIP and back.
    heavy.command(0, Command::ToggleSeekerMode, level);
    assert_eq!(heavy.own().delivery_mode, DeliveryMode::Ccip);
    heavy.command(0, Command::ToggleSeekerMode, level);
    assert_eq!(heavy.own().delivery_mode, DeliveryMode::Level);
    // Every other class keeps the situational rule.
    for class in [
        StrikeClass::Fighter,
        StrikeClass::Attack,
        StrikeClass::Helicopter,
        StrikeClass::Gunship,
    ] {
        let mut s = carrying_as(mk82(), 4, class);
        assert_eq!(s.own().delivery_mode, DeliveryMode::Ccip, "{class:?}");
        assert_eq!(select(&mut s, level), DeliveryMode::Ccip, "{class:?}");
        designate_ground(&mut s, 0., 20_000.);
        assert_eq!(select(&mut s, level), DeliveryMode::Level, "{class:?}");
        assert_eq!(select(&mut s, dive), DeliveryMode::Ccip, "{class:?}");
    }
}

/// Flies the level pass with the trigger as `held` says for each second of
/// it, and returns each release's step with the solution of that step.
fn level_pass(s: &mut State, held: impl Fn(f64) -> bool, seconds: f64) -> Vec<(f64, f64, f64)> {
    let speed = 450. * KNOT;
    let mut releases = Vec::new();
    for tick in 0..(seconds * 120.) as u64 {
        let t = tick as f64 / 120.;
        let before = bombs(s);
        step(s, held(t), pass(5_000., speed * t, 450., 0.));
        if bombs(s) > before {
            let level = s.own().delivery.as_ref().unwrap().level.unwrap();
            releases.push((t, level.along_ft, level.cross_ft));
        }
    }
    releases
}

#[test]
fn a_held_trigger_releases_once_when_the_marker_meets_the_crossbar() {
    let mut s = carrying(mk82(), 4);
    designate_ground(&mut s, 0., 20_000.);
    s.command(0, Command::ToggleSeekerMode, pass(5_000., 0., 450., 0.));
    // Pickle and hold from far out, and on past the target.
    let releases = level_pass(&mut s, |_| true, 20.);
    assert_eq!(releases.len(), 1, "{releases:?}");
    let (t, along, cross) = releases[0];
    let tick = 450. * KNOT / 120.;
    assert!(
        along <= 0. && along > -RELEASE_WINDOW_TICKS * tick,
        "{along}"
    );
    assert!(cross.abs() < 1.);
    // The release came where the throw said: the mean impact on the target.
    let throw = 759. * (5_000_f64 / 16.).sqrt();
    assert!(
        ((20_000. - throw) / (450. * KNOT) - t).abs() < 0.05,
        "t {t}"
    );
    // Past it: OVERFLY and the trigger is a plain press again.
    let d = s.own().delivery.clone().unwrap();
    assert!(d.warnings.contains(&Warning::Overfly));
    assert_eq!(d.level_trigger(1), None);
    assert_eq!(s.own().rounds(1), 3);
}

#[test]
fn a_press_before_the_cue_waits_and_a_miss_outside_the_radius_never_releases() {
    // Pressed and let go well before the cue: nothing falls.
    let mut s = carrying(mk82(), 4);
    designate_ground(&mut s, 0., 20_000.);
    s.command(0, Command::ToggleSeekerMode, pass(5_000., 0., 450., 0.));
    let releases = level_pass(&mut s, |t| t < 0.5, 12.);
    assert!(releases.is_empty(), "{releases:?}");
    // Held all the way with the target 600 ft off the track (radius 500).
    let mut s = carrying(mk82(), 4);
    designate_ground(&mut s, 600., 20_000.);
    s.command(0, Command::ToggleSeekerMode, pass(5_000., 0., 450., 0.));
    let releases = level_pass(&mut s, |_| true, 12.);
    assert!(releases.is_empty(), "{releases:?}");
    // Overflown, a new press releases at once.
    let speed = 450. * KNOT;
    step(&mut s, false, pass(5_000., speed * 12., 450., 0.));
    step(&mut s, true, pass(5_000., speed * 12.1, 450., 0.));
    assert_eq!(bombs(&s), 1);
}

#[test]
fn ccip_releases_on_the_press_as_before() {
    let mut s = carrying(mk82(), 4);
    designate_ground(&mut s, 0., 20_000.);
    step(&mut s, true, pass(5_000., 0., 450., 0.));
    assert_eq!(bombs(&s), 1);
    assert_eq!(s.own().delivery_mode, DeliveryMode::Ccip);
}

#[test]
fn the_rocket_pipper_is_where_the_flown_rocket_lands() {
    for (altitude, knots, dive) in [
        (1_500., 400., -10.),
        (3_000., 450., -25.),
        (800., 350., -5.),
    ] {
        let mut s = carrying(lau61(), 1);
        let launcher = pass(altitude, 0., knots, dive);
        // Leave the clock at an odd service phase, the aircraft where it fires.
        for _ in 0..7 {
            step(&mut s, false, launcher);
        }
        step(&mut s, true, launcher);
        let d = s.own().delivery.clone().unwrap();
        assert!(d.rocket);
        let c = d.ccip.expect("the rocket comes down");
        let id = s.projectiles.iter().find(|p| p.station == 1).unwrap().id;
        let mut burst = None;
        for _ in 0..(30 * 120) {
            step(&mut s, false, away());
            if !s.projectiles.iter().any(|p| p.id == id) {
                burst = s
                    .effects
                    .iter()
                    .find(|e| matches!(e.kind, EffectKind::Ground))
                    .map(|e| e.position);
                break;
            }
        }
        let burst = burst.expect("the rocket burst on the ground");
        let miss = (0..3)
            .map(|i| (burst[i] - c.impact[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!(
            miss < 0.01,
            "{altitude} ft {dive} deg: missed the pipper by {miss} ft"
        );
    }
}

#[test]
fn imp_counts_down_the_last_bombs_fall() {
    let mut s = carrying(mk82(), 4);
    let launcher = pass(5_000., 0., 450., 0.);
    step(&mut s, true, launcher);
    step(&mut s, false, launcher);
    let first = s.own().delivery.as_ref().unwrap().falling.unwrap();
    // Its own path, scatter and all, solved when it had flown one tick.
    let fall = s
        .bomb_motion(s.projectiles[0].id)
        .unwrap()
        .fall_time(0.)
        .unwrap();
    assert!((first - (fall - 1. / 120.)).abs() < 0.02, "{first}");
    for _ in 0..120 {
        step(&mut s, false, launcher);
    }
    let later = s.own().delivery.as_ref().unwrap().falling.unwrap();
    assert!((first - later - 1.).abs() < 0.02, "{first} {later}");
}

/// Drops one Mk 82 (five seconds to arm, 2,000 ft splash at 100 percent) from a
/// level pass at `altitude` over a ground target placed where it lands, and
/// returns the target's hit points, the craters and the ground effects' blasts.
fn drop_on_target(altitude: f64) -> (i32, usize, Vec<Option<u8>>) {
    let mut w = mk82();
    w.damage.fuze_arm_t = 20;
    w.damage.fuze_radius = 0;
    w.damage.collateral_radius = 2_000;
    w.damage.collateral_percent = 100;
    w.effects.land_explosion = 21;
    w.effects.crater_size = 3;
    let mut s = carrying(w, 4);
    let launcher = pass(altitude, 0., 450., 0.);
    let release = ReleaseState::from_station(&launcher, &s.own().configuration().stations[1], 0);
    let impact = predict_impact(release, |_, _| 0.).unwrap().position;
    // 300 ft beside the mean impact: inside the splash wherever the scatter
    // takes the bomb.
    designate_ground(&mut s, impact[0] + 300., impact[2]);
    step(&mut s, true, launcher);
    assert_eq!(bombs(&s), 1);
    let mut blasts = Vec::new();
    for _ in 0..(10 * 120) {
        step(&mut s, false, away());
        blasts.extend(
            s.effects
                .iter()
                .filter(|e| matches!(e.kind, EffectKind::Ground) && e.ticks > 0)
                .map(|e| e.blast),
        );
        if s.projectiles.is_empty() {
            break;
        }
    }
    let hp = s.targets.iter().find(|t| t.id == TARGET).unwrap().hp;
    let craters = s
        .marks
        .iter()
        .filter(|m| matches!(m.kind, crate::combat::blast::MarkKind::Crater(_)))
        .count();
    (hp, craters, blasts)
}

#[test]
fn a_bomb_that_lands_before_its_fuze_arms_is_a_dud() {
    // From 50 ft it falls 1.8 s (3.4 s at most with the scatter lofting it),
    // short of the 5 s arm time: LOW, and on the ground a puff of dirt with
    // no burst, splash or crater.
    let low = solve_low(50.);
    assert!(low);
    let (hp, craters, blasts) = drop_on_target(50.);
    assert_eq!(hp, 1_000, "a dud hurt the target");
    assert_eq!(craters, 0);
    assert!(!blasts.is_empty());
    // The bullet-on-land dust burst, after its variety roll (types 15, 16).
    let puff = |b: &Option<u8>| matches!(b, Some(15 | 16));
    assert!(blasts.iter().all(puff), "{blasts:?}");
    // From 800 ft (7 s, 5.9 s at least) it is armed: the burst, its splash
    // and a crater.
    assert!(!solve_low(800.));
    let (hp, craters, blasts) = drop_on_target(800.);
    assert!(
        hp < 1_000,
        "the armed burst missed: hp {hp} craters {craters} {blasts:?}"
    );
    assert_eq!(craters, 1);
    assert!(!blasts.iter().any(puff), "{blasts:?}");
}

/// Whether the sight warns LOW for the five-second Mk 82 at `altitude`.
fn solve_low(altitude: f64) -> bool {
    let mut w = mk82();
    w.damage.fuze_arm_t = 20;
    let mut s = carrying(w, 4);
    step(&mut s, false, pass(altitude, 0., 450., 0.));
    s.own()
        .delivery
        .as_ref()
        .unwrap()
        .warnings
        .contains(&Warning::Low)
}
