//! BMB-C1 and BMB-C2: the bomber batch C identities read the retail records.
//! Pins the three records' decoded station tables (every row, in order, with
//! the BMB-C2 station corrections), the type sizes the parser accepts, the
//! corrected record values and the shape layouts the interim rigs expect. It
//! skips quietly without a retail install and reads data only; no retail
//! bytes are committed. Evidence: docs/formats/aircraft-variety.md, the three
//! batch C source reviews and "Bomber batch C identities (BMB-C1,
//! 2026-11-15)".
use std::path::PathBuf;
use tore_formats::{
    aircraft::{Aircraft, AircraftId, stations},
    shape::Shape,
};
use tore_import::MediaSource;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

fn fa2() -> Option<tore_formats::Archive> {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return None;
    };
    let archive = media.archive("FA_2.LIB").ok();
    if archive.is_none() {
        eprintln!("skipped: no retail FA_2.LIB");
    }
    archive
}

fn rows(aircraft: &Aircraft) -> Vec<(String, i32)> {
    aircraft
        .hardpoints
        .iter()
        .map(|h| (h.store.clone().unwrap_or_default(), h.count))
        .collect()
}

/// Each record parses as its own identity (the `typeSize` 660 all three
/// share) and its decoded rows are the retail ones, in order, with the
/// BMB-C2 counts (the Tu-26's glove pairs two, the Tu-95's pylon row ten, the
/// Su-34's gun 180).
#[test]
fn the_three_records_parse_as_their_own_identities_with_their_station_tables() {
    let Some(fa2) = fa2() else { return };
    let expect: [(AircraftId, &[(&str, i32)]); 3] = [
        (
            AircraftId::Tu26,
            &[
                ("VIS300.SEE", 1),
                ("TU26R.SEE", 1),
                ("TU26.ECM", 1),
                ("T23_1.JT", 2_000),
                ("SU37L.SEE", 1),
                ("", 8),
                ("AS16.JT", 2),
                // The glove pairs hold one a side (BMB-C2): four underwing.
                ("AS16.JT", 2),
                ("AS16.JT", 2),
            ],
        ),
        (
            AircraftId::Tu95,
            &[
                ("VIS300.SEE", 1),
                ("TU95R.SEE", 1),
                ("TU95.ECM", 1),
                ("T23_1.JT", 2_000),
                ("SU37L.SEE", 1),
                // The Bear-A to F turrets: nothing by default (BMB-C2).
                ("", 2_000),
                ("", 2_000),
                ("AS16.JT", 6),
                ("", 10),
            ],
        ),
        (
            AircraftId::Su34,
            &[
                ("VIS300.SEE", 1),
                ("SU27R.SEE", 1),
                ("SU27I.SEE", 1),
                ("SU27.ECM", 1),
                ("SU37L.SEE", 1),
                ("GSH301.JT", 180),
                ("AS14.JT", 4),
                ("AT12.JT", 4),
                ("AA11.JT", 2),
            ],
        ),
    ];
    for (id, table) in expect {
        let aircraft = Aircraft::parse(&fa2.read(id.pt()).unwrap())
            .unwrap_or_else(|error| panic!("{}: {error}", id.pt()));
        assert_eq!(aircraft.id, id);
        let found = rows(&aircraft);
        let found: Vec<(&str, i32)> = found.iter().map(|(s, n)| (s.as_str(), *n)).collect();
        assert_eq!(found, table, "{} station table", id.pt());
    }
}

