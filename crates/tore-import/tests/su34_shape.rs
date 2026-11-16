//! SU34-2: the Su-34 draws `SU34.SH` from `FA_2.LIB` at the shape's own header
//! scale (8, so 1/3 ft per unit, drawn 1:1: span 7.3 percent over the real
//! figure, length 7.2 percent under, height 1.5 percent over, no per-axis
//! factor). This reads the retail shape and its damage shapes and pins the
//! measurements the slice's notes quote. It skips quietly without a retail
//! install and reads data only; no retail bytes are committed. Evidence:
//! docs/formats/aircraft-variety.md, "Su-34 shape and rig (SU34-2/3,
//! 2026-11-15)".
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{
    module,
    shape::{Face, Shape, object_scale},
};
use tore_import::MediaSource;

/// The gear word that puts the three plates down (`67cc`).
const GEAR: usize = 0x67cc;
/// Real Su-34 span, length and height in feet (the SU34-0 review: 48.2, 76.6,
/// 20.0).
const REAL_FEET: [f64; 3] = [48.2, 76.6, 20.0];

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

/// `SU34.SH` has header scale 1 (a third of a foot a unit) and, drawn so,
/// is within 8 percent of the real span, length and height; the gear word
/// adds the three plates (eight faces) and nothing else, and the nose plate
/// is the 26 unit low point.
#[test]
fn su34_sh_is_drawn_at_its_header_scale_within_eight_percent() {
    let Some(bytes) = read("SU34.SH") else {
        return;
    };
    assert_eq!(module::code(&bytes).unwrap().0.len(), 22_506);
    assert_eq!(object_scale(&bytes).unwrap(), 1.);
    let neutral = Shape::parse(&bytes).unwrap();
    assert_eq!(neutral.faces.len(), 321);
    let down = Shape::with_state(&bytes, &BTreeMap::from([(GEAR, 1)])).unwrap();
    assert_eq!(down.faces.len(), 321 + 8);
    let (clean_low, clean_high) = bounds(&neutral);
    let (low, high) = bounds(&down);
    eprintln!("neutral: {clean_low:?} {clean_high:?}; gear down: {low:?} {high:?}");
    // Units: x -79 to 76, y -105 to 108, z -16 to 35; gear low -26.
    assert_eq!(
        ([clean_low[0], clean_high[0]], [clean_low[1], clean_high[1]]),
        ([-79., 76.], [-105., 108.])
    );
    assert_eq!((clean_low[2], clean_high[2]), (-16., 35.));
    assert_eq!(low[2], -26.);
    let feet = |lo: f64, hi: f64| (hi - lo) / 3.;
    for (got, real, axis, tolerance) in [
        (
            feet(clean_low[0], clean_high[0]),
            REAL_FEET[0],
            "span",
            0.08,
        ),
        (
            feet(clean_low[1], clean_high[1]),
            REAL_FEET[1],
            "length",
            0.08,
        ),
        (feet(low[2], high[2]), REAL_FEET[2], "height", 0.02),
    ] {
        assert!(
            (got - real).abs() / real < tolerance,
            "{axis}: {got} ft against {real}"
        );
    }
    // The gear word's own faces: the two main plates (x 25 and -24, y -28 to
    // -14, z -25 to 0) and the nose plate and the plate across it (y 57 to
    // 69, z -26 to -5), both windings; the nose reaches the low point.
    let known: std::collections::BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
    let gear: Vec<&Face> = down
        .faces
        .iter()
        .filter(|f| !known.contains(&f.address))
        .collect();
    assert_eq!(gear.len(), 8);
    let lowest = |pick: &dyn Fn(&Face) -> bool| {
        gear.iter()
            .filter(|f| pick(f))
            .flat_map(|f| f.positions.iter().map(|p| f64::from(p[2])))
            .fold(f64::INFINITY, f64::min)
    };
    assert_eq!(
        lowest(&|f| f.positions.iter().all(|p| p[0] == 0. || p[1] == 66.)),
        -26.
    );
    assert_eq!(
        lowest(&|f| f.positions.iter().all(|p| p[0].abs() >= 24.)),
        -25.
    );
}

/// Skin review: the exact-vertex twin groups are double-sided panels with
/// opposite stored normals, so the smooth path pairs them and hides the rear
/// member. 41 groups, no same-side member.
#[test]
fn su34_twin_pairs_have_opposite_normals() {
    let Some(bytes) = read("SU34.SH") else {
        return;
    };
    let shape = Shape::parse(&bytes).unwrap();
    let mut groups: BTreeMap<Vec<[u32; 3]>, Vec<usize>> = BTreeMap::new();
    for (i, f) in shape.faces.iter().enumerate() {
        groups.entry(twin_key(f)).or_default().push(i);
    }
    let twins: Vec<_> = groups.values().filter(|g| g.len() > 1).collect();
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
    eprintln!("{} twin groups, same-side {same_side:x?}", twins.len());
    assert!(same_side.is_empty(), "{same_side:x?}");
    assert_eq!(twins.len(), TWIN_GROUPS);
}
const TWIN_GROUPS: usize = 41;

/// The damage shapes are authored at the same header scale as the intact
/// shape, so the damage looks and the fragments are drawn at the same size.
/// The two body looks lie inside the intact extents (A is the body with the
/// left wing cut off, C the body with the right wing cut off); B and D are
/// small pieces.
#[test]
fn su34_damage_shapes_share_the_header_scale_and_the_body() {
    let Some(intact) = read("SU34.SH") else {
        return;
    };
    let scale = object_scale(&intact).unwrap();
    let (low, high) = bounds(&Shape::parse(&intact).unwrap());
    for (name, faces) in [
        ("SU34_A.SH", 305),
        ("SU34_B.SH", 22),
        ("SU34_C.SH", 283),
        ("SU34_D.SH", 22),
    ] {
        let bytes = read(name).unwrap();
        assert_eq!(object_scale(&bytes).unwrap(), scale, "{name}");
        let shape = Shape::parse(&bytes).unwrap();
        assert_eq!(shape.faces.len(), faces, "{name}");
        let (l, h) = bounds(&shape);
        eprintln!("{name}: {l:?} {h:?}");
        if name.ends_with("A.SH") || name.ends_with("C.SH") {
            // The body looks reach a few units past the intact nose and tail.
            for i in 0..3 {
                assert!(
                    l[i] >= low[i] - 6. && h[i] <= high[i] + 6.,
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

/// The wingtip vapor record is in third-of-a-foot CE units whatever the
/// shape's header scale; at the Su-34's header scale (1/3 ft a unit) the
/// attachment points are the drawn wing tips to within a unit (exactly, x -79 and 76), so the
/// record needs no fix.
#[test]
fn su34_streamer_points_sit_on_the_drawn_wing_tips() {
    let Some(bytes) = read("SU34.SH") else {
        return;
    };
    let def = tore_formats::shape::StreamerDef::parse(&bytes)
        .unwrap()
        .expect("streamer record");
    let shape = Shape::parse(&bytes).unwrap();
    let (low, high) = bounds(&shape);
    for (side, tip) in [(0, low[0]), (1, high[0])] {
        let p = def.attachment(side, 0).unwrap();
        eprintln!("streamer side {side}: {p:?} against tip x {tip}");
        assert!(
            (p[0].abs() - tip.abs()).abs() <= 1.,
            "side {side}: {} against tip {}",
            p[0],
            tip
        );
    }
}
