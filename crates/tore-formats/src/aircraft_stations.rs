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
/// The F-15E's conformal-style bomb pair (`F15E.PT` row 7, six stores a side)
/// and its wing pair (row 8).
pub const F15E_BELLY: usize = 7;
pub const F15E_WING: usize = 8;
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
/// The Mirage F1's belly row (`MF1.PT` row 5, four stores) whose capacity
/// [`adjust_defaults`] cuts to two.
pub const MF1_BELLY: usize = 5;
/// The capacity of the Mirage F1's belly row after [`adjust_defaults`]: one store
/// a side, as the real aircraft carries one tank on each inner wing pylon,
/// never four abreast.
pub const MF1_BELLY_CAPACITY: i32 = 2;
/// The Mirage F1's paired wing row (`MF1.PT` row 6, two `R550.JT`).
pub const MF1_WING: usize = 6;
/// The B-52's tail gun row (`B52.PT` row 4, `M61.JT`, slewed aft) whose round
/// count [`adjust_defaults`] sets to the real magazine.
pub const B52_GUN: usize = 4;
/// The B-52H's magazine (BMB-A2, John's decision 13): 1,242 rounds of 20 mm
/// (aircraftinformation.info B-52H page, read 2026-11-15, fed by the gunner's
/// AN/ASG-21). The record carries 2,000, the Tu-95 and Tu-26 tail-gun rows'
/// number.
pub const B52_GUN_ROUNDS: i32 = 1_242;
/// Where the B-52's tail gun hangs, record units (a third of a foot): on the
/// centre line at the end of the drawn tail cone, 92 ft aft and 2 ft up
/// (`B52.SH` y -138 and z 3 units at 2/3 ft a unit). The retail mount is
/// (0, 20, -220), 73 ft aft, 19 ft ahead of the cone (B52-0). Fitted; John's
/// decision 13, 2026-11-15 (B52-5).
pub const B52_GUN_MOUNT: [i32; 3] = [0, 6, -276];
/// The B-52's three internal bay rows (`B52.PT` rows 3, 5 and 6): the
/// retail 12 stores and 16,600 lb class become the real bay's 27 bombs of
/// 500 lb class (nine a row) and 7,000 lb a row, 21,000 lb in all (B52-5,
/// opinionated, John's decision 7; the real bay is 27 bombs or 8 heavy
/// stores, aircraftinformation.info). The row's default store goes: the bay
/// starts empty.
pub const B52_BAY_ROWS: [usize; 3] = [3, 5, 6];
pub const B52_BAY_ITEMS: i32 = 9;
pub const B52_BAY_CLASS: i32 = 70;
/// The B-52's underwing pylon pair (decision 8): row 7, one pylon a wing (a
/// wing row is always a pair), 17.3 ft out and 10.3 ft forward (the middle of
/// the free span between the fuselage and the inboard engine pod and of the
/// wing's chord there, measured, B52-0), store axis 1 ft under the wing's
/// underside (3.8 ft up there). Capacity 18 a row (nine a wing) is opinionated
/// (the real stub pylon takes 12 to 18, aircraftinformation.info). The weight
/// class is 17,000 lb, not the 20,000 lb of B52-0's proposal (John's decision 8):
/// a row's default weapon loads at its weight limit, and a saved load must equal
/// the default count (`mission.rs`), so the class that holds the default 12
/// Harpoons (1,380 lb each) and no more is 166 to 179; 20,000 lb would admit 14.
/// Twelve is also the real figure (aircraftinformation.info: Harpoon 8 to 12
/// a pylon pair). Mk 82 reaches 18 (9,000 lb) at this class. The flags are the bay row's bomb mask (`0x1100`), the pair bit and
/// the base bit. The record holds the default 12 AGM-84E Harpoons, six a wing
/// (John's decision 9).
pub const B52_PYLONS: usize = 7;
pub const B52_PYLON_FLAGS: i32 = 0x1105;
pub const B52_PYLON_MOUNT: [i32; 3] = [52, 8, 31];
pub const B52_PYLON_ITEMS: i32 = 18;
pub const B52_PYLON_CLASS: i32 = 170;
/// The default Harpoons, 12.
pub const B52_PYLON_DEFAULT: i32 = 12;
/// Stores the underwing pylon row refuses though its mask and weight class
/// admit them: the 13.7 to 13.9 ft stores whose three rows of three, 1.05
/// lengths apart, reach the flap strip and the wing's trailing edge (0.00 to
/// 0.01 ft at capacity, measured with the pylon clearance report, B52-5,
/// 2026-11-15): `AGM45.JT`, `AGM88.JT` (HARM class) and the three
/// GBU-27 class (`GBU29.JT`, `GBU29P.JT`, `GBU30.JT`).
pub const B52_PYLON_REFUSED: [&str; 5] =
    ["AGM45.JT", "AGM88.JT", "GBU29.JT", "GBU29P.JT", "GBU30.JT"];
/// Stores the B-52's bay rows refuse though the retail masks admit them,
/// where the measured grid (the shared magazine grid of B2-5: 4 abreast at
/// the span plus 0.03 ft capped at 1.3 ft, one long in an 8.67 ft slab, tiers
/// up to a flat roof 6 ft over the belly plane, a 6 ft wide recess, stores
/// hung by their shape's origin) leaves under 0.05 ft to a wall, the roof, the
/// floor, a rail or post, or another store, or runs through one: the long
/// stores, the 3 ft span missiles the 0x20 mask admits on rows 5 and 6, the
/// Snakeye, and (since the fold onto the shared grid, whose 1.3 ft pitch is
/// tighter than B52-5's span plus 0.15 ft) `GBU38.JT` and `RBK250.JT`, whose
/// fins now touch their neighbours. Three stay (`MK82`, `MK82P3`, `FAB250`);
/// measured with the bay clearance sweep, B52-5 and the batch A integration,
/// 2026-11-15. See `docs/spec/stores-on-pylons.md`.
pub const B52_BAY_REFUSED: [&str; 22] = [
    "AGM45.JT",
    "AGM84A.JT",
    "AGM84E.JT",
    "AGM88.JT",
    "AM39.JT",
    "AS16.JT",
    "AT2.JT",
    "CBU87.JT",
    "CBU89.JT",
    "FAB1000.JT",
    "FAB500.JT",
    "GBU29.JT",
    "GBU29P.JT",
    "GBU30.JT",
    "GBU38.JT",
    "MK20.JT",
    "MK82AIR.JT",
    "MK82HD.JT",
    "MK82P.JT",
    "MK84.JT",
    "RBK250.JT",
    "RBK500.JT",
];
/// The B-2's four internal bay rows (`B2.PT` rows 5 to 8, flags `0x1504`):
/// [`adjust_defaults`] clears the designator bit `0x400` on them (BMB-A2) and
/// the pair flag `0x4` (B2-5).
pub const B2_BAY_ROWS: std::ops::Range<usize> = 5..9;
/// The B-2's two Mk 82 rows (`B2.PT` rows 5 and 6, one a bay) and its two
/// Paveway III rows (rows 7 and 8): the rows of one bay are alternatives
/// ([`tore_sim::combat::bays::alternative`]).
pub const B2_MK82_ROWS: std::ops::Range<usize> = 5..7;
pub const B2_PAVEWAY_ROWS: std::ops::Range<usize> = 7..9;
/// The stores the B-2's two Mk 82 rows (rows 5 and 6) refuse (B2-5, fitted,
/// the 0.3 ft rule of the F-117's bays, measured with the doors open at each
/// store's full count under the grid's ceiling): the three Paveway III class
/// laser and JDAM bombs, whose 5.4 ft fins stand 0.9 to 1.2 ft through the roof
/// and skin and 0.65 ft through the door planes on a row that is not hung
/// low (they are the Paveway rows' stores), and the Mk 20 Rockeye (0.42 ft
/// through the roof). The Paveway rows (7 and 8) refuse nothing: every store
/// there clears the doors and the ends by 0.5 ft or more, bar the GBU-27
/// class, whose fin tips stand 0.65 ft past the door planes on every row.
pub const B2_MK82_REFUSED: [&str; 4] = ["GBU29.JT", "GBU29P.JT", "GBU30.JT", "MK20.JT"];
/// A station's pair flag (`0x4`): a record at x mirrors to -x and its stores
/// alternate sides.
pub const PAIRED_BIT: i32 = 0x4;
/// The designator bit of a station's flags (a targeting or laser pod may sit
/// there, and a multi-store pod: `loading.rs`).
pub const DESIGNATOR_BIT: i32 = 0x400;
/// The B-1B's three bay rows (`B1.PT` rows 5 to 7; BMB-B2).
pub const B1B_BAY_ROWS: std::ops::Range<usize> = 5..8;
/// The F-111F's four wing pair rows (`F111.PT` rows 5 to 8; BMB-B2).
pub const F111_WING_ROWS: std::ops::Range<usize> = 5..9;
/// The real F-111 pylon rating as a weight class (class x 100 lb, BMB-B2):
/// 5,000 lb a store.
pub const F111_WING_CLASS: i32 = 50;
/// The active-radar seeker bit of a station's flags (`loading.rs`: an
/// active-radar missile such as the Kh-15, `AS16.JT`, needs it).
pub const ACTIVE_RADAR_BIT: i32 = 0x20;
/// The Tu-160's two bay rows (`TU160.PT` rows 4 and 5, flags `0x1184`, six
/// `AS16.JT` each, weight class 180): [`adjust_defaults`] clears the pair flag
/// `0x4` (T160-5), so a row is one bay, not a pair that mirrors onto the other
/// (both rows sit on one point at x 0). The designator bit was never set.
pub const TU160_BAY_ROWS: std::ops::Range<usize> = 4..6;
/// The stores the Tu-160's two bay rows refuse though the retail mask admits
/// them (the mask itself refuses air-to-air missiles, `AS15.JT` the Kh-55 and
/// every radar-guided store but the default): the five that do not fit a 7.6 ft
/// wide, 6 ft deep bay on the shared grid with the doors open at the row's full
/// count, under the 0.05 ft rule of the B-52's bays on the walls, door leaves,
/// ends, roof, floor and rail (store to store is not a test here: the default
/// Kh-15's 3 ft fins cross their neighbours' on the grid's 1.59 ft pitch, as the
/// B-2's Paveway III do). `AT12.JT` hangs 0.41 ft below the belly plane (out of
/// the shut skin); the GBU-27 class (`GBU29.JT`, `GBU29P.JT`, `GBU30.JT` and
/// `PAVEWA3.JT`) has 5.4 ft fins and stands 0.85 ft through the side walls.
/// Measured with the bay clearance sweep, T160-5, 2026-11-16 (fitted); see
/// docs/formats/aircraft-variety.md "Tu-160 stations, bays and pylons".
pub const TU160_BAY_REFUSED: [&str; 5] =
    ["AT12.JT", "GBU29.JT", "GBU29P.JT", "GBU30.JT", "PAVEWA3.JT"];
