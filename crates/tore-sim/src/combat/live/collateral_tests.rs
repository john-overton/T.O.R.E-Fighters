//! Splash damage for every weapon (slice X1): the falloff, the ground share,
//! who is spared, what is reached and who is credited.
use super::super::tests::{fixture, target};
use super::super::*;
use super::*;
use crate::airport::OrientedBox;
use tore_formats::weapons::Movement;

const BLUE: Side = Side(1);
const RED: Side = Side(2);
/// An AI shooter's actor id, as the AI aircraft rounds carry.
const SHOOTER: u32 = 9;

fn far() -> Launcher {
    Launcher {
        position: [90_000., 30_000., -90_000.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 300.,
        velocity: [0., 0., 300.],
        bay_ready: true,
        radar_power: true,
        radar: true,
        jammer: false,
        alive: true,
        body_present: true,
        controls: sensors::Controls::default(),
    }
}
fn run_from(s: &mut State, ticks: usize, own: Launcher) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..ticks {
        events.extend(s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: own,
            }],
            |_, _| 0.,
        ));
    }
    events
}
fn scene() -> State {
    let mut s = fixture(false);
    s.targets.clear();
    s
}
fn aircraft(id: u32, position: Vector, side: Side) -> Target {
    let mut t = target(id, position, 1000, 0x80);
    t.side = side;
    t
}
/// A parked aircraft as slice PA1 registers one: an aircraft target row with
/// the Surface role, on the ground.
fn parked(id: u32, position: Vector) -> Target {
    let mut t = target(id, position, 1000, 0x80);
    t.role = TargetRole::Surface;
    t.airborne = false;
    t.on_ground = true;
    t
}
fn ground_object(s: &mut State, id: u32, center: Vector, hp: i32, side: Side) {
    s.add_ground_target(
        id,
        OrientedBox {
            center,
            half: [15., 8., 15.],
            heading: 0.,
            pitch: 0.,
            bank: 0.,
        },
        hp,
        0x800,
        side,
    )
    .unwrap();
}
fn constant(speed: i16) -> Movement {
    Movement {
        minimum_speed: speed,
        corner_speed: speed,
        maximum_speed: speed,
        acceleration: 0,
        deceleration: 0,
        initial_speed: speed,
        final_speed: speed,
        launch_retard: 0,
        ignite_t: 0,
        fuel_t: 0,
        remove_t: 60,
        powered_turn_rate: 0,
        unpowered_turn_rate: 0,
        performance_at_0: 100,
        performance_at_20: 100,
        cruise: [0; 4],
        jink: [0; 3],
    }
}
/// The retail AIM-7M's damage, fuze and collateral (750 ft at 35 percent),
/// flying straight at a constant 1,000 ft/s so a test is short.
fn sparrow() -> Weapon {
    let mut w = fixture(false).own().configuration().stations[0]
        .weapon
        .clone();
    w.source = "AIM7.JT".into();
    w.flags = 0x1;
    w.movement = constant(1000);
    w.damage.by_class = [170, 17, 51, 34, 170];
    w.damage.fuze_radius = 100;
    w.damage.fuze_arm_t = 0;
    w.damage.collateral_radius = 750;
    w.damage.collateral_percent = 35;
    w.effects.object_explosion = 30;
    w.effects.land_explosion = 21;
    w
}
/// The retail Mk 84's damage and collateral (1,000 ft at 100 percent),
/// falling straight down at 500 ft/s.
fn mk84() -> Weapon {
    let mut w = sparrow();
    w.source = "MK84.JT".into();
    w.flags = 0x10;
    w.movement = constant(500);
    w.damage.by_class = [400; 5];
    // (The record's 300 ft fuze would burst it on an aircraft row it
    // passes; these tests drop it clear of aircraft onto the ground.)
    w.damage.fuze_radius = 0;
    w.damage.collateral_radius = 1000;
    w.damage.collateral_percent = 100;
    w.effects.land_explosion = 35;
    w.effects.crater_size = 18;
    w
}
fn round(id: u32, owner: u32, w: &Weapon, from: Vector, toward: Vector) -> Projectile {
    let direction = crate::attitude::unit(sub(toward, from));
    Projectile {
        id,
        owner,
        weapon: Some(w.clone()),
        guidance: None,
        motion: None,
        guidance_ticks: None,
        age: 0,
        incoming: None,
        station: 0,
        position: from,
        previous: from,
        direction,
        speed_f8: i32::from(w.movement.initial_speed) * 256,
        launched_t: 0,
        target: None,
        fall: FallState::default(),
        gun_round: None,
        tracer: false,
    }
}
fn hp(s: &State, id: u32) -> i32 {
    s.targets.iter().find(|t| t.id == id).unwrap().hp
}
fn burst(w: &Weapon, detonation: Detonation) -> Burst {
    Burst::new(
        &round(1, SHOOTER, w, [0.; 3], [0., 0., 1.]),
        w,
        Shooter {
            side: NO_SIDE,
            surface: false,
            ownship: false,
        },
        [0.; 3],
        detonation,
        None,
        false,
    )
}

