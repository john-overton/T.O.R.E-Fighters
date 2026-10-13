//! Which destroyed ground objects burn, and how hard (B16,
//! docs/spec/surface-defenses.md, "Wreck fires").
//!
//! Every destroyed building and fixed object burns (John, 2026-10-12), but
//! pavement (runways, airstrips, roads, lots, crop fields), bridges, rocks
//! and the crater object do not (agent decision). Fuel, ammunition and
//! industry burn hard; other buildings burn smaller (agent fit). A destroyed
//! vehicle, SAM launcher or gun burns about half the time (John,
//! 2026-10-12): the pick is a fixed function of the object's id and the
//! mission's surface digest, never a random stream, so every machine and
//! every replay of the same mission agrees.

use super::{DestroyedLook, Surface, Unit};

/// How a destroyed object's fire burns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FireClass {
    /// No fire: pavement, a bridge, a rock, a man, a vehicle that did not
    /// catch.
    None,
    /// A building or fixed object's fire, sized by the object.
    Building,
    /// Fuel, ammunition and industry: a fire twice as wide (agent fit).
    Hard,
    /// A wrecked vehicle, SAM launcher, gun or ship that caught: the fire
    /// the surface round sized by its hit points.
    Vehicle,
}

impl FireClass {
    pub fn burns(self) -> bool {
        self != FireClass::None
    }
}

/// The object type's stem: `FUEL.OT` to `FUEL`, `~BNK5.OT` to `BNK5`.
fn stem(object_type: &str) -> String {
    object_type
        .trim_start_matches('~')
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase()
}

/// The fire class of a destroyed static object of type `object_type`
/// (`HANGR.OT`), `runway` when it is a runway or airstrip.
pub fn object_fire(object_type: &str, runway: bool) -> FireClass {
    if runway {
        return FireClass::None;
    }
    let stem = stem(object_type);
    let starts = |prefixes: &[&str]| prefixes.iter().any(|p| stem.starts_with(p));
    // Pavement and ground cover, bridges and their spans, rocks and the
    // crater, flags and the destroyed-vehicle wreck object itself.
    if starts(&[
        "STRIP", "DTSTRP", "ROAD", "LOT", "CROP", "BR", "ROCK", "CRATER", "FLAG", "DEST",
    ]) {
        return FireClass::None;
    }
    // Fuel, ammunition, supply and industry.
    if starts(&[
        "FUEL", "OILW", "SUPPLY", "STORE", "CRATE", "FACT", "FCTY", "IND", "PRDR", "REACT", "NUC",
        "SILO", "CONT",
    ]) {
        return FireClass::Hard;
    }
    FireClass::Building
}

