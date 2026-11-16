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
    fitted_row(plus_eight, one_g, M2000_G_MAX)
}

/// A row for `g` fitted from the record's own top row `base` (**fitted**,
/// agent decision 2026-11-10): the rule of [`plus_nine_row`] with the new G
/// in place of 9 and the base row's G in place of 8. The slow side scales by
/// sqrt(g / base.g), the fast side and every altitude are the base row's, and
/// each fast-side speed is held to the 1 G row's top speed at its altitude.
pub fn fitted_row(base: &Envelope, one_g: Option<&Envelope>, g: i32) -> Envelope {
    let top = base.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
    let top_index = base.points.iter().position(|p| p[1] == top).unwrap_or(0);
    let scale = (f64::from(g) / f64::from(base.g)).sqrt();
    let points = base
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
    Envelope { g, points }
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

/// Tornado IDS (B5-2, **opinionated**, John, 2026-11-10: "real thrust (two
/// RB199 Mk 103), real weights and +7.5 G"). `TOR.PT` is the Su-24 record with
/// eight numbers changed (docs/formats/aircraft-variety.md, "Tornado source
/// review"); its thrust, weights and G limit are the Fencer's. Real figures
/// (MILAVIA "Tornado specifications", accessed 2026-11-10, Tornado IDS
/// PA-200 / GR.1): two Turbo-Union RB199-34R Mk 103 at 38.48 kN (8,650 lbf)
/// dry and 71.50 kN (16,075 lbf) with reheat each, so 17,300 and 32,150 lbf for
/// the record's two engines; take-off ("empty") weight 14,091 kg (31,065 lb,
/// the figure aerospaceweb.org "Tornado IDS data" also gives) and maximum
/// take-off 27,961 kg (61,620 lb). The record's thrust is the engines'
/// total (engines stays 2). The record carries 35,270 / 50,700 lbf, 41,885 lb
/// and 90,390 lb.
pub const TORNADO_THRUST_LBF: i32 = 17_300;
pub const TORNADO_AFT_THRUST_LBF: i32 = 32_150;
pub const TORNADO_EMPTY_LB: i32 = 31_065;
pub const TORNADO_MAX_TAKEOFF_LB: i32 = 61_620;
/// The Tornado's fuel flows (B5-3, **opinionated**, John, 2026-11-10, round 2
/// item D3 "follow recs"): the record's `fuelConsumption` 6 and
/// `aftFuelConsumption` 38 are the Su-24's. The record stores whole pounds a
/// second for all the engines together, at full military power and at full
/// afterburner ([`docs/formats/aircraft.md`], "Rotorcraft fuel records"). The
/// flow is the corrected thrust times the engine's thrust-specific fuel
/// consumption, over 3,600 seconds an hour, rounded to the whole pound:
/// dry 17,300 lbf x 0.598 lb/(lbf h) / 3,600 = 2.87, so 3 lb/s; reheat
/// 32,150 lbf x 2.5 lb/(lbf h) / 3,600 = 22.3, so 22 lb/s. The SFCs are the
/// Turbo-Union RB.199 (Tornado) static sea-level figures, 0.598 dry and 2.5
/// reheat (Wikipedia "Thrust-specific fuel consumption", citing the
/// jet-engine.net engine database, accessed 2026-11-10; the Tornado ECR's
/// Mk 105 is listed at 0.637 dry). The retail 6 and 38 lb/s are the Su-24's.
pub const TORNADO_FUEL_LB_PER_S: i32 = 3;
pub const TORNADO_AB_FUEL_LB_PER_S: i32 = 22;
/// The Tornado's ceiling (B5-4, **opinionated**, John, 2026-11-11): 50,000 ft,
/// the published service ceiling (Wikipedia "Panavia Tornado" and the GR4
/// page, aerospaceweb.org "Tornado IDS data", as read for TOR-8 on
/// 2026-11-11). The record carries the Su-24's 60,000 ft, in `maxAlt` and in
/// the top edge of its 1 G polygon (0 G polygon 64,000 ft).
pub const TORNADO_CEILING_FT: i32 = 50_000;
const TORNADO_RETAIL_CEILING_FT: i32 = 60_000;
const TORNADO_RETAIL_ZERO_G_TOP_FT: i32 = 64_000;
/// The Tornado's positive G limit: **+8** (B5-4, **opinionated**, John,
/// 2026-11-11: "correct all flight model maximums ... per realistic numbers
/// that fit in TORE"; B5-2 had held it to +7). The real limit is +7.5 (the
/// encyclopedia text `TOR.INF`). Envelope rows are whole G (`envMax` and each
/// row's G are integers), so +7.5 has no row of its own. The model divides a
/// row's G by the loaded-elevator divisor (`1 + (fuel + stores) / empty
/// weight x loadedElevator / 100`), so a limit read off the top row is only
/// reached near empty. The row is chosen where the clean aircraft at half its
/// internal fuel pulls closest to the real limit without passing it by more
/// than about 0.3 G (the Mirage 2000 precedent, M2K-11): at 5,610 lb of fuel
/// the divisor is 1.072, so +7 pulls 6.53 G (0.97 under) and +8 pulls 7.46 G
/// (0.04 under; 7.9 G near empty). The retail record is -3 to +6; rows +7 and
/// +8 are fitted from the +6 row.
pub const TORNADO_G_MAX: i32 = 8;
const TORNADO_RETAIL_G_MIN: i32 = -3;
const TORNADO_RETAIL_G_MAX: i32 = 6;
/// The retail +6 row (speed ft/s, altitude ft), the base of the fitted +7 row.
const TORNADO_RETAIL_PLUS_6: [[f64; 2]; 5] = [
    [575., 0.],
    [667., 7_000.],
    [880., 14_000.],
    [1_120., 14_000.],
    [1_140., 0.],
];

/// Mirage F1 (B5-2, **opinionated**, John, 2026-11-10, decision 23 in the MF1
/// plan): one real SNECMA Atar 9K-50, 11,020 lbf dry (49.0 kN, MIGFlug "Dassault
/// Mirage F1", accessed 2026-11-10) and 15,873 lbf with afterburner (70.6 kN,
/// Jane's text in `MF1.INF`; MILAVIA "Mirage F1 specifications" gives 70.21
/// kN), as cited in the Mirage F1 source review in
/// docs/formats/aircraft-variety.md. The record already says 1 engine and
/// carries the Mirage III's 15,000 / 30,000 lbf. Weights stay retail.
pub const MF1_THRUST_LBF: i32 = 11_020;
pub const MF1_AFT_THRUST_LBF: i32 = 15_873;

/// F-5E (B5-2, **opinionated**, John, 2026-11-10: "follow the RL F-5E").
/// Internal fuel: 4,400 lb, the figure the game's own `F5E.INF` prints (677 US
/// gallons, Wikipedia "Northrop F-5", Specifications (F-5E Tiger II), is
/// about 4,500 lb); the record carries 6,240 lb (38 percent over).
pub const F5E_INTERNAL_FUEL_LB: i32 = 4_400;
/// Service ceiling 51,800 ft (15,800 m; Wikipedia "Northrop F-5",
/// Specifications (F-5E Tiger II); `F5E.INF` 15,790 m). The record's 42,000 ft
/// is the F-4 family's. `maxAlt` is the record's ceiling field; the
/// flight model's ceiling is the top of the 1 G envelope polygon, which
/// [`fit_envelopes`] moves to the same altitude.
pub const F5E_CEILING_FT: i32 = 51_800;
const F5E_RETAIL_CEILING_FT: i32 = 42_000;
/// The F-5E's positive G limit: **+8** (B5-4; B5-2 had held it to +7, see
/// [`F5E_RETAIL_STUB`] for the reasoning). The real limit is +7.33 G (T.O.
/// 1F-5E-1 "F-5E/F Flight Manual", load limit +7.33 / -3.0 G; cited secondhand
/// from the DCS World forum thread "maximum g for f5 e", accessed 2026-11-10).
/// The retail record's +8 row repeats its -4 row, so it is not an authored
/// row; B5-2 retired it and the authored +7 row became the top, B5-4 replaces
/// it with a +8 row fitted from that +7 row.
pub const F5E_G_MAX: i32 = 8;
const F5E_RETAIL_G_MIN: i32 = -4;
const F5E_RETAIL_G_MAX: i32 = 8;
/// The retail 0 G and 1 G rows. Their top edges are the record's ceiling
/// (42,000 ft for 1 G; the 0 G row reaches 2,000 ft higher).
const F5E_RETAIL_ZERO_G: [[f64; 2]; 10] = [
    [200., 0.],
    [255., 21_000.],
    [317., 30_500.],
    [418., 39_750.],
    [570., 44_000.],
    [995., 44_000.],
    [1_607., 41_000.],
    [1_940., 36_000.],
    [1_545., 18_000.],
    [1_270., 0.],
];
const F5E_RETAIL_ONE_G: [[f64; 2]; 10] = [
    [240., 0.],
    [295., 21_000.],
    [357., 30_500.],
    [458., 38_750.],
    [580., 42_000.],
    [965., 42_000.],
    [1_607., 39_000.],
    [1_840., 36_000.],
    [1_475., 18_000.],
    [1_220., 0.],
];

/// F-5E fuel flows (B5-4, **opinionated**, John, 2026-11-11): the record's
/// `fuelConsumption` 1 and `aftFuelConsumption` 8 lb/s (whole pounds a second
/// for both engines, as in [`TORNADO_FUEL_LB_PER_S`]) become the corrected
/// thrust times the J85-GE-21's thrust-specific fuel consumption (1.24 dry and
/// 2.13 reheat lb/(lbf h), Wikipedia "Thrust-specific fuel consumption", static
/// sea-level table, read 2026-11-11): 7,000 x 1.24 / 3,600 = 2.41, so 2 lb/s;
/// 10,000 x 2.13 / 3,600 = 5.92, so 6 lb/s. The whole-pound record unit costs
/// 17 percent on the dry flow (2 against 2.41) and 1 percent in reheat.
pub const F5E_FUEL_LB_PER_S: i32 = 2;
pub const F5E_AB_FUEL_LB_PER_S: i32 = 6;
/// The F-5E's G limits (B5-4, **opinionated**, John, 2026-11-11): the real
/// +7.33 now has a row, +8. The chosen row is the one where the clean
/// aircraft at half fuel (2,200 lb, divisor 1.092) pulls closest to the real
/// limit without passing it by more than 0.3 G: +7 pulls 6.41 G (0.92 under),
/// +8 pulls 7.32 G (0.01 under). The retail record's +8 row is a repeat of its
/// -4 row (B5-2 retired it); the new +8 row is fitted from the record's own +7
/// row. The -4 row is that same stub (a 190 ft/s sliver below 4,000 ft that no
/// pull reaches); the real negative limit is -3 (T.O. 1F-5E-1, as cited for
/// +7.33), which the record's -3 row already gives, so the stub is retired and
/// `envMin` is -3: at half fuel the push is 2.77 G, 0.23 G under the real -3.
const F5E_RETAIL_STUB: [[f64; 2]; 4] = [[570., 0.], [610., 4_000.], [730., 3_000.], [760., 0.]];

/// The F-5E's top-speed trim (B5-5, **opinionated**, John, 2026-11-11: "trim
/// F-5E only"; the factor is a **fitted** agent decision). The record's 1 G
/// polygon reaches 1,840 ft/s (1,090 kt, Mach 1.90) at 36,000 ft and 1,220
/// ft/s (723 kt) at sea level; the real F-5E reaches Mach 1.63 at 36,000 ft
/// (Wikipedia "Northrop F-5", Specifications (F-5E Tiger II); MILAVIA F-5
/// specifications, Mach 1.64; both read 2026-11-11), 16 percent under the
/// record. TORE's level top speed is the 1 G polygon's fast edge divided by
/// the loaded-drag factor of the shared drag law (0.937 for the F-5E at full
/// fuel, the same at every altitude), so the model flew Mach 1.78 and a
/// scale `s` of the fast edge moves it by exactly `s`. `F5E_TOP_SPEED_SCALE`
/// 0.92 gives Mach 1.64 at 36,000 ft (measured, docs/baselines/variety-flight.md).
/// No sea-level figure was found (aerospaceweb.org lists it as unknown), so
/// the sea level and every other altitude take the same factor and the record's
/// shape.
pub const F5E_TOP_SPEED_SCALE: f64 = 0.92;
/// The retail top speeds of the 1 G and 0 G polygons, ft/s, that the trim needs.
const F5E_RETAIL_ONE_G_TOP_FPS: f64 = 1_840.;
const F5E_RETAIL_ZERO_G_TOP_FPS: f64 = 1_940.;

/// F-15E (B5-4, **opinionated**, John, 2026-11-11). The thrust is John's
/// earlier decision (retail, 32,558 / 47,860 lbf) and stays.
///
/// Fuel flows: the record carries 4 and 24 lb/s (whole pounds a second for both
/// engines). The corrected flow is the record's thrust times the F100's
/// thrust-specific fuel consumption: 0.73 lb/(lbf h) at military power (the
/// F100-PW-220, the engine the record's thrust matches; Wikipedia "Pratt &
/// Whitney F100", read 2026-11-11) and 1.94 at full afterburner (the only
/// afterburning figure that page lists, under the -229): 32,558 x 0.73 / 3,600
/// = 6.60, so 7 lb/s (6 percent over); 47,860 x 1.94 / 3,600 = 25.79, so 26
/// lb/s (1 percent over).
pub const F15E_FUEL_LB_PER_S: i32 = 7;
pub const F15E_AB_FUEL_LB_PER_S: i32 = 26;
/// F-15E ceiling: 60,000 ft (18,000 m service ceiling, Wikipedia "McDonnell
/// Douglas F-15E Strike Eagle", specifications, read 2026-11-11; the game's
/// `F15.INF` 60,000 to 65,000 ft). The record carries 100,000 ft in `maxAlt`
/// and the 1 G polygon's top edge (the 0 G polygon reaches 105,000 ft).
pub const F15E_CEILING_FT: i32 = 60_000;
const F15E_RETAIL_CEILING_FT: i32 = 100_000;
const F15E_RETAIL_ZERO_G_TOP_FT: i32 = 105_000;
/// The F-15E's positive G limit: **+10** (real +9, `F15E.INF` and Wikipedia
/// "design +9 / -3"). The model divides a row's G by the loaded-elevator
/// divisor (1 + (fuel + stores) / empty weight x 43 / 100 for this record), so
/// the clean aircraft at half fuel (6,831 lb, divisor 1.103) pulls 8.28 G with
/// the record's +9 row (0.72 G under the real limit) and 9.2 G with a +10 row
/// (0.2 G over; 8.4 G at full fuel, 10 G nearly dry). A +10 row is the one
/// whole row that lands closest to the real limit at half fuel without passing
/// it by more than 0.3 G (the Mirage 2000 precedent, M2K-11). It is fitted from
/// the record's own +9 row.
pub const F15E_G_MAX: i32 = 10;
const F15E_RETAIL_G_MIN: i32 = -4;
const F15E_RETAIL_G_MAX: i32 = 9;
const F15E_RETAIL_PLUS_9: [[f64; 2]; 5] = [
    [585., 0.],
    [687., 5_500.],
    [940., 10_000.],
    [1_020., 9_000.],
    [1_180., 0.],
];
/// The F-15E record's -4 row is a sliver (910 to 950 ft/s near sea level):
/// the real negative limit is -3 (`F15.INF`, "design +9 / -3") and the -3 row
/// carries it (a 2.74 G push at half fuel, 0.26 under). The sliver is
/// retired and `envMin` is -3.
const F15E_RETAIL_STUB: [[f64; 2]; 4] = [[910., 0.], [920., 1_000.], [940., 0.], [950., 0.]];

/// Gripen (B5-4, **opinionated**, John, 2026-11-11). Thrust and weights are
/// the real JAS 39A's and stay.
///
/// Fuel flows: the record carries 2 and 14 lb/s. The RM12's thrust-specific
/// fuel consumption is 0.844 lb/(lbf h) dry and 1.79 with afterburner
/// (Wikipedia "Volvo RM12", read 2026-11-11): 12,140 x 0.844 / 3,600 = 2.85,
/// so 3 lb/s (5 percent over); 18,100 x 1.79 / 3,600 = 9.00, so 9 lb/s.
pub const GRIPEN_FUEL_LB_PER_S: i32 = 3;
pub const GRIPEN_AB_FUEL_LB_PER_S: i32 = 9;
/// Gripen ceiling: 50,000 ft (15,240 m, Wikipedia "Saab JAS 39 Gripen",
/// specifications (JAS 39C/D), read 2026-11-11). The record carries 60,000 ft
/// in `maxAlt` and in the 1 G polygon's top edge (0 G polygon 63,000 ft).
pub const GRIPEN_CEILING_FT: i32 = 50_000;
const GRIPEN_RETAIL_CEILING_FT: i32 = 60_000;
const GRIPEN_RETAIL_ZERO_G_TOP_FT: i32 = 63_000;
/// The Gripen's negative limit is -3 (real +9 / -3, same page). The record's -4
/// row is a copy of its +8 row (the record mirrors every negative row on the
/// positive row of twice the G); at half fuel (2,500 lb, divisor 1.068) it
/// pushes 3.78 G, 0.78 over the real -3, where the -3 row pushes 2.81 G. The -4
/// row is retired and `envMin` is -3. The +9 limit stays: at half fuel it pulls
/// 8.52 G (0.48 under); a +10 row would pull 9.47 G (0.47 over, more than the
/// 0.3 G allowed).
const GRIPEN_RETAIL_G_MIN: i32 = -4;
const GRIPEN_RETAIL_G_MAX: i32 = 9;
/// The Gripen's new lowest G row.
pub const GRIPEN_G_MIN: i32 = -3;

/// Mirage F1 (B5-4, **opinionated**, John, 2026-11-11). Thrust is the real
/// Atar 9K-50's (B5-2).
///
/// Fuel flow: the dry flow (3 lb/s) already matches (0.98 lb/(lbf h) against
/// 0.981); the afterburner flow of 23 lb/s was authored for 30,000 lbf and is
/// 5.2 lb/(lbf h). The corrected flow is 15,873 lbf x 1.991 lb/(lbf h) / 3,600
/// = 8.78, so 9 lb/s (2.5 percent over); the Atar 09K-50's static sea-level
/// figures, Wikipedia "Thrust-specific fuel consumption", read 2026-11-11.
pub const MF1_AB_FUEL_LB_PER_S: i32 = 9;
/// The Mirage F1's G limits: +8 and -4 rows. Real limits are +7.5 and -3.6
/// (MIGFlug "Dassault Mirage F1", "as commonly published", read 2026-11-11).
/// The clean aircraft at half fuel (3,060 lb, divisor 1.079) pulls 6.49 G with
/// the record's +7 row (1.0 under) and 7.42 G with a +8 row (0.08 under; 7.9 G
/// nearly dry), and pushes 2.78 G with the -3 row (0.82 under) and 3.71 G with
/// a -4 row (0.11 over). Both new rows are fitted: +8 from the record's own +7
/// row, and the -4 row is that same polygon, as every row of this record
/// mirrors the positive row of twice the G (-3 is +6, -2 is +4, -1 is +2).
pub const MF1_G_MAX: i32 = 8;
pub const MF1_G_MIN: i32 = -4;
const MF1_RETAIL_G_MIN: i32 = -3;
const MF1_RETAIL_G_MAX: i32 = 7;
const MF1_RETAIL_PLUS_7: [[f64; 2]; 6] = [
    [520., 0.],
    [645., 11_500.],
    [900., 20_000.],
    [1_075., 20_000.],
    [1_240., 17_000.],
    [1_230., 0.],
];

/// B-52H (BMB-A2, **opinionated**, John, 2026-11-15, decision 5 "all real").
/// `B52.PT` is the game's own authored record (no template matches it), and
/// three values are off the real B-52H (docs/formats/aircraft-variety.md,
/// "B-52 source review"): empty weight 185,000 lb (Wikipedia "Boeing B-52
/// Stratofortress", Specifications (B-52H); aircraftinformation.info B-52H
/// page, both read 2026-11-15; the record carries 235,220), internal fuel
/// 312,197 lb (47,975 US gal, the same Wikipedia page; the record carries
/// 199,000; Jane's text in `B52.INF` gives 174,130 L, about 308,000 lb) and
/// military thrust 136,000 lbf, eight TF33-P-3 at 17,000 each (Wikipedia
/// "Pratt & Whitney TF33", variants list; Jane's 75.6 kN; the record carries
/// 120,000). Maximum takeoff stays the record's 488,000 lb, which is also the
/// real figure. Empty plus full fuel is then 497,197 lb, over the maximum, as
/// on the real jet: `Loadout::new` starts the tank at what the maximum allows
/// and the loadout refuses a takeoff over it.
pub const B52_EMPTY_LB: i32 = 185_000;
pub const B52_INTERNAL_FUEL_LB: i32 = 312_197;
pub const B52_THRUST_LBF: i32 = 136_000;
/// B-52H fuel flow (BMB-A2, **opinionated**): the record's `fuelConsumption`
/// 18 lb/s (whole pounds a second for all eight engines at full military
/// power, [`TORNADO_FUEL_LB_PER_S`]) is the B-52's own authored number, not a
/// template (shared with no other record). It implies 0.476 lb/(lbf h) at the
/// corrected thrust. The corrected flow is 136,000 lbf x 0.52 lb/(lbf h) /
/// 3,600 s = 19.6, so 20 lb/s (1.8 percent over): the TF33-P-3 static
/// sea-level dry figure, 0.52 (Wikipedia "Thrust-specific fuel consumption",
/// "Dry jet engines, static, sea level", read 2026-11-15). No afterburner.
pub const B52_FUEL_LB_PER_S: i32 = 20;

/// B-2A (BMB-A2, **opinionated**, John, 2026-11-15, decision 16 "all real").
/// `B2.PT` is the B-1B record with 48 values re-authored; four are off the
/// real B-2A: thrust 69,200 lbf (four F118-GE-100 at 17,300 each, `B2.INF`
/// and Air & Space Forces Magazine "B-2 Spirit", read 2026-11-15; the record
/// carries 114,000), empty weight about 158,000 lb (public sources 158,000 to
/// 160,000, aerospaceweb "B-2 data" 150,000 to 160,000; the record's 100,000
/// is the low end of the game's own Jane's range), internal fuel about 167,000
/// lb (Whiteman AFB fact sheet; the record carries 180,000, also the low end
/// of Jane's 180,000 to 200,000) and maximum takeoff 376,000 lb (`B2.INF`,
/// aerospaceweb; the record carries 400,000). The sources are those the B2-0
/// review read (docs/formats/aircraft-variety.md, "B-2 source review").
pub const B2_THRUST_LBF: i32 = 69_200;
pub const B2_EMPTY_LB: i32 = 158_000;
pub const B2_INTERNAL_FUEL_LB: i32 = 167_000;
pub const B2_MAX_TAKEOFF_LB: i32 = 376_000;
/// B-2A fuel flow (B2-6, **opinionated**, coordinator first pass 2026-11-15,
/// held for John): the record's `fuelConsumption` 6 lb/s (whole pounds a
/// second for all four engines at full military power,
/// [`TORNADO_FUEL_LB_PER_S`]) is the B-1B's, not a B-2 figure. It implies 0.312
/// lb/(lbf h) at the corrected thrust. The corrected flow is 69,200 lbf x 0.375
/// lb/(lbf h) / 3,600 s = 7.2, so 7 lb/s (2.8 percent under): the F118-GE-100
/// thrust-specific fuel consumption of 0.375 at 69,200 lbf that BMB-A2 and the
/// coordinator's first pass cite (the page for that figure was not recorded,
/// and Wikipedia's "General Electric F118" lists none). The B-2 has no
/// afterburner, so the dry flow is the only one.
pub const B2_FUEL_LB_PER_S: i32 = 7;
/// B-2A ceiling (B2-8, **opinionated**, agent decision 2026-11-15; the
/// figure is the real one): 50,000 ft. `maxAlt` carries 49,000 ft in the
/// record. The game's own Jane's text (`B2.INF`, "Service ceiling: 15,240 m
/// (50,000 ft)"), Wikipedia "Northrop Grumman B-2 Spirit" (specifications,
/// "Service ceiling 50,000 ft") and the Air Force fact sheets all say 50,000
/// ft (read 2026-11-15). The flight model's ceiling is the top of the 1 G
/// polygon, so [`fit_envelopes`] moves the 1 G and 0 G top vertices up by the
/// same 1,000 ft (the F-117's way, BMB-A2) and nothing else in the polygons.
pub const B2_CEILING_FT: i32 = 50_000;
const B2_RETAIL_CEILING_FT: i32 = 49_000;
/// B-2A speed brake (B2-8, **fitted**): the airbrake drag word. The record
/// carries the B-1B's 256, which the hybrid adapter turns into a drag of
/// weight x 256/256 x the brake x the speed share, so the B-2's two drag
/// rudders (the only surfaces the rig opens for the brake, B2-3) pulled a
/// 325,000 lb flying wing back at 0.4 G at 312 kt and 0.8 G at 400 kt, a
/// fighter airbrake. The fit sizes the brake from the drawn plates: two plates
/// on each side (upper and lower), each about 15.2 ft along the hinge by 4.1 ft
/// of chord, opened about 50 degrees (shape units, `docs/formats/aircraft-variety.md`,
/// "B-2 rig"), so about 190 square feet of frontal plate for all four, at a
/// drag coefficient of 0.8 in the wake of a thick wing: q S Cd over the
/// 325,000 lb weight is 0.14 G at 300 kt and 0.25 G at 400 kt at sea level. The
/// model's device fraction is 0.66 at 300 kt and 0.93 at 400 kt, so the factor
/// that fits both is 0.22 to 0.27; 64 is a quarter of the template (0.25). It
/// is a fit to the drawn size, not a cited figure (no public drag-rudder drag
/// was found); the model's brake drag cannot fall with altitude as the real
/// dynamic pressure does, so at 36,000 ft it is about three times the plate
/// estimate.
pub const B2_AIRBRAKE_DRAG: i32 = 64;
const B2_RETAIL_AIRBRAKE_DRAG: i32 = 256;
/// The retail 0 G and 1 G polygons the ceiling fit needs (speed ft/s, altitude
/// ft): the 1 G row tops out at 49,000 ft, the 0 G row 3,000 ft higher.
const B2_RETAIL_ZERO_G: [[f64; 2]; 9] = [
    [170., 0.],
    [200., 15_500.],
    [290., 34_250.],
    [420., 46_625.],
    [550., 52_000.],
    [720., 51_500.],
    [855., 45_750.],
    [895., 36_000.],
    [790., 0.],
];
const B2_RETAIL_ONE_G: [[f64; 2]; 9] = [
    [220., 0.],
    [250., 15_000.],
    [350., 33_500.],
    [460., 44_750.],
    [560., 49_000.],
    [700., 48_000.],
    [800., 43_000.],
    [840., 36_000.],
    [750., 0.],
];

/// F-117A (BMB-A2, **opinionated**, John, 2026-11-15, decisions 23 and 26).
/// Maximum takeoff 52,500 lb (Wikipedia "Lockheed F-117 Nighthawk",
/// aerospaceweb "F-117 data", both read 2026-11-15; the record's 54,000 is
/// the `F105.PT` value) and ceiling 45,000 ft (Wikipedia; the record's 42,000
/// is shared with 15 records). `maxAlt` is the record's ceiling field; the
/// flight model's ceiling is the top of the 1 G polygon, which
/// [`fit_envelopes`] moves to the same altitude (the F-5E's way, B5-2).
pub const F117_MAX_TAKEOFF_LB: i32 = 52_500;
pub const F117_CEILING_FT: i32 = 45_000;
const F117_RETAIL_CEILING_FT: i32 = 42_000;
/// The F-117's high-altitude top speed (BMB-A2, John's decision 23, "about
/// Mach 0.92"; the Mach number is the real one, Wikipedia "Lockheed F-117
/// Nighthawk" infobox, aerospaceweb; the figure is **fitted**): the record's
/// 1 G fast edge reaches 1,020 ft/s (604 kt, Mach 1.05) at 36,000 ft, past the
/// speed of sound, which the real subsonic jet never was. Mach 0.92 at
/// 36,000 ft is 527.7 kt, 891 ft/s (sound 573.6 kt, 968 ft/s). The edge's
/// other vertices stay: sea level keeps the record's 1,000 ft/s (592 kt, Mach
/// 0.90), the 40,000 ft vertex its 847 ft/s (Mach 0.875). TORE's level top
/// speed is the 1 G fast edge divided by the loaded-drag factor, which is 1
/// for this record (measured, docs/baselines/variety-flight.md), so the model
/// flies the edge. The 0 G polygon keeps its 70 ft/s step over the 1 G edge
/// there (1,090 to 961), and every other row's fast side is held to the new 1
/// G edge ([`hold_to_one_g`]).
const F117_TOP_FPS: f64 = 891.;
const F117_RETAIL_TOP_FPS: f64 = 1_020.;
const F117_RETAIL_ZERO_G_TOP_FPS: f64 = 1_090.;
/// The altitude of the vertex the trim moves.
const F117_TRIM_ALTITUDE_FT: f64 = 36_000.;
/// F-117A speed brake (F117-8, **fitted**): the airbrake drag word is 0. The
/// record carries 256 (a fighter's airbrake: the hybrid adapter turns it into
/// a drag of weight x the brake x the speed share, 0.30 G at 295 kt and 0.66 G
/// at 530 kt on the 48,000 lb jet), but the real jet has no speed brake, no
/// flaps and no drag device but the landing gear and, on landing, the drag
/// chute: a pilot's account says "there are no drag devices on the jet: no
/// speed brakes, no flaps, nothing" (jetcareers.com forum, read 2026-11-15, an
/// unofficial source), and the shape has no airbrake word to draw (F117-3).
/// The record's `flapsLift` and `flapsDrag` are already 0, so this makes the
/// brake lever agree with them: it moves the lever's state but adds no drag.
pub const F117_AIRBRAKE_DRAG: i32 = 0;
const F117_RETAIL_AIRBRAKE_DRAG: i32 = 256;
/// F-117A fuel flow (F117-9, **fitted**, the coordinator's first pass,
/// 2026-11-15): the record's `fuelConsumption` is 2 lb/s for both engines at
/// full military power (0.333 lb/(lbf h) on the 21,600 lbf, a high-bypass
/// figure), which flies the model's Mach 0.82 cruise at 36,000 ft for 1,516
/// nautical miles from the 18,500 lb tank against the published 930 (F117-8,
/// Wikipedia and flugzeuginfo; 1,140 with a full bomb load, Aerocorner). A
/// flow of 3.3 lb/s reproduces 930 exactly, but the record keeps whole pounds
/// a second (a word), so the nearest whole value is 3: 1,011 nautical miles,
/// 9 percent over the published figure (4 would give 758, 18 percent under).
/// The F404's own cited 0.81 lb/(lbf h) would be 4.86 lb/s and about 630
/// nautical miles; that is not used (a fit to the one published range, not a
/// thrust-specific figure for this engine). Dry only: the record has no
/// afterburner row.
pub const F117_FUEL_LB_PER_S: i32 = 3;
const F117_RETAIL_FUEL_LB_PER_S: i32 = 2;
/// The retail 0 G and 1 G rows (speed ft/s, altitude ft). The top edges are the
/// record's ceiling (42,000 ft for 1 G; the 0 G row reaches 2,000 ft higher).
const F117_RETAIL_ZERO_G: [[f64; 2]; 9] = [
    [250., 0.],
    [265., 16_000.],
    [327., 29_500.],
    [418., 38_750.],
    [570., 44_000.],
    [715., 44_000.],
    [897., 42_000.],
    [1_090., 36_000.],
    [1_060., 0.],
];
const F117_RETAIL_ONE_G: [[f64; 2]; 9] = [
    [285., 0.],
    [305., 17_000.],
    [367., 30_500.],
    [458., 37_750.],
    [570., 42_000.],
    [715., 42_000.],
    [847., 40_000.],
    [1_020., 36_000.],
    [1_000., 0.],
];

/// B-1B Lancer (BMB-B2, **opinionated**, coordinator first pass 2026-11-15,
/// BOMBERS-DECISIONS-B B-1B item 1 "all real", pending John). `B1.PT` is the
/// game's own authored record (19 statements unique to it, no template;
/// docs/formats/aircraft-variety.md, "B-1B source review"), and four values
/// are off the real B-1B. Maximum takeoff 477,000 lb (the game's own Jane's
/// text `B1.INF`, Air & Space Forces Magazine "B-1B Lancer" and Wikipedia
/// "Rockwell B-1 Lancer"; the record's 447,000 is a digit transposition, the
/// B-1A's maximum was 395,000). Internal fuel 265,274 lb (Air Force Global
/// Strike Command B-1B fact sheet; the record's 105,000 lb is a probable typo
/// of the game's own `B1.INF` 195,000). Thrust: four F101-GE-102 at 17,390
/// lbf dry and 30,780 lbf with afterburner each (Wikipedia "General Electric
/// F101", read 2026-11-15; `B1.INF` for 30,780), 69,560 and 123,120 lbf
/// totals; the record's 58,400 (4 x 14,600) and 140,000 (4 x 35,000) are off.
/// Empty weight 192,000 lb, ceiling 55,000 ft and the G rows stay (the first
/// is real, the others have no source). The corrected empty plus full fuel is
/// 457,274 lb, under the maximum.
pub const B1B_MAX_TAKEOFF_LB: i32 = 477_000;
pub const B1B_INTERNAL_FUEL_LB: i32 = 265_274;
pub const B1B_THRUST_LBF: i32 = 69_560;
pub const B1B_AFT_THRUST_LBF: i32 = 123_120;
/// The B-1B's high-altitude top speed (BMB-B2, coordinator decision "trim to
/// Mach 1.25"; the Mach number is the real one, `B1.INF` "approx Mach 1.25"
/// and Wikipedia "Rockwell B-1 Lancer" infobox, Mach 1.25 at altitude; the
/// figure is **fitted**): the record's 1 G fast edge reaches 1,410 ft/s
/// (835 kt, Mach 1.46) at 36,000 ft, the game's own Jane's 832 knots, which is
/// Mach 1.25 at the sea-level speed of sound. Mach 1.25 above the tropopause
/// (36,089 ft; sound 968.08 ft/s, standard atmosphere) is 1,210 ft/s (716.9
/// kt), so the two 1 G vertices past it (1,400 ft/s at 41,500 ft and 1,410 at
/// 36,000) become 1,210. The 1,200 ft/s vertex at 50,000 ft is already under
/// it and stays, as does sea level (1,120 ft/s, 664 kt, Mach 1.00; the real
/// low-level figure is about Mach 0.92 but the decision names only the edge at
/// altitude). The 0 G polygon keeps its 65 ft/s step over the 1 G edge at
/// 36,000 ft (1,475 to 1,410): its two vertices at and above it become 1,275;
/// every other row's fast side is held to the new 1 G edge ([`hold_to_one_g`]).
const B1B_TOP_FPS: f64 = 1_210.;
const B1B_ZERO_G_STEP_FPS: f64 = 65.;
/// The altitude from which the trim applies (the lowest fast-edge vertex it
/// moves).
const B1B_TRIM_ALTITUDE_FT: f64 = 36_000.;
const B1B_RETAIL_ZERO_G: [[f64; 2]; 10] = [
    [170., 0.],
    [200., 15_500.],
    [290., 34_250.],
    [420., 46_625.],
    [610., 58_000.],
    [917., 57_875.],
    [1_235., 52_750.],
    [1_465., 42_375.],
    [1_475., 36_000.],
    [1_180., 0.],
];
const B1B_RETAIL_ONE_G: [[f64; 2]; 10] = [
    [220., 0.],
    [250., 15_000.],
    [350., 33_500.],
    [460., 44_750.],
    [630., 55_000.],
    [900., 55_000.],
    [1_200., 50_000.],
    [1_400., 41_500.],
    [1_410., 36_000.],
    [1_120., 0.],
];

/// Tu-160 Blackjack (BMB-B2, **opinionated**, coordinator first pass
/// 2026-11-15, BOMBERS-DECISIONS-B Tu-160 item 1 "all real", pending John).
/// `TU160.PT` is the Tu-26's record with 35 values changed, and three of the
/// authored ones are off the real Tu-160 (docs/formats/aircraft-variety.md,
/// "Tu-160 source review"). Empty weight 242,500 lb (the game's own Jane's text
/// `TU160.INF`; Wikipedia "Tupolev Tu-160" 242,508; the record's 202,260 is
/// 17 percent low). Thrust: four NK-32 at about 30,900 lbf dry (137.3 kN) and
/// 55,100 lbf with afterburner (245 kN) each (Wikipedia "Kuznetsov NK-32"
/// and "Tupolev Tu-160", read 2026-11-15), 123,600 and 220,400 lbf totals; the
/// record's 158,092 (+28 percent) and 232,398 are off. Internal fuel 326,285
/// lb (148,000 kg, aerospaceweb.org "Tu-160" as the T160-0 review read it;
/// sources disagree: `TU160.INF` 352,735, Wikipedia 286,600; the decision's
/// "about 326,000"; the record's 368,000 is above all three). Maximum takeoff
/// 606,260 lb, the four engines and the 50,000 ft ceiling stay (real, T160-0).
/// The corrected empty plus full fuel is 568,785 lb, under the maximum. The
/// top speed (1,270 kt against the real Mach 2.05, about 1,200 kt) is not
/// trimmed: the decision lists weights, fuel and thrust only.
pub const T160_EMPTY_LB: i32 = 242_500;
pub const T160_INTERNAL_FUEL_LB: i32 = 326_285;
pub const T160_THRUST_LBF: i32 = 123_600;
pub const T160_AFT_THRUST_LBF: i32 = 220_400;

/// F-111F Aardvark (BMB-B2, **opinionated**, coordinator first pass
/// 2026-11-15, BOMBERS-DECISIONS-B F-111F item 1, pending John). `F111.PT` is
/// the A-6E's record with 26 values rewritten (docs/formats/aircraft-variety.md,
/// "F-111F source review"). Internal fuel: the record's 5,025 is **gallons**,
/// not pounds (the real jet holds about 5,040 US gal, American Airpower Museum
/// F-111 page, and 5,043 elsewhere); 5,025 gal x 6.5 lb/gal (JP-4, the planning
/// figure; JP-5 is about 6.8) is 32,663 lb. Thrust: two TF30-P-100 at 14,560
/// lbf dry and 25,100 lbf with afterburner each (Wikipedia "Pratt & Whitney
/// TF30", read 2026-11-15, `F111.INF`), 29,120 and 50,200 lbf totals; the
/// record's "military" 50,200 is the real afterburner total and its 80,000
/// afterburner figure matches nothing. The corrected empty (47,481) plus full
/// fuel is 80,144 lb, under the 100,000 lb maximum.
pub const F111_INTERNAL_FUEL_LB: i32 = 32_663;
pub const F111_THRUST_LBF: i32 = 29_120;
pub const F111_AFT_THRUST_LBF: i32 = 50_200;
/// F-111F fuel flows (BMB-B2, **fitted** for an **opinionated** choice): the
/// record's `fuelConsumption` 3 and `aftFuelConsumption` 6 lb/s (whole
/// pounds a second for both engines, [`TORNADO_FUEL_LB_PER_S`]) are the
/// F-111's own authored numbers (the A-6 has 2 and 0), 3 being a TSFC of 0.37
/// at the real 29,120 lbf and 6 low for 50,200 lbf with afterburner. **No
/// TF30-P-100 specific fuel consumption was found** (Wikipedia "Pratt &
/// Whitney TF30", all-aero and Pilotfriend list none; Wikipedia "Thrust-
/// specific fuel consumption" has no TF30 row), so the flow uses the cited
/// figure of the repository's own low-bypass afterburning-turbofan analog, the
/// Turbo-Union RB.199 (0.598 lb/(lbf h) dry, 2.5 reheat, as the Tornado's
/// B5-3 flows; the JT8D-derived Volvo RM8 on the same Wikipedia page gives
/// 0.64 and 2.52 and rounds to the same pounds): dry 29,120 lbf x 0.598 /
/// 3,600 s = 4.84, so 5 lb/s; reheat 50,200 lbf x 2.5 / 3,600 = 34.9, so
/// 35 lb/s. The correction is to be replaced when a TF30 figure is found.
pub const F111_FUEL_LB_PER_S: i32 = 5;
pub const F111_AB_FUEL_LB_PER_S: i32 = 35;

/// Tu-26 Backfire-C (BMB-C2, **opinionated**, coordinator first pass
/// 2026-11-15, BOMBERS-DECISIONS-C Tu-26 item 1, pending John). `TU26.PT` is
/// the game's own record (170 of its 207 statements equal the Tu-95's, so the
/// tuning is the shared large-aircraft block, not a Backfire measurement;
/// docs/formats/aircraft-variety.md, "Tu-26 source review"). Empty weight
/// 127,868 lb (58,000 kg, Wikipedia "Tupolev Tu-22M", Tu-22M3 specification,
/// read 2026-11-15; the game's own Jane's text `TU26.INF` gives none): the
/// record's 110,230 is derived (maximum takeoff 273,370 minus fuel 110,230
/// minus the Jane's 52,910 lb weapon load), not the airframe's. Thrust: two
/// NK-25 at 33,000 lbf dry (145 kN) and 55,000 lbf with afterburner (245 kN)
/// each, 66,000 and 110,000 lbf totals (Wikipedia "Kuznetsov NK-25", read
/// 2026-11-15; Wikipedia's Tu-22M3 page gives 55,700 each, the decision takes
/// 110,000); the record's 59,986 and 88,180 are 9 and 20 percent low. The
/// fuel flows (10 and 66 lb/s) already match the NK-25's cited specific fuel
/// consumption at the real thrust (0.58 and 2.1 kg/(kgf h): 10.5 and 64.3
/// lb/s), the maximum takeoff 273,370 lb and internal fuel 110,230 lb are
/// the game's own Jane's figures (`TU26.INF`): all kept. The corrected empty
/// plus full fuel is 238,098 lb, under the maximum.
pub const TU26_EMPTY_LB: i32 = 127_868;
pub const TU26_THRUST_LBF: i32 = 66_000;
pub const TU26_AFT_THRUST_LBF: i32 = 110_000;
/// The Tu-26's ceiling (BMB-C2, **opinionated**): 43,635 ft (13,300 m,
/// `TU26.INF` and Wikipedia "Tupolev Tu-22M", read 2026-11-15). The record
/// carries 60,000 ft in `maxAlt` and the top edge of its 1 G polygon (0 G
/// polygon 65,500 ft; the G rows run to 41,000 to 54,000 ft). It is lowered
/// by [`lower_ceiling`], the Tornado's, F-15E's and Gripen's way: each polygon
/// is cut off at the new ceiling (the F-117's way of moving the top vertices
/// cannot lower a ceiling 16,365 ft past the vertices below it), the 0 G
/// polygon 5,500 ft higher, its record step.
pub const TU26_CEILING_FT: i32 = 43_635;
const TU26_RETAIL_CEILING_FT: i32 = 60_000;
const TU26_RETAIL_ZERO_G_TOP_FT: i32 = 65_500;
/// The Tu-26's high-altitude top speed (BMB-C2, decision "trim to about 1,080
/// kt"; the figure is the real one, Mach 1.88, 1,080 kt, `TU26.INF`; the fit
/// is **fitted**): the record's 1 G fast edge reaches 1,933 ft/s (1,145 kt)
/// at 36,000 ft. 1,080 kt is 1,823 ft/s. Every fast-edge vertex of the 1 G
/// polygon at or above 36,000 ft that is faster becomes 1,823 (after the
/// ceiling cut, the 36,000 ft vertex only: the cut vertex at 43,635 ft is
/// 1,812); the 0 G polygon keeps its 151 ft/s step over it (2,084 to 1,933).
/// Sea level (1,010 ft/s, 598 kt against the cited Mach 0.86, 567 kt) stays.
/// Every other row's fast side is held to the new 1 G edge
/// ([`hold_to_one_g`]).
const TU26_TOP_FPS: f64 = 1_823.;
const TU26_ZERO_G_STEP_FPS: f64 = 151.;
const TU26_TRIM_ALTITUDE_FT: f64 = 36_000.;
const TU26_RETAIL_ONE_G_TOP_FPS: f64 = 1_933.;
const TU26_RETAIL_ZERO_G_TOP_FPS: f64 = 2_084.;

/// Tu-95 Bear-H (BMB-C2, **opinionated**, coordinator first pass 2026-11-15,
/// BOMBERS-DECISIONS-C Tu-95 items 1 and 2, pending John). `TU95.PT` has no
/// template (it equals `C130.PT` in 173 of 207 statements, the shared
/// large-aircraft block; docs/formats/aircraft-variety.md, "Tu-95 source
/// review"). Thrust: the record's 355,080 is 4 x 14,795 shp x 6 exactly (the
/// NK-12MV's 11,033 kW, 14,795 ehp in the game's own Jane's text `TU95.INF`;
/// Wikipedia "Kuznetsov NK-12"), the rule that gave the C-130 108,192 before
/// John's 2026-10-13 fix ([`C130_THRUST_LBF`]). The corrected thrust is
/// **fitted**: 4 x 14,795 shp x 2.18 lbf per shp, the AC-130U's authored
/// ratio for the same kind of engine (40,000 lbf over 4 x 4,591 shp) is
/// 129,012, so 129,000 lbf. Cross-check: the review's at-speed estimate (eta
/// 0.8, an assumption, no cited static thrust was found) is 43,500 lbf at
/// 355 kt; 2.8 times that, the factor the C-130's accepted correction has over
/// the same estimate, is 121,500, within 6 percent of 129,000. Takeoff
/// thrust-to-weight at the maximum is 0.31. No afterburner, fuel flow 4 lb/s
/// stays (full power burns 5.9 lb/s at 0.36 lb/(hp h)); T95-8 may change the
/// thrust after probing takeoff and climb. Empty weight 198,415 lb (90,000
/// kg, `TU95.INF` Bear-H; Wikipedia "Tupolev Tu-95" Tu-95MS 198,416; the
/// record's 264,550 matches neither), internal fuel 148,150 lb (67,200 kg,
/// Wikipedia "Tupolev Tu-95" Tu-95MS; the record's 100,000), maximum takeoff
/// 414,470 lb (188,000 kg, `TU95.INF` Bear-H and Wikipedia; the record's
/// 407,850 is Jane's Bear-F). Ceiling and speeds stay (the 1 G row's 462 kt
/// agrees with Jane's within 3 percent, the ceiling 40,750 ft is between the
/// cited 39,370 and 45,000). The corrected empty plus full fuel is 346,565
/// lb, under the maximum.
///
/// **Update (T95-8, 2026-11-16, fitted, agent decision): 100,000 lbf, not
/// 129,000.** The model's thrust is one number that has to serve the takeoff
/// run, the climb and the cruise (a turboprop's falls with speed, the model's
/// does not). The review's cited-formula estimate at the 162 kt takeoff speed is
/// 71,600 to 95,400 lbf (eta 0.6 to 0.8); 129,000 needed a rolling term of
/// about 9.8 ft/s squared to reach the published 2,540 m run, which makes the
/// AI's taxi throttle 0.92, and climbed at 2,100 to 2,500 ft/min; 100,000 needs
/// 4.8 (the B-52H's 4.8, taxi throttle 0.58) and climbs at 1,800 to 2,700
/// ft/min against the published 2,000 (Wikipedia "Tupolev Tu-95", Tu-95MS).
pub const TU95_THRUST_LBF: i32 = 100_000;
pub const TU95_EMPTY_LB: i32 = 198_415;
pub const TU95_INTERNAL_FUEL_LB: i32 = 148_150;
pub const TU95_MAX_TAKEOFF_LB: i32 = 414_470;
/// The Tu-95's mid-altitude top speed (T95-8, **fitted**, agent decision
/// 2026-11-16): the record's 1 G fast edge is a straight line from 355 kt
/// (600 ft/s) at sea level to 462 kt (780 ft/s) at 36,000 ft, so 430 kt at
/// 25,000 ft, 14 percent under the game's own Jane's text (`TU95.INF`, Bear-H:
/// Mach 0.83, 499 kt at 25,000 ft; Mach 0.78, 447 kt at 38,000 ft; 350 kt at sea
/// level). One vertex at 25,000 ft puts the edge at 842 ft/s (499 kt, Mach
/// 0.83) there, 0 G row 55 ft/s over it (the record's own step at 36,000 ft).
/// The sea-level edge (355 kt against Jane's 350), the 36,000 ft vertex and
/// the ceiling stay. Applies only while the record's 1 G and 0 G rows are the
/// retail ones and the thrust is the corrected one.
const TU95_ONE_G_25K_FPS: f64 = 842.;
const TU95_ZERO_G_25K_FPS: f64 = 897.;
const TU95_TRIM_ALTITUDE_FT: f64 = 25_000.;
const TU95_RETAIL_ONE_G: [[f64; 2]; 8] = [
    [210., 0.],
    [250., 15_000.],
    [350., 33_500.],
    [450., 38_750.],
    [520., 40_750.],
    [730., 40_375.],
    [780., 36_000.],
    [600., 0.],
];
const TU95_RETAIL_ZERO_G: [[f64; 2]; 8] = [
    [160., 0.],
    [200., 15_500.],
    [300., 35_250.],
    [440., 41_625.],
    [520., 43_625.],
    [750., 43_750.],
    [835., 36_000.],
    [650., 0.],
];

/// Su-34 Platypus (BMB-C2, **opinionated**, coordinator first pass
/// 2026-11-15, BOMBERS-DECISIONS-C Su-34 item 1, pending John). `SU34.PT` is
/// the Su-27's record with 21 statements changed (198 of 219 equal
/// `SU27.PT`'s; docs/formats/aircraft-variety.md, "Su-34 source review"):
/// empty weight, both thrusts and the flows are the Su-27's. Empty weight
/// 49,604 lb (22,500 kg) and internal fuel 26,676 lb (12,100 kg), both
/// Wikipedia "Sukhoi Su-34" (read 2026-11-15; the record carries the Su-27's
/// 36,000 lb empty weight and 20,000 lb of fuel). Dry thrust 33,800 lbf: two AL-31F at about
/// 16,900 lbf each (the Su-27 comparison in `docs/formats/aircraft.md`, "dry
/// thrust 21 percent high, shared with SU33, SU34, SU35"; the Su-34's own
/// Wikipedia page lists no dry thrust); the record's 40,755 is the Su-27's.
/// The afterburner 59,510 lbf (two AL-31FM1 at 132 kN, 29,700 lbf each, 59,400
/// lbf, within 0.2 percent), maximum takeoff 97,000 lb (real 99,428), G
/// -4/+9, ceiling 55,000 ft and the sea-level edge stay. The corrected empty
/// plus full fuel is 76,280 lb, under the maximum.
pub const SU34_EMPTY_LB: i32 = 49_604;
pub const SU34_INTERNAL_FUEL_LB: i32 = 26_676;
pub const SU34_THRUST_LBF: i32 = 33_800;
/// The Su-34's high-altitude top speed (BMB-C2, decision "trim the
/// high-altitude edge to Mach 1.8"; the Mach number is the real one,
/// Wikipedia "Sukhoi Su-34", 1,900 km/h at altitude, about 1,030 kt; the
/// figure is **fitted**): the record's 1 G fast edge reaches 2,473 ft/s
/// (1,465 kt, Mach 2.55) at 36,000 ft. Mach 1.8 above the tropopause (36,089
/// ft; sound 968.08 ft/s, standard atmosphere) is 1,742 ft/s (1,032 kt). The
/// two 1 G vertices at or above 36,000 ft that are faster (2,308 at 49,500 ft
/// and 2,473 at 36,000 ft) become 1,742; the 55,000 ft vertex (1,704) is
/// already under it, and the 18,000 ft vertex (1,731 ft/s, Mach 1.66 there)
/// and sea level (1,250 ft/s, 741 kt against the real 775 kt) stay. The 0 G
/// polygon keeps its 127 ft/s step over the 1 G edge at 36,000 ft (2,600 to
/// 2,473); every other row's fast side is held to the new 1 G edge
/// ([`hold_to_one_g`]).
const SU34_TOP_FPS: f64 = 1_742.;
const SU34_ZERO_G_STEP_FPS: f64 = 127.;
const SU34_TRIM_ALTITUDE_FT: f64 = 36_000.;
const SU34_RETAIL_ZERO_G: [[f64; 2]; 11] = [
    [120., 0.],
    [160., 13_000.],
    [225., 27_000.],
    [313., 39_000.],
    [501., 50_000.],
    [870., 58_000.],
    [1_725., 58_000.],
    [2_417., 52_000.],
    [2_600., 36_000.],
    [1_835., 18_000.],
    [1_280., 0.],
];
const SU34_RETAIL_ONE_G: [[f64; 2]; 11] = [
    [180., 0.],
    [225., 13_000.],
    [307., 26_000.],
    [433., 39_000.],
    [601., 48_000.],
    [870., 55_000.],
    [1_704., 55_000.],
    [2_308., 49_500.],
    [2_473., 36_000.],
    [1_731., 18_000.],
    [1_250., 0.],
];

fn top_altitude(e: &Envelope) -> f64 {
    e.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max)
}

