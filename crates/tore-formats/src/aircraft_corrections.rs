//! Corrections to retail aircraft records that are plainly copied from another
//! aircraft (slice HF3, John's requests of 2026-10-13: "V-22 tank: fix (real
//! fuel)", and "check other records for copying issues as well", which gave
//! the CH-47 its own weights; slice nr-rec added the Su-25, MiG-17F and C-130
//! rows, docs/spec/aircraft-records.md). See [aircraft records](../../../docs/formats/aircraft.md#rotorcraft-fuel-records).
//!
//! A correction applies only when every value it replaces still holds the
//! copied retail number, so a different or edited record is read as it is.
//! The corrected numbers are published figures (each row says which); they
//! are **opinionated** (John's request), the choice of figure an agent
//! decision.
use super::{AircraftId, Envelope, Token};
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

/// Mirage 2000 (M2K-1, **opinionated**, John, 2026-11-09, decision 2 in the
/// M2000 plan): TORE runs the Mirage 2000 with one engine and the thrust of
/// one real SNECMA M53-P2 instead of the retail record's 2 engines and
/// 17,687 / 32,000 lbf. 64.3 kN dry = 14,455 lbf and 95.1 kN with afterburner
/// = 21,380 lbf (MILAVIA Mirage 2000 specifications and aerospaceweb.org
/// Mirage 2000 data, both accessed 2026-11-09, as cited in the "Mirage 2000
/// source review" in docs/formats/aircraft-variety.md). The retail record
/// stays documented as retail; this override is the only place TORE reads
/// different numbers, so every consumer (flight model, damage, sound, trails,
/// AI) sees one engine.
pub const M2000_ENGINES: i32 = 1;
pub const M2000_THRUST_LBF: i32 = 14_455;
pub const M2000_AFT_THRUST_LBF: i32 = 21_380;

/// Mirage 2000 positive G limit (M2K-11, **opinionated**, John, 2026-11-10):
/// TORE gives the Mirage 2000 the real fly-by-wire limit of +9 G instead of
/// the retail record's +8 (a Rafale-template value; the record's -4 stays).
/// The real aircraft is cleared to +9 and -3.2 G (GlobalSecurity "Mirage 2000
/// Specifications", "Design and Technical Features", as cited in the Mirage
/// baseline in docs/baselines/variety-flight.md), and the F-16A and Rafale
/// records already carry +9.
pub const M2000_G_MAX: i32 = 9;
/// The retail record's G range and its +8 row (speed ft/s, altitude ft); the
/// +9 row is only added while the record still holds exactly this row.
const M2000_RETAIL_G_MIN: i32 = -4;
const M2000_RETAIL_G_MAX: i32 = 8;
const M2000_RETAIL_PLUS_8: [[f64; 2]; 7] = [
    [570., 0.],
    [675., 11_000.],
    [920., 20_000.],
    [1_235., 20_000.],
    [1_330., 18_000.],
    [1_205., 9_000.],
    [1_100., 0.],
];

/// The +9 G row fitted from the Mirage's own +8 row (**fitted**, agent
/// decision 2026-11-10): the slow side (the vertices up to the first highest
/// one) is lift-limited, so its speed at each altitude scales by
/// sqrt(9 / 8), the corner speed rising with the square root of G, rounded to
/// the record's whole ft/s; the fast side and every altitude are the +8
/// row's, so any altitude cap keeps the +8 row's shape; each fast-side
/// speed is then held to the 1 G row's top speed at its altitude, as a
/// higher row never reaches past the 1 G edge.
pub fn plus_nine_row(plus_eight: &Envelope, one_g: Option<&Envelope>) -> Envelope {
    let top = plus_eight
        .points
        .iter()
        .map(|p| p[1])
        .fold(f64::MIN, f64::max);
    let top_index = plus_eight
        .points
        .iter()
        .position(|p| p[1] == top)
        .unwrap_or(0);
    let scale = (f64::from(M2000_G_MAX) / f64::from(plus_eight.g)).sqrt();
    let points = plus_eight
        .points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i <= top_index {
                [(p[0] * scale).round(), p[1]]
            } else {
                let limit = one_g
                    .and_then(|e| e.speeds(p[1]))
                    .map_or(f64::INFINITY, |(_, fast)| fast);
                [p[0].min(limit.floor()), p[1]]
            }
        })
        .collect();
    Envelope {
        g: M2000_G_MAX,
        points,
    }
}

