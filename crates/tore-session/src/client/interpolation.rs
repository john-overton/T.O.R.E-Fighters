//! Drawing everything else in the past: every other aircraft, missile,
//! debris piece and ejected pilot, between the snapshots around the render
//! time (docs/ARCHITECTURE.md, "The client session"; the numbers are
//! docs/MULTIPLAYER.md, "Netcode numbers").
//!
//! Each entity keeps its received states by tick. It is drawn at the render
//! time less its own extra delay: none for an entity the host sends every
//! snapshot, its own update interval for one sent 4 times a second (the far
//! band, [`far_interval_ticks`]), sliding
//! between the two at a tenth of real time. Positions follow a cubic curve
//! through two states' positions and velocities, attitudes turn the short
//! way, devices blend; the rest is the earlier state's. Past its newest state
//! an entity continues along its last velocity for up to 250 ms, then holds.
//!
//! A rotorcraft's rotor speed and disk tilts blend too, and its blade angle
//! is the rotor speed integrated over the drawn time (slice P7b): exactly,
//! through the received states taken as straight lines between ticks, from
//! the entity's first state, where it starts as if the rotor had turned at
//! 100 percent since tick 0. So it is continuous, the same whatever the
//! frame rate, and the same for a given stream of states; it is the client's
//! own, not the host's blade angle, which no player can compare.

use super::clock::{ExtraDelay, JUMP_TICKS, TICKS_PER_SECOND, ticks_of};
use crate::wire::entity::{
    AircraftState, DebrisState, EntityKey, EntityKind, EntityState, PilotState, ProjectileState,
    RATE_STEP, ROTOR_SPEED_STEP, ROTOR_TILT_STEP, SPEED_STEP, radians,
};
use crate::wire::names::NameIndex;
use crate::wire::priority::{far_interval_ticks, far_snapshots};
use crate::wire::snapshot::ReceivedSnapshot;
use std::collections::{BTreeMap, VecDeque};
use std::f64::consts::{PI, TAU};
use std::time::Duration;
use tore_sim::attitude::Basis;
use tore_world::snapshot::{
    AircraftPose, Damage, DebrisPose, Draw, Engine, PilotPose, ProjectilePose,
};

/// An entity past its newest state goes on along its velocity for at most
/// this many ticks (250 ms), then holds.
pub const EXTRAPOLATE_TICKS: f64 = JUMP_TICKS;
/// The longest extra delay (one second), however rarely an entity comes.
pub const EXTRA_MAX: f64 = 120.;
/// Snapshots between an entity's states at which it is far: 4, or the far
/// band's own interval when that is shorter (3 at 10 and 12 a second), and
/// at least half the far band's interval (8 at 60 a second), so a near
/// entity that loses a few packets in a row is not taken for a far one.
/// Two or fewer is near; between the two the band stays as it was. *Agent
/// decision* (D12).
pub fn far_gap_snapshots(ticks_per_snapshot: u32) -> u32 {
    let far = far_snapshots(ticks_per_snapshot);
    far.div_ceil(2).max(4).min(far)
}
/// States kept per entity.
const KEPT: usize = 64;

/// One entity's received states.
#[derive(Clone, Debug, Default)]
struct Track {
    states: VecDeque<(u32, EntityState)>,
    removed: Option<u32>,
    extra: ExtraDelay,
    /// The entity is sent twice a second, and its last gaps between states.
    far: bool,
    gaps: VecDeque<f64>,
    /// The entity has been drawn: its extra delay now only slides.
    drawn: bool,
    /// A rotorcraft's blade angle at a tick, seconds at 100 percent rotor
    /// speed: the anchor its drawn angle integrates from. It moves on with
    /// the oldest kept state.
    turns: Option<(u32, f64)>,
}

/// An entity state's rotor speed, a share of 100 percent; zero without
/// rotors.
fn rotor_speed(state: &EntityState) -> f64 {
    match state {
        EntityState::Aircraft(a) => a
            .rotor
            .map_or(0., |r| f64::from(r.speed) * ROTOR_SPEED_STEP),
        _ => 0.,
    }
}

impl Track {
    fn newest(&self) -> Option<u32> {
        self.states.back().map(|(t, _)| *t)
    }