/// The BMB-C2 corrections reach the decoded records: weights, fuel, thrust,
/// ceiling, the top-speed edges and the station flags, positions and counts.
#[test]
fn the_corrected_records_carry_the_real_figures_and_station_flags() {
    let Some(fa2) = fa2() else { return };
    let read = |id: AircraftId| Aircraft::parse(&fa2.read(id.pt()).unwrap()).unwrap();
    let n = |a: &Aircraft, key: &str| a.number(key) as i64;
    let top = |a: &Aircraft, g: i32| {
        a.envelopes
            .iter()
            .find(|e| e.g == g)
            .unwrap()
            .points
            .iter()
            .map(|p| p[0])
            .fold(0., f64::max)
    };
    let ceiling = |a: &Aircraft, g: i32| {
        a.envelopes
            .iter()
            .find(|e| e.g == g)
            .unwrap()
            .points
            .iter()
            .map(|p| p[1])
            .fold(0., f64::max)
    };

    let tu26 = read(AircraftId::Tu26);
    assert_eq!(n(&tu26, "weight"), 127_868);
    assert_eq!(
        (n(&tu26, "thrust"), n(&tu26, "aftThrust")),
        (66_000, 110_000)
    );
    assert_eq!(n(&tu26, "maxAlt"), 43_635);
    // The kept record values: the game's own Jane's maximum and fuel and the
    // NK-25's flows.
    assert_eq!(
        (n(&tu26, "maxTakeoffWeight"), n(&tu26, "internalFuel")),
        (273_370, 110_230)
    );
    assert_eq!(
        (n(&tu26, "fuelConsumption"), n(&tu26, "aftFuelConsumption")),
        (10, 66)
    );
    // Every polygon is cut at the ceiling (the 0 G one 5,500 ft above it) and
    // the 1 G edge is 1,080 kt (1,823 ft/s) at 36,000 ft.
    for e in &tu26.envelopes {
        let cap = if e.g == 0 { 49_135. } else { 43_635. };
        assert!(
            e.points.iter().all(|p| p[1] <= cap),
            "Tu-26 {} G rows run past {cap} ft",
            e.g
        );
    }
    assert_eq!(ceiling(&tu26, 1), 43_635.);
    assert_eq!(top(&tu26, 1), 1_823.);
    assert_eq!(top(&tu26, 0), 1_974.);
    // The bay row: no designator bit, the active-radar bit.
    assert_eq!(tu26.hardpoints[stations::TU26_BAY].flags, 0x1120);
    assert!(tu26.hardpoints[6..].iter().all(|h| h.flags == 0x1185));
    assert_eq!(
        tu26.hardpoints[stations::TU26_GLOVE_ROWS]
            .iter()
            .map(|h| (h.count, h.weight_class))
            .collect::<Vec<_>>(),
        [(2, 108), (2, 160)]
    );

    let tu95 = read(AircraftId::Tu95);
    assert_eq!(n(&tu95, "thrust"), 129_000);
    assert_eq!(
        (n(&tu95, "weight"), n(&tu95, "internalFuel")),
        (198_415, 148_150)
    );
    assert_eq!(n(&tu95, "maxTakeoffWeight"), 414_470);
    assert_eq!(
        (n(&tu95, "maxAlt"), n(&tu95, "fuelConsumption")),
        (40_750, 4)
    );
    let pylons = &tu95.hardpoints[stations::TU95_WING_PYLONS];
    assert_eq!(pylons.position, [36, -6, 27]);
    assert_eq!(
        (pylons.flags, pylons.count, pylons.weight_class),
        (0x1025, 10, 0)
    );
    assert!(tu95.hardpoints[5..=6].iter().all(|h| h.store.is_none()));
    assert_eq!(tu95.hardpoints[3].store.as_deref(), Some("T23_1.JT"));

    let su34 = read(AircraftId::Su34);
    assert_eq!(
        (n(&su34, "weight"), n(&su34, "internalFuel")),
        (49_604, 26_676)
    );
    assert_eq!(n(&su34, "thrust"), 33_800);
    // Kept: afterburner, maximum, ceiling, G and the name.
    assert_eq!(
        (n(&su34, "aftThrust"), n(&su34, "maxTakeoffWeight")),
        (59_510, 97_000)
    );
    assert_eq!((n(&su34, "maxAlt"), n(&su34, "envMax")), (55_000, 9));
    // Mach 1.8 above the tropopause: 1,742 ft/s on the 1 G row.
    assert_eq!(top(&su34, 1), 1_742.);
    assert_eq!(
        su34.hardpoints[stations::SU34_FUSELAGE].position,
        [24, -18, -13]
    );
    // Empty plus full fuel fits under the maximum on all three: none needs the
    // loadout's fuel cap.
    for a in [&tu26, &tu95, &su34] {
        assert!(n(a, "weight") + n(a, "internalFuel") < n(a, "maxTakeoffWeight"));
    }
}

/// The shapes load with the layouts the interim rigs pin (CODE size is the rig
/// loader's; faces and words here).
#[test]
fn the_batch_c_shapes_load_with_their_reviewed_layouts() {
    let Some(fa2) = fa2() else { return };
    for (name, code, faces, words, gear, plates) in [
        (
            "TU26.SH",
            26_812,
            310,
            vec![0x7880, 0x788c, 0x7892, 0x789e, 0x78a4, 0x78aa],
            0x7892,
            12,
        ),
        ("TU95.SH", 9_148, 155, vec![0x33b0], 0x33b0, 6),
        (
            "SU34.SH",
            22_506,
            321,
            vec![0x67c0, 0x67c6, 0x67cc, 0x67d8, 0x67de],
            0x67cc,
            8,
        ),
    ] {
        let bytes = fa2.read(name).unwrap();
        assert_eq!(
            tore_formats::module::code(&bytes).unwrap().0.len(),
            code,
            "{name} CODE"
        );
        let shape = Shape::parse(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(shape.faces.len(), faces, "{name}");
        assert_eq!(
            shape.state_words.iter().copied().collect::<Vec<_>>(),
            words,
            "{name}"
        );
        // The gear word adds the reviews' plates.
        assert_eq!(
            Shape::with_state(&bytes, &[(gear, 1)].into())
                .unwrap()
                .faces
                .len(),
            faces + plates,
            "{name} gear"
        );
    }
}
