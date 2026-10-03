use super::tests::{fixture, target};
use super::*;
use missiles::{DWELL, MEMORY, Profile};
fn weapon(name: &str) -> Weapon {
    let mut w = fixture(true).own().configuration().stations[0]
        .weapon
        .clone();
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
        owner: 0,
        weapon: None,
        id: 0,
        guidance: Some(Flight::new(profile, mode, target, [0., 1000., 0.])),
        motion: Some(Motion::new(&w.movement, [0., 0., 600.], 1000.)),
        guidance_ticks: Some(profile.guidance_ticks),
        age: 0,
        incoming: None,
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
    let sensors = fixture(true).own().sensors.clone();
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
fn a_passed_intercept_cannot_make_an_amraam_circle_but_reacquisition_still_works() {
    let sensors = fixture(true).own().sensors.clone();
    let mut w = weapon("AIM120.JT");
    w.movement.remove_t = 528;
    let mut p = shot(&w, LaunchMode::Cued, Some(1));
    let f = p.guidance.as_mut().unwrap();
    f.last_intercept = Some([150., 1000., -500.]);
    f.enabled = true;
    f.seeker.acquired = true;
    let heading = p.direction;
    for age in 0..720 {
        p.age = age;
        guide(&mut p, &w, &[], &sensors, &|_, _| false);
        assert_eq!(p.direction, heading, "age={age}");
        let delta = p
            .motion
            .as_mut()
            .unwrap()
            .step(&w.movement, age, p.direction);
        p.position = std::array::from_fn(|i| p.position[i] + delta[i]);
    }
    assert_eq!(p.guidance.as_ref().unwrap().seeker.status, Status::Lost);
    let mut wreck = target(
        1,
        [p.position[0] + 500., p.position[1], p.position[2] + 4000.],
        100,
        0x80,
    );
    wreck.hp = 0;
    for age in 720..720 + u64::from(DWELL) + 12 {
        p.age = age;
        guide(&mut p, &w, &[wreck.clone()], &sensors, &|_, _| false);
    }
    assert_eq!(p.guidance.as_ref().unwrap().seeker.status, Status::Pitbull);
    assert_ne!(p.direction, heading);
    p.age = p.guidance_ticks.unwrap();
    let heading = p.direction;
    guide(&mut p, &w, &[wreck], &sensors, &|_, _| false);
    assert_eq!(p.guidance.as_ref().unwrap().seeker.status, Status::Expired);
    assert_eq!(p.direction, heading);
}

#[test]
fn hidden_movement_never_updates_intercept_and_expiry_precedes_acquisition() {
    let sensors = fixture(true).own().sensors.clone();
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
    s.own_mut().config.stations[0].weapon = weapon("AIM9M.JT");
    s.own_mut().launch_mode = LaunchMode::Boresight;
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
        body_present: true,
        controls: Default::default(),
    };
    s.own_mut().config.sensors.radar = None;
    s.own_mut().config.sensors.infrared = None;
    assert_eq!(s.own_view().readiness(l), Readiness::Ready);
    assert!(
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: true,
                launcher: l
            }],
            |_, _| 0.
        )
        .contains(&Event::Fired {
            aircraft: 0,
            station: 0
        })
    );
    assert_eq!(s.projectiles[0].target, None);
    assert_eq!(
        s.projectiles[0].guidance.as_ref().unwrap().mode,
        LaunchMode::Boresight
    );
    assert_eq!(s.projectiles[0].motion.as_ref().unwrap().velocity[0], 40.);
    s.release(0);
    let pos = s.projectiles[0].position;
    s.targets
        .push(target(77, [pos[0], pos[1], pos[2] + 2500.], 20, 0x80));
    for _ in 0..DWELL + 1 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert_eq!(s.projectiles[0].target, Some(77));
    assert_eq!(s.own_view().designated(), None);
    assert!(
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: true,
                launcher: l
            }],
            |_, _| 0.
        )
        .contains(&Event::Fired {
            aircraft: 0,
            station: 0
        })
    );
    assert!(!s.own().mounted.acquired);
    assert_eq!(s.own().mounted.dwell, 0);
}
#[test]
fn two_active_shots_own_targets_and_reacquire_without_support() {
    let sensors = fixture(true).own().sensors.clone();
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
    s.own_mut().config.stations[0].weapon = weapon("AIM9M.JT");
    s.own_mut().launch_mode = LaunchMode::Boresight;
    let mut l = Launcher {
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 600.,
        velocity: [0., 0., 600.],
        radar_power: false,
        radar: false,
        jammer: false,
        alive: true,
        body_present: true,
        bay_ready: false,
        controls: Default::default(),
    };
    // A closed bay is not an inhibit: the trigger opens it.
    assert_eq!(s.own_view().readiness(l), Readiness::Ready);
    assert!(!s.bay_demand(0));
    let ammo = s.own().ammo.clone();
    let mut probe = s.clone();
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: true,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert_eq!(s.own().ammo, ammo);
    assert_eq!(s.own().release_readiness, Readiness::BayClosed);
    assert!(s.bay_demand(0));
    // The press alone commits the shot; it fires when the doors are open.
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert_eq!(s.own().ammo, ammo);
    l.bay_ready = true;
    let events = s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Fired {
            aircraft: 0,
            station: 0
        }
    )));
    assert!(s.own().rounds(0) < ammo[0] & 0x7fff);
    assert!(s.bay_demand(0));
    let mut held_open = 0;
    while s.bay_demand(0) && held_open < 1000 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
        held_open += 1;
    }
    assert_eq!(held_open, 120);
    // A request that never sees the doors open lapses after 3 seconds.
    l.bay_ready = false;
    probe.step(
        &[OwnshipInput {
            aircraft: 0,
            held: true,
            launcher: l,
        }],
        |_, _| 0.,
    );
    for _ in 0..361 {
        probe.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert!(!probe.bay_demand(0));
    assert_eq!(probe.own().ammo, ammo);
    l.bay_ready = true;
    s.own_mut().armed = false;
    assert_eq!(s.own_view().readiness(l), Readiness::Safe);
    assert!(s.own_view().seeker_tone(l).is_none());
    s.own_mut().armed = true;
    s.own_mut().ammo[0] |= 0x8000;
    assert_eq!(s.own_view().readiness(l), Readiness::StationFailed);
    assert!(s.own_view().seeker_tone(l).is_none());
    s.own_mut().ammo[0] = 0;
    assert_eq!(s.own_view().readiness(l), Readiness::Empty);
    assert!(s.own_view().seeker_tone(l).is_none());
}