/// The Tu-26's bay row (`TU26.PT` row 5, flags `0x1500`; BMB-C2):
/// [`adjust_defaults`] clears the designator bit and sets the active-radar bit
/// so the bay carries the Kh-15.
pub const TU26_BAY: usize = 5;
/// The Tu-26's wing glove rows (`TU26.PT` rows 7 and 8, AS-16 pairs of four and
/// six) whose capacity, and so default load, [`adjust_defaults`] cuts to two
/// each (BMB-C2).
pub const TU26_GLOVE_ROWS: std::ops::RangeInclusive<usize> = 7..=8;
/// The capacity of each Tu-26 glove row: one store a side, so four AS-16
/// underwing in all, the real Kh-15 count (`TU26.INF`; BMB-C2). A default
/// store's capacity is the row's count (`Loadout::capacity`, the mission
/// validator), so the default cannot be capped without the capacity.
pub const TU26_GLOVE_CAPACITY: i32 = 2;
/// The Tu-95's two Bear-A to F gun turret rows (`TU95.PT` rows 5 and 6,
/// `T23_1.JT`, flags `0x180a`), whose defaults [`adjust_defaults`] clears
/// (BMB-C2); row 3, the tail turret, stays.
pub const TU95_EXTRA_GUNS: std::ops::RangeInclusive<usize> = 5..=6;
/// The Tu-95's wing pylon row (`TU95.PT` row 8, flags `0x17e5`, paired, six):
/// [`adjust_defaults`] moves it out and up, trims its mask and raises its
/// count to the Tu-95MS16's ten, and [`weapon_allowed`] leaves only the AS-16
/// on it.
pub const TU95_WING_PYLONS: usize = 8;
/// The Tu-95 pylon row's new position in record units (a third of a foot):
/// 36 is 12 ft out (fitted, opinionated), up -6 is 2 ft below the reference
/// point, 0.7 ft above the 1.3 ft wing underside's lowest plane and 7.3 ft
/// above the gear's ground-contact plane (-28 units; the retail -35 was 2.4 ft
/// below it), forward 35 stays (the record's).
pub const TU95_WING_POSITION: [i32; 3] = [36, -6, 35];
/// The Tu-95's pylon row mask: the retail `0x17e5` (every seeker, bomb, tank
/// and pod bit) down to the reviewed row, the pair flag, the base bit and the
/// active-radar seeker bit the AS-16 needs (`0x1025`).
pub const TU95_WING_FLAGS: i32 = 0x1025;
/// The Tu-95 pylon row's capacity: ten stores, five a side (the Tu-95MS16's
/// ten underwing Kh-55s, Wikipedia "Kh-55", read 2026-11-15; opinionated, the
/// real pylon arrangement was not found).
pub const TU95_WING_COUNT: i32 = 10;
/// The Su-34's gun row (`SU34.PT` row 5, `GSH301.JT`) whose round count
/// [`adjust_defaults`] sets to the real magazine (BMB-C2).
pub const SU34_GUN: usize = 5;
/// The Su-34's GSh-30-1 magazine: 180 rounds (Wikipedia "Sukhoi Su-34", read
/// 2026-11-15). The record carries the Su-27's 150.
pub const SU34_GUN_ROUNDS: i32 = 180;
/// The Su-34's fuselage row (`SU34.PT` row 6, four `AS14.JT`, flags `0x7e5`)
/// whose height [`adjust_defaults`] raises (BMB-C2).
pub const SU34_FUSELAGE: usize = 6;
/// The Su-34 fuselage row's new height in record units: -18 is 6 ft below the
/// reference point (fitted), 2.3 ft above the main wheels' bottoms (-25 shape
/// units, 8.3 ft) and 0.7 ft below the nacelle belly (-16 units, 5.3 ft). The
/// retail -30 is 10 ft down, 1.7 ft below the wheels.
pub const SU34_FUSELAGE_UP: i32 = -18;
/// The F-117's two internal bay rows (`F117.PT` rows 4 and 5, flags `0x1100`,
/// one store each): [`weapon_allowed`] refuses the stores that cannot clear
/// the fitted bays and [`extra_weapon`] admits the Paveway II laser bombs.
pub const F117_BAY_ROWS: std::ops::RangeInclusive<usize> = 4..=5;
/// The one Paveway II the F-117's bay rows refuse by name: the 1,000 lb GBU-16
/// shares its donor with the GBU-12, so the audit's donor rule needs the refusal
/// stated, though the real jet's laser bombs are the GBU-10, 12 and 27
/// ([`weapon_allowed`]).
pub const F117_BAY_REFUSED: [&str; 1] = ["GBU16.JT"];
/// The laser-guided bombs the F-117's bay rows take beyond the retail mask
/// `0x1100` (bomb signature only, which refuses every seeker-guided bomb and
/// leaves the GBU-27 only as the row's own default): the GBU-10 class
/// (`GBU10.JT`, `GBU10A.JT`, `PAVEWAY.JT`) and the GBU-12 (`GBU12.JT`), the
/// real F-117's other laser bombs (F117-0 sources, F117-5, 2026-11-15).
pub const F117_EXTRA_BOMBS: [&str; 4] = ["GBU10.JT", "GBU10A.JT", "PAVEWAY.JT", "GBU12.JT"];
/// The F-117's jammer row (`F117.PT` row 2, `F18.ECM`), whose default
/// [`adjust_defaults`] clears.
pub const F117_JAMMER: usize = 2;
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

