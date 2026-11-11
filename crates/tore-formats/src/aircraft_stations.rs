//! Opinionated extra stations requested by John, 2026-10-15 and 2026-10-16.
//! Guarded additions and requested bay corrections; row indices are preserved.
//! Contract and fitted coordinates: docs/spec/stores-on-pylons.md.
use super::{AircraftId, Hardpoint};

pub const F15_CENTER: usize = 6;
pub const F15_WING_TANKS: usize = 7;
pub const F16_WET: usize = 9;
pub const F16_TIP: usize = 8;
pub const F22_WET: usize = 8;
pub const MIG29_CENTER: usize = 8;
pub const RAFALE_CENTER: usize = 9;
/// The Mirage 2000's belly pairs (`M2000.PT` row 5, four stores in two
/// tandem pairs either side of the centreline tank).
pub const M2000_BELLY: usize = 5;
/// The Mirage 2000's decoded wingtip row (`M2000.PT` row 8), retired by
/// [`retire`].
pub const M2000_TIP: usize = 8;
/// The Tornado's decoded outer wing pair (`TOR.PT` row 8, the last row),
/// retired by [`retire`].
pub const TORNADO_OUTER_WING: usize = 8;
/// The Tornado's AIM-9 side rails (`TOR.PT` row 8 again, appended by
/// [`side_rail`] where [`retire`] dropped the outer wing pair): one rail a
/// side on the inboard swivelling pylon (row 7).
pub const TORNADO_SIDE_RAIL: usize = 8;
/// The F-5E's radar, jammer and wing-pair rows and the Mirage F1's centreline
/// row, whose defaults [`adjust_defaults`] changes.
pub const F5E_RADAR: usize = 1;
pub const F5E_JAMMER: usize = 2;
pub const F5E_WING_PAIR: usize = 5;
pub const MF1_CENTRELINE: usize = 4;
/// The radar the F-5E carries in place of the generic `F18R.SEE`: the F-104
/// record, the closest reviewed small-fighter radar (see [`adjust_defaults`]).
pub const F5E_RADAR_RECORD: &str = "F104R.SEE";
/// The countermeasures record the F-5E's row 2 holds in place of the generic
/// `F18.ECM`: 20 chaff, 20 flares and no deception jammer, the stand-in for
/// the real AN/ALE-40 dispenser (see [`adjust_defaults`]).
pub const F5E_ECM_RECORD: &str = "F104.ECM";

/// The source row of the F-4B's centreline station (`F4B.PT` row 3: four
/// AIM-9Bs, Mk 82s and the other stores on the centreline) that also takes
/// the SUU-16 gun pod, one in place of whatever else it carries (John,
/// 2026-10-16: "a gun pod is missing from the Phantom B"). A real Phantom's
/// centreline is one station, a tank, bombs or a gun pod: the pod replaces
/// the row's stores and does not add a station.
pub const F4B_POD_ROW: usize = 3;

/// The source row of aircraft `id` that takes a gun pod in place of its
/// stores, if any. Only the F-4B's: the F-4J's pod is its own fixed row, and
/// the F-4E and F-4G are built with a gun (an open question for John).
pub fn gun_pod_row(id: AircraftId) -> Option<usize> {
    (id == AircraftId::F4B).then_some(F4B_POD_ROW)
}

/// How many of an aircraft's appended weapon rows came after a saved loadout
/// could already have been made without them: the Eagle's wing pylons (row 7)
/// took missiles on 2026-10-20, so a load saved with the one weapon row before
/// it (the centreline bombs) still restores, the new row empty.
pub fn later_weapon_rows(id: AircraftId) -> usize {
    usize::from(id == AircraftId::F15)
}

/// These names have authored GAS records, generated in memory at pack load.
/// Fuel uses the game's 6.6 lb/US gal convention; shell weights are fitted by
/// scaling the 350 gallon class (248 lb), rounded to whole pounds.
pub const TANKS: [(&str, u16, i32, u16); 5] = [
    ("F610.GAS", 610, 4026, 432),
    ("F370.GAS", 370, 2442, 262),
    ("F600.GAS", 600, 3960, 425),
    ("M396.GAS", 396, 2614, 281),
    ("R528.GAS", 528, 3485, 374),
];

/// Canonical authored bytes, also used by the old combat-tape media guard.
pub fn resource(name: &str) -> Option<Vec<u8>> {
    let (_, gallons, fuel, shell) = TANKS.iter().find(|t| t.0 == name)?;
    Some(format!("[brent's_relocatable_format]\nbyte 8\nptr si_names\nword {shell}\nbyte 1\ndword {fuel}\n:si_names\nstring \"{gallons} gallon tank\"\nstring \"{gallons} Gallon External Fuel Tank\"\nstring \"{name}\"\nend\n").into_bytes())
}

/// Adds each authored tank only to a pack holding its carrier, as the
/// derived bombs need their donor: a pack without a supported carrier stays
/// exactly as imported.
pub fn extend_resources(resources: &mut std::collections::BTreeMap<String, Vec<u8>>) {
    for ((name, ..), carrier) in
        TANKS
            .iter()
            .zip(["F15.PT", "F16C.PT", "F22.PT", "MIG29.PT", "RAFALE.PT"])
    {
        if resources.contains_key(carrier)
            || (carrier == "F22.PT" && resources.contains_key("F22N.PT"))
        {
            resources
                .entry((*name).into())
                .or_insert_with(|| resource(name).expect("known authored tank"));
        }
    }
}

/// Index in the existing nine fuel banks. F-16 and Rafale row 9 use bank 0; their
/// fixed visual sensors cannot fuel. Configuration rejects bank collisions.
/// No serialized array or previous aircraft's fuel bank changes.
pub fn fuel_bank(hardpoint: usize) -> Option<usize> {
    match hardpoint {
        0..=8 => Some(hardpoint),
        F16_WET => Some(0),
        _ => None,
    }
}

/// Restrict the new tank identities to the requested real carriers. Existing
/// GAS types retain the game's compatibility rules, including Cheat loading.
pub fn tank_allowed(id: AircraftId, row: usize, name: &str) -> bool {
    match name {
        "F610.GAS" => id == AircraftId::F15 && matches!(row, F15_CENTER | F15_WING_TANKS),
        "F370.GAS" => id == AircraftId::F16C && row == F16_WET,
        "F600.GAS" => {
            matches!(id, AircraftId::F22 | AircraftId::F22n | AircraftId::Faxx) && row == F22_WET
        }
        "M396.GAS" => id == AircraftId::Mig29 && row == MIG29_CENTER,
        "R528.GAS" => id == AircraftId::Rafale && row == RAFALE_CENTER,
        // No big tanks on the Mirage 2000's belly pairs: they would cross the
        // centreline tank (John, 2026-11-10). Normal loading only; Cheat
        // loading skips this check, and the 150 and 250 gallon tanks stay.
        "F350.GAS" | "F500.GAS" => !(id == AircraftId::M2000 && row == M2000_BELLY),
        _ => true,
    }
}

