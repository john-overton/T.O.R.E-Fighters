//! Pure missile-defense assessment for actor-owned threat observations.

use crate::{
    attitude::Vector,
    combat::threats::{EvidenceSource, GuidanceClass, ThreatRecord},
};

use super::Experience;

pub const JINK_OFFSET_DEG: f64 = 45.0;
pub const JINK_LEG_TICKS: u64 = 240;
pub const DIVE_PITCH_DEG: f64 = -20.0;
pub const DIVE_LOOKAHEAD_S: u64 = 5;
pub const DIVE_CLEARANCE_FT: f64 = 1_000.0;
pub const RELEASE_WINDOW_S: f64 = 6.0;
pub const BURST_INTERVAL_TICKS: u64 = 240;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefenseOwn {
    pub position: Vector,
    pub velocity: Vector,
    pub heading_deg: f64,
    pub flight_path_pitch_deg: f64,
    pub speed_ft_s: f64,
    pub bank_deg: f64,
    pub usable_turn_rate_deg_s: f64,
    pub usable_pitch_rate_deg_s: f64,
    pub roll_in_time_s: f64,
    pub dive_speed_safe: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Maneuver {
    Jink,
    Notch,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionSuggestion {
    pub maneuver: Maneuver,
    pub heading_deg: f64,
    pub flight_path_pitch_deg: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BurstRequest {
    pub chaff: u8,
    pub flares: u8,
}

impl BurstRequest {
    pub const RADAR: Self = Self {
        chaff: 2,
        flares: 0,
    };
    pub const INFRARED: Self = Self {
        chaff: 0,
        flares: 2,
    };
    pub const MIXED: Self = Self {
        chaff: 2,
        flares: 2,
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefenseDebug {
    pub estimated_threat_time_s: Option<f64>,
    pub estimated_maneuver_time_s: f64,
    pub margin_s: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefenseDecision {
    pub threat_id: u32,
    pub motion: Option<MotionSuggestion>,
    pub burst: Option<BurstRequest>,
    pub debug: DefenseDebug,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DefenseState {
    selected_threat: Option<u32>,
    jink_base_heading_deg: Option<f64>,
    jink_heading_deg: Option<f64>,
    jink_started_tick: u64,
    last_burst_tick: Option<u64>,
}

impl DefenseState {
    pub fn clear_threat(&mut self) {
        self.selected_threat = None;
        self.jink_base_heading_deg = None;
        self.jink_heading_deg = None;
    }
}

pub fn decide<F>(
    tick: u64,
    experience: Experience,
    own: DefenseOwn,
    contacts: &[ThreatRecord],
    state: &mut DefenseState,
    mut ground_elevation_ft: F,
) -> Option<DefenseDecision>
where
    F: FnMut(Vector) -> f64,
{
    let Some(selected) = select_threat(own, contacts, state.selected_threat) else {
        state.clear_threat();
        return None;
    };
    let changed = state.selected_threat != Some(selected.missile_id);
    state.selected_threat = Some(selected.missile_id);

    let time = time_to_cpa(own, selected);
    let radar_bearing = selected.radar_bearing_deg;
    let preferred = if radar_bearing.is_some()
        && matches!(
            selected.source,
            EvidenceSource::ElectronicSupported | EvidenceSource::ElectronicActive
        ) {
        Maneuver::Notch
    } else {
        Maneuver::Jink
    };
    let heading = match (preferred, radar_bearing) {
        (Maneuver::Notch, Some(bearing)) => notch_heading(own.heading_deg, bearing),
        _ => jink_heading(tick, own.heading_deg, selected.missile_id, changed, state),
    };
    let pitch = if dive_is_safe(own, heading, &mut ground_elevation_ft) {
        DIVE_PITCH_DEG
    } else {
        own.flight_path_pitch_deg.max(0.0)
    };
    let maneuver_time = maneuver_time_s(own, heading, pitch);
    let margin = margin_s(experience);
    let insufficient_time = time.is_some_and(|value| value <= maneuver_time);
    let bearing_only = selected.position.is_none() || selected.velocity.is_none();
    let uncertain_directed_radar = time.is_none()
        && selected.targeting_receiver
        && matches!(
            selected.source,
            EvidenceSource::ElectronicSupported | EvidenceSource::ElectronicActive
        );
    let maneuver_now = experience == Experience::Novice
        || bearing_only
        || uncertain_directed_radar
        || time.is_some_and(|value| value <= maneuver_time + margin)
        || selected.stale;
    let may_burst = state
        .last_burst_tick
        .is_none_or(|last| tick.saturating_sub(last) >= BURST_INTERVAL_TICKS);
    let release_now = bearing_only
        || uncertain_directed_radar
        || insufficient_time
        || time.is_some_and(|value| value <= RELEASE_WINDOW_S);
    let burst =
        (release_now && may_burst && !selected.stale).then_some(match selected.guidance_class {
            Some(GuidanceClass::Radar) => BurstRequest::RADAR,
            Some(GuidanceClass::Infrared) => BurstRequest::INFRARED,
            Some(GuidanceClass::Passive) | None => BurstRequest::MIXED,
        });
    if burst.is_some() {
        state.last_burst_tick = Some(tick);
    }

    Some(DefenseDecision {
        threat_id: selected.missile_id,
        motion: maneuver_now.then_some(MotionSuggestion {
            maneuver: preferred,
            heading_deg: heading,
            flight_path_pitch_deg: pitch,
        }),
        burst,
        debug: DefenseDebug {
            estimated_threat_time_s: time,
            estimated_maneuver_time_s: maneuver_time,
            margin_s: margin,
        },
    })
}

fn select_threat(
    own: DefenseOwn,
    contacts: &[ThreatRecord],
    current: Option<u32>,
) -> Option<&ThreatRecord> {
    contacts
        .iter()
        .filter(|contact| {
            contact.targeting_receiver || (contact.stale && contact.was_targeting_receiver)
        })
        .min_by(|a, b| {
            let priority = |contact: &ThreatRecord| {
                time_to_cpa(own, contact).unwrap_or_else(|| {
                    if contact.position.is_none() || contact.velocity.is_none() {
                        -1.0
                    } else {
                        f64::INFINITY
                    }
                })
            };
            priority(a).total_cmp(&priority(b)).then_with(|| {
                match (current == Some(a.missile_id), current == Some(b.missile_id)) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.missile_id.cmp(&b.missile_id),
                }
            })
        })
}

fn time_to_cpa(own: DefenseOwn, contact: &ThreatRecord) -> Option<f64> {
    let position = contact.position?;
    let velocity = contact.velocity?;
    let relative_position = std::array::from_fn::<_, 3, _>(|i| position[i] - own.position[i]);
    let relative_velocity = std::array::from_fn::<_, 3, _>(|i| velocity[i] - own.velocity[i]);
    let speed_sq = relative_velocity
        .iter()
        .map(|value| value * value)
        .sum::<f64>();
    if speed_sq <= f64::EPSILON {
        return None;
    }
    let dot = relative_position
        .iter()
        .zip(relative_velocity)
        .map(|(a, b)| a * b)
        .sum::<f64>();
    (dot < 0.0).then_some(-dot / speed_sq)
}

fn margin_s(experience: Experience) -> f64 {
    match experience {
        Experience::Novice => 8.0,
        Experience::Average => 5.0,
        Experience::Experienced => 3.0,
        Experience::Ace => 1.5,
    }
}

fn notch_heading(heading: f64, relative_radar_bearing: f64) -> f64 {
    let radar_world = wrap(heading + relative_radar_bearing);
    let left = wrap(radar_world - 90.0);
    let right = wrap(radar_world + 90.0);
    if angle_delta(heading, left).abs() <= angle_delta(heading, right).abs() {
        left
    } else {
        right
    }
}

fn jink_heading(
    tick: u64,
    heading: f64,
    missile_id: u32,
    reset: bool,
    state: &mut DefenseState,
) -> f64 {
    if reset || state.jink_heading_deg.is_none() {
        let side = if missile_id.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        state.jink_base_heading_deg = Some(heading);
        state.jink_heading_deg = Some(wrap(heading + side * JINK_OFFSET_DEG));
        state.jink_started_tick = tick;
    } else if tick.saturating_sub(state.jink_started_tick) >= JINK_LEG_TICKS {
        let previous = state.jink_heading_deg.unwrap_or(heading);
        let base = state.jink_base_heading_deg.unwrap_or(heading);
        state.jink_heading_deg = Some(wrap(2.0 * base - previous));
        state.jink_started_tick = tick;
    }
    state.jink_heading_deg.unwrap_or(heading)
}

fn maneuver_time_s(own: DefenseOwn, heading: f64, pitch: f64) -> f64 {
    let heading_time =
        angle_delta(own.heading_deg, heading).abs() / own.usable_turn_rate_deg_s.max(f64::EPSILON);
    let pitch_time =
        (pitch - own.flight_path_pitch_deg).abs() / own.usable_pitch_rate_deg_s.max(f64::EPSILON);
    own.roll_in_time_s.max(0.0) + heading_time.max(pitch_time)
}

fn dive_is_safe<F>(own: DefenseOwn, heading: f64, terrain: &mut F) -> bool
where
    F: FnMut(Vector) -> f64,
{
    if !own.dive_speed_safe || own.speed_ft_s <= 0.0 {
        return false;
    }
    let heading = heading.to_radians();
    let pitch = DIVE_PITCH_DEG.to_radians();
    (1..=DIVE_LOOKAHEAD_S).all(|second| {
        let distance = own.speed_ft_s * second as f64;
        let position = [
            own.position[0] + heading.sin() * pitch.cos() * distance,
            own.position[1] + pitch.sin() * distance,
            own.position[2] + heading.cos() * pitch.cos() * distance,
        ];
        position[1] - terrain(position) >= DIVE_CLEARANCE_FT
    })
}

fn angle_delta(from: f64, to: f64) -> f64 {
    (to - from + 180.0).rem_euclid(360.0) - 180.0
}

fn wrap(value: f64) -> f64 {
    value.rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own() -> DefenseOwn {
        DefenseOwn {
            position: [0.0, 5_000.0, 0.0],
            velocity: [0.0; 3],
            heading_deg: 0.0,
            flight_path_pitch_deg: 0.0,
            speed_ft_s: 500.0,
            bank_deg: 0.0,
            usable_turn_rate_deg_s: 30.0,
            usable_pitch_rate_deg_s: 20.0,
            roll_in_time_s: 0.5,
            dive_speed_safe: true,
        }
    }
    fn threat(source: EvidenceSource, class: Option<GuidanceClass>) -> ThreatRecord {
        ThreatRecord {
            missile_id: 2,
            source,
            observed_tick: 0,
            bearing_deg: 0.0,
            position: Some([0.0, 5_000.0, 5_000.0]),
            velocity: Some([0.0, 0.0, -1_000.0]),
            guidance_class: class,
            targeting_receiver: true,
            was_targeting_receiver: true,
            stale: false,
            radar_bearing_deg: None,
        }
    }

    #[test]
    fn supported_radar_notches_support_bearing_and_requests_chaff() {
        let mut state = DefenseState::default();
        let mut contact = threat(
            EvidenceSource::ElectronicSupported,
            Some(GuidanceClass::Radar),
        );
        contact.radar_bearing_deg = Some(30.0);
        let decision = decide(
            0,
            Experience::Average,
            own(),
            &[contact],
            &mut state,
            |_| 0.0,
        )
        .unwrap();
        let motion = decision.motion.unwrap();
        assert_eq!(motion.maneuver, Maneuver::Notch);
        assert_eq!(motion.heading_deg, 300.0);
        assert_eq!(decision.burst, Some(BurstRequest::RADAR));
    }

    #[test]
    fn visual_unknown_jinks_and_uses_mixed_burst() {
        let mut state = DefenseState::default();
        let decision = decide(
            0,
            Experience::Novice,
            own(),
            &[threat(EvidenceSource::Visual, None)],
            &mut state,
            |_| 0.0,
        )
        .unwrap();
        assert_eq!(decision.motion.unwrap().maneuver, Maneuver::Jink);
        assert_eq!(decision.burst, Some(BurstRequest::MIXED));
    }

    #[test]
    fn dive_rejected_when_five_second_path_lacks_clearance() {
        let mut state = DefenseState::default();
        let decision = decide(
            0,
            Experience::Novice,
            own(),
            &[threat(EvidenceSource::Visual, None)],
            &mut state,
            |_| 4_000.0,
        )
        .unwrap();
        assert_eq!(decision.motion.unwrap().flight_path_pitch_deg, 0.0);
    }

    #[test]
    fn average_waits_when_reliable_motion_leaves_ample_time() {
        let mut state = DefenseState::default();
        let mut contact = threat(EvidenceSource::Visual, None);
        contact.position = Some([0.0, 5_000.0, 30_000.0]);
        contact.velocity = Some([0.0, 0.0, -1_000.0]);
        let decision = decide(
            0,
            Experience::Average,
            own(),
            &[contact],
            &mut state,
            |_| 0.0,
        )
        .unwrap();
        assert_eq!(decision.motion, None);
        assert_eq!(decision.burst, None);
    }

    #[test]
    fn directed_radar_without_positive_cpa_uses_conservative_response() {
        let mut state = DefenseState::default();
        let mut contact = threat(EvidenceSource::ElectronicActive, Some(GuidanceClass::Radar));
        contact.velocity = Some([0.0, 0.0, 1_000.0]);
        contact.radar_bearing_deg = Some(0.0);
        let decision = decide(0, Experience::Ace, own(), &[contact], &mut state, |_| 0.0).unwrap();
        assert!(decision.motion.is_some());
        assert_eq!(decision.burst, Some(BurstRequest::RADAR));
        assert_eq!(decision.debug.estimated_threat_time_s, None);
    }

    #[test]
    fn clearing_or_switching_threats_does_not_reset_burst_cooldown() {
        let mut state = DefenseState::default();
        let first = threat(EvidenceSource::Visual, None);
        assert!(
            decide(0, Experience::Novice, own(), &[first], &mut state, |_| 0.0)
                .unwrap()
                .burst
                .is_some()
        );
        assert!(decide(1, Experience::Novice, own(), &[], &mut state, |_| 0.0).is_none());
        let mut second = first;
        second.missile_id = 3;
        assert_eq!(
            decide(
                100,
                Experience::Novice,
                own(),
                &[second],
                &mut state,
                |_| 0.0
            )
            .unwrap()
            .burst,
            None
        );
    }

    #[test]
    fn burst_is_per_aircraft_and_limited_to_two_seconds() {
        let mut state = DefenseState::default();
        let contact = threat(EvidenceSource::Visual, None);
        assert!(
            decide(0, Experience::Novice, own(), &[contact], &mut state, |_| {
                0.0
            })
            .unwrap()
            .burst
            .is_some()
        );
        assert_eq!(
            decide(
                239,
                Experience::Novice,
                own(),
                &[contact],
                &mut state,
                |_| 0.0
            )
            .unwrap()
            .burst,
            None
        );
        assert!(
            decide(
                240,
                Experience::Novice,
                own(),
                &[contact],
                &mut state,
                |_| 0.0
            )
            .unwrap()
            .burst
            .is_some()
        );
    }
}