#[test]
fn the_share_falls_off_in_a_straight_line_to_the_radius_edge() {
    let air = burst(&sparrow(), Detonation::Air);
    assert_eq!(air.percent_at(0.), 35);
    assert_eq!(air.percent_at(-5.), 35);
    // 35 * 375 / 750 = 17.5, whole percent 17 off: 18 left.
    assert_eq!(air.percent_at(375.9), 18);
    assert_eq!(air.percent_at(749.), 1);
    assert_eq!(air.percent_at(750.), 0);
    assert_eq!(air.percent_at(5_000.), 0);
    // On the ground a missile passes half its share on, a bomb 70 percent.
    let ground = burst(&sparrow(), Detonation::Ground);
    assert_eq!(ground.percent_at(0.), 17);
    assert_eq!(ground.percent_at(375.), 9);
    let bomb = burst(&mk84(), Detonation::Ground);
    assert_eq!(bomb.percent_at(0.), 70);
    assert_eq!(bomb.percent_at(500.), 35);
    assert_eq!(bomb.percent_at(999.), 0);
    assert_eq!(burst(&mk84(), Detonation::Air).percent_at(250.), 75);
    assert_eq!(ground_share(100), 70);
    assert_eq!(ground_share(35), 50);
    // A record without collateral reaches no one.
    let mut gun = sparrow();
    gun.damage.collateral_radius = 0;
    assert!(!burst(&gun, Detonation::Air).collateral());
    assert_eq!(burst(&gun, Detonation::Air).percent_at(0.), 0);
}

#[test]
fn distance_is_measured_to_the_target_surface() {
    let bounds = OrientedBox {
        center: [0., 10., 0.],
        half: [15., 8., 30.],
        heading: 0.,
        pitch: 0.,
        bank: 0.,
    };
    assert_eq!(box_distance(&bounds, [5., 10., 5.]), 0.);
    assert!((box_distance(&bounds, [115., 10., 0.]) - 100.).abs() < 1e-9);
    assert!((box_distance(&bounds, [0., 10., 130.]) - 100.).abs() < 1e-9);
    // Past a corner, to the corner.
    let corner = box_distance(&bounds, [18., 22., 34.]);
    assert!((corner - (9_f64 + 16. + 16.).sqrt()).abs() < 1e-9);
    // Turned a quarter, the long side faces east.
    let turned = OrientedBox {
        heading: std::f64::consts::FRAC_PI_2,
        ..bounds
    };
    assert!((box_distance(&turned, [130., 10., 0.]) - 100.).abs() < 1e-6);
    assert!((sphere_distance([0.; 3], 20., [0., 0., 120.]) - 100.).abs() < 1e-12);
    assert_eq!(sphere_distance([0.; 3], 20., [0., 0., 10.]), 0.);
}

