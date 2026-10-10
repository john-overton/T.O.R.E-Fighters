//! Surface units' shots: guns, flak, SAMs, collateral damage, sides and
//! support (plan section 9.1, slice W2).
use super::super::tests::{fixture, target};
use super::super::*;
use super::*;
use crate::airport::OrientedBox;
use crate::combat::ledger::Outcome;
use crate::combat::surface_guns;
use tore_formats::weapons::Movement;

/// A surface unit's id in the template range (plan 2.5).
const UNIT: u32 = 0x5000_0001;
const REDFOR: Side = Side(2);

fn launcher() -> Launcher {
    Launcher {
        position: [0., 1000., 0.],
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
/// The ownship sits at `launcher()`, out of the way unless a test puts a
/// shot near it.
fn run(s: &mut State, ticks: usize) -> Vec<Event> {
    run_from(s, ticks, launcher())
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
/// A scene with the ownship far away and no targets.
fn scene() -> State {
    let mut s = fixture(true);
    s.targets.clear();
    s
}
fn far() -> Launcher {
    Launcher {
        position: [90_000., 30_000., -90_000.],
        ..launcher()
    }
}
/// A surface unit standing at `center` on `side`, as the world registers one.
fn unit(s: &mut State, id: u32, center: Vector, side: Side) {
    s.add_ground_target(
        id,
        OrientedBox {
            center,
            half: [15., 8., 15.],
            heading: 0.,
            pitch: 0.,
            bank: 0.,
        },
        100,
        0x800,
        side,
    )
    .unwrap();
}
fn aircraft(id: u32, position: Vector, hp: i32, side: Side) -> Target {
    let mut t = target(id, position, hp, 0x80);
    t.side = side;
    t
}
fn record(source: &str) -> Weapon {
    let mut w = fixture(false).own().configuration().stations[0]
        .weapon
        .clone();
    w.source = source.into();
    w
}
fn constant(speed: i16, remove_t: u16) -> Movement {
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
        remove_t,
        powered_turn_rate: 0,
        unpowered_turn_rate: 0,
        performance_at_0: 100,
        performance_at_20: 100,
        cruise: [0; 4],
        jink: [0; 3],
    }
}
/// The retail ZSU-23 record's fields, tuned by the table.
fn zsu23() -> Weapon {
    let mut w = record("ZSU23.JT");
    w.flags = 0x140c0;
    w.seeker.signature = 3;
    w.movement = constant(3666, 20);
    w.burst.game_rounds_in_burst = 4;
    w.burst.actual_rounds_per_game = 1;
    w.burst.game_burst_t = 1;
    w.burst.reload_t = 4;
    w.damage.by_class = [20, 2, 6, 4, 20];
    w.damage.fuze_radius = 100;
    w.damage.fuze_arm_t = 0;
    w.effects.object_explosion = 18;
    surface_guns::apply("ZSU23", &mut w).unwrap();
    w
}
/// The retail KS-19 record's fields, tuned by the table: flak.
fn ks19() -> Weapon {
    let mut w = record("KS19.JT");
    w.flags = 0x2940c0;
    w.seeker.signature = 3;
    w.movement = constant(4400, 60);
    w.burst.game_rounds_in_burst = 1;
    w.burst.actual_rounds_per_game = 1;
    w.burst.game_burst_t = 2;
    w.burst.reload_t = 16;
    w.burst.startup_shots = 8;
    w.damage.by_class = [80, 8, 24, 16, 80];
    w.damage.fuze_radius = 250;
    w.damage.fuze_arm_t = 0;
    w.damage.collateral_radius = 750;
    w.damage.collateral_percent = 35;
    w.effects.object_explosion = 27;
    surface_guns::apply("KS19", &mut w).unwrap();
    w
}
/// A fast-boosting SAM under a reviewed supported name: the SA-6 record's
/// fuze and collateral, a quicker motor so a test is short.
fn sam() -> Weapon {
    let mut w = record("SA6.JT");
    w.flags = 0x12341;
    w.seeker.signature = 3;
    w.movement.minimum_speed = 0;
    w.movement.initial_speed = 0;
    w.movement.corner_speed = 3000;
    w.movement.maximum_speed = 3000;
    w.movement.final_speed = 1026;
    w.movement.acceleration = 30_000;
    w.movement.ignite_t = 0;
    w.movement.fuel_t = 40;
    w.movement.remove_t = 80;
    w.damage.by_class = [100, 10, 30, 20, 100];
    w.damage.fuze_radius = 100;
    w.damage.fuze_arm_t = 0;
    w.damage.collateral_radius = 750;
    w.damage.collateral_percent = 35;
    w.effects.object_explosion = 30;
    w
}
fn shot(weapon: Weapon, from: Vector, toward: Vector, target: Option<u32>) -> SurfaceShot {
    SurfaceShot {
        owner: UNIT,
        weapon,
        mount: 0,
        position: from,
        direction: crate::attitude::unit(sub(toward, from)),
        velocity: [0.; 3],
        target,
        observation: None,
        ordinal: 0,
        end_tick: None,
    }
}
fn hp(s: &State, id: u32) -> i32 {
    s.targets.iter().find(|t| t.id == id).unwrap().hp
}
fn flak_effects(s: &State) -> Vec<&Effect> {
    s.effects
        .iter()
        .filter(|e| e.kind == EffectKind::Flak)
        .collect()
}
fn outcomes(s: &mut State, projectile: u32) -> Vec<Outcome> {
    s.ledger
        .take_outcomes()
        .into_iter()
        .filter(|o| o.projectile == projectile)
        .collect()
}

#[test]
fn every_surface_gun_is_a_gun_at_its_record_damage_without_critical_kills() {
    for row in surface_guns::TABLE {
        let w = record(row.record);
        assert!(is_gun(&w), "{}", row.record);
        assert!(!is_aircraft_gun(&w), "{}", row.record);
        // No one-third rule: the table already matches retail damage.
        assert_eq!(scaled_weapon_damage(&w, 20), 20, "{}", row.record);
        let victim = target(1, [0.; 3], 100, 0x80);
        assert!(!critical_hit(&victim, &w, DamageSection::Cockpit, 100));
    }
    // The aircraft guns keep both rules.
    let m61 = record("M61.JT");
    assert!(is_gun(&m61) && is_aircraft_gun(&m61));
    assert_eq!(scaled_weapon_damage(&m61, 20), 6);
    let victim = target(1, [0.; 3], 100, 0x80);
    assert!(critical_hit(&victim, &m61, DamageSection::Cockpit, 1));
    // Missiles are neither.
    assert!(!is_gun(&record("SA6.JT")));
}

#[test]
fn surface_sams_have_reviewed_profiles_and_the_held_records_none() {
    for name in missiles::SURFACE_SUPPORTED {
        let profile = missiles::Profile::for_weapon(&record(name)).expect(name);
        assert_eq!(profile.guidance, Guidance::Supported, "{name}");
        assert_eq!(profile.role, TargetRole::Aircraft, "{name}");
        assert!(missiles::Profile::reviewed(name));
    }
    for name in missiles::SURFACE_INFRARED {
        let profile = missiles::Profile::for_weapon(&record(name)).expect(name);
        assert_eq!(profile.guidance, Guidance::Infrared, "{name}");
        assert_eq!(profile.role, TargetRole::Aircraft, "{name}");
    }
    for name in ["ASROC.JT", "SSN9.JT"] {
        assert!(missiles::Profile::for_weapon(&record(name)).is_none());
    }
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    let refused = s.fire_surface(shot(
        record("SSN9.JT"),
        [0., 30., 0.],
        [0., 3000., 3000.],
        None,
    ));
    assert_eq!(refused, Err(Refused::Unreviewed));
    assert!(s.projectiles.is_empty());
}

#[test]
fn a_surface_gun_round_carries_its_record_its_share_and_its_tracer() {
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    let w = zsu23();
    let per_game = u64::from(w.burst.actual_rounds_per_game);
    assert!(per_game > 1, "the ZSU-23 row splits its game rounds");
    let mut ids = Vec::new();
    for ordinal in 0..per_game + 2 {
        let mut fired = shot(w.clone(), [0., 30., 0.], [0., 1000., 0.], Some(0));
        fired.ordinal = ordinal;
        ids.push(s.fire_surface(fired).unwrap());
    }
    assert_eq!(ids[0], SURFACE_PROJECTILE_ID_BASE);
    assert_eq!(ids[1], SURFACE_PROJECTILE_ID_BASE + 1);
    for (ordinal, p) in s.projectiles.iter().enumerate() {
        let ordinal = ordinal as u64;
        assert_eq!(p.owner, UNIT);
        assert_eq!(p.weapon.as_ref(), Some(&w));
        assert_eq!(p.gun_round, Some((ordinal % per_game) as u8));
        assert_eq!(p.tracer, ordinal.is_multiple_of(3));
        // A gun round never homes; who it was aimed at is metadata.
        assert_eq!(p.target, None);
        assert_eq!(p.incoming, Some(0));
        assert_eq!(p.speed_f8, i32::from(w.movement.initial_speed) * 256);
        assert!(s.surface_round(p.id).is_some_and(|r| !r.flak));
    }
    // Flak and tank guns carry no tracer.
    assert!(!surface_tracer(&ks19(), 0));
    assert!(surface_tracer(&zsu23(), 3) && !surface_tracer(&zsu23(), 1));
}

#[test]
fn a_surface_gun_round_hits_an_aircraft_for_its_share_and_credits_its_unit() {
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    let mut victim = aircraft(7, [0., 1500., 0.], 1000, Side(1));
    victim.radius = 60.;
    s.targets.push(victim);
    let w = zsu23();
    let id = s
        .fire_surface(shot(w.clone(), [0., 30., 0.], [0., 1500., 0.], Some(7)))
        .unwrap();
    let events = run_from(&mut s, 120, far());
    assert!(events.contains(&Event::Hit(7)));
    // Ordinal 0 takes the first share of a game round's 20 points.
    let share = 20 / i32::from(w.burst.actual_rounds_per_game)
        + i32::from(20 % i32::from(w.burst.actual_rounds_per_game) > 0);
    assert_eq!(hp(&s, 7), 1000 - share);
    let strike = s.take_strikes().pop().unwrap();
    assert_eq!((strike.owner, strike.victim), (UNIT, 7));
    assert!(matches!(
        outcomes(&mut s, id).as_slice(),
        [Outcome {
            resolution: Resolution::Hit(_),
            ..
        }]
    ));
}

#[test]
fn surface_rounds_never_strike_ground_objects_or_their_own_launcher() {
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    // A building in the line of fire.
    unit(&mut s, UNIT + 1, [0., 10., 400.], REDFOR);
    // A SAM leaving its rail inside its launcher's box, low over the building.
    s.fire_surface(shot(sam(), [0., 12., 0.], [0., 30., 3000.], None))
        .unwrap();
    // A gun round straight through the building.
    s.fire_surface(shot(zsu23(), [0., 12., 0.], [0., 12., 3000.], None))
        .unwrap();
    run_from(&mut s, 240, far());
    assert_eq!(hp(&s, UNIT), 100);
    assert_eq!(hp(&s, UNIT + 1), 100);
}

#[test]
fn a_surface_missile_does_collateral_damage_to_other_aircraft_once() {
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    let struck = aircraft(1, [0., 3000., 3000.], 1000, Side(1));
    let near = aircraft(2, [500., 3000., 3000.], 1000, Side(1));
    let beyond = aircraft(3, [1000., 3000., 3000.], 1000, Side(1));
    s.targets.extend([struck, near, beyond]);
    let id = s
        .fire_surface(shot(sam(), [0., 30., 0.], [0., 3000., 3000.], Some(1)))
        .unwrap();
    let events = run_from(&mut s, 600, far());
    // The aircraft struck takes the record's damage once, the one within
    // 750 ft a share falling from 35 percent at the burst to nothing at
    // 750 ft (about 480 ft from it: 12 percent), the one beyond nothing.
    assert_eq!(hp(&s, 1), 900);
    let share = 1000 - hp(&s, 2);
    assert!(share > 0 && share < 35, "{share}");
    assert_eq!(hp(&s, 3), 1000);
    assert_eq!(events.iter().filter(|e| **e == Event::Hit(2)).count(), 1);
    // Its jolt comes from the burst, where the missile struck the first.
    let jolt = |id: u32| {
        events.iter().find_map(|e| match e {
            Event::Jolt(j) if j.target == id => Some(*j),
            _ => None,
        })
    };
    let (direct, collateral) = (jolt(1).unwrap(), jolt(2).unwrap());
    assert_eq!(collateral.from, direct.from);
    assert!((collateral.strength - f64::from(share) / 100.).abs() < 1e-12);
    assert_eq!(jolt(3), None);
    // The missile resolves once, as a hit on the aircraft it struck.
    assert!(matches!(
        outcomes(&mut s, id).as_slice(),
        [Outcome {
            resolution: Resolution::Hit(100),
            ..
        }]
    ));
    assert!(s.ledger.kills().is_empty());
}

#[test]
fn a_surface_burst_reaches_the_ownship_and_so_does_an_aircraft_weapons() {
    // A SAM striking an aircraft 400 ft ahead of the ownship.
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    let mut w = sam();
    w.damage.by_class = [40, 4, 12, 8, 40];
    s.targets.push(aircraft(1, [0., 1000., 400.], 1000, REDFOR));
    s.fire_surface(shot(
        w.clone(),
        [0., 1000., 2400.],
        [0., 1000., 400.],
        Some(1),
    ))
    .unwrap();
    let before = s.own().hp;
    let events = run(&mut s, 240);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::OwnshipDamaged { aircraft: 0, .. }))
    );
    assert!(s.own().hp < before);

    // The same missile fired by an aircraft does collateral damage too
    // (slice X1: every weapon whose record carries it).
    let mut s = scene();
    s.targets.push(aircraft(1, [0., 1000., 400.], 1000, REDFOR));
    s.targets
        .push(aircraft(9, [0., 1000., 3000.], 1000, REDFOR));
    let profile = missiles::Profile::for_weapon(&w).unwrap();
    s.projectiles.push(Projectile {
        id: 77,
        owner: 9,
        weapon: Some(w.clone()),
        guidance: Some(Flight::new(
            profile,
            LaunchMode::Cued,
            Some(1),
            [0., 1000., 2400.],
        )),
        motion: Some(Motion::launch(&w, [0.; 3], 1000.)),
        guidance_ticks: Some(profile.guidance_ticks),
        age: 0,
        incoming: None,
        station: 0,
        position: [0., 1000., 2400.],
        previous: [0., 1000., 2400.],
        direction: [0., 0., -1.],
        speed_f8: 0,
        launched_t: 0,
        target: Some(1),
        fall: FallState::default(),
        gun_round: None,
        tracer: false,
    });
    let before = s.own().hp;
    let events = run(&mut s, 240);
    assert!(events.contains(&Event::Hit(1)));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::OwnshipDamaged { aircraft: 0, .. }))
    );
    assert!(s.own().hp < before);
}