/// The Tornado's shoulder pair (`TOR.PT` row 6: Fuselage, x 6, four
/// `PAVEWAY.JT`) and inner wing pair (row 7: Wing, x 11, two `MK82P3.JT`,
/// carried on the swinging panel by the sim's `station_move`).
pub const TORNADO_SHOULDER: usize = 6;
pub const TORNADO_INNER_WING: usize = 7;

/// Stores normal loading refuses on the Tornado's shoulder pair (row 6), John's
/// F-15E A2 precedent applied by an agent (2026-11-11, TOR-6): refuse the
/// stores that enter the airframe over about 0.3 ft, keep anything at 0.3 ft
/// or less and anything that only crosses a neighbour store. Each store below enters the
/// fuselage flank (intake and belly) by more than 0.3 ft with the row's
/// own pylon geometry, measured with the CPU stores preview against the
/// airframe at both sweeps (docs/spec/stores-on-pylons.md, TOR-6 section):
/// every row 6 catalog store, 0.33 ft (GBU-28, AT-12) to 2.12 ft (AS-30). The
/// default `PAVEWAY.JT` and the two other stores that hang the same shape
/// (`GBU10.JT`, `GBU10A.JT`; the fin-tip entry of 0.96 ft front and 0.55 ft
/// rear that John accepted on 2026-11-11, TOR-5 round 3) stay. Stores at or
/// under 0.3 ft (the Mk 82 and Mk 84 families, cluster and rocket dispensers,
/// FAB bombs, the Maverick, GBU-38, the AS-7 at exactly 0.30,
/// and every tank) stay.
pub const TORNADO_SHOULDER_REFUSED: [&str; 33] = [
    "AA11.JT",
    "AA12.JT",
    "AA2.JT",
    "AA6.JT",
    "AA9.JT",
    "AGM45.JT",
    "AGM84A.JT",
    "AGM84E.JT",
    "AGM88.JT",
    "AIM120.JT",
    "AIM7.JT",
    "AIM7E.JT",
    "AIM9B.JT",
    "AIM9M.JT",
    "AIM9X.JT",
    "AM39.JT",
    "AS14.JT",
    "AS15.JT",
    "AS30.JT",
    "AT12.JT",
    "AT2.JT",
    "GBU12.JT",
    "GBU16.JT",
    "GBU28.JT",
    "GBU29.JT",
    "GBU29P.JT",
    "GBU30.JT",
    "MICA.JT",
    "PAVEWA3.JT",
    "PL10.JT",
    "PL7.JT",
    "R530.JT",
    "R550.JT",
];

/// Stores normal loading refuses on the Tornado's inner wing pair (row 7,
/// on the swinging panel): the three 2,000 lb bombs of the `GB27.SH` shape,
/// which enter the airframe 1.10 ft at 67 degrees of sweep (0.00 at 25).
/// Every other row 7 catalog store reads 0.00 at both sweeps.
pub const TORNADO_INNER_WING_REFUSED: [&str; 3] = ["GBU29.JT", "GBU29P.JT", "GBU30.JT"];

/// Stores normal loading refuses on the Tornado's AIM-9 side rails (row 8,
/// TOR-6b, 2026-11-11, agent decision applying the 0.3 ft line): the four
/// stores the rail's infrared mask admits that enter the airframe or the
/// pylon's store more than the line, measured alone on the rail at 25 and 67
/// degrees against the Mk 82 pack: `PL7.JT` (0.50 ft, 0.83 at 67), `R550.JT`
/// (0.40), `AT12.JT` (0.44) and `GBU12.JT` (1.02, a laser bomb on a missile
/// rail). The seven that stay (`AIM9B`, `AIM9M`, `AIM9X`, `AA2`, `AA8`,
/// `AA11`, `AA11B`) read 0.00 ft against the airframe at both sweeps.
pub const TORNADO_SIDE_RAIL_REFUSED: [&str; 4] = ["PL7.JT", "R550.JT", "AT12.JT", "GBU12.JT"];

/// Stores normal loading refuses on one source row (John's F-15E A2
/// precedent, applied to the Tornado by an agent decision, 2026-11-11). Cheat loading skips the check; every other aircraft and row
/// keeps its stores. The side-rail row (8, TOR-6b, 2026-11-11) takes what its
/// infrared mask (0x85) admits, as the F-14's glove-pylon rail does, minus
/// [`TORNADO_SIDE_RAIL_REFUSED`].
pub fn weapon_allowed(id: AircraftId, row: usize, source: &str) -> bool {
    if id != AircraftId::Tornado {
        return true;
    }
    match row {
        TORNADO_SHOULDER => !TORNADO_SHOULDER_REFUSED.contains(&source),
        TORNADO_INNER_WING => !TORNADO_INNER_WING_REFUSED.contains(&source),
        TORNADO_SIDE_RAIL => !TORNADO_SIDE_RAIL_REFUSED.contains(&source),
        _ => true,
    }
}

/// Source rows T.O.R.E does not carry (John, 2026-11-10, opinionated): the
/// Mirage 2000's wingtip row (`M2000.PT` row 8: Wingtip, x 33, two `R550.JT`,
/// mask 0x85). It is the Rafale template's wingtip rail carried into the
/// Mirage record; the real aircraft has nine store stations (five under the
/// fuselage, two under each wing) and no tip rail, and its two Magics ride the
/// outer wing pylons (row 7). The decoder still reads the retail row and the
/// documents keep it as retail (docs/formats/aircraft-variety.md); like
/// the guarded additions of [`append`], the row is dropped here, after
/// decoding, only when it is exactly the retail row and the last one, so every
/// consumer (default and saved loads, the AI, the audit, Cheat loading, the
/// stores drawing) sees the same eight rows and no station index moves.
/// Repeated application is a no-op.
pub fn retire(id: AircraftId, rows: &mut Vec<Hardpoint>) {
    let (index, retail): (usize, Signature) = match id {
        AircraftId::M2000 => (M2000_TIP, (5, 0x85, [33, -4, -29], "R550.JT", 2, 4)),
        // The Tornado's outer wing pair (John, 2026-11-10, opinionated: "remove
        // end wing stations typically reserved for the pods"): `TOR.PT` row 8,
        // Wing, x 16, two `AIM9M.JT`, mask 0x85. The real aircraft's outer
        // wing pylons carry pods and no weapons; the row is the last, so no
        // other index moves. The inner wing pair (row 7) stays.
        AircraftId::Tornado => (
            TORNADO_OUTER_WING,
            (4, 0x85, [16, -22, 4], "AIM9M.JT", 2, 5),
        ),
        _ => return,
    };
    if rows.len() != index + 1 {
        return;
    }
    let row = &rows[index];
    if (
        row.location,
        row.flags,
        row.position,
        row.store.as_deref(),
        row.count,
        row.weight_class,
    ) == (
        retail.0,
        retail.1,
        retail.2,
        Some(retail.3),
        retail.4,
        retail.5,
    ) {
        rows.pop();
    }
}

