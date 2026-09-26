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
    t.role = TargetRole::Surface;
    t.airborne = false;
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
        owner: crate::combat::live::PLAYER_OWNER,
        weapon: None,
        id: 0,
        guidance: Some(Flight::new(profile, mode, target, [0., 1000., 0.])),
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
        gun_round: None,
        tracer: false,
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
        assert!(!Profile::reviewed(name));
    }
    // The name-only answer agrees with the full profile.
    for name in ["AIM120.JT", "R530.JT", "AIM9M.JT", "AGM65G.JT", "AGM88.JT"] {
        assert!(Profile::for_weapon(&weapon(name)).is_some());
        assert!(Profile::reviewed(name));
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
fn boresight_live_release_without_sensor_equipment_and_next_round_reset() {
    let mut s = fixture(true);
    s.config.stations[0].weapon = weapon("AIM9M.JT");
    s.launch_mode = LaunchMode::Boresight;
    let l = Launcher {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 600.,
        velocity: [40., 60., 600.],
        bay_ready: true,
        radar_power: true,
        radar: true,
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
        radar_power: false,
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

#[test]
fn mounted_lock_is_not_boresight_release_permission() {
    let mut s = fixture(true);
    s.config.stations[0].weapon = weapon("AIM9M.JT");
    s.launch_mode = LaunchMode::Boresight;
    let l = Launcher {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 600.,
        velocity: [0., 0., 600.],
        bay_ready: true,
        radar_power: false,
        radar: false,
        jammer: false,
        alive: true,
        controls: Default::default(),
    };
    assert_eq!(s.readiness(l), Readiness::Ready);
    assert!(!s.can_lock(l));
    s.command(Command::CompatibilityWeapons, l);
    assert_eq!(s.launch_mode, LaunchMode::Cued);
    s.command(Command::ToggleSeekerMode, l);
    assert_eq!(s.launch_mode, LaunchMode::Cued);
    assert_eq!(s.readiness(l), Readiness::NoTarget);
}
#[test]
fn render_cadence_and_pause_do_not_change_missile_state() {
    let run = |fps: u32| {
        let mut s = fixture(true);
        s.config.stations[0].weapon = weapon("AIM120.JT");
        s.launch_mode = LaunchMode::Boresight;
        let l = Launcher {
            position: [0., 1000., 0.],
            basis: Basis::new(0., 0., 0.),
            speed_fps: 600.,
            velocity: [0., 0., 600.],
            bay_ready: true,
            radar_power: false,
            radar: false,
            jammer: false,
            alive: true,
            controls: Default::default(),
        };
        s.range_target(l);
        let mut remainder = 0;
        let mut tick = 0;
        let mut events = Vec::new();
        for frame in 0..fps * 4 {
            // One second of pause, with no simulation or dwell updates.
            if (fps..fps * 2).contains(&frame) {
                continue;
            }
            remainder += 120;
            while remainder >= fps {
                remainder -= fps;
                if tick == 120 {
                    s.command(Command::ToggleSeekerMode, l);
                }
                events.extend(s.step(tick == 60, l, |_, _| 0.));
                tick += 1;
            }
        }
        (s.projectiles, s.targets, s.mounted, events)
    };
    assert_eq!(run(30), run(60));
    assert_eq!(run(60), run(144));
}

#[test]
fn strongest_heat_ties_dwell_reset_and_exact_memory_boundary() {
    let profile = Profile::for_weapon(&weapon("AIM9M.JT")).unwrap();
    let o = seeker::Observation {
        id: 8,
        position: [0., 0., 1000.],
        velocity: [0.; 3],
        quality: 0.8,
        off_axis: 0.,
        range: 1000.,
    };
    let other = seeker::Observation { id: 7, ..o };
    let hot = seeker::Observation {
        id: 9,
        quality: 1.,
        ..o
    };
    let mut s = seeker::Seeker::default();
    for _ in 0..DWELL - 1 {
        s.step(profile, &[o, other]);
    }
    assert_eq!(s.candidate, Some(7));
    s.step(profile, &[o, other, hot]);
    assert_eq!(s.dwell, 1);
    assert!(!s.acquired);
    for _ in 1..DWELL {
        s.step(profile, &[hot]);
    }
    assert_eq!(s.target, Some(9));
    s.step(
        profile,
        &[seeker::Observation {
            quality: 0.20,
            ..hot
        }],
    );
    assert_eq!(s.status, Status::Locked);
    for _ in 0..MEMORY - 1 {
        s.step(profile, &[]);
    }
    assert_eq!(s.status, Status::Memory);
    s.step(profile, &[]);
    assert_eq!(s.status, Status::Lost);
}
#[test]
fn supported_update_freezes_on_radar_shutdown_and_cockpit_switch() {
    let mut s = fixture(true);
    let mut w = range_weapon();
    for z in &mut w.seeker.zones {
        z.maximum_range = 100000;
    }
    s.config.stations[0].weapon = w;
    let mut l = Launcher {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 600.,
        velocity: [0., 0., 600.],
        bay_ready: true,
        radar_power: true,
        radar: true,
        jammer: false,
        alive: true,
        controls: Default::default(),
    };
    s.range_target(l);
    s.targets[0].position = [0., 1000., 40000.];
    s.targets[0].velocity = [0., 0., 300.];
    s.step(false, l, |_, _| 0.);
    s.designate_next(true);
    for _ in 0..90 {
        s.step(false, l, |_, _| 0.);
    }
    assert!(s.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
    s.release();
    let known = s.projectiles[0]
        .guidance
        .as_ref()
        .unwrap()
        .last_intercept
        .unwrap();
    assert!(!s.projectiles[0].guidance.as_ref().unwrap().enabled);
    l.radar = false;
    s.targets[0].position = [100000., 1000., 90000.];
    s.command(Command::ClearDesignation, l);
    for _ in 0..60 {
        s.step(false, l, |_, _| 0.);
    }
    assert_eq!(
        s.projectiles[0].guidance.as_ref().unwrap().last_intercept,
        Some(known)
    );
    assert_eq!(s.projectiles[0].target, Some(s.targets[0].id));
}

#[test]
fn invalid_activation_profiles_fail_explicitly() {
    let profile = Profile::for_weapon(&weapon("AIM120.JT")).unwrap();
    assert!(profile.validate().is_ok());
    for value in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(
            Profile {
                activation_ft: Some(value),
                ..profile
            }
            .validate()
            .is_err()
        );
    }
    assert!(
        Profile {
            guidance: Guidance::Infrared,
            ..profile
        }
        .validate()
        .is_err()
    );
    assert!(
        Profile {
            guidance_ticks: 0,
            ..profile
        }
        .validate()
        .is_err()
    );
}

#[test]
fn a_radar_missile_goes_quiet_inside_minimum_range() {
    let l = Launcher {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 600.,
        velocity: [0., 0., 600.],
        bay_ready: true,
        radar_power: true,
        radar: true,
        jammer: false,
        alive: true,
        controls: Default::default(),
    };
    let tone_at = |distance: f64| {
        let mut s = fixture(true);
        s.config.stations[0].weapon = weapon("AIM120.JT");
        s.config.stations[0].weapon.seeker.zones[0].minimum_range = 0;
        s.config.stations[0].weapon.seeker.zones[1].minimum_range = 4000;
        s.targets.push(target(7, [0., 1000., distance], 20, 0x80));
        for _ in 0..DWELL + 1 {
            s.step(false, l, |_, _| 0.);
        }
        assert!(s.bore_observation.is_some());
        s.seeker_tone(l)
    };
    assert!(tone_at(3000.).is_none());
    assert!(tone_at(5000.).is_some());
}

#[test]
fn automatic_bore_release_and_radar_search_start_without_designation() {
    let l = Launcher {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 600.,
        velocity: [0., 0., 600.],
        bay_ready: true,
        radar_power: true,
        radar: true,
        jammer: false,
        alive: true,
        controls: Default::default(),
    };
    let mut s = fixture(true);
    s.config.stations[0].weapon = weapon("AIM120.JT");
    s.targets.push(target(7, [0., 1000., 5000.], 20, 0x80));
    for _ in 0..DWELL + 1 {
        s.step(false, l, |_, _| 0.);
    }
    assert_eq!(s.launch_mode, LaunchMode::Boresight);
    assert_eq!(s.mounted.target, None);
    assert_eq!(s.mounted.status, Status::Search);
    assert_eq!(s.bore_observation.unwrap().id, 7);
    assert_eq!(s.designated(), None);
    // The bore return sounds the radar lock tone (John, 2026-09-23).
    assert!(s.seeker_tone(l).is_some_and(|t| t.radar && t.locked));
    assert!(s.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
    assert!(s.projectiles[0].guidance.as_ref().unwrap().enabled);
    assert_eq!(s.projectiles[0].target, None);
    s.command(Command::DesignateTarget(7), l);
    assert_eq!(s.designated(), Some(7));
    assert_eq!(s.launch_mode, LaunchMode::Cued);
    // Silent until the seeker is actually tracking the new selection.
    assert!(s.seeker_tone(l).is_none());
    for _ in 0..crate::sensors::track::ACQUISITION_STEPS + DWELL + 1 {
        s.step(false, l, |_, _| 0.);
    }
    assert!(s.seeker_tone(l).is_some());
    s.command(Command::ClearDesignation, l);
    assert_eq!(s.designated(), None);
    assert_eq!(s.mounted.target, None);
    s.step(false, l, |_, _| 0.);
    assert_eq!(s.launch_mode, LaunchMode::Boresight);
    s.config.stations[0].weapon = weapon("AIM9M.JT");
    s.ammo[0] = 5;
    for _ in 0..DWELL + 1 {
        s.step(false, l, |_, _| 0.);
    }
    assert_eq!(s.mounted.target, Some(7));
    let mut hot = target(8, [400., 1000., 5000.], 20, 0x80);
    hot.signature.infrared = 400.;
    s.targets.push(hot);
    for _ in 0..DWELL {
        s.step(false, l, |_, _| 0.);
    }
    assert_eq!(s.mounted.target, Some(8));
    // This off-axis fixture acquires heat but cannot produce a launch/HUD solution.
    assert!(s.weapon_observation(l).is_none());
    assert!(s.seeker_tone(l).is_none());
    s.targets[1].position = [5000., 1000., 5000.];
    s.targets[0].position = [-5000., 1000., 5000.];
    s.step(false, l, |_, _| 0.);
    assert_eq!(s.mounted.status, Status::Search);
    assert_eq!(s.mounted.target, None);
    assert!(s.seeker_tone(l).is_none());
    // A supported weapon cannot gain an independent radar seeker.
    s.config.stations[0].weapon = weapon("R530.JT");
    s.config.stations[0].weapon.flags |= 0x200;
    s.launch_mode = LaunchMode::Cued;
    s.step(false, l, |_, _| 0.);
    assert_eq!(s.launch_mode, LaunchMode::Cued);
    assert_eq!(s.readiness(l), Readiness::NoTarget);
}

#[test]
fn bore_is_circular_and_prefers_signal_strength_over_centering() {
    let position = [0., 1000., 0.];
    let basis = Basis::new(0., 0., 0.);
    for name in ["AIM9M.JT", "AIM120.JT"] {
        let w = weapon(name);
        let profile = Profile::for_weapon(&w).unwrap();
        let view = seeker::View {
            position,
            basis,
            cap: Some(profile.search_cap()),
            obscured: &|_, _| false,
        };
        let near = target(1, [0., 1000., 5000.], 20, 0x80);
        let mut strong = target(2, [400., 1000., 5000.], 20, 0x80);
        strong.signature.infrared = 400.;
        strong.signature.radar = 400.;
        let a = seeker::observe(&w, profile, &view, &near).unwrap();
        let b = seeker::observe(&w, profile, &view, &strong).unwrap();
        assert!(b.quality > a.quality);
        let mut seeker = seeker::Seeker::default();
        for _ in 0..DWELL {
            seeker.step(profile, &[a, b]);
        }
        assert_eq!(seeker.target, Some(2));
        // Five degrees on each axis is outside a five-degree circular bore.
        let offset = 5000. * 5f64.to_radians().tan();
        let corner = target(3, [offset, 1000. + offset, 5000.], 20, 0x80);
        assert!(seeker::observe(&w, profile, &view, &corner).is_none());
        let edge = target(4, [5000. * 5f64.to_radians().tan(), 1000., 5000.], 20, 0x80);
        assert!(seeker::observe(&w, profile, &view, &edge).is_some());
    }
}

#[test]
fn bore_centre_weight_balances_alignment_and_signal() {
    let profile = Profile::for_weapon(&weapon("AIM120.JT")).unwrap();
    let centre = seeker::Observation {
        id: 1,
        position: [0., 0., 1000.],
        velocity: [0.; 3],
        quality: 1.,
        range: 1000.,
        off_axis: 0.,
    };
    let edge = seeker::Observation {
        id: 2,
        quality: 2.,
        off_axis: profile.search_cap(),
        ..centre
    };
    assert_eq!(seeker::centre_weight(0., profile.search_cap()), 1.);
    assert_eq!(
        seeker::centre_weight(profile.search_cap(), profile.search_cap()),
        0.25
    );
    assert!(seeker::compare_returns(&centre, &edge, profile).is_lt());
    let strong = seeker::Observation {
        quality: 5.,
        ..edge
    };
    assert!(seeker::compare_returns(&strong, &centre, profile).is_lt());
    let tie = seeker::Observation { id: 3, ..centre };
    assert!(seeker::compare_returns(&centre, &tie, profile).is_lt());
}

#[test]
fn estimated_hit_has_explicit_numbers_and_never_claims_certainty() {
    let w = weapon("AIM120.JT");
    let mut zone = w.seeker.zones[1];
    zone.minimum_range = 1000;
    zone.maximum_range = 9000;
    let o = seeker::Observation {
        id: 1,
        position: [0., 0., 5000.],
        velocity: [0.; 3],
        quality: 0.8,
        range: 5000.,
        off_axis: 0.,
    };
    let solution = Some(missiles::Solution {
        point: o.position,
        seconds: 10.,
    });
    let cap = 5f64.to_radians();
    let estimate =
        |o, solution| missiles::estimated_hit_percent(o, solution, &zone, 100., Some(cap));
    // round(95 * .8 * 1 * .8125 * .94) = 58.
    assert_eq!(estimate(o, solution), 58);
    assert_eq!(
        estimate(seeker::Observation { off_axis: cap, ..o }, solution),
        15
    );
    assert_eq!(estimate(o, None), 0);
    assert_eq!(
        estimate(seeker::Observation { range: 9001., ..o }, solution),
        0
    );
    assert_eq!(
        estimate(seeker::Observation { range: 999., ..o }, solution),
        0
    );
    assert_eq!(
        estimate(seeker::Observation { quality: 0., ..o }, solution),
        0
    );
    let best = seeker::Observation {
        quality: 4.,
        range: 1000.,
        ..o
    };
    assert_eq!(
        estimate(
            best,
            Some(missiles::Solution {
                point: o.position,
                seconds: 0.
            })
        ),
        95
    );
    assert!(
        estimate(
            o,
            Some(missiles::Solution {
                point: o.position,
                seconds: 90.
            })
        ) < 58
    );
}

fn range_launcher() -> Launcher {
    Launcher {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 600.,
        velocity: [0., 0., 600.],
        bay_ready: true,
        radar_power: true,
        radar: true,
        jammer: false,
        alive: true,
        controls: Default::default(),
    }
}

#[test]
fn minimum_range_inhibits_bore_release_at_the_inclusive_boundary() {
    for name in ["AIM120.JT", "AIM9M.JT"] {
        for distance in [999., 1000., 1001.] {
            let mut s = fixture(true);
            s.config.stations[0].weapon = weapon(name);
            s.config.stations[0].weapon.seeker.zones[1].minimum_range = 1000;
            s.targets.push(target(7, [0., 1000., distance], 200, 0x80));
            let l = range_launcher();
            for _ in 0..DWELL + 1 {
                s.step(false, l, |_, _| 0.);
            }
            assert!(s.bore_observation.is_some());
            let rounds = s.rounds(0);
            if distance < 1000. {
                assert_eq!(s.readiness(l), Readiness::MinimumRange, "{name}");
                s.step(true, l, |_, _| 0.);
                assert_eq!(s.rounds(0), rounds);
                assert!(s.projectiles.is_empty());
            } else {
                assert_eq!(s.readiness(l), Readiness::Ready, "{name}");
                s.step(true, l, |_, _| 0.);
                assert!(s.rounds(0) < rounds);
            }
        }
    }
}

#[test]
fn blind_shot_rejects_close_contacts_but_retains_valid_terminal_tracking() {
    for name in ["AIM120.JT", "AIM9M.JT"] {
        let mut w = weapon(name);
        w.seeker.zones[1].minimum_range = 1000;
        let mut p = shot(&w, LaunchMode::Boresight, None);
        let sensors = fixture(true).sensors;
        let mut t = target(7, [0., 1000., 999.], 200, 0x80);
        for _ in 0..DWELL + 1 {
            guide(&mut p, &w, &[t.clone()], &sensors, &|_, _| false);
        }
        assert_eq!(p.target, None);
        assert!(!p.guidance.as_ref().unwrap().eligible(&w, &t));
        t.position[2] = 1000.;
        for _ in 0..DWELL {
            guide(&mut p, &w, &[t.clone()], &sensors, &|_, _| false);
        }
        assert_eq!(p.target, Some(7));
        // A valid engagement remains valid when both missile and target close.
        p.position[2] = 700.;
        t.position[2] = 800.;
        guide(&mut p, &w, &[t.clone()], &sensors, &|_, _| false);
        let f = p.guidance.as_ref().unwrap();
        assert!(f.eligible(&w, &t));
        assert!(matches!(f.seeker.status, Status::Locked | Status::Pitbull));
    }
}

#[test]
fn blind_shot_cannot_damage_inside_minimum_range() {
    for name in ["AIM120.JT", "AIM9M.JT"] {
        let mut s = fixture(true);
        let mut w = weapon(name);
        w.seeker.zones[1].minimum_range = 1000;
        w.damage.fuze_arm_t = 0;
        let mut p = shot(&w, LaunchMode::Boresight, None);
        p.position[2] = 500.;
        p.previous = p.position;
        s.config.stations[0].weapon = w;
        s.targets.push(target(7, [0., 1000., 500.], 200, 0x80));
        s.projectiles.push(p);
        s.step(false, range_launcher(), |_, _| 0.);
        assert_eq!(s.targets[0].hp, 200, "{name}");
    }
}

#[test]
fn surface_weapons_reject_aircraft_and_maverick_uses_surface_contrast() {
    let view = seeker::View {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        cap: None,
        obscured: &|_, _| false,
    };
    for name in [
        "AGM65G.JT",
        "AGM45.JT",
        "AGM88.JT",
        "AGM84A.JT",
        "AM39.JT",
        "AS16.JT",
        "AS7.JT",
    ] {
        let w = weapon(name);
        let profile = Profile::for_weapon(&w).unwrap();
        assert!(!profile.supports_boresight());
        let mut t = target(7, [0., 1000., 1000.], 200, 0x200);
        t.radar_emitting = true;
        assert!(seeker::observe(&w, profile, &view, &t).is_none());
        t.airborne = false;
        assert!(seeker::observe(&w, profile, &view, &t).is_none());
        t.role = TargetRole::Surface;
        let first = seeker::observe(&w, profile, &view, &t).unwrap();
        if name == "AGM65G.JT" {
            t.basis = Basis::new(std::f64::consts::PI, 0., 0.);
            t.heat = Heat::Engine {
                on: false,
                throttle: 0.,
                afterburner: false,
            };
            assert_eq!(
                seeker::observe(&w, profile, &view, &t).unwrap().quality,
                first.quality
            );
        }
        let mut s = fixture(true);
        s.config.stations[0].weapon = w;
        s.targets.push(target(7, [0., 1000., 1000.], 200, 0x80));
        let l = range_launcher();
        for _ in 0..DWELL + 1 {
            s.step(false, l, |_, _| 0.);
        }
        s.command(Command::ToggleSeekerMode, l);
        assert_eq!(s.launch_mode, LaunchMode::Cued);
        s.command(Command::DesignateTarget(7), l);
        assert_eq!(s.designated(), Some(7));
        assert_eq!(s.readiness(l), Readiness::WrongTarget);
        assert!(s.weapon_observation(l).is_none());
        let rounds = s.rounds(0);
        s.step(true, l, |_, _| 0.);
        assert_eq!(s.rounds(0), rounds);
    }
}

#[test]
fn radar_power_off_disables_bore_and_latches_unguided_release() {
    for name in ["AIM120.JT", "R530.JT"] {
        let mut s = fixture(true);
        s.config.stations[0].weapon = weapon(name);
        s.targets.push(target(7, [0., 1000., 5000.], 200, 0x80));
        let mut l = range_launcher();
        for _ in 0..DWELL + 1 {
            s.step(false, l, |_, _| 0.);
        }
        l.radar = false;
        l.radar_power = false;
        s.step(false, l, |_, _| 0.);
        s.command(Command::ToggleSeekerMode, l);
        assert_eq!(s.launch_mode, LaunchMode::Cued);
        assert!(s.bore_observation.is_none());
        assert!(s.mounted.observation.is_none());
        assert!(s.seeker_tone(l).is_none());
        assert!(s.weapon_observation(l).is_none());
        assert!(!s.can_lock(l));
        assert_eq!(s.readiness(l), Readiness::Ready);
        assert!(s.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
        let mut p = s.projectiles[0].clone();
        let direction = p.direction;
        let w = s.config.stations[0].weapon.clone();
        assert!(p.motion.is_some());
        assert!(p.guidance.as_ref().unwrap().unguided);
        assert!(!p.guidance.as_ref().unwrap().enabled);
        l.radar = true;
        l.radar_power = true;
        for _ in 0..DWELL + 1 {
            s.step(false, l, |_, _| 0.);
            guide(&mut p, &w, &s.targets, &s.sensors, &|_, _| false);
        }
        assert_eq!(p.target, None);
        assert_eq!(p.direction, direction);
        let f = p.guidance.as_ref().unwrap();
        assert_eq!(f.seeker.status, Status::Unguided);
        assert!(f.last_intercept.is_none());
        if f.profile.supports_boresight() {
            assert_eq!(s.launch_mode, LaunchMode::Boresight);
        }
    }
}

#[test]
fn passive_channel_is_not_the_radar_power_switch() {
    let mut s = fixture(true);
    s.config.stations[0].weapon = weapon("AIM9M.JT");
    let mut l = range_launcher();
    l.radar = false;
    l.controls.channel = sensors::Channel::Infrared;
    s.step(false, l, |_, _| 0.);
    assert_eq!(s.launch_mode, LaunchMode::Boresight);
    assert!(s.seeker_tone(l).is_none());
}

#[test]
fn ir_bore_audio_uses_candidate_percentage_and_same_target_lock_without_designation() {
    let mut s = fixture(true);
    s.config.stations[0].weapon = weapon("AIM9M.JT");
    s.config.stations[0].weapon.flags |= 0x10000;
    s.targets.push(target(7, [0., 1000., 1000.], 200, 0x80));
    let mut l = range_launcher();
    l.radar_power = false;
    l.radar = false;
    let mut empty = s.clone();
    empty.targets.clear();
    empty.step(false, l, |_, _| 0.);
    assert!(empty.seeker_tone(l).is_none());
    let mut weak = s.clone();
    weak.targets[0].signature.infrared = 10.;
    weak.step(false, l, |_, _| 0.);
    assert!(weak.bore_observation.is_some());
    assert!(weak.mounted.observation.is_none());
    assert!(weak.seeker_tone(l).is_none());
    s.step(false, l, |_, _| 0.);
    assert_eq!(s.launch_mode, LaunchMode::Boresight);
    assert_eq!(s.designated(), None);
    assert_eq!(s.weapon_observation(l).unwrap().id, 7);
    let percent = s.estimated_hit_percent(l);
    assert!(percent > 0);
    let tracking = s.seeker_tone(l).unwrap();
    assert!(!tracking.locked && !tracking.radar && !tracking.ground);
    assert_eq!(tracking.strength, SeekerTone::ir_strength(percent, false));
    for _ in 1..DWELL {
        s.step(false, l, |_, _| 0.);
    }
    let locked = s.seeker_tone(l).unwrap();
    assert!(locked.locked);
    assert_eq!(s.estimated_hit_percent(l), percent);
    assert_eq!(locked.strength, 2. * tracking.strength);

    // A changed percentage on the same target changes volume immediately.
    s.targets[0].signature.infrared = 35.;
    s.step(false, l, |_, _| 0.);
    let lower = s.seeker_tone(l).unwrap();
    assert!(lower.locked);
    assert!(s.estimated_hit_percent(l) < percent);
    assert_eq!(
        lower.strength,
        SeekerTone::ir_strength(s.estimated_hit_percent(l), true)
    );
    assert!(lower.strength < locked.strength);

    let mut hot = target(8, [0., 1000., 1500.], 200, 0x80);
    hot.signature.infrared = 200.;
    s.targets.push(hot);
    s.step(false, l, |_, _| 0.);
    assert_eq!(s.weapon_observation(l).unwrap().id, 8);
    let switched = s.seeker_tone(l).unwrap();
    assert!(!switched.locked);
    assert_eq!(
        switched.strength,
        SeekerTone::ir_strength(s.estimated_hit_percent(l), false)
    );
    // Even a stale lock on the other identity cannot elevate this candidate.
    s.mounted.target = Some(7);
    s.mounted.status = Status::Locked;
    assert!(!s.seeker_tone(l).unwrap().locked);
    for _ in 0..DWELL {
        s.step(false, l, |_, _| 0.);
    }
    assert!(s.seeker_tone(l).unwrap().locked);
    assert_eq!(s.designated(), None);

    for gate in 0..5 {
        let mut gated = s.clone();
        let mut launcher = l;
        match gate {
            0 => gated.armed = false,
            1 => gated.ammo[0] = 0,
            2 => gated.ammo[0] |= 0x8000,
            3 => gated.player_hp = 0,
            _ => launcher.alive = false,
        }
        assert!(gated.seeker_tone(launcher).is_none());
    }
    let mut released = s.clone();
    assert!(released.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
    assert!(released.seeker_tone(l).is_none());
    let mut masked = s.clone();
    masked.step(false, l, |_, _| 2000.);
    assert!(masked.seeker_tone(l).is_none());
    for target in &mut s.targets {
        target.position[0] = 5000.;
    }
    s.step(false, l, |_, _| 0.);
    assert!(s.weapon_observation(l).is_none());
    assert!(s.seeker_tone(l).is_none());
}

#[test]
fn armed_ir_bore_ignores_radar_power_without_designation() {
    let mut s = fixture(true);
    s.config.stations[0].weapon = weapon("AIM9M.JT");
    s.targets.push(target(7, [0., 1000., 1000.], 200, 0x80));
    let mut l = range_launcher();
    for _ in 0..DWELL + 1 {
        s.step(false, l, |_, _| 0.);
    }
    s.command(Command::ClearDesignation, l);
    l.radar_power = false;
    l.radar = false;
    for _ in 0..DWELL + 1 {
        s.step(false, l, |_, _| 0.);
    }
    assert_eq!(s.launch_mode, LaunchMode::Boresight);
    assert_eq!(s.mounted.target, Some(7));
    assert!(
        s.seeker_tone(l)
            .is_some_and(|tone| tone.locked && !tone.radar)
    );
    assert!(s.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
    assert!(!s.projectiles[0].guidance.as_ref().unwrap().unguided);
    assert_eq!(s.projectiles[0].target, Some(7));
    s.config.stations[0].weapon.seeker.zones[1].minimum_range = 2000;
    s.step(false, l, |_, _| 0.);
    assert!(s.weapon_observation(l).is_none());
    assert_eq!(s.readiness(l), Readiness::MinimumRange);
}

#[test]
fn radar_bore_respects_scope_and_aircraft_tracking_ranges() {
    let mut s = fixture(true);
    let mut w = weapon("AIM120.JT");
    w.seeker.zones[0].maximum_range = 100000;
    w.seeker.zones[1].maximum_range = 100000;
    s.config.stations[0].weapon = w;
    s.config.sensors.radar.as_mut().unwrap().track.maximum_ft = 50000.;
    s.targets.push(target(7, [0., 1000., 35000.], 200, 0x80));
    let mut l = range_launcher();
    l.controls.range_index = 0; // 5 nmi
    s.step(false, l, |_, _| 0.);
    assert!(s.bore_observation.is_none());
    l.controls.range_index = 1; // 10 nmi
    s.step(false, l, |_, _| 0.);
    assert!(s.bore_observation.is_some());
    s.config.sensors.radar.as_mut().unwrap().track.maximum_ft = 34999.;
    s.step(false, l, |_, _| 0.);
    assert!(s.bore_observation.is_none());
    s.config.sensors.radar.as_mut().unwrap().track.maximum_ft = 35000.;
    s.step(false, l, |_, _| 0.);
    assert!(s.bore_observation.is_some());
}

#[test]
fn selected_track_overrides_ir_bore_and_release_restores_search() {
    let mut s = fixture(true);
    s.config.stations[0].weapon = weapon("AIM9M.JT");
    let mut strongest = target(7, [0., 1000., 1000.], 200, 0x80);
    strongest.signature.infrared = 400.;
    s.targets.push(strongest);
    s.targets.push(target(8, [300., 1000., 1000.], 200, 0x80));
    let l = range_launcher();
    for _ in 0..DWELL + 1 {
        s.step(false, l, |_, _| 0.);
    }
    assert_eq!(s.mounted.target, Some(7));
    s.command(Command::DesignateTarget(8), l);
    assert_eq!(s.designated(), Some(8));
    s.command(Command::ToggleSeekerMode, l);
    assert_eq!(s.launch_mode, LaunchMode::Cued);
    for _ in 0..DWELL + 1 {
        s.step(false, l, |_, _| 0.);
    }
    assert_eq!(s.mounted.target, Some(8));
    assert_eq!(s.mounted.status, Status::Locked);
    assert!(s.bore_observation.is_none());
    assert!(s.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
    assert_eq!(s.projectiles[0].target, Some(8));
    s.command(Command::ClearDesignation, l);
    for _ in 0..DWELL + 1 {
        s.step(false, l, |_, _| 0.);
    }
    assert_eq!(s.launch_mode, LaunchMode::Boresight);
    assert_eq!(s.mounted.target, Some(7));
    assert_eq!(s.projectiles[0].target, Some(8));
}

fn range_weapon() -> Weapon {
    let mut w = weapon("AIM120.JT");
    w.movement.ignite_t = 0;
    w.movement.fuel_t = 24;
    w.movement.remove_t = 120;
    w.movement.initial_speed = 0;
    w.movement.maximum_speed = 1600;
    w.movement.acceleration = 400;
    w.movement.deceleration = 20;
    w.movement.final_speed = 200;
    w.movement.powered_turn_rate = 10000;
    w.movement.unpowered_turn_rate = 5000;
    w.movement.performance_at_0 = 100;
    w.movement.performance_at_20 = 100;
    for zone in &mut w.seeker.zones {
        zone.minimum_range = 0;
        zone.maximum_range = 100000;
        zone.heading = i16::MAX;
        zone.pitch = i16::MAX;
    }
    w
}

#[test]
fn maximum_range_changes_with_launch_speed_aspect_and_turn_cost() {
    let w = range_weapon();
    let origin = [0., 1000., 0.];
    let target = [0., 1000., 10000.];
    let life = Profile::for_weapon(&w).unwrap().guidance_ticks;
    let range = |speed, heading, target_velocity| {
        missiles::maximum_range(
            &w,
            origin,
            Basis::new(heading, 0., 0.).forward,
            [0., 0., speed],
            target,
            target_velocity,
            life,
        )
    };
    let head = range(600., 0., [0., 0., -300.]);
    let cross = range(600., 0., [300., 0., 0.]);
    let tail = range(600., 0., [0., 0., 300.]);
    assert!(head > cross && cross > tail, "{head} {cross} {tail}");
    let slow = range(300., 0., [0., 0., 300.]);
    let fast = range(900., 0., [0., 0., 300.]);
    assert!(fast > tail && tail > slow, "{fast} {tail} {slow}");
    let turned = range(600., 60f64.to_radians(), [0., 0., 300.]);
    assert!(turned < tail, "{turned} {tail}");
    eprintln!(
        "fitted range feet: head={head:.1} cross={cross:.1} tail={tail:.1} slow={slow:.1} fast={fast:.1} turned={turned:.1}"
    );
    assert_eq!(head, range(600., 0., [0., 0., -300.]));
}

#[test]
fn predicted_interception_matches_live_guidance_for_observed_motion() {
    let w = range_weapon();
    let sensors = fixture(true).sensors;
    for (heading, launch_velocity, target_velocity) in [
        (0., [0., 0., 600.], [0., 0., -300.]),
        (0., [0., 0., 600.], [0., 0., 300.]),
        (0., [0., 0., 600.], [250., 0., 0.]),
        (0., [0., 100., 600.], [0., 80., 200.]),
        (25f64.to_radians(), [100., 40., 600.], [150., 0., 100.]),
    ] {
        let mut p = shot(&w, LaunchMode::Cued, Some(7));
        p.direction = Basis::new(heading, 0., 0.).forward;
        p.motion = Some(Motion::new(&w.movement, launch_velocity, 1000.));
        let f = p.guidance.as_mut().unwrap();
        f.enabled = true;
        f.seeker.acquired = true;
        f.seeker.candidate = Some(7);
        f.seeker.dwell = DWELL;
        let life = f.profile.guidance_ticks;
        let mut t = target(7, [0., 1000., 10000.], 200, 0x80);
        t.velocity = target_velocity;
        let predicted = missiles::intercept(
            &w.movement,
            p.motion.unwrap(),
            p.position,
            p.direction,
            t.position,
            t.velocity,
            0,
            life,
        )
        .expect("reachable fixture");
        let mut hit_time = None;
        for age in 0..life {
            p.age = age;
            let relative = sub(t.position, p.position);
            let old_direction = p.direction;
            guide(&mut p, &w, &[t.clone()], &sensors, &|_, _| false);
            let motion = p.motion.as_mut().unwrap();
            motion.turn(old_direction, p.direction);
            let delta = motion.step(&w.movement, age, p.direction);
            for (i, movement) in delta.into_iter().enumerate() {
                p.position[i] += movement;
                t.position[i] += t.velocity[i] * missiles::DT;
            }
            if segment_sphere(relative, sub(t.position, p.position), 25.).is_some() {
                hit_time = Some((age + 1) as f64 * missiles::DT);
                break;
            }
        }
        assert_eq!(hit_time, Some(predicted.seconds), "heading={heading}");
    }
}

#[test]
fn turns_reduce_energy_without_inventing_speed_or_instant_reversal() {
    let w = range_weapon();
    let forward = [0., 0., 1.];
    let next = missiles::steer(&w.movement, 0, forward, [0., 0., -1.]);
    assert!(next[2] > 0.99 && next != forward);
    let mut motion = Motion::new(&w.movement, [0., 0., 600.], 1000.);
    motion.turn(forward, next);
    assert!(missiles::length(motion.velocity) < 600.);
    let retained = motion.velocity;
    motion.turn(next, next);
    assert_eq!(motion.velocity, retained);
    let mut zero = w.movement;
    zero.powered_turn_rate = 0;
    assert!(
        missiles::intercept(
            &zero,
            Motion::new(&zero, [0., 0., 600.], 1000.),
            [0., 1000., 0.],
            forward,
            [0., 1000., 3000.],
            [300., 0., 0.],
            0,
            500
        )
        .is_none()
    );
}

#[test]
fn favorable_firing_band_uses_prediction_and_reserves_minimum_margin() {
    let w = range_weapon();
    let l = range_launcher();
    let profile = Profile::for_weapon(&w).unwrap();
    let o = seeker::Observation {
        id: 7,
        position: [0., 1000., 10000.],
        velocity: [0., 0., 300.],
        quality: 1.,
        off_axis: 0.,
        range: 10000.,
    };
    let max = missiles::maximum_range(
        &w,
        l.position,
        l.basis.forward,
        l.velocity,
        o.position,
        o.velocity,
        profile.guidance_ticks,
    );
    let band = missiles::firing_band(
        &w,
        l.position,
        l.basis,
        l.velocity,
        o,
        max,
        profile.guidance_ticks,
        false,
    )
    .unwrap();
    assert!((band.minimum - max * 0.1).abs() < 1e-9);
    assert!(band.maximum > band.minimum && band.maximum < max);
    let mut zone = w.seeker.zones[1];
    zone.maximum_range = max.floor() as _;
    for range in [band.minimum, band.maximum] {
        let point = [0., 1000., range];
        let solution = missiles::intercept(
            &w.movement,
            Motion::new(&w.movement, l.velocity, l.position[1]),
            l.position,
            l.basis.forward,
            point,
            o.velocity,
            0,
            profile.guidance_ticks,
        );
        assert!(
            missiles::estimated_hit_percent(
                seeker::Observation {
                    position: point,
                    range,
                    ..o
                },
                solution,
                &zone,
                profile.guidance_ticks as f64 * missiles::DT,
                None
            ) >= 70
        );
    }
    assert!(
        missiles::firing_band(
            &w,
            l.position,
            l.basis,
            l.velocity,
            seeker::Observation { quality: 0.2, ..o },
            max,
            profile.guidance_ticks,
            false
        )
        .is_none()
    );
    assert!(
        missiles::firing_band(
            &w,
            l.position,
            l.basis,
            l.velocity,
            o,
            0.,
            profile.guidance_ticks,
            false
        )
        .is_none()
    );
    assert!(
        missiles::firing_band(
            &w,
            l.position,
            l.basis,
            l.velocity,
            seeker::Observation {
                off_axis: profile.search_cap(),
                ..o
            },
            max,
            profile.guidance_ticks,
            true
        )
        .is_none()
    );
}

#[test]
fn in_range_is_not_gated_by_rounded_hit_percentage_and_safe_hides_band() {
    let mut s = fixture(true);
    s.config.stations[0].weapon = range_weapon();
    s.targets.push(target(7, [0., 1000., 1000.], 200, 0x80));
    let l = range_launcher();
    s.step(false, l, |_, _| 0.);
    assert!(s.favorable_firing_band(l).is_some());
    s.bore_observation.as_mut().unwrap().quality = 0.001;
    assert_eq!(s.estimated_hit_percent(l), 0);
    assert!(s.in_estimated_range(l));
    s.command(Command::ToggleArm, l);
    assert!(!s.in_estimated_range(l));
    assert!(s.favorable_firing_band(l).is_none());
}

#[test]
fn radar_estimated_range_exceeds_nominal_and_changes_above_old_ceiling() {
    let mut w = range_weapon();
    w.seeker.zones[1].maximum_range = 5000;
    w.seeker.zones[0].maximum_range = 5000;
    let profile = Profile::for_weapon(&w).unwrap();
    let origin = [0., 20000., 0.];
    let target = [0., 20000., 20000.];
    let estimate = |speed: f64, pitch: f64| {
        let basis = Basis::new(0., pitch, 0.);
        missiles::maximum_range(
            &w,
            origin,
            basis.forward,
            basis.forward.map(|v| v * speed),
            target,
            [0., 0., -300.],
            profile.guidance_ticks,
        )
    };
    let level = estimate(500. * 1.68781, 0.);
    let fast = estimate(800. * 1.68781, 0.);
    let climb = estimate(800. * 1.68781, 15f64.to_radians());
    assert!(level > 5000. && fast > level && climb > 5000.);
    assert!((climb - fast).abs() > 100.);
    assert_eq!(w.seeker.zones[0].maximum_range, 5000);
    eprintln!(
        "uncapped synthetic feet: level500={level:.1} level800={fast:.1} climb800={climb:.1}"
    );
}

#[test]
fn cued_radar_release_uses_predicted_reach_not_nominal_launch_max() {
    let mut s = fixture(true);
    let mut w = range_weapon();
    w.seeker.zones[0].maximum_range = 5000;
    w.seeker.zones[1].maximum_range = 5000;
    s.config.stations[0].weapon = w.clone();
    let radar = s.config.sensors.radar.as_mut().unwrap();
    radar.search.maximum_ft = 1000000.;
    radar.track.maximum_ft = 1000000.;
    // Sensors keeps its own imported profiles, so replace that service too.
    s.sensors = Sensors::new(s.config.sensors.clone());
    s.targets.push(target(7, [0., 1000., 10000.], 200, 0x80));
    let l = range_launcher();
    s.step(false, l, |_, _| 0.);
    s.command(Command::DesignateTarget(7), l);
    for _ in 0..90 {
        s.step(false, l, |_, _| 0.);
    }
    assert!(s.estimated_max_range(l).unwrap() > 10000.);
    assert_eq!(s.readiness(l), Readiness::Ready);
    assert!(s.in_estimated_range(l));
    let view = seeker::View {
        position: l.position,
        basis: l.basis,
        cap: None,
        obscured: &|_, _| false,
    };
    assert!(seeker::observe(&w, Profile::for_weapon(&w).unwrap(), &view, &s.targets[0]).is_none());
    assert!(s.step(true, l, |_, _| 0.).contains(&Event::Fired(0)));
    s.release();
    s.targets[0].velocity = [0., 0., 10000.];
    for _ in 0..90 {
        s.step(false, l, |_, _| 0.);
    }
    assert_eq!(s.readiness(l), Readiness::MaximumRange);
    assert!(!s.in_estimated_range(l));
    let ammo = s.ammo.clone();
    s.step(true, l, |_, _| 0.);
    assert_eq!(s.ammo, ammo);
}
