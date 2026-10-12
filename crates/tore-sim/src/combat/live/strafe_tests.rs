//! An AI strafing round against a ground vehicle (slice ST-LV, the A-10's
//! GAU-8 that did not hurt a 150 hit point vehicle in two bursts): a round
//! that passes through the vehicle's volume damages it by the record's class
//! damage, and one that passes beside it does nothing. The misses in the
//! driver runs were the burst's aim, not the hit test or the damage.
use super::tests::{fixture, target};
use super::*;

const VEHICLE: u32 = 0x5000_0003;
const STRIKER: u32 = 7;

/// The GAU-8 as the record has it: 80 against class 3 (armour, category
/// 0x400), two rounds a game round, no fuze or splash, 2,933 ft/s.
fn gau8() -> Weapon {
    let mut w = fixture(false).own().configuration().stations[0]
        .weapon
        .clone();
    w.source = "GAU8.JT".into();
    w.damage.by_class = [80, 16, 80, 80, 80];
    w.damage.fuze_radius = 0;
    w.damage.collateral_radius = 0;
    w.damage.fuze_arm_t = 0;
    w.burst.actual_rounds_per_game = 2;
    w.burst.game_rounds_in_burst = 4;
    w.movement.minimum_speed = 2933;
    w.movement.maximum_speed = 2933;
    w.movement.initial_speed = 2933;
    w.seeker.signature = 0;
    w
}

/// A world with a 150 hit point armoured vehicle standing at the origin,
/// its 15 ft volume as the column's lighter vehicle has it.
fn world() -> State {
    let mut s = fixture(false);
    s.targets.retain(|t| t.id == 0);
    let mut vehicle = target(VEHICLE, [0., 8.5, 0.], 150, 0x400);
    vehicle.role = TargetRole::Surface;
    vehicle.aircraft = None;
    vehicle.airborne = false;
    vehicle.on_ground = true;
    vehicle.radius = 15.;
    s.targets.push(vehicle);
    s
}

/// One round fired by the AI striker from 400 ft back along +z, 18 degrees
/// down, passing `beside` ft to the side of the vehicle's centre.
fn round(beside: f64, index: u8) -> Projectile {
    let dive = 18_f64.to_radians();
    let direction = [0., -dive.sin(), dive.cos()];
    let back = 400.;
    let position = [beside, 8.5 + back * dive.tan(), -back];
    Projectile {
        id: 1 << 24 | u32::from(index),
        owner: STRIKER,
        weapon: Some(gau8()),
        guidance: None,
        motion: None,
        guidance_ticks: None,
        age: 0,
        incoming: None,
        station: 0,
        position,
        previous: position,
        direction,
        speed_f8: 2933 * 256,
        launched_t: 0,
        target: None,
        fall: FallState::default(),
        gun_round: Some(index % 2),
        tracer: false,
    }
}

/// The fixture's own aircraft, far above and away.
fn far() -> Launcher {
    Launcher {
        position: [200_000., 40_000., -200_000.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 700.,
        velocity: [0., 0., 700.],
        bay_ready: true,
        radar_power: false,
        radar: false,
        jammer: false,
        alive: true,
        body_present: true,
        controls: sensors::Controls::default(),
    }
}

fn fly(s: &mut State) {
    for _ in 0..120 {
        if s.projectiles.is_empty() {
            return;
        }
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: far(),
            }],
            |_, _| 0.,
        );
    }
}

fn hp(s: &State) -> i32 {
    s.targets.iter().find(|t| t.id == VEHICLE).unwrap().hp
}

#[test]
fn a_strafing_round_through_a_vehicle_takes_its_share_of_the_class_damage() {
    let mut s = world();
    s.projectiles.push(round(0., 0));
    s.projectiles.push(round(5., 1));
    fly(&mut s);
    // 80 a game round against armour, a third for a gun (26), split between
    // the record's two rounds a game round: 13 each.
    assert_eq!(hp(&s), 150 - 26);
    let applied: Vec<i32> = s
        .history
        .iter()
        .filter(|h| h.target == VEHICLE)
        .map(|h| h.applied)
        .collect();
    assert_eq!(applied, [13, 13]);
    // Twelve such hits kill it: about 60% of a 20-round burst.
    assert_eq!((150 + 12) / 13, 12);
}

#[test]
fn a_round_beside_the_vehicle_neither_hits_it_nor_hurts_it_from_the_ground() {
    // 20 ft from the centre is outside the 15 ft volume; the round strikes
    // the ground beside it with no fuze or splash, so nothing is done.
    let mut s = world();
    s.projectiles.push(round(20., 0));
    fly(&mut s);
    assert_eq!(hp(&s), 150);
    assert!(s.history.iter().all(|h| h.target != VEHICLE));
}