#[test]
fn mounted_lock_is_not_boresight_release_permission() {
    let mut s = fixture(true);
    s.own_mut().config.stations[0].weapon = weapon("AIM9M.JT");
    s.own_mut().launch_mode = LaunchMode::Boresight;
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
        body_present: true,
        controls: Default::default(),
    };
    assert_eq!(s.own_view().readiness(l), Readiness::Ready);
    assert!(!s.own_view().can_lock(l));
    s.command(0, Command::CompatibilityWeapons, l);
    assert_eq!(s.own().launch_mode, LaunchMode::Cued);
    s.command(0, Command::ToggleSeekerMode, l);
    assert_eq!(s.own().launch_mode, LaunchMode::Cued);
    assert_eq!(s.own_view().readiness(l), Readiness::NoTarget);
}
#[test]
fn render_cadence_and_pause_do_not_change_missile_state() {
    let run = |fps: u32| {
        let mut s = fixture(true);
        s.own_mut().config.stations[0].weapon = weapon("AIM120.JT");
        s.own_mut().launch_mode = LaunchMode::Boresight;
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
            body_present: true,
            controls: Default::default(),
        };
        s.range_target(0, l);
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
                    s.command(0, Command::ToggleSeekerMode, l);
                }
                events.extend(s.step(
                    &[OwnshipInput {
                        aircraft: 0,
                        held: tick == 60,
                        launcher: l,
                    }],
                    |_, _| 0.,
                ));
                tick += 1;
            }
        }
        let mounted = s.own().mounted.clone();
        (s.projectiles, s.targets, mounted, events)
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
    s.own_mut().config.stations[0].weapon = w;
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
        body_present: true,
        controls: Default::default(),
    };
    s.range_target(0, l);
    s.targets[0].position = [0., 1000., 40000.];
    s.targets[0].velocity = [0., 0., 300.];
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    s.designate_next(0, true);
    for _ in 0..90 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert!(
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: true,
                launcher: l
            }],
            |_, _| 0.
        )
        .contains(&Event::Fired {
            aircraft: 0,
            station: 0
        })
    );
    s.release(0);
    let known = s.projectiles[0]
        .guidance
        .as_ref()
        .unwrap()
        .last_intercept
        .unwrap();
    assert!(!s.projectiles[0].guidance.as_ref().unwrap().enabled);
    l.radar = false;
    s.targets[0].position = [100000., 1000., 90000.];
    s.command(0, Command::ClearDesignation, l);
    for _ in 0..60 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
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
        body_present: true,
        controls: Default::default(),
    };
    let tone_at = |distance: f64| {
        let mut s = fixture(true);
        s.own_mut().config.stations[0].weapon = weapon("AIM120.JT");
        s.own_mut().config.stations[0].weapon.seeker.zones[0].minimum_range = 0;
        s.own_mut().config.stations[0].weapon.seeker.zones[1].minimum_range = 4000;
        s.targets.push(target(7, [0., 1000., distance], 20, 0x80));
        for _ in 0..DWELL + 1 {
            s.step(
                &[OwnshipInput {
                    aircraft: 0,
                    held: false,
                    launcher: l,
                }],
                |_, _| 0.,
            );
        }
        assert!(s.own().bore_observation.is_some());
        s.own_view().seeker_tone(l)
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
        body_present: true,
        controls: Default::default(),
    };
    let mut s = fixture(true);
    s.own_mut().config.stations[0].weapon = weapon("AIM120.JT");
    s.targets.push(target(7, [0., 1000., 5000.], 20, 0x80));
    for _ in 0..DWELL + 1 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert_eq!(s.own().launch_mode, LaunchMode::Boresight);
    assert_eq!(s.own().mounted.target, None);
    assert_eq!(s.own().mounted.status, Status::Search);
    assert_eq!(s.own().bore_observation.unwrap().id, 7);
    assert_eq!(s.own_view().designated(), None);
    // The bore return sounds the radar lock tone (John, 2026-09-23).
    assert!(
        s.own_view()
            .seeker_tone(l)
            .is_some_and(|t| t.radar && t.locked)
    );
    assert!(
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: true,
                launcher: l
            }],
            |_, _| 0.
        )
        .contains(&Event::Fired {
            aircraft: 0,
            station: 0
        })
    );
    assert!(s.projectiles[0].guidance.as_ref().unwrap().enabled);
    assert_eq!(s.projectiles[0].target, None);
    s.command(0, Command::DesignateTarget(7), l);
    assert_eq!(s.own_view().designated(), Some(7));
    assert_eq!(s.own().launch_mode, LaunchMode::Cued);
    // Silent until the seeker is actually tracking the new selection.
    assert!(s.own_view().seeker_tone(l).is_none());
    for _ in 0..crate::sensors::track::ACQUISITION_STEPS + DWELL + 1 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert!(s.own_view().seeker_tone(l).is_some());
    s.command(0, Command::ClearDesignation, l);
    assert_eq!(s.own_view().designated(), None);
    assert_eq!(s.own().mounted.target, None);
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert_eq!(s.own().launch_mode, LaunchMode::Boresight);
    s.own_mut().config.stations[0].weapon = weapon("AIM9M.JT");
    s.own_mut().ammo[0] = 5;
    for _ in 0..DWELL + 1 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert_eq!(s.own().mounted.target, Some(7));
    let mut hot = target(8, [400., 1000., 5000.], 20, 0x80);
    hot.signature.infrared = 400.;
    s.targets.push(hot);
    for _ in 0..DWELL {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert_eq!(s.own().mounted.target, Some(8));
    // This off-axis fixture acquires heat but cannot produce a launch/HUD solution.
    assert!(s.own_view().weapon_observation(l).is_none());
    assert!(s.own_view().seeker_tone(l).is_none());
    s.targets[1].position = [5000., 1000., 5000.];
    s.targets[0].position = [-5000., 1000., 5000.];
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert_eq!(s.own().mounted.status, Status::Search);
    assert_eq!(s.own().mounted.target, None);
    assert!(s.own_view().seeker_tone(l).is_none());
    // A supported weapon cannot gain an independent radar seeker.
    s.own_mut().config.stations[0].weapon = weapon("R530.JT");
    s.own_mut().config.stations[0].weapon.flags |= 0x200;
    s.own_mut().launch_mode = LaunchMode::Cued;
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert_eq!(s.own().launch_mode, LaunchMode::Cued);
    assert_eq!(s.own_view().readiness(l), Readiness::NoTarget);
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
        body_present: true,
        controls: Default::default(),
    }
}

