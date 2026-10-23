//! Bomb release, scatter and prediction using the shared fitted flight model.
use super::bomb_flight::{Body, DT, Flight, MAX_SECONDS};
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
/// stream (a release rolls from [`scatter_stream`]), so a replay and every
/// machine of a network game agree. With the scatter at 0 nothing is drawn.
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

/// The random stream one released bomb's scatter is rolled from: combat's
/// random state mixed with the release tick and the round's number
/// (SplitMix64), a value from 0 to n - 1 a roll.
///
/// Combat's own stream moves only when something rolls (a hit, a jammer, a
/// fault), so a mission's first bomb, released before anything else had
/// rolled, used to take the very same error every time: 1.25 degrees left and
/// 1.31 degrees low, about 330 ft short of a 5,000 ft level release, at every
/// skill and on every target (slice ST-LV). Mixing in the tick and the round
/// gives every bomb its own error, still a pure function of the simulation's
/// state, so a replay, a checkpoint and every machine agree. Combat's stream
/// itself is left as it is.
pub fn scatter_stream(state: u32, tick: u64, round: u32) -> impl FnMut(u16) -> u16 {
    const GOLDEN: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut seed =
        (u64::from(state) << 32 | u64::from(round)) ^ tick.wrapping_mul(GOLDEN).rotate_left(17);
    move |bound| {
        seed = seed.wrapping_add(GOLDEN);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        (((z >> 32) * u64::from(bound)) >> 32) as u16
    }
}

/// Where a bomb leaves its aircraft and how fast it is going.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReleaseState {
    pub body: Body,
    pub position: Vector,
    /// Feet a second.
    pub velocity: Vector,
}

impl ReleaseState {
    /// Leaving `position` along `direction` at `speed` ft/s, with a nominal
    /// 500 lb slick body. Store-aware callers set `body` or use `from_station`.
    pub fn new(position: Vector, direction: Vector, speed: f64) -> Self {
        let direction = unit(direction);
        Self {
            body: Body::default(),
            position,
            velocity: direction.map(|v| v * speed),
        }
    }

    /// What a station releases now, before scatter.
    /// The store leaves its mount rotated with the aircraft (bomb `index` of
    /// a pack also its [`pack_offset`]), with the aircraft's velocity. The fire loop and the bomb
    /// sight both start here, so the sight shows where a released bomb's
    /// mean path goes.
    pub fn from_station(launcher: &Launcher, station: &Station, index: usize) -> Self {
        Self::from_mount(launcher, station, station.mount, index)
    }

    /// [`Self::from_station`] from `mount` (feet right, up and forward), the
    /// side of a paired station the store leaves from
    /// ([`super::hardpoints::Rack::release_mount`]).
    pub fn from_mount(launcher: &Launcher, station: &Station, mount: Vector, index: usize) -> Self {
        let [side, up] = pack_offset(index);
        let basis = launcher.basis;
        let position = std::array::from_fn(|i| {
            launcher.position[i]
                + basis.right[i] * (mount[0] + side)
                + basis.up[i] * mount[1]
                + basis.forward[i] * mount[2]
                + [0., up, 0.][i]
        });
        Self {
            position,
            velocity: launcher.velocity,
            body: Body::of(&station.weapon),
        }
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
            body: self.body,
            position: self.position,
            velocity: [speed * cp * sh, speed * sp, speed * cp * ch],
        }
    }

    /// The release that puts a bomb leaving along `direction` at `speed`
    /// on `target`, `height` feet above it, over flat ground at the target's
    /// height. For development drivers that place a bomb as a pass would
    /// have released it.
    pub fn onto(target: Vector, direction: Vector, speed: f64, height: f64) -> Self {
        Self::onto_body(target, direction, speed, height, Body::default())
    }

    pub fn onto_body(
        target: Vector,
        direction: Vector,
        speed: f64,
        height: f64,
        body: Body,
    ) -> Self {
        let mut aimed = Self::new([0., target[1] + height, 0.], direction, speed);
        aimed.body = body;
        let impact = impact_at(aimed, target[1]);
        let point = impact.map_or([0.; 3], |i| i.position);
        Self {
            body,
            position: [
                target[0] - point[0],
                target[1] + height,
                target[2] - point[2],
            ],
            velocity: aimed.velocity,
        }
    }
}

