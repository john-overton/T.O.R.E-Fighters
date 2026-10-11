//! Unguided bomb motion and the impact predictor the bomb sight uses.
//!
//! Spec-derived from the original's bomb routine as R1 read it
//! (docs/formats/weapons.md, "Bomb motion"; docs/spec/air-to-ground.md). A
//! record with the bomb type flag 0x10 never needs the gravity flag 0x4: the
//! bomb flag alone gives it a closed-form vacuum parabola.
//!
//! - Height after `s` seconds is `y0 + vz s - 16 s²` (g = 32 ft/s²), with no
//!   drag, no terminal speed and no retard after release.
//! - Horizontal motion is linear at the release's horizontal speed, with no cap.
//! - The drawn bomb pitches along its velocity; its heading stays the release
//!   heading.
//! - A bomb leaves along the aircraft's attitude (the nose, not the flight
//!   path) at `max(launcher speed x launchRetard / 100, initialSpeed)`,
//!   clamped by the record (0 to 1,466 ft/s for slick, retarded and packed
//!   bombs, 0 to 2,933 ft/s for cluster bombs), with no ejection push, and
//!   then turns by a uniform random error of up to [`BOMB_SCATTER_DEGREES`] in
//!   heading and in pitch, every bomb its own.
//! - A pack's bombs leave in one tick, each offset by [`pack_offset`].
//!
//! The predictor ([`predict_impact`]) runs the same equation without the
//! scatter against the ground under the release point, as the original's
//! sight does. Guided bombs (no 0x10) do not come here.
//!
//! Coordinates are the live adapter's: feet, x and z horizontal, y up, and
//! headings measured from +z toward +x.
use super::live::{Launcher, Station};
use crate::attitude::{Vector, unit};
use tore_formats::weapons::Weapon;

/// The projectile type flag that selects the bomb equation.
pub const BOMB_FLAG: u32 = 0x10;
/// The bomb equation's gravity, ft/s² (retail, not 32.2).
pub const GRAVITY_FPS2: f64 = 32.;
/// The largest random error a released bomb takes in heading and, separately,
/// in pitch. Retail's `adjustBombAngle` is 546 of 65,536 units a turn, 3
/// degrees (FA.EXE 1.02F `0x50ce08`, applied at `0x4c1fc0`); John halved it
/// to 1.5 degrees (273 units) on 2026-10-11, for every bomb, the player's
/// and the AI's (opinionated, John). One constant, so it can change.
pub const BOMB_SCATTER_DEGREES: f64 = 1.5;
/// Binary angle units a turn, the unit the scatter is rolled in.
const UNITS_PER_TURN: f64 = 65_536.;
/// The steepest release pitch, retail's `0x3ffc` clamp.
const PITCH_LIMIT: f64 = 0x3ffc as f64 * std::f64::consts::TAU / UNITS_PER_TURN;

/// A record that falls on the bomb equation.
pub fn is_bomb(w: &Weapon) -> bool {
    w.flags & BOMB_FLAG != 0
}

/// A pod station's store holds several rounds (a rocket pod); one trigger
/// call fires one of them. Gun records keep their own round accounting.
pub fn is_pod(w: &Weapon) -> bool {
    w.flags & 0x80 == 0 && w.burst.projectiles_in_pod > 1
}

/// How many projectiles one trigger call releases for a weapon that is not a
/// gun: a pod fires one round, any other record its whole burst (a 3-, 6- or
/// 7-pack in the same tick). Either way one round is debited.
pub fn release_count(w: &Weapon) -> usize {
    if is_pod(w) {
        1
    } else {
        usize::from(w.burst.game_rounds_in_burst.max(1)).min(32)
    }
}

/// Where bomb `index` of a pack leaves, relative to its station: feet along
/// the launcher's right axis and feet up. Retail staggers them cumulatively
/// by `(even i ? +i : -i) x 2` ft sideways and `-2 i` ft down: x 0, -2, +2,
/// -4, +4, -6, +6 and y 0, -2, -6, -12, -20, -30, -42. Retail adds the
/// sideways step on world x; which way that points is unknown, so it is laid
/// along the aircraft's lateral axis (fitted).
pub fn pack_offset(index: usize) -> [f64; 2] {
    let (mut x, mut y) = (0i64, 0i64);
    for i in 0..=index as i64 {
        x += if i % 2 == 0 { i } else { -i } * 2;
        y -= 2 * i;
    }
    [x as f64, y as f64]
}

/// Half the scatter's span in binary angle units (273 for 1.5 degrees, retail's 546 for 3).
fn scatter_units() -> u16 {
    (BOMB_SCATTER_DEGREES / 360. * UNITS_PER_TURN).round() as u16
}

/// One bomb's heading and pitch errors, radians, each uniform over the
/// scatter: `roll(n)` returns a value from 0 to n - 1 of the sim's own random
/// stream, so a replay and every machine of a network game agree. With the
/// scatter at 0 nothing is drawn.
pub fn scatter(mut roll: impl FnMut(u16) -> u16) -> (f64, f64) {
    let half = scatter_units();
    if half == 0 {
        return (0., 0.);
    }
    let mut angle = || {
        let units = i32::from(roll(2 * half + 1)) - i32::from(half);
        f64::from(units) * std::f64::consts::TAU / UNITS_PER_TURN
    };
    let heading = angle();
    let pitch = angle();
    (heading, pitch)
}

/// Where a bomb leaves its aircraft and how fast it is going.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReleaseState {
    pub position: Vector,
    /// Feet a second.
    pub velocity: Vector,
}

