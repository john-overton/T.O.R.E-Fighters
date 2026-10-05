//! The coders of the AI's formation guidance (docs/formats/checkpoint.md): the
//! phase machine a wingman flies to its slot, with its repositioning plan.
//!
//! `Guidance::trace` is named a "hidden inspection hook", but it is not a
//! why-record: `Guidance::change_slot` reads whether it is set to tell a
//! guidance that has stepped from one that has not, and the mission copies
//! each actor's trace phase and planned velocity into the other actors'
//! same-tick traffic. So it is coded, with its `Trace`.
//!
//! `Traffic` is the same-tick observation a controller is handed; the
//! controller's copy of it is scratch (see `controller_checkpoint.rs`), but
//! its coder is here for whoever holds a list of it.

use super::{Guidance, Phase, Reposition, Trace, Traffic};

crate::checkpoint_enum!(Phase {
    Close = 0,
    Trail = 1,
    Breakout = 2,
    Intercept = 3,
    Stabilize = 4,
    Capture = 5,
    Reposition = 6,
});

crate::checkpoint_struct!(Traffic {
    id,
    position,
    velocity,
    phase,
    planned_velocity,
});

crate::checkpoint_struct!(Trace {
    phase,
    phase_seconds,
    slot_distance_ft,
    closure_fps,
    altitude_error_ft,
    minimum_predicted_separation_ft,
    yielding_to,
    aim,
    planned_velocity,
});

crate::checkpoint_struct!(Reposition {
    origin,
    aft,
    stage,
    hold,
});

crate::checkpoint_struct!(Guidance {
    phase,
    phase_seconds,
    side,
    capture,
    last_heading,
    trace,
    reposition,
});
