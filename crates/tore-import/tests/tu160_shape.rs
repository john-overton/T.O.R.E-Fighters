//! T160-2: the Tu-160 draws `TU160.SH` from `FA_2.LIB` at the shape's own header
//! scale (exponent 11, so object scale 8 and 8/3 ft per unit: true size within
//! 1 percent on span and length, height gear up 5 percent high and gear down 18
//! percent high, no per-axis factor, no gear change). This reads the retail
//! shape and its damage shapes and pins the measurements the slice's notes
//! quote. It skips quietly without a retail install and reads data only; no
//! retail bytes are committed. Evidence: docs/formats/aircraft-variety.md,
//! "Tu-160 shape and rig (T160-2/3, 2026-11-15)".
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{
    module,
    shape::{Face, Shape, object_scale},
};
use tore_import::MediaSource;

/// The gear word that puts the nose leg and the two main legs down (`8002`).
const GEAR: usize = 0x8002;
/// Real Tu-160 span (spread, 20 degrees), length and height in feet (55.70 m,
/// 54.10 m, 13.10 m; `TU160.INF`, the T160-0 review cites them).
const REAL_FEET: [f64; 3] = [182.75, 177.5, 43.0];
const FEET_PER_UNIT: f64 = 8. / 3.;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

fn read(name: &str) -> Option<Vec<u8>> {
    let Ok(media) = MediaSource::detect(&install()) else {
        eprintln!("skipped: no retail install at {}", install().display());
        return None;
    };
    let Ok(fa2) = media.archive("FA_2.LIB") else {
        eprintln!("skipped: no retail FA_2.LIB");
        return None;
    };
    Some(fa2.read(name).unwrap())
}

fn bounds(shape: &Shape) -> ([f64; 3], [f64; 3]) {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for p in shape.faces.iter().flat_map(|f| &f.positions) {
        for i in 0..3 {
            low[i] = low[i].min(f64::from(p[i]));
            high[i] = high[i].max(f64::from(p[i]));
        }
    }
    (low, high)
}

fn twin_key(f: &Face) -> Vec<[u32; 3]> {
    let mut p: Vec<[u32; 3]> = f.positions.iter().map(|p| p.map(f32::to_bits)).collect();
    p.sort();
    p
}

/// `TU160.SH` has header exponent 11 (object scale 8) and, drawn at the 8/3 ft a
/// unit that gives, matches the real span and length within 1 percent. The
/// height is 5 percent high gear up (the nacelles at z -3) and 18 percent
/// high gear down (the gear plates, 5 units long). The gear word adds the nose
/// leg and the two main legs and nothing else.
#[test]
fn tu160_sh_is_true_size_at_its_header_scale() {
    let Some(bytes) = read("TU160.SH") else {
        return;
    };
    assert_eq!(module::code(&bytes).unwrap().0.len(), 28_716);
    assert_eq!(object_scale(&bytes).unwrap(), 8.);
    let neutral = Shape::parse(&bytes).unwrap();
    assert_eq!(neutral.faces.len(), 316);
    let down = Shape::with_state(&bytes, &BTreeMap::from([(GEAR, 1)])).unwrap();
    assert_eq!(down.faces.len(), 316 + 12);
    let (clean_low, clean_high) = bounds(&neutral);
    let (low, high) = bounds(&down);
    eprintln!("neutral: {clean_low:?} {clean_high:?}; gear down: {low:?} {high:?}");
    // Units: span x -34..34, length y -29..38, belly z 0, nacelles z -3, fin
    // tip z 14, gear contact z -5.
    assert_eq!(
        ([clean_low[0], clean_high[0]], [clean_low[1], clean_high[1]]),
        ([-34., 34.], [-29., 38.])
    );
    assert_eq!((clean_low[2], clean_high[2]), (-3., 14.));
    assert_eq!(low[2], -5.);
    let feet = |lo: f64, hi: f64| (hi - lo) * FEET_PER_UNIT;
    let span = feet(clean_low[0], clean_high[0]);
    let length = feet(clean_low[1], clean_high[1]);
    let height_up = feet(clean_low[2], clean_high[2]);
    let height_down = feet(low[2], high[2]);
    eprintln!("span {span} length {length} height up {height_up} down {height_down}");
    for (got, real, axis, tolerance) in [
        (span, REAL_FEET[0], "span", 0.01),
        (length, REAL_FEET[1], "length", 0.01),
        (height_up, REAL_FEET[2], "height gear up", 0.06),
        (height_down, REAL_FEET[2], "height gear down", 0.19),
    ] {
        assert!(
            (got - real).abs() / real < tolerance,
            "{axis}: {got} ft against {real}"
        );
    }
    // The gear word's own faces: the nose leg (4 plates at y 20 to 22) and two
    // main legs of 4 plates (x +/-1 and the cross plates at y -1), all at z -5
    // at the bottom: 12 plates, the wheelbase 22 units (58.7 ft).
    let known: std::collections::BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
    let gear: Vec<&Face> = down
        .faces
        .iter()
        .filter(|f| !known.contains(&f.address))
        .collect();
    assert_eq!(gear.len(), 12);
    let lowest = |pick: &dyn Fn(&Face) -> bool| {
        gear.iter()
            .filter(|f| pick(f))
            .flat_map(|f| f.positions.iter().map(|p| f64::from(p[2])))
            .fold(f64::INFINITY, f64::min)
    };
    let nose = |f: &Face| f.positions.iter().all(|p| p[1] >= 20.);
    assert_eq!(gear.iter().filter(|f| nose(f)).count(), 4);
    assert_eq!(lowest(&nose), -5.);
    assert_eq!(lowest(&|f| !nose(f)), -5.);
}

