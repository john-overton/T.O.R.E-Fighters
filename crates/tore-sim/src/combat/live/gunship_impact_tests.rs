//! The pipper against rounds the live simulation actually fires
//! (gunship_impact.rs, plan section 2.5). The pipper is computed on the
//! firing tick from the guns' real train; the round then flies through
//! `State::step` over the same terrain and its ground effect is read back.
use super::*;
use crate::combat::{
    gunship,
    gunship_impact::{self, Impact, Shot},
};
use std::f64::consts::{FRAC_PI_2, PI};

const OWN: u32 = 0;

fn launcher(bank: f64, pitch: f64, speed: f64) -> Launcher {
    Launcher {
        radar_power: true,
        position: [0., 3000., 0.],
        basis: Basis::new(0., pitch, bank),
        speed_fps: speed,
        velocity: [0.; 3],
        bay_ready: true,
        radar: true,
        jammer: false,
        alive: true,
        body_present: true,
        controls: Default::default(),
    }
}

/// An AC-130 world with the three guns trained on a stationary object at
/// `at` and every gun ready. `flags` is the guns' weapon flags.
fn world(flags: u32, l: Launcher, at: Vector, ground: &(impl Fn(f64, f64) -> f64 + Sync)) -> State {
    let mut old = super::tests::fixture(false);
    old.command(OWN, Command::ReplaceTarget, l);
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
            station.weapon.flags = flags;
            station.weapon.seeker.signature = 0;
            // The rounds never arm in flight, so they pass through the object
            // the guns are trained on and end on the terrain behind it.
            station.weapon.damage.fuze_arm_t = 200;
            station.weapon.burst.game_rounds_in_burst = 1;
            station.weapon.burst.actual_rounds_per_game = 1;
            station.weapon.burst.game_burst_t = (i + 1) as u8;
            station.count = 400;
            station.mount = gunship::pivot(i);
            station
        })
        .collect();
    config.hardpoint_slots = (0..3).map(Some).collect();
    config.radar_hardpoint = None;
    let mut s = State::new(config, false).unwrap();
    let mut target = old.targets[0].clone();
    target.id = 42;
    target.position = [at[0], at[1] + 15., at[2]];
    target.velocity = [0.; 3];
    target.hp = 100000;
    target.initial_hp = 100000;
    s.targets.push(target);
    step(&mut s, l, false, ground);
    s.command(OWN, Command::DesignateTarget(42), l);
    assert_eq!(s.own().designated(), Some(42));
    for _ in 0..480 {
        step(&mut s, l, false, ground);
    }
    let group = s.own().gunship.as_ref().unwrap();
    assert!(
        group.status.iter().all(|r| *r == Readiness::Ready),
        "{:?} flags {flags:x} bank {} pitch {} speed {} at {at:?}",
        group.status,
        l.basis.up[0],
        l.basis.forward[1],
        l.speed_fps
    );
    s
}

fn step(
    s: &mut State,
    l: Launcher,
    held: bool,
    ground: &(impl Fn(f64, f64) -> f64 + Sync),
) -> Vec<Event> {
    s.step(
        &[OwnshipInput {
            aircraft: OWN,
            held,
            launcher: l,
        }],
        ground,
    )
}

fn gap(a: Vector, b: Vector) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

/// The round number whose spread from the centre line is smallest for this
/// owner and station, so the fired round flies almost exactly down the bore.
fn straightest_id(w: &Weapon, forward: Vector, station: usize) -> (u32, f64) {
    (1..400_000)
        .map(|id| {
            let d = projectile_launch_direction(w, forward, id, OWN, station);
            (id, dot(d, unit(forward)).clamp(-1., 1.).acos())
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
}

/// Fires `slot` once as round `id` (or the next number, if `None`) and flies
/// it out. Returns the pipper computed on the firing tick, the round's
/// spread angle and the ground effect it made.
fn fire_and_compare(
    s: &mut State,
    l: Launcher,
    slot: usize,
    id: Option<u32>,
    ground: &(impl Fn(f64, f64) -> f64 + Sync),
) -> (Impact, f64, Vector, f64) {
    {
        let group = s.own_mut().gunship.as_mut().unwrap();
        group.included = std::array::from_fn(|i| i == slot);
    }
    step(s, l, false, ground);
    let (heading, elevation) = {
        let group = s.own().gunship.as_ref().unwrap();
        (group.headings[slot], group.elevations[slot])
    };
    let station = s.own().gunship.as_ref().unwrap().stations[slot].unwrap();
    let weapon = s.own().configuration().stations[station].weapon.clone();
    let tick = s.tick;
    let pipper = gunship_impact::impact(
        slot,
        &weapon,
        &l,
        heading,
        elevation,
        Shot {
            target: None,
            launch_tick: tick,
        },
        ground,
    )
    .unwrap();
    if let Some(id) = id {
        s.next_shot = id;
    }
    s.effects.clear();
    let events = step(s, l, true, ground);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Fired { station: n, .. } if *n == station)),
        "no shot: {:?}",
        s.own().gunship.as_ref().unwrap().status
    );
    let group = s.own().gunship.as_ref().unwrap();
    assert_eq!(
        (group.headings[slot], group.elevations[slot]),
        (heading, elevation)
    );
    let round = s.projectiles.last().unwrap();
    let spread = dot(round.direction, gunship::direction(l, heading, elevation))
        .clamp(-1., 1.)
        .acos();
    let flown_from = s.tick;
    for _ in 0..4000 {
        if s.projectiles.is_empty() {
            break;
        }
        step(s, l, false, ground);
    }
    assert!(s.projectiles.is_empty());
    let landed = s
        .effects
        .iter()
        .rfind(|e| e.kind == EffectKind::Ground)
        .expect("a ground effect")
        .position;
    let seconds = (s.tick - flown_from + 1) as f64 / 120.;
    (pipper, spread, landed, seconds)
}