    /// The rotors' turns at drawn tick `at`: the anchor's, plus the rotor
    /// speed integrated from the anchor through the kept states, each span
    /// a straight line, held past the newest.
    fn turns_at(&self, at: f64) -> f64 {
        let Some((anchor, mut turns)) = self.turns else {
            return 0.;
        };
        let mut from = f64::from(anchor);
        if at <= from {
            return turns;
        }
        let dt = tore_sim::flight::DT;
        let mut previous: Option<(f64, f64)> = None;
        for (tick, state) in &self.states {
            let (t, nr) = (f64::from(*tick), rotor_speed(state));
            if let Some((t0, n0)) = previous
                && t > from
            {
                let speed = |x: f64| n0 + (nr - n0) * (x - t0) / (t - t0);
                let to = at.min(t);
                let start = from.max(t0);
                if to > start {
                    turns += (to - start) * (speed(start) + speed(to)) / 2. * dt;
                    from = to;
                }
                if at <= t {
                    return turns;
                }
            }
            previous = Some((t, nr));
        }
        // Past the newest state, at its rotor speed.
        let held = self.states.back().map_or(0., |(_, s)| rotor_speed(s));
        turns + (at - from).max(0.) * held * dt
    }

    /// Drops the oldest kept state, first moving the turns anchor up to the
    /// next one.
    fn pop_front(&mut self) {
        if let (Some((anchor, _)), Some((next, _))) = (self.turns, self.states.get(1))
            && *next > anchor
        {
            self.turns = Some((*next, self.turns_at(f64::from(*next))));
        }
        self.states.pop_front();
    }

    fn insert(&mut self, tick: u32, state: EntityState, ticks_per_snapshot: u32, far_gap: f64) {
        let newest = self.newest();
        match self.states.iter().position(|(t, _)| *t >= tick) {
            Some(i) if self.states[i].0 == tick => self.states[i].1 = state,
            Some(i) => self.states.insert(i, (tick, state)),
            None => self.states.push_back((tick, state)),
        }
        if self.turns.is_none() {
            self.turns = Some((tick, f64::from(tick) * tore_sim::flight::DT));
        }
        while self.states.len() > KEPT {
            self.pop_front();
        }
        if self.removed.is_some_and(|removed| removed < tick) {
            self.removed = None;
        }
        // The entity's band, from the gap to the state before: a far one's
        // interval is the shortest of its last three gaps, so one lost
        // update does not stretch it.
        if let Some(newest) = newest.filter(|n| tick > *n) {
            let gap = f64::from(tick - newest);
            let tps = f64::from(ticks_per_snapshot);
            if gap >= far_gap {
                self.far = true;
                self.gaps.push_back(gap);
                while self.gaps.len() > 3 {
                    self.gaps.pop_front();
                }
            } else if gap <= 2. * tps {
                self.far = false;
                self.gaps.clear();
            }
        }
    }

    /// The extra delay the entity's band asks for: none for a near one, its
    /// interval for a far one (`far_interval` before its gaps are known), two
    /// while loss is high.
    fn target(&self, lossy: bool, far_interval: f64) -> f64 {
        if !self.far {
            return 0.;
        }
        let interval = self
            .gaps
            .iter()
            .copied()
            .reduce(f64::min)
            .unwrap_or(far_interval);
        (interval * if lossy { 2. } else { 1. }).min(EXTRA_MAX)
    }
}

/// What a frame's interpolation drew.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawn {
    pub aircraft: Vec<AircraftPose>,
    pub projectiles: Vec<ProjectilePose>,
    pub debris: Vec<DebrisPose>,
    pub pilots: Vec<PilotPose>,
    /// Entities drawn, and of them the ones drawn past their newest state.
    pub entities: usize,
    pub extrapolated: usize,
    /// Of the entities, the ones drawn with an extra delay, and of those
    /// the ones past their newest state.
    pub far: usize,
    pub far_extrapolated: usize,
}

/// Every entity's received states, and their drawing.
#[derive(Clone, Debug)]
pub struct Interpolator {
    ticks_per_snapshot: u32,
    /// The far band's interval and the gap that marks an entity far, ticks.
    far_interval: f64,
    far_gap: f64,
    tracks: BTreeMap<EntityKey, Track>,
    last: Option<Duration>,
}

impl Interpolator {
    /// Nothing received, from a host that sends a snapshot every
    /// `ticks_per_snapshot` ticks.
    pub fn new(ticks_per_snapshot: u32) -> Self {
        let ticks_per_snapshot = ticks_per_snapshot.clamp(1, 120);
        Self {
            ticks_per_snapshot,
            far_interval: f64::from(far_interval_ticks(ticks_per_snapshot)),
            far_gap: f64::from(far_gap_snapshots(ticks_per_snapshot) * ticks_per_snapshot),
            tracks: BTreeMap::new(),
            last: None,
        }
    }

    /// Forgets everything (a new seat).
    pub fn clear(&mut self) {
        self.tracks.clear();
    }

    /// Takes what one snapshot said.
    pub fn receive(&mut self, snapshot: &ReceivedSnapshot) {
        for entity in &snapshot.updated {
            self.tracks.entry(entity.key()).or_default().insert(
                snapshot.tick,
                entity.state,
                self.ticks_per_snapshot,
                self.far_gap,
            );
        }
        for key in &snapshot.removed {
            let track = self.tracks.entry(*key).or_default();
            if track.newest().is_none_or(|newest| newest <= snapshot.tick) {
                track.removed = Some(
                    track
                        .removed
                        .map_or(snapshot.tick, |r| r.max(snapshot.tick)),
                );
            }
        }
    }

