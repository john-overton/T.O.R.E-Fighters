//! F5E-2: the F-5E's shape, skin and damage looks. Reads the retail shapes as
//! data and pins what the review found: the F-5E draws its own `F5E.SH` and
//! `_F5E.PIC` (never the desert or snow record's), every double-sided panel is
//! an exact-vertex twin with opposite stored normals, the two E0 wing decal
//! slots are omitted by the runtime path, and the four damage looks carry
//! their own textures. It skips quietly without a retail install and reads data
//! only; no retail bytes are committed.
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{
    aircraft::{AircraftId, scale},
    shape::{Face, Shape},
};
use tore_import::MediaSource;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

fn fa2() -> Option<tore_formats::Archive> {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return None;
    };
    let archive = media.archive("FA_2.LIB").ok();
    if archive.is_none() {
        eprintln!("skipped: no retail FA_2.LIB");
    }
    archive
}

fn extent(shape: &Shape) -> ([f32; 3], [f32; 3]) {
    let mut low = [f32::INFINITY; 3];
    let mut high = [f32::NEG_INFINITY; 3];
    for p in shape.faces.iter().flat_map(|f| &f.positions) {
        for i in 0..3 {
            low[i] = low[i].min(p[i]);
            high[i] = high[i].max(p[i]);
        }
    }
    (low, high)
}

fn key(f: &Face) -> Vec<[u32; 3]> {
    let mut p: Vec<[u32; 3]> = f.positions.iter().map(|p| p.map(f32::to_bits)).collect();
    p.sort();
    p
}

/// The F-5E draws its own shape and skin; the desert (`F5EE`) and snow
/// (`F5EV`) records are separate identities with their own shape bytes, even
/// though the desert skins are byte-identical to the F-5E's.
#[test]
fn the_f5e_draws_its_own_shape_and_skin_and_aliases_no_other_record() {
    let Some(fa2) = fa2() else { return };
    assert_eq!(AircraftId::F5E.stem(), "F5E");
    assert_eq!(scale::shape_axis_scale(AircraftId::F5E), [1., 1., 1.]);
    let own = fa2.read("F5E.SH").unwrap();
    for other in ["F5EE.SH", "F5EV.SH"] {
        assert_ne!(own, fa2.read(other).unwrap(), "{other} must stay separate");
    }
    let shape = Shape::parse(&own).unwrap();
    assert_eq!(shape.faces.len(), 253);
    assert_eq!(
        shape.state_words.iter().copied().collect::<Vec<_>>(),
        [0x63c0, 0x63c6, 0x63cc, 0x63d8, 0x63de, 0x63e4]
    );
    // Every runtime face is textured with the F-5E's own skin: no decal slot
    // and no other record's picture.
    assert!(shape.faces.iter().all(|f| f.texture == "_F5E.PIC"));
    for desert in [
        "_F5EE.PIC",
        "_F5EE_A.PIC",
        "_F5EE_B.PIC",
        "_F5EE_C.PIC",
        "_F5EE_D.PIC",
    ] {
        // The importer's `_F5E` prefix also carries these; they are never
        // referenced by the F-5E's shapes.
        assert!(shape.faces.iter().all(|f| f.texture != desert));
    }
    let shadow = Shape::parse(&fa2.read("F5E_S.SH").unwrap()).unwrap();
    assert_eq!(shadow.faces.len(), 12);
}

/// Measured extents (shape units, x right, y forward, z up) and the gear-down
/// low point the vehicle model's 18 unit clearance comes from.
#[test]
fn the_pinned_extents_are_the_shapes() {
    let Some(fa2) = fa2() else { return };
    let bytes = fa2.read("F5E.SH").unwrap();
    let neutral = Shape::with_state(&bytes, &BTreeMap::new()).unwrap();
    assert_eq!(extent(&neutral), ([-44., -48., -5.], [44., 87., 24.]));
    let down = Shape::with_state(&bytes, &BTreeMap::from([(0x63cc, 1)])).unwrap();
    assert_eq!(down.faces.len(), 259);
    assert_eq!(extent(&down), ([-44., -48., -18.], [44., 87., 24.]));
}

