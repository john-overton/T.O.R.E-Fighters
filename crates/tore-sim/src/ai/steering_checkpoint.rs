//! The coder of the AI's steering command mode (docs/formats/checkpoint.md).
//! The rest of the steering module is stateless: a request, a state value and
//! the rates are all passed in each tick and none is kept between ticks, so
//! `SteeringState`, `SteeringRequest` and `AxisRates` hold nothing a
//! checkpoint needs. Only the controller's motion intent keeps a mode.

use super::CommandMode;

crate::checkpoint_enum!(CommandMode {
    Ordinary = 0,
    ReducedRate = 1,
    OtherState = 2,
});