/// Whether destroyed ground unit `id` catches fire in the mission whose
/// surface digest is `mission`: about one in two (John, 2026-10-12), the
/// same on every machine and in every replay.
pub fn vehicle_burns(mission: u64, id: u32) -> bool {
    let mut v = mission ^ (u64::from(id).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    v ^= v >> 30;
    v = v.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    v ^= v >> 27;
    v = v.wrapping_mul(0x94d0_49bb_1331_11eb);
    v ^= v >> 31;
    v & 1 == 1
}

/// The scorch a burning building leaves round its foot (B16): a crater
/// mark (16 ft of half width a unit) reaching a tenth past the larger half
/// of its plan, `half` its contact box's half sizes (right, up, forward),
/// from [`SCORCH_SIZES`]`[0]` to `[1]` (agent fit, 2026-10-12).
pub fn scorch_size(half: [f64; 3]) -> u8 {
    let reach = half[0].max(half[2]) * 1.1;
    ((reach / 16.).ceil() as u8).clamp(SCORCH_SIZES[0], SCORCH_SIZES[1])
}
/// The smallest and largest scorch, in crater size units.
pub const SCORCH_SIZES: [u8; 2] = [2, 16];

/// Whether static object type `object_type` is a unit type (`TANKER.NT`),
/// placed by a layout without a unit record: a vehicle.
pub fn is_vehicle_type(object_type: &str) -> bool {
    object_type.to_ascii_uppercase().ends_with(".NT")
}

/// The fire class of surface unit `unit` in the mission whose surface
/// digest is `mission`.
pub fn unit_fire(unit: &Unit, mission: u64) -> FireClass {
    match &unit.look {
        // Ships burn as before.
        DestroyedLook::DamagedShape(_) => FireClass::Vehicle,
        DestroyedLook::Wreck(_) => {
            if vehicle_burns(mission, unit.id.0) {
                FireClass::Vehicle
            } else {
                FireClass::None
            }
        }
        // Buildings among the units (a template's objectives, bunkers, the
        // GCI radar) burn as buildings.
        DestroyedLook::DamagedObject(_) | DestroyedLook::Removed => {
            object_fire(&unit.resource, false)
        }
        DestroyedLook::Bodies(_) | DestroyedLook::Vanish => FireClass::None,
    }
}

/// The mission's seed for the fire picks: the surface digest every machine
/// computes alike (it holds the Quick Mission's surface seed and every
/// placed unit).
pub fn mission_seed(surface: &Surface) -> u64 {
    surface.digest()
}

/// The fire class of every object of a mission's scene, by id: its
/// surface units (with the mission's vehicle picks) and every other static
/// object of `scene`. The simulation lights the fires of those that burn;
/// the picture sizes each fire by its class.
pub fn scene_fires(
    surface: &Surface,
    scene: &tore_sim::airport::Scene,
) -> std::collections::BTreeMap<u32, FireClass> {
    let mission = mission_seed(surface);
    let mut out: std::collections::BTreeMap<u32, FireClass> = surface
        .units
        .iter()
        .map(|unit| (unit.id.0, unit_fire(unit, mission)))
        .collect();
    for object in &scene.objects {
        out.entry(object.id).or_insert_with(|| {
            // A layout's vehicle with no unit record (a parked fuel truck)
            // catches as any vehicle.
            if is_vehicle_type(&object.object_type) {
                if vehicle_burns(mission, object.id) {
                    FireClass::Vehicle
                } else {
                    FireClass::None
                }
            } else {
                object_fire(&object.object_type, object.runway)
            }
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vehicle_pick_is_fixed_and_about_half() {
        let mission = 0x1234_5678_9abc_def0;
        let burning = (0..10_000u32)
            .filter(|i| vehicle_burns(mission, 0x5000_0000 + i))
            .count();
        assert!((4_700..=5_300).contains(&burning), "{burning}");
        // The same object in the same mission always gives the same answer,
        // and another mission picks differently.
        for i in 0..100u32 {
            assert_eq!(vehicle_burns(mission, i), vehicle_burns(mission, i));
        }
        let differ = (0..1_000u32)
            .filter(|i| vehicle_burns(mission, *i) != vehicle_burns(mission ^ 1, *i))
            .count();
        assert!(differ > 300, "{differ}");
        // Recorded values: a change to the pick changes every recording.
        let picks: Vec<bool> = (0..8u32)
            .map(|i| vehicle_burns(7, 0x5000_0000 + i))
            .collect();
        assert_eq!(picks, PICKS_7);
    }
    const PICKS_7: [bool; 8] = [true, true, false, false, true, true, true, false];

    #[test]
    fn buildings_burn_and_pavement_bridges_and_rocks_do_not() {
        assert_eq!(object_fire("HANGR.OT", false), FireClass::Building);
        assert_eq!(object_fire("CTWR1.OT", false), FireClass::Building);
        assert_eq!(object_fire("~BNK5.OT", false), FireClass::Building);
        assert_eq!(object_fire("FUEL.OT", false), FireClass::Hard);
        assert_eq!(object_fire("FACT1.OT", false), FireClass::Hard);
        assert_eq!(object_fire("STORE.OT", false), FireClass::Hard);
        assert_eq!(object_fire("STRIP.OT", true), FireClass::None);
        assert_eq!(object_fire("HANGR.OT", true), FireClass::None);
        for none in [
            "ROAD2.OT",
            "BR1MID.OT",
            "BRD3.OT",
            "ROCKA.OT",
            "LOT.OT",
            "CROPA.OT",
        ] {
            assert_eq!(object_fire(none, false), FireClass::None, "{none}");
        }
        assert!(!FireClass::None.burns() && FireClass::Hard.burns());
    }

    #[test]
    fn a_scorch_reaches_past_the_building() {
        // A hangar 150 ft long: 83 ft of reach, six units of 16 ft.
        assert_eq!(scorch_size([75., 30., 40.]), 6);
        assert_eq!(scorch_size([1., 1., 1.]), SCORCH_SIZES[0]);
        assert_eq!(scorch_size([900., 30., 40.]), SCORCH_SIZES[1]);
    }
}
