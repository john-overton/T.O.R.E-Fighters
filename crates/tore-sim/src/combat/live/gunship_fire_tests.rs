//! The AI gunship's gun train and release (docs/spec/ai-strike.md,
//! "AC-130"): the mount arcs, the range, and a round fired along the train
//! landing on its aim. Synthetic records only.
use super::*;
use crate::attitude::Basis;
use std::f64::consts::FRAC_PI_2;

/// A synthetic gun record in gun slot `slot`, through the TORE AC-130
/// record (muzzle speed and cadence), reaching 13,000 ft.
fn gun(slot: usize) -> Weapon {
    let s = super::super::tests::fixture(false);
    let mut w = s.own().configuration().stations[0].weapon.clone();
    w.source = gunship::GUNS[slot].into();
    w.flags = 0x880;
    w.seeker.signature = 0;
    w.seeker.zones[1].minimum_range = 0;
    w.seeker.zones[1].maximum_range = 13_000;
    w.movement.remove_t = 40;
    w.movement.deceleration = 7;
    gunship::apply_tore_record(&mut w);
    w
}

/// An AC-130 flying heading `heading_deg` level at `height` ft, at `speed`
/// ft/s.
fn launcher(height: f64, heading_deg: f64, speed: f64) -> Launcher {
    let yaw = heading_deg.to_radians();
    let basis = Basis::new(yaw, 0., 0.);
    Launcher {
        radar_power: false,
        position: [0., height, 0.],
        basis,
        speed_fps: speed,
        velocity: basis.forward.map(|v| v * speed),
        bay_ready: false,
        radar: false,
        jammer: false,
        alive: true,
        body_present: true,
        controls: Default::default(),
    }
}

fn still(position: Vector) -> TargetObservation {
    TargetObservation {
        position,
        velocity: [0.; 3],
    }
}

fn flat(_: f64, _: f64) -> f64 {
    0.
}

/// The orbit's geometry: the target 8,520 ft abeam left, 5,965 ft below
/// (35 degrees down, 10,400 ft slant).
const ABEAM_LEFT: Vector = [-8_520., 0., 0.];

#[test]
fn every_gun_bears_on_a_target_abeam_left_from_the_orbit() {
    let l = launcher(5_965., 0., 330.);
    for slot in 0..3 {
        let train = ai_gun_train(slot, &gun(slot), &l, still(ABEAM_LEFT), 0, &flat)
            .unwrap_or_else(|why| panic!("slot {slot}: {why:?}"));
        // Abeam left, a little aft for the aircraft's own motion.
        assert!(
            (train.heading + FRAC_PI_2).abs() < 8_f64.to_radians(),
            "slot {slot} heading {}",
            train.heading.to_degrees()
        );
        // 35 degrees down, raised by the round's drop (the ballistics).
        let elevation = train.elevation.to_degrees();
        assert!(
            (-35.5..-30.).contains(&elevation),
            "slot {slot} elevation {elevation}"
        );
        assert!(
            (10_000. ..10_800.).contains(&train.range_ft),
            "slot {slot} range {}",
            train.range_ft
        );
    }
}

#[test]
fn the_guns_never_fire_outside_their_mount_arcs() {
    let l = launcher(5_965., 0., 0.);
    // On the right: no gun bears.
    for slot in 0..3 {
        assert_eq!(
            ai_gun_train(slot, &gun(slot), &l, still([8_520., 0., 0.]), 0, &flat),
            Err(NoShot::Arc)
        );
    }
    // 40 degrees ahead of abeam: the 25 mm (60) and the 40 mm (45) bear,
    // the 105 mm (25) does not.
    let ahead = 40_f64.to_radians();
    let at = [-8_520. * ahead.cos(), 0., 8_520. * ahead.sin()];
    assert!(ai_gun_train(0, &gun(0), &l, still(at), 0, &flat).is_ok());
    assert!(ai_gun_train(1, &gun(1), &l, still(at), 0, &flat).is_ok());
    assert_eq!(
        ai_gun_train(2, &gun(2), &l, still(at), 0, &flat),
        Err(NoShot::Arc)
    );
    // Steeply down: past the 105 mm's 45 degrees.
    assert_eq!(
        ai_gun_train(2, &gun(2), &l, still([-1_500., 0., 0.]), 0, &flat),
        Err(NoShot::Arc)
    );
    // Beyond the 13,000 ft reach.
    assert_eq!(
        ai_gun_train(0, &gun(0), &l, still([-16_000., 0., 0.]), 0, &flat),
        Err(NoShot::Range)
    );
}

