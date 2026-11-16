//! T95-2: the Tu-95 draws `TU95.SH` from `FA_2.LIB` at the shape's own header
//! scale (2, so 2/3 ft per unit: true size within 1 percent on span and length,
//! height 6 percent low, no per-axis factor, no gear lengthening). This reads
//! the retail shape and its damage shapes and pins the measurements the slice's
//! notes quote. It skips quietly without a retail install and reads data only;
//! no retail bytes are committed. Evidence: docs/formats/aircraft-variety.md,
//! "Tu-95 shape and rig (T95-2/3, 2026-11-15)".
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{
    module,
    shape::{Face, Shape, object_scale},
};
use tore_import::MediaSource;

/// The gear word that puts the six wheel plates down (`33b0`).
const GEAR: usize = 0x33b0;
/// Real Bear-H span, length and height in feet (Wikipedia, the Tu-95MS: 50.1 m,
/// 46.2 m, 12.12 m; the T95-0 review cites them).
const REAL_FEET: [f64; 3] = [164.4, 151.6, 39.8];

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

/// `TU95.SH` has header scale 2 and, drawn at the 2/3 ft a unit that gives,
/// matches the real span and length within 1 percent. The height is 6 percent
/// low (the gear legs are one flat plate each); the gear word adds the six wheel
/// plates and nothing else, and every plate reaches the contact line.
#[test]
fn tu95_sh_is_true_size_at_its_header_scale() {
    let Some(bytes) = read("TU95.SH") else {
        return;
    };
    assert_eq!(module::code(&bytes).unwrap().0.len(), 9_148);
    assert_eq!(object_scale(&bytes).unwrap(), 2.);
    let neutral = Shape::parse(&bytes).unwrap();
    assert_eq!(neutral.faces.len(), 155);
    let down = Shape::with_state(&bytes, &BTreeMap::from([(GEAR, 1)])).unwrap();
    assert_eq!(down.faces.len(), 155 + 6);
    let (clean_low, clean_high) = bounds(&neutral);
    let (low, high) = bounds(&down);
    eprintln!("neutral: {clean_low:?} {clean_high:?}; gear down: {low:?} {high:?}");
    // Units: span x -123..124, length y -130..96 (the tailplane tips; the tail
    // cone ends at -121), belly z -12 (the propeller discs), fin tip z 42, gear
    // contact z -14.
    assert_eq!(
        ([clean_low[0], clean_high[0]], [clean_low[1], clean_high[1]]),
        ([-123., 124.], [-130., 96.])
    );
    assert_eq!((clean_low[2], clean_high[2]), (-12., 42.));
    assert_eq!(low[2], -14.);
    let feet = |lo: f64, hi: f64| (hi - lo) * 2. / 3.;
    let span = feet(clean_low[0], clean_high[0]);
    let length = feet(clean_low[1], clean_high[1]);
    let height = feet(low[2], high[2]);
    eprintln!("span {span} length {length} height {height}");
    for (got, real, axis, tolerance) in [
        (span, REAL_FEET[0], "span", 0.01),
        (length, REAL_FEET[1], "length", 0.01),
        (height, REAL_FEET[2], "height", 0.07),
    ] {
        assert!(
            (got - real).abs() / real < tolerance,
            "{axis}: {got} ft against {real}"
        );
    }
    // The gear word's own faces: two windings each of the left main plate (x
    // -33), the right main plate (x 34) and the nose plate (x 0), all reaching
    // z -14, the mains y 3 to 9 and the nose y 70 to 76.
    let known: std::collections::BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
    let gear: Vec<&Face> = down
        .faces
        .iter()
        .filter(|f| !known.contains(&f.address))
        .collect();
    assert_eq!(gear.len(), 6);
    for f in &gear {
        assert_eq!(f.positions.len(), 4);
        let x = f.positions[0][0];
        assert!(f.positions.iter().all(|p| p[0] == x));
        assert!(matches!(x as i32, -33 | 34 | 0), "plate x {x}");
        let y: Vec<f32> = f.positions.iter().map(|p| p[1]).collect();
        let (lo, hi) = (
            y.iter().copied().fold(f32::INFINITY, f32::min),
            y.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        );
        assert_eq!((lo, hi), if x == 0. { (70., 76.) } else { (3., 9.) });
        assert_eq!(
            f.positions
                .iter()
                .map(|p| p[2])
                .fold(f32::INFINITY, f32::min),
            -14.
        );
    }
}

