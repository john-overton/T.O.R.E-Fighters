//! The guided air-to-ground weapons: laser bombs and missiles, the JDAMs and
//! the AT-2 (docs/spec/air-to-ground.md, "Weapon classes" 4, 5, 7 and 12, and
//! "Guided bomb release range indicator"). Their flight in combat is
//! `live::guided`; this module holds what both the flight and the cockpit
//! need: which records they are, how they leave the aircraft, the zone-1
//! cones, the fly-out that says whether one can still reach a point and how
//! long it takes, and the reach the HUD's release range scale shows.
//!
//! Motion (retail data, docs/formats/missiles.md, "Laser and designator" and
//! "Optical"): the laser bombs and JDAMs have no motor, no bomb flag 0x10 and
//! no gravity flag 0x4; they leave along the nose at the launch speed (retard
//! 90), slow at 14 ft/s² toward 278 ft/s (the 0x40 speed command) and turn at
//! 6,370 units a second, about 35 degrees a second, with no fall while they
//! guide. The missiles keep their record's motor, sag and cruise profile, as
//! the profiled missiles do. A guided bomb that is not guiding falls on the
//! bomb equation (fitted: retail holds its attitude, and whether anything
//! makes it fall is open, R1's addendum).
use super::live::Launcher;
use super::missiles::{self, DT, Guidance, Motion};
use crate::attitude::{Basis, Vector, dot, unit};
use crate::combat::EnginePhase;
use tore_formats::weapons::{Movement, Weapon, Zone};

/// A hit, as the missiles' fly-out counts one: within 25 ft.
pub const HIT_FT: f64 = 25.;
/// Samples of the reach search across the zone's ground range, before each
/// edge is bisected.
const REACH_SAMPLES: usize = 32;
/// Bisection steps for each edge of the reach (about 1 part in 4,000 of
/// the zone's range).
const REACH_BISECTIONS: usize = 12;
/// The steering error, radians, below which a fly-out treats the rest of
/// the path as straight and works it out from the record's speeds.
const STRAIGHT_RAD: f64 = 0.002;

/// The guidance of an air-to-ground weapon record: laser, coordinate (the
/// JDAMs) or command (the AT-2). `None` for any other record.
pub fn kind(w: &Weapon) -> Option<Guidance> {
    Guidance::air_to_ground(w)
}

/// A guided bomb: a guided air-to-ground record with no motor (the laser
/// bombs and the JDAMs). It glides on its release speed.
pub fn is_glide_bomb(w: &Weapon) -> bool {
    kind(w).is_some() && w.movement.fuel_t <= w.movement.ignite_t
}

/// The velocity a guided weapon leaves its station with: a guided bomb
/// along the nose at the record's launch speed (`launchRetard` 90 of the
/// aircraft's speed), a missile with the aircraft's velocity, as a profiled
/// missile leaves.
pub fn release_velocity(w: &Weapon, launcher: &Launcher) -> Vector {
    if is_glide_bomb(w) {
        let speed = super::launch_speed(&w.movement, (launcher.speed_fps * 256.) as i32)
            .map_or(0., f64::from);
        launcher.basis.forward.map(|v| v * speed)
    } else {
        launcher.velocity
    }
}

/// The frame of a weapon flying along `direction`, wings level: what its
/// seeker's cone is measured about in flight.
pub fn nose(direction: Vector) -> Basis {
    let yaw = direction[0].atan2(direction[2]);
    let pitch = direction[1].atan2(direction[0].hypot(direction[2]));
    Basis::new(yaw, pitch, 0.)
}

/// Whether `point` lies inside the weapon's zone-1 cone about `basis` at
/// `position`: heading and pitch each within the zone's half angles (45
/// degrees on every guided air-to-ground record), angles only, as the
/// retail lock reruns in flight.
pub fn in_cone(w: &Weapon, position: Vector, basis: Basis, point: Vector) -> bool {
    let zone = Zone {
        minimum_range: 0,
        maximum_range: i32::MAX,
        minimum_altitude: i32::MIN,
        maximum_altitude: i32::MAX,
        ..w.seeker.zones[1]
    };
    missiles::geometry(&zone, position, basis, point, None)
}

/// Whether `point` lies inside the weapon's zone 1 at release from a mount
/// at `position` pointing along `basis`: the cone, the range (its maximum
/// times `scale`, the weather's share for a laser weapon) and the
/// altitudes, as the retail lock tests at launch.
pub fn in_release_zone(
    w: &Weapon,
    position: Vector,
    basis: Basis,
    point: Vector,
    scale: f64,
) -> bool {
    let zone = w.seeker.zones[1];
    let zone = Zone {
        maximum_range: (f64::from(zone.maximum_range.max(0)) * scale.clamp(0., 1.)) as i32,
        ..zone
    };
    missiles::geometry(&zone, position, basis, point, None)
}

