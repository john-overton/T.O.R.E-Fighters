//! Flying the player's own plane ahead of the host (docs/ARCHITECTURE.md,
//! "The client session" and "One step for a human's plane").
//!
//! Every client tick runs the plane's shared step ([`OwnPlane::step`]) with
//! the pilot's quantized input, the latest ownship terms, the weather
//! clock's reading at that tick and the ground objects standing. The
//! predictor keeps two seconds of inputs, commands and own state hashes, so a
//! snapshot's hash at tick N is compared with its own, and an exact state
//! from the host restarts the plane at its tick and steps the stored inputs
//! again to now. The drawn plane then slides from where it was to where the
//! correction put it ([`Offset`]).

use super::seen::{OwnSample, own_sample};
use crate::wire::inputs::{Command, InputFrame};
use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;
use tore_sim::attitude::Vector;
use tore_sim::combat::live::Configuration;
use tore_sim::flight;
use tore_world::WorldResult;
use tore_world::seats::SeatId;
use tore_world::terrain::Terrain;
use tore_world::world::plane::{ExactState, OwnPlane, OwnshipTerms, PlaneTick, WeatherReading};

/// Ticks of inputs, commands and hashes kept (2 seconds).
pub const HISTORY_TICKS: usize = 240;
/// The drawn correction's time constant, seconds (95 percent within 150 ms).
pub const BLEND_SECONDS: f64 = 0.05;
/// A correction over this many feet, or [`SNAP_DEGREES`], is not blended.
pub const SNAP_FEET: f64 = 100.;
pub const SNAP_DEGREES: f64 = 20.;
/// A correction under this many feet and [`SMALL_DEGREES`] is not shown at
/// all (last-digit differences).
pub const SMALL_FEET: f64 = 0.01;
pub const SMALL_DEGREES: f64 = 0.01;

/// The weather clock's reading after it stepped for host tick `tick`, for a
/// mission starting at `start_seconds` of the day: the clock steps once per
/// tick from tick 0, 256 native ticks a second, as `World::step` steps it.
pub fn weather_at(start_seconds: i32, tick: u64) -> WeatherReading {
    let native = (256 * (i128::from(tick) + 1) / 120) as i64;
    let elapsed = native / tore_sim::environment::TICKS_PER_SECOND;
    WeatherReading {
        ticks: native,
        seconds_of_day: ((i64::from(start_seconds) + elapsed)
            .rem_euclid(tore_sim::environment::SECONDS_PER_DAY)) as i32,
    }
}

/// One predicted tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub tick: u64,
    pub frame: InputFrame,
    /// The commands applied at the tick, in number order.
    pub commands: Vec<Command>,
    /// The own state hash after the tick, at snapshot ticks.
    pub hash: Option<u64>,
}

/// One thing the predictor did, kept for the capture's conversion into a
/// replay ([`Predictor::trace_on`]).
#[derive(Clone, Debug, PartialEq)]
pub enum Trace {
    /// A tick was stepped (or stepped again after a correction): the plane
    /// after it.
    Stepped(OwnSample),
    /// The host's exact state at its tick: `differs` when the prediction had
    /// it somewhere else (the plane was restarted from it) and not when the
    /// prediction already equalled it.
    Host { sample: OwnSample, differs: bool },
}

/// What an exact state from the host did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Restored {
    /// It equals the prediction at its tick: nothing changed.
    Same,
    /// The plane restarted from it and stepped again to now; the drawn plane
    /// at now moved by this much.
    Corrected { feet: f64, degrees: f64 },
    /// It is for a tick the client has not reached: the plane is now that
    /// state at its tick.
    Adopted,
    /// It is older than the inputs the client keeps: ignored.
    Stale,
}