    /// Slides every extra delay on to `now`; `lossy` while snapshots lost
    /// more than 1 percent over 10 seconds.
    pub fn advance(&mut self, now: Duration, lossy: bool) {
        let last = *self.last.get_or_insert(now);
        let dt = ticks_of(now.saturating_sub(last));
        self.last = Some(now.max(last));
        for track in self.tracks.values_mut() {
            let target = track.target(lossy, self.far_interval);
            if track.drawn {
                track.extra.slide(target, dt);
            } else {
                track.extra.ticks = target;
            }
        }
    }

    /// The entity's extra delay, ticks.
    pub fn extra(&self, key: EntityKey) -> Option<f64> {
        self.tracks.get(&key).map(|t| t.extra.ticks)
    }

    /// Everything drawn at host tick `render`. `name` resolves a projectile's
    /// weapon and shape; `own` is the player's plane, whose debris is drawn
    /// with its own airframe.
    pub fn draw(&mut self, render: f64, own: u32, name: &dyn Fn(NameIndex) -> String) -> Drawn {
        let tps = f64::from(self.ticks_per_snapshot);
        let (far_interval, far_gap) = (self.far_interval, self.far_gap);
        let mut drawn = Drawn::default();
        self.tracks.retain(|_, track| {
            let at = render - track.extra.ticks;
            // Gone for good once the removal is well behind the drawn time.
            if track
                .removed
                .is_some_and(|r| f64::from(r) + EXTRA_MAX + tps < at)
            {
                return false;
            }
            // States wholly behind the drawn time, but the last before it.
            while track.states.len() > 2
                && track
                    .states
                    .get(1)
                    .is_some_and(|(t, _)| f64::from(*t) <= at)
            {
                track.pop_front();
            }
            !(track.states.is_empty() && track.removed.is_none())
        });
        for (key, track) in &mut self.tracks {
            let at = render - track.extra.ticks;
            if track.removed.is_some_and(|r| f64::from(r) <= at) {
                continue;
            }
            let Some(&(first, _)) = track.states.front() else {
                continue;
            };
            // One state only and no second where a near entity would have
            // sent one: a far entity, drawn its interval back from the start.
            if track.states.len() == 1 && !track.drawn && at > f64::from(first) + far_gap {
                track.far = true;
                track.extra.ticks = track.extra.ticks.max(far_interval);
                continue;
            }
            let at = render - track.extra.ticks;
            if at < f64::from(first) {
                continue;
            }
            track.drawn = true;
            let after = track.states.iter().position(|(t, _)| f64::from(*t) > at);
            let sample = match after {
                Some(i) if i > 0 => {
                    let (t0, s0) = track.states[i - 1];
                    let (t1, s1) = track.states[i];
                    let s = (at - f64::from(t0)) / f64::from(t1 - t0);
                    between(&s0, &s1, f64::from(t1 - t0), s)
                }
                _ => {
                    let (t0, s0) = *track.states.back().expect("a state");
                    let ahead = (at - f64::from(t0)).max(0.);
                    let removed_soon = track.removed.is_some();
                    if ahead > 0. && !removed_soon {
                        drawn.extrapolated += 1;
                        if track.extra.ticks > 0. {
                            drawn.far_extrapolated += 1;
                        }
                    }
                    beyond(&s0, ahead.min(EXTRAPOLATE_TICKS))
                }
            };
            drawn.entities += 1;
            if track.extra.ticks > 0. {
                drawn.far += 1;
            }
            let id = key.id;
            match (key.kind, sample) {
                (EntityKind::Aircraft, Sample::Aircraft(mut pose)) => {
                    if pose.engine.rotor > 0. {
                        pose.engine.rotor_turns = track.turns_at(at);
                    }
                    drawn.aircraft.push(AircraftPose { id, ..pose });
                }
                (EntityKind::Projectile, Sample::Projectile(p, pos, vel, dir)) => {
                    drawn
                        .projectiles
                        .push(projectile_pose(id, &p, pos, vel, dir, name));
                }
                (EntityKind::Debris, Sample::Debris(d, pos, att)) => {
                    drawn.debris.push(DebrisPose {
                        owner: d.owner,
                        draw: if d.owner == own {
                            Draw::Ownship
                        } else {
                            d.model.map_or(Draw::Hidden, Draw::Model)
                        },
                        position: pos,
                        attitude: att,
                        variant: d.variant.map(usize::from),
                    })
                }
                (EntityKind::Pilot, Sample::Pilot(p, pos, heading)) => {
                    let (owner, crew) = crate::wire::entity::pilot_owner(p.owner);
                    drawn.pilots.push(PilotPose {
                        owner,
                        position: pos,
                        heading,
                        phase: p.phase,
                        crew,
                    });
                }
                _ => {}
            }
        }
        drawn
    }
}

