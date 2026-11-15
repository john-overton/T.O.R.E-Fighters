//! The B-52H's fitted flight terms (slice B52-8, 2026-11-15).
//!
//! The record (`B52.PT`) is an authored one on the shared large-aircraft
//! family (169 of 207 statements equal the AC-130's), and BMB-A2 gave it the
//! real thrust, weights, fuel and flow. The shared drag law ties the clean
//! drag to the thrust at the 1 G top-speed edge, and the B-52's low-altitude
//! edge (385 kt at sea level, 440 kt at 15,000 ft) is a placard, not the
//! speed the thrust reaches, so at 300 kt the law gave about 3.5 times the
//! real drag: a 2,300 ft/min climb against the published 6,270 ft/min and
//! about a third of the published range. The terms below are all **fitted**
//! agent decisions, each against the published figure named with it; the
//! top-speed edges, the G rows, the record's thrust, weights and fuel are
//! untouched (docs/formats/aircraft-variety.md, "B-52 flight review").
use crate::models::{Tuning, config::Configuration, config::DragShape};

/// Share of the reference drag that follows the square law (fitted). With
/// [`WALL_EXPONENT`] the drag is 1 at the edge speed, 0.24 of the thrust at
/// 0.857 of it (Mach 0.77 at 36,000 ft, a 0.26 throttle holds that speed and
/// burns about 4.7 lb/s, the 7,600 nautical mile combat range the published
/// 442 kt cruise implies) and 0.13 at 0.78 of it (300 kt at sea level, about
/// 18,000 lbf, which gives the published 6,270 ft/min climb within 40 percent
/// at 400,000 lb; the induced drag a real 400,000 lb aircraft adds at low
/// speed, about 10,000 lbf more, is not modelled).
pub const QUADRATIC_SHARE: f64 = 0.15;
pub const WALL_EXPONENT: f64 = 12.;
/// The thrust lapse height, ft (fitted; the shared value is 70,000). The
/// TF33-P-3 keeps about 0.3 of its sea-level thrust at 36,000 ft and Mach
/// 0.8; the shared curve keeps 0.6. 45,000 ft gives 0.45 there, which is
/// what the drag shape above needs to land the cruise drag against the
/// thrust.
pub const THRUST_LAPSE_FEET: f64 = 32_000.;
/// Rolling and takeoff-run deceleration, ft/s squared (fitted; the shared
/// value is 0.8). It carries the flap, gear and induced drag and the thrust
/// the engines lose with speed, which the shared takeoff has none of, to
/// give the roughly 8,000 ft ground run at 488,000 lb that a pilot forum
/// estimate states (an unofficial figure; the spec-sheet takeoff distance is
/// 9,843 ft).
pub const ROLLING_DECELERATION: f64 = 4.8;
/// Wheel brake deceleration, ft/s squared (fitted; the shared value is 18,
/// which stops a heavy from 140 kt in about 1,500 ft). The same forum
/// figures give a 4,900 ft landing ground run from a 135 kt flare at
/// 270,000 lb with brakes alone (4,100 ft with the drag chute), a mean of
/// 5.3 ft/s squared.
pub const BRAKE_DECELERATION: f64 = 2.6;

/// Stall reference fraction (fitted; every other variety aircraft has 1). The
/// weight-scaled stall speed takes the empty weight times this as its
/// reference. With the real empty weight (BMB-A2) the shared 1 gave a flapped
/// stall of 152 kt at 488,000 lb and, at 270,000 lb, about 113 kt; a pilot
/// forum estimate gives a 135 kt flare speed at 270,000 lb (unofficial), a
/// flapped stall near 108 kt at 1.25 times, and 1.09 puts the model there.
pub const STALL_REFERENCE_FRACTION: f64 = 1.09;

/// Applies the B-52's terms to a configuration built from its record.
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
