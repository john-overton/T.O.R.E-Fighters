//! The player's air-to-ground missiles through combat (slice W2,
//! docs/spec/air-to-ground.md, "Missiles and the HARM boresight cue"): the
//! designated Maverick lock and its BORE pick, the HARM candidate and its
//! BORE launch, the CUED override, the HARM homing on after a shutdown, the
//! surface seekers seeing ground objects without the air notch, and the A-G
//! track that supports and retargets the AS-7 and AS-15.
use super::tests::fixture;
use super::*;
use crate::airport::OrientedBox;

const REDFOR: Side = Side(2);

fn launcher() -> Launcher {
    Launcher {
        position: [0., 3000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 400.,
        velocity: [0., 0., 400.],
        bay_ready: true,
        radar_power: true,
        radar: true,
        jammer: false,
        alive: true,
        body_present: true,
        controls: sensors::Controls::default(),
    }
}

fn with_channel(channel: sensors::Channel) -> Launcher {
    Launcher {
        controls: sensors::Controls {
            channel,
            ..sensors::Controls::default()
        },
        ..launcher()
    }
}

/// A listed ground object `id` at `(x, z)`, class `class`.
fn ground(s: &mut State, id: u32, x: f64, z: f64, class: u16) {
    s.add_ground_target(
        id,
        OrientedBox {
            center: [x, 5., z],
            half: [8., 5., 15.],
            heading: 0.,
            pitch: 0.,
            bank: 0.,
        },
        100,
        class,
        REDFOR,
    )
    .unwrap();
    assert!(s.set_ground_listing(
        id,
        GroundListing {
            flags: 0x821,
            max_visible: 78,
        }
    ));
}

/// The fixture without its range target, carrying `count` of record `source`
/// (seeker `signature`) with wide synthetic zones: zone 0 and zone 1 45
/// degrees, out to 60,000 ft.
fn armed(source: &str, signature: u8) -> State {
    let mut s = fixture(true);
    s.targets.clear();
    let own = s.own_mut();
    let w = &mut own.config.stations[0].weapon;
    w.source = source.into();
    w.seeker.signature = signature;
    w.movement.remove_t = 400;
    w.movement.fuel_t = 200;
    w.movement.maximum_speed = 3000;
    w.movement.acceleration = 800;
    for zone in &mut w.seeker.zones {
        zone.minimum_range = 0;
        zone.maximum_range = 60_000;
        zone.heading = 8190;
        zone.pitch = 8190;
    }
    own.config.stations[0].internal = true;
    own.ammo = vec![4];
    own.armed = true;
    s
}

fn step(s: &mut State, l: Launcher, held: bool) -> Vec<Event> {
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held,
            launcher: l,
        }],
        |_, _| 0.,
    )
}

/// Steps until the selected weapon reads READY, at most `ticks`.
fn until_ready(s: &mut State, l: Launcher, ticks: usize) -> Readiness {
    for _ in 0..ticks {
        step(s, l, false);
        if s.own_view().readiness(l) == Readiness::Ready {
            return Readiness::Ready;
        }
    }
    s.own_view().readiness(l)
}

/// Fires the selected weapon and returns the new projectile.
fn fire(s: &mut State, l: Launcher) -> Projectile {
    let before = s.projectiles.len();
    step(s, l, true);
    assert_eq!(s.projectiles.len(), before + 1, "one round left");
    s.projectiles.last().unwrap().clone()
}

#[test]
fn the_reviewed_air_to_ground_records_have_surface_profiles() {
    let s = fixture(true);
    let profile = |source: &str| {
        let mut w = s.own().config.stations[0].weapon.clone();
        w.source = source.into();
        missiles::Profile::for_weapon(&w).unwrap()
    };
    for (source, guidance) in [
        ("AGM65G.JT", Guidance::Infrared),
        ("AGM65A.JT", Guidance::Infrared),
        ("AGM84E.JT", Guidance::Infrared),
        ("AGM45.JT", Guidance::Emitter),
        ("AGM88.JT", Guidance::Emitter),
        ("AGM84A.JT", Guidance::Active),
        ("AM39.JT", Guidance::Active),
        ("AS16.JT", Guidance::Active),
        ("AS7.JT", Guidance::Supported),
        ("AS15.JT", Guidance::Supported),
    ] {
        let p = profile(source);
        assert_eq!(
            (p.role, p.guidance),
            (TargetRole::Surface, guidance),
            "{source}"
        );
        assert_eq!(
            p.supports_boresight(),
            matches!(guidance, Guidance::Infrared | Guidance::Emitter),
            "{source}"
        );
        assert_eq!(
            p.needs_air_ground(),
            matches!(guidance, Guidance::Active | Guidance::Supported),
            "{source}"
        );
    }
    // The air-to-air profiles are unchanged.
    for source in ["AIM9M.JT", "AIM120.JT", "AIM7.JT"] {
        let p = profile(source);
        assert_eq!(p.role, TargetRole::Aircraft);
        assert!(!p.needs_air_ground() && !p.holds_on_angles());
        assert_eq!(p.search_cap(), 5f64.to_radians());
    }
    assert_eq!(profile("AGM88.JT").search_cap(), std::f64::consts::PI);
}