const FLAGS: [u32; 2] = [0x884, 0x8c4];

fn ground_plane(_: f64, _: f64) -> f64 {
    0.
}
fn ground_slope(_: f64, z: f64) -> f64 {
    0.04 * z - 20.
}

#[test]
fn pipper_lands_on_the_fired_rounds_ground_impact_for_each_gun_and_angle() {
    // (bank, pitch, speed, target) over flat ground. The target gives the
    // guns their angles; the check is about where the rounds land.
    let mut worst: f64 = 0.;
    let cases = [
        (0., 0., 0., [-3200., 0., 0.]),
        (-0.4, 0.03, 250., [-3500., 0., 500.]),
        (-0.55, -0.02, 330., [-2600., 0., -700.]),
        (-0.2, 0.0, 120., [-3700., 0., 300.]),
    ];
    for flags in FLAGS {
        for (bank, pitch, speed, at) in cases {
            let l = launcher(bank, pitch, speed);
            let mut s = world(flags, l, at, &ground_plane);
            for slot in 0..3 {
                let station = s.own().gunship.as_ref().unwrap().stations[slot].unwrap();
                let weapon = s.own().configuration().stations[station].weapon.clone();
                let (id, spread) = straightest_id(&weapon, l.basis.forward, station);
                assert!(spread < 1e-4, "{spread}");
                let (pipper, _, landed, seconds) =
                    fire_and_compare(&mut s, l, slot, Some(id), &ground_plane);
                let Impact::Ground {
                    point,
                    seconds: predicted,
                    ..
                } = pipper
                else {
                    panic!("{pipper:?}");
                };
                let miss = gap(point, landed);
                worst = worst.max(miss);
                assert!(
                    miss < 2.,
                    "flags {flags:x} bank {bank} slot {slot}: pipper {point:?} round {landed:?} ({miss:.2} ft)"
                );
                assert!((predicted - seconds).abs() < 0.05, "{predicted} {seconds}");
            }
        }
    }
    eprintln!("worst pipper miss against a straight round: {worst:.3} ft");
}

#[test]
fn ordinary_rounds_land_within_their_own_spread_of_the_pipper() {
    let l = launcher(-0.4, 0.02, 250.);
    let at = [-3300., 0., 400.];
    let mut s = world(0x884, l, at, &ground_plane);
    for slot in 0..3 {
        for _ in 0..4 {
            let (pipper, spread, landed, _) =
                fire_and_compare(&mut s, l, slot, None, &ground_plane);
            let range = pipper.range_ft();
            assert!(spread <= 0.25_f64.to_radians() + 1e-9);
            // The cone's own footprint, plus the same few feet.
            // A cone strikes the ground stretched by one over the sine of the
            // angle it comes down at.
            let from = pipper.point();
            let down = (l.position[1] - from[1]) / range.max(1.);
            let allowed = range * spread / down.max(0.3) * 1.1 + 3.;
            assert!(
                gap(pipper.point(), landed) < allowed,
                "slot {slot}: {} > {allowed} at {range} ft",
                gap(pipper.point(), landed)
            );
        }
    }
}

#[test]
fn pipper_follows_sloping_terrain_under_the_rounds() {
    for flags in FLAGS {
        let l = launcher(-0.3, 0.0, 200.);
        let at = [-3000., 0., 900.];
        let mut s = world(
            flags,
            l,
            [at[0], ground_slope(at[0], at[2]), at[2]],
            &ground_slope,
        );
        for slot in 0..3 {
            let station = s.own().gunship.as_ref().unwrap().stations[slot].unwrap();
            let weapon = s.own().configuration().stations[station].weapon.clone();
            let (id, _) = straightest_id(&weapon, l.basis.forward, station);
            let (pipper, _, landed, _) = fire_and_compare(&mut s, l, slot, Some(id), &ground_slope);
            assert!(matches!(pipper, Impact::Ground { .. }), "{pipper:?}");
            assert!(
                gap(pipper.point(), landed) < 2.,
                "flags {flags:x} slot {slot}: {:?} vs {landed:?}",
                pipper.point()
            );
        }
    }
}
