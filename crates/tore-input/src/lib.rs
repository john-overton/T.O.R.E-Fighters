//! Hand-rolled physical controls, bindings and pilot frames; no OS or renderer.
pub mod pilot;
pub use pilot::{
    FlightAxis, LiftCommand, NozzlePreset, PilotCommand, PilotInput, StabilityLevel, Switch,
    TrimAxis, trim_keys,
};
pub mod bindings;
pub use bindings::{
    Action, Axis, Binding, Calibration, Event, LONG_PRESS_TICKS, Mode, Profile, Resolver,
    chord_parts, token_base, token_value,
};
pub mod sight;
pub use sight::SightSlew;
pub mod feedback;
pub mod recording;
#[cfg(test)]
mod tests;
pub use feedback::{FeedbackEvent, FeedbackMixer, FeedbackUpdate};

pub mod profile_text;