/// An AI shooter's Sparrow flying at aircraft 1 from the south; aircraft 2
/// flies 300 ft beside it, aircraft 3 900 ft beside it. Returns the events.
fn sparrow_at_a_formation(s: &mut State, wing_side: Side) -> Vec<Event> {
    s.targets.push(aircraft(SHOOTER, [0., 5000., -3000.], RED));
    s.targets.push(aircraft(1, [0., 5000., 0.], BLUE));
    s.targets.push(aircraft(2, [300., 5000., 0.], wing_side));
    s.targets.push(aircraft(3, [900., 5000., 0.], wing_side));
    s.projectiles.push(round(
        70,
        SHOOTER,
        &sparrow(),
        [0., 5000., -600.],
        [0., 5000., 0.],
    ));
    run_from(s, 120, far())
}

#[test]
fn missile_splash_hurts_a_wingman_in_formation_and_credits_the_shooter() {
    let mut s = scene();
    let events = sparrow_at_a_formation(&mut s, BLUE);
    // The struck aircraft takes the direct hit only.
    assert_eq!(hp(&s, 1), 1000 - 170);
    // The fuze fires 120 ft short of aircraft 1 (20 ft body, 100 ft fuze):
    // the wingman's surface is sqrt(300^2 + 120^2) - 20 = 303 ft away, so
    // 35 - 14 = 21 percent of 170.
    assert_eq!(hp(&s, 2), 1000 - 170 * 21 / 100);
    assert_eq!(hp(&s, 3), 1000, "beyond 750 ft");
    assert_eq!(events.iter().filter(|e| **e == Event::Hit(2)).count(), 1);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Jolt(Jolt { target: 2, strength, .. }) if (*strength - 0.35).abs() < 1e-12
    )));
    let strikes = s.take_strikes();
    assert!(strikes.contains(&Strike {
        owner: SHOOTER,
        victim: 2,
        weapon_flags: 0x1,
        destroyed: false,
        amount: 35,
    }));
    assert_eq!(s.history.last().unwrap().target, 2);
    assert!(s.ledger.kills().is_empty());
}

#[test]
fn a_shooter_caught_in_its_own_blast_is_hurt_and_credits_no_one() {
    for (setting, hurt) in [(FriendlyFire::On, true), (FriendlyFire::Off, false)] {
        let mut s = scene();
        s.friendly_fire = setting;
        // The shooter flies 200 ft from the burst.
        s.targets.push(aircraft(SHOOTER, [200., 5000., 0.], RED));
        s.targets.push(aircraft(1, [0., 5000., 0.], BLUE));
        s.projectiles.push(round(
            70,
            SHOOTER,
            &sparrow(),
            [0., 5000., -600.],
            [0., 5000., 0.],
        ));
        run_from(&mut s, 120, far());
        assert_eq!(hp(&s, 1), 830, "{setting:?}");
        assert_eq!(hp(&s, SHOOTER) < 1000, hurt, "{setting:?}");
        assert!(s.ledger.kills().is_empty());
        assert!(s.take_strikes().iter().all(|k| k.victim != SHOOTER));
    }
    // The player's own bomb dropped too low: damage, and no one credited.
    let mut s = scene();
    s.projectiles
        .push(round(70, 0, &mk84(), [0., 60., 0.], [0., 0., 0.]));
    let low = Launcher {
        position: [0., 300., 50.],
        ..far()
    };
    let before = s.own().hp;
    let events = run_from(&mut s, 60, low);
    assert!(s.own().hp < before);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::OwnshipDamaged { aircraft: 0, .. }))
    );
    assert!(s.ledger.kills().iter().all(|k| k.owner != 0));
    assert!(s.take_strikes().iter().all(|k| k.owner != 0));
}

