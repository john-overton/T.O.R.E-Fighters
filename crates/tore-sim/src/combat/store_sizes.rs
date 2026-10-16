//! Real store sizes (slice BP2) and the size a store hangs by (slice PY2,
//! docs/spec/stores-on-pylons.md#store-sizes).
//!
//! The simulation needs a store's length and how far its lugs are above its
//! origin to place it on its pylon, so that a store leaves from where it is
//! drawn; the drawing refits the retail shapes to the same table
//! (`tore-app/src/store_sizes.rs`). Opinionated (John 2026-10-12: real
//! sizes); the numbers are public dimensions, rounded.

/// A store's real size, feet: overall length, body diameter, and the span
/// across its largest fins or wings as carried. `flight_span` is the span
/// once released, when that differs (a retarded bomb's fins open).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Real {
    pub length: f64,
    pub diameter: f64,
    pub span: f64,
    pub flight_span: Option<f64>,
}

pub const fn real(length: f64, diameter: f64, span: f64) -> Real {
    Real {
        length,
        diameter,
        span,
        flight_span: None,
    }
}

/// Real sizes by retail shape name. A shape several weapons share takes the
/// size of the store it depicts (the HARM shape also draws the Shrike).
/// Sources: US Navy and US Air Force fact files and the manufacturers' data
/// for the western stores, the Russian and French makers' published data
/// for the others, as collected in public references (Wikipedia, FAS
/// Military Analysis Network); the docs table lists each.
pub const SIZES: &[(&str, Real)] = &[
    // Air-to-air missiles.
    ("SIDEM.SH", real(9.35, 0.42, 2.07)),
    ("SIDEB.SH", real(9.3, 0.42, 1.83)),
    ("AM9X.SH", real(9.9, 0.42, 1.47)),
    ("AMRAM.SH", real(12.0, 0.58, 1.75)),
    ("SPAR.SH", real(12.0, 0.67, 3.3)),
    ("PHENX.SH", real(13.0, 1.25, 3.0)),
    ("MICA.SH", real(10.2, 0.53, 1.83)),
    ("M55.SH", real(9.0, 0.52, 2.17)),
    ("M53.SH", real(12.5, 0.86, 2.03)),
    ("APHID.SH", real(6.9, 0.39, 1.28)),
    ("ARCHR.SH", real(9.5, 0.56, 1.67)),
    ("AA11B.SH", real(9.5, 0.56, 1.67)),
    ("ALAMO.SH", real(14.8, 0.75, 3.2)),
    ("AMSKI.SH", real(11.8, 0.66, 2.2)),
    ("ATOLL.SH", real(9.2, 0.42, 1.73)),
    ("ACRID.SH", real(20.0, 1.02, 4.76)),
    ("AMOS.SH", real(13.6, 1.25, 3.87)),
    ("STGR.SH", real(5.0, 0.23, 0.3)),
    // Air-to-ground missiles.
    ("MAVRK.SH", real(8.2, 1.0, 2.36)),
    ("HARM.SH", real(13.7, 0.83, 3.7)),
    ("HARPN.SH", real(12.6, 1.12, 3.0)),
    ("SLAM.SH", real(14.6, 1.12, 3.0)),
    ("KERRY.SH", real(11.6, 0.9, 2.58)),
    ("KEDGE.SH", real(12.8, 1.25, 3.6)),
    ("KIKBK.SH", real(15.7, 1.49, 3.0)),
    ("AS12.SH", real(7.1, 0.62, 1.85)),
    ("AS30.SH", real(12.0, 1.12, 3.3)),
    ("EXO.SH", real(15.4, 1.15, 3.6)),
    ("SWATR.SH", real(3.8, 0.48, 2.17)),
    // Bombs and dispensers.
    ("MK82.SH", real(7.3, 0.9, 1.27)),
    ("MK82A.SH", real(7.3, 0.9, 1.27)),
    (
        "SNAKE.SH",
        Real {
            length: 7.4,
            diameter: 0.9,
            span: 1.27,
            // Retail draws the Snakeye's fins open; they open after release.
            flight_span: Some(4.0),
        },
    ),
    ("MK84.SH", real(10.8, 1.5, 2.1)),
    ("MK20.SH", real(7.7, 1.1, 1.45)),
    ("CB87.SH", real(7.7, 1.3, 1.9)),
    ("CB89.SH", real(7.7, 1.3, 1.9)),
    // Fitted smaller kit envelopes; private aliases retain donor geometry.
    ("GB12.SH", real(10.8, 0.9, 4.4)),
    ("GB16.SH", real(12.1, 1.17, 5.0)),
    ("GB38.SH", real(7.7, 0.9, 1.67)),
    ("PAVE.SH", real(14.2, 1.5, 5.4)),
    ("GB27.SH", real(13.9, 1.2, 5.4)),
    ("GB28.SH", real(19.2, 1.21, 2.2)),
    ("F250.SH", real(6.4, 1.07, 1.35)),
    ("F500.SH", real(7.9, 1.3, 1.6)),
    ("F1000.SH", real(9.4, 2.07, 2.6)),
    ("RBK2.SH", real(6.4, 1.07, 1.35)),
    ("RBK5.SH", real(8.2, 1.48, 1.8)),
];