#[test]
fn friendly_fire_off_spares_the_surface_shooters_side_from_rounds_and_bursts() {
    for (setting, side, hurt) in [
        (FriendlyFire::Off, REDFOR, false),
        (FriendlyFire::Off, Side(1), true),
        (FriendlyFire::On, REDFOR, true),
    ] {
        // A gun round straight at an aircraft.
        let mut s = scene();
        s.friendly_fire = setting;
        unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
        let mut victim = aircraft(7, [0., 1500., 0.], 1000, side);
        victim.radius = 60.;
        s.targets.push(victim);
        s.fire_surface(shot(zsu23(), [0., 30., 0.], [0., 1500., 0.], Some(7)))
            .unwrap();
        run_from(&mut s, 120, far());
        assert_eq!(hp(&s, 7) < 1000, hurt, "{setting:?} {side:?} gun");

        // A SAM bursting on an enemy beside it.
        let mut s = scene();
        s.friendly_fire = setting;
        unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
        s.targets
            .push(aircraft(1, [0., 3000., 3000.], 1000, Side(1)));
        s.targets
            .push(aircraft(2, [400., 3000., 3000.], 1000, side));
        s.fire_surface(shot(sam(), [0., 30., 0.], [0., 3000., 3000.], Some(1)))
            .unwrap();
        run_from(&mut s, 600, far());
        assert_eq!(hp(&s, 1), 900);
        assert_eq!(hp(&s, 2) < 1000, hurt, "{setting:?} {side:?} burst");
    }
}