#[test]
fn friendly_fire_off_spares_the_shooters_side_from_splash() {
    for (setting, wing_side, hurt) in [
        (FriendlyFire::On, BLUE, true),
        (FriendlyFire::On, RED, true),
        (FriendlyFire::Off, BLUE, true),
        (FriendlyFire::Off, RED, false),
    ] {
        let mut s = scene();
        s.friendly_fire = setting;
        sparrow_at_a_formation(&mut s, wing_side);
        assert_eq!(hp(&s, 1), 830, "{setting:?} {wing_side:?}");
        assert_eq!(hp(&s, 2) < 1000, hurt, "{setting:?} {wing_side:?}");
    }
    // Friendly fire off spares the side's ground objects and parked
    // aircraft too.
    for (side, hurt) in [(RED, false), (BLUE, true), (NO_SIDE, true)] {
        let mut s = scene();
        s.friendly_fire = FriendlyFire::Off;
        s.targets.push(aircraft(SHOOTER, [0., 5000., -3000.], RED));
        ground_object(&mut s, 0x4000_0001, [200., 8., 0.], 1000, side);
        let mut plane = parked(0x5000_0001, [0., 8., 300.]);
        plane.side = side;
        s.targets.push(plane);
        s.projectiles
            .push(round(70, SHOOTER, &mk84(), [0., 60., 0.], [0., 0., 0.]));
        run_from(&mut s, 60, far());
        assert_eq!(hp(&s, 0x4000_0001) < 1000, hurt, "{side:?}");
        assert_eq!(hp(&s, 0x5000_0001) < 1000, hurt, "{side:?}");
    }
}

#[test]
fn a_bomb_on_the_ground_hurts_ground_objects_and_parked_aircraft() {
    let mut s = scene();
    s.targets.push(aircraft(SHOOTER, [0., 3000., -3000.], RED));
    // A vehicle 200 ft east of the impact: its near side is 185 ft away.
    ground_object(&mut s, 0x4000_0001, [200., 8., 0.], 1000, BLUE);
    // A parked aircraft 400 ft north: its body's surface is 380 ft away.
    s.targets.push(parked(0x5000_0001, [0., 0., 400.]));
    // A building beyond the radius.
    ground_object(&mut s, 0x4000_0002, [1100., 8., 0.], 1000, BLUE);
    s.projectiles
        .push(round(70, SHOOTER, &mk84(), [0., 60., 0.], [0., 0., 0.]));
    let events = run_from(&mut s, 60, far());
    assert!(events.contains(&Event::Ground));
    // On the ground the Mk 84 passes on 70 percent of its share:
    // (100 - 18) * 70 / 100 = 57 percent, and (100 - 38) * 70 / 100 = 43.
    assert_eq!(hp(&s, 0x4000_0001), 1000 - 400 * 57 / 100);
    assert_eq!(hp(&s, 0x5000_0001), 1000 - 400 * 43 / 100);
    assert_eq!(hp(&s, 0x4000_0002), 1000);
    // No jolt for things on the ground that are not aircraft rows.
    assert!(!events.iter().any(|e| matches!(
        e,
        Event::Jolt(Jolt {
            target: 0x4000_0001,
            ..
        })
    )));
}

#[test]
fn a_surface_units_splash_still_reaches_aircraft_only() {
    let mut s = scene();
    ground_object(&mut s, 0x5000_0009, [0., 8., -5000.], 1000, RED);
    s.targets.push(parked(0x5000_0001, [0., 0., 300.]));
    ground_object(&mut s, 0x4000_0001, [200., 8., 0.], 1000, BLUE);
    s.targets.push(aircraft(1, [0., 200., 300.], BLUE));
    let mut sam = sparrow();
    sam.source = "SA6.JT".into();
    s.projectiles
        .push(round(70, 0x5000_0009, &sam, [0., 60., 0.], [0., 0., 0.]));
    s.surface_rounds.insert(
        70,
        SurfaceRound {
            end_tick: None,
            flak: false,
        },
    );
    run_from(&mut s, 60, far());
    assert_eq!(hp(&s, 0x5000_0001), 1000);
    assert_eq!(hp(&s, 0x4000_0001), 1000);
    // The aircraft overhead: its surface is sqrt(200^2 + 300^2) - 20 = 340
    // ft from the ground burst, so half of (35 - 15) percent.
    assert_eq!(hp(&s, 1), 1000 - 170 * 10 / 100);
}