/// One entity at the drawn time. Aircraft are the common case and a drawn
/// pose is plain data, so the other variants stay unboxed (the pose grew by
/// the rotor speed past clippy's size gap).
#[allow(clippy::large_enum_variant)]
pub(crate) enum Sample {
    Aircraft(AircraftPose),
    Projectile(ProjectileState, [f64; 3], [f64; 3], [f64; 2]),
    Debris(DebrisState, [f64; 3], [f64; 3]),
    Pilot(PilotState, [f64; 3], f64),
}

/// Velocity in feet per tick.
pub(crate) fn per_tick(v: [f64; 3]) -> [f64; 3] {
    v.map(|v| v / TICKS_PER_SECOND)
}

/// The cubic curve through `p0` and `p1` with velocities `v0` and `v1` (feet
/// per tick), `h` ticks apart, at fraction `s`.
pub(crate) fn hermite(
    p0: [f64; 3],
    v0: [f64; 3],
    p1: [f64; 3],
    v1: [f64; 3],
    h: f64,
    s: f64,
) -> [f64; 3] {
    let s2 = s * s;
    let s3 = s2 * s;
    let h00 = 2. * s3 - 3. * s2 + 1.;
    let h10 = s3 - 2. * s2 + s;
    let h01 = -2. * s3 + 3. * s2;
    let h11 = s3 - s2;
    std::array::from_fn(|i| h00 * p0[i] + h10 * h * v0[i] + h01 * p1[i] + h11 * h * v1[i])
}

pub(crate) fn lerp3(a: [f64; 3], b: [f64; 3], s: f64) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * s)
}

/// An angle from `a` to `b` the short way, at fraction `s`, in -pi to pi.
pub(crate) fn angle_between(a: f64, b: f64, s: f64) -> f64 {
    let d = (b - a + PI).rem_euclid(TAU) - PI;
    (a + d * s + PI).rem_euclid(TAU) - PI
}

fn attitude(angles: [u16; 3]) -> [f64; 3] {
    angles.map(radians)
}

pub(crate) fn blend_attitude(a: [u16; 3], b: [u16; 3], s: f64) -> [f64; 3] {
    let [y0, p0, b0] = attitude(a);
    let [y1, p1, b1] = attitude(b);
    Basis::new(y0, p0, b0)
        .blended(Basis::new(y1, p1, b1), s)
        .angles()
}

/// A rotorcraft's rotor speed and disk tilts in the picture's units (zero
/// without rotors).
pub fn rotor_of(state: &AircraftState) -> (f64, [[f64; 2]; 2]) {
    state.rotor.map_or((0., [[0.; 2]; 2]), |r| {
        (
            f64::from(r.speed) * ROTOR_SPEED_STEP,
            r.tilt.map(|t| t.map(|v| f64::from(v) * ROTOR_TILT_STEP)),
        )
    })
}

/// An aircraft state's pose at `position`, `velocity` and `attitude`, with
/// the devices and the rotor speed and disk tilts given (already blended).
pub fn aircraft_pose(
    state: &AircraftState,
    position: [f64; 3],
    velocity: [f64; 3],
    attitude: [f64; 3],
    devices: Option<[f64; tore_world::snapshot::DEVICES]>,
    rotor: (f64, [[f64; 2]; 2]),
) -> AircraftPose {
    AircraftPose {
        id: 0,
        aircraft: state.aircraft,
        draw: state.aircraft.map_or(Draw::Hidden, Draw::Model),
        position,
        attitude,
        velocity,
        devices,
        engine: Engine {
            lit: state.engine.lit,
            afterburner: state.engine.afterburner,
            rates: state.engine.rates.map(|r| f64::from(r) * RATE_STEP),
            rotor: rotor.0,
            // The drawing integrates the rotor speed into the blade angle.
            rotor_turns: 0.,
            rotor_tilt: rotor.1,
            flame: state.engine.flame,
        },
        damage: Damage {
            hp: state.damage.hp,
            initial_hp: state.damage.initial_hp,
            sections: state.damage.sections,
            structural: state.damage.structural,
        },
        airborne: state.status.airborne,
        wreck: state.status.wreck,
        crashed: state.status.crashed,
    }
}