#[test]
fn minimum_range_inhibits_bore_release_at_the_inclusive_boundary() {
    for name in ["AIM120.JT", "AIM9M.JT"] {
        for distance in [999., 1000., 1001.] {
            let mut s = fixture(true);
            s.own_mut().config.stations[0].weapon = weapon(name);
            s.own_mut().config.stations[0].weapon.seeker.zones[1].minimum_range = 1000;
            s.targets.push(target(7, [0., 1000., distance], 200, 0x80));
            let l = range_launcher();
            for _ in 0..DWELL + 1 {
                s.step(
                    &[OwnshipInput {
                        aircraft: 0,
                        held: false,
                        launcher: l,
                    }],
                    |_, _| 0.,
                );
            }
            assert!(s.own().bore_observation.is_some());
            let rounds = s.own().rounds(0);
            if distance < 1000. {
                assert_eq!(s.own_view().readiness(l), Readiness::MinimumRange, "{name}");
                s.step(
                    &[OwnshipInput {
                        aircraft: 0,
                        held: true,
                        launcher: l,
                    }],
                    |_, _| 0.,
                );
                assert_eq!(s.own().rounds(0), rounds);
                assert!(s.projectiles.is_empty());
            } else {
                assert_eq!(s.own_view().readiness(l), Readiness::Ready, "{name}");
                s.step(
                    &[OwnshipInput {
                        aircraft: 0,
                        held: true,
                        launcher: l,
                    }],
                    |_, _| 0.,
                );
                assert!(s.own().rounds(0) < rounds);
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
        let sensors = fixture(true).own().sensors.clone();
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
        s.own_mut().config.stations[0].weapon = w;
        s.targets.push(target(7, [0., 1000., 500.], 200, 0x80));
        s.projectiles.push(p);
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: range_launcher(),
            }],
            |_, _| 0.,
        );
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
        s.own_mut().config.stations[0].weapon = w;
        s.targets.push(target(7, [0., 1000., 1000.], 200, 0x80));
        let l = range_launcher();
        for _ in 0..DWELL + 1 {
            s.step(
                &[OwnshipInput {
                    aircraft: 0,
                    held: false,
                    launcher: l,
                }],
                |_, _| 0.,
            );
        }
        s.command(0, Command::ToggleSeekerMode, l);
        assert_eq!(s.own().launch_mode, LaunchMode::Cued);
        s.command(0, Command::DesignateTarget(7), l);
        assert_eq!(s.own_view().designated(), Some(7));
        assert_eq!(s.own_view().readiness(l), Readiness::WrongTarget);
        assert!(s.own_view().weapon_observation(l).is_none());
        let rounds = s.own().rounds(0);
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: true,
                launcher: l,
            }],
            |_, _| 0.,
        );
        assert_eq!(s.own().rounds(0), rounds);
    }
}

#[test]
fn radar_power_off_disables_bore_and_latches_unguided_release() {
    for name in ["AIM120.JT", "R530.JT"] {
        let mut s = fixture(true);
        s.own_mut().config.stations[0].weapon = weapon(name);
        s.targets.push(target(7, [0., 1000., 5000.], 200, 0x80));
        let mut l = range_launcher();
        for _ in 0..DWELL + 1 {
            s.step(
                &[OwnshipInput {
                    aircraft: 0,
                    held: false,
                    launcher: l,
                }],
                |_, _| 0.,
            );
        }
        l.radar = false;
        l.radar_power = false;
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
        s.command(0, Command::ToggleSeekerMode, l);
        assert_eq!(s.own().launch_mode, LaunchMode::Cued);
        assert!(s.own().bore_observation.is_none());
        assert!(s.own().mounted.observation.is_none());
        assert!(s.own_view().seeker_tone(l).is_none());
        assert!(s.own_view().weapon_observation(l).is_none());
        assert!(!s.own_view().can_lock(l));
        assert_eq!(s.own_view().readiness(l), Readiness::Ready);
        assert!(
            s.step(
                &[OwnshipInput {
                    aircraft: 0,
                    held: true,
                    launcher: l
                }],
                |_, _| 0.
            )
            .contains(&Event::Fired {
                aircraft: 0,
                station: 0
            })
        );
        let mut p = s.projectiles[0].clone();
        let direction = p.direction;
        let w = s.own().config.stations[0].weapon.clone();
        assert!(p.motion.is_some());
        assert!(p.guidance.as_ref().unwrap().unguided);
        assert!(!p.guidance.as_ref().unwrap().enabled);
        l.radar = true;
        l.radar_power = true;
        for _ in 0..DWELL + 1 {
            s.step(
                &[OwnshipInput {
                    aircraft: 0,
                    held: false,
                    launcher: l,
                }],
                |_, _| 0.,
            );
            guide(&mut p, &w, &s.targets, &s.own().sensors, &|_, _| false);
        }
        assert_eq!(p.target, None);
        assert_eq!(p.direction, direction);
        let f = p.guidance.as_ref().unwrap();
        assert_eq!(f.seeker.status, Status::Unguided);
        assert!(f.last_intercept.is_none());
        if f.profile.supports_boresight() {
            assert_eq!(s.own().launch_mode, LaunchMode::Boresight);
        }
    }
}

