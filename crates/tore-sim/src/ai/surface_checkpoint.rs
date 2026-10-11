//! The coders of the surface engagement controller (docs/formats/checkpoint.md):
//! its phase, target, clocks and the burst in progress. Profiles, inputs and
//! outcomes live for one call or are rebuilt from the unit's records.

use super::{Burst, Controller, Phase};

crate::checkpoint_enum!(Phase {
    Idle = 0,
    Search = 1,
    Prepare = 2,
    Track = 3,
    Fire = 4,
    Pause = 5,
    Reload = 6,
    Empty = 7,
    Blind = 8,
});

crate::checkpoint_struct!(Burst {
    start,
    rounds,
    released,
    span,
    opening,
});

crate::checkpoint_struct!(Controller {
    phase,
    target,
    deadline,
    lock_since,
    gates_failed_since,
    last_hostile,
    engaged,
    opened,
    retarget_at,
    burst,
});