/// Raises the Mirage 2000's positive G limit to +9 where the record is read:
/// sets `envMax` and appends the fitted +9 row, so every consumer sees one G
/// range. Applies only to `M2000` and only while the record still holds the
/// retail range and the retail +8 row; true when it did.
pub(crate) fn raise_g_limit(
    id: AircraftId,
    plane: &mut BTreeMap<String, Token>,
    envelopes: &mut Vec<Envelope>,
) -> bool {
    if id != AircraftId::M2000 {
        return false;
    }
    let range = |key: &str| plane.get(key).and_then(|t| t.number().ok());
    if range("envMin") != Some(M2000_RETAIL_G_MIN) || range("envMax") != Some(M2000_RETAIL_G_MAX) {
        return false;
    }
    let Some(last) = envelopes.last() else {
        return false;
    };
    if last.g != M2000_RETAIL_G_MAX || last.points != M2000_RETAIL_PLUS_8 {
        return false;
    }
    let row = plus_nine_row(last, envelopes.iter().find(|e| e.g == 1));
    envelopes.push(row);
    if let Some(token) = plane.get_mut("envMax") {
        token.value = M2000_G_MAX.to_string();
    }
    true
}

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
    // Mirage 2000: one M53-P2 (opinionated, John, 2026-11-09). Not a copy
    // fix: the retail values are the Rafale template's engine count and
    // afterburner thrust.
    Correction {
        id: AircraftId::M2000,
        block: Block::Plane,
        key: "engines",
        retail: 2,
        corrected: M2000_ENGINES,
    },
    Correction {
        id: AircraftId::M2000,
        block: Block::Plane,
        key: "thrust",
        retail: 17_687,
        corrected: M2000_THRUST_LBF,
    },
    Correction {
        id: AircraftId::M2000,
        block: Block::Plane,
        key: "aftThrust",
        retail: 32_000,
        corrected: M2000_AFT_THRUST_LBF,
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
    fn the_mirage_2000_runs_one_m53_p2_and_the_retail_values_still_decode_as_retail() {
        // Opinionated (John, 2026-11-09): 1 engine, 14,455 / 21,380 lbf.
        let n = |map: &BTreeMap<String, Token>, key: &str| map[key].number().unwrap();
        let retail = || {
            BTreeMap::from([
                ("engines".to_string(), token(2)),
                ("thrust".to_string(), token(17_687)),
                ("aftThrust".to_string(), token(32_000)),
            ])
        };
        let mut o = BTreeMap::new();
        let mut p = retail();
        assert!(apply(AircraftId::M2000, &mut o, &mut p));
        assert_eq!(n(&p, "engines"), 1);
        assert_eq!(n(&p, "thrust"), 14_455);
        assert_eq!(n(&p, "aftThrust"), 21_380);
        // The same retail numbers on any other identity (the Rafale, the
        // desert record) stay retail: no aliasing.
        for other in [AircraftId::Rafale, AircraftId::F16A, AircraftId::Mig31] {
            let mut p = retail();
            assert!(!apply(other, &mut o, &mut p));
            assert_eq!(n(&p, "engines"), 2);
            assert_eq!(n(&p, "thrust"), 17_687);
            assert_eq!(n(&p, "aftThrust"), 32_000);
        }
        // A record that is not the retail one is read as it is.
        let mut p = retail();
        p.get_mut("aftThrust").unwrap().value = "31999".into();
        assert!(!apply(AircraftId::M2000, &mut o, &mut p));
        assert_eq!(n(&p, "engines"), 2);
    }

    #[test]
    fn the_plus_9_row_scales_the_slow_side_by_root_9_over_8_and_clips_the_fast_side_to_1_g() {
        let plus_eight = Envelope {
            g: 8,
            points: M2000_RETAIL_PLUS_8.to_vec(),
        };
        let fit = |one: Option<&Envelope>| plus_nine_row(&plus_eight, one).points;
        // Without a 1 G row only the slow side moves: x sqrt(9/8) = 1.06066.
        let free = fit(None);
        assert_eq!(free[0], [605., 0.]);
        assert_eq!(free[1], [716., 11_000.]);
        assert_eq!(free[2], [976., 20_000.]);
        assert_eq!(free[3..], plus_eight.points[3..]);
        // A 1 G row that tops out at 1,150 ft/s at every altitude holds the
        // fast side to 1,150.
        let one = Envelope {
            g: 1,
            points: vec![[100., 0.], [200., 60_000.], [1_150., 60_000.], [1_150., 0.]],
        };
        let clipped = fit(Some(&one));
        assert_eq!(clipped[2], [976., 20_000.]);
        for (c, f) in clipped[3..].iter().zip(&free[3..]) {
            assert_eq!(c[1], f[1]);
            assert_eq!(c[0], f[0].min(1_150.));
        }
        assert_eq!(clipped[3], [1_150., 20_000.]);
    }

    #[test]
    fn the_mirage_2000_g_correction_needs_the_retail_range_and_row() {
        let n = |map: &BTreeMap<String, Token>, key: &str| map[key].number().unwrap();
        let record = || {
            let plane = BTreeMap::from([
                ("envMin".to_string(), token(-4)),
                ("envMax".to_string(), token(8)),
            ]);
            let rows = vec![
                Envelope {
                    g: 1,
                    points: vec![[200., 0.], [900., 59_000.], [1_500., 59_000.], [1_170., 0.]],
                },
                Envelope {
                    g: 8,
                    points: M2000_RETAIL_PLUS_8.to_vec(),
                },
            ];
            (plane, rows)
        };
        let (mut p, mut rows) = record();
        assert!(raise_g_limit(AircraftId::M2000, &mut p, &mut rows));
        assert_eq!(n(&p, "envMax"), M2000_G_MAX);
        assert_eq!(rows.last().unwrap().g, 9);
        // Not for another identity, not for a record whose range or +8 row
        // differs.
        for other in [AircraftId::Rafale, AircraftId::F16A, AircraftId::Mig31] {
            let (mut p, mut rows) = record();
            assert!(!raise_g_limit(other, &mut p, &mut rows));
            assert_eq!((n(&p, "envMax"), rows.len()), (8, 2));
        }
        let (mut p, mut rows) = record();
        p.get_mut("envMin").unwrap().value = "-3".into();
        assert!(!raise_g_limit(AircraftId::M2000, &mut p, &mut rows));
        let (mut p, mut rows) = record();
        rows[1].points[6][0] = 1_101.;
        assert!(!raise_g_limit(AircraftId::M2000, &mut p, &mut rows));
        assert_eq!((n(&p, "envMax"), rows.len()), (8, 2));
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
