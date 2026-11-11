//! GRI-2: the Gripen's shape, skin and damage looks read from the retail
//! `GRI.SH` family. Pins the review facts (true size so no scale, double-sided
//! panel twins, the five omitted decal slots, the four damage looks and their
//! textures). It skips quietly without a retail install and reads data only;
//! no retail bytes are committed.
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{
    Archive, Pic,
    aircraft::{AircraftId, scale::shape_axis_scale},
    shape::{Face, Shape},
};
use tore_import::MediaSource;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

fn archive() -> Option<Archive> {
    let Ok(media) = MediaSource::detect(&install()) else {
        eprintln!("skipped: no retail install at {}", install().display());
        return None;
    };
    let Ok(fa2) = media.archive("FA_2.LIB") else {
        eprintln!("skipped: no retail FA_2.LIB");
        return None;
    };
    Some(fa2)
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

/// The review measured the shape at true size, so the Gripen keeps the
/// identity scale; the gear-down extents land within 2% of the JAS 39A span,
/// length and height (27.56, 46.25 and 14.76 ft).
#[test]
fn the_gripen_shape_is_true_size_with_no_scale() {
    assert_eq!(shape_axis_scale(AircraftId::Gripen), [1., 1., 1.]);
    let Some(fa2) = archive() else { return };
    let bytes = fa2.read("GRI.SH").unwrap();
    assert_eq!(Shape::parse(&bytes).unwrap().faces.len(), 383);
    // The gear word 0x808c puts the wheels down (gear low point z -16).
    let down = Shape::with_state(&bytes, &BTreeMap::from([(0x808c, 1)])).unwrap();
    let size = extent(&down);
    assert_eq!(size, [82., 138., 45.]);
    for (got, real) in size.into_iter().zip([27.56, 46.25, 14.76]) {
        let feet = got / 3.;
        assert!(
            (feet - real).abs() / real < 0.02,
            "{feet} ft against {real} ft"
        );
    }
}

/// Every exact-vertex twin pair (a double-sided panel, 18 of the 19 groups) carries opposite stored
/// normals, so the smooth path pairs and hides the rear member without the
/// MiG-21 normal exception.
#[test]
fn every_double_sided_panel_is_an_exact_twin_with_opposite_normals() {
    let Some(fa2) = archive() else { return };
    let shape = Shape::parse(&fa2.read("GRI.SH").unwrap()).unwrap();
    let key = |f: &Face| {
        let mut p: Vec<[u32; 3]> = f.positions.iter().map(|p| p.map(f32::to_bits)).collect();
        p.sort();
        p
    };
    let mut groups: BTreeMap<Vec<[u32; 3]>, Vec<usize>> = BTreeMap::new();
    for (i, f) in shape.faces.iter().enumerate() {
        groups.entry(key(f)).or_default().push(i);
    }
    let twins: Vec<_> = groups.values().filter(|g| g.len() > 1).collect();
    assert_eq!(twins.len(), 19);
    let mut degenerate = 0;
    for group in twins {
        assert_eq!(group.len(), 2);
        let distinct = |i: usize| {
            let mut p: Vec<[u32; 3]> = shape.faces[i]
                .positions
                .iter()
                .map(|p| p.map(f32::to_bits))
                .collect();
            p.sort();
            p.dedup();
            p.len()
        };
        // One pair (addresses 2856 and 286e, an inboard-wing sliver at
        // x 8, y 30 to 38, z -5) has only two distinct corners, so it has no
        // area, draws nothing and carries meaningless stored normals.
        if distinct(group[0]) < 3 {
            degenerate += 1;
            assert_eq!(
                (shape.faces[group[0]].address, shape.faces[group[1]].address),
                (0x2856, 0x286e)
            );
            continue;
        }
        let a = shape.faces[group[0]].normal.unwrap();
        let b = shape.faces[group[1]].normal.unwrap();
        let dot: f32 = (0..3).map(|k| a[k] * b[k]).sum();
        assert!(dot < 0., "faces {group:?} share a side: {a:?} {b:?}");
    }
    assert_eq!(degenerate, 1);
}

/// The export path keeps the five `E0` runtime decal slots the runtime
/// projector omits: nose art (slot 2), the two sides of the fin (slots 0 and
/// one) and the two wing markings (slots 3 and 4). None is a piece of the main
/// skin.
#[test]
fn the_five_decal_slots_are_omitted_from_the_runtime_shape() {
    let Some(fa2) = archive() else { return };
    let bytes = fa2.read("GRI.SH").unwrap();
    let runtime = Shape::parse(&bytes).unwrap();
    assert!(runtime.faces.iter().all(|f| !f.texture.starts_with('@')));
    let export = Shape::with_export_state(&bytes, &BTreeMap::new()).unwrap();
    assert_eq!(export.faces.len(), 388);
    let mut slots: Vec<(String, usize)> = export
        .faces
        .iter()
        .filter(|f| f.texture.starts_with('@'))
        .map(|f| (f.texture.clone(), f.address))
        .collect();
    slots.sort();
    assert_eq!(
        slots,
        [
            ("@indexed:0".to_string(), 0x4b7a),
            ("@indexed:1".to_string(), 0x4ac6),
            ("@indexed:2".to_string(), 0x2edb),
            ("@indexed:3".to_string(), 0x5288),
            ("@indexed:4".to_string(), 0x57ee),
        ]
    );
}

/// The four damage looks load: face counts, one shared texture each, and no
/// private palette (the damage atlas refuses one).
#[test]
fn the_four_damage_looks_load_with_their_textures() {
    let Some(fa2) = archive() else { return };
    for (suffix, faces) in [("A", 409), ("B", 50), ("C", 296), ("D", 256)] {
        let shape = Shape::parse(&fa2.read(&format!("GRI_{suffix}.SH")).unwrap()).unwrap();
        assert_eq!(shape.faces.len(), faces, "GRI_{suffix}.SH");
        let name = format!("_GRI_{suffix}.PIC");
        for f in shape.faces.iter().filter(|f| !f.texture.is_empty()) {
            assert!(
                f.texture == name || f.texture == "_GRI.PIC" || f.texture == "_F18_A.PIC",
                "GRI_{suffix}.SH names {}",
                f.texture
            );
        }
        let pic = Pic::parse(&fa2.read(&name).unwrap()).unwrap();
        assert!(pic.palette.is_empty(), "{name} has a private palette");
    }
}
