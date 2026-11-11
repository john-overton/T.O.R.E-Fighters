//! F15E-2: the F-15E's shape skin review. Reads the retail `F15E.SH` and
//! checks that every exact-vertex twin pair (a double-sided panel) carries
//! opposite stored normals, so the smooth path pairs and hides the rear member
//! without the MiG-21 normal exception. It skips quietly without a retail
//! install and reads data only; no retail bytes are committed.
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::shape::{Face, Shape};
use tore_import::MediaSource;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

fn key(f: &Face) -> Vec<[u32; 3]> {
    let mut p: Vec<[u32; 3]> = f.positions.iter().map(|p| p.map(f32::to_bits)).collect();
    p.sort();
    p
}

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
    let shape = Shape::parse(&fa2.read("F15E.SH").unwrap()).unwrap();
    assert_eq!(shape.faces.len(), 350);
    let mut groups: BTreeMap<Vec<[u32; 3]>, Vec<usize>> = BTreeMap::new();
    for (i, f) in shape.faces.iter().enumerate() {
        groups.entry(key(f)).or_default().push(i);
    }
    let twins: Vec<_> = groups.values().filter(|g| g.len() > 1).collect();
    assert_eq!(twins.len(), 17);
    for group in twins {
        assert_eq!(group.len(), 2);
        let a = shape.faces[group[0]].normal.unwrap();
        let b = shape.faces[group[1]].normal.unwrap();
        let dot: f32 = (0..3).map(|k| a[k] * b[k]).sum();
        assert!(dot < 0., "faces {group:?} share a side: {a:?} {b:?}");
    }
}

/// F15E-2 damage looks: the four damage shapes and their skins are all in the
/// archive, parse, and carry the reviewed face counts. No variant is
/// registered for this identity (the F-15C has none either), so they stay
/// loaded by `DamageArt::load` for the regional cuts only.
#[test]
fn the_four_damage_shapes_and_skins_load_with_the_reviewed_counts() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let Ok(fa2) = media.archive("FA_2.LIB") else {
        eprintln!("skipped: no retail FA_2.LIB");
        return;
    };
    for (letter, faces) in [("A", 311), ("B", 138), ("C", 350), ("D", 33)] {
        let shape = Shape::parse(&fa2.read(&format!("F15E_{letter}.SH")).unwrap()).unwrap();
        assert_eq!(shape.faces.len(), faces, "F15E_{letter}.SH");
        assert!(
            fa2.read(&format!("_F15E_{letter}.PIC")).is_ok(),
            "_F15E_{letter}.PIC"
        );
    }
}
