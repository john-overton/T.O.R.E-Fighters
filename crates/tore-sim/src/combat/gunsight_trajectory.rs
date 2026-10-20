//! The round's path for the gun solve, worked out once and kept.
//!
//! What a round does after it leaves the muzzle (its speed, how far it has
//! flown and how far it has dropped at each tick) depends on its record, its
//! launch speed and, only while a motor drives it, the height it flies at;
//! never on the target. Every AI aircraft asks the gunsight every tick, most
//! with the same gun at the same launch speed, so the path is worked out
//! once per thread and read for each target: the first tick whose crossing
//! could turn non-negative is found by bisection over a running bound, and
//! the crossings are worked out from there, exactly as the tick-by-tick
//! loop in [`super::solve_observed`] works them out.
use super::{Apart, DT, SERVICE_RATE, Sample, TargetObservation, crossing};
use crate::attitude::Vector;
use crate::combat::{EnginePhase, FallState, axial_speed, commanded_speed, engine_phase};
use std::cell::RefCell;
use tore_formats::weapons::{Movement, Weapon};

/// Paths kept per thread.
const KEPT: usize = 16;

/// What a path depends on.
#[derive(Clone, Copy, PartialEq)]
struct Key {
    movement: Movement,
    /// The record's motor (0x40) and gravity (4) flags.
    flags: u32,
    speed_f8: i32,
    height: Height,
    ticks: u64,
}

/// How a path depends on the height it starts at.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Height {
    /// Not at all: no motor drives the round, or never in its flight. The
    /// fall takes the same steps at any height.
    Free,
    /// Through the 256-foot band only: a motor drives the round from its
    /// first tick at the height it starts at, and nothing else reads it.
    Band(i16),
    /// Through the exact height: the round falls before its motor lights.
    Exact(i32),
}

pub(super) struct Path {
    key: Key,
    distance: Vec<f64>,
    drop: Vec<f64>,
    speed_f8: Vec<i32>,
    fall_f8: Vec<u16>,
    /// The most of |distance| + |drop| over the ticks up to each one.
    most: Vec<f64>,
}

/// The two samples the path is reached between, with the round's speed and
/// fall speed after the reaching tick.
pub(super) struct Reached {
    pub samples: (Sample, Sample),
    pub speed_f8: i32,
    pub fall_velocity_f8: u16,
}

thread_local! {
    static PATHS: RefCell<(Vec<Path>, usize)> = const { RefCell::new((Vec::new(), 0)) };
}

/// Reads the path of `weapon`'s round launched at `speed_f8` from
/// `initial_height_f8` for `ticks`, kept or worked out now. `None` when the
/// path is not kept: a height too far out for its drop to be exact in
/// 32 bits, or a record whose motion fails (the tick-by-tick loop then
/// reports the failure at the tick it happens).
pub(super) fn solve<R>(
    weapon: &Weapon,
    speed_f8: i32,
    initial_height_f8: i32,
    ticks: u64,
    read: impl FnOnce(&Path) -> R,
) -> Option<R> {
    let key = Key {
        movement: weapon.movement,
        flags: weapon.flags & 0x44,
        speed_f8,
        height: height_key(&weapon.movement, weapon.flags, initial_height_f8, ticks)?,
        ticks,
    };
    PATHS.with(|paths| {
        let mut paths = paths.borrow_mut();
        let (kept, next) = &mut *paths;
        if let Some(path) = kept.iter().find(|path| path.key == key) {
            return Some(read(path));
        }
        let path = Path::build(key, initial_height_f8)?;
        let at = if kept.len() < KEPT {
            kept.push(path);
            kept.len() - 1
        } else {
            let at = *next;
            kept[at] = path;
            *next = (at + 1) % KEPT;
            at
        };
        Some(read(&kept[at]))
    })
}

fn height_key(m: &Movement, flags: u32, height_f8: i32, ticks: u64) -> Option<Height> {
    // The drop is the start height less the height now, exact while
    // neither wraps.
    if !(-(1 << 30)..(1 << 30)).contains(&height_f8) {
        return None;
    }
    if flags & 0x40 == 0 {
        return Some(Height::Free);
    }
    let last = (ticks.max(1) - 1) / 30;
    let phases = (0..=last).map(|now| engine_phase(m, now as u16, 0));
    if !phases.clone().any(|phase| phase == EnginePhase::Powered) {
        return Some(Height::Free);
    }
    Some(
        if phases
            .clone()
            .any(|phase| phase == EnginePhase::BeforeIgnition)
        {
            Height::Exact(height_f8)
        } else {
            Height::Band((height_f8 >> 16) as i16)
        },
    )
}

