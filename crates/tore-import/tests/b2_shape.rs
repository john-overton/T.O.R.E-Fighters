//! B2-2: the B-2 draws `B2.SH` from `FA_2.LIB` at the shape's own header
//! scale (exponent 8, so 1/3 ft a unit, the aircraft default): true span, a
//! length 6 percent short and a height (gear down) 12 percent tall, no
//! per-axis factor (John's decision 19, 2026-11-15). This reads the retail
//! shape, the four drag-rudder poses of word 6498 and the damage shapes and
//! pins the measurements the slice's notes quote. It skips quietly without a
//! retail install and reads data only; no retail bytes are committed.
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{
    aircraft::{AircraftId, scale::shape_axis_scale},
    module,
    shape::{Face, Shape, StreamerDef, object_scale},
};
use tore_import::MediaSource;

const GEAR: usize = 0x648c;
const BAY: usize = 0x6486;
const RUDDER: usize = 0x6498;
/// Real B-2 dimensions in feet (`B2.INF` and the B2-0 review): span, length
/// and height.
const REAL_FEET: [f64; 3] = [172., 69., 17.];

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
    Shape::with_state(bytes, &words.iter().copied().collect::<BTreeMap<_, _>>()).unwrap()
}

fn addresses(shape: &Shape) -> Vec<usize> {
    shape.faces.iter().map(|f| f.address).collect()
}

/// The shape header gives scale 1 (one unit is 1/3 ft), so the table needs no
/// entry: drawn clean it is exactly the real span and 6 percent short, and
/// with the gear down 12 percent tall. The gear plates' low points are the
/// deployed gear contact heights: the inner main plates reach -33 (11 ft), the
/// nose strut -31 (10.3 ft) and the outer main plates only -27.
#[test]
fn b2_sh_is_drawn_at_its_header_scale_and_needs_no_scale_entry() {
    let Some(bytes) = read("B2.SH") else {
        return;
    };
    assert_eq!(module::code(&bytes).unwrap().0.len(), 21_668);
    assert_eq!(object_scale(&bytes).unwrap(), 1.);
    assert_eq!(shape_axis_scale(AircraftId::B2), [1., 1., 1.]);
    let neutral = Shape::parse(&bytes).unwrap();
    assert_eq!(neutral.faces.len(), 268);
    assert_eq!(
        neutral
            .faces
            .iter()
            .map(|f| f.texture.as_str())
            .collect::<std::collections::BTreeSet<_>>(),
        ["_B2.PIC"].into()
    );
    let down = pose(&bytes, &[(GEAR, 1)]);
    assert_eq!(down.faces.len(), 268 + 14);
    let (low, high) = bounds(&neutral.faces);
    let (down_low, down_high) = bounds(&down.faces);
    // Raw units: x -258..258, y -87..107, z -12..24 clean, -33..24 gear down.
    assert_eq!((low[0], high[0]), (-258., 258.));
    assert_eq!((low[1], high[1]), (-87., 107.));
    assert_eq!((low[2], high[2]), (-12., 24.));
    assert_eq!((down_low[2], down_high[2]), (-33., 24.));
    let feet = |lo: f64, hi: f64| (hi - lo) / 3.;
    let span = feet(low[0], high[0]);
    let length = feet(low[1], high[1]);
    let height = feet(down_low[2], down_high[2]);
    assert!((span - REAL_FEET[0]).abs() < 0.01);
    assert!((length / REAL_FEET[1] - 1. + 0.063).abs() < 0.002);
    assert!((height / REAL_FEET[2] - 1. - 0.118).abs() < 0.002);
    // The leading edge makes 32.8 degrees with the lateral axis (Jane's 33).
    let sweep = ((107f64 + 59.) / 258.).atan().to_degrees();
    assert!((sweep - 32.8).abs() < 0.1, "{sweep}");
    // Gear plates: low points by plate group (nose strut, outer and inner mains).
    let old = addresses(&neutral);
    let mut lows = BTreeMap::new();
    for f in down.faces.iter().filter(|f| !old.contains(&f.address)) {
        let (l, _) = bounds(std::slice::from_ref(f));
        let key = (l[0] as i32).abs();
        let at: &mut f64 = lows.entry(key).or_insert(l[2]);
        *at = at.min(l[2]);
    }
    // Keys are |x| of the plate: 0 nose strut, 6 nose pair, 62 and 65 inner
    // mains, 69 and 71 outer mains.
    assert_eq!(
        lows,
        BTreeMap::from([
            (0, -31.),
            (6, -24.),
            (62, -33.),
            (65, -33.),
            (69, -27.),
            (71, -27.)
        ])
    );
    // The wingtip vapor record is valid (the B2-0 review took it for junk): a
    // zero pivot and both points on the drawn wing tips, 258 right, 3 down and
    // 59 aft in CE units (one third of a foot, the shape's own units here).
    let def = StreamerDef::parse(&bytes)
        .unwrap()
        .expect("streamer record");
    assert_eq!(def.pivot, [0; 3]);
    for (side, tip) in [(0, low[0]), (1, high[0])] {
        assert_eq!(def.attachment(side, 0).unwrap(), [tip, -3., -59.]);
    }
}

