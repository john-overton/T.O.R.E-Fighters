//! The B-1B's fitted flight term (slice B1B-8, 2026-11-16).
//!
//! `B1.PT` is the game's own authored record on the shared flight-tuning
//! block, and BMB-B2 gave it the real thrust, weights and fuel, and B1B-8 the
//! real fuel flows (11 lb/s dry, 84 with afterburner, from the F101-GE-102's
//! thrust-specific fuel consumption). With the retail 6 and 16 lb/s the record
//! flew about 6,800 nautical miles at Mach 0.8 and 36,000 ft; the real flows
//! take that to 3,700 against the published 5,100 (Wikipedia "Rockwell B-1
//! Lancer", specification, Pace 1998) because the shared drag law ties the
//! clean drag to the afterburner thrust at the top-speed edge: at Mach 0.8
//! and 36,000 ft it asks for about 39,000 lbf of a 326,000 lb aircraft whose
//! lift-to-drag ratio of about 12 needs 27,000. The term below is **fitted**
//! (agent decision, 2026-11-16) and changes nothing at the edge, so the top
//! speeds, the G rows, the thrust, weights and fuel are untouched
//! (docs/formats/aircraft-variety.md, "B-1B Lancer flight review").
use crate::models::config::{Configuration, DragShape};

/// Share of the reference drag that follows the square law (fitted). With
/// [`WALL_EXPONENT`] the drag is 1 at the edge speed (the reference speed is
/// Mach 1.25 at altitude and Mach 0.92 at sea level) and, at 0.726 of it (520
/// kt, Mach 0.80 at 36,000 ft), 0.38 of the reference: 28,000 lbf, the
/// throttle that holds the speed is 0.68 and burns 7.5 lb/s, the 5,100
/// nautical miles of the published range from the full 265,274 lb tank.
pub const QUADRATIC_SHARE: f64 = 0.72;
/// Exponent of the remaining share (the B-52's, B52-8).
pub const WALL_EXPONENT: f64 = 12.;

/// Applies the B-1B's drag shape to a configuration built from its record.
pub(super) fn fit(configuration: &mut Configuration) {
    configuration.aerodynamics.drag_shape = Some(DragShape {
        quadratic_share: QUADRATIC_SHARE,
        wall_exponent: WALL_EXPONENT,
    });
}
