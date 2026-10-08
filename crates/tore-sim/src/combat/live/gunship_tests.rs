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
fn tracked_mounts_slew_within_limits_and_loss_or_right_targets_block_fire() {
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
    for _ in 0..240 {
        assert!(fired(tick(&mut s, true)).is_empty());
    }
    assert_eq!(
        s.own().gunship.as_ref().unwrap().status[0],
        Readiness::GunArc
    );
    assert!(s.own().gunship.as_ref().unwrap().headings[0] <= -30_f64.to_radians() + 1e-12);
    s.command(OWN, Command::ClearDesignation, launcher());
    assert!(fired(tick(&mut s, true)).is_empty());
    assert_eq!(s.own().gunship.as_ref().unwrap().target, None);
    assert_eq!(
        s.own().gunship.as_ref().unwrap().status[0],
        Readiness::NoTarget
    );
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
fn failed_gun_does_not_block_other_members_and_terrain_los_stops_the_group() {
    let mut s = gunship();
    s.own_mut().gunship.as_mut().unwrap().included = [true; 3];
    s.own_mut().selected = 1;
    s.own_mut().ammo[1] |= 0x8000;
    assert_eq!(fired(tick(&mut s, true)), vec![0, 2]);
    assert_eq!(s.own_view().readiness(launcher()), Readiness::StationFailed);
    tick(&mut s, false);
    let before = s.own().shots;
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
        assert!(fired(events).is_empty());
    }
    assert_eq!(s.own().shots, before);
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
    for _ in 0..240 {
        assert!(fired(tick(&mut s, true)).is_empty());
    }
    assert_eq!(
        s.own().gunship.as_ref().unwrap().status[1],
        Readiness::GunObscured
    );
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