#[test]
fn a_maverick_locks_a_designated_ground_target_and_in_bore_the_strongest_return() {
    // CUED: the A-G page and T give the target; the seeker locks it.
    let mut s = armed("AGM65G.JT", 2);
    ground(&mut s, 0x5000_0001, 300., 20_000., 0x400);
    ground(&mut s, 0x5000_0002, -300., 22_000., 0x400);
    s.command(0, Command::ToggleAirGround, launcher());
    step(&mut s, launcher(), false);
    s.command(0, Command::Designate, launcher());
    assert_eq!(s.own().designated(), Some(0x5000_0001));
    assert_eq!(until_ready(&mut s, launcher(), 120), Readiness::Ready);
    assert_eq!(s.own().launch_mode, LaunchMode::Cued);
    assert_eq!(s.own().mounted.target, Some(0x5000_0001));
    let p = fire(&mut s, launcher());
    assert_eq!(p.target, Some(0x5000_0001));
    assert!(p.guidance.as_ref().unwrap().seeker.acquired);

    // BORE: nothing designated; the hotter of two returns inside the bore
    // circle wins, and the trigger waits for the seeker to hold it. The nose
    // points down at them.
    let dive = Launcher {
        basis: Basis::new(0., -(3000f64 / 20_000.).atan(), 0.),
        ..launcher()
    };
    let mut s = armed("AGM65G.JT", 2);
    ground(&mut s, 0x5000_0001, 100., 20_000., 0x400);
    ground(&mut s, 0x5000_0002, -150., 20_000., 0x400);
    ground(&mut s, 0x5000_0003, 6_000., 20_000., 0x400);
    for t in &mut s.targets {
        t.signature.infrared = match t.id {
            0x5000_0002 => 90.,
            0x5000_0003 => 100.,
            _ => 40.,
        };
    }
    step(&mut s, dive, false);
    assert_eq!(s.own().launch_mode, LaunchMode::Boresight);
    assert_eq!(
        s.own_view().readiness(dive),
        Readiness::RadarAcquiring,
        "the seeker has not held the candidate yet"
    );
    assert_eq!(until_ready(&mut s, dive, 120), Readiness::Ready);
    // The hottest object outside the 5 degree circle is not the pick.
    assert_eq!(s.own().bore_observation.map(|o| o.id), Some(0x5000_0002));
    let p = fire(&mut s, dive);
    assert_eq!(p.target, Some(0x5000_0002));
    assert_eq!(p.guidance.as_ref().unwrap().mode, LaunchMode::Boresight);
    // An empty bore circle has nothing to fire at.
    let mut s = armed("AGM65G.JT", 2);
    step(&mut s, launcher(), false);
    assert_eq!(s.own_view().readiness(launcher()), Readiness::NoTarget);
}

/// The HARM fixture: three ground radars ahead, emitting.
fn harm_scene() -> State {
    let mut s = armed("AGM88.JT", 4);
    ground(&mut s, 0x5000_0001, 500., 30_000., 0x1000);
    ground(&mut s, 0x5000_0002, 4_000., 30_000., 0x1000);
    ground(&mut s, 0x5000_0003, 40_000., 30_000., 0x1000);
    for t in &mut s.targets {
        t.radar_emitting = true;
    }
    s
}

