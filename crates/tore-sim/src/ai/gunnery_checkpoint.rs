//! The coders of the AI's gun employment state (docs/formats/checkpoint.md):
//! the aim and firing solution a controller holds while it tracks a target
//! with a gun, the gun's view of its station, and the burst and recovery
//! cycle of each gun station. The gunner's `Target` is a leaf type coded in
//! `checkpoint_shared.rs`; `Trace` is a why-record (it exists only inside a
//! controller's write-only trace), so it has no coder.

use super::{Aim, Cycle, Solution, View};

crate::checkpoint_struct!(Aim {
    direction,
    heading_rate,
    pitch_rate,
});

crate::checkpoint_struct!(Solution {
    aim,
    aligned,
    miss_ft,
    seconds,
    range_ft,
});

crate::checkpoint_struct!(View {
    station,
    target,
    solution,
    rounds,
    period_ticks,
    target_speed,
    ammunition,
});

// `Cycle::advance` reads all three fields on the next tick.
crate::checkpoint_struct!(Cycle {
    burst,
    recovery_until,
    next_scaled,
});