/// An aircraft's devices in the picture's order and units.
pub fn devices_of(state: &AircraftState) -> Option<[f64; tore_world::snapshot::DEVICES]> {
    state.devices.map(|d| {
        let l = d.levels.map(|v| f64::from(v) / 255.);
        let s = d.surfaces.map(|v| f64::from(v) / 127.);
        [
            l[0],
            l[1],
            l[2],
            l[3],
            l[4],
            l[5],
            s[0],
            s[1],
            s[2],
            f64::from(d.speed) * SPEED_STEP,
            f64::from(d.throttle) / 255.,
            f64::from(d.lift_levels[0]) / 255.,
            f64::from(d.vector_yaw) / 127.,
            f64::from(d.lift_levels[1]) / 255.,
            f64::from(d.lift_levels[2]) / 255.,
            f64::from(d.gun_aim[0]) / 127.,
            f64::from(d.gun_aim[1]) / 127.,
            f64::from(d.gun_aim[2]) / 127.,
            f64::from(d.gun_aim[3]) / 127.,
            f64::from(d.gun_aim[4]) / 127.,
            f64::from(d.gun_aim[5]) / 127.,
            f64::from(d.gun_group),
        ]
    })
}

pub(crate) fn between(a: &EntityState, b: &EntityState, h: f64, s: f64) -> Sample {
    let (ma, mb) = (a.motion(), b.motion());
    let (p0, p1) = (ma.position_ft(), mb.position_ft());
    let (v0, v1) = (ma.velocity_fps(), mb.velocity_fps());
    let position = hermite(p0, per_tick(v0), p1, per_tick(v1), h, s);
    let velocity = lerp3(v0, v1, s);
    match (a, b) {
        (EntityState::Aircraft(x), EntityState::Aircraft(y)) => {
            let devices = match (devices_of(x), devices_of(y)) {
                (Some(d0), Some(d1)) => Some(std::array::from_fn(|i| {
                    if i == 21 {
                        if s < 1. { d0[i] } else { d1[i] }
                    } else {
                        d0[i] + (d1[i] - d0[i]) * s
                    }
                })),
                (d0, _) => d0,
            };
            let rotor = match (x.rotor, y.rotor) {
                (Some(_), Some(_)) => {
                    let ((n0, t0), (n1, t1)) = (rotor_of(x), rotor_of(y));
                    (
                        n0 + (n1 - n0) * s,
                        std::array::from_fn(|k| {
                            std::array::from_fn(|i| t0[k][i] + (t1[k][i] - t0[k][i]) * s)
                        }),
                    )
                }
                _ => rotor_of(x),
            };
            Sample::Aircraft(aircraft_pose(
                x,
                position,
                velocity,
                blend_attitude(x.attitude, y.attitude, s),
                devices,
                rotor,
            ))
        }
        (EntityState::Projectile(x), EntityState::Projectile(y)) => {
            let dir =
                [0, 1].map(|i| angle_between(radians(x.direction[i]), radians(y.direction[i]), s));
            Sample::Projectile(*x, position, velocity, dir)
        }
        (EntityState::Debris(x), EntityState::Debris(y)) => {
            Sample::Debris(*x, position, blend_attitude(x.attitude, y.attitude, s))
        }
        (EntityState::Pilot(x), EntityState::Pilot(y)) => Sample::Pilot(
            *x,
            position,
            angle_between(radians(x.heading), radians(y.heading), s),
        ),
        // A key never changes kind; keep the earlier state.
        _ => beyond(a, 0.),
    }
}

/// `state` carried `ahead` ticks along its velocity.
pub(crate) fn beyond(state: &EntityState, ahead: f64) -> Sample {
    let motion = state.motion();
    let v = motion.velocity_fps();
    let p = motion.position_ft();
    let position: [f64; 3] = std::array::from_fn(|i| p[i] + v[i] / TICKS_PER_SECOND * ahead);
    match state {
        EntityState::Aircraft(x) => Sample::Aircraft(aircraft_pose(
            x,
            position,
            v,
            attitude(x.attitude),
            devices_of(x),
            rotor_of(x),
        )),
        EntityState::Projectile(x) => Sample::Projectile(
            *x,
            position,
            v,
            [radians(x.direction[0]), radians(x.direction[1])],
        ),
        EntityState::Debris(x) => Sample::Debris(*x, position, attitude(x.attitude)),
        EntityState::Pilot(x) => Sample::Pilot(*x, position, radians(x.heading)),
    }
}