#[test]
fn passive_channel_is_not_the_radar_power_switch() {
    let mut s = fixture(true);
    s.own_mut().config.stations[0].weapon = weapon("AIM9M.JT");
    let mut l = range_launcher();
    l.radar = false;
    l.controls.channel = sensors::Channel::Infrared;
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert_eq!(s.own().launch_mode, LaunchMode::Boresight);
    assert!(s.own_view().seeker_tone(l).is_none());
}

#[test]
fn ir_bore_audio_uses_candidate_percentage_and_same_target_lock_without_designation() {
    let mut s = fixture(true);
    s.own_mut().config.stations[0].weapon = weapon("AIM9M.JT");
    s.own_mut().config.stations[0].weapon.flags |= 0x10000;
    s.targets.push(target(7, [0., 1000., 1000.], 200, 0x80));
    let mut l = range_launcher();
    l.radar_power = false;
    l.radar = false;
    let mut empty = s.clone();
    empty.targets.clear();
    empty.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert!(empty.own_view().seeker_tone(l).is_none());
    let mut weak = s.clone();
    weak.targets[0].signature.infrared = 10.;
    weak.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert!(weak.own().bore_observation.is_some());
    assert!(weak.own().mounted.observation.is_none());
    assert!(weak.own_view().seeker_tone(l).is_none());
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert_eq!(s.own().launch_mode, LaunchMode::Boresight);
    assert_eq!(s.own_view().designated(), None);
    assert_eq!(s.own_view().weapon_observation(l).unwrap().id, 7);
    let percent = s.own_view().estimated_hit_percent(l);
    assert!(percent > 0);
    let tracking = s.own_view().seeker_tone(l).unwrap();
    assert!(!tracking.locked && !tracking.radar && !tracking.ground);
    assert_eq!(tracking.strength, SeekerTone::ir_strength(percent, false));
    for _ in 1..DWELL {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    let locked = s.own_view().seeker_tone(l).unwrap();
    assert!(locked.locked);
    assert_eq!(s.own_view().estimated_hit_percent(l), percent);
    assert_eq!(locked.strength, 2. * tracking.strength);

    // A changed percentage on the same target changes volume immediately.
    s.targets[0].signature.infrared = 35.;
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    let lower = s.own_view().seeker_tone(l).unwrap();
    assert!(lower.locked);
    assert!(s.own_view().estimated_hit_percent(l) < percent);
    assert_eq!(
        lower.strength,
        SeekerTone::ir_strength(s.own_view().estimated_hit_percent(l), true)
    );
    assert!(lower.strength < locked.strength);

    let mut hot = target(8, [0., 1000., 1500.], 200, 0x80);
    hot.signature.infrared = 200.;
    s.targets.push(hot);
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert_eq!(s.own_view().weapon_observation(l).unwrap().id, 8);
    let switched = s.own_view().seeker_tone(l).unwrap();
    assert!(!switched.locked);
    assert_eq!(
        switched.strength,
        SeekerTone::ir_strength(s.own_view().estimated_hit_percent(l), false)
    );
    // Even a stale lock on the other identity cannot elevate this candidate.
    s.own_mut().mounted.target = Some(7);
    s.own_mut().mounted.status = Status::Locked;
    assert!(!s.own_view().seeker_tone(l).unwrap().locked);
    for _ in 0..DWELL {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert!(s.own_view().seeker_tone(l).unwrap().locked);
    assert_eq!(s.own_view().designated(), None);

    for gate in 0..5 {
        let mut gated = s.clone();
        let mut launcher = l;
        match gate {
            0 => gated.own_mut().armed = false,
            1 => gated.own_mut().ammo[0] = 0,
            2 => gated.own_mut().ammo[0] |= 0x8000,
            3 => gated.own_mut().hp = 0,
            _ => launcher.alive = false,
        }
        assert!(gated.own_view().seeker_tone(launcher).is_none());
    }
    let mut released = s.clone();
    assert!(
        released
            .step(
                &[OwnshipInput {
                    aircraft: 0,
                    held: true,
                    launcher: l
                }],
                |_, _| 0.
            )
            .contains(&Event::Fired {
                aircraft: 0,
                station: 0
            })
    );
    assert!(released.own_view().seeker_tone(l).is_none());
    let mut masked = s.clone();
    masked.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 2000.,
    );
    assert!(masked.own_view().seeker_tone(l).is_none());
    for target in &mut s.targets {
        target.position[0] = 5000.;
    }
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert!(s.own_view().weapon_observation(l).is_none());
    assert!(s.own_view().seeker_tone(l).is_none());
}

#[test]
fn armed_ir_bore_ignores_radar_power_without_designation() {
    let mut s = fixture(true);
    s.own_mut().config.stations[0].weapon = weapon("AIM9M.JT");
    s.targets.push(target(7, [0., 1000., 1000.], 200, 0x80));
    let mut l = range_launcher();
    for _ in 0..DWELL + 1 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    s.command(0, Command::ClearDesignation, l);
    l.radar_power = false;
    l.radar = false;
    for _ in 0..DWELL + 1 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert_eq!(s.own().launch_mode, LaunchMode::Boresight);
    assert_eq!(s.own().mounted.target, Some(7));
    assert!(
        s.own_view()
            .seeker_tone(l)
            .is_some_and(|tone| tone.locked && !tone.radar)
    );
    assert!(
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: true,
                launcher: l
            }],
            |_, _| 0.
        )
        .contains(&Event::Fired {
            aircraft: 0,
            station: 0
        })
    );
    assert!(!s.projectiles[0].guidance.as_ref().unwrap().unguided);
    assert_eq!(s.projectiles[0].target, Some(7));
    s.own_mut().config.stations[0].weapon.seeker.zones[1].minimum_range = 2000;
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert!(s.own_view().weapon_observation(l).is_none());
    assert_eq!(s.own_view().readiness(l), Readiness::MinimumRange);
}