/// Every exact-vertex twin pair of `F5E.SH` (a double-sided panel) carries
/// opposite stored normals, so the smooth path pairs and hides the rear
/// member without the MiG-21 normal exception.
#[test]
fn every_double_sided_panel_is_an_exact_twin_with_opposite_normals() {
    let Some(fa2) = fa2() else { return };
    let shape = Shape::parse(&fa2.read("F5E.SH").unwrap()).unwrap();
    let mut groups: BTreeMap<Vec<[u32; 3]>, Vec<usize>> = BTreeMap::new();
    for (i, f) in shape.faces.iter().enumerate() {
        groups.entry(key(f)).or_default().push(i);
    }
    let twins: Vec<_> = groups.values().filter(|g| g.len() > 1).collect();
    assert_eq!(twins.len(), TWIN_GROUPS);
    for group in twins {
        assert_eq!(group.len(), 2);
        let a = shape.faces[group[0]].normal.unwrap();
        let b = shape.faces[group[1]].normal.unwrap();
        let dot: f32 = (0..3).map(|k| a[k] * b[k]).sum();
        assert!(dot < 0., "faces {group:?} share a side: {a:?} {b:?}");
    }
}
const TWIN_GROUPS: usize = 8;

/// The two E0 decal slots sit on the wing, slot 4 (`3c2f`, x 18 to 30) and
/// slot 3 (`40b9`, x -30 to -18), y -4 to 8. The runtime path omits both and
/// fills neither; only the export path lists them.
#[test]
fn the_two_wing_decal_slots_are_omitted_by_the_runtime_path() {
    let Some(fa2) = fa2() else { return };
    let bytes = fa2.read("F5E.SH").unwrap();
    let runtime = Shape::with_state(&bytes, &BTreeMap::new()).unwrap();
    let export = Shape::with_export_state(&bytes, &BTreeMap::new()).unwrap();
    assert_eq!(runtime.faces.len(), 253);
    assert_eq!(export.faces.len(), 255);
    let slots: Vec<_> = export
        .faces
        .iter()
        .filter(|f| f.texture.starts_with("@indexed:"))
        .collect();
    assert_eq!(slots.len(), 2);
    for (face, (address, texture, side)) in slots
        .iter()
        .zip([(0x3c2f, "@indexed:4", 1.), (0x40b9, "@indexed:3", -1.)])
    {
        assert_eq!((face.address, face.texture.as_str()), (address, texture));
        for p in &face.positions {
            assert!((18. ..=30.).contains(&(p[0] * side)), "{p:?}");
            assert!((-4. ..=8.).contains(&p[1]), "{p:?}");
        }
    }
    assert!(
        runtime
            .faces
            .iter()
            .all(|f| !f.texture.is_empty() && !f.texture.starts_with('@'))
    );
}

/// The four damage looks load as the F-5E's own shapes with their own skins.
#[test]
fn the_damage_looks_are_the_f5es_own_shapes_and_skins() {
    let Some(fa2) = fa2() else { return };
    let intact = Shape::parse(&fa2.read("F5E.SH").unwrap()).unwrap();
    let (_, intact_high) = extent(&intact);
    let looks = [
        ("A", 228, "_F5E_A.PIC"),
        ("B", 128, "_F5E_B.PIC"),
        ("C", 234, "_F5E_C.PIC"),
        ("D", 24, "_F5E_D.PIC"),
    ];
    let mut shapes = BTreeMap::new();
    for (letter, faces, texture) in looks {
        let shape = Shape::parse(&fa2.read(&format!("F5E_{letter}.SH")).unwrap()).unwrap();
        assert_eq!(shape.faces.len(), faces, "F5E_{letter}");
        assert!(shape.faces.iter().all(|f| f.texture == texture));
        assert!(shape.state_words.is_empty());
        shapes.insert(letter, shape);
    }
    // A is the body with the nose cut away at y 35 (B is the nose piece); C is
    // the body with the fin cut to z 13 (D is a flat fin plate on x 0).
    let (_, a_high) = extent(&shapes["A"]);
    assert_eq!(a_high[1], 35.);
    assert!(a_high[1] < intact_high[1]);
    let (_, c_high) = extent(&shapes["C"]);
    assert_eq!(c_high[2], 13.);
    assert!(c_high[2] < intact_high[2]);
    let (d_low, d_high) = extent(&shapes["D"]);
    assert_eq!((d_low[0], d_high[0]), (0., 0.));
    // No damage look is a subset of the intact shape: each is its own mesh.
    let intact_keys: Vec<_> = intact.faces.iter().map(key).collect();
    for letter in ["A", "C"] {
        let shared = shapes[letter]
            .faces
            .iter()
            .filter(|f| intact_keys.contains(&key(f)))
            .count();
        assert_eq!(shared, 0, "F5E_{letter} shares faces with F5E.SH");
    }
}
