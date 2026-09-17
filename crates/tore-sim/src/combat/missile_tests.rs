use super::tests::{fixture, target};
use super::*;
use missiles::{DWELL, MEMORY, Profile};
fn weapon(name: &str) -> Weapon {
    let mut w = fixture(true).configuration().stations[0].weapon.clone();
    w.source = name.into();
    w.seeker.signature = match name {
        "AGM45.JT" | "AGM88.JT" => 4,
        "AIM9M.JT" | "AGM65G.JT" => 2,
        _ => 3,
    };
    w
}
#[test]
fn heat_aspect_power_masking_and_dwell() {
    let w = weapon("AIM9M.JT");
    let profile = Profile::for_weapon(&w).unwrap();
    let mut t = target(7, [0., 1000., 5000.], 20, 0x80);
    let mut seeker = seeker::Seeker::default();
    let view = seeker::View {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        cap: Some(profile.search_cap()),
        obscured: &|_, _| false,
    };
    let o = seeker::observe(&w, profile, &view, &t).unwrap();
    assert!((o.quality - 0.8).abs() < 1e-12);
    for _ in 0..DWELL - 1 {
        seeker.step(profile, &[o]);
    }
    assert_eq!(seeker.status, Status::Acquiring);
    seeker.step(profile, &[o]);
    assert_eq!(seeker.status, Status::Locked);
    assert_eq!(seeker.target, Some(7));
    t.basis = Basis::new(std::f64::consts::PI, 0., 0.);
    let head = seeker::observe(&w, profile, &view, &t).unwrap();
    assert!((head.quality - 0.2).abs() < 1e-12);
    let mut fresh = seeker::Seeker::default();
    for _ in 0..DWELL {
        fresh.step(profile, &[head]);
    }
    assert_eq!(fresh.target, None);
    t.heat = Heat::Engine {
        on: true,
        throttle: 1.,
        afterburner: true,
    };
    let hot = seeker::observe(&w, profile, &view, &t).unwrap();
    assert!((hot.quality - 0.3).abs() < 1e-12);
    for _ in 0..DWELL {
        fresh.step(profile, &[hot]);
    }
    assert_eq!(fresh.target, Some(7));
    let masked = seeker::View {
        obscured: &|_, _| true,
        ..view
    };
    assert!(seeker::observe(&w, profile, &masked, &t).is_none());
}
#[test]
fn identity_and_reacquisition_survive_memory_timeout() {
    let profile = Profile::for_weapon(&weapon("AIM9M.JT")).unwrap();
    let o = seeker::Observation {
        id: 1,
        position: [0., 0., 1000.],
        velocity: [0.; 3],
        quality: 0.8,
        off_axis: 0.,
        range: 1000.,
    };
    let mut s = seeker::Seeker::default();
    for _ in 0..DWELL {
        s.step(profile, &[o]);
    }
    for _ in 0..MEMORY + 60 {
        s.step(profile, &[]);
    }
    assert_eq!(s.status, Status::Lost);
    assert_eq!(s.target, Some(1));
    let other = seeker::Observation {
        id: 2,
        quality: 1.,
        ..o
    };
    for _ in 0..DWELL {
        s.step(profile, &[other]);
    }
    assert_eq!(s.status, Status::Lost);
    for _ in 0..DWELL - 1 {
        s.step(profile, &[other, o]);
    }
    assert_eq!(s.status, Status::Lost);
    s.step(profile, &[other, o]);
    assert_eq!(s.status, Status::Locked);
    assert_eq!(s.target, Some(1));
}
#[test]
fn emitter_shutdown_has_no_heat_or_reflection_fallback() {
    let w = weapon("AGM88.JT");
    let mut profile = Profile::for_weapon(&w).unwrap();
    let mut t = target(1, [0., 1000., 1000.], 20, 0x80);
    let view = seeker::View {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        cap: None,
        obscured: &|_, _| false,
    };
    t.signature.radar = 10000.;
    t.signature.infrared = 10000.;
    assert!(seeker::observe(&w, profile, &view, &t).is_none());
    t.radar_emitting = true;
    assert!(seeker::observe(&w, profile, &view, &t).is_some());
    t.radar_emitting = false;
    t.jammer_active = true;
    t.jammer = Some(sensors::JammerProfile {
        record: "TEST".into(),
        generation: sensors::Generation::LateColdWar,
        strength: 1.,
        band: 0,
        radio_frequency: true,
    });
    assert!(seeker::observe(&w, profile, &view, &t).is_none());
    profile.jammer_emissions = true;
    assert!(seeker::observe(&w, profile, &view, &t).is_some());
    t.jammer_active = false;
    assert!(seeker::observe(&w, profile, &view, &t).is_none());
}
#[test]
fn radar_seeker_uses_shared_aspect_signature_range() {
    let w = weapon("AIM120.JT");
    let profile = Profile::for_weapon(&w).unwrap();
    let mut t = target(1, [0., 1000., 9000.], 20, 0x80);
    t.signature.radar = 50.;
    let view = seeker::View {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        cap: None,
        obscured: &|_, _| false,
    };
    assert!(seeker::observe(&w, profile, &view, &t).is_none());
    t.basis = Basis::new(std::f64::consts::FRAC_PI_2, 0., 0.);
    assert!(seeker::observe(&w, profile, &view, &t).is_some());
}