/// The polygon `points` cut off above `cap` (Sutherland-Hodgman against the
/// half-plane altitude <= `cap`): every part at or below the cap is kept as it
/// was, and each edge that crosses the cap ends on it, at the speed the edge
/// had there (rounded to the record's whole ft/s). The speed bands at every
/// altitude up to the cap are therefore the record's own.
fn clip_below(points: &[[f64; 2]], cap: f64) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = Vec::new();
    let n = points.len();
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        if a[1] <= cap {
            out.push(a);
        }
        if (a[1] <= cap) != (b[1] <= cap) {
            let t = (cap - a[1]) / (b[1] - a[1]);
            out.push([(a[0] + (b[0] - a[0]) * t).round(), cap]);
        }
    }
    out.dedup();
    if out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    out
}

/// Lowers a record's ceiling in the envelope polygons (B5-4), the companion of
/// the `maxAlt` correction: each polygon is cut off at the new ceiling
/// ([`clip_below`]), the 1 G polygon and every row but 0 G at `ceiling`, the 0
/// G polygon at the step above the 1 G polygon that the record has
/// (`retail_zero - retail_one`). Nothing below the cut moves, so every speed
/// band up to the ceiling is the record's own; the F-5E's top-edge shift of B5-2
/// is not used because these records have rows above their new ceiling. Applies
/// only when `maxAlt` has just been corrected to `ceiling` and the 1 G and 0 G
/// polygons still top out at the retail altitudes; true when it did.
fn lower_ceiling(
    object: &BTreeMap<String, Token>,
    envelopes: &mut [Envelope],
    ceiling: i32,
    retail_one: i32,
    retail_zero: i32,
) -> bool {
    if object.get("maxAlt").and_then(|t| t.number().ok()) != Some(ceiling) {
        return false;
    }
    let top_of = |g: i32| envelopes.iter().find(|e| e.g == g).map(top_altitude);
    if top_of(1) != Some(f64::from(retail_one)) || top_of(0) != Some(f64::from(retail_zero)) {
        return false;
    }
    let step = retail_zero - retail_one;
    for row in envelopes.iter_mut() {
        let cap = f64::from(if row.g == 0 { ceiling + step } else { ceiling });
        if top_altitude(row) > cap {
            row.points = clip_below(&row.points, cap);
        }
    }
    true
}