/// Stores normal loading refuses on one source row of the F-15E (John,
/// 2026-11-10, opinionated; F15E-5 round 2). Row 7 refuses the stores that,
/// after the 0.1 ft belly rail drop, still enter the intake flank by more than
/// about 0.3 ft (measured, `docs/spec/stores-on-pylons.md`): the 2,000 lb
/// guided bombs and the 500 lb guided bomb (2.04, 1.83 and 0.32 ft), the two
/// anti-radiation missiles (0.92 ft) and the AT-2 dispenser (0.46 ft). Row 8
/// refuses the AA-6, whose 20 ft body crosses the wing. Cheat loading skips
/// the check; every other aircraft and row keeps its stores. The Mirage F1
/// has its own refusals (agent decision, 2026-11-11, following John's F-15E A2
/// answer, the 0.3 ft line; the F-5E kept its wing entries, C2; MF1-6): row 5
/// (the belly pair at 3.24 ft, [`MF1_BELLY_REFUSED`], 33 stores) and row 6 (the
/// paired wing row, [`MF1_WING_REFUSED`], `AS30.JT` at 0.33 ft) refuse the stores
/// that enter the airframe by more than about 0.3 ft. The default `R550.JT` is
/// never refused (its left Magic reads 0.05 ft on row 6, kept).
/// The Tornado has its own refusals (agent decision, 2026-11-11, John's F-15E
/// A2 precedent, TOR-6 and TOR-6b): the shoulder pair (row 6,
/// [`TORNADO_SHOULDER_REFUSED`]), the inner wing pair (row 7,
/// [`TORNADO_INNER_WING_REFUSED`]) and the AIM-9 side rails (row 8,
/// [`TORNADO_SIDE_RAIL_REFUSED`], which otherwise take what their infrared
/// mask (0x85) admits, as the F-14's glove-pylon rail does). The F-117's two bay
/// rows refuse only [`F117_BAY_REFUSED`] (F117-5: the measured bay clearances
/// leave every store the mask and [`extra_weapon`] admit clear of the doors).
/// The B-2's two Mk 82 rows refuse [`B2_MK82_REFUSED`] (B2-5).
/// The Tu-95's wing pylon row (row 8) refuses everything but `AS16.JT`
/// (BMB-C2).
pub fn weapon_allowed(id: AircraftId, row: usize, source: &str) -> bool {
    match id.source() {
        AircraftId::F15E => match row {
            F15E_BELLY => !F15E_BELLY_REFUSED.contains(&source),
            F15E_WING => source != "AA6.JT",
            _ => true,
        },
        AircraftId::Mf1 => match row {
            MF1_BELLY => !MF1_BELLY_REFUSED.contains(&source),
            MF1_WING => !MF1_WING_REFUSED.contains(&source),
            _ => true,
        },
        AircraftId::F117 => !(F117_BAY_ROWS.contains(&row) && F117_BAY_REFUSED.contains(&source)),
        AircraftId::B2 => !(B2_MK82_ROWS.contains(&row) && B2_MK82_REFUSED.contains(&source)),
        AircraftId::Tu160 => {
            !(TU160_BAY_ROWS.contains(&row) && TU160_BAY_REFUSED.contains(&source))
        }
        AircraftId::B52 => {
            let refused = if row == B52_PYLONS {
                &B52_PYLON_REFUSED[..]
            } else if B52_BAY_ROWS.contains(&row) {
                &B52_BAY_REFUSED[..]
            } else {
                &[]
            };
            !refused.contains(&source)
        }
        // The Tu-95's wing pylons take the AS-16 and nothing else (BMB-C2):
        // the active-radar bit the Kh-15 needs also admits the Harpoon class.
        AircraftId::Tu95 => row != TU95_WING_PYLONS || source == "AS16.JT",
        AircraftId::Tornado => match row {
            TORNADO_SHOULDER => !TORNADO_SHOULDER_REFUSED.contains(&source),
            TORNADO_INNER_WING => !TORNADO_INNER_WING_REFUSED.contains(&source),
            TORNADO_SIDE_RAIL => !TORNADO_SIDE_RAIL_REFUSED.contains(&source),
            _ => true,
        },
        _ => true,
    }
}

/// The weapons row 7 of the F-15E refuses ([`weapon_allowed`]).
pub const F15E_BELLY_REFUSED: [&str; 7] = [
    "GBU29.JT",
    "GBU29P.JT",
    "GBU30.JT",
    "GBU38.JT",
    "AGM88.JT",
    "AGM45.JT",
    "AT2.JT",
];

/// The weapons row 5 of the Mirage F1 refuses ([`weapon_allowed`]): measured
/// airframe entries of 0.37 to 3.39 ft (`docs/spec/stores-on-pylons.md`). The 13
/// at 0.3 ft or less stay (AA11, AA12, AS7, AIM9X, AIM120, AGM65G, AT2,
/// FAB1000, CBU89, GBU38, AGM65A, and the F350 and F500 tanks).
pub const MF1_BELLY_REFUSED: [&str; 33] = [
    "AA11B.JT",
    "AA2.JT",
    "AA6.JT",
    "AA9.JT",
    "AIM9M.JT",
    "MICA.JT",
    "R530.JT",
    "R550.JT",
    "PL7.JT",
    "PL10.JT",
    "AGM88.JT",
    "AIM7.JT",
    "AIM7E.JT",
    "AIM9B.JT",
    "GBU12.JT",
    "GBU16.JT",
    "GBU10.JT",
    "GBU10A.JT",
    "PAVEWAY.JT",
    "PAVEWA3.JT",
    "GBU28.JT",
    "GBU29.JT",
    "GBU29P.JT",
    "GBU30.JT",
    "AS14.JT",
    "AS30.JT",
    "AT12.JT",
    "AGM84E.JT",
    "AGM45.JT",
    "AGM84A.JT",
    "AM39.JT",
    "AS16.JT",
    "AS15.JT",
];

/// The weapons row 6 of the Mirage F1 refuses ([`weapon_allowed`]): only the
/// `AS30.JT`, 0.33 ft into the wing, the one entry over the 0.3 ft line. The
/// six that enter by 0.06 to 0.15 ft (`PL7`, `PL10`, `AGM88`, `AGM45`, `AIM7`,
/// `AIM7E`) and the default `R550.JT` (0.05 ft on the left) are kept.
pub const MF1_WING_REFUSED: [&str; 1] = ["AS30.JT"];

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

/// Whether `rows` are exactly the retail rows of `id`'s [`default_signature`]
/// table (every source row, so an edited record keeps its own stations).
fn is_retail(id: AircraftId, rows: &[Hardpoint]) -> bool {
    let signature = default_signature(id);
    !signature.is_empty()
        && rows.len() == signature.len()
        && rows.iter().zip(signature).all(|(h, s)| {
            (
                h.location,
                h.flags,
                h.position,
                h.store.as_deref().unwrap_or(""),
                h.count,
                h.weight_class,
            ) == (s.0, s.1, s.2, s.3, s.4, s.5)
        })
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
        slew: [0; 4],
    });
}

/// The Tornado's jammer and countermeasures row (`TOR.PT` row 2: Fuselage,
/// flag 0x8, `MIG31.ECM`), which the Clean version drops ([`variant`]).
pub const TORNADO_JAMMER: usize = 2;

/// The record [`TORNADO_JAMMER`] holds in the Combat version and the retail
/// file: 60 chaff, 60 flares and a deception jammer (one bundle).
pub const TORNADO_JAMMER_RECORD: &str = "MIG31.ECM";

/// Runtime variants' station tables (John, 2026-11-12, TOR-13, opinionated):
/// the Tornado IDS (Clean) is the Tornado record with no jammer, chaff or
/// flares, which is how an aircraft has none of them: the jammer row holds no
/// store (the F-5E's B5-2 interim and the sensor profile read a countermeasure
/// bundle only from the row that holds one). `MIG31.ECM` is one record, so the
/// jammer, the chaff and the flares go together. Guarded on the exact record
/// name; every other identity and an edited record are untouched; repeated
/// application is a no-op. Applied by [`super::Aircraft::into_identity`].
pub fn variant(id: AircraftId, rows: &mut [Hardpoint]) {
    if id != AircraftId::TornadoClean {
        return;
    }
    if let Some(row) = rows.get_mut(TORNADO_JAMMER)
        && row
            .store
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(TORNADO_JAMMER_RECORD))
    {
        row.store = None;
    }
}

/// A pod an aircraft carries as fixed external equipment on a station that no
/// source row records: the Tornado IDS (Combat)'s two outer wing pylons
/// (John, 2026-11-12, TOR-13). It is not a hardpoint row, so Load Ordnance,
/// saved loads, the lobby and the AI never see it and cannot change it; the
/// simulation builds it with the aircraft's configuration (a
/// `Configuration::fixed_pods` entry, its mass in the fixed external
/// equipment) and the drawing finds its model by [`Self::record`]. There is no
/// retail equipment record for these pods, so `record` is an authored
/// identifier, never a file name, and the model of `crates/tore-app/src/pylons/
/// pod_models.rs` carries the same key (`assets/pods/build_pods.py`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoredPod {
    /// The station index, past the source table's rows, that the pod's mount
    /// and swivel are keyed by (`hardpoints::station_move`, `swing::swivels`).
    pub row: usize,
    /// The authored model's key.
    pub record: &'static str,
    /// Which wing it hangs from: the right-hand one.
    pub right: bool,
    /// Drawn and carried length and diameter, feet, and carried mass, pounds.
    pub length: f64,
    pub diameter: f64,
    pub weight_lbs: i32,
    /// Chaff and flares leave from the model's aft face centre.
    pub dispenser: bool,
    /// How far aft of the pod's middle its lugs (the point its pylon holds it
    /// by) are, feet. Update (TOR-14b, 2026-11-13, John: "Both pods are
    /// technically mounted towards the rear in RL"): the middle of the pod's
    /// rear third, a third of its length aft of its middle. Fitted (agent
    /// decision; no source gives the lug station), so the pod hangs mostly
    /// forward of its pylon.
    pub lug_aft: f64,
}

/// The first station index past the Tornado's nine source rows.
pub const TORNADO_RIGHT_POD: usize = 9;
pub const TORNADO_LEFT_POD: usize = 10;

