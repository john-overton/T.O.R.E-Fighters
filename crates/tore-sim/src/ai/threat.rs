//! Threat warnings, countermeasures and script reason priority (B47), from
//! [`docs/spec/ai.md`](../../../../docs/spec/ai.md), with the device-release
//! gate from the experience spec ("Other experience effects").
//!
//! This component produces decisions only: how long a launch warning is
//! delayed, what an AI aircraft does when it arrives, whether devices are
//! released and from which dispenser, whether one device decoys one missile,
//! and whether a new script reason resumes or restarts a running script. The
//! release cadence itself is consumed by
//! [`weapon_service::DeviceReleaseSchedule`](super::weapon_service::DeviceReleaseSchedule),
//! which takes [`DeviceRelease`] as its input; nothing is duplicated here.
//! Every fact from the rest of the simulation (flight state, station fit,
//! side, current target, seeker class, mission hold time) is an explicit
//! input.
//!
//! Fitted choices (agent decisions, 2026-09-17), each where the spec is silent:
//!
//! - The receiver gates run in the order the spec lists them: hold time,
//!   flight state, dispenser station, countermeasure roll, same side, current
//!   target. The countermeasure roll happens before the same-side check, so a
//!   friendly launch still triggers devices; the spec only says it sends no
//!   maneuver.
//! - The mission hold time is an opaque host time-of-day; a warning arriving
//!   exactly at the hold time is no longer suppressed.
//! - Non-aircraft targets are reported as [`WarningDelay::NotWarned`]: the
//!   spec delivers warnings to "the aircraft the missile was fired at" and
//!   says nothing about ships or sites.

use super::{AiError, DecisionRandom, Experience, Result, ScalarSpeed, SpeedLimits};

/// Two statute miles in feet: one second of warning delay per two miles.
pub const DELAY_DISTANCE_STEP_FT: f64 = 10560.0;
/// B47: a human-flown target is warned one second after launch.
pub const HUMAN_DELAY_QUARTERS: u32 = 4;
/// B47: an AI aircraft in ordinary flight is warned six seconds after launch.
pub const ORDINARY_FLIGHT_DELAY_QUARTERS: u32 = 24;
/// B47: attack state, already engaging the launcher: one second.
pub const ENGAGING_LAUNCHER_DELAY_QUARTERS: u32 = 4;
/// B47: attack state, not engaging the launcher: three seconds.
pub const ATTACK_STATE_DELAY_QUARTERS: u32 = 12;
/// B47: the distance term is capped at twenty seconds.
pub const DISTANCE_DELAY_CAP_QUARTERS: u32 = 80;
/// B47: experience term of 6, 3, 1 or 0 seconds, Novice through Ace.
pub const EXPERIENCE_DELAY_QUARTERS: [u32; 4] = [24, 12, 4, 0];
/// B47: the minimum delay is half a second.
pub const MIN_DELAY_QUARTERS: u32 = 2;
/// Experience spec: countermeasure roll thresholds, Novice through Ace.
pub const COUNTERMEASURE_THRESHOLDS: [u8; 4] = [35, 50, 75, 90];
/// B47: a successful roll releases two or three devices.
pub const MIN_DEVICE_COUNT: u8 = 2;
pub const MAX_DEVICE_COUNT: u8 = 3;
/// B47: the fallback maneuver reverses course, left or right.
pub const COURSE_REVERSAL_DEGREES: i16 = 180;

/// Seeker class carried by the warning; it selects the device class and the
/// script reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SeekerClass {
    Infrared,
    Radar,
}

/// The target's AI state at launch as far as the delay rule cares. The
/// producers of the two attack states are open (B47), so the caller names
/// the state explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackState {
    OrdinaryFlight,
    /// One of the two attack states; `engaging_launcher` is true when the
    /// target's current target is the launching aircraft.
    AttackState {
        engaging_launcher: bool,
    },
}