/// The Tornado's AIM-9 side rails (John, 2026-11-11, after TOR-6: "Lets do a
/// custom side rail kind of like the f-14 for this station and put it on the
/// inner [wing] rail like the RL Tornado", opinionated). The real Tornado
/// carries its AIM-9L on a rail beside each inboard swivelling wing pylon,
/// alongside the pylon's main store; [`retire`] drops the retail outer wing
/// pair, and this appends one row in its place at the end of the table, so
/// no other row's index moves. The row is the retail outer pair's own
/// content, restored: Wing, mask 0x85 (infrared missiles), two `AIM9M.JT`
/// (one a side), weight class 5, as the retail row 8 held them. Only its
/// position is the inner pylon's record's (the sim's `station_move` hangs it
/// on that pylon's side). Applied after
/// [`retire`], only when the table is exactly the retail eight rows the
/// retirement leaves (an edited record or a different count keeps its own
/// rows), on the Tornado only; repeated application is a no-op.
pub fn side_rail(id: AircraftId, rows: &mut Vec<Hardpoint>) {
    if id != AircraftId::Tornado || rows.len() != TORNADO_SIDE_RAIL {
        return;
    }
    let inner = &rows[TORNADO_INNER_WING];
    if (
        inner.location,
        inner.flags,
        inner.position,
        inner.store.as_deref(),
        inner.count,
        inner.weight_class,
    ) != (4, 0x505, [11, -25, 9], Some("MK82P3.JT"), 2, 30)
    {
        return;
    }
    rows.push(Hardpoint {
        location: 4,
        flags: 0x85,
        position: inner.position,
        store: Some("AIM9M.JT".into()),
        count: 2,
        weight_class: 5,
    });
}

/// The retail rows of the aircraft whose default load [`adjust_defaults`]
/// changes: the whole table, so an edited record keeps its own defaults.
fn default_signature(id: AircraftId) -> &'static [Signature] {
    match id {
        AircraftId::F5E => &[
            (1, 0x8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "F18R.SEE", 1, 0),
            (1, 0x8, [0, 0, -50], "F18.ECM", 1, 0),
            (2, 0xc, [4, -5, 41], "M61.JT", 560, 0),
            (0, 0x605, [0, -14, 0], "F150.GAS", 1, 11),
            (4, 0x7e5, [22, -14, 3], "AIM7.JT", 4, 40),
            (5, 0x85, [43, -3, -3], "AIM9B.JT", 2, 4),
        ],
        AircraftId::Mf1 => &[
            (1, 0x8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "F18R.SEE", 1, 0),
            (1, 0x8, [0, 0, -50], "AV8.ECM", 1, 0),
            (2, 0xc, [3, -5, 51], "DEFA.JT", 250, 0),
            (0, 0x7e5, [0, -18, 5], "ALQ167.ECM", 1, 22),
            (4, 0x7e5, [6, -17, 0], "F250.GAS", 4, 46),
            (4, 0x1c5, [21, -10, -3], "R550.JT", 2, 20),
            (5, 0x85, [41, 3, -11], "R550.JT", 2, 4),
        ],
        _ => &[],
    }
}

/// Default-load corrections (John, 2026-11-10, opinionated, batch decision
/// round), applied after decoding only when every source row is exactly the
/// retail one. No row is added, removed or moved and no row's mask, count,
/// weight class or position changes, so every station index and everything
/// each station accepts stays as decoded; only what the row holds by default
/// changes (`store`).
///
/// - **Mirage F1** (`MF1.PT` row 4, the centreline station): no default
///   `ALQ167.ECM` pod. The record's American jammer pod is not in Jane's list
///   of the F1's loads (`MF1.INF`); the station is real and still takes what
///   its mask admits.
/// - **F-5E** (`F5E.PT`): row 5, the wing pair, defaults to empty (no `AIM7.JT`:
///   the real F-5E does not carry the Sparrow); the `AIM9B.JT` wingtip pair
///   stays. Row 2 holds `F104.ECM` in place of the generic `F18.ECM` (John,
///   2026-11-10, B5-3, opinionated, replacing the B5-2 empty row that also
///   dropped the chaff and flares): 20 chaff, 20 flares and no deception
///   jammer. The real F-5E has no internal jammer but carries an AN/ALE-40
///   chaff and flare dispenser (Wikipedia "Northrop F-5"; the F-5E's tail-cone
///   dispensers; the AN/ALE-40 is a standard fit on USAF F-5E aggressors,
///   Wikipedia "AN/ALE-40"), which the F-104 record's dispenser-only bundle
///   matches best. The row keeps its place, mask, count and position. Row 1 holds `F104R.SEE` in place of the generic
///   `F18R.SEE`: the real radar is the Emerson AN/APQ-159 (20 nm, `F5E.INF`;
///   Wikipedia "Northrop F-5"), which TORE has no record for; the closest
///   reviewed record is the F-104's `F104R.SEE` (50 nm search, 25 nm track,
///   look-down 50, the Basic radar preset: a lightweight 1960s fighter set).
///   The alternatives are `F4BR.SEE` (the same ranges, a two-seat interceptor's
///   set) and `YAK141R.SEE` (40 / 25 nm, Transitional); `F8R.SEE` (35 / 20 nm)
///   is closest in range but has no reviewed preset and would never switch on.
pub fn adjust_defaults(id: AircraftId, rows: &mut [Hardpoint]) {
    let signature = default_signature(id);
    if signature.is_empty()
        || rows.len() != signature.len()
        || !rows.iter().zip(signature).all(|(h, s)| {
            (
                h.location,
                h.flags,
                h.position,
                h.store.as_deref(),
                h.count,
                h.weight_class,
            ) == (s.0, s.1, s.2, Some(s.3), s.4, s.5)
        })
    {
        return;
    }
    match id {
        AircraftId::F5E => {
            rows[F5E_RADAR].store = Some(F5E_RADAR_RECORD.into());
            rows[F5E_JAMMER].store = Some(F5E_ECM_RECORD.into());
            rows[F5E_WING_PAIR].store = None;
        }
        AircraftId::Mf1 => rows[MF1_CENTRELINE].store = None,
        _ => {}
    }
}