/// The ARI 23246 Sky Shadow ECM pod, 3.0 m (9.84 ft), 200 kg (441 lb), and the
/// BOZ-107 chaff and flare dispenser, 4.0 m (13.12 ft), 325 kg (717 lb): real
/// lengths and masses (Wikipedia, "Sky Shadow (radar jammer)", and the Czech
/// armedconflicts.com database, secondary and approximate, both read
/// 2026-11-12); diameters 0.42 m (1.38 ft) and 0.48 m (1.58 ft) are fitted from
/// John's labelled photo (`assets/pods/build_pods.py`).
pub const SKY_SHADOW: AuthoredPod = AuthoredPod {
    row: TORNADO_LEFT_POD,
    record: "SKYSHADOW",
    right: false,
    length: 3.0 / 0.3048,
    diameter: 0.42 / 0.3048,
    weight_lbs: 441,
    dispenser: false,
    lug_aft: 3.0 / 0.3048 / 3.,
};
pub const BOZ_107: AuthoredPod = AuthoredPod {
    row: TORNADO_RIGHT_POD,
    record: "BOZ107",
    right: true,
    length: 4.0 / 0.3048,
    diameter: 0.48 / 0.3048,
    weight_lbs: 717,
    dispenser: true,
    lug_aft: 4.0 / 0.3048 / 3.,
};

/// The authored fixed pods of `id`: the Combat Tornado's outer pair, the Sky
/// Shadow on the left (port) pylon and the BOZ-107 on the right (starboard)
/// one (John's labelled photo, 2026-11-12). The Clean version and every other
/// aircraft carry none.
pub fn authored_pods(id: AircraftId) -> &'static [AuthoredPod] {
    if id == AircraftId::Tornado {
        &[BOZ_107, SKY_SHADOW]
    } else {
        &[]
    }
}

