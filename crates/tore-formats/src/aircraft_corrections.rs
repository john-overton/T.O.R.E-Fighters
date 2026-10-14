//! Corrections to retail aircraft records that are plainly copied from another
//! aircraft (slice HF3, John's request of 2026-10-13: "V-22 tank: fix (real
//! fuel)"; slice nr-rec added the Su-25, MiG-17F and C-130 rows for "Su-25
//! record: fix, and check every other record for copying",
//! docs/spec/aircraft-records.md). See [aircraft records](../../../docs/formats/aircraft.md#rotorcraft-fuel-records).
//!
//! A correction applies only when every value it replaces still holds the
//! copied retail number, so a different or edited record is read as it is.
//! The corrected numbers are published figures (each row says which); they
//! are **opinionated** (John's request), the choice of figure an agent
//! decision.
use super::{AircraftId, Token};
use std::collections::BTreeMap;

/// Which record block a field lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    /// The OBJ_TYPE block (`weight` is the empty weight).
    Object,
    /// The PLANE_TYPE block.
    Plane,
}

/// One field of one record replaced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Correction {
    pub id: AircraftId,
    pub block: Block,
    pub key: &'static str,
    /// The copied retail value; nothing is replaced unless every row for the
    /// aircraft still reads its retail value.
    pub retail: i32,
    pub corrected: i32,
}

/// The V-22's empty weight, lb: the MV-22B's 31,818 lb (Pub, Wikipedia
/// "Bell Boeing V-22 Osprey", specifications). The record carries the
/// AH-64 record's 18,298 lb.
pub const V22_EMPTY_LB: i32 = 31_818;
/// The V-22's internal fuel, lb: the MV-22B's 1,721 US gallons (Pub, Bell
/// Boeing V-22 brochure) at 6.8 lb a gallon (JP-5). The record carries the
/// AH-64 record's 2,000 lb.
pub const V22_INTERNAL_FUEL_LB: i32 = 11_700;
/// The V-22's maximum takeoff weight, lb: the published 52,600 lb vertical
/// takeoff maximum (Pub, Wikipedia; the rotor model already scaled its power
/// by it). The record carries 23,810 lb.
pub const V22_MAX_TAKEOFF_LB: i32 = 52_600;
/// The V-22's record thrust, lbf, scaled with the maximum weight so the
/// record's thrust-to-weight (1.10) is kept: 26,280 x 52,600 / 23,810. The
/// tiltrotor's power is set by its engines (`models/variety/lift.rs`); this
/// only keeps the record's own ratio for the readers that use it (the wreck,
/// the flight watch).
pub const V22_THRUST_LBF: i32 = 58_057;

// About 12,000 lb of fuel, and empty plus fuel within the maximum.
const _: () = assert!(V22_INTERNAL_FUEL_LB > 11_000 && V22_INTERNAL_FUEL_LB < 12_500);
const _: () = assert!(V22_EMPTY_LB + V22_INTERNAL_FUEL_LB < V22_MAX_TAKEOFF_LB);

/// Su-25 Frogfoot-A (slice nr-rec): SU25.PT carries SU24.PT's empty weight,
/// fuel, maximum takeoff weight, thrust and dry flow word for word. Published
/// Su-25/Su-25K: empty 9,800 kg, maximum takeoff 19,300 kg, two R-195 at
/// 44.18 kN (9,930 lbf) each (Pub, Wikipedia "Sukhoi Su-25",
/// specifications); internal fuel 3,000 kg (Pub, MILAVIA Su-25
/// specifications). Dry flow `fitted` at the retail fleet's median, 1.12 lb/s
/// per 10,000 lbf of military thrust, rounded (docs/spec/aircraft-records.md).
pub const SU25_EMPTY_LB: i32 = 21_605;
pub const SU25_INTERNAL_FUEL_LB: i32 = 6_614;
pub const SU25_MAX_TAKEOFF_LB: i32 = 42_549;
pub const SU25_THRUST_LBF: i32 = 19_860;
pub const SU25_FUEL_LB_PER_S: i32 = 2;
/// MiG-17F (slice nr-rec): MIG17F.PT carries MIG21.PT's 4,534 lb of fuel, 1.8
/// times the real tank, and flows 4.7 (dry) and 5.3 (burner) times the fleet
/// median per pound of thrust. Published internal fuel 1,143 kg (Pub, FAS
/// "MiG-17"); flows `fitted` at the fleet medians (1.12 dry, 5.0 burner lb/s
/// per 10,000 lbf), rounded. The retail empty weight (6,000 lb) is kept: the
/// stall reference and loading are taken from it.
pub const MIG17_INTERNAL_FUEL_LB: i32 = 2_520;
pub const MIG17_FUEL_LB_PER_S: i32 = 1;
pub const MIG17_AB_FUEL_LB_PER_S: i32 = 4;
/// C-130 (slice nr-rec): C130.PT's 108,192 lbf is 2.7 times AC130.PT's
/// 40,000 for the same airframe, weights and four T56 engines (a
/// thrust-to-weight near 0.9). The AC-130U record's value is used (`fitted`).
pub const C130_THRUST_LBF: i32 = 40_000;

