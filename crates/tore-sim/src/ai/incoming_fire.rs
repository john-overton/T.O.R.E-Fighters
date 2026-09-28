//! Spec-derived incoming gunfire and anonymous-hit awareness.
//! See docs/spec/visual-awareness-under-fire.md. Shooter identity is deliberately
//! absent: recognizing danger cannot manufacture an offensive target.

use std::collections::BTreeMap;

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
    samples: BTreeMap<u32, Vector>,
    cue: Option<Cue>,
    pending_hit: bool,
    episode: Option<(u64, f64, f64)>,
}

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
        let mut samples = BTreeMap::new();
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
            let bearing = delta[0].atan2(delta[2]).to_degrees().rem_euclid(360.);
            // Swept relative separation catches a round crossing between ticks.
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
                    bearing_world_deg: Some(bearing),
                });
            }
            if !round.tracer || !visible(round.position) {
                continue;
            }
            samples.insert(round.id, round.position);
            let Some(previous) = self.samples.get(&round.id) else {
                continue;
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
                    bearing_world_deg: Some(bearing),
                });
            }
        }
        self.samples = samples;
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
}