#[test]
fn a_ridge_between_the_gun_and_its_aim_holds_it() {
    let l = launcher(5_965., 0., 0.);
    let ridge = |x: f64, _: f64| {
        if (-6_000. ..-5_000.).contains(&x) {
            4_000.
        } else {
            0.
        }
    };
    assert_eq!(
        ai_gun_train(0, &gun(0), &l, still(ABEAM_LEFT), 0, &ridge),
        Err(NoShot::Terrain)
    );
}

#[test]
fn the_cadence_keeps_each_guns_rate_and_only_the_fast_gun_bursts() {
    let due = |slot: usize| (0..1_440).filter(|t| ai_gun_due(&gun(slot), *t)).count();
    // 1,800, 100 and 10 rounds a minute over 12 s.
    assert_eq!(due(0), 360);
    assert_eq!(due(1), 20);
    assert_eq!(due(2), 2);
    assert!(!ai_gun_slow(&gun(0)));
    assert!(ai_gun_slow(&gun(1)) && ai_gun_slow(&gun(2)));
}

/// A round fired along the train from a moving gunship lands on its aim,
/// within its own dispersion:
/// the lead and drop of the gun's own ballistics, which retail's AI aim (20
/// ft high, no drop) lacks.
#[test]
fn a_round_fired_along_the_train_lands_on_its_aim() {
    for slot in 0..3 {
        let mut s = super::super::tests::fixture(false);
        let mut config = s.own().configuration().clone();
        let template = config.stations[0].clone();
        config.stations = (0..3)
            .map(|i| {
                let mut station = template.clone();
                station.weapon = gun(i);
                station.mount = gunship::pivot(i);
                station
            })
            .collect();
        // Far from every row of the fixture.
        let mut l = launcher(5_965., 0., 330.);
        l.position = [200_000., 5_965., 200_000.];
        let aim = [l.position[0] - 8_520., 0., l.position[2]];
        let train = ai_gun_train(
            slot,
            &config.stations[slot].weapon,
            &l,
            still(aim),
            s.tick,
            &flat,
        )
        .expect("bears");
        let released = s.release_ai_gun_round(
            AiGunRound {
                id: 9_000,
                aircraft: 77,
                config: &config,
                station: slot,
                slot,
                launcher: l,
                train,
                ordinal: 0,
            },
            &flat,
        );
        assert!(released);
        // The round leaves inside the gun's dispersion cone (drawn from its
        // number on its first step), as a human's does: the pipper (and this
        // aim) is the cone's centre.
        let centre = gunship::direction(l, train.heading, train.elevation);
        let spread = crate::attitude::dot(
            super::super::projectile_launch_direction(
                &config.stations[slot].weapon,
                centre,
                9_000,
                77,
                slot,
            ),
            centre,
        )
        .clamp(-1., 1.)
        .acos();
        s.effects.clear();
        for _ in 0..2_400 {
            if !s.projectiles.iter().any(|p| p.id == 9_000) {
                break;
            }
            s.step(&[], flat);
        }
        // Where it burst on the ground.
        let landed = s
            .effects
            .iter()
            .rfind(|e| e.kind == super::super::EffectKind::Ground)
            .expect("a ground burst")
            .position;
        let miss = (landed[0] - aim[0]).hypot(landed[2] - aim[2]);
        let cone = 1.2 * train.range_ft * spread;
        assert!(
            miss < 20. + cone,
            "slot {slot} missed by {miss:.0} ft (the cone allows {cone:.0})"
        );
    }
}