fn shot(w: &Weapon, mode: LaunchMode, target: Option<u32>) -> Projectile {
    let profile = Profile::for_weapon(w).unwrap();
    Projectile {
        id: 0,
        guidance: Some(Flight::new(profile, mode, target)),
        motion: Some(Motion::new(&w.movement, [0., 0., 600.], 1000.)),
        guidance_ticks: Some(profile.guidance_ticks),
        age: 0,
        incoming: false,
        station: 0,
        position: [0., 1000., 0.],
        previous: [0., 1000., 0.],
        direction: [0., 0., 1.],
        speed_f8: 600 * 256,
        launched_t: 0,
        target,
        fall: FallState::default(),
    }
}
#[test]
fn all_nine_activation_thresholds_and_no_false_pitbull() {
    let sensors = fixture(true).sensors;
    for (name, nmi) in [
        ("AIM120.JT", 5.),
        ("MICA.JT", 5.),
        ("AA12.JT", 5.),
        ("AAML.JT", 8.),
        ("AIM54C.JT", 10.),
        ("AEMP1.JT", 3.),
        ("AGM84A.JT", 8.),
        ("AM39.JT", 8.),
        ("AS16.JT", 2.),
    ] {
        let w = weapon(name);
        for delta in [-0.01, 0., 0.01] {
            let mut p = shot(&w, LaunchMode::Cued, Some(1));
            p.guidance.as_mut().unwrap().last_intercept = Some([0., 1000., nmi * 6076. + delta]);
            guide(&mut p, &w, &[], &sensors, &|_, _| false);
            let f = p.guidance.unwrap();
            assert_eq!(f.enabled, delta <= 0., "{name} {delta}");
            assert_eq!(
                f.seeker.status,
                if delta <= 0. {
                    Status::Search
                } else {
                    Status::Midcourse
                }
            );
        }
        let mut uncued = shot(&w, LaunchMode::Boresight, None);
        guide(&mut uncued, &w, &[], &sensors, &|_, _| false);
        assert!(uncued.guidance.as_ref().unwrap().enabled);
        assert_eq!(uncued.guidance.as_ref().unwrap().last_intercept, None);
    }
    for name in ["R530.JT", "AIM9M.JT", "AGM45.JT"] {
        assert_eq!(
            Profile::for_weapon(&weapon(name)).unwrap().activation_ft,
            None
        );
    }
    for name in [
        "AS14.JT", "AS30.JT", "AT12.JT", "AT2.JT", "ASROC.JT", "SA19.JT", "SAN11.JT",
    ] {
        assert!(Profile::for_weapon(&weapon(name)).is_none());
    }
}
#[test]
fn hidden_movement_never_updates_intercept_and_expiry_precedes_acquisition() {
    let sensors = fixture(true).sensors;
    let w = weapon("AIM120.JT");
    let mut p = shot(&w, LaunchMode::Cued, Some(1));
    let intercept = [0., 1000., 40000.];
    p.guidance.as_mut().unwrap().last_intercept = Some(intercept);
    let mut t = target(1, [0., 1000., 3000.], 20, 0x80);
    for i in 0..300 {
        t.position[0] = f64::from(i) * 100.;
        guide(&mut p, &w, &[t.clone()], &sensors, &|_, _| true);
        p.age += 1;
    }
    assert_eq!(p.guidance.as_ref().unwrap().last_intercept, Some(intercept));
    assert!(!p.guidance.as_ref().unwrap().enabled);
    p.guidance_ticks = Some(p.age);
    p.guidance.as_mut().unwrap().last_intercept = Some(p.position);
    guide(&mut p, &w, &[t], &sensors, &|_, _| false);
    assert_eq!(p.guidance.as_ref().unwrap().seeker.status, Status::Expired);
    assert!(!p.guidance.as_ref().unwrap().enabled);
    assert!(!missiles::removed(&w.movement, p.age));
}
#[test]
fn boresight_live_release_without_cockpit_sensors_and_next_round_reset() {
    let mut s = fixture(true);
    s.config.stations[0].weapon = weapon("AIM9M.JT");
    s.launch_mode = LaunchMode::Boresight;
    let l = Launcher {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 600.,
        velocity: [40., 60., 600.],
        bay_ready: true,
        radar: false,
        jammer: false,
        alive: true,
        controls: Default::default(),
    };
    s.config.sensors.radar = None;
    s.config.sensors.infrared = None;
    assert_eq!(s.readiness(l), Readiness::Ready);
    assert!(s.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
    assert_eq!(s.projectiles[0].target, None);
    assert_eq!(
        s.projectiles[0].guidance.as_ref().unwrap().mode,
        LaunchMode::Boresight
    );
    assert_eq!(s.projectiles[0].motion.as_ref().unwrap().velocity[0], 40.);
    s.release();
    let pos = s.projectiles[0].position;
    s.targets
        .push(target(77, [pos[0], pos[1], pos[2] + 2500.], 20, 0x80));
    for _ in 0..DWELL + 1 {
        s.step(false, l, |_, _| 0.);
    }
    assert_eq!(s.projectiles[0].target, Some(77));
    assert_eq!(s.designated(), None);
    assert!(s.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
    assert!(!s.mounted.acquired);
    assert_eq!(s.mounted.dwell, 0);
}
#[test]
fn two_active_shots_own_targets_and_reacquire_without_support() {
    let sensors = fixture(true).sensors;
    let w = weapon("AIM120.JT");
    let mut a = shot(&w, LaunchMode::Cued, Some(1));
    let mut b = shot(&w, LaunchMode::Cued, Some(2));
    let targets = [
        target(1, [0., 1000., 3000.], 20, 0x80),
        target(2, [10., 1000., 4000.], 20, 0x80),
    ];
    a.guidance.as_mut().unwrap().last_intercept = Some(targets[0].position);
    b.guidance.as_mut().unwrap().last_intercept = Some(targets[1].position);
    for _ in 0..DWELL {
        guide(&mut a, &w, &targets, &sensors, &|_, _| false);
        guide(&mut b, &w, &targets, &sensors, &|_, _| false);
    }
    assert_eq!(a.target, Some(1));
    assert_eq!(b.target, Some(2));
    assert_eq!(a.guidance.as_ref().unwrap().seeker.status, Status::Pitbull);
    let known = a.guidance.as_ref().unwrap().last_intercept;
    for _ in 0..MEMORY + 1 {
        guide(&mut a, &w, &targets, &sensors, &|_, _| true);
    }
    assert_eq!(a.guidance.as_ref().unwrap().last_intercept, known);
    assert_eq!(a.guidance.as_ref().unwrap().seeker.status, Status::Lost);
    for _ in 0..DWELL {
        guide(&mut a, &w, &targets, &sensors, &|_, _| false);
    }
    assert_eq!(a.guidance.as_ref().unwrap().seeker.status, Status::Pitbull);
}

#[test]
fn bay_safe_empty_and_failed_gates_survive_uncued_mode() {
    let mut s = fixture(true);
    s.config.stations[0].weapon = weapon("AIM9M.JT");
    s.launch_mode = LaunchMode::Boresight;
    let mut l = Launcher {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 600.,
        velocity: [0., 0., 600.],
        radar: false,
        jammer: false,
        alive: true,
        bay_ready: false,
        controls: Default::default(),
    };
    assert_eq!(s.readiness(l), Readiness::BayClosed);
    let ammo = s.ammo.clone();
    s.step(true, l, |_, _| 0.);
    assert_eq!(s.ammo, ammo);
    l.bay_ready = true;
    s.armed = false;
    assert_eq!(s.readiness(l), Readiness::Safe);
    assert!(s.seeker_tone(l).is_none());
    s.armed = true;
    s.ammo[0] |= 0x8000;
    assert_eq!(s.readiness(l), Readiness::StationFailed);
    assert!(s.seeker_tone(l).is_none());
    s.ammo[0] = 0;
    assert_eq!(s.readiness(l), Readiness::Empty);
    assert!(s.seeker_tone(l).is_none());
}