/// Skin review: `B2.SH` has 11 exact-vertex twin groups, all double-sided
/// panels with opposite stored normals (no MiG-21 exception needed), and the
/// export path finds one runtime decal slot the runtime path omits.
#[test]
fn skin_twins_and_the_one_decal_slot() {
    let Some(bytes) = read("B2.SH") else {
        return;
    };
    let shape = Shape::parse(&bytes).unwrap();
    let mut groups: BTreeMap<Vec<[u32; 3]>, Vec<usize>> = BTreeMap::new();
    for (i, f) in shape.faces.iter().enumerate() {
        groups.entry(twin_key(f)).or_default().push(i);
    }
    let twins: Vec<_> = groups.values().filter(|g| g.len() > 1).collect();
    assert_eq!(twins.len(), 11);
    for group in twins {
        assert_eq!(group.len(), 2);
        let a = shape.faces[group[0]].normal.unwrap();
        let b = shape.faces[group[1]].normal.unwrap();
        let dot: f32 = (0..3).map(|k| a[k] * b[k]).sum();
        assert!(dot < 0., "{:x}", shape.faces[group[0]].address);
    }
    // Runtime decal slot 2 (`2a73`, a 7 by 5 unit plate on the right of the
    // top skin, x 21 to 28, y 60 to 65, z 4 to 5) is omitted at runtime.
    let export = Shape::with_export_state(&bytes, &BTreeMap::new()).unwrap();
    assert_eq!((export.faces.len(), shape.faces.len()), (269, 268));
    let runtime = addresses(&shape);
    let extra: Vec<_> = export
        .faces
        .iter()
        .filter(|f| !runtime.contains(&f.address))
        .collect();
    assert_eq!(extra.len(), 1);
    assert_eq!(extra[0].address, 0x2a73);
    let (l, h) = bounds(std::slice::from_ref(extra[0]));
    assert_eq!((l, h), ([21., 60., 4.], [28., 65., 5.]));
}

/// Word 6498 is signed: +1 opens the right drag rudder, -1 the left, 0 is
/// closed, 2 or more removes the right closed pair and adds nothing, and -2
/// likewise removes the left one. The two open poses mirror about the mesh axis within a
/// unit, and the bay word 6486 adds the four hanging plates.
#[test]
fn the_drag_rudder_poses_load_and_mirror() {
    let Some(bytes) = read("B2.SH") else {
        return;
    };
    let neutral = pose(&bytes, &[]);
    let base = addresses(&neutral);
    let at = |v| pose(&bytes, &[(RUDDER, v)]);
    assert_eq!(addresses(&at(0)), base);
    let mut added = Vec::new();
    for (value, faces, gone) in [
        (1, 272, vec![0x465f, 0x4679]),
        (-1, 272, vec![0x4600, 0x461a]),
        (2, 266, vec![0x465f, 0x4679]),
        (3, 266, vec![0x465f, 0x4679]),
        (-2, 266, vec![0x4600, 0x461a]),
    ] {
        let shape = at(value);
        assert_eq!(shape.faces.len(), faces, "{value}");
        let now = addresses(&shape);
        let removed: Vec<_> = base.iter().copied().filter(|a| !now.contains(a)).collect();
        assert_eq!(removed, gone, "{value}");
        let new: Vec<Face> = shape
            .faces
            .iter()
            .filter(|f| !base.contains(&f.address))
            .cloned()
            .collect();
        added.push((value, new));
    }
    let bounds_of = |v: i32| bounds(&added.iter().find(|(x, _)| *x == v).unwrap().1);
    assert_eq!(added[0].1.len(), 6);
    assert_eq!(added[1].1.len(), 6);
    assert!(added[2..].iter().all(|(_, faces)| faces.is_empty()));
    let (rl, rh) = bounds_of(1);
    let (ll, lh) = bounds_of(-1);
    assert_eq!((rl, rh), ([167., -77., -14.], [215., -46., 7.]));
    assert_eq!((ll, lh), ([-218., -78., -14.], [-169., -46., 8.]));
    // Mirror about the mesh axis x = -1, within a unit and a half.
    assert!((-2. - rl[0] - lh[0]).abs() <= 1.5 && (-2. - rh[0] - ll[0]).abs() <= 1.5);
    for i in 1..3 {
        assert!((rl[i] - ll[i]).abs() <= 1.5 && (rh[i] - lh[i]).abs() <= 1.5);
    }
    // Every added face is a whole quad or the one inboard triangle.
    for (_, faces) in &added {
        assert!(faces.iter().all(|f| matches!(f.positions.len(), 3 | 4)));
    }
    // The bay word adds four plates (two windings each) and the poses compose.
    assert_eq!(pose(&bytes, &[(BAY, 1)]).faces.len(), 268 + 8);
    assert_eq!(
        pose(&bytes, &[(BAY, 1), (GEAR, 1), (RUDDER, 1)])
            .faces
            .len(),
        268 + 8 + 14 + 4
    );
}

/// The damage bodies and pieces are authored at the same header scale as the
/// intact shape, so the damage looks and the fragments draw at the same size.
#[test]
fn damage_shapes_share_the_header_scale() {
    let Some(intact) = read("B2.SH") else {
        return;
    };
    let scale = object_scale(&intact).unwrap();
    let (low, high) = bounds(&Shape::parse(&intact).unwrap().faces);
    for (name, faces, body) in [
        ("B2_A.SH", 248, true),
        ("B2_B.SH", 114, false),
        ("B2_C.SH", 254, true),
        ("B2_D.SH", 74, false),
    ] {
        let bytes = read(name).unwrap();
        assert_eq!(object_scale(&bytes).unwrap(), scale, "{name}");
        let shape = Shape::parse(&bytes).unwrap();
        assert_eq!(shape.faces.len(), faces, "{name}");
        let (l, h) = bounds(&shape.faces);
        for i in 0..3 {
            if body {
                // A body sits on the intact one: within 8 units (the C
                // body's nose reaches 7 units, 2.3 ft, past the intact tip).
                assert!(l[i] >= low[i] - 8. && h[i] <= high[i] + 8., "{name} {i}");
            } else {
                assert!(h[i] - l[i] < high[i] - low[i], "{name} {i}");
            }
        }
    }
}
