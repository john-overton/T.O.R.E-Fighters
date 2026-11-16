//! T26-2: the Tu-26 draws `TU26.SH` from `FA_2.LIB` at the shape's header scale
//! (10, so 4/3 ft per unit): 4 to 10 percent under the real Tu-22M3, the
//! coordinator's accepted fit (no per-axis factor). This reads the retail
//! shape, its twin panels, decal slots and damage shapes and pins the numbers
//! the slice's notes quote. It skips quietly without a retail install and
//! reads data only; no retail bytes are committed. Evidence:
//! docs/formats/aircraft-variety.md, "Tu-26 shape and rig (T26-2/3, 2026-11-15)".
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{
    module,
    shape::{Face, Shape, object_scale},
};
use tore_import::MediaSource;

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

/// `TU26.SH` has header scale 10 (4/3 ft a unit) and, drawn there, measures
/// 101.3 ft span, 130.7 ft length and 34.7 ft height gear up (44 ft with the
/// gear down) against the real Tu-22M3's 112.5 / 139.3 / 36.25; the exact-vertex
/// twin groups are double-sided panels with opposite stored normals; the damage
/// shapes share the header scale.
#[test]
fn tu26_sh_size_twins_and_damage_looks() {
    let Some(bytes) = read("TU26.SH") else {
        return;
    };
    assert_eq!(module::code(&bytes).unwrap().0.len(), 26_812);
    assert_eq!(object_scale(&bytes).unwrap(), 4.);
    let neutral = Shape::parse(&bytes).unwrap();
    assert_eq!(neutral.faces.len(), 310);
    let down = Shape::with_state(&bytes, &BTreeMap::from([(0x7892, 1)])).unwrap();
    let (low, high) = bounds(&neutral.faces);
    let (down_low, _) = bounds(&down.faces);
    assert_eq!(
        ([low[0], high[0]], [low[1], high[1]]),
        ([-38., 38.], [-36., 62.])
    );
    assert_eq!((low[2], high[2], down_low[2]), (-4., 22., -11.));
    let feet = |lo: f64, hi: f64| (hi - lo) * 4. / 3.;
    for (got, real, axis, floor) in [
        (feet(low[0], high[0]), 112.5, "span", 0.89),
        (feet(low[1], high[1]), 139.3, "length", 0.93),
        (feet(low[2], high[2]), 36.25, "height", 0.95),
    ] {
        assert!(got < real && got / real > floor, "{axis}: {got} ft");
    }
    let mut groups: BTreeMap<Vec<[u32; 3]>, Vec<usize>> = BTreeMap::new();
    for (i, f) in neutral.faces.iter().enumerate() {
        groups.entry(twin_key(f)).or_default().push(i);
    }
    let mut same_side = Vec::new();
    let mut twins = 0;
    for group in groups.values().filter(|g| g.len() > 1) {
        twins += 1;
        let a = neutral.faces[group[0]].normal.unwrap();
        let b = neutral.faces[group[1]].normal.unwrap();
        if (0..3).map(|k| a[k] * b[k]).sum::<f32>() >= 0. {
            same_side.push(neutral.faces[group[0]].address);
        }
    }
    eprintln!("{twins} twin groups, same-side {same_side:x?}");
    let export = Shape::with_export_state(&bytes, &BTreeMap::new()).unwrap();
    let runtime: std::collections::BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
    let extra: Vec<_> = export
        .faces
        .iter()
        .filter(|f| !runtime.contains(&f.address))
        .collect();
    eprintln!(
        "export {} runtime {}; export-only: {:?}",
        export.faces.len(),
        neutral.faces.len(),
        extra
            .iter()
            .map(|f| (f.address, bounds(std::slice::from_ref(f))))
            .collect::<Vec<_>>()
    );
    for (name, faces) in [
        ("TU26_A.SH", 322),
        ("TU26_B.SH", 41),
        ("TU26_C.SH", 335),
        ("TU26_D.SH", 44),
    ] {
        let bytes = read(name).unwrap();
        assert_eq!(object_scale(&bytes).unwrap(), 4., "{name}");
        let shape = Shape::parse(&bytes).unwrap();
        assert_eq!(shape.faces.len(), faces, "{name}");
        eprintln!("{name}: {:?}", bounds(&shape.faces));
    }
    let def = tore_formats::shape::StreamerDef::parse(&bytes)
        .unwrap()
        .expect("streamer record");
    eprintln!(
        "streamer {:?} {:?}",
        def.attachment(0, 0),
        def.attachment(1, 0)
    );
}