#[test]
fn flak_bursts_at_its_time_fuze_as_a_flak_effect() {
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    let w = ks19();
    let mut fired = shot(w.clone(), [0., 30., 0.], [0., 1000., 0.], None);
    fired.end_tick = Some(120);
    let id = s.fire_surface(fired).unwrap();
    assert!(s.surface_round(id).is_some_and(|r| r.flak));
    assert!(!s.projectiles[0].tracer);
    run_from(&mut s, 119, far());
    assert!(flak_effects(&s).is_empty());
    assert_eq!(s.projectiles.len(), 1);
    run_from(&mut s, 1, far());
    assert!(s.projectiles.is_empty());
    assert_eq!(s.surface_round(id), None);
    let bursts = flak_effects(&s);
    assert_eq!(bursts.len(), 1);
    assert_eq!(
        bursts[0].blast,
        Some(28),
        "the 100 mm shell bursts as the larger flak sheet"
    );
    // One second at the tuned muzzle velocity, straight up.
    let height = bursts[0].position[1] - 30.;
    assert!(
        (height - f64::from(w.movement.initial_speed)).abs() < 40.,
        "{height}"
    );
    assert!(matches!(
        outcomes(&mut s, id).as_slice(),
        [Outcome {
            resolution: Resolution::Missed,
            ..
        }]
    ));
}

