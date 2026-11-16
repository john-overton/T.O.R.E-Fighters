//! B1B-2: the B-1B draws `B1.SH` from `FA_2.LIB` at the shape's own header
//! scale (4, so 4/3 ft per unit: true size within 3 percent on span and length,
//! height 2 percent low gear up and 10 percent high gear down, no per-axis
//! factor). This reads the retail shape and its damage shapes and pins the
//! measurements the slice's notes quote. It skips quietly without a retail
//! install and reads data only; no retail bytes are committed. Evidence:
//! docs/formats/aircraft-variety.md, "B-1B shape and rig (B1B-2/3,
//! 2026-11-15)".
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{
    module,
    shape::{Face, Shape, StreamerDef, object_scale},
};
use tore_import::MediaSource;

/// The gear word (`7b22`) and the state words the slice rigs.
const GEAR: usize = 0x7b22;
/// Real B-1B span spread, length and height in feet (136 ft 8.5 in, 147 ft,
/// 34 ft: `B1.INF`; the Air Force fact sheet says 137, 146, 34).
const REAL_FEET: [f64; 3] = [136.7, 146.5, 34.];

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

fn bounds(faces: &[Face]) -> ([f64; 3], [f64; 3]) {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for p in faces.iter().flat_map(|f| &f.positions) {
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

fn pose(bytes: &[u8], words: &[(usize, i32)]) -> Shape {
    Shape::with_state(bytes, &words.iter().copied().collect()).unwrap()
}

fn addresses(shape: &Shape) -> Vec<usize> {
    shape.faces.iter().map(|f| f.address).collect()
}

/// `B1.SH` has header scale 4 and, drawn at the 4/3 ft a unit that gives,
/// matches the real span and length within 3 percent. The height is 2 percent
/// low with the gear up and 10 percent high with it down (the gear plates
/// reach 8 units under the reference point); the gear word adds twelve plates
/// and nothing else.
#[test]
fn b1_sh_is_true_size_at_its_header_scale() {
    let Some(bytes) = read("B1.SH") else {
        return;
    };
    assert_eq!(module::code(&bytes).unwrap().0.len(), 27_468);
    assert_eq!(object_scale(&bytes).unwrap(), 4.);
    let neutral = Shape::parse(&bytes).unwrap();
    assert_eq!(neutral.faces.len(), 314);
    let down = pose(&bytes, &[(GEAR, 1)]);
    assert_eq!(down.faces.len(), 314 + 12);
    let (clean_low, clean_high) = bounds(&neutral.faces);
    let (low, high) = bounds(&down.faces);
    // Units: span x -51..51, length y -42..65, belly -5 (the nacelle
    // bottoms), fin tip 20, gear contact -8.
    assert_eq!(
        ([clean_low[0], clean_high[0]], [clean_low[1], clean_high[1]]),
        ([-51., 51.], [-42., 65.])
    );
    assert_eq!((clean_low[2], clean_high[2]), (-5., 20.));
    assert_eq!(low[2], -8.);
    let feet = |lo: f64, hi: f64| (hi - lo) * 4. / 3.;
    let span = feet(clean_low[0], clean_high[0]);
    let length = feet(clean_low[1], clean_high[1]);
    let height_up = feet(clean_low[2], clean_high[2]);
    let height_down = feet(low[2], high[2]);
    eprintln!("span {span} length {length} height {height_up} / {height_down}");
    for (got, real, axis, tolerance) in [
        (span, REAL_FEET[0], "span", 0.03),
        (length, REAL_FEET[1], "length", 0.03),
        (height_up, REAL_FEET[2], "height gear up", 0.03),
        (height_down, REAL_FEET[2], "height gear down", 0.10),
    ] {
        assert!(
            (got - real).abs() / real < tolerance,
            "{axis}: {got} ft against {real}"
        );
    }
    // The gear word's own faces: the right main (side plate x 2, cross plate
    // y -1), the left main, the nose (x 0 plate and y 45 cross plate), two
    // windings each, lowest at -8.
    let known: std::collections::BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
    let gear: Vec<&Face> = down
        .faces
        .iter()
        .filter(|f| !known.contains(&f.address))
        .collect();
    assert_eq!(gear.len(), 12);
    assert!(
        gear.iter()
            .flat_map(|f| f.positions.iter().map(|p| f64::from(p[2])))
            .all(|z| (-8. ..=-1.).contains(&z))
    );
    assert_eq!(
        gear.iter()
            .filter(|f| f.positions.iter().all(|p| p[1] > 40.))
            .count(),
        4
    );
}

/// Skin review: the exact-vertex twin groups are double-sided panels with
/// opposite stored normals, so the smooth path pairs them and hides the rear
/// member. Decal slots: the export path finds two runtime `E0` slots the
/// runtime path omits, one plate a side on the forward glove (x 4 to 7,
/// y 4 to 9, z 1 to 5), so no blank patch is left.
#[test]
fn skin_twins_and_the_two_decal_slots() {
    let Some(bytes) = read("B1.SH") else {
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
        assert_eq!(group.len(), 2);
        let a = shape.faces[group[0]].normal.unwrap();
        let b = shape.faces[group[1]].normal.unwrap();
        let dot: f32 = (0..3).map(|k| a[k] * b[k]).sum();
        if dot >= 0. {
            same_side.push((shape.faces[group[0]].address, shape.faces[group[1]].address));
        }
    }
    eprintln!("same-side twins: {same_side:x?}");
    assert!(same_side.is_empty(), "{same_side:x?}");
    assert_eq!(twins.len(), TWIN_GROUPS);
    let export = Shape::with_export_state(&bytes, &BTreeMap::new()).unwrap();
    assert_eq!((export.faces.len(), shape.faces.len()), (316, 314));
    let runtime = addresses(&shape);
    let mut extra: Vec<&Face> = export
        .faces
        .iter()
        .filter(|f| !runtime.contains(&f.address))
        .collect();
    extra.sort_by_key(|f| f.address);
    assert_eq!(
        extra.iter().map(|f| f.address).collect::<Vec<_>>(),
        [0x2249, 0x2a36]
    );
    assert_eq!(
        bounds(std::slice::from_ref(extra[0])),
        ([-7., 4., 1.], [-4., 9., 5.])
    );
    assert_eq!(
        bounds(std::slice::from_ref(extra[1])),
        ([4., 4., 1.], [7., 9., 5.])
    );
}
const TWIN_GROUPS: usize = 12;

/// The seven words add and remove exactly the reviewed faces, and the two
/// wing panels are 17 whole faces each that share only the pivot line
/// (+-10, 11) with every fixed face.
#[test]
fn the_state_words_and_the_swing_panels() {
    let Some(bytes) = read("B1.SH") else {
        return;
    };
    let shape = Shape::parse(&bytes).unwrap();
    assert_eq!(
        shape.state_words.iter().copied().collect::<Vec<_>>(),
        [0x7b10, 0x7b16, 0x7b1c, 0x7b22, 0x7b2e, 0x7b34, 0x7b3a]
    );
    for (words, count) in [
        (vec![(0x7b10, 1)], 314 + 16),
        (vec![(0x7b16, 1)], 314 + 8),
        (vec![(0x7b1c, 1)], 314 + 4),
        (vec![(0x7b22, 1)], 314 + 12),
        (vec![(0x7b2e, -1)], 314 - 2 + 3),
        (vec![(0x7b34, -1)], 314 - 2 + 3),
        (vec![(0x7b2e, 1)], 314 - 2),
        (vec![(0x7b34, 1)], 314 - 2),
        (vec![(0x7b3a, 1)], 314),
        (vec![(0x7b3a, -1)], 314),
        (vec![(0x7b3a, 2)], 314 - 2),
    ] {
        assert_eq!(pose(&bytes, &words).faces.len(), count, "{words:x?}");
    }
    let right: [usize; 17] = [
        0x4558, 0x45a7, 0x45c8, 0x460f, 0x4528, 0x45f6, 0x4579, 0x44ce, 0x4541, 0x44fc, 0x4667,
        0x4686, 0x4626, 0x45df, 0x4513, 0x44e5, 0x4590,
    ];
    let left: [usize; 17] = [
        0x4a37, 0x4a7e, 0x49de, 0x4a16, 0x49ae, 0x4a65, 0x49ff, 0x493d, 0x49c7, 0x496b, 0x4ad6,
        0x4af5, 0x4a95, 0x4a4e, 0x4999, 0x4954, 0x4982,
    ];
    let bits = |f: &Face| {
        f.positions
            .iter()
            .map(|p| p.map(f32::to_bits))
            .collect::<Vec<_>>()
    };
    for (ids, side) in [(&right, 1f32), (&left, -1f32)] {
        let panel: Vec<_> = shape
            .faces
            .iter()
            .filter(|f| ids.contains(&f.address))
            .collect();
        assert_eq!(panel.len(), 17);
        let points: std::collections::BTreeSet<[u32; 3]> =
            panel.iter().flat_map(|f| bits(f)).collect();
        let shared: std::collections::BTreeSet<[u32; 3]> = shape
            .faces
            .iter()
            .filter(|f| !right.contains(&f.address) && !left.contains(&f.address))
            .flat_map(&bits)
            .filter(|p| points.contains(p))
            .collect();
        assert!(!shared.is_empty());
        for p in shared {
            let p = p.map(f32::from_bits);
            assert_eq!((p[0], p[1]), (side * 10., 11.), "{p:?}");
        }
    }
}

/// The record's streamer is a valid swing-wing one: its pivot (40, 0, 44)
/// CE units (third-of-a-foot) is exactly the sweep pivot (10, 11) units at
/// 4/3 ft a unit, and its tip points are the drawn trailing tips (51, -3).
#[test]
fn the_streamer_record_is_the_swing_pivot_and_the_drawn_tip() {
    let Some(bytes) = read("B1.SH") else {
        return;
    };
    let def = StreamerDef::parse(&bytes)
        .unwrap()
        .expect("streamer record");
    assert_eq!(def.pivot, [10240, 0, 11264]);
    assert_eq!(def.hinge_scale, 40);
    let feet = |side: usize| def.attachment(side, 0).unwrap().map(|v| v / 3.);
    let (left, right) = (feet(0), feet(1));
    for (p, sign) in [(left, -1.), (right, 1.)] {
        assert!((p[0] - sign * 68.).abs() < 1e-9, "{p:?}");
        assert!(p[1].abs() < 1e-9 && (p[2] + 4.).abs() < 1e-9, "{p:?}");
    }
    let shape = Shape::parse(&bytes).unwrap();
    let (low, high) = bounds(&shape.faces);
    assert_eq!((low[0] * 4. / 3., high[0] * 4. / 3.), (-68., 68.));
    // The pivot in feet: 13.33 right, 14.67 forward.
    assert_eq!((def.pivot[0] / 256, def.pivot[2] / 256), (40, 44));
}

/// The damage shapes are authored at the same header scale as the intact
/// shape, so the damage looks and the fragments are drawn at the same size.
#[test]
fn damage_shapes_share_the_header_scale() {
    let Some(intact) = read("B1.SH") else {
        return;
    };
    let scale = object_scale(&intact).unwrap();
    let (low, high) = bounds(&Shape::parse(&intact).unwrap().faces);
    for (name, faces, body) in [
        ("B1_A.SH", 346, true),
        ("B1_B.SH", 74, false),
        ("B1_C.SH", 283, true),
        ("B1_D.SH", 114, false),
    ] {
        let bytes = read(name).unwrap();
        assert_eq!(object_scale(&bytes).unwrap(), scale, "{name}");
        let shape = Shape::parse(&bytes).unwrap();
        assert_eq!(shape.faces.len(), faces, "{name}");
        let (l, h) = bounds(&shape.faces);
        eprintln!("{name}: {l:?} {h:?}");
        for i in 0..3 {
            if body {
                assert!(l[i] >= low[i] - 8. && h[i] <= high[i] + 8., "{name} {i}");
            } else {
                assert!(h[i] - l[i] < high[i] - low[i], "{name} {i}");
            }
        }
    }
}
