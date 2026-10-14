//! Corrections to retail aircraft records that are plainly copied from another
//! aircraft (slice HF3, John's requests of 2026-10-13: "V-22 tank: fix (real
//! fuel)", and "check other records for copying issues as well", which gave
//! the CH-47 its own weights). See [aircraft records](../../../docs/formats/aircraft.md#rotorcraft-fuel-records).
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

/// The CH-47's empty weight, lb: the CH-47F's 24,578 lb (Pub, Wikipedia
/// "Boeing CH-47 Chinook", specifications). The record carries the Mi-24
/// record's 18,078 lb.
pub const CH47_EMPTY_LB: i32 = 24_578;
/// The CH-47's internal fuel, lb: the CH-47F's 1,080 US gallons (Pub,
/// Wikipedia) at 6.8 lb a gallon (JP-5). The record carries the Mi-24's
/// 3,307 lb (1,500 kg).
pub const CH47_INTERNAL_FUEL_LB: i32 = 7_340;
/// The CH-47's maximum takeoff weight, lb: the CH-47F's 54,000 lb gross
/// weight (Pub, Wikipedia; the rotor model already scaled its power by it).
/// The record carries the Mi-24's 28,660 lb.
pub const CH47_MAX_TAKEOFF_LB: i32 = 54_000;

// About 12,000 lb of fuel, and empty plus fuel within the maximum.
const _: () = assert!(V22_INTERNAL_FUEL_LB > 11_000 && V22_INTERNAL_FUEL_LB < 12_500);
const _: () = assert!(V22_EMPTY_LB + V22_INTERNAL_FUEL_LB < V22_MAX_TAKEOFF_LB);
const _: () = assert!(CH47_EMPTY_LB + CH47_INTERNAL_FUEL_LB < CH47_MAX_TAKEOFF_LB);

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
    // The CH-47's own thrust (135,795 lbf, implausible and set aside by the
    // rotor model) is not a copy and stays.
    Correction {
        id: AircraftId::Ch47,
        block: Block::Object,
        key: "weight",
        retail: 18_078,
        corrected: CH47_EMPTY_LB,
    },
    Correction {
        id: AircraftId::Ch47,
        block: Block::Plane,
        key: "internalFuel",
        retail: 3_307,
        corrected: CH47_INTERNAL_FUEL_LB,
    },
    Correction {
        id: AircraftId::Ch47,
        block: Block::Plane,
        key: "maxTakeoffWeight",
        retail: 28_660,
        corrected: CH47_MAX_TAKEOFF_LB,
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
    fn the_ch47_record_with_the_mi24s_weights_gets_the_ch47fs_but_the_mi24_keeps_its_own() {
        let mi24 = || {
            let object = BTreeMap::from([("weight".to_string(), token(18_078))]);
            let plane = BTreeMap::from([
                ("internalFuel".to_string(), token(3_307)),
                ("maxTakeoffWeight".to_string(), token(28_660)),
                ("thrust".to_string(), token(135_795)),
            ]);
            (object, plane)
        };
        let (mut object, mut plane) = mi24();
        assert!(apply(AircraftId::Ch47, &mut object, &mut plane));
        assert_eq!(object["weight"].number().unwrap(), CH47_EMPTY_LB);
        assert_eq!(
            plane["internalFuel"].number().unwrap(),
            CH47_INTERNAL_FUEL_LB
        );
        assert_eq!(
            plane["maxTakeoffWeight"].number().unwrap(),
            CH47_MAX_TAKEOFF_LB
        );
        assert_eq!(plane["thrust"].number().unwrap(), 135_795);
        let (mut object, mut plane) = mi24();
        assert!(!apply(AircraftId::Mi24, &mut object, &mut plane));
        assert_eq!(object["weight"].number().unwrap(), 18_078);
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
}
