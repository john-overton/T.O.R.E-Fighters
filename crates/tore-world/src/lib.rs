//! The mission core: everything that advances a mission one fixed 120 Hz tick
//! at a time, with no window, GPU or audio. `World` owns the mission state and
//! steps it; the terrain, combat, the AI wings, the radio and the Quick Mission
//! layout are its parts. The app presents what a tick reports.
//!
//! Rules for what lives here are in docs/ARCHITECTURE.md ("Mission core and
//! seats", "Rules for mission state"): no file handles, environment variables,
//! `log`, wall clock or replay types.
pub mod ai_wings;
pub mod aircraft_type;
pub mod airfield_radio;
pub mod combat;
pub mod combat_tape;
pub mod comms;
pub mod crew_voice;
pub mod mission_layout;
pub mod radio_calls;
pub mod situation;
pub mod snapshot;
pub mod target_window;
pub mod terrain;
pub mod world;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

/// A result whose error is any boxed error, as the mission code reports them.
pub type WorldResult<T> = Result<T, Box<dyn std::error::Error>>;