/// The exported state (the original game's own phase choice) keeps one blur
/// phase per winding of each propeller disc: 30 of the 40 disc faces go, 125
/// faces remain, plus the two indexed `E0` wing decal slots the runtime projector omits (127).
#[test]
fn export_keeps_one_propeller_phase_and_two_decal_slots() {
    let Some(bytes) = read("TU95.SH") else {
        return;
    };
    let neutral = Shape::parse(&bytes).unwrap();
    let export = Shape::with_export_state(&bytes, &BTreeMap::new()).unwrap();
    assert_eq!(export.faces.len(), 127);
    let kept: std::collections::BTreeSet<_> = export.faces.iter().map(|f| f.address).collect();
    let dropped = neutral
        .faces
        .iter()
        .filter(|f| !kept.contains(&f.address))
        .count();
    assert_eq!(dropped, 30);
    let decals: Vec<_> = export
        .faces
        .iter()
        .filter(|f| f.texture.starts_with("@indexed"))
        .map(|f| (f.address, f.texture.clone()))
        .collect();
    assert_eq!(
        decals,
        [
            (0x1f00, "@indexed:4".to_string()),
            (0x2710, "@indexed:3".to_string())
        ]
    );
}

/// Skin review: the exact-vertex twin groups are double-sided panels with
/// opposite stored normals, so the smooth path pairs them and hides the rear
/// member.
#[test]
fn twin_pairs_have_opposite_normals() {
    let Some(bytes) = read("TU95.SH") else {
        return;
    };
    let shape = Shape::parse(&bytes).unwrap();
    let mut groups: BTreeMap<Vec<[u32; 3]>, Vec<usize>> = BTreeMap::new();
    for (i, f) in shape.faces.iter().enumerate() {
        groups.entry(twin_key(f)).or_default().push(i);
    }
    // The propeller discs are five groups of eight coincident faces (four
    // phases, both windings); everything else that repeats is a pair.
    let discs = groups.values().filter(|g| g.len() == 8).count();
    assert_eq!(discs, 5);
    let pairs: Vec<_> = groups.values().filter(|g| g.len() == 2).collect();
    assert_eq!(pairs.len(), 12);
    assert!(groups.values().all(|g| matches!(g.len(), 1 | 2 | 8)));
    let mut same_side = Vec::new();
    for group in &pairs {
        let (a, b) = (&shape.faces[group[0]], &shape.faces[group[1]]);
        let (a_n, b_n) = (a.normal.unwrap(), b.normal.unwrap());
        let dot: f32 = (0..3).map(|k| a_n[k] * b_n[k]).sum();
        if dot >= 0. {
            same_side.push((a.address, b.address));
        }
    }
    assert!(same_side.is_empty(), "{same_side:x?}");
}

/// The damage shapes are authored at the same header scale as the intact
/// shape, so the damage looks and the fragments are drawn at the same size.
#[test]
fn damage_shapes_share_the_header_scale_and_the_body() {
    let Some(intact) = read("TU95.SH") else {
        return;
    };
    let scale = object_scale(&intact).unwrap();
    let (low, high) = bounds(&Shape::parse(&intact).unwrap());
    for (name, faces) in [
        ("TU95_A.SH", 154),
        ("TU95_B.SH", 33),
        ("TU95_C.SH", 146),
        ("TU95_D.SH", 45),
    ] {
        let bytes = read(name).unwrap();
        assert_eq!(object_scale(&bytes).unwrap(), scale, "{name}");
        let shape = Shape::parse(&bytes).unwrap();
        assert_eq!(shape.faces.len(), faces, "{name}");
        let (l, h) = bounds(&shape);
        eprintln!("{name}: {l:?} {h:?}");
        if name.ends_with("A.SH") || name.ends_with("C.SH") {
            for i in 0..3 {
                // TU95_A draws torn geometry past the intact extents (3 units
                // past the left tip, 11 units aft of the tail): within 12.
                assert!(
                    l[i] >= low[i] - 12. && h[i] <= high[i] + 12.,
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