/// The slant range the weapon's zone 1 allows at release, feet, scaled.
pub fn zone_range(w: &Weapon, scale: f64) -> f64 {
    f64::from(w.seeker.zones[1].maximum_range.max(0)) * scale.clamp(0., 1.)
}

/// The last tick of the weapon's life.
fn life_ticks(m: &Movement) -> u64 {
    u64::from(m.remove_t) * 30
}

/// Seconds a weapon flying straight at `speed` ft/s from age `age` (ticks)
/// needs to cover `distance` feet, with the record's motor, slowdown and
/// final speed and the motor's thrust still unspent (`budget - gain`).
/// `None` when its life ends first. The coast is worked out in closed form;
/// a burning motor is stepped tick by tick.
fn straight_seconds(
    m: &Movement,
    mut speed: f64,
    mut gain: f64,
    budget: f64,
    age: u64,
    distance: f64,
) -> Option<f64> {
    let end = life_ticks(m);
    let mut tick = age;
    let mut covered = 0.;
    while tick < end && missiles::phase(m, tick) != EnginePhase::Coast {
        if missiles::phase(m, tick) == EnginePhase::Powered {
            let step = (f64::from(m.acceleration.max(0)) * DT).min((budget - gain).max(0.));
            gain += step;
            speed += step;
        }
        covered += speed * DT;
        tick += 1;
        if covered >= distance {
            return Some((tick - age) as f64 * DT);
        }
    }
    if tick >= end {
        return None;
    }
    let left = distance - covered;
    let deceleration = f64::from(m.deceleration.max(0));
    let floor = f64::from(m.final_speed.max(0)).min(speed);
    let coast = if speed > floor && deceleration > 0. {
        let slowing = (speed - floor) / deceleration;
        let slowed = (speed + floor) / 2. * slowing;
        if left <= slowed {
            (speed - (speed * speed - 2. * deceleration * left).max(0.).sqrt()) / deceleration
        } else if floor > 0. {
            slowing + (left - slowed) / floor
        } else {
            return None;
        }
    } else if speed > 0. {
        left / speed
    } else {
        return None;
    };
    let seconds = (tick - age) as f64 * DT + coast;
    (age as f64 * DT + seconds <= end as f64 * DT).then_some(seconds)
}

/// Seconds a guided weapon flying `motion`, at `position` and pointing
/// along `forward`, `age` ticks after its release, needs to reach `point`
/// with its own turn and speeds: the turn is flown tick by tick, as the
/// weapon flies it, until it points at the point; the straight rest comes
/// from [`straight_seconds`]. `None` when its life ends first. Agent design
/// for the reach and the time to impact (the missiles' maximum range flies
/// the same physics).
pub fn time_to(
    w: &Weapon,
    mut motion: Motion,
    mut position: Vector,
    mut forward: Vector,
    point: Vector,
    age: u64,
) -> Option<f64> {
    let m = &w.movement;
    let end = life_ticks(m);
    for tick in age..end {
        let gap = missiles::sub(point, position);
        let distance = missiles::length(gap);
        if distance <= HIT_FT {
            return Some((tick - age) as f64 * DT);
        }
        let aim = motion.aim(position, point);
        let desired = missiles::commanded_heading(
            forward,
            motion.velocity,
            unit(missiles::sub(aim, position)),
        );
        let error = dot(forward, desired).clamp(-1., 1.).acos();
        let pointing = dot(forward, unit(gap)).clamp(-1., 1.).acos();
        if error < STRAIGHT_RAD && pointing < STRAIGHT_RAD {
            return straight_seconds(
                m,
                missiles::length(motion.velocity),
                motion.gain,
                motion.budget,
                tick,
                distance - HIT_FT,
            )
            .map(|rest| (tick - age) as f64 * DT + rest);
        }
        let next = missiles::steer(m, tick, forward, desired);
        motion.turn(forward, next);
        forward = next;
        let delta = motion.step(m, tick, forward);
        for (p, d) in position.iter_mut().zip(delta) {
            *p += d;
        }
    }
    None
}

/// A guided weapon's reach from where it would leave now: the nearest and
/// farthest ground range ahead at which it can still reach a point on the
/// ground (docs/spec/air-to-ground.md, "Guided bomb release range
/// indicator"). Slice H1's type; [`reach`] is the search for guided weapons
/// (H1's `Reach::search` starts from the unguided throw, which a guided
/// glide with no fall does not make good).
pub use super::delivery::Reach;

