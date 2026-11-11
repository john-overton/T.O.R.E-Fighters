//! One scale per aircraft for its shape: how many times larger than the
//! source drawing it is built in the aircraft renderer, per axis.
//!
//! Every aircraft shape draws at the aircraft convention of one third of a foot
//! per source unit (`tore_sim::combat::debris::scale` holds that scalar).
//! This table adds a per-axis multiplier on top of it, `[x, y, z]` in shape
//! units (x right, y forward, z up). It is `[1, 1, 1]` for every aircraft but
//! the Mirage 2000 (M2K-2) and the Mirage F1 (MF1-2), docs/formats/objects-and-shapes.md,
//! "Aircraft shape scale". The factors are exact constants, never read from a device, so every
//! multiplayer peer holds the same values.
//!
//! Apply the factors to the shape's vertices when the shape is loaded
//! ([`crate::shape::Shape::scaled`]) so animation rigs turn already scaled
//! geometry and panels stay rigid. A rig that names a hinge point in shape
//! units scales that point with [`scale_point`] (or reads it off the scaled
//! geometry). A distance a record gives in shape units (a station row, a
//! gear low point) scales with the matching axis.
use super::AircraftId;

/// No scale: the shape as drawn at the aircraft convention.
pub const IDENTITY: [f64; 3] = [1.; 3];

/// Measured `m20.SH` extents with the gear down, shape units
/// (`docs/formats/aircraft-variety.md`, Mirage 2000 source review): x -33 to
/// 33, y -44 to 71, z -12 to 22.
pub const M2000_SHAPE_EXTENT: [f64; 3] = [66., 115., 34.];

/// The real Mirage 2000 (2000C) in feet: wingspan 9.13 m, length 14.36 m,
/// height 5.20 m ground to fin tip (MILAVIA Mirage 2000 specifications;
/// Deagel Mirage 2000 gives 5.2 m height; Dassault Aviation's data sheet gives
/// 29.9 ft span and 47 ft length; accessed 2026-11-09). Other sources round
/// the length and height differently (14.55 m and 5.15 m on MILAVIA's main
/// page, 5.30 m at FAS), a difference under 3 percent.
pub const M2000_REAL_FEET: [f64; 3] = [29.954, 47.113, 17.060];

/// `[x, y, z]` factors that bring `m20.SH` to [`M2000_REAL_FEET`]:
/// `real_ft * 3 / extent_units`, rounded to four places. **Fitted** (agent
/// derivation) for an **opinionated** choice (John, 2026-11-09, "Per axis to
/// real", over uniform scaling). x 1.3616 (span), y 1.2290 (length), z 1.5053
/// (height, so the 12 unit gear low point becomes 18.06 units, 6.02 ft).
pub const M2000_SHAPE_SCALE: [f64; 3] = [1.3616, 1.2290, 1.5053];

/// Measured `mf1.SH` extents with the gear down (gear word `0x5adc`), shape
/// units (`docs/formats/aircraft-variety.md`, Mirage F1 source review): x -40
/// to 40 (the wingtip rails included; the wing alone is 78), y -45 to 82 (the
/// nose probe tip included; the radome ends at y 68), z -17 to 25.
pub const MF1_SHAPE_EXTENT: [f64; 3] = [80., 127., 42.];

/// The real Mirage F1C in feet: wingspan 8.40 m, length 15.23 m **with the
/// nose probe** (the spike on the radome, so the length is matched to the
/// shape's own extent including its spike), height 4.50 m ground to fin tip
/// (Jane's text in the game's own `MF1.INF` and MILAVIA's Mirage F1
/// specifications give 8.40 m and 15.23 m; Aerospaceweb gives 15.30 m and
/// 4.50 m; accessed 2026-11-10). The length to the radome tip alone is about
/// 11.5 m, which no source states, so it is not used.
pub const MF1_REAL_FEET: [f64; 3] = [27.559, 49.967, 14.764];

/// `[x, y, z]` factors that bring `mf1.SH` to [`MF1_REAL_FEET`]:
/// `real_ft * 3 / extent_units`, rounded to four places. **Fitted** (agent
/// derivation) for an **opinionated** choice (John, 2026-11-10, per-axis scale
/// to the real span, length and height, as the Mirage 2000). x 1.0335 (span),
/// y 1.1804 (length with the probe), z 1.0546 (height, so the 17 unit gear low
/// point becomes 17.93 units, 5.98 ft).
pub const MF1_SHAPE_SCALE: [f64; 3] = [1.0335, 1.1804, 1.0546];