#[test]
fn radar_bore_respects_scope_and_aircraft_tracking_ranges() {
    let mut s = fixture(true);
    let mut w = weapon("AIM120.JT");
    w.seeker.zones[0].maximum_range = 100000;
    w.seeker.zones[1].maximum_range = 100000;
    s.own_mut().config.stations[0].weapon = w;
    s.own_mut()
        .config
        .sensors
        .radar
        .as_mut()
        .unwrap()
        .track
        .maximum_ft = 50000.;
    s.targets.push(target(7, [0., 1000., 35000.], 200, 0x80));
    let mut l = range_launcher();
    l.controls.range_index = 0; // 5 nmi
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert!(s.own().bore_observation.is_none());
    l.controls.range_index = 1; // 10 nmi
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert!(s.own().bore_observation.is_some());
    s.own_mut()
        .config
        .sensors
        .radar
        .as_mut()
        .unwrap()
        .track
        .maximum_ft = 34999.;
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert!(s.own().bore_observation.is_none());
    s.own_mut()
        .config
        .sensors
        .radar
        .as_mut()
        .unwrap()
        .track
        .maximum_ft = 35000.;
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert!(s.own().bore_observation.is_some());
}

#[test]
fn selected_track_overrides_ir_bore_and_release_restores_search() {
    let mut s = fixture(true);
    s.own_mut().config.stations[0].weapon = weapon("AIM9M.JT");
    let mut strongest = target(7, [0., 1000., 1000.], 200, 0x80);
    strongest.signature.infrared = 400.;
    s.targets.push(strongest);
    s.targets.push(target(8, [300., 1000., 1000.], 200, 0x80));
    let l = range_launcher();
    for _ in 0..DWELL + 1 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert_eq!(s.own().mounted.target, Some(7));
    s.command(0, Command::DesignateTarget(8), l);
    assert_eq!(s.own_view().designated(), Some(8));
    s.command(0, Command::ToggleSeekerMode, l);
    assert_eq!(s.own().launch_mode, LaunchMode::Cued);
    for _ in 0..DWELL + 1 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert_eq!(s.own().mounted.target, Some(8));
    assert_eq!(s.own().mounted.status, Status::Locked);
    assert!(s.own().bore_observation.is_none());
    assert!(
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: true,
                launcher: l
            }],
            |_, _| 0.
        )
        .contains(&Event::Fired {
            aircraft: 0,
            station: 0
        })
    );
    assert_eq!(s.projectiles[0].target, Some(8));
    s.command(0, Command::ClearDesignation, l);
    for _ in 0..DWELL + 1 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert_eq!(s.own().launch_mode, LaunchMode::Boresight);
    assert_eq!(s.own().mounted.target, Some(7));
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
    let sensors = fixture(true).own().sensors.clone();
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
    s.own_mut().config.stations[0].weapon = range_weapon();
    s.targets.push(target(7, [0., 1000., 1000.], 200, 0x80));
    let l = range_launcher();
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert!(s.own_view().favorable_firing_band(l).is_some());
    s.own_mut().bore_observation.as_mut().unwrap().quality = 0.001;
    assert_eq!(s.own_view().estimated_hit_percent(l), 0);
    assert!(s.own_view().in_estimated_range(l));
    s.command(0, Command::ToggleArm, l);
    assert!(!s.own_view().in_estimated_range(l));
    assert!(s.own_view().favorable_firing_band(l).is_none());
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
    s.own_mut().config.stations[0].weapon = w.clone();
    let radar = s.own_mut().config.sensors.radar.as_mut().unwrap();
    radar.search.maximum_ft = 1000000.;
    radar.track.maximum_ft = 1000000.;
    // Sensors keeps its own imported profiles, so replace that service too.
    s.own_mut().sensors = Sensors::new(s.own().config.sensors.clone());
    s.targets.push(target(7, [0., 1000., 10000.], 200, 0x80));
    let l = range_launcher();
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: l,
        }],
        |_, _| 0.,
    );
    s.command(0, Command::DesignateTarget(7), l);
    for _ in 0..90 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert!(s.own_view().estimated_max_range(l).unwrap() > 10000.);
    assert_eq!(s.own_view().readiness(l), Readiness::Ready);
    assert!(s.own_view().in_estimated_range(l));
    let view = seeker::View {
        position: l.position,
        basis: l.basis,
        cap: None,
        obscured: &|_, _| false,
    };
    assert!(seeker::observe(&w, Profile::for_weapon(&w).unwrap(), &view, &s.targets[0]).is_none());
    assert!(
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: true,
                launcher: l
            }],
            |_, _| 0.
        )
        .contains(&Event::Fired {
            aircraft: 0,
            station: 0
        })
    );
    s.release(0);
    s.targets[0].velocity = [0., 0., 10000.];
    for _ in 0..90 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert_eq!(s.own_view().readiness(l), Readiness::MaximumRange);
    assert!(!s.own_view().in_estimated_range(l));
    let ammo = s.own().ammo.clone();
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: true,
            launcher: l,
        }],
        |_, _| 0.,
    );
    assert_eq!(s.own().ammo, ammo);
}

/// `missiles::maximum_range` as it was before fly-outs were skipped where the
/// answer is already fixed: every sample flew its whole fly-out (the reference
/// copy of `intercept`, which also stops for nothing).
#[allow(clippy::too_many_arguments)]
fn full_flight_maximum_range(
    w: &Weapon,
    position: Vector,
    forward: Vector,
    velocity: Vector,
    target: Vector,
    target_velocity: Vector,
    lifetime: u64,
) -> f64 {
    use missiles::{Guidance, length, sub};
    let minimum = f64::from(w.seeker.zones[1].minimum_range.max(0));
    let motion = Motion::launch(w, velocity, position[1]);
    let seconds = lifetime.min(u64::from(w.movement.remove_t) * 30) as f64 * missiles::DT;
    let travel_bound = (length(velocity) + motion.budget + length(target_velocity)) * seconds + 25.;
    let cap = if Profile::for_weapon(w).is_some_and(|p| p.guidance == Guidance::Active) {
        travel_bound
    } else {
        travel_bound.min(f64::from(w.seeker.zones[0].maximum_range))
    };
    if cap <= minimum {
        return 0.;
    }
    let bearing = crate::attitude::unit(sub(target, position));
    let reaches = |range| {
        reference_intercept(
            &w.movement,
            Motion::launch(w, velocity, position[1]),
            position,
            forward,
            std::array::from_fn(|i| position[i] + bearing[i] * range),
            target_velocity,
            0,
            lifetime,
        )
        .is_some()
    };
    if reaches(cap) {
        return cap;
    }
    let step = (cap - minimum) / 16.;
    for sample in (0..16).rev() {
        let mut low = minimum + f64::from(sample) * step;
        if !reaches(low) {
            continue;
        }
        let mut high = low + step;
        for _ in 0..10 {
            let middle = (low + high) * 0.5;
            if reaches(middle) {
                low = middle;
            } else {
                high = middle;
            }
        }
        return low;
    }
    0.
}