/// Who the missile was fired at, for the delay rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningTarget {
    Human,
    Ai {
        experience: Experience,
        state: AttackState,
    },
    /// Ships, sites and other non-aircraft objects.
    NotAircraft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningDelay {
    /// Quarter-second counts after launch.
    Quarters(u32),
    /// The spec delivers launch warnings to aircraft only.
    NotWarned,
}

/// B47: delay between a missile launch and the target's warning.
///
/// `distance_feet` is the missile-to-target distance at launch. A human
/// target is warned after one second regardless of distance or experience.
pub fn warning_delay(target: &WarningTarget, distance_feet: f64) -> Result<WarningDelay> {
    if !(distance_feet.is_finite() && distance_feet >= 0.0) {
        return Err(AiError::InvalidInput(
            "launch distance must be finite and non-negative",
        ));
    }
    let (base, experience_term, cap) = match *target {
        WarningTarget::NotAircraft => return Ok(WarningDelay::NotWarned),
        WarningTarget::Human => (HUMAN_DELAY_QUARTERS, 0, Some(HUMAN_DELAY_QUARTERS)),
        WarningTarget::Ai { experience, state } => {
            let base = match state {
                AttackState::OrdinaryFlight => ORDINARY_FLIGHT_DELAY_QUARTERS,
                AttackState::AttackState {
                    engaging_launcher: true,
                } => ENGAGING_LAUNCHER_DELAY_QUARTERS,
                AttackState::AttackState {
                    engaging_launcher: false,
                } => ATTACK_STATE_DELAY_QUARTERS,
            };
            (base, EXPERIENCE_DELAY_QUARTERS[experience.index()], None)
        }
    };
    let distance_steps = (distance_feet / DELAY_DISTANCE_STEP_FT).floor();
    let distance_term = (distance_steps * 4.0).min(f64::from(DISTANCE_DELAY_CAP_QUARTERS)) as u32;
    let mut quarters = base + distance_term + experience_term;
    if let Some(cap) = cap {
        quarters = quarters.min(cap);
    }
    Ok(WarningDelay::Quarters(quarters.max(MIN_DELAY_QUARTERS)))
}

/// Opaque host time of day, used only for ordering against the mission hold
/// time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeOfDay(pub u64);

/// The receiving AI aircraft's flight state as the warning gates see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightState {
    /// Free flight, including the attack and evasion states.
    Free,
    TakingOff,
    /// One of the first two approach states: the warning cancels the approach.
    EarlyApproach,
    /// One of the later landing states: warnings are ignored.
    LateLanding,
}

/// What the receiver knows about the launching aircraft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Launcher {
    pub same_side: bool,
    /// The launcher is already the receiver's current target.
    pub is_current_target: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WarningInputs {
    pub seeker: SeekerClass,
    pub experience: Experience,
    pub flight_state: FlightState,
    pub has_dispenser_station: bool,
    pub launcher: Launcher,
    /// Mission-authored hold time, if any, and the current time of day.
    pub hold_until: Option<TimeOfDay>,
    pub now: TimeOfDay,
}