impl Path {
    /// The tick-by-tick motion of [`super::solve_observed`], recorded.
    fn build(key: Key, initial_height_f8: i32) -> Option<Self> {
        let m = &key.movement;
        let ticks = key.ticks as usize;
        let mut path = Self {
            key,
            distance: Vec::with_capacity(ticks),
            drop: Vec::with_capacity(ticks),
            speed_f8: Vec::with_capacity(ticks),
            fall_f8: Vec::with_capacity(ticks),
            most: Vec::with_capacity(ticks),
        };
        let mut speed_f8 = key.speed_f8;
        let mut fall = FallState::default();
        let mut height_f8 = initial_height_f8;
        let mut service_remainder = 0u16;
        let mut distance = 0.;
        let mut most = 0f64;
        for tick in 0..key.ticks {
            service_remainder += SERVICE_RATE;
            let service = (service_remainder / 120) as i16;
            service_remainder %= 120;
            let now = (tick / 30) as u16;
            let phase = engine_phase(m, now, 0);
            if key.flags & 0x40 != 0 {
                let target_speed = commanded_speed(m, phase, speed_f8, height_f8) as i16;
                speed_f8 = axial_speed(m, speed_f8, target_speed, false, service).ok()?;
            }
            distance += f64::from(speed_f8) * f64::from(service) / 65536.;
            height_f8 = fall
                .advance(key.flags & 4 != 0, phase, service, height_f8)
                .ok()?;
            let drop = f64::from(initial_height_f8 - height_f8) / 256.;
            most = most.max(distance.abs() + drop.abs());
            path.distance.push(distance);
            path.drop.push(drop);
            path.speed_f8.push(speed_f8);
            path.fall_f8.push(fall.velocity_f8);
            path.most.push(most);
        }
        Some(path)
    }

    fn sample(&self, tick: usize) -> (f64, f64, f64) {
        ((tick + 1) as f64 * DT, self.distance[tick], self.drop[tick])
    }

    /// Where the round's path meets `target` seen from `muzzle`: the first
    /// tick whose crossing is non-negative after a negative one, as the
    /// tick-by-tick loop finds it.
    pub(super) fn reach(&self, target: TargetObservation, muzzle: Vector) -> Option<Reached> {
        let apart = Apart::new(target, muzzle);
        let ticks = self.distance.len();
        // Every tick before `start` surely has a negative crossing: the
        // bound grows with the tick, so the first tick it no longer holds
        // for is found by bisection.
        let surely_before =
            |end: usize| end == 0 || apart.surely_near((end as f64) * DT, self.most[end - 1]);
        let (mut low, mut high) = (0, ticks);
        while low < high {
            let middle = low + (high - low).div_ceil(2);
            if surely_before(middle) {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        let start = low;
        let target = Some(target);
        let mut previous = if start == 0 {
            Sample {
                seconds: 0.,
                distance: 0.,
                drop: 0.,
                crossing: crossing(target, muzzle, 0., 0., 0.),
                pending: false,
            }
        } else {
            let (seconds, distance, drop) = self.sample(start - 1);
            Sample {
                seconds,
                distance,
                drop,
                crossing: 0.,
                pending: true,
            }
        };
        for tick in start..ticks {
            let (seconds, distance, drop) = self.sample(tick);
            if apart.surely(seconds, distance, drop) {
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
                crossing: crossing(target, muzzle, seconds, distance, drop),
                pending: false,
            };
            if previous.pending {
                previous.crossing = crossing(
                    target,
                    muzzle,
                    previous.seconds,
                    previous.distance,
                    previous.drop,
                );
            }
            if current.crossing >= 0. && previous.crossing < 0. {
                return Some(Reached {
                    samples: (previous, current),
                    speed_f8: self.speed_f8[tick],
                    fall_velocity_f8: self.fall_f8[tick],
                });
            }
            previous = current;
        }
        None
    }
}
