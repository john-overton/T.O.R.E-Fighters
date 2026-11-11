//! MF1-2: the Mirage F1's per-axis shape scale was fitted to the measured
//! `MF1.SH` extents. This reads the retail shape and checks that the pinned
//! extents are the real ones, that the scaled shape lands on the real
//! dimensions, that every stored face normal stays perpendicular to its
//! scaled face, that every double-sided panel is an exact twin with an opposite
//! normal, and that the two E0 decal slots stay omitted. It skips quietly
//! without a retail install and reads data only; no retail bytes are
//! committed.
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{
    aircraft::scale::{MF1_REAL_FEET, MF1_SHAPE_EXTENT, MF1_SHAPE_SCALE},
    shape::Shape,
};
use tore_import::MediaSource;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

fn shape_bytes() -> Option<Vec<u8>> {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return None;
    };
    let Ok(fa2) = media.archive("FA_2.LIB") else {
        eprintln!("skipped: no retail FA_2.LIB");
        return None;
    };
    Some(fa2.read("MF1.SH").unwrap())
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

fn extent(shape: &Shape) -> [f64; 3] {
    let (low, high) = bounds(shape);
    std::array::from_fn(|i| high[i] - low[i])
}

#[test]
fn the_pinned_extents_are_the_shapes_and_the_scaled_shape_is_real_size() {
    let Some(bytes) = shape_bytes() else {
        return;
    };
    // The gear word 0x5adc puts the wheels down (gear low point z -17).
    let down = Shape::with_state(&bytes, &BTreeMap::from([(0x5adc, 1)])).unwrap();
    assert_eq!(extent(&down), MF1_SHAPE_EXTENT);
    let (low, high) = bounds(&down);
    assert_eq!((low, high), ([-40., -45., -17.], [40., 82., 25.]));
    // The length is measured with the nose probe: the spike runs from the
    // radome end (y 68) to the tip (y 82), so the extent includes it.
    let neutral = Shape::parse(&bytes).unwrap();
    let nose: Vec<f32> = neutral
        .faces
        .iter()
        .flat_map(|f| f.positions.iter().map(|p| p[1]))
        .filter(|&y| y > 68.)
        .collect();
    assert!(!nose.is_empty(), "the probe spike lies beyond the radome");
    assert_eq!(nose.iter().cloned().fold(f32::MIN, f32::max), 82.);
    let factors = MF1_SHAPE_SCALE.map(|v| v as f32);
    let scaled = down.scaled(factors);
    for (axis, got) in extent(&scaled).into_iter().enumerate() {
        let feet = got / 3.;
        assert!(
            (feet - MF1_REAL_FEET[axis]).abs() / MF1_REAL_FEET[axis] < 0.001,
            "axis {axis}: {feet} ft against {}",
            MF1_REAL_FEET[axis]
        );
    }
    // A stored normal (right, up, forward) is perpendicular to its planar
    // face before and after the scale.
    let mut checked = 0;
    for (before, after) in down.faces.iter().zip(&scaled.faces) {
        let (Some(_), Some(n)) = (before.normal, after.normal) else {
            continue;
        };
        let n = [n[0], n[2], n[1]];
        let length = n.iter().map(|v| v * v).sum::<f32>().sqrt();
        if length == 0. {
            continue;
        }
        let p = &after.positions;
        for i in 1..p.len() {
            let edge: [f32; 3] = std::array::from_fn(|k| p[i][k] - p[0][k]);
            let edge_length = edge.iter().map(|v| v * v).sum::<f32>().sqrt();
            if edge_length < 1e-3 {
                continue;
            }
            let cos = (0..3).map(|k| n[k] * edge[k]).sum::<f32>() / (length * edge_length);
            // Source faces are quantized: only a face that was perpendicular
            // before must stay so.
            let q = &before.positions;
            let b: [f32; 3] = std::array::from_fn(|k| q[i][k] - q[0][k]);
            let bn = [
                before.normal.unwrap()[0],
                before.normal.unwrap()[2],
                before.normal.unwrap()[1],
            ];
            let bl = bn.iter().map(|v| v * v).sum::<f32>().sqrt();
            let bb = b.iter().map(|v| v * v).sum::<f32>().sqrt();
            if bb > 1e-3 && ((0..3).map(|k| bn[k] * b[k]).sum::<f32>() / (bl * bb)).abs() < 0.02 {
                assert!(
                    cos.abs() < 0.05,
                    "face {:x} edge {i}: cos {cos}",
                    after.address
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "only {checked} perpendicular edges checked");
}

/// MF1-2 skin review: every exact-vertex twin pair of `MF1.SH` (a
/// double-sided panel) carries opposite stored normals, so the smooth path
/// pairs and hides the rear member without the MiG-21 normal exception.
#[test]
fn every_double_sided_panel_is_an_exact_twin_with_opposite_normals() {
    let Some(bytes) = shape_bytes() else {
        return;
    };
    let shape = Shape::parse(&bytes).unwrap();
    assert_eq!(shape.faces.len(), 236);
    let key = |f: &tore_formats::shape::Face| {
        let mut p: Vec<[u32; 3]> = f.positions.iter().map(|p| p.map(f32::to_bits)).collect();
        p.sort();
        p
    };
    let mut groups: BTreeMap<Vec<[u32; 3]>, Vec<usize>> = BTreeMap::new();
    for (i, f) in shape.faces.iter().enumerate() {
        groups.entry(key(f)).or_default().push(i);
    }
    let twins: Vec<_> = groups.values().filter(|g| g.len() > 1).collect();
    assert_eq!(twins.len(), 23);
    for group in &twins {
        assert_eq!(group.len(), 2);
        let a = shape.faces[group[0]].normal.unwrap();
        let b = shape.faces[group[1]].normal.unwrap();
        let dot: f32 = (0..3).map(|k| a[k] * b[k]).sum();
        assert!(dot < 0., "faces {group:?} share a side: {a:?} {b:?}");
    }
}

/// The two E0 runtime decal slots (slots 3 and 4, the left and right wing
/// markings) exist only on the export path; the runtime path omits them and
/// they are not filled with a piece of the main skin.
#[test]
fn the_two_wing_decal_slots_are_omitted_on_the_runtime_path() {
    let Some(bytes) = shape_bytes() else {
        return;
    };
    let runtime = Shape::parse(&bytes).unwrap();
    assert!(runtime.faces.iter().all(|f| !f.texture.starts_with('@')));
    let export = Shape::with_export_state(&bytes, &BTreeMap::new()).unwrap();
    let slots: BTreeMap<usize, String> = export
        .faces
        .iter()
        .filter(|f| f.texture.starts_with('@'))
        .map(|f| (f.address, f.texture.clone()))
        .collect();
    assert_eq!(
        slots,
        BTreeMap::from([
            (0x2587, "@indexed:3".to_string()),
            (0x2d74, "@indexed:4".to_string())
        ])
    );
    assert_eq!(export.faces.len(), runtime.faces.len() + 2);
}
