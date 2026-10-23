//! Where a linked AC-130 gun's rounds land if it fires now, at its actual train.
//!
//! The AC-130 gunsight's pipper (docs/spec/ac130-linked-guns.md, plan
//! section 2.5). A pure, deterministic march of the trajectory the live
//! simulation flies: the same muzzle pose, launch speed, speed command, fall,
//! service cadence and round life, through [`Round`], the one projectile step
//! this crate keeps outside the combat tick. Nothing here reads or writes
//! combat state, the renderer or the clock, so the host and a replay agree on
//! every digit. Provenance: **opinionated (John, 2026-10-09)**.
//!
//! The march follows the gun's centre line. A fired round also gets up to a
//! quarter of a degree of random spread ([`super::live::projectile_launch_direction`]),
//! which a pipper cannot show.
//!
//! Three ends are possible, each reported with the point, the time of flight
//! and the straight-line range from the muzzle:
//!
//! - [`Impact::Ground`]: the round reaches the terrain first.
//! - [`Impact::Air`]: for an observed target, the round's travelled distance
//!   first catches up with the distance to that target's future position, the
//!   rule [`super::gunsight::solve_observed`] applies. The point is where the
//!   round is, minus the target's velocity times the time of flight, so a
//!   correctly led gun puts the pipper on the target's present position.
//! - [`Impact::Spent`]: the round's life ends in the air (or the march limit
//!   is reached). The point is the last position.
use super::{
    gun_round::{Round, service_ticks},
    gunship,
    gunsight::TargetObservation,
    live::{Launcher, terrain_hit},
};
use crate::attitude::Vector;
use tore_formats::weapons::Weapon;

/// One combat tick, in seconds.
const DT: f64 = 1. / 120.;
/// The longest march: 30 seconds. Guns live 10 seconds or less (a record's
/// `remove_t` counts quarter seconds); this only bounds a damaged record.
const MAX_TICKS: u64 = 3600;

/// How a gun's centre-line round ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Impact {
    /// The terrain contact point.
    Ground {
        point: Vector,
        seconds: f64,
        range_ft: f64,
    },
    /// The lead point against an observed target (see the module notes).
    /// `range_ft` is the round's distance from the muzzle at the crossing.
    Air {
        point: Vector,
        seconds: f64,
        range_ft: f64,
    },
    /// The round's last position when its life ran out.
    Spent {
        point: Vector,
        seconds: f64,
        range_ft: f64,
    },
}

impl Impact {
    pub fn point(&self) -> Vector {
        match *self {
            Self::Ground { point, .. } | Self::Air { point, .. } | Self::Spent { point, .. } => {
                point
            }
        }
    }
    pub fn seconds(&self) -> f64 {
        match *self {
            Self::Ground { seconds, .. }
            | Self::Air { seconds, .. }
            | Self::Spent { seconds, .. } => seconds,
        }
    }
    pub fn range_ft(&self) -> f64 {
        match *self {
            Self::Ground { range_ft, .. }
            | Self::Air { range_ft, .. }
            | Self::Spent { range_ft, .. } => range_ft,
        }
    }
}

/// What the march needs to know about the moment of firing.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Shot {
    /// The target whose motion the guns lead (an aircraft, or any moving
    /// object), or `None` for the ground, a pinned point or free slew.
    pub target: Option<TargetObservation>,
    /// The combat tick the round would be released at: the number a world
    /// reports before it steps. Rounds are advanced on a 15-tick cadence of
    /// two or three service units and live a whole number of 30-tick quarter
    /// seconds counted from their launch, so the answer depends on it by a
    /// few feet.
    pub launch_tick: u64,
}

/// The impact of gun `slot` (0 to 2, [`gunship::GUNS`] order) at
/// `heading` and `elevation` (radians, mount-local, as
/// [`gunship::State`] keeps them), fired from `launcher` over `terrain`
/// (height by x and z).
///
/// Returns `None` for an invalid slot, a non-finite input or a weapon whose
/// speed limits are inverted.
pub fn impact(
    slot: usize,
    weapon: &Weapon,
    launcher: &Launcher,
    heading: f64,
    elevation: f64,
    shot: Shot,
    terrain: &impl Fn(f64, f64) -> f64,
) -> Option<Impact> {
    if slot >= gunship::GUNS.len() || !heading.is_finite() || !elevation.is_finite() {
        return None;
    }
    if !launcher
        .position
        .iter()
        .chain(&launcher.basis.right)
        .chain(&launcher.basis.up)
        .chain(&launcher.basis.forward)
        .all(|v| v.is_finite())
        || !launcher.speed_fps.is_finite()
    {
        return None;
    }
    march(
        weapon,
        gunship::muzzle(slot, *launcher, heading, elevation),
        gunship::direction(*launcher, heading, elevation),
        launcher.speed_fps,
        shot,
        terrain,
    )
}