impl ReleaseState {
    /// Leaving `position` along `direction` (any length) at `speed` ft/s.
    pub fn new(position: Vector, direction: Vector, speed: f64) -> Self {
        let direction = unit(direction);
        Self {
            position,
            velocity: direction.map(|v| v * speed),
        }
    }

    /// What a station releases now, before the scatter: retail `PROJFire`.
    /// The store leaves its mount rotated with the aircraft (bomb `index` of
    /// a pack also its [`pack_offset`]), along the nose, at the record's
    /// launch speed ([`super::launch_speed`]). The fire loop and the bomb
    /// sight both start here, so the sight shows where a released bomb's
    /// mean path goes.
    pub fn from_station(launcher: &Launcher, station: &Station, index: usize) -> Self {
        let [side, up] = pack_offset(index);
        let basis = launcher.basis;
        let position = std::array::from_fn(|i| {
            launcher.position[i]
                + basis.right[i] * (station.mount[0] + side)
                + basis.up[i] * station.mount[1]
                + basis.forward[i] * station.mount[2]
                + [0., up, 0.][i]
        });
        let speed =
            super::launch_speed(&station.weapon.movement, (launcher.speed_fps * 256.) as i32)
                .map_or(0., f64::from);
        Self::new(position, basis.forward, speed)
    }

    pub fn speed(&self) -> f64 {
        self.velocity.iter().map(|v| v * v).sum::<f64>().sqrt()
    }

    /// Radians from +z toward +x.
    pub fn heading(&self) -> f64 {
        self.velocity[0].atan2(self.velocity[2])
    }

    /// Radians above the horizon.
    pub fn pitch(&self) -> f64 {
        self.velocity[1].atan2(self.velocity[0].hypot(self.velocity[2]))
    }

    /// The same release turned by `heading` and `pitch` radians, at the
    /// same speed; the pitch stays within retail's clamp.
    pub fn turned(self, heading: f64, pitch: f64) -> Self {
        let speed = self.speed();
        let heading = self.heading() + heading;
        let pitch = (self.pitch() + pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        let (sh, ch) = heading.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        Self {
            position: self.position,
            velocity: [speed * cp * sh, speed * sp, speed * cp * ch],
        }
    }

    /// The release that puts a bomb leaving along `direction` at `speed`
    /// on `target`, `height` feet above it, over flat ground at the target's
    /// height. For development drivers that place a bomb as a pass would
    /// have released it.
    pub fn onto(target: Vector, direction: Vector, speed: f64, height: f64) -> Self {
        let aimed = Self::new([0., height, 0.], direction, speed);
        let seconds = BombMotion::new(aimed).fall_time(0.).unwrap_or(0.);
        Self {
            position: [
                target[0] - aimed.velocity[0] * seconds,
                target[1] + height,
                target[2] - aimed.velocity[2] * seconds,
            ],
            velocity: aimed.velocity,
        }
    }
}

/// A released bomb's path: a vacuum parabola from its release.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BombMotion {
    pub origin: Vector,
    /// Horizontal velocity, ft/s along x and z.
    pub horizontal: [f64; 2],
    /// Vertical speed at release, ft/s, up positive.
    pub vertical: f64,
}

impl BombMotion {
    pub fn new(release: ReleaseState) -> Self {
        Self {
            origin: release.position,
            horizontal: [release.velocity[0], release.velocity[2]],
            vertical: release.velocity[1],
        }
    }

    /// Where the bomb is `seconds` after release.
    pub fn position(&self, seconds: f64) -> Vector {
        let fall = GRAVITY_FPS2 / 2. * seconds * seconds;
        [
            self.origin[0] + self.horizontal[0] * seconds,
            self.origin[1] + self.vertical * seconds - fall,
            self.origin[2] + self.horizontal[1] * seconds,
        ]
    }

    /// Its velocity then, ft/s.
    pub fn velocity(&self, seconds: f64) -> Vector {
        [
            self.horizontal[0],
            self.vertical - GRAVITY_FPS2 * seconds,
            self.horizontal[1],
        ]
    }

    /// Which way it points then: along its velocity, so it noses over while
    /// its heading stays the release heading.
    pub fn direction(&self, seconds: f64) -> Vector {
        let v = self.velocity(seconds);
        if v.iter().all(|c| c.abs() < 1e-12) {
            [0., -1., 0.]
        } else {
            unit(v)
        }
    }

    /// Seconds until it comes down to the height `ground`, retail's
    /// `(vz + sqrt(vz² + 64 h)) / 32` with h the release height above it.
    /// `None` when the path never gets there.
    pub fn fall_time(&self, ground: f64) -> Option<f64> {
        let height = self.origin[1] - ground;
        let root = self.vertical * self.vertical + 2. * GRAVITY_FPS2 * height;
        if root < 0. {
            return None;
        }
        let seconds = (self.vertical + root.sqrt()) / GRAVITY_FPS2;
        (seconds >= 0.).then_some(seconds)
    }
}

/// Where and when a bomb comes down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub position: Vector,
    /// Seconds after release.
    pub seconds: f64,
}

/// The bomb sight's prediction for `release`: the mean impact point (no
/// scatter), with the ground taken as the height under the release point, as
/// retail's predictor keeps it. Over flat ground a real bomb with no scatter
/// lands there. `None` when the path never reaches that height.
pub fn predict_impact(release: ReleaseState, ground: impl Fn(f64, f64) -> f64) -> Option<Impact> {
    let height = ground(release.position[0], release.position[2]);
    let motion = BombMotion::new(release);
    let seconds = motion.fall_time(height)?;
    let mut position = motion.position(seconds);
    position[1] = height;
    Some(Impact { position, seconds })
}

#[cfg(test)]
#[path = "ballistics_tests.rs"]
mod tests;