#[test]
fn the_85_mm_flak_keeps_the_records_small_flak_explosion() {
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    let mut w = ks19();
    w.source = "KS12.JT".into();
    surface_guns::apply("KS12", &mut w).unwrap();
    assert_eq!(flak_explosion(&w), 27);
    let mut fired = shot(w, [0., 30., 0.], [0., 1000., 0.], None);
    fired.end_tick = Some(60);
    s.fire_surface(fired).unwrap();
    run_from(&mut s, 60, far());
    let bursts = flak_effects(&s);
    assert_eq!(bursts.len(), 1);
    assert_eq!(bursts[0].blast, Some(27));
}

#[test]
fn flak_bursts_near_a_hostile_aircraft_and_passes_a_friendly_one() {
    for (side, bursts) in [(Side(1), true), (REDFOR, false)] {
        let mut s = scene();
        unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
        // 200 ft beside the shell's path: inside its 250 ft fuze radius.
        s.targets.push(aircraft(7, [200., 6000., 0.], 1000, side));
        let id = s
            .fire_surface(shot(ks19(), [0., 30., 0.], [0., 1000., 0.], None))
            .unwrap();
        // The shell reaches the fuze sphere at about tick 236, and the 100 mm
        // burst's effect lasts one second: look while it shows.
        run_from(&mut s, 300, far());
        let found = flak_effects(&s);
        if bursts {
            assert_eq!(found.len(), 1);
            // It bursts as it enters the fuze sphere, below the aircraft.
            let at = found[0].position;
            // (The gun's dispersion moves it a few feet off the bore line.)
            assert!(
                at[0].abs() < 50. && at[1] < 6000. && at[1] > 5700.,
                "{at:?}"
            );
            // The aircraft's surface is the 250 ft fuze radius from the
            // burst: (35 - 11) percent of 80.
            assert_eq!(hp(&s, 7), 1000 - 80 * 24 / 100);
            assert!(matches!(
                outcomes(&mut s, id).as_slice(),
                [Outcome {
                    resolution: Resolution::Hit(19),
                    ..
                }]
            ));
        } else {
            assert!(found.is_empty());
            assert_eq!(hp(&s, 7), 1000);
            assert_eq!(s.projectiles.len(), 1, "still climbing");
        }
    }
}

