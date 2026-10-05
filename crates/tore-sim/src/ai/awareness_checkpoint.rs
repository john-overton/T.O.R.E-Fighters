//! The coders of an AI aircraft's visual awareness: its lookout and its
//! memory of observed aircraft (docs/formats/checkpoint.md, stage H slice
//! H4).
//!
//! Every field of every stored type is coded. `VisualResult`, `VisualTrace`
//! and `Observation` are per-call values and never held between ticks.

use super::{Lookout, Memory, ObservationSource, Snapshot, SourceTimestamps};

crate::checkpoint_struct!(Lookout {
    position,
    forward,
    scan,
    attention,
    sector,
});

crate::checkpoint_enum!(ObservationSource {
    Visual = 0,
    Radar = 1,
    Infrared = 2,
    Fixture = 3,
});

crate::checkpoint_struct!(SourceTimestamps {
    visual,
    radar,
    infrared,
    fixture,
});

crate::checkpoint_struct!(Snapshot {
    target,
    velocity,
    first_observed_tick,
    last_observed_tick,
    source_ticks,
});

crate::checkpoint_struct!(Memory {
    experience,
    own_side,
    current,
    remembered,
    novice_target,
});
