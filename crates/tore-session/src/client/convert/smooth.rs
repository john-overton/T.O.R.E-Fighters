//! The curves a conversion draws: every received state of an entity and the
//! own plane's predicted ticks, turned into one state per host tick.

use super::MAX_BRIDGE_TICKS;
use crate::client::interpolation::{self, Sample, hermite, per_tick};
use crate::client::prediction::Trace;
use crate::client::seen::OwnSample;
use crate::wire::entity::{EntityKey, EntityState};
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};
use tore_sim::attitude::Basis;

/// One entity's received states and a cursor along them. Ticks must be asked
/// for in increasing order.
pub(crate) struct Track<'a> {
    pub key: EntityKey,
    states: &'a [(u32, EntityState)],
    idx: usize,
}

impl<'a> Track<'a> {
    pub fn new(key: EntityKey, states: &'a [(u32, EntityState)]) -> Self {
        Self {
            key,
            states,
            idx: 0,
        }
    }

    /// The tick of the first received state.
    pub fn first(&self) -> u32 {
        self.states.first().map_or(0, |(tick, _)| *tick)
    }

    /// The entity at `tick`: exactly the state received for it there, or the
    /// curve between the two states around it. Nothing before its first
    /// state, after its last, or across a silence longer than
    /// [`MAX_BRIDGE_TICKS`]: the replay never guesses ahead.
    pub fn at(&mut self, tick: u64) -> Option<Sample> {
        let t = u32::try_from(tick).ok()?;
        while self.idx + 1 < self.states.len() && self.states[self.idx + 1].0 <= t {
            self.idx += 1;
        }
        let (t0, s0) = *self.states.get(self.idx)?;
        if t < t0 {
            return None;
        }
        if t == t0 {
            return Some(interpolation::beyond(&s0, 0.));
        }
        let (t1, s1) = *self.states.get(self.idx + 1)?;
        let h = t1 - t0;
        if h > MAX_BRIDGE_TICKS {
            return None;
        }
        let s = f64::from(t - t0) / f64::from(h);
        Some(interpolation::between(&s0, &s1, f64::from(h), s))
    }
}

/// How far the host's state was from the prediction at an exact state.
#[derive(Clone, Copy, Debug)]
struct Delta {
    position: [f64; 3],
    attitude: [f64; 3],
}

fn wrap(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// The own plane's states by tick, after the corrections are spread back.
pub(crate) struct Own {
    line: BTreeMap<u64, OwnSample>,
}

/// The newest tick the trace holds a state for.
pub(crate) fn own_last_tick(trace: &[Trace]) -> Option<u64> {
    trace
        .iter()
        .map(|t| match t {
            Trace::Stepped(s) | Trace::Host { sample: s, .. } => s.tick,
        })
        .max()
}

impl Own {
    /// Builds the line from the predictor's trace. Each tick holds the last
    /// state written for it, so a correction's re-stepped ticks replace the
    /// predictions they corrected; the host's own state is exact at its
    /// tick. Where the prediction ended up away from the next exact state,
    /// the difference is spread over the span before it, growing linearly
    /// from nothing at the earlier exact state to all of it at the later.
    pub fn new(trace: &[Trace]) -> Self {
        let mut line: BTreeMap<u64, OwnSample> = BTreeMap::new();
        let mut anchor: Option<u64> = None;
        let mut spreads: Vec<(u64, u64, Delta)> = Vec::new();
        for step in trace {
            match step {
                Trace::Stepped(sample) => {
                    line.insert(sample.tick, sample.clone());
                }
                Trace::Host { sample, differs } => {
                    let tick = sample.tick;
                    if *differs
                        && let (Some(before), Some(from)) = (line.get(&tick), anchor)
                        && from < tick
                    {
                        spreads.push((
                            from,
                            tick,
                            Delta {
                                position: std::array::from_fn(|i| {
                                    sample.pose.position[i] - before.pose.position[i]
                                }),
                                attitude: std::array::from_fn(|i| {
                                    wrap(sample.pose.attitude[i] - before.pose.attitude[i])
                                }),
                            },
                        ));
                    }
                    line.insert(tick, sample.clone());
                    anchor = Some(anchor.map_or(tick, |a| a.max(tick)));
                }
            }
        }
        for (from, to, delta) in spreads {
            for (tick, sample) in line.range_mut(from + 1..to) {
                let share = (*tick - from) as f64 / (to - from) as f64;
                for i in 0..3 {
                    sample.pose.position[i] += delta.position[i] * share;
                    sample.pose.attitude[i] =
                        wrap(sample.pose.attitude[i] + delta.attitude[i] * share);
                }
            }
        }
        Self { line }
    }

    /// The plane at `tick`: the state kept for it, or a curve between the
    /// states either side when none was.
    pub fn at(&self, tick: u64) -> Option<OwnSample> {
        if let Some(sample) = self.line.get(&tick) {
            return Some(sample.clone());
        }
        let (t0, a) = self.line.range(..tick).next_back()?;
        let (t1, b) = self.line.range(tick + 1..).next()?;
        let h = (t1 - t0) as f64;
        let s = (tick - t0) as f64 / h;
        let mut out = a.clone();
        out.tick = tick;
        out.pose.position = hermite(
            a.pose.position,
            per_tick(a.pose.velocity),
            b.pose.position,
            per_tick(b.pose.velocity),
            h,
            s,
        );
        out.pose.attitude = Basis::new(a.pose.attitude[0], a.pose.attitude[1], a.pose.attitude[2])
            .blended(
                Basis::new(b.pose.attitude[0], b.pose.attitude[1], b.pose.attitude[2]),
                s,
            )
            .angles();
        for i in 0..3 {
            out.pose.velocity[i] =
                a.pose.velocity[i] + (b.pose.velocity[i] - a.pose.velocity[i]) * s;
        }
        Some(out)
    }
}