/// `missiles::firing_band` as it was before the fly-outs were cut short once
/// a later interception could no longer score 70.
#[allow(clippy::too_many_arguments)]
fn full_flight_firing_band(
    w: &Weapon,
    position: Vector,
    basis: Basis,
    velocity: Vector,
    observation: seeker::Observation,
    maximum: f64,
    lifetime: u64,
    bore: bool,
) -> Option<missiles::FiringBand> {
    use missiles::{FiringBand, geometry, launch_geometry, sub};
    let minimum = f64::from(w.seeker.zones[1].minimum_range.max(0));
    if maximum <= minimum {
        return None;
    }
    let profile = Profile::for_weapon(w)?;
    let lower = minimum + 0.1 * (maximum - minimum);
    let step = (maximum - lower) / 16.;
    let bearing = crate::attitude::unit(sub(observation.position, position));
    let mut zone = w.seeker.zones[1];
    zone.maximum_range = maximum.floor() as _;
    let mut start = None;
    let mut best: Option<FiringBand> = None;
    for sample in 0..=16 {
        let range = lower + f64::from(sample) * step;
        let target = std::array::from_fn(|i| position[i] + bearing[i] * range);
        let solution = if geometry(&launch_geometry(w), position, basis, target, None) {
            reference_intercept(
                &w.movement,
                Motion::launch(w, velocity, position[1]),
                position,
                basis.forward,
                target,
                observation.velocity,
                0,
                lifetime,
            )
        } else {
            None
        };
        let score = missiles::estimated_hit_percent(
            seeker::Observation {
                position: target,
                range,
                ..observation
            },
            solution,
            &zone,
            lifetime.min(u64::from(w.movement.remove_t) * 30) as f64 * missiles::DT,
            bore.then(|| profile.search_cap()),
        );
        if score >= 70 {
            let first = *start.get_or_insert(range);
            if range > first && best.is_none_or(|b| range - first > b.maximum - b.minimum) {
                best = Some(FiringBand {
                    minimum: first,
                    maximum: range,
                });
            }
        } else {
            start = None;
        }
    }
    best
}

thread_local! {
    /// Ticks the reference fly-outs have flown on this thread.
    static REFERENCE_TICKS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
fn reference_ticks() -> u64 {
    REFERENCE_TICKS.with(|n| n.get())
}

/// `missiles::intercept` as it was before a coasting fly-out stopped once the
/// target was out of reach: a verbatim copy, kept here as the independent
/// reference, that always flies to a hit or the end.
#[allow(clippy::too_many_arguments)]
fn reference_intercept(
    m: &tore_formats::weapons::Movement,
    mut motion: Motion,
    mut position: Vector,
    mut forward: Vector,
    mut target: Vector,
    velocity: Vector,
    age: u64,
    lifetime: u64,
) -> Option<missiles::Solution> {
    use missiles::{DT, Solution, commanded_heading, lead, length, steer, sub};
    let end = lifetime.min(u64::from(m.remove_t) * 30);
    let mut aim = target;
    for tick in age..end {
        REFERENCE_TICKS.with(|n| n.set(n.get() + 1));
        if tick == age || tick.is_multiple_of(12) {
            aim = lead(position, length(motion.velocity), target, velocity).point;
        }
        let previous = sub(target, position);
        let desired = commanded_heading(
            forward,
            motion.velocity,
            crate::attitude::unit(sub(motion.aim(position, aim), position)),
        );
        let next = steer(m, tick, forward, desired);
        motion.turn(forward, next);
        forward = next;
        let delta = motion.step(m, tick, forward);
        for i in 0..3 {
            position[i] += delta[i];
            target[i] += velocity[i] * DT;
        }
        let relative = sub(target, position);
        let segment = sub(relative, previous);
        let u = (-crate::attitude::dot(previous, segment)
            / crate::attitude::dot(segment, segment).max(1e-12))
        .clamp(0., 1.);
        let closest = std::array::from_fn(|i| previous[i] + segment[i] * u);
        if length(closest) <= 25. {
            return Some(Solution {
                point: target,
                seconds: (tick - age + 1) as f64 * DT,
            });
        }
    }
    None
}

/// A random missile for the fly-out comparisons: motor times, speeds, turn
/// rates, sag, ejection and the cruise profile all vary.
fn random_flyer(case: usize, next: &mut impl FnMut() -> f64) -> Weapon {
    let mut w = range_weapon();
    w.movement.remove_t = [20, 30, 45, 90][case % 4];
    w.movement.ignite_t = [0, 0, 2, 4][case % 4];
    w.movement.fuel_t = w.movement.ignite_t + [2, 6, 12, 24][case % 4];
    w.movement.initial_speed = [0, 300][case % 2];
    w.movement.maximum_speed = (900. + 1200. * next()) as _;
    w.movement.acceleration = (150. + 700. * next()) as _;
    w.movement.deceleration = (10. + 120. * next()) as _;
    w.movement.final_speed = (100. + 400. * next()) as _;
    w.movement.powered_turn_rate = (2000. + 12000. * next()) as _;
    w.movement.unpowered_turn_rate = (1000. + 8000. * next()) as _;
    if case % 5 == 1 {
        w.flags |= missiles::SAG_FLAG;
    }
    if case % 7 == 2 {
        w.flags |= missiles::EJECT_FLAG;
    }
    if case % 6 == 3 {
        w.flags |= missiles::CRUISE_FLAG;
        w.movement.cruise = [4, 8, 2, 4];
    }
    w
}

/// A coasting missile stops flying once the target is out of reach, and that
/// only ever saves time: over many random missiles, launch speeds, target
/// distances, motions and ages the answer is the same, bit for bit, as the
/// copy that always flies on. Cases that a coasting missile hits late, and
/// cases it gives up on early, must both occur, or the comparison proves
/// nothing.
#[test]
fn a_coasting_fly_out_that_gives_up_early_leaves_the_answer_unchanged() {
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut hit, mut late_hit, mut miss) = (0, 0, 0);
    let (mut flown, mut full) = (0, 0);
    for case in 0..1500 {
        let w = random_flyer(case, &mut next);
        let basis = Basis::new((next() - 0.5) * 0.6, (next() - 0.5) * 0.4, 0.);
        let speed = 200. + 1000. * next();
        let velocity: Vector = std::array::from_fn(|i| basis.forward[i] * speed);
        let position = [0., 1000. + 20000. * next(), 0.];
        // Mostly in the missile's reach, some well past it.
        let distance = 1000. + 70000. * next() * next();
        let bearing = Basis::new((next() - 0.5) * 0.8, (next() - 0.5) * 0.4, 0.).forward;
        let target: Vector = std::array::from_fn(|i| position[i] + bearing[i] * distance);
        let target_velocity: Vector = [
            (next() - 0.5) * 900.,
            (next() - 0.5) * 200.,
            (next() - 0.5) * 900.,
        ];
        // Some fly-outs start with the missile already under way.
        let age = [0, 0, 0, 7, 40, 300, 700][case % 7];
        let lifetime = Profile::for_weapon(&w).unwrap().guidance_ticks;
        let launch = Motion::launch(&w, velocity, position[1]);
        let before = missiles::flown_ticks();
        let got = missiles::intercept(
            &w.movement,
            launch,
            position,
            basis.forward,
            target,
            target_velocity,
            age,
            lifetime,
        );
        flown += missiles::flown_ticks() - before;
        let reference = reference_intercept(
            &w.movement,
            launch,
            position,
            basis.forward,
            target,
            target_velocity,
            age,
            lifetime,
        );
        assert_eq!(got, reference, "case {case}");
        let end = lifetime.min(u64::from(w.movement.remove_t) * 30);
        full += match reference {
            Some(s) => (s.seconds / missiles::DT).round() as u64,
            None => end.saturating_sub(age),
        };
        match reference {
            Some(s) => {
                hit += 1;
                late_hit += usize::from(
                    age + (s.seconds / missiles::DT).round() as u64
                        > u64::from(w.movement.fuel_t) * 30 + 16,
                );
            }
            None => miss += 1,
        }
    }
    assert!(
        hit >= 200 && late_hit >= 40 && miss >= 200,
        "hits {hit}, late hits {late_hit}, misses {miss}"
    );
    // Each early stop shows as fewer ticks flown than the whole fly-out.
    assert!(flown * 10 < full * 9, "{flown} of {full} ticks flown");
    eprintln!("hits {hit}, late hits {late_hit}, misses {miss}, flown {flown} of {full}");
}