/// Devices requested by a successful countermeasure roll. Consumed by the
/// weapon service's release schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceRelease {
    /// 2 or 3.
    pub count: u8,
    pub class: SeekerClass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suppression {
    /// The mission hold time has not been reached.
    HoldTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningReaction {
    Suppressed(Suppression),
    /// Taking off or in a late landing state.
    Ignored,
    /// No countermeasure dispenser station: nothing happens at all.
    NoReaction,
    /// Same-side launcher: a radio message, no maneuver.
    RadioOnly,
    /// The launcher is already the current target: no maneuver.
    NoManeuver,
    /// Send a wing reaction and run the fighter script with this reason.
    Maneuver {
        reason: ScriptReason,
        wing_reaction: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WarningOutcome {
    /// True when an early approach was cancelled for free flight.
    pub approach_abandoned: bool,
    /// Devices to schedule, when the roll ran and passed.
    pub devices: Option<DeviceRelease>,
    pub reaction: WarningReaction,
}

/// B47: what an AI aircraft does when its launch warning arrives.
pub fn receive_warning(inputs: &WarningInputs, random: &mut DecisionRandom) -> WarningOutcome {
    let mut outcome = WarningOutcome {
        approach_abandoned: false,
        devices: None,
        reaction: WarningReaction::NoReaction,
    };
    if inputs.hold_until.is_some_and(|hold| inputs.now < hold) {
        outcome.reaction = WarningReaction::Suppressed(Suppression::HoldTime);
        return outcome;
    }
    match inputs.flight_state {
        FlightState::TakingOff | FlightState::LateLanding => {
            outcome.reaction = WarningReaction::Ignored;
            return outcome;
        }
        FlightState::EarlyApproach => outcome.approach_abandoned = true,
        FlightState::Free => {}
    }
    if !inputs.has_dispenser_station {
        return outcome;
    }
    outcome.devices = countermeasure_gate(inputs.experience, inputs.seeker, random);
    outcome.reaction = if inputs.launcher.same_side {
        WarningReaction::RadioOnly
    } else if inputs.launcher.is_current_target {
        WarningReaction::NoManeuver
    } else {
        WarningReaction::Maneuver {
            reason: ScriptReason::from(inputs.seeker),
            wing_reaction: true,
        }
    };
    outcome
}

/// Experience spec: roll for countermeasures at 35, 50, 75 or 90 percent by
/// level; on success request two or three devices of the warning's class.
pub fn countermeasure_gate(
    experience: Experience,
    seeker: SeekerClass,
    random: &mut DecisionRandom,
) -> Option<DeviceRelease> {
    if !random
        .site("countermeasure roll")
        .chance(COUNTERMEASURE_THRESHOLDS[experience.index()])
    {
        return None;
    }
    let count = MIN_DEVICE_COUNT
        + random
            .site("countermeasure count")
            .below(u32::from(MAX_DEVICE_COUNT - MIN_DEVICE_COUNT) + 1) as u8;
    Some(DeviceRelease {
        count,
        class: seeker,
    })
}

/// One countermeasure dispenser as the host reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DispenserStore {
    pub class: SeekerClass,
    pub count: u32,
}

/// B47: the first dispenser holding devices of the warning's class. The other
/// class is never substituted; `None` means the release stops.
pub fn select_dispenser(stores: &[DispenserStore], class: SeekerClass) -> Option<usize> {
    stores
        .iter()
        .position(|store| store.class == class && store.count > 0)
}

/// B47: take one device from a dispenser. Human aircraft with the
/// unlimited-ammunition option do not consume devices.
pub fn debit_device(store: &mut DispenserStore, unlimited_ammunition: bool) -> Result<u32> {
    if store.count == 0 {
        return Err(AiError::InvalidInput(
            "device debit from an empty dispenser",
        ));
    }
    if !unlimited_ammunition {
        store.count -= 1;
    }
    Ok(store.count)
}

/// B47: one device against one missile, `susceptibility * effectiveness / 100`
/// percent with integer truncation. Both inputs are whole percentages.
pub fn decoy_roll(
    susceptibility_percent: u8,
    effectiveness_percent: u8,
    random: &mut DecisionRandom,
) -> Result<bool> {
    if susceptibility_percent > 100 || effectiveness_percent > 100 {
        return Err(AiError::InvalidInput("decoy percentages exceed 100"));
    }
    Ok(random.site("decoy roll").chance(decoy_threshold(
        susceptibility_percent,
        effectiveness_percent,
    )))
}

/// B47: the decoy chance in whole percent, `susceptibility * effectiveness /
/// 100` with integer truncation. The player's dispensers use the same rule.
pub fn decoy_threshold(susceptibility_percent: u8, effectiveness_percent: u8) -> u8 {
    (u32::from(susceptibility_percent) * u32::from(effectiveness_percent) / 100) as u8
}

/// A missile in flight as the decoy rule sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuidingMissile {
    pub seeker: SeekerClass,
    /// The missile is guiding on the aircraft that released the device.
    pub guiding_on_releaser: bool,
    pub decoy_susceptibility_percent: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecoyOutcome {
    /// Wrong seeker class or not guiding on the releasing aircraft.
    NotEligible,
    Decoyed,
    Resisted,
}

/// B47: apply one released device to one missile. Only missiles whose seeker
/// class matches the device and which guide on the releasing aircraft roll.
/// What a decoy does to the missile's remaining flight time is open.
pub fn decoy_missile(
    missile: &GuidingMissile,
    device_class: SeekerClass,
    device_effectiveness_percent: u8,
    random: &mut DecisionRandom,
) -> Result<DecoyOutcome> {
    if missile.seeker != device_class || !missile.guiding_on_releaser {
        return Ok(DecoyOutcome::NotEligible);
    }
    let decoyed = decoy_roll(
        missile.decoy_susceptibility_percent,
        device_effectiveness_percent,
        random,
    )?;
    Ok(if decoyed {
        DecoyOutcome::Decoyed
    } else {
        DecoyOutcome::Resisted
    })
}

/// B47: the maneuver flown when the script requests nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CourseReversal {
    /// +180 or -180 degrees from the current heading.
    pub heading_offset_degrees: i16,
    pub speed: ScalarSpeed,
}

/// B47: reverse course left or right with equal probability at corner speed.
pub fn script_fallback_reversal(
    limits: &SpeedLimits,
    random: &mut DecisionRandom,
) -> CourseReversal {
    let heading_offset_degrees = if random.site("course reversal side").choose(2) == 0 {
        COURSE_REVERSAL_DEGREES
    } else {
        -COURSE_REVERSAL_DEGREES
    };
    CourseReversal {
        heading_offset_degrees,
        speed: limits.corner,
    }
}

/// B47: fighter script reasons in rank order, lowest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScriptReason {
    Idle = 0,
    Evade = 1,
    Attack = 2,
    RadarLaunch = 3,
    IrLaunch = 4,
    Hit = 5,
}
impl ScriptReason {
    pub const ALL: [Self; 6] = [
        Self::Idle,
        Self::Evade,
        Self::Attack,
        Self::RadarLaunch,
        Self::IrLaunch,
        Self::Hit,
    ];
}
impl From<SeekerClass> for ScriptReason {
    fn from(seeker: SeekerClass) -> Self {
        match seeker {
            SeekerClass::Infrared => Self::IrLaunch,
            SeekerClass::Radar => Self::RadarLaunch,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptStart {
    /// The running script continues; the new reason is not higher.
    Resume,
    /// Nothing is running, or the new reason outranks the saved one.
    Restart,
}

/// B47: a script in progress resumes when the new reason is not higher than
/// the one it was started with; a higher reason restarts it from the top.
/// What a restart does to a motion command in flight is open.
pub fn on_new_reason(saved: Option<ScriptReason>, new: ScriptReason) -> ScriptStart {
    match saved {
        Some(saved) if saved >= new => ScriptStart::Resume,
        _ => ScriptStart::Restart,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MILE_FT: f64 = 5280.0;

    fn ai(experience: Experience, state: AttackState) -> WarningTarget {
        WarningTarget::Ai { experience, state }
    }

    fn quarters(delay: Result<WarningDelay>) -> u32 {
        match delay.unwrap() {
            WarningDelay::Quarters(q) => q,
            WarningDelay::NotWarned => panic!("expected a delay"),
        }
    }

    #[test]
    fn human_target_is_warned_after_one_second() {
        assert_eq!(quarters(warning_delay(&WarningTarget::Human, 0.0)), 4);
        assert_eq!(
            quarters(warning_delay(&WarningTarget::Human, 40.0 * MILE_FT)),
            4,
            "human cap"
        );
    }

    #[test]
    fn novice_ten_miles_ordinary_flight_is_seventeen_seconds() {
        let target = ai(Experience::Novice, AttackState::OrdinaryFlight);
        assert_eq!(quarters(warning_delay(&target, 10.0 * MILE_FT)), 17 * 4);
    }

    #[test]
    fn ace_two_miles_ordinary_flight_is_seven_seconds() {
        let target = ai(Experience::Ace, AttackState::OrdinaryFlight);
        assert_eq!(quarters(warning_delay(&target, 2.0 * MILE_FT)), 7 * 4);
        assert_eq!(
            quarters(warning_delay(&target, 2.0 * MILE_FT - 1.0)),
            6 * 4,
            "whole two-mile steps only"
        );
    }

    #[test]
    fn distance_term_caps_at_twenty_seconds() {
        let target = ai(Experience::Ace, AttackState::OrdinaryFlight);
        assert_eq!(
            quarters(warning_delay(&target, 40.0 * MILE_FT)),
            (6 + 20) * 4
        );
        assert_eq!(
            quarters(warning_delay(&target, 400.0 * MILE_FT)),
            (6 + 20) * 4
        );
    }

    #[test]
    fn attack_states_shorten_the_delay() {
        let engaging = ai(
            Experience::Ace,
            AttackState::AttackState {
                engaging_launcher: true,
            },
        );
        let other = ai(
            Experience::Ace,
            AttackState::AttackState {
                engaging_launcher: false,
            },
        );
        assert_eq!(quarters(warning_delay(&engaging, 0.0)), 4);
        assert_eq!(quarters(warning_delay(&other, 0.0)), 12);
        let novice_engaging = ai(
            Experience::Novice,
            AttackState::AttackState {
                engaging_launcher: true,
            },
        );
        assert_eq!(quarters(warning_delay(&novice_engaging, 0.0)), 4 + 24);
    }

    #[test]
    fn delay_floor_and_non_aircraft() {
        assert_eq!(
            quarters(warning_delay(&WarningTarget::Human, 0.0)),
            4.max(MIN_DELAY_QUARTERS)
        );
        assert_eq!(
            warning_delay(&WarningTarget::NotAircraft, 100.0),
            Ok(WarningDelay::NotWarned)
        );
        assert!(warning_delay(&WarningTarget::Human, -1.0).is_err());
        assert!(warning_delay(&WarningTarget::Human, f64::NAN).is_err());
    }

    fn inputs() -> WarningInputs {
        WarningInputs {
            seeker: SeekerClass::Radar,
            experience: Experience::Ace,
            flight_state: FlightState::Free,
            has_dispenser_station: true,
            launcher: Launcher {
                same_side: false,
                is_current_target: false,
            },
            hold_until: None,
            now: TimeOfDay(100),
        }
    }

    fn passing_random() -> DecisionRandom {
        // An Ace passes at 90%; find a seed whose first draw passes.
        (0..100u64)
            .map(DecisionRandom::seeded)
            .find(|r| r.clone().chance(90))
            .unwrap()
    }

    #[test]
    fn hold_time_suppresses_everything_until_reached() {
        let mut held = inputs();
        held.hold_until = Some(TimeOfDay(200));
        let outcome = receive_warning(&held, &mut passing_random());
        assert_eq!(
            outcome,
            WarningOutcome {
                approach_abandoned: false,
                devices: None,
                reaction: WarningReaction::Suppressed(Suppression::HoldTime),
            }
        );
        held.hold_until = Some(TimeOfDay(100));
        let outcome = receive_warning(&held, &mut passing_random());
        assert!(matches!(outcome.reaction, WarningReaction::Maneuver { .. }));
    }

    #[test]
    fn takeoff_and_late_landing_ignore_warnings() {
        for state in [FlightState::TakingOff, FlightState::LateLanding] {
            let mut i = inputs();
            i.flight_state = state;
            let outcome = receive_warning(&i, &mut passing_random());
            assert_eq!(outcome.reaction, WarningReaction::Ignored);
            assert_eq!(outcome.devices, None);
            assert!(!outcome.approach_abandoned);
        }
    }

    #[test]
    fn early_approach_is_abandoned_and_processing_continues() {
        let mut i = inputs();
        i.flight_state = FlightState::EarlyApproach;
        let outcome = receive_warning(&i, &mut passing_random());
        assert!(outcome.approach_abandoned);
        assert!(outcome.devices.is_some());
        assert_eq!(
            outcome.reaction,
            WarningReaction::Maneuver {
                reason: ScriptReason::RadarLaunch,
                wing_reaction: true
            }
        );
    }

    #[test]
    fn no_dispenser_station_means_no_reaction() {
        let mut i = inputs();
        i.has_dispenser_station = false;
        let outcome = receive_warning(&i, &mut passing_random());
        assert_eq!(outcome.reaction, WarningReaction::NoReaction);
        assert_eq!(outcome.devices, None);
    }

    #[test]
    fn same_side_launcher_rolls_devices_but_only_radios() {
        let mut i = inputs();
        i.launcher.same_side = true;
        i.seeker = SeekerClass::Infrared;
        let outcome = receive_warning(&i, &mut passing_random());
        assert_eq!(outcome.reaction, WarningReaction::RadioOnly);
        assert_eq!(
            outcome.devices.map(|d| d.class),
            Some(SeekerClass::Infrared)
        );
    }

    #[test]
    fn current_target_launcher_gets_no_maneuver() {
        let mut i = inputs();
        i.launcher.is_current_target = true;
        let outcome = receive_warning(&i, &mut passing_random());
        assert_eq!(outcome.reaction, WarningReaction::NoManeuver);
        assert!(outcome.devices.is_some());
    }

    #[test]
    fn otherwise_maneuver_with_the_seeker_reason() {
        let mut i = inputs();
        i.seeker = SeekerClass::Infrared;
        let outcome = receive_warning(&i, &mut DecisionRandom::seeded(1));
        assert_eq!(
            outcome.reaction,
            WarningReaction::Maneuver {
                reason: ScriptReason::IrLaunch,
                wing_reaction: true
            }
        );
    }

    #[test]
    fn countermeasure_gate_frequencies_by_level() {
        for (level, threshold) in Experience::ALL.iter().zip(COUNTERMEASURE_THRESHOLDS) {
            let mut random = DecisionRandom::seeded(11 + level.level() as u64);
            let trials = 100_000;
            let passes = (0..trials)
                .filter(|_| countermeasure_gate(*level, SeekerClass::Radar, &mut random).is_some())
                .count();
            let expected = trials * usize::from(threshold) / 100;
            assert!(
                (expected - 1000..=expected + 1000).contains(&passes),
                "{level:?}: {passes} of {trials}"
            );
        }
    }

    #[test]
    fn countermeasure_count_is_two_or_three_of_the_warning_class() {
        let mut random = DecisionRandom::seeded(5);
        let mut seen = [false; 2];
        for _ in 0..1000 {
            if let Some(release) =
                countermeasure_gate(Experience::Ace, SeekerClass::Infrared, &mut random)
            {
                assert!((2..=3).contains(&release.count));
                assert_eq!(release.class, SeekerClass::Infrared);
                seen[usize::from(release.count - 2)] = true;
            }
        }
        assert_eq!(seen, [true, true]);
    }

    #[test]
    fn dispenser_selection_skips_empties_and_never_crosses_class() {
        let stores = [
            DispenserStore {
                class: SeekerClass::Radar,
                count: 0,
            },
            DispenserStore {
                class: SeekerClass::Infrared,
                count: 4,
            },
            DispenserStore {
                class: SeekerClass::Radar,
                count: 2,
            },
        ];
        assert_eq!(select_dispenser(&stores, SeekerClass::Radar), Some(2));
        assert_eq!(select_dispenser(&stores, SeekerClass::Infrared), Some(1));
        let only_ir = &stores[..2];
        assert_eq!(select_dispenser(only_ir, SeekerClass::Radar), None);
        assert_eq!(select_dispenser(&[], SeekerClass::Infrared), None);
    }

    #[test]
    fn device_debit_respects_unlimited_option() {
        let mut store = DispenserStore {
            class: SeekerClass::Radar,
            count: 1,
        };
        assert_eq!(debit_device(&mut store, true), Ok(1));
        assert_eq!(debit_device(&mut store, false), Ok(0));
        assert!(debit_device(&mut store, false).is_err());
    }

    #[test]
    fn decoy_roll_boundaries() {
        let mut random = DecisionRandom::seeded(2);
        assert!((0..500).all(|_| !decoy_roll(0, 100, &mut random).unwrap()));
        assert!((0..500).all(|_| !decoy_roll(100, 0, &mut random).unwrap()));
        assert!((0..500).all(|_| decoy_roll(100, 100, &mut random).unwrap()));
        // 50 * 50 / 100 = 25 percent.
        let hits = (0..100_000)
            .filter(|_| decoy_roll(50, 50, &mut random).unwrap())
            .count();
        assert!((24_000..26_000).contains(&hits), "{hits}");
        // 15 * 15 / 100 truncates to 2 percent, not 2.25.
        let hits = (0..100_000)
            .filter(|_| decoy_roll(15, 15, &mut random).unwrap())
            .count();
        assert!((1_500..2_500).contains(&hits), "{hits}");
        assert!(decoy_roll(101, 10, &mut random).is_err());
    }

    #[test]
    fn decoy_only_applies_to_matching_missiles_guiding_on_releaser() {
        let mut random = DecisionRandom::seeded(3);
        let missile = GuidingMissile {
            seeker: SeekerClass::Infrared,
            guiding_on_releaser: true,
            decoy_susceptibility_percent: 100,
        };
        assert_eq!(
            decoy_missile(&missile, SeekerClass::Infrared, 100, &mut random),
            Ok(DecoyOutcome::Decoyed)
        );
        assert_eq!(
            decoy_missile(&missile, SeekerClass::Radar, 100, &mut random),
            Ok(DecoyOutcome::NotEligible)
        );
        let other_target = GuidingMissile {
            guiding_on_releaser: false,
            ..missile
        };
        assert_eq!(
            decoy_missile(&other_target, SeekerClass::Infrared, 100, &mut random),
            Ok(DecoyOutcome::NotEligible)
        );
        let immune = GuidingMissile {
            decoy_susceptibility_percent: 0,
            ..missile
        };
        assert_eq!(
            decoy_missile(&immune, SeekerClass::Infrared, 100, &mut random),
            Ok(DecoyOutcome::Resisted)
        );
    }

    #[test]
    fn fallback_reversal_is_either_direction_at_corner_speed() {
        let limits = SpeedLimits {
            minimum: ScalarSpeed(200.0),
            maximum: ScalarSpeed(1400.0),
            corner: ScalarSpeed(650.0),
        };
        let mut random = DecisionRandom::seeded(9);
        let mut left = 0;
        let mut right = 0;
        for _ in 0..10_000 {
            let reversal = script_fallback_reversal(&limits, &mut random);
            assert_eq!(reversal.speed, ScalarSpeed(650.0));
            match reversal.heading_offset_degrees {
                180 => right += 1,
                -180 => left += 1,
                other => panic!("{other}"),
            }
        }
        assert!(
            (4_500..=5_500).contains(&left),
            "{left} left, {right} right"
        );
    }

    #[test]
    fn reason_ranking_resumes_or_restarts_for_all_pairs() {
        for new in ScriptReason::ALL {
            assert_eq!(on_new_reason(None, new), ScriptStart::Restart);
            for saved in ScriptReason::ALL {
                let expected = if saved >= new {
                    ScriptStart::Resume
                } else {
                    ScriptStart::Restart
                };
                assert_eq!(
                    on_new_reason(Some(saved), new),
                    expected,
                    "{saved:?} -> {new:?}"
                );
            }
        }
        assert_eq!(
            on_new_reason(Some(ScriptReason::Hit), ScriptReason::IrLaunch),
            ScriptStart::Resume
        );
        assert_eq!(
            on_new_reason(Some(ScriptReason::Evade), ScriptReason::RadarLaunch),
            ScriptStart::Restart
        );
        assert!(ScriptReason::IrLaunch > ScriptReason::RadarLaunch);
    }
}
