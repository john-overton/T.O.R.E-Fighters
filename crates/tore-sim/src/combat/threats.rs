//! Actor-owned missile observations shared by AI and the RWR display.
//!
//! This service deliberately copies permitted measurements. Consumers cannot
//! inspect a missile's live pose or hidden target between observation ticks.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::{
    ai::{Experience, awareness::visual_eligible},
    attitude::Vector,
};

use super::missiles::Guidance;

pub const REPORT_INTERVAL_TICKS: u64 = 30;
pub const LOST_GRACE_TICKS: u64 = 240;
pub const PASSIVE_EMITTER_RANGE_FT: f64 = 50.0 * crate::sensors::FEET_PER_NAUTICAL_MILE;
const VISUAL_CPA_LIMIT_FT: f64 = 1_000.0;
const VISUAL_CPA_HORIZON_S: f64 = 15.0;
const TICKS_PER_SECOND: f64 = 120.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Receiver {
    pub id: u32,
    pub position: Vector,
    pub velocity: Vector,
    pub heading_deg: f64,
    pub pitch_deg: f64,
    pub skill: Experience,
    pub rwr_operating: bool,
    pub visual_operating: bool,
    pub visibility_limit_ft: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MissileSnapshot {
    pub id: u32,
    pub owner: u32,
    pub position: Vector,
    pub velocity: Vector,
    pub guidance: Guidance,
    pub target: Option<u32>,
    pub radar_active: bool,
    pub radar_acquired: bool,
    pub supported: bool,
    pub supporting_radar_position: Option<Vector>,
    pub alive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceSource {
    ElectronicSupported,
    ElectronicActive,
    OwnLaunch,
    Visual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuidanceClass {
    Radar,
    Infrared,
    Passive,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThreatRecord {
    pub missile_id: u32,
    pub source: EvidenceSource,
    pub observed_tick: u64,
    pub bearing_deg: f64,
    pub position: Option<Vector>,
    pub velocity: Option<Vector>,
    pub guidance_class: Option<GuidanceClass>,
    pub targeting_receiver: bool,
    pub was_targeting_receiver: bool,
    pub stale: bool,
    /// Receiver-relative bearing to the supporting radar for S, or the
    /// missile's own radar for A. No launcher identity or pose is exposed.
    pub radar_bearing_deg: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
struct VisualSample {
    tick: u64,
    position: Vector,
}

#[derive(Clone, Copy, Debug)]
struct Stored {
    record: ThreatRecord,
    current_this_tick: bool,
    last_report_tick: u64,
    visual_sample: Option<VisualSample>,
    radar_source_position: Option<Vector>,
    lost_since_tick: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct ThreatService {
    receiver_id: u32,
    contacts: BTreeMap<u32, Stored>,
}

impl ThreatService {
    pub fn new(receiver_id: u32) -> Self {
        Self {
            receiver_id,
            contacts: BTreeMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.contacts.clear();
    }

    /// Observe one simulation tick. `terrain_clear` is the caller's line-of-
    /// sight predicate and is consulted only for visual evidence.
    pub fn observe<F>(
        &mut self,
        tick: u64,
        receiver: Receiver,
        missiles: &[MissileSnapshot],
        mut terrain_clear: F,
    ) where
        F: FnMut(Vector, Vector) -> bool,
    {
        debug_assert_eq!(receiver.id, self.receiver_id);
        for stored in self.contacts.values_mut() {
            stored.current_this_tick = false;
            if let Some(position) = stored.record.position {
                stored.record.bearing_deg =
                    bearing_deg(receiver.position, receiver.heading_deg, position);
            }
            stored.record.radar_bearing_deg = stored
                .radar_source_position
                .map(|position| bearing_deg(receiver.position, receiver.heading_deg, position));
        }

        let live_ids: BTreeSet<_> = missiles
            .iter()
            .filter(|missile| missile.alive)
            .map(|missile| missile.id)
            .collect();
        self.contacts.retain(|id, _| live_ids.contains(id));

        for missile in missiles {
            if !missile.alive {
                self.contacts.remove(&missile.id);
                continue;
            }

            let own_launch = missile.owner == receiver.id && receiver.rwr_operating;
            let electronic = receiver
                .rwr_operating
                .then(|| electronic_evidence(self.receiver_id, missile))
                .flatten();
            let visible = receiver.visual_operating
                && visual_eligible(
                    receiver.skill,
                    receiver.position,
                    receiver.heading_deg,
                    receiver.pitch_deg,
                    missile.position,
                    receiver.visibility_limit_ft,
                    terrain_clear(receiver.position, missile.position),
                );

            if own_launch {
                let due = self.contacts.get(&missile.id).is_none_or(|stored| {
                    tick.saturating_sub(stored.last_report_tick) >= REPORT_INTERVAL_TICKS
                });
                if due {
                    self.insert_report(
                        tick,
                        receiver,
                        missile,
                        EvidenceSource::OwnLaunch,
                        false,
                        None,
                    );
                } else if let Some(stored) = self.contacts.get_mut(&missile.id) {
                    stored.current_this_tick = true;
                    stored.record.stale = false;
                    stored.lost_since_tick = None;
                }
            } else if let Some((source, targeting, radar_source)) =
                electronic.filter(|(_, targeting, _)| {
                    *targeting
                        || distance(receiver.position, missile.position) <= PASSIVE_EMITTER_RANGE_FT
                })
            {
                let due = self.contacts.get(&missile.id).is_none_or(|stored| {
                    tick.saturating_sub(stored.last_report_tick) >= REPORT_INTERVAL_TICKS
                        || (targeting && !stored.record.targeting_receiver)
                        || !matches!(
                            stored.record.source,
                            EvidenceSource::ElectronicSupported | EvidenceSource::ElectronicActive
                        )
                });
                if due {
                    if targeting {
                        self.insert_report(tick, receiver, missile, source, true, radar_source);
                    } else {
                        self.insert_bearing_report(tick, receiver, missile, source);
                    }
                } else if let Some(stored) = self.contacts.get_mut(&missile.id) {
                    stored.current_this_tick = true;
                    stored.record.targeting_receiver = targeting;
                    stored.record.was_targeting_receiver |= targeting;
                    stored.record.stale = false;
                    stored.lost_since_tick = None;
                }
            } else if visible {
                self.observe_visual(tick, receiver, missile);
            }
        }

        self.contacts.retain(|_, stored| {
            if stored.current_this_tick {
                true
            } else {
                stored.record.targeting_receiver = false;
                stored.record.stale = true;
                let lost_since = if stored.record.source == EvidenceSource::Visual {
                    stored.record.observed_tick
                } else {
                    *stored.lost_since_tick.get_or_insert(tick)
                };
                tick.saturating_sub(lost_since) < LOST_GRACE_TICKS
            }
        });
    }

    pub fn records(&self) -> impl Iterator<Item = &ThreatRecord> {
        self.contacts.values().map(|stored| &stored.record)
    }

    fn insert_report(
        &mut self,
        tick: u64,
        receiver: Receiver,
        missile: &MissileSnapshot,
        source: EvidenceSource,
        targeting: bool,
        radar_source_position: Option<Vector>,
    ) {
        self.contacts.insert(
            missile.id,
            Stored {
                record: ThreatRecord {
                    missile_id: missile.id,
                    source,
                    observed_tick: tick,
                    bearing_deg: bearing_deg(
                        receiver.position,
                        receiver.heading_deg,
                        missile.position,
                    ),
                    position: Some(missile.position),
                    velocity: Some(missile.velocity),
                    guidance_class: Some(guidance_class(missile.guidance)),
                    targeting_receiver: targeting,
                    was_targeting_receiver: targeting,
                    stale: false,
                    radar_bearing_deg: radar_source_position.map(|position| {
                        bearing_deg(receiver.position, receiver.heading_deg, position)
                    }),
                },
                current_this_tick: true,
                last_report_tick: tick,
                visual_sample: None,
                radar_source_position,
                lost_since_tick: None,
            },
        );
    }

    fn observe_visual(&mut self, tick: u64, receiver: Receiver, missile: &MissileSnapshot) {
        let previous = self
            .contacts
            .get(&missile.id)
            .and_then(|stored| stored.visual_sample);
        let observed_velocity = previous.and_then(|sample| {
            let elapsed = tick.saturating_sub(sample.tick);
            (elapsed > 0).then(|| {
                let scale = TICKS_PER_SECOND / elapsed as f64;
                std::array::from_fn(|i| (missile.position[i] - sample.position[i]) * scale)
            })
        });
        let incoming = observed_velocity.is_some_and(|velocity| {
            visual_incoming(
                receiver.position,
                receiver.velocity,
                missile.position,
                velocity,
            )
        });
        let record = ThreatRecord {
            missile_id: missile.id,
            source: EvidenceSource::Visual,
            observed_tick: tick,
            bearing_deg: bearing_deg(receiver.position, receiver.heading_deg, missile.position),
            position: Some(missile.position),
            velocity: observed_velocity,
            guidance_class: None,
            targeting_receiver: incoming,
            was_targeting_receiver: incoming,
            stale: false,
            radar_bearing_deg: None,
        };
        self.contacts.insert(
            missile.id,
            Stored {
                record,
                current_this_tick: true,
                last_report_tick: tick,
                visual_sample: Some(VisualSample {
                    tick,
                    position: missile.position,
                }),
                radar_source_position: None,
                lost_since_tick: None,
            },
        );
    }

    fn insert_bearing_report(
        &mut self,
        tick: u64,
        receiver: Receiver,
        missile: &MissileSnapshot,
        source: EvidenceSource,
    ) {
        self.contacts.insert(
            missile.id,
            Stored {
                record: ThreatRecord {
                    missile_id: missile.id,
                    source,
                    observed_tick: tick,
                    bearing_deg: bearing_deg(
                        receiver.position,
                        receiver.heading_deg,
                        missile.position,
                    ),
                    position: None,
                    velocity: None,
                    guidance_class: Some(GuidanceClass::Radar),
                    targeting_receiver: false,
                    was_targeting_receiver: false,
                    stale: false,
                    radar_bearing_deg: Some(bearing_deg(
                        receiver.position,
                        receiver.heading_deg,
                        missile.position,
                    )),
                },
                current_this_tick: true,
                last_report_tick: tick,
                visual_sample: None,
                radar_source_position: Some(missile.position),
                lost_since_tick: None,
            },
        );
    }
}

fn guidance_class(guidance: Guidance) -> GuidanceClass {
    match guidance {
        Guidance::Supported | Guidance::Active => GuidanceClass::Radar,
        Guidance::Infrared => GuidanceClass::Infrared,
        Guidance::Emitter => GuidanceClass::Passive,
    }
}

fn electronic_evidence(
    receiver_id: u32,
    missile: &MissileSnapshot,
) -> Option<(EvidenceSource, bool, Option<Vector>)> {
    match missile.guidance {
        Guidance::Supported if missile.supported && missile.target == Some(receiver_id) => Some((
            EvidenceSource::ElectronicSupported,
            true,
            missile.supporting_radar_position,
        )),
        Guidance::Active if missile.radar_active => Some((
            EvidenceSource::ElectronicActive,
            missile.radar_acquired && missile.target == Some(receiver_id),
            Some(missile.position),
        )),
        Guidance::Supported | Guidance::Active | Guidance::Infrared | Guidance::Emitter => None,
    }
}

fn bearing_deg(from: Vector, own_heading_deg: f64, to: Vector) -> f64 {
    let world = (to[0] - from[0]).atan2(to[2] - from[2]).to_degrees();
    (world - own_heading_deg).rem_euclid(360.0)
}

fn distance(a: Vector, b: Vector) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn visual_incoming(
    own_position: Vector,
    own_velocity: Vector,
    missile_position: Vector,
    missile_velocity: Vector,
) -> bool {
    let relative_position =
        std::array::from_fn::<_, 3, _>(|i| missile_position[i] - own_position[i]);
    let relative_velocity =
        std::array::from_fn::<_, 3, _>(|i| missile_velocity[i] - own_velocity[i]);
    let speed_sq = relative_velocity.iter().map(|v| v * v).sum::<f64>();
    if speed_sq <= f64::EPSILON {
        return false;
    }
    let dot = relative_position
        .iter()
        .zip(relative_velocity)
        .map(|(a, b)| a * b)
        .sum::<f64>();
    if dot >= 0.0 {
        return false;
    }
    let time = (-dot / speed_sq).clamp(0.0, VISUAL_CPA_HORIZON_S);
    if time <= 0.0 || time > VISUAL_CPA_HORIZON_S {
        return false;
    }
    let closest_sq = (0..3)
        .map(|i| (relative_position[i] + relative_velocity[i] * time).powi(2))
        .sum::<f64>();
    closest_sq <= VISUAL_CPA_LIMIT_FT.powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receiver() -> Receiver {
        Receiver {
            id: 7,
            position: [0.0; 3],
            velocity: [0.0; 3],
            heading_deg: 0.0,
            pitch_deg: 0.0,
            skill: Experience::Ace,
            rwr_operating: true,
            visual_operating: true,
            visibility_limit_ft: None,
        }
    }
    fn missile(guidance: Guidance) -> MissileSnapshot {
        MissileSnapshot {
            id: 2,
            owner: 99,
            position: [0.0, 0.0, 10_000.0],
            velocity: [0.0, 0.0, -1_000.0],
            guidance,
            target: Some(7),
            radar_active: false,
            radar_acquired: false,
            supported: false,
            supporting_radar_position: None,
            alive: true,
        }
    }

    #[test]
    fn supported_warns_and_active_requires_actual_acquisition_to_blink() {
        let mut service = ThreatService::new(7);
        let mut s = missile(Guidance::Supported);
        s.supported = true;
        s.supporting_radar_position = Some([1_000.0, 0.0, 0.0]);
        service.observe(0, receiver(), &[s], |_, _| false);
        assert!(service.records().next().unwrap().targeting_receiver);

        service.clear();
        let mut a = missile(Guidance::Active);
        a.radar_active = true;
        service.observe(1, receiver(), &[a], |_, _| false);
        let steady = service.records().next().unwrap();
        assert!(!steady.targeting_receiver);
        assert_eq!(steady.position, None);
        a.radar_acquired = true;
        service.observe(2, receiver(), &[a], |_, _| false);
        let pitbull = service.records().next().unwrap();
        assert!(pitbull.targeting_receiver);
        assert_eq!(pitbull.position, Some(a.position));
    }

    #[test]
    fn passive_missile_needs_two_visual_samples_and_never_leaks_class() {
        let mut service = ThreatService::new(7);
        let mut m = missile(Guidance::Infrared);
        m.target = Some(99);
        service.observe(0, receiver(), &[m], |_, _| true);
        assert!(!service.records().next().unwrap().targeting_receiver);
        m.position[2] -= 1_000.0 / TICKS_PER_SECOND;
        service.observe(1, receiver(), &[m], |_, _| true);
        let record = service.records().next().unwrap();
        assert!(record.targeting_receiver);
        assert_eq!(record.guidance_class, None);
    }

    #[test]
    fn snapshots_freeze_then_become_steady_during_grace() {
        let mut service = ThreatService::new(7);
        let mut m = missile(Guidance::Active);
        m.radar_active = true;
        m.radar_acquired = true;
        service.observe(0, receiver(), &[m], |_, _| false);
        m.position[2] = 5_000.0;
        service.observe(1, receiver(), &[m], |_, _| false);
        assert_eq!(
            service.records().next().unwrap().position.unwrap()[2],
            10_000.0
        );
        m.radar_active = false;
        service.observe(29, receiver(), &[m], |_, _| false);
        let record = service.records().next().unwrap();
        assert!(record.stale);
        assert!(!record.targeting_receiver);
        assert_eq!(record.source, EvidenceSource::ElectronicActive);
        service.observe(268, receiver(), &[m], |_, _| false);
        assert!(service.records().next().is_some());
        service.observe(269, receiver(), &[m], |_, _| false);
        assert!(service.records().next().is_none());
    }

    #[test]
    fn visual_grace_expires_exactly_from_the_last_sighting() {
        let mut service = ThreatService::new(7);
        let mut m = missile(Guidance::Infrared);
        service.observe(0, receiver(), &[m], |_, _| true);
        m.position[2] -= 1_000.0 / TICKS_PER_SECOND;
        service.observe(1, receiver(), &[m], |_, _| true);
        assert!(service.records().next().unwrap().targeting_receiver);

        m.position[2] = -10_000.0;
        service.observe(240, receiver(), &[m], |_, _| true);
        assert!(service.records().next().is_some());
        service.observe(241, receiver(), &[m], |_, _| true);
        assert!(service.records().next().is_none());
    }

    #[test]
    fn own_silent_ordnance_is_steady_and_requires_an_operating_rwr() {
        let mut service = ThreatService::new(7);
        let mut m = missile(Guidance::Infrared);
        m.owner = 7;
        service.observe(0, receiver(), &[m], |_, _| false);
        let own = service.records().next().unwrap();
        assert_eq!(own.source, EvidenceSource::OwnLaunch);
        assert!(!own.targeting_receiver);

        let mut failed = receiver();
        failed.rwr_operating = false;
        service.observe(1, failed, &[m], |_, _| false);
        assert!(service.records().next().unwrap().stale);
    }
}