#[test]
fn flak_bursts_at_the_end_of_its_life_and_other_rounds_end_unseen() {
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    let w = ks19();
    let life = usize::from(w.movement.remove_t) * 30;
    s.fire_surface(shot(w, [0., 30., 0.], [0., 1000., 1000.], None))
        .unwrap();
    run_from(&mut s, life + 2, far());
    assert!(s.projectiles.is_empty());
    assert_eq!(flak_effects(&s).len(), 1);

    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    let mut fired = shot(zsu23(), [0., 30., 0.], [0., 1000., 1000.], None);
    fired.end_tick = Some(30);
    let id = s.fire_surface(fired).unwrap();
    run_from(&mut s, 29, far());
    assert_eq!(s.projectiles.len(), 1);
    run_from(&mut s, 1, far());
    assert!(s.projectiles.is_empty());
    assert!(s.effects.is_empty(), "{:?}", s.effects);
    assert!(matches!(
        outcomes(&mut s, id).as_slice(),
        [Outcome {
            resolution: Resolution::Missed,
            ..
        }]
    ));
}

#[test]
fn a_time_fuze_set_from_the_range_bursts_there() {
    let w = ks19();
    let speed = f64::from(w.movement.initial_speed);
    let ticks = ticks_to_range(&w, 10_000.).unwrap();
    let expected = 10_000. / speed * 120.;
    assert!((ticks as f64 - expected).abs() <= 2., "{ticks} {expected}");
    // Beyond the shell's reach there is no fuze time.
    assert_eq!(ticks_to_range(&w, surface_guns::reach_ft(&w) + 100.), None);
    assert_eq!(ticks_to_range(&w, 0.), Some(0));
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    let mut fired = shot(w, [0., 30., 0.], [0., 1000., 0.], None);
    fired.end_tick = Some(ticks);
    s.fire_surface(fired).unwrap();
    run_from(&mut s, ticks as usize + 1, far());
    let burst = flak_effects(&s)[0].position;
    assert!((burst[1] - 30. - 10_000.).abs() < 50., "{burst:?}");
}

