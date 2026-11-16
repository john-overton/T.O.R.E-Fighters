//! The Tu-26 Backfire-C's fitted flight terms (slice T26-8, 2026-11-16).
//!
//! The record (`TU26.PT`) is the game's own authored one on the shared
//! large-aircraft family (170 of 207 statements equal the Tu-95's), and BMB-C2
//! gave it the real thrust (66,000 / 110,000 lbf), empty weight and edges (top
//! speed 1,080 kt, ceiling 43,635 ft). What the shared takeoff and landing
//! contact law gets wrong for a 273,000 lb bomber is the ground run: it has no
//! flap, gear or induced drag and no loss of thrust with speed, and its wheel
//! brakes stop a heavy from 140 kt in about 1,500 ft. The game's own Jane's
//! text (`TU26.INF`, reference media) gives a take-off run of 2,100 m (6,890
//! ft) at the 124,000 kg maximum weight and a landing run of 1,300 m (4,265 ft),
//! against 3,900 ft and about 2,200 ft in the model. The two terms below are
//! **fitted** agent decisions; nothing else about the flight is changed (the
//! climb, the cruise burn, the top speeds and the turns already sit near the
//! real figures, docs/formats/aircraft-variety.md, "Tu-26 Backfire-C flight
//! review").
use crate::models::{Tuning, config::Configuration};

/// Rolling and takeoff-run deceleration, ft/s squared (fitted; the shared
/// value is 0.8). It carries the flap, gear and induced drag and the thrust the
/// engines lose with speed, which the shared takeoff has none of, to give the
/// published 6,890 ft take-off run at the 273,370 lb maximum weight.
pub const ROLLING_DECELERATION: f64 = 6.0;
/// Wheel brake deceleration, ft/s squared (fitted; the shared value is 18).
/// Jane's 4,265 ft landing run from the 154 kt landing speed, a mean of 8.0
/// ft/s squared once the airbrake and the rolling term are counted.
pub const BRAKE_DECELERATION: f64 = 6.;

/// Applies the Tu-26's terms to a configuration built from its record.
pub(super) fn fit(tuning: &mut Tuning, configuration: &mut Configuration) {
    tuning.rolling_deceleration = ROLLING_DECELERATION;
    tuning.brake_deceleration = BRAKE_DECELERATION;
    configuration.tuning = *tuning;
}
