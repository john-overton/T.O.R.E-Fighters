//! The headless bot: the client session with a scripted pilot, for the
//! simulator matrix (D10) and the LAN smoke test (D11). See
//! docs/ARCHITECTURE.md, "The client session".
//!
//! With nothing to pursue the pilot flies straight and level, then turns, in
//! a 40-second cycle, holding its seating altitude. An aircraft of the other
//! side within 12 nautical miles is pursued (slice BOT): an intercept course
//! while it is far, then the gun's own lead point, tracked with pull and
//! rudder; the gun fires while the line of fire passes within 40 feet of where
//! the round will meet the target, and a pass head on is broken off to the
//! right. The lead is the weapon's own ballistics ([`gunsight`]), aimed at
//! the target where the shooter's screen has it, as the host judges a round.
//! See docs/ARCHITECTURE.md, "The client session", for the whole of it.
//! It reads only what a player's game has: its predicted flight and the
//! frame's picture.
//!
//! In the lobby (slice EF4) the bot plays as a game with no lobby screen:
//! the client's automatic ready takes the slot asked for (or the first free
//! one) with the standard loadout and marks ready, after each return to the
//! lobby and each mission change too. A bot that is the King
//! ([`Bot::start_when_ready`]) starts the mission as soon as every player
//! holding a slot is ready.
//!
//! Chat (slice EF6): a bot sends the lines it is told to
//! ([`Bot::say_at`], [`Bot::quick_at`]) when their time comes, to its
//! receivers once it may (a line to anyone but All waits until it flies).
//! `tore-bot` prints every line its bots receive, with the receiver, so
//! tests and the windowed run can see both directions.
//!
//! Observers (stage F phase 2, slice F2-O1): a bot told to watch
//! ([`Bot::watch`]) takes no plane; whenever the mission flies it watches
//! with its camera on the subject given, and draws the observer's frames
//! ([`Bot::watched`], [`Bot::watched_aircraft`]).

use crate::client::{Client, ClientFrame, Controls};
use crate::wire::chat::{ChatSend, QuickMessage, Receiver, Refusal};
use crate::wire::events::{LinkEvent, WireEvent};
use crate::wire::messages::{LobbyPhase, RosterPlane, Subject};
use std::collections::BTreeSet;
use std::f64::consts::{PI, TAU};
use std::sync::Arc;
use std::time::Duration;
use tore_sim::attitude::{Basis, Vector, dot};
use tore_sim::combat::gunsight::{self, TargetObservation};
use tore_sim::combat::live::Configuration;
use tore_sim::flight::{self, PilotInput};
use tore_world::combat::launcher;
use tore_world::seats::SeatCommand;
use tore_world::snapshot::{AircraftPose, RenderSnapshot};

/// The cycle: straight, a left turn, straight, a right turn, 10 s each.
const LEG: Duration = Duration::from_secs(10);
/// The throttle the cruise holds.
const CRUISE_THROTTLE: f64 = 0.7;
/// An enemy this close is chased, feet (12 nm).
const CHASE_FEET: f64 = 12. * 6076.;
/// The gun's round, when the loadout is not known: its speed (feet a second,
/// the imported guns' 2,933 slowing to half that at the end of its life)
/// and the pull of gravity on it.
const ROUND_FPS: f64 = 2700.;
const GRAVITY: f64 = 32.17;
/// The gun fires at an enemy this near (feet) and no nearer than
/// [`GUN_CLOSE`], when the line of fire passes within [`FIRE_MISS_FEET`] of
/// where the round will meet it (the aircraft's radius is 28), and keeps on
/// firing while it passes within [`HOLD_MISS_FEET`]. Inside [`SNAP_FEET`] a
/// line of fire within [`SNAP_MISS_FEET`] is enough for a snap shot.
const GUN_FEET: f64 = 6000.;
const GUN_CLOSE: f64 = 450.;
const FIRE_MISS_FEET: f64 = 40.;
const HOLD_MISS_FEET: f64 = 55.;
const SNAP_FEET: f64 = 7000.;
const SNAP_MISS_FEET: f64 = 300.;
/// The enemy's acceleration is taken over at least this many seconds.
const ACCELERATION_WINDOW: f64 = 0.25;
/// The gun's round is solved afresh this often.
const SOLVE_EVERY: Duration = Duration::from_millis(25);
/// A pull of the trigger lasts at least this long, and the next comes this
/// long after it.
const MIN_BURST: Duration = Duration::from_millis(250);
const MIN_PAUSE: Duration = Duration::from_millis(150);
/// A kept target is dropped for the nearest when it is this many times as
/// far.
const KEEP_FACTOR: f64 = 1.5;
/// How far ahead of an enemy to aim when far off, seconds at most.
const LEAD_SECONDS: f64 = 12.;
/// Inside `GUN_NEAR` feet the gun's lead point is the aim, beyond `GUN_FAR`
/// the intercept course, between them a blend.
const GUN_NEAR: f64 = 6000.;
const GUN_FAR: f64 = 9000.;
/// An enemy flying at us (the cosine of its aspect below `HEAD_ON`) is shot
/// at until `BREAK_FEET` away, and then the pass is broken off up and to the
/// right; an overtaken one is, within `OVERTAKE_FEET`.
const HEAD_ON: f64 = -0.5;
const BREAK_FEET: f64 = 1500.;
const OVERTAKE_FEET: f64 = 450.;
const BREAK_BANK: f64 = 1.0;
/// Beyond this range the throttle is all there is.
const THROTTLE_RANGE: f64 = 9000.;
/// Below this height, feet, the aim is held above the horizon by this slope.
const FLOOR_FEET: f64 = 4000.;
const FLOOR_CLIMB: f64 = 0.35;
/// The stick gains. Far off the nose: bank per radian the heading is off and
/// the most bank, roll per radian of bank wanted and its damping, pull per
/// radian of elevation, the lift a bank takes back, the pitch damping, the
/// most push, and the angle off the nose past which all the pull is used.
/// Near the nose (inside `FINE_RADIANS`): the wings-level gain, the pull and
/// rudder per radian of aim off the nose and the yaw damping.
const FINE_RADIANS: f64 = 0.12;
const HARD_RADIANS: f64 = 0.6;
const BANK_PER_RADIAN: f64 = 8.;
const BANK_MAX: f64 = 1.4;
const ROLL_GAIN: f64 = 1.;
const ROLL_DAMP: f64 = 0.2;
const PULL_GAIN: f64 = 3.;
const LIFT_GAIN: f64 = 0.15;
const PITCH_DAMP: f64 = 0.5;
const PUSH_LIMIT: f64 = 0.25;
const LEVEL_GAIN: f64 = 0.8;
const FINE_PULL: f64 = 16.;
const FINE_RUDDER: f64 = 12.;
const YAW_DAMP: f64 = 2.;
const TRIM_GAIN: f64 = 6.;
const TRIM_MAX: f64 = 0.3;

/// The bot asks for a frame this often.
const FRAME_EVERY: Duration = Duration::from_millis(33);

