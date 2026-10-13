//! The coders of the AI's rotorcraft and jet-lift control memory
//! (docs/formats/checkpoint.md, slice H1): the laws' integrators
//! (`autopilot::Loops`, coded beside it) and the rotor adapter's regime.

use super::jet_lift::JetLift;
use super::rotor_adapter::{Regime, RotorAdapter};

crate::checkpoint_enum!(Regime {
    Forward = 0,
    Pointing = 1,
    Hover = 2,
});

crate::checkpoint_struct!(RotorAdapter { loops, regime });

crate::checkpoint_struct!(JetLift { loops });