/// The authored pod keyed `record` (for its drawn size), if any.
pub fn authored_pod(record: &str) -> Option<AuthoredPod> {
    [BOZ_107, SKY_SHADOW]
        .into_iter()
        .find(|pod| pod.record.eq_ignore_ascii_case(record))
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
        // The bomber batch A records (BMB-A2). The empty B-52 bay rows have no
        // default store (`""`).
        AircraftId::B52 => &[
            (1, 0x1008, [0, 0, 0], "VIS300.SEE", 1, 0),
            (1, 0x1008, [0, 0, 0], "B52R.SEE", 1, 0),
            (1, 0x1008, [0, 0, 0], "B52.ECM", 1, 0),
            (3, 0x1100, [0, -40, 39], "AGM84E.JT", 12, 166),
            (1, 0x180a, [0, 20, -220], "M61.JT", 2_000, 0),
            (3, 0x1120, [0, -40, 13], "", 12, 166),
            (3, 0x1120, [0, -40, -13], "", 12, 166),
        ],
        AircraftId::B2 => &[
            (1, 0x1008, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 0x1008, [0, 0, 0], "F18R.SEE", 1, 0),
            (1, 0x1008, [0, 0, -50], "F18.ECM", 1, 0),
            (1, 0x1008, [0, 0, 0], "PAVESPK.SEE", 1, 0),
            (1, 0x1008, [0, 0, 0], "AV8IR.SEE", 1, 0),
            (3, 0x1504, [-15, -38, 34], "MK82.JT", 32, 160),
            (3, 0x1504, [14, -38, 35], "MK82.JT", 32, 160),
            (3, 0x1504, [-15, -38, 6], "PAVEWA3.JT", 4, 160),
            (3, 0x1504, [14, -38, 7], "PAVEWA3.JT", 4, 160),
        ],
        AircraftId::F117 => &[
            (1, 0x1008, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 0x1008, [0, 0, 0], "PAVESPK.SEE", 1, 0),
            (1, 0x1008, [0, 0, -50], "F18.ECM", 1, 0),
            (1, 0x1008, [0, 0, 0], "AV8IR.SEE", 1, 0),
            (3, 0x1100, [-10, -22, -2], "PAVEWA3.JT", 1, 20),
            (3, 0x1100, [10, -22, -2], "PAVEWA3.JT", 1, 20),
        ],
        // The bomber batch B rows (BMB-B2). The Tu-160's two bay rows carry no
        // designator bit (0x1184); T160-5 adds its entry for the pair flag.
        AircraftId::Tu160 => &[
            (1, 0x1008, [0, 0, 0], "VIS300.SEE", 1, 0),
            (1, 0x1008, [0, 0, 0], "TU160R.SEE", 1, 0),
            (1, 0x1008, [0, 0, -50], "TU160.ECM", 1, 0),
            (1, 0x1008, [0, 0, 0], "SU37L.SEE", 1, 0),
            (3, 0x1184, [0, -50, 91], "AS16.JT", 6, 180),
            (3, 0x1184, [0, -50, 91], "AS16.JT", 6, 180),
        ],
        AircraftId::B1b => &[
            (1, 0x1008, [0, 0, 0], "VIS340.SEE", 1, 0),
            (1, 0x1008, [0, 0, 0], "F18R.SEE", 1, 0),
            (1, 0x1008, [0, 0, -50], "F18.ECM", 1, 0),
            (1, 0x1008, [0, 0, 0], "PAVESPK.SEE", 1, 0),
            (1, 0x1008, [0, 0, 0], "AV8IR.SEE", 1, 0),
            (3, 0x1500, [0, -31, 110], "MK82AIR.JT", 4, 140),
            (3, 0x1500, [0, -31, 63], "MK82AIR.JT", 4, 140),
            (3, 0x1500, [0, -31, -37], "MK82AIR.JT", 4, 140),
        ],
        AircraftId::F111 => &[
            (1, 0x8, [0, 0, 0], "VIS240.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "A6R.SEE", 1, 0),
            (1, 0x8, [0, 0, -50], "A6.ECM", 1, 0),
            (1, 0x8, [0, 0, 0], "AV8IR.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "PAVESPK.SEE", 1, 0),
            (4, 0x7e5, [29, -12, -4], "GBU28.JT", 2, 70),
            (4, 0x7e5, [41, -12, -8], "MK82P.JT", 2, 70),
            (4, 0x7e5, [54, -12, -13], "AGM65G.JT", 4, 70),
            (4, 0x7e5, [65, -12, -17], "MK82P.JT", 2, 60),
        ],
        // The bomber batch C rows (BMB-C2). The Tu-26's bay row has no default
        // store (`""`) and is the one that takes the active-radar bit.
        AircraftId::Tu26 => &[
            (1, 0x1008, [0, 0, 0], "VIS300.SEE", 1, 0),
            (1, 0x1008, [0, 0, 0], "TU26R.SEE", 1, 0),
            (1, 0x1008, [0, 0, -100], "TU26.ECM", 1, 0),
            (1, 0x180a, [0, 22, -130], "T23_1.JT", 2_000, 0),
            (1, 0x1008, [0, 0, 0], "SU37L.SEE", 1, 0),
            (3, 0x1500, [0, -40, 72], "", 8, 120),
            (1, 0x1185, [19, -40, 29], "AS16.JT", 2, 54),
            (4, 0x1185, [34, -35, 13], "AS16.JT", 4, 108),
            (4, 0x1185, [50, -35, -1], "AS16.JT", 6, 160),
        ],
        AircraftId::Tu95 => &[
            (1, 0x1008, [0, 0, 0], "VIS300.SEE", 1, 0),
            (1, 0x1008, [0, 0, 0], "TU95R.SEE", 1, 0),
            (1, 0x1008, [0, 0, -100], "TU95.ECM", 1, 0),
            (1, 0x180a, [0, 0, -100], "T23_1.JT", 2_000, 0),
            (1, 0x1008, [0, 0, 0], "SU37L.SEE", 1, 0),
            (1, 0x180a, [0, 16, 20], "T23_1.JT", 2_000, 0),
            (1, 0x180a, [0, -15, -20], "T23_1.JT", 2_000, 0),
            (3, 0x1180, [0, -35, 38], "AS16.JT", 6, 160),
            (4, 0x17e5, [27, -35, 35], "", 6, 160),
        ],
        AircraftId::Su34 => &[
            (1, 0x8, [0, 0, 0], "VIS300.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "SU27R.SEE", 1, 0),
            (1, 0x8, [0, 0, 0], "SU27I.SEE", 1, 0),
            (1, 0x8, [0, 0, -50], "SU27.ECM", 1, 0),
            (1, 0x8, [0, 0, 0], "SU37L.SEE", 1, 0),
            (2, 0x8, [10, 5, 60], "GSH301.JT", 150, 0),
            (1, 0x7e5, [24, -30, -13], "AS14.JT", 4, 80),
            (4, 0x7e5, [50, -15, -23], "AT12.JT", 4, 80),
            (5, 0x85, [81, -1, -34], "AA11.JT", 2, 10),
        ],
        _ => &[],
    }
}

/// Default-load corrections (John, 2026-11-10, opinionated, batch decision
/// round), applied after decoding only when every source row is exactly the
/// retail one. No row is added, removed or moved and no row's mask, count,
/// weight class or position changes, so every station index and everything
/// each station accepts stays as decoded; only what the row holds by default
/// changes (`store`), with one exception, the Mirage F1's belly capacity
/// (below). Update (BMB-A2 to BMB-C2, 2026-11-15): the bomber records also take
/// flag, weight class, capacity and position changes where listed below (the
/// B-2's, B-1B's and F-111F's flags, the F-111F's class, the Tu-26's flags and
/// glove capacity, the Tu-95's pylon row, the Su-34's fuselage row height);
/// no row is added or removed, so every index stays.
///
/// - **Mirage F1** (`MF1.PT` row 4, the centreline station): no default
///   `ALQ167.ECM` pod. The record's American jammer pod is not in Jane's list
///   of the F1's loads (`MF1.INF`); the station is real and still takes what
///   its mask admits. Update (MF1-5b, 2026-11-11, opinionated, John's round 3,
///   item M1): the belly row (row 5, Wing, paired, four `F250.GAS`, class 46)
///   has capacity two (one a side, the real aircraft's tank on each inner
///   wing pylon, Jane's text in `MF1.INF`); four abreast made the inner tanks
///   cross each other and both main-gear plates. This is the one place the
///   function changes a count, and the default `F250.GAS` count follows it, so
///   the default load is two. The row keeps its place, mask, class and store;
///   its drawn position moves in `hardpoints::station_move`.
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
/// - **B-52** (`B52.PT` row 4, the tail gun; BMB-A2, John's decision 13): the
///   row's round count is 1,242 in place of 2,000, the real H magazine
///   ([`B52_GUN_ROUNDS`]). The gun keeps its row, mask, slew, weight class.
///   Update (B52-5, 2026-11-15): its mount moves to the drawn tail cone
///   ([`B52_GUN_MOUNT`]), the three bay rows are capped to the real bay and
///   start empty ([`B52_BAY_ROWS`]), and one row is appended, row 7, the
///   underwing pylon pair holding the default Harpoons ([`B52_PYLONS`]): the
///   only place this function adds a row, guarded on the whole retail table
///   like the rest (so the table has seven rows only while it is retail).
/// - **B-2** (`B2.PT` rows 5 to 8, the four bay rows; BMB-A2, John's decision
///   16 and the coordinator's call on the BMB-A1 finding): the designator bit
///   `0x400` of the flags (`0x1504`) is cleared (`0x1104`), so no targeting or
///   laser pod and no multi-store pod is offered on a bomb bay (B7: the real
///   jet has no pod station). **Fitted** (agent decision 2026-11-15). The
///   default stores, counts, classes and positions stay; every other bit stays.
///   Update (B2-5, 2026-11-15, John's decision 21, as the F-22N's bay rows
///   were corrected): the pair flag `0x4` is cleared too (`0x1100`), so a row
///   is one bay, not a pair that mirrors to the other bay. The counts (32 Mk 82
///   and 4 Paveway III a bay, the record's 64 and 8) and the weight classes
///   stay: the real 80 Mk 82 does not fit the bay (docs/formats/aircraft-variety.md,
///   "B-2 weapon bays").
/// - **F-117** (`F117.PT` row 2): no default `F18.ECM`. The real F-117
///   carried no jammer (docs/formats/aircraft-variety.md, "F-117 sensors,
///   jammer and pods"), and the record's row also carried 30 chaff and 30
///   flares, which go with it (no source found for a chaff or flare dispenser
///   on the real jet, and none is added). The row keeps its place, mask, count
///   and position, the way the F-5E's did in B5-2.
/// - **B-1B** (`B1.PT` rows 5 to 7, the three bay rows; BMB-B2, coordinator
///   first pass 2026-11-15): the designator bit `0x400` is cleared (`0x1500`
///   to `0x1100`), as for the B-2: no pod on a bomb bay. **Fitted** (agent
///   decision). Nothing else of a row changes.
/// - **F-111F** (`F111.PT` rows 5 to 8, the four wing pairs; BMB-B2,
///   coordinator first pass 2026-11-15): the designator bit is cleared
///   (`0x07e5` to `0x03e5`; the real Pave Tack rides in the bay, not on a
///   wing pylon, docs/formats/aircraft-variety.md "F-111F sensors"), and each
///   weight class is capped at 50 (5,000 lb a store, the real pylon rating,
///   Wikipedia "General Dynamics F-111 Aardvark": "each pylon had a capacity
///   of 5,000 lb"; the record's 70 and 60 are 7,000 and 6,000 lb). **Fitted**
///   (agent decision). Defaults, counts, masks and positions stay.
/// - **Tu-160** (`TU160.PT` rows 4 and 5, the two bay rows; T160-5, 2026-11-16,
///   the coordinator's first pass): the pair flag `0x4` is cleared (`0x1184` to
///   `0x1180`), so a row is one bay of six `AS16.JT` (the record's 12 in all,
///   the retail default) and not a pair that mirrors onto the other bay. The
///   designator bit was never set and the active-radar bit stays clear: the
///   Kh-55 (`AS15.JT`) stays refused. Counts, classes, masks and the
///   record's point stay (the placement code puts the stores,
///   `tore_sim::combat::bays`). **Fitted** (agent decision).
/// - **Tu-26** (`TU26.PT` row 5, the bay row; BMB-C2, coordinator first pass
///   2026-11-15): the designator bit `0x400` is cleared and the active-radar
///   bit `0x20` set (`0x1500` to `0x1120`): no pod on a bomb bay (B7), and the
///   bay takes the Kh-15 (`AS16.JT`, the real six-round rotary launcher,
///   `TU26.INF`) as the B-52's bay rows do. The same bit admits the other
///   active-radar missiles (`AGM84A.JT`, `AM39.JT`), as on the B-52. **Fitted**
///   (agent decision). Count, class, position and the empty default stay.
///   The two wing glove rows (rows 7 and 8, `AS16.JT` pairs of four and six,
///   twelve in all with the intake pair) are cut to two each
///   ([`TU26_GLOVE_CAPACITY`]), so the wings carry the real four Kh-15
///   ("four more underwing", `TU26.INF`; Wikipedia "Tupolev Tu-22M": six
///   internal and four underwing) and the default external load is six with
///   the intake-trunk pair (row 6, kept). This is the second place the
///   function changes a count (the Mirage F1's belly was the first): a default
///   store's capacity is its row's count, so a default cannot be capped
///   alone, and the glove rows' bomb capacity falls with it. Classes 108 and
///   160, masks and positions stay.
/// - **Tu-95** (`TU95.PT`; BMB-C2, coordinator first pass 2026-11-15): rows 5
///   and 6, the Bear-A to F dorsal and ventral turrets (`T23_1.JT`, the real
///   Bear-H has the tail turret alone, Wikipedia "Tupolev Tu-95"), hold
///   nothing; row 3, the tail gun, stays (2,000 rounds, the review found no
///   real magazine). Row 8, the wing pylons: x +-12 ft and 2 ft below the
///   reference point ([`TU95_WING_POSITION`]; the retail 9 ft out and 11.7 ft
///   down was inside the fuselage and under the gear's ground plane), the mask
///   `0x17e5` to `0x1025` ([`TU95_WING_FLAGS`]: no tank, pod, bomb or
///   air-to-air bit, B7), ten stores ([`TU95_WING_COUNT`]) with no weight
///   class (the class byte holds 255 at most, nine AS-16, and ten need 264),
///   and [`weapon_allowed`] leaves the AS-16 alone on it. **Opinionated**
///   (the Tu-95MS16's ten pylon missiles are cited, the arrangement is not),
///   positions fitted.
/// - **Su-34** (`SU34.PT`; BMB-C2, coordinator first pass 2026-11-15): the gun
///   row 5 holds 180 rounds in place of 150 ([`SU34_GUN_ROUNDS`], the real
///   GSh-30-1 magazine), and the fuselage row 6 moves up from -10 ft to -6 ft
///   ([`SU34_FUSELAGE_UP`]: the retail point is 1.7 ft under the main wheels'
///   bottoms). **Fitted** (agent decision).
pub fn adjust_defaults(id: AircraftId, rows: &mut Vec<Hardpoint>) {
    if !is_retail(id, rows) {
        return;
    }
    match id {
        AircraftId::F5E => {
            rows[F5E_RADAR].store = Some(F5E_RADAR_RECORD.into());
            rows[F5E_JAMMER].store = Some(F5E_ECM_RECORD.into());
            rows[F5E_WING_PAIR].store = None;
        }
        AircraftId::Mf1 => {
            rows[MF1_CENTRELINE].store = None;
            rows[MF1_BELLY].count = MF1_BELLY_CAPACITY;
        }
        AircraftId::B52 => {
            rows[B52_GUN].count = B52_GUN_ROUNDS;
            rows[B52_GUN].position = B52_GUN_MOUNT;
            for row in B52_BAY_ROWS {
                rows[row].count = B52_BAY_ITEMS;
                rows[row].weight_class = B52_BAY_CLASS;
                rows[row].store = None;
            }
            rows.push(Hardpoint {
                location: 4,
                flags: B52_PYLON_FLAGS,
                position: B52_PYLON_MOUNT,
                count: B52_PYLON_ITEMS,
                weight_class: B52_PYLON_CLASS,
                store: Some("AGM84E.JT".into()),
                slew: [0; 4],
            });
        }
        AircraftId::B2 => {
            for row in &mut rows[B2_BAY_ROWS] {
                row.flags &= !(DESIGNATOR_BIT | PAIRED_BIT);
            }
        }
        AircraftId::F117 => rows[F117_JAMMER].store = None,
        AircraftId::B1b => {
            for row in &mut rows[B1B_BAY_ROWS] {
                row.flags &= !DESIGNATOR_BIT;
            }
        }
        AircraftId::Tu160 => {
            for row in &mut rows[TU160_BAY_ROWS] {
                row.flags &= !PAIRED_BIT;
            }
        }
        AircraftId::F111 => {
            for row in &mut rows[F111_WING_ROWS] {
                row.flags &= !DESIGNATOR_BIT;
                row.weight_class = row.weight_class.min(F111_WING_CLASS);
            }
        }
        AircraftId::Tu26 => {
            let bay = &mut rows[TU26_BAY];
            bay.flags = (bay.flags & !DESIGNATOR_BIT) | ACTIVE_RADAR_BIT;
            for row in &mut rows[TU26_GLOVE_ROWS] {
                row.count = row.count.min(TU26_GLOVE_CAPACITY);
            }
        }
        AircraftId::Tu95 => {
            for row in &mut rows[TU95_EXTRA_GUNS] {
                row.store = None;
            }
            let pylons = &mut rows[TU95_WING_PYLONS];
            pylons.position = TU95_WING_POSITION;
            pylons.flags = TU95_WING_FLAGS;
            pylons.count = TU95_WING_COUNT;
            pylons.weight_class = 0;
        }
        AircraftId::Su34 => {
            rows[SU34_GUN].count = SU34_GUN_ROUNDS;
            rows[SU34_FUSELAGE].position[1] = SU34_FUSELAGE_UP;
        }
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
    if id == AircraftId::B2 {
        // The B-2's two Paveway III rows start empty (B2-5, John's decision
        // 21): a bay holds the Mk 82 or the Paveway III, not both, and the
        // Mk 82 rows are the default. The rows keep their store (the mask
        // admits the GBU-27 only as the row's own default), capacity and
        // position, so Load Ordnance still offers the Paveway III.
        for row in B2_PAVEWAY_ROWS {
            if let Some(row) = rows.get_mut(row)
                && row.store.as_deref() == Some("PAVEWA3.JT")
                && row.location == 3
                && row.flags == 0x1100
            {
                row.count = 0;
            }
        }
        return;
    }
    // The B-52's pylon row (B52-5, John's decision 9) holds 12 Harpoons of
    // its 18: six a wing. The row is the one `adjust_defaults` appended.
    if id.source() == AircraftId::B52
        && let Some(row) = rows.get_mut(B52_PYLONS)
        && (
            row.location,
            row.flags,
            row.position,
            row.count,
            row.weight_class,
        ) == (
            4,
            B52_PYLON_FLAGS,
            B52_PYLON_MOUNT,
            B52_PYLON_ITEMS,
            B52_PYLON_CLASS,
        )
    {
        row.count = B52_PYLON_DEFAULT;
    }
    if id.source() != AircraftId::Tornado {
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
///
/// The F-117's two bay rows (`F117.PT` rows 4 and 5) also take the Paveway II
/// laser bombs, [`F117_EXTRA_BOMBS`]: the retail bomb-only mask `0x1100`
/// leaves them out, and a laser bomb is what the real jet carries.
pub fn extra_weapon(id: AircraftId, rows: usize, row: usize, source: &str) -> bool {
    (id == AircraftId::F16C && rows > F16_WET && row == F16_TIP && source == "AIM120.JT")
        || (id == AircraftId::F117
            && F117_BAY_ROWS.contains(&row)
            && F117_EXTRA_BOMBS.contains(&source))
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
        slew: [0; 4],
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
                    slew: [0; 4],
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
                slew: [0; 4],
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
                    slew: [0; 4],
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

    /// John, 2026-11-10 (F15E-5 round 2, opinionated): the F-15E's bomb pair
    /// refuses the large stores that enter the belly flank, its wing pair
    /// refuses the AA-6; nothing else of the aircraft or any other aircraft moves.
    #[test]
    fn the_f15e_refuses_the_large_stores_on_its_bomb_pair_and_the_aa6_on_its_wing_pair() {
        for name in F15E_BELLY_REFUSED {
            assert!(
                !weapon_allowed(AircraftId::F15E, F15E_BELLY, name),
                "{name}"
            );
            // Not on the wing pair or elsewhere, and not on another aircraft.
            assert!(weapon_allowed(AircraftId::F15E, F15E_WING, name), "{name}");
            assert!(weapon_allowed(AircraftId::F15E, 6, name), "{name}");
            assert!(weapon_allowed(AircraftId::F15, F15E_BELLY, name), "{name}");
            assert!(
                weapon_allowed(AircraftId::Mig23, F15E_BELLY, name),
                "{name}"
            );
        }
        for name in ["MK82.JT", "MK84.JT", "FAB1000.JT", "CBU89.JT", "LAU61.JT"] {
            assert!(weapon_allowed(AircraftId::F15E, F15E_BELLY, name), "{name}");
        }
        assert!(!weapon_allowed(AircraftId::F15E, F15E_WING, "AA6.JT"));
        assert!(weapon_allowed(AircraftId::F15E, F15E_WING, "AIM120.JT"));
        assert!(weapon_allowed(AircraftId::M2000, F15E_WING, "AA6.JT"));
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
            slew: [0; 4],
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
            store: (!store.is_empty()).then(|| store.into()),
            count,
            weight_class,
            slew: [0; 4],
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

    /// TOR-13 (John, 2026-11-12): the Clean version is the Tornado record with
    /// no jammer row; the Combat version and every other identity keep it.
    #[test]
    fn the_clean_tornado_loses_only_the_jammer_row_and_the_pods_are_the_combats() {
        let retail = tornado_retail();
        let mut combat = retail.clone();
        variant(AircraftId::Tornado, &mut combat);
        assert_eq!(key(&combat), key(&retail));
        let mut clean = retail.clone();
        variant(AircraftId::TornadoClean, &mut clean);
        assert_eq!(clean[TORNADO_JAMMER].store, None);
        let mut kept = key(&clean);
        kept[TORNADO_JAMMER].3 = retail[TORNADO_JAMMER].store.clone();
        assert_eq!(kept, key(&retail), "only the jammer row's store goes");
        // Repeated application is a no-op, an edited row keeps its own record.
        let once = key(&clean);
        variant(AircraftId::TornadoClean, &mut clean);
        assert_eq!(key(&clean), once);
        let mut edited = retail.clone();
        edited[TORNADO_JAMMER].store = Some("F18.ECM".into());
        variant(AircraftId::TornadoClean, &mut edited);
        assert_eq!(edited[TORNADO_JAMMER].store.as_deref(), Some("F18.ECM"));
        // Both versions share the Tornado's station refusals and defaults.
        for name in TORNADO_SHOULDER_REFUSED {
            assert!(!weapon_allowed(
                AircraftId::TornadoClean,
                TORNADO_SHOULDER,
                name
            ));
        }
        let mut counts = retail.clone();
        default_counts(AircraftId::TornadoClean, &mut counts);
        assert_eq!(counts[TORNADO_CENTRELINE].count, 1);
        assert_eq!(retail[TORNADO_JAMMER].location, 1);
    }

    #[test]
    fn only_the_combat_tornado_carries_the_authored_pods_with_real_sizes_and_masses() {
        let pods = authored_pods(AircraftId::Tornado);
        assert_eq!(pods, [BOZ_107, SKY_SHADOW]);
        assert!(authored_pods(AircraftId::TornadoClean).is_empty());
        assert!(authored_pods(AircraftId::Mig23).is_empty());
        // Stations past the nine source rows: no Load Ordnance row.
        assert!(pods.iter().all(|pod| pod.row >= 9));
        assert!(pods[0].right && !pods[1].right);
        assert!(pods[0].dispenser && !pods[1].dispenser);
        // 3.0 m by 0.42 m, 200 kg, and 4.0 m by 0.48 m, 325 kg.
        assert!((SKY_SHADOW.length - 9.843).abs() < 1e-3);
        assert!((BOZ_107.length - 13.123).abs() < 1e-3);
        assert!((SKY_SHADOW.diameter - 1.378).abs() < 1e-3);
        assert!((BOZ_107.diameter - 1.575).abs() < 1e-3);
        assert_eq!(
            f64::from(SKY_SHADOW.weight_lbs).round(),
            (200. * 2.20462_f64).round()
        );
        assert_eq!(
            f64::from(BOZ_107.weight_lbs).round(),
            (325. * 2.20462_f64).round()
        );
        assert_eq!(authored_pod("boz107"), Some(BOZ_107));
        assert_eq!(authored_pod("PAVESPK.SEE"), None);
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
    fn the_mirage_f1_centreline_keeps_its_station_but_defaults_to_no_pod_and_the_belly_holds_two() {
        let retail = default_rows(AircraftId::Mf1);
        assert_eq!(retail.len(), 8);
        assert_eq!(retail[MF1_CENTRELINE].store.as_deref(), Some("ALQ167.ECM"));
        let mut rows = retail.clone();
        adjust_defaults(AircraftId::Mf1, &mut rows);
        // The centreline row changes only its default store (same index, mask,
        // count, weight class and position, so it accepts what it did); the
        // belly row changes only its capacity, four to two (John, 2026-11-11,
        // round 3, M1): same index, mask, class, position and store.
        let mut expected = key(&retail);
        expected[MF1_CENTRELINE].3 = None;
        expected[MF1_BELLY].4 = 2;
        assert_eq!(retail[MF1_BELLY].count, 4);
        assert_eq!(key(&rows), expected);
        assert_eq!(rows[MF1_CENTRELINE].flags, 0x7e5);
        assert_eq!(rows[MF1_BELLY].count, MF1_BELLY_CAPACITY);
        // The jammer suite (row 2), the belly tanks and the Magics stay.
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

    /// Agent decision, 2026-11-11 (MF1-6, following John's F-15E A2 0.3 ft
    /// line): the Mirage F1's belly pair refuses the deep entries, its wing pair
    /// the one alternative over the line (`AS30.JT`); nothing else moves.
    #[test]
    fn the_mirage_f1_refuses_the_stores_that_enter_its_belly_pair_and_wing_pair() {
        for name in MF1_BELLY_REFUSED {
            assert!(!weapon_allowed(AircraftId::Mf1, MF1_BELLY, name), "{name}");
            assert!(weapon_allowed(AircraftId::Mf1, 7, name), "{name}");
            assert!(
                weapon_allowed(AircraftId::Mf1, MF1_CENTRELINE, name),
                "{name}"
            );
            assert!(
                weapon_allowed(AircraftId::M2000, M2000_BELLY, name),
                "{name}"
            );
        }
        for name in MF1_WING_REFUSED {
            assert!(!weapon_allowed(AircraftId::Mf1, MF1_WING, name), "{name}");
            assert!(weapon_allowed(AircraftId::Mf1, 7, name), "{name}");
            assert!(
                weapon_allowed(AircraftId::Mf1, MF1_BELLY + 2, name),
                "{name}"
            );
            assert!(weapon_allowed(AircraftId::M2000, MF1_WING, name), "{name}");
        }
        // Never a default store: the Magic stays on the wing pair and the tip
        // pair, and the kept shallow entries stay on the belly pair.
        assert!(weapon_allowed(AircraftId::Mf1, MF1_WING, "R550.JT"));
        assert!(weapon_allowed(AircraftId::Mf1, 7, "R550.JT"));
        assert!(!weapon_allowed(AircraftId::Mf1, MF1_BELLY, "R550.JT"));
        for name in [
            "AA11.JT",
            "AA12.JT",
            "AIM9X.JT",
            "AIM120.JT",
            "AGM65G.JT",
            "MK82.JT",
        ] {
            assert!(weapon_allowed(AircraftId::Mf1, MF1_BELLY, name), "{name}");
        }
        // The six shallow wing entries stay (0.06 to 0.15 ft).
        for name in [
            "PL7.JT", "PL10.JT", "AGM88.JT", "AGM45.JT", "AIM7.JT", "AIM7E.JT",
        ] {
            assert!(weapon_allowed(AircraftId::Mf1, MF1_WING, name), "{name}");
        }
        assert_eq!(MF1_BELLY_REFUSED.len() + MF1_WING_REFUSED.len(), 34);
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
    #[test]
    fn the_bombers_get_the_real_gun_rounds_a_bay_without_pod_flags_and_no_jammer() {
        // BMB-A2 (John, 2026-11-15; the B-2 flag is a fitted agent call).
        // B-52: 1,242 rounds on the tail gun row, nothing else moves.
        let retail = default_rows(AircraftId::B52);
        assert_eq!(retail.len(), 7);
        assert_eq!(retail[5].store, None);
        let mut rows = retail.clone();
        adjust_defaults(AircraftId::B52, &mut rows);
        let mut expected = key(&retail);
        expected[B52_GUN].4 = 1_242;
        // B52-5 (2026-11-15): the gun moves to the tail cone, the three bay
        // rows are capped and start empty, and row 7 is the pylon pair with
        // the Harpoons.
        expected[B52_GUN].2 = [0, 6, -276];
        for row in B52_BAY_ROWS {
            expected[row].3 = None;
            expected[row].4 = 9;
            expected[row].5 = 70;
        }
        expected.push((4, 0x1105, [52, 8, 31], Some("AGM84E.JT".into()), 18, 170));
        assert_eq!(key(&rows), expected);
        assert_eq!(rows[B52_GUN].store.as_deref(), Some("M61.JT"));
        adjust_defaults(AircraftId::B52, &mut rows);
        assert_eq!(key(&rows), expected);
        // The default load is 12 of the pylon row's 18, six a wing, and the
        // bay holds nothing.
        let mut load = rows.clone();
        default_counts(AircraftId::B52, &mut load);
        assert_eq!((load[B52_PYLONS].count, rows[B52_PYLONS].count), (12, 18));
        assert!(B52_BAY_ROWS.iter().all(|r| load[*r].store.is_none()));
        default_counts(AircraftId::B52, &mut load);
        assert_eq!(load[B52_PYLONS].count, 12);
        // A record with a changed row keeps all of its own rows, no pylon.
        let mut edited = retail.clone();
        edited[3].weight_class += 1;
        adjust_defaults(AircraftId::B52, &mut edited);
        assert_eq!(edited.len(), 7);
        // B-2: the four bay rows lose the designator bit and the pair flag
        // (0x1504 to 0x1100); the sensor rows and every other field stay.
        let retail = default_rows(AircraftId::B2);
        let mut rows = retail.clone();
        adjust_defaults(AircraftId::B2, &mut rows);
        let mut expected = key(&retail);
        for row in &mut expected[5..9] {
            row.1 = 0x1100;
        }
        assert_eq!(key(&rows), expected);
        assert!(rows[5..9].iter().all(|h| h.flags & DESIGNATOR_BIT == 0));
        assert!(rows[..5].iter().all(|h| h.flags == 0x1008));
        // B2-5: the Paveway III rows start empty (a bay holds the Mk 82 or the
        // Paveway III); the Mk 82 rows and every other field stay.
        let mut defaults = rows.clone();
        default_counts(AircraftId::B2, &mut defaults);
        let mut expected = key(&rows);
        expected[7].4 = 0;
        expected[8].4 = 0;
        assert_eq!(key(&defaults), expected);
        default_counts(AircraftId::B2, &mut defaults);
        assert_eq!(key(&defaults), expected);
        // F-117: the jammer row holds nothing, in its place with its mask.
        let retail = default_rows(AircraftId::F117);
        assert_eq!(retail[F117_JAMMER].store.as_deref(), Some("F18.ECM"));
        let mut rows = retail.clone();
        adjust_defaults(AircraftId::F117, &mut rows);
        let mut expected = key(&retail);
        expected[F117_JAMMER].3 = None;
        assert_eq!(key(&rows), expected);
        assert_eq!(
            (rows[F117_JAMMER].flags, rows[F117_JAMMER].position),
            (0x1008, [0, 0, -50])
        );
        // The same rows on another identity, or an edited record, stay retail.
        for (id, from) in [
            (AircraftId::F117, AircraftId::B2),
            (AircraftId::B52, AircraftId::F117),
        ] {
            let mut other = default_rows(from);
            let before = key(&other);
            adjust_defaults(id, &mut other);
            assert_eq!(key(&other), before);
        }
        let mut edited = default_rows(AircraftId::B2);
        edited[0].weight_class += 1;
        let before = key(&edited);
        adjust_defaults(AircraftId::B2, &mut edited);
        assert_eq!(key(&edited), before);
    }
    /// BMB-B2 (2026-11-15): the B-1B's three bay rows lose only the designator
    /// bit; the F-111F's four wing rows lose it and cap their weight class at
    /// 50 (5,000 lb); an edited record or another identity is left alone.
    #[test]
    fn the_b1b_bay_rows_and_the_f111_wing_rows_are_corrected_and_nothing_else_moves() {
        let retail = default_rows(AircraftId::B1b);
        assert_eq!(retail.len(), 8);
        assert!(retail[B1B_BAY_ROWS].iter().all(|h| h.flags == 0x1500));
        let mut rows = retail.clone();
        adjust_defaults(AircraftId::B1b, &mut rows);
        let mut expected = key(&retail);
        for row in &mut expected[5..8] {
            row.1 = 0x1100;
        }
        assert_eq!(key(&rows), expected);
        assert!(
            rows[B1B_BAY_ROWS]
                .iter()
                .all(|h| h.flags & DESIGNATOR_BIT == 0)
        );
        assert!(rows[..5].iter().all(|h| h.flags == 0x1008));
        // Repeated application is a no-op (the signature no longer matches).
        adjust_defaults(AircraftId::B1b, &mut rows);
        assert_eq!(key(&rows), expected);
        let retail = default_rows(AircraftId::F111);
        assert_eq!(retail.len(), 9);
        let classes: Vec<_> = retail[F111_WING_ROWS]
            .iter()
            .map(|h| h.weight_class)
            .collect();
        assert_eq!(classes, [70, 70, 70, 60]);
        let mut rows = retail.clone();
        adjust_defaults(AircraftId::F111, &mut rows);
        let mut expected = key(&retail);
        for row in &mut expected[5..9] {
            row.1 = 0x3e5;
            row.5 = 50;
        }
        assert_eq!(key(&rows), expected);
        assert!(
            rows[F111_WING_ROWS]
                .iter()
                .all(|h| h.flags & DESIGNATOR_BIT == 0)
        );
        assert!(rows[..5].iter().all(|h| h.flags == 0x8));
        let mut edited = default_rows(AircraftId::F111);
        edited[0].count += 1;
        let before = key(&edited);
        adjust_defaults(AircraftId::F111, &mut edited);
        assert_eq!(key(&edited), before);
        let mut other = default_rows(AircraftId::B2);
        let before = key(&other);
        adjust_defaults(AircraftId::F111, &mut other);
        adjust_defaults(AircraftId::B1b, &mut other);
        assert_eq!(key(&other), before);
    }
    /// T160-5 (2026-11-16): the Tu-160's two bay rows lose only the pair flag
    /// (`0x1184` to `0x1180`), so a row is one bay; counts, classes, masks and
    /// positions stay; the Kh-55 stays refused by the mask (no active-radar
    /// bit); an edited record or another identity is left alone.
    #[test]
    fn the_tu160_bay_rows_lose_only_the_pair_flag() {
        let retail = default_rows(AircraftId::Tu160);
        assert_eq!(retail.len(), 6);
        assert!(retail[TU160_BAY_ROWS].iter().all(|h| h.flags == 0x1184));
        let mut rows = retail.clone();
        adjust_defaults(AircraftId::Tu160, &mut rows);
        let mut expected = key(&retail);
        for row in &mut expected[4..6] {
            row.1 = 0x1180;
        }
        assert_eq!(key(&rows), expected);
        assert!(
            rows[TU160_BAY_ROWS]
                .iter()
                .all(|h| h.flags & PAIRED_BIT == 0
                    && h.flags & DESIGNATOR_BIT == 0
                    && h.flags & ACTIVE_RADAR_BIT == 0
                    && h.count == 6
                    && h.weight_class == 180
                    && h.store.as_deref() == Some("AS16.JT"))
        );
        assert!(rows[..4].iter().all(|h| h.flags == 0x1008));
        adjust_defaults(AircraftId::Tu160, &mut rows);
        assert_eq!(key(&rows), expected);
        let mut edited = default_rows(AircraftId::Tu160);
        edited[0].count += 1;
        let before = key(&edited);
        adjust_defaults(AircraftId::Tu160, &mut edited);
        assert_eq!(key(&edited), before);
        let mut other = default_rows(AircraftId::B1b);
        let before = key(&other);
        adjust_defaults(AircraftId::Tu160, &mut other);
        assert_eq!(key(&other), before);
        // The refusal list: nothing outside the table is refused by name, and
        // only the two bay rows read it.
        for source in TU160_BAY_REFUSED {
            assert!(!weapon_allowed(AircraftId::Tu160, 4, source));
            assert!(!weapon_allowed(AircraftId::Tu160, 5, source));
            assert!(weapon_allowed(AircraftId::Tu160, 3, source));
        }
        assert!(weapon_allowed(AircraftId::Tu160, 4, "AS16.JT"));
    }
    /// BMB-C2 (2026-11-15): the Tu-26's bay row loses the designator bit and
    /// gains the active-radar bit, its glove rows hold two AS-16 each; the Tu-95 keeps only its tail turret and
    /// gets the AS-16-only pylons; the Su-34's gun holds 180 rounds and its
    /// fuselage row hangs higher. Everything else of every row stays, and an
    /// edited record or another identity is left alone.
    #[test]
    fn the_batch_c_rows_are_corrected_and_nothing_else_moves() {
        // Tu-26.
        let retail = default_rows(AircraftId::Tu26);
        assert_eq!(retail.len(), 9);
        assert_eq!(retail[TU26_BAY].flags, 0x1500);
        let mut rows = retail.clone();
        adjust_defaults(AircraftId::Tu26, &mut rows);
        let mut expected = key(&retail);
        expected[TU26_BAY].1 = 0x1120;
        // The glove pairs carry one store a side (four AS-16 underwing); the
        // intake pair and every class stay.
        expected[7].4 = 2;
        expected[8].4 = 2;
        assert_eq!(key(&rows), expected);
        assert_eq!((rows[6].count, rows[7].count, rows[8].count), (2, 2, 2));
        assert_eq!(
            (
                rows[6].weight_class,
                rows[7].weight_class,
                rows[8].weight_class
            ),
            (54, 108, 160)
        );
        assert_eq!(rows[TU26_BAY].flags & DESIGNATOR_BIT, 0);
        assert_eq!(rows[TU26_BAY].flags & ACTIVE_RADAR_BIT, ACTIVE_RADAR_BIT);
        adjust_defaults(AircraftId::Tu26, &mut rows);
        assert_eq!(key(&rows), expected);
        // Tu-95.
        let retail = default_rows(AircraftId::Tu95);
        assert_eq!(retail.len(), 9);
        let mut rows = retail.clone();
        adjust_defaults(AircraftId::Tu95, &mut rows);
        let mut expected = key(&retail);
        expected[5].3 = None;
        expected[6].3 = None;
        expected[TU95_WING_PYLONS].1 = 0x1025;
        expected[TU95_WING_PYLONS].2 = [36, -6, 35];
        expected[TU95_WING_PYLONS].4 = 10;
        expected[TU95_WING_PYLONS].5 = 0;
        assert_eq!(key(&rows), expected);
        // The tail turret and the bay row are untouched.
        assert_eq!(rows[3].store.as_deref(), Some("T23_1.JT"));
        assert_eq!((rows[3].count, rows[3].position), (2_000, [0, 0, -100]));
        assert_eq!(rows[7].store.as_deref(), Some("AS16.JT"));
        // 12 ft out, above the gear's ground-contact plane (-28 units).
        assert_eq!(rows[TU95_WING_PYLONS].position[0], 36);
        assert!(rows[TU95_WING_PYLONS].position[1] > -28);
        adjust_defaults(AircraftId::Tu95, &mut rows);
        assert_eq!(key(&rows), expected);
        assert!(weapon_allowed(
            AircraftId::Tu95,
            TU95_WING_PYLONS,
            "AS16.JT"
        ));
        for source in ["AGM84A.JT", "AM39.JT", "AIM120.JT", "MK82.JT", "AS15.JT"] {
            assert!(!weapon_allowed(AircraftId::Tu95, TU95_WING_PYLONS, source));
            assert!(weapon_allowed(AircraftId::Tu95, 7, source), "{source}");
        }
        assert!(weapon_allowed(AircraftId::Tu26, 8, "AGM84A.JT"));
        // Su-34.
        let retail = default_rows(AircraftId::Su34);
        assert_eq!(retail.len(), 9);
        let mut rows = retail.clone();
        adjust_defaults(AircraftId::Su34, &mut rows);
        let mut expected = key(&retail);
        expected[SU34_GUN].4 = 180;
        expected[SU34_FUSELAGE].2 = [24, -18, -13];
        assert_eq!(key(&rows), expected);
        // The wheels' bottoms are 25 units down: the row is above them.
        assert!(rows[SU34_FUSELAGE].position[1] > -25);
        adjust_defaults(AircraftId::Su34, &mut rows);
        assert_eq!(key(&rows), expected);
        // An edited record or another identity stays as it was.
        for id in [AircraftId::Tu26, AircraftId::Tu95, AircraftId::Su34] {
            let mut edited = default_rows(id);
            edited[0].weight_class += 1;
            let before = key(&edited);
            adjust_defaults(id, &mut edited);
            assert_eq!(key(&edited), before, "{id:?}");
        }
        let mut other = default_rows(AircraftId::B52);
        let before = key(&other);
        for id in [AircraftId::Tu26, AircraftId::Tu95, AircraftId::Su34] {
            adjust_defaults(id, &mut other);
        }
        assert_eq!(key(&other), before);
    }
    /// F117-5 (2026-11-15): the F-117's two bay rows take the Paveway II laser
    /// bombs beyond the retail bomb-only mask, and nothing else of the row,
    /// the aircraft or any other aircraft changes.
    #[test]
    fn the_f117_bay_rows_take_the_paveway_ii_bombs_and_nothing_else_moves() {
        for name in F117_EXTRA_BOMBS {
            for row in F117_BAY_ROWS {
                assert!(extra_weapon(AircraftId::F117, 6, row, name), "{name}");
            }
            assert!(!extra_weapon(AircraftId::F117, 6, 3, name));
            assert!(!extra_weapon(AircraftId::F117, 6, 6, name));
            assert!(!extra_weapon(AircraftId::B2, 9, 4, name));
            assert!(!extra_weapon(AircraftId::F16C, 10, 4, name));
        }
        assert!(!extra_weapon(AircraftId::F117, 6, 4, "AGM88.JT"));
        assert!(extra_weapon(AircraftId::F16C, 10, F16_TIP, "AIM120.JT"));
        assert!(weapon_allowed(AircraftId::F117, 4, "MK84.JT"));
        assert!(!weapon_allowed(AircraftId::F117, 5, "GBU16.JT"));
        assert!(weapon_allowed(AircraftId::F117, 3, "GBU16.JT"));
    }
}
