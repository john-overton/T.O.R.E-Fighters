//! The coders of an AI aircraft's incoming gunfire awareness
//! (docs/formats/checkpoint.md, stage H slice H4).
//!
//! `Round` is the host's per-tick input and `Trace` an explanation built
//! for the debug panels; neither is held between ticks. Every field of
//! `Service` is coded.

use super::{Cue, Evidence, Service};

crate::checkpoint_enum!(Evidence {
    VisualTrajectory = 0,
    ClosePass = 1,
    Hit = 2,
});

crate::checkpoint_struct!(Cue {
    evidence,
    round,
    observed_tick,
    time_to_danger_s,
    bearing_world_deg,
});

crate::checkpoint_struct!(Service {
    samples,
    cue,
    pending_hit,
    episode,
});
