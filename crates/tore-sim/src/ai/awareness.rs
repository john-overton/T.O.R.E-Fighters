//! Skill-scaled aircraft observations and frozen target memory.
//!
//! Spec-derived implementation of the authored rules in
//! `docs/spec/ai-awareness.md`, visual awareness and memory.
//!
//! This module owns no world references. Callers supply same-tick observations
//! and the mission's surviving actor IDs, so a lost contact cannot receive
//! hidden pose updates through its remembered ID.

use std::collections::{BTreeMap, BTreeSet};

use super::{Experience, controller::TargetView, experience::ResolvedExperience, targeting::Side};

pub const VISUAL_CONE_HALF_ANGLE_DEG: f64 = 60.0;
pub const FEET_PER_NAUTICAL_MILE: f64 = crate::sensors::FEET_PER_NAUTICAL_MILE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationSource {
    Visual,
    Radar,
    Infrared,
    /// Explicit input from a deterministic replay or synthetic fixture.
    Fixture,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observation {
    pub target: TargetView,
    pub velocity: [f64; 3],
    pub source: ObservationSource,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceTimestamps {
    pub visual: Option<u64>,
    pub radar: Option<u64>,
    pub infrared: Option<u64>,
    pub fixture: Option<u64>,
}

impl SourceTimestamps {
    fn record(&mut self, source: ObservationSource, tick: u64) {
        *match source {
            ObservationSource::Visual => &mut self.visual,
            ObservationSource::Radar => &mut self.radar,
            ObservationSource::Infrared => &mut self.infrared,
            ObservationSource::Fixture => &mut self.fixture,
        } = Some(tick);
    }
}

/// A copied observation. Its pose changes only when [`Memory::observe`] gets a
/// new observation for this ID.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub target: TargetView,
    pub velocity: [f64; 3],
    pub first_observed_tick: u64,
    pub last_observed_tick: u64,
    pub source_ticks: SourceTimestamps,
}

impl Snapshot {
    fn new(tick: u64, observation: Observation) -> Self {
        let mut source_ticks = SourceTimestamps::default();
        source_ticks.record(observation.source, tick);
        Self {
            target: observation.target,
            velocity: observation.velocity,
            first_observed_tick: tick,
            last_observed_tick: tick,
            source_ticks,
        }
    }

    fn refresh(&mut self, tick: u64, observation: Observation) {
        self.target = observation.target;
        self.velocity = observation.velocity;
        self.last_observed_tick = tick;
        self.source_ticks.record(observation.source, tick);
    }
}

/// Current observations and retained frozen snapshots for one aircraft.
#[derive(Clone, Debug, PartialEq)]
pub struct Memory {
    experience: ResolvedExperience,
    own_side: Side,
    current: BTreeMap<u32, Snapshot>,
    remembered: BTreeMap<u32, Snapshot>,
    novice_target: Option<u32>,
}

impl Memory {
    pub fn new(experience: ResolvedExperience, own_side: Side) -> Self {
        Self {
            experience,
            own_side,
            current: BTreeMap::new(),
            remembered: BTreeMap::new(),
            novice_target: None,
        }
    }

    pub fn experience(&self) -> ResolvedExperience {
        self.experience
    }
    /// A Novice keeps its extra remembered contacts until its next target
    /// choice, where the one-target limit already discards them.
    pub fn set_experience(&mut self, experience: ResolvedExperience) {
        self.experience = experience;
    }

    /// Replace the same-tick observation set and refresh retained snapshots.
    /// Invalid and non-aircraft contacts are never admitted.
    pub fn observe(&mut self, tick: u64, observations: &[Observation]) {
        self.expire(tick);
        self.current.clear();

        for &observation in observations {
            if !observation.target.valid || !observation.target.is_aircraft {
                continue;
            }

            let id = observation.target.id;
            self.current
                .entry(id)
                .and_modify(|snapshot| snapshot.refresh(tick, observation))
                .or_insert_with(|| Snapshot::new(tick, observation));

            let retain =
                self.experience.level != Experience::Novice || self.novice_target == Some(id);
            if retain {
                self.remembered
                    .entry(id)
                    .and_modify(|snapshot| snapshot.refresh(tick, observation))
                    .or_insert_with(|| Snapshot::new(tick, observation));
            }
        }
    }

    /// Select a known target. A Novice may select a freshly observed hostile
    /// or its one retained hostile; changing selection discards the old record.
    pub fn select_target(&mut self, requested: Option<u32>) -> Option<&Snapshot> {
        if self.experience.level != Experience::Novice {
            return requested
                .and_then(|id| self.current.get(&id).or_else(|| self.remembered.get(&id)));
        }

        let Some(id) = requested else {
            return self.novice_target.and_then(|id| self.remembered.get(&id));
        };
        let candidate = self
            .current
            .get(&id)
            .or_else(|| self.remembered.get(&id))
            .copied()?;
        if candidate.target.side == self.own_side || !candidate.target.valid {
            return None;
        }

        if self.novice_target != Some(id) {
            self.remembered.clear();
            self.remembered.insert(id, candidate);
            self.novice_target = Some(id);
        }
        self.remembered.get(&id)
    }

    pub fn current_observations(&self) -> impl Iterator<Item = &Snapshot> {
        self.current.values()
    }

    pub fn remembered(&self) -> impl Iterator<Item = &Snapshot> {
        self.remembered.values()
    }

    pub fn snapshot(&self, id: u32) -> Option<&Snapshot> {
        self.remembered.get(&id)
    }

    /// Clear records for actors absent from the mission's explicit live-ID set.
    pub fn prune_lifecycle(&mut self, live_actor_ids: &[u32]) {
        let live: BTreeSet<_> = live_actor_ids.iter().copied().collect();
        self.current.retain(|id, _| live.contains(id));
        self.remembered.retain(|id, _| live.contains(id));
        if self.novice_target.is_some_and(|id| !live.contains(&id)) {
            self.novice_target = None;
        }
    }

    /// Mission restart hook. Actor IDs may be reused only after this reset.
    pub fn clear(&mut self) {
        self.current.clear();
        self.remembered.clear();
        self.novice_target = None;
    }

    pub fn expire(&mut self, tick: u64) {
        let Some(duration) = retention_ticks(self.experience.level) else {
            return;
        };
        self.remembered
            .retain(|_, snapshot| tick.saturating_sub(snapshot.last_observed_tick) < duration);
        if self
            .novice_target
            .is_some_and(|id| !self.remembered.contains_key(&id))
        {
            self.novice_target = None;
        }
    }
}

pub const fn retention_ticks(experience: Experience) -> Option<u64> {
    match experience {
        Experience::Novice => Some(1_800),
        Experience::Average => Some(10_800),
        Experience::Experienced => Some(14_400),
        Experience::Ace => None,
    }
}

pub const fn visual_range_feet(experience: Experience) -> f64 {
    let nautical_miles = match experience {
        Experience::Novice => 3.0,
        Experience::Average => 3.5,
        Experience::Experienced => 4.0,
        Experience::Ace => 5.0,
    };
    nautical_miles * FEET_PER_NAUTICAL_MILE
}

/// Test the inclusive, circular 3D visual cone. `visibility_limit_ft` is the
/// caller's environmental limit; terrain and other occlusion remain explicit.
pub fn visual_eligible(
    experience: Experience,
    observer_position: [f64; 3],
    observer_heading_deg: f64,
    observer_pitch_deg: f64,
    target_position: [f64; 3],
    visibility_limit_ft: Option<f64>,
    terrain_clear: bool,
) -> bool {
    if !terrain_clear {
        return false;
    }
    if visibility_limit_ft.is_some_and(f64::is_nan) {
        return false;
    }
    let delta: [f64; 3] = std::array::from_fn(|i| target_position[i] - observer_position[i]);
    let distance_sq = delta.iter().map(|v| v * v).sum::<f64>();
    if !distance_sq.is_finite() {
        return false;
    }
    let range = visibility_limit_ft
        .unwrap_or(f64::INFINITY)
        .max(0.0)
        .min(visual_range_feet(experience));
    if distance_sq > range * range {
        return false;
    }
    if distance_sq == 0.0 {
        return true;
    }

    let heading = observer_heading_deg.to_radians();
    let pitch = observer_pitch_deg.to_radians();
    let forward = [
        heading.sin() * pitch.cos(),
        pitch.sin(),
        heading.cos() * pitch.cos(),
    ];
    let cosine = forward.iter().zip(delta).map(|(a, b)| a * b).sum::<f64>() / distance_sq.sqrt();
    cosine + f64::EPSILON >= VISUAL_CONE_HALF_ANGLE_DEG.to_radians().cos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{ScalarSpeed, controller::TargetView, experience::ExperienceOrigin};

    fn resolved(level: Experience) -> ResolvedExperience {
        ResolvedExperience {
            level,
            origin: ExperienceOrigin::ExplicitPerObject,
        }
    }

    fn target(id: u32, side: u32, position: [f64; 3]) -> TargetView {
        TargetView {
            id,
            side: Side(side),
            position,
            heading_deg: 0.0,
            pitch_deg: 0.0,
            speed: ScalarSpeed(500.0),
            maximum_speed: ScalarSpeed(1_000.0),
            is_aircraft: true,
            is_fighter: true,
            human_controlled: false,
            valid: true,
            type_allowed: true,
            seeker_eligible: true,
            wing_attackers: 0,
            terrain_blocked: false,
            sensor_supported: true,
        }
    }

    fn observation(id: u32, side: u32, source: ObservationSource) -> Observation {
        Observation {
            target: target(id, side, [id as f64, 0.0, 100.0]),
            velocity: [1.0, 2.0, 3.0],
            source,
        }
    }

    #[test]
    fn visual_range_and_cone_boundaries_are_inclusive() {
        let range = visual_range_feet(Experience::Novice);
        let angle = VISUAL_CONE_HALF_ANGLE_DEG.to_radians();
        let edge = [range * angle.sin(), 0.0, range * angle.cos()];
        assert!(visual_eligible(
            Experience::Novice,
            [0.0; 3],
            0.0,
            0.0,
            edge,
            None,
            true
        ));
        assert!(!visual_eligible(
            Experience::Novice,
            [0.0; 3],
            0.0,
            0.0,
            [edge[0] + 1.0, 0.0, edge[2]],
            None,
            true
        ));
        assert!(!visual_eligible(
            Experience::Ace,
            [0.0; 3],
            0.0,
            0.0,
            [0.0, 0.0, 100.0],
            Some(99.0),
            true
        ));
        assert!(!visual_eligible(
            Experience::Ace,
            [0.0; 3],
            0.0,
            0.0,
            [0.0, 0.0, 100.0],
            None,
            false
        ));
    }

    #[test]
    fn lost_contact_keeps_frozen_snapshot_until_exact_expiry() {
        let mut memory = Memory::new(resolved(Experience::Average), Side(1));
        memory.observe(10, &[observation(2, 2, ObservationSource::Visual)]);
        let old_position = memory.snapshot(2).unwrap().target.position;
        let old_heading = memory.snapshot(2).unwrap().target.heading_deg;
        let mut hidden_turn = observation(2, 2, ObservationSource::Visual);
        hidden_turn.target.position = [9_000.0, 2_000.0, -8_000.0];
        hidden_turn.target.heading_deg = 180.0;
        assert_ne!(hidden_turn.target.position, old_position);
        // The hidden world state is deliberately not passed to `observe`.
        memory.observe(10_809, &[]);
        assert_eq!(memory.snapshot(2).unwrap().target.position, old_position);
        assert_eq!(memory.snapshot(2).unwrap().target.heading_deg, old_heading);
        memory.observe(10_810, &[]);
        assert!(memory.snapshot(2).is_none());
    }

    #[test]
    fn higher_skills_retain_all_observed_aircraft_including_friendlies() {
        let mut memory = Memory::new(resolved(Experience::Average), Side(1));
        memory.observe(
            0,
            &[
                observation(2, 2, ObservationSource::Radar),
                observation(3, 2, ObservationSource::Infrared),
                observation(4, 1, ObservationSource::Visual),
            ],
        );
        assert_eq!(memory.remembered().count(), 3);
        memory.observe(1, &[]);
        assert_eq!(memory.remembered().count(), 3);
    }

    #[test]
    fn novice_remembers_only_selected_hostile_and_change_discards_old() {
        let mut memory = Memory::new(resolved(Experience::Novice), Side(1));
        memory.observe(
            0,
            &[
                observation(2, 2, ObservationSource::Visual),
                observation(3, 2, ObservationSource::Radar),
                observation(4, 1, ObservationSource::Visual),
            ],
        );
        assert_eq!(memory.current_observations().count(), 3);
        assert_eq!(memory.remembered().count(), 0);
        assert!(memory.select_target(Some(4)).is_none());
        assert_eq!(memory.select_target(Some(2)).unwrap().target.id, 2);
        assert_eq!(memory.remembered().count(), 1);
        assert_eq!(memory.select_target(Some(3)).unwrap().target.id, 3);
        assert!(memory.snapshot(2).is_none());
    }

    #[test]
    fn each_source_refreshes_its_own_timestamp() {
        let mut memory = Memory::new(resolved(Experience::Experienced), Side(1));
        memory.observe(2, &[observation(2, 2, ObservationSource::Visual)]);
        memory.observe(7, &[observation(2, 2, ObservationSource::Radar)]);
        let snapshot = memory.snapshot(2).unwrap();
        assert_eq!(snapshot.source_ticks.visual, Some(2));
        assert_eq!(snapshot.source_ticks.radar, Some(7));
        assert_eq!(snapshot.source_ticks.infrared, None);
        assert_eq!(snapshot.last_observed_tick, 7);
    }

    #[test]
    fn pause_does_not_age_and_restart_or_death_clears_records() {
        let mut memory = Memory::new(resolved(Experience::Ace), Side(1));
        memory.observe(500, &[observation(2, 2, ObservationSource::Visual)]);
        memory.observe(500, &[]);
        assert!(memory.snapshot(2).is_some());
        memory.prune_lifecycle(&[1]);
        assert!(memory.snapshot(2).is_none());
        memory.observe(501, &[observation(2, 2, ObservationSource::Visual)]);
        memory.clear();
        assert_eq!(memory.current_observations().count(), 0);
        assert_eq!(memory.remembered().count(), 0);
    }

    #[test]
    fn invalid_observation_never_enters_current_or_novice_memory() {
        let mut invalid = observation(2, 2, ObservationSource::Visual);
        invalid.target.valid = false;
        let mut memory = Memory::new(resolved(Experience::Novice), Side(1));
        memory.observe(0, &[invalid]);
        assert_eq!(memory.current_observations().count(), 0);
        assert!(memory.select_target(Some(2)).is_none());
    }
}