/// A released bomb and its last integrated tick. The cache avoids replaying
/// the fall on every live tick; prediction uses the identical integrator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BombMotion {
    pub origin: Vector,
    pub horizontal: [f64; 2],
    pub vertical: f64,
    pub body: Body,
    pub tick: u64,
    pub flight: Flight,
}
impl BombMotion {
    pub fn new(release: ReleaseState) -> Self {
        Self {
            origin: release.position,
            horizontal: [release.velocity[0], release.velocity[2]],
            vertical: release.velocity[1],
            body: release.body,
            tick: 0,
            flight: Flight {
                position: release.position,
                velocity: release.velocity,
            },
        }
    }
    pub fn advance(&mut self, tick: u64) {
        if tick < self.tick {
            self.tick = 0;
            self.flight = Flight {
                position: self.origin,
                velocity: [self.horizontal[0], self.vertical, self.horizontal[1]],
            };
        }
        while self.tick < tick {
            self.flight.step(self.body, self.tick, None);
            self.tick += 1;
        }
    }
    fn at(&self, seconds: f64) -> Flight {
        let mut motion = *self;
        let ticks = seconds.max(0.) / DT;
        motion.advance(ticks.floor() as u64);
        motion.between(ticks.fract())
    }
    /// The flight `f` of the way from this tick to the next: the one sample
    /// [`Self::at`] takes once it has reached the whole tick.
    fn between(mut self, f: f64) -> Flight {
        let before = self.flight;
        self.advance(self.tick + 1);
        let motion = self;
        Flight {
            position: std::array::from_fn(|i| {
                before.position[i] + f * (motion.flight.position[i] - before.position[i])
            }),
            velocity: std::array::from_fn(|i| {
                before.velocity[i] + f * (motion.flight.velocity[i] - before.velocity[i])
            }),
        }
    }
    pub fn position(&self, seconds: f64) -> Vector {
        self.at(seconds).position
    }
    pub fn velocity(&self, seconds: f64) -> Vector {
        self.at(seconds).velocity
    }
    pub fn direction(&self, seconds: f64) -> Vector {
        unit(self.velocity(seconds))
    }
    pub fn fall_time(&self, ground: f64) -> Option<f64> {
        self.impact(&|_, _| ground).map(|i| i.seconds)
    }
    pub fn impact(&self, ground: &impl Fn(f64, f64) -> f64) -> Option<Impact> {
        let m = *self;
        let above = m.flight.position[1] - ground(m.flight.position[0], m.flight.position[2]);
        if above <= 0. {
            return None;
        }
        Self::crossing(m, ground)
    }
    /// The first terrain crossing after `m`'s tick, stepping on from it.
    fn crossing(mut m: Self, ground: &impl Fn(f64, f64) -> f64) -> Option<Impact> {
        while (m.tick as f64) < MAX_SECONDS / DT {
            let before = m.flight.position;
            m.advance(m.tick + 1);
            let p = m.flight.position;
            let next = p[1] - ground(p[0], p[2]);
            if next <= 0. {
                let mut low = 0.;
                let mut high = 1.;
                for _ in 0..32 {
                    let mid = (low + high) * 0.5;
                    let p: Vector = std::array::from_fn(|i| before[i] + mid * (p[i] - before[i]));
                    if p[1] > ground(p[0], p[2]) {
                        low = mid;
                    } else {
                        high = mid;
                    }
                }
                let fraction = high;
                let mut position =
                    std::array::from_fn(|i| before[i] + fraction * (p[i] - before[i]));
                position[1] = ground(position[0], position[2]);
                return Some(Impact {
                    position,
                    seconds: (m.tick as f64 - 1. + fraction) * DT,
                });
            }
        }
        None
    }
}

/// A cursor along a stored [`BombMotion`], advanced one whole tick at a time,
/// so a store falling for `n` ticks costs `n` integration steps instead of
/// the `n` squared of sampling the stored motion from its release each tick
/// (optimization slice S1). It runs the identical step sequence, so every
/// answer is bit for bit the stored motion's own. Transient: never
/// checkpointed, sent or recorded; it is rebuilt from the stored motion
/// whenever it is missing or no longer follows it.
#[derive(Clone, Copy, Debug)]
pub struct FallCursor {
    motion: BombMotion,
    /// The lowest height reached at any tick after the release up to the
    /// cursor's tick, so [`Self::fall_time`] knows no crossing was skipped.
    lowest: f64,
}
impl FallCursor {
    pub fn new(stored: BombMotion) -> Self {
        Self {
            motion: stored,
            lowest: f64::INFINITY,
        }
    }