/// A coasting missile that has stopped moving forward can still fall onto a
/// target beneath it, by sag or by the ejection kick, so the check that ends a
/// coasting fly-out has to count the fall as reach. The random comparison
/// above moves too fast for the fall to matter, so this one is built for it.
#[test]
fn a_coasting_fly_out_counts_sag_and_ejection_as_reach() {
    for (flag, label) in [
        (missiles::SAG_FLAG, "sag"),
        (missiles::EJECT_FLAG, "ejection"),
    ] {
        let mut w = range_weapon();
        w.flags |= flag;
        w.movement.ignite_t = 0;
        w.movement.fuel_t = 0;
        w.movement.remove_t = 60;
        w.movement.initial_speed = 0;
        w.movement.final_speed = 0;
        w.movement.deceleration = 5000;
        let life = Profile::for_weapon(&w).unwrap().guidance_ticks;
        let position = [0., 5000., 0.];
        let target = [0., 4800., 0.];
        let launch = Motion::launch(&w, [0.; 3], position[1]);
        let args = (position, [0., 0., 1.], target, [0.; 3], 0, life);
        let reference = reference_intercept(
            &w.movement,
            launch,
            args.0,
            args.1,
            args.2,
            args.3,
            args.4,
            args.5,
        );
        let got = missiles::intercept(
            &w.movement,
            launch,
            args.0,
            args.1,
            args.2,
            args.3,
            args.4,
            args.5,
        );
        assert!(
            reference.is_some(),
            "{label}: the fall should reach the target"
        );
        assert_eq!(got, reference, "{label}");
    }
}