/// The index of a polygon's first highest vertex: the vertices before it are
/// the slow side, the ones after it the fast side (the decoded row layout).
fn top_index(points: &[[f64; 2]]) -> usize {
    let top = points.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
    points.iter().position(|p| p[1] == top).unwrap_or(0)
}

fn top_speed_of(e: &Envelope) -> f64 {
    e.points.iter().map(|p| p[0]).fold(f64::MIN, f64::max)
}

/// Trims a record's top speed (B5-5): multiplies the fast side of the 1 G and
/// 0 G polygons by `scale` (to the record's whole ft/s, never below the top
/// vertex's own speed), which moves the flight model's level top speed and the
/// overspeed limit by the same factor. Every other row's fast side is then held
/// to the new 1 G edge at its altitude, the rule of [`fitted_row`], so no
/// higher row reaches past the 1 G edge; a row already inside it (every low
/// altitude G row) does not move, so the pull at ordinary fighting speeds is the
/// record's own. Applies only while the 1 G and 0 G polygons still top out at
/// `retail_one` and `retail_zero` ft/s; true when it did.
fn trim_top_speed(
    envelopes: &mut [Envelope],
    scale: f64,
    retail_one: f64,
    retail_zero: f64,
) -> bool {
    let top_of =
        |envelopes: &[Envelope], g: i32| envelopes.iter().find(|e| e.g == g).map(top_speed_of);
    if top_of(envelopes, 1) != Some(retail_one) || top_of(envelopes, 0) != Some(retail_zero) {
        return false;
    }
    for row in envelopes.iter_mut().filter(|e| e.g == 0 || e.g == 1) {
        let top = top_index(&row.points);
        let slowest = row.points[top][0];
        for p in row.points.iter_mut().skip(top + 1) {
            p[0] = (p[0] * scale).round().max(slowest);
        }
    }
    hold_to_one_g(envelopes)
}

