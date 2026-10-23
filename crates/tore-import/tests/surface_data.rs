//! Ignored import tests over a user-owned retail source (slice IM1). Each one
//! imports into a fresh folder, loads the pack back and proves the surface
//! round can resolve every ground target from it. Run with
//!
//! ```text
//! TORE_GAME_DIR=/path/to/install cargo test -p tore-import --test surface_data -- --ignored --nocapture
//! TORE_DISC_DIR=/path/to/disc1  cargo test -p tore-import --test surface_data -- --ignored --nocapture
//! ```
//!
//! `TORE_GAME_DIR` defaults to the checkout's `gameassets/fighters-anthology`
//! link (the 1.02F install) and `TORE_DISC_DIR` to its `disc1` folder (the
//! 1.0 disc). Nothing here writes retail bytes outside the test's own scratch
//! folder, which is removed at the end.
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use tore_formats::{
    quick_template::{ObjectKind, Placeholder, Template, tables},
    surface_set::Family,
    surface_unit::SurfaceUnit,
};
use tore_import::{MediaSource, Resources, check_markers, import_with_progress, surface};

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

fn disc() -> PathBuf {
    std::env::var_os("TORE_DISC_DIR").map_or_else(|| install().join("disc1"), PathBuf::from)
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let base = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
        let path = base.join(format!("tore-im1-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Imports `source` into a fresh folder and returns the pack's resources, the
/// pack's size and the import report.
fn import(source: &Path, name: &str) -> (Resources, u64, String) {
    let media = MediaSource::detect(source).unwrap_or_else(|e| panic!("{}: {e}", source.display()));
    let scratch = Scratch::new(name);
    let imported = import_with_progress(&media, &scratch.0, &mut |_| {}, &|resources| {
        check_markers(resources)?;
        Ok(())
    })
    .expect("the import succeeds");
    let pack = std::fs::read_dir(&scratch.0)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().ends_with(".pack"))
        .expect("a pack");
    let size = pack.metadata().unwrap().len();
    let report = std::fs::read_to_string(scratch.0.join("import-report.txt")).unwrap();
    eprintln!(
        "{name}: {} resources, pack {size} bytes",
        imported.resources.len()
    );
    (imported.resources, size, report)
}

/// Everything the surface round can ask a pack for.
fn resolves_every_ground_target(resources: &Resources) {
    assert!(surface::present(resources), "the surface marker");
    check_markers(resources).unwrap();
    // The 129 templates, by the executable's table and by the archive.
    let expected: BTreeSet<String> = tables::TEMPLATES
        .iter()
        .flat_map(|list| list.iter())
        .chain(tables::UNREFERENCED.iter())
        .map(|stem| format!("~{stem}.M"))
        .collect();
    assert_eq!(expected.len(), 129);
    let held: BTreeSet<String> = surface::templates(resources)
        .map(|(name, _)| name.to_owned())
        .collect();
    assert_eq!(held, expected);

    // Every type a template names resolves to a record the pack holds.
    let mut named = BTreeSet::new();
    for (name, bytes) in surface::templates(resources) {
        let template = Template::parse(name, bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        for object in &template.objects {
            if let ObjectKind::Named(type_name) = &object.kind {
                named.insert(type_name.clone());
            }
        }
    }
    // Every type an equipment list can turn a placeholder into, for every
    // equipment group.
    let mut listed = BTreeSet::new();
    for placeholder in Placeholder::ALL {
        for group in 0..tables::GROUPS {
            for name in tables::equipment(placeholder, group).unwrap_or(&[]) {
                listed.insert((*name).to_owned());
            }
        }
    }
    for name in tables::NIGHT_AAA {
        listed.insert((*name).to_owned());
    }
    assert!(
        named.len() > 100 && listed.len() > 40,
        "{named:?} {listed:?}"
    );
    for name in named.iter().chain(listed.iter()) {
        assert!(resources.contains_key(name), "{name} is not in the pack");
    }
    // The units: shape, damaged look, weapons, sensor.
    for name in resources.keys().filter(|n| n.ends_with(".NT")) {
        let unit = SurfaceUnit::parse(&resources[name]).unwrap_or_else(|e| panic!("{name}: {e}"));
        for part in unit
            .shape
            .iter()
            .chain(unit.shadow_shape.iter())
            .chain(unit.damaged_shape.iter().filter(|_| unit.is_ship()))
            .chain(unit.sensor.iter())
            .chain(unit.mounts.iter().filter_map(|m| m.store.as_ref()))
        {
            assert!(resources.contains_key(part), "{name}: {part}");
        }
    }
    // Wrecks and the damaged-bunker variants.
    for name in ["DEST.OT", "DEST.SH", "~BNK5.OT", "~BNK6.OT", "~BNK8.OT"] {
        assert!(resources.contains_key(name), "{name}");
    }
    // The shapes the surface round could not draw before, with their textures.
    for name in [
        "KRIV.SH",
        "_KRIV.PIC",
        "KRIV_A.SH",
        "SA3.SH",
        "SOLDIER.SH",
        "SOLDIER.PIC",
        "SOVR.SH",
        "SCD.SH",
        "NIMZ.SH",
        "NIMZT.SH",
        "KITT.SH",
        "KITTT.SH",
        "CLEM.SH",
        "CLEMT.SH",
        "WASP.SH",
        "WASPT.SH",
    ] {
        assert!(resources.contains_key(name), "{name}");
    }
    // The pack as a whole: nothing a template, list or layout can name is
    // missing, and every texture the shape reader finds is kept.
    let missing = surface::missing(resources).unwrap();
    assert!(missing.is_empty(), "{missing:#?}");
    let selection = tore_formats::surface_set::select(resources).unwrap();
    eprintln!(
        "{} templates, {} units, {} objects, {} aircraft, {} surface resources",
        selection.templates.len(),
        selection.count(Family::Unit),
        selection.count(Family::Object),
        selection.count(Family::Aircraft),
        selection.resources.len()
    );
    assert_eq!(selection.count(Family::Unit), 84);
    // The aircraft parked in the templates: record and shapes.
    for name in selection.types[&Family::Aircraft].iter() {
        let definition = tore_formats::static_object::Definition::parse(&resources[name]).unwrap();
        let shape = definition
            .main_shape
            .expect("a parked aircraft has a shape");
        assert!(resources.contains_key(&shape), "{name}: {shape}");
    }
}

#[test]
#[ignore = "needs a retail install (TORE_GAME_DIR or the gameassets link)"]
fn the_installed_game_imports_every_ground_target() {
    let (resources, size, report) = import(&install(), "installed");
    resolves_every_ground_target(&resources);
    assert!(report.contains("Surface data: 129 templates"), "{report}");
    assert!(!report.contains("Surface data unavailable"), "{report}");
    eprintln!("installed pack: {size} bytes");
}

#[test]
#[ignore = "needs the 1.0 disc (TORE_DISC_DIR or the gameassets link's disc1)"]
fn the_disc_imports_every_ground_target() {
    let (resources, size, report) = import(&disc(), "disc");
    resolves_every_ground_target(&resources);
    assert!(report.contains("Surface data: 129 templates"), "{report}");
    assert!(!report.contains("Surface data unavailable"), "{report}");
    eprintln!("disc pack: {size} bytes");
}
