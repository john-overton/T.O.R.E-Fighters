//! The Tu-160's fitted flight terms (slice T160-8, 2026-11-16).
//!
//! The record (`TU160.PT`) is the Tu-26's with 35 values changed, and BMB-B2
//! gave it the real weights, thrust and fuel. T160-8 gave it the real top-speed
//! edges (556 kt at sea level, 1,200 kt at 40,000 ft: a dynamic-pressure limit
//! low down, a Mach limit up high, `aircraft_corrections.rs`) and the NK-32
//! fuel flows. The shared drag law ties the clean drag to the thrust at the
//! 1 G top-speed edge, so with the sea-level edge at a placard (the real jet
//! reaches 556 kt on a fraction of its thrust) the shared square law would give
//! a climb of 5,000 ft/min against the published 14,000. The terms below are
//! all **fitted** agent decisions, each against the published figure named with
//! it; the G rows, the record's thrust, weights and fuel are untouched
//! (docs/formats/aircraft-variety.md, "Tu-160 flight review").
use crate::models::{Tuning, config::Configuration, config::DragShape};

/// Share of the reference drag that follows the square law (fitted); the
/// rest is the steep rise to 1 at the edge ([`WALL_EXPONENT`]). Fitted between
/// the published initial climb of 13,780 ft/min at sea level (`TU160.INF`) and
/// the 6,640 nautical mile range (`TU160.INF`): 0.35 gave a climb 5 percent
/// higher and a range of 8,361 nautical miles at Mach 0.9 and 45,000 ft, 0.45
/// the published range and a climb 5 percent lower; 0.40 splits them (7,388
/// nautical miles, 14,600 ft/min of peak excess power at a typical 450,000 lb).
pub const QUADRATIC_SHARE: f64 = 0.4;
pub const WALL_EXPONENT: f64 = 12.;

/// Stall reference fraction (fitted; every other variety aircraft has 1 and
/// the B-52H 1.09). The weight-scaled stall speed takes the empty weight times
/// this as its reference. With the real empty weight (BMB-B2) the shared 1
/// gave a flapped stall of 171 kt at the 600,532 lb default load, a liftoff at
/// 252 kt and an approach that could not fly at its 166 kt: the scripted
/// landing crashed at 207 kt with a 100 ft/s sink. The published wing is 4,306
/// square feet (400 square metres) at 20 degrees: at 600,000 lb a flapped
/// lift coefficient of 2.2 stalls at 137 kt, so a liftoff near 160 to 175 kt.
pub const STALL_REFERENCE_FRACTION: f64 = 2.3;

// The rolling deceleration (the takeoff run) is not fitted: the model's 4,072 ft
// run at 600,532 lb in full afterburner is what the thrust to weight gives, and
// the published 2,200 m (7,218 ft, weight not stated) and 900 m disagree by more
// than the gap (docs/formats/aircraft-variety.md, "Tu-160 flight review").

/// Wheel brake deceleration, ft/s squared (fitted; the shared value is 18, which
/// stopped the aircraft in 573 m, 0.56 G). Fitted to the published landing
/// distance of 1,600 m (5,250 ft, Aerocorner, weight not stated); 4.5 gives a
/// 1,250 m roll from the touchdown at 486,000 lb.
pub const BRAKE_DECELERATION: f64 = 4.5;

/// Applies the Tu-160's terms to a configuration built from its record.
pub(super) fn fit(tuning: &mut Tuning, configuration: &mut Configuration) {
    tuning.brake_deceleration = BRAKE_DECELERATION;
    configuration.tuning = *tuning;
    configuration.aerodynamics.drag_shape = Some(DragShape {
        quadratic_share: QUADRATIC_SHARE,
        wall_exponent: WALL_EXPONENT,
    });
}