/// Holds every row but 0 G and 1 G to the 1 G polygon's fast edge at its
/// altitude (the rule of [`fitted_row`]), so no higher row reaches past it.
/// False when the record has no 1 G row.
fn hold_to_one_g(envelopes: &mut [Envelope]) -> bool {
    let Some(one) = envelopes.iter().find(|e| e.g == 1).cloned() else {
        return false;
    };
    for row in envelopes.iter_mut().filter(|e| e.g != 0 && e.g != 1) {
        let top = top_index(&row.points);
        for p in row.points.iter_mut().skip(top + 1) {
            if let Some((_, fast)) = one.speeds(p[1]) {
                p[0] = p[0].min(fast.floor());
            }
        }
    }
    true
}

/// Envelope corrections of the batch aircraft, applied where the record is
/// read: the G limits of the Tornado (+8), F-5E (+8), F-15E (+10), Gripen (-3)
/// and Mirage F1 (+8, -4), the F-5E's, F-15E's, Gripen's and Tornado's
/// ceilings. Each applies only to its own identity and only while the record
/// still holds the retail range and the retail rows it replaces; true when it
/// did. `object` is read after [`apply`], so a ceiling moves only together with
/// its `maxAlt`.
pub(crate) fn fit_envelopes(
    id: AircraftId,
    object: &BTreeMap<String, Token>,
    plane: &mut BTreeMap<String, Token>,
    envelopes: &mut Vec<Envelope>,
) -> bool {
    let range =
        |plane: &BTreeMap<String, Token>, key: &str| plane.get(key).and_then(|t| t.number().ok());
    let set = |plane: &mut BTreeMap<String, Token>, key: &str, g: i32| {
        if let Some(token) = plane.get_mut(key) {
            token.value = g.to_string();
        }
    };
    let one_g = |envelopes: &[Envelope]| envelopes.iter().find(|e| e.g == 1).cloned();
    let row_g = |envelopes: &[Envelope], g: i32| envelopes.iter().find(|e| e.g == g).cloned();
    let (min, max) = (range(plane, "envMin"), range(plane, "envMax"));
    match id {
        AircraftId::Tornado => {
            let mut done = false;
            if min == Some(TORNADO_RETAIL_G_MIN)
                && max == Some(TORNADO_RETAIL_G_MAX)
                && let Some(last) = envelopes.last().cloned()
                && last.g == TORNADO_RETAIL_G_MAX
                && last.points == TORNADO_RETAIL_PLUS_6
            {
                // Rows +7 and +8, each fitted from the retail +6 row.
                let one = one_g(envelopes);
                for g in TORNADO_RETAIL_G_MAX + 1..=TORNADO_G_MAX {
                    envelopes.push(fitted_row(&last, one.as_ref(), g));
                }
                set(plane, "envMax", TORNADO_G_MAX);
                done = true;
            }
            done | lower_ceiling(
                object,
                envelopes,
                TORNADO_CEILING_FT,
                TORNADO_RETAIL_CEILING_FT,
                TORNADO_RETAIL_ZERO_G_TOP_FT,
            )
        }
        AircraftId::F5E => {
            if min != Some(F5E_RETAIL_G_MIN)
                || max != Some(F5E_RETAIL_G_MAX)
                || envelopes.len() != 13
            {
                return false;
            }
            // The +8 row repeats the -4 row, which is itself a sliver.
            if envelopes[12].g != F5E_RETAIL_G_MAX
                || envelopes[12].points != envelopes[0].points
                || envelopes[0].points != F5E_RETAIL_STUB
            {
                return false;
            }
            // B5-2 retired the repeat; B5-4 fits a +8 row from the +7 row and
            // retires the -4 sliver (the -3 row carries the real -3 G).
            envelopes.pop();
            let plus_seven = envelopes[11].clone();
            let one = one_g(envelopes);
            envelopes.push(fitted_row(&plus_seven, one.as_ref(), F5E_G_MAX));
            envelopes.remove(0);
            set(plane, "envMin", -3);
            set(plane, "envMax", F5E_G_MAX);
            // The ceiling follows `maxAlt`: only a record whose ceiling field
            // was just corrected, with the retail 0 G and 1 G polygons, moves.
            let one = envelopes.iter().position(|e| e.g == 1);
            let zero = envelopes.iter().position(|e| e.g == 0);
            if let (Some(zero), Some(one)) = (zero, one) {
                let corrected =
                    object.get("maxAlt").and_then(|t| t.number().ok()) == Some(F5E_CEILING_FT);
                if corrected
                    && envelopes[zero].points == F5E_RETAIL_ZERO_G
                    && envelopes[one].points == F5E_RETAIL_ONE_G
                {
                    let shift = f64::from(F5E_CEILING_FT - F5E_RETAIL_CEILING_FT);
                    for row in [zero, one] {
                        let top = top_altitude(&envelopes[row]);
                        for p in &mut envelopes[row].points {
                            if p[1] == top {
                                p[1] += shift;
                            }
                        }
                    }
                }
            }
            // B5-5: the top-speed trim, after every row change above.
            trim_top_speed(
                envelopes,
                F5E_TOP_SPEED_SCALE,
                F5E_RETAIL_ONE_G_TOP_FPS,
                F5E_RETAIL_ZERO_G_TOP_FPS,
            );
            true
        }
        AircraftId::F15E => {
            let mut done = false;
            if min == Some(F15E_RETAIL_G_MIN)
                && max == Some(F15E_RETAIL_G_MAX)
                && envelopes.len() == 14
                && envelopes[0].g == F15E_RETAIL_G_MIN
                && envelopes[0].points == F15E_RETAIL_STUB
                && let Some(last) = envelopes.last().cloned()
                && last.g == F15E_RETAIL_G_MAX
                && last.points == F15E_RETAIL_PLUS_9
            {
                let one = one_g(envelopes);
                envelopes.push(fitted_row(&last, one.as_ref(), F15E_G_MAX));
                envelopes.remove(0);
                set(plane, "envMin", -3);
                set(plane, "envMax", F15E_G_MAX);
                done = true;
            }
            done | lower_ceiling(
                object,
                envelopes,
                F15E_CEILING_FT,
                F15E_RETAIL_CEILING_FT,
                F15E_RETAIL_ZERO_G_TOP_FT,
            )
        }
        AircraftId::Gripen => {
            let mut done = false;
            // The -4 row is the +8 row (the record mirrors each negative row on
            // the positive row of twice the G).
            if min == Some(GRIPEN_RETAIL_G_MIN)
                && max == Some(GRIPEN_RETAIL_G_MAX)
                && envelopes
                    .first()
                    .is_some_and(|e| e.g == GRIPEN_RETAIL_G_MIN)
                && let Some(plus_eight) = row_g(envelopes, 8)
                && envelopes[0].points == plus_eight.points
            {
                envelopes.remove(0);
                set(plane, "envMin", GRIPEN_G_MIN);
                done = true;
            }
            done | lower_ceiling(
                object,
                envelopes,
                GRIPEN_CEILING_FT,
                GRIPEN_RETAIL_CEILING_FT,
                GRIPEN_RETAIL_ZERO_G_TOP_FT,
            )
        }
        AircraftId::F117 => {
            // The ceiling and the top-speed trim follow the corrected `maxAlt`
            // and need the retail 0 G and 1 G polygons (BMB-A2).
            let corrected =
                object.get("maxAlt").and_then(|t| t.number().ok()) == Some(F117_CEILING_FT);
            let zero = envelopes.iter().position(|e| e.g == 0);
            let one = envelopes.iter().position(|e| e.g == 1);
            let (Some(zero), Some(one)) = (zero, one) else {
                return false;
            };
            if !corrected
                || envelopes[zero].points != F117_RETAIL_ZERO_G
                || envelopes[one].points != F117_RETAIL_ONE_G
            {
                return false;
            }
            let shift = f64::from(F117_CEILING_FT - F117_RETAIL_CEILING_FT);
            let step = F117_RETAIL_ZERO_G_TOP_FPS - F117_RETAIL_TOP_FPS;
            for (row, top_fps) in [(zero, F117_TOP_FPS + step), (one, F117_TOP_FPS)] {
                let top = top_altitude(&envelopes[row]);
                for p in &mut envelopes[row].points {
                    if p[1] == top {
                        p[1] += shift;
                    } else if p[1] == F117_TRIM_ALTITUDE_FT {
                        p[0] = top_fps;
                    }
                }
            }
            hold_to_one_g(envelopes);
            true
        }
        AircraftId::B1b => {
            // BMB-B2: the high-altitude top-speed trim follows the corrected
            // `maxTakeoffWeight` (the signal that the corrections ran) and
            // needs the retail 0 G and 1 G polygons, like the F-117's.
            let corrected = plane.get("maxTakeoffWeight").and_then(|t| t.number().ok())
                == Some(B1B_MAX_TAKEOFF_LB);
            let zero = envelopes.iter().position(|e| e.g == 0);
            let one = envelopes.iter().position(|e| e.g == 1);
            let (Some(zero), Some(one)) = (zero, one) else {
                return false;
            };
            if !corrected
                || envelopes[zero].points != B1B_RETAIL_ZERO_G
                || envelopes[one].points != B1B_RETAIL_ONE_G
            {
                return false;
            }
            for (row, cap) in [
                (zero, B1B_TOP_FPS + B1B_ZERO_G_STEP_FPS),
                (one, B1B_TOP_FPS),
            ] {
                let top = top_index(&envelopes[row].points);
                for p in envelopes[row].points.iter_mut().skip(top + 1) {
                    if p[1] >= B1B_TRIM_ALTITUDE_FT && p[0] > cap {
                        p[0] = cap;
                    }
                }
            }
            hold_to_one_g(envelopes);
            true
        }
        AircraftId::Tu26 => {
            // BMB-C2: the ceiling and the top-speed trim follow the corrected
            // `maxAlt` and need the retail 1 G and 0 G top speeds and top
            // altitudes (both are checked before anything moves).
            let zero = envelopes.iter().position(|e| e.g == 0);
            let one = envelopes.iter().position(|e| e.g == 1);
            let (Some(zero), Some(one)) = (zero, one) else {
                return false;
            };
            if top_speed_of(&envelopes[one]) != TU26_RETAIL_ONE_G_TOP_FPS
                || top_speed_of(&envelopes[zero]) != TU26_RETAIL_ZERO_G_TOP_FPS
                || !lower_ceiling(
                    object,
                    envelopes,
                    TU26_CEILING_FT,
                    TU26_RETAIL_CEILING_FT,
                    TU26_RETAIL_ZERO_G_TOP_FT,
                )
            {
                return false;
            }
            for (g, cap) in [(0, TU26_TOP_FPS + TU26_ZERO_G_STEP_FPS), (1, TU26_TOP_FPS)] {
                if let Some(row) = envelopes.iter_mut().find(|e| e.g == g) {
                    let top = top_index(&row.points);
                    for p in row.points.iter_mut().skip(top + 1) {
                        if p[1] >= TU26_TRIM_ALTITUDE_FT && p[0] > cap {
                            p[0] = cap;
                        }
                    }
                }
            }
            hold_to_one_g(envelopes);
            true
        }
        AircraftId::Tu95 => {
            // T95-8: the mid-altitude top speed follows the corrected `thrust`
            // (the signal that the corrections ran) and needs the retail 0 G
            // and 1 G polygons, like the B-1B's and the Su-34's.
            let corrected =
                plane.get("thrust").and_then(|t| t.number().ok()) == Some(TU95_THRUST_LBF);
            let zero = envelopes.iter().position(|e| e.g == 0);
            let one = envelopes.iter().position(|e| e.g == 1);
            let (Some(zero), Some(one)) = (zero, one) else {
                return false;
            };
            if !corrected
                || envelopes[zero].points != TU95_RETAIL_ZERO_G
                || envelopes[one].points != TU95_RETAIL_ONE_G
            {
                return false;
            }
            for (row, fps) in [(zero, TU95_ZERO_G_25K_FPS), (one, TU95_ONE_G_25K_FPS)] {
                // The fast side runs from the 36,000 ft vertex down to sea
                // level: the new vertex goes between them.
                let at = envelopes[row]
                    .points
                    .iter()
                    .position(|p| p[1] == 36_000.)
                    .map_or(envelopes[row].points.len(), |i| i + 1);
                envelopes[row]
                    .points
                    .insert(at, [fps, TU95_TRIM_ALTITUDE_FT]);
            }
            hold_to_one_g(envelopes);
            true
        }
        AircraftId::Su34 => {
            // BMB-C2: the Mach 1.8 trim follows the corrected `thrust` (the
            // signal that the corrections ran) and needs the retail 0 G and
            // 1 G polygons, like the B-1B's.
            let corrected =
                plane.get("thrust").and_then(|t| t.number().ok()) == Some(SU34_THRUST_LBF);
            let zero = envelopes.iter().position(|e| e.g == 0);
            let one = envelopes.iter().position(|e| e.g == 1);
            let (Some(zero), Some(one)) = (zero, one) else {
                return false;
            };
            if !corrected
                || envelopes[zero].points != SU34_RETAIL_ZERO_G
                || envelopes[one].points != SU34_RETAIL_ONE_G
            {
                return false;
            }
            for (row, cap) in [
                (zero, SU34_TOP_FPS + SU34_ZERO_G_STEP_FPS),
                (one, SU34_TOP_FPS),
            ] {
                let top = top_index(&envelopes[row].points);
                for p in envelopes[row].points.iter_mut().skip(top + 1) {
                    if p[1] >= SU34_TRIM_ALTITUDE_FT && p[0] > cap {
                        p[0] = cap;
                    }
                }
            }
            hold_to_one_g(envelopes);
            true
        }
        AircraftId::B2 => {
            // B2-8: the ceiling follows the corrected `maxAlt` and needs the
            // retail 0 G and 1 G polygons, like the F-117's.
            let corrected =
                object.get("maxAlt").and_then(|t| t.number().ok()) == Some(B2_CEILING_FT);
            let zero = envelopes.iter().position(|e| e.g == 0);
            let one = envelopes.iter().position(|e| e.g == 1);
            let (Some(zero), Some(one)) = (zero, one) else {
                return false;
            };
            if !corrected
                || envelopes[zero].points != B2_RETAIL_ZERO_G
                || envelopes[one].points != B2_RETAIL_ONE_G
            {
                return false;
            }
            let shift = f64::from(B2_CEILING_FT - B2_RETAIL_CEILING_FT);
            for row in [zero, one] {
                let top = top_altitude(&envelopes[row]);
                for p in &mut envelopes[row].points {
                    if p[1] == top {
                        p[1] += shift;
                    }
                }
            }
            true
        }
        AircraftId::Mf1 => {
            if min != Some(MF1_RETAIL_G_MIN) || max != Some(MF1_RETAIL_G_MAX) {
                return false;
            }
            let (Some(last), Some(plus_six)) = (envelopes.last().cloned(), row_g(envelopes, 6))
            else {
                return false;
            };
            if last.g != MF1_RETAIL_G_MAX
                || last.points != MF1_RETAIL_PLUS_7
                || envelopes
                    .first()
                    .is_none_or(|e| e.points != plus_six.points)
            {
                return false;
            }
            let plus_eight = fitted_row(&last, one_g(envelopes).as_ref(), MF1_G_MAX);
            envelopes.insert(
                0,
                Envelope {
                    g: MF1_G_MIN,
                    points: plus_eight.points.clone(),
                },
            );
            envelopes.push(plus_eight);
            set(plane, "envMin", MF1_G_MIN);
            set(plane, "envMax", MF1_G_MAX);
            true
        }
        _ => false,
    }
}