/// The march itself, from `muzzle` along the unit vector `direction` with the
/// launcher flying at `launcher_speed_fps`. Public so a caller can march a
/// specific spread direction or a non-gunship barrel.
pub fn march(
    weapon: &Weapon,
    muzzle: Vector,
    direction: Vector,
    launcher_speed_fps: f64,
    shot: Shot,
    terrain: &impl Fn(f64, f64) -> f64,
) -> Option<Impact> {
    let Shot {
        target,
        launch_tick,
    } = shot;
    if !muzzle.iter().chain(&direction).all(|v| v.is_finite())
        || target.is_some_and(|t| !t.position.iter().chain(&t.velocity).all(|v| v.is_finite()))
    {
        return None;
    }
    let mut round = Round::aimed(weapon, muzzle, direction, launcher_speed_fps, launch_tick)?;
    let straight_up = |travel: f64| muzzle[1] + direction[1] * travel;
    let mut travel = 0.;
    // The range-crossing function of `gunsight::solve_observed`: how far the
    // round has come minus how far the drop-compensated target is away.
    let crossing = |travel: f64, drop: f64, seconds: f64| {
        target.map_or(f64::NEG_INFINITY, |t| {
            let future: Vector = std::array::from_fn(|i| {
                t.position[i] + t.velocity[i] * seconds - muzzle[i] + if i == 1 { drop } else { 0. }
            });
            travel - future.iter().map(|v| v * v).sum::<f64>().sqrt()
        })
    };
    let mut before_crossing = crossing(0., 0., 0.);
    let range = |p: Vector| {
        (0..3)
            .map(|i| (p[i] - muzzle[i]).powi(2))
            .sum::<f64>()
            .sqrt()
    };
    for k in 0..MAX_TICKS {
        let tick = launch_tick + k;
        if round.expired(tick) {
            return Some(Impact::Spent {
                point: round.position,
                seconds: k as f64 * DT,
                range_ft: range(round.position),
            });
        }
        let start = round.position;
        let alive = round.step(tick, terrain);
        travel += f64::from(round.speed_f8) * f64::from(service_ticks(tick)) / 65536.;
        let seconds = (k + 1) as f64 * DT;
        let drop = straight_up(travel) - round.position[1];
        let now_crossing = crossing(travel, drop, seconds);
        let lerp_point = |at: f64| -> Vector {
            std::array::from_fn(|i| start[i] + (round.position[i] - start[i]) * at)
        };
        // Both ends within the one tick: the earlier wins.
        let air = (before_crossing < 0. && now_crossing >= 0.)
            .then(|| (-before_crossing / (now_crossing - before_crossing)).clamp(0., 1.));
        let ground = if alive {
            None
        } else if let Some(at) = terrain_hit(start, round.position, terrain) {
            Some(at)
        } else {
            // Not the ground: the record's speed command was refused.
            return Some(Impact::Spent {
                point: round.position,
                seconds,
                range_ft: range(round.position),
            });
        };
        match (air, ground) {
            (Some(a), g) if g.is_none_or(|g| a < g) => {
                let velocity = target.map_or([0.; 3], |t| t.velocity);
                let flight = k as f64 * DT + a * DT;
                let at = lerp_point(a);
                return Some(Impact::Air {
                    point: std::array::from_fn(|i| at[i] - velocity[i] * flight),
                    seconds: flight,
                    range_ft: range(at),
                });
            }
            (_, Some(g)) => {
                let at = lerp_point(g);
                return Some(Impact::Ground {
                    point: at,
                    seconds: k as f64 * DT + g * DT,
                    range_ft: range(at),
                });
            }
            _ => {}
        }
        before_crossing = now_crossing;
    }
    Some(Impact::Spent {
        point: round.position,
        seconds: MAX_TICKS as f64 * DT,
        range_ft: range(round.position),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        attitude::{Basis, unit},
        combat::gunsight::{self, tests::weapon},
        sensors,
    };
    use std::f64::consts::FRAC_PI_2;

    fn launcher(basis: Basis, speed_fps: f64) -> Launcher {
        Launcher {
            radar_power: true,
            position: [100., 4000., 200.],
            basis,
            speed_fps,
            velocity: [0.; 3],
            bay_ready: true,
            radar: true,
            jammer: false,
            alive: true,
            body_present: true,
            controls: sensors::Controls::default(),
        }
    }
    fn level() -> Launcher {
        launcher(Basis::new(0., 0., 0.), 300.)
    }
    fn shot() -> Shot {
        Shot::default()
    }
    fn flat(_: f64, _: f64) -> f64 {
        0.
    }
    fn dist(a: Vector, b: Vector) -> f64 {
        (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
    }

    #[test]
    fn flat_ground_impact_lies_on_the_terrain_ahead_of_the_barrel() {
        let w = weapon();
        let l = level();
        let (heading, elevation) = (-FRAC_PI_2, -0.35_f64);
        let hit = impact(1, &w, &l, heading, elevation, shot(), &flat).unwrap();
        let Impact::Ground { point, seconds, .. } = hit else {
            panic!("{hit:?}");
        };
        assert!(point[1].abs() < 0.01);
        // Left of the aircraft (negative x), downrange of the muzzle.
        assert!(point[0] < l.position[0] - 3000.);
        let muzzle = gunship::muzzle(1, l, heading, elevation);
        let straight = (muzzle[1] / elevation.abs().sin()) / 1000.;
        // Gravity makes it land shorter than the straight line (about 1000 fps).
        assert!(seconds > 0. && seconds < straight, "{seconds} {straight}");
        assert_eq!(
            hit,
            impact(1, &w, &l, heading, elevation, shot(), &flat).unwrap()
        );
    }

    #[test]
    fn impact_follows_the_slope_of_the_terrain() {
        let w = weapon();
        let l = level();
        let slope = |x: f64, _: f64| -0.1 * x;
        let hit = impact(0, &w, &l, -FRAC_PI_2, -0.3, shot(), &slope).unwrap();
        let Impact::Ground { point, .. } = hit else {
            panic!("{hit:?}");
        };
        assert!(
            (point[1] - slope(point[0], point[2])).abs() < 0.01,
            "{point:?}"
        );
        let level_hit = impact(0, &w, &l, -FRAC_PI_2, -0.3, shot(), &flat).unwrap();
        assert_ne!(point, level_hit.point());
    }

    #[test]
    fn guns_land_apart_and_each_follows_its_own_pose() {
        let w = weapon();
        let l = level();
        let points: Vec<_> = (0..3)
            .map(|slot| {
                impact(slot, &w, &l, -FRAC_PI_2, -0.4, shot(), &flat)
                    .unwrap()
                    .point()
            })
            .collect();
        assert!(dist(points[0], points[1]) > 5. && dist(points[1], points[2]) > 5.);
        // Aft guns are behind the forward one on the ground.
        assert!(points[0][2] > points[1][2] && points[1][2] > points[2][2]);
    }

    #[test]
    fn rounds_with_life_left_report_spent_at_their_launch_phase_dependent_end() {
        let mut w = weapon();
        w.movement.remove_t = 8;
        let l = level();
        let sky = |_: f64, _: f64| -1.0e9;
        for (launch, steps) in [(0, 240), (29, 211), (30, 240), (44, 226)] {
            let hit = impact(
                1,
                &w,
                &l,
                -FRAC_PI_2,
                0.2,
                Shot {
                    launch_tick: launch,
                    ..shot()
                },
                &sky,
            )
            .unwrap();
            let Impact::Spent { seconds, .. } = hit else {
                panic!("{hit:?}");
            };
            assert!(
                (seconds - steps as f64 / 120.).abs() < 1e-9,
                "{launch} {seconds}"
            );
        }
    }

    #[test]
    fn spent_point_is_the_last_position_and_a_longer_life_flies_farther() {
        let mut short = weapon();
        short.movement.remove_t = 8;
        let long = weapon();
        let l = level();
        let sky = |_: f64, _: f64| -1.0e9;
        let a = impact(1, &short, &l, -FRAC_PI_2, 0.1, shot(), &sky).unwrap();
        let b = impact(1, &long, &l, -FRAC_PI_2, 0.1, shot(), &sky).unwrap();
        assert!(matches!(a, Impact::Spent { .. }) && matches!(b, Impact::Spent { .. }));
        assert!(a.range_ft() > 1900. && b.range_ft() > a.range_ft() * 1.5);
        assert!(a.seconds() < b.seconds());
    }

    #[test]
    fn invalid_inputs_give_no_impact() {
        let w = weapon();
        let l = level();
        assert!(impact(3, &w, &l, -FRAC_PI_2, 0., shot(), &flat).is_none());
        assert!(impact(0, &w, &l, f64::NAN, 0., shot(), &flat).is_none());
        let mut bad = l;
        bad.position[0] = f64::INFINITY;
        assert!(impact(0, &w, &bad, -FRAC_PI_2, 0., shot(), &flat).is_none());
        let mut inverted = weapon();
        inverted.movement.minimum_speed = 3000;
        assert!(impact(0, &inverted, &l, -FRAC_PI_2, 0., shot(), &flat).is_none());
        let nan_target = TargetObservation {
            position: [f64::NAN, 0., 0.],
            velocity: [0.; 3],
        };
        assert!(
            impact(
                0,
                &w,
                &l,
                -FRAC_PI_2,
                0.,
                Shot {
                    target: Some(nan_target),
                    ..shot()
                },
                &flat
            )
            .is_none()
        );
    }

    #[test]
    fn launcher_speed_changes_the_impact_only_through_the_launch_retard() {
        let mut w = weapon();
        let slow = launcher(Basis::new(0., 0., 0.), 0.);
        let fast = launcher(Basis::new(0., 0., 0.), 1500.);
        let a = impact(1, &w, &slow, -FRAC_PI_2, -0.3, shot(), &flat).unwrap();
        let b = impact(1, &w, &fast, -FRAC_PI_2, -0.3, shot(), &flat).unwrap();
        assert_eq!(a, b);
        w.movement.launch_retard = 100;
        let a = impact(1, &w, &slow, -FRAC_PI_2, -0.3, shot(), &flat).unwrap();
        let b = impact(1, &w, &fast, -FRAC_PI_2, -0.3, shot(), &flat).unwrap();
        assert!(b.seconds() < a.seconds());
    }

    #[test]
    fn banked_and_pitched_aircraft_aim_the_pipper_by_the_aircraft_frame() {
        let w = weapon();
        let level_hit = impact(1, &w, &level(), -FRAC_PI_2, -0.5, shot(), &flat).unwrap();
        let banked = launcher(Basis::new(0., 0., -0.5), 300.);
        let banked_hit = impact(1, &w, &banked, -FRAC_PI_2, -0.5, shot(), &flat).unwrap();
        // A left bank (negative) lowers the left side's barrels: it lands closer.
        assert!(banked_hit.range_ft() < level_hit.range_ft());
        let bank_vector = unit(gunship::direction(banked, -FRAC_PI_2, -0.5));
        assert!(bank_vector[1] < gunship::direction(level(), -FRAC_PI_2, -0.5)[1]);
    }

    /// Trains gun `slot` on `target` the way [`gunship::State::update`] does,
    /// until the angles stop moving, with no arc limits.
    fn trained(slot: usize, w: &Weapon, l: &Launcher, target: TargetObservation) -> (f64, f64) {
        let (mut heading, mut elevation) = (-FRAC_PI_2, 0.);
        for _ in 0..40 {
            let mount = gunship::local_muzzle(slot, heading, elevation);
            let solution = gunsight::solve_observed(w, l, mount, Some(target))
                .unwrap()
                .unwrap();
            let pivot = gunship::world_mount(*l, gunship::pivot(slot));
            let toward: Vector = std::array::from_fn(|i| {
                target.position[i]
                    + target.velocity[i] * solution.seconds
                    + if i == 1 { solution.drop_ft } else { 0. }
                    - pivot[i]
            });
            let right = crate::attitude::dot(toward, l.basis.right);
            let forward = crate::attitude::dot(toward, l.basis.forward);
            let up = crate::attitude::dot(toward, l.basis.up);
            heading = right.atan2(forward);
            elevation = up.atan2(right.hypot(forward));
        }
        (heading, elevation)
    }

    #[test]
    fn trained_guns_put_the_air_pipper_on_the_target_and_agree_with_the_lead_rule() {
        let w = weapon();
        for (basis, velocity) in [
            (Basis::new(0., 0., 0.), [0.; 3]),
            (Basis::new(0.2, 0.05, -0.4), [60., 0., -200.]),
            (Basis::new(-0.3, 0., 0.3), [-150., 20., 100.]),
        ] {
            let l = launcher(basis, 250.);
            for slot in 0..3 {
                let target = TargetObservation {
                    position: [-1300., 3600., 400.],
                    velocity,
                };
                let (heading, elevation) = trained(slot, &w, &l, target);
                let mount = gunship::local_muzzle(slot, heading, elevation);
                let rule = gunsight::solve_observed(&w, &l, mount, Some(target))
                    .unwrap()
                    .unwrap();
                let hit = impact(
                    slot,
                    &w,
                    &l,
                    heading,
                    elevation,
                    Shot {
                        target: Some(target),
                        launch_tick: 7,
                    },
                    &flat,
                )
                .unwrap();
                let Impact::Air { point, seconds, .. } = hit else {
                    panic!("{hit:?}");
                };
                // On the target's present position, and the rule's flight time.
                assert!(dist(point, target.position) < 6., "slot {slot}: {hit:?}");
                assert!(
                    (seconds - rule.seconds).abs() < 0.02,
                    "{seconds} {}",
                    rule.seconds
                );
            }
        }
    }

    #[test]
    fn an_untrained_air_pipper_misses_the_target_by_the_train_error() {
        let w = weapon();
        let l = level();
        let target = TargetObservation {
            position: [-1300., 3600., 400.],
            velocity: [0.; 3],
        };
        let (heading, elevation) = trained(1, &w, &l, target);
        let off = 3_f64.to_radians();
        let hit = impact(
            1,
            &w,
            &l,
            heading + off,
            elevation,
            Shot {
                target: Some(target),
                ..shot()
            },
            &flat,
        )
        .unwrap();
        let miss = dist(hit.point(), target.position);
        // Three degrees at about 1,400 ft is about 75 ft.
        assert!((50. ..110.).contains(&miss), "{miss}");
    }

    #[test]
    fn terrain_in_front_of_a_target_ends_the_march_on_the_ground() {
        let w = weapon();
        let l = level();
        let target = TargetObservation {
            position: [-3000., 3900., 200.],
            velocity: [0.; 3],
        };
        let wall = |x: f64, _: f64| if x < -700. { 5000. } else { 0. };
        let (heading, elevation) = trained(1, &w, &l, target);
        let hit = impact(
            1,
            &w,
            &l,
            heading,
            elevation,
            Shot {
                target: Some(target),
                ..shot()
            },
            &wall,
        )
        .unwrap();
        assert!(matches!(hit, Impact::Ground { .. }), "{hit:?}");
        assert!(hit.point()[0] > -800.);
    }

    #[test]
    fn a_target_beyond_the_round_life_leaves_the_round_spent() {
        let mut w = weapon();
        w.movement.remove_t = 8;
        let l = level();
        let target = TargetObservation {
            position: [-9000., 4000., 0.],
            velocity: [0.; 3],
        };
        let hit = impact(
            1,
            &w,
            &l,
            -FRAC_PI_2,
            0.,
            Shot {
                target: Some(target),
                ..shot()
            },
            &|_, _| -1.0e9,
        )
        .unwrap();
        assert!(matches!(hit, Impact::Spent { .. }), "{hit:?}");
    }

    /// Timing note for the plan (section 2.5): the cost of one pipper.
    #[test]
    fn a_pipper_costs_far_less_than_a_tick() {
        let w = weapon();
        let l = level();
        let calls = 2000usize;
        let begin = std::time::Instant::now();
        let mut sink = 0.;
        for i in 0..calls {
            let elevation = -0.15 - 0.4 * ((i % 50) as f64) / 50.;
            sink += impact(
                i % 3,
                &w,
                &l,
                -FRAC_PI_2,
                elevation,
                Shot {
                    launch_tick: i as u64,
                    ..shot()
                },
                &flat,
            )
            .unwrap()
            .seconds();
        }
        let each = begin.elapsed().as_secs_f64() / calls as f64;
        eprintln!("pipper march: {:.1} us each (sink {sink:.1})", each * 1e6);
        // A 120 Hz tick has 8.3 ms; three linked guns must stay a sliver of it.
        assert!(each < 0.002, "{each}");
    }
}
