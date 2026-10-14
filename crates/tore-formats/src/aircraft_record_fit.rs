//! Reviewed corrections to retail aircraft records that copy another
//! aircraft's numbers, or are off by a large factor, against published
//! figures. See docs/spec/aircraft-records.md. These are agent decisions on
//! John's request of 2026-10-13 (opinionated), with fitted fuel flows; they
//! are not retail behaviour.
//!
//! A correction applies only when every listed field of that record still
//! holds its known retail value, so an edited or different record is left as
//! decoded. The imported bytes are never changed:
//! [`super::Aircraft::parse_decoded`] reads the record as the retail game has it.
use super::{AircraftId, Token};
use std::collections::BTreeMap;

/// Which PT block a corrected field lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    /// OBJ_TYPE (`Aircraft::object`): the empty weight.
    Object,
    /// PLANE_TYPE (`Aircraft::fields`): fuel, thrust, flows, maximum weight.
    Plane,
}

/// One corrected record field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordFit {
    pub block: Block,
    pub key: &'static str,
    /// The value the retail record holds.
    pub retail: i32,
    /// The value the game uses instead.
    pub fitted: i32,
}

const fn object(key: &'static str, retail: i32, fitted: i32) -> RecordFit {
    RecordFit {
        block: Block::Object,
        key,
        retail,
        fitted,
    }
}

const fn plane(key: &'static str, retail: i32, fitted: i32) -> RecordFit {
    RecordFit {
        block: Block::Plane,
        key,
        retail,
        fitted,
    }
}

/// Su-25 Frogfoot-A: the retail record is the Su-24's (SU24.PT) mass, fuel,
/// maximum weight, thrust and dry fuel flow, word for word. Published
/// Su-25/Su-25K: empty 9,800 kg (21,605 lb), maximum takeoff 19,300 kg
/// (42,549 lb), two R-195 at 44.18 kN (9,930 lbf) each (Wikipedia, Sukhoi
/// Su-25, specifications); internal fuel 3,000 kg (6,614 lb) (MILAVIA Su-25
/// specifications). Dry flow fitted at the retail fleet's median of 1.12 lb/s
/// per 10,000 lbf of military thrust, rounded: 2 lb/s.
const SU25: &[RecordFit] = &[
    object("weight", 41_885, 21_605),
    plane("internalFuel", 22_664, 6_614),
    plane("maxTakeoffWeight", 90_390, 42_549),
    plane("thrust", 35_270, 19_860),
    plane("fuelConsumption", 6, 2),
];

/// MiG-17F Fresco: the retail internal fuel is the MiG-21's (MIG21.PT) 4,534
/// lb, 1.8 times the real tank, and its fuel flows are 4.7 (dry) and 5.3
/// (burner) times the retail fleet's median per pound of thrust. Published
/// MiG-17F internal fuel: 1,143 kg (2,520 lb) (FAS, MiG-17). Flows fitted at
/// the fleet medians (1.12 dry, 5.0 burner lb/s per 10,000 lbf): 1 and 4
/// lb/s. The retail empty weight (6,000 lb against a published 8,640) is left:
/// the variety model's weight-scaled stall and loading are taken from it, and
/// raising it left the clean MiG-17 unable to climb away after liftoff.
const MIG17: &[RecordFit] = &[
    plane("internalFuel", 4_534, 2_520),
    plane("fuelConsumption", 3, 1),
    plane("aftFuelConsumption", 20, 4),
];

/// C-130 Hercules: retail military thrust 108,192 lbf is 2.7 times the
/// AC-130U record's 40,000 lbf for the same airframe, weights and four T56
/// engines, and gave a loaded transport a thrust-to-weight near 0.9. The
/// AC-130U value is used.
const C130: &[RecordFit] = &[plane("thrust", 108_192, 40_000)];

/// The corrections for one aircraft identity (empty for the rest).
pub fn record_fits(id: AircraftId) -> &'static [RecordFit] {
    match id {
        AircraftId::Su25 => SU25,
        AircraftId::Mig17 => MIG17,
        AircraftId::C130 => C130,
        _ => &[],
    }
}

/// Apply `id`'s corrections when every listed field holds its retail value.
/// Returns whether they were applied.
pub(super) fn apply(
    id: AircraftId,
    object: &mut BTreeMap<String, Token>,
    plane: &mut BTreeMap<String, Token>,
) -> bool {
    let fits = record_fits(id);
    let retail = fits.iter().all(|f| {
        let map = match f.block {
            Block::Object => &*object,
            Block::Plane => &*plane,
        };
        map.get(f.key)
            .and_then(|t| t.number().ok())
            .is_some_and(|v| v == f.retail)
    });
    if fits.is_empty() || !retail {
        return false;
    }
    for f in fits {
        let map = match f.block {
            Block::Object => &mut *object,
            Block::Plane => &mut *plane,
        };
        if let Some(t) = map.get_mut(f.key) {
            t.value = f.fitted.to_string();
        }
    }
    true
}
