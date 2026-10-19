//! Spec-derived incoming gunfire and anonymous-hit awareness.
//! See docs/spec/visual-awareness-under-fire.md. Shooter identity is deliberately
//! absent: recognizing danger cannot manufacture an offensive target.

use crate::attitude::{Vector, dot};

use super::defense::{self, DefenseOwn, MotionSuggestion};

pub const PREDICTION_SECONDS: f64 = 2.;
pub const THREAT_RADIUS_FT: f64 = 250.;
pub const PROXIMITY_RADIUS_FT: f64 = 100.;
pub const RETENTION_TICKS: u64 = 240;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Round {
    pub id: u32,
    /// Used only to exclude the receiver's own ordnance.
    pub owner: u32,
    pub position: Vector,
    pub previous: Vector,
    pub tracer: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Evidence {
    VisualTrajectory,
    ClosePass,
    Hit,
}

impl Evidence {
    pub fn label(self) -> &'static str {
        match self {
            Self::VisualTrajectory => "observed incoming tracer",
            Self::ClosePass => "round passed within 100 ft",
            Self::Hit => "weapon hit, shooter not identified",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cue {
    pub evidence: Evidence,
    pub round: Option<u32>,
    pub observed_tick: u64,
    pub time_to_danger_s: f64,
    /// Measured direction only, never a hidden shooter position.
    pub bearing_world_deg: Option<f64>,
}

impl Cue {
    pub fn remaining_time(self, tick: u64) -> f64 {
        (self.time_to_danger_s - tick.saturating_sub(self.observed_tick) as f64 / 120.).max(0.)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Trace {
    pub cue: Option<Cue>,
    pub motion: Option<MotionSuggestion>,
    pub selected: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Service {
    samples: Samples,
    cue: Option<Cue>,
    pending_hit: bool,
    episode: Option<(u64, f64, f64)>,
}

/// The last tick's visible tracer positions by round id, ascending: a map
/// kept as a sorted list, with a spare list the next tick fills, so no tick
/// allocates. It compares, prints and is coded exactly as the map
/// `BTreeMap<u32, Vector>` it replaces; the spare is scratch and is none of
/// those.
#[derive(Default)]
pub(crate) struct Samples {
    by_id: Vec<(u32, Vector)>,
    spare: Vec<(u32, Vector)>,
}

impl Samples {
    fn get(&self, id: u32) -> Option<&Vector> {
        self.by_id
            .binary_search_by_key(&id, |(key, _)| *key)
            .ok()
            .map(|at| &self.by_id[at].1)
    }
    /// From entries in ascending, unique id order.
    pub(crate) fn from_sorted(by_id: Vec<(u32, Vector)>) -> Self {
        debug_assert!(by_id.windows(2).all(|pair| pair[0].0 < pair[1].0));
        Self {
            by_id,
            spare: Vec::new(),
        }
    }
    /// Takes the spare list, emptied, to fill with this tick's samples.
    fn begin(&mut self) -> Vec<(u32, Vector)> {
        let mut next = std::mem::take(&mut self.spare);
        next.clear();
        next
    }
    /// Keeps `next` as the samples: sorted by id, and where an id repeats,
    /// its last entry, as inserting each into a map would leave it.
    fn finish(&mut self, mut next: Vec<(u32, Vector)>) {
        if !next.windows(2).all(|pair| pair[0].0 < pair[1].0) {
            // Stable: equal ids keep their insertion order.
            next.sort_by_key(|(id, _)| *id);
            let mut kept = 0;
            for at in 0..next.len() {
                if at + 1 < next.len() && next[at + 1].0 == next[at].0 {
                    continue;
                }
                next[kept] = next[at];
                kept += 1;
            }
            next.truncate(kept);
        }
        self.spare = std::mem::replace(&mut self.by_id, next);
    }
}

impl Clone for Samples {
    fn clone(&self) -> Self {
        Self {
            by_id: self.by_id.clone(),
            spare: Vec::new(),
        }
    }
}

impl PartialEq for Samples {
    fn eq(&self, other: &Self) -> bool {
        self.by_id == other.by_id
    }
}

impl std::fmt::Debug for Samples {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.by_id.iter().map(|(id, at)| (id, at)))
            .finish()
    }
}

/// The tick's gun rounds with an index by x, built once per tick for every
/// AI aircraft, so each visits only the rounds that can matter to it.
pub struct Rounds<'a> {
    rounds: &'a [Round],
    /// Each round with finite positions: its x and its index, by x.
    by_x: Vec<(f64, u32)>,
    /// The longest step any of those rounds took this tick, in feet.
    step: f64,
    /// The indices one aircraft visits, reused aircraft to aircraft.
    scratch: std::cell::RefCell<Vec<u32>>,
}

impl<'a> Rounds<'a> {
    pub fn new(rounds: &'a [Round]) -> Self {
        let mut step = 0_f64;
        let mut by_x: Vec<(f64, u32)> = rounds
            .iter()
            .enumerate()
            .filter(|(_, r)| r.position.iter().chain(&r.previous).all(|v| v.is_finite()))
            .map(|(index, r)| {
                let moved = sub(r.position, r.previous);
                step = step.max(dot(moved, moved).sqrt());
                (r.position[0], index as u32)
            })
            .collect();
        by_x.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        Self {
            rounds,
            by_x,
            step,
            scratch: Default::default(),
        }
    }

    pub fn all(&self) -> &'a [Round] {
        self.rounds
    }
}

/// Where a round must be to be seen at all: within `radius` feet of
/// `center` (the lookout's visual range around its eye).
#[derive(Clone, Copy, Debug)]
pub struct SightReach {
    pub center: Vector,
    pub radius: f64,
}

/// Slack on every reach test, in feet: far more than the rounding of any
/// distance here, so a round the exact tests could count is never left out.
const REACH_SLACK_FT: f64 = 16.;

impl Service {
    pub fn hit(&mut self) {
        self.pending_hit = true;
    }

    pub fn cue(&self) -> Option<Cue> {
        self.cue
    }

    /// Samples are retained only across consecutive visible ticks. An unseen
    /// projectile's truth velocity/target/launcher never enters prediction.
    pub fn observe(
        &mut self,
        tick: u64,
        receiver: u32,
        own: DefenseOwn,
        rounds: &[Round],
        visible: impl Fn(Vector) -> bool,
        terrain_clear: impl Fn(Vector) -> bool,
    ) {
        self.observe_rounds(
            tick,
            own,
            rounds.iter().filter(|r| r.owner != receiver),
            visible,
            terrain_clear,
        );
    }

    /// [`Self::observe`] over an indexed tick of rounds, visiting only the
    /// rounds that can count: within a close pass of the aircraft this tick,
    /// or within `sight`, outside which `visible` is never true. Every other
    /// round would raise no cue and leave no sample, so the outcome is the
    /// same as visiting them all, in the same order.
    #[allow(clippy::too_many_arguments)]
    pub fn observe_near(
        &mut self,
        tick: u64,
        receiver: u32,
        own: DefenseOwn,
        rounds: &Rounds<'_>,
        sight: SightReach,
        visible: impl Fn(Vector) -> bool,
        terrain_clear: impl Fn(Vector) -> bool,
    ) {
        let mut scratch = rounds.scratch.borrow_mut();
        // A round's closest swept approach is at least its distance now less
        // its step relative to the aircraft.
        let own_step = dot(own.velocity, own.velocity).sqrt() / 120.;
        let pass = PROXIMITY_RADIUS_FT + rounds.step + own_step + REACH_SLACK_FT;
        let seen = sight.radius + REACH_SLACK_FT;
        let near = |position: Vector| {
            let to_own = sub(position, own.position);
            let to_eye = sub(position, sight.center);
            dot(to_own, to_own) <= pass * pass || dot(to_eye, to_eye) <= seen * seen
        };
        scratch.clear();
        let reach = |center: f64, radius: f64| (center - radius, center + radius);
        let windows = [reach(own.position[0], pass), reach(sight.center[0], seen)];
        for (low, high) in windows {
            if low.partial_cmp(&high).is_none_or(|order| order.is_gt()) {
                // Not a number: no window can be trusted, visit every round.
                scratch.clear();
                scratch.extend(rounds.by_x.iter().map(|(_, index)| *index));
                break;
            }
            let first = rounds.by_x.partition_point(|(x, _)| *x < low);
            let last = rounds.by_x.partition_point(|(x, _)| *x <= high);
            scratch.extend(rounds.by_x[first..last].iter().map(|(_, index)| *index));
        }
        scratch.sort_unstable();
        scratch.dedup();
        let all = rounds.rounds;
        self.observe_rounds(
            tick,
            own,
            scratch
                .iter()
                .map(|index| &all[*index as usize])
                .filter(|r| r.owner != receiver && near(r.position)),
            visible,
            terrain_clear,
        );
    }

    fn observe_rounds<'r>(
        &mut self,
        tick: u64,
        own: DefenseOwn,
        rounds: impl Iterator<Item = &'r Round>,
        visible: impl Fn(Vector) -> bool,
        terrain_clear: impl Fn(Vector) -> bool,
    ) {
        if self
            .cue
            .is_some_and(|c| tick.saturating_sub(c.observed_tick) >= RETENTION_TICKS)
        {
            self.cue = None;
            self.episode = None;
        }
        let mut fresh = Vec::new();
        if std::mem::take(&mut self.pending_hit) {
            fresh.push(Cue {
                evidence: Evidence::Hit,
                round: None,
                observed_tick: tick,
                time_to_danger_s: 0.,
                bearing_world_deg: None,
            });
        }
        let mut samples = self.samples.begin();
        for round in rounds {
            self.consider(
                tick,
                own,
                round,
                &visible,
                &terrain_clear,
                &mut fresh,
                &mut samples,
            );
        }
        self.samples.finish(samples);
        // One episode survives a burst; new evidence does not reset the break.
        // Store only the evidence actually measured on this observation.
        if let Some(cue) = fresh.into_iter().min_by(|a, b| {
            a.time_to_danger_s
                .total_cmp(&b.time_to_danger_s)
                .then(a.round.cmp(&b.round))
        }) {
            self.cue = Some(cue);
        }
    }

    /// One round's evidence: a close pass, and for a visible tracer its
    /// sample and any predicted danger.
    #[allow(clippy::too_many_arguments)]
    fn consider(
        &self,
        tick: u64,
        own: DefenseOwn,
        round: &Round,
        visible: &impl Fn(Vector) -> bool,
        terrain_clear: &impl Fn(Vector) -> bool,
        fresh: &mut Vec<Cue>,
        samples: &mut Vec<(u32, Vector)>,
    ) {
        if !round
            .position
            .iter()
            .chain(&round.previous)
            .all(|v| v.is_finite())
        {
            return;
        }
        let delta = sub(round.position, own.position);
        // Worked out only for a round that raises a cue.
        let bearing = || delta[0].atan2(delta[2]).to_degrees().rem_euclid(360.);
        // Swept relative separation catches a round crossing between ticks.
        let start =
            std::array::from_fn(|i| round.previous[i] - own.position[i] + own.velocity[i] / 120.);
        let movement = sub(delta, start);
        let length = dot(movement, movement);
        let fraction = if length > 0. {
            (-dot(start, movement) / length).clamp(0., 1.)
        } else {
            0.
        };
        let nearest: Vector = std::array::from_fn(|i| start[i] + movement[i] * fraction);
        let point = std::array::from_fn(|i| own.position[i] + nearest[i]);
        if dot(nearest, nearest) <= PROXIMITY_RADIUS_FT.powi(2) && terrain_clear(point) {
            fresh.push(Cue {
                evidence: Evidence::ClosePass,
                round: Some(round.id),
                observed_tick: tick,
                time_to_danger_s: 0.,
                bearing_world_deg: Some(bearing()),
            });
        }
        if !round.tracer || !visible(round.position) {
            return;
        }
        samples.push((round.id, round.position));
        let Some(previous) = self.samples.get(round.id) else {
            return;
        };
        let velocity =
            std::array::from_fn(|i| (round.position[i] - previous[i]) * 120. - own.velocity[i]);
        if let Some((time, miss)) = closest_approach(delta, velocity)
            && time <= PREDICTION_SECONDS
            && miss <= THREAT_RADIUS_FT
        {
            fresh.push(Cue {
                evidence: Evidence::VisualTrajectory,
                round: Some(round.id),
                observed_tick: tick,
                time_to_danger_s: time,
                bearing_world_deg: Some(bearing()),
            });
        }
    }

    pub fn motion(
        &mut self,
        tick: u64,
        receiver: u32,
        own: DefenseOwn,
        terrain: impl FnMut(Vector) -> f64,
    ) -> Option<MotionSuggestion> {
        let cue = self.cue?;
        let (start, heading, side) = *self.episode.get_or_insert_with(|| {
            let side = cue
                .bearing_world_deg
                .map(|bearing| {
                    let relative = (bearing - own.heading_deg + 180.).rem_euclid(360.) - 180.;
                    if relative > 0. { -1. } else { 1. }
                })
                .unwrap_or(if receiver.is_multiple_of(2) { 1. } else { -1. });
            (tick, own.heading_deg, side)
        });
        let leg = (tick.saturating_sub(start) / defense::JINK_LEG_TICKS) % 2;
        let heading = (heading + side * if leg == 0 { 1. } else { -1. } * defense::JINK_OFFSET_DEG)
            .rem_euclid(360.);
        Some(defense::safe_jink(own, heading, terrain))
    }
}

fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}