/// `[x, y, z]` factors for `id`'s shape, in shape-unit axes (x right, y
/// forward, z up). `[1, 1, 1]` for every aircraft but the Mirage 2000 and the
/// Mirage F1.
pub fn shape_axis_scale(id: AircraftId) -> [f64; 3] {
    match id {
        AircraftId::M2000 => M2000_SHAPE_SCALE,
        AircraftId::Mf1 => MF1_SHAPE_SCALE,
        _ => IDENTITY,
    }
}

/// [`shape_axis_scale`] as the `f32` factors shape vertices are multiplied by,
/// rounded once here so every reader and every peer scales to the same bits.
pub fn shape_axis_scale_f32(id: AircraftId) -> [f32; 3] {
    shape_axis_scale(id).map(|v| v as f32)
}

/// The same factors in the order a hardpoint record, a CE vector or a world
/// offset uses: right, up, forward.
pub fn right_up_forward(id: AircraftId) -> [f64; 3] {
    let [x, y, z] = shape_axis_scale(id);
    [x, z, y]
}

/// Whether `id`'s shape is drawn at the plain aircraft convention.
pub fn is_identity(id: AircraftId) -> bool {
    shape_axis_scale(id) == IDENTITY
}

/// A point in shape units (x, y, z) scaled for `id`. For a rig's hinge or axis
/// point named in shape units.
pub fn scale_point(id: AircraftId, point: [f32; 3]) -> [f32; 3] {
    let scale = shape_axis_scale(id);
    std::array::from_fn(|i| point[i] * scale[i] as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_aircraft_but_the_two_mirages_keeps_the_plain_shape_scale() {
        for id in AircraftId::ALL.into_iter().chain(AircraftId::SELECTABLE) {
            if id == AircraftId::M2000 {
                assert_eq!(shape_axis_scale(id), M2000_SHAPE_SCALE);
                assert!(!is_identity(id));
            } else if id == AircraftId::Mf1 {
                assert_eq!(shape_axis_scale(id), MF1_SHAPE_SCALE);
                assert!(!is_identity(id));
            } else {
                assert_eq!(shape_axis_scale(id), IDENTITY, "{id:?}");
                assert!(is_identity(id));
            }
        }
        // The exact record, desert variant and Rafale are never aliased to it.
        assert_eq!(shape_axis_scale(AircraftId::Rafale), IDENTITY);
    }

    #[test]
    fn the_mirage_f1_factors_bring_the_shape_to_its_real_size() {
        for axis in 0..3 {
            let feet = MF1_SHAPE_EXTENT[axis] / 3. * MF1_SHAPE_SCALE[axis];
            assert!(
                (feet - MF1_REAL_FEET[axis]).abs() / MF1_REAL_FEET[axis] < 0.0005,
                "axis {axis}: {feet} against {}",
                MF1_REAL_FEET[axis]
            );
        }
        // Span 8.40 m, length 15.23 m, height 4.50 m.
        for (feet, meters) in MF1_REAL_FEET.iter().zip([8.40, 15.23, 4.50]) {
            assert!((feet - meters / 0.3048).abs() < 0.001);
        }
        // Per axis, never uniform.
        assert_ne!(MF1_SHAPE_SCALE[0], MF1_SHAPE_SCALE[1]);
        assert_ne!(MF1_SHAPE_SCALE[1], MF1_SHAPE_SCALE[2]);
    }

    #[test]
    fn the_mirage_2000_factors_bring_the_shape_to_its_real_size() {
        for axis in 0..3 {
            let feet = M2000_SHAPE_EXTENT[axis] / 3. * M2000_SHAPE_SCALE[axis];
            assert!(
                (feet - M2000_REAL_FEET[axis]).abs() / M2000_REAL_FEET[axis] < 0.0005,
                "axis {axis}: {feet} against {}",
                M2000_REAL_FEET[axis]
            );
        }
        // Span 9.13 m, length 14.36 m, height 5.20 m.
        for (feet, meters) in M2000_REAL_FEET.iter().zip([9.13, 14.36, 5.20]) {
            assert!((feet - meters / 0.3048).abs() < 0.001);
        }
    }

    #[test]
    fn the_orders_agree_and_points_scale_per_axis() {
        let [x, y, z] = M2000_SHAPE_SCALE;
        assert_eq!(right_up_forward(AircraftId::M2000), [x, z, y]);
        assert_eq!(
            scale_point(AircraftId::M2000, [10., 20., -12.]),
            [10. * x as f32, 20. * y as f32, -12. * z as f32]
        );
        assert_eq!(scale_point(AircraftId::F16C, [1., 2., 3.]), [1., 2., 3.]);
        let [x, y, z] = MF1_SHAPE_SCALE;
        assert_eq!(right_up_forward(AircraftId::Mf1), [x, z, y]);
        assert_eq!(
            scale_point(AircraftId::Mf1, [10., 20., -17.]),
            [10. * x as f32, 20. * y as f32, -17. * z as f32]
        );
    }
}
