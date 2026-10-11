//! Cluster bomb bomblet flashes (docs/spec/ordnance-presentation.md,
//! "Cluster bombs"; A2G plan decision 13, opinionated, agent).
//!
//! The retail game draws a cluster burst as one small flash, and in a
//! one-machine session about ten bursts in eleven as nothing at all (the
//! 0x800000 flag, docs/formats/weapons.md). The remake keeps the wide splash
//! damage and draws a scatter of small ground flashes over it, so a cluster
//! bomb looks like one. Presentation only: the flashes are derived here from
//! the burst's effect alone, so nothing in the simulation, a checkpoint, the
//! wire or a replay changes, and every client draws the same pattern.
use crate::snapshot::EffectPose;
use tore_sim::combat::{blast, live::EffectKind};

/// Flashes in one burst (fitted).
pub const BOMBLETS: usize = 28;
/// How far from the burst a flash can stand, in feet (fitted). The smallest
/// cluster splash is 1,500 ft (RBK-250), so every flash lies inside the
/// splash of every cluster record.
pub const SPREAD_FT: f64 = 1_200.;
const _: () = assert!(SPREAD_FT < 1_500.);
/// How long one flash plays, in 120 Hz ticks (fitted). Flashes start within
/// the first half of the burst's second, so the last one ends with it.
pub const FLASH_TICKS: u16 = 60;
/// The small explosion each flash draws: type 18 (`AIRSML`), the burst a
/// cluster record names, or its variants 19 and 20 (`AIRSMLA`, `AIRSMLB2`),
/// standing on the ground. The orange flashes read as bomblets; the ground
/// sheets (types 15 and 16) draw dust.
pub const FLASH_TYPES: [u8; 3] = [18, 19, 20];
/// A flash is this share of its explosion type's rolled width (fitted).
pub const FLASH_SCALE: f64 = 1.5;

/// One bomblet flash.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flash {
    pub position: [f64; 3],
    /// Ticks after the burst that the flash starts, 0 to [`FLASH_TICKS`].
    pub start: u16,
    /// The small ground explosion it draws, one of [`FLASH_TYPES`].
    pub kind: u8,
    /// Drawn width in feet.
    pub width: f64,
}

/// The explosion types a cluster burst can show: type 18, which every
/// cluster record names for land and water, and the variants the original
/// rolls for it ([`blast::vary`]).
fn small_air(kind: u8) -> bool {
    matches!(kind, 18..=20 | 24..=26 | 28 | 29)
}

/// Whether `effect` is a cluster bomb going off on the ground or the sea.
/// The five cluster records are the only weapons whose land explosion is
/// type 18 (guns name 15, bombs and missiles 21 and 35), and only an impact
/// on the ground makes a `Ground` effect, so the burst identifies itself.
/// A cluster bomb that strikes an object directly makes a `Hit`, which a
/// gun burst on an aircraft also makes; it draws no flashes.
pub fn is_burst(effect: &EffectPose) -> bool {
    effect.kind == EffectKind::Ground && effect.blast.is_some_and(small_air)
}