/// The Tornado's centreline tank row (`TOR.PT` row 5: Wing, x 0, two
/// `F250.GAS`), whose default quantity [`default_counts`] changes.
pub const TORNADO_CENTRELINE: usize = 5;

/// Default quantities where T.O.R.E's default load differs from the source
/// row's capacity (John, 2026-11-11, round 3 "T3", opinionated: "the default
/// load gets one centreline tank instead of the crossing pair"). The real
/// Tornado IDS carries one tank on its centreline station; the retail row
/// holds two (a pair 1.18 ft either side of x 0 that crosses the shoulder
/// Paveways beside it). The default is one tank, the record's own type, at
/// x 0. Only the quantity that starts loaded changes: the row keeps its
/// place, mask, capacity (two), weight class and position, so Load Ordnance
/// still admits what it did. Guarded on the whole retail row (an edited
/// record keeps its own default) and on the identity; repeated application
/// is a no-op. Applied by `tore_sim::combat::hardpoints::default_records`.
pub fn default_counts(id: AircraftId, rows: &mut [Hardpoint]) {
    if id != AircraftId::Tornado {
        return;
    }
    if let Some(row) = rows.get_mut(TORNADO_CENTRELINE)
        && (
            row.location,
            row.flags,
            row.position,
            row.store.as_deref(),
            row.count,
            row.weight_class,
        ) == (4, 0x205, [0, -25, 17], Some("F250.GAS"), 2, 40)
    {
        row.count = 1;
    }
}

/// Weapons an existing source row takes in normal loading beyond its own mask,
/// on aircraft whose guarded rows were appended (so the reviewed source
/// signature matched). The F-16C's wingtip launchers (stations 1 and 9,
/// LAU-129) carry the AIM-120 as well as the AIM-9 (John, 2026-10-16). The
/// source row stays as imported: its mask (0x85, infrared only) and weight
/// class (4, sized for two AIM-9s) cannot admit a 345 lb radar missile, and
/// raising either would also change the Archer and other infrared counts.
/// The count is the row's own, one a tip. Cheat loading already allows it.
pub fn extra_weapon(id: AircraftId, rows: usize, row: usize, source: &str) -> bool {
    id == AircraftId::F16C && rows > F16_WET && row == F16_TIP && source == "AIM120.JT"
}

type Signature = (u8, i32, [i32; 3], &'static str, i32, i32);

fn signature(id: AircraftId) -> &'static [Signature] {
    // Check every source row, not just the identity. Edited records keep
    // their own station definitions; repeated application is a no-op.
    match id {
        AircraftId::F15 => &[
            (1, 8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 8, [0, 0, 0], "F15R.SEE", 1, 0),
            (1, 8, [0, 0, -50], "F15.ECM", 1, 0),
            (1, 8, [5, -5, -5], "M61.JT", 940, 0),
            (1, 0x7e5, [25, -10, 0], "AIM120.JT", 4, 60),
            (4, 0x7e5, [50, -20, 0], "AIM9M.JT", 4, 14),
        ],
        AircraftId::F16C => &[
            (1, 8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 8, [0, 0, 0], "F18R.SEE", 1, 0),
            (1, 8, [0, 0, -50], "F18.ECM", 1, 0),
            (1, 8, [0, 0, 50], "PAVESPK.SEE", 1, 0),
            (2, 8, [-11, 3, 31], "M61.JT", 500, 0),
            (0, 0x601, [0, -20, 0], "F150.GAS", 1, 26),
            (4, 0x7e5, [32, -10, -7], "AGM65G.JT", 4, 27),
            (4, 0x185, [43, -10, -11], "AIM120.JT", 2, 13),
            (5, 0x85, [54, 2, -14], "AIM9M.JT", 2, 4),
        ],
        AircraftId::F22 => &[
            (1, 0x8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "F22R.SEE", 1, 0),
            (1, 0x8, [0, 0, -50], "F22.ECM", 1, 0),
            (1, 0x8, [11, 3, 50], "M61.JT", 750, 0),
            (3, 0x8, [0, 0, 0], "AV8IR.SEE", 1, 0),
            (3, 0x5e0, [-10, -25, 0], "AIM120.JT", 2, 40),
            (3, 0x5e0, [10, -25, 0], "AGM65G.JT", 2, 40),
            (3, 0x5e4, [27, -11, 0], "AIM9X.JT", 4, 8),
        ],
        AircraftId::F22n => &[
            (1, 0x8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "F22R.SEE", 1, 0),
            (1, 0x8, [0, 0, -50], "F22.ECM", 1, 0),
            (1, 0x8, [10, 3, 50], "M61.JT", 750, 0),
            (3, 0x8, [0, 0, 0], "AV8IR.SEE", 1, 0),
            (1, 0x5e1, [-10, -25, 0], "AIM120.JT", 2, 40),
            (1, 0x5e1, [10, -25, 0], "AGM65G.JT", 2, 40),
            (1, 0x5e5, [27, -11, 0], "AIM9X.JT", 4, 8),
        ],
        AircraftId::Mig29 => &[
            (1, 0x8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "MIG29I.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "MIG29R.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "MIG29.ECM", 1, 0),
            (2, 0x8, [-5, -5, 60], "GSH301.JT", 150, 0),
            (4, 0x7e5, [20, -10, -5], "AA8.JT", 2, 20),
            (4, 0x7e5, [25, -10, -5], "AA8.JT", 2, 20),
            (4, 0x7e5, [50, -10, -5], "AA8.JT", 2, 20),
        ],
        AircraftId::Rafale => &[
            (1, 0x8, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "F18R.SEE", 1, 0),
            (1, 0x8, [0, 0, -50], "F18.ECM", 1, 0),
            (2, 0x8, [-4, -5, 59], "DEFA.JT", 250, 0),
            (0, 0x601, [0, -20, 0], "AAS38.SEE", 1, 22),
            (1, 0x5e5, [10, -20, 0], "AGM65G.JT", 4, 46),
            (4, 0x5e5, [20, -20, 8], "MICA.JT", 2, 30),
            (4, 0x5e5, [36, -20, -18], "R530.JT", 2, 14),
            (5, 0x85, [59, -3, -24], "R550.JT", 2, 4),
        ],
        _ => &[],
    }
}