/// The real size of the store shape `name` draws, when it is listed.
pub fn real_size(name: &str) -> Option<Real> {
    SIZES
        .iter()
        .find(|(shape, _)| shape.eq_ignore_ascii_case(name))
        .map(|(_, real)| *real)
}

/// The SUU-16 gun pod's length, feet: its muzzle is half this ahead of the
/// pod's middle ([`StoreSize::of_weapon`]'s origin).
pub const SUU16_LENGTH: f64 = 15.7;

/// A fitted rocket pod's length and diameter, feet: LAU-61 size (retail
/// has no pod shape).
pub const POD_BODY: [f64; 2] = [8.5, 1.4];

/// A rocket pod's length and diameter, feet, by weapon record: the real
/// pod's size (slice BP2); [`POD_BODY`] for one not listed. Opinionated
/// (John 2026-10-12: real sizes), from public data.
pub fn pod_body(source: &str) -> [f64; 2] {
    match source.to_ascii_uppercase().as_str() {
        "LAU61.JT" => [5.5, 1.38],
        "LAU10.JT" => [8.3, 1.25],
        "B8.JT" => [8.9, 1.71],
        "B13.JT" => [11.7, 1.35],
        // The SUU-16/A: about 16 ft by 22 in, public data (2026-10-16); the
        // game's record has no shape.
        "SUU16.JT" => [SUU16_LENGTH, 1.8],
        _ => POD_BODY,
    }
}

/// A fitted fuel tank's length and diameter, feet, from the fuel it holds:
/// 13 by 2 ft for 2,000 lb (300 US gallons), scaled by the cube root of the
/// fuel (retail has no tank shape).
pub fn tank_body(fuel_lbs: f64) -> [f64; 2] {
    let scale = (fuel_lbs.max(100.) / 2_000.).cbrt();
    [13. * scale, 2. * scale]
}

/// A store with no listed size: length and diameter, feet (fitted).
pub const PLAIN: [f64; 2] = [8., 0.9];

/// What placing a store on its pylon needs, feet: its length, how far its
/// lugs (the top of its body) are above its origin, and its width across
/// its fins, which spaces stores side by side on a rack. The origin is
/// taken as the store's middle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StoreSize {
    pub length: f64,
    pub top: f64,
    pub width: f64,
}

impl StoreSize {
    /// A round body (a tank, a pod, a store with no listed size).
    pub fn body([length, diameter]: [f64; 2]) -> Self {
        Self {
            length,
            top: diameter / 2.,
            width: diameter,
        }
    }
    /// A store drawn with the retail shape `shape`: its real size when the
    /// table lists it, else [`PLAIN`].
    pub fn of_shape(shape: Option<&str>) -> Self {
        match shape.and_then(real_size) {
            Some(real) => Self {
                length: real.length,
                top: real.diameter / 2.,
                width: real.span.max(real.diameter),
            },
            None => Self::body(PLAIN),
        }
    }
    /// A weapon station's store: a rocket pod's body, or the weapon's shape.
    pub fn of_weapon(weapon: &tore_formats::weapons::Weapon) -> Self {
        if super::ballistics::is_pod(weapon) || super::live::is_gun_pod(weapon) {
            Self::body(pod_body(&weapon.source))
        } else {
            Self::of_shape(weapon.shape.as_deref())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_store_hangs_by_its_real_size() {
        let sparrow = StoreSize::of_shape(Some("spar.sh"));
        assert_eq!(sparrow.length, 12.0);
        assert!((sparrow.top - 0.335).abs() < 1e-9);
        assert_eq!(sparrow.width, 3.3);
        assert_eq!(StoreSize::of_shape(None), StoreSize::body(PLAIN));
        assert_eq!(tank_body(2_000.), [13., 2.]);
    }
}