/// The nose, as the flight's attitude gives it.
fn wrap(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn basis(flight: &flight::State) -> Basis {
    Basis::new(flight.yaw, flight.pitch, flight.bank)
}

/// Where a round fired now meets a target that flies on at `velocity`, from
/// `own`: the target's position when the round arrives, raised by the fall.
fn lead_point(own: Vector, target: Vector, velocity: Vector) -> Vector {
    let mut seconds = 0.;
    let mut point = target;
    for _ in 0..4 {
        point = std::array::from_fn(|i| target[i] + velocity[i] * seconds);
        let range = (0..3)
            .map(|i| (point[i] - own[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        seconds = range / ROUND_FPS;
    }
    point[1] += 0.5 * GRAVITY * seconds * seconds;
    point
}

/// The enemy a pilot pursues: its plane, its range and how far, feet, the
/// line of fire passes from where the gun's round would meet it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pursuit {
    pub target: u32,
    pub range: f64,
    pub miss: f64,
}

/// The scripted pilot.
#[derive(Clone, Debug, Default)]
pub struct ScriptedPilot {
    started: Option<Duration>,
    altitude: Option<f64>,
    burst_since: Option<Duration>,
    /// No new burst before this.
    paused_until: Option<Duration>,
    /// The enemy being pursued, kept while it is a fair choice.
    target: Option<u32>,
    /// The aircraft's loadout, whose first station is the gun
    /// ([`ScriptedPilot::arm`]).
    gun: Option<Arc<Configuration>>,
    /// The last flight tick and heading seen, for the yaw rate.
    last_yaw: Option<(u64, f64)>,
    yaw_rate: f64,
    /// What the near-the-nose pull and rudder have so far added up to,
    /// against the error left (the trim a steady error asks for), and the
    /// flight tick it was last added at.
    trim: [f64; 2],
    trim_tick: u64,
    /// The enemy's velocity when last sampled and what its acceleration is
    /// made of since (feet a second, a second squared), for leading it in a
    /// turn.
    motion: Option<(Duration, u32, Vector)>,
    acceleration: Vector,
    /// The gun's flight to the target, solved at the time given: the
    /// seconds and the fall, which change slowly.
    flight_of_round: Option<(Duration, u32, f64, f64)>,
    /// The throttle was set for a pursuit; the cruise sets it back.
    throttled: bool,
    /// Gun bursts begun.
    pub bursts: u64,
    /// Fly the cruise cycle whatever is about: no pursuit, no fire.
    pub passive: bool,
    /// The enemy pursued now.
    pub pursuing: Option<Pursuit>,
}

fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}

fn add_scaled(a: Vector, b: Vector, k: f64) -> Vector {
    std::array::from_fn(|i| a[i] + b[i] * k)
}

fn length(v: Vector) -> f64 {
    dot(v, v).sqrt()
}

/// The seconds in which something flying `speed` from here meets a target
/// that is `offset` away and flies on at `velocity`; the straight-line time
/// when it cannot.
fn intercept_seconds(offset: Vector, velocity: Vector, speed: f64) -> f64 {
    let a = dot(velocity, velocity) - speed * speed;
    let b = 2. * dot(offset, velocity);
    let c = dot(offset, offset);
    let straight = c.sqrt() / speed;
    let roots = if a.abs() < 1e-6 {
        (b.abs() > 1e-6).then(|| (-c / b, f64::NAN))
    } else {
        let disc = b * b - 4. * a * c;
        (disc >= 0.).then(|| {
            let root = disc.sqrt();
            ((-b - root) / (2. * a), (-b + root) / (2. * a))
        })
    };
    roots
        .into_iter()
        .flat_map(|(x, y)| [x, y])
        .filter(|t| t.is_finite() && *t > 0.)
        .fold(None, |best: Option<f64>, t| {
            Some(best.map_or(t, |b| b.min(t)))
        })
        .unwrap_or(straight)
}

impl ScriptedPilot {
    /// A pilot that starts its cycle when first asked.
    pub fn new() -> Self {
        Self::default()
    }

    /// Gives the pilot the aircraft's loadout, whose first station is the
    /// gun, so that its lead is the gun's own ballistics rather than a
    /// guess. A pilot without it leads by a fitted round speed.
    pub fn arm(&mut self, config: &Arc<Configuration>) {
        if !self
            .gun
            .as_ref()
            .is_some_and(|own| Arc::ptr_eq(own, config))
        {
            self.gun = Some(Arc::clone(config));
        }
    }

    /// The gun's mount on the aircraft.
    fn mount(&self) -> Vector {
        self.gun
            .as_ref()
            .and_then(|config| config.stations.first())
            .map_or([0.; 3], |station| station.mount)
    }

    /// Where the gun must point to hit a target `id` now at `target` flying
    /// `velocity`: its place when the round arrives, raised by the round's
    /// fall. The round's flight is the weapon's own when the loadout is
    /// known, solved afresh every [`SOLVE_EVERY`].
    fn lead(
        &mut self,
        now: Duration,
        id: u32,
        flight: &flight::State,
        target: Vector,
        velocity: Vector,
        acceleration: Vector,
    ) -> Vector {
        let Some(config) = self.gun.as_ref() else {
            return lead_point(flight.position, target, velocity);
        };
        let fresh = self.flight_of_round.is_some_and(|(at, who, ..)| {
            who == id && now >= at && now.saturating_sub(at) < SOLVE_EVERY
        });
        if !fresh {
            self.flight_of_round = config.stations.first().and_then(|station| {
                let solution = gunsight::solve_observed(
                    &station.weapon,
                    &launcher(flight),
                    station.mount,
                    Some(TargetObservation {
                        position: target,
                        velocity,
                    }),
                )
                .ok()??;
                Some((now, id, solution.seconds, solution.drop_ft))
            });
        }
        match self.flight_of_round {
            Some((_, who, seconds, drop)) if who == id => {
                let mut point = add_scaled(target, velocity, seconds);
                point = add_scaled(point, acceleration, 0.5 * seconds * seconds);
                point[1] += drop;
                point
            }
            _ => lead_point(flight.position, target, velocity),
        }
    }

    /// The enemy to pursue: the one it has, while it is still within reach
    /// and not much farther than the nearest, else the nearest.
    fn choose<'a>(
        &mut self,
        flight: &flight::State,
        picture: &'a RenderSnapshot,
        enemy: &dyn Fn(u32) -> bool,
    ) -> Option<&'a AircraftPose> {
        let mut nearest: Option<(f64, &AircraftPose)> = None;
        let mut kept: Option<(f64, &AircraftPose)> = None;
        for pose in picture.targets.iter().filter(|p| p.aircraft.is_some()) {
            if pose.crashed || pose.damage.hp <= 0 || !enemy(pose.id) {
                continue;
            }
            let d = length(sub(pose.position, flight.position));
            if !(1. ..=CHASE_FEET).contains(&d) {
                continue;
            }
            if Some(pose.id) == self.target {
                kept = Some((d, pose));
            }
            if nearest.is_none_or(|(n, _)| d < n) {
                nearest = Some((d, pose));
            }
        }
        let chosen = match (kept, nearest) {
            (Some((k, pose)), Some((n, _))) if k < n * KEEP_FACTOR => Some(pose),
            (_, Some((_, pose))) => Some(pose),
            _ => None,
        };
        self.target = chosen.map(|pose| pose.id);
        chosen
    }

    /// The enemy's acceleration, from how its velocity changed over the last
    /// quarter second or so (the picture's velocities move in steps of its
    /// frames), smoothed.
    fn track_acceleration(&mut self, now: Duration, id: u32, velocity: Vector) -> Vector {
        match self.motion {
            Some((at, who, before)) if who == id && now >= at => {
                let seconds = now.saturating_sub(at).as_secs_f64();
                if seconds >= ACCELERATION_WINDOW {
                    let rate: Vector = std::array::from_fn(|i| (velocity[i] - before[i]) / seconds);
                    self.acceleration = std::array::from_fn(|i| {
                        self.acceleration[i] + (rate[i] - self.acceleration[i]) * 0.5
                    });
                    self.motion = Some((now, id, velocity));
                }
            }
            _ => {
                self.acceleration = [0.; 3];
                self.motion = Some((now, id, velocity));
            }
        }
        self.acceleration
    }

    /// The heading rate from the flight's own heading, radians a second.
    fn track_yaw(&mut self, flight: &flight::State) -> f64 {
        match self.last_yaw {
            Some((tick, yaw)) if flight.ticks > tick + 2 => {
                let seconds = (flight.ticks - tick) as f64 / 120.;
                let rate = wrap(flight.yaw - yaw) / seconds;
                self.yaw_rate += (rate - self.yaw_rate) * 0.5;
                self.last_yaw = Some((flight.ticks, flight.yaw));
            }
            Some((tick, _)) if flight.ticks < tick => {
                self.last_yaw = Some((flight.ticks, flight.yaw));
            }
            None => self.last_yaw = Some((flight.ticks, flight.yaw)),
            _ => {}
        }
        self.yaw_rate
    }

    /// The sticks that fly the gun's line of fire to `aim`, a point in the
    /// world. Far off the nose: bank for the heading and pull for the
    /// elevation (hard when it is far off). Near it: wings level, pull for
    /// the height of the aim above the nose, rudder for its side, damped by
    /// the aircraft's own rates. Returns them with the angle left.
    fn steer(&mut self, flight: &flight::State, muzzle: Vector, aim: Vector) -> (PilotInput, f64) {
        let nose = basis(flight);
        let d = sub(aim, muzzle);
        let range = length(d).max(1.);
        let (tx, ty, tz) = (
            dot(d, nose.right) / range,
            dot(d, nose.up) / range,
            dot(d, nose.forward) / range,
        );
        let error = tz.clamp(-1., 1.).acos();
        let yaw_rate = self.track_yaw(flight);
        if error < FINE_RADIANS {
            let up = ty.atan2(tz);
            let side = tx.atan2(tz);
            // Add up the error left, for the trim it asks for.
            if flight.ticks > self.trim_tick && flight.ticks - self.trim_tick < 60 {
                let seconds = (flight.ticks - self.trim_tick) as f64 / 120.;
                self.trim[0] = (self.trim[0] + up * seconds * TRIM_GAIN).clamp(-TRIM_MAX, TRIM_MAX);
                self.trim[1] =
                    (self.trim[1] + side * seconds * TRIM_GAIN).clamp(-TRIM_MAX, TRIM_MAX);
            }
            self.trim_tick = flight.ticks;
            let roll = (-flight.bank * LEVEL_GAIN - flight.roll_rate * ROLL_DAMP).clamp(-1., 1.);
            let pitch =
                (up * FINE_PULL + self.trim[0] - flight.pitch_rate * PITCH_DAMP).clamp(-1., 1.);
            let yaw = (side * FINE_RUDDER + self.trim[1] - yaw_rate * YAW_DAMP).clamp(-1., 1.);
            return (
                PilotInput {
                    pitch,
                    roll,
                    yaw,
                    ..PilotInput::default()
                },
                error,
            );
        }
        let wanted = d[0].atan2(d[2]);
        let sideways = wrap(wanted - flight.yaw);
        let elevation = d[1].atan2(d[0].hypot(d[2]));
        let up = (elevation - flight.pitch).clamp(-1., 1.);
        let bank_wanted = (sideways * BANK_PER_RADIAN).clamp(-BANK_MAX, BANK_MAX);
        let roll =
            ((bank_wanted - flight.bank) * ROLL_GAIN - flight.roll_rate * ROLL_DAMP).clamp(-1., 1.);
        let lift = (1. / flight.bank.cos().abs().max(0.3) - 1.) * LIFT_GAIN;
        let mut pitch =
            (up * PULL_GAIN + lift - flight.pitch_rate * PITCH_DAMP).clamp(-PUSH_LIMIT, 1.);
        // Far off the nose, with the wings near where they are wanted: all
        // the pull there is.
        if error > HARD_RADIANS && (bank_wanted - flight.bank).abs() < 0.5 {
            pitch = 1.;
        }
        (
            PilotInput {
                pitch,
                roll,
                ..PilotInput::default()
            },
            error,
        )
    }

    /// The controls at `now` for `flight`, chasing and firing at the
    /// aircraft of `picture` that `enemy` says are the other side's.
    pub fn controls(
        &mut self,
        now: Duration,
        flight: &flight::State,
        picture: Option<&RenderSnapshot>,
        enemy: &dyn Fn(u32) -> bool,
    ) -> Controls {
        let started = *self.started.get_or_insert(now);
        self.altitude
            .get_or_insert(flight.position[1].clamp(3000., 30_000.));
        let target = picture
            .filter(|_| !self.passive)
            .and_then(|picture| self.choose(flight, picture, enemy));
        let Some(target) = target else {
            self.target = None;
            self.pursuing = None;
            return self.cruise(now, started, flight);
        };
        let nose = basis(flight);
        let mount = self.mount();
        let muzzle: Vector = std::array::from_fn(|i| {
            flight.position[i]
                + nose.right[i] * mount[0]
                + nose.up[i] * mount[1]
                + nose.forward[i] * mount[2]
        });
        // The target where the shooter's screen has it: the host judges a
        // gun round against the targets as the shooter saw them.
        let position = target.position;
        let velocity = target.velocity;
        let offset = sub(position, muzzle);
        let range = length(offset);
        let line = offset.map(|v| v / range.max(1.));
        let speed = length(flight.velocity).max(300.);
        let target_speed = length(velocity);
        // Positive while the target flies away from us: we are behind it.
        let away = if target_speed > 30. {
            dot(velocity, line) / target_speed
        } else {
            0.
        };
        let acceleration = self.track_acceleration(now, target.id, velocity);
        let gun = self.lead(now, target.id, flight, position, velocity, acceleration);
        let seconds = intercept_seconds(offset, velocity, speed).min(LEAD_SECONDS);
        let intercept = add_scaled(position, velocity, seconds);
        let blend = ((range - GUN_NEAR) / (GUN_FAR - GUN_NEAR)).clamp(0., 1.);
        let mut aim: Vector = std::array::from_fn(|i| gun[i] + (intercept[i] - gun[i]) * blend);
        // Never aim into the ground.
        if flight.position[1] < FLOOR_FEET {
            aim[1] = aim[1].max(flight.position[1] + range * FLOOR_CLIMB);
        }
        let closing = -dot(sub(velocity, flight.velocity), line);
        let (mut input, _) = self.steer(flight, muzzle, aim);
        // A pass is not flown through: break off up and to the right, a
        // head-on one early (the closing speed is two aircraft's), an
        // overtaking one when it is close.
        let breaking = closing > 150.
            && range
                < if away < HEAD_ON {
                    BREAK_FEET
                } else {
                    OVERTAKE_FEET
                };
        if breaking {
            input = PilotInput {
                pitch: 1.,
                roll: ((BREAK_BANK - flight.bank) * ROLL_GAIN).clamp(-1., 1.),
                ..PilotInput::default()
            };
        }
        // Throttle: all of it while the target is far or the nose is off it;
        // behind it and close, the target's speed and a little more.
        let throttle = if range > THROTTLE_RANGE || away < 0. {
            1.
        } else {
            let wanted = target_speed + ((range - GUN_CLOSE) * 0.08).clamp(0., 250.);
            (0.75 + (wanted - flight.speed) * 0.004).clamp(0.3, 1.)
        };
        input.throttle = Some(throttle);
        self.throttled = true;
        // Fire while the line of fire is on the gun's lead point: the miss
        // there is the angle left at the range, against the aircraft's size.
        let miss = Self::angle(&nose, muzzle, gun) * range;
        let limit = if self.burst_since.is_some() {
            HOLD_MISS_FEET
        } else {
            FIRE_MISS_FEET
        };
        // And, in a close fight with an enemy that turns, a snap shot at the
        // lead point as the nose crosses it: the aim cannot hold with that,
        // but the burst is the one chance.
        let snap = range < SNAP_FEET && miss < SNAP_MISS_FEET;
        let fire = !breaking && range > GUN_CLOSE && ((range < GUN_FEET && miss < limit) || snap);
        self.pursuing = Some(Pursuit {
            target: target.id,
            range,
            miss,
        });
        let trigger = self.trigger(now, fire);
        Controls {
            pilot: input,
            trigger,
            ..Controls::default()
        }
    }

    /// The angle between the nose and `point`, seen from `muzzle`.
    fn angle(nose: &Basis, muzzle: Vector, point: Vector) -> f64 {
        let d = sub(point, muzzle);
        (dot(d, nose.forward) / length(d).max(1.))
            .clamp(-1., 1.)
            .acos()
    }

    /// The trigger: held while `fire`, each pull a burst. A pull lasts at
    /// least [`MIN_BURST`], and the next waits [`MIN_PAUSE`].
    fn trigger(&mut self, now: Duration, fire: bool) -> bool {
        match self.burst_since {
            Some(since) if fire || now.saturating_sub(since) < MIN_BURST => true,
            Some(since) => {
                // Released: the pause counts from here.
                self.burst_since = None;
                self.paused_until = Some(now.max(since) + MIN_PAUSE);
                false
            }
            None if fire && self.paused_until.is_none_or(|until| now >= until) => {
                self.burst_since = Some(now);
                self.bursts += 1;
                true
            }
            None => false,
        }
    }

    /// Nothing to pursue: straight and level, then turns, in the 40-second
    /// cycle, holding the altitude of the first seating.
    fn cruise(&mut self, now: Duration, started: Duration, flight: &flight::State) -> Controls {
        let hold = self
            .altitude
            .unwrap_or_else(|| flight.position[1].clamp(3000., 30_000.));
        let leg = (now.saturating_sub(started).as_secs_f64() / LEG.as_secs_f64()) as u64 % 4;
        let bank_target = match leg {
            1 => -0.8,
            3 => 0.8,
            _ => 0.,
        };
        let roll = ((bank_target - flight.bank) * 1.2).clamp(-0.6, 0.6);
        let climb = ((hold - flight.position[1]) * 0.15).clamp(-80., 80.);
        let pull = (1. / flight.bank.cos().abs().max(0.3) - 1.) * 0.25;
        let pitch = ((climb - flight.vertical_speed) * 0.01 + pull).clamp(-0.4, 0.6);
        // The throttle a pursuit set goes back to where the game starts it.
        let throttle = std::mem::take(&mut self.throttled).then_some(CRUISE_THROTTLE);
        Controls {
            pilot: PilotInput {
                pitch,
                roll,
                throttle,
                ..PilotInput::default()
            },
            ..Controls::default()
        }
    }
}