pub fn append(id: AircraftId, rows: &mut Vec<Hardpoint>) {
    let signature = signature(id);
    if signature.is_empty() {
        return;
    }
    if rows.len() != signature.len()
        || !rows.iter().zip(signature).all(|(h, s)| {
            (
                h.location,
                h.flags,
                h.position,
                h.store.as_deref(),
                h.count,
                h.weight_class,
            ) == (s.0, s.1, s.2, Some(s.3), s.4, s.5)
        })
    {
        return;
    }
    let row = |location, flags, position, count, weight_class| Hardpoint {
        location,
        flags,
        position,
        count,
        weight_class,
        store: None,
    };
    match id {
        AircraftId::F15 => {
            rows.push(row(0, 0x3c1, [0, -24, -9], 3, 45));
            // The wing pylons: a tank, or an AIM-120 or Sidewinder in its
            // place (0x2a5: tank, infrared and active radar), one a side.
            rows.push(row(4, 0x2a5, [50, -20, -16], 2, 90));
        }
        AircraftId::F16C => rows.push(row(4, 0x3e5, [21, -18, -3], 2, 60)),
        AircraftId::F22 | AircraftId::F22n => {
            // Requested 2026-10-21: the naval donor carries the same bay
            // weapons. Its own gun, sensors, hook and flight record remain.
            if id == AircraftId::F22n {
                for bay in &mut rows[5..=7] {
                    bay.location = 3;
                    bay.flags &= !1;
                }
            }
            rows.push(row(4, 0x2a5, [36, -18, -18], 4, 90));
        }
        AircraftId::Mig29 => rows.push(row(0, 0x201, [0, -24, -6], 1, 30)),
        AircraftId::Rafale => rows.push(row(0, 0x201, [0, -24, -12], 1, 40)),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn additions_are_empty_guarded_and_idempotent() {
        for (id, total) in [
            (AircraftId::F15, 8),
            (AircraftId::F16C, 10),
            (AircraftId::F22, 9),
            (AircraftId::F22n, 9),
            (AircraftId::Mig29, 9),
            (AircraftId::Rafale, 10),
        ] {
            let original: Vec<_> = signature(id)
                .iter()
                .map(|s| Hardpoint {
                    location: s.0,
                    flags: s.1,
                    position: s.2,
                    store: Some(s.3.into()),
                    count: s.4,
                    weight_class: s.5,
                })
                .collect();
            let mut rows = original.clone();
            append(id, &mut rows);
            assert_eq!(rows.len(), total);
            assert!(rows[original.len()..].iter().all(|h| h.store.is_none()));
            append(id, &mut rows);
            assert_eq!(rows.len(), total);
            for index in 0..original.len() {
                for field in 0..6 {
                    let mut edited = original.clone();
                    match field {
                        0 => edited[index].position[0] += 1,
                        1 => edited[index].location ^= 1,
                        2 => edited[index].flags ^= 1,
                        3 => edited[index].count += 1,
                        4 => edited[index].weight_class += 1,
                        _ => edited[index].store = None,
                    }
                    append(id, &mut edited);
                    assert_eq!(edited.len(), original.len());
                }
            }
        }
    }
    #[test]
    fn the_eagles_wing_row_takes_tank_and_missile_masks_and_old_loads_still_fit() {
        let mut rows: Vec<_> = signature(AircraftId::F15)
            .iter()
            .map(|s| Hardpoint {
                location: s.0,
                flags: s.1,
                position: s.2,
                store: Some(s.3.into()),
                count: s.4,
                weight_class: s.5,
            })
            .collect();
        append(AircraftId::F15, &mut rows);
        // Tank 0x200, infrared 0x80, active radar 0x20 and the base 0x5:
        // not the Sparrow's 0x40, not bombs (0x100).
        assert_eq!(rows[F15_WING_TANKS].flags, 0x2a5);
        assert_eq!(
            (
                rows[F15_WING_TANKS].count,
                rows[F15_WING_TANKS].weight_class
            ),
            (2, 90)
        );
        // One weapon slot was added after the bomb rack's save format.
        assert_eq!(later_weapon_rows(AircraftId::F15), 1);
        assert_eq!(later_weapon_rows(AircraftId::F18), 0);
        assert_eq!(later_weapon_rows(AircraftId::F22), 0);
    }
    #[test]
    fn only_the_f4b_swaps_a_gun_pod_into_its_centreline_row() {
        assert_eq!(gun_pod_row(AircraftId::F4B), Some(3));
        for id in [
            AircraftId::F4J,
            AircraftId::F4E,
            AircraftId::F4G,
            AircraftId::F15,
        ] {
            assert_eq!(gun_pod_row(id), None);
        }
        assert_eq!(AircraftId::F4B.gun_pods(), &["SUU16.JT"]);
        // The pod is a gun pod of that aircraft only where the table has a row.
        for id in AircraftId::ALL {
            assert_eq!(!id.gun_pods().is_empty(), gun_pod_row(id).is_some());
        }
    }

    #[test]
    fn only_the_appended_f16c_tip_row_takes_the_amraam() {
        assert!(extra_weapon(AircraftId::F16C, 10, F16_TIP, "AIM120.JT"));
        assert!(!extra_weapon(AircraftId::F16C, 9, F16_TIP, "AIM120.JT"));
        assert!(!extra_weapon(AircraftId::F16C, 10, 7, "AIM120.JT"));
        assert!(!extra_weapon(AircraftId::F16C, 10, F16_TIP, "AIM7.JT"));
        assert!(!extra_weapon(AircraftId::F18, 10, F16_TIP, "AIM120.JT"));
        // The row is the reviewed source tip, which the signature guards.
        let tip = signature(AircraftId::F16C)[F16_TIP];
        assert_eq!(
            (tip.0, tip.1, tip.3, tip.4, tip.5),
            (5, 0x85, "AIM9M.JT", 2, 4)
        );
    }

    #[test]
    fn each_tank_requires_its_own_carrier() {
        for (index, carrier) in ["F15.PT", "F16C.PT", "F22.PT", "MIG29.PT", "RAFALE.PT"]
            .iter()
            .enumerate()
        {
            let mut resources = std::collections::BTreeMap::from([((*carrier).into(), Vec::new())]);
            extend_resources(&mut resources);
            assert_eq!(resources.len(), 2);
            assert!(resources.contains_key(TANKS[index].0));
        }
        {
            let id = AircraftId::A10;
            let mut resources = std::collections::BTreeMap::from([(id.pt().into(), Vec::new())]);
            extend_resources(&mut resources);
            assert_eq!(resources.len(), 1);
            let mut rows = Vec::new();
            append(id, &mut rows);
            assert!(rows.is_empty());
            assert!(!tank_allowed(id, 8, "F600.GAS"));
        }
    }

    #[test]
    fn naval_bays_keep_counts_and_match_the_raptors_carriage() {
        let source = |id| {
            signature(id)
                .iter()
                .map(|s| Hardpoint {
                    location: s.0,
                    flags: s.1,
                    position: s.2,
                    store: Some(s.3.into()),
                    count: s.4,
                    weight_class: s.5,
                })
                .collect::<Vec<_>>()
        };
        let mut air = source(AircraftId::F22);
        let mut naval = source(AircraftId::F22n);
        append(AircraftId::F22, &mut air);
        append(AircraftId::F22n, &mut naval);
        for (a, n) in air[5..].iter().zip(&naval[5..]) {
            assert_eq!(
                (
                    a.location,
                    a.flags,
                    a.position,
                    &a.store,
                    a.count,
                    a.weight_class
                ),
                (
                    n.location,
                    n.flags,
                    n.position,
                    &n.store,
                    n.count,
                    n.weight_class
                )
            );
        }
        assert_eq!(naval[3].position, [10, 3, 50]);
        let mut pack = std::collections::BTreeMap::from([("F22N.PT".into(), Vec::new())]);
        extend_resources(&mut pack);
        assert!(pack.contains_key("F600.GAS"));
        assert!(tank_allowed(AircraftId::F22n, 8, "F600.GAS"));
        assert!(!tank_allowed(AircraftId::F22n, 7, "F600.GAS"));
    }

    #[test]
    fn authored_tanks_parse_and_explicit_records_win() {
        let mut data = std::collections::BTreeMap::new();
        extend_resources(&mut data);
        assert!(data.is_empty(), "no carrier, no authored tank");
        data.insert("F15.PT".into(), Vec::new());
        data.insert("F16C.PT".into(), Vec::new());
        for name in ["F22.PT", "MIG29.PT", "RAFALE.PT"] {
            data.insert(name.into(), Vec::new());
        }
        extend_resources(&mut data);
        for (name, _, fuel, shell) in TANKS {
            let t = crate::weapons::Tank::parse(&data[name]).unwrap();
            assert_eq!((t.fuel_weight, t.empty_weight), (fuel, shell));
        }
        data.insert("F610.GAS".into(), b"edited".to_vec());
        extend_resources(&mut data);
        assert_eq!(data["F610.GAS"], b"edited");
        assert_eq!(fuel_bank(F16_WET), Some(0));
        assert_eq!(fuel_bank(10), None);
        assert!(!tank_allowed(AircraftId::F18, 7, "F610.GAS"));
    }

    #[test]
    fn the_mirage_2000_belly_pairs_take_no_big_tanks() {
        for name in ["F350.GAS", "F500.GAS"] {
            assert!(!tank_allowed(AircraftId::M2000, M2000_BELLY, name));
            // Every other row of the Mirage and every other aircraft keep them.
            assert!(tank_allowed(AircraftId::M2000, 6, name));
            assert!(tank_allowed(AircraftId::M2000, 4, name));
            assert!(tank_allowed(AircraftId::Rafale, M2000_BELLY, name));
        }
        for name in ["F150.GAS", "F250.GAS"] {
            assert!(tank_allowed(AircraftId::M2000, M2000_BELLY, name));
        }
    }

    /// TOR-6 (agent decision 2026-11-11, John's F-15E A2 precedent): the
    /// Tornado's shoulder and inner wing pairs refuse the stores that enter
    /// the airframe over 0.3 ft; the default Paveway family, the bombs and
    /// every other row and aircraft are untouched.
    #[test]
    fn the_tornado_refuses_the_stores_that_enter_the_airframe_on_its_wing_and_shoulder_rows() {
        for name in TORNADO_SHOULDER_REFUSED {
            assert!(
                !weapon_allowed(AircraftId::Tornado, TORNADO_SHOULDER, name),
                "{name}"
            );
            // Not another row, not another aircraft.
            assert!(weapon_allowed(AircraftId::Tornado, 5, name), "{name}");
            assert!(weapon_allowed(AircraftId::Tornado, 4, name), "{name}");
            assert!(
                weapon_allowed(AircraftId::Mig23, TORNADO_SHOULDER, name),
                "{name}"
            );
        }
        // The shoulder keeps the default, its two siblings of the same shape,
        // and the bombs, dispensers and Maverick that fit.
        for name in [
            "PAVEWAY.JT",
            "GBU10.JT",
            "GBU10A.JT",
            "MK82.JT",
            "MK84.JT",
            "CBU87.JT",
            "AGM65G.JT",
            "AS7.JT",
        ] {
            assert!(
                weapon_allowed(AircraftId::Tornado, TORNADO_SHOULDER, name),
                "{name}"
            );
        }
        for name in TORNADO_INNER_WING_REFUSED {
            assert!(
                !weapon_allowed(AircraftId::Tornado, TORNADO_INNER_WING, name),
                "{name}"
            );
            // The same three bombs are also refused on the shoulder pair.
            assert!(TORNADO_SHOULDER_REFUSED.contains(&name), "{name}");
        }
        for name in ["MK82P3.JT", "MK84.JT", "GBU38.JT", "AGM88.JT", "LAU10.JT"] {
            assert!(
                weapon_allowed(AircraftId::Tornado, TORNADO_INNER_WING, name),
                "{name}"
            );
        }
        assert!(!TORNADO_SHOULDER_REFUSED.contains(&"PAVEWAY.JT"));
    }

    #[test]
    fn the_mirage_2000_carries_no_wingtip_row_and_only_that_row_goes() {
        let row = |location, flags, position, store: &str, count, weight_class| Hardpoint {
            location,
            flags,
            position,
            store: Some(store.into()),
            count,
            weight_class,
        };
        let retail = vec![
            row(1, 0x8, [0, 0, 0], "VIS340.SEE", 1, 0),
            row(1, 0x8, [0, 0, 0], "F18R.SEE", 1, 0),
            row(1, 0x8, [0, 0, -50], "F18.ECM", 1, 0),
            row(2, 0xc, [3, -5, 45], "DEFA.JT", 250, 0),
            row(0, 0x605, [0, -15, 0], "F150.GAS", 1, 13),
            row(1, 0x7e5, [4, -20, 0], "AS15.JT", 4, 46),
            row(4, 0x7e5, [11, -20, -10], "R530.JT", 2, 20),
            row(4, 0x7e5, [20, -20, -20], "R550.JT", 2, 20),
            row(5, 0x85, [33, -4, -29], "R550.JT", 2, 4),
        ];
        let key = |rows: &[Hardpoint]| -> Vec<_> {
            rows.iter()
                .map(|h| (h.location, h.flags, h.position, h.store.clone(), h.count))
                .collect()
        };
        let mut rows = retail.clone();
        retire(AircraftId::M2000, &mut rows);
        assert_eq!(key(&rows), key(&retail[..M2000_TIP]));
        // The outer wing pair keeps its two Magics, and a second call is a no-op.
        assert_eq!(rows[7].store.as_deref(), Some("R550.JT"));
        retire(AircraftId::M2000, &mut rows);
        assert_eq!(rows.len(), 8);
        // Any other aircraft, or an edited tip row, keeps every row.
        let mut rafale = retail.clone();
        retire(AircraftId::Rafale, &mut rafale);
        assert_eq!(key(&rafale), key(&retail));
        let mut edited = retail.clone();
        edited[M2000_TIP].count = 4;
        retire(AircraftId::M2000, &mut edited);
        assert_eq!(edited.len(), 9);
    }

    fn row((location, flags, position, store, count, weight_class): Signature) -> Hardpoint {
        Hardpoint {
            location,
            flags,
            position,
            store: Some(store.into()),
            count,
            weight_class,
        }
    }

    fn tornado_retail() -> Vec<Hardpoint> {
        [
            (1, 0x8, [0, 0, 0], "VIS240.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "F18R.SEE", 1, 0),
            (1, 0x8, [0, -15, -60], "MIG31.ECM", 1, 0),
            (1, 0x8, [0, 0, 0], "PAVESPK.SEE", 1, 0),
            (2, 0xc, [7, -5, 55], "BK27.JT", 360, 0),
            (4, 0x205, [0, -25, 17], "F250.GAS", 2, 40),
            (1, 0x7e5, [6, -25, 14], "PAVEWAY.JT", 4, 80),
            (4, 0x505, [11, -25, 9], "MK82P3.JT", 2, 30),
            (4, 0x85, [16, -22, 4], "AIM9M.JT", 2, 5),
        ]
        .into_iter()
        .map(row)
        .collect()
    }

    type RowKey = (u8, i32, [i32; 3], Option<String>, i32, i32);

    fn key(rows: &[Hardpoint]) -> Vec<RowKey> {
        rows.iter()
            .map(|h| {
                (
                    h.location,
                    h.flags,
                    h.position,
                    h.store.clone(),
                    h.count,
                    h.weight_class,
                )
            })
            .collect()
    }

    #[test]
    fn the_tornado_carries_no_outer_wing_row_and_only_that_row_goes() {
        // John, 2026-11-10: "remove end wing stations typically reserved for
        // the pods". The row is the last, so no other index moves.
        let retail = tornado_retail();
        let mut rows = retail.clone();
        retire(AircraftId::Tornado, &mut rows);
        assert_eq!(key(&rows), key(&retail[..TORNADO_OUTER_WING]));
        // The inner wing pair (row 7) keeps its place and its Mk 82 default.
        assert_eq!(rows[7].store.as_deref(), Some("MK82P3.JT"));
        retire(AircraftId::Tornado, &mut rows);
        assert_eq!(rows.len(), 8);
        // Any other aircraft, or an edited outer row, keeps every row.
        for other in [AircraftId::Mig23, AircraftId::Su25, AircraftId::Mf1] {
            let mut rows = retail.clone();
            retire(other, &mut rows);
            assert_eq!(key(&rows), key(&retail));
        }
        let mut edited = retail.clone();
        edited[TORNADO_OUTER_WING].count = 4;
        retire(AircraftId::Tornado, &mut edited);
        assert_eq!(edited.len(), 9);
        let mut shifted = retail.clone();
        shifted.push(row((4, 0x85, [16, -22, 4], "AIM9M.JT", 2, 5)));
        retire(AircraftId::Tornado, &mut shifted);
        assert_eq!(shifted.len(), 10);
    }

    /// TOR-6b (John, 2026-11-11): the retired outer pair's place at the end of
    /// the table is taken by one AIM-9 side rail row, the retail row's own
    /// content (mask, class, default `AIM9M.JT` pair); guarded and idempotent.
    #[test]
    fn the_tornado_gains_one_side_rail_row_in_place_of_the_retired_outer_pair() {
        let retail = tornado_retail();
        let mut rows = retail.clone();
        retire(AircraftId::Tornado, &mut rows);
        side_rail(AircraftId::Tornado, &mut rows);
        assert_eq!(rows.len(), 9);
        // No other row moved: rows 0 to 7 are the retail rows.
        assert_eq!(key(&rows[..8]), key(&retail[..8]));
        let rail = &rows[TORNADO_SIDE_RAIL];
        assert_eq!(
            (rail.location, rail.flags, rail.count, rail.weight_class),
            (4, 0x85, 2, 5)
        );
        assert_eq!(rail.store.as_deref(), Some("AIM9M.JT"));
        // Repeated application, in either order, changes nothing.
        let once = key(&rows);
        retire(AircraftId::Tornado, &mut rows);
        side_rail(AircraftId::Tornado, &mut rows);
        assert_eq!(key(&rows), once);
        // Another aircraft, an edited inner row or an edited outer row keeps its own.
        let mut other = retail.clone();
        retire(AircraftId::Mig23, &mut other);
        side_rail(AircraftId::Mig23, &mut other);
        assert_eq!(key(&other), key(&retail));
        let mut edited = retail.clone();
        edited[TORNADO_INNER_WING].count = 4;
        retire(AircraftId::Tornado, &mut edited);
        side_rail(AircraftId::Tornado, &mut edited);
        assert_eq!(edited.len(), 8);
        let mut edited = retail.clone();
        edited[TORNADO_OUTER_WING].count = 4;
        retire(AircraftId::Tornado, &mut edited);
        side_rail(AircraftId::Tornado, &mut edited);
        assert_eq!(edited.len(), 9);
        assert_eq!(edited[8].count, 4);
    }

    #[test]
    fn the_tornado_side_rail_refuses_what_enters_the_airframe_and_the_bombs() {
        for name in TORNADO_SIDE_RAIL_REFUSED {
            assert!(
                !weapon_allowed(AircraftId::Tornado, TORNADO_SIDE_RAIL, name),
                "{name}"
            );
            assert!(weapon_allowed(AircraftId::Mig23, TORNADO_SIDE_RAIL, name));
        }
        for name in [
            "AIM9B.JT", "AIM9M.JT", "AIM9X.JT", "AA2.JT", "AA8.JT", "AA11.JT", "AA11B.JT",
        ] {
            assert!(
                weapon_allowed(AircraftId::Tornado, TORNADO_SIDE_RAIL, name),
                "{name}"
            );
        }
    }

    #[test]
    fn the_tornado_default_load_is_one_centreline_tank() {
        // John, 2026-11-11 (T3): one tank at x 0 instead of the crossing pair.
        let mut rows = tornado_retail();
        retire(AircraftId::Tornado, &mut rows);
        let retail = rows.clone();
        default_counts(AircraftId::Tornado, &mut rows);
        let mut expected = key(&retail);
        expected[TORNADO_CENTRELINE].4 = 1;
        assert_eq!(key(&rows), expected);
        // The record's own tank type and position stay; only the count moves.
        assert_eq!(rows[TORNADO_CENTRELINE].store.as_deref(), Some("F250.GAS"));
        assert_eq!(rows[TORNADO_CENTRELINE].position, [0, -25, 17]);
        default_counts(AircraftId::Tornado, &mut rows);
        assert_eq!(key(&rows), expected);
        // Another identity with the same rows, or an edited record, keeps its own.
        let mut other = retail.clone();
        default_counts(AircraftId::Mig23, &mut other);
        assert_eq!(key(&other), key(&retail));
        let mut edited = retail.clone();
        edited[TORNADO_CENTRELINE].store = Some("F350.GAS".into());
        default_counts(AircraftId::Tornado, &mut edited);
        assert_eq!(edited[TORNADO_CENTRELINE].count, 2);
        let mut edited = retail.clone();
        edited[TORNADO_CENTRELINE].count = 4;
        default_counts(AircraftId::Tornado, &mut edited);
        assert_eq!(edited[TORNADO_CENTRELINE].count, 4);
    }

    fn default_rows(id: AircraftId) -> Vec<Hardpoint> {
        default_signature(id).iter().copied().map(row).collect()
    }

    #[test]
    fn the_mirage_f1_centreline_keeps_its_station_but_defaults_to_no_pod() {
        let retail = default_rows(AircraftId::Mf1);
        assert_eq!(retail.len(), 8);
        assert_eq!(retail[MF1_CENTRELINE].store.as_deref(), Some("ALQ167.ECM"));
        let mut rows = retail.clone();
        adjust_defaults(AircraftId::Mf1, &mut rows);
        // Only the default store of the centreline row changes: same index,
        // same mask, count, weight class and position, so it accepts what it did.
        let mut expected = key(&retail);
        expected[MF1_CENTRELINE].3 = None;
        assert_eq!(key(&rows), expected);
        assert_eq!(rows[MF1_CENTRELINE].flags, 0x7e5);
        // The jammer suite (row 2), the tank pair and the Magics stay.
        assert_eq!(rows[2].store.as_deref(), Some("AV8.ECM"));
        assert_eq!(rows[5].store.as_deref(), Some("F250.GAS"));
        adjust_defaults(AircraftId::Mf1, &mut rows);
        assert_eq!(key(&rows), expected);
        // Another identity with the same rows, or an edited record: unchanged.
        let mut other = retail.clone();
        adjust_defaults(AircraftId::M2000, &mut other);
        assert_eq!(key(&other), key(&retail));
        for index in 0..retail.len() {
            let mut edited = retail.clone();
            edited[index].count += 1;
            adjust_defaults(AircraftId::Mf1, &mut edited);
            assert_eq!(
                key(&edited),
                key(&{
                    let mut r = retail.clone();
                    r[index].count += 1;
                    r
                })
            );
        }
    }

    #[test]
    fn the_f5e_has_the_f104_radar_the_f104_dispensers_and_an_empty_wing_pair() {
        let retail = default_rows(AircraftId::F5E);
        assert_eq!(retail.len(), 7);
        let mut rows = retail.clone();
        adjust_defaults(AircraftId::F5E, &mut rows);
        let mut expected = key(&retail);
        expected[F5E_RADAR].3 = Some("F104R.SEE".into());
        expected[F5E_JAMMER].3 = Some("F104.ECM".into());
        expected[F5E_WING_PAIR].3 = None;
        assert_eq!(key(&rows), expected);
        // Only the store changes on the countermeasures row (B5-3, John,
        // 2026-11-10): no index, mask, count or position moves.
        assert_eq!(
            (
                rows[F5E_JAMMER].flags,
                rows[F5E_JAMMER].position,
                rows[F5E_JAMMER].count
            ),
            (0x8, [0, 0, -50], 1)
        );
        // The wing pair still takes what its mask took (0x7e5: the Sparrow
        // among them), and the AIM-9B wingtip default stays.
        assert_eq!(rows[F5E_WING_PAIR].flags, 0x7e5);
        assert_eq!(
            (rows[F5E_WING_PAIR].count, rows[F5E_WING_PAIR].weight_class),
            (4, 40)
        );
        assert_eq!(rows[6].store.as_deref(), Some("AIM9B.JT"));
        assert_eq!(rows[3].store.as_deref(), Some("M61.JT"));
        assert_eq!(rows[4].store.as_deref(), Some("F150.GAS"));
        adjust_defaults(AircraftId::F5E, &mut rows);
        assert_eq!(key(&rows), expected);
        // The F-5E's retail rows on another identity, or edited, stay retail.
        for id in [AircraftId::F4E, AircraftId::F104, AircraftId::Mf1] {
            let mut other = retail.clone();
            adjust_defaults(id, &mut other);
            assert_eq!(key(&other), key(&retail));
        }
        for index in 0..retail.len() {
            let mut edited = retail.clone();
            edited[index].weight_class += 1;
            let before = key(&edited);
            adjust_defaults(AircraftId::F5E, &mut edited);
            assert_eq!(key(&edited), before);
        }
    }
}