#[test]
fn a_battery_launcher_flies_its_sam_on_its_radars_support() {
    let radar = [1500., 40., -500.];
    let build = || {
        let mut s = scene();
        unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
        // Off the launch line: only a supported missile turns onto it.
        s.targets
            .push(aircraft(1, [2000., 3000., 4000.], 1000, Side(1)));
        let observation = seeker::Observation {
            id: 1,
            position: [2000., 3000., 4000.],
            velocity: [0.; 3],
            quality: 1.,
            off_axis: 0.,
            range: 5385.,
        };
        let mut fired = shot(sam(), [0., 30., 0.], [0., 3000., 3000.], Some(1));
        fired.observation = Some(observation);
        let id = s.fire_surface(fired).unwrap();
        (s, id, observation)
    };
    // Supported: the battery radar, not the launcher, answers for it.
    let (mut s, id, observation) = build();
    let support = ActorSupport {
        owner: UNIT,
        observation: Some(observation),
        supported: true,
        radar_position: radar,
        radar_emitting: true,
    };
    let mut events = Vec::new();
    for _ in 0..360 {
        s.set_actor_supports([support]);
        if s.projectiles.iter().any(|p| p.id == id) {
            let snapshot = s
                .missile_snapshots(&[(0, far())])
                .into_iter()
                .find(|m| m.id == id)
                .unwrap();
            assert_eq!(snapshot.owner, UNIT);
            assert!(snapshot.supported);
            assert_eq!(snapshot.supporting_radar_position, Some(radar));
        }
        events.extend(run_from(&mut s, 1, far()));
    }
    assert!(events.contains(&Event::Hit(1)), "a supported SAM strikes");
    assert_eq!(hp(&s, 1), 900);
    assert_eq!(s.take_strikes().last().map(|k| k.owner), Some(UNIT));

    // Without support (radar dead or off) it flies on and misses.
    let (mut s, id, _) = build();
    s.set_actor_supports([ActorSupport {
        radar_emitting: false,
        ..support
    }]);
    let snapshot = s
        .missile_snapshots(&[(0, far())])
        .into_iter()
        .find(|m| m.id == id)
        .unwrap();
    assert!(!snapshot.supported);
    assert_eq!(snapshot.supporting_radar_position, None);
    let events = run_from(&mut s, 360, far());
    assert!(!events.contains(&Event::Hit(1)));
    assert_eq!(hp(&s, 1), 1000);
}

#[test]
fn an_owner_may_support_missiles_at_two_targets_at_once() {
    let mut s = scene();
    let observe = |id: u32| seeker::Observation {
        id,
        position: [0.; 3],
        velocity: [0.; 3],
        quality: 1.,
        off_axis: 0.,
        range: 0.,
    };
    let support = |id: u32| ActorSupport {
        owner: UNIT,
        observation: Some(observe(id)),
        supported: true,
        radar_position: [1., 2., 3.],
        radar_emitting: true,
    };
    s.set_actor_supports([
        support(1),
        support(2),
        ActorSupport {
            observation: None,
            ..support(3)
        },
    ]);
    assert!(s.support_for(UNIT, Some(1)).is_some());
    assert!(s.support_for(UNIT, Some(2)).is_some());
    assert_eq!(s.support_for(UNIT, Some(3)), None);
    assert_eq!(s.support_for(UNIT, None), None);
}

