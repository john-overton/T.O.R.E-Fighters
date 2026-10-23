//! The coders of an AI aircraft's missile-defense state and its last
//! decision (docs/formats/checkpoint.md, stage H slice H4).
//!
//! `DefenseOwn` is the caller's per-tick input and is not state. A
//! `DefenseDecision` is held as the actor's last decision, which the next
//! tick reads (whether it flew a motion), so it is coded whole. Its `debug`
//! explanation is write-only, but the type has no empty value to rebuild it
//! from and costs a few dozen bits, so it is coded too and a restored
//! decision compares equal.

use super::{
    BurstRequest, DefenseDebug, DefenseDecision, DefenseState, Maneuver, MotionSuggestion,
};

crate::checkpoint_enum!(Maneuver {
    Jink = 0,
    Notch = 1,
});

crate::checkpoint_struct!(MotionSuggestion {
    maneuver,
    heading_deg,
    flight_path_pitch_deg,
});

crate::checkpoint_struct!(BurstRequest { chaff, flares });

crate::checkpoint_struct!(DefenseDebug {
    estimated_threat_time_s,
    estimated_maneuver_time_s,
    margin_s,
    preferred,
    heading_deg,
    flight_path_pitch_deg,
    dive_safe,
    new_threat,
    bearing_only,
    uncertain_directed_radar,
    insufficient_time,
    stale,
    maneuver_now,
    may_burst,
    release_now,
});

crate::checkpoint_struct!(DefenseDecision {
    threat_id,
    motion,
    burst,
    debug,
});

crate::checkpoint_struct!(DefenseState {
    selected_threat,
    jink_base_heading_deg,
    jink_heading_deg,
    jink_started_tick,
    last_burst_tick,
});