/// The planes the roster puts on the other side from the client's own.
pub fn enemies(client: &Client) -> BTreeSet<u32> {
    let (Some(roster), Some((_, own))) = (client.roster(), client.seat()) else {
        return BTreeSet::new();
    };
    let side = |id: u32| {
        roster
            .planes
            .iter()
            .find(|p: &&RosterPlane| p.id == id)
            .map(|p| p.wing.side)
    };
    let Some(mine) = side(own.0) else {
        return BTreeSet::new();
    };
    roster
        .planes
        .iter()
        .filter(|p| p.wing.side != mine)
        .map(|p| p.id)
        .collect()
}

/// How long after its first eject press a bot presses again to confirm.
const EJECT_CONFIRM: Duration = Duration::from_millis(500);

/// A client with the scripted pilot.
pub struct Bot {
    pub client: Client,
    pub pilot: ScriptedPilot,
    last_frame: Option<Duration>,
    picture: Option<RenderSnapshot>,
    /// Frames drawn.
    pub frames: u64,
    /// As the King, start the mission once every player holding a slot is
    /// ready.
    pub start_when_ready: bool,
    /// The lobby state the last Start was asked for.
    asked: Option<crate::wire::messages::LobbyState>,
    /// When the bot's first update was, which its chat times count from.
    started: Option<Duration>,
    /// Lines to send, each at a time after `started`.
    chat: Vec<(Duration, ChatSend)>,
    /// Lines the host's rules refused the bot, with why.
    pub chat_refused: Vec<(String, Refusal)>,
    /// Lines sent.
    pub chat_sent: u64,
    /// Watch the flying mission with the camera on this subject, flying no
    /// plane ([`Bot::watch`]).
    observe: Option<Subject>,
    /// The watch was asked for in this flying spell of the lobby.
    observe_asked: bool,
    /// Observer frames drawn, and the aircraft the last one drew.
    pub watched: u64,
    pub watched_aircraft: usize,
    /// Eject this long after the first seating, then fly again when the
    /// host allows it ([`Bot::revive_after`], stage F phase 2).
    revive_after: Option<Duration>,
    /// When the bot was first seated.
    first_seated: Option<Duration>,
    /// The bot has pressed eject twice, the second press confirming the
    /// first (retail's rule: within two seconds).
    pub ejected: bool,
    /// When eject was first pressed.
    eject_pressed: Option<Duration>,
    /// Revive was asked for since the plane was lost.
    revive_asked: bool,
    /// The plane it flies now.
    plane: Option<u32>,
    /// Go away this long after the first seating and come back after the
    /// second span ([`Bot::away_after`], stage F phase 2).
    away_plan: Option<(Duration, Duration)>,
    /// When the bot was first seated, for the away plan.
    away_seated: Option<Duration>,
    /// Away was sent, and when the AI took its plane.
    away_sent: bool,
    away_since: Option<Duration>,
    /// The AI flew its plane while it was away, and it asked for it back.
    pub went_away: bool,
    pub came_back: bool,
    /// Seat commands to give once each, this long after the first seating
    /// ([`Bot::send_at`], stage F phase 2, slice F2-R).
    scripted: Vec<(Duration, SeatCommand)>,
    /// When the bot was first seated, for the scripted commands.
    scripted_seated: Option<Duration>,
    /// The radio lines the bot heard ("Red two: 'Winchester'") and the HUD
    /// lines it read, in order, for `tore-bot` to print.
    pub radio_heard: Vec<String>,
    pub lines_read: Vec<String>,
    /// The data link as the bot saw it (slice G7): each Link event in words
    /// ("plane 0 assigned plane 1 bandit 7 (Sort)") and each change of the
    /// readout's assignment ("assigned: bandit 7 by plane 0"), in order.
    pub link_heard: Vec<String>,
    /// The readout's assignment last seen, for its changes.
    link_assigned: Option<tore_world::readout::LinkAssigned>,
    /// The flightmates' assignments the readout's marks last showed, by
    /// bandit (a mask of member numbers), for their changes.
    link_marked: std::collections::BTreeMap<u32, u16>,
}