#[test]
fn surface_fire_leaves_room_for_the_aircraft_weapons() {
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    let filler = shot(zsu23(), [0., 30., 0.], [0., 1000., 0.], None);
    let limit = MAX_PROJECTILES - SURFACE_PROJECTILE_RESERVE;
    for n in 0..limit {
        assert!(s.fire_surface(filler.clone()).is_ok(), "{n}");
    }
    assert_eq!(s.surface_capacity(), 0);
    assert_eq!(s.fire_surface(filler.clone()), Err(Refused::Capacity));
    assert_eq!(s.projectiles.len(), limit);
    // Bad aims are refused without taking an id.
    let mut bad = filler.clone();
    bad.direction = [0.; 3];
    s.projectiles.clear();
    assert_eq!(s.fire_surface(bad), Err(Refused::Invalid));
    // Ids wrap below the theater objects' range.
    s.next_surface_shot = super::SURFACE_PROJECTILE_ID_LAST;
    assert_eq!(
        s.fire_surface(filler.clone()),
        Ok(super::SURFACE_PROJECTILE_ID_LAST)
    );
    assert_eq!(s.fire_surface(filler), Ok(SURFACE_PROJECTILE_ID_BASE));
}

#[test]
fn a_destroyed_unit_explodes_and_craters_as_its_record_says() {
    for (look, water) in [
        (
            Some(GroundLook {
                explosion: 21,
                crater: 5,
            }),
            false,
        ),
        (
            Some(GroundLook {
                explosion: 21,
                crater: 5,
            }),
            true,
        ),
        (None, false),
    ] {
        let mut s = scene();
        unit(&mut s, UNIT, [0., 10., 3000.], REDFOR);
        if let Some(look) = look {
            assert!(s.set_ground_look(UNIT, look));
            assert_eq!(s.ground_look(UNIT), Some(look));
        }
        let mut w = record("MK82.JT");
        w.seeker.signature = 0;
        w.flags = 0x14;
        w.damage.by_class = [500; 5];
        w.effects.object_explosion = 18;
        s.projectiles.push(Projectile {
            id: 5,
            owner: 0,
            weapon: Some(w),
            guidance: None,
            motion: None,
            guidance_ticks: None,
            age: 0,
            incoming: None,
            station: 0,
            position: [0., 15., 2900.],
            previous: [0., 15., 2900.],
            direction: [0., 0., 1.],
            speed_f8: 600 << 8,
            launched_t: 0,
            target: None,
            fall: FallState::default(),
            gun_round: None,
            tracer: false,
        });
        let events = (0..60)
            .flat_map(|_| {
                s.step_surface(
                    &[OwnshipInput {
                        aircraft: 0,
                        held: false,
                        launcher: far(),
                    }],
                    |_, _| 0.,
                    |_, _| water,
                )
            })
            .collect::<Vec<_>>();
        assert!(events.contains(&Event::Destroyed(UNIT)));
        let blast = s
            .effects
            .iter()
            .find(|e| e.kind == EffectKind::Destroyed)
            .and_then(|e| e.blast)
            .unwrap();
        let craters: Vec<_> = s
            .marks
            .iter()
            .filter_map(|m| match m.kind {
                crate::combat::blast::MarkKind::Crater(size) => Some(size),
                _ => None,
            })
            .collect();
        match (look, water) {
            (Some(_), false) => {
                assert!([21, 23].contains(&blast), "{blast}");
                assert_eq!(craters, [5]);
            }
            (Some(_), true) => {
                assert!([21, 23].contains(&blast), "{blast}");
                assert!(craters.is_empty());
            }
            (None, _) => {
                assert!([35, 36, 37].contains(&blast), "{blast}");
                assert!(craters.is_empty());
            }
        }
    }
    let mut s = scene();
    assert!(!s.set_ground_look(
        42,
        GroundLook {
            explosion: 21,
            crater: 1
        }
    ));
}

