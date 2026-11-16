//! The Tu-95 Bear-H's fitted flight terms (slice T95-8, 2026-11-16).
//!
//! The record (`TU95.PT`) is an authored one on the shared large-aircraft
//! family, and BMB-C2 gave it the real weights and a fitted thrust (the
//! retail 355,080 lbf was four times 14,795 shp times six). The shared drag
//! law ties the clean drag to the thrust at the 1 G top-speed edge as a
//! square law, which for a loaded Bear at 0.96 of the sea-level edge gave a
//! 295 kt top speed at sea level, a 1,500 ft/min best climb and a ceiling of
//! 9,600 ft in the bomber climb schedule: the full-power drag was set by the
//! edge and the weight, not by what the aircraft needs. The terms below are
//! all **fitted** agent decisions, each against the figure named with it
//! (the game's own Jane's text `TU95.INF` and public figures, listed in
//! docs/formats/aircraft-variety.md, "Tu-95 Bear-H flight review"); the G
//! rows, the record's weights and the fuel flow are untouched, and the 1 G
//! edge gains one vertex in `aircraft_corrections.rs`.
//!
//! The model has no thrust that falls with speed. A turboprop's does (the
//! real thrust is about 80,000 lbf at the 162 kt takeoff speed and 43,000
//! at 355 kt), so one scalar stands for takeoff and cruise: the thrust is
//! fitted for the climb and the drag shape for the cruise, and the
//! takeoff run takes its own rolling term.
use crate::models::{Tuning, config::Configuration, config::DragShape};

/// Share of the reference drag that follows the square law (fitted).
pub const QUADRATIC_SHARE: f64 = 0.56;
/// Exponent of the rest of the drag (fitted). With the share above the drag is
/// 1 at the edge, about 0.39 of the thrust at 0.8 of it (400 kt at 25,000
/// ft) and about 0.5 at 0.87.
pub const WALL_EXPONENT: f64 = 12.;
/// The thrust lapse height, ft (fitted; the shared value is 70,000). A
/// turboprop's power follows the air density about as its 0.8 power, so it
/// keeps about 0.5 at 25,000 ft; `exp(-h / 38,000)` is 0.52 there.
pub const THRUST_LAPSE_FEET: f64 = 38_000.;
/// Rolling and takeoff-run deceleration, ft/s squared (fitted; the shared value
/// is 0.8). It carries the flap and gear drag and the thrust the propellers
/// lose with speed, which the shared takeoff lacks, to give a run near the
/// published 2,540 m (8,333 ft) at the maximum weight.
pub const ROLLING_DECELERATION: f64 = 4.8;
/// Wheel brake deceleration, ft/s squared (fitted; the shared value is 18).
/// The published landing run is 1,700 m (5,577 ft) at the 135 t maximum
/// landing weight.
pub const BRAKE_DECELERATION: f64 = 1.0;

/// Applies the Tu-95's terms to a configuration built from its record.
pub(super) fn fit(tuning: &mut Tuning, configuration: &mut Configuration) {
    tuning.thrust_lapse_feet = THRUST_LAPSE_FEET;
    tuning.rolling_deceleration = ROLLING_DECELERATION;
    tuning.brake_deceleration = BRAKE_DECELERATION;
    configuration.tuning = *tuning;
    configuration.aerodynamics.drag_shape = Some(DragShape {
        quadratic_share: QUADRATIC_SHARE,
        wall_exponent: WALL_EXPONENT,
    });
}