const _: () = assert!(SU25_EMPTY_LB + SU25_INTERNAL_FUEL_LB < SU25_MAX_TAKEOFF_LB);
// Batch B (BMB-B2): each real empty weight plus full real fuel fits under the
// real maximum takeoff weight, so none of the three needs the loadout's fuel
// cap (the B-1B's retail empty weight is 192,000 lb, the F-111F's 47,481 lb and
// its maximum 100,000 lb, the Tu-160's maximum 606,260 lb).
const _: () = assert!(192_000 + B1B_INTERNAL_FUEL_LB < B1B_MAX_TAKEOFF_LB);
const _: () = assert!(T160_EMPTY_LB + T160_INTERNAL_FUEL_LB < 606_260);
const _: () = assert!(47_481 + F111_INTERNAL_FUEL_LB < 100_000);
// Batch C (BMB-C2): each real empty weight plus full fuel fits under the
// maximum takeoff weight (the Tu-26's record maximum 273,370 lb, the Tu-95's
// real 414,470 lb, the Su-34's record 97,000 lb), so none needs the loadout's
// fuel cap.
const _: () = assert!(TU26_EMPTY_LB + 110_230 < 273_370);
const _: () = assert!(TU95_EMPTY_LB + TU95_INTERNAL_FUEL_LB < TU95_MAX_TAKEOFF_LB);
const _: () = assert!(SU34_EMPTY_LB + SU34_INTERNAL_FUEL_LB < 97_000);
// The Tornado's retail internal fuel (11,221 lb) fits under the real maximum.
const _: () = assert!(TORNADO_EMPTY_LB + 11_221 < TORNADO_MAX_TAKEOFF_LB);

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
    // Tornado IDS: two RB199 Mk 103, real weights (opinionated, John,
    // 2026-11-10). The retail values are the Su-24 record's.
    Correction {
        id: AircraftId::Tornado,
        block: Block::Object,
        key: "weight",
        retail: 41_885,
        corrected: TORNADO_EMPTY_LB,
    },
    Correction {
        id: AircraftId::Tornado,
        block: Block::Plane,
        key: "maxTakeoffWeight",
        retail: 90_390,
        corrected: TORNADO_MAX_TAKEOFF_LB,
    },
    Correction {
        id: AircraftId::Tornado,
        block: Block::Plane,
        key: "thrust",
        retail: 35_270,
        corrected: TORNADO_THRUST_LBF,
    },
    Correction {
        id: AircraftId::Tornado,
        block: Block::Plane,
        key: "aftThrust",
        retail: 50_700,
        corrected: TORNADO_AFT_THRUST_LBF,
    },
    // B5-3: the fuel flows of two RB199 Mk 103 (opinionated, John,
    // 2026-11-10); the retail 6 / 38 lb/s are the Su-24's.
    Correction {
        id: AircraftId::Tornado,
        block: Block::Plane,
        key: "fuelConsumption",
        retail: 6,
        corrected: TORNADO_FUEL_LB_PER_S,
    },
    Correction {
        id: AircraftId::Tornado,
        block: Block::Plane,
        key: "aftFuelConsumption",
        retail: 38,
        corrected: TORNADO_AB_FUEL_LB_PER_S,
    },
    // Mirage F1: one Atar 9K-50 (opinionated, John, 2026-11-10).
    Correction {
        id: AircraftId::Mf1,
        block: Block::Plane,
        key: "thrust",
        retail: 15_000,
        corrected: MF1_THRUST_LBF,
    },
    Correction {
        id: AircraftId::Mf1,
        block: Block::Plane,
        key: "aftThrust",
        retail: 30_000,
        corrected: MF1_AFT_THRUST_LBF,
    },
    // F-5E: the real fuel and ceiling (opinionated, John, 2026-11-10).
    Correction {
        id: AircraftId::F5E,
        block: Block::Plane,
        key: "internalFuel",
        retail: 6_240,
        corrected: F5E_INTERNAL_FUEL_LB,
    },
    Correction {
        id: AircraftId::F5E,
        block: Block::Object,
        key: "maxAlt",
        retail: F5E_RETAIL_CEILING_FT,
        corrected: F5E_CEILING_FT,
    },
    // B5-4 (opinionated, John, 2026-11-11): maxima and fuel flows per real
    // figures. F-15E: flows of two F100 at the record's thrust, 60,000 ft.
    Correction {
        id: AircraftId::F15E,
        block: Block::Plane,
        key: "fuelConsumption",
        retail: 4,
        corrected: F15E_FUEL_LB_PER_S,
    },
    Correction {
        id: AircraftId::F15E,
        block: Block::Plane,
        key: "aftFuelConsumption",
        retail: 24,
        corrected: F15E_AB_FUEL_LB_PER_S,
    },
    Correction {
        id: AircraftId::F15E,
        block: Block::Object,
        key: "maxAlt",
        retail: F15E_RETAIL_CEILING_FT,
        corrected: F15E_CEILING_FT,
    },
    // Gripen: RM12 flows, 50,000 ft.
    Correction {
        id: AircraftId::Gripen,
        block: Block::Plane,
        key: "fuelConsumption",
        retail: 2,
        corrected: GRIPEN_FUEL_LB_PER_S,
    },
    Correction {
        id: AircraftId::Gripen,
        block: Block::Plane,
        key: "aftFuelConsumption",
        retail: 14,
        corrected: GRIPEN_AB_FUEL_LB_PER_S,
    },
    Correction {
        id: AircraftId::Gripen,
        block: Block::Object,
        key: "maxAlt",
        retail: GRIPEN_RETAIL_CEILING_FT,
        corrected: GRIPEN_CEILING_FT,
    },
    // F-5E: two J85-GE-21 flows.
    Correction {
        id: AircraftId::F5E,
        block: Block::Plane,
        key: "fuelConsumption",
        retail: 1,
        corrected: F5E_FUEL_LB_PER_S,
    },
    Correction {
        id: AircraftId::F5E,
        block: Block::Plane,
        key: "aftFuelConsumption",
        retail: 8,
        corrected: F5E_AB_FUEL_LB_PER_S,
    },
    // Tornado: 50,000 ft (its fuel flows are the B5-3 rows above).
    Correction {
        id: AircraftId::Tornado,
        block: Block::Object,
        key: "maxAlt",
        retail: TORNADO_RETAIL_CEILING_FT,
        corrected: TORNADO_CEILING_FT,
    },
    // Mirage F1: the Atar 9K-50's afterburner flow (the dry 3 lb/s is right).
    Correction {
        id: AircraftId::Mf1,
        block: Block::Plane,
        key: "aftFuelConsumption",
        retail: 23,
        corrected: MF1_AB_FUEL_LB_PER_S,
    },
    // BMB-A2 (opinionated, John, 2026-11-15): the bomber batch A records.
    // B-52H: real thrust, weights and fuel, and the TF33 flow.
    Correction {
        id: AircraftId::B52,
        block: Block::Object,
        key: "weight",
        retail: 235_220,
        corrected: B52_EMPTY_LB,
    },
    Correction {
        id: AircraftId::B52,
        block: Block::Plane,
        key: "internalFuel",
        retail: 199_000,
        corrected: B52_INTERNAL_FUEL_LB,
    },
    Correction {
        id: AircraftId::B52,
        block: Block::Plane,
        key: "thrust",
        retail: 120_000,
        corrected: B52_THRUST_LBF,
    },
    Correction {
        id: AircraftId::B52,
        block: Block::Plane,
        key: "fuelConsumption",
        retail: 18,
        corrected: B52_FUEL_LB_PER_S,
    },
    // B-2A: real thrust, weights and fuel, and the F118 flow (B2-6).
    Correction {
        id: AircraftId::B2,
        block: Block::Object,
        key: "weight",
        retail: 100_000,
        corrected: B2_EMPTY_LB,
    },
    Correction {
        id: AircraftId::B2,
        block: Block::Plane,
        key: "internalFuel",
        retail: 180_000,
        corrected: B2_INTERNAL_FUEL_LB,
    },
    Correction {
        id: AircraftId::B2,
        block: Block::Plane,
        key: "thrust",
        retail: 114_000,
        corrected: B2_THRUST_LBF,
    },
    Correction {
        id: AircraftId::B2,
        block: Block::Plane,
        key: "maxTakeoffWeight",
        retail: 400_000,
        corrected: B2_MAX_TAKEOFF_LB,
    },
    Correction {
        id: AircraftId::B2,
        block: Block::Plane,
        key: "fuelConsumption",
        retail: 6,
        corrected: B2_FUEL_LB_PER_S,
    },
    // B2-8: the real ceiling and a speed brake sized from the drawn plates.
    Correction {
        id: AircraftId::B2,
        block: Block::Object,
        key: "maxAlt",
        retail: B2_RETAIL_CEILING_FT,
        corrected: B2_CEILING_FT,
    },
    Correction {
        id: AircraftId::B2,
        block: Block::Plane,
        key: "airBrakesDrag",
        retail: B2_RETAIL_AIRBRAKE_DRAG,
        corrected: B2_AIRBRAKE_DRAG,
    },
    // F-117A: the real maximum takeoff weight and ceiling, and no speed brake
    // (F117-8).
    Correction {
        id: AircraftId::F117,
        block: Block::Plane,
        key: "maxTakeoffWeight",
        retail: 54_000,
        corrected: F117_MAX_TAKEOFF_LB,
    },
    Correction {
        id: AircraftId::F117,
        block: Block::Object,
        key: "maxAlt",
        retail: F117_RETAIL_CEILING_FT,
        corrected: F117_CEILING_FT,
    },
    Correction {
        id: AircraftId::F117,
        block: Block::Plane,
        key: "airBrakesDrag",
        retail: F117_RETAIL_AIRBRAKE_DRAG,
        corrected: F117_AIRBRAKE_DRAG,
    },
    // F117-9: the fuel flow fitted to the published range (whole pounds a
    // second; 3.3 fits exactly, 3 is the nearest the record can hold).
    Correction {
        id: AircraftId::F117,
        block: Block::Plane,
        key: "fuelConsumption",
        retail: F117_RETAIL_FUEL_LB_PER_S,
        corrected: F117_FUEL_LB_PER_S,
    },
    // BMB-B2 (opinionated, coordinator first pass, 2026-11-15): the bomber
    // batch B records. B-1B: real maximum takeoff weight, fuel and thrust (the
    // top-speed trim is `fit_envelopes`).
    Correction {
        id: AircraftId::B1b,
        block: Block::Plane,
        key: "maxTakeoffWeight",
        retail: 447_000,
        corrected: B1B_MAX_TAKEOFF_LB,
    },
    Correction {
        id: AircraftId::B1b,
        block: Block::Plane,
        key: "internalFuel",
        retail: 105_000,
        corrected: B1B_INTERNAL_FUEL_LB,
    },
    Correction {
        id: AircraftId::B1b,
        block: Block::Plane,
        key: "thrust",
        retail: 58_400,
        corrected: B1B_THRUST_LBF,
    },
    Correction {
        id: AircraftId::B1b,
        block: Block::Plane,
        key: "aftThrust",
        retail: 140_000,
        corrected: B1B_AFT_THRUST_LBF,
    },
    // Tu-160: real empty weight, thrust and fuel.
    Correction {
        id: AircraftId::Tu160,
        block: Block::Object,
        key: "weight",
        retail: 202_260,
        corrected: T160_EMPTY_LB,
    },
    Correction {
        id: AircraftId::Tu160,
        block: Block::Plane,
        key: "internalFuel",
        retail: 368_000,
        corrected: T160_INTERNAL_FUEL_LB,
    },
    Correction {
        id: AircraftId::Tu160,
        block: Block::Plane,
        key: "thrust",
        retail: 158_092,
        corrected: T160_THRUST_LBF,
    },
    Correction {
        id: AircraftId::Tu160,
        block: Block::Plane,
        key: "aftThrust",
        retail: 232_398,
        corrected: T160_AFT_THRUST_LBF,
    },
    // F-111F: fuel in pounds (the record's figure is gallons), TF30-P-100
    // thrust and the flows that go with it.
    Correction {
        id: AircraftId::F111,
        block: Block::Plane,
        key: "internalFuel",
        retail: 5_025,
        corrected: F111_INTERNAL_FUEL_LB,
    },
    Correction {
        id: AircraftId::F111,
        block: Block::Plane,
        key: "thrust",
        retail: 50_200,
        corrected: F111_THRUST_LBF,
    },
    Correction {
        id: AircraftId::F111,
        block: Block::Plane,
        key: "aftThrust",
        retail: 80_000,
        corrected: F111_AFT_THRUST_LBF,
    },
    Correction {
        id: AircraftId::F111,
        block: Block::Plane,
        key: "fuelConsumption",
        retail: 3,
        corrected: F111_FUEL_LB_PER_S,
    },
    Correction {
        id: AircraftId::F111,
        block: Block::Plane,
        key: "aftFuelConsumption",
        retail: 6,
        corrected: F111_AB_FUEL_LB_PER_S,
    },
    // BMB-C2 (opinionated, coordinator first pass, 2026-11-15): the bomber
    // batch C records. Tu-26: real empty weight, NK-25 thrust and ceiling (the
    // polygons and the top-speed trim are `fit_envelopes`).
    Correction {
        id: AircraftId::Tu26,
        block: Block::Object,
        key: "weight",
        retail: 110_230,
        corrected: TU26_EMPTY_LB,
    },
    Correction {
        id: AircraftId::Tu26,
        block: Block::Plane,
        key: "thrust",
        retail: 59_986,
        corrected: TU26_THRUST_LBF,
    },
    Correction {
        id: AircraftId::Tu26,
        block: Block::Plane,
        key: "aftThrust",
        retail: 88_180,
        corrected: TU26_AFT_THRUST_LBF,
    },
    Correction {
        id: AircraftId::Tu26,
        block: Block::Object,
        key: "maxAlt",
        retail: TU26_RETAIL_CEILING_FT,
        corrected: TU26_CEILING_FT,
    },
    // Tu-95: fitted turboprop thrust, real weights, fuel and maximum takeoff.
    Correction {
        id: AircraftId::Tu95,
        block: Block::Plane,
        key: "thrust",
        retail: 355_080,
        corrected: TU95_THRUST_LBF,
    },
    Correction {
        id: AircraftId::Tu95,
        block: Block::Object,
        key: "weight",
        retail: 264_550,
        corrected: TU95_EMPTY_LB,
    },
    Correction {
        id: AircraftId::Tu95,
        block: Block::Plane,
        key: "internalFuel",
        retail: 100_000,
        corrected: TU95_INTERNAL_FUEL_LB,
    },
    Correction {
        id: AircraftId::Tu95,
        block: Block::Plane,
        key: "maxTakeoffWeight",
        retail: 407_850,
        corrected: TU95_MAX_TAKEOFF_LB,
    },
    // Su-34: real empty weight, fuel and dry thrust (the Mach 1.8 trim is
    // `fit_envelopes`, the gun rounds `aircraft_stations`).
    Correction {
        id: AircraftId::Su34,
        block: Block::Object,
        key: "weight",
        retail: 36_000,
        corrected: SU34_EMPTY_LB,
    },
    Correction {
        id: AircraftId::Su34,
        block: Block::Plane,
        key: "internalFuel",
        retail: 20_000,
        corrected: SU34_INTERNAL_FUEL_LB,
    },
    Correction {
        id: AircraftId::Su34,
        block: Block::Plane,
        key: "thrust",
        retail: 40_755,
        corrected: SU34_THRUST_LBF,
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

    fn n(map: &BTreeMap<String, Token>, key: &str) -> i32 {
        map[key].number().unwrap()
    }

    #[test]
    fn the_tornado_gets_two_rb199_mk_103s_and_its_real_weights_only_while_retail() {
        // Opinionated (John, 2026-11-10): 17,300 / 32,150 lbf, 31,065 lb empty,
        // 61,620 lb maximum take-off; the engine count (2) is already right.
        let retail = || {
            (
                BTreeMap::from([
                    ("weight".to_string(), token(41_885)),
                    ("maxAlt".to_string(), token(60_000)),
                ]),
                BTreeMap::from([
                    ("engines".to_string(), token(2)),
                    ("thrust".to_string(), token(35_270)),
                    ("aftThrust".to_string(), token(50_700)),
                    ("maxTakeoffWeight".to_string(), token(90_390)),
                    ("internalFuel".to_string(), token(11_221)),
                    ("fuelConsumption".to_string(), token(6)),
                    ("aftFuelConsumption".to_string(), token(38)),
                ]),
            )
        };
        let (mut o, mut p) = retail();
        assert!(apply(AircraftId::Tornado, &mut o, &mut p));
        // B5-3 (John, 2026-11-10): the flows of two RB199 Mk 103, the
        // corrected thrust times the cited SFC (0.598 dry, 2.5 reheat).
        assert_eq!(n(&p, "fuelConsumption"), 3);
        assert_eq!(n(&p, "aftFuelConsumption"), 22);
        assert_eq!(
            TORNADO_FUEL_LB_PER_S,
            (f64::from(TORNADO_THRUST_LBF) * 0.598 / 3600.).round() as i32
        );
        assert_eq!(
            TORNADO_AB_FUEL_LB_PER_S,
            (f64::from(TORNADO_AFT_THRUST_LBF) * 2.5 / 3600.).round() as i32
        );
        // B5-4 (John, 2026-11-11): the published 50,000 ft service ceiling.
        assert_eq!(n(&o, "maxAlt"), 50_000);
        assert_eq!(n(&o, "weight"), 31_065);
        assert_eq!(n(&p, "thrust"), 17_300);
        assert_eq!(n(&p, "aftThrust"), 32_150);
        assert_eq!(n(&p, "maxTakeoffWeight"), 61_620);
        assert_eq!((n(&p, "engines"), n(&p, "internalFuel")), (2, 11_221));
        // The same Su-24 numbers on another identity stay retail (no aliasing).
        for other in [AircraftId::Mig23, AircraftId::Su25, AircraftId::F15E] {
            let (mut o, mut p) = retail();
            assert!(!apply(other, &mut o, &mut p));
            assert_eq!((n(&o, "weight"), n(&p, "thrust")), (41_885, 35_270));
            assert_eq!((n(&o, "maxAlt"), n(&p, "aftFuelConsumption")), (60_000, 38));
        }
        // An edited record is read as it is (a changed flow included).
        let (mut o, mut p) = retail();
        p.get_mut("aftFuelConsumption").unwrap().value = "37".into();
        assert!(!apply(AircraftId::Tornado, &mut o, &mut p));
        assert_eq!(n(&p, "fuelConsumption"), 6);
        let (mut o, mut p) = retail();
        p.get_mut("aftThrust").unwrap().value = "50701".into();
        assert!(!apply(AircraftId::Tornado, &mut o, &mut p));
        assert_eq!((n(&o, "weight"), n(&p, "thrust")), (41_885, 35_270));
    }

    #[test]
    fn the_mirage_f1_runs_one_atar_9k_50_and_keeps_its_retail_weights() {
        // Opinionated (John, 2026-11-10): 11,020 / 15,873 lbf, one engine.
        let retail = || {
            BTreeMap::from([
                ("engines".to_string(), token(1)),
                ("thrust".to_string(), token(15_000)),
                ("aftThrust".to_string(), token(30_000)),
                ("maxTakeoffWeight".to_string(), token(30_203)),
                ("fuelConsumption".to_string(), token(3)),
                ("aftFuelConsumption".to_string(), token(23)),
            ])
        };
        let mut o = BTreeMap::from([("weight".to_string(), token(15_542))]);
        let mut p = retail();
        assert!(apply(AircraftId::Mf1, &mut o, &mut p));
        // B5-4 (John, 2026-11-11): the Atar's afterburner flow, the corrected
        // thrust times the cited 1.991 lb/(lbf h) (8.78 lb/s, so 9); the dry
        // 3 lb/s already matches (11,020 x 0.981 / 3,600 = 3.00).
        assert_eq!(n(&p, "aftFuelConsumption"), 9);
        assert_eq!(n(&p, "fuelConsumption"), 3);
        assert_eq!(
            MF1_AB_FUEL_LB_PER_S,
            (f64::from(MF1_AFT_THRUST_LBF) * 1.991 / 3600.).round() as i32
        );
        assert_eq!(n(&p, "thrust"), 11_020);
        assert_eq!(n(&p, "aftThrust"), 15_873);
        assert_eq!((n(&p, "engines"), n(&p, "maxTakeoffWeight")), (1, 30_203));
        assert_eq!(n(&o, "weight"), 15_542);
        // The Mirage III template numbers on another identity stay retail.
        for other in [AircraftId::M2000, AircraftId::Rafale, AircraftId::F5E] {
            let mut p = retail();
            assert!(!apply(other, &mut o, &mut p));
            assert_eq!((n(&p, "thrust"), n(&p, "aftThrust")), (15_000, 30_000));
        }
        let mut p = retail();
        p.get_mut("thrust").unwrap().value = "15001".into();
        assert!(!apply(AircraftId::Mf1, &mut o, &mut p));
        assert_eq!(n(&p, "aftThrust"), 30_000);
    }

    #[test]
    fn the_f5e_gets_its_real_fuel_and_ceiling_only_while_retail() {
        let retail = || {
            (
                BTreeMap::from([("maxAlt".to_string(), token(42_000))]),
                BTreeMap::from([
                    ("internalFuel".to_string(), token(6_240)),
                    ("thrust".to_string(), token(7_000)),
                    ("aftThrust".to_string(), token(10_000)),
                    ("fuelConsumption".to_string(), token(1)),
                    ("aftFuelConsumption".to_string(), token(8)),
                ]),
            )
        };
        let (mut o, mut p) = retail();
        assert!(apply(AircraftId::F5E, &mut o, &mut p));
        assert_eq!(n(&p, "internalFuel"), 4_400);
        // B5-4 (John, 2026-11-11): two J85-GE-21 at 1.24 dry and 2.13 reheat
        // lb/(lbf h): 2.41 and 5.92 lb/s, so 2 and 6.
        assert_eq!(n(&p, "fuelConsumption"), 2);
        assert_eq!(n(&p, "aftFuelConsumption"), 6);
        assert_eq!(F5E_FUEL_LB_PER_S, (7_000. * 1.24 / 3600_f64).round() as i32);
        assert_eq!(
            F5E_AB_FUEL_LB_PER_S,
            (10_000. * 2.13 / 3600_f64).round() as i32
        );
        assert_eq!(n(&o, "maxAlt"), 51_800);
        // Thrust, which is already the real F-5E's, is untouched.
        assert_eq!(n(&p, "thrust"), 7_000);
        // The F-4 family's 42,000 ft ceiling on another identity stays.
        for other in [AircraftId::F4E, AircraftId::F104, AircraftId::A7] {
            let (mut o, mut p) = retail();
            assert!(!apply(other, &mut o, &mut p));
            assert_eq!((n(&o, "maxAlt"), n(&p, "internalFuel")), (42_000, 6_240));
        }
        let (mut o, mut p) = retail();
        p.get_mut("internalFuel").unwrap().value = "6241".into();
        assert!(!apply(AircraftId::F5E, &mut o, &mut p));
        assert_eq!(n(&o, "maxAlt"), 42_000);
    }

    #[test]
    fn the_tu95_gains_one_mid_altitude_vertex_only_on_the_retail_rows() {
        let rows = || {
            vec![
                Envelope {
                    g: 0,
                    points: TU95_RETAIL_ZERO_G.to_vec(),
                },
                Envelope {
                    g: 1,
                    points: TU95_RETAIL_ONE_G.to_vec(),
                },
                Envelope {
                    g: 2,
                    points: vec![[250., 0.], [670., 37_000.], [720., 34_000.], [590., 0.]],
                },
            ]
        };
        let object = BTreeMap::new();
        let mut plane = BTreeMap::from([("thrust".to_string(), token(TU95_THRUST_LBF))]);
        let mut rows_a = rows();
        assert!(fit_envelopes(
            AircraftId::Tu95,
            &object,
            &mut plane,
            &mut rows_a
        ));
        let at = |rows: &[Envelope], g: i32, alt: f64| {
            rows.iter()
                .find(|e| e.g == g)
                .unwrap()
                .speeds(alt)
                .unwrap()
                .1
        };
        // 842 ft/s (499 kt, Mach 0.83) at 25,000 ft, the 0 G row 55 ft/s over
        // it; the sea-level and 36,000 ft vertices are the record's.
        assert_eq!(at(&rows_a, 1, 25_000.), 842.);
        assert_eq!(at(&rows_a, 0, 25_000.), 897.);
        assert_eq!(at(&rows_a, 1, 0.), 600.);
        assert_eq!(at(&rows_a, 1, 36_000.), 780.);
        assert_eq!(rows_a[1].points.len(), 9);
        // The rows above 1 G stay inside the 1 G edge, and nothing else moved.
        assert!(at(&rows_a, 2, 25_000.) <= 842.);
        assert_eq!(rows_a[2].points, rows()[2].points);
        // Another identity, the retail thrust or an edited row: unchanged.
        let mut rows_b = rows();
        assert!(!fit_envelopes(
            AircraftId::Tu26,
            &object,
            &mut plane,
            &mut rows_b
        ));
        let mut retail = BTreeMap::from([("thrust".to_string(), token(355_080))]);
        assert!(!fit_envelopes(
            AircraftId::Tu95,
            &object,
            &mut retail,
            &mut rows_b
        ));
        rows_b[1].points[2][0] = 351.;
        assert!(!fit_envelopes(
            AircraftId::Tu95,
            &object,
            &mut plane,
            &mut rows_b
        ));
        assert_eq!(rows_b[1].points.len(), 8);
    }

    fn tornado_record() -> (BTreeMap<String, Token>, Vec<Envelope>) {
        let plane = BTreeMap::from([
            ("envMin".to_string(), token(-3)),
            ("envMax".to_string(), token(6)),
        ]);
        let mut rows: Vec<Envelope> = (-3..=5)
            .map(|g| Envelope {
                g,
                points: vec![[100., 0.], [200., 20_000.], [900., 20_000.], [800., 0.]],
            })
            .collect();
        // The 1 G row's fast edge: 1,360 ft/s at sea level to 2,310 at 36,000 ft.
        rows[4].points = vec![
            [200., 0.],
            [870., 60_000.],
            [1_484., 60_000.],
            [2_310., 36_000.],
            [1_360., 0.],
        ];
        rows.push(Envelope {
            g: 6,
            points: TORNADO_RETAIL_PLUS_6.to_vec(),
        });
        (plane, rows)
    }

    #[test]
    fn the_tornado_gets_fitted_plus_7_and_plus_8_rows_from_its_own_plus_6_row() {
        let (mut plane, mut rows) = tornado_record();
        let none = BTreeMap::new();
        assert!(fit_envelopes(
            AircraftId::Tornado,
            &none,
            &mut plane,
            &mut rows
        ));
        assert_eq!(n(&plane, "envMax"), 8);
        assert_eq!(rows.len(), 12);
        let plus_seven = &rows[10];
        assert_eq!(plus_seven.g, 7);
        // The slow side scales by sqrt(7 / 6) = 1.0801 and rounds; the fast
        // side and every altitude are the +6 row's.
        assert_eq!(
            plus_seven.points,
            vec![
                [621., 0.],
                [720., 7_000.],
                [951., 14_000.],
                [1_120., 14_000.],
                [1_140., 0.]
            ]
        );
        // B5-4: +8, sqrt(8 / 6) = 1.1547 on the slow side (575 to 664, 667 to
        // 770, 880 to 1,016).
        let top = rows.last().unwrap();
        assert_eq!(top.g, 8);
        assert_eq!(
            top.points,
            vec![
                [664., 0.],
                [770., 7_000.],
                [1_016., 14_000.],
                [1_120., 14_000.],
                [1_140., 0.]
            ]
        );
        // The +6 row itself is untouched.
        assert_eq!(rows[9].points, TORNADO_RETAIL_PLUS_6.to_vec());
        // Another identity, a different range or an edited +6 row: unchanged.
        for other in [AircraftId::Mig23, AircraftId::Su25, AircraftId::F5E] {
            let (mut plane, mut rows) = tornado_record();
            assert!(!fit_envelopes(other, &none, &mut plane, &mut rows));
            assert_eq!((n(&plane, "envMax"), rows.len()), (6, 10));
        }
        let (mut plane, mut rows) = tornado_record();
        plane.get_mut("envMin").unwrap().value = "-4".into();
        assert!(!fit_envelopes(
            AircraftId::Tornado,
            &none,
            &mut plane,
            &mut rows
        ));
        let (mut plane, mut rows) = tornado_record();
        rows[9].points[2][0] = 881.;
        assert!(!fit_envelopes(
            AircraftId::Tornado,
            &none,
            &mut plane,
            &mut rows
        ));
        assert_eq!((n(&plane, "envMax"), rows.len()), (6, 10));
    }

    #[test]
    fn the_tornado_flies_to_50_000_ft_only_with_the_corrected_ceiling_field() {
        let corrected = BTreeMap::from([("maxAlt".to_string(), token(50_000))]);
        let top = |e: &Envelope| e.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
        let (mut plane, mut rows) = tornado_record();
        // The 0 G polygon reaches 64,000 ft, the 1 G polygon 60,000 ft, and
        // the 2 G polygon 52,000 ft; a 3 G polygon at 44,000 ft is under the
        // new ceiling and is left alone.
        rows[3].points = vec![[140., 0.], [820., 64_000.], [1_514., 64_000.], [1_420., 0.]];
        rows[5].points = vec![
            [270., 0.],
            [600., 44_250.],
            [870., 52_000.],
            [1_375., 52_000.],
            [2_007., 39_500.],
            [1_290., 0.],
        ];
        rows[6].points = vec![[345., 0.], [870., 44_000.], [1_305., 44_000.], [1_250., 0.]];
        let three = rows[6].points.clone();
        assert!(fit_envelopes(
            AircraftId::Tornado,
            &corrected,
            &mut plane,
            &mut rows
        ));
        // 0 G keeps its 4,000 ft step above the 1 G polygon.
        assert_eq!(top(&rows[3]), 54_000.);
        assert_eq!(top(&rows[4]), 50_000.);
        // The polygons are cut off at 50,000 ft: each edge that crossed it ends
        // on it at the speed it had there (the 1 G slow edge, 200 ft/s at sea
        // level to 870 at 60,000 ft, is 758 at 50,000 ft; the fast edge from
        // 1,484 at 60,000 ft to 2,310 at 36,000 ft is 1,828 there), and every
        // vertex at or below it stays.
        assert_eq!(
            rows[3].points,
            vec![[140., 0.], [714., 54_000.], [1_499., 54_000.], [1_420., 0.]]
        );
        assert_eq!(
            rows[4].points,
            vec![
                [200., 0.],
                [758., 50_000.],
                [1_828., 50_000.],
                [2_310., 36_000.],
                [1_360., 0.]
            ]
        );
        assert_eq!(
            rows[5].points,
            vec![
                [270., 0.],
                [600., 44_250.],
                [800., 50_000.],
                [1_476., 50_000.],
                [2_007., 39_500.],
                [1_290., 0.]
            ]
        );
        assert_eq!(rows[6].points, three);
        // With the retail ceiling field still on the record the polygons stay.
        let stale = BTreeMap::from([("maxAlt".to_string(), token(60_000))]);
        let (mut plane, mut rows) = tornado_record();
        assert!(fit_envelopes(
            AircraftId::Tornado,
            &stale,
            &mut plane,
            &mut rows
        ));
        assert_eq!(top(&rows[4]), 60_000.);
    }

    fn f5e_record() -> (BTreeMap<String, Token>, Vec<Envelope>) {
        let plane = BTreeMap::from([
            ("envMin".to_string(), token(-4)),
            ("envMax".to_string(), token(8)),
        ]);
        let mut rows: Vec<Envelope> = (-4..=8)
            .map(|g| Envelope {
                g,
                points: vec![[300. + f64::from(g), 0.], [400., 9_000.], [900., 9_000.]],
            })
            .collect();
        rows[4].points = F5E_RETAIL_ZERO_G.to_vec();
        rows[5].points = F5E_RETAIL_ONE_G.to_vec();
        // The +7 row (retail, 525@0 to 800@0).
        rows[11].points = vec![[525., 0.], [580., 7_000.], [780., 6_000.], [800., 0.]];
        // The +8 row repeats the -4 row (570@0, 610@4000, 730@3000, 760@0).
        rows[0].points = F5E_RETAIL_STUB.to_vec();
        rows[12].points = F5E_RETAIL_STUB.to_vec();
        (plane, rows)
    }

    #[test]
    fn the_f5e_tops_out_at_plus_8_with_a_fitted_row_and_flies_to_51_800_ft() {
        let object = BTreeMap::from([("maxAlt".to_string(), token(51_800))]);
        let (mut plane, mut rows) = f5e_record();
        assert!(fit_envelopes(
            AircraftId::F5E,
            &object,
            &mut plane,
            &mut rows
        ));
        // B5-4 (John, 2026-11-11): the -4 sliver is retired (-3 is the real
        // negative limit) and the repeated +8 row is replaced by a +8 row
        // fitted from the +7 row (sqrt(8 / 7) = 1.069 on the slow side).
        assert_eq!((n(&plane, "envMin"), n(&plane, "envMax")), (-3, 8));
        assert_eq!(rows.len(), 12);
        assert_eq!((rows[0].g, rows.last().unwrap().g), (-3, 8));
        assert_eq!(
            rows[10].points,
            vec![[525., 0.], [580., 7_000.], [780., 6_000.], [800., 0.]]
        );
        assert_eq!(
            rows[11].points,
            vec![[561., 0.], [620., 7_000.], [780., 6_000.], [800., 0.]]
        );
        // The 1 G and 0 G top edges move by 9,800 ft; the slow side does not.
        // B5-5: their fast sides are trimmed by 0.92 (the vertices after the
        // first highest one), whole ft/s.
        let one = rows.iter().find(|e| e.g == 1).unwrap();
        assert_eq!(one.points[3], [458., 38_750.]);
        assert_eq!(one.points[4], [580., 51_800.]);
        assert_eq!(one.points[5], [888., 51_800.]);
        assert_eq!(one.points[6], [1_478., 39_000.]);
        assert_eq!(one.points[7], [1_693., 36_000.]);
        assert_eq!(one.points[8], [1_357., 18_000.]);
        assert_eq!(one.points[9], [1_122., 0.]);
        let zero = rows.iter().find(|e| e.g == 0).unwrap();
        assert_eq!(zero.points[4], [570., 53_800.]);
        assert_eq!(zero.points[5], [915., 53_800.]);
        assert_eq!(zero.points[6], [1_478., 41_000.]);
        assert_eq!(zero.points[7], [1_785., 36_000.]);
        let top_alt = |e: &Envelope| e.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
        assert_eq!(top_alt(one), 51_800.);
        // The retail ceiling field still on the record (an edited or partly
        // corrected record) keeps its retail rows, but still gets the G rows.
        let stale = BTreeMap::from([("maxAlt".to_string(), token(42_000))]);
        let (mut plane, mut rows) = f5e_record();
        assert!(fit_envelopes(
            AircraftId::F5E,
            &stale,
            &mut plane,
            &mut rows
        ));
        assert_eq!(top_alt(rows.iter().find(|e| e.g == 1).unwrap()), 42_000.);
        assert_eq!((n(&plane, "envMin"), n(&plane, "envMax")), (-3, 8));
    }

    #[test]
    fn the_f5e_envelope_correction_needs_the_retail_range_and_repeat_row() {
        let object = BTreeMap::from([("maxAlt".to_string(), token(51_800))]);
        for other in [AircraftId::F4E, AircraftId::F104, AircraftId::Mf1] {
            let (mut plane, mut rows) = f5e_record();
            assert!(!fit_envelopes(other, &object, &mut plane, &mut rows));
            assert_eq!((n(&plane, "envMax"), rows.len()), (8, 13));
        }
        // A +8 row that is not the -4 row is authored: the record is left alone.
        let (mut plane, mut rows) = f5e_record();
        rows[12].points[1][0] += 1.;
        assert!(!fit_envelopes(
            AircraftId::F5E,
            &object,
            &mut plane,
            &mut rows
        ));
        assert_eq!((n(&plane, "envMax"), rows.len()), (8, 13));
        // A -4 row that is not the sliver is authored too.
        let (mut plane, mut rows) = f5e_record();
        rows[0].points[1][0] += 1.;
        rows[12].points[1][0] += 1.;
        assert!(!fit_envelopes(
            AircraftId::F5E,
            &object,
            &mut plane,
            &mut rows
        ));
        let (mut plane, mut rows) = f5e_record();
        plane.get_mut("envMax").unwrap().value = "9".into();
        assert!(!fit_envelopes(
            AircraftId::F5E,
            &object,
            &mut plane,
            &mut rows
        ));
    }

    #[test]
    fn the_f5e_top_speed_trim_scales_the_fast_edge_and_keeps_the_other_rows() {
        let object = BTreeMap::from([("maxAlt".to_string(), token(51_800))]);
        let (mut plane, mut rows) = f5e_record();
        // A high-altitude row that reaches past the trimmed 1 G edge: its fast
        // vertex is held to the new edge at its altitude, never raised.
        rows[7].points = vec![[320., 0.], [400., 36_000.], [1_700., 36_000.]];
        let before_low = rows[11].points.clone();
        assert!(fit_envelopes(
            AircraftId::F5E,
            &object,
            &mut plane,
            &mut rows
        ));
        let speed = |g: i32| top_speed_of(rows.iter().find(|e| e.g == g).unwrap());
        // 1,840 ft/s x 0.92 = 1,692.8, so 1,693; the record's top edge was 1,840.
        assert_eq!(speed(1), 1_693.);
        assert_eq!(speed(0), 1_785.);
        assert!((speed(1) / F5E_RETAIL_ONE_G_TOP_FPS - F5E_TOP_SPEED_SCALE).abs() < 1e-3);
        // The +8 row's fast side (780 at 6,000 ft and 800 at sea level) is
        // inside the trimmed 1 G edge and stays the record's own.
        assert_eq!(
            rows.last().unwrap().points,
            vec![[561., 0.], [620., 7_000.], [780., 6_000.], [800., 0.]]
        );
        assert_eq!(rows[10].points, before_low);
        // The +3 row's 1,700 ft/s at 36,000 ft is held to the 1 G edge there.
        let three = rows.iter().find(|e| e.g == 3).unwrap();
        assert_eq!(three.points[2], [1_693., 36_000.]);
        assert_eq!(three.points[..2], [[320., 0.], [400., 36_000.]]);
        // No row passes the 1 G edge at its own fast-side altitudes.
        let one = rows.iter().find(|e| e.g == 1).unwrap().clone();
        for row in rows.iter().filter(|e| e.g > 1) {
            for p in row.points.iter().skip(top_index(&row.points) + 1) {
                let (_, fast) = one.speeds(p[1]).unwrap();
                assert!(p[0] <= fast, "g {} {:?} past {}", row.g, p, fast);
            }
        }
    }

    #[test]
    fn the_f5e_top_speed_trim_needs_the_retail_top_speeds() {
        let object = BTreeMap::from([("maxAlt".to_string(), token(51_800))]);
        // A 1 G polygon whose top speed is not the retail 1,840 ft/s: the G
        // rows and ceiling still correct, the speeds are left alone.
        let (mut plane, mut rows) = f5e_record();
        rows[5].points[7][0] = 1_800.;
        assert!(fit_envelopes(
            AircraftId::F5E,
            &object,
            &mut plane,
            &mut rows
        ));
        assert_eq!(
            top_speed_of(rows.iter().find(|e| e.g == 1).unwrap()),
            1_800.
        );
        assert_eq!(
            top_speed_of(rows.iter().find(|e| e.g == 0).unwrap()),
            1_940.
        );
        // Likewise a 0 G polygon that is not the retail one.
        let (mut plane, mut rows) = f5e_record();
        rows[4].points[7][0] = 1_900.;
        assert!(fit_envelopes(
            AircraftId::F5E,
            &object,
            &mut plane,
            &mut rows
        ));
        assert_eq!(
            top_speed_of(rows.iter().find(|e| e.g == 1).unwrap()),
            1_840.
        );
        // Applied twice (a record already read once): the second pass finds
        // 1,693 ft/s, not the retail edge, and does not trim again.
        let (mut plane, mut rows) = f5e_record();
        assert!(fit_envelopes(
            AircraftId::F5E,
            &object,
            &mut plane,
            &mut rows
        ));
        let once = rows.clone();
        assert!(!trim_top_speed(
            &mut rows,
            F5E_TOP_SPEED_SCALE,
            F5E_RETAIL_ONE_G_TOP_FPS,
            F5E_RETAIL_ZERO_G_TOP_FPS
        ));
        assert_eq!(rows, once);
        // No other identity is trimmed.
        for other in [AircraftId::F4E, AircraftId::F104, AircraftId::Mf1] {
            let (mut plane, mut rows) = f5e_record();
            assert!(!fit_envelopes(other, &object, &mut plane, &mut rows));
            assert_eq!(
                top_speed_of(rows.iter().find(|e| e.g == 1).unwrap()),
                1_840.
            );
        }
    }

    /// A synthetic record shaped like the F-15E's: rows -4 (the sliver) to +9,
    /// ceilings at 100,000 ft (1 G) and 105,000 ft (0 G).
    fn f15e_record() -> (BTreeMap<String, Token>, Vec<Envelope>) {
        let plane = BTreeMap::from([
            ("envMin".to_string(), token(-4)),
            ("envMax".to_string(), token(9)),
        ]);
        let mut rows: Vec<Envelope> = (-4..=9)
            .map(|g| Envelope {
                g,
                points: vec![
                    [300. + f64::from(g), 0.],
                    [500., 20_000.],
                    [1_500., 20_000.],
                    [1_300., 0.],
                ],
            })
            .collect();
        rows[0].points = F15E_RETAIL_STUB.to_vec();
        rows[4].points = vec![
            [140., 0.],
            [531., 74_000.],
            [870., 105_000.],
            [1_574., 105_000.],
            [2_297., 75_500.],
            [1_440., 0.],
        ];
        rows[5].points = vec![
            [200., 0.],
            [563., 65_000.],
            [801., 96_000.],
            [870., 100_000.],
            [1_554., 100_000.],
            [2_183., 74_250.],
            [1_360., 0.],
        ];
        // The 2 G polygon reaches 85,000 ft; the 4 G polygon, 54,000 ft.
        rows[6].points = vec![
            [250., 0.],
            [900., 85_000.],
            [1_635., 85_000.],
            [2_007., 71_000.],
            [1_310., 0.],
        ];
        rows[8].points = vec![[365., 0.], [930., 54_000.], [1_440., 54_000.], [1_270., 0.]];
        rows[13].points = F15E_RETAIL_PLUS_9.to_vec();
        (plane, rows)
    }

    #[test]
    fn the_f15e_gets_a_fitted_plus_10_row_and_loses_its_sliver_row() {
        let object = BTreeMap::from([("maxAlt".to_string(), token(100_000))]);
        let (mut plane, mut rows) = f15e_record();
        assert!(fit_envelopes(
            AircraftId::F15E,
            &object,
            &mut plane,
            &mut rows
        ));
        // -3 to +10, 14 rows, the +9 row untouched.
        assert_eq!((n(&plane, "envMin"), n(&plane, "envMax")), (-3, 10));
        let gs: Vec<i32> = rows.iter().map(|e| e.g).collect();
        assert_eq!(gs, (-3..=10).collect::<Vec<_>>());
        assert_eq!(rows[12].points, F15E_RETAIL_PLUS_9.to_vec());
        // sqrt(10 / 9) = 1.0541 on the slow side (585 to 617, 687 to 724, 940
        // to 991); the fast side and every altitude are the +9 row's.
        assert_eq!(
            rows[13].points,
            vec![
                [617., 0.],
                [724., 5_500.],
                [991., 10_000.],
                [1_020., 9_000.],
                [1_180., 0.]
            ]
        );
        // The sliver is gone and no -3 row moved.
        assert!(rows.iter().all(|e| e.points != F15E_RETAIL_STUB.to_vec()));
        // Another identity (the F-15C carries the same rows), an authored -4
        // row, a +9 row of another shape: unchanged.
        for other in [AircraftId::F15, AircraftId::Gripen, AircraftId::F5E] {
            let (mut plane, mut rows) = f15e_record();
            assert!(!fit_envelopes(other, &object, &mut plane, &mut rows));
            assert_eq!((n(&plane, "envMax"), rows.len()), (9, 14));
        }
        let (mut plane, mut rows) = f15e_record();
        rows[0].points[1][0] += 1.;
        assert!(!fit_envelopes(
            AircraftId::F15E,
            &BTreeMap::new(),
            &mut plane,
            &mut rows
        ));
        assert_eq!((n(&plane, "envMin"), rows.len()), (-4, 14));
        let (mut plane, mut rows) = f15e_record();
        rows[13].points[2][0] += 1.;
        assert!(!fit_envelopes(
            AircraftId::F15E,
            &BTreeMap::new(),
            &mut plane,
            &mut rows
        ));
        assert_eq!((n(&plane, "envMax"), rows.len()), (9, 14));
    }

    #[test]
    fn the_f15e_flies_to_60_000_ft_with_its_polygons_cut_off_there() {
        let corrected = BTreeMap::from([("maxAlt".to_string(), token(60_000))]);
        let top = |e: &Envelope| e.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
        let (mut plane, mut rows) = f15e_record();
        assert!(fit_envelopes(
            AircraftId::F15E,
            &corrected,
            &mut plane,
            &mut rows
        ));
        let row = |g: i32| rows.iter().find(|e| e.g == g).unwrap();
        // 1 G: cut off at 60,000 ft; the slow edge (200 ft/s at sea level to 563
        // at 65,000 ft) is 535 there, the fast edge (2,183 at 74,250 ft to
        // 1,360 at sea level) 2,025, and everything below is the record's own.
        assert_eq!(
            row(1).points,
            vec![[200., 0.], [535., 60_000.], [2_025., 60_000.], [1_360., 0.]]
        );
        // 0 G keeps its 5,000 ft step above the 1 G polygon (cut at 65,000).
        assert_eq!(
            row(0).points,
            vec![[140., 0.], [483., 65_000.], [2_178., 65_000.], [1_440., 0.]]
        );
        // The 2 G row (85,000 ft) is cut at the 1 G ceiling; 4 G (54,000 ft)
        // is under it and untouched.
        assert_eq!(
            row(2).points,
            vec![[250., 0.], [709., 60_000.], [1_899., 60_000.], [1_310., 0.]]
        );
        assert_eq!(top(row(4)), 54_000.);
        assert_eq!(row(4).points.len(), 4);
        // The ceiling does not move a record whose `maxAlt` is not corrected,
        // or whose polygons are not the retail ones.
        let (mut plane, mut rows) = f15e_record();
        fit_envelopes(
            AircraftId::F15E,
            &BTreeMap::from([("maxAlt".to_string(), token(100_000))]),
            &mut plane,
            &mut rows,
        );
        assert_eq!(top(rows.iter().find(|e| e.g == 1).unwrap()), 100_000.);
        let (mut plane, mut rows) = f15e_record();
        rows[5].points[3][1] = 99_000.;
        rows[5].points[4][1] = 99_000.;
        fit_envelopes(AircraftId::F15E, &corrected, &mut plane, &mut rows);
        assert_eq!(top(rows.iter().find(|e| e.g == 1).unwrap()), 99_000.);
    }

    /// A synthetic record shaped like the Gripen's: rows -4 to +9 with the -4
    /// row the +8 row, ceilings 60,000 ft (1 G) and 63,000 ft (0 G).
    fn gripen_record() -> (BTreeMap<String, Token>, Vec<Envelope>) {
        let plane = BTreeMap::from([
            ("envMin".to_string(), token(-4)),
            ("envMax".to_string(), token(9)),
        ]);
        let mut rows: Vec<Envelope> = (-4..=9)
            .map(|g| Envelope {
                g,
                points: vec![
                    [300. + f64::from(g), 0.],
                    [500., 20_000.],
                    [1_500., 20_000.],
                    [1_300., 0.],
                ],
            })
            .collect();
        rows[4].points = vec![[160., 0.], [870., 63_000.], [1_734., 63_000.], [1_280., 0.]];
        rows[5].points = vec![[200., 0.], [870., 60_000.], [1_684., 60_000.], [1_230., 0.]];
        // The 3 G polygon: 52,000 ft with a second vertex just under it.
        rows[7].points = vec![
            [280., 0.],
            [870., 52_000.],
            [1_526., 51_875.],
            [1_750., 32_000.],
            [1_180., 0.],
        ];
        let plus_eight = vec![[520., 0.], [615., 11_000.], [920., 21_000.], [1_010., 0.]];
        rows[12].points = plus_eight.clone();
        rows[0].points = plus_eight;
        (plane, rows)
    }

    #[test]
    fn the_gripen_loses_its_minus_4_row_and_flies_to_50_000_ft() {
        let corrected = BTreeMap::from([("maxAlt".to_string(), token(50_000))]);
        let top = |e: &Envelope| e.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
        let (mut plane, mut rows) = gripen_record();
        assert!(fit_envelopes(
            AircraftId::Gripen,
            &corrected,
            &mut plane,
            &mut rows
        ));
        // The -4 row (a copy of +8) is retired; +9 stays the top row.
        assert_eq!((n(&plane, "envMin"), n(&plane, "envMax")), (-3, 9));
        let gs: Vec<i32> = rows.iter().map(|e| e.g).collect();
        assert_eq!(gs, (-3..=9).collect::<Vec<_>>());
        let row = |g: i32| rows.iter().find(|e| e.g == g).unwrap();
        assert_eq!(top(row(1)), 50_000.);
        assert_eq!(top(row(0)), 53_000.);
        assert_eq!(
            row(1).points,
            vec![[200., 0.], [758., 50_000.], [1_608., 50_000.], [1_230., 0.]]
        );
        // The 3 G polygon's 52,000 ft and 51,875 ft vertices are both cut
        // away; the edges ending on 50,000 ft keep the speed they had there.
        assert_eq!(
            row(3).points,
            vec![
                [280., 0.],
                [847., 50_000.],
                [1_547., 50_000.],
                [1_750., 32_000.],
                [1_180., 0.]
            ]
        );
        // A -4 row that is its own shape, another identity or a stale ceiling
        // field: the G rows or the ceiling stay as they were.
        let (mut plane, mut rows) = gripen_record();
        rows[0].points[1][0] += 1.;
        assert!(!fit_envelopes(
            AircraftId::Gripen,
            &BTreeMap::new(),
            &mut plane,
            &mut rows
        ));
        assert_eq!((n(&plane, "envMin"), rows.len()), (-4, 14));
        for other in [AircraftId::F15E, AircraftId::M2000, AircraftId::Rafale] {
            let (mut plane, mut rows) = gripen_record();
            assert!(!fit_envelopes(other, &corrected, &mut plane, &mut rows));
            assert_eq!((n(&plane, "envMin"), rows.len()), (-4, 14));
        }
        let (mut plane, mut rows) = gripen_record();
        let stale = BTreeMap::from([("maxAlt".to_string(), token(60_000))]);
        assert!(fit_envelopes(
            AircraftId::Gripen,
            &stale,
            &mut plane,
            &mut rows
        ));
        assert_eq!(top(rows.iter().find(|e| e.g == 1).unwrap()), 60_000.);
        assert_eq!(n(&plane, "envMin"), -3);
    }

    /// A synthetic record shaped like the Mirage F1's: rows -3 to +7, each
    /// negative row the positive row of twice the G.
    fn mf1_record() -> (BTreeMap<String, Token>, Vec<Envelope>) {
        let plane = BTreeMap::from([
            ("envMin".to_string(), token(-3)),
            ("envMax".to_string(), token(7)),
        ]);
        let mut rows: Vec<Envelope> = (-3..=7)
            .map(|g| Envelope {
                g,
                points: vec![
                    [300. + f64::from(g), 0.],
                    [500., 20_000.],
                    [1_500., 20_000.],
                    [1_300., 0.],
                ],
            })
            .collect();
        rows[0].points = rows[9].points.clone();
        rows[10].points = MF1_RETAIL_PLUS_7.to_vec();
        rows[4].points = vec![[170., 0.], [870., 66_000.], [1_481., 66_125.], [1_350., 0.]];
        (plane, rows)
    }

    #[test]
    fn the_mirage_f1_gets_a_fitted_plus_8_row_and_the_same_polygon_as_its_minus_4_row() {
        let (mut plane, mut rows) = mf1_record();
        let none = BTreeMap::new();
        assert!(fit_envelopes(AircraftId::Mf1, &none, &mut plane, &mut rows));
        assert_eq!((n(&plane, "envMin"), n(&plane, "envMax")), (-4, 8));
        let gs: Vec<i32> = rows.iter().map(|e| e.g).collect();
        assert_eq!(gs, (-4..=8).collect::<Vec<_>>());
        // sqrt(8 / 7) = 1.069 on the slow side (520 to 556, 645 to 690, 900 to
        // 962); the fast side and every altitude are the +7 row's.
        let expected = vec![
            [556., 0.],
            [690., 11_500.],
            [962., 20_000.],
            [1_075., 20_000.],
            [1_240., 17_000.],
            [1_230., 0.],
        ];
        assert_eq!(rows[12].points, expected);
        assert_eq!(rows[0].points, expected);
        assert_eq!(rows[11].points, MF1_RETAIL_PLUS_7.to_vec());
        // The ceiling field (66,125 ft) is the real one and stays.
        let top = |e: &Envelope| e.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
        assert_eq!(top(rows.iter().find(|e| e.g == 1).unwrap()), 66_125.);
        // Another identity, an edited +7 row, or a -3 row that is not the +6
        // row: unchanged.
        for other in [AircraftId::Tornado, AircraftId::M2000, AircraftId::F5E] {
            let (mut plane, mut rows) = mf1_record();
            assert!(!fit_envelopes(other, &none, &mut plane, &mut rows));
            assert_eq!((n(&plane, "envMax"), rows.len()), (7, 11));
        }
        let (mut plane, mut rows) = mf1_record();
        rows[10].points[1][0] += 1.;
        assert!(!fit_envelopes(
            AircraftId::Mf1,
            &none,
            &mut plane,
            &mut rows
        ));
        let (mut plane, mut rows) = mf1_record();
        rows[0].points[1][0] += 1.;
        assert!(!fit_envelopes(
            AircraftId::Mf1,
            &none,
            &mut plane,
            &mut rows
        ));
        assert_eq!((n(&plane, "envMin"), rows.len()), (-3, 11));
    }

    #[test]
    fn cutting_a_polygon_at_the_ceiling_keeps_every_speed_band_below_it() {
        let polygon = Envelope {
            g: 1,
            points: vec![
                [200., 0.],
                [563., 65_000.],
                [801., 96_000.],
                [870., 100_000.],
                [1_554., 100_000.],
                [2_183., 74_250.],
                [2_366., 53_625.],
                [2_420., 36_000.],
                [1_360., 0.],
            ],
        };
        let cut = Envelope {
            g: 1,
            points: clip_below(&polygon.points, 60_000.),
        };
        for altitude in [0., 1_000., 18_000., 36_000., 50_000., 53_625., 59_999.] {
            let (old, new) = (
                polygon.speeds(altitude).unwrap(),
                cut.speeds(altitude).unwrap(),
            );
            // Rounding the two cut vertices to whole ft/s moves a band by
            // less than 1 ft/s (a vertex at or below the cut is not moved).
            assert!((old.0 - new.0).abs() < 1., "{altitude}: {old:?} {new:?}");
            assert!((old.1 - new.1).abs() < 1., "{altitude}: {old:?} {new:?}");
        }
        assert_eq!(cut.speeds(60_001.), None);
        assert_eq!(top_altitude(&cut), 60_000.);
        // A polygon already under the cap is returned as it was, and a vertex
        // exactly on the cap is not doubled.
        let low = [[100., 0.], [200., 9_000.], [900., 9_000.], [800., 0.]];
        assert_eq!(clip_below(&low, 9_000.), low.to_vec());
        assert_eq!(clip_below(&low, 20_000.), low.to_vec());
    }

    /// F117-9 (fitted): the whole-pound flow keeps the model's cruise range
    /// near the published 930 nautical miles. The range at Mach 0.82 and
    /// 36,000 ft is inversely proportional to the flow, 1,516 nautical miles at
    /// the retail 2 lb/s (measured, F117-8), so 3 lb/s is 1,011. Pinned loosely
    /// (within 15 percent above and 5 percent below the published figure): the
    /// flow is a fit to one published range, not a thrust-specific figure.
    #[test]
    fn the_f117_fuel_flow_keeps_the_cruise_range_near_the_published_930() {
        let range = |flow: i32| 1_516. * f64::from(F117_RETAIL_FUEL_LB_PER_S) / f64::from(flow);
        assert!((range(F117_RETAIL_FUEL_LB_PER_S) - 1_516.).abs() < 1e-9);
        let corrected = range(F117_FUEL_LB_PER_S);
        assert!(
            (0.95 * 930. ..=1.15 * 930.).contains(&corrected),
            "{corrected} nmi"
        );
        // The record's whole pounds: the next whole flow is further from 930.
        assert!(
            (range(F117_FUEL_LB_PER_S + 1) - 930.).abs() > (corrected - 930.).abs(),
            "3.3 lb/s rounds to {F117_FUEL_LB_PER_S}"
        );
    }

    #[test]
    fn the_batch_fuel_flows_are_the_corrected_thrust_times_the_cited_sfc_to_the_whole_pound() {
        // (flow, thrust lbf, SFC lb/(lbf h)), each within the rounding of one
        // whole pound a second, which is the record's unit.
        let rows: [(i32, i32, f64); 10] = [
            (F15E_FUEL_LB_PER_S, 32_558, 0.73),
            (F15E_AB_FUEL_LB_PER_S, 47_860, 1.94),
            (GRIPEN_FUEL_LB_PER_S, 12_140, 0.844),
            (GRIPEN_AB_FUEL_LB_PER_S, 18_100, 1.79),
            (MF1_AB_FUEL_LB_PER_S, MF1_AFT_THRUST_LBF, 1.991),
            (TORNADO_FUEL_LB_PER_S, TORNADO_THRUST_LBF, 0.598),
            (TORNADO_AB_FUEL_LB_PER_S, TORNADO_AFT_THRUST_LBF, 2.5),
            (F5E_AB_FUEL_LB_PER_S, 10_000, 2.13),
            (B52_FUEL_LB_PER_S, B52_THRUST_LBF, 0.52),
            (B2_FUEL_LB_PER_S, B2_THRUST_LBF, 0.375),
        ];
        for (flow, thrust, sfc) in rows {
            let exact = f64::from(thrust) * sfc / 3600.;
            assert_eq!(flow, exact.round() as i32, "{thrust} lbf at {sfc}");
            assert!((f64::from(flow) - exact).abs() <= 0.5);
        }
    }
}
