//! The coder of the script reason a controller runs under (B47,
//! docs/formats/checkpoint.md). The launch warning, countermeasure and decoy
//! types in the module are inputs and outputs of one call and are not kept;
//! the threat reports, seeker classes and dispenser stores that controllers
//! do keep are leaf types coded in `checkpoint_shared.rs`.

use super::ScriptReason;

crate::checkpoint_enum!(ScriptReason {
    Idle = 0,
    Evade = 1,
    Attack = 2,
    RadarLaunch = 3,
    IrLaunch = 4,
    Hit = 5,
});
