//! The coder of the reason an AI aircraft is returning for damage
//! (docs/formats/checkpoint.md, stage H slice H4). `Response`, `Power` and
//! `Trace` are rebuilt from the flight state every tick or are explanations.

use super::Reason;

crate::checkpoint_enum!(Reason {
    Fire = 0,
    Pilot = 1,
    Fuel = 2,
    Engine = 3,
    Oil = 4,
    Hydraulics = 5,
    Controls = 6,
    Structure = 7,
});