#[test]
fn a_collateral_kill_is_credited_to_its_shooter() {
    // The player's bomb lands beside a fuel truck and an enemy fighter
    // taxiing past.
    let mut s = scene();
    let truck = 0x5000_0002;
    ground_object(&mut s, truck, [100., 8., 0.], 50, RED);
    assert!(s.set_ground_look(
        truck,
        GroundLook {
            explosion: 21,
            crater: 6,
        }
    ));
    let mut taxiing = aircraft(4, [0., 10., 150.], RED);
    taxiing.hp = 20;
    s.targets.push(taxiing);
    s.projectiles
        .push(round(70, 0, &mk84(), [0., 60., 0.], [0., 0., 0.]));
    let events = run_from(&mut s, 60, far());
    assert_eq!(hp(&s, truck), 0);
    assert_eq!(hp(&s, 4), 0);
    let kills = s.ledger.kills();
    assert!(kills.contains(&Kill {
        owner: 0,
        victim: truck,
        category: 0x800,
        aircraft: false,
    }));
    assert!(kills.contains(&Kill {
        owner: 0,
        victim: 4,
        category: 0x80,
        aircraft: true,
    }));
    assert!(events.contains(&Event::Destroyed(truck)));
    assert!(events.contains(&Event::Destroyed(4)));
    let strikes = s.take_strikes();
    assert!(
        strikes
            .iter()
            .any(|k| k.owner == 0 && k.victim == truck && k.destroyed && k.amount == 50)
    );
    // Both kills count on the player's score; the bomb itself struck nothing.
    assert_eq!(s.own().kills, 2);
    // The truck explodes as its record says, where it stands, and craters.
    assert!(s.effects.iter().any(|e| e.kind == EffectKind::Destroyed
        && e.blast.is_some_and(|b| (21..=23).contains(&b))
        && (e.position[1] - 0.).abs() < 1e-9));
    assert!(
        s.marks
            .iter()
            .any(|m| m.kind == crate::combat::blast::MarkKind::Crater(6))
    );
}

#[test]
fn an_ai_missile_splash_kill_of_a_wingman_goes_to_the_ai_shooter() {
    let mut s = scene();
    s.targets.push(aircraft(SHOOTER, [0., 5000., -3000.], RED));
    s.targets.push(aircraft(1, [0., 5000., 0.], BLUE));
    let mut wingman = aircraft(2, [150., 5000., 0.], BLUE);
    wingman.hp = 10;
    s.targets.push(wingman);
    s.projectiles.push(round(
        70,
        SHOOTER,
        &sparrow(),
        [0., 5000., -600.],
        [0., 5000., 0.],
    ));
    let events = run_from(&mut s, 120, far());
    assert_eq!(hp(&s, 2), 0);
    assert_eq!(
        s.ledger.kills(),
        &[Kill {
            owner: SHOOTER,
            victim: 2,
            category: 0x80,
            aircraft: true,
        }]
    );
    assert!(events.contains(&Event::Destroyed(2)));
    // An AI shooter has no ownship score.
    assert_eq!(s.own().kills, 0);
}

#[test]
fn an_aircraft_missile_splash_reaches_the_player() {
    // An enemy's missile strikes an aircraft 400 ft ahead of the player.
    let mut s = scene();
    let own = Launcher {
        position: [0., 5000., 400.],
        ..far()
    };
    s.targets.push(aircraft(SHOOTER, [0., 5000., -3000.], RED));
    s.targets.push(aircraft(1, [0., 5000., 0.], BLUE));
    s.projectiles.push(round(
        70,
        SHOOTER,
        &sparrow(),
        [0., 5000., -600.],
        [0., 5000., 0.],
    ));
    let before = s.own().hp;
    let events = run_from(&mut s, 120, own);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::OwnshipDamaged { aircraft: 0, .. }))
    );
    assert!(s.own().hp < before);
}
