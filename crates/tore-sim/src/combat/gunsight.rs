//! Deterministic gun-pipper ballistics, independent of rendering and live contact.
use super::{
    FALL_SPEED_CAP_F8, FallState, axial_speed, commanded_speed, engine_phase, launch_speed,
};
use crate::{attitude::Vector, combat::live::Launcher};
use tore_formats::{Result, weapons::Weapon};

const DT: f64 = 1. / 120.;
const SERVICE_RATE: u16 = 256;
const MAX_TICKS: u64 = 1200;
const VISUAL_RANGE_FT: f64 = 1000.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetObservation {
    pub position: Vector,
    pub velocity: Vector,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solution {
    pub point: Vector,
    pub seconds: f64,
    pub range_ft: f64,
    pub maximum_range_ft: f64,
    pub radar: bool,
    pub travel_ft: f64,
    pub drop_ft: f64,
    pub flight_speed_fps: f64,
    pub fall_speed_fps: f64,
    pub arc_fraction: f64,
}

/// Solve the visual 1,000-foot pipper or a radar target lead.
pub fn solve(
    weapon: &Weapon,
    launcher: &Launcher,
    mount: Vector,
    target: Option<TargetObservation>,
) -> Result<Option<Solution>> {
    let target = (launcher.radar && launcher.radar_power)
        .then_some(target)
        .flatten();
    solve_observed(weapon, launcher, mount, target)
}

/// The shared trajectory using explicitly permitted observations. Visual AI
/// measurements need no radar emission; this does not alter the player's scope.
pub fn solve_observed(
    weapon: &Weapon,
    launcher: &Launcher,
    mount: Vector,
    target: Option<TargetObservation>,
) -> Result<Option<Solution>> {
    if !launcher
        .position
        .iter()
        .chain(&launcher.velocity)
        .all(|v| v.is_finite())
        || !mount.iter().all(|v| v.is_finite())
        || !launcher.speed_fps.is_finite()
        || target.is_some_and(|t| !t.position.iter().chain(&t.velocity).all(|v| v.is_finite()))
    {
        return Err(super::invalid("nonfinite gunsight input"));
    }
    let maximum = f64::from(weapon.seeker.zones[1].maximum_range.max(0));
    if maximum <= 0. {
        return Ok(None);
    }
    let muzzle = std::array::from_fn(|i| {
        launcher.position[i]
            + launcher.basis.right[i] * mount[0]
            + launcher.basis.up[i] * mount[1]
            + launcher.basis.forward[i] * mount[2]
    });
    let radar_target = target;
    let radar = radar_target.is_some();
    let movement = &weapon.movement;
    let mut speed_f8 = launch_speed(movement, (launcher.speed_fps * 256.) as i32)? * 256;
    let mut fall = FallState::default();
    let initial_height_f8 = (muzzle[1] * 256.) as i32;
    let mut height_f8 = initial_height_f8;
    let ticks = (u64::from(movement.remove_t) * 30).min(MAX_TICKS);
    if let Some(target) = radar_target
        && weapon.flags & 0x40 == 0
        && beyond_reach(target, muzzle, speed_f8, ticks)
    {
        return Ok(None);
    }
    // A tick whose crossing cannot be non-negative needs no crossing worked
    // out until the next tick needs it as the previous one (see `apart`).
    let apart = radar_target.map(|target| Apart::new(target, muzzle));
    let mut service_remainder = 0u16;
    let mut distance = 0.;
    let mut previous = Sample {
        seconds: 0.,
        distance: 0.,
        drop: 0.,
        crossing: crossing(radar_target, muzzle, 0., 0., 0.),
        pending: false,
    };
    for tick in 0..ticks {
        service_remainder += SERVICE_RATE;
        let service = (service_remainder / 120) as i16;
        service_remainder %= 120;
        let now = (tick / 30) as u16;
        let phase = engine_phase(movement, now, 0);
        if weapon.flags & 0x40 != 0 {
            let target_speed = commanded_speed(movement, phase, speed_f8, height_f8) as i16;
            speed_f8 = axial_speed(movement, speed_f8, target_speed, false, service)?;
        }
        distance += f64::from(speed_f8) * f64::from(service) / 65536.;
        height_f8 = fall.advance(weapon.flags & 4 != 0, phase, service, height_f8)?;
        let drop = f64::from(initial_height_f8 - height_f8) / 256.;
        let seconds = (tick + 1) as f64 * DT;
        if apart.is_some_and(|apart| apart.surely(seconds, distance, drop)) {
            // Not reached: the crossing is negative. Kept without it.
            previous = Sample {
                seconds,
                distance,
                drop,
                crossing: 0.,
                pending: true,
            };
            continue;
        }
        let current = Sample {
            seconds,
            distance,
            drop,
            crossing: crossing(radar_target, muzzle, seconds, distance, drop),
            pending: false,
        };
        if previous.pending {
            // Exactly what that tick would have worked out.
            previous.crossing = crossing(
                radar_target,
                muzzle,
                previous.seconds,
                previous.distance,
                previous.drop,
            );
        }
        let reached = if radar {
            current.crossing >= 0. && previous.crossing < 0.
        } else {
            current.distance >= VISUAL_RANGE_FT
        };
        if reached {
            let fraction = if radar {
                (-previous.crossing / (current.crossing - previous.crossing)).clamp(0., 1.)
            } else {
                ((VISUAL_RANGE_FT - previous.distance)
                    / (current.distance - previous.distance).max(f64::EPSILON))
                .clamp(0., 1.)
            };
            let seconds = lerp(previous.seconds, current.seconds, fraction);
            let range = lerp(previous.distance, current.distance, fraction);
            let drop = lerp(previous.drop, current.drop, fraction);
            let target_velocity = radar_target.map_or([0.; 3], |t| t.velocity);
            let point = std::array::from_fn(|i| {
                muzzle[i] + launcher.basis.forward[i] * range
                    - if i == 1 { drop } else { 0. }
                    - target_velocity[i] * seconds
            });
            let indicated_range = radar_target.map_or(range, |target| {
                (0..3)
                    .map(|i| (target.position[i] - launcher.position[i]).powi(2))
                    .sum::<f64>()
                    .sqrt()
            });
            return Ok(Some(Solution {
                point,
                seconds,
                range_ft: indicated_range,
                maximum_range_ft: maximum,
                radar,
                travel_ft: range,
                drop_ft: drop,
                flight_speed_fps: f64::from(speed_f8) / 256.,
                fall_speed_fps: f64::from(fall.velocity_f8) / 256.,
                arc_fraction: range_arc_fraction(indicated_range, maximum),
            }));
        }
        previous = current;
    }
    Ok(None)
}

/// Whether an unpowered round can never reach `target` within its `ticks`
/// of flight, so the trajectory below could only run out and say no.
///
/// Without a motor the round keeps its launch speed, so it covers exactly
/// `speed_f8` times the service ticks it is given (the sum below is the
/// same sum of multiples of 2^-16 the loop adds, which f64 holds exactly);
/// its drop never exceeds the capped fall speed for those service ticks; and
/// the target is never closer than its distance now less what it can fly in
/// that time. When even all of that leaves a gap (with a foot and a
/// relative 1e-9 to spare for rounding in the distances), no tick's crossing
/// can turn non-negative.
fn beyond_reach(target: TargetObservation, muzzle: Vector, speed_f8: i32, ticks: u64) -> bool {
    if speed_f8 < 0 {
        return false;
    }
    let service = (ticks * u64::from(SERVICE_RATE) / 120) as f64;
    let reach = f64::from(speed_f8) * service / 65536.;
    let drop = f64::from(FALL_SPEED_CAP_F8) * service / 65536.;
    let gap = (0..3)
        .map(|i| (target.position[i] - muzzle[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    let closing = target.velocity.iter().map(|v| v * v).sum::<f64>().sqrt() * ticks as f64 * DT;
    let slack = 1. + 1e-9 * (gap + reach + drop + closing);
    reach + drop + closing + slack < gap
}

/// A bound on how near a round and the target can be: the target is never
/// nearer than its distance from the muzzle now less how far it can fly and
/// how far the round has dropped. While the round has flown less than that
/// (with a foot and a relative 1e-9 to spare for rounding), its crossing is
/// negative.
#[derive(Clone, Copy)]
struct Apart {
    gap: f64,
    target_speed: f64,
}
impl Apart {
    fn new(target: TargetObservation, muzzle: Vector) -> Self {
        Self {
            gap: (0..3)
                .map(|i| (target.position[i] - muzzle[i]).powi(2))
                .sum::<f64>()
                .sqrt(),
            target_speed: target.velocity.iter().map(|v| v * v).sum::<f64>().sqrt(),
        }
    }
    /// Whether the crossing at this sample is surely negative.
    fn surely(self, seconds: f64, distance: f64, drop: f64) -> bool {
        let closing = self.target_speed * seconds;
        let near = distance.abs() + closing + drop.abs();
        near + 1. + 1e-9 * (self.gap + near) < self.gap
    }
}

#[derive(Clone, Copy)]
struct Sample {
    seconds: f64,
    distance: f64,
    drop: f64,
    crossing: f64,
    /// The crossing is not worked out yet (it is surely negative).
    pending: bool,
}

fn crossing(
    target: Option<TargetObservation>,
    muzzle: Vector,
    seconds: f64,
    distance: f64,
    drop: f64,
) -> f64 {
    let Some(target) = target else {
        return distance - VISUAL_RANGE_FT;
    };
    let future: Vector = std::array::from_fn(|i| {
        target.position[i] + target.velocity[i] * seconds - muzzle[i]
            + if i == 1 { drop } else { 0. }
    });
    distance - future.iter().map(|v| v * v).sum::<f64>().sqrt()
}

fn lerp(a: f64, b: f64, fraction: f64) -> f64 {
    a + (b - a) * fraction
}

/// Manual anchor interpolation: absent at maximum range, half at half range,
/// and complete at 100 feet or closer.
pub fn range_arc_fraction(range_ft: f64, maximum_range_ft: f64) -> f64 {
    if !range_ft.is_finite() || !maximum_range_ft.is_finite() || maximum_range_ft <= 0. {
        return 0.;
    }
    let range = range_ft.max(0.);
    if range >= maximum_range_ft {
        0.
    } else if range <= 100. || maximum_range_ft <= 200. {
        1.
    } else if range >= maximum_range_ft * 0.5 {
        (maximum_range_ft - range) / (maximum_range_ft * 0.5) * 0.5
    } else {
        0.5 + (maximum_range_ft * 0.5 - range) / (maximum_range_ft * 0.5 - 100.) * 0.5
    }
    .clamp(0., 1.)
}

/// Where the gun's rounds meet the ground: the strafing cue (AI strike
/// round, slice ST-T2). The same round path as [`solve_observed`] (the
/// record's speed, the bomb fall for a gravity round, along the barrel from
/// the mount), followed until it reaches the ground under it or its range
/// runs out. Returns the point, the seconds of flight and the slant range.
pub fn ground_cue(
    weapon: &Weapon,
    launcher: &Launcher,
    mount: Vector,
    ground: impl Fn(f64, f64) -> f64,
) -> Result<Option<(Vector, f64, f64)>> {
    if !launcher
        .position
        .iter()
        .chain(&launcher.velocity)
        .all(|v| v.is_finite())
        || !mount.iter().all(|v| v.is_finite())
        || !launcher.speed_fps.is_finite()
    {
        return Err(super::invalid("nonfinite gunsight input"));
    }
    let maximum = f64::from(weapon.seeker.zones[1].maximum_range.max(0));
    if maximum <= 0. {
        return Ok(None);
    }
    let muzzle: Vector = std::array::from_fn(|i| {
        launcher.position[i]
            + launcher.basis.right[i] * mount[0]
            + launcher.basis.up[i] * mount[1]
            + launcher.basis.forward[i] * mount[2]
    });
    let movement = &weapon.movement;
    let mut speed_f8 = launch_speed(movement, (launcher.speed_fps * 256.) as i32)? * 256;
    let mut fall = FallState::default();
    let initial_height_f8 = (muzzle[1] * 256.) as i32;
    let mut height_f8 = initial_height_f8;
    let ticks = (u64::from(movement.remove_t) * 30).min(MAX_TICKS);
    let mut service_remainder = 0u16;
    let mut distance = 0.;
    let at = |distance: f64, drop: f64| -> Vector {
        std::array::from_fn(|i| {
            muzzle[i] + launcher.basis.forward[i] * distance - if i == 1 { drop } else { 0. }
        })
    };
    let mut previous = (0., muzzle[1] - ground(muzzle[0], muzzle[2]), muzzle);
    for tick in 0..ticks {
        service_remainder += SERVICE_RATE;
        let service = (service_remainder / 120) as i16;
        service_remainder %= 120;
        let now = (tick / 30) as u16;
        let phase = engine_phase(movement, now, 0);
        if weapon.flags & 0x40 != 0 {
            let target_speed = commanded_speed(movement, phase, speed_f8, height_f8) as i16;
            speed_f8 = axial_speed(movement, speed_f8, target_speed, false, service)?;
        }
        distance += f64::from(speed_f8) * f64::from(service) / 65536.;
        height_f8 = fall.advance(weapon.flags & 4 != 0, phase, service, height_f8)?;
        let drop = f64::from(initial_height_f8 - height_f8) / 256.;
        let seconds = (tick + 1) as f64 * DT;
        let point = at(distance, drop);
        let above = point[1] - ground(point[0], point[2]);
        if above <= 0. {
            let fraction = (previous.1 / (previous.1 - above).max(f64::EPSILON)).clamp(0., 1.);
            let hit: Vector =
                std::array::from_fn(|i| previous.2[i] + (point[i] - previous.2[i]) * fraction);
            let range = (0..3)
                .map(|i| (hit[i] - muzzle[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            return Ok(Some((hit, lerp(previous.0, seconds, fraction), range)));
        }
        if distance > maximum * 2. {
            return Ok(None);
        }
        previous = (seconds, above, point);
    }
    Ok(None)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{attitude::Basis, sensors};
    use tore_formats::{aircraft::AircraftId, weapons::*};

    pub(crate) fn weapon() -> Weapon {
        let zone = Zone {
            heading: 12000,
            pitch: 12000,
            minimum_range: 0,
            maximum_range: 2000,
            minimum_altitude: i32::MIN,
            maximum_altitude: i32::MAX,
        };
        Weapon {
            source: "SYNTHETIC.JT".into(),
            name: "Synthetic gun".into(),
            hud_name: "GUN".into(),
            shape: None,
            fire_sound: None,
            native_callback: "_PROJProc".into(),
            flags: 4,
            object_flags: 0,
            weight: 10,
            movement: Movement {
                minimum_speed: 10,
                corner_speed: 1000,
                maximum_speed: 2000,
                acceleration: 100,
                deceleration: 2,
                initial_speed: 1000,
                final_speed: 500,
                launch_retard: 0,
                ignite_t: 0,
                fuel_t: 0,
                remove_t: 40,
                powered_turn_rate: 0,
                unpowered_turn_rate: 0,
                performance_at_0: 100,
                performance_at_20: 100,
                cruise: [0; 4],
                jink: [0; 3],
            },
            burst: Burst {
                projectiles_in_pod: 1,
                actual_rounds_per_game: 1,
                game_rounds_in_burst: 1,
                game_rounds_in_carpet_burst: 1,
                game_burst_t: 1,
                reload_t: 0,
                startup_shots: 0,
                random_fire_percent: 0,
                offset_fire_percent: 0,
                offset_fire_heading: 0,
                offset_fire_pitch: 0,
                sine_pattern: [0; 4],
            },
            seeker: Seeker {
                flags: [0; 2],
                signature: 0,
                look_down: 0,
                doppler_above: 0,
                doppler_below: 0,
                doppler_minimum_range: 0,
                all_aspect: 0,
                zones: [zone; 2],
                chaff_flare_chance: 0,
                deception_chance: 0,
            },
            guidance: Guidance {
                track_t: 0,
                track_max_g_raw: 0,
                target_sun_chance: 0,
                max_aon: 0,
                chances: [0; 4],
                hit_modifiers: [0; 9],
            },
            damage: Damage {
                by_class: [10; 5],
                fuze_arm_t: 0,
                fuze_radius: 0,
                side_hit_fuze_failure: 0,
                collateral_radius: 0,
                collateral_percent: 0,
            },
            effects: Effects {
                object_explosion: 0,
                land_explosion: 0,
                water_explosion: 0,
                crater_size: 0,
                smoke: [0; 5],
                max_sound_distance: 0,
                frequency_adjustment: 0,
            },
        }
    }

    fn launcher() -> Launcher {
        Launcher {
            radar_power: true,
            position: [100., 5000., 200.],
            basis: Basis::new(0., 0., 0.),
            speed_fps: 300.,
            velocity: [900., 0., -500.],
            bay_ready: true,
            radar: false,
            jammer: false,
            alive: true,
            body_present: true,
            controls: sensors::Controls::default(),
        }
    }

    #[test]
    fn visual_solution_is_stationary_bore_at_one_thousand_feet_with_drop() {
        let weapon = weapon();
        let launcher = launcher();
        let solution = solve(&weapon, &launcher, [2., 3., 4.], None)
            .unwrap()
            .unwrap();
        let muzzle = [102., 5003., 204.];
        assert!(!solution.radar);
        assert!((solution.range_ft - 1000.).abs() < 1e-9);
        assert_eq!(solution.maximum_range_ft, 2000.);
        assert_eq!(solution.point[0], muzzle[0]);
        assert!((solution.point[2] - muzzle[2] - 1000.).abs() < 1e-9);
        assert!(solution.point[1] < muzzle[1]);
        // Lateral launcher velocity is deliberately absent from the trajectory.
        let mut other = launcher;
        other.velocity = [-5000., 2000., 7000.];
        assert_eq!(
            solve(&weapon, &other, [2., 3., 4.], None).unwrap(),
            Some(solution)
        );
    }

    #[test]
    fn radar_solution_uses_target_motion_gravity_and_scalar_launch_speed() {
        let mut weapon = weapon();
        let mut launcher = launcher();
        launcher.radar = true;
        let target = TargetObservation {
            position: [
                launcher.position[0],
                launcher.position[1],
                launcher.position[2] + 800.,
            ],
            velocity: [120., 0., 0.],
        };
        let moving = solve(&weapon, &launcher, [0.; 3], Some(target))
            .unwrap()
            .unwrap();
        let stationary = solve(
            &weapon,
            &launcher,
            [0.; 3],
            Some(TargetObservation {
                velocity: [0.; 3],
                ..target
            }),
        )
        .unwrap()
        .unwrap();
        assert!(moving.radar && moving.point[0] < stationary.point[0]);
        assert_eq!(moving.range_ft, stationary.range_ft);
        assert!(moving.seconds > stationary.seconds);
        assert!(moving.point[1] < launcher.position[1]);

        weapon.movement.launch_retard = 100;
        launcher.speed_fps = 1500.;
        let fast = solve(&weapon, &launcher, [0.; 3], Some(target))
            .unwrap()
            .unwrap();
        launcher.speed_fps = 0.;
        let slow = solve(&weapon, &launcher, [0.; 3], Some(target))
            .unwrap()
            .unwrap();
        assert!(fast.seconds < slow.seconds);
    }

    /// [`solve_observed`] as it was before the out-of-reach shortcut.
    fn solve_observed_reference(
        weapon: &Weapon,
        launcher: &Launcher,
        mount: Vector,
        target: Option<TargetObservation>,
    ) -> Result<Option<Solution>> {
        if !launcher
            .position
            .iter()
            .chain(&launcher.velocity)
            .all(|v| v.is_finite())
            || !mount.iter().all(|v| v.is_finite())
            || !launcher.speed_fps.is_finite()
            || target.is_some_and(|t| !t.position.iter().chain(&t.velocity).all(|v| v.is_finite()))
        {
            return Err(crate::combat::invalid("nonfinite gunsight input"));
        }
        let maximum = f64::from(weapon.seeker.zones[1].maximum_range.max(0));
        if maximum <= 0. {
            return Ok(None);
        }
        let muzzle = std::array::from_fn(|i| {
            launcher.position[i]
                + launcher.basis.right[i] * mount[0]
                + launcher.basis.up[i] * mount[1]
                + launcher.basis.forward[i] * mount[2]
        });
        let radar_target = target;
        let radar = radar_target.is_some();
        let movement = &weapon.movement;
        let mut speed_f8 = launch_speed(movement, (launcher.speed_fps * 256.) as i32)? * 256;
        let mut fall = FallState::default();
        let initial_height_f8 = (muzzle[1] * 256.) as i32;
        let mut height_f8 = initial_height_f8;
        let ticks = (u64::from(movement.remove_t) * 30).min(MAX_TICKS);
        let mut service_remainder = 0u16;
        let mut distance = 0.;
        let mut previous = Sample {
            seconds: 0.,
            distance: 0.,
            drop: 0.,
            crossing: crossing(radar_target, muzzle, 0., 0., 0.),
            pending: false,
        };
        for tick in 0..ticks {
            service_remainder += SERVICE_RATE;
            let service = (service_remainder / 120) as i16;
            service_remainder %= 120;
            let now = (tick / 30) as u16;
            let phase = engine_phase(movement, now, 0);
            if weapon.flags & 0x40 != 0 {
                let target_speed = commanded_speed(movement, phase, speed_f8, height_f8) as i16;
                speed_f8 = axial_speed(movement, speed_f8, target_speed, false, service)?;
            }
            distance += f64::from(speed_f8) * f64::from(service) / 65536.;
            height_f8 = fall.advance(weapon.flags & 4 != 0, phase, service, height_f8)?;
            let drop = f64::from(initial_height_f8 - height_f8) / 256.;
            let seconds = (tick + 1) as f64 * DT;
            let current = Sample {
                seconds,
                distance,
                drop,
                crossing: crossing(radar_target, muzzle, seconds, distance, drop),
                pending: false,
            };
            let reached = if radar {
                current.crossing >= 0. && previous.crossing < 0.
            } else {
                current.distance >= VISUAL_RANGE_FT
            };
            if reached {
                let fraction = if radar {
                    (-previous.crossing / (current.crossing - previous.crossing)).clamp(0., 1.)
                } else {
                    ((VISUAL_RANGE_FT - previous.distance)
                        / (current.distance - previous.distance).max(f64::EPSILON))
                    .clamp(0., 1.)
                };
                let seconds = lerp(previous.seconds, current.seconds, fraction);
                let range = lerp(previous.distance, current.distance, fraction);
                let drop = lerp(previous.drop, current.drop, fraction);
                let target_velocity = radar_target.map_or([0.; 3], |t| t.velocity);
                let point = std::array::from_fn(|i| {
                    muzzle[i] + launcher.basis.forward[i] * range
                        - if i == 1 { drop } else { 0. }
                        - target_velocity[i] * seconds
                });
                let indicated_range = radar_target.map_or(range, |target| {
                    (0..3)
                        .map(|i| (target.position[i] - launcher.position[i]).powi(2))
                        .sum::<f64>()
                        .sqrt()
                });
                return Ok(Some(Solution {
                    point,
                    seconds,
                    range_ft: indicated_range,
                    maximum_range_ft: maximum,
                    radar,
                    travel_ft: range,
                    drop_ft: drop,
                    flight_speed_fps: f64::from(speed_f8) / 256.,
                    fall_speed_fps: f64::from(fall.velocity_f8) / 256.,
                    arc_fraction: range_arc_fraction(indicated_range, maximum),
                }));
            }
            previous = current;
        }
        Ok(None)
    }

    #[test]
    fn the_out_of_reach_shortcuts_give_what_the_full_trajectory_gave() {
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1_u64 << 53) as f64
        };
        let mut shortcut = 0;
        let mut found = 0;
        for case in 0..6000 {
            let mut weapon = weapon();
            weapon.flags = [0, 4, 4, 0x44, 0x40][case % 5];
            weapon.movement.remove_t = [1, 10, 40, 200, 0][(case / 5) % 5];
            weapon.movement.initial_speed = (next() * 3000.) as i16;
            weapon.movement.minimum_speed = (next() * 200.) as i16 - 50;
            weapon.movement.maximum_speed = weapon.movement.minimum_speed + (next() * 4000.) as i16;
            weapon.movement.launch_retard = [0, 50, 100][case % 3];
            weapon.seeker.zones[1].maximum_range = [2000, 0, 7500][case % 3];
            let mut launcher = launcher();
            launcher.position = [next() * 1e5, next() * 30_000., next() * 1e5];
            launcher.basis = Basis::new(next() * 6.3, next() * 3. - 1.5, next() * 6.3);
            launcher.speed_fps = next() * 2000.;
            launcher.velocity = launcher.basis.forward.map(|v| v * launcher.speed_fps);
            let mount = [next() * 6. - 3., next() * 2. - 1., next() * 10.];
            // Ranges from touching to tens of miles, many near the reach.
            let range = if case % 4 == 0 {
                f64::from(weapon.movement.initial_speed.max(0))
                    * f64::from(weapon.movement.remove_t)
                    * (0.8 + next() * 0.6)
            } else {
                10f64.powf(next() * 5.5)
            };
            let direction = Basis::new(next() * 6.3, next() * 3. - 1.5, 0.).forward;
            let speed = [0., 300., 1500., 4000.][case % 4] * next();
            let velocity = Basis::new(next() * 6.3, next() * 3. - 1.5, 0.)
                .forward
                .map(|v| v * speed);
            let target = TargetObservation {
                position: std::array::from_fn(|i| launcher.position[i] + direction[i] * range),
                velocity,
            };
            let new = solve_observed(&weapon, &launcher, mount, Some(target));
            let old = solve_observed_reference(&weapon, &launcher, mount, Some(target));
            assert_eq!(format!("{new:?}"), format!("{old:?}"), "case {case}");
            if let Ok(Some(_)) = new {
                found += 1;
            }
            if let Ok(None) = new
                && weapon.flags & 0x40 == 0
                && weapon.seeker.zones[1].maximum_range > 0
            {
                shortcut += 1;
            }
        }
        assert!(shortcut > 1000, "{shortcut}");
        assert!(found > 500, "{found}");
    }

    #[test]
    fn range_arc_hits_all_manual_anchors() {
        assert_eq!(range_arc_fraction(2000., 2000.), 0.);
        assert_eq!(range_arc_fraction(1000., 2000.), 0.5);
        assert_eq!(range_arc_fraction(100., 2000.), 1.);
        assert_eq!(range_arc_fraction(50., 2000.), 1.);
        assert!((range_arc_fraction(1500., 2000.) - 0.25).abs() < 1e-12);
        assert!((range_arc_fraction(550., 2000.) - 0.75).abs() < 1e-12);
    }

    #[test]
    fn every_selectable_aircraft_gun_uses_the_same_bounded_solver() {
        let launcher = launcher();
        let mut baseline = None;
        for id in AircraftId::ALL
            .into_iter()
            .chain([AircraftId::Faxx])
            .filter(|id| id.gun().is_some())
        {
            let mut gun = weapon();
            gun.source = id.gun().unwrap().into();
            let solution = solve(&gun, &launcher, [0.; 3], None).unwrap().unwrap();
            assert!(solution.seconds <= 10. && solution.range_ft <= 1000.);
            if let Some(expected) = baseline {
                assert_eq!(solution, expected, "{} solver diverged", id.label());
            } else {
                baseline = Some(solution);
            }
        }
    }
}