#[test]
fn the_harm_cue_lists_emitters_in_the_cone_and_bore_fires_at_the_candidate() {
    let mut s = harm_scene();
    step(&mut s, launcher(), false);
    assert_eq!(s.own().launch_mode, LaunchMode::Boresight);
    // The cue: emitters inside the 45 degree cone, nearest the bore first;
    // the radar 53 degrees off the nose is not one.
    let cue: Vec<u32> = s
        .own()
        .harm_cue()
        .iter()
        .map(|e| e.observation.id)
        .collect();
    assert_eq!(cue, [0x5000_0001, 0x5000_0002]);
    assert!(s.own().harm_cue().iter().all(|e| e.emitting && !e.painting));
    // Nearest the bore is the candidate; the trigger fires at it once the
    // seeker holds it, with nothing designated.
    assert_eq!(s.own().bore_observation.map(|o| o.id), Some(0x5000_0001));
    assert_eq!(until_ready(&mut s, launcher(), 120), Readiness::Ready);
    assert_eq!(s.own().designated(), None);
    let p = fire(&mut s, launcher());
    assert_eq!(p.target, Some(0x5000_0001));
    let flight = p.guidance.as_ref().unwrap();
    assert!(flight.seeker.acquired && flight.mode == LaunchMode::Boresight);

    // A radar painting the aircraft comes first, wherever it is in the cone.
    let mut s = harm_scene();
    s.set_painted_by(0, [0x5000_0002]);
    step(&mut s, launcher(), false);
    assert_eq!(s.own().bore_observation.map(|o| o.id), Some(0x5000_0002));
    assert!(
        s.own()
            .harm_cue()
            .iter()
            .any(|e| e.observation.id == 0x5000_0002 && e.painting)
    );
    // Nothing emitting: NO EMITTER, and nothing leaves.
    let mut s = harm_scene();
    for t in &mut s.targets {
        t.radar_emitting = false;
    }
    step(&mut s, launcher(), false);
    assert_eq!(s.own_view().readiness(launcher()), Readiness::NoEmitter);
    assert_eq!(Readiness::NoEmitter.label(), "NO EMITTER");
    step(&mut s, launcher(), true);
    assert!(s.projectiles.is_empty());
}

#[test]
fn a_harm_page_designation_turns_the_cue_cued_and_holds_it_there() {
    let mut s = harm_scene();
    let page = with_channel(sensors::Channel::Harm);
    step(&mut s, page, false);
    assert_eq!(s.own().sensors.ground_page(), Some(sensors::Mode::Harm));
    s.command(0, Command::Designate, page);
    s.command(0, Command::Designate, page);
    assert_eq!(s.own().designated(), Some(0x5000_0002));
    step(&mut s, page, false);
    assert_eq!(s.own().launch_mode, LaunchMode::Cued);
    // The seeker mode key cannot take a designated HARM back to BORE.
    s.command(0, Command::ToggleSeekerMode, page);
    assert_eq!(s.own().launch_mode, LaunchMode::Cued);
    assert_eq!(until_ready(&mut s, page, 120), Readiness::Ready);
    let p = fire(&mut s, page);
    assert_eq!(p.target, Some(0x5000_0002));
    assert_eq!(p.guidance.as_ref().unwrap().mode, LaunchMode::Cued);
}

#[test]
fn a_launched_harm_keeps_homing_on_its_radar_after_it_stops_emitting() {
    let mut s = harm_scene();
    assert_eq!(until_ready(&mut s, launcher(), 120), Readiness::Ready);
    let id = fire(&mut s, launcher()).id;
    for _ in 0..60 {
        step(&mut s, launcher(), false);
    }
    // The radar shuts down: the HARM still sees it on angles and homes on.
    for t in &mut s.targets {
        t.radar_emitting = false;
    }
    for _ in 0..120 {
        step(&mut s, launcher(), false);
    }
    let p = s.projectiles.iter().find(|p| p.id == id).unwrap();
    let flight = p.guidance.as_ref().unwrap();
    assert_eq!(flight.seeker.status, Status::Locked);
    assert_eq!(flight.seeker.observation.map(|o| o.id), Some(0x5000_0001));
    // The cue keeps the held radar, dim.
    assert!(
        s.own()
            .harm_cue()
            .iter()
            .all(|e| !e.emitting && e.observation.id == 0x5000_0001)
    );
    // Flown on to the end, it hits.
    for _ in 0..3000 {
        step(&mut s, launcher(), false);
        if s.projectiles.is_empty() {
            break;
        }
    }
    let radar = s.targets.iter().find(|t| t.id == 0x5000_0001).unwrap();
    assert!(radar.hp < radar.initial_hp, "the shut-down radar was hit");
}

#[test]
fn a_surface_seeker_sees_a_stationary_ground_object_without_the_air_notch() {
    let mut s = armed("AGM84A.JT", 3);
    ground(&mut s, 0x5000_0001, 0., 30_000., 0x2000);
    let target = s.targets[0].clone();
    let w = s.own().config.stations[0].weapon.clone();
    let basis = Basis::new(0., -0.1, 0.);
    let at = [0., 3000., 0.];
    assert!(missiles::active_radar_visible(&w, at, basis, &target, 0.));
    // The same seeker under an air profile loses the stationary object in
    // the clutter notch.
    let mut air = w.clone();
    air.source = "AIM120.JT".into();
    assert!(!missiles::active_radar_visible(
        &air, at, basis, &target, 0.
    ));
}

