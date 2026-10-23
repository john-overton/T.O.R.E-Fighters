//! Where a land gun's barrel ends (docs/spec/surface-defenses.md, "Muzzles").
//!
//! Every NT places a land gun's hardpoint at the same spot, `[0, 30, 0]`: 10
//! ft above the unit's origin, at its middle. A flash and a shell that start
//! there come out of the middle of the gun (John, 2026-10-13: "the flash
//! comes from the center of the gun instead of the end of the barrel"; the
//! KS-12 and KS-19 showed it most). The shape knows better: its barrel is
//! drawn out to a tip. [`TIPS`] lists that tip for each land gun's shape,
//! read from the shape's faces (the end of the barrel, as the scenery pose
//! draws it), in shape units as `[right, forward, up]` about the shape's
//! origin; [`barrel_ft`] turns it into world feet from the unit's ground
//! point, `[right, up, forward]`, with the shape's drawn scale and its
//! [grounding](crate::grounding) (the shape stands on its lowest point).
//!
//! Fitted (agent, 2026-10-13): the tips are read by eye from the faces and
//! the drawn barrel is fixed in the shape (a land gun turns about its up
//! axis but its barrel does not elevate), so a shell leaves from the drawn
//! tip along the direction the gun aims.
use crate::grounding;
use crate::terrain::PlacedSize;
use tore_formats::shape::{Shape, object_scale};

/// Each land gun shape's barrel tip, shape units `[right, forward, up]`.
pub const TIPS: &[(&str, [f64; 3])] = &[
    // The flak guns: the KS-12 (85 mm), the KS-19 (100 mm) and the 37 mm M1939.
    ("AA85.SH", [0., 68., 84.]),
    ("AA105.SH", [0., 83., 96.]),
    ("AA35.SH", [-6., 45., 52.]),
    // Self-propelled AA guns: ZSU-23-4, ZSU-57-2, 2S6 and the M163.
    ("ZSU23.SH", [-1., 30., 18.]),
    ("ZSU57.SH", [-0.5, 47., 39.]),
    ("ZSU30.SH", [0., 33., 20.]),
    ("M3V.SH", [3., 20., 10.]),
    // Tank, IFV and APC guns.
    ("M1.SH", [0., 54., 5.]),
    ("T72.SH", [-1., 54., 7.5]),
    ("T80.SH", [0., 58., 2.5]),
    ("T90.SH", [0., 53., -2.5]),
    ("BMP2.SH", [0., 20., 8.]),
    ("BTR70.SH", [0., 15., 10.]),
    ("M113.SH", [3., 18., 0.5]),
    ("M2.SH", [-1., 18., 8.5]),
];

/// The barrel tip of the gun shape `name`, shape units `[right, forward, up]`.
pub fn tip(name: &str) -> Option<[f64; 3]> {
    TIPS.iter()
        .find(|(shape, _)| shape.eq_ignore_ascii_case(name))
        .map(|(_, tip)| *tip)
}

/// The barrel tip of the shape `name` with the bytes `bytes`, as `[right,
/// up, forward]` feet from the unit's ground point, when it has a listed
/// tip and reads. The unit is drawn at real size, standing on the lowest
/// point of its shape.
pub fn barrel_ft(name: &str, bytes: &[u8]) -> Option<[f64; 3]> {
    let [right, forward, up] = tip(name)?;
    let shape = Shape::scenery(bytes).ok()?;
    let scale = object_scale(bytes).ok()? * PlacedSize::RealSize.factor();
    let lift = grounding::lift_ft(&shape, scale);
    Some([right * scale, lift + up * scale, forward * scale])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_land_gun_shape_has_a_tip_past_its_pivot() {
        for (name, [_, forward, up]) in TIPS {
            assert!(*forward > 10., "{name}");
            assert!(up.abs() < 120., "{name}");
        }
        assert_eq!(tip("aa85.sh"), Some([0., 68., 84.]));
        assert_eq!(tip("TROOPS.SH"), None);
    }
}