const _: () = assert!(SU25_EMPTY_LB + SU25_INTERNAL_FUEL_LB < SU25_MAX_TAKEOFF_LB);

/// Every correction, by aircraft.
pub const CORRECTIONS: &[Correction] = &[
    Correction {
        id: AircraftId::V22,
        block: Block::Object,
        key: "weight",
        retail: 18_298,
        corrected: V22_EMPTY_LB,
    },
    Correction {
        id: AircraftId::V22,
        block: Block::Plane,
        key: "internalFuel",
        retail: 2_000,
        corrected: V22_INTERNAL_FUEL_LB,
    },
    Correction {
        id: AircraftId::V22,
        block: Block::Plane,
        key: "maxTakeoffWeight",
        retail: 23_810,
        corrected: V22_MAX_TAKEOFF_LB,
    },
    Correction {
        id: AircraftId::V22,
        block: Block::Plane,
        key: "thrust",
        retail: 26_280,
        corrected: V22_THRUST_LBF,
    },
    Correction {
        id: AircraftId::Su25,
        block: Block::Object,
        key: "weight",
        retail: 41_885,
        corrected: SU25_EMPTY_LB,
    },
    Correction {
        id: AircraftId::Su25,
        block: Block::Plane,
        key: "internalFuel",
        retail: 22_664,
        corrected: SU25_INTERNAL_FUEL_LB,
    },
    Correction {
        id: AircraftId::Su25,
        block: Block::Plane,
        key: "maxTakeoffWeight",
        retail: 90_390,
        corrected: SU25_MAX_TAKEOFF_LB,
    },
    Correction {
        id: AircraftId::Su25,
        block: Block::Plane,
        key: "thrust",
        retail: 35_270,
        corrected: SU25_THRUST_LBF,
    },
    Correction {
        id: AircraftId::Su25,
        block: Block::Plane,
        key: "fuelConsumption",
        retail: 6,
        corrected: SU25_FUEL_LB_PER_S,
    },
    Correction {
        id: AircraftId::Mig17,
        block: Block::Plane,
        key: "internalFuel",
        retail: 4_534,
        corrected: MIG17_INTERNAL_FUEL_LB,
    },
    Correction {
        id: AircraftId::Mig17,
        block: Block::Plane,
        key: "fuelConsumption",
        retail: 3,
        corrected: MIG17_FUEL_LB_PER_S,
    },
    Correction {
        id: AircraftId::Mig17,
        block: Block::Plane,
        key: "aftFuelConsumption",
        retail: 20,
        corrected: MIG17_AB_FUEL_LB_PER_S,
    },
    Correction {
        id: AircraftId::C130,
        block: Block::Plane,
        key: "thrust",
        retail: 108_192,
        corrected: C130_THRUST_LBF,
    },
];