/// The player's plane, predicted.
#[derive(Clone, Debug)]
pub struct Predictor {
    seat: SeatId,
    plane: OwnPlane,
    /// The plane's state is the one after this host tick.
    tick: u64,
    terms: Option<OwnshipTerms>,
    config: Arc<Configuration>,
    ticks_per_snapshot: u64,
    /// The ticks of this seat's snapshots: the tick modulo
    /// `ticks_per_snapshot` is this.
    phase: u64,
    start_seconds: i32,
    /// The ground objects that can still be hit; the destroyed ones are
    /// taken out as the host says.
    standing: BTreeSet<u32>,
    history: VecDeque<Record>,
    /// What happened, when someone asked to keep it.
    trace: Option<Vec<Trace>>,
}

impl Predictor {
    /// The plane `state` at host tick `tick`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        seat: SeatId,
        plane: u32,
        tick: u64,
        state: ExactState,
        config: Arc<Configuration>,
        ticks_per_snapshot: u64,
        start_seconds: i32,
        standing: BTreeSet<u32>,
    ) -> Self {
        let (plane, terms) = state.into_own_plane(plane);
        Self {
            seat,
            plane,
            tick,
            terms,
            config,
            ticks_per_snapshot: ticks_per_snapshot.max(1),
            phase: crate::wire::snapshot_phase(seat.0, ticks_per_snapshot.clamp(1, 120) as u32),
            start_seconds,
            standing,
            history: VecDeque::new(),
            trace: None,
        }
    }

    /// Starts keeping a [`Trace`] of every step and every exact state, for
    /// [`Self::take_trace`].
    pub fn trace_on(&mut self) {
        self.trace.get_or_insert_with(Vec::new);
    }

    /// The trace kept since the last call, oldest first.
    pub fn take_trace(&mut self) -> Vec<Trace> {
        self.trace.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// The newest predicted tick.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// The plane as predicted.
    pub fn plane(&self) -> &OwnPlane {
        &self.plane
    }

    /// The ownship terms in use.
    pub fn terms(&self) -> Option<&OwnshipTerms> {
        self.terms.as_ref()
    }

    /// The plane's ownship configuration.
    pub fn config(&self) -> &Arc<Configuration> {
        &self.config
    }

    /// The stored ticks, oldest first.
    pub fn history(&self) -> &VecDeque<Record> {
        &self.history
    }

    /// A ground object was destroyed.
    pub fn destroyed(&mut self, object: u32) {
        self.standing.remove(&object);
    }

    /// The prediction's own state hash after `tick`, when it was a snapshot
    /// tick the client still remembers.
    pub fn hash_at(&self, tick: u64) -> Option<u64> {
        self.record(tick).and_then(|r| r.hash)
    }

    fn record(&self, tick: u64) -> Option<&Record> {
        let first = self.history.front()?.tick;
        self.history.get(tick.checked_sub(first)? as usize)
    }

    /// Steps the next tick with `frame` and `commands`.
    pub fn step(
        &mut self,
        frame: InputFrame,
        commands: Vec<Command>,
        terrain: &Terrain,
    ) -> WorldResult<()> {
        let tick = self.tick + 1;
        let hash = self.step_one(tick, &frame, &commands, terrain)?;
        self.history.push_back(Record {
            tick,
            frame,
            commands,
            hash,
        });
        while self.history.len() > HISTORY_TICKS {
            self.history.pop_front();
        }
        Ok(())
    }

    fn step_one(
        &mut self,
        tick: u64,
        frame: &InputFrame,
        commands: &[Command],
        terrain: &Terrain,
    ) -> WorldResult<Option<u64>> {
        let input = frame.seat_input(self.seat, tick, commands, None);
        let standing: Vec<u32> = self.standing.iter().copied().collect();
        let plane_tick = PlaneTick {
            sensors: input.sensors,
            pilot: &input.pilot,
            standing: &standing,
            weather: weather_at(self.start_seconds, tick),
            terms: self.terms.as_ref(),
            events: &[],
        };
        self.plane.step(&plane_tick, terrain, &self.config)?;
        // The host gives these to the seat as its HUD lines.
        self.plane.flight.systems.messages.clear();
        self.tick = tick;
        if self.trace.is_some() {
            let sample = own_sample(
                tick,
                self.plane.plane,
                &self.plane.flight,
                &self.config,
                self.terms.as_ref(),
                terrain,
                Some(&input.pilot),
            );
            self.trace
                .get_or_insert_with(Vec::new)
                .push(Trace::Stepped(sample));
        }
        Ok(if tick % self.ticks_per_snapshot == self.phase {
            Some(self.exact().hash()?)
        } else {
            None
        })
    }

    /// The exact state of the prediction now.
    pub fn exact(&self) -> ExactState {
        ExactState::of(&self.plane, self.terms.as_ref())
    }

    /// The host's exact `state` of the plane at `tick`: compared with the
    /// prediction there, and when it differs the plane restarts from it and
    /// steps the stored inputs again to now.
    pub fn restore(
        &mut self,
        tick: u64,
        state: ExactState,
        terrain: &Terrain,
    ) -> WorldResult<Restored> {
        let first = self.history.front().map(|r| r.tick);
        let host_sample = |me: &Self, differs: bool| {
            me.trace.is_some().then(|| Trace::Host {
                sample: own_sample(
                    tick,
                    me.plane.plane,
                    &state.flight,
                    &me.config,
                    state.terms.as_ref(),
                    terrain,
                    None,
                ),
                differs,
            })
        };
        if tick > self.tick {
            if let Some(host) = host_sample(self, true) {
                self.trace.get_or_insert_with(Vec::new).push(host);
            }
            // Not reached yet: the plane is the host's from there.
            let (plane, terms) = state.into_own_plane(self.plane.plane);
            self.plane = plane;
            self.terms = terms;
            self.tick = tick;
            self.history.clear();
            return Ok(Restored::Adopted);
        }
        if tick < self.tick && first.is_none_or(|f| tick + 1 < f) {
            // Too old to step again from; a newer one follows within a second.
            return Ok(Restored::Stale);
        }
        let hash = state.hash()?;
        let ours = if tick == self.tick {
            Some(self.exact().hash()?)
        } else {
            self.hash_at(tick)
        };
        if let Some(host) = host_sample(self, ours != Some(hash)) {
            self.trace.get_or_insert_with(Vec::new).push(host);
        }
        if ours == Some(hash) {
            return Ok(Restored::Same);
        }
        let before = pose(&self.plane.flight);
        let (plane, terms) = state.into_own_plane(self.plane.plane);
        let now = self.tick;
        self.plane = plane;
        self.terms = terms;
        self.tick = tick;
        let again: Vec<(u64, InputFrame, Vec<Command>)> = self
            .history
            .iter()
            .filter(|r| r.tick > tick && r.tick <= now)
            .map(|r| (r.tick, r.frame, r.commands.clone()))
            .collect();
        for (t, frame, commands) in again {
            let hash = self.step_one(t, &frame, &commands, terrain)?;
            if let Some(record) = first
                .and_then(|f| t.checked_sub(f))
                .and_then(|i| self.history.get_mut(i as usize))
            {
                record.hash = hash;
            }
        }
        // The host's state is the record's at its tick.
        if tick % self.ticks_per_snapshot == self.phase
            && let Some(record) = first
                .and_then(|f| tick.checked_sub(f))
                .and_then(|i| self.history.get_mut(i as usize))
        {
            record.hash = Some(hash);
        }
        let after = pose(&self.plane.flight);
        let (feet, degrees) = difference(&before, &after);
        Ok(Restored::Corrected { feet, degrees })
    }
}