/// The range estimate and the firing band are the same numbers as when every
/// sample flew its whole fly-out, over a spread of weapons, launch speeds,
/// target ranges and motions, aspects, sag, ejection and boresight launches.
/// The skips (a target out of reach of any fly-out; a fly-out stopped once a
/// later interception cannot score 70) are only allowed to save time.
#[test]
fn skipped_fly_outs_leave_the_range_estimate_and_band_unchanged() {
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut with_band, mut with_range, mut cut_short) = (0, 0, 0);
    for case in 0..48 {
        let mut w = range_weapon();
        w.movement.remove_t = [20, 30, 45][case % 3];
        w.movement.ignite_t = [0, 0, 2, 4][case % 4];
        w.movement.fuel_t = w.movement.ignite_t + [6, 12, 24][case % 3];
        w.movement.maximum_speed = (900. + 1200. * next()) as _;
        w.movement.acceleration = (150. + 700. * next()) as _;
        w.movement.deceleration = (10. + 120. * next()) as _;
        w.movement.final_speed = (100. + 400. * next()) as _;
        w.movement.powered_turn_rate = (2000. + 12000. * next()) as _;
        w.movement.unpowered_turn_rate = (1000. + 8000. * next()) as _;
        if case % 5 == 1 {
            w.flags |= missiles::SAG_FLAG;
        }
        if case % 7 == 2 {
            w.flags |= missiles::EJECT_FLAG;
        }
        w.seeker.zones[1].minimum_range = [0, 500, 1500][case % 3];
        if case % 6 == 3 {
            w.seeker.zones[1].heading = 12000;
            w.seeker.zones[1].pitch = 9000;
        }
        let life = Profile::for_weapon(&w).unwrap().guidance_ticks;
        let speed = 300. + 900. * next();
        let yaw = (next() - 0.5) * 0.6;
        let pitch = (next() - 0.5) * 0.4;
        let basis = Basis::new(yaw, pitch, 0.);
        let position = [0., 1000. + 20000. * next(), 0.];
        let velocity: Vector = std::array::from_fn(|i| basis.forward[i] * speed);
        let distance = 3000. + 90000. * next() * next();
        let bearing =
            Basis::new(yaw + (next() - 0.5) * 0.4, pitch + (next() - 0.5) * 0.2, 0.).forward;
        let target: Vector = std::array::from_fn(|i| position[i] + bearing[i] * distance);
        let target_velocity: Vector = [
            (next() - 0.5) * 900.,
            (next() - 0.5) * 200.,
            (next() - 0.5) * 900.,
        ];
        let expected = full_flight_maximum_range(
            &w,
            position,
            basis.forward,
            velocity,
            target,
            target_velocity,
            life,
        );
        let got = missiles::maximum_range(
            &w,
            position,
            basis.forward,
            velocity,
            target,
            target_velocity,
            life,
        );
        assert_eq!(
            got.to_bits(),
            expected.to_bits(),
            "maximum range, case {case}"
        );
        with_range += usize::from(expected > 0.);
        let observation = seeker::Observation {
            id: 7,
            position: target,
            velocity: target_velocity,
            quality: [1., 0.9, 0.8, 0.6][case % 4],
            off_axis: next() * 0.08,
            range: distance,
        };
        let bore = case % 4 == 3;
        for maximum in [expected, expected * 0.6, expected * 1.4] {
            let reference = full_flight_firing_band(
                &w,
                position,
                basis,
                velocity,
                observation,
                maximum,
                life,
                bore,
            );
            let band = missiles::firing_band(
                &w,
                position,
                basis,
                velocity,
                observation,
                maximum,
                life,
                bore,
            );
            assert_eq!(
                band, reference,
                "firing band, case {case}, maximum {maximum}"
            );
            with_band += usize::from(reference.is_some());
            cut_short += usize::from(reference.is_none() && maximum > 0.);
        }
    }
    // The spread has to exercise both answers, or the comparison proves nothing.
    assert!(
        with_range >= 10 && with_band >= 6 && cut_short >= 6,
        "{with_range} {with_band} {cut_short}"
    );
}

/// The firing estimates run every 60 ticks while a missile is selected and a
/// target is in view, so what one costs is paid again and again, and at eight
/// times time compression sixteen times a second. Each interception flown is
/// about a tenth of a microsecond a tick in a release build, so this counts
/// the ticks flown, which no machine changes, for a fixed set of missiles and
/// targets, and bounds them: the answers match the whole-flight copies, and the
/// work stays well under what they spend. A change that flies every sample to
/// the end again (it was about three times the ticks) fails here.
#[test]
fn a_range_estimate_flies_a_bounded_number_of_ticks() {
    // A 10,200 tick (85 s) fly-out like the long-range radar missiles, the
    // default test missile (30 s), and a short, quick one like a dogfight missile.
    let mut long = range_weapon();
    long.movement.remove_t = 340;
    long.movement.fuel_t = 120;
    let standard = range_weapon();
    let mut short = range_weapon();
    short.movement.remove_t = 30;
    short.movement.fuel_t = 6;
    short.movement.maximum_speed = 2200;
    let l = range_launcher();
    let (mut flown, mut whole) = (0, 0);
    for (name, w) in [("long", &long), ("standard", &standard), ("short", &short)] {
        let life = Profile::for_weapon(w).unwrap().guidance_ticks;
        // Receding, closing, head-on fast, crossing, level and high.
        for (offset, velocity) in [
            ([0., 0., 20000.], [0., 0., 300.]),
            ([0., 0., 20000.], [0., 0., -300.]),
            ([0., 3000., 40000.], [0., 0., -900.]),
            ([6000., 0., 30000.], [-500., 0., 0.]),
            ([-4000., 8000., 15000.], [200., -50., 400.]),
            ([0., -500., 90000.], [0., 0., 0.]),
        ] {
            let position: Vector = std::array::from_fn(|i| l.position[i] + offset[i]);
            let range = missiles::length(offset);
            let o = seeker::Observation {
                id: 7,
                position,
                velocity,
                quality: 1.,
                off_axis: 0.,
                range,
            };
            let before = missiles::flown_ticks();
            let maximum = missiles::maximum_range(
                w,
                l.position,
                l.basis.forward,
                l.velocity,
                o.position,
                o.velocity,
                life,
            );
            let band =
                missiles::firing_band(w, l.position, l.basis, l.velocity, o, maximum, life, false);
            let spent = missiles::flown_ticks() - before;
            let before = reference_ticks();
            let reference_maximum = full_flight_maximum_range(
                w,
                l.position,
                l.basis.forward,
                l.velocity,
                o.position,
                o.velocity,
                life,
            );
            let reference_band = full_flight_firing_band(
                w,
                l.position,
                l.basis,
                l.velocity,
                o,
                reference_maximum,
                life,
                false,
            );
            let full = reference_ticks() - before;
            eprintln!("{name} at {offset:?}: flown {spent} ticks, whole flights {full}");
            assert_eq!(
                (maximum, band),
                (reference_maximum, reference_band),
                "{name} at {offset:?}"
            );
            flown += spent;
            whole += full;
        }
    }
    eprintln!("total: flown {flown} ticks, whole flights {whole}");
    // Measured: 1,231,686 ticks flown against 2,187,517 for the whole flights.
    // The bound leaves room for last-digit differences between platforms'
    // maths, and sits well under what flying every sample out would cost.
    assert!(flown <= 1_500_000, "{flown} ticks flown, over the bound");
    assert!(flown * 10 <= whole * 7, "{flown} of {whole} ticks flown");
}