fn projectile_pose(
    id: u32,
    p: &ProjectileState,
    position: [f64; 3],
    velocity: [f64; 3],
    direction: [f64; 2],
    name: &dyn Fn(NameIndex) -> String,
) -> ProjectilePose {
    let [azimuth, elevation] = direction;
    let speed = velocity.iter().map(|v| v * v).sum::<f64>().sqrt();
    ProjectilePose {
        id,
        owner: p.owner,
        weapon: name(p.weapon),
        shape: p.shape.map(name),
        gun: false,
        tracer: false,
        position,
        previous: std::array::from_fn(|i| position[i] - velocity[i] * tore_sim::flight::DT),
        direction: [
            elevation.cos() * azimuth.sin(),
            elevation.sin(),
            elevation.cos() * azimuth.cos(),
        ],
        target: p.target,
        incoming: p.aimed_at_player,
        speed_f8: (speed * 256.)
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::entity::{Entity, Motion};

    fn aircraft(position: [f64; 3], velocity: [f64; 3]) -> EntityState {
        EntityState::Aircraft(AircraftState {
            aircraft: Some(tore_formats::aircraft::AircraftId::F18),
            motion: Motion::of(position, velocity),
            ..AircraftState::default()
        })
    }

    fn snapshot(tick: u32, states: &[(u32, EntityState)]) -> ReceivedSnapshot {
        ReceivedSnapshot {
            tick,
            updated: states
                .iter()
                .map(|(id, state)| Entity {
                    id: *id,
                    state: *state,
                })
                .collect(),
            ..ReceivedSnapshot::default()
        }
    }

    fn no_names(_: NameIndex) -> String {
        String::new()
    }

    #[test]
    fn a_turning_aircraft_follows_its_curve_between_snapshots() {
        // A circle of radius 6,000 ft flown at 600 ft/s.
        let (r, w) = (6000., 0.1);
        let at = |t: f64| {
            let a = w * t / 120.;
            (
                [r * a.sin(), 10_000., r * a.cos()],
                [r * w * a.cos(), 0., -r * w * a.sin()],
            )
        };
        let mut interp = Interpolator::new(4);
        for tick in (0..=80).step_by(4) {
            let (p, v) = at(f64::from(tick));
            interp.receive(&snapshot(tick, &[(3, aircraft(p, v))]));
        }
        let mut worst: f64 = 0.;
        for tenth in 0..400 {
            let render = 20. + f64::from(tenth) * 0.1;
            let drawn = interp.draw(render, 0, &no_names);
            let pose = &drawn.aircraft[0];
            let (truth, _) = at(render);
            let error = (0..3)
                .map(|i| (pose.position[i] - truth[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            worst = worst.max(error);
            assert_eq!(drawn.extrapolated, 0);
        }
        assert!(worst < 0.1, "worst {worst} ft");
    }

    #[test]
    fn past_its_newest_state_an_entity_goes_on_for_250_ms_then_holds() {
        let mut interp = Interpolator::new(4);
        interp.receive(&snapshot(0, &[(1, aircraft([0.; 3], [120., 0., 0.]))]));
        interp.receive(&snapshot(4, &[(1, aircraft([4., 0., 0.], [120., 0., 0.]))]));
        let drawn = interp.draw(14., 0, &no_names);
        assert_eq!(drawn.extrapolated, 1);
        assert!((drawn.aircraft[0].position[0] - 14.).abs() < 1e-9);
        let drawn = interp.draw(100., 0, &no_names);
        assert!((drawn.aircraft[0].position[0] - 34.).abs() < 1e-9);
    }

    /// A two-seater's second chute arrives under an id of its own and is
    /// drawn as its aircraft's second crew member, beside its pilot.
    #[test]
    fn a_pilot_and_the_second_crew_member_of_one_aircraft_are_drawn_apart() {
        use crate::wire::entity::{CREW_PILOT_BIT, PilotState, pilot_id};
        let pilot = |owner: u32, x: f64| {
            EntityState::Pilot(PilotState {
                owner,
                motion: Motion::of([x, 3000., 0.], [0.; 3]),
                heading: 0,
                phase: tore_sim::ejection::Phase::Parachute,
            })
        };
        let mut interp = Interpolator::new(4);
        interp.receive(&snapshot(
            0,
            &[
                (5, pilot(5, 0.)),
                (pilot_id(5, true), pilot(pilot_id(5, true), 40.)),
            ],
        ));
        let drawn = interp.draw(0., 5, &no_names);
        let mut seen: Vec<(u32, bool, f64)> = drawn
            .pilots
            .iter()
            .map(|p| (p.owner, p.crew, p.position[0]))
            .collect();
        seen.sort_by(|a, b| a.2.total_cmp(&b.2));
        assert_eq!(seen, [(5, false, 0.), (5, true, 40.)]);
        assert!(pilot_id(5, true) & CREW_PILOT_BIT != 0);
    }

    #[test]
    fn a_removed_entity_is_drawn_until_the_removal_and_then_not() {
        let mut interp = Interpolator::new(4);
        interp.receive(&snapshot(0, &[(1, aircraft([0.; 3], [0.; 3]))]));
        interp.receive(&snapshot(4, &[(1, aircraft([0.; 3], [0.; 3]))]));
        interp.receive(&ReceivedSnapshot {
            tick: 8,
            removed: vec![EntityKey {
                kind: EntityKind::Aircraft,
                id: 1,
            }],
            ..ReceivedSnapshot::default()
        });
        assert_eq!(interp.draw(6., 0, &no_names).aircraft.len(), 1);
        let drawn = interp.draw(8., 0, &no_names);
        assert!(drawn.aircraft.is_empty());
    }

    #[test]
    fn a_far_entity_is_drawn_its_interval_back_and_slides_when_it_comes_near() {
        let mut interp = Interpolator::new(4);
        let key = EntityKey {
            kind: EntityKind::Aircraft,
            id: 7,
        };
        interp.receive(&snapshot(0, &[(7, aircraft([0.; 3], [0.; 3]))]));
        interp.receive(&snapshot(60, &[(7, aircraft([0.; 3], [0.; 3]))]));
        interp.advance(Duration::ZERO, false);
        assert_eq!(interp.extra(key), Some(60.));
        let _ = interp.draw(100., 0, &no_names);
        // One update lost: the interval is still its shortest gap.
        interp.receive(&snapshot(180, &[(7, aircraft([0.; 3], [0.; 3]))]));
        interp.advance(Duration::from_millis(500), false);
        assert_eq!(interp.extra(key), Some(60.));
        // While loss is high a far entity is drawn two intervals back,
        // sliding there at a tenth of real time.
        interp.advance(Duration::from_millis(1500), true);
        assert!((interp.extra(key).unwrap() - 72.).abs() < 1e-9);
        interp.advance(Duration::from_secs(10), true);
        assert_eq!(interp.extra(key), Some(120.));
        // Now it comes every snapshot: the delay slides down at a tenth of
        // real time.
        interp.receive(&snapshot(184, &[(7, aircraft([0.; 3], [0.; 3]))]));
        interp.advance(Duration::from_secs(11), false);
        assert!((interp.extra(key).unwrap() - 108.).abs() < 1e-9);
    }

    /// The far band at every rate: an entity sent at the host's far
    /// interval is drawn that interval back, and one heard once is drawn the
    /// far interval back once a near one would have come again; a near one
    /// that loses two snapshots in a row stays near wherever the far band is
    /// 4 snapshots or more apart.
    #[test]
    fn the_far_band_follows_the_rate() {
        let key = EntityKey {
            kind: EntityKind::Aircraft,
            id: 7,
        };
        let still = || aircraft([0.; 3], [0.; 3]);
        for (tps, far) in [(2u32, 30u32), (4, 32), (8, 32), (10, 30), (12, 36)] {
            assert_eq!(far_interval_ticks(tps), far);
            let mut interp = Interpolator::new(tps);
            for k in 0..4 {
                interp.receive(&snapshot(k * far, &[(7, still())]));
            }
            interp.advance(Duration::ZERO, false);
            assert_eq!(interp.extra(key), Some(f64::from(far)), "tps {tps}");

            let mut once = Interpolator::new(tps);
            once.receive(&snapshot(0, &[(7, still())]));
            let gap = f64::from(far_gap_snapshots(tps) * tps);
            let _ = once.draw(gap + 1., 0, &no_names);
            assert_eq!(once.extra(key), Some(f64::from(far)), "tps {tps}");

            if far_gap_snapshots(tps) >= 4 {
                let mut near = Interpolator::new(tps);
                for tick in [0, tps, 2 * tps, 5 * tps] {
                    near.receive(&snapshot(tick, &[(7, still())]));
                }
                near.advance(Duration::ZERO, false);
                assert_eq!(near.extra(key), Some(0.), "tps {tps}");
            }
        }
        // At 60 a second a near entity may lose 6 snapshots (50 ms) in a row
        // and stay near.
        assert_eq!(far_gap_snapshots(2), 8);
    }

    #[test]
    fn angles_turn_the_short_way() {
        let a = angle_between(PI - 0.1, -PI + 0.1, 0.5);
        assert!((a.abs() - PI).abs() < 1e-9);
    }

    /// A CH-47 at `tick` with rotor speed `speed` (thousandths) and tilts.
    fn rotorcraft(tick: u32, speed: u16, tilt: [[i8; 2]; 2]) -> EntityState {
        EntityState::Aircraft(AircraftState {
            aircraft: Some(tore_formats::aircraft::AircraftId::Ch47),
            motion: Motion::of([f64::from(tick), 500., 0.], [120., 0., 0.]),
            rotor: Some(crate::wire::entity::RotorState { speed, tilt }),
            ..AircraftState::default()
        })
    }

    /// The rotor speed (a share) the test's states give at drawn tick `t`.
    fn speed_at(t: f64) -> f64 {
        // A droop from 100 to 80 percent over the first 40 ticks, then
        // steady: states every 4 ticks carry it in thousandths.
        (1000. - 200. * (t / 40.).min(1.)).round() / 1000.
    }

    fn droop(interp: &mut Interpolator, to: u32) {
        for tick in (0..=to).step_by(4) {
            let speed = (speed_at(f64::from(tick)) * 1000.).round() as u16;
            let tilt = [[(tick / 4) as i8, -3], [5, -((tick / 4) as i8)]];
            interp.receive(&snapshot(tick, &[(7, rotorcraft(tick, speed, tilt))]));
        }
    }

    #[test]
    fn remote_rotors_carry_their_speed_and_tilt_between_states() {
        let mut interp = Interpolator::new(4);
        droop(&mut interp, 80);
        let pose = &interp.draw(6., 0, &no_names).aircraft[0];
        // Halfway from tick 4 to tick 8.
        let expected = (speed_at(4.) + speed_at(8.)) / 2.;
        assert!((pose.engine.rotor - expected).abs() < 1e-12);
        let step = crate::wire::entity::ROTOR_TILT_STEP;
        assert!((pose.engine.rotor_tilt[0][0] - 1.5 * step).abs() < 1e-12);
        assert!((pose.engine.rotor_tilt[1][1] + 1.5 * step).abs() < 1e-12);
        assert!((pose.engine.rotor_tilt[0][1] + 3. * step).abs() < 1e-12);
        // A fixed-wing aircraft carries none.
        let mut plain = Interpolator::new(4);
        plain.receive(&snapshot(0, &[(1, aircraft([0.; 3], [0.; 3]))]));
        plain.receive(&snapshot(4, &[(1, aircraft([0.; 3], [0.; 3]))]));
        let engine = plain.draw(2., 0, &no_names).aircraft[0].engine;
        assert_eq!(
            (engine.rotor, engine.rotor_turns, engine.rotor_tilt),
            (0., 0., [[0.; 2]; 2])
        );
    }

    #[test]
    fn remote_blades_turn_by_the_integrated_rotor_speed_whatever_the_frame_rate() {
        let dt = tore_sim::flight::DT;
        // The exact integral of the states' straight lines from tick 0,
        // where the blades start as if turning at 100 percent since tick 0.
        let exact = |at: f64| {
            let mut turns = 0.;
            let mut t = 0.;
            while t < at {
                let next = (t + 4.).min(at);
                let (a, b) = (speed_at(t), speed_at(t + 4.));
                let end = a + (b - a) * (next - t) / 4.;
                turns += (next - t) * (a + end) / 2. * dt;
                t = next;
            }
            turns
        };
        // One client draws at 144 frames a second, another at 30, a third
        // jumps straight to the end; all three agree at every shared time.
        let mut results = Vec::new();
        for frame in [120. / 144., 4., 77.] {
            let mut interp = Interpolator::new(4);
            droop(&mut interp, 80);
            let mut render = 1.;
            let mut last: Option<f64> = None;
            while render < 77. {
                let turns = interp.draw(render, 0, &no_names).aircraft[0]
                    .engine
                    .rotor_turns;
                assert!((turns - exact(render)).abs() < 1e-9, "{frame}: {render}");
                if let Some(last) = last {
                    // Never backwards, never a jump of more than the frame's
                    // turning at 100 percent.
                    assert!(turns >= last && turns - last <= frame * dt + 1e-12);
                }
                last = Some(turns);
                render += frame;
            }
            results.push(
                interp.draw(77., 0, &no_names).aircraft[0]
                    .engine
                    .rotor_turns,
            );
        }
        assert!(
            results.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-12),
            "{results:?}"
        );
        assert!((results[0] - exact(77.)).abs() < 1e-9);
        // Past the newest state the rotor goes on at its last speed.
        let mut interp = Interpolator::new(4);
        droop(&mut interp, 80);
        let at = 90.;
        let turns = interp.draw(at, 0, &no_names).aircraft[0].engine.rotor_turns;
        assert!((turns - (exact(80.) + 10. * 0.8 * dt)).abs() < 1e-9);
    }

    #[test]
    fn the_blade_angle_anchor_moves_with_the_kept_states_without_a_jump() {
        // Far more states than are kept, drawn as time goes on, so the
        // oldest are dropped all along: the angle stays the exact integral.
        let dt = tore_sim::flight::DT;
        let mut interp = Interpolator::new(4);
        let mut previous = None;
        for tick in (0..2_000u32).step_by(4) {
            let speed = 900 + (tick % 200) as u16;
            interp.receive(&snapshot(
                tick,
                &[(7, rotorcraft(tick, speed, [[0; 2]; 2]))],
            ));
            if tick >= 8 {
                let render = f64::from(tick) - 6.;
                let turns = interp.draw(render, 0, &no_names).aircraft[0]
                    .engine
                    .rotor_turns;
                if let Some((at, before)) = previous {
                    let step: f64 = render - at;
                    assert!(turns > before && turns - before < step * 1.2 * dt);
                }
                previous = Some((render, turns));
            }
        }
    }
}
