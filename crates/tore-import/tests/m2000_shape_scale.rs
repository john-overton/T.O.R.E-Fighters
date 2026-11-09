//! M2K-2: the Mirage 2000's per-axis shape scale was fitted to the measured
//! `M20.SH` extents. This reads the retail shape and checks that the pinned
//! extents are the real ones, that the scaled shape lands on the real
//! dimensions, and that every stored face normal stays perpendicular to its
//! scaled face. It skips quietly without a retail install and reads data only;
//! no retail bytes are committed.
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{
    aircraft::scale::{M2000_REAL_FEET, M2000_SHAPE_EXTENT, M2000_SHAPE_SCALE},
    shape::Shape,
};
use tore_import::MediaSource;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

fn extent(shape: &Shape) -> [f64; 3] {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for p in shape.faces.iter().flat_map(|f| &f.positions) {
        for i in 0..3 {
            low[i] = low[i].min(f64::from(p[i]));
            high[i] = high[i].max(f64::from(p[i]));
        }
    }
    std::array::from_fn(|i| high[i] - low[i])
}

#[test]
fn the_pinned_extents_are_the_shapes_and_the_scaled_shape_is_real_size() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let Ok(fa2) = media.archive("FA_2.LIB") else {
        eprintln!("skipped: no retail FA_2.LIB");
        return;
    };
    let bytes = fa2.read("M20.SH").unwrap();
    // The gear word 0x589c puts the wheels down (gear low point z -12).
    let down = Shape::with_state(&bytes, &BTreeMap::from([(0x589c, 1)])).unwrap();
    assert_eq!(extent(&down), M2000_SHAPE_EXTENT);
    let factors = M2000_SHAPE_SCALE.map(|v| v as f32);
    let scaled = down.scaled(factors);
    for (axis, got) in extent(&scaled).into_iter().enumerate() {
        let feet = got / 3.;
        assert!(
            (feet - M2000_REAL_FEET[axis]).abs() / M2000_REAL_FEET[axis] < 0.001,
            "axis {axis}: {feet} ft against {}",
            M2000_REAL_FEET[axis]
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

/// M2K-2 skin review: every exact-vertex twin pair of `M20.SH` (a
/// double-sided panel) carries opposite stored normals, so the smooth path
/// pairs and hides the rear member without the MiG-21 normal exception.
#[test]
fn every_double_sided_panel_is_an_exact_twin_with_opposite_normals() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let Ok(fa2) = media.archive("FA_2.LIB") else {
        eprintln!("skipped: no retail FA_2.LIB");
        return;
    };
    let shape = Shape::parse(&fa2.read("M20.SH").unwrap()).unwrap();
    assert_eq!(shape.faces.len(), 206);
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
    assert_eq!(twins.len(), 25);
    for group in twins {
        assert_eq!(group.len(), 2);
        let a = shape.faces[group[0]].normal.unwrap();
        let b = shape.faces[group[1]].normal.unwrap();
        let dot: f32 = (0..3).map(|k| a[k] * b[k]).sum();
        assert!(dot < 0., "faces {group:?} share a side: {a:?} {b:?}");
    }
}
