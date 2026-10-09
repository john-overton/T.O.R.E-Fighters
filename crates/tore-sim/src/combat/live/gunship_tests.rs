use super::*;
use crate::combat::gunship;
use std::f64::consts::{FRAC_PI_2, PI};

fn launcher() -> Launcher {
    Launcher {
        radar_power: true,
        position: [0., 1000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 0.,
        velocity: [0.; 3],
        bay_ready: true,
        radar: true,
        jammer: false,
        alive: true,
        body_present: true,
        controls: Default::default(),
    }
}
fn gunship() -> State {
    let mut old = super::tests::fixture(false);
    old.command(OWN, Command::ReplaceTarget, launcher());
    let mut config = old.own().configuration().clone();
    config.aircraft = AircraftId::Ac130;
    config.sensors.aircraft = AircraftId::Ac130;
    for volume in [
        config.sensors.radar.as_mut().map(|r| &mut r.search),
        config.sensors.visual.as_mut().map(|v| &mut v.search),
    ]
    .into_iter()
    .flatten()
    {
        volume.azimuth_rad = PI;
        volume.elevation_rad = FRAC_PI_2;
    }
    config.sensors.radar.as_mut().unwrap().notch.enabled = false;
    let template = config.stations[0].clone();
    config.stations = gunship::GUNS
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let mut station = template.clone();
            station.weapon.source = name.into();
            station.weapon.flags = 0x880;
            station.weapon.seeker.signature = 0;
            station.weapon.burst.game_rounds_in_burst = 1;
            station.weapon.burst.actual_rounds_per_game = 1;
            station.weapon.burst.game_burst_t = (i + 1) as u8;
            station.count = 40;
            station.mount = gunship::pivot(i);
            station
        })
        .collect();
    config.hardpoint_slots = (0..3).map(Some).collect();
    config.radar_hardpoint = None;
    let mut s = State::new(config, false).unwrap();
    let mut target = old.targets[0].clone();
    target.id = 42;
    target.position = [-1000., 1000., 0.];
    target.velocity = [0.; 3];
    target.hp = 100000;
    target.initial_hp = 100000;
    s.targets.push(target);
    tick(&mut s, false);
    s.command(OWN, Command::DesignateTarget(42), launcher());
    assert_eq!(s.own().designated(), Some(42));
    for _ in 0..120 {
        tick(&mut s, false);
    }
    assert!(
        s.own()
            .gunship
            .as_ref()
            .unwrap()
            .status
            .iter()
            .all(|r| *r == Readiness::Ready),
        "{:?}",
        s.own().gunship.as_ref().unwrap().status
    );
    s
}
const OWN: u32 = 0;
fn tick(s: &mut State, held: bool) -> Vec<Event> {
    s.step(
        &[OwnshipInput {
            aircraft: OWN,
            held,
            launcher: launcher(),
        }],
        |_, _| 0.,
    )
}
fn fired(events: Vec<Event>) -> Vec<usize> {
    events
        .into_iter()
        .filter_map(|e| {
            if let Event::Fired { station, .. } = e {
                Some(station)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn linked_guns_start_together_keep_cadence_and_share_actual_muzzle_pose() {
    let mut s = gunship();
    s.own_mut().gunship.as_mut().unwrap().included = [true; 3];
    let first = fired(tick(&mut s, true));
    assert_eq!(first, vec![0, 1, 2]);
    let mut counts = [1usize; 3];
    for _ in 0..120 {
        for station in fired(tick(&mut s, true)) {
            counts[station] += 1;
        }
    }
    assert!(counts[0] > counts[1] && counts[1] > counts[2], "{counts:?}");
    let pose = s.own().gunship.as_ref().unwrap();
    for slot in 0..3 {
        let expected =
            gunship::muzzle(slot, launcher(), pose.headings[slot], pose.elevations[slot]);
        assert!(expected[0] <= -8.);
        let ray = gunship::direction(launcher(), pose.headings[slot], pose.elevations[slot]);
        assert!(ray[0] < 0.);
        assert!((ray.iter().map(|v| v * v).sum::<f64>() - 1.).abs() < 1e-12);
    }
    let shots = s.own().shots;
    for _ in 0..60 {
        assert!(fired(tick(&mut s, false)).is_empty());
    }
    assert_eq!(s.own().shots, shots);
}
#[test]
fn gun_candidate_changes_membership_empty_station_and_empty_group_are_independent() {
    let mut s = gunship();
    s.command(OWN, Command::NextGunGroup, launcher());
    assert_eq!(s.own().selected, 1);
    assert_eq!(s.own().gunship.as_ref().unwrap().mask(), 1);
    s.command(OWN, Command::ToggleGunGroup, launcher());
    assert_eq!(s.own().gunship.as_ref().unwrap().mask(), 3);
    s.own_mut().ammo[0] = 0;
    assert_eq!(fired(tick(&mut s, true)), vec![1]);
    s.command(OWN, Command::ToggleGunGroup, launcher());
    s.command(OWN, Command::NextGunGroup, launcher());
    s.command(OWN, Command::NextGunGroup, launcher());
    assert_eq!(s.own().selected, 0);
    s.command(OWN, Command::ToggleGunGroup, launcher());
    assert_eq!(s.own().gunship.as_ref().unwrap().mask(), 0);
    assert!(fired(tick(&mut s, true)).is_empty());
    assert_eq!(s.own_view().readiness(launcher()), Readiness::GroupEmpty);
}
#[test]
fn tracked_mounts_slew_within_limits_and_fire_on_at_the_arc_limit() {
    let mut s = gunship();
    let old = s.own().gunship.as_ref().unwrap().headings;
    s.targets[0].position = [-1000., 1000., 1000.];
    tick(&mut s, false);
    let pose = s.own().gunship.as_ref().unwrap();
    for (actual, before) in pose.headings.iter().zip(old) {
        assert!((actual - before).abs() <= 30_f64.to_radians() / 120. + 1e-12);
    }
    assert_eq!(pose.status[0], Readiness::GunSlewing);
    s.targets[0].position = [1000., 1000., 0.];
    // Out of every arc the guns stop at the limit and still fire along it.
    let mut released = 0;
    for _ in 0..240 {
        released += fired(tick(&mut s, true)).len();
    }
    assert!(released > 0);
    assert_eq!(
        s.own().gunship.as_ref().unwrap().status[0],
        Readiness::GunArc
    );
    assert!(s.own().gunship.as_ref().unwrap().headings[0] <= -30_f64.to_radians() + 1e-12);
    // L drops the track and the sight slews freely from where it looked:
    // level out of the right side, where no gun can bear.
    s.command(OWN, Command::ClearDesignation, launcher());
    tick(&mut s, true);
    let group = s.own().gunship.as_ref().unwrap();
    assert_eq!(group.target(), None);
    assert_eq!(group.sight, gunship::Sight::Free);
    assert!((group.look[0] - FRAC_PI_2).abs() < 1e-9, "{:?}", group.look);
    assert_ne!(group.status[0], Readiness::Ready);
}
#[test]
fn gun_group_restart_restores_solo_neutral_and_non_gunship_commands_do_nothing() {
    let mut s = gunship();
    s.own_mut().gunship.as_mut().unwrap().included = [true; 3];
    let restarted = State::new(s.own().configuration().clone(), false).unwrap();
    let pose = restarted.own().gunship.as_ref().unwrap();
    assert_eq!(pose.mask(), 1);
    assert_eq!(pose.normalized_devices(), [-0.5, 0., -0.5, 0., -0.5, 0.]);
    let mut ordinary = super::tests::fixture(false);
    let selected = ordinary.own().selected;
    ordinary.command(OWN, Command::NextGunGroup, launcher());
    ordinary.command(OWN, Command::ToggleGunGroup, launcher());
    assert_eq!(ordinary.own().selected, selected);
    assert!(ordinary.own().gunship.is_none());
}

#[test]
fn failed_gun_does_not_block_other_members_and_terrain_los_does_not_stop_the_group() {
    let mut s = gunship();
    s.own_mut().gunship.as_mut().unwrap().included = [true; 3];
    s.own_mut().selected = 1;
    s.own_mut().ammo[1] |= 0x8000;
    assert_eq!(fired(tick(&mut s, true)), vec![0, 2]);
    assert_eq!(s.own_view().readiness(launcher()), Readiness::StationFailed);
    tick(&mut s, false);
    let before = s.own().shots;
    let mut released = 0;
    for _ in 0..60 {
        let events = s.step(
            &[OwnshipInput {
                aircraft: OWN,
                held: true,
                launcher: launcher(),
            }],
            |x, _| {
                if (-700. ..-300.).contains(&x) {
                    1200.
                } else {
                    0.
                }
            },
        );
        released += fired(events).len();
    }
    // Terrain between the muzzle and the aim point is advisory: the rounds
    // leave along the barrels and meet the ridge.
    assert!(released > 0);
    assert_eq!(s.own().shots, before + released as u32);
}
#[test]
fn self_clearance_rejects_upward_fire_through_wing_or_nacelle_and_accepts_downward_rays() {
    assert!(!gunship::clear_airframe(1, -FRAC_PI_2, 45_f64.to_radians()));
    assert!(!gunship::clear_airframe(1, -FRAC_PI_2, 10_f64.to_radians()));
    for slot in 0..3 {
        assert!(gunship::clear_airframe(
            slot,
            -FRAC_PI_2,
            -20_f64.to_radians()
        ));
    }
    let mut s = gunship();
    s.targets[0].position = [-1000., 1800., 0.];
    s.own_mut().selected = 1;
    s.own_mut().gunship.as_mut().unwrap().included = [false, true, false];
    // The gun may fire while it is still clear on its way up, never once the
    // wing or nacelle is in the line.
    for _ in 0..240 {
        let released = fired(tick(&mut s, true));
        if group(&s).status[1] == Readiness::GunObscured {
            assert!(released.is_empty());
        }
    }
    assert_eq!(group(&s).status[1], Readiness::GunObscured);
    let shots = s.own().shots;
    for _ in 0..120 {
        assert!(fired(tick(&mut s, true)).is_empty());
    }
    assert_eq!(s.own().shots, shots);
}

/// Exact checkpoints carry the AC-130's mounts: a combat state saved while
/// the guns slew to a moved target restores with the same angles, linked
/// membership and readiness, and fires on identically.
#[test]
fn a_gunship_checkpoint_restores_mid_slew_and_fires_on_identically() {
    use crate::checkpoint::{Models, from_bytes, to_bytes};
    fn drained(mut s: State) -> State {
        s.take_device_notes();
        s.take_decoy_rolls();
        s.ledger.take_outcomes();
        s
    }
    let mut s = gunship();
    s.command(OWN, Command::NextGunGroup, launcher());
    s.command(OWN, Command::ToggleGunGroup, launcher());
    s.targets[0].position = [-800., 1000., 700.];
    for _ in 0..3 {
        tick(&mut s, false);
    }
    let before = s.own().gunship.clone().unwrap();
    assert!(before.status.contains(&Readiness::GunSlewing), "{before:?}");
    assert!(before.included.iter().filter(|on| **on).count() >= 2);
    let models = Models::default();
    let mut s = drained(s);
    let coded = to_bytes(&s, &models).unwrap();
    let mut copy: State = from_bytes(&coded, &models).unwrap();
    assert_eq!(copy.own().gunship.as_ref(), Some(&before));
    for n in 0..240 {
        let held = (10..200).contains(&n);
        let a = tick(&mut s, held);
        let b = tick(&mut copy, held);
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "tick {n}");
    }
    assert_eq!(
        to_bytes(&drained(s), &models).unwrap(),
        to_bytes(&drained(copy), &models).unwrap()
    );
}

// ----- The gunsight (plan slice S1) ------------------------------------------

use gunship::{DEFAULT_LOOK, RETURN_PER_TICK, Sight, SightObject};

fn group(s: &State) -> &gunship::State {
    s.own().gunship.as_ref().unwrap()
}
fn group_mut(s: &mut State) -> &mut gunship::State {
    s.own_mut().gunship.as_mut().unwrap()
}
/// The fixture with nothing held, looking along the default view.
fn free_gunship() -> State {
    let mut s = gunship();
    s.command(OWN, Command::ClearDesignation, launcher());
    let g = group_mut(&mut s);
    assert_eq!(g.sight, Sight::Free);
    g.look = DEFAULT_LOOK;
    g.returning = false;
    s
}
fn tick_over(s: &mut State, ground: impl Fn(f64, f64) -> f64 + Sync) -> Vec<Event> {
    s.step(
        &[OwnshipInput {
            aircraft: OWN,
            held: false,
            launcher: launcher(),
        }],
        ground,
    )
}
fn angular_distance(look: [f64; 2]) -> f64 {
    let dh = (DEFAULT_LOOK[0] - look[0] + PI).rem_euclid(2. * PI) - PI;
    dh.hypot(DEFAULT_LOOK[1] - look[1])
}
fn sight_object(id: u32, position: Vector) -> SightObject {
    SightObject {
        id,
        position,
        velocity: [0.; 3],
        alive: true,
        airborne: false,
        friendly: false,
    }
}

#[test]
fn the_default_view_is_abeam_left_and_down_inside_every_arc_and_clear_of_the_airframe() {
    let config = gunship().own().configuration().clone();
    let fresh = gunship::State::new(&config).unwrap();
    assert_eq!(fresh.sight, Sight::Free);
    assert_eq!(fresh.look, DEFAULT_LOOK);
    assert_eq!(fresh.zoom(), gunship::DEFAULT_ZOOM);
    assert_eq!(DEFAULT_LOOK[0], -FRAC_PI_2);
    assert!((DEFAULT_LOOK[1] + 25_f64.to_radians()).abs() < 1e-15);
    for slot in 0..3 {
        assert!(gunship::clear_airframe(
            slot,
            DEFAULT_LOOK[0],
            DEFAULT_LOOK[1]
        ));
    }
    // Inside every arc: the guns settle on the default point and are ready.
    let mut s = free_gunship();
    group_mut(&mut s).included = [true; 3];
    for _ in 0..120 {
        tick(&mut s, false);
    }
    let g = group(&s);
    assert_eq!(g.status, [Readiness::Ready; 3], "{g:?}");
    for slot in 0..3 {
        assert!((g.headings[slot] + FRAC_PI_2).abs() < 5_f64.to_radians());
        assert!((g.elevations[slot] + 25_f64.to_radians()).abs() < 5_f64.to_radians());
    }
}

#[test]
fn guns_start_neutral_and_reach_the_default_point_within_a_second() {
    let config = gunship().own().configuration().clone();
    let mut s = State::new(config, false).unwrap();
    assert_eq!(group(&s).elevations, [0.; 3]);
    for _ in 0..120 {
        tick(&mut s, false);
    }
    let g = group(&s);
    assert_eq!(g.sight, Sight::Free);
    let candidate = g.slot(s.own().selected).unwrap_or(0);
    assert_eq!(g.status[candidate], Readiness::Ready, "{g:?}");
}

#[test]
fn slew_integrates_from_deflection_and_zoom_exactly_and_repeatably() {
    let mut a = free_gunship();
    let mut b = free_gunship();
    for s in [&mut a, &mut b] {
        s.set_sight_input(OWN, [127, 0], 3);
        for _ in 0..120 {
            tick(s, false);
        }
    }
    assert_eq!(group(&a).look, group(&b).look);
    // A second at full deflection and step 3 turns 0.75 of 7.5 degrees
    // across the screen: 5.625 / cos 25 degrees of heading.
    let mut expected = DEFAULT_LOOK;
    for _ in 0..120 {
        expected = gunship::slewed(expected, [1., 0.], 3);
    }
    assert_eq!(group(&a).look, expected);
    let turned = (group(&a).look[0] - DEFAULT_LOOK[0]).to_degrees();
    assert!(
        (turned - 5.625 / 25_f64.to_radians().cos()).abs() < 1e-9,
        "{turned}"
    );
    assert_eq!(group(&a).look[1], DEFAULT_LOOK[1]);
    // Step 1 slews 22.5 degrees a second, step 6 under one degree.
    let mut wide = free_gunship();
    wide.set_sight_input(OWN, [0, 127], 1);
    for _ in 0..120 {
        tick(&mut wide, false);
    }
    let raised = (group(&wide).look[1] - DEFAULT_LOOK[1]).to_degrees();
    assert!((raised - 22.5).abs() < 1e-9, "{raised}");
    assert!((gunship::field_of_view(6).to_degrees() - 0.9375).abs() < 1e-12);
    assert_eq!(gunship::field_of_view(0), gunship::field_of_view(3));
    // Heading wraps through a full turn; elevation stops at 89 degrees.
    let mut look = [PI - 0.001, 1.5532];
    look = gunship::slewed(look, [1., 1.], 1);
    assert!(look[0] < 0., "{look:?}");
    assert_eq!(look[1], gunship::LOOK_ELEVATION_LIMIT);
}

#[test]
fn the_free_aim_point_is_where_the_line_of_sight_meets_the_ground_or_the_gun_range() {
    let mut s = free_gunship();
    tick(&mut s, false);
    let aim = group(&s).aim.unwrap();
    // 1,000 feet up, 25 degrees down, out of the left side.
    assert!(aim[1].abs() < 0.01, "{aim:?}");
    assert!(
        (aim[0] + 1000. / 25_f64.to_radians().tan()).abs() < 0.1,
        "{aim:?}"
    );
    assert!(aim[2].abs() < 1e-6);
    // Rising ground: the aim lies on it.
    let slope = |x: f64, _: f64| (-x * 0.3).max(0.);
    tick_over(&mut s, slope);
    let aim = group(&s).aim.unwrap();
    assert!((aim[1] - slope(aim[0], aim[2])).abs() < 0.01, "{aim:?}");
    assert!(aim[0] > -1000. / 25_f64.to_radians().tan());
    // Looking above the horizon: a point at the guns' range.
    group_mut(&mut s).look = [-FRAC_PI_2, 10_f64.to_radians()];
    tick(&mut s, false);
    let aim = group(&s).aim.unwrap();
    let origin = launcher().position;
    let range = (0..3)
        .map(|i| (aim[i] - origin[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    assert!((range - 10_000.).abs() < 1e-6, "{range}");
}

#[test]
fn pins_hold_a_ground_point_move_with_slew_and_l_drops_then_travels_home() {
    let mut s = free_gunship();
    s.command(OWN, Command::SightPinGround, launcher());
    tick(&mut s, false);
    let Sight::Pinned(pin) = group(&s).sight else {
        panic!("{:?}", group(&s).sight)
    };
    assert!(pin[1].abs() < 0.01 && pin[0] < -2000., "{pin:?}");
    // The aircraft flies on; the pin stays put and the look follows it.
    let moved = |s: &mut State| {
        let mut l = launcher();
        l.position[2] = 500.;
        s.step(
            &[OwnshipInput {
                aircraft: OWN,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        )
    };
    moved(&mut s);
    assert_eq!(group(&s).sight, Sight::Pinned(pin));
    assert_eq!(group(&s).aim, Some(pin));
    assert!(group(&s).look[0] < -FRAC_PI_2, "{:?}", group(&s).look);
    // Slewing moves the pin; Backslash on bare ground re-pins.
    s.set_sight_input(OWN, [0, -127], 3);
    tick(&mut s, false);
    s.set_sight_input(OWN, [0, 0], 3);
    let Sight::Pinned(slewed) = group(&s).sight else {
        panic!()
    };
    assert!(slewed[0] > pin[0], "{slewed:?} {pin:?}");
    s.command(OWN, Command::SightDesignate, launcher());
    tick(&mut s, false);
    assert!(matches!(group(&s).sight, Sight::Pinned(_)));
    // L drops the pin and keeps the view.
    let look = group(&s).look;
    s.command(OWN, Command::ClearDesignation, launcher());
    tick(&mut s, false);
    assert_eq!(group(&s).sight, Sight::Free);
    assert_eq!(group(&s).look, look);
    assert!(!group(&s).returning);
    // L again travels back to the default view at the slew speed: never a
    // snap, and exactly home at the end.
    group_mut(&mut s).look = [-20_f64.to_radians(), -50_f64.to_radians()];
    s.command(OWN, Command::ClearDesignation, launcher());
    assert!(group(&s).returning);
    let mut ticks = 0;
    let mut before = angular_distance(group(&s).look);
    while group(&s).returning {
        tick(&mut s, false);
        let now = angular_distance(group(&s).look);
        assert!(before - now <= RETURN_PER_TICK + 1e-12, "{before} {now}");
        before = now;
        ticks += 1;
        assert!(ticks < 1000);
    }
    assert_eq!(group(&s).look, DEFAULT_LOOK);
    let expected = (angular_distance([-20_f64.to_radians(), -50_f64.to_radians()])
        / RETURN_PER_TICK)
        .ceil() as usize;
    assert_eq!(ticks, expected);
    // About 75 degrees at 22.5 degrees a second.
    assert!((380..420).contains(&ticks), "{ticks}");
    // A slew on the way home takes over.
    group_mut(&mut s).look = [0., 0.];
    s.command(OWN, Command::ClearDesignation, launcher());
    tick(&mut s, false);
    s.set_sight_input(OWN, [10, 0], 3);
    tick(&mut s, false);
    assert!(!group(&s).returning);
}

#[test]
fn a_pin_from_a_track_holds_the_ground_under_it_and_the_radar_selection_follows() {
    let mut s = gunship();
    s.targets[0].position = [-1500., 0., 0.];
    tick(&mut s, false);
    assert_eq!(group(&s).sight, Sight::Tracked(42));
    // Backslash while tracking keeps the track.
    s.command(OWN, Command::SightDesignate, launcher());
    tick(&mut s, false);
    assert_eq!(group(&s).sight, Sight::Tracked(42));
    // Slewing while tracking only says L drops the target, once per push.
    s.set_sight_input(OWN, [127, 0], 3);
    tick(&mut s, false);
    let notice = group(&s).notice.unwrap();
    assert_eq!(notice.notice, gunship::Notice::DropToSlew);
    tick(&mut s, false);
    assert_eq!(group(&s).notice, Some(notice));
    assert_eq!(group(&s).sight, Sight::Tracked(42));
    s.set_sight_input(OWN, [0, 0], 3);
    s.command(OWN, Command::SightPinGround, launcher());
    tick(&mut s, false);
    let Sight::Pinned(pin) = group(&s).sight else {
        panic!("{:?}", group(&s).sight)
    };
    assert!(
        (pin[0] + 1500.).abs() < 30. && pin[1].abs() < 0.01,
        "{pin:?}"
    );
    assert_eq!(s.own().designated(), None);
    assert!(s.own_view().display_target().is_none());
}

#[test]
fn backslash_picks_inside_the_pipper_skipping_friendlies_and_masked_objects() {
    let origin = [0., 1000., 0.];
    let line = [-1., 0., 0.];
    let flat = |_: f64, _: f64| 0.;
    // The radius at step 3 is 9/114 of 7.5 degrees, about 0.59 degrees.
    let radius = 9. / 114. * 7.5_f64.to_radians();
    let off = |range: f64, angle: f64| [-range * angle.cos(), 1000. + range * angle.sin(), 0.];
    let objects = [
        sight_object(7, off(5000., radius * 1.5)),
        sight_object(5, off(5000., radius * 0.5)),
        sight_object(4, off(3000., radius * 0.5)),
        sight_object(3, off(3000., radius * 0.5)),
    ];
    // Same angle: nearest wins, then lowest id.
    assert_eq!(gunship::pick(origin, line, 3, &objects, &flat), Some(3));
    // Outside the radius at step 3, inside it at step 1.
    assert_eq!(gunship::pick(origin, line, 3, &objects[..1], &flat), None);
    assert_eq!(
        gunship::pick(origin, line, 1, &objects[..1], &flat),
        Some(7)
    );
    // Friendlies and destroyed objects are skipped.
    let mut friendly = objects;
    friendly[2].friendly = true;
    friendly[3].alive = false;
    assert_eq!(gunship::pick(origin, line, 3, &friendly, &flat), Some(5));
    // A ridge masks the near pair; the far one is behind it too.
    let ridge = |x: f64, _: f64| {
        if (-2500. ..-2000.).contains(&x) {
            2000.
        } else {
            0.
        }
    };
    assert_eq!(gunship::pick(origin, line, 3, &objects, &ridge), None);
    // Closest to the line beats nearest.
    let near_line = [
        sight_object(9, off(9000., radius * 0.1)),
        sight_object(2, off(1000., radius * 0.9)),
    ];
    assert_eq!(gunship::pick(origin, line, 3, &near_line, &flat), Some(9));
    // A ground object on flat ground is not masked by the ground it is on.
    let ground_point = [-1000. / 25_f64.to_radians().tan(), 0., 0.];
    let down = gunship::local_direction(DEFAULT_LOOK[0], DEFAULT_LOOK[1]);
    let down = [down[0], down[1], down[2]];
    assert_eq!(
        gunship::pick(origin, down, 3, &[sight_object(11, ground_point)], &flat),
        Some(11)
    );
}

#[test]
fn a_ground_object_is_tracked_at_30_nmi_trains_the_guns_geometrically_and_its_death_pins() {
    let mut s = free_gunship();
    let mut object = s.targets[0].clone();
    object.id = 77;
    object.airborne = false;
    object.velocity = [0.; 3];
    let range = 30. * 6076.12;
    object.position = [-range, 0., 0.];
    s.targets.push(object);
    // A friendly beside it on the same line is skipped.
    let mut friend = s.targets[1].clone();
    friend.id = 78;
    friend.position = [-range, 0., 1.];
    s.targets.push(friend);
    s.own_mut().friendlies.insert(78);
    let l = launcher();
    group_mut(&mut s).look = gunship::body_angles(l, [-range, -1000., 0.]);
    s.command(OWN, Command::SightDesignate, l);
    tick(&mut s, false);
    assert_eq!(group(&s).sight, Sight::Tracked(77));
    assert_eq!(s.own_view().display_target().map(|t| t.id), Some(77));
    // Far beyond any solution: MAX RANGE, yet the guns still come round
    // toward it, here level and abeam.
    group_mut(&mut s).headings = [-1.2; 3];
    for _ in 0..120 {
        tick(&mut s, false);
    }
    let g = group(&s);
    let candidate = g.slot(s.own().selected).unwrap();
    assert_eq!(g.status[candidate], Readiness::MaximumRange);
    assert!((g.headings[candidate] + FRAC_PI_2).abs() < 0.01, "{g:?}");
    assert!(g.elevations[candidate].abs() < 0.01, "{g:?}");
    // The object is destroyed: the sight pins the ground where it was.
    let index = s.targets.iter().position(|t| t.id == 77).unwrap();
    s.targets[index].hp = 0;
    tick(&mut s, false);
    let Sight::Pinned(pin) = group(&s).sight else {
        panic!("{:?}", group(&s).sight)
    };
    assert!(
        (pin[0] + range).abs() < 200. && pin[1].abs() < 0.01,
        "{pin:?}"
    );
    // Removed outright works the same way.
    let mut s = gunship();
    s.targets[0].position = [-1500., 0., 0.];
    tick(&mut s, false);
    s.targets.retain(|t| t.id != 42);
    tick(&mut s, false);
    assert!(
        matches!(group(&s).sight, Sight::Pinned(_)),
        "{:?}",
        group(&s).sight
    );
}

#[test]
fn guns_follow_free_and_pinned_points_and_stop_at_each_arc() {
    let mut s = free_gunship();
    group_mut(&mut s).included = [true; 3];
    // Forward and down: past every gun's forward limit.
    group_mut(&mut s).look = [-10_f64.to_radians(), -25_f64.to_radians()];
    for _ in 0..360 {
        tick(&mut s, false);
    }
    let g = group(&s);
    for (slot, limit) in [-30_f64, -45., -65.].into_iter().enumerate() {
        assert!(
            (g.headings[slot] - limit.to_radians()).abs() < 1e-9,
            "{slot}: {g:?}"
        );
        assert_eq!(g.status[slot], Readiness::GunArc);
    }
    // Pinned aft and down: inside C_25's arc only.
    group_mut(&mut s).look = [-140_f64.to_radians(), -30_f64.to_radians()];
    s.command(OWN, Command::SightPinGround, launcher());
    for _ in 0..600 {
        tick(&mut s, false);
    }
    let g = group(&s);
    assert!(matches!(g.sight, Sight::Pinned(_)));
    assert_eq!(g.status[0], Readiness::Ready, "{g:?}");
    assert_eq!(g.status[1], Readiness::GunArc);
    assert_eq!(g.status[2], Readiness::GunArc);
    assert!((g.headings[1] + 135_f64.to_radians()).abs() < 1e-9);
    assert!((g.headings[2] + 115_f64.to_radians()).abs() < 1e-9);
}

#[test]
fn terrain_between_muzzle_and_aim_reads_terrain_mask_not_line_of_fire() {
    let mut s = gunship();
    let ridge = |x: f64, _: f64| {
        if (-700. ..-300.).contains(&x) {
            1200.
        } else {
            0.
        }
    };
    // Track a radar designation behind a ridge, as the guns were ready.
    tick_over(&mut s, ridge);
    let g = group(&s);
    let candidate = g.slot(s.own().selected).unwrap();
    assert_eq!(g.status[candidate], Readiness::TerrainMask);
    assert_eq!(Readiness::TerrainMask.label(), "TERRAIN MASK");
    assert_eq!(Readiness::GunObscured.label(), "NO LINE OF FIRE");
}

#[test]
fn the_ac130_keeps_selections_as_easy_targeting_does_with_the_cheat_off() {
    let mut s = gunship();
    assert!(!s.cheats.easy_targeting);
    assert!(s.own().sensors.keep_selection);
    // The radar goes off: the selection and the sight's track remain.
    let mut dark = launcher();
    dark.radar = false;
    dark.radar_power = false;
    for _ in 0..30 {
        s.step(
            &[OwnshipInput {
                aircraft: OWN,
                held: false,
                launcher: dark,
            }],
            |_, _| 0.,
        );
    }
    assert_eq!(s.own().designated(), Some(42));
    assert_eq!(group(&s).sight, Sight::Tracked(42));
    assert_eq!(s.own_view().display_target().map(|t| t.id), Some(42));
    // Other aircraft are unchanged.
    let mut ordinary = super::tests::fixture(false);
    tick(&mut ordinary, false);
    assert!(!ordinary.own().sensors.keep_selection);
    ordinary.command(OWN, Command::SightDesignate, launcher());
    ordinary.command(OWN, Command::SightPinGround, launcher());
    ordinary.set_sight_input(OWN, [127, 127], 6);
    tick(&mut ordinary, false);
    assert!(ordinary.own().gunship.is_none());
}

#[test]
fn a_pin_needs_ground_and_says_so_when_there_is_none() {
    let mut s = free_gunship();
    group_mut(&mut s).look = [-FRAC_PI_2, 20_f64.to_radians()];
    s.command(OWN, Command::SightPinGround, launcher());
    tick(&mut s, false);
    let g = group(&s);
    assert_eq!(g.sight, Sight::Free);
    assert_eq!(
        g.notice.map(|n| n.notice),
        Some(gunship::Notice::NoGroundPoint)
    );
}

/// Checkpoints restore the sight mid-slew, on the way home, pinned and
/// tracked, and every copy steps on bit-identically.
#[test]
fn a_gunsight_checkpoint_restores_mid_slew_pinned_and_tracked_and_steps_on_identically() {
    use crate::checkpoint::{Models, from_bytes, to_bytes};
    fn drained(mut s: State) -> State {
        s.take_device_notes();
        s.take_decoy_rolls();
        s.ledger.take_outcomes();
        s
    }
    let slewing = {
        let mut s = free_gunship();
        s.set_sight_input(OWN, [90, -40], 4);
        for _ in 0..5 {
            tick(&mut s, false);
        }
        s
    };
    let returning = {
        let mut s = free_gunship();
        group_mut(&mut s).look = [-0.3, 0.1];
        s.command(OWN, Command::ClearDesignation, launcher());
        for _ in 0..5 {
            tick(&mut s, false);
        }
        assert!(group(&s).returning);
        s
    };
    let pinned = {
        let mut s = free_gunship();
        s.command(OWN, Command::SightPinGround, launcher());
        s.set_sight_input(OWN, [30, 0], 2);
        for _ in 0..3 {
            tick(&mut s, false);
        }
        assert!(matches!(group(&s).sight, Sight::Pinned(_)));
        s
    };
    let tracked = {
        let mut s = gunship();
        s.targets[0].position = [-1400., 0., 300.];
        s.command(OWN, Command::SightPinGround, launcher());
        tick(&mut s, false);
        s.command(OWN, Command::Designate, launcher());
        tick(&mut s, false);
        s
    };
    let models = Models::default();
    for (name, s) in [
        ("slewing", slewing),
        ("returning", returning),
        ("pinned", pinned),
        ("tracked", tracked),
    ] {
        let before = group(&s).clone();
        let mut s = drained(s);
        let coded = to_bytes(&s, &models).unwrap();
        let mut copy: State = from_bytes(&coded, &models).unwrap();
        assert_eq!(copy.own().gunship.as_ref(), Some(&before), "{name}");
        for n in 0..240 {
            let held = (10..200).contains(&n);
            if n == 100 {
                for state in [&mut s, &mut copy] {
                    state.set_sight_input(OWN, [-60, 20], 5);
                }
            }
            let a = tick(&mut s, held);
            let b = tick(&mut copy, held);
            assert_eq!(format!("{a:?}"), format!("{b:?}"), "{name} tick {n}");
            assert_eq!(group(&s), group(&copy), "{name} tick {n}");
        }
        assert_eq!(
            to_bytes(&drained(s), &models).unwrap(),
            to_bytes(&drained(copy), &models).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn the_pipper_is_kept_for_linked_guns_and_the_candidate_and_sits_on_a_trained_aim() {
    use crate::combat::gunship_impact::Impact;
    let mut s = free_gunship();
    group_mut(&mut s).included = [true, false, true];
    s.own_mut().selected = group(&s).stations[0].unwrap();
    for _ in 0..120 {
        tick(&mut s, false);
    }
    let g = group(&s);
    assert_eq!(g.impacts_tick, s.tick - 1);
    assert!(g.impacts[1].is_none(), "{g:?}");
    let aim = g.aim.unwrap();
    for slot in [0, 2] {
        let Some(Impact::Ground { point, .. }) = g.impacts[slot] else {
            panic!("{slot}: {:?}", g.impacts[slot])
        };
        let miss = (0..3)
            .map(|i| (point[i] - aim[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!(miss < 60., "{slot}: {miss} ft from the aim point");
    }
    // A tracked aircraft is led: the pipper is an air intercept.
    let mut s = gunship();
    for _ in 0..10 {
        tick(&mut s, false);
    }
    let g = group(&s);
    let candidate = g.slot(s.own().selected).unwrap();
    assert!(
        matches!(g.impacts[candidate], Some(Impact::Air { .. })),
        "{:?}",
        g.impacts
    );
}

/// Holds the trigger for `ticks` over `ground`, returning the station of
/// every round released.
fn hold_fire(s: &mut State, ticks: usize, ground: impl Fn(f64, f64) -> f64 + Sync) -> Vec<usize> {
    let mut stations = Vec::new();
    for _ in 0..ticks {
        stations.extend(fired(s.step(
            &[OwnshipInput {
                aircraft: OWN,
                held: true,
                launcher: launcher(),
            }],
            &ground,
        )));
    }
    stations
}
/// Every linked gun released rounds, and its rounds are in flight.
fn assert_all_fire(stations: &[usize]) {
    for slot in 0..3 {
        assert!(stations.contains(&slot), "gun {slot} never fired");
    }
}
fn flat(_: f64, _: f64) -> f64 {
    0.
}

#[test]
fn the_trigger_fires_with_nothing_held_along_the_default_view() {
    let mut s = free_gunship();
    group_mut(&mut s).included = [true; 3];
    assert_eq!(group(&s).sight, Sight::Free);
    let shots = s.own().shots;
    let stations = hold_fire(&mut s, 120, flat);
    assert_all_fire(&stations);
    assert_eq!(s.own().shots, shots + stations.len() as u32);
    assert_eq!(s.own().gunship.as_ref().unwrap().target(), None);
    // The rounds leave along the actual barrels, not at any target.
    assert!(!s.projectiles.is_empty());
}

#[test]
fn the_trigger_fires_at_a_pin_and_after_the_target_is_lost() {
    let mut s = free_gunship();
    group_mut(&mut s).included = [true; 3];
    s.command(OWN, Command::SightPinGround, launcher());
    tick(&mut s, false);
    assert!(matches!(group(&s).sight, Sight::Pinned(_)));
    assert_all_fire(&hold_fire(&mut s, 120, flat));
    // Track the radar target, then destroy it: the pin takes over and the
    // guns keep firing.
    let mut s = gunship();
    group_mut(&mut s).included = [true; 3];
    s.targets[0].hp = 0;
    assert_all_fire(&hold_fire(&mut s, 120, flat));
}

#[test]
fn the_trigger_fires_out_of_arc_at_the_limit() {
    let mut s = free_gunship();
    group_mut(&mut s).included = [true; 3];
    group_mut(&mut s).look = [-10_f64.to_radians(), -25_f64.to_radians()];
    for _ in 0..360 {
        tick(&mut s, false);
    }
    assert_eq!(group(&s).status, [Readiness::GunArc; 3]);
    let before = group(&s).headings;
    assert_all_fire(&hold_fire(&mut s, 120, flat));
    assert_eq!(group(&s).status, [Readiness::GunArc; 3]);
    assert_eq!(group(&s).headings, before);
}

#[test]
fn the_trigger_fires_at_max_range_and_while_slewing() {
    let mut s = free_gunship();
    group_mut(&mut s).included = [true; 3];
    let mut object = s.targets[0].clone();
    object.id = 77;
    object.airborne = false;
    object.velocity = [0.; 3];
    let range = 30. * 6076.12;
    object.position = [-range, 0., 0.];
    s.targets.push(object);
    let l = launcher();
    group_mut(&mut s).look = gunship::body_angles(l, [-range, -1000., 0.]);
    s.command(OWN, Command::SightDesignate, l);
    tick(&mut s, false);
    assert_eq!(group(&s).sight, Sight::Tracked(77));
    let stations = hold_fire(&mut s, 240, flat);
    assert_all_fire(&stations);
    let g = group(&s);
    let candidate = g.slot(s.own().selected).unwrap();
    assert_eq!(g.status[candidate], Readiness::MaximumRange);
    // Slewing: a new pin far from where the guns point releases at once.
    let mut s = free_gunship();
    group_mut(&mut s).included = [true; 3];
    for _ in 0..60 {
        tick(&mut s, false);
    }
    group_mut(&mut s).look = [-60_f64.to_radians(), -20_f64.to_radians()];
    let mut slewing_fire = 0;
    for _ in 0..30 {
        let released = hold_fire(&mut s, 1, flat);
        if group(&s).status.contains(&Readiness::GunSlewing) {
            slewing_fire += released.len();
        }
    }
    assert!(slewing_fire > 0);
}

#[test]
fn the_trigger_fires_through_terrain_mask_and_the_rounds_meet_the_ridge() {
    let mut s = gunship();
    group_mut(&mut s).included = [true; 3];
    let ridge = |x: f64, _: f64| {
        if (-700. ..-300.).contains(&x) {
            1200.
        } else {
            0.
        }
    };
    tick_over(&mut s, ridge);
    let candidate = group(&s).slot(s.own().selected).unwrap();
    assert_eq!(group(&s).status[candidate], Readiness::TerrainMask);
    let stations = hold_fire(&mut s, 120, ridge);
    assert!(stations.contains(&candidate));
    assert!(!s.projectiles.is_empty() || s.own().shots > 0);
}

#[test]
fn safe_empty_failed_and_empty_groups_still_block_the_trigger() {
    // SAFE.
    let mut s = free_gunship();
    group_mut(&mut s).included = [true; 3];
    s.own_mut().armed = false;
    assert!(hold_fire(&mut s, 60, flat).is_empty());
    // EMPTY: every linked magazine dry.
    let mut s = free_gunship();
    group_mut(&mut s).included = [true; 3];
    for i in 0..3 {
        s.own_mut().ammo[i] = 0;
    }
    assert!(hold_fire(&mut s, 60, flat).is_empty());
    // STATION FAILED on the one linked gun.
    let mut s = free_gunship();
    group_mut(&mut s).included = [false, true, false];
    s.own_mut().selected = 1;
    s.own_mut().ammo[1] |= 0x8000;
    assert!(hold_fire(&mut s, 60, flat).is_empty());
    // GROUP EMPTY: no gun linked or selected into the group.
    let mut s = free_gunship();
    group_mut(&mut s).included = [false; 3];
    s.own_mut().selected = 3;
    assert!(hold_fire(&mut s, 60, flat).is_empty());
}

#[test]
fn the_airframe_blocks_even_when_the_aim_is_out_of_range_or_arc() {
    let mut s = free_gunship();
    s.own_mut().selected = 1;
    group_mut(&mut s).included = [false, true, false];
    // Level-up sky aim, abeam: the nacelle is in the way of gun 1, and the
    // sky point is also beyond any arc.
    group_mut(&mut s).look = [-FRAC_PI_2, 30_f64.to_radians()];
    for _ in 0..360 {
        tick(&mut s, false);
    }
    assert_eq!(group(&s).status[1], Readiness::GunObscured);
    let shots = s.own().shots;
    assert!(hold_fire(&mut s, 120, flat).is_empty());
    assert_eq!(s.own().shots, shots);
}

#[test]
fn only_the_hard_blocks_stop_a_gun() {
    use Readiness::*;
    for blocked in [
        Safe,
        LauncherLost,
        StationFailed,
        Empty,
        Capacity,
        GroupEmpty,
        GunObscured,
    ] {
        assert!(!blocked.gun_may_fire(), "{blocked:?}");
    }
    for advisory in [
        Ready,
        NoTarget,
        TargetDestroyed,
        MinimumRange,
        MaximumRange,
        GunArc,
        GunSlewing,
        TerrainMask,
    ] {
        assert!(advisory.gun_may_fire(), "{advisory:?}");
    }
}