#[test]
fn a_surface_radar_is_received_as_a_ground_emitter() {
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 30_000.], REDFOR);
    unit(&mut s, UNIT + 1, [3000., 10., 30_000.], REDFOR);
    s.targets
        .iter_mut()
        .find(|t| t.id == UNIT)
        .unwrap()
        .radar_emitting = true;
    run(&mut s, 2);
    let received: Vec<_> = s.own().emitters.iter().map(|e| (e.id, e.symbol)).collect();
    assert_eq!(received, [(UNIT, passive::Symbol::Ground)]);
    // A destroyed radar is silent.
    s.targets.iter_mut().find(|t| t.id == UNIT).unwrap().hp = 0;
    run(&mut s, 1);
    assert!(s.own().emitters.is_empty());
}

#[test]
fn surface_rounds_and_looks_restore_from_a_checkpoint_and_fly_on_identically() {
    use crate::checkpoint::{Models, from_bytes, to_bytes};
    let mut s = scene();
    unit(&mut s, UNIT, [0., 10., 0.], REDFOR);
    s.set_ground_look(
        UNIT,
        GroundLook {
            explosion: 21,
            crater: 4,
        },
    );
    s.targets
        .push(aircraft(7, [200., 6000., 0.], 1000, Side(1)));
    let mut fired = shot(ks19(), [0., 30., 0.], [0., 1000., 0.], None);
    fired.end_tick = Some(3000);
    s.fire_surface(fired).unwrap();
    s.fire_surface(shot(zsu23(), [0., 30., 0.], [0., 1000., 1000.], None))
        .unwrap();
    run_from(&mut s, 10, far());
    let models = Models::default();
    let coded = to_bytes(&s, &models).unwrap();
    let mut copy: State = from_bytes(&coded, &models).unwrap();
    assert_eq!(copy.surface_rounds, s.surface_rounds);
    assert_eq!(copy.ground_looks, s.ground_looks);
    assert_eq!(copy.next_surface_shot, s.next_surface_shot);
    let a = run_from(&mut s, 300, far());
    let b = run_from(&mut copy, 300, far());
    assert_eq!(a, b);
    assert_eq!(format!("{:?}", s.effects), format!("{:?}", copy.effects));
    assert_eq!(flak_effects(&s).len(), 1);
}

/// Run with `TORE_DATA_DIR` naming a data folder with an import: the shipped
/// surface-to-air records have the seekers their guidance class assumes, and
/// every supported one but SA-19 and SA-N-11 carries the support flag.
#[test]
#[ignore = "needs an imported data profile (TORE_DATA_DIR)"]
fn real_data_surface_sam_records_match_their_guidance() {
    use std::io::Read;
    let directory = std::env::var_os("TORE_DATA_DIR").expect("TORE_DATA_DIR names an import");
    let mut packs: Vec<_> = std::fs::read_dir(&directory)
        .expect("the data folder")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "pack")
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("menu-"))
        })
        .collect();
    packs.sort();
    let mut bytes = Vec::new();
    std::fs::File::open(packs.pop().expect("an import pack"))
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .expect("the pack reads");
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let mut records = std::collections::BTreeMap::new();
    let (count, mut at) = (word(12), 16);
    for _ in 0..count {
        let len = u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
        let name = String::from_utf8(bytes[at + 2..at + 2 + len].to_vec()).unwrap();
        let size = word(at + 2 + len);
        let start = at + 6 + len;
        if name.ends_with(".JT") {
            records.insert(
                name.clone(),
                Weapon::parse(&name, &bytes[start..start + size]).expect("a record"),
            );
        }
        at = start + size;
    }
    for name in missiles::SURFACE_SUPPORTED {
        let w = &records[name];
        assert_eq!(w.seeker.signature, 3, "{name}");
        assert_eq!(
            w.flags & 0x200 != 0,
            !["SA19.JT", "SAN11.JT"].contains(&name),
            "{name}"
        );
        assert_eq!(w.movement.initial_speed, 0, "{name} starts from rest");
    }
    for name in missiles::SURFACE_INFRARED {
        assert_eq!(records[name].seeker.signature, 2, "{name}");
    }
    for name in ["KS12.JT", "KS19.JT"] {
        assert!(is_flak(&records[name]), "{name}");
    }
    for row in surface_guns::TABLE {
        if !["KS12.JT", "KS19.JT"].contains(&row.record) {
            assert!(!is_flak(&records[row.record]), "{}", row.record);
        }
    }
}
