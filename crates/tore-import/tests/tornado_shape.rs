//! TOR-2: the Tornado IDS draws `TOR.SH` from `FA_2.LIB` at the shape's own
//! header scale (2, so 2/3 ft per unit: true size within 3 percent, no
//! per-axis factor). This reads the retail shape and its damage shapes and
//! pins the measurements the slice's notes quote. It skips quietly without a
//! retail install and reads data only; no retail bytes are committed.
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{
    module,
    shape::{Face, Shape, StreamerDef, object_scale},
};
use tore_import::MediaSource;

/// Gear word that puts the wheels down (the Tornado review's `7dcc`).
const GEAR: usize = 0x7dcc;
/// Real Tornado IDS dimensions in feet: span with the wing spread, length and
/// height (45.6 ft, 54.8 ft, 19.5 ft; the TOR-0 review cites them).
const REAL_FEET: [f64; 3] = [45.6, 54.8, 19.5];

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

/// The base `FA_2.LIB` shape (CODE 28,150 bytes, not the toolkit patch's
/// 28,198) has header scale 2 and, drawn at the 2/3 ft a unit that scale
/// gives, is the real Tornado's size within 3 percent on every axis.
#[test]
fn tor_sh_is_true_size_at_its_header_scale() {
    let Some(bytes) = read("TOR.SH") else {
        return;
    };
    assert_eq!(module::code(&bytes).unwrap().0.len(), 28_150);
    // Header scale 2 is 2/3 ft per unit at the aircraft convention.
    assert_eq!(object_scale(&bytes).unwrap(), 2.);
    let neutral = Shape::parse(&bytes).unwrap();
    assert_eq!(neutral.faces.len(), 287);
    let down = Shape::with_state(&bytes, &BTreeMap::from([(GEAR, 1)])).unwrap();
    assert_eq!(down.faces.len(), 287 + 26);
    let (low, high) = bounds(&down);
    eprintln!("gear down: {low:?} {high:?}");
    let (clean_low, clean_high) = bounds(&neutral);
    eprintln!("neutral: {clean_low:?} {clean_high:?}");
    let feet = |lo: f64, hi: f64| (hi - lo) * 2. / 3.;
    let span = feet(clean_low[0], clean_high[0]);
    let length = feet(clean_low[1], clean_high[1]);
    let height = feet(low[2], high[2]);
    eprintln!("span {span} length {length} height {height}");
    for (got, real, axis) in [
        (span, REAL_FEET[0], "span"),
        (length, REAL_FEET[1], "length"),
        (height, REAL_FEET[2], "height"),
    ] {
        assert!(
            (got - real).abs() / real < 0.03,
            "{axis}: {got} ft against {real}"
        );
    }
}

/// Skin review: `TOR.SH` has 22 exact-vertex twin groups. Twenty-one are
/// double-sided panels with opposite stored normals, which the smooth path
/// pairs and hides the rear member of without the MiG-21 exception. The 22nd
/// (`255c` and `257a`, a one-unit sliver on the centreline at x 0, y 0 to -25,
/// z 3 to 4) carries two normals mirrored in x, not opposite, so the pairing
/// skips it and draws both members. Both lie in the dorsal ridge line.
#[test]
fn twin_pairs_have_opposite_normals_except_the_centreline_sliver() {
    let Some(bytes) = read("TOR.SH") else {
        return;
    };
    let shape = Shape::parse(&bytes).unwrap();
    let mut groups: BTreeMap<Vec<[u32; 3]>, Vec<usize>> = BTreeMap::new();
    for (i, f) in shape.faces.iter().enumerate() {
        groups.entry(twin_key(f)).or_default().push(i);
    }
    let twins: Vec<_> = groups.values().filter(|g| g.len() > 1).collect();
    assert_eq!(twins.len(), 22);
    let mut same_side = Vec::new();
    for group in twins {
        assert_eq!(group.len(), 2);
        let a = shape.faces[group[0]].normal.unwrap();
        let b = shape.faces[group[1]].normal.unwrap();
        let dot: f32 = (0..3).map(|k| a[k] * b[k]).sum();
        if dot >= 0. {
            same_side.push((shape.faces[group[0]].address, shape.faces[group[1]].address));
            assert!(
                shape.faces[group[0]].positions.iter().all(|p| p[0] == 0.),
                "only a centreline face may keep both members"
            );
        }
    }
    assert_eq!(same_side, [(0x255c, 0x257a)]);
}

/// The damage shapes are authored at the same header scale as the intact
/// shape, so the damage looks and the fragments are drawn at the same size.
#[test]
fn damage_shapes_share_the_header_scale_and_the_body() {
    let Some(intact) = read("TOR.SH") else {
        return;
    };
    let scale = object_scale(&intact).unwrap();
    let (low, high) = bounds(&Shape::parse(&intact).unwrap());
    for (name, faces) in [
        ("TOR_A.SH", 290),
        ("TOR_B.SH", 36),
        ("TOR_C.SH", 314),
        ("TOR_D.SH", 37),
    ] {
        let bytes = read(name).unwrap();
        assert_eq!(object_scale(&bytes).unwrap(), scale, "{name}");
        let shape = Shape::parse(&bytes).unwrap();
        assert_eq!(shape.faces.len(), faces, "{name}");
        let (l, h) = bounds(&shape);
        eprintln!("{name}: {l:?} {h:?}");
        if name.ends_with("A.SH") || name.ends_with("C.SH") {
            // A body sits on the intact body: no reach past it by a unit.
            for i in 0..3 {
                assert!(
                    l[i] >= low[i] - 1. && h[i] <= high[i] + 1.,
                    "{name} axis {i}"
                );
            }
        } else {
            // A fragment is authored about its own origin and is smaller
            // than the aircraft on every axis.
            for i in 0..3 {
                assert!(h[i] - l[i] < high[i] - low[i], "{name} axis {i}");
            }
        }
    }
}

/// The wingtip vapor record is in third-of-a-foot CE units whatever the
/// shape's header scale, and its spread-wing attachment lands within a foot
/// of the wing tip drawn at 2/3 ft a unit.
#[test]
fn streamer_points_sit_on_the_drawn_wing_tips() {
    let Some(bytes) = read("TOR.SH") else {
        return;
    };
    let def = StreamerDef::parse(&bytes)
        .unwrap()
        .expect("streamer record");
    let shape = Shape::parse(&bytes).unwrap();
    let (low, high) = bounds(&shape);
    for (side, tip) in [(0, low[0]), (1, high[0])] {
        let p = def.attachment(side, 0).unwrap();
        eprintln!("streamer side {side}: {p:?} ft*3, tip x {tip}");
        let right_ft = p[0] / 3.;
        assert!(
            (right_ft - tip * 2. / 3.).abs() < 1.,
            "side {side}: {right_ft} ft against tip {} ft",
            tip * 2. / 3.
        );
    }
}