/// The state words add and remove exactly the faces the rig reads: flames +16,
/// keel plates +4, each flap word -1 drooped (+4 -2) and 1 deleted (-2), the
/// rudder-strip word +/-1 (+2 -2) and 2 deleted (-2).
#[test]
fn the_state_words_add_what_the_rig_reads() {
    let Some(bytes) = read("TU160.SH") else {
        return;
    };
    let count = |word: usize, value: i32| {
        Shape::with_state(&bytes, &BTreeMap::from([(word, value)]))
            .unwrap()
            .faces
            .len()
    };
    assert_eq!(count(0x7ff0, 1), 316 + 16);
    assert_eq!(count(0x7ffc, 1), 316 + 4);
    for word in [0x800e, 0x8014] {
        assert_eq!(count(word, -1), 316 + 4 - 2);
        assert_eq!(count(word, 1), 316 - 2);
    }
    assert_eq!(count(0x801a, 1), 316);
    assert_eq!(count(0x801a, -1), 316);
    assert_eq!(count(0x801a, 2), 316 - 2);
}

/// Skin review: the exact-vertex twin groups are double-sided panels with
/// opposite stored normals, so the smooth path pairs them and hides the rear
/// member.
#[test]
fn twin_pairs_have_opposite_normals() {
    let Some(bytes) = read("TU160.SH") else {
        return;
    };
    let shape = Shape::parse(&bytes).unwrap();
    let mut groups: BTreeMap<Vec<[u32; 3]>, Vec<usize>> = BTreeMap::new();
    for (i, f) in shape.faces.iter().enumerate() {
        groups.entry(twin_key(f)).or_default().push(i);
    }
    let twins: Vec<_> = groups.values().filter(|g| g.len() > 1).collect();
    eprintln!("{} twin groups", twins.len());
    let mut same_side = Vec::new();
    for group in &twins {
        let a = shape.faces[group[0]].normal.unwrap();
        for other in &group[1..] {
            let b = shape.faces[*other].normal.unwrap();
            let dot: f32 = (0..3).map(|k| a[k] * b[k]).sum();
            if dot >= 0. {
                same_side.push((shape.faces[group[0]].address, shape.faces[*other].address));
            }
        }
    }
    eprintln!("same-side twins: {same_side:x?}");
    assert!(same_side.is_empty(), "{same_side:x?}");
}

/// The damage shapes are authored at the same header scale as the intact
/// shape, so the damage looks and the fragments are drawn at the same size.
/// `TU160_A.SH` is the body with the left wing cut at x -24 (piece B), and
/// `TU160_C.SH` the body with the fin cut to z 9 (piece D); the bodies lie
/// within one unit of the intact extents (A's nose is one unit longer).
#[test]
fn damage_shapes_share_the_header_scale_and_the_body() {
    let Some(intact) = read("TU160.SH") else {
        return;
    };
    let scale = object_scale(&intact).unwrap();
    let (low, high) = bounds(&Shape::parse(&intact).unwrap());
    for (name, faces) in [
        ("TU160_A.SH", 313),
        ("TU160_B.SH", 54),
        ("TU160_C.SH", 315),
        ("TU160_D.SH", 38),
    ] {
        let bytes = read(name).unwrap();
        assert_eq!(object_scale(&bytes).unwrap(), scale, "{name}");
        let shape = Shape::parse(&bytes).unwrap();
        assert_eq!(shape.faces.len(), faces, "{name}");
        let (l, h) = bounds(&shape);
        eprintln!("{name}: {l:?} {h:?}");
        if name.ends_with("A.SH") || name.ends_with("C.SH") {
            for i in 0..3 {
                assert!(
                    l[i] >= low[i] - 1. && h[i] <= high[i] + 1.,
                    "{name} axis {i}"
                );
            }
        } else {
            for i in 0..3 {
                assert!(h[i] - l[i] < high[i] - low[i], "{name} axis {i}");
            }
        }
    }
}

/// The wingtip vapor record is in third-of-a-foot CE units whatever the shape's
/// header scale (a Tu-26 copy): x +/-280 is 93.3 ft, one unit (2.7 ft) outside
/// the drawn tip (34 units at 8/3 ft, 90.7 ft), 2.7 ft down (the tip vertex is at
/// z -1) and 16 ft aft (6 units, inside the tip chord y -7 to -3), so the record
/// sits on the spread tip and the rig carries it with the wing's turn.
#[test]
fn streamer_points_sit_on_the_drawn_wing_tips() {
    let Some(bytes) = read("TU160.SH") else {
        return;
    };
    let def = tore_formats::shape::StreamerDef::parse(&bytes)
        .unwrap()
        .expect("streamer record");
    let shape = Shape::parse(&bytes).unwrap();
    let (low, high) = bounds(&shape);
    for (side, tip) in [(0, low[0]), (1, high[0])] {
        let p = def.attachment(side, 0).unwrap();
        eprintln!("streamer side {side}: {p:?} ft*3, tip x {tip}");
        assert!(
            (p[0] / 3. - tip * FEET_PER_UNIT).abs() < 3.,
            "side {side}: {} ft against tip {} ft",
            p[0] / 3.,
            tip * FEET_PER_UNIT
        );
        // Up and forward of the aircraft in feet: z -1 unit, y -6 units.
        assert!((p[1] / 3. + FEET_PER_UNIT).abs() < 0.1, "up {}", p[1] / 3.);
        assert!(
            (p[2] / 3. + 6. * FEET_PER_UNIT).abs() < 0.1,
            "forward {}",
            p[2] / 3.
        );
    }
}
