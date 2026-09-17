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