/// Applies the corrections for `id` when the record still carries every
/// copied value; true when it did.
pub(crate) fn apply(
    id: AircraftId,
    object: &mut BTreeMap<String, Token>,
    plane: &mut BTreeMap<String, Token>,
) -> bool {
    let rows: Vec<_> = CORRECTIONS.iter().filter(|c| c.id == id).collect();
    if rows.is_empty() {
        return false;
    }
    let reads =
        |object: &BTreeMap<String, Token>, plane: &BTreeMap<String, Token>, c: &Correction| {
            let map = match c.block {
                Block::Object => object,
                Block::Plane => plane,
            };
            map.get(c.key).and_then(|t| t.number().ok()) == Some(c.retail)
        };
    if !rows.iter().all(|c| reads(object, plane, c)) {
        return false;
    }
    for c in rows {
        let map = match c.block {
            Block::Object => &mut *object,
            Block::Plane => &mut *plane,
        };
        if let Some(token) = map.get_mut(c.key) {
            token.value = c.corrected.to_string();
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(value: i32) -> Token {
        Token {
            kind: "dword".into(),
            value: value.to_string(),
            scaled: false,
        }
    }

    fn v22(fuel: i32) -> (BTreeMap<String, Token>, BTreeMap<String, Token>) {
        let object = BTreeMap::from([("weight".to_string(), token(18_298))]);
        let plane = BTreeMap::from([
            ("internalFuel".to_string(), token(fuel)),
            ("maxTakeoffWeight".to_string(), token(23_810)),
            ("thrust".to_string(), token(26_280)),
        ]);
        (object, plane)
    }

    #[test]
    fn the_copied_v22_record_gets_the_mv22bs_weights_and_fuel() {
        let (mut object, mut plane) = v22(2_000);
        assert!(apply(AircraftId::V22, &mut object, &mut plane));
        let n = |map: &BTreeMap<String, Token>, key: &str| map[key].number().unwrap();
        assert_eq!(n(&object, "weight"), V22_EMPTY_LB);
        assert_eq!(n(&plane, "internalFuel"), V22_INTERNAL_FUEL_LB);
        assert_eq!(n(&plane, "maxTakeoffWeight"), V22_MAX_TAKEOFF_LB);
        assert_eq!(n(&plane, "thrust"), V22_THRUST_LBF);
    }

    #[test]
    fn a_record_that_differs_from_the_copy_is_left_as_it_is() {
        let (mut object, mut plane) = v22(2_001);
        assert!(!apply(AircraftId::V22, &mut object, &mut plane));
        assert_eq!(object["weight"].number().unwrap(), 18_298);
        assert_eq!(plane["internalFuel"].number().unwrap(), 2_001);
        // The AH-64 carries the same numbers and is its own, right, record.
        let (mut object, mut plane) = v22(2_000);
        assert!(!apply(AircraftId::Ah64, &mut object, &mut plane));
        assert_eq!(plane["thrust"].number().unwrap(), 26_280);
    }
    #[test]
    fn the_su25_loses_the_su24s_numbers() {
        let object = BTreeMap::from([("weight".to_string(), token(41_885))]);
        let plane = BTreeMap::from([
            ("internalFuel".to_string(), token(22_664)),
            ("maxTakeoffWeight".to_string(), token(90_390)),
            ("thrust".to_string(), token(35_270)),
            ("fuelConsumption".to_string(), token(6)),
        ]);
        let n = |map: &BTreeMap<String, Token>, key: &str| map[key].number().unwrap();
        let (mut o, mut p) = (object.clone(), plane.clone());
        assert!(apply(AircraftId::Su25, &mut o, &mut p));
        assert_eq!(n(&o, "weight"), SU25_EMPTY_LB);
        assert_eq!(n(&p, "internalFuel"), SU25_INTERNAL_FUEL_LB);
        assert_eq!(n(&p, "maxTakeoffWeight"), SU25_MAX_TAKEOFF_LB);
        assert_eq!(n(&p, "thrust"), SU25_THRUST_LBF);
        assert_eq!(n(&p, "fuelConsumption"), SU25_FUEL_LB_PER_S);
        // The Su-24 carries the same numbers and is its own record.
        let (mut o, mut p) = (object.clone(), plane.clone());
        assert!(!apply(AircraftId::Mig23, &mut o, &mut p));
        // One value off the copy leaves the record alone.
        let (mut o, mut p) = (object, plane);
        p.get_mut("thrust").unwrap().value = "35271".into();
        assert!(!apply(AircraftId::Su25, &mut o, &mut p));
        assert_eq!(n(&o, "weight"), 41_885);
    }

    #[test]
    fn the_mig17_and_c130_corrections_apply_to_their_retail_values() {
        let n = |map: &BTreeMap<String, Token>, key: &str| map[key].number().unwrap();
        let mut o = BTreeMap::new();
        let mut p = BTreeMap::from([
            ("internalFuel".to_string(), token(4_534)),
            ("fuelConsumption".to_string(), token(3)),
            ("aftFuelConsumption".to_string(), token(20)),
        ]);
        assert!(apply(AircraftId::Mig17, &mut o, &mut p));
        assert_eq!(n(&p, "internalFuel"), MIG17_INTERNAL_FUEL_LB);
        assert_eq!(n(&p, "fuelConsumption"), MIG17_FUEL_LB_PER_S);
        assert_eq!(n(&p, "aftFuelConsumption"), MIG17_AB_FUEL_LB_PER_S);
        let mut p = BTreeMap::from([("thrust".to_string(), token(108_192))]);
        assert!(apply(AircraftId::C130, &mut o, &mut p));
        assert_eq!(n(&p, "thrust"), C130_THRUST_LBF);
        // The AC-130U's own 40,000 is not touched.
        let mut p = BTreeMap::from([("thrust".to_string(), token(40_000))]);
        assert!(!apply(AircraftId::Ac130, &mut o, &mut p));
    }
}