/// The flashes of `effect`: none unless it is a cluster burst, otherwise
/// [`BOMBLETS`] flashes scattered evenly over a disc of [`SPREAD_FT`] around
/// the burst, each at a repeatable place, time and size taken from where the
/// burst is.
pub fn derive(effect: &EffectPose) -> Vec<Flash> {
    if !is_burst(effect) {
        return Vec::new();
    }
    let at = effect.position;
    (0..BOMBLETS as u64)
        .map(|n| {
            let salt = 100 + n * 4;
            let angle = f64::from(blast::pick(at, salt, 3600)) / 10.;
            // The square root spreads the flashes evenly over the disc.
            let share = (f64::from(blast::pick(at, salt + 1, 10_000)) + 0.5) / 10_000.;
            let reach = SPREAD_FT * share.sqrt();
            let position = [
                at[0] + reach * angle.to_radians().sin(),
                at[1],
                at[2] + reach * angle.to_radians().cos(),
            ];
            let kind =
                FLASH_TYPES[usize::from(blast::pick(at, salt + 2, FLASH_TYPES.len() as u16))];
            Flash {
                position,
                start: blast::pick(at, salt + 3, FLASH_TICKS + 1),
                kind,
                width: FLASH_SCALE * f64::from(blast::rolled_size(kind, position)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(kind: EffectKind, blast: Option<u8>) -> EffectPose {
        EffectPose {
            kind,
            position: [1_000_000., 40., 790_000.],
            ticks: 120,
            blast,
        }
    }

    #[test]
    fn only_a_ground_impact_of_the_small_air_family_is_a_cluster_burst() {
        for kind in [18, 19, 20, 24, 25, 26, 28, 29] {
            assert!(is_burst(&effect(EffectKind::Ground, Some(kind))), "{kind}");
        }
        // Guns and rockets on land, bombs, missiles, flak and water.
        for kind in [15, 16, 17, 21, 22, 23, 27, 30, 34, 35, 36, 37, 38] {
            assert!(!is_burst(&effect(EffectKind::Ground, Some(kind))), "{kind}");
        }
        // A gun hit on an aircraft is a type 18 too, and is not one.
        for other in [EffectKind::Hit, EffectKind::Destroyed, EffectKind::Flak] {
            assert!(!is_burst(&effect(other, Some(18))), "{other:?}");
        }
        assert!(!is_burst(&effect(EffectKind::Ground, None)));
        assert!(derive(&effect(EffectKind::Hit, Some(18))).is_empty());
    }

    #[test]
    fn the_flashes_are_repeatable_and_belong_to_the_burst() {
        let burst = effect(EffectKind::Ground, Some(20));
        let flashes = derive(&burst);
        assert_eq!(flashes.len(), BOMBLETS);
        assert_eq!(flashes, derive(&burst), "the same burst, the same pattern");
        // The pattern does not depend on the life left or the variety rolled.
        let later = EffectPose {
            ticks: 30,
            blast: Some(28),
            ..burst
        };
        assert_eq!(flashes, derive(&later));
        // Another burst has another pattern.
        let elsewhere = EffectPose {
            position: [1_000_100., 40., 790_000.],
            ..burst
        };
        assert_ne!(flashes[0].position, derive(&elsewhere)[0].position);
    }

    #[test]
    fn the_flashes_stay_inside_every_cluster_splash_and_end_with_the_burst() {
        // Different places, so different patterns, all inside the bounds.
        for east in 0..40 {
            let burst = EffectPose {
                position: [900_000. + f64::from(east) * 137.5, 12., 700_000.],
                ..effect(EffectKind::Ground, Some(18))
            };
            for flash in derive(&burst) {
                let gap = (flash.position[0] - burst.position[0])
                    .hypot(flash.position[2] - burst.position[2]);
                assert!(gap <= SPREAD_FT, "{gap}");
                assert_eq!(flash.position[1], burst.position[1]);
                assert!(flash.start + FLASH_TICKS <= 120, "ends with the burst");
                assert!(FLASH_TYPES.contains(&flash.kind));
                assert!((38. ..=120.).contains(&flash.width), "{}", flash.width);
            }
        }
    }

    #[test]
    fn the_flashes_spread_over_the_disc_and_over_the_second() {
        let burst = effect(EffectKind::Ground, Some(18));
        let flashes = derive(&burst);
        let reach = |f: &Flash| {
            (f.position[0] - burst.position[0]).hypot(f.position[2] - burst.position[2])
        };
        assert!(flashes.iter().any(|f| reach(f) < SPREAD_FT / 2.));
        assert!(flashes.iter().any(|f| reach(f) > SPREAD_FT * 0.8));
        assert!(flashes.iter().any(|f| f.start < 15));
        assert!(flashes.iter().any(|f| f.start > 45));
        for kind in FLASH_TYPES {
            assert!(flashes.iter().any(|f| f.kind == kind), "{kind}");
        }
    }

    /// Checks the retail numbers the module depends on against an import:
    /// run by hand with `TORE_DATA_DIR` set.
    #[test]
    #[ignore]
    fn the_cluster_records_are_the_only_ones_that_name_type_18_for_land() {
        let assets = crate::reel::load_assets().expect("an import");
        let mut clusters = Vec::new();
        for (name, bytes) in assets
            .theater_resources
            .iter()
            .filter(|(n, _)| n.ends_with(".JT"))
        {
            let Ok(w) = tore_formats::weapons::Weapon::parse(name, bytes) else {
                continue;
            };
            let land = w.effects.land_explosion;
            assert_eq!(
                small_air(land),
                w.flags & 0x80_0000 != 0,
                "{name}: land {land} flags {:#x}",
                w.flags
            );
            if small_air(land) {
                clusters.push(name.clone());
            }
        }
        clusters.sort();
        assert_eq!(
            clusters,
            ["CBU87.JT", "CBU89.JT", "MK20.JT", "RBK250.JT", "RBK500.JT"]
        );
    }
}