/// Where a flight is drawn: position and yaw, pitch, bank.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub position: Vector,
    pub angles: [f64; 3],
}

/// The flight's drawn pose.
pub fn pose(flight: &flight::State) -> Pose {
    Pose {
        position: flight.position,
        angles: [flight.yaw, flight.pitch, flight.bank],
    }
}

fn wrap(angle: f64) -> f64 {
    (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

/// How far apart two poses are: feet, and the largest angle in degrees.
pub fn difference(a: &Pose, b: &Pose) -> (f64, f64) {
    let feet = (0..3)
        .map(|i| (a.position[i] - b.position[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    let degrees = (0..3)
        .map(|i| wrap(a.angles[i] - b.angles[i]).abs().to_degrees())
        .fold(0., f64::max);
    (feet, degrees)
}

/// The drawn plane's offset from the predicted one after a correction,
/// fading with a 50 ms time constant.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Offset {
    position: Vector,
    angles: [f64; 3],
    /// Seconds of the client's time when it was last set.
    at: f64,
}

impl Offset {
    /// What is left of the offset at `now` seconds.
    pub fn at(&self, now: f64) -> Offset {
        let fade = (-(now - self.at).max(0.) / BLEND_SECONDS).exp();
        Offset {
            position: self.position.map(|v| v * fade),
            angles: self.angles.map(|v| v * fade),
            at: now,
        }
    }

    /// A correction moved the predicted plane from `before` to `after` at
    /// `now` seconds: the drawn plane keeps where it was and slides. One
    /// over 100 ft or 20 degrees snaps, and one under 0.01 ft and 0.01
    /// degrees is not shown. Returns whether it is blended.
    pub fn correct(&mut self, before: &Pose, after: &Pose, now: f64) -> bool {
        let (feet, degrees) = difference(before, after);
        let left = self.at(now);
        if feet > SNAP_FEET || degrees > SNAP_DEGREES {
            *self = Offset::default();
            self.at = now;
            return false;
        }
        if feet < SMALL_FEET && degrees < SMALL_DEGREES {
            *self = left;
            return false;
        }
        *self = Offset {
            position: std::array::from_fn(|i| {
                left.position[i] + before.position[i] - after.position[i]
            }),
            angles: std::array::from_fn(|i| {
                left.angles[i] + wrap(before.angles[i] - after.angles[i])
            }),
            at: now,
        };
        true
    }

    /// No offset (a snap, or seating).
    pub fn clear(&mut self, now: f64) {
        *self = Offset {
            at: now,
            ..Offset::default()
        };
    }

    /// `flight` moved by what is left of the offset at `now` seconds.
    pub fn apply(&self, flight: &mut flight::State, now: f64) {
        let left = self.at(now);
        for i in 0..3 {
            flight.position[i] += left.position[i];
        }
        flight.yaw = wrap(flight.yaw + left.angles[0]);
        flight.pitch += left.angles[1];
        flight.bank = wrap(flight.bank + left.angles[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offset_fades_to_five_percent_within_150_ms() {
        let mut offset = Offset::default();
        let before = Pose {
            position: [10., 0., 0.],
            angles: [0.; 3],
        };
        assert!(offset.correct(&before, &Pose::default(), 1.));
        let left = offset.at(1.15);
        assert!((left.position[0] - 10. * (-3f64).exp()).abs() < 1e-9);
        assert!(left.position[0] < 0.5);
    }

    #[test]
    fn big_corrections_snap_and_tiny_ones_do_not_show() {
        let mut offset = Offset::default();
        let far = Pose {
            position: [150., 0., 0.],
            angles: [0.; 3],
        };
        assert!(!offset.correct(&far, &Pose::default(), 0.));
        assert_eq!(offset.at(0.).position, [0.; 3]);
        let turned = Pose {
            position: [0.; 3],
            angles: [0.5, 0., 0.],
        };
        assert!(!offset.correct(&turned, &Pose::default(), 0.));
        let tiny = Pose {
            position: [0.001, 0., 0.],
            angles: [0.; 3],
        };
        assert!(!offset.correct(&tiny, &Pose::default(), 0.));
        assert_eq!(offset.at(0.).position, [0.; 3]);
    }
}
