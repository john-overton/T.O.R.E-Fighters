//! B52-2: the B-52H draws `B52.SH` from `FA_2.LIB` at the shape's own header
//! scale (2, so 2/3 ft per unit: true size within 3 percent on span and length,
//! height 8 percent low, no per-axis factor, no gear lengthening). This reads
//! the retail shape and its damage shapes and pins the measurements the slice's
//! notes quote. It skips quietly without a retail install and reads data only;
//! no retail bytes are committed. Evidence: docs/formats/aircraft-variety.md,
//! "B-52 shape, size and looks (B52-2, 2026-11-15)".
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{
    module,
    shape::{Face, Shape, object_scale},
};
use tore_import::MediaSource;

/// The gear word that puts the four trucks and two outriggers down (`7a26`).
const GEAR: usize = 0x7a26;
/// Real B-52H span, length and height in feet (185 ft, 159 ft 4 in, 40 ft
/// 8 in; the B52-0 review cites them).
const REAL_FEET: [f64; 3] = [185., 159.33, 40.67];

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

/// `B52.SH` has header scale 2 and, drawn at the 2/3 ft a unit that gives,
/// matches the real span and length within 3 percent. The height is 8 percent
/// low (the gear legs); the gear word adds the four trucks and two outriggers
/// and nothing else, and the outriggers reach the trucks' contact line.
#[test]
fn b52_sh_is_true_size_at_its_header_scale() {
    let Some(bytes) = read("B52.SH") else {
        return;
    };
    assert_eq!(module::code(&bytes).unwrap().0.len(), 27_204);
    assert_eq!(object_scale(&bytes).unwrap(), 2.);
    let neutral = Shape::parse(&bytes).unwrap();
    assert_eq!(neutral.faces.len(), 271);
    let down = Shape::with_state(&bytes, &BTreeMap::from([(GEAR, 1)])).unwrap();
    assert_eq!(down.faces.len(), 271 + 32);
    let (clean_low, clean_high) = bounds(&neutral);
    let (low, high) = bounds(&down);
    eprintln!("neutral: {clean_low:?} {clean_high:?}; gear down: {low:?} {high:?}");
    // Units: span x -139..139, length y -138..107, belly z -9, fin tip z 42,
    // gear contact z -14.
    assert_eq!(
        ([clean_low[0], clean_high[0]], [clean_low[1], clean_high[1]]),
        ([-139., 139.], [-138., 107.])
    );
    assert_eq!((clean_low[2], clean_high[2]), (-9., 42.));
    assert_eq!(low[2], -14.);
    let feet = |lo: f64, hi: f64| (hi - lo) * 2. / 3.;
    let span = feet(clean_low[0], clean_high[0]);
    let length = feet(clean_low[1], clean_high[1]);
    let height = feet(low[2], high[2]);
    eprintln!("span {span} length {length} height {height}");
    for (got, real, axis, tolerance) in [
        (span, REAL_FEET[0], "span", 0.03),
        (length, REAL_FEET[1], "length", 0.03),
        (height, REAL_FEET[2], "height", 0.09),
    ] {
        assert!(
            (got - real).abs() / real < tolerance,
            "{axis}: {got} ft against {real}"
        );
    }
    // The gear word's own faces: four trucks of 6 plates and two outriggers
    // of 4 plates, lowest at -14, the outriggers out at x +/-108 to 113 and
    // the trucks 47.3 ft apart (y 39.5 and -31.5 units, 71 units).
    let known: std::collections::BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
    let gear: Vec<&Face> = down
        .faces
        .iter()
        .filter(|f| !known.contains(&f.address))
        .collect();
    assert_eq!(gear.len(), 32);
    let lowest = |pick: &dyn Fn(&Face) -> bool| {
        gear.iter()
            .filter(|f| pick(f))
            .flat_map(|f| f.positions.iter().map(|p| f64::from(p[2])))
            .fold(f64::INFINITY, f64::min)
    };
    let outrigger = |f: &Face| f.positions.iter().all(|p| p[0].abs() >= 108.);
    assert_eq!(gear.iter().filter(|f| outrigger(f)).count(), 8);
    assert_eq!(lowest(&outrigger), -14.);
    assert_eq!(lowest(&|f| !outrigger(f)), -14.);
}

/// Skin review: the exact-vertex twin groups are double-sided panels with
/// opposite stored normals (twelve groups; the MiG-21 exception and the
/// Tornado's centreline sliver do not occur), so the smooth path pairs them
/// and hides the rear member.
#[test]
fn twin_pairs_have_opposite_normals() {
    let Some(bytes) = read("B52.SH") else {
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
}
const TWIN_GROUPS: usize = 12;

/// The damage shapes are authored at the same header scale as the intact
/// shape, so the damage looks and the fragments are drawn at the same size.
#[test]
fn damage_shapes_share_the_header_scale_and_the_body() {
    let Some(intact) = read("B52.SH") else {
        return;
    };
    let scale = object_scale(&intact).unwrap();
    let (low, high) = bounds(&Shape::parse(&intact).unwrap());
    for (name, faces) in [
        ("B52_A.SH", 372),
        ("B52_B.SH", 116),
        ("B52_C.SH", 387),
        ("B52_D.SH", 84),
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

/// The wingtip vapor record is in third-of-a-foot CE units whatever the
/// shape's header scale: x +/-278 is 92.67 ft, exactly the drawn tip (139
/// units at 2/3 ft), 2 ft up (the wing tip is at z 3) and 43.3 ft aft (65
/// units, inside the tip chord y -49 to -69). No fix needed.
#[test]
fn streamer_points_sit_on_the_drawn_wing_tips() {
    let Some(bytes) = read("B52.SH") else {
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
            (p[0] / 3. - tip * 2. / 3.).abs() < 1.,
            "side {side}: {} ft against tip {} ft",
            p[0] / 3.,
            tip * 2. / 3.
        );
    }
}
