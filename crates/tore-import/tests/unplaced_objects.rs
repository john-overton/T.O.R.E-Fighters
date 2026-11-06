//! Headless import check (slice OT-3): the ten OT records that no scanned
//! template or base layout places still have to load and project, so an M3
//! mission placement can rely on them.
//!
//! The check reads the user's own install (the `gameassets` link or
//! `TORE_GAME_DIR`) through the importer's media source and skips quietly
//! without it. It reads records only; no retail bytes are committed.
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{shape::Shape, static_object::Definition, surface_set::shape_textures};
use tore_import::MediaSource;

/// The records and the main shape each names
/// (docs/research/m3-asset-inventory.md, "Static objects (OT)").
const UNPLACED: [(&str, &str); 10] = [
    ("APTB1.OT", "APTB1.SH"),
    ("COMM1.OT", "COMM1.SH"),
    ("COMM2.OT", "COMM2.SH"),
    ("DKHOS2.OT", "DKHOS2.SH"),
    ("ROAD2.OT", "ROAD2.SH"),
    ("ROADC.OT", "ROADC.SH"),
    ("SGRP1.OT", "SGRP1.SH"),
    ("SGRP2.OT", "SGRP2.SH"),
    ("IND2.OT", "IND2.SH"),
    ("OILW.OT", "RIG1.SH"),
];

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

#[test]
fn unplaced_static_objects_load_and_project() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let (Ok(fa1), Ok(fa2)) = (media.archive("FA_1.LIB"), media.archive("FA_2.LIB")) else {
        eprintln!("skipped: no retail FA_1.LIB and FA_2.LIB");
        return;
    };
    let libraries = [&fa1, &fa2];
    for (object, shape_name) in UNPLACED {
        let library = libraries
            .iter()
            .find(|library| library.entries.contains_key(object))
            .unwrap_or_else(|| panic!("{object} is in neither archive"));
        let bytes = library.read(object).unwrap();
        let definition =
            Definition::parse(&bytes).unwrap_or_else(|error| panic!("{object}: {error}"));
        assert_eq!(definition.resource_name, object, "{object} names itself");
        assert_eq!(
            definition.main_shape.as_deref(),
            Some(shape_name),
            "{object} main shape"
        );
        assert!(
            definition.callbacks.iter().any(|name| name == "_OBJProc"),
            "{object} is a static object"
        );

        let shape_bytes = library
            .read(shape_name)
            .unwrap_or_else(|error| panic!("{object}: {shape_name}: {error}"));
        let scenery = Shape::scenery(&shape_bytes)
            .unwrap_or_else(|error| panic!("{shape_name} scenery: {error}"));
        assert!(
            !scenery.faces.is_empty(),
            "{shape_name} projects no scenery faces"
        );
        for (path, projected) in [
            ("state", Shape::with_state(&shape_bytes, &BTreeMap::new())),
            (
                "export",
                Shape::with_export_state(&shape_bytes, &BTreeMap::new()),
            ),
        ] {
            let shape = projected.unwrap_or_else(|error| panic!("{shape_name} {path}: {error}"));
            assert!(
                !shape.faces.is_empty(),
                "{shape_name} projects no {path} faces"
            );
        }
        for texture in shape_textures(&shape_bytes) {
            assert!(
                libraries
                    .iter()
                    .any(|library| library.entries.contains_key(&texture)),
                "{object}: {shape_name} names {texture}, which the media lacks"
            );
        }
    }
}