    /// Whether this cursor runs along `stored`: the same release, body and
    /// starting point, compared bit for bit.
    pub fn follows(&self, stored: &BombMotion) -> bool {
        let release = |m: &BombMotion| {
            // Every field, so a new one cannot be missed.
            let Body {
                mass,
                area,
                retarder,
                steering,
                guidance_seconds,
            } = m.body;
            [
                m.origin[0],
                m.origin[1],
                m.origin[2],
                m.horizontal[0],
                m.horizontal[1],
                m.vertical,
                mass,
                area,
                retarder,
                f64::from(steering),
                guidance_seconds,
            ]
            .map(f64::to_bits)
        };
        release(&self.motion) == release(stored)
    }

    /// [`BombMotion::position`], [`BombMotion::velocity`] and
    /// [`BombMotion::direction`] of `stored` at `seconds`, in one sample:
    /// the identical bits, stepping on from the cursor's last tick.
    pub fn at(&mut self, stored: &BombMotion, seconds: f64) -> Flight {
        let ticks = seconds.max(0.) / DT;
        let floor = ticks.floor() as u64;
        // `BombMotion::at` starts from the stored tick, or from the release
        // when asked for an earlier one: start again wherever the cursor
        // could not be on that same path.
        if floor < self.motion.tick || (self.motion.tick < stored.tick && stored.tick <= floor) {
            *self = Self::new(*stored);
        }
        if floor < self.motion.tick {
            // Earlier than the stored tick: from the release, as `advance`.
            self.motion.advance(floor);
            self.lowest = f64::NEG_INFINITY;
        }
        while self.motion.tick < floor {
            self.motion.advance(self.motion.tick + 1);
            self.lowest = self.lowest.min(self.motion.flight.position[1]);
        }
        self.motion.between(ticks.fract())
    }

    /// [`BombMotion::fall_time`] of `stored` to `ground`, bit for bit,
    /// searching on from the cursor's tick when no tick before it reached
    /// the ground. The "already below" check stays on the stored release.
    pub fn fall_time(&self, stored: &BombMotion, ground: f64) -> Option<f64> {
        let known = stored.tick == 0 && self.lowest > f64::NEG_INFINITY;
        if !known || self.lowest - ground <= 0. {
            return stored.fall_time(ground);
        }
        if stored.flight.position[1] - ground <= 0. {
            return None;
        }
        BombMotion::crossing(self.motion, &|_, _| ground).map(|i| i.seconds)
    }
}

/// [`FallCursor`]s by projectile number: the falls of jettisoned stores and
/// of guided bombs falling blind. Transient, so its `Debug` shows nothing and
/// the checkpoint, the wire and replays never see it.
#[derive(Clone, Default)]
pub struct FallCursors(std::collections::BTreeMap<u32, FallCursor>);
impl std::fmt::Debug for FallCursors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FallCursors")
    }
}
impl FallCursors {
    /// The cursor of projectile `id` along `stored`, made afresh when there
    /// is none or it follows another fall.
    pub fn follow(&mut self, id: u32, stored: &BombMotion) -> &mut FallCursor {
        let cursor = self.0.entry(id).or_insert_with(|| FallCursor::new(*stored));
        if !cursor.follows(stored) {
            *cursor = FallCursor::new(*stored);
        }
        cursor
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn retain(&mut self, mut keep: impl FnMut(u32) -> bool) {
        self.0.retain(|id, _| keep(*id));
    }
}

/// Where and when a bomb comes down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub position: Vector,
    /// Seconds after release.
    pub seconds: f64,
}

/// Mean impact without scatter, at the first terrain crossing of the shared
/// flight model. The release includes the selected store's body parameters.
pub fn predict_impact(release: ReleaseState, ground: impl Fn(f64, f64) -> f64) -> Option<Impact> {
    terrain_impact(release, &ground)
}

/// Flat-ground reference using the shared flight model and the height under
/// the release for the whole fall. This preserves the original sight's
/// terrain assumption, not its former vacuum equation, for diagnostics.
pub fn predict_impact_flat(
    release: ReleaseState,
    ground: impl Fn(f64, f64) -> f64,
) -> Option<Impact> {
    impact_at(release, ground(release.position[0], release.position[2]))
}

/// First terrain crossing on the shared integration grid.
pub fn terrain_impact(release: ReleaseState, ground: &impl Fn(f64, f64) -> f64) -> Option<Impact> {
    BombMotion::new(release).impact(ground)
}

/// Where the mean path of `release` comes down to the height `height`: the
/// level T and the AI's release judge a bomb against its target's ground
/// this way, so a target higher or lower than the ground under the release
/// point is still hit (slice ST-LV). `None` when the path never gets there.
pub fn impact_at(release: ReleaseState, height: f64) -> Option<Impact> {
    BombMotion::new(release).impact(&|_, _| height)
}

#[cfg(test)]
#[path = "ballistics_tests.rs"]
mod tests;