/// The ground track a release from `launcher` runs along: its velocity's,
/// else its nose's, as a horizontal unit vector.
pub fn ground_track(launcher: &Launcher) -> Option<[f64; 2]> {
    [launcher.velocity, launcher.basis.forward]
        .into_iter()
        .map(|v| (v[0], v[2]))
        .find(|(x, z)| x.hypot(*z) > 1e-6)
        .map(|(x, z)| {
            let length = x.hypot(z);
            [x / length, z / length]
        })
}

/// The reach of guided weapon `w` released now from `launcher` at a point
/// on ground `ground` feet high: the points straight ahead along the ground
/// track that lie inside zone 1 at release (range scaled by `scale`) and,
/// for a guided bomb, that its glide reaches within its life (a fly-out of
/// [`time_to`]). For a missile the fly-out is not run (its motor reaches
/// the zone, fitted). The first stretch of good points is taken, each edge
/// bisected. `None` when no point ahead qualifies.
///
/// Agent design: retail gives no release range cue (retail manual p. 125).
/// Deterministic, in a bounded number of fly-outs.
pub fn reach(w: &Weapon, launcher: &Launcher, ground: f64, scale: f64) -> Option<Reach> {
    let track = ground_track(launcher)?;
    let height = launcher.position[1] - ground;
    let slant = zone_range(w, scale);
    if slant <= height.abs() {
        return None;
    }
    let cap = (slant * slant - height * height).sqrt();
    let velocity = release_velocity(w, launcher);
    let glide = is_glide_bomb(w);
    let motion = Motion::launch(w, velocity, launcher.position[1]);
    let forward = unit(velocity);
    let at = |range: f64| -> Vector {
        [
            launcher.position[0] + track[0] * range,
            ground,
            launcher.position[2] + track[1] * range,
        ]
    };
    let good = |range: f64| {
        let point = at(range);
        in_release_zone(w, launcher.position, launcher.basis, point, scale)
            && (!glide || time_to(w, motion, launcher.position, forward, point, 0).is_some())
    };
    let step = cap / REACH_SAMPLES as f64;
    let samples: Vec<bool> = (0..=REACH_SAMPLES).map(|i| good(i as f64 * step)).collect();
    let first = samples.iter().position(|ok| *ok)?;
    let last = first + samples[first..].iter().take_while(|ok| **ok).count() - 1;
    let edge = |mut inside: f64, mut outside: f64| {
        for _ in 0..REACH_BISECTIONS {
            let middle = (inside + outside) / 2.;
            if good(middle) {
                inside = middle;
            } else {
                outside = middle;
            }
        }
        inside
    };
    let minimum = if first == 0 {
        0.
    } else {
        edge(first as f64 * step, (first - 1) as f64 * step)
    };
    let maximum = if last == REACH_SAMPLES {
        cap
    } else {
        edge(last as f64 * step, (last + 1) as f64 * step)
    };
    Some(Reach {
        minimum_ft: minimum,
        maximum_ft: maximum,
    })
}

/// Horizontal distance between two points, feet.
pub fn ground_range(a: Vector, b: Vector) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

/// What the cockpit shows for a selected guided weapon: the advisory the
/// readiness slot carries (none of them stops a release).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Advisory {
    /// A laser weapon with no spot to home on: it can still be released and
    /// may find one once, inside its nose cone (NO LASER).
    NoLaser,
    /// The plane's laser is wanted but masked: the spot is gone (MASK).
    Masked,
    /// The weapon cannot reach the point from here (OUT OF REACH).
    OutOfReach,
    /// A JDAM or AT-2 with nothing designated (NO TARGET); this one does
    /// stop the release, through the readiness.
    NoTarget,
}
impl Advisory {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoLaser => "NO LASER",
            Self::Masked => "MASK",
            Self::OutOfReach => "OUT OF REACH",
            Self::NoTarget => "NO TARGET",
        }
    }
}

/// The selected guided weapon's sight: where it would go and whether it can
/// get there (docs/spec/air-to-ground.md, "Guided bomb release range
/// indicator" and "Warnings"). Worked out every tick for the cockpit; never
/// read back by combat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuidedSight {
    pub kind: Guidance,
    /// The point the weapon would home on: the laser spot (own or a
    /// buddy's), the JDAM's point or the AT-2's target.
    pub aim: Option<Vector>,
    /// The aim is a buddy's laser spot, not the plane's own.
    pub buddy: bool,
    /// From the plane to the aim, feet: along the ground and slant.
    pub ground_range_ft: Option<f64>,
    pub slant_range_ft: Option<f64>,
    /// The reach ahead along the ground track (a guided bomb's fly-out, a
    /// missile's zone 1).
    pub reach: Option<Reach>,
    /// The aim lies inside the release zone and the reach: IN RNG.
    pub in_reach: bool,
    pub advisory: Option<Advisory>,
}

#[cfg(test)]
#[path = "guided_tests.rs"]
mod tests;