pub fn closest_approach(delta: Vector, velocity: Vector) -> Option<(f64, f64)> {
    let vv = dot(velocity, velocity);
    if !vv.is_finite() || vv <= 0. {
        return None;
    }
    let time = -dot(delta, velocity) / vv;
    if !time.is_finite() || time < 0. {
        return None;
    }
    let closest: Vector = std::array::from_fn(|i| delta[i] + velocity[i] * time);
    Some((time, dot(closest, closest).sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own() -> DefenseOwn {
        DefenseOwn {
            position: [0., 5000., 0.],
            velocity: [0.; 3],
            heading_deg: 0.,
            flight_path_pitch_deg: 0.,
            speed_ft_s: 700.,
            bank_deg: 0.,
            usable_turn_rate_deg_s: 20.,
            usable_pitch_rate_deg_s: 15.,
            roll_in_time_s: 1.,
            dive_speed_safe: true,
        }
    }
    fn round(position: Vector) -> Round {
        Round {
            id: 7,
            owner: 99,
            position,
            previous: position,
            tracer: true,
        }
    }
    fn sample(service: &mut Service, tick: u64, p: Round, visible: bool, clear: bool) {
        service.observe(tick, 1, own(), &[p], |_| visible, |_| clear);
    }

    #[test]
    fn two_visible_samples_are_required_and_loss_breaks_the_track() {
        let mut s = Service::default();
        sample(&mut s, 0, round([0., 5000., 2010.]), true, true);
        assert!(s.cue().is_none());
        sample(&mut s, 1, round([0., 5000., 2000.]), false, true);
        sample(&mut s, 2, round([0., 5000., 1990.]), true, true);
        assert!(s.cue().is_none());
        sample(&mut s, 3, round([0., 5000., 1980.]), true, true);
        assert_eq!(s.cue().unwrap().evidence, Evidence::VisualTrajectory);
    }

    #[test]
    fn prediction_boundaries_and_harmless_crossing_are_distinct() {
        for (x, z, expected) in [
            (250., 2400., true),
            (250.01, 2400., false),
            (0., 2400.01, false),
            (800., 1000., false),
            (0., -1000., false),
        ] {
            let mut s = Service::default();
            sample(&mut s, 0, round([x, 5000., z + 10.]), true, true);
            sample(&mut s, 1, round([x, 5000., z]), true, true);
            assert_eq!(s.cue().is_some(), expected, "x={x} z={z}");
        }
    }

    #[test]
    fn close_pass_is_swept_anonymous_and_terrain_limited() {
        for (offset, clear, owner, expected) in [
            (100., true, 99, true),
            (100.01, true, 99, false),
            (50., false, 99, false),
            (50., true, 1, false),
        ] {
            let mut s = Service::default();
            let p = Round {
                previous: [offset, 5000., -500.],
                position: [offset, 5000., 500.],
                owner,
                tracer: false,
                ..round([0.; 3])
            };
            sample(&mut s, 0, p, false, clear);
            assert_eq!(s.cue().is_some(), expected);
            if expected {
                assert_eq!(s.cue().unwrap().evidence, Evidence::ClosePass);
            }
        }
    }

    #[test]
    fn hit_is_immediate_expires_exactly_and_needs_no_shooter() {
        let mut s = Service::default();
        s.hit();
        s.observe(10, 1, own(), &[], |_| false, |_| false);
        let cue = s.cue().unwrap();
        assert_eq!(cue.evidence, Evidence::Hit);
        assert_eq!(cue.round, None);
        assert!(s.motion(10, 1, own(), |_| 0.).is_some());
        s.observe(249, 1, own(), &[], |_| false, |_| false);
        assert!(s.cue().is_some());
        s.observe(250, 1, own(), &[], |_| false, |_| false);
        assert!(s.cue().is_none());
        assert!(s.motion(250, 1, own(), |_| 0.).is_none());
    }

    #[test]
    fn a_burst_does_not_restart_the_break_and_low_flight_does_not_dive() {
        let mut s = Service::default();
        s.hit();
        s.observe(0, 1, own(), &[], |_| false, |_| true);
        let start = s.motion(0, 1, own(), |_| 0.).unwrap();
        for tick in 1..=240 {
            s.hit();
            s.observe(tick, 1, own(), &[], |_| false, |_| true);
            let motion = s.motion(tick, 1, own(), |_| 0.).unwrap();
            if tick < 240 {
                assert_eq!(motion.heading_deg, start.heading_deg);
            } else {
                assert_eq!((motion.heading_deg - start.heading_deg).abs(), 270.);
            }
        }
        let low = DefenseOwn {
            position: [0., 500., 0.],
            ..own()
        };
        assert!(s.motion(241, 1, low, |_| 0.).unwrap().flight_path_pitch_deg >= 0.);
        let fast = DefenseOwn {
            dive_speed_safe: false,
            ..own()
        };
        assert!(
            s.motion(242, 1, fast, |_| 0.)
                .unwrap()
                .flight_path_pitch_deg
                >= 0.
        );
    }

    #[test]
    fn ground_trajectory_uses_the_same_observation_contract() {
        let mut s = Service::default();
        // A measured upward flight segment, with no aircraft or shooter identity.
        sample(&mut s, 0, round([0., 3990., 1010.]), true, true);
        sample(&mut s, 1, round([0., 4000., 1000.]), true, true);
        assert_eq!(s.cue().unwrap().evidence, Evidence::VisualTrajectory);
        assert!(s.motion(1, 1, own(), |_| 0.).is_some());
    }

    /// The original service: a map of samples rebuilt every tick and every
    /// round visited.
    #[derive(Default)]
    struct Reference {
        samples: std::collections::BTreeMap<u32, Vector>,
        cue: Option<Cue>,
        pending_hit: bool,
        episode: Option<(u64, f64, f64)>,
    }

    impl Reference {
        fn observe(
            &mut self,
            tick: u64,
            receiver: u32,
            own: DefenseOwn,
            rounds: &[Round],
            visible: impl Fn(Vector) -> bool,
            terrain_clear: impl Fn(Vector) -> bool,
        ) {
            if self
                .cue
                .is_some_and(|c| tick.saturating_sub(c.observed_tick) >= RETENTION_TICKS)
            {
                self.cue = None;
                self.episode = None;
            }
            let mut fresh = Vec::new();
            if std::mem::take(&mut self.pending_hit) {
                fresh.push(Cue {
                    evidence: Evidence::Hit,
                    round: None,
                    observed_tick: tick,
                    time_to_danger_s: 0.,
                    bearing_world_deg: None,
                });
            }
            let mut samples = std::collections::BTreeMap::new();
            for round in rounds.iter().filter(|r| r.owner != receiver) {
                if !round
                    .position
                    .iter()
                    .chain(&round.previous)
                    .all(|v| v.is_finite())
                {
                    continue;
                }
                let delta = sub(round.position, own.position);
                let bearing = || delta[0].atan2(delta[2]).to_degrees().rem_euclid(360.);
                let start = std::array::from_fn(|i| {
                    round.previous[i] - own.position[i] + own.velocity[i] / 120.
                });
                let movement = sub(delta, start);
                let length = dot(movement, movement);
                let fraction = if length > 0. {
                    (-dot(start, movement) / length).clamp(0., 1.)
                } else {
                    0.
                };
                let nearest: Vector = std::array::from_fn(|i| start[i] + movement[i] * fraction);
                let point = std::array::from_fn(|i| own.position[i] + nearest[i]);
                if dot(nearest, nearest) <= PROXIMITY_RADIUS_FT.powi(2) && terrain_clear(point) {
                    fresh.push(Cue {
                        evidence: Evidence::ClosePass,
                        round: Some(round.id),
                        observed_tick: tick,
                        time_to_danger_s: 0.,
                        bearing_world_deg: Some(bearing()),
                    });
                }
                if !round.tracer || !visible(round.position) {
                    continue;
                }
                samples.insert(round.id, round.position);
                let Some(previous) = self.samples.get(&round.id) else {
                    continue;
                };
                let velocity = std::array::from_fn(|i| {
                    (round.position[i] - previous[i]) * 120. - own.velocity[i]
                });
                if let Some((time, miss)) = closest_approach(delta, velocity)
                    && time <= PREDICTION_SECONDS
                    && miss <= THREAT_RADIUS_FT
                {
                    fresh.push(Cue {
                        evidence: Evidence::VisualTrajectory,
                        round: Some(round.id),
                        observed_tick: tick,
                        time_to_danger_s: time,
                        bearing_world_deg: Some(bearing()),
                    });
                }
            }
            self.samples = samples;
            if let Some(cue) = fresh.into_iter().min_by(|a, b| {
                a.time_to_danger_s
                    .total_cmp(&b.time_to_danger_s)
                    .then(a.round.cmp(&b.round))
            }) {
                self.cue = Some(cue);
            }
        }
    }

    /// Indexed and prefiltered, with reused sample lists, the service gives
    /// the original's cues and samples tick after tick: dense and sparse
    /// fire, repeated and unordered ids, odd values, rounds seen far away and
    /// passing close.
    #[test]
    fn indexed_observation_matches_the_original_over_random_fire() {
        let mut state = 0xa54f_f53a_5f1d_36f1_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let unit = move |value: u64| (value % 1_000_000) as f64 / 1e6;
        let (mut cues, mut sampled, mut passes) = (0, 0, 0);
        for scenario in 0..60 {
            let mut reference = Reference::default();
            let mut service = Service::default();
            let mut position = [0., 8_000., 0.];
            let velocity = [600. * unit(next()) - 300., 0., 700.];
            // A few hundred rounds around the aircraft, some through it.
            let count = 1 + next() % 400;
            let mut rounds: Vec<Round> = (0..count)
                .map(|k| {
                    let spread = if k % 3 == 0 { 600. } else { 40_000. };
                    let at = [
                        position[0] + (unit(next()) - 0.5) * spread,
                        position[1] + (unit(next()) - 0.5) * spread * 0.2,
                        position[2] + (unit(next()) - 0.5) * spread,
                    ];
                    Round {
                        // Some ids repeat, and the list is not in id order.
                        id: if k % 37 == 5 {
                            3
                        } else {
                            (next() % 5_000) as u32
                        },
                        owner: (next() % 4) as u32,
                        position: at,
                        previous: at,
                        tracer: next() % 3 != 0,
                    }
                })
                .collect();
            if scenario % 7 == 0 {
                rounds[0].position[1] = f64::NAN;
            }
            let lookout_offset = [unit(next()) * 30., 0., unit(next()) * 30.];
            let radius = [18_228., 21_266., 24_304., 30_380.][scenario % 4];
            for tick in 0..30 {
                position = std::array::from_fn(|i| position[i] + velocity[i] / 120.);
                for (k, round) in rounds.iter_mut().enumerate() {
                    round.previous = round.position;
                    // Fast rounds toward the aircraft, slow ones across.
                    let speed = if k % 2 == 0 { 3_300. } else { 900. };
                    let to = sub(position, round.position);
                    let length = dot(to, to).sqrt().max(1.);
                    round.position =
                        std::array::from_fn(|i| round.position[i] + to[i] / length * speed / 120.);
                }
                let own = DefenseOwn {
                    position,
                    velocity,
                    ..own()
                };
                let center = std::array::from_fn(|i| position[i] + lookout_offset[i]);
                let visible = |p: Vector| {
                    let d = sub(p, center);
                    dot(d, d).sqrt() <= radius && !(p[0].to_bits() >> 7).is_multiple_of(3)
                };
                let clear = |p: Vector| !(p[2].to_bits() >> 9).is_multiple_of(5);
                if tick % 11 == 3 {
                    reference.pending_hit = true;
                    service.hit();
                }
                reference.observe(tick, 1, own, &rounds, visible, clear);
                let indexed = Rounds::new(&rounds);
                service.observe_near(
                    tick,
                    1,
                    own,
                    &indexed,
                    SightReach { center, radius },
                    visible,
                    clear,
                );
                assert_eq!(service.cue, reference.cue, "{scenario} {tick}");
                assert_eq!(
                    format!("{:?}", service.samples),
                    format!("{:?}", reference.samples)
                );
                cues += usize::from(service.cue.is_some_and(|c| c.observed_tick == tick));
                sampled += reference.samples.len();
                passes +=
                    usize::from(service.cue.is_some_and(|c| {
                        c.evidence == Evidence::ClosePass && c.observed_tick == tick
                    }));
            }
        }
        assert!(
            cues > 200 && sampled > 10_000 && passes > 20,
            "{cues} {sampled} {passes}"
        );
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "incoming_fire_checkpoint.rs"]
mod checkpoint;