/// A data link change in words, for `tore-bot` to print (slice G7).
pub fn link_words(link: &LinkEvent) -> String {
    match *link {
        LinkEvent::Member { plane, radar } => format!(
            "plane {plane} joined{}",
            if radar { "" } else { " with no radar" }
        ),
        LinkEvent::Lock { plane, target } => format!("plane {plane} locked {target}"),
        LinkEvent::Unlock { plane, target } => format!("plane {plane} let go of {target}"),
        LinkEvent::Assign {
            plane,
            target,
            by,
            order,
        } => format!("plane {by} assigned plane {plane} bandit {target} ({order:?})"),
        LinkEvent::Clear { plane, target, why } => {
            format!("plane {plane} cleared of {target} ({})", why.name())
        }
        LinkEvent::Acknowledge { plane, target } => {
            format!("plane {plane} acknowledged {target}")
        }
        LinkEvent::SortWarning {
            plane,
            other,
            target,
        } => format!("plane {plane} warned: plane {other} holds {target}"),
    }
}

impl Bot {
    /// The bot for a client that has started joining.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            pilot: ScriptedPilot::new(),
            last_frame: None,
            picture: None,
            frames: 0,
            start_when_ready: false,
            asked: None,
            started: None,
            chat: Vec::new(),
            chat_refused: Vec::new(),
            chat_sent: 0,
            observe: None,
            observe_asked: false,
            watched: 0,
            watched_aircraft: 0,
            revive_after: None,
            first_seated: None,
            ejected: false,
            eject_pressed: None,
            revive_asked: false,
            plane: None,
            away_plan: None,
            away_seated: None,
            away_sent: false,
            away_since: None,
            went_away: false,
            came_back: false,
            scripted: Vec::new(),
            scripted_seated: None,
            radio_heard: Vec::new(),
            lines_read: Vec::new(),
            link_heard: Vec::new(),
            link_assigned: None,
            link_marked: Default::default(),
        }
    }

    /// Gives the seat command `command` once, `after` the first seating: an
    /// order from a lead, or a reply from a wingman (stage F phase 2, slice
    /// F2-R's `tore-bot --order` and `--reply`).
    pub fn send_at(&mut self, after: Duration, command: SeatCommand) {
        self.scripted.push((after, command));
    }

    /// The scripted commands that are due.
    fn scripted_commands(&mut self, now: Duration, controls: &mut Controls) {
        if self.scripted.is_empty() || self.client.seat().is_none() {
            return;
        }
        let seated = *self.scripted_seated.get_or_insert(now);
        let due = now.saturating_sub(seated);
        let (ready, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.scripted)
            .into_iter()
            .partition(|(after, _)| due >= *after);
        self.scripted = waiting;
        controls
            .commands
            .extend(ready.into_iter().map(|(_, command)| command));
    }

    /// Keeps what a frame's events said to the player: radio lines and HUD
    /// lines (bounded, as the client's own list of events is).
    fn hear(&mut self, frame: &ClientFrame) {
        for event in &frame.events {
            match &event.event {
                WireEvent::Radio { label, text, .. } => {
                    self.radio_heard.push(format!("{label}: '{text}'"));
                }
                WireEvent::Message { text } => self.lines_read.push(text.clone()),
                WireEvent::Link(link) => self.link_heard.push(link_words(link)),
                _ => {}
            }
        }
        let assigned = frame.readout.as_ref().and_then(|r| r.link.assigned);
        if assigned != self.link_assigned {
            self.link_heard.push(match assigned {
                Some(a) => format!(
                    "assigned: bandit {} by plane {}{}",
                    a.target,
                    a.by,
                    if a.acknowledged { ", locked" } else { "" }
                ),
                None => "assigned: none".into(),
            });
            self.link_assigned = assigned;
        }
        // The flightmates' assignments as the readout marks them.
        if let Some(readout) = &frame.readout {
            let marked: std::collections::BTreeMap<u32, u16> = readout
                .link
                .marks
                .iter()
                .filter(|mark| mark.assigned_to != 0)
                .map(|mark| (mark.target, mark.assigned_to))
                .collect();
            for (target, mask) in &marked {
                if self.link_marked.get(target) != Some(mask) {
                    let numbers: Vec<String> = tore_world::readout::LinkMark::numbers(*mask)
                        .iter()
                        .map(u8::to_string)
                        .collect();
                    self.link_heard.push(format!(
                        "marked: bandit {target} assigned to {}",
                        numbers.join(", ")
                    ));
                }
            }
            self.link_marked = marked;
        }
        for list in [
            &mut self.radio_heard,
            &mut self.lines_read,
            &mut self.link_heard,
        ] {
            if list.len() > 256 {
                list.drain(..list.len() - 256);
            }
        }
    }

    /// The bot's game says it is away `after` its first seating (as a game
    /// left at its menu does), and once the AI flies its plane, says it is
    /// back `away` later and flies on in it (stage F phase 2, slice F2-A's
    /// `tore-bot --away`). Once.
    pub fn away_after(&mut self, after: Duration, away: Duration) {
        self.away_plan = Some((after, away));
    }

    /// Away when due, and Back once the AI has flown the plane long enough.
    fn away(&mut self, now: Duration) {
        let Some((after, away)) = self.away_plan else {
            return;
        };
        if self.client.seat().is_some() {
            let seated = *self.away_seated.get_or_insert(now);
            if !self.away_sent && now.saturating_sub(seated) >= after {
                self.away_sent = true;
                self.client.away();
            }
            return;
        }
        if self.client.ai_flies().is_some() {
            self.went_away = true;
            let since = *self.away_since.get_or_insert(now);
            if !self.came_back && now.saturating_sub(since) >= away {
                self.came_back = true;
                self.client.back();
            }
        }
    }

    /// The bot ejects `after` its first seating and then, once the host's
    /// Revival allows it, asks to fly again (Revive), and flies on in the
    /// plane it gets (stage F phase 2, slice F2-V's `tore-bot --revive`).
    pub fn revive_after(&mut self, after: Duration) {
        self.revive_after = Some(after);
    }

    /// The ejection when it is due, and Revive once it may fly again.
    fn revive(&mut self, now: Duration, controls: &mut Controls) {
        let Some(after) = self.revive_after else {
            return;
        };
        let plane = self.client.seat().map(|(_, plane)| plane.0);
        if plane.is_some() && plane != self.plane {
            self.plane = plane;
            self.revive_asked = false;
        }
        if plane.is_none() {
            return;
        }
        let first = *self.first_seated.get_or_insert(now);
        match self.eject_pressed {
            None if now.saturating_sub(first) >= after => {
                self.eject_pressed = Some(now);
                controls.pilot.commands.push(flight::PilotCommand::Eject);
            }
            Some(pressed) if !self.ejected && now.saturating_sub(pressed) >= EJECT_CONFIRM => {
                self.ejected = true;
                controls.pilot.commands.push(flight::PilotCommand::Eject);
            }
            _ => {}
        }
        if self.ejected && !self.revive_asked && self.client.may_fly_again() {
            self.revive_asked = true;
            self.client.revive();
        }
    }

    /// The bot watches instead of flying: whenever the mission flies it asks
    /// to watch with the camera on `subject`. Its client should not take a
    /// plane by itself ([`crate::client::ClientConfig::auto_ready`] off).
    pub fn watch(&mut self, subject: Subject) {
        self.observe = Some(subject);
    }

    /// Asks to watch once the mission flies, again after each mission.
    fn watch_when_flying(&mut self) {
        let Some(subject) = self.observe else {
            return;
        };
        let flying = self
            .client
            .lobby()
            .is_some_and(|l| l.phase == LobbyPhase::Flying)
            && self.client.mission().is_some();
        if !flying {
            self.observe_asked = false;
        } else if !self.observe_asked && self.client.watching().is_none() {
            self.observe_asked = true;
            self.client.watch(subject);
        }
    }

    /// Sends `text` to `receiver` `after` the bot's first update.
    pub fn say_at(&mut self, after: Duration, receiver: Receiver, text: &str) {
        self.chat.push((after, ChatSend::typed(receiver, text)));
    }

    /// Sends `CHAT.TXT`'s line `number` (1 to 12) of `lines` `after` the
    /// bot's first update, to the line's receiver or All. A number past
    /// the file's lines is dropped, noted in [`Bot::chat_refused`].
    pub fn quick_at(&mut self, after: Duration, number: u8, lines: &[QuickMessage]) {
        match lines.get(usize::from(number).wrapping_sub(1)) {
            Some(line) => self
                .chat
                .push((after, ChatSend::quick(number, line, Receiver::All))),
            None => self
                .chat_refused
                .push((format!("quick message {number}"), Refusal::BadQuick)),
        }
    }

    /// Sends the lines that are due and can go.
    fn send_chat(&mut self, now: Duration) {
        let started = *self.started.get_or_insert(now);
        let mut i = 0;
        while i < self.chat.len() {
            if now.saturating_sub(started) < self.chat[i].0 {
                i += 1;
                continue;
            }
            match self.client.chat_send(self.chat[i].1.clone()) {
                Ok(()) => {
                    self.chat_sent += 1;
                    self.chat.remove(i);
                }
                // Not yet: connecting, or waiting for a plane for a line
                // to anyone but All.
                Err(Refusal::NotConnected | Refusal::OnlyAll) => i += 1,
                Err(refusal) => {
                    let (_, send) = self.chat.remove(i);
                    self.chat_refused.push((send.text, refusal));
                }
            }
        }
    }

    /// The King's start, when every player holding a slot is ready and the
    /// lobby has changed since the last time it asked.
    fn start_if_ready(&mut self) {
        if !self.start_when_ready {
            return;
        }
        let Some(lobby) = self.client.lobby() else {
            return;
        };
        if lobby.is_king()
            && lobby.phase == crate::wire::messages::LobbyPhase::Lobby
            && lobby.all_ready()
            && self.asked.as_ref() != Some(lobby)
        {
            self.asked = Some(lobby.clone());
            self.client.start_mission();
        }
    }

    /// One update at `now`: a frame when due, the pilot's controls, the
    /// client's update. Returns the frame when one was drawn.
    pub fn update(&mut self, now: Duration) -> Option<ClientFrame> {
        let mut drawn = None;
        if self
            .last_frame
            .is_none_or(|last| now.saturating_sub(last) >= FRAME_EVERY)
        {
            self.last_frame = Some(now);
            if let Some(frame) = self.client.frame(now) {
                self.frames += 1;
                self.pilot.arm(&frame.config);
                self.picture = Some(frame.picture.clone());
                self.hear(&frame);
                drawn = Some(frame);
            } else if let Some(frame) = self.client.observer_frame(now) {
                self.watched += 1;
                self.watched_aircraft = frame
                    .picture
                    .targets
                    .iter()
                    .filter(|t| t.aircraft.is_some())
                    .count();
            }
        }
        let controls = match self.client.prediction() {
            Some(prediction) => {
                let flight = prediction.plane().flight.clone();
                let picture = self.picture.take();
                let enemies = enemies(&self.client);
                let controls = self
                    .pilot
                    .controls(now, &flight, picture.as_ref(), &|id| enemies.contains(&id));
                self.picture = picture;
                controls
            }
            None => Controls::default(),
        };
        let mut controls = controls;
        self.revive(now, &mut controls);
        self.scripted_commands(now, &mut controls);
        self.away(now);
        self.client.update(now, &controls);
        self.watch_when_flying();
        self.start_if_ready();
        self.send_chat(now);
        drawn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{ClientConfig, ClientPhase};
    use crate::host::{Host, HostConfig, HostLog, OpenPlanes, StartMode};
    use crate::settings::{KillOwner, Mode, number};
    use std::collections::BTreeMap;
    use tore_formats::aircraft::AircraftId;
    use tore_net::Entropy;
    use tore_net::sim::{LinkConfig, SimNetwork};
    use tore_world::mission::MissionSpec;

    fn build() -> crate::BuildId {
        crate::BuildId {
            version: "test".into(),
            commit: "test".into(),
            release: false,
        }
    }

    /// A target aircraft of the picture.
    fn pose(id: u32, position: Vector, velocity: Vector) -> AircraftPose {
        AircraftPose {
            id,
            aircraft: Some(AircraftId::Mig29),
            position,
            velocity,
            damage: tore_world::snapshot::Damage {
                hp: 100,
                ..Default::default()
            },
            ..AircraftPose::default()
        }
    }

    #[test]
    fn an_intercept_is_found_head_on_and_in_a_stern_chase() {
        // Head on at 1,000 and 500 feet a second, 15,000 feet apart: meet
        // in 10 seconds, when the target has flown 5,000 feet.
        let seconds = intercept_seconds([15_000., 0., 0.], [-500., 0., 0.], 1000.);
        assert!((seconds - 10.).abs() < 1e-9, "{seconds}");
        // Chasing a slower target: 10,000 feet behind at 600 against 1,000
        // closes in 25 seconds.
        let seconds = intercept_seconds([10_000., 0., 0.], [600., 0., 0.], 1000.);
        assert!((seconds - 25.).abs() < 1e-9, "{seconds}");
        // A faster target running away is never met: the straight line's time.
        let seconds = intercept_seconds([10_000., 0., 0.], [1500., 0., 0.], 1000.);
        assert!((seconds - 10.).abs() < 1e-9, "{seconds}");
        // A crossing target: the aim leads it.
        let seconds = intercept_seconds([8_000., 0., 6_000.], [0., 0., -300.], 1000.);
        let met = [8_000., 0., 6_000. - 300. * seconds];
        assert!((length(met) - 1000. * seconds).abs() < 1e-6);
    }

    #[test]
    fn the_lead_without_a_loadout_is_ahead_of_the_target_and_raised_for_the_fall() {
        let own = [0.; 3];
        // A target ahead and still: only the fall, about 20 feet at 3,000
        // feet (a second and a bit of flight).
        let still = lead_point(own, [0., 0., 3000.], [0.; 3]);
        assert!(still[1] > 15. && still[1] < 25., "{still:?}");
        assert!(still[0].abs() < 1e-9 && (still[2] - 3000.).abs() < 1e-9);
        // A crossing target is led along its velocity, by the round's time of
        // flight.
        let led = lead_point(own, [0., 0., 3000.], [500., 0., 0.]);
        assert!(led[0] > 500. && led[0] < 650., "{led:?}");
    }

    #[test]
    fn the_trigger_holds_each_burst_and_waits_between_bursts() {
        let mut pilot = ScriptedPilot::new();
        let at = |ms: u64| Duration::from_millis(ms);
        assert!(!pilot.trigger(at(0), false));
        assert!(pilot.trigger(at(10), true), "a burst begins");
        assert!(pilot.trigger(at(100), false), "held for its shortest");
        assert!(pilot.trigger(at(200), false));
        assert!(!pilot.trigger(at(300), false), "then released");
        assert!(!pilot.trigger(at(350), true), "and a pause waited out");
        assert!(pilot.trigger(at(500), true), "the next burst");
        assert_eq!(pilot.bursts, 2);
        // A long hold is one burst.
        for ms in (510..3000).step_by(10) {
            assert!(pilot.trigger(at(ms), true));
        }
        assert_eq!(pilot.bursts, 2);
    }

    #[test]
    fn the_enemy_pursued_is_the_nearest_and_kept_until_another_is_much_nearer() {
        let flight = test_flight();
        let enemies = [3, 4, 5];
        let is_enemy = |id: u32| enemies.contains(&id);
        let mut picture = RenderSnapshot {
            targets: vec![
                pose(1, [0., 0., 2000.], [0.; 3]), // a friend, nearest
                pose(3, [0., 0., 20_000.], [0.; 3]),
                pose(4, [0., 0., 30_000.], [0.; 3]),
                pose(9, [0., 0., 200_000.], [0.; 3]), // not an enemy, and far
            ],
            ..RenderSnapshot::default()
        };
        let mut pilot = ScriptedPilot::new();
        let chosen = pilot.choose(&flight, &picture, &is_enemy).map(|p| p.id);
        assert_eq!(chosen, Some(3), "the nearest enemy, not the friend");
        // Another comes nearer than the first, but not much: the first stays.
        picture.targets.push(pose(5, [0., 0., 15_000.], [0.; 3]));
        let chosen = pilot.choose(&flight, &picture, &is_enemy).map(|p| p.id);
        assert_eq!(chosen, Some(3));
        // The first is lost: the nearest is chosen.
        picture.targets[1].damage.hp = 0;
        let chosen = pilot.choose(&flight, &picture, &is_enemy).map(|p| p.id);
        assert_eq!(chosen, Some(5));
        // Past the chase distance there is nothing to pursue.
        picture.targets = vec![pose(3, [0., 0., CHASE_FEET + 1000.], [0.; 3])];
        assert!(pilot.choose(&flight, &picture, &is_enemy).is_none());
    }

    #[test]
    fn with_nothing_to_pursue_the_pilot_flies_its_cycle_and_never_fires() {
        let flight = test_flight();
        let mut pilot = ScriptedPilot::new();
        let picture = RenderSnapshot::default();
        for ms in (0..40_000).step_by(100) {
            let controls =
                pilot.controls(Duration::from_millis(ms), &flight, Some(&picture), &|_| {
                    true
                });
            assert!(!controls.trigger);
            assert_eq!(controls.pilot.throttle, None, "the throttle is left alone");
            assert_eq!(controls.pilot.yaw, 0.);
        }
        assert_eq!(pilot.bursts, 0);
        assert_eq!(pilot.pursuing, None);
        // A passive pilot ignores an enemy right ahead of it.
        let picture = RenderSnapshot {
            targets: vec![pose(3, [0., 0., 2000.], [0.; 3])],
            ..RenderSnapshot::default()
        };
        pilot.passive = true;
        let controls = pilot.controls(Duration::ZERO, &flight, Some(&picture), &|_| true);
        assert!(!controls.trigger && pilot.pursuing.is_none());
    }

    /// A flight state: the synthetic profile's aircraft at 20,000 feet, level.
    fn test_flight() -> flight::State {
        flight::State::new(&tore_world::test_support::profile(), [0., 20_000., 0.]).unwrap()
    }

    /// What a fight of bots came to.
    #[derive(Debug)]
    struct Fight {
        /// Seconds flown when the mission ended, and why.
        ended: Option<(f64, String)>,
        /// Bursts each bot began.
        bursts: Vec<u64>,
        /// The last scores a bot saw.
        scores: String,
    }

    /// One fight on the simulator.
    struct Setup {
        resources: Arc<BTreeMap<String, Vec<u8>>>,
        spec: MissionSpec,
        /// The plane each bot takes: one bot, or two.
        planes: Vec<u32>,
        /// The bots from the second on fly the cruise cycle only.
        passive: bool,
        seconds: u64,
        seed: u64,
        link: LinkConfig,
        /// Each bot starts to join this long after the host starts: the
        /// mission flies from the start, so a late bot takes over a plane
        /// the AI has been flying.
        delays: Vec<Duration>,
        /// Print the bots' ranges and banks as they close.
        trace: bool,
    }

    impl Setup {
        fn new(resources: BTreeMap<String, Vec<u8>>, spec: MissionSpec, planes: &[u32]) -> Self {
            Self {
                resources: Arc::new(resources),
                spec,
                planes: planes.to_vec(),
                passive: false,
                seconds: 60,
                seed: 1,
                link: LinkConfig::PERFECT,
                delays: Vec::new(),
                trace: false,
            }
        }

        /// A link with `ms` milliseconds each way, half as much spread and
        /// 2 percent loss.
        fn slow(mut self, ms: u64) -> Self {
            self.link = LinkConfig {
                latency: Duration::from_millis(ms),
                spread: Duration::from_millis(ms / 2),
                loss: 0.02,
                ..LinkConfig::PERFECT
            };
            self
        }
    }

    /// The bots fly `setup`'s mission as PvP with a kill limit of one, stepped
    /// a millisecond at a time, until it ends or `seconds` of flying pass.
    fn fight(setup: Setup) -> Fight {
        let trace = setup.trace;
        let net = SimNetwork::new(setup.seed);
        net.set_default_link(setup.link);
        let address = "10.0.0.1:26900".parse().unwrap();
        let mut host_socket = net.bind(address).unwrap();
        let mut host = Host::new(
            setup.spec,
            Arc::clone(&setup.resources),
            HostConfig {
                open_planes: OpenPlanes::All,
                start: StartMode::Now,
                entropy: Entropy::Seeded(setup.seed + 11),
                settings: vec![
                    (number::MODE, Mode::Pvp.value()),
                    (number::KILL_LIMIT, 1),
                    (number::KILL_OWNER, KillOwner::Total.value()),
                ],
                ..HostConfig::new(build())
            },
        )
        .expect("the host starts");
        let mut players: Vec<_> = setup
            .planes
            .iter()
            .enumerate()
            .map(|(i, &plane)| {
                let socket = net
                    .bind(format!("10.0.1.{}:40000", i + 1).parse().unwrap())
                    .unwrap();
                let config = ClientConfig {
                    entropy: Entropy::Seeded(setup.seed * 100 + i as u64),
                    plane: Some(plane),
                    ..ClientConfig::new(address, &format!("Bot{}", i + 1), build())
                };
                let client =
                    Client::connect(config, Arc::clone(&setup.resources), net.now()).unwrap();
                let mut bot = Bot::new(client);
                bot.pilot.passive = setup.passive && i > 0;
                (socket, bot)
            })
            .collect();
        let mut seated_at: Option<Duration> = None;
        let mut scores = String::new();
        let mut ended = None;
        let mut after_end = 0;
        loop {
            net.advance(Duration::from_millis(1));
            let now = net.now();
            host.receive_from(now, &mut host_socket).unwrap();
            host.update(now);
            host.transmit(&mut host_socket).unwrap();
            for (i, (socket, bot)) in players.iter_mut().enumerate() {
                if setup.delays.get(i).is_some_and(|&delay| now < delay) {
                    continue;
                }
                bot.client.receive_from(now, socket).unwrap();
                bot.update(now);
                bot.client.transmit(socket).unwrap();
                while bot.client.poll_event().is_some() {}
            }
            if seated_at.is_none() && players[0].1.client.phase() == ClientPhase::Flying {
                seated_at = Some(now);
            }
            let flown = seated_at.map_or(Duration::ZERO, |at| now.saturating_sub(at));
            if trace && now.as_millis().is_multiple_of(250) {
                let bot = &players[0].1;
                if let (Some(p), Some(pursuit)) = (bot.client.prediction(), bot.pilot.pursuing) {
                    let f = &p.plane().flight;
                    println!(
                        "{:6.1} s: range {:5.0} miss {:5.0} ft bank {:4.0} pitch {:3.0} g {:3.1} speed {:4.0} y {:6.0}",
                        flown.as_secs_f64(),
                        pursuit.range,
                        pursuit.miss,
                        f.bank.to_degrees(),
                        f.pitch.to_degrees(),
                        f.g,
                        f.speed,
                        f.position[1],
                    );
                }
            }
            while let Some(log) = host.poll_log() {
                if let HostLog::MissionEnded { reason, .. } = log {
                    ended = Some((flown.as_secs_f64(), format!("{reason:?}")));
                }
            }
            if ended.is_some() {
                // Let the final scores arrive.
                after_end += 1;
                if after_end > 300 {
                    break;
                }
            } else if flown >= Duration::from_secs(setup.seconds) {
                break;
            }
            assert!(
                seated_at.is_some() || now < Duration::from_secs(60),
                "the bots were not seated in time"
            );
        }
        if let Some(text) = players[0].1.client.scores() {
            scores = crate::client::scores::summary(text);
        }
        Fight {
            ended,
            bursts: players.iter().map(|(_, bot)| bot.pilot.bursts).collect(),
            scores,
        }
    }

    /// One F/A-18D (plane 0) and one enemy aircraft (plane 1), no AI to take
    /// a kill or a life, `nm` nautical miles apart at `altitude` feet. The
    /// enemy is a dummy, which flies straight, level and at 400 knots.
    fn duel(nm: u32, altitude: u32, enemy: &str, skill: &str) -> MissionSpec {
        MissionSpec::from_text(&format!(
            "tore-mission 1\ntheater UKR\nstart airborne {altitude}\nseparation-nm {nm}\n\
             wing friendly 1 F18.PT 1 average\nwing enemy 1 {enemy} 1 {skill}\n"
        ))
        .expect("the duel parses")
    }

    /// The dedicated server guide's example mission, the enemy `nm` away.
    fn guide_mission(nm: u32) -> MissionSpec {
        let guide = include_str!("../../../docs/DEDICATED-SERVER.md");
        let start = guide.find("tore-mission 1").expect("the guide's mission");
        let text = &guide[start..];
        let text = &text[..text.find("```").expect("the mission's end")];
        let text = text.replace("separation-nm 20", &format!("separation-nm {nm}"));
        let text = if std::env::var("T_HOLD").is_ok() {
            text.replace("preset free", "preset hold")
                .lines()
                .filter(|l| !l.starts_with("objective"))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            text
        };
        MissionSpec::from_text(&text).expect("the guide's mission parses")
    }

    /// Two bots in one friendly wing over the simulator, the lead giving an
    /// order and the wingman a reply (slice F2-R): each hears the other's call,
    /// as the game's radio line, through the host and the wire.
    fn wing_exchange(order_at: u64, reply_at: u64, lead_replies: bool) -> (Bot, Bot) {
        use tore_sim::ai::wing::{PlayerBreak, PlayerOrder};
        use tore_world::world::replies::Reply;
        let resources = Arc::new(tore_world::test_support::resources::resources());
        let spec = MissionSpec::from_text(
            "tore-mission 1\ntheater UKR\nstart airborne 20000\nseparation-nm 50\n\
             wing friendly 1 F18.PT 3 average\nwing enemy 1 F18.PT 1 dummy\n",
        )
        .expect("the wing parses");
        let net = SimNetwork::new(5);
        net.set_default_link(LinkConfig::PERFECT);
        let address = "10.0.0.1:26900".parse().unwrap();
        let mut host_socket = net.bind(address).unwrap();
        let mut host = Host::new(
            spec,
            Arc::clone(&resources),
            HostConfig {
                open_planes: OpenPlanes::All,
                start: StartMode::Now,
                entropy: Entropy::Seeded(16),
                ..HostConfig::new(build())
            },
        )
        .expect("the host starts");
        let mut players: Vec<_> = [0u32, 1]
            .into_iter()
            .map(|plane| {
                let socket = net
                    .bind(format!("10.0.1.{}:40000", plane + 1).parse().unwrap())
                    .unwrap();
                let config = ClientConfig {
                    entropy: Entropy::Seeded(500 + u64::from(plane)),
                    plane: Some(plane),
                    ..ClientConfig::new(address, &format!("Bot{}", plane + 1), build())
                };
                let client = Client::connect(config, Arc::clone(&resources), net.now()).unwrap();
                (socket, Bot::new(client))
            })
            .collect();
        let say = |bot: &mut Bot, at: u64, command: SeatCommand| {
            bot.send_at(Duration::from_secs(at), command);
        };
        say(
            &mut players[0].1,
            order_at,
            SeatCommand::WingOrder(PlayerOrder::Break(PlayerBreak::Left)),
        );
        let replier = usize::from(!lead_replies);
        say(
            &mut players[replier].1,
            reply_at,
            SeatCommand::WingReply(Reply::Winchester),
        );
        for _ in 0..30_000 {
            net.advance(Duration::from_millis(1));
            let now = net.now();
            host.receive_from(now, &mut host_socket).unwrap();
            host.update(now);
            host.transmit(&mut host_socket).unwrap();
            for (socket, bot) in &mut players {
                bot.client.receive_from(now, socket).unwrap();
                bot.update(now);
                bot.client.transmit(socket).unwrap();
                while bot.client.poll_event().is_some() {}
            }
        }
        let wingman = players.pop().unwrap().1;
        let lead = players.pop().unwrap().1;
        (lead, wingman)
    }

    /// A lead bot sorts its flight (slice G7): the wingman bot, a human, is
    /// given a bandit by data link. Both bots of the flight hear the
    /// assignment as a Link event, and the wingman's readout carries it.
    #[test]
    fn a_wingman_bot_is_given_a_bandit_by_its_leads_sort_over_the_wire() {
        use tore_sim::ai::wing::PlayerOrder;
        let resources = Arc::new(tore_world::test_support::resources::resources());
        let spec = MissionSpec::from_text(
            "tore-mission 1\ntheater UKR\nstart airborne 20000\nseparation-nm 10\n\
             wing friendly 1 F18.PT 3 average\nwing enemy 1 F18.PT 2 dummy\n",
        )
        .expect("the wings parse");
        let net = SimNetwork::new(5);
        net.set_default_link(LinkConfig::PERFECT);
        let address = "10.0.0.1:26900".parse().unwrap();
        let mut host_socket = net.bind(address).unwrap();
        let mut host = Host::new(
            spec,
            Arc::clone(&resources),
            HostConfig {
                open_planes: OpenPlanes::All,
                start: StartMode::Now,
                entropy: Entropy::Seeded(17),
                ..HostConfig::new(build())
            },
        )
        .expect("the host starts");
        let mut players: Vec<_> = [0u32, 1]
            .into_iter()
            .map(|plane| {
                let socket = net
                    .bind(format!("10.0.1.{}:40000", plane + 1).parse().unwrap())
                    .unwrap();
                let config = ClientConfig {
                    entropy: Entropy::Seeded(600 + u64::from(plane)),
                    plane: Some(plane),
                    ..ClientConfig::new(address, &format!("Bot{}", plane + 1), build())
                };
                let client = Client::connect(config, Arc::clone(&resources), net.now()).unwrap();
                (socket, Bot::new(client))
            })
            .collect();
        players[0].1.send_at(
            Duration::from_secs(6),
            SeatCommand::WingOrder(PlayerOrder::Sort),
        );
        let mut readout_assigned = None;
        for _ in 0..20_000 {
            net.advance(Duration::from_millis(1));
            let now = net.now();
            host.receive_from(now, &mut host_socket).unwrap();
            host.update(now);
            host.transmit(&mut host_socket).unwrap();
            for (index, (socket, bot)) in players.iter_mut().enumerate() {
                bot.client.receive_from(now, socket).unwrap();
                if let Some(frame) = bot.update(now)
                    && index == 1
                    && let Some(assigned) = frame.readout.and_then(|r| r.link.assigned)
                {
                    readout_assigned = Some(assigned);
                }
                bot.client.transmit(socket).unwrap();
                while bot.client.poll_event().is_some() {}
            }
        }
        let (lead, wingman) = (&players[0].1, &players[1].1);
        let given = |bot: &Bot| {
            bot.link_heard
                .iter()
                .find(|line| line.starts_with("plane 0 assigned plane 1 bandit "))
                .cloned()
        };
        let line = given(wingman).unwrap_or_else(|| {
            panic!(
                "the wingman's link: {:?}; the lead's lines: {:?}",
                wingman.link_heard, lead.lines_read
            )
        });
        assert!(line.ends_with(" (Sort)"), "{line}");
        assert_eq!(
            given(lead),
            Some(line.clone()),
            "the lead hears its flight's change"
        );
        let assigned = readout_assigned.expect("the wingman's readout holds the assignment");
        assert_eq!(assigned.by, 0);
        assert_eq!(
            line,
            format!("plane 0 assigned plane 1 bandit {} (Sort)", assigned.target)
        );
        assert!(
            wingman
                .link_heard
                .iter()
                .any(|l| l.starts_with(&format!("assigned: bandit {} by plane 0", assigned.target))),
            "{:?}",
            wingman.link_heard
        );
        // Every Link event a bot heard was about its own flight (planes 0 to
        // 2), never the enemy's.
        for bot in [lead, wingman] {
            for line in bot.link_heard.iter().filter(|l| l.starts_with("plane ")) {
                let plane: u32 = line["plane ".len()..]
                    .split(' ')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap();
                assert!(plane <= 2, "{line}");
            }
        }
    }

    #[test]
    fn a_lead_bots_order_and_a_wingman_bots_reply_reach_each_other_over_the_wire() {
        let (lead, wingman) = wing_exchange(4, 12, false);
        // The wingman heard the order as the lead's call, the lead the reply
        // under the wingman's place; each hears its own reply or order never
        // as another's.
        assert!(
            wingman
                .radio_heard
                .iter()
                .any(|line| line.starts_with("Red one: '")),
            "the wingman's radio: {:?}",
            wingman.radio_heard
        );
        assert!(
            lead.radio_heard
                .contains(&"Red two: 'Winchester'".to_owned()),
            "the lead's radio: {:?}",
            lead.radio_heard
        );
        assert!(
            wingman
                .radio_heard
                .contains(&"YOU: 'Winchester'".to_owned()),
            "the wingman hears its own call: {:?}",
            wingman.radio_heard
        );
        assert!(!lead.lines_read.iter().any(|l| l == "You lead this flight."));
    }

    #[test]
    fn a_lead_bots_reply_is_refused_with_a_line_and_no_call() {
        let (lead, wingman) = wing_exchange(4, 12, true);
        assert!(lead.lines_read.iter().any(|l| l == "You lead this flight."));
        assert!(
            !lead.radio_heard.iter().any(|l| l.contains("Winchester"))
                && !wingman.radio_heard.iter().any(|l| l.contains("Winchester")),
            "nobody hears a lead's reply: {:?} {:?}",
            lead.radio_heard,
            wingman.radio_heard
        );
    }

    fn killed_by(fight: &Fight, limit: f64) {
        let (at, reason) = fight.ended.as_ref().unwrap_or_else(|| {
            panic!("the mission did not end: {fight:?}");
        });
        assert_eq!(reason, "KillLimit", "{fight:?}");
        assert!(*at < limit, "ended at {at} s: {fight:?}");
        assert!(fight.bursts[0] >= 1, "{fight:?}");
    }

    #[test]
    fn a_bot_shoots_down_a_dummy_five_miles_off_within_a_minute() {
        // The synthetic import's own F/A-18D gun and enemy.
        for ms in [0, 40] {
            let setup = Setup::new(
                tore_world::test_support::resources::resources(),
                duel(5, 20_000, "F18.PT", "dummy"),
                &[0],
            );
            let fight = fight(if ms == 0 { setup } else { setup.slow(ms) });
            killed_by(&fight, 60.);
        }
    }

    #[test]
    fn a_bot_shoots_down_a_second_bot_that_only_cruises_and_ends_a_pvp_mission() {
        // Two players, one on each side; the second flies its cruise cycle
        // and fires at nothing, a target that does not evade. They are two
        // miles apart, so that the pass comes in the cycle's first straight
        // leg (it turns after ten seconds).
        for ms in [0, 40] {
            let setup = Setup::new(
                tore_world::test_support::resources::resources(),
                duel(2, 20_000, "F18.PT", "average"),
                &[0, 1],
            );
            let setup = Setup {
                passive: true,
                ..setup
            };

            let fight = fight(if ms == 0 { setup } else { setup.slow(ms) });
            killed_by(&fight, 60.);
            assert!(fight.scores.contains("ends at 1 kill in all"), "{fight:?}");
        }
    }

    /// The real import's guns: a bot shoots down a dummy MiG-29 at each
    /// separation the guide allows from 2 to 10 miles, at several altitudes
    /// and on a clean link and a slow one, each within a minute. Run by the
    /// full suite: `TORE_DATA_DIR=<an import> cargo test --locked -p
    /// tore-session --lib bot::tests::real_dummies -- --ignored`.
    #[test]
    #[ignore = "reads a real import through TORE_DATA_DIR; named for the full run"]
    fn real_dummies_fall_to_a_bot_at_every_separation_and_height() {
        let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
        let resources = tore_import::load(&directory).expect("an imported pack");
        let mut missed = Vec::new();
        for altitude in [5_000, 20_000, 40_000] {
            for nm in [2, 5, 10] {
                for ms in [0, 40] {
                    let setup = Setup::new(
                        resources.clone(),
                        duel(nm, altitude, "MIG29.PT", "dummy"),
                        &[0],
                    );
                    let fight = fight(if ms == 0 { setup } else { setup.slow(ms) });
                    let limit = 60. + f64::from(nm) * 4.;
                    if !fight
                        .ended
                        .as_ref()
                        .is_some_and(|(at, why)| why == "KillLimit" && *at < limit)
                    {
                        missed.push(format!("{altitude} ft, {nm} nm, {ms} ms: {fight:?}"));
                    }
                }
            }
        }
        // One miss in eighteen is the allowance (the separation of 2 miles
        // leaves one pass only).
        assert!(missed.len() <= 1, "{missed:#?}");
    }

    /// The guide's mission as net-server-pvp flies it, two bots, the AI of
    /// both sides about: the kill limit ends it within a minute.
    #[test]
    #[ignore = "reads a real import through TORE_DATA_DIR; study"]
    fn real_fight() {
        let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
        let resources = tore_import::load(&directory).expect("an imported pack");
        for (ms, late) in [
            (0, 0),
            (20, 0),
            (40, 1),
            (80, 0),
            (120, 2),
            (0, 3),
            (30, 0),
            (60, 1),
            (5, 0),
            (10, 2),
        ] {
            let setup = Setup {
                seconds: 75,
                delays: vec![Duration::ZERO, Duration::from_secs(late)],
                seed: ms + 1,
                ..Setup::new(resources.clone(), guide_mission(5), &[0, 1])
            };
            let setup = if ms == 0 { setup } else { setup.slow(ms) };
            let fight = fight(setup);
            println!(
                "fight {ms} ms late {late}: {:?} {}",
                fight.bursts, fight.scores
            );
        }
    }

    #[test]
    #[ignore = "reads a real import through TORE_DATA_DIR; study"]
    fn real_furball() {
        let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
        let resources = tore_import::load(&directory).expect("an imported pack");
        let spec = MissionSpec::from_text(
            "tore-mission 1\ntheater UKR\nstart airborne 20000\nseparation-nm 5\n\
             wing friendly 1 F18.PT 1 average\nwing enemy 1 MIG29.PT 1 average\n\
             cheats damage=invulnerable\n",
        )
        .unwrap();
        let setup = Setup {
            seconds: 100,
            trace: true,
            ..Setup::new(resources, spec, &[0, 1])
        };
        println!("{:?}", fight(setup));
    }

    #[test]
    #[ignore = "reads a real import through TORE_DATA_DIR; named for the full run"]
    fn real_guide_mission_pvp_ends_by_the_kill_limit() {
        let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
        let resources = tore_import::load(&directory).expect("an imported pack");
        for (ms, late) in [
            (0, 0),
            (20, 0),
            (40, 0),
            (80, 0),
            (120, 0),
            (0, 1),
            (0, 2),
            (0, 3),
            (0, 5),
            (40, 4),
        ] {
            let setup = Setup::new(resources.clone(), guide_mission(5), &[0, 6]);
            let setup = Setup {
                delays: vec![Duration::ZERO, Duration::from_secs(late)],
                ..setup
            };
            let fight = fight(if ms == 0 { setup } else { setup.slow(ms) });
            println!("late {late} s, {ms} ms: {fight:?}");
            killed_by(&fight, 60.);
        }
    }
}