#[test]
fn a_surface_radar_weapon_needs_the_ag_track_at_release() {
    let mut s = armed("AGM84A.JT", 3);
    ground(&mut s, 0x5000_0001, 0., 30_000., 0x2000);
    s.command(0, Command::ToggleAirGround, launcher());
    step(&mut s, launcher(), false);
    s.command(0, Command::Designate, launcher());
    assert_eq!(s.own().designated(), Some(0x5000_0001));
    assert_eq!(until_ready(&mut s, launcher(), 10), Readiness::Ready);
    // The A-G page off: the designation stays, the track is gone.
    s.command(0, Command::ToggleAirGround, launcher());
    step(&mut s, launcher(), false);
    assert_eq!(s.own().designated(), Some(0x5000_0001));
    assert_eq!(s.own_view().readiness(launcher()), Readiness::TrackLost);
    assert_eq!(Readiness::TrackLost.label(), "NO TRACK");
    // Back on, the launch leaves for the point the track gives.
    s.command(0, Command::ToggleAirGround, launcher());
    assert_eq!(until_ready(&mut s, launcher(), 10), Readiness::Ready);
    let p = fire(&mut s, launcher());
    let flight = p.guidance.as_ref().unwrap();
    assert_eq!(p.target, Some(0x5000_0001));
    assert!(flight.last_intercept.is_some());
}

/// An AS-7 in flight on target 1 of two tanks, on the A-G track.
fn sarh_in_flight() -> (State, u32) {
    let mut s = armed("AS7.JT", 3);
    ground(&mut s, 0x5000_0001, 0., 20_000., 0x400);
    ground(&mut s, 0x5000_0002, 1_500., 21_000., 0x400);
    ground(&mut s, 0x5000_0003, -12_000., 9_000., 0x400);
    s.command(0, Command::ToggleAirGround, launcher());
    step(&mut s, launcher(), false);
    // A scope click on the first tank.
    s.command(0, Command::DesignateTarget(0x5000_0001), launcher());
    assert_eq!(s.own().designated(), Some(0x5000_0001));
    assert_eq!(until_ready(&mut s, launcher(), 10), Readiness::Ready);
    let id = fire(&mut s, launcher()).id;
    for _ in 0..60 {
        step(&mut s, launcher(), false);
    }
    (s, id)
}

fn flight(s: &State, id: u32) -> (Option<u32>, bool) {
    let p = s.projectiles.iter().find(|p| p.id == id).unwrap();
    (p.target, p.guidance.as_ref().unwrap().unguided)
}

#[test]
fn an_as7_rides_the_ag_track_and_loses_its_target_for_good_without_it() {
    let (mut s, id) = sarh_in_flight();
    assert_eq!(flight(&s, id), (Some(0x5000_0001), false));
    // The page off: the missile flies on unguided, and stays so.
    s.command(0, Command::ToggleAirGround, launcher());
    step(&mut s, launcher(), false);
    assert_eq!(flight(&s, id), (None, true));
    s.command(0, Command::ToggleAirGround, launcher());
    for _ in 0..30 {
        step(&mut s, launcher(), false);
    }
    assert_eq!(flight(&s, id), (None, true));
}

#[test]
fn a_new_designation_retargets_an_as7_inside_its_cone_and_blinds_it_outside() {
    let (mut s, id) = sarh_in_flight();
    // T to the next tank, a few degrees off the missile's nose.
    s.command(0, Command::Designate, launcher());
    assert_eq!(s.own().designated(), Some(0x5000_0002));
    assert_eq!(flight(&s, id), (Some(0x5000_0002), false));
    for _ in 0..30 {
        step(&mut s, launcher(), false);
    }
    assert_eq!(flight(&s, id), (Some(0x5000_0002), false));
    // Then to one far outside its 45 degree cone: blinded.
    s.command(0, Command::DesignateTarget(0x5000_0003), launcher());
    assert_eq!(s.own().designated(), Some(0x5000_0003));
    assert_eq!(flight(&s, id), (None, true));
}

#[test]
fn the_painting_list_and_the_harm_cue_survive_a_checkpoint() {
    let mut s = harm_scene();
    s.set_painted_by(0, [0x5000_0002]);
    step(&mut s, launcher(), false);
    let models = crate::checkpoint::Models::default();
    let own: Ownship = crate::checkpoint::round_trip(s.own(), &models).unwrap();
    assert_eq!(own.painted_by(), s.own().painted_by());
    assert_eq!(own.harm_cue(), s.own().harm_cue());
    assert!(!own.harm_cue().is_empty());
}
